//! Where the time in one pass goes, and where the time in one sight recast
//! goes.
//!
//! `turns.rs` measured a player turn at about 110 microseconds per awake
//! mind, and `rl-ui`'s `frame.rs` measured a lit frame at about 10
//! microseconds per actor. Both are totals. These split them, because the
//! fixes the two halves want are nothing like each other:
//!
//! - a pass that is mostly schedule dispatch wants fewer systems run per
//!   actor, or a pass that is not the whole schedule;
//! - a pass that is mostly work wants the work made cheaper;
//! - a sight recast that is mostly shadowcast wants fewer recasts;
//! - one that is mostly the light gate wants the gate made incremental.
//!
//! Both groups strip the engine back rather than sampling it, so the
//! numbers are differences between builds rather than attributions inside
//! one, and they hold up under a different compiler or allocator.

use std::sync::Arc;

use bevy::prelude::*;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rl_bevy::prelude::*;
use rl_bevy::testing;
use rl_core::{DiceRoll, Grid2D};

/// How much engine is in the app.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Build {
    /// `CorePlugin` alone: the turn loop and nothing else. The floor.
    Bare,
    /// Every plugin a fighting game adds, with actors that carry no
    /// `Mind`, so every system is in the schedule and none of them has
    /// anything to do. The gap to `Bare` is what the schedule costs to
    /// dispatch per pass.
    Loaded,
    /// The same plugins, with minds. The gap to `Loaded` is the deciding.
    Thinking,
}

/// Spends the turn of whoever holds one, so a pass always completes.
///
/// Stands in for a game's input and for the minds in the builds that have
/// none: without it a pass deals a turn nobody takes and the loop stalls,
/// and what is being measured is a pass that ran.
fn wait_for_holder(mut waits: MessageWriter<Intent<Wait>>, holding: Query<Entity, With<MyTurn>>) {
    for e in &holding {
        waits.write(Intent::new(e, Wait));
    }
}

/// An app with `crowd` actors on a real streamed surface and no player, so
/// every pass is dealt to one of the crowd.
///
/// No player on purpose: with one, the loop stops the moment it holds a
/// turn, and the pass being measured would be the frame's last rather than
/// a typical one.
fn passes(crowd: usize, build: Build) -> App {
    let mut app = rl_bevy::plugin::headless_app();
    app.add_plugins(rl_bevy::world::StreamingPlugin);
    if build != Build::Bare {
        app.add_plugins((
            rl_bevy::fov::FovPlugin,
            rl_bevy::combat::CombatPlugin,
            rl_bevy::minds::MindsPlugin,
            rl_bevy::items::ItemsPlugin,
            rl_bevy::status::StatusPlugin,
        ));
    }
    let start = testing::surface(&mut app);
    let sides = testing::two_sides(&mut app);
    // In `Decide`, where a mind would decide, and claiming nothing: the
    // minds claim first when they are there, so this only picks up the
    // turns nobody else wanted.
    app.add_systems(rl_bevy::Turn, wait_for_holder.in_set(rl_bevy::TurnSet::Decide).after(rl_bevy::DecideSet::Minds));

    let brain = Arc::new(rl_rules::Brain::new().then(rl_rules::ai::tactics::MeleeAdjacent).then(rl_rules::ai::tactics::Hunt));
    let mut placed = 0;
    let mut radius: i32 = 2;
    while placed < crowd {
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                if placed >= crowd || (dx.abs() != radius && dy.abs() != radius) {
                    continue;
                }
                let mut actor = app.world_mut().spawn((Actor, Blocks, Position(start.offset(dx, dy))));
                if build != Build::Bare {
                    actor.insert((
                        Health::full(1_000_000),
                        Faction(sides.theirs),
                        Perception(10),
                        MeleeAttack::new(sides.kind, DiceRoll::flat(1)),
                        Viewshed::new(10),
                    ));
                }
                if build == Build::Thinking {
                    actor.insert(Mind(brain.clone()));
                }
                placed += 1;
            }
        }
        radius += 1;
    }

    app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
    // Enough frames to stream the window in and admit everyone. Each one
    // runs the loop to its per-frame bound, since nobody holds a turn to
    // stop it, so this is the expensive part of the setup and not of the
    // measurement.
    for _ in 0..4 {
        app.update();
    }
    app
}

/// One pass of the `Turn` schedule, run directly rather than through the
/// frame, so the number is one pass and not a frame's worth of them.
fn pass_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("one_pass");
    for crowd in [16usize, 64] {
        for (build, name) in [(Build::Bare, "bare"), (Build::Loaded, "loaded"), (Build::Thinking, "thinking")] {
            group.bench_with_input(BenchmarkId::new(name, crowd), &crowd, |b, &crowd| {
                let mut app = passes(crowd, build);
                b.iter(|| {
                    app.world_mut().run_schedule(rl_bevy::Turn);
                    black_box(());
                });
            });
        }
    }
    group.finish();
}

/// How many systems are in the `Turn` schedule of each build, printed once
/// so the dispatch numbers above have a denominator.
fn schedule_size(c: &mut Criterion) {
    for (build, name) in [(Build::Bare, "bare"), (Build::Loaded, "loaded"), (Build::Thinking, "thinking")] {
        let mut app = passes(4, build);
        let count = app.world_mut().resource_mut::<bevy::ecs::schedule::Schedules>().get(rl_bevy::Turn).map(|s| s.systems_len()).unwrap_or(0);
        println!("systems in the Turn schedule, {name}: {count}");
    }
    let _ = c;
}

/// One sight recast, split into its two halves.
///
/// `cast` is a shadowcast into the `line` grid and then, with lighting on,
/// a `gate` that walks that line and keeps what is lit. The lit frame's
/// cost is one `cast` per actor; which half it is decides the fix.
fn sight_cost(c: &mut Criterion) {
    let mut app = rl_bevy::plugin::headless_app();
    app.add_plugins((rl_bevy::fov::FovPlugin, rl_bevy::world::StreamingPlugin, rl_bevy::lighting::LightingPlugin));
    let start = testing::surface(&mut app);
    app.insert_resource(rl_bevy::seed::Seed(testing::TEST_SEED));
    let player = app
        .world_mut()
        .spawn((
            Actor,
            Player,
            Blocks,
            Position(start),
            Viewshed::new(12),
            RevealsMap,
            rl_bevy::lighting::LightSource::new(200, 12, rl_grid::Rgb::new(255, 220, 160)),
        ))
        .id();
    app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
    for _ in 0..4 {
        app.update();
    }

    let world = app.world();
    let map = world.resource::<rl_bevy::WorldMap>();
    let lighting = world.resource::<rl_bevy::lighting::Lighting>().clone();
    let view = map.view();
    let (w, h) = (view.width(), view.height());
    let origin = map.window_tiles().origin();
    let at = world.get::<Position>(player).expect("the player stands somewhere").0;
    let local = at - origin;
    let range = 12;

    let mut group = c.benchmark_group("one_sight_recast");
    // The shadowcast alone, into a grid the right size.
    let mut line = rl_grid::BitGrid::new(w, h);
    group.bench_function("shadowcast", |b| {
        b.iter(|| {
            rl_grid::fov::compute(&view, black_box(local), black_box(range), &mut line);
        });
    });
    // The gate alone, over the line the shadowcast just produced.
    let mut gated = Viewshed::new(range);
    gated.line = line.clone();
    gated.visible = rl_grid::BitGrid::new(w, h);
    gated.origin = origin;
    group.bench_function("light_gate", |b| {
        b.iter(|| {
            rl_bevy::lighting::gate(black_box(&lighting), black_box(at), 1, &mut gated);
        });
    });
    // Both, as `update_viewsheds` runs them.
    let mut whole = Viewshed::new(range);
    group.bench_function("both", |b| {
        b.iter(|| {
            rl_bevy::fov::cast(black_box(map), Some(&lighting), black_box(at), 1, None, &mut whole);
        });
    });
    group.finish();
}

criterion_group!(benches, schedule_size, pass_cost, sight_cost);
criterion_main!(benches);
