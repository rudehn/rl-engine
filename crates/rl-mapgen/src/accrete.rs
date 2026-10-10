//! Growing a place piece by piece.
//!
//! A [`Shape`] draws a [`Piece`]: the floor of one area and the [sites
//! ](Piece::sites) where it may be joined to another. [`Accrete`] lays a
//! first piece and attaches every later one to a wall of what already
//! stands, so the place is walkable from end to end before anything checks
//! that it is. A [`Barrier`] is rock growth goes round, so a place bends
//! round it rather than running straight across. What it laid it
//! publishes: an [`Area`] per piece, an [`Opening`] per join and a
//! [`LaidBarrier`] per barrier present.

use rand::seq::SliceRandom;
use rand::{Rng, RngCore};
use rl_core::geometry::{euclidean_sq, line};
use rl_core::{Direction, Grid, Grid2D, Point, Rect};
use rl_grid::{BitGrid, Terrain, TileId};

use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::dungeon::Room;
use crate::prefab::pick_weighted;

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
    /// would open onto the piece's own wall. A shape may offer fewer: a
    /// [`Cave`](crate::cave::Cave) offers none on rock it encloses.
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
    /// A passage is not: it emits no `Room`, so nothing furnishes it, and
    /// since nothing can be put in it, one left with a single way in is
    /// taken back out of the place. A shape meant to be a blind corridor
    /// says it is a room.
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

/// A region's taste in shapes: a name and the shapes drawn there, by
/// weight. It biases a draw and builds nothing, so no wall follows a
/// district's edge and a pass with one district is a pass with none.
#[derive(Debug, Clone)]
pub struct District {
    /// The district's name, which an [`Area`] grown in it carries.
    pub name: String,
    /// Each an index into [`Accrete::shapes`] and the weight it is drawn at.
    pub shapes: Vec<(usize, u32)>,
}

/// One piece as it was laid: what a later pass needs to decide what
/// belongs in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Area {
    /// Its place in the order the pieces were laid; an [`Opening`] names
    /// its two areas by this.
    pub index: usize,
    /// The name of the shape that drew it.
    pub shape: String,
    /// The name of the district it was grown in.
    pub district: String,
    /// Every cell of its floor, in map coordinates.
    pub cells: Vec<Point>,
    /// The smallest rectangle holding them.
    pub bounds: Rect,
    /// The largest rectangle made only of them: where a stamp is centred.
    pub inner: Rect,
}

/// A way between two areas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    /// The cells opened in the wall between them; the first is where the
    /// two were joined, and a mouth has more.
    pub cells: Vec<Point>,
    /// The direction that leads from the first area to the second.
    pub facing: Direction,
    /// The two areas, by [`Area::index`].
    pub areas: (usize, usize),
    /// How they were joined.
    pub join: Join,
}

/// The cell a share of a side of `side` cells falls on, rounded: nought is
/// the first cell and a hundred the last, so a share names the same part
/// of a map at any size.
pub fn cell_at_share(pct: u32, side: i32) -> i32 {
    ((i64::from(pct) * i64::from(side - 1) + 50) / 100) as i32
}

/// A straight segment of rock growth may not cross, present at a chance
/// and shifted by a jitter, so one recipe bends its place round a
/// different wall each time it is built.
///
/// It is given in shares of the map, each 0 to 100, so a recipe lays it
/// at any size. No piece's body or the ring round it may take a cell of
/// it, so a barrier is never a shared wall a join could open: what lies
/// either side is reached only round its ends. One that may reach from
/// an edge to the opposite one is refused, since growth could never cross
/// to the far side; one that cuts off a corner is allowed, and the corner
/// stays rock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Barrier {
    /// One end, as shares of the map's width and height.
    pub from: (u32, u32),
    /// The other end.
    pub to: (u32, u32),
    /// The chance in a hundred it is laid at all.
    pub chance_pct: u32,
    /// The furthest it is shifted, as a share of the width along a row and
    /// of the height along a column. Both ends move together, so the
    /// barrier keeps its length and slope and only its place changes.
    pub jitter_pct: u32,
}

/// How close to an edge a barrier's end may come before nothing can pass
/// it there: a piece needs a cell inside the border, and its body and
/// ring stand a cell clear of the barrier, so an end within this many
/// cells of the edge leaves no way round.
const REACH: i32 = 2;

impl Barrier {
    /// Its two ends in cells on a `w` by `h` map, unshifted.
    fn ends(&self, w: i32, h: i32) -> (Point, Point) {
        let at = |(x, y): (u32, u32)| Point::new(cell_at_share(x, w), cell_at_share(y, h));
        (at(self.from), at(self.to))
    }

    /// Whether some shift its jitter allows lays it from one edge of a `w`
    /// by `h` map to the opposite one, so that growth on one side could
    /// never cross to the other. `rl-rules` refuses such a barrier when it
    /// loads a layout, by the same arithmetic.
    pub fn may_cut(&self, w: i32, h: i32) -> bool {
        let (a, b) = self.ends(w, h);
        // A shift `s` along a side of `n` cells lays the ends at `lo + s`
        // and `hi + s`; it cuts when the first is within reach of the near
        // edge and the second of the far one.
        let across = |lo: i32, hi: i32, n: i32, jitter: i32| (-jitter).max(n - 1 - REACH - hi) <= jitter.min(REACH - lo);
        across(a.x.min(b.x), a.x.max(b.x), w, cell_at_share(self.jitter_pct, w)) || across(a.y.min(b.y), a.y.max(b.y), h, cell_at_share(self.jitter_pct, h))
    }
}

/// A barrier that was laid: what a later pass needs to know which rock
/// growth was kept off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaidBarrier {
    /// Which barrier, by its place in [`Accrete::barriers`].
    pub index: usize,
    /// Its cells on the map, after the shift; a shift may carry some or
    /// all of it off.
    pub cells: Vec<Point>,
}

/// Grows a place by accretion over a map of solid tile: a first piece,
/// then each later one attached to a wall of what already stands.
///
/// Every area is joined to one that was already joined, so every open cell
/// can be walked to from every other by construction. A piece fits only
/// where its body and the ring one cell round it are solid, which leaves
/// exactly one wall between two areas and never none.
///
/// How much is grown and how many districts there are is a share and a
/// density, never a count of cells of map, so one pass grows a small place
/// and a large one. A passage left with
/// one way in is turned back to what it was carved from, since nothing
/// is ever put in a passage and a dead end should be somewhere. It fails
/// if the open share is not reached before `patience` attempts in a row
/// are spent.
///
/// A [`Barrier`] present on this build is drawn before anything else and
/// kept clear as the border is, and further: a ring may stand on the
/// border, but no body or ring may take a barrier's cells, so no join or
/// later loop opens through one and what grows on one side reaches the
/// other only round its ends.
pub struct Accrete {
    /// The tile a piece's body is carved from.
    pub floor: TileId,
    /// The tile a [`Join::Door`] opens as.
    pub door: TileId,
    /// Every shape a district may draw.
    pub shapes: Vec<Box<dyn Shape>>,
    /// Every district the mix may scatter.
    pub districts: Vec<District>,
    /// Each an index into `districts` and the weight a seed draws it at.
    pub mix: Vec<(usize, u32)>,
    /// District seeds per ten thousand cells of map, rounded, at least one.
    pub districts_per_10k: u32,
    /// The share of the map, in a hundred, to open before stopping.
    pub open_pct: u32,
    /// How many attempts in a row may fail before growth gives up.
    pub patience: u32,
    /// Rock growth goes round, each laid at its own chance; none for a
    /// place grown wherever it can.
    pub barriers: Vec<Barrier>,
}

fn solid(terrain: &Terrain, walkable: &[bool], p: Point) -> bool {
    terrain.get(p).is_some_and(|t| !walkable[t.index()])
}

/// Where growth may stand: the map inside its border, less every cell a
/// body may not take because its ring would touch a barrier.
struct Ground<'a> {
    inner: Rect,
    /// Each barrier cell and the cells round it.
    kept: &'a BitGrid,
}

impl Ground<'_> {
    fn holds(&self, p: Point) -> bool {
        self.inner.contains(p) && !self.kept.contains(p)
    }
}

/// Whether a piece moved by `by` lies where growth may stand with its body
/// and the ring round it all solid.
fn fits(terrain: &Terrain, walkable: &[bool], ground: &Ground, cells: &[Point], by: Point) -> bool {
    cells.iter().all(|c| {
        let p = *c + by;
        ground.holds(p) && (-1..=1).all(|dy| (-1..=1).all(|dx| solid(terrain, walkable, p.offset(dx, dy))))
    })
}

/// One of `rows` by weight; `None` when nothing carries weight.
fn draw_weighted(rows: &[(usize, u32)], rng: &mut dyn RngCore) -> Option<usize> {
    let weights: Vec<u32> = rows.iter().map(|(_, w)| *w).collect();
    let total = weights.iter().try_fold(0u32, |sum, w| sum.checked_add(*w)).filter(|t| *t > 0)?;
    Some(rows[pick_weighted(&weights, rng.random_range(0..total))].0)
}

/// What growth has laid so far.
struct Grown {
    owner: Grid<u32>,
    areas: Vec<Area>,
    /// Whether each area's shape draws rooms.
    rooms: Vec<bool>,
    /// The tiles each area's cells were carved from, cell for cell.
    under: Vec<Vec<TileId>>,
    /// Each opening, and the tiles its cells were opened from.
    openings: Vec<(Opening, Vec<TileId>)>,
    frontier: Vec<(Point, Direction, usize)>,
    open: u32,
}

impl Accrete {
    fn err(reason: impl Into<String>) -> BuildError {
        BuildError::new("accrete", reason)
    }

    /// What can be told is wrong before a cell is drawn, so a mistake in
    /// how the pass was built is one error and not a panic mid-growth.
    fn check(&self, w: i32, h: i32) -> Result<(), BuildError> {
        if self.shapes.is_empty() || self.districts.is_empty() {
            return Err(Self::err("it has no shapes or no districts to draw from"));
        }
        // In range, and adding up to more than nought without overflowing:
        // exactly what `draw_weighted` needs to hand back a row.
        let weighs = |rows: &[(usize, u32)], len: usize| {
            rows.iter().all(|(i, _)| *i < len) && rows.iter().try_fold(0u32, |sum, (_, w)| sum.checked_add(*w)).is_some_and(|total| total > 0)
        };
        if !weighs(&self.mix, self.districts.len()) {
            return Err(Self::err("its mix names no district with a weight, or the weights overflow a u32"));
        }
        for d in &self.districts {
            if !weighs(&d.shapes, self.shapes.len()) {
                return Err(Self::err(format!("district {:?} names no shape with a weight, or the weights overflow a u32", d.name)));
            }
        }
        for (i, b) in self.barriers.iter().enumerate() {
            if [b.from.0, b.from.1, b.to.0, b.to.1, b.chance_pct, b.jitter_pct].iter().any(|pct| *pct > 100) {
                return Err(Self::err(format!("barrier {i} has a share past a hundred")));
            }
            if b.may_cut(w, h) {
                return Err(Self::err(format!("barrier {i} may reach from one edge of a {w}x{h} map to the opposite, and growth could never cross it")));
            }
        }
        Ok(())
    }

    /// Carves `piece` moved by `by` and records it as drawn by that shape
    /// in that district, with every site but the one it was joined at
    /// added to the frontier. Returns its index.
    fn lay<C: BuildContext>(
        &self,
        ctx: &mut C,
        grown: &mut Grown,
        piece: &Piece,
        by: Point,
        (shape, district): (usize, usize),
        joined: Option<Point>,
    ) -> usize {
        let index = grown.areas.len();
        let cells: Vec<Point> = piece.cells.iter().map(|c| *c + by).collect();
        let under = cells.iter().map(|c| ctx.terrain().get(*c).expect("a piece that fits is on the map")).collect();
        for c in &cells {
            ctx.terrain_mut().set(*c, self.floor);
            grown.owner.set(*c, index as u32 + 1);
        }
        grown.under.push(under);
        grown.rooms.push(self.shapes[shape].is_room());
        grown.open += cells.len() as u32;
        grown.frontier.extend(piece.sites.iter().map(|(q, d)| (*q + by, *d, index)).filter(|(q, _, _)| Some(*q) != joined));
        let bounds = bounds_of(&cells);
        grown.areas.push(Area {
            index,
            shape: self.shapes[shape].name().to_string(),
            district: self.districts[district].name.clone(),
            inner: largest_rect(&cells, bounds),
            bounds,
            cells,
        });
        index
    }

    /// Turns every passage that leads nowhere back into what it was carved
    /// from, with the opening into it, until none is left, and renumbers
    /// what remains. Returns how many went.
    ///
    /// A passage holds nothing, so one with a single way in is a walk to a
    /// wall. Taking one away may leave the passage it hung from leading
    /// nowhere in its turn, so it repeats.
    fn trim<C: BuildContext>(&self, ctx: &mut C, grown: &mut Grown) -> usize {
        let mut gone = vec![false; grown.areas.len()];
        let mut left = gone.len();
        loop {
            let ways = |i: usize| grown.openings.iter().filter(|(o, _)| o.areas.0 == i || o.areas.1 == i).count();
            let Some(i) = (0..gone.len()).find(|i| !gone[*i] && !grown.rooms[*i] && ways(*i) <= 1) else { break };
            if left == 1 {
                break;
            }
            gone[i] = true;
            left -= 1;
            for (c, tile) in grown.areas[i].cells.iter().zip(&grown.under[i]) {
                ctx.terrain_mut().set(*c, *tile);
                grown.owner.set(*c, 0);
            }
            grown.open -= grown.areas[i].cells.len() as u32;
            if let Some(at) = grown.openings.iter().position(|(o, _)| o.areas.0 == i || o.areas.1 == i) {
                let (opening, under) = grown.openings.remove(at);
                for (c, tile) in opening.cells.iter().zip(&under) {
                    ctx.terrain_mut().set(*c, *tile);
                }
                grown.open -= opening.cells.len() as u32;
                // The wall is whole again, so what is left may be joined there once more.
                let (kept, out) = if opening.areas.0 == i { (opening.areas.1, opening.facing.opposite()) } else { (opening.areas.0, opening.facing) };
                grown.frontier.push((opening.cells[0], out, kept));
            }
        }
        let removed = gone.len() - left;
        if removed == 0 {
            return 0;
        }
        let mut renumbered = vec![usize::MAX; gone.len()];
        for (next, i) in (0..gone.len()).filter(|i| !gone[*i]).enumerate() {
            renumbered[i] = next;
        }
        grown.frontier.retain(|(_, _, area)| !gone[*area]);
        for site in &mut grown.frontier {
            site.2 = renumbered[site.2];
        }
        for (opening, _) in &mut grown.openings {
            opening.areas = (renumbered[opening.areas.0], renumbered[opening.areas.1]);
        }
        let mut kept = gone.iter().map(|g| !g);
        grown.areas.retain(|_| kept.next().expect("one flag per area"));
        let mut kept = gone.iter().map(|g| !g);
        grown.rooms.retain(|_| kept.next().expect("one flag per area"));
        let mut kept = gone.iter().map(|g| !g);
        grown.under.retain(|_| kept.next().expect("one flag per area"));
        for (i, area) in grown.areas.iter_mut().enumerate() {
            area.index = i;
            for c in &area.cells {
                grown.owner.set(*c, i as u32 + 1);
            }
        }
        removed
    }
}

/// How many times growth is taken up again after passages that led
/// nowhere were taken back. Each round only grows from what the
/// last left standing, so a few are enough to refill what was lost.
const ROUNDS: u32 = 8;

impl<C: BuildContext> Pass<C> for Accrete {
    fn name(&self) -> &'static str {
        "accrete"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let walkable = ctx.tiles().tables().walkable;
        let bounds = ctx.terrain().bounds();
        self.check(bounds.width, bounds.height)?;
        if bounds.width < 3 || bounds.height < 3 {
            return Err(Self::err("the map has no room inside its border"));
        }
        let inner = bounds.inflate(-1);

        // The barriers first, so where they lie decides where everything
        // after may grow. A recipe with none draws nothing here.
        let mut laid = Vec::new();
        let mut kept = BitGrid::new(bounds.width, bounds.height);
        for (index, b) in self.barriers.iter().enumerate() {
            if ctx.rng().random_range(0..100) >= b.chance_pct {
                continue;
            }
            let (jx, jy) = (cell_at_share(b.jitter_pct, bounds.width), cell_at_share(b.jitter_pct, bounds.height));
            let shift = Point::new(ctx.rng().random_range(-jx..=jx), ctx.rng().random_range(-jy..=jy));
            let (from, to) = b.ends(bounds.width, bounds.height);
            let cells: Vec<Point> = line(from + shift, to + shift).filter(|c| bounds.contains(*c)).collect();
            for c in &cells {
                for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
                    kept.insert(c.offset(dx, dy));
                }
            }
            laid.push(LaidBarrier { index, cells });
        }
        let ground = Ground { inner, kept: &kept };
        let total = (bounds.width * bounds.height) as u32;
        let point_inside = |rng: &mut dyn RngCore| Point::new(rng.random_range(inner.x..inner.right()), rng.random_range(inner.y..inner.bottom()));

        let seed_count = ((u64::from(total) * u64::from(self.districts_per_10k) + 5_000) / 10_000).max(1);
        if seed_count > u64::from(total) {
            return Err(Self::err(format!("{} district seeds per ten thousand cells is more seeds than the map has cells", self.districts_per_10k)));
        }
        let mut seeds = Vec::new();
        for _ in 0..seed_count {
            let at = point_inside(ctx.rng());
            seeds.push((at, draw_weighted(&self.mix, ctx.rng()).expect("checked")));
        }
        // `min_by_key` keeps the first of equals, so a tie goes to the earlier seed.
        let district_at = |p: Point| seeds.iter().min_by_key(|(s, _)| euclidean_sq(*s, p)).expect("at least one seed").1;

        let mut grown = Grown {
            owner: Grid::filled(bounds.width, bounds.height, 0),
            areas: Vec::new(),
            rooms: Vec::new(),
            under: Vec::new(),
            openings: Vec::new(),
            frontier: Vec::new(),
            open: 0,
        };

        // The first piece, anywhere it fits.
        for _ in 0..self.patience {
            let at = point_inside(ctx.rng());
            let district = district_at(at);
            let shape = draw_weighted(&self.districts[district].shapes, ctx.rng()).expect("checked");
            let piece = self.shapes[shape].draw(ctx.rng());
            if fits(ctx.terrain(), &walkable, &ground, &piece.cells, at) {
                self.lay(ctx, &mut grown, &piece, at, (shape, district), None);
                break;
            }
        }
        if grown.areas.is_empty() {
            return Err(Self::err(format!("no first piece fit a {}x{} map in {} attempts", bounds.width, bounds.height, self.patience)));
        }

        for round in 1..=ROUNDS {
            let mut spent = 0;
            while grown.open * 100 < self.open_pct * total && spent < self.patience && !grown.frontier.is_empty() {
                let i = ctx.rng().random_range(0..grown.frontier.len());
                let (site, facing, from) = grown.frontier[i];
                let beyond = site + facing.offset();
                // A site another piece has since grown over or against leads nowhere.
                if !solid(ctx.terrain(), &walkable, site) || !ground.holds(beyond) || !solid(ctx.terrain(), &walkable, beyond) {
                    grown.frontier.swap_remove(i);
                    continue;
                }
                let district = district_at(site);
                let shape = draw_weighted(&self.districts[district].shapes, ctx.rng()).expect("checked");
                let piece = self.shapes[shape].draw(ctx.rng());
                let mut meets: Vec<Point> = piece.sites.iter().filter(|(_, d)| *d == facing.opposite()).map(|(q, _)| *q).collect();
                meets.shuffle(ctx.rng());
                let Some(by) = meets.iter().map(|q| site - *q).find(|by| fits(ctx.terrain(), &walkable, &ground, &piece.cells, *by)) else {
                    spent += 1;
                    continue;
                };
                spent = 0;
                grown.frontier.swap_remove(i);
                let to = self.lay(ctx, &mut grown, &piece, by, (shape, district), Some(site));

                let join = self.shapes[shape].join();
                let mut cells = vec![site];
                let mut under = vec![ctx.terrain().get(site).expect("a site is on the map")];
                ctx.terrain_mut().set(site, if join == Join::Door { self.door } else { self.floor });
                if let Join::Mouth(width) = join {
                    // Widen along the wall while the same two areas face each
                    // other across it, a cell to one side and then the other,
                    // so the mouth is centred on the join where both allow.
                    let sides = facing.perpendicular();
                    let mut next = [site + sides[0].offset(), site + sides[1].offset()];
                    let mut shut = [false, false];
                    let mut turn = 0;
                    while cells.len() < width as usize && !(shut[0] && shut[1]) {
                        let k = turn % 2;
                        turn += 1;
                        if shut[k] {
                            continue;
                        }
                        let q = next[k];
                        let across = solid(ctx.terrain(), &walkable, q)
                            && grown.owner.get(q - facing.offset()).copied() == Some(from as u32 + 1)
                            && grown.owner.get(q + facing.offset()).copied() == Some(to as u32 + 1);
                        if !across {
                            shut[k] = true;
                            continue;
                        }
                        under.push(ctx.terrain().get(q).expect("solid, so on the map"));
                        ctx.terrain_mut().set(q, self.floor);
                        cells.push(q);
                        next[k] = q + sides[k].offset();
                    }
                }
                grown.open += cells.len() as u32;
                grown.openings.push((Opening { cells, facing, areas: (from, to), join }, under));
            }
            // What was grown to fill the share may itself lead nowhere, so
            // the last round's trim always stands.
            if self.trim(ctx, &mut grown) == 0 || round == ROUNDS {
                break;
            }
        }

        if grown.open * 100 < self.open_pct * total {
            return Err(Self::err(format!(
                "grew to {}% open of the {}% asked for, once every passage with one way in was taken back",
                grown.open * 100 / total,
                self.open_pct
            )));
        }
        for (area, room) in grown.areas.into_iter().zip(grown.rooms) {
            if room {
                ctx.emit(Room(area.inner));
            }
            ctx.emit(area);
        }
        for (opening, _) in grown.openings {
            ctx.emit(opening);
        }
        for barrier in laid {
            ctx.emit(barrier);
        }
        Ok(())
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

    use crate::chain::Chain;
    use crate::context::BaseContext;
    use rl_core::{Grid2D, RunSeed, Steps};
    use rl_grid::region::flood;
    use rl_grid::{BitGrid, TileProps, TileRegistry};

    pub(crate) struct Rig {
        pub tiles: TileRegistry,
        pub wall: TileId,
        pub floor: TileId,
        pub door: TileId,
    }

    pub(crate) fn rig() -> Rig {
        let mut tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let door = tiles.register(TileProps::floor("way through")).unwrap();
        Rig { tiles, wall, floor, door }
    }

    /// Up to three districts over four shapes, at a share both test sizes reach.
    pub(crate) fn accrete(rig: &Rig, districts: usize) -> Accrete {
        let shapes: Vec<Box<dyn Shape>> = vec![
            Box::new(Rectangle { name: "cell".into(), w: (5, 9), h: (5, 8), join: Join::Door }),
            Box::new(Rectangle { name: "big".into(), w: (11, 18), h: (8, 12), join: Join::Mouth(3) }),
            Box::new(Cross { name: "cross".into(), long: (9, 13), short: (3, 5), join: Join::Door }),
            Box::new(Passage { name: "run".into(), width: (1, 2), length: (4, 12), bend_pct: 30, join: Join::Arch }),
        ];
        let all = vec![
            District { name: "wide".into(), shapes: vec![(1, 5), (0, 2), (3, 1)] },
            District { name: "tight".into(), shapes: vec![(0, 5), (2, 2), (3, 2)] },
            District { name: "runs".into(), shapes: vec![(3, 5), (0, 2)] },
        ];
        let districts: Vec<District> = all.into_iter().take(districts).collect();
        let mix = (0..districts.len()).map(|i| (i, 1)).collect();
        Accrete { floor: rig.floor, door: rig.door, shapes, districts, mix, districts_per_10k: 7, open_pct: 38, patience: 300, barriers: Vec::new() }
    }

    pub(crate) fn build(pass: Accrete, rig: &Rig, w: i32, h: i32, seed: u64) -> Result<BaseContext, BuildError> {
        Chain::new().then(pass).run_retrying(|| BaseContext::blank(w, h, rig.tiles.clone(), rig.wall), RunSeed(seed), 20)
    }

    pub(crate) fn walkable(ctx: &BaseContext, p: Point) -> bool {
        let tables = ctx.tiles().tables();
        ctx.terrain().get(p).is_some_and(|t| tables.walkable[t.index()])
    }

    fn sizes() -> [(i32, i32, std::ops::Range<u64>); 2] {
        [(84, 48, 0..60), (160, 90, 0..12)]
    }

    #[test]
    fn every_open_cell_of_an_accreted_place_can_be_walked_to_from_every_other() {
        let rig = rig();
        for (w, h, seeds) in sizes() {
            for s in seeds {
                let ctx = build(accrete(&rig, 3), &rig, w, h, s).unwrap_or_else(|e| panic!("{w}x{h} seed {s}: {e}"));
                let tables = ctx.tiles().tables();
                let open = |i: usize| tables.walkable[ctx.terrain().get_idx(i).index()];
                let start = ctx.outputs().first::<Area>().unwrap().cells[0];
                let mut seen = BitGrid::new(w, h);
                let reached = flood(ctx.terrain().grid(), start, open, Steps::Four, &mut seen);
                let total = (0..(w * h) as usize).filter(|i| open(*i)).count();
                assert_eq!(reached, total, "{w}x{h} seed {s}");
            }
        }
    }

    #[test]
    fn no_two_areas_share_a_cell_and_no_two_touch_except_through_an_opening() {
        let rig = rig();
        for (w, h, seeds) in sizes() {
            for s in seeds {
                let ctx = build(accrete(&rig, 3), &rig, w, h, s).unwrap();
                let mut owner = Grid::<u32>::filled(w, h, 0);
                for a in ctx.outputs().iter::<Area>() {
                    for c in &a.cells {
                        assert_eq!(*owner.get(*c).unwrap(), 0, "{w}x{h} seed {s}: {c:?} is in two areas");
                        owner.set(*c, a.index as u32 + 1);
                    }
                }
                for a in ctx.outputs().iter::<Area>() {
                    for c in &a.cells {
                        for dy in -1..=1 {
                            for dx in -1..=1 {
                                let o = owner.get(c.offset(dx, dy)).copied().unwrap_or(0);
                                assert!(o == 0 || o == a.index as u32 + 1, "{w}x{h} seed {s}: area {} touches area {} at {c:?}", a.index, o - 1);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_opening_has_floor_of_its_two_areas_on_either_side_of_the_way_through() {
        let rig = rig();
        for s in 0..60 {
            let ctx = build(accrete(&rig, 3), &rig, 84, 48, s).unwrap();
            let areas: Vec<&Area> = ctx.outputs().iter::<Area>().collect();
            for o in ctx.outputs().iter::<Opening>() {
                for c in &o.cells {
                    assert!(walkable(&ctx, *c), "seed {s}: opening cell {c:?} is solid");
                    let (back, on) = (*c - o.facing.offset(), *c + o.facing.offset());
                    assert!(areas[o.areas.0].cells.contains(&back), "seed {s}: {back:?} is not in area {}", o.areas.0);
                    assert!(areas[o.areas.1].cells.contains(&on), "seed {s}: {on:?} is not in area {}", o.areas.1);
                }
                let limit = match o.join {
                    Join::Mouth(n) => n as usize,
                    _ => 1,
                };
                assert!(!o.cells.is_empty() && o.cells.len() <= limit, "seed {s}: {:?} is {} wide", o.join, o.cells.len());
            }
            assert_eq!(ctx.outputs().iter::<Opening>().count(), areas.len() - 1, "seed {s}: growth alone is a tree");
        }
    }

    #[test]
    fn a_place_that_built_reached_its_open_share_and_one_that_did_not_failed() {
        let rig = rig();
        for s in 0..40 {
            for target in [38, 95] {
                let mut pass = accrete(&rig, 3);
                pass.open_pct = target;
                // One attempt: a share out of reach fails every one of them the same way.
                match Chain::new().then(pass).run_retrying(|| BaseContext::blank(84, 48, rig.tiles.clone(), rig.wall), RunSeed(s), 1) {
                    Ok(ctx) => {
                        let open = ctx.terrain().bounds().cells().filter(|p| walkable(&ctx, *p)).count();
                        assert!(open * 100 >= target as usize * 84 * 48, "seed {s}: built at {open} open cells short of {target}%");
                    }
                    Err(e) => assert_eq!(e.pass, "accrete", "seed {s}"),
                }
            }
        }
        // A share no place can reach fails, and does not hang.
        let mut pass = accrete(&rig, 3);
        pass.open_pct = 95;
        assert!(Chain::new().then(pass).run_retrying(|| BaseContext::blank(84, 48, rig.tiles.clone(), rig.wall), RunSeed(1), 1).is_err());
    }

    #[test]
    fn one_district_and_three_both_build_and_three_put_more_than_one_name_on_the_map() {
        let rig = rig();
        let mut several = 0;
        for s in 0..40 {
            let one = build(accrete(&rig, 1), &rig, 84, 48, s).unwrap();
            assert!(one.outputs().iter::<Area>().all(|a| a.district == "wide"), "seed {s}");
            let three = build(accrete(&rig, 3), &rig, 84, 48, s).unwrap();
            let names: std::collections::BTreeSet<&str> = three.outputs().iter::<Area>().map(|a| a.district.as_str()).collect();
            several += usize::from(names.len() > 1);
        }
        assert!(several > 30, "three districts showed on {several} of 40 maps");
    }

    #[test]
    fn a_room_is_emitted_for_every_area_but_a_passage_and_lies_inside_its_area() {
        let rig = rig();
        for s in 0..40 {
            let ctx = build(accrete(&rig, 3), &rig, 84, 48, s).unwrap();
            let rooms: Vec<Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
            let areas: Vec<&Area> = ctx.outputs().iter::<Area>().collect();
            assert_eq!(rooms.len(), areas.iter().filter(|a| a.shape != "run").count(), "seed {s}");
            for a in &areas {
                assert!(a.inner.cells().all(|c| a.cells.contains(&c)), "seed {s}: area {}'s inner rectangle leaves it", a.index);
            }
        }
    }

    #[test]
    fn the_same_seed_grows_the_same_place_twice() {
        let rig = rig();
        for s in 0..20 {
            let (a, b) = (build(accrete(&rig, 3), &rig, 84, 48, s).unwrap(), build(accrete(&rig, 3), &rig, 84, 48, s).unwrap());
            assert_eq!(a.terrain(), b.terrain(), "seed {s}");
        }
    }

    /// What cannot be built is an error, never a panic and never a loop
    /// that does not end.
    #[test]
    fn a_map_too_small_and_a_pass_with_nothing_to_draw_are_errors_by_name() {
        let rig = rig();
        assert_eq!(build(accrete(&rig, 3), &rig, 6, 6, 1).unwrap_err().pass, "accrete");
        let mut none = accrete(&rig, 3);
        none.shapes.clear();
        assert_eq!(build(none, &rig, 84, 48, 1).unwrap_err().pass, "accrete");
        let mut no_districts = accrete(&rig, 3);
        no_districts.districts.clear();
        no_districts.mix.clear();
        assert_eq!(build(no_districts, &rig, 84, 48, 1).unwrap_err().pass, "accrete");
        let mut weightless = accrete(&rig, 3);
        weightless.mix = vec![(0, 0)];
        assert_eq!(build(weightless, &rig, 84, 48, 1).unwrap_err().pass, "accrete");
        let mut empty_district = accrete(&rig, 1);
        empty_district.districts[0].shapes.clear();
        assert_eq!(build(empty_district, &rig, 84, 48, 1).unwrap_err().pass, "accrete");
    }

    /// A passage holds nothing, so one that leads nowhere is taken back
    /// before the place is handed on: a dead end is always a room.
    #[test]
    fn no_passage_is_left_leading_nowhere() {
        let rig = rig();
        for (w, h, seeds) in sizes() {
            for s in seeds {
                let ctx = build(accrete(&rig, 3), &rig, w, h, s).unwrap();
                let areas: Vec<&Area> = ctx.outputs().iter::<Area>().collect();
                for (i, a) in areas.iter().enumerate() {
                    assert_eq!(a.index, i, "{w}x{h} seed {s}: areas are numbered in the order they are emitted");
                    let ways = ctx.outputs().iter::<Opening>().filter(|o| o.areas.0 == i || o.areas.1 == i).count();
                    assert!(a.shape != "run" || ways >= 2, "{w}x{h} seed {s}: passage {i} has {ways} ways out");
                }
            }
        }
    }

    /// Weights too large to add up are an error like any other list that
    /// cannot be drawn from, never a panic part-way through growth.
    #[test]
    fn weights_that_overflow_are_an_error_by_name() {
        let rig = rig();
        let mut heavy = accrete(&rig, 3);
        heavy.districts[0].shapes = vec![(0, 3_000_000_000), (1, 3_000_000_000)];
        assert_eq!(build(heavy, &rig, 84, 48, 1).expect_err("refused").pass, "accrete");
        let mut heavy_mix = accrete(&rig, 3);
        heavy_mix.mix = vec![(0, 3_000_000_000), (1, 3_000_000_000)];
        assert_eq!(build(heavy_mix, &rig, 84, 48, 1).expect_err("refused").pass, "accrete");
        let mut dense = accrete(&rig, 3);
        dense.districts_per_10k = u32::MAX;
        assert_eq!(build(dense, &rig, 84, 48, 1).expect_err("refused").pass, "accrete");
    }

    /// A mouth widens to both sides of where the pieces met before it goes
    /// two cells to one: where a three-wide mouth sits all to one side,
    /// the cell on the other side could not have been opened.
    #[test]
    fn a_mouth_is_centred_on_the_join_wherever_both_sides_allow() {
        let rig = rig();
        let mut wide = 0;
        for s in 0..120 {
            let ctx = build(accrete(&rig, 3), &rig, 84, 48, s).unwrap();
            let areas: Vec<&Area> = ctx.outputs().iter::<Area>().collect();
            for o in ctx.outputs().iter::<Opening>().filter(|o| o.cells.len() == 3) {
                wide += 1;
                let site = o.cells[0];
                for side in o.facing.perpendicular() {
                    let beside = site + side.offset();
                    if !o.cells.contains(&beside) {
                        let could =
                            areas[o.areas.0].cells.contains(&(beside - o.facing.offset())) && areas[o.areas.1].cells.contains(&(beside + o.facing.offset()));
                        assert!(!could, "seed {s}: the mouth at {site:?} left {beside:?} shut and went two the other way");
                    }
                }
            }
        }
        assert!(wide > 0, "some mouth was three wide");
    }

    fn barrier(from: (u32, u32), to: (u32, u32), chance_pct: u32, jitter_pct: u32) -> Barrier {
        Barrier { from, to, chance_pct, jitter_pct }
    }

    /// A wall down from the top edge two thirds of the way, always there
    /// and shifted a little, and a slanting one up from the bottom that is
    /// there half the time.
    fn barred(rig: &Rig) -> Accrete {
        let mut pass = accrete(rig, 3);
        pass.barriers = vec![barrier((50, 0), (50, 65), 100, 6), barrier((15, 100), (35, 50), 50, 10)];
        pass
    }

    #[test]
    fn no_piece_s_body_or_ring_takes_a_cell_of_a_barrier_that_was_laid() {
        let rig = rig();
        let mut checked = 0;
        for (w, h, seeds) in sizes() {
            for s in seeds {
                let ctx = build(barred(&rig), &rig, w, h, s).unwrap_or_else(|e| panic!("{w}x{h} seed {s}: {e}"));
                let mut rock = BitGrid::new(w, h);
                for b in ctx.outputs().iter::<LaidBarrier>() {
                    assert!(!b.cells.is_empty(), "{w}x{h} seed {s}: barrier {} was laid on the map", b.index);
                    for c in &b.cells {
                        rock.insert(*c);
                        assert!(!walkable(&ctx, *c), "{w}x{h} seed {s}: barrier cell {c:?} was opened");
                    }
                }
                for a in ctx.outputs().iter::<Area>() {
                    for c in &a.cells {
                        for (dx, dy) in (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))) {
                            assert!(!rock.contains(c.offset(dx, dy)), "{w}x{h} seed {s}: area {} at {c:?} touches a barrier", a.index);
                        }
                    }
                }
                checked += ctx.outputs().iter::<LaidBarrier>().count();
            }
        }
        assert!(checked > 72, "every build laid the first barrier and some the second: {checked}");
    }

    #[test]
    fn a_barrier_at_chance_nought_is_never_laid_at_a_hundred_always_and_at_fifty_sometimes() {
        let rig = rig();
        let mut laid = [0; 3];
        for s in 0..40 {
            let mut pass = accrete(&rig, 3);
            pass.barriers = vec![barrier((30, 0), (30, 50), 0, 5), barrier((60, 100), (60, 50), 100, 5), barrier((10, 50), (40, 50), 50, 5)];
            let ctx = build(pass, &rig, 84, 48, s).unwrap_or_else(|e| panic!("seed {s}: {e}"));
            for b in ctx.outputs().iter::<LaidBarrier>() {
                laid[b.index] += 1;
            }
        }
        assert_eq!(laid[0], 0, "chance nought");
        assert_eq!(laid[1], 40, "chance a hundred");
        assert!((5..=35).contains(&laid[2]), "chance fifty was laid on {} of 40", laid[2]);
    }

    #[test]
    fn every_open_cell_of_a_place_with_barriers_can_be_walked_to_from_every_other() {
        let rig = rig();
        for (w, h, seeds) in sizes() {
            for s in seeds {
                let ctx = build(barred(&rig), &rig, w, h, s).unwrap_or_else(|e| panic!("{w}x{h} seed {s}: {e}"));
                let tables = ctx.tiles().tables();
                let open = |i: usize| tables.walkable[ctx.terrain().get_idx(i).index()];
                let start = ctx.outputs().first::<Area>().unwrap().cells[0];
                let mut seen = BitGrid::new(w, h);
                let reached = flood(ctx.terrain().grid(), start, open, Steps::Four, &mut seen);
                let total = (0..(w * h) as usize).filter(|i| open(*i)).count();
                assert_eq!(reached, total, "{w}x{h} seed {s}");
            }
        }
    }

    /// The point of a barrier: what lies just across it is reached only
    /// round its end.
    #[test]
    fn the_walk_to_the_far_side_of_a_barrier_is_on_average_longer_than_without_it() {
        let rig = rig();
        let (w, h) = (84, 48);
        // The open cell nearest a share of the map.
        let nearest = |ctx: &BaseContext, (x, y): (u32, u32)| {
            let at = Point::new(cell_at_share(x, w), cell_at_share(y, h));
            ctx.terrain().bounds().cells().filter(|p| walkable(ctx, *p)).min_by_key(|p| euclidean_sq(*p, at)).expect("a place has open cells")
        };
        let walk = |barriers: Vec<Barrier>, s: u64| {
            let mut pass = accrete(&rig, 3);
            pass.barriers = barriers;
            let ctx = build(pass, &rig, w, h, s).unwrap_or_else(|e| panic!("seed {s}: {e}"));
            let steps = crate::walk::steps_between(w, h, |p| walkable(&ctx, p), nearest(&ctx, (30, 15)), nearest(&ctx, (70, 15)));
            assert_ne!(steps, crate::walk::UNREACHED, "seed {s}");
            u64::from(steps)
        };
        let (mut with, mut without) = (0, 0);
        for s in 0..40 {
            with += walk(vec![barrier((50, 0), (50, 75), 100, 0)], s);
            without += walk(Vec::new(), s);
        }
        assert!(with > without, "with the barrier the walks came to {with} steps, without it {without}");
    }

    #[test]
    fn a_barrier_that_may_reach_edge_to_edge_or_names_a_share_past_a_hundred_is_an_error_by_name() {
        let rig = rig();
        for b in [
            barrier((50, 0), (50, 100), 100, 0),
            barrier((0, 40), (100, 60), 100, 0),
            // Two cells short of each edge, which leaves no way round.
            barrier((3, 50), (97, 50), 100, 0),
            // Stopping short of the bottom where it is drawn, but a shift may carry it there.
            barrier((50, 0), (50, 92), 100, 10),
            barrier((50, 0), (50, 50), 101, 0),
            barrier((50, 0), (50, 150), 100, 0),
        ] {
            let mut pass = accrete(&rig, 3);
            pass.barriers = vec![b];
            assert_eq!(build(pass, &rig, 84, 48, 1).expect_err("refused").pass, "accrete", "{b:?}");
        }
        // The same short of the bottom with no shift, or only cutting off a corner: allowed.
        for b in [barrier((50, 0), (50, 92), 100, 0), barrier((0, 30), (30, 0), 100, 0)] {
            assert!(!b.may_cut(84, 48), "{b:?}");
        }
    }

    #[test]
    fn a_share_falls_on_the_first_cell_at_nought_and_the_last_at_a_hundred() {
        for side in [3, 48, 84, 4096] {
            assert_eq!(cell_at_share(0, side), 0);
            assert_eq!(cell_at_share(100, side), side - 1);
            assert_eq!(cell_at_share(50, side), (side - 1 + 1) / 2);
        }
    }
}
