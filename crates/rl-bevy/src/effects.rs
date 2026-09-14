//! The effects the engine ships: what an ability can do with the subsystems
//! the engine owns.
//!
//! Seven, one per mechanic. [`Harm`] and [`Mend`] ask combat's damage
//! pipeline, [`Inflict`] and [`Cleanse`] ask statuses, and [`Shove`],
//! [`Pull`] and [`Teleport`] move an actor through [`EffectWorld`]. Here,
//! rather than each in the module that owns its mechanic, so the dependency
//! runs one way: abilities are built on combat and statuses, and neither of
//! those has to know an ability exists.
//!
//! A game's own effects sit beside these through
//! [`AddEffect::add_effect`], and the resolver cannot tell them apart.

use bevy::prelude::*;
use rl_core::{DiceRoll, Point};
use rl_rules::ability::{RawValue, read_args};
use rl_rules::damage::DamageKindId;
use rl_rules::{Hit, Names, StatusId};

use crate::ability::{AddEffect, Effect, EffectWorld, FromArgs, Landing};
use crate::combat::DamageEvent;
use crate::status::{Afflict, Cure};

/// Damage everyone the ability's aim wanted under its footprint.
///
/// Through [`DamageEvent`] rather than onto health directly, so a
/// fireball is mitigated by the same armor, resistances and stages a
/// sword is, and a game that inserts a stage gets it on both at once.
#[derive(Debug, Clone, Copy)]
pub struct Harm {
    /// What kind of damage.
    pub kind: DamageKindId,
    /// How much, rolled per target.
    pub roll: DiceRoll,
}

impl Effect for Harm {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in &landing.targets {
            let amount = self.roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent { target: *target, hit: Hit::by(landing.user, self.kind, amount) });
        }
    }
}

impl FromArgs for Harm {
    const KIND: &'static str = "Harm";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            kind: String,
            roll: String,
        }
        let a: Args = read_args(args)?;
        Ok(Self { kind: names.damage_kind(&a.kind)?, roll: a.roll.parse().map_err(|e| format!("{e}"))? })
    }
}

/// Heal everyone under the footprint.
///
/// Negative damage of a named kind, so resistance to it is a game's to
/// define: a construct that resists the kind a medkit deals cannot be
/// patched up, and nothing in the engine had to learn the word undead.
#[derive(Debug, Clone, Copy)]
pub struct Mend {
    /// The kind healing counts as.
    pub kind: DamageKindId,
    /// How much, rolled per target.
    pub roll: DiceRoll,
}

impl Effect for Mend {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in &landing.targets {
            let amount = self.roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent { target: *target, hit: Hit::by(landing.user, self.kind, -amount) });
        }
    }
}

impl FromArgs for Mend {
    const KIND: &'static str = "Mend";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            kind: String,
            roll: String,
        }
        let a: Args = read_args(args)?;
        Ok(Self { kind: names.damage_kind(&a.kind)?, roll: a.roll.parse().map_err(|e| format!("{e}"))? })
    }
}

/// Put a status on everyone under the footprint.
///
/// Named for what it does rather than for the message it writes, because
/// [`Afflict`] is already the request and an effect is not a request.
#[derive(Debug, Clone, Copy)]
pub struct Inflict {
    /// Which status.
    pub status: StatusId,
    /// For how many whole turns.
    pub turns: u32,
}

impl Effect for Inflict {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in &landing.targets {
            world.afflict.write(Afflict { target: *target, status: self.status, turns: self.turns, by: Some(landing.user) });
        }
    }
}

impl FromArgs for Inflict {
    const KIND: &'static str = "Inflict";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            status: String,
            turns: u32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { status: names.status(&a.status)?, turns: a.turns })
    }
}

/// Take a status off everyone under the footprint.
#[derive(Debug, Clone, Copy)]
pub struct Cleanse {
    /// Which status.
    pub status: StatusId,
}

impl Effect for Cleanse {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in &landing.targets {
            world.cure.write(Cure { target: *target, status: self.status });
        }
    }
}

impl FromArgs for Cleanse {
    const KIND: &'static str = "Cleanse";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            status: String,
        }
        let a: Args = read_args(args)?;
        Ok(Self { status: names.status(&a.status)? })
    }
}

/// Push everyone under the footprint away from the user.
///
/// Here rather than in the turn loop because the move goes through
/// [`EffectWorld::slide`], which is what keeps a shove out of a wall and
/// the occupancy index straight.
#[derive(Debug, Clone, Copy)]
pub struct Shove {
    /// How many cells.
    pub cells: i32,
}

impl Effect for Shove {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in landing.targets.clone() {
            let Some(at) = world.position(target) else { continue };
            let away = Point::new(at.x + (at.x - landing.origin.x).signum(), at.y + (at.y - landing.origin.y).signum());
            world.slide(target, at, away, self.cells);
        }
    }
}

impl FromArgs for Shove {
    const KIND: &'static str = "Shove";

    fn from_args(args: &RawValue, _names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            cells: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { cells: a.cells })
    }
}

/// Drag everyone under the footprint towards the user.
#[derive(Debug, Clone, Copy)]
pub struct Pull {
    /// How many cells.
    pub cells: i32,
}

impl Effect for Pull {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in landing.targets.clone() {
            let Some(at) = world.position(target) else { continue };
            world.slide(target, at, landing.origin, self.cells);
        }
    }
}

impl FromArgs for Pull {
    const KIND: &'static str = "Pull";

    fn from_args(args: &RawValue, _names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            cells: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { cells: a.cells })
    }
}

/// Move the user to where the ability landed.
///
/// Refused rather than approximated when the cell will not take it: a
/// blink that lands you inside a wall is worse than a blink that fizzles,
/// and the turn is spent either way.
#[derive(Debug, Clone, Copy, Default)]
pub struct Teleport;

impl Effect for Teleport {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let Some(to) = landing.landed_at.or(Some(landing.aim)) else { return };
        world.place(landing.user, to);
    }
}

impl FromArgs for Teleport {
    const KIND: &'static str = "Teleport";

    fn from_args(_args: &RawValue, _names: &Names<'_>) -> Result<Self, String> {
        Ok(Self)
    }
}

/// The effects the engine ships, registered together.
///
/// A convenience, not a requirement: a game that wants three of them
/// registers three, and one that wants none registers none. Nothing is
/// registered by default, because an ability file naming an effect the
/// game did not ask for should fail at load rather than work by accident.
pub trait AddEngineEffects {
    /// Registers `Harm`, `Mend`, `Inflict`, `Cleanse`, `Shove`, `Pull` and
    /// `Teleport`.
    fn add_engine_effects(&mut self) -> &mut Self;
}

impl AddEngineEffects for App {
    fn add_engine_effects(&mut self) -> &mut Self {
        self.add_effect::<Harm>()
            .add_effect::<Mend>()
            .add_effect::<Inflict>()
            .add_effect::<Cleanse>()
            .add_effect::<Shove>()
            .add_effect::<Pull>()
            .add_effect::<Teleport>()
    }
}
