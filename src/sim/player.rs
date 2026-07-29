//! What the amoeba does.
//!
//! The one action that matters is [`Sim::attack_move`]. Walking into your own
//! body swaps with it, walking into a human eats it, walking into a gate
//! breaks it if you are big enough, walking onto a catalyst absorbs it, and
//! walking into empty floor drags a chain of organelles along behind you.
//! Every organelle that acts on its own uses the same entry point, so the
//! rules only exist once.
//!
//! The drag is the interesting half. A breadth-first spanning tree is grown
//! over the connected mass from whoever is moving; one of the deepest tails is
//! chosen at random; and every organelle along that chain shuffles forward
//! into the cell its predecessor just left. The mass flows a cell in the
//! direction of travel and stays connected. The bright highlight you see is
//! the union of *all* deepest paths, so when tails tie it promises more than
//! the move delivers — that is the original's behaviour and it is what makes
//! the highlight readable.

use super::actors::{self, Extra, ItemKind, Kind};
use super::grid::{Coord, Dir, rand_index};
use super::{ActorId, Cue, ItemId, Phase, Sim};

/// One node of the drag's spanning tree.
#[derive(Clone, Copy, Debug)]
struct Node {
    id: ActorId,
    /// Index into [`SlimeTree::nodes`], or `None` for the mover.
    parent: Option<usize>,
    dist: i32,
}

/// A breadth-first spanning tree of the connected mass, rooted at whoever is
/// about to move.
///
/// Computed once per turn when the active nucleus takes over — which is also
/// when the highlight is painted — and reused by the move itself as long as
/// nothing has touched the world in between.
#[derive(Clone, Debug)]
pub struct SlimeTree {
    root: ActorId,
    version: u64,
    nodes: Vec<Node>,
    max_depth: i32,
}

impl SlimeTree {
    /// Indices of every node at the greatest depth reached.
    fn deepest(&self) -> Vec<usize> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.dist == self.max_depth)
            .map(|(i, _)| i)
            .collect()
    }

    /// The chain from the mover down to `index`, mover first.
    fn chain(&self, index: usize) -> Vec<ActorId> {
        let mut out = Vec::new();
        let mut at = Some(index);
        while let Some(i) = at {
            out.push(self.nodes[i].id);
            at = self.nodes[i].parent;
        }
        out.reverse();
        out
    }
}

impl Sim {
    // -- the spanning tree --------------------------------------------------

    /// The drag tree for `root`, rebuilt only if the world has moved since it
    /// was last computed.
    fn slime_tree(&mut self, root: ActorId) -> &SlimeTree {
        let stale = self
            .drag_cache
            .as_ref()
            .is_none_or(|t| t.root != root || t.version != self.version);
        if stale {
            let tree = self.build_slime_tree(root);
            self.drag_cache = Some(tree);
        }
        self.drag_cache.as_ref().expect("just rebuilt")
    }

    /// Breadth-first over everything player-aligned and not anchored.
    ///
    /// DELIBERATE CHANGE from C#: neighbours come from the constant-time cell
    /// index in left/right/up/down order rather than from a linear scan of the
    /// whole actor list. The set of nodes is identical; only the order two
    /// equally-deep tails are discovered in can differ.
    fn build_slime_tree(&self, root: ActorId) -> SlimeTree {
        let mut nodes = vec![Node {
            id: root,
            parent: None,
            dist: 0,
        }];
        let mut in_tree = vec![false; self.actors.capacity()];
        in_tree[root.raw()] = true;
        let mut last = vec![0_usize];
        let mut depth = 1;
        let mut max_depth = 0;
        while !last.is_empty() {
            let mut frontier = Vec::new();
            for &parent in &last {
                let pos = self.actors[nodes[parent].id].pos;
                for cell in self.grid.adjacent(pos) {
                    let Some(id) = self.actor_at(cell) else {
                        continue;
                    };
                    let Some(actor) = self.actors.get(id) else {
                        continue;
                    };
                    // Anchored organelles are immovable: the gravity core sets
                    // that flag while it is pulling everybody else around.
                    if actor.slime == 0 || actor.anchor || in_tree[id.raw()] {
                        continue;
                    }
                    in_tree[id.raw()] = true;
                    max_depth = depth;
                    nodes.push(Node {
                        id,
                        parent: Some(parent),
                        dist: depth,
                    });
                    frontier.push(nodes.len() - 1);
                }
            }
            depth += 1;
            last = frontier;
        }
        SlimeTree {
            root,
            version: self.version,
            nodes,
            max_depth,
        }
    }

    /// Repaint the mass: body green everywhere, path green on every organelle
    /// that lies on *some* deepest chain.
    pub(crate) fn color_moving_slime(&mut self, root: ActorId) {
        for &id in &self.player_mass.clone() {
            if let Some(actor) = self.actors.get_mut(id)
                && actor.slime > 0
            {
                actor.slime = 1;
            }
        }
        let chains: Vec<Vec<ActorId>> = {
            let tree = self.slime_tree(root);
            tree.deepest().into_iter().map(|i| tree.chain(i)).collect()
        };
        for chain in chains {
            for id in chain {
                if let Some(actor) = self.actors.get_mut(id) {
                    actor.slime = 2;
                }
            }
        }
    }

    /// The chain that will actually move: one deepest tail, chosen uniformly.
    fn drag_path(&mut self, root: ActorId) -> Vec<ActorId> {
        let deepest = self.slime_tree(root).deepest();
        let Some(pick) = rand_index(&mut self.rng, deepest.len()) else {
            return vec![root];
        };
        let tree = self.drag_cache.as_ref().expect("built above");
        tree.chain(deepest[pick])
    }

    // -- nucleus control ----------------------------------------------------

    /// Hand control to `id` and put every nucleus back on the clock together.
    ///
    /// All nuclei are rescheduled at the *active* one's speed, which is what
    /// makes "only one nucleus may act per turn" true no matter how many you
    /// are carrying or how fast they individually are.
    pub(crate) fn set_active_nucleus(&mut self, id: ActorId) {
        if !self.actors.contains(id) {
            return;
        }
        self.active = Some(id);
        let delay = self.actors[id].delay;
        self.schedule.remove(id);
        self.schedule.add(id, delay);
        let others: Vec<ActorId> = self
            .player_mass
            .iter()
            .copied()
            .filter(|&n| {
                n != id
                    && self
                        .actors
                        .get(n)
                        .is_some_and(|a| actors::is_nucleus_family(a.kind))
            })
            .collect();
        for other in others {
            self.schedule.remove(other);
            self.schedule.add(other, delay);
        }
        self.color_moving_slime(id);
    }

    /// Step through the nuclei in acquisition order. Free, but it does put
    /// every nucleus back on the clock.
    pub(crate) fn next_nucleus(&mut self, shift: i32) {
        let nuclei: Vec<ActorId> = self
            .player_mass
            .iter()
            .copied()
            .filter(|&id| {
                self.actors
                    .get(id)
                    .is_some_and(|a| actors::is_nucleus_family(a.kind))
            })
            .collect();
        if nuclei.is_empty() {
            return;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let len = nuclei.len() as i32;
        // A nucleus that just died leaves `active` dangling; starting from -1
        // then lands on the end of the list, which is what the original's
        // `IndexOf == -1` path did.
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let current = self
            .active
            .and_then(|a| nuclei.iter().position(|&n| n == a))
            .map_or(-1, |i| i as i32);
        #[allow(clippy::cast_sign_loss)]
        let index = (current + shift).rem_euclid(len) as usize;
        self.set_active_nucleus(nuclei[index]);
    }

    // -- the master action --------------------------------------------------

    /// Move the active nucleus one cell.
    pub(crate) fn attack_move_player(&mut self, dir: Dir) -> bool {
        let Some(active) = self.active else {
            return false;
        };
        let Some(actor) = self.actors.get(active) else {
            return false;
        };
        let dest = actor.pos + dir.offset();
        self.attack_move(active, dest)
    }

    /// Everything an organelle can do by walking into a cell.
    ///
    /// Returns whether the action happened. A refused action costs nothing,
    /// which is why walking into a wall does not waste a turn.
    #[allow(clippy::too_many_lines)] // One branch per thing you can walk into.
    pub(crate) fn attack_move(&mut self, mover: ActorId, dest: Coord) -> bool {
        if !self.actors.contains(mover) {
            return false;
        }
        // The quantum core banks its full speed before every action; the swap
        // branch below is the only thing that discounts it.
        if self.actors[mover].kind == Kind::QuantumCore {
            self.actors[mover].delay = 8;
        }
        let mover_name = self.actors[mover].name;
        let mut success = false;
        if let Some(target) = self.actor_at(dest) {
            let target_kind = self.actors[target].kind;
            let target_name = self.actors[target].name;
            if self.actors[target].slime > 0 {
                self.swap_actors(mover, target);
                // Spending a material can replace the mover with the thing it
                // upgraded into, so nothing below may assume it still exists.
                if actors::is_crafting_material(target_kind) {
                    self.try_upgrade(target, mover);
                }
                if self
                    .actors
                    .get(mover)
                    .is_some_and(|a| a.kind == Kind::QuantumCore)
                {
                    self.actors[mover].delay /= 2;
                    self.set_active_nucleus(mover);
                }
                self.cue(Cue::Swap);
                success = true;
            } else if actors::is_npc(target_kind) && self.actors[target].armor > 0 {
                let mover_kind = self.actors[mover].kind;
                if mover_kind == Kind::ReinforcedMaw {
                    self.messages.add(&format!(
                        "The {target_name} is crushed by the jaws of the {mover_name}!"
                    ));
                    self.eat_actor(mover, target);
                    self.cue(Cue::Eat);
                    success = true;
                } else if mover_kind == Kind::LaserCore {
                    self.messages.add(&format!(
                        "The {target_name} is obliterated by the {mover_name}'s laser beam!"
                    ));
                    self.eat_actor(mover, target);
                    self.cue(Cue::Eat);
                    success = true;
                } else {
                    self.messages.add(&format!(
                        "The {target_name}'s armor is too strong for the {mover_name}!"
                    ));
                    self.cue(Cue::Bump);
                }
            } else if actors::is_city(target_kind) {
                let armor = self.actors[target].armor;
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let mass = self.player_mass.len() as i32;
                if mass >= armor {
                    self.destroy_city(target);
                    success = true;
                } else {
                    self.messages.add(&format!(
                        "There is not enough mass to destroy the {target_name}! (Have {mass}, need {armor})"
                    ));
                    self.cue(Cue::Bump);
                }
            } else if actors::is_eatable(target_kind) {
                self.messages
                    .add(&format!("The {mover_name} consumes the {target_name}."));
                self.eat_actor(mover, target);
                self.cue(Cue::Eat);
                success = true;
            }
        } else if let Some(item) = self.item_at(dest) {
            // Failing to find room for a catalyst is not a failed action: the
            // organelle just walks onto the tile instead.
            success = self.ingest(mover, item) || {
                let walked = self.move_organelle(mover, dest);
                if walked {
                    self.cue(Cue::Step);
                }
                walked
            };
        } else {
            success = self.move_organelle(mover, dest);
            if success {
                self.cue(Cue::Step);
            } else {
                self.cue(Cue::Bump);
            }
        }
        if success {
            let pos = self.actors.get(mover).map(|a| a.pos);
            if let Some(pos) = pos {
                let neighbours: Vec<Coord> = self.grid.adjacent(pos).collect();
                for cell in neighbours {
                    if let Some(other) = self.actor_at(cell)
                        && self
                            .actors
                            .get(other)
                            .is_some_and(|a| actors::is_npc(a.kind))
                    {
                        self.engulf(other);
                    }
                }
            }
            // Whatever the mover does *after* arriving: the quantum core banks
            // its full speed again, the gravity core hauls its neighbours up,
            // and the terror core takes the turn off whoever it has just come
            // to stand beside.
            match self.actors.get(mover).map(|a| a.kind) {
                Some(Kind::QuantumCore) => self.actors[mover].delay = 8,
                Some(Kind::GravityCore) => self.gravity_pull(mover),
                Some(Kind::TerrorCore) => self.terrify(mover),
                _ => {}
            }
        }
        success
    }

    /// Flow the mass one cell: the mover steps, and one chain of organelles
    /// shuffles up behind it.
    pub(crate) fn move_organelle(&mut self, mover: ActorId, dest: Coord) -> bool {
        if !self.grid.in_bounds(dest) || !self.actors.contains(mover) {
            return false;
        }
        let path = self.drag_path(mover);
        let mut vacated = self.actors[mover].pos;
        if !self.set_actor_position(mover, dest) {
            return false;
        }
        for &id in path.iter().skip(1) {
            if !self.actors.contains(id) {
                continue;
            }
            let buffer = self.actors[id].pos;
            self.set_actor_position(id, vacated);
            vacated = buffer;
        }
        true
    }

    // -- eating -------------------------------------------------------------

    /// Consume an actor by contact.
    ///
    /// The eater and the victim swap first, so the eater ends up on the
    /// victim's cell and the dissolving corpse materialises behind it. An item
    /// that was lying under the victim is absorbed too, unless it is a
    /// nutrient — the original skipped those deliberately so that eating twice
    /// in one action could not grow the mass twice.
    pub(crate) fn eat_actor(&mut self, eating: ActorId, eaten: ActorId) {
        let Some(victim) = self.actors.get(eaten) else {
            return;
        };
        let under = self.item_at(victim.pos);
        if actors::is_eatable(victim.kind) {
            self.swap_actors(eating, eaten);
            self.on_eaten_actor(eaten);
        } else {
            self.remove_actor(eaten);
        }
        if let Some(item) = under
            && self
                .items
                .get(item)
                .is_some_and(|i| i.kind != ItemKind::Nutrient)
        {
            self.ingest(eating, item);
        }
    }

    /// A human becomes its dissolving form and joins the mass.
    fn on_eaten_actor(&mut self, id: ActorId) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let (kind, pos) = (actor.kind, actor.pos);
        let Some(next) = kind.becomes_on_eaten() else {
            return;
        };
        self.remove_actor(id);
        self.add_actor(next, pos);
    }

    /// Absorb a catalyst.
    ///
    /// A nutrient converts where it lies and the mass grows by one. Every
    /// other catalyst needs a cytoplasm to grow inside, found by spreading
    /// outward through the mass one ring at a time; the mass stays the same
    /// size because a cytoplasm is spent. Returning `false` means there was no
    /// cytoplasm anywhere and the caller should just walk onto the tile.
    pub(crate) fn ingest(&mut self, eating: ActorId, item: ItemId) -> bool {
        let Some(&super::Item { kind, pos }) = self.items.get(item) else {
            return false;
        };
        if !self.actors.contains(eating) {
            return false;
        }
        let product = kind.new_organelle();
        if kind == ItemKind::Nutrient {
            self.remove_item(item);
            self.add_actor(product, pos);
            self.cue(Cue::Ingest);
            self.attack_move(eating, pos);
            return true;
        }
        // The eater may be its own host, in which case it transforms on the
        // catalyst's cell rather than moving twice.
        let host = if self.actors[eating].kind == Kind::Cytoplasm {
            Some(eating)
        } else {
            let ring = self.nearest_cytoplasm(eating);
            self.pick(&ring)
        };
        let Some(host) = host else {
            self.messages
                .add(&format!("There is no room to eat the {}.", kind.name()));
            return false;
        };
        let transforms_self = host == eating;
        let where_to = if transforms_self {
            pos
        } else {
            self.actors[host].pos
        };
        self.remove_item(item);
        self.remove_actor(host);
        self.add_actor(product, where_to);
        self.messages.add(&format!(
            "The {} is absorbed and becomes a {}.",
            kind.name(),
            product.name()
        ));
        self.cue(Cue::Ingest);
        if !transforms_self {
            self.attack_move(eating, pos);
        }
        true
    }

    /// Every cytoplasm on the nearest ring of the mass that has any.
    fn nearest_cytoplasm(&self, from: ActorId) -> Vec<ActorId> {
        let mut seen = vec![false; self.actors.capacity()];
        seen[from.raw()] = true;
        let mut ring = vec![from];
        loop {
            let found: Vec<ActorId> = ring
                .iter()
                .copied()
                .filter(|&id| self.actors[id].kind == Kind::Cytoplasm)
                .collect();
            if !found.is_empty() {
                return found;
            }
            let mut next = Vec::new();
            for id in ring {
                let pos = self.actors[id].pos;
                for cell in self.grid.adjacent(pos) {
                    if let Some(other) = self.actor_at(cell)
                        && !seen[other.raw()]
                        && self.actors[other].slime > 0
                    {
                        seen[other.raw()] = true;
                        next.push(other);
                    }
                }
            }
            if next.is_empty() {
                return Vec::new();
            }
            ring = next;
        }
    }

    // -- engulfing ----------------------------------------------------------

    /// Capture a human that has been sealed in, along with everybody sealed in
    /// with it.
    pub(crate) fn engulf(&mut self, npc: ActorId) -> bool {
        let mut group = Vec::new();
        if !self.can_engulf(npc, &mut group) {
            return false;
        }
        for id in group {
            if !self.actors.contains(id) {
                continue;
            }
            let name = self.actors[id].name;
            self.messages.add(&format!("The {name} is engulfed!"));
            self.on_eaten_actor(id);
        }
        self.cue(Cue::Engulf);
        true
    }

    /// Whether a human, and everything contiguous with it, has nowhere to go.
    ///
    /// Walls seal. Your own organelles seal, including dissolving humans,
    /// which are organelles rather than humans. A gate next to any member of
    /// the group breaks the seal for the whole group — gates will not help you
    /// trap their own people.
    fn can_engulf(&self, npc: ActorId, group: &mut Vec<ActorId>) -> bool {
        group.push(npc);
        let Some(actor) = self.actors.get(npc) else {
            return false;
        };
        let neighbours: Vec<Coord> = self.grid.adjacent(actor.pos).collect();
        if neighbours.iter().any(|c| self.grid.walkable(*c)) {
            return false;
        }
        if neighbours.iter().any(|c| {
            self.actor_at(*c)
                .is_some_and(|a| self.actors.get(a).is_some_and(|x| actors::is_city(x.kind)))
        }) {
            return false;
        }
        let mut has_escape = false;
        for cell in neighbours {
            if let Some(other) = self.actor_at(cell)
                && self
                    .actors
                    .get(other)
                    .is_some_and(|a| actors::is_npc(a.kind))
                && !group.contains(&other)
                && !self.can_engulf(other, group)
            {
                has_escape = true;
            }
        }
        !has_escape
    }

    // -- digestion ----------------------------------------------------------

    /// One turn of a dissolving human coming apart.
    pub(crate) fn digest(&mut self, id: ActorId) {
        let Some(actor) = self.actors.get_mut(id) else {
            return;
        };
        actor.hp -= 1;
        if actor.hp > 0 {
            return;
        }
        let (kind, pos, max_hp) = {
            let actor = &self.actors[id];
            (actor.kind, actor.pos, actor.max_hp)
        };
        // Every butcher anywhere in the mass squeezes one extra product out of
        // a finished corpse.
        let butchers = self
            .player_mass
            .iter()
            .filter(|&&m| self.actors.get(m).is_some_and(|a| a.kind == Kind::Butcher))
            .count();
        for _ in 0..butchers {
            self.set_overfill(id, max_hp * 2);
            self.produce_if_overfull(id);
        }
        self.remove_actor(id);
        if let Some(product) = self.digest_product(kind) {
            self.add_actor(product, pos);
            self.messages.add(&format!(
                "The {} finishes dissolving into a {}.",
                kind.name(),
                product.name()
            ));
        }
        self.cue(Cue::Digest);
    }

    /// Cash in a dissolving human's banked overfill for a free product.
    pub(crate) fn produce_if_overfull(&mut self, id: ActorId) -> bool {
        let Some(actor) = self.actors.get(id) else {
            return false;
        };
        let Extra::Dissolving { overfill } = actor.extra else {
            return false;
        };
        if overfill < actor.max_hp {
            return false;
        }
        let (kind, pos) = (actor.kind, actor.pos);
        let drops = self.nearest_no_actor(pos);
        if let Some(cell) = self.pick(&drops)
            && let Some(product) = self.digest_product(kind)
        {
            self.add_actor(product, cell);
            self.set_overfill(id, 0);
        }
        true
    }

    fn set_overfill(&mut self, id: ActorId, value: i32) {
        if let Some(actor) = self.actors.get_mut(id)
            && let Extra::Dissolving { overfill } = &mut actor.extra
        {
            *overfill = value;
        }
    }

    /// What a finished corpse yields. A caravan's cargo is a coin flip.
    fn digest_product(&mut self, kind: Kind) -> Option<Kind> {
        let options = kind.digests_to();
        self.pick(options)
    }

    // -- death --------------------------------------------------------------

    /// Kill an organelle by melee: it drops only its first component.
    ///
    /// Attacking a dissolving human instead *rescues* it, putting the live
    /// human back on its feet — which is why leaving corpses on your frontier
    /// is a bad idea.
    pub fn destroy_organelle(&mut self, id: ActorId) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let (kind, pos) = (actor.kind, actor.pos);
        let components = actor.components();
        if let Some(live) = kind.rescues_to() {
            self.remove_actor(id);
            self.add_actor(live, pos);
            self.messages
                .add(&format!("The {} struggles back to its feet!", kind.name()));
            return;
        }
        let was_nucleus = actors::is_nucleus_family(kind);
        self.remove_actor(id);
        // Cytoplasm drops nothing when destroyed; everything else leaves the
        // first thing it was built out of.
        if kind != Kind::Cytoplasm
            && let Some(first) = components.first()
        {
            self.become_item(*first, pos);
        }
        if was_nucleus {
            self.cue(Cue::NucleusLost);
            self.handle_game_over();
        }
    }

    /// Strip an organelle out of the mass: it drops everything it was made of.
    pub fn unslime_organelle(&mut self, id: ActorId) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let (kind, pos) = (actor.kind, actor.pos);
        let components = actor.components();
        if let Some(live) = kind.rescues_to() {
            self.remove_actor(id);
            self.add_actor(live, pos);
            return;
        }
        let was_nucleus = actors::is_nucleus_family(kind);
        self.remove_actor(id);
        for component in components {
            self.become_item(component, pos);
        }
        if was_nucleus {
            self.cue(Cue::NucleusLost);
            self.handle_game_over();
        }
    }

    /// Drop an item as near to `from` as there is room for.
    pub(crate) fn become_item(&mut self, kind: ItemKind, from: Coord) {
        let drops = self.nearest_loot_drops(from);
        if let Some(cell) = self.pick(&drops) {
            self.add_item(kind, cell);
        } else {
            self.messages.add(&format!(
                "The {} had nowhere to drop and was crushed!",
                kind.name()
            ));
        }
    }

    /// A nucleus dodges death by trading places with a neighbouring
    /// organelle, which takes the hit instead.
    ///
    /// Somewhere nothing is aiming is preferred, so a nucleus does not dive out
    /// of a militiaman's way into a hunter's line — but if that is the only
    /// cover there is, it takes it.
    ///
    /// Returns the sacrifice, or `None` if there was nobody to hide behind.
    pub fn retreat(&mut self, nucleus: ActorId) -> Option<ActorId> {
        let pos = self.actors.get(nucleus)?.pos;
        let sacrifices: Vec<ActorId> = self
            .player_mass
            .iter()
            .copied()
            .filter(|&id| {
                id != nucleus
                    && self.actors.get(id).is_some_and(|a| {
                        a.pos.taxi(pos) == 1
                            && !actors::is_nucleus_family(a.kind)
                            && actors::is_organelle(a.kind)
                    })
            })
            .collect();
        let unpainted: Vec<ActorId> = sacrifices
            .iter()
            .copied()
            .filter(|&id| self.actors.get(id).is_some_and(|a| !self.reticle_at(a.pos)))
            .collect();
        let pool = if unpainted.is_empty() {
            &sacrifices
        } else {
            &unpainted
        };
        let chosen = self.pick(pool)?;
        self.swap_actors(nucleus, chosen);
        Some(chosen)
    }

    /// End the run if the last nucleus is gone.
    pub fn handle_game_over(&mut self) {
        let alive = self.player_mass.iter().any(|&id| {
            self.actors
                .get(id)
                .is_some_and(|a| actors::is_nucleus_family(a.kind))
        });
        if alive {
            return;
        }
        self.messages.add(&format!(
            "You lose. Final Score: {}.",
            self.player_mass.len()
        ));
        self.messages
            .add("Press ESC to quit. Press R to play again.");
        self.schedule.clear();
        self.phase = Phase::GameOver { won: false };
        self.player_turn = true;
        self.active = None;
        self.cue(Cue::Lose);
    }

    // -- gates --------------------------------------------------------------

    /// Bring a gate down and check whether that was the last one you needed.
    pub(crate) fn destroy_city(&mut self, id: ActorId) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let pos = actor.pos;
        self.remove_actor(id);
        // The gate's tile becomes open cavern, and you have already seen it.
        self.grid.set_props(pos, true, true, true);
        self.cue(Cue::CityDestroyed);
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let remaining = self.cities.len() as i32;
        if remaining <= self.rules.grace_cities {
            self.win();
            return;
        }
        self.messages
            .add("The humans trigger a cave-in, blocking off this exit to the surface!");
        let charges = remaining - self.rules.grace_cities - 1;
        if charges > 0 {
            let plural = if charges == 1 { "" } else { "s" };
            self.messages.add(&format!(
                "You were able to glimpse their stockpile: The humans have {charges} blast charge{plural} remaining..."
            ));
        } else {
            self.messages
                .add("The humans are out of blast charges! Now is your chance, find an exit!");
        }
    }

    /// Escape.
    fn win(&mut self) {
        self.messages.add(
            "The humans try to trigger a cave-in without a blast charge, but you slip through just in time!",
        );
        self.messages
            .add("You escape to the surface and live out the rest of your days in peace.");
        self.messages.add(&format!(
            "Final score: {}. Time to win (A turn is 16 time units): {}.",
            self.player_mass.len(),
            self.schedule.time()
        ));
        self.messages.add("Thanks for playing!.");
        self.schedule.clear();
        self.phase = Phase::GameOver { won: true };
        self.player_turn = true;
        self.active = None;
        self.cue(Cue::Win);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::tests::{line, sandbox};
    use crate::sim::{Command, Phase, UiMode};

    #[test]
    fn a_straight_chain_shuffles_forward() {
        let mut sim = sandbox(1);
        let ids = line(
            &mut sim,
            &[Kind::Nucleus, Kind::Cytoplasm, Kind::Cytoplasm],
            Coord::new(5, 5),
            Coord::new(1, 0),
        );
        assert!(sim.attack_move(ids[0], Coord::new(4, 5)));
        assert_eq!(sim.actors[ids[0]].pos, Coord::new(4, 5));
        assert_eq!(sim.actors[ids[1]].pos, Coord::new(5, 5));
        assert_eq!(sim.actors[ids[2]].pos, Coord::new(6, 5));
        // The tail's original cell is free again.
        assert!(sim.grid.walkable(Coord::new(7, 5)));
    }

    #[test]
    fn only_the_deepest_tail_is_dragged() {
        // A T: the mover at the junction with a long arm and a short one. Only
        // the long arm can be deepest, so the short arm never moves.
        let mut sim = sandbox(2);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(10, 10));
        let long = line(
            &mut sim,
            &[Kind::Cytoplasm, Kind::Cytoplasm, Kind::Cytoplasm],
            Coord::new(11, 10),
            Coord::new(1, 0),
        );
        let short = sim.add_actor(Kind::Cytoplasm, Coord::new(10, 11));
        assert!(sim.attack_move(root, Coord::new(9, 10)));
        assert_eq!(sim.actors[short].pos, Coord::new(10, 11));
        assert_eq!(sim.actors[long[0]].pos, Coord::new(10, 10));
        assert_eq!(sim.actors[long[2]].pos, Coord::new(12, 10));
    }

    #[test]
    fn tied_tails_are_chosen_by_the_seeded_rng() {
        // Two arms of equal length: exactly one of them moves, the same one
        // every time for a given seed, and both are reachable across seeds.
        let mut moved_left = 0;
        let mut moved_right = 0;
        for seed in 0..24 {
            let mut sim = sandbox(seed);
            let root = sim.add_actor(Kind::Nucleus, Coord::new(10, 5));
            let west = sim.add_actor(Kind::Cytoplasm, Coord::new(9, 5));
            let east = sim.add_actor(Kind::Cytoplasm, Coord::new(11, 5));
            assert!(sim.attack_move(root, Coord::new(10, 4)));
            let west_moved = sim.actors[west].pos == Coord::new(10, 5);
            let east_moved = sim.actors[east].pos == Coord::new(10, 5);
            assert!(west_moved ^ east_moved, "exactly one tail follows");
            moved_left += i32::from(west_moved);
            moved_right += i32::from(east_moved);
        }
        assert!(
            moved_left > 0 && moved_right > 0,
            "both tails are reachable"
        );
    }

    #[test]
    fn the_highlight_covers_every_tied_tail() {
        let mut sim = sandbox(3);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(10, 5));
        let west = sim.add_actor(Kind::Cytoplasm, Coord::new(9, 5));
        let east = sim.add_actor(Kind::Cytoplasm, Coord::new(11, 5));
        sim.set_active_nucleus(root);
        assert_eq!(sim.actors[root].slime, 2);
        assert_eq!(sim.actors[west].slime, 2);
        assert_eq!(sim.actors[east].slime, 2);
        // A shorter branch is body, not path.
        let mut sim = sandbox(3);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(10, 5));
        let far = line(
            &mut sim,
            &[Kind::Cytoplasm, Kind::Cytoplasm],
            Coord::new(11, 5),
            Coord::new(1, 0),
        );
        let near = sim.add_actor(Kind::Cytoplasm, Coord::new(9, 5));
        sim.set_active_nucleus(root);
        assert_eq!(sim.actors[near].slime, 1);
        assert_eq!(sim.actors[far[1]].slime, 2);
    }

    #[test]
    fn anchored_organelles_are_left_out_of_the_drag() {
        let mut sim = sandbox(4);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        let anchored = sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        let behind = sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        sim.actors[anchored].anchor = true;
        assert!(sim.attack_move(root, Coord::new(4, 5)));
        assert_eq!(sim.actors[anchored].pos, Coord::new(6, 5));
        assert_eq!(sim.actors[behind].pos, Coord::new(7, 5));
    }

    #[test]
    fn walking_into_your_own_body_swaps_without_dragging() {
        let mut sim = sandbox(5);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        let neighbour = sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        let tail = sim.add_actor(Kind::Cytoplasm, Coord::new(7, 5));
        assert!(sim.attack_move(root, Coord::new(6, 5)));
        assert_eq!(sim.actors[root].pos, Coord::new(6, 5));
        assert_eq!(sim.actors[neighbour].pos, Coord::new(5, 5));
        assert_eq!(sim.actors[tail].pos, Coord::new(7, 5), "the tail stays put");
    }

    #[test]
    fn walking_into_rock_costs_nothing() {
        let mut sim = sandbox(6);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(1, 1));
        assert!(!sim.attack_move(root, Coord::new(0, 1)));
        assert_eq!(sim.actors[root].pos, Coord::new(1, 1));
    }

    #[test]
    fn eating_a_human_leaves_the_corpse_behind_you() {
        let mut sim = sandbox(7);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        let militia = sim.add_actor(Kind::Militia, Coord::new(6, 5));
        assert!(sim.attack_move(root, Coord::new(6, 5)));
        assert!(!sim.actors.contains(militia));
        assert_eq!(sim.actors[root].pos, Coord::new(6, 5));
        let corpse = sim.actor_at(Coord::new(5, 5)).expect("a corpse behind us");
        assert_eq!(sim.actors[corpse].kind, Kind::DissolvingMilitia);
        assert_eq!(sim.actors[corpse].hp, 8);
        assert!(sim.player_mass.contains(&corpse));
    }

    #[test]
    fn armour_turns_away_anything_but_a_laser_or_a_reinforced_maw() {
        let mut sim = sandbox(8);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        let tank = sim.add_actor(Kind::Tank, Coord::new(6, 5));
        assert!(!sim.attack_move(root, Coord::new(6, 5)));
        assert!(sim.actors.contains(tank));
        sim.actors[root].kind = Kind::LaserCore;
        assert!(sim.attack_move(root, Coord::new(6, 5)));
        assert!(!sim.actors.contains(tank));
    }

    #[test]
    fn a_nutrient_grows_the_mass_by_one() {
        let mut sim = sandbox(9);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        sim.add_item(ItemKind::Nutrient, Coord::new(6, 5));
        let before = sim.mass();
        assert!(sim.attack_move(root, Coord::new(6, 5)));
        assert_eq!(sim.mass(), before + 1);
        assert_eq!(sim.actors[root].pos, Coord::new(6, 5));
        assert!(sim.item_at(Coord::new(6, 5)).is_none());
    }

    #[test]
    fn a_catalyst_spends_the_nearest_cytoplasm() {
        let mut sim = sandbox(10);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        // A membrane sits between the nucleus and the far cytoplasm, so the
        // ring search has to walk past it.
        let near = sim.add_actor(Kind::Cytoplasm, Coord::new(5, 6));
        let far = sim.add_actor(Kind::Cytoplasm, Coord::new(5, 7));
        sim.add_item(ItemKind::BarbedWire, Coord::new(6, 5));
        let before = sim.mass();
        assert!(sim.attack_move(root, Coord::new(6, 5)));
        assert_eq!(sim.mass(), before, "a cytoplasm was spent, not gained");
        assert!(!sim.actors.contains(near), "the nearest ring is used first");
        assert!(sim.actors.contains(far));
        // The membrane grows where the spent cytoplasm was; the nucleus then
        // steps onto the wire's tile, which drags the chain up behind it.
        let membranes = sim
            .player_mass
            .iter()
            .filter(|id| sim.actors[**id].kind == Kind::Membrane)
            .count();
        assert_eq!(membranes, 1);
        assert_eq!(sim.actors[root].pos, Coord::new(6, 5));
    }

    #[test]
    fn every_catalyst_grows_what_the_table_says() {
        for (item, grown) in [
            (ItemKind::Nutrient, Kind::Cytoplasm),
            (ItemKind::CalciumDust, Kind::Calcium),
            (ItemKind::SiliconDust, Kind::Electronics),
            (ItemKind::BarbedWire, Kind::Membrane),
            (ItemKind::Plant, Kind::Chloroplast),
            (ItemKind::Dna, Kind::Nucleus),
        ] {
            let mut sim = sandbox(40);
            let root = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
            // One spare cytoplasm, which every catalyst but a nutrient spends.
            sim.add_actor(Kind::Cytoplasm, Coord::new(5, 6));
            sim.add_item(item, Coord::new(6, 5));
            let before = sim.mass();
            assert!(sim.attack_move(root, Coord::new(6, 5)), "{item:?}");
            let made = sim
                .player_mass
                .iter()
                .filter(|id| sim.actors[**id].kind == grown)
                .count();
            assert!(made >= 1, "{item:?} did not grow a {grown:?}");
            let expected = if item == ItemKind::Nutrient {
                before + 1
            } else {
                before
            };
            assert_eq!(sim.mass(), expected, "{item:?} changed the mass wrongly");
            assert!(sim.item_at(Coord::new(6, 5)).is_none(), "{item:?} lingered");
            assert_eq!(sim.actors[root].pos, Coord::new(6, 5), "{item:?}");
        }
    }

    #[test]
    fn a_cytoplasm_eating_a_catalyst_transforms_where_it_stands() {
        let mut sim = sandbox(41);
        sim.add_actor(Kind::Nucleus, Coord::new(9, 9));
        let cell = sim.add_actor(Kind::Cytoplasm, Coord::new(5, 5));
        sim.add_item(ItemKind::Plant, Coord::new(6, 5));
        let before = sim.mass();
        assert!(sim.attack_move(cell, Coord::new(6, 5)));
        assert!(!sim.actors.contains(cell), "it spent itself");
        let grown = sim.actor_at(Coord::new(6, 5)).expect("a chloroplast");
        assert_eq!(sim.actors[grown].kind, Kind::Chloroplast);
        assert_eq!(sim.mass(), before);
    }

    #[test]
    fn a_catalyst_with_no_cytoplasm_is_just_walked_over() {
        let mut sim = sandbox(11);
        let root = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        sim.add_item(ItemKind::Dna, Coord::new(6, 5));
        assert!(sim.attack_move(root, Coord::new(6, 5)));
        assert_eq!(sim.actors[root].pos, Coord::new(6, 5));
        assert!(
            sim.item_at(Coord::new(6, 5)).is_some(),
            "the DNA is still there"
        );
    }

    #[test]
    fn a_sealed_human_is_engulfed() {
        let mut sim = sandbox(12);
        // Wall the militia in on three sides, then close the fourth.
        let militia = sim.add_actor(Kind::Militia, Coord::new(1, 1));
        sim.add_actor(Kind::Cytoplasm, Coord::new(2, 1));
        let root = sim.add_actor(Kind::Nucleus, Coord::new(1, 3));
        assert!(sim.attack_move(root, Coord::new(1, 2)));
        assert!(!sim.actors.contains(militia));
        let corpse = sim
            .actor_at(Coord::new(1, 1))
            .expect("a corpse in the corner");
        assert_eq!(sim.actors[corpse].kind, Kind::DissolvingMilitia);
    }

    #[test]
    fn an_escape_route_prevents_engulfing() {
        let mut sim = sandbox(13);
        let militia = sim.add_actor(Kind::Militia, Coord::new(2, 1));
        let root = sim.add_actor(Kind::Nucleus, Coord::new(2, 3));
        // (1,1) and (3,1) are open floor, so the militia can still walk out.
        assert!(sim.attack_move(root, Coord::new(2, 2)));
        assert!(sim.actors.contains(militia));
        assert_eq!(sim.actors[militia].kind, Kind::Militia);
    }

    #[test]
    fn a_gate_next_to_the_group_breaks_the_seal() {
        let mut sim = sandbox(14);
        // Corner pocket at (1,1), sealed except by our approach — but with a
        // gate embedded in the rock beside it.
        sim.grid.set_props(Coord::new(1, 0), false, false, true);
        let city = sim.add_actor(Kind::City, Coord::new(1, 0));
        let militia = sim.add_actor(Kind::Militia, Coord::new(1, 1));
        sim.add_actor(Kind::Cytoplasm, Coord::new(2, 1));
        let root = sim.add_actor(Kind::Nucleus, Coord::new(1, 3));
        assert!(sim.attack_move(root, Coord::new(1, 2)));
        assert!(sim.actors.contains(militia), "the gate protects its own");
        assert!(sim.actors.contains(city));
    }

    #[test]
    fn a_whole_clump_is_engulfed_at_once() {
        let mut sim = sandbox(15);
        let a = sim.add_actor(Kind::Militia, Coord::new(1, 1));
        let b = sim.add_actor(Kind::Militia, Coord::new(1, 2));
        sim.add_actor(Kind::Cytoplasm, Coord::new(2, 1));
        sim.add_actor(Kind::Cytoplasm, Coord::new(2, 2));
        let root = sim.add_actor(Kind::Nucleus, Coord::new(1, 4));
        assert!(sim.attack_move(root, Coord::new(1, 3)));
        assert!(!sim.actors.contains(a));
        assert!(!sim.actors.contains(b));
    }

    #[test]
    fn digestion_ticks_down_and_yields_a_product() {
        let mut sim = sandbox(16);
        let corpse = sim.add_actor(Kind::DissolvingMilitia, Coord::new(5, 5));
        for _ in 0..7 {
            sim.digest(corpse);
            assert!(sim.actors.contains(corpse));
        }
        sim.digest(corpse);
        assert!(!sim.actors.contains(corpse));
        let product = sim.actor_at(Coord::new(5, 5)).expect("a product");
        assert_eq!(sim.actors[product].kind, Kind::Cytoplasm);
    }

    #[test]
    fn attacking_a_corpse_rescues_the_human() {
        let mut sim = sandbox(17);
        let corpse = sim.add_actor(Kind::DissolvingTank, Coord::new(5, 5));
        sim.destroy_organelle(corpse);
        let rescued = sim.actor_at(Coord::new(5, 5)).expect("a tank again");
        assert_eq!(sim.actors[rescued].kind, Kind::Tank);
    }

    #[test]
    fn a_nucleus_retreats_into_a_neighbour() {
        let mut sim = sandbox(18);
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        let shield = sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        let other_nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 6));
        let sacrifice = sim.retreat(nucleus).expect("a neighbour to hide behind");
        assert_eq!(sacrifice, shield, "another nucleus cannot be a sacrifice");
        assert_eq!(sim.actors[nucleus].pos, Coord::new(6, 5));
        assert_eq!(sim.actors[shield].pos, Coord::new(5, 5));
        assert_eq!(sim.actors[other_nucleus].pos, Coord::new(5, 6));
    }

    #[test]
    fn a_lone_nucleus_cannot_retreat() {
        let mut sim = sandbox(19);
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        assert!(sim.retreat(nucleus).is_none());
    }

    #[test]
    fn losing_the_last_nucleus_ends_the_run() {
        let mut sim = sandbox(20);
        let first = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        let second = sim.add_actor(Kind::Nucleus, Coord::new(6, 5));
        sim.destroy_organelle(first);
        assert_eq!(sim.phase(), Phase::Playing);
        sim.destroy_organelle(second);
        assert_eq!(sim.phase(), Phase::GameOver { won: false });
        assert!(sim.cues().contains(&Cue::Lose));
    }

    #[test]
    fn a_destroyed_nucleus_drops_only_its_first_component() {
        let mut sim = sandbox(21);
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(8, 8));
        sim.destroy_organelle(nucleus);
        let dropped: Vec<ItemKind> = (0..20)
            .flat_map(|y| (0..20).map(move |x| Coord::new(x, y)))
            .filter_map(|c| sim.item_at(c))
            .map(|id| sim.items[id].kind)
            .collect();
        assert_eq!(dropped, vec![ItemKind::Dna]);
    }

    #[test]
    fn an_unslimed_organelle_drops_everything() {
        let mut sim = sandbox(22);
        sim.add_actor(Kind::Nucleus, Coord::new(8, 8));
        let core = sim.add_actor(Kind::QuantumCore, Coord::new(5, 5));
        sim.unslime_organelle(core);
        let dropped = (0..20)
            .flat_map(|y| (0..20).map(move |x| Coord::new(x, y)))
            .filter_map(|c| sim.item_at(c))
            .count();
        assert_eq!(dropped, Kind::QuantumCore.components().len());
    }

    #[test]
    fn breaking_the_last_needed_gate_wins() {
        let mut sim = sandbox(23);
        sim.rules.grace_cities = 0;
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        // Enough mass to break a gate, without touching the gate's cell.
        for i in 0..8 {
            sim.add_actor(Kind::Cytoplasm, Coord::new(5 + i % 4, 8 + i / 4));
        }
        sim.grid.set_props(Coord::new(6, 5), false, false, true);
        let city = sim.add_actor(Kind::City, Coord::new(6, 5));
        sim.actors[city].armor = 4;
        assert!(sim.attack_move(nucleus, Coord::new(6, 5)));
        assert_eq!(sim.phase(), Phase::GameOver { won: true });
        assert!(sim.cues().contains(&Cue::Win));
        assert!(sim.grid.walkable(Coord::new(6, 5)));
    }

    #[test]
    fn too_little_mass_leaves_the_gate_standing() {
        let mut sim = sandbox(24);
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        sim.grid.set_props(Coord::new(6, 5), false, false, true);
        let city = sim.add_actor(Kind::City, Coord::new(6, 5));
        sim.actors[city].armor = 100;
        assert!(!sim.attack_move(nucleus, Coord::new(6, 5)));
        assert!(sim.actors.contains(city));
        assert_eq!(sim.phase(), Phase::Playing);
    }

    #[test]
    fn cycling_nuclei_wraps_in_acquisition_order() {
        let mut sim = sandbox(25);
        let first = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        let second = sim.add_actor(Kind::Nucleus, Coord::new(6, 5));
        sim.set_active_nucleus(first);
        sim.next_nucleus(1);
        assert_eq!(sim.active, Some(second));
        sim.next_nucleus(1);
        assert_eq!(sim.active, Some(first));
        sim.next_nucleus(-1);
        assert_eq!(sim.active, Some(second));
    }

    #[test]
    fn every_nucleus_is_rescheduled_together() {
        let mut sim = sandbox(26);
        let slow = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        let fast = sim.add_actor(Kind::SmartCore, Coord::new(6, 5));
        sim.set_active_nucleus(fast);
        assert_eq!(sim.schedule.scheduled_for(fast), Some(8));
        assert_eq!(
            sim.schedule.scheduled_for(slow),
            Some(8),
            "the slow nucleus adopts the active one's speed"
        );
    }

    #[test]
    fn a_restart_command_begins_a_fresh_run() {
        let mut sim = sandbox(27);
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        sim.destroy_organelle(nucleus);
        assert_eq!(sim.phase(), Phase::GameOver { won: false });
        sim.advance(Some(Command::Restart { seed: 99 }));
        assert_eq!(sim.phase(), Phase::Playing);
        assert_eq!(sim.mass(), 6);
        assert_eq!(sim.mode(), UiMode::Messages);
    }
}
