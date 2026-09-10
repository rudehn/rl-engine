//! Field of view by symmetric shadowcasting.
//!
//! Albert Ford's formulation: each quadrant is scanned row by row, tracking
//! the slopes that bound the visible arc, and a floor tile is revealed only
//! when its centre lies inside the arc. That last rule is what makes the
//! result symmetric: if A can see B, B can see A, for any two floor tiles.
//! Walls are revealed more generously so rooms read as solid.
//!
//! Output goes into a [`BitGrid`] the caller owns and reuses, so a viewshed
//! recompute allocates nothing.

use rl_core::{Grid2D, Point, geometry};

use crate::bitgrid::BitGrid;
use crate::terrain::OpacitySource;

/// Fills `out` with every cell visible from `origin` within Euclidean
/// `range`. `out` is cleared first and must match the source's shape.
///
/// The origin is always visible. A range of 0 sees the origin alone.
pub fn compute(source: &impl OpacitySource, origin: Point, range: i32, out: &mut BitGrid) {
    debug_assert_eq!((out.width(), out.height()), (source.width(), source.height()));
    out.clear();
    if !source.in_bounds(origin) {
        return;
    }
    out.insert(origin);
    if range <= 0 {
        return;
    }
    let mut scanner = Scanner {
        source,
        out,
        origin,
        range,
        quadrant: Quadrant::North,
    };
    for q in [Quadrant::North, Quadrant::East, Quadrant::South, Quadrant::West] {
        scanner.quadrant = q;
        scanner.scan(Row {
            depth: 1,
            start: Slope { num: -1, den: 1 },
            end: Slope { num: 1, den: 1 },
        });
    }
}

/// Whether `target` is visible from `origin`: a full scan, then a lookup.
/// For one query it is cheaper to call [`compute`] once and test many.
pub fn can_see(source: &impl OpacitySource, origin: Point, target: Point, range: i32) -> bool {
    let mut out = BitGrid::new(source.width(), source.height());
    compute(source, origin, range, &mut out);
    out.contains(target)
}

#[derive(Clone, Copy)]
enum Quadrant {
    North,
    East,
    South,
    West,
}

/// A rational slope, `num / den` with `den > 0`.
#[derive(Clone, Copy)]
struct Slope {
    num: i64,
    den: i64,
}

#[derive(Clone, Copy)]
struct Row {
    depth: i64,
    start: Slope,
    end: Slope,
}

impl Row {
    fn next(self) -> Row {
        Row {
            depth: self.depth + 1,
            ..self
        }
    }

    /// Columns whose centre lies in the arc: `round_ties_up(depth * start)`
    /// to `round_ties_down(depth * end)`.
    fn cols(&self) -> std::ops::RangeInclusive<i64> {
        let min = floor_div(2 * self.depth * self.start.num + self.start.den, 2 * self.start.den);
        let max = ceil_div(2 * self.depth * self.end.num - self.end.den, 2 * self.end.den);
        min..=max
    }

    fn is_symmetric(&self, col: i64) -> bool {
        col * self.start.den >= self.depth * self.start.num && col * self.end.den <= self.depth * self.end.num
    }
}

fn floor_div(a: i64, b: i64) -> i64 {
    let d = a / b;
    if (a % b != 0) && ((a < 0) != (b < 0)) { d - 1 } else { d }
}

fn ceil_div(a: i64, b: i64) -> i64 {
    -floor_div(-a, b)
}

/// The slope through the near edge of the tile at `(depth, col)`.
fn slope(depth: i64, col: i64) -> Slope {
    Slope {
        num: 2 * col - 1,
        den: 2 * depth,
    }
}

struct Scanner<'a, S: OpacitySource> {
    source: &'a S,
    out: &'a mut BitGrid,
    origin: Point,
    range: i32,
    quadrant: Quadrant,
}

impl<S: OpacitySource> Scanner<'_, S> {
    fn transform(&self, depth: i64, col: i64) -> Point {
        let (d, c) = (depth as i32, col as i32);
        match self.quadrant {
            Quadrant::North => Point::new(self.origin.x + c, self.origin.y - d),
            Quadrant::South => Point::new(self.origin.x + c, self.origin.y + d),
            Quadrant::East => Point::new(self.origin.x + d, self.origin.y + c),
            Quadrant::West => Point::new(self.origin.x - d, self.origin.y + c),
        }
    }

    fn is_wall(&self, depth: i64, col: i64) -> bool {
        self.source.is_opaque(self.transform(depth, col))
    }

    fn reveal(&mut self, depth: i64, col: i64) {
        let p = self.transform(depth, col);
        if geometry::within_disc(self.origin, p, self.range) {
            self.out.insert(p);
        }
    }

    fn scan(&mut self, mut row: Row) {
        if row.depth > self.range as i64 {
            return;
        }
        let mut prev_wall: Option<bool> = None;
        for col in row.cols() {
            let wall = self.is_wall(row.depth, col);
            if wall || row.is_symmetric(col) {
                self.reveal(row.depth, col);
            }
            if prev_wall == Some(true) && !wall {
                row.start = slope(row.depth, col);
            }
            if prev_wall == Some(false) && wall {
                let mut next = row.next();
                next.end = slope(row.depth, col);
                self.scan(next);
            }
            prev_wall = Some(wall);
        }
        if prev_wall == Some(false) {
            self.scan(row.next());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::fixtures::parse;
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use rl_core::Grid2D;

    fn p(x: i32, y: i32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn an_open_room_is_fully_visible_within_range() {
        let (t, r) = parse(&["#######", "#.....#", "#.....#", "#.....#", "#######"]);
        let view = t.view(&r);
        let mut out = BitGrid::new(view.width(), view.height());
        compute(&view, p(3, 2), 10, &mut out);
        for (pt, _) in t.iter() {
            assert!(out.contains(pt), "{pt:?} should be visible");
        }
    }

    #[test]
    fn a_pillar_casts_a_shadow() {
        let (t, r) = parse(&[".........", ".........", "....#....", ".........", "........."]);
        let view = t.view(&r);
        let mut out = BitGrid::new(view.width(), view.height());
        compute(&view, p(0, 2), 20, &mut out);
        assert!(out.contains(p(4, 2)), "the pillar itself is seen");
        assert!(!out.contains(p(5, 2)), "directly behind the pillar is hidden");
        assert!(!out.contains(p(8, 2)));
        assert!(out.contains(p(8, 0)), "well off the shadow line is visible");
    }

    #[test]
    fn range_is_a_disc() {
        let rows = vec![".".repeat(21); 21];
        let (t, r) = parse(&rows.iter().map(String::as_str).collect::<Vec<_>>());
        let view = t.view(&r);
        let mut out = BitGrid::new(21, 21);
        compute(&view, p(10, 10), 5, &mut out);
        assert!(out.contains(p(15, 10)));
        assert!(out.contains(p(13, 14)), "3,4,5 triangle is on the circle");
        assert!(!out.contains(p(14, 14)), "the square corner is outside the disc");
        assert!(!out.contains(p(16, 10)));
        compute(&view, p(10, 10), 0, &mut out);
        assert_eq!(out.count(), 1);
    }

    #[test]
    fn walls_stop_sight_and_origin_out_of_bounds_sees_nothing() {
        let (t, r) = parse(&["....#....", "....#....", "....#...."]);
        let view = t.view(&r);
        let mut out = BitGrid::new(view.width(), view.height());
        compute(&view, p(1, 1), 20, &mut out);
        assert!(out.contains(p(4, 1)), "the wall face is seen");
        assert!(!out.contains(p(5, 1)));
        assert!(!out.contains(p(8, 2)));
        compute(&view, p(-1, 1), 20, &mut out);
        assert!(out.is_clear());
    }

    /// The property that names the algorithm: between two floor tiles,
    /// visibility is mutual. Checked over random maps and origins.
    #[test]
    fn visibility_between_floor_tiles_is_symmetric() {
        let registry = crate::tile::TileRegistry::standard();
        let (wall, floor) = (registry.expect("wall"), registry.expect("floor"));
        for seed in 0..12u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let terrain = crate::terrain::Terrain::from_fn(24, 18, |_| {
                if rng.random_range(0..100) < 30 { wall } else { floor }
            });
            let view = terrain.view(&registry);
            let mut from_a = BitGrid::new(24, 18);
            let mut from_b = BitGrid::new(24, 18);
            for _ in 0..6 {
                let a = p(rng.random_range(0..24), rng.random_range(0..18));
                if view.is_opaque(a) {
                    continue;
                }
                compute(&view, a, 9, &mut from_a);
                for b in from_a.iter() {
                    if view.is_opaque(b) {
                        continue;
                    }
                    compute(&view, b, 9, &mut from_b);
                    assert!(from_b.contains(a), "seed {seed}: {a:?} sees {b:?} but not back");
                }
            }
        }
    }

    #[test]
    fn can_see_matches_compute() {
        let (t, r) = parse(&["...#...", ".......", "...#..."]);
        let view = t.view(&r);
        assert!(can_see(&view, p(0, 1), p(6, 1), 10));
        assert!(!can_see(&view, p(0, 0), p(6, 0), 10));
    }
}
