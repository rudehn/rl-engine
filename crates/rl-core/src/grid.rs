//! Dense row-major grids and the addressing contract they share.

use serde::{Deserialize, Serialize};

use crate::direction::Direction;
use crate::point::{Point, Rect};

/// Row-major 2D addressing.
///
/// Implementors expose `width` and `height`; everything else has a default.
/// Every map-shaped type in the engine implements this, so algorithms can be
/// written once against the trait and run on a terrain map, a scratch grid,
/// or a chunk alike.
pub trait Grid2D {
    /// Width in cells.
    fn width(&self) -> i32;

    /// Height in cells.
    fn height(&self) -> i32;

    /// Number of cells.
    fn len(&self) -> usize {
        (self.width() as usize) * (self.height() as usize)
    }

    /// Whether the grid has no cells.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The rectangle covering every cell.
    fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width(), self.height())
    }

    /// Flat row-major index for `(x, y)`. Inverse of [`idx_xy`](Self::idx_xy).
    ///
    /// Callers must check bounds first; this does no checking.
    fn xy_idx(&self, x: i32, y: i32) -> usize {
        (y as usize) * (self.width() as usize) + (x as usize)
    }

    /// Flat index for a point. Inverse of [`idx_point`](Self::idx_point).
    fn point_idx(&self, p: Point) -> usize {
        self.xy_idx(p.x, p.y)
    }

    /// `(x, y)` for a flat index. Inverse of [`xy_idx`](Self::xy_idx).
    fn idx_xy(&self, idx: usize) -> (i32, i32) {
        let w = self.width() as usize;
        ((idx % w) as i32, (idx / w) as i32)
    }

    /// The point for a flat index.
    fn idx_point(&self, idx: usize) -> Point {
        let (x, y) = self.idx_xy(idx);
        Point::new(x, y)
    }

    /// Whether `(x, y)` lies inside the grid.
    fn in_bounds_xy(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width() && y >= 0 && y < self.height()
    }

    /// Whether `p` lies inside the grid.
    fn in_bounds(&self, p: Point) -> bool {
        self.in_bounds_xy(p.x, p.y)
    }

    /// The flat index of `p` if it is inside the grid.
    fn checked_idx(&self, p: Point) -> Option<usize> {
        self.in_bounds(p).then(|| self.point_idx(p))
    }

    /// The in-bounds neighbours of `p`, clockwise from north.
    fn neighbours(&self, p: Point, steps: Steps) -> impl Iterator<Item = Point> {
        steps
            .directions()
            .iter()
            .map(move |d| p + d.offset())
            .filter(move |n| self.in_bounds(*n))
    }

    /// The in-bounds neighbours of `p` as flat indices, clockwise from north.
    fn neighbour_indices(&self, p: Point, steps: Steps) -> impl Iterator<Item = usize> {
        self.neighbours(p, steps).map(move |n| self.point_idx(n))
    }
}

/// Which neighbours count as adjacent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Steps {
    /// The four orthogonal neighbours.
    Four,
    /// All eight neighbours.
    Eight,
}

impl Steps {
    /// The directions this adjacency includes, clockwise from north.
    pub const fn directions(self) -> &'static [Direction] {
        match self {
            Steps::Four => &Direction::CARDINALS,
            Steps::Eight => &Direction::ALL,
        }
    }
}

/// A dense row-major grid of `T`.
///
/// The shared container for every layer of a map: terrain ids, heights,
/// distances, flags. Keeping each layer in its own `Grid` (structure of
/// arrays) keeps sweeps over one layer cache-friendly, which is why there is
/// no `Grid<Tile>` with everything in one struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid<T> {
    width: i32,
    height: i32,
    cells: Vec<T>,
}

impl<T: Clone> Grid<T> {
    /// A grid with every cell set to `fill`.
    ///
    /// # Panics
    /// Panics if either dimension is negative.
    pub fn filled(width: i32, height: i32, fill: T) -> Self {
        assert!(width >= 0 && height >= 0, "grid dimensions must be non-negative");
        Self {
            width,
            height,
            cells: vec![fill; (width as usize) * (height as usize)],
        }
    }

    /// Sets every cell to `value`.
    pub fn fill(&mut self, value: T) {
        for cell in &mut self.cells {
            *cell = value.clone();
        }
    }
}

impl<T: Default + Clone> Grid<T> {
    /// A grid with every cell set to `T::default()`.
    pub fn new(width: i32, height: i32) -> Self {
        Self::filled(width, height, T::default())
    }
}

impl<T> Grid<T> {
    /// A grid built by evaluating `f` at every coordinate, row-major.
    ///
    /// # Panics
    /// Panics if either dimension is negative.
    pub fn from_fn(width: i32, height: i32, mut f: impl FnMut(Point) -> T) -> Self {
        assert!(width >= 0 && height >= 0, "grid dimensions must be non-negative");
        let mut cells = Vec::with_capacity((width as usize) * (height as usize));
        for y in 0..height {
            for x in 0..width {
                cells.push(f(Point::new(x, y)));
            }
        }
        Self {
            width,
            height,
            cells,
        }
    }

    /// A grid over an existing row-major vector.
    ///
    /// # Panics
    /// Panics if `cells.len() != width * height`.
    pub fn from_vec(width: i32, height: i32, cells: Vec<T>) -> Self {
        assert_eq!(
            cells.len(),
            (width.max(0) as usize) * (height.max(0) as usize),
            "cell count must match the dimensions"
        );
        Self {
            width,
            height,
            cells,
        }
    }

    /// Borrows the cell at `p`, or `None` if out of bounds.
    pub fn get(&self, p: Point) -> Option<&T> {
        self.checked_idx(p).map(|i| &self.cells[i])
    }

    /// Mutably borrows the cell at `p`, or `None` if out of bounds.
    pub fn get_mut(&mut self, p: Point) -> Option<&mut T> {
        self.checked_idx(p).map(move |i| &mut self.cells[i])
    }

    /// Sets the cell at `p`. Out-of-bounds writes are ignored and reported.
    pub fn set(&mut self, p: Point, value: T) -> bool {
        match self.get_mut(p) {
            Some(cell) => {
                *cell = value;
                true
            }
            None => false,
        }
    }

    /// All cells, row-major.
    pub fn cells(&self) -> &[T] {
        &self.cells
    }

    /// All cells, row-major, mutably.
    pub fn cells_mut(&mut self) -> &mut [T] {
        &mut self.cells
    }

    /// Consumes the grid, returning its cells.
    pub fn into_cells(self) -> Vec<T> {
        self.cells
    }

    /// Iterates `(point, &cell)` row-major without a division per cell.
    pub fn iter(&self) -> impl Iterator<Item = (Point, &T)> {
        let width = self.width;
        self.cells.chunks(width.max(1) as usize).enumerate().flat_map(move |(y, row)| {
            row.iter()
                .enumerate()
                .map(move |(x, cell)| (Point::new(x as i32, y as i32), cell))
        })
    }

    /// Iterates `(point, &mut cell)` row-major.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Point, &mut T)> {
        let width = self.width;
        self.cells
            .chunks_mut(width.max(1) as usize)
            .enumerate()
            .flat_map(move |(y, row)| {
                row.iter_mut()
                    .enumerate()
                    .map(move |(x, cell)| (Point::new(x as i32, y as i32), cell))
            })
    }

    /// A new grid of the same shape with `f` applied to every cell.
    pub fn map<U>(&self, mut f: impl FnMut(&T) -> U) -> Grid<U> {
        Grid {
            width: self.width,
            height: self.height,
            cells: self.cells.iter().map(&mut f).collect(),
        }
    }

    /// The cell nearest `from` that `accept` approves of, searching outward
    /// in square rings. `from` itself is tested first.
    ///
    /// The ring order is load-bearing: it decides which of several equally
    /// near cells wins, and so where a plaza lands and how a whole town is
    /// laid out. Rows top to bottom, then left to right within a ring; it is
    /// pinned by a test.
    pub fn nearest_from(&self, from: Point, accept: impl Fn(&T) -> bool) -> Option<Point> {
        if let Some(cell) = self.get(from)
            && accept(cell)
        {
            return Some(from);
        }
        let limit = self.width.max(self.height);
        for radius in 1..limit {
            for dy in -radius..=radius {
                for dx in -radius..=radius {
                    if dx.abs() != radius && dy.abs() != radius {
                        continue;
                    }
                    let p = from.offset(dx, dy);
                    if let Some(cell) = self.get(p)
                        && accept(cell)
                    {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    /// Swaps the contents of two grids of the same shape, for double buffering.
    ///
    /// # Panics
    /// Panics if the shapes differ.
    pub fn swap_with(&mut self, other: &mut Grid<T>) {
        assert!(
            self.width == other.width && self.height == other.height,
            "cannot swap grids of different shapes"
        );
        std::mem::swap(&mut self.cells, &mut other.cells);
    }
}

impl<T> Grid2D for Grid<T> {
    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }
}

impl<T> std::ops::Index<Point> for Grid<T> {
    type Output = T;

    fn index(&self, p: Point) -> &T {
        debug_assert!(self.in_bounds(p), "{p:?} out of bounds");
        &self.cells[self.point_idx(p)]
    }
}

impl<T> std::ops::IndexMut<Point> for Grid<T> {
    fn index_mut(&mut self, p: Point) -> &mut T {
        debug_assert!(self.in_bounds(p), "{p:?} out of bounds");
        let i = self.point_idx(p);
        &mut self.cells[i]
    }
}

impl<T> std::ops::Index<usize> for Grid<T> {
    type Output = T;

    fn index(&self, idx: usize) -> &T {
        &self.cells[idx]
    }
}

impl<T> std::ops::IndexMut<usize> for Grid<T> {
    fn index_mut(&mut self, idx: usize) -> &mut T {
        &mut self.cells[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_fn_visits_every_cell_in_row_major_order() {
        let grid = Grid::from_fn(3, 2, |p| (p.x, p.y));
        assert_eq!(grid.cells(), &[(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)]);
        assert_eq!(grid[Point::new(2, 1)], (2, 1));
    }

    #[test]
    fn index_round_trips() {
        let grid: Grid<u8> = Grid::new(7, 4);
        for y in 0..4 {
            for x in 0..7 {
                let idx = grid.xy_idx(x, y);
                assert_eq!(grid.idx_xy(idx), (x, y));
                assert_eq!(grid.idx_point(idx), Point::new(x, y));
            }
        }
    }

    #[test]
    fn bounds_are_enforced_on_get() {
        let grid = Grid::filled(4, 4, 0u8);
        assert!(grid.get(Point::new(0, 0)).is_some());
        assert!(grid.get(Point::new(3, 3)).is_some());
        assert!(grid.get(Point::new(4, 3)).is_none());
        assert!(grid.get(Point::new(-1, 0)).is_none());
        assert!(!grid.in_bounds(Point::new(0, 4)));
    }

    #[test]
    fn set_reports_whether_it_landed() {
        let mut grid = Grid::filled(2, 2, 0u8);
        assert!(grid.set(Point::new(1, 1), 5));
        assert!(!grid.set(Point::new(2, 2), 5));
        assert_eq!(grid[Point::new(1, 1)], 5);
    }

    #[test]
    fn iter_reports_matching_coordinates_without_division() {
        let grid = Grid::from_fn(5, 3, |p| p.x * 10 + p.y);
        let mut count = 0;
        for (p, value) in grid.iter() {
            assert_eq!(*value, p.x * 10 + p.y);
            count += 1;
        }
        assert_eq!(count, 15);
        let empty: Grid<u8> = Grid::new(0, 0);
        assert_eq!(empty.iter().count(), 0);
        assert!(empty.is_empty());
    }

    #[test]
    fn iter_mut_writes_through() {
        let mut grid = Grid::filled(3, 3, 0);
        for (p, cell) in grid.iter_mut() {
            *cell = p.x + p.y;
        }
        assert_eq!(grid[Point::new(2, 2)], 4);
    }

    #[test]
    fn neighbours_are_clockwise_and_clipped() {
        let grid: Grid<u8> = Grid::new(3, 3);
        let n: Vec<Point> = grid.neighbours(Point::new(0, 0), Steps::Eight).collect();
        assert_eq!(n, vec![Point::new(1, 0), Point::new(1, 1), Point::new(0, 1)]);
        let n4: Vec<Point> = grid.neighbours(Point::new(1, 1), Steps::Four).collect();
        assert_eq!(
            n4,
            vec![Point::new(1, 0), Point::new(2, 1), Point::new(1, 2), Point::new(0, 1)]
        );
    }

    #[test]
    fn the_nearest_search_takes_the_starting_cell_when_it_qualifies() {
        let grid = Grid::filled(9, 9, 1u8);
        assert_eq!(grid.nearest_from(Point::new(4, 4), |v| *v == 1), Some(Point::new(4, 4)));
    }

    #[test]
    fn the_nearest_search_finds_nothing_in_a_grid_that_has_none() {
        let grid = Grid::filled(6, 4, 0u8);
        assert_eq!(grid.nearest_from(Point::new(2, 2), |v| *v == 9), None);
    }

    #[test]
    fn the_nearest_search_widens_a_ring_at_a_time() {
        let mut grid = Grid::filled(11, 11, 0u8);
        grid[Point::new(5, 3)] = 1;
        grid[Point::new(6, 5)] = 1;
        assert_eq!(grid.nearest_from(Point::new(5, 5), |v| *v == 1), Some(Point::new(6, 5)));
    }

    #[test]
    fn the_nearest_search_scans_a_ring_in_a_fixed_order() {
        // Rows top to bottom, then left to right. Changing this reshapes every
        // town ever generated, so it is pinned rather than left to chance.
        let mut grid = Grid::filled(11, 11, 0u8);
        for (x, y) in [(4, 4), (6, 4), (4, 6), (6, 6)] {
            grid[Point::new(x, y)] = 1;
        }
        assert_eq!(grid.nearest_from(Point::new(5, 5), |v| *v == 1), Some(Point::new(4, 4)));
    }

    #[test]
    fn the_nearest_search_reaches_the_far_corner() {
        let mut grid = Grid::filled(12, 5, 0u8);
        grid[Point::new(11, 4)] = 1;
        assert_eq!(grid.nearest_from(Point::new(0, 0), |v| *v == 1), Some(Point::new(11, 4)));
    }

    #[test]
    fn map_and_swap_keep_shape() {
        let a = Grid::from_fn(4, 2, |p| p.x);
        let mut doubled = a.map(|v| v * 2);
        assert_eq!(doubled[Point::new(3, 1)], 6);
        let mut other = Grid::filled(4, 2, 0);
        doubled.swap_with(&mut other);
        assert_eq!(other[Point::new(3, 1)], 6);
        assert_eq!(doubled[Point::new(3, 1)], 0);
    }

    #[test]
    fn grid_round_trips_through_serde() {
        let grid = Grid::from_fn(3, 2, |p| p.x + p.y);
        let text = ron::to_string(&grid).unwrap();
        let back: Grid<i32> = ron::from_str(&text).unwrap();
        assert_eq!(grid, back);
    }
}
