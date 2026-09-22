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
//!
//! The one exception is drawing into a rectangle of the screen: the
//! engine's panels and `upgrades::ChoicePanel` take the rectangle
//! `main.rs` cuts for them, which a headless test has no screen to cut.
use bevy::prelude::*;
use rl_engine::rl_bevy::EngineState;
use rl_engine::rl_bevy::plugin::{EngineSet, NewRun, Turn, TurnSet};
use rl_engine::rl_bevy::{AddSound, AddVerb, PropSet, PropsPlugin, RemainsPlugin};
use rl_engine::rl_ui::{AddModal, AimFire, AimThrow, NarrationViewPlugin, ViewSet};

/// Foundry's own systems: the run's start, and every reaction a task
/// after this one adds.
pub struct FoundryPlugin;

impl Plugin for FoundryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(NewRun, crate::run::start);
        // Props and remains: the crates, the cable, the console and the
        // wreck a droid leaves. `PropsPlugin` owns what a prop is and does;
        // Foundry says where one stands, what goes in a container, and that
        // a wreck is worth opening.
        app.add_plugins((PropsPlugin, RemainsPlugin::naming("{what} remains")));

        app.add_systems(Turn, crate::props::wreck_the_dead.in_set(TurnSet::React));
        // In the engine's own filling stage, so what goes into a crate
        // lands in the frame the crate was put down: the engine asks in
        // `PropSet::Stock` and a game answers in `PropSet::Fill`.
        app.add_systems(Update, crate::props::fill_containers.in_set(PropSet::Fill));
        // The alarm is a sound of Foundry's own, declared once so
        // `sound_alarm` finds it by name.
        app.add_sound(crate::droids::ALARM_SOUND);
        // Chained, and in this order: the engine's own `schedule`
        // (crates/rl-bevy/src/turn.rs) can write a `TurnEnd` and deal the
        // next actor's turn in the same pass, so `TurnSet::React` can see
        // the ending turn's `TurnEnd` together with the new turn's
        // `Struck`. Venting first reads each `Heat`'s `fired` as that
        // ending turn left it; running `heat_on_struck` first would mark
        // `fired` for the turn that is only just starting, and the turn
        // that actually just ended quietly would wrongly skip its vent.
        app.add_systems(Turn, (crate::heat::vent_heat, crate::heat::heat_on_struck).chain().in_set(TurnSet::React));
        // Correct with or without a gear panel: `note_heat` and
        // `note_ammo` read `GearView` as `Option<ResMut<_>>` and do nothing
        // until a game adds `GearViewPlugin`, as the binary's `GearPanel`
        // does and a headless test need not. Unordered against each
        // other: no weapon carries both `Heat` and `Ammo`, so no row ever
        // gets a facet from both.
        app.add_systems(Update, (crate::heat::note_heat, crate::ammo::note_ammo).in_set(ViewSet::Annotate));
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
        //
        // After heat's pair, only for the log: every line Foundry tells in
        // one pass reads in one fixed order, the deck arrived on, then the
        // alarm, then the weapon's heat, then its ammunition, rather than
        // whichever order the executor ran the tellers in.
        app.add_systems(Turn, (crate::ammo::spend_ammo, crate::ammo::sync_ammo).chain().after(crate::heat::heat_on_struck).in_set(TurnSet::React));
        // A deck fills the moment it is first entered, the way delve's own
        // floors do: its droids, then its props, then its loot, each from
        // its own stream and each reading the same `PlaceEntered` through
        // its own cursor.
        //
        // Props before loot, because loot lands where nothing stands: an
        // item under a crate is an item nothing can pick up.
        //
        // Chained, though no two of them share a tile-claiming concern:
        // three systems that all spawn, left unordered, queue their
        // commands in whatever order they finish in, and the entities come
        // out with different ids from one run to the next. Nothing in the
        // game plays differently for it, but `tests/fingerprint.rs` hashes
        // a run by spawn order, and a tripwire that flickers is worse than
        // no tripwire. This is the order the deck is built in.
        app.add_systems(
            Turn,
            (crate::droids::populate_deck, crate::props::place_on_arrival, crate::mission::spawn_console_on_arrival, crate::loot::scatter_on_arrival)
                .chain()
                .in_set(TurnSet::React),
        );
        // Whatever a kill's kind carries falls where it died, from
        // `Drops` rather than the combat stream a kill's own dice came
        // from. Needs no ordering against `process_deaths`
        // (crates/rl-bevy/src/combat.rs): that runs in `TurnSet::Cleanup`,
        // which the engine's own schedule always runs after every
        // `TurnSet::React` system, this one included.
        app.add_systems(Turn, crate::loot::drop_on_death.in_set(TurnSet::React));
        // A probe's alarm: its line reacts to the same `Noticed` the
        // engine's own stealth writes, before heat's pair for the order of
        // the log, as above; its shout answers each action it finishes,
        // and touches nothing else of Foundry's.
        app.add_systems(Turn, (crate::droids::sound_alarm.before(crate::heat::vent_heat), crate::droids::shout_alarm).in_set(TurnSet::React));
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
        // The mission: loaded fresh every run, the way the roster and the
        // armory are.
        app.add_systems(NewRun, crate::mission::start);
        // The console is a prop, and its verb is Foundry's: declared once,
        // answered in `React` like every other reaction to a turn.
        app.add_verb(crate::mission::CHARGE);
        app.add_systems(Turn, crate::mission::answer_charge.in_set(TurnSet::React));
        // A game's reaction to the mission finishing, not to a turn
        // itself: an ordinary `Update` system, the way Corsair's own
        // `narrate_quests` is, reading `QuestChange` one frame behind the
        // fact that finished it (`rl_bevy::events`'s own doc on it).
        // Before the input set, so the pick is open before any key handler
        // reads `Modals` that frame and a key cannot fall through to the
        // world while the pick is pending, and before the drawing.
        app.add_systems(Update, crate::mission::offer_the_pick.before(EngineSet::Input));
        // Uplink's own reach bonus: after `heat`'s and `ammo`'s chains
        // above, for the reason `upgrades::react_uplink` gives.
        app.add_systems(Turn, crate::upgrades::react_uplink.after(crate::heat::heat_on_struck).after(crate::ammo::sync_ammo).in_set(TurnSet::React));
        // The pick screen: declared while building, the way Corsair
        // declares its ledger, so `upgrades::modal` finds it the moment
        // anything looks. `choice_keys` is exclusive (it calls
        // `upgrades::apply`, which needs the whole `World`), so it is
        // ordered the same place a game's own key handlers run, chained
        // below with the rest of the keys. Its drawing is not here:
        // `upgrades::ChoicePanel` takes a rectangle of the screen, so
        // `main.rs` adds it beside the engine's own panels.
        app.add_modal(crate::upgrades::MODAL);
        app.init_resource::<crate::upgrades::ChoiceScreen>();
        app.init_resource::<crate::upgrades::Choosing>();
        // The keys, declared once so the controls screen lists what these
        // systems read. Chained, the pick first: a confirm that closes the
        // pick must not fall through, the same frame, to the world's own
        // keys as a step or a lift. `f` and `t` hand the engine's
        // targeting cursor an `AimFire` or `AimThrow`; registering both
        // here is a no-op beside the cursor's own plugin and lets a
        // headless test with no cursor press the keys all the same.
        crate::input::declare_controls(app);
        app.add_message::<AimFire>().add_message::<AimThrow>();
        app.add_systems(Update, (crate::upgrades::choice_keys, crate::light::toggle_lamp, crate::input::player_input).chain().in_set(EngineSet::Input));
        // The lifts between decks, laid on first arrival beside everything
        // else a deck fills with, and the line the log gives each deck.
        // First of Foundry's lines in a pass, for the order of the log.
        app.add_systems(Turn, crate::lifts::link_decks.before(crate::droids::sound_alarm).in_set(TurnSet::React));
        // Light: each deck's ambient, set between the turns and the light
        // the way delve's `set_ambient` is, so the frame a lift lands on
        // is drawn in the new deck's light; and the stores' wall lamps,
        // hung on first arrival.
        app.add_systems(Update, crate::light::set_ambient.after(EngineSet::Turns).before(EngineSet::Light).run_if(in_state(EngineState::Playing)));
        app.add_systems(Turn, crate::light::light_the_lamps.in_set(TurnSet::React));
    }

    /// Every line Foundry says from inside a turn is a `Tell`, which only
    /// the engine's narrator collects and speaks.
    fn finish(&self, app: &mut App) {
        rl_engine::rl_bevy::plugin::depends_on::<NarrationViewPlugin>(app, "FoundryPlugin");
    }
}

#[cfg(test)]
mod ambiguity;
