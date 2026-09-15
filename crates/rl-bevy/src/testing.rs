//! What a test needs to stand an engine app up without a window.
//!
//! Every test of a system that runs in the loop needs the same three
//! things before it can ask its question: a map the player can stand on,
//! combat rules for anything that fights, and a way to press a key the way
//! a keyboard does. Each test module used to build its own, so the engine
//! carried nine copies of one world and three of one key player, and a
//! change to how a world is set up meant editing all of them. These are the
//! one copy, public so a game's tests and the engine's crates share it.
//!
//! [`headless_app`](crate::plugin::headless_app) is the app itself; this
//! module is what goes into it.

use bevy::input::InputSystems;
use bevy::prelude::*;
use rl_core::{Point, RunSeed};
use rl_grid::{TileId, TileRegistry};
use rl_mapgen::Chain;
use rl_mapgen::passes::Fill;
use rl_rules::damage::{DamageKind, DamageKindId, SubtractArmor};
use rl_rules::faction::FactionDef;
use rl_rules::{FactionId, Factions, Registry, Relation};
use rl_world::{BandId, CellFacts, ChunkContext, ChunkRules, Layers, Site, SiteKindId, Surroundings, WorldConfig, WorldGraph, WorldRules};

use crate::combat::{CombatRules, DamageStages};
use crate::world::{ChunkRulesRes, WorldMap, WorldRes};

/// The seed every test world and stream is built from, so a test that
/// fails fails the same way twice.
pub const TEST_SEED: RunSeed = RunSeed(5);

/// A small surface for tests: twelve by ten regions of sixteen tiles, land
/// wherever the generator did not make sea, and one town on the first land
/// region so discovery has something to find.
///
/// Land chunks are floor throughout, so sight reaches as far as its range
/// and a test's actors stand wherever it puts them. Sea chunks are wall, so
/// a step off the land is something a test can bump into.
pub struct TestWorld {
    tiles: TileRegistry,
}

impl TestWorld {
    /// The world over the standard wall and floor tiles.
    pub fn new() -> Self {
        Self::with_tiles(TileRegistry::standard())
    }

    /// The world over `tiles`, which must hold a `wall` and a `floor`, for a
    /// test that registers a tile of its own.
    pub fn with_tiles(tiles: TileRegistry) -> Self {
        Self { tiles }
    }
}

impl Default for TestWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldRules for TestWorld {
    fn classify(&self, f: &CellFacts) -> BandId {
        BandId(if f.is_sea { 0 } else { 1 })
    }

    fn road_friction(&self, band: BandId, _: &CellFacts) -> Option<f32> {
        (band.0 == 1).then_some(0.0)
    }

    fn settlements(&self, layers: &Layers, _: u64) -> Vec<Site> {
        layers.bands.iter().find(|(_, b)| b.0 == 1).map(|(p, _)| vec![Site { kind: SiteKindId(1), position: p }]).unwrap_or_default()
    }
}

impl ChunkRules for TestWorld {
    fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    fn fill(&self, _: &Surroundings) -> TileId {
        self.tiles.expect("floor")
    }

    fn chain(&self, _: &WorldGraph, around: &Surroundings) -> Chain<ChunkContext> {
        let tile = if around.here.band.0 == 0 { self.tiles.expect("wall") } else { self.tiles.expect("floor") };
        Chain::new().then(Fill { tile })
    }
}

/// Installs a [`TestWorld`] over the standard tiles and returns where to
/// stand: eight tiles into the first land region, far enough from its edges
/// that anything a test puts within seven tiles is on land too.
///
/// Inserts the [`WorldMap`], [`WorldRes`] and [`ChunkRulesRes`], so the app
/// wants [`StreamingPlugin`](crate::world::StreamingPlugin) to load the
/// window around the player.
pub fn surface(app: &mut App) -> Point {
    surface_with(app, TileRegistry::standard())
}

/// [`surface`] over `tiles`.
pub fn surface_with(app: &mut App, tiles: TileRegistry) -> Point {
    let rules = TestWorld::with_tiles(tiles.clone());
    let world = WorldGraph::generate(TEST_SEED, WorldConfig { region_size: 16, ..WorldConfig::regions(12, 10) }, &rules);
    let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("a test world has land");
    let start = world.tile_origin(region).offset(8, 8);
    app.insert_resource(WorldMap::new(tiles.tables())).insert_resource(WorldRes(world)).insert_resource(ChunkRulesRes(Box::new(rules)));
    start
}

/// Two sides at war and the one kind of damage between them, as
/// [`two_sides`] registered them.
#[derive(Debug, Clone, Copy)]
pub struct Sides {
    /// The player's side.
    pub ours: FactionId,
    /// Everyone the player's side is hostile to, and who is hostile back.
    pub theirs: FactionId,
    /// The one damage kind, `kinetic`, armored.
    pub kind: DamageKindId,
}

/// Inserts combat rules for a test that fights: the two sides of [`Sides`],
/// mutually hostile, one armored damage kind, armor subtracted, and
/// [`TEST_SEED`] as the run's seed.
pub fn two_sides(app: &mut App) -> Sides {
    let kinds = Registry::from_defs(vec![DamageKind::new("kinetic")]).expect("one kind");
    let kind = kinds.expect("kinetic");
    let names = Registry::from_defs(vec![FactionDef { name: "ours".into() }, FactionDef { name: "theirs".into() }]).expect("two sides");
    let (ours, theirs) = (names.expect("ours"), names.expect("theirs"));
    let mut factions = Factions::new(&names);
    factions.set_mutual(ours, theirs, Relation::Hostile);
    app.insert_resource(CombatRules { kinds, factions })
        .insert_resource(DamageStages(vec![Box::new(SubtractArmor)]))
        .insert_resource(crate::seed::Seed(TEST_SEED));
    Sides { ours, theirs, kind }
}

/// Plays keys into [`ButtonInput<KeyCode>`] the way a keyboard does.
///
/// A key pressed on `ButtonInput` from outside the schedule is wiped in
/// `PreUpdate` before any system sees it, because Bevy clears
/// `just_pressed` at the top of every frame. This presses what
/// [`KeyScript`] holds after that clearing, and lets it go the frame after.
/// Adds Bevy's `InputPlugin` if the app has none.
pub struct KeyScriptPlugin;

impl Plugin for KeyScriptPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<bevy::input::InputPlugin>() {
            app.add_plugins(bevy::input::InputPlugin);
        }
        app.init_resource::<KeyScript>().add_systems(PreUpdate, play_keys.after(InputSystems));
    }
}

/// Keys to press on the next frame.
#[derive(Resource, Debug, Default)]
pub struct KeyScript {
    next: Vec<KeyCode>,
    held: Vec<KeyCode>,
}

impl KeyScript {
    /// Queues `key` to be pressed on the next frame.
    pub fn press(&mut self, key: KeyCode) {
        self.next.push(key);
    }
}

fn play_keys(mut script: ResMut<KeyScript>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    for k in std::mem::take(&mut script.held) {
        keys.release(k);
    }
    let next = std::mem::take(&mut script.next);
    for k in &next {
        keys.press(*k);
    }
    script.held = next;
}

/// Presses `key` for one frame and lets it go on the next, running both.
///
/// Needs [`KeyScriptPlugin`].
pub fn press(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<KeyScript>().press(key);
    app.update();
    app.update();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Viewshed};
    use crate::plugin::headless_app;
    use crate::state::EngineState;

    /// The start `surface` hands back is land, with land for seven tiles
    /// every way, and the player stood there is dealt a turn.
    #[test]
    fn the_surface_stands_a_player_on_open_ground() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin));
        let start = surface(&mut app);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap)).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds a turn");
        let map = app.world().resource::<WorldMap>();
        for dy in -7..=7 {
            for dx in -7..=7 {
                assert!(map.is_walkable(start.offset(dx, dy)), "({dx}, {dy}) from the start is land");
            }
        }
    }

    /// A scripted key reads as just pressed to a system in `Update`, once.
    #[test]
    fn a_scripted_key_is_just_pressed_for_one_frame() {
        #[derive(Resource, Default)]
        struct Seen(u32);
        fn count(keys: Res<ButtonInput<KeyCode>>, mut seen: ResMut<Seen>) {
            if keys.just_pressed(KeyCode::Space) {
                seen.0 += 1;
            }
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, KeyScriptPlugin)).init_resource::<Seen>().add_systems(Update, count);
        press(&mut app, KeyCode::Space);
        app.update();
        assert_eq!(app.world().resource::<Seen>().0, 1);
    }
}
