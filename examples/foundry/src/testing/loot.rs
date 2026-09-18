//! What a deck scatters, and what a kill drops, for a test to arrange and
//! then read back.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{Grid2D, Point, RunSeed};
use rl_engine::rl_rules::Hit;

/// Warps the player onto `deck`'s entry and lets the arrival, and
/// whatever it seeds or scatters, resolve. Deck one needs nothing beyond
/// the two updates every other helper here opens with: `run::start`
/// warps there the moment the run begins.
pub fn arrive_on(app: &mut App, deck: u32) {
    app.update();
    app.update();
    if deck != 1 {
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: crate::decks::map_of(deck), arrive: Arrive::Entry } });
        app.update();
        app.update();
    }
}

/// How many items sit on every armory mark, and beside every store mark,
/// on the current deck: armories first, then stores, each in mark order.
///
/// A real store's own two `L`s can sit close enough
/// (`decks::Foundry::stores`) that a tile beside one also falls within a
/// step of the other, so guessing an item's mark from where it landed can
/// credit the wrong one. Recomputes `scatter_on_arrival`'s own plan
/// instead, from the same seed, the same `foundry.scatter` stream and the
/// same terrain it already used, and reads each item's origin off that:
/// `Seed::stream` is a pure function of the seed, the domain and the
/// index, so calling it again here reproduces the exact draws
/// `scatter_on_arrival` made, without needing a plan the game keeps
/// around. Each count is then checked against what is actually on the
/// ground, so a bug in `scatter_on_arrival` itself (spawning something
/// other than what it planned) still shows up as a mismatch here.
pub fn items_at_marks(app: &mut App) -> (Vec<usize>, Vec<usize>) {
    let map = app.world().resource::<WorldMap>().current();
    let deck = crate::decks::deck_of(map);
    let seed = *app.world().resource::<Seed>();
    let registries = app.world().resource::<Registries>().clone();
    let armory = crate::gear::Armory::load(&registries);
    let positions: Vec<Point> = {
        let world = app.world_mut();
        // `OnMap` too, not just `Item`: an earlier deck's own scatter is
        // still alive, off to one side, once the run has moved past it.
        let mut q = world.query_filtered::<(&Position, &OnMap), With<Item>>();
        q.iter(world).filter(|(_, on)| on.0 == map).map(|(p, _)| p.0).collect()
    };
    let wm = app.world().resource::<WorldMap>();
    let place = wm.place(map).expect("the current deck is built");
    let armories: Vec<Point> = place.spots.iter().filter(|s| s.tag == 'A' as u32).map(|s| s.at).collect();
    let stores: Vec<Point> = place.spots.iter().filter(|s| s.tag == 'L' as u32).map(|s| s.at).collect();
    let layout = crate::loot::Layout { bounds: place.terrain.bounds(), armories: &armories, stores: &stores };
    let mut rng = seed.stream(b"foundry.scatter", deck as u64);
    let plan = crate::loot::plan_scatter(&armory.table, deck as i32, &layout, crate::loot::extra_loose_items(deck), &mut |p| wm.is_walkable(p), &mut rng);
    let count_origin = |o: crate::loot::Origin| plan.iter().filter(|(_, p, at)| *at == o && positions.contains(p)).count();
    let armories = (0..armories.len()).map(|i| count_origin(crate::loot::Origin::Armory(i))).collect();
    let stores = (0..stores.len()).map(|i| count_origin(crate::loot::Origin::Store(i))).collect();
    (armories, stores)
}

/// Runs the same fight twice from `seed`: once where the actor that dies
/// carries drops (`heavy droid`, two non-empty rows) and once where it
/// carries none (`coolant rat`, none at all), and returns the sequence of
/// damage amounts after the kill, for each run in turn.
///
/// The two monsters differ in every stat there is; that is the point. The
/// kill itself is written as a bare `DeathEvent` rather than a real blow
/// rolled through combat, since nothing about reaching zero health should
/// ever touch `CombatRng` here: the only thing left that could shift the
/// follow-up rolls is `drop_on_death` (`crate::loot::drop_on_death`)
/// drawing from the wrong stream. The dying actor carries only `Kind` and
/// `OnMap`, never `Actor`, so it can never be dealt a turn of its own to
/// blur the comparison with a move or a swing neither run should have.
///
/// [`kill_with_a_guaranteed_drop`] is the other half of this file's kill
/// tests: a real kill, through the engine's own damage and death path,
/// for a test that cares about where an item lands rather than about the
/// stream it was rolled from.
pub fn combat_rolls_across_a_kill(seed: RunSeed) -> (Vec<i32>, Vec<i32>) {
    let run = |kind: &str| -> Vec<i32> {
        let mut app = super::headless(seed);
        app.update();
        app.update();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let pos = app.world().get::<Position>(player).copied().expect("the player stands somewhere");
        let map = app.world().resource::<WorldMap>().current();
        let id = app.world().resource::<crate::droids::Roster>().defs.expect(kind);
        let victim = app.world_mut().spawn((crate::droids::Kind(id), OnMap(map))).id();
        app.world_mut().write_message(DeathEvent { entity: victim, at: pos.0, credit: None, was_player: false });
        app.update();
        fire_at_an_adjacent_unkillable_target(&mut app, player, 20)
    };
    (run("heavy droid"), run("coolant rat"))
}

/// Health `attacker` takes off a fresh, adjacent, effectively unkillable
/// target with each of `shots` melee blows, in order: adjacent so it
/// needs no ranged weapon the way [`super::fire_at_a_target`] does.
fn fire_at_an_adjacent_unkillable_target(app: &mut App, attacker: Entity, shots: usize) -> Vec<i32> {
    let pos = app.world().get::<Position>(attacker).copied().expect("the attacker stands somewhere");
    let at = pos.0.offset(1, 0);
    let floor = app.world().resource::<WorldMap>().tile(pos.0).expect("the attacker's own tile is loaded");
    app.world_mut().resource_mut::<WorldMap>().set_tile(at, floor);
    let target = app.world_mut().spawn((Blocks, Position(at), Health::full(1_000_000))).id();
    let mut amounts = Vec::new();
    let mut last = 1_000_000;
    for _ in 0..shots {
        app.world_mut().write_message(Intent::new(attacker, Attack(target)));
        app.update();
        let now = app.world().get::<Health>(target).unwrap().current;
        amounts.push(last - now);
        last = now;
    }
    amounts
}

/// Spawns a monster from a roster of one kind, made up for this call
/// alone with a guaranteed (100%) drop of `item`, and kills it with a
/// real `DamageEvent` through the engine's own damage and death path
/// (`apply_damage`, `process_deaths`, `bury_the_dead`) rather than a
/// hand-written `DeathEvent`: the property this proves is that a real
/// kill's drop lands where the kill actually happened, which a
/// synthetic death event could get right by construction and still say
/// nothing about. Returns where it died.
///
/// `Roster::from_ron` exists for exactly this: `monsters.ron` names no
/// monster at 100%, since the real roster's own rates are the design's,
/// not a test's to bend.
pub fn kill_with_a_guaranteed_drop(app: &mut App, item: &str) -> Point {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let ron = format!(
        "[(name: \"test target\", glyph: 'x', color: (1.0, 1.0, 1.0), hp: 10, armor: 0, \
         profile: \"chassis\", faction: \"droids\", wits: \"mindless\", perception: 1, \
         melee: (roll: \"1d1\", kind: \"kinetic\"), speed: 100, flee_at: 0, spawn: [(1, 99, 1, 1, 1)], \
         drops: [(\"{item}\", 100)])]"
    );
    let roster = crate::droids::Roster::from_ron(&ron, &registries);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let pos = app.world().get::<Position>(player).copied().expect("the player stands somewhere");
    let map = app.world().resource::<WorldMap>().current();
    let id = roster.defs.expect("test target");
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    let monster = crate::droids::spawn_monster(&mut commands, &roster, id, pos.0, map, &registries);
    queue.apply(app.world_mut());
    app.insert_resource(roster);
    let kinetic = registries.damage_kinds.expect("kinetic");
    app.world_mut().write_message(DamageEvent { target: monster, hit: Hit::by(player, kinetic, 999) });
    app.update();
    pos.0
}
