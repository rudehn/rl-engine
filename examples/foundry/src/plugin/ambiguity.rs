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

/// A name for each of Foundry's systems, and for each engine system
/// one of them meets, for a failure to read by.
fn names() -> Vec<(&'static str, TypeId)> {
    use crate::*;
    use rl_engine::rl_bevy::{ability, combat, consumable, items, minds, props as engine_props, remains, stealth, throwing};
    vec![
        ("run::start", id(run::start)),
        ("heat::vent_heat", id(heat::vent_heat)),
        ("heat::heat_on_struck", id(heat::heat_on_struck)),
        ("heat::note_heat", id(heat::note_heat)),
        ("ammo::narrate_ammo", id(ammo::narrate_ammo)),
        ("gear::note_the_pack", id(gear::note_the_pack)),
        ("engine ammo::spend_ammo", id(rl_engine::rl_bevy::ammo::spend_ammo)),
        ("engine ammo::sync_ammo", id(rl_engine::rl_bevy::ammo::sync_ammo)),
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
        ("engine loot::scatter_places", id(rl_engine::rl_bevy::loot::scatter_places::<gear::Armory>)),
        ("engine loot::drop_on_death", id(rl_engine::rl_bevy::loot::drop_on_death::<gear::Armory>)),
        ("engine loot::fill_containers", id(rl_engine::rl_bevy::loot::fill_containers::<gear::Armory>)),
        ("engine save::refresh_stash", id(rl_engine::rl_save::run::refresh_stash)),
        ("engine save::save_on_arrival", id(rl_engine::rl_save::save_on_arrival)),
        ("engine save::flush_on_exit", id(rl_engine::rl_save::unload::flush_on_exit)),
        ("engine save::forget_save", id(rl_engine::rl_save::forget_save)),
        ("engine combat::bury_the_dead", id(rl_engine::rl_bevy::combat::bury_the_dead)),
        ("engine consumable::bury_spent", id(rl_engine::rl_bevy::bury_spent)),
        ("engine plugin::restart_runs", id(rl_engine::rl_bevy::plugin::restart_runs)),
        ("upgrades::react_uplink", id(upgrades::react_uplink)),
        ("upgrades::choice_keys", id(upgrades::choice_keys)),
        ("light::toggle_lamp", id(light::toggle_lamp)),
        ("input::say_why_not", id(input::say_why_not)),
        ("engine player_keys::player_keys", id(rl_engine::rl_ui::player_keys::player_keys)),
        ("cheats::search_keys", id(cheats::search_keys)),
        ("cheats::menu_keys", id(cheats::menu_keys)),
        ("cheats::open_cheats", id(cheats::open_cheats)),
        ("cheats::keep_godmode", id(cheats::keep_godmode)),
        ("cheats::reveal_the_deck", id(cheats::reveal_the_deck)),
        ("lifts::link_decks", id(lifts::link_decks)),
        ("engine stealth::wake_on_damage", id(stealth::wake_on_damage)),
        ("engine combat::resolve_attacks", id(combat::resolve_attacks)),
        ("engine combat::land_shots", id(combat::land_shots)),
        ("engine items::resolve_items", id(items::resolve_items)),
        ("engine items::fold_gear", id(items::fold_gear)),
        ("engine ability::resolve_abilities", id(ability::resolve_abilities)),
        ("engine ability::land_abilities", id(ability::land_abilities)),
        ("engine ability::refresh_known", id(ability::refresh_known)),
        ("engine throwing::resolve_throws", id(throwing::resolve_throws)),
        ("engine throwing::land_throws", id(throwing::land_throws)),
        ("props::place_on_arrival", id(props::place_on_arrival)),
        ("props::wreck_the_dead", id(props::wreck_the_dead)),
        ("props::spend_the_keycard", id(props::spend_the_keycard)),
        ("mission::answer_charge", id(mission::answer_charge)),
        ("engine props::offer_here", id(engine_props::offer_here)),
        ("engine props::resolve_interactions", id(engine_props::resolve_interactions)),
        ("engine props::resolve_takes", id(engine_props::resolve_takes)),
        ("engine props::report_entered", id(engine_props::report_entered)),
        ("engine props::report_destroyed", id(engine_props::report_destroyed)),
        ("engine props::arm_props", id(engine_props::arm_props)),
        ("engine effects::land_triggers", id(rl_engine::rl_bevy::land_triggers)),
        ("engine props::close_emptied_containers", id(engine_props::close_emptied_containers)),
        ("engine props::stock_containers", id(engine_props::stock_containers)),
        ("engine props::build_prop_effects", id(engine_props::build_prop_effects)),
        ("engine props::spot_hidden_props", id(engine_props::spot_hidden_props)),
        ("engine props::perceive_props", id(engine_props::perceive_props)),
        ("engine props::report_bare_props", id(engine_props::report_bare_props)),
        ("engine remains::leave_remains", id(remains::leave_remains)),
        ("engine minds::sense", id(minds::sense)),
        ("engine minds::begin_thinking", id(minds::begin_thinking)),
        ("engine minds::perceive_roster", id(minds::perceive_roster)),
        ("engine stealth::filter_unnoticed", id(stealth::filter_unnoticed)),
        ("engine items::perceive_belongings", id(items::perceive_belongings)),
        ("engine consumable::spend_charges", id(consumable::spend_charges)),
        ("engine consumable::recharge_charges", id(consumable::recharge_charges)),
        ("engine combat::perceive_reach", id(combat::perceive_reach)),
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
fn allowed(world: &World) -> Vec<Allowed> {
    use crate::*;
    use rl_engine::rl_bevy::{ability, combat, items, props as engine_props, stealth, throwing};
    let ids = ids(world);
    let on = |names: &[&str]| -> Vec<ComponentId> { ambiguity::on(&ids, names) };
    let pair = |a, b, what: &[&str], why| Allowed::pair(a, b, on(what), why);
    let claims = ["Acting", "Messages<ActionDone>", "Messages<ActionRefused>"];
    let allowed = vec![
        Allowed::any(on(&claims), "any two resolvers or sweepers: Resolution::claim spends one actor's one turn once a pass"),
        pair(
            id(droids::lower_alarm),
            id(stealth::wake_on_damage),
            &["Aware"],
            "an alarm is lowered only on a probe that knows of nobody; one a blow wakes in the same pass either keeps its alarm or signals again, and both are right for a probe that had lost the commando",
        ),
        Allowed::with_any(
            id(droids::sense_alarm),
            on(&["Thinking"]),
            "one contributor per field of the snapshot: the probe's sense pushes `Signal`, a sense of its own type, and nothing else, and it is sorted once after them all",
        ),
        pair(
            id(droids::shout_alarm),
            id(stealth::wake_on_damage),
            &["Aware"],
            "a probe shouts in its own pass and strikes nobody, so no blow wakes it in the pass it shouts in",
        ),
        // Props, and what they meet. Every entry below is about one of
        // three invariants: one action a pass, one contributor per field of
        // the snapshot, and a container's bag is never a commando's.
        Allowed::with_any(
            id(engine_props::perceive_props),
            on(&["Thinking"]),
            "one contributor per field of the snapshot: props fill `props` and nothing else, and it is sorted once after them all",
        ),
        pair(
            id(engine_props::resolve_interactions),
            id(engine_props::resolve_takes),
            // The pack a choosing offer's thing is checked in, as a take
            // fills it.
            &[&claims[..], &["Position", "Inventory"]].concat(),
            "one action a pass: opening a crate and taking out of it are two actions, never resolved in the same pass",
        ),
        pair(
            id(engine_props::resolve_interactions),
            id(throwing::resolve_throws),
            &[&claims[..], &["Occupancy", "Messages<Cued>", "Messages<DamageEvent>", "Position", "Inventory"]].concat(),
            "one action a pass: an interaction and a throw are never resolved in the same one",
        ),
        pair(
            id(engine_props::resolve_interactions),
            id(combat::resolve_attacks),
            &[&claims[..], &["Occupancy", "Messages<Cued>", "Messages<DamageEvent>", "Position", "Viewshed"]].concat(),
            "one action a pass: an interaction and an attack are never resolved in the same one",
        ),
        pair(
            id(engine_props::resolve_takes),
            id(combat::resolve_attacks),
            &[&claims[..], &["Equipped"]].concat(),
            "one action a pass: taking out of a container and an attack are never resolved in the same one",
        ),
        pair(
            id(engine_props::resolve_interactions),
            id(ability::resolve_abilities),
            &[
                &claims[..],
                &[
                    "Occupancy",
                    "Messages<Cued>",
                    "Messages<DamageEvent>",
                    "Messages<Heal>",
                    "Messages<Afflict>",
                    "Messages<Cure>",
                    "Position",
                    "Viewshed",
                    "EffectRng",
                ],
            ]
            .concat(),
            "one action a pass: an interaction and an ability are never resolved in the same one",
        ),
        pair(
            id(engine_props::resolve_interactions),
            id(items::resolve_items),
            // Who stands where and what the user sees, for an aimed use's
            // refusals, read as an ability's resolver reads them.
            &[&claims[..], &["Position", "Occupancy", "Viewshed", "Inventory"]].concat(),
            "one action a pass: an interaction and an item action are never resolved in the same one",
        ),
        pair(
            id(engine_props::resolve_takes),
            id(throwing::resolve_throws),
            &[&claims[..], &["Messages<ItemEvent>", "Equipped", "Inventory", "Stack"]].concat(),
            "one action a pass: taking out of a crate and throwing are never resolved in the same one",
        ),
        pair(
            id(engine_props::resolve_takes),
            id(ability::resolve_abilities),
            &[&claims[..], &["Position", "Equipped", "Inventory", "Stack"]].concat(),
            "one action a pass: taking out of a crate and using an ability are never resolved in the same one",
        ),
        pair(
            id(engine_props::resolve_takes),
            id(items::resolve_items),
            &[&claims[..], &["Messages<ItemEvent>", "Equipped", "Inventory"]].concat(),
            "one action a pass: taking out of a crate and picking up are never resolved in the same one",
        ),
        // What lands only ever lands in a pass that dealt nobody a turn,
        // since nothing is dealt while it flies, so never beside an
        // interaction, which is an action somebody spent a turn on.
        pair(
            id(engine_props::resolve_interactions),
            id(ability::land_abilities),
            &[
                "Occupancy",
                "Messages<Cued>",
                "Messages<DamageEvent>",
                "Messages<Heal>",
                "Messages<Afflict>",
                "Messages<Cure>",
                "Position",
                "Viewshed",
                "EffectRng",
            ],
            "an ability lands in a pass no interaction is resolved in",
        ),
        pair(
            id(engine_props::resolve_interactions),
            id(throwing::land_throws),
            &["Occupancy", "Messages<Cued>", "Messages<DamageEvent>", "Position", "Inventory"],
            "a throw lands in a pass no interaction is resolved in",
        ),
        pair(
            id(engine_props::resolve_interactions),
            id(combat::land_shots),
            &["Messages<DamageEvent>", "Position"],
            "a shot lands in a pass no interaction is resolved in",
        ),
        pair(id(engine_props::resolve_takes), id(ability::land_abilities), &["Position"], "an ability lands in a pass nothing is taken in"),
        pair(
            id(engine_props::resolve_takes),
            id(throwing::land_throws),
            &["Messages<ItemEvent>", "Equipped", "Inventory", "Stack"],
            "a throw lands in a pass nothing is taken in",
        ),
        // A trap springs on the one who stepped, or on what broke. Every
        // trap in `props.ron` lands `Harm` and nothing that moves anyone or
        // jams anything, so a trap never changes what a probe reads or
        // afflicts what an ion hit would.
        // The keycard is taken in the same pass the locker it opened was
        // opened in, which is a pass nothing else spent a slug or fired in.
        pair(
            id(props::spend_the_keycard),
            id(rl_engine::rl_bevy::ammo::spend_ammo),
            &["Inventory", "Stack"],
            "one action a pass: a locker opened and a shot fired are never the same one",
        ),
        pair(
            id(props::spend_the_keycard),
            id(rl_engine::rl_bevy::ammo::sync_ammo),
            &["Inventory", "Stack"],
            "a card is not a slug, and a bag with one card fewer still holds whatever the guns draw from",
        ),
        pair(
            id(props::spend_the_keycard),
            id(engine_props::close_emptied_containers),
            &["Inventory"],
            "a crate's bag and a commando's are never the same bag, and only a commando carries a card",
        ),
        Allowed::with_any(
            id(props::spend_the_keycard),
            on(&["Messages<Tell>"]),
            "every reaction writes its own line for the pass, and the narrator speaks them after it; two lines that answer different things say nothing by their order",
        ),
        // A container's bag and a commando's are never the same bag.
        pair(
            id(engine_props::close_emptied_containers),
            id(rl_engine::rl_bevy::ammo::spend_ammo),
            &["Inventory"],
            "a crate's bag and a commando's are never the same bag, and only a commando fires",
        ),
        // Every line a pass leaves is its own, and so is every fact: the
        // lift out answers a refused step through it, the console a charge
        // set, and one pass holds one of the player's actions, never both.
        Allowed::with_any(
            id(lifts::ride_out),
            on(&["Messages<Tell>", "Messages<Happened>"]),
            "every reaction writes its own line and its own fact for the pass; the narrator speaks the lines after it and the tracker counts the facts in any order, so two that answer different things say nothing by their order",
        ),
        Allowed::with_any(
            id(mission::answer_charge),
            on(&["Messages<Tell>"]),
            "every reaction writes its own line for the pass, and the narrator speaks them after it; two lines that answer different things say nothing by their order",
        ),
        // The repair drone: a sense of its own, and a line when a wreck
        // stands back up.
        Allowed::with_any(
            id(droids::sense_wrecks),
            on(&["Thinking"]),
            "one contributor per field of the snapshot: the drone's sense pushes `Wrecks`, a sense of its own type, and nothing else, and it is sorted once after them all",
        ),
        Allowed::with_any(
            id(droids::rebuild_wrecks),
            on(&["Messages<Tell>"]),
            "every reaction writes its own line for the pass, and the narrator speaks them after it; two lines that answer different things say nothing by their order",
        ),
    ];
    allowed
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
