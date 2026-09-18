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
use rl_engine::rl_core::{Point, RunSeed, geometry};
use rl_engine::rl_grid::Rgb;
use rl_engine::rl_rules::Hit;
use rl_engine::rl_rules::prelude::Ledger;
use rl_engine::rl_ui::UiPlugin;

use crate::droids::Roster;
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

/// Spawns `name` `range` tiles east of the player, on floor stamped clear
/// for it the way [`fire_at_a_target`] clears its own line, so its brain
/// always has a shot regardless of what the deck generated there. Also
/// lights the player's own tile: the foundry's decks are otherwise pitch
/// dark (`run::start` inserts `Lighting::dark()`), and a monster with no
/// `DarkSight` of its own, such as a line droid, sees nothing past what it
/// is touching outside of light, spec section 8.2's whole point for radar.
/// Returns the monster, then the player.
pub fn droid_facing_player(app: &mut App, name: &str, range: i32) -> (Entity, Entity) {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let roster = Roster::load(&registries);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let pos = app.world().get::<Position>(player).copied().expect("the player stands somewhere");
    let map = app.world().resource::<WorldMap>().current();
    let floor = app.world().resource::<WorldMap>().tile(pos.0).expect("the player's own tile is loaded");
    {
        let mut world_map = app.world_mut().resource_mut::<WorldMap>();
        for dx in 1..=range {
            world_map.set_tile(pos.0.offset(dx, 0), floor);
        }
    }
    let id = roster.defs.expect(name);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    commands.spawn((Position(pos.0), LightSource::new(255, range + 2, Rgb::WHITE)));
    let droid = crate::droids::spawn_monster(&mut commands, &roster, id, pos.0.offset(range, 0), map, &registries);
    queue.apply(app.world_mut());
    app.update();
    (droid, player)
}

/// What one `Struck` a monster caused looked like, for a test that only
/// cares about a monster's own attack rather than an item's economy.
pub struct DroidStruck {
    /// Whether it was a shot rather than a blow in reach.
    pub ranged: bool,
    /// Whom it hit.
    pub target: Entity,
}

/// Runs the app forward up to `max_turns` whole turns, waiting the player
/// forward one at a time so the scheduler moves past its held turn onto
/// whatever else is due, and watching for a `Struck` `attacker` caused,
/// returning it the moment one lands.
///
/// Panics if `attacker` never strikes within `max_turns`: a brain wired
/// wrong so it never shoots is a test failure, not a silent value the
/// caller might overlook.
pub fn run_until_struck(app: &mut App, attacker: Entity, max_turns: usize) -> DroidStruck {
    if !app.world().contains_resource::<StruckLog>() {
        app.init_resource::<StruckLog>();
        app.add_systems(PostUpdate, collect_struck);
    }
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    for _ in 0..max_turns {
        app.world_mut().write_message(Intent::new(player, Wait));
        app.update();
        let mut log = app.world_mut().resource_mut::<StruckLog>();
        if let Some(i) = log.0.iter().position(|s| s.attacker == attacker) {
            let struck = log.0.remove(i);
            return DroidStruck { ranged: struck.ranged, target: struck.target };
        }
        log.0.clear();
    }
    panic!("{attacker:?} never struck within {max_turns} turns");
}

/// A probe carrying `Alarm`, the player, and two line droids elsewhere on
/// the same deck that have noticed nothing, for a test of `sound_alarm`.
/// Returns the probe, then the player, then the two sleepers.
pub fn probe_and_sleepers(app: &mut App) -> (Entity, Entity, Vec<Entity>) {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let roster = Roster::load(&registries);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let pos = app.world().get::<Position>(player).copied().expect("the player stands somewhere");
    let map = app.world().resource::<WorldMap>().current();
    let probe_id = roster.defs.expect("probe droid");
    let line_id = roster.defs.expect("line droid");
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    let probe = crate::droids::spawn_monster(&mut commands, &roster, probe_id, pos.0, map, &registries);
    let a = crate::droids::spawn_monster(&mut commands, &roster, line_id, pos.0, map, &registries);
    let b = crate::droids::spawn_monster(&mut commands, &roster, line_id, pos.0, map, &registries);
    queue.apply(app.world_mut());
    (probe, player, vec![a, b])
}

/// Spawns `name` alone on the current deck, at no point that matters to the
/// caller.
pub fn lone_monster(app: &mut App, name: &str) -> Entity {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let roster = Roster::load(&registries);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let pos = app.world().get::<Position>(player).copied().expect("the player stands somewhere");
    let map = app.world().resource::<WorldMap>().current();
    let id = roster.defs.expect(name);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    let monster = crate::droids::spawn_monster(&mut commands, &roster, id, pos.0, map, &registries);
    queue.apply(app.world_mut());
    monster
}

/// Writes a `DamageDealt` of `kind` dealing `amount` straight to `target`,
/// bypassing combat entirely, and runs the turn that lets whatever reacts
/// to it react.
pub fn hit(app: &mut App, target: Entity, kind: &str, amount: i32) {
    let registries = app.world().resource::<Registries>().clone();
    let kind = registries.damage_kinds.expect(kind);
    app.world_mut().write_message(DamageDealt { target, hit: Hit::from_source(None, kind, amount), dealt: amount });
    app.update();
}

/// Warps the player onto `deck`'s entry and lets the arrival resolve; deck one needs nothing extra, since `run::start` warps there already.
pub fn arrive_on(app: &mut App, deck: u32) {
    (0..2).for_each(|_| app.update());
    if deck != 1 {
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: crate::decks::map_of(deck), arrive: Arrive::Entry } });
        (0..2).for_each(|_| app.update());
    }
}

/// Items on, or beside, every armory and store mark on the current deck: `OnMap` filters out an earlier deck's own scatter, and a store's item
/// counts against whichever of its two marks is nearest, never both.
pub fn items_at_marks(app: &mut App) -> (Vec<usize>, Vec<usize>) {
    let map = app.world().resource::<WorldMap>().current();
    let spots = app.world().resource::<WorldMap>().place(map).map(|p| p.spots.clone()).unwrap_or_default();
    let positions: Vec<Point> = {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(&Position, &OnMap), With<Item>>();
        q.iter(world).filter(|(_, on)| on.0 == map).map(|(p, _)| p.0).collect()
    };
    let by_tag = |c: char| spots.iter().filter(|s| s.tag == c as u32).map(|s| s.at).collect::<Vec<Point>>();
    let (armory_marks, store_marks) = (by_tag('A'), by_tag('L'));
    let armories = armory_marks.iter().map(|m| positions.iter().filter(|p| *p == m).count()).collect();
    let mut stores = vec![0usize; store_marks.len()];
    for p in &positions {
        let closest = store_marks.iter().enumerate().map(|(i, m)| (i, geometry::chebyshev(*p, *m))).filter(|(_, d)| *d <= 1).min_by_key(|(_, d)| *d);
        if let Some((i, _)) = closest {
            stores[i] += 1;
        }
    }
    (armories, stores)
}

/// Runs the same fight twice from `seed`: once where the dying actor carries drops (`heavy droid`) and once with none (`coolant rat`), returning
/// the health twenty follow-up melee blows took off a fresh target each time. The kill is a bare `DeathEvent`; the victim carries only `Kind`
/// and `OnMap`, never `Actor`, so only `drop_on_death` reading the wrong stream could shift the follow-up rolls.
pub fn combat_rolls_across_a_kill(seed: RunSeed) -> (Vec<i32>, Vec<i32>) {
    let run = |kind: &str| -> Vec<i32> {
        let mut app = headless(seed);
        app.update();
        app.update();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let pos = app.world().get::<Position>(player).copied().expect("the player stands somewhere");
        let (map, id) = (app.world().resource::<WorldMap>().current(), app.world().resource::<crate::droids::Roster>().defs.expect(kind));
        let victim = app.world_mut().spawn((crate::droids::Kind(id), OnMap(map))).id();
        app.world_mut().write_message(DeathEvent { entity: victim, at: pos.0, credit: None, was_player: false });
        app.update();
        let (at, floor) = (pos.0.offset(1, 0), app.world().resource::<WorldMap>().tile(pos.0).unwrap());
        app.world_mut().resource_mut::<WorldMap>().set_tile(at, floor);
        let target = app.world_mut().spawn((Blocks, Position(at), Health::full(1_000_000))).id();
        let (mut amounts, mut last) = (Vec::new(), 1_000_000);
        for _ in 0..20 {
            app.world_mut().write_message(Intent::new(player, Attack(target)));
            app.update();
            let now = app.world().get::<Health>(target).unwrap().current;
            amounts.push(last - now);
            last = now;
        }
        amounts
    };
    (run("heavy droid"), run("coolant rat"))
}

/// Waits the player forward `n` whole turns, one at a time.
pub fn pass_turns(app: &mut App, n: u32) {
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    for _ in 0..n {
        app.world_mut().write_message(Intent::new(player, Wait));
        app.update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_headless_run_wires_every_plugin_the_harness_names() {
        let _app = headless(RunSeed(7));
    }
}
