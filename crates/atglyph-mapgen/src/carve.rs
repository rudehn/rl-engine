//! Carving open ground out of solid rock a cell at a time: the drunkard's
//! walk and diffusion-limited aggregation.
//!
//! Both open floor only beside floor already open, from one cell to start,
//! so what they carve is one piece by its making, walkable end to end with
//! no pass after them to join it up; and both stop at the share of the map
//! they were asked for, where a cellular automaton's open share swings
//! with the seed. What they make differs.
//!
//! [`DrunkardsWalk`] sends walkers out from the open ground, each a long
//! random walk opening a brush of floor round every step: a broad, blobby
//! hall with ragged walls and islands of rock standing in it, more open the
//! wider the brush and the longer each walk.
//!
//! [`Aggregate`] lets particles wander in from the rock round what is open
//! until one touches it, and opens the cell it stopped on: what grows is
//! branching, root and coral, all dead ends and no loops, bushier the
//! weaker the pull toward the middle. A burrow, a nest, a crack system.
//!
//! Both are [`Phase::Ground`] passes over a map of solid rock, and neither
//! publishes an [`Area`](crate::Area): they carve one shape, not rooms.

use atglyph_core::{Direction, Grid2D, GridRect, Point};
use atglyph_grid::TileId;
use rand::Rng;

use crate::chain::{BuildError, Pass, Phase};
use crate::context::BaseContext;

/// Where carving starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// The middle of the map.
    Middle,
    /// This cell.
    At(Point),
}

/// The part of the map a carving may open: all of it but `margin` cells
/// of rock round the edge.
fn inside(ctx: &BaseContext, margin: i32) -> Result<GridRect, BuildError> {
    let b = ctx.terrain().bounds();
    let (w, h) = (b.width - 2 * margin, b.height - 2 * margin);
    if w < 1 || h < 1 {
        return Err(BuildError::new("carve", format!("a margin of {margin} leaves nothing of a {}x{} map", b.width, b.height)));
    }
    Ok(GridRect::new(b.x + margin, b.y + margin, w, h))
}

/// Where carving starts inside `inside`, or an error naming `pass` if the
/// cell asked for is outside it.
fn start(from: Origin, inside: GridRect, pass: &'static str) -> Result<Point, BuildError> {
    let at = match from {
        Origin::Middle => inside.center(),
        Origin::At(p) => p,
    };
    if inside.contains(at) { Ok(at) } else { Err(BuildError::new(pass, format!("{at:?} is outside the ground it may open"))) }
}

/// The cells opened so far, and how to open more.
struct Carving {
    inside: GridRect,
    floor: TileId,
    brush: i32,
    opened: Vec<Point>,
    goal: usize,
}

impl Carving {
    fn new(ctx: &BaseContext, inside: GridRect, floor: TileId, brush: i32, open_pct: u32) -> Carving {
        let goal = (inside.area() as u64 * u64::from(open_pct.min(100)) / 100).max(1) as usize;
        let opened = inside.cells().filter(|&p| ctx.terrain().get(p) == Some(floor)).collect();
        Carving { inside, floor, brush: brush.max(0), opened, goal }
    }

    fn done(&self) -> bool {
        self.opened.len() >= self.goal
    }

    fn is_open(&self, ctx: &BaseContext, p: Point) -> bool {
        ctx.terrain().get(p) == Some(self.floor)
    }

    /// Opens the brush round `p`, inside.
    fn open(&mut self, ctx: &mut BaseContext, p: Point) {
        for dy in -self.brush..=self.brush {
            for dx in -self.brush..=self.brush {
                let q = Point::new(p.x + dx, p.y + dy);
                if self.inside.contains(q) && !self.is_open(ctx, q) {
                    ctx.terrain_mut().set(q, self.floor);
                    self.opened.push(q);
                }
            }
        }
    }
}

/// A drunkard's walk: walkers set out from the open ground, the first from
/// [`from`](Self::from), each taking [`stride`](Self::stride) random steps
/// and opening a brush of floor round every one, until
/// [`open_pct`](Self::open_pct) of the ground inside the margin is open.
#[derive(Debug, Clone, Copy)]
pub struct DrunkardsWalk {
    /// What is opened.
    pub floor: TileId,
    /// The share of the ground inside the margin to open, in a hundred.
    pub open_pct: u32,
    /// How far the brush reaches either side of a step: 0 opens the one
    /// cell, a lace of one-cell ways; 1 a square of nine, open ground.
    pub brush: i32,
    /// How many steps one walker takes before the next sets out from
    /// somewhere already open: short strides gather round the start, long
    /// ones wander off and widen the hall.
    pub stride: u32,
    /// The cells of rock left round the map's edge.
    pub margin: i32,
    /// Where the first walker sets out.
    pub from: Origin,
}

impl Pass for DrunkardsWalk {
    fn name(&self) -> &'static str {
        "drunkards_walk"
    }

    fn phase(&self) -> Phase {
        Phase::Ground
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let inside = inside(ctx, self.margin)?;
        let first = start(self.from, inside, self.name())?;
        let mut carving = Carving::new(ctx, inside, self.floor, self.brush, self.open_pct);
        carving.open(ctx, first);
        while !carving.done() {
            let mut at = carving.opened[ctx.rng().random_range(0..carving.opened.len())];
            for _ in 0..self.stride.max(1) {
                let next = at + Direction::CARDINALS[ctx.rng().random_range(0..4)].offset();
                if !inside.contains(next) {
                    continue;
                }
                at = next;
                carving.open(ctx, at);
                if carving.done() {
                    break;
                }
            }
        }
        Ok(())
    }
}

/// Diffusion-limited aggregation: particles set out from the rock round
/// what is open and wander until one stands beside it, and the cell it
/// stopped on opens, until [`open_pct`](Self::open_pct) of the ground
/// inside the margin is open.
///
/// A particle sets out a few cells beyond what is open rather than
/// anywhere on the map, and gives up after a long wander for a fresh one,
/// so growth costs about the same however small it still is.
#[derive(Debug, Clone, Copy)]
pub struct Aggregate {
    /// What is opened.
    pub floor: TileId,
    /// The share of the ground inside the margin to open, in a hundred.
    pub open_pct: u32,
    /// How far the brush reaches either side of where a particle sticks:
    /// 0 a crack a cell wide, 1 a tunnel three wide.
    pub brush: i32,
    /// The chance in a hundred that a particle's step goes toward where
    /// growth started rather than at random: none for the bushiest growth,
    /// branching every way; more for long arms reaching out from a dense
    /// heart.
    pub pull_pct: u32,
    /// The cells of rock left round the map's edge.
    pub margin: i32,
    /// Where growth starts.
    pub from: Origin,
}

/// How far beyond what is open a particle sets out, and how many steps it
/// wanders before it is given up.
const SPAWN_BEYOND: i32 = 6;
const WANDER: u32 = 2_000;

impl Pass for Aggregate {
    fn name(&self) -> &'static str {
        "aggregate"
    }

    fn phase(&self) -> Phase {
        Phase::Ground
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let inside = inside(ctx, self.margin)?;
        let seed = start(self.from, inside, self.name())?;
        let mut carving = Carving::new(ctx, inside, self.floor, self.brush, self.open_pct);
        carving.open(ctx, seed);
        // The box round what is open, which particles set out from.
        let (mut lo, mut hi) = (seed, seed);
        while !carving.done() {
            let field = GridRect::from_corners(lo, hi).inflate(SPAWN_BEYOND + self.brush).intersection(&inside).unwrap_or(inside);
            let mut at = Point::new(ctx.rng().random_range(field.x..field.right()), ctx.rng().random_range(field.y..field.bottom()));
            if carving.is_open(ctx, at) {
                continue;
            }
            for _ in 0..WANDER {
                if Direction::CARDINALS.iter().any(|d| carving.is_open(ctx, at + d.offset())) {
                    carving.open(ctx, at);
                    lo = Point::new(lo.x.min(at.x), lo.y.min(at.y));
                    hi = Point::new(hi.x.max(at.x), hi.y.max(at.y));
                    break;
                }
                let toward = ctx.rng().random_range(0..100) < self.pull_pct;
                let step = if toward {
                    let (dx, dy) = (seed.x - at.x, seed.y - at.y);
                    if dx.abs() >= dy.abs() { Point::new(dx.signum(), 0) } else { Point::new(0, dy.signum()) }
                } else {
                    Direction::CARDINALS[ctx.rng().random_range(0..4)].offset()
                };
                let next = at + step;
                if field.contains(next) {
                    at = next;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Chain;
    use atglyph_core::{PlaceSeed, RunSeed};
    use atglyph_grid::TileRegistry;

    fn carved(pass: impl Pass + 'static, seed: u32) -> (BaseContext, TileId) {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut ctx = BaseContext::blank(60, 40, tiles, wall);
        Chain::new().then(pass).run(&mut ctx, PlaceSeed::of(RunSeed(7), seed)).expect("it carves");
        (ctx, floor)
    }

    /// The open cells, and how many of them the first reaches orthogonally.
    fn reach(ctx: &BaseContext, floor: TileId) -> (usize, usize) {
        let open: Vec<Point> = ctx.terrain().bounds().cells().filter(|&p| ctx.terrain().get(p) == Some(floor)).collect();
        let mut seen = vec![open[0]];
        let mut i = 0;
        while i < seen.len() {
            for d in Direction::CARDINALS {
                let q = seen[i] + d.offset();
                if ctx.terrain().get(q) == Some(floor) && !seen.contains(&q) {
                    seen.push(q);
                }
            }
            i += 1;
        }
        (open.len(), seen.len())
    }

    /// The share of open cells with open ground on all eight sides.
    fn openness(ctx: &BaseContext, floor: TileId) -> f32 {
        let open = |p: Point| ctx.terrain().get(p) == Some(floor);
        let cells: Vec<Point> = ctx.terrain().bounds().cells().filter(|&p| open(p)).collect();
        cells.iter().filter(|&&p| Direction::ALL.iter().all(|d| open(p + d.offset()))).count() as f32 / cells.len() as f32
    }

    fn walk(floor: TileId) -> DrunkardsWalk {
        DrunkardsWalk { floor, open_pct: 45, brush: 1, stride: 300, margin: 2, from: Origin::Middle }
    }

    fn aggregate(floor: TileId) -> Aggregate {
        Aggregate { floor, open_pct: 25, brush: 0, pull_pct: 0, margin: 2, from: Origin::Middle }
    }

    #[test]
    fn either_carving_opens_its_share_in_one_piece_inside_its_margin() {
        let floor = TileRegistry::standard().expect("floor");
        for seed in 0..4 {
            for (name, (ctx, floor)) in [("walk", carved(walk(floor), seed)), ("aggregate", carved(aggregate(floor), seed))] {
                let (open, reached) = reach(&ctx, floor);
                assert_eq!(open, reached, "{name} {seed}: one piece");
                let inside = GridRect::new(2, 2, 56, 36);
                let asked = if name == "walk" { 45 } else { 25 } * inside.area() as usize / 100;
                assert!(open >= asked && open < asked + 20, "{name} {seed}: {open} open for {asked} asked");
                assert!(
                    ctx.terrain().bounds().cells().filter(|p| !inside.contains(*p)).all(|p| ctx.terrain().get(p) != Some(floor)),
                    "{name} {seed}: the margin stands"
                );
            }
        }
    }

    #[test]
    fn a_walk_opens_halls_and_aggregation_grows_branches() {
        let floor = TileRegistry::standard().expect("floor");
        let (walked, _) = carved(walk(floor), 1);
        let (grown, _) = carved(Aggregate { open_pct: 45, ..aggregate(floor) }, 1);
        let (hall, branches) = (openness(&walked, floor), openness(&grown, floor));
        assert!(hall > 0.4 && branches < 0.1, "a walk's hall {hall:.2}, aggregation's branches {branches:.2}");
    }

    #[test]
    fn a_start_off_the_ground_it_may_open_is_refused() {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut ctx = BaseContext::blank(60, 40, tiles, wall);
        let off = DrunkardsWalk { from: Origin::At(Point::new(0, 0)), ..walk(floor) };
        assert!(Chain::new().then(off).run(&mut ctx, PlaceSeed::of(RunSeed(7), 0)).is_err());
    }
}
