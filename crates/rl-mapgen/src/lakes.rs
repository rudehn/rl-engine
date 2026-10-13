//! Lakes laid over a grown place, and bridges where walking round one is
//! long.
//!
//! Brogue designs its lakes against the connectivity of the floor and then
//! bridges what they leave long to walk round; these two passes are that
//! practice. [`Lakes`] lays blobs of a tile over the open floor and the
//! rock beside it and, unless told it may [`cut`](Lakes::cut), refuses
//! any that would part the floor, so a lake is scenery a place is walked
//! round. [`Bridges`] then lays a crossing straight over a liquid wherever
//! the walk round is long, which makes a wide lake a choice of routes
//! rather than a detour, and first of all wherever there is no walk round,
//! which is what joins an island back on; it fails a place it cannot join.
//!
//! Both read the floor the way the exit flood does: a cell is a way when
//! the registry marks it passable, a shut door included, and is not an
//! [`Opening`] joined locked or sealed, which no walker gets through.
//! Neither touches what lies behind such an opening, so no lake parts it
//! and no bridge leads round its lock.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use rand::Rng;
use rand::seq::SliceRandom;
use rl_core::{Direction, Grid, Grid2D, Point, Steps};
use rl_grid::region::{Regions, flood, label_regions};
use rl_grid::{BitGrid, TileId};

use crate::accrete::{Area, Join, Opening, bounds_of, largest_rect};
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::dungeon::{Room, lock_cells};
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
/// [`Require`](crate::Require) of its own. [`cut`](Self::cut) lifts the
/// test, for a place of islands.
///
/// Unless it may cut, a lake keeps to one area, or to one region on a
/// place with none, and takes a cell of rock only straight beside its own
/// floor. Either way it takes rock only when every way beside that rock,
/// corners too, is under the lake as well. So a pond bites into the wall
/// round its own floor and never through a wall into the next area, and
/// no lake opens a new way or a new window onto floor it leaves dry,
/// whatever the tile is. It never covers an [`Opening`] or a cell
/// beside one, a piece painted from a prefab, an area reached from the
/// first only through an opening joined locked or sealed, a [`Stamped`]
/// piece or the ring round it, or a lake or shore already laid, and it
/// never touches the map's edge.
///
/// Whatever it may cut, a lake never leaves an area it touches without
/// floor.
///
/// Floor a lake makes impassable is taken out of its area, and the area's
/// bounds and inner rectangle are measured again, as is every [`Room`]
/// it lay in, inside the room's own rectangle, so a pass that stands
/// something in an area or a room never stands it in water. A room the
/// lake covered whole is dropped, which renumbers the rooms after it.
/// That is why it runs in [`Phase::Structures`], after growth and loops
/// and before the ways in and out are chosen round it. Each lake laid is
/// published as a [`Lake`].
#[derive(Debug, Clone)]
pub struct Lakes {
    /// A stable name, so two lakings in one chain, deep lakes and small
    /// pools or one liquid and another, draw different streams.
    pub name: &'static str,
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
    /// Whether a lake may part the floor: span several areas, through the
    /// walls between them, and leave islands and spits that only a bridge
    /// reaches. Off, a lake is a pond in one area that the floor is walked
    /// round. On, the chain must hold a [`Bridges`] over the same tile,
    /// which joins every part back or fails the place, and the tile must
    /// stop a body: one a body crosses parts nothing, and eating the walls
    /// between areas it would open ways no [`Opening`] records, so the pass
    /// fails rather than lay it.
    ///
    /// Every part a lake leaves of what it parts has room to stand in, a
    /// cell with floor all round it, so a lake never leaves a doorway or
    /// a sliver of wall-side floor as an island.
    ///
    /// An area a lake parts stays one [`Area`] at its own index, its
    /// cells every part that is left, because the openings and the survey
    /// name it by that index and a bridge, not a new area, is what joins
    /// the parts.
    pub cut: bool,
    /// The shapes a lake may start in, by [`Area::shape`]: deep water in a
    /// hall and never in the small rooms round it, which a lake of the same
    /// size fills wall to wall. A lake that may cut still spreads from
    /// there into whatever it parts. Empty, any area; on a place with no
    /// areas, anywhere, since there is no shape to ask.
    pub within: Vec<String>,
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
/// to the other is at least [`detour`](Self::detour) steps; then fails
/// the place if any of its floor still cannot be walked to from the rest,
/// every lock open, or if floor of an area that was walked to without a
/// key is now reached only through a lock, so
/// [`Chain::run_retrying`](crate::Chain::run_retrying) grows another.
///
/// A crossing has the liquid on both flanks of every cell it spans, as
/// Brogue's bridges do, so it crosses a lake and never runs along its edge
/// beside a wall or a shore, where it would be a ledge and not a bridge.
/// The one exception is a crossing with no walk round at all, where the
/// liquid parts the floor: joining the place matters more than how the
/// bridge sits.
///
/// No walk round is the longest walk of all, so what [`Lakes::cut`] left
/// apart is joined first; then the longest walk round, as
/// [`OpenLoops`](crate::OpenLoops) opens its doors, each measured again
/// before it is laid, since one bridge shortens the walk round for the
/// crossings beside it, so a lake gains one bridge where one is worth
/// having and not a row of them. Among crossings equally long the pass's
/// own stream draws which. A bank is never an [`Opening`], a cell of a
/// piece painted from a prefab, which is entered only at its entries, or
/// a cell of an area reached from the first only through an opening joined
/// locked or sealed, so a bridge never leads round a lock.
///
/// It runs in [`Phase::Connect`], after [`Lakes`] and before the ways in
/// and out, and publishes each crossing as a [`Bridge`]. A bridge belongs
/// to no [`Area`]: it is a way over a lake, not floor to stand a thing on.
#[derive(Debug, Clone, Copy)]
pub struct Bridges {
    /// A stable name, so two bridgings in one chain draw different
    /// streams; it names the pass when a place is left in parts.
    pub name: &'static str,
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

/// Whether an opening is one no walker gets through until it is opened
/// for them.
fn shuts(o: &Opening) -> bool {
    matches!(o.join, Join::Locked(_) | Join::Sealed)
}

/// Every cell a walker who opens doors gets to: passable, and none of the
/// [`lock_cells`] the exit's own rule never walks through.
fn ways<C: BuildContext>(ctx: &C, passable: &BitGrid) -> BitGrid {
    let mut open = passable.clone();
    for c in lock_cells(ctx).iter() {
        open.set(c, false);
    }
    open
}

/// Whether each area, by index, is reached from the first only through an
/// opening joined locked or sealed. The first area is where growth began,
/// and a piece joined by a lock is attached to what already stood, so it
/// is the side of every lock nobody needs a key for.
fn behind_locks(areas: usize, openings: &[&Opening]) -> Vec<bool> {
    let mut behind = vec![true; areas];
    let mut stack = Vec::new();
    if areas > 0 {
        behind[0] = false;
        stack.push(0);
    }
    while let Some(a) = stack.pop() {
        for o in openings.iter().filter(|o| !shuts(o)) {
            let next = if o.areas.0 == a {
                o.areas.1
            } else if o.areas.1 == a {
                o.areas.0
            } else {
                continue;
            };
            if next < areas && behind[next] {
                behind[next] = false;
                stack.push(next);
            }
        }
    }
    behind
}

/// The cells a bridge never lands on: every opening's cells, every cell
/// of a painted piece with its entries, and every cell of an area behind
/// a lock.
fn landings_barred<C: BuildContext>(ctx: &C) -> BitGrid {
    let mut barred = BitGrid::new(ctx.terrain().width(), ctx.terrain().height());
    let openings: Vec<&Opening> = ctx.outputs().iter::<Opening>().collect();
    for o in &openings {
        for c in &o.cells {
            barred.insert(*c);
        }
    }
    let areas = ctx.outputs().iter::<Area>().map(|a| a.index + 1).max().unwrap_or(0);
    let behind = behind_locks(areas, &openings);
    for a in ctx.outputs().iter::<Area>() {
        let painted = a.entries.iter().flatten();
        if a.entries.is_some() || behind[a.index] {
            for c in a.cells.iter().chain(painted) {
                barred.insert(*c);
            }
        }
    }
    barred
}

/// What a lake keeps off: everything a bridge may not land on, the cells
/// beside every opening, and every stamp with the ring round it.
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
    /// Every way once `draft` is laid.
    fn open_after(&self, passable: &[bool], open: &BitGrid, draft: &Draft) -> BitGrid {
        let mut after = open.clone();
        for i in draft.lake.iter_indices() {
            after.set_idx(i, passable[self.tile.index()]);
        }
        for i in draft.wreath.iter_indices() {
            after.set_idx(i, self.wreath.is_some_and(|(t, _)| passable[t.index()]));
        }
        after
    }

    /// Whether the floor is as whole with `after` the ways once `draft` is
    /// laid as it is now, as much as this laking asks: unless it may cut,
    /// every region keeps a cell and stays one, and the area the lake lies
    /// in stays one piece by itself; and in any case no area loses all its
    /// floor.
    ///
    /// Floor taken away never joins two regions, and rock is taken only
    /// where every way beside it goes under the same lake, so even a lake
    /// a body can cross joins none: as many regions after as before, each
    /// keeping a cell, means none was cut.
    fn keeps_whole(&self, after: &BitGrid, regions: &Regions, owner: Option<&Grid<u32>>, draft: &Draft) -> bool {
        if let Some(owner) = owner {
            let touched: BTreeSet<u32> = draft.lake.iter_indices().chain(draft.wreath.iter_indices()).map(|i| owner.cells()[i]).filter(|o| *o != 0).collect();
            // Each area the lake touches keeps some floor, so every area
            // still has cells to name.
            let mut left = vec![false; owner.cells().iter().copied().max().unwrap_or(0) as usize + 1];
            for i in after.iter_indices() {
                left[owner.cells()[i] as usize] = true;
            }
            if touched.iter().any(|o| !left[*o as usize]) {
                return false;
            }
        }
        if self.cut {
            return parts_are_roomy(after, regions);
        }
        if label_regions(after, |i| after.get_idx(i), Steps::Four).count() != regions.count() {
            return false;
        }
        let mut kept = vec![false; regions.count()];
        for i in after.iter_indices() {
            if let Some(region) = regions.label_idx(i) {
                kept[region as usize - 1] = true;
            }
        }
        if kept.contains(&false) {
            return false;
        }
        // A pond lies in one area, so only that one can be in two.
        let Some(owner) = owner else { return true };
        let Some(own) = draft.lake.iter_indices().map(|i| owner.cells()[i]).find(|o| *o != 0) else { return true };
        let inside = |i: usize| owner.cells()[i] == own && after.get_idx(i);
        let left: Vec<usize> = after.iter_indices().filter(|i| inside(*i)).collect();
        let mut reached = BitGrid::new(after.width(), after.height());
        flood(after, after.idx_point(left[0]), inside, Steps::Four, &mut reached) == left.len()
    }
}

/// Whether every part `after` leaves of a region in `regions` that it
/// parts has room to stand in: a cell with floor all round it.
///
/// Without it a lake over a doorway leaves the door and the cell each side
/// of it as an island, and a lake over most of a room leaves a sliver
/// along its wall: floor nobody would go to, that a bridge must still
/// reach or the place fails. A region the lake does not part is left as it
/// is, so a narrow place it never touches is no reason to refuse it.
fn parts_are_roomy(after: &BitGrid, regions: &Regions) -> bool {
    let parts = label_regions(after, |i| after.get_idx(i), Steps::Four);
    // Taking cells away only refines regions, so each part lies in one
    // region, and a region in more than one part was parted.
    let mut region_of = vec![0u32; parts.count()];
    for i in after.iter_indices() {
        let part = parts.label_idx(i).expect("an open cell has a part") as usize - 1;
        region_of[part] = region_of[part].max(regions.label_idx(i).unwrap_or(0));
    }
    let mut pieces = vec![0u32; regions.count() + 1];
    for r in &region_of {
        pieces[*r as usize] += 1;
    }
    let parted = |part: usize| region_of[part] != 0 && pieces[region_of[part] as usize] > 1;
    if !(0..parts.count()).any(parted) {
        return true;
    }
    let mut roomy = vec![false; parts.count()];
    for i in after.iter_indices() {
        let part = parts.label_idx(i).expect("an open cell has a part") as usize - 1;
        if parted(part) && !roomy[part] {
            roomy[part] = rl_core::geometry::square(after.idx_point(i), 1).all(|q| after.contains(q));
        }
    }
    (0..parts.count()).all(|part| !parted(part) || roomy[part])
}

impl<C: BuildContext> Pass<C> for Lakes {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        if self.cut && tables.passable[self.tile.index()] {
            return Err(BuildError::new(
                self.name,
                "a lake that may cut is of a tile that stops a body; one a body crosses cuts nothing and eats walls into ways no opening records",
            ));
        }
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
        // Which owners a lake may start in, by the same label: every one
        // with no shapes named or no areas to name them by.
        let mut starts = vec![self.within.is_empty() || !has_areas; owner.iter().map(|(_, o)| *o as usize).max().unwrap_or(0) + 1];
        for a in ctx.outputs().iter::<Area>().filter(|a| a.entries.is_none() && self.within.contains(&a.shape)) {
            if let Some(start) = starts.get_mut(a.index + 1) {
                *start = true;
            }
        }
        let may_start = |p: Point| starts.get(whose(p) as usize).copied().unwrap_or(false);
        let interior = bounds.inflate(-1);

        let count = ctx.rng().random_range(self.count.0..=self.count.1);
        for _ in 0..count {
            let floor = |p: Point, own: u32| whose(p) != 0 && (self.cut || whose(p) == own) && open.contains(p) && !kept.contains(p);
            let rock = |p: Point| interior.contains(p) && !cells.contains(p) && !kept.contains(p);
            let seeds: Vec<Point> = interior.cells().filter(|p| may_start(*p) && rl_core::geometry::square(*p, 1).all(|q| floor(q, whose(*p)))).collect();
            if seeds.is_empty() {
                break;
            }
            for _ in 0..TRIES {
                let seed = seeds[ctx.rng().random_range(0..seeds.len())];
                let own = whose(seed);
                let draft = self.draw(ctx.rng(), seed, |p| floor(p, own), rock, &cells);
                let after = self.open_after(&tables.passable, &open, &draft);
                if !self.keeps_whole(&after, &regions, has_areas.then_some(&owner), &draft) {
                    continue;
                }
                // What this lake made impassable: only that leaves an area
                // or a room, so a cell some other pass changed is left to it.
                let wet = |c: Point| (draft.lake.contains(c) || draft.wreath.contains(c)) && !after.contains(c);
                if has_areas {
                    for a in ctx.outputs_mut().iter_mut::<Area>().filter(|a| a.entries.is_none()) {
                        let before = a.cells.len();
                        a.cells.retain(|c| !wet(*c));
                        if a.cells.len() != before {
                            a.bounds = bounds_of(&a.cells);
                            a.inner = largest_rect(&a.cells, a.bounds);
                        }
                    }
                }
                // A room is measured again inside its own rectangle, on a
                // place of areas or not, since what stamps a room reads
                // only the rectangle; one left with no dry cell is no room.
                let mut drowned = false;
                for room in ctx.outputs_mut().iter_mut::<Room>() {
                    let dry: Vec<Point> = room.0.cells().filter(|c| !wet(*c)).collect();
                    if dry.len() as i32 != room.0.area() {
                        room.0 = largest_rect(&dry, room.0);
                        drowned |= dry.is_empty();
                    }
                }
                if drowned {
                    ctx.outputs_mut().retain::<Room>(|room| room.0.area() > 0);
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
            // A pond's rock is its edge and grows nothing further, so it
            // bites into the wall beside its floor and never runs off
            // through the rock. A lake that may cut grows through rock
            // and floor alike, as a blob laid over the map whatever is
            // under it, which is how it reaches past a wall into the next
            // area; the rock it grew through is trimmed off below.
            if !floor(p) && !self.cut {
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
        // Rock is taken only beside floor under the lake, and only where
        // every way beside it, corners too, is under the lake as well: so
        // the lake eats the wall round its own floor, never the wall
        // between it and floor it leaves dry, and never solid rock.
        let mut trimmed = BitGrid::new(w, h);
        for p in grown.iter() {
            let ring = || rl_core::geometry::square(p, 1);
            if floor(p) || ring().all(|q| !passable.contains(q) || grown.contains(q)) && ring().any(|q| grown.contains(q) && floor(q)) {
                trimmed.insert(p);
            }
        }
        // A pond is what stays joined to its seed. A lake that may cut is
        // a blob over the map, and every place it touches floor is part of
        // it, joined under the rock or not.
        let mut lake = trimmed.clone();
        if !self.cut {
            flood(&trimmed, seed, |i| trimmed.get_idx(i), Steps::Four, &mut lake);
        }
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
/// from, which way it runs, how many cells of liquid it spans, whether
/// the liquid flanks every one, and the walk round as last measured, a
/// ceiling on what it is now.
#[derive(Debug, Clone, Copy)]
struct Crossing {
    from: Point,
    facing: Direction,
    span: u32,
    flanked: bool,
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
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Connect
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        if self.tile == self.across {
            return Err(BuildError::new(self.name, "a bridge laid as the liquid it crosses crosses nothing"));
        }
        let bounds = ctx.terrain().bounds();
        let (w, h) = (bounds.width, bounds.height);
        let barred = landings_barred(ctx);
        let mut open = ways(ctx, &passable_cells(ctx));
        let bank = |open: &BitGrid, p: Point| open.contains(p) && !barred.contains(p);

        // Every straight run from a bank over liquid to a bank, found from
        // its west or north end only, so each is found once.
        let terrain = ctx.terrain();
        let liquid = |p: Point| terrain.get(p) == Some(self.across);
        let mut crossings = Vec::new();
        for from in bounds.cells().filter(|p| bank(&open, *p)) {
            for facing in [Direction::East, Direction::South] {
                let flanks = facing.perpendicular().map(|d| d.offset());
                let mut c = Crossing { from, facing, span: 0, flanked: true, detour: UNREACHED };
                while c.span < self.longest && liquid(c.to()) {
                    c.flanked &= flanks.iter().all(|f| liquid(c.to() + *f));
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
        let lay = |ctx: &mut C, c: Crossing| {
            let cells: Vec<Point> = c.cells().collect();
            for p in &cells {
                ctx.terrain_mut().set(*p, self.tile);
            }
            ctx.emit(Bridge { cells, facing: c.facing, detour: c.detour });
        };
        let unbridged = |ctx: &C, c: &Crossing| c.cells().all(|p| ctx.terrain().get(p) == Some(self.across));

        // No walk round is the longest walk of all, so what is apart is
        // joined first: a crossing between two regions, by the region
        // labels and without walking any of them, until none is left.
        loop {
            let regions = label_regions(&open, |i| open.get_idx(i), Steps::Four);
            let apart = |c: &Crossing| regions.label(c.from) != regions.label(c.to());
            let Some(i) = crossings.iter().position(|c| apart(c) && unbridged(ctx, c)) else { break };
            lay(ctx, crossings.remove(i));
            open = ways(ctx, &passable_cells(ctx));
        }

        // Most crossings left are a few steps round, so each is ruled out by
        // a flood that stops at the shortest detour worth a bridge, and only
        // the rest are ever walked in full; and from here on a crossing
        // runs between liquid on both flanks.
        crossings.retain(|c| c.flanked && !walks_under(w, h, |p| open.contains(p), c.from, c.to(), self.detour));
        loop {
            crossings.retain(|c| unbridged(ctx, c));
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
            lay(ctx, crossings.swap_remove(best));
            open = ways(ctx, &passable_cells(ctx));
        }

        // Every lock open, the place must be one: whatever a lake parted
        // and no bridge joined is a place to grow again, never one to hand
        // on with floor nobody can reach.
        let mut joined = passable_cells(ctx);
        for o in ctx.outputs().iter::<Opening>() {
            for c in &o.cells {
                joined.insert(*c);
            }
        }
        let parts = label_regions(&joined, |i| joined.get_idx(i), Steps::Four).count();
        if parts > 1 {
            return Err(BuildError::new(self.name, format!("the floor is in {parts} parts after every bridge that could be laid")));
        }

        // Every lock shut, what was walked to without a key must still be:
        // a lake that parted an area so that some of it is reached only
        // through a lock drawn on a piece cell by cell has put a key on the
        // way to floor that never needed one. The area graph is what the
        // place was before any lake, so the areas it reaches without a lock
        // are the ones that must still be walked between.
        let openings: Vec<&Opening> = ctx.outputs().iter::<Opening>().collect();
        let areas = ctx.outputs().iter::<Area>().map(|a| a.index + 1).max().unwrap_or(0);
        let behind = behind_locks(areas, &openings);
        let open = ways(ctx, &passable_cells(ctx));
        let free: Vec<Point> =
            ctx.outputs().iter::<Area>().filter(|a| !behind[a.index]).flat_map(|a| a.cells.iter().copied()).filter(|p| open.contains(*p)).collect();
        if let Some(start) = free.first() {
            let mut reached = BitGrid::new(w, h);
            flood(&open, *start, |i| open.get_idx(i), Steps::Four, &mut reached);
            let stranded = free.iter().filter(|p| !reached.contains(**p)).count();
            if stranded > 0 {
                return Err(BuildError::new(self.name, format!("{stranded} cells walked to without a key before are reached only through a lock")));
            }
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
    use crate::dungeon::Rooms;
    use crate::loops::OpenLoops;
    use crate::testing::par_seeds;
    use rl_core::{Rect, RunSeed};
    use rl_grid::{Terrain, TileProps, TileRegistry};
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    /// The accretion rig's tiles with a liquid that stops a body, a shore
    /// that does not, and a bridge.
    struct Rig {
        tiles: TileRegistry,
        wall: TileId,
        door: TileId,
        water: TileId,
        shore: TileId,
        bridge: TileId,
        pool: TileId,
    }

    fn rig_with_water() -> Rig {
        let base = rig();
        let mut tiles = base.tiles.clone();
        let water = tiles.register(TileProps::named("still")).unwrap();
        let shore = tiles.register(TileProps::floor("shallow")).unwrap();
        let bridge = tiles.register(TileProps::floor("span")).unwrap();
        let pool = tiles.register(TileProps::named("pool")).unwrap();
        Rig { tiles, wall: base.wall, door: base.door, water, shore, bridge, pool }
    }

    fn lakes(r: &Rig) -> Lakes {
        Lakes { name: "lakes", tile: r.water, count: (2, 5), size: (20, 70), wreath: Some((r.shore, 60)), cut: false, within: Vec::new() }
    }

    fn bridges(r: &Rig) -> Bridges {
        Bridges { name: "bridges", across: r.water, tile: r.bridge, longest: 6, detour: 20 }
    }

    /// Lakes that may part the floor, over about a fifth of it on the
    /// smaller map.
    fn islands(r: &Rig) -> Lakes {
        Lakes { name: "lakes", tile: r.water, count: (12, 20), size: (60, 160), wreath: Some((r.shore, 40)), cut: true, within: Vec::new() }
    }

    const SIZES: [(i32, i32); 2] = [(84, 48), (160, 90)];

    /// A place grown with loops at `seed`, then whatever `more` adds; with
    /// `owed`, one piece painted from a prefab and joined locked.
    fn grown(seed: u64, size: (i32, i32), owed: bool, more: impl FnOnce(Chain<BaseContext>, &Rig) -> Chain<BaseContext>) -> BaseContext {
        attempt(seed, size, owed, 20, more).unwrap_or_else(|e| panic!("seed {seed}: {e}"))
    }

    /// [`grown`], given `attempts` and handing back the failure.
    fn attempt(
        seed: u64,
        (w, h): (i32, i32),
        owed: bool,
        attempts: u32,
        more: impl FnOnce(Chain<BaseContext>, &Rig) -> Chain<BaseContext>,
    ) -> Result<BaseContext, BuildError> {
        let r = rig_with_water();
        let growth = if owed { owing(&rig(), Join::Locked(7)) } else { accrete(&rig(), 3) };
        let chain = more(Chain::new().then(growth).then(OpenLoops { door: r.door, min_detour: 20, max: 6 }), &r);
        chain.run_retrying(|| BaseContext::blank(w, h, r.tiles.clone(), r.wall), RunSeed(seed), attempts)
    }

    /// The parts of `terrain` with every lock open: every passable cell
    /// and every opening's cells, whatever they were laid as.
    fn parts(ctx: &BaseContext, terrain: &Terrain) -> usize {
        let passable = ctx.tiles().tables().passable;
        let openings: Vec<Point> = ctx.outputs().iter::<Opening>().flat_map(|o| o.cells.clone()).collect();
        let open = |i: usize| passable[terrain.get_idx(i).index()] || openings.contains(&terrain.idx_point(i));
        label_regions(terrain, open, Steps::Four).count()
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
            let (places, wet) = (AtomicUsize::new(0), AtomicUsize::new(0));
            par_seeds(0..200, |s| {
                let dry = grown(s, size, false, |c, _| c);
                let lakes = grown(s, size, false, |c, r| c.then(lakes(r)));
                let mut terrain = dry.terrain().clone();
                assert_eq!(regions(&terrain, &r.tiles, &[]), 1, "{size:?} seed {s}: grown in one piece");
                for (i, lake) in lakes.outputs().iter::<Lake>().enumerate() {
                    lay(&mut terrain, lake, &r);
                    assert_eq!(regions(&terrain, &r.tiles, &[]), 1, "{size:?} seed {s}: lake {i} parts the floor");
                }
                assert_eq!(&terrain, lakes.terrain(), "{size:?} seed {s}: every lake laid is published");
                places.fetch_add(1, Relaxed);
                wet.fetch_add(usize::from(lakes.outputs().first::<Lake>().is_some()), Relaxed);
            });
            let (places, wet) = (places.into_inner(), wet.into_inner());
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
        par_seeds(0..60, |s| {
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
        });
    }

    #[test]
    fn a_wreath_only_borders_its_own_lake_and_only_over_floor() {
        let r = rig_with_water();
        let walkable = r.tiles.tables().walkable;
        let shores = AtomicUsize::new(0);
        for size in SIZES {
            par_seeds(0..200, |s| {
                let dry = grown(s, size, false, |c, _| c);
                let wet = grown(s, size, false, |c, r| c.then(lakes(r)));
                for lake in wet.outputs().iter::<Lake>() {
                    for p in &lake.wreath {
                        assert!(rl_core::geometry::square(*p, 1).any(|q| lake.cells.contains(&q)), "{size:?} seed {s}: {p:?} borders no cell of its lake");
                        assert!(walkable[dry.terrain().get(*p).unwrap().index()], "{size:?} seed {s}: {p:?} was not floor");
                        assert_eq!(wet.terrain().get(*p), Some(r.shore), "{size:?} seed {s}: {p:?} was laid over by another lake");
                    }
                    shores.fetch_add(lake.wreath.len(), Relaxed);
                }
            });
        }
        assert!(shores.into_inner() > 0, "no lake on any seed has a shore");
    }

    #[test]
    fn a_bridge_spans_at_most_its_longest_and_is_laid_only_where_the_walk_round_was_at_least_its_detour() {
        let r = rig_with_water();
        let pass = bridges(&r);
        let passable = r.tiles.tables().passable;
        let laid = AtomicUsize::new(0);
        for size in SIZES {
            par_seeds(0..200, |s| {
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
                    laid.fetch_add(1, Relaxed);
                }
                assert_eq!(&terrain, bridged.terrain(), "{size:?} seed {s}: every bridge laid is published");
            });
        }
        assert!(laid.into_inner() > 0, "no bridge on any seed");
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
            par_seeds(0..200, |s| {
                let plain = grown(s, size, false, |c, r| c.then(lakes(r)));
                let more = grown(s, size, false, |c, r| c.then(Idle).then(lakes(r)).then(bridges(r)));
                let (a, b): (Vec<&Lake>, Vec<&Lake>) = (plain.outputs().iter().collect(), more.outputs().iter().collect());
                assert_eq!(a, b, "{size:?} seed {s}");
            });
        }
    }

    #[test]
    fn a_bridge_of_the_liquid_it_crosses_is_refused() {
        let r = rig_with_water();
        let mut ctx = BaseContext::blank(8, 8, r.tiles.clone(), r.wall);
        let pass = Bridges { name: "bridges", across: r.water, tile: r.water, longest: 4, detour: 2 };
        assert!(pass.apply(&mut ctx).is_err());
    }

    /// With lakes that may cut over a large share of the floor, some
    /// place has an island, floor only a bridge reaches, and every place
    /// built is one piece with its locks open: the bridges joined it all.
    #[test]
    fn a_lake_that_may_cut_leaves_islands_and_the_bridges_join_every_one() {
        let r = rig_with_water();
        for (size, seeds) in [(SIZES[0], 0..200), (SIZES[1], 0..20)] {
            let islanded = AtomicUsize::new(0);
            par_seeds(seeds, |s| {
                let ctx = grown(s, size, false, |c, r| c.then(islands(r)).then(bridges(r)));
                assert_eq!(parts(&ctx, ctx.terrain()), 1, "{size:?} seed {s}: left in parts");
                let mut unbridged = ctx.terrain().clone();
                for b in ctx.outputs().iter::<Bridge>() {
                    for p in &b.cells {
                        unbridged.set(*p, r.water);
                    }
                }
                islanded.fetch_add(usize::from(parts(&ctx, &unbridged) > 1), Relaxed);
                for a in ctx.outputs().iter::<Area>() {
                    assert!(!a.cells.is_empty(), "{size:?} seed {s}: area {} is all under water", a.index);
                    assert!(a.cells.iter().all(|c| r.tiles.tables().passable[ctx.terrain().get(*c).unwrap().index()]), "{size:?} seed {s}: area {}", a.index);
                }
            });
            assert!(islanded.into_inner() > 0, "{size:?}: no place has an island");
        }
    }

    /// A lake never parts what lies behind a lock and a bridge never lands
    /// there, so with the lock shut the piece behind it is still out of
    /// reach of the first area, and with it open the place is one piece.
    #[test]
    fn a_bridge_never_leads_round_a_lock() {
        let r = rig_with_water();
        let passable = r.tiles.tables().passable;
        par_seeds(0..60, |s| {
            let ctx = grown(s, SIZES[0], true, |c, r| c.then(islands(r)).then(bridges(r)));
            assert_eq!(parts(&ctx, ctx.terrain()), 1, "seed {s}: left in parts");
            let shut: Vec<Point> = ctx.outputs().iter::<Opening>().filter(|o| shuts(o)).flat_map(|o| o.cells.clone()).collect();
            let terrain = ctx.terrain();
            let first = ctx.outputs().iter::<Area>().find(|a| a.index == 0).unwrap().cells[0];
            let mut reached = BitGrid::new(terrain.width(), terrain.height());
            let open = |i: usize| passable[terrain.get_idx(i).index()] && !shut.contains(&terrain.idx_point(i));
            flood(terrain, first, open, Steps::Four, &mut reached);
            for a in ctx.outputs().iter::<Area>().filter(|a| a.entries.is_some()) {
                assert!(a.cells.iter().all(|c| !reached.contains(*c)), "seed {s}: the locked piece is reached with its lock shut");
            }
            for b in ctx.outputs().iter::<Bridge>() {
                let step = b.facing.offset();
                let banks = [b.cells[0].offset(-step.x, -step.y), b.cells[b.cells.len() - 1].offset(step.x, step.y)];
                assert!(banks.iter().all(|p| reached.contains(*p)), "seed {s}: a bridge lands behind a lock");
            }
        });
    }

    /// Two rooms either side of four cells of liquid: a bridge three long
    /// cannot join them, so the pass fails naming itself; four long can.
    #[test]
    fn a_place_no_bridge_can_join_fails_naming_the_pass() {
        let r = rig_with_water();
        let banded = || {
            let mut ctx = BaseContext::blank(14, 5, r.tiles.clone(), r.wall);
            for p in rl_core::Rect::new(1, 1, 12, 3).cells() {
                ctx.terrain_mut().set(p, if (5..9).contains(&p.x) { r.water } else { r.tiles.expect("floor") });
            }
            ctx
        };
        let short = Bridges { name: "spans", across: r.water, tile: r.bridge, longest: 3, detour: 20 };
        let err = Chain::new().then(short).run(&mut banded(), RunSeed(1)).unwrap_err();
        assert_eq!(err.pass, "spans");
        let mut ctx = banded();
        Chain::new().then(Bridges { longest: 4, ..short }).run(&mut ctx, RunSeed(1)).unwrap();
        assert_eq!(ctx.outputs().iter::<Bridge>().count(), 1, "one bridge joins it, and none beside it is worth laying");
    }

    /// A place a bridge cannot join is not handed on: the attempt fails,
    /// and the next attempt of the same seed grows one that can be.
    #[test]
    fn a_place_left_in_parts_is_grown_again() {
        let mut retried = 0;
        for s in 0..30 {
            let short = |c: Chain<BaseContext>, r: &Rig| c.then(islands(r)).then(Bridges { longest: 2, ..bridges(r) });
            // Growth may fail a first attempt of its own; only the ones the
            // bridges failed are the case here.
            if let Err(e) = attempt(s, SIZES[0], false, 1, short)
                && e.pass == "bridges"
            {
                let ctx = attempt(s, SIZES[0], false, 40, short).unwrap_or_else(|e| panic!("seed {s}: {e}"));
                assert_eq!(parts(&ctx, ctx.terrain()), 1, "seed {s}");
                retried += 1;
            }
        }
        assert!(retried > 0, "no first attempt was left in parts");
    }

    /// Each laking draws from its own name's stream, so a chain may hold
    /// lakes of one liquid and small pools of another.
    #[test]
    fn two_lakings_in_one_chain_each_lay_their_own() {
        let mut both = 0;
        for s in 0..20 {
            let ctx = grown(s, SIZES[0], false, |c, r| {
                c.then(lakes(r)).then(Lakes { name: "pools", tile: r.pool, size: (4, 9), count: (3, 6), wreath: None, ..lakes(r) })
            });
            let r = rig_with_water();
            let laid = |t: TileId| ctx.terrain().iter().any(|(_, at)| at == t);
            both += usize::from(laid(r.water) && laid(r.pool));
        }
        assert!(both >= 15, "only {both} of 20 places hold both");
    }

    /// The areas as they stood before any lake, kept by a pass that runs
    /// ahead of the lakes, so a test can ask what a lake took.
    struct Before(Vec<Area>);

    struct Record;

    impl Pass<BaseContext> for Record {
        fn name(&self) -> &'static str {
            "record"
        }
        fn phase(&self) -> Phase {
            Phase::Structures
        }
        fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
            let areas = ctx.outputs().iter::<Area>().cloned().collect();
            ctx.emit(Before(areas));
            Ok(())
        }
    }

    /// The usual growth, with a rectangle drawn cell by cell and joined by
    /// a lock among its shapes: growth goes on past it, so what grows on
    /// from it lies behind the lock too.
    fn vaulted() -> crate::Accrete {
        let base = rig();
        let mut pass = accrete(&base, 3);
        pass.shapes.push(Box::new(crate::accrete::Rectangle { name: "vault".into(), w: (5, 8), h: (5, 7), join: Join::Locked(5) }));
        let vault = pass.shapes.len() - 1;
        for d in &mut pass.districts {
            d.shapes.push((vault, 2));
        }
        pass.locked = base.tiles.id("locked").expect("registered by the rig");
        pass
    }

    /// No lake covers and no bridge lands on an area reached from the first
    /// only through a lock drawn on a piece cell by cell, and with every
    /// lock shut every cell of the areas reached without one is still
    /// walked to from the rest.
    #[test]
    fn no_lake_or_bridge_reaches_past_a_lock_on_a_piece_drawn_cell_by_cell() {
        let r = rig_with_water();
        let passable = r.tiles.tables().passable;
        let locked_away = AtomicUsize::new(0);
        par_seeds(0..80, |s| {
            let chain = Chain::new().then(vaulted()).then(OpenLoops { door: r.door, min_detour: 20, max: 6 }).then(Record).then(islands(&r)).then(bridges(&r));
            let ctx = chain.run_retrying(|| BaseContext::blank(84, 48, r.tiles.clone(), r.wall), RunSeed(s), 40).unwrap_or_else(|e| panic!("seed {s}: {e}"));
            let before = &ctx.outputs().first::<Before>().unwrap().0;
            let openings: Vec<&Opening> = ctx.outputs().iter::<Opening>().collect();
            // Walked to from the first area through openings with no lock.
            let mut free = vec![false; before.len()];
            free[0] = true;
            let mut stack = vec![0];
            while let Some(a) = stack.pop() {
                for o in openings.iter().filter(|o| !matches!(o.join, Join::Locked(_) | Join::Sealed)) {
                    for (x, y) in [(o.areas.0, o.areas.1), (o.areas.1, o.areas.0)] {
                        if x == a && !free[y] {
                            free[y] = true;
                            stack.push(y);
                        }
                    }
                }
            }
            let kept: Vec<Point> = before.iter().filter(|a| !free[a.index]).flat_map(|a| a.cells.iter().copied()).collect();
            locked_away.fetch_add(usize::from(before.iter().any(|a| !free[a.index] && a.entries.is_none())), Relaxed);
            for lake in ctx.outputs().iter::<Lake>() {
                assert!(lake.cells.iter().chain(&lake.wreath).all(|p| !kept.contains(p)), "seed {s}: a lake lies behind a lock");
            }
            for b in ctx.outputs().iter::<Bridge>() {
                let step = b.facing.offset();
                let banks = [b.cells[0].offset(-step.x, -step.y), b.cells[b.cells.len() - 1].offset(step.x, step.y)];
                assert!(banks.iter().all(|p| !kept.contains(p)), "seed {s}: a bridge lands behind a lock");
            }
            let terrain = ctx.terrain();
            let shut: Vec<Point> = openings.iter().filter(|o| matches!(o.join, Join::Locked(_) | Join::Sealed)).flat_map(|o| o.cells.clone()).collect();
            let open = |i: usize| passable[terrain.get_idx(i).index()] && !shut.contains(&terrain.idx_point(i));
            let walked: Vec<Point> = ctx
                .outputs()
                .iter::<Area>()
                .filter(|a| free[a.index])
                .flat_map(|a| a.cells.iter().copied())
                .filter(|p| terrain.checked_idx(*p).is_some_and(open))
                .collect();
            let mut reached = BitGrid::new(terrain.width(), terrain.height());
            flood(terrain, walked[0], open, Steps::Four, &mut reached);
            assert!(walked.iter().all(|p| reached.contains(*p)), "seed {s}: floor once walked to is reached only through a lock");
        });
        let locked_away = locked_away.into_inner();
        assert!(locked_away >= 10, "only {locked_away} of 80 places have an area drawn cell by cell behind a lock");
    }

    #[test]
    fn a_lake_that_may_cut_of_a_tile_a_body_crosses_is_refused() {
        let r = rig_with_water();
        let mut ctx = BaseContext::blank(8, 8, r.tiles.clone(), r.wall);
        let pass = Lakes { tile: r.shore, ..islands(&r) };
        assert_eq!(pass.apply(&mut ctx).unwrap_err().pass, "lakes");
    }

    /// Room `A` opens on `C` by a door and on `B` by a lock, and `C` opens on
    /// `B` by a second lock. Liquid too wide to bridge parts `C`, so its far
    /// part is one piece with the rest only through `B`, every lock open,
    /// and reached only through a lock with them shut: the place fails.
    #[test]
    fn a_place_whose_lake_left_free_floor_behind_a_lock_fails_naming_the_pass() {
        let r = rig_with_water();
        let (floor, locked) = (r.tiles.expect("floor"), r.tiles.id("locked").unwrap());
        let mut ctx = BaseContext::blank(20, 5, r.tiles.clone(), r.wall);
        let row = |x0: i32, x1: i32, y: i32| (x0..=x1).map(move |x| Point::new(x, y)).collect::<Vec<_>>();
        let a = row(1, 3, 1);
        let c: Vec<Point> = row(5, 7, 1).into_iter().chain(row(12, 13, 1)).collect();
        let mut b = row(15, 17, 1);
        b.extend([Point::new(17, 2)]);
        b.extend(row(1, 17, 3));
        for p in a.iter().chain(&c).chain(&b) {
            ctx.terrain_mut().set(*p, floor);
        }
        for p in row(8, 11, 1) {
            ctx.terrain_mut().set(p, r.water);
        }
        ctx.terrain_mut().set(Point::new(4, 1), r.door);
        ctx.terrain_mut().set(Point::new(14, 1), locked);
        ctx.terrain_mut().set(Point::new(2, 2), locked);
        for (index, cells) in [a, c, b].into_iter().enumerate() {
            let bounds = bounds_of(&cells);
            ctx.emit(Area { index, shape: "room".into(), district: "d".into(), inner: bounds, bounds, cells, entries: None });
        }
        ctx.emit(Opening { cells: vec![Point::new(4, 1)], facing: Direction::East, areas: (0, 1), join: Join::Door });
        ctx.emit(Opening { cells: vec![Point::new(14, 1)], facing: Direction::East, areas: (1, 2), join: Join::Locked(1) });
        ctx.emit(Opening { cells: vec![Point::new(2, 2)], facing: Direction::South, areas: (0, 2), join: Join::Locked(2) });
        let pass = Bridges { name: "spans", across: r.water, tile: r.bridge, longest: 2, detour: 20 };
        let err = Chain::new().then(pass).run(&mut ctx, RunSeed(1)).unwrap_err();
        assert_eq!(err.pass, "spans");
        assert!(err.reason.contains("only through a lock"), "{}", err.reason);
    }

    /// A lake kept within a shape starts only in an area of that shape,
    /// and one that may not cut stays there: no floor of an area of any
    /// other shape goes under it, on either map size.
    #[test]
    fn a_lake_within_a_shape_lies_only_in_areas_of_that_shape() {
        let mut laid = 0;
        for size in SIZES {
            for s in 0..12 {
                let dry = grown(s, size, false, |c, _| c);
                let within = grown(s, size, false, |c, r| c.then(Lakes { within: vec!["big".into()], ..lakes(r) }));
                let elsewhere: BTreeSet<Point> = dry.outputs().iter::<Area>().filter(|a| a.shape != "big").flat_map(|a| a.cells.iter().copied()).collect();
                for lake in within.outputs().iter::<Lake>() {
                    laid += 1;
                    assert!(lake.cells.iter().all(|p| !elsewhere.contains(p)), "{size:?} seed {s}: a lake in an area not drawn by \"big\"");
                }
            }
        }
        assert!(laid > 0, "some lake was laid in a big area");
    }

    /// A lake that may cut, kept within a shape, starts only in an area of
    /// that shape and still spreads from there into what it parts: kept to
    /// a shape no area is drawn by, nothing is laid; kept to "big", every
    /// lake reaches into a big area and some reach past one. Each place is
    /// a first attempt, so the dry place is the same one the lakes are laid
    /// over.
    #[test]
    fn a_lake_that_may_cut_within_a_shape_starts_there_and_spreads_past_it() {
        let (mut laid, mut past) = (0, 0);
        for s in 0..16 {
            let nowhere = grown(s, SIZES[0], false, |c, r| c.then(Lakes { within: vec!["drawn by none".into()], ..islands(r) }));
            assert_eq!(nowhere.outputs().iter::<Lake>().count(), 0, "seed {s}: a lake started in no area of the shape named");
            let (Ok(dry), Ok(within)) =
                (attempt(s, SIZES[0], false, 1, |c, _| c), attempt(s, SIZES[0], false, 1, |c, r| c.then(Lakes { within: vec!["big".into()], ..islands(r) })))
            else {
                continue;
            };
            let big: BTreeSet<Point> = dry.outputs().iter::<Area>().filter(|a| a.shape == "big").flat_map(|a| a.cells.iter().copied()).collect();
            for lake in within.outputs().iter::<Lake>() {
                laid += 1;
                assert!(lake.cells.iter().any(|p| big.contains(p)), "seed {s}: a lake that touches no big area");
                past += usize::from(lake.cells.iter().any(|p| !big.contains(p)));
            }
        }
        assert!(laid > 0, "some lake was laid in a big area");
        assert!(past > 0, "no lake that may cut spread past the big area it started in");
    }

    /// A bridge is a way over a lake, not somewhere to arrive or leave:
    /// over a span of seeds of islands, neither the start nor the exit is
    /// ever on one.
    #[test]
    fn neither_way_stands_on_a_bridge() {
        use crate::dungeon::{ExitPoint, FarthestExit, RandomStart};
        use crate::passes::StartPoint;
        par_seeds(0..40, |s| {
            let ctx = grown(s, SIZES[0], false, |c, r| c.then(islands(r)).then(bridges(r)).then(RandomStart).then(FarthestExit));
            let spans: BTreeSet<Point> = ctx.outputs().iter::<Bridge>().flat_map(|b| b.cells.iter().copied()).collect();
            let start = ctx.outputs().first::<StartPoint>().unwrap().0;
            let exit = ctx.outputs().first::<ExitPoint>().unwrap().0;
            assert!(!spans.contains(&start), "seed {s}: the start at {start:?} is on a bridge");
            assert!(!spans.contains(&exit), "seed {s}: the exit at {exit:?} is on a bridge");
        });
    }

    /// A room a lake was laid in is measured again round the water, as
    /// its area is, so what stamps a room or draws a cell from one stands
    /// on dry floor: over a span of seeds of ponds and of islands, every
    /// cell of every room can be stood on, and some room moved.
    #[test]
    fn a_room_a_lake_lies_in_is_measured_again_round_it() {
        let passable = rig_with_water().tiles.tables().passable;
        let moved = AtomicUsize::new(0);
        par_seeds(0..40, |s| {
            let dry = grown(s, SIZES[0], false, |c, _| c);
            let rooms = |ctx: &BaseContext| ctx.outputs().iter::<Room>().map(|r| r.0).collect::<Vec<_>>();
            for (kind, wet) in
                [("ponds", grown(s, SIZES[0], false, |c, r| c.then(lakes(r)))), ("islands", grown(s, SIZES[0], false, |c, r| c.then(islands(r))))]
            {
                for room in rooms(&wet) {
                    let under = room.cells().find(|c| !passable[wet.terrain().get(*c).unwrap().index()]);
                    assert!(under.is_none(), "{kind} seed {s}: room {room:?} holds {under:?}, which no body stands on");
                }
                moved.fetch_add(usize::from(rooms(&wet) != rooms(&dry)), Relaxed);
            }
        });
        let moved = moved.into_inner();
        assert!(moved > 10, "only {moved} places had a room a lake lay in, or this proves little");
    }

    /// Small rooms and corridors with no areas at all, then `more`: a
    /// place where nothing but the rooms say where the floor is.
    fn roomed(seed: u64, more: impl FnOnce(Chain<BaseContext>, &Rig) -> Chain<BaseContext>) -> Result<BaseContext, BuildError> {
        let r = rig_with_water();
        let floor = rig().floor;
        let rooms = Rooms { floor, attempts: 60, min_size: 2, max_size: 4, min_rooms: 3 };
        more(Chain::new().then(rooms), &r).run_retrying(|| BaseContext::blank(40, 24, r.tiles.clone(), r.wall), RunSeed(seed), 20)
    }

    /// A lake that covers a room whole leaves no room there, the rooms
    /// that remain keep their order, and every room cell on a place with
    /// no areas is one a body can stand on.
    #[test]
    fn a_lake_that_covers_a_room_whole_drops_it_and_the_rest_keep_their_order_on_a_place_with_no_areas() {
        let passable = rig_with_water().tiles.tables().passable;
        let (dropped, shrunk) = (AtomicUsize::new(0), AtomicUsize::new(0));
        par_seeds(0..120, |s| {
            let dry = roomed(s, |c, _| c).unwrap();
            let Ok(wet) = roomed(s, |c, r| c.then(islands(r))) else { return };
            assert!(wet.outputs().first::<Area>().is_none(), "seed {s}: this rig has no areas");
            let before: Vec<Rect> = dry.outputs().iter::<Room>().map(|r| r.0).collect();
            let after: Vec<Rect> = wet.outputs().iter::<Room>().map(|r| r.0).collect();
            // Each room left lies in a distinct earlier one, in order.
            let mut at = 0;
            for room in &after {
                let inside = |b: &Rect| b.contains(room.origin()) && b.contains(Point::new(room.right() - 1, room.bottom() - 1));
                let found = before[at..].iter().position(inside);
                at += found.unwrap_or_else(|| panic!("seed {s}: room {room:?} is in none of {before:?} after the ones before it")) + 1;
                let under = room.cells().find(|c| !passable[wet.terrain().get(*c).unwrap().index()]);
                assert!(under.is_none(), "seed {s}: room {room:?} holds {under:?}, which no body stands on");
            }
            dropped.fetch_add(usize::from(after.len() < before.len()), Relaxed);
            shrunk.fetch_add(usize::from(after.len() == before.len() && after != before), Relaxed);
        });
        let (dropped, shrunk) = (dropped.into_inner(), shrunk.into_inner());
        assert!(dropped > 0, "no lake covered a room whole in 120 places, or this proves little ({shrunk} rooms only shrank)");
    }
}
