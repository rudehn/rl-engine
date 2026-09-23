//! Benchmarks on the loops above tier 1: one player turn with a crowd of
//! minds awake, which is the frame a game actually spends its time in.
//!
//! `rl-grid`'s benches measure the algorithms the passes call; these
//! measure the passes. The number that matters is the cost of one player
//! turn as the crowd grows: every awake mind gets a pass of its own before
//! the player is dealt another turn, and each pass opens a snapshot that
//! every contributor fills. If that cost is linear in the crowd the engine
//! scales; if it is quadratic, it does not, and the perceive stage is
//! where to look.
//!
//! Headless throughout: `MinimalPlugins`, no window, no renderer, and
//! nothing watching the cues, so no pass waits to be seen.

use std::sync::Arc;

use bevy::prelude::*;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rl_bevy::prelude::*;
use rl_bevy::testing;
use rl_core::DiceRoll;

/// An app with the engine's loops, a real streamed surface, and `crowd`
/// hunters awake around the player.
///
/// The hunters are spread over a disc around the player rather than in a
/// line, so they see each other as well as the player and the roster each
/// one builds is the roster a real fight builds. They are out of reach of
/// the player at the start, so a run of the loop is spent deciding and
/// stepping rather than resolving blows, which is the part that scales
/// with the crowd.
fn awake(crowd: usize) -> (App, Entity) {
    let mut app = rl_bevy::plugin::headless_app();
    app.add_plugins((
        rl_bevy::fov::FovPlugin,
        rl_bevy::world::StreamingPlugin,
        rl_bevy::combat::CombatPlugin,
        rl_bevy::minds::MindsPlugin,
        rl_bevy::items::ItemsPlugin,
    ));
    let start = testing::surface(&mut app);
    let sides = testing::two_sides(&mut app);

    let player = app
        .world_mut()
        .spawn((
            Actor,
            Player,
            Blocks,
            Position(start),
            Viewshed::new(12),
            RevealsMap,
            Health::full(1_000_000),
            Faction(sides.ours),
            MeleeAttack::new(sides.kind, DiceRoll::flat(1)),
        ))
        .id();

    // A ring at radius five and outward, one per cell, so nobody starts
    // adjacent and nobody starts stacked.
    let brain = Arc::new(rl_rules::Brain::new().then(rl_rules::ai::tactics::MeleeAdjacent).then(rl_rules::ai::tactics::Hunt));
    let mut placed = 0;
    let mut radius: i32 = 5;
    while placed < crowd {
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                if placed >= crowd || (dx.abs() != radius && dy.abs() != radius) {
                    continue;
                }
                app.world_mut().spawn((
                    Actor,
                    Blocks,
                    Position(start.offset(dx, dy)),
                    Health::full(1_000),
                    Faction(sides.theirs),
                    Perception(10),
                    MeleeAttack::new(sides.kind, DiceRoll::flat(1)),
                    Mind(brain.clone()),
                ));
                placed += 1;
            }
        }
        radius += 1;
    }

    app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
    // Let the window stream in and everyone be admitted and dealt a first
    // turn, so the measured frames are steady-state frames.
    for _ in 0..8 {
        if app.world().get::<MyTurn>(player).is_some() {
            app.world_mut().write_message(Intent::new(player, Wait));
        }
        app.update();
    }
    (app, player)
}

/// One player turn: wait, and run frames until the player holds a turn
/// again, which is every awake mind having had one.
fn one_player_turn(app: &mut App, player: Entity) {
    app.world_mut().write_message(Intent::new(player, Wait));
    for _ in 0..64 {
        app.update();
        if app.world().get::<MyTurn>(player).is_some() {
            return;
        }
    }
}

fn turn_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("one_player_turn");
    for crowd in [1usize, 8, 32, 64, 128] {
        group.bench_with_input(BenchmarkId::from_parameter(crowd), &crowd, |b, &crowd| {
            let (mut app, player) = awake(crowd);
            b.iter(|| one_player_turn(black_box(&mut app), player));
        });
    }
    group.finish();
}

/// One player turn with the crowd fixed and the floor loaded with things
/// nobody is going to pick up.
///
/// The direct test of whether the perceive stage's full-world scans cost
/// what reading the code suggests: every contributor iterates every actor,
/// every prop and every item on the ground once per pass, filtering
/// afterwards by what the mind can see. With the minds fixed, the passes
/// per turn are fixed too, so anything this adds is the scan and nothing
/// else.
fn crowded_floor(c: &mut Criterion) {
    let mut group = c.benchmark_group("one_player_turn_with_litter");
    for litter in [0usize, 500, 2000] {
        group.bench_with_input(BenchmarkId::from_parameter(litter), &litter, |b, &litter| {
            let (mut app, player) = awake(16);
            let start = app.world().get::<Position>(player).expect("the player stands somewhere").0;
            for i in 0..litter {
                let (dx, dy) = ((i % 40) as i32 - 20, (i / 40) as i32 - 20);
                app.world_mut().spawn((rl_bevy::Item, Position(start.offset(dx, dy)), Name::new("a bolt")));
            }
            app.update();
            b.iter(|| one_player_turn(black_box(&mut app), player));
        });
    }
    group.finish();
}

/// What a veiling gas costs a turn.
///
/// `step_gases` rewrites the map's veil on every whole turn, and any
/// change to it moves `WorldMap::opacity_epoch`. Two readers treat that
/// epoch as "everything is invalid": every viewshed recasts, and
/// `update_lighting` rebuilds both light layers from every emitter, which
/// is the one path the bounded invalidation added the same day does not
/// narrow.
///
/// Three builds, same crowd: no gas plugin at all, the plugin with no gas
/// in the air, and a vent filling the room with smoke thick enough to
/// veil. The gap between the last two is the epoch.
fn smoke(c: &mut Criterion) {
    use rl_rules::{GasDef, Registry};

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Air {
        None,
        Clear,
        /// Thick gas that never veils: the diffusion, with no epoch bump.
        Thick,
        Smoky,
    }

    fn built(air: Air, lit: bool) -> (App, Entity) {
        let crowd = 32;
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins((
            rl_bevy::fov::FovPlugin,
            rl_bevy::world::StreamingPlugin,
            rl_bevy::combat::CombatPlugin,
            rl_bevy::minds::MindsPlugin,
            rl_bevy::items::ItemsPlugin,
            rl_bevy::status::StatusPlugin,
        ));
        if lit {
            app.add_plugins(rl_bevy::lighting::LightingPlugin);
        }
        if air != Air::None {
            app.add_plugins(rl_bevy::gas::GasPlugin);
        }
        let start = testing::surface(&mut app);
        let sides = testing::two_sides(&mut app);
        let def = match air {
            Air::Thick => GasDef::new("smoke").spread(60).fade(1),
            _ => GasDef::new("smoke").spread(60).fade(1).veils_at(40),
        };
        let gases = Registry::from_defs(vec![def]).expect("one gas");
        let smoke = gases.expect("smoke");
        app.world_mut().resource_mut::<Registries>().gases = gases;

        let player = app
            .world_mut()
            .spawn((
                (Actor, Player, Blocks, Position(start), Viewshed::new(12), RevealsMap),
                (Health::full(1_000_000), Faction(sides.ours), MeleeAttack::new(sides.kind, DiceRoll::flat(1))),
            ))
            .id();
        if lit {
            app.world_mut().entity_mut(player).insert(rl_bevy::lighting::LightSource::new(200, 10, rl_grid::Rgb::new(255, 220, 160)));
        }
        if air == Air::Smoky || air == Air::Thick {
            // Four vents around the player, so the cloud is large, moves
            // every turn and never settles.
            for (dx, dy) in [(4, 0), (-4, 0), (0, 4), (0, -4)] {
                app.world_mut().spawn((Position(start.offset(dx, dy)), rl_bevy::gas::Vents { gas: smoke, amount: 200 }));
            }
        }
        let brain = std::sync::Arc::new(rl_rules::Brain::new().then(rl_rules::ai::tactics::Wander { chance_pct: 0 }));
        let mut placed = 0;
        let mut radius: i32 = 6;
        while placed < crowd {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if placed >= crowd || (dx.abs() != radius && dy.abs() != radius) {
                        continue;
                    }
                    app.world_mut().spawn((
                        (Actor, Blocks, Position(start.offset(dx, dy)), Viewshed::new(10)),
                        (Health::full(1_000), Faction(sides.theirs), Perception(10), Mind(brain.clone())),
                    ));
                    placed += 1;
                }
            }
            radius += 1;
        }
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        for _ in 0..12 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
        }
        (app, player)
    }

    let mut group = c.benchmark_group("one_player_turn_in_smoke");
    for lit in [false, true] {
        for (air, name) in [(Air::None, "no_gas_plugin"), (Air::Clear, "clear_air"), (Air::Thick, "thick_gas_no_veil"), (Air::Smoky, "thick_smoke_veiling")] {
            let label = format!("{}/{name}", if lit { "lit" } else { "unlit" });
            group.bench_function(BenchmarkId::from_parameter(label), |b| {
                let (mut app, player) = built(air, lit);
                b.iter(|| one_player_turn(black_box(&mut app), player));
            });
        }
    }
    group.finish();
}

criterion_group!(benches, turn_loop, crowded_floor, smoke);
criterion_main!(benches);
