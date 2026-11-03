//! Dressing the areas a place grew: things along their walls, in their
//! corners, in their centers, in rows or scattered, at a density, on the
//! area's own cells, so one dressing fills a small room, a long hall and a
//! cave alike.
//!
//! A [`Dressing`] is a list of [`Thing`]s, each a glyph that stands as a
//! tile or as a slot, a [`Spread`] and a density or a count, and an
//! optional repaint of the floor left clear. A house's rules ask more of
//! where a thing goes: near the door or far from it or behind another
//! thing ([`FromDoor`]), beside another thing, in a line across the room
//! with a gap, or just outside the door. [`Dress`] lays the dressings a
//! pool gave an area first, each [`DressAt`] in its own area, then dresses
//! every other room no stamp took from a weighted list by its district,
//! nothing among the choices. A slot is emitted as a stamp's mark is, a
//! [`Stamped`] of one cell carrying the dressing's key, so whoever fills a
//! stamp's slots fills a dressing's the same way.
//!
//! Two rules hold whatever a dressing asks: nothing stands on or beside
//! an opening, a hidden one included since it is an opening once found,
//! or on or beside the way in or the way down; and nothing cuts an area,
//! checked by flooding the area before and after each thing that blocks
//! and taking back the one that cut it or that left a slot, its own or one
//! laid before it, with no open cell beside it. A thing laid
//! [`OutsideDoor`](Spread::OutsideDoor) stands beside its opening by
//! design, but never on it nor on the cell straight out from it, and is
//! held to the second rule outside: the ground round it stays joined. The
//! file, its names and its checks are `atglyph-rules`'s; this reads only
//! what growth and the passes after it published.

use atglyph_core::{Direction, Grid2D, GridRect, Point};
use atglyph_grid::{Terrain, TileId, TileRegistry};
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
    /// A cell straight beside one an earlier rule of the dressing laid
    /// this glyph on, as a stool beside its table; none while no cell
    /// holds it.
    Beside(char),
    /// One line across the area through its middle, square to the way in
    /// from the opening nearest the middle, as a counter across a shop: a
    /// row where the way in runs up or down, a column where it runs
    /// across, the extra line of an even room left behind it. With no
    /// opening the way in runs along the longer side.
    Across {
        /// The cells left open at the line's middle, the way through it;
        /// with none the line would cut the area, which is refused.
        gap: u32,
    },
    /// A cell outside the area, beside one of its openings, that a body
    /// can stand on, as a barrel by a shop's door. It flanks the way out:
    /// never the opening, never the cell straight out from it, never
    /// beside any other opening or way, and never where it would cut the
    /// ground outside, so a lane one cell wide takes nothing.
    OutsideDoor,
}

/// How far from the area's openings a thing goes, by the path a body
/// walks in from the nearest one, measured before the dressing is laid,
/// and for [`Behind`](FromDoor::Behind) again round what it stands
/// behind; with no opening, every cell is as far as any other and a rule
/// keeps to none of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum FromDoor {
    /// The nearer half of the cells the spread offers, as a coat peg by
    /// the door.
    Near,
    /// The farther half, as a bed or a hearth.
    Far,
    /// The cells farther than every cell an earlier rule laid this glyph
    /// on, as shelves behind a counter, and every cell the spread offers
    /// while nothing holds the glyph. Each cell is measured by the path
    /// round the things of that glyph, and each of them by the path with
    /// nothing standing, so a cell they stand between the door and is
    /// behind them however near the door it lies; the cells straight
    /// behind a gap in a counter are no farther than its ends, so they
    /// stay clear as the way behind it.
    Behind(char),
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
    /// rounded; fewer where the cells it may take run out first. 0 when
    /// `count` is in force.
    pub density_pct: u32,
    /// How many it lays in place of a share, drawn from `fewest..=most`,
    /// fewer where the cells run out: one hearth in a room however big.
    /// `None` lays `density_pct`.
    pub count: Option<(u32, u32)>,
    /// How far from the openings it keeps among the cells `at` offers;
    /// `None` takes any of them.
    pub from_door: Option<FromDoor>,
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
        let mut place = Place::new(ctx.terrain().width(), ctx.terrain().height(), &areas, &stamps);
        for (index, dressing) in laid {
            let area = areas.iter().find(|a| a.index == index).expect("checked above");
            let d = &self.dressings[dressing];
            let (terrain, tiles, rng) = ctx.dressing_parts();
            let marks = dress_area(terrain, tiles, rng, area, d, Around { keep: &keep, place: Some(&place) }, self.floor);
            for (glyph, at) in marks {
                place.hold(at);
                ctx.emit(Stamped { bounds: GridRect::new(at.x, at.y, 1, 1), marks: vec![(glyph, at)], prefab: d.key });
            }
            ctx.emit(Dressed { area: index, dressing: d.name.clone() });
        }
        Ok(())
    }
}

/// What lies round the areas of a place, which a thing laid outside an
/// area by its opening reads: [`Dress`] keeps one over its whole map, and
/// [`dress_room`], which knows only its room, has none.
///
/// A thing outside an area stands in the next one, or on ground no area
/// holds, so it must keep that area whole as its own dressing would, and
/// an area dressed after it must count it as standing.
struct Place {
    width: i32,
    height: i32,
    /// The area each cell is of, by [`Area::index`].
    owner: Vec<Option<usize>>,
    /// Cells a slot laid for an earlier area stands on.
    held: Vec<bool>,
    /// Cells on or beside a stamp, or of an area painted from a prefab,
    /// which nothing is laid in from outside, since what stands there is
    /// not this pass's to know.
    barred: Vec<bool>,
}

impl Place {
    fn new(width: i32, height: i32, areas: &[Area], stamps: &[GridRect]) -> Self {
        let size = (width * height).max(0) as usize;
        let mut place = Self { width, height, owner: vec![None; size], held: vec![false; size], barred: vec![false; size] };
        for a in areas {
            for c in &a.cells {
                if let Some(i) = place.index(*c) {
                    place.owner[i] = Some(a.index);
                    place.barred[i] |= a.entries.is_some();
                }
            }
        }
        for s in stamps {
            for c in s.inflate(1).cells() {
                if let Some(i) = place.index(c) {
                    place.barred[i] = true;
                }
            }
        }
        place
    }

    fn index(&self, p: Point) -> Option<usize> {
        (p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height).then(|| (p.y * self.width + p.x) as usize)
    }

    fn owner(&self, p: Point) -> Option<usize> {
        self.index(p).and_then(|i| self.owner[i])
    }

    fn held(&self, p: Point) -> bool {
        self.index(p).is_some_and(|i| self.held[i])
    }

    fn barred(&self, p: Point) -> bool {
        self.index(p).is_some_and(|i| self.barred[i])
    }

    fn hold(&mut self, p: Point) {
        if let Some(i) = self.index(p) {
            self.held[i] = true;
        }
    }
}

/// What a dressing keeps clear of round the area it dresses: the cells
/// nothing stands on or beside, and the place round the area, if known.
struct Around<'a> {
    keep: &'a [Point],
    place: Option<&'a Place>,
}

/// One area's cells in a frame [`LOOK`] cells larger than its bounds all round,
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

    /// Whether every cell of `cells` is reached from the first by orthogonal
    /// steps over cells of the frame that `pass`; true for none or one.
    fn meet(&self, cells: &[Point], pass: impl Fn(Point) -> bool) -> bool {
        let Some(&first) = cells.first() else { return true };
        let Some(start) = self.at(first) else { return false };
        let mut seen = vec![false; self.own.len()];
        seen[start] = true;
        let mut stack = vec![start];
        let mut left = cells.iter().filter(|c| **c != first).count();
        while let Some(i) = stack.pop() {
            if left == 0 {
                break;
            }
            let p = self.point(i);
            for d in Direction::CARDINALS {
                let q = p + d.offset();
                if let Some(j) = self.at(q)
                    && !seen[j]
                    && pass(q)
                {
                    seen[j] = true;
                    left -= cells.iter().filter(|c| **c == q).count();
                    stack.push(j);
                }
            }
        }
        left == 0
    }

    /// Whether the cell at `i` has a cell straight beside it that `open`
    /// leaves clear or the rim holds: somewhere a body stands to step onto
    /// it or to use what stands there.
    fn beside_open(&self, open: &[bool], i: usize) -> bool {
        let p = self.point(i);
        Direction::CARDINALS.iter().any(|d| self.at(p + d.offset()).is_some_and(|j| open[j] || self.rim[j]))
    }
}

/// Lays `dressing` on the cells of one room of `terrain`, with no chain
/// to run it, and returns its slots as the glyph and cell of each.
///
/// `cells` are the room's floor and `openings` its doors, or any other way
/// in: by the two rules [`Dress`] keeps, nothing stands on or beside an
/// opening but what is laid outside it, and nothing is laid that cuts the
/// room in more pieces than it was or leaves a slot with no open cell
/// straight beside it. The openings are also the door a rule keeps near
/// or far from, and the door outside which [`Spread::OutsideDoor`] lays;
/// what stands outside is read from `terrain` alone, so a game dressing
/// a house room by room passes the street's things as tiles or not at
/// all. Things are drawn from `rng`, and `floor` is the plain floor a tile
/// is laid over.
pub fn dress_room(
    terrain: &mut Terrain,
    tiles: &TileRegistry,
    cells: &[Point],
    openings: &[Point],
    dressing: &Dressing,
    floor: TileId,
    rng: &mut impl Rng,
) -> Vec<(char, Point)> {
    let Some(first) = cells.first() else { return Vec::new() };
    let bounds = cells.iter().fold(GridRect::new(first.x, first.y, 1, 1), |b, c| b.union(&GridRect::new(c.x, c.y, 1, 1)));
    let area = Area { index: 0, shape: String::new(), district: String::new(), cells: cells.to_vec(), bounds, inner: bounds, entries: None };
    dress_area(terrain, tiles, rng, &area, dressing, Around { keep: openings, place: None }, floor)
}

/// How far round an area its frame reaches: a thing outside a door stands
/// two from the area, a post or a well two out from the door is a cell
/// beyond it, and the street round that is the one beyond, with a cell to
/// spare for an obstacle two deep.
const LOOK: i32 = 5;

/// Lays `dressing` in `area` and repaints it, and returns its slots as
/// marks: each thing on a cell its spread offers, drawn from `rng`, never
/// on or beside a cell of `around.keep` or a way out of the area but
/// outside a door, and never one that leaves the area in more regions
/// than before, that cuts the ground outside, or that leaves a slot with
/// no open cell straight beside it.
fn dress_area(
    terrain: &mut Terrain,
    tiles: &TileRegistry,
    rng: &mut impl Rng,
    area: &Area,
    dressing: &Dressing,
    around: Around<'_>,
    floor: TileId,
) -> Vec<(char, Point)> {
    let walkable = tiles.tables().walkable;
    let walk = |terrain: &Terrain, p: Point| terrain.get(p).is_some_and(|t| walkable[t.index()]);
    let held = |p: Point| around.place.is_some_and(|place| place.held(p));
    let rect = area.bounds.inflate(LOOK);
    let size = (rect.width * rect.height) as usize;
    let mut frame = Frame { rect, own: vec![false; size], rim: vec![false; size] };
    for c in &area.cells {
        // A slot an earlier area laid outside its door may stand here,
        // and stands as anything else does.
        if walk(terrain, *c)
            && !held(*c)
            && let Some(i) = frame.at(*c)
        {
            frame.own[i] = true;
        }
    }
    for i in 0..size {
        let p = frame.point(i);
        frame.rim[i] = !frame.own[i] && walk(terrain, p) && Direction::CARDINALS.iter().any(|d| frame.own(p + d.offset()));
    }
    let ground: Vec<bool> = (0..size).map(|i| walk(terrain, frame.point(i))).collect();
    let near: Vec<bool> = (0..size)
        .map(|i| {
            let p = frame.point(i);
            frame.own[i] && (around.keep.iter().any(|k| beside(p, *k)) || Direction::ALL.iter().any(|d| frame.at(p + d.offset()).is_some_and(|j| frame.rim[j])))
        })
        .collect();

    let spread = Spreads::measure(&frame, area.bounds, &around, &ground);
    let mut open = frame.own.clone();
    let mut taken = vec![false; size];
    let mut regions = frame.regions(&open);
    let mut marks = Vec::new();
    // Every slot laid so far, each asked again after every thing that
    // blocks: the one that shuts a slot in is rarely the slot's own.
    let mut slots: Vec<usize> = Vec::new();
    // Every slot laid outside the area, and every thing laid with its
    // glyph, for the rules that stand beside or behind one.
    let mut outside: Vec<usize> = Vec::new();
    let mut holding: Vec<(char, usize)> = Vec::new();
    for thing in &dressing.things {
        let mut cells = spread.cells(&frame, thing.at, &holding);
        if let Some(from) = thing.from_door {
            cells = spread.keeping(&frame, cells, from, &holding);
        }
        cells.shuffle(rng);
        let want = match thing.count {
            Some((fewest, most)) => rng.random_range(fewest.min(most)..=most.max(fewest)) as usize,
            None => (cells.len() * thing.density_pct as usize + 50) / 100,
        };
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
            let painted = matches!(thing.stands, Stand::Tile(_)) && terrain.get(frame.point(i)) != Some(floor);
            if taken[i] || near[i] || painted {
                continue;
            }
            let slot = thing.stands == Stand::Slot;
            if blocks && frame.own[i] {
                open[i] = false;
                let after = frame.regions(&open);
                let shut_in = slot.then_some(i).into_iter().chain(slots.iter().copied()).any(|s| !frame.beside_open(&open, s));
                if after > regions || shut_in {
                    open[i] = true;
                    continue;
                }
                regions = after;
            } else if blocks {
                let clear = |q: Point| match frame.at(q) {
                    Some(j) if frame.own[j] => open[j],
                    Some(j) => around.keep.contains(&q) || (ground[j] && !taken[j] && !held(q)),
                    None => false,
                };
                let slot_at = |q: Point| held(q) || frame.at(q).is_some_and(|j| outside.contains(&j));
                if !stands_clear_outside(&frame, frame.point(i), slot, clear, slot_at, around.place) {
                    continue;
                }
            }
            taken[i] = true;
            placed += 1;
            holding.push((thing.glyph, i));
            let p = frame.point(i);
            match thing.stands {
                Stand::Tile(t) => {
                    terrain.set(p, t);
                }
                Stand::Slot => {
                    if frame.own[i] {
                        slots.push(i);
                    } else {
                        outside.push(i);
                    }
                    marks.push((thing.glyph, p));
                }
            }
        }
    }
    if let Some((tile, pct)) = dressing.repaint {
        let mut bare: Vec<usize> = (0..size).filter(|&i| frame.own[i] && !taken[i] && terrain.get(frame.point(i)) == Some(floor)).collect();
        bare.shuffle(rng);
        let want = (bare.len() * pct as usize + 50) / 100;
        for i in bare.into_iter().take(want) {
            terrain.set(frame.point(i), tile);
        }
    }
    marks
}

/// Beside: any of the eight cells round, so nothing stands even corner-on
/// to a doorway.
fn beside(p: Point, q: Point) -> bool {
    (p.x - q.x).abs() <= 1 && (p.y - q.y).abs() <= 1
}

/// Whether a thing that blocks may stand on `p`, outside the area being
/// dressed, with `clear` saying where a body may stand now and `slot_at`
/// where a slot stands.
///
/// Read over `frame`: what `p` joins orthogonally it joins through its
/// four straight neighbours, so if those that are clear still meet with
/// `p` taken, by a way within the frame, taking `p` parts nothing. The
/// frame reaches round an obstacle beside the way out, so a barrel by a
/// door with a post two cells out still stands where the street goes
/// round the post; a way that leaves the frame is not seen, which only
/// refuses more, and nor is a way through the area being dressed, so a
/// cell before one door is never joined to the street only in at that
/// door and out at another. Where a [`Place`] is known that is asked of
/// `p`'s own area, its straight neighbours all of it and the way between
/// them too, so neither that area nor the ground between areas is cut;
/// and a slot on `p`, or one beside it, keeps a clear cell straight beside
/// it to be reached from.
fn stands_clear_outside(frame: &Frame, p: Point, slot: bool, clear: impl Fn(Point) -> bool, slot_at: impl Fn(Point) -> bool, place: Option<&Place>) -> bool {
    let mine = place.and_then(|pl| pl.owner(p));
    let ours = |q: Point| clear(q) && place.and_then(|pl| pl.owner(q)) == mine;
    let straight = Direction::CARDINALS.map(|d| p + d.offset());
    if straight.iter().any(|q| clear(*q) && !ours(*q)) {
        return false;
    }
    let joined: Vec<Point> = straight.iter().copied().filter(|q| ours(*q)).collect();
    if !frame.meet(&joined, |q| q != p && ours(q) && !frame.own(q)) {
        return false;
    }
    if slot && joined.is_empty() {
        return false;
    }
    straight.iter().filter(|q| slot_at(**q)).all(|q| Direction::CARDINALS.iter().map(|d| *q + d.offset()).any(|r| r != p && clear(r)))
}

/// The cells each spread offers in one area, measured once.
struct Spreads {
    /// Each own cell's distance to the nearest cell not its own, counting
    /// diagonals: one along a wall, two a cell in.
    depth: Vec<u32>,
    /// Each own cell's distance by path from the nearest opening, stepping
    /// orthogonally; `None` for every cell with no opening to measure from.
    path: Vec<Option<u32>>,
    /// The cells of the line [`Spread::Across`] runs, in order along it.
    line: Vec<usize>,
    /// The cells [`Spread::OutsideDoor`] offers.
    outside: Vec<usize>,
    /// The ways in, which every path is measured from.
    ways: Vec<Point>,
}

impl Spreads {
    fn measure(frame: &Frame, bounds: GridRect, around: &Around<'_>, ground: &[bool]) -> Self {
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
        // The ways into this area: every cell kept clear that is its own
        // or straight beside it. A door is one outside it.
        let touches = |p: Point| frame.own(p) || Direction::CARDINALS.iter().any(|d| frame.own(p + d.offset()));
        let ways: Vec<Point> = around.keep.iter().copied().filter(|k| frame.at(*k).is_some() && touches(*k)).collect();
        let doors: Vec<Point> = ways.iter().copied().filter(|w| !frame.own(*w)).collect();
        Self {
            depth,
            path: Self::paths(frame, &ways, &[]),
            line: Self::line(frame, bounds, &ways),
            outside: Self::outside(frame, around, ground, &doors),
            ways,
        }
    }

    /// Each own cell's distance by path from `ways`, or from the rim where
    /// there are none, as a mouth with no door is still a way in, stepping
    /// round the cells of `standing`, which have none.
    fn paths(frame: &Frame, ways: &[Point], standing: &[usize]) -> Vec<Option<u32>> {
        let size = frame.own.len();
        let mut path = vec![None; size];
        let mut queue: std::collections::VecDeque<(Point, u32)> = ways.iter().map(|w| (*w, 0)).collect();
        if queue.is_empty() {
            queue = (0..size).filter(|&i| frame.rim[i]).map(|i| (frame.point(i), 0)).collect();
        }
        queue.retain(|(p, _)| frame.at(*p).is_none_or(|i| !standing.contains(&i)));
        for (p, _) in &queue {
            if let Some(i) = frame.at(*p).filter(|&i| frame.own[i]) {
                path[i] = Some(0);
            }
        }
        while let Some((p, d)) = queue.pop_front() {
            for dir in Direction::CARDINALS {
                if let Some(j) = frame.at(p + dir.offset())
                    && frame.own[j]
                    && path[j].is_none()
                    && !standing.contains(&j)
                {
                    path[j] = Some(d + 1);
                    queue.push_back((frame.point(j), d + 1));
                }
            }
        }
        path
    }

    /// The line across the area through its middle, square to the way in
    /// from the way nearest the middle.
    fn line(frame: &Frame, bounds: GridRect, ways: &[Point]) -> Vec<usize> {
        // Twice the middle, so a room of even size has one too.
        let (mx, my) = (2 * bounds.x + bounds.width - 1, 2 * bounds.y + bounds.height - 1);
        let nearest = ways.iter().min_by_key(|w| (2 * w.x - mx).pow(2) + (2 * w.y - my).pow(2));
        // Whether the way in runs up or down, so the line is a row, and
        // whether it comes from the low side, the top or the left.
        let (rows, low) = match nearest {
            Some(w) => {
                let inward = Direction::CARDINALS.into_iter().find(|d| frame.own(*w + d.offset())).filter(|_| !frame.own(*w));
                let rows = match inward {
                    Some(d) => d.offset().x == 0,
                    None => (2 * w.y - my).abs() * bounds.width >= (2 * w.x - mx).abs() * bounds.height,
                };
                (rows, if rows { 2 * w.y < my } else { 2 * w.x < mx })
            }
            None => (bounds.height >= bounds.width, true),
        };
        let (start, length) = if rows { (bounds.y, bounds.height) } else { (bounds.x, bounds.width) };
        let at = if low { start + (length - 1) / 2 } else { start + length - 1 - (length - 1) / 2 };
        (0..frame.own.len())
            .filter(|&i| frame.own[i])
            .filter(|&i| {
                let p = frame.point(i);
                if rows { p.y == at } else { p.x == at }
            })
            .collect()
    }

    /// The cells outside the area flanking the way out of each of `doors`.
    fn outside(frame: &Frame, around: &Around<'_>, ground: &[bool], doors: &[Point]) -> Vec<usize> {
        let straight_out: Vec<Point> =
            doors.iter().flat_map(|d| Direction::CARDINALS.into_iter().filter(|dir| frame.own(*d + dir.offset())).map(|dir| *d - dir.offset())).collect();
        let others: Vec<Point> = around.keep.iter().copied().filter(|k| !doors.contains(k)).collect();
        let free = |p: Point| around.place.is_none_or(|place| !place.held(p) && !place.barred(p));
        (0..frame.own.len())
            .filter(|&i| !frame.own[i] && !frame.rim[i] && ground[i])
            .filter(|&i| {
                let p = frame.point(i);
                doors.iter().any(|d| beside(p, *d))
                    && !around.keep.contains(&p)
                    && !straight_out.contains(&p)
                    && !others.iter().any(|k| beside(p, *k))
                    && free(p)
            })
            .collect()
    }

    /// The cells `at` offers, in the frame's order, before any is drawn,
    /// with `holding` every thing laid so far by its glyph.
    fn cells(&self, frame: &Frame, at: Spread, holding: &[(char, usize)]) -> Vec<usize> {
        let size = frame.own.len();
        match at {
            Spread::Across { gap } => {
                let gap = gap as usize;
                let n = self.line.len();
                if gap >= n {
                    return Vec::new();
                }
                let from = (n - gap) / 2;
                return self.line.iter().enumerate().filter(|(k, _)| !(from..from + gap).contains(k)).map(|(_, i)| *i).collect();
            }
            Spread::OutsideDoor => return self.outside.clone(),
            _ => {}
        }
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
                    Spread::Beside(g) => Direction::CARDINALS.iter().any(|d| frame.at(p + d.offset()).is_some_and(|j| holding.contains(&(g, j)))),
                    Spread::Across { .. } | Spread::OutsideDoor => false,
                }
            })
            .collect()
    }

    /// Those of `cells` that keep as far from the openings as `from` asks,
    /// in the order given; all of them where the area has no opening.
    fn keeping(&self, frame: &Frame, cells: Vec<usize>, from: FromDoor, holding: &[(char, usize)]) -> Vec<usize> {
        if self.path.iter().all(Option::is_none) {
            return cells;
        }
        let mut far: Vec<u32> = cells.iter().filter_map(|&i| self.path[i]).collect();
        far.sort_unstable();
        let keep = |test: &dyn Fn(u32) -> bool| cells.iter().copied().filter(|&i| self.path[i].is_some_and(test)).collect();
        match from {
            FromDoor::Near => match far.get(far.len().saturating_sub(1) / 2) {
                Some(&median) => keep(&|d| d <= median),
                None => Vec::new(),
            },
            FromDoor::Far => match far.get(far.len() / 2) {
                Some(&median) => keep(&|d| d >= median),
                None => Vec::new(),
            },
            FromDoor::Behind(g) => {
                let standing: Vec<usize> = holding.iter().filter(|(c, _)| *c == g).map(|(_, i)| *i).collect();
                let Some(front) = standing.iter().filter_map(|&i| self.path[i]).max() else { return cells };
                // Measured again with them standing, so a cell they
                // stand between the door and is reached round them.
                let round = Self::paths(frame, &self.ways, &standing);
                cells.into_iter().filter(|&i| round[i].is_some_and(|d| d > front)).collect()
            }
        }
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
    use crate::testing::{first_seeds, par_seeds, seeds};
    use atglyph_core::{Grid2D, RunSeed};
    use atglyph_grid::{CostSource, DijkstraMap, PathRules, TileProps};
    use rand::SeedableRng;
    use rand::rngs::StdRng;
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
        Thing { glyph, stands: Stand::Slot, at, density_pct, count: None, from_door: None }
    }

    /// A slot laid exactly `count` times, fewer only where its cells run
    /// out.
    fn counted(glyph: char, at: Spread, count: (u32, u32)) -> Thing {
        Thing { count: Some(count), density_pct: 0, ..slot(glyph, at, 0) }
    }

    /// A house's room of `w` by `h` at (2, 3), as [`room`] lays it, and its
    /// cells and its door.
    fn house(r: &Rig, w: i32, h: i32) -> (Terrain, Vec<Point>, Point) {
        let ctx = room(r, w, h);
        (ctx.terrain().clone(), GridRect::new(2, 3, w, h).cells().collect(), Point::new(2 + w / 2, 2))
    }

    /// Each cell of `cells` and its distance by path from `door`, stepping
    /// orthogonally over `cells` alone.
    fn from_door(cells: &[Point], door: Point) -> Vec<(Point, u32)> {
        let mut dist: Vec<(Point, u32)> = Vec::new();
        let mut queue = std::collections::VecDeque::from([(door, 0)]);
        let mut seen = BTreeSet::from([door]);
        while let Some((p, d)) = queue.pop_front() {
            if p != door {
                dist.push((p, d));
            }
            for q in Direction::CARDINALS.map(|dir| p + dir.offset()) {
                if cells.contains(&q) && seen.insert(q) {
                    queue.push_back((q, d + 1));
                }
            }
        }
        dist
    }

    fn glyphs(marks: &[(char, Point)], glyph: char) -> Vec<Point> {
        marks.iter().filter(|(g, _)| *g == glyph).map(|(_, p)| *p).collect()
    }

    /// One hearth in a room however big, by a count where a density would
    /// give a big room many.
    #[test]
    fn exactly_one_hearth_however_big_the_room() {
        let (r, ..) = tiles();
        for s in 0..20 {
            let (mut terrain, cells, door) = house(&r, 9, 7);
            let d = dressing("home", vec![counted('H', Spread::Walls, (1, 1))], None);
            let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(s));
            assert_eq!(glyphs(&marks, 'H').len(), 1, "seed {s}: {marks:?}");
        }
    }

    /// A count more than the cells its spread offers lays what there is.
    #[test]
    fn a_count_is_cut_to_the_room_there_is() {
        let (r, ..) = tiles();
        let (mut terrain, cells, door) = house(&r, 5, 5);
        let d = dressing("middle", vec![counted('x', Spread::Center, (3, 3))], None);
        let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(1));
        assert_eq!(marks, vec![('x', Point::new(4, 5))], "the one middle cell of a five-cell square");
    }

    /// Beds far from the door stand in the farther half, by path, of the
    /// cells along the walls.
    #[test]
    fn far_from_the_door_keeps_to_the_far_half() {
        let (r, ..) = tiles();
        let walls = walls_of(GridRect::new(2, 3, 9, 7));
        for s in 0..20 {
            let (mut terrain, cells, door) = house(&r, 9, 7);
            let dist = from_door(&cells, door);
            let far = |p: Point| dist.iter().find(|(q, _)| *q == p).unwrap().1;
            let mut offered: Vec<u32> = walls.iter().map(|p| far(*p)).collect();
            offered.sort_unstable();
            let median = offered[(offered.len() - 1) / 2];
            let d = dressing("beds", vec![Thing { from_door: Some(FromDoor::Far), ..slot('B', Spread::Walls, 100) }], None);
            let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(s));
            let beds = glyphs(&marks, 'B');
            assert!(!beds.is_empty(), "seed {s}: no bed");
            assert!(beds.iter().all(|p| far(*p) >= median), "seed {s}: a bed nearer the door than {median}: {beds:?}");
        }
    }

    /// A stool stands straight beside a table an earlier rule laid.
    #[test]
    fn a_stool_stands_beside_its_table() {
        let (r, ..) = tiles();
        for s in 0..20 {
            let (mut terrain, cells, door) = house(&r, 9, 7);
            let d = dressing("tavern", vec![slot('T', Spread::Center, 100), slot('h', Spread::Beside('T'), 50)], None);
            let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(s));
            let (tables, stools) = (glyphs(&marks, 'T'), glyphs(&marks, 'h'));
            assert!(!tables.is_empty() && !stools.is_empty(), "seed {s}: {marks:?}");
            for p in &stools {
                assert!(Direction::CARDINALS.iter().any(|d| tables.contains(&(*p + d.offset()))), "seed {s}: the stool at {p:?} is beside no table");
            }
        }
    }

    /// A counter runs across the room's middle, square to the way in from
    /// the door, and leaves one cell of its line open to pass.
    #[test]
    fn a_counter_runs_across_the_room_with_a_gap() {
        let (r, ..) = tiles();
        for s in 0..20 {
            let (mut terrain, cells, door) = house(&r, 9, 7);
            let d = dressing("shop", vec![slot('C', Spread::Across { gap: 1 }, 100)], None);
            let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(s));
            let counter = glyphs(&marks, 'C');
            // The door is in the top wall, so the way in runs down and the
            // line runs along a row.
            let row = counter.first().map(|p| p.y).unwrap_or_else(|| panic!("seed {s}: no counter"));
            assert!(counter.iter().all(|p| p.y == row), "seed {s}: the counter is not one row: {counter:?}");
            assert_eq!(row, 6, "seed {s}: the row through the room's middle");
            let line = cells.iter().filter(|p| p.y == row).count();
            assert_eq!(line - counter.len(), 1, "seed {s}: one cell of the line is left open: {counter:?}");
            assert_eq!(regions(&cells, &counter), 1, "seed {s}: the room was cut");
        }
    }

    /// A shop's counter across and its shelves behind it, every shelf
    /// along a wall.
    fn shop() -> Dressing {
        dressing(
            "shop",
            vec![slot('C', Spread::Across { gap: 1 }, 100), Thing { from_door: Some(FromDoor::Behind('C')), ..slot('S', Spread::Walls, 100) }],
            None,
        )
    }

    /// Shelves behind a counter stand farther from the door, by the path
    /// round the counter through its gap, than any of the counter stands
    /// by the path with nothing in the way.
    #[test]
    fn shelves_stand_behind_the_counter() {
        let (r, ..) = tiles();
        for s in 0..20 {
            let (mut terrain, cells, door) = house(&r, 9, 7);
            let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &shop(), r.floor, &mut StdRng::seed_from_u64(s));
            let (counter, shelves) = (glyphs(&marks, 'C'), glyphs(&marks, 'S'));
            assert!(!counter.is_empty() && !shelves.is_empty(), "seed {s}: {marks:?}");
            let bare = from_door(&cells, door);
            let front = counter.iter().map(|p| bare.iter().find(|(q, _)| q == p).unwrap().1).max().unwrap();
            let open: Vec<Point> = cells.iter().copied().filter(|p| !counter.contains(p)).collect();
            let round = from_door(&open, door);
            assert!(
                shelves.iter().all(|p| round.iter().find(|(q, _)| q == p).unwrap().1 > front),
                "seed {s}: a shelf no farther round the counter than its {front}: {shelves:?}"
            );
            let stood: Vec<Point> = counter.iter().chain(&shelves).copied().collect();
            assert_eq!(regions(&cells, &stood), 1, "seed {s}: the room was cut");
        }
    }

    /// A room four across and six down, its door at the foot of its west
    /// wall: the counter runs down it with its gap above the door's row,
    /// its top end in the far corner from the door. The cells behind it
    /// on the door's side of the gap are near the door with nothing
    /// standing, yet a body walks round through the gap to reach them, so
    /// they are behind the counter and shelves stand along the east wall
    /// there too.
    #[test]
    fn shelves_stand_all_along_the_far_wall_behind_a_counter_that_ends_in_the_far_corner() {
        let (r, ..) = tiles();
        let bounds = GridRect::new(3, 3, 4, 6);
        let door = Point::new(2, 8);
        let mut low = 0;
        for s in 0..20 {
            let mut terrain = Terrain::from_fn(10, 12, |p| if bounds.contains(p) || p == door.offset(-1, 0) { r.floor } else { r.wall });
            terrain.set(door, r.door);
            let cells: Vec<Point> = bounds.cells().collect();
            let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &shop(), r.floor, &mut StdRng::seed_from_u64(s));
            let (counter, shelves) = (glyphs(&marks, 'C'), glyphs(&marks, 'S'));
            assert!(
                counter.iter().all(|p| p.x == 4) && !counter.contains(&Point::new(4, 5)),
                "seed {s}: the counter runs down x 4 with its gap at y 5: {counter:?}"
            );
            assert!(shelves.iter().all(|p| p.x >= 5), "seed {s}: a shelf before the counter: {shelves:?}");
            assert!(!shelves.contains(&Point::new(5, 5)), "seed {s}: the way behind the gap is clear: {shelves:?}");
            low += shelves.iter().filter(|p| p.x == 6 && p.y >= 6).count();
            let stood: Vec<Point> = counter.iter().chain(&shelves).copied().collect();
            assert_eq!(regions(&cells, &stood), 1, "seed {s}: the room was cut");
        }
        assert!(low > 0, "no shelf on the east wall below the gap");
    }

    /// A room with a street two cells wide outside its door, along the top
    /// of the map: its terrain, cells and door. With `wide` false the street
    /// is one cell wide.
    fn on_a_street(r: &Rig, wide: bool) -> (Terrain, Vec<Point>, Point) {
        let (mut terrain, cells, door) = house(r, 7, 5);
        for x in 0..terrain.width() {
            terrain.set(Point::new(x, 1), r.floor);
            terrain.set(Point::new(x, 0), if wide { r.floor } else { r.wall });
        }
        (terrain, cells, door)
    }

    /// A barrel outside the door stands on the street flanking the way out,
    /// never on the door nor on the cell straight out from it.
    #[test]
    fn a_barrel_stands_outside_the_door() {
        let (r, ..) = tiles();
        for s in 0..20 {
            let (mut terrain, cells, door) = on_a_street(&r, true);
            let d = dressing("store", vec![slot('b', Spread::OutsideDoor, 100)], None);
            let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(s));
            let barrels = glyphs(&marks, 'b');
            let walkable = r.tiles.tables().walkable;
            for p in &barrels {
                assert!(!cells.contains(p) && *p != door && *p != door.offset(0, -1), "seed {s}: a barrel in the way at {p:?}");
                assert!((p.x - door.x).abs() <= 1 && (p.y - door.y).abs() <= 1, "seed {s}: a barrel away from the door at {p:?}");
                assert!(terrain.get(*p).is_some_and(|t| walkable[t.index()]), "seed {s}: a barrel on no ground at {p:?}");
            }
            assert_eq!(barrels.len(), 2, "seed {s}: both sides of the way out: {barrels:?}");
        }
    }

    /// Outside a door on a street three cells wide with a pillar two cells
    /// straight out from the door, a barrel flanking the way out leaves
    /// the cell before the door joined to the street round the pillar, so
    /// one is laid; the second would shut that cell in with the door.
    #[test]
    fn a_barrel_stands_outside_a_door_with_a_pillar_two_out() {
        let (r, pillar, _) = tiles();
        let bounds = GridRect::new(2, 6, 7, 5);
        let door = Point::new(5, 5);
        let street = GridRect::new(0, 2, 13, 3);
        let cells: Vec<Point> = bounds.cells().collect();
        for s in 0..20 {
            let mut terrain = Terrain::from_fn(13, 14, |p| if bounds.contains(p) || street.contains(p) { r.floor } else { r.wall });
            terrain.set(door, r.door);
            terrain.set(door.offset(0, -2), pillar);
            let d = dressing("store", vec![slot('b', Spread::OutsideDoor, 100)], None);
            let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(s));
            let barrels = glyphs(&marks, 'b');
            assert_eq!(barrels.len(), 1, "seed {s}: one side of the way out: {barrels:?}");
            assert!([door.offset(-1, -1), door.offset(1, -1)].contains(&barrels[0]), "seed {s}: {barrels:?}");
        }
    }

    /// Outside two doors onto one street three cells wide, a pillar two
    /// cells straight out from the first, the cell before each door stays
    /// joined to the street outside: never by a way in through one door,
    /// across the room and out through the other.
    #[test]
    fn a_barrel_by_one_of_two_doors_never_leaves_its_way_out_reached_only_through_the_room() {
        let (r, pillar, _) = tiles();
        let bounds = GridRect::new(2, 6, 7, 5);
        let doors = [Point::new(3, 5), Point::new(7, 5)];
        let street = GridRect::new(0, 2, 13, 3);
        let cells: Vec<Point> = bounds.cells().collect();
        let walkable = r.tiles.tables().walkable;
        for s in 0..20 {
            let mut terrain = Terrain::from_fn(13, 14, |p| if bounds.contains(p) || street.contains(p) { r.floor } else { r.wall });
            for door in doors {
                terrain.set(door, r.door);
            }
            terrain.set(doors[0].offset(0, -2), pillar);
            let d = dressing("store", vec![slot('b', Spread::OutsideDoor, 100)], None);
            let marks = dress_room(&mut terrain, &r.tiles, &cells, &doors, &d, r.floor, &mut StdRng::seed_from_u64(s));
            let barrels = glyphs(&marks, 'b');
            // The street's open cells, the room and its doors left out.
            let open = |p: Point| street.contains(p) && terrain.get(p).is_some_and(|t| walkable[t.index()]) && !barrels.contains(&p);
            for door in doors {
                let out = door.offset(0, -1);
                let mut seen = vec![out];
                let mut i = 0;
                while i < seen.len() {
                    for d in Direction::CARDINALS {
                        let q = seen[i] + d.offset();
                        if open(q) && !seen.contains(&q) {
                            seen.push(q);
                        }
                    }
                    i += 1;
                }
                assert!(seen.contains(&Point::new(0, 2)), "seed {s}: the way out of {door:?} is shut off the street by {barrels:?}");
            }
        }
    }

    /// Outside a door on a lane one cell wide, a barrel on either side of
    /// the way out would cut the lane, so none is laid.
    #[test]
    fn a_barrel_outside_the_door_never_cuts_a_lane_one_wide() {
        let (r, ..) = tiles();
        let (mut terrain, cells, door) = on_a_street(&r, false);
        let d = dressing("store", vec![slot('b', Spread::OutsideDoor, 100)], None);
        let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(1));
        assert_eq!(marks, Vec::new());
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

    /// A room dressed with no chain: the dressing lands on the room's own
    /// cells and keeps off its door.
    #[test]
    fn a_room_is_dressed_on_its_own_without_a_chain() {
        let (r, ..) = tiles();
        let ctx = room(&r, 7, 5);
        let cells: Vec<Point> = GridRect::new(2, 3, 7, 5).cells().collect();
        let door = Point::new(2 + 7 / 2, 2);
        let mut terrain = ctx.terrain().clone();
        let d = dressing("beds", vec![slot('b', Spread::Walls, 100)], None);
        let marks = dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(1));
        assert!(!marks.is_empty());
        assert!(marks.iter().all(|&(g, p)| g == 'b' && cells.contains(&p)));
        assert!(marks.iter().all(|&(_, p)| (p.x - door.x).abs().max((p.y - door.y).abs()) > 1), "nothing on or beside the door");
    }

    /// A room dressed with things that block is never left in pieces.
    #[test]
    fn a_dressed_room_is_never_cut_in_two() {
        let (r, pillar, _) = tiles();
        for s in 0..20 {
            let ctx = room(&r, 5, 3);
            let cells: Vec<Point> = GridRect::new(2, 3, 5, 3).cells().collect();
            let door = Point::new(2 + 5 / 2, 2);
            let mut terrain = ctx.terrain().clone();
            let d = dressing(
                "junk",
                vec![Thing { glyph: 'x', stands: Stand::Tile(pillar), at: Spread::Scatter, density_pct: 100, count: None, from_door: None }],
                None,
            );
            dress_room(&mut terrain, &r.tiles, &cells, &[door], &d, r.floor, &mut StdRng::seed_from_u64(s));
            let blocked: Vec<Point> = cells.iter().copied().filter(|&c| terrain.get(c) == Some(pillar)).collect();
            assert!(!blocked.is_empty(), "seed {s}: nothing was laid");
            assert_eq!(regions(&cells, &blocked), 1, "seed {s}: the room was cut: {blocked:?}");
        }
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
                Thing { glyph: 'p', stands: Stand::Tile(pillar), at: Spread::Center, density_pct: 100, count: None, from_door: None },
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

    /// A room whose door opens on a lane one cell wide, with a yard open
    /// beside the lane: a barrel flanking the door is refused while the
    /// lane and the yard are two areas, since the yard joining round it
    /// would leave the lane in two, and laid on both sides once they are
    /// one area.
    #[test]
    fn a_barrel_outside_the_door_never_cuts_the_area_it_stands_in() {
        let (r, ..) = tiles();
        let barrels = |one: bool| {
            let mut ctx = BaseContext::blank(13, 10, r.tiles.clone(), r.wall);
            let (room, lane, yard) = (GridRect::new(2, 3, 7, 5), GridRect::new(0, 1, 13, 1), GridRect::new(0, 0, 13, 1));
            for c in room.cells().chain(lane.cells()).chain(yard.cells()) {
                ctx.terrain_mut().set(c, r.floor);
            }
            let door = Point::new(5, 2);
            ctx.terrain_mut().set(door, r.door);
            let area = |index: usize, district: &str, cells: Vec<Point>, bounds: GridRect| Area {
                index,
                shape: "room".into(),
                district: district.into(),
                cells,
                bounds,
                inner: bounds,
                entries: None,
            };
            ctx.emit(Room(room));
            ctx.emit(area(0, "d", room.cells().collect(), room));
            if one {
                ctx.emit(area(1, "out", lane.cells().chain(yard.cells()).collect(), lane.union(&yard)));
            } else {
                ctx.emit(area(1, "out", lane.cells().collect(), lane));
                ctx.emit(area(2, "out", yard.cells().collect(), yard));
            }
            ctx.emit(Opening { cells: vec![door], facing: Direction::North, areas: (0, 1), join: Join::Door });
            Chain::new().then(dress(r.floor, dressing("store", vec![slot('b', Spread::OutsideDoor, 100)], None))).run(&mut ctx, RunSeed(1)).unwrap();
            let mut got: Vec<Point> = slots(&ctx).into_iter().map(|(_, p)| p).collect();
            got.sort_unstable();
            got
        };
        assert_eq!(barrels(false), Vec::new(), "the lane would be cut");
        assert_eq!(barrels(true), vec![Point::new(4, 1), Point::new(6, 1)]);
    }

    /// Over grown places of rectangles, crosses and caves, with secrets
    /// hidden, the rules a house is furnished by, a counted hearth far from
    /// the door, tables and stools beside them, a counter across with
    /// shelves behind it, things near the door and barrels outside it, keep
    /// what every dressing keeps: nothing but a barrel on or beside an
    /// opening or a way, and no barrel on one; every area one region with
    /// every cell of it reached; and the whole place walked from the start.
    #[test]
    fn the_rules_a_house_is_furnished_by_keep_every_area_whole_and_its_openings_clear() {
        let (r, ..) = tiles();
        let d = dressing(
            "house",
            vec![
                Thing { from_door: Some(FromDoor::Far), ..counted('H', Spread::Walls, (1, 1)) },
                counted('T', Spread::Center, (1, 2)),
                slot('h', Spread::Beside('T'), 50),
                slot('C', Spread::Across { gap: 1 }, 100),
                Thing { from_door: Some(FromDoor::Behind('C')), ..slot('S', Spread::Walls, 100) },
                Thing { from_door: Some(FromDoor::Near), ..slot('n', Spread::Corners, 100) },
                slot('b', Spread::OutsideDoor, 100),
            ],
            None,
        );
        let pass = Dress {
            name: "dress",
            floor: r.floor,
            dressings: vec![d],
            districts: vec![DistrictDressings::new("rooms", vec![("house".into(), 1)]), DistrictDressings::new("caves", vec![("house".into(), 1)])],
        };
        let hide = HideOpenings { loops: (1, 3), rooms: (1, 2), loop_spot: 5, room_spot: 5, at_least_rooms: 0 };
        let laid: Vec<AtomicUsize> = (0..7).map(|_| AtomicUsize::new(0)).collect();
        let order = ['H', 'T', 'h', 'C', 'S', 'n', 'b'];
        par_seeds(seeds(0..120), |s| {
            let chain = Chain::new().then(grow(&r)).then(OpenLoops { door: r.door, min_detour: 20, max: 6 });
            let chain = chain.then(RandomStart).then(FarthestExit).then(Survey).then(hide).then(pass.clone());
            let ctx = chain.run_retrying(|| BaseContext::blank(84, 48, r.tiles.clone(), r.wall), RunSeed(s), 20).unwrap_or_else(|e| panic!("seed {s}: {e}"));
            let got = slots(&ctx);
            for (k, glyph) in order.iter().enumerate() {
                laid[k].fetch_add(got.iter().filter(|(g, _)| g == glyph).count(), Relaxed);
            }
            let doors: Vec<Point> = ctx.outputs().iter::<Opening>().flat_map(|o| o.cells.clone()).collect();
            let keep: Vec<Point> = doors
                .iter()
                .copied()
                .chain(ctx.outputs().iter::<SecretCell>().map(|s| s.at))
                .chain([ctx.outputs().first::<StartPoint>().unwrap().0, ctx.outputs().first::<ExitPoint>().unwrap().0])
                .collect();
            let beside = |p: Point, k: Point| (p.x - k.x).abs() <= 1 && (p.y - k.y).abs() <= 1;
            for &(glyph, p) in &got {
                if glyph == 'b' {
                    assert!(!keep.contains(&p), "seed {s}: a barrel on an opening or a way at {p:?}");
                    assert!(keep.iter().any(|k| beside(p, *k)), "seed {s}: a barrel beside no opening at {p:?}");
                } else {
                    assert!(keep.iter().all(|k| !beside(p, *k)), "seed {s}: '{glyph}' on or beside an opening or a way at {p:?}");
                }
            }
            let stood: BTreeSet<Point> = got.iter().map(|(_, p)| *p).collect();
            let blocked: Vec<Point> = stood.iter().copied().collect();
            let walkable = ctx.tiles().tables().walkable;
            for a in ctx.outputs().iter::<Area>() {
                let open: Vec<Point> = a.cells.iter().copied().filter(|c| ctx.terrain().get(*c).is_some_and(|t| walkable[t.index()])).collect();
                assert_eq!(pieces(&open, &[], &blocked), 1, "seed {s}: area {} ({}) was cut", a.index, a.shape);
                let lost = unreached(&ctx, a, &[], &stood);
                assert!(lost.is_empty(), "seed {s}: area {} ({}) has cells no mover reaches: {lost:?}", a.index, a.shape);
            }
            let stranded = beyond_reach(&ctx, &stood);
            assert!(stranded.is_empty(), "seed {s}: cells no walk from the start reaches: {stranded:?}");
        });
        let laid: Vec<usize> = laid.into_iter().map(AtomicUsize::into_inner).collect();
        assert!(laid.iter().all(|n| *n > 0), "every rule laid something, or this proves little: {:?}", order.iter().zip(&laid).collect::<Vec<_>>());
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
        let d = dressing(
            "heaps",
            vec![Thing { glyph: 'h', stands: Stand::Tile(rubble), at: Spread::Scatter, density_pct: 100, count: None, from_door: None }],
            None,
        );
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
