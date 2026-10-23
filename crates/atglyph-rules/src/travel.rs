//! Who goes through a way behind whoever took it, as plain values.
//!
//! The Bevy layer gathers who is near, who travels and was after the
//! traveller, and applies the order this returns; the order lives here so
//! it is tested without an `App`. `A` is whoever might follow, so the rule
//! needs no entities.

use atglyph_core::{Point, geometry::chebyshev};

/// Who travels behind the one that went through at `at`, nearest first.
///
/// Each candidate is its id, where it stands and whether it follows at all
/// (the caller folds in everything that decides that: being beside the
/// traveller, a mind that travels, a trail that names it). Ties go to the
/// lower id, so who gets the cell beside the landing never depends on the
/// order a query happened to run in.
pub fn followers<A: Ord + Copy>(candidates: impl IntoIterator<Item = (A, Point, bool)>, at: Point) -> Vec<A> {
    let mut behind: Vec<(i32, A)> = candidates.into_iter().filter(|&(_, _, follows)| follows).map(|(id, pos, _)| (chebyshev(pos, at), id)).collect();
    behind.sort();
    behind.into_iter().map(|(_, id)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: Point = Point::new(10, 10);

    #[test]
    fn followers_are_nearest_first_and_ties_go_to_the_lower_id() {
        let candidates = [(5u32, Point::new(11, 10), true), (2, Point::new(10, 11), true), (9, Point::new(12, 10), true)];
        assert_eq!(followers(candidates, AT), vec![2, 5, 9]);
    }

    #[test]
    fn those_that_do_not_follow_are_left_out() {
        let candidates = [(1u32, Point::new(11, 10), false), (2, Point::new(11, 11), true)];
        assert_eq!(followers(candidates, AT), vec![2]);
    }

    #[test]
    fn nearness_outranks_the_id() {
        let candidates = [(1u32, Point::new(12, 12), true), (7, Point::new(10, 10), true)];
        assert_eq!(followers(candidates, AT), vec![7, 1]);
    }

    #[test]
    fn nobody_following_is_an_empty_list() {
        assert!(followers(Vec::<(u32, Point, bool)>::new(), AT).is_empty());
    }
}
