//! Where an arrival that takes up its cell stands.

use atglyph_core::{Direction, Point};

/// Where an arrival wanting `want` stands: `want` itself when it is free,
/// and otherwise the nearest free cell that can be walked to from it.
///
/// `want` is taken on trust, as it always was: it is where the builder or
/// the game said arrivals stand, so only the cells searched round it are
/// asked whether they are floor (`walkable`). That way the second arrival
/// lands beside the first rather than on it or in a wall. The search is
/// breadth first over [`Direction::ALL`] and tries at most `reach` cells
/// before it gives up, so a crowded arrival is refused rather than
/// searched for across the whole map; a taken but walkable cell is
/// searched through, a wall is not.
pub fn landing(want: Point, free: impl Fn(Point) -> bool, walkable: impl Fn(Point) -> bool, reach: usize) -> Option<Point> {
    if free(want) {
        return Some(want);
    }
    let mut seen = vec![want];
    let mut next = 0;
    while next < seen.len() && seen.len() < reach {
        let from = seen[next];
        next += 1;
        for dir in Direction::ALL {
            let p = from + dir.offset();
            if seen.contains(&p) || !walkable(p) {
                continue;
            }
            if free(p) {
                return Some(p);
            }
            seen.push(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const WANT: Point = Point::new(5, 5);

    #[test]
    fn a_free_wanted_cell_is_taken_on_trust_even_in_a_wall() {
        assert_eq!(landing(WANT, |_| true, |_| false, 25), Some(WANT));
    }

    #[test]
    fn a_taken_cell_lands_on_the_first_free_neighbour_in_direction_order() {
        let first = WANT + Direction::ALL[0].offset();
        assert_eq!(landing(WANT, |p| p != WANT, |_| true, 25), Some(first));
    }

    #[test]
    fn a_wall_is_never_landed_on_or_searched_through() {
        let open = |p: Point| p.x >= 5;
        let got = landing(WANT, |p| p != WANT, open, 25).unwrap();
        assert!(open(got) && got != WANT);
        assert_eq!(landing(WANT, |p| p != WANT, |_| false, 25), None);
    }

    #[test]
    fn a_taken_ring_is_searched_through_to_the_next() {
        let taken = |p: Point| atglyph_core::geometry::chebyshev(p, WANT) <= 1;
        let got = landing(WANT, |p| !taken(p), |_| true, 25).unwrap();
        assert_eq!(atglyph_core::geometry::chebyshev(got, WANT), 2);
    }

    #[test]
    fn the_search_gives_up_once_reach_cells_are_seen() {
        // Expanding the wanted cell sees it and its eight neighbours, nine
        // in all, and the first of those is expanded next; the free cell
        // lies just past it.
        let far = WANT + Direction::ALL[0].offset() + Direction::ALL[0].offset();
        let free = |p: Point| p == far;
        assert_eq!(landing(WANT, free, |_| true, 10), Some(far));
        assert_eq!(landing(WANT, free, |_| true, 9), None);
    }
}
