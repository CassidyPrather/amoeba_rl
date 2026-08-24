//! Map generation.
//!
//! There are two caverns, and which one a run is played in is part of what a
//! difficulty *is* — see [`CavernStyle`]. Normal and Easy are played in a
//! [`warren`]: small chambers on a jittered lattice, passages that wander
//! between one and two cells wide, and a pillar pass that leaves no cell of the
//! map more than a step from rock. GJ is played in the [`classic`] districts:
//! the rock cut into districts, a chamber dissolved out of the middle of each,
//! and streets two cells wide between them. A warren is a cavern to fight in
//! and the districts are a cavern to cross, and sixteen gates with no grace is
//! a run that is mostly crossing.
//!
//! Everything either of them hands back goes through the same rest of the
//! generator, which is what this file is:
//!
//! 1. **Join what came apart.** [`Sim::connect_pockets`] tunnels between any
//!    regions the carving left disjoint, because an unreachable pocket is an
//!    unwinnable run.
//! 2. **Sink the gates.** [`Sim::place_gates`] puts every gate in the rock at
//!    the end of its own dead-end passage, spread as far apart as the map
//!    allows. A gate with exactly one doorway is what the whole "hold the
//!    doorway" tactic rests on, and everything downstream is careful not to
//!    give it a second.
//! 3. **Tighten the warren, and then let the air back in.** The erosion and the
//!    pillars are the warren's own — the districts skip both — and
//!    [`Sim::open_the_chokepoints`] is what stops the tightening from going too
//!    far. A throat is somewhere to meet a crowd on your terms; a throat that
//!    is the *only* way between two halves of the cavern is a door, and the
//!    humans walk armour through this cavern that stands in doors. So nothing
//!    that corks off [`CHOKE_POCKET`] cells stays a single cell wide: it gets a
//!    second way round, which is a decision, in place of a wall with a wagon in
//!    it, which was a wait.
//! 4. **Measure the depth.** [`Sim::measure_depth`] floods outward from every
//!    gate doorway and records how far each cell is from the nearest one.
//! 5. **Put everything else where the depth field says it goes.**
//!
//! That last number is what the rest of the generator asks about everything it
//! places:
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

mod classic;
mod warren;

/// Which of the two generators lays a difficulty's cavern out.
///
/// Two, rather than one, because the map is the difficulty. A warren is a
/// cavern the amoeba can fight in; the districts are a cavern it has to cross.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CavernStyle {
    /// Small chambers on a jittered lattice, joined by passages that wander
    /// between one and two cells wide, tightened afterwards with erosion and
    /// pillars. See [`warren`].
    Warren,
    /// The underside of a human city: the rock cut into districts, a chamber
    /// dissolved out of the middle of each, streets two cells wide between
    /// them. See [`classic`].
    Districts,
}

/// What a cavern generator hands the rest of generation.
struct Cavern {
    /// The middle of every chamber. The humans anchor their beats and patrols
    /// on these and the amoeba wakes up in one of them.
    hubs: Vec<Coord>,
    /// Cells that the erosion and the pillars must leave alone, by cell index.
    /// A generator that tightens nothing hands back a mask of all `false`.
    open_ground: Vec<bool>,
}

/// Cells that have to be stranded behind one cell before it counts as a
/// chokepoint rather than as a dead end.
///
/// Dead ends are the point of half this generator — a gatehouse is four cells
/// of blind passage and an alcove is one — so the number has to sit above them
/// and below anything that could be called part of the map. Twelve is about
/// two per cent of a warren's floor.
const CHOKE_POCKET: usize = 12;
/// Bypasses one map may be cut before the pass calls it a day.
///
/// A backstop only: every bypass kills the whole run of chokepoints along the
/// passage it goes round, so maps run out of them long before this.
const CHOKE_ROUNDS: u32 = 24;
/// How far from a chokepoint a bypass has to leave the old route, taxicab.
///
/// Anything less and the "bypass" is the same throat one cell over, which is
/// still one caravan wide.
const CHOKE_CLEAR: i32 = 3;
/// How far from a chokepoint a bypass may look for its far end, taxicab.
///
/// A way round is only a way round if taking it is not its own expedition, and
/// a long tunnel is a long scar through the warren besides.
const CHOKE_REACH: i32 = 14;

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
        let style = self.difficulty.cavern();
        let Some(cavern) = (match style {
            CavernStyle::Warren => self.carve_warren(),
            CavernStyle::Districts => self.carve_districts(),
        }) else {
            return false;
        };
        self.connect_pockets();
        if !self.place_gates() {
            return false;
        }
        let Cavern {
            hubs,
            mut open_ground,
        } = cavern;
        if style == CavernStyle::Warren {
            self.clear_the_approaches(&mut open_ground);
            self.erode(&open_ground);
            self.plant_pillars(&open_ground);
            self.open_the_chokepoints(&open_ground);
        }
        self.measure_depth();
        // Planting can put a pillar on a chamber's own middle. The humans anchor
        // their beats and patrols on these, so each one is nudged to whatever
        // open ground is nearest — which, a pillar being a single cell in an
        // otherwise open place, is always right beside it.
        self.junctions = hubs.into_iter().map(|at| self.on_open_ground(at)).collect();
        if !self.place_starting_mass() {
            return false;
        }
        self.place_features(&open_ground);
        true
    }

    /// This cell if it is open, otherwise whatever open cell is next to it.
    fn on_open_ground(&self, at: Coord) -> Coord {
        if self.grid.walkable(at) {
            return at;
        }
        self.grid.adjacent_walkable(at).next().unwrap_or(at)
    }

    /// Whether a per-cell mask is set here. Anything off the map reads false.
    fn marked(&self, mask: &[bool], at: Coord) -> bool {
        self.cell_index(at).is_some_and(|i| mask[i])
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
    fn place_features(&mut self, open_ground: &[bool]) {
        self.fortify_gates();
        self.stock_caches(open_ground);
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

    /// Nutrients in caches, out along the passages and by preference in the
    /// galleries.
    ///
    /// A clump is a thing somebody left somewhere; a scatter is weather. And a
    /// gallery is where anybody would stack supplies — which is convenient,
    /// because a gallery is also the one place on the map where nothing can be
    /// cornered and a shot crosses the whole room. The food being in the
    /// dangerous rooms is the point: a cavern where the amoeba is safe
    /// everywhere it needs to go is a cavern with nothing to decide.
    fn stock_caches(&mut self, open_ground: &[bool]) {
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
        // The middle of a hall first, then anywhere else in one, then the rest
        // of the band behind them — so a map that came out short of galleries
        // still gets fed. The shuffle above is what decides the order inside
        // each of the three.
        sites.sort_by_key(|c| {
            u8::from(!self.wide_open(*c)) * 2 + u8::from(!self.marked(open_ground, *c))
        });
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
            spots.sort_by_key(|c| u8::from(!self.marked(open_ground, *c)));
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

    // -- the chokepoints ----------------------------------------------------

    /// Give every chokepoint on the map a way round it.
    ///
    /// A warren is made of throats, which is the whole of what it is for. But a
    /// throat that is also the *only* way between two halves of the cavern is
    /// not a place to meet a crowd on your terms, it is a door — and the humans
    /// walk things through the cavern that stand in doors. A caravan is armoured
    /// enough that an early amoeba cannot eat it and slow enough that it does
    /// not move on, and one parked in the wrong cell used to mean the run was
    /// over as a game and continued as a wait.
    ///
    /// So: nothing that strands [`CHOKE_POCKET`] cells stays a single cell wide.
    /// The fix is a loop rather than a widening, because widening the throat
    /// would take the throat away, and the throats are what the amoeba fights
    /// with. What the player gets is a second way round, which is a decision;
    /// what they had was a wall with a wagon in it.
    ///
    /// It runs after the pillars because the pillars are what put most of these
    /// chokepoints there, and it plants them again afterwards because a bypass
    /// is cut out of rock and can open a hall on its way past.
    fn open_the_chokepoints(&mut self, open_ground: &[bool]) {
        // Where a bypass must not go, worked out once: the gates are in and
        // nothing moves them again.
        let beside_a_gate: Vec<bool> = (0..self.grid.height())
            .flat_map(|y| (0..self.grid.width()).map(move |x| Coord::new(x, y)))
            .map(|at| self.touches_gate(at))
            .collect();
        for _ in 0..CHOKE_ROUNDS {
            if !self.bypass_a_chokepoint(&beside_a_gate) {
                break;
            }
        }
        self.plant_pillars(open_ground);
    }

    /// Cut a way round the worst chokepoint the map still has.
    ///
    /// `false` when there is none left, or none left that can be gone round.
    /// Worst first — the more cells a cell strands, the more of the run is
    /// spent behind it — and on down the list, because a chokepoint whose
    /// bypass would have to run alongside a gate is one this pass has to leave
    /// alone rather than one it should stop at.
    fn bypass_a_chokepoint(&mut self, beside_a_gate: &[bool]) -> bool {
        let mut cuts = self.chokepoints();
        cuts.sort_by_key(|(_, behind)| std::cmp::Reverse(*behind));
        cuts.into_iter()
            .any(|(cut, _)| self.carve_bypass(cut, beside_a_gate))
    }

    /// Every cell that corks off [`CHOKE_POCKET`] cells or more, with how many
    /// it corks off.
    ///
    /// One depth-first walk of the whole cavern finds all of them at once.
    /// A cell is a chokepoint when some branch of the walk below it never
    /// found a way back to anything above it: that branch is then reachable
    /// only through this cell, and its size is what filling the cell in would
    /// strand. What is left over — everything not in one of those branches —
    /// is a piece too, and on an ordinary corridor cell it is the whole rest of
    /// the map.
    ///
    /// The number reported is the second largest of those pieces rather than
    /// the smallest: a cell can cork a two-cell alcove and half the map at the
    /// same time, and it is the half of the map that decides whether anybody
    /// cares.
    fn chokepoints(&self) -> Vec<(Coord, usize)> {
        let cells = self.depth.len();
        // `seen` doubles as the discovery order, counting from one so that zero
        // means a cell the walk has not reached.
        let mut seen = vec![0_u32; cells];
        let mut low = vec![0_u32; cells];
        let mut size = vec![1_u32; cells];
        let mut parent = vec![usize::MAX; cells];
        // The two largest branches cut off below each cell, and everything cut
        // off below it put together.
        let mut biggest = vec![(0_u32, 0_u32); cells];
        let mut cut_off = vec![0_u32; cells];
        let mut order = 0;
        let mut stack: Vec<(Coord, usize, usize)> = Vec::new();
        let mut out = Vec::new();
        for root in self.open_cells() {
            let Some(r) = self.cell_index(root) else {
                continue;
            };
            if seen[r] != 0 {
                continue;
            }
            let before = order;
            order += 1;
            seen[r] = order;
            low[r] = order;
            stack.push((root, r, 0));
            let mut reached = 1_u32;
            while let Some((at, i, step)) = stack.last_mut() {
                let (at, i) = (*at, *i);
                let Some(dir) = [Dir::Left, Dir::Right, Dir::Up, Dir::Down].get(*step) else {
                    // Nothing left below this cell: hand what the walk found
                    // back up to whatever it came from.
                    stack.pop();
                    let up = parent[i];
                    if up != usize::MAX {
                        low[up] = low[up].min(low[i]);
                        size[up] += size[i];
                        if low[i] >= seen[up] {
                            let (first, second) = biggest[up];
                            biggest[up] = if size[i] > first {
                                (size[i], first)
                            } else {
                                (first, second.max(size[i]))
                            };
                            cut_off[up] += size[i];
                        }
                    }
                    continue;
                };
                *step += 1;
                let next = at + dir.offset();
                let Some(j) = self.cell_index(next).filter(|_| self.grid.walkable(next)) else {
                    continue;
                };
                if seen[j] == 0 {
                    order += 1;
                    reached += 1;
                    seen[j] = order;
                    low[j] = order;
                    parent[j] = i;
                    stack.push((next, j, 0));
                } else if j != parent[i] {
                    low[i] = low[i].min(seen[j]);
                }
            }
            // Now the walk of this piece of cavern is finished, every cell in it
            // knows what hangs off it, and the pieces one cell would leave
            // behind are those branches and whatever is not in any of them.
            for at in self.open_cells() {
                let Some(i) = self.cell_index(at) else {
                    continue;
                };
                if seen[i] <= before || seen[i] > order || cut_off[i] == 0 {
                    continue;
                }
                let rest = reached - 1 - cut_off[i];
                let (first, second) = biggest[i];
                let mut pieces = [first, second, rest];
                pieces.sort_unstable_by_key(|size| std::cmp::Reverse(*size));
                let behind = pieces[1] as usize;
                if behind >= CHOKE_POCKET {
                    out.push((at, behind));
                }
            }
        }
        out
    }

    /// What the cavern falls into when one cell is filled in, largest piece
    /// first.
    ///
    /// Only the pieces that touch the cell are collected, which is all of them:
    /// [`Sim::connect_pockets`] has already made the rest of the map one piece,
    /// so anything that comes apart here comes apart around this cell.
    fn sides_of(&self, cut: Coord) -> Vec<Vec<Coord>> {
        let Some(i) = self.cell_index(cut) else {
            return Vec::new();
        };
        let mut seen = vec![false; self.depth.len()];
        seen[i] = true;
        let starts: Vec<Coord> = self.grid.adjacent_walkable(cut).collect();
        let mut sides: Vec<Vec<Coord>> = Vec::new();
        for start in starts {
            if self.cell_index(start).is_some_and(|i| seen[i]) {
                continue;
            }
            sides.push(self.flood(start, &mut seen));
        }
        sides.sort_by_key(|side| std::cmp::Reverse(side.len()));
        sides
    }

    /// Every cell reachable from `from`, marking `seen` as it goes.
    fn flood(&self, from: Coord, seen: &mut [bool]) -> Vec<Coord> {
        let Some(i) = self.cell_index(from) else {
            return Vec::new();
        };
        seen[i] = true;
        let mut members = vec![from];
        let mut queue = VecDeque::from([from]);
        while let Some(at) = queue.pop_front() {
            for next in self.grid.adjacent_walkable(at) {
                if let Some(i) = self.cell_index(next)
                    && !seen[i]
                {
                    seen[i] = true;
                    members.push(next);
                    queue.push_back(next);
                }
            }
        }
        members
    }

    /// Cut the cheapest tunnel that joins the two sides of one chokepoint
    /// without going through it.
    ///
    /// Cheapest in cells of rock, because every cell this cuts is a cell the
    /// warren does not get to keep, and then in cells walked, because of two
    /// ways round the same throat the short one is the one anybody would take.
    /// Both ends have to stand [`CHOKE_CLEAR`] clear of the chokepoint, or the
    /// way round is the same throat one cell over.
    ///
    /// `false` when there is no such tunnel: nothing in reach, or every way
    /// round would run alongside a gate and hand it the second doorway the
    /// whole doorway tactic rests on it not having.
    fn carve_bypass(&mut self, cut: Coord, beside_a_gate: &[bool]) -> bool {
        let sides = self.sides_of(cut);
        let (Some(near), Some(far)) = (sides.first(), sides.get(1)) else {
            return false;
        };
        let ends = |side: &[Coord]| -> Vec<Coord> {
            side.iter()
                .copied()
                .filter(|c| (CHOKE_CLEAR..=CHOKE_REACH).contains(&c.taxi(cut)))
                .collect()
        };
        let (from, to) = (ends(near), ends(far));
        let mut best: Option<(i32, i32, Coord, Coord, Coord)> = None;
        for a in &from {
            for b in &to {
                for corner in [Coord::new(b.x, a.y), Coord::new(a.x, b.y)] {
                    let Some((rock, length)) = self.tunnel_cost(*a, corner, *b, cut, beside_a_gate)
                    else {
                        continue;
                    };
                    if best.is_none_or(|(cheapest, shortest, ..)| {
                        (rock, length) < (cheapest, shortest)
                    }) {
                        best = Some((rock, length, *a, corner, *b));
                    }
                }
            }
        }
        let Some((_, _, a, corner, b)) = best else {
            return false;
        };
        for at in elbow_path(a, corner, b) {
            self.carve(at);
        }
        true
    }

    /// What one elbow of a tunnel would cost in cells of rock and in cells
    /// walked, or `None` when it must not be cut at all.
    ///
    /// A tunnel through the chokepoint itself joins nothing to anything, and a
    /// tunnel alongside a gate gives the gate a second doorway.
    fn tunnel_cost(
        &self,
        from: Coord,
        corner: Coord,
        to: Coord,
        cut: Coord,
        beside_a_gate: &[bool],
    ) -> Option<(i32, i32)> {
        let (mut rock, mut length) = (0, 0);
        for at in run(from, corner).chain(run(corner, to)) {
            let i = self.cell_index(at)?;
            if at == cut || beside_a_gate[i] {
                return None;
            }
            length += 1;
            if !self.grid.walkable(at) {
                rock += 1;
            }
        }
        (rock > 0).then_some((rock, length))
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
        for at in elbow_path(from, Coord::new(to.x, from.y), to) {
            self.carve(at);
        }
    }
}

/// The cells of an L from `from` to `to`, turning the corner at `corner`.
fn elbow_path(from: Coord, corner: Coord, to: Coord) -> Vec<Coord> {
    run(from, corner).chain(run(corner, to)).collect()
}

/// The cells of one straight run, both ends included.
fn run(from: Coord, to: Coord) -> impl Iterator<Item = Coord> {
    let step = Coord::new((to.x - from.x).signum(), (to.y - from.y).signum());
    let steps = (to.x - from.x).abs().max((to.y - from.y).abs());
    (0..=steps).map(move |i| from + Coord::new(step.x * i, step.y * i))
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

    /// A run under way on a difficulty of your choosing, for the things that
    /// are not the same on both sides of [`CavernStyle`].
    fn played_as(seed: u64, difficulty: Difficulty) -> Sim {
        let mut sim = Sim::new(seed, difficulty);
        sim.advance(Some(Command::Start(difficulty)));
        sim
    }

    /// How many cells filling one cell in would cut off from the largest piece
    /// of cavern left standing.
    ///
    /// Worked out from the grid alone rather than through the generator's own
    /// [`Sim::sides_of`], so that it is a check on the answer and not a
    /// restatement of it. Zero for a cell that strands nothing.
    fn stranded_behind(sim: &Sim, cut: Coord) -> usize {
        let (w, h) = (sim.grid().width(), sim.grid().height());
        let index = |c: Coord| (c.y * w + c.x).unsigned_abs() as usize;
        let mut seen = vec![false; (w * h).unsigned_abs() as usize];
        seen[index(cut)] = true;
        let mut pieces: Vec<usize> = Vec::new();
        for start in sim.grid().adjacent(cut) {
            if !sim.grid().transparent(start) || seen[index(start)] {
                continue;
            }
            seen[index(start)] = true;
            let mut size = 0;
            let mut queue = VecDeque::from([start]);
            while let Some(at) = queue.pop_front() {
                size += 1;
                for next in sim.grid().adjacent(at) {
                    if sim.grid().transparent(next) && !seen[index(next)] {
                        seen[index(next)] = true;
                        queue.push_back(next);
                    }
                }
            }
            pieces.push(size);
        }
        pieces.sort_unstable_by_key(|size| std::cmp::Reverse(*size));
        pieces.get(1).copied().unwrap_or(0)
    }

    /// The cell that corks off the most cavern, and how much it corks off.
    fn worst_cork(sim: &Sim) -> (usize, Coord) {
        carved(sim)
            .into_iter()
            .filter(|c| {
                sim.grid()
                    .adjacent(*c)
                    .filter(|n| sim.grid().transparent(*n))
                    .count()
                    > 1
            })
            .map(|c| (stranded_behind(sim, c), c))
            .max()
            .unwrap_or((0, Coord::new(0, 0)))
    }

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

    /// Cells with nothing but floor in all eight directions: the middle of a
    /// hall, where the amoeba can corner nothing and a shot crosses unimpeded.
    fn wide_open(sim: &Sim) -> Vec<Coord> {
        carved(sim)
            .into_iter()
            .filter(|c| {
                (-1..=1).all(|dy| {
                    (-1..=1).all(|dx| sim.grid().transparent(Coord::new(c.x + dx, c.y + dy)))
                })
            })
            .collect()
    }

    #[test]
    fn the_warren_is_tight_and_the_galleries_are_not() {
        // The whole design in one number. Nothing wide open at all and the
        // amoeba gets every fight on its own terms; too much of it and the
        // pillars have stopped meaning anything.
        for seed in 0..24 {
            let sim = playing(seed);
            let share = wide_open(&sim).len() * 100 / carved(&sim).len();
            assert!(
                share >= 2,
                "seed {seed}: {share}% open, the map is all warren"
            );
            assert!(
                share <= 25,
                "seed {seed}: {share}% open, the pillars gave up"
            );
        }
    }

    #[test]
    fn every_map_has_a_hall_in_it() {
        // Not scattered open cells — one contiguous piece of room big enough
        // that being caught in the middle of it is a real problem.
        for seed in 0..16 {
            let sim = playing(seed);
            let open = wide_open(&sim);
            let mut seen: Vec<Coord> = Vec::new();
            let mut biggest = 0;
            for start in &open {
                if seen.contains(start) {
                    continue;
                }
                seen.push(*start);
                let mut size = 1;
                let mut queue = VecDeque::from([*start]);
                while let Some(at) = queue.pop_front() {
                    for next in sim.grid().adjacent(at) {
                        if open.contains(&next) && !seen.contains(&next) {
                            seen.push(next);
                            size += 1;
                            queue.push_back(next);
                        }
                    }
                }
                biggest = biggest.max(size);
            }
            assert!(
                biggest >= 8,
                "seed {seed}: the biggest hall is {biggest} cells"
            );
        }
    }

    #[test]
    fn the_food_is_in_the_dangerous_rooms() {
        // Growing has to mean going somewhere the warren is not helping, or the
        // warren is just a free win with a longer walk.
        for seed in 0..12 {
            let sim = playing(seed);
            let open = carved(&sim);
            let mean = |cells: &[Coord]| {
                let sum: usize = cells.iter().map(|c| openings(&sim, *c)).sum();
                sum as f64 / cells.len().max(1) as f64
            };
            let food: Vec<Coord> = loot(&sim)
                .into_iter()
                .filter(|(kind, _)| *kind == ItemKind::Nutrient)
                .map(|(_, at)| at)
                .collect();
            assert!(
                mean(&food) > mean(&open),
                "seed {seed}: caches sit at {:.2} ways out, the map at {:.2}",
                mean(&food),
                mean(&open)
            );
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
            assert!(share >= 28, "seed {seed}: only {share}% costs under four");
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
        for seed in 0..8 {
            let sim = played_as(seed, Difficulty::Gj);
            assert_eq!(
                reachable_open_cells(&sim),
                open_cells(&sim),
                "GJ seed {seed} left an unreachable pocket"
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
        // On the warren this is a claim about the bypasses too: a way round a
        // chokepoint that ran alongside a gate would give it a second door.
        let runs = (0..20)
            .map(|seed| (seed, playing(seed), 12))
            .chain((0..8).map(|seed| (seed, played_as(seed, Difficulty::Gj), 16)));
        for (seed, sim, gates) in runs {
            assert_eq!(sim.cities().len(), gates, "seed {seed}");
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
        let sim = played_as(3, Difficulty::Gj);
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
    fn no_single_cell_corks_the_warren() {
        // The guarantee the bypasses exist for. A caravan is one cell of
        // armour that an early amoeba cannot eat and that does not move on,
        // and before this there were maps where one of them could park in a
        // throat with half the cavern behind it. Now the most anybody can shut
        // away by standing in a doorway is a dead end.
        for difficulty in [Difficulty::Normal, Difficulty::Easy] {
            for seed in 0..12 {
                let sim = played_as(seed, difficulty);
                let (behind, at) = worst_cork(&sim);
                assert!(
                    behind < CHOKE_POCKET,
                    "{difficulty:?} seed {seed}: standing at {at:?} shuts {behind} cells away"
                );
            }
        }
    }

    #[test]
    fn a_bypass_is_a_way_round_rather_than_a_wider_throat() {
        // The other half of the same bargain. The warren is made of throats and
        // the throats are what the amoeba fights with, so the pass is only
        // allowed to answer a chokepoint with a second way round it — never by
        // opening the chokepoint itself out. If it were widening them the
        // cavern would be measurably slacker than the design's band, which is
        // what `the_warren_is_tight_and_the_galleries_are_not` holds, and the
        // single-file cells would be disappearing, which is this.
        for seed in 0..12 {
            let sim = playing(seed);
            let single_file = carved(&sim)
                .into_iter()
                .filter(|c| openings(&sim, *c) <= 2)
                .count();
            let share = single_file * 100 / carved(&sim).len();
            assert!(
                share >= 28,
                "seed {seed}: only {share}% of the floor is still single file"
            );
        }
    }

    #[test]
    fn gj_is_played_in_the_old_cavern() {
        // GJ is the districts: chambers dissolved out of the rock and streets
        // two cells wide between them, with no pillar pass to put a wall back
        // in the middle of anything. What that looks like from outside is a map
        // that is mostly floor and mostly floor you can stand in the middle of
        // — which is exactly what the warren is not.
        assert_eq!(Difficulty::Gj.cavern(), CavernStyle::Districts);
        assert_eq!(Difficulty::Normal.cavern(), CavernStyle::Warren);
        assert_eq!(Difficulty::Easy.cavern(), CavernStyle::Warren);
        for seed in 0..8 {
            let sim = played_as(seed, Difficulty::Gj);
            let cells = (sim.grid().width() * sim.grid().height()).unsigned_abs() as usize;
            let floor = carved(&sim).len();
            assert!(
                floor * 100 / cells > 45,
                "GJ seed {seed}: {}% floor is not the old open cavern",
                floor * 100 / cells
            );
            let room = wide_open(&sim).len() * 100 / floor;
            assert!(
                room > 40,
                "GJ seed {seed}: only {room}% of the floor has room to swing in"
            );
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
        for (difficulty, fewest) in [
            (Difficulty::Normal, warren::SITES_MIN),
            (Difficulty::Gj, classic::CHAMBERS_MIN),
        ] {
            for seed in 0..12 {
                let sim = played_as(seed, difficulty);
                assert!(
                    sim.junctions().len() >= fewest,
                    "{difficulty:?} seed {seed}: {} chambers",
                    sim.junctions().len()
                );
                for hub in sim.junctions() {
                    assert!(
                        !sim.is_wall(*hub),
                        "{difficulty:?} seed {seed}: chamber centre {hub:?} is rock"
                    );
                }
            }
        }
    }
}
