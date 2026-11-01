//! The per-observer rule: each observer has its own chance, and the subject its own stealth.
//!
//! Two knobs rather than a radius. A radius alone is a hard line the player
//! learns to stand behind; a chance alone is a lottery with no readable
//! edge. A certain distance inside which nothing helps, and a chance per
//! turn beyond it, is both readable and tense.
//!
//! Light is one number, [`NoticeStats::lit_bonus`], and it defaults to zero,
//! so a game with no lighting is unaffected without saying so.

use super::Glimpse;
use serde::{Deserialize, Serialize};

/// How well an observer notices what is trying not to be seen.
///
/// Authored per kind of observer, and serde-ready so a game writes it
/// straight into a bestiary file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoticeStats {
    /// Always noticed at or within this many tiles, however quiet.
    pub certain: i32,
    /// Chance per turn of noticing beyond that, in percent.
    pub chance_pct: u32,
    /// Added to `certain` while the subject stands in light. Zero is a game,
    /// or a creature, for which light has nothing to do with being seen.
    #[serde(default)]
    pub lit_bonus: i32,
    /// Turns it keeps looking after losing sight, before it forgets. What
    /// turns breaking contact into an escape rather than a coin flip.
    #[serde(default = "default_memory")]
    pub memory: u32,
}

fn default_memory() -> u32 {
    6
}

impl Default for NoticeStats {
    /// Notices anything adjacent, a quarter of the time beyond, and
    /// remembers for six turns: an unremarkable sentry.
    fn default() -> Self {
        Self { certain: 1, chance_pct: 25, lit_bonus: 0, memory: default_memory() }
    }
}

/// How hard a subject is to notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct StealthStats {
    /// Taken off an observer's certain radius. The radius never falls below
    /// one, so no stack of gear makes somebody standing next to you
    /// invisible.
    #[serde(default)]
    pub quiet: i32,
    /// Taken off an observer's chance, in percentage points.
    #[serde(default)]
    pub subtlety: u32,
}

/// The distance within which `notice` is certain to spot `stealth`.
pub fn certain_radius(notice: &NoticeStats, stealth: &StealthStats, lit: bool) -> i32 {
    let bonus = if lit { notice.lit_bonus } else { 0 };
    (notice.certain + bonus - stealth.quiet).max(1)
}

/// The chance per turn, in percent, of spotting `stealth` beyond the
/// certain radius.
pub fn notice_chance(notice: &NoticeStats, stealth: &StealthStats) -> u32 {
    notice.chance_pct.saturating_sub(stealth.subtlety).min(100)
}

/// Whether `notice` spots `stealth` `distance` tiles away this turn.
///
/// `roll` is uniform in `0..100`, drawn by the caller. The caller has
/// already decided the subject is perceivable at all - in line, lit or
/// within dark sight, within perception - so this answers only whether
/// the observer paid attention.
pub fn notices(distance: i32, notice: &NoticeStats, stealth: &StealthStats, lit: bool, roll: u32) -> bool {
    distance <= certain_radius(notice, stealth, lit) || roll < notice_chance(notice, stealth)
}

/// Whether an unaware observer spots the subject it has a line to this turn.
///
/// A sleeper reads the chance alone, with no certain radius, however close;
/// one awake reads [`notices`]. `roll` is uniform in `0..100`.
pub fn spots(notice: &NoticeStats, stealth: &StealthStats, glimpse: Glimpse, roll: u32) -> bool {
    if glimpse.asleep { roll < notice_chance(notice, stealth) } else { notices(glimpse.distance, notice, stealth, glimpse.lit, roll) }
}

/// Whether an alert observer keeps hold of a subject it has a line to: always,
/// whatever it would roll.
pub fn holds(glimpse: Glimpse) -> bool {
    glimpse.in_view
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn sentry() -> NoticeStats {
        NoticeStats { certain: 3, chance_pct: 20, lit_bonus: 4, memory: 5 }
    }

    #[test]
    fn inside_the_certain_radius_every_roll_notices_and_beyond_it_only_a_low_roll_does() {
        let notice = sentry();
        let plain = StealthStats::default();
        for roll in 0..100 {
            assert!(notices(3, &notice, &plain, false, roll), "at the radius, roll {roll}");
        }
        assert!(notices(4, &notice, &plain, false, 19), "just under the chance");
        assert!(!notices(4, &notice, &plain, false, 20), "at the chance is a miss");
    }

    #[test]
    fn light_widens_the_certain_radius_by_exactly_its_bonus() {
        let notice = sentry();
        let plain = StealthStats::default();
        assert_eq!(certain_radius(&notice, &plain, false), 3);
        assert_eq!(certain_radius(&notice, &plain, true), 7);
        assert!(!notices(6, &notice, &plain, false, 99), "six tiles in the dark, a high roll misses");
        assert!(notices(6, &notice, &plain, true, 99), "six tiles in the light is certain");
    }

    #[test]
    fn quiet_narrows_the_radius_but_never_below_one() {
        let notice = sentry();
        let quiet = StealthStats { quiet: 2, subtlety: 0 };
        assert_eq!(certain_radius(&notice, &quiet, false), 1);
        let silent = StealthStats { quiet: 50, subtlety: 100 };
        assert_eq!(certain_radius(&notice, &silent, true), 1, "no stack of gear hides you from someone adjacent");
        assert!(notices(1, &notice, &silent, false, 99));
        assert_eq!(notice_chance(&notice, &silent), 0, "subtlety can take the chance to nothing");
    }

    #[test]
    fn over_a_seed_range_a_nonzero_chance_is_eventually_noticed_and_a_zero_one_never_is() {
        let notice = NoticeStats { certain: 1, chance_pct: 10, lit_bonus: 0, memory: 5 };
        let hidden = StealthStats { quiet: 0, subtlety: 10 };
        for seed in 0..200 {
            let mut rng = StdRng::seed_from_u64(seed);
            let first = (0..500).find(|_| notices(5, &notice, &StealthStats::default(), false, rng.random_range(0..100)));
            assert!(first.is_some_and(|t| t < 500), "seed {seed}: a 10% chance lands within 500 turns");
            let mut rng = StdRng::seed_from_u64(seed);
            assert!(
                (0..500).all(|_| !notices(5, &notice, &hidden, false, rng.random_range(0..100))),
                "seed {seed}: a zero chance beyond the radius never lands"
            );
        }
    }

    #[test]
    fn per_observer_spots_by_the_chance_alone_while_asleep() {
        let notice = NoticeStats { certain: 3, chance_pct: 20, lit_bonus: 0, memory: 5 };
        let plain = StealthStats::default();
        let asleep = Glimpse { distance: 1, in_view: true, lit: false, asleep: true };
        assert!(!spots(&notice, &plain, asleep, 25), "inside the certain radius, a sleeper still needs the chance");
        assert!(spots(&notice, &plain, asleep, 19));
        let awake = Glimpse { asleep: false, ..asleep };
        assert!(spots(&notice, &plain, awake, 99), "awake, the certain radius notices whatever the roll");
    }

    #[test]
    fn notice_stats_read_from_ron_with_light_and_memory_left_out() {
        let n: NoticeStats = ron::from_str("(certain: 2, chance_pct: 15)").unwrap();
        assert_eq!(n, NoticeStats { certain: 2, chance_pct: 15, lit_bonus: 0, memory: 6 });
    }
}
