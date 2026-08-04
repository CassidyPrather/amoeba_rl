//! The two places the title and post-mortem screens can send you, and the one
//! thing in the frontend that has to reach outside the window to do its job.
//!
//! Everything about a link that a screen needs — what it is called, how wide
//! that is — is plain data, so [`render`](crate::render) can lay the row out
//! and [`input`](crate::input) can hit-test it without either of them knowing
//! that a browser exists. Only [`open`] is platform-specific, and it is the
//! last thing that happens.

/// Somewhere to go that is not this game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Link {
    /// Where the game is published, and whatever else is up there with it.
    Home,
    /// The source, this file included.
    Source,
}

/// Both links, in the order the screens draw them left to right.
///
/// **This order is load-bearing on the web**: [`open`] sends the position in
/// this array across to `web/links.js`, whose own list has to match.
pub const LINKS: [Link; 2] = [Link::Home, Link::Source];

impl Link {
    /// What the screens draw.
    ///
    /// Short enough that both of them and the gap between fit on one row at
    /// the size the key hints are drawn at, which is the tightest the title
    /// screen ever gets. The full URLs would not, and a link nobody can read
    /// is worse than a name nobody can type.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "wirenook.net",
            Self::Source => "source on github",
        }
    }

    /// Where it goes.
    ///
    /// Absent from the web build, which sends an index across to js and lets
    /// js hold the strings — see [`open`]. The test below is what keeps the
    /// two lists saying the same thing.
    #[cfg(any(not(target_arch = "wasm32"), test))]
    #[must_use]
    pub const fn url(self) -> &'static str {
        match self {
            Self::Home => "https://wirenook.net/",
            Self::Source => "https://github.com/CassidyPrather/amoeba_rl",
        }
    }

    /// This link's place in [`LINKS`]. Only the web build has anything to say
    /// it to; the test below is what keeps it honest on every other target.
    #[cfg(any(target_arch = "wasm32", test))]
    #[must_use]
    const fn index(self) -> u32 {
        match self {
            Self::Home => 0,
            Self::Source => 1,
        }
    }
}

/// The gap between the two labels, in characters. Three spaces, as the key
/// hints separate their clauses.
pub const GAP: &str = "   ";

/// Both labels and the gap, as one string's worth of width.
#[must_use]
pub fn row_width_chars() -> usize {
    LINKS
        .iter()
        .map(|link| link.label().chars().count())
        .sum::<usize>()
        + GAP.chars().count() * (LINKS.len() - 1)
}

/// Hand `link` to whatever opens URLs out there.
///
/// Best effort, deliberately: a player has no use for the news that their
/// desktop has no URL handler, and nothing in the game depends on this
/// working. It fails by doing nothing.
#[cfg(not(target_arch = "wasm32"))]
pub fn open(link: Link) {
    use std::process::{Command, Stdio};

    // The one command per desktop that means "open this the way I would".
    // Windows' is a `cmd` builtin rather than a program, and it reads a lone
    // quoted first argument as the window title — hence the empty one, which
    // is the documented way to say the title is not what this is.
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "windows") {
        ("cmd", &["/C", "start", ""])
    } else if cfg!(target_os = "macos") {
        ("open", &[])
    } else {
        ("xdg-open", &[])
    };
    // Detached from our stdio: xdg-open is a shell script that can be chatty,
    // and this is a game with a window, not a terminal to print into.
    let _ = Command::new(program)
        .args(args)
        .arg(link.url())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// The browser's half of [`open`].
#[cfg(target_arch = "wasm32")]
mod web {
    // Registered by `web/links.js` as a miniquad plugin, the way `web/gl.js`
    // and `web/audio.js` register theirs. The URLs live over there and only
    // the position in [`super::LINKS`] crosses: a `u32` in, nothing out, no
    // pointer to get wrong and no string to marshal. That is what the `safe`
    // below is claiming, and it is the whole of the claim.
    //
    // gl.js stubs out imports it cannot find, so a stale `index.html` without
    // links.js still runs the game; the links just stop doing anything. It
    // also logs one line about this plugin having no version export on the
    // Rust side, which is true and costs nothing.
    #[expect(
        unsafe_code,
        reason = "an extern block is the only way to reach the browser at all"
    )]
    unsafe extern "C" {
        pub safe fn amoeba_open_link(which: u32);
    }
}

/// Hand `link` to the browser, in a new tab where the browser allows one.
///
/// See the native build's [`open`] — this one fails by doing nothing too.
#[cfg(target_arch = "wasm32")]
pub fn open(link: Link) {
    web::amoeba_open_link(link.index());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_link_is_somewhere_you_could_actually_go() {
        for link in LINKS {
            assert!(link.url().starts_with("https://"), "{link:?}");
            assert!(!link.label().is_empty(), "{link:?}");
        }
    }

    #[test]
    fn the_indices_are_the_positions_the_js_will_look_up() {
        // `open` sends these numbers to web/links.js, which indexes its own
        // list with them. If this ever disagrees with LINKS, the web build
        // sends people to the wrong place while the native one stays right.
        for (i, link) in LINKS.iter().enumerate() {
            assert_eq!(link.index() as usize, i, "{link:?}");
        }
    }

    #[test]
    fn the_js_sends_people_to_the_same_places_in_the_same_order() {
        // The other half of the check above, and the only thing standing
        // between an edit here and a web build that quietly points somewhere
        // else: nothing at build time reads one list against the other, and a
        // wasm build cannot call `url` at all to compare them at runtime.
        let js = include_str!("../web/links.js");
        let mut cursor = 0;
        for link in LINKS {
            let quoted = format!("\"{}\"", link.url());
            let at = js[cursor..].find(&quoted).map(|at| at + cursor);
            let at = at.unwrap_or_else(|| {
                panic!("web/links.js is missing {quoted}, or has it out of LINKS order")
            });
            cursor = at + quoted.len();
        }
    }

    #[test]
    fn the_row_is_narrower_than_the_key_hints_it_sits_under() {
        // The hints are what the title screen's bottom strip is sized for, so
        // staying under the shorter of them means the links never widen it.
        assert!(row_width_chars() <= 38, "{} chars", row_width_chars());
    }
}
