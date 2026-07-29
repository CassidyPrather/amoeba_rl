//! The two logs: what just happened, and what you are made of.
//!
//! Both are sim-side state because both are part of the save-worthy world,
//! not of any particular renderer. The message log word-wraps at the width the
//! original's info console used so that line counts match what a 64-column
//! frontend will show; a frontend free to lay out differently can ignore the
//! wrapping and re-wrap the text itself.

/// Columns the message log wraps to: the original info console was 64 wide
/// with a one-cell margin on each side.
pub const WRAP_WIDTH: usize = 62;

/// Lines the original showed at once. More than this is kept so a frontend
/// can scroll back.
pub const VISIBLE_LINES: usize = 9;

/// Wrapped lines retained before the oldest is dropped.
const CAPACITY: usize = 256;

/// Entries the original's sidebar fit on one page.
pub const PAGE_LINES: usize = 55;

/// A bounded ring of wrapped message lines, oldest first.
#[derive(Clone, Debug, Default)]
pub struct MessageLog {
    lines: Vec<String>,
}

impl MessageLog {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a message, wrapping it to [`WRAP_WIDTH`] first.
    pub(crate) fn add(&mut self, message: &str) {
        for line in wrap(message, WRAP_WIDTH) {
            self.lines.push(line);
        }
        if self.lines.len() > CAPACITY {
            self.lines.drain(..self.lines.len() - CAPACITY);
        }
    }

    /// Every retained line, oldest first.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// The last `n` lines, oldest first — what a fixed-height panel shows.
    #[must_use]
    pub fn tail(&self, n: usize) -> &[String] {
        let start = self.lines.len().saturating_sub(n);
        &self.lines[start..]
    }
}

/// Greedy word wrap.
///
/// DELIBERATE CHANGE from C#: the original emitted a trailing space on every
/// wrapped line and turned a single over-long word into an empty line followed
/// by the word. Here over-long words are hard-split and no trailing space is
/// emitted, because both quirks are only visible as ragged output.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let mut word = word;
        while word.chars().count() > width {
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            let split = word
                .char_indices()
                .nth(width)
                .map_or(word.len(), |(i, _)| i);
            let (head, tail) = word.split_at(split);
            out.push(head.to_owned());
            word = tail;
        }
        let extra = usize::from(!line.is_empty()) + word.chars().count();
        if line.chars().count() + extra > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() || out.is_empty() {
        out.push(line);
    }
    out
}

/// Cursor and paging state for the organelle sidebar.
///
/// The list itself is the player's mass, which lives on the world; only the
/// selection lives here. `nice_turn` exists because winning and losing rewind
/// the schedule clock to zero, and the displayed turn counter must not follow
/// it back down.
#[derive(Clone, Copy, Debug, Default)]
pub struct OrganelleLog {
    idx: usize,
    page: usize,
    nice_turn: u64,
}

impl OrganelleLog {
    /// A fresh cursor at the top of the list.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The selected entry's index, clamped to a list of `count` entries.
    #[must_use]
    pub const fn selected(&self, count: usize) -> usize {
        if count == 0 || self.idx >= count {
            0
        } else {
            self.idx
        }
    }

    /// The page currently scrolled to, given `count` entries.
    #[must_use]
    pub const fn page(&self, count: usize) -> usize {
        let pages = count.div_ceil(PAGE_LINES);
        if self.page == 0 || pages == 0 {
            0
        } else {
            self.page % pages
        }
    }

    /// Move the selection, wrapping at both ends.
    pub(crate) const fn scroll(&mut self, by: i32, count: usize) {
        if count == 0 {
            self.idx = 0;
            return;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let len = count as i32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let cur = self.selected(count) as i32;
        #[allow(clippy::cast_sign_loss)]
        {
            self.idx = (cur + by).rem_euclid(len) as usize;
        }
    }

    /// Move by whole screens. Clamped at the top, wrapped at the bottom by
    /// [`OrganelleLog::page`].
    pub(crate) fn turn_page(&mut self, by: i32) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let cur = self.page as i32;
        #[allow(clippy::cast_sign_loss)]
        {
            self.page = (cur + by).max(0) as usize;
        }
    }

    /// Feed the schedule clock in; the counter only ever climbs.
    pub(crate) fn observe_time(&mut self, time: u64) {
        self.nice_turn = self.nice_turn.max(time);
    }

    /// The displayed turn number: time divided by one turn's length.
    #[must_use]
    pub fn turn(&self) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        {
            self.nice_turn as f32 / super::schedule::TURN as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_messages_stay_on_one_line() {
        let mut log = MessageLog::new();
        log.add("The Militia is engulfed!");
        assert_eq!(log.lines(), ["The Militia is engulfed!"]);
    }

    #[test]
    fn long_messages_wrap_at_the_console_width() {
        let mut log = MessageLog::new();
        log.add(
            "Consult the README file to review these instructions and more. F1 to show these messages again.",
        );
        assert!(log.lines().len() > 1);
        assert!(log.lines().iter().all(|l| l.chars().count() <= WRAP_WIDTH));
    }

    #[test]
    fn over_long_words_are_split_rather_than_dropped() {
        let word = "x".repeat(150);
        let lines = wrap(&word, 62);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines.concat(), word);
    }

    #[test]
    fn tail_returns_the_most_recent_lines() {
        let mut log = MessageLog::new();
        for i in 0..20 {
            log.add(&format!("line {i}"));
        }
        let tail = log.tail(VISIBLE_LINES);
        assert_eq!(tail.len(), VISIBLE_LINES);
        assert_eq!(tail[VISIBLE_LINES - 1], "line 19");
    }

    #[test]
    fn the_ring_is_bounded() {
        let mut log = MessageLog::new();
        for i in 0..(CAPACITY + 50) {
            log.add(&format!("line {i}"));
        }
        assert_eq!(log.lines().len(), CAPACITY);
        assert_eq!(log.lines()[0], "line 50");
    }

    #[test]
    fn selection_wraps_both_ways() {
        let mut log = OrganelleLog::new();
        log.scroll(-1, 4);
        assert_eq!(log.selected(4), 3);
        log.scroll(1, 4);
        assert_eq!(log.selected(4), 0);
        log.scroll(6, 4);
        assert_eq!(log.selected(4), 2);
    }

    #[test]
    fn selection_survives_a_shrinking_list() {
        let mut log = OrganelleLog::new();
        log.scroll(9, 20);
        assert_eq!(log.selected(20), 9);
        assert_eq!(log.selected(3), 0);
        assert_eq!(log.selected(0), 0);
    }

    #[test]
    fn paging_clamps_at_the_top_and_wraps_at_the_bottom() {
        let mut log = OrganelleLog::new();
        log.turn_page(-3);
        assert_eq!(log.page(200), 0);
        log.turn_page(1);
        assert_eq!(log.page(200), 1);
        log.turn_page(3);
        assert_eq!(log.page(200), 0);
    }

    #[test]
    fn the_turn_counter_never_goes_backwards() {
        let mut log = OrganelleLog::new();
        log.observe_time(320);
        assert!((log.turn() - 20.0).abs() < f32::EPSILON);
        log.observe_time(0);
        assert!((log.turn() - 20.0).abs() < f32::EPSILON);
    }
}
