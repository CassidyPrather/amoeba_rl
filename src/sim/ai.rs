//! What the humans do.
//!
//! Every enemy in the game is one of three behaviours. Most of them walk at
//! you and hit whatever they reach. Caravans do the opposite and run. Hunters
//! and scouts paint a line across the cavern, wait two turns, and then fire
//! down it. Gates do not move at all; they count down and open.
//!
//! Two details are worth keeping in mind while reading this. Sight is a
//! taxicab diamond with **no occlusion at all** — humans detect you through
//! solid rock, which the original chose deliberately for speed and which the
//! whole difficulty curve is balanced against. And a "shortest path" here is
//! found by one breadth-first sweep that stops as soon as it has reached the
//! nearest targets, rather than by one search per target; the set of nearest
//! targets that comes out is the same.

use std::collections::VecDeque;

use super::actors::{self, Actor, ActorId, Extra, Kind, Reticle};
use super::grid::{Coord, rand_inclusive, rand_index};
use super::{Cue, Sim};

/// Cells a maw can smell food from.
const HUNGER_RANGE: i32 = 2;

/// How close a tank has to be before a tentacle gives ground.
const TERROR_RADIUS: i32 = 1;

/// Wave cost of one scout.
const SCOUT_COST: i32 = 2;
/// Wave cost of one hunter.
const HUNTER_COST: i32 = 3;
/// Wave cost of one tank.
const TANK_COST: i32 = 2;
/// Wave cost of one mech.
const MECH_COST: i32 = 3;
/// Humans one "militia group" purchase puts in the queue at most.
const MILITIA_GROUP: i32 = 3;

impl Sim {
    // -- sensing ------------------------------------------------------------

    /// Everything matching `pred` within this actor's awareness.
    ///
    /// KEPT from C# (§11.2): this is a plain taxicab-diamond scan with no
    /// line-of-sight test, so humans notice you through solid rock. It was a
    /// deliberate performance trade in the original and every awareness number
    /// in the game was tuned against it.
    pub(crate) fn seen(&self, id: ActorId, pred: impl Fn(&Actor) -> bool) -> Vec<ActorId> {
        let Some(actor) = self.actors.get(id) else {
            return Vec::new();
        };
        self.grid
            .cells_in_diamond(actor.pos, actor.awareness)
            .filter_map(|c| self.actor_at(c))
            .filter(|&other| other != id && self.actors.get(other).is_some_and(&pred))
            .collect()
    }

    // -- pathing ------------------------------------------------------------

    /// Every shortest path from `from` to whichever of `targets` are nearest.
    ///
    /// A cell may be walked through when `passable` says so; a target cell is
    /// always reachable even when it is not, which is how the original's
    /// pathfinder temporarily forced its two endpoints walkable. One path comes
    /// back per tied-nearest target, mover first, so `path[1]` is the step to
    /// take.
    ///
    /// DELIBERATE CHANGE from C#: the original ran one Dijkstra per target and
    /// then kept the ties. One sweep that stops expanding once it is deeper
    /// than the nearest target found so far produces the same set of nearest
    /// targets for a fraction of the work, which matters because every human
    /// on the map does this every turn.
    pub(crate) fn shortest_paths_to(
        &self,
        from: Coord,
        targets: &[Coord],
        passable: impl Fn(&Self, Coord) -> bool,
    ) -> Vec<Vec<Coord>> {
        let width = self.grid.width();
        let Some(start) = self.cell_index(from) else {
            return Vec::new();
        };
        if targets.is_empty() {
            return Vec::new();
        }
        let cells = (width * self.grid.height()).unsigned_abs() as usize;
        let mut came = vec![usize::MAX; cells];
        let mut dist = vec![i32::MAX; cells];
        came[start] = start;
        dist[start] = 0;
        let mut queue = VecDeque::from([from]);
        let mut best = i32::MAX;
        while let Some(at) = queue.pop_front() {
            let Some(here) = self.cell_index(at) else {
                continue;
            };
            if dist[here] >= best {
                continue;
            }
            for next in self.grid.adjacent(at) {
                let Some(i) = self.cell_index(next) else {
                    continue;
                };
                if came[i] != usize::MAX {
                    continue;
                }
                let open = passable(self, next);
                let target = targets.contains(&next);
                if !open && !target {
                    continue;
                }
                came[i] = here;
                dist[i] = dist[here] + 1;
                if target {
                    best = best.min(dist[i]);
                }
                if open {
                    queue.push_back(next);
                }
            }
        }
        if best == i32::MAX {
            return Vec::new();
        }
        let mut out = Vec::new();
        for &target in targets {
            let Some(i) = self.cell_index(target) else {
                continue;
            };
            if dist[i] != best {
                continue;
            }
            let mut path = vec![target];
            let mut cursor = i;
            while cursor != start {
                cursor = came[cursor];
                #[allow(clippy::cast_possible_wrap)] // Cell indices fit an i32 by construction.
                path.push(Coord::new(cursor as i32 % width, cursor as i32 / width));
            }
            path.reverse();
            out.push(path);
        }
        out
    }

    /// Ring-by-ring outward search for the nearest actors matching `filter`.
    ///
    /// Returns every match on the first ring that holds one, the way the
    /// original's `NearestActors` did.
    pub(crate) fn nearest_actors(
        &self,
        from: Coord,
        filter: impl Fn(&Self, ActorId) -> bool,
    ) -> Vec<ActorId> {
        let cells = (self.grid.width() * self.grid.height()).unsigned_abs() as usize;
        let mut seen = vec![false; cells];
        if let Some(i) = self.cell_index(from) {
            seen[i] = true;
        }
        let mut ring = vec![from];
        loop {
            let found: Vec<ActorId> = ring
                .iter()
                .filter_map(|c| self.actor_at(*c))
                .filter(|&id| filter(self, id))
                .collect();
            if !found.is_empty() {
                return found;
            }
            let mut next = Vec::new();
            for c in ring {
                for n in self.grid.adjacent(c) {
                    if let Some(i) = self.cell_index(n)
                        && !seen[i]
                        && !self.is_wall(n)
                    {
                        seen[i] = true;
                        next.push(n);
                    }
                }
            }
            if next.is_empty() {
                return Vec::new();
            }
            ring = next;
        }
    }

    /// A path that may cut through unarmoured humans, which is how the gravity
    /// core reaches organelles standing behind a crowd.
    pub(crate) fn path_ignoring_militia(&self, from: Coord, to: Coord) -> Option<Vec<Coord>> {
        self.shortest_paths_to(from, &[to], |sim, c| {
            sim.grid.walkable(c)
                || sim.actor_at(c).is_some_and(|a| {
                    sim.actors.get(a).is_some_and(|x| {
                        actors::is_militia_family(x.kind) && !actors::is_tank_family(x.kind)
                    })
                })
        })
        .into_iter()
        .next()
    }

    /// Whether an organelle is standing here. Maws and tentacles travel only
    /// through their own body, and this is the test they use.
    pub(crate) fn body_at(&self, c: Coord) -> bool {
        self.actor_at(c).is_some_and(|a| {
            self.actors
                .get(a)
                .is_some_and(|x| actors::is_organelle(x.kind))
        })
    }

    // -- the basic human ----------------------------------------------------

    /// Militia, tanks and mechs: notice, approach, hit.
    ///
    /// The engulf check comes first, so a human that has just been sealed in is
    /// captured *instead of* taking its turn. That is the whole reason closing
    /// a pocket around somebody works even when it is their turn next.
    pub(crate) fn militia_act(&mut self, id: ActorId) {
        if self.engulf(id) || !self.actors.contains(id) {
            return;
        }
        let targets = self.seen(id, Actor::is_player_aligned);
        if targets.is_empty() {
            self.wander(id);
        } else {
            self.act_to_targets(id, &targets);
        }
    }

    /// Step one cell along a shortest path to the nearest thing of yours.
    fn act_to_targets(&mut self, id: ActorId, targets: &[ActorId]) {
        let Some(from) = self.actors.get(id).map(|a| a.pos) else {
            return;
        };
        let cells: Vec<Coord> = targets
            .iter()
            .filter_map(|t| self.actors.get(*t))
            .map(|a| a.pos)
            .collect();
        let paths = self.shortest_paths_to(from, &cells, |sim, c| sim.grid.walkable(c));
        let Some(pick) = rand_index(&mut self.rng, paths.len()) else {
            return;
        };
        if paths[pick].len() < 2 {
            let name = self.actors[id].name;
            self.messages.add(&format!(
                "The {name} contemplates the irrationality of its existence."
            ));
            return;
        }
        let step = paths[pick][1];
        self.attack_move_npc(id, step);
    }

    /// Wander one cell, or stand still. Standing still is as likely as any one
    /// direction, because the original's inclusive random ran one past the end
    /// of the list.
    fn wander(&mut self, id: ActorId) {
        let Some(from) = self.actors.get(id).map(|a| a.pos) else {
            return;
        };
        let adjacent: Vec<Coord> = self.grid.adjacent_walkable(from).collect();
        let count = i32::try_from(adjacent.len()).unwrap_or(i32::MAX);
        let pick = rand_inclusive(&mut self.rng, 0, count);
        if pick < count {
            #[allow(clippy::cast_sign_loss)] // Bounded by the branch above.
            let step = adjacent[pick as usize];
            self.attack_move_npc(id, step);
        }
    }

    /// A human's move: walk there, or hit whatever of yours is in the way.
    pub(crate) fn attack_move_npc(&mut self, monster: ActorId, cell: Coord) {
        if self.set_actor_position(monster, cell) {
            return;
        }
        if let Some(target) = self.actor_at(cell)
            && self.player_mass.contains(&target)
        {
            self.attack(monster, target);
        }
    }

    // -- the caravan --------------------------------------------------------

    /// A caravan wants to be anywhere you are not.
    pub(crate) fn caravan_act(&mut self, id: ActorId) {
        if self.engulf(id) || !self.actors.contains(id) {
            return;
        }
        let targets = self.seen(id, Actor::is_player_aligned);
        if targets.is_empty() {
            self.wander(id);
            return;
        }
        let sources: Vec<Coord> = targets
            .iter()
            .filter_map(|t| self.actors.get(*t))
            .map(|a| a.pos)
            .collect();
        let step = self.immediate_uphill_step(id, &sources, false);
        self.attack_move_npc(id, step);
    }

    /// One greedy step away from everything in `sources`, scored by the sum of
    /// taxicab distances.
    ///
    /// KEPT from C# (§11.7): the candidate lists are appended to on `>=` and
    /// never cleared when a strictly better cell turns up, so options that were
    /// beaten stay in the draw. It makes fleeing look panicked rather than
    /// optimal, which is how the original played.
    pub(crate) fn immediate_uphill_step(
        &mut self,
        id: ActorId,
        sources: &[Coord],
        can_pass_through_others: bool,
    ) -> Coord {
        let Some(from) = self.actors.get(id).map(|a| a.pos) else {
            return Coord::new(0, 0);
        };
        let score = |c: Coord| sources.iter().map(|s| c.taxi(*s)).sum::<i32>();
        let mut best_free = score(from);
        let mut free = vec![from];
        let mut best_sacrifice = i32::MIN;
        let mut sacrifices: Vec<Coord> = Vec::new();
        let neighbours: Vec<Coord> = self.grid.adjacent(from).collect();
        for cell in neighbours {
            let value = score(cell);
            if self.grid.walkable(cell) {
                if value >= best_free {
                    best_free = value;
                    free.push(cell);
                }
            } else if can_pass_through_others
                && let Some(other) = self.actor_at(cell)
                && !sources.contains(&cell)
                && !self
                    .actors
                    .get(other)
                    .is_some_and(|a| actors::is_city(a.kind))
                && value >= best_sacrifice
            {
                best_sacrifice = value;
                sacrifices.push(cell);
            }
        }
        if !sacrifices.is_empty()
            && (best_sacrifice > best_free
                || (best_sacrifice == best_free && rand_inclusive(&mut self.rng, 0, 1) == 0))
        {
            return self.pick(&sacrifices).unwrap_or(from);
        }
        self.pick(&free).unwrap_or(from)
    }

    // -- hunters and scouts -------------------------------------------------

    /// Aim, charge, fire — one step of that cycle per turn.
    pub(crate) fn ranged_act(&mut self, id: ActorId) {
        if self.engulf(id) || !self.actors.contains(id) {
            return;
        }
        let Extra::Ranged(ranged) = self.actors[id].extra else {
            return;
        };
        if ranged.firing <= 0 {
            self.fire(id);
        } else if ranged.firing < ranged.firing_time {
            if let Extra::Ranged(state) = &mut self.actors[id].extra {
                state.firing -= 1;
            }
        } else {
            let targets = self.seen(id, Actor::is_player_aligned);
            if targets.is_empty() {
                self.wander(id);
            } else {
                self.ranged_act_to_targets(id, &targets);
            }
        }
    }

    /// Look for a target on the same row or column within range; if there is
    /// one, paint the line, otherwise close in like a militiaman.
    fn ranged_act_to_targets(&mut self, id: ActorId, targets: &[ActorId]) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let from = actor.pos;
        let Extra::Ranged(ranged) = actor.extra else {
            return;
        };
        let cells: Vec<Coord> = targets
            .iter()
            .filter_map(|t| self.actors.get(*t))
            .map(|a| a.pos)
            .collect();
        let mut paths = self.shortest_paths_to(from, &cells, |sim, c| sim.grid.walkable(c));
        let mut sighted = None;
        while let Some(pick) = rand_index(&mut self.rng, paths.len()) {
            let path = paths.remove(pick);
            let Some(&target) = path.last() else {
                continue;
            };
            // Path length counts cells including the start, so the step count
            // is one less: this is the original's `Length <= Range + 1`.
            let steps = i32::try_from(path.len()).unwrap_or(i32::MAX) - 1;
            if steps <= ranged.range && (target.x == from.x || target.y == from.y) {
                sighted = Some(target);
                break;
            }
        }
        if let Some(target) = sighted {
            self.aim(id, target);
        } else {
            self.act_to_targets(id, targets);
        }
    }

    /// Paint reticles down the line toward `target` until rock stops them.
    ///
    /// Bodies do not stop the line: a cell with somebody standing on it is not
    /// a wall, which is why a hunter's shot goes straight through your mass.
    fn aim(&mut self, id: ActorId, target: Coord) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let from = actor.pos;
        let Extra::Ranged(ranged) = actor.extra else {
            return;
        };
        let mut sights = from;
        if target.x > from.x {
            sights.x += 1;
        } else if target.x < from.x {
            sights.x -= 1;
        } else if target.y > from.y {
            sights.y += 1;
        } else {
            sights.y -= 1;
        }
        let direction = sights - from;
        let mut bullet = sights;
        let mut travelled = 0;
        while self.grid.in_bounds(bullet) && !self.is_wall(bullet) && travelled < ranged.range {
            travelled += 1;
            self.reticles.push(Reticle {
                pos: bullet,
                owner: id,
            });
            bullet = bullet + direction;
        }
        if let Extra::Ranged(state) = &mut self.actors[id].extra {
            state.direction = direction;
            state.firing -= 1;
        }
        self.cue(Cue::Aim);
    }

    /// Fire down the painted line.
    ///
    /// Everything of yours under a reticle is *unslimed* rather than destroyed,
    /// so it drops the whole of what it was made of. A nucleus gets its retreat
    /// first, and whatever it retreated into takes the shot instead.
    fn fire(&mut self, id: ActorId) {
        let name = self.actors[id].name;
        if let Extra::Ranged(state) = &mut self.actors[id].extra {
            state.firing = state.firing_time;
        }
        let line: Vec<Coord> = self
            .reticles
            .iter()
            .filter(|r| r.owner == id)
            .map(|r| r.pos)
            .collect();
        let mut hits = 0;
        for cell in line {
            let Some(hit) = self.actor_at(cell) else {
                continue;
            };
            let Some(kind) = self.actors.get(hit).map(|a| a.kind) else {
                continue;
            };
            hits += 1;
            if actors::is_nucleus_family(kind) {
                let victim = self.retreat(hit).unwrap_or(hit);
                self.unslime_organelle(victim);
            } else if actors::is_organelle(kind) {
                self.unslime_organelle(hit);
                self.cue(Cue::OrganelleLost);
            } else if actors::is_npc(kind) {
                // Friendly fire is real: a hunter will shoot straight through
                // its own side.
                self.kill_npc(hit);
            }
        }
        if hits > 0 {
            self.messages.add(&format!("The {name} hit {hits} mass."));
        }
        self.reticles.retain(|r| r.owner != id);
        self.cue(Cue::Shot);
    }

    // -- gates --------------------------------------------------------------

    /// A gate counts down, queues a wave, and lets one human out per turn.
    ///
    /// Because only one leaves per turn, a gate whose single doorway is blocked
    /// backs up indefinitely — standing in a doorway is a real tactic.
    pub(crate) fn city_act(&mut self, id: ActorId) {
        let Extra::City(state) = &mut self.actors[id].extra else {
            return;
        };
        state.turns_to_next_wave -= 1;
        let due = state.turns_to_next_wave <= 0;
        let level = state.level;
        let countdown = state.turns_to_next_wave;
        if due {
            let budget = self.rules.max_budget.min(level);
            self.spawn_next_wave(id, budget);
            let evolution = self.rules.evolution_rate.max(1);
            if let Extra::City(state) = &mut self.actors[id].extra {
                // The wave number has already been incremented, which is what
                // makes the first wave the only one at level one.
                state.level = state.wave_number / evolution + 2;
            }
        } else if countdown < 10 {
            self.cue(Cue::GateCountdown);
        }
        let waiting = matches!(&self.actors[id].extra, Extra::City(s) if !s.queue.is_empty());
        if !waiting {
            return;
        }
        let pos = self.actors[id].pos;
        let Some(door) = self.grid.adjacent_walkable(pos).next() else {
            return;
        };
        let Extra::City(state) = &mut self.actors[id].extra else {
            return;
        };
        let Some(baby) = state.queue.pop_front() else {
            return;
        };
        self.add_actor(baby, door);
    }

    /// Fill the queue with one wave's worth of humans.
    fn spawn_next_wave(&mut self, id: ActorId, budget: i32) {
        let mut queued: Vec<Kind> = Vec::new();
        let mut stock = budget;
        while stock > 0 {
            stock = self.add_new_militia(stock, &mut queued);
        }
        let Some(wave) = (match &self.actors[id].extra {
            Extra::City(state) => Some(state.wave_number),
            _ => None,
        }) else {
            return;
        };
        // Caravans are common at the very start and rare afterwards. The rolls
        // are inclusive, so this is one in four, then three in twenty, then one
        // in twenty.
        let has_caravan = if wave == 0 {
            rand_inclusive(&mut self.rng, 0, 3) == 0
        } else if wave < 4 {
            rand_inclusive(&mut self.rng, 0, 19) <= 2
        } else {
            rand_inclusive(&mut self.rng, 0, 19) == 0
        };
        if has_caravan {
            queued.push(Kind::Caravan);
        }
        let rate = self.rules.spawn_rate;
        if let Extra::City(state) = &mut self.actors[id].extra {
            state.queue.extend(queued);
            state.wave_number += 1;
            state.turns_to_next_wave += rate;
        }
        self.cue(Cue::WaveSpawned);
    }

    /// Spend part of a wave's budget, and report what is left.
    ///
    /// Tanks and scouts can only ever be bought at exactly two, and mechs and
    /// hunters only at three or more, because the cheaper option is dropped
    /// from the draw as soon as the expensive one is affordable.
    fn add_new_militia(&mut self, budget: i32, queued: &mut Vec<Kind>) -> i32 {
        let mut allowed = vec![0_u8];
        if budget >= MECH_COST {
            allowed.push(1);
        } else if budget >= TANK_COST {
            allowed.push(2);
        }
        if budget >= HUNTER_COST {
            allowed.push(3);
        } else if budget >= SCOUT_COST {
            allowed.push(4);
        }
        match self.pick(&allowed).unwrap_or(0) {
            1 => {
                queued.push(Kind::Mech);
                budget - MECH_COST
            }
            2 => {
                queued.push(Kind::Tank);
                budget - TANK_COST
            }
            3 => {
                queued.push(Kind::Hunter);
                budget - HUNTER_COST
            }
            4 => {
                queued.push(Kind::Scout);
                budget - SCOUT_COST
            }
            _ => {
                let group = budget.min(MILITIA_GROUP);
                for _ in 0..group {
                    queued.push(Kind::Militia);
                }
                budget - group
            }
        }
    }

    // -- maws and tentacles -------------------------------------------------

    /// Whether the entity on this cell is something a maw would bite.
    fn hungry_for(&self, maw: Kind, cell: Coord) -> bool {
        if let Some(id) = self.actor_at(cell)
            && let Some(actor) = self.actors.get(id)
        {
            let unarmoured =
                actors::is_militia_family(actor.kind) && !actors::is_tank_family(actor.kind);
            // A reinforced maw is the one thing in your body that bites armour.
            if unarmoured || (maw == Kind::ReinforcedMaw && actors::is_tank_family(actor.kind)) {
                return true;
            }
        }
        self.item_at(cell).is_some()
    }

    /// A maw smells two cells in every direction and crawls at whatever it
    /// finds, travelling only through your own body.
    pub(crate) fn maw_act(&mut self, id: ActorId) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let (kind, pos) = (actor.kind, actor.pos);
        let desire: Vec<Coord> = self
            .grid
            .cells_in_diamond(pos, HUNGER_RANGE)
            .filter(|c| self.hungry_for(kind, *c))
            .collect();
        let Some(nearest) = desire.iter().map(|c| c.taxi(pos)).min() else {
            return;
        };
        let targets: Vec<Coord> = desire
            .into_iter()
            .filter(|c| c.taxi(pos) == nearest)
            .collect();
        self.advance_toward(id, &targets);
    }

    /// A tentacle hunts anything human except caravans, and steps away from
    /// tanks it cannot chew.
    pub(crate) fn tentacle_act(&mut self, id: ActorId) {
        let mut targets = self.seen(id, |a| {
            actors::is_militia_family(a.kind) && !actors::is_caravan(a.kind)
        });
        let Some(pos) = self.actors.get(id).map(|a| a.pos) else {
            return;
        };
        let tanks: Vec<ActorId> = targets
            .iter()
            .copied()
            .filter(|t| {
                self.actors
                    .get(*t)
                    .is_some_and(|a| actors::is_tank_family(a.kind))
            })
            .collect();
        let mut brave = true;
        if !tanks.is_empty() {
            let sources: Vec<Coord> = tanks
                .iter()
                .filter_map(|t| self.actors.get(*t))
                .map(|a| a.pos)
                .collect();
            if sources.iter().any(|s| s.taxi(pos) <= TERROR_RADIUS) {
                let step = self.immediate_uphill_step(id, &sources, true);
                if step != pos {
                    self.attack_move(id, step);
                    brave = false;
                }
            }
            if brave {
                targets.retain(|t| !tanks.contains(t));
            }
        }
        if targets.is_empty() || !brave {
            return;
        }
        let cells: Vec<Coord> = targets
            .iter()
            .filter_map(|t| self.actors.get(*t))
            .map(|a| a.pos)
            .collect();
        self.advance_toward(id, &cells);
    }

    /// One step of a maw's or tentacle's crawl, through your own body only.
    fn advance_toward(&mut self, id: ActorId, targets: &[Coord]) {
        let Some(pos) = self.actors.get(id).map(|a| a.pos) else {
            return;
        };
        let paths = self.shortest_paths_to(pos, targets, Self::body_at);
        let Some(pick) = rand_index(&mut self.rng, paths.len()) else {
            return;
        };
        if paths[pick].len() < 2 {
            return;
        }
        let next = paths[pick][1];
        let kind = self.actors[id].kind;
        if self.hungry_for(kind, next) || self.body_at(next) {
            self.attack_move(id, next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Difficulty;
    use crate::sim::tests::sandbox;

    #[test]
    fn sight_is_a_diamond_that_ignores_walls() {
        let mut sim = sandbox(1);
        let militia = sim.add_actor(Kind::Militia, Coord::new(5, 5));
        let far = sim.add_actor(Kind::Cytoplasm, Coord::new(9, 5));
        let near = sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        // Solid rock in between changes nothing: this is the deliberate
        // wallhack the original shipped with.
        sim.grid.set_props(Coord::new(6, 5), false, false, true);
        let seen = sim.seen(militia, Actor::is_player_aligned);
        assert!(seen.contains(&near));
        assert!(!seen.contains(&far), "awareness 3 stops at three cells");
    }

    #[test]
    fn a_militia_walks_toward_you_and_then_hits() {
        let mut sim = sandbox(2);
        let militia = sim.add_actor(Kind::Militia, Coord::new(5, 5));
        let cyto = sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.militia_act(militia);
        assert_eq!(sim.actors[militia].pos, Coord::new(6, 5), "it closed in");
        sim.militia_act(militia);
        assert!(!sim.actors.contains(cyto), "and then destroyed the cell");
    }

    #[test]
    fn a_militia_with_nothing_in_sight_wanders() {
        let mut sim = sandbox(3);
        let militia = sim.add_actor(Kind::Militia, Coord::new(5, 5));
        let mut moved = 0;
        for _ in 0..40 {
            let before = sim.actors[militia].pos;
            sim.militia_act(militia);
            if sim.actors[militia].pos != before {
                moved += 1;
            }
        }
        assert!(moved > 10, "wandering moves most turns");
        assert!(moved < 40, "and stands still sometimes");
    }

    #[test]
    fn a_caravan_runs_away() {
        let mut sim = sandbox(4);
        let caravan = sim.add_actor(Kind::Caravan, Coord::new(5, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(3, 5));
        for _ in 0..4 {
            sim.caravan_act(caravan);
        }
        assert!(
            sim.actors[caravan].pos.x > 5,
            "it put distance between us: {:?}",
            sim.actors[caravan].pos
        );
    }

    #[test]
    fn an_engulfed_human_does_not_get_to_act() {
        let mut sim = sandbox(5);
        // A pocket at (1,1) sealed by the arena walls and one cytoplasm.
        let militia = sim.add_actor(Kind::Militia, Coord::new(1, 1));
        sim.add_actor(Kind::Cytoplasm, Coord::new(2, 1));
        sim.add_actor(Kind::Cytoplasm, Coord::new(1, 2));
        sim.militia_act(militia);
        assert!(!sim.actors.contains(militia));
        let corpse = sim.actor_at(Coord::new(1, 1)).expect("a corpse");
        assert_eq!(sim.actors[corpse].kind, Kind::DissolvingMilitia);
    }

    #[test]
    fn a_hunter_paints_a_line_then_fires_down_it() {
        let mut sim = sandbox(6);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        let victim = sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        // Turn one: aim.
        sim.ranged_act(hunter);
        assert!(!sim.reticles.is_empty(), "the line is painted");
        assert!(sim.reticles.iter().any(|r| r.pos == Coord::new(7, 5)));
        let Extra::Ranged(state) = sim.actors[hunter].extra else {
            panic!("a hunter aims")
        };
        assert_eq!(state.firing, 1);
        assert_eq!(state.direction, Coord::new(1, 0));
        // Turn two: charge.
        sim.ranged_act(hunter);
        let Extra::Ranged(state) = sim.actors[hunter].extra else {
            panic!("a hunter charges")
        };
        assert_eq!(state.firing, 0);
        assert!(sim.actors.contains(victim));
        // Turn three: fire.
        sim.ranged_act(hunter);
        assert!(!sim.actors.contains(victim));
        assert!(sim.reticles.is_empty(), "the line is cleared");
        let Extra::Ranged(state) = sim.actors[hunter].extra else {
            panic!("a hunter reloads")
        };
        assert_eq!(state.firing, state.firing_time);
    }

    #[test]
    fn a_shot_unslimes_rather_than_destroys() {
        let mut sim = sandbox(7);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        sim.add_actor(Kind::QuantumCore, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        for _ in 0..3 {
            sim.ranged_act(hunter);
        }
        let dropped = (0..20)
            .flat_map(|y| (0..20).map(move |x| Coord::new(x, y)))
            .filter_map(|c| sim.item_at(c))
            .count();
        assert_eq!(
            dropped,
            Kind::QuantumCore.components().len(),
            "a shot drops the whole organelle"
        );
    }

    #[test]
    fn a_shot_nucleus_retreats_and_the_sacrifice_takes_it() {
        let mut sim = sandbox(30);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(7, 5));
        let shield = sim.add_actor(Kind::Cytoplasm, Coord::new(7, 6));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        for _ in 0..3 {
            sim.ranged_act(hunter);
        }
        assert!(sim.actors.contains(nucleus), "the nucleus ducked");
        assert_eq!(sim.actors[nucleus].pos, Coord::new(7, 6));
        assert!(!sim.actors.contains(shield), "the cytoplasm did not");
        assert_eq!(sim.phase(), crate::sim::Phase::Playing);
    }

    #[test]
    fn wave_draws_are_reproducible() {
        let draw = |seed: u64| {
            let mut sim = sandbox(seed);
            sim.grid.set_props(Coord::new(6, 5), false, false, true);
            let city = sim.add_actor(Kind::City, Coord::new(6, 5));
            let mut out = Vec::new();
            for _ in 0..8 {
                if let Extra::City(state) = &mut sim.actors[city].extra {
                    state.turns_to_next_wave = 1;
                }
                sim.city_act(city);
                if let Extra::City(state) = &sim.actors[city].extra {
                    out.push(state.queue.iter().copied().collect::<Vec<Kind>>());
                }
            }
            out
        };
        assert_eq!(draw(41), draw(41), "the same seed draws the same wave");
        assert_ne!(draw(41), draw(42), "a different one does not");
    }

    #[test]
    fn a_shot_stops_at_rock_but_not_at_bodies() {
        let mut sim = sandbox(8);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.grid.set_props(Coord::new(10, 5), false, false, true);
        sim.ranged_act(hunter);
        let painted: Vec<i32> = sim.reticles.iter().map(|r| r.pos.x).collect();
        assert!(painted.contains(&7), "the beam passes through a body");
        assert!(painted.contains(&9));
        assert!(!painted.contains(&10), "and stops at rock");
    }

    #[test]
    fn a_scout_out_of_line_closes_in_instead() {
        let mut sim = sandbox(9);
        let scout = sim.add_actor(Kind::Scout, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(7, 7));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.ranged_act(scout);
        assert!(sim.reticles.is_empty(), "nothing to line up on");
        assert_ne!(sim.actors[scout].pos, Coord::new(5, 5), "so it walks");
    }

    #[test]
    fn a_gate_queues_its_first_wave_on_turn_fifty() {
        for (difficulty, rate) in [
            (Difficulty::Normal, 50),
            (Difficulty::Easy, 75),
            (Difficulty::Gj, 50),
        ] {
            let mut sim = sandbox(10);
            sim.rules = difficulty.rules();
            sim.grid.set_props(Coord::new(6, 5), false, false, true);
            let city = sim.add_actor(Kind::City, Coord::new(6, 5));
            for turn in 1..50 {
                sim.city_act(city);
                let Extra::City(state) = &sim.actors[city].extra else {
                    panic!("a gate")
                };
                assert_eq!(state.wave_number, 0, "{difficulty:?} turn {turn}");
            }
            sim.city_act(city);
            let Extra::City(state) = &sim.actors[city].extra else {
                panic!("a gate")
            };
            assert_eq!(state.wave_number, 1, "{difficulty:?} first wave");
            assert_eq!(state.turns_to_next_wave, rate, "{difficulty:?} next wave");
        }
    }

    #[test]
    fn wave_budgets_climb_with_the_difficulty_step() {
        let mut sim = sandbox(11);
        sim.grid.set_props(Coord::new(6, 5), false, false, true);
        let city = sim.add_actor(Kind::City, Coord::new(6, 5));
        let mut levels = Vec::new();
        for _ in 0..20 {
            // Force the wave immediately rather than waiting fifty turns.
            if let Extra::City(state) = &mut sim.actors[city].extra {
                state.turns_to_next_wave = 1;
            }
            let level = match &sim.actors[city].extra {
                Extra::City(state) => state.level,
                _ => unreachable!(),
            };
            levels.push(sim.rules.max_budget.min(level));
            sim.city_act(city);
        }
        // Level one for the first wave, then two for waves one to five, then
        // three from wave six — the spec's table, read straight off.
        assert_eq!(levels[0], 1);
        assert_eq!(&levels[1..6], &[2, 2, 2, 2, 2]);
        assert_eq!(&levels[6..12], &[3, 3, 3, 3, 3, 3]);
        assert_eq!(levels[12], 4);
        assert_eq!(*levels.last().unwrap(), 5, "capped at the maximum budget");
    }

    #[test]
    fn the_spawn_table_only_offers_what_the_budget_allows() {
        let mut sim = sandbox(12);
        let mut seen = [false; 6];
        for _ in 0..200 {
            let mut queued = Vec::new();
            sim.add_new_militia(2, &mut queued);
            for kind in queued {
                match kind {
                    Kind::Militia => seen[0] = true,
                    Kind::Tank => seen[1] = true,
                    Kind::Scout => seen[2] = true,
                    Kind::Mech => seen[3] = true,
                    Kind::Hunter => seen[4] = true,
                    _ => seen[5] = true,
                }
            }
        }
        assert!(seen[0] && seen[1] && seen[2], "budget two buys these three");
        assert!(!seen[3] && !seen[4], "and never a mech or a hunter");
        let mut budgets = Vec::new();
        for _ in 0..200 {
            let mut queued = Vec::new();
            let left = sim.add_new_militia(5, &mut queued);
            budgets.push((queued, left));
        }
        for (queued, left) in budgets {
            match queued.first() {
                Some(Kind::Militia) => assert_eq!((queued.len(), left), (3, 2)),
                Some(Kind::Mech | Kind::Hunter) => assert_eq!((queued.len(), left), (1, 2)),
                other => panic!("budget five bought a {other:?}"),
            }
        }
    }

    #[test]
    fn a_gate_lets_one_human_out_per_turn() {
        let mut sim = sandbox(13);
        sim.grid.set_props(Coord::new(6, 5), false, false, true);
        let city = sim.add_actor(Kind::City, Coord::new(6, 5));
        if let Extra::City(state) = &mut sim.actors[city].extra {
            state.queue.push_back(Kind::Militia);
            state.queue.push_back(Kind::Militia);
            state.queue.push_back(Kind::Militia);
        }
        sim.city_act(city);
        assert_eq!(sim.actors.len(), 2, "one gate and one human");
        sim.city_act(city);
        assert_eq!(sim.actors.len(), 3);
    }

    #[test]
    fn a_blocked_doorway_backs_the_queue_up() {
        let mut sim = sandbox(14);
        sim.grid.set_props(Coord::new(6, 5), false, false, true);
        // Wall the gate in on every side but one, then plug that one.
        for cell in [Coord::new(6, 4), Coord::new(6, 6), Coord::new(7, 5)] {
            sim.grid.set_props(cell, false, false, true);
        }
        let city = sim.add_actor(Kind::City, Coord::new(6, 5));
        sim.add_actor(Kind::Membrane, Coord::new(5, 5));
        if let Extra::City(state) = &mut sim.actors[city].extra {
            state.queue.push_back(Kind::Militia);
        }
        sim.city_act(city);
        let queued = match &sim.actors[city].extra {
            Extra::City(state) => state.queue.len(),
            _ => unreachable!(),
        };
        assert_eq!(queued, 1, "nobody can get out");
    }

    #[test]
    fn a_maw_crawls_through_your_body_toward_food() {
        let mut sim = sandbox(15);
        let maw = sim.add_actor(Kind::Maw, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        let militia = sim.add_actor(Kind::Militia, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.maw_act(maw);
        assert_eq!(sim.actors[maw].pos, Coord::new(6, 5), "it swaps forward");
        sim.maw_act(maw);
        assert!(!sim.actors.contains(militia), "and then eats");
    }

    #[test]
    fn a_plain_maw_leaves_armour_alone() {
        let mut sim = sandbox(16);
        let maw = sim.add_actor(Kind::Maw, Coord::new(5, 5));
        let tank = sim.add_actor(Kind::Tank, Coord::new(6, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.maw_act(maw);
        assert!(sim.actors.contains(tank));
        assert_eq!(sim.actors[maw].pos, Coord::new(5, 5));
        // A reinforced one is hungry for exactly that.
        sim.actors[maw].kind = Kind::ReinforcedMaw;
        sim.maw_act(maw);
        assert!(!sim.actors.contains(tank));
    }

    #[test]
    fn a_tentacle_backs_away_from_an_adjacent_tank() {
        let mut gave_ground = 0;
        for seed in 0..24 {
            let mut sim = sandbox(seed);
            let tentacle = sim.add_actor(Kind::Tentacle, Coord::new(5, 5));
            let tank = sim.add_actor(Kind::Tank, Coord::new(6, 5));
            sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
            sim.tentacle_act(tentacle);
            let now = sim.actors[tentacle].pos;
            assert!(sim.actors.contains(tank), "seed {seed}: it did not attack");
            if now == Coord::new(5, 5) {
                continue;
            }
            gave_ground += 1;
            assert_eq!(
                now.taxi(Coord::new(6, 5)),
                2,
                "seed {seed}: it stepped away"
            );
        }
        // KEPT (§11.7): standing still stays in the draw even once a better
        // cell has been found, so retreating is usual rather than certain.
        assert!(gave_ground > 8, "it usually backs off: {gave_ground}/24");
        assert!(gave_ground < 24, "and sometimes freezes");
    }
}
