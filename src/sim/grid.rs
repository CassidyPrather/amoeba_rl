//! The map grid, its field of view, and the geometry the rest of the sim is
//! built out of.
//!
//! Two details here are load-bearing and easy to lose in a port. Sight stops
//! each cast line at *taxicab* radius, so a field of view is a diamond and not
//! a circle — every awareness number in the game was balanced against that
//! shape. And the original's RNG was inclusive on both ends, so
//! [`rand_inclusive`] is the only way randomness may enter: `Next(3) == 0` is
//! one chance in four, not one in three.

use std::ops::{Add, Sub};

use fastrand::Rng;

/// An integer map coordinate.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, PartialOrd, Ord)]
pub struct Coord {
    /// Column, increasing rightwards.
    pub x: i32,
    /// Row, increasing downwards.
    pub y: i32,
}

impl Coord {
    /// A coordinate at `(x, y)`.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Manhattan distance. The game measures every radius this way.
    #[must_use]
    pub const fn taxi(self, other: Self) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    /// Whether the two coordinates share an edge.
    #[must_use]
    pub const fn adjacent_to(self, other: Self) -> bool {
        self.taxi(other) == 1
    }
}

impl Add for Coord {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Coord {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

/// The four directions anything ever moves in. There are no diagonals.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Dir {
    /// Toward row zero.
    Up,
    /// Away from row zero.
    Down,
    /// Toward column zero.
    Left,
    /// Away from column zero.
    Right,
}

impl Dir {
    /// The one-cell offset this direction moves by.
    #[must_use]
    pub const fn offset(self) -> Coord {
        match self {
            Self::Up => Coord::new(0, -1),
            Self::Down => Coord::new(0, 1),
            Self::Left => Coord::new(-1, 0),
            Self::Right => Coord::new(1, 0),
        }
    }
}

/// The four neighbour offsets, in the order the original produced them.
/// Several algorithms pick "the first walkable neighbour", so the order is
/// part of the behaviour.
const NEIGHBOURS: [Coord; 4] = [
    Coord::new(-1, 0),
    Coord::new(1, 0),
    Coord::new(0, -1),
    Coord::new(0, 1),
];

/// One map cell.
///
/// Walkability is mutable and doubles as occupancy: placing an actor clears
/// it. Transparency is set once by the generator and never touched again,
/// which is why actors never block line of sight.
#[allow(clippy::struct_excessive_bools)] // Four independent flags; packing them would only hide them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cell {
    /// Something can stand here.
    pub walkable: bool,
    /// Light passes through here.
    pub transparent: bool,
    /// The player has seen this cell at least once. Sticky.
    pub explored: bool,
    /// This cell is in the current field of view.
    pub in_fov: bool,
}

/// The map: a flat row-major grid of [`Cell`]s.
///
/// Anything outside the grid reads back as solid, opaque, unexplored rock, so
/// callers can probe past the edge without bounds dances.
#[derive(Clone, Debug)]
pub struct Grid {
    width: i32,
    height: i32,
    cells: Vec<Cell>,
}

impl Grid {
    /// A grid of solid, opaque, unexplored rock.
    #[must_use]
    pub fn new(width: i32, height: i32) -> Self {
        let len = (width.max(0) * height.max(0)).unsigned_abs() as usize;
        Self {
            width: width.max(0),
            height: height.max(0),
            cells: vec![Cell::default(); len],
        }
    }

    /// Number of columns.
    #[must_use]
    pub const fn width(&self) -> i32 {
        self.width
    }

    /// Number of rows.
    #[must_use]
    pub const fn height(&self) -> i32 {
        self.height
    }

    /// Whether a coordinate names a real cell.
    #[must_use]
    pub const fn in_bounds(&self, c: Coord) -> bool {
        c.x >= 0 && c.y >= 0 && c.x < self.width && c.y < self.height
    }

    #[allow(clippy::cast_sign_loss)] // Only called after `in_bounds`, so both terms are non-negative.
    const fn index(&self, c: Coord) -> usize {
        (c.y * self.width + c.x) as usize
    }

    /// The cell at `c`, or solid rock if `c` is off the map.
    #[must_use]
    pub fn cell(&self, c: Coord) -> Cell {
        if self.in_bounds(c) {
            self.cells[self.index(c)]
        } else {
            Cell::default()
        }
    }

    /// Whether something can stand at `c`.
    #[must_use]
    pub fn walkable(&self, c: Coord) -> bool {
        self.cell(c).walkable
    }

    /// Whether light passes through `c`.
    #[must_use]
    pub fn transparent(&self, c: Coord) -> bool {
        self.cell(c).transparent
    }

    /// Whether the player has ever seen `c`.
    #[must_use]
    pub fn explored(&self, c: Coord) -> bool {
        self.cell(c).explored
    }

    /// Whether `c` is lit by the current field of view.
    #[must_use]
    pub fn in_fov(&self, c: Coord) -> bool {
        self.cell(c).in_fov
    }

    /// Set all three persistent flags at once, the way the generator does.
    pub(crate) fn set_props(
        &mut self,
        c: Coord,
        transparent: bool,
        walkable: bool,
        explored: bool,
    ) {
        if self.in_bounds(c) {
            let i = self.index(c);
            self.cells[i].transparent = transparent;
            self.cells[i].walkable = walkable;
            self.cells[i].explored = explored;
        }
    }

    /// Occupancy: actors clear this and leave transparency alone.
    pub(crate) fn set_walkable(&mut self, c: Coord, walkable: bool) {
        if self.in_bounds(c) {
            let i = self.index(c);
            self.cells[i].walkable = walkable;
        }
    }

    pub(crate) fn set_explored(&mut self, c: Coord, explored: bool) {
        if self.in_bounds(c) {
            let i = self.index(c);
            self.cells[i].explored = explored;
        }
    }

    /// In-bounds orthogonal neighbours, in the original's left/right/up/down
    /// order.
    pub(crate) fn adjacent(&self, c: Coord) -> impl Iterator<Item = Coord> + '_ {
        NEIGHBOURS
            .into_iter()
            .map(move |d| c + d)
            .filter(|n| self.in_bounds(*n))
    }

    /// In-bounds orthogonal neighbours nothing is standing on.
    pub(crate) fn adjacent_walkable(&self, c: Coord) -> impl Iterator<Item = Coord> + '_ {
        self.adjacent(c).filter(|n| self.walkable(*n))
    }

    /// Every in-bounds cell within taxicab `radius` of `center`, including
    /// `center` itself.
    pub fn cells_in_diamond(&self, center: Coord, radius: i32) -> impl Iterator<Item = Coord> + '_ {
        (-radius..=radius)
            .flat_map(move |dy| {
                let span = radius - dy.abs();
                (-span..=span).map(move |dx| Coord::new(center.x + dx, center.y + dy))
            })
            .filter(|c| self.in_bounds(*c))
    }

    /// The cells on the border of the bounding square of side `2 * radius + 1`,
    /// clipped to the map. These are the field of view's cast targets.
    fn border_cells_in_square(&self, center: Coord, radius: i32) -> Vec<Coord> {
        let x_min = (center.x - radius).max(0);
        let x_max = (center.x + radius).min(self.width - 1);
        let y_min = (center.y - radius).max(0);
        let y_max = (center.y + radius).min(self.height - 1);
        let mut out = Vec::new();
        for x in x_min..=x_max {
            out.push(Coord::new(x, y_min));
            if y_max != y_min {
                out.push(Coord::new(x, y_max));
            }
        }
        for y in (y_min + 1)..y_max {
            out.push(Coord::new(x_min, y));
            if x_max != x_min {
                out.push(Coord::new(x_max, y));
            }
        }
        out
    }

    /// Bresenham line from `from` to `to`, both ends included.
    pub(crate) fn cells_along_line(from: Coord, to: Coord) -> Vec<Coord> {
        let dx = (to.x - from.x).abs();
        let dy = (to.y - from.y).abs();
        let sx = if from.x < to.x { 1 } else { -1 };
        let sy = if from.y < to.y { 1 } else { -1 };
        let mut err = dx - dy;
        let mut at = from;
        let mut out = Vec::new();
        loop {
            out.push(at);
            if at == to {
                return out;
            }
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                at.x += sx;
            }
            if e2 < dx {
                err += dx;
                at.y += sy;
            }
        }
    }

    /// Whether anything at `from` can see `to`.
    ///
    /// Every cell strictly between the two has to be transparent; the endpoints
    /// themselves do not, so somebody standing in a doorway is visible from
    /// both sides of it. Transparency is fixed by the generator and occupancy
    /// never touches it, so bodies do not block sight — which is what lets a
    /// hunter's shot pass through your own mass.
    ///
    /// The walk is the same Bresenham [`Grid::append_fov`] casts along, so what
    /// a human can see and what the player's field of view lights up agree
    /// about every wall.
    #[must_use]
    pub fn line_of_sight(&self, from: Coord, to: Coord) -> bool {
        let line = Self::cells_along_line(from, to);
        line.iter()
            .skip(1)
            .take(line.len().saturating_sub(2))
            .all(|c| self.transparent(*c))
    }

    /// Forget the current field of view.
    pub(crate) fn clear_fov(&mut self) {
        for cell in &mut self.cells {
            cell.in_fov = false;
        }
    }

    /// Add one light source's diamond to the current field of view.
    ///
    /// Lines are cast to every border cell of the bounding square and walked
    /// until they leave taxicab `radius` or hit something opaque; with
    /// `light_walls` the blocking cell is itself lit, and a quadrant pass then
    /// lights the wall cells that no line could reach but that visibly border
    /// lit floor. Without that pass, wall corners flicker.
    pub(crate) fn append_fov(&mut self, origin: Coord, radius: i32, light_walls: bool) {
        if !self.in_bounds(origin) || radius < 0 {
            return;
        }
        for border in self.border_cells_in_square(origin, radius) {
            for cell in Self::cells_along_line(origin, border) {
                if cell.taxi(origin) > radius {
                    break;
                }
                let transparent = self.transparent(cell);
                if transparent || light_walls {
                    let i = self.index(cell);
                    self.cells[i].in_fov = true;
                }
                if !transparent {
                    break;
                }
            }
        }
        if !light_walls {
            return;
        }
        let x_min = (origin.x - radius).max(0);
        let x_max = (origin.x + radius).min(self.width - 1);
        let y_min = (origin.y - radius).max(0);
        let y_max = (origin.y + radius).min(self.height - 1);
        for y in y_min..=y_max {
            for x in x_min..=x_max {
                if x != origin.x && y != origin.y {
                    self.post_process_quadrant(Coord::new(x, y), origin);
                }
            }
        }
    }

    /// Light a wall that borders lit floor on the side facing `origin`.
    fn post_process_quadrant(&mut self, c: Coord, origin: Coord) {
        if self.in_fov(c) || self.transparent(c) {
            return;
        }
        let toward_y = Coord::new(c.x, c.y + (origin.y - c.y).signum());
        let toward_x = Coord::new(c.x + (origin.x - c.x).signum(), c.y);
        let diagonal = Coord::new(toward_x.x, toward_y.y);
        let lit = |g: &Self, p: Coord| g.transparent(p) && g.in_fov(p);
        if lit(self, toward_y) || lit(self, toward_x) || lit(self, diagonal) {
            let i = self.index(c);
            self.cells[i].in_fov = true;
        }
    }

    /// Shortest 4-way path from `from` to `to`, both ends included.
    ///
    /// Intermediate cells must be walkable; the endpoints are forced passable
    /// the way the original's `QuickShortestPath` temporarily did, so a path
    /// can be found to and from an occupied cell.
    #[must_use]
    pub fn shortest_path(&self, from: Coord, to: Coord) -> Option<Vec<Coord>> {
        if !self.in_bounds(from) || !self.in_bounds(to) {
            return None;
        }
        if from == to {
            return Some(vec![from]);
        }
        let mut came_from = vec![usize::MAX; self.cells.len()];
        let start = self.index(from);
        came_from[start] = start;
        let mut frontier = std::collections::VecDeque::from([from]);
        while let Some(at) = frontier.pop_front() {
            for next in self.adjacent(at) {
                let i = self.index(next);
                if came_from[i] != usize::MAX || (!self.walkable(next) && next != to) {
                    continue;
                }
                came_from[i] = self.index(at);
                if next == to {
                    let mut path = vec![to];
                    let mut cursor = i;
                    while cursor != start {
                        cursor = came_from[cursor];
                        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                        path.push(Coord::new(
                            cursor as i32 % self.width,
                            cursor as i32 / self.width,
                        ));
                    }
                    path.reverse();
                    return Some(path);
                }
                frontier.push_back(next);
            }
        }
        None
    }
}

/// Uniform integer in `[min, max]`, **both ends included**.
///
/// The original ran on `RogueSharp`'s `IRandom`, whose `Next` is inclusive, so
/// every ported probability reads `Next(3) == 0` and means one in four. An
/// empty or inverted range collapses to `min` rather than panicking, because
/// several call sites pass `len - 1` for a list that may be empty.
pub(crate) fn rand_inclusive(rng: &mut Rng, min: i32, max: i32) -> i32 {
    if max <= min { min } else { rng.i32(min..=max) }
}

/// Uniform index into a list of `len` items, or `None` when it is empty.
///
/// This is the ported shape of `list[Rand.Next(0, list.Count - 1)]`.
pub(crate) fn rand_index(rng: &mut Rng, len: usize) -> Option<usize> {
    if len == 0 {
        None
    } else {
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        #[allow(clippy::cast_sign_loss)]
        Some(rand_inclusive(rng, 0, len as i32 - 1) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_grid(w: i32, h: i32) -> Grid {
        let mut g = Grid::new(w, h);
        for y in 0..h {
            for x in 0..w {
                g.set_props(Coord::new(x, y), true, true, false);
            }
        }
        g
    }

    #[test]
    fn rand_inclusive_covers_both_ends() {
        let mut rng = Rng::with_seed(7);
        let mut low = false;
        let mut high = false;
        for _ in 0..500 {
            let v = rand_inclusive(&mut rng, 3, 5);
            assert!((3..=5).contains(&v));
            low |= v == 3;
            high |= v == 5;
        }
        assert!(low && high, "both ends must be reachable");
    }

    #[test]
    fn rand_inclusive_collapses_empty_range() {
        let mut rng = Rng::with_seed(1);
        assert_eq!(rand_inclusive(&mut rng, 4, 4), 4);
        assert_eq!(rand_inclusive(&mut rng, 0, -1), 0);
        assert_eq!(rand_index(&mut rng, 0), None);
        assert_eq!(rand_index(&mut rng, 1), Some(0));
    }

    #[test]
    fn fov_is_a_diamond() {
        let mut g = open_grid(21, 21);
        let origin = Coord::new(10, 10);
        g.clear_fov();
        g.append_fov(origin, 4, true);
        for y in 0..21 {
            for x in 0..21 {
                let c = Coord::new(x, y);
                assert_eq!(
                    g.in_fov(c),
                    c.taxi(origin) <= 4,
                    "cell {c:?} at taxi {}",
                    c.taxi(origin)
                );
            }
        }
    }

    #[test]
    fn fov_radius_zero_lights_only_the_source() {
        let mut g = open_grid(9, 9);
        let origin = Coord::new(4, 4);
        g.clear_fov();
        g.append_fov(origin, 0, true);
        assert!(g.in_fov(origin));
        assert_eq!(g.cells.iter().filter(|c| c.in_fov).count(), 1);
    }

    #[test]
    fn walls_occlude_and_are_themselves_lit() {
        let mut g = open_grid(21, 21);
        // A vertical wall two cells to the right of the source.
        for y in 0..21 {
            g.set_props(Coord::new(12, y), false, false, false);
        }
        let origin = Coord::new(10, 10);
        g.clear_fov();
        g.append_fov(origin, 6, true);
        // lightWalls: the blocking cell itself is visible.
        assert!(g.in_fov(Coord::new(12, 10)));
        // Everything behind it is not.
        assert!(!g.in_fov(Coord::new(13, 10)));
        assert!(!g.in_fov(Coord::new(14, 10)));
        // The floor in front still is.
        assert!(g.in_fov(Coord::new(11, 10)));
    }

    #[test]
    fn light_walls_off_hides_the_blocking_cell() {
        let mut g = open_grid(21, 21);
        for y in 0..21 {
            g.set_props(Coord::new(12, y), false, false, false);
        }
        let origin = Coord::new(10, 10);
        g.clear_fov();
        g.append_fov(origin, 6, false);
        assert!(!g.in_fov(Coord::new(12, 10)));
        assert!(g.in_fov(Coord::new(11, 10)));
    }

    #[test]
    fn fov_appends_rather_than_replaces() {
        let mut g = open_grid(21, 21);
        g.clear_fov();
        g.append_fov(Coord::new(3, 3), 1, true);
        g.append_fov(Coord::new(17, 17), 1, true);
        assert!(g.in_fov(Coord::new(3, 3)));
        assert!(g.in_fov(Coord::new(17, 17)));
        assert!(!g.in_fov(Coord::new(10, 10)));
    }

    #[test]
    fn diamond_iterator_matches_taxi_distance() {
        let g = open_grid(11, 11);
        let center = Coord::new(5, 5);
        let cells: Vec<_> = g.cells_in_diamond(center, 2).collect();
        assert_eq!(cells.len(), 13);
        assert!(cells.iter().all(|c| c.taxi(center) <= 2));
    }

    #[test]
    fn diamond_iterator_clips_to_the_map() {
        let g = open_grid(5, 5);
        let cells: Vec<_> = g.cells_in_diamond(Coord::new(0, 0), 2).collect();
        assert!(cells.iter().all(|c| g.in_bounds(*c)));
        assert_eq!(cells.len(), 6);
    }

    #[test]
    fn adjacency_order_is_left_right_up_down() {
        let g = open_grid(5, 5);
        let got: Vec<_> = g.adjacent(Coord::new(2, 2)).collect();
        assert_eq!(
            got,
            vec![
                Coord::new(1, 2),
                Coord::new(3, 2),
                Coord::new(2, 1),
                Coord::new(2, 3),
            ]
        );
    }

    #[test]
    fn shortest_path_routes_around_a_wall() {
        let mut g = open_grid(9, 9);
        for y in 0..8 {
            g.set_props(Coord::new(4, y), false, false, false);
        }
        let path = g
            .shortest_path(Coord::new(1, 1), Coord::new(7, 1))
            .expect("a way around the wall exists");
        assert_eq!(path[0], Coord::new(1, 1));
        assert_eq!(*path.last().unwrap(), Coord::new(7, 1));
        assert!(path.windows(2).all(|w| w[0].adjacent_to(w[1])));
        // Down to row 8, across, and back up.
        assert_eq!(path.len(), 21);
    }

    #[test]
    fn shortest_path_reports_impossible_routes() {
        let mut g = open_grid(9, 9);
        for y in 0..9 {
            g.set_props(Coord::new(4, y), false, false, false);
        }
        assert!(
            g.shortest_path(Coord::new(1, 1), Coord::new(7, 1))
                .is_none()
        );
    }

    #[test]
    fn out_of_bounds_reads_as_rock() {
        let g = open_grid(4, 4);
        let outside = Coord::new(-1, 2);
        assert!(!g.walkable(outside));
        assert!(!g.transparent(outside));
        assert!(!g.explored(outside));
    }
}
