//! Passes that build a bounded map: rooms, partitions, doors, ways in and
//! out.
//!
//! Every tile is a parameter, as in [`passes`](crate::passes). Rooms are
//! published as [`Room`] outputs so later passes (doors, spawns, a
//! prefab wanting a room to sit in) can find them, and the ways in and out
//! as [`StartPoint`] and [`ExitPoint`].

use rand::Rng;
use rl_core::{Direction, Grid2D, Point, Rect};
use rl_grid::{BitGrid, CostSource, DijkstraMap, PathRules, Terrain, TileId, TileTables};

use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::lakes::Bridge;
pub use crate::passes::StartPoint;
use crate::prefab::Stamped;

/// A room a pass carved, emitted once per room in placement order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room(pub Rect);

/// Where a map's exit should stand, emitted by [`FarthestExit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitPoint(pub Point);

/// Carves an L-shaped corridor of `floor` between two points.
fn corridor(ctx: &mut impl BuildContext, from: Point, to: Point, floor: TileId, horizontal_first: bool) {
    let mut cells = vec![from];
    let (mut x, mut y) = (from.x, from.y);
    let run_x = |x: &mut i32, y: i32, cells: &mut Vec<Point>| {
        while *x != to.x {
            *x += (to.x - *x).signum();
            cells.push(Point::new(*x, y));
        }
    };
    let run_y = |y: &mut i32, x: i32, cells: &mut Vec<Point>| {
        while *y != to.y {
            *y += (to.y - *y).signum();
            cells.push(Point::new(x, *y));
        }
    };
    if horizontal_first {
        run_x(&mut x, y, &mut cells);
        run_y(&mut y, x, &mut cells);
    } else {
        run_y(&mut y, x, &mut cells);
        run_x(&mut x, y, &mut cells);
    }
    for p in cells {
        ctx.terrain_mut().set(p, floor);
    }
}

/// Scatters non-overlapping rectangular rooms and joins each to the one
/// placed before it with an L-shaped corridor.
///
/// Fails if fewer than `min_rooms` fit.
#[derive(Debug, Clone, Copy)]
pub struct Rooms {
    /// The tile rooms and corridors are carved from.
    pub floor: TileId,
    /// How many placements to try. More tries, denser map.
    pub attempts: u32,
    /// Smallest room side, interior cells.
    pub min_size: i32,
    /// Largest room side, interior cells.
    pub max_size: i32,
    /// Fail with fewer rooms than this.
    pub min_rooms: usize,
}

impl Default for Rooms {
    fn default() -> Self {
        Self { floor: TileId(1), attempts: 30, min_size: 4, max_size: 10, min_rooms: 3 }
    }
}

impl<C: BuildContext> Pass<C> for Rooms {
    fn name(&self) -> &'static str {
        "rooms"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let bounds = ctx.terrain().bounds();
        let mut rooms: Vec<Rect> = Vec::new();
        for _ in 0..self.attempts {
            let w = ctx.rng().random_range(self.min_size..=self.max_size);
            let h = ctx.rng().random_range(self.min_size..=self.max_size);
            if w + 2 > bounds.width || h + 2 > bounds.height {
                continue;
            }
            let x = ctx.rng().random_range(1..bounds.width - w - 1 + 1);
            let y = ctx.rng().random_range(1..bounds.height - h - 1 + 1);
            let room = Rect::new(x, y, w, h);
            if rooms.iter().any(|r| r.too_close(&room, 1)) {
                continue;
            }
            for p in room.cells() {
                ctx.terrain_mut().set(p, self.floor);
            }
            if let Some(prev) = rooms.last() {
                let horizontal_first = ctx.rng().random_bool(0.5);
                corridor(ctx, prev.center(), room.center(), self.floor, horizontal_first);
            }
            rooms.push(room);
        }
        if rooms.len() < self.min_rooms {
            return Err(BuildError::new("rooms", format!("only {} of {} rooms fit", rooms.len(), self.min_rooms)));
        }
        for r in rooms {
            ctx.emit(Room(r));
        }
        Ok(())
    }
}

/// Splits the map into a binary tree of leaves, carves one room in each,
/// and joins sibling leaves with a corridor, so every room is reachable
/// and the layout fills the map evenly.
#[derive(Debug, Clone, Copy)]
pub struct Bsp {
    /// The tile rooms and corridors are carved from.
    pub floor: TileId,
    /// A leaf smaller than this on either side is not split further.
    pub min_leaf: i32,
    /// Cells of wall kept between a room and its leaf's edge.
    pub padding: i32,
}

impl Default for Bsp {
    fn default() -> Self {
        Self { floor: TileId(1), min_leaf: 8, padding: 1 }
    }
}

impl Bsp {
    fn split(&self, ctx: &mut impl BuildContext, leaf: Rect, depth: u32) -> Option<Rect> {
        let can_split_w = leaf.width >= 2 * self.min_leaf;
        let can_split_h = leaf.height >= 2 * self.min_leaf;
        if depth > 0 && (can_split_w || can_split_h) {
            let vertical = if can_split_w && can_split_h { ctx.rng().random_bool(0.5) } else { can_split_w };
            let (a, b) = if vertical {
                let cut = ctx.rng().random_range(self.min_leaf..=leaf.width - self.min_leaf);
                (Rect::new(leaf.x, leaf.y, cut, leaf.height), Rect::new(leaf.x + cut, leaf.y, leaf.width - cut, leaf.height))
            } else {
                let cut = ctx.rng().random_range(self.min_leaf..=leaf.height - self.min_leaf);
                (Rect::new(leaf.x, leaf.y, leaf.width, cut), Rect::new(leaf.x, leaf.y + cut, leaf.width, leaf.height - cut))
            };
            let ra = self.split(ctx, a, depth - 1);
            let rb = self.split(ctx, b, depth - 1);
            if let (Some(ra), Some(rb)) = (ra, rb) {
                let horizontal_first = ctx.rng().random_bool(0.5);
                corridor(ctx, ra.center(), rb.center(), self.floor, horizontal_first);
            }
            return ra.or(rb);
        }
        // A leaf: a room inset from its edges, at least 2x2.
        let inner = Rect::new(leaf.x + self.padding, leaf.y + self.padding, leaf.width - 2 * self.padding, leaf.height - 2 * self.padding);
        if inner.width < 2 || inner.height < 2 {
            return None;
        }
        let w = ctx.rng().random_range(2..=inner.width);
        let h = ctx.rng().random_range(2..=inner.height);
        let x = ctx.rng().random_range(inner.x..=inner.right() - w);
        let y = ctx.rng().random_range(inner.y..=inner.bottom() - h);
        let room = Rect::new(x, y, w, h);
        for p in room.cells() {
            ctx.terrain_mut().set(p, self.floor);
        }
        ctx.emit(Room(room));
        Some(room)
    }
}

impl<C: BuildContext> Pass<C> for Bsp {
    fn name(&self) -> &'static str {
        "bsp"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let bounds = ctx.terrain().bounds();
        let area = Rect::new(1, 1, bounds.width - 2, bounds.height - 2);
        if self.split(ctx, area, 8).is_none() {
            return Err(BuildError::new("bsp", "no leaf could hold a room"));
        }
        Ok(())
    }
}

/// Puts `door` where a corridor meets a room: on the ring just outside
/// each emitted [`Room`], every open cell whose ring neighbours are both
/// closed.
#[derive(Debug, Clone, Copy)]
pub struct Doors {
    /// The door tile.
    pub door: TileId,
}

impl<C: BuildContext> Pass<C> for Doors {
    fn name(&self) -> &'static str {
        "doors"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        let rooms: Vec<Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
        let mut doors = Vec::new();
        for room in rooms {
            let ring = room.inflate(1);
            let open = |p: Point| ctx.terrain().get(p).is_some_and(|t| tables.walkable[t.index()]);
            for p in ring.border() {
                let corner = (p.x == ring.x || p.x == ring.right() - 1) && (p.y == ring.y || p.y == ring.bottom() - 1);
                if corner || !open(p) {
                    continue;
                }
                let along = if p.y == ring.y || p.y == ring.bottom() - 1 { [Direction::West, Direction::East] } else { [Direction::North, Direction::South] };
                if along.iter().all(|d| !open(p + d.offset())) {
                    doors.push(p);
                }
            }
        }
        for p in doors {
            ctx.terrain_mut().set(p, self.door);
        }
        Ok(())
    }
}

/// Emits a random open cell as the [`StartPoint`]; inside the first
/// emitted [`Room`] when there is one, and never on a [`Bridge`]. Fails if
/// there is no open cell.
///
/// [`clear_of_stamps`](Self::clear_of_stamps) keeps it away from the
/// pieces stamped before it.
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomStart;

impl RandomStart {
    /// This start, kept at least `cells` cells, Chebyshev, from the bounds
    /// of every [`Stamped`] a pass emitted before it.
    ///
    /// Opt-in, so a chain that never asks draws exactly the start it
    /// always has.
    pub fn clear_of_stamps(self, cells: i32) -> ClearStart {
        ClearStart { cells }
    }
}

/// Every cell a [`Bridge`] was laid on. Neither way in nor way out
/// stands on one: a bridge is a way over a lake, and a [`Room`] a lake
/// was laid in still names the cells the bridge crosses.
fn bridged<C: BuildContext>(ctx: &C) -> BitGrid {
    let mut cells = BitGrid::new(ctx.terrain().width(), ctx.terrain().height());
    for p in ctx.outputs().iter::<Bridge>().flat_map(|b| b.cells.iter()) {
        cells.insert(*p);
    }
    cells
}

/// The open cells of `area`, never one a bridge was laid on.
fn open_cells<C: BuildContext>(ctx: &C, area: Rect) -> Vec<Point> {
    let tables = ctx.tiles().tables();
    let bridged = bridged(ctx);
    area.cells().filter(|p| ctx.terrain().get(*p).is_some_and(|t| tables.walkable[t.index()]) && !bridged.contains(*p)).collect()
}

/// The cells [`RandomStart`] draws from: the first room's open cells, or
/// the whole map's when no room was emitted.
fn start_cells<C: BuildContext>(ctx: &C) -> Vec<Point> {
    let area = ctx.outputs().first::<Room>().map(|r| r.0).unwrap_or_else(|| ctx.terrain().bounds());
    open_cells(ctx, area)
}

/// One of `cells`, drawn from the chain's stream for the start, as the
/// [`StartPoint`].
fn emit_start<C: BuildContext>(ctx: &mut C, cells: &[Point]) -> Result<(), BuildError> {
    if cells.is_empty() {
        return Err(BuildError::new("random_start", "no open cell"));
    }
    let p = cells[ctx.rng().random_range(0..cells.len())];
    ctx.emit(StartPoint(p));
    Ok(())
}

impl<C: BuildContext> Pass<C> for RandomStart {
    fn name(&self) -> &'static str {
        "random_start"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let cells = start_cells(ctx);
        emit_start(ctx, &cells)
    }
}

/// A [`RandomStart`] kept at least `cells` cells from every piece stamped
/// before it, made by [`RandomStart::clear_of_stamps`].
///
/// A piece that places what fights, stamped in the room the start is
/// drawn from, would stand it beside the arrival. So the start is drawn
/// from the first room, in the order they were emitted, with an open cell
/// that clear of every [`Stamped`] bounds, and stands in a room as the
/// plain start does. Where no cell anywhere is that clear, it is
/// [`RandomStart`]'s own pick, from the same stream: a start closer than
/// a game hoped is a worse map, and a chain that failed over it would be
/// no map at all.
#[derive(Debug, Clone, Copy)]
pub struct ClearStart {
    cells: i32,
}

impl<C: BuildContext> Pass<C> for ClearStart {
    // The plain start's name, so both draw from one stream and the
    // fallback is exactly the plain pick.
    fn name(&self) -> &'static str {
        "random_start"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let stamps: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
        let clear = |p: &Point| stamps.iter().all(|r| rl_core::geometry::chebyshev(*p, rl_core::geometry::clamp_to(*p, *r)) >= self.cells);
        let mut areas: Vec<Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
        if areas.is_empty() {
            areas.push(ctx.terrain().bounds());
        }
        let cells = areas.into_iter().map(|area| open_cells(ctx, area).into_iter().filter(clear).collect::<Vec<_>>()).find(|cells| !cells.is_empty());
        let cells = cells.unwrap_or_else(|| start_cells(ctx));
        emit_start(ctx, &cells)
    }
}

/// Floods as a walker who opens doors does: a cell counts as open when the
/// registry marks it passable, even where it is not walkable this moment,
/// because a corridor through a shut door still joins the rooms either
/// side of it. Only [`FarthestExit`] and the passes that must agree with
/// it, through [`walk_through_doors`], read terrain this way; everything
/// that moves a body during play still asks a
/// [`TerrainView`](rl_grid::TerrainView), which stops at a shut door as it
/// should.
struct ThroughDoors<'a> {
    terrain: &'a Terrain,
    tables: &'a TileTables,
    /// Every cell of an opening joined locked or sealed, which no walker
    /// gets through, whatever tile it was laid as.
    shut: BitGrid,
}

impl Grid2D for ThroughDoors<'_> {
    fn width(&self) -> i32 {
        self.terrain.width()
    }
    fn height(&self) -> i32 {
        self.terrain.height()
    }
}

impl CostSource for ThroughDoors<'_> {
    fn cost_idx(&self, idx: usize) -> Option<u32> {
        let id = self.terrain.get_idx(idx).index();
        (self.tables.passable[id] && !self.shut.contains(self.idx_point(idx))).then(|| self.tables.move_cost[id])
    }
}

/// The walk [`FarthestExit`] draws the way down by, from `from`: doors
/// opened on the way, and, with `locks_shut`, never through an
/// [`Opening`](crate::accrete::Opening) joined locked or sealed. Without
/// it every lock and seal is taken as opened, for a pass asking what is
/// reached once nothing is shut. A pass that must not change what the
/// exit was drawn by asks this, rather than a walk of its own that could
/// disagree with it about a door.
pub(crate) fn walk_through_doors<C: BuildContext>(ctx: &C, from: Point, locks_shut: bool) -> DijkstraMap {
    let tables = ctx.tiles().tables();
    let mut shut = BitGrid::new(ctx.terrain().width(), ctx.terrain().height());
    let locked = |o: &&crate::accrete::Opening| matches!(o.join, crate::accrete::Join::Locked(_) | crate::accrete::Join::Sealed);
    for o in ctx.outputs().iter::<crate::accrete::Opening>().filter(|o| locks_shut && locked(o)) {
        for c in &o.cells {
            shut.insert(*c);
        }
    }
    let through_doors = ThroughDoors { terrain: ctx.terrain(), tables: &tables, shut };
    let mut map = DijkstraMap::covering(&through_doors);
    map.build(&through_doors, [from], PathRules::default());
    map
}

/// Emits the cell farthest by walking from the [`StartPoint`] as the
/// [`ExitPoint`], doors opened on the way: a shut door is passable even
/// where it is not walkable, and a map of rooms joined only through doors
/// otherwise measures no farther than the doors round the room the start
/// stands in. Never a door or a bridge itself, since the exit is somewhere to stand
/// and not a cell nothing has opened yet, and never past an
/// [`Opening`](crate::accrete::Opening) joined locked or sealed, which no
/// walker gets through however its tile is drawn. Ties break toward the cell first
/// in row order, the lower point by [`Point`]'s own ordering, so a seed
/// always picks the same one. Fails without a start or with nowhere to
/// stand that is farther than it.
///
/// [`clear_of_stamps`](Self::clear_of_stamps) keeps it out of and away
/// from the pieces stamped before it, and [`among_farthest`
/// ](Self::among_farthest) draws it from the farthest share rather than
/// taking the one farthest cell.
#[derive(Debug, Clone, Copy, Default)]
pub struct FarthestExit;

impl FarthestExit {
    /// This exit, kept at least `cells` cells, Chebyshev, from the bounds
    /// of every [`Stamped`] a pass emitted before it.
    ///
    /// Opt-in, as [`RandomStart::clear_of_stamps`] is, so a chain that
    /// never asks draws exactly the exit it always has.
    pub fn clear_of_stamps(self, cells: i32) -> FarExit {
        FarExit::default().clear_of_stamps(cells)
    }

    /// This exit drawn from every cell whose walk from the start is
    /// within the farthest `pct` in a hundred of the longest walk.
    pub fn among_farthest(self, pct: u32) -> FarExit {
        FarExit::default().among_farthest(pct)
    }
}

/// A [`FarthestExit`] kept clear of stamps, drawn from the farthest share,
/// or both, made by [`FarthestExit::clear_of_stamps`] and
/// [`FarthestExit::among_farthest`].
///
/// Clear of stamps, so a piece holding what fights, or a vault reached
/// through a door, never holds the way down. Where no cell is that clear,
/// it is drawn from the cells outside every stamp, so it never lands in
/// one; a place with nowhere to stand outside one fails. Among the
/// farthest, so one place does not always put its way down in the same
/// corner: the longest walk is measured to where it may stand, and the
/// exit is drawn from the chain's stream among every such cell whose walk
/// is at least `100 - pct` in a hundred of it.
#[derive(Debug, Clone, Copy, Default)]
pub struct FarExit {
    clear: i32,
    among_pct: Option<u32>,
}

impl FarExit {
    /// Kept at least `cells` cells from every stamp, as
    /// [`FarthestExit::clear_of_stamps`].
    pub fn clear_of_stamps(mut self, cells: i32) -> Self {
        self.clear = cells;
        self
    }

    /// Drawn from the farthest `pct` in a hundred, as
    /// [`FarthestExit::among_farthest`].
    pub fn among_farthest(mut self, pct: u32) -> Self {
        self.among_pct = Some(pct);
        self
    }
}

impl<C: BuildContext> Pass<C> for FarthestExit {
    fn name(&self) -> &'static str {
        "farthest_exit"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        FarExit::default().apply(ctx)
    }
}

impl<C: BuildContext> Pass<C> for FarExit {
    // The plain exit's name, so both draw from one stream.
    fn name(&self) -> &'static str {
        "farthest_exit"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let err = |reason: String| BuildError::new("farthest_exit", reason);
        if self.among_pct.is_some_and(|pct| pct > 100) {
            return Err(err(format!("the farthest {}% is a share past a hundred", self.among_pct.unwrap_or(0))));
        }
        let start = ctx.outputs().first::<StartPoint>().ok_or_else(|| err("no start point emitted before it".into()))?.0;
        let tables = ctx.tiles().tables();
        let map = walk_through_doors(ctx, start, true);
        let bridged = bridged(ctx);
        let walkable = |p: Point| ctx.terrain().get(p).is_some_and(|t| tables.walkable[t.index()]) && !bridged.contains(p);
        let stamps: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
        let away = |p: Point| stamps.iter().map(|r| rl_core::geometry::chebyshev(p, rl_core::geometry::clamp_to(p, *r))).min().unwrap_or(i32::MAX);
        let standing: Vec<(Point, i32)> = map.iter().filter(|(p, _)| *p != start && walkable(*p)).collect();
        let clear: Vec<(Point, i32)> = standing.iter().copied().filter(|(p, _)| away(*p) >= self.clear).collect();
        let kept = self.clear > 0;
        let cells = if clear.is_empty() && kept { standing.into_iter().filter(|(p, _)| away(*p) >= 1).collect() } else { clear };
        let far = cells
            .iter()
            .map(|(_, v)| *v)
            .max()
            .ok_or_else(|| err(format!("nowhere to stand farther than the start{}", if kept { " and outside every stamp" } else { "" })))?;
        let at = match self.among_pct {
            None => cells.iter().max_by_key(|(p, v)| (*v, std::cmp::Reverse(*p))).expect("some cell is the farthest").0,
            Some(pct) => {
                let within: Vec<Point> = cells.iter().filter(|(_, v)| i64::from(*v) * 100 >= i64::from(far) * i64::from(100 - pct)).map(|(p, _)| *p).collect();
                within[ctx.rng().random_range(0..within.len())]
            }
        };
        ctx.emit(ExitPoint(at));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use rl_core::{RunSeed, Steps};
    use rl_grid::{TileProps, TileRegistry};

    fn ctx() -> (BaseContext, TileId, TileId, TileId) {
        let mut tiles = TileRegistry::standard();
        let door = tiles.register(TileProps::floor("door").opaque(true)).unwrap();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        (BaseContext::blank(60, 40, tiles, wall), wall, floor, door)
    }

    fn connected(c: &BaseContext) -> bool {
        let tables = c.tiles().tables();
        let regions = rl_grid::region::label_regions(c.terrain(), |i| tables.walkable[c.terrain().get_idx(i).index()], Steps::Eight);
        regions.count() == 1
    }

    #[test]
    fn rooms_are_disjoint_connected_and_have_doors_and_a_far_exit() {
        for seed in 1..=6 {
            let (mut c, wall, floor, door) = ctx();
            Chain::new()
                .then(Rooms { floor, ..Default::default() })
                .then(Doors { door })
                .then(RandomStart)
                .then(FarthestExit)
                .run(&mut c, RunSeed(seed))
                .unwrap();
            let rooms: Vec<Rect> = c.outputs().iter::<Room>().map(|r| r.0).collect();
            assert!(rooms.len() >= 3, "seed {seed}");
            for (i, a) in rooms.iter().enumerate() {
                for b in &rooms[i + 1..] {
                    assert!(!a.too_close(b, 1), "seed {seed}: rooms touch");
                }
                assert!(a.cells().all(|p| c.terrain().get(p) == Some(floor)));
            }
            assert!(connected(&c), "seed {seed}");
            assert!(c.terrain().count(door) >= 2, "seed {seed}: doors {}", c.terrain().count(door));
            let start = c.outputs().first::<StartPoint>().unwrap().0;
            let exit = c.outputs().first::<ExitPoint>().unwrap().0;
            assert!(rooms[0].contains(start));
            assert_ne!(start, exit);
            assert_ne!(c.terrain().get(exit), Some(wall));
            assert!(rl_core::geometry::chebyshev(start, exit) > 10, "seed {seed}: exit too near");
        }
    }

    #[test]
    fn bsp_fills_the_map_evenly_and_connects() {
        for seed in 1..=6 {
            let (mut c, _, floor, _) = ctx();
            Chain::new().then(Bsp { floor, ..Default::default() }).then(RandomStart).run(&mut c, RunSeed(seed)).unwrap();
            let rooms: Vec<Rect> = c.outputs().iter::<Room>().map(|r| r.0).collect();
            assert!(rooms.len() >= 8, "seed {seed}: {} rooms", rooms.len());
            assert!(connected(&c), "seed {seed}");
            let left = rooms.iter().filter(|r| r.center().x < 30).count();
            assert!(left >= 2 && left <= rooms.len() - 2, "seed {seed}: rooms all on one side");
            assert!(c.terrain().bounds().border().all(|p| c.terrain().get(p) != Some(floor)), "the edge is kept");
        }
    }

    /// Rooms with a three-by-three piece stamped in one of them, and then
    /// `start`, on `seed`: the start and the piece's bounds.
    fn start_beside_a_stamp(start: impl Pass<BaseContext> + 'static, seed: u64) -> (Point, Rect, Vec<Rect>) {
        use crate::prefab::{Orient, Placement, Prefab, StampPrefab, Stamped};
        let (mut c, _, floor, _) = ctx();
        let piece = Prefab::parse(&["...", ".m.", "..."], |ch| (ch == '.').then_some(floor)).unwrap();
        Chain::new()
            .then(Rooms { floor, ..Default::default() })
            .then(StampPrefab { name: "piece", prefab: piece, at: Placement::AnyRoom, orient: Orient::Fixed })
            .then(start)
            .run(&mut c, RunSeed(seed))
            .unwrap();
        let start = c.outputs().first::<StartPoint>().unwrap().0;
        let stamp = c.outputs().first::<Stamped>().unwrap().bounds;
        (start, stamp, c.outputs().iter::<Room>().map(|r| r.0).collect())
    }

    /// How far `p` stands from the nearest cell of `r`, nought inside it.
    fn clearance(p: Point, r: Rect) -> i32 {
        rl_core::geometry::chebyshev(p, rl_core::geometry::clamp_to(p, r))
    }

    /// A start kept clear of stamps stands at least that far from every
    /// piece stamped before it, and still in a room, over a span of seeds
    /// on which the plain start sometimes lands beside the piece.
    #[test]
    fn a_start_kept_clear_of_stamps_stands_that_far_from_every_one_and_in_a_room() {
        let mut close = 0;
        for seed in 1..=60 {
            let (start, stamp, rooms) = start_beside_a_stamp(RandomStart.clear_of_stamps(6), seed);
            assert!(clearance(start, stamp) >= 6, "seed {seed}: {start:?} is {} from {stamp:?}", clearance(start, stamp));
            assert!(rooms.iter().any(|r| r.contains(start)), "seed {seed}: {start:?} is in no room");
            close += usize::from(clearance(start_beside_a_stamp(RandomStart, seed).0, stamp) < 6);
        }
        assert!(close > 0, "the plain start never came near the piece, so this proves nothing");
    }

    /// With no cell that clear anywhere, the start is the plain one, from
    /// the same stream, rather than a chain that fails.
    #[test]
    fn a_start_with_nowhere_clear_enough_falls_back_to_the_plain_pick() {
        for seed in 1..=10 {
            let (clear, ..) = start_beside_a_stamp(RandomStart.clear_of_stamps(100), seed);
            let (plain, ..) = start_beside_a_stamp(RandomStart, seed);
            assert_eq!(clear, plain, "seed {seed}");
        }
    }

    #[test]
    fn a_door_sits_in_a_one_wide_gap_only() {
        let (mut c, _, floor, door) = ctx();
        // A room with a corridor leaving north and a wide opening east.
        let room = Rect::new(10, 10, 5, 5);
        for p in room.cells() {
            c.terrain_mut().set(p, floor);
        }
        c.terrain_mut().set(Point::new(12, 9), floor);
        c.terrain_mut().set(Point::new(12, 8), floor);
        for y in 10..15 {
            c.terrain_mut().set(Point::new(15, y), floor);
        }
        c.emit(Room(room));
        Chain::new().then(Doors { door }).run(&mut c, RunSeed(1)).unwrap();
        assert_eq!(c.terrain().get(Point::new(12, 9)), Some(door));
        assert_eq!(c.terrain().count(door), 1, "the wide opening got no door");
    }

    /// Two rooms joined only by a shut door: the door is passable but not
    /// walkable, so a flood that stopped at `walkable` would end in the
    /// near room, where the start stands. The farthest exit crosses the
    /// door into the room beyond it, and never stands on the door itself.
    #[test]
    fn the_farthest_exit_crosses_a_shut_door_to_reach_the_room_beyond_it() {
        let tiles = TileRegistry::standard();
        let (wall, floor, door) = (tiles.expect("wall"), tiles.expect("floor"), tiles.expect("door_closed"));
        let mut c = BaseContext::blank(20, 7, tiles, wall);
        let near = Rect::new(1, 1, 4, 4);
        let far_room = Rect::new(10, 1, 4, 4);
        for p in near.cells().chain(far_room.cells()) {
            c.terrain_mut().set(p, floor);
        }
        for x in 5..10 {
            c.terrain_mut().set(Point::new(x, 2), floor);
        }
        c.terrain_mut().set(Point::new(6, 2), door);
        c.emit(StartPoint(Point::new(2, 2)));
        Chain::new().then(FarthestExit).run(&mut c, RunSeed(1)).unwrap();
        let exit = c.outputs().first::<ExitPoint>().unwrap().0;
        assert!(far_room.contains(exit), "exit {exit:?} should cross the shut door into the far room {far_room:?}");
        assert_ne!(c.terrain().get(exit), Some(door), "the exit should be somewhere to stand, not the door itself");
    }

    /// A straight corridor with the start in its middle: both ends are
    /// equally far, and the tie breaks toward the cell first in row
    /// order, the lower point by `Point`'s own ordering, so a seed always
    /// picks the same one of a tie rather than whichever an iterator
    /// happens to visit last.
    #[test]
    fn a_tied_farthest_exit_breaks_toward_the_cell_first_in_row_order() {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut c = BaseContext::blank(11, 3, tiles, wall);
        for x in 0..11 {
            c.terrain_mut().set(Point::new(x, 1), floor);
        }
        c.emit(StartPoint(Point::new(5, 1)));
        Chain::new().then(FarthestExit).run(&mut c, RunSeed(1)).unwrap();
        let exit = c.outputs().first::<ExitPoint>().unwrap().0;
        assert_eq!(exit, Point::new(0, 1), "a tie should break toward the lower point, not the higher");
    }

    /// A grown place owing a hall painted from a prefab, its way in drawn,
    /// and its way out by `exit`: the exit and the hall's stamp.
    fn exit_beside_a_grown_piece(exit: impl Pass<BaseContext> + 'static, seed: u64) -> (Point, Rect) {
        use crate::accrete::tests::{owing, rig};
        let rig = rig();
        let chain = Chain::new().then(owing(&rig, crate::Join::Door)).then(RandomStart).then(exit);
        let c =
            chain.run_retrying(|| BaseContext::blank(84, 48, rig.tiles.clone(), rig.wall), RunSeed(seed), 20).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        (c.outputs().first::<ExitPoint>().unwrap().0, c.outputs().first::<Stamped>().unwrap().bounds)
    }

    /// Over a span of grown places, each holding a hall painted from a
    /// prefab, the exit kept clear of stamps stands as far off as it was
    /// asked.
    #[test]
    fn an_exit_kept_clear_of_stamps_never_lands_inside_one() {
        for seed in 0..40 {
            let (exit, hall) = exit_beside_a_grown_piece(FarthestExit.clear_of_stamps(3), seed);
            assert!(clearance(exit, hall) >= 3, "seed {seed}: {exit:?} is {} from {hall:?}", clearance(exit, hall));
        }
    }

    /// The exit `exit` draws on a corridor from (1, 2) to (28, 2) with a
    /// stamp over its last five cells.
    fn exit_on_a_corridor(exit: impl Pass<BaseContext> + 'static) -> Point {
        let (mut c, _, floor, _) = ctx();
        for x in 1..29 {
            c.terrain_mut().set(Point::new(x, 2), floor);
        }
        c.emit(StartPoint(Point::new(1, 2)));
        c.emit(Stamped { bounds: Rect::new(24, 1, 5, 3), marks: Vec::new(), prefab: None });
        Chain::new().then(exit).run(&mut c, RunSeed(1)).unwrap();
        c.outputs().first::<ExitPoint>().unwrap().0
    }

    /// A corridor with a stamp over its far end: the plain exit lands in
    /// the stamp, one kept three clear stands three short of it, and one
    /// asked to stand farther off than the corridor allows still stands
    /// outside it, on the nearest cell to the stamp it may.
    #[test]
    fn a_clear_exit_stands_short_of_a_stamp_over_the_far_end_and_outside_it_when_nothing_is_that_clear() {
        assert_eq!(exit_on_a_corridor(FarthestExit), Point::new(28, 2), "the plain exit is in the stamp");
        assert_eq!(exit_on_a_corridor(FarthestExit.clear_of_stamps(3)), Point::new(21, 2));
        assert_eq!(exit_on_a_corridor(FarthestExit.clear_of_stamps(100)), Point::new(23, 2));
    }

    /// Drawn from the farthest third of one place, the exit falls on many
    /// cells over a span of the exit's own seeds, each at least two thirds
    /// as far on foot as the farthest.
    #[test]
    fn among_the_farthest_third_the_exit_lands_on_different_cells_each_within_the_farthest_third() {
        let (mut c, _, floor, door) = ctx();
        Chain::new().then(Rooms { floor, ..Default::default() }).then(Doors { door }).then(RandomStart).run(&mut c, RunSeed(4)).unwrap();
        let start = c.outputs().first::<StartPoint>().unwrap().0;
        let tables = c.tiles().tables();
        let through = ThroughDoors { terrain: c.terrain(), tables: &tables, shut: BitGrid::new(60, 40) };
        let mut walk = DijkstraMap::covering(&through);
        walk.build(&through, [start], PathRules::default());
        let far = walk.iter().filter(|(p, _)| tables.walkable[c.terrain().get(*p).unwrap().index()]).map(|(_, v)| v).max().unwrap();
        let mut landed = std::collections::BTreeSet::new();
        for seed in 0..40 {
            let mut drawn = BaseContext::blank(60, 40, c.tiles().clone(), c.tiles().expect("wall"));
            *drawn.terrain_mut() = c.terrain().clone();
            drawn.emit(StartPoint(start));
            Chain::new().then(FarthestExit.among_farthest(33)).run(&mut drawn, RunSeed(seed)).unwrap();
            let exit = drawn.outputs().first::<ExitPoint>().unwrap().0;
            let steps = walk.value(exit).expect("the exit was reached");
            assert!(steps * 100 >= far * 67, "seed {seed}: {exit:?} is {steps} of the farthest {far}");
            assert!(tables.walkable[c.terrain().get(exit).unwrap().index()] && exit != start, "seed {seed}");
            landed.insert(exit);
        }
        assert!(landed.len() >= 10, "the exit fell on only {} cells", landed.len());
    }

    #[test]
    fn a_farthest_share_past_a_hundred_is_an_error_by_name() {
        let (mut c, _, floor, _) = ctx();
        let e = Chain::new()
            .then(Rooms { floor, ..Default::default() })
            .then(RandomStart)
            .then(FarthestExit.among_farthest(101))
            .run(&mut c, RunSeed(1))
            .unwrap_err();
        assert_eq!(e.pass, "farthest_exit");
    }

    /// A corridor shut half way along by a door of the passable kind: a
    /// plain door is walked through, one joined locked or sealed is not,
    /// so the way down stands short of it.
    #[test]
    fn the_farthest_exit_never_crosses_an_opening_joined_locked_or_sealed() {
        for (join, exit) in [(crate::Join::Door, Point::new(28, 2)), (crate::Join::Locked(5), Point::new(19, 2)), (crate::Join::Sealed, Point::new(19, 2))] {
            let tiles = TileRegistry::standard();
            let (wall, floor, door) = (tiles.expect("wall"), tiles.expect("floor"), tiles.expect("door_closed"));
            let mut c = BaseContext::blank(30, 5, tiles, wall);
            for x in 1..29 {
                c.terrain_mut().set(Point::new(x, 2), floor);
            }
            c.terrain_mut().set(Point::new(20, 2), door);
            c.emit(crate::accrete::Opening { cells: vec![Point::new(20, 2)], facing: Direction::East, areas: (0, 1), join });
            c.emit(StartPoint(Point::new(1, 2)));
            Chain::new().then(FarthestExit).run(&mut c, RunSeed(1)).unwrap();
            assert_eq!(c.outputs().first::<ExitPoint>().unwrap().0, exit, "{join:?}");
        }
    }
}
