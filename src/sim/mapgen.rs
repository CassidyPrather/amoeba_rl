//! Map generation.
//!
//! The cavern is the underside of a human city, and it is built the way one
//! would be. The rock is cut into districts; a chamber is dissolved out of the
//! middle of each; streets two cells wide are run between the chambers, with a
//! couple of loops so the map is a network rather than a tree; and the humans'
//! gates are sunk into the rock at the end of narrow dead-end passages, spread
//! out so no two of them open onto the same corner.
//!
//! Everything after that is placed against one number. [`Sim::measure_depth`]
//! floods outward from every gate doorway and records how far each cell is from
//! the nearest one, and that field is what the rest of the generator asks:
//!
//! * **Barbed wire** goes in a ring just outside each gate, because that is
//!   where humans put barbed wire.
//! * **Nutrients** go in caches out along the streets — clumps, not confetti,
//!   because a cache is a thing somebody left somewhere.
//! * **Plants** grow in the deep, where nobody has been to pull them up.
//! * **DNA** hides in the dead ends of the deep, so the one catalyst that grows
//!   a second nucleus is the one you have to go looking for.
//! * **The amoeba** wakes up in the chamber the humans have the furthest to
//!   walk to, which is what makes the first fifty turns yours.
//!
//! The depth field outlives generation: the humans read it too, so "a scout
//! patrolling far from the gates" and "a plant growing where nobody goes" mean
//! the same thing measured the same way.
//!
//! DELIBERATE CHANGE from C#, wholesale. The original filled an open box with
//! twenty rectangular boulders and tunnelled between whatever got sealed off,
//! then scattered every item by rejection sampling. That produced a map with no
//! opinion about itself: nowhere was anywhere in particular, and nothing on the
//! floor meant anything by being where it was. Two of the original's own bugs
//! are gone with it — the unbounded gate search that could spin forever, and
//! the elbow tunnel that never reached its far end and left pockets stranded.

use std::collections::VecDeque;

use fastrand::Rng;

use super::actors::{ActorId, ItemKind, Kind};
use super::grid::{Coord, Dir, Grid, rand_inclusive, rand_index};
use super::{Sim, actors};

/// Smallest side a district may be cut down to.
const DISTRICT_MIN: i32 = 13;
/// How many times every district is offered a cut.
const DISTRICT_CUTS: u32 = 4;
/// Rock left inside a district's edge, so two chambers never touch.
const DISTRICT_WALL: i32 = 1;
/// Percent of a district seeded as rock before smoothing.
const ROCK_PERCENT: i32 = 45;
/// Smoothing passes over a district.
const SMOOTHING_ROUNDS: u32 = 4;
/// Rock neighbours that turn a cell to rock in the next pass. The classic
/// four-five rule: five of the eight, and everything past the district's edge
/// counts, which is what pulls a chamber away from its own walls.
const CROWDED: i32 = 5;
/// Cells a smoothed chamber needs before it is worth carving.
const CHAMBER_MIN_CELLS: usize = 24;
/// Chambers a map needs before it is worth playing.
const CHAMBERS_MIN: usize = 3;
/// Streets carved beyond the spanning tree, so the map holds loops.
const STREET_LOOPS: usize = 2;
/// Shortest gatehouse passage, in cells of rock cut away.
const GATEHOUSE_MIN: i32 = 2;
/// Longest gatehouse passage.
const GATEHOUSE_MAX: i32 = 4;
/// Distances gates are asked to keep from each other, tried in order. The last
/// one is a formality: by then the map is telling us it has no room left.
const GATE_GAPS: [i32; 4] = [14, 10, 6, 2];
/// Cells in the starting blob: two nuclei and four cytoplasm.
const START_MASS: usize = 6;
/// How far from a chamber's middle the blob may be seeded.
const START_SPREAD: i32 = 4;
/// Chambers tried, deepest first, before the map is given up on.
const START_CHAMBERS: usize = 4;
/// Nutrients scattered.
const FOOD_AMT: i32 = 32;
/// DNA scattered.
const DNA_AMT: i32 = 5;
/// Barbed wire scattered.
const WIRE_AMT: i32 = 8;
/// Plants scattered.
const PLANT_AMT: i32 = 8;
/// Nearest and furthest a coil of wire is dropped from its gate's doorway.
const WIRE_RING: (i32, i32) = (2, 5);
/// Fewest and most nutrients in one cache.
const CACHE_SIZE: (i32, i32) = (3, 5);
/// How far a cache's contents spill from its middle.
const CACHE_SPREAD: i32 = 2;
/// How far apart two caches are kept.
const CACHE_GAP: i32 = 6;
/// The share of the way to the deepest cell that counts as "out on the
/// streets": a cache belongs past the gates' fortifications and short of the
/// dark.
const CACHE_BAND: (i32, i32) = (25, 70);
/// The share of the way to the deepest cell that counts as "the deep".
const DEEP_BAND: i32 = 60;
/// Rejection-sampling budget for the starting blob.
const START_ATTEMPTS: i32 = 64;
/// Whole maps to throw away before giving up and playing the last one.
const MAP_ATTEMPTS: u32 = 64;

/// A half-open rectangle of cells: `x..x + w` by `y..y + h`.
#[derive(Clone, Copy, Debug)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

impl Rect {
    /// The same rectangle with `by` cells taken off every side.
    const fn inset(self, by: i32) -> Self {
        Self {
            x: self.x + by,
            y: self.y + by,
            w: self.w - by * 2,
            h: self.h - by * 2,
        }
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
        self.cities_destroyed = 0;
        self.reticles.clear();
        self.terrified.clear();
        self.rumors.clear();
        self.junctions.clear();
        self.depth = vec![i32::MAX; cells];
        self.schedule.clear();
        self.active = None;
        self.player_turn = false;
        self.cursor = None;
        self.drag_cache = None;
        self.version += 1;
    }

    /// One generation attempt. `false` means the map is unusable.
    fn try_generate(&mut self) -> bool {
        let districts = self.districts();
        let mut hubs = Vec::new();
        for district in districts {
            if let Some(hub) = self.carve_chamber(district) {
                hubs.push(hub);
            }
        }
        if hubs.len() < CHAMBERS_MIN {
            return false;
        }
        self.carve_streets(&hubs);
        self.connect_pockets();
        self.junctions = hubs;
        if !self.place_gates() {
            return false;
        }
        self.measure_depth();
        if !self.place_starting_mass() {
            return false;
        }
        self.place_features();
        true
    }

    // -- districts and chambers ---------------------------------------------

    /// Cut the map's interior into districts, recursively and along the long
    /// side of whatever is being cut.
    fn districts(&mut self) -> Vec<Rect> {
        let mut leaves = vec![Rect {
            x: 1,
            y: 1,
            w: self.grid.width() - 2,
            h: self.grid.height() - 2,
        }];
        for _ in 0..DISTRICT_CUTS {
            let mut next = Vec::with_capacity(leaves.len() * 2);
            for leaf in leaves {
                match self.cut(leaf) {
                    Some((left, right)) => {
                        next.push(left);
                        next.push(right);
                    }
                    None => next.push(leaf),
                }
            }
            leaves = next;
        }
        leaves
    }

    /// Cut one district in two, or refuse because either half would be too
    /// small to hold a chamber.
    fn cut(&mut self, r: Rect) -> Option<(Rect, Rect)> {
        // Cut the long way when a district is clearly oblong, and toss a coin
        // when it is roughly square: districts that are all one shape read as a
        // grid, and a grid is the thing this generator exists to avoid.
        let across = if r.w * 4 > r.h * 5 {
            true
        } else if r.h * 4 > r.w * 5 {
            false
        } else {
            rand_inclusive(&mut self.rng, 0, 1) == 0
        };
        let span = if across { r.w } else { r.h };
        if span < DISTRICT_MIN * 2 {
            return None;
        }
        let at = rand_inclusive(&mut self.rng, DISTRICT_MIN, span - DISTRICT_MIN);
        Some(if across {
            (
                Rect { w: at, ..r },
                Rect {
                    x: r.x + at,
                    w: r.w - at,
                    ..r
                },
            )
        } else {
            (
                Rect { h: at, ..r },
                Rect {
                    y: r.y + at,
                    h: r.h - at,
                    ..r
                },
            )
        })
    }

    /// Dissolve a chamber out of the middle of one district and report the cell
    /// nearest its centre, or `None` when the smoothing left nothing worth
    /// carving.
    ///
    /// Random noise smoothed four times is the standard recipe for a cave that
    /// looks eaten rather than drawn. Only the largest piece of what comes out
    /// is carved, so a district never contributes an island for the connector
    /// to have to find later.
    fn carve_chamber(&mut self, district: Rect) -> Option<Coord> {
        let room = district.inset(DISTRICT_WALL);
        if room.w < 5 || room.h < 5 {
            return None;
        }
        let (w, h) = (room.w, room.h);
        #[allow(clippy::cast_sign_loss)] // Both sides are positive by the guard above.
        let len = (w * h) as usize;
        #[allow(clippy::cast_sign_loss)]
        let at = |x: i32, y: i32| (y * w + x) as usize;
        let mut open: Vec<bool> = (0..len)
            .map(|_| rand_inclusive(&mut self.rng, 0, 99) >= ROCK_PERCENT)
            .collect();
        for _ in 0..SMOOTHING_ROUNDS {
            let mut next = vec![false; len];
            for y in 0..h {
                for x in 0..w {
                    let mut rock = 0;
                    for dy in -1..=1 {
                        for dx in -1_i32..=1 {
                            let (nx, ny) = (x + dx, y + dy);
                            let outside = nx < 0 || ny < 0 || nx >= w || ny >= h;
                            if (dx != 0 || dy != 0) && (outside || !open[at(nx, ny)]) {
                                rock += 1;
                            }
                        }
                    }
                    next[at(x, y)] = rock < CROWDED;
                }
            }
            open = next;
        }
        let body = largest_region(&open, w, h)?;
        if body.len() < CHAMBER_MIN_CELLS {
            return None;
        }
        let mut sum = (0_i64, 0_i64);
        let mut cells = Vec::with_capacity(body.len());
        for i in body {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let c = Coord::new(room.x + i as i32 % w, room.y + i as i32 / w);
            self.carve(c);
            sum.0 += i64::from(c.x);
            sum.1 += i64::from(c.y);
            cells.push(c);
        }
        let count = i64::try_from(cells.len()).unwrap_or(1).max(1);
        #[allow(clippy::cast_possible_truncation)] // Map coordinates fit an i32 with room to spare.
        let mean = Coord::new((sum.0 / count) as i32, (sum.1 / count) as i32);
        // The mean of a crescent lands in the rock, so report the cell of the
        // chamber nearest it rather than the mean itself.
        cells.into_iter().min_by_key(|c| c.taxi(mean))
    }

    /// Turn one cell to floor, unless it is part of the map's outer wall.
    ///
    /// Everything that cuts rock goes through here, which is the whole of why
    /// the border stays solid without anybody having to remember it.
    fn carve(&mut self, c: Coord) -> bool {
        let inside =
            c.x > 0 && c.y > 0 && c.x < self.grid.width() - 1 && c.y < self.grid.height() - 1;
        if inside {
            self.grid.set_props(c, true, true, false);
        }
        inside
    }

    // -- streets ------------------------------------------------------------

    /// Join the chambers with streets: a minimum spanning tree so everywhere is
    /// reachable, plus [`STREET_LOOPS`] short-cuts so the map is not a tree.
    ///
    /// The loops matter more than they look. A map with none is a map where
    /// every retreat is back the way you came, which is the shape that turns a
    /// long run into a walk.
    fn carve_streets(&mut self, hubs: &[Coord]) {
        let n = hubs.len();
        if n < 2 {
            return;
        }
        let mut joined = vec![false; n];
        joined[0] = true;
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for _ in 1..n {
            let mut best: Option<(i32, usize, usize)> = None;
            for (i, inside) in joined.iter().enumerate() {
                if !inside {
                    continue;
                }
                for (j, outside) in joined.iter().enumerate() {
                    if *outside {
                        continue;
                    }
                    let dist = hubs[i].taxi(hubs[j]);
                    if best.is_none_or(|(seen, _, _)| dist < seen) {
                        best = Some((dist, i, j));
                    }
                }
            }
            let Some((_, i, j)) = best else { break };
            joined[j] = true;
            edges.push((i, j));
        }
        let mut spare: Vec<(i32, usize, usize)> = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .filter(|(i, j)| !edges.contains(&(*i, *j)) && !edges.contains(&(*j, *i)))
            .map(|(i, j)| (hubs[i].taxi(hubs[j]), i, j))
            .collect();
        spare.sort_unstable();
        edges.extend(spare.into_iter().take(STREET_LOOPS).map(|(_, i, j)| (i, j)));
        for (i, j) in edges {
            self.carve_street(hubs[i], hubs[j]);
        }
    }

    /// One street: two straight legs meeting at an elbow.
    fn carve_street(&mut self, from: Coord, to: Coord) {
        let elbow = if rand_inclusive(&mut self.rng, 0, 1) == 0 {
            Coord::new(to.x, from.y)
        } else {
            Coord::new(from.x, to.y)
        };
        self.carve_run(from, elbow);
        self.carve_run(elbow, to);
    }

    /// One straight leg, two cells wide.
    ///
    /// Two rather than one on purpose: a single-file corridor is a place a
    /// hundred-cell amoeba goes to get stuck, and every one of them is a turn
    /// spent shuffling rather than playing.
    fn carve_run(&mut self, from: Coord, to: Coord) {
        let step = Coord::new((to.x - from.x).signum(), (to.y - from.y).signum());
        let lane = Coord::new(step.y.abs(), step.x.abs());
        let steps = (to.x - from.x).abs().max((to.y - from.y).abs());
        for i in 0..=steps {
            let c = from + Coord::new(step.x * i, step.y * i);
            self.carve(c);
            self.carve(c + lane);
        }
    }

    // -- gates --------------------------------------------------------------

    /// Sink every gate into the rock at the end of its own passage, keeping
    /// them as far apart as the map will allow.
    fn place_gates(&mut self) -> bool {
        let mut placed = 0;
        for gap in GATE_GAPS {
            let mut sites = self.gatehouse_sites();
            self.shuffle(&mut sites);
            for (from, dir) in sites {
                if placed >= self.rules.num_cities {
                    return true;
                }
                let far = self
                    .cities
                    .iter()
                    .filter_map(|id| self.actors.get(*id))
                    .all(|gate| gate.pos.taxi(from) >= gap);
                if far && self.dig_gatehouse(from, dir) {
                    placed += 1;
                }
            }
            if placed >= self.rules.num_cities {
                return true;
            }
        }
        false
    }

    /// Every floor cell with rock beside it, paired with the way the rock lies.
    ///
    /// A cheap sieve, not a promise: [`Sim::dig_gatehouse`] is what decides
    /// whether a passage really fits, because that depends on what has been dug
    /// since.
    fn gatehouse_sites(&self) -> Vec<(Coord, Coord)> {
        let mut out = Vec::new();
        for y in 0..self.grid.height() {
            for x in 0..self.grid.width() {
                let c = Coord::new(x, y);
                if !self.grid.walkable(c) {
                    continue;
                }
                for dir in [Dir::Left, Dir::Right, Dir::Up, Dir::Down] {
                    if !self.grid.walkable(c + dir.offset()) {
                        out.push((c, dir.offset()));
                    }
                }
            }
        }
        out
    }

    /// Cut one dead-end passage into the rock and seat a gate at the end of it.
    ///
    /// Every cell of the passage has to be rock with rock down both flanks
    /// before it is cut, and the gate needs rock on its three other sides. That
    /// is what leaves the finished gate with exactly one doorway — the property
    /// the whole "hold the doorway" tactic rests on — and what stops a second
    /// passage from being dug alongside an existing gate and quietly giving it
    /// a second way out.
    fn dig_gatehouse(&mut self, from: Coord, dir: Coord) -> bool {
        let flank = Coord::new(dir.y.abs(), dir.x.abs());
        let along = |k: i32| from + Coord::new(dir.x * k, dir.y * k);
        for len in (GATEHOUSE_MIN..=GATEHOUSE_MAX).rev() {
            let solid = |sim: &Self, c: Coord| {
                !sim.grid.walkable(c)
                    && !sim.grid.walkable(c + flank)
                    && !sim.grid.walkable(c - flank)
            };
            let gate = along(len + 1);
            let fits = (1..=len).all(|k| {
                let c = along(k);
                c.x > 0
                    && c.y > 0
                    && c.x < self.grid.width() - 1
                    && c.y < self.grid.height() - 1
                    && solid(self, c)
                    && !self.touches_gate(c)
            }) && self.grid.in_bounds(gate)
                && solid(self, gate)
                && !self.grid.walkable(gate + dir)
                && !self.touches_gate(gate)
                && self.is_empty_cell(gate);
            if !fits {
                continue;
            }
            for k in 1..=len {
                self.carve(along(k));
            }
            self.add_actor(Kind::City, gate);
            return true;
        }
        false
    }

    /// Whether a gate is standing on this cell or beside it.
    fn touches_gate(&self, c: Coord) -> bool {
        std::iter::once(c)
            .chain(self.grid.adjacent(c))
            .filter_map(|n| self.actor_at(n))
            .filter_map(|id| self.actors.get(id))
            .any(|a| actors::is_city(a.kind))
    }

    // -- the depth field ----------------------------------------------------

    /// Flood outward from every gate doorway and record how far each cell is
    /// from the nearest gate.
    ///
    /// Rock and anything a gate cannot reach keep [`i32::MAX`]. Nothing but the
    /// gates has been placed when this runs, so "walkable" here is exactly "cut
    /// out of the rock" and the answer is a property of the map rather than of
    /// whoever happens to be standing on it.
    fn measure_depth(&mut self) {
        let cells = (self.grid.width() * self.grid.height()).unsigned_abs() as usize;
        let mut depth = vec![i32::MAX; cells];
        let mut doors: Vec<Coord> = Vec::new();
        for id in &self.cities {
            if let Some(gate) = self.actors.get(*id) {
                doors.extend(self.grid.adjacent_walkable(gate.pos));
            }
        }
        let mut queue = VecDeque::new();
        for door in doors {
            if let Some(i) = self.cell_index(door)
                && depth[i] != 0
            {
                depth[i] = 0;
                queue.push_back(door);
            }
        }
        while let Some(at) = queue.pop_front() {
            let Some(here) = self.cell_index(at).map(|i| depth[i]) else {
                continue;
            };
            for next in self.grid.adjacent(at) {
                if let Some(i) = self.cell_index(next)
                    && depth[i] == i32::MAX
                    && self.grid.walkable(next)
                {
                    depth[i] = here + 1;
                    queue.push_back(next);
                }
            }
        }
        self.depth = depth;
    }

    /// The depth a share of the way from the gates to the deepest cell there
    /// is. Used to name bands — "out on the streets", "down in the deep" —
    /// without hard-coding a number that a wider map would make wrong.
    fn depth_at_percent(&self, percent: i32) -> i32 {
        let deepest = self
            .depth
            .iter()
            .copied()
            .filter(|d| *d != i32::MAX)
            .max()
            .unwrap_or(0);
        deepest * percent / 100
    }

    // -- the amoeba ---------------------------------------------------------

    /// Seed the player in the deepest chamber the map has: a blob of six
    /// adjacent cells holding a nucleus at each end and cytoplasm between.
    ///
    /// Waking up at the far end of the depth field is what gives the run its
    /// opening. The first wave is fifty turns away and every gate is a walk, so
    /// those turns are yours to grow in — and the direction the humans will
    /// come from is the direction you have to go.
    fn place_starting_mass(&mut self) -> bool {
        let mut hubs = self.junctions.clone();
        self.shuffle(&mut hubs);
        // Rank chambers by how far into the deep they reach rather than by
        // where their middle falls: the centre of mass of a crescent says
        // nothing about the end of it.
        hubs.sort_by_key(|c| std::cmp::Reverse(self.deepest_near(*c)));
        for hub in hubs.into_iter().take(START_CHAMBERS) {
            let mut seeds: Vec<Coord> = self
                .grid
                .cells_in_diamond(hub, START_SPREAD)
                .filter(|c| self.grid.walkable(*c))
                .collect();
            seeds.sort_by_key(|c| std::cmp::Reverse(self.depth_at(*c)));
            seeds.truncate((seeds.len() / 3).max(1));
            for _ in 0..START_ATTEMPTS {
                let Some(seed) = self.pick(&seeds) else { break };
                let Some(blob) = self.fluid_select(seed, START_MASS) else {
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
        }
        false
    }

    /// The deepest cell a chamber reaches, measured over the same
    /// neighbourhood the blob can be seeded in.
    fn deepest_near(&self, hub: Coord) -> i32 {
        self.grid
            .cells_in_diamond(hub, START_SPREAD)
            .filter(|c| self.grid.walkable(*c))
            .map(|c| self.depth_at(c))
            .filter(|d| *d != i32::MAX)
            .max()
            .unwrap_or(0)
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

    // -- what is lying about -------------------------------------------------

    /// Everything on the floor, each kind placed for its own reason.
    fn place_features(&mut self) {
        self.fortify_gates();
        self.stock_caches();
        self.seed_the_deep();
    }

    /// Barbed wire, in a ring outside each gate. It is human fortification, and
    /// the humans fortify their gates; taking it is walking up to one.
    fn fortify_gates(&mut self) {
        let mut gates: Vec<ActorId> = self.cities.clone();
        self.shuffle(&mut gates);
        if gates.is_empty() {
            return;
        }
        for i in 0..WIRE_AMT {
            #[allow(clippy::cast_sign_loss)] // `i` counts up from zero.
            let gate = gates[i as usize % gates.len()];
            let Some(door) = self
                .actors
                .get(gate)
                .and_then(|a| self.grid.adjacent_walkable(a.pos).next())
            else {
                continue;
            };
            let spots: Vec<Coord> = self
                .grid
                .cells_in_diamond(door, WIRE_RING.1)
                .filter(|c| {
                    c.taxi(door) >= WIRE_RING.0 && self.grid.walkable(*c) && self.is_empty_cell(*c)
                })
                .collect();
            if let Some(spot) = self.pick(&spots) {
                self.add_item(ItemKind::BarbedWire, spot);
            }
        }
    }

    /// Nutrients in caches out along the streets, rather than one at a time
    /// everywhere. A clump is a thing somebody left somewhere; a scatter is
    /// weather.
    fn stock_caches(&mut self) {
        let (low, high) = (
            self.depth_at_percent(CACHE_BAND.0),
            self.depth_at_percent(CACHE_BAND.1),
        );
        let mut sites: Vec<Coord> = self
            .open_cells()
            .into_iter()
            .filter(|c| (low..=high).contains(&self.depth_at(*c)))
            .collect();
        self.shuffle(&mut sites);
        let mut left = FOOD_AMT;
        let mut caches: Vec<Coord> = Vec::new();
        for site in sites {
            if left <= 0 {
                break;
            }
            if !self.is_empty_cell(site) || caches.iter().any(|c| c.taxi(site) < CACHE_GAP) {
                continue;
            }
            caches.push(site);
            let size = rand_inclusive(&mut self.rng, CACHE_SIZE.0, CACHE_SIZE.1).min(left);
            let mut spots: Vec<Coord> = self
                .grid
                .cells_in_diamond(site, CACHE_SPREAD)
                .filter(|c| self.grid.walkable(*c) && self.is_empty_cell(*c))
                .collect();
            self.shuffle(&mut spots);
            #[allow(clippy::cast_sign_loss)] // `size` is at least `CACHE_SIZE.0`.
            for spot in spots.into_iter().take(size as usize) {
                self.add_item(ItemKind::Nutrient, spot);
                left -= 1;
            }
        }
        // A map with nowhere in the band left over still owes the player its
        // food, so whatever is unspent goes wherever there is room for it.
        let mut spare = self.open_cells();
        self.shuffle(&mut spare);
        for spot in spare {
            if left <= 0 {
                break;
            }
            if self.is_empty_cell(spot) {
                self.add_item(ItemKind::Nutrient, spot);
                left -= 1;
            }
        }
    }

    /// Plants and DNA, both down in the deep and for the same reason: they are
    /// what is left where the humans do not go. The DNA goes further still,
    /// into the dead ends, because a second nucleus should cost a walk.
    fn seed_the_deep(&mut self) {
        let far = self.depth_at_percent(DEEP_BAND);
        let mut deep: Vec<Coord> = self
            .open_cells()
            .into_iter()
            .filter(|c| {
                let d = self.depth_at(*c);
                d >= far && d != i32::MAX
            })
            .collect();
        self.shuffle(&mut deep);
        // A dead end is measured against the rock rather than against who is
        // standing where, so the amoeba the generator has already placed does
        // not invent alcoves that are not there.
        let mut ends: Vec<Coord> = deep
            .iter()
            .copied()
            .filter(|c| self.grid.adjacent(*c).filter(|n| !self.is_wall(*n)).count() <= 1)
            .collect();
        ends.extend(deep.iter().copied());
        self.scatter(&ends, ItemKind::Dna, DNA_AMT);
        self.scatter(&deep, ItemKind::Plant, PLANT_AMT);
    }

    /// Drop `amount` of one kind on the first cells of `spots` with room.
    fn scatter(&mut self, spots: &[Coord], kind: ItemKind, amount: i32) {
        let mut left = amount;
        for spot in spots {
            if left <= 0 {
                return;
            }
            if self.is_empty_cell(*spot) && self.add_item(kind, *spot).is_some() {
                left -= 1;
            }
        }
    }

    /// Every cell cut out of the rock, in reading order.
    fn open_cells(&self) -> Vec<Coord> {
        (0..self.grid.height())
            .flat_map(|y| (0..self.grid.width()).map(move |x| Coord::new(x, y)))
            .filter(|c| self.grid.walkable(*c))
            .collect()
    }

    /// Fisher-Yates through the run's own RNG, so a shuffled list is as
    /// reproducible as everything else here.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            #[allow(clippy::cast_sign_loss)]
            let j = rand_inclusive(&mut self.rng, 0, i as i32) as usize;
            items.swap(i, j);
        }
    }

    // -- the connectivity backstop ------------------------------------------

    /// Tunnel between any regions the carving left disjoint.
    ///
    /// The streets already join every chamber, so on almost every map this is a
    /// no-op that runs once and finds one region. It stays because "almost
    /// every" is not a guarantee and an unreachable pocket is an unwinnable
    /// run.
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

    /// Four-connected components of the walkable space.
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
            self.carve(Coord::new(x, from.y));
        }
        for y in from.y.min(to.y)..=from.y.max(to.y) {
            self.carve(Coord::new(to.x, y));
        }
    }
}

/// The largest four-connected run of `true` in a `width` by `height` bitmap,
/// as indices into it.
fn largest_region(open: &[bool], width: i32, height: i32) -> Option<Vec<usize>> {
    #[allow(clippy::cast_sign_loss)] // Both sides are checked positive by the caller.
    let at = |col: i32, row: i32| (row * width + col) as usize;
    let mut seen = vec![false; open.len()];
    let mut best: Option<Vec<usize>> = None;
    for start in 0..open.len() {
        if !open[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        let mut members = vec![start];
        let mut queue = VecDeque::from([start]);
        while let Some(here) = queue.pop_front() {
            #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let (col, row) = (here as i32 % width, here as i32 / width);
            for step in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (col, row) = (col + step.0, row + step.1);
                if col < 0 || row < 0 || col >= width || row >= height {
                    continue;
                }
                let next = at(col, row);
                if open[next] && !seen[next] {
                    seen[next] = true;
                    members.push(next);
                    queue.push_back(next);
                }
            }
        }
        if best
            .as_ref()
            .is_none_or(|found| members.len() > found.len())
        {
            best = Some(members);
        }
    }
    best
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
        (0..sim.grid().height())
            .flat_map(|y| (0..sim.grid().width()).map(move |x| Coord::new(x, y)))
            .filter(|c| !sim.is_wall(*c))
            .count()
    }

    /// Everything on the floor of one map, by kind.
    fn loot(sim: &Sim) -> Vec<(ItemKind, Coord)> {
        (0..sim.grid().height())
            .flat_map(|y| (0..sim.grid().width()).map(move |x| Coord::new(x, y)))
            .filter_map(|c| sim.item_at(c).map(|id| (sim.items()[id].kind, c)))
            .collect()
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
    fn the_cavern_has_room_to_move_in_but_is_not_an_open_box() {
        // Chambers and streets: enough floor for a hundred-cell amoeba to live
        // on, and enough rock left that the map has a shape.
        for seed in 0..12 {
            let sim = playing(seed);
            let open = open_cells(&sim);
            let all = (sim.grid().width() * sim.grid().height()).unsigned_abs() as usize;
            let share = open * 100 / all;
            assert!(share > 20, "seed {seed}: only {share}% of the map is floor");
            assert!(share < 70, "seed {seed}: {share}% floor is an arena");
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
    fn the_amoeba_wakes_up_deep() {
        // The opening fifty turns are only the player's if the humans have a
        // walk ahead of them, so the blob is seeded at the far end of the depth
        // field: almost none of the cavern is further from a gate than it is.
        for seed in 0..24 {
            let sim = playing(seed);
            let floor: Vec<i32> = (0..sim.grid().height())
                .flat_map(|y| (0..sim.grid().width()).map(move |x| Coord::new(x, y)))
                .map(|c| sim.depth_at(c))
                .filter(|d| *d != i32::MAX)
                .collect();
            let here = sim.depth_at(sim.actors()[sim.player_mass()[0]].pos);
            let deeper = floor.iter().filter(|d| **d > here).count();
            assert!(
                deeper * 5 < floor.len(),
                "seed {seed}: woke up {here} from a gate, with {deeper} of {} cells deeper",
                floor.len()
            );
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
                let doors = sim.grid().adjacent_walkable(pos).count();
                assert_eq!(doors, 1, "seed {seed}: gate at {pos:?}");
            }
        }
    }

    #[test]
    fn gates_are_spread_around_the_map() {
        // Twelve gates all opening onto one chamber would be one fight and a
        // long walk, which is the shape the spread is there to break up.
        for seed in 0..12 {
            let sim = playing(seed);
            let at: Vec<Coord> = sim
                .cities()
                .iter()
                .map(|id| sim.actors()[*id].pos)
                .collect();
            for (i, a) in at.iter().enumerate() {
                let nearest = at
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, b)| a.taxi(*b))
                    .min()
                    .expect("more than one gate");
                assert!(nearest >= 2, "seed {seed}: two gates at {a:?}");
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
            assert_eq!(sim.actors()[*id].armor, 32);
        }
    }

    #[test]
    fn loot_counts_match_the_generator() {
        for seed in 0..10 {
            let sim = playing(seed);
            let mut counts = [0_i32; 6];
            for (kind, _) in loot(&sim) {
                let slot = match kind {
                    ItemKind::Nutrient => 0,
                    ItemKind::CalciumDust => 1,
                    ItemKind::SiliconDust => 2,
                    ItemKind::BarbedWire => 3,
                    ItemKind::Plant => 4,
                    ItemKind::Dna => 5,
                };
                counts[slot] += 1;
            }
            assert_eq!(counts[0], FOOD_AMT, "seed {seed} nutrients");
            assert_eq!(counts[3], WIRE_AMT, "seed {seed} wire");
            assert_eq!(counts[4], PLANT_AMT, "seed {seed} plants");
            assert_eq!(counts[5], DNA_AMT, "seed {seed} dna");
            assert_eq!(counts[1] + counts[2], 0, "no dust is scattered at start");
        }
    }

    #[test]
    fn wire_lies_at_the_gates_and_the_greenery_lies_deep() {
        // The whole point of the depth field: where a thing is says what it is
        // doing there.
        for seed in 0..12 {
            let sim = playing(seed);
            let depth = |c: Coord| sim.depth_at(c);
            let mean = |kind: ItemKind| {
                let of: Vec<i32> = loot(&sim)
                    .into_iter()
                    .filter(|(k, _)| *k == kind)
                    .map(|(_, c)| depth(c))
                    .filter(|d| *d != i32::MAX)
                    .collect();
                #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                let n = of.len().max(1) as i32;
                of.iter().sum::<i32>() / n
            };
            let wire = mean(ItemKind::BarbedWire);
            let food = mean(ItemKind::Nutrient);
            let plants = mean(ItemKind::Plant);
            assert!(wire < food, "seed {seed}: wire {wire} vs caches {food}");
            assert!(
                food < plants,
                "seed {seed}: caches {food} vs plants {plants}"
            );
        }
    }

    #[test]
    fn nutrients_come_in_caches_rather_than_confetti() {
        for seed in 0..8 {
            let sim = playing(seed);
            let food: Vec<Coord> = loot(&sim)
                .into_iter()
                .filter(|(k, _)| *k == ItemKind::Nutrient)
                .map(|(_, c)| c)
                .collect();
            let lonely = food
                .iter()
                .filter(|c| {
                    !food
                        .iter()
                        .any(|o| *o != **c && o.taxi(**c) <= CACHE_SPREAD * 2)
                })
                .count();
            assert!(
                lonely * 4 < food.len(),
                "seed {seed}: {lonely} of {} nutrients are on their own",
                food.len()
            );
        }
    }

    #[test]
    fn items_never_share_a_cell_with_each_other() {
        let sim = playing(21);
        let mut seen =
            vec![false; (sim.grid().width() * sim.grid().height()).unsigned_abs() as usize];
        for y in 0..sim.grid().height() {
            for x in 0..sim.grid().width() {
                let c = Coord::new(x, y);
                if sim.item_at(c).is_some() {
                    let i = (c.y * sim.grid().width() + c.x).unsigned_abs() as usize;
                    assert!(!seen[i]);
                    seen[i] = true;
                    assert!(sim.grid().walkable(c) || sim.actor_at(c).is_some());
                }
            }
        }
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
        assert_eq!(a.junctions(), b.junctions());
    }

    #[test]
    fn every_chamber_is_somewhere_you_can_stand() {
        for seed in 0..12 {
            let sim = playing(seed);
            assert!(
                sim.junctions().len() >= CHAMBERS_MIN,
                "seed {seed}: {} chambers",
                sim.junctions().len()
            );
            for hub in sim.junctions() {
                assert!(
                    !sim.is_wall(*hub),
                    "seed {seed}: chamber centre {hub:?} is rock"
                );
            }
        }
    }
}
