//! Where things go on the screen, and the drawing of everything that is not a
//! panel.
//!
//! The original was a fixed 86×59 console at 12 px a cell, which is a lovely
//! thing on a desktop and unusable on a phone. [`Layout::fit`] keeps that shape
//! wherever it fits and falls back to a map-first arrangement — full-width map,
//! collapsed log, sidebar as an overlay — wherever it does not. Both branches
//! are integer-cell letterboxed, so glyphs always land on whole pixels.
//!
//! Everything in here that decides *where* is a pure function of the screen
//! size and the map size and is unit-tested; everything that decides *what* was
//! already decided by [`amoeba_rl::sim::RenderView`]. What is left is a loop
//! that draws a rectangle and a glyph per cell.

use macroquad::color::Color;
use macroquad::math::{Rect, Vec2};
use macroquad::shapes::{draw_rectangle, draw_rectangle_lines};
use macroquad::window::clear_background;

use amoeba_rl::sim::actors::{Rgb, palette};
use amoeba_rl::sim::grid::Coord;
use amoeba_rl::sim::view::CellView;
use amoeba_rl::sim::{Phase, RenderView, UiMode};

use crate::hud;
use crate::input::{Action, Controls};
use crate::tileset::Tileset;

/// Cells the organelle sidebar is wide, as the original's player console was.
pub const SIDEBAR_COLS: i32 = 22;

/// Cells the message bar is tall in the three-panel layout: nine log lines, a
/// margin, and a row of key hints.
pub const INFO_ROWS: i32 = 11;

/// Cells the message bar keeps once it has collapsed: four log lines and the
/// hints.
pub const NARROW_INFO_ROWS: i32 = 6;

/// Cell size below which the three-panel layout stops being worth its sidebar.
const WIDE_MIN_CELL: f32 = 9.0;

/// Screen aspect below which the sidebar is stealing width the map needs. The
/// reference layout is 86:59, so anything squarer than roughly 4:3 is already
/// letterboxing badly.
const WIDE_MIN_ASPECT: f32 = 1.25;

/// The smallest map cell the map-first layout will draw. Below this it scrolls
/// instead of shrinking: a 390 px phone would otherwise render a 48-column map
/// at eight pixels a cell, and twelve-pixel glyphs do not survive that.
const NARROW_MIN_CELL: f32 = 10.0;

/// Behind the letterbox margins.
const VOID: Color = Color::new(0.02, 0.02, 0.03, 1.0);

/// Behind the message bar, as the original's info console was.
const INFO_BACKGROUND: Rgb = [48, 61, 59];

/// Behind anything drawn over the map: the sidebar overlay, the status strip,
/// the post-mortem panel.
const OVERLAY_ALPHA: f32 = 0.92;

/// Which arrangement of panels is on screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Map, message bar underneath, sidebar to the right. The original.
    Wide,
    /// Map first: full width, collapsed message bar, sidebar on demand.
    Narrow,
}

/// Every rectangle a frame is drawn into, in window pixels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Layout {
    /// Which arrangement this is.
    pub mode: Mode,
    /// Pixels along one cell's edge. Always a whole number of pixels once
    /// there is more than one to go around.
    pub cell: f32,
    /// The map viewport.
    pub map: Rect,
    /// Map columns the viewport shows.
    pub cols: i32,
    /// Map rows the viewport shows.
    pub rows: i32,
    /// Whether the viewport is smaller than the map, and so needs a camera.
    pub scrolls: bool,
    /// The message bar.
    pub info: Rect,
    /// Rows the message bar has.
    pub info_rows: i32,
    /// The organelle sidebar.
    pub sidebar: Rect,
    /// Rows the sidebar has.
    pub sidebar_rows: i32,
    /// Whether the sidebar is drawn over the map rather than beside it, and so
    /// only when the organelle browser is open.
    pub sidebar_overlay: bool,
}

impl Layout {
    /// Fit the game to a window.
    ///
    /// `map_w` and `map_h` are the sim's map size, which the difficulty picks:
    /// 48×48 normally and 64×48 on GJ. Laying out from those rather than from
    /// the original's fixed 64 means the smaller maps do not pay for columns
    /// they never draw.
    #[must_use]
    pub fn fit(screen_w: f32, screen_h: f32, map_w: i32, map_h: i32) -> Self {
        // A minimised window or a hidden tab reports zero; the floor keeps
        // every division below finite.
        let width = screen_w.max(1.0);
        let height = screen_h.max(1.0);
        let map_w = map_w.max(1);
        let map_h = map_h.max(1);
        let wide_cell =
            snap((width / (map_w + SIDEBAR_COLS) as f32).min(height / (map_h + INFO_ROWS) as f32));
        if width / height >= WIDE_MIN_ASPECT && wide_cell >= WIDE_MIN_CELL {
            Self::wide(width, height, map_w, map_h, wide_cell)
        } else {
            Self::narrow(width, height, map_w, map_h)
        }
    }

    /// The original's three panels, centred in whatever is left over.
    fn wide(width: f32, height: f32, map_w: i32, map_h: i32, cell: f32) -> Self {
        let map_px = Vec2::new(map_w as f32 * cell, map_h as f32 * cell);
        let info_h = INFO_ROWS as f32 * cell;
        let total = Vec2::new(
            (SIDEBAR_COLS as f32).mul_add(cell, map_px.x),
            map_px.y + info_h,
        );
        let origin = letterbox(width, height, total);
        Self {
            mode: Mode::Wide,
            cell,
            map: Rect::new(origin.x, origin.y, map_px.x, map_px.y),
            cols: map_w,
            rows: map_h,
            scrolls: false,
            info: Rect::new(origin.x, origin.y + map_px.y, map_px.x, info_h),
            info_rows: INFO_ROWS,
            sidebar: Rect::new(
                origin.x + map_px.x,
                origin.y,
                SIDEBAR_COLS as f32 * cell,
                total.y,
            ),
            sidebar_rows: map_h + INFO_ROWS,
            sidebar_overlay: false,
        }
    }

    /// Map first. The sidebar keeps its 22 columns but moves on top of the
    /// map, where the organelle browser can call it up and dismiss it again.
    fn narrow(width: f32, height: f32, map_w: i32, map_h: i32) -> Self {
        let full = (width / map_w as f32).min(height / (map_h + NARROW_INFO_ROWS) as f32);
        let cell = snap(full.max(NARROW_MIN_CELL));
        let cols = map_w.min(floor_div(width, cell).max(1));
        let rows = map_h.min((floor_div(height, cell) - NARROW_INFO_ROWS).max(1));
        let map_px = Vec2::new(cols as f32 * cell, rows as f32 * cell);
        let info_h = NARROW_INFO_ROWS as f32 * cell;
        let origin = letterbox(width, height, Vec2::new(map_px.x, map_px.y + info_h));
        let sidebar_cols = SIDEBAR_COLS.min(cols);
        Self {
            mode: Mode::Narrow,
            cell,
            map: Rect::new(origin.x, origin.y, map_px.x, map_px.y),
            cols,
            rows,
            scrolls: cols < map_w || rows < map_h,
            info: Rect::new(origin.x, origin.y + map_px.y, map_px.x, info_h),
            info_rows: NARROW_INFO_ROWS,
            sidebar: Rect::new(
                (sidebar_cols as f32).mul_add(-cell, origin.x + map_px.x),
                origin.y,
                sidebar_cols as f32 * cell,
                map_px.y + info_h,
            ),
            sidebar_rows: rows + NARROW_INFO_ROWS,
            sidebar_overlay: true,
        }
    }

    /// The map cell under a window point, if the point is over the map at all.
    #[must_use]
    pub fn cell_at(&self, point: Vec2, camera: Coord) -> Option<Coord> {
        if !self.map.contains(point) || self.cell <= 0.0 {
            return None;
        }
        let col = ((point.x - self.map.x) / self.cell) as i32;
        let row = ((point.y - self.map.y) / self.cell) as i32;
        (col < self.cols && row < self.rows).then(|| Coord::new(camera.x + col, camera.y + row))
    }
}

/// The top-left map cell a viewport should start at to keep `focus` centred
/// without ever showing past the edge of the map.
#[must_use]
pub fn camera_origin(focus: Coord, cols: i32, rows: i32, map_w: i32, map_h: i32) -> Coord {
    Coord::new(
        (focus.x - cols / 2).clamp(0, (map_w - cols).max(0)),
        (focus.y - rows / 2).clamp(0, (map_h - rows).max(0)),
    )
}

/// The largest whole number of pixels that fits, or the fraction itself when
/// even one pixel a cell is too many.
fn snap(cell: f32) -> f32 {
    let whole = cell.floor();
    if whole >= 1.0 {
        whole
    } else {
        cell.max(f32::EPSILON)
    }
}

/// How many whole cells of `cell` pixels fit in `span`.
fn floor_div(span: f32, cell: f32) -> i32 {
    (span / cell.max(f32::EPSILON)) as i32
}

/// The largest glyph size at which `text` still fits inside `width`.
///
/// The screens outside the map lay text out in pixels rather than cells, and a
/// phone is narrow enough that a line of key hints has to shrink rather than
/// run off the edge.
#[must_use]
pub fn fit_text(text: &str, width: f32, max: f32) -> f32 {
    (width / text.chars().count().max(1) as f32)
        .min(max)
        .max(1.0)
}

/// Centre `content` in the window, on whole pixels so cells stay crisp.
fn letterbox(width: f32, height: f32, content: Vec2) -> Vec2 {
    Vec2::new(
        ((width - content.x) * 0.5).floor().max(0.0),
        ((height - content.y) * 0.5).floor().max(0.0),
    )
}

/// A sim colour as a macroquad one.
#[must_use]
pub const fn rgb(color: Rgb) -> Color {
    Color::from_rgba(color[0], color[1], color[2], 255)
}

/// A sim colour, faded.
#[must_use]
pub fn rgba(color: Rgb, alpha: f32) -> Color {
    Color::new(
        f32::from(color[0]) / 255.0,
        f32::from(color[1]) / 255.0,
        f32::from(color[2]) / 255.0,
        alpha,
    )
}

/// A terminal-shaped painter over one rectangle of the window.
///
/// Cells are addressed in columns and rows from the rectangle's corner, which
/// is what lets [`hud`] be written against the original's console coordinates
/// without knowing where on screen its panel ended up.
pub struct Term<'a> {
    font: &'a Tileset,
    origin: Vec2,
    cell: f32,
}

impl<'a> Term<'a> {
    /// A painter over `area`, one cell every `cell` pixels.
    #[must_use]
    pub const fn new(font: &'a Tileset, area: Rect, cell: f32) -> Self {
        Self {
            font,
            origin: area.point(),
            cell,
        }
    }

    /// Pixels along one cell's edge.
    #[must_use]
    pub const fn cell_size(&self) -> f32 {
        self.cell
    }

    /// The window position of this painter's top-left corner.
    #[must_use]
    pub const fn origin(&self) -> Vec2 {
        self.origin
    }

    /// Background rectangle, then glyph on top: the whole of the draw order.
    ///
    /// Use [`Term::grid`] for anything larger than a few cells — see there for
    /// why the two halves want separating.
    pub fn cell(&self, col: i32, row: i32, glyph: char, fg: Rgb, bg: Rgb) {
        self.background(col, row, bg);
        self.glyph(col, row, glyph, fg);
    }

    /// A whole rectangle of cells: every background, then every glyph.
    ///
    /// The order matters for speed rather than for looks. macroquad starts a
    /// new draw call whenever the bound texture changes, and a background is
    /// untextured while a glyph is not, so drawing them cell by cell would
    /// cost a draw call per cell — six thousand of them on a full map. Two
    /// passes cost two batches.
    pub fn grid<F: Fn(i32, i32) -> Option<CellView>>(&self, cols: i32, rows: i32, cell: F) {
        for row in 0..rows {
            for col in 0..cols {
                if let Some(cell) = cell(col, row) {
                    self.background(col, row, cell.bg);
                }
            }
        }
        for row in 0..rows {
            for col in 0..cols {
                if let Some(cell) = cell(col, row) {
                    self.glyph(col, row, cell.glyph, cell.fg);
                }
            }
        }
    }

    /// Just the background of one cell.
    pub fn background(&self, col: i32, row: i32, bg: Rgb) {
        let at = self.at(col, row);
        draw_rectangle(at.x, at.y, self.cell, self.cell, rgb(bg));
    }

    /// Just the glyph of one cell. Spaces cost nothing.
    pub fn glyph(&self, col: i32, row: i32, glyph: char, fg: Rgb) {
        if glyph == ' ' {
            return;
        }
        let at = self.at(col, row);
        self.font.draw(glyph, at.x, at.y, self.cell, rgb(fg));
    }

    /// A string, left to right from `col`.
    pub fn text(&self, col: i32, row: i32, text: &str, fg: Rgb) {
        for (i, glyph) in text.chars().enumerate() {
            let Ok(offset) = i32::try_from(i) else { return };
            self.glyph(col + offset, row, glyph, fg);
        }
    }

    /// A string with its own background, for the sidebar's navigation hints.
    pub fn text_on(&self, col: i32, row: i32, text: &str, fg: Rgb, bg: Rgb) {
        for (i, glyph) in text.chars().enumerate() {
            let Ok(offset) = i32::try_from(i) else { return };
            self.cell(col + offset, row, glyph, fg, bg);
        }
    }

    /// The window position of a cell's top-left corner.
    fn at(&self, col: i32, row: i32) -> Vec2 {
        Vec2::new(
            self.cell.mul_add(col as f32, self.origin.x),
            self.cell.mul_add(row as f32, self.origin.y),
        )
    }
}

/// One frame.
///
/// `audio` is whatever the sound half wants said about itself, which is
/// nothing at all while it is working.
pub fn draw(
    font: &Tileset,
    view: &RenderView,
    layout: &Layout,
    camera: Coord,
    controls: &Controls,
    page: usize,
    audio: Option<&str>,
) {
    clear_background(VOID);
    if view.phase == Phase::Title {
        title(font, layout, controls, audio);
    } else {
        play(font, view, layout, camera, page, audio);
        if let Phase::GameOver { won } = view.phase {
            post_mortem(font, view, controls, won);
        }
    }
    draw_controls(font, controls);
}

/// Map, panels, and whatever the narrow layout owes the player on top.
fn play(
    font: &Tileset,
    view: &RenderView,
    layout: &Layout,
    camera: Coord,
    page: usize,
    audio: Option<&str>,
) {
    let map = Term::new(font, layout.map, layout.cell);
    map.grid(layout.cols, layout.rows, |col, row| {
        view.cell(camera.x + col, camera.y + row)
    });

    draw_rectangle(
        layout.info.x,
        layout.info.y,
        layout.info.w,
        layout.info.h,
        rgb(INFO_BACKGROUND),
    );
    hud::info(
        &Term::new(font, layout.info, layout.cell),
        view,
        layout.cols,
        layout.info_rows,
        audio,
    );

    if layout.mode == Mode::Narrow {
        hud::status_strip(&map, view, layout.cols);
    }
    if !layout.sidebar_overlay || view.mode == UiMode::Organelles {
        let alpha = if layout.sidebar_overlay {
            OVERLAY_ALPHA
        } else {
            1.0
        };
        draw_rectangle(
            layout.sidebar.x,
            layout.sidebar.y,
            layout.sidebar.w,
            layout.sidebar.h,
            rgba(palette::ORGANELLE_CONSOLE_BG, alpha),
        );
        hud::sidebar(
            &Term::new(font, layout.sidebar, layout.cell),
            view,
            layout.sidebar_rows,
            page,
        );
    }
}

/// The three difficulty buttons, in the order the number keys pick them.
#[must_use]
pub fn title_buttons(screen_w: f32, screen_h: f32) -> [Rect; 3] {
    let width = (screen_w * 0.55).clamp(180.0, 420.0);
    let height = (screen_h * 0.085).clamp(48.0, 72.0);
    let gap = height * 0.3;
    let top = screen_h * 0.42;
    let left = (screen_w - width) * 0.5;
    [0.0, 1.0, 2.0].map(|i| Rect::new(left, (height + gap).mul_add(i, top), width, height))
}

/// The post-mortem panel, and the restart button inside it.
#[must_use]
pub fn post_mortem_panel(screen_w: f32, screen_h: f32) -> (Rect, Rect) {
    let width = (screen_w * 0.8).min(680.0);
    let height = (screen_h * 0.6).min(460.0);
    let panel = Rect::new(
        (screen_w - width) * 0.5,
        (screen_h - height) * 0.5,
        width,
        height,
    );
    let button_w = (width * 0.5).clamp(160.0, 320.0);
    let button_h = (height * 0.16).clamp(48.0, 64.0);
    let button = Rect::new(
        (width - button_w).mul_add(0.5, panel.x),
        panel.bottom() - button_h - 16.0,
        button_w,
        button_h,
    );
    (panel, button)
}

/// The title screen: the name, the three difficulties, and the controls.
fn title(font: &Tileset, layout: &Layout, controls: &Controls, audio: Option<&str>) {
    const TAGLINE: &str = "a giant, constantly evolving amoeba";
    const HINTS: [&str; 2] = [
        "arrows move   space waits   X examines   Z organelles",
        "A and D cycle nuclei   Q and E page   F1 for help",
    ];

    let (screen_w, screen_h) = (controls.screen.x, controls.screen.y);
    // Everything is sized to fit the window rather than to the cell grid: a
    // phone in portrait has room for a big title and not much else.
    let margin = screen_w * 0.06;
    let inner = (screen_w - margin * 2.0).max(1.0);
    let big = fit_text("AMOEBA RL", inner, (layout.cell * 3.0).clamp(24.0, 72.0));
    let small = fit_text(TAGLINE, inner, layout.cell.clamp(10.0, 20.0));
    let centre = screen_w * 0.5;

    font.draw_text_centred(
        "AMOEBA RL",
        centre,
        screen_h * 0.14,
        big,
        rgb(palette::SLIME),
    );
    font.draw_text_centred(
        TAGLINE,
        centre,
        big.mul_add(1.4, screen_h * 0.14),
        small,
        rgb(palette::TEXT_BODY),
    );

    let labels = ["1  Normal", "2  Easy", "3  GJ"];
    for (button, label) in title_buttons(screen_w, screen_h).iter().zip(labels) {
        panel_frame(*button, palette::SLIME);
        let size = fit_text(label, button.w * 0.8, button.h * 0.5);
        font.draw_text_centred(
            label,
            button.center().x,
            (button.center().y - size * 0.5).floor(),
            size,
            rgb(palette::TEXT_HEADING),
        );
    }

    let hint = fit_text(HINTS[1], inner, small * 0.8);
    for (i, line) in HINTS.iter().enumerate() {
        font.draw_text_centred(
            line,
            centre,
            hint.mul_add(1.5 * i as f32, screen_h * 0.86),
            hint,
            rgb(palette::FLOOR_FOV),
        );
    }

    // The only screen where the audio nag can still be true: choosing a
    // difficulty is a press, and a press is what a browser is waiting for.
    if let Some(note) = audio {
        font.draw_text_centred(
            note,
            centre,
            hint.mul_add(3.0, screen_h * 0.86),
            hint,
            rgb(palette::SUPER_BRIGHT),
        );
    }
}

/// The win or loss screen. The sim already wrote the verdict into the message
/// log, so this is a frame around it plus a way back in.
fn post_mortem(font: &Tileset, view: &RenderView, controls: &Controls, won: bool) {
    let (panel, button) = post_mortem_panel(controls.screen.x, controls.screen.y);
    draw_rectangle(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        rgba(palette::FLOOR_BACKGROUND_FOV, OVERLAY_ALPHA),
    );
    let accent = if won {
        palette::SLIME
    } else {
        palette::ROOT_ORGANELLE
    };
    panel_frame(panel, accent);

    let size = (panel.h * 0.045).clamp(10.0, 20.0);
    font.draw_text_centred(
        if won { "YOU ESCAPE" } else { "YOU DIE" },
        panel.center().x,
        panel.y + size,
        size * 2.0,
        rgb(accent),
    );
    // The log wraps at 62 columns, so the widest line it can hand over decides
    // how small the verdict has to be drawn to stay inside the frame.
    let widest = view
        .messages
        .iter()
        .max_by_key(|line| line.chars().count())
        .map_or("", String::as_str);
    let body = fit_text(widest, panel.w - size * 2.0, size * 0.85);
    let top = size.mul_add(3.5, panel.y);
    for (i, line) in view.messages.iter().enumerate() {
        let baseline = (body * 1.35).mul_add(i as f32, top);
        if baseline + body > button.top() {
            break;
        }
        font.draw_text(
            line,
            panel.x + size,
            baseline,
            body,
            rgb(palette::TEXT_HEADING),
        );
    }

    panel_frame(button, accent);
    let label = fit_text("R  play again", button.w * 0.8, button.h * 0.4);
    font.draw_text_centred(
        "R  play again",
        button.center().x,
        (button.center().y - label * 0.5).floor(),
        label,
        rgb(palette::TEXT_HEADING),
    );
}

/// The on-screen pad, drawn only once a touch has arrived or the screen is
/// small enough that one is coming.
fn draw_controls(font: &Tileset, controls: &Controls) {
    if !controls.visible {
        return;
    }
    let pad = controls.dpad;
    let step = pad.w / 3.0;
    for (_, label, col, row) in Controls::DPAD_CELLS {
        let cell = Rect::new(
            step.mul_add(col as f32, pad.x),
            step.mul_add(row as f32, pad.y),
            step,
            step,
        );
        button(font, cell, label);
    }
    for (action, rect) in controls.buttons {
        button(font, rect, action.label());
    }
}

/// One translucent, labelled hit target.
fn button(font: &Tileset, rect: Rect, label: char) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        rgba(palette::FLOOR_BACKGROUND_FOV, 0.55),
    );
    draw_rectangle_lines(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        2.0,
        rgba(palette::SLIME, 0.65),
    );
    let size = (rect.h * 0.5).max(8.0);
    font.draw(
        label,
        (rect.center().x - size * 0.5).floor(),
        (rect.center().y - size * 0.5).floor(),
        size,
        rgba(palette::TEXT_HEADING, 0.9),
    );
}

/// The two-pixel border every framed thing on this screen shares.
fn panel_frame(rect: Rect, color: Rgb) {
    draw_rectangle(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        rgba(palette::FLOOR_BACKGROUND_FOV, 0.75),
    );
    draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, rgb(color));
}

/// Everything [`Action`] needs from this module: the glyph its button wears.
impl Action {
    /// The code page 437 glyph this button is labelled with.
    #[must_use]
    pub const fn label(self) -> char {
        match self {
            Self::Wait => '\u{2022}',
            Self::Examine => 'X',
            Self::Organelles => 'Z',
            Self::CyclePrev => 'A',
            Self::CycleNext => 'D',
            Self::Help => '?',
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window the original ran in, and the desktop default.
    const DESKTOP: (f32, f32) = (1032.0, 708.0);
    /// A phone, upright.
    const PHONE: (f32, f32) = (390.0, 844.0);

    fn fit(screen: (f32, f32), map: (i32, i32)) -> Layout {
        Layout::fit(screen.0, screen.1, map.0, map.1)
    }

    #[test]
    fn the_desktop_default_reproduces_the_original_console() {
        let layout = fit(DESKTOP, (64, 48));
        assert_eq!(layout.mode, Mode::Wide);
        // 86 x 59 cells at twelve pixels is exactly 1032 x 708.
        assert!((layout.cell - 12.0).abs() < f32::EPSILON);
        assert!((layout.map.w - 768.0).abs() < f32::EPSILON);
        assert!((layout.map.h - 576.0).abs() < f32::EPSILON);
        assert!((layout.sidebar.w - 264.0).abs() < f32::EPSILON);
        assert!(!layout.scrolls);
    }

    #[test]
    fn a_smaller_map_does_not_pay_for_columns_it_never_draws() {
        let wide = fit(DESKTOP, (64, 48));
        let narrow_map = fit(DESKTOP, (48, 48));
        assert_eq!(narrow_map.mode, Mode::Wide);
        assert_eq!(narrow_map.cols, 48);
        // Same window, same cell size, less of it used.
        assert!((narrow_map.cell - wide.cell).abs() < f32::EPSILON);
        assert!(narrow_map.map.w < wide.map.w);
    }

    #[test]
    fn cells_are_always_a_whole_number_of_pixels() {
        for width in [320.0_f32, 390.0, 800.0, 1032.0, 1440.0, 1920.0] {
            for height in [480.0_f32, 708.0, 844.0, 1080.0] {
                let layout = Layout::fit(width, height, 48, 48);
                assert!((layout.cell - layout.cell.floor()).abs() < f32::EPSILON);
                assert!(layout.cell >= 1.0);
            }
        }
    }

    #[test]
    fn a_phone_gets_the_map_first_layout_at_a_legible_cell_size() {
        let layout = fit(PHONE, (48, 48));
        assert_eq!(layout.mode, Mode::Narrow);
        assert!(layout.sidebar_overlay);
        assert!(
            layout.cell >= NARROW_MIN_CELL,
            "cells were {} px",
            layout.cell
        );
        // Ten pixels a cell does not fit 48 columns in 390 px, so it scrolls.
        assert!(layout.scrolls);
        assert!(layout.cols < 48);
        assert_eq!(layout.rows, 48);
    }

    #[test]
    fn a_phone_on_its_side_still_refuses_the_sidebar() {
        let layout = Layout::fit(PHONE.1, PHONE.0, 48, 48);
        assert_eq!(layout.mode, Mode::Narrow);
    }

    #[test]
    fn a_tablet_in_landscape_keeps_the_three_panels() {
        let layout = Layout::fit(1024.0, 768.0, 48, 48);
        assert_eq!(layout.mode, Mode::Wide);
        assert!(!layout.sidebar_overlay);
    }

    #[test]
    fn a_tablet_upright_does_not() {
        let layout = Layout::fit(768.0, 1024.0, 48, 48);
        assert_eq!(layout.mode, Mode::Narrow);
        // Everything fits at this width, so there is nothing to scroll.
        assert!(!layout.scrolls);
        assert_eq!(layout.cols, 48);
    }

    #[test]
    fn panels_tile_without_gaps_or_overlaps() {
        let layout = fit(DESKTOP, (64, 48));
        assert!((layout.map.bottom() - layout.info.top()).abs() < f32::EPSILON);
        assert!((layout.map.right() - layout.sidebar.left()).abs() < f32::EPSILON);
        assert!((layout.sidebar.h - (layout.map.h + layout.info.h)).abs() < f32::EPSILON);
        assert!((layout.info.w - layout.map.w).abs() < f32::EPSILON);
    }

    #[test]
    fn a_degenerate_window_still_produces_something_finite() {
        let layout = Layout::fit(0.0, 0.0, 48, 48);
        assert!(layout.cell.is_finite() && layout.cell > 0.0);
        assert!(layout.cols >= 1 && layout.rows >= 1);
    }

    #[test]
    fn the_camera_centres_the_focus_and_stops_at_the_edges() {
        // Middle of the map: centred.
        assert_eq!(
            camera_origin(Coord::new(24, 24), 20, 20, 48, 48),
            Coord::new(14, 14)
        );
        // Top-left corner: clamped to the origin, never negative.
        assert_eq!(
            camera_origin(Coord::new(0, 0), 20, 20, 48, 48),
            Coord::new(0, 0)
        );
        // Bottom-right corner: clamped so the last column is the last column.
        assert_eq!(
            camera_origin(Coord::new(47, 47), 20, 20, 48, 48),
            Coord::new(28, 28)
        );
    }

    #[test]
    fn a_viewport_larger_than_the_map_never_scrolls_off_it() {
        assert_eq!(
            camera_origin(Coord::new(30, 30), 64, 64, 48, 48),
            Coord::new(0, 0)
        );
    }

    #[test]
    fn map_taps_land_on_the_cell_under_them() {
        let layout = fit(DESKTOP, (64, 48));
        let camera = Coord::new(0, 0);
        let origin = layout.map.point();
        assert_eq!(
            layout.cell_at(origin + Vec2::new(6.0, 6.0), camera),
            Some(Coord::new(0, 0))
        );
        assert_eq!(
            layout.cell_at(origin + Vec2::new(30.0, 42.0), camera),
            Some(Coord::new(2, 3))
        );
        // The camera offset is added, so a scrolled map still reports truly.
        assert_eq!(
            layout.cell_at(origin + Vec2::new(6.0, 6.0), Coord::new(10, 7)),
            Some(Coord::new(10, 7))
        );
    }

    #[test]
    fn taps_outside_the_map_hit_nothing() {
        let layout = fit(DESKTOP, (64, 48));
        assert_eq!(
            layout.cell_at(Vec2::new(-5.0, -5.0), Coord::new(0, 0)),
            None
        );
        assert_eq!(layout.cell_at(layout.info.center(), Coord::new(0, 0)), None);
    }

    #[test]
    fn the_title_buttons_stack_without_touching() {
        let buttons = title_buttons(DESKTOP.0, DESKTOP.1);
        for pair in buttons.windows(2) {
            assert!(pair[0].bottom() < pair[1].top());
        }
        for button in buttons {
            assert!(button.h >= 48.0, "hit targets stay thumb-sized");
        }
    }

    #[test]
    fn text_shrinks_to_fit_and_no_further() {
        // A short string on a wide screen keeps the size it asked for.
        assert!((fit_text("R", 400.0, 24.0) - 24.0).abs() < f32::EPSILON);
        // A long one gives up width until it fits.
        let size = fit_text(&"x".repeat(50), 400.0, 24.0);
        assert!(size * 50.0 <= 400.0, "{size} px a character was too wide");
        // Degenerate inputs still produce something drawable.
        assert!(fit_text("", 0.0, 24.0) >= 1.0);
    }

    #[test]
    fn the_title_screen_fits_the_narrowest_phone() {
        // Nothing here can be checked by eye in CI, so check the arithmetic:
        // the widest line the title draws has to fit the smallest screen.
        let screen_w = 320.0_f32;
        let inner = screen_w * 0.88;
        let hint = "A and D cycle nuclei   Q and E page   F1 for help";
        let size = fit_text(hint, inner, 20.0);
        assert!(size * hint.chars().count() as f32 <= inner);
    }

    #[test]
    fn the_restart_button_sits_inside_its_panel() {
        let (panel, button) = post_mortem_panel(DESKTOP.0, DESKTOP.1);
        assert!(panel.contains(button.point()));
        assert!(panel.contains(Vec2::new(button.right(), button.bottom())));
        assert!(button.h >= 48.0);
    }
}
