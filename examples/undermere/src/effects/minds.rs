//! Whose side someone is on: a wand of domination's foe bound to the
//! adventurer for good.
//!
//! A side is the faction matrix's, which every mind and every blow reads
//! live, so binding a foe is changing its faction and its mind, which
//! [`befriend`] does, and nothing in the rules has to be told.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::EffectRng;
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
