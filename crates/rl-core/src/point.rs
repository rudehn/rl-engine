//! Integer coordinates and rectangles.
//!
//! `y` grows downward, matching the screen and every grid in the engine, so
//! "north" is negative `y`.

use std::ops::{Add, AddAssign, Sub, SubAssign};

use serde::{Deserialize, Serialize};

/// A cell coordinate.
///
/// Ordered row-major (`y` first, then `x`) so a sorted list of points reads
/// top to bottom, left to right. That order is what several generation
/// passes rely on to break ties the same way on every run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Point {
    /// Column.
    pub x: i32,
    /// Row.
    pub y: i32,
}

impl Point {
    /// The origin.
    pub const ZERO: Point = Point { x: 0, y: 0 };

    /// A point at `(x, y)`.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// The point offset by `(dx, dy)`.
    pub const fn offset(self, dx: i32, dy: i32) -> Self {
        Self { x: self.x + dx, y: self.y + dy }
    }

    /// `(x, y)` as a tuple.
    pub const fn tuple(self) -> (i32, i32) {
        (self.x, self.y)
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Point {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.y.cmp(&other.y).then(self.x.cmp(&other.x))
    }
}

impl From<(i32, i32)> for Point {
    fn from((x, y): (i32, i32)) -> Self {
        Self { x, y }
    }
}

impl From<Point> for (i32, i32) {
    fn from(p: Point) -> Self {
        (p.x, p.y)
    }
}

impl Add for Point {
    type Output = Point;
    fn add(self, rhs: Point) -> Point {
        Point::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, rhs: Point) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Point {
    type Output = Point;
    fn sub(self, rhs: Point) -> Point {
        Point::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Point {
    fn sub_assign(&mut self, rhs: Point) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

/// An axis-aligned rectangle of cells, half-open: `x..x + width`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    /// Left edge, inclusive.
    pub x: i32,
    /// Top edge, inclusive.
    pub y: i32,
    /// Width in cells.
    pub width: i32,
    /// Height in cells.
    pub height: i32,
}

impl Rect {
    /// A rectangle with its top-left corner at `(x, y)`.
    ///
    /// # Panics
    /// Panics if either dimension is negative.
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        assert!(width >= 0 && height >= 0, "rect dimensions must be non-negative");
        Self { x, y, width, height }
    }

    /// The rectangle spanning two corners, inclusive of both.
    pub fn from_corners(a: Point, b: Point) -> Self {
        let x = a.x.min(b.x);
        let y = a.y.min(b.y);
        Self { x, y, width: (a.x - b.x).abs() + 1, height: (a.y - b.y).abs() + 1 }
    }

    /// One past the right edge.
    pub const fn right(&self) -> i32 {
        self.x + self.width
    }

    /// One past the bottom edge.
    pub const fn bottom(&self) -> i32 {
        self.y + self.height
    }

    /// The top-left cell.
    pub const fn origin(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// The middle cell.
    pub const fn center(&self) -> Point {
        Point::new(self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Number of cells.
    pub const fn area(&self) -> i32 {
        self.width * self.height
    }

    /// Whether the rectangle covers no cells.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Whether `p` is inside, border included.
    pub const fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    /// Whether `p` is on the one-cell border ring.
    pub const fn is_border(&self, p: Point) -> bool {
        self.contains(p) && (p.x == self.x || p.y == self.y || p.x + 1 == self.right() || p.y + 1 == self.bottom())
    }

    /// Whether two rectangles share at least one cell.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.too_close(other, 0)
    }

    /// Whether two rectangles come within `margin` cells of each other.
    ///
    /// Rooms placed without a margin share walls and read as one blob; this
    /// is what keeps them separate.
    pub fn too_close(&self, other: &Rect, margin: i32) -> bool {
        let separated =
            self.right() + margin <= other.x || other.right() + margin <= self.x || self.bottom() + margin <= other.y || other.bottom() + margin <= self.y;
        !separated
    }

    /// The rectangle grown by `n` cells on every side (shrunk for negative `n`).
    ///
    /// Shrinking past zero yields an empty rectangle rather than a negative one.
    pub fn inflate(&self, n: i32) -> Rect {
        let width = (self.width + 2 * n).max(0);
        let height = (self.height + 2 * n).max(0);
        Rect { x: self.x - n, y: self.y - n, width, height }
    }

    /// The overlap of two rectangles, or `None` if they do not touch.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right > x && bottom > y { Some(Rect::new(x, y, right - x, bottom - y)) } else { None }
    }

    /// Every cell inside, row-major.
    pub fn cells(self) -> impl Iterator<Item = Point> {
        (self.y..self.bottom()).flat_map(move |y| (self.x..self.right()).map(move |x| Point::new(x, y)))
    }

    /// Every cell strictly inside the border ring, row-major.
    pub fn interior(self) -> impl Iterator<Item = Point> {
        self.inflate(-1).cells()
    }

    /// Every cell on the border ring, row-major.
    pub fn border(self) -> impl Iterator<Item = Point> {
        self.cells().filter(move |p| self.is_border(*p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_order_row_major() {
        let mut pts = vec![Point::new(2, 1), Point::new(0, 2), Point::new(5, 0), Point::new(1, 1)];
        pts.sort();
        assert_eq!(pts, vec![Point::new(5, 0), Point::new(1, 1), Point::new(2, 1), Point::new(0, 2)]);
    }

    #[test]
    fn point_arithmetic() {
        let a = Point::new(3, 4);
        let b = Point::new(-1, 2);
        assert_eq!(a + b, Point::new(2, 6));
        assert_eq!(a - b, Point::new(4, 2));
        assert_eq!(a.offset(1, -1), Point::new(4, 3));
    }

    #[test]
    fn a_rect_covers_the_cells_it_says_it_does() {
        let r = Rect::new(2, 3, 4, 2);
        assert_eq!(r.right(), 6);
        assert_eq!(r.bottom(), 5);
        assert!(r.contains(Point::new(2, 3)));
        assert!(r.contains(Point::new(5, 4)));
        assert!(!r.contains(Point::new(6, 4)));
        assert!(!r.contains(Point::new(2, 5)));
        assert_eq!(r.cells().count(), 8);
        assert_eq!(r.area(), 8);
    }

    #[test]
    fn from_corners_is_inclusive_and_order_free() {
        let a = Rect::from_corners(Point::new(4, 4), Point::new(1, 2));
        let b = Rect::from_corners(Point::new(1, 2), Point::new(4, 4));
        assert_eq!(a, b);
        assert_eq!(a, Rect::new(1, 2, 4, 3));
    }

    #[test]
    fn the_border_ring_is_the_border_only() {
        let r = Rect::new(0, 0, 4, 4);
        assert!(r.is_border(Point::new(0, 0)));
        assert!(r.is_border(Point::new(3, 2)));
        assert!(r.is_border(Point::new(2, 3)));
        assert!(!r.is_border(Point::new(1, 1)));
        assert_eq!(r.border().count(), 12);
        assert_eq!(r.interior().count(), 4);
    }

    #[test]
    fn rects_that_do_not_touch_are_not_too_close() {
        let a = Rect::new(0, 0, 3, 3);
        let far = Rect::new(6, 0, 3, 3);
        assert!(!a.too_close(&far, 1));
        assert!(!a.intersects(&far));
    }

    #[test]
    fn overlapping_rects_are_always_too_close() {
        let a = Rect::new(0, 0, 4, 4);
        let b = Rect::new(2, 2, 4, 4);
        assert!(a.too_close(&b, 0));
        assert!(a.intersects(&b));
        assert_eq!(a.intersection(&b), Some(Rect::new(2, 2, 2, 2)));
    }

    #[test]
    fn the_margin_pushes_rects_apart() {
        let a = Rect::new(0, 0, 3, 3);
        let adjacent = Rect::new(4, 0, 3, 3);
        assert!(!a.too_close(&adjacent, 1));
        assert!(a.too_close(&adjacent, 2));
        assert_eq!(a.too_close(&adjacent, 2), adjacent.too_close(&a, 2));
    }

    #[test]
    fn inflate_grows_and_shrinks_without_going_negative() {
        let r = Rect::new(5, 5, 4, 4);
        assert_eq!(r.inflate(1), Rect::new(4, 4, 6, 6));
        assert_eq!(r.inflate(-1), Rect::new(6, 6, 2, 2));
        assert!(r.inflate(-3).is_empty());
    }

    #[test]
    fn center_is_the_middle_cell() {
        assert_eq!(Rect::new(0, 0, 5, 5).center(), Point::new(2, 2));
        assert_eq!(Rect::new(10, 10, 4, 4).center(), Point::new(12, 12));
    }
}
