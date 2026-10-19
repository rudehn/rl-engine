//! A layout: the shapes a place is grown from, the districts that favor
//! some of them, and the recipes a place names.
//!
//! A layout file is [`LayoutDef`], read once by [`load`], which resolves
//! every name in it to an index and reports every mistake at once. A
//! shape is one of the generators the engine ships, with its ranges and
//! how it is joined; a district is a region's weighted taste in shapes; a
//! recipe is a size, how full to grow it, which districts to scatter, the
//! barriers growth goes round, how many detours to cut through, and what
//! the built place must meet or be built again.
//!
//! What this module does not hold is the growing. The passes are
//! `rl-mapgen`'s, a tier 1 crate beside this one; this crate names no
//! mapgen type and keeps no dependency on it, so a layout file loads and
//! every one of its mistakes is reported with no map, no `App` and no grid
//! anywhere in the test.
//!
//! ```text
//! // A layout: the shapes a place is grown from, the districts that favor
//! // some of them, and the recipes a place names.
//! //
//! // Every field:
//! //   shapes: every shape a district may draw
//! //     name: the shape's name; an area remembers it, and nothing in the
//! //           engine reads a meaning into it
//! //     kind: one of, every range from 1 to 4096
//! //       Rectangle(w: (fewest, most), h: (fewest, most))
//! //       Cross(long: (fewest, most), short: (fewest, most))
//! //                         two bars overlapped on their centers; the most
//! //                         of short is less than the fewest of long
//! //       Passage(width: (fewest, most), length: (fewest, most), bend_pct: 0)
//! //                         bend_pct is the chance in a hundred of a second
//! //                         leg at a right angle; a passage is not a room,
//! //                         and nothing is stamped in it
//! //       Cave(w: (fewest, most), h: (fewest, most), fill_pct: 55, visits_pct: 100)
//! //                         grown by the cellular automaton inside a box of
//! //                         w by h, and its largest region kept; fill_pct is
//! //                         the share of the box that starts as rock, 1 to
//! //                         99, and visits_pct how many cells the rule is
//! //                         applied at as a share of the box, 1 to 10000
//! //     join: how a piece of it is joined to what it is attached to, one of
//! //       Door              one cell of the door tile
//! //       Arch              one cell of floor
//! //       Mouth(n)          floor up to n cells wide, n at least 1
//! //   districts: a region's taste in shapes
//! //     name:   the district's name; an area remembers it
//! //     shapes: [(shape, weight)], at least one weight more than nought
//! //   recipes: what a place names to be built
//! //     name:              the recipe's name
//! //     size:              (width, height) in cells, each 3 to 4096
//! //     open_pct:          the share of the map to open, 1 to 100
//! //     late_pct:          optional, the share open before the pieces a
//! //                        place must hold are laid, 0 to open_pct; two
//! //                        thirds of open_pct, rounded down, if left out
//! //     patience:          attempts in a row that may fail before growth
//! //                        gives up, more than nought
//! //     districts_per_10k: district seeds per ten thousand cells, rounded,
//! //                        never fewer than one seed; at most 10000
//! //     districts:         [(district, weight)], at least one weight more
//! //                        than nought
//! //     barriers:          optional, none if left out: rock growth goes
//! //                        round, so a place bends round it
//! //       from:       (x_pct, y_pct), one end as shares of the width and
//! //                   height, each 0 to 100
//! //       to:         (x_pct, y_pct), the other end
//! //       chance_pct: the chance in a hundred it is laid, 0 to 100
//! //       jitter_pct: the furthest both ends are shifted together, as a
//! //                   share of the width and of the height, 0 to 100
//! //                   A barrier a shift could lay from one edge of the
//! //                   map to the opposite is refused; one cutting off a
//! //                   corner is not, and the corner stays rock.
//! //     loops:             (min_detour, max): the shortest detour in steps
//! //                        worth a door, and the most doors
//! //     require:           what the built place must meet or be built again
//! //       open_pct:     (at least, at most)
//! //       areas:        [(shape, fewest)], each a shape some district of
//! //                     this recipe draws
//! //       loops:        fewest, no more than loops' max
//! //       dead_end_pct: the largest share of areas with one way in, 0 to 100
//! //       route:        fewest steps from the way in to the way out
//! (
//!     shapes: [
//!         (name: "room", kind: Rectangle(w: (5, 9), h: (5, 8)), join: Door),
//!         (name: "run", kind: Passage(width: (1, 2), length: (4, 12), bend_pct: 30), join: Arch),
//!     ],
//!     districts: [
//!         (name: "rooms", shapes: [("room", 4), ("run", 1)]),
//!     ],
//!     recipes: [
//!         (
//!             name: "floor", size: (60, 40), open_pct: 35, patience: 200, districts_per_10k: 5,
//!             districts: [("rooms", 1)],
//!             barriers: [(from: (50, 0), to: (50, 60), chance_pct: 50, jitter_pct: 10)],
//!             loops: (min_detour: 20, max: 4),
//!             require: (open_pct: (30, 60), areas: [("room", 4)], loops: 1, dead_end_pct: 60, route: 20),
//!         ),
//!     ],
//! )
//! ```

use serde::Deserialize;

use crate::content::ContentError;

/// The largest a map's side, a shape's range or a district density may
/// be: far past anything a place is, and small enough that no sum of them
/// overflows.
const MOST: i32 = 4096;

/// The most a cave's visits may be, a hundred to a cell: far past where
/// more visits change the cave. A ceiling against a slip of the keyboard
/// and an overflow, not a promise of speed: a box of the largest sides at
/// this many is billions of visits, while a cave a place would hold is
/// drawn in moments even here.
const MOST_VISITS_PCT: u32 = 10_000;

/// Which of the engine's generators draws a shape, and its ranges, each
/// `(fewest, most)` in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum ShapeKind {
    /// A rectangle.
    Rectangle {
        /// The width.
        w: (i32, i32),
        /// The height.
        h: (i32, i32),
    },
    /// Two bars overlapped on their centers.
    Cross {
        /// A bar's length.
        long: (i32, i32),
        /// A bar's thickness, all of it less than any length.
        short: (i32, i32),
    },
    /// A run of floor, straight or with one bend. Not a room.
    Passage {
        /// The width.
        width: (i32, i32),
        /// The length of each leg.
        length: (i32, i32),
        /// The chance in a hundred of a second leg at a right angle.
        bend_pct: u32,
    },
    /// A cave grown by the cellular automaton inside a box, its largest
    /// region kept.
    Cave {
        /// The box's width.
        w: (i32, i32),
        /// The box's height.
        h: (i32, i32),
        /// The share of the box that starts as rock, 1 to 99.
        fill_pct: u32,
        /// How many cells the rule is applied at, as a share of the box's
        /// cells, more than nought.
        visits_pct: u32,
    },
}

impl ShapeKind {
    /// Whether the smallest piece this kind can draw lies inside `wide` by
    /// `tall`. A rectangle and a cross are drawn one way up; a passage
    /// lies or stands, so either will do. A cave lies inside its box, so
    /// the smallest box is what must fit.
    fn fits(self, wide: i32, tall: i32) -> bool {
        match self {
            ShapeKind::Rectangle { w, h } | ShapeKind::Cave { w, h, .. } => w.0 <= wide && h.0 <= tall,
            ShapeKind::Cross { long, .. } => long.0 <= wide && long.0 <= tall,
            ShapeKind::Passage { width, length, .. } => {
                let (long, short) = (length.0.max(width.0), width.0);
                (long <= wide && short <= tall) || (long <= tall && short <= wide)
            }
        }
    }

    /// The largest rectangles of floor a piece of this kind can offer what
    /// is stamped inside it, each `(wide, tall)`: a rectangle's largest
    /// sides, a cross's longest bar lying and standing, and a cave's
    /// largest box. A cave never fills its box, so that one is a bound
    /// rather than a promise. None for a passage, which is no room and has
    /// nothing stamped in it.
    pub(crate) fn largest_rooms(self) -> Vec<(i32, i32)> {
        match self {
            ShapeKind::Rectangle { w, h } | ShapeKind::Cave { w, h, .. } => vec![(w.1, h.1)],
            ShapeKind::Cross { long, short } => vec![(long.1, short.1), (short.1, long.1)],
            ShapeKind::Passage { .. } => Vec::new(),
        }
    }

    /// Whether some piece of this kind could hold a stamp `wide` by `tall`
    /// with a cell of its floor all round it, the margin `rl-mapgen` keeps
    /// round anything stamped inside a room. Read from the ranges alone, so
    /// a `false` is certain and a `true` only says the ranges allow it: a
    /// stamp that fits no piece of the shape at its largest fits none of
    /// it ever drawn, which a load can say before any place is grown.
    pub fn could_hold(self, wide: i32, tall: i32) -> bool {
        self.largest_rooms().into_iter().any(|(w, h)| wide + 2 <= w && tall + 2 <= h)
    }

    /// Each range by the name the file gives it.
    fn ranges(self) -> Vec<(&'static str, (i32, i32))> {
        match self {
            ShapeKind::Rectangle { w, h } | ShapeKind::Cave { w, h, .. } => vec![("w", w), ("h", h)],
            ShapeKind::Cross { long, short } => vec![("long", long), ("short", short)],
            ShapeKind::Passage { width, length, .. } => vec![("width", width), ("length", length)],
        }
    }
}

/// How a piece is joined to what it is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum JoinDef {
    /// One cell of the door tile.
    Door,
    /// One cell of floor.
    Arch,
    /// Floor up to this many cells wide, at least one.
    Mouth(u8),
}

/// One shape a district may draw.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShapeDef {
    /// The shape's name.
    pub name: String,
    /// Which generator draws it, and its ranges.
    pub kind: ShapeKind,
    /// How it is joined.
    pub join: JoinDef,
}

/// A region's taste in shapes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistrictDef {
    /// The district's name.
    pub name: String,
    /// Each an index into [`LayoutDef::shapes`] and its weight.
    pub shapes: Vec<(usize, u32)>,
}

/// How many detours to cut through once a place is grown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopsDef {
    /// The shortest detour, in steps, worth a door.
    pub min_detour: u32,
    /// The most doors.
    pub max: u32,
}

/// A segment of rock growth goes round, in shares of the map, laid at a
/// chance and shifted by a jitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BarrierDef {
    /// One end, as shares of the width and height, each 0 to 100.
    pub from: (u32, u32),
    /// The other end.
    pub to: (u32, u32),
    /// The chance in a hundred it is laid.
    pub chance_pct: u32,
    /// The furthest both ends are shifted together, as a share of the
    /// width along a row and of the height along a column.
    pub jitter_pct: u32,
}

/// The cell a share of a side of `side` cells falls on, rounded: nought
/// the first and a hundred the last. `rl-mapgen`'s `cell_at_share` is its
/// twin, which lays the barrier this checks.
fn cell_at_share(pct: u32, side: i32) -> i32 {
    ((i64::from(pct) * i64::from(side - 1) + 50) / 100) as i32
}

/// How near an edge a barrier's end may come and still leave a way round
/// it: a cell inside the border, with a cell of clearance either side of
/// a body, as `Accrete` grows. The twin of `rl-mapgen`'s own, tied to the
/// margin growth keeps; `rl-bevy`'s test of the two `may_cut`s fails when
/// one moves without the other.
const REACH: i32 = 2;

impl BarrierDef {
    /// Whether some shift its jitter allows lays it from one edge of a `w`
    /// by `h` map to the opposite one. The same arithmetic as
    /// `rl_mapgen::accrete::Barrier::may_cut`, which refuses it again at
    /// growth for a pass built without a file; `rl-bevy` holds the two
    /// together in a test, since this crate names no mapgen type.
    pub fn may_cut(&self, w: i32, h: i32) -> bool {
        let across = |a: u32, b: u32, n: i32| {
            let (lo, hi) = (cell_at_share(a.min(b), n), cell_at_share(a.max(b), n));
            let jitter = cell_at_share(self.jitter_pct, n);
            (-jitter).max(n - 1 - REACH - hi) <= jitter.min(REACH - lo)
        };
        across(self.from.0, self.to.0, w) || across(self.from.1, self.to.1, h)
    }
}

/// What a built place must meet or be built again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequireDef {
    /// The open share, at least and at most.
    pub open_pct: (u32, u32),
    /// Each an index into [`LayoutDef::shapes`] and the fewest areas it
    /// must have drawn.
    pub areas: Vec<(usize, u32)>,
    /// The fewest loops.
    pub loops: u32,
    /// The largest share of areas that may have one way in.
    pub dead_end_pct: u32,
    /// The fewest steps from the way in to the way out.
    pub route: u32,
}

/// What a place names to be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeDef {
    /// The recipe's name.
    pub name: String,
    /// The map's width and height.
    pub size: (i32, i32),
    /// The share of the map to open.
    pub open_pct: u32,
    /// The share open before the pieces a place must hold are laid, so
    /// they attach out along the frontier rather than to the first room.
    pub late_pct: u32,
    /// Attempts in a row that may fail before growth gives up.
    pub patience: u32,
    /// District seeds per ten thousand cells.
    pub districts_per_10k: u32,
    /// Each an index into [`LayoutDef::districts`] and its weight.
    pub districts: Vec<(usize, u32)>,
    /// The barriers growth goes round; none if the file names none.
    pub barriers: Vec<BarrierDef>,
    /// The detours to cut through.
    pub loops: LoopsDef,
    /// What the built place must meet.
    pub require: RequireDef,
}

/// A loaded layout file, every name resolved to an index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutDef {
    /// Every shape.
    pub shapes: Vec<ShapeDef>,
    /// Every district.
    pub districts: Vec<DistrictDef>,
    /// Every recipe.
    pub recipes: Vec<RecipeDef>,
}

impl LayoutDef {
    /// The recipe called `name`.
    pub fn recipe(&self, name: &str) -> Option<&RecipeDef> {
        self.recipes.iter().find(|r| r.name == name)
    }

    /// Every shape `recipe` may grow: each one some district it scatters
    /// at a weight more than nought draws at a weight more than nought,
    /// once each, in the file's order. A shape no such district draws is
    /// never an area of a place grown by `recipe`, however the file
    /// otherwise names it.
    pub fn shapes_of(&self, recipe: &RecipeDef) -> Vec<&ShapeDef> {
        let drawn = |s: usize| recipe.districts.iter().filter(|(_, w)| *w > 0).any(|(d, _)| self.districts[*d].shapes.iter().any(|(i, w)| *i == s && *w > 0));
        self.shapes.iter().enumerate().filter(|(i, _)| drawn(*i)).map(|(_, s)| s).collect()
    }
}

/// A layout as authored, before its names are resolved.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayoutRon {
    shapes: Vec<ShapeDef>,
    districts: Vec<DistrictRon>,
    recipes: Vec<RecipeRon>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DistrictRon {
    name: String,
    shapes: Vec<(String, u32)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecipeRon {
    name: String,
    size: (i32, i32),
    open_pct: u32,
    // Optional, so a layout written before pieces were grown loads unchanged.
    #[serde(default)]
    late_pct: Option<u32>,
    patience: u32,
    districts_per_10k: u32,
    districts: Vec<(String, u32)>,
    // Optional, so a layout written before barriers loads unchanged.
    #[serde(default)]
    barriers: Vec<BarrierDef>,
    loops: LoopsDef,
    require: RequireRon,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequireRon {
    open_pct: (u32, u32),
    areas: Vec<(String, u32)>,
    loops: u32,
    dead_end_pct: u32,
    route: u32,
}

/// Whether the weights of `rows` add up without overflowing.
fn adds_up(rows: &[(String, u32)]) -> bool {
    rows.iter().try_fold(0u32, |sum, (_, w)| sum.checked_add(*w)).is_some()
}

/// Reads a layout file.
///
/// Reports every problem in the file at once rather than the first, each
/// naming the shape, district or recipe it is about: a name used twice, a
/// name that resolves to nothing, a list in which nothing carries weight,
/// a range that runs backwards or starts below one, a cross whose bars
/// are thicker than they are long, a cave whose rock fills none or all of
/// its box or that is visited never or past reason, a mouth of no width,
/// a share outside a hundred, no patience, weights that overflow, a size
/// or a range or a density past any map, more loops required than may be
/// opened, a requirement for a shape the recipe never draws, a map too
/// small for any shape its recipe draws or for one it requires, a late
/// share past the open share, and a barrier with a share past a hundred
/// or that may reach from one edge of its recipe's map to the opposite.
pub fn load(text: &str) -> Result<LayoutDef, ContentError> {
    let options = ron::options::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME);
    let a: LayoutRon = options.from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    let mut errors = Vec::new();

    let mut twice = |what: &str, names: Vec<&str>| {
        for (i, name) in names.iter().enumerate() {
            if names[..i].contains(name) {
                errors.push(format!("{name}: two {what}s share this name"));
            }
        }
    };
    twice("shape", a.shapes.iter().map(|s| s.name.as_str()).collect());
    twice("district", a.districts.iter().map(|d| d.name.as_str()).collect());
    twice("recipe", a.recipes.iter().map(|r| r.name.as_str()).collect());

    for s in &a.shapes {
        for (field, (fewest, most)) in s.kind.ranges() {
            if fewest < 1 || fewest > most || most > MOST {
                errors.push(format!("{}: {field} runs from {fewest} to {most}, and must start at 1 or more, not run backwards and stop by {MOST}", s.name));
            }
        }
        match s.kind {
            ShapeKind::Cross { long, short } if short.1 >= long.0 => {
                errors.push(format!("{}: a bar as thick as {} is not shorter than one as long as {}", s.name, short.1, long.0));
            }
            ShapeKind::Passage { bend_pct, .. } if bend_pct > 100 => errors.push(format!("{}: bend_pct is {bend_pct}, more than 100", s.name)),
            ShapeKind::Cave { fill_pct, visits_pct, .. } => {
                if !(1..=99).contains(&fill_pct) {
                    errors.push(format!("{}: fill_pct is {fill_pct}, outside 1 to 99, so the cave is all rock or all open", s.name));
                }
                if visits_pct == 0 {
                    errors.push(format!("{}: visits_pct is nought, so the automaton never runs", s.name));
                } else if visits_pct > MOST_VISITS_PCT {
                    errors.push(format!("{}: visits_pct is {visits_pct}, more than {MOST_VISITS_PCT}", s.name));
                }
            }
            _ => {}
        }
        if s.join == JoinDef::Mouth(0) {
            errors.push(format!("{}: Mouth(0) is no way through; a mouth is at least 1 wide", s.name));
        }
    }

    // Resolves a weighted list of names against `known`, reporting each
    // unknown name and a list in which nothing carries weight.
    let resolve = |owner: &str, what: &str, rows: &[(String, u32)], known: &[&str], errors: &mut Vec<String>| -> Vec<(usize, u32)> {
        let mut out = Vec::new();
        for (name, weight) in rows {
            match known.iter().position(|k| k == name) {
                Some(i) => out.push((i, *weight)),
                None => errors.push(format!("{owner}: names unknown {what} {name:?}")),
            }
        }
        out
    };
    let shape_names: Vec<&str> = a.shapes.iter().map(|s| s.name.as_str()).collect();
    let district_names: Vec<&str> = a.districts.iter().map(|d| d.name.as_str()).collect();

    let mut districts = Vec::new();
    for d in &a.districts {
        let shapes = resolve(&d.name, "shape", &d.shapes, &shape_names, &mut errors);
        if !d.shapes.iter().any(|(_, w)| *w > 0) {
            errors.push(format!("{}: no shape carries a weight more than nought", d.name));
        }
        if !adds_up(&d.shapes) {
            errors.push(format!("{}: its weights add up to more than a u32 holds", d.name));
        }
        districts.push(DistrictDef { name: d.name.clone(), shapes });
    }

    let mut recipes = Vec::new();
    for r in &a.recipes {
        let mix = resolve(&r.name, "district", &r.districts, &district_names, &mut errors);
        if !r.districts.iter().any(|(_, w)| *w > 0) {
            errors.push(format!("{}: no district carries a weight more than nought", r.name));
        }
        if !adds_up(&r.districts) {
            errors.push(format!("{}: its weights add up to more than a u32 holds", r.name));
        }
        if !(3..=MOST).contains(&r.size.0) || !(3..=MOST).contains(&r.size.1) {
            errors.push(format!("{}: size is {}x{}, and each side must be from 3 to {MOST}", r.name, r.size.0, r.size.1));
        }
        if r.districts_per_10k > 10_000 {
            errors.push(format!("{}: districts_per_10k is {}, more than one seed a cell", r.name, r.districts_per_10k));
        }
        if r.require.loops > r.loops.max {
            errors.push(format!("{}: requires {} loops, and loops.max opens {} at most", r.name, r.require.loops, r.loops.max));
        }
        let areas = resolve(&r.name, "shape", &r.require.areas, &shape_names, &mut errors);
        if !(1..=100).contains(&r.open_pct) {
            errors.push(format!("{}: open_pct is {}, outside 1 to 100", r.name, r.open_pct));
        }
        let late_pct = r.late_pct.unwrap_or(r.open_pct * 2 / 3);
        if late_pct > r.open_pct {
            errors.push(format!("{}: late_pct is {late_pct}, past the open_pct of {} growth stops at", r.name, r.open_pct));
        }
        if r.patience == 0 {
            errors.push(format!("{}: patience is nought, so growth gives up before it starts", r.name));
        }
        let (least, most) = r.require.open_pct;
        if least > most || most > 100 {
            errors.push(format!("{}: require.open_pct runs from {least} to {most}, and must not run backwards or past 100", r.name));
        }
        if r.require.dead_end_pct > 100 {
            errors.push(format!("{}: require.dead_end_pct is {}, more than 100", r.name, r.require.dead_end_pct));
        }
        for (i, b) in r.barriers.iter().enumerate() {
            let shares = [("from", b.from.0), ("from", b.from.1), ("to", b.to.0), ("to", b.to.1), ("chance_pct", b.chance_pct), ("jitter_pct", b.jitter_pct)];
            for (field, pct) in shares.iter().filter(|(_, pct)| *pct > 100) {
                errors.push(format!("{}: barrier {i}'s {field} has a share of {pct}, more than 100", r.name));
            }
            // A share past a hundred is already wrong, and would lay the
            // barrier off the map besides.
            if shares.iter().all(|(_, pct)| *pct <= 100) && b.may_cut(r.size.0, r.size.1) {
                errors.push(format!(
                    "{}: barrier {i} may reach from one edge of a {}x{} map to the opposite, and growth could never cross it",
                    r.name, r.size.0, r.size.1
                ));
            }
        }
        // Every shape a district of this recipe draws with a weight.
        let drawn: Vec<usize> =
            mix.iter().filter(|(_, w)| *w > 0).flat_map(|(d, _)| districts[*d].shapes.iter().filter(|(_, w)| *w > 0).map(|(s, _)| *s)).collect();
        for (shape, _) in &areas {
            if !drawn.contains(shape) {
                errors.push(format!("{}: requires areas drawn by {:?}, which no district of it draws", r.name, a.shapes[*shape].name));
            }
        }
        // The map less its border.
        let fits = |s: &usize| a.shapes[*s].kind.fits(r.size.0.saturating_sub(2), r.size.1.saturating_sub(2));
        if !drawn.is_empty() && !drawn.iter().any(fits) {
            errors.push(format!("{}: a {}x{} map is too small for any shape it draws", r.name, r.size.0, r.size.1));
        }
        for (shape, _) in areas.iter().filter(|(s, _)| !fits(s)) {
            errors.push(format!("{}: a {}x{} map is too small for the {:?} it requires", r.name, r.size.0, r.size.1, a.shapes[*shape].name));
        }
        recipes.push(RecipeDef {
            name: r.name.clone(),
            size: r.size,
            open_pct: r.open_pct,
            late_pct,
            patience: r.patience,
            districts_per_10k: r.districts_per_10k,
            districts: mix,
            barriers: r.barriers.clone(),
            loops: r.loops,
            require: RequireDef { open_pct: r.require.open_pct, areas, loops: r.require.loops, dead_end_pct: r.require.dead_end_pct, route: r.require.route },
        });
    }

    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Ok(LayoutDef { shapes: a.shapes, districts, recipes })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"(
        shapes: [
            (name: "cell", kind: Rectangle(w: (7, 10), h: (7, 9)), join: Door),
            (name: "big", kind: Rectangle(w: (12, 18), h: (9, 12)), join: Mouth(3)),
            (name: "cross", kind: Cross(long: (11, 15), short: (5, 7)), join: Door),
            (name: "run", kind: Passage(width: (1, 2), length: (5, 12), bend_pct: 30), join: Arch),
            (name: "hollow", kind: Cave(w: (10, 16), h: (8, 12), fill_pct: 55, visits_pct: 100), join: Mouth(2)),
        ],
        districts: [
            (name: "wide", shapes: [("big", 5), ("cell", 2), ("run", 1)]),
            (name: "tight", shapes: [("cell", 5), ("cross", 2), ("run", 2)]),
        ],
        recipes: [
            (
                name: "upper", size: (84, 48), open_pct: 38, patience: 300, districts_per_10k: 7,
                districts: [("wide", 2), ("tight", 1)],
                loops: (min_detour: 20, max: 6),
                require: (open_pct: (35, 60), areas: [("big", 1)], loops: 1, dead_end_pct: 60, route: 30),
            ),
        ],
    )"#;

    fn problems(text: &str) -> Vec<String> {
        match load(text) {
            Err(ContentError::Invalid(e)) => e,
            other => panic!("expected problems, got {other:?}"),
        }
    }

    #[test]
    fn a_layout_file_loads_with_every_name_resolved_to_an_index() {
        let def = load(GOOD).unwrap();
        assert_eq!(def.shapes.len(), 5);
        assert_eq!(def.shapes[4].kind, ShapeKind::Cave { w: (10, 16), h: (8, 12), fill_pct: 55, visits_pct: 100 });
        assert_eq!(def.districts[0].shapes, vec![(1, 5), (0, 2), (3, 1)]);
        let recipe = def.recipe("upper").unwrap();
        assert_eq!(recipe.districts, vec![(0, 2), (1, 1)]);
        assert_eq!(recipe.require.areas, vec![(1, 1)]);
        assert!(def.recipe("lower").is_none());
    }

    /// A recipe grows the shapes its districts draw, and none a district
    /// it scatters at nought, or one drawn at nought, adds: the cave is
    /// named by no district at all, and a district weighted nought drops
    /// the cross it alone drew.
    #[test]
    fn a_recipe_grows_every_shape_its_weighted_districts_draw_and_no_other() {
        let def = load(GOOD).unwrap();
        let names = |def: &LayoutDef| -> Vec<String> { def.shapes_of(def.recipe("upper").unwrap()).iter().map(|s| s.name.clone()).collect() };
        assert_eq!(names(&def), ["cell", "big", "cross", "run"]);
        let mut quiet = def.clone();
        quiet.recipes[0].districts[1].1 = 0;
        assert_eq!(names(&quiet), ["cell", "big", "run"], "the tight district scattered at nought adds nothing");
        let mut unweighted = def;
        unweighted.districts[1].shapes[1].1 = 0;
        assert_eq!(names(&unweighted), ["cell", "big", "run"], "the cross drawn at nought is never grown");
    }

    /// A shape holds a stamp with a cell of floor all round it at its
    /// largest: a 10 by 9 room a 8 by 7 stamp and no larger, a cross either
    /// of its bars, a cave its box at most, and a passage nothing.
    #[test]
    fn a_shape_could_hold_a_stamp_only_with_a_cell_of_its_largest_floor_all_round() {
        let room = ShapeKind::Rectangle { w: (7, 10), h: (7, 9) };
        assert!(room.could_hold(8, 7) && !room.could_hold(9, 7) && !room.could_hold(8, 8));
        let cross = ShapeKind::Cross { long: (11, 15), short: (5, 7) };
        assert!(cross.could_hold(13, 5) && cross.could_hold(5, 13) && !cross.could_hold(13, 6));
        let cave = ShapeKind::Cave { w: (10, 16), h: (8, 12), fill_pct: 55, visits_pct: 100 };
        assert!(cave.could_hold(14, 10) && !cave.could_hold(15, 10));
        let run = ShapeKind::Passage { width: (1, 2), length: (5, 12), bend_pct: 30 };
        assert!(!run.could_hold(1, 1), "a passage is no room");
    }

    #[test]
    fn each_mistake_is_refused_by_name_and_all_of_them_at_once() {
        let swap = |from: &str, to: &str| {
            assert!(GOOD.contains(from), "the good file has no {from:?} to swap");
            GOOD.replacen(from, to, 1)
        };
        for (text, says) in [
            (swap(r#"("big", 5), ("cell", 2), ("run", 1)"#, r#"("vast", 5)"#), "vast"),
            (swap(r#"("wide", 2), ("tight", 1)"#, r#"("narrow", 2)"#), "narrow"),
            (swap(r#"("cell", 5), ("cross", 2), ("run", 2)"#, r#"("cell", 0)"#), "weight"),
            (swap("w: (7, 10)", "w: (10, 7)"), "cell"),
            (swap("open_pct: 38", "open_pct: 140"), "open_pct"),
            (swap("bend_pct: 30", "bend_pct: 300"), "bend_pct"),
            (swap("short: (5, 7)", "short: (15, 17)"), "cross"),
            (swap("size: (84, 48)", "size: (8, 8)"), "upper"),
            (
                swap(r#"areas: [("big", 1)]"#, r#"areas: [("cross", 1)]"#).replacen(
                    r#"districts: [("wide", 2), ("tight", 1)]"#,
                    r#"districts: [("wide", 2)]"#,
                    1,
                ),
                "cross",
            ),
            (swap("open_pct: (35, 60)", "open_pct: (60, 35)"), "open_pct"),
            (swap(r#"name: "big""#, r#"name: "cell""#), "cell"),
            (swap("Mouth(3)", "Mouth(0)"), "Mouth"),
            (swap("patience: 300", "patience: 0"), "patience"),
            (swap(r#"("big", 5), ("cell", 2)"#, r#"("big", 3000000000), ("cell", 3000000000)"#), "weights"),
            (swap("districts_per_10k: 7", "districts_per_10k: 4000000000"), "districts_per_10k"),
            (swap("w: (12, 18)", "w: (12, 2000000000)"), "big"),
            (swap("size: (84, 48)", "size: (84, 2000000000)"), "size"),
            // A rectangle is never turned, so one wider than the map does not fit it.
            (swap("w: (12, 18), h: (9, 12)", "w: (60, 60), h: (5, 5)").replacen("size: (84, 48)", "size: (48, 84)", 1), "big"),
            (swap("loops: 1, dead_end_pct", "loops: 7, dead_end_pct"), "loops"),
            (swap("open_pct: 38,", "open_pct: 38, late_pct: 39,"), "late_pct"),
        ] {
            let found = problems(&text);
            assert!(found.iter().any(|p| p.contains(says)), "wanted a problem naming {says:?}, got {found:?}");
        }
        // Two mistakes, two problems: the file is read to the end.
        let two = swap("w: (7, 10)", "w: (10, 7)").replacen("open_pct: 38", "open_pct: 140", 1);
        assert!(problems(&two).len() >= 2);
    }

    #[test]
    fn a_cave_whose_rock_fills_none_or_all_of_it_never_visited_or_written_backwards_is_refused_by_name() {
        for (from, to, field) in [
            ("fill_pct: 55", "fill_pct: 0", "fill_pct"),
            ("fill_pct: 55", "fill_pct: 100", "fill_pct"),
            ("visits_pct: 100", "visits_pct: 0", "visits_pct"),
            ("visits_pct: 100", "visits_pct: 4000000000", "visits_pct"),
            ("w: (10, 16)", "w: (16, 10)", "w runs"),
            ("h: (8, 12)", "h: (12, 8)", "h runs"),
        ] {
            assert!(GOOD.contains(from), "the good file has no {from:?} to swap");
            let found = problems(&GOOD.replacen(from, to, 1));
            assert!(found.iter().any(|p| p.contains("hollow") && p.contains(field)), "{to}: wanted a problem naming the cave and {field:?}, got {found:?}");
        }
        // A cave box wider than the map less its border does not fit it.
        let wide = GOOD.replacen(r#"areas: [("big", 1)]"#, r#"areas: [("hollow", 1)]"#, 1).replacen(
            r#"("cell", 5), ("cross", 2), ("run", 2)"#,
            r#"("cell", 5), ("cross", 2), ("run", 2), ("hollow", 1)"#,
            1,
        );
        assert!(load(&wide).is_ok(), "the cave fits an 84x48 map and the recipe draws it");
        let too_wide = wide.replacen("w: (10, 16)", "w: (90, 100)", 1);
        assert!(problems(&too_wide).iter().any(|p| p.contains("upper") && p.contains("hollow")));
    }

    #[test]
    fn a_recipe_with_no_barriers_has_none_and_one_with_barriers_keeps_them_in_order() {
        assert!(load(GOOD).unwrap().recipe("upper").unwrap().barriers.is_empty(), "left out, a recipe has none");
        assert_eq!(load(GOOD).unwrap().recipe("upper").unwrap().late_pct, 25, "left out, two thirds of 38");
        let barred = GOOD.replacen(
            "loops: (min_detour",
            "barriers: [(from: (50, 0), to: (50, 70), chance_pct: 100, jitter_pct: 5), (from: (0, 40), to: (30, 0), chance_pct: 0, jitter_pct: 0)],
                loops: (min_detour",
            1,
        );
        let barriers = load(&barred).unwrap().recipe("upper").unwrap().barriers.clone();
        assert_eq!(barriers.len(), 2);
        assert_eq!(barriers[0], BarrierDef { from: (50, 0), to: (50, 70), chance_pct: 100, jitter_pct: 5 });
        assert_eq!(barriers[1].chance_pct, 0, "a corner cut off is allowed, and nought is a chance like any other");
    }

    #[test]
    fn a_barrier_that_may_reach_edge_to_edge_or_names_a_share_past_a_hundred_is_refused_by_name() {
        let with = |barrier: &str| GOOD.replacen("loops: (min_detour", &format!("barriers: [{barrier}], loops: (min_detour"), 1);
        for (barrier, says) in [
            ("(from: (50, 0), to: (50, 100), chance_pct: 50, jitter_pct: 0)", "opposite"),
            ("(from: (0, 30), to: (100, 70), chance_pct: 50, jitter_pct: 0)", "opposite"),
            // Two cells short of each edge of an 84x48 map: no way round either end.
            ("(from: (3, 50), to: (97, 50), chance_pct: 50, jitter_pct: 0)", "opposite"),
            // Short of the bottom as written, but a shift may carry it there.
            ("(from: (50, 0), to: (50, 92), chance_pct: 50, jitter_pct: 10)", "opposite"),
            ("(from: (50, 0), to: (50, 60), chance_pct: 101, jitter_pct: 0)", "'s chance_pct"),
            ("(from: (50, 0), to: (50, 60), chance_pct: 50, jitter_pct: 400)", "'s jitter_pct"),
            ("(from: (150, 0), to: (50, 60), chance_pct: 50, jitter_pct: 0)", "'s from has a share of 150"),
            ("(from: (50, 0), to: (50, 160), chance_pct: 50, jitter_pct: 0)", "'s to has a share of 160"),
        ] {
            let found = problems(&with(barrier));
            assert!(found.iter().any(|p| p.contains("upper") && p.contains("barrier 0") && p.contains(says)), "{barrier}: wanted {says:?}, got {found:?}");
        }
        assert!(load(&with("(from: (50, 0), to: (50, 92), chance_pct: 50, jitter_pct: 0)")).is_ok(), "short of the bottom with no shift");
        let typo = with("(from: (50, 0), to: (50, 60), chance: 50, jitter_pct: 0)");
        assert!(matches!(load(&typo), Err(ContentError::Parse(_))), "an unknown field is refused");
    }

    #[test]
    fn text_that_is_not_a_layout_is_a_parse_error() {
        assert!(matches!(load("(shapes: 3)"), Err(ContentError::Parse(_))));
    }
}
