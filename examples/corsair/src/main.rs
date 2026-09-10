//! Corsair: walk the world.
//!
//! Milestone 1. A seeded continuous world, an `@` that walks it with sight
//! across chunk boundaries, a message log, a status line, and the world map
//! with a portal picker over the towns you have found.

mod content;
mod input;
mod monsters;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{Rect, RunSeed};
use rl_engine::rl_rules::FactionId;
use rl_engine::rl_overworld::{OverworldLayout, OverworldPlugin, PortalRequest};
use rl_engine::rl_render::{Glyph, MapView, MapViewPlugin, TerminalPlugin};
use rl_engine::rl_ui::{ChromeLayout, ChromePlugin, LogCategory, MessageLog, StatusLine};
use rl_engine::rl_world::{WorldConfig, WorldGraph};

use crate::content::{Content, PORT};

/// Terminal size in cells.
const COLS: i32 = 100;
const ROWS: i32 = 40;
const CELL: Vec2 = Vec2::new(10.0, 16.0);
const FONT: f32 = 14.0;
/// Rows given to chrome: one status line at the top, four log lines at the bottom.
const LOG_ROWS: i32 = 4;

fn main() -> AppExit {
    let mut seed = RunSeed::fresh();
    let mut regions = (64, 64);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                seed = RunSeed(args[i + 1].parse().expect("seed"));
                i += 2;
            }
            "--regions" => {
                let (w, h) = args[i + 1].split_once('x').expect("WxH");
                regions = (w.parse().expect("w"), h.parse().expect("h"));
                i += 2;
            }
            other => {
                eprintln!("unknown argument {other}");
                return AppExit::error();
            }
        }
    }

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("Corsair - seed {}", seed.0),
                    resolution: WindowResolution::new((COLS as f32 * CELL.x) as u32, (ROWS as f32 * CELL.y) as u32),
                    ..default()
                }),
                ..default()
            })
            .set(ImagePlugin::default_nearest()),
    )
    .add_plugins(TerminalPlugin {
        width: COLS,
        height: ROWS,
        cell_size: CELL,
        font_size: FONT,
    })
    .add_plugins((EnginePlugins, MapViewPlugin, ChromePlugin, OverworldPlugin))
    .insert_resource(StartSeed { seed, regions })
    .insert_resource(MapView::new(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
    .insert_resource(ChromeLayout {
        log_rows: Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS),
        status_row: 0,
    })
    .insert_resource(OverworldLayout {
        viewport: Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS),
    })
    .add_systems(Startup, start_world)
    .add_systems(Update, input::player_input.in_set(EngineSet::Decide))
    .add_systems(Update, honour_portals.in_set(EngineSet::Resolve))
    .add_systems(Update, monsters::spawn_on_load.in_set(EngineSet::Stream))
    .add_systems(Update, (note_discoveries, monsters::narrate, update_status).chain().in_set(EngineSet::Present));
    app.run()
}

#[derive(Resource)]
struct StartSeed {
    seed: RunSeed,
    regions: (i32, i32),
}

/// Generates the world, spawns the player at the first town, and starts play.
fn start_world(mut commands: Commands, start: Res<StartSeed>, mut next: ResMut<NextState<EngineState>>, mut log: ResMut<MessageLog>) {
    let content = Content::new();
    // Islands rather than a continent: less land, more of it coast.
    let mut config = WorldConfig::regions(start.regions.0, start.regions.1);
    config.elevation.land_fraction = 0.22;
    config.elevation.continent_frequency = 3.2;
    let began = std::time::Instant::now();
    let world = WorldGraph::generate(start.seed, config, &content);
    info!("world {}x{} regions in {:?}", world.width(), world.height(), began.elapsed());

    let town = world.sites().iter().find(|s| s.kind == PORT).expect("a world with a town");
    let spawn = world.region_tiles(town.position).center();
    let region_size = world.region_size();

    let (bestiary, rules) = monsters::Bestiary::load(start.seed, town.position);
    let player_faction: FactionId = bestiary.factions.expect("player");
    commands.spawn((
        Actor,
        Player,
        Blocks,
        Position(spawn),
        Viewshed::new(12),
        RevealsMap,
        Speed(100),
        Health::full(30),
        Armor(1),
        Faction(player_faction),
        MeleeAttack { kind: bestiary.kinds.expect("cutlass"), dice: rl_engine::rl_core::DiceRoll::new(1, 6) },
        Glyph::new('@', Color::WHITE).on_layer(10),
    ));
    commands.insert_resource(bestiary);
    commands.insert_resource(rules);
    commands.insert_resource(monsters::stages());
    commands.insert_resource(CombatRng::for_run(start.seed));

    commands.insert_resource(content.tile_appearance());
    commands.insert_resource(content.band_appearance());
    commands.insert_resource(WorldMap::new(region_size, content.tiles().tables()));
    commands.insert_resource(Knowledge::new(region_size));
    commands.insert_resource(WorldRes(world));
    commands.insert_resource(ChunkRulesRes(Box::new(content)));
    log.push(format!("Seed {}. You step off the gangplank onto the docks of a small port.", start.seed.0), LogCategory::Notice, 0);
    next.set(EngineState::Playing);
}

/// Moves the player to a discovered site when the overworld asks.
fn honour_portals(
    mut requests: MessageReader<PortalRequest>,
    world: Res<WorldRes>,
    mut occupancy: ResMut<Occupancy>,
    mut log: ResMut<MessageLog>,
    turns: Res<Turns>,
    mut player: Query<(Entity, &mut Position, &mut Viewshed), With<Player>>,
) {
    let Ok((entity, mut pos, mut viewshed)) = player.single_mut() else { return };
    for req in requests.read() {
        let Some(site) = world.sites().get(req.site) else { continue };
        let target = world.region_tiles(site.position).center();
        occupancy.relocate(entity, pos.0, target);
        pos.0 = target;
        viewshed.dirty = true;
        log.push("The portal takes you.", LogCategory::Notice, turns.turn_number());
    }
}

#[derive(Default)]
struct Discovered(usize);

/// Logs each new site the player sights.
fn note_discoveries(knowledge: Res<Knowledge>, world: Res<WorldRes>, turns: Res<Turns>, mut log: ResMut<MessageLog>, mut seen: Local<Discovered>) {
    let count = knowledge.discovered_sites().count();
    if count > seen.0 {
        for site in knowledge.discovered_sites().skip(seen.0) {
            let kind = if world.sites()[site].kind == PORT { "a port" } else { "a cove" };
            log.push(format!("You discover {kind}."), LogCategory::Good, turns.turn_number());
        }
        seen.0 = count;
    }
}

fn update_status(mut status: ResMut<StatusLine>, turns: Res<Turns>, world: Res<WorldRes>, state: Res<State<EngineState>>, player: Query<(&Position, &Health), With<Player>>) {
    if *state.get() != EngineState::Playing {
        return;
    }
    let Ok((pos, hp)) = player.single() else { return };
    let region = world.region_of_tile(pos.0);
    let band = world.layers().band(region).map(content::band_name).unwrap_or("nowhere");
    status.0 = format!(
        "HP {}/{}   Turn {}   ({}, {})   region ({}, {}) {}   [m]ap  [q]uit",
        hp.hp,
        hp.max,
        turns.turn_number(),
        pos.0.x,
        pos.0.y,
        region.x,
        region.y,
        band
    );
}
