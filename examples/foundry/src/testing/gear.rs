//! Weapons, armor and ammunition for a test to spawn and equip.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;

use crate::gear::Armory;

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
    let player = empty_handed(app);
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
    let player = empty_handed(app);
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
    let player = empty_handed(app);
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

/// Takes the commando's starting kit out of its hands and its bag and
/// out of the world, so a test that arms it with something else knows
/// exactly what it wields and which hand each piece lands in. Returns the
/// player.
pub fn empty_handed(app: &mut App) -> Entity {
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let kit = std::mem::take(&mut app.world_mut().get_mut::<Inventory>(player).unwrap().items);
    for item in kit {
        app.world_mut().get_mut::<Equipped>(player).unwrap().0.unequip(item);
        app.world_mut().despawn(item);
    }
    player
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
