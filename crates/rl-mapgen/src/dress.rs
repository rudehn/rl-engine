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
//! and taking back the one that cut it. The file, its names and its checks
//! are `rl-rules`'s; this reads only what growth and the passes after it
//! published.

use rand::Rng;
use rand::seq::SliceRandom;
use rl_core::{Direction, Point, Rect};
use rl_grid::TileId;

use crate::accrete::{Area, Opening};
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::dungeon::{ExitPoint, Room};
use crate::passes::StartPoint;
use crate::pool::DressAt;
use crate::prefab::{Stamped, pick_weighted};
use crate::secrets::Secret;

/// Where a thing goes in an area.
///
/// A wall here is anything beside a cell that is not the area's own floor
/// a body can stand on: rock, a lake, the wall of the next piece. So a
/// cave's ragged edge is all wall and corner, and a lake's shore is wall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// whoever fills the dressing's slots. A slot blocks, as far as the
    /// check that nothing cuts an area knows, since what it holds, a prop
    /// that blocks or an item that does not, is not this crate's to know.
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
    /// able to stand on it: a repaint is never checked for cutting.
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
    /// The floor a repaint repaints; any other tile is left as it is.
    pub floor: TileId,
    /// Every dressing a list or a `DressAt` may name.
    pub dressings: Vec<Dressing>,
    /// Each district's list.
    pub districts: Vec<DistrictDressings>,
}

/// What the free rooms grown in one district are dressed with: each
/// dressing at a weight, and nothing at all at a weight of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistrictDressings {
    /// The district, by [`Area::district`].
    pub district: String,
    /// The weight of leaving a room bare.
    pub nothing: u32,
    /// Each an index into [`Dress::dressings`] and its weight.
    pub dressings: Vec<(usize, u32)>,
}

impl DistrictDressings {
    /// `district` dressed with `dressings`, never left bare.
    pub fn new(district: impl Into<String>, dressings: Vec<(usize, u32)>) -> Self {
        Self { district: district.into(), nothing: 0, dressings }
    }

    /// This list, with nothing at all drawn at `weight`.
    pub fn or_nothing(mut self, weight: u32) -> Self {
        self.nothing = weight;
        self
    }
}

impl<C: BuildContext> Pass<C> for Dress {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Finish
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let err = |reason: String| BuildError::new(self.name, reason);
        let areas: Vec<Area> = ctx.outputs().iter::<Area>().cloned().collect();
        let rooms: Vec<Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
        let stamps: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
        let given: Vec<DressAt> = ctx.outputs().iter::<DressAt>().cloned().collect();
        // Every cell nothing may stand on or beside: the openings, hidden
        // ones too, and the two ways.
        let keep: Vec<Point> = ctx
            .outputs()
            .iter::<Opening>()
            .flat_map(|o| o.cells.iter().copied())
            .chain(ctx.outputs().iter::<Secret>().map(|s| s.at))
            .chain(ctx.outputs().iter::<StartPoint>().map(|s| s.0))
            .chain(ctx.outputs().iter::<ExitPoint>().map(|e| e.0))
            .collect();

        for list in &self.districts {
            if let Some((i, _)) = list.dressings.iter().find(|(i, _)| *i >= self.dressings.len()) {
                return Err(err(format!("district {:?} names dressing {i}, and there are {}", list.district, self.dressings.len())));
            }
        }
        // Each area to dress and the dressing, by index: the given first.
        let mut laid: Vec<(usize, usize)> = Vec::new();
        for at in &given {
            let dressing = self.dressings.iter().position(|d| d.name == at.dressing).ok_or_else(|| err(format!("no dressing is called {:?}", at.dressing)))?;
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
            let Some(list) = self.districts.iter().find(|l| l.district == area.district).filter(|_| free) else { continue };
            // Nothing is the first choice, so a roll on it lays nothing.
            let weights: Vec<u32> = std::iter::once(list.nothing).chain(list.dressings.iter().map(|(_, w)| *w)).collect();
            let total: u32 = weights.iter().sum();
            if total == 0 {
                continue;
            }
            let roll = ctx.rng().random_range(0..total);
            if let Some(k) = pick_weighted(&weights, roll).checked_sub(1) {
                laid.push((area.index, list.dressings[k].0));
            }
        }
        for (index, dressing) in laid {
            let area = areas.iter().find(|a| a.index == index).expect("checked above");
            let d = &self.dressings[dressing];
            let marks = dress_area(ctx, area, d, &keep, self.floor);
            for (glyph, at) in marks {
                ctx.emit(Stamped { bounds: Rect::new(at.x, at.y, 1, 1), marks: vec![(glyph, at)], prefab: d.key });
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
    rect: Rect,
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
}

/// Lays `dressing` in `area` and repaints it, and returns its slots as
/// marks: each thing on a cell its spread offers, drawn from the pass's
/// stream, never on or beside a cell of `keep` or a way out of the area,
/// and never one that leaves the area in more regions than before.
fn dress_area<C: BuildContext>(ctx: &mut C, area: &Area, dressing: &Dressing, keep: &[Point], floor: TileId) -> Vec<(char, Point)> {
    let walkable = ctx.tiles().tables().walkable;
    let walk = |ctx: &C, p: Point| ctx.terrain().get(p).is_some_and(|t| walkable[t.index()]);
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
            if taken[i] || near[i] {
                continue;
            }
            if blocks {
                open[i] = false;
                let after = frame.regions(&open);
                if after > regions {
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
                Stand::Slot => marks.push((thing.glyph, p)),
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
    use crate::lakes::Lakes;
    use crate::loops::OpenLoops;
    use crate::secrets::HideOpenings;
    use crate::survey::Survey;
    use rl_core::RunSeed;
    use rl_grid::TileProps;
    use std::collections::BTreeSet;

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
        Dress { name: "dress", floor, dressings: vec![d], districts: vec![DistrictDressings::new("d", vec![(0, 1)])] }
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
        let bounds = Rect::new(2, 3, w, h);
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
    fn walls_of(bounds: Rect) -> Vec<Point> {
        let inner = bounds.inflate(-1);
        let corner = |p: Point| (p.x == bounds.x || p.x == bounds.right() - 1) && (p.y == bounds.y || p.y == bounds.bottom() - 1);
        bounds.cells().filter(|p| !inner.contains(*p) && !corner(*p)).collect()
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
            let bounds = Rect::new(2, 3, w, h);
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
            let inner = Rect::new(2, 3, 20, 12).inflate(-1);
            for p in inner.cells() {
                let line = (p.y - 4) % (aisle as i32 + 1) == 0;
                assert_eq!(got.contains(&p), line, "aisle {aisle}: {p:?} is {} a line", if line { "on" } else { "off" });
            }
            assert!(got.iter().all(|p| inner.contains(*p)), "aisle {aisle}: a line runs only a cell in from the walls");
            let cells: Vec<Point> = Rect::new(2, 3, 20, 12).cells().collect();
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
            Rect::new(1, 2, 4, 4).cells().chain([Point::new(5, 3), Point::new(6, 3), Point::new(7, 3)]).chain(Rect::new(8, 2, 4, 4).cells()).collect();
        let neck = || {
            let mut ctx = BaseContext::blank(16, 9, r.tiles.clone(), r.wall);
            for c in &cells {
                ctx.terrain_mut().set(*c, r.floor);
            }
            ctx.emit(Room(Rect::new(1, 2, 4, 4)));
            ctx.emit(Area {
                index: 0,
                shape: "room".into(),
                district: "d".into(),
                cells: cells.clone(),
                bounds: Rect::new(1, 2, 11, 4),
                inner: Rect::new(1, 2, 4, 4),
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
        }
    }

    /// Over grown places of rectangles, crosses and caves, with secrets
    /// hidden and ponds laid or not, a dressing asking for everything at
    /// once, pillars in the middle, rows and slots along every wall and in
    /// every corner, never stands a thing on or beside an opening, a
    /// hidden one, the way in or the way down, and leaves every area one
    /// region with everything standing.
    #[test]
    fn nothing_stands_beside_an_opening_and_every_area_stays_one_region() {
        let (r, pillar, rubble) = (tiles().0, tiles().1, tiles().2);
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
            districts: vec![DistrictDressings::new("rooms", vec![(0, 3)]).or_nothing(1), DistrictDressings::new("caves", vec![(0, 1)])],
        };
        let hide = HideOpenings { loops: (1, 3), rooms: (1, 2), loop_spot: 5, room_spot: 5, at_least_rooms: 0 };
        let mut wet_tiles = r.tiles.clone();
        let water = wet_tiles.register(TileProps::wall("water")).unwrap();
        let shore = wet_tiles.register(TileProps::floor("shore")).unwrap();
        let ponds = Lakes { name: "ponds", tile: water, count: (2, 5), size: (20, 70), wreath: Some((shore, 60)), cut: false, within: Vec::new() };
        let (mut caves, mut crosses, mut secrets, mut lakes) = (0, 0, 0, 0);
        for (w, h, seeds, wet) in [(84, 48, 0..200, false), (160, 90, 0..30, false), (84, 48, 0..60, true), (160, 90, 0..8, true)] {
            for s in seeds {
                let chain = Chain::new().then(grow(&r)).then(OpenLoops { door: r.door, min_detour: 20, max: 6 });
                let chain = if wet { chain.then(ponds.clone()) } else { chain };
                let chain = chain.then(RandomStart).then(FarthestExit).then(Survey).then(hide).then(pass.clone());
                let tiles = if wet { &wet_tiles } else { &r.tiles };
                let ctx =
                    chain.run_retrying(|| BaseContext::blank(w, h, tiles.clone(), r.wall), RunSeed(s), 20).unwrap_or_else(|e| panic!("{w}x{h} seed {s}: {e}"));
                lakes += ctx.outputs().iter::<crate::lakes::Lake>().count();
                let got = slots(&ctx);
                let stood: Vec<Point> =
                    got.iter().map(|(_, p)| *p).chain((0..w * h).map(|i| Point::new(i % w, i / w)).filter(|p| ctx.terrain().get(*p) == Some(pillar))).collect();
                let keep: Vec<Point> = ctx
                    .outputs()
                    .iter::<Opening>()
                    .flat_map(|o| o.cells.clone())
                    .chain(ctx.outputs().iter::<Secret>().map(|s| s.at))
                    .chain([ctx.outputs().first::<StartPoint>().unwrap().0, ctx.outputs().first::<ExitPoint>().unwrap().0])
                    .collect();
                for p in &stood {
                    assert!(
                        keep.iter().all(|k| (p.x - k.x).abs() > 1 || (p.y - k.y).abs() > 1),
                        "{w}x{h} seed {s}: a thing on or beside an opening or a way at {p:?}"
                    );
                }
                let walkable = ctx.tiles().tables().walkable;
                for a in ctx.outputs().iter::<Area>() {
                    let open: Vec<Point> = a.cells.iter().copied().filter(|c| ctx.terrain().get(*c).is_some_and(|t| walkable[t.index()])).collect();
                    assert_eq!(regions(&open, &stood), 1, "{w}x{h} seed {s}: area {} ({}) was cut", a.index, a.shape);
                }
                let dressed: Vec<usize> = ctx.outputs().iter::<Dressed>().map(|d| d.area).collect();
                let shape = |i: usize| ctx.outputs().iter::<Area>().find(|a| a.index == i).unwrap().shape.clone();
                caves += dressed.iter().filter(|i| shape(**i) == "hollow" || shape(**i) == "cavern").count();
                crosses += dressed.iter().filter(|i| shape(**i) == "cross").count();
                assert!(dressed.iter().all(|i| shape(*i) != "run"), "{w}x{h} seed {s}: a passage holds nothing");
                secrets += ctx.outputs().iter::<Secret>().count();
            }
        }
        assert!(
            caves > 100 && crosses > 50 && secrets > 100 && lakes > 50,
            "caves {caves}, crosses {crosses}, secrets {secrets} and lakes {lakes} dressed beside, or this proves little"
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
                let bounds = Rect::new(x, 1, 5, 5);
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
            ctx.emit(Stamped { bounds: Rect::new(10, 2, 1, 1), marks: Vec::new(), prefab: None });
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
        assert_eq!(dressed(DistrictDressings::new("d", vec![(0, 1)])), vec![(0, "given".to_string()), (3, "plain".to_string())]);
        assert_eq!(dressed(DistrictDressings::new("d", vec![(0, 0)])), vec![(0, "given".to_string())], "a list weighing nothing dresses nothing");
    }

    #[test]
    fn a_given_dressing_there_is_none_of_fails_naming_it() {
        let (r, ..) = tiles();
        let mut ctx = room(&r, 5, 5);
        ctx.emit(DressAt { dressing: "missing".into(), area: 0 });
        let e = Chain::new().then(dress(r.floor, dressing("store", vec![slot('w', Spread::Walls, 50)], None))).run(&mut ctx, RunSeed(0)).unwrap_err();
        assert!(e.pass == "dress" && e.reason.contains("missing"), "{e}");
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
        let painted: Vec<Point> = Rect::new(2, 3, 20, 12).cells().filter(|p| ctx.terrain().get(*p) == Some(rubble)).collect();
        assert_eq!(painted.len(), (240 - got.len() + 2) / 4, "a quarter of the bare floor, rounded");
        assert!(painted.iter().all(|p| !got.contains(p)));
    }
}
