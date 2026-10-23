//! How far a sound carries, and whether a listener hears what is left.
//!
//! Hearing is a second sense beside sight, and deliberately not a knob on
//! [`NoticeStats`](super::NoticeStats): sight answers who is seen, hearing
//! answers where something happened. A listener that hears a sound knows a
//! place, never a who, and goes to look; whether it then sees anyone is the
//! ordinary notice roll.
//!
//! Loudness and threshold are whole steps, the way a designer thinks of
//! "heard eight tiles off". What a sound spends travelling is hundredths of
//! a step, the engine's one unit for costs, so a diagonal spends 141 of it
//! and not 100.
//!
//! What a listener remembers is an [`Awareness`](super::Awareness): `Alert`
//! is "heard something there, this many turns ago", and it is forgotten
//! the way a lost trail is.

use std::cmp::Reverse;

use atglyph_core::Point;
use atglyph_core::turn::BASE_ACTION_COST;
use serde::{Deserialize, Serialize};

/// How keenly a listener hears.
///
/// Serde-ready, so a game writes it straight into a bestiary file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HearingStats {
    /// Loudness must arrive with at least this many steps of it left to be
    /// heard. Zero hears a sound to the last step it carries.
    pub threshold: i32,
    /// Turns it goes on looking for a sound before it forgets.
    #[serde(default = "default_memory")]
    pub memory: u32,
}

fn default_memory() -> u32 {
    6
}

impl Default for HearingStats {
    /// Hears to the edge of a sound, and looks for six turns.
    fn default() -> Self {
        Self { threshold: 0, memory: default_memory() }
    }
}

/// What entering a tile costs a sound, in hundredths of a step, or `None`
/// when the tile stops it.
///
/// Read from the flags a tile already has rather than a field of its own:
/// what a thrown thing passes, sound passes at one step; what stops a
/// thrown thing and opens is a closed door, which sound passes at one step
/// and `muffle` more; anything else that stops a thrown thing is a wall.
pub fn carries(blocks_projectiles: bool, opens: bool, muffle: i32) -> Option<u32> {
    if !blocks_projectiles {
        Some(BASE_ACTION_COST)
    } else if opens {
        Some(BASE_ACTION_COST + muffle.max(0) as u32 * BASE_ACTION_COST)
    } else {
        None
    }
}

/// How much of `loudness` steps is left after travelling `travelled`
/// hundredths of a step, in hundredths.
pub fn left_after(loudness: i32, travelled: i32) -> i32 {
    loudness.saturating_mul(BASE_ACTION_COST as i32).saturating_sub(travelled)
}

/// Whether `left` hundredths of loudness reach a listener of `threshold`.
pub fn heard(left: i32, threshold: i32) -> bool {
    left >= threshold.saturating_mul(BASE_ACTION_COST as i32)
}

/// Of the sounds that reached one listener in a pass, as how much of each
/// was left and where it was made, the place it goes to look: the one with
/// the most left.
///
/// Equally loud arrivals go to the cell earliest in row-major order (the
/// order [`Point`] sorts in), so which place a listener follows never
/// depends on the order the sounds were written in.
pub fn loudest(arrivals: impl IntoIterator<Item = (i32, Point)>) -> Option<Point> {
    arrivals.into_iter().max_by_key(|&(left, at)| (left, Reverse(at))).map(|(_, at)| at)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: i32 = BASE_ACTION_COST as i32;

    #[test]
    fn a_wall_carries_nothing_and_a_closed_door_exactly_its_muffle_more_than_open_ground() {
        assert_eq!(carries(true, false, 0), None, "a wall");
        assert_eq!(carries(true, false, 50), None, "a wall, whatever the muffle");
        let open = carries(false, false, 3).unwrap();
        let door = carries(true, true, 3).unwrap();
        assert_eq!(door - open, 3 * BASE_ACTION_COST, "a closed door is three steps more");
        assert_eq!(carries(false, true, 3), Some(open), "an open doorway is open ground");
        assert_eq!(carries(true, true, -4), Some(open), "a muffle below zero is none, not a shortcut");
    }

    #[test]
    fn what_is_left_never_grows_with_distance() {
        for loudness in 0..20 {
            let mut last = i32::MAX;
            for travelled in (0..3000).step_by(47) {
                let left = left_after(loudness, travelled);
                assert!(left <= last, "loudness {loudness}: {left} after {travelled}, {last} before");
                last = left;
            }
        }
    }

    #[test]
    fn a_sound_of_loudness_n_reaches_a_threshold_of_zero_at_n_steps_and_not_one_more() {
        for n in 0..16 {
            assert!(heard(left_after(n, n * STEP), 0), "loudness {n} at {n} steps");
            assert!(!heard(left_after(n, (n + 1) * STEP), 0), "loudness {n} at {} steps", n + 1);
        }
    }

    #[test]
    fn a_threshold_takes_its_steps_off_the_reach() {
        let threshold = 3;
        assert!(heard(left_after(8, 5 * STEP), threshold), "eight carries five steps to a threshold of three");
        assert!(!heard(left_after(8, 5 * STEP + 1), threshold), "and not a hundredth further");
    }

    #[test]
    fn of_several_arrivals_the_one_with_the_most_left_wins_whatever_the_order() {
        let (near, far) = ((500, Point::new(0, 2)), (400, Point::new(6, 6)));
        assert_eq!(loudest([near, far]), Some(near.1));
        assert_eq!(loudest([far, near]), Some(near.1));
    }

    #[test]
    fn of_two_arrivals_equally_loud_the_earlier_cell_in_row_major_order_wins_whatever_the_order() {
        let (above, beside, below) = (Point::new(9, 1), Point::new(2, 1), Point::new(0, 3));
        assert_eq!(loudest([(300, below), (300, above), (300, beside)]), Some(beside));
        assert_eq!(loudest([(300, beside), (300, above), (300, below)]), Some(beside));
        assert_eq!(loudest([(300, above), (300, below)]), Some(above), "a lower row beats a lower column");
    }

    #[test]
    fn no_arrivals_leave_nothing_to_follow() {
        assert_eq!(loudest([]), None);
    }

    #[test]
    fn hearing_stats_read_from_ron_with_memory_left_out() {
        let h: HearingStats = ron::from_str("(threshold: 2)").unwrap();
        assert_eq!(h, HearingStats { threshold: 2, memory: 6 });
    }
}
