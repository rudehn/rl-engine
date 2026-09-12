//! A headless Corsair, for the tests in this crate.
//!
//! The engine plugins and the resources the startup system reads, with no
//! window: enough of the real game that a test exercises the real wiring
//! rather than a mock of it.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_save::{FileBackend, Saves};
use rl_engine::rl_ui::MessageLog;

/// A run with no window, seeded and pointed at `dir` for its saves.
pub fn headless(seed: RunSeed, resume: bool, dir: &std::path::Path) -> App {
    let mut app = rl_engine::rl_bevy::plugin::headless_app();
    app.add_plugins((FovPlugin, CombatPlugin, StatusPlugin, ItemsPlugin, LightingPlugin, StreamingPlugin, FactsPlugin));
    app.insert_resource(crate::StartSeed { seed, regions: (24, 24), resume })
        .insert_resource(Saves(Box::new(FileBackend::new(dir))))
        .init_resource::<MessageLog>()
        .init_resource::<crate::inventory::InventoryScreen>()
        .init_resource::<crate::places::Entrances>()
        .init_resource::<crate::quests::LedgerScreen>()
        .add_systems(Startup, crate::start_world)
        .add_systems(Update, (crate::monsters::spawn_on_load, crate::items::scatter_on_load, crate::places::mark_entrances).in_set(EngineSet::Stream))
        .add_systems(Turn, crate::items::refresh_gear.in_set(TurnSet::React));
    app
}
