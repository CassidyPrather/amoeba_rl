//! The warren: the cavern Normal and Easy are played in.
//!
//! One rule decides its character: **no cell of it is more than a step from
//! rock.** [`Sim::plant_pillars`] enforces that by putting rock back into
//! anything that came out wide open.
//!
//! That rule is not decoration. Every mechanic this game has that is worth
//! anything needs a wall to work against:
//!
//! * **Engulfing** — the only answer to armour — captures a human with no free
//!   cell to step into. In the middle of a hall that costs four organelles.
//!   Beside a pillar it costs three, in an alcove two, at the end of a blind
//!   tendril one. A map of halls is a map where the amoeba's signature move is
//!   priced out of reach.
//! * **Cover.** A wall a shot cannot cross is the difference between being
//!   hunted and being shot at.
//! * **Chokepoints.** A passage that wanders between one and two cells wide has
//!   a throat every few cells, which is somewhere to meet a crowd on your
//!   terms.
//!
//! And then the rule has exceptions, which is where the game is. A cavern that
//! is tight *everywhere* hands the amoeba every fight on its own terms, so two
//! kinds of place are exempted from the pillars and the erosion and left as
//! open ground:
//!
//! * **Galleries.** Some chambers are dug out big enough to have a middle, and
//!   keep it. Nothing can be cornered in one and a shot crosses the whole room.
//!   The nutrient caches go here by preference — a hall is where anybody would
//!   stack supplies, and it means growing means going somewhere dangerous.
//! * **Approaches.** The humans keep the ground outside their own gates clear,
//!   so the last few cells before every gate are a field of fire rather than a
//!   warren. The gate you have to break is the fight you do not get to pick.
//!
//! The order of work is: scatter chamber sites on a jittered lattice, eat a
//! chamber out of each — a few of them galleries — join them with passages of
//! wandering width, and then, once the gates are in,
//! [`Sim::clear_the_approaches`], erode alcoves and plant pillars everywhere
//! that is not open ground. What the throats the pillars leave behind are *not*
//! allowed to become is the subject of
//! [`Sim::open_the_chokepoints`].

use super::Cavern;
use crate::sim::Sim;
use crate::sim::grid::{Coord, Dir, rand_inclusive, rand_index};

/// Roughly how far apart the warren's chambers sit.
const SITE_SPACING: i32 = 8;
/// How far a chamber may be jittered off the lattice, so the warren is not a
/// grid wearing a disguise.
const SITE_JITTER: i32 = 2;
/// Fewest and most cells one chamber is eaten out to, before erosion. Small on
/// purpose: a chamber is a widening of the warren, not a room.
const CHAMBER_CELLS: (i32, i32) = (12, 26);
/// Chance in a hundred that a chamber comes out as a gallery instead.
const GALLERY_PERCENT: i32 = 28;
/// Galleries every map gets whatever the rolls said. The point of a hall is the
/// contrast with the warren around it, and a run that never met one would never
/// find out that the warren was doing it a favour.
const GALLERY_MIN: usize = 2;
/// Fewest and most cells a gallery is eaten out to, fringe included.
const GALLERY_CELLS: (i32, i32) = (30, 50);
/// Cells either side of a gallery's site that are opened as a solid block.
///
/// Accretion alone makes a sprawl as often as a room, and a sprawl has no
/// middle — every cell of it is still within reach of a wall, which would leave
/// a gallery indistinguishable from the warren it is meant to contrast with. A
/// block guarantees a hall of three by three, the accretion that follows takes
/// the straight edges off it, and two is as large as it can be without the
/// galleries swallowing the map.
const GALLERY_CORE: i32 = 2;
/// How far outside a gate the humans keep the ground clear, taxicab.
const APPROACH_RADIUS: i32 = 5;
/// Growth attempts one chamber may waste before it is called finished.
const CHAMBER_ATTEMPTS: i32 = 200;
/// Sites a map needs before it is worth playing.
pub(super) const SITES_MIN: usize = 6;
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
impl Sim {
    /// Eat the whole warren out of the rock: chambers on a jittered lattice, a
    /// few of them galleries, joined by passages.
    ///
    /// `None` means the map came out too small to be worth playing, which is
    /// the retry loop's cue to throw it away.
    pub(super) fn carve_warren(&mut self) -> Option<Cavern> {
        let hubs = self.sites();
        if hubs.len() < SITES_MIN {
            return None;
        }
        // Open ground: the cells the erosion and the pillars are told to leave
        // alone. Everywhere else comes out as warren.
        let mut open_ground = vec![false; self.depth.len()];
        let mut galleries: Vec<bool> = (0..hubs.len())
            .map(|_| rand_inclusive(&mut self.rng, 0, 99) < GALLERY_PERCENT)
            .collect();
        let mut promote: Vec<usize> = (0..hubs.len()).collect();
        self.shuffle(&mut promote);
        for i in promote {
            if galleries.iter().filter(|is| **is).count() >= GALLERY_MIN {
                break;
            }
            galleries[i] = true;
        }
        for (site, gallery) in hubs.iter().zip(&galleries) {
            let body = self.carve_chamber(*site, *gallery);
            if *gallery {
                for at in body {
                    if let Some(i) = self.cell_index(at) {
                        open_ground[i] = true;
                    }
                }
            }
        }
        self.carve_passages(&hubs);
        Some(Cavern { hubs, open_ground })
    }

    /// Mark the ground outside every gate as open.
    ///
    /// The humans keep a field of fire outside their own gates, so the last
    /// stretch of every approach is exempt from the pillars and the erosion
    /// the same way a gallery is. It runs after the gates are dug, because
    /// until then there is nothing to keep a field of fire outside of.
    pub(super) fn clear_the_approaches(&self, open_ground: &mut [bool]) {
        for gate in self.cities.clone() {
            let Some(at) = self.actors.get(gate).map(|a| a.pos) else {
                continue;
            };
            let forecourt: Vec<Coord> = self.grid.cells_in_diamond(at, APPROACH_RADIUS).collect();
            for cell in forecourt {
                if let Some(i) = self.cell_index(cell) {
                    open_ground[i] = true;
                }
            }
        }
    }

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

    /// Eat a chamber out of the rock around one site.
    ///
    /// Growth is by accretion — pick a cell already eaten, eat one of its
    /// neighbours — which gives a lumpy blob rather than the smooth disc a
    /// radius would and the stringy worm a drunkard's walk would.
    ///
    /// `gallery` is the whole of the difference between the two kinds of place.
    /// A chamber is a widening of the warren and the pillars will find little in
    /// it to plant; a gallery is opened around a solid block, is bigger, and is
    /// left off the pillars' list entirely, so it keeps its middle.
    fn carve_chamber(&mut self, site: Coord, gallery: bool) -> Vec<Coord> {
        let extent = if gallery {
            GALLERY_CELLS
        } else {
            CHAMBER_CELLS
        };
        let want = rand_inclusive(&mut self.rng, extent.0, extent.1);
        let mut body = vec![site];
        self.carve(site);
        if gallery {
            for dy in -GALLERY_CORE..=GALLERY_CORE {
                for dx in -GALLERY_CORE..=GALLERY_CORE {
                    let at = site + Coord::new(dx, dy);
                    if !body.contains(&at) && self.carve(at) {
                        body.push(at);
                    }
                }
            }
        }
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
        body
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
    pub(super) fn erode(&mut self, open_ground: &[bool]) {
        for _ in 0..EROSION_ROUNDS {
            let mut eaten = Vec::new();
            for y in 1..self.grid.height() - 1 {
                for x in 1..self.grid.width() - 1 {
                    let at = Coord::new(x, y);
                    // Nothing is nibbled out of open ground, and nothing beside
                    // a gate: an alcove there would hand the gate a second
                    // doorway, and the whole doorway tactic rests on it having
                    // exactly one.
                    if self.grid.walkable(at)
                        || self.marked(open_ground, at)
                        || self.touches_gate(at)
                    {
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
    pub(super) fn plant_pillars(&mut self, open_ground: &[bool]) {
        for _ in 0..PILLAR_ROUNDS {
            let mut open: Vec<Coord> = Vec::new();
            for y in 1..self.grid.height() - 1 {
                for x in 1..self.grid.width() - 1 {
                    let at = Coord::new(x, y);
                    if self.wide_open(at) && !self.marked(open_ground, at) {
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
                if self.wide_open(at) && !self.marked(open_ground, at) {
                    self.grid.set_props(at, false, false, false);
                }
            }
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
