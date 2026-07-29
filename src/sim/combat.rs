//! How the humans hurt you, and what your skin does about it.
//!
//! There is only one entry point, [`Sim::attack`], and it resolves four very
//! different rules depending on what got hit. A nucleus is saved if anything
//! can save it and otherwise trades places with a neighbour who dies instead. A
//! membrane resolves *its own* kill-or-die rule and is never rescued by
//! anybody. Everything else in your body gets one chance at a save and is then
//! destroyed, dropping only the first thing it was built from.
//!
//! Two of the rules here read as bugs and are kept on purpose, because they are
//! what the game is balanced around: a force field's long reach applies to
//! every unarmoured attacker rather than only to militia, and a membrane
//! standing beside a phase membrane is never rescued by it.

use super::actors::{self, ActorId, Kind};
use super::{Cue, Sim};

/// How far a force field reaches when the attacker is not armoured.
const LONG_RANGE_FF_DIST: i32 = 3;

impl Sim {
    /// One human striking one thing of yours.
    pub(crate) fn attack(&mut self, monster: ActorId, victim: ActorId) {
        // The original recursed here, once, when a nucleus retreated into a
        // neighbour and the attacker followed it in. A loop says the same thing
        // without putting an unbounded chain on the stack.
        let mut victim = victim;
        loop {
            let (Some(attacker), Some(target)) =
                (self.actors.get(monster), self.actors.get(victim))
            else {
                return;
            };
            let (monster_name, monster_armor) = (attacker.name, attacker.armor);
            let monster_is_npc = actors::is_npc(attacker.kind);
            let (victim_name, victim_kind) = (target.name, target.kind);

            if actors::is_nucleus_family(victim_kind) {
                if self.check_and_save(monster, victim) {
                    return;
                }
                let Some(sacrifice) = self.retreat(victim) else {
                    self.messages.add(&format!(
                        "{victim_name} could not retreat and was destroyed"
                    ));
                    self.destroy_organelle(victim);
                    return;
                };
                let sacrifice_name = self.actors[sacrifice].name;
                self.messages.add(&format!(
                    "The {victim_name} retreated into the nearby {sacrifice_name}, thereby avoiding death"
                ));
                victim = sacrifice;
            } else if actors::is_membrane_family(victim_kind) && monster_is_npc {
                // KEPT from C# (§11.10): membranes never reach `check_and_save`,
                // so a membrane beside a phase membrane is on its own.
                if monster_armor > 0 && !actors::kills_armored_attackers(victim_kind) {
                    self.messages.add(&format!(
                        "The {monster_name} shrugs off the {victim_name}'s proteins"
                    ));
                    self.destroy_organelle(victim);
                    self.cue(Cue::OrganelleLost);
                } else {
                    self.messages.add(&format!(
                        "The {monster_name} is impaled by sharp {victim_name} proteins!"
                    ));
                    self.kill_npc(monster);
                }
                return;
            } else if actors::is_organelle(victim_kind) {
                if !self.check_and_save(monster, victim) {
                    self.messages
                        .add(&format!("The {monster_name} destroys the {victim_name}"));
                    self.destroy_organelle(victim);
                    self.cue(Cue::OrganelleLost);
                }
                return;
            } else {
                self.messages
                    .add(&format!("A {victim_name} is destroyed by a {monster_name}"));
                self.remove_actor(victim);
                return;
            }
        }
    }

    /// Whether anything nearby steps in and stops the blow.
    ///
    /// Three chances, in order: a force field within three cells when the
    /// attacker is unarmoured, a force field right beside the victim whoever
    /// the attacker is, and finally a phase membrane, which swaps itself into
    /// the victim's place and hardens around the attacker.
    ///
    /// KEPT from C# (§11.9): the force field's own description says it does not
    /// help against tanks and mechs. The code says otherwise — the adjacency
    /// check applies to everything, and the long-range one to every attacker
    /// that is not armoured, not merely to militia. The code wins.
    fn check_and_save(&mut self, monster: ActorId, victim: ActorId) -> bool {
        let (Some(attacker), Some(target)) = (self.actors.get(monster), self.actors.get(victim))
        else {
            return false;
        };
        let (monster_name, monster_kind) = (attacker.name, attacker.kind);
        let (victim_name, victim_pos) = (target.name, target.pos);

        let is_force_field = |sim: &Self, c| {
            sim.actor_at(c)
                .and_then(|a| sim.actors.get(a))
                .is_some_and(|a| a.kind == Kind::ForceField)
        };
        let long_reach = !actors::is_tank_family(monster_kind)
            && self
                .grid
                .cells_in_diamond(victim_pos, LONG_RANGE_FF_DIST)
                .any(|c| is_force_field(self, c));
        let beside = self
            .grid
            .adjacent(victim_pos)
            .any(|c| is_force_field(self, c));
        if long_reach || beside {
            self.messages.add(&format!(
                "An energy mantle force protects the {victim_name} from the {monster_name}"
            ));
            return true;
        }

        let saviors: Vec<ActorId> = self
            .grid
            .adjacent(victim_pos)
            .filter_map(|c| self.actor_at(c))
            .filter(|&a| {
                self.actors
                    .get(a)
                    .is_some_and(|x| x.kind == Kind::PhaseMembrane)
            })
            .collect();
        let Some(savior) = self.pick(&saviors) else {
            return false;
        };
        let savior_name = self.actors[savior].name;
        self.swap_actors(savior, victim);
        self.messages.add(&format!(
            "The {savior_name} rematerializes and protects the {victim_name} from the {monster_name}!"
        ));
        if actors::is_npc(monster_kind) {
            self.messages.add(&format!(
                "{monster_name} is killed by the rematerializing phase membrane!!"
            ));
            self.kill_npc(monster);
        }
        true
    }

    /// Kill a human outright: it drops its loot on the floor rather than
    /// joining you.
    ///
    /// This is what membranes, phase membranes and stray hunter fire do. A
    /// caravan's cargo is a coin flip between bone and circuitry.
    pub(crate) fn kill_npc(&mut self, id: ActorId) {
        let Some(actor) = self.actors.get(id) else {
            return;
        };
        let (kind, pos) = (actor.kind, actor.pos);
        let dropped = self.pick(kind.drops_on_die());
        self.remove_actor(id);
        if let Some(item) = dropped {
            self.become_item(item, pos);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::grid::Coord;
    use crate::sim::tests::sandbox;

    /// A monster of `attacker` beside a victim of `victim`, three cells clear
    /// of the arena wall.
    fn duel(seed: u64, attacker: Kind, victim: Kind) -> (Sim, ActorId, ActorId) {
        let mut sim = sandbox(seed);
        let monster = sim.add_actor(attacker, Coord::new(9, 9));
        let target = sim.add_actor(victim, Coord::new(10, 9));
        // Somewhere for the run to keep going from if a nucleus dies.
        sim.add_actor(Kind::Nucleus, Coord::new(3, 3));
        (sim, monster, target)
    }

    #[test]
    fn a_membrane_kills_an_unarmoured_attacker() {
        for membrane in [
            Kind::Membrane,
            Kind::Maw,
            Kind::Tentacle,
            Kind::ReinforcedMembrane,
            Kind::ForceField,
            Kind::PhaseMembrane,
            Kind::ReinforcedMaw,
        ] {
            let (mut sim, monster, target) = duel(1, Kind::Militia, membrane);
            sim.attack(monster, target);
            assert!(
                !sim.actors.contains(monster),
                "{membrane:?} let a militia live"
            );
            assert!(sim.actors.contains(target), "{membrane:?} did not survive");
        }
    }

    #[test]
    fn armour_splits_only_on_reinforced_proteins() {
        for membrane in [
            Kind::ReinforcedMembrane,
            Kind::ForceField,
            Kind::PhaseMembrane,
            Kind::ReinforcedMaw,
        ] {
            let (mut sim, monster, target) = duel(2, Kind::Tank, membrane);
            sim.attack(monster, target);
            assert!(
                !sim.actors.contains(monster),
                "{membrane:?} let a tank live"
            );
            assert!(sim.actors.contains(target));
        }
        for membrane in [Kind::Membrane, Kind::Maw, Kind::Tentacle] {
            let (mut sim, monster, target) = duel(3, Kind::Tank, membrane);
            sim.attack(monster, target);
            assert!(sim.actors.contains(monster), "{membrane:?} killed a tank");
            assert!(!sim.actors.contains(target), "{membrane:?} survived a tank");
        }
    }

    #[test]
    fn a_dead_human_leaves_its_loot_behind() {
        let (mut sim, monster, target) = duel(4, Kind::Tank, Kind::ReinforcedMembrane);
        sim.attack(monster, target);
        let dropped: Vec<_> = (0..20)
            .flat_map(|y| (0..20).map(move |x| Coord::new(x, y)))
            .filter_map(|c| sim.item_at(c))
            .map(|i| sim.items[i].kind)
            .collect();
        assert_eq!(dropped, vec![crate::sim::actors::ItemKind::CalciumDust]);
    }

    #[test]
    fn a_force_field_reaches_three_cells_for_an_unarmoured_attacker() {
        for distance in 1..=4 {
            let mut sim = sandbox(5);
            let monster = sim.add_actor(Kind::Militia, Coord::new(4, 9));
            let victim = sim.add_actor(Kind::Cytoplasm, Coord::new(5, 9));
            sim.add_actor(Kind::ForceField, Coord::new(5 + distance, 9));
            sim.add_actor(Kind::Nucleus, Coord::new(3, 3));
            sim.attack(monster, victim);
            assert_eq!(
                sim.actors.contains(victim),
                distance <= LONG_RANGE_FF_DIST,
                "at distance {distance}"
            );
        }
    }

    #[test]
    fn a_force_field_only_reaches_one_cell_for_a_tank() {
        for distance in 1..=3 {
            let mut sim = sandbox(6);
            let monster = sim.add_actor(Kind::Tank, Coord::new(4, 9));
            let victim = sim.add_actor(Kind::Cytoplasm, Coord::new(5, 9));
            sim.add_actor(Kind::ForceField, Coord::new(5 + distance, 9));
            sim.add_actor(Kind::Nucleus, Coord::new(3, 3));
            sim.attack(monster, victim);
            assert_eq!(
                sim.actors.contains(victim),
                distance == 1,
                "a tank at distance {distance}"
            );
        }
    }

    #[test]
    fn a_phase_membrane_swaps_in_and_kills_the_attacker() {
        let mut sim = sandbox(7);
        let monster = sim.add_actor(Kind::Mech, Coord::new(4, 9));
        let victim = sim.add_actor(Kind::Cytoplasm, Coord::new(5, 9));
        let savior = sim.add_actor(Kind::PhaseMembrane, Coord::new(5, 10));
        sim.add_actor(Kind::Nucleus, Coord::new(3, 3));
        sim.attack(monster, victim);
        assert!(!sim.actors.contains(monster), "the mech is gone");
        assert!(sim.actors.contains(victim), "the cytoplasm lived");
        assert_eq!(
            sim.actors[savior].pos,
            Coord::new(5, 9),
            "they traded places"
        );
        assert_eq!(sim.actors[victim].pos, Coord::new(5, 10));
    }

    #[test]
    fn a_membrane_beside_a_phase_membrane_is_still_on_its_own() {
        // KEPT (§11.10): membranes resolve their own rule and never reach the
        // save check, so the phase membrane next door does nothing for them.
        let mut sim = sandbox(8);
        let monster = sim.add_actor(Kind::Tank, Coord::new(4, 9));
        let victim = sim.add_actor(Kind::Membrane, Coord::new(5, 9));
        sim.add_actor(Kind::PhaseMembrane, Coord::new(5, 10));
        sim.add_actor(Kind::Nucleus, Coord::new(3, 3));
        sim.attack(monster, victim);
        assert!(sim.actors.contains(monster));
        assert!(!sim.actors.contains(victim));
    }

    #[test]
    fn a_saved_organelle_is_not_destroyed() {
        let (mut sim, monster, victim) = duel(9, Kind::Militia, Kind::Cytoplasm);
        sim.attack(monster, victim);
        assert!(!sim.actors.contains(victim), "with no saviour it dies");
        let (mut sim, monster, victim) = duel(10, Kind::Militia, Kind::Cytoplasm);
        sim.add_actor(Kind::ForceField, Coord::new(10, 10));
        sim.attack(monster, victim);
        assert!(sim.actors.contains(victim), "with one, it does not");
    }

    #[test]
    fn a_struck_nucleus_retreats_into_a_neighbour() {
        let mut sim = sandbox(11);
        let monster = sim.add_actor(Kind::Militia, Coord::new(4, 9));
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 9));
        let shield = sim.add_actor(Kind::Cytoplasm, Coord::new(5, 10));
        sim.add_actor(Kind::Nucleus, Coord::new(3, 3));
        sim.attack(monster, nucleus);
        assert!(sim.actors.contains(nucleus), "the nucleus lived");
        assert_eq!(sim.actors[nucleus].pos, Coord::new(5, 10));
        assert!(!sim.actors.contains(shield), "the cytoplasm did not");
    }

    #[test]
    fn a_cornered_nucleus_dies() {
        let mut sim = sandbox(12);
        let monster = sim.add_actor(Kind::Militia, Coord::new(4, 9));
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(5, 9));
        sim.add_actor(Kind::Nucleus, Coord::new(3, 3));
        sim.attack(monster, nucleus);
        assert!(!sim.actors.contains(nucleus));
        assert!(sim.cues().contains(&Cue::NucleusLost));
    }

    #[test]
    fn a_retreat_prefers_somewhere_nothing_is_aiming() {
        // Both neighbours are legal sacrifices, so without the preference the
        // painted one would come up about half the time across these seeds.
        for seed in 0..12 {
            let mut sim = sandbox(seed);
            let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(9, 9));
            let hunter = sim.add_actor(Kind::Hunter, Coord::new(2, 2));
            let painted = sim.add_actor(Kind::Cytoplasm, Coord::new(8, 9));
            let clear = sim.add_actor(Kind::Cytoplasm, Coord::new(10, 9));
            sim.reticles.push(crate::sim::actors::Reticle {
                pos: Coord::new(8, 9),
                owner: hunter,
            });
            let chosen = sim.retreat(nucleus).expect("a sacrifice");
            assert_eq!(chosen, clear, "seed {seed}: it hid behind the clear cell");
            assert_ne!(chosen, painted);
        }
    }

    #[test]
    fn a_retreat_will_take_a_painted_cell_if_it_is_the_only_one() {
        let mut sim = sandbox(14);
        let nucleus = sim.add_actor(Kind::Nucleus, Coord::new(9, 9));
        let hunter = sim.add_actor(Kind::Hunter, Coord::new(2, 2));
        let painted = sim.add_actor(Kind::Cytoplasm, Coord::new(8, 9));
        sim.reticles.push(crate::sim::actors::Reticle {
            pos: Coord::new(8, 9),
            owner: hunter,
        });
        assert_eq!(sim.retreat(nucleus), Some(painted));
    }
}
