//! Biomes, as data: `assets/biomes.ron`'s own schema, the draw that picks
//! one of a floor's band, and [`BiomeDraw`], the plain handful every system
//! that reads a floor's look keeps instead of the whole of [`Undermere`].
//!
//! Twelve biomes stand two or three to a band, floors one to nine, drawn
//! by weight from a stream of the floor alone, so a run's biomes are a
//! pure function of its seed and any floor may be grown first. The three
//! throne biomes stand on floor ten, one to a boss, never drawn by
//! weight: [`throne`] reads off [`FinalBoss::biome`](super::lairs::FinalBoss)
//! instead, so the two draws can never disagree.
//!
//! This task draws the biome and reads its name, its ambient and its
//! favored kinds; [`floor`](super::Undermere::floor) still grows every
//! floor with today's six chains, chosen by [`Family`](super::Family), so
//! `recipes`, `lakes`, `decor` and `dressings` are carried and checked
//! against what this task can check, a tile's name among them, and left
//! for task 10 to grow a floor by.

use super::*;

/// One named recipe of `assets/layout.ron` a biome may grow a floor by,
/// checked against the layout at load; the floors are grown by it once
/// the floor chain reads it.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct RecipeWeight {
    pub(crate) name: String,
    pub(crate) weight: u32,
}

/// `rl_mapgen::lakes::Lakes`'s own fields, `tile` a name rather than a
/// [`TileId`] until this is resolved. Not yet run: task 10 wires the pass
/// in.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct LakesDef {
    pub(crate) name: String,
    pub(crate) tile: String,
    pub(crate) count: (u32, u32),
    pub(crate) size: (u32, u32),
    pub(crate) wreath: Option<(String, u32)>,
    #[allow(dead_code)] // Read by task 10, once the `Lakes` pass is wired in.
    pub(crate) cut: bool,
}

/// One biome, as `assets/biomes.ron` writes it: the file's own doc comment
/// lists every field.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct BiomeDef {
    pub(crate) name: String,
    pub(crate) floors: (u32, u32),
    pub(crate) weight: u32,
    pub(crate) wall: String,
    pub(crate) floor: String,
    pub(crate) ambient: (u8, (u8, u8, u8)),
    pub(crate) recipes: Vec<RecipeWeight>,
    pub(crate) lakes: Option<LakesDef>,
    #[allow(dead_code)] // Read by task 10, once a pool file exists to check it against.
    pub(crate) decor: Vec<String>,
    #[allow(dead_code)] // Read by task 10, once a dressing file exists to check it against.
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
}

fn to_draw(def: &BiomeDef, tiles: &TileRegistry) -> BiomeDraw {
    let (intensity, (r, g, b)) = def.ambient;
    BiomeDraw {
        name: format!("the {}", def.name),
        wall: tiles.expect(&def.wall),
        floor: tiles.expect(&def.floor),
        ambient: Light::new(intensity, Rgb::new(r, g, b)),
        favored: def.favored.clone(),
    }
}

/// Every biome, compiled in so the binary runs from anywhere.
const BIOMES_RON: &str = include_str!("../../assets/biomes.ron");

/// Checks `def` against `tiles` and `layout`: its wall, its floor, a
/// lake's tile and wreath, if it carries one, and every recipe it names,
/// which must be in `assets/layout.ron`. Its floor band, its weight, and
/// its recipes' own weights are checked against nothing but themselves.
fn check(def: &BiomeDef, tiles: &TileRegistry, layout: &LayoutDef) -> Result<(), String> {
    if def.floors.0 > def.floors.1 || def.floors.0 < 1 || def.floors.1 > FLOORS {
        return Err(format!("floors {:?} outside 1..={FLOORS}", def.floors));
    }
    if def.weight == 0 {
        return Err("a weight of nought is never drawn".to_string());
    }
    for name in [&def.wall, &def.floor] {
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
    }
    if let Some(lakes) = &def.lakes {
        if tiles.id(&lakes.tile).is_none() {
            return Err(format!("lake tile {:?} is not a registered tile", lakes.tile));
        }
        if let Some((wreath, _)) = &lakes.wreath
            && tiles.id(wreath).is_none()
        {
            return Err(format!("lake wreath {wreath:?} is not a registered tile"));
        }
        if lakes.count.0 > lakes.count.1 || lakes.size.0 > lakes.size.1 {
            return Err(format!("lake {:?} has its fewest past its most", lakes.name));
        }
    }
    Ok(())
}

/// Loads and checks every biome against `tiles` and `layout`, and that
/// every recipe of `layout` is named by some biome, so none is written for
/// nothing. Panics with every problem at once, since a broken biome is a
/// run that cannot grow a floor.
pub(crate) fn load(tiles: &TileRegistry, layout: &LayoutDef) -> Registry<BiomeDef> {
    let defs: Registry<BiomeDef> = Registry::from_ron_str(BIOMES_RON).unwrap_or_else(|e| panic!("assets/biomes.ron: {e}"));
    defs.validate(|def, _| check(def, tiles, layout)).unwrap_or_else(|e| panic!("assets/biomes.ron: {e}"));
    let unnamed: Vec<&str> =
        layout.recipes.iter().map(|r| r.name.as_str()).filter(|r| !defs.iter().any(|(_, d)| d.recipes.iter().any(|w| w.name == *r))).collect();
    assert!(unnamed.is_empty(), "assets/layout.ron: no biome grows {unnamed:?}");
    defs
}

/// Draws `floor`'s biome from its band, weighted, from the floor's own
/// stream: pure over `seed`, so a run's biomes are a pure function of its
/// seed and any floor may be grown first.
pub(crate) fn draw(defs: &Registry<BiomeDef>, floor: u32, seed: RunSeed, tiles: &TileRegistry) -> BiomeDraw {
    let mut rng = seed.rng(SeedDomain::new(b"undermere.biome"), u64::from(floor));
    let candidates: Vec<&BiomeDef> = defs.iter().map(|(_, d)| d).filter(|d| (d.floors.0..=d.floors.1).contains(&floor)).collect();
    let total: u32 = candidates.iter().map(|d| d.weight).sum();
    assert!(total > 0, "no biome stands on floor {floor}");
    let mut roll = rng.random_range(0..total);
    let def = candidates
        .into_iter()
        .find(|d| {
            if d.weight > roll {
                true
            } else {
                roll -= d.weight;
                false
            }
        })
        .expect("a positive roll under the total always finds a row");
    to_draw(def, tiles)
}

/// Floor ten's biome: whichever of the three throne biomes stands with
/// `boss`, named by [`FinalBoss::biome`](super::lairs::FinalBoss). Never a
/// weighted draw of its own, so it can never disagree with the boss
/// [`Undermere::final_boss`](super::Undermere::final_boss) draws.
pub(crate) fn throne(defs: &Registry<BiomeDef>, boss_biome: &str, tiles: &TileRegistry) -> BiomeDraw {
    let id = defs.expect(boss_biome);
    to_draw(defs.get(id), tiles)
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
            let drawn = throne(&defs, name, &tiles);
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
}
