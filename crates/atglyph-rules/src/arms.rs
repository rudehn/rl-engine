//! What an actor strikes with and what a blow is worth, as plain values.
//!
//! The Bevy layer gathers the weapons, the armor and the distance from the
//! world; the choices live here so they are tested without an `App`, and so
//! the three places that need the melee-or-ranged choice (the resolver, the
//! hit odds and the forecast) cannot drift apart.

use atglyph_core::DiceRoll;

use crate::equip::SlotId;

/// How an attack is delivered: with the blow or with the shot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// Struck with the melee weapon, at a target beside the attacker.
    Melee,
    /// Fired with the ranged weapon, at a target further off.
    Ranged,
}

/// Which weapon an attack on a target is made with, or `None` when it
/// cannot be made at all.
///
/// Beside the target only the blow is offered: an actor with no blow of its
/// own cannot hurt what stands next to it, and falling back to its gun would
/// be a fight the resolver never fights. Further off only the shot is, and
/// only down a clear `line_of_fire`, which the caller works out against the
/// shot's range because that needs the map.
pub fn choose_reach(adjacent: bool, line_of_fire: bool, has_melee: bool, has_ranged: bool) -> Option<Reach> {
    if adjacent { has_melee.then_some(Reach::Melee) } else { (has_ranged && line_of_fire).then_some(Reach::Ranged) }
}

/// The weapon an actor strikes with: the first worn item that has a charge
/// and carries one, in slot order, else the actor's `own`.
///
/// `worn` is each worn item in slot order with the weapon it carries, if
/// any, and whether it has a charge to attack with. An empty one is
/// skipped rather than ending the search, so the item after it is still
/// offered. The first of the pair is the item the weapon comes from, `None`
/// when it is the actor's own.
pub fn first_charged<I: Copy, W: Copy>(worn: impl IntoIterator<Item = (I, Option<W>, bool)>, own: Option<W>) -> Option<(Option<I>, W)> {
    worn.into_iter().find_map(|(item, weapon, charged)| weapon.filter(|_| charged).map(|w| (Some(item), w))).or_else(|| own.map(|w| (None, w)))
}

/// A blow struck harder in both hands: the bigger `roll` it is swung with
/// while the slot `while_free` names is empty, a longsword's 1d10 while
/// the off hand holds nothing.
///
/// It claims nothing: the slot stays free, so anything put in it moves the
/// blow to one hand rather than coming off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Versatile {
    /// The roll struck while the slot is free, as `"NdS+B"`.
    pub roll: DiceRoll,
    /// The slot that must be empty for it.
    pub while_free: SlotId,
}

/// The roll a blow of `dice` strikes for: `versatile`'s while the slot it
/// names is free, as `is_free` answers for the wearer, else `dice`.
///
/// The one place the choice is made, so the fight, the forecast and a
/// screen weighing gear on a wearer as it would be dressed strike the same
/// roll; each passes its own equipment's
/// [`is_free`](crate::Equipment::is_free), real or tried on a copy.
pub fn versatile_roll(dice: DiceRoll, versatile: Option<&Versatile>, is_free: impl Fn(SlotId) -> bool) -> DiceRoll {
    match versatile {
        Some(v) if is_free(v.while_free) => v.roll,
        _ => dice,
    }
}

/// The flat armor a blow meets: the actor's `own`, every `worn` item's and
/// the armor `stat`, summed.
///
/// A sum and not a cap, so a coat's share leaves with the coat and nothing
/// was copied onto the wearer to make it so.
pub fn armor_total(own: i32, worn: impl IntoIterator<Item = i32>, stat: i32) -> i32 {
    own + worn.into_iter().sum::<i32>() + stat
}

/// How much of a heal reaches its target, in percent: a hundred and the
/// mending `stat`, never below nothing.
///
/// Floored because a stat that drove it negative would turn a heal into a
/// wound, and a wound has its own path through mitigation.
pub fn mending_pct(stat: i32) -> i32 {
    (100 + stat).max(0)
}

/// A rolled `amount` scaled to `pct` percent, for a blow on a target that
/// has not noticed its attacker.
///
/// Computed wide so a large roll times a large percent does not overflow,
/// and saturating at the top for the same reason. Truncates toward zero, as
/// the integer arithmetic it replaces did.
pub fn scale_unaware(amount: i32, pct: u32) -> i32 {
    (i64::from(amount) * i64::from(pct) / 100).try_into().unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beside_the_target_only_the_blow_is_offered_and_never_the_gun() {
        assert_eq!(choose_reach(true, true, true, true), Some(Reach::Melee));
        assert_eq!(choose_reach(true, true, false, true), None, "no blow of its own means it cannot hurt what stands next to it");
        assert_eq!(choose_reach(true, false, true, false), Some(Reach::Melee), "a line of fire is no part of a blow");
    }

    #[test]
    fn further_off_only_the_shot_is_offered_and_only_down_a_clear_line() {
        assert_eq!(choose_reach(false, true, true, true), Some(Reach::Ranged));
        assert_eq!(choose_reach(false, false, true, true), None, "a wall in the way is no shot");
        assert_eq!(choose_reach(false, true, true, false), None, "and no gun is no shot");
    }

    #[test]
    fn the_first_worn_weapon_with_a_charge_replaces_the_actors_own() {
        let worn = [('a', None, true), ('b', Some(7), true), ('c', Some(9), true)];
        assert_eq!(first_charged(worn, Some(1)), Some((Some('b'), 7)), "slot order, and the one carrying nothing is passed over");
        assert_eq!(
            first_charged([('a', Some(7), false), ('b', Some(9), true)], Some(1)),
            Some((Some('b'), 9)),
            "an empty one is skipped, not the end of the search"
        );
        assert_eq!(first_charged([('a', Some(7), false)], Some(1)), Some((None, 1)), "and with nothing charged the actor's own is struck with");
        assert_eq!(first_charged([('a', None::<i32>, true)], None), None, "something with nothing to strike with cannot");
    }

    #[test]
    fn a_versatile_blow_rolls_its_bigger_die_only_while_its_slot_is_free() {
        let left = crate::SlotId::from_raw(1);
        let v = Versatile { roll: DiceRoll::new(1, 10), while_free: left };
        assert_eq!(versatile_roll(DiceRoll::new(1, 8), Some(&v), |_| true), DiceRoll::new(1, 10));
        assert_eq!(versatile_roll(DiceRoll::new(1, 8), Some(&v), |_| false), DiceRoll::new(1, 8));
        assert_eq!(versatile_roll(DiceRoll::new(1, 8), None, |_| true), DiceRoll::new(1, 8));
        assert_eq!(versatile_roll(DiceRoll::new(1, 8), Some(&v), |slot| slot != left), DiceRoll::new(1, 8), "the slot it names is the one asked after");
    }

    #[test]
    fn armor_is_the_hide_the_coats_and_the_stat_added_and_a_coat_taken_off_takes_its_share() {
        assert_eq!(armor_total(1, [2], 3), 6);
        assert_eq!(armor_total(1, [], 3), 4);
        assert_eq!(armor_total(0, [2, 5], 0), 7);
    }

    #[test]
    fn mending_is_a_hundred_and_the_stat_and_never_below_nothing() {
        assert_eq!(mending_pct(0), 100);
        assert_eq!(mending_pct(50), 150);
        assert_eq!(mending_pct(-30), 70);
        assert_eq!(mending_pct(-500), 0);
    }

    #[test]
    fn an_unaware_blow_is_scaled_by_its_percent_and_a_hundred_changes_nothing() {
        assert_eq!(scale_unaware(2, 300), 6);
        assert_eq!(scale_unaware(2, 500), 10);
        assert_eq!(scale_unaware(2, 100), 2);
        assert_eq!(scale_unaware(7, 150), 10, "truncated, not rounded");
        assert_eq!(scale_unaware(0, 300), 0);
        assert_eq!(scale_unaware(i32::MAX, 300), i32::MAX, "saturates rather than wrapping");
    }
}
