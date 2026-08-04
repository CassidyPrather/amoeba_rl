//! macroquad frontend: a window, a font atlas, and one command a turn.
//!
//! Everything that decides what happens lives in [`amoeba_rl::sim`], which
//! resolves the world all the way down to a glyph and two colours per cell
//! before this file sees it. What is left is genuinely only presentation:
//! where the panels go ([`render`]), what goes in them ([`hud`]), which pixels
//! a character is ([`tileset`]), which key or thumb meant which [`Command`]
//! ([`input`]), and what the player has asked for ([`settings`]).
//!
//! The loop is deliberately plain. The game is turn-based, so there is no
//! timestep and nothing to interpolate: read the world, draw it, hand the sim
//! at most one command, repeat. No clock reaches the sim — the blink counter
//! below is passed in as an argument, keeping [`Sim::view`] the pure function
//! of world and frame that it claims to be.
//!
//! The sim reports what happened twice, and the frontend's two other halves
//! each take one of those reports. [`audio`] turns
//! [`Cue`](amoeba_rl::sim::Cue)s — a deduplicated set — into noise, and
//! [`anim`] turns [`Effect`](amoeba_rl::sim::effect::Effect)s — the same news
//! in order and with cells attached — into overlays on the map. Those two and
//! the frame clock are the only places allowed to read a wall clock.
//!
//! ## Holding a command back
//!
//! An animation is the one thing in here that takes time, so it is the one
//! thing that can get between a keypress and a turn. The rule is that it never
//! does: while a turn is showing itself the sim is fed nothing, but any command
//! asked for in the meantime *cuts the animation short* and is kept in
//! `queued` for the very next frame. An idle player watches the whole thing; a
//! player in a hurry loses a frame and no keypress.

mod anim;
mod audio;
mod hud;
mod input;
mod links;
mod render;
mod settings;
mod tileset;

use macroquad::math::Rect;
use macroquad::time::{get_frame_time, get_time};
use macroquad::window::{Conf, next_frame, screen_height, screen_width};

use amoeba_rl::sim::grid::Coord;
use amoeba_rl::sim::{Command, Difficulty, InputStyle, Phase, RenderView, Sim};

use anim::Anim;
use audio::Audio;
use input::{Controls, Input};
use render::{Frontend, Layout};
use settings::Settings;
use tileset::Tileset;

/// Seconds per animation step, as the original's `ANIMATION_RATE` was.
const ANIMATION_RATE: f64 = 0.25;

fn window_conf() -> Conf {
    Conf {
        window_title: "Amoeba Roguelike Remastered".to_owned(),
        // 86 x 59 cells of the 12 px font: the console the original opened.
        window_width: 1032,
        window_height: 708,
        high_dpi: true,
        ..Conf::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let font = Tileset::load();
    let mut sim = Sim::new(input::fresh_seed(), Difficulty::Normal);
    let mut input = Input::new();
    let mut audio = Audio::load().await;
    let mut settings = Settings::new();
    let mut anim = Anim::new();
    // The one command held back while a turn is showing itself.
    let mut queued: Option<Command> = None;

    loop {
        let dt = get_frame_time();
        let view = sim.view(anim_frame());
        // Three passes over the same window, because the pad and the panels
        // each need to know something about the other: which layout is on
        // screen decides whether there is a pad, the pad decides how much of
        // the window is left, and what is left decides where the panels go.
        // The middle step only ever takes room away, and taking room away can
        // only turn a wide layout narrow — never the other way about — so the
        // first pass's answer to "is there a pad?" survives the third.
        let probe = Layout::fit(
            Rect::new(0.0, 0.0, screen_width(), screen_height()),
            view.width,
            view.height,
            settings.log,
            false,
        );
        let touch = input.wants_controls(probe.mode);
        let area = if touch {
            Controls::free(screen_width(), screen_height())
        } else {
            Rect::new(0.0, 0.0, screen_width(), screen_height())
        };
        let layout = Layout::fit(area, view.width, view.height, settings.log, touch);
        let camera = if layout.scrolls {
            render::camera_origin(
                focus(&view),
                layout.cols,
                layout.rows,
                view.width,
                view.height,
            )
        } else {
            Coord::new(0, 0)
        };
        // The pad steers the amoeba, so it only belongs on screen while there
        // is an amoeba: the title and post-mortem screens have buttons of
        // their own, and the settings panel is a modal with its own way out.
        // The room it claimed stays claimed through all of them, so the map
        // does not jump about underneath a panel that is about to close.
        let controls = Controls::fit(
            screen_width(),
            screen_height(),
            touch && view.phase == Phase::Playing && !settings.is_open(),
        );
        let frame = input.gather(&view, &layout, camera, &controls, &mut settings, dt);
        render::draw(
            &font,
            &view,
            &layout,
            camera,
            &Frontend {
                controls: &controls,
                page: input.page(),
                audio: audio.hint(layout.touch),
                sound_on: !audio.muted(),
                settings: &settings,
                anim: &anim,
                pulse: telegraph_phase(),
            },
        );

        let command = if anim.playing() {
            // Somebody in a hurry is somebody who has stopped watching, so a
            // command arriving now ends the show and waits one frame. A quiet
            // frame leaves whatever was already waiting alone.
            if frame.command.is_some() {
                anim.skip();
                queued = frame.command;
            }
            None
        } else {
            // The newest ask wins, and the older one is dropped rather than
            // kept: it would arrive after the thing that superseded it.
            let waiting = queued.take();
            frame.command.or(waiting)
        };
        anim.tick(dt);

        // The sim writes a few lines that name controls; this is the only
        // thing it needs from out here to name them the way the buttons do.
        // After `gather` rather than before, because the very first tap a
        // touchscreen ever gets is the one that picks a difficulty — and that
        // is the command that makes the sim write the help.
        sim.set_input_style(if input.wants_controls(layout.mode) {
            InputStyle::Touch
        } else {
            InputStyle::Keys
        });
        sim.advance(command);
        // Both reports live for exactly this one `advance`, so both halves read
        // them here — the cues alongside the mute toggle from the same frame,
        // and the effects for however long they take to draw. A frame that
        // resolved nothing hands over two empty lists and disturbs neither.
        audio.update(sim.cues(), &frame);
        anim.load(sim.effects(), &settings);

        next_frame().await;
    }
}

/// The blink counter, ticking four times a second of wall time.
///
/// Wall time is fine out here — the sim takes this as an argument and never
/// asks the clock itself, so a given world and a given frame still draw
/// identically anywhere.
fn anim_frame() -> u32 {
    #[allow(clippy::cast_sign_loss)] // `get_time` counts up from process start.
    {
        (get_time() / ANIMATION_RATE) as u32
    }
}

/// Where the telegraph pulse is in its cycle, `0..1`.
///
/// Kept in `f64` until the last moment: this counts up from process start, and
/// taking the fraction first is what stops a long session from turning the
/// pulse into a stutter.
fn telegraph_phase() -> f32 {
    #[allow(clippy::cast_possible_truncation)] // A fraction of one, by construction.
    {
        (get_time() / anim::PULSE_SECS).fract() as f32
    }
}

/// What a scrolling viewport should keep in the middle: the examine cursor
/// while it is out, otherwise the nucleus you are steering.
fn focus(view: &RenderView) -> Coord {
    view.examine.as_ref().map_or_else(
        || {
            view.status.active_nucleus.as_ref().map_or_else(
                || Coord::new(view.width / 2, view.height / 2),
                |(_, at)| *at,
            )
        },
        |examine| examine.pos,
    )
}
