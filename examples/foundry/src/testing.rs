//! A headless Foundry, for the tests this crate and its integration
//! tests add.
//!
//! The engine plugins the design needs, with no window, plus
//! [`FoundryPlugin`](crate::plugin::FoundryPlugin): the same one `main.rs`
//! adds, so a test exercises exactly what the player runs rather than a
//! harness that quietly fell behind it.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_rules::prelude::Ledger;
use rl_engine::rl_ui::UiPlugin;

use crate::gear::Armory;

/// A run with no window, seeded, with every plugin Foundry's stealth,
/// radar and combat need already added.
///
/// `FactsPlugin` and `AbilitiesPlugin` are here for later tasks; a plugin
/// asserts what it cannot work without the moment play begins, so this
/// harness satisfies both with the smallest thing that counts as
/// "nothing yet": an empty ledger, and abilities loaded from no
/// definitions at all.
pub fn headless(seed: RunSeed) -> App {
    let mut app = rl_engine::rl_bevy::plugin::headless_app();
    app.add_plugins((
        FovPlugin,
        CombatPlugin,
        MindsPlugin,
        StatusPlugin,
        ItemsPlugin,
        ThrowingPlugin,
        LightingPlugin,
        StealthPlugin,
        FactsPlugin,
        AbilitiesPlugin,
    ));
    app.add_engine_effects().insert_resource(Seed(seed)).insert_resource(crate::content::registries());
    app.insert_resource(Counters(Ledger::default()));
    let abilities = {
        let world = app.world();
        let (kinds, registries) = (world.resource::<EffectKinds>(), world.resource::<Registries>());
        Abilities::load("[]", kinds, &registries.names()).unwrap_or_else(|e| panic!("no abilities: {e}"))
    };
    app.insert_resource(abilities);
    app.add_plugins(UiPlugin);
    app.add_plugins(crate::plugin::FoundryPlugin);
    app
}

/// Spawns two hand blasters, puts them in the player's bag and equips
/// both one after the other through the engine's own `Equip` intent, so
/// the first lands in the main hand and the second, finding it taken,
/// lands in the off hand. Returns the player, then both items in the hand
/// order they landed.
///
/// `headless` must already have run at least one turn (the two updates
/// here finish that) before an item can be spawned into the player's bag.
pub fn dual_blasters(app: &mut App) -> (Entity, Entity, Entity) {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let armory = Armory::load(&registries);
    let id = armory.defs.expect("hand blaster");
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let mut equip_one = || {
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let item = crate::gear::spawn_item(&mut commands, &armory, id, &registries);
        queue.apply(app.world_mut());
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(item);
        app.world_mut().write_message(Intent::new(player, Equip(item)));
        app.update();
        item
    };
    let first = equip_one();
    let second = equip_one();
    (player, first, second)
}

/// Spawns the player wielding a fresh slug pistol, with `slugs` loose
/// slugs already in the bag before it goes on, and runs the turn that
/// equips it: with none at all, `sync_ammo` reads that same turn's bag,
/// finds nothing in it, and dries the pistol on the spot, so the caller
/// never sees a first shot for free. Returns the player, then the pistol.
pub fn slug_pistol_with(app: &mut App, slugs: u32) -> (Entity, Entity) {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let armory = Armory::load(&registries);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    if slugs > 0 {
        give_slugs(app, player, slugs);
    }
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    let pistol = crate::gear::spawn_item(&mut commands, &armory, armory.defs.expect("slug pistol"), &registries);
    queue.apply(app.world_mut());
    app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(pistol);
    app.world_mut().write_message(Intent::new(player, Equip(pistol)));
    app.update();
    (player, pistol)
}

/// Spawns two slug pistols, with `slugs` loose slugs already in the bag,
/// and equips both one after the other the way [`dual_blasters`] does for
/// hand blasters, so the first lands in the main hand and the second,
/// finding it taken, lands in the off hand. Returns the player, then both
/// pistols in the hand order they landed.
pub fn dual_slug_pistols_with(app: &mut App, slugs: u32) -> (Entity, Entity, Entity) {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let armory = Armory::load(&registries);
    let id = armory.defs.expect("slug pistol");
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    if slugs > 0 {
        give_slugs(app, player, slugs);
    }
    let mut equip_one = || {
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let item = crate::gear::spawn_item(&mut commands, &armory, id, &registries);
        queue.apply(app.world_mut());
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(item);
        app.world_mut().write_message(Intent::new(player, Equip(item)));
        app.update();
        item
    };
    let first = equip_one();
    let second = equip_one();
    (player, first, second)
}

/// Puts `count` slugs straight in `actor`'s bag and writes the same
/// `ItemEvent::PickedUp` a real pickup off the deck would, so anything
/// that reacts to a real pickup treats this the same way.
pub fn give_slugs(app: &mut App, actor: Entity, count: u32) {
    let registries = app.world().resource::<Registries>().clone();
    let armory = Armory::load(&registries);
    let id = armory.defs.expect("slugs");
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    let slugs = crate::gear::spawn_item(&mut commands, &armory, id, &registries);
    commands.entity(slugs).insert(Stack { key: id.index() as u64, count });
    queue.apply(app.world_mut());
    app.world_mut().get_mut::<Inventory>(actor).unwrap().items.push(slugs);
    app.world_mut().write_message(ItemEvent::PickedUp { actor, item: slugs, merged_into: None });
}

/// `Struck` messages copied out as they are written, the way the engine's
/// own combat tests keep them: a headless app rotates its message buffers
/// on wall time, so reading them straight off `Messages<Struck>` after
/// `update()` can miss what a reader added this same run would have
/// caught.
#[derive(Resource, Default)]
struct StruckLog(Vec<Struck>);

/// Copies every `Struck` written this frame onto the end of `StruckLog`.
fn collect_struck(mut events: MessageReader<Struck>, mut log: ResMut<StruckLog>) {
    log.0.extend(events.read().copied());
}

/// Places a target three tiles east of `shooter`, on floor stamped clear
/// for it so the shot always has a line regardless of what the deck
/// generated there, and sends `shots` attacks at it one at a time,
/// returning every `Struck` each one wrote, in order.
///
/// The target carries no `Actor`, so it never enters the turn queue and
/// never acts: a still target for a test that cares only about what the
/// shooter's own weapon does.
pub fn fire_at_a_target(app: &mut App, shooter: Entity, shots: usize) -> Vec<Struck> {
    if !app.world().contains_resource::<StruckLog>() {
        app.init_resource::<StruckLog>();
        app.add_systems(PostUpdate, collect_struck);
    }
    let pos = app.world().get::<Position>(shooter).copied().expect("the shooter stands somewhere");
    let floor = app.world().resource::<WorldMap>().tile(pos.0).expect("the shooter's own tile is loaded");
    let at = pos.0.offset(3, 0);
    {
        let mut map = app.world_mut().resource_mut::<WorldMap>();
        for dx in 1..=3 {
            map.set_tile(pos.0.offset(dx, 0), floor);
        }
    }
    let target = app.world_mut().spawn((Blocks, Position(at), Health::full(10_000))).id();
    for _ in 0..shots {
        app.world_mut().write_message(Intent::new(shooter, Attack(target)));
        app.update();
    }
    app.world_mut().resource_mut::<StruckLog>().0.drain(..).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_headless_run_wires_every_plugin_the_harness_names() {
        let _app = headless(RunSeed(7));
    }
}
