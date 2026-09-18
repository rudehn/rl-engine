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
use rl_engine::rl_ui::ViewSet;

/// Foundry's own systems: the run's start, and every reaction a task
/// after this one adds.
pub struct FoundryPlugin;

impl Plugin for FoundryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(NewRun, crate::run::start);
        app.add_systems(Turn, crate::gear::grant_dark_sight.in_set(TurnSet::React));
        // Chained, and in this order: the engine's own `schedule`
        // (crates/rl-bevy/src/turn.rs) can write a `TurnEnd` and deal the
        // next actor's turn in the same pass, so `TurnSet::React` can see
        // the ending turn's `TurnEnd` together with the new turn's
        // `Struck`. Venting first reads each `Heat`'s `fired` as that
        // ending turn left it; running `heat_on_struck` first would mark
        // `fired` for the turn that is only just starting, and the turn
        // that actually just ended quietly would wrongly skip its vent.
        app.add_systems(Turn, (crate::heat::vent_heat, crate::heat::heat_on_struck).chain().in_set(TurnSet::React));
        // Correct with or without a gear panel: `note_heat` reads
        // `GearView` as `Option<Res<_>>` and does nothing until a game
        // adds `GearViewPlugin`, which this slice's binary does not yet.
        app.add_systems(Update, crate::heat::note_heat.in_set(ViewSet::Annotate));
    }
}
