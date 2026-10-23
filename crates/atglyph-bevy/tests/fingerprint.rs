//! A seeded run of minds, combat, stealth and items, hashed.
//!
//! A fingerprint tripwire, not a property: the number below is what this
//! run came to on the day it was written, and the test fails when any
//! change moves it. That is the point. The engine promises that a run is
//! its seed and the player's keys, and this is the one test that turns a
//! quiet change to the order of a roll, a decision or a query into a red
//! test rather than a replay that drifts in someone's bug report. A
//! change that means to move it re-baselines the number and says so in
//! the changelog, since every recording made before it will not replay.

use std::sync::Arc;

use atglyph_bevy::prelude::*;
use atglyph_bevy::testing::{Sides, ground, two_sides};
use atglyph_core::{DiceRoll, RunSeed, geometry};
use atglyph_rules::Brain;
use atglyph_rules::ai::awareness::{NoticeStats, StealthStats};
use atglyph_rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, SearchLastKnown, Wander};
use bevy::prelude::*;

/// Folds one value into a running FNV-1a hash.
fn fold(hash: &mut u64, value: i64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// The world as the fingerprint reads it: the clock, then every actor's
/// place and health in the order of where they stand, row then column.
///
/// Where they stand rather than which entity they are: Bevy numbers every
/// entity from one allocator, which counts whatever is spawned anywhere
/// and hands a freed number to the next spawn, so a change to when
/// anything unrelated is made renumbers every actor after it, and that is
/// not a change to the run. Two that block cannot share a cell; any that
/// do are put after by their health, and two alike in all three are
/// folded the same whichever comes first.
fn fingerprint(app: &mut App) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    fold(&mut hash, i64::from(app.world().resource::<Turns>().now()));
    let world = app.world_mut();
    let mut actors: Vec<(i32, i32, i32)> = world.query::<(&Position, &Health)>().iter(world).map(|(p, h)| (p.0.y, p.0.x, h.current)).collect();
    actors.sort();
    for (rank, (y, x, hp)) in actors.into_iter().enumerate() {
        fold(&mut hash, rank as i64);
        fold(&mut hash, i64::from(x));
        fold(&mut hash, i64::from(y));
        fold(&mut hash, i64::from(hp));
    }
    hash
}

/// A player that strikes what stands beside it and waits otherwise, in a
/// field of hunters that notice, flee, search and drift.
fn run(seed: u64, turns: usize) -> u64 {
    let mut app = atglyph_bevy::testing::headless_app();
    app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, StealthPlugin, ItemsPlugin));
    let start = ground(&mut app);
    let Sides { ours, theirs, kind } = two_sides(&mut app);
    app.insert_resource(Seed(RunSeed(seed)));
    let player = app
        .world_mut()
        .spawn((
            (Actor, Player, Blocks, Position(start), Viewshed::new(10), RevealsMap),
            (Health::full(400), Armor(1), Faction(ours), MeleeAttack::new(kind, DiceRoll::new(1, 6)), Stealth(StealthStats { quiet: 1, subtlety: 10 })),
        ))
        .id();
    let brain = Arc::new(Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { at_pct: 30 }).then(Hunt).then(SearchLastKnown).then(Wander { chance_pct: 40 }));
    for (i, (dx, dy)) in [(4, 0), (-5, 2), (0, 6), (7, -3), (-3, -6), (6, 6)].into_iter().enumerate() {
        app.world_mut().spawn((
            (Actor, Blocks, Position(start.offset(dx, dy)), Health::full(12 + i as i32), Armor(0), Faction(theirs)),
            (MeleeAttack::new(kind, DiceRoll::new(1, 4)), Perception(8), Speed(90 + 10 * i as u32), Mind(brain.clone())),
            Notice(NoticeStats { certain: 2, chance_pct: 30, lit_bonus: 0, memory: 5 }),
        ));
    }
    app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
    app.update();
    for _ in 0..turns {
        if app.world().get::<MyTurn>(player).is_some() {
            let me = app.world().get::<Position>(player).unwrap().0;
            let world = app.world_mut();
            let beside = world
                .query_filtered::<(Entity, &Position), (With<Mind>, Without<Dead>)>()
                .iter(world)
                .find(|(_, p)| geometry::is_adjacent(me, p.0))
                .map(|(e, _)| e);
            match beside {
                Some(foe) => {
                    world.write_message(Intent::new(player, Attack(foe)));
                }
                None => {
                    world.write_message(Intent::new(player, Wait));
                }
            }
        }
        app.update();
    }
    let hurt = {
        let world = app.world_mut();
        world.query::<&Health>().iter(world).filter(|h| h.current < h.max).count()
    };
    assert!(app.world().resource::<Turns>().now() >= 100 * turns as u32, "the clock ran");
    assert!(hurt > 0, "something was struck, so the fingerprint is of a fight and not of a frozen field");
    fingerprint(&mut app)
}

/// Fingerprint tripwire: the same seed and the same keys come to the
/// same run, and this run comes to this number. Re-baseline deliberately.
#[test]
fn a_seeded_run_of_minds_combat_and_stealth_comes_to_the_same_run_every_time() {
    let first = run(7, 120);
    assert_eq!(first, run(7, 120), "one seed, two runs, one fingerprint");
    assert_ne!(first, run(8, 120), "another seed is another run");
    assert_eq!(
        first, 15_309_660_172_097_327_494,
        "fingerprint tripwire: a change moved a roll, a decision or an order; re-baseline on purpose and say so in the changelog"
    );
}
