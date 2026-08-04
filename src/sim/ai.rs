//! What the humans do.
//!
//! Every human's turn has the same two halves, in this order: **do what you
//! said you would, then say what you will do next.** [`Sim::human_act`] is that
//! sentence and nothing else. Everything a human does was decided at the end of
//! its previous turn and has been drawn over its head ever since, so by the
//! time it happens the player has had a whole turn to answer it — and answering
//! by stepping aside really works, because a blow committed to a cell lands on
//! that cell whatever is standing there when it arrives.
//!
//! DELIBERATE CHANGE from C#, and the largest one in this file. The original
//! decided and acted in the same instant, so nothing a human did could be seen
//! coming; and with nothing in sight it took a random step, so a cavern of idle
//! humans was a cavern of drunks. Both are gone. What replaces the second is
//! [`Duty`](actors::Duty): a caravan is going to the far gate and will leave
//! through it with everything it is carrying if nobody stops it, its escort
//! goes wherever it goes, scouts walk the chambers furthest from the gates,
//! hunters walk down [rumors](super::Rumor), and everyone else holds a beat
//! outside the gate they came from. Humans out of sight are still *going
//! somewhere*, and where they are going is a thing the player can learn.
//!
//! One detail of the machinery is worth knowing before reading further: a
//! "shortest path" here is found by one breadth-first sweep that stops as soon
//! as it has reached the nearest targets, rather than by one search per target.
//! The set of nearest targets that comes out is the same.

use std::collections::VecDeque;

use super::actors::{self, Actor, ActorId, Duty, Extra, Intent, IntentKind, Kind, Reticle};
use super::effect::EffectKind;
use super::grid::{Coord, rand_inclusive, rand_index};
use super::schedule::TURN;
use super::{Cue, Rumor, Sim};

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

/// Turns a rumor is worth walking to before the humans stop believing it.
const RUMOR_LIFE: u64 = 40;
/// How far apart two reports have to be to count as two rumors rather than one
/// that moved.
const RUMOR_MERGE: i32 = 6;
/// Rumors kept at once. Older ones are forgotten to make room.
const MAX_RUMORS: usize = 8;
/// Close enough to count as having arrived somewhere. Exact arrival is not
/// something a walk through a crowd can promise.
const ARRIVED: i32 = 2;
/// How close an escort keeps to its caravan before it stops closing.
const ESCORT_RANGE: i32 = 2;
/// Soldiers a gate assigns to a caravan it has just sent out.
const ESCORT_SIZE: i32 = 3;
/// How far a caravan's destination gate has to be for the trip to be worth
/// making.
const CONVOY_MIN: i32 = 12;
/// Gates a caravan chooses its destination from, furthest first.
const CONVOY_CHOICES: usize = 4;

impl Sim {
    // -- sensing ------------------------------------------------------------

    /// Everything matching `pred` this actor can actually see: inside its
    /// awareness, and with rock out of the way.
    ///
    /// DELIBERATE CHANGE from C# (§11.2). The original scanned a taxicab
    /// diamond with no line-of-sight test at all, so every human in the game
    /// detected the amoeba through solid rock. It was a deliberate performance
    /// trade — `Seen` ran for every human every turn on a 2008-era engine — and
    /// the whole difficulty curve was balanced against it.
    ///
    /// The constraint behind that trade is long gone — the cost is one Bresenham
    /// walk per cell of a diamond a few cells across, per human, per turn, which
    /// is nothing on any machine this will ever run on — and what it bought was
    /// never worth having. Cover that does not conceal is scenery.
    ///
    /// What the wallhack was doing for the humans, [rumors](super::Rumor) do
    /// instead, and better: lose sight of the amoeba and you walk to where it
    /// was, and so does everybody the report reaches.
    pub(crate) fn seen(&self, id: ActorId, pred: impl Fn(&Actor) -> bool) -> Vec<ActorId> {
        let Some(actor) = self.actors.get(id) else {
            return Vec::new();
        };
        self.grid
            .cells_in_diamond(actor.pos, actor.awareness)
            .filter(|c| self.grid.line_of_sight(actor.pos, *c))
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

    // -- a human's turn ------------------------------------------------------

    /// Do what you said you would, then say what you will do next.
    ///
    /// The engulf check comes first, so a human that has just been sealed in is
    /// captured *instead of* taking its turn. That is the whole reason closing
    /// a pocket around somebody works even when it is their turn next.
    ///
    /// A human that has only just stepped out of a gate has nothing to carry
    /// out yet, so its first turn is spent arriving and deciding. That is
    /// deliberate: the first thing a new human does is stand in the doorway
    /// showing you what it is about to do.
    pub(crate) fn human_act(&mut self, id: ActorId) {
        if self.engulf(id) || !self.actors.contains(id) {
            return;
        }
        self.carry_out(id);
        if self.actors.contains(id) {
            self.form_intent(id);
        }
    }

    /// Carry out whatever this human committed to last turn.
    ///
    /// The commitment is honoured as written. A step onto a cell that has since
    /// emptied is still a step onto that cell; a blow at a cell your nucleus
    /// has left lands on the floor. That is the counterplay the telegraph is
    /// for, and it is only counterplay if the promise is kept.
    fn carry_out(&mut self, id: ActorId) {
        let Some(intent) = self.actors.get(id).and_then(|a| a.intent) else {
            return;
        };
        if let Some(actor) = self.actors.get_mut(id) {
            actor.intent = None;
        }
        match intent.kind {
            IntentKind::Step | IntentKind::Strike => self.attack_move_npc(id, intent.at),
            IntentKind::Aim => self.aim(id, intent.at),
            IntentKind::Fire => self.discharge(id),
            IntentKind::Depart => self.depart(id, intent.at),
            IntentKind::Hold => {}
        }
    }

    /// Decide what this human will do next, and commit to it.
    fn form_intent(&mut self, id: ActorId) {
        let intent = self.choose_intent(id);
        if let Some(actor) = self.actors.get_mut(id) {
            actor.intent = intent;
        }
    }

    /// The decision itself: a gun mid-cycle finishes its shot, anybody who can
    /// see you answers that, and everybody else does their job.
    fn choose_intent(&mut self, id: ActorId) -> Option<Intent> {
        let actor = self.actors.get(id)?;
        let (kind, from) = (actor.kind, actor.pos);
        // A painted line is a decision already taken. Nothing a gunman can see
        // in the meantime talks it out of firing down the line it has drawn.
        if let Extra::Ranged(ranged) = actor.extra
            && ranged.firing < ranged.firing_time
        {
            let end = self
                .reticles
                .iter()
                .filter(|r| r.owner == id)
                .map(|r| r.pos)
                .next_back()
                .unwrap_or(from);
            return Some(Intent {
                kind: IntentKind::Fire,
                at: end,
            });
        }
        let targets = self.seen(id, Actor::is_player_aligned);
        if targets.is_empty() {
            return self.idle_intent(id);
        }
        let cells: Vec<Coord> = targets
            .iter()
            .filter_map(|t| self.actors.get(*t))
            .map(|a| a.pos)
            .collect();
        // Anybody who lays eyes on the mass tells everybody else roughly where
        // it was. That is what turns one scout's bad luck into a hunter walking
        // your way ten turns later.
        if let Some(nearest) = cells.iter().min_by_key(|c| c.taxi(from)) {
            self.raise_rumor(*nearest);
        }
        if actors::is_caravan(kind) {
            let step = self.immediate_uphill_step(id, &cells, false);
            return Some(self.step_intent(from, step));
        }
        if actors::is_hunter_family(kind)
            && let Some(target) = self.line_of_fire(id, &cells)
        {
            return Some(Intent {
                kind: IntentKind::Aim,
                at: target,
            });
        }
        Some(self.approach(id, &cells))
    }

    /// A step onto a cell, named for what is standing on it.
    fn step_intent(&self, from: Coord, to: Coord) -> Intent {
        if to == from {
            return Intent {
                kind: IntentKind::Hold,
                at: from,
            };
        }
        let kind = if self
            .actor_at(to)
            .and_then(|id| self.actors.get(id))
            .is_some_and(Actor::is_player_aligned)
        {
            IntentKind::Strike
        } else {
            IntentKind::Step
        };
        Intent { kind, at: to }
    }

    /// One step along a shortest path to the nearest thing of yours.
    fn approach(&mut self, id: ActorId, cells: &[Coord]) -> Intent {
        let from = self.actors[id].pos;
        let hold = Intent {
            kind: IntentKind::Hold,
            at: from,
        };
        let paths = self.shortest_paths_to(from, cells, |sim, c| sim.grid.walkable(c));
        let Some(pick) = rand_index(&mut self.rng, paths.len()) else {
            // It can see you and there is no way to you at all — which is what
            // being walled in behind your own mass looks like from outside.
            let name = self.actors[id].name;
            self.messages.add(&format!(
                "The {name} contemplates the irrationality of its existence."
            ));
            self.effect_at(EffectKind::Refused, from);
            return hold;
        };
        paths[pick]
            .get(1)
            .map_or(hold, |step| self.step_intent(from, *step))
    }

    /// One step of a walk to somewhere in particular, or a wander when there is
    /// no way there.
    fn walk_toward(&mut self, id: ActorId, goal: Coord) -> Intent {
        let Some(from) = self.actors.get(id).map(|a| a.pos) else {
            return Intent {
                kind: IntentKind::Hold,
                at: goal,
            };
        };
        let paths = self.shortest_paths_to(from, &[goal], |sim, c| sim.grid.walkable(c));
        let step = paths.first().and_then(|path| path.get(1)).copied();
        match step {
            Some(step) => self.step_intent(from, step),
            None => self.wander_intent(id),
        }
    }

    /// Wander one cell, or stand still. Standing still is as likely as any one
    /// direction, because the original's inclusive random ran one past the end
    /// of the list.
    fn wander_intent(&mut self, id: ActorId) -> Intent {
        let Some(from) = self.actors.get(id).map(|a| a.pos) else {
            return Intent {
                kind: IntentKind::Hold,
                at: Coord::new(0, 0),
            };
        };
        let adjacent: Vec<Coord> = self.grid.adjacent_walkable(from).collect();
        let count = i32::try_from(adjacent.len()).unwrap_or(i32::MAX);
        let pick = rand_inclusive(&mut self.rng, 0, count);
        if pick < count {
            #[allow(clippy::cast_sign_loss)] // Bounded by the branch above.
            let step = adjacent[pick as usize];
            return self.step_intent(from, step);
        }
        Intent {
            kind: IntentKind::Hold,
            at: from,
        }
    }

    // -- what a human does when nobody is in sight ---------------------------

    /// The duty roster.
    fn idle_intent(&mut self, id: ActorId) -> Option<Intent> {
        match self.actors.get(id)?.orders.duty {
            Duty::Convoy => self.convoy_intent(id),
            Duty::Escort(caravan) => self.escort_intent(id, caravan),
            Duty::Garrison => self.garrison_intent(id),
            Duty::Patrol => {
                let goal = self.patrol_goal(id);
                Some(self.walk_or_wander(id, goal))
            }
            Duty::Investigate => {
                let goal = self.rumor_goal(id).or_else(|| self.patrol_goal(id));
                Some(self.walk_or_wander(id, goal))
            }
            Duty::None => Some(self.wander_intent(id)),
        }
    }

    /// A garrison walks a beat: out from the doorway it came from to the
    /// nearest chamber, and back again. It is the least interesting job in the
    /// cavern and that is the point — a garrison is scenery until you walk into
    /// it, and then it is a wall between you and the gate.
    fn garrison_intent(&mut self, id: ActorId) -> Option<Intent> {
        let orders = self.actors.get(id)?.orders;
        let from = self.actors.get(id)?.pos;
        let goal = match orders.goal {
            Some(goal) if goal.taxi(from) > ARRIVED => Some(goal),
            // Turned round: whichever end of the beat it is not standing at.
            _ if orders.home.taxi(from) > ARRIVED => Some(orders.home),
            _ => self.nearest_junction(from),
        };
        if let Some(goal) = goal {
            self.actors.get_mut(id)?.orders.goal = Some(goal);
        }
        Some(self.walk_or_wander(id, goal))
    }

    /// Walk to a goal, or mill about when the duty could not name one.
    fn walk_or_wander(&mut self, id: ActorId, goal: Option<Coord>) -> Intent {
        match goal {
            Some(goal) => self.walk_toward(id, goal),
            None => self.wander_intent(id),
        }
    }

    /// A caravan is going somewhere: the far gate it was given when it came out
    /// of its own. Reaching that doorway is the last step it takes — the turn
    /// after, it is through the gate and away with everything it was carrying,
    /// which is what makes ignoring one a decision rather than an oversight.
    fn convoy_intent(&mut self, id: ActorId) -> Option<Intent> {
        let from = self.actors.get(id)?.pos;
        let standing = self.actors.get(id)?.orders.goal;
        let goal = match standing {
            // A gate that has already fallen is not a way out any more.
            Some(goal) if self.gate_beside(goal).is_some() => goal,
            // Nowhere left to take the cargo: the cavern is the caravan's
            // problem now, and it mills about in it like everybody else.
            _ => match self.assign_convoy_goal(id) {
                Some(goal) => goal,
                None => return Some(self.wander_intent(id)),
            },
        };
        if goal == from
            && let Some(gate) = self.gate_beside(from)
        {
            return Some(Intent {
                kind: IntentKind::Depart,
                at: gate,
            });
        }
        Some(self.walk_toward(id, goal))
    }

    /// Give a caravan somewhere to be: the doorway of a gate that is not the
    /// one it came out of, the further off the better.
    fn assign_convoy_goal(&mut self, id: ActorId) -> Option<Coord> {
        let from = self.actors.get(id)?.pos;
        let mut doors: Vec<Coord> = self
            .cities
            .clone()
            .into_iter()
            .filter_map(|gate| self.gate_door(gate))
            .filter(|door| door.taxi(from) >= CONVOY_MIN)
            .collect();
        doors.sort_by_key(|door| std::cmp::Reverse(door.taxi(from)));
        doors.truncate(CONVOY_CHOICES);
        let goal = self.pick(&doors)?;
        self.actors.get_mut(id)?.orders.goal = Some(goal);
        Some(goal)
    }

    /// An escort keeps station on its caravan: close in until it is beside the
    /// wagon, then hold the ground it is on. Holding rather than following is
    /// what turns a convoy into a moving wall instead of a queue.
    fn escort_intent(&mut self, id: ActorId, caravan: ActorId) -> Option<Intent> {
        let from = self.actors.get(id)?.pos;
        let Some(wagon) = self.actors.get(caravan).map(|a| a.pos) else {
            // The wagon is gone — eaten, or away through its gate. Whoever was
            // guarding it goes back to guarding the gate.
            self.actors.get_mut(id)?.orders.duty = Duty::Garrison;
            return self.garrison_intent(id);
        };
        if wagon.taxi(from) <= ESCORT_RANGE {
            return Some(self.wander_intent(id));
        }
        Some(self.walk_toward(id, wagon))
    }

    /// A scout's beat is the far end of the map: it walks between the chambers
    /// the gates are furthest from, which is exactly where an amoeba nobody has
    /// found yet is living.
    fn patrol_goal(&mut self, id: ActorId) -> Option<Coord> {
        let from = self.actors.get(id)?.pos;
        if let Some(goal) = self.actors.get(id)?.orders.goal
            && goal.taxi(from) > ARRIVED
            && !self.is_wall(goal)
        {
            return Some(goal);
        }
        let mut deep = self.junctions.clone();
        deep.sort_by_key(|c| std::cmp::Reverse(self.depth_at(*c)));
        deep.truncate((deep.len() / 2).max(1));
        deep.retain(|c| c.taxi(from) > ARRIVED);
        let goal = self.pick(&deep)?;
        self.actors.get_mut(id)?.orders.goal = Some(goal);
        Some(goal)
    }

    /// The freshest report worth walking to.
    fn rumor_goal(&self, id: ActorId) -> Option<Coord> {
        let from = self.actors.get(id)?.pos;
        self.rumors
            .iter()
            .rev()
            .find(|rumor| rumor.pos.taxi(from) > ARRIVED)
            .map(|rumor| rumor.pos)
    }

    /// The chamber nearest a cell, which is where a garrison's beat runs to.
    fn nearest_junction(&self, from: Coord) -> Option<Coord> {
        self.junctions
            .iter()
            .copied()
            .filter(|c| c.taxi(from) > ARRIVED)
            .min_by_key(|c| c.taxi(from))
    }

    // -- rumors --------------------------------------------------------------

    /// Report the amoeba at a cell, and let everybody who cares hear about it.
    ///
    /// A report near one already on the books moves that one rather than adding
    /// another, so a fight that lasts twenty turns leaves one rumor that
    /// follows the fight rather than twenty stale ones behind it.
    pub(crate) fn raise_rumor(&mut self, pos: Coord) {
        let until = self.schedule.time() + RUMOR_LIFE * TURN;
        if let Some(known) = self
            .rumors
            .iter_mut()
            .find(|rumor| rumor.pos.taxi(pos) <= RUMOR_MERGE)
        {
            known.pos = pos;
            known.until = until;
            return;
        }
        self.rumors.push(Rumor { pos, until });
        if self.rumors.len() > MAX_RUMORS {
            self.rumors.remove(0);
        }
    }

    /// Drop the reports nobody believes any more.
    pub(crate) fn forget_stale_rumors(&mut self) {
        let now = self.schedule.time();
        self.rumors.retain(|rumor| rumor.until > now);
    }

    /// The one cell a gate opens onto.
    ///
    /// Measured against the rock rather than against walkability, because a
    /// doorway with somebody standing in it is still the doorway.
    pub(crate) fn gate_door(&self, gate: ActorId) -> Option<Coord> {
        let pos = self.actors.get(gate)?.pos;
        self.grid.adjacent(pos).find(|n| !self.is_wall(*n))
    }

    /// The gate beside a cell, if one is.
    fn gate_beside(&self, c: Coord) -> Option<Coord> {
        self.grid.adjacent(c).find(|n| {
            self.actor_at(*n)
                .and_then(|id| self.actors.get(id))
                .is_some_and(|a| actors::is_city(a.kind))
        })
    }

    /// A caravan that reached a far gate with its cargo intact is gone, and so
    /// is everything it was carrying.
    fn depart(&mut self, id: ActorId, gate: Coord) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let (name, from) = (actor.name, actor.pos);
        if self.gate_beside(from) != Some(gate) {
            return;
        }
        self.effect(EffectKind::Depart, from, gate);
        self.messages.add(&format!(
            "The {name} slips through the gate with its cargo."
        ));
        self.remove_actor(id);
        self.cue(Cue::Depart);
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

    /// One turn of the countdown a painted line runs on. The last of them is
    /// the shot.
    fn discharge(&mut self, id: ActorId) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let Extra::Ranged(ranged) = actor.extra else {
            return;
        };
        if ranged.firing <= 0 {
            self.fire(id);
        } else if let Extra::Ranged(state) = &mut self.actors[id].extra {
            state.firing -= 1;
        }
    }

    /// Whichever of these targets is on the same row or column and inside
    /// range, if any is.
    fn line_of_fire(&mut self, id: ActorId, cells: &[Coord]) -> Option<Coord> {
        let actor = self.actors.get(id)?;
        let from = actor.pos;
        let Extra::Ranged(ranged) = actor.extra else {
            return None;
        };
        let mut paths = self.shortest_paths_to(from, cells, |sim, c| sim.grid.walkable(c));
        while let Some(pick) = rand_index(&mut self.rng, paths.len()) {
            let path = paths.remove(pick);
            let Some(&target) = path.last() else {
                continue;
            };
            // Path length counts cells including the start, so the step count
            // is one less: this is the original's `Length <= Range + 1`.
            let steps = i32::try_from(path.len()).unwrap_or(i32::MAX) - 1;
            if steps <= ranged.range && (target.x == from.x || target.y == from.y) {
                return Some(target);
            }
        }
        None
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
        let mut last = from;
        while self.grid.in_bounds(bullet) && !self.is_wall(bullet) && travelled < ranged.range {
            travelled += 1;
            self.reticles.push(Reticle {
                pos: bullet,
                owner: id,
            });
            last = bullet;
            bullet = bullet + direction;
        }
        if let Extra::Ranged(state) = &mut self.actors[id].extra {
            state.direction = direction;
            state.firing -= 1;
        }
        self.effect(EffectKind::Aim, from, last);
        self.cue(Cue::Aim);
    }

    /// Fire down the painted line.
    ///
    /// Everything of yours under a reticle is *unslimed* rather than destroyed,
    /// so it drops the whole of what it was made of. A nucleus gets its retreat
    /// first, and whatever it retreated into takes the shot instead.
    fn fire(&mut self, id: ActorId) {
        let name = self.actors[id].name;
        let from = self.actors[id].pos;
        if let Extra::Ranged(state) = &mut self.actors[id].extra {
            state.firing = state.firing_time;
        }
        let line: Vec<Coord> = self
            .reticles
            .iter()
            .filter(|r| r.owner == id)
            .map(|r| r.pos)
            .collect();
        // The bullet is drawn before anything it hits comes apart, so the shot
        // reads as the cause of the losses reported after it.
        self.effect(EffectKind::Shot, from, line.last().copied().unwrap_or(from));
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
        let born = self.add_actor(baby, door);
        self.brief(born, id, door);
        self.effect(EffectKind::Produce, pos, door);
    }

    /// Tell a human what it is for, the moment it steps out of the gate.
    ///
    /// A caravan claims the soldiers behind it in the queue, which is what
    /// makes a wave with a wagon in it read as a convoy rather than as a wagon
    /// that happens to have company.
    fn brief(&mut self, id: ActorId, gate: ActorId, door: Coord) {
        let Some(actor) = self.actors.get_mut(id) else {
            return;
        };
        actor.orders.home = door;
        let kind = actor.kind;
        if actors::is_caravan(kind) {
            self.assign_convoy_goal(id);
            if let Extra::City(state) = &mut self.actors[gate].extra {
                state.convoy = Some(id);
                state.escorts = ESCORT_SIZE;
            }
            return;
        }
        // Guns have their own jobs; it is the soldiers that get walking orders.
        if actors::is_hunter_family(kind) || !actors::is_militia_family(kind) {
            return;
        }
        let convoy = match &mut self.actors[gate].extra {
            Extra::City(state) if state.escorts > 0 => {
                state.escorts -= 1;
                state.convoy
            }
            _ => None,
        };
        if let Some(caravan) = convoy
            && self.actors.contains(caravan)
            && let Some(actor) = self.actors.get_mut(id)
        {
            actor.orders.duty = Duty::Escort(caravan);
        }
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
            // At the head of the queue rather than the tail, so the wagon comes
            // out first and the soldiers behind it are its escort.
            queued.insert(0, Kind::Caravan);
        }
        let rate = self.rules.spawn_rate;
        let pos = self.actors[id].pos;
        if let Extra::City(state) = &mut self.actors[id].extra {
            state.queue.extend(queued);
            state.wave_number += 1;
            state.turns_to_next_wave += rate;
        }
        self.effect_at(EffectKind::Wave, pos);
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
    fn sight_is_a_diamond_that_rock_stops() {
        let mut sim = sandbox(1);
        let militia = sim.add_actor(Kind::Militia, Coord::new(5, 5));
        let far = sim.add_actor(Kind::Cytoplasm, Coord::new(10, 5));
        let near = sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        assert!(sim.seen(militia, Actor::is_player_aligned).contains(&near));
        assert!(
            !sim.seen(militia, Actor::is_player_aligned).contains(&far),
            "awareness four stops at four cells"
        );
        // One cell of rock in the way is the whole of the difference. This is
        // the original's deliberate wallhack, gone.
        sim.grid.set_props(Coord::new(6, 5), false, false, true);
        assert!(
            !sim.seen(militia, Actor::is_player_aligned).contains(&near),
            "it saw straight through the rock"
        );
    }

    #[test]
    fn a_human_that_loses_sight_of_you_walks_to_where_you_were() {
        // What the wallhack used to do, the rumors do instead.
        let mut sim = sandbox(48);
        let militia = sim.add_actor(Kind::Militia, Coord::new(5, 5));
        let cyto = sim.add_actor(Kind::Cytoplasm, Coord::new(8, 5));
        sim.human_act(militia);
        assert_eq!(sim.rumors().len(), 1, "it reported what it saw");
        // Now wall the mass off and let it keep coming.
        for y in 3..8 {
            sim.grid.set_props(Coord::new(7, y), false, false, true);
        }
        assert!(sim.seen(militia, Actor::is_player_aligned).is_empty());
        for _ in 0..3 {
            sim.human_act(militia);
        }
        assert!(
            sim.actors[militia].pos.x > 5,
            "it went to look where the report put them: {:?}",
            sim.actors[militia].pos
        );
        assert!(sim.actors.contains(cyto), "and has not got there yet");
    }

    #[test]
    fn a_militia_declares_a_step_and_then_takes_it() {
        let mut sim = sandbox(2);
        let militia = sim.add_actor(Kind::Militia, Coord::new(5, 5));
        let cyto = sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        // A human that has only just arrived spends its first turn deciding.
        sim.human_act(militia);
        assert_eq!(
            sim.actors[militia].pos,
            Coord::new(5, 5),
            "it has not moved"
        );
        assert_eq!(
            sim.actors[militia].intent,
            Some(Intent {
                kind: IntentKind::Step,
                at: Coord::new(6, 5)
            }),
        );
        // And then it does exactly what it said, and says what is next.
        sim.human_act(militia);
        assert_eq!(sim.actors[militia].pos, Coord::new(6, 5), "it closed in");
        assert_eq!(
            sim.actors[militia].intent,
            Some(Intent {
                kind: IntentKind::Strike,
                at: Coord::new(7, 5)
            }),
        );
        sim.human_act(militia);
        assert!(!sim.actors.contains(cyto), "and then destroyed the cell");
    }

    #[test]
    fn a_blow_lands_where_it_was_promised_and_not_where_you_went() {
        // The whole reason a telegraph is worth drawing. A commitment is to a
        // cell, not to a victim, so stepping off that cell before the blow
        // arrives really does make it swing at the floor.
        let mut sim = sandbox(31);
        let militia = sim.add_actor(Kind::Militia, Coord::new(5, 5));
        let cyto = sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.human_act(militia);
        assert_eq!(
            sim.actors[militia].intent.map(|i| i.kind),
            Some(IntentKind::Strike),
        );
        sim.set_actor_position(cyto, Coord::new(6, 6));
        sim.human_act(militia);
        assert!(
            sim.actors.contains(cyto),
            "it hit the cell, not the cell wall"
        );
        assert_eq!(
            sim.actors[militia].pos,
            Coord::new(6, 5),
            "and walked into the gap instead"
        );
    }

    #[test]
    fn a_militia_with_nowhere_to_be_holds_the_ground_it_is_on() {
        // A sandbox has no chambers, so a garrison's beat has no far end and it
        // mills about where it stands — the original's wander, kept for exactly
        // the case where a duty cannot name anywhere better.
        let mut sim = sandbox(3);
        let militia = sim.add_actor(Kind::Militia, Coord::new(5, 5));
        let mut moved = 0;
        for _ in 0..40 {
            let before = sim.actors[militia].pos;
            sim.human_act(militia);
            if sim.actors[militia].pos != before {
                moved += 1;
            }
        }
        assert!(moved > 8, "it moved on {moved} of forty turns");
        assert!(moved < 40, "and stood still on some of them");
    }

    #[test]
    fn a_caravan_runs_away() {
        let mut sim = sandbox(4);
        let caravan = sim.add_actor(Kind::Caravan, Coord::new(5, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(3, 5));
        for _ in 0..6 {
            sim.human_act(caravan);
        }
        assert!(
            sim.actors[caravan].pos.x > 5,
            "it put distance between us: {:?}",
            sim.actors[caravan].pos
        );
    }

    #[test]
    fn a_caravan_makes_for_a_far_gate_and_leaves_through_it() {
        // Ignoring a caravan is a decision, not an oversight: it is going
        // somewhere, and if it gets there the cargo goes with it.
        let mut sim = sandbox(40);
        let gate = sim.add_actor(Kind::City, Coord::new(19, 5));
        let caravan = sim.add_actor(Kind::Caravan, Coord::new(3, 5));
        for _ in 0..40 {
            if !sim.actors.contains(caravan) {
                break;
            }
            sim.human_act(caravan);
        }
        assert!(!sim.actors.contains(caravan), "it never got away");
        assert!(sim.actors.contains(gate), "and the gate is still standing");
        assert!(
            sim.messages()
                .lines()
                .iter()
                .any(|line| line.contains("slips through the gate")),
        );
    }

    #[test]
    fn the_soldiers_behind_a_caravan_come_out_as_its_escort() {
        let mut sim = sandbox(41);
        sim.grid.set_props(Coord::new(6, 5), false, false, true);
        let gate = sim.add_actor(Kind::City, Coord::new(6, 5));
        // Somewhere for the wagon to be going.
        sim.add_actor(Kind::City, Coord::new(19, 9));
        if let Extra::City(state) = &mut sim.actors[gate].extra {
            state
                .queue
                .extend([Kind::Caravan, Kind::Militia, Kind::Militia]);
        }
        for _ in 0..3 {
            sim.city_act(gate);
        }
        let caravan = sim
            .actors
            .iter()
            .find(|(_, a)| a.kind == Kind::Caravan)
            .map(|(id, _)| id)
            .expect("the wagon came out first");
        let escorts = sim
            .actors
            .iter()
            .filter(|(_, a)| a.orders.duty == Duty::Escort(caravan))
            .count();
        assert_eq!(escorts, 2, "both soldiers were assigned to it");
    }

    #[test]
    fn an_escort_whose_wagon_is_gone_goes_back_to_the_gate() {
        let mut sim = sandbox(46);
        let caravan = sim.add_actor(Kind::Caravan, Coord::new(9, 9));
        let guard = sim.add_actor(Kind::Militia, Coord::new(8, 9));
        sim.actors[guard].orders.duty = Duty::Escort(caravan);
        sim.remove_actor(caravan);
        sim.human_act(guard);
        assert_eq!(sim.actors[guard].orders.duty, Duty::Garrison);
    }

    #[test]
    fn seeing_the_mass_is_what_starts_a_rumor() {
        let mut sim = sandbox(44);
        let scout = sim.add_actor(Kind::Scout, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        assert!(sim.rumors().is_empty());
        sim.human_act(scout);
        assert_eq!(sim.rumors().len(), 1, "it told everybody");
        assert_eq!(sim.rumors()[0].pos, Coord::new(7, 5));
    }

    #[test]
    fn a_report_beside_an_old_one_moves_it_rather_than_adding_another() {
        let mut sim = sandbox(43);
        sim.raise_rumor(Coord::new(5, 5));
        sim.raise_rumor(Coord::new(6, 5));
        assert_eq!(sim.rumors().len(), 1, "one fight is one rumor");
        assert_eq!(sim.rumors()[0].pos, Coord::new(6, 5), "and it followed");
        sim.raise_rumor(Coord::new(18, 18));
        assert_eq!(sim.rumors().len(), 2);
    }

    #[test]
    fn a_hunter_walks_down_the_freshest_rumor() {
        let mut sim = sandbox(42);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        sim.raise_rumor(Coord::new(15, 5));
        for _ in 0..3 {
            sim.human_act(hunter);
        }
        assert!(
            sim.actors[hunter].pos.x > 5,
            "it went to look: {:?}",
            sim.actors[hunter].pos
        );
    }

    #[test]
    fn a_scout_patrols_the_chambers_furthest_from_the_gates() {
        let mut sim = sandbox(45);
        sim.junctions = vec![Coord::new(3, 3), Coord::new(16, 16)];
        sim.depth[3 * 20 + 3] = 1;
        sim.depth[16 * 20 + 16] = 30;
        let scout = sim.add_actor(Kind::Scout, Coord::new(9, 9));
        sim.human_act(scout);
        assert_eq!(
            sim.actors[scout].orders.goal,
            Some(Coord::new(16, 16)),
            "it walks the deep half, not the near one"
        );
    }

    #[test]
    fn an_engulfed_human_does_not_get_to_act() {
        let mut sim = sandbox(5);
        // A pocket at (1,1) sealed by the arena walls and one cytoplasm.
        let militia = sim.add_actor(Kind::Militia, Coord::new(1, 1));
        sim.add_actor(Kind::Cytoplasm, Coord::new(2, 1));
        sim.add_actor(Kind::Cytoplasm, Coord::new(1, 2));
        sim.human_act(militia);
        assert!(!sim.actors.contains(militia));
        let corpse = sim.actor_at(Coord::new(1, 1)).expect("a corpse");
        assert_eq!(sim.actors[corpse].kind, Kind::DissolvingMilitia);
    }

    #[test]
    fn a_hunter_declares_a_shot_before_it_paints_one() {
        let mut sim = sandbox(6);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        let victim = sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        // Turn one: it says it is about to line up, and paints nothing.
        sim.human_act(hunter);
        assert!(sim.reticles.is_empty(), "nothing is painted yet");
        assert_eq!(
            sim.actors[hunter].intent,
            Some(Intent {
                kind: IntentKind::Aim,
                at: Coord::new(7, 5)
            }),
        );
        // Turn two: the line goes up and the shot is promised for next turn.
        sim.human_act(hunter);
        assert!(sim.reticles.iter().any(|r| r.pos == Coord::new(7, 5)));
        let Extra::Ranged(state) = sim.actors[hunter].extra else {
            panic!("a hunter aims")
        };
        assert_eq!(state.firing, 0);
        assert_eq!(state.direction, Coord::new(1, 0));
        assert_eq!(
            sim.actors[hunter].intent.map(|i| i.kind),
            Some(IntentKind::Fire),
        );
        assert!(sim.actors.contains(victim));
        // Turn three: it fires.
        sim.human_act(hunter);
        assert!(!sim.actors.contains(victim));
        assert!(sim.reticles.is_empty(), "the line is cleared");
        let Extra::Ranged(state) = sim.actors[hunter].extra else {
            panic!("a hunter reloads")
        };
        assert_eq!(state.firing, state.firing_time);
    }

    #[test]
    fn a_painted_line_is_fired_down_whatever_turns_up_in_the_meantime() {
        // A gun that has drawn its line is committed to it, so walking a fresh
        // organelle into the beam does not talk it into a better target.
        let mut sim = sandbox(47);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        sim.human_act(hunter);
        sim.human_act(hunter);
        let painted: Vec<Coord> = sim.reticles.iter().map(|r| r.pos).collect();
        sim.add_actor(Kind::Cytoplasm, Coord::new(5, 4));
        assert_eq!(
            sim.actors[hunter].intent.map(|i| i.kind),
            Some(IntentKind::Fire),
        );
        sim.human_act(hunter);
        assert!(
            painted.iter().all(|c| !sim.reticle_at(*c)),
            "it fired down the line it had drawn"
        );
    }

    #[test]
    fn a_shot_unslimes_rather_than_destroys() {
        let mut sim = sandbox(7);
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(5, 5));
        sim.add_actor(Kind::QuantumCore, Coord::new(7, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(15, 15));
        for _ in 0..3 {
            sim.human_act(hunter);
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
            sim.human_act(hunter);
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
        sim.human_act(hunter);
        sim.human_act(hunter);
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
        sim.human_act(scout);
        sim.human_act(scout);
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
