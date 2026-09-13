//! Who has noticed whom, and for how long they remember.
//!
//! Without this a mind sees everything it has a line to on the turn it
//! first has one, which leaves a player nothing to break and a monster
//! nothing to regain. This is the layer between "could be seen" and "has
//! been seen": a roll to notice, and a memory that decays.
//!
//! Two knobs rather than a radius. A radius alone is a hard line the player
//! learns to stand behind; a chance alone is a lottery with no readable
//! edge. A certain distance inside which nothing helps, and a chance per
//! turn beyond it, is both readable and tense.
//!
//! Light is one number, [`NoticeStats::lit_bonus`], and it defaults to zero,
//! so a game with no lighting is unaffected without saying so. Everything
//! here is pure: the caller rolls, the caller decides what "lit" means, and
//! two runs of one seed agree.

use rl_core::{Point, geometry};
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

/// One observer's knowledge of one subject.
///
/// Two variants because the third is derivable: `Alert` and in sight this
/// turn is hunting, `Alert` and out of sight is searching. Storing a third
/// would be a third thing to keep true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Awareness {
    /// Has not noticed.
    #[default]
    Unaware,
    /// Knows where the subject was, and how many turns ago.
    Alert {
        /// Where it was last seen, or last heard from.
        at: Point,
        /// Turns since then.
        stale_turns: u32,
    },
}

impl Awareness {
    /// A turn in which the subject was noticed at `at`. Returns whether
    /// that was news: `true` on the flip from unaware.
    ///
    /// Resets the staleness as well as the position. Without that a
    /// monster that has chased for longer than its memory gives up on the
    /// turn it catches you.
    pub fn saw(&mut self, at: Point) -> bool {
        let news = !self.is_alert();
        *self = Awareness::Alert { at, stale_turns: 0 };
        news
    }

    /// A turn in which the subject was not noticed. Forgets it once more
    /// than `memory` turns have passed since it was.
    pub fn lost(&mut self, memory: u32) {
        if let Awareness::Alert { at, stale_turns } = *self {
            let stale_turns = stale_turns + 1;
            *self = if stale_turns > memory { Awareness::Unaware } else { Awareness::Alert { at, stale_turns } };
        }
    }

    /// Something told it where the subject is without it looking: a blow,
    /// a shout. The same as seeing it there.
    pub fn alerted_to(&mut self, at: Point) -> bool {
        self.saw(at)
    }

    /// Whether it knows about the subject at all.
    pub fn is_alert(&self) -> bool {
        matches!(self, Awareness::Alert { .. })
    }

    /// Where it last knew the subject to be.
    pub fn last_known(&self) -> Option<Point> {
        match self {
            Awareness::Alert { at, .. } => Some(*at),
            Awareness::Unaware => None,
        }
    }

    /// How many turns since it last knew, while alert.
    pub fn stale_turns(&self) -> Option<u32> {
        match self {
            Awareness::Alert { stale_turns, .. } => Some(*stale_turns),
            Awareness::Unaware => None,
        }
    }
}

/// Whether `from` is within `reach` of `to`. The perception cap every
/// caller applies before asking [`notices`], here so the two agree on the
/// metric.
pub fn within_reach(from: Point, to: Point, reach: i32) -> bool {
    geometry::chebyshev(from, to) <= reach
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
    fn a_lost_trail_is_forgotten_exactly_when_memory_runs_out() {
        let mut a = Awareness::default();
        assert!(a.saw(Point::new(4, 4)), "the first sighting is news");
        for turn in 1..=5 {
            a.lost(5);
            assert_eq!(a.stale_turns(), Some(turn), "still searching after {turn}");
        }
        a.lost(5);
        assert_eq!(a, Awareness::Unaware, "forgotten on the sixth turn without a sighting");
    }

    #[test]
    fn a_sighting_mid_search_resets_the_memory_and_is_not_news() {
        let mut a = Awareness::default();
        a.saw(Point::new(1, 1));
        a.lost(5);
        a.lost(5);
        a.lost(5);
        assert!(!a.saw(Point::new(2, 2)), "already alert, so not news");
        assert_eq!(a.stale_turns(), Some(0), "the count restarts, not only the position");
        assert_eq!(a.last_known(), Some(Point::new(2, 2)));
    }

    #[test]
    fn a_blow_wakes_an_unaware_observer_and_points_it_at_the_attacker() {
        let mut a = Awareness::Unaware;
        assert!(a.alerted_to(Point::new(9, 3)));
        assert_eq!(a.last_known(), Some(Point::new(9, 3)));
    }

    #[test]
    fn notice_stats_read_from_ron_with_light_and_memory_left_out() {
        let n: NoticeStats = ron::from_str("(certain: 2, chance_pct: 15)").unwrap();
        assert_eq!(n, NoticeStats { certain: 2, chance_pct: 15, lit_bonus: 0, memory: 6 });
    }
}
