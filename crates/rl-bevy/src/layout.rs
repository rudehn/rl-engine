//! A layout file as the passes that grow it.
//!
//! `rl-rules` owns the file and names no mapgen type; `rl-mapgen` owns
//! the passes and reads no file. This is the one place that sees both, as
//! [`Prefabs::piece`](crate::prefabs::Prefabs::piece) is for a prefab, and
//! like it there is no plugin and no system here: a game calls [`passes`]
//! where it builds a place.

use rl_grid::TileId;
use rl_mapgen::accrete::{Accrete, Cross, District, Join, Passage, Rectangle, Shape};
use rl_mapgen::{BuildError, OpenLoops, Require};
use rl_rules::layout::{JoinDef, LayoutDef, ShapeKind};

/// The passes one recipe asks for, and the size of the map to run them on.
///
/// A game puts them in its own chain: `accrete` and `loops` first, its
/// stamps, its start and exit, a `Survey`, and `require` last.
pub struct LayoutPasses {
    /// The map's width and height.
    pub size: (i32, i32),
    /// Growth.
    pub accrete: Accrete,
    /// The loops cut after it.
    pub loops: OpenLoops,
    /// What the built place must meet.
    pub require: Require,
}

/// The passes for the recipe called `recipe`, carving `floor` and opening
/// doors as `door`. An error naming the recipe when there is none, so a
/// chain can `?` it.
pub fn passes(def: &LayoutDef, recipe: &str, floor: TileId, door: TileId) -> Result<LayoutPasses, BuildError> {
    let r = def.recipe(recipe).ok_or_else(|| BuildError::new("layout", format!("no recipe is called {recipe:?}")))?;
    let join = |j: JoinDef| match j {
        JoinDef::Door => Join::Door,
        JoinDef::Arch => Join::Arch,
        JoinDef::Mouth(n) => Join::Mouth(n),
    };
    let shapes: Vec<Box<dyn Shape>> = def
        .shapes
        .iter()
        .map(|s| -> Box<dyn Shape> {
            let (name, join) = (s.name.clone(), join(s.join));
            match s.kind {
                ShapeKind::Rectangle { w, h } => Box::new(Rectangle { name, w, h, join }),
                ShapeKind::Cross { long, short } => Box::new(Cross { name, long, short, join }),
                ShapeKind::Passage { width, length, bend_pct } => Box::new(Passage { name, width, length, bend_pct, join }),
            }
        })
        .collect();
    let districts = def.districts.iter().map(|d| District { name: d.name.clone(), shapes: d.shapes.clone() }).collect();
    Ok(LayoutPasses {
        size: r.size,
        accrete: Accrete {
            floor,
            door,
            shapes,
            districts,
            mix: r.districts.clone(),
            districts_per_10k: r.districts_per_10k,
            open_pct: r.open_pct,
            patience: r.patience,
        },
        loops: OpenLoops { door, min_detour: r.loops.min_detour, max: r.loops.max },
        require: Require {
            open_pct: r.require.open_pct,
            areas: r.require.areas.iter().map(|(i, n)| (def.shapes[*i].name.clone(), *n)).collect(),
            loops: r.require.loops,
            dead_end_pct: r.require.dead_end_pct,
            route: r.require.route,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_core::RunSeed;
    use rl_grid::{TileProps, TileRegistry};
    use rl_mapgen::dungeon::{FarthestExit, RandomStart};
    use rl_mapgen::{BaseContext, BuildContext, Chain, Survey, Surveyed};

    const FILE: &str = r#"(
        shapes: [
            (name: "cell", kind: Rectangle(w: (5, 9), h: (5, 8)), join: Door),
            (name: "big", kind: Rectangle(w: (11, 18), h: (8, 12)), join: Mouth(3)),
            (name: "run", kind: Passage(width: (1, 2), length: (4, 12), bend_pct: 30), join: Arch),
        ],
        districts: [
            (name: "wide", shapes: [("big", 5), ("cell", 2), ("run", 1)]),
            (name: "tight", shapes: [("cell", 5), ("run", 2)]),
        ],
        recipes: [
            (
                name: "test", size: (84, 48), open_pct: 36, patience: 300, districts_per_10k: 7,
                districts: [("wide", 1), ("tight", 1)],
                loops: (min_detour: 20, max: 6),
                require: (open_pct: (30, 65), areas: [("big", 1)], loops: 1, dead_end_pct: 70, route: 20),
            ),
        ],
    )"#;

    #[test]
    fn a_recipe_from_a_file_builds_a_place_that_meets_what_the_file_requires() {
        let def = rl_rules::layout::load(FILE).unwrap();
        let mut tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let door = tiles.register(TileProps::floor("way through")).unwrap();
        for s in 0..30 {
            let p = passes(&def, "test", floor, door).unwrap();
            let (w, h) = p.size;
            let chain = Chain::new().then(p.accrete).then(p.loops).then(RandomStart).then(FarthestExit).then(Survey).then(p.require);
            let ctx = chain.run_retrying(|| BaseContext::blank(w, h, tiles.clone(), wall), RunSeed(s), 40).unwrap_or_else(|e| panic!("seed {s}: {e}"));
            let surveyed = ctx.outputs().first::<Surveyed>().unwrap();
            assert!(surveyed.loops >= 1 && surveyed.route.is_some_and(|r| r >= 20), "seed {s}: {surveyed:?}");
            assert!(ctx.outputs().iter::<rl_mapgen::Area>().any(|a| a.shape == "big"), "seed {s}: the file's names reach the areas");
        }
    }

    #[test]
    fn a_recipe_no_file_names_is_an_error_naming_it() {
        let def = rl_rules::layout::load(FILE).unwrap();
        let e = passes(&def, "missing", rl_grid::TileId(1), rl_grid::TileId(1)).err().expect("no such recipe");
        assert!(e.pass == "layout" && e.reason.contains("missing"));
    }
}
