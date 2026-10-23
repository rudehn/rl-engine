//! Bevy's ambiguity detection over Undermere's schedules: every pair of
//! Undermere's systems that touch the same data unordered, with at least
//! one writing it, is either ordered or allowed here with its reason.
//!
//! The check is the engine's,
//! [`atglyph::ecs::testing::ambiguity`](atglyph::ecs::testing::ambiguity),
//! and Undermere's systems are whatever
//! [`UndermerePlugin`](crate::plugin::UndermerePlugin) adds beyond the
//! engine's plugins it plays with, so one added later is covered without
//! being listed. What is here is the few pairs left unordered on purpose,
//! each with why; everything else is ordered where it is registered,
//! mostly by [`ReactStep`](crate::plugin::ReactStep). A failure names a
//! system this list does not by Bevy's own name, which is readable in a
//! build with `--features bevy/debug`.

use std::any::TypeId;

use atglyph::ecs::minds::Thinking;
use atglyph::ecs::plugin::TurnSet;
use atglyph::ecs::testing::ambiguity::{self, Allowed, system_type as id};
use atglyph::ecs::turn::{Acting, ActionDone, ActionRefused};
use atglyph::primitives::RunSeed;
use bevy::ecs::component::ComponentId;
use bevy::prelude::*;

/// A name for each system the allowances below name, for a failure to
/// read by.
fn names() -> Vec<(&'static str, TypeId)> {
    vec![("tricks::sense_patrols", id(crate::tricks::sense_patrols)), ("townsfolk::sense_beats", id(crate::townsfolk::sense_beats))]
}

/// The component or resource ids the allowances name, by what they are.
fn ids(world: &World) -> Vec<(&'static str, ComponentId)> {
    let c = world.components();
    ambiguity::data([
        ("Thinking", c.component_id::<Thinking>()),
        ("Acting", c.component_id::<Acting>()),
        ("Messages<ActionDone>", c.component_id::<Messages<ActionDone>>()),
        ("Messages<ActionRefused>", c.component_id::<Messages<ActionRefused>>()),
    ])
}

/// Every conflicting pair Undermere leaves unordered on purpose, each
/// with its reason. A pair not here fails the test, and so does an entry
/// no pair needs any more.
fn allowed(world: &World) -> Vec<Allowed> {
    let ids = ids(world);
    let on = |names: &[&str]| -> Vec<ComponentId> { ambiguity::on(&ids, names) };
    let senses = "one contributor per field of the snapshot: a game's sense pushes a sense of its own type and nothing else, and the snapshot is sorted once after every contributor";
    vec![
        Allowed::among(
            vec![TurnSet::Resolve.intern(), TurnSet::Sweep.intern()],
            on(&["Acting", "Messages<ActionDone>", "Messages<ActionRefused>"]),
            "any two resolvers or sweepers: Resolution::claim spends one actor's one turn once a pass, so the first to claim it is the only one that acts on it",
        ),
        Allowed::with_any(id(crate::tricks::sense_patrols), on(&["Thinking"]), senses),
        Allowed::with_any(id(crate::townsfolk::sense_beats), on(&["Thinking"]), senses),
    ]
}

#[test]
fn every_conflicting_pair_of_undermeres_systems_is_ordered_or_allowed_for_a_stated_reason() {
    let mut app = crate::testing::headless(RunSeed(0));
    // Undermere's own: whatever UndermerePlugin added beyond the engine's
    // plugins it plays with, by schedule and type.
    let ours = ambiguity::added(&mut app, &mut crate::testing::headless_without_undermere(RunSeed(0)));
    let world = app.world();
    ambiguity::assert_ordered(world, "Undermere", &ours, &allowed(world), &names(), &ids(world));
}
