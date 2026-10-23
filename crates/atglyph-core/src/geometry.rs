//! Pure grid geometry: distances, lines, discs, cones and squares.
//!
//! Everything takes plain [`Point`]s and returns plain values. Shapes are
//! iterators so callers that only want to test membership never allocate.

use crate::point::{Point, Rect};

/// `|dx| + |dy|`: movement cost on a 4-connected grid.
pub const fn manhattan(a: Point, b: Point) -> i32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

/// `max(|dx|, |dy|)`: king-move distance on an 8-connected grid.
pub fn chebyshev(a: Point, b: Point) -> i32 {
    (a.x - b.x).abs().max((a.y - b.y).abs())
}

/// Octile distance scaled by 1000: the admissible A* heuristic for a grid
/// where a diagonal step costs `sqrt(2)` of an orthogonal one.
///
/// Integer so two runs cannot disagree over a last-bit float compare.
pub fn octile_milli(a: Point, b: Point) -> u32 {
    let dx = (a.x - b.x).unsigned_abs();
    let dy = (a.y - b.y).unsigned_abs();
    let (long, short) = if dx > dy { (dx, dy) } else { (dy, dx) };
    // 1414 - 1000 = 414 is the extra a diagonal costs over an orthogonal.
    long * 1000 + short * 414
}

/// Squared Euclidean distance, for comparisons that need a true circle.
pub const fn euclidean_sq(a: Point, b: Point) -> i32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

/// Whether the two points are neighbours, diagonals included.
pub fn is_adjacent(a: Point, b: Point) -> bool {
    chebyshev(a, b) == 1
}

/// Whether `p` lies within Euclidean `radius` of `center`: a circle, not a
/// Chebyshev square.
pub const fn within_disc(center: Point, p: Point, radius: i32) -> bool {
    euclidean_sq(center, p) <= radius * radius
}

/// Clamps `p` into `bounds`.
pub fn clamp_to(p: Point, bounds: Rect) -> Point {
    Point::new(p.x.clamp(bounds.x, bounds.right() - 1), p.y.clamp(bounds.y, bounds.bottom() - 1))
}

/// The cells a straight line passes through from `a` to `b`, both ends
/// included, by integer Bresenham.
pub fn line(a: Point, b: Point) -> Line {
    let dx = (b.x - a.x).abs();
    let dy = -(b.y - a.y).abs();
    Line { x: a.x, y: a.y, end: b, dx, dy, sx: if a.x < b.x { 1 } else { -1 }, sy: if a.y < b.y { 1 } else { -1 }, err: dx + dy, done: false }
}

/// Iterator over a Bresenham line. See [`line()`].
#[derive(Debug, Clone)]
pub struct Line {
    x: i32,
    y: i32,
    end: Point,
    dx: i32,
    dy: i32,
    sx: i32,
    sy: i32,
    err: i32,
    done: bool,
}

impl Iterator for Line {
    type Item = Point;

    fn next(&mut self) -> Option<Point> {
        if self.done {
            return None;
        }
        let here = Point::new(self.x, self.y);
        if here == self.end {
            self.done = true;
            return Some(here);
        }
        let e2 = 2 * self.err;
        if e2 >= self.dy {
            self.err += self.dy;
            self.x += self.sx;
        }
        if e2 <= self.dx {
            self.err += self.dx;
            self.y += self.sy;
        }
        Some(here)
    }
}

/// Every cell within Chebyshev `radius` of `center`, row-major: a
/// `(2r + 1)` square. Radius 0 is the centre alone.
pub fn square(center: Point, radius: i32) -> impl Iterator<Item = Point> {
    Rect::new(center.x - radius, center.y - radius, 2 * radius + 1, 2 * radius + 1).cells()
}

/// Every cell within Euclidean `radius` of `center`, row-major.
pub fn disc(center: Point, radius: i32) -> impl Iterator<Item = Point> {
    square(center, radius).filter(move |p| within_disc(center, *p, radius))
}

/// The cells a directional cone covers, fanning out from `origin` toward
/// `aim` up to `length` cells (Chebyshev), excluding the origin.
///
/// A quadrant fan: the aim is reduced to one of eight directions, then the
/// 90 degree arc facing that way is covered. A cardinal aim is a widening
/// triangle, `2d + 1` wide at `d` cells ahead. A diagonal aim is the full
/// quadrant toward that corner. A degenerate aim or non-positive length
/// yields nothing. The result is pure shape; callers bound it by line of
/// sight.
pub fn cone(origin: Point, aim: Point, length: i32) -> impl Iterator<Item = Point> {
    let (sx, sy) = ((aim.x - origin.x).signum(), (aim.y - origin.y).signum());
    let live = length > 0 && (sx != 0 || sy != 0);
    square(origin, length.max(0)).filter(move |p| {
        if !live {
            return false;
        }
        let rx = p.x - origin.x;
        let ry = p.y - origin.y;
        if rx == 0 && ry == 0 {
            return false;
        }
        if sx != 0 && sy != 0 {
            rx * sx >= 0 && ry * sy >= 0
        } else if sx != 0 {
            let along = rx * sx;
            along >= 1 && ry.abs() <= along
        } else {
            let along = ry * sy;
            along >= 1 && rx.abs() <= along
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn p(x: i32, y: i32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn distances_agree_on_axis_and_diverge_off_it() {
        for d in 0..=10 {
            assert_eq!(manhattan(p(0, 0), p(d, 0)), chebyshev(p(0, 0), p(d, 0)));
        }
        assert_eq!(manhattan(p(0, 0), p(3, 4)), 7);
        assert_eq!(chebyshev(p(0, 0), p(3, 4)), 4);
        assert_eq!(euclidean_sq(p(0, 0), p(3, 4)), 25);
        assert_eq!(manhattan(p(-3, -2), p(4, 5)), 14);
    }

    #[test]
    fn octile_is_exact_on_pure_steps() {
        assert_eq!(octile_milli(p(0, 0), p(5, 0)), 5000);
        assert_eq!(octile_milli(p(0, 0), p(3, 3)), 3 * 1414);
        assert_eq!(octile_milli(p(0, 0), p(5, 2)), 5000 + 2 * 414);
        assert_eq!(octile_milli(p(2, 5), p(0, 0)), octile_milli(p(0, 0), p(2, 5)));
    }

    #[test]
    fn adjacency_is_chebyshev_one() {
        for d in [p(6, 5), p(4, 5), p(5, 6), p(5, 4), p(6, 6), p(4, 4)] {
            assert!(is_adjacent(p(5, 5), d));
        }
        assert!(!is_adjacent(p(5, 5), p(5, 5)));
        assert!(!is_adjacent(p(5, 5), p(7, 7)));
    }

    #[test]
    fn line_traces_endpoints_and_a_straight_run() {
        assert_eq!(line(p(2, 4), p(5, 4)).collect::<Vec<_>>(), vec![p(2, 4), p(3, 4), p(4, 4), p(5, 4)]);
        assert_eq!(line(p(0, 0), p(3, 3)).collect::<Vec<_>>(), vec![p(0, 0), p(1, 1), p(2, 2), p(3, 3)]);
        assert_eq!(line(p(7, 7), p(7, 7)).collect::<Vec<_>>(), vec![p(7, 7)]);
        let back: Vec<Point> = line(p(5, 4), p(2, 4)).collect();
        assert_eq!(back, vec![p(5, 4), p(4, 4), p(3, 4), p(2, 4)]);
    }

    #[test]
    fn line_steps_are_always_adjacent() {
        let pts: Vec<Point> = line(p(0, 0), p(7, 3)).collect();
        for w in pts.windows(2) {
            assert!(is_adjacent(w[0], w[1]));
        }
        assert_eq!(pts.first(), Some(&p(0, 0)));
        assert_eq!(pts.last(), Some(&p(7, 3)));
    }

    #[test]
    fn square_counts_and_includes_center() {
        for r in 0..=4 {
            let tiles: Vec<Point> = square(p(0, 0), r).collect();
            assert_eq!(tiles.len(), ((2 * r + 1) * (2 * r + 1)) as usize);
            assert!(tiles.contains(&p(0, 0)));
        }
        assert_eq!(square(p(10, 10), 0).collect::<Vec<_>>(), vec![p(10, 10)]);
    }

    #[test]
    fn disc_excludes_the_square_corners() {
        let tiles: BTreeSet<Point> = disc(p(5, 5), 2).collect();
        assert!(tiles.contains(&p(5, 5)));
        assert!(tiles.contains(&p(7, 5)));
        assert!(tiles.contains(&p(6, 6)));
        assert!(!tiles.contains(&p(7, 7)));
        assert!(!tiles.contains(&p(7, 6)));
        assert!(within_disc(p(5, 5), p(5, 3), 2));
    }

    #[test]
    fn cone_east_is_a_widening_triangle_that_excludes_the_origin() {
        let tiles: BTreeSet<Point> = cone(p(0, 0), p(1, 0), 2).collect();
        assert!(!tiles.contains(&p(0, 0)));
        for t in [p(1, -1), p(1, 0), p(1, 1)] {
            assert!(tiles.contains(&t), "expected {t:?}");
        }
        for t in [p(2, -2), p(2, -1), p(2, 0), p(2, 1), p(2, 2)] {
            assert!(tiles.contains(&t), "expected {t:?}");
        }
        assert!(!tiles.contains(&p(-1, 0)));
        assert!(!tiles.contains(&p(1, 2)));
        assert!(!tiles.contains(&p(3, 0)));
        assert_eq!(tiles.len(), 8);
    }

    #[test]
    fn cone_diagonal_covers_the_quadrant_toward_the_aim() {
        let tiles: BTreeSet<Point> = cone(p(0, 0), p(1, 1), 2).collect();
        for t in [p(1, 0), p(0, 1), p(1, 1), p(2, 0), p(0, 2), p(2, 2), p(2, 1)] {
            assert!(tiles.contains(&t), "expected {t:?}");
        }
        assert!(!tiles.contains(&p(-1, 0)));
        assert!(!tiles.contains(&p(0, -1)));
        assert!(!tiles.contains(&p(0, 0)));
    }

    #[test]
    fn cone_degenerate_aim_or_zero_length_is_empty() {
        assert_eq!(cone(p(4, 4), p(4, 4), 3).count(), 0);
        assert_eq!(cone(p(0, 0), p(1, 0), 0).count(), 0);
    }

    #[test]
    fn clamp_keeps_a_point_inside_the_bounds() {
        let b = Rect::new(0, 0, 80, 60);
        assert_eq!(clamp_to(p(40, 30), b), p(40, 30));
        assert_eq!(clamp_to(p(-1, 30), b), p(0, 30));
        assert_eq!(clamp_to(p(80, 60), b), p(79, 59));
        assert_eq!(clamp_to(p(-3, -7), b), p(0, 0));
    }
}
