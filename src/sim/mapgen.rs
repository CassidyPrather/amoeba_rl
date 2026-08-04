//! Map generation.
//!
//! The cavern is a warren, and one rule decides its whole character: **no cell
//! of it is more than a step from rock.** [`Sim::plant_pillars`] enforces that
//! by putting rock back into anything that came out wide open, and everything
//! else here is in service of it.
//!
//! That rule is not decoration. Every mechanic this game has that is worth
//! anything needs a wall to work against:
//!
//! * **Engulfing** — the only answer to armour — captures a human with no free
//!   cell to step into. In the middle of a hall that costs four organelles. Beside
//!   a pillar it costs three, in an alcove two, at the end of a blind tendril
//!   one. A map of halls is a map where the amoeba's signature move is priced
//!   out of reach.
//! * **Cover.** A wall a shot cannot cross is the difference between being
//!   hunted and being shot at.
//! * **Chokepoints.** A passage that wanders between one and two cells wide has a
//!   throat every few cells, which is somewhere to meet a crowd on your terms.
//!
//! An earlier pass at this generated smoothed cellular-automata caverns. Their
//! jagged edges were real geometry in the wrong place. Everything that walks
//! this map walks between gates and chambers, so the ragged rim of a big room
//! is the one part of it nothing ever happens against — and the fights that
//! happened in the middle of the room happened as if the walls were not there,
//! because for those fights they were not. Interesting walls have to be *on the
//! traffic*, which is what the passages, the erosion and the pillars are
//! between them for. The map is smaller for the same reason: space nothing
//! happens in is not scenery, it is a walk.
//!
//! The order of work is: scatter chamber sites on a jittered lattice, eat a
//! small chamber out of each, join them with passages of wandering width, erode
//! alcoves and tendrils off whatever exists, plant pillars in whatever is still
//! open, then sink the gates into the rock at the end of dead-end passages.
//!
//! Everything on the floor is then placed against one number.
//! [`Sim::measure_depth`] floods outward from every gate doorway and records how
//! far each cell is from the nearest one, and that field is what the rest of the
//! generator asks:
//!
//! * **Barbed wire** goes in a ring just outside each gate, because that is
//!   where humans put barbed wire.
//! * **Nutrients** go in caches out along the passages — clumps, not confetti,
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

/// Roughly how far apart the warren's chambers sit.
const SITE_SPACING: i32 = 8;
/// How far a chamber may be jittered off the lattice, so the warren is not a
/// grid wearing a disguise.
const SITE_JITTER: i32 = 2;
/// Fewest and most cells one chamber is eaten out to, before erosion. Small on
/// purpose: a chamber is a widening of the warren, not a room.
const CHAMBER_CELLS: (i32, i32) = (12, 26);
/// Growth attempts one chamber may waste before it is called finished.
const CHAMBER_ATTEMPTS: i32 = 200;
/// Sites a map needs before it is worth playing.
const SITES_MIN: usize = 6;
/// Passages carved beyond the spanning tree, so the warren holds loops.
const PASSAGE_LOOPS: usize = 3;
/// Chance in a hundred that a passage cell gets its second lane. Anything under
/// certainty puts a throat in every few cells; anything over nothing keeps a
/// hundred-cell amoeba from having to file down the whole thing.
const PASSAGE_WIDE_PERCENT: i32 = 66;
/// Erosion passes over the whole map.
const EROSION_ROUNDS: u32 = 2;
/// Chance in a hundred that an eligible rock cell is eaten in one pass.
const EROSION_PERCENT: i32 = 22;
/// Floor neighbours a rock cell must have to be eroded: exactly one, so every
/// cell erosion takes is a blind alcove hung off something that already exists.
/// Allowing two would let it join passages together, which dissolves the
/// skeleton — and a map you cannot recognise your way around is its own kind of
/// dead space.
const EROSION_NEIGHBOURS: usize = 1;
/// Passes of pillar planting before the loop gives up. A backstop only: each
/// pass plants at least one pillar and there are finitely many cells.
const PILLAR_ROUNDS: u32 = 32;
/// Shortest gatehouse passage, in cells of rock cut away.
const GATEHOUSE_MIN: i32 = 2;
/// Longest gatehouse passage.
const GATEHOUSE_MAX: i32 = 4;
/// Shares of the map's short side that gates are asked to keep from each other,
/// tried in order. The last is a formality: by then the map has no room left.
const GATE_GAPS: [i32; 4] = [3, 4, 6, 16];
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
/// Nearest and furthest a coil of wire is dropped from a gate, in steps of open
/// floor rather than as the crow flies. In a warren the two are nothing like
/// each other — a cell three cells off can be a dozen steps round a wall — and
/// it is the walk that decides whether taking the wire means walking up to a
/// gate.
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
        self.wrecks.clear();
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
        let sites = self.sites();
        if sites.len() < SITES_MIN {
            return false;
        }
        for site in &sites {
            self.carve_chamber(*site);
        }
        self.carve_passages(&sites);
        self.erode();
        self.connect_pockets();
        self.plant_pillars();
        // Planting can put a pillar on a chamber's own middle. The humans anchor
        // their beats and patrols on these, so each one is nudged to whatever
        // open ground is nearest — which, a pillar being a single cell in an
        // otherwise open place, is always right beside it.
        self.junctions = sites
            .into_iter()
            .map(|at| self.on_open_ground(at))
            .collect();
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

    // -- the warren ----------------------------------------------------------

    /// Chamber sites: a lattice, jittered off true so the warren does not read
    /// as a grid.
    ///
    /// A lattice rather than a scatter because the spacing has to be reliable.
    /// Two chambers that land on top of each other make a hall, and a hall is
    /// the thing this generator exists to not produce. It is stretched to the
    /// map's own edges rather than stepped out from one corner, because a
    /// remainder left over at the far side is exactly the dead space the whole
    /// design is trying to be rid of.
    fn sites(&mut self) -> Vec<Coord> {
        let margin = SITE_JITTER + 2;
        let across = lattice(self.grid.width(), margin);
        let down = lattice(self.grid.height(), margin);
        let mut out = Vec::with_capacity(across.len() * down.len());
        for y in &down {
            for x in &across {
                let at = Coord::new(
                    x + rand_inclusive(&mut self.rng, -SITE_JITTER, SITE_JITTER),
                    y + rand_inclusive(&mut self.rng, -SITE_JITTER, SITE_JITTER),
                );
                out.push(Coord::new(
                    at.x.clamp(margin, self.grid.width() - 1 - margin),
                    at.y.clamp(margin, self.grid.height() - 1 - margin),
                ));
            }
        }
        out
    }

    /// Eat a small chamber out of the rock around one site.
    ///
    /// Growth is by accretion — pick a cell already eaten, eat one of its
    /// neighbours — which gives a lumpy blob rather than the smooth disc a
    /// radius would and the stringy worm a drunkard's walk would.
    fn carve_chamber(&mut self, site: Coord) {
        let want = rand_inclusive(&mut self.rng, CHAMBER_CELLS.0, CHAMBER_CELLS.1);
        let mut body = vec![site];
        self.carve(site);
        for _ in 0..CHAMBER_ATTEMPTS {
            if i32::try_from(body.len()).unwrap_or(i32::MAX) >= want {
                break;
            }
            let Some(index) = rand_index(&mut self.rng, body.len()) else {
                break;
            };
            let from = body[index];
            let Some(dir) = self.pick(&[Dir::Left, Dir::Right, Dir::Up, Dir::Down]) else {
                break;
            };
            let next = from + dir.offset();
            if body.contains(&next) || !self.carve(next) {
                continue;
            }
            body.push(next);
        }
    }

    /// Join the chambers: a minimum spanning tree so everywhere is reachable,
    /// plus [`PASSAGE_LOOPS`] short-cuts so the warren is not a tree.
    ///
    /// The loops matter more than they look. A map with none is a map where
    /// every retreat is back the way you came, which is the shape that turns a
    /// long run into a walk.
    fn carve_passages(&mut self, sites: &[Coord]) {
        let n = sites.len();
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
                    let dist = sites[i].taxi(sites[j]);
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
            .map(|(i, j)| (sites[i].taxi(sites[j]), i, j))
            .collect();
        spare.sort_unstable();
        edges.extend(
            spare
                .into_iter()
                .take(PASSAGE_LOOPS)
                .map(|(_, i, j)| (i, j)),
        );
        for (i, j) in edges {
            self.carve_passage(sites[i], sites[j]);
        }
    }

    /// One passage: two straight legs meeting at an elbow.
    fn carve_passage(&mut self, from: Coord, to: Coord) {
        let elbow = if rand_inclusive(&mut self.rng, 0, 1) == 0 {
            Coord::new(to.x, from.y)
        } else {
            Coord::new(from.x, to.y)
        };
        self.carve_run(from, elbow);
        self.carve_run(elbow, to);
    }

    /// One straight leg, wandering between one and two cells wide.
    ///
    /// The wandering is the point. Two cells everywhere is a road, and nothing
    /// happens on a road; one cell everywhere is a queue, and a hundred-cell
    /// amoeba spends its turns filing down it. Rolling for the second lane per
    /// cell puts a throat every few cells — somewhere a crowd has to come at you
    /// in ones, and somewhere a shot has to come straight down.
    fn carve_run(&mut self, from: Coord, to: Coord) {
        let step = Coord::new((to.x - from.x).signum(), (to.y - from.y).signum());
        let lane = Coord::new(step.y.abs(), step.x.abs());
        let steps = (to.x - from.x).abs().max((to.y - from.y).abs());
        for i in 0..=steps {
            let at = from + Coord::new(step.x * i, step.y * i);
            self.carve(at);
            if rand_inclusive(&mut self.rng, 0, 99) < PASSAGE_WIDE_PERCENT {
                self.carve(at + lane);
            }
        }
    }

    /// Eat at the edges of what is already there.
    ///
    /// Only rock with exactly one open neighbour is eligible, so every cell this
    /// takes is a blind alcove hung off what is already there. That is the
    /// cheapest geometry in the game — a human standing in one costs two
    /// organelles to seal instead of four — and it is bought without touching
    /// the shape of the warren, because nothing that joins two passages or fills
    /// in a corner passes the test.
    fn erode(&mut self) {
        for _ in 0..EROSION_ROUNDS {
            let mut eaten = Vec::new();
            for y in 1..self.grid.height() - 1 {
                for x in 1..self.grid.width() - 1 {
                    let at = Coord::new(x, y);
                    if self.grid.walkable(at) {
                        continue;
                    }
                    if self.grid.adjacent_walkable(at).count() == EROSION_NEIGHBOURS
                        && rand_inclusive(&mut self.rng, 0, 99) < EROSION_PERCENT
                    {
                        eaten.push(at);
                    }
                }
            }
            for at in eaten {
                self.carve(at);
            }
        }
    }

    /// Plant rock back into anything that came out wide open.
    ///
    /// A cell with all eight of its neighbours open is the middle of a hall, and
    /// a hall is where this game has nothing to offer: nothing can be cornered
    /// in one, a shot crosses it unobstructed, and there is no cell in it that
    /// costs fewer than four organelles to seal. So the middle of every hall
    /// becomes a pillar, and the rule is applied until there is no such cell
    /// left — which leaves the whole map within one step of a wall.
    ///
    /// It cannot cut the map in two, and needs no check that it has not. The
    /// four orthogonal neighbours of such a cell are joined to each other around
    /// the diagonals, which are open too by the same test, so everything that
    /// reached the cell still reaches everything else without it.
    fn plant_pillars(&mut self) {
        for _ in 0..PILLAR_ROUNDS {
            let mut open: Vec<Coord> = Vec::new();
            for y in 1..self.grid.height() - 1 {
                for x in 1..self.grid.width() - 1 {
                    let at = Coord::new(x, y);
                    if self.wide_open(at) {
                        open.push(at);
                    }
                }
            }
            if open.is_empty() {
                return;
            }
            // Shuffled, because taking them in reading order lays the pillars
            // out in ranks and the warren starts to look like a car park.
            self.shuffle(&mut open);
            for at in open {
                if self.wide_open(at) {
                    self.grid.set_props(at, false, false, false);
                }
            }
        }
    }

    /// This cell if it is open, otherwise whatever open cell is next to it.
    fn on_open_ground(&self, at: Coord) -> Coord {
        if self.grid.walkable(at) {
            return at;
        }
        self.grid.adjacent_walkable(at).next().unwrap_or(at)
    }

    /// Whether this cell and every one of its eight neighbours is open floor.
    fn wide_open(&self, at: Coord) -> bool {
        self.grid.walkable(at)
            && (-1..=1)
                .all(|dy| (-1..=1).all(|dx| self.grid.walkable(Coord::new(at.x + dx, at.y + dy))))
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

    // -- gates --------------------------------------------------------------

    /// Sink every gate into the rock at the end of its own passage, keeping
    /// them as far apart as the map will allow.
    fn place_gates(&mut self) -> bool {
        let mut placed = 0;
        let short = self.grid.width().min(self.grid.height());
        for share in GATE_GAPS {
            let gap = short / share;
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
                .cells_in_diamond(door, WIRE_RING.1 * 2)
                .filter(|c| {
                    (WIRE_RING.0..=WIRE_RING.1).contains(&self.depth_at(*c))
                        && self.grid.walkable(*c)
                        && self.is_empty_cell(*c)
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
    /// The passages already join every chamber, so on almost every map this is a
    /// no-op that runs once and finds one region. It stays because "almost
    /// every" is not a guarantee and an unreachable pocket is an unwinnable
    /// run. It goes before [`Sim::plant_pillars`] so that the pillars get the
    /// last word: a bridge carved afterwards could reopen a hall.
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

/// Evenly spaced lattice coordinates from `margin` to `span - 1 - margin`.
///
/// The count comes from [`SITE_SPACING`] but the spacing comes from the span,
/// so the lattice always reaches both edges and the sites end up a little
/// closer together than asked for rather than leaving a strip over.
fn lattice(span: i32, margin: i32) -> Vec<i32> {
    let low = margin;
    let usable = (span - 1 - margin - low).max(1);
    let count = (usable / SITE_SPACING + 1).max(2);
    (0..count).map(|i| low + usable * i / (count - 1)).collect()
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

    /// Every cell the generator cut out of the rock.
    ///
    /// Transparency, rather than walkability or [`Sim::is_wall`]: the generator
    /// sets it and nothing standing on a cell ever changes it, so this is the
    /// map's own shape and not a snapshot of who is where. A gate is rock with
    /// somebody in it and does not count.
    fn carved(sim: &Sim) -> Vec<Coord> {
        (0..sim.grid().height())
            .flat_map(|y| (0..sim.grid().width()).map(move |x| Coord::new(x, y)))
            .filter(|c| sim.grid().transparent(*c))
            .collect()
    }

    /// Open neighbours of a cell, which is also what it costs to seal somebody
    /// standing on it.
    fn openings(sim: &Sim, at: Coord) -> usize {
        sim.grid()
            .adjacent(at)
            .filter(|c| sim.grid().transparent(*c))
            .count()
    }

    #[test]
    fn nowhere_is_more_than_a_step_from_rock() {
        // The rule the whole generator is built around. Break it and there is
        // somewhere on the map where the amoeba cannot corner anything, a shot
        // cannot be broken, and the geometry may as well not be there.
        for seed in 0..24 {
            let sim = playing(seed);
            for at in carved(&sim) {
                let beside_rock = (-1..=1).any(|dy| {
                    (-1..=1).any(|dx| !sim.grid().transparent(Coord::new(at.x + dx, at.y + dy)))
                });
                assert!(
                    beside_rock,
                    "seed {seed}: {at:?} is in the middle of a hall"
                );
            }
        }
    }

    #[test]
    fn most_of_the_cavern_is_somewhere_you_could_corner_something() {
        // Engulfing is the amoeba's answer to armour and it is priced in
        // organelles: four to seal a cell in the open, three beside a pillar,
        // two in an alcove. If that price is four almost everywhere then the
        // mechanic is decorative, so a good map is mostly cheaper than four.
        for seed in 0..16 {
            let sim = playing(seed);
            let open = carved(&sim);
            let cheap = open.iter().filter(|c| openings(&sim, **c) <= 2).count();
            let share = cheap * 100 / open.len();
            assert!(share >= 30, "seed {seed}: only {share}% costs under four");
            let sum: usize = open.iter().map(|c| openings(&sim, *c)).sum();
            let mean = sum as f64 / open.len() as f64;
            assert!(mean < 3.0, "seed {seed}: mean {mean:.2} ways out of a cell");
        }
    }

    #[test]
    fn the_cavern_is_dense_without_being_solid() {
        // Floor the amoeba never reaches is a walk, and floor it cannot move
        // through is a wall. The band is what a warren looks like from outside.
        for seed in 0..16 {
            let sim = playing(seed);
            let cells = (sim.grid().width() * sim.grid().height()).unsigned_abs() as usize;
            let share = carved(&sim).len() * 100 / cells;
            assert!(share > 25, "seed {seed}: only {share}% of the map is floor");
            assert!(share < 55, "seed {seed}: {share}% floor is an arena");
        }
    }

    #[test]
    fn rock_actually_stops_a_militias_eyes() {
        // The other half of the same bargain: geometry only counts if sight
        // runs into it. A militia's diamond is forty-one cells; in a warren it
        // should be seeing well under all of them.
        let sim = playing(9);
        let radius = Kind::Militia.stats().awareness;
        let open = carved(&sim);
        let full: usize = open
            .iter()
            .map(|c| sim.grid().cells_in_diamond(*c, radius).count())
            .sum();
        let seen: usize = open
            .iter()
            .map(|c| {
                sim.grid()
                    .cells_in_diamond(*c, radius)
                    .filter(|t| sim.grid().line_of_sight(*c, *t))
                    .count()
            })
            .sum();
        let share = seen * 100 / full;
        assert!(
            share < 80,
            "rock hides almost nothing: {share}% still visible"
        );
        assert!(share > 30, "rock hides almost everything: {share}% visible");
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
        assert_eq!(sim.grid().width(), 48);
        for id in sim.cities() {
            assert_eq!(sim.actors()[*id].armor, 14);
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
                sim.junctions().len() >= SITES_MIN,
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
