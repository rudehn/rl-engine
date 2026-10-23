//! Who a mind sees, sorted by how it regards them, and the trail it follows.

use atglyph_core::Point;

use crate::faction::Relation;

/// Sorts what a mind sees into its enemies, its allies and the others, in
/// the order seen.
///
/// `None` is a pair with no stance between them, which is the others: a
/// mind still steps round someone it has no side to regard.
pub fn sort_roster<V>(seen: impl IntoIterator<Item = (V, Option<Relation>)>) -> (Vec<V>, Vec<V>, Vec<V>) {
    let (mut enemies, mut allies, mut others) = (Vec::new(), Vec::new(), Vec::new());
    for (view, relation) in seen {
        match relation {
            Some(Relation::Hostile) => enemies.push(view),
            Some(Relation::Allied) => allies.push(view),
            _ => others.push(view),
        }
    }
    (enemies, allies, others)
}

/// The trail a search follows, offered rather than written.
///
/// Stealth offers where it last saw something and hearing where it last
/// heard something; the freshest wins and a tie goes to the lower cell, so
/// which contributor offered first cannot reach a tactic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Trail(Option<(u32, Point)>);

impl Trail {
    /// Offers something known of at `at`, `stale_turns` ago.
    pub fn offer(&mut self, at: Point, stale_turns: u32) {
        let offer = (stale_turns, at);
        if self.0.is_none_or(|held| offer < held) {
            self.0 = Some(offer);
        }
    }

    /// The cell of the winning offer, if any, leaving none held.
    pub fn take(&mut self) -> Option<Point> {
        self.0.take().map(|(_, at)| at)
    }

    /// Forgets any offer.
    pub fn clear(&mut self) {
        self.0 = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_roster_sorts_into_foes_friends_and_the_others_in_the_order_seen() {
        let seen = [
            (1, Some(Relation::Hostile)),
            (2, Some(Relation::Allied)),
            (3, Some(Relation::Neutral)),
            (4, None),
            (5, Some(Relation::Hostile)),
            (6, Some(Relation::Allied)),
        ];
        assert_eq!(sort_roster(seen), (vec![1, 5], vec![2, 6], vec![3, 4]));
    }

    #[test]
    fn nobody_seen_is_three_empty_lists() {
        assert_eq!(sort_roster(Vec::<(u8, Option<Relation>)>::new()), (vec![], vec![], vec![]));
    }

    #[test]
    fn the_freshest_trail_offered_wins_and_a_tie_goes_to_the_lower_cell() {
        let mut trail = Trail::default();
        assert_eq!(trail.take(), None);
        trail.offer(Point::new(9, 9), 4);
        trail.offer(Point::new(2, 2), 6);
        assert_eq!(trail.take(), Some(Point::new(9, 9)), "the fresher one");
        trail.offer(Point::new(5, 1), 3);
        trail.offer(Point::new(1, 7), 3);
        trail.offer(Point::new(1, 2), 3);
        assert_eq!(trail.take(), Some(Point::new(5, 1)), "ties go to the lower cell, row first, whoever offered first");
        assert_eq!(trail.take(), None, "taking it clears it");
    }

    #[test]
    fn the_order_trails_are_offered_in_cannot_change_which_wins() {
        let offers = [(Point::new(4, 4), 2), (Point::new(1, 1), 2), (Point::new(8, 8), 1), (Point::new(0, 0), 5)];
        let mut forward = Trail::default();
        offers.iter().for_each(|(at, stale)| forward.offer(*at, *stale));
        let mut backward = Trail::default();
        offers.iter().rev().for_each(|(at, stale)| backward.offer(*at, *stale));
        assert_eq!(forward.take(), backward.take());
    }
}
