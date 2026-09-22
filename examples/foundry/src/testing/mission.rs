//! The reactor console and the upgrade pick, for a test to reach and read.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::geometry;
use rl_engine::rl_rules::QuestState;

use crate::gear::Armory;
use crate::upgrades::Upgrade;

/// Warps the player onto deck three and stands it beside the console
/// `mission::spawn_console_on_arrival` plants there on first arrival, so a
/// test can charge it without hunting for a walkable tile of its own.
///
/// The console is a prop, so it is found by its kind: whatever `props.ron`
/// calls a reactor console.
pub fn beside_the_console(app: &mut App) -> Entity {
    crate::testing::arrive_on(app, 3);
    let console = app.world().resource::<Registries>().props.expect("reactor console");
    let console_at = {
        let world = app.world_mut();
        let mut q = world.query::<(&Position, &PropKind)>();
        q.iter(world).find(|(_, kind)| kind.0 == console).expect("deck three spawns a console on arrival").0.0
    };
    let beside = {
        let map = app.world().resource::<WorldMap>();
        geometry::square(console_at, 1).find(|&p| p != console_at && map.is_walkable(p)).expect("the reactor chamber has floor beside its console")
    };
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let map = app.world().resource::<WorldMap>().current();
    app.world_mut().get_mut::<Position>(player).unwrap().0 = beside;
    app.world_mut().entity_mut(player).insert(OnMap(map));
    player
}

/// The key that walks from `player` toward the console, for a test that
/// charges it the way a player does: by walking into it.
///
/// The console blocks, so a bump into it is what the engine turns into the
/// interaction its offer describes; there is no charge key any more.
pub fn key_toward_the_console(app: &App, player: Entity) -> bevy::prelude::KeyCode {
    let console = app.world().resource::<Registries>().props.expect("reactor console");
    let world = app.world();
    let at = world.get::<Position>(player).expect("the player stands somewhere").0;
    let console_at = world
        .iter_entities()
        .filter_map(|e| e.get::<Position>().zip(e.get::<PropKind>()))
        .find(|(_, kind)| kind.0 == console)
        .map(|(pos, _)| pos.0)
        .expect("deck three spawns a console on arrival");
    let dir = rl_engine::rl_core::Direction::between(at, console_at).expect("the player stands beside it");
    let keys = world.resource::<rl_engine::rl_ui::DirectionKeys>();
    keys.0.iter().find(|(_, d)| *d == dir).map(|(k, _)| *k).expect("every direction has a key")
}

/// Whether the quest named `name` reads [`QuestState::Done`].
pub fn quest_done(app: &App, name: &str) -> bool {
    let quests = app.world().resource::<Quests>();
    quests.tracker.state(quests.defs.expect(name)) == QuestState::Done
}

/// The run's clock, in hundredths of a whole turn.
pub fn clock(app: &App) -> u32 {
    app.world().resource::<Turns>().now()
}

/// Spawns the player wielding a fresh hand blaster. Returns the player,
/// then the blaster.
pub fn player_with_hand_blaster(app: &mut App) -> (Entity, Entity) {
    app.update();
    app.update();
    let player = crate::testing::empty_handed(app);
    let item = equip_new(app, player, "hand blaster");
    (player, item)
}

/// Spawns `name`, puts it in `player`'s bag, and equips it through the
/// engine's own `Equip` intent, the way `gear.rs`'s own unit tests do,
/// exposed here so `upgrades.rs` can equip something new after a pick and
/// see the pick's own effect on it.
pub fn equip_new(app: &mut App, player: Entity, name: &str) -> Entity {
    let registries = app.world().resource::<Registries>().clone();
    let armory = Armory::load(&registries);
    let id = armory.defs.expect(name);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    let item = crate::gear::spawn_item(&mut commands, &armory, id, &registries);
    queue.apply(app.world_mut());
    app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(item);
    app.world_mut().write_message(Intent::new(player, Equip(item)));
    app.update();
    item
}

/// Takes `item` off `player` through the engine's own `Unequip` intent.
pub fn unequip(app: &mut App, player: Entity, item: Entity) {
    app.world_mut().write_message(Intent::new(player, Unequip(item)));
    app.update();
}

/// Applies `upgrade` to `player` directly, the way the pick screen's own
/// confirm key does, without needing a screen or a keypress for a test to
/// drive.
pub fn pick(app: &mut App, player: Entity, upgrade: Upgrade) {
    crate::upgrades::apply(upgrade, player, app.world_mut());
    app.update();
}

/// Whether `player` knows the ability named `name`.
pub fn knows(app: &App, player: Entity, name: &str) -> bool {
    let id = app.world().resource::<Abilities>().expect(name);
    app.world().get::<Known>(player).is_some_and(|k| k.has(id))
}
