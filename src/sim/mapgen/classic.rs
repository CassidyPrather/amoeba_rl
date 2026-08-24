//! The districts: the cavern GJ is played in, and the one every difficulty was
//! played in before the warren.
//!
//! It is the underside of a human city, and it is built the way one would be.
//! The rock is cut into districts; a chamber is dissolved out of the middle of
//! each; streets two cells wide are run between the chambers, with a couple of
//! loops so the map is a network rather than a tree. Nothing tightens it
//! afterwards — there is no erosion pass and no pillars — so what comes out is
//! big rooms joined by roads.
//!
//! That is a different game from the warren, which is why GJ keeps it. Two
//! cells of street is a street a caravan cannot cork: whatever is standing in
//! it, there is a lane past. The warren buys its geometry by taking that lane
//! away, and pays for it with
//! [`Sim::open_the_chokepoints`]; the
//! districts never take it in the first place. What they give up for it is
//! everything the warren's walls are for — a hall is where engulfing costs
//! four organelles and a shot crosses the whole room — so GJ is a cavern to be
//! crossed rather than one to be fought in, on the difficulty that already
//! asks for every gate.

use std::collections::VecDeque;

use super::Cavern;
use crate::sim::Sim;
use crate::sim::grid::{Coord, rand_inclusive};

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
pub(super) const CHAMBERS_MIN: usize = 3;
/// Streets carved beyond the spanning tree, so the map holds loops.
const STREET_LOOPS: usize = 2;

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

impl Sim {
    /// Cut the rock into districts, dissolve a chamber out of each, and run
    /// streets between them.
    ///
    /// `None` means the smoothing left too few chambers to be worth playing,
    /// which is the retry loop's cue to throw the map away.
    pub(super) fn carve_districts(&mut self) -> Option<Cavern> {
        let districts = self.districts();
        let mut hubs = Vec::new();
        for district in districts {
            if let Some(hub) = self.dissolve_chamber(district) {
                hubs.push(hub);
            }
        }
        if hubs.len() < CHAMBERS_MIN {
            return None;
        }
        self.carve_streets(&hubs);
        // Nothing here is tightened afterwards, so nothing here has to be
        // exempted from the tightening.
        let open_ground = vec![false; self.depth.len()];
        Some(Cavern { hubs, open_ground })
    }

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
                match self.cut_district(leaf) {
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
    fn cut_district(&mut self, r: Rect) -> Option<(Rect, Rect)> {
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
    fn dissolve_chamber(&mut self, district: Rect) -> Option<Coord> {
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
        self.carve_street_run(from, elbow);
        self.carve_street_run(elbow, to);
    }

    /// One straight leg, two cells wide.
    ///
    /// Two rather than one on purpose: a single-file corridor is a place a
    /// hundred-cell amoeba goes to get stuck, and every one of them is a turn
    /// spent shuffling rather than playing.
    fn carve_street_run(&mut self, from: Coord, to: Coord) {
        let step = Coord::new((to.x - from.x).signum(), (to.y - from.y).signum());
        let lane = Coord::new(step.y.abs(), step.x.abs());
        let steps = (to.x - from.x).abs().max((to.y - from.y).abs());
        for i in 0..=steps {
            let c = from + Coord::new(step.x * i, step.y * i);
            self.carve(c);
            self.carve(c + lane);
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
