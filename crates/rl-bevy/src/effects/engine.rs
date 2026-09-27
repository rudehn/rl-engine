//! The effects the engine ships: what an effect list can ask of the
//! subsystems the engine owns.
//!
//! `Harm`, `Mend` and `Inflict` grow with the enchant level of what landed
//! them, through `per_level`; the others do the same thing at any level.
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
    /// How much, rolled per target, at level zero.
    pub roll: DiceRoll,
    /// Added to the roll for each enchant level of what landed it.
    pub per_level: i32,
}

impl Harm {
    /// The roll at `level`; a level below one adds nothing.
    pub fn roll_at(&self, level: i32) -> DiceRoll {
        DiceRoll { bonus: self.roll.bonus + self.per_level * level.max(0), ..self.roll }
    }
}

impl Effect for Harm {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let roll = self.roll_at(landing.level);
        for target in &landing.targets {
            let amount = roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent::new(*target, Hit::by(landing.user, self.kind, amount)));
        }
    }

    fn describe(&self, registries: &Registries, level: i32) -> String {
        format!("{} {}", self.roll_at(level), registries.damage_kinds.name(self.kind))
    }
}

impl FromArgs for Harm {
    const KIND: &'static str = "Harm";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            kind: String,
            roll: String,
            #[serde(default)]
            per_level: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { kind: names.damage_kind(&a.kind)?, roll: a.roll.parse().map_err(|e| format!("{e}"))?, per_level: a.per_level })
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
    /// How much, rolled per target, at level zero.
    pub roll: DiceRoll,
    /// Added to the roll for each enchant level of what landed it.
    pub per_level: i32,
}

impl Mend {
    /// The roll at `level`; a level below one adds nothing.
    pub fn roll_at(&self, level: i32) -> DiceRoll {
        DiceRoll { bonus: self.roll.bonus + self.per_level * level.max(0), ..self.roll }
    }
}

impl Effect for Mend {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let roll = self.roll_at(landing.level);
        for target in &landing.targets {
            let amount = roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent::new(*target, Hit::by(landing.user, self.kind, -amount)));
        }
    }

    fn describe(&self, registries: &Registries, level: i32) -> String {
        format!("mends {} {}", self.roll_at(level), registries.damage_kinds.name(self.kind))
    }
}

impl FromArgs for Mend {
    const KIND: &'static str = "Mend";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            kind: String,
            roll: String,
            #[serde(default)]
            per_level: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { kind: names.damage_kind(&a.kind)?, roll: a.roll.parse().map_err(|e| format!("{e}"))?, per_level: a.per_level })
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
    /// For how many whole turns, at level zero.
    pub turns: u32,
    /// Turns added for each enchant level of what landed it.
    pub per_level: u32,
    /// Whether the status lasts only while the thing that landed it is
    /// worn. Read only when a trigger landed it, since only a thing can be
    /// worn: the [`Afflict`] is then held by the thing, held for every
    /// target under the footprint alike, and an ability or an offer lands
    /// the status as if this were false.
    ///
    /// Meant for a trigger that only ever lands on its own wearer, a `use`
    /// on oneself: a held refresh reaching a bystander who already carries
    /// the same status unheld takes their instance over, and since the
    /// bystander does not wear the thing, the very next pass cures it, so
    /// writing this true on an area or a `hit` ends the status early on
    /// anyone it was not meant to hold for.
    pub while_worn: bool,
}

impl Inflict {
    /// The turns at `level`; a level below one adds nothing.
    pub fn turns_at(&self, level: i32) -> u32 {
        self.turns + self.per_level * level.max(0) as u32
    }
}

impl Effect for Inflict {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let held_by = match landing.source {
            super::Source::Trigger { on, .. } if self.while_worn => Some(on),
            _ => None,
        };
        for target in &landing.targets {
            world.afflict.write(Afflict { target: *target, status: self.status, turns: self.turns_at(landing.level), by: Some(landing.user), held_by });
        }
    }

    fn describe(&self, registries: &Registries, level: i32) -> String {
        let held = if self.while_worn { " while worn" } else { "" };
        format!("{} for {} turns{held}", registries.statuses.name(self.status), self.turns_at(level))
    }
}

impl FromArgs for Inflict {
    const KIND: &'static str = "Inflict";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            status: String,
            turns: u32,
            #[serde(default)]
            per_level: u32,
            #[serde(default)]
            while_worn: bool,
        }
        let a: Args = read_args(args)?;
        Ok(Self { status: names.status(&a.status)?, turns: a.turns, per_level: a.per_level, while_worn: a.while_worn })
    }
}

/// Take a status off everyone under the footprint.
#[derive(Debug, Clone, Copy)]
pub struct Cleanse {
    /// Which status.
    pub status: StatusId,
}

impl Effect for Cleanse {
    fn describe(&self, registries: &Registries, _: i32) -> String {
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
    fn describe(&self, _: &Registries, _: i32) -> String {
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
    fn describe(&self, _: &Registries, _: i32) -> String {
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
    fn describe(&self, _: &Registries, _: i32) -> String {
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

    fn describe(&self, _: &Registries, _: i32) -> String {
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
/// A full cell is 255, and what a cell has no room for spills to the nearest
/// cells, so a footprint of one cell and an amount of thousands is a cloud
/// that fills a room: a grenade's burst of smoke.
///
/// Registered by [`GasPlugin`](crate::gas::GasPlugin), which is what answers
/// it.
#[derive(Debug, Clone, Copy)]
pub struct Emit {
    /// Which gas.
    pub gas: GasId,
    /// How much on each cell, a full cell being 255.
    pub amount: u16,
}

impl Effect for Emit {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for cell in &landing.cells {
            world.commands.write_message(crate::gas::Release { gas: self.gas, at: *cell, amount: self.amount });
        }
    }

    fn describe(&self, registries: &Registries, _: i32) -> String {
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
            amount: u16,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// What a level adds: a point on the roll per level for harm and a
    /// mend, a turn per level for a status, and nothing at all below `+1`.
    #[test]
    fn each_level_adds_its_per_level_and_a_plain_thing_adds_nothing() {
        let kind = DamageKindId::from_raw(0);
        let mend = Mend { kind, roll: DiceRoll::flat(1), per_level: 2 };
        assert_eq!((mend.roll_at(0), mend.roll_at(3)), (DiceRoll::flat(1), DiceRoll::flat(7)));
        let harm = Harm { kind, roll: DiceRoll::new(2, 6), per_level: 1 };
        assert_eq!(harm.roll_at(2), DiceRoll { num: 2, sides: 6, bonus: 2 });
        let hiding = Inflict { status: StatusId::from_raw(0), turns: 5, per_level: 1, while_worn: false };
        assert_eq!((hiding.turns_at(0), hiding.turns_at(2), hiding.turns_at(-1)), (5, 7, 5), "a negative level is plain, never shorter");
    }

    /// The arguments read `per_level` when it is written and nought when
    /// it is not, so every content file written before this still loads.
    #[test]
    fn per_level_is_read_when_written_and_nought_when_not() {
        let kinds = rl_rules::Registry::from_defs(vec![rl_rules::DamageKind::new("care")]).unwrap();
        let statuses = rl_rules::Registry::from_defs(vec![rl_rules::StatusDef::new("hidden")]).unwrap();
        let names = Names::new().damage_kinds(&kinds).statuses(&statuses);
        let args = |text: &str| rl_rules::ability::parse_args(text).unwrap();
        let old = Mend::from_args(&args(r#"(kind: "care", roll: "4")"#), &names).unwrap();
        assert_eq!(old.per_level, 0);
        let new = Inflict::from_args(&args(r#"(status: "hidden", turns: 5, per_level: 1)"#), &names).unwrap();
        assert_eq!(new.per_level, 1);
    }

    /// `while_worn` is read when written and false when not, and a held
    /// status says so where it is described, which is the line the bag
    /// shows under the thing.
    #[test]
    fn while_worn_is_read_when_written_and_said_where_the_effect_is_described() {
        let statuses = rl_rules::Registry::from_defs(vec![rl_rules::StatusDef::new("hidden")]).unwrap();
        let names = Names::new().statuses(&statuses);
        let args = |text: &str| rl_rules::ability::parse_args(text).unwrap();
        let plain = Inflict::from_args(&args(r#"(status: "hidden", turns: 5)"#), &names).unwrap();
        assert!(!plain.while_worn);
        let held = Inflict::from_args(&args(r#"(status: "hidden", turns: 10, per_level: 2, while_worn: true)"#), &names).unwrap();
        assert!(held.while_worn);
        let registries = Registries { statuses, ..Default::default() };
        assert_eq!(plain.describe(&registries, 0), "hidden for 5 turns");
        assert_eq!(held.describe(&registries, 2), "hidden for 14 turns while worn");
    }
}
