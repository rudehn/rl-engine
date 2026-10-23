//! Bevy's ambiguity detection over Foundry's schedules: every pair of
//! Foundry's systems that touch the same data unordered, with at least one
//! writing it, is either ordered or allowed here with its reason.
//!
//! The check itself is the engine's,
//! [`rl_bevy::testing::ambiguity`](rl_engine::rl_bevy::testing::ambiguity);
//! what is Foundry's is the names a failure reads by and the pairs it
//! leaves unordered on purpose, each with why.

use std::any::TypeId;

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use rl_engine::rl_bevy::testing::ambiguity::{self, Allowed, system_type as id};
use rl_engine::rl_bevy::turn::{Acting, ActionDone, ActionRefused, Turns};
use rl_engine::rl_bevy::{Aware, DarkSight, Inventory, Noticed, Position, RangedAttack};
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_ui::{Facets, GearView, MessageLog, Modals};

/// A name for each of Foundry's systems, for a failure to read by. An
/// engine system a failure meets reads by Bevy's own name for it.
fn names() -> Vec<(&'static str, TypeId)> {
    use crate::*;
    vec![
        ("run::start", id(run::start)),
        ("heat::vent_heat", id(heat::vent_heat)),
        ("heat::heat_on_struck", id(heat::heat_on_struck)),
        ("heat::note_heat", id(heat::note_heat)),
        ("ammo::narrate_ammo", id(ammo::narrate_ammo)),
        ("gear::note_the_pack", id(gear::note_the_pack)),
        ("droids::populate_deck", id(droids::populate_deck)),
        ("droids::sound_alarm", id(droids::sound_alarm)),
        ("droids::sense_alarm", id(droids::sense_alarm)),
        ("droids::lower_alarm", id(droids::lower_alarm)),
        ("droids::sense_wrecks", id(droids::sense_wrecks)),
        ("droids::rebuild_wrecks", id(droids::rebuild_wrecks)),
        ("droids::shout_alarm", id(droids::shout_alarm)),
        ("droids::unjam_sensors", id(droids::unjam_sensors)),
        ("droids::jam_sensors", id(droids::jam_sensors)),
        ("mission::start", id(mission::start)),
        ("mission::spawn_console_on_arrival", id(mission::spawn_console_on_arrival)),
        ("mission::answer_charge", id(mission::answer_charge)),
        ("mission::offer_the_pick", id(mission::offer_the_pick)),
        ("lifts::ride_out", id(lifts::ride_out)),
        ("run::resume", id(run::resume)),
        ("gear::load_armory", id(gear::load_armory)),
        ("upgrades::react_uplink", id(upgrades::react_uplink)),
        ("upgrades::choice_keys", id(upgrades::choice_keys)),
        ("light::toggle_lamp", id(light::toggle_lamp)),
        ("input::say_why_not", id(input::say_why_not)),
        ("cheats::search_keys", id(cheats::search_keys)),
        ("cheats::menu_keys", id(cheats::menu_keys)),
        ("cheats::open_cheats", id(cheats::open_cheats)),
        ("cheats::keep_godmode", id(cheats::keep_godmode)),
        ("cheats::reveal_the_deck", id(cheats::reveal_the_deck)),
        ("lifts::link_decks", id(lifts::link_decks)),
        ("props::place_on_arrival", id(props::place_on_arrival)),
        ("props::wreck_the_dead", id(props::wreck_the_dead)),
        ("props::spend_the_keycard", id(props::spend_the_keycard)),
    ]
}

/// The component or resource ids a test names, by what they are.
fn ids(world: &World) -> Vec<(&'static str, ComponentId)> {
    let c = world.components();
    let found = [
        ("MessageLog", c.component_id::<MessageLog>()),
        ("Messages<Happened>", c.component_id::<Messages<rl_engine::rl_bevy::Happened>>()),
        ("GearView", c.component_id::<GearView>()),
        ("Facets", c.component_id::<Facets>()),
        ("Modals", c.component_id::<Modals>()),
        ("Aware", c.component_id::<Aware>()),
        ("Messages<Noticed>", c.component_id::<Messages<Noticed>>()),
        ("Acting", c.component_id::<Acting>()),
        ("Messages<ActionDone>", c.component_id::<Messages<ActionDone>>()),
        ("Messages<ActionRefused>", c.component_id::<Messages<ActionRefused>>()),
        ("Turns", c.component_id::<Turns>()),
        ("Inventory", c.component_id::<Inventory>()),
        ("Position", c.component_id::<Position>()),
        ("RangedAttack", c.component_id::<RangedAttack>()),
        ("Stowed", c.component_id::<rl_engine::rl_bevy::Stowed>()),
        ("DarkSight", c.component_id::<DarkSight>()),
        ("Jammed", c.component_id::<crate::droids::Jammed>()),
        // What props, traps and the lines a pass leaves brought in.
        ("Messages<Tell>", c.component_id::<Messages<rl_engine::rl_ui::Tell>>()),
        ("Thinking", c.component_id::<rl_engine::rl_bevy::Thinking>()),
        ("Messages<Cued>", c.component_id::<Messages<rl_engine::rl_bevy::Cued>>()),
        ("Messages<DamageEvent>", c.component_id::<Messages<rl_engine::rl_bevy::DamageEvent>>()),
        ("Messages<Heal>", c.component_id::<Messages<rl_engine::rl_bevy::Heal>>()),
        ("Messages<Afflict>", c.component_id::<Messages<rl_engine::rl_bevy::Afflict>>()),
        ("Messages<Cure>", c.component_id::<Messages<rl_engine::rl_bevy::Cure>>()),
        ("Messages<FillContainer>", c.component_id::<Messages<rl_engine::rl_bevy::FillContainer>>()),
        ("Messages<Interacted>", c.component_id::<Messages<rl_engine::rl_bevy::Interacted>>()),
        ("EffectRng", c.component_id::<rl_engine::rl_bevy::EffectRng>()),
        ("Viewshed", c.component_id::<rl_engine::rl_bevy::Viewshed>()),
        ("Equipped", c.component_id::<rl_engine::rl_bevy::Equipped>()),
        ("PropKind", c.component_id::<rl_engine::rl_bevy::PropKind>()),
        ("Occupancy", c.component_id::<rl_engine::rl_bevy::turn::Occupancy>()),
        ("Stack", c.component_id::<rl_engine::rl_bevy::Stack>()),
        ("Consumable", c.component_id::<rl_engine::rl_bevy::Consumable>()),
        ("Triggers", c.component_id::<rl_engine::rl_bevy::Triggers>()),
        ("Messages<ItemEvent>", c.component_id::<Messages<rl_engine::rl_bevy::ItemEvent>>()),
        ("Messages<Fired>", c.component_id::<Messages<rl_engine::rl_bevy::Fired>>()),
    ];
    ambiguity::data(found)
}

/// Every conflicting pair Foundry leaves unordered on purpose, each with
/// its reason. A pair not here fails the test, and so does an entry no
/// pair needs any more.
///
/// Each names one of Foundry's own systems and the data, never an engine
/// system: the engine's systems are its own to rename, and a reason here
/// holds for whichever engine system meets Foundry's on that data. The
/// pairs of two engine systems are the engine's own test's.
fn allowed(world: &World) -> Vec<Allowed> {
    use crate::*;
    let ids = ids(world);
    let on = |names: &[&str]| -> Vec<ComponentId> { ambiguity::on(&ids, names) };
    let lines = "every reaction writes its own line for the pass, and the narrator speaks them after it; two lines that answer different things say nothing by their order";
    let senses = "one contributor per field of the snapshot: a sense of Foundry's pushes a sense of its own type and nothing else, and the snapshot is sorted once after every contributor";
    vec![
        // A probe's awareness: the engine changes it only for what was
        // done to the probe, a blow or a noise, and never in the pass the
        // probe itself acted in.
        Allowed::with_any(
            id(droids::lower_alarm),
            on(&["Aware"]),
            "an alarm is lowered only on a probe that knows of nobody; one made aware of somebody in the same pass either keeps its alarm or signals again, and both are right for a probe that had lost the commando",
        ),
        Allowed::with_any(
            id(droids::shout_alarm),
            on(&["Aware"]),
            "a probe shouts in its own pass and strikes nobody, so nothing the engine makes it aware of lands in the pass it shouts in",
        ),
        Allowed::with_any(id(droids::sense_alarm), on(&["Thinking"]), senses),
        Allowed::with_any(id(droids::sense_wrecks), on(&["Thinking"]), senses),
        // The keycard is taken in the pass the locker it opened was opened
        // in, an action of the commando's own, so nothing else was fired,
        // taken or put away in it; and a card is no slug and in no crate,
        // so a bag with one card fewer reads the same to whatever counts
        // what else it holds.
        Allowed::with_any(
            id(props::spend_the_keycard),
            on(&["Inventory", "Stack"]),
            "one action a pass: a locker opened with a card and anything else done with the pack are never the same one, and a card is neither a slug nor a crate's",
        ),
        Allowed::with_any(id(props::spend_the_keycard), on(&["Messages<Tell>"]), lines),
        // Every line a pass leaves is its own, and so is every fact: the
        // lift out answers a refused step through it, the console a charge
        // set, and one pass holds one of the player's actions, never both.
        Allowed::with_any(
            id(lifts::ride_out),
            on(&["Messages<Tell>", "Messages<Happened>"]),
            "every reaction writes its own line and its own fact for the pass; the narrator speaks the lines after it and the tracker counts the facts in any order, so two that answer different things say nothing by their order",
        ),
        Allowed::with_any(id(mission::answer_charge), on(&["Messages<Tell>"]), lines),
        Allowed::with_any(id(droids::rebuild_wrecks), on(&["Messages<Tell>"]), lines),
    ]
}

#[test]
fn every_conflicting_pair_of_foundrys_systems_is_ordered_or_allowed_for_a_stated_reason() {
    let mut app = crate::testing::headless(RunSeed(0));
    // Foundry's own: whatever FoundryPlugin added, by schedule and type,
    // so a system added later is covered without being listed anywhere.
    let ours = ambiguity::added(&mut app, &mut crate::testing::headless_without_foundry(RunSeed(0)));
    let world = app.world();
    ambiguity::assert_ordered(world, "Foundry", &ours, &allowed(world), &names(), &ids(world));
}
