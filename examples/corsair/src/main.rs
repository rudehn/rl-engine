//! Corsair: a small pirate roguelike, and the worked example of the engine.
//!
//! A seeded continuous world of islands, an `@` that walks it with sight
//! across chunk boundaries, monsters that hunt from RON, loot on the sand
//! and in the pockets of the dead, a sea chest to wear it from, a message
//! log, a status line, and the world map with a portal picker over the
//! ports you have found.
//!
//! `--seed N` picks the world, `--continue` resumes the saved run,
//! `--balance` prints the spawn table's threat by band and exits.
//! `CORSAIR_OPEN=inventory` or `CORSAIR_OPEN=ledger` starts with that screen open, and
//! `CORSAIR_START=cave` starts at the bottom of the nearest smugglers'
//! cave, both for screenshots.

mod content;
mod input;
mod inventory;
mod items;
mod monsters;
mod places;
mod quests;
mod save;
mod statuses;
#[cfg(test)]
mod testing;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{Rect, RunSeed};
use rl_engine::rl_overworld::{OverworldLayout, OverworldPlugin, PortalRequest};
use rl_engine::rl_render::{CapturePlugin, capture};
use rl_engine::rl_render::{Glyph, MapView, MapViewPlugin, TerminalPlugin};
use rl_engine::rl_rules::FactionId;
use rl_engine::rl_ui::{
    Facets, GearPanel, InspectPanel, LogPanel, MessageLog, Modals, NearbyPanel, NearbyView, ScrollbackPanel, Tones, UiPlugin, ViewSet, VitalsPanel, panel,
};
use rl_engine::rl_world::{WorldConfig, WorldGraph};

use crate::content::{Content, PORT};
use crate::items::{Armory, ItemKind};
use crate::monsters::Bestiary;
use rl_engine::rl_save::Saves;

/// Terminal size in cells.
const COLS: i32 = 100;
const ROWS: i32 = 40;
const CELL: Vec2 = Vec2::new(10.0, 16.0);
const FONT: f32 = 14.0;
/// Rows given to the log at the bottom of the screen.
const LOG_ROWS: i32 = 4;
/// Columns given to the rail down the right.
const RAIL: i32 = 26;
/// Rows the rail gives to vitals and to gear; the rest is what is nearby.
const VITALS_ROWS: i32 = 9;
const GEAR_ROWS: i32 = 7;

/// The screen, cut up once so every panel and the map agree on it.
struct Screen {
    map: Rect,
    log: Rect,
    vitals: Rect,
    gear: Rect,
    nearby: Rect,
    inspect: Rect,
    scrollback: Rect,
}

impl Screen {
    fn new() -> Self {
        let (left, rail) = panel::split_right(Rect::new(0, 0, COLS, ROWS), RAIL);
        let (map, log) = panel::split_bottom(left, LOG_ROWS);
        let (vitals, below) = panel::split_top(rail, VITALS_ROWS);
        let (gear, nearby) = panel::split_top(below, GEAR_ROWS);
        let inspect = Rect::new(map.x + 2, map.bottom() - 12, map.width.min(52), 10);
        // The whole map area, since reading back is all you are doing.
        let scrollback = map.inflate(-2);
        Self { map, log, vitals, gear, nearby, inspect, scrollback }
    }
}

fn main() -> AppExit {
    let mut seed = RunSeed::fresh();
    let mut regions = (64, 64);
    let mut resume = false;
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
            "--continue" => {
                resume = true;
                i += 1;
            }
            "--balance" => {
                print!("{}", balance_report());
                return AppExit::Success;
            }
            other => {
                eprintln!("unknown argument {other}");
                eprintln!("usage: corsair [--seed N] [--regions WxH] [--continue] [--balance]");
                return AppExit::error();
            }
        }
    }

    let screen = Screen::new();
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(capture::prepare(Window {
                    title: "Corsair".to_string(),
                    resolution: WindowResolution::new((COLS as f32 * CELL.x) as u32, (ROWS as f32 * CELL.y) as u32),
                    ..default()
                })),
                ..default()
            })
            .set(ImagePlugin::default_nearest()),
    )
    .add_plugins(TerminalPlugin { width: COLS, height: ROWS, cell_size: CELL, font_size: FONT })
    .add_plugins((CorePlugin, FovPlugin, CombatPlugin, StatusPlugin, ItemsPlugin, LightingPlugin, StreamingPlugin, FactsPlugin))
    .add_plugins((MapViewPlugin, UiPlugin, OverworldPlugin, CapturePlugin))
    // The panels. Each one draws itself from a view the engine keeps
    // current; none of them needs a system of Corsair's.
    .add_plugins((
        VitalsPanel::new(screen.vitals).bars(12).heading("Vitals"),
        GearPanel::new(screen.gear),
        NearbyPanel::new(screen.nearby).titled("").headings("In sight", "On the ground"),
        LogPanel::new(screen.log),
        InspectPanel::new(screen.inspect).hints("move \u{2022} tab next \u{2022} esc close"),
        // A second presenter over the same log the strip draws: `p` opens
        // all of it, scrollable and filterable by tone.
        ScrollbackPanel::new(screen.scrollback).titled("Ship's log"),
    ))
    .insert_resource(StartSeed { seed, regions, resume })
    .insert_resource(Saves::platform_default("corsair"))
    .insert_resource(MapView::new(screen.map))
    .insert_resource(OverworldLayout { viewport: screen.map })
    .init_resource::<inventory::InventoryScreen>()
    .init_resource::<places::Entrances>()
    .init_resource::<quests::LedgerScreen>()
    .add_systems(Startup, start_world)
    .add_systems(Update, (quests::ledger_keys, inventory::inventory_keys, input::player_input, input::fire).chain().in_set(EngineSet::Input))
    // Saving reads the whole world, so it runs outside the engine's sets, after the frame's turns.
    .add_systems(Update, save::save_keys.after(EngineSet::Present))
    .add_systems(Turn, honour_portals.in_set(TurnSet::Resolve))
    .add_systems(Update, places::light_the_way.after(EngineSet::Turns).before(EngineSet::Light).run_if(in_state(EngineState::Playing)))
    .add_systems(Update, (monsters::spawn_on_load, items::scatter_on_load, places::mark_entrances).in_set(EngineSet::Stream))
    // What this turn caused, answered inside the turn: the floor that
    // fills on first arrival, what the dead leave, what a drink does,
    // what gear is worth, what a bite leaves behind. Inside the pass, so
    // a drink heals before the next blow lands.
    .add_systems(
        Turn,
        (places::populate_places, items::drop_loot, items::use_items, items::refresh_gear, statuses::inflict_on_hit).chain().in_set(TurnSet::React),
    )
    // Once a frame, in words: everything the chrome is about to draw.
    .add_systems(
        Update,
        (
            note_discoveries,
            monsters::narrate,
            items::narrate_items,
            statuses::narrate_statuses,
            quests::report_facts,
            quests::narrate_quests,
            save::delete_on_death,
        )
            .chain()
            .in_set(PresentSet::Narrate),
    )
    // What the engine cannot know about a row: what an enemy is holding,
    // and what is underfoot. Named by set, not by ordering after a
    // collector.
    .add_systems(Update, (note_what_they_wield, note_where_you_are).in_set(ViewSet::Annotate))
    .add_systems(Update, (inventory::draw_inventory, quests::draw_ledger).chain().in_set(PresentSet::Overlay));
    app.add_plugins(StealthPlugin);
    // Corsair's own screens, declared while building so the lookups in
    // `inventory` and `quests` find them.
    {
        let mut modals = app.world_mut().resource_mut::<Modals>();
        modals.declare(inventory::MODAL);
        modals.declare(quests::MODAL);
    }
    app.run()
}

/// The spawn table scored band by band, for `--balance`.
fn balance_report() -> String {
    let (bestiary, _) = monsters::Bestiary::load(RunSeed(0), rl_engine::rl_core::Point::ZERO);
    let report = rl_engine::rl_rules::Report::over(&bestiary.table, 0..=40, |id| {
        let m = bestiary.defs.get(*id);
        (m.name.clone(), rl_engine::rl_rules::threat(m))
    });
    format!("Corsair spawn bands (distance from the home port)\n{}", report.render())
}

#[derive(Resource, Clone, Copy)]
struct StartSeed {
    seed: RunSeed,
    regions: (i32, i32),
    resume: bool,
}

/// Generates the world and either spawns a fresh player at the first
/// port or restores the saved run into it, then starts play.
fn start_world(world: &mut World) {
    let start = *world.resource::<StartSeed>();
    let saved = if start.resume {
        match save::load_run(world.resource::<Saves>()) {
            Ok(Some(s)) => Some(s),
            Ok(None) => {
                warn!("no save to continue; starting a new run");
                None
            }
            Err(e) => {
                error!("the save could not be read: {e}; starting a new run");
                None
            }
        }
    } else {
        None
    };
    let (seed, regions) = saved.as_ref().map(|s| (s.engine.seed, s.regions)).unwrap_or((start.seed, start.regions));

    let content = Content::new();
    // Islands rather than a continent: less land, more of it coast.
    let mut config = WorldConfig::regions(regions.0, regions.1);
    config.elevation.land_fraction = 0.22;
    config.elevation.continent_frequency = 3.2;
    let began = std::time::Instant::now();
    let graph = WorldGraph::generate(seed, config, &content);
    info!("world {}x{} regions in {:?}", graph.width(), graph.height(), began.elapsed());

    let town = graph.sites().iter().find(|s| s.kind == PORT).expect("a world with a town");
    let spawn = graph.region_tiles(town.position).center();

    let (bestiary, rules) = monsters::Bestiary::load(seed, town.position);
    let armory = Armory::load(seed, town.position, &bestiary.kinds);
    bestiary
        .defs
        .validate(|m, _| m.drops.iter().find(|(name, _)| armory.defs.id(name).is_none()).map_or(Ok(()), |(name, _)| Err(format!("unknown drop {name:?}"))))
        .unwrap_or_else(|e| panic!("assets/monsters.ron: {e}"));
    let (quest_log, facts) = quests::load(&bestiary, &armory);
    world.insert_resource(statuses::load(&armory, &bestiary));
    let cove = graph.sites().iter().position(|s| s.kind == content::COVE);

    world.insert_resource(quest_log);
    world.insert_resource(facts);
    world.insert_resource(rules);
    world.insert_resource(monsters::stages());
    world.insert_resource(CombatRng::for_run(seed));
    world.insert_resource(content.tile_appearance());
    world.insert_resource(content.band_appearance());
    world.insert_resource(WorldMap::new(content.tiles().tables()));
    world.insert_resource(Lighting::new(places::daylight()));
    world.insert_resource(WorldRes(graph));
    world.insert_resource(PlaceRulesRes(Box::new(places::Caves::new(content.clone()))));
    world.insert_resource(ChunkRulesRes(Box::new(content)));
    world.insert_resource(Slots(armory.slots.clone()));
    world.insert_resource(armory);
    world.insert_resource(bestiary);

    match saved {
        Some(saved) => {
            save::restore_run(world, &saved);
            let text = save::describe(&saved);
            world.resource_mut::<MessageLog>().push(text, Tones::NOTICE, saved.turn);
        }
        None => {
            spawn_fresh_player(world, spawn);
            world.resource_mut::<MessageLog>().push(format!("Seed {}. You step off the gangplank onto the docks of a small port.", seed.0), Tones::NOTICE, 0);
            if std::env::var("CORSAIR_START").is_ok_and(|v| v == "cave")
                && let Some(cove) = cove
            {
                let mut players = world.query_filtered::<Entity, With<Player>>();
                let player = players.single(world).expect("the player was just spawned");
                world.write_message(WarpRequest {
                    actor: player,
                    to: Destination::Place { map: places::cave_id(cove, places::LEVELS - 1), arrive: Arrive::Entry },
                });
            }
        }
    }
    let open = match std::env::var("CORSAIR_OPEN").as_deref() {
        Ok("inventory") => Some(inventory::MODAL),
        Ok("ledger") => Some(quests::MODAL),
        _ => None,
    };
    if let Some(name) = open {
        let mut modals = world.resource_mut::<Modals>();
        let id = modals.declare(name);
        modals.open(id);
    }
    world.resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
}

/// A new player at `spawn` with a cutlass in hand and a bottle in the bag.
fn spawn_fresh_player(world: &mut World, spawn: rl_engine::rl_core::Point) {
    // The armory comes out while its spawner borrows commands.
    let armory = world.remove_resource::<Armory>().expect("the armory is inserted first");
    let (cutlass, rum, worn) = {
        let mut commands = world.commands();
        let cutlass = armory.spawn(&mut commands, armory.defs.expect("cutlass"), 1, None);
        let rum = armory.spawn(&mut commands, armory.defs.expect("rum"), 2, None);
        let mut worn = Equipped(rl_engine::rl_rules::Equipment::for_slots(&armory.slots));
        worn.equip(cutlass, armory.shape(armory.defs.expect("cutlass")).expect("a cutlass is worn")).expect("the slots exist");
        (cutlass, rum, worn)
    };
    world.flush();
    world.insert_resource(armory);
    let (faction, unarmed) = {
        let bestiary = world.resource::<Bestiary>();
        let faction: FactionId = bestiary.factions.expect("player");
        (faction, items::unarmed(bestiary))
    };
    world.spawn((
        (Actor, Player, Blocks, Position(spawn), Viewshed::new(12), RevealsMap, Speed(100)),
        (
            Health::full(30),
            Armor(0),
            Faction(faction),
            unarmed,
            Inventory { items: vec![cutlass, rum] },
            worn,
            Name::new("you"),
            // Quiet enough that a smuggler in the dark has to be close,
            // or catch you in your own lantern light, to be sure of you.
            Stealth(rl_engine::rl_rules::ai::awareness::StealthStats { quiet: 1, subtlety: 10 }),
            StatBlock::default(),
            Afflicted::default(),
            Strikes::default(),
            Glyph::new('@', Color::WHITE).on_layer(10),
        ),
    ));
}

/// Asks the engine to move the player to a discovered site when the
/// overworld asks, from wherever the player is, a cave included.
fn honour_portals(
    mut requests: MessageReader<PortalRequest>,
    mut warps: MessageWriter<WarpRequest>,
    world: Res<WorldRes>,
    mut log: ResMut<MessageLog>,
    turns: Res<Turns>,
    player: Query<Entity, With<Player>>,
) {
    let Ok(entity) = player.single() else { return };
    for req in requests.read() {
        let Some(site) = world.sites().get(req.site) else { continue };
        let target = world.region_tiles(site.position).center();
        warps.write(WarpRequest { actor: entity, to: Destination::Surface(target) });
        log.push("The portal takes you.", Tones::NOTICE, turns.turn_number());
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
            log.push(format!("You discover {kind}."), Tones::GOOD, turns.turn_number());
        }
        seen.0 = count;
    }
}

/// What a foe is holding, which the engine has no way to know: it has no
/// armory and no idea what a weapon is. One facet per row, pushed in
/// [`ViewSet::Annotate`].
fn note_what_they_wield(mut nearby: ResMut<NearbyView>, mut facets: ResMut<Facets>, armory: Res<Armory>, worn: Query<&Equipped>, kinds: Query<&ItemKind>) {
    let main = armory.slots.expect("main hand");
    for row in nearby.actors.iter_mut() {
        let Some(held) = worn.get(row.entity).ok().and_then(|w| w.in_slot(main)) else { continue };
        let Ok(kind) = kinds.get(held) else { continue };
        row.facets.push(facets.facet("wielding", armory.defs.get(kind.0).name.clone()).toned(Tones::MUTED));
    }
}

/// Where the player is and what is underfoot: a band of the world, a
/// level of a cave, and whatever is lying on this tile. None of it is
/// anything the engine could name.
fn note_where_you_are(
    mut vitals: ResMut<rl_engine::rl_ui::VitalsView>,
    mut facets: ResMut<Facets>,
    armory: Res<Armory>,
    world: Res<WorldRes>,
    map: Res<WorldMap>,
    ground: Query<items::GroundData, With<Item>>,
    player: Query<&Position, With<Player>>,
) {
    let Ok(pos) = player.single() else { return };
    let region = world.region_of_tile(pos.0);
    let below = places::place_name(map.current());
    let band = below.as_deref().unwrap_or_else(|| world.layers().band(region).map(content::band_name).unwrap_or("nowhere"));
    vitals.facets.push(facets.facet("whereabouts", band.to_string()));
    if let Some(here) = items::whats_here(pos.0, &armory, &ground) {
        vitals.facets.push(facets.facet("underfoot", format!("here: {here} [g]")).toned(Tones::NOTICE));
    }
}
