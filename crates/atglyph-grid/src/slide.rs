//! How far a push or a pull carries, along a straight line.

use atglyph_core::Point;

/// Where something ends after sliding up to `cells` steps from `from` along
/// the line toward `to`, stopping at the last cell `free` accepts.
///
/// Each step moves by the sign of each axis, so the walk is the same one
/// whatever the distance to `to`; only its direction is read. A push and a
/// pull are the same walk with the ends swapped, so both stop at a wall
/// rather than through it. Nothing moves toward its own cell or for no
/// cells.
pub fn slide(from: Point, toward: Point, cells: i32, free: impl Fn(Point) -> bool) -> Point {
    let (dx, dy) = ((toward.x - from.x).signum(), (toward.y - from.y).signum());
    if dx == 0 && dy == 0 {
        return from;
    }
    let mut at = from;
    for _ in 0..cells.max(0) {
        let next = at.offset(dx, dy);
        if !free(next) {
            break;
        }
        at = next;
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    const WALL_AT_X: i32 = 5;
    fn free(p: Point) -> bool {
        p.x != WALL_AT_X && p.x >= 0
    }

    #[test]
    fn a_slide_goes_the_whole_distance_over_open_ground() {
        assert_eq!(slide(Point::new(1, 1), Point::new(2, 1), 3, free), Point::new(4, 1));
    }

    #[test]
    fn a_slide_stops_at_the_last_free_cell_before_a_wall() {
        assert_eq!(slide(Point::new(1, 1), Point::new(2, 1), 9, free), Point::new(4, 1));
    }

    #[test]
    fn a_slide_does_not_pass_through_a_wall_to_free_ground_beyond() {
        assert_eq!(slide(Point::new(4, 1), Point::new(5, 1), 3, free), Point::new(4, 1));
    }

    #[test]
    fn a_slide_toward_its_own_cell_or_for_no_cells_stays_put() {
        let from = Point::new(2, 2);
        assert_eq!(slide(from, from, 4, free), from);
        assert_eq!(slide(from, Point::new(3, 2), 0, free), from);
        assert_eq!(slide(from, Point::new(3, 2), -2, free), from);
    }

    #[test]
    fn a_slide_walks_diagonally_by_the_sign_of_each_axis_and_not_toward_the_target_cell() {
        assert_eq!(slide(Point::new(0, 0), Point::new(9, 2), 3, |_| true), Point::new(3, 3));
        assert_eq!(slide(Point::new(8, 8), Point::new(7, 8), 2, |p| p.x >= 0), Point::new(6, 8));
    }
}
