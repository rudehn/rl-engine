//! Bevy's ambiguity detection over the engine's own schedules: every pair
//! of the engine's systems that touch the same data unordered, with at
//! least one writing it, is either ordered or allowed here with its reason.
//!
//! A game's own test, [`testing::ambiguity`](crate::testing::ambiguity),
//! checks only the pairs with one of the game's systems in them, and names
//! no engine function, since a game describes itself through the engine's
//! public API alone. The pairs of two engine systems are the engine's to
//! answer for, so they are answered here, exactly, by the functions' types.

use std::any::TypeId;

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;

use crate::testing::ambiguity::{self, Allowed, system_type as id};

/// Every engine plugin, over the test makers the engine's own prefab and
/// loot tests use, never a list taken from a game.
fn engine() -> App {
    use crate::app::EngineApp;
    use crate::loot::tests::Toys;
    use crate::noise::NoiseRules;
    use crate::prefabs::tests::Beasts;
    let mut app = crate::testing::headless_app();
    app.add_plugins((
        crate::fov::FovPlugin,
        crate::combat::CombatPlugin,
        crate::minds::MindsPlugin,
        crate::items::ItemsPlugin,
        crate::throwing::ThrowingPlugin,
        crate::lighting::LightingPlugin,
        crate::stealth::StealthPlugin,
    ))
    .add_plugins((
        crate::events::FactsPlugin,
        crate::ability::AbilitiesPlugin,
        crate::fire::FirePlugin,
        crate::gas::GasPlugin,
        crate::noise::NoisePlugin::new(NoiseRules { step: 2, strike: 10, door: 6, landing: 6, door_muffle: 3 }),
        crate::props::PropsPlugin,
        crate::ammo::AmmoPlugin,
    ))
    .add_plugins((
        crate::prefabs::PrefabPlugin::<Beasts, Toys>::default(),
        crate::loot::LootPlugin::<Toys>::default(),
        crate::records::RecordsPlugin,
        crate::remains::RemainsPlugin,
        crate::trade::TradePlugin,
    ))
    .add_engine_effects();
    app
}

/// A name for each system an allowance names, for a failure to read by.
fn names() -> Vec<(&'static str, TypeId)> {
    use crate::{ability, ammo, combat, items, props, throwing};
    vec![
        ("items::resolve_items", id(items::resolve_items)),
        ("props::resolve_interactions", id(props::resolve_interactions)),
        ("props::resolve_takes", id(props::resolve_takes)),
        ("props::close_emptied_containers", id(props::close_emptied_containers)),
        ("combat::resolve_attacks", id(combat::resolve_attacks)),
        ("combat::land_shots", id(combat::land_shots)),
        ("throwing::resolve_throws", id(throwing::resolve_throws)),
        ("throwing::land_throws", id(throwing::land_throws)),
        ("ability::resolve_abilities", id(ability::resolve_abilities)),
        ("ability::land_abilities", id(ability::land_abilities)),
        ("ammo::spend_ammo", id(ammo::spend_ammo)),
    ]
}

/// The component or resource ids the allowances name, by what they are.
fn ids(world: &World) -> Vec<(&'static str, ComponentId)> {
    use crate::turn::{Acting, ActionDone, ActionRefused, Occupancy, Turns};
    let c = world.components();
    ambiguity::data([
        ("Acting", c.component_id::<Acting>()),
        ("Messages<ActionDone>", c.component_id::<Messages<ActionDone>>()),
        ("Messages<ActionRefused>", c.component_id::<Messages<ActionRefused>>()),
        ("Thinking", c.component_id::<crate::minds::Thinking>()),
        ("Turns", c.component_id::<Turns>()),
        ("CueWait", c.component_id::<crate::cue::CueWait>()),
        ("Occupancy", c.component_id::<Occupancy>()),
        ("Position", c.component_id::<crate::components::Position>()),
        ("Viewshed", c.component_id::<crate::components::Viewshed>()),
        ("Inventory", c.component_id::<crate::items::Inventory>()),
        ("Equipped", c.component_id::<crate::items::Equipped>()),
        ("Stack", c.component_id::<crate::items::Stack>()),
        ("Messages<ItemEvent>", c.component_id::<Messages<crate::items::ItemEvent>>()),
        ("Messages<Fired>", c.component_id::<Messages<crate::effects::Fired>>()),
        ("Messages<Cued>", c.component_id::<Messages<crate::cue::Cued>>()),
        ("Messages<DamageEvent>", c.component_id::<Messages<crate::combat::DamageEvent>>()),
        ("Messages<Heal>", c.component_id::<Messages<crate::combat::Heal>>()),
        ("Messages<ApplyStatus>", c.component_id::<Messages<crate::status::ApplyStatus>>()),
        ("Messages<RemoveStatus>", c.component_id::<Messages<crate::status::RemoveStatus>>()),
        ("Messages<Missed>", c.component_id::<Messages<crate::accuracy::Missed>>()),
        ("Messages<Reflected>", c.component_id::<Messages<crate::ability::Reflected>>()),
        ("EffectRng", c.component_id::<crate::effects::EffectRng>()),
        ("CombatRng", c.component_id::<crate::combat::CombatRng>()),
        ("ReflectRng", c.component_id::<crate::ability::ReflectRng>()),
    ])
}

/// Every conflicting pair of the engine's systems left unordered on
/// purpose, each with its reason. A pair not here fails the test, and so
/// does an entry no pair needs any more.
///
/// Nearly all are about one invariant: one action a pass. The turn loop
/// deals one actor's turn a pass and `Resolution::claim` lets that actor
/// resolve one action in it, so no two resolvers ever act in one pass;
/// and nothing is dealt while anything flies, so what lands only ever
/// lands in a pass no action is resolved in.
fn allowed(world: &World) -> Vec<Allowed> {
    use crate::plugin::{PerceiveSet, TurnSet};
    use crate::{ability, ammo, combat, items, props, throwing};
    let ids = ids(world);
    let on = |names: &[&str]| -> Vec<ComponentId> { ambiguity::on(&ids, names) };
    let claims = ["Acting", "Messages<ActionDone>", "Messages<ActionRefused>"];
    let claimed = |more: &[&str]| -> Vec<ComponentId> { on(&[&claims[..], more].concat()) };
    let resolved = "one action a pass: the two are never resolved in the same one";
    let landed = "what lands lands in a pass that dealt nobody a turn, so never beside an action resolved";
    let (items, interact, take) = (id(items::resolve_items), id(props::resolve_interactions), id(props::resolve_takes));
    let (attack, shot) = (id(combat::resolve_attacks), id(combat::land_shots));
    let (throw, thrown) = (id(throwing::resolve_throws), id(throwing::land_throws));
    let (ability, cast) = (id(ability::resolve_abilities), id(ability::land_abilities));
    let effects = ["Messages<Cued>", "Messages<DamageEvent>", "Messages<Heal>", "Messages<ApplyStatus>", "Messages<RemoveStatus>", "EffectRng"];
    let pack = ["Messages<ItemEvent>", "Inventory", "Equipped", "Stack"];
    vec![
        Allowed::among(
            vec![TurnSet::Resolve.intern(), TurnSet::Sweep.intern()],
            on(&claims),
            "any two resolvers or sweepers: Resolution::claim spends one actor's one turn once a pass, so the first to claim it is the only one that acts on it",
        ),
        Allowed::among(
            vec![PerceiveSet::Annotate.intern()],
            on(&["Thinking"]),
            "one contributor per field of the snapshot: each annotation fills a field of its own, and the snapshot is sorted once after them all",
        ),
        // Two resolvers.
        Allowed::pair(items, interact, claimed(&["Occupancy", "Position", "Viewshed", "Inventory"]), resolved),
        Allowed::pair(items, take, claimed(&pack[..3]), resolved),
        Allowed::pair(items, attack, claimed(&["Messages<Fired>", "Messages<Reflected>", "Equipped", "ReflectRng"]), resolved),
        Allowed::pair(items, throw, claimed(&[&pack[..], &["Messages<Fired>"]].concat()), resolved),
        Allowed::pair(items, ability, claimed(&[&pack[1..], &["Occupancy", "Position", "Viewshed", "Messages<Reflected>", "ReflectRng"]].concat()), resolved),
        Allowed::pair(interact, take, claimed(&["Position", "Inventory"]), resolved),
        Allowed::pair(interact, attack, claimed(&["Occupancy", "Messages<Cued>", "Messages<DamageEvent>", "Position", "Viewshed"]), resolved),
        Allowed::pair(interact, throw, claimed(&["Occupancy", "Messages<Cued>", "Messages<DamageEvent>", "Position", "Inventory"]), resolved),
        Allowed::pair(interact, ability, claimed(&[&effects[..], &["Occupancy", "Position", "Viewshed"]].concat()), resolved),
        Allowed::pair(take, attack, claimed(&["Equipped"]), resolved),
        Allowed::pair(take, throw, claimed(&pack), resolved),
        Allowed::pair(take, ability, claimed(&[&pack[1..], &["Position"]].concat()), resolved),
        Allowed::pair(
            attack,
            throw,
            claimed(&["CueWait", "Messages<Cued>", "Messages<DamageEvent>", "Messages<Fired>", "Messages<Missed>", "Equipped", "CombatRng"]),
            resolved,
        ),
        Allowed::pair(
            attack,
            ability,
            claimed(&["Occupancy", "CueWait", "Messages<Cued>", "Messages<DamageEvent>", "Messages<Reflected>", "Position", "Viewshed", "ReflectRng"]),
            resolved,
        ),
        Allowed::pair(
            throw,
            ability,
            claimed(&[&pack[1..], &["Occupancy", "CueWait", "Messages<Cued>", "Messages<DamageEvent>", "Position"]].concat()),
            resolved,
        ),
        // A resolver and what lands.
        Allowed::pair(items, cast, on(&["Occupancy", "Position", "Viewshed"]), landed),
        Allowed::pair(items, thrown, on(&[&pack[..], &["Messages<Fired>"]].concat()), landed),
        Allowed::pair(items, shot, on(&["Messages<Fired>"]), landed),
        Allowed::pair(interact, cast, on(&[&effects[..], &["Occupancy", "Position", "Viewshed"]].concat()), landed),
        Allowed::pair(interact, thrown, on(&["Occupancy", "Messages<Cued>", "Messages<DamageEvent>", "Position", "Inventory"]), landed),
        Allowed::pair(interact, shot, on(&["Messages<DamageEvent>", "Position"]), landed),
        Allowed::pair(take, cast, on(&["Position"]), landed),
        Allowed::pair(take, thrown, on(&pack), landed),
        Allowed::pair(
            ability,
            thrown,
            on(&[&pack[1..], &["Turns", "Occupancy", "CueWait", "Messages<Cued>", "Messages<DamageEvent>", "Position"]].concat()),
            landed,
        ),
        Allowed::pair(ability, shot, on(&["Turns", "CueWait", "Messages<DamageEvent>", "Position"]), landed),
        Allowed::pair(throw, shot, on(&["CueWait", "Messages<DamageEvent>", "Messages<Fired>", "Messages<Missed>"]), landed),
        // Two bags that are never the same bag.
        Allowed::pair(
            id(props::close_emptied_containers),
            id(ammo::spend_ammo),
            on(&["Inventory"]),
            "a container's bag and an actor's are never the same bag, and only an actor fires",
        ),
    ]
}

#[test]
fn every_conflicting_pair_of_the_engines_systems_is_ordered_or_allowed_for_a_stated_reason() {
    let mut app = engine();
    // The engine's own: whatever its plugins add to the bare headless app,
    // whose `CorePlugin` systems each of them meets.
    let ours = ambiguity::added(&mut app, &mut crate::testing::headless_app());
    let world = app.world();
    ambiguity::assert_ordered(world, "the engine", &ours, &allowed(world), &names(), &ids(world));
}
