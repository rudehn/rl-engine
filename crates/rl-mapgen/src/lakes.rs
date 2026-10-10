//! Lakes laid over a grown place, and bridges where walking round one is
//! long.
//!
//! Brogue designs its lakes against the connectivity of the floor and then
//! bridges what they leave long to walk round; these two passes are that
//! practice. [`Lakes`] lays blobs of a tile over the open floor and the
//! rock beside it and refuses any that would part the floor, so a lake is
//! scenery a place is walked round and never a wall that shuts half of it
//! off. [`Bridges`] then lays a crossing straight over a liquid wherever
//! the walk round is long, which is what makes a wide lake a choice of
//! routes rather than a detour.
//!
//! Both read the floor the way the exit flood does: a cell is a way when
//! the registry marks it passable, a shut door included, and is not an
//! [`Opening`] joined locked or sealed, which no walker gets through.

use std::cmp::Reverse;

use rand::Rng;
use rand::seq::SliceRandom;
use rl_core::{Direction, Grid, Grid2D, Point, Steps};
use rl_grid::region::{Regions, flood, label_regions};
use rl_grid::{BitGrid, TileId};

use crate::accrete::{Area, Join, Opening, bounds_of, largest_rect};
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::prefab::Stamped;
use crate::walk::{UNREACHED, steps_between, walks_under};

/// How many blobs a lake draws before it is given up as having nowhere
/// to lie. A refused blob costs one flood of the map, so this is a few
/// floods per lake on a place with little room, and nothing more.
const TRIES: u32 = 24;

/// Lays lakes of [`tile`](Self::tile): each a blob grown from a cell with
/// open floor all round it, over that floor and the rock beside it, to a
/// size drawn from [`size`](Self::size), and laid only when the walkable
/// floor is as whole after it as before.
///
/// Whole means every region of floor still has a cell and is still one
/// region, and, on a place grown by [`Accrete`](crate::Accrete), the
/// [`Area`] the lake lies in is still one piece by itself, so what
/// [`Survey`](crate::Survey) says of the area graph stays true. A lake
/// that would cut is refused whole and another drawn, rather than trimmed
/// cell by cell, because a lake trimmed to fit reads as a lake drawn
/// round a path. One with nowhere to lie after a few draws is skipped:
/// a lake is scenery, and a place that must have one says so with a
/// [`Require`](crate::Require) of its own.
///
/// A lake keeps to one area, or to one region on a place with none, and
/// takes a cell of rock only straight beside its own floor, and only when
/// every way beside that rock, corners too, is under the lake as well. So
/// it bites into the wall round its own floor
/// and never through a wall into the next area: no new way and no new
/// window opens between two areas, whatever the tile is. It never covers
/// an [`Opening`] or a cell beside one, a piece painted from a prefab, a
/// [`Stamped`] piece or the ring round it, or a lake or shore already
/// laid, and it never touches the map's edge.
///
/// Floor a lake makes impassable is taken out of its area, and the area's
/// bounds and inner rectangle are measured again, so a pass that stands
/// something in an area never stands it in water. That is why it runs in
/// [`Phase::Structures`], after growth and loops and before the ways in
/// and out are chosen round it. Each lake laid is published as a [`Lake`].
#[derive(Debug, Clone, Copy)]
pub struct Lakes {
    /// What a lake is made of.
    pub tile: TileId,
    /// The fewest and the most lakes to try for.
    pub count: (u32, u32),
    /// The fewest and the most cells a lake grows to, floor and rock
    /// together, before the rock it may not take is trimmed off.
    pub size: (u32, u32),
    /// A shore: each floor cell beside a lake, corners too, becomes this
    /// tile at this chance in a hundred, so deep water has a ragged
    /// shallow edge. A shore that stops a body is held to the same test as
    /// the lake it edges.
    pub wreath: Option<(TileId, u32)>,
}

/// One lake as it was laid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lake {
    /// Every cell laid as the lake's tile, row by row.
    pub cells: Vec<Point>,
    /// Every cell laid as its shore, row by row; empty without a wreath.
    pub wreath: Vec<Point>,
}

/// Lays [`tile`](Self::tile) straight across [`across`](Self::across),
/// west to east or north to south, wherever the liquid is at most
/// [`longest`](Self::longest) cells wide and walking round from one bank
/// to the other is at least [`detour`](Self::detour) steps.
///
/// A crossing has the liquid on both flanks of every cell it spans, as
/// Brogue's bridges do, so it crosses a lake and never runs along its edge
/// beside a wall or a shore, where it would be a ledge and not a bridge.
///
/// Longest walk round first, as [`OpenLoops`](crate::OpenLoops) opens its
/// doors, and each measured again before it is laid, since one bridge
/// shortens the walk round for the crossings beside it; so a lake gains
/// one bridge where one is worth having, not a row of them. A bank with no
/// walk round at all, where the liquid parts what was laid, is the longest
/// walk of all and is bridged first. Among crossings equally long the
/// pass's own stream draws which. A bank is never an [`Opening`] or a cell
/// of a piece painted from a prefab, which is entered only at its entries.
///
/// It runs in [`Phase::Connect`], after [`Lakes`] and before the ways in
/// and out, and publishes each crossing as a [`Bridge`]. A bridge belongs
/// to no [`Area`]: it is a way over a lake, not floor to stand a thing on.
#[derive(Debug, Clone, Copy)]
pub struct Bridges {
    /// The liquid to cross.
    pub across: TileId,
    /// What a bridge is made of. Something other than `across`, or a
    /// bridge would cross nothing.
    pub tile: TileId,
    /// The most cells of liquid one bridge spans.
    pub longest: u32,
    /// The shortest walk round, in orthogonal steps between the two banks,
    /// worth a bridge.
    pub detour: u32,
}

/// One bridge as it was laid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bridge {
    /// The cells laid as the bridge, from the first bank out.
    pub cells: Vec<Point>,
    /// The direction from the first bank to the second.
    pub facing: Direction,
    /// The walk round from bank to bank as it was when the bridge was
    /// laid, in orthogonal steps; `u32::MAX` where there was none.
    pub detour: u32,
}

/// Every cell the registry marks passable, a shut door included.
fn passable_cells<C: BuildContext>(ctx: &C) -> BitGrid {
    let passable = ctx.tiles().tables().passable;
    let terrain = ctx.terrain();
    let mut cells = BitGrid::new(terrain.width(), terrain.height());
    for (p, t) in terrain.iter() {
        cells.set(p, passable[t.index()]);
    }
    cells
}

/// Every cell a walker who opens doors gets to: passable, and not an
/// opening joined locked or sealed.
fn ways<C: BuildContext>(ctx: &C, passable: &BitGrid) -> BitGrid {
    let mut open = passable.clone();
    for o in ctx.outputs().iter::<Opening>().filter(|o| matches!(o.join, Join::Locked(_) | Join::Sealed)) {
        for c in &o.cells {
            open.set(*c, false);
        }
    }
    open
}

/// The cells a bridge never lands on: every opening's cells and every
/// cell of a painted piece with its entries.
fn landings_barred<C: BuildContext>(ctx: &C) -> BitGrid {
    let mut barred = BitGrid::new(ctx.terrain().width(), ctx.terrain().height());
    for o in ctx.outputs().iter::<Opening>() {
        for c in &o.cells {
            barred.insert(*c);
        }
    }
    for a in ctx.outputs().iter::<Area>() {
        if let Some(entries) = &a.entries {
            for c in a.cells.iter().chain(entries) {
                barred.insert(*c);
            }
        }
    }
    barred
}

/// What a lake keeps off: every opening and the cells beside it, every
/// painted piece with its entries, and every stamp with the ring round
/// it.
fn kept_dry<C: BuildContext>(ctx: &C) -> BitGrid {
    let bounds = ctx.terrain().bounds();
    let mut kept = landings_barred(ctx);
    for o in ctx.outputs().iter::<Opening>() {
        for c in &o.cells {
            for d in Direction::CARDINALS {
                kept.insert(*c + d.offset());
            }
        }
    }
    for s in ctx.outputs().iter::<Stamped>() {
        if let Some(r) = s.bounds.inflate(1).intersection(&bounds) {
            for c in r.cells() {
                kept.insert(c);
            }
        }
    }
    kept
}

/// What a lake about to be laid would make of the map: the floor it
/// covers and the floor its shore covers.
struct Draft {
    lake: BitGrid,
    wreath: BitGrid,
}

impl Lakes {
    /// Whether the cell at `idx` would be a way once `draft` is laid.
    fn open_after(&self, passable: &[bool], open: &BitGrid, draft: &Draft, idx: usize) -> bool {
        if draft.lake.get_idx(idx) {
            passable[self.tile.index()]
        } else if draft.wreath.get_idx(idx) {
            self.wreath.is_some_and(|(t, _)| passable[t.index()])
        } else {
            open.get_idx(idx)
        }
    }

    /// Whether the floor is as whole with `draft` laid as it is now: every
    /// region keeps a cell and stays one, and the area the lake lies in,
    /// if the place has areas, stays one piece by itself.
    ///
    /// Floor taken away never joins two regions, and rock is taken only
    /// beside one area's floor, so even a lake a body can cross joins
    /// none: as many regions after as before, each keeping a cell, means
    /// none was cut.
    fn keeps_whole(&self, passable: &[bool], open: &BitGrid, regions: &Regions, area: Option<(&Grid<u32>, u32)>, draft: &Draft) -> bool {
        let after = |i: usize| self.open_after(passable, open, draft, i);
        if label_regions(open, after, Steps::Four).count() != regions.count() {
            return false;
        }
        let mut kept = vec![false; regions.count()];
        for i in open.iter_indices().filter(|i| after(*i)) {
            kept[regions.label_idx(i).expect("an open cell has a region") as usize - 1] = true;
        }
        if kept.contains(&false) {
            return false;
        }
        let Some((owner, own)) = area else { return true };
        let inside = |i: usize| owner.cells()[i] == own && after(i);
        let left: Vec<usize> = (0..owner.cells().len()).filter(|i| inside(*i)).collect();
        let Some(first) = left.first() else { return false };
        let mut reached = BitGrid::new(open.width(), open.height());
        flood(open, open.idx_point(*first), inside, Steps::Four, &mut reached) == left.len()
    }
}

impl<C: BuildContext> Pass<C> for Lakes {
    fn name(&self) -> &'static str {
        "lakes"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        let bounds = ctx.terrain().bounds();
        let (w, h) = (bounds.width, bounds.height);
        if w < 3 || h < 3 {
            return Ok(());
        }
        let mut kept = kept_dry(ctx);
        let mut cells = passable_cells(ctx);
        let mut open = ways(ctx, &cells);
        let mut regions = label_regions(&open, |i| open.get_idx(i), Steps::Four);
        // Whose floor each cell is: its area's index and one on a grown
        // place, painted pieces left out; its region's label on one with
        // no areas. Nought is no one's, which no lake takes.
        let has_areas = ctx.outputs().first::<Area>().is_some();
        let owner = if has_areas {
            let mut owner = Grid::filled(w, h, 0u32);
            for a in ctx.outputs().iter::<Area>().filter(|a| a.entries.is_none()) {
                for c in &a.cells {
                    owner.set(*c, a.index as u32 + 1);
                }
            }
            owner
        } else {
            regions.labels().clone()
        };
        let whose = |p: Point| owner.get(p).copied().unwrap_or(0);
        let interior = bounds.inflate(-1);

        let count = ctx.rng().random_range(self.count.0..=self.count.1);
        for _ in 0..count {
            let floor = |p: Point, own: u32| own != 0 && whose(p) == own && open.contains(p) && !kept.contains(p);
            let rock = |p: Point| interior.contains(p) && !cells.contains(p) && !kept.contains(p);
            let seeds: Vec<Point> = interior.cells().filter(|p| rl_core::geometry::square(*p, 1).all(|q| floor(q, whose(*p)))).collect();
            if seeds.is_empty() {
                break;
            }
            for _ in 0..TRIES {
                let seed = seeds[ctx.rng().random_range(0..seeds.len())];
                let own = whose(seed);
                let draft = self.draw(ctx.rng(), seed, |p| floor(p, own), rock, &cells);
                let area = has_areas.then_some((&owner, own));
                if !self.keeps_whole(&tables.passable, &open, &regions, area, &draft) {
                    continue;
                }
                let after = |p: Point| open.checked_idx(p).is_some_and(|i| self.open_after(&tables.passable, &open, &draft, i));
                if has_areas && let Some(a) = ctx.outputs_mut().iter_mut::<Area>().find(|a| a.index as u32 + 1 == own) {
                    a.cells.retain(|c| after(*c));
                    a.bounds = bounds_of(&a.cells);
                    a.inner = largest_rect(&a.cells, a.bounds);
                }
                for p in draft.lake.iter() {
                    ctx.terrain_mut().set(p, self.tile);
                }
                if let Some((shore, _)) = self.wreath {
                    for p in draft.wreath.iter() {
                        ctx.terrain_mut().set(p, shore);
                    }
                }
                // A lake laid is no later lake's to take, so what each
                // publishes stays what it is.
                kept.union_with(&draft.lake);
                kept.union_with(&draft.wreath);
                ctx.emit(Lake { cells: draft.lake.iter().collect(), wreath: draft.wreath.iter().collect() });
                cells = passable_cells(ctx);
                open = ways(ctx, &cells);
                regions = label_regions(&open, |i| open.get_idx(i), Steps::Four);
                break;
            }
        }
        Ok(())
    }
}

impl Lakes {
    /// Grows one lake from `seed` over `floor` and the `rock` beside it, a
    /// cell at a time from a frontier drawn at random, so it comes out a
    /// blob and not a square; trims the rock it may not take and whatever
    /// that leaves apart from the seed; and draws its shore.
    fn draw(&self, rng: &mut impl Rng, seed: Point, floor: impl Fn(Point) -> bool, rock: impl Fn(Point) -> bool, passable: &BitGrid) -> Draft {
        let (w, h) = (passable.width(), passable.height());
        let size = rng.random_range(self.size.0..=self.size.1);
        let (mut grown, mut queued) = (BitGrid::new(w, h), BitGrid::new(w, h));
        let (mut frontier, mut cells) = (vec![seed], 0);
        queued.insert(seed);
        while cells < size && !frontier.is_empty() {
            let p = frontier.swap_remove(rng.random_range(0..frontier.len()));
            if !floor(p) && !rock(p) {
                continue;
            }
            grown.insert(p);
            cells += 1;
            // Rock is the lake's edge and grows nothing further, so a lake
            // bites into the wall beside its floor and never runs off
            // through the rock.
            if !floor(p) {
                continue;
            }
            for d in Direction::CARDINALS {
                let n = p + d.offset();
                if passable.in_bounds(n) && !queued.contains(n) {
                    queued.insert(n);
                    frontier.push(n);
                }
            }
        }
        // Rock is taken only where every way beside it, corners too, is
        // under the lake: so the lake eats the wall round its own floor and
        // never the wall between it and anything else.
        let mut trimmed = BitGrid::new(w, h);
        for p in grown.iter() {
            if floor(p) || rl_core::geometry::square(p, 1).all(|q| !passable.contains(q) || grown.contains(q)) {
                trimmed.insert(p);
            }
        }
        let mut lake = BitGrid::new(w, h);
        flood(&trimmed, seed, |i| trimmed.get_idx(i), Steps::Four, &mut lake);
        let mut wreath = BitGrid::new(w, h);
        if let Some((_, pct)) = self.wreath {
            let mut shore = BitGrid::new(w, h);
            for p in lake.iter() {
                for q in rl_core::geometry::square(p, 1) {
                    if !lake.contains(q) && floor(q) {
                        shore.insert(q);
                    }
                }
            }
            for q in shore.iter() {
                if rng.random_range(0..100) < pct {
                    wreath.insert(q);
                }
            }
        }
        Draft { lake, wreath }
    }
}

/// A straight run of liquid that could be bridged: the bank it starts
/// from, which way it runs, how many cells of liquid it spans, and the
/// walk round as last measured, a ceiling on what it is now.
#[derive(Debug, Clone, Copy)]
struct Crossing {
    from: Point,
    facing: Direction,
    span: u32,
    detour: u32,
}

impl Crossing {
    fn cells(&self) -> impl Iterator<Item = Point> + '_ {
        let step = self.facing.offset();
        (1..=self.span as i32).map(move |k| self.from.offset(step.x * k, step.y * k))
    }

    fn to(&self) -> Point {
        let step = self.facing.offset();
        let k = self.span as i32 + 1;
        self.from.offset(step.x * k, step.y * k)
    }
}

impl<C: BuildContext> Pass<C> for Bridges {
    fn name(&self) -> &'static str {
        "bridges"
    }
    fn phase(&self) -> Phase {
        Phase::Connect
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        if self.tile == self.across {
            return Err(BuildError::new("bridges", "a bridge laid as the liquid it crosses crosses nothing"));
        }
        let bounds = ctx.terrain().bounds();
        let (w, h) = (bounds.width, bounds.height);
        let barred = landings_barred(ctx);
        let mut open = ways(ctx, &passable_cells(ctx));
        let bank = |open: &BitGrid, p: Point| open.contains(p) && !barred.contains(p);

        // Every straight run from a bank over liquid to a bank, found from
        // its west or north end only, so each is found once, with liquid
        // on both flanks of every cell, so a bridge crosses the middle of
        // a liquid and never runs along its edge beside a wall or a shore.
        let terrain = ctx.terrain();
        let liquid = |p: Point| terrain.get(p) == Some(self.across);
        let mut crossings = Vec::new();
        for from in bounds.cells().filter(|p| bank(&open, *p)) {
            for facing in [Direction::East, Direction::South] {
                let flanks = facing.perpendicular().map(|d| d.offset());
                let mut c = Crossing { from, facing, span: 0, detour: UNREACHED };
                while c.span < self.longest && liquid(c.to()) && flanks.iter().all(|f| liquid(c.to() + *f)) {
                    c.span += 1;
                }
                if c.span > 0 && bank(&open, c.to()) {
                    crossings.push(c);
                }
            }
        }
        // Drawn into an order first, so among crossings equally long the
        // stream picks and not the corner of the map they lie nearest.
        crossings.shuffle(ctx.rng());
        // Most crossings are a few steps round, so each is ruled out by a
        // flood that stops at the shortest detour worth a bridge, and
        // only the rest are ever walked in full.
        crossings.retain(|c| !walks_under(w, h, |p| open.contains(p), c.from, c.to(), self.detour));

        loop {
            let terrain = ctx.terrain();
            crossings.retain(|c| c.cells().all(|p| terrain.get(p) == Some(self.across)));
            // A bridge only ever shortens a walk, so a walk measured before
            // the last bridge is a ceiling on what it is now. Longest
            // ceiling first, each measured again, until the longest found
            // is no shorter than every ceiling left, as `OpenLoops` does.
            crossings.sort_by_key(|c| Reverse(c.detour));
            let mut best: Option<usize> = None;
            for i in 0..crossings.len() {
                if best.is_some_and(|b| crossings[b].detour >= crossings[i].detour) {
                    break;
                }
                let c = crossings[i];
                crossings[i].detour = steps_between(w, h, |p| open.contains(p), c.from, c.to());
                if best.is_none_or(|b| crossings[i].detour > crossings[b].detour) {
                    best = Some(i);
                }
            }
            let Some(best) = best else { break };
            if crossings[best].detour < self.detour {
                break;
            }
            let c = crossings.swap_remove(best);
            let cells: Vec<Point> = c.cells().collect();
            for p in &cells {
                ctx.terrain_mut().set(*p, self.tile);
            }
            open = ways(ctx, &passable_cells(ctx));
            ctx.emit(Bridge { cells, facing: c.facing, detour: c.detour });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accrete::tests::{accrete, owing, rig};
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use crate::loops::OpenLoops;
    use rl_core::RunSeed;
    use rl_grid::{Terrain, TileProps, TileRegistry};

    /// The accretion rig's tiles with a liquid that stops a body, a shore
    /// that does not, and a bridge.
    struct Rig {
        tiles: TileRegistry,
        wall: TileId,
        door: TileId,
        water: TileId,
        shore: TileId,
        bridge: TileId,
    }

    fn rig_with_water() -> Rig {
        let base = rig();
        let mut tiles = base.tiles.clone();
        let water = tiles.register(TileProps::named("still")).unwrap();
        let shore = tiles.register(TileProps::floor("shallow")).unwrap();
        let bridge = tiles.register(TileProps::floor("span")).unwrap();
        Rig { tiles, wall: base.wall, door: base.door, water, shore, bridge }
    }

    fn lakes(r: &Rig) -> Lakes {
        Lakes { tile: r.water, count: (2, 5), size: (20, 70), wreath: Some((r.shore, 60)) }
    }

    fn bridges(r: &Rig) -> Bridges {
        Bridges { across: r.water, tile: r.bridge, longest: 6, detour: 20 }
    }

    const SIZES: [(i32, i32); 2] = [(84, 48), (160, 90)];

    /// A place grown with loops at `seed`, then whatever `more` adds; with
    /// `owed`, one piece painted from a prefab and joined locked.
    fn grown(seed: u64, (w, h): (i32, i32), owed: bool, more: impl FnOnce(Chain<BaseContext>, &Rig) -> Chain<BaseContext>) -> BaseContext {
        let r = rig_with_water();
        let growth = if owed { owing(&rig(), Join::Locked(7)) } else { accrete(&rig(), 3) };
        let chain = more(Chain::new().then(growth).then(OpenLoops { door: r.door, min_detour: 20, max: 6 }), &r);
        chain.run_retrying(|| BaseContext::blank(w, h, r.tiles.clone(), r.wall), RunSeed(seed), 20).unwrap_or_else(|e| panic!("seed {seed}: {e}"))
    }

    /// The regions of `terrain` a walker who opens doors sees, the cells
    /// of locked and sealed openings shut.
    fn regions(terrain: &Terrain, tiles: &TileRegistry, shut: &[Point]) -> usize {
        let passable = tiles.tables().passable;
        let open = |i: usize| passable[terrain.get_idx(i).index()] && !shut.contains(&terrain.idx_point(i));
        label_regions(terrain, open, Steps::Four).count()
    }

    fn lay(terrain: &mut Terrain, lake: &Lake, r: &Rig) {
        for p in &lake.cells {
            terrain.set(*p, r.water);
        }
        for p in &lake.wreath {
            terrain.set(*p, r.shore);
        }
    }

    #[test]
    fn the_walkable_floor_is_one_region_after_every_lake() {
        let r = rig_with_water();
        for size in SIZES {
            let (mut places, mut wet) = (0, 0);
            for s in 0..200 {
                let dry = grown(s, size, false, |c, _| c);
                let lakes = grown(s, size, false, |c, r| c.then(lakes(r)));
                let mut terrain = dry.terrain().clone();
                assert_eq!(regions(&terrain, &r.tiles, &[]), 1, "{size:?} seed {s}: grown in one piece");
                for (i, lake) in lakes.outputs().iter::<Lake>().enumerate() {
                    lay(&mut terrain, lake, &r);
                    assert_eq!(regions(&terrain, &r.tiles, &[]), 1, "{size:?} seed {s}: lake {i} parts the floor");
                }
                assert_eq!(&terrain, lakes.terrain(), "{size:?} seed {s}: every lake laid is published");
                places += 1;
                wet += usize::from(lakes.outputs().first::<Lake>().is_some());
            }
            assert!(wet * 10 >= places * 9, "{size:?}: only {wet} of {places} places have a lake");
        }
    }

    /// The area a lake lies in loses the floor it covered and keeps the
    /// rest in one piece, so a survey's area graph and a stamp's inner
    /// rectangle stay true; and no lake or shore takes an opening, a cell
    /// beside one, or a painted piece, which is also how a locked piece
    /// keeps its one way in.
    #[test]
    fn a_lake_keeps_off_openings_and_painted_pieces_and_every_area_stays_one_piece() {
        let r = rig_with_water();
        let passable = r.tiles.tables().passable;
        for s in 0..60 {
            let dry = grown(s, SIZES[0], true, |c, _| c);
            let wet = grown(s, SIZES[0], true, |c, r| c.then(lakes(r)));
            let shut: Vec<Point> = wet.outputs().iter::<Opening>().filter(|o| matches!(o.join, Join::Locked(_))).flat_map(|o| o.cells.clone()).collect();
            assert!(!shut.is_empty(), "seed {s}: the piece is locked");
            assert_eq!(regions(dry.terrain(), &r.tiles, &shut), regions(wet.terrain(), &r.tiles, &shut), "seed {s}: as many regions as before");
            let painted: Vec<Point> =
                wet.outputs().iter::<Area>().filter(|a| a.entries.is_some()).flat_map(|a| a.cells.iter().chain(a.entries.iter().flatten()).copied()).collect();
            assert!(!painted.is_empty(), "seed {s}: the piece is laid");
            for lake in wet.outputs().iter::<Lake>() {
                for p in lake.cells.iter().chain(&lake.wreath) {
                    let beside = wet.outputs().iter::<Opening>().any(|o| o.cells.iter().any(|c| rl_core::geometry::manhattan(*c, *p) <= 1));
                    assert!(!beside, "seed {s}: {p:?} is on or beside an opening");
                    assert!(!painted.contains(p), "seed {s}: {p:?} is in a painted piece");
                }
            }
            for a in wet.outputs().iter::<Area>() {
                assert!(a.cells.iter().all(|c| passable[wet.terrain().get(*c).unwrap().index()]), "seed {s}: area {} keeps covered floor", a.index);
                assert_eq!(a.bounds, bounds_of(&a.cells), "seed {s}: area {} bounds", a.index);
                let inside = |i: usize| a.cells.contains(&wet.terrain().idx_point(i));
                let mut reached = BitGrid::new(wet.terrain().width(), wet.terrain().height());
                assert_eq!(flood(wet.terrain(), a.cells[0], inside, Steps::Four, &mut reached), a.cells.len(), "seed {s}: area {} is in two", a.index);
            }
        }
    }

    #[test]
    fn a_wreath_only_borders_its_own_lake_and_only_over_floor() {
        let r = rig_with_water();
        let walkable = r.tiles.tables().walkable;
        let mut shores = 0;
        for size in SIZES {
            for s in 0..200 {
                let dry = grown(s, size, false, |c, _| c);
                let wet = grown(s, size, false, |c, r| c.then(lakes(r)));
                for lake in wet.outputs().iter::<Lake>() {
                    for p in &lake.wreath {
                        assert!(rl_core::geometry::square(*p, 1).any(|q| lake.cells.contains(&q)), "{size:?} seed {s}: {p:?} borders no cell of its lake");
                        assert!(walkable[dry.terrain().get(*p).unwrap().index()], "{size:?} seed {s}: {p:?} was not floor");
                        assert_eq!(wet.terrain().get(*p), Some(r.shore), "{size:?} seed {s}: {p:?} was laid over by another lake");
                    }
                    shores += lake.wreath.len();
                }
            }
        }
        assert!(shores > 0, "no lake on any seed has a shore");
    }

    #[test]
    fn a_bridge_spans_at_most_its_longest_and_is_laid_only_where_the_walk_round_was_at_least_its_detour() {
        let r = rig_with_water();
        let pass = bridges(&r);
        let passable = r.tiles.tables().passable;
        let mut laid = 0;
        for size in SIZES {
            for s in 0..200 {
                let wet = grown(s, size, false, |c, r| c.then(lakes(r)));
                let bridged = grown(s, size, false, |c, r| c.then(lakes(r)).then(bridges(r)));
                let mut terrain = wet.terrain().clone();
                for b in bridged.outputs().iter::<Bridge>() {
                    let span = b.cells.len() as u32;
                    assert!((1..=pass.longest).contains(&span), "{size:?} seed {s}: a bridge spans {span}");
                    let step = b.facing.offset();
                    let (from, to) = (b.cells[0].offset(-step.x, -step.y), b.cells[b.cells.len() - 1].offset(step.x, step.y));
                    for (k, p) in b.cells.iter().enumerate() {
                        assert_eq!(*p, from.offset(step.x * (k as i32 + 1), step.y * (k as i32 + 1)), "{size:?} seed {s}: a bridge runs straight");
                        assert_eq!(terrain.get(*p), Some(r.water), "{size:?} seed {s}: {p:?} was not the liquid");
                    }
                    let (w, h) = size;
                    let open = |p: Point| terrain.get(p).is_some_and(|t| passable[t.index()]);
                    let round = steps_between(w, h, open, from, to);
                    assert_eq!(round, b.detour, "{size:?} seed {s}: the walk round is as published");
                    assert!(round >= pass.detour, "{size:?} seed {s}: a bridge where the walk round was {round}");
                    for p in &b.cells {
                        terrain.set(*p, r.bridge);
                    }
                    laid += 1;
                }
                assert_eq!(&terrain, bridged.terrain(), "{size:?} seed {s}: every bridge laid is published");
            }
        }
        assert!(laid > 0, "no bridge on any seed");
    }

    /// A pass the chain gains draws from its own stream, so the lakes laid
    /// after it are the lakes laid without it.
    #[test]
    fn the_same_seed_lays_the_same_lakes_after_a_pass_is_added_to_the_chain() {
        struct Idle;
        impl Pass<BaseContext> for Idle {
            fn name(&self) -> &'static str {
                "idle"
            }
            fn phase(&self) -> Phase {
                Phase::Structures
            }
            fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
                let drawn: u64 = ctx.rng().random();
                ctx.emit(drawn);
                Ok(())
            }
        }
        for size in SIZES {
            for s in 0..200 {
                let plain = grown(s, size, false, |c, r| c.then(lakes(r)));
                let more = grown(s, size, false, |c, r| c.then(Idle).then(lakes(r)).then(bridges(r)));
                let (a, b): (Vec<&Lake>, Vec<&Lake>) = (plain.outputs().iter().collect(), more.outputs().iter().collect());
                assert_eq!(a, b, "{size:?} seed {s}");
            }
        }
    }

    #[test]
    fn a_bridge_of_the_liquid_it_crosses_is_refused() {
        let r = rig_with_water();
        let mut ctx = BaseContext::blank(8, 8, r.tiles.clone(), r.wall);
        let pass = Bridges { across: r.water, tile: r.water, longest: 4, detour: 2 };
        assert!(pass.apply(&mut ctx).is_err());
    }
}
