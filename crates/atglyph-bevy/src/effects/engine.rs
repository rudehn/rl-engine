//! The effects the engine ships: what an effect list can ask of the
//! subsystems the engine owns.
//!
//! `Harm`, `Mend` and `Inflict` grow with their carrier's `EffectBonus`;
//! the others ignore it.
//!
//! [`Harm`] asks combat's damage pipeline and [`Mend`] its healing, [`Inflict`] and
//! [`Cleanse`] ask statuses, and [`Shove`], [`Pull`] and [`Teleport`] move
//! an actor through [`EffectWorld`]; [`EngineApp::add_engine_effects`](crate::app::EngineApp::add_engine_effects) registers those
//! seven. [`Ignite`] asks fire, [`Emit`] asks gas and [`Noise`] asks
//! hearing, and each is registered by the plugin that answers it, so a
//! content file naming one works exactly when the game has that subsystem.
//! Here, rather than each in the module that owns its mechanic, so the
//! dependency runs one way: effects are built on combat, statuses, fire,
//! gas and noise, and none of those has to know an effect list exists.

use atglyph_core::{DiceRoll, Point};
use atglyph_rules::ability::{RawValue, read_args};
use atglyph_rules::damage::DamageKindId;
use atglyph_rules::gas::GasId;
use atglyph_rules::{Hit, Names, StatusId};
use bevy::prelude::*;

use atglyph_rules::ai::{ActorView, Impact, Prospect};

use super::{Effect, EffectBonus, EffectWorld, FromArgs, Landing, PredictCtx};
use crate::app::EngineApp;
use crate::combat::{DamageEvent, Heal};
use crate::registries::Registries;
use crate::status::{ApplyStatus, RemoveStatus};

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
    pub(crate) fn roll_with(&self, bonus: EffectBonus) -> DiceRoll {
        DiceRoll { bonus: self.roll.bonus + bonus.amount, ..self.roll }
    }
}

impl Effect for Harm {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let roll = self.roll_with(landing.bonus);
        // Arriving by what landed it, so a bolt from a rod is not told as
        // a blow struck by its user.
        let reach = landing.reach();
        for target in &landing.targets {
            let amount = roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent::arriving(*target, Hit::by(landing.user, self.kind, amount), reach));
        }
    }

    fn describe(&self, registries: &Registries, bonus: EffectBonus) -> String {
        format!("{} {}", self.roll_with(bonus), registries.damage_kinds.name(self.kind))
    }

    /// The average of what each roll comes to through the game's own
    /// [`DamageStages`](crate::combat::DamageStages), against the target's
    /// armor and resistances: damage, a heal where it absorbs the kind, or
    /// nothing where it is immune.
    fn predict(&self, on: &ActorView<Entity>, user: &ActorView<Entity>, ctx: &PredictCtx<'_>) -> Prospect {
        let fallback = crate::combat::DamageStages::default();
        let stages = ctx.world.get_resource::<crate::combat::DamageStages>().unwrap_or(&fallback);
        let stages: Vec<&dyn atglyph_rules::DamageStage<Entity>> = stages.0.iter().map(|s| s.as_ref() as &dyn atglyph_rules::DamageStage<Entity>).collect();
        let defender = atglyph_rules::damage::Defender { armor: on.armor, blocked: false };
        let lands = |amount: i32| {
            let hit = Hit::by(user.id, self.kind, amount.max(0));
            atglyph_rules::resolve(&hit, &defender, &on.resists, &ctx.registries.damage_kinds, &stages)
        };
        match expected_hundredths(self.roll_with(ctx.bonus), lands) {
            n if n > 0 => Prospect::on(Impact::Damage(n)),
            n if n < 0 => Prospect::on(Impact::Heal(-n)),
            _ => Prospect::on(Impact::Nothing),
        }
    }
}

/// The most dice of the most sides a roll is weighed face by face for; a
/// larger one is weighed at its average alone.
const WEIGHED_FACES: u32 = 2048;

/// What `roll` comes to through `lands` on average, in hundredths: every
/// sum it can roll weighed by how many ways it rolls, so a floor or a
/// threshold in `lands` is weighed as it bites; or, for a roll of more
/// faces than [`WEIGHED_FACES`], `lands` of its average.
///
/// Integers throughout, so two runs weigh alike.
fn expected_hundredths(roll: atglyph_core::DiceRoll, lands: impl Fn(i32) -> i32) -> i32 {
    let faces = roll.num.saturating_mul(roll.sides);
    let ways_total = u128::from(roll.sides).checked_pow(roll.num);
    // Weighed only while every weight times the most a roll comes to stays
    // well inside an i128; past that, at its average.
    let (Some(total), true) = (ways_total.filter(|t| *t <= u128::from(u64::MAX)), faces <= WEIGHED_FACES && roll.sides > 0) else {
        let average = (i64::from(roll.num) * i64::from(roll.sides + 1) / 2 + i64::from(roll.bonus)) as i32;
        return lands(average).saturating_mul(100);
    };
    // Ways to roll each sum of the dice, from `num` up.
    let mut ways: Vec<u128> = vec![1];
    for _ in 0..roll.num {
        let mut next = vec![0u128; ways.len() + roll.sides as usize];
        for (sum, w) in ways.iter().enumerate() {
            for face in 1..=roll.sides as usize {
                next[sum + face] += w;
            }
        }
        ways = next;
    }
    let weighed: i128 = ways.iter().enumerate().filter(|(_, w)| **w > 0).map(|(sum, w)| *w as i128 * i128::from(lands(sum as i32 + roll.bonus))).sum();
    (weighed * 100 / total as i128).clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
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
/// A [`Heal`], not a hit: no kind, so no armor, resistance or stage is
/// asked about it. What scales it is each target's own mending,
/// [`CombatRules::mending_stat`](crate::combat::CombatRules::mending_stat),
/// which is where a game says a thing cannot be patched up.
#[derive(Debug, Clone, Copy)]
pub struct Mend {
    /// How much, rolled per target, before any bonus.
    pub roll: DiceRoll,
}

impl Mend {
    /// The roll with `bonus`'s amount added.
    pub(crate) fn roll_with(&self, bonus: EffectBonus) -> DiceRoll {
        DiceRoll { bonus: self.roll.bonus + bonus.amount, ..self.roll }
    }
}

impl Effect for Mend {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let roll = self.roll_with(landing.bonus);
        for target in &landing.targets {
            let amount = roll.roll_at_least(&mut **world.rng, 0);
            world.heal.write(Heal::new(*target, amount).by(landing.user));
        }
    }

    fn describe(&self, _: &Registries, bonus: EffectBonus) -> String {
        format!("mends {}", self.roll_with(bonus))
    }

    fn mends(&self) -> bool {
        true
    }

    /// Only with someone hurt to mend: a mend on the whole restores nothing
    /// and spends the turn.
    fn would_act(&self, landing: &Landing, world: &World) -> bool {
        landing.targets.iter().any(|t| world.get::<crate::combat::Health>(*t).is_some_and(|h| h.current < h.max))
    }

    /// What it would give back on average, no more than the target is
    /// missing: nothing to the unhurt, or to what has no health to mend.
    fn predict(&self, on: &ActorView<Entity>, _: &ActorView<Entity>, ctx: &PredictCtx<'_>) -> Prospect {
        let missing = on.health.map_or(0, |h| (h.max - h.current).max(0));
        match expected_hundredths(self.roll_with(ctx.bonus), |amount| amount.clamp(0, missing)) {
            0 => Prospect::on(Impact::Nothing),
            n => Prospect::on(Impact::Heal(n)),
        }
    }
}

impl FromArgs for Mend {
    const KIND: &'static str = "Mend";

    fn from_args(args: &RawValue, _names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            roll: String,
        }
        // Said by name rather than left to "unknown field": every content
        // file written before mending left the damage table has one.
        let a: Args = read_args(args)
            .map_err(|e| if e.contains("kind") { format!("{e}; a mend has no kind, since healing is not damage: write `(roll: ..)`") } else { e })?;
        Ok(Self { roll: a.roll.parse().map_err(|e| format!("{e}"))? })
    }
}

/// How many whole turns an inflicted status lasts: one number, or a range
/// rolled each time it lands, both ends included.
///
/// Read from a file as `turns: 2` or `turns: (1, 2)`, so a paralysis that
/// holds for a turn or two is one line of content rather than an effect of
/// a game's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnSpan {
    /// Always this many.
    Fixed(u32),
    /// From the first to the second, both included.
    Between(u32, u32),
}

impl TurnSpan {
    /// The turns it comes to this time: the number, or one drawn from the
    /// range.
    pub fn roll(&self, rng: &mut impl rand::Rng) -> u32 {
        match *self {
            TurnSpan::Fixed(n) => n,
            // Either way round: one built in code backwards is the same range.
            TurnSpan::Between(a, b) => rng.random_range(a.min(b)..=a.max(b)),
        }
    }

    /// The same, `turns` longer at both ends.
    fn plus(self, turns: u32) -> Self {
        match self {
            TurnSpan::Fixed(n) => TurnSpan::Fixed(n + turns),
            TurnSpan::Between(fewest, most) => TurnSpan::Between(fewest + turns, most + turns),
        }
    }
}

impl std::fmt::Display for TurnSpan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TurnSpan::Fixed(n) => write!(f, "{n}"),
            TurnSpan::Between(fewest, most) => write!(f, "{fewest} to {most}"),
        }
    }
}

/// Put a status on everyone under the footprint.
///
/// Named for what it does rather than for the message it writes, because
/// [`ApplyStatus`] is already the request and an effect is not a request.
#[derive(Debug, Clone, Copy)]
pub struct Inflict {
    /// Which status.
    pub status: StatusId,
    /// For how many whole turns, before any bonus: a number, or a range
    /// rolled for each target as it lands.
    pub turns: TurnSpan,
    /// Whether the status lasts only while the thing that landed it is
    /// worn. Read only when a trigger landed it, since only a thing can be
    /// worn: the [`ApplyStatus`] is then held by the thing, held for every
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
    /// The turns with `bonus`'s turns added, at both ends of a range.
    pub(crate) fn turns_with(&self, bonus: EffectBonus) -> TurnSpan {
        self.turns.plus(bonus.turns)
    }
}

impl Effect for Inflict {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let held_by = match landing.source {
            super::Source::Trigger { on, .. } if self.while_worn => Some(on),
            _ => None,
        };
        let span = self.turns_with(landing.bonus);
        for target in &landing.targets {
            let turns = span.roll(&mut **world.rng);
            world.afflict.write(ApplyStatus { target: *target, status: self.status, turns, by: Some(landing.user), held_by });
        }
    }

    fn describe(&self, registries: &Registries, bonus: EffectBonus) -> String {
        let held = if self.while_worn { " while worn" } else { "" };
        format!("{} for {} turns{held}", registries.statuses.name(self.status), self.turns_with(bonus))
    }

    /// The status, a boon or a bane by its definition; nothing on one that
    /// holds it already or can never be given it.
    fn predict(&self, on: &ActorView<Entity>, _: &ActorView<Entity>, ctx: &PredictCtx<'_>) -> Prospect {
        if on.statuses.contains(&self.status) || on.immune_to.contains(&self.status) {
            return Prospect::on(Impact::Nothing);
        }
        Prospect::on(Impact::Status { id: self.status, boon: ctx.registries.statuses.get(self.status).boon })
    }
}

impl FromArgs for Inflict {
    const KIND: &'static str = "Inflict";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            status: String,
            turns: Span,
            #[serde(default)]
            while_worn: bool,
        }
        /// One number, or a range of two.
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Span {
            One(u32),
            Range((u32, u32)),
        }
        let a: Args = read_args(args)?;
        let turns = match a.turns {
            Span::One(n) => TurnSpan::Fixed(n),
            Span::Range((fewest, most)) if fewest > most => {
                return Err(format!("Inflict: turns ({fewest}, {most}) is no range; write the fewest first"));
            }
            Span::Range((fewest, most)) => TurnSpan::Between(fewest, most),
        };
        Ok(Self { status: names.status(&a.status)?, turns, while_worn: a.while_worn })
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
            world.cure.write(RemoveStatus { target: *target, status: self.status });
        }
    }

    /// The status taken off, on one that holds it; nothing on one that
    /// does not.
    fn predict(&self, on: &ActorView<Entity>, _: &ActorView<Entity>, ctx: &PredictCtx<'_>) -> Prospect {
        match on.statuses.contains(&self.status) {
            true => Prospect::on(Impact::Cleanse { id: self.status, boon: ctx.registries.statuses.get(self.status).boon }),
            false => Prospect::on(Impact::Nothing),
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
pub(crate) struct Teleport;

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
    /// How far it carries over open ground, in whole steps; at least one,
    /// since a content file asking for less is refused at load.
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
        // Refused rather than loaded, as a pulse's `every` of 0 is: a noise
        // that carries no step is heard by nobody, and a grenade written
        // that way would land in a silence nothing explains.
        if a.loudness < 1 {
            return Err(format!("a noise's `loudness` of {} carries no step; it is at least 1", a.loudness));
        }
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

/// The body of [`EngineApp::add_engine_effects`](crate::app::EngineApp::add_engine_effects).
pub(crate) fn add_engine_effects(app: &mut App) -> &mut App {
    app.add_effect::<Harm>()
        .add_effect::<Mend>()
        .add_effect::<Inflict>()
        .add_effect::<Cleanse>()
        .add_effect::<Shove>()
        .add_effect::<Pull>()
        .add_effect::<Teleport>()
}

#[cfg(test)]
mod tests {
    use super::*;

    use atglyph_rules::ai::{ActorView, Impact, Prospect, Vitals};

    /// A world holding registries of `fire`, which armor does not stop,
    /// and `blow`, which it does, and of `slowed`, a bane, and `hasted`, a
    /// boon; with the engine's own damage stages, armor and then resistance.
    fn predicting() -> World {
        let mut world = World::new();
        let mut blow = atglyph_rules::DamageKind::new("blow");
        blow.armored = true;
        let mut fire = atglyph_rules::DamageKind::new("fire");
        fire.armored = false;
        let statuses = vec![atglyph_rules::StatusDef::new("slowed"), atglyph_rules::StatusDef::new("hasted").boon()];
        world.insert_resource(Registries {
            damage_kinds: atglyph_rules::Registry::from_defs(vec![fire, blow]).unwrap(),
            statuses: atglyph_rules::Registry::from_defs(statuses).unwrap(),
            ..Default::default()
        });
        let stages: Vec<Box<dyn atglyph_rules::DamageStage<Entity> + Send + Sync>> =
            vec![Box::new(atglyph_rules::damage::SubtractArmor), Box::new(atglyph_rules::damage::ApplyResistance)];
        world.insert_resource(crate::combat::DamageStages(stages));
        world
    }

    fn predict(effect: &dyn Effect, world: &World, on: &ActorView<Entity>) -> Prospect {
        let user = ActorView::at(Entity::from_bits(1), Point::ZERO);
        effect.predict(on, &user, &super::super::PredictCtx::new(world, EffectBonus::default()))
    }

    fn someone(hp: i32) -> ActorView<Entity> {
        ActorView::of(Entity::from_bits(2), Point::new(3, 0), Some(Vitals { current: hp, max: 20 }), None)
    }

    /// A harm predicts its average through the game's own stages, roll by
    /// roll as the stages would take each: about half on one that resists
    /// half, a heal on one that absorbs, nothing on one immune, and armor
    /// taking its share of every roll, never below nothing, on a kind armor
    /// stops.
    #[test]
    fn harm_predicts_through_resistance_and_absorbing_is_a_heal() {
        let world = predicting();
        let (fire, blow) = (DamageKindId::from_raw(0), DamageKindId::from_raw(1));
        let fireball = Harm { kind: fire, roll: DiceRoll::new(2, 6) };
        let resisting = |pct: i32| {
            let mut on = someone(20);
            on.resists.set(fire, pct);
            on
        };
        assert_eq!(predict(&fireball, &world, &someone(20)), Prospect::on(Impact::Damage(700)), "seven on average");
        // Each roll keeps the half the stage leaves it, rounded up.
        assert_eq!(predict(&fireball, &world, &resisting(50)), Prospect::on(Impact::Damage(375)));
        assert_eq!(predict(&fireball, &world, &resisting(150)), Prospect::on(Impact::Heal(325)), "absorbing half again: a heal");
        assert_eq!(predict(&fireball, &world, &resisting(100)), Prospect::on(Impact::Nothing), "immune");
        let mut armored = someone(20);
        armored.armor = 2;
        assert_eq!(predict(&fireball, &world, &armored), Prospect::on(Impact::Damage(700)), "armor does not stop fire");
        // 1d4 against two armor: 0, 0, 1 and 2, three quarters on average.
        assert_eq!(predict(&Harm { kind: blow, roll: DiceRoll::new(1, 4) }, &world, &armored), Prospect::on(Impact::Damage(75)));
    }

    /// A roll with more ways to fall than can be weighed, 24d32, is
    /// predicted at its average rather than overflowing; and a range built
    /// backwards in code rolls within it all the same.
    #[test]
    fn a_roll_too_big_to_weigh_is_predicted_at_its_average_and_a_backward_range_still_rolls() {
        let world = predicting();
        let huge = Harm { kind: DamageKindId::from_raw(0), roll: DiceRoll::new(24, 32) };
        assert_eq!(predict(&huge, &world, &someone(1000)), Prospect::on(Impact::Damage(39600)), "24 times 16.5");
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        assert!((0..50).map(|_| TurnSpan::Between(2, 1).roll(&mut rng)).all(|t| (1..=2).contains(&t)));
    }

    /// A mend predicts what it would give back, no more than is missing,
    /// and nothing to the unhurt or to what has no health.
    #[test]
    fn mend_predicts_what_it_gives_back_and_nothing_at_full_health() {
        let world = predicting();
        let mend = Mend { roll: DiceRoll::new(2, 6) };
        assert_eq!(predict(&mend, &world, &someone(20)), Prospect::on(Impact::Nothing));
        // Three missing: all three but on the one roll in thirty-six of two.
        assert_eq!(predict(&mend, &world, &someone(17)), Prospect::on(Impact::Heal(297)), "no more than the three missing");
        assert_eq!(predict(&mend, &world, &someone(5)), Prospect::on(Impact::Heal(700)));
        assert_eq!(predict(&mend, &world, &ActorView::at(Entity::from_bits(2), Point::ZERO)), Prospect::on(Impact::Nothing));
    }

    /// A status predicts itself, a boon or a bane by its definition, and
    /// nothing on one that holds it already or can never be given it; a
    /// cleanse predicts itself only on a holder.
    #[test]
    fn inflict_on_a_holder_or_an_immune_target_predicts_nothing() {
        let world = predicting();
        let (slowed, hasted) = (StatusId::from_raw(0), StatusId::from_raw(1));
        let slow = Inflict { status: slowed, turns: TurnSpan::Fixed(3), while_worn: false };
        let haste = Inflict { status: hasted, ..slow };
        assert_eq!(predict(&slow, &world, &someone(20)), Prospect::on(Impact::Status { id: slowed, boon: false }));
        assert_eq!(predict(&haste, &world, &someone(20)), Prospect::on(Impact::Status { id: hasted, boon: true }));
        let holder = ActorView { statuses: vec![slowed], ..someone(20) };
        let immune = ActorView { immune_to: vec![slowed], ..someone(20) };
        assert_eq!(predict(&slow, &world, &holder), Prospect::on(Impact::Nothing), "already slowed");
        assert_eq!(predict(&slow, &world, &immune), Prospect::on(Impact::Nothing), "never to be slowed");
        let cure = Cleanse { status: slowed };
        assert_eq!(predict(&cure, &world, &holder), Prospect::on(Impact::Cleanse { id: slowed, boon: false }));
        assert_eq!(predict(&cure, &world, &someone(20)), Prospect::on(Impact::Nothing), "nothing to take off");
        assert_eq!(predict(&Shove { cells: 2 }, &world, &someone(20)), Prospect::UNKNOWN, "and what says nothing of itself is unknown");
    }

    /// A bonus adds its amount to a harm and a mend roll and its turns to
    /// an inflicted status, and a default bonus, what a plain carrier
    /// reads, adds nothing at all.
    #[test]
    fn a_bonus_adds_its_amount_to_harm_and_mend_and_its_turns_to_a_status_and_a_default_adds_nothing() {
        let kind = DamageKindId::from_raw(0);
        let bonus = EffectBonus { turns: 2, amount: 3 };
        let mend = Mend { roll: DiceRoll::flat(1) };
        assert_eq!((mend.roll_with(EffectBonus::default()), mend.roll_with(bonus)), (DiceRoll::flat(1), DiceRoll::flat(4)));
        let harm = Harm { kind, roll: DiceRoll::new(2, 6) };
        assert_eq!(harm.roll_with(bonus), DiceRoll { num: 2, sides: 6, bonus: 3 });
        let hiding = Inflict { status: StatusId::from_raw(0), turns: TurnSpan::Fixed(5), while_worn: false };
        assert_eq!((hiding.turns_with(EffectBonus::default()), hiding.turns_with(bonus)), (TurnSpan::Fixed(5), TurnSpan::Fixed(7)));
    }

    /// A `per_level` left in a `Harm`, a `Mend` or an `Inflict`'s
    /// arguments, which the engine no longer reads, is refused at load,
    /// naming the field, rather than silently ignored.
    #[test]
    fn per_level_in_harm_mend_or_inflict_args_is_refused_naming_it() {
        let kinds = atglyph_rules::Registry::from_defs(vec![atglyph_rules::DamageKind::new("kinetic")]).unwrap();
        let statuses = atglyph_rules::Registry::from_defs(vec![atglyph_rules::StatusDef::new("hidden")]).unwrap();
        let names = Names::new().damage_kinds(&kinds).statuses(&statuses);
        let args = |text: &str| atglyph_rules::ability::parse_args(text).unwrap();
        let harm = Harm::from_args(&args(r#"(kind: "kinetic", roll: "4", per_level: 1)"#), &names).expect_err("per_level is gone from Harm");
        assert!(harm.contains("per_level"), "{harm}");
        let mend = Mend::from_args(&args(r#"(roll: "4", per_level: 1)"#), &names).expect_err("per_level is gone from Mend");
        assert!(mend.contains("per_level"), "{mend}");
        let inflict = Inflict::from_args(&args(r#"(status: "hidden", turns: 5, per_level: 1)"#), &names).expect_err("per_level is gone from Inflict");
        assert!(inflict.contains("per_level"), "{inflict}");
    }

    /// A mend is written with a roll and nothing else, and one still
    /// written with the kind healing used to be dealt as is refused with
    /// the reason, since every content file older than this has one.
    #[test]
    fn a_mend_has_no_kind_and_one_written_with_a_kind_is_told_why() {
        let names = Names::new();
        let args = |text: &str| atglyph_rules::ability::parse_args(text).unwrap();
        let mend = Mend::from_args(&args(r#"(roll: "2d4")"#), &names).expect("a roll is all it takes");
        assert_eq!(mend.roll, DiceRoll::new(2, 4));
        assert_eq!(mend.describe(&Registries::default(), EffectBonus { turns: 0, amount: 1 }), "mends 2d4+1");
        let old = Mend::from_args(&args(r#"(kind: "care", roll: "2d4")"#), &names).expect_err("a kind is refused");
        assert!(old.contains("kind") && old.contains("healing is not damage"), "{old}");
    }

    /// A typo'd argument on another engine effect is refused the same way,
    /// naming it, rather than parsed away as a field nobody asked for.
    #[test]
    fn a_typod_argument_on_another_engine_effect_is_refused_naming_it() {
        let names = Names::new();
        let args = atglyph_rules::ability::parse_args(r#"(cells: 2, cellz: 1)"#).unwrap();
        let err = Shove::from_args(&args, &names).expect_err("an unknown field is a typo");
        assert!(err.contains("cellz"), "{err}");
    }

    /// A status's turns are one number or a range, `turns: 2` or `turns:
    /// (1, 2)`; a range written backwards is refused naming the effect, and
    /// a range is said as one where the effect is described.
    #[test]
    fn turns_are_one_number_or_a_range_and_a_backward_range_is_refused() {
        let statuses = atglyph_rules::Registry::from_defs(vec![atglyph_rules::StatusDef::new("paralysed")]).unwrap();
        let names = Names::new().statuses(&statuses);
        let args = |text: &str| atglyph_rules::ability::parse_args(text).unwrap();
        let one = Inflict::from_args(&args(r#"(status: "paralysed", turns: 2)"#), &names).unwrap();
        assert_eq!(one.turns, TurnSpan::Fixed(2));
        let range = Inflict::from_args(&args(r#"(status: "paralysed", turns: (1, 2))"#), &names).unwrap();
        assert_eq!(range.turns, TurnSpan::Between(1, 2));
        let backward = Inflict::from_args(&args(r#"(status: "paralysed", turns: (2, 1))"#), &names).expect_err("2 to 1 is no range");
        assert!(backward.contains("Inflict") && backward.contains("(2, 1)"), "{backward}");
        let registries = Registries { statuses, ..Default::default() };
        assert_eq!(range.describe(&registries, EffectBonus::default()), "paralysed for 1 to 2 turns");
        assert_eq!(range.describe(&registries, EffectBonus { turns: 1, amount: 0 }), "paralysed for 2 to 3 turns", "a bonus lengthens both ends");
    }

    /// A range rolls a number of turns within it, both ends included, and
    /// both ends turn up; one number is always itself.
    #[test]
    fn a_range_of_turns_rolls_within_it_and_both_ends_turn_up() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let rolled: Vec<u32> = (0..200).map(|_| TurnSpan::Between(1, 2).roll(&mut rng)).collect();
        assert!(rolled.iter().all(|t| (1..=2).contains(t)), "{rolled:?}");
        assert!(rolled.contains(&1) && rolled.contains(&2), "both ends");
        assert!((0..20).all(|_| TurnSpan::Fixed(4).roll(&mut rng) == 4));
    }

    /// `while_worn` is read when written and false when not, and a held
    /// status says so where it is described, which is the line the bag
    /// shows under the thing; the bonus reaches the turns it names there
    /// too.
    #[test]
    fn while_worn_is_read_when_written_and_said_where_the_effect_is_described() {
        let statuses = atglyph_rules::Registry::from_defs(vec![atglyph_rules::StatusDef::new("hidden")]).unwrap();
        let names = Names::new().statuses(&statuses);
        let args = |text: &str| atglyph_rules::ability::parse_args(text).unwrap();
        let plain = Inflict::from_args(&args(r#"(status: "hidden", turns: 5)"#), &names).unwrap();
        assert!(!plain.while_worn);
        let held = Inflict::from_args(&args(r#"(status: "hidden", turns: 10, while_worn: true)"#), &names).unwrap();
        assert!(held.while_worn);
        let registries = Registries { statuses, ..Default::default() };
        assert_eq!(plain.describe(&registries, EffectBonus::default()), "hidden for 5 turns");
        assert_eq!(held.describe(&registries, EffectBonus { turns: 4, amount: 0 }), "hidden for 14 turns while worn");
    }
}
