//! `FoundryPlugin`: every one of Foundry's own systems and resources, in
//! one place.
//!
//! A system registered separately in `main.rs` and again in
//! `testing::headless` is two places to keep in step, and nothing stops
//! them drifting apart: Corsair's binary runs `honour_portals`,
//! `populate_places`, `drop_loot` and `inflict_on_hit`, and its harness
//! runs none of them, so a test there exercises a different game from
//! the one that ships. `FoundryPlugin` is the one list instead: `main.rs`
//! adds it beside the engine's plugins, `testing::headless` adds it
//! beside the engine plugins it needs, and neither registers a game
//! system of its own. Every later task adds its systems here.
use bevy::prelude::*;
use rl_engine::rl_bevy::plugin::{NewRun, Turn, TurnSet};

/// Foundry's own systems: the run's start, and every reaction a task
/// after this one adds.
pub struct FoundryPlugin;

impl Plugin for FoundryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(NewRun, crate::run::start);
        app.add_systems(Turn, crate::gear::grant_dark_sight.in_set(TurnSet::React));
    }
}
