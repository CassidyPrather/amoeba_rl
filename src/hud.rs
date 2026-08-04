//! The two panels: what just happened, and what you are made of.
//!
//! Both are transcriptions of the original's consoles, written against
//! [`Term`]'s cell coordinates so they do not care where on screen they ended
//! up or how big a cell got. The sim decided every string and every colour in
//! here; what is left is column numbers, plus the small pure arithmetic that
//! keeps the organelle list honest when the panel drawing it is shorter than
//! the fixed page the sim counts in.

use amoeba_rl::sim::actors::{Rgb, palette};
use amoeba_rl::sim::view::{Described, OrganelleEntry};
use amoeba_rl::sim::{Phase, RenderView, UiMode};

use crate::render::{SIDEBAR_COLS, Term, rgb, rgba};
use macroquad::math::Rect;
use macroquad::shapes::{draw_rectangle, draw_rectangle_lines};

/// Column the sidebar's selection marker sits in.
const MARK_COL: i32 = 1;

/// Column the sidebar's names and progress bars start at.
const NAME_COL: i32 = 3;

/// Columns a sidebar name, and so a progress bar, is wide.
const NAME_COLS: i32 = SIDEBAR_COLS - 4;

/// First sidebar row an organelle can appear on. Three more than the original,
/// which had no room for the win condition or the price of the next gate.
const ENTRY_ROW: i32 = 8;

/// Below this many columns the key hints have to be abbreviated.
///
/// The panel is as wide as the map, and the maps are 38 columns on Normal and
/// Easy and 48 on GJ, so this is the line between the two of them.
const WIDE_HINT_COLS: i32 = 46;

/// The message bar: the log, or a description of whatever has your attention.
///
/// `audio` is the sound half asking for a press or admitting it is muted. It
/// shares the key-hint row and yields to it, because the hints are what the
/// player is actually looking for down there.
///
/// `log` is the player's choice about the running account of what happened.
/// Only the account goes: examine mode and the organelle browser still describe
/// what they are pointing at, because those answer a question the player just
/// asked rather than narrating one they did not.
///
/// `touch` decides whether the bottom row names keys or taps. It is the whole
/// of this module's answer to a phone: every key letter in here is a dead end
/// on a screen that has no keys, so a screen with none is told about the
/// buttons it does have instead.
pub fn info(
    term: &Term,
    view: &RenderView,
    cols: i32,
    rows: i32,
    audio: Option<&str>,
    log: bool,
    touch: bool,
) {
    let content = (rows - 2).max(1);
    match view.mode {
        UiMode::Messages if log => {
            // Filled upwards from the last row, so the newest line is always
            // in the same place however few lines there are to show.
            let shown = usize::try_from(content).unwrap_or(0);
            for (row, line) in fill_upwards(&view.messages, cols - 2, shown)
                .iter()
                .enumerate()
            {
                let Ok(offset) = i32::try_from(row) else {
                    break;
                };
                term.text(1, content - offset, line, palette::TEXT_HEADING);
            }
        }
        UiMode::Messages => {}
        UiMode::Organelles => {
            if let Some(entry) = selected(view) {
                describe(
                    term,
                    cols,
                    content,
                    (&entry.name, palette::SLIME),
                    &entry.description,
                    &[],
                );
            }
        }
        UiMode::Examine => match view.examine.as_ref().and_then(|e| e.target.as_ref()) {
            Some(target) => describe_target(term, cols, content, target),
            None => term.text(1, 1, "You see nothing there.", palette::FLOOR_FOV),
        },
    }
    let hints = hint(view.mode, view.phase, cols, touch);
    // A phone's viewport can be narrower than any useful line of hints, so the
    // row is cut to the panel rather than allowed to run off the end of it.
    term.text(1, rows - 1, &truncate(hints, cols - 2), palette::TEXT_BODY);
    if let Some(note) = audio
        && let Some(col) = fits_after(hints, note, cols)
    {
        term.text(col, rows - 1, note, palette::SUPER_BRIGHT);
    }
}

/// The last `rows` rows of the log, wrapped to `width` and ordered bottom row
/// first — the order the panel fills in, newest at the floor.
///
/// The wrapping is the point. The sim breaks its own lines at sixty-two
/// columns and a phone's panel is barely half that, so cutting them to fit
/// would lose the end of every sentence that had any news in it. A message
/// that no longer fits takes two rows and pushes an older one off the top
/// instead, which is what a message log is for.
fn fill_upwards(messages: &[String], width: i32, rows: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(rows);
    for message in messages.iter().rev() {
        if out.len() >= rows {
            break;
        }
        // A message's own rows read downwards; this list is being built the
        // other way, so its last row is the one that goes in first.
        out.extend(wrap(message, width).into_iter().rev());
    }
    out.truncate(rows);
    out
}

/// The column a right-aligned `note` starts in, if the row still has room for
/// it after `hints` and a gap. `None` when it would collide.
fn fits_after(hints: &str, note: &str, cols: i32) -> Option<i32> {
    let end = i32::try_from(hints.chars().count()).ok()? + 1;
    let col = cols - 1 - i32::try_from(note.chars().count()).ok()?;
    (col >= end + 2).then_some(col)
}

/// The organelle list's paging control on a screen with no keyboard: where its
/// two buttons are, and how many rows of the panel they have taken.
///
/// The rectangles are in window pixels and come from the layout, so the
/// buttons are drawn exactly where [`input`](crate::input) is listening for
/// them.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pager {
    /// Rows at the foot of the sidebar that are the control's, not the list's.
    pub rows: i32,
    /// Previous page, then next page.
    pub buttons: [Rect; 2],
}

/// The organelle sidebar: the numbers that decide the run, then the mass.
///
/// `page` is the frontend's own, counted in the rows this panel actually has.
/// The sim tracks a page too, but in fixed fifty-five-line pages it never puts
/// in [`RenderView`], so paging has to be mirrored out here.
///
/// `pager` is the touchable version of the `Q` and `E` hints in the bottom
/// corners, and is `None` wherever there is a keyboard to press them on.
pub fn sidebar(term: &Term, view: &RenderView, rows: i32, page: usize, pager: Option<Pager>) {
    let status = &view.status;
    term.text(1, 1, "Organelles", palette::TEXT_HEADING);
    for (row, line) in [
        format!("Mass:  {}", status.mass),
        format!("Turn:  {:.0}", status.turn),
        format!("Gates left: {}", status.cities_remaining),
        format!("Must break: {}", status.cities_required),
        // Gates are not all priced the same once one has come down, so this
        // is the cheapest of the ones still standing rather than a number that
        // holds for all of them — the mass to reach before any gate at all is
        // breakable. What a particular gate costs is on the gate.
        format!("Best gate: {}", status.city_armor),
    ]
    .iter()
    .enumerate()
    {
        let Ok(offset) = i32::try_from(row) else {
            break;
        };
        term.text(1, 2 + offset, line, palette::TEXT_BODY);
    }

    let footer = pager.map_or(1, |p| p.rows);
    let capacity = sidebar_capacity(rows, footer);
    let count = view.organelles.len();
    let selected_at = view.organelles.iter().position(|e| e.selected).unwrap_or(0);
    let (start, end) = visible_range(count, page, selected_at, capacity);
    let shown = &view.organelles[start..end];
    // Backgrounds for every row before any glyph, for the reason [`Term::grid`]
    // gives: the bars are untextured and the names are not.
    for (row, entry) in shown.iter().enumerate() {
        let Ok(offset) = i32::try_from(row) else {
            break;
        };
        entry_background(term, entry, ENTRY_ROW + offset);
    }
    for (row, entry) in shown.iter().enumerate() {
        let Ok(offset) = i32::try_from(row) else {
            break;
        };
        entry_text(term, entry, ENTRY_ROW + offset, pager.is_some());
    }
    // The control stays put whether or not the list overflows, because a
    // button that comes and goes under a thumb is worse than a dead one; the
    // key hints, which cost a row nobody is aiming at, still only appear when
    // there is somewhere to page to.
    match pager {
        Some(pager) => page_buttons(term, &pager),
        None if count > capacity => {
            term.text_on(
                0,
                rows - 1,
                "Q\u{25B2}",
                palette::TEXT_HEADING,
                palette::SLIME,
            );
            term.text_on(
                SIDEBAR_COLS - 2,
                rows - 1,
                "E\u{25BC}",
                palette::TEXT_HEADING,
                palette::SLIME,
            );
        }
        None => {}
    }
}

/// The two paging buttons, drawn where the layout put them.
fn page_buttons(term: &Term, pager: &Pager) {
    for (rect, glyph) in pager.buttons.iter().zip(['\u{25B2}', '\u{25BC}']) {
        draw_rectangle(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            rgba(palette::FLOOR_BACKGROUND_FOV, 0.55),
        );
        draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 2.0, rgb(palette::SLIME));
        let size = (rect.h * 0.5).min(rect.w * 0.5).max(8.0);
        term.font().draw(
            glyph,
            (rect.center().x - size * 0.5).floor(),
            (rect.center().y - size * 0.5).floor(),
            size,
            rgb(palette::TEXT_HEADING),
        );
    }
}

/// The one line the map-first layout owes the numbers the sidebar would
/// otherwise be showing. It has a row of its own above the map, so this can
/// paint its background solid rather than hoping a cell of the world reads
/// through it.
pub fn status_strip(term: &Term, view: &RenderView, cols: i32) {
    let status = &view.status;
    let line = format!(
        "Mass {}  Turn {:.0}  Gates {}/{}",
        status.mass, status.turn, status.cities_remaining, status.cities_required
    );
    let cell = term.cell_size();
    draw_rectangle(
        term.origin().x,
        term.origin().y,
        cols as f32 * cell,
        cell,
        rgb(palette::FLOOR_BACKGROUND_FOV),
    );
    term.text(1, 0, &truncate(&line, cols - 2), palette::TEXT_HEADING);
}

/// One sidebar row's progress bar, and the patch behind its navigation hint.
fn entry_background(term: &Term, entry: &OrganelleEntry, row: i32) {
    if let Some(bar) = entry.bar {
        let cutoff = fill_cols(bar.fraction);
        for col in 0..NAME_COLS {
            let color = if col < cutoff { bar.fill } else { bar.empty };
            term.background(NAME_COL + col, row, color);
        }
        // A dissolving human that has already banked a free product shows it
        // as a second bar over the top of the first.
        if let Some(overfill) = bar.overfill {
            for col in 0..fill_cols(overfill) {
                term.background(NAME_COL + col, row, palette::OVERFILL);
            }
        }
    }
    if entry.nucleus_hint.is_some() {
        term.background(SIDEBAR_COLS - 1, row, palette::SLIME);
    }
}

/// One sidebar row's marker, name and navigation hint.
fn entry_text(term: &Term, entry: &OrganelleEntry, row: i32, touch: bool) {
    if entry.examined {
        term.glyph(MARK_COL, row, '>', palette::CURSOR);
    } else if entry.selected {
        term.glyph(MARK_COL, row, '>', palette::TEXT_HEADING);
    }
    term.text(NAME_COL, row, &truncate(&entry.name, NAME_COLS), entry.fg);
    if let Some(hint) = entry.nucleus_hint {
        term.glyph(
            SIDEBAR_COLS - 1,
            row,
            nucleus_mark(hint, touch),
            palette::SUPER_BRIGHT,
        );
    }
}

/// The mark against the nuclei either side of the one you are steering.
///
/// The sim names the keys that reach them, which is the answer on a keyboard
/// and nothing at all on a phone. There the same two are marked with the
/// arrows the *prev* and *next* buttons wear, so the mark points at the button
/// that gets you there. `@` — you are here — needs no translating.
const fn nucleus_mark(hint: char, touch: bool) -> char {
    match (hint, touch) {
        ('A', true) => '\u{25C4}',
        ('D', true) => '\u{25BA}',
        (mark, _) => mark,
    }
}

/// A name, a description, and whatever else the sim knows about it.
fn describe_target(term: &Term, cols: i32, rows: i32, target: &Described) {
    describe(
        term,
        cols,
        rows,
        (&target.name, target.color),
        &target.description,
        &target.detail,
    );
}

/// The info console's one layout, from §9.5: heading, body, extra lines.
fn describe(
    term: &Term,
    cols: i32,
    rows: i32,
    heading: (&str, Rgb),
    body: &str,
    detail: &[String],
) {
    let (name, color) = heading;
    term.text(1, 1, name, color);
    // The full-height panel can afford the original's blank separator row; the
    // collapsed one cannot.
    let first_row = if rows >= 9 { 3 } else { 2 };
    let width = (cols - 2).max(1);
    let lines = wrap(body, width)
        .into_iter()
        .chain(detail.iter().flat_map(|line| wrap(line, width)));
    for (row, line) in (first_row..=rows).zip(lines) {
        term.text(1, row, &line, palette::TEXT_HEADING);
    }
}

/// The sidebar entry the organelle browser is pointing at.
fn selected(view: &RenderView) -> Option<&OrganelleEntry> {
    view.organelles.iter().find(|entry| entry.selected)
}

/// The slice of entries to draw.
///
/// It starts where `page` says and then slides just far enough to keep the
/// selection on screen, so the arrow keys can always see what they are moving
/// even when `Q` and `E` have paged somewhere else.
#[must_use]
fn visible_range(count: usize, page: usize, selected: usize, capacity: usize) -> (usize, usize) {
    if count == 0 || capacity == 0 {
        return (0, 0);
    }
    let mut start = (page * capacity).min(count - 1);
    if selected < start {
        start = selected;
    } else if selected >= start + capacity {
        start = selected + 1 - capacity;
    }
    (start, (start + capacity).min(count))
}

/// How many pages of `capacity` rows `count` entries take.
#[must_use]
pub const fn pages(count: usize, capacity: usize) -> usize {
    if capacity == 0 || count <= capacity {
        1
    } else {
        count.div_ceil(capacity)
    }
}

/// Rows this window gives the organelle list, given the sidebar's height.
///
/// `footer` rows go to the paging control at the bottom — one for the key
/// hints, several for a pair of buttons a thumb can hit — and the rest of the
/// difference is the header.
#[must_use]
pub fn sidebar_capacity(rows: i32, footer: i32) -> usize {
    usize::try_from(rows - footer.max(0) - ENTRY_ROW).unwrap_or(0)
}

/// How many of a bar's columns are filled, floored as the original floored it.
fn fill_cols(fraction: f32) -> i32 {
    (NAME_COLS as f32 * fraction.clamp(0.0, 1.0)) as i32
}

/// The hints for whatever mode is open, abbreviated on a narrow panel.
///
/// Every one of these has to fit the map it is drawn under, and the default
/// difficulty's map is 38 columns wide — so the narrow tier, not the wide one,
/// is what most players read, and it has thirty-six characters to say
/// everything in. `S settings` earns ten of them in every keyboard tier: it is
/// the only way to find the panel that turns any of this off. What gets cut
/// first is whatever the player has already found out by pressing it — nobody
/// needs to be told twice that the arrows move.
///
/// The touch tier names no keys and no settings, for the same reason in both
/// cases — the buttons are already on the screen, labelled. What it says
/// instead is the one thing they cannot say for themselves: that the map takes
/// taps too.
const fn hint(mode: UiMode, phase: Phase, cols: i32, touch: bool) -> &'static str {
    if touch {
        return match (mode, phase) {
            (_, Phase::GameOver { .. }) => "tap play again",
            (UiMode::Messages, _) => "tap a tile beside you to move",
            (UiMode::Organelles, _) => "list closes   arrows page",
            (UiMode::Examine, _) => "tap a tile to look at it",
        };
    }
    let wide = cols >= WIDE_HINT_COLS;
    if matches!(phase, Phase::GameOver { .. }) {
        return "R plays again   S settings";
    }
    match (mode, wide) {
        (UiMode::Messages, true) => "arrows move  wait  X look  Z list  S settings",
        (UiMode::Messages, false) => "wait  X look  Z list  S settings",
        (UiMode::Organelles, true) => "arrows select  Q/E page  Esc back  S settings",
        (UiMode::Organelles, false) => "Q/E page  Esc back  S settings",
        (UiMode::Examine, true) => "move cursor  X or Esc back  S settings",
        (UiMode::Examine, false) => "X or Esc back  S settings",
    }
}

/// Greedy word wrap, matching what the sim already did to the message log so
/// that descriptions break the same way messages do.
fn wrap(text: &str, width: i32) -> Vec<String> {
    let Ok(width) = usize::try_from(width) else {
        return Vec::new();
    };
    if width == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let extra = usize::from(!line.is_empty()) + word.chars().count();
        if !line.is_empty() && line.chars().count() + extra > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&truncate(word, {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            {
                width as i32
            }
        }));
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

/// The first `cols` characters, counted in characters rather than bytes.
fn truncate(text: &str, cols: i32) -> String {
    let Ok(cols) = usize::try_from(cols) else {
        return String::new();
    };
    text.chars().take(cols).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_list_shows_nothing() {
        assert_eq!(visible_range(0, 0, 0, 40), (0, 0));
        assert_eq!(visible_range(10, 0, 0, 0), (0, 0));
    }

    #[test]
    fn a_short_list_starts_at_the_top() {
        assert_eq!(visible_range(12, 0, 3, 40), (0, 12));
    }

    #[test]
    fn a_full_panel_stops_at_its_capacity() {
        assert_eq!(visible_range(200, 0, 0, 40), (0, 40));
    }

    #[test]
    fn the_selection_is_always_on_screen() {
        // Whichever page is showing, the row the arrow keys are on has to be
        // one of the rows drawn, or navigation goes blind.
        let capacity = 40;
        for selected in 0..200 {
            for page in 0..6 {
                let (start, end) = visible_range(200, page, selected, capacity);
                assert!(
                    (start..end).contains(&selected),
                    "page {page} row {selected} fell outside {start}..{end}"
                );
                assert!(end - start <= capacity);
            }
        }
    }

    #[test]
    fn paging_moves_the_window_by_the_rows_this_panel_has() {
        let (start, end) = visible_range(400, 1, 40, 40);
        assert_eq!((start, end), (40, 80));
        let (start, end) = visible_range(400, 2, 80, 40);
        assert_eq!((start, end), (80, 120));
    }

    #[test]
    fn a_page_past_the_end_still_lands_inside_the_list() {
        let (start, end) = visible_range(10, 5, 0, 40);
        assert!(start < 10 && end <= 10);
        assert!(start <= end);
    }

    #[test]
    fn page_counts_cover_every_entry() {
        assert_eq!(pages(0, 40), 1);
        assert_eq!(pages(1, 40), 1);
        assert_eq!(pages(40, 40), 1);
        assert_eq!(pages(41, 40), 2);
        assert_eq!(pages(400, 40), 10);
        // A panel with no room still reports one page rather than dividing by
        // zero, so the paging keys stay harmless.
        assert_eq!(pages(9, 0), 1);
    }

    #[test]
    fn the_sidebar_reports_the_rows_it_can_actually_draw() {
        // The reference sidebar: 59 rows, eight of header, one of paging keys.
        assert_eq!(sidebar_capacity(59, 1), 50);
        assert_eq!(sidebar_capacity(ENTRY_ROW + 1, 1), 0);
        assert_eq!(sidebar_capacity(0, 1), 0);
        assert_eq!(sidebar_capacity(-20, 1), 0);
        // A thumb needs a taller control than a key hint does, and every row
        // of it is a row the list does not get.
        assert_eq!(sidebar_capacity(59, 5), 46);
        assert_eq!(sidebar_capacity(ENTRY_ROW + 3, 5), 0);
    }

    #[test]
    fn bars_fill_from_empty_to_full() {
        assert_eq!(fill_cols(0.0), 0);
        assert_eq!(fill_cols(1.0), NAME_COLS);
        assert_eq!(fill_cols(0.5), NAME_COLS / 2);
        // Out-of-range fractions are clamped rather than drawn off the end.
        assert_eq!(fill_cols(-1.0), 0);
        assert_eq!(fill_cols(2.0), NAME_COLS);
        assert_eq!(fill_cols(f32::NAN), 0);
    }

    #[test]
    fn the_log_fills_upwards_newest_first() {
        let log = ["one".to_owned(), "two".to_owned(), "three".to_owned()];
        // Bottom row first, so the newest line always lands in the same place.
        assert_eq!(fill_upwards(&log, 20, 4), ["three", "two", "one"]);
        // And an older line falls off the top rather than the newest.
        assert_eq!(fill_upwards(&log, 20, 2), ["three", "two"]);
        assert!(fill_upwards(&log, 20, 0).is_empty());
        assert!(fill_upwards(&[], 20, 4).is_empty());
    }

    #[test]
    fn a_line_too_wide_for_the_panel_wraps_rather_than_losing_its_end() {
        // The sim wraps at sixty-two columns; a phone's panel has about half
        // that, and the half it would drop is the half with the news in it.
        let log = ["The militia hits your maw for four".to_owned()];
        let rows = fill_upwards(&log, 20, 4);
        assert!(rows.len() > 1, "it was cut instead of wrapped");
        // Read bottom-up, the message is whole and in order.
        let whole: Vec<&str> = rows.iter().rev().map(String::as_str).collect();
        assert_eq!(whole.join(" "), log[0]);
        for row in &rows {
            assert!(row.chars().count() <= 20, "{row:?} was too wide");
        }
    }

    #[test]
    fn wrapping_never_exceeds_the_width() {
        let text =
            "A nucleus grown around a lens of bone. It sees more than twice as far into the dark.";
        for width in [10, 18, 30, 62] {
            for line in wrap(text, width) {
                assert!(
                    line.chars().count() <= usize::try_from(width).unwrap(),
                    "{width}: {line:?} was too long"
                );
            }
        }
    }

    #[test]
    fn wrapping_keeps_every_word() {
        let text = "It takes 100 mass to break this gate.";
        assert_eq!(wrap(text, 20).join(" "), text);
    }

    #[test]
    fn an_over_long_word_is_cut_rather_than_dropped() {
        let lines = wrap(&"x".repeat(90), 20);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].chars().count(), 20);
    }

    #[test]
    fn wrapping_a_degenerate_width_gives_up_quietly() {
        assert!(wrap("anything", 0).is_empty());
        assert!(wrap("anything", -4).is_empty());
    }

    #[test]
    fn the_audio_note_gives_way_to_the_key_hints() {
        // The hints are what a player scans that row for; the note about sound
        // only gets what is left, and takes nothing when there is nothing left.
        let hints = "arrows move   space wait   X examine   Z organelles   F1 help";
        assert_eq!(fits_after(hints, "audio muted - M", 86), Some(70));
        assert_eq!(fits_after(hints, "audio muted - M", 62), None);
        let narrow = "move  space wait  X examine  Z list";
        assert_eq!(fits_after(narrow, "audio muted - M", 40), None);
        // A note that ends exactly where the hints begin is still a collision.
        assert_eq!(fits_after("abc", "xy", 8), None);
        assert_eq!(fits_after("abc", "xy", 9), Some(6));
    }

    /// Every mode a hint is written for.
    const MODES: [UiMode; 3] = [UiMode::Messages, UiMode::Organelles, UiMode::Examine];

    #[test]
    fn every_hint_fits_the_panel_it_is_drawn_under() {
        // Nothing here can be checked by eye in CI, so check the arithmetic:
        // the panel is as wide as the map, and the default difficulty's map is
        // 38 columns — the wide tier only gets more room on GJ's 48.
        for (cols, budget) in [(38, 36_usize), (48, 46)] {
            for mode in MODES {
                for phase in [Phase::Playing, Phase::GameOver { won: true }] {
                    for touch in [false, true] {
                        let line = hint(mode, phase, cols, touch);
                        assert!(
                            line.chars().count() <= budget,
                            "{cols} cols, {mode:?}: {line:?} is {} characters",
                            line.chars().count()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_touch_hints_fit_a_phones_share_of_a_phone() {
        // The narrow layout draws this row under a map cut to the window, and
        // a 390 px phone at ten pixels a cell has 39 columns of it — two of
        // which are the panel's own margins.
        for mode in MODES {
            for phase in [Phase::Playing, Phase::GameOver { won: true }] {
                let line = hint(mode, phase, 39, true);
                assert!(
                    line.chars().count() <= 37,
                    "{mode:?}: {line:?} is {} characters",
                    line.chars().count()
                );
            }
        }
    }

    #[test]
    fn no_touch_hint_names_a_key() {
        // The whole point: a phone has no S to press, no Esc to go back with
        // and no Q and E to page with, so a row that names them is a row of
        // dead ends.
        for cols in [30, 48, 64] {
            for mode in MODES {
                for phase in [Phase::Playing, Phase::GameOver { won: false }] {
                    let line = hint(mode, phase, cols, true);
                    for key in ["S settings", "Esc", "Q/E", "Q and E", "R plays", "space"] {
                        assert!(!line.contains(key), "{mode:?}: {line:?} still says {key:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn the_nucleus_marks_point_at_whatever_gets_you_there() {
        // A keyboard is told which key; a thumb is told which button, using
        // the same two arrows those buttons wear.
        assert_eq!(nucleus_mark('A', false), 'A');
        assert_eq!(nucleus_mark('D', false), 'D');
        assert_eq!(nucleus_mark('A', true), '\u{25C4}');
        assert_eq!(nucleus_mark('D', true), '\u{25BA}');
        // "You are here" needs no translating either way.
        assert_eq!(nucleus_mark('@', true), '@');
        assert_eq!(nucleus_mark('@', false), '@');
    }

    #[test]
    fn every_key_hint_points_at_the_settings_panel() {
        // It is the only way to find the panel, and the panel is the only way
        // to turn the log or the animations off. A phone reaches it by the
        // button on the pad, so its hints are excused.
        for cols in [30, 48, 64] {
            for mode in MODES {
                for phase in [Phase::Playing, Phase::GameOver { won: false }] {
                    let line = hint(mode, phase, cols, false);
                    assert!(
                        line.contains("S settings"),
                        "{cols} cols, {mode:?}: {line:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn truncation_counts_characters_not_bytes() {
        assert_eq!(truncate("Biometal Forge", 8), "Biometal");
        assert_eq!(truncate("\u{e9}\u{e9}\u{e9}", 2), "\u{e9}\u{e9}");
        assert_eq!(truncate("short", 40), "short");
        assert_eq!(truncate("short", -1), "");
    }

    #[test]
    fn the_name_column_leaves_room_for_the_nucleus_hint() {
        // Cols 3..21 for the name, col 21 for the hint, col 1 for the marker.
        assert_eq!(NAME_COL + NAME_COLS, SIDEBAR_COLS - 1);
        const { assert!(MARK_COL < NAME_COL) }
    }
}
