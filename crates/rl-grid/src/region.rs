//! Flood fill and connected-component labelling.
//!
//! Three generation passes in the source repos each carried their own
//! flood; this is the one copy. Both functions take the passability test
//! as a closure over flat indices, so they run on a terrain view, a scratch
//! grid, or anything else that implements [`Grid2D`].

use rl_core::{Grid, Grid2D, Point, Steps};

use crate::bitgrid::BitGrid;

/// Marks every cell reachable from `start` through `passable` cells into
/// `out`, which is cleared first. Returns how many cells were reached.
/// `start` itself must be passable or nothing is reached.
pub fn flood(grid: &impl Grid2D, start: Point, passable: impl Fn(usize) -> bool, steps: Steps, out: &mut BitGrid) -> usize {
    out.clear();
    let Some(start_idx) = grid.checked_idx(start) else {
        return 0;
    };
    if !passable(start_idx) {
        return 0;
    }
    let mut stack = vec![start_idx];
    out.insert_idx(start_idx);
    let mut count = 0;
    while let Some(idx) = stack.pop() {
        count += 1;
        let p = grid.idx_point(idx);
        for n in grid.neighbour_indices(p, steps) {
            if !out.get_idx(n) && passable(n) {
                out.insert_idx(n);
                stack.push(n);
            }
        }
    }
    count
}

/// Connected components of the passable cells, each numbered from 1.
/// Label 0 means impassable.
#[derive(Debug, Clone)]
pub struct Regions {
    labels: Grid<u32>,
    sizes: Vec<usize>,
}

impl Regions {
    /// The label of `p`, or `None` if impassable or out of bounds.
    pub fn label(&self, p: Point) -> Option<u32> {
        self.labels.get(p).copied().filter(|l| *l != 0)
    }

    /// The label at a flat index, or `None` if impassable.
    pub fn label_idx(&self, idx: usize) -> Option<u32> {
        let l = self.labels[idx];
        (l != 0).then_some(l)
    }

    /// Number of regions.
    pub fn count(&self) -> usize {
        self.sizes.len()
    }

    /// Cells in region `label`.
    pub fn size(&self, label: u32) -> usize {
        self.sizes[label as usize - 1]
    }

    /// The label of the biggest region, lowest label on a tie.
    pub fn largest(&self) -> Option<u32> {
        (1..=self.sizes.len() as u32).max_by_key(|l| (self.size(*l), std::cmp::Reverse(*l)))
    }

    /// The label grid itself.
    pub fn labels(&self) -> &Grid<u32> {
        &self.labels
    }

    /// Whether `a` and `b` are passable and in the same region.
    pub fn connected(&self, a: Point, b: Point) -> bool {
        matches!((self.label(a), self.label(b)), (Some(x), Some(y)) if x == y)
    }
}

/// Labels every connected component of passable cells in one pass.
pub fn label_regions(grid: &impl Grid2D, passable: impl Fn(usize) -> bool, steps: Steps) -> Regions {
    let mut labels = Grid::filled(grid.width(), grid.height(), 0u32);
    let mut sizes = Vec::new();
    let mut stack = Vec::new();
    for start in 0..grid.len() {
        if labels[start] != 0 || !passable(start) {
            continue;
        }
        let label = sizes.len() as u32 + 1;
        let mut size = 0;
        labels[start] = label;
        stack.push(start);
        while let Some(idx) = stack.pop() {
            size += 1;
            let p = grid.idx_point(idx);
            for n in grid.neighbour_indices(p, steps) {
                if labels[n] == 0 && passable(n) {
                    labels[n] = label;
                    stack.push(n);
                }
            }
        }
        sizes.push(size);
    }
    Regions { labels, sizes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::OpacitySource;
    use crate::terrain::fixtures::parse;

    #[test]
    fn flood_reaches_a_room_and_not_past_its_walls() {
        let (t, r) = parse(&["....#...", "....#...", "....#...", "........"]);
        let view = t.view(&r);
        let mut out = BitGrid::new(8, 4);
        let n = flood(&view, Point::new(0, 0), |i| !view.is_opaque_idx(i), Steps::Four, &mut out);
        assert_eq!(n, 29);
        assert!(out.contains(Point::new(7, 0)), "reachable around the wall's end");
        assert!(!out.contains(Point::new(4, 1)));
        assert_eq!(flood(&view, Point::new(4, 1), |i| !view.is_opaque_idx(i), Steps::Four, &mut out), 0);
    }

    #[test]
    fn labelling_separates_and_sizes_components() {
        let (t, r) = parse(&["..#..", "..#..", "#####", "....."]);
        let view = t.view(&r);
        let regions = label_regions(&view, |i| !view.is_opaque_idx(i), Steps::Four);
        assert_eq!(regions.count(), 3);
        assert_eq!(regions.label(Point::new(0, 0)), Some(1));
        assert_eq!(regions.label(Point::new(4, 0)), Some(2));
        assert_eq!(regions.label(Point::new(0, 3)), Some(3));
        assert_eq!(regions.label(Point::new(2, 0)), None);
        assert_eq!(regions.size(3), 5);
        assert_eq!(regions.largest(), Some(3));
        assert!(regions.connected(Point::new(0, 0), Point::new(1, 1)));
        assert!(!regions.connected(Point::new(0, 0), Point::new(4, 0)));
    }

    #[test]
    fn eight_way_connects_through_diagonals_and_four_way_does_not() {
        let (t, r) = parse(&[".#", "#."]);
        let view = t.view(&r);
        assert_eq!(label_regions(&view, |i| !view.is_opaque_idx(i), Steps::Four).count(), 2);
        assert_eq!(label_regions(&view, |i| !view.is_opaque_idx(i), Steps::Eight).count(), 1);
    }

    #[test]
    fn largest_prefers_the_lowest_label_on_a_tie() {
        let (t, r) = parse(&[".#."]);
        let view = t.view(&r);
        let regions = label_regions(&view, |i| !view.is_opaque_idx(i), Steps::Four);
        assert_eq!(regions.largest(), Some(1));
    }
}
