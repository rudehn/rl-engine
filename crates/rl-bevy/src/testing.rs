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
//! module is what goes into it, and what a test does with it once it runs:
//! [`settle`] and [`pass_turns`] move it on, [`player`], [`here`] and
//! [`at`] find where things stand, [`stand_on`], [`go_through`],
//! [`take_up`], [`put_down`], [`give`], [`equip`], [`unequip`], [`kill`],
//! [`lay`], [`lay_tile`] and [`wall_off`] set a scene up the way the
//! engine's own intents and warps would, and [`name_of`] and [`pack`] read
//! it back. None names a game's content: a kind of harm or a tile is the
//! test's to name. [`seeds`], [`first_seeds`] and [`par_seeds`] play a generation test over
//! a range of seeds on every core. [`ambiguity`] is the other thing every
//! game's tests want and none should write twice: the check that a game's
//! systems touching the same data are ordered, or allowed unordered for a
//! stated reason.

pub mod ambiguity;

use bevy::input::InputSystems;
use bevy::prelude::*;
use rl_core::{Point, RunSeed};
use rl_grid::{TileId, TileRegistry};
use rl_mapgen::Chain;
use rl_mapgen::passes::Fill;
use rl_rules::damage::{DamageKind, DamageKindId, SubtractArmor};
use rl_rules::faction::FactionDef;
use rl_rules::{FactionId, Registry};
use rl_world::{BandId, CellFacts, ChunkContext, ChunkRules, Layers, Site, SiteKindId, Surroundings, WorldConfig, WorldGraph, WorldRules};

use crate::combat::{CombatRules, DamageStages};
use crate::prelude::{
    Arrive, DamageEvent, Destination, DropItem, Equip, GoThrough, Intent, Inventory, MapId, OnMap, PickUp, Player, Position, Stack, Unequip, Wait, WarpRequest,
};
use crate::registries::Registries;
use crate::world::{ChunkRulesRes, WorldMap, WorldRes};

/// The seed range a generation test plays, and the cores it plays them on:
/// the same helpers the generator's own tests use, for a game's.
pub use rl_mapgen::testing::{QUICK, first_seeds, par_seeds, seeds};

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

/// Inserts combat rules and [`Registries`] for a test that fights: the two sides of [`Sides`],
/// mutually hostile, one armored damage kind, armor subtracted, and
/// [`TEST_SEED`] as the run's seed.
pub fn two_sides(app: &mut App) -> Sides {
    let kinds = Registry::from_defs(vec![DamageKind::new("kinetic")]).expect("one kind");
    let kind = kinds.expect("kinetic");
    let names = Registry::from_defs(vec![FactionDef::new("ours"), FactionDef::new("theirs")]).expect("two sides");
    let (ours, theirs) = (names.expect("ours"), names.expect("theirs"));
    app.insert_resource(CombatRules::new(&names).hostile(ours, theirs))
        .insert_resource(Registries { damage_kinds: kinds, factions: names, ..Default::default() })
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

/// Keys to press on the next frame, and keys to keep down.
#[derive(Resource, Debug, Default)]
pub struct KeyScript {
    next: Vec<KeyCode>,
    held: Vec<KeyCode>,
    pinned: Vec<KeyCode>,
}

impl KeyScript {
    /// Queues `key` to be pressed on the next frame and released on the
    /// one after.
    pub fn press(&mut self, key: KeyCode) {
        self.next.push(key);
    }

    /// Queues `key` to be pressed on the next frame and kept down until
    /// [`release`](Self::release): what a finger resting on a key does,
    /// one `just_pressed` and then `pressed` for as long as it rests.
    pub fn hold(&mut self, key: KeyCode) {
        self.next.push(key);
        self.pinned.push(key);
    }

    /// Lets go of a key held with [`hold`](Self::hold), on the next frame.
    pub fn release(&mut self, key: KeyCode) {
        self.pinned.retain(|k| *k != key);
        self.held.push(key);
    }
}

fn play_keys(mut script: ResMut<KeyScript>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    let pinned = script.pinned.clone();
    for k in std::mem::take(&mut script.held) {
        if !pinned.contains(&k) {
            keys.release(k);
        }
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

/// Two frames: enough for one resolved action, and whatever it sets off,
/// to settle, and for a run begun at startup to deal the player a turn.
pub fn settle(app: &mut App) {
    app.update();
    app.update();
}

/// The player, the run begun first if it has not been.
///
/// # Panics
/// Panics when even a begun run stood no player anywhere.
pub fn player(app: &mut App) -> Entity {
    let find = |app: &mut App| app.world_mut().query_filtered::<Entity, With<Player>>().iter(app.world()).next();
    if let Some(me) = find(app) {
        return me;
    }
    settle(app);
    find(app).expect("the run put the player somewhere")
}

/// Waits the player forward `n` whole turns, one frame each.
pub fn pass_turns(app: &mut App, n: u32) {
    let me = player(app);
    for _ in 0..n {
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
    }
}

/// The place being read, which is the one the player stands in.
pub fn here(app: &App) -> MapId {
    app.world().resource::<WorldMap>().current()
}

/// Where `who` stands.
///
/// # Panics
/// Panics when `who` stands nowhere.
pub fn at(app: &App, who: Entity) -> Point {
    app.world().get::<Position>(who).expect("standing somewhere").0
}

/// Moves `who` to `to` on the place being read, through the engine's own
/// warp, so what it stood on and what it stands on are both kept right,
/// and lets the move settle.
pub fn stand_on(app: &mut App, who: Entity, to: Point) {
    let here = here(app);
    app.world_mut().write_message(WarpRequest { actor: who, to: Destination::Place { map: here, arrive: Arrive::At(to) }, after: None });
    settle(app);
}

/// Takes the way through `who` stands on, as the key does.
pub fn go_through(app: &mut App, who: Entity) {
    app.world_mut().write_message(Intent::new(who, GoThrough));
    settle(app);
}

/// Takes up whatever lies where `who` stands, as the key does.
pub fn take_up(app: &mut App, who: Entity) {
    app.world_mut().write_message(Intent::new(who, PickUp));
    settle(app);
}

/// Puts `thing` down where `who` stands, as the pack's key does.
pub fn put_down(app: &mut App, who: Entity, thing: Entity) {
    app.world_mut().write_message(Intent::new(who, DropItem(thing)));
    settle(app);
}

/// Kills `victim` with a blow of the damage kind `kind` names, past what
/// anything survives, credited to `by` or to nobody, the way a burn or a
/// fall would be, and lets the death settle.
///
/// # Panics
/// Panics on a kind the game never registered.
pub fn kill(app: &mut App, victim: Entity, by: Option<Entity>, kind: &str) {
    let kind = app.world().resource::<Registries>().damage_kinds.expect(kind);
    app.world_mut().write_message(DamageEvent::new(victim, rl_rules::Hit::from_source(by, kind, 100_000)));
    settle(app);
}

/// Puts `thing` in `who`'s pack.
///
/// # Panics
/// Panics when `who` carries no pack.
pub fn give(app: &mut App, who: Entity, thing: Entity) {
    app.world_mut().get_mut::<Inventory>(who).expect("a pack").items.push(thing);
}

/// Puts `thing` in `who`'s pack and on, by `Equip`, as a key would: for
/// the player while it holds the turn.
pub fn equip(app: &mut App, who: Entity, thing: Entity) {
    give(app, who, thing);
    app.world_mut().write_message(Intent::new(who, Equip(thing)));
    settle(app);
}

/// Takes `thing` off `who`, by `Unequip`, as a key would, and lets it
/// settle.
pub fn unequip(app: &mut App, who: Entity, thing: Entity) {
    app.world_mut().write_message(Intent::new(who, Unequip(thing)));
    settle(app);
}

/// What `thing` is called.
///
/// # Panics
/// Panics on a thing with no `Name`.
pub fn name_of(app: &App, thing: Entity) -> String {
    app.world().get::<Name>(thing).expect("a name").as_str().to_string()
}

/// What `who` carries, by name and count, in pack order.
///
/// # Panics
/// Panics when `who` carries no pack.
pub fn pack(app: &App, who: Entity) -> Vec<(String, u32)> {
    let world = app.world();
    let bag = &world.get::<Inventory>(who).expect("a pack").items;
    bag.iter().map(|item| (name_of(app, *item), world.get::<Stack>(*item).map_or(1, |s| s.count))).collect()
}

/// Lays `thing` on the floor of the place being read, at `to`.
pub fn lay(app: &mut App, thing: Entity, to: Point) {
    let here = here(app);
    app.world_mut().entity_mut(thing).insert((Position(to), OnMap(here)));
}

/// Paints `cells` of the place being read with `tile`, so a test has the
/// ground it wants there whatever the place built.
pub fn lay_tile(app: &mut App, cells: &[Point], tile: TileId) {
    let mut map = app.world_mut().resource_mut::<WorldMap>();
    for p in cells {
        map.set_tile(*p, tile);
    }
}

/// Walls `cells` of the place being read in with the first tile nobody can
/// walk on, so a test has a wall where it wants one.
pub fn wall_off(app: &mut App, cells: &[Point]) {
    let mut map = app.world_mut().resource_mut::<WorldMap>();
    let count = map.tables().names.len() as u16;
    for cell in cells {
        for id in 0..count {
            map.set_tile(*cell, TileId(id));
            if !map.is_walkable(*cell) {
                break;
            }
        }
    }
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
