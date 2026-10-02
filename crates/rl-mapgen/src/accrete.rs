//! Growing a place piece by piece.
//!
//! A [`Shape`] draws a [`Piece`]: the floor of one area and the [sites
//! ](Piece::sites) where it may be joined to another.

use rand::{Rng, RngCore};
use rl_core::{Direction, Point, Rect};

/// How a piece is joined to the one it is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join {
    /// One cell of the door tile.
    Door,
    /// One cell of floor: a way through with nothing to open.
    Arch,
    /// Floor as wide as both pieces allow, up to this many cells, so a
    /// large piece opens onto its neighbour rather than hiding behind a
    /// doorway.
    Mouth(u8),
}

/// The floor of one area, relative to its own corner, and where it may be
/// joined.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// The body: every cell of floor, none negative, sorted by row then
    /// column.
    pub cells: Vec<Point>,
    /// Each cell outside the body that touches exactly one body cell
    /// orthogonally, with the direction that leads out of the body through
    /// it. A cell touching two is in a concave corner, and a door there
    /// would open onto the piece's own wall.
    pub sites: Vec<(Point, Direction)>,
}

impl Piece {
    /// The piece whose body is `cells`, with every site worked out.
    pub fn from_cells(mut cells: Vec<Point>) -> Self {
        cells.sort_by_key(|p| (p.y, p.x));
        cells.dedup();
        let has = |p: Point| cells.binary_search_by_key(&(p.y, p.x), |c| (c.y, c.x)).is_ok();
        let mut sites = Vec::new();
        for &c in &cells {
            for d in Direction::CARDINALS {
                let p = c + d.offset();
                if !has(p) && Direction::CARDINALS.iter().filter(|n| has(p + n.offset())).count() == 1 {
                    sites.push((p, d));
                }
            }
        }
        Self { cells, sites }
    }
}

/// The smallest rectangle holding every one of `cells`; empty for none.
pub(crate) fn bounds_of(cells: &[Point]) -> Rect {
    let Some(first) = cells.first() else {
        return Rect::new(0, 0, 0, 0);
    };
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
    for c in cells {
        x0 = x0.min(c.x);
        y0 = y0.min(c.y);
        x1 = x1.max(c.x);
        y1 = y1.max(c.y);
    }
    Rect::new(x0, y0, x1 - x0 + 1, y1 - y0 + 1)
}

/// The largest rectangle made only of `cells`, which all lie in `bounds`.
/// It is where a stamp is centred, so an area that is not a rectangle
/// still has one to offer.
pub(crate) fn largest_rect(cells: &[Point], bounds: Rect) -> Rect {
    let (w, h) = (bounds.width as usize, bounds.height as usize);
    let mut filled = vec![false; w * h];
    for c in cells {
        filled[(c.y - bounds.y) as usize * w + (c.x - bounds.x) as usize] = true;
    }
    let mut best = Rect::new(bounds.x, bounds.y, 0, 0);
    for top in 0..h {
        // Whether each column is floor on every row from `top` to `y`.
        let mut column = vec![true; w];
        for y in top..h {
            for (x, all) in column.iter_mut().enumerate() {
                *all &= filled[y * w + x];
            }
            let mut x = 0;
            while x < w {
                if !column[x] {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < w && column[x] {
                    x += 1;
                }
                let r = Rect::new(bounds.x + start as i32, bounds.y + top as i32, (x - start) as i32, (y - top + 1) as i32);
                if r.area() > best.area() {
                    best = r;
                }
            }
        }
    }
    best
}

/// Draws one piece. A trait and not a list, so a game draws a piece the
/// engine never thought of.
pub trait Shape: Send + Sync {
    /// The shape's name, which an area it drew carries. The engine reads
    /// no meaning into it.
    fn name(&self) -> &str;

    /// How a piece of this shape is joined to what it is attached to.
    fn join(&self) -> Join;

    /// Whether a piece of this shape is somewhere a prefab may be stamped.
    /// A passage is not: it emits no `Room`, so nothing furnishes it.
    fn is_room(&self) -> bool {
        true
    }

    /// One piece, from the pass's own stream.
    fn draw(&self, rng: &mut dyn RngCore) -> Piece;
}

/// A rectangle, each side drawn between a smallest and a largest.
#[derive(Debug, Clone)]
pub struct Rectangle {
    /// The shape's name.
    pub name: String,
    /// The width, smallest and largest, in cells of floor.
    pub w: (i32, i32),
    /// The height, smallest and largest.
    pub h: (i32, i32),
    /// How it is joined.
    pub join: Join,
}

impl Shape for Rectangle {
    fn name(&self) -> &str {
        &self.name
    }
    fn join(&self) -> Join {
        self.join
    }
    fn draw(&self, rng: &mut dyn RngCore) -> Piece {
        let w = rng.random_range(self.w.0..=self.w.1);
        let h = rng.random_range(self.h.0..=self.h.1);
        Piece::from_cells(Rect::new(0, 0, w, h).cells().collect())
    }
}

/// Two bars overlapped on their centers, each `long` by `short`, one lying
/// and one standing.
#[derive(Debug, Clone)]
pub struct Cross {
    /// The shape's name.
    pub name: String,
    /// A bar's length, smallest and largest.
    pub long: (i32, i32),
    /// A bar's thickness, smallest and largest; less than the length, or
    /// the cross is a rectangle.
    pub short: (i32, i32),
    /// How it is joined.
    pub join: Join,
}

impl Shape for Cross {
    fn name(&self) -> &str {
        &self.name
    }
    fn join(&self) -> Join {
        self.join
    }
    fn draw(&self, rng: &mut dyn RngCore) -> Piece {
        let (lying, standing) = (rng.random_range(self.long.0..=self.long.1), rng.random_range(self.long.0..=self.long.1));
        let (thick_l, thick_s) = (rng.random_range(self.short.0..=self.short.1), rng.random_range(self.short.0..=self.short.1));
        let mut cells: Vec<Point> = Rect::new(0, (standing - thick_l) / 2, lying, thick_l).cells().collect();
        cells.extend(Rect::new((lying - thick_s) / 2, 0, thick_s, standing).cells());
        Piece::from_cells(cells)
    }
}

/// A run of floor a few cells wide, straight or with one bend.
#[derive(Debug, Clone)]
pub struct Passage {
    /// The shape's name.
    pub name: String,
    /// The width, smallest and largest.
    pub width: (i32, i32),
    /// The length of each leg, smallest and largest.
    pub length: (i32, i32),
    /// The chance in a hundred of a second leg at a right angle.
    pub bend_pct: u32,
    /// How it is joined.
    pub join: Join,
}

impl Shape for Passage {
    fn name(&self) -> &str {
        &self.name
    }
    fn join(&self) -> Join {
        self.join
    }
    fn is_room(&self) -> bool {
        false
    }
    fn draw(&self, rng: &mut dyn RngCore) -> Piece {
        let width = rng.random_range(self.width.0..=self.width.1);
        let first = rng.random_range(self.length.0..=self.length.1).max(width);
        let mut cells: Vec<Point> = Rect::new(0, 0, first, width).cells().collect();
        if rng.random_range(0..100) < self.bend_pct {
            let second = rng.random_range(self.length.0..=self.length.1).max(width);
            cells.extend(Rect::new(first - width, 0, width, second).cells());
        }
        // Lying or standing, so a passage runs either way off a wall.
        if rng.random_bool(0.5) {
            for c in &mut cells {
                *c = Point::new(c.y, c.x);
            }
        }
        Piece::from_cells(cells)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use rand::rngs::StdRng;

    fn stream(s: u64) -> StdRng {
        rl_core::RunSeed(s).rng(rl_core::SeedDomain::new(b"test"), 0)
    }

    #[test]
    fn a_rectangle_has_a_site_outside_every_edge_cell_and_none_at_a_corner() {
        let piece = Piece::from_cells(Rect::new(0, 0, 3, 2).cells().collect());
        assert_eq!(piece.cells.len(), 6);
        assert_eq!(piece.sites.len(), 10, "three above, three below, two each side");
        assert!(piece.sites.contains(&(Point::new(1, -1), Direction::North)));
        assert!(piece.sites.contains(&(Point::new(3, 0), Direction::East)));
        assert!(!piece.sites.iter().any(|(p, _)| *p == Point::new(-1, -1)), "a corner touches no body cell orthogonally");
    }

    #[test]
    fn a_cell_in_a_concave_corner_is_not_a_site() {
        // An L: the cell at (1, 0) touches the body on two sides.
        let piece = Piece::from_cells(vec![Point::new(0, 0), Point::new(0, 1), Point::new(1, 1)]);
        assert!(!piece.sites.iter().any(|(p, _)| *p == Point::new(1, 0)));
    }

    #[test]
    fn every_shape_draws_within_its_ranges_over_a_span_of_seeds() {
        for s in 0..200 {
            let mut rng = stream(s);
            let r = Rectangle { name: "r".into(), w: (4, 9), h: (3, 6), join: Join::Door }.draw(&mut rng);
            let b = bounds_of(&r.cells);
            assert!((4..=9).contains(&b.width) && (3..=6).contains(&b.height), "seed {s}: {b:?}");
            assert_eq!(r.cells.len() as i32, b.area(), "a rectangle fills its bounds");

            let c = Cross { name: "c".into(), long: (9, 13), short: (3, 5), join: Join::Door }.draw(&mut rng);
            let b = bounds_of(&c.cells);
            assert!((9..=13).contains(&b.width) && (9..=13).contains(&b.height), "seed {s}: {b:?}");
            assert!((c.cells.len() as i32) < b.area(), "a cross leaves its corners solid");

            let p = Passage { name: "p".into(), width: (1, 2), length: (5, 12), bend_pct: 50, join: Join::Arch }.draw(&mut rng);
            assert!(!p.cells.is_empty(), "seed {s}");
            assert!(p.cells.iter().all(|c| c.x >= 0 && c.y >= 0), "seed {s}: a piece's cells are never negative");
        }
    }

    #[test]
    fn a_passage_is_not_a_room_and_the_others_are() {
        assert!(Rectangle { name: "r".into(), w: (4, 4), h: (4, 4), join: Join::Door }.is_room());
        assert!(Cross { name: "c".into(), long: (9, 9), short: (3, 3), join: Join::Door }.is_room());
        assert!(!Passage { name: "p".into(), width: (1, 1), length: (5, 5), bend_pct: 0, join: Join::Arch }.is_room());
    }

    #[test]
    fn the_largest_rectangle_inside_a_cross_is_one_of_its_bars() {
        // A 7x3 bar across a 3x7 bar.
        let mut cells: Vec<Point> = Rect::new(0, 2, 7, 3).cells().collect();
        cells.extend(Rect::new(2, 0, 3, 7).cells());
        let piece = Piece::from_cells(cells);
        let inner = largest_rect(&piece.cells, bounds_of(&piece.cells));
        assert_eq!(inner.area(), 21);
    }
}
