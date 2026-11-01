//! The by-range rule: a subject is noticed within a range that its own conduct sets, the same for every observer.

use super::Glimpse;
use serde::{Deserialize, Serialize};

/// The numbers of the by-range rule, one set for the whole game.
///
/// Authored by the game and serde-ready; the engine has no defaults for
/// them, because what is a quiet distance is the game's to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeRules {
    /// The range of a subject in the open, in tiles.
    pub base: i32,
    /// The range is divided by this, rounding down, while the subject is in the dark.
    pub dark_divisor: i32,
    /// The range is divided by this, rounding down, while the subject is in cover.
    pub cover_divisor: i32,
    /// The range is divided by this, rounding up, while the subject stands still.
    pub still_divisor: i32,
    /// The range is multiplied by this on the turn the subject attacks.
    pub attack_multiplier: i32,
    /// The range never falls below this, however the rest stack.
    pub floor: i32,
    /// Chance per turn, in percent, of an unaware observer spotting a subject within range.
    pub chance_pct: u32,
    /// An alert observer holds a subject in view within this many times the range.
    pub hold: i32,
    /// Turns an observer keeps looking after losing sight, before it forgets.
    pub memory: u32,
}

/// What a subject is doing and standing in, as far as its range goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Standing {
    /// In the dark.
    pub dark: bool,
    /// In cover.
    pub cover: bool,
    /// Did not move this turn.
    pub still: bool,
    /// Attacked this turn.
    pub attacked: bool,
}

/// The distance within which a subject in `standing` can be spotted.
///
/// Starts at the base; the dark and cover each divide it down, standing
/// still divides it up, an attack multiplies it, in that order; it never
/// falls below the floor.
pub fn range_of(rules: &RangeRules, standing: Standing) -> i32 {
    let mut range = rules.base;
    if standing.dark {
        range /= rules.dark_divisor;
    }
    if standing.cover {
        range /= rules.cover_divisor;
    }
    if standing.still {
        range = (range + rules.still_divisor - 1) / rules.still_divisor;
    }
    if standing.attacked {
        range *= rules.attack_multiplier;
    }
    range.max(rules.floor)
}

/// Whether an unaware observer spots the subject it has a line to this turn.
///
/// Only within `range`, and then by the chance alone: nearer is no surer,
/// and a sleeper reads the same rule. `roll` is uniform in `0..100`.
pub fn spots(rules: &RangeRules, range: i32, glimpse: Glimpse, roll: u32) -> bool {
    glimpse.distance <= range && roll < rules.chance_pct
}

/// Whether an alert observer keeps hold of a subject it has a line to: in
/// view and within `hold` times the range.
pub fn holds(rules: &RangeRules, range: i32, glimpse: Glimpse) -> bool {
    glimpse.in_view && glimpse.distance <= range * rules.hold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> RangeRules {
        RangeRules { base: 7, dark_divisor: 2, cover_divisor: 2, still_divisor: 2, attack_multiplier: 2, floor: 2, chance_pct: 25, hold: 3, memory: 6 }
    }

    fn standing(dark: bool, cover: bool, still: bool, attacked: bool) -> Standing {
        Standing { dark, cover, still, attacked }
    }

    fn glimpse(distance: i32, in_view: bool, asleep: bool) -> Glimpse {
        Glimpse { distance, in_view, lit: false, asleep }
    }

    #[test]
    fn the_range_is_the_base_in_the_open() {
        assert_eq!(range_of(&rules(), Standing::default()), 7);
    }

    #[test]
    fn each_step_in_its_order_and_rounding() {
        let r = rules();
        assert_eq!(range_of(&r, standing(false, true, false, false)), 3, "cover rounds down");
        assert_eq!(range_of(&r, standing(false, false, true, false)), 4, "still rounds up");
        assert_eq!(range_of(&r, standing(false, true, true, false)), 2, "cover then still");
        assert_eq!(range_of(&r, standing(true, true, false, false)), 2, "dark and cover make 1, floored");
        assert_eq!(range_of(&r, standing(false, false, false, true)), 14, "attacking doubles");
        assert_eq!(range_of(&r, standing(false, true, false, true)), 6, "cover, then attacked");
    }

    #[test]
    fn the_floor_holds_whatever_stacks() {
        assert_eq!(range_of(&rules(), standing(true, true, true, true)), 2);
        assert_eq!(range_of(&RangeRules { base: 1, ..rules() }, Standing::default()), 2);
    }

    #[test]
    fn beyond_the_range_no_roll_spots() {
        assert!(!spots(&rules(), 7, glimpse(8, true, false), 0));
    }

    #[test]
    fn within_the_range_only_a_roll_under_the_chance_spots() {
        let r = rules();
        assert!(spots(&r, 7, glimpse(7, true, false), 24));
        assert!(!spots(&r, 7, glimpse(7, true, false), 25));
        assert!(!spots(&r, 7, glimpse(1, true, false), 25), "adjacent is no surer");
    }

    #[test]
    fn a_sleeper_spots_by_the_same_rule() {
        let r = rules();
        assert!(spots(&r, 7, glimpse(7, true, true), 24));
        assert!(!spots(&r, 7, glimpse(7, true, true), 25));
        assert!(!spots(&r, 7, glimpse(1, true, true), 25));
    }

    #[test]
    fn it_holds_in_view_within_three_times_the_range() {
        let r = rules();
        assert!(holds(&r, 7, glimpse(21, true, false)));
        assert!(!holds(&r, 7, glimpse(22, true, false)));
        assert!(!holds(&r, 7, glimpse(1, false, false)), "out of view is not held");
    }
}
