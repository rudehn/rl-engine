//! Who has noticed whom, and for how long they remember.
//!
//! Without this a mind sees everything it has a line to on the turn it
//! first has one, which leaves a player nothing to break and a monster
//! nothing to regain. This is the layer between "could be seen" and "has
//! been seen": a roll to notice, and a memory that decays.
//!
//! Which rule judges a glimpse lives beside this, one module each: [`per_observer`]
//! and [`by_range`]. What they share is here: [`Awareness`], the [`Glimpse`] a rule
//! judges, and [`observe`], which turns a judgement into what the observer knows.
//!
//! Everything here is pure: the caller rolls, the caller decides what "lit"
//! means, and two runs of one seed agree.

use atglyph_core::{Point, geometry};

pub mod by_range;
pub mod per_observer;

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

/// What one observer has of one subject this turn, for a rule to judge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glimpse {
    /// Chebyshev distance between them.
    pub distance: i32,
    /// Whether the observer has a line to the subject, within its reach.
    pub in_view: bool,
    /// Whether the subject stands in the lit band.
    pub lit: bool,
    /// Whether the observer is asleep.
    pub asleep: bool,
}

/// One observer's turn with one subject: what it knows afterward, and
/// whether that is news (it flipped from unaware).
///
/// The rule behind `holds` and `spots` is the caller's. An alert observer
/// in view keeps its subject when `holds`, whatever a roll would say; when
/// it does not hold, the subject goes stale as if out of view. Only an
/// unaware one in view asks `spots`, so a monster in plain view does not
/// lose you to a bad roll and a draw is spent exactly when one was before.
/// `spots` is a closure so the caller's stream is untouched on every other
/// path. Out of view an alert observer ages its memory by a turn,
/// forgetting past `memory`, and an unaware one stays so.
pub fn observe(state: Awareness, glimpse: Glimpse, holds: bool, spots: impl FnOnce() -> bool, at: Point, memory: u32) -> (Awareness, bool) {
    let mut state = state;
    let news = if glimpse.in_view && state.is_alert() && holds {
        state.saw(at)
    } else if glimpse.in_view && !state.is_alert() {
        spots() && state.saw(at)
    } else {
        state.lost(memory);
        false
    };
    (state, news)
}

/// Whether a watcher has not noticed a subject at all, which a blow from
/// the subject asks before it lands harder for it.
///
/// A sleeper is unaware of everyone, hiding or not: it noticed nothing, and
/// the blow that wakes it is still a blow on a sleeper. Otherwise only where
/// stealth could have hidden the one from the other: `running` is whether
/// stealth applies to this pair at all (the plugin is on, the watcher keeps
/// a memory and is not the subject), `hides` whether the subject can hide,
/// `at_odds` whether they are on opposite sides, and `knows` whether the
/// watcher remembers the subject, so one searching a lost trail is not
/// caught unaware.
pub fn unaware(asleep: bool, running: bool, hides: bool, at_odds: bool, knows: bool) -> bool {
    asleep || (running && hides && at_odds && !knows)
}

/// Whether `from` is within `reach` of `to`. The perception cap every
/// caller applies before asking [`per_observer::notices`], here so the two agree on the
/// metric.
pub fn within_reach(from: Point, to: Point, reach: i32) -> bool {
    geometry::chebyshev(from, to) <= reach
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn glimpse(in_view: bool) -> Glimpse {
        Glimpse { distance: 4, in_view, lit: false, asleep: false }
    }

    fn calls_of(state: Awareness, in_view: bool, spots: bool, at: Point, memory: u32) -> (Awareness, bool, u32) {
        let mut calls = 0;
        let (next, news) = observe(
            state,
            glimpse(in_view),
            in_view,
            || {
                calls += 1;
                spots
            },
            at,
            memory,
        );
        (next, news, calls)
    }

    #[test]
    fn an_unaware_observer_in_view_rolls_once_and_is_news_only_when_it_spots() {
        let at = Point::new(5, 5);
        let (state, news, calls) = calls_of(Awareness::Unaware, true, true, at, 3);
        assert_eq!((state, news, calls), (Awareness::Alert { at, stale_turns: 0 }, true, 1));
        let (state, news, calls) = calls_of(Awareness::Unaware, true, false, at, 3);
        assert_eq!((state, news, calls), (Awareness::Unaware, false, 1), "a miss leaves it unaware, having rolled");
    }

    #[test]
    fn an_alert_observer_in_view_that_does_not_hold_ages_its_memory() {
        let at = Point::new(3, 3);
        let old = Awareness::Alert { at, stale_turns: 0 };
        let (state, news) = observe(old, glimpse(true), false, || panic!("no roll"), at, 6);
        assert_eq!((state, news), (Awareness::Alert { at, stale_turns: 1 }, false));
    }

    #[test]
    fn an_alert_observer_in_view_keeps_its_subject_without_rolling() {
        let old = Awareness::Alert { at: Point::new(1, 1), stale_turns: 2 };
        let at = Point::new(6, 2);
        let (state, news, calls) = calls_of(old, true, false, at, 3);
        assert_eq!((state, news, calls), (Awareness::Alert { at, stale_turns: 0 }, false, 0), "moved up to date, no roll to lose it to, not news");
    }

    #[test]
    fn an_observer_out_of_view_never_rolls_and_ages_what_it_knew_until_memory_runs_out() {
        let at = Point::new(2, 2);
        let (state, news, calls) = calls_of(Awareness::Unaware, false, true, at, 3);
        assert_eq!((state, news, calls), (Awareness::Unaware, false, 0));
        let mut state = Awareness::Alert { at, stale_turns: 0 };
        for turn in 1..=3 {
            let (next, news, calls) = calls_of(state, false, true, Point::new(9, 9), 3);
            assert_eq!((news, calls), (false, 0));
            assert_eq!(next, Awareness::Alert { at, stale_turns: turn }, "still searching where it last knew");
            state = next;
        }
        assert_eq!(calls_of(state, false, true, at, 3).0, Awareness::Unaware, "forgotten one turn past its memory");
    }

    #[test]
    fn a_sleeper_a_hider_and_an_ally_are_each_unaware_and_a_remembered_subject_is_not() {
        // asleep, running, hides, at_odds, knows
        assert!(unaware(true, false, false, false, true), "a sleeper is unaware of everyone, hiding or not");
        assert!(unaware(false, true, true, true, false), "a hider at odds that nothing remembers");
        assert!(!unaware(false, true, true, true, true), "one that lost the trail still remembers");
        assert!(!unaware(false, false, true, true, false), "no stealth running hides nothing");
        assert!(!unaware(false, true, false, true, false), "a subject that does not hide is always seen");
        assert!(!unaware(false, true, true, false, false), "an ally is not caught unaware by a friend");
    }
}
