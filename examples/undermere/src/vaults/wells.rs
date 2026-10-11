//! The reward pieces used where they stand, each once: a healing spring
//! drunk from, a cold forge's anvil tempering a worn weapon or armor, and
//! a moonwell telling what one carried thing is.
//!
//! Each is a place rather than a thing to carry off, as the pool means
//! it: a spring found at full health is worth coming back to, and an
//! anvil that tempered anything carried would be a scroll of enchanting,
//! which the enchanter's altar already gives. The spring's draught is its
//! offer's own `Mend`, written in `assets/props.ron`. The anvil and the
//! moonwell choose a thing first, which a prop's written offer cannot, so
//! [`offer_at_wells`] puts a choosing offer on each as it is stood up,
//! choosing among what [`temper_choices`] and [`tell_choices`] name.
//! [`answer_wells`] does what each was asked and stands it down spent.
//!
//! Told apart by what a prop is, which the save keeps, so a continued run
//! offers again what was not used and nothing that was.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::props::{Among, Chooser, Scenery, spawn_prop};

use crate::armory::{Armory, ItemKind};
use crate::lore::{Lore, Unfamiliar};

/// A healing spring, offering [`DRINK_VERB`] once, its draught a full
/// mend.
pub const SPRING: &str = "spring";
/// What a spring is once drunk from.
pub const DRY_SPRING: &str = "dry spring";
/// A cold forge's anvil, offering [`TEMPER_VERB`] once.
pub const ANVIL: &str = "frost-rimed anvil";
/// What an anvil is once it has tempered something.
pub const COLD_ANVIL: &str = "cold anvil";
/// A moonwell, offering [`DRINK_VERB`] once, telling what one carried
/// thing is.
pub const MOONWELL: &str = "moonwell";
/// What a moonwell is once drunk from.
pub const DARK_MOONWELL: &str = "darkened moonwell";
/// What a spring and a moonwell offer, declared by
/// [`crate::plugin::UndermerePlugin`].
pub const DRINK_VERB: &str = "drink";
/// What an anvil offers, declared by [`crate::plugin::UndermerePlugin`].
pub const TEMPER_VERB: &str = "temper";
/// What tempering or drinking takes, a step's worth.
pub const WELL_TIME: u32 = 100;

/// The systems the anvil's and the moonwell's offers choose among,
/// registered once by the plugin.
#[derive(Resource, Debug, Clone, Copy)]
pub struct WellChoices {
    /// What an anvil may temper: [`temper_choices`].
    pub temper: Among,
    /// What a moonwell may tell of: [`tell_choices`].
    pub tell: Among,
}

/// Puts its choosing offer on an anvil or a moonwell as it is stood up,
/// on a floor's first arrival or a continued run's, and takes off the
/// [`Scenery`] marker its written definition earns it, so it is listed
/// with what can be acted on.
///
/// An observer of that marker going on, which `spawn_prop` puts on every
/// prop like these as it stands it up, rather than a system in the turn:
/// one more system there reorders the systems around it that nothing
/// orders, and the engine's `close_emptied_containers` then marked a
/// vault's prize container emptied before it was stocked.
pub fn offer_at_wells(
    stood: On<Add, Scenery>,
    mut commands: Commands,
    names: Query<&Name, With<PropKind>>,
    verbs: Option<Res<Verbs>>,
    choices: Option<Res<WellChoices>>,
) {
    // By the name `spawn_prop` gives it, and every resource optional: a
    // place is often stood up while a system has the registries out of the
    // world, and only an anvil or a moonwell needs the others.
    let Ok(name) = names.get(stood.entity) else { return };
    if name.as_str() != ANVIL && name.as_str() != MOONWELL {
        return;
    }
    let (verbs, choices) = (verbs.expect("the verbs stand before any place"), choices.expect("UndermerePlugin registers the wells' choices"));
    let offer = if name.as_str() == ANVIL {
        Offering::new(verbs.get(TEMPER_VERB).expect("UndermerePlugin declares temper"), WELL_TIME).choosing("Temper which?", choices.temper)
    } else {
        Offering::new(verbs.get(DRINK_VERB).expect("UndermerePlugin declares drink"), WELL_TIME).choosing("Learn the nature of which?", choices.tell)
    };
    commands.entity(stood.entity).insert(Offers(vec![offer])).remove::<Scenery>();
}

/// What an anvil may temper: each weapon or piece of armor the taker
/// wears, once each, however many slots it fills. With none, says so, as
/// a choice with nothing to choose shuts without a word.
pub fn temper_choices(
    In(chooser): In<Chooser>,
    worn: Query<&Equipped>,
    kinds: Query<&ItemKind>,
    armory: Res<Armory>,
    registries: Res<Registries>,
    mut tell: MessageWriter<Tell>,
) -> Vec<Entity> {
    let tags: Vec<_> = ["weapon", "armor"].iter().map(|t| registries.tags.id(t)).collect();
    let gear = |thing: Entity| kinds.get(thing).is_ok_and(|k| armory.defs.get(k.0).tags.iter().any(|t| tags.contains(&Some(t.id()))));
    let mut choices: Vec<Entity> = worn.get(chooser.taker).map(|w| w.0.worn().map(|(_, thing)| thing).filter(|t| gear(*t)).collect()).unwrap_or_default();
    choices.dedup();
    if choices.is_empty() {
        tell.write(Tell::new("You wear nothing the anvil could temper.", Tones::MUTED));
    }
    choices
}

/// What a moonwell may tell of: each thing the taker carries that the
/// adventurer does not yet know whole, its kind or, for a thing learned
/// by wearing, its level. With none, says so.
pub fn tell_choices(
    In(chooser): In<Chooser>,
    bags: Query<&Inventory>,
    things: Query<(&ItemKind, Has<Unfamiliar>)>,
    armory: Res<Armory>,
    lore: Res<Lore>,
    mut tell: MessageWriter<Tell>,
) -> Vec<Entity> {
    let unknown = |thing: &Entity| {
        things.get(*thing).is_ok_and(|(kind, unfamiliar)| !lore.knows(&armory.defs.get(kind.0).name) || (unfamiliar && armory.learned_by_wear(kind.0)))
    };
    let choices: Vec<Entity> = bags.get(chooser.taker).map(|bag| bag.items.iter().copied().filter(unknown).collect()).unwrap_or_default();
    if choices.is_empty() {
        tell.write(Tell::new("You carry nothing the moonwell could tell you of.", Tones::MUTED));
    }
    choices
}

/// Answers a spring, an anvil or a moonwell used: the anvil tempers the
/// thing chosen a level and the moonwell tells what it is, as a scroll of
/// enchanting and one of identify would; the spring's draught has already
/// landed as its offer's own effect. Each is then stood down spent where
/// it stood, and says so.
///
/// In `TurnSet::React`, where every answer to an `Interacted` is.
pub fn answer_wells(
    mut commands: Commands,
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    registries: Res<Registries>,
    props: Query<(&PropKind, &Position, &OnMap)>,
) {
    let (Some(drink), Some(temper)) = (verbs.get(DRINK_VERB), verbs.get(TEMPER_VERB)) else { return };
    for ev in done.read() {
        let Ok((kind, at, on)) = props.get(ev.target) else { continue };
        let name = registries.props.get(kind.0).name.as_str();
        let (spent, line) = match (name, ev.thing) {
            (SPRING, _) if ev.verb == drink => (DRY_SPRING, "The spring sinks into the stone, and runs dry."),
            (ANVIL, Some(thing)) if ev.verb == temper => {
                commands.queue(move |world: &mut World| crate::enchant::enchant(world, thing));
                (COLD_ANVIL, "The frost on the anvil melts away.")
            }
            (MOONWELL, Some(thing)) if ev.verb == drink => {
                commands.queue(move |world: &mut World| crate::lore::identify(world, thing));
                (DARK_MOONWELL, "The light in the moonwell fades.")
            }
            _ => continue,
        };
        commands.entity(ev.target).despawn();
        spawn_prop(&mut commands, &registries, registries.props.expect(spent), at.0, on.0);
        commands.write_message(Tell::new(line, Tones::NOTICE));
    }
}
