//! A headless Undermere, for the tests this crate adds.
//!
//! The engine plugins a window would have brought, with no window, and
//! [`UndermerePlugin`](crate::plugin::UndermerePlugin): the same one the
//! binary adds, so a test plays the game that ships rather than a harness
//! that quietly fell behind it. The run begins at startup, as it does for
//! a player who picked New Game, and is saved in memory, so a test never
//! touches the disk and each starts with an empty slot of its own.
//!
//! The helpers stand the adventurer where a test wants them, through the
//! engine's own warp, and put things in their hands the way the game
//! makes them: by the armory, worn through the engine's own `Equip`.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::RunSeed;

use crate::armory::Armory;
use crate::bestiary::Bestiary;
use crate::dungeon::{Biome, floor_of};
use crate::run::Adventurer;

/// A run with no window, seeded, begun on the first update.
pub fn headless(seed: RunSeed) -> App {
    let mut app = rl_engine::rl_bevy::plugin::headless_app();
    app.add_plugins((FovPlugin, UiPlugin, crate::plugin::narrator()));
    app.insert_resource(Seed(seed)).insert_resource(Saves::new(MemoryBackend::default()));
    app.add_plugins(crate::plugin::UndermerePlugin);
    app
}

/// A run continued from `text`, a save as the slot holds it: the save in
/// the memory backend, `ContinueRun` asked for as the title's Continue
/// asks for it, and the first frames run so the continued run stands.
pub fn continued(text: &str) -> App {
    let mut app = headless(RunSeed(0));
    app.world().resource::<Saves>().persist(crate::save::SLOT, text).expect("the memory backend takes it");
    app.insert_resource(ContinueRun);
    settle(&mut app);
    settle(&mut app);
    app
}

/// Two updates: enough for one resolved action and whatever it sets off
/// to settle.
pub fn settle(app: &mut App) {
    app.update();
    app.update();
}

/// Waits the adventurer forward `n` whole turns, one at a time.
pub fn pass_turns(app: &mut App, n: u32) {
    let me = adventurer(app);
    for _ in 0..n {
        app.world_mut().write_message(Intent::new(me, Wait));
        app.update();
    }
}

/// The adventurer, the run begun first if it has not been.
pub fn adventurer(app: &mut App) -> Entity {
    let find = |app: &mut App| app.world_mut().query_filtered::<Entity, With<Adventurer>>().iter(app.world()).next();
    if let Some(me) = find(app) {
        return me;
    }
    settle(app);
    find(app).expect("the run put the adventurer somewhere")
}

/// The place being read, which is the one the adventurer stands in.
pub fn here(app: &App) -> MapId {
    app.world().resource::<WorldMap>().current()
}

/// Where `who` stands.
pub fn at(app: &App, who: Entity) -> Point {
    app.world().get::<Position>(who).expect("standing somewhere").0
}

/// Warps the adventurer to `map`'s way in, the way the stairs would, and
/// lets the arrival settle. Returns the adventurer.
pub fn arrive(app: &mut App, map: MapId) -> Entity {
    let me = adventurer(app);
    app.world_mut().write_message(WarpRequest::into_place(me, map));
    settle(app);
    me
}

/// Moves `me` to `at` on the place it is in, through the engine's own
/// warp, so what it stood on and what it stands on are both kept right.
pub fn stand_on(app: &mut App, me: Entity, at: Point) {
    let here = here(app);
    app.world_mut().write_message(WarpRequest { actor: me, to: Destination::Place { map: here, arrive: Arrive::At(at) } });
    settle(app);
}

/// Takes the way through `me` stands on.
pub fn go_through(app: &mut App, me: Entity) {
    app.world_mut().write_message(Intent::new(me, GoThrough));
    settle(app);
}

/// Gives `me` more health than anything under the lake can take in a
/// test's few turns, for a test about where the adventurer goes rather
/// than whether they live through it.
pub fn sturdy(app: &mut App, me: Entity) {
    app.world_mut().entity_mut(me).insert(Health::full(10_000));
}

/// Kills `victim` with a blow nothing under the lake survives, credited
/// to `by`, or to nobody, the way a burn or a fall would be, and lets the
/// death settle.
pub fn kill(app: &mut App, victim: Entity, by: Option<Entity>) {
    let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
    app.world_mut().write_message(DamageEvent::new(victim, rl_engine::rl_rules::Hit::from_source(by, crush, 100_000)));
    settle(app);
}

/// Takes up whatever lies where `me` stands, as the key does.
pub fn take_up(app: &mut App, me: Entity) {
    app.world_mut().write_message(Intent::new(me, PickUp));
    settle(app);
}

/// Puts `thing` down where `me` stands, as the key does.
pub fn put_down(app: &mut App, me: Entity, thing: Entity) {
    app.world_mut().write_message(Intent::new(me, DropItem(thing)));
    settle(app);
}

/// Takes every monster off every place, for a test about the adventurer's
/// own things rather than about who is near them.
pub fn clear_monsters(app: &mut App) {
    let world = app.world_mut();
    let monsters: Vec<Entity> = world.query_filtered::<Entity, With<crate::bestiary::Kind>>().iter(world).collect();
    for monster in monsters {
        world.despawn(monster);
    }
}

/// Takes every thing lying on the place being read, for a test about what
/// it lays there itself: a floor's own loot lies where it fell, and a
/// goblin fetching something to throw picks up everything on its cell.
pub fn clear_floor(app: &mut App) {
    let here = here(app);
    let world = app.world_mut();
    let lying: Vec<Entity> =
        world.query_filtered::<(Entity, &OnMap), (With<Item>, With<Position>)>().iter(world).filter(|(_, on)| on.0 == here).map(|(e, _)| e).collect();
    for thing in lying {
        world.despawn(thing);
    }
}

/// Paints `cells` of the place being read with its own floor, so a test
/// has open ground there whatever the place built.
pub fn lay_floor(app: &mut App, cells: &[Point]) {
    let name = floor_of(here(app)).map_or("cobbles", |floor| Biome::of(floor).floor());
    let mut map = app.world_mut().resource_mut::<WorldMap>();
    let floor = TileId(map.tables().names.iter().position(|n| n == name).expect("the place's floor is a tile") as u16);
    for p in cells {
        map.set_tile(*p, floor);
    }
}

/// Makes `name` by the game's armory, placed nowhere.
pub fn make(app: &mut App, name: &str) -> Entity {
    let mut queue = CommandQueue::default();
    let thing = {
        let world = app.world();
        let armory = world.resource::<Armory>();
        crate::armory::spawn_item(&mut Commands::new(&mut queue, world), armory, armory.defs.expect(name), world.resource::<Registries>())
    };
    queue.apply(app.world_mut());
    thing
}

/// Lays `thing` on the floor of the place being read, at `at`.
pub fn lay(app: &mut App, thing: Entity, at: Point) {
    let here = here(app);
    app.world_mut().entity_mut(thing).insert((Position(at), OnMap(here)));
}

/// Puts `thing` in `who`'s pack.
pub fn give(app: &mut App, who: Entity, thing: Entity) {
    app.world_mut().get_mut::<Inventory>(who).expect("a pack").items.push(thing);
}

/// Puts `thing` in `who`'s pack and on, by `Equip`, as a key would: for
/// the adventurer while they hold the turn.
pub fn equip(app: &mut App, who: Entity, thing: Entity) {
    give(app, who, thing);
    app.world_mut().write_message(Intent::new(who, Equip(thing)));
    settle(app);
}

/// Takes `thing` off `who`, by `Unequip`, as a key would.
pub fn unequip(app: &mut App, who: Entity, thing: Entity) {
    app.world_mut().write_message(Intent::new(who, Unequip(thing)));
    settle(app);
}

/// Puts `thing`, in `who`'s pack, straight into its slots, for a monster,
/// which nobody's key equips: as if it had put it on itself.
pub fn wear(app: &mut App, who: Entity, thing: Entity) {
    give(app, who, thing);
    let shape = app.world().get::<Wearable>(thing).expect("a thing to wear").0.clone();
    app.world_mut().get_mut::<Equipped>(who).expect("slots").0.equip(thing, &shape).expect("it fits");
}

/// Spawns the monster `name` at `at` on the place being read, made as a
/// floor's own are.
pub fn monster(app: &mut App, name: &str, at: Point) -> Entity {
    let registries = app.world().resource::<Registries>().clone();
    let here = here(app);
    let mut queue = CommandQueue::default();
    let e = {
        let world = app.world();
        let bestiary = world.resource::<Bestiary>();
        let mut commands = Commands::new(&mut queue, world);
        crate::bestiary::spawn_monster(&mut commands, bestiary.defs.get(bestiary.defs.expect(name)), at, here, &registries)
    };
    queue.apply(app.world_mut());
    e
}

/// The names of what `who` wears, slot by slot, `None` for an empty one.
pub fn worn(app: &App, who: Entity) -> Vec<Option<String>> {
    let world = app.world();
    let equipped = world.get::<Equipped>(who).expect("slots");
    let registries = world.resource::<Registries>();
    registries.slots.iter().map(|(slot, _)| equipped.in_slot(slot).map(|item| name_of(app, item))).collect()
}

/// What `who` carries, by name and count, in pack order.
pub fn pack(app: &App, who: Entity) -> Vec<(String, u32)> {
    let world = app.world();
    let bag = &world.get::<Inventory>(who).expect("a pack").items;
    bag.iter().map(|item| (name_of(app, *item), world.get::<Stack>(*item).map_or(1, |s| s.count))).collect()
}

/// What `thing` is called.
pub fn name_of(app: &App, thing: Entity) -> String {
    app.world().get::<Name>(thing).expect("a name").as_str().to_string()
}
