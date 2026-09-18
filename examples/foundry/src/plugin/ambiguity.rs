//! Bevy's ambiguity detection over Foundry's schedules: every pair of
//! Foundry's systems that touch the same data unordered, with at least one
//! writing it, is either ordered or allowed here with its reason.
//!
//! A chain that goes missing still passes every behaviour test while Bevy
//! happens to run the pair in the order they were declared; this is what
//! notices.

use std::any::TypeId;

use bevy::ecs::component::ComponentId;
use bevy::ecs::schedule::{InternedScheduleLabel, Schedules};
use bevy::prelude::*;
use rl_engine::rl_bevy::turn::{Acting, ActionDone, ActionRefused, Turns};
use rl_engine::rl_bevy::{Aware, DarkSight, Inventory, Noticed, Position, RangedAttack};
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_ui::{Facets, GearView, MessageLog, Modals};

/// Every system in every schedule of `app`, by schedule and by the
/// type of the system, each schedule built first so its conflicts are
/// known. By type, not by name: without Bevy's `debug` feature, which
/// the workspace does not build with, every system is named alike.
fn systems(app: &mut App) -> Vec<(InternedScheduleLabel, TypeId)> {
    let world = app.world_mut();
    let labels: Vec<_> = world.resource::<Schedules>().iter().map(|(_, schedule)| schedule.label()).collect();
    let mut all = Vec::new();
    for label in labels {
        world.schedule_scope(label, |world, schedule| {
            schedule.initialize(world).expect("every schedule builds");
            all.extend(schedule.systems().expect("just initialized").map(|(_, system)| (label, system.system_type())));
        });
    }
    all
}

/// The type a function registers as, to recognise it among a schedule's
/// systems.
fn id<M>(system: impl IntoSystem<(), (), M>) -> TypeId {
    IntoSystem::into_system(system).system_type()
}

/// A name for each of Foundry's systems, and for each engine system
/// one of them meets, for a failure to read by.
fn names() -> Vec<(&'static str, TypeId)> {
    use crate::*;
    use rl_engine::rl_bevy::{ability, combat, items, stealth, throwing};
    vec![
        ("run::start", id(run::start)),
        ("heat::vent_heat", id(heat::vent_heat)),
        ("heat::heat_on_struck", id(heat::heat_on_struck)),
        ("heat::note_heat", id(heat::note_heat)),
        ("ammo::note_ammo", id(ammo::note_ammo)),
        ("ammo::spend_ammo", id(ammo::spend_ammo)),
        ("ammo::sync_ammo", id(ammo::sync_ammo)),
        ("droids::populate_deck", id(droids::populate_deck)),
        ("loot::scatter_on_arrival", id(loot::scatter_on_arrival)),
        ("loot::drop_on_death", id(loot::drop_on_death)),
        ("droids::sound_alarm", id(droids::sound_alarm)),
        ("droids::unjam_sensors", id(droids::unjam_sensors)),
        ("droids::jam_sensors", id(droids::jam_sensors)),
        ("droids::sync_dark_sight", id(droids::sync_dark_sight)),
        ("mission::start", id(mission::start)),
        ("mission::spawn_console_on_arrival", id(mission::spawn_console_on_arrival)),
        ("mission::resolve_set_charge", id(mission::resolve_set_charge)),
        ("mission::offer_the_pick", id(mission::offer_the_pick)),
        ("upgrades::react_uplink", id(upgrades::react_uplink)),
        ("upgrades::choice_keys", id(upgrades::choice_keys)),
        ("light::toggle_lamp", id(light::toggle_lamp)),
        ("input::player_input", id(input::player_input)),
        ("lifts::link_decks", id(lifts::link_decks)),
        ("light::set_ambient", id(light::set_ambient)),
        ("light::light_the_lamps", id(light::light_the_lamps)),
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
    ]
}

/// Two systems that may conflict on `on` without being ordered, and
/// why that is harmless. `None` stands for any system.
struct Allowed {
    a: Option<TypeId>,
    b: Option<TypeId>,
    on: Vec<ComponentId>,
    why: &'static str,
}

/// The component or resource ids a test names, by what they are.
fn ids(world: &World) -> Vec<(&'static str, ComponentId)> {
    let c = world.components();
    let found = [
        ("MessageLog", c.component_id::<MessageLog>()),
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
        ("Stowed", c.component_id::<crate::heat::Stowed>()),
        ("DarkSight", c.component_id::<DarkSight>()),
        ("Jammed", c.component_id::<crate::droids::Jammed>()),
    ];
    found.into_iter().map(|(name, id)| (name, id.unwrap_or_else(|| panic!("{name} is registered once every schedule is built")))).collect()
}

/// Every conflicting pair Foundry leaves unordered on purpose, each with
/// its reason. A pair not here fails the test, and so does an entry no
/// pair needs any more.
fn allowed(world: &World) -> Vec<Allowed> {
    use crate::*;
    use rl_engine::rl_bevy::{ability, combat, stealth, throwing};
    let ids = ids(world);
    let on = |names: &[&str]| -> Vec<ComponentId> { names.iter().map(|n| ids.iter().find(|(name, _)| name == n).expect("named in ids").1).collect() };
    let pair = |a, b, what: &[&str], why| Allowed { a: Some(a), b: Some(b), on: on(what), why };
    let claims = ["Acting", "Messages<ActionDone>", "Messages<ActionRefused>"];
    let mut allowed = vec![
        Allowed { a: None, b: None, on: on(&claims), why: "any two resolvers or sweepers: Resolution::claim spends one actor's one turn once a pass" },
        pair(
            id(droids::sound_alarm),
            id(stealth::wake_on_damage),
            &["Aware", "Messages<Noticed>"],
            "both only raise awareness; one sees the other's a pass late",
        ),
        pair(id(ammo::note_ammo), id(heat::note_heat), &["GearView", "Facets"], "no weapon has both Ammo and Heat, so no row gets a facet from both"),
        // Something lands only in a pass that dealt nobody a turn, since
        // nothing is dealt while it flies, so never beside a charge.
        pair(id(mission::resolve_set_charge), id(throwing::land_throws), &["Turns"], "a throw lands in a pass no charge is set in"),
        pair(id(mission::resolve_set_charge), id(combat::land_shots), &["Turns"], "a shot lands in a pass no charge is set in"),
        pair(id(mission::resolve_set_charge), id(ability::land_abilities), &["Turns", "Position"], "an ability lands in a pass no charge is set in"),
        pair(
            id(mission::resolve_set_charge),
            id(ability::resolve_abilities),
            &[&claims[..], &["Position"]].concat(),
            "one action a pass: a charge and an ability are never resolved in the same one",
        ),
        pair(id(ability::refresh_known), id(ammo::spend_ammo), &["Inventory"], "Known is rebuilt every pass, and a slug grants nothing"),
        pair(id(ability::refresh_known), id(ammo::sync_ammo), &["Inventory"], "Known is rebuilt every pass, and a slug grants nothing"),
    ];
    // Five systems write the log in `TurnSet::React`; these are the
    // pairs of them nothing else happens to order.
    let logs = [
        (id(heat::vent_heat), id(droids::sound_alarm)),
        (id(heat::vent_heat), id(lifts::link_decks)),
        (id(heat::heat_on_struck), id(droids::sound_alarm)),
        (id(heat::heat_on_struck), id(ammo::sync_ammo)),
        (id(heat::heat_on_struck), id(lifts::link_decks)),
        (id(droids::sound_alarm), id(ammo::sync_ammo)),
        (id(droids::sound_alarm), id(lifts::link_decks)),
        (id(ammo::sync_ammo), id(lifts::link_decks)),
    ];
    for (a, b) in logs {
        allowed.push(pair(a, b, &["MessageLog"], "two log lines in one pass: only their order within it"));
    }
    allowed
}

#[test]
fn every_conflicting_pair_of_foundrys_systems_is_ordered_or_allowed_for_a_stated_reason() {
    let mut app = crate::testing::headless(RunSeed(0));
    let with = systems(&mut app);
    let mut without = systems(&mut crate::testing::headless_without_foundry(RunSeed(0)));
    // Foundry's own: whatever FoundryPlugin added, by schedule and type,
    // so a system added later is covered without being listed anywhere.
    let mut ours = Vec::new();
    for entry in with {
        match without.iter().position(|e| *e == entry) {
            Some(i) => {
                without.swap_remove(i);
            }
            None => ours.push(entry),
        }
    }
    let world = app.world();
    let (allowed, names, ids) = (allowed(world), names(), ids(world));
    let fits = |rule: Option<TypeId>, t: TypeId| rule.is_none_or(|r| r == t);
    let mut used = vec![false; allowed.len()];
    let mut unexplained = Vec::new();
    for (_, schedule) in world.resource::<Schedules>().iter() {
        let label = schedule.label();
        let types: Vec<_> = schedule.systems().expect("initialized").map(|(key, system)| (key, system.system_type())).collect();
        let type_of = |k| types.iter().find(|(key, _)| *key == k).map(|(_, t)| *t).expect("a system of this schedule");
        for (a, b, on) in schedule.graph().conflicting_systems().iter() {
            let (a, b) = (type_of(*a), type_of(*b));
            if !ours.contains(&(label, a)) && !ours.contains(&(label, b)) {
                continue;
            }
            // An empty list is a conflict over the whole world, an
            // exclusive system's, which no entry allows.
            let rule = allowed
                .iter()
                .position(|r| ((fits(r.a, a) && fits(r.b, b)) || (fits(r.a, b) && fits(r.b, a))) && !on.is_empty() && on.iter().all(|c| r.on.contains(c)));
            match rule {
                Some(i) => used[i] = true,
                None => {
                    let system = |t: TypeId| names.iter().find(|(_, n)| *n == t).map_or_else(|| format!("{t:?}"), |(n, _)| n.to_string());
                    let what: Vec<String> =
                        on.iter().map(|c| ids.iter().find(|(_, id)| id == c).map_or_else(|| format!("{c:?}"), |(n, _)| n.to_string())).collect();
                    unexplained.push(format!("{label:?}: {} and {} on {what:?}", system(a), system(b)));
                }
            }
        }
    }
    assert!(
        unexplained.is_empty(),
        "{} unordered conflicting pairs of Foundry's; order them, or allow them with a reason:\n{}",
        unexplained.len(),
        unexplained.join("\n")
    );
    let system = |t: Option<TypeId>| {
        t.map_or_else(|| "any system".to_string(), |t| names.iter().find(|(_, n)| *n == t).map_or_else(|| format!("{t:?}"), |(n, _)| n.to_string()))
    };
    let stale: Vec<String> =
        allowed.iter().zip(&used).filter(|(_, used)| !**used).map(|(r, _)| format!("{} and {}: {}", system(r.a), system(r.b), r.why)).collect();
    assert!(stale.is_empty(), "allowed pairs that no longer conflict, to take off the list: {stale:#?}");
}
