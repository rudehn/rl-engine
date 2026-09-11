//! Point-to-point search.
//!
//! For one mover with one goal. Many movers sharing a goal want a
//! [`DijkstraMap`](crate::dijkstra::DijkstraMap) instead.
//!
//! Costs are integers in hundredths of a step; a diagonal costs 1.414 of an
//! orthogonal move into the same tile. The heuristic is octile distance in
//! the same units, so the search is admissible and returns the cheapest
//! path. Ties break by insertion order, so the same query on the same map
//! returns the same path every run.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rl_core::{Direction, Point, geometry};

use crate::terrain::CostSource;

/// How movement is allowed to step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathRules {
    /// Whether diagonal steps are allowed at all.
    pub diagonals: bool,
    /// Whether a diagonal step may squeeze between two impassable
    /// orthogonal neighbours. Off by default: a mover cannot slip through
    /// the crack between two wall corners.
    pub cut_corners: bool,
}

impl Default for PathRules {
    fn default() -> Self {
        Self { diagonals: true, cut_corners: false }
    }
}

impl PathRules {
    /// Four-way movement only.
    pub const CARDINAL: PathRules = PathRules { diagonals: false, cut_corners: false };

    /// Eight-way movement that cannot cut corners.
    pub const EIGHT_WAY: PathRules = PathRules { diagonals: true, cut_corners: false };

    /// The directions a step may take.
    pub const fn directions(self) -> &'static [Direction] {
        if self.diagonals { &Direction::ALL } else { &Direction::CARDINALS }
    }
}

/// The cost of entering `to` from `from` through `dir`, or `None` if the
/// step is not allowed. Shared by every search in the crate.
pub(crate) fn step_cost(source: &impl CostSource, from: Point, dir: Direction, rules: PathRules) -> Option<(Point, u32)> {
    let to = from + dir.offset();
    let base = source.cost(to)?;
    if dir.is_diagonal() {
        if !rules.cut_corners {
            let (dx, dy) = dir.delta();
            let a = source.is_passable(from.offset(dx, 0));
            let b = source.is_passable(from.offset(0, dy));
            if !a || !b {
                return None;
            }
        }
        Some((to, base * 1414 / 1000))
    } else {
        Some((to, base))
    }
}

/// A found path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    /// The cells to step through, excluding the start and including the goal.
    pub steps: Vec<Point>,
    /// The total entry cost, in hundredths of a step.
    pub cost: u32,
}

/// A* with scratch buffers that persist across searches.
///
/// One instance per pather class or per thread; a search on a map of a
/// different size resizes the buffers once.
#[derive(Debug, Default)]
pub struct AStar {
    g: Vec<u32>,
    came_from: Vec<u32>,
    seen: Vec<u32>,
    closed: Vec<u32>,
    generation: u32,
    heap: BinaryHeap<Reverse<(u32, u64, u32)>>,
}

const NONE: u32 = u32::MAX;

impl AStar {
    /// A searcher with empty buffers.
    pub fn new() -> Self {
        Self::default()
    }

    fn prepare(&mut self, cells: usize) {
        if self.g.len() != cells {
            self.g = vec![0; cells];
            self.came_from = vec![NONE; cells];
            self.seen = vec![0; cells];
            self.closed = vec![0; cells];
            self.generation = 0;
        }
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.seen.iter_mut().for_each(|s| *s = 0);
            self.closed.iter_mut().for_each(|s| *s = 0);
            self.generation = 1;
        }
        self.heap.clear();
    }

    /// The cheapest path from `start` to `goal`, or `None` if unreachable.
    ///
    /// `start` need not be passable (a mover standing in fire still leaves),
    /// but `goal` must be.
    pub fn find(&mut self, source: &impl CostSource, start: Point, goal: Point, rules: PathRules) -> Option<Path> {
        self.find_within(source, start, goal, rules, u32::MAX)
    }

    /// Like [`find`](Self::find) but gives up once every frontier cell costs
    /// more than `max_cost`, so a hopeless search on a big map stays cheap.
    pub fn find_within(&mut self, source: &impl CostSource, start: Point, goal: Point, rules: PathRules, max_cost: u32) -> Option<Path> {
        let start_idx = source.checked_idx(start)?;
        let goal_idx = source.checked_idx(goal)?;
        if !source.is_passable(goal) {
            return None;
        }
        self.prepare(source.len());
        let stamp = self.generation;
        let mut order = 0u64;

        self.g[start_idx] = 0;
        self.came_from[start_idx] = NONE;
        self.seen[start_idx] = stamp;
        let h = geometry::octile_milli(start, goal) / 10;
        self.heap.push(Reverse((h, order, start_idx as u32)));

        while let Some(Reverse((f, _, idx))) = self.heap.pop() {
            let idx = idx as usize;
            if self.closed[idx] == stamp {
                continue;
            }
            if f > max_cost {
                break;
            }
            self.closed[idx] = stamp;
            if idx == goal_idx {
                return Some(self.reconstruct(source, start_idx, goal_idx));
            }
            let here = source.idx_point(idx);
            let g_here = self.g[idx];
            for dir in rules.directions() {
                let Some((next, cost)) = step_cost(source, here, *dir, rules) else {
                    continue;
                };
                let n = source.point_idx(next);
                if self.closed[n] == stamp {
                    continue;
                }
                let tentative = g_here.saturating_add(cost);
                if self.seen[n] != stamp || tentative < self.g[n] {
                    self.seen[n] = stamp;
                    self.g[n] = tentative;
                    self.came_from[n] = idx as u32;
                    order += 1;
                    let h = geometry::octile_milli(next, goal) / 10;
                    self.heap.push(Reverse((tentative.saturating_add(h), order, n as u32)));
                }
            }
        }
        None
    }

    fn reconstruct(&self, source: &impl CostSource, start: usize, goal: usize) -> Path {
        let mut steps = Vec::new();
        let mut idx = goal;
        while idx != start {
            steps.push(source.idx_point(idx));
            idx = self.came_from[idx] as usize;
        }
        steps.reverse();
        Path { steps, cost: self.g[goal] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::fixtures::parse;

    fn p(x: i32, y: i32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_straight_corridor_is_walked_end_to_end() {
        let (t, r) = parse(&["#######", "#.....#", "#######"]);
        let view = t.view(&r);
        let path = AStar::new().find(&view, p(1, 1), p(5, 1), PathRules::default()).unwrap();
        assert_eq!(path.steps, vec![p(2, 1), p(3, 1), p(4, 1), p(5, 1)]);
        assert_eq!(path.cost, 400);
    }

    #[test]
    fn diagonals_are_taken_when_cheaper_and_cost_more_each() {
        let (t, r) = parse(&["....", "....", "....", "...."]);
        let view = t.view(&r);
        let path = AStar::new().find(&view, p(0, 0), p(3, 3), PathRules::default()).unwrap();
        assert_eq!(path.steps.len(), 3);
        assert_eq!(path.cost, 3 * 141);
        let cardinal = AStar::new().find(&view, p(0, 0), p(3, 3), PathRules::CARDINAL).unwrap();
        assert_eq!(cardinal.steps.len(), 6);
        assert_eq!(cardinal.cost, 600);
    }

    #[test]
    fn corners_are_not_cut_unless_allowed() {
        let (t, r) = parse(&[".#", "#."]);
        let view = t.view(&r);
        assert!(AStar::new().find(&view, p(0, 0), p(1, 1), PathRules::default()).is_none());
        let cutting = PathRules { diagonals: true, cut_corners: true };
        assert_eq!(AStar::new().find(&view, p(0, 0), p(1, 1), cutting).unwrap().steps, vec![p(1, 1)]);
    }

    #[test]
    fn expensive_ground_is_avoided_when_a_detour_is_cheaper() {
        let (t, r) = parse(&[".....", ".~~~.", "....."]);
        let view = t.view(&r);
        let path = AStar::new().find(&view, p(0, 1), p(4, 1), PathRules::default()).unwrap();
        assert!(!path.steps.iter().any(|s| s.y == 1 && (1..=3).contains(&s.x)), "walked through mud: {:?}", path.steps);
    }

    #[test]
    fn unreachable_goals_and_walls_return_none() {
        let (t, r) = parse(&["..#..", "..#..", "..#.."]);
        let view = t.view(&r);
        let mut astar = AStar::new();
        assert!(astar.find(&view, p(0, 0), p(4, 0), PathRules::default()).is_none());
        assert!(astar.find(&view, p(0, 0), p(2, 0), PathRules::default()).is_none(), "goal is a wall");
        assert!(astar.find(&view, p(0, 0), p(9, 9), PathRules::default()).is_none(), "goal off map");
        assert_eq!(astar.find(&view, p(0, 0), p(0, 0), PathRules::default()).unwrap().steps.len(), 0);
    }

    #[test]
    fn the_cost_limit_stops_a_long_search() {
        let (t, r) = parse(&[&".".repeat(40)]);
        let view = t.view(&r);
        let mut astar = AStar::new();
        assert!(astar.find_within(&view, p(0, 0), p(39, 0), PathRules::default(), 500).is_none());
        assert!(astar.find_within(&view, p(0, 0), p(39, 0), PathRules::default(), 3900).is_some());
    }

    #[test]
    fn buffers_are_reused_across_searches_and_sizes() {
        let (a, ra) = parse(&["...", "...", "..."]);
        let (b, rb) = parse(&["......", "......"]);
        let mut astar = AStar::new();
        assert!(astar.find(&a.view(&ra), p(0, 0), p(2, 2), PathRules::default()).is_some());
        assert!(astar.find(&b.view(&rb), p(0, 0), p(5, 1), PathRules::default()).is_some());
        assert!(astar.find(&a.view(&ra), p(2, 2), p(0, 0), PathRules::default()).is_some());
    }
}
