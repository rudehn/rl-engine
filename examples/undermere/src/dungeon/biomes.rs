//! Biomes, as data: `assets/biomes.ron`'s own schema, the draw that picks
//! one of a floor's band and one of its recipes, and [`BiomeDraw`], the
//! plain handful every system that reads a floor's look or growth keeps
//! instead of the whole of [`Undermere`].
//!
//! Twelve biomes stand two or three to a band, floors one to nine, drawn
//! by weight from a stream of the floor alone, so a run's biomes are a
//! pure function of its seed and any floor may be grown first. The three
//! throne biomes stand on floor ten, one to a boss, never drawn by
//! weight: [`throne`] reads off [`FinalBoss::biome`](super::lairs::FinalBoss)
//! instead, so the two draws can never disagree. A floor's recipe is drawn
//! from its biome's list by weight on a stream of its own, so adding a
//! recipe to a biome moves no other floor's biome.
//!
//! A recipe names one in `assets/layout.ron`, checked at load both ways:
//! every recipe a biome names is in the layout, and every recipe in the
//! layout is named by some biome, so none is grown for nothing. `decor` is
//! carried and left for a pool-consistency check of a later task;
//! [`district_dressings`] turns `dressings` into the weighted list
//! `rl_mapgen::Dress` gives every area the pool's picks left bare.

use rl_engine::rl_rules::dressing::DistrictDressings;

use super::*;

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
    #[allow(dead_code)] // Read once a pool file exists to check it against.
    pub(crate) decor: Vec<String>,
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
        wall: tiles.expect(&def.wall),
        floor: tiles.expect(&def.floor),
        ambient: Light::new(intensity, Rgb::new(r, g, b)),
        favored: def.favored.clone(),
        recipe: r_def.name.clone(),
        lakes: r_def.lakes.as_ref().map(|l| to_lakes(l, tiles)),
        paint: def.paint.iter().map(|p| (p.on.clone(), tiles.expect(&p.tile))).collect(),
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

/// Checks `def` against `tiles` and `layout`: its wall, its floor, what it
/// paints, and every recipe it names with its lakes. Its floor band and
/// its weights are checked against nothing but themselves.
fn check(def: &BiomeDef, tiles: &TileRegistry, layout: &LayoutDef) -> Result<(), String> {
    if def.floors.0 > def.floors.1 || def.floors.0 < 1 || def.floors.1 > FLOORS {
        return Err(format!("floors {:?} outside 1..={FLOORS}", def.floors));
    }
    if def.moods.len() as u32 != def.floors.1 - def.floors.0 + 1 {
        return Err(format!("{} moods for {} floors: one a floor", def.moods.len(), def.floors.1 - def.floors.0 + 1));
    }
    if def.weight == 0 {
        return Err("a weight of nought is never drawn".to_string());
    }
    for name in [&def.wall, &def.floor].into_iter().chain(def.paint.iter().map(|p| &p.tile)) {
        if tiles.id(name).is_none() {
            return Err(format!("{name:?} is not a registered tile"));
        }
    }
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

/// Loads and checks every biome against `tiles` and `layout`, and that
/// every recipe of `layout` is named by some biome. Panics with every
/// problem at once, since a broken biome is a run that cannot grow a
/// floor.
pub(crate) fn load(tiles: &TileRegistry, layout: &LayoutDef) -> Registry<BiomeDef> {
    let defs: Registry<BiomeDef> = Registry::from_ron_str(BIOMES_RON).unwrap_or_else(|e| panic!("assets/biomes.ron: {e}"));
    defs.validate(|def, _| check(def, tiles, layout)).unwrap_or_else(|e| panic!("assets/biomes.ron: {e}"));
    let unnamed: Vec<&str> =
        layout.recipes.iter().map(|r| r.name.as_str()).filter(|r| !defs.iter().any(|(_, d)| d.recipes.iter().any(|w| w.name == *r))).collect();
    assert!(unnamed.is_empty(), "assets/layout.ron: no biome grows {unnamed:?}");
    defs
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

    /// Every floor one to ten has at least one biome that stands on it,
    /// so [`draw`] and [`throne`] never run out of candidates.
    #[test]
    fn every_floor_has_a_biome_of_its_own() {
        let defs = defs();
        for floor in 1..=FLOORS {
            let total: u32 = defs.iter().map(|(_, d)| d).filter(|d| (d.floors.0..=d.floors.1).contains(&floor)).map(|d| d.weight).sum();
            assert!(total > 0, "floor {floor} has no biome");
        }
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

    /// Over a span of seeds, every biome of a band is drawn somewhere on
    /// it, and none takes more than two thirds of its band's floors.
    #[test]
    fn every_biome_is_drawn_and_none_takes_more_than_two_thirds_of_its_bands_floors() {
        let defs = defs();
        let tiles = registry();
        let bands: [(u32, u32); 5] = [(1, 2), (3, 4), (5, 6), (7, 8), (9, 9)];
        for band in bands {
            let members: Vec<&str> = defs.iter().map(|(_, d)| d).filter(|d| (d.floors.0..=d.floors.1) == (band.0..=band.1)).map(|d| d.name.as_str()).collect();
            let mut counts: std::collections::BTreeMap<&str, u32> = members.iter().map(|m| (*m, 0)).collect();
            let mut total = 0u32;
            for s in 0..200u64 {
                for floor in band.0..=band.1 {
                    let drawn = draw(&defs, floor, RunSeed(s), &tiles);
                    let bare = drawn.name.trim_start_matches("the ").to_string();
                    if let Some(c) = counts.get_mut(bare.as_str()) {
                        *c += 1;
                    }
                    total += 1;
                }
            }
            for (name, count) in &counts {
                assert!(*count > 0, "band {band:?}: {name} is never drawn");
                assert!(u64::from(*count) * 3 <= u64::from(total) * 2, "band {band:?}: {name} takes {count} of {total}");
            }
        }
    }

    /// Over seeds 0 to 200, every recipe of every biome is drawn on some
    /// floor, and none takes more than two thirds of its biome's floors,
    /// the throne biomes' single recipes aside.
    #[test]
    fn every_recipe_of_every_biome_is_drawn_and_none_takes_more_than_two_thirds_of_its_biomes_floors() {
        let defs = defs();
        let tiles = registry();
        let mut counts: std::collections::BTreeMap<(String, String), u32> = std::collections::BTreeMap::new();
        for (_, def) in defs.iter() {
            for r in &def.recipes {
                counts.insert((def.name.clone(), r.name.clone()), 0);
            }
        }
        for s in 0..200u64 {
            for floor in 1..FLOORS {
                let drawn = draw(&defs, floor, RunSeed(s), &tiles);
                let biome = drawn.name.trim_start_matches("the ").to_string();
                *counts.get_mut(&(biome, drawn.recipe.clone())).expect("a recipe of its own biome") += 1;
            }
            let boss = lairs::final_boss(RunSeed(s));
            let drawn = throne(&defs, boss.biome, RunSeed(s), &tiles);
            let biome = drawn.name.trim_start_matches("the ").to_string();
            *counts.get_mut(&(biome, drawn.recipe.clone())).expect("the throne's own recipe") += 1;
        }
        for (_, def) in defs.iter() {
            let of_biome: u32 = def.recipes.iter().map(|r| counts[&(def.name.clone(), r.name.clone())]).sum();
            for r in &def.recipes {
                let n = counts[&(def.name.clone(), r.name.clone())];
                assert!(n > 0, "{}: {} is never drawn", def.name, r.name);
                assert!(def.recipes.len() == 1 || n * 3 <= of_biome * 2, "{}: {} takes {n} of {of_biome}", def.name, r.name);
            }
        }
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
