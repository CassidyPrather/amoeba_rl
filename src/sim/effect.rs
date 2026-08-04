//! What just happened, and *where*.
//!
//! This is the second of the sim's two report channels and the counterpart to
//! [`Cue`](super::Cue). A cue is a deduplicated set of what happened, which is
//! all a sound needs: twenty chloroplasts producing on the same turn want one
//! noise, not twenty. An [`Effect`] is the opposite shape — an ordered list,
//! one entry per thing that actually happened, each carrying the cells it
//! happened between — because a picture of an event has to be drawn somewhere
//! in particular, and a militia's blow has to be shown *before* the retreat it
//! caused.
//!
//! The order is causal: effects are appended as the turn resolves, so replaying
//! the list in order replays the turn. That is the whole contract. Nothing in
//! here knows how long an animation lasts or what colour it is, in the same way
//! that nothing in [`Cue`](super::Cue) knows what a cue sounds like.
//!
//! Every message the sim writes to the log has an effect beside it, so a player
//! who turns the log off has not turned the game's explanations off with it.

use super::grid::Coord;

/// One thing that happened, and the two cells it happened between.
///
/// The convention is the same for every kind: `to` is what it happened *to* and
/// `from` is what caused it. Anything that happens in one place sets both to
/// that place, which is what [`Effect::at`] is for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Effect {
    /// What happened.
    pub kind: EffectKind,
    /// Where it came from: whoever acted.
    pub from: Coord,
    /// Where it landed: whoever it happened to.
    pub to: Coord,
    /// Whether either of those cells was in sight when it happened.
    ///
    /// The list keeps the rest anyway, because "a gate somewhere out in the
    /// dark queued a wave" is real news that a frontend might want to say
    /// *something* about. But anything drawn on the map has to skip them, for
    /// the reason the map itself does not draw actors out of sight: a burst
    /// painted on unexplored black says where a gate is.
    pub in_sight: bool,
}

impl Effect {
    /// An effect between two cells, in sight.
    #[must_use]
    pub const fn new(kind: EffectKind, from: Coord, to: Coord) -> Self {
        Self {
            kind,
            from,
            to,
            in_sight: true,
        }
    }

    /// An effect that happened in one cell, in sight.
    #[must_use]
    pub const fn at(kind: EffectKind, at: Coord) -> Self {
        Self::new(kind, at, at)
    }

    /// The same thing, out in the dark.
    #[must_use]
    pub const fn unseen(mut self) -> Self {
        self.in_sight = false;
        self
    }

    /// Whether this happened in a single cell.
    #[must_use]
    pub fn is_local(&self) -> bool {
        self.from == self.to
    }
}

/// Everything the sim will show you.
///
/// One variant per *kind of interaction*, not per actor: a militia, a tank and
/// a stray shot all destroying a cytoplasm are one [`EffectKind::Destroyed`],
/// because they should all look like a cytoplasm being destroyed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectKind {
    /// Something walked from `from` to `to`. The mass flowing a cell reports one
    /// of these per organelle that shuffled up, so the whole drag is described.
    Flow,
    /// The occupants of `from` and `to` traded places.
    Swap,
    /// An action from `from` bounced off `to`: rock, armour, or too little mass.
    Refused,
    /// The human at `from` struck whatever of yours stood at `to`.
    Strike,
    /// The nucleus at `from` dodged into `to`, whose occupant takes the blow.
    Retreat,
    /// Whatever is at `from` stepped in and saved what is at `to`.
    Shielded,
    /// The organelle at `from` ate what was standing at `to`.
    Consume,
    /// The human at `to` was sealed in with no way out and captured.
    Engulf,
    /// A catalyst or crafting material at `from` was taken into `to`.
    Absorb,
    /// Whatever is at `to` finished changing into something else.
    Transform,
    /// The thing at `from` put something new into the world at `to`.
    Produce,
    /// Something fell out of `from` and landed at `to`.
    Drop,
    /// Something of yours at `to` was destroyed.
    Destroyed,
    /// A nucleus at `to` died. Separate from [`EffectKind::Destroyed`] because
    /// it is the only loss that can end the run.
    NucleusLost,
    /// The human at `to` was killed outright rather than eaten.
    Slain,
    /// The ranged human at `from` painted a line as far as `to`.
    Aim,
    /// It fired, from `from` down the line to `to`.
    Shot,
    /// The core at `from` frightened the human at `to` out of its turn.
    Terrify,
    /// The caravan at `from` reached the gate at `to` and left the map through
    /// it, taking its cargo with it.
    Depart,
    /// The mass at `to` sat out a stretch of turns nothing came of.
    Rest,
    /// The gate at `from` queued a fresh wave.
    Wave,
    /// The gate at `to` came down.
    GateFell,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_effect_says_so() {
        let here = Coord::new(3, 4);
        assert!(Effect::at(EffectKind::Engulf, here).is_local());
        assert!(
            !Effect::new(EffectKind::Shot, here, Coord::new(3, 9)).is_local(),
            "two cells apart is not local"
        );
    }

    #[test]
    fn the_cells_survive_construction() {
        let effect = Effect::new(EffectKind::Absorb, Coord::new(1, 2), Coord::new(3, 4));
        assert_eq!(effect.from, Coord::new(1, 2));
        assert_eq!(effect.to, Coord::new(3, 4));
        assert_eq!(effect.kind, EffectKind::Absorb);
        assert!(effect.in_sight, "an effect is visible unless it is not");
        assert!(!effect.unseen().in_sight);
    }
}
