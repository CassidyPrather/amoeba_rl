//! Keys and taps in, at most one [`Command`] out.
//!
//! The game is turn-based and the sim takes one command per `advance`, so the
//! whole job here is to collapse a frame's worth of held keys, touches and
//! clicks into a single choice. Everything that decides what a command *means*
//! — which panel the arrows are steering, whether a move is legal — belongs to
//! the sim; this module only reads [`RenderView::mode`] to keep two keys from
//! firing where the original would have swallowed them.
//!
//! Two pieces of state survive between frames and are the reason this is a
//! struct rather than a function. [`Repeat`] is the auto-repeat a roguelike
//! needs and no windowing system agrees on, driven by wall-clock `dt` and
//! tested on its own. `pending` is the examine cursor's destination, because
//! the sim has no "put the cursor here" command and a tap therefore has to be
//! walked out one cell a frame.

use macroquad::input::{
    KeyCode, MouseButton, Touch, TouchPhase, get_last_key_pressed, is_key_down, is_key_pressed,
    is_mouse_button_down, is_mouse_button_pressed, mouse_position, touches,
};
use macroquad::math::{Rect, Vec2};

use amoeba_rl::sim::grid::{Coord, Dir};
use amoeba_rl::sim::{Command, Difficulty, Phase, RenderView, UiMode};

use crate::hud;
use crate::links::{self, LINKS, Link};
use crate::render::{Layout, LinkRow, Mode, post_mortem_panel, settings_panel, title_buttons};
use crate::settings::{ROWS, Settings, Toggled};

/// How long a held direction waits before it starts repeating.
const REPEAT_DELAY: f32 = 0.35;

/// How long between repeats once one has started.
const REPEAT_INTERVAL: f32 = 0.09;

/// The longest frame the repeat clock will believe. A backgrounded tab hands
/// back seconds at a time, and without this the amoeba would sprint across the
/// map the moment the window came back.
const MAX_DT: f32 = 0.1;

/// The smallest an on-screen control is allowed to get, in pixels. Thumbs.
///
/// Shared with [`render`](crate::render), whose settings panel is a column of
/// hit targets and answers to the same rule.
pub const MIN_TARGET: f32 = 48.0;

/// The largest one is allowed to get. A thumb is a thumb on a tablet too, and
/// the pad is charged for the space it takes: every pixel of it comes out of
/// the map.
const MAX_TARGET: f32 = 64.0;

/// A wall-clock seed, for the runs the sim is not allowed to seed itself.
#[must_use]
pub fn fresh_seed() -> u64 {
    macroquad::miniquad::date::now().to_bits()
}

/// One thing an on-screen button asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Pass the turn.
    Wait,
    /// Open or close the examine cursor.
    Examine,
    /// Open or close the organelle browser.
    Organelles,
    /// Hand control backwards through the nuclei.
    CyclePrev,
    /// Hand control forwards through the nuclei.
    CycleNext,
    /// Print the controls into the log.
    Help,
    /// Open or close the settings panel.
    Settings,
}

impl Action {
    /// What this button is called.
    ///
    /// A word rather than the key that does the same job, because the screens
    /// these buttons appear on have no keyboard to press an `X` or a `Z` on.
    /// The sim's own help uses the same words, so the two halves of the
    /// interface agree about what a thing is called.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Wait => "wait",
            Self::Examine => "look",
            Self::Organelles => "list",
            Self::CyclePrev => "prev",
            Self::CycleNext => "next",
            Self::Help => "help",
            Self::Settings => "menu",
        }
    }

    /// The command this button stands for, if the sim is the one being asked.
    ///
    /// The settings panel is a frontend overlay, so its button is the one that
    /// answers `None`: nothing about it reaches the sim.
    #[must_use]
    pub const fn command(self) -> Option<Command> {
        match self {
            Self::Wait => Some(Command::Wait),
            Self::Examine => Some(Command::ToggleExamine),
            Self::Organelles => Some(Command::ToggleOrganelles),
            Self::CyclePrev => Some(Command::CycleNucleus { forward: false }),
            Self::CycleNext => Some(Command::CycleNucleus { forward: true }),
            Self::Help => Some(Command::Help),
            Self::Settings => None,
        }
    }
}

/// Where the action buttons sit in their grid, bottom row first.
const BUTTON_GRID: [(Action, i32, i32); 6] = [
    (Action::CyclePrev, 0, 0),
    (Action::CycleNext, 1, 0),
    (Action::Help, 2, 0),
    (Action::Organelles, 0, 1),
    (Action::Examine, 1, 1),
    (Action::Wait, 2, 1),
];

/// The on-screen pad: geometry only, so hit testing can be tested without a
/// window and drawing can be tested by eye.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Controls {
    /// The window, so the title and post-mortem screens can share this one
    /// measurement of it.
    pub screen: Vec2,
    /// Whether the pad is on screen at all.
    pub visible: bool,
    /// The direction pad, a three-by-three grid of which four cells are live.
    pub dpad: Rect,
    /// The action buttons.
    pub buttons: [(Action, Rect); 6],
    /// The way into the settings panel on a screen with no keyboard. It sits
    /// one row above the button cluster rather than in it, because a fourth
    /// column of buttons does not fit a 320 px phone beside the pad — and
    /// inside the pad's band rather than off in a screen corner, because
    /// anywhere else is somewhere the map wanted to be.
    pub settings: Rect,
}

impl Controls {
    /// The live cells of the direction pad: what they mean, what they are
    /// labelled with, and where they sit in the three-by-three grid.
    pub const DPAD_CELLS: [(Dir, &'static str, i32, i32); 4] = [
        (Dir::Up, "\u{25B2}", 1, 0),
        (Dir::Left, "\u{25C4}", 0, 1),
        (Dir::Right, "\u{25BA}", 2, 1),
        (Dir::Down, "\u{25BC}", 1, 2),
    ];

    /// Place the pad in a window.
    #[must_use]
    pub fn fit(screen_w: f32, screen_h: f32, visible: bool) -> Self {
        let screen = Vec2::new(screen_w.max(1.0), screen_h.max(1.0));
        let (unit, margin) = metrics(screen);
        // Three rows of the pad's band: the dpad fills all three, the action
        // buttons the lower two, and the settings button the corner the
        // buttons leave over.
        let top = unit.mul_add(-3.0, screen.y - margin);
        let corner = Vec2::new(unit.mul_add(-3.0, screen.x - margin), top + unit);
        Self {
            screen,
            visible,
            dpad: Rect::new(margin, top, unit * 3.0, unit * 3.0),
            buttons: BUTTON_GRID.map(|(action, col, row)| {
                (
                    action,
                    Rect::new(
                        unit.mul_add(col as f32, corner.x),
                        unit.mul_add(row as f32, corner.y),
                        unit,
                        unit,
                    ),
                )
            }),
            settings: Rect::new(screen.x - margin - unit, top, unit, unit),
        }
    }

    /// The part of the window left over for the game.
    ///
    /// The pad is drawn over everything, so this is what keeps it from being
    /// drawn over *something*: the panels are laid out inside this rectangle
    /// and the pad has the rest to itself. On a tall screen it takes a band
    /// along the bottom; on a wide one it takes a column either side, because
    /// a phone on its side has height to spare nowhere and width to spare
    /// everywhere.
    #[must_use]
    pub fn free(screen_w: f32, screen_h: f32) -> Rect {
        let screen = Vec2::new(screen_w.max(1.0), screen_h.max(1.0));
        let (unit, margin) = metrics(screen);
        let band = unit.mul_add(3.0, margin * 2.0);
        let beside = screen.x > screen.y && (screen.x - band * 2.0) > screen.x * 0.5;
        if beside {
            Rect::new(band, 0.0, (screen.x - band * 2.0).max(1.0), screen.y)
        } else {
            Rect::new(0.0, 0.0, screen.x, (screen.y - band).max(1.0))
        }
    }

    /// Which direction a point on the pad asks for, if any. The corners are
    /// deliberately dead: a diagonal thumb should do nothing rather than pick.
    #[must_use]
    pub fn dpad_dir(&self, point: Vec2) -> Option<Dir> {
        if !self.visible || !self.dpad.contains(point) {
            return None;
        }
        let step = self.dpad.w / 3.0;
        let col = ((point.x - self.dpad.x) / step) as i32;
        let row = ((point.y - self.dpad.y) / step) as i32;
        Self::DPAD_CELLS
            .iter()
            .find_map(|&(dir, _, at_col, at_row)| (col == at_col && row == at_row).then_some(dir))
    }

    /// Which action a point asks for, if any.
    #[must_use]
    pub fn action_at(&self, point: Vec2) -> Option<Action> {
        if !self.visible {
            return None;
        }
        if self.settings.contains(point) {
            return Some(Action::Settings);
        }
        self.buttons
            .iter()
            .find_map(|&(action, rect)| rect.contains(point).then_some(action))
    }

    /// Whether a point is over any on-screen control, and so is not a map tap.
    #[must_use]
    pub fn covers(&self, point: Vec2) -> bool {
        self.visible && (self.dpad.contains(point) || self.action_at(point).is_some())
    }
}

/// One thumb-sized square, and the gap around the pad made of them.
///
/// The square grows with the screen up to a point and then stops: past
/// [`MAX_TARGET`] a bigger screen is buying a bigger thumb, which nobody has.
/// The last term is the one that matters on a small phone — six squares and
/// their margins have to fit across the window, or the pad and the buttons
/// meet in the middle.
fn metrics(screen: Vec2) -> (f32, f32) {
    let unit = (screen.x.min(screen.y) * 0.13)
        .clamp(MIN_TARGET, MAX_TARGET)
        .min(screen.x / 6.5)
        .max(1.0);
    (unit, (unit * 0.25).floor())
}

/// Auto-repeat for a held direction.
///
/// Fires once the instant a direction is taken up, waits [`REPEAT_DELAY`],
/// then fires every [`REPEAT_INTERVAL`] until it is let go. Changing direction
/// restarts the whole cycle, so a diagonal-ish roll of the thumb never
/// machine-guns.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Repeat {
    held: Option<Dir>,
    elapsed: f32,
    repeating: bool,
}

impl Repeat {
    /// Feed the frame: which direction is held, and how long the frame was.
    pub fn update(&mut self, held: Option<Dir>, dt: f32) -> Option<Dir> {
        let Some(dir) = held else {
            *self = Self::default();
            return None;
        };
        if self.held != Some(dir) {
            *self = Self {
                held: Some(dir),
                elapsed: 0.0,
                repeating: false,
            };
            return Some(dir);
        }
        self.elapsed += dt.clamp(0.0, MAX_DT);
        let threshold = if self.repeating {
            REPEAT_INTERVAL
        } else {
            REPEAT_DELAY
        };
        if self.elapsed < threshold {
            return None;
        }
        // Carry the remainder so the rate does not drift with the frame rate,
        // but never more than one interval of it.
        self.elapsed = (self.elapsed - threshold).min(threshold);
        self.repeating = true;
        Some(dir)
    }
}

/// One pointer this frame, whether it came from a finger or a mouse.
#[derive(Clone, Copy)]
struct Pointer {
    at: Vec2,
    /// It went down this frame, so it is a tap.
    pressed: bool,
    /// It is down now, so it can hold a direction.
    down: bool,
}

/// Touch ids that were alive last frame. A tap quick enough to begin and end
/// between two frames reaches us as a single `Ended` touch; only this memory
/// tells it apart from the tail of a touch whose press was already counted.
/// Browsers reuse ids (Chromium hands out 0 every time), so ended touches
/// must fall out immediately.
type TouchMemory = [Option<u64>; 8];

/// What one frame of input asked for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Frame {
    /// The single command to feed the sim, if any.
    pub command: Option<Command>,
    /// The mute key went down this frame.
    pub mute: bool,
}

/// The frontend state that outlives a frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    repeat: Repeat,
    /// Where a map tap sent the examine cursor. Walked one cell a frame,
    /// because the sim only moves the cursor by steps.
    pending: Option<Coord>,
    /// Whether a touch has ever arrived. Once one has, the on-screen pad stays
    /// out even on a screen big enough to do without it.
    touched: bool,
    /// Which page of the sidebar to draw. The sim keeps one too, in pages of
    /// a fixed fifty-five rows, but never puts it in the view — so `Q` and `E`
    /// are mirrored here, counted in the rows this window actually has.
    page: usize,
    /// See [`TouchMemory`].
    prev_touches: TouchMemory,
}

impl Input {
    /// Fresh state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            repeat: Repeat {
                held: None,
                elapsed: 0.0,
                repeating: false,
            },
            pending: None,
            touched: false,
            page: 0,
            prev_touches: [None; 8],
        }
    }

    /// Whether the on-screen pad should be drawn: someone has touched the
    /// screen, or the layout is the one that expects them to.
    #[must_use]
    pub const fn wants_controls(&self, mode: Mode) -> bool {
        self.touched || matches!(mode, Mode::Narrow)
    }

    /// Which page of the sidebar to draw.
    #[must_use]
    pub const fn page(&self) -> usize {
        self.page
    }

    /// Collapse this frame's input into at most one command.
    ///
    /// `settings` is the player's own state rather than the world's, so it is
    /// changed in place here and never becomes a [`Command`]: the sim resolves
    /// the same turn whatever the panel says.
    pub fn gather(
        &mut self,
        view: &RenderView,
        layout: &Layout,
        camera: Coord,
        controls: &Controls,
        settings: &mut Settings,
        dt: f32,
    ) -> Frame {
        let fingers = touches();
        self.touched |= !fingers.is_empty();
        // A keyboard turning up says as much as a finger did. The pad costs the
        // map the room it stands in, so a screen that has another way in should
        // not keep paying for one — a touchscreen laptop is not stuck with a
        // phone's layout because somebody once tapped the map.
        if get_last_key_pressed().is_some() {
            self.touched = false;
        }
        let pointers = pointers(&fingers, &self.prev_touches);
        self.prev_touches = live_touches(&fingers);
        let mut mute = is_key_pressed(KeyCode::M);

        // The panel is an overlay over every screen, so it is answered before
        // the phase is: while it is open it eats every key and every tap, and
        // the sim hears nothing at all.
        if settings.is_open() {
            self.repeat = Repeat::default();
            self.pending = None;
            mute |= menu(settings, controls, &pointers) == Some(Toggled::Sound);
            return Frame {
                command: None,
                mute,
            };
        }
        if opens_settings(controls, &pointers) {
            settings.toggle_panel();
            return Frame {
                command: None,
                mute,
            };
        }

        Frame {
            command: match view.phase {
                Phase::Playing => self.playing(view, layout, camera, controls, &pointers, dt),
                phase => {
                    // The menus have no auto-repeat and no cursor, and a key
                    // still held across the end of a run must not come back
                    // mid-repeat into the next one.
                    self.repeat = Repeat::default();
                    self.pending = None;
                    menu_screen(phase, layout, controls, &pointers)
                }
            },
            mute,
        }
    }

    /// A turn's worth of choice, in the order the original resolved it: keys
    /// that mean one thing, then movement, then the screen.
    fn playing(
        &mut self,
        view: &RenderView,
        layout: &Layout,
        camera: Coord,
        controls: &Controls,
        pointers: &[Pointer],
        dt: f32,
    ) -> Option<Command> {
        // Always feed the clock, even on a frame something else wins, or a key
        // held through a menu would come back already repeating.
        let step = self.repeat.update(held_direction(controls, pointers), dt);

        if let Some(command) = key_command(view.mode) {
            self.pending = None;
            if let Command::PageOrganelles(by) = command {
                self.turn_page(by, view, layout);
            }
            return Some(command);
        }
        if let Some(dir) = step {
            self.pending = None;
            return Some(Command::Move(dir));
        }
        if let Some(action) = pointers
            .iter()
            .filter(|pointer| pointer.pressed)
            .find_map(|pointer| controls.action_at(pointer.at))
        {
            self.pending = None;
            // `Settings` was answered before the phase was, so every action
            // that reaches here has a command behind it.
            return action.command();
        }
        // A pad press normally moves via the repeat machine the moment it is
        // seen held; if that emitted this frame we returned above, so this
        // only catches presses the hold path missed — chiefly one released
        // too fast to ever be seen down.
        if let Some(dir) = pointers
            .iter()
            .filter(|pointer| pointer.pressed)
            .find_map(|pointer| controls.dpad_dir(pointer.at))
        {
            self.pending = None;
            return Some(Command::Move(dir));
        }

        // The paging control listens wherever the sidebar is drawn, which in
        // the three-panel layout is all the time — a list you can read is a
        // list you can page, browser open or not.
        let sidebar_showing = !layout.sidebar_overlay || view.mode == UiMode::Organelles;
        if sidebar_showing
            && let Some(by) = pointers
                .iter()
                .filter(|pointer| pointer.pressed)
                .find_map(|pointer| sidebar_page_at(layout, pointer.at))
        {
            self.turn_page(by, view, layout);
            return Some(Command::PageOrganelles(by));
        }

        // The organelle browser owns the whole screen while it is open — in
        // the map-first layout it is literally drawn over the map — so nothing
        // there is a map tap.
        if view.mode != UiMode::Organelles
            && let Some(cell) = pointers
                .iter()
                .filter(|pointer| pointer.pressed && !controls.covers(pointer.at))
                .find_map(|pointer| layout.cell_at(pointer.at, camera))
        {
            return self.map_tap(view, cell);
        }
        self.follow_pending(view)
    }

    /// A tap on the map. In examine mode it is a destination for the cursor;
    /// in live mode it is a single step, and only if it is a single step away
    /// — a tap across the room would otherwise spend several turns getting
    /// there without ever being asked to confirm.
    fn map_tap(&mut self, view: &RenderView, cell: Coord) -> Option<Command> {
        if let Some(examine) = view.examine.as_ref() {
            self.pending = Some(cell);
            return step_toward(examine.pos, cell).map(Command::MoveCursor);
        }
        self.pending = None;
        let (_, nucleus) = view.status.active_nucleus.as_ref()?;
        nucleus
            .adjacent_to(cell)
            .then(|| step_toward(*nucleus, cell).map(Command::Move))?
    }

    /// Move the sidebar by whole screens, wrapping at both ends.
    fn turn_page(&mut self, by: i32, view: &RenderView, layout: &Layout) {
        let footer = layout.sidebar_pager().map_or(1, |pager| pager.rows);
        let pages = hud::pages(
            view.organelles.len(),
            hud::sidebar_capacity(layout.sidebar_rows, footer),
        );
        let pages = i32::try_from(pages).unwrap_or(1).max(1);
        let at = i32::try_from(self.page).unwrap_or(0);
        self.page = usize::try_from((at + by).rem_euclid(pages)).unwrap_or(0);
    }

    /// Keep walking the examine cursor toward wherever it was last sent.
    fn follow_pending(&mut self, view: &RenderView) -> Option<Command> {
        let target = self.pending?;
        let Some(examine) = view.examine.as_ref() else {
            self.pending = None;
            return None;
        };
        let step = step_toward(examine.pos, target);
        if step.is_none() {
            self.pending = None;
        }
        step.map(Command::MoveCursor)
    }
}

/// The keys that mean exactly one thing.
///
/// `mode` is only consulted to reproduce the original's dispatch: its examine
/// handler consumed every key it did not use, so nucleus cycling and sidebar
/// paging are unreachable while the cursor is out.
fn key_command(mode: UiMode) -> Option<Command> {
    if [
        KeyCode::Space,
        KeyCode::Period,
        KeyCode::KpDecimal,
        KeyCode::Kp5,
    ]
    .into_iter()
    .any(is_key_pressed)
    {
        return Some(Command::Wait);
    }
    if is_key_pressed(KeyCode::X) {
        return Some(Command::ToggleExamine);
    }
    if is_key_pressed(KeyCode::Z) {
        return Some(Command::ToggleOrganelles);
    }
    if is_key_pressed(KeyCode::Escape) {
        return Some(Command::Back);
    }
    if is_key_pressed(KeyCode::F1) {
        return Some(Command::Help);
    }
    if mode == UiMode::Examine {
        return None;
    }
    if is_key_pressed(KeyCode::A) {
        return Some(Command::CycleNucleus { forward: false });
    }
    if is_key_pressed(KeyCode::D) {
        return Some(Command::CycleNucleus { forward: true });
    }
    if is_key_pressed(KeyCode::Q) {
        return Some(Command::PageOrganelles(-1));
    }
    if is_key_pressed(KeyCode::E) {
        return Some(Command::PageOrganelles(1));
    }
    None
}

/// Whether the player asked for the settings panel. Works on every screen, and
/// on a phone comes from the one button that sits outside the pad.
fn opens_settings(controls: &Controls, pointers: &[Pointer]) -> bool {
    is_key_pressed(KeyCode::S)
        || is_key_pressed(KeyCode::F2)
        || pointers
            .iter()
            .filter(|pointer| pointer.pressed)
            .any(|pointer| controls.action_at(pointer.at) == Some(Action::Settings))
}

/// A frame of the settings panel: move the selection, change a row, or leave.
///
/// Returns what the row that changed wants doing about it, so the one setting
/// that lives elsewhere — mute — can be handed back to the audio half.
///
/// There is no auto-repeat in here on purpose: four rows do not need one, and
/// the arrow keys are the only thing a held key could run away with. Left and
/// right both cycle forwards, which only shows on the three-way speed row.
fn menu(settings: &mut Settings, controls: &Controls, pointers: &[Pointer]) -> Option<Toggled> {
    if is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::S) || is_key_pressed(KeyCode::F2)
    {
        settings.close();
        return None;
    }
    if is_key_pressed(KeyCode::Up) {
        settings.scroll(-1);
    }
    if is_key_pressed(KeyCode::Down) {
        settings.scroll(1);
    }
    let geometry = settings_panel(controls.screen.x, controls.screen.y);
    let tapped = |rect: Rect| {
        pointers
            .iter()
            .any(|pointer| pointer.pressed && rect.contains(pointer.at))
    };
    if tapped(geometry.close) {
        settings.close();
        return None;
    }
    if let Some(row) = ROWS
        .iter()
        .zip(geometry.rows)
        .find_map(|(row, rect)| tapped(rect).then_some(*row))
    {
        return Some(settings.toggle(row));
    }
    if [
        KeyCode::Enter,
        KeyCode::KpEnter,
        KeyCode::Space,
        KeyCode::Left,
        KeyCode::Right,
    ]
    .into_iter()
    .any(is_key_pressed)
    {
        return Some(settings.toggle_selected());
    }
    None
}

/// The difficulty choice, by key or by button.
fn title(controls: &Controls, pointers: &[Pointer]) -> Option<Command> {
    const CHOICES: [(Difficulty, KeyCode, KeyCode); 3] = [
        (Difficulty::Normal, KeyCode::Key1, KeyCode::N),
        (Difficulty::Easy, KeyCode::Key2, KeyCode::E),
        (Difficulty::Gj, KeyCode::Key3, KeyCode::G),
    ];
    for (difficulty, digit, letter) in CHOICES {
        if is_key_pressed(digit) || is_key_pressed(letter) {
            return Some(Command::Start(difficulty));
        }
    }
    let buttons = title_buttons(controls.screen.x, controls.screen.y);
    pointers
        .iter()
        .filter(|pointer| pointer.pressed)
        .find_map(|pointer| {
            buttons
                .iter()
                .position(|button| button.contains(pointer.at))
                .and_then(|i| CHOICES.get(i))
                .map(|&(difficulty, _, _)| Command::Start(difficulty))
        })
}

/// The two screens that are not the game: the title and the post-mortem.
///
/// They share a row of links, and the link has first claim on a press. A tap
/// that opens one is not a command and must not fall through to the difficulty
/// button or the restart button it was drawn near.
fn menu_screen(
    phase: Phase,
    layout: &Layout,
    controls: &Controls,
    pointers: &[Pointer],
) -> Option<Command> {
    let row = if phase == Phase::Title {
        LinkRow::title(
            controls.screen.x,
            controls.screen.y,
            layout.cell,
            layout.touch,
        )
    } else {
        LinkRow::post_mortem(controls.screen.x, controls.screen.y)
    };
    if let Some(link) = link_at(row, pointers) {
        links::open(link);
        return None;
    }
    if phase == Phase::Title {
        title(controls, pointers)
    } else {
        game_over(controls, pointers)
    }
}

/// Which link a press landed on, if any.
///
/// Pointer-only, and deliberately so: a link is a thing you point at, every
/// letter the title screen could spare is already spoken for by the
/// difficulties, and a key that opened a browser tab from under someone's
/// fingers would be a worse surprise than a link they have to click.
fn link_at(row: LinkRow, pointers: &[Pointer]) -> Option<Link> {
    let rects = row.rects();
    pointers
        .iter()
        .filter(|pointer| pointer.pressed)
        .find_map(|pointer| {
            rects
                .iter()
                .position(|rect| rect.contains(pointer.at))
                .and_then(|i| LINKS.get(i).copied())
        })
}

/// Restarting. `Esc` is not wired up: the original quit on it, and a browser
/// tab has nothing to quit to.
fn game_over(controls: &Controls, pointers: &[Pointer]) -> Option<Command> {
    let (_, button) = post_mortem_panel(controls.screen.x, controls.screen.y);
    let tapped = pointers
        .iter()
        .any(|pointer| pointer.pressed && button.contains(pointer.at));
    (is_key_pressed(KeyCode::R) || tapped).then(|| Command::Restart { seed: fresh_seed() })
}

/// Which way a tap on the organelle list's paging control asks to go, if the
/// control is on screen at all and the tap landed on it.
fn sidebar_page_at(layout: &Layout, point: Vec2) -> Option<i32> {
    let pager = layout.sidebar_pager()?;
    pager
        .buttons
        .iter()
        .zip([-1, 1])
        .find_map(|(rect, by)| rect.contains(point).then_some(by))
}

/// Whichever direction is being held: arrow keys first, then the pad.
fn held_direction(controls: &Controls, pointers: &[Pointer]) -> Option<Dir> {
    const KEYS: [(KeyCode, Dir); 4] = [
        (KeyCode::Up, Dir::Up),
        (KeyCode::Down, Dir::Down),
        (KeyCode::Left, Dir::Left),
        (KeyCode::Right, Dir::Right),
    ];
    for (key, dir) in KEYS {
        if is_key_down(key) {
            return Some(dir);
        }
    }
    pointers
        .iter()
        .filter(|pointer| pointer.down)
        .find_map(|pointer| controls.dpad_dir(pointer.at))
}

/// One cell's worth of progress from `from` toward `to`, longest axis first.
#[must_use]
pub const fn step_toward(from: Coord, to: Coord) -> Option<Dir> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if dx == 0 && dy == 0 {
        return None;
    }
    Some(if dx.abs() >= dy.abs() {
        if dx > 0 { Dir::Right } else { Dir::Left }
    } else if dy > 0 {
        Dir::Down
    } else {
        Dir::Up
    })
}

/// The finger half of [`pointers`], macroquad-free so it can be tested. A
/// touch whose id was not alive last frame counts as pressed whatever its
/// phase: a fast enough tap shows up already `Ended`, and its press must
/// still be counted.
fn touch_pointers(fingers: &[Touch], prev: &TouchMemory) -> Vec<Pointer> {
    fingers
        .iter()
        .map(|touch| Pointer {
            at: touch.position,
            pressed: !prev.contains(&Some(touch.id)),
            down: !matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled),
        })
        .collect()
}

/// Every pointer the window has this frame, fingers and mouse alike, so the
/// same hit tests serve a phone and a desktop.
fn pointers(fingers: &[Touch], prev: &TouchMemory) -> Vec<Pointer> {
    let mut out = touch_pointers(fingers, prev);
    let pressed = is_mouse_button_pressed(MouseButton::Left);
    let down = is_mouse_button_down(MouseButton::Left);
    if pressed || down {
        let (x, y) = mouse_position();
        out.push(Pointer {
            at: Vec2::new(x, y),
            pressed,
            down,
        });
    }
    out
}

/// The ids still alive after this frame — ended touches drop out at once so a
/// browser reusing their id (Chromium always says 0) reads as a fresh press.
fn live_touches(fingers: &[Touch]) -> TouchMemory {
    let mut out = [None; 8];
    let live = fingers
        .iter()
        .filter(|touch| !matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled));
    for (slot, touch) in out.iter_mut().zip(live) {
        *slot = Some(touch.id);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame at sixty hertz.
    const FRAME: f32 = 1.0 / 60.0;

    fn pad() -> Controls {
        Controls::fit(390.0, 844.0, true)
    }

    #[test]
    fn a_fresh_press_fires_at_once() {
        let mut repeat = Repeat::default();
        assert_eq!(repeat.update(Some(Dir::Up), FRAME), Some(Dir::Up));
    }

    fn touch(id: u64, phase: TouchPhase) -> Touch {
        Touch {
            id,
            phase,
            position: Vec2::new(1.0, 2.0),
        }
    }

    #[test]
    fn a_tap_faster_than_a_frame_still_counts_as_a_press() {
        // Began and ended between two frames: all we ever see is `Ended`.
        let blink = [touch(0, TouchPhase::Ended)];
        let seen = touch_pointers(&blink, &[None; 8]);
        assert!(seen[0].pressed && !seen[0].down);
    }

    #[test]
    fn a_touch_is_pressed_on_its_first_frame_only() {
        let start = [touch(3, TouchPhase::Started)];
        assert!(touch_pointers(&start, &[None; 8])[0].pressed);

        let memory = live_touches(&start);
        let moved = [touch(3, TouchPhase::Moved)];
        let seen = touch_pointers(&moved, &memory);
        assert!(!seen[0].pressed && seen[0].down);
    }

    #[test]
    fn an_ended_touch_frees_its_id_for_the_next_tap() {
        // Chromium reuses identifier 0 for every successive tap.
        let ended = [touch(0, TouchPhase::Ended)];
        assert_eq!(live_touches(&ended), [None; 8]);
        assert!(touch_pointers(&[touch(0, TouchPhase::Started)], &live_touches(&ended))[0].pressed);
    }

    #[test]
    fn holding_waits_out_the_delay_then_repeats() {
        let mut repeat = Repeat::default();
        assert_eq!(repeat.update(Some(Dir::Left), FRAME), Some(Dir::Left));

        // One frame short of the delay, so floating point cannot tip it over.
        let quiet = ((REPEAT_DELAY / FRAME) as i32 - 1).max(0);
        assert!(quiet > 15, "the delay is long enough to be deliberate");
        for _ in 0..quiet {
            assert_eq!(repeat.update(Some(Dir::Left), FRAME), None);
        }

        // Somewhere in the next couple of frames the delay elapses.
        let mut repeats = 0;
        for _ in 0..60 {
            if repeat.update(Some(Dir::Left), FRAME).is_some() {
                repeats += 1;
            }
        }
        // One second of frames past the delay, at ninety milliseconds a step.
        assert!((9..=12).contains(&repeats), "repeated {repeats} times");
    }

    #[test]
    fn letting_go_resets_the_cycle() {
        let mut repeat = Repeat::default();
        repeat.update(Some(Dir::Down), FRAME);
        for _ in 0..40 {
            repeat.update(Some(Dir::Down), FRAME);
        }
        assert_eq!(repeat.update(None, FRAME), None);
        // The next press is immediate again, not still mid-repeat.
        assert_eq!(repeat.update(Some(Dir::Down), FRAME), Some(Dir::Down));
        assert_eq!(repeat.update(Some(Dir::Down), FRAME), None);
    }

    #[test]
    fn changing_direction_starts_over() {
        let mut repeat = Repeat::default();
        repeat.update(Some(Dir::Up), FRAME);
        for _ in 0..40 {
            repeat.update(Some(Dir::Up), FRAME);
        }
        assert_eq!(repeat.update(Some(Dir::Right), FRAME), Some(Dir::Right));
        assert_eq!(repeat.update(Some(Dir::Right), FRAME), None);
    }

    #[test]
    fn a_backgrounded_tab_does_not_come_back_sprinting() {
        let mut repeat = Repeat::default();
        assert_eq!(repeat.update(Some(Dir::Up), FRAME), Some(Dir::Up));
        // Ten seconds in one frame is believed only as far as `MAX_DT`, so the
        // delay still takes several frames to run out and the amoeba crosses
        // one cell rather than four.
        let fired = (0..4)
            .filter(|_| repeat.update(Some(Dir::Up), 10.0).is_some())
            .count();
        assert_eq!(fired, 1, "four enormous frames, one step");
    }

    #[test]
    fn the_pad_reads_its_four_live_cells() {
        let controls = pad();
        let step = controls.dpad.w / 3.0;
        let corner = controls.dpad.point();
        let at = |col: f32, row: f32| corner + Vec2::new(step * (col + 0.5), step * (row + 0.5));
        assert_eq!(controls.dpad_dir(at(1.0, 0.0)), Some(Dir::Up));
        assert_eq!(controls.dpad_dir(at(0.0, 1.0)), Some(Dir::Left));
        assert_eq!(controls.dpad_dir(at(2.0, 1.0)), Some(Dir::Right));
        assert_eq!(controls.dpad_dir(at(1.0, 2.0)), Some(Dir::Down));
    }

    #[test]
    fn the_pads_corners_and_centre_are_dead() {
        let controls = pad();
        let step = controls.dpad.w / 3.0;
        let corner = controls.dpad.point();
        let at = |col: f32, row: f32| corner + Vec2::new(step * (col + 0.5), step * (row + 0.5));
        for (col, row) in [(0.0, 0.0), (2.0, 0.0), (0.0, 2.0), (2.0, 2.0), (1.0, 1.0)] {
            assert_eq!(controls.dpad_dir(at(col, row)), None);
        }
        assert_eq!(controls.dpad_dir(Vec2::new(-10.0, -10.0)), None);
    }

    #[test]
    fn an_invisible_pad_swallows_nothing() {
        let controls = Controls::fit(390.0, 844.0, false);
        assert_eq!(controls.dpad_dir(controls.dpad.center()), None);
        assert_eq!(controls.action_at(controls.buttons[0].1.center()), None);
        assert!(!controls.covers(controls.dpad.center()));
    }

    #[test]
    fn every_button_is_reachable_and_thumb_sized() {
        let controls = pad();
        for (action, rect) in controls.buttons {
            assert!(rect.w >= MIN_TARGET && rect.h >= MIN_TARGET);
            assert_eq!(controls.action_at(rect.center()), Some(action));
            assert!(rect.right() <= controls.screen.x);
            assert!(rect.bottom() <= controls.screen.y);
        }
    }

    #[test]
    fn the_pad_and_the_buttons_do_not_overlap() {
        for (width, height) in [
            (320.0, 568.0),
            (390.0, 844.0),
            (844.0, 390.0),
            (1440.0, 900.0),
        ] {
            let controls = Controls::fit(width, height, true);
            let buttons = controls
                .buttons
                .iter()
                .fold(controls.buttons[0].1, |acc, &(_, rect)| {
                    acc.combine_with(rect)
                });
            assert!(
                controls.dpad.right() <= buttons.left(),
                "{width}x{height}: pad and buttons collided"
            );
            assert!(controls.dpad.x >= 0.0 && controls.dpad.bottom() <= height);
        }
    }

    #[test]
    fn every_action_maps_to_a_command() {
        use Action::{CycleNext, CyclePrev, Examine, Help, Organelles, Settings, Wait};
        assert_eq!(Wait.command(), Some(Command::Wait));
        assert_eq!(Examine.command(), Some(Command::ToggleExamine));
        assert_eq!(Organelles.command(), Some(Command::ToggleOrganelles));
        assert_eq!(
            CyclePrev.command(),
            Some(Command::CycleNucleus { forward: false })
        );
        assert_eq!(
            CycleNext.command(),
            Some(Command::CycleNucleus { forward: true })
        );
        assert_eq!(Help.command(), Some(Command::Help));
        // Except the settings panel, which the sim is not told about at all.
        assert_eq!(Settings.command(), None);
    }

    #[test]
    fn the_settings_button_is_reachable_and_out_of_everything_else_way() {
        for (width, height) in [
            (320.0, 568.0),
            (390.0, 844.0),
            (844.0, 390.0),
            (1440.0, 900.0),
        ] {
            let controls = Controls::fit(width, height, true);
            let gear = controls.settings;
            assert!(gear.w >= MIN_TARGET && gear.h >= MIN_TARGET);
            assert_eq!(
                controls.action_at(gear.center()),
                Some(Action::Settings),
                "{width}x{height}"
            );
            assert!(controls.covers(gear.center()), "it swallowed a map tap");
            assert!(gear.right() <= width && gear.bottom() <= height);
            // It sits in the row above the button cluster: clear of the pad,
            // and touching the buttons without ever taking one of their taps.
            assert!(!controls.dpad.overlaps(&gear), "{width}x{height}");
            for (action, rect) in controls.buttons {
                assert!(
                    gear.bottom() <= rect.y,
                    "{width}x{height}: it sat on a button"
                );
                assert_eq!(
                    controls.action_at(rect.center()),
                    Some(action),
                    "{width}x{height}: it stole a button's taps"
                );
            }
        }
    }

    #[test]
    fn stepping_toward_takes_the_longest_axis_first() {
        let from = Coord::new(10, 10);
        assert_eq!(step_toward(from, Coord::new(20, 12)), Some(Dir::Right));
        assert_eq!(step_toward(from, Coord::new(2, 12)), Some(Dir::Left));
        assert_eq!(step_toward(from, Coord::new(12, 20)), Some(Dir::Down));
        assert_eq!(step_toward(from, Coord::new(12, 2)), Some(Dir::Up));
        assert_eq!(step_toward(from, from), None);
    }

    #[test]
    fn stepping_toward_always_closes_the_distance() {
        let from = Coord::new(5, 9);
        for x in 0..20 {
            for y in 0..20 {
                let to = Coord::new(x, y);
                let Some(dir) = step_toward(from, to) else {
                    assert_eq!(from, to);
                    continue;
                };
                assert!((from + dir.offset()).taxi(to) < from.taxi(to));
            }
        }
    }

    #[test]
    fn the_dpad_table_covers_each_direction_once() {
        for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
            assert_eq!(
                Controls::DPAD_CELLS
                    .iter()
                    .filter(|cell| cell.0 == dir)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn the_pad_hides_itself_on_a_desktop_until_a_finger_arrives() {
        let mut input = Input::new();
        assert!(!input.wants_controls(Mode::Wide));
        assert!(input.wants_controls(Mode::Narrow));
        input.touched = true;
        assert!(input.wants_controls(Mode::Wide));
    }
}
