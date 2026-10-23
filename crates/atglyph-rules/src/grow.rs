//! Growing a place from its files: a layout file as the passes that grow
//! it, a pool's picks as the pieces and the stamps a place is given, and a
//! dressing file as the pass that dresses what they left.
//!
//! This crate owns the files, read straight into the values `atglyph-mapgen`'s
//! passes take, and `atglyph-mapgen` owns the passes and reads no file. This is
//! where a file becomes passes, as [`PrefabLibrary::piece`] is for a
//! prefab, and like it there is no plugin and no system here: a game calls
//! [`passes`], [`picked`] and [`dress`] where it builds a place, and a tool
//! or a terminal game does the same with no Bevy in its build.

use crate::layout::{JoinDef, LayoutDef, ShapeKind};
use crate::pool::{Joined, Laid, Pick, Pool};
use crate::prefab::PrefabLibrary;
use atglyph_grid::TileId;
use atglyph_mapgen::accrete::{Accrete, Cross, District, FirstPiece, Join, Passage, Rectangle, Shape};
use atglyph_mapgen::dungeon::Stairs;
use atglyph_mapgen::prefab::Orient;
use atglyph_mapgen::{BuildError, Cave, DistrictDressings, Dress, Lay, OpenLoops, PoolPick, Require, StampPicks};

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
    /// What a [`Join::Locked`] is laid as: a tile that opens into an open
    /// door. No mind paths through it while its lock holds, since the
    /// opening view reads the place's locks, so it may be a door minds
    /// otherwise use. A game that grows no locked piece may name its door.
    pub locked: TileId,
    /// What a [`Join::Sealed`] is laid as: a gate the game lifts. A game
    /// that grows no sealed piece may name its door.
    pub sealed: TileId,
}

/// The passes for the recipe called `recipe`, laying `tiles`. They grow no
/// piece a place must hold until the game puts one in `accrete.pieces`,
/// as [`PrefabLibrary::grown`] makes them, laid
/// once the recipe's `late_pct` is open, and no landmark until it puts
/// one in `accrete.first`, as [`picked`] makes it. An error naming the
/// recipe when there is none, so a chain can `?` it.
///
/// A place grown from a layout file with nothing but the tier 1 crates in
/// the build, as a tool or a terminal game grows one:
///
/// ```
/// use atglyph_core::RunSeed;
/// use atglyph_grid::{TileProps, TileRegistry};
/// use atglyph_mapgen::dungeon::{FarthestExit, RandomStart};
/// use atglyph_mapgen::{Area, BaseContext, Chain, Survey};
/// use atglyph_rules::grow::{LayoutTiles, passes};
///
/// let layout = atglyph_rules::layout::load(
///     r#"(
///         shapes: [(name: "room", kind: Rectangle(w: (5, 9), h: (5, 8)), join: Door)],
///         districts: [(name: "rooms", shapes: [("room", 1)])],
///         recipes: [(
///             name: "cellar", size: (48, 32), open_pct: 30, patience: 300, districts_per_10k: 7,
///             districts: [("rooms", 1)],
///             loops: (min_detour: 20, max: 4),
///             require: (open_pct: (20, 70), areas: [("room", 3)], loops: 0, dead_end_pct: 100, route: 10),
///         )],
///     )"#,
/// )
/// .unwrap();
/// let mut tiles = TileRegistry::standard();
/// let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
/// let door = tiles.register(TileProps::floor("doorway")).unwrap();
/// let grow = passes(&layout, "cellar", &LayoutTiles { floor, door, locked: door, sealed: door }).unwrap();
/// let (w, h) = grow.size;
/// let chain = Chain::new().then(grow.accrete).then(grow.loops).then(RandomStart).then(FarthestExit).then(Survey).then(grow.require);
/// let place = chain.run_retrying(|| BaseContext::blank(w, h, tiles.clone(), wall), RunSeed(7), 40).unwrap();
/// assert!(place.outputs().iter::<Area>().filter(|a| a.shape == "room").count() >= 3);
/// ```
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
            barriers: r.barriers.clone(),
            locked,
            sealed,
            pieces: Vec::new(),
            late_pct: r.late_pct,
            first: None,
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

/// One place's picks as what builds them.
pub struct Picked {
    /// The pieces grown as areas of their own, for the place's
    /// `accrete.pieces`; growth fails the place when one is not laid,
    /// which is why the pool file grows only a required kind.
    pub pieces: Vec<Box<dyn Shape>>,
    /// The landmark, a seeded pick, for the place's `accrete.first`, and
    /// `None` for a place the pool seeded nothing in, which grows as it
    /// would have.
    pub first: Option<FirstPiece>,
    /// Where the landmark wants the way in and the way down, for the
    /// game's own start and exit to be passed through: [`Stairs::Free`]
    /// with no landmark.
    pub stairs: Stairs,
    /// The pass that places the rest, a piece stamped inside an area or a
    /// dressing given one, for the chain after its `Survey` and any
    /// `HideOpenings`, and before the pass that dresses.
    pub stamp: StampPicks,
}

/// The picks `picks` of `pool`, as [`Pool::plan_run`] drew them for one
/// place, as the pieces growth lays and the pass that places the rest in
/// the order drawn, each piece stamped turned as `orient` and its own file
/// allow, and the landmark seeded first with the stairs it wants, in any
/// facing its file allows, since it is grown as a piece is and nothing
/// stamps it: every piece cut
/// from `prefabs` and keyed, so its slots fill as any stamp's do. An error
/// naming the piece when there is none, a grown or seeded one has no
/// entry, or two seeded picks were drawn for one place, which the pool
/// file refuses, so a chain can `?` it.
pub fn picked<M>(pool: &Pool, picks: &[Pick], prefabs: &PrefabLibrary<M>, orient: Orient) -> Result<Picked, BuildError> {
    let mut pieces: Vec<Box<dyn Shape>> = Vec::new();
    let mut first: Option<FirstPiece> = None;
    let mut stairs = Stairs::Free;
    let mut placed = Vec::new();
    for pick in picks {
        let entry = &pool.entries[pick.entry];
        let (lay, shapes) = match &entry.placed {
            Laid::Grown { piece, join } => {
                let join = match *join {
                    Joined::Door => Join::Door,
                    Joined::Arch => Join::Arch,
                    Joined::Mouth(n) => Join::Mouth(n),
                    Joined::Locked(tag) => Join::Locked(tag.raw()),
                    Joined::Sealed => Join::Sealed,
                };
                pieces.push(Box::new(prefabs.grown(piece, join)?));
                continue;
            }
            Laid::Seeded { piece, placements, stairs: wanted } => {
                if first.is_some() {
                    return Err(BuildError::new("layout", format!("{}: a second landmark for one place, which grows round one", entry.name)));
                }
                // Grown, so no stamp stands over it to narrow its facings:
                // its drawing's declaration alone decides, as for a grown
                // piece, and `orient` is the ceiling of what is stamped.
                first = Some(FirstPiece::new(piece.clone(), &prefabs.piece(piece)?, placements.clone(), Orient::TurnedOrMirrored)?);
                stairs = *wanted;
                continue;
            }
            Laid::Inside { piece, shapes } => (Lay::Stamp(prefabs.piece(piece)?), shapes.clone()),
            Laid::Dressed { dressing } => (Lay::Dress(dressing.clone()), Vec::new()),
        };
        placed.push(PoolPick { name: entry.name.clone(), lay, shapes, prefer: entry.prefer, required: pick.required });
    }
    Ok(Picked { pieces, first, stairs, stamp: StampPicks { name: "pool picks", picks: placed, orient } })
}

/// The pass that dresses a place: every dressing in `prefabs`, cut from
/// it by [`PrefabLibrary::dressing`] so its
/// slots fill as a stamp's, laid where a pick's
/// [`DressAt`](atglyph_mapgen::DressAt) says and in every room nothing took by
/// its district's list in `lists`, repainting only `floor`. For the chain
/// after the pool's [`StampPicks`] and anything else that stamps. An error
/// naming a dressing a list names and `prefabs` does not hold, which
/// [`check_districts`](crate::dressing::check_districts) refuses at
/// load.
pub fn dress<M>(prefabs: &PrefabLibrary<M>, floor: TileId, lists: &[DistrictDressings]) -> Result<Dress, BuildError> {
    let dressings = prefabs.dressings().iter().map(|(_, d)| prefabs.dressing(&d.name)).collect::<Result<Vec<_>, _>>()?;
    // The pass fails on a name no dressing has, which `check_districts`
    // refuses at load.
    Ok(Dress { name: "dress", floor, dressings, districts: lists.to_vec() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_core::RunSeed;
    use atglyph_grid::{TileProps, TileRegistry};
    use atglyph_mapgen::dungeon::{FarthestExit, RandomStart};
    use atglyph_mapgen::{BaseContext, Chain, LaidBarrier, Survey, Surveyed};

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
        let def = crate::layout::load(FILE).unwrap();
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
            assert!(ctx.outputs().iter::<atglyph_mapgen::Area>().any(|a| a.shape == "big"), "seed {s}: the file's names reach the areas");
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
        let def = crate::layout::load(&file).unwrap();
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
            for a in ctx.outputs().iter::<atglyph_mapgen::Area>().filter(|a| a.shape == "hollow") {
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
        let def = crate::layout::load(&file).unwrap();
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

    #[test]
    fn a_recipe_no_file_names_is_an_error_naming_it() {
        let def = crate::layout::load(FILE).unwrap();
        let one = atglyph_grid::TileId(1);
        let e = passes(&def, "missing", &LayoutTiles { floor: one, door: one, locked: one, sealed: one }).err().expect("no such recipe");
        assert!(e.pass == "layout" && e.reason.contains("missing"));
    }

    const POOL: &str = r#"(
        weights: [("common", 50)],
        tags: [],
        budgets: [
            (floors: (1, 3), kind: "reward", count: (1, 1), required: true),
            (floors: (1, 3), kind: "flavor", count: (1, 2)),
        ],
        entries: [
            (name: "closet", kind: "reward", piece: "closet", floors: (1, 3), weight: "common", per_run: 3, placed: Grown(Door)),
            (name: "pile", kind: "flavor", piece: "pile", floors: (1, 3), weight: "common", per_place: 2, per_run: 6, placed: Inside(["cell", "big"]), prefer: Secluded),
            (name: "rubble", kind: "flavor", dressing: "rubble", floors: (1, 3), weight: "common", per_run: 3, placed: Dressed, prefer: DeadEnd),
        ],
    )"#;

    /// A run planned from a pool and each of its places built from the
    /// picks: every grown pick is an area of its own, every pick stamped
    /// inside lands in an area of its shapes, and every dressed pick is
    /// given a dead end, as it prefers, or dropped, since it is flavor.
    #[test]
    fn a_places_picks_are_grown_stamped_and_dressed_as_the_pool_drew_them() {
        use crate::content::Registry;
        use crate::names::Names;
        use crate::pool::Place;
        use atglyph_mapgen::prefab::Stamped;
        use rand::SeedableRng;

        let def = crate::layout::load(FILE).unwrap();
        let mut tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let door = tiles.register(TileProps::floor("way through")).unwrap();
        let piece = |text: &str| crate::prefab::load::<()>(text, &tiles, &Names::new()).unwrap();
        let defs = Registry::from_defs(vec![
            piece(r######"(name: "closet", rows: ["#####", "#...#", "#...#", "##E##"], legend: { '#': Tile("wall"), '.': Tile("floor"), 'E': Entry })"######),
            piece(r#"(name: "pile", rows: ["...", "...", "..."], legend: { '.': Tile("floor") })"#),
        ])
        .unwrap();
        let prefabs = PrefabLibrary::<()>::new(defs, Registry::from_defs(Vec::new()).unwrap());
        let pile = prefabs.defs().id("pile").unwrap().raw();
        let pool = crate::pool::load(POOL, &def, prefabs.defs(), &Names::new()).unwrap();
        let places: Vec<Place> = (1..=3).map(|band| Place { band, tags: Vec::new() }).collect();
        let mut dressings = 0;
        for seed in 0..10 {
            let plan = pool.plan_run(&mut rand::rngs::StdRng::seed_from_u64(seed), &places);
            for (p, picks) in plan.iter().enumerate() {
                let picked = picked(&pool, picks, &prefabs, Orient::Turned).unwrap();
                let named = |name: &str| picks.iter().filter(|k| pool.entries[k.entry].name == name).count();
                assert_eq!(picked.pieces.len(), named("closet"));
                assert_eq!(picked.stamp.picks.len(), named("pile") + named("rubble"), "the rest are the pass's");
                let mut grow = passes(&def, "test", &LayoutTiles { floor, door, locked: door, sealed: door }).unwrap();
                grow.accrete.pieces = picked.pieces;
                let (w, h) = grow.size;
                let chain =
                    Chain::new().then(grow.accrete).then(grow.loops).then(RandomStart).then(FarthestExit).then(Survey).then(picked.stamp).then(grow.require);
                let ctx = chain
                    .run_retrying(|| BaseContext::blank(w, h, tiles.clone(), wall), RunSeed(seed * 10 + p as u64), 40)
                    .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
                let areas: Vec<&atglyph_mapgen::Area> = ctx.outputs().iter::<atglyph_mapgen::Area>().collect();
                assert_eq!(areas.iter().filter(|a| a.shape == "closet").count(), named("closet"), "seed {seed} place {p}: every grown pick is an area");
                let piles: Vec<&Stamped> = ctx.outputs().iter::<Stamped>().filter(|s| s.prefab == Some(pile)).collect();
                assert_eq!(piles.len(), named("pile"), "seed {seed} place {p}: every inside pick is stamped");
                for s in piles {
                    let home = areas.iter().find(|a| a.inner.intersection(&s.bounds) == Some(s.bounds)).expect("inside one area");
                    assert!(home.shape == "cell" || home.shape == "big", "seed {seed}: a pile in a {}", home.shape);
                }
                let at: Vec<&atglyph_mapgen::DressAt> = ctx.outputs().iter::<atglyph_mapgen::DressAt>().collect();
                assert!(at.len() <= named("rubble"), "seed {seed} place {p}: no more dressings placed than drawn");
                let surveyed = ctx.outputs().first::<Surveyed>().unwrap();
                for d in &at {
                    assert!(d.dressing == "rubble" && surveyed.areas[d.area].dead_end, "seed {seed} place {p}: {d:?} is not in a dead end");
                }
                dressings += at.len();
            }
        }
        assert!(dressings > 0, "some dressing was placed, or this proves nothing");
    }

    /// Two landmarks on floors two and three, one to a run each, and none
    /// on floors one and four.
    const LANDMARKS: &str = r#"(
        weights: [("common", 50)],
        tags: [],
        budgets: [(floors: (1, 4), kind: "landmark", count: (1, 1), required: true)],
        entries: [
            (name: "hall", kind: "landmark", piece: "hall", floors: (2, 3), weight: "common", placed: Seeded([Center, Edge], Beyond)),
            (name: "stair", kind: "landmark", piece: "stair", floors: (2, 3), weight: "common", placed: Seeded([Anywhere], Within('>'))),
        ],
    )"#;

    /// A run planned from a pool of landmarks and each place built from
    /// its picks: a place seeded with one grows round it, its first area,
    /// with its stairs where the entry says, and its file's facings
    /// reach the piece; a place no landmark fits is given none.
    #[test]
    fn a_landmark_a_pool_seeds_is_the_piece_a_place_grows_round_with_its_stairs_where_the_entry_says() {
        use crate::content::Registry;
        use crate::names::Names;
        use crate::pool::Place;
        use atglyph_mapgen::dungeon::{ExitPoint, FarExit, StartPoint};
        use atglyph_mapgen::prefab::{Rotation, Stamped};
        use atglyph_mapgen::{Area, Landmark};
        use rand::SeedableRng;

        let def = crate::layout::load(FILE).unwrap();
        let mut tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let door = tiles.register(TileProps::floor("way through")).unwrap();
        let piece = |text: &str| crate::prefab::load::<()>(text, &tiles, &Names::new()).unwrap();
        let defs = Registry::from_defs(vec![
            piece(
                r##########"(name: "hall", turn: None, mirror: LeftRight, rows: ["####E####", "#.......#", "E.......E", "#########"], legend: { '#': Tile("wall"), '.': Tile("floor"), 'E': Entry })"##########,
            ),
            piece(
                r######"(name: "stair", ground: "floor", rows: ["##E##", "#...#", "E.>.E", "#...#", "##E##"], legend: { '#': Tile("wall"), '.': Tile("floor"), '>': Mark, 'E': Entry })"######,
            ),
        ])
        .unwrap();
        let prefabs = PrefabLibrary::<()>::new(defs, Registry::from_defs(Vec::new()).unwrap());
        assert_eq!(prefabs.piece("hall").unwrap().turn(), Rotation::None, "the file's facings reach the piece");
        let pool = crate::pool::load(LANDMARKS, &def, prefabs.defs(), &Names::new()).unwrap();
        let places: Vec<Place> = (1..=4).map(|band| Place { band, tags: Vec::new() }).collect();
        for seed in 0..10 {
            let plan = pool.plan_run(&mut rand::rngs::StdRng::seed_from_u64(seed), &places);
            for (p, picks) in plan.iter().enumerate() {
                let picked = picked(&pool, picks, &prefabs, Orient::TurnedOrMirrored).unwrap();
                let seeded = picks.first().map(|k| pool.entries[k.entry].name.clone());
                assert_eq!(seeded.is_some(), p == 1 || p == 2, "seed {seed} place {p}: a landmark only where one fits");
                assert_eq!(picked.first.as_ref().map(|f| f.name().to_string()), seeded, "seed {seed} place {p}");
                let mut grow = passes(&def, "test", &LayoutTiles { floor, door, locked: door, sealed: door }).unwrap();
                grow.accrete.first = picked.first;
                let (w, h) = grow.size;
                let chain = Chain::new()
                    .then(grow.accrete)
                    .then(grow.loops)
                    .then(picked.stairs.start(RandomStart.clear_of_stamps(0)))
                    .then(picked.stairs.exit(FarExit::default().among_farthest(30)))
                    .then(Survey)
                    .then(grow.require);
                let ctx = chain
                    .run_retrying(|| BaseContext::blank(w, h, tiles.clone(), wall), RunSeed(seed * 10 + p as u64), 40)
                    .unwrap_or_else(|e| panic!("seed {seed} place {p}: {e}"));
                let landmark = ctx.outputs().first::<Landmark>().cloned();
                let Some(name) = seeded else {
                    assert!(landmark.is_none(), "seed {seed} place {p}: no landmark was drawn and one was laid");
                    continue;
                };
                let landmark = landmark.expect("a seeded place publishes its landmark");
                assert_eq!(ctx.outputs().first::<Area>().unwrap().shape, name, "seed {seed} place {p}: the landmark is the first area");
                let (start, exit) = (ctx.outputs().first::<StartPoint>().unwrap().0, ctx.outputs().first::<ExitPoint>().unwrap().0);
                if name == "hall" {
                    assert!(landmark.side(start).is_some_and(|s| landmark.side(exit) == Some(s.other())), "seed {seed} place {p}: beyond");
                } else {
                    let stamp = ctx.outputs().iter::<Stamped>().find(|st| st.bounds == landmark.bounds).unwrap();
                    assert_eq!(Some(exit), stamp.marks.iter().find(|(m, _)| *m == '>').map(|(_, at)| *at), "seed {seed} place {p}: within");
                }
            }
        }
    }

    /// A landmark is grown and nothing stamps it, so the ceiling a game
    /// passes for what it stamps does not narrow it: picked under a fixed
    /// ceiling, a piece whose file allows every facing still backs onto
    /// more than the south edge over a span of seeds.
    #[test]
    fn a_landmarks_facings_are_its_files_alone_whatever_ceiling_the_stamps_are_given() {
        use crate::content::Registry;
        use crate::names::Names;
        use crate::pool::Place;
        use atglyph_mapgen::Landmark;
        use rand::SeedableRng;

        let def = crate::layout::load(FILE).unwrap();
        let mut tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let door = tiles.register(TileProps::floor("way through")).unwrap();
        let hall = r##########"(name: "hall", rows: ["####E####", "#.......#", "E.......E", "#########"], legend: { '#': Tile("wall"), '.': Tile("floor"), 'E': Entry })"##########;
        let defs = Registry::from_defs(vec![crate::prefab::load::<()>(hall, &tiles, &Names::new()).unwrap()]).unwrap();
        let prefabs = PrefabLibrary::<()>::new(defs, Registry::from_defs(Vec::new()).unwrap());
        let text = LANDMARKS.replacen(
            "(2, 3), weight: \"common\", placed: Seeded([Center, Edge], Beyond)",
            "(1, 4), weight: \"common\", placed: Seeded([Edge], Free)",
            1,
        );
        let text = text.replacen("\n            (name: \"stair\", kind: \"landmark\", piece: \"stair\", floors: (2, 3), weight: \"common\", placed: Seeded([Anywhere], Within('>'))),", "", 1);
        let pool = crate::pool::load(&text, &def, prefabs.defs(), &Names::new()).unwrap();
        let place = [Place { band: 1, tags: Vec::new() }];
        let mut backs = std::collections::BTreeSet::new();
        for seed in 0..20 {
            let plan = pool.plan_run(&mut rand::rngs::StdRng::seed_from_u64(seed), &place);
            let picked = picked(&pool, &plan[0], &prefabs, Orient::Fixed).unwrap();
            let mut grow = passes(&def, "test", &LayoutTiles { floor, door, locked: door, sealed: door }).unwrap();
            grow.accrete.first = Some(picked.first.expect("the one landmark is drawn"));
            let (w, h) = grow.size;
            let ctx = Chain::new()
                .then(grow.accrete)
                .run_retrying(|| BaseContext::blank(w, h, tiles.clone(), wall), RunSeed(seed), 40)
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            let b = ctx.outputs().first::<Landmark>().unwrap().bounds;
            backs.insert(
                [(b.bottom() == h - 1, "south"), (b.y == 1, "north"), (b.x == 1, "west"), (b.right() == w - 1, "east")]
                    .into_iter()
                    .find(|(on, _)| *on)
                    .map(|(_, edge)| edge),
            );
        }
        assert!(backs.len() > 1, "only {backs:?} under a fixed ceiling");
    }

    /// Rubble a pool hands out, and crates a district's list lays.
    const DRESSINGS: &str = r#"[
        (name: "rubble", legend: { 'r': Tile("floor") }, place: [(glyph: 'r', at: Scatter, density_pct: 10)]),
        (name: "crates", legend: { 'c': Mark }, place: [(glyph: 'c', at: Walls, density_pct: 40)]),
    ]"#;

    /// A place built from a run's picks and then dressed: every area a
    /// dressed pick was given is laid with that dressing whatever its
    /// district's list says, and the rooms nothing took are dressed from
    /// the list, `tight`'s always and `wide`'s never, each slot keyed so it
    /// finds its meaning again.
    #[test]
    fn a_place_is_dressed_where_its_picks_say_and_by_its_districts_elsewhere() {
        use crate::content::Registry;
        use crate::names::Names;
        use crate::pool::Place;
        use atglyph_mapgen::{Area, DressAt, Dressed};
        use rand::SeedableRng;

        let def = crate::layout::load(FILE).unwrap();
        let mut tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let door = tiles.register(TileProps::floor("way through")).unwrap();
        let piece = |text: &str| crate::prefab::load::<()>(text, &tiles, &Names::new()).unwrap();
        let defs = Registry::from_defs(vec![
            piece(r######"(name: "closet", rows: ["#####", "#...#", "#...#", "##E##"], legend: { '#': Tile("wall"), '.': Tile("floor"), 'E': Entry })"######),
            piece(r#"(name: "pile", rows: ["...", "...", "..."], legend: { '.': Tile("floor") })"#),
        ])
        .unwrap();
        let dressings = crate::dressing::load::<()>(DRESSINGS, &tiles, &Names::new()).unwrap();
        let prefabs = PrefabLibrary::<()>::with_dressings(defs, Registry::from_defs(Vec::new()).unwrap(), dressings);
        let pool = crate::pool::load(POOL, &def, prefabs.defs(), &Names::new()).unwrap();
        crate::dressing::check_pool(&pool, prefabs.dressings()).unwrap();
        let lists: Vec<DistrictDressings> =
            ron::from_str(r#"[(district: "tight", dressings: [("crates", 1)]), (district: "wide", nothing: 1, dressings: [])]"#).unwrap();
        crate::dressing::check_districts(&lists, prefabs.dressings(), &def).unwrap();
        let places: Vec<Place> = (1..=3).map(|band| Place { band, tags: Vec::new() }).collect();
        let (mut given, mut listed) = (0, 0);
        for seed in 0..10 {
            let plan = pool.plan_run(&mut rand::rngs::StdRng::seed_from_u64(seed), &places);
            for (p, picks) in plan.iter().enumerate() {
                let picked = picked(&pool, picks, &prefabs, Orient::Turned).unwrap();
                let mut grow = passes(&def, "test", &LayoutTiles { floor, door, locked: door, sealed: door }).unwrap();
                grow.accrete.pieces = picked.pieces;
                let (w, h) = grow.size;
                let chain = Chain::new()
                    .then(grow.accrete)
                    .then(grow.loops)
                    .then(RandomStart)
                    .then(FarthestExit)
                    .then(Survey)
                    .then(picked.stamp)
                    .then(dress(&prefabs, floor, &lists).unwrap())
                    .then(grow.require);
                let ctx = chain
                    .run_retrying(|| BaseContext::blank(w, h, tiles.clone(), wall), RunSeed(seed * 10 + p as u64), 40)
                    .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
                let at: Vec<&DressAt> = ctx.outputs().iter::<DressAt>().collect();
                let dressed: Vec<&Dressed> = ctx.outputs().iter::<Dressed>().collect();
                for d in &at {
                    assert!(dressed.iter().any(|x| x.area == d.area && x.dressing == "rubble"), "seed {seed} place {p}: {d:?} was laid");
                }
                let district = |i: usize| ctx.outputs().iter::<Area>().find(|a| a.index == i).unwrap().district.clone();
                for d in dressed.iter().filter(|d| !at.iter().any(|a| a.area == d.area)) {
                    assert!(d.dressing == "crates" && district(d.area) == "tight", "seed {seed} place {p}: {d:?} in {}", district(d.area));
                    listed += 1;
                }
                given += at.len();
                for s in ctx.outputs().iter::<atglyph_mapgen::prefab::Stamped>().filter(|s| s.marks.iter().any(|(c, _)| *c == 'c')) {
                    assert!(s.prefab.is_some_and(|key| prefabs.slot(key, 'c').is_some()), "seed {seed}: a crate finds its slot");
                }
            }
        }
        assert!(given > 0 && listed > 0, "given {given} and listed {listed}, or this proves nothing");
    }
}
