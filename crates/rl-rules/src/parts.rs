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
//! [`Throw`] into a `Throwable`.

use rl_core::DiceRoll;
use serde::{Deserialize, Serialize};

use crate::ability::Look;
use crate::damage::DamageKind;
use crate::names::NameRef;

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
}
