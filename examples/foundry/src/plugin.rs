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
        // Ammunition's own economy, chained in this order: `spend_ammo`
        // takes a slug off the bag a `Struck` just fired from, and
        // `sync_ammo` reads whatever bag every `Ammo` item's wielder now
        // has and dries or reloads from it. Reversed, a pass that spent
        // the last slug would leave every one of that bag's weapons
        // reading as loaded into the next pass's `Resolve`, and a shot
        // would go out on a bag `spend_ammo` had already emptied.
        // Unordered against the heat systems above: `Heat` and `Ammo`
        // never share an item (`gear::Armory::load` refuses a file that
        // tries), so neither ammo system ever touches an entity
        // `vent_heat` or `heat_on_struck` does.
        //
        // The two never need ordering against a *different* actor's turn
        // the way heat's pair does against a `TurnEnd` the scheduler can
        // deal in the same pass as the next turn: `Resolution::claim`
        // (crates/rl-bevy/src/turn.rs, around line 212) is `Acting::claim_action`
        // underneath, and it lets one actor resolve at most one action
        // per pass, so one actor's own shot and its own bag changing
        // (dropping, picking up, equipping) can never land in the same
        // pass to race each other in the first place.
        app.add_systems(Turn, (crate::ammo::spend_ammo, crate::ammo::sync_ammo).chain().in_set(TurnSet::React));
        // A deck fills the moment it is first entered, the way delve's own
        // floors do.
        app.add_systems(Turn, crate::droids::populate_deck.in_set(TurnSet::React));
        // A probe's alarm reacts to the same `Noticed` the engine's own
        // stealth writes; nothing here needs ordering against it.
        app.add_systems(Turn, crate::droids::sound_alarm.in_set(TurnSet::React));
        // Chained, and in this order: the engine's own `schedule` can
        // write a `TurnEnd` and deal the very next turn's `DamageDealt` in
        // the same pass, so `unjam_sensors` must count the turn that just
        // ended down before `jam_sensors` reads a hit that turn's actor
        // just landed (reversed, a fresh jam from that new hit would be
        // counted down before it had stood for even one whole turn of its
        // own), and `sync_dark_sight` must read `DarkSight` last of the
        // three, after both, so it derives an actor's sight from the jam
        // exactly as this pass leaves it rather than as it stood before
        // either ran. `DarkSight` is otherwise never written anywhere
        // else in this game: `sync_dark_sight` is its one owner, the same
        // way `ammo.rs`'s `sync_ammo` owns a weapon's loaded state.
        app.add_systems(Turn, (crate::droids::unjam_sensors, crate::droids::jam_sensors, crate::droids::sync_dark_sight).chain().in_set(TurnSet::React));
    }
}
