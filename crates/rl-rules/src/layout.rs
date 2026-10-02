//! A layout: the shapes a place is grown from, the districts that favor
//! some of them, and the recipes a place names.
//!
//! A layout file is [`LayoutDef`], read once by [`load`], which resolves
//! every name in it to an index and reports every mistake at once. A
//! shape is one of the generators the engine ships, with its ranges and
//! how it is joined; a district is a region's weighted taste in shapes; a
//! recipe is a size, how full to grow it, which districts to scatter, how
//! many detours to cut through, and what the built place must meet or be
//! built again.
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
//! //     kind: one of
//! //       Rectangle(w: (fewest, most), h: (fewest, most))
//! //       Cross(long: (fewest, most), short: (fewest, most))
//! //                         two bars overlapped on their centers; the most
//! //                         of short is less than the fewest of long
//! //       Passage(width: (fewest, most), length: (fewest, most), bend_pct: 0)
//! //                         bend_pct is the chance in a hundred of a second
//! //                         leg at a right angle; a passage is not a room,
//! //                         and nothing is stamped in it
//! //     join: how a piece of it is joined to what it is attached to, one of
//! //       Door              one cell of the door tile
//! //       Arch              one cell of floor
//! //       Mouth(n)          floor up to n cells wide, n at least 1
//! //   districts: a region's taste in shapes
//! //     name:   the district's name; an area remembers it
//! //     shapes: [(shape, weight)], at least one weight more than nought
//! //   recipes: what a place names to be built
//! //     name:              the recipe's name
//! //     size:              (width, height) in cells
//! //     open_pct:          the share of the map to open, 1 to 100
//! //     patience:          attempts in a row that may fail before growth
//! //                        gives up, more than nought
//! //     districts_per_10k: district seeds per ten thousand cells, rounded,
//! //                        never fewer than one seed
//! //     districts:         [(district, weight)], at least one weight more
//! //                        than nought
//! //     loops:             (min_detour, max): the shortest detour in steps
//! //                        worth a door, and the most doors
//! //     require:           what the built place must meet or be built again
//! //       open_pct:     (at least, at most)
//! //       areas:        [(shape, fewest)], each a shape some district of
//! //                     this recipe draws
//! //       loops:        fewest
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
//!             loops: (min_detour: 20, max: 4),
//!             require: (open_pct: (30, 60), areas: [("room", 4)], loops: 1, dead_end_pct: 60, route: 20),
//!         ),
//!     ],
//! )
//! ```

use serde::Deserialize;

use crate::content::ContentError;

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
}

impl ShapeKind {
    /// The smallest piece this kind can draw, as its longer and its
    /// shorter side: what a map must have room for.
    fn smallest(self) -> (i32, i32) {
        match self {
            ShapeKind::Rectangle { w, h } => (w.0.max(h.0), w.0.min(h.0)),
            ShapeKind::Cross { long, .. } => (long.0, long.0),
            ShapeKind::Passage { width, length, .. } => (length.0.max(width.0), width.0),
        }
    }

    /// Each range by the name the file gives it.
    fn ranges(self) -> Vec<(&'static str, (i32, i32))> {
        match self {
            ShapeKind::Rectangle { w, h } => vec![("w", w), ("h", h)],
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
    /// Attempts in a row that may fail before growth gives up.
    pub patience: u32,
    /// District seeds per ten thousand cells.
    pub districts_per_10k: u32,
    /// Each an index into [`LayoutDef::districts`] and its weight.
    pub districts: Vec<(usize, u32)>,
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
    patience: u32,
    districts_per_10k: u32,
    districts: Vec<(String, u32)>,
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

/// Reads a layout file.
///
/// Reports every problem in the file at once rather than the first, each
/// naming the shape, district or recipe it is about: a name used twice, a
/// name that resolves to nothing, a list in which nothing carries weight,
/// a range that runs backwards or starts below one, a cross whose bars
/// are thicker than they are long, a mouth of no width, a share outside a
/// hundred, no patience, a requirement for a shape the recipe never
/// draws, and a map too small for any shape its recipe draws or for one
/// it requires.
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
            if fewest < 1 || fewest > most {
                errors.push(format!("{}: {field} runs from {fewest} to {most}, and must start at 1 or more and not run backwards", s.name));
            }
        }
        match s.kind {
            ShapeKind::Cross { long, short } if short.1 >= long.0 => {
                errors.push(format!("{}: a bar as thick as {} is not shorter than one as long as {}", s.name, short.1, long.0));
            }
            ShapeKind::Passage { bend_pct, .. } if bend_pct > 100 => errors.push(format!("{}: bend_pct is {bend_pct}, more than 100", s.name)),
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
        districts.push(DistrictDef { name: d.name.clone(), shapes });
    }

    let mut recipes = Vec::new();
    for r in &a.recipes {
        let mix = resolve(&r.name, "district", &r.districts, &district_names, &mut errors);
        if !r.districts.iter().any(|(_, w)| *w > 0) {
            errors.push(format!("{}: no district carries a weight more than nought", r.name));
        }
        let areas = resolve(&r.name, "shape", &r.require.areas, &shape_names, &mut errors);
        if !(1..=100).contains(&r.open_pct) {
            errors.push(format!("{}: open_pct is {}, outside 1 to 100", r.name, r.open_pct));
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
        // Every shape a district of this recipe draws with a weight.
        let drawn: Vec<usize> =
            mix.iter().filter(|(_, w)| *w > 0).flat_map(|(d, _)| districts[*d].shapes.iter().filter(|(_, w)| *w > 0).map(|(s, _)| *s)).collect();
        for (shape, _) in &areas {
            if !drawn.contains(shape) {
                errors.push(format!("{}: requires areas drawn by {:?}, which no district of it draws", r.name, a.shapes[*shape].name));
            }
        }
        // The map less its border, as its longer and its shorter side.
        let (long, short) = ((r.size.0.max(r.size.1) - 2), (r.size.0.min(r.size.1) - 2));
        let fits = |s: &usize| {
            let (l, sh) = a.shapes[*s].kind.smallest();
            l <= long && sh <= short
        };
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
            patience: r.patience,
            districts_per_10k: r.districts_per_10k,
            districts: mix,
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
        assert_eq!(def.shapes.len(), 4);
        assert_eq!(def.districts[0].shapes, vec![(1, 5), (0, 2), (3, 1)]);
        let recipe = def.recipe("upper").unwrap();
        assert_eq!(recipe.districts, vec![(0, 2), (1, 1)]);
        assert_eq!(recipe.require.areas, vec![(1, 1)]);
        assert!(def.recipe("lower").is_none());
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
        ] {
            let found = problems(&text);
            assert!(found.iter().any(|p| p.contains(says)), "wanted a problem naming {says:?}, got {found:?}");
        }
        // Two mistakes, two problems: the file is read to the end.
        let two = swap("w: (7, 10)", "w: (10, 7)").replacen("open_pct: 38", "open_pct: 140", 1);
        assert!(problems(&two).len() >= 2);
    }

    #[test]
    fn text_that_is_not_a_layout_is_a_parse_error() {
        assert!(matches!(load("(shapes: 3)"), Err(ContentError::Parse(_))));
    }
}
