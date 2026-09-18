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
use crate::droids::Roster;
use crate::gear::{Armory, shape_of, spawn_item};

/// The deck a run starts on, when it is not deck one: `main.rs` reads it
/// from `FOUNDRY_START` for a screenshot of a deeper deck. Absent, the run
/// starts on deck one, which is every real run.
#[derive(Resource, Debug, Clone, Copy)]
pub struct StartDeck(pub u32);

/// Builds the foundry's decks and combat rules, loads the roster every
/// deck spawns from, spawns the commando with its lamp lit and a hand
/// blaster in hand, and warps it
/// onto deck one, or onto [`StartDeck`]'s. Runs once, in [`NewRun`].
///
/// Reads the registries from a resource rather than building them itself:
/// `main.rs` inserts them before the run starts, the way its content is
/// loaded before anything else runs.
pub fn start(mut commands: Commands, seed: Res<Seed>, registries: Res<Registries>, first: Option<Res<StartDeck>>, mut begin: Begin) {
    let foundry = Foundry::new(seed.0);
    commands.insert_resource(crate::light::LampTile(foundry.lamp()));
    commands.insert_resource(foundry.appearance());
    commands.insert_resource(WorldMap::new(foundry.tiles().tables()));

    let (commando, droids, vermin) = (registries.factions.expect("commando"), registries.factions.expect("droids"), registries.factions.expect("vermin"));
    let combat = CombatRules::new(&registries.factions).hostile(commando, droids).hostile(commando, vermin).hostile(droids, vermin);
    commands.insert_resource(combat);
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    commands.insert_resource(Lighting::dark());
    commands.insert_resource(Roster::load(&registries));
    commands.insert_resource(crate::droids::Sounded::default());
    // Seeded once, here, and never again: `Drops` is a resource a kill's
    // roll keeps advancing, not a stream `Seed::stream` is asked for
    // fresh on every event the way `scatter_on_arrival`'s own is.
    commands.insert_resource(crate::loot::Drops(seed.stream(b"foundry.drops", 0)));

    let kinetic = registries.damage_kinds.expect("kinetic");
    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(20), RevealsMap),
            (Health::full(30), Armor(0), Faction(commando), Resists(resistances(Profile::Organic, &registries)), crate::light::SHOULDER_LAMP),
            (MeleeAttack::new(kinetic, DiceRoll::new(1, 3)), Name::new("you"), Glyph::new('@', Color::WHITE).on_layer(10)),
            // Noticeable, not sneaky: no skill at hiding, but a subject a
            // droid has to notice rather than one it sees the instant it
            // comes into view. Without it nothing ever notices the
            // commando, so a probe that spots it never sounds its alarm,
            // and the lamp decides only whether a droid can see, never how
            // sure it is.
            Stealth(StealthStats::default()),
        ))
        .id();
    // A hand blaster in hand from the first turn: the slice's own weapon,
    // spawned as any found one is so it carries its heat, since a deck's
    // droids shoot from five tiles and nothing promises a gun on deck one.
    let armory = Armory::load(&registries);
    let id = armory.defs.expect("hand blaster");
    let blaster = spawn_item(&mut commands, &armory, id, &registries);
    let mut worn = Equipment::with_slot_count(registries.slots.len());
    let shape = shape_of(armory.defs.get(id), &registries).expect("a hand blaster is worn");
    worn.equip(blaster, &shape).expect("an empty commando has a free hand");
    commands.entity(player).insert((Inventory { items: vec![blaster] }, Equipped(worn)));

    commands.insert_resource(PlaceRulesRes(Box::new(foundry)));
    let deck = first.map_or(1, |f| f.0.clamp(1, crate::decks::DECKS));
    begin.log.notice(format!("Seed {}. The drop ship is gone. The reactor is three decks down.", seed.0.0), 0);
    begin.warps.write(WarpRequest::into_place(player, map_of(deck)));
    begin.next.set(EngineState::Playing);
}

/// What starting a run writes beyond the world itself: the log's first
/// line, the warp onto the first deck, and the state that begins play.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Begin<'w> {
    warps: MessageWriter<'w, WarpRequest>,
    next: ResMut<'w, NextState<EngineState>>,
    log: ResMut<'w, MessageLog>,
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
    fn a_new_run_puts_a_hand_blaster_that_runs_hot_in_the_commandos_hand_over_a_span_of_seeds() {
        for s in 0..4u64 {
            let mut app = crate::testing::headless(RunSeed(s));
            crate::testing::settle(&mut app);
            let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
            let worn: Vec<Entity> = app.world().get::<Equipped>(me).unwrap().0.worn().map(|(_, item)| item).collect();
            assert_eq!(worn.len(), 1, "seed {s}: one thing in hand");
            assert_eq!(app.world().get::<Name>(worn[0]).map(Name::as_str), Some("hand blaster"), "seed {s}");
            assert!(app.world().get::<crate::heat::Heat>(worn[0]).is_some(), "seed {s}: and it carries its heat");
            assert!(app.world().get::<Inventory>(me).unwrap().items.contains(&worn[0]), "seed {s}: worn from the bag, as a found one is");
        }
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
