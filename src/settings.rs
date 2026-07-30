//! What the player has decided about the interface.
//!
//! None of this reaches [`amoeba_rl::sim`]. Whether the message log is on
//! screen, how long an animation runs, whether there is any sound — these are
//! all facts about a window, and the sim resolves the same world either way.
//! So the settings live out here with the rest of the presentation, and the
//! menu is a frontend overlay rather than a [`UiMode`](amoeba_rl::sim::UiMode):
//! the sim never learns the player opened it.
//!
//! The panel is state that outlives a frame, so it is a struct rather than a
//! function, and everything in it is pure — which row is selected, what a row's
//! value reads as, what toggling one does. Only [`Settings::sound`] is a mirror
//! of somebody else's state: mute belongs to the audio half, which owns the
//! only copy, and the menu asks it to flip rather than keeping a second one.

/// How fast the animation half plays a turn back.
///
/// A multiplier on every duration *and* on the cap, so choosing `Fast` shortens
/// the longest a turn can spend showing itself rather than merely speeding up
/// the quiet ones.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Speed {
    /// Half again as long. For a first run.
    Slow,
    /// The default.
    #[default]
    Normal,
    /// About half as long. For somebody who already knows what a maw does.
    Fast,
}

impl Speed {
    /// The multiplier on every animation duration.
    #[must_use]
    pub const fn scale(self) -> f32 {
        match self {
            Self::Slow => 1.75,
            Self::Normal => 1.0,
            Self::Fast => 0.5,
        }
    }

    /// The next speed, wrapping.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Slow => Self::Normal,
            Self::Normal => Self::Fast,
            Self::Fast => Self::Slow,
        }
    }

    /// How this reads in the menu.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Slow => "slow",
            Self::Normal => "normal",
            Self::Fast => "fast",
        }
    }
}

/// One line of the settings panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Row {
    /// Whether the message log is on screen.
    Log,
    /// Whether anything is animated at all.
    Animations,
    /// How long the animations run for.
    Speed,
    /// Whether there is any sound.
    Sound,
}

/// The panel, top to bottom.
pub const ROWS: [Row; 4] = [Row::Log, Row::Animations, Row::Speed, Row::Sound];

impl Row {
    /// The label in the left column.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Log => "Message log",
            Self::Animations => "Animations",
            Self::Speed => "Animation speed",
            Self::Sound => "Sound",
        }
    }

    /// One line about what this row is for, shown under the selection.
    #[must_use]
    pub const fn help(self) -> &'static str {
        match self {
            Self::Log => "The running account of what just happened.",
            Self::Animations => "Show every attack, retreat and upgrade as it happens.",
            Self::Speed => "How long a turn spends showing itself.",
            Self::Sound => "The same news, for the ears.",
        }
    }
}

/// What toggling a row needs somebody else to do about it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[must_use]
pub enum Toggled {
    /// Nothing: the setting lives here and has already changed.
    Done,
    /// Ask the audio half to flip its mute, which is the only copy of it.
    Sound,
}

/// Everything the player has decided, plus where the menu cursor is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    /// Whether the message log is drawn. Off gives its rows back to the map.
    pub log: bool,
    /// Whether turns are animated.
    pub animations: bool,
    /// How fast, when they are.
    pub speed: Speed,
    /// Whether the panel is on screen.
    open: bool,
    /// Which row the arrow keys are on.
    cursor: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Settings {
    /// The defaults: everything on, at the pace the game was designed at.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            log: true,
            animations: true,
            speed: Speed::Normal,
            open: false,
            cursor: 0,
        }
    }

    /// Whether the panel is on screen, and so whether it is eating the arrow
    /// keys.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// Open the panel, or close it again.
    pub const fn toggle_panel(&mut self) {
        self.open = !self.open;
    }

    /// Close the panel however it was opened.
    pub const fn close(&mut self) {
        self.open = false;
    }

    /// The row the arrow keys are on.
    #[must_use]
    pub const fn selected(&self) -> Row {
        ROWS[self.cursor % ROWS.len()]
    }

    /// The index of that row, for a renderer drawing a marker beside it.
    #[must_use]
    pub const fn selected_index(&self) -> usize {
        self.cursor % ROWS.len()
    }

    /// Move the selection, wrapping at both ends.
    pub const fn scroll(&mut self, by: i32) {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let len = ROWS.len() as i32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let at = self.cursor as i32;
        #[allow(clippy::cast_sign_loss)]
        {
            self.cursor = (at + by).rem_euclid(len) as usize;
        }
    }

    /// Change whatever the selection is on.
    pub const fn toggle_selected(&mut self) -> Toggled {
        self.toggle(self.selected())
    }

    /// Change one row, and put the cursor on it — a tap should leave the
    /// keyboard where the thumb was.
    pub const fn toggle(&mut self, row: Row) -> Toggled {
        if let Some(at) = index_of(row) {
            self.cursor = at;
        }
        match row {
            Row::Log => {
                self.log = !self.log;
                Toggled::Done
            }
            Row::Animations => {
                self.animations = !self.animations;
                Toggled::Done
            }
            Row::Speed => {
                self.speed = self.speed.next();
                Toggled::Done
            }
            Row::Sound => Toggled::Sound,
        }
    }

    /// What a row's right-hand column reads as.
    ///
    /// `sound_on` comes from the audio half, because that is where mute lives.
    #[must_use]
    pub const fn value(&self, row: Row, sound_on: bool) -> &'static str {
        match row {
            Row::Log => {
                if self.log {
                    "shown"
                } else {
                    "hidden"
                }
            }
            Row::Animations => {
                if self.animations {
                    "on"
                } else {
                    "off"
                }
            }
            Row::Speed => self.speed.name(),
            Row::Sound => {
                if sound_on {
                    "on"
                } else {
                    "muted"
                }
            }
        }
    }
}

/// Where a row sits in [`ROWS`].
const fn index_of(row: Row) -> Option<usize> {
    let mut i = 0;
    while i < ROWS.len() {
        if matches!(
            (ROWS[i], row),
            (Row::Log, Row::Log)
                | (Row::Animations, Row::Animations)
                | (Row::Speed, Row::Speed)
                | (Row::Sound, Row::Sound)
        ) {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_everything_on() {
        let settings = Settings::new();
        assert!(settings.log);
        assert!(settings.animations);
        assert_eq!(settings.speed, Speed::Normal);
        assert!(!settings.is_open(), "and the menu is out of the way");
    }

    #[test]
    fn the_panel_opens_and_closes_on_the_same_key() {
        let mut settings = Settings::new();
        settings.toggle_panel();
        assert!(settings.is_open());
        settings.toggle_panel();
        assert!(!settings.is_open());
        settings.toggle_panel();
        settings.close();
        assert!(!settings.is_open());
    }

    #[test]
    fn the_selection_wraps_both_ways() {
        let mut settings = Settings::new();
        assert_eq!(settings.selected(), Row::Log);
        settings.scroll(-1);
        assert_eq!(settings.selected(), Row::Sound);
        settings.scroll(1);
        assert_eq!(settings.selected(), Row::Log);
        settings.scroll(6);
        assert_eq!(settings.selected(), ROWS[6 % ROWS.len()]);
    }

    #[test]
    fn every_row_toggles_and_says_so() {
        let mut settings = Settings::new();
        assert_eq!(settings.toggle(Row::Log), Toggled::Done);
        assert!(!settings.log);
        assert_eq!(settings.value(Row::Log, true), "hidden");

        assert_eq!(settings.toggle(Row::Animations), Toggled::Done);
        assert!(!settings.animations);
        assert_eq!(settings.value(Row::Animations, true), "off");

        // Mute is the audio half's, so this row can only ask.
        assert_eq!(settings.toggle(Row::Sound), Toggled::Sound);
        assert_eq!(settings.value(Row::Sound, false), "muted");
        assert_eq!(settings.value(Row::Sound, true), "on");
    }

    #[test]
    fn toggling_a_row_moves_the_keyboard_to_it() {
        // A tap and a keypress have to leave the panel in the same state, or
        // the next arrow key jumps somewhere the thumb was not.
        let mut settings = Settings::new();
        let _ = settings.toggle(Row::Sound);
        assert_eq!(settings.selected(), Row::Sound);
        settings.scroll(1);
        assert_eq!(settings.selected(), Row::Log, "and carries on from there");
    }

    #[test]
    fn the_speed_cycles_through_every_pace() {
        let mut settings = Settings::new();
        let mut seen = Vec::new();
        for _ in 0..ROWS.len() * 2 {
            seen.push(settings.speed);
            let _ = settings.toggle(Row::Speed);
        }
        for speed in [Speed::Slow, Speed::Normal, Speed::Fast] {
            assert!(seen.contains(&speed), "{speed:?} is unreachable");
        }
    }

    #[test]
    fn faster_is_shorter_and_slower_is_longer() {
        assert!(Speed::Fast.scale() < Speed::Normal.scale());
        assert!(Speed::Slow.scale() > Speed::Normal.scale());
        for speed in [Speed::Slow, Speed::Normal, Speed::Fast] {
            assert!(speed.scale() > 0.0, "{speed:?} would stop time");
        }
    }

    #[test]
    fn every_row_has_a_label_a_value_and_a_reason() {
        let settings = Settings::new();
        for row in ROWS {
            assert!(!row.label().is_empty());
            assert!(!row.help().is_empty());
            assert!(!settings.value(row, true).is_empty());
            assert_eq!(index_of(row).map(|i| ROWS[i]), Some(row));
        }
    }
}
