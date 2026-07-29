//! What the organelles do when nobody is steering them.
//!
//! Most of your body is inert, but the parts that are not run the whole
//! economy. Chloroplasts turn time into mass on a sixteen-turn cycle.
//! Crafting materials wander into a neighbour and upgrade it. Maws hunt.
//! Cultivators keep corpses from finishing too soon and squeeze extra product
//! out of them. And the nucleus line has a special each, from a gravity well
//! that drags the mass along to a core that frightens humans out of their turn.
//!
//! Two rules here look wrong and are kept because the game plays on them. Every
//! member of the chloroplast line produces at exactly the same rate — the
//! constructors that set a faster delay overwrite it a line later — and the
//! primordial soup's nucleus cap counts organelles whose *name* is "Nucleus",
//! so upgraded cores slip past it.

use super::actors::{self, ActorId, Extra, Kind, Resource};
use super::grid::{Coord, rand_index};
use super::{Cue, Sim, Terrified};

/// Cells a gravity core reaches for.
const GRAVITY_RANGE: i32 = 2;
/// Organelles a gravity core pulls per move.
const GRAVITY_ATTEMPTS: u32 = 2;
/// How much health a cultivator puts back into a corpse each turn.
const CULTIVATOR_HEAL: i32 = 2;
/// How much a cultivator overfills a corpse each turn.
const OVERFILL_RATE: i32 = 1;
/// Unupgraded nuclei the primordial soup will make more than.
const SOUP_NUCLEUS_CAP: usize = 4;

impl Sim {
    // -- crafting -----------------------------------------------------------

    /// Offer a crafting material to one specific organelle.
    ///
    /// This is the route a nucleus is upgraded by, and the only one: a material
    /// left to its own devices deliberately skips nuclei, so the upgrade has to
    /// be delivered by walking into it.
    pub(crate) fn try_upgrade(&mut self, material: ActorId, recipient: ActorId) -> bool {
        let Some(resource) = self.actors.get(material).and_then(|a| a.kind.provides()) else {
            return false;
        };
        if !self.upgrade(recipient, resource) {
            return false;
        }
        self.spend_material(material);
        true
    }

    /// Consume a crafting material, leaving a cytoplasm where it stood so the
    /// mass is conserved.
    fn spend_material(&mut self, material: ActorId) {
        let Some(pos) = self.actors.get(material).map(|a| a.pos) else {
            return;
        };
        self.remove_actor(material);
        self.add_actor(Kind::Cytoplasm, pos);
    }

    /// Put one unit of `resource` into an organelle.
    ///
    /// The first material absorbed locks in which of the two upgrade paths this
    /// organelle is on; the wrong material afterwards is refused. Returns
    /// whether the material was taken.
    pub(crate) fn upgrade(&mut self, id: ActorId, resource: Resource) -> bool {
        let Some(actor) = self.actors.get(id) else {
            return false;
        };
        let kind = actor.kind;
        let paths = kind.upgrade_paths();
        let Some(state) = actor.extra.upgrade() else {
            return false;
        };
        let index = if let Some(locked) = state.path {
            locked
        } else if let Some(first) = paths.iter().position(|p| p.resource == resource) {
            first
        } else {
            return false;
        };
        let Some(path) = paths.get(index) else {
            return false;
        };
        if path.resource != resource {
            return false;
        }
        let (amount, result) = (path.amount, path.result);
        let Some(state) = self.actors[id].extra.upgrade_mut() else {
            return false;
        };
        state.path = Some(index);
        state.progress += 1;
        let progress = state.progress;
        let old_name = kind.name();
        let material = resource.name();
        if progress < amount {
            self.messages
                .add(&format!("The {old_name} absorbs the {material}"));
            return true;
        }
        let pos = self.actors[id].pos;
        self.remove_actor(id);
        let grown = self.add_actor(result, pos);
        self.messages.add(&format!(
            "The {old_name} absorbs the {material} and transforms into a {}!",
            result.name()
        ));
        // Control follows the upgrade, so growing a new core out of the nucleus
        // you were steering does not hand the turn to a different one.
        if actors::is_nucleus_family(result) {
            self.set_active_nucleus(grown);
        }
        self.cue(Cue::Upgrade);
        true
    }

    /// A crafting material's turn: find a neighbour it can improve and spend
    /// itself on it.
    ///
    /// Nuclei are skipped, which is what makes walking a nucleus into a
    /// material the only way to upgrade one.
    pub(crate) fn craft(&mut self, id: ActorId) {
        let Some(resource) = self.actors.get(id).and_then(|a| a.kind.provides()) else {
            return;
        };
        let pos = self.actors[id].pos;
        let mut candidates: Vec<ActorId> = self
            .grid
            .adjacent(pos)
            .filter_map(|c| self.actor_at(c))
            .filter(|&a| {
                self.actors.get(a).is_some_and(|x| {
                    actors::is_upgradable(x.kind) && !actors::is_nucleus_family(x.kind)
                })
            })
            .collect();
        while let Some(pick) = rand_index(&mut self.rng, candidates.len()) {
            let recipient = candidates.remove(pick);
            if self.upgrade(recipient, resource) {
                self.spend_material(id);
                return;
            }
        }
    }

    // -- the chloroplast line -----------------------------------------------

    /// One turn of a producer's cycle.
    ///
    /// KEPT from C# (§11.5): every member of the line runs on the same
    /// sixteen-turn cycle. The bioreactor's constructor sets a delay of ten and
    /// then overwrites it with sixteen on the very next line, and so do the
    /// forge and the soup. The only real difference between a chloroplast and a
    /// bioreactor is that the bioreactor's first product comes four turns
    /// sooner.
    pub(crate) fn produce_act(&mut self, id: ActorId) {
        let Extra::Chloroplast { next_food, .. } = &mut self.actors[id].extra else {
            return;
        };
        *next_food -= 1;
        if *next_food > 0 {
            return;
        }
        if !self.produce(id) {
            return;
        }
        let cycle = self.actors[id].delay;
        if let Extra::Chloroplast { next_food, .. } = &mut self.actors[id].extra {
            *next_food = cycle;
        }
        self.cue(Cue::Produce);
    }

    /// Spawn this producer's product on the nearest free cell.
    fn produce(&mut self, id: ActorId) -> bool {
        let Some(actor) = self.actors.get(id) else {
            return false;
        };
        let (kind, pos) = (actor.kind, actor.pos);
        let drops = self.nearest_no_actor(pos);
        let Some(cell) = self.pick(&drops) else {
            return false;
        };
        let Some(product) = self.product_of(kind) else {
            return false;
        };
        self.add_actor(product, cell);
        true
    }

    /// What a given producer makes this time.
    fn product_of(&mut self, kind: Kind) -> Option<Kind> {
        match kind {
            Kind::Chloroplast | Kind::Bioreactor => Some(Kind::Cytoplasm),
            Kind::BiometalForge => self.pick(&[Kind::Calcium, Kind::Electronics]),
            Kind::PrimordialSoup => {
                // KEPT from C# (§11.15): the cap counts organelles literally
                // named "Nucleus", so an eye core or a smart core does not
                // count toward it and the soup will happily make more.
                let plain = self
                    .player_mass
                    .iter()
                    .filter(|id| {
                        self.actors
                            .get(**id)
                            .is_some_and(|a| actors::is_unupgraded_nucleus(a.kind))
                    })
                    .count();
                let choice = if plain > SOUP_NUCLEUS_CAP {
                    super::grid::rand_inclusive(&mut self.rng, 0, 3)
                } else {
                    super::grid::rand_inclusive(&mut self.rng, 0, 4)
                };
                Some(if choice < 2 {
                    Kind::Membrane
                } else if choice < 4 {
                    Kind::Chloroplast
                } else {
                    Kind::Nucleus
                })
            }
            _ => None,
        }
    }

    /// A cultivator's turn: hold adjacent corpses together and overfill them.
    ///
    /// Two health a turn out-heals the one a corpse loses on its own, so a
    /// tended corpse never finishes — it just keeps handing over free product.
    pub(crate) fn cultivate(&mut self, id: ActorId) {
        let Some(pos) = self.actors.get(id).map(|a| a.pos) else {
            return;
        };
        let corpses: Vec<ActorId> = self
            .grid
            .adjacent(pos)
            .filter_map(|c| self.actor_at(c))
            .filter(|&a| {
                self.actors
                    .get(a)
                    .is_some_and(|x| actors::is_dissolving(x.kind))
            })
            .collect();
        for corpse in corpses {
            if let Some(actor) = self.actors.get_mut(corpse) {
                if actor.hp < actor.max_hp {
                    actor.hp += CULTIVATOR_HEAL.min(actor.max_hp - actor.hp);
                }
                if let Extra::Dissolving { overfill } = &mut actor.extra {
                    *overfill += OVERFILL_RATE;
                }
            }
            self.produce_if_overfull(corpse);
        }
    }

    /// An extractor's turn: haul distant corpses in toward itself, then behave
    /// as a cultivator.
    ///
    /// KEPT from C# (§11.6): the "nearest" filter on the candidate list is a
    /// typo that compares each corpse's distance to itself, so it is always
    /// true. Every corpse in the mass that is not already touching the
    /// extractor is a candidate, not merely the closest one.
    pub(crate) fn extract(&mut self, id: ActorId) {
        let Some(pos) = self.actors.get(id).map(|a| a.pos) else {
            return;
        };
        let mut destinations: Vec<Coord> = self.grid.adjacent_walkable(pos).collect();
        let neighbours: Vec<Coord> = self.grid.adjacent(pos).collect();
        for cell in neighbours {
            if let Some(other) = self.actor_at(cell)
                && self
                    .actors
                    .get(other)
                    .is_some_and(|a| !actors::is_dissolving(a.kind))
            {
                destinations.push(cell);
            }
        }
        let mut order = Vec::with_capacity(destinations.len());
        while let Some(pick) = rand_index(&mut self.rng, destinations.len()) {
            order.push(destinations.remove(pick));
        }
        for dest in order {
            let mut wants = self.needs_to_reach(id);
            if wants.is_empty() {
                break;
            }
            while let Some(pick) = rand_index(&mut self.rng, wants.len()) {
                let goer = wants[pick];
                let Some(from) = self.actors.get(goer).map(|a| a.pos) else {
                    wants.remove(pick);
                    continue;
                };
                if let Some(path) = self.extractor_path(id, from, dest) {
                    if path.len() >= 2 {
                        self.attack_move(goer, path[1]);
                    }
                    break;
                }
                wants.remove(pick);
            }
        }
    }

    /// Corpses in the mass that are not already beside this extractor.
    fn needs_to_reach(&self, id: ActorId) -> Vec<ActorId> {
        let Some(pos) = self.actors.get(id).map(|a| a.pos) else {
            return Vec::new();
        };
        self.player_mass
            .iter()
            .copied()
            .filter(|&m| {
                self.actors
                    .get(m)
                    .is_some_and(|a| actors::is_dissolving(a.kind) && !a.pos.adjacent_to(pos))
            })
            .collect()
    }

    /// A route for a hauled corpse: through the rest of your body, but not
    /// through another extractor and not through a corpse already being tended.
    fn extractor_path(&self, extractor: ActorId, from: Coord, to: Coord) -> Option<Vec<Coord>> {
        let anchor = self.actors.get(extractor)?.pos;
        self.shortest_paths_to(from, &[to], |sim, c| {
            if sim.grid.walkable(c) {
                return true;
            }
            sim.actor_at(c).is_some_and(|a| {
                sim.actors.get(a).is_some_and(|x| {
                    x.slime > 0
                        && x.kind != Kind::Extractor
                        && !(actors::is_dissolving(x.kind) && x.pos.adjacent_to(anchor))
                })
            })
        })
        .into_iter()
        .next()
    }

    // -- the nucleus line ---------------------------------------------------

    /// A gravity core's wake: up to two nearby organelles are yanked one step
    /// toward an empty cell beside it.
    ///
    /// The core anchors itself first, which takes it out of the drag tree so
    /// the organelles it is hauling cannot haul it back.
    ///
    /// DELIBERATE CHANGE from C#: a core already mid-pull refuses to start
    /// another one. Two gravity cores in reach of each other could otherwise
    /// pull one another back and forth without ever unwinding the stack.
    pub(crate) fn gravity_pull(&mut self, id: ActorId) {
        if !self.actors.contains(id) || self.actors[id].anchor {
            return;
        }
        self.actors[id].anchor = true;
        for _ in 0..GRAVITY_ATTEMPTS {
            let Some(pos) = self.actors.get(id).map(|a| a.pos) else {
                break;
            };
            let free: Vec<Coord> = self.grid.adjacent_walkable(pos).collect();
            let Some(toward) = self.pick(&free) else {
                continue;
            };
            let mut nearest = self.nearest_actors(pos, |sim, other| {
                other != id
                    && sim.actors.get(other).is_some_and(|a| {
                        actors::is_organelle(a.kind) && a.pos.taxi(toward) <= GRAVITY_RANGE
                    })
                    && sim
                        .actors
                        .get(other)
                        .is_some_and(|a| sim.path_ignoring_militia(a.pos, toward).is_some())
            });
            while let Some(pick) = rand_index(&mut self.rng, nearest.len()) {
                let hauled = nearest.remove(pick);
                let Some(from) = self.actors.get(hauled).map(|a| a.pos) else {
                    continue;
                };
                if let Some(path) = self.path_ignoring_militia(from, toward) {
                    if path.len() >= 2 {
                        self.attack_move(hauled, path[1]);
                    }
                    break;
                }
            }
        }
        if self.actors.contains(id) {
            self.actors[id].anchor = false;
        }
    }

    /// A terror core's wake: adjacent humans forget how to take their turn.
    ///
    /// Each one is lifted off the schedule and has its own delay inflated by
    /// however long it had left plus a whole turn, so it comes back roughly two
    /// turns late.
    pub(crate) fn terrify(&mut self, id: ActorId) {
        self.terrified.retain(|t| t.core != id);
        let Some(pos) = self.actors.get(id).map(|a| a.pos) else {
            return;
        };
        let victims: Vec<ActorId> = self
            .grid
            .adjacent(pos)
            .filter_map(|c| self.actor_at(c))
            .filter(|&a| {
                self.actors
                    .get(a)
                    .is_some_and(|x| actors::is_militia_family(x.kind))
            })
            .collect();
        let now = self.schedule.time();
        for victim in victims {
            let Some(at) = self.schedule.scheduled_for(victim) else {
                let name = self.actors[victim].name;
                self.messages.add(&format!("{name} is already terrified."));
                continue;
            };
            let until = i32::try_from(at.saturating_sub(now)).unwrap_or(i32::MAX);
            self.schedule.remove(victim);
            let delay = self.actors[victim].delay;
            self.terrified.push(Terrified {
                core: id,
                victim,
                delay,
            });
            self.actors[victim].delay = delay.saturating_add(until).saturating_add(16);
            self.cue(Cue::Terrify);
        }
    }

    /// Hand everything a terror core froze back to the schedule.
    ///
    /// DELIBERATE CHANGE from C#: the list is emptied here. The original only
    /// emptied it when the core *moved*, so a player who terrified somebody and
    /// then waited put the same humans back on the schedule twice, and they got
    /// two turns for every one of yours. That is a corrupt schedule rather than
    /// a gameplay quirk, so it is fixed.
    pub(crate) fn terror_post_schedule(&mut self, id: ActorId) {
        let mine: Vec<Terrified> = self
            .terrified
            .iter()
            .copied()
            .filter(|t| t.core == id)
            .collect();
        self.terrified.retain(|t| t.core != id);
        if mine.is_empty() {
            return;
        }
        for entry in mine {
            if !self.actors.contains(entry.victim) {
                continue;
            }
            let delay = self.actors[entry.victim].delay;
            self.schedule.add(entry.victim, delay);
            self.actors[entry.victim].delay = entry.delay;
        }
        self.set_active_nucleus(id);
    }

    /// Put back on the schedule anything a departing terror core was holding,
    /// and forget a victim that has left the world.
    ///
    /// DELIBERATE CHANGE from C#: the original had no such tidy-up, so killing
    /// a terror core left its victims off the clock permanently.
    pub(crate) fn release_terrified(&mut self, gone: ActorId) {
        let held: Vec<Terrified> = self
            .terrified
            .iter()
            .copied()
            .filter(|t| t.core == gone)
            .collect();
        self.terrified
            .retain(|t| t.core != gone && t.victim != gone);
        for entry in held {
            if !self.actors.contains(entry.victim) {
                continue;
            }
            self.actors[entry.victim].delay = entry.delay;
            self.schedule.add(entry.victim, entry.delay);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::actors::ItemKind;
    use crate::sim::tests::sandbox;

    #[test]
    fn a_chloroplast_produces_every_sixteen_turns_after_twenty() {
        let mut sim = sandbox(1);
        let plant = sim.add_actor(Kind::Chloroplast, Coord::new(5, 5));
        let before = sim.mass();
        for turn in 1..20 {
            sim.produce_act(plant);
            assert_eq!(sim.mass(), before, "turn {turn} produced early");
        }
        sim.produce_act(plant);
        assert_eq!(sim.mass(), before + 1, "the first product lands on turn 20");
        for _ in 1..16 {
            sim.produce_act(plant);
        }
        assert_eq!(sim.mass(), before + 1, "and the cycle is sixteen turns");
        sim.produce_act(plant);
        assert_eq!(sim.mass(), before + 2);
    }

    #[test]
    fn a_bioreactor_only_starts_sooner_it_is_not_faster() {
        // KEPT (§11.5): the whole line runs on the same sixteen-turn cycle.
        let mut sim = sandbox(2);
        let reactor = sim.add_actor(Kind::Bioreactor, Coord::new(5, 5));
        for _ in 0..16 {
            sim.produce_act(reactor);
        }
        assert_eq!(sim.mass(), 2, "the reactor and its first cytoplasm");
        for _ in 0..15 {
            sim.produce_act(reactor);
        }
        assert_eq!(sim.mass(), 2);
        sim.produce_act(reactor);
        assert_eq!(sim.mass(), 3);
    }

    #[test]
    fn a_forge_makes_bone_and_circuitry_in_equal_measure() {
        let mut sim = sandbox(3);
        let forge = sim.add_actor(Kind::BiometalForge, Coord::new(10, 10));
        let mut calcium = 0;
        let mut electronics = 0;
        for _ in 0..(16 * 40) {
            sim.produce_act(forge);
        }
        for id in sim.player_mass() {
            match sim.actors[*id].kind {
                Kind::Calcium => calcium += 1,
                Kind::Electronics => electronics += 1,
                _ => {}
            }
        }
        assert_eq!(calcium + electronics, 40);
        assert!(calcium > 8 && electronics > 8, "{calcium} / {electronics}");
    }

    #[test]
    fn primordial_soup_stops_making_plain_nuclei_past_the_cap() {
        let mut sim = sandbox(4);
        let soup = sim.add_actor(Kind::PrimordialSoup, Coord::new(10, 10));
        for _ in 0..(16 * 60) {
            sim.produce_act(soup);
        }
        let nuclei = sim
            .player_mass()
            .iter()
            .filter(|id| sim.actors[**id].kind == Kind::Nucleus)
            .count();
        assert!(nuclei > SOUP_NUCLEUS_CAP, "it made some nuclei: {nuclei}");
        assert!(nuclei <= SOUP_NUCLEUS_CAP + 1, "and then stopped: {nuclei}");
        // Upgraded cores deliberately do not count toward the cap.
        let plain = sim
            .player_mass()
            .iter()
            .filter(|id| actors::is_unupgraded_nucleus(sim.actors[**id].kind))
            .count();
        assert_eq!(plain, nuclei);
    }

    #[test]
    fn a_producer_with_nowhere_to_put_it_keeps_trying() {
        let mut sim = sandbox(5);
        // A one-cell pocket: the producer is walled in with nowhere to drop.
        for cell in [
            Coord::new(4, 5),
            Coord::new(6, 5),
            Coord::new(5, 4),
            Coord::new(5, 6),
        ] {
            sim.grid.set_props(cell, false, false, true);
        }
        let plant = sim.add_actor(Kind::Chloroplast, Coord::new(5, 5));
        for _ in 0..40 {
            sim.produce_act(plant);
        }
        assert_eq!(sim.mass(), 1, "nothing was produced");
        let Extra::Chloroplast { next_food, .. } = sim.actors[plant].extra else {
            panic!("a producer")
        };
        assert!(next_food <= 0, "and the timer is still due");
    }

    #[test]
    fn a_crafting_material_upgrades_a_neighbour_and_becomes_cytoplasm() {
        let mut sim = sandbox(6);
        let calcium = sim.add_actor(Kind::Calcium, Coord::new(5, 5));
        let membrane = sim.add_actor(Kind::Membrane, Coord::new(6, 5));
        sim.craft(calcium);
        assert!(!sim.actors.contains(calcium), "the material was spent");
        assert!(!sim.actors.contains(membrane), "the membrane was replaced");
        let grown = sim.actor_at(Coord::new(6, 5)).expect("something grew");
        assert_eq!(sim.actors[grown].kind, Kind::ReinforcedMembrane);
        let left = sim.actor_at(Coord::new(5, 5)).expect("a byproduct");
        assert_eq!(sim.actors[left].kind, Kind::Cytoplasm);
        assert_eq!(sim.mass(), 2, "mass is conserved");
    }

    #[test]
    fn a_crafting_material_will_not_touch_a_nucleus() {
        let mut sim = sandbox(7);
        let calcium = sim.add_actor(Kind::Calcium, Coord::new(5, 5));
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(6, 5));
        sim.craft(calcium);
        assert!(sim.actors.contains(calcium));
        assert_eq!(sim.actors[nucleus].kind, Kind::Nucleus);
    }

    #[test]
    fn a_material_of_the_wrong_kind_is_refused_once_a_path_is_locked_in() {
        let mut sim = sandbox(8);
        let maw = sim.add_actor(Kind::Maw, Coord::new(5, 5));
        // Three calcium takes a maw to a reinforced maw; one electronics in the
        // middle must be refused.
        assert!(sim.upgrade(maw, Resource::Calcium));
        assert!(!sim.upgrade(maw, Resource::Electronics));
        assert!(sim.upgrade(maw, Resource::Calcium));
        assert!(sim.upgrade(maw, Resource::Calcium));
        let grown = sim.actor_at(Coord::new(5, 5)).expect("a maw");
        assert_eq!(sim.actors[grown].kind, Kind::ReinforcedMaw);
    }

    #[test]
    fn every_upgrade_path_leads_where_the_table_says() {
        let paths = [
            (Kind::Nucleus, Resource::Calcium, 1, Kind::EyeCore),
            (Kind::Nucleus, Resource::Electronics, 2, Kind::SmartCore),
            (Kind::EyeCore, Resource::Calcium, 2, Kind::LaserCore),
            (Kind::EyeCore, Resource::Electronics, 2, Kind::TerrorCore),
            (Kind::SmartCore, Resource::Calcium, 2, Kind::GravityCore),
            (Kind::SmartCore, Resource::Electronics, 3, Kind::QuantumCore),
            (
                Kind::Membrane,
                Resource::Calcium,
                1,
                Kind::ReinforcedMembrane,
            ),
            (Kind::Membrane, Resource::Electronics, 1, Kind::Maw),
            (
                Kind::ReinforcedMembrane,
                Resource::Calcium,
                3,
                Kind::ForceField,
            ),
            (
                Kind::ReinforcedMembrane,
                Resource::Electronics,
                1,
                Kind::PhaseMembrane,
            ),
            (Kind::Maw, Resource::Calcium, 3, Kind::ReinforcedMaw),
            (Kind::Maw, Resource::Electronics, 3, Kind::Tentacle),
            (Kind::Chloroplast, Resource::Calcium, 1, Kind::Bioreactor),
            (
                Kind::Chloroplast,
                Resource::Electronics,
                1,
                Kind::Cultivator,
            ),
            (Kind::Bioreactor, Resource::Calcium, 3, Kind::BiometalForge),
            (
                Kind::Bioreactor,
                Resource::Electronics,
                2,
                Kind::PrimordialSoup,
            ),
            (Kind::Cultivator, Resource::Calcium, 2, Kind::Extractor),
            (Kind::Cultivator, Resource::Electronics, 2, Kind::Butcher),
        ];
        for (from, resource, amount, into) in paths {
            let mut sim = sandbox(9);
            let id = sim.add_actor(from, Coord::new(5, 5));
            for step in 1..amount {
                assert!(sim.upgrade(id, resource), "{from:?} step {step}");
                assert!(sim.actors.contains(id), "{from:?} finished early");
            }
            assert!(sim.upgrade(id, resource), "{from:?} last step");
            let grown = sim.actor_at(Coord::new(5, 5)).expect("something grew");
            assert_eq!(sim.actors[grown].kind, into, "{from:?} + {resource:?}");
            assert!(sim.player_mass().contains(&grown));
        }
    }

    #[test]
    fn upgrading_a_nucleus_keeps_you_in_control_of_it() {
        let mut sim = sandbox(10);
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        let other = sim.add_actor(Kind::Nucleus, Coord::new(9, 9));
        sim.set_active_nucleus(nucleus);
        assert!(sim.upgrade(nucleus, Resource::Calcium));
        let grown = sim.actor_at(Coord::new(5, 5)).expect("an eye core");
        assert_eq!(sim.actors[grown].kind, Kind::EyeCore);
        assert_eq!(sim.active_nucleus(), Some(grown), "control followed it");
        assert_ne!(sim.active_nucleus(), Some(other));
    }

    #[test]
    fn walking_a_nucleus_into_a_material_upgrades_it() {
        let mut sim = sandbox(11);
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 5));
        sim.add_actor(Kind::Calcium, Coord::new(6, 5));
        sim.set_active_nucleus(nucleus);
        assert!(sim.attack_move(nucleus, Coord::new(6, 5)));
        let grown = sim.actor_at(Coord::new(6, 5)).expect("an eye core");
        assert_eq!(sim.actors[grown].kind, Kind::EyeCore);
        assert_eq!(sim.actors[grown].awareness, 6);
        let byproduct = sim.actor_at(Coord::new(5, 5)).expect("a cytoplasm");
        assert_eq!(sim.actors[byproduct].kind, Kind::Cytoplasm);
    }

    #[test]
    fn an_interrupted_upgrade_refunds_the_whole_path() {
        // KEPT (§11.18): components charge the full cost of the locked-in path
        // however little of it has been paid, so a half-built force field drops
        // more dust than went into it.
        let mut sim = sandbox(12);
        let membrane = sim.add_actor(Kind::ReinforcedMembrane, Coord::new(5, 5));
        assert!(sim.upgrade(membrane, Resource::Calcium));
        sim.unslime_organelle(membrane);
        let dust = (0..20)
            .flat_map(|y| (0..20).map(move |x| Coord::new(x, y)))
            .filter_map(|c| sim.item_at(c))
            .filter(|i| sim.items[*i].kind == ItemKind::CalciumDust)
            .count();
        assert_eq!(dust, 4, "one from the membrane and three unpaid");
    }

    #[test]
    fn a_cultivator_outheals_the_corpse_beside_it() {
        let mut sim = sandbox(13);
        let cultivator = sim.add_actor(Kind::Cultivator, Coord::new(5, 5));
        let corpse = sim.add_actor(Kind::DissolvingTank, Coord::new(6, 5));
        for _ in 0..30 {
            sim.digest(corpse);
            sim.cultivate(cultivator);
        }
        assert!(sim.actors.contains(corpse), "it never finishes");
        assert_eq!(sim.actors[corpse].hp, sim.actors[corpse].max_hp);
        let calcium = sim
            .player_mass()
            .iter()
            .filter(|id| sim.actors[**id].kind == Kind::Calcium)
            .count();
        assert!(calcium > 0, "and it hands over free product");
    }

    #[test]
    fn a_butcher_squeezes_an_extra_product_out_of_every_corpse() {
        let mut sim = sandbox(14);
        sim.add_actor(Kind::Butcher, Coord::new(1, 1));
        let corpse = sim.add_actor(Kind::DissolvingMilitia, Coord::new(5, 5));
        for _ in 0..8 {
            sim.digest(corpse);
        }
        let cytoplasm = sim
            .player_mass()
            .iter()
            .filter(|id| sim.actors[**id].kind == Kind::Cytoplasm)
            .count();
        assert_eq!(cytoplasm, 2, "one from the corpse and one from the butcher");
    }

    #[test]
    fn an_extractor_hauls_a_distant_corpse_toward_itself() {
        let mut sim = sandbox(15);
        let extractor = sim.add_actor(Kind::Extractor, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        let corpse = sim.add_actor(Kind::DissolvingMilitia, Coord::new(7, 5));
        let before = sim.actors[corpse].pos;
        sim.extract(extractor);
        assert_ne!(sim.actors[corpse].pos, before, "the corpse was pulled in");
    }

    #[test]
    fn a_gravity_core_drags_its_neighbours_along() {
        let mut sim = sandbox(16);
        let core = sim.add_actor(Kind::GravityCore, Coord::new(5, 5));
        let near = sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        let far = sim.add_actor(Kind::Cytoplasm, Coord::new(15, 15));
        sim.gravity_pull(core);
        assert_ne!(sim.actors[near].pos, Coord::new(6, 5), "the near one moved");
        assert_eq!(
            sim.actors[far].pos,
            Coord::new(15, 15),
            "the far one did not"
        );
        assert_eq!(
            sim.actors[core].pos,
            Coord::new(5, 5),
            "the core stayed put"
        );
        assert!(!sim.actors[core].anchor, "the anchor is released again");
    }

    #[test]
    fn a_gravity_core_does_not_haul_itself() {
        let mut sim = sandbox(17);
        let core = sim.add_actor(Kind::GravityCore, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        sim.gravity_pull(core);
        assert_eq!(sim.actors[core].pos, Coord::new(5, 5));
    }

    #[test]
    fn an_anchored_core_is_left_out_of_the_drag() {
        let mut sim = sandbox(18);
        let core = sim.add_actor(Kind::GravityCore, Coord::new(5, 5));
        let tail = sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        sim.actors[core].anchor = true;
        // With the core anchored the tail has no chain to hang off, so pulling
        // it toward the core cannot drag the core backwards.
        sim.attack_move(tail, Coord::new(6, 4));
        assert_eq!(sim.actors[core].pos, Coord::new(5, 5));
    }

    #[test]
    fn a_terror_core_costs_an_adjacent_human_its_turn() {
        let mut sim = sandbox(19);
        let core = sim.add_actor(Kind::TerrorCore, Coord::new(5, 5));
        let militia = sim.add_actor(Kind::Militia, Coord::new(6, 5));
        let before = sim.schedule.scheduled_for(militia).expect("on the clock");
        sim.terrify(core);
        assert_eq!(sim.schedule.scheduled_for(militia), None, "lifted off it");
        sim.terror_post_schedule(core);
        let after = sim.schedule.scheduled_for(militia).expect("back on it");
        assert!(after > before, "and put back later: {before} -> {after}");
        assert_eq!(sim.actors[militia].delay, 16, "with its own speed restored");
    }

    #[test]
    fn a_terror_core_never_schedules_the_same_human_twice() {
        // DELIBERATE CHANGE (§11.8): the original left the list populated, so a
        // player who terrified somebody and then waited gave them two entries
        // on the schedule and two turns for every one of yours.
        let mut sim = sandbox(20);
        let core = sim.add_actor(Kind::TerrorCore, Coord::new(5, 5));
        let militia = sim.add_actor(Kind::Militia, Coord::new(6, 5));
        sim.terrify(core);
        sim.terror_post_schedule(core);
        sim.terror_post_schedule(core);
        let entries = sim.schedule.entries_for(militia);
        assert_eq!(entries, 1, "one entry, not two");
    }

    #[test]
    fn a_dead_terror_core_hands_its_victims_back() {
        let mut sim = sandbox(21);
        let core = sim.add_actor(Kind::TerrorCore, Coord::new(5, 5));
        let militia = sim.add_actor(Kind::Militia, Coord::new(6, 5));
        sim.add_actor(Kind::Nucleus, Coord::new(9, 9));
        sim.terrify(core);
        sim.destroy_organelle(core);
        assert!(
            sim.schedule.scheduled_for(militia).is_some(),
            "the militia is not frozen forever"
        );
        assert_eq!(sim.actors[militia].delay, 16);
    }

    #[test]
    fn a_quantum_core_slips_through_its_own_body_for_half_price() {
        let mut sim = sandbox(22);
        let core = sim.add_actor(Kind::QuantumCore, Coord::new(5, 5));
        sim.add_actor(Kind::Cytoplasm, Coord::new(6, 5));
        sim.set_active_nucleus(core);
        let now = sim.schedule.time();
        assert!(sim.attack_move(core, Coord::new(6, 5)));
        assert_eq!(
            sim.schedule.scheduled_for(core),
            Some(now + 4),
            "a swap costs four"
        );
        assert_eq!(sim.actors[core].delay, 8, "and the next move costs eight");
        sim.set_active_nucleus(core);
        let now = sim.schedule.time();
        assert!(sim.attack_move(core, Coord::new(5, 4)));
        assert_eq!(sim.schedule.scheduled_for(core), Some(now + 8));
    }

    #[test]
    fn a_smart_core_acts_twice_as_often_as_a_plain_one() {
        let mut sim = sandbox(23);
        let smart = sim.add_actor(Kind::SmartCore, Coord::new(5, 5));
        let plain = sim.add_actor(Kind::Nucleus, Coord::new(9, 9));
        sim.set_active_nucleus(smart);
        let now = sim.schedule.time();
        assert_eq!(sim.schedule.scheduled_for(smart), Some(now + 8));
        assert_eq!(
            sim.schedule.scheduled_for(plain),
            Some(now + 8),
            "every nucleus adopts the active one's speed"
        );
    }
}
