//! Counting steps over open cells, for the passes that ask how far one
//! place is from another on foot.
//!
//! Orthogonal steps and no costs: the strictest reading of "can be walked
//! to", and a generator comparing two routes wants their lengths, not how
//! long a particular mover takes over them.

use std::collections::VecDeque;

use rl_core::{Direction, Grid, Point};

/// The count at a cell no source reaches.
pub(crate) const UNREACHED: u32 = u32::MAX;

/// Orthogonal steps to every open cell from the nearest of `from`, and
/// [`UNREACHED`] everywhere else. A source that is not open is skipped.
pub(crate) fn steps_from(width: i32, height: i32, open: impl Fn(Point) -> bool, from: impl IntoIterator<Item = Point>) -> Grid<u32> {
    let mut steps = Grid::filled(width, height, UNREACHED);
    let mut queue = VecDeque::new();
    for p in from {
        if open(p) && steps.get(p).copied() == Some(UNREACHED) {
            steps.set(p, 0);
            queue.push_back(p);
        }
    }
    while let Some(p) = queue.pop_front() {
        let next = steps.get(p).copied().expect("queued cells are on the grid") + 1;
        for d in Direction::CARDINALS {
            let n = p + d.offset();
            if steps.get(n).copied() == Some(UNREACHED) && open(n) {
                steps.set(n, next);
                queue.push_back(n);
            }
        }
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_core::Point;

    #[test]
    fn steps_are_counted_orthogonally_round_a_wall_and_a_walled_cell_is_unreached() {
        // 5x3, a wall down the middle column except its bottom cell.
        let open = |p: Point| !(p.x == 2 && p.y < 2);
        let d = steps_from(5, 3, open, [Point::new(0, 0)]);
        assert_eq!(*d.get(Point::new(1, 0)).unwrap(), 1);
        assert_eq!(*d.get(Point::new(3, 0)).unwrap(), 7, "down two, across three, up two");
        assert_eq!(*d.get(Point::new(2, 0)).unwrap(), UNREACHED);
    }

    #[test]
    fn steps_from_several_sources_are_from_the_nearest() {
        let d = steps_from(7, 1, |_| true, [Point::new(0, 0), Point::new(6, 0)]);
        assert_eq!(*d.get(Point::new(3, 0)).unwrap(), 3);
        assert_eq!(*d.get(Point::new(5, 0)).unwrap(), 1);
    }
}
