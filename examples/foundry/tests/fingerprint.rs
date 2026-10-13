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
/// place and health in the order of where they stand, by deck, row and
/// column, then every weapon's heat in the order of where it is.
///
/// Where they stand rather than which entity they are: Bevy numbers every
/// entity from one allocator, which counts whatever is spawned anywhere
/// and hands a freed number to the next spawn, so a light made a moment
/// earlier or a wreck left a turn later renumbers every droid after it,
/// and that is not a change to the run. Two things that block cannot
/// share a cell; anything else that can is put after by its health, and
/// two alike in every part are folded the same whichever comes first.
fn fingerprint(app: &mut App) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    fold(&mut hash, i64::from(app.world().resource::<Turns>().now()));
    let world = app.world_mut();
    let mut actors: Vec<(Option<u32>, i32, i32, i32)> =
        world.query::<(&Position, Option<&OnMap>, &Health)>().iter(world).map(|(p, on, h)| (on.map(|on| on.0.0), p.0.y, p.0.x, h.current)).collect();
    actors.sort();
    for (rank, (map, y, x, hp)) in actors.into_iter().enumerate() {
        fold(&mut hash, rank as i64);
        fold(&mut hash, map.map_or(-1, i64::from));
        fold(&mut hash, i64::from(x));
        fold(&mut hash, i64::from(y));
        fold(&mut hash, i64::from(hp));
    }
    let mut heats: Vec<(Place, u32, bool)> = world.query::<(Entity, &Heat)>().iter(world).map(|(e, h)| (place(world, e), h.now, h.locked)).collect();
    heats.sort();
    for (rank, ((map, y, x, held, slot), now, locked)) in heats.into_iter().enumerate() {
        fold(&mut hash, rank as i64);
        for part in [map.map_or(-1, i64::from), i64::from(x), i64::from(y), i64::from(held), slot as i64] {
            fold(&mut hash, part);
        }
        fold(&mut hash, i64::from(now));
        fold(&mut hash, i64::from(locked));
    }
    hash
}

/// Where a thing is, as the fingerprint orders things: the deck, the row
/// and the column of the cell it lies on or its holder stands on, then
/// whether it lies there (0), is worn (1) or is carried (2), then the
/// slot it is worn in or its place in the pack. Two weapons lying on one
/// cell tie, and are put in order by their heat.
type Place = (Option<u32>, i32, i32, u8, usize);

/// Where `item` is: see [`Place`]. One held by nobody and lying nowhere
/// is put last.
fn place(world: &World, item: Entity) -> Place {
    let cell = |e: Entity| {
        let at = world.get::<Position>(e).map_or(Point::new(i32::MAX, i32::MAX), |p| p.0);
        (world.get::<OnMap>(e).map(|on| on.0.0), at.y, at.x)
    };
    if world.get::<Position>(item).is_some() {
        let (map, y, x) = cell(item);
        return (map, y, x, 0, 0);
    }
    for holder in world.iter_entities() {
        if let Some(slot) = holder.get::<Equipped>().and_then(|worn| worn.slot_of(item)) {
            let (map, y, x) = cell(holder.id());
            return (map, y, x, 1, slot.index());
        }
    }
    for holder in world.iter_entities() {
        if let Some(i) = holder.get::<Inventory>().and_then(|bag| bag.items.iter().position(|e| *e == item)) {
            let (map, y, x) = cell(holder.id());
            return (map, y, x, 2, i);
        }
    }
    (Some(u32::MAX), i32::MAX, i32::MAX, 3, 0)
}

/// Which of two offsets from the commando comes first going clockwise
/// from due north: the east half, north itself included, before the west
/// half, and within a half the one the other lies clockwise of.
fn clockwise(a: Point, b: Point) -> std::cmp::Ordering {
    let half = |p: Point| u8::from(!(p.x > 0 || (p.x == 0 && p.y < 0)));
    half(a).cmp(&half(b)).then_with(|| 0.cmp(&(i64::from(a.x) * i64::from(b.y) - i64::from(a.y) * i64::from(b.x))))
}

/// The walk the script takes when there is nothing to shoot: a slow
/// square, so the commando covers ground and turns corners.
const WALK: [Direction; 8] =
    [Direction::East, Direction::East, Direction::South, Direction::South, Direction::West, Direction::West, Direction::North, Direction::North];

/// A commando handed a hand blaster, and health enough to last the run,
/// who shoots the nearest droid in sight whenever there is one and walks
/// the square otherwise, for `turns` of its own turns. Of two droids as
/// near, the first clockwise from north, so which one is shot is a fact
/// about the deck rather than about the order they were spawned in; two
/// as near in the same direction are the same cell, which two droids
/// cannot share.
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
                .map(|(e, p)| (e, Point::new(p.0.x - me.x, p.0.y - me.y)))
                .min_by(|(_, a), (_, b)| {
                    geometry::chebyshev(Point::new(0, 0), *a).cmp(&geometry::chebyshev(Point::new(0, 0), *b)).then_with(|| clockwise(*a, *b))
                })
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
///
/// Seed five since the decks were grown: on nine's deck no droid comes
/// into the commando's sight inside two hundred turns, and a seed that
/// never fires pins a walk rather than a fight. Six is the other run for
/// the same reason, since it fires too.
#[test]
fn fingerprint_tripwire_a_scripted_two_hundred_turn_run_on_seed_five_comes_to_the_same_run_every_time() {
    let first = run(5, 200);
    assert_eq!(first, run(5, 200), "one seed, two runs, one fingerprint");
    assert_ne!(first, run(6, 200), "another seed is another run");
    assert_eq!(
        first, 10_917_675_082_181_141_982,
        "fingerprint tripwire: a change moved a roll, a spawn or an order; re-baseline on purpose and say so in the changelog"
    );
}
