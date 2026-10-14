//! Authored parts: what a content file writes for something the engine
//! already simulates, in one shape every game can read.
//!
//! A part is a field's type, not a file's schema. A game's own definition
//! names its fields as it likes and types the ones the engine plays with
//! these, so a blow is `(roll: "1d6", kind: "slash")` in any game's file and
//! a game that wants none of them writes its own and spawns by hand. Every
//! name resolves through [`Names`](crate::Names) when the file is read, as
//! the rest of an authored form does, and every field past the essentials
//! is optional, absent meaning the engine's plain behaviour rather than a
//! mechanic switched on.
//!
//! The Bevy layer turns each into the component it describes: a
//! [`Melee`] into a `MeleeAttack`, a [`Ranged`] into a `RangedAttack`, a
//! [`Throw`] into a `Throwable`, a [`Wear`] into a `Wearable`, a
//! [`Charges`] into a `Consumable`, and a [`Level`] into what each level of
//! a thing adds to all of those. The rest of what an item's line writes
//! for the engine is a plain list, its armor, its tags, its effects and
//! triggers, needing no part of its own; [`resistances`] reads the one
//! list that adds up.
//!
//! No part has a default the engine chose. A thing with no level part
//! takes no level, and a level part grows only the amounts it writes, so
//! what a `+2` is in a game is in that game's file, or in the defaults its
//! own loader writes under its lines.

use rl_core::DiceRoll;
use serde::{Deserialize, Serialize};

use crate::ability::Look;
use crate::content::Registry;
use crate::damage::{DamageKind, Resistances};
use crate::equip::{EquipShape, SlotDef, pair_of};
use crate::names::NameRef;
use crate::stats::StatDef;

/// Who else an attack strikes besides the one it was made at.
///
/// Every one it reaches is rolled for on its own, to land and for damage,
/// and the whole is one swing: whatever counts swings, heat or ammunition,
/// counts one, while the weapon's `hit` triggers land on each one struck. It
/// reaches foes only, those the attacker is hostile to, so a swing beside an
/// ally passes over them; the target is struck whoever it is.
///
/// Read from RON as `Sweep` or `Pierce(1)`. Here rather than beside the
/// combat that plays it, because a [`Melee`] part names one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttackShape {
    /// Every foe beside the attacker, in all eight directions: a swing
    /// round the wielder. On a shot it is the same ring, round the shooter
    /// and not where the shot lands; a burst where a shot lands is a `hit`
    /// trigger with an area.
    Sweep,
    /// The target and every foe in the next `n` cells beyond it, on the
    /// line from the attacker through the target: a thrust that runs on
    /// through. A wall or a shut door stops it, as either stops a shot, and
    /// a shot's never runs past the shot's range.
    Pierce(u32),
}

/// A blow, as content writes it: `(roll: "1d6", kind: "slash")`, with an
/// optional `look`, `shape` and `unaware`.
///
/// The same part on a weapon and on a monster, since a blow is one thing
/// whoever strikes it. What one costs is not here: it is the carrier's, a
/// weapon's speed applying to its shot as well, so it is passed beside the
/// part when the attack is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Melee {
    /// The damage roll, as `"NdS+B"`.
    pub roll: DiceRoll,
    /// The damage kind.
    pub kind: NameRef<DamageKind>,
    /// The colour it bursts in on whoever it strikes; absent, it shows
    /// nothing, and the log and the health bar say it.
    #[serde(default)]
    pub look: Option<Look>,
    /// Who else it strikes; absent, the target alone.
    #[serde(default)]
    pub shape: Option<AttackShape>,
    /// What a blow on a target that has not noticed its striker is worth,
    /// in percent of its roll; absent, whatever the game's combat rules say
    /// for every blow.
    #[serde(default)]
    pub unaware: Option<u32>,
}

/// A shot, as content writes it: `(range: 8, roll: "1d6", kind:
/// "pierce")`, with an optional `effective` and `look`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ranged {
    /// The furthest cell it reaches.
    pub range: i32,
    /// The furthest cell with no range penalty, for a hit model that reads
    /// one; absent, a third of `range`.
    #[serde(default)]
    pub effective: Option<i32>,
    /// The damage roll, as `"NdS+B"`.
    pub roll: DiceRoll,
    /// The damage kind.
    pub kind: NameRef<DamageKind>,
    /// What flies from the shooter to the target; absent, a shot lands at
    /// once and unseen.
    #[serde(default)]
    pub look: Option<Look>,
}

/// A throw, as content writes it: `(range: 6)`, with an optional
/// `effective` and the `strike` it deals whoever it hits.
///
/// A thing with no strike hits nobody for damage: a flask is thrown for its
/// `land` trigger, not for the bruise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Throw {
    /// The furthest cell it reaches.
    pub range: i32,
    /// The furthest cell with no range penalty; absent, a third of `range`.
    #[serde(default)]
    pub effective: Option<i32>,
    /// The roll and damage kind it strikes whoever it hits with, as
    /// `("1d6", "pierce")`; absent, nothing.
    #[serde(default)]
    pub strike: Option<(DiceRoll, NameRef<DamageKind>)>,
}

/// What happens to a thing when its last charge is spent.
///
/// Here rather than beside the charges it is about, because a [`Charges`]
/// part names one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WhenEmpty {
    /// It is gone: a draught, a grenade, a ration.
    Destroyed,
    /// It stays, empty, and does nothing it is spent by until it refills: a
    /// wand at zero is still a wand.
    Kept,
}

/// Where a thing is worn, as content writes it: `(slot: "main hand")`,
/// with `either` for a thing that fits either slot of a pair and `also` for
/// the slots a two-hander claims besides.
///
/// Which two slots make a pair is the slot registry's, written on a
/// [`SlotDef`] with [`SlotDef::paired_with`], so `either` reaches for a
/// game's own pairs, hands, fingers, wrists, without the engine naming one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wear {
    /// The slot it goes in, or the first of the pair it fits.
    pub slot: NameRef<SlotDef>,
    /// True for a thing that fits either slot of the pair `slot` is one of:
    /// a one-handed weapon either hand.
    #[serde(default)]
    pub either: bool,
    /// Slots it also claims wherever it went, displacing what holds them.
    #[serde(default)]
    pub also: Vec<NameRef<SlotDef>>,
}

impl Wear {
    /// The shape this is in `slots`, or why it has none: `either` on a slot
    /// of no pair, or `either` beside `also`, which would claim a pair and
    /// a slot the pair might be.
    pub fn shape(&self, slots: &Registry<SlotDef>) -> Result<EquipShape, String> {
        let slot = self.slot.id();
        if !self.either {
            return Ok(self.also.iter().fold(EquipShape::in_slot(slot), |shape, also| shape.and_claims(also.id())));
        }
        if !self.also.is_empty() {
            return Err("either with also".into());
        }
        match pair_of(slots, slot) {
            Some(other) => Ok(EquipShape::in_any([slot, other])),
            None => Err(format!("either, on the {} slot, which is one of no pair", slots.name(slot))),
        }
    }
}

/// A thing's charges, as content writes them: `(charges: 5, when_empty:
/// Kept)`, with an optional `recharge` and `while_worn`.
///
/// One unit for the clock, hundredths of a step, as every cost and clock
/// in the engine is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Charges {
    /// What a fresh unit holds, at level nought.
    pub charges: u16,
    /// What happens at the last.
    pub when_empty: WhenEmpty,
    /// Hundredths of a step a spent charge takes to come back; absent, a
    /// spent charge is gone until something refills it.
    #[serde(default)]
    pub recharge: Option<u32>,
    /// True for a worn thing whose charges come back only while it is worn
    /// and which is emptied each time it is put on, so it cannot be carried
    /// charged and swapped to; absent, it refills in the pack.
    #[serde(default)]
    pub while_worn: bool,
}

/// What one level of a thing adds, as content writes it: `(armor: 1)`,
/// `(damage: 1, stats: [("accuracy", 5)])`, `(charges: 1, amount: 2)`,
/// with the levels it may be at.
///
/// The thing's own numbers are what it is at level nought, and a level adds
/// each amount here once per level, below nought taking it away: linear,
/// and nothing else. Every amount is nought unless written, so a level part
/// says exactly what grows and a reader never has to know a rule the
/// engine applies on its own. A thing with no level part takes no level.
///
/// What a level is worth, how one is found, what one below nought means, a
/// curse or nothing, and whether a game has levels at all are the game's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Level {
    /// The lowest level it may be at; absent, nought, so a thing goes below
    /// plain only where its line says how far.
    #[serde(default)]
    pub min: Option<i32>,
    /// The highest level it may be at; absent, no ceiling.
    #[serde(default)]
    pub max: Option<i32>,
    /// Armor a level adds.
    #[serde(default)]
    pub armor: Option<i32>,
    /// Added a level to every damage roll it makes: its blow, its shot and
    /// its thrown strike alike.
    #[serde(default)]
    pub damage: Option<i32>,
    /// Added a level to what its effects land, every harm and mend roll and
    /// whatever a game's own effects read it as.
    #[serde(default)]
    pub amount: Option<i32>,
    /// Whole turns added a level to every status its effects inflict.
    #[serde(default)]
    pub turns: Option<i32>,
    /// Hundredths of a step a level adds to its pulse's period, below
    /// nought for a clock that quickens.
    #[serde(default)]
    pub pulse: Option<i32>,
    /// Charges a level adds to what a fresh unit holds.
    #[serde(default)]
    pub charges: Option<i32>,
    /// What a level adds to each named stat of its wearer's while it is
    /// worn.
    #[serde(default)]
    pub stats: Vec<(NameRef<StatDef>, i32)>,
}

impl Level {
    /// This part written over `under`: each amount and bound this writes,
    /// and `under`'s where this writes none, and each stat this names in
    /// place of `under`'s for it, the rest of `under`'s kept.
    ///
    /// For a game whose things share a default, a weapon's damage a level
    /// or armor's armor, and whose line writes only what it changes; the
    /// default is the game's, never the engine's.
    pub fn over(&self, under: &Level) -> Level {
        let mut stats: Vec<(NameRef<StatDef>, i32)> =
            under.stats.iter().filter(|(stat, _)| !self.stats.iter().any(|(mine, _)| mine == stat)).copied().collect();
        stats.extend(self.stats.iter().copied());
        Level {
            min: self.min.or(under.min),
            max: self.max.or(under.max),
            armor: self.armor.or(under.armor),
            damage: self.damage.or(under.damage),
            amount: self.amount.or(under.amount),
            turns: self.turns.or(under.turns),
            pulse: self.pulse.or(under.pulse),
            charges: self.charges.or(under.charges),
            stats,
        }
    }

    /// The lowest and the highest level a thing may be at: nought and no
    /// ceiling where neither is written.
    pub fn bounds(&self) -> (i32, i32) {
        (self.min.unwrap_or(0), self.max.unwrap_or(i32::MAX))
    }

    /// `level` held within [`bounds`](Self::bounds), so a level asked for
    /// beyond what the line allows, a save written before a ceiling was
    /// lowered, is the nearest the line allows.
    pub fn clamp(&self, level: i32) -> i32 {
        let (min, max) = self.bounds();
        level.clamp(min, max.max(min))
    }
}

/// The resistances `list` writes, each kind's percents summed: a kind
/// written twice resists by both, as two worn things' resistances add up
/// on a wearer, rather than the second quietly replacing the first.
pub fn resistances(list: &[(NameRef<DamageKind>, i32)]) -> Resistances {
    let mut resists = Resistances::new();
    for (kind, pct) in list {
        resists.add(kind.id(), *pct);
    }
    resists
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Registry;
    use crate::names::Names;

    /// The one entry of a list holding `text`, read with `names` in scope.
    fn one<T: serde::de::DeserializeOwned>(names: &Names<'_>, text: &str) -> Result<T, crate::ContentError> {
        names.load_list::<T>(&format!("[{text}]")).map(|mut all| all.remove(0))
    }

    fn kinds() -> Registry<DamageKind> {
        Registry::from_defs(vec![DamageKind::new("slash"), DamageKind::new("pierce")]).unwrap()
    }

    #[test]
    fn a_blow_with_only_its_roll_and_kind_has_no_shape_look_or_surprise_of_its_own() {
        let kinds = kinds();
        let names = Names::new().damage_kinds(&kinds);
        let blow: Melee = one(&names, r#"(roll: "1d4+1", kind: "slash")"#).unwrap();
        assert_eq!((blow.roll, blow.kind.id()), (DiceRoll { num: 1, sides: 4, bonus: 1 }, kinds.expect("slash")));
        assert_eq!((blow.look, blow.shape, blow.unaware), (None, None, None));
        let thrust: Melee = one(&names, r#"(roll: "1d8", kind: "pierce", shape: Pierce(1), unaware: 500)"#).unwrap();
        assert_eq!((thrust.shape, thrust.unaware), (Some(AttackShape::Pierce(1)), Some(500)));
    }

    #[test]
    fn a_shot_and_a_throw_read_their_reach_and_a_field_nobody_has_is_refused() {
        let kinds = kinds();
        let names = Names::new().damage_kinds(&kinds);
        let shot: Ranged = one(&names, r#"(range: 10, effective: 7, roll: "2d4", kind: "pierce")"#).unwrap();
        assert_eq!((shot.range, shot.effective), (10, Some(7)));
        let throw: Throw = one(&names, r#"(range: 6, strike: ("1d3", "pierce"))"#).unwrap();
        assert_eq!(throw.strike.map(|(roll, kind)| (roll, kind.id())), Some((DiceRoll::new(1, 3), kinds.expect("pierce"))));
        assert!(one::<Ranged>(&names, r#"(range: 10, roll: "2d4", kind: "pierce", heat: 3)"#).is_err(), "an unknown field");
    }

    fn slots() -> Registry<SlotDef> {
        Registry::from_defs(vec![SlotDef::new("main hand").paired_with("off hand"), SlotDef::new("off hand"), SlotDef::new("head")]).unwrap()
    }

    #[test]
    fn either_reaches_for_the_registrys_pair_and_refuses_a_slot_of_none_or_a_claim_beside_it() {
        let slots = slots();
        let names = Names::new().slots(&slots);
        let (main, off) = (slots.expect("main hand"), slots.expect("off hand"));
        let shape = |text: &str| one::<Wear>(&names, text).unwrap().shape(&slots);
        assert_eq!(shape(r#"(slot: "off hand", either: true)"#), Ok(EquipShape::in_any([off, main])), "from either slot of the pair");
        assert_eq!(shape(r#"(slot: "main hand", also: ["off hand"])"#), Ok(EquipShape::in_slot(main).and_claims(off)));
        assert_eq!(shape(r#"(slot: "head", either: true)"#), Err("either, on the head slot, which is one of no pair".to_string()));
        assert_eq!(shape(r#"(slot: "main hand", either: true, also: ["head"])"#), Err("either with also".to_string()));
    }

    #[test]
    fn a_level_written_over_a_default_keeps_what_it_does_not_write_and_holds_within_its_bounds() {
        let stats = Registry::from_defs(vec![StatDef::new("accuracy", 0), StatDef::new("stealth", 0)]).unwrap();
        let names = Names::new().stats(&stats);
        let default: Level = one(&names, r#"(min: -3, damage: 1, stats: [("accuracy", 5)])"#).unwrap();
        let line: Level = one(&names, r#"(damage: 2, stats: [("accuracy", 0), ("stealth", 10)])"#).unwrap();
        let level = line.over(&default);
        assert_eq!((level.min, level.max, level.damage, level.armor), (Some(-3), None, Some(2), None));
        let stat = |name: &str| stats.expect(name).into();
        assert_eq!(level.stats, [(stat("accuracy"), 0), (stat("stealth"), 10)], "the line's own for a stat it names, the default's kept for the rest");
        assert_eq!((level.clamp(-9), level.clamp(40)), (-3, 40), "no ceiling written");
        assert_eq!(Level::default().clamp(-2), 0, "no level below nought unless the line says how far");
    }

    #[test]
    fn a_kind_written_twice_resists_by_both() {
        let kinds = kinds();
        let names = Names::new().damage_kinds(&kinds);
        let list: Vec<(NameRef<DamageKind>, i32)> = names.load_list(r#"[("slash", 10), ("slash", 15), ("pierce", 5)]"#).unwrap();
        let resists = resistances(&list);
        assert_eq!((resists.get(kinds.expect("slash")), resists.get(kinds.expect("pierce"))), (25, 5));
    }
}
