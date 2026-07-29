//! The turn queue.
//!
//! Time is an integer that only ever moves forward, and everything that acts
//! sits in a bucket keyed by the instant it next acts. Popping takes the
//! lowest bucket and, within it, the earliest insertion — so a tie is broken
//! by who was scheduled first, which is what keeps a stack of nuclei from
//! drifting out of order.
//!
//! The original read the actor's mutable `Delay` field at insertion time and
//! several call sites temporarily rewrote that field to fake a different cost.
//! Here the cost is an explicit argument to [`Schedule::add`] instead, so the
//! caller says what it means.

use std::collections::BTreeMap;
use std::collections::VecDeque;

use super::actors::ActorId;

/// Time units in one game turn. The UI divides by this to show a turn count.
pub const TURN: u64 = 16;

/// A time-ordered queue of everything waiting to act.
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    time: u64,
    buckets: BTreeMap<u64, VecDeque<ActorId>>,
}

impl Schedule {
    /// An empty schedule at time zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The instant of the most recently popped entry.
    #[must_use]
    pub const fn time(&self) -> u64 {
        self.time
    }

    /// Queue `id` to act `cost` time units from now.
    pub(crate) fn add(&mut self, id: ActorId, cost: i32) {
        let key = self.time + u64::from(cost.max(0).unsigned_abs());
        self.buckets.entry(key).or_default().push_back(id);
    }

    /// Take the next entry due, advancing the clock to its instant.
    pub(crate) fn pop(&mut self) -> Option<ActorId> {
        let key = *self.buckets.keys().next()?;
        let bucket = self.buckets.get_mut(&key)?;
        let id = bucket.pop_front()?;
        if bucket.is_empty() {
            self.buckets.remove(&key);
        }
        self.time = key;
        Some(id)
    }

    /// Drop the earliest entry for `id`, if it has one.
    ///
    /// Only one: an actor can legitimately hold several entries, and the
    /// original's `Remove` deleted exactly one of them.
    pub(crate) fn remove(&mut self, id: ActorId) {
        let mut empty = None;
        for (key, bucket) in &mut self.buckets {
            if let Some(at) = bucket.iter().position(|&q| q == id) {
                bucket.remove(at);
                if bucket.is_empty() {
                    empty = Some(*key);
                }
                break;
            }
        }
        if let Some(key) = empty {
            self.buckets.remove(&key);
        }
    }

    /// When `id` next acts, if it is queued at all.
    pub(crate) fn scheduled_for(&self, id: ActorId) -> Option<u64> {
        self.buckets
            .iter()
            .find(|(_, bucket)| bucket.contains(&id))
            .map(|(key, _)| *key)
    }

    /// How many entries `id` holds. More than one means it will act more than
    /// once before its next proper turn, which should never happen — it is the
    /// exact corruption the terror core's fix is there to prevent.
    #[cfg(test)]
    pub(crate) fn entries_for(&self, id: ActorId) -> usize {
        self.buckets
            .values()
            .map(|bucket| bucket.iter().filter(|&&q| q == id).count())
            .sum()
    }

    /// The first actor holding more than one entry, if any does.
    ///
    /// Nothing should: two entries means two turns for the price of one, which
    /// is the exact corruption the terror core's fix is there to prevent.
    #[cfg(test)]
    pub(crate) fn duplicate_entry(&self) -> Option<ActorId> {
        let mut all: Vec<ActorId> = self.buckets.values().flatten().copied().collect();
        all.sort_unstable();
        all.windows(2)
            .find(|pair| pair[0] == pair[1])
            .map(|pair| pair[0])
    }

    /// Whether anything at all is queued.
    pub(crate) fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// Drop everything and rewind the clock, as the original did on win and
    /// loss. The UI keeps its own monotone turn counter for exactly this
    /// reason.
    pub(crate) fn clear(&mut self) {
        self.time = 0;
        self.buckets.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: usize) -> ActorId {
        ActorId::from_raw(n)
    }

    #[test]
    fn pops_in_time_order() {
        let mut s = Schedule::new();
        s.add(id(1), 16);
        s.add(id(2), 4);
        s.add(id(3), 8);
        assert_eq!(s.pop(), Some(id(2)));
        assert_eq!(s.time(), 4);
        assert_eq!(s.pop(), Some(id(3)));
        assert_eq!(s.time(), 8);
        assert_eq!(s.pop(), Some(id(1)));
        assert_eq!(s.time(), 16);
        assert_eq!(s.pop(), None);
    }

    #[test]
    fn ties_are_first_in_first_out() {
        let mut s = Schedule::new();
        s.add(id(1), 16);
        s.add(id(2), 16);
        s.add(id(3), 16);
        assert_eq!(s.pop(), Some(id(1)));
        assert_eq!(s.pop(), Some(id(2)));
        assert_eq!(s.pop(), Some(id(3)));
    }

    #[test]
    fn cost_is_relative_to_the_current_time() {
        let mut s = Schedule::new();
        s.add(id(1), 16);
        s.pop();
        assert_eq!(s.time(), 16);
        s.add(id(1), 16);
        assert_eq!(s.scheduled_for(id(1)), Some(32));
    }

    #[test]
    fn remove_takes_only_one_entry() {
        let mut s = Schedule::new();
        s.add(id(1), 8);
        s.add(id(1), 8);
        s.remove(id(1));
        assert_eq!(s.scheduled_for(id(1)), Some(8));
        s.remove(id(1));
        assert_eq!(s.scheduled_for(id(1)), None);
        assert!(s.is_empty());
    }

    #[test]
    fn remove_of_an_absent_entry_is_harmless() {
        let mut s = Schedule::new();
        s.add(id(1), 8);
        s.remove(id(2));
        assert_eq!(s.scheduled_for(id(1)), Some(8));
    }

    #[test]
    fn clear_rewinds_the_clock() {
        let mut s = Schedule::new();
        s.add(id(1), 48);
        s.pop();
        s.clear();
        assert_eq!(s.time(), 0);
        assert!(s.is_empty());
    }

    #[test]
    fn zero_cost_lands_on_the_current_instant() {
        let mut s = Schedule::new();
        s.add(id(1), 16);
        s.pop();
        s.add(id(2), 0);
        assert_eq!(s.scheduled_for(id(2)), Some(16));
    }
}
