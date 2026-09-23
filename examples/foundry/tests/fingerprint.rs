//! A seeded Foundry run, driven by a fixed script and hashed.
//!
//! A fingerprint tripwire, not a property: the number below is what this
//! run came to on the day it was written, and the test fails when any
//! change moves it. Nothing here has a property to state. It pins the
//! whole game at once, the deck the seed builds, the droids and loot it
//! spawns, how they hunt and shoot, and how a blaster heats and cools, so
//! a quiet change to the order of a roll, a spawn or a system is a red
//! test rather than a run that plays differently in someone's hands. A
//! change that means to move it re-baselines the number on purpose and
//! says so in a `CHANGELOG.md` line.

use bevy::prelude::*;
use foundry::heat::Heat;
use rl_engine::prelude::*;
use rl_engine::rl_core::{Direction, RunSeed};

/// Folds one value into a running FNV-1a hash.
fn fold(hash: &mut u64, value: i64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// The run as the fingerprint reads it: the clock, then every actor's
/// place and health, then every weapon's heat, each in spawn order.
///
/// Spawn order rather than the entity index itself: systems are entities
/// too, so adding one anywhere shifts every index after it, and that is
/// not a change to the run.
fn fingerprint(app: &mut App) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    fold(&mut hash, i64::from(app.world().resource::<Turns>().now()));
    let world = app.world_mut();
    let mut actors: Vec<(u32, Point, i32)> =
        world.query::<(Entity, &Position, &Health)>().iter(world).map(|(e, p, h)| (e.index().index(), p.0, h.current)).collect();
    actors.sort();
    for (rank, (_, at, hp)) in actors.into_iter().enumerate() {
        fold(&mut hash, rank as i64);
        fold(&mut hash, i64::from(at.x));
        fold(&mut hash, i64::from(at.y));
        fold(&mut hash, i64::from(hp));
    }
    let mut heats: Vec<(u32, u32, bool)> = world.query::<(Entity, &Heat)>().iter(world).map(|(e, h)| (e.index().index(), h.now, h.locked)).collect();
    heats.sort();
    for (rank, (_, now, locked)) in heats.into_iter().enumerate() {
        fold(&mut hash, rank as i64);
        fold(&mut hash, i64::from(now));
        fold(&mut hash, i64::from(locked));
    }
    hash
}

/// The walk the script takes when there is nothing to shoot: a slow
/// square, so the commando covers ground and turns corners.
const WALK: [Direction; 8] =
    [Direction::East, Direction::East, Direction::South, Direction::South, Direction::West, Direction::West, Direction::North, Direction::North];

/// A commando handed a hand blaster, and health enough to last the run,
/// who shoots the nearest droid in sight whenever there is one and walks
/// the square otherwise, for `turns` of its own turns.
///
/// The gun is handed over here rather than come by honestly: a real run
/// starts empty-handed and may walk deck one without finding a weapon at
/// all, and a fingerprint of a walk pins nothing about combat.
fn run(seed: u64, turns: usize) -> u64 {
    let mut app = foundry::testing::headless(RunSeed(seed));
    foundry::testing::settle(&mut app);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    foundry::testing::equip_new(&mut app, player, "hand blaster");
    app.world_mut().entity_mut(player).insert(Health::full(400));
    let mut taken = 0;
    let mut shots = 0;
    for _ in 0..turns * 4 {
        if taken == turns {
            break;
        }
        if app.world().get::<MyTurn>(player).is_some() {
            let world = app.world_mut();
            let me = world.get::<Position>(player).unwrap().0;
            let sight = world.get::<Viewshed>(player).unwrap().clone();
            let target = world
                .query_filtered::<(Entity, &Position), (With<Mind>, Without<Dead>)>()
                .iter(world)
                .filter(|(_, p)| sight.can_see(p.0))
                .min_by_key(|(e, p)| (geometry::chebyshev(me, p.0), e.index().index()))
                .map(|(e, _)| e);
            match target {
                Some(foe) => {
                    shots += 1;
                    world.write_message(Intent::new(player, Attack(foe)));
                }
                None => {
                    world.write_message(Intent::new(player, Bump(WALK[taken % WALK.len()])));
                }
            }
            taken += 1;
        }
        app.update();
    }
    assert_eq!(taken, turns, "the commando was dealt every one of its turns");
    assert!(shots > 0, "the script fired, so the fingerprint is of a fight and not of a walk");
    fingerprint(&mut app)
}

/// Fingerprint tripwire: the same seed and the same script come to the
/// same run, and this run comes to this number. Re-baseline deliberately,
/// with a `CHANGELOG.md` line.
#[test]
fn fingerprint_tripwire_a_scripted_two_hundred_turn_run_on_seed_seven_comes_to_the_same_run_every_time() {
    let first = run(7, 200);
    assert_eq!(first, run(7, 200), "one seed, two runs, one fingerprint");
    assert_ne!(first, run(8, 200), "another seed is another run");
    assert_eq!(
        first, 3_340_263_862_586_230_787,
        "fingerprint tripwire: a change moved a roll, a spawn or an order; re-baseline on purpose and say so in the changelog"
    );
}
