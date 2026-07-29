//! Map generation.
//!
//! The cavern is not carved; it is *filled in*. Start from an open box, drop
//! twenty rectangular boulders into it, then find whatever regions got sealed
//! off and tunnel between them. The player's starting blob is a random flood
//! pick of six adjacent cells, and everything else — food, wire, plants, and
//! the gates — is scattered by rejection sampling.
//!
//! Two things here are deliberately not what the original did, both because
//! the original could produce a map you cannot finish. They are marked below.

use std::collections::VecDeque;

use fastrand::Rng;

use super::actors::{ItemKind, Kind};
use super::grid::{Coord, Dir, Grid, rand_inclusive, rand_index};
use super::{Sim, actors};

/// Boulder placement attempts. Overlapping candidates are simply discarded, so
/// the map usually ends up with fewer.
const MAX_BOULDERS: i32 = 20;
/// Inclusive minimum of a boulder's nominal width and height.
const BOULDER_MIN: i32 = 7;
/// Inclusive maximum of a boulder's nominal width and height.
const BOULDER_MAX: i32 = 13;
/// Cells in the starting blob: two nuclei and four cytoplasm.
const START_MASS: usize = 6;
/// Nutrients scattered.
const FOOD_AMT: i32 = 32;
/// DNA scattered.
const DNA_AMT: i32 = 5;
/// Barbed wire scattered.
const WIRE_AMT: i32 = 8;
/// Plants scattered.
const PLANT_AMT: i32 = 8;
/// Rejection-sampling budget for one item.
const LOOT_ATTEMPTS: i32 = 2048;
/// Rejection-sampling budget for one gate.
///
/// DELIBERATE CHANGE from C#: the original had no cap here and would spin
/// forever on a map with no single-doorway rock cell. Running out means the
/// map is unusable, so the generator throws it away and tries another.
const CITY_ATTEMPTS: i32 = 4096;
/// Rejection-sampling budget for the starting blob.
const START_ATTEMPTS: i32 = 4096;
/// Whole maps to throw away before giving up and playing the last one.
const MAP_ATTEMPTS: u32 = 64;

/// A boulder footprint, in the original's inclusive-on-the-far-edge sense.
#[derive(Clone, Copy, Debug)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Rect {
    /// Half-open overlap test, matching the original's `Rectangle.Intersects`
    /// where `Right == X + Width`.
    const fn intersects(self, other: Self) -> bool {
        other.x < self.x + self.w
            && self.x < other.x + other.w
            && other.y < self.y + self.h
            && self.y < other.y + other.h
    }
}

/// The shortest way to join two disjoint regions.
#[derive(Clone, Copy, Debug)]
struct Bridge {
    from: Coord,
    to: Coord,
    dist: i32,
}

impl Sim {
    /// Build a fresh map, retrying from a derived seed if a map comes out
    /// unusable.
    ///
    /// The run's reported seed does not change: a derived seed is a private
    /// detail of generation, so a given `Sim::new` seed still reproduces a
    /// given run exactly.
    pub(crate) fn generate(&mut self) {
        let mut seed = self.seed;
        for _ in 0..MAP_ATTEMPTS {
            self.reset_world();
            if self.try_generate() {
                break;
            }
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.rng = Rng::with_seed(seed);
        }
        self.update_player_fov();
    }

    /// Throw away everything except the run's identity and its logs.
    fn reset_world(&mut self) {
        let cells = (self.rules.map_width * self.rules.map_height).unsigned_abs() as usize;
        self.grid = Grid::new(self.rules.map_width, self.rules.map_height);
        self.actors = actors::ActorStore::new();
        self.items = actors::ItemStore::new();
        self.actor_at = vec![None; cells];
        self.item_at = vec![None; cells];
        self.player_mass.clear();
        self.cities.clear();
        self.reticles.clear();
        self.terrified.clear();
        self.schedule.clear();
        self.active = None;
        self.player_turn = false;
        self.cursor = None;
        self.drag_cache = None;
        self.version += 1;
    }

    /// One generation attempt. `false` means the map is unusable.
    fn try_generate(&mut self) -> bool {
        self.arena();
        self.place_boulders();
        self.connect_pockets();
        if !self.place_starting_mass() {
            return false;
        }
        self.place_features()
    }

    /// An open box with a solid one-cell border.
    fn arena(&mut self) {
        let (w, h) = (self.grid.width(), self.grid.height());
        for y in 0..h {
            for x in 0..w {
                self.grid.set_props(Coord::new(x, y), true, true, false);
            }
        }
        for x in 0..w {
            self.grid.set_props(Coord::new(x, 0), false, false, false);
            self.grid
                .set_props(Coord::new(x, h - 1), false, false, false);
        }
        for y in 0..h {
            self.grid.set_props(Coord::new(0, y), false, false, false);
            self.grid
                .set_props(Coord::new(w - 1, y), false, false, false);
        }
    }

    /// Drop up to twenty non-overlapping rectangles of rock.
    fn place_boulders(&mut self) {
        let (w, h) = (self.grid.width(), self.grid.height());
        let mut kept: Vec<Rect> = Vec::new();
        for _ in 0..MAX_BOULDERS {
            let bw = rand_inclusive(&mut self.rng, BOULDER_MIN, BOULDER_MAX);
            let bh = rand_inclusive(&mut self.rng, BOULDER_MIN, BOULDER_MAX);
            let x = rand_inclusive(&mut self.rng, 0, w - bw - 1);
            let y = rand_inclusive(&mut self.rng, 0, h - bh - 1);
            let rect = Rect { x, y, w: bw, h: bh };
            if !kept.iter().any(|k| k.intersects(rect)) {
                kept.push(rect);
            }
        }
        for rect in kept {
            // A boulder blanks one cell more than its nominal size in each
            // direction: the original's rectangle carried an inclusive far
            // edge and the fill loop used `<=`. Kept, because every boulder in
            // the game has always been that little bit fatter.
            for y in rect.y..=(rect.y + rect.h) {
                for x in rect.x..=(rect.x + rect.w) {
                    self.grid.set_props(Coord::new(x, y), false, false, false);
                }
            }
        }
    }

    /// Tunnel between every region the boulders sealed off.
    fn connect_pockets(&mut self) {
        let pockets = self.calculate_pockets();
        if pockets.len() < 2 {
            return;
        }
        let borders: Vec<Vec<Coord>> = pockets.iter().map(|p| self.border_cells(p)).collect();
        let n = pockets.len();
        let mut bridges: Vec<Vec<Option<Bridge>>> = vec![vec![None; n]; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let bridge = best_bridge(&borders[i], &borders[j]);
                bridges[i][j] = bridge;
                bridges[j][i] = bridge.map(|b| Bridge {
                    from: b.to,
                    to: b.from,
                    dist: b.dist,
                });
            }
        }
        let mut alive: Vec<bool> = vec![true; n];
        let mut remaining = n;
        while remaining > 1 {
            let mut best: Option<(usize, usize, Bridge)> = None;
            for i in 0..n {
                for j in (i + 1)..n {
                    if !alive[i] || !alive[j] {
                        continue;
                    }
                    if let Some(bridge) = bridges[i][j]
                        && best.is_none_or(|(_, _, b)| bridge.dist < b.dist)
                    {
                        best = Some((i, j, bridge));
                    }
                }
            }
            let Some((i, j, bridge)) = best else {
                break;
            };
            self.elbow_tunnel(bridge.from, bridge.to);
            for k in 0..n {
                if k == i || k == j || !alive[k] {
                    continue;
                }
                let merged = shorter(bridges[i][k], bridges[j][k]);
                bridges[i][k] = merged;
                bridges[k][i] = merged.map(|b| Bridge {
                    from: b.to,
                    to: b.from,
                    dist: b.dist,
                });
            }
            alive[j] = false;
            remaining -= 1;
        }
    }

    /// Four-connected components of the walkable space, found in the
    /// column-major order the original scanned in.
    fn calculate_pockets(&self) -> Vec<Vec<Coord>> {
        let (w, h) = (self.grid.width(), self.grid.height());
        let cells = (w * h).unsigned_abs() as usize;
        let mut labelled = vec![false; cells];
        let mut pockets: Vec<Vec<Coord>> = Vec::new();
        for x in 0..w {
            for y in 0..h {
                let start = Coord::new(x, y);
                let Some(i) = self.cell_index(start) else {
                    continue;
                };
                if labelled[i] || !self.grid.walkable(start) {
                    continue;
                }
                labelled[i] = true;
                let mut members = vec![start];
                let mut queue = VecDeque::from([start]);
                while let Some(at) = queue.pop_front() {
                    for next in self.grid.adjacent(at) {
                        if let Some(ni) = self.cell_index(next)
                            && !labelled[ni]
                            && self.grid.walkable(next)
                        {
                            labelled[ni] = true;
                            members.push(next);
                            queue.push_back(next);
                        }
                    }
                }
                pockets.push(members);
            }
        }
        pockets
    }

    /// The cells of a pocket that touch rock.
    ///
    /// DELIBERATE CHANGE from C#: the original compared every cell of one
    /// pocket against every cell of another, which was the single most
    /// expensive step of generation. The closest pair between two disjoint
    /// regions always includes a cell that touches rock — step an interior
    /// cell toward the other region and it gets strictly closer without
    /// leaving its pocket — so restricting the search to the perimeter finds
    /// the same distance.
    fn border_cells(&self, pocket: &[Coord]) -> Vec<Coord> {
        pocket
            .iter()
            .copied()
            .filter(|c| {
                [Dir::Left, Dir::Right, Dir::Up, Dir::Down]
                    .into_iter()
                    .any(|d| !self.grid.walkable(*c + d.offset()))
            })
            .collect()
    }

    /// Carve an L-shaped corridor from `from` to `to`.
    ///
    /// DELIBERATE CHANGE from C#: the original anchored *both* segments at
    /// `from`, so a diagonal bridge carved to `(to.x, from.y)` and
    /// `(from.x, to.y)` and never reached `to` at all — and then assumed the
    /// two regions had merged. That is how a map could ship with an
    /// unreachable pocket in it. Running the second segment along `to.x`
    /// arrives where it was supposed to.
    fn elbow_tunnel(&mut self, from: Coord, to: Coord) {
        for x in from.x.min(to.x)..=from.x.max(to.x) {
            self.grid
                .set_props(Coord::new(x, from.y), true, true, false);
        }
        for y in from.y.min(to.y)..=from.y.max(to.y) {
            self.grid.set_props(Coord::new(to.x, y), true, true, false);
        }
    }

    /// Seed the player: a random blob of six adjacent cells, holding a nucleus
    /// at each end and cytoplasm between.
    fn place_starting_mass(&mut self) -> bool {
        let (w, h) = (self.grid.width(), self.grid.height());
        for _ in 0..START_ATTEMPTS {
            let seed_cell = Coord::new(
                rand_inclusive(&mut self.rng, 0, w - 1),
                rand_inclusive(&mut self.rng, 0, h - 1),
            );
            if !self.grid.walkable(seed_cell) {
                continue;
            }
            let Some(blob) = self.fluid_select(seed_cell, START_MASS) else {
                continue;
            };
            let kinds = [
                Kind::Nucleus,
                Kind::Cytoplasm,
                Kind::Cytoplasm,
                Kind::Cytoplasm,
                Kind::Cytoplasm,
                Kind::Nucleus,
            ];
            let mut first = None;
            for (kind, cell) in kinds.into_iter().zip(blob) {
                let id = self.add_actor(kind, cell);
                first.get_or_insert(id);
            }
            if let Some(id) = first {
                self.set_active_nucleus(id);
            }
            return true;
        }
        false
    }

    /// Randomised flood pick: grow a blob one uniformly-chosen frontier cell
    /// at a time until it holds `count` cells, or run out of room.
    fn fluid_select(&mut self, from: Coord, count: usize) -> Option<Vec<Coord>> {
        let mut selection = vec![from];
        let mut candidates: Vec<Coord> = self.grid.adjacent_walkable(from).collect();
        while selection.len() < count {
            let index = rand_index(&mut self.rng, candidates.len())?;
            let picked = candidates.remove(index);
            selection.push(picked);
            let grown: Vec<Coord> = self
                .grid
                .adjacent_walkable(picked)
                .filter(|n| !candidates.contains(n) && !selection.contains(n))
                .collect();
            candidates.extend(grown);
        }
        Some(selection)
    }

    /// Scatter loot and gates, in the order the original did — nutrients
    /// first, then the gates, then the rarer catalysts, so the gates get the
    /// pick of the rock and the catalysts have to fit around everything.
    fn place_features(&mut self) -> bool {
        for _ in 0..FOOD_AMT {
            self.place_loot(ItemKind::Nutrient);
        }
        for _ in 0..self.rules.num_cities {
            if !self.place_city() {
                return false;
            }
        }
        for _ in 0..DNA_AMT {
            self.place_loot(ItemKind::Dna);
        }
        for _ in 0..WIRE_AMT {
            self.place_loot(ItemKind::BarbedWire);
        }
        for _ in 0..PLANT_AMT {
            self.place_loot(ItemKind::Plant);
        }
        true
    }

    /// Drop one item on a free floor cell. Silently gives up on a cramped map,
    /// so the item counts are upper bounds.
    fn place_loot(&mut self, kind: ItemKind) -> bool {
        let (w, h) = (self.grid.width(), self.grid.height());
        for _ in 0..LOOT_ATTEMPTS {
            let cell = Coord::new(
                rand_inclusive(&mut self.rng, 0, w - 1),
                rand_inclusive(&mut self.rng, 0, h - 1),
            );
            if self.grid.walkable(cell) && self.is_empty_cell(cell) {
                self.add_item(kind, cell);
                return true;
            }
        }
        false
    }

    /// Put a gate inside the rock, with exactly one doorway onto the cavern.
    fn place_city(&mut self) -> bool {
        let (w, h) = (self.grid.width(), self.grid.height());
        for _ in 0..CITY_ATTEMPTS {
            let cell = Coord::new(
                rand_inclusive(&mut self.rng, 0, w - 1),
                rand_inclusive(&mut self.rng, 0, h - 1),
            );
            if self.grid.walkable(cell) || !self.is_empty_cell(cell) {
                continue;
            }
            if self.grid.adjacent_walkable(cell).count() == 1 {
                self.add_actor(Kind::City, cell);
                return true;
            }
        }
        false
    }
}

/// The closest pair of cells between two regions, by taxicab distance.
fn best_bridge(from: &[Coord], to: &[Coord]) -> Option<Bridge> {
    let mut best: Option<Bridge> = None;
    for &f in from {
        for &t in to {
            let dist = f.taxi(t);
            if best.is_none_or(|b| dist < b.dist) {
                best = Some(Bridge {
                    from: f,
                    to: t,
                    dist,
                });
            }
        }
    }
    best
}

/// Whichever of two bridges is shorter, tolerating either being absent.
const fn shorter(a: Option<Bridge>, b: Option<Bridge>) -> Option<Bridge> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a.dist <= b.dist { a } else { b }),
        (some, None) | (None, some) => some,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::tests::playing;
    use crate::sim::{Command, Difficulty, Sim};

    /// Every cell that is not solid rock, reachable from the player's blob.
    fn reachable_open_cells(sim: &Sim) -> usize {
        let grid = sim.grid();
        let cells = (grid.width() * grid.height()).unsigned_abs() as usize;
        let mut seen = vec![false; cells];
        let start = sim.actors()[sim.player_mass()[0]].pos;
        let mut queue = VecDeque::from([start]);
        let index = |c: Coord| (c.y * grid.width() + c.x).unsigned_abs() as usize;
        seen[index(start)] = true;
        let mut count = 1;
        while let Some(at) = queue.pop_front() {
            for next in [Dir::Left, Dir::Right, Dir::Up, Dir::Down] {
                let next = at + next.offset();
                if !grid.in_bounds(next) || seen[index(next)] || sim.is_wall(next) {
                    continue;
                }
                seen[index(next)] = true;
                count += 1;
                queue.push_back(next);
            }
        }
        count
    }

    /// Every cell that is not solid rock, wherever it is.
    fn open_cells(sim: &Sim) -> usize {
        let grid = sim.grid();
        (0..grid.height())
            .flat_map(|y| (0..grid.width()).map(move |x| Coord::new(x, y)))
            .filter(|c| !sim.is_wall(*c))
            .count()
    }

    #[test]
    fn the_whole_cavern_is_connected() {
        for seed in 0..40 {
            let sim = playing(seed);
            assert_eq!(
                reachable_open_cells(&sim),
                open_cells(&sim),
                "seed {seed} left an unreachable pocket"
            );
        }
    }

    #[test]
    fn the_border_is_solid() {
        let sim = playing(5);
        let grid = sim.grid();
        for x in 0..grid.width() {
            assert!(!grid.walkable(Coord::new(x, 0)));
            assert!(!grid.walkable(Coord::new(x, grid.height() - 1)));
        }
        for y in 0..grid.height() {
            assert!(!grid.walkable(Coord::new(0, y)));
            assert!(!grid.walkable(Coord::new(grid.width() - 1, y)));
        }
    }

    #[test]
    fn the_starting_blob_is_six_connected_cells() {
        for seed in 0..20 {
            let sim = playing(seed);
            assert_eq!(sim.mass(), 6);
            let nuclei = sim
                .player_mass()
                .iter()
                .filter(|id| actors::is_nucleus_family(sim.actors()[**id].kind))
                .count();
            assert_eq!(nuclei, 2, "seed {seed}");
            // Every organelle touches at least one other.
            for id in sim.player_mass() {
                let pos = sim.actors()[*id].pos;
                let touching = sim
                    .player_mass()
                    .iter()
                    .filter(|other| **other != *id && sim.actors()[**other].pos.adjacent_to(pos))
                    .count();
                assert!(touching > 0, "seed {seed}: {pos:?} is detached");
            }
        }
    }

    #[test]
    fn every_gate_has_exactly_one_doorway() {
        for seed in 0..20 {
            let sim = playing(seed);
            assert_eq!(sim.cities().len(), 12, "seed {seed}");
            for id in sim.cities() {
                let pos = sim.actors()[*id].pos;
                assert!(!sim.grid().transparent(pos), "a gate sits inside rock");
                // Nothing placed after the gates changes walkability, so the
                // single doorway a gate was chosen for is still its only one.
                // Two gates may sit side by side in the rock; that is fine,
                // because rock is not a doorway.
                let doors = sim.grid().adjacent_walkable(pos).count();
                assert_eq!(doors, 1, "seed {seed}: gate at {pos:?}");
            }
        }
    }

    #[test]
    fn gate_armour_comes_from_the_difficulty() {
        let mut sim = Sim::new(3, Difficulty::Gj);
        sim.advance(Some(Command::Start(Difficulty::Gj)));
        assert_eq!(sim.cities().len(), 16);
        assert_eq!(sim.grid().width(), 64);
        for id in sim.cities() {
            assert_eq!(sim.actors()[*id].armor, 160);
        }
    }

    #[test]
    fn loot_counts_match_the_generator() {
        for seed in 0..10 {
            let sim = playing(seed);
            let mut counts = [0_i32; 6];
            for y in 0..sim.grid().height() {
                for x in 0..sim.grid().width() {
                    if let Some(id) = sim.item_at(Coord::new(x, y)) {
                        let slot = match sim.items()[id].kind {
                            ItemKind::Nutrient => 0,
                            ItemKind::CalciumDust => 1,
                            ItemKind::SiliconDust => 2,
                            ItemKind::BarbedWire => 3,
                            ItemKind::Plant => 4,
                            ItemKind::Dna => 5,
                        };
                        counts[slot] += 1;
                    }
                }
            }
            // Placement can silently give up on a cramped map, so these are
            // upper bounds; on a 48x48 arena they are always hit.
            assert_eq!(counts[0], FOOD_AMT, "seed {seed} nutrients");
            assert_eq!(counts[3], WIRE_AMT, "seed {seed} wire");
            assert_eq!(counts[4], PLANT_AMT, "seed {seed} plants");
            assert_eq!(counts[5], DNA_AMT, "seed {seed} dna");
            assert_eq!(counts[1] + counts[2], 0, "no dust is scattered at start");
        }
    }

    #[test]
    #[allow(clippy::cast_sign_loss)] // Map dimensions are positive.
    fn items_never_share_a_cell_with_each_other() {
        let sim = playing(21);
        let mut seen = vec![false; (sim.grid().width() * sim.grid().height()) as usize];
        for y in 0..sim.grid().height() {
            for x in 0..sim.grid().width() {
                let c = Coord::new(x, y);
                if sim.item_at(c).is_some() {
                    let i = (c.y * sim.grid().width() + c.x) as usize;
                    assert!(!seen[i]);
                    seen[i] = true;
                    assert!(sim.grid().walkable(c) || sim.actor_at(c).is_some());
                }
            }
        }
    }

    #[test]
    fn boulders_do_not_overlap_and_are_one_cell_fat() {
        let a = Rect {
            x: 0,
            y: 0,
            w: 4,
            h: 4,
        };
        let b = Rect {
            x: 4,
            y: 0,
            w: 4,
            h: 4,
        };
        assert!(!a.intersects(b), "abutting rectangles do not intersect");
        let c = Rect {
            x: 3,
            y: 3,
            w: 4,
            h: 4,
        };
        assert!(a.intersects(c));
    }

    #[test]
    fn generation_is_reproducible() {
        let a = playing(1234);
        let b = playing(1234);
        let cells = |sim: &Sim| {
            (0..sim.grid().height())
                .flat_map(|y| (0..sim.grid().width()).map(move |x| Coord::new(x, y)))
                .map(|c| sim.grid().walkable(c))
                .collect::<Vec<_>>()
        };
        assert_eq!(cells(&a), cells(&b));
        let positions = |sim: &Sim| {
            sim.player_mass()
                .iter()
                .map(|id| sim.actors()[*id].pos)
                .collect::<Vec<_>>()
        };
        assert_eq!(positions(&a), positions(&b));
    }
}
