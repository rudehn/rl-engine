//! Benchmarks on the frame, as `rl-bevy`'s are on the turn.
//!
//! A turn is what the loop costs; a frame is what the window costs, and
//! they are different questions. A frame recasts the light, recasts every
//! stale viewshed, refills every view and repaints every cell, and none of
//! that had a number before this file. The three claims it exists to
//! settle, all of them read off the code in `docs/TODO.md` rather than off
//! a profile:
//!
//! - a moving light marks every viewshed in the world stale, so one actor
//!   carrying a torch costs every other actor a shadowcast;
//! - the collectors of screens nobody has open run anyway;
//! - the terminal's cost is the cells that changed.
//!
//! Headless, with a real `Terminal` that nothing flushes to a window:
//! `flush_terminal` is Bevy's own sprite and text work and is not what
//! these measure. What they measure is everything up to it.

use bevy::prelude::*;
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rl_bevy::prelude::*;
use rl_core::Rect;
use rl_grid::{Light, Rgb};
use rl_render::{Glyph, MapViewPlugin, Terminal};
use rl_ui::{LogPanel, NearbyPanel, UiPlugin, VitalsPanel};

/// How the screen is cut up, the way a game cuts it.
const SCREEN: Rect = Rect { x: 0, y: 0, width: 100, height: 40 };

/// What goes in the app besides the engine: none, the panels a game always
/// shows, or those plus the screens a game opens now and then.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Panels {
    /// The map alone, so a frame is the loops and the drawing and no views.
    None,
    /// The rail a game always has up: vitals, nearby, log.
    Always,
    /// Those, plus the four screen-backed views whose collectors run
    /// whether or not their screen is open.
    WithScreens,
}

/// An app drawing a frame, with `crowd` actors standing in the player's
/// sight, lit or not.
fn drawing(crowd: usize, lit: bool, panels: Panels) -> (App, Entity) {
    drawing_spread(crowd, lit, panels, false)
}

/// The same, with `spread` scattering the crowd across the window instead
/// of huddling it around the player.
///
/// Both are worth measuring and they answer different questions. A huddle
/// is a fight, where every actor is inside the player's lamp anyway; a
/// spread is a level, where most actors are nowhere near it. What a bound
/// on the light's invalidation can save is exactly the difference.
fn drawing_spread(crowd: usize, lit: bool, panels: Panels, spread: bool) -> (App, Entity) {
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
    app.add_plugins((UiPlugin, MapViewPlugin::new(SCREEN)));
    match panels {
        Panels::None => {}
        Panels::Always => {
            app.add_plugins((VitalsPanel::new(Rect::new(0, 0, 100, 1)), NearbyPanel::new(Rect::new(76, 1, 24, 30)), LogPanel::new(Rect::new(0, 35, 76, 5))));
        }
        Panels::WithScreens => {
            app.add_plugins((VitalsPanel::new(Rect::new(0, 0, 100, 1)), NearbyPanel::new(Rect::new(76, 1, 24, 30)), LogPanel::new(Rect::new(0, 35, 76, 5))));
            app.add_plugins((
                rl_ui::panel::InventoryPanel::new(Rect::new(20, 5, 60, 30)),
                rl_ui::panel::SheetPanel::new(Rect::new(20, 5, 60, 30)),
                rl_ui::panel::AbilityPanel::new(Rect::new(20, 5, 60, 30)),
                rl_ui::panel::InspectPanel::new(Rect::new(20, 5, 60, 30)),
            ));
        }
    }
    app.insert_resource(Terminal::new(SCREEN.width, SCREEN.height, Vec2::ONE));

    let start = rl_bevy::testing::surface(&mut app);
    let sides = rl_bevy::testing::two_sides(&mut app);

    let mut player = app.world_mut().spawn((
        (Actor, Player, Blocks, Position(start), Viewshed::new(12), RevealsMap),
        (
            Health::full(1_000_000),
            Faction(sides.ours),
            MeleeAttack::new(sides.kind, rl_core::DiceRoll::flat(1)),
            Name::new("you"),
            Inventory::default(),
            Glyph::new('@', Color::WHITE).on_layer(10),
        ),
    ));
    if lit {
        // The torch the player carries: one dynamic source, moving with
        // whoever holds it, which is the case the claim is about.
        player.insert(rl_bevy::lighting::LightSource::new(200, 10, Rgb::new(255, 220, 160)));
    }
    let player = player.id();

    let brain = std::sync::Arc::new(rl_rules::Brain::new().then(rl_rules::ai::tactics::Hunt));
    let mut placed = 0;
    // Spread: a lattice over the window, so most of the crowd is well
    // outside the player's lamp. Huddled: rings from three cells out.
    let mut radius: i32 = if spread { 14 } else { 3 };
    let step = if spread { 3 } else { 1 };
    while placed < crowd {
        for dx in (-radius..=radius).step_by(step) {
            for dy in (-radius..=radius).step_by(step) {
                if placed >= crowd || (dx.abs() != radius && dy.abs() != radius) {
                    continue;
                }
                app.world_mut().spawn((
                    (Actor, Blocks, Position(start.offset(dx, dy)), Viewshed::new(10)),
                    (
                        Health::full(1_000),
                        Faction(sides.theirs),
                        Perception(10),
                        MeleeAttack::new(sides.kind, rl_core::DiceRoll::flat(1)),
                        Mind(brain.clone()),
                        Name::new("a droid"),
                        Glyph::new('d', Color::WHITE),
                    ),
                ));
                placed += 1;
            }
        }
        radius += step as i32;
    }

    if lit {
        app.world_mut().resource_mut::<rl_bevy::lighting::Lighting>().ambient = Light::DARK;
    }
    app.finish();
    app.cleanup();
    app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
    for _ in 0..8 {
        if app.world().get::<MyTurn>(player).is_some() {
            app.world_mut().write_message(Intent::new(player, Wait));
        }
        app.update();
    }
    (app, player)
}

/// A frame in which nothing moved: the player holds its turn and presses
/// nothing, so the loops do nothing and the frame is the drawing.
fn still_frame(app: &mut App) {
    app.update();
}

/// What a still frame costs, lit and unlit, as the crowd grows.
///
/// Lighting is opt-in, so the pair is the honest comparison: the
/// difference is what a game pays for turning it on at all, and how that
/// difference grows with the crowd is whether the invalidation is global.
fn still(c: &mut Criterion) {
    let mut group = c.benchmark_group("still_frame");
    for crowd in [8usize, 32, 64] {
        for lit in [false, true] {
            let name = if lit { "lit" } else { "unlit" };
            group.bench_with_input(BenchmarkId::new(name, crowd), &crowd, |b, &crowd| {
                let (mut app, _) = drawing(crowd, lit, Panels::Always);
                b.iter(|| still_frame(black_box(&mut app)));
            });
        }
    }
    group.finish();
}

/// A frame in which the light moved and nothing else did.
///
/// The claim item 2 of `docs/TODO.md` makes: `update_lighting` marks every
/// `Viewshed` in the world stale whenever the composed light changed, so
/// one actor carrying a torch one cell costs every other actor a
/// shadowcast on the next frame.
///
/// The player is moved by writing its `Position`, not by taking a turn, so
/// the turn loop does nothing and the frame is the light, the sight and
/// the drawing. Walking it through a real turn measured nothing at the
/// larger crowds: a player hemmed in by thirty-two hunters does not get to
/// step, and the frames were still frames.
fn relight(c: &mut Criterion) {
    let mut group = c.benchmark_group("light_moved_frame");
    for crowd in [8usize, 32, 64] {
        for lit in [false, true] {
            let name = if lit { "lit" } else { "unlit" };
            group.bench_with_input(BenchmarkId::new(name, crowd), &crowd, |b, &crowd| {
                let (mut app, player) = drawing(crowd, lit, Panels::Always);
                let home = app.world().get::<Position>(player).expect("the player stands somewhere").0;
                let mut east = true;
                b.iter(|| {
                    east = !east;
                    let to = if east { home.offset(1, 0) } else { home };
                    app.world_mut().get_mut::<Position>(player).expect("the player stands somewhere").0 = to;
                    app.update();
                });
            });
        }
    }
    group.finish();
}

/// What the collectors of unopened screens cost a frame.
///
/// Three apps drawing the same world: no views at all, the rail a game
/// always shows, and the rail plus four screens nobody has opened. The
/// last gap is the whole of what gating them on their screen could save.
fn collectors(c: &mut Criterion) {
    let mut group = c.benchmark_group("still_frame_by_panel");
    for panels in [Panels::None, Panels::Always, Panels::WithScreens] {
        let name = match panels {
            Panels::None => "map_only",
            Panels::Always => "rail",
            Panels::WithScreens => "rail_and_closed_screens",
        };
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            let (mut app, _) = drawing(32, false, panels);
            b.iter(|| still_frame(black_box(&mut app)));
        });
    }
    group.finish();
}

/// The same, with the crowd spread across the window rather than huddled
/// round the player: a level rather than a fight.
fn relight_spread(c: &mut Criterion) {
    let mut group = c.benchmark_group("light_moved_frame_spread");
    for crowd in [32usize, 64] {
        for lit in [false, true] {
            let name = if lit { "lit" } else { "unlit" };
            group.bench_with_input(BenchmarkId::new(name, crowd), &crowd, |b, &crowd| {
                let (mut app, player) = drawing_spread(crowd, lit, Panels::Always, true);
                let home = app.world().get::<Position>(player).expect("the player stands somewhere").0;
                let mut east = true;
                b.iter(|| {
                    east = !east;
                    let to = if east { home.offset(1, 0) } else { home };
                    app.world_mut().get_mut::<Position>(player).expect("the player stands somewhere").0 = to;
                    app.update();
                });
            });
        }
    }
    group.finish();
}

criterion_group!(benches, still, relight, relight_spread, collectors);
criterion_main!(benches);
