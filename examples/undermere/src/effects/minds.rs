//! Whose side someone is on, and what they know of the adventurer: a wand
//! of domination's foe bound to the adventurer for good, a scroll of
//! discord's foes turned on everyone near them for a while, and a scroll
//! of aggravate monsters' whole floor woken to where its reader stands.
//!
//! A side is the faction matrix's, which every mind and every blow reads
//! live, so binding a foe is changing its faction and its mind, which
//! [`befriend`] does, and a discordant one's side is changed for as long as
//! its status lasts by [`conditions`](crate::conditions); nothing in the
//! rules has to be told. What a monster knows of the adventurer is
//! stealth's [`Aware`], which a shriek fills in.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::EffectRng;
use rl_engine::rl_rules::StatusId;
use rl_engine::rl_rules::ability::RawValue;

use super::moving::no_args;
use super::the;
use crate::allies::{Ally, befriend};
use crate::bestiary::{Bestiary, Kind};

/// The share of its health, in percent, at or under which a foe is bound
/// for certain: a quarter, so beating one down first is how to make sure.
pub const SURE_UNDER: i32 = 25;

/// Binds everyone it struck to its user's side for good, certainly at or
/// under [`SURE_UNDER`] percent of its health and otherwise as often in a
/// hundred as it has lost: a wand of domination's. A bound foe follows the
/// adventurer and fights what they fight. Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dominate;

impl Effect for Dominate {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let targets = landing.targets.clone();
        world.commands.queue(move |world: &mut World| {
            for target in targets {
                dominate(world, target);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "binds it to you for good, the surer the more it is hurt".to_string()
    }
}

impl FromArgs for Dominate {
    const KIND: &'static str = "Dominate";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// The chance in a hundred that a foe at `health` is bound: certain at or
/// under [`SURE_UNDER`] percent, and otherwise what it has lost.
pub fn chance(health: Health) -> u32 {
    let left = health.current.max(0) * 100 / health.max.max(1);
    if left <= SURE_UNDER { 100 } else { (100 - left).max(0) as u32 }
}

/// Binds `target` to the adventurer's side, or fails to, as [`Dominate`]
/// says, and says which. The Drowned King is bound by nobody, and one
/// already on the adventurer's side is left as it is.
pub fn dominate(world: &mut World, target: Entity) {
    let (Some(kind), Some(health)) = (world.get::<Kind>(target).map(|k| k.0), world.get::<Health>(target).copied()) else { return };
    if world.get::<Ally>(target).is_some() {
        return;
    }
    let name = world.get::<Name>(target).map_or_else(String::new, |n| n.as_str().to_string());
    let boss = world.resource::<Bestiary>().defs.get(kind).boss;
    let roll = world.resource_mut::<EffectRng>().random_range(0..100);
    let (line, tone) = if !boss && roll < chance(health) {
        befriend(world, target);
        (format!("{} is bound to your will.", the(&name)), Tones::GOOD)
    } else {
        (format!("{} shakes off your will.", the(&name)), Tones::BAD)
    };
    world.write_message(Tell::new(line, tone));
}

/// Every foe its user can see made discordant for `turns` turns, at odds
/// with every side, its own among them: a scroll of discord's. With no foe
/// in sight it shows nothing. Written `(kind: "Discord", args: (turns:
/// 10))`; the status is the game's own `discordant`, looked up as the file
/// is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Discord {
    /// Whole turns each foe stays discordant, before any bonus.
    pub turns: u32,
    /// `discordant`.
    status: StatusId,
}

impl Effect for Discord {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (landing, status, turns) = (landing.clone(), self.status, self.turns + landing.bonus.turns);
        world.commands.queue(move |world: &mut World| {
            let foes = in_sight(world, landing.user);
            if foes.is_empty() {
                super::showed_nothing(world, &landing);
                return;
            }
            for foe in foes {
                world.write_message(Afflict { target: foe, status, turns, by: Some(landing.user), held_by: None });
            }
            world.write_message(Tell::new("A wail of discord rises, and your foes turn on each other!", Tones::GOOD));
        });
    }

    fn describe(&self, _: &Registries, bonus: EffectBonus) -> String {
        format!("every foe in sight fights everyone near it for {} turns", self.turns + bonus.turns)
    }
}

impl FromArgs for Discord {
    const KIND: &'static str = "Discord";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            turns: u32,
        }
        let a: Args = rl_engine::rl_rules::ability::read_args(args)?;
        Ok(Self { turns: a.turns, status: names.status(crate::content::DISCORDANT)? })
    }
}

/// Every monster `who` can see that is at odds with them, by the faction
/// matrix and their viewshed, in spawn order. Nobody for one with no sight
/// to ask.
pub fn in_sight(world: &mut World, who: Entity) -> Vec<Entity> {
    let (Some(sight), Some(side)) = (world.get::<Viewshed>(who).cloned(), world.get::<Faction>(who).copied()) else { return Vec::new() };
    let here = world.resource::<WorldMap>().current();
    let factions = world.resource::<CombatRules>().factions.clone();
    let mut seen = world.query_filtered::<(Entity, &Position, &OnMap, &Faction), (With<Kind>, With<Actor>)>();
    let mut foes: Vec<Entity> =
        seen.iter(world).filter(|(_, at, on, theirs)| on.0 == here && sight.can_see(at.0) && factions.is_hostile(side.0, theirs.0)).map(|(e, ..)| e).collect();
    foes.sort_by_key(|e| e.index());
    foes
}

/// A shriek heard over the whole floor, and every monster on it made aware
/// of where its user stands: a scroll of aggravate monsters'. Takes no
/// arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Aggravate;

impl Effect for Aggravate {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let user = landing.user;
        world.commands.queue(move |world: &mut World| aggravate(world, user));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "every monster on the floor knows where you are".to_string()
    }
}

impl FromArgs for Aggravate {
    const KIND: &'static str = "Aggravate";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// How far the shriek carries over open ground, in steps of loudness:
/// across any floor under the lake, so whatever has ears hears it.
pub const SHRIEK: i32 = 200;

/// Makes the shriek where `reader` stands and every monster on their floor
/// aware of them there, as [`Aggravate`] says.
pub fn aggravate(world: &mut World, reader: Entity) {
    let Some(at) = world.get::<Position>(reader).map(|p| p.0) else { return };
    let here = world.get::<OnMap>(reader).map_or(MapId::SURFACE, |m| m.0);
    let mut woken = world.query_filtered::<(&OnMap, &mut Aware), With<Kind>>();
    for (on, mut aware) in woken.iter_mut(world) {
        if on.0 != here {
            continue;
        }
        let mut knows = aware.of(reader);
        knows.alerted_to(at);
        aware.0.insert(reader, knows);
    }
    if let Some(sound) = world.resource::<Sounds>().get(SHRIEK_SOUND) {
        world.write_message(MakeNoise { at, loudness: SHRIEK, sound, maker: Some(reader) });
    }
    if world.get::<Player>(reader).is_some() {
        world.write_message(Tell::new("A piercing shriek echoes through the floor!", Tones::BAD));
    }
}

/// What the shriek sounds like, declared by the game with its noise.
pub const SHRIEK_SOUND: &str = "shriek";

/// Makes every one of the drowned on `user`'s own floor aware of where
/// `user` stands, however far off: the Abbess's chalice, each drink heard
/// by the drowned wherever they are, unlike [`Aggravate`]'s shriek, which
/// carries no further than a noise does and wakes everything under the
/// lake rather than one kind of it. Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AlertDrowned;

impl Effect for AlertDrowned {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let user = landing.user;
        world.commands.queue(move |world: &mut World| alert_drowned(world, user));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "every drowned thing on the floor senses where you are".to_string()
    }
}

impl FromArgs for AlertDrowned {
    const KIND: &'static str = "AlertDrowned";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Whether a kind's own name says it is one of the drowned: `drowned
/// thrall`, `the Drowned Abbess`, `the Drowned King`, read loosely by the
/// word rather than by a trait or a faction, since none of the three
/// share either and the word is the one thing the content sheet gives
/// them in common.
fn is_drowned(name: &str) -> bool {
    name.to_lowercase().contains("drowned")
}

/// Makes every one of the drowned on `reader`'s own floor aware of them
/// where they stand, as [`AlertDrowned`] says: no noise at all, since the
/// chalice's own cost is heard by the drowned wherever they are and not
/// by how far a sound carries.
pub fn alert_drowned(world: &mut World, reader: Entity) {
    let Some(at) = world.get::<Position>(reader).map(|p| p.0) else { return };
    let here = world.get::<OnMap>(reader).map_or(MapId::SURFACE, |m| m.0);
    let drowned: std::collections::BTreeSet<_> = world.resource::<Bestiary>().defs.iter().filter(|(_, def)| is_drowned(&def.name)).map(|(id, _)| id).collect();
    if drowned.is_empty() {
        return;
    }
    let targets: Vec<Entity> = world
        .query_filtered::<(Entity, &OnMap, &Kind), With<Actor>>()
        .iter(world)
        .filter(|(_, on, kind)| on.0 == here && drowned.contains(&kind.0))
        .map(|(e, ..)| e)
        .collect();
    for e in targets {
        if let Some(mut aware) = world.get_mut::<Aware>(e) {
            let mut knows = aware.of(reader);
            knows.alerted_to(at);
            aware.0.insert(reader, knows);
        }
    }
}
