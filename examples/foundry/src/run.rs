//! The run's start: the foundry's decks, its combat rules, and the
//! commando dropped onto deck one.
//!
//! In the library rather than `main.rs`, since `main.rs` is only the
//! binary that opens the window; a later integration test reaches this
//! the same way `testing::headless` does, through the crate the `tests/`
//! directory links against.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::hearing::HearingStats;
use rl_engine::rl_rules::damage::{ApplyResistance, SubtractArmor};

use crate::content::{Profile, resistances};
use crate::decks::{Foundry, map_of};
use crate::droids::Roster;

/// The deck a run starts on, when it is not deck one: `main.rs` reads it
/// from `FOUNDRY_START` for a screenshot of a deeper deck. Absent, the run
/// starts on deck one, which is every real run.
#[derive(Resource, Debug, Clone, Copy)]
pub struct StartDeck(pub u32);

/// The player, as a kind the save can name.
///
/// A marker rather than the engine's `Player`, because a save names each
/// kind by its own component and restores it through that component's own
/// account of it; the engine's marker is every game's.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Commando;

/// Asks the next `NewRun` to continue the saved run rather than begin one.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Resume;

/// Everything a run stands on that is the same however it began: the
/// decks, the combat rules and the damage pipeline, the dark, the fire,
/// the roster, the loot stream. Shared by [`start`] and [`resume`], so a
/// continued run is built on exactly the foundry a fresh one is.
pub fn prepare(commands: &mut Commands, seed: &Seed, registries: &Registries) {
    let foundry = Foundry::new(seed.0);
    commands.insert_resource(crate::light::LampTile(foundry.lamp()));
    commands.insert_resource(foundry.appearance());
    commands.insert_resource(WorldMap::new(foundry.tiles().tables()));

    let (commando, droids, vermin) = (registries.factions.expect("commando"), registries.factions.expect("droids"), registries.factions.expect("vermin"));
    let combat = CombatRules::new(&registries.factions).hostile(commando, droids).hostile(commando, vermin).hostile(droids, vermin);
    commands.insert_resource(combat);
    // Resistance first, then plate: the damage table in `content.rs` is
    // what makes ion undo a chassis and barely touch flesh, and the
    // engine's default list is armor alone, which would leave it written
    // down and never played.
    commands.insert_resource(DamageStages(vec![Box::new(ApplyResistance), Box::new(SubtractArmor)]));
    commands.insert_resource(Lighting::dark());
    // Standing in fire scorches, and whatever burns smokes: an incendiary
    // is a fire and a screen at once.
    commands.insert_resource(FireRules::new().inflicts(registries.statuses.expect("scorched"), 3).smoke(registries.gases.expect("smoke"), 30));
    commands.insert_resource(Roster::load(registries));
    // Seeded once, here, and never again: `Drops` is a resource a kill's
    // roll keeps advancing, not a stream `Seed::stream` is asked for
    // fresh on every event the way `scatter_on_arrival`'s own is. A
    // continued run derives it again from the saved seed, as the engine
    // does its own streams.
    commands.insert_resource(crate::loot::Drops(seed.stream(b"foundry.drops", 0)));
    commands.insert_resource(PlaceRulesRes(Box::new(foundry)));
}

/// Spawns the commando with its lamp lit and nothing else, nowhere yet:
/// the warp onto a deck puts it somewhere, and a continued run's save
/// does.
pub fn spawn_commando(commands: &mut Commands, registries: &Registries) -> Entity {
    let commando = registries.factions.expect("commando");
    let kinetic = registries.damage_kinds.expect("kinetic");
    let player = commands
        .spawn((
            (Actor, Player, Commando, Blocks, Position(Point::ZERO), Viewshed::new(20), RevealsMap),
            (Health::full(30), Armor(0), Faction(commando), Resists(resistances(Profile::Organic, registries)), crate::light::SHOULDER_LAMP),
            (MeleeAttack::new(kinetic, DiceRoll::new(1, 3)), Name::new("you"), Glyph::new('@', Color::WHITE).on_layer(10)),
            // Noticeable, not sneaky: no skill at hiding, but a subject a
            // droid has to notice rather than one it sees the instant it
            // comes into view. Without it nothing ever notices the
            // commando, so a probe that spots it never sounds its alarm,
            // and the lamp decides only whether a droid can see, never how
            // sure it is.
            Stealth(StealthStats::default()),
            // Ears, so the deck's noise reads on the vitals strip: a
            // commando hears what a droid hears, from a threshold of
            // nothing, and decides for themselves what it means.
            Hearing(HearingStats { threshold: 0, memory: 6 }),
        ))
        .id();
    // Empty hands and an empty bag: what the commando carries is what the
    // deck gave them. The innate `MeleeAttack` above is the whole opening
    // arsenal, which is why nothing on deck one shoots back and a pistol
    // lies somewhere on it most runs.
    commands.entity(player).insert((Inventory::default(), Equipped(Equipment::with_slot_count(registries.slots.len()))));
    player
}

/// Builds the foundry, spawns the commando and warps it onto deck one, or
/// onto [`StartDeck`]'s. Runs once, in [`NewRun`].
///
/// Reads the registries from a resource rather than building them itself:
/// `main.rs` inserts them before the run starts, the way its content is
/// loaded before anything else runs.
pub fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    registries: Res<Registries>,
    first: Option<Res<StartDeck>>,
    title: Option<Res<crate::title::Title>>,
    resume: Option<Res<Resume>>,
    mut begin: Begin,
) {
    // Nothing until the player asks for it. The engine runs `NewRun` at
    // startup, and with the title screen up this is where that first run
    // stops: no `WorldMap` is inserted, so the engine stays idle and the
    // screen has the terminal to itself. Picking a run lowers the flag and
    // runs `NewRun` again. A continued run is [`resume`]'s.
    if title.is_some_and(|t| t.up) || resume.is_some() {
        return;
    }
    prepare(&mut commands, &seed, &registries);
    let player = spawn_commando(&mut commands, &registries);
    let deck = first.map_or(1, |f| f.0.clamp(1, crate::decks::DECKS));
    begin.log.notice(format!("Seed {}. The drop ship is gone. The reactor is three decks down.", seed.0.0), 0);
    begin.warps.write(WarpRequest::into_place(player, map_of(deck)));
    begin.next.set(EngineState::Playing);
}

/// Continues the saved run: the foundry built again from the saved seed,
/// the run restored into it, and what the run did to the commando put
/// back once the engine has given it back its gear.
///
/// After [`start`] and `mission::start` in `NewRun`, so the resources a
/// restore writes into exist. A save that cannot be read is logged and a
/// fresh run begins instead: the title offers Continue only for a save it
/// could read, so this is a save that went bad in between.
pub fn resume(world: &mut World) {
    if world.remove_resource::<Resume>().is_none() {
        return;
    }
    let saved = match rl_engine::rl_save::load_run(world) {
        Ok(Some(saved)) => saved,
        Ok(None) | Err(_) => {
            error!("there is no save that can be continued; beginning a new run");
            if let Err(e) = world.run_system_once(start) {
                error!("the new run could not begin: {e}");
            }
            return;
        }
    };
    let seed = Seed(saved.engine.seed);
    world.insert_resource(seed);
    let registries = world.resource::<Registries>().clone();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    prepare(&mut Commands::new(&mut queue, world), &seed, &registries);
    queue.apply(world);
    // A save that restores only in part is worse than none: the run it
    // leaves is neither the one saved nor a fresh one. So a failure tears
    // down what it got to, and a new run begins, as for a save that could
    // not be read at all.
    let restored = saved.restore(world);
    let player = world.query_filtered::<Entity, With<Player>>().single(world).ok();
    let (Ok(()), Some(player)) = (restored, player) else {
        error!("the save could not be restored; beginning a new run");
        rl_engine::rl_bevy::plugin::clear_run(world);
        if let Err(e) = world.run_system_once(start) {
            error!("the new run could not begin: {e}");
        }
        return;
    };
    // What the run did to the commando is its upgrades, given again to a
    // commando that has none, now that its gear is back on: the uplink's
    // tile of reach is given to the gun it wears the way it was the first
    // time, rather than saved on the gun and given twice.
    let taken = world.resource::<crate::upgrades::Taken>().clone();
    for upgrade in taken.0.iter().copied() {
        crate::upgrades::apply(upgrade, player, world);
    }
    // The wall lamps, which are the decks' own and not the run's, hung
    // again on every deck built, as each first arrival hung them.
    let lamp = world.resource::<crate::light::LampTile>().0;
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let map = world.resource::<WorldMap>();
        let mut commands = Commands::new(&mut queue, world);
        for deck in 1..=crate::decks::DECKS {
            crate::light::hang_lamps(&mut commands, map, map_of(deck), lamp);
        }
    }
    queue.apply(world);
    // A charge set whose pick was still waiting when the run was saved:
    // a pick is not a turn, so the save may have caught the charge and not
    // the choice, and the choice is offered again rather than lost.
    let quests = world.resource::<Quests>();
    let charged = crate::mission::CHARGE_QUESTS.iter().filter(|name| quests.tracker.state(quests.defs.expect(name)) == QuestState::Done).count();
    if charged > taken.0.len() {
        let offered = world.run_system_once(
            |mut modals: ResMut<Modals>,
             mut screen: ResMut<crate::upgrades::ChoiceScreen>,
             mut choosing: ResMut<crate::upgrades::Choosing>,
             taken: Res<crate::upgrades::Taken>| {
                crate::upgrades::offer(&mut modals, &mut screen, &mut choosing, &taken);
            },
        );
        if let Err(e) = offered {
            error!("the waiting pick could not be offered again: {e}");
        }
    }
    let deck = crate::decks::deck_of(world.get::<OnMap>(player).map_or(MapId::SURFACE, |m| m.0));
    world.resource_mut::<MessageLog>().notice(format!("Continuing on deck {deck}."), saved.turn());
    world.resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
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
    fn a_new_run_leaves_the_commando_empty_handed_but_able_to_carry_and_to_punch_over_a_span_of_seeds() {
        for s in 0..4u64 {
            let mut app = crate::testing::headless(RunSeed(s));
            crate::testing::settle(&mut app);
            let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
            let worn: Vec<Entity> = app.world().get::<Equipped>(me).unwrap().0.worn().map(|(_, item)| item).collect();
            assert!(worn.is_empty(), "seed {s}: nothing in hand and nothing worn");
            assert!(app.world().get::<Inventory>(me).unwrap().items.is_empty(), "seed {s}: and an empty bag");
            // The bag and the slots are there all the same, or the first
            // thing the commando steps on could not be picked up or worn.
            assert!(app.world().get::<MeleeAttack>(me).is_some(), "seed {s}: bare hands are still an attack");
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
