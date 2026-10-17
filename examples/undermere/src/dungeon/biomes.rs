//! Biomes, as data: `assets/biomes.ron`'s own schema, the draw that picks
//! one of a floor's band and one of its recipes, and [`BiomeDraw`], the
//! plain handful every system that reads a floor's look or growth keeps
//! instead of the whole of [`Undermere`].
//!
//! Twelve biomes stand one to four to a stratum ([`super::strata`]),
//! floors one to nine, drawn by weight from a stream of the floor alone,
//! so a run's biomes are a pure function of its seed and any floor may be
//! grown first. The three throne biomes stand on floor ten, one to a boss,
//! never drawn by weight: [`throne`] reads off
//! [`FinalBoss::biome`](super::lairs::FinalBoss) instead, so the two draws
//! can never disagree. A floor's recipe is drawn
//! from its biome's list by weight on a stream of its own, so adding a
//! recipe to a biome moves no other floor's biome.
//!
//! A recipe names one in `assets/layout.ron`, checked at load both ways:
//! every recipe a biome names is in the layout, and every recipe in the
//! layout is named by some biome, so none is grown for nothing. `decor` is
//! what a floor of the biome is strewn with once its ways are known, and
//! so most of what lights it: the Fungal Deeps' glowcaps, the Catacombs'
//! candles, and nothing at all in the Spider Warrens, whose dark is the
//! design's. [`district_dressings`] turns `dressings` into the weighted
//! list `rl_mapgen::Dress` gives every area the pool's picks left bare.

use rl_engine::rl_rules::dressing::DistrictDressings;

use super::*;
use crate::bestiary::Bestiary;

/// The weight a biome's own named dressing is given in every district it
/// grows, in [`district_dressings`].
const DRESSING_WEIGHT: u32 = 1;

/// The weight of leaving an area bare in every district, well past any
/// one dressing's own so most rooms the pool did not take stay plain
/// (research 7.1, 7.2: leave floor empty for tactics; one composition of
/// six is empty).
const NOTHING_WEIGHT: u32 = 4;

/// One recipe of `assets/layout.ron` a biome may grow a floor by, at a
/// weight, and the lakes a floor grown by it lays, if any: lakes belong to
/// a recipe, not a biome, since a cathedral's naves stand in water and its
/// cloister does not.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct RecipeWeight {
    pub(crate) name: String,
    pub(crate) weight: u32,
    #[serde(default)]
    pub(crate) lakes: Option<LakesDef>,
}

/// `rl_mapgen::Lakes`'s own fields, `tile` a name rather than a
/// [`TileId`], and the bridges laid over what it lays.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct LakesDef {
    pub(crate) tile: String,
    pub(crate) count: (u32, u32),
    pub(crate) size: (u32, u32),
    pub(crate) wreath: Option<(String, u32)>,
    pub(crate) cut: bool,
    #[serde(default)]
    pub(crate) within: Vec<String>,
    #[serde(default)]
    pub(crate) bridges: Option<BridgesDef>,
}

/// `rl_mapgen::Bridges`'s own fields, `tile` a name; what it crosses is
/// the lake's own tile.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct BridgesDef {
    pub(crate) tile: String,
    pub(crate) longest: u32,
    pub(crate) detour: u32,
}

/// One thing a biome strews on its floors, as `assets/biomes.ron` writes
/// it: the file's own doc comment lists every field.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) enum DecorDef {
    /// Patches of `tile` grown over the biome's floor.
    Patches { tile: String, count: (u32, u32), size: (u32, u32) },
    /// A one-cell piece of `assets/prefabs/` stood on open floor.
    Furnish { piece: String, count: (u32, u32) },
}

/// [`DecorDef`] resolved against the tiles, for the chain: a piece stays a
/// name, since the pieces are [`Undermere`]'s and the chain asks them for
/// it, and [`check_pieces`] has already found every one.
#[derive(Debug, Clone)]
pub(crate) enum Decor {
    /// Patches of `tile` over the floor, `count` of them, each of `size`
    /// cells.
    Patches { tile: TileId, count: (u32, u32), size: (u32, u32) },
    /// `count` of the one-cell piece `piece`.
    Furnish { piece: String, count: (u32, u32) },
}

/// The pass name each of a biome's decor rows runs under, in its order: a
/// pass's name keys its stream, so two rows of one biome never draw the
/// same one, and a name must be `'static`. A biome may strew as many
/// kinds of thing as there are names here, which the load checks.
pub(crate) const DECOR_PASSES: [&str; 4] = ["decor one", "decor two", "decor three", "decor four"];

/// An area's floor painted again by the name of its shape or district.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct PaintDef {
    pub(crate) on: String,
    pub(crate) tile: String,
}

/// One biome, as `assets/biomes.ron` writes it: the file's own doc comment
/// lists every field.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct BiomeDef {
    pub(crate) name: String,
    pub(crate) floors: (u32, u32),
    pub(crate) weight: u32,
    /// What the log says on arriving, one line for each of its floors.
    pub(crate) moods: Vec<String>,
    pub(crate) wall: String,
    pub(crate) floor: String,
    pub(crate) ambient: (u8, (u8, u8, u8)),
    pub(crate) recipes: Vec<RecipeWeight>,
    #[serde(default)]
    pub(crate) paint: Vec<PaintDef>,
    /// What its floors are strewn with, in order, each under the pass
    /// name [`DECOR_PASSES`] gives its place in the list.
    pub(crate) decor: Vec<DecorDef>,
    /// Named dressings of `assets/dressings.ron`, this biome's own rather
    /// than any floor's; [`district_dressings`] weighs each at
    /// [`DRESSING_WEIGHT`] in every district this biome grows.
    pub(crate) dressings: Vec<String>,
    pub(crate) favored: Vec<String>,
}

impl Named for BiomeDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// What every system that reads a floor's look keeps instead of the whole
/// of [`Undermere`]: built once by [`Undermere::new`] and handed out by
/// [`Undermere::biome_of`], and kept a second time, by [`BiomeDraws`], for
/// a system that cannot hold `Undermere` at all because the engine boxes
/// it behind [`PlaceRules`] the moment a run begins.
#[derive(Debug, Clone)]
pub(crate) struct BiomeDraw {
    /// The log's own name, "the" already on it: "the Fungal Deeps".
    pub(crate) name: String,
    /// What the log says of each of its floors on arriving, from
    /// `first`'s on.
    pub(crate) moods: Vec<String>,
    /// The first floor it may be drawn on, which `moods` counts from.
    pub(crate) first: u32,
    /// The last floor it may be drawn on: with `first`, the band it
    /// stands in, for whatever reads floors by their band.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) last: u32,
    /// What the floor is filled with before anything is carved.
    pub(crate) wall: TileId,
    /// What its rooms or caves are carved as.
    pub(crate) floor: TileId,
    /// How the floor reads before anything lights it.
    pub(crate) ambient: Light,
    /// Names from `monsters.ron`, tripled in [`population`](crate::population)'s
    /// own draw; a name the bestiary does not carry is dropped there rather
    /// than refused here, the same tolerance `population::BANDS` already
    /// gives its own names.
    pub(crate) favored: Vec<String>,
    /// The recipe of `assets/layout.ron` the floor is grown by.
    pub(crate) recipe: String,
    /// The lakes the floor's recipe lays, and the bridges over them;
    /// `None` for a dry floor.
    pub(crate) lakes: Option<(Lakes, Option<Bridges>)>,
    /// What an area is floored with in place of `floor`, by the name of
    /// its shape or its district, the first that names either.
    pub(crate) paint: Vec<(String, TileId)>,
    /// What the floor is strewn with once its ways are known, in order.
    pub(crate) decor: Vec<Decor>,
}

impl BiomeDraw {
    /// What the log says of `floor` on arriving: its own line, or the
    /// last there is for a floor past them, which the load refuses.
    pub(crate) fn mood(&self, floor: u32) -> &str {
        let at = floor.saturating_sub(self.first) as usize;
        self.moods.get(at).or(self.moods.last()).map_or("", String::as_str)
    }
}

/// The resolved lakes of `def`, named `lakes` and `bridges` for the chain:
/// one of each to a floor, so the stream a name keys never collides.
fn to_lakes(def: &LakesDef, tiles: &TileRegistry) -> (Lakes, Option<Bridges>) {
    let tile = tiles.expect(&def.tile);
    let lakes = Lakes {
        name: "lakes",
        tile,
        count: def.count,
        size: def.size,
        wreath: def.wreath.as_ref().map(|(t, pct)| (tiles.expect(t), *pct)),
        cut: def.cut,
        within: def.within.clone(),
    };
    let bridges = def.bridges.as_ref().map(|b| Bridges { name: "bridges", across: tile, tile: tiles.expect(&b.tile), longest: b.longest, detour: b.detour });
    (lakes, bridges)
}

/// `def` drawn with its recipe at `recipe`, an index into its list.
pub(super) fn to_draw(def: &BiomeDef, recipe: usize, tiles: &TileRegistry) -> BiomeDraw {
    let (intensity, (r, g, b)) = def.ambient;
    let r_def = &def.recipes[recipe];
    BiomeDraw {
        name: format!("the {}", def.name),
        moods: def.moods.clone(),
        first: def.floors.0,
        last: def.floors.1,
        wall: tiles.expect(&def.wall),
        floor: tiles.expect(&def.floor),
        ambient: Light::new(intensity, Rgb::new(r, g, b)),
        favored: def.favored.clone(),
        recipe: r_def.name.clone(),
        lakes: r_def.lakes.as_ref().map(|l| to_lakes(l, tiles)),
        paint: def.paint.iter().map(|p| (p.on.clone(), tiles.expect(&p.tile))).collect(),
        decor: def
            .decor
            .iter()
            .map(|d| match d {
                DecorDef::Patches { tile, count, size } => Decor::Patches { tile: tiles.expect(tile), count: *count, size: *size },
                DecorDef::Furnish { piece, count } => Decor::Furnish { piece: piece.clone(), count: *count },
            })
            .collect(),
    }
}

/// Every biome, compiled in so the binary runs from anywhere.
const BIOMES_RON: &str = include_str!("../../assets/biomes.ron");

/// Checks one lake's tiles, its ranges, the shapes it starts in against
/// `layout`, and that a lake that may cut is bridged over a tile that
/// stops a body, as `Lakes::cut` asks.
fn check_lakes(lakes: &LakesDef, tiles: &TileRegistry, layout: &LayoutDef) -> Result<(), String> {
    let named = |name: &str| tiles.id(name).ok_or_else(|| format!("{name:?} is not a registered tile"));
    let tile = named(&lakes.tile)?;
    if let Some((wreath, _)) = &lakes.wreath {
        named(wreath)?;
    }
    if lakes.count.0 > lakes.count.1 || lakes.size.0 > lakes.size.1 {
        return Err(format!("lake of {:?} has its fewest past its most", lakes.tile));
    }
    if let Some(bridges) = &lakes.bridges {
        named(&bridges.tile)?;
    }
    if let Some(shape) = lakes.within.iter().find(|w| !layout.shapes.iter().any(|s| s.name == **w)) {
        return Err(format!("lake of {:?} starts within {shape:?}, which is no shape of assets/layout.ron", lakes.tile));
    }
    if lakes.cut && (lakes.bridges.is_none() || tiles.tables().passable[tile.index()]) {
        return Err(format!("lake of {:?} may cut, so it must stop a body and be bridged", lakes.tile));
    }
    Ok(())
}

/// Checks `def`'s decor against `tiles`: no more rows than
/// [`DECOR_PASSES`] names, every range in order, and every patch of a
/// registered tile a body can stand on, since `Patches` never asks whether
/// what it lays cuts the floor in two. Water and lava that stop a body are
/// a recipe's lakes, which the structures lay before the ways are chosen.
fn check_decor(def: &BiomeDef, tiles: &TileRegistry) -> Result<(), String> {
    if def.decor.len() > DECOR_PASSES.len() {
        return Err(format!("{} decor rows, past the {} there are pass names for", def.decor.len(), DECOR_PASSES.len()));
    }
    for d in &def.decor {
        let (what, count) = match d {
            DecorDef::Patches { tile, count, size } => {
                let id = tiles.id(tile).ok_or_else(|| format!("decor {tile:?} is not a registered tile"))?;
                if !tiles.tables().walkable[id.index()] {
                    return Err(format!("decor {tile:?} stops a body; lay it as a recipe's lakes"));
                }
                if size.0 == 0 || size.0 > size.1 {
                    return Err(format!("decor {tile:?} has a size of {size:?}"));
                }
                (tile, count)
            }
            DecorDef::Furnish { piece, count } => (piece, count),
        };
        if count.0 > count.1 {
            return Err(format!("decor {what:?} has its fewest past its most"));
        }
    }
    Ok(())
}

/// Checks every piece a biome of `defs` furnishes with against `prefabs`:
/// it must be there, and of one cell, which is all `Furnish` stands. Apart
/// from [`load`], since the pieces are loaded by
/// [`crate::prefabs::load`] beside the bestiary, and the rumours read the
/// biomes without either.
pub(crate) fn check_pieces(defs: &Registry<BiomeDef>, prefabs: &Prefabs<MonsterDef>) {
    for (_, def) in defs.iter() {
        for d in &def.decor {
            let DecorDef::Furnish { piece, .. } = d else { continue };
            let found = prefabs.piece(piece).unwrap_or_else(|e| panic!("assets/biomes.ron: {}: {e}", def.name));
            assert!((found.width(), found.height()) == (1, 1), "assets/biomes.ron: {}: {piece:?} is not of one cell", def.name);
        }
    }
}

/// Checks `def` against `tiles` and `layout`: its wall, its floor, what it
/// paints, what it strews, and every recipe it names with its lakes. Its floor band and
/// its weights are checked against nothing but themselves.
fn check(def: &BiomeDef, tiles: &TileRegistry, layout: &LayoutDef) -> Result<(), String> {
    if def.floors.0 > def.floors.1 || def.floors.0 < 1 || def.floors.1 > FLOORS {
        return Err(format!("floors {:?} outside 1..={FLOORS}", def.floors));
    }
    if def.moods.len() as u32 != def.floors.1 - def.floors.0 + 1 {
        return Err(format!("{} moods for {} floors: one a floor", def.moods.len(), def.floors.1 - def.floors.0 + 1));
    }
    super::strata::check_within(def.floors)?;
    if def.weight == 0 {
        return Err("a weight of nought is never drawn".to_string());
    }
    for name in [&def.wall, &def.floor].into_iter().chain(def.paint.iter().map(|p| &p.tile)) {
        if tiles.id(name).is_none() {
            return Err(format!("{name:?} is not a registered tile"));
        }
    }
    check_decor(def, tiles)?;
    if def.recipes.is_empty() {
        return Err("no recipe to grow a floor by".to_string());
    }
    for r in &def.recipes {
        if r.weight == 0 {
            return Err(format!("recipe {:?} has a weight of nought", r.name));
        }
        if layout.recipe(&r.name).is_none() {
            return Err(format!("recipe {:?} is not in assets/layout.ron", r.name));
        }
        if let Some(lakes) = &r.lakes {
            check_lakes(lakes, tiles, layout)?;
        }
    }
    Ok(())
}

/// Loads and checks every biome against `tiles` and `layout`, that every
/// recipe of `layout` is named by some biome, that every floor has a biome
/// to draw, and that every throne biome is some floor ten boss's, so none
/// stands in the file never drawn. Panics with every problem at once,
/// since a broken biome is a run that cannot grow a floor.
pub(crate) fn load(tiles: &TileRegistry, layout: &LayoutDef) -> Registry<BiomeDef> {
    super::strata::check();
    let defs: Registry<BiomeDef> = Registry::from_ron_str(BIOMES_RON).unwrap_or_else(|e| panic!("assets/biomes.ron: {e}"));
    defs.validate(|def, _| check(def, tiles, layout)).unwrap_or_else(|e| panic!("assets/biomes.ron: {e}"));
    let unnamed: Vec<&str> =
        layout.recipes.iter().map(|r| r.name.as_str()).filter(|r| !defs.iter().any(|(_, d)| d.recipes.iter().any(|w| w.name == *r))).collect();
    assert!(unnamed.is_empty(), "assets/layout.ron: no biome grows {unnamed:?}");
    let drawn = drawn_problems(&defs);
    assert!(drawn.is_empty(), "assets/biomes.ron: {drawn:#?}");
    defs
}

/// Why some floor or biome of `defs` could never be drawn: a floor one to
/// nine no biome stands on, or a throne biome no floor ten boss names, and
/// so one no run ever grows. A biome or recipe of nought weight is already
/// refused by [`check`], so a biome standing on a floor is drawn there
/// some of the time.
fn drawn_problems(defs: &Registry<BiomeDef>) -> Vec<String> {
    let mut problems: Vec<String> = (1..FLOORS).filter(|f| band(defs, *f).is_empty()).map(|f| format!("no biome stands on floor {f}")).collect();
    for def in band(defs, FLOORS) {
        if !super::lairs::FINAL_BOSSES.iter().any(|b| b.biome == def.name) {
            problems.push(format!("{}: stands on floor ten, and no floor ten boss names it, so it is never drawn", def.name));
        }
    }
    problems
}

/// How many favored kinds a biome of floors one to nine must have able to
/// spawn on every floor of its window: the least that tells a biome by what
/// lives in it, and a floor that favors fewer than this is a floor told only
/// by its walls.
pub(crate) const FAVORED_AT_LEAST: usize = 3;

/// Checks that every name a biome of floors one to nine favors is a kind
/// the bestiary carries and spawns, by the bestiary's spawn table, on every
/// floor of the biome's window, and that at least [`FAVORED_AT_LEAST`] of
/// them do on each. A favored kind with no row on one of its biome's floors
/// is favored for nothing there, and the biome is told by what lives in it
/// on the rest of its floors only. A throne biome favors nothing: its floor
/// is the boss's. Panics with every shortfall at once, since a biome told
/// by nothing is content that moved without its monsters.
pub(crate) fn check_favored(defs: &Registry<BiomeDef>, bestiary: &Bestiary) {
    let mut errors = Vec::new();
    for (_, def) in defs.iter().filter(|(_, d)| d.floors.1 < FLOORS) {
        let mut ids = Vec::new();
        for name in &def.favored {
            match bestiary.defs.id(name) {
                Some(id) => ids.push((name, id)),
                None => errors.push(format!("{}: favors {name:?}, which monsters.ron does not carry", def.name)),
            }
        }
        for floor in def.floors.0..=def.floors.1 {
            let spawns = |id: Id<MonsterDef>| bestiary.spawns.at(floor as i32).any(|e| e.item == id && e.weight > 0);
            for (name, _) in ids.iter().filter(|(_, id)| !spawns(*id)) {
                errors.push(format!("{}: favors {name:?}, which monster_spawns.ron spawns on none of floor {floor}", def.name));
            }
            let able = ids.iter().filter(|(_, id)| spawns(*id)).count();
            if able < FAVORED_AT_LEAST {
                errors.push(format!("{}: {able} favored kinds can spawn on floor {floor}, and a biome needs {FAVORED_AT_LEAST}", def.name));
            }
        }
    }
    assert!(errors.is_empty(), "assets/biomes.ron: {errors:#?}");
}

/// Checks every entry of the run's pool against the biomes it may stand
/// in, from the files alone: an entry of a weight more than nought must
/// have some floor where a budget of its kind gives picks and a biome it
/// may stand in is drawn, or it is never drawn at all; and on each such
/// floor and biome it must fit every recipe the biome grows when its kind
/// is required there, since a floor owing a pick it cannot lay fails every
/// attempt, or some recipe of some biome it may stand in when it is not,
/// since a pick that fits nowhere is only ever dropped. A landmark the
/// throne's floor may draw must leave its stairs free, since that floor
/// has no way down. What fits is
/// [`misfit`](rl_engine::rl_rules::pool::misfit)'s, the same rule
/// `rl_mapgen` lays by. Panics with every problem at once.
pub(crate) fn check_pool(defs: &Registry<BiomeDef>, pool: &Pool, layout: &LayoutDef, pieces: &Registry<rl_engine::rl_rules::prefab::PrefabDef<MonsterDef>>) {
    let errors = pool_problems(defs, pool, layout, pieces);
    assert!(errors.is_empty(), "assets/pool.ron: {errors:#?}");
}

/// [`check_pool`]'s problems, each naming the entry, for a test to read.
fn pool_problems(
    defs: &Registry<BiomeDef>,
    pool: &Pool,
    layout: &LayoutDef,
    pieces: &Registry<rl_engine::rl_rules::prefab::PrefabDef<MonsterDef>>,
) -> Vec<String> {
    use rl_engine::rl_rules::pool::{Placed, Stairs, misfit};
    let mut errors = Vec::new();
    for entry in pool.entries.iter().filter(|e| e.weight > 0) {
        // Every floor and biome it may be drawn for, and whether its kind
        // is required there.
        let mut ways: Vec<(u32, &BiomeDef, bool)> = Vec::new();
        for floor in 1..=FLOORS {
            let Some(budget) = pool.budget(entry.kind, floor as i32).filter(|b| b.count.1 > 0) else { continue };
            for def in band(defs, floor) {
                if entry.fits(floor as i32, std::slice::from_ref(&def.name)) {
                    ways.push((floor, def, budget.required));
                }
            }
        }
        if ways.is_empty() {
            errors.push(format!("{}: no floor where a budget of its kind gives picks stands a biome it may stand in, so it is never drawn", entry.name));
            continue;
        }
        if let Placed::Seeded { stairs, .. } = &entry.placed
            && *stairs != Stairs::Free
            && ways.iter().any(|(floor, ..)| *floor == FLOORS)
        {
            errors.push(format!("{}: may be the throne's landmark, and asks its stairs {stairs:?} on a floor with no way down", entry.name));
        }
        let mut fits_somewhere = false;
        let mut owed: Vec<String> = Vec::new();
        for (_, def, required) in &ways {
            for recipe in def.recipes.iter().filter_map(|r| layout.recipe(&r.name)) {
                match misfit(entry, recipe, layout, pieces) {
                    None => fits_somewhere = true,
                    Some(why) if *required => {
                        let line = format!("{why}, on a floor of the {} that owes it", def.name);
                        if !owed.contains(&line) {
                            owed.push(line);
                        }
                    }
                    Some(_) => {}
                }
            }
        }
        errors.extend(owed);
        if !fits_somewhere {
            errors.push(format!("{}: fits no recipe of any biome it may stand in, so it is never laid", entry.name));
        }
    }
    errors
}

/// Every district of `layout` that some biome of `defs` grows, each
/// weighted with that biome's own `dressings` at [`DRESSING_WEIGHT`] and
/// leaving the area bare at [`NOTHING_WEIGHT`], for the pass that dresses
/// every area the pool's picks left untaken. A district two of a biome's
/// recipes both grow is named once; `Undermere::new` checks the result
/// against the dressing file and the layout once, at load, rather than on
/// the floor that first grows it.
pub(crate) fn district_dressings(defs: &Registry<BiomeDef>, layout: &LayoutDef) -> Vec<DistrictDressings> {
    let mut out = Vec::new();
    for (_, def) in defs.iter() {
        let mut named: Vec<&str> = Vec::new();
        for r in def.recipes.iter().filter_map(|r| layout.recipe(&r.name)) {
            for &(i, _) in &r.districts {
                let name = layout.districts[i].name.as_str();
                if !named.contains(&name) {
                    named.push(name);
                }
            }
        }
        for district in named {
            out.push(DistrictDressings {
                district: district.to_string(),
                nothing: NOTHING_WEIGHT,
                dressings: def.dressings.iter().map(|d| (d.clone(), DRESSING_WEIGHT)).collect(),
            });
        }
    }
    out
}

/// One of `weights` drawn by weight from `rng`, as its index.
fn weighted(rng: &mut impl Rng, weights: impl Iterator<Item = u32> + Clone) -> usize {
    let total: u32 = weights.clone().sum();
    assert!(total > 0, "nothing to draw from");
    let mut roll = rng.random_range(0..total);
    weights
        .enumerate()
        .find(|(_, w)| {
            if *w > roll {
                true
            } else {
                roll -= w;
                false
            }
        })
        .map(|(i, _)| i)
        .expect("a roll under the total always finds a row")
}

/// The recipe `def` grows `floor` by, as an index into its list, drawn by
/// weight from the floor's own recipe stream.
fn draw_recipe(def: &BiomeDef, floor: u32, seed: RunSeed) -> usize {
    let mut rng = seed.rng(SeedDomain::new(b"undermere.recipe"), u64::from(floor));
    weighted(&mut rng, def.recipes.iter().map(|r| r.weight))
}

/// Draws `floor`'s biome from its band, weighted, from the floor's own
/// stream, and its recipe from the floor's recipe stream: pure over
/// `seed`, so a run's biomes are a pure function of its seed and any floor
/// may be grown first.
pub(crate) fn draw(defs: &Registry<BiomeDef>, floor: u32, seed: RunSeed, tiles: &TileRegistry) -> BiomeDraw {
    let def = drawn_def(&band(defs, floor), floor, seed);
    to_draw(def, draw_recipe(def, floor, seed), tiles)
}

/// Every biome that may stand on `floor`, in the file's order.
fn band(defs: &Registry<BiomeDef>, floor: u32) -> Vec<&BiomeDef> {
    defs.iter().map(|(_, d)| d).filter(|d| (d.floors.0..=d.floors.1).contains(&floor)).collect()
}

/// The one of `band` that `floor` draws by weight from its own stream:
/// [`draw`]'s and [`biomes_drawn`]'s both, so the floor and the rumour of
/// it can never disagree.
fn drawn_def<'a>(band: &[&'a BiomeDef], floor: u32, seed: RunSeed) -> &'a BiomeDef {
    assert!(!band.is_empty(), "no biome stands on floor {floor}");
    let mut rng = seed.rng(SeedDomain::new(b"undermere.biome"), u64::from(floor));
    band[weighted(&mut rng, band.iter().map(|d| d.weight))]
}

/// Each biome the run of `seed` draws on a floor that could have drawn
/// another, by the name the log gives it on arrival, shallowest first and
/// each once: what the innkeeper's rumours of the floors are told from,
/// since a floor that could have been nothing else is no news. Reads
/// `assets/biomes.ron` itself and draws each floor by [`drawn_def`], from
/// the seed alone, so a rumour told in Mereholt before a floor is built
/// is of the biome that floor will be grown as. Floor ten's is the
/// boss's, never drawn here, and its rumour is the boss's own.
pub(crate) fn biomes_drawn(seed: RunSeed) -> Vec<String> {
    drawn_names(&load(&registry(), &layout()), seed)
}

/// Every biome [`biomes_drawn`] may ever name, read from the file with no
/// seed drawn: each one standing on a floor below the throne's beside
/// another, by the name the log gives it, in the file's order.
#[cfg(test)]
pub(crate) fn biomes_told_of() -> Vec<String> {
    let defs = load(&registry(), &layout());
    defs.iter()
        .map(|(_, d)| d)
        .filter(|d| (d.floors.0..=d.floors.1.min(FLOORS - 1)).any(|f| band(&defs, f).len() > 1))
        .map(|d| format!("the {}", d.name))
        .collect()
}

/// [`biomes_drawn`] over `defs` already loaded.
fn drawn_names(defs: &Registry<BiomeDef>, seed: RunSeed) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for floor in 1..FLOORS {
        let band = band(defs, floor);
        let name = format!("the {}", drawn_def(&band, floor, seed).name);
        if band.len() > 1 && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Floor ten's biome: whichever of the three throne biomes stands with
/// `boss`, named by [`FinalBoss::biome`](super::lairs::FinalBoss). Never a
/// weighted draw of its own, so it can never disagree with the boss
/// [`Undermere::final_boss`](super::Undermere::final_boss) draws.
pub(crate) fn throne(defs: &Registry<BiomeDef>, boss_biome: &str, seed: RunSeed, tiles: &TileRegistry) -> BiomeDraw {
    let def = defs.get(defs.expect(boss_biome));
    to_draw(def, draw_recipe(def, FLOORS, seed), tiles)
}

/// The biome standing on `floor` that names `recipe`, drawn with it: what
/// `undermere --floor N --seed S --recipe R` prints, so a recipe can be
/// read on any seed without hunting for one that draws it. `None` when no
/// biome of the floor names it.
pub(crate) fn with_recipe(defs: &Registry<BiomeDef>, floor: u32, recipe: &str, tiles: &TileRegistry) -> Option<BiomeDraw> {
    defs.iter().map(|(_, d)| d).filter(|d| (d.floors.0..=d.floors.1).contains(&floor)).find_map(|d| {
        let at = d.recipes.iter().position(|r| r.name == recipe)?;
        Some(to_draw(d, at, tiles))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loads against the real tiles, the way `Undermere::new` does.
    fn defs() -> Registry<BiomeDef> {
        load(&registry(), &layout())
    }

    /// The shipped biomes with `edit` done to each biome's own row, as a
    /// registry the favored check can be asked of.
    fn edited(edit: impl Fn(&mut BiomeDef)) -> Registry<BiomeDef> {
        let mut rows: Vec<BiomeDef> = defs().iter().map(|(_, d)| d.clone()).collect();
        rows.iter_mut().for_each(edit);
        Registry::from_defs(rows).expect("the same names as before")
    }

    fn bestiary() -> Bestiary {
        crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default()).bestiary
    }

    /// The shipped biomes each have three favored kinds able to spawn on
    /// every floor of their window, which `Undermere::new` holds at load.
    #[test]
    fn every_shipped_biome_has_three_favored_kinds_able_to_spawn_on_each_of_its_floors() {
        check_favored(&defs(), &bestiary());
    }

    /// A biome left with two favored kinds is refused, naming it.
    #[test]
    #[should_panic(expected = "Cellars: 2 favored kinds can spawn on floor 1")]
    fn a_biome_with_only_two_favored_kinds_is_refused_naming_it() {
        check_favored(
            &edited(|d| {
                if d.name == "Cellars" {
                    d.favored.truncate(2);
                }
            }),
            &bestiary(),
        );
    }

    /// A favored kind with no spawn row on the floor counts for nothing
    /// there: the myconid, moved down to floors five and six, no longer
    /// serves a biome of floors three and four.
    #[test]
    #[should_panic(expected = "Sunken Library: 2 favored kinds can spawn on floor 3")]
    fn a_favored_kind_that_cannot_spawn_on_a_floor_of_the_window_is_not_counted_there() {
        check_favored(
            &edited(|d| {
                if d.name == "Sunken Library" {
                    d.favored[2] = "myconid".to_string();
                }
            }),
            &bestiary(),
        );
    }

    /// A favored kind that spawns on none of one of its biome's floors is
    /// refused naming the kind and the floor, though the biome still has
    /// its three others there: favored for nothing on that floor.
    #[test]
    #[should_panic(expected = "myconid\\\", which monster_spawns.ron spawns on none of floor 1")]
    fn a_favored_kind_that_spawns_on_none_of_its_biomes_floors_is_refused_naming_the_floor() {
        check_favored(
            &edited(|d| {
                if d.name == "Cellars" {
                    d.favored.push("myconid".to_string());
                }
            }),
            &bestiary(),
        );
    }

    /// A name the bestiary does not carry is refused rather than dropped.
    #[test]
    #[should_panic(expected = "which monsters.ron does not carry")]
    fn a_favored_name_that_is_no_monster_is_refused() {
        check_favored(
            &edited(|d| {
                if d.name == "Cellars" {
                    d.favored.push("unicorn".to_string());
                }
            }),
            &bestiary(),
        );
    }

    /// The three throne biomes are the only ones on floor ten, and
    /// `throne` finds all three by the names `FinalBoss::biome` carries.
    #[test]
    fn the_three_throne_biomes_stand_only_on_floor_ten_and_throne_finds_each_by_name() {
        let defs = defs();
        let tiles = registry();
        for name in ["Drowned Throne", "Brood Hollow", "Wyrm's Hall"] {
            let def = defs.get(defs.expect(name));
            assert_eq!(def.floors, (FLOORS, FLOORS), "{name} stands only on floor ten");
            let drawn = throne(&defs, name, RunSeed(0), &tiles);
            assert_eq!(drawn.name, format!("the {name}"));
        }
    }

    /// The draw is a pure function of the seed and the floor: calling it
    /// twice, or for floors in either order, never disagrees with itself.
    #[test]
    fn the_draw_is_pure_over_the_seed_and_the_floor_and_the_order_floors_are_asked_in() {
        let defs = defs();
        let tiles = registry();
        for s in 0..20 {
            let seed = RunSeed(s);
            let first: Vec<String> = (1..FLOORS).map(|f| draw(&defs, f, seed, &tiles).name).collect();
            let reversed: Vec<String> = (1..FLOORS).rev().map(|f| draw(&defs, f, seed, &tiles).name).collect();
            let forward_again: Vec<String> = (1..FLOORS).map(|f| draw(&defs, f, seed, &tiles).name).collect();
            assert_eq!(first, forward_again, "seed {s}: the same floor asked for twice");
            assert_eq!(first, reversed.into_iter().rev().collect::<Vec<_>>(), "seed {s}: asked in the other order");
        }
    }

    /// The shares a design keeps, read from the weights with no draw: no
    /// biome takes more than two thirds of any floor it shares with
    /// another, and no recipe more than two thirds of its biome's floors,
    /// the throne biomes' single recipes and a stratum's lone biome aside.
    /// That every biome and recipe is drawn at all is the load's: a weight
    /// of nought is refused, and so is a floor with no biome or a throne
    /// biome no boss names.
    #[test]
    fn no_biome_takes_more_than_two_thirds_of_a_floor_it_shares_and_no_recipe_more_than_two_thirds_of_its_biomes() {
        let defs = defs();
        for floor in 1..FLOORS {
            let band = band(&defs, floor);
            let total: u32 = band.iter().map(|d| d.weight).sum();
            for def in band.iter().filter(|_| band.len() > 1) {
                assert!(def.weight * 3 <= total * 2, "floor {floor}: {} weighs {} of {total}", def.name, def.weight);
            }
        }
        for (_, def) in defs.iter().filter(|(_, d)| d.recipes.len() > 1) {
            let total: u32 = def.recipes.iter().map(|r| r.weight).sum();
            for r in &def.recipes {
                assert!(r.weight * 3 <= total * 2, "{}: {} weighs {} of {total}", def.name, r.name, r.weight);
            }
        }
    }

    /// A floor no biome stands on, and a throne biome no floor ten boss
    /// names, are each refused by name: the one a run that cannot draw its
    /// floor, the other a biome no run ever grows.
    #[test]
    fn a_floor_with_no_biome_and_a_throne_biome_no_boss_names_are_each_refused_by_name() {
        assert_eq!(drawn_problems(&defs()), Vec::<String>::new(), "the shipped biomes");
        let moved = edited(|d| {
            if d.floors == (1, 2) {
                d.floors = (2, 2);
                d.moods.truncate(1);
            }
            if d.name == "Brood Hollow" {
                d.name = "Brood Pit".to_string();
            }
        });
        assert_eq!(
            drawn_problems(&moved),
            ["no biome stands on floor 1", "Brood Pit: stands on floor ten, and no floor ten boss names it, so it is never drawn"]
        );
    }

    /// Every shipped pool entry is drawn on some floor of some biome, fits
    /// every recipe of every biome that owes it, and fits somewhere it is
    /// only offered: what `Undermere::new` holds at load.
    #[test]
    fn every_pool_entry_is_drawn_somewhere_and_fits_every_recipe_that_owes_it() {
        let (prefabs, pool, _) = crate::prefabs::load(&registry(), &layout());
        assert_eq!(pool_problems(&defs(), &pool, &layout(), prefabs.defs()), Vec::<String>::new());
    }

    /// An entry no biome it may stand in is drawn with, and a required
    /// piece too large for every room it may stand in, are each refused
    /// naming the entry and why: the one never drawn, the other a floor
    /// that fails every attempt.
    #[test]
    fn an_entry_drawn_nowhere_and_a_required_piece_too_large_for_any_room_are_each_refused_by_name() {
        use rl_engine::rl_rules::pool::Placed;
        let (prefabs, mut pool, _) = crate::prefabs::load(&registry(), &layout());
        let nowhere = pool.entries.iter().position(|e| e.weight > 0).expect("an entry in play");
        pool.entries[nowhere].within = vec!["Drowned Throne".to_string()];
        pool.entries[nowhere].floors = (1, 2);
        let owed = pool
            .entries
            .iter()
            .enumerate()
            .position(|(i, e)| {
                i != nowhere
                    && matches!(e.placed, Placed::Inside { .. })
                    && e.weight > 0
                    && pool.budgets.iter().filter(|b| b.kind == e.kind).all(|b| b.required)
            })
            .expect("a required inside piece");
        if let Placed::Inside { piece, .. } = &mut pool.entries[owed].placed {
            *piece = "Mereholt".to_string();
        }
        let errors = pool_problems(&defs(), &pool, &layout(), prefabs.defs());
        let (a, b) = (pool.entries[nowhere].name.clone(), pool.entries[owed].name.clone());
        assert!(
            errors.iter().any(|e| *e == format!("{a}: no floor where a budget of its kind gives picks stands a biome it may stand in, so it is never drawn")),
            "{errors:#?}"
        );
        assert!(errors.iter().any(|e| e.starts_with(&format!("{b}: \"Mereholt\" is 64 by 36")) && e.contains("that owes it")), "{errors:#?}");
        assert!(errors.iter().any(|e| *e == format!("{b}: fits no recipe of any biome it may stand in, so it is never laid")), "{errors:#?}");
    }

    /// The biomes the innkeeper's rumours are told from are the floors'
    /// own, shallowest first and each once, over a span of seeds: the
    /// rumour and the floor are drawn by the one function, and a floor
    /// that could have drawn nothing else is left out.
    #[test]
    fn the_biomes_a_rumour_is_told_of_are_the_ones_the_floors_draw_over_a_span_of_seeds() {
        let defs = defs();
        let tiles = registry();
        for s in 0..200u64 {
            let mut floors: Vec<String> = Vec::new();
            for floor in (1..FLOORS).filter(|f| band(&defs, *f).len() > 1) {
                let name = draw(&defs, floor, RunSeed(s), &tiles).name;
                if !floors.contains(&name) {
                    floors.push(name);
                }
            }
            assert_eq!(drawn_names(&defs, RunSeed(s)), floors, "seed {s}");
        }
    }
}
