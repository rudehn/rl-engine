//! `FoundryPlugin`: every one of Foundry's own systems and resources, in
//! one place.
//!
//! A system registered separately in `main.rs` and again in
//! `testing::headless` is two places to keep in step, and nothing stops
//! them drifting apart until a test exercises a different game from the
//! one that ships. `FoundryPlugin` is the one list instead: `main.rs`
//! adds it beside the engine's plugins, `testing::headless` adds it
//! beside the engine plugins it needs, and neither registers a game
//! system of its own. Every later task adds its systems here.
//!
//! The one exception is drawing into a rectangle of the screen: the
//! engine's panels and `upgrades::ChoicePanel` take the rectangle
//! `main.rs` cuts for them, which a headless test has no screen to cut.
//! The title is one of them, the engine's `TitlePanel` with Foundry's
//! picture painted under it, so a headless run has none and begins at
//! startup.
use atglyph::ecs::EngineApp;
use atglyph::ecs::plugin::{EngineSet, NewRun, PerceiveSet, PrepareRun, ReactSet, RunResumed, Turn, TurnSet};
use atglyph::ecs::{AmmoPlugin, LootPlugin, LootSet, PrefabPlugin, PrefabSet, PropsPlugin, RemainsPlugin};
use atglyph::ui::UiApp;
use atglyph::ui::{AimFire, AimThrow, NarrationViewPlugin, NarratorPlugin, Phrase, PlayerKeysSet, Tones, ViewSet};
use bevy::prelude::*;

/// The narrator as Foundry words it, for `main.rs` and
/// `testing::headless` both, so a test reads the log the player reads.
///
/// What the commando calls on is named as a thing, the stims, never as a
/// spell with a proper name, so a use takes an article: "You use the
/// stims", where the engine's own wording would read "You use stims". No
/// droid uses an ability, so only the commando's lines are reworded.
pub fn narrator() -> NarratorPlugin {
    NarratorPlugin::default()
        .phrase(Phrase::YouUse, "You use the {named}.", Tones::TEXT)
        .phrase(Phrase::YouUseOn, "You use the {named} on {whom}.", Tones::TEXT)
        .phrase(Phrase::YouUseOnMany, "You use the {named}, catching {n}.", Tones::TEXT)
        .phrase(Phrase::YouCannotUse, "You cannot use the {named}: {detail}.", Tones::BAD)
}

/// The engine's subsystems `FoundryPlugin` plays with beyond the ones
/// `main.rs` and `testing::headless` add beside it, and nothing of
/// Foundry's own: props and the wrecks a droid leaves, loot, the prefab
/// slots and ammunition.
///
/// [`FoundryPlugin`] adds these before any system of its own, and the
/// ordering test builds the game with these and without the plugin, so
/// whatever the plugin adds beyond them is the game's to order.
pub fn add_engine(app: &mut App) {
    // Props and remains: the crates, the cable, the console and the wreck
    // a droid leaves. `PropsPlugin` owns what a prop is and does; Foundry
    // says where one stands, what goes in a container, and that a wreck is
    // worth opening.
    app.add_plugins((PropsPlugin, RemainsPlugin::naming("{what} remains")))
        // Loot is the engine's: what lies on a deck when it is first
        // entered, what a kill leaves and what a crate holds, each made by
        // the armory.
        .add_plugins(LootPlugin::<crate::gear::Armory>::default())
        // Prefab slots are the engine's too: the lockers and crates the
        // pieces name, the guard post's weapon and its guards, each put
        // down on a deck's first arrival, the guards made by the roster
        // and the items by the armory.
        .add_plugins(PrefabPlugin::<crate::droids::Roster, crate::gear::Armory>::default())
        // Ammunition: `spend_ammo` takes a slug off the bag a `Struck`
        // just fired from, and `sync_ammo`, chained right after it in
        // `AmmoSet::Resolve`, dries or reloads every `Ammo` item from
        // whatever bag its wielder now has.
        .add_plugins(AmmoPlugin);
}

/// Foundry's own systems: the run's start, and every reaction a task
/// after this one adds, after the engine's plugins [`add_engine`] adds.
pub struct FoundryPlugin;

impl Plugin for FoundryPlugin {
    fn build(&self, app: &mut App) {
        add_engine(app);
        // ANCHOR: run_schedules
        // What every run stands on, the run's start, and what a continued
        // run gives back and says once the engine has restored it: the
        // upgrades first, so a pick offered again is offered to the
        // commando the upgrades made.
        app.add_systems(PrepareRun, crate::run::prepare);
        app.add_systems(NewRun, crate::run::start);
        app.add_systems(RunResumed, (crate::run::give_upgrades_again, crate::run::offer_the_waiting_pick, crate::run::say_where_it_goes_on).chain());
        // What a run is saved as, and when: on the way out and on every
        // deck arrival.
        crate::save::register(app);
        // ANCHOR_END: run_schedules
        // Work, for the repair drone: the engine keeps its turns and its
        // row; `droids::repair` says which wrecks and what finishing means.
        app.add_work(crate::droids::REPAIRING).add_work(crate::droids::SIGNALLING);
        app.add_systems(Turn, crate::droids::sense_wrecks.in_set(PerceiveSet::Annotate))
            .add_systems(Turn, crate::droids::rebuild_wrecks.in_set(TurnSet::React));
        // The armory the engine's loot and prefab slots make things by,
        // loaded once before any run begins.
        app.add_systems(PreStartup, crate::gear::load_armory);
        app.add_systems(Turn, crate::props::wreck_the_dead.in_set(TurnSet::React));
        app.add_systems(Turn, crate::props::spend_the_keycard.in_set(TurnSet::React));
        // The commando fades while a cloak hides them, in the pass it went
        // on or came off.
        app.add_systems(Turn, crate::run::fade_the_unseen.in_set(TurnSet::React));
        // Foundry's own sounds, the alarm's that `sound_alarm` finds by
        // name and the grenades' that `items.ron` names, declared from the
        // one list `gear::sounds` also walks for an armory loaded with no
        // app, so each has the same id in both.
        for sound in crate::gear::SOUNDS {
            app.add_sound(sound);
        }
        // Chained, and in this order: the engine's own `schedule`
        // (crates/atglyph-bevy/src/turn.rs) can write a `TurnEnd` and deal the
        // next actor's turn in the same pass, so `TurnSet::React` can see
        // the ending turn's `TurnEnd` together with the new turn's
        // `Struck`. Venting first reads each `Heat`'s `fired` as that
        // ending turn left it; running `heat_on_struck` first would mark
        // `fired` for the turn that is only just starting, and the turn
        // that actually just ended quietly would wrongly skip its vent.
        app.add_systems(Turn, (crate::heat::vent_heat, crate::heat::heat_on_struck).chain().in_set(TurnSet::React));
        // Ammunition is the engine's own plugin, added by `add_engine`,
        // and unordered against the heat chain above on purpose: `Heat`
        // and `Ammo` never share an item (`gear::Armory::load` refuses a
        // file that tries), so neither economy ever touches an entity the
        // other's systems do. Nor do the two ever need ordering against a
        // *different* actor's turn the way heat's pair does against a
        // `TurnEnd` the scheduler can deal in the same pass as the next
        // turn: `Resolution::claim` lets one actor resolve at most one
        // action per pass, so one actor's own shot and its own bag
        // changing can never land in the same pass to race each other.
        // Correct with or without a gear panel: `note_heat` reads
        // `GearView` as `Option<ResMut<_>>` and does nothing until a game
        // adds `GearViewPlugin`, as the binary's `GearPanel` does and a
        // headless test need not. An ammunition-fed weapon's own row is
        // `atglyph-ui`'s own to note now, in `ViewSet::Collect` rather than
        // here: the engine knows ammunition, and `Annotate` is only for
        // what it cannot know.
        app.add_systems(Update, crate::heat::note_heat.in_set(ViewSet::Annotate));
        // The pack's rows, the same way, with or without a pack panel.
        // After the heat notes, which fill another view but intern the
        // same facet keys, so each key is declared in one order every run;
        // nothing to order against for ammunition's own facet any more,
        // since `ViewSet::Collect`, where it is filled now, already runs
        // before `ViewSet::Annotate`, where this does, for every game,
        // ammunition-fed weapons or none.
        app.add_systems(Update, crate::gear::note_the_pack.after(crate::heat::note_heat).in_set(ViewSet::Annotate));
        // Only for the log, and only Foundry's own word choice: the engine
        // writes `AmmoChanged` for every weapon that dries or loads again,
        // and `narrate_ammo` is this game's reaction to it, the way
        // `heat::heat_on_struck` and `heat::vent_heat` tell their own
        // lines directly. After `AmmoSet::Resolve`, so it reads this
        // pass's own transitions rather than last pass's; after heat's
        // pair too, only for the log: every line Foundry tells in one pass
        // reads in one fixed order, the deck arrived on, then the alarm,
        // then the weapon's heat, then its ammunition, rather than
        // whichever order the executor ran the tellers in.
        app.add_systems(Turn, crate::ammo::narrate_ammo.after(crate::heat::heat_on_struck).after(atglyph::ecs::AmmoSet::Resolve).in_set(TurnSet::React));
        // The run's depth memory, one at the start, only ever raised, and
        // put back to one when the run ends: the next run in the same
        // launch, a restart's or one begun from the title, has been nowhere.
        app.init_resource::<crate::climb::Deepest>().reset_on_new_run::<crate::climb::Deepest>();
        // A deck fills the moment it is first entered: the engine's prefab slots in `PrefabSet::Fill`, then
        // its droids, then its props, then the engine's loot in
        // `LootSet::Scatter`, each from its own stream and each reading the
        // same `PlaceEntered` through its own cursor. The slots are filled
        // first, so the deck's own population and props, which keep off
        // every slot's cell, never stand where a guard or a locker goes.
        //
        // `remember_depth` goes first, ahead of anything of Foundry's that
        // spawns: it writes no entity of its own, but `populate_deck` will
        // read `Deepest` to draw a revisited deck's population at the band
        // the run has actually reached, and that read is only correct once
        // this same arrival's `Deepest` is current.
        //
        // Props before loot, because loot lands where nothing stands: an
        // item under a crate is an item nothing can pick up. The loot is
        // the engine's, so the chain is ordered before its set rather than
        // ending in it, and after the slots' set for the same reason.
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
            (crate::climb::remember_depth, crate::droids::populate_deck, crate::props::place_on_arrival, crate::mission::spawn_console_on_arrival)
                .chain()
                .in_set(TurnSet::React)
                .after(PrefabSet::Fill)
                .before(LootSet::Scatter),
        );
        // A probe's alarm: its line reacts to the same `Noticed` the
        // engine's own stealth writes, before heat's pair for the order of
        // the log, as above; its shout answers each action it finishes,
        // and touches nothing else of Foundry's.
        app.add_systems(
            Turn,
            (crate::droids::sound_alarm.before(crate::heat::vent_heat), crate::droids::shout_alarm, crate::droids::lower_alarm).in_set(TurnSet::React),
        )
        .add_systems(Turn, crate::droids::sense_alarm.in_set(PerceiveSet::Annotate));
        // Chained, and in this order: the engine's own `schedule` can
        // write a `TurnEnd` and deal the very next turn's `DamageDealt` in
        // the same pass, so `unjam_sensors` must count the turn that just
        // ended down before `jam_sensors` reads a hit that turn's actor
        // just landed (reversed, a fresh jam from that new hit would be
        // counted down before it had stood for even one whole turn of its
        // own); and both before the engine folds dark sight in
        // `ReactSet::Lend`, so it derives an actor's sight from the jam
        // exactly as this pass leaves it rather than as it stood before
        // either ran. `DarkSight` is otherwise never written anywhere in
        // this game: the engine's fold is its one owner.
        app.add_systems(Turn, (crate::droids::unjam_sensors, crate::droids::jam_sensors).chain().in_set(TurnSet::React).before(ReactSet::Lend));
        // The mission: loaded fresh every run, the way the roster and the
        // armory are, and before a continued run's restore, which writes
        // its progress into it.
        app.add_systems(PrepareRun, crate::mission::start);
        // The console is a prop, and its verb is Foundry's: declared once,
        // answered in `React` like every other reaction to a turn.
        app.add_verb(crate::mission::CHARGE);
        app.add_systems(Turn, crate::mission::answer_charge.in_set(TurnSet::React));
        // A game's reaction to the mission finishing, not to a turn
        // itself: an ordinary `Update` system, reading the `QuestChange`
        // the tracker wrote in the pass the fact landed. Before the input set, so the pick is open before any key handler
        // reads `Modals` that frame and a key cannot fall through to the
        // world while the pick is pending, and before the drawing.
        app.add_systems(Update, crate::mission::offer_the_pick.before(EngineSet::Input));
        // The lift out: a refused `GoThrough` on it, answered in the pass.
        app.add_systems(Turn, crate::lifts::ride_out.in_set(TurnSet::React));
        // Uplink's own reach bonus: after `heat`'s chain and ammunition's
        // own `AmmoSet::Resolve`, for the reason `upgrades::react_uplink`
        // gives.
        app.add_systems(Turn, crate::upgrades::react_uplink.after(crate::heat::heat_on_struck).after(atglyph::ecs::AmmoSet::Resolve).in_set(TurnSet::React));
        // The pick screen: declared while building, so `upgrades::modal`
        // finds it the moment
        // anything looks. `choice_keys` is exclusive (it calls
        // `upgrades::apply`, which needs the whole `World`), so it is
        // ordered the same place a game's own key handlers run, chained
        // below with the rest of the keys. Its drawing is not here:
        // `upgrades::ChoicePanel` takes a rectangle of the screen, so
        // `main.rs` adds it beside the engine's own panels.
        app.add_modal(crate::upgrades::MODAL);
        app.init_resource::<crate::upgrades::ChoiceScreen>();
        app.init_resource::<crate::upgrades::Choosing>();
        // The upgrades taken are the run's, emptied when it ends, as the
        // commando they were fitted to is gone with it.
        app.init_resource::<crate::upgrades::Taken>().reset_on_new_run::<crate::upgrades::Taken>();
        // The keys, declared once so the controls screen lists what these
        // systems read. Chained, the pick first: a confirm that closes the
        // pick must not fall through, the same frame, to the world's own
        // keys as a step or a lift. `f` and `t` hand the engine's
        // targeting cursor an `AimFire` or `AimThrow`; registering both
        // here is a no-op beside the cursor's own plugin and lets a
        // headless test with no cursor press the keys all the same.
        crate::input::declare_controls(app);
        app.add_message::<AimFire>().add_message::<AimThrow>();
        // The cheat screens first of all: a key one of them takes is never
        // also a pick, the lamp or a step. The search before the menu and
        // the menu before opening it, so the key that opens the search is
        // not typed into it and the key that closes the menu does not open
        // it again. The engine's standard keys after all of those, then why
        // `f` found nothing to fire, the same frame. The two toggles'
        // upkeep last, so one takes hold the frame it is pressed.
        app.add_plugins(crate::cheats::CheatsPlugin);
        app.add_systems(
            Update,
            (
                (crate::cheats::search_keys, crate::cheats::menu_keys, crate::cheats::open_cheats, crate::upgrades::choice_keys, crate::light::toggle_lamp)
                    .chain()
                    .before(PlayerKeysSet),
                (crate::input::say_why_not, crate::cheats::keep_godmode, crate::cheats::reveal_the_deck).chain().after(PlayerKeysSet),
            )
                .in_set(EngineSet::Input),
        );
        // The lifts between decks, laid on first arrival beside everything
        // else a deck fills with, and the line the log gives each deck.
        // First of Foundry's lines in a pass, for the order of the log.
        app.add_systems(Turn, crate::lifts::link_decks.before(crate::droids::sound_alarm).in_set(TurnSet::React));
    }

    /// Every line Foundry says from inside a turn is a `Tell`, which only
    /// the engine's narrator collects and speaks.
    fn finish(&self, app: &mut App) {
        app.depends_on::<NarrationViewPlugin>("FoundryPlugin");
    }
}

#[cfg(test)]
mod ambiguity;
