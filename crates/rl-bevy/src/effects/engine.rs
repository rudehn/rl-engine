//! The effects the engine ships: what an effect list can ask of the
//! subsystems the engine owns.
//!
//! [`Harm`] and [`Mend`] ask combat's damage pipeline, [`Inflict`] and
//! [`Cleanse`] ask statuses, and [`Shove`], [`Pull`] and [`Teleport`] move
//! an actor through [`EffectWorld`]; [`AddEngineEffects`] registers those
//! seven. [`Ignite`] asks fire and [`Emit`] asks gas, and each is registered
//! by the plugin that answers it, so a content file naming one works exactly
//! when the game has that subsystem. Here, rather than each in the module
//! that owns its mechanic, so the dependency runs one way: effects are built
//! on combat, statuses, fire and gas, and none of those has to know an
//! effect list exists.

use bevy::prelude::*;
use rl_core::{DiceRoll, Point};
use rl_rules::ability::{RawValue, read_args};
use rl_rules::damage::DamageKindId;
use rl_rules::gas::GasId;
use rl_rules::{Hit, Names, StatusId};

use super::{AddEffect, Effect, EffectWorld, FromArgs, Landing};
use crate::combat::DamageEvent;
use crate::registries::Registries;
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
            world.damage.write(DamageEvent::new(*target, Hit::by(landing.user, self.kind, amount)));
        }
    }

    fn describe(&self, registries: &Registries) -> String {
        format!("{} {}", self.roll, registries.damage_kinds.name(self.kind))
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
            world.damage.write(DamageEvent::new(*target, Hit::by(landing.user, self.kind, -amount)));
        }
    }

    fn describe(&self, registries: &Registries) -> String {
        format!("mends {} {}", self.roll, registries.damage_kinds.name(self.kind))
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

    fn describe(&self, registries: &Registries) -> String {
        format!("{} for {} turns", registries.statuses.name(self.status), self.turns)
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
    fn describe(&self, registries: &Registries) -> String {
        format!("cures {}", registries.statuses.name(self.status))
    }

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
    fn describe(&self, _: &Registries) -> String {
        format!("shoves {} back", cells(self.cells))
    }

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
    fn describe(&self, _: &Registries) -> String {
        format!("pulls {} closer", cells(self.cells))
    }

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
    fn describe(&self, _: &Registries) -> String {
        "moves you there".to_string()
    }

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

/// Set fire to every cell under the footprint, for at least `turns`.
///
/// A cell with nothing to burn burns that long and goes out, which is a
/// fireball scorching bare stone; one with something to burn catches and
/// burns as that does. Registered by [`FirePlugin`](crate::fire::FirePlugin),
/// which is what answers it.
#[derive(Debug, Clone, Copy)]
pub struct Ignite {
    /// For at least how many turns.
    pub turns: u8,
}

impl Effect for Ignite {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for cell in &landing.cells {
            world.commands.write_message(crate::fire::Kindle { at: *cell, turns: self.turns });
        }
    }

    fn describe(&self, _: &Registries) -> String {
        format!("sets the ground alight for {} turns", self.turns)
    }
}

impl FromArgs for Ignite {
    const KIND: &'static str = "Ignite";

    fn from_args(args: &RawValue, _names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            turns: u8,
        }
        let a: Args = read_args(args)?;
        Ok(Self { turns: a.turns })
    }
}

/// Give off `amount` of a gas on every cell under the footprint.
///
/// Registered by [`GasPlugin`](crate::gas::GasPlugin), which is what answers
/// it.
#[derive(Debug, Clone, Copy)]
pub struct Emit {
    /// Which gas.
    pub gas: GasId,
    /// How much on each cell.
    pub amount: u8,
}

impl Effect for Emit {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for cell in &landing.cells {
            world.commands.write_message(crate::gas::Release { gas: self.gas, at: *cell, amount: self.amount });
        }
    }

    fn describe(&self, registries: &Registries) -> String {
        format!("gives off {}", registries.gases.name(self.gas))
    }
}

/// `n` cells, as a phrase.
fn cells(n: i32) -> String {
    if n == 1 { "a cell".to_string() } else { format!("{n} cells") }
}

impl FromArgs for Emit {
    const KIND: &'static str = "Emit";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            gas: String,
            amount: u8,
        }
        let a: Args = read_args(args)?;
        Ok(Self { gas: names.gas(&a.gas)?, amount: a.amount })
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
