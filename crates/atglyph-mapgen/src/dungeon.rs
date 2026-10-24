//! Passes that build a bounded map: rooms, partitions, doors, ways in and
//! out.
//!
//! Every tile is a parameter, as in [`passes`](crate::passes). Rooms are
//! published as [`Room`] outputs so later passes (doors, spawns, a
//! prefab wanting a room to sit in) can find them, and the ways in and out
//! as [`StartPoint`] and [`ExitPoint`].
//!
//! Where the ways in and out stand round a place's [`Landmark`] is
//! [`Stairs`], three configurations of these same passes rather than a
//! system beside them: free, [`beyond`](RandomStart::beyond) it on opposite
//! sides, or the way down [`within`](MarkedExit) it, on a cell its drawing
//! marks.

use atglyph_core::{Direction, Grid2D, GridRect, Point};
use atglyph_grid::{BitGrid, CostSource, DijkstraMap, PathRules, Terrain, TileId};
use rand::Rng;

use crate::accrete::{Landmark, Side};
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BaseContext;
use crate::lakes::Bridge;
pub use crate::passes::StartPoint;
use crate::prefab::Stamped;

/// A room a pass carved, emitted once per room in placement order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room(pub GridRect);

/// Where a map's exit should stand, emitted by [`FarthestExit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitPoint(pub Point);

/// Carves an L-shaped corridor of `floor` between two points.
fn corridor(ctx: &mut BaseContext, from: Point, to: Point, floor: TileId, horizontal_first: bool) {
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

impl Pass for Rooms {
    fn name(&self) -> &'static str {
        "rooms"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let bounds = ctx.terrain().bounds();
        let mut rooms: Vec<GridRect> = Vec::new();
        for _ in 0..self.attempts {
            let w = ctx.rng().random_range(self.min_size..=self.max_size);
            let h = ctx.rng().random_range(self.min_size..=self.max_size);
            if w + 2 > bounds.width || h + 2 > bounds.height {
                continue;
            }
            let x = ctx.rng().random_range(1..bounds.width - w - 1 + 1);
            let y = ctx.rng().random_range(1..bounds.height - h - 1 + 1);
            let room = GridRect::new(x, y, w, h);
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
    fn split(&self, ctx: &mut BaseContext, leaf: GridRect, depth: u32) -> Option<GridRect> {
        let can_split_w = leaf.width >= 2 * self.min_leaf;
        let can_split_h = leaf.height >= 2 * self.min_leaf;
        if depth > 0 && (can_split_w || can_split_h) {
            let vertical = if can_split_w && can_split_h { ctx.rng().random_bool(0.5) } else { can_split_w };
            let (a, b) = if vertical {
                let cut = ctx.rng().random_range(self.min_leaf..=leaf.width - self.min_leaf);
                (GridRect::new(leaf.x, leaf.y, cut, leaf.height), GridRect::new(leaf.x + cut, leaf.y, leaf.width - cut, leaf.height))
            } else {
                let cut = ctx.rng().random_range(self.min_leaf..=leaf.height - self.min_leaf);
                (GridRect::new(leaf.x, leaf.y, leaf.width, cut), GridRect::new(leaf.x, leaf.y + cut, leaf.width, leaf.height - cut))
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
        let inner = GridRect::new(leaf.x + self.padding, leaf.y + self.padding, leaf.width - 2 * self.padding, leaf.height - 2 * self.padding);
        if inner.width < 2 || inner.height < 2 {
            return None;
        }
        let w = ctx.rng().random_range(2..=inner.width);
        let h = ctx.rng().random_range(2..=inner.height);
        let x = ctx.rng().random_range(inner.x..=inner.right() - w);
        let y = ctx.rng().random_range(inner.y..=inner.bottom() - h);
        let room = GridRect::new(x, y, w, h);
        for p in room.cells() {
            ctx.terrain_mut().set(p, self.floor);
        }
        ctx.emit(Room(room));
        Some(room)
    }
}

impl Pass for Bsp {
    fn name(&self) -> &'static str {
        "bsp"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let bounds = ctx.terrain().bounds();
        let area = GridRect::new(1, 1, bounds.width - 2, bounds.height - 2);
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

impl Pass for Doors {
    fn name(&self) -> &'static str {
        "doors"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        let rooms: Vec<GridRect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
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
        ClearStart { cells, beyond: false, far_from: None }
    }

    /// This start, on one side of the place's [`Landmark`], the side drawn
    /// from the chain's stream, so a way down kept [`beyond`
    /// ](FarthestExit::beyond) it on the other leaves a route that crosses
    /// it.
    ///
    /// On its side it stands as the plain start does: in the first room,
    /// in the order they were emitted, lying wholly on that side, or on any
    /// open cell of the side when no room does. A side with nowhere to
    /// stand fails the place naming the landmark, since a way down kept to
    /// the other side would have nowhere to go either. With no landmark
    /// laid it is the plain start, from the same stream, so a chain can
    /// ask for it on every floor and only those grown round a landmark
    /// keep to a side.
    pub fn beyond(self) -> ClearStart {
        ClearStart { cells: 0, beyond: true, far_from: None }
    }
}

/// Every cell a [`Bridge`] was laid on. Neither way in nor way out
/// stands on one: a bridge is a way over a lake, and a [`Room`] a lake
/// was laid in still names the cells the bridge crosses.
fn bridged(ctx: &BaseContext) -> BitGrid {
    let mut cells = BitGrid::new(ctx.terrain().width(), ctx.terrain().height());
    for p in ctx.outputs().iter::<Bridge>().flat_map(|b| b.cells.iter()) {
        cells.insert(*p);
    }
    cells
}

/// The open cells of `area`, never one of `bridged`.
fn open_cells(ctx: &BaseContext, walkable: &[bool], bridged: &BitGrid, area: GridRect) -> Vec<Point> {
    area.cells().filter(|p| ctx.terrain().get(*p).is_some_and(|t| walkable[t.index()]) && !bridged.contains(*p)).collect()
}

/// The cells [`RandomStart`] draws from: the first room's open cells, or
/// the whole map's when no room was emitted.
fn start_cells(ctx: &BaseContext, walkable: &[bool], bridged: &BitGrid) -> Vec<Point> {
    let area = ctx.outputs().first::<Room>().map(|r| r.0).unwrap_or_else(|| ctx.terrain().bounds());
    open_cells(ctx, walkable, bridged, area)
}

/// One of `cells`, drawn from the chain's stream for the start, as the
/// [`StartPoint`].
fn emit_start(ctx: &mut BaseContext, cells: &[Point]) -> Result<(), BuildError> {
    if cells.is_empty() {
        return Err(BuildError::new("random_start", "no open cell"));
    }
    let p = cells[ctx.rng().random_range(0..cells.len())];
    ctx.emit(StartPoint(p));
    Ok(())
}

impl Pass for RandomStart {
    fn name(&self) -> &'static str {
        "random_start"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let cells = start_cells(ctx, &ctx.tiles().tables().walkable, &bridged(ctx));
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
/// plain start does, and never on a [`Bridge`], as the plain start never
/// is. Where no cell anywhere is that clear, it is
/// [`RandomStart`]'s own pick, from the same stream: a start closer than
/// a game hoped is a worse map, and a chain that failed over it would be
/// no map at all.
///
/// [`beyond`](Self::beyond) keeps it to one side of the landmark as well,
/// and [`Stairs::Within`] draws it far from the cell the landmark's
/// drawing marks for the way down.
#[derive(Debug, Clone, Copy)]
pub struct ClearStart {
    cells: i32,
    beyond: bool,
    /// The glyph of a way down within the landmark, which this start is
    /// drawn far from.
    far_from: Option<char>,
}

/// The farthest share, in a hundred, of the walk from a way down within a
/// landmark that the way in is drawn among. The landmark is the first
/// piece, so the first room grown is beside it, and a start drawn there
/// would stand a few steps from the stairs; drawn among the farthest
/// third and less, the walk between them is most of the place's width, as
/// a recipe's route bound asks of any way down.
const WITHIN_START_PCT: u32 = 30;

impl ClearStart {
    /// Kept to one side of the place's [`Landmark`] as well, as
    /// [`RandomStart::beyond`].
    pub fn beyond(mut self) -> Self {
        self.beyond = true;
        self
    }

    /// The way in for a way down within `landmark` on the cell its
    /// drawing marks `mark`: walked from that cell as [`FarthestExit`]
    /// walks from a start, and drawn among the farthest
    /// [`WITHIN_START_PCT`] of what it reaches, clear of every stamp where
    /// any such cell is.
    fn far_from_mark(&self, ctx: &mut BaseContext, landmark: &Landmark, mark: char, clear: &impl Fn(&Point) -> bool) -> Result<(), BuildError> {
        let at = landmark_mark(ctx, landmark, mark, "random_start")?;
        let (walkable, bridged) = (ctx.tiles().tables().walkable, bridged(ctx));
        let map = walk_through_doors(ctx, at, true);
        let standing = |p: &Point| *p != at && ctx.terrain().get(*p).is_some_and(|t| walkable[t.index()]) && !bridged.contains(*p);
        let reached: Vec<(Point, i32)> = map.iter().filter(|(p, _)| standing(p)).collect();
        let clear_ones: Vec<(Point, i32)> = reached.iter().copied().filter(|(p, _)| clear(p)).collect();
        let cells = if clear_ones.is_empty() { reached } else { clear_ones };
        let p = draw_far(ctx, &cells, Some(WITHIN_START_PCT))
            .ok_or_else(|| BuildError::new("random_start", format!("nowhere to stand walked to from the '{mark}' of the landmark {}", landmark.name)))?;
        ctx.emit(StartPoint(p));
        Ok(())
    }

    /// The cells it draws from on `side` of `landmark`: the first room
    /// lying wholly on it with an open cell clear of every stamp, or every
    /// such cell of the side, or every open cell of the side however near
    /// a stamp it stands.
    fn beyond_cells(&self, ctx: &BaseContext, landmark: &Landmark, side: Side, clear: &impl Fn(&Point) -> bool) -> Vec<Point> {
        let (walkable, bridged) = (ctx.tiles().tables().walkable, bridged(ctx));
        let on = |p: &Point| landmark.side(*p) == Some(side);
        let rooms = ctx.outputs().iter::<Room>().map(|r| r.0).filter(|r| on(&r.origin()) && on(&Point::new(r.right() - 1, r.bottom() - 1)));
        let room = rooms.map(|area| open_cells(ctx, &walkable, &bridged, area).into_iter().filter(clear).collect::<Vec<_>>()).find(|cells| !cells.is_empty());
        room.unwrap_or_else(|| {
            let open: Vec<Point> = open_cells(ctx, &walkable, &bridged, ctx.terrain().bounds()).into_iter().filter(on).collect();
            let clear_ones: Vec<Point> = open.iter().copied().filter(clear).collect();
            if clear_ones.is_empty() { open } else { clear_ones }
        })
    }
}

impl Pass for ClearStart {
    // The plain start's name, so both draw from one stream and the
    // fallback is exactly the plain pick.
    fn name(&self) -> &'static str {
        "random_start"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let stamps: Vec<GridRect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
        let clear = |p: &Point| stamps.iter().all(|r| atglyph_core::geometry::chebyshev(*p, atglyph_core::geometry::clamp_to(*p, *r)) >= self.cells);
        let landmark = ctx.outputs().first::<Landmark>().cloned();
        if let (Some(mark), Some(landmark)) = (self.far_from, &landmark) {
            return self.far_from_mark(ctx, landmark, mark, &clear);
        }
        if let Some(landmark) = landmark.filter(|_| self.beyond) {
            let sides = [Side::Before, Side::After].map(|side| self.beyond_cells(ctx, &landmark, side, &clear));
            if let Some(empty) = sides.iter().position(|cells| cells.is_empty()) {
                let side = [Side::Before, Side::After][empty];
                let reason =
                    format!("the landmark {} at {:?} has nowhere to stand on its {side:?} side of {:?}", landmark.name, landmark.bounds, landmark.sides);
                return Err(BuildError::new("random_start", reason));
            }
            let drawn = usize::from(!ctx.rng().random_bool(0.5));
            return emit_start(ctx, &sides[drawn]);
        }
        let mut areas: Vec<GridRect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
        if areas.is_empty() {
            areas.push(ctx.terrain().bounds());
        }
        let (walkable, bridged) = (ctx.tiles().tables().walkable, bridged(ctx));
        let cells = areas
            .into_iter()
            .map(|area| open_cells(ctx, &walkable, &bridged, area).into_iter().filter(clear).collect::<Vec<_>>())
            .find(|cells| !cells.is_empty());
        let cells = cells.unwrap_or_else(|| start_cells(ctx, &walkable, &bridged));
        emit_start(ctx, &cells)
    }
}

/// The one rule a walker who opens doors goes by: a cell is open when the
/// registry marks it passable, even where it is not walkable this moment,
/// because a corridor through a shut door still joins the rooms either
/// side of it, and when it is no cell of an
/// [`Opening`](crate::accrete::Opening) joined locked or sealed, which no
/// walker gets through, whatever tile it was laid as.
///
/// [`FarthestExit`] draws the way down by it, through
/// [`walk_through_doors`], and [`Survey`](crate::survey::Survey) and
/// [`OpenLoops`](crate::loops::OpenLoops) count their four-way steps by
/// it, so the route the survey measures and the detour a loop is cut for
/// are walked as the exit was. A pass that must agree with the exit asks
/// this rather than a rule of its own that could disagree with it about a
/// door or a lock. Everything that moves a body during play still asks a
/// [`TerrainView`](atglyph_grid::TerrainView), which stops at a shut door as
/// it should.
pub(crate) struct DoorWalk {
    passable: Vec<bool>,
    locks: BitGrid,
}

impl DoorWalk {
    /// The rule over `ctx` as it stands. With `locks_shut`, every lock and
    /// seal is shut; without it every one is taken as opened, for a pass
    /// asking what is reached once nothing is shut.
    pub(crate) fn new(ctx: &BaseContext, locks_shut: bool) -> Self {
        let locks = if locks_shut { lock_cells(ctx) } else { BitGrid::new(ctx.terrain().width(), ctx.terrain().height()) };
        DoorWalk { passable: ctx.tiles().tables().passable, locks }
    }

    /// Whether a walker who opens doors, and never a lock, gets through
    /// `p` of `terrain`. The terrain is a parameter so a pass changing it
    /// as it goes reads the rule over what it has built so far.
    pub(crate) fn open(&self, terrain: &Terrain, p: Point) -> bool {
        terrain.get(p).is_some_and(|t| self.passable[t.index()]) && !self.locks.contains(p)
    }
}

/// Every cell of an [`Opening`](crate::accrete::Opening) joined locked or
/// sealed: the cells [`DoorWalk`] never walks through, and the ones a
/// pass taking the passable cells away from itself takes away.
pub(crate) fn lock_cells(ctx: &BaseContext) -> BitGrid {
    use crate::accrete::{Join, Opening};
    let mut cells = BitGrid::new(ctx.terrain().width(), ctx.terrain().height());
    for o in ctx.outputs().iter::<Opening>().filter(|o| matches!(o.join, Join::Locked(_) | Join::Sealed)) {
        for c in &o.cells {
            cells.insert(*c);
        }
    }
    cells
}

/// [`DoorWalk`] as a [`CostSource`], each open cell at its tile's cost.
struct ThroughDoors<'a> {
    terrain: &'a Terrain,
    move_cost: Vec<u32>,
    rule: DoorWalk,
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
        self.rule.open(self.terrain, self.idx_point(idx)).then(|| self.move_cost[self.terrain.get_idx(idx).index()])
    }
}

/// The walk [`FarthestExit`] draws the way down by, from `from`, by
/// [`DoorWalk`]: doors opened on the way, and, with `locks_shut`, never
/// through a lock or a seal. Without it every lock and seal is taken as
/// opened, for a pass asking what is reached once nothing is shut.
pub(crate) fn walk_through_doors(ctx: &BaseContext, from: Point, locks_shut: bool) -> DijkstraMap {
    let through_doors = ThroughDoors { terrain: ctx.terrain(), move_cost: ctx.tiles().tables().move_cost, rule: DoorWalk::new(ctx, locks_shut) };
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

    /// This exit, on the side of the place's [`Landmark`] across from the
    /// start, so the route between them crosses it; a start level with it
    /// fails the place. With no landmark laid it is the plain exit.
    pub fn beyond(self) -> FarExit {
        FarExit::default().beyond()
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
    beyond: bool,
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

    /// Across the place's [`Landmark`] from the start, as
    /// [`FarthestExit::beyond`].
    pub fn beyond(mut self) -> Self {
        self.beyond = true;
        self
    }
}

impl Pass for FarthestExit {
    fn name(&self) -> &'static str {
        "farthest_exit"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        FarExit::default().apply(ctx)
    }
}

impl Pass for FarExit {
    // The plain exit's name, so both draw from one stream.
    fn name(&self) -> &'static str {
        "farthest_exit"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let err = |reason: String| BuildError::new("farthest_exit", reason);
        if self.among_pct.is_some_and(|pct| pct > 100) {
            return Err(err(format!("the farthest {}% is a share past a hundred", self.among_pct.unwrap_or(0))));
        }
        let start = ctx.outputs().first::<StartPoint>().ok_or_else(|| err("no start point emitted before it".into()))?.0;
        let tables = ctx.tiles().tables();
        let map = walk_through_doors(ctx, start, true);
        let bridged = bridged(ctx);
        let walkable = |p: Point| ctx.terrain().get(p).is_some_and(|t| tables.walkable[t.index()]) && !bridged.contains(p);
        let stamps: Vec<GridRect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
        let away = |p: Point| stamps.iter().map(|r| atglyph_core::geometry::chebyshev(p, atglyph_core::geometry::clamp_to(p, *r))).min().unwrap_or(i32::MAX);
        // The side across the landmark from the start, when kept beyond it.
        let landmark = ctx.outputs().first::<Landmark>().cloned().filter(|_| self.beyond);
        let across = match &landmark {
            Some(l) => Some(
                l.side(start).ok_or_else(|| err(format!("the way in at {start:?} stands level with the landmark {}, on neither side of it", l.name)))?.other(),
            ),
            None => None,
        };
        let beyond = |p: Point| across.is_none_or(|side| landmark.as_ref().and_then(|l| l.side(p)) == Some(side));
        let standing: Vec<(Point, i32)> = map.iter().filter(|(p, _)| *p != start && walkable(*p) && beyond(*p)).collect();
        let clear: Vec<(Point, i32)> = standing.iter().copied().filter(|(p, _)| away(*p) >= self.clear).collect();
        let kept = self.clear > 0;
        let cells = if clear.is_empty() && kept { standing.into_iter().filter(|(p, _)| away(*p) >= 1).collect() } else { clear };
        let at = draw_far(ctx, &cells, self.among_pct)
            .ok_or_else(|| err(format!("nowhere to stand farther than the start{}", if kept { " and outside every stamp" } else { "" })))?;
        ctx.emit(ExitPoint(at));
        Ok(())
    }
}

/// The far end of a walk: of `cells`, each with its walk, the one
/// farthest, ties toward the lower [`Point`], or with `among` a share, one
/// drawn from the chain's stream among every cell whose walk is at least
/// `100 - among` in a hundred of the longest. `None` when there are no
/// cells. The way down is drawn by it from the start, and a way in far
/// from a way down within a landmark from the landmark's mark.
fn draw_far(ctx: &mut BaseContext, cells: &[(Point, i32)], among: Option<u32>) -> Option<Point> {
    let far = cells.iter().map(|(_, v)| *v).max()?;
    Some(match among {
        None => cells.iter().max_by_key(|(p, v)| (*v, std::cmp::Reverse(*p))).expect("some cell is the farthest").0,
        Some(pct) => {
            let within: Vec<Point> = cells.iter().filter(|(_, v)| i64::from(*v) * 100 >= i64::from(far) * i64::from(100 - pct)).map(|(p, _)| *p).collect();
            within[ctx.rng().random_range(0..within.len())]
        }
    })
}

/// The cell the stamp of `landmark` itself marks `mark` at, never an
/// earlier stamp drawing the same glyph: the stamp whose bounds are the
/// landmark's. Fails naming the landmark and the glyph when its drawing
/// marks none.
fn landmark_mark(ctx: &BaseContext, landmark: &Landmark, mark: char, pass: &'static str) -> Result<Point, BuildError> {
    let stamp = ctx.outputs().iter::<Stamped>().find(|st| st.bounds == landmark.bounds);
    let at = stamp.and_then(|st| st.marks.iter().find(|(c, _)| *c == mark)).map(|(_, p)| *p);
    at.ok_or_else(|| BuildError::new(pass, format!("the landmark {} marks no '{mark}'", landmark.name)))
}

/// Emits the cell a stamped piece marked `mark` as the [`StartPoint`]:
/// the arrival a hand-drawn place, a town, has drawn into it.
///
/// The first stamp, in the order they were emitted, with the mark, so a
/// piece a place is grown round, laid before anything else, answers
/// before any stamped later. Fails when no stamp marks it, since a place
/// drawn to say where its way in is and saying nothing is drawn wrong.
#[derive(Debug, Clone, Copy)]
pub struct MarkedStart {
    /// The glyph the piece marks its way in with.
    pub mark: char,
}

/// The cell the first stamp marking `mark` marks it at.
fn marked(ctx: &BaseContext, mark: char, pass: &'static str) -> Result<Point, BuildError> {
    let at = ctx.outputs().iter::<Stamped>().flat_map(|st| st.marks.iter()).find(|(c, _)| *c == mark).map(|(_, p)| *p);
    at.ok_or_else(|| BuildError::new(pass, format!("no piece marks a '{mark}'")))
}

impl Pass for MarkedStart {
    fn name(&self) -> &'static str {
        "marked_start"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let at = marked(ctx, self.mark, "marked_start")?;
        ctx.emit(StartPoint(at));
        Ok(())
    }
}

/// Emits the cell a stamped piece marked `mark` as the [`ExitPoint`]: a
/// way down a landmark's drawing holds, a stair at the heart of a hall or
/// a shaft at the end of a bridge. [`MarkedStart`]'s pair, found the same
/// way and failing the same way.
///
/// Neither kept clear of stamps nor measured from the start, since the
/// drawing has already said where it is; whether it can be walked to is
/// the recipe's `require` bound on the route, as for every way down.
#[derive(Debug, Clone, Copy)]
pub struct MarkedExit {
    /// The glyph the piece marks its way down with.
    pub mark: char,
}

impl Pass for MarkedExit {
    fn name(&self) -> &'static str {
        "marked_exit"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let at = marked(ctx, self.mark, "marked_exit")?;
        ctx.emit(ExitPoint(at));
        Ok(())
    }
}

/// Where the way in and the way down stand relative to a place's
/// [`Landmark`], as a pool entry seeding one says it.
///
/// Not a system beside the passes that choose them but those passes
/// configured, so a game keeps its own way in and way down, clear of
/// stamps or among the farthest, and this only adds what the landmark
/// asks: [`start`](Self::start) and [`exit`](Self::exit) take the game's
/// own and hand back what goes in its chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
pub enum Stairs {
    /// Wherever the game's own passes put them. The landmark is the first
    /// piece, so the first room grown, where [`RandomStart`] stands the
    /// way in, is most often the one beside it: a free way in tends to
    /// stand at the landmark, which is the arrival a game asking for this
    /// gets.
    #[default]
    Free,
    /// On opposite sides of it, so the route between them crosses it.
    Beyond,
    /// The way down on the cell its drawing marks with this glyph, and the
    /// way in drawn among the cells farthest from it, so the walk to the
    /// stairs is not the few steps from the room grown beside it.
    Within(char),
}

impl Stairs {
    /// The way in `start` draws, kept to a side of the landmark when the
    /// stairs stand beyond it, and far from the way down when they stand
    /// within it.
    pub fn start(self, start: ClearStart) -> ClearStart {
        match self {
            Stairs::Free => start,
            Stairs::Beyond => start.beyond(),
            Stairs::Within(mark) => ClearStart { far_from: Some(mark), ..start },
        }
    }

    /// The way down: `exit`, kept across the landmark from the start when
    /// the stairs stand beyond it, or the cell the drawing marks when they
    /// stand within it.
    pub fn exit(self, exit: FarExit) -> WayDown {
        match self {
            Stairs::Free => WayDown::Far(exit),
            Stairs::Beyond => WayDown::Far(exit.beyond()),
            Stairs::Within(mark) => WayDown::Within(mark),
        }
    }
}

/// The way down [`Stairs::exit`] chose, as one pass for a chain.
#[derive(Debug, Clone, Copy)]
pub enum WayDown {
    /// The walk's far end, as [`FarthestExit`] draws it.
    Far(FarExit),
    /// The cell the landmark's own drawing marks with this glyph, as
    /// [`MarkedExit`] finds a mark but never on an earlier stamp drawing
    /// the same glyph. Fails when no landmark was laid.
    Within(char),
}

impl Pass for WayDown {
    fn name(&self) -> &'static str {
        match self {
            WayDown::Far(exit) => Pass::name(exit),
            WayDown::Within(_) => "marked_exit",
        }
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        match self {
            WayDown::Far(exit) => exit.apply(ctx),
            WayDown::Within(mark) => {
                let landmark = ctx.outputs().first::<Landmark>().cloned();
                let landmark = landmark.ok_or_else(|| BuildError::new("marked_exit", format!("no landmark was laid to hold the way down '{mark}'")))?;
                let at = landmark_mark(ctx, &landmark, *mark, "marked_exit")?;
                ctx.emit(ExitPoint(at));
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use atglyph_core::{RunSeed, Steps};
    use atglyph_grid::{TileProps, TileRegistry};

    fn ctx() -> (BaseContext, TileId, TileId, TileId) {
        let mut tiles = TileRegistry::standard();
        let door = tiles.register(TileProps::floor("door").opaque(true)).unwrap();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        (BaseContext::blank(60, 40, tiles, wall), wall, floor, door)
    }

    fn connected(c: &BaseContext) -> bool {
        let tables = c.tiles().tables();
        let regions = atglyph_grid::region::label_regions(c.terrain(), |i| tables.walkable[c.terrain().get_idx(i).index()], Steps::Eight);
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
            let rooms: Vec<GridRect> = c.outputs().iter::<Room>().map(|r| r.0).collect();
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
            assert!(atglyph_core::geometry::chebyshev(start, exit) > 10, "seed {seed}: exit too near");
        }
    }

    #[test]
    fn bsp_fills_the_map_evenly_and_connects() {
        for seed in 1..=6 {
            let (mut c, _, floor, _) = ctx();
            Chain::new().then(Bsp { floor, ..Default::default() }).then(RandomStart).run(&mut c, RunSeed(seed)).unwrap();
            let rooms: Vec<GridRect> = c.outputs().iter::<Room>().map(|r| r.0).collect();
            assert!(rooms.len() >= 8, "seed {seed}: {} rooms", rooms.len());
            assert!(connected(&c), "seed {seed}");
            let left = rooms.iter().filter(|r| r.center().x < 30).count();
            assert!(left >= 2 && left <= rooms.len() - 2, "seed {seed}: rooms all on one side");
            assert!(c.terrain().bounds().border().all(|p| c.terrain().get(p) != Some(floor)), "the edge is kept");
        }
    }

    /// Rooms with a three-by-three piece stamped in one of them, and then
    /// `start`, on `seed`: the start and the piece's bounds.
    fn start_beside_a_stamp(start: impl Pass + 'static, seed: u64) -> (Point, GridRect, Vec<GridRect>) {
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
    fn clearance(p: Point, r: GridRect) -> i32 {
        atglyph_core::geometry::chebyshev(p, atglyph_core::geometry::clamp_to(p, r))
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
        let room = GridRect::new(10, 10, 5, 5);
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
        let near = GridRect::new(1, 1, 4, 4);
        let far_room = GridRect::new(10, 1, 4, 4);
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
    fn exit_beside_a_grown_piece(exit: impl Pass + 'static, seed: u64) -> (Point, GridRect) {
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
    fn exit_on_a_corridor(exit: impl Pass + 'static) -> Point {
        let (mut c, _, floor, _) = ctx();
        for x in 1..29 {
            c.terrain_mut().set(Point::new(x, 2), floor);
        }
        c.emit(StartPoint(Point::new(1, 2)));
        c.emit(Stamped { bounds: GridRect::new(24, 1, 5, 3), marks: Vec::new(), prefab: None });
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
        let walk = walk_through_doors(&c, start, true);
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

    /// A place grown round `prefab` at `at`, with its way in and way down
    /// chosen as `stairs` says over the plain start and exit, on `seed`:
    /// the place and its landmark.
    fn stairs_round(prefab: &crate::prefab::Prefab, at: Vec<crate::prefab::Placement>, stairs: Stairs, seed: u64) -> (BaseContext, Landmark) {
        use crate::accrete::tests::{rig, seeded};
        let rig = rig();
        let chain = Chain::new().then(seeded(&rig, prefab, at)).then(stairs.start(RandomStart.clear_of_stamps(0))).then(stairs.exit(FarExit::default()));
        let c =
            chain.run_retrying(|| BaseContext::blank(84, 48, rig.tiles.clone(), rig.wall), RunSeed(seed), 20).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let landmark = c.outputs().first::<Landmark>().unwrap().clone();
        (c, landmark)
    }

    /// Over a span of seeds and every placement that divides a place
    /// differently, the way in and the way down kept beyond a landmark
    /// stand on opposite sides of it, each wholly past its band, and the
    /// way in is drawn on both sides over the span.
    #[test]
    fn beyond_puts_the_way_in_and_the_way_down_on_opposite_sides_of_the_landmark() {
        use crate::accrete::tests::{bar, landmark, rig};
        use crate::prefab::Placement;
        let rig = rig();
        let mut starts = std::collections::BTreeSet::new();
        for (piece, at) in [(landmark(&rig), Placement::Center), (landmark(&rig), Placement::Edge), (bar(&rig), Placement::Span)] {
            for seed in 0..30 {
                let (c, l) = stairs_round(&piece, vec![at], Stairs::Beyond, seed);
                let start = c.outputs().first::<StartPoint>().unwrap().0;
                let exit = c.outputs().first::<ExitPoint>().unwrap().0;
                let (from, to) = (l.side(start), l.side(exit));
                assert!(from.is_some() && to == from.map(Side::other), "{at:?} seed {seed}: {start:?} and {exit:?} round {l:?}");
                starts.insert(format!("{from:?}"));
            }
        }
        assert_eq!(starts.len(), 2, "the way in was drawn on both sides");
    }

    /// The way down within a landmark is the cell its drawing marks, on
    /// every seed and wherever the landmark was laid.
    #[test]
    fn a_way_down_within_the_landmark_stands_on_the_cell_its_drawing_marks() {
        use crate::accrete::tests::{landmark, rig};
        use crate::prefab::Placement;
        let rig = rig();
        for seed in 0..30 {
            let (c, l) = stairs_round(&landmark(&rig), vec![Placement::Center, Placement::Edge, Placement::Anywhere], Stairs::Within('X'), seed);
            let stamp = c.outputs().iter::<Stamped>().find(|st| st.bounds == l.bounds).unwrap();
            let mark = stamp.marks.iter().find(|(m, _)| *m == 'X').unwrap().1;
            assert_eq!(c.outputs().first::<ExitPoint>().unwrap().0, mark, "seed {seed}");
        }
    }

    /// With no landmark laid, a start and an exit asked to keep beyond one
    /// are the plain ones, drawn from the same streams, so a chain can ask
    /// on every floor and only the floors with a landmark change.
    #[test]
    fn beyond_with_no_landmark_laid_is_the_plain_start_and_exit() {
        fn ways(start: impl Pass + 'static, exit: FarExit, seed: u64) -> (Point, Point) {
            let (mut c, _, floor, door) = ctx();
            Chain::new().then(Rooms { floor, ..Default::default() }).then(Doors { door }).then(start).then(exit).run(&mut c, RunSeed(seed)).unwrap();
            (c.outputs().first::<StartPoint>().unwrap().0, c.outputs().first::<ExitPoint>().unwrap().0)
        }
        for seed in 1..=20 {
            let plain = ways(RandomStart, FarthestExit.among_farthest(30), seed);
            assert_eq!(ways(RandomStart.beyond(), FarthestExit.among_farthest(30).beyond(), seed), plain, "seed {seed}");
        }
    }

    /// A start kept beyond a landmark that is laid but beside which the
    /// way in already stands fails the place rather than picking a way
    /// down level with it.
    #[test]
    fn an_exit_kept_beyond_a_landmark_the_start_stands_level_with_fails_by_name() {
        let (mut c, _, floor, _) = ctx();
        for x in 1..59 {
            c.terrain_mut().set(Point::new(x, 5), floor);
        }
        c.emit(Landmark { name: "hall".into(), bounds: GridRect::new(10, 2, 5, 3), sides: crate::accrete::Sides::WestEast });
        c.emit(StartPoint(Point::new(12, 5)));
        let e = Chain::new().then(FarthestExit.beyond()).run(&mut c, RunSeed(1)).unwrap_err();
        assert!(e.pass == "farthest_exit" && e.reason.contains("level with the landmark hall"), "{e}");
    }

    /// A marked way in or way down stands on the first stamp's mark, and a
    /// place whose stamps mark none fails naming the glyph.
    #[test]
    fn a_marked_start_and_a_marked_exit_stand_on_the_cell_a_stamp_marks_and_fail_naming_a_glyph_none_marks() {
        let (mut c, ..) = ctx();
        c.emit(Stamped { bounds: GridRect::new(2, 2, 6, 3), marks: vec![('@', Point::new(3, 3)), ('>', Point::new(6, 3))], prefab: None });
        c.emit(Stamped { bounds: GridRect::new(20, 2, 3, 3), marks: vec![('@', Point::new(21, 3))], prefab: None });
        Chain::new().then(MarkedStart { mark: '@' }).then(MarkedExit { mark: '>' }).run(&mut c, RunSeed(1)).unwrap();
        assert_eq!(c.outputs().first::<StartPoint>().unwrap().0, Point::new(3, 3), "the first stamp's mark");
        assert_eq!(c.outputs().first::<ExitPoint>().unwrap().0, Point::new(6, 3));
        let e = Chain::new().then(MarkedExit { mark: 'v' }).run(&mut c, RunSeed(1)).unwrap_err();
        assert!(e.pass == "marked_exit" && e.reason.contains("'v'"), "{e}");
    }

    /// Over a span of seeds and every placement a piece with a marked
    /// way down may take, the way down within the landmark meets a
    /// recipe's route bound of thirty steps from the way in, on the place
    /// as grown and with no attempt grown again for it: the way in is
    /// drawn far from the marked cell, never in the room grown beside the
    /// landmark.
    #[test]
    fn a_way_down_within_a_landmark_is_a_recipes_route_from_the_way_in_on_every_seed() {
        use crate::accrete::tests::{landmark, rig};
        use crate::prefab::Placement;
        use crate::survey::{Require, Survey, Surveyed};
        let rig = rig();
        for at in [Placement::Center, Placement::Edge, Placement::Anywhere] {
            for seed in crate::testing::seeds(0..120) {
                let (mut c, _) = stairs_round(&landmark(&rig), vec![at], Stairs::Within('X'), seed);
                let require = Require { open_pct: (0, 100), areas: Vec::new(), loops: 0, dead_end_pct: 100, route: 30 };
                let met = Chain::new().then(Survey).then(require).run(&mut c, RunSeed(seed));
                let route = c.outputs().first::<Surveyed>().and_then(|s| s.route);
                assert!(met.is_ok(), "{at:?} seed {seed}: the route is {route:?}: {met:?}");
            }
        }
    }

    /// A way in kept beyond a landmark with nowhere to stand on one side
    /// of it fails the place naming the landmark and the side, whichever
    /// side the stream draws, rather than standing on the other side and
    /// leaving the way down nowhere to go.
    #[test]
    fn a_start_kept_beyond_a_landmark_with_one_side_empty_fails_naming_the_landmark() {
        for seed in 1..=10 {
            let (mut c, _, floor, _) = ctx();
            for x in 1..9 {
                c.terrain_mut().set(Point::new(x, 5), floor);
            }
            c.emit(Landmark { name: "hall".into(), bounds: GridRect::new(10, 2, 5, 3), sides: crate::accrete::Sides::WestEast });
            let e = Chain::new().then(Stairs::Beyond.start(RandomStart.clear_of_stamps(0))).run(&mut c, RunSeed(seed)).unwrap_err();
            assert!(e.pass == "random_start" && e.reason.contains("landmark hall") && e.reason.contains("After"), "seed {seed}: {e}");
        }
    }

    /// The way down within a landmark is the cell the landmark's own
    /// drawing marks, even when a stamp emitted before it draws the same
    /// glyph, and a place with no landmark laid fails asking for one.
    #[test]
    fn a_way_down_within_a_landmark_is_its_own_stamps_mark_and_never_an_earlier_stamps() {
        let (mut c, ..) = ctx();
        c.emit(Stamped { bounds: GridRect::new(2, 2, 6, 3), marks: vec![('X', Point::new(3, 3))], prefab: None });
        let e = Chain::new().then(Stairs::Within('X').exit(FarExit::default())).run(&mut c, RunSeed(1)).unwrap_err();
        assert!(e.pass == "marked_exit" && e.reason.contains("no landmark"), "{e}");
        c.emit(Stamped { bounds: GridRect::new(20, 2, 5, 3), marks: vec![('X', Point::new(22, 3))], prefab: None });
        c.emit(Landmark { name: "hall".into(), bounds: GridRect::new(20, 2, 5, 3), sides: crate::accrete::Sides::WestEast });
        Chain::new().then(Stairs::Within('X').exit(FarExit::default())).run(&mut c, RunSeed(1)).unwrap();
        assert_eq!(c.outputs().first::<ExitPoint>().unwrap().0, Point::new(22, 3));
    }
}
