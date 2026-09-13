//! The arithmetic two cursors share.
//!
//! The look cursor and the targeting cursor open onto the nearest thing
//! worth pointing at, step with the direction keys, cycle through what is
//! in sight and stay inside the loaded window. That is the same three
//! sums twice, and getting the third one wrong is how a cursor walks off
//! the map into tiles the engine cannot answer questions about.
//!
//! State is not shared, because the two cursors hold different things:
//! one remembers a cell, the other a cell and what is being aimed. Only
//! the sums are here.

use rl_core::{Direction, Point, Rect, geometry};

/// `cells` nearest to `origin` first, ties by position, without repeats.
///
/// The tie-break is by position rather than by whatever order the query
/// returned, so cycling twice from one place lands on the same second
/// thing in two runs of one seed.
pub fn ordered(origin: Point, cells: impl IntoIterator<Item = Point>) -> Vec<Point> {
    let mut out: Vec<Point> = cells.into_iter().collect();
    out.sort_by_key(|p| (geometry::chebyshev(origin, *p), p.x, p.y));
    out.dedup();
    out
}

/// The candidate after `at`, wrapping round the end.
///
/// The first when `at` is not one of them, which is what a cursor nudged
/// off a target with the direction keys should snap back to, and `None`
/// when there is nothing to cycle through.
pub fn next_of(cells: &[Point], at: Point) -> Option<Point> {
    match cells.iter().position(|p| *p == at) {
        Some(i) => cells.get((i + 1) % cells.len()).copied(),
        None => cells.first().copied(),
    }
}

/// `at` moved one step, or left where it was when that leaves `bounds`.
///
/// Stopping rather than clamping: a cursor at the edge that slides along
/// it when pushed outwards reads as the cursor moving on its own.
pub fn stepped(at: Point, step: Direction, bounds: Rect) -> Point {
    let moved = at + step.offset();
    if bounds.contains(moved) { moved } else { at }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_come_back_nearest_first_and_once_each() {
        let origin = Point::new(5, 5);
        let cells = [Point::new(9, 5), Point::new(6, 5), Point::new(9, 5), Point::new(5, 7)];
        assert_eq!(ordered(origin, cells), vec![Point::new(6, 5), Point::new(5, 7), Point::new(9, 5)]);
        // Two at the same distance are ordered by position, so the second
        // press of the cycle key is the same thing in two runs.
        let tied = ordered(origin, [Point::new(5, 3), Point::new(3, 5)]);
        assert_eq!(tied, vec![Point::new(3, 5), Point::new(5, 3)]);
    }

    #[test]
    fn cycling_wraps_and_a_cursor_off_the_list_snaps_to_the_first() {
        let cells = [Point::new(1, 0), Point::new(2, 0), Point::new(3, 0)];
        assert_eq!(next_of(&cells, Point::new(1, 0)), Some(Point::new(2, 0)));
        assert_eq!(next_of(&cells, Point::new(3, 0)), Some(Point::new(1, 0)), "round the end");
        assert_eq!(next_of(&cells, Point::new(9, 9)), Some(Point::new(1, 0)), "nudged off, snapped back");
        assert_eq!(next_of(&[], Point::ZERO), None);
    }

    #[test]
    fn a_step_that_would_leave_the_window_leaves_the_cursor_alone() {
        let bounds = Rect::new(0, 0, 10, 10);
        assert_eq!(stepped(Point::new(5, 5), Direction::East, bounds), Point::new(6, 5));
        assert_eq!(stepped(Point::new(9, 5), Direction::East, bounds), Point::new(9, 5), "held at the edge");
        assert_eq!(stepped(Point::new(9, 5), Direction::NorthEast, bounds), Point::new(9, 5), "not slid along it");
        assert_eq!(stepped(Point::new(0, 0), Direction::NorthWest, bounds), Point::new(0, 0));
    }
}
