//! Light in Mereholt and under it: an ambient for every place, the
//! fixtures each place is lit by, and how far the adventurer's lantern
//! reaches as the dark grows with the danger.
//!
//! The dark is the design's: the town at dusk and the Upper Halls are lit
//! enough to see by, and below them a cell is seen only when something
//! lights it, so the lantern, a brazier, a clump of glowcaps or a pool of
//! lava is what a floor is seen by. The engine's lighting does the rest:
//! a fixture is a `LightSource` on an entity with no turns, cast once and
//! cast again only when the map changes.
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

use crate::armory::{Armory, ItemKind};
use crate::bestiary::MonsterDef;
use crate::content::LightDef;
use crate::dungeon::{Biome, TOWN, Undermere, floor_of};
use crate::run::Adventurer;

/// A lamp post's lantern: warm, and far enough to light the street round
/// it.
pub const LAMP: LightDef = LightDef { radius: 5, color: (1.0, 0.8, 0.52), flicker: 0.12 };
/// A lit window: firelight through the glass, a little way into the street
/// and the room behind it.
pub const WINDOW: LightDef = LightDef { radius: 3, color: (1.0, 0.76, 0.42), flicker: 0.1 };
/// A forge or a hearth: orange, and restless.
pub const FIRE: LightDef = LightDef { radius: 4, color: (1.0, 0.5, 0.2), flicker: 0.35 };
/// Candles, on an altar or a candelabra: pale gold, and steadier than fire.
pub const CANDLES: LightDef = LightDef { radius: 4, color: (1.0, 0.88, 0.58), flicker: 0.2 };
/// A brazier left burning in the Upper Halls: wide enough to light the room
/// it stands in.
pub const BRAZIER: LightDef = LightDef { radius: 6, color: (1.0, 0.6, 0.28), flicker: 0.4 };
/// A brazier before the throne, burning a cold blue-green.
pub const COLD_BRAZIER: LightDef = LightDef { radius: 6, color: (0.35, 0.95, 0.8), flicker: 0.25 };
/// The Drowned Gate, on both its sides: violet.
pub const GATE: LightDef = LightDef { radius: 5, color: (0.75, 0.42, 1.0), flicker: 0.15 };
/// A glowcap: a soft teal, a step or so round it, so a clump of them is a
/// pool of light and a lone one a mark in the dark.
pub const GLOWCAP: LightDef = LightDef { radius: 2, color: (0.4, 1.0, 0.85), flicker: 0.0 };
/// A cell of lava: orange, and every cell of a pool sheds it, so a pool
/// lights its shore.
pub const LAVA: LightDef = LightDef { radius: 3, color: (1.0, 0.52, 0.15), flicker: 0.3 };
/// A shaft of pale light come down through the lake onto the cathedral's
/// marble.
pub const SHAFT: LightDef = LightDef { radius: 2, color: (0.72, 0.84, 1.0), flicker: 0.0 };

/// How far the adventurer's lantern reaches on `floor`, `None` for the
/// town: seven tiles at the top, and a tile less every third floor, down
/// to four before the throne, so the dark closes in as the floors grow
/// more dangerous.
pub fn lantern_radius(floor: Option<u32>) -> i32 {
    let dimmed = floor.map_or(0, |floor| (floor.saturating_sub(1) / 3).min(3));
    7 - dimmed as i32
}

/// The light everywhere in `map` before anything in it is lit.
///
/// Mereholt is at dusk, blue and low but enough to see the town by, and
/// the Upper Halls keep a dim, warm grey, so the first floors are learned
/// in sight of what is coming. Every band below is darker than the seen
/// threshold, its own colour tinting only what something else lights. Any
/// other map is dark.
pub fn ambient_of(map: MapId) -> Light {
    if map == TOWN {
        return Light::new(30, Rgb::new(120, 140, 215));
    }
    match floor_of(map).map(Biome::of) {
        Some(Biome::UpperHalls) => Light::new(22, Rgb::new(205, 190, 170)),
        Some(Biome::FungalDeeps) => Light::new(10, Rgb::new(90, 170, 150)),
        Some(Biome::Catacombs) => Light::new(9, Rgb::new(150, 160, 205)),
        Some(Biome::DrownedCathedral) => Light::new(11, Rgb::new(80, 115, 210)),
        Some(Biome::ForgeOfEmbers) => Light::new(12, Rgb::new(210, 80, 50)),
        Some(Biome::Throne) => Light::new(5, Rgb::new(90, 140, 160)),
        None => Light::DARK,
    }
}

/// Keeps every lantern the adventurer carries reaching as far as
/// [`lantern_radius`] says for where they stand: seven tiles in Mereholt
/// and on the first floors, and a tile less every third floor down.
///
/// Read off where the adventurer is every pass rather than set on an
/// arrival, so a lantern picked up on floor seven, or brought back by a
/// continued run, dims to the floor it is carried on at once. In
/// `TurnSet::React`, so the floor a pass arrives on is lit by the lantern
/// as it reaches there before anybody on it decides by that light. Only
/// the lantern: a torch burns as it burns, and lies about to be found.
pub fn dim_the_lantern(
    adventurer: Query<(&Inventory, Option<&OnMap>), With<Adventurer>>,
    mut lights: Query<(&ItemKind, &mut LightSource)>,
    armory: Res<Armory>,
) {
    let Some(lantern) = armory.defs.id(crate::run::LANTERN) else { return };
    for (bag, on) in &adventurer {
        let radius = lantern_radius(floor_of(on.map_or(MapId::SURFACE, |m| m.0)));
        for item in &bag.items {
            if let Ok((kind, mut light)) = lights.get_mut(*item)
                && kind.0 == lantern
                && light.radius != radius
            {
                light.radius = radius;
            }
        }
    }
}

/// Sets the current place's ambient, between the turns and the light, so
/// the frame the stairs land on is drawn in the new place's light rather
/// than the old one's.
pub fn set_ambient(map: Res<WorldMap>, mut lighting: ResMut<Lighting>) {
    let ambient = ambient_of(map.current());
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
    /// The brazier, whose flame is the colour of the place it burns in.
    brazier: PropId,
    /// The pieces, to read what a place's slots hold.
    prefabs: Prefabs<MonsterDef>,
}

impl Fixtures {
    /// What lights the places of `undermere`, with the props named in
    /// `registries`: of the tiles, the town's lit windows and the gate, and
    /// below it the glowcaps, the lava and the cathedral's shafts of light;
    /// of the props, lamp posts, the forge and the hearth, the altar and
    /// candelabras, and braziers wherever they stand.
    pub fn new(undermere: &Undermere, registries: &Registries) -> Self {
        let tile = |name: &str| undermere.tiles().expect(name);
        let prop = |name: &str| registries.props.expect(name);
        Self {
            tiles: vec![(tile("lit window"), WINDOW), (tile("gate"), GATE), (tile("glowcaps"), GLOWCAP), (tile("lava"), LAVA), (tile("moonlit marble"), SHAFT)],
            props: vec![(prop("lamp post"), LAMP), (prop("forge"), FIRE), (prop("hearth"), FIRE), (prop("altar"), CANDLES), (prop("candelabra"), CANDLES)],
            brazier: prop("brazier"),
            prefabs: undermere.prefabs().clone(),
        }
    }

    /// Every light the place `map`, of `terrain` and `spots`, is lit by:
    /// each glowing tile in row order, then each lit prop in the order its
    /// spots were stamped, so the fixtures of a place are spawned in one
    /// order every run.
    pub fn of(&self, terrain: &Terrain, spots: &[Spot], map: MapId) -> Vec<(Point, LightSource)> {
        let brazier = if floor_of(map).map(Biome::of) == Some(Biome::Throne) { COLD_BRAZIER } else { BRAZIER };
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

    use rl_engine::rl_bevy::lighting::DEFAULT_THRESHOLD;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::prefab::Slot;

    use super::*;
    use crate::dungeon::{Biome, FLOORS, TOWN, map_of};

    #[test]
    fn the_lantern_dims_by_a_tile_every_third_floor() {
        assert_eq!(lantern_radius(None), 7, "in Mereholt");
        let down: Vec<i32> = (1..=FLOORS).map(|floor| lantern_radius(Some(floor))).collect();
        assert_eq!(down, [7, 7, 7, 6, 6, 6, 5, 5, 5, 4]);
    }

    /// The town at dusk and the first band are lit enough to see by, so the
    /// first fights are learned with the lights on; below them a cell is
    /// seen only by what lights it. Each band has an ambient of its own.
    #[test]
    fn the_town_and_the_upper_halls_are_lit_enough_to_see_by_and_the_floors_below_are_not() {
        let seen = |map: MapId| ambient_of(map).intensity >= DEFAULT_THRESHOLD;
        assert!(seen(TOWN), "Mereholt at dusk");
        assert!(seen(map_of(1)) && seen(map_of(2)), "the Upper Halls");
        for floor in 3..=FLOORS {
            assert!(!seen(map_of(floor)), "floor {floor} is dark");
        }
        let mut bands: Vec<Light> = Biome::ALL.iter().map(|b| ambient_of(map_of((1..=FLOORS).find(|f| Biome::of(*f) == *b).unwrap()))).collect();
        bands.push(ambient_of(TOWN));
        let distinct: BTreeSet<(u8, u8, u8, u8)> = bands.iter().map(|l| (l.intensity, l.color.r, l.color.g, l.color.b)).collect();
        assert_eq!(distinct.len(), bands.len(), "each band and the town have their own ambient");
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
        assert_eq!(app.world().resource::<Lighting>().ambient, ambient_of(TOWN));
        crate::testing::arrive(&mut app, map_of(5));
        assert_eq!(app.world().resource::<Lighting>().ambient, ambient_of(map_of(5)));
    }
}
