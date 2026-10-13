//! Light in Mereholt and under it: an ambient for every place, the
//! fixtures each place is lit by, and how far the adventurer's lantern
//! reaches as the dark grows with the danger.
//!
//! The dark is the design's, and Brogue's: every place is seen dimly by
//! its own ambient, so a floor's shape reads beyond the lantern, and the
//! lantern, a brazier, a clump of glowcaps or a pool of lava is what makes
//! a place bright, warm and plain to read. The engine's lighting does the
//! rest: a fixture is a `LightSource` on an entity with no turns, cast
//! once and cast again only when the map changes.
//!
//! What a place is lit by is read off the place itself, by [`Fixtures`]:
//! the tiles that glow, and the props a piece stood at its slots that shed
//! light, which the engine's props do not carry, so the game names them
//! here. Read off the built place rather than kept, so a run continued
//! from a save can light every place it restores with [`light_place`] the
//! way each first arrival did, and the two can never disagree.
//!
//! Every light is a [`LightDef`], as the content files write a light, at
//! the one brightness every light has: what tells a lamp from a glowcap is
//! how far it reaches and its colour.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::Spot;
use rl_engine::rl_rules::prefab::Slot;
use rl_engine::rl_rules::prop::PropId;

use std::collections::BTreeMap;

use crate::armory::{Armory, ItemKind};
use crate::bestiary::MonsterDef;
use crate::content::LightDef;
use crate::dungeon::{FLOORS, TOWN, Undermere, floor_of};
use crate::run::Adventurer;

/// A lamp post's lantern: warm, and far enough to light the street round
/// it.
pub const LAMP: LightDef = LightDef { radius: 5, color: (1.0, 0.8, 0.52), flicker: 0.12 };
/// A lit window: firelight through the glass, a little way into the street
/// and the room behind it.
pub const WINDOW: LightDef = LightDef { radius: 3, color: (1.0, 0.76, 0.42), flicker: 0.1 };
/// A forge or a hearth: orange, and restless.
pub const FIRE: LightDef = LightDef { radius: 4, color: (1.0, 0.5, 0.2), flicker: 0.35 };
/// A camp's fire in the open: wider than a hearth's, so a sleeping band
/// round it is seen from across the dark before it is reached. Yellower
/// than the forge's own orange: light multiplies a tile's colour channel
/// by channel, and an orange light on the Fungal Deeps' green moss, which
/// has almost no red to warm, came out darker than the dark round it.
pub const CAMPFIRE: LightDef = LightDef { radius: 5, color: (1.0, 0.82, 0.5), flicker: 0.4 };
/// Candles, on an altar or a candelabra: pale gold, and steadier than fire.
pub const CANDLES: LightDef = LightDef { radius: 4, color: (1.0, 0.88, 0.58), flicker: 0.2 };
/// A lantern hung in the Cellars or by the goblins in the Old Mines: warm,
/// and nearer in than a brazier, so a cellar is lit in pools.
pub const HUNG_LANTERN: LightDef = LightDef { radius: 4, color: (1.0, 0.72, 0.4), flicker: 0.15 };
/// The Sunken Library's witchlight: a pale violet, steady under its glass.
pub const WITCHLIGHT: LightDef = LightDef { radius: 4, color: (0.78, 0.6, 1.0), flicker: 0.05 };
/// A brazier left burning in the Upper Halls: wide enough to light the room
/// it stands in.
pub const BRAZIER: LightDef = LightDef { radius: 6, color: (1.0, 0.6, 0.28), flicker: 0.4 };
/// A brazier before the throne, burning a cold blue-green.
pub const COLD_BRAZIER: LightDef = LightDef { radius: 6, color: (0.35, 0.95, 0.8), flicker: 0.25 };
/// The Drowned Gate, on both its sides: violet.
pub const GATE: LightDef = LightDef { radius: 5, color: (0.75, 0.42, 1.0), flicker: 0.15 };
/// A gate mark, where a waystone was last used: the gate's violet, nearer
/// in, a glow on the floor rather than a gate's worth of light.
pub const GATE_MARK: LightDef = LightDef { radius: 4, ..GATE };
/// A glowcap: a soft teal, a step or so round it, so a clump of them is a
/// pool of light and a lone one a mark in the dark.
pub const GLOWCAP: LightDef = LightDef { radius: 2, color: (0.4, 1.0, 0.85), flicker: 0.0 };
/// A cell of lava: orange, and every cell of a pool sheds it, so a pool
/// lights its shore.
pub const LAVA: LightDef = LightDef { radius: 3, color: (1.0, 0.52, 0.15), flicker: 0.3 };
/// A shaft of pale light come down through the lake onto the cathedral's
/// marble.
pub const SHAFT: LightDef = LightDef { radius: 2, color: (0.72, 0.84, 1.0), flicker: 0.0 };
/// The Underlake's shallows, lit from below: a cold blue-green, and every
/// cell of a pool sheds it, as lava's does.
pub const SHALLOWS: LightDef = LightDef { radius: 2, color: (0.45, 0.9, 1.0), flicker: 0.1 };

/// Every tile that glows by itself, by its name in `assets/tiles.ron`, and
/// the light it sheds: the town's lit windows and the gate, and below them
/// the glowcaps, the lava, the cathedral's shafts of light and the
/// Underlake's lit shallows. Each is drawn brighter than the floor round
/// it, as a light is.
pub const GLOWING: [(&str, LightDef); 6] =
    [("lit window", WINDOW), ("gate", GATE), ("glowcaps", GLOWCAP), ("lava", LAVA), ("moonlit marble", SHAFT), ("glowing shallows", SHALLOWS)];

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
/// reads this every pass, [`set_ambient`], must not rebuild it to ask
/// either. Built once, by [`BiomeDraws::new`], off `Undermere::biome_of`,
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

/// The light everywhere in `map` before anything in it is lit: in every
/// place, the engine's dim band, above the least a cell needs to be seen
/// and below the bright band.
///
/// Mereholt is at dusk, blue and low; every floor below is its own drawn
/// biome's own ambient, from `assets/biomes.ron`, darker the deeper a
/// band stands, down to the throne's, among the darkest; the Spider
/// Warrens' alone is nought, truly dark, so the lantern is the only
/// light. Seen at all otherwise, so a floor's shape reads beyond the
/// lantern in deep, dark colour and is remembered where it was seen, as
/// Brogue keeps it; and no brighter than dim, so a body out there with no
/// light of its own is harder to pick out than one in a lamp's light, and
/// the lantern's pool stands out warm and bright. Any other map is dark.
pub fn ambient_of(draws: &BiomeDraws, map: MapId) -> Light {
    if map == TOWN {
        return Light::new(30, Rgb::new(120, 140, 215));
    }
    floor_of(map).map_or(Light::DARK, |floor| draws.of(floor).ambient)
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

/// Sets the current place's ambient, between the turns and the light, so
/// the frame the stairs land on is drawn in the new place's light rather
/// than the old one's.
pub fn set_ambient(map: Res<WorldMap>, draws: Res<BiomeDraws>, mut lighting: ResMut<Lighting>) {
    let ambient = ambient_of(&draws, map.current());
    if lighting.ambient != ambient {
        lighting.ambient = ambient;
    }
}

/// What lights a place: the tiles that glow by themselves, and the props
/// that shed light where a piece stood them.
///
/// Built once from the places' tiles and the content's props, and read for
/// every place, so the names below are looked up once and a name nobody
/// registered stops the run at its start.
#[derive(Resource, Clone)]
pub struct Fixtures {
    /// Each tile that glows, and its light.
    tiles: Vec<(TileId, LightDef)>,
    /// Each prop that sheds light, and its light, braziers aside.
    props: Vec<(PropId, LightDef)>,
    /// Each prop that sheds light wherever it stands, stood by the game
    /// rather than a piece's slot: a camp's fire, and a gate mark.
    loose: Vec<(PropId, LightDef)>,
    /// The brazier, whose flame is the colour of the place it burns in.
    brazier: PropId,
    /// The pieces, to read what a place's slots hold.
    prefabs: Prefabs<MonsterDef>,
}

impl Fixtures {
    /// What lights the places of `undermere`, with the props named in
    /// `registries`: of the tiles, every one [`GLOWING`] names; of the
    /// props, lamp posts, the forge and the hearth, the altar and
    /// candelabras, hanging lanterns and witchlights, and braziers
    /// wherever they stand.
    pub fn new(undermere: &Undermere, registries: &Registries) -> Self {
        let tile = |name: &str| undermere.tiles().expect(name);
        let prop = |name: &str| registries.props.expect(name);
        Self {
            tiles: GLOWING.iter().map(|(name, light)| (tile(name), *light)).collect(),
            props: vec![
                (prop("lamp post"), LAMP),
                (prop("forge"), FIRE),
                (prop("hearth"), FIRE),
                (prop("altar"), CANDLES),
                (prop("candelabra"), CANDLES),
                (prop("hanging lantern"), HUNG_LANTERN),
                (prop("witchlight"), WITCHLIGHT),
            ],
            loose: vec![(prop("fire"), CAMPFIRE), (prop(crate::waystone::GATE_MARK), GATE_MARK)],
            brazier: prop("brazier"),
            prefabs: undermere.prefabs().clone(),
        }
    }

    /// Every light the place `map`, of `terrain` and `spots`, is lit by:
    /// each glowing tile in row order, then each lit prop in the order its
    /// spots were stamped, so the fixtures of a place are spawned in one
    /// order every run.
    pub fn of(&self, terrain: &Terrain, spots: &[Spot], map: MapId) -> Vec<(Point, LightSource)> {
        let brazier = if floor_of(map) == Some(FLOORS) { COLD_BRAZIER } else { BRAZIER };
        let glowing = terrain.iter().filter_map(|(p, t)| self.tiles.iter().find(|(tile, _)| *tile == t).map(|(_, light)| (p, light.source())));
        let lit = spots.iter().filter_map(|spot| {
            let slot = spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, glyph)| self.prefabs.slot(key, glyph));
            let Some(Slot::Prop(id)) = slot else { return None };
            let light = if *id == self.brazier { Some(brazier) } else { self.props.iter().find(|(prop, _)| prop == id).map(|(_, light)| *light) };
            light.map(|light| (spot.at, light.source()))
        });
        glowing.chain(lit).collect()
    }
}

/// Lights every fixture of a place the first time it is entered. A fixture
/// is an entity with no turns, which the engine keeps in its static layer
/// and casts again only when the map changes.
pub fn light_fixtures(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, map: Res<WorldMap>, fixtures: Res<Fixtures>) {
    for ev in entered.read().filter(|ev| ev.first) {
        light_place(&mut commands, &map, ev.map, &fixtures);
    }
}

/// Lights every prop that sheds light wherever it stands, on the prop
/// itself, the frame it appears: a camp's fire, which the population stands
/// on a floor's first arrival, and a gate mark, which a waystone leaves,
/// neither with a slot [`Fixtures::of`] could read it off.
///
/// On the prop rather than a light beside it, so a fire put out takes its
/// light with it, and read off the prop's kind as it appears rather than
/// kept, so a fire a save restores is lit the same way, with nothing in the
/// save to say so. Before the light is cast, so the frame a camp is first
/// seen is the frame it is lit.
pub fn light_loose_props(mut commands: Commands, props: Query<(Entity, &PropKind), Added<PropKind>>, fixtures: Res<Fixtures>) {
    for (prop, kind) in &props {
        if let Some((_, light)) = fixtures.loose.iter().find(|(id, _)| *id == kind.0) {
            commands.entity(prop).insert(light.source());
        }
    }
}

/// Lights every fixture of the built place `place`.
///
/// Read off the place's own terrain and spots rather than kept, so a run
/// continued from a save lights each place it restores the way the first
/// arrival lit it.
pub fn light_place(commands: &mut Commands, map: &WorldMap, place: MapId, fixtures: &Fixtures) {
    let Some(built) = map.place(place) else { return };
    for (p, light) in fixtures.of(&built.terrain, &built.spots, place) {
        commands.spawn((Position(p), OnMap(place), light));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::prefab::Slot;

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
        let draws = BiomeDraws::new(&Undermere::new(RunSeed(0)));
        assert_eq!(band(ambient_of(&draws, TOWN)), LightBand::Dim, "Mereholt at dusk");

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

    /// Everything that glows in a place has its light and nothing else
    /// does: every lit window, the gate, every glowcap, every cell of lava
    /// and every shaft of pale light, and every prop that sheds one. A
    /// brazier burns the colour of the place it stands in.
    #[test]
    fn every_glowing_tile_and_every_lit_prop_of_every_place_has_its_light() {
        let registries = crate::content::registries();
        for s in 0..6 {
            let undermere = Undermere::new(RunSeed(s));
            let fixtures = Fixtures::new(&undermere, &registries);
            let tiles = undermere.tiles();
            let glowing = |name: &str| match name {
                "lit window" => Some(WINDOW),
                "gate" => Some(GATE),
                "glowcaps" => Some(GLOWCAP),
                "lava" => Some(LAVA),
                "moonlit marble" => Some(SHAFT),
                "glowing shallows" => Some(SHALLOWS),
                _ => None,
            };
            for map in std::iter::once(TOWN).chain((1..=FLOORS).map(map_of)) {
                let built = undermere.build(map, None).unwrap();
                let lit = fixtures.of(&built.terrain, &built.spots, map);
                let mut expected: Vec<(Point, LightSource)> =
                    built.terrain.iter().filter_map(|(p, t)| glowing(&tiles.get(t).name).map(|light| (p, light.source()))).collect();
                for spot in &built.spots {
                    let prop = spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, glyph)| undermere.prefabs().slot(key, glyph));
                    let Some(Slot::Prop(id)) = prop else { continue };
                    let light = match registries.props.get(*id).name.as_str() {
                        "lamp post" => LAMP,
                        "forge" | "hearth" => FIRE,
                        "altar" | "candelabra" => CANDLES,
                        "hanging lantern" => HUNG_LANTERN,
                        "witchlight" => WITCHLIGHT,
                        "brazier" if map == map_of(FLOORS) => COLD_BRAZIER,
                        "brazier" => BRAZIER,
                        _ => continue,
                    };
                    expected.push((spot.at, light.source()));
                }
                let (mut lit, mut expected) = (lit, expected);
                lit.sort_by_key(|(p, _)| *p);
                expected.sort_by_key(|(p, _)| *p);
                assert_eq!(lit, expected, "{map:?}, seed {s}");
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
            let fixtures = Fixtures::new(&undermere, &registries);
            for floor in 1..FLOORS {
                let drawn = undermere.biome_of(floor);
                if drawn.name != "the Spider Warrens" {
                    continue;
                }
                assert_eq!(drawn.ambient, Light::DARK, "the warrens' ambient is nought");
                for decor in &drawn.decor {
                    match decor {
                        crate::dungeon::Decor::Patches { tile, .. } => {
                            assert!(fixtures.tiles.iter().all(|(t, _)| t != tile), "the warrens strew {:?}, which glows", undermere.tiles().get(*tile).name);
                        }
                        crate::dungeon::Decor::Furnish { piece, .. } => {
                            let defs = undermere.prefabs().defs();
                            let lit = defs.get(defs.expect(piece)).slots().any(|(_, slot)| match slot {
                                Slot::Prop(id) => *id == fixtures.brazier || fixtures.props.iter().any(|(p, _)| p == id),
                                _ => false,
                            });
                            assert!(!lit, "the warrens furnish with {piece:?}, which sheds light");
                        }
                    }
                }
                let built = undermere.build(map_of(floor), None).unwrap();
                let glowing: Vec<Point> = built.terrain.iter().filter(|(_, t)| fixtures.tiles.iter().any(|(g, _)| g == t)).map(|(p, _)| p).collect();
                assert!(glowing.is_empty(), "seed {s}, floor {floor}: the warrens glow at {glowing:?}");
                checked += 1;
            }
        }
        assert!(checked > 0, "some seed drew the Spider Warrens");
    }

    /// On the first arrival in Mereholt every one of its fixtures is lit,
    /// and the street beside each lamp post is lit by it.
    #[test]
    fn mereholt_is_lit_by_its_lamps_its_windows_and_the_gate_on_the_first_arrival() {
        let mut app = crate::testing::headless(RunSeed(2));
        crate::testing::arrive(&mut app, TOWN);
        let mut placed: Vec<Point> = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<(&Position, &OnMap), (With<LightSource>, Without<Player>)>();
            q.iter(world).filter(|(_, on)| on.0 == TOWN).map(|(p, _)| p.0).collect()
        };
        let place = app.world().resource::<WorldMap>().place(TOWN).unwrap();
        let mut planned: Vec<Point> = app.world().resource::<Fixtures>().of(&place.terrain, &place.spots, TOWN).into_iter().map(|(p, _)| p).collect();
        placed.sort();
        planned.sort();
        assert_eq!(placed, planned, "every fixture of the town is lit, once");
        assert!(placed.len() >= 20, "lamps, windows and the gate: {}", placed.len());
        let registries = app.world().resource::<Registries>().clone();
        let lamp_post = registries.props.expect("lamp post");
        let posts: Vec<Point> = {
            let world = app.world_mut();
            let mut q = world.query::<(&Position, &PropKind)>();
            q.iter(world).filter(|(_, kind)| kind.0 == lamp_post).map(|(p, _)| p.0).collect()
        };
        assert_eq!(posts.len(), 7, "Mereholt's seven lamp posts stand");
        let (map, lighting) = (app.world().resource::<WorldMap>(), app.world().resource::<Lighting>());
        for post in posts {
            let beside: Vec<Point> = geometry::square(post, 1).filter(|p| map.is_walkable(*p)).collect();
            assert!(!beside.is_empty(), "the lamp post at {post:?} stands in the street");
            assert!(beside.iter().all(|p| lighting.band(*p) == LightBand::Lit), "the street beside the lamp post at {post:?} is lit");
        }
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
            assert_eq!(light, Some(CAMPFIRE.source()), "seed {s}: the camp's fire carries its own light");
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
        crate::testing::arrive(&mut app, TOWN);
        let draws = app.world().resource::<BiomeDraws>().clone();
        assert_eq!(app.world().resource::<Lighting>().ambient, ambient_of(&draws, TOWN));
        crate::testing::arrive(&mut app, map_of(5));
        assert_eq!(app.world().resource::<Lighting>().ambient, ambient_of(&draws, map_of(5)));
    }
}
