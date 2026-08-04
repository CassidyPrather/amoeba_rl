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

use crate::anim::Anim;
use crate::hud;
use crate::input::{Action, Controls, MIN_TARGET};
use crate::links::{GAP, LINKS, row_width_chars};
use crate::settings::{ROWS, Settings};
use crate::tileset::Tileset;

/// Cells the organelle sidebar is wide, as the original's player console was.
pub const SIDEBAR_COLS: i32 = 22;

/// Cells the message bar is tall in the three-panel layout: nine log lines, a
/// margin, and a row of key hints.
pub const INFO_ROWS: i32 = 11;

/// Cells the message bar keeps once it has collapsed: four log lines and the
/// hints.
///
/// Also the height it keeps when the player has turned the log off: the panel
/// still owes examine mode somewhere to describe what the cursor is over, and
/// that is worth more rows than the key hints alone need. What the wide layout
/// gets out of hiding the log is the other five rows, which go to the map.
pub const NARROW_INFO_ROWS: i32 = 6;

/// Cells the map-first layout gives the numbers the sidebar is not there to
/// show. Its own row rather than a translucent strip over the map: a cell of
/// the map is a thing you can be standing on, and a line of text over the top
/// of one is a line of text over the top of a militia.
const STATUS_ROWS: i32 = 1;

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
    /// The one-line strip of numbers the map-first layout owes the player,
    /// there being no sidebar on screen to read them off. `None` wherever
    /// there is one.
    pub status: Option<Rect>,
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
    /// Whether this is a screen with thumbs on it and no keyboard behind it.
    /// Everything that would otherwise name a key reads this and names a
    /// button instead.
    pub touch: bool,
    /// Rows at the foot of the sidebar that belong to the paging control
    /// rather than to organelles.
    footer_rows: i32,
}

impl Layout {
    /// Fit the game to a window.
    ///
    /// `map_w` and `map_h` are the sim's map size, which the difficulty picks:
    /// 48×48 normally and 64×48 on GJ. Laying out from those rather than from
    /// the original's fixed 64 means the smaller maps do not pay for columns
    /// they never draw.
    ///
    /// `log` is the player's choice about the message bar. Hiding it is a
    /// layout decision rather than a drawing one — the rows it gives up go to
    /// the map — so it belongs here and not in [`hud`].
    ///
    /// `area` is the window minus whatever the on-screen pad has claimed (see
    /// [`Controls::free`]), so the panels and the pad cannot land on top of
    /// each other. `touch` says whether that pad is there at all, which is the
    /// one thing everything that would otherwise name a key needs to know.
    #[must_use]
    pub fn fit(area: Rect, map_w: i32, map_h: i32, log: bool, touch: bool) -> Self {
        // A minimised window or a hidden tab reports zero; the floor keeps
        // every division below finite.
        let area = Rect::new(area.x, area.y, area.w.max(1.0), area.h.max(1.0));
        let map_w = map_w.max(1);
        let map_h = map_h.max(1);
        let info_rows = if log { INFO_ROWS } else { NARROW_INFO_ROWS };
        let wide_cell =
            snap((area.w / (map_w + SIDEBAR_COLS) as f32).min(area.h / (map_h + info_rows) as f32));
        if area.w / area.h >= WIDE_MIN_ASPECT && wide_cell >= WIDE_MIN_CELL {
            Self::wide(area, map_w, map_h, wide_cell, info_rows, touch)
        } else {
            Self::narrow(area, map_w, map_h, touch)
        }
    }

    /// The original's three panels, centred in whatever is left over.
    fn wide(area: Rect, map_w: i32, map_h: i32, cell: f32, info_rows: i32, touch: bool) -> Self {
        let map_px = Vec2::new(map_w as f32 * cell, map_h as f32 * cell);
        let info_h = info_rows as f32 * cell;
        let total = Vec2::new(
            (SIDEBAR_COLS as f32).mul_add(cell, map_px.x),
            map_px.y + info_h,
        );
        let origin = area.point() + letterbox(area.w, area.h, total);
        Self {
            mode: Mode::Wide,
            cell,
            map: Rect::new(origin.x, origin.y, map_px.x, map_px.y),
            cols: map_w,
            rows: map_h,
            scrolls: false,
            status: None,
            info: Rect::new(origin.x, origin.y + map_px.y, map_px.x, info_h),
            info_rows,
            sidebar: Rect::new(
                origin.x + map_px.x,
                origin.y,
                SIDEBAR_COLS as f32 * cell,
                total.y,
            ),
            sidebar_rows: map_h + info_rows,
            sidebar_overlay: false,
            touch,
            footer_rows: footer_rows(cell, touch),
        }
    }

    /// Map first. The sidebar keeps its 22 columns but moves on top of the
    /// map, where the organelle browser can call it up and dismiss it again.
    fn narrow(area: Rect, map_w: i32, map_h: i32, touch: bool) -> Self {
        let chrome = NARROW_INFO_ROWS + STATUS_ROWS;
        let full = (area.w / map_w as f32).min(area.h / (map_h + chrome) as f32);
        let cell = snap(full.max(NARROW_MIN_CELL));
        let cols = map_w.min(floor_div(area.w, cell).max(1));
        let rows = map_h.min((floor_div(area.h, cell) - chrome).max(1));
        let map_px = Vec2::new(cols as f32 * cell, rows as f32 * cell);
        let (status_h, info_h) = (STATUS_ROWS as f32 * cell, NARROW_INFO_ROWS as f32 * cell);
        let total = Vec2::new(map_px.x, status_h + map_px.y + info_h);
        let origin = area.point() + letterbox(area.w, area.h, total);
        let map_y = origin.y + status_h;
        let sidebar_cols = SIDEBAR_COLS.min(cols);
        Self {
            mode: Mode::Narrow,
            cell,
            map: Rect::new(origin.x, map_y, map_px.x, map_px.y),
            cols,
            rows,
            scrolls: cols < map_w || rows < map_h,
            status: Some(Rect::new(origin.x, origin.y, map_px.x, status_h)),
            info: Rect::new(origin.x, map_y + map_px.y, map_px.x, info_h),
            info_rows: NARROW_INFO_ROWS,
            // Down as far as the map and no further. The overlay covers the
            // strip, whose numbers it repeats in its own header, but it stops
            // at the message bar — which while the browser is open is where
            // the description of the selected organelle is being written, and
            // that is the half of the browser with the words in it.
            sidebar: Rect::new(
                (sidebar_cols as f32).mul_add(-cell, origin.x + map_px.x),
                origin.y,
                sidebar_cols as f32 * cell,
                status_h + map_px.y,
            ),
            sidebar_rows: rows + STATUS_ROWS,
            sidebar_overlay: true,
            touch,
            footer_rows: footer_rows(cell, touch),
        }
    }

    /// The organelle list's paging control, on a screen with no `Q` and `E` to
    /// press: previous page on the left, next page on the right, along the
    /// foot of the sidebar.
    ///
    /// One rectangle serves the drawing and the hit testing, so a button
    /// cannot end up somewhere other than where it is listening.
    #[must_use]
    pub fn sidebar_pager(&self) -> Option<hud::Pager> {
        if !self.touch {
            return None;
        }
        let height = self.footer_rows as f32 * self.cell;
        let top = self.sidebar.bottom() - height;
        let half = self.sidebar.w * 0.5;
        Some(hud::Pager {
            rows: self.footer_rows,
            buttons: [
                Rect::new(self.sidebar.x, top, half, height),
                Rect::new(self.sidebar.x + half, top, half, height),
            ],
        })
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

/// Rows the sidebar owes its paging control.
///
/// One, which is what the key hints have always taken, unless a thumb is going
/// to have to hit it — in which case as many rows as it takes to be worth
/// aiming at.
fn footer_rows(cell: f32, touch: bool) -> i32 {
    if touch {
        (MIN_TARGET / cell.max(f32::EPSILON)).ceil() as i32
    } else {
        1
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
    fit_chars(text.chars().count(), width, max)
}

/// The same, for a row whose width is known before its text is: the two link
/// labels and the gap between them are laid out as one line, then drawn as
/// three separate pieces.
#[must_use]
pub fn fit_chars(chars: usize, width: f32, max: f32) -> f32 {
    (width / chars.max(1) as f32).min(max).max(1.0)
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

    /// The atlas behind this painter, for the few things inside a panel that
    /// are sized in pixels rather than in cells — a button a thumb has to hit
    /// cannot be one cell tall.
    #[must_use]
    pub const fn font(&self) -> &Tileset {
        self.font
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

/// Everything one frame needs that is not the world: the player's choices, the
/// overlays mid-flight, and whatever the two other halves of the frontend want
/// said about themselves.
pub struct Frontend<'a> {
    /// The on-screen pad and the window it is measured against.
    pub controls: &'a Controls,
    /// Which page of the sidebar to draw.
    pub page: usize,
    /// What the sound half wants said, which is nothing while it is working.
    pub audio: Option<&'a str>,
    /// Whether sound is on, for the settings panel to report.
    pub sound_on: bool,
    /// The player's choices.
    pub settings: &'a Settings,
    /// The turn currently showing itself.
    pub anim: &'a Anim,
}

/// One frame.
pub fn draw(font: &Tileset, view: &RenderView, layout: &Layout, camera: Coord, ui: &Frontend) {
    clear_background(VOID);
    if view.phase == Phase::Title {
        title(font, layout, ui.controls, ui.audio);
    } else {
        play(font, view, layout, camera, ui);
        if let Phase::GameOver { won } = view.phase {
            post_mortem(font, view, ui.controls, won, layout.touch);
        }
    }
    draw_controls(font, ui.controls);
    if ui.settings.is_open() {
        settings_menu(font, ui, layout.touch);
    }
}

/// Map, panels, and whatever the narrow layout owes the player on top.
fn play(font: &Tileset, view: &RenderView, layout: &Layout, camera: Coord, ui: &Frontend) {
    let (page, audio) = (ui.page, ui.audio);
    let map = Term::new(font, layout.map, layout.cell);
    map.grid(layout.cols, layout.rows, |col, row| {
        view.cell(camera.x + col, camera.y + row)
    });
    // Over the world and under the panels: an overlay is a note about the map,
    // so it may cover a cell but never a number.
    ui.anim.draw(
        font,
        layout.map,
        layout.cell,
        camera,
        layout.cols,
        layout.rows,
    );

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
        ui.settings.log,
        layout.touch,
    );

    if let Some(strip) = layout.status {
        hud::status_strip(&Term::new(font, strip, layout.cell), view, layout.cols);
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
            layout.sidebar_pager(),
        );
    }
}

/// Where the difficulty buttons start, as a fraction of the window height.
/// The title above them is sized to stop short of it.
const TITLE_BUTTONS_TOP: f32 = 0.42;

/// The name, in two lines: too long to draw as one and still be a title.
const NAME: &str = "AMOEBA ROGUELIKE";
const REMASTER: &str = "REMASTERED";
const TAGLINE: &str = "a giant, constantly evolving amoeba";

/// The title screen's text is laid out in pixels rather than in cells, inside
/// a margin of its own: a phone in portrait has room for a big title and not
/// much else.
fn title_inner(screen_w: f32) -> f32 {
    (screen_w * 0.88).max(1.0)
}

/// The glyph size and baseline of each line of the title block.
///
/// Split out of [`title`] because three stacked lines can grow into the
/// difficulty buttons on a short window, and that is arithmetic a test can
/// check where a title screen is not.
struct TitleBlock {
    name: f32,
    remaster: f32,
    name_y: f32,
    remaster_y: f32,
    tagline_y: f32,
}

impl TitleBlock {
    /// Where the name starts, as a fraction of the window height.
    const TOP: f32 = 0.14;
    /// `REMASTERED` relative to the name above it.
    const REMASTER_SCALE: f32 = 0.55;
    /// Each baseline relative to the glyph size of the line before it.
    const NAME_LEADING: f32 = 1.15;
    const REMASTER_LEADING: f32 = 1.5;

    fn fit(screen_w: f32, screen_h: f32, cell: f32) -> Self {
        let inner = title_inner(screen_w);
        let name_y = screen_h * Self::TOP;
        // The name is capped by the height it has to play with as well as by
        // the width: a phone in landscape runs out of the first long before
        // the second. The divisor is the block's height in multiples of the
        // name's glyph size, with a whole one allowed for the tagline — which
        // is clamped to 20px and so is never actually that tall.
        let stack = Self::REMASTER_SCALE.mul_add(Self::REMASTER_LEADING, Self::NAME_LEADING) + 1.0;
        let room = (screen_h.mul_add(TITLE_BUTTONS_TOP, -name_y) / stack).max(1.0);
        let name = fit_text(NAME, inner, (cell * 3.0).clamp(24.0, 72.0).min(room));
        // No width check on `REMASTERED`: it is shorter than the line above it
        // and drawn smaller, so whatever fits that fits this with room spare.
        let remaster = name * Self::REMASTER_SCALE;
        let remaster_y = name.mul_add(Self::NAME_LEADING, name_y);
        Self {
            name,
            remaster,
            name_y,
            remaster_y,
            tagline_y: remaster.mul_add(Self::REMASTER_LEADING, remaster_y),
        }
    }
}

/// One row of links: where it is centred, where its glyphs start, and how big
/// they are drawn.
///
/// The title and post-mortem screens each own where their row goes; from
/// there on both of them draw it and hit-test it through this.
#[derive(Clone, Copy)]
pub struct LinkRow {
    centre: f32,
    /// The top of the glyph row, which is what [`Tileset::draw_text`] takes.
    top: f32,
    size: f32,
}

impl LinkRow {
    /// The title screen's row: below the two lines of key hints, in the same
    /// rhythm and at the same size.
    ///
    /// The audio nag goes below *this* rather than above it. The nag comes and
    /// goes with the first press, and a row that is always on screen should
    /// not move about when a transient one turns up over it.
    #[must_use]
    pub fn title(screen_w: f32, screen_h: f32, cell: f32, touch: bool) -> Self {
        let size = title_hint_size(screen_w, cell, touch);
        Self {
            centre: screen_w * 0.5,
            top: size.mul_add(3.0, screen_h * 0.86),
            size,
        }
    }

    /// The post-mortem screen's row: under the panel rather than inside it.
    /// The panel is already rationing rows between the verdict and the log,
    /// and the map behind it is not using them.
    #[must_use]
    pub fn post_mortem(screen_w: f32, screen_h: f32) -> Self {
        let (panel, _) = post_mortem_panel(screen_w, screen_h);
        let size = fit_chars(
            row_width_chars(),
            panel.w,
            (panel.h * 0.04).clamp(9.0, 15.0),
        );
        Self {
            centre: panel.center().x,
            top: panel.bottom() + size * 2.0,
            size,
        }
    }

    /// One rect per link, in [`LINKS`] order.
    ///
    /// Padded out around the glyphs to something a thumb can hit: at the size
    /// these are drawn at, the text alone would be a nine-pixel-tall target.
    #[must_use]
    pub fn rects(self) -> [Rect; 2] {
        let mut x = (self.size * row_width_chars() as f32).mul_add(-0.5, self.centre);
        let pad = self.size * 0.5;
        LINKS.map(|link| {
            let w = self.size * link.label().chars().count() as f32;
            let rect = Rect::new(
                x - pad * 0.5,
                self.top - pad,
                w + pad,
                self.size + pad * 2.0,
            );
            x += self.size.mul_add(GAP.chars().count() as f32, w);
            rect
        })
    }

    /// Draw the row.
    ///
    /// Cyan because nothing else in the interface is: the chrome around them
    /// is the muted grey of the key hints, and a link wants to look like the
    /// one thing on the screen that goes somewhere else.
    fn draw(self, font: &Tileset) {
        for (link, rect) in LINKS.iter().zip(self.rects()) {
            font.draw_text(
                link.label(),
                self.size.mul_add(0.25, rect.x),
                self.top,
                self.size,
                rgb(palette::OVERFILL),
            );
        }
    }
}

/// The size the title screen's key hints — and so its links — are drawn at.
///
/// The bottom strip is laid out in rows of this, so it is worked out once here
/// and read by both [`title`] and the input code, which has to find the links
/// again without redrawing them.
fn title_hint_size(screen_w: f32, cell: f32, touch: bool) -> f32 {
    let inner = title_inner(screen_w);
    let hints = if touch {
        TOUCH_TITLE_HINTS
    } else {
        TITLE_HINTS
    };
    let small = fit_text(TAGLINE, inner, cell.clamp(10.0, 20.0));
    fit_text(hints[1], inner, small * 0.8)
}

/// The three difficulty buttons, in the order the number keys pick them.
#[must_use]
pub fn title_buttons(screen_w: f32, screen_h: f32) -> [Rect; 3] {
    let width = (screen_w * 0.55).clamp(180.0, 420.0);
    let height = (screen_h * 0.085).clamp(48.0, 72.0);
    let gap = height * 0.3;
    let top = screen_h * TITLE_BUTTONS_TOP;
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

/// Every rectangle the settings panel is made of.
///
/// Pure geometry, so hit testing in [`input`](crate::input) and drawing in here
/// agree by construction rather than by two people keeping two sets of numbers
/// the same.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SettingsPanel {
    /// The frame around the whole thing.
    pub panel: Rect,
    /// One hit target per row of [`ROWS`], top to bottom.
    pub rows: [Rect; ROWS.len()],
    /// The way out.
    pub close: Rect,
}

/// Lay the settings panel out in a window.
///
/// Sized from its contents rather than as a fraction of the screen: four rows
/// no smaller than a thumb, a heading over them, and a line of help and a way
/// out under them. A phone on its side is barely taller than that, so a panel
/// laid out the other way round — a share of the screen, rows dividing what is
/// left — runs its last row into its own close button.
#[must_use]
pub fn settings_panel(screen_w: f32, screen_h: f32) -> SettingsPanel {
    let width = (screen_w * 0.86).min(520.0);
    let pad = (width * 0.06).min(24.0);
    let row_h = (screen_h * 0.09).clamp(MIN_TARGET, 60.0);
    let close_h = (screen_h * 0.11).clamp(MIN_TARGET, 60.0);
    let heading = (screen_h * 0.12).clamp(36.0, 72.0);
    let help = (screen_h * 0.06).clamp(18.0, 34.0);
    let rows = row_h * ROWS.len() as f32;
    let height = (heading + rows + help + close_h + pad * 2.0).min(screen_h);
    let panel = Rect::new(
        ((screen_w - width) * 0.5).max(0.0),
        ((screen_h - height) * 0.5).max(0.0),
        width,
        height,
    );
    let first = panel.y + heading;
    SettingsPanel {
        panel,
        rows: std::array::from_fn(|i| {
            Rect::new(
                panel.x + pad,
                row_h.mul_add(i as f32, first),
                width - pad * 2.0,
                row_h,
            )
        }),
        close: Rect::new(
            (width - close_h * 3.0).mul_add(0.5, panel.x),
            // Along the bottom, unless the window is too short for that to
            // still be under the rows.
            (panel.bottom() - close_h - pad).max(first + rows + help),
            close_h * 3.0,
            close_h,
        ),
    }
}

/// The settings panel: one row per choice, the selection marked, and a line
/// about whatever the selection is on.
fn settings_menu(font: &Tileset, ui: &Frontend, touch: bool) {
    let (screen_w, screen_h) = (ui.controls.screen.x, ui.controls.screen.y);
    let geometry = settings_panel(screen_w, screen_h);
    let panel = geometry.panel;
    draw_rectangle(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        rgba(palette::ORGANELLE_CONSOLE_BG, OVERLAY_ALPHA),
    );
    panel_frame(panel, palette::SLIME);

    let heading = (panel.h * 0.06).clamp(12.0, 22.0);
    font.draw_text_centred(
        "SETTINGS",
        panel.center().x,
        panel.y + heading,
        heading * 1.6,
        rgb(palette::SLIME),
    );

    let selected = ui.settings.selected_index();
    // One size for the whole column, and it is the size at which the *widest*
    // row still has a gap between its label and its value. Sizing each row to
    // its own text would make the longest label the smallest writing on the
    // panel, and would let "Message log" run into "shown".
    let widest = ROWS
        .iter()
        .map(|row| {
            row.label().chars().count() + ui.settings.value(*row, ui.sound_on).chars().count() + 4
        })
        .max()
        .unwrap_or(1);
    let size = geometry.rows.first().map_or(1.0, |rect| {
        (rect.w / widest as f32).min(rect.h * 0.42).max(1.0)
    });
    for (i, (row, rect)) in ROWS.iter().zip(geometry.rows).enumerate() {
        let on = i == selected;
        if on {
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                rgba(palette::BODY_SLIME, 0.35),
            );
        }
        let value = ui.settings.value(*row, ui.sound_on);
        let baseline = (rect.center().y - size * 0.5).floor();
        font.draw_text(
            row.label(),
            rect.x + size,
            baseline,
            size,
            rgb(if on {
                palette::TEXT_HEADING
            } else {
                palette::TEXT_BODY
            }),
        );
        // The value is right-aligned, so the column of them reads as a column.
        let right = size * value.chars().count() as f32;
        font.draw_text(
            value,
            rect.right() - right - size,
            baseline,
            size,
            rgb(palette::SUPER_BRIGHT),
        );
    }

    // A line about the selection, centred in whatever gap the rows left above
    // the way out — measured rather than guessed, because the rows grow to
    // stay thumb-sized on a small screen and can leave very little of it.
    let help = ui.settings.selected().help();
    let note = fit_text(help, panel.w - 24.0, (panel.h * 0.04).clamp(9.0, 15.0));
    let rows_end = geometry.rows.last().map_or(panel.y, Rect::bottom);
    font.draw_text_centred(
        help,
        panel.center().x,
        note.mul_add(-0.5, (rows_end + geometry.close.y) * 0.5)
            .floor(),
        note,
        rgb(palette::FLOOR_FOV),
    );

    panel_frame(geometry.close, palette::SLIME);
    let label = if touch { "close" } else { "S  close" };
    let size = fit_text(label, geometry.close.w * 0.8, geometry.close.h * 0.4);
    font.draw_text_centred(
        label,
        geometry.close.center().x,
        (geometry.close.center().y - size * 0.5).floor(),
        size,
        rgb(palette::TEXT_HEADING),
    );
}

/// What the title screen says about the controls. Hoisted so the test that
/// checks it fits the narrowest phone is checking the real thing.
const TITLE_HINTS: [&str; 2] = [
    "arrows move   space waits   X examines   Z organelles",
    "A and D cycle nuclei   S settings   F1 for help",
];

/// The same, for a screen with no keys to name: the pad and the buttons are
/// waiting on the other side of this screen, so all it has to promise is that
/// they are there.
const TOUCH_TITLE_HINTS: [&str; 2] = [
    "the pad moves you   the map takes taps",
    "the buttons wait, look, list and cycle",
];

/// The title screen: the name, the three difficulties, and the controls.
fn title(font: &Tileset, layout: &Layout, controls: &Controls, audio: Option<&str>) {
    let hints = if layout.touch {
        TOUCH_TITLE_HINTS
    } else {
        TITLE_HINTS
    };

    let (screen_w, screen_h) = (controls.screen.x, controls.screen.y);
    let block = TitleBlock::fit(screen_w, screen_h, layout.cell);
    let inner = title_inner(screen_w);
    let small = fit_text(TAGLINE, inner, layout.cell.clamp(10.0, 20.0));
    let centre = screen_w * 0.5;

    font.draw_text_centred(NAME, centre, block.name_y, block.name, rgb(palette::SLIME));
    font.draw_text_centred(
        REMASTER,
        centre,
        block.remaster_y,
        block.remaster,
        rgb(palette::TEXT_HEADING),
    );
    font.draw_text_centred(
        TAGLINE,
        centre,
        block.tagline_y,
        small,
        rgb(palette::TEXT_BODY),
    );

    // The digits are how a keyboard picks; a thumb picks by hitting the thing
    // it wants, and a "1" in front of it is only in the way.
    let labels = if layout.touch {
        ["Normal", "Easy", "GJ"]
    } else {
        ["1  Normal", "2  Easy", "3  GJ"]
    };
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

    let hint = title_hint_size(screen_w, layout.cell, layout.touch);
    for (i, line) in hints.iter().enumerate() {
        font.draw_text_centred(
            line,
            centre,
            hint.mul_add(1.5 * i as f32, screen_h * 0.86),
            hint,
            rgb(palette::FLOOR_FOV),
        );
    }

    LinkRow::title(screen_w, screen_h, layout.cell, layout.touch).draw(font);

    // The only screen where the audio nag can still be true: choosing a
    // difficulty is a press, and a press is what a browser is waiting for.
    if let Some(note) = audio {
        font.draw_text_centred(
            note,
            centre,
            hint.mul_add(4.5, screen_h * 0.86),
            hint,
            rgb(palette::SUPER_BRIGHT),
        );
    }
}

/// The win or loss screen. The sim already wrote the verdict into the message
/// log, so this is a frame around it plus a way back in.
fn post_mortem(font: &Tileset, view: &RenderView, controls: &Controls, won: bool, touch: bool) {
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
    let label = if touch { "play again" } else { "R  play again" };
    let size = fit_text(label, button.w * 0.8, button.h * 0.4);
    font.draw_text_centred(
        label,
        button.center().x,
        (button.center().y - size * 0.5).floor(),
        size,
        rgb(palette::TEXT_HEADING),
    );

    // Outside the frame, where the run has just ended and nothing else is
    // competing for the room.
    LinkRow::post_mortem(controls.screen.x, controls.screen.y).draw(font);
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
        // An arrow means the same thing to everyone, so the pad is drawn a
        // glyph high; the rest wear words, which have to be smaller.
        button(font, cell, label, 0.5);
    }
    for (action, rect) in controls.buttons {
        button(font, rect, action.name(), 0.28);
    }
    button(font, controls.settings, Action::Settings.name(), 0.28);
}

/// One translucent, labelled hit target. `scale` is the label's height as a
/// share of the button's.
fn button(font: &Tileset, rect: Rect, label: &str, scale: f32) {
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
    let size = fit_text(label, rect.w * 0.8, (rect.h * scale).max(8.0));
    font.draw_text_centred(
        label,
        rect.center().x,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The window the original ran in, and the desktop default.
    const DESKTOP: (f32, f32) = (1032.0, 708.0);
    /// A phone, upright.
    const PHONE: (f32, f32) = (390.0, 844.0);

    /// A whole window as an area, for the layouts with no pad in the way.
    fn window(screen: (f32, f32)) -> Rect {
        Rect::new(0.0, 0.0, screen.0, screen.1)
    }

    /// The layout with the log where it has always been and a keyboard behind
    /// it, which is the arrangement every test predating the pad assumes.
    fn fit(screen: (f32, f32), map: (i32, i32)) -> Layout {
        Layout::fit(window(screen), map.0, map.1, true, false)
    }

    /// The same window once the on-screen pad has taken its share.
    fn fit_touch(screen: (f32, f32), map: (i32, i32)) -> Layout {
        Layout::fit(Controls::free(screen.0, screen.1), map.0, map.1, true, true)
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
                let layout = Layout::fit(window((width, height)), 48, 48, true, false);
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
        let layout = fit((PHONE.1, PHONE.0), (48, 48));
        assert_eq!(layout.mode, Mode::Narrow);
    }

    #[test]
    fn a_tablet_in_landscape_keeps_the_three_panels() {
        let layout = fit((1024.0, 768.0), (48, 48));
        assert_eq!(layout.mode, Mode::Wide);
        assert!(!layout.sidebar_overlay);
    }

    #[test]
    fn a_tablet_upright_does_not() {
        let layout = fit((768.0, 1024.0), (48, 48));
        assert_eq!(layout.mode, Mode::Narrow);
        // Everything fits at this width, so there is nothing to scroll.
        assert!(!layout.scrolls);
        assert_eq!(layout.cols, 48);
    }

    #[test]
    fn panels_tile_without_gaps_or_overlaps() {
        let layout = fit(DESKTOP, (64, 48));
        assert!(layout.status.is_none(), "the sidebar is showing them");
        assert!((layout.map.bottom() - layout.info.top()).abs() < f32::EPSILON);
        assert!((layout.map.right() - layout.sidebar.left()).abs() < f32::EPSILON);
        assert!((layout.sidebar.h - (layout.map.h + layout.info.h)).abs() < f32::EPSILON);
        assert!((layout.info.w - layout.map.w).abs() < f32::EPSILON);
    }

    #[test]
    fn the_map_first_layout_stacks_the_strip_the_map_and_the_log() {
        // The strip used to be painted over the map's top row, which meant a
        // row of the world you could stand in and not see.
        for screen in [PHONE, (PHONE.1, PHONE.0), (320.0, 568.0)] {
            for layout in [fit(screen, (48, 48)), fit_touch(screen, (48, 48))] {
                let strip = layout.status.expect("the map-first layout has one");
                assert!((strip.h - layout.cell).abs() < f32::EPSILON);
                assert!((strip.bottom() - layout.map.top()).abs() < f32::EPSILON);
                assert!((layout.map.bottom() - layout.info.top()).abs() < f32::EPSILON);
                assert!((strip.w - layout.map.w).abs() < f32::EPSILON);
                // The overlay covers the strip and the map, and stops at the
                // message bar so the description it is writing there is not
                // written underneath itself.
                assert!(layout.sidebar.top() <= strip.top() + f32::EPSILON);
                assert!(
                    layout.sidebar.bottom() <= layout.info.top() + f32::EPSILON,
                    "the sidebar covered its own description"
                );
            }
        }
    }

    #[test]
    fn a_degenerate_window_still_produces_something_finite() {
        let layout = fit((0.0, 0.0), (48, 48));
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
        let inner = title_inner(320.0);
        for hint in TITLE_HINTS.iter().chain([&NAME, &REMASTER, &TAGLINE]) {
            let size = fit_text(hint, inner, 20.0);
            assert!(size * hint.chars().count() as f32 <= inner, "{hint:?}");
        }
    }

    #[test]
    fn the_title_block_stacks_downwards_and_stops_above_the_buttons() {
        // A landscape phone is the tight one: plenty of width for the name and
        // barely any height to stack three lines of it in.
        for screen in [DESKTOP, PHONE, (844.0, 390.0), (320.0, 480.0)] {
            let (w, h) = screen;
            let block = TitleBlock::fit(w, h, fit(screen, (64, 48)).cell);
            assert!(block.name_y < block.remaster_y, "{screen:?}");
            assert!(block.remaster_y < block.tagline_y, "{screen:?}");
            assert!(
                block.remaster < block.name,
                "{screen:?} loses the hierarchy"
            );
            // The tagline is clamped to 20px, so that is the deepest its
            // baseline can sit below the line it hangs off.
            let bottom = block.tagline_y + 20.0;
            let buttons = title_buttons(w, h);
            assert!(
                bottom <= buttons[0].top(),
                "{screen:?}: title runs into the buttons"
            );
            // The name still has to fit across, whatever the height allowed.
            assert!(
                block.name * NAME.chars().count() as f32 <= title_inner(w),
                "{screen:?}"
            );
        }
    }

    /// Every window shape the layout tests care about, plus the two that make
    /// the bottom strip tight: a landscape phone and a small square screen.
    const SHAPES: [(f32, f32); 5] = [
        DESKTOP,
        PHONE,
        (844.0, 390.0),
        (320.0, 480.0),
        (1920.0, 1080.0),
    ];

    #[test]
    fn the_title_links_take_a_row_of_their_own_in_the_bottom_strip() {
        for screen in SHAPES {
            let (w, h) = screen;
            for touch in [false, true] {
                let cell = fit(screen, (64, 48)).cell;
                let hint = title_hint_size(w, cell, touch);
                let row = LinkRow::title(w, h, cell, touch);
                let where_ = format!("{screen:?} touch={touch}");
                // Four rows share the strip, each a glyph tall: two lines of
                // key hints, the links, then the audio nag. What is checked
                // here is the drawn text, not the rects — those are padded out
                // past the glyphs on purpose, and the strip has nothing else
                // in it for that padding to steal a press from.
                let hint_bottom = hint.mul_add(1.5, h * 0.86) + hint;
                assert!(hint_bottom <= row.top, "{where_}: links overwrite a hint");
                let nag_top = hint.mul_add(4.5, h * 0.86);
                assert!(
                    row.top + row.size <= nag_top,
                    "{where_}: links overwrite the audio nag"
                );
                assert!(nag_top + hint <= h, "{where_}: the strip runs off the foot");
            }
        }
    }

    #[test]
    fn the_title_links_cannot_swallow_a_press_meant_for_a_difficulty() {
        // The padding that makes the links thumb-sized is only free because
        // nothing else down there is listening. The buttons are.
        for screen in SHAPES {
            let (w, h) = screen;
            for touch in [false, true] {
                let rects = LinkRow::title(w, h, fit(screen, (64, 48)).cell, touch).rects();
                for button in title_buttons(w, h) {
                    for rect in rects {
                        assert!(button.bottom() <= rect.y, "{screen:?} touch={touch}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_post_mortem_links_sit_below_the_panel_and_stay_on_screen() {
        for screen in SHAPES {
            let (w, h) = screen;
            let (panel, _) = post_mortem_panel(w, h);
            let rects = LinkRow::post_mortem(w, h).rects();
            assert!(panel.bottom() <= rects[0].y, "{screen:?}");
            assert!(rects[1].bottom() <= h, "{screen:?}: links run off the foot");
        }
    }

    #[test]
    fn a_link_row_reads_left_to_right_without_its_targets_overlapping() {
        for screen in SHAPES {
            let (w, h) = screen;
            for row in [
                LinkRow::title(w, h, fit(screen, (64, 48)).cell, false),
                LinkRow::post_mortem(w, h),
            ] {
                let rects = row.rects();
                assert!(rects[0].right() <= rects[1].x, "{screen:?}");
                for rect in rects {
                    assert!(rect.x >= 0.0 && rect.right() <= w, "{screen:?} off screen");
                    // Wider than the glyphs and taller than the line, so a
                    // thumb aimed at the middle of the word lands on it.
                    assert!(rect.h > row.size, "{screen:?} no padding to hit");
                }
            }
        }
    }

    #[test]
    fn the_title_screen_says_where_the_settings_are() {
        assert!(TITLE_HINTS.iter().any(|line| line.contains("S settings")));
    }

    #[test]
    fn the_settings_rows_stack_inside_their_panel() {
        for (width, height) in [
            (320.0_f32, 568.0_f32),
            (390.0, 844.0),
            (844.0, 390.0),
            DESKTOP,
            (1920.0, 1080.0),
        ] {
            let geometry = settings_panel(width, height);
            let panel = geometry.panel;
            for (i, row) in geometry.rows.iter().enumerate() {
                assert!(row.h >= MIN_TARGET, "{width}x{height}: row {i} is small");
                assert!(
                    panel.contains(row.point())
                        && row.right() <= panel.right()
                        && row.bottom() <= panel.bottom(),
                    "{width}x{height}: row {i} escaped the panel"
                );
                if let Some(next) = geometry.rows.get(i + 1) {
                    assert!(row.bottom() <= next.y, "{width}x{height}: rows overlap");
                }
            }
            let last = geometry.rows.last().expect("the panel has rows");
            assert!(
                last.bottom() <= geometry.close.y,
                "{width}x{height}: the last row ran into the way out"
            );
            assert!(geometry.close.bottom() <= panel.bottom());
            assert!(geometry.close.h >= MIN_TARGET);
        }
    }

    /// The screens the pad has to share with the panels: two phones upright,
    /// one on its side, two tablets, and the desktop window somebody touched.
    const TOUCHED: [(f32, f32); 7] = [
        (320.0, 568.0),
        (390.0, 844.0),
        (414.0, 896.0),
        (844.0, 390.0),
        (768.0, 1024.0),
        (1024.0, 768.0),
        (1032.0, 708.0),
    ];

    #[test]
    fn the_pad_never_lands_on_a_panel() {
        // The pad is drawn last and over everything, so the only thing keeping
        // it off the log, the map and the organelle list is the room the
        // layout was told not to use. Check every rectangle against every
        // other one rather than trusting that arithmetic by eye.
        for (width, height) in TOUCHED {
            for map in [(48, 48), (64, 48)] {
                for log in [true, false] {
                    let layout =
                        Layout::fit(Controls::free(width, height), map.0, map.1, log, true);
                    let controls = Controls::fit(width, height, true);
                    let pad = std::iter::once(controls.dpad)
                        .chain(std::iter::once(controls.settings))
                        .chain(controls.buttons.iter().map(|&(_, rect)| rect));
                    for control in pad {
                        for (name, panel) in [
                            ("map", layout.map),
                            ("log", layout.info),
                            ("sidebar", layout.sidebar),
                        ] {
                            assert!(
                                !control.overlaps(&panel),
                                "{width}x{height} {map:?} log={log}: the pad sat on the {name}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_phone_with_the_pad_out_still_has_a_map_worth_looking_at() {
        let layout = fit_touch(PHONE, (48, 48));
        assert_eq!(layout.mode, Mode::Narrow);
        assert!(layout.touch);
        assert!(layout.cell >= NARROW_MIN_CELL, "cells were {}", layout.cell);
        // Two thirds of the map's rows, which is what makes it a game rather
        // than a keyhole.
        assert!(layout.rows >= 32, "only {} rows survived", layout.rows);
    }

    #[test]
    fn a_phone_on_its_side_puts_the_pad_beside_the_map_rather_than_under_it() {
        // Height is the scarce thing in landscape and width is not, so a band
        // along the bottom would cost the map half its rows for nothing.
        let side = fit_touch((PHONE.1, PHONE.0), (48, 48));
        let upright = fit_touch(PHONE, (48, 48));
        assert!(
            side.rows > upright.rows / 2,
            "landscape kept only {} rows",
            side.rows
        );
        let free = Controls::free(PHONE.1, PHONE.0);
        assert!(free.x > 0.0, "the pad took a band instead of a column");
        assert!(
            (free.h - PHONE.0).abs() < f32::EPSILON,
            "and kept the height"
        );
    }

    #[test]
    fn the_pager_is_a_thumb_sized_pair_at_the_foot_of_the_sidebar() {
        for (width, height) in TOUCHED {
            let layout = Layout::fit(Controls::free(width, height), 48, 48, true, true);
            let pager = layout.sidebar_pager().expect("a touch layout has one");
            for button in pager.buttons {
                assert!(
                    button.h >= MIN_TARGET,
                    "{width}x{height}: {} px is not a thumb",
                    button.h
                );
                assert!(
                    button.x >= layout.sidebar.x - f32::EPSILON
                        && button.right() <= layout.sidebar.right() + f32::EPSILON
                        && button.bottom() <= layout.sidebar.bottom() + f32::EPSILON,
                    "{width}x{height}: it escaped the sidebar"
                );
            }
            // Two buttons, side by side, with no seam a tap can fall through.
            let [prev, next] = pager.buttons;
            assert!((prev.right() - next.x).abs() < f32::EPSILON);
            // And the list stops above them.
            let capacity = hud::sidebar_capacity(layout.sidebar_rows, pager.rows);
            let last_entry = layout.cell.mul_add(capacity as f32 + 8.0, layout.sidebar.y);
            assert!(
                last_entry <= prev.y + f32::EPSILON,
                "{width}x{height}: the list ran under the paging buttons"
            );
        }
    }

    #[test]
    fn a_keyboard_layout_has_no_paging_buttons_to_hit() {
        assert!(fit(DESKTOP, (64, 48)).sidebar_pager().is_none());
        assert!(fit(PHONE, (48, 48)).sidebar_pager().is_none());
    }

    #[test]
    fn the_restart_button_sits_inside_its_panel() {
        let (panel, button) = post_mortem_panel(DESKTOP.0, DESKTOP.1);
        assert!(panel.contains(button.point()));
        assert!(panel.contains(Vec2::new(button.right(), button.bottom())));
        assert!(button.h >= 48.0);
    }
}
