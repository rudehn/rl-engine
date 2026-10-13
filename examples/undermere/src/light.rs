//! Light in Mereholt and under it: an ambient for every place, and how far
//! the adventurer's lantern reaches as the dark grows with the danger.
//!
//! The dark is the design's, and Brogue's: every place is seen dimly by
//! its own ambient, so a floor's shape reads beyond the lantern, and the
//! lantern, a brazier, a clump of glowcaps or a pool of lava is what makes
//! a place bright, warm and plain to read.
//!
//! What a place is lit by is its content's, and the engine's to light:
//! every tile that glows says so beside its look in `assets/tiles.ron`,
//! and every prop that sheds light says so in `assets/props.ron`, so the
//! engine hangs a fixture on each glowing cell of every place built or
//! restored and a light on each lit prop however it was stood, a piece's
//! slot, a camp's fire, a gate mark. A brazier before the throne burns a
//! cold blue-green because the throne stands a cold brazier, a kind of its
//! own. What is left here is the lantern, the one light that is the run's
//! rather than a place's.
//!
//! Every light is written at the one brightness,
//! [`LIGHT_INTENSITY`](crate::content::LIGHT_INTENSITY): what tells a lamp
//! from a glowcap is how far it reaches and its colour.

use bevy::prelude::*;
use rl_engine::prelude::*;

use std::collections::BTreeMap;

use crate::armory::{Armory, ItemKind};
use crate::dungeon::{FLOORS, Undermere, floor_of};
use crate::run::Adventurer;

/// How far the adventurer's lantern reaches on `floor`, `None` for the
/// town: seven tiles at the top, and a tile less every third floor, down
/// to four before the throne, so the dark closes in as the floors grow
/// more dangerous.
pub fn lantern_radius(floor: Option<u32>) -> i32 {
    let dimmed = floor.map_or(0, |floor| (floor.saturating_sub(1) / 3).min(3));
    7 - dimmed as i32
}

/// This run's biome for every floor, kept in a shape every system can
/// hold: the engine boxes `Undermere` behind `PlaceRules` the moment a run
/// begins, so nothing after that can ask it directly, and a system that
/// reads this every pass must not rebuild it to ask either. Built once, by [`BiomeDraws::new`], off `Undermere::biome_of`,
/// which is pure over the seed, so this is too.
#[derive(Resource, Clone, Debug)]
pub struct BiomeDraws(BTreeMap<u32, crate::dungeon::BiomeDraw>);

impl BiomeDraws {
    /// Every floor's draw, read off `undermere` once.
    pub fn new(undermere: &Undermere) -> Self {
        Self((1..=FLOORS).map(|f| (f, undermere.biome_of(f).clone())).collect())
    }

    /// Floor `floor`'s draw: `pub(crate)`, since what it hands back,
    /// [`BiomeDraw`](crate::dungeon::BiomeDraw), is itself `pub(crate)` and
    /// every caller is in this crate.
    pub(crate) fn of(&self, floor: u32) -> &crate::dungeon::BiomeDraw {
        self.0.get(&floor).unwrap_or_else(|| panic!("floor {floor} outside 1..={FLOORS}"))
    }
}

/// The adventurer as the lantern reads them: what they carry, where, the
/// stats a light reaches by, and whether darkness has put it out.
type Carrying = (&'static Inventory, Option<&'static OnMap>, Option<&'static StatBlock>, Option<&'static Afflicted>);

/// Keeps every lantern the adventurer carries reaching as far as
/// [`lantern_radius`] says for where they stand: seven tiles in Mereholt
/// and on the first floors, and a tile less every third floor down; and
/// as many tiles further as the adventurer's light stat says, never less
/// than the one they stand in. While they are `shrouded`, a potion of
/// darkness's, it is out, and lit again the pass the dark lifts.
///
/// Read off where the adventurer is every pass rather than set on an
/// arrival, so a lantern picked up on floor seven, or brought back by a
/// continued run, dims to the floor it is carried on at once, and a ring
/// of light put on brightens it at once. In `TurnSet::React`, so the floor
/// a pass arrives on is lit by the lantern as it reaches there before
/// anybody on it decides by that light. Only the lantern: a torch burns as
/// it burns, and lies about to be found.
pub fn dim_the_lantern(
    adventurer: Query<Carrying, With<Adventurer>>,
    mut lights: Query<(&ItemKind, &mut LightSource)>,
    armory: Res<Armory>,
    registries: Res<Registries>,
) {
    let Some(lantern) = armory.defs.id(crate::run::LANTERN) else { return };
    let light = registries.stats.id(crate::content::LIGHT);
    let shrouded = registries.statuses.id(crate::content::SHROUDED);
    for (bag, on, stats, held) in &adventurer {
        let brighter = match (light, stats) {
            (Some(light), Some(stats)) => stats.0.value(light, &registries.stats),
            _ => 0,
        };
        let radius = (lantern_radius(floor_of(on.map_or(MapId::SURFACE, |m| m.0))) + brighter).max(1);
        let out = shrouded.zip(held).is_some_and(|(status, held)| held.0.has(status));
        let intensity = if out { 0 } else { crate::content::LIGHT_INTENSITY };
        for item in &bag.items {
            if let Ok((kind, mut light)) = lights.get_mut(*item)
                && kind.0 == lantern
                && (light.radius, light.intensity) != (radius, intensity)
            {
                light.radius = radius;
                light.intensity = intensity;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::prefab::Slot;
    use rl_engine::rl_rules::prop::PropId;

    use super::*;
    use crate::dungeon::{FLOORS, TOWN, map_of};
    use crate::testing::{par_seeds, seeds};

    #[test]
    fn the_lantern_dims_by_a_tile_every_third_floor() {
        assert_eq!(lantern_radius(None), 7, "in Mereholt");
        let down: Vec<i32> = (1..=FLOORS).map(|floor| lantern_radius(Some(floor))).collect();
        assert_eq!(down, [7, 7, 7, 6, 6, 6, 5, 5, 5, 4]);
    }

    /// Every place is seen by its own ambient, and only dimly, by the
    /// lines the game plays by: above the least a cell needs to be seen,
    /// so a floor's shape reads beyond the lantern and is remembered where
    /// it was seen, and below the bright band, so a body outside every
    /// light is dim to whoever watches, never lit; the Spider Warrens'
    /// alone is truly dark, by the design's own exception. No band ever
    /// reads brighter than the one above it, and each band and the town
    /// have an ambient of their own.
    #[test]
    fn every_biomes_ambient_falls_in_the_dim_band_but_the_spider_warrens_own_truly_dark_and_no_band_reads_brighter_than_the_one_above_it() {
        let (threshold, bright, town_ambient) = {
            let mut app = crate::testing::headless(RunSeed(0));
            crate::testing::settle(&mut app);
            let lighting = app.world().resource::<Lighting>();
            (lighting.threshold, lighting.bright, lighting.ambient)
        };
        let band = |light: Light| LightBand::of(light.intensity, threshold, bright);
        assert_eq!(band(Undermere::new(RunSeed(0)).ambient_of(TOWN)), LightBand::Dim, "Mereholt at dusk");

        // Every one of the nine floors' biomes, over a span of seeds, by
        // the band its own floors fall in: (1, 2), (3, 4), (5, 6), (7, 8)
        // or (9, 9), so a band's biomes never read brighter than the
        // band above it, whichever of them a seed draws.
        let mut by_band: std::collections::BTreeMap<(u32, u32), Vec<Light>> = std::collections::BTreeMap::new();
        for s in 0..60u64 {
            let undermere = Undermere::new(RunSeed(s));
            for floor in 1..FLOORS {
                let drawn = undermere.biome_of(floor);
                let key = (drawn.first, drawn.last);
                let seen = by_band.entry(key).or_default();
                if !seen.contains(&drawn.ambient) {
                    seen.push(drawn.ambient);
                }
            }
        }
        let bands: Vec<((u32, u32), Vec<Light>)> = by_band.into_iter().collect();
        let mut distinct: BTreeSet<(u8, u8, u8, u8)> = BTreeSet::new();
        let mut darkest_above: Option<u8> = None;
        for (key, lights) in &bands {
            for light in lights {
                // The Spider Warrens read true dark; every other biome
                // reads dim, by design.
                let expected = if key == &(3, 4) && *light == Light::DARK { LightBand::Dark } else { LightBand::Dim };
                assert_eq!(band(*light), expected, "{key:?}: {light:?}");
                distinct.insert((light.intensity, light.color.r, light.color.g, light.color.b));
            }
            if let Some(darkest) = darkest_above {
                let brightest = lights.iter().map(|l| l.intensity).max().expect("a band with no biome");
                assert!(brightest <= darkest, "{key:?} reads brighter than the band above it: {lights:?}");
            }
            // The Spider Warrens' own nought is excluded: it reads truly
            // dark by its own rule, not by standing deeper than every
            // other biome, so it sets no bound on the band below it.
            darkest_above = Some(lights.iter().filter(|l| **l != Light::DARK).map(|l| l.intensity).min().unwrap_or(u8::MAX));
        }
        distinct.insert((town_ambient.intensity, town_ambient.color.r, town_ambient.color.g, town_ambient.color.b));
        let total: usize = bands.iter().map(|(_, l)| l.len()).sum::<usize>() + 1;
        assert_eq!(distinct.len(), total, "each biome and the town have their own ambient: {bands:?}");
    }

    /// On every floor, whatever the adventurer has a line to beyond every
    /// light is seen, and seen dimly: the floor's shape reads past the
    /// lantern's pool, and a body standing out there with no light of its
    /// own is in the dim band, which stealth reads as harder to pick out
    /// than the lit one. The Spider Warrens are the one exception: truly
    /// dark, so nothing beyond the lantern there is seen at all. Over two
    /// seeds, every band, by its biomes' own floors, and the throne's floor
    /// have such cells in sight on one of their floors' arrivals.
    #[test]
    fn beyond_every_light_a_floor_is_seen_dimly_but_the_spider_warrens_truly_dark() {
        let mut checked = BTreeSet::new();
        for s in 0..2 {
            let mut app = crate::testing::headless(RunSeed(s));
            let me = crate::testing::adventurer(&mut app);
            crate::testing::sturdy(&mut app, me);
            for floor in 1..=FLOORS {
                crate::testing::arrive(&mut app, map_of(floor));
                let truly_dark = floor_of(map_of(floor)).is_some_and(|f| app.world().resource::<BiomeDraws>().of(f).name == "the Spider Warrens");
                let world = app.world_mut();
                // Every light on the floor: what stands or lies with one, and
                // what anyone carries, the adventurer's lantern among them,
                // which sheds from where its carrier stands.
                let mut lights: Vec<(Point, i32)> = Vec::new();
                let mut q = world.query::<(&Position, &OnMap, Option<&LightSource>, Option<&Inventory>)>();
                for (p, _, light, bag) in q.iter(world).filter(|(_, on, _, _)| on.0 == map_of(floor)) {
                    lights.extend(light.map(|l| (p.0, l.radius)));
                    let carried = bag.map_or(&[][..], |b| &b.items[..]).iter().filter_map(|item| world.get::<LightSource>(*item));
                    lights.extend(carried.map(|l| (p.0, l.radius)));
                }
                let (sight, lighting) = (world.get::<Viewshed>(me).expect("eyes"), world.resource::<Lighting>());
                let unlit = |p: &Point| lights.iter().all(|(light, radius)| geometry::chebyshev(*p, *light) > *radius);
                let beyond: Vec<Point> = sight.line.iter().map(|p| p + sight.origin).filter(unlit).collect();
                if truly_dark {
                    let seen: Vec<&Point> = beyond.iter().filter(|p| sight.can_see(**p)).collect();
                    assert!(seen.is_empty(), "seed {s}, floor {floor}: the Spider Warrens show something beyond every light: {seen:?}");
                } else {
                    let unseen: Vec<&Point> = beyond.iter().filter(|p| !sight.can_see(**p)).collect();
                    assert!(unseen.is_empty(), "seed {s}, floor {floor}: in sight beyond every light, yet unseen: {unseen:?}");
                    let bright: Vec<(Point, LightBand)> = beyond.iter().map(|p| (*p, lighting.band(*p))).filter(|(_, band)| *band != LightBand::Dim).collect();
                    assert!(bright.is_empty(), "seed {s}, floor {floor}: beyond every light is dim, not {bright:?}");
                }
                if !beyond.is_empty() {
                    let drawn = app.world().resource::<BiomeDraws>().of(floor);
                    checked.insert((drawn.first, drawn.last));
                }
            }
        }
        assert_eq!(checked.len(), 6, "every band and the throne's floor had something in sight beyond its lights: {checked:?}");
    }

    /// One light standing on a map: where, the prop it is on if it is on
    /// one, and what it sheds.
    type Standing = (Point, Option<PropId>, LightSource);

    /// Every light standing on `map` that nobody is or carries: each
    /// glowing cell's fixture, and each prop's own.
    fn standing(app: &mut App, map: MapId) -> Vec<Standing> {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(&Position, &OnMap, &LightSource, Option<&PropKind>), (Without<Actor>, Without<Item>)>();
        let mut found: Vec<Standing> = q.iter(world).filter(|(_, on, _, _)| on.0 == map).map(|(p, _, l, k)| (p.0, k.map(|k| k.0), *l)).collect();
        found.sort_by_key(|(p, k, _)| (*p, *k));
        found
    }

    /// What `map` should be lit by, read off the definitions: every cell
    /// of a tile that writes a light, and every prop of a kind that does.
    fn planned(app: &mut App, map: MapId) -> Vec<Standing> {
        let registries = app.world().resource::<Registries>().clone();
        let world = app.world_mut();
        let props: Vec<(Point, PropId)> =
            world.query::<(&Position, &OnMap, &PropKind)>().iter(world).filter(|(_, on, _)| on.0 == map).map(|(p, _, k)| (p.0, k.0)).collect();
        let world_map = world.resource::<WorldMap>();
        let lights = &world_map.tables().lights;
        let place = world_map.place(map).expect("the place was built");
        let mut planned: Vec<Standing> = place.terrain.iter().filter_map(|(p, t)| lights[t.index()].map(|glow| (p, None, LightSource(glow)))).collect();
        planned.extend(props.into_iter().filter_map(|(p, kind)| registries.props.get(kind).light.map(|glow| (p, Some(kind), LightSource(glow)))));
        planned.sort_by_key(|(p, k, _)| (*p, *k));
        planned
    }

    /// The tiles and the props that glow are the ones the design lights,
    /// written in the files: the town's lit windows and the gate, the
    /// glowcaps, the lava, the shafts of light and the Underlake's lit
    /// shallows; the lamp posts, the forge and the hearth, the altar and
    /// the candelabras, the hanging lanterns, the witchlights, the
    /// braziers warm and cold, a camp's fire and the gate mark. Nothing
    /// else.
    #[test]
    fn exactly_the_tiles_and_props_the_design_lights_write_a_light() {
        let undermere = Undermere::new(RunSeed(0));
        let glowing: BTreeSet<&str> = undermere.tiles().iter().filter(|(_, t)| t.light.is_some()).map(|(_, t)| t.name.as_str()).collect();
        assert_eq!(glowing, BTreeSet::from(["lit window", "gate", "glowcaps", "lava", "moonlit marble", "glowing shallows"]));
        let registries = crate::content::registries();
        let lit: BTreeSet<&str> = registries.props.iter().filter(|(_, p)| p.light.is_some()).map(|(_, p)| p.name.as_str()).collect();
        let expected = [
            "lamp post",
            "forge",
            "hearth",
            "altar",
            "candelabra",
            "hanging lantern",
            "witchlight",
            "brazier",
            "cold brazier",
            "fire",
            crate::waystone::GATE_MARK,
        ];
        assert_eq!(lit, BTreeSet::from(expected));
        let (warm, cold) = (registries.props.get(registries.props.expect("brazier")), registries.props.get(registries.props.expect("cold brazier")));
        assert_eq!((warm.look, warm.blocks), (cold.look, cold.blocks), "a cold brazier is the same iron, drawn the same");
        assert_ne!(warm.light, cold.light, "burning another flame");
    }

    /// Every place, the town and every floor, is lit on its first arrival
    /// by exactly what its definitions write, and by nothing else that
    /// stands: a fixture on every glowing cell, and every lit prop's own
    /// light on the prop. Before the throne every brazier burns cold, and
    /// nowhere else does one.
    #[test]
    fn every_place_is_lit_on_arrival_by_exactly_what_its_tiles_and_props_write() {
        for s in 0..2 {
            let mut app = crate::testing::headless(RunSeed(s));
            let me = crate::testing::adventurer(&mut app);
            crate::testing::sturdy(&mut app, me);
            let registries = app.world().resource::<Registries>().clone();
            let (brazier, cold) = (registries.props.expect("brazier"), registries.props.expect("cold brazier"));
            for map in std::iter::once(TOWN).chain((1..=FLOORS).map(map_of)) {
                crate::testing::arrive(&mut app, map);
                let planned = planned(&mut app, map);
                assert!(!planned.is_empty() || floor_of(map).is_some(), "seed {s}: Mereholt is lit");
                assert_eq!(standing(&mut app, map), planned, "seed {s}, {map:?}");
                let kinds: Vec<PropId> = planned.iter().filter_map(|(_, k, _)| *k).collect();
                if map == map_of(FLOORS) {
                    assert!(!kinds.contains(&brazier), "seed {s}: a warm brazier before the throne");
                } else {
                    assert!(!kinds.contains(&cold), "seed {s}: a cold brazier on {map:?}");
                }
            }
        }
    }

    /// The Spider Warrens are truly dark, so nothing their own decor lays
    /// sheds light: no glowing tile on any floor drawn as the warrens, and
    /// none of the pieces it furnishes with stands a lit prop. What else
    /// lights a warren is a risk-and-reward piece's own, a camp's fire or
    /// the lantern, each a light somebody put there.
    #[test]
    fn a_floor_drawn_as_the_spider_warrens_lays_nothing_that_glows() {
        let registries = crate::content::registries();
        let mut checked = 0;
        for s in 0..30 {
            let undermere = Undermere::new(RunSeed(s));
            let glows = |tile: TileId| undermere.tiles().get(tile).light.is_some();
            for floor in 1..FLOORS {
                let drawn = undermere.biome_of(floor);
                if drawn.name != "the Spider Warrens" {
                    continue;
                }
                assert_eq!(drawn.ambient, Light::DARK, "the warrens' ambient is nought");
                for decor in &drawn.decor {
                    match decor {
                        crate::dungeon::Decor::Patches { tile, .. } => {
                            assert!(!glows(*tile), "the warrens strew {:?}, which glows", undermere.tiles().get(*tile).name);
                        }
                        crate::dungeon::Decor::Furnish { piece, .. } => {
                            let defs = undermere.prefabs().defs();
                            let lit = defs.get(defs.expect(piece)).slots().any(|(_, slot)| match slot {
                                Slot::Prop(id) => registries.props.get(*id).light.is_some(),
                                _ => false,
                            });
                            assert!(!lit, "the warrens furnish with {piece:?}, which sheds light");
                        }
                    }
                }
                let built = undermere.build(map_of(floor), None).unwrap();
                let glowing: Vec<Point> = built.terrain.iter().filter(|(_, t)| glows(*t)).map(|(p, _)| p).collect();
                assert!(glowing.is_empty(), "seed {s}, floor {floor}: the warrens glow at {glowing:?}");
                checked += 1;
            }
        }
        assert!(checked > 0, "some seed drew the Spider Warrens");
    }

    /// On the first arrival in Mereholt every one of its lights is lit, the
    /// street beside each lamp post is lit by it, and a run continued from
    /// the save that arrival wrote is lit by the same lights again, the
    /// windows' and the gate's as well as the lamp posts', once each.
    #[test]
    fn mereholt_is_lit_by_its_lamps_its_windows_and_the_gate_on_the_first_arrival_and_once_the_run_is_continued() {
        let mut app = crate::testing::headless(RunSeed(2));
        crate::testing::arrive(&mut app, TOWN);
        let placed = standing(&mut app, TOWN);
        assert_eq!(placed, planned(&mut app, TOWN), "every light of the town is lit, once");
        assert!(placed.len() >= 20, "lamps, windows and the gate: {}", placed.len());
        let registries = app.world().resource::<Registries>().clone();
        let lamp_post = registries.props.expect("lamp post");
        let posts: Vec<Point> = placed.iter().filter(|(_, k, _)| *k == Some(lamp_post)).map(|(p, _, _)| *p).collect();
        assert_eq!(posts.len(), 7, "Mereholt's seven lamp posts stand, lit");
        {
            let (map, lighting) = (app.world().resource::<WorldMap>(), app.world().resource::<Lighting>());
            for post in posts {
                let beside: Vec<Point> = geometry::square(post, 1).filter(|p| map.is_walkable(*p)).collect();
                assert!(!beside.is_empty(), "the lamp post at {post:?} stands in the street");
                assert!(beside.iter().all(|p| lighting.band(*p) == LightBand::Lit), "the street beside the lamp post at {post:?} is lit");
            }
        }
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");
        let mut again = crate::testing::continued(&text);
        crate::testing::settle(&mut again);
        assert_eq!(standing(&mut again, TOWN), placed, "continued, the town is lit by the same lights, once each");
    }

    /// A goblin camp's fire, stood by the population with no slot to read
    /// it off, sheds its own light on the floor it is first seen on and
    /// again on the same floor continued from the save its arrival wrote,
    /// found somewhere over a span of seeds since a camp is a coin flip.
    #[test]
    fn a_camps_fire_lights_the_ground_round_it_and_still_does_once_the_run_is_continued_over_a_span_of_seeds() {
        let lit_fires = |app: &mut App, map: MapId| -> Vec<(Point, Option<LightSource>)> {
            let fire = app.world().resource::<Registries>().props.expect("fire");
            let world = app.world_mut();
            let mut q = world.query::<(&Position, &OnMap, &PropKind, Option<&LightSource>)>();
            q.iter(world).filter(|(_, on, kind, _)| on.0 == map && kind.0 == fire).map(|(p, _, _, light)| (p.0, light.copied())).collect()
        };
        let campfire = {
            let registries = crate::content::registries();
            registries.props.get(registries.props.expect("fire")).light.map(LightSource)
        };
        assert!(campfire.is_some(), "a camp's fire is written with a light");
        let camps = std::sync::atomic::AtomicUsize::new(0);
        par_seeds(seeds(0..40), |s| {
            let mut app = crate::testing::headless(RunSeed(s));
            let me = crate::testing::adventurer(&mut app);
            crate::testing::sturdy(&mut app, me);
            let map = map_of(4);
            crate::testing::arrive(&mut app, map);
            crate::testing::settle(&mut app);
            let fires = lit_fires(&mut app, map);
            let Some((at, light)) = fires.first().copied() else { return };
            assert_eq!(light, campfire, "seed {s}: the camp's fire carries its own light");
            let me_at = crate::testing::at(&app, me);
            let (world_map, lighting) = (app.world().resource::<WorldMap>(), app.world().resource::<Lighting>());
            let beyond = geometry::square(at, 1).filter(|p| world_map.is_walkable(*p) && geometry::chebyshev(*p, me_at) > lantern_radius(Some(4)) + 1);
            for p in beyond {
                assert_eq!(lighting.band(p), LightBand::Lit, "seed {s}: the ground at {p:?} beside the fire is lit, the lantern far off");
            }
            let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");
            let mut again = crate::testing::continued(&text);
            assert_eq!(lit_fires(&mut again, map), fires, "seed {s}: every fire is lit again once the run is continued");
            camps.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });
        assert!(camps.into_inner() > 0, "some seed over the span stood a camp's fire on floor four");
    }

    /// The lantern the adventurer carries reaches as far as the floor they
    /// stand on allows, all the way down, and as far as ever back in town.
    #[test]
    fn the_lantern_dims_as_the_adventurer_descends() {
        let mut app = crate::testing::headless(RunSeed(1));
        let me = crate::testing::adventurer(&mut app);
        crate::testing::sturdy(&mut app, me);
        let lantern = {
            let world = app.world();
            let bag = &world.get::<Inventory>(me).expect("a pack").items;
            bag.iter().copied().find(|i| world.get::<Name>(*i).is_some_and(|n| n.as_str() == crate::run::LANTERN)).expect("the lantern")
        };
        let reach = |app: &App| app.world().get::<LightSource>(lantern).map(|l| l.radius);
        assert_eq!(reach(&app), Some(lantern_radius(None)), "in Mereholt");
        for floor in 1..=FLOORS {
            crate::testing::arrive(&mut app, map_of(floor));
            assert_eq!(reach(&app), Some(lantern_radius(Some(floor))), "on floor {floor}");
        }
        crate::testing::arrive(&mut app, TOWN);
        assert_eq!(reach(&app), Some(lantern_radius(None)), "and back in Mereholt");
    }

    #[test]
    fn each_place_is_drawn_in_its_own_ambient() {
        let mut app = crate::testing::headless(RunSeed(3));
        let undermere = Undermere::new(RunSeed(3));
        crate::testing::arrive(&mut app, TOWN);
        assert_eq!(app.world().resource::<Lighting>().ambient, undermere.ambient_of(TOWN));
        crate::testing::arrive(&mut app, map_of(5));
        assert_eq!(app.world().resource::<Lighting>().ambient, undermere.ambient_of(map_of(5)));
    }
}
