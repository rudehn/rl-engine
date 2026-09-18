//! The run's start: the foundry's decks, its combat rules, and the
//! commando dropped onto deck one.
//!
//! In the library rather than `main.rs`, since `main.rs` is only the
//! binary that opens the window; a later integration test reaches this
//! the same way `testing::headless` does, through the crate the `tests/`
//! directory links against.
//!
//! The commando spawns without [`Actor`] and [`admit_the_player`] gives it
//! one back on the run's first [`PlaceEntered`]; see that function's own
//! doc for why.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::damage::SubtractArmor;

use crate::content::{Profile, resistances};
use crate::decks::{Foundry, map_of};
use crate::droids::Roster;

/// Builds the foundry's decks and combat rules, loads the roster every
/// deck spawns from, spawns the commando, and warps it onto deck one.
/// Runs once, in [`NewRun`].
///
/// Reads the registries from a resource rather than building them itself:
/// `main.rs` inserts them before the run starts, the way its content is
/// loaded before anything else runs.
///
/// Spawned without [`Actor`], deliberately: see [`admit_the_player`].
pub fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    registries: Res<Registries>,
    mut warps: MessageWriter<WarpRequest>,
    mut next: ResMut<NextState<EngineState>>,
) {
    let foundry = Foundry::new(seed.0);
    commands.insert_resource(foundry.appearance());
    commands.insert_resource(WorldMap::new(foundry.tiles().tables()));

    let (commando, droids, vermin) = (registries.factions.expect("commando"), registries.factions.expect("droids"), registries.factions.expect("vermin"));
    let combat = CombatRules::new(&registries.factions).hostile(commando, droids).hostile(commando, vermin).hostile(droids, vermin);
    commands.insert_resource(combat);
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    commands.insert_resource(Lighting::dark());
    commands.insert_resource(Roster::load(&registries));
    // Seeded once, here, and never again: `Drops` is a resource a kill's
    // roll keeps advancing, not a stream `Seed::stream` is asked for
    // fresh on every event the way `scatter_on_arrival`'s own is.
    commands.insert_resource(crate::loot::Drops(seed.stream(b"foundry.drops", 0)));

    let kinetic = registries.damage_kinds.expect("kinetic");
    let player = commands
        .spawn((
            (Player, Blocks, Position(Point::ZERO), Viewshed::new(20), RevealsMap),
            (Health::full(30), Armor(0), Faction(commando), Resists(resistances(Profile::Organic, &registries))),
            (MeleeAttack { kind: kinetic, dice: DiceRoll::new(1, 3), cost: None }, Name::new("you"), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Inventory::default(), Equipped(Equipment::with_slot_count(registries.slots.len()))),
        ))
        .id();

    commands.insert_resource(PlaceRulesRes(Box::new(foundry)));
    warps.write(WarpRequest::into_place(player, map_of(1)));
    next.set(EngineState::Playing);
}

/// Gives the player [`Actor`] the moment its first [`PlaceEntered`]
/// arrives: the warp [`start`] queues onto deck one, resolved in
/// `TurnSet::Resolve` of the very same pass this reacts to in
/// `TurnSet::React`.
///
/// `start` spawns the player at `Position(Point::ZERO)` with no [`OnMap`],
/// which the engine's own `tag_new_positions` (`crates/rl-bevy/src/places.rs`)
/// tags onto the surface, since [`WorldMap`] has no notion of "not yet
/// warped anywhere". Foundry has no surface at all, so nothing there is
/// ever loaded. Spawned with `Actor` outright, the player would be
/// admitted into the turn queue on this same first pass, before its own
/// warp has had a chance to resolve; `crates/rl-bevy/src/turn.rs`'s own
/// `schedule` finds it standing nowhere loaded and freezes it forward by
/// a whole `BASE_ACTION_COST`, twice over, once for the check on
/// admission and once more before `TurnSet::Resolve` ever runs within the
/// same call to `schedule`, since freezing does not itself advance the
/// clock and neither freeze can see the warp that only resolves
/// afterward. `droids::spawns::populate_deck`'s own monsters, spawned
/// fresh on the very same pass once the warp has landed, are admitted a
/// pass later at whatever the clock reads by then, which the player's own
/// two-turn head start has already left behind: every one of deck one's
/// own groups gets a free turn or two before the player ever holds one at
/// all.
///
/// Keeping the player off the turn queue until this fires sidesteps the
/// whole freeze: `admit_new_actors` never sees it until the exact pass its
/// `Position` and `OnMap` are already correct, the same pass a fresh
/// deck's own monsters are spawned and first become visible to it one
/// pass later still, from the very same clock reading. `admit_new_actors`'s
/// own doc is "the player first" for actors admitted together at one
/// reading; the freeze this replaces was never a tie in the first place,
/// since being pushed two turns ahead is not the same reading at all.
///
/// A query filtered on `Without<Actor>` rather than a resource flag: once
/// the player has one, every later `PlaceEntered` (an ordinary deck
/// transition) finds nothing to do, with no state of its own to forget
/// between runs.
pub fn admit_the_player(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, player: Query<Entity, (With<Player>, Without<Actor>)>) {
    if entered.read().count() == 0 {
        return;
    }
    if let Ok(player) = player.single() {
        commands.entity(player).insert(Actor);
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::plugin::{Turn, TurnSet};
    use rl_engine::rl_core::RunSeed;

    use super::*;

    /// Every actor ever dealt a turn, in the order it was dealt, whether
    /// player or not: a resource a test-only system fills from inside the
    /// `Turn` schedule itself, in `TurnSet::Decide`, right after a pass's
    /// own `TurnSet::Schedule` deals it and well before that same pass's
    /// `TurnSet::Cleanup` could take `MyTurn` away again. A reader placed
    /// in the outer `Update` schedule instead would miss every actor dealt
    /// and resolved within the same `app.update()` the runner in
    /// `crates/rl-bevy/src/plugin.rs` can spend several `Turn` passes on.
    #[derive(Resource, Default)]
    struct Dealt(Vec<bool>);

    fn record_deals(mut dealt: ResMut<Dealt>, fresh: Query<Has<Player>, Added<MyTurn>>) {
        dealt.0.extend(fresh.iter());
    }

    #[test]
    fn on_a_fresh_run_the_player_is_dealt_the_first_turn_over_a_span_of_seeds() {
        for s in 0..8u64 {
            let mut app = crate::testing::headless(RunSeed(s));
            app.init_resource::<Dealt>();
            app.add_systems(Turn, record_deals.in_set(TurnSet::Decide));
            app.update();
            app.update();
            let dealt = &app.world().resource::<Dealt>().0;
            assert!(!dealt.is_empty(), "seed {s}: nobody was ever dealt a turn");
            assert_eq!(dealt.first(), Some(&true), "seed {s}: {dealt:?} dealt someone else the first turn before the player");
        }
    }
}
