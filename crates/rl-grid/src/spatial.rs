//! Who is standing where.
//!
//! Sparse and ordered: a map keyed by position, so it costs nothing for the
//! millions of empty cells of a large world and iterates the same way every
//! run. Occupancy is deliberately not part of the terrain, so mutating it
//! every turn does not mark the terrain changed.

use std::collections::BTreeMap;

use rl_core::{Point, Rect, geometry};

/// Entities indexed by cell.
#[derive(Debug, Clone)]
pub struct SpatialGrid<E> {
    cells: BTreeMap<Point, Vec<E>>,
    count: usize,
}

impl<E> Default for SpatialGrid<E> {
    fn default() -> Self {
        Self { cells: BTreeMap::new(), count: 0 }
    }
}

impl<E: Copy + Eq> SpatialGrid<E> {
    /// An empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `e` at `p`. An entity may be recorded at several cells only
    /// if the caller means it; nothing here prevents it.
    pub fn insert(&mut self, p: Point, e: E) {
        self.cells.entry(p).or_default().push(e);
        self.count += 1;
    }

    /// Forgets `e` at `p`. Returns whether it was there.
    pub fn remove(&mut self, p: Point, e: E) -> bool {
        let Some(list) = self.cells.get_mut(&p) else { return false };
        let Some(i) = list.iter().position(|x| *x == e) else { return false };
        list.remove(i);
        if list.is_empty() {
            self.cells.remove(&p);
        }
        self.count -= 1;
        true
    }

    /// Moves `e` from one cell to another. Returns whether it was at `from`.
    pub fn relocate(&mut self, e: E, from: Point, to: Point) -> bool {
        if !self.remove(from, e) {
            return false;
        }
        self.insert(to, e);
        true
    }

    /// Everything at `p`, in insertion order.
    pub fn at(&self, p: Point) -> &[E] {
        self.cells.get(&p).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Whether anything is at `p`.
    pub fn is_occupied(&self, p: Point) -> bool {
        self.cells.contains_key(&p)
    }

    /// The first entity at `p`.
    pub fn first_at(&self, p: Point) -> Option<E> {
        self.at(p).first().copied()
    }

    /// Everything inside `rect`, row-major.
    pub fn in_rect(&self, rect: Rect) -> impl Iterator<Item = (Point, E)> + '_ {
        (rect.y..rect.bottom())
            .flat_map(move |y| self.cells.range(Point::new(rect.x, y)..Point::new(rect.right(), y)).flat_map(|(p, list)| list.iter().map(move |e| (*p, *e))))
    }

    /// Everything within Chebyshev `radius` of `center`, row-major.
    pub fn in_square(&self, center: Point, radius: i32) -> impl Iterator<Item = (Point, E)> + '_ {
        self.in_rect(Rect::new(center.x - radius, center.y - radius, 2 * radius + 1, 2 * radius + 1))
    }

    /// Everything within Euclidean `radius` of `center`, row-major.
    pub fn in_disc(&self, center: Point, radius: i32) -> impl Iterator<Item = (Point, E)> + '_ {
        self.in_square(center, radius).filter(move |(p, _)| geometry::within_disc(center, *p, radius))
    }

    /// Every entity with its cell, row-major.
    pub fn iter(&self) -> impl Iterator<Item = (Point, E)> + '_ {
        self.cells.iter().flat_map(|(p, list)| list.iter().map(move |e| (*p, *e)))
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether nothing is recorded.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Forgets everything.
    pub fn clear(&mut self) {
        self.cells.clear();
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: i32, y: i32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn insert_remove_relocate() {
        let mut g: SpatialGrid<u32> = SpatialGrid::new();
        g.insert(p(1, 1), 7);
        g.insert(p(1, 1), 8);
        assert_eq!(g.at(p(1, 1)), &[7, 8]);
        assert!(g.is_occupied(p(1, 1)));
        assert!(g.remove(p(1, 1), 7));
        assert!(!g.remove(p(1, 1), 7));
        assert!(g.relocate(8, p(1, 1), p(2, 2)));
        assert!(!g.is_occupied(p(1, 1)));
        assert_eq!(g.first_at(p(2, 2)), Some(8));
        assert_eq!(g.len(), 1);
        g.clear();
        assert!(g.is_empty());
    }

    #[test]
    fn range_queries_are_row_major_and_bounded() {
        let mut g: SpatialGrid<u32> = SpatialGrid::new();
        for (i, pt) in [p(5, 5), p(0, 0), p(3, 7), p(7, 3), p(9, 9), p(5, 6)].into_iter().enumerate() {
            g.insert(pt, i as u32);
        }
        let square: Vec<Point> = g.in_square(p(5, 5), 2).map(|(pt, _)| pt).collect();
        assert_eq!(square, vec![p(7, 3), p(5, 5), p(5, 6), p(3, 7)]);
        let disc: Vec<Point> = g.in_disc(p(5, 5), 2).map(|(pt, _)| pt).collect();
        assert_eq!(disc, vec![p(5, 5), p(5, 6)]);
        assert_eq!(g.in_rect(Rect::new(0, 0, 1, 1)).count(), 1);
        assert_eq!(g.iter().count(), 6);
    }
}
