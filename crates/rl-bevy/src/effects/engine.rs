//! The effects the engine ships: what an effect list can ask of the
//! subsystems the engine owns.
//!
//! `Harm`, `Mend` and `Inflict` grow with their carrier's `EffectBonus`;
//! the others ignore it.
//!
//! [`Harm`] and [`Mend`] ask combat's damage pipeline, [`Inflict`] and
//! [`Cleanse`] ask statuses, and [`Shove`], [`Pull`] and [`Teleport`] move
//! an actor through [`EffectWorld`]; [`AddEngineEffects`] registers those
//! seven. [`Ignite`] asks fire, [`Emit`] asks gas and [`Noise`] asks
//! hearing, and each is registered by the plugin that answers it, so a
//! content file naming one works exactly when the game has that subsystem.
//! Here, rather than each in the module that owns its mechanic, so the
//! dependency runs one way: effects are built on combat, statuses, fire,
//! gas and noise, and none of those has to know an effect list exists.

use bevy::prelude::*;
use rl_core::{DiceRoll, Point};
use rl_rules::ability::{RawValue, read_args};
use rl_rules::damage::DamageKindId;
use rl_rules::gas::GasId;
use rl_rules::{Hit, Names, StatusId};

use super::{AddEffect, Effect, EffectBonus, EffectWorld, FromArgs, Landing};
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
    /// How much, rolled per target, before any bonus.
    pub roll: DiceRoll,
}

impl Harm {
    /// The roll with `bonus`'s amount added.
    pub fn roll_with(&self, bonus: EffectBonus) -> DiceRoll {
        DiceRoll { bonus: self.roll.bonus + bonus.amount, ..self.roll }
    }
}

impl Effect for Harm {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let roll = self.roll_with(landing.bonus);
        for target in &landing.targets {
            let amount = roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent::new(*target, Hit::by(landing.user, self.kind, amount)));
        }
    }

    fn describe(&self, registries: &Registries, bonus: EffectBonus) -> String {
        format!("{} {}", self.roll_with(bonus), registries.damage_kinds.name(self.kind))
    }
}

impl FromArgs for Harm {
    const KIND: &'static str = "Harm";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
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
    /// How much, rolled per target, before any bonus.
    pub roll: DiceRoll,
}

impl Mend {
    /// The roll with `bonus`'s amount added.
    pub fn roll_with(&self, bonus: EffectBonus) -> DiceRoll {
        DiceRoll { bonus: self.roll.bonus + bonus.amount, ..self.roll }
    }
}

impl Effect for Mend {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let roll = self.roll_with(landing.bonus);
        for target in &landing.targets {
            let amount = roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent::new(*target, Hit::by(landing.user, self.kind, -amount)));
        }
    }

    fn describe(&self, registries: &Registries, bonus: EffectBonus) -> String {
        format!("mends {} {}", self.roll_with(bonus), registries.damage_kinds.name(self.kind))
    }
}

impl FromArgs for Mend {
    const KIND: &'static str = "Mend";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
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
    /// For how many whole turns, before any bonus.
    pub turns: u32,
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
    /// The turns with `bonus`'s turns added.
    pub fn turns_with(&self, bonus: EffectBonus) -> u32 {
        self.turns + bonus.turns
    }
}

impl Effect for Inflict {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let held_by = match landing.source {
            super::Source::Trigger { on, .. } if self.while_worn => Some(on),
            _ => None,
        };
        for target in &landing.targets {
            world.afflict.write(Afflict { target: *target, status: self.status, turns: self.turns_with(landing.bonus), by: Some(landing.user), held_by });
        }
    }

    fn describe(&self, registries: &Registries, bonus: EffectBonus) -> String {
        let held = if self.while_worn { " while worn" } else { "" };
        format!("{} for {} turns{held}", registries.statuses.name(self.status), self.turns_with(bonus))
    }
}

impl FromArgs for Inflict {
    const KIND: &'static str = "Inflict";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            status: String,
            turns: u32,
            #[serde(default)]
            while_worn: bool,
        }
        let a: Args = read_args(args)?;
        Ok(Self { status: names.status(&a.status)?, turns: a.turns, while_worn: a.while_worn })
    }
}

/// Take a status off everyone under the footprint.
#[derive(Debug, Clone, Copy)]
pub struct Cleanse {
    /// Which status.
    pub status: StatusId,
}

impl Effect for Cleanse {
    fn describe(&self, registries: &Registries, _: EffectBonus) -> String {
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
        #[serde(deny_unknown_fields)]
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
    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
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
        #[serde(deny_unknown_fields)]
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
    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
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
        #[serde(deny_unknown_fields)]
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
    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
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

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("sets the ground alight for {} turns", self.turns)
    }
}

impl FromArgs for Ignite {
    const KIND: &'static str = "Ignite";

    fn from_args(args: &RawValue, _names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
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

    fn describe(&self, registries: &Registries, _: EffectBonus) -> String {
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
        #[serde(deny_unknown_fields)]
        struct Args {
            gas: String,
            amount: u16,
        }
        let a: Args = read_args(args)?;
        Ok(Self { gas: names.gas(&a.gas)?, amount: a.amount })
    }
}

/// Make a noise where it landed, as its user making it: a grenade going
/// off, a trap's alarm, a dropped thing clattering.
///
/// One noise, at the cell a projectile stopped in or else the cell aimed
/// at, however many cells the footprint covers, because a burst is one
/// sound and not one per cell. Heard as any other
/// [`MakeNoise`](crate::noise::MakeNoise) is: its user does not hear it,
/// and a listener goes to see.
///
/// Registered by [`NoisePlugin`](crate::noise::NoisePlugin), which is what
/// answers it, so content naming it builds exactly when the game has
/// hearing. Written `(kind: "Noise", args: (sound: "name", loudness: N))`,
/// the sound one the engine or the game declared; the names it is built
/// against carry them through [`SoundNames`](crate::noise::SoundNames).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Noise {
    /// What it sounds like.
    pub sound: crate::noise::SoundId,
    /// How far it carries over open ground, in whole steps.
    pub loudness: i32,
}

impl Effect for Noise {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let at = landing.landed_at.unwrap_or(landing.aim);
        world.commands.write_message(crate::noise::MakeNoise { at, loudness: self.loudness, sound: self.sound, maker: Some(landing.user) });
    }

    // `describe` is left to say nothing: a noise is not something a thing
    // does to anyone, and a bag line reading "makes a noise" under every
    // grenade would crowd out what it does.
}

impl FromArgs for Noise {
    const KIND: &'static str = "Noise";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            sound: String,
            loudness: i32,
        }
        use crate::noise::{Sound, Sounds};
        let a: Args = read_args(args)?;
        // The engine's first sound is in every `Sounds`, so names that
        // cannot find it were built without any, which is the loader's
        // mistake rather than the file's, and the message says where to fix it.
        let sound = names.id::<Sound>(&a.sound).map_err(|e| match names.id::<Sound>(Sounds::BUILT_IN[0]) {
            Ok(_) => e,
            Err(_) => format!("{e}; the names effects are built against carry the sounds through `SoundNames::sounds`"),
        })?;
        Ok(Self { sound, loudness: a.loudness })
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

    /// A bonus adds its amount to a harm and a mend roll and its turns to
    /// an inflicted status, and a default bonus, what a plain carrier
    /// reads, adds nothing at all.
    #[test]
    fn a_bonus_adds_its_amount_to_harm_and_mend_and_its_turns_to_a_status_and_a_default_adds_nothing() {
        let kind = DamageKindId::from_raw(0);
        let bonus = EffectBonus { turns: 2, amount: 3 };
        let mend = Mend { kind, roll: DiceRoll::flat(1) };
        assert_eq!((mend.roll_with(EffectBonus::default()), mend.roll_with(bonus)), (DiceRoll::flat(1), DiceRoll::flat(4)));
        let harm = Harm { kind, roll: DiceRoll::new(2, 6) };
        assert_eq!(harm.roll_with(bonus), DiceRoll { num: 2, sides: 6, bonus: 3 });
        let hiding = Inflict { status: StatusId::from_raw(0), turns: 5, while_worn: false };
        assert_eq!((hiding.turns_with(EffectBonus::default()), hiding.turns_with(bonus)), (5, 7));
    }

    /// A `per_level` left in a `Harm`, a `Mend` or an `Inflict`'s
    /// arguments, which the engine no longer reads, is refused at load,
    /// naming the field, rather than silently ignored.
    #[test]
    fn per_level_in_harm_mend_or_inflict_args_is_refused_naming_it() {
        let kinds = rl_rules::Registry::from_defs(vec![rl_rules::DamageKind::new("care")]).unwrap();
        let statuses = rl_rules::Registry::from_defs(vec![rl_rules::StatusDef::new("hidden")]).unwrap();
        let names = Names::new().damage_kinds(&kinds).statuses(&statuses);
        let args = |text: &str| rl_rules::ability::parse_args(text).unwrap();
        let harm = Harm::from_args(&args(r#"(kind: "care", roll: "4", per_level: 1)"#), &names).expect_err("per_level is gone from Harm");
        assert!(harm.contains("per_level"), "{harm}");
        let mend = Mend::from_args(&args(r#"(kind: "care", roll: "4", per_level: 1)"#), &names).expect_err("per_level is gone from Mend");
        assert!(mend.contains("per_level"), "{mend}");
        let inflict = Inflict::from_args(&args(r#"(status: "hidden", turns: 5, per_level: 1)"#), &names).expect_err("per_level is gone from Inflict");
        assert!(inflict.contains("per_level"), "{inflict}");
    }

    /// A typo'd argument on another engine effect is refused the same way,
    /// naming it, rather than parsed away as a field nobody asked for.
    #[test]
    fn a_typod_argument_on_another_engine_effect_is_refused_naming_it() {
        let names = Names::new();
        let args = rl_rules::ability::parse_args(r#"(cells: 2, cellz: 1)"#).unwrap();
        let err = Shove::from_args(&args, &names).expect_err("an unknown field is a typo");
        assert!(err.contains("cellz"), "{err}");
    }

    /// `while_worn` is read when written and false when not, and a held
    /// status says so where it is described, which is the line the bag
    /// shows under the thing; the bonus reaches the turns it names there
    /// too.
    #[test]
    fn while_worn_is_read_when_written_and_said_where_the_effect_is_described() {
        let statuses = rl_rules::Registry::from_defs(vec![rl_rules::StatusDef::new("hidden")]).unwrap();
        let names = Names::new().statuses(&statuses);
        let args = |text: &str| rl_rules::ability::parse_args(text).unwrap();
        let plain = Inflict::from_args(&args(r#"(status: "hidden", turns: 5)"#), &names).unwrap();
        assert!(!plain.while_worn);
        let held = Inflict::from_args(&args(r#"(status: "hidden", turns: 10, while_worn: true)"#), &names).unwrap();
        assert!(held.while_worn);
        let registries = Registries { statuses, ..Default::default() };
        assert_eq!(plain.describe(&registries, EffectBonus::default()), "hidden for 5 turns");
        assert_eq!(held.describe(&registries, EffectBonus { turns: 4, amount: 0 }), "hidden for 14 turns while worn");
    }
}
