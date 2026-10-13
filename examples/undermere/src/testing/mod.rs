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

use crate::armory::{Armory, ItemKind};
use crate::bestiary::Bestiary;
use crate::dungeon::floor_of;
use crate::run::Adventurer;

mod seeds;

pub use seeds::{QUICK, par_seeds, seeds};

/// A run with no window, seeded, begun on the first update.
///
/// The adventurer's long actions take a turn a frame rather than one each
/// [`WorkPace`] of the wall clock, so a test, a fingerprint or a soak
/// counts frames and never waits on time.
pub fn headless(seed: RunSeed) -> App {
    let mut app = rl_engine::rl_bevy::plugin::headless_app();
    app.add_plugins((FovPlugin, UiPlugin, crate::plugin::narrator()));
    app.insert_resource(Seed(seed)).insert_resource(Saves::new(MemoryBackend::default()));
    app.add_plugins(crate::plugin::UndermerePlugin);
    app.insert_resource(WorkPace::every_frame());
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

/// The one of Mereholt's people whose role in `townsfolk.ron` is `role`,
/// whatever this run named them.
pub fn townsperson(app: &mut App, role: &str) -> Entity {
    let world = app.world_mut();
    let found = world.query::<(Entity, &crate::townsfolk::Townsperson)>().iter(world).find(|(_, p)| p.0 == role).map(|(e, _)| e);
    found.unwrap_or_else(|| panic!("nobody in Mereholt is the {role}"))
}

/// Talks to the person of `role` through the engine's own offer, as a
/// walk into them would, from ground beside them in Mereholt, and returns
/// what the log said last.
pub fn talk_to(app: &mut App, me: Entity, role: &str) -> String {
    let who = townsperson(app, role);
    let talk = app.world().resource::<Verbs>().get(crate::townsfolk::TALK).expect("declared");
    let there = at(app, who);
    let beside = {
        let map = app.world().resource::<WorldMap>();
        (0..8).map(|i| there + Direction::from_index(i).offset()).find(|p| map.is_walkable_on(crate::dungeon::TOWN, *p)).expect("ground beside them")
    };
    stand_on(app, me, beside);
    app.world_mut().write_message(Intent::new(me, Interact { target: who, verb: talk, thing: None }));
    settle(app);
    said(app).last().cloned().expect("something said")
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
    let floor = match floor_of(here(app)) {
        Some(f) => app.world().resource::<crate::light::BiomeDraws>().of(f).floor,
        None => {
            let names = &app.world().resource::<WorldMap>().tables().names;
            TileId(names.iter().position(|n| n == "cobbles").expect("cobbles is a tile") as u16)
        }
    };
    let mut map = app.world_mut().resource_mut::<WorldMap>();
    for p in cells {
        map.set_tile(*p, floor);
    }
}

/// Walls `cells` of the place being read in with the first tile that
/// nobody can walk on, so a test has a wall where it wants one.
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

/// Makes `name` by the game's armory at `level`, as a thing found rather
/// than set out with, placed nowhere: one whose level its wearer learns by
/// wearing it is not known to be any level yet. Called what the run knows
/// of it once a pass or a frame has gone by.
pub fn find(app: &mut App, name: &str, level: i32) -> Entity {
    let mut queue = CommandQueue::default();
    let thing = {
        let world = app.world();
        let armory = world.resource::<Armory>();
        crate::armory::spawn_found(&mut Commands::new(&mut queue, world), armory, armory.defs.expect(name), level, world.resource::<Registries>())
    };
    queue.apply(app.world_mut());
    thing
}

/// Uses `thing`, out of `who`'s pack, as the pack's use key does.
pub fn use_thing(app: &mut App, who: Entity, thing: Entity) {
    app.world_mut().write_message(Intent::new(who, UseItem(thing)));
    settle(app);
}

/// Reads `scroll`, out of `who`'s pack, on `on`, as the pack's choosing
/// does once the thing is picked.
pub fn read_on(app: &mut App, who: Entity, scroll: Entity, on: Entity) {
    app.world_mut().write_message(Intent::new(who, UseOn { item: scroll, on }));
    settle(app);
}

/// Every line of the log, oldest first.
pub fn said(app: &App) -> Vec<String> {
    app.world().resource::<MessageLog>().iter().map(|line| line.text.clone()).collect()
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
        crate::bestiary::spawn_monster(&mut commands, bestiary.defs.get(bestiary.defs.expect(name)), at, here, &registries, false)
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

/// How far round the adventurer [`arena`] clears the ground.
pub const REACH: i32 = 5;

/// A fresh run's adventurer, sturdy, strong enough for anything the armory
/// holds and with nothing in their hands, with nobody else about and open
/// ground [`REACH`] round them with nothing standing on it: where every
/// fight and every zap a test stages is staged, so what a test puts in
/// their hands is what strikes, and a miss for want of strength or a lamp
/// post in the way is never what it reads.
///
/// On floor one rather than in Mereholt: part 5's peace there drops every
/// blow and every zap to nothing before it reaches `Health`, which an
/// arena staged in the town would make true of nothing a test put in the
/// adventurer's hands. [`arena_on`] is this with the floor named, for a
/// test about the floor itself; this is that with the one every other
/// test about combat, which cares only that a floor is under its feet,
/// never names.
///
/// Empty-handed, since a blow is struck with the first weapon worn in slot
/// order: a dagger put on beside the sword they set out with goes to the
/// free hand, and the sword would strike.
pub fn arena(seed: u64) -> (App, Entity, Point) {
    arena_on(seed, 1)
}

/// An [`arena`] staged in Mereholt rather than on a floor: for a test
/// that asks something of the town itself rather than of open ground
/// under the adventurer's feet, a scroll of summon monsters finding no
/// spawn table to draw from, since nothing stands there that the town
/// did not put.
pub fn arena_in_mereholt(seed: u64) -> (App, Entity, Point) {
    let mut app = headless(RunSeed(seed));
    let me = adventurer(&mut app);
    let here = stage(&mut app, me);
    (app, me, here)
}

/// An [`arena`] with `views` added before anything runs, for a test that
/// reads what a panel would show: a view's plugin is added before the
/// first update or not at all.
pub fn arena_with<M>(seed: u64, views: impl bevy::app::Plugins<M>) -> (App, Entity, Point) {
    let mut app = headless(RunSeed(seed));
    app.add_plugins(views);
    let me = arrive(&mut app, crate::dungeon::map_of(1));
    let here = stage(&mut app, me);
    (app, me, here)
}

/// An [`arena`] on floor `floor` rather than floor one, where that floor's
/// own monsters are found, for what is about the floor: a summoning, a
/// shriek heard over it, its map.
pub fn arena_on(seed: u64, floor: u32) -> (App, Entity, Point) {
    let mut app = headless(RunSeed(seed));
    let me = arrive(&mut app, crate::dungeon::map_of(floor));
    let here = stage(&mut app, me);
    (app, me, here)
}

/// Makes the [`arena`] round `me` where they stand, and says where that is.
fn stage(app: &mut App, me: Entity) -> Point {
    sturdy(app, me);
    clear_monsters(app);
    let hand = app.world().resource::<Registries>().slots.expect("main hand");
    let sword = app.world().get::<Equipped>(me).and_then(|worn| worn.in_slot(hand)).expect("the sword they set out with");
    unequip(app, me, sword);
    let here = at(app, me);
    let ground: Vec<Point> = geometry::square(here, REACH).collect();
    lay_floor(app, &ground);
    let world = app.world_mut();
    let standing: Vec<Entity> = world
        .query_filtered::<(Entity, &Position), With<Prop>>()
        .iter(world)
        .filter(|(_, p)| geometry::chebyshev(p.0, here) <= REACH)
        .map(|(e, _)| e)
        .collect();
    for prop in standing {
        world.despawn(prop);
    }
    let strength = world.resource::<Registries>().stats.expect(crate::content::STRENGTH);
    world.get_mut::<StatBlock>(me).expect("stats").0.set_base(strength, 20);
    settle(app);
    here
}

/// A `name` made as a floor's own is, at `at`, that stands where it is
/// put and never strikes back, with health enough to take any blow: a
/// target, for a test about what the adventurer's blow or zap reaches.
pub fn still(app: &mut App, name: &str, at: Point) -> Entity {
    let e = monster(app, name, at);
    app.world_mut().entity_mut(e).remove::<Mind>().insert(Health::full(1000));
    e
}

/// What `who` has lost of the thousand [`still`] gave it.
pub fn lost(app: &App, who: Entity) -> i32 {
    1000 - app.world().get::<Health>(who).expect("still standing").current
}

/// The gold coins `who` carries.
pub fn gold(app: &App, who: Entity) -> u32 {
    let world = app.world();
    let coin = world.resource::<Armory>().defs.expect("gold coin");
    let bag = world.get::<Inventory>(who).map(|b| b.items.clone()).unwrap_or_default();
    bag.iter().filter(|i| world.get::<ItemKind>(**i).is_some_and(|k| k.0 == coin)).map(|i| world.get::<Stack>(*i).map_or(1, |s| s.count)).sum()
}

/// Leaves the adventurer carrying exactly `coins` gold coins.
pub fn set_gold(app: &mut App, me: Entity, coins: u32) {
    let coin = app.world().resource::<Armory>().defs.expect("gold coin");
    let bag = app.world().get::<Inventory>(me).expect("a pack").items.clone();
    let purses: Vec<Entity> = bag.into_iter().filter(|i| app.world().get::<ItemKind>(*i).is_some_and(|k| k.0 == coin)).collect();
    for purse in purses {
        app.world_mut().get_mut::<Inventory>(me).expect("a pack").items.retain(|i| *i != purse);
        app.world_mut().despawn(purse);
    }
    let purse = make(app, "gold coin");
    app.world_mut().get_mut::<Stack>(purse).expect("gold stacks").count = coins;
    give(app, me, purse);
    assert_eq!(gold(app, me), coins);
}

/// A way through, as a test tells them apart: a stair the engine laid,
/// up or down, or the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Way {
    /// A stair up.
    Up,
    /// A stair down.
    Down,
    /// The Drowned Gate.
    Gate,
}

/// Every way through standing on `map`, in the order they stand: where,
/// which it is, and where it leads. Panics on a transition that is neither
/// a stair nor the gate, which nothing in Undermere lays.
pub fn ways(world: &mut World, map: MapId) -> Vec<(Point, Way, Destination)> {
    let mut q = world.query::<(&Position, &OnMap, &Transition, Option<&Stair>, Has<crate::stairs::Gate>)>();
    let mut ways: Vec<(Point, Way, Destination)> = q
        .iter(world)
        .filter(|(_, on, _, _, _)| on.0 == map)
        .map(|(p, _, t, stair, gate)| {
            let way = match (stair.map(|s| s.climb), gate) {
                (Some(Climb::Up), false) => Way::Up,
                (Some(Climb::Down), false) => Way::Down,
                (None, true) => Way::Gate,
                other => panic!("a way through at {:?} is neither a stair nor the gate alone: {other:?}", p.0),
            };
            (p.0, way, t.to)
        })
        .collect();
    ways.sort_by_key(|(p, w, _)| (*p, *w));
    ways
}

/// Where the way `want` stands on `map`, if it has one.
pub fn way_on(world: &mut World, map: MapId, want: Way) -> Option<Point> {
    ways(world, map).into_iter().find(|(_, w, _)| *w == want).map(|(p, ..)| p)
}
