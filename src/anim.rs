//! Effects in, pictures out. The third half of the frontend.
//!
//! [`audio`](crate::audio) turns the sim's [`Cue`](amoeba_rl::sim::Cue)s into
//! noise; this turns its [`Effect`]s into short overlays drawn on top of the
//! map, so that a mechanic you could only have worked out by reading the
//! message log can be *watched* instead. A militia hits your nucleus, the
//! nucleus dives into the cytoplasm behind it, and the cytoplasm comes apart:
//! four things in one turn, drawn in that order, in the cells they happened in.
//!
//! ## What is being drawn over
//!
//! The sim resolves a whole turn before the frontend sees any of it, so the
//! world underneath these overlays is already in its final state — the nucleus
//! is *already* where it retreated to. That is why nothing here animates a
//! position: it animates the *event*, as a swing, a flash, a burst or a spark
//! crossing a gap. What the player is watching is a diagram of the turn drawn
//! over its outcome, not an interpolation of the world.
//!
//! ## Keeping turns short
//!
//! Three rules, in the order they apply, and all of them exist because a turn
//! late in a run can contain hundreds of interactions:
//!
//! 1. **Simultaneous things are drawn simultaneously.** A hundred-cell amoeba
//!    flowing one cell reports a hundred steps, and they all happened at once,
//!    so they share one slot. Consecutive effects of the same kind are one
//!    *wave*; a change of kind is a new moment.
//! 2. **Only [`MAX_WAVES`] moments are shown**, and when there are more the
//!    quiet ones go first — the same ranking [`audio`](crate::audio) uses to
//!    decide which sounds survive a busy turn. Losing a nucleus outranks a
//!    footstep.
//! 3. **The whole turn is squeezed into [`MAX_TURN`]**, uniformly, so a busy
//!    turn plays faster rather than longer. [`Speed`] scales that ceiling as
//!    well as the durations, so *fast* really is a shorter turn and not merely
//!    a brisker one.
//!
//! Playback never costs the player latency: any command asked for while an
//! animation is running cuts it short (see the main loop), so the show only
//! happens while nobody is in a hurry.
//!
//! ## The other tense
//!
//! Everything above is the past tense: a replay of a turn that has already
//! resolved. [`telegraphs`] is the future tense, and the only thing in the
//! frontend that is — every human in sight has committed to one action, and
//! this draws that commitment over the cell it will land on until it does. It
//! pulses rather than plays, because it is a standing statement and not an
//! event, and the two never share the screen: while a turn is showing itself
//! the telegraphs are held back, or the player would be looking at two turns at
//! once.

use macroquad::math::{Rect, Vec2};
use macroquad::shapes::draw_rectangle;

use amoeba_rl::sim::RenderView;
use amoeba_rl::sim::actors::{IntentKind, Rgb, palette};
use amoeba_rl::sim::effect::{Effect, EffectKind};
use amoeba_rl::sim::grid::Coord;
use amoeba_rl::sim::view::Telegraph;

use crate::render::rgba;
use crate::settings::{Settings, Speed};
use crate::tileset::Tileset;

/// Longest a turn may spend showing itself, in seconds, at [`Speed::Normal`].
///
/// Everything is compressed to fit inside this, so it is a promise about the
/// worst case and not a target: an ordinary step is over in a fifth of it.
pub const MAX_TURN: f32 = 0.70;

/// Distinct moments a turn will draw. Beyond this the lowest-ranked go.
pub const MAX_WAVES: usize = 12;

/// Seconds between the starts of two consecutive moments.
const STAGGER: f32 = 0.055;

/// The run just changed. Never dropped, and given room to land.
const HEADLINE: u8 = 3;

/// Something happened that changes what you should do next.
const MAJOR: u8 = 2;

/// Something happened.
const MINOR: u8 = 1;

/// Something is still happening. The first thing dropped when a turn is busy.
const TEXTURE: u8 = 0;

/// Every glyph the overlays draw, so the atlas can be checked for all of them.
const BULLET: char = '\u{2022}';
/// Light shade: the slime of a step.
const LIGHT_SHADE: char = '\u{2591}';
/// Dark shade: a gate winding up.
const DARK_SHADE: char = '\u{2593}';
/// Full block: a wall of protein.
const BLOCK: char = '\u{2588}';

/// How an overlay moves while it is on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Motion {
    /// Crosses the gap, then flashes where it landed. Blows, steps and sparks.
    Travel,
    /// Swells and fades where it happened. Deaths, bursts, transformations.
    Burst,
    /// Hangs over the cell and drifts up. Marks a state rather than an event.
    Mark,
    /// Lunges partway and comes back: an action that was refused.
    Nudge,
    /// Two glyphs crossing in opposite directions.
    Trade,
    /// Lights the line from one end to the other.
    Sweep,
}

/// What one glyph is, when the direction of the thing decides it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Face {
    /// This glyph, always.
    Glyph(char),
    /// An arrow pointing the way the effect went.
    Arrow,
}

/// What one kind of effect looks like.
struct Style {
    /// The glyph, or a rule for picking one.
    face: Face,
    /// The glyph's colour.
    color: Rgb,
    /// How it moves.
    motion: Motion,
    /// A wash over the cell it happened *to*, if it deserves one.
    tint: Option<Rgb>,
    /// How long it is on screen, at [`Speed::Normal`].
    secs: f32,
    /// Who survives a turn with more moments in it than [`MAX_WAVES`].
    rank: u8,
}

impl Style {
    /// The usual case: a fixed glyph.
    const fn glyph(
        glyph: char,
        color: Rgb,
        motion: Motion,
        tint: Option<Rgb>,
        secs: f32,
        rank: u8,
    ) -> Self {
        Self {
            face: Face::Glyph(glyph),
            color,
            motion,
            tint,
            secs,
            rank,
        }
    }
}

/// The look of the whole game, in one table.
///
/// Exhaustive on purpose, like the sound design's `slot`: a new
/// [`EffectKind`] in the sim does not compile until somebody here has decided
/// what it looks like. That is the mechanism that keeps the promise in the
/// sim's [`effect`](amoeba_rl::sim::effect) module — that nothing reaches the
/// message log without something to look at beside it.
#[allow(clippy::too_many_lines)] // One arm per kind of event, which is the point.
const fn style(kind: EffectKind) -> Style {
    match kind {
        // The mass flowing. Quiet, brief, and the first thing sacrificed on a
        // busy turn: a step is the one event the player is already causing.
        EffectKind::Flow => Style::glyph(
            LIGHT_SHADE,
            palette::PATH_SLIME,
            Motion::Travel,
            Some(palette::BODY_SLIME),
            0.14,
            TEXTURE,
        ),
        EffectKind::Swap => Style::glyph(BULLET, palette::SLIME, Motion::Trade, None, 0.16, MINOR),
        EffectKind::Refused => Style {
            face: Face::Arrow,
            color: palette::RETICLE_FOREGROUND,
            motion: Motion::Nudge,
            tint: Some(palette::RETICLE_BACKGROUND),
            secs: 0.18,
            rank: MINOR,
        },
        // A human's blow: the swing crosses the gap and the impact flashes.
        EffectKind::Strike => Style {
            face: Face::Arrow,
            color: palette::MILITIA,
            motion: Motion::Travel,
            tint: Some(palette::RETICLE_FOREGROUND),
            secs: 0.22,
            rank: MAJOR,
        },
        // Long enough to be read as a consequence of the blow before it.
        EffectKind::Retreat => Style {
            face: Face::Arrow,
            color: palette::ROOT_ORGANELLE,
            motion: Motion::Travel,
            tint: Some(palette::PATH_SLIME),
            secs: 0.26,
            rank: MAJOR,
        },
        EffectKind::Shielded => Style::glyph(
            BLOCK,
            palette::CALCIUM,
            Motion::Travel,
            Some(palette::CALCIUM),
            0.24,
            MAJOR,
        ),
        EffectKind::Consume => Style::glyph(
            'O',
            palette::SLIME,
            Motion::Burst,
            Some(palette::BODY_SLIME),
            0.20,
            MAJOR,
        ),
        EffectKind::Engulf => Style::glyph(
            'O',
            palette::OVERFILL,
            Motion::Burst,
            Some(palette::BODY_SLIME),
            0.26,
            MAJOR,
        ),
        // The spark that says which cytoplasm a catalyst was spent on.
        EffectKind::Absorb => {
            Style::glyph('%', palette::ELECTRONICS, Motion::Travel, None, 0.20, MINOR)
        }
        EffectKind::Transform => Style::glyph(
            '*',
            palette::SUPER_BRIGHT,
            Motion::Burst,
            Some(palette::PATH_SLIME),
            0.24,
            MAJOR,
        ),
        EffectKind::Produce => Style::glyph('+', palette::SLIME, Motion::Travel, None, 0.20, MINOR),
        EffectKind::Drop => Style::glyph(
            '%',
            palette::ORGANELLE_INACTIVE,
            Motion::Travel,
            None,
            0.16,
            TEXTURE,
        ),
        EffectKind::Destroyed => Style::glyph(
            'x',
            palette::ROOT_ORGANELLE,
            Motion::Burst,
            Some(palette::RETICLE_BACKGROUND),
            0.22,
            MAJOR,
        ),
        EffectKind::NucleusLost => Style::glyph(
            'X',
            palette::ROOT_ORGANELLE,
            Motion::Burst,
            Some(palette::RETICLE_FOREGROUND),
            0.34,
            HEADLINE,
        ),
        EffectKind::Slain => Style::glyph(
            'x',
            palette::MILITIA,
            Motion::Burst,
            Some(palette::RETICLE_BACKGROUND),
            0.20,
            MINOR,
        ),
        // The line a hunter has just painted, lit end to end once so it is
        // clear the reticles that follow are one shot and not many.
        EffectKind::Aim => Style::glyph(
            LIGHT_SHADE,
            palette::RETICLE_FOREGROUND,
            Motion::Sweep,
            None,
            0.24,
            MINOR,
        ),
        EffectKind::Shot => Style {
            face: Face::Arrow,
            color: palette::ELECTRONICS,
            motion: Motion::Sweep,
            tint: Some(palette::RETICLE_FOREGROUND),
            secs: 0.20,
            rank: MAJOR,
        },
        EffectKind::Terrify => Style::glyph(
            '!',
            palette::CURSOR,
            Motion::Mark,
            Some(palette::DARK_CURSOR),
            0.24,
            MINOR,
        ),
        // A caravan getting away is somebody else's good news, and it is drawn
        // as an exit rather than as a loss: the wagon crosses into the gate.
        EffectKind::Depart => Style {
            face: Face::Arrow,
            color: palette::MILITIA,
            motion: Motion::Travel,
            tint: Some(palette::CITY),
            secs: 0.26,
            rank: MAJOR,
        },
        // A stretch of turns nobody did anything with, drifting off the mass
        // that spent them. The one overlay that stands for a span of time
        // rather than for a moment, which is why it is the slowest thing here.
        EffectKind::Rest => Style::glyph(
            LIGHT_SHADE,
            palette::OVERFILL,
            Motion::Mark,
            None,
            0.30,
            MINOR,
        ),
        EffectKind::Wave => Style::glyph(
            DARK_SHADE,
            palette::ELECTRONICS,
            Motion::Burst,
            Some(palette::RETICLE_BACKGROUND),
            0.28,
            MAJOR,
        ),
        EffectKind::GateFell => Style::glyph(
            BLOCK,
            palette::CITY,
            Motion::Burst,
            Some(palette::WALL_BACKGROUND_FOV),
            0.34,
            HEADLINE,
        ),
    }
}

/// One effect with the window it is on screen for.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Shot {
    effect: Effect,
    start: f32,
    end: f32,
}

/// The overlays currently on screen, and the clock they run on.
#[derive(Clone, Debug, Default)]
pub struct Anim {
    shots: Vec<Shot>,
    /// Seconds since this turn's playback began.
    clock: f32,
    /// When the last shot finishes.
    total: f32,
}

impl Anim {
    /// Nothing playing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            shots: Vec::new(),
            clock: 0.0,
            total: 0.0,
        }
    }

    /// Whether anything is still on screen, and so whether the loop should be
    /// holding the player's next command back.
    #[must_use]
    pub fn playing(&self) -> bool {
        self.clock < self.total
    }

    /// Take a turn's worth of news.
    ///
    /// An empty list is *not* an interruption. The main loop calls this after
    /// every `advance`, and most frames resolve nothing at all — a frame that
    /// reported nothing must leave what is still on screen alone.
    pub fn load(&mut self, effects: &[Effect], settings: &Settings) {
        if effects.is_empty() {
            return;
        }
        self.shots.clear();
        self.clock = 0.0;
        self.total = 0.0;
        if !settings.animations {
            return;
        }
        self.shots = plan(effects, settings.speed);
        self.total = self.shots.iter().fold(0.0, |acc, s| acc.max(s.end));
    }

    /// Run the clock. `dt` is the frame, in seconds.
    pub fn tick(&mut self, dt: f32) {
        if self.playing() {
            // A backgrounded tab hands back whole seconds; believing it would
            // skip the animation rather than play it, which is fine, but the
            // clock should stay a number either way.
            self.clock += dt.clamp(0.0, MAX_TURN);
        }
    }

    /// Cut to the end. The player has asked for something and is not watching.
    pub const fn skip(&mut self) {
        self.clock = self.total;
    }

    /// Draw whatever is on screen this instant, over the map.
    pub fn draw(&self, font: &Tileset, screen: &Viewport) {
        if !self.playing() {
            return;
        }
        for shot in &self.shots {
            if self.clock < shot.start || self.clock >= shot.end {
                continue;
            }
            let span = (shot.end - shot.start).max(f32::EPSILON);
            screen.shot(font, shot.effect, (self.clock - shot.start) / span);
        }
    }
}

/// Lay a turn's effects out in time.
///
/// Anything that happened out in the dark is dropped before any of it: a burst
/// painted on unexplored black is not an explanation, it is a map of where the
/// gates are. The three rules from this module's docs then apply in order:
/// group, drop, compress.
fn plan(all: &[Effect], speed: Speed) -> Vec<Shot> {
    let effects: Vec<Effect> = all.iter().copied().filter(|e| e.in_sight).collect();
    let effects = effects.as_slice();
    let kept = keep(effects, &waves(effects));
    let mut shots = Vec::with_capacity(effects.len());
    let mut total = 0.0_f32;
    for (slot, range) in kept.iter().enumerate() {
        let start = STAGGER * slot as f32;
        let secs = effects[range.clone()]
            .iter()
            .fold(0.0_f32, |acc, e| acc.max(style(e.kind).secs));
        let end = start + secs;
        total = total.max(end);
        for &effect in &effects[range.clone()] {
            shots.push(Shot { effect, start, end });
        }
    }
    // Compress the whole turn rather than truncating it: a busy turn plays
    // faster, and every moment kept is still shown.
    let squeeze = if total > MAX_TURN {
        MAX_TURN / total
    } else {
        1.0
    } * speed.scale();
    for shot in &mut shots {
        shot.start *= squeeze;
        shot.end *= squeeze;
    }
    shots
}

/// Split a turn into moments: runs of consecutive effects of the same kind.
fn waves(effects: &[Effect]) -> Vec<std::ops::Range<usize>> {
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    for (i, effect) in effects.iter().enumerate() {
        match out.last_mut() {
            Some(last) if effects[last.start].kind == effect.kind => last.end = i + 1,
            _ => out.push(i..i + 1),
        }
    }
    out
}

/// The moments that survive, in the order they happened.
///
/// Highest rank first when there are too many, ties broken by when they
/// happened, so a turn that overflows loses its footsteps and keeps its news.
/// Every effect in a wave is the same kind, so the first one names its rank.
fn keep(effects: &[Effect], waves: &[std::ops::Range<usize>]) -> Vec<std::ops::Range<usize>> {
    if waves.len() <= MAX_WAVES {
        return waves.to_vec();
    }
    let rank = |wave: &std::ops::Range<usize>| style(effects[wave.start].kind).rank;
    let mut order: Vec<usize> = (0..waves.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(rank(&waves[i])), i));
    order.truncate(MAX_WAVES);
    order.sort_unstable();
    order.into_iter().map(|i| waves[i].clone()).collect()
}

// -- telegraphs ---------------------------------------------------------------

/// Seconds a telegraph takes to pulse once.
///
/// Slow enough to read as breathing rather than as flashing. It is a standing
/// state and not an event, and the whole point of it is to be looked at for as
/// long as the player wants to think.
pub const PULSE_SECS: f64 = 1.2;

/// How opaque a telegraph's wash gets under the glyph.
const TELEGRAPH_WASH: f32 = 0.45;

/// Exclamation mark: a shot going off this turn.
const BANG: char = '!';
/// Plus: a gun about to line up on a cell.
const CROSS: char = '+';

/// What one committed move looks like.
struct Look {
    /// The glyph, or a rule for picking one.
    face: Face,
    /// The glyph's colour.
    color: Rgb,
    /// A wash over the cell it will happen to.
    tint: Option<Rgb>,
    /// How loudly it is drawn, at the top of the pulse.
    strength: f32,
}

/// The look of every commitment a human can make.
///
/// Exhaustive like [`style`], and for the same reason: a new
/// [`IntentKind`](amoeba_rl::sim::actors::IntentKind) in the sim does not
/// compile until somebody here has decided how the player is to be shown it.
///
/// Loudness is a ranking of consequence, so the eye sorts the crowd on its own:
/// a blow about to land on your mass is the brightest thing on the map, a
/// footstep is barely there, and standing still is a dot.
const fn telegraph_look(kind: IntentKind) -> Look {
    match kind {
        IntentKind::Step => Look {
            face: Face::Arrow,
            color: palette::MILITIA,
            tint: None,
            strength: 0.45,
        },
        IntentKind::Strike => Look {
            face: Face::Arrow,
            color: palette::RETICLE_FOREGROUND,
            tint: Some(palette::RETICLE_BACKGROUND),
            strength: 0.95,
        },
        IntentKind::Aim => Look {
            face: Face::Glyph(CROSS),
            color: palette::ELECTRONICS,
            tint: Some(palette::RETICLE_BACKGROUND),
            strength: 0.85,
        },
        // The reticles are already blinking down the whole line, so this only
        // has to say which gun is about to let go of it.
        IntentKind::Fire => Look {
            face: Face::Glyph(BANG),
            color: palette::RETICLE_FOREGROUND,
            tint: None,
            strength: 0.9,
        },
        IntentKind::Hold => Look {
            face: Face::Glyph(BULLET),
            color: palette::RESTING_MILITIA,
            tint: None,
            strength: 0.35,
        },
        IntentKind::Depart => Look {
            face: Face::Arrow,
            color: palette::CITY,
            tint: Some(palette::CITY),
            strength: 0.8,
        },
    }
}

/// Which cell a telegraph is drawn on: where the thing will happen, except for
/// a shot, which happens down a line the reticles are already drawing, and for
/// standing still, which happens where the human is.
const fn telegraph_cell(mark: &Telegraph) -> Coord {
    match mark.kind {
        IntentKind::Fire | IntentKind::Hold => mark.from,
        _ => mark.to,
    }
}

/// Draw what every human in sight has committed to doing next.
///
/// `phase` runs `0..1` once per [`PULSE_SECS`]. Unlike everything else in this
/// module these are not a replay of a turn that has happened; they are a
/// standing statement about the turn that has not, and they sit on screen for
/// exactly as long as the player takes to answer them.
pub fn telegraphs(font: &Tileset, view: &RenderView, screen: &Viewport, phase: f32) {
    let pulse = 0.35f32.mul_add((phase * std::f32::consts::TAU).sin(), 0.65);
    for mark in &view.telegraphs {
        let look = telegraph_look(mark.kind);
        let Some(at) = screen.at(telegraph_cell(mark)) else {
            continue;
        };
        let face = match look.face {
            Face::Glyph(glyph) => glyph,
            Face::Arrow => arrow(mark.from, mark.to),
        };
        let alpha = look.strength * pulse;
        if let Some(tint) = look.tint {
            draw_rectangle(
                at.x,
                at.y,
                screen.cell,
                screen.cell,
                rgba(tint, alpha * TELEGRAPH_WASH),
            );
        }
        font.draw(face, at.x, at.y, screen.cell, rgba(look.color, alpha));
    }
}

/// The rectangle of window an overlay is drawn into, and the part of the map
/// it is showing.
///
/// Anything outside it is skipped rather than clipped, so an overlay can never
/// spill onto a panel.
pub struct Viewport {
    /// The window rectangle the map occupies.
    pub map: Rect,
    /// The side of one cell, in pixels.
    pub cell: f32,
    /// The map cell drawn in the viewport's top-left corner.
    pub camera: Coord,
    /// Columns of map on screen.
    pub cols: i32,
    /// Rows of map on screen.
    pub rows: i32,
}

impl Viewport {
    /// The window position of a map cell's top-left corner, or `None` when the
    /// cell is not on screen.
    fn at(&self, c: Coord) -> Option<Vec2> {
        let (col, row) = (c.x - self.camera.x, c.y - self.camera.y);
        (col >= 0 && row >= 0 && col < self.cols && row < self.rows).then(|| {
            Vec2::new(
                self.cell.mul_add(col as f32, self.map.x),
                self.cell.mul_add(row as f32, self.map.y),
            )
        })
    }

    /// One overlay, `p` of the way through its life.
    fn shot(&self, font: &Tileset, effect: Effect, p: f32) {
        let style = style(effect.kind);
        let face = match style.face {
            Face::Glyph(glyph) => glyph,
            Face::Arrow => arrow(effect.from, effect.to),
        };
        match style.motion {
            Motion::Travel => self.travel(font, &effect, &style, face, p),
            Motion::Burst => self.burst(font, effect.to, &style, face, p),
            Motion::Mark => self.mark(font, effect.to, &style, face, p),
            Motion::Nudge => self.nudge(font, &effect, &style, face, p),
            Motion::Trade => self.trade(font, &effect, &style, face, p),
            Motion::Sweep => self.sweep(font, &effect, &style, face, p),
        }
    }

    /// Cross the gap over the first [`ARRIVAL`] of the window, then flash.
    fn travel(&self, font: &Tileset, effect: &Effect, style: &Style, face: char, p: f32) {
        /// Where in the window the thing lands.
        const ARRIVAL: f32 = 0.6;
        let (Some(from), Some(to)) = (self.at(effect.from), self.at(effect.to)) else {
            return;
        };
        if p < ARRIVAL {
            // An effect that happened in one cell has nothing to cross, and
            // `lerp` between a point and itself says so without a special case.
            let at = from.lerp(to, smooth(p / ARRIVAL));
            font.draw(
                face,
                at.x,
                at.y,
                self.cell,
                rgba(style.color, p.mul_add(-0.3, 1.0)),
            );
        }
        if p >= ARRIVAL {
            let landed = (p - ARRIVAL) / (1.0 - ARRIVAL);
            self.flash(to, style, 1.0 - landed);
            font.draw(face, to.x, to.y, self.cell, rgba(style.color, 1.0 - landed));
        }
    }

    /// Swell and fade in place.
    fn burst(&self, font: &Tileset, at: Coord, style: &Style, face: char, p: f32) {
        let Some(at) = self.at(at) else { return };
        self.flash(at, style, 1.0 - p);
        let size = self.cell * 0.75f32.mul_add(p, 0.55);
        let inset = (self.cell - size) * 0.5;
        font.draw(
            face,
            at.x + inset,
            at.y + inset,
            size,
            rgba(style.color, p.mul_add(-p, 1.0)),
        );
    }

    /// Hang over the cell and drift up.
    fn mark(&self, font: &Tileset, at: Coord, style: &Style, face: char, p: f32) {
        let Some(at) = self.at(at) else { return };
        self.flash(at, style, (1.0 - p) * 0.7);
        font.draw(
            face,
            at.x,
            self.cell.mul_add(-0.35 * p, at.y),
            self.cell,
            rgba(style.color, p.mul_add(-p, 1.0)),
        );
    }

    /// Lunge partway and come back.
    fn nudge(&self, font: &Tileset, effect: &Effect, style: &Style, face: char, p: f32) {
        /// How far into the cell it refused it gets.
        const REACH: f32 = 0.45;
        let (Some(from), Some(to)) = (self.at(effect.from), self.at(effect.to)) else {
            return;
        };
        self.flash(to, style, 1.0 - p);
        let out = (p * std::f32::consts::PI).sin() * REACH;
        let at = from.lerp(to, out);
        font.draw(
            face,
            at.x,
            at.y,
            self.cell,
            rgba(style.color, p.mul_add(-0.5, 1.0)),
        );
    }

    /// Two glyphs crossing.
    fn trade(&self, font: &Tileset, effect: &Effect, style: &Style, face: char, p: f32) {
        let (Some(from), Some(to)) = (self.at(effect.from), self.at(effect.to)) else {
            return;
        };
        let travel = smooth(p);
        let alpha = rgba(style.color, p.mul_add(-0.4, 1.0));
        let there = from.lerp(to, travel);
        let back = to.lerp(from, travel);
        font.draw(face, there.x, there.y, self.cell, alpha);
        font.draw(face, back.x, back.y, self.cell, alpha);
    }

    /// Light the line from one end to the other.
    fn sweep(&self, font: &Tileset, effect: &Effect, style: &Style, face: char, p: f32) {
        let cells = line(effect.from, effect.to);
        let reached = smooth(p) * cells.len() as f32;
        for (i, cell) in cells.iter().enumerate() {
            let Some(at) = self.at(*cell) else { continue };
            let lead = reached - i as f32;
            if lead < 0.0 {
                continue;
            }
            // Brightest at the leading edge, trailing off behind it.
            let alpha = lead.mul_add(-0.25, 1.0).clamp(0.0, 1.0) * p.mul_add(-p, 1.0);
            self.flash(at, style, alpha);
            font.draw(face, at.x, at.y, self.cell, rgba(style.color, alpha));
        }
    }

    /// The wash over a cell something happened to.
    fn flash(&self, at: Vec2, style: &Style, strength: f32) {
        /// How opaque a flash gets at its brightest.
        const PEAK: f32 = 0.55;
        if let Some(tint) = style.tint {
            draw_rectangle(
                at.x,
                at.y,
                self.cell,
                self.cell,
                rgba(tint, strength.clamp(0.0, 1.0) * PEAK),
            );
        }
    }
}

/// Which arrow points from `from` toward `to`, longest axis first. A pair of
/// cells that are the same reads as a downward nudge, which is what a refused
/// action in place looks like.
const fn arrow(from: Coord, to: Coord) -> char {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dx.abs() > dy.abs() {
        if dx > 0 { '\u{25BA}' } else { '\u{25C4}' }
    } else if dy < 0 {
        '\u{25B2}'
    } else {
        '\u{25BC}'
    }
}

/// The cells from `from` to `to` inclusive, along whichever axis they share.
///
/// Reticle lines are always straight, so this is a walk and not a Bresenham;
/// anything diagonal degenerates to its two endpoints, which is the honest
/// thing to draw for a relationship that is not a line.
fn line(from: Coord, to: Coord) -> Vec<Coord> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dx != 0 && dy != 0 {
        return vec![from, to];
    }
    let steps = dx.abs().max(dy.abs());
    let step = Coord::new(dx.signum(), dy.signum());
    (0..=steps)
        .map(|i| Coord::new(from.x + step.x * i, from.y + step.y * i))
        .collect()
}

/// Smoothstep, so nothing starts or stops abruptly.
fn smooth(p: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    p * p * 2.0f32.mul_add(-p, 3.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tileset::cp437;

    /// Every kind, for the table sweeps.
    const KINDS: [EffectKind; 22] = [
        EffectKind::Flow,
        EffectKind::Swap,
        EffectKind::Refused,
        EffectKind::Strike,
        EffectKind::Retreat,
        EffectKind::Shielded,
        EffectKind::Consume,
        EffectKind::Engulf,
        EffectKind::Absorb,
        EffectKind::Transform,
        EffectKind::Produce,
        EffectKind::Drop,
        EffectKind::Destroyed,
        EffectKind::NucleusLost,
        EffectKind::Slain,
        EffectKind::Aim,
        EffectKind::Shot,
        EffectKind::Terrify,
        EffectKind::Depart,
        EffectKind::Rest,
        EffectKind::Wave,
        EffectKind::GateFell,
    ];

    fn effect(kind: EffectKind) -> Effect {
        Effect::new(kind, Coord::new(4, 4), Coord::new(5, 4))
    }

    fn settings() -> Settings {
        Settings::new()
    }

    #[test]
    fn every_kind_is_drawable() {
        for kind in KINDS {
            let style = style(kind);
            assert!(style.secs > 0.0, "{kind:?} is on screen for no time");
            assert!(style.secs <= MAX_TURN, "{kind:?} outlasts a whole turn");
            assert!(style.rank <= HEADLINE, "{kind:?} outranks the run ending");
            if let Face::Glyph(glyph) = style.face {
                assert_ne!(
                    cp437(glyph),
                    b'?',
                    "{kind:?} draws a glyph the atlas has not got"
                );
            }
        }
    }

    /// Every commitment a human can make, for the telegraph table sweep.
    const INTENTS: [IntentKind; 6] = [
        IntentKind::Step,
        IntentKind::Strike,
        IntentKind::Aim,
        IntentKind::Fire,
        IntentKind::Hold,
        IntentKind::Depart,
    ];

    #[test]
    fn every_commitment_is_drawable() {
        for kind in INTENTS {
            let look = telegraph_look(kind);
            assert!(look.strength > 0.0, "{kind:?} is drawn invisibly");
            assert!(look.strength <= 1.0, "{kind:?} is drawn over the top");
            if let Face::Glyph(glyph) = look.face {
                assert_ne!(
                    cp437(glyph),
                    b'?',
                    "{kind:?} draws a glyph the atlas has not got"
                );
            }
        }
    }

    #[test]
    fn a_blow_is_drawn_louder_than_a_footstep() {
        // The eye is meant to sort a crowd of humans on its own, so the ranking
        // has to hold: what is about to hurt you outshines what is about to
        // walk past you, and standing still is the quietest thing on the map.
        let loudness = |kind| telegraph_look(kind).strength;
        assert!(loudness(IntentKind::Strike) > loudness(IntentKind::Step));
        assert!(loudness(IntentKind::Aim) > loudness(IntentKind::Step));
        assert!(loudness(IntentKind::Step) > loudness(IntentKind::Hold));
    }

    #[test]
    fn a_telegraph_is_drawn_where_the_thing_will_happen() {
        let mark = |kind| Telegraph {
            from: Coord::new(4, 4),
            to: Coord::new(5, 4),
            kind,
        };
        assert_eq!(telegraph_cell(&mark(IntentKind::Step)), Coord::new(5, 4));
        assert_eq!(telegraph_cell(&mark(IntentKind::Strike)), Coord::new(5, 4));
        // Except a shot, which is already drawn down its whole line by the
        // reticles, and a pause, which happens where the human is standing.
        assert_eq!(telegraph_cell(&mark(IntentKind::Fire)), Coord::new(4, 4));
        assert_eq!(telegraph_cell(&mark(IntentKind::Hold)), Coord::new(4, 4));
    }

    #[test]
    fn the_pulse_never_leaves_the_screen() {
        // `phase` comes off a wall clock, so it has to survive whatever the
        // clock hands over.
        for step in 0..24 {
            let phase = step as f32 / 12.0;
            let pulse = 0.35f32.mul_add((phase * std::f32::consts::TAU).sin(), 0.65);
            assert!((0.0..=1.0).contains(&pulse), "{pulse}");
        }
    }

    #[test]
    fn the_arrows_it_picks_are_all_in_the_atlas() {
        let here = Coord::new(5, 5);
        for other in [
            Coord::new(9, 5),
            Coord::new(1, 5),
            Coord::new(5, 1),
            Coord::new(5, 9),
            here,
        ] {
            assert_ne!(cp437(arrow(here, other)), b'?');
        }
        assert_eq!(arrow(here, Coord::new(9, 5)), '\u{25BA}');
        assert_eq!(arrow(here, Coord::new(1, 5)), '\u{25C4}');
        assert_eq!(arrow(here, Coord::new(5, 1)), '\u{25B2}');
        assert_eq!(arrow(here, Coord::new(5, 9)), '\u{25BC}');
    }

    #[test]
    fn simultaneous_things_share_a_moment() {
        // The signature case: a hundred-cell amoeba flowing one cell reports a
        // hundred steps that all happened at once. One slot, not a hundred.
        let flow: Vec<Effect> = (0..100).map(|_| effect(EffectKind::Flow)).collect();
        let shots = plan(&flow, Speed::Normal);
        assert_eq!(shots.len(), 100, "every step is still drawn");
        let starts: Vec<f32> = shots.iter().map(|s| s.start).collect();
        assert!(
            starts.iter().all(|s| (s - starts[0]).abs() < f32::EPSILON),
            "they were staggered"
        );
        let total = shots.iter().fold(0.0_f32, |acc, s| acc.max(s.end));
        assert!(total <= style(EffectKind::Flow).secs + f32::EPSILON);
    }

    #[test]
    fn a_change_of_kind_is_a_new_moment() {
        let script = [
            effect(EffectKind::Strike),
            effect(EffectKind::Retreat),
            effect(EffectKind::Strike),
            effect(EffectKind::Destroyed),
        ];
        let shots = plan(&script, Speed::Normal);
        assert_eq!(shots.len(), 4);
        for pair in shots.windows(2) {
            assert!(
                pair[0].start < pair[1].start,
                "the turn has to read in order"
            );
        }
    }

    #[test]
    fn a_busy_turn_is_squeezed_rather_than_lengthened() {
        // Alternating kinds, so every one of them is its own moment.
        let script: Vec<Effect> = (0..60)
            .map(|i| {
                effect(if i % 2 == 0 {
                    EffectKind::Strike
                } else {
                    EffectKind::Destroyed
                })
            })
            .collect();
        let shots = plan(&script, Speed::Normal);
        let total = shots.iter().fold(0.0_f32, |acc, s| acc.max(s.end));
        assert!(total <= MAX_TURN + f32::EPSILON, "a turn ran {total}s");
        assert!(total > 0.0);
    }

    #[test]
    fn a_turn_with_too_much_in_it_keeps_its_news() {
        // Twenty moments of nothing much, with one nucleus dying in the middle.
        let mut script: Vec<Effect> = Vec::new();
        for i in 0..40 {
            script.push(effect(if i % 2 == 0 {
                EffectKind::Flow
            } else {
                EffectKind::Drop
            }));
            if i == 21 {
                script.push(effect(EffectKind::NucleusLost));
            }
        }
        let shots = plan(&script, Speed::Normal);
        assert!(
            shots
                .iter()
                .any(|s| s.effect.kind == EffectKind::NucleusLost),
            "the run-changing event was dropped for footsteps"
        );
        let moments = shots
            .iter()
            .map(|s| s.start.to_bits())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(moments.len() <= MAX_WAVES, "{} moments", moments.len());
    }

    #[test]
    fn speed_scales_the_ceiling_as_well_as_the_pace() {
        let script: Vec<Effect> = (0..60)
            .map(|i| {
                effect(if i % 2 == 0 {
                    EffectKind::Strike
                } else {
                    EffectKind::Destroyed
                })
            })
            .collect();
        let total = |speed| {
            plan(&script, speed)
                .iter()
                .fold(0.0_f32, |acc, s| acc.max(s.end))
        };
        assert!(total(Speed::Fast) < total(Speed::Normal));
        assert!(total(Speed::Normal) < total(Speed::Slow));
        assert!(
            total(Speed::Fast) <= MAX_TURN.mul_add(Speed::Fast.scale(), f32::EPSILON),
            "fast has to mean a shorter turn, not just a brisker one"
        );
    }

    #[test]
    fn a_turn_plays_once_and_then_stops() {
        let mut anim = Anim::new();
        assert!(!anim.playing(), "nothing to show yet");
        anim.load(&[effect(EffectKind::Strike)], &settings());
        assert!(anim.playing());
        for _ in 0..600 {
            anim.tick(1.0 / 60.0);
        }
        assert!(!anim.playing(), "it never finished");
    }

    #[test]
    fn a_quiet_frame_does_not_interrupt_what_is_on_screen() {
        // The main loop calls `load` after every advance, and most frames
        // resolve nothing at all.
        let mut anim = Anim::new();
        anim.load(&[effect(EffectKind::Engulf)], &settings());
        anim.tick(0.01);
        anim.load(&[], &settings());
        assert!(anim.playing(), "an empty frame wiped the animation");
    }

    #[test]
    fn nothing_out_in_the_dark_is_drawn() {
        // Twelve gates queueing waves across an unexplored map would otherwise
        // paint twelve bursts on black and give away where every one of them
        // is — the map itself does not draw actors out of sight, and neither
        // may anything drawn over it.
        let hidden: Vec<Effect> = (0..12).map(|_| effect(EffectKind::Wave).unseen()).collect();
        assert!(plan(&hidden, Speed::Normal).is_empty());

        let mut mixed = hidden;
        mixed.push(effect(EffectKind::Strike));
        let shots = plan(&mixed, Speed::Normal);
        assert_eq!(shots.len(), 1, "only the one in sight");
        assert_eq!(shots[0].effect.kind, EffectKind::Strike);
        assert!(
            shots[0].start.abs() < f32::EPSILON,
            "and it starts at once rather than queueing behind them"
        );
    }

    #[test]
    fn skipping_ends_it_at_once() {
        let mut anim = Anim::new();
        anim.load(&[effect(EffectKind::GateFell)], &settings());
        anim.skip();
        assert!(!anim.playing());
    }

    #[test]
    fn turning_animations_off_leaves_nothing_to_wait_for() {
        let mut off = settings();
        off.animations = false;
        let mut anim = Anim::new();
        anim.load(&[effect(EffectKind::NucleusLost)], &off);
        assert!(!anim.playing(), "the loop would still be blocking on it");
    }

    #[test]
    fn an_enormous_frame_cannot_wedge_the_clock() {
        let mut anim = Anim::new();
        anim.load(&[effect(EffectKind::Strike)], &settings());
        anim.tick(f32::INFINITY);
        assert!(!anim.playing());
        assert!(anim.clock.is_finite());
    }

    #[test]
    fn a_straight_line_is_walked_and_a_crooked_one_is_not() {
        let cells = line(Coord::new(3, 5), Coord::new(6, 5));
        assert_eq!(
            cells,
            vec![
                Coord::new(3, 5),
                Coord::new(4, 5),
                Coord::new(5, 5),
                Coord::new(6, 5)
            ]
        );
        assert_eq!(
            line(Coord::new(3, 5), Coord::new(3, 5)),
            vec![Coord::new(3, 5)]
        );
        assert_eq!(
            line(Coord::new(0, 0), Coord::new(3, 4)),
            vec![Coord::new(0, 0), Coord::new(3, 4)],
            "a diagonal is a relationship, not a line"
        );
    }

    #[test]
    fn easing_stays_inside_its_bounds() {
        for step in 0..=20 {
            let p = smooth(step as f32 / 20.0);
            assert!((0.0..=1.0).contains(&p), "{p}");
        }
        assert!(smooth(-3.0).abs() < f32::EPSILON);
        assert!((smooth(9.0) - 1.0).abs() < f32::EPSILON);
    }
}
