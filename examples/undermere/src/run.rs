//! The run: the Undermere built for it, the adventurer who walks out of
//! Mereholt with what they can carry, and the same run carried on from a
//! save.
//!
//! In the library rather than the binary, as Foundry's is, so a test
//! begins and continues a run the way the window does, through
//! [`UndermerePlugin`](crate::plugin::UndermerePlugin).

use bevy::ecs::system::RunSystemOnce;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::hearing::HearingStats;
use rl_engine::rl_rules::damage::{ApplyResistance, SubtractArmor};

use crate::armory::Armory;
use crate::dungeon::{FLOORS, TOWN, Undermere, floor_of, map_of};
use crate::light::Fixtures;

/// The adventurer's health at the start of a run, and the most they ever
/// have: Brogue's forty, a first cut, since no experience and no levels
/// raise it and what keeps them alive is what they find.
pub const HEALTH: i32 = 40;

/// The lantern, by its name in `items.ron`: what the adventurer walks out
/// with lit, and what dims as they descend.
pub const LANTERN: &str = "lantern";

/// What the adventurer walks out of Mereholt with, by name and count: a
/// sword in hand, leather on their back, a lit lantern, and two potions of
/// healing in the pack. Everything that is worn is worn.
pub const KIT: [(&str, u32); 4] = [("sword", 1), ("leather armor", 1), (LANTERN, 1), ("potion of healing", 2)];

/// How the adventurer is drawn.
pub const ADVENTURER: Color = Color::WHITE;

/// The floor a run starts on, when it is not Mereholt: `main.rs` reads it
/// from `UNDERMERE_START` for a photograph of a deeper floor. Absent, or
/// nought, the run starts in Mereholt, which is every real run.
#[derive(Resource, Debug, Clone, Copy)]
pub struct StartFloor(pub u32);

/// The adventurer, as a kind the save can name.
///
/// A marker rather than the engine's `Player`, because a save names each
/// kind by its own component and restores it through that component's own
/// account of it; the engine's marker is every game's.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Adventurer;

/// The deepest floor the run has reached, nought while it has reached
/// none: what the score's depth line counts, and saved with the run.
///
/// Only ever raised, by [`remember_depth`], and put back to nought when the
/// run ends, so a run begun after another in the same launch has been
/// nowhere.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Deepest(pub u32);

/// Raises [`Deepest`] on every arrival on a floor deeper than any before.
pub fn remember_depth(mut entered: MessageReader<PlaceEntered>, mut deepest: ResMut<Deepest>) {
    for floor in entered.read().filter_map(|ev| floor_of(ev.map)) {
        if floor > deepest.0 {
            deepest.0 = floor;
        }
    }
}

/// Everything a run stands on that is the same however it began: the
/// places and how they look and are lit, who fights whom and how a blow is
/// taken, the dark and the fire. Shared by [`start`] and [`resume`], so a
/// continued run is built on exactly the Undermere a fresh one is.
pub fn prepare(commands: &mut Commands, seed: &Seed, registries: &Registries) {
    let undermere = Undermere::new(seed.0);
    commands.insert_resource(undermere.appearance());
    commands.insert_resource(WorldMap::new(undermere.tiles().tables()));
    commands.insert_resource(crate::content::combat_rules(registries));
    // Resistance first, then armor: the traits' resistances are what make
    // fire nothing to a salamander and poison nothing to the dead, and the
    // engine's default list is armor alone, which would leave them written
    // down and never played.
    commands.insert_resource(DamageStages(vec![Box::new(ApplyResistance), Box::new(SubtractArmor)]));
    commands.insert_resource(Lighting::dark());
    // Standing in fire scorches, a point a turn for three turns.
    commands.insert_resource(FireRules::new().inflicts(registries.statuses.expect("scorched"), 3));
    commands.insert_resource(Fixtures::new(&undermere, registries));
    // The pieces the places are stamped from, shared with the engine that
    // fills their slots, so a stamp's key names the same piece in both.
    commands.insert_resource(undermere.prefabs().clone());
    commands.insert_resource(PlaceRulesRes(Box::new(undermere)));
}

/// Spawns the adventurer with empty hands and an empty pack, nowhere yet:
/// the warp into Mereholt puts them somewhere, [`equip_kit`] gives a fresh
/// run's adventurer what they set out with, and a continued run's save
/// gives back what they carried.
pub fn spawn_adventurer(commands: &mut Commands, registries: &Registries) -> Entity {
    let side = registries.factions.expect("adventurer");
    let crush = registries.damage_kinds.expect("crush");
    commands
        .spawn((
            (Actor, Player, Adventurer, Blocks, Position(Point::ZERO), Viewshed::new(20), RevealsMap),
            // Bare hands: what is left when the sword is gone.
            (Health::full(HEALTH), Armor(0), Faction(side), MeleeAttack::new(crush, DiceRoll::new(1, 2))),
            (Name::new("you"), Glyph::new('@', ADVENTURER).on_layer(10)),
            // Noticeable, not sneaky: a subject a monster has to notice
            // rather than one it sees the moment it comes into view, so
            // whether the lantern is lit decides how soon it does.
            Stealth(StealthStats::default()),
            // Ears, so the floor's noise reads on the vitals: the
            // adventurer hears what a keen-eared monster hears.
            Hearing(HearingStats { threshold: 0, memory: 6 }),
            (Inventory::default(), Equipped(Equipment::with_slot_count(registries.slots.len()))),
        ))
        .id()
}

/// Gives `adventurer` the [`KIT`], made by the `armory`: every piece in the
/// pack, and each that is worn in its slot.
///
/// Put on straight into the slots rather than asked for with `Equip`,
/// which would spend the adventurer's first turns dressing: a run begins
/// with them dressed.
pub fn equip_kit(commands: &mut Commands, adventurer: Entity, armory: &Armory, registries: &Registries) {
    let mut pack = Vec::new();
    let mut worn = Equipment::with_slot_count(registries.slots.len());
    for (name, count) in KIT {
        let id = armory.defs.expect(name);
        for item in crate::armory::spawn_items(commands, armory, id, count, registries) {
            if let Some(shape) = crate::armory::shape_of(armory.defs.get(id), registries) {
                // Each piece of the kit has a slot of its own, so none
                // displaces another.
                worn.equip(item, &shape).expect("the kit fits the adventurer's slots");
            }
            pack.push(item);
        }
    }
    commands.entity(adventurer).insert((Inventory { items: pack }, Equipped(worn)));
}

/// Builds the Undermere, spawns the adventurer with their kit and warps
/// them into Mereholt, or onto [`StartFloor`]'s floor. Runs in [`NewRun`],
/// for every run but one the title asked to continue with `ContinueRun`,
/// which is [`resume`]'s.
///
/// Asks nothing about a title: while one is up the engine runs no
/// `NewRun` at all, and runs it once the player picks.
pub fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    content: Content,
    first: Option<Res<StartFloor>>,
    continuing: Option<Res<ContinueRun>>,
    mut begin: Begin,
) {
    if continuing.is_some() {
        return;
    }
    prepare(&mut commands, &seed, &content.registries);
    let adventurer = spawn_adventurer(&mut commands, &content.registries);
    equip_kit(&mut commands, adventurer, &content.armory, &content.registries);
    begin.log.notice(format!("Seed {}. Ten floors under the lake, the Drowned King waits.", seed.0.0), 0);
    let place = match first.map_or(0, |f| f.0.min(FLOORS)) {
        0 => TOWN,
        floor => map_of(floor),
    };
    begin.warps.write(WarpRequest::into_place(adventurer, place));
    begin.next.set(EngineState::Playing);
}

/// Continues the saved run, when the title's Continue asked for it with
/// `ContinueRun`: the Undermere built again from the saved seed, the run
/// restored into it, and every place it had built lit again. Takes the
/// marker away, so the run after this one, a restart's, begins afresh.
///
/// After [`start`] in `NewRun`, so the resources a restore writes into
/// exist. A save that cannot be read is logged and a fresh run begins
/// instead: the title offers Continue only for a save it could read, so
/// this is a save that went bad in between.
pub fn resume(world: &mut World) {
    if world.remove_resource::<ContinueRun>().is_none() {
        return;
    }
    let saved = match rl_engine::rl_save::load_run(world) {
        Ok(Some(saved)) => saved,
        Ok(None) | Err(_) => {
            error!("there is no save that can be continued; beginning a new run");
            begin_afresh(world);
            return;
        }
    };
    let seed = Seed(saved.engine.seed);
    world.insert_resource(seed);
    let registries = world.resource::<Registries>().clone();
    let mut queue = CommandQueue::default();
    prepare(&mut Commands::new(&mut queue, world), &seed, &registries);
    queue.apply(world);
    // A save that restores only in part is worse than none: the run it
    // leaves is neither the one saved nor a fresh one. So a failure tears
    // down what it got to, and a new run begins, as for a save that could
    // not be read at all.
    let restored = saved.restore(world);
    let adventurer = world.query_filtered::<Entity, With<Adventurer>>().single(world).ok();
    let (Ok(()), Some(adventurer)) = (restored, adventurer) else {
        error!("the save could not be restored; beginning a new run");
        rl_engine::rl_bevy::plugin::clear_run(world);
        begin_afresh(world);
        return;
    };
    // The fixtures' light is the places' own and not the run's, lit again
    // on every place built, as each first arrival lit it.
    let mut queue = CommandQueue::default();
    {
        let (map, fixtures) = (world.resource::<WorldMap>(), world.resource::<Fixtures>());
        let mut commands = Commands::new(&mut queue, world);
        for place in std::iter::once(TOWN).chain((1..=FLOORS).map(map_of)) {
            crate::light::light_place(&mut commands, map, place, fixtures);
        }
    }
    queue.apply(world);
    let line = match world.get::<OnMap>(adventurer).and_then(|on| floor_of(on.0)) {
        Some(floor) => format!("Continuing on floor {floor}."),
        None => "Continuing in Mereholt.".to_string(),
    };
    world.resource_mut::<MessageLog>().notice(line, saved.turn());
    world.resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
}

/// Begins a fresh run where a continued one could not be.
fn begin_afresh(world: &mut World) {
    if let Err(e) = world.run_system_once(start) {
        error!("the new run could not begin: {e}");
    }
}

/// The content a run is made from, loaded once before any run begins.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Content<'w> {
    registries: Res<'w, Registries>,
    armory: Res<'w, Armory>,
}

/// What starting a run writes beyond the world itself: the log's first
/// line, the warp into Mereholt, and the state that begins play.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Begin<'w> {
    warps: MessageWriter<'w, WarpRequest>,
    next: ResMut<'w, NextState<EngineState>>,
    log: ResMut<'w, MessageLog>,
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::testing::{adventurer, arrive, at, headless, here, pack, settle, worn};

    /// A new run, over a span of seeds: the one adventurer in Mereholt,
    /// where its piece marks the arrival, at full health, the sword in
    /// hand, the leather on and the lantern lit in its place, and two
    /// potions of healing in the pack.
    #[test]
    fn a_new_run_puts_the_adventurer_in_mereholt_with_their_kit() {
        for s in 0..4 {
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            let entry = app.world().resource::<WorldMap>().place(TOWN).expect("Mereholt was built").entry;
            assert_eq!((here(&app), at(&app, me)), (TOWN, entry), "seed {s}: in Mereholt, where it marks the arrival");
            assert_eq!(app.world_mut().query_filtered::<(), With<Player>>().iter(app.world()).count(), 1, "seed {s}: one adventurer");
            assert_eq!(app.world().get::<Health>(me).map(|h| (h.current, h.max)), Some((HEALTH, HEALTH)), "seed {s}");
            let named = |names: [Option<&str>; 5]| names.map(|n| n.map(String::from)).to_vec();
            assert_eq!(worn(&app, me), named([Some("sword"), None, None, Some("leather armor"), Some(LANTERN)]), "seed {s}: hands, head, body and light");
            let mut carried = pack(&app, me);
            carried.sort();
            let kit: Vec<(String, u32)> =
                [(LANTERN, 1), ("leather armor", 1), ("potion of healing", 2), ("sword", 1)].map(|(n, c)| (n.to_string(), c)).to_vec();
            assert_eq!(carried, kit, "seed {s}: everything worn is carried, and the potions are one stack of two");
            let bag = &app.world().get::<Inventory>(me).expect("a pack").items;
            let lantern = bag.iter().copied().find(|i| app.world().get::<Name>(*i).is_some_and(|n| n.as_str() == LANTERN));
            assert!(lantern.is_some_and(|l| app.world().get::<LightSource>(l).is_some()), "seed {s}: and the lantern is lit");
        }
    }

    /// A blow meets resistance before armor: fire does nothing to a
    /// salamander, whatever its plate, and cold, which goes through plate,
    /// bites it half as hard again.
    #[test]
    fn a_resistance_is_taken_before_armor_so_fire_does_nothing_to_a_salamander() {
        use crate::testing::{clear_monsters, monster, settle};
        let mut app = headless(RunSeed(4));
        let me = arrive(&mut app, map_of(1));
        clear_monsters(&mut app);
        let beside = at(&app, me).offset(0, 1);
        let salamander = monster(&mut app, "salamander", beside);
        let registries = app.world().resource::<Registries>().clone();
        let health = |app: &App| app.world().get::<Health>(salamander).map(|h| h.current);
        let full = health(&app);
        app.world_mut().write_message(DamageEvent::new(salamander, rl_engine::rl_rules::Hit::by(me, registries.damage_kinds.expect("fire"), 10)));
        settle(&mut app);
        assert_eq!(health(&app), full, "fire-blooded: fire does nothing");
        app.world_mut().write_message(DamageEvent::new(salamander, rl_engine::rl_rules::Hit::by(me, registries.damage_kinds.expect("cold"), 10)));
        settle(&mut app);
        assert_eq!(health(&app), full.map(|h| h - 15), "and cold bites half as hard again");
    }

    /// A run started on a floor begins on that floor, at its way in, for
    /// every floor there is: what a photograph of a deeper floor starts
    /// from.
    #[test]
    fn a_run_started_on_a_floor_begins_on_that_floor() {
        for floor in 1..=FLOORS {
            let mut app = headless(RunSeed(7));
            app.insert_resource(StartFloor(floor));
            let me = adventurer(&mut app);
            let entry = app.world().resource::<WorldMap>().place(map_of(floor)).map(|p| p.entry);
            assert_eq!((here(&app), Some(at(&app, me))), (map_of(floor), entry), "floor {floor}: on it, at its way in");
        }
    }

    /// A run begun after another in the same launch has been nowhere: the
    /// deepest floor the last one reached is not the new one's.
    #[test]
    fn a_run_begun_after_another_has_reached_no_floor() {
        let mut app = headless(RunSeed(2));
        arrive(&mut app, map_of(3));
        assert_eq!(app.world().resource::<Deepest>().0, 3, "the run went down to floor three");
        app.world_mut().write_message(Restart::fresh());
        settle(&mut app);
        settle(&mut app);
        assert_eq!(app.world().resource::<Deepest>().0, 0, "the next run has been nowhere");
        let me = adventurer(&mut app);
        assert_eq!((here(&app), app.world().get::<OnMap>(me).map(|m| m.0)), (TOWN, Some(TOWN)), "and sets out from Mereholt");
    }
}
