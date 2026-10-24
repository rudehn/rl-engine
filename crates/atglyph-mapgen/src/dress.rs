//! Dressing the areas a place grew: things along their walls, in their
//! corners, in their centers, in rows or scattered, at a density, on the
//! area's own cells, so one dressing fills a small room, a long hall and a
//! cave alike.
//!
//! A [`Dressing`] is a list of [`Thing`]s, each a glyph that stands as a
//! tile or as a slot, a [`Spread`] and a density, and an optional repaint
//! of the floor left clear. [`Dress`] lays the dressings a pool gave an
//! area first, each [`DressAt`] in its own area, then dresses every other
//! room no stamp took from a weighted list by its district, nothing among
//! the choices. A slot is emitted as a stamp's mark is, a [`Stamped`] of
//! one cell carrying the dressing's key, so whoever fills a stamp's slots
//! fills a dressing's the same way.
//!
//! Two rules hold whatever a dressing asks: nothing stands on or beside
//! an opening, a hidden one included since it is an opening once found,
//! or on or beside the way in or the way down; and nothing cuts an area,
//! checked by flooding the area before and after each thing that blocks
//! and taking back the one that cut it or that left a slot, its own or one
//! laid before it, with no open cell beside it. The file, its names and its
//! checks are `atglyph-rules`'s; this reads only what growth and the passes
//! after it published.

use atglyph_core::{Direction, GridRect, Point};
use atglyph_grid::TileId;
use rand::Rng;
use rand::seq::SliceRandom;

use crate::accrete::{Area, Opening};
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BaseContext;
use crate::dungeon::{ExitPoint, Room};
use crate::passes::StartPoint;
use crate::pool::DressAt;
use crate::prefab::{Stamped, pick_weighted};
use crate::secrets::SecretCell;

/// Where a thing goes in an area.
///
/// A wall here is anything beside a cell that is not the area's own floor
/// a body can stand on: rock, a lake, the wall of the next piece. So a
/// cave's ragged edge is all wall and corner, and a lake's shore is wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum Spread {
    /// A cell with a wall straight beside it, not a corner.
    Walls,
    /// A cell with walls on two sides at a right angle.
    Corners,
    /// The cells farthest from every wall, counting corners, and at least
    /// two from one: one cell in a five-cell square, a strip down a long
    /// hall, the heart of a cave, and none in a room too thin to have a
    /// middle off its walls.
    Center,
    /// Lines along the longer side of the area's bounds over the cells
    /// two or more from every wall, with `aisle` lines left clear between
    /// two, so the cells along the walls and every aisle stay a way round.
    Rows {
        /// The clear lines between two lines of things.
        aisle: u32,
    },
    /// Any cell of the area.
    Scatter,
}

/// What a thing is once it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stand {
    /// This tile, painted on its cell.
    Tile(TileId),
    /// A slot: a mark on its cell, emitted as a [`Stamped`] mark for
    /// whoever fills the dressing's slots. What it holds, a prop that
    /// blocks or an item that does not, is not this crate's to know, so
    /// the check that nothing cuts an area takes it both ways: it blocks
    /// the area's other cells, and its own cell keeps an open cell
    /// straight beside it, from which an item in it is stepped onto and a
    /// thing in it is used. Taken only the first way, a slot holding an
    /// item could be shut in by things laid after it, which the area's
    /// count of regions never sees, since the slot was never counted.
    Slot,
}

/// One rule of a dressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thing {
    /// The glyph, which is the mark a slot is emitted as.
    pub glyph: char,
    /// What it is.
    pub stands: Stand,
    /// Where it goes.
    pub at: Spread,
    /// The share in a hundred of the cells `at` offers that it fills,
    /// rounded; fewer where the cells it may take run out first.
    pub density_pct: u32,
}

/// A dressing: what fills an area of any size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dressing {
    /// The name a [`DressAt`] names it by.
    pub name: String,
    /// The rules, laid in order, so an earlier rule's things stand when a
    /// later one looks for room.
    pub things: Vec<Thing>,
    /// The tile and the share in a hundred of [`Dress::floor`] left clear
    /// that is repainted with it once every thing stands. A body must be
    /// able to stand on it, since a repaint is never checked for cutting,
    /// and [`Dress`] fails on one it could not.
    pub repaint: Option<(TileId, u32)>,
    /// An opaque number its owner set, carried to [`Stamped::prefab`] on
    /// every slot, so the slot's meaning is found again; `None` for a
    /// dressing nobody keyed.
    pub key: Option<u32>,
}

/// An area a dressing was laid in, by [`Area::index`], and the dressing's
/// name: what a game reads to say what a room was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dressed {
    /// The area.
    pub area: usize,
    /// The dressing.
    pub dressing: String,
}

/// Dresses a place's areas: each [`DressAt`] first, in its named area,
/// and then every free room by its district's list.
///
/// A free room is an area with a [`Room`] its growth emitted on its floor,
/// as every shape but a passage has, that no [`Stamped`] covers a cell of,
/// no `DressAt` took and that was not painted from a prefab. Its dressing
/// is drawn by weight from the [`DistrictDressings`] of the district it
/// was grown in, which may leave it bare; a district with no list leaves its
/// rooms bare. The rooms are dressed in the order they were laid, each
/// emitting a [`Dressed`].
///
/// Runs in [`Phase::Finish`], after [`StampPicks`](crate::StampPicks),
/// whose `DressAt`s it lays, and after anything stamped, which it keeps
/// out of; fails on a `DressAt` naming a dressing or an area there is not.
#[derive(Debug, Clone)]
pub struct Dress {
    /// A stable name, so two passes in one chain draw different streams.
    pub name: &'static str,
    /// The floor a repaint repaints and a [`Stand::Tile`] is laid on; any
    /// other tile, a shore or a bridge, is left as it is.
    pub floor: TileId,
    /// Every dressing a list or a `DressAt` may name.
    pub dressings: Vec<Dressing>,
    /// Each district's list.
    pub districts: Vec<DistrictDressings>,
}

/// What the free rooms grown in one district are dressed with: each
/// dressing by name at a weight, and nothing at all at a weight of its own.
///
/// By name and not by place in [`Dress::dressings`], as a [`DressAt`] is,
/// so a game reads its lists straight from its own file, as
/// `(district: "halls", nothing: 2, dressings: [("barracks", 3)])` with
/// `nothing` 0 when left out, and `atglyph-rules`' `check_districts` holds the
/// names to the dressing file at load.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DistrictDressings {
    /// The district, by [`Area::district`].
    pub district: String,
    /// The weight of leaving a room bare.
    #[serde(default)]
    pub nothing: u32,
    /// Each dressing, by its [`Dressing::name`], and its weight.
    pub dressings: Vec<(String, u32)>,
}

impl DistrictDressings {
    /// `district` dressed with `dressings`, never left bare.
    pub fn new(district: impl Into<String>, dressings: Vec<(String, u32)>) -> Self {
        Self { district: district.into(), nothing: 0, dressings }
    }

    /// This list, with nothing at all drawn at `weight`.
    pub fn or_nothing(mut self, weight: u32) -> Self {
        self.nothing = weight;
        self
    }
}

impl Pass for Dress {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Finish
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let err = |reason: String| BuildError::new(self.name, reason);
        let areas: Vec<Area> = ctx.outputs().iter::<Area>().cloned().collect();
        let rooms: Vec<GridRect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
        let stamps: Vec<GridRect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
        let given: Vec<DressAt> = ctx.outputs().iter::<DressAt>().cloned().collect();
        // Every cell nothing may stand on or beside: the openings, hidden
        // ones too, and the two ways.
        let keep: Vec<Point> = ctx
            .outputs()
            .iter::<Opening>()
            .flat_map(|o| o.cells.iter().copied())
            .chain(ctx.outputs().iter::<SecretCell>().map(|s| s.at))
            .chain(ctx.outputs().iter::<StartPoint>().map(|s| s.0))
            .chain(ctx.outputs().iter::<ExitPoint>().map(|e| e.0))
            .collect();

        let walkable = ctx.tiles().tables().walkable;
        if let Some(d) = self.dressings.iter().find(|d| d.repaint.is_some_and(|(t, _)| !walkable[t.index()])) {
            return Err(err(format!("dressing {:?} repaints the floor with a tile no body stands on, and a repaint is never checked for cutting", d.name)));
        }
        let index = |name: &str| self.dressings.iter().position(|d| d.name == name).ok_or_else(|| err(format!("no dressing is called {name:?}")));
        // Each list as indices, so a name no dressing has fails the pass
        // before anything is laid, not on the roll that first draws it.
        let lists = self
            .districts
            .iter()
            .map(|l| Ok((l, l.dressings.iter().map(|(name, w)| Ok((index(name)?, *w))).collect::<Result<Vec<_>, BuildError>>()?)))
            .collect::<Result<Vec<_>, BuildError>>()?;
        // Each area to dress and the dressing, by index: the given first.
        let mut laid: Vec<(usize, usize)> = Vec::new();
        for at in &given {
            let dressing = index(&at.dressing)?;
            if !areas.iter().any(|a| a.index == at.area) {
                return Err(err(format!("{:?} is given area {}, and there is none", at.dressing, at.area)));
            }
            laid.push((at.area, dressing));
        }
        // Pieces' floors never share a cell, so a room's rectangle on an
        // area's floor is that area's own, as `HideOpenings` reads it.
        let is_room = |a: &Area| a.cells.iter().any(|c| rooms.iter().any(|r| r.contains(*c)));
        let stamped = |a: &Area| a.cells.iter().any(|c| stamps.iter().any(|s| s.contains(*c)));
        for area in &areas {
            let free = area.entries.is_none() && is_room(area) && !stamped(area) && !given.iter().any(|g| g.area == area.index);
            let Some((list, dressings)) = lists.iter().find(|(l, _)| l.district == area.district).filter(|_| free) else { continue };
            // Nothing is the first choice, so a roll on it lays nothing.
            let weights: Vec<u32> = std::iter::once(list.nothing).chain(dressings.iter().map(|(_, w)| *w)).collect();
            let total: u32 = weights.iter().sum();
            if total == 0 {
                continue;
            }
            let roll = ctx.rng().random_range(0..total);
            if let Some(k) = pick_weighted(&weights, roll).checked_sub(1) {
                laid.push((area.index, dressings[k].0));
            }
        }
        for (index, dressing) in laid {
            let area = areas.iter().find(|a| a.index == index).expect("checked above");
            let d = &self.dressings[dressing];
            let marks = dress_area(ctx, area, d, &keep, self.floor);
            for (glyph, at) in marks {
                ctx.emit(Stamped { bounds: GridRect::new(at.x, at.y, 1, 1), marks: vec![(glyph, at)], prefab: d.key });
            }
            ctx.emit(Dressed { area: index, dressing: d.name.clone() });
        }
        Ok(())
    }
}

/// One area's cells in a frame a cell larger than its bounds all round,
/// indexed locally, so a look at a neighbour is an index and not a search
/// of the area's cells.
struct Frame {
    rect: GridRect,
    /// The area's own cells a body can stand on.
    own: Vec<bool>,
    /// Cells beside the area a body can stand on, orthogonally: a door, an
    /// opening, the end of a bridge. Flooded with the area, so a thing
    /// that shuts one off from the rest is a thing that cut it.
    rim: Vec<bool>,
}

impl Frame {
    fn at(&self, p: Point) -> Option<usize> {
        self.rect.contains(p).then(|| ((p.y - self.rect.y) * self.rect.width + (p.x - self.rect.x)) as usize)
    }

    fn point(&self, i: usize) -> Point {
        let w = self.rect.width as usize;
        Point::new(self.rect.x + (i % w) as i32, self.rect.y + (i / w) as i32)
    }

    fn own(&self, p: Point) -> bool {
        self.at(p).is_some_and(|i| self.own[i])
    }

    /// How many regions the cells `open` leaves of the area, with the rim,
    /// joined orthogonally, as growth promises every open cell is.
    ///
    /// Orthogonally is how a mover joins them, read strictly. A mover
    /// steps eight ways but never past the corner of a cell it cannot
    /// stand on, and what stands in a slot may be such a corner to some
    /// mover: the move resolver lets a diagonal past a thing on the floor,
    /// but a mind declines one whose crook it cannot step on. A diagonal
    /// a mind steps between two open cells passes two cells that are open
    /// too, the rim's or the area's, so it joins nothing two orthogonal
    /// steps do not; the resolver's reading, past a slot, can only join
    /// more. So an area this counts as one region is one to either.
    fn regions(&self, open: &[bool]) -> usize {
        let mut seen = vec![false; open.len()];
        let mut count = 0;
        let mut stack = Vec::new();
        for start in 0..open.len() {
            if seen[start] || !(open[start] || self.rim[start]) {
                continue;
            }
            count += 1;
            seen[start] = true;
            stack.push(start);
            while let Some(i) = stack.pop() {
                let p = self.point(i);
                for d in Direction::CARDINALS {
                    if let Some(j) = self.at(p + d.offset())
                        && !seen[j]
                        && (open[j] || self.rim[j])
                    {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        count
    }

    /// Whether the cell at `i` has a cell straight beside it that `open`
    /// leaves clear or the rim holds: somewhere a body stands to step onto
    /// it or to use what stands there.
    fn beside_open(&self, open: &[bool], i: usize) -> bool {
        let p = self.point(i);
        Direction::CARDINALS.iter().any(|d| self.at(p + d.offset()).is_some_and(|j| open[j] || self.rim[j]))
    }
}

/// Lays `dressing` in `area` and repaints it, and returns its slots as
/// marks: each thing on a cell its spread offers, drawn from the pass's
/// stream, never on or beside a cell of `keep` or a way out of the area,
/// and never one that leaves the area in more regions than before or that
/// leaves a slot with no open cell straight beside it.
fn dress_area(ctx: &mut BaseContext, area: &Area, dressing: &Dressing, keep: &[Point], floor: TileId) -> Vec<(char, Point)> {
    let walkable = ctx.tiles().tables().walkable;
    let walk = |ctx: &BaseContext, p: Point| ctx.terrain().get(p).is_some_and(|t| walkable[t.index()]);
    let rect = area.bounds.inflate(1);
    let size = (rect.width * rect.height) as usize;
    let mut frame = Frame { rect, own: vec![false; size], rim: vec![false; size] };
    for c in &area.cells {
        if walk(ctx, *c)
            && let Some(i) = frame.at(*c)
        {
            frame.own[i] = true;
        }
    }
    for i in 0..size {
        let p = frame.point(i);
        frame.rim[i] = !frame.own[i] && walk(ctx, p) && Direction::CARDINALS.iter().any(|d| frame.own(p + d.offset()));
    }
    // Beside: any of the eight cells round, so nothing stands even
    // corner-on to a doorway.
    let beside = |p: Point, q: Point| (p.x - q.x).abs() <= 1 && (p.y - q.y).abs() <= 1;
    let near: Vec<bool> = (0..size)
        .map(|i| {
            let p = frame.point(i);
            frame.own[i] && (keep.iter().any(|k| beside(p, *k)) || Direction::ALL.iter().any(|d| frame.at(p + d.offset()).is_some_and(|j| frame.rim[j])))
        })
        .collect();

    let spread = Spreads::measure(&frame);
    let mut open = frame.own.clone();
    let mut taken = vec![false; size];
    let mut regions = frame.regions(&open);
    let mut marks = Vec::new();
    // Every slot laid so far, each asked again after every thing that
    // blocks: the one that shuts a slot in is rarely the slot's own.
    let mut slots: Vec<usize> = Vec::new();
    for thing in &dressing.things {
        let mut cells = spread.cells(&frame, thing.at);
        cells.shuffle(ctx.rng());
        let want = (cells.len() * thing.density_pct as usize + 50) / 100;
        let blocks = match thing.stands {
            Stand::Tile(t) => !walkable[t.index()],
            Stand::Slot => true,
        };
        let mut placed = 0;
        for i in cells {
            if placed == want {
                break;
            }
            // A tile is laid only over the plain floor, so it never
            // paints out a shore, a bridge or a tile another pass laid;
            // a slot only marks its cell, so it may stand on any of them.
            let painted = matches!(thing.stands, Stand::Tile(_)) && ctx.terrain().get(frame.point(i)) != Some(floor);
            if taken[i] || near[i] || painted {
                continue;
            }
            let slot = thing.stands == Stand::Slot;
            if blocks {
                open[i] = false;
                let after = frame.regions(&open);
                let shut_in = slot.then_some(i).into_iter().chain(slots.iter().copied()).any(|s| !frame.beside_open(&open, s));
                if after > regions || shut_in {
                    open[i] = true;
                    continue;
                }
                regions = after;
            }
            taken[i] = true;
            placed += 1;
            let p = frame.point(i);
            match thing.stands {
                Stand::Tile(t) => {
                    ctx.terrain_mut().set(p, t);
                }
                Stand::Slot => {
                    slots.push(i);
                    marks.push((thing.glyph, p));
                }
            }
        }
    }
    if let Some((tile, pct)) = dressing.repaint {
        let mut bare: Vec<usize> = (0..size).filter(|&i| frame.own[i] && !taken[i] && ctx.terrain().get(frame.point(i)) == Some(floor)).collect();
        bare.shuffle(ctx.rng());
        let want = (bare.len() * pct as usize + 50) / 100;
        for i in bare.into_iter().take(want) {
            ctx.terrain_mut().set(frame.point(i), tile);
        }
    }
    marks
}

/// The cells each spread offers in one area, measured once.
struct Spreads {
    /// Each own cell's distance to the nearest cell not its own, counting
    /// diagonals: one along a wall, two a cell in.
    depth: Vec<u32>,
}

impl Spreads {
    fn measure(frame: &Frame) -> Self {
        let size = frame.own.len();
        let mut depth = vec![0u32; size];
        let mut queue = std::collections::VecDeque::new();
        for (i, depth) in depth.iter_mut().enumerate() {
            let p = frame.point(i);
            if frame.own[i] && Direction::ALL.iter().any(|d| !frame.own(p + d.offset())) {
                *depth = 1;
                queue.push_back(i);
            }
        }
        while let Some(i) = queue.pop_front() {
            let p = frame.point(i);
            for d in Direction::ALL {
                if let Some(j) = frame.at(p + d.offset())
                    && frame.own[j]
                    && depth[j] == 0
                {
                    depth[j] = depth[i] + 1;
                    queue.push_back(j);
                }
            }
        }
        Self { depth }
    }

    /// The cells `at` offers, in the frame's order, before any is drawn.
    fn cells(&self, frame: &Frame, at: Spread) -> Vec<usize> {
        let size = frame.own.len();
        let wall = |p: Point, d: Direction| !frame.own(p + d.offset());
        let corner = |p: Point| (wall(p, Direction::North) || wall(p, Direction::South)) && (wall(p, Direction::East) || wall(p, Direction::West));
        let deepest = (0..size).filter(|&i| frame.own[i]).map(|i| self.depth[i]).max().unwrap_or(0);
        let inner: Vec<usize> = (0..size).filter(|&i| frame.own[i] && self.depth[i] >= 2).collect();
        let lying = frame.rect.width >= frame.rect.height;
        let across = |i: usize| if lying { frame.point(i).y } else { frame.point(i).x };
        let first = inner.iter().map(|&i| across(i)).min().unwrap_or(0);
        (0..size)
            .filter(|&i| frame.own[i])
            .filter(|&i| {
                let p = frame.point(i);
                match at {
                    Spread::Walls => Direction::CARDINALS.iter().any(|d| wall(p, *d)) && !corner(p),
                    Spread::Corners => corner(p),
                    Spread::Center => deepest >= 2 && self.depth[i] == deepest,
                    Spread::Rows { aisle } => self.depth[i] >= 2 && ((across(i) - first) as u32).is_multiple_of(aisle + 1),
                    Spread::Scatter => true,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accrete::tests::{Rig, rig};
    use crate::accrete::{Accrete, Cross, District, Join, Passage, Rectangle, Shape};
    use crate::cave::Cave;
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use crate::dungeon::{FarthestExit, RandomStart};
    use crate::lakes::{Bridge, Bridges, Lakes};
    use crate::loops::OpenLoops;
    use crate::secrets::HideOpenings;
    use crate::survey::Survey;
    use crate::testing::{first_seeds, par_seeds};
    use atglyph_core::{Grid2D, RunSeed};
    use atglyph_grid::{CostSource, DijkstraMap, PathRules, TileProps};
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    /// The key the test dressing's slots carry.
    const KEY: u32 = 7;

    /// The rig's tiles, a pillar nothing passes and rubble anything may
    /// stand on.
    fn tiles() -> (Rig, TileId, TileId) {
        let mut r = rig();
        let pillar = r.tiles.register(TileProps::wall("pillar")).unwrap();
        let rubble = r.tiles.register(TileProps::floor("rubble")).unwrap();
        (r, pillar, rubble)
    }

    fn slot(glyph: char, at: Spread, density_pct: u32) -> Thing {
        Thing { glyph, stands: Stand::Slot, at, density_pct }
    }

    fn dressing(name: &str, things: Vec<Thing>, repaint: Option<(TileId, u32)>) -> Dressing {
        Dressing { name: name.into(), things, repaint, key: Some(KEY) }
    }

    /// A dress pass laying `d` in every room of district `d`.
    fn dress(floor: TileId, d: Dressing) -> Dress {
        let list = DistrictDressings::new("d", vec![(d.name.clone(), 1)]);
        Dress { name: "dress", floor, dressings: vec![d], districts: vec![list] }
    }

    /// The dressing's slots: every mark a stamp keyed by it emitted.
    fn slots(ctx: &BaseContext) -> Vec<(char, Point)> {
        ctx.outputs().iter::<Stamped>().filter(|s| s.prefab == Some(KEY)).flat_map(|s| s.marks.iter().copied()).collect()
    }

    /// One area of `w` by `h`, of district `d`, at (2, 3) on a map of
    /// rock, its door in the middle of its top wall and the way in on the
    /// cell beyond the door.
    fn room(r: &Rig, w: i32, h: i32) -> BaseContext {
        let mut ctx = BaseContext::blank(w + 6, h + 6, r.tiles.clone(), r.wall);
        let bounds = GridRect::new(2, 3, w, h);
        for c in bounds.cells() {
            ctx.terrain_mut().set(c, r.floor);
        }
        let door = Point::new(2 + w / 2, 2);
        ctx.terrain_mut().set(door, r.door);
        ctx.terrain_mut().set(door.offset(0, -1), r.floor);
        ctx.emit(Room(bounds));
        ctx.emit(Area { index: 0, shape: "room".into(), district: "d".into(), cells: bounds.cells().collect(), bounds, inner: bounds, entries: None });
        ctx.emit(Opening { cells: vec![door], facing: Direction::North, areas: (0, 0), join: Join::Door });
        ctx.emit(StartPoint(door.offset(0, -1)));
        ctx
    }

    /// The cells of `bounds` with a cell outside it straight beside them
    /// and not at a corner: what `Walls` offers in a rectangle.
    fn walls_of(bounds: GridRect) -> Vec<Point> {
        let inner = bounds.inflate(-1);
        let corner = |p: Point| (p.x == bounds.x || p.x == bounds.right() - 1) && (p.y == bounds.y || p.y == bounds.bottom() - 1);
        bounds.cells().filter(|p| !inner.contains(*p) && !corner(*p)).collect()
    }

    /// How many of the regions `cells` and the `ways` beside them leave,
    /// less `blocked`, hold a cell of `cells`, joined orthogonally.
    fn pieces(cells: &[Point], ways: &[Point], blocked: &[Point]) -> usize {
        let blocked: BTreeSet<&Point> = blocked.iter().collect();
        let own: BTreeSet<Point> = cells.iter().copied().filter(|c| !blocked.contains(c)).collect();
        let open: BTreeSet<Point> = own.iter().copied().chain(ways.iter().copied().filter(|c| !blocked.contains(c))).collect();
        let mut seen: BTreeSet<Point> = BTreeSet::new();
        let mut count = 0;
        for &start in &own {
            if !seen.insert(start) {
                continue;
            }
            count += 1;
            let mut stack = vec![start];
            while let Some(p) = stack.pop() {
                for d in Direction::CARDINALS {
                    let q = p + d.offset();
                    if open.contains(&q) && seen.insert(q) {
                        stack.push(q);
                    }
                }
            }
        }
        count
    }

    /// How many regions `cells`, less those in `blocked`, make, joined
    /// orthogonally.
    fn regions(cells: &[Point], blocked: &[Point]) -> usize {
        let blocked: BTreeSet<&Point> = blocked.iter().collect();
        let open: BTreeSet<Point> = cells.iter().copied().filter(|c| !blocked.contains(c)).collect();
        let mut seen: BTreeSet<Point> = BTreeSet::new();
        let mut count = 0;
        for &start in &open {
            if !seen.insert(start) {
                continue;
            }
            count += 1;
            let mut stack = vec![start];
            while let Some(p) = stack.pop() {
                for d in Direction::CARDINALS {
                    let q = p + d.offset();
                    if open.contains(&q) && seen.insert(q) {
                        stack.push(q);
                    }
                }
            }
        }
        count
    }

    /// One dressing fills a small room and a hall each to its density:
    /// half the cells along the walls, none beside the door, and a tenth
    /// of the floor scattered, within a tolerance for what the door and
    /// the rule against cutting refuse.
    #[test]
    fn one_dressing_dresses_a_small_room_and_a_large_hall_to_its_density() {
        let (r, ..) = tiles();
        let d = dressing("store", vec![slot('w', Spread::Walls, 50), slot('s', Spread::Scatter, 10)], None);
        for (w, h) in [(5, 5), (20, 12)] {
            let bounds = GridRect::new(2, 3, w, h);
            let walls = walls_of(bounds);
            for s in 0..200 {
                let mut ctx = room(&r, w, h);
                Chain::new().then(dress(r.floor, d.clone())).run(&mut ctx, RunSeed(s)).unwrap();
                let got = slots(&ctx);
                let along: Vec<Point> = got.iter().filter(|(c, _)| *c == 'w').map(|(_, p)| *p).collect();
                assert!(!along.is_empty() && along.iter().all(|p| walls.contains(p)), "{w}x{h} seed {s}: things along the walls: {along:?}");
                let share = along.len() * 100 / walls.len();
                assert!((35..=50).contains(&share), "{w}x{h} seed {s}: {} of {} wall cells is {share}%", along.len(), walls.len());
                let door = Point::new(2 + w / 2, 2);
                assert!(got.iter().all(|(_, p)| (p.x - door.x).abs() > 1 || (p.y - door.y).abs() > 1), "{w}x{h} seed {s}: beside the door");
                let scattered = got.iter().filter(|(c, _)| *c == 's').count();
                let share = scattered * 100 / (w * h) as usize;
                assert!((5..=12).contains(&share), "{w}x{h} seed {s}: {scattered} scattered of {} is {share}%", w * h);
            }
        }
    }

    /// Lines of things along a hall's long side leave every aisle between
    /// them clear and walkable, at one aisle and at two.
    #[test]
    fn rows_fill_their_lines_and_leave_their_aisles() {
        let (r, ..) = tiles();
        for aisle in [1, 2] {
            let mut ctx = room(&r, 20, 12);
            let d = dressing("rows", vec![slot('r', Spread::Rows { aisle }, 100)], None);
            Chain::new().then(dress(r.floor, d)).run(&mut ctx, RunSeed(3)).unwrap();
            let got: Vec<Point> = slots(&ctx).into_iter().map(|(_, p)| p).collect();
            // A cell in from the walls: x 3 to 20 and y 4 to 13, lines
            // from y 4 every aisle plus one.
            let inner = GridRect::new(2, 3, 20, 12).inflate(-1);
            for p in inner.cells() {
                let line = (p.y - 4) % (aisle as i32 + 1) == 0;
                assert_eq!(got.contains(&p), line, "aisle {aisle}: {p:?} is {} a line", if line { "on" } else { "off" });
            }
            assert!(got.iter().all(|p| inner.contains(*p)), "aisle {aisle}: a line runs only a cell in from the walls");
            let cells: Vec<Point> = GridRect::new(2, 3, 20, 12).cells().collect();
            assert_eq!(regions(&cells, &got), 1, "aisle {aisle}: every aisle joins the ring along the walls");
        }
    }

    /// Two small rooms joined by a neck: scattered over every cell, the
    /// things take every cell but those whose taking would part the two,
    /// which are taken back, so the area stays one region.
    #[test]
    fn a_thing_that_would_cut_an_area_in_two_is_taken_back() {
        let (r, ..) = tiles();
        let cells: Vec<Point> =
            GridRect::new(1, 2, 4, 4).cells().chain([Point::new(5, 3), Point::new(6, 3), Point::new(7, 3)]).chain(GridRect::new(8, 2, 4, 4).cells()).collect();
        let neck = || {
            let mut ctx = BaseContext::blank(16, 9, r.tiles.clone(), r.wall);
            for c in &cells {
                ctx.terrain_mut().set(*c, r.floor);
            }
            ctx.emit(Room(GridRect::new(1, 2, 4, 4)));
            ctx.emit(Area {
                index: 0,
                shape: "room".into(),
                district: "d".into(),
                cells: cells.clone(),
                bounds: GridRect::new(1, 2, 11, 4),
                inner: GridRect::new(1, 2, 4, 4),
                entries: None,
            });
            ctx
        };
        for s in 0..50 {
            let mut c = neck();
            let d = dressing("heap", vec![slot('x', Spread::Scatter, 100)], None);
            Chain::new().then(dress(r.floor, d)).run(&mut c, RunSeed(s)).unwrap();
            let got: Vec<Point> = slots(&c).into_iter().map(|(_, p)| p).collect();
            assert_eq!(regions(&cells, &got), 1, "seed {s}: the area was cut: {got:?}");
            assert!(!got.contains(&Point::new(6, 3)), "seed {s}: the neck's middle stands empty");
            assert!(got.len() >= 10, "seed {s}: the rest is taken: {got:?}");
        }
    }

    /// Every slot in the corners first and then a slot on every cell along
    /// the walls: each corner's two neighbours are wall cells, and taking
    /// both shut the corner in while the count of regions, which had
    /// already let the corner go, stayed one. Each slot keeps a clear cell
    /// straight beside it now, so an item in it is stepped onto.
    #[test]
    fn a_slot_laid_first_is_never_shut_in_by_the_things_laid_after_it() {
        let (r, ..) = tiles();
        let d = dressing("crowd", vec![slot('c', Spread::Corners, 100), slot('w', Spread::Walls, 100)], None);
        for (w, h) in [(5, 5), (7, 6), (12, 9)] {
            let bounds = GridRect::new(2, 3, w, h);
            for s in 0..50 {
                let mut ctx = room(&r, w, h);
                Chain::new().then(dress(r.floor, d.clone())).run(&mut ctx, RunSeed(s)).unwrap();
                let got: BTreeSet<Point> = slots(&ctx).into_iter().map(|(_, p)| p).collect();
                assert!(got.iter().any(|p| bounds.contains(*p) && !bounds.inflate(-1).contains(*p)), "{w}x{h} seed {s}: something stands");
                for p in &got {
                    let clear = Direction::CARDINALS.iter().map(|d| *p + d.offset()).any(|q| bounds.contains(q) && !got.contains(&q));
                    assert!(clear, "{w}x{h} seed {s}: the slot at {p:?} is shut in by {got:?}");
                }
            }
        }
    }

    /// The ground a strict mover reads in one area: the area's own cells a
    /// body can stand on that nothing stands in, and its openings, each
    /// entered and passed beside; nothing else, so no diagonal passes the
    /// corner of a thing standing, which is the reading the guard keeps.
    struct Ground {
        width: i32,
        open: Vec<bool>,
    }

    impl Grid2D for Ground {
        fn width(&self) -> i32 {
            self.width
        }

        fn height(&self) -> i32 {
            self.open.len() as i32 / self.width
        }
    }

    impl CostSource for Ground {
        fn cost_idx(&self, idx: usize) -> Option<u32> {
            self.open[idx].then_some(100)
        }
    }

    /// The cells of `area` a body can stand on, or could if what stands
    /// there does not block, that a mover stepping eight ways from the
    /// area's openings never reaches: a clear cell it never stands on, or
    /// a slot it never steps onto from a cell beside it.
    fn unreached(ctx: &BaseContext, area: &Area, over: &[Point], stood: &BTreeSet<Point>) -> Vec<Point> {
        let walkable = ctx.tiles().tables().walkable;
        let (w, h) = (ctx.terrain().width(), ctx.terrain().height());
        let walk = |p: Point| ctx.terrain().get(p).is_some_and(|t| walkable[t.index()]);
        let own: Vec<Point> = area.cells.iter().copied().filter(|c| walk(*c)).collect();
        let doors: Vec<Point> =
            ctx.outputs().iter::<Opening>().filter(|o| o.areas.0 == area.index || o.areas.1 == area.index).flat_map(|o| o.cells.clone()).collect();
        let mut ground = Ground { width: w, open: vec![false; (w * h) as usize] };
        for p in own.iter().filter(|p| !stood.contains(p)).chain(&doors).chain(over) {
            ground.open[(p.y * w + p.x) as usize] = true;
        }
        let goals = if doors.is_empty() { own.iter().filter(|p| !stood.contains(p)).take(1).copied().collect() } else { doors };
        let mut flood = DijkstraMap::covering(&ground);
        flood.build(&ground, goals, PathRules::EIGHT_WAY);
        let entered = |from: Point, to: Point| {
            let (dx, dy) = (to.x - from.x, to.y - from.y);
            dx == 0 || dy == 0 || (ground.lets_diagonal_past(from.offset(dx, 0)) && ground.lets_diagonal_past(from.offset(0, dy)))
        };
        own.into_iter()
            .filter(|p| {
                if stood.contains(p) {
                    !Direction::ALL.iter().map(|d| *p + d.offset()).any(|n| flood.value(n).is_some() && entered(n, *p))
                } else {
                    flood.value(*p).is_none()
                }
            })
            .collect()
    }

    /// Every cell of the place a body can stand on, slots left out, that a
    /// walk from the start does not reach, stepping orthogonally over what
    /// can be stood on and every opening, a hidden one found: the place
    /// whole, bridges and the floor between areas included, and not only
    /// each area by itself.
    fn beyond_reach(ctx: &BaseContext, stood: &BTreeSet<Point>) -> Vec<Point> {
        let walkable = ctx.tiles().tables().walkable;
        let (w, h) = (ctx.terrain().width(), ctx.terrain().height());
        let ways: BTreeSet<Point> =
            ctx.outputs().iter::<Opening>().flat_map(|o| o.cells.iter().copied()).chain(ctx.outputs().iter::<SecretCell>().map(|s| s.at)).collect();
        let open = |p: Point| !stood.contains(&p) && (ways.contains(&p) || ctx.terrain().get(p).is_some_and(|t| walkable[t.index()]));
        let start = ctx.outputs().first::<StartPoint>().unwrap().0;
        let mut reached = vec![false; (w * h) as usize];
        let mut stack = vec![start];
        reached[(start.y * w + start.x) as usize] = true;
        while let Some(p) = stack.pop() {
            for d in Direction::CARDINALS {
                let q = p + d.offset();
                if ctx.terrain().in_bounds(q) && !reached[(q.y * w + q.x) as usize] && open(q) {
                    reached[(q.y * w + q.x) as usize] = true;
                    stack.push(q);
                }
            }
        }
        (0..w * h).map(|i| Point::new(i % w, i / w)).filter(|p| open(*p) && !reached[(p.y * w + p.x) as usize]).collect()
    }

    /// Rooms, a hall, crosses and two sizes of cave in two districts,
    /// joined by passages, at a share both test sizes reach.
    fn grow(r: &Rig) -> Accrete {
        let shapes: Vec<Box<dyn Shape>> = vec![
            Box::new(Rectangle { name: "cell".into(), w: (5, 9), h: (5, 8), join: Join::Door }),
            Box::new(Rectangle { name: "hall".into(), w: (11, 18), h: (8, 12), join: Join::Mouth(3) }),
            Box::new(Cross { name: "cross".into(), long: (9, 13), short: (3, 5), join: Join::Door }),
            Box::new(Cave { name: "hollow".into(), w: (8, 14), h: (6, 10), fill_pct: 55, visits_pct: 50, join: Join::Mouth(2) }),
            Box::new(Cave { name: "cavern".into(), w: (16, 26), h: (10, 16), fill_pct: 52, visits_pct: 300, join: Join::Mouth(3) }),
            Box::new(Passage { name: "run".into(), width: (1, 2), length: (4, 12), bend_pct: 30, join: Join::Arch }),
        ];
        let districts = vec![
            District { name: "rooms".into(), shapes: vec![(0, 4), (1, 2), (2, 3), (5, 1)] },
            District { name: "caves".into(), shapes: vec![(3, 3), (4, 2), (2, 1), (5, 1)] },
        ];
        Accrete {
            floor: r.floor,
            door: r.door,
            shapes,
            districts,
            mix: vec![(0, 1), (1, 1)],
            districts_per_10k: 7,
            open_pct: 35,
            patience: 300,
            barriers: Vec::new(),
            locked: r.door,
            sealed: r.door,
            pieces: Vec::new(),
            late_pct: 0,
            first: None,
        }
    }

    /// Over grown places of rectangles, crosses and caves, with secrets
    /// hidden and ponds laid or not, a dressing asking for everything at
    /// once, pillars in the middle, rows and slots along every wall and in
    /// every corner, never stands a thing on or beside an opening, a
    /// hidden one, the way in or the way down, and leaves every area one
    /// region with everything standing, a bridge over a pond in it a way
    /// as its doors are, every cell of it a body can stand on reached from
    /// its openings by a mover stepping eight ways and never past a
    /// corner, whether what stands in a slot blocks or not; and the whole
    /// place is walked from the start, every secret found.
    #[test]
    fn nothing_stands_beside_an_opening_and_every_area_stays_one_region() {
        let (r, pillar, rubble) = tiles();
        let d = dressing(
            "everything",
            vec![
                Thing { glyph: 'p', stands: Stand::Tile(pillar), at: Spread::Center, density_pct: 100 },
                slot('c', Spread::Corners, 100),
                slot('w', Spread::Walls, 60),
                slot('r', Spread::Rows { aisle: 1 }, 70),
                slot('s', Spread::Scatter, 15),
            ],
            Some((rubble, 20)),
        );
        let pass = Dress {
            name: "dress",
            floor: r.floor,
            dressings: vec![d],
            districts: vec![
                DistrictDressings::new("rooms", vec![("everything".into(), 3)]).or_nothing(1),
                DistrictDressings::new("caves", vec![("everything".into(), 1)]),
            ],
        };
        let hide = HideOpenings { loops: (1, 3), rooms: (1, 2), loop_spot: 5, room_spot: 5, at_least_rooms: 0 };
        let mut wet_tiles = r.tiles.clone();
        let water = wet_tiles.register(TileProps::wall("water")).unwrap();
        let shore = wet_tiles.register(TileProps::floor("shore")).unwrap();
        let span = wet_tiles.register(TileProps::floor("span")).unwrap();
        let ponds = Lakes { name: "ponds", tile: water, count: (2, 5), size: (20, 70), wreath: Some((shore, 60)), cut: false, within: Vec::new() };
        let bridges = Bridges { name: "bridges", across: water, tile: span, longest: 6, detour: 20 };
        let (caves, crosses, secrets, lakes, spans) = (AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0));
        // Thirty seeds of the small maps every day, not `seeds`' eight: the
        // counts asserted below need that many dressed beside a way before
        // they show anything.
        for (w, h, range, wet) in
            [(84, 48, first_seeds(0..200, 30), false), (160, 90, 0..30, false), (84, 48, first_seeds(0..60, 30), true), (160, 90, 0..8, true)]
        {
            par_seeds(range, |s| {
                let chain = Chain::new().then(grow(&r)).then(OpenLoops { door: r.door, min_detour: 20, max: 6 });
                let chain = if wet { chain.then(ponds.clone()).then(bridges) } else { chain };
                let chain = chain.then(RandomStart).then(FarthestExit).then(Survey).then(hide).then(pass.clone());
                let tiles = if wet { &wet_tiles } else { &r.tiles };
                let ctx =
                    chain.run_retrying(|| BaseContext::blank(w, h, tiles.clone(), r.wall), RunSeed(s), 20).unwrap_or_else(|e| panic!("{w}x{h} seed {s}: {e}"));
                lakes.fetch_add(ctx.outputs().iter::<crate::lakes::Lake>().count(), Relaxed);
                spans.fetch_add(ctx.outputs().iter::<Bridge>().count(), Relaxed);
                let got = slots(&ctx);
                let stood: Vec<Point> =
                    got.iter().map(|(_, p)| *p).chain((0..w * h).map(|i| Point::new(i % w, i / w)).filter(|p| ctx.terrain().get(*p) == Some(pillar))).collect();
                let keep: Vec<Point> = ctx
                    .outputs()
                    .iter::<Opening>()
                    .flat_map(|o| o.cells.clone())
                    .chain(ctx.outputs().iter::<SecretCell>().map(|s| s.at))
                    .chain([ctx.outputs().first::<StartPoint>().unwrap().0, ctx.outputs().first::<ExitPoint>().unwrap().0])
                    .collect();
                for p in &stood {
                    assert!(
                        keep.iter().all(|k| (p.x - k.x).abs() > 1 || (p.y - k.y).abs() > 1),
                        "{w}x{h} seed {s}: a thing on or beside an opening or a way at {p:?}"
                    );
                }
                let walkable = ctx.tiles().tables().walkable;
                // A bridge is a way, as a door is: a pond's two banks in
                // one area may be joined over one as well as round it.
                let over: Vec<Point> = ctx.outputs().iter::<Bridge>().flat_map(|b| b.cells.iter().copied()).collect();
                for a in ctx.outputs().iter::<Area>() {
                    let open: Vec<Point> = a.cells.iter().copied().filter(|c| ctx.terrain().get(*c).is_some_and(|t| walkable[t.index()])).collect();
                    assert_eq!(pieces(&open, &over, &stood), 1, "{w}x{h} seed {s}, wet {wet}: area {} ({}) was cut", a.index, a.shape);
                    let lost = unreached(&ctx, a, &over, &got.iter().map(|(_, p)| *p).collect());
                    assert!(lost.is_empty(), "{w}x{h} seed {s}: area {} ({}) has cells no mover reaches: {lost:?}", a.index, a.shape);
                }
                let stranded = beyond_reach(&ctx, &got.iter().map(|(_, p)| *p).collect());
                assert!(stranded.is_empty(), "{w}x{h} seed {s}: cells no walk from the start reaches, every secret found: {stranded:?}");
                let dressed: Vec<usize> = ctx.outputs().iter::<Dressed>().map(|d| d.area).collect();
                let shape = |i: usize| ctx.outputs().iter::<Area>().find(|a| a.index == i).unwrap().shape.clone();
                caves.fetch_add(dressed.iter().filter(|i| shape(**i) == "hollow" || shape(**i) == "cavern").count(), Relaxed);
                crosses.fetch_add(dressed.iter().filter(|i| shape(**i) == "cross").count(), Relaxed);
                assert!(dressed.iter().all(|i| shape(*i) != "run"), "{w}x{h} seed {s}: a passage holds nothing");
                secrets.fetch_add(ctx.outputs().iter::<SecretCell>().count(), Relaxed);
            });
        }
        let (caves, crosses, secrets, lakes, spans) = (caves.into_inner(), crosses.into_inner(), secrets.into_inner(), lakes.into_inner(), spans.into_inner());
        assert!(
            caves > 100 && crosses > 50 && secrets > 100 && lakes > 50 && spans > 5,
            "caves {caves}, crosses {crosses}, secrets {secrets}, lakes {lakes} and bridges {spans} dressed beside, or this proves little"
        );
    }

    /// Four rooms: the first given its dressing by a pick, the second with
    /// a stamp in it, the third painted from a prefab, the fourth plain;
    /// and a passage. With a list that leaves every room bare, only the
    /// pick's is dressed; with one that dresses every room, the plain room
    /// is dressed too, and never the stamped, the painted or the passage.
    #[test]
    fn a_given_dressing_is_laid_first_and_only_free_rooms_are_dressed_by_their_district() {
        let (r, ..) = tiles();
        let place = || {
            let mut ctx = BaseContext::blank(40, 9, r.tiles.clone(), r.wall);
            for (index, x) in [1, 9, 17, 25, 33].into_iter().enumerate() {
                let bounds = GridRect::new(x, 1, 5, 5);
                for c in bounds.cells() {
                    ctx.terrain_mut().set(c, r.floor);
                }
                let entries = (index == 2).then(Vec::new);
                if index < 4 {
                    ctx.emit(Room(bounds));
                }
                let shape = if index < 4 { "room" } else { "run" };
                ctx.emit(Area { index, shape: shape.into(), district: "d".into(), cells: bounds.cells().collect(), bounds, inner: bounds, entries });
            }
            ctx.emit(Stamped { bounds: GridRect::new(10, 2, 1, 1), marks: Vec::new(), prefab: None });
            ctx.emit(DressAt { dressing: "given".into(), area: 0 });
            ctx
        };
        let things = |name: &str| dressing(name, vec![slot('x', Spread::Center, 100)], None);
        let dressed = |list: DistrictDressings| {
            let mut ctx = place();
            let pass = Dress { name: "dress", floor: r.floor, dressings: vec![things("plain"), things("given")], districts: vec![list] };
            Chain::new().then(pass).run(&mut ctx, RunSeed(1)).unwrap();
            ctx.outputs().iter::<Dressed>().map(|d| (d.area, d.dressing.clone())).collect::<Vec<_>>()
        };
        assert_eq!(dressed(DistrictDressings::new("d", Vec::new()).or_nothing(1)), vec![(0, "given".to_string())]);
        assert_eq!(dressed(DistrictDressings::new("d", vec![("plain".into(), 1)])), vec![(0, "given".to_string()), (3, "plain".to_string())]);
        assert_eq!(dressed(DistrictDressings::new("d", vec![("plain".into(), 0)])), vec![(0, "given".to_string())], "a list weighing nothing dresses nothing");
    }

    #[test]
    fn a_given_dressing_there_is_none_of_fails_naming_it() {
        let (r, ..) = tiles();
        let mut ctx = room(&r, 5, 5);
        ctx.emit(DressAt { dressing: "missing".into(), area: 0 });
        let e = Chain::new().then(dress(r.floor, dressing("store", vec![slot('w', Spread::Walls, 50)], None))).run(&mut ctx, RunSeed(0)).unwrap_err();
        assert!(e.pass == "dress" && e.reason.contains("missing"), "{e}");
    }

    #[test]
    fn a_district_list_naming_a_dressing_there_is_none_of_fails_naming_it() {
        let (r, ..) = tiles();
        let mut ctx = room(&r, 5, 5);
        let mut pass = dress(r.floor, dressing("store", vec![slot('w', Spread::Walls, 50)], None));
        pass.districts = vec![DistrictDressings::new("d", vec![("missing".into(), 1)])];
        let e = Chain::new().then(pass).run(&mut ctx, RunSeed(0)).unwrap_err();
        assert!(e.pass == "dress" && e.reason.contains("missing"), "{e}");
    }

    #[test]
    fn a_given_dressing_in_an_area_there_is_none_of_fails_naming_it() {
        let (r, ..) = tiles();
        let mut ctx = room(&r, 5, 5);
        ctx.emit(DressAt { dressing: "store".into(), area: 3 });
        let e = Chain::new().then(dress(r.floor, dressing("store", vec![slot('w', Spread::Walls, 50)], None))).run(&mut ctx, RunSeed(0)).unwrap_err();
        assert!(e.pass == "dress" && e.reason.contains("area 3"), "{e}");
    }

    /// A repaint is never checked for cutting, so one of a tile no body
    /// stands on is refused before anything is laid.
    #[test]
    fn a_repaint_no_body_stands_on_is_refused() {
        let (r, pillar, _) = tiles();
        let mut ctx = room(&r, 6, 6);
        let before = ctx.terrain().clone();
        let e =
            Chain::new().then(dress(r.floor, dressing("walls", vec![slot('w', Spread::Walls, 50)], Some((pillar, 30))))).run(&mut ctx, RunSeed(0)).unwrap_err();
        assert!(e.pass == "dress" && e.reason.contains("\"walls\""), "{e}");
        assert_eq!(ctx.terrain(), &before, "nothing was laid");
    }

    /// A tile thing is laid only over the plain floor, so a shore or a
    /// bridge in a room keeps its tile; a slot may mark either.
    #[test]
    fn a_tile_thing_never_paints_over_a_tile_that_is_not_the_floor() {
        let (mut r, _, rubble) = tiles();
        let shore = r.tiles.register(TileProps::floor("shore")).unwrap();
        let mut ctx = room(&r, 12, 9);
        let banks: Vec<Point> = GridRect::new(2, 3, 12, 9).cells().filter(|p| (p.x + p.y) % 3 == 0).collect();
        for p in &banks {
            ctx.terrain_mut().set(*p, shore);
        }
        let d = dressing("heaps", vec![Thing { glyph: 'h', stands: Stand::Tile(rubble), at: Spread::Scatter, density_pct: 100 }], None);
        Chain::new().then(dress(r.floor, d)).run(&mut ctx, RunSeed(3)).unwrap();
        assert!(banks.iter().all(|p| ctx.terrain().get(*p) == Some(shore)), "a shore cell was painted over");
        assert!(GridRect::new(2, 3, 12, 9).cells().any(|p| ctx.terrain().get(p) == Some(rubble)), "the floor between took the heaps");
    }

    /// A repaint takes its share of the floor left clear, and never a cell
    /// a thing stands on.
    #[test]
    fn a_repaint_takes_its_share_of_the_bare_floor() {
        let (r, _, rubble) = tiles();
        let mut ctx = room(&r, 20, 12);
        let d = dressing("dust", vec![slot('w', Spread::Walls, 50)], Some((rubble, 25)));
        Chain::new().then(dress(r.floor, d)).run(&mut ctx, RunSeed(5)).unwrap();
        let got: Vec<Point> = slots(&ctx).into_iter().map(|(_, p)| p).collect();
        let painted: Vec<Point> = GridRect::new(2, 3, 20, 12).cells().filter(|p| ctx.terrain().get(*p) == Some(rubble)).collect();
        assert_eq!(painted.len(), (240 - got.len() + 2) / 4, "a quarter of the bare floor, rounded");
        assert!(painted.iter().all(|p| !got.contains(p)));
    }
}
