//! macroquad frontend: a window, a font atlas, and one command a turn.
//!
//! Everything that decides what happens lives in [`amoeba_rl::sim`], which
//! resolves the world all the way down to a glyph and two colours per cell
//! before this file sees it. What is left is genuinely only presentation:
//! where the panels go ([`render`]), what goes in them ([`hud`]), which pixels
//! a character is ([`tileset`]), and which key or thumb meant which
//! [`Command`] ([`input`]).
//!
//! The loop is deliberately plain. The game is turn-based, so there is no
//! timestep and nothing to interpolate: read the world, draw it, hand the sim
//! at most one command, repeat. The only clock anywhere is the animation
//! counter, which ticks four times a second outside the sim and is passed in,
//! keeping [`Sim::view`] the pure function of world and frame that it claims
//! to be.

mod hud;
mod input;
mod render;
mod tileset;

use macroquad::time::{get_frame_time, get_time};
use macroquad::window::{Conf, next_frame, screen_height, screen_width};

use amoeba_rl::sim::grid::Coord;
use amoeba_rl::sim::{Difficulty, Phase, RenderView, Sim};

use input::{Controls, Input};
use render::Layout;
use tileset::Tileset;

/// Seconds per animation step, as the original's `ANIMATION_RATE` was.
const ANIMATION_RATE: f64 = 0.25;

fn window_conf() -> Conf {
    Conf {
        window_title: "Amoeba RL".to_owned(),
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

    loop {
        let view = sim.view(anim_frame());
        let layout = Layout::fit(screen_width(), screen_height(), view.width, view.height);
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
        // their own.
        let controls = Controls::fit(
            screen_width(),
            screen_height(),
            input.wants_controls(layout.mode) && view.phase == Phase::Playing,
        );

        let frame = input.gather(&view, &layout, camera, &controls, get_frame_time());
        render::draw(&font, &view, &layout, camera, &controls, input.page());

        sim.advance(frame.command);
        // AUDIO STAGE: cues live for exactly this one `advance`, so the audio
        // half reads them here, alongside the mute toggle from the same frame.
        let _cues = sim.cues();
        let _ = frame.mute;

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
