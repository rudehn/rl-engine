//! A headless Undermere, for the tests this crate adds.
//!
//! The engine's windowless app, [`rl_engine::testing`], with
//! [`UndermerePlugin`](crate::plugin::UndermerePlugin): the same one the
//! binary adds, so a test plays the game that ships rather than a harness
//! that quietly fell behind it. The run begins at startup, as it does for
//! a player who picked New Game, and is saved in memory, so a test never
//! touches the disk and each starts with an empty slot of its own.
//!
//! What every game's tests do alike, settling a frame, waiting a turn,
//! standing somebody somewhere, putting a thing in a pack and reading the
//! log, is the engine's and re-exported here. What is Undermere's own is
//! below: things made by its armory, monsters by its bestiary, the town's
//! people, its ways through, and the arena its fights are staged in.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::RunSeed;

use crate::armory::{Armory, ItemKind};
use crate::bestiary::Bestiary;
use crate::bestiary::MonsterDef;
use crate::dungeon::{Undermere, floor_of};
use rl_engine::rl_bevy::places::Spot;
use rl_engine::rl_rules::prefab::Slot;

/// The adventurer, the run begun first if it has not been.
pub use rl_engine::testing::player as adventurer;
pub use rl_engine::testing::{
    QUICK, at, equip, first_seeds, give, go_through, here, lay, let_time_pass, name_of, pack, par_seeds, pass_turns, put_down, said, seeds, settle, stand_on,
    take_up, unequip, wall_off,
};

/// A run with no window, seeded, begun on the first update.
///
/// The adventurer's long actions take a turn a frame rather than one each
/// [`WorkPace`] of the wall clock, so a test, a fingerprint or a soak
/// counts frames and never waits on time.
pub fn headless(seed: RunSeed) -> App {
    let mut app = rl_engine::testing::headless(seed, crate::plugin::narrator(), crate::plugin::UndermerePlugin);
    app.insert_resource(WorkPace::every_frame());
    app
}

/// [`headless`] without [`UndermerePlugin`](crate::plugin::UndermerePlugin):
/// the engine's plugins the game plays with and what it saves, and none
/// of its own systems, so the ordering test can tell the game's systems
/// from the engine's by what the plugin adds beyond these.
pub fn headless_without_undermere(seed: RunSeed) -> App {
    rl_engine::testing::headless(seed, crate::plugin::narrator(), |app: &mut App| {
        crate::plugin::add_engine(app);
        crate::save::register(app);
    })
}

/// A run continued from `text`, a save as the slot holds it: the save in
/// the memory backend, `ContinueRun` asked for as the title's Continue
/// asks for it, and the first frames run so the continued run stands.
pub fn continued(text: &str) -> App {
    rl_engine::testing::continued(headless(RunSeed(0)), crate::save::SLOT, text)
}

/// What a piece's slot at `spot` holds, if the spot is a slot: the lookup
/// a test of a placed piece does for every spot of a built floor, here once
/// so none keeps its own.
pub fn slot_of<'a>(undermere: &'a Undermere, spot: &Spot) -> Option<&'a Slot<MonsterDef>> {
    spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, glyph)| undermere.prefabs().slot(key, glyph))
}

/// The first seed and floor whose run draws the pool piece `piece`, for a
/// test that stages one arrival where the piece stands instead of playing
/// seeds to find it. Panics when no run draws it, so a renamed piece fails
/// the test instead of leaving it to pass over nothing.
#[cfg(test)]
pub(crate) fn first_floor_holding(piece: &str) -> (RunSeed, u32) {
    crate::dungeon::drawn(&[piece])
}

/// The first seed and floor whose run draws every one of the pool pieces
/// `pieces` on that one floor, for a test that asks something of them side
/// by side. Panics as [`first_floor_holding`] does.
#[cfg(test)]
pub(crate) fn first_floor_holding_all(pieces: &[&str]) -> (RunSeed, u32) {
    crate::dungeon::drawn(pieces)
}

/// Warps the adventurer to `map`'s way in, the way the stairs would, and
/// lets the arrival settle. Returns the adventurer.
pub fn arrive(app: &mut App, map: MapId) -> Entity {
    let me = adventurer(app);
    app.world_mut().write_message(WarpRequest::into_place(me, map));
    settle(app);
    me
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
    rl_engine::testing::kill(app, victim, by, "crush");
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
    rl_engine::testing::lay_tile(app, cells, floor);
}

/// Makes `name` by the game's armory, placed nowhere.
pub fn make(app: &mut App, name: &str) -> Entity {
    let mut queue = CommandQueue::default();
    let thing = {
        let world = app.world();
        let armory = world.resource::<Armory>();
        crate::armory::spawn_item(&mut Commands::new(&mut queue, world), armory, armory.defs.expect(name))
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
        crate::armory::spawn_found(&mut Commands::new(&mut queue, world), armory, armory.defs.expect(name), level)
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

/// Puts `thing`, in `who`'s pack, straight into its slots, for a monster,
/// which nobody's key equips: as if it had put it on itself.
pub fn wear(app: &mut App, who: Entity, thing: Entity) {
    give(app, who, thing);
    let shape = app.world().get::<Wearable>(thing).expect("a thing to wear").0.clone();
    app.world_mut().get_mut::<Equipped>(who).expect("slots").0.equip(thing, &shape).expect("it fits");
}

/// Makes `thing`, which the adventurer wears, known by wear: counted one
/// turn short of [`FAMILIAR_AFTER`](crate::lore::FAMILIAR_AFTER) and worn
/// the one turn more, so the count that makes it known and the line that
/// says so are the game's own rather than this helper's, without the
/// two hundred turns played.
///
/// Panics, naming the thing, if it is already known or not worn, or if
/// that turn did not make it known.
pub fn familiar(app: &mut App, thing: Entity) {
    use crate::lore::{FAMILIAR_AFTER, Unfamiliar};
    let name = name_of(app, thing);
    let me = adventurer(app);
    assert!(app.world().get::<Equipped>(me).is_some_and(|worn| worn.contains(thing)), "{name} is not worn, so wearing it longer makes nothing known");
    app.world_mut().get_mut::<Unfamiliar>(thing).unwrap_or_else(|| panic!("{name} is already known")).worn = FAMILIAR_AFTER - 1;
    // A frame for the count to note the turn it counts on from, as one
    // made on putting it on would.
    app.update();
    pass_turns(app, 1);
    assert!(app.world().get::<Unfamiliar>(thing).is_none(), "{name} was worn the last turn and is still not known");
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

/// How far in from every edge of the place the [`arena`] is staged: a
/// test aims and stands things up to twelve cells off, and a cell past the
/// edge of the map is nothing a bolt reaches or a step lands on.
const ROOM: i32 = 12;

/// Stands `me` [`ROOM`] in from every edge of the place, on ground laid
/// for them, when they arrived nearer one than that, as a way in against
/// an edge does; and says where they stand, which is where the warp set
/// them down.
fn in_from_the_edge(app: &mut App, me: Entity) -> Point {
    let p = at(app, me);
    let b = app.world().resource::<WorldMap>().window_tiles();
    let room = Point::new(p.x.clamp(b.x + ROOM, b.right() - 1 - ROOM), p.y.clamp(b.y + ROOM, b.bottom() - 1 - ROOM));
    if room != p {
        lay_floor(app, &geometry::square(room, REACH).collect::<Vec<_>>());
        stand_on(app, me, room);
    }
    at(app, me)
}

/// Makes the [`arena`] round `me` where they stand, and says where that is.
fn stage(app: &mut App, me: Entity) -> Point {
    sturdy(app, me);
    clear_monsters(app);
    let hand = app.world().resource::<Registries>().slots.expect("main hand");
    let sword = app.world().get::<Equipped>(me).and_then(|worn| worn.in_slot(hand)).expect("the sword they set out with");
    unequip(app, me, sword);
    let here = in_from_the_edge(app, me);
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
