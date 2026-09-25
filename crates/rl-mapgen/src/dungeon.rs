//! Passes that build a bounded map: rooms, partitions, doors, ways in and
//! out.
//!
//! Every tile is a parameter, as in [`passes`](crate::passes). Rooms are
//! published as [`Room`] outputs so later passes (doors, spawns, a
//! prefab wanting a room to sit in) can find them, and the ways in and out
//! as [`StartPoint`] and [`ExitPoint`].

use rand::Rng;
use rl_core::{Direction, Grid2D, Point, Rect};
use rl_grid::{DijkstraMap, PathRules, TileId};

use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
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
/// emitted [`Room`] when there is one. Fails if there is no open cell.
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

/// The open cells of `area`.
fn open_cells<C: BuildContext>(ctx: &C, area: Rect) -> Vec<Point> {
    let tables = ctx.tiles().tables();
    area.cells().filter(|p| ctx.terrain().get(*p).is_some_and(|t| tables.walkable[t.index()])).collect()
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

/// Emits the open cell farthest by walking from the [`StartPoint`] as the
/// [`ExitPoint`]. Fails without a start or with nowhere to walk to.
#[derive(Debug, Clone, Copy, Default)]
pub struct FarthestExit;

impl<C: BuildContext> Pass<C> for FarthestExit {
    fn name(&self) -> &'static str {
        "farthest_exit"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let start = ctx.outputs().first::<StartPoint>().ok_or_else(|| BuildError::new("farthest_exit", "no start point emitted before it"))?.0;
        let view = ctx.terrain().view(ctx.tiles());
        let mut map = DijkstraMap::covering(&view);
        map.build(&view, [start], PathRules::default());
        let far = map.iter().filter(|(p, _)| *p != start).max_by_key(|(_, v)| *v).map(|(p, _)| p);
        let far = far.ok_or_else(|| BuildError::new("farthest_exit", "nowhere to walk to from the start"))?;
        ctx.emit(ExitPoint(far));
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
}
