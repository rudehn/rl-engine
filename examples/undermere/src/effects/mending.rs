//! What mends a body outright rather than by a roll: health made whole,
//! and every status that would never run out on its own taken off. The
//! temple's `mend` is both; a potion could be either.
//!
//! Each lands on its user, as [`Life`](super::bodies::Life) does, so a
//! rite taken up from the priest lands on whoever took it up.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::UNTIL_CURED;
use rl_engine::rl_rules::ability::RawValue;

use super::moving::no_args;

/// Heals its user to their greatest health, however far below it they
/// were. Written `(kind: "Whole")`.
///
/// Set rather than rolled through the engine's `Heal`, since "whole" is a
/// number the body already holds: a roll big enough would still be scaled
/// by the user's mending, and a body that mends badly would come out of
/// it short.
#[derive(Debug, Clone, Copy, Default)]
pub struct Whole;

impl Effect for Whole {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let user = landing.user;
        world.commands.queue(move |world: &mut World| make_whole(world, user));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "heals whole".to_string()
    }

    fn mends(&self) -> bool {
        true
    }
}

impl FromArgs for Whole {
    const KIND: &'static str = "Whole";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Sets `who`'s health to the most it can be, and tells the adventurer
/// their wounds closed, or that there were none.
pub fn make_whole(world: &mut World, who: Entity) {
    let Some(mut health) = world.get_mut::<Health>(who) else { return };
    let wounded = health.current < health.max;
    health.current = health.max;
    if world.get::<Player>(who).is_none() {
        return;
    }
    let told = if wounded { Tell::new("Your wounds close.", Tones::GOOD) } else { Tell::new("You were whole already.", Tones::MUTED) };
    world.write_message(told);
}

/// Cures every status on its user that lasts until cured, but one a worn
/// thing holds, which is the thing's to end when it comes off: a ring's
/// regeneration stays. Written `(kind: "CureLasting")`.
///
/// A status with turns to run is left to run them; the ones taken off are
/// those nothing else would ever end. Through the engine's `Cure`, so each
/// is told of as any cure is.
#[derive(Debug, Clone, Copy, Default)]
pub struct CureLasting;

impl Effect for CureLasting {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let user = landing.user;
        world.commands.queue(move |world: &mut World| cure_lasting(world, user));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "cures whatever would never pass".to_string()
    }
}

impl FromArgs for CureLasting {
    const KIND: &'static str = "CureLasting";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Asks a [`Cure`] for every status on `who` that lasts until cured and
/// nothing worn holds.
pub fn cure_lasting(world: &mut World, who: Entity) {
    let Some(held) = world.get::<Afflicted>(who) else { return };
    let lasting: Vec<StatusId> = held.0.iter().filter(|s| s.turns == UNTIL_CURED && s.held_by.is_none()).map(|s| s.id).collect();
    for status in lasting {
        world.write_message(Cure { target: who, status });
    }
}
