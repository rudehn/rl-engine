//! The run's start: the foundry's decks, its combat rules, and the
//! commando dropped onto deck one.
//!
//! In the library rather than `main.rs`, since `main.rs` is only the
//! binary that opens the window; a later integration test reaches this
//! the same way `testing::headless` does, through the crate the `tests/`
//! directory links against.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::damage::SubtractArmor;

use crate::content::{Profile, resistances};
use crate::decks::{Foundry, map_of};

/// Builds the foundry's decks and combat rules, spawns the commando, and
/// warps it onto deck one. Runs once, in [`NewRun`].
///
/// Reads the registries from a resource rather than building them itself:
/// `main.rs` inserts them before the run starts, the way its content is
/// loaded before anything else runs.
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

    let kinetic = registries.damage_kinds.expect("kinetic");
    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(20), RevealsMap),
            (Health::full(30), Armor(0), Faction(commando), Resists(resistances(Profile::Organic, &registries))),
            (MeleeAttack { kind: kinetic, dice: DiceRoll::new(1, 3), cost: None }, Name::new("you"), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Inventory::default(), Equipped(Equipment::with_slot_count(registries.slots.len()))),
        ))
        .id();

    commands.insert_resource(PlaceRulesRes(Box::new(foundry)));
    warps.write(WarpRequest::into_place(player, map_of(1)));
    next.set(EngineState::Playing);
}
