//! A layout file as the passes that grow it.
//!
//! `rl-rules` owns the file and names no mapgen type; `rl-mapgen` owns
//! the passes and reads no file. This is the one place that sees both, as
//! [`Prefabs::piece`](crate::prefabs::Prefabs::piece) is for a prefab, and
//! like it there is no plugin and no system here: a game calls [`passes`]
//! where it builds a place.

use rl_grid::TileId;
use rl_mapgen::accrete::{Accrete, Barrier, Cross, District, Join, Passage, Rectangle, Shape};
use rl_mapgen::{BuildError, Cave, OpenLoops, Require};
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

/// The tiles a recipe's passes lay, which only the game can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutTiles {
    /// What a piece is carved from, and an arch or a mouth opens as.
    pub floor: TileId,
    /// What a door, and a loop cut through, opens as.
    pub door: TileId,
    /// What a [`Join::Locked`] is laid as: a tile that opens and that no
    /// mind paths through, since only a carrier of the lock's tag gets it
    /// open. A game that grows no locked piece may name its door.
    pub locked: TileId,
    /// What a [`Join::Sealed`] is laid as: a gate the game lifts. A game
    /// that grows no sealed piece may name its door.
    pub sealed: TileId,
}

/// The passes for the recipe called `recipe`, laying `tiles`. They grow no
/// piece a place must hold until the game puts one in `accrete.pieces`,
/// as [`Prefabs::grown`](crate::prefabs::Prefabs::grown) makes them, laid
/// once the recipe's `late_pct` is open. An error naming the recipe when
/// there is none, so a chain can `?` it.
pub fn passes(def: &LayoutDef, recipe: &str, tiles: &LayoutTiles) -> Result<LayoutPasses, BuildError> {
    let LayoutTiles { floor, door, locked, sealed } = *tiles;
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
                ShapeKind::Cave { w, h, fill_pct, visits_pct } => Box::new(Cave { name, w, h, fill_pct, visits_pct, join }),
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
            barriers: r.barriers.iter().map(|b| Barrier { from: b.from, to: b.to, chance_pct: b.chance_pct, jitter_pct: b.jitter_pct }).collect(),
            locked,
            sealed,
            pieces: Vec::new(),
            late_pct: r.late_pct,
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
    use rl_mapgen::{BaseContext, BuildContext, Chain, LaidBarrier, Survey, Surveyed};
    use rl_rules::layout::BarrierDef;

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
            let p = passes(&def, "test", &LayoutTiles { floor, door, locked: door, sealed: door }).unwrap();
            let (w, h) = p.size;
            let chain = Chain::new().then(p.accrete).then(p.loops).then(RandomStart).then(FarthestExit).then(Survey).then(p.require);
            let ctx = chain.run_retrying(|| BaseContext::blank(w, h, tiles.clone(), wall), RunSeed(s), 40).unwrap_or_else(|e| panic!("seed {s}: {e}"));
            let surveyed = ctx.outputs().first::<Surveyed>().unwrap();
            assert!(surveyed.loops >= 1 && surveyed.route.is_some_and(|r| r >= 20), "seed {s}: {surveyed:?}");
            assert!(ctx.outputs().iter::<rl_mapgen::Area>().any(|a| a.shape == "big"), "seed {s}: the file's names reach the areas");
        }
    }

    #[test]
    fn a_cave_named_in_a_file_is_grown_as_a_cave_and_surveyed_like_any_area() {
        let file = FILE
            .replacen(
                r#"(name: "run","#,
                r#"(name: "hollow", kind: Cave(w: (10, 18), h: (8, 12), fill_pct: 55, visits_pct: 100), join: Mouth(2)),
            (name: "run","#,
                1,
            )
            .replacen(r#"("cell", 5), ("run", 2)"#, r#"("hollow", 5), ("cell", 2), ("run", 2)"#, 1)
            .replacen(r#"areas: [("big", 1)]"#, r#"areas: [("big", 1), ("hollow", 1)]"#, 1);
        let def = rl_rules::layout::load(&file).unwrap();
        let mut tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let door = tiles.register(TileProps::floor("way through")).unwrap();
        let (mut caves, mut shaped) = (0, 0);
        for s in 0..30 {
            let p = passes(&def, "test", &LayoutTiles { floor, door, locked: door, sealed: door }).unwrap();
            let (w, h) = p.size;
            let chain = Chain::new().then(p.accrete).then(p.loops).then(RandomStart).then(FarthestExit).then(Survey).then(p.require);
            let ctx = chain.run_retrying(|| BaseContext::blank(w, h, tiles.clone(), wall), RunSeed(s), 40).unwrap_or_else(|e| panic!("seed {s}: {e}"));
            let surveyed = ctx.outputs().first::<Surveyed>().unwrap();
            for a in ctx.outputs().iter::<rl_mapgen::Area>().filter(|a| a.shape == "hollow") {
                assert_ne!(surveyed.areas[a.index].seclusion, u32::MAX, "seed {s}: cave {} is rated", a.index);
                caves += 1;
                shaped += usize::from(a.inner.area() < a.cells.len() as i32);
            }
        }
        // A small cave can come out square, so the shape shows in the many.
        assert!(shaped * 10 > caves * 9, "{shaped} of {caves} caves are more than a rectangle");
    }

    #[test]
    fn a_barrier_named_in_a_file_is_laid_at_its_chance_and_growth_goes_round_it() {
        let file = FILE.replacen(
            "loops: (min_detour",
            "barriers: [(from: (50, 0), to: (50, 65), chance_pct: 100, jitter_pct: 5), (from: (20, 100), to: (20, 55), chance_pct: 0, jitter_pct: 0)],
                loops: (min_detour",
            1,
        );
        let def = rl_rules::layout::load(&file).unwrap();
        let mut tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let door = tiles.register(TileProps::floor("way through")).unwrap();
        for s in 0..20 {
            let p = passes(&def, "test", &LayoutTiles { floor, door, locked: door, sealed: door }).unwrap();
            let (w, h) = p.size;
            let chain = Chain::new().then(p.accrete).then(p.loops).then(RandomStart).then(FarthestExit).then(Survey).then(p.require);
            let ctx = chain.run_retrying(|| BaseContext::blank(w, h, tiles.clone(), wall), RunSeed(s), 40).unwrap_or_else(|e| panic!("seed {s}: {e}"));
            let laid: Vec<&LaidBarrier> = ctx.outputs().iter::<LaidBarrier>().collect();
            assert_eq!(laid.iter().map(|b| b.index).collect::<Vec<_>>(), vec![0], "seed {s}: the sure one and not the other");
            let walkable = ctx.tiles().tables().walkable;
            assert!(laid[0].cells.iter().all(|c| !walkable[ctx.terrain().get(*c).unwrap().index()]), "seed {s}: no loop or door opens a barrier");
        }
    }

    /// `rl-rules` refuses a barrier at load and `rl-mapgen` again at
    /// growth, each by its own copy of the arithmetic, since neither crate
    /// names the other; this is where the two are held to one answer.
    #[test]
    fn the_file_and_the_pass_agree_on_which_barriers_may_reach_edge_to_edge() {
        let shares = [0, 2, 5, 8, 50, 92, 95, 98, 100];
        let mut refused = 0;
        for (w, h) in [(84, 48), (30, 60), (12, 9)] {
            for &(a, b) in &[(0, 100), (2, 98), (5, 95), (8, 92), (0, 50), (50, 100), (3, 97)] {
                for &across in &shares {
                    for jitter in [0, 3, 10, 40] {
                        let file = BarrierDef { from: (a, across), to: (b, across), chance_pct: 100, jitter_pct: jitter };
                        let pass = Barrier { from: file.from, to: file.to, chance_pct: 100, jitter_pct: jitter };
                        assert_eq!(file.may_cut(w, h), pass.may_cut(w, h), "{w}x{h} {file:?}");
                        let (file, pass) = (BarrierDef { from: (across, a), to: (across, b), ..file }, Barrier { from: (across, a), to: (across, b), ..pass });
                        assert_eq!(file.may_cut(w, h), pass.may_cut(w, h), "{w}x{h} {file:?}");
                        refused += usize::from(pass.may_cut(w, h));
                    }
                }
            }
        }
        assert!(refused > 0 && refused < 3 * 7 * 9 * 4, "some cut and some do not: {refused}");
    }

    #[test]
    fn a_recipe_no_file_names_is_an_error_naming_it() {
        let def = rl_rules::layout::load(FILE).unwrap();
        let one = rl_grid::TileId(1);
        let e = passes(&def, "missing", &LayoutTiles { floor: one, door: one, locked: one, sealed: one }).err().expect("no such recipe");
        assert!(e.pass == "layout" && e.reason.contains("missing"));
    }
}
