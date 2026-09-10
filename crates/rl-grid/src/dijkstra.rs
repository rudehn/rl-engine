//! Dijkstra maps: one flood, any number of consumers.
//!
//! A map holds, for every cell in a region, its cost to the nearest goal.
//! Building it is one search from every goal at once. Using it is looking
//! at eight neighbours and stepping onto the lowest, which is what lets
//! fifty monsters share one map instead of running fifty searches.
//!
//! Maps are bounded to a region so a continuous world never floods a
//! million cells per turn. Cells outside the region are unreached.
//!
//! Values are signed so a map can be inverted into a safety map: scale
//! every value by a negative factor and [`rescan`](DijkstraMap::rescan),
//! and the low points become the places far from the goals that are also
//! reachable without passing near them. Fleeing monsters descend that.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rl_core::{Direction, Grid2D, Point, Rect};

use crate::astar::{PathRules, step_cost};
use crate::terrain::CostSource;

/// The value of a cell the flood did not reach.
pub const UNREACHED: i32 = i32::MAX;

/// A grid of costs-to-goal over a region of a larger map.
#[derive(Debug, Clone)]
pub struct DijkstraMap {
    region: Rect,
    values: Vec<i32>,
    heap: BinaryHeap<Reverse<(i32, u64, u32)>>,
}

impl DijkstraMap {
    /// An unreached map over `region`.
    pub fn new(region: Rect) -> Self {
        Self {
            region,
            values: vec![UNREACHED; region.area().max(0) as usize],
            heap: BinaryHeap::new(),
        }
    }

    /// A map over a whole grid.
    pub fn covering(grid: &impl Grid2D) -> Self {
        Self::new(grid.bounds())
    }

    /// The region this map covers.
    pub fn region(&self) -> Rect {
        self.region
    }

    /// Moves the map to a new region, resizing if needed. Values are reset.
    pub fn reset(&mut self, region: Rect) {
        self.region = region;
        let cells = region.area().max(0) as usize;
        self.values.clear();
        self.values.resize(cells, UNREACHED);
        self.heap.clear();
    }

    fn local(&self, p: Point) -> Option<usize> {
        self.region
            .contains(p)
            .then(|| ((p.y - self.region.y) * self.region.width + (p.x - self.region.x)) as usize)
    }

    fn point(&self, local: usize) -> Point {
        let w = self.region.width as usize;
        Point::new(self.region.x + (local % w) as i32, self.region.y + (local / w) as i32)
    }

    /// The value at `p`, or `None` if outside the region or unreached.
    pub fn value(&self, p: Point) -> Option<i32> {
        self.local(p).map(|i| self.values[i]).filter(|v| *v != UNREACHED)
    }

    /// Floods from `goals` (each at value 0) through `source`.
    /// Goals outside the region or on impassable cells are ignored.
    pub fn build(&mut self, source: &impl CostSource, goals: impl IntoIterator<Item = Point>, rules: PathRules) {
        self.build_weighted(source, goals.into_iter().map(|g| (g, 0)), rules);
    }

    /// Floods from `goals`, each starting at its own value, so a more
    /// attractive goal can start lower.
    pub fn build_weighted(&mut self, source: &impl CostSource, goals: impl IntoIterator<Item = (Point, i32)>, rules: PathRules) {
        self.values.iter_mut().for_each(|v| *v = UNREACHED);
        self.heap.clear();
        let mut order = 0u64;
        for (goal, start) in goals {
            let Some(local) = self.local(goal) else { continue };
            if !source.is_passable(goal) {
                continue;
            }
            if start < self.values[local] {
                self.values[local] = start;
                order += 1;
                self.heap.push(Reverse((start, order, local as u32)));
            }
        }
        self.relax(source, rules, order);
    }

    /// Re-runs the relaxation from the current values, after the caller has
    /// changed them. The standard use is a safety map:
    ///
    /// ```
    /// # use rl_core::{Point, Rect};
    /// # use rl_grid::{DijkstraMap, PathRules, Terrain, TileRegistry};
    /// # let registry = TileRegistry::standard();
    /// # let terrain = Terrain::filled(10, 10, registry.expect("floor"));
    /// # let view = terrain.view(&registry);
    /// let mut map = DijkstraMap::covering(&view);
    /// map.build(&view, [Point::new(5, 5)], PathRules::default());
    /// map.scale(-12, 10);
    /// map.rescan(&view, PathRules::default());
    /// // Now descending the map walks away from (5, 5).
    /// assert!(map.value(Point::new(0, 0)) < map.value(Point::new(4, 5)));
    /// ```
    pub fn rescan(&mut self, source: &impl CostSource, rules: PathRules) {
        self.heap.clear();
        let mut order = 0u64;
        for (local, v) in self.values.iter().enumerate() {
            if *v != UNREACHED {
                order += 1;
                self.heap.push(Reverse((*v, order, local as u32)));
            }
        }
        self.relax(source, rules, order);
    }

    /// Multiplies every reached value by `numer / denom`, truncating.
    pub fn scale(&mut self, numer: i32, denom: i32) {
        for v in &mut self.values {
            if *v != UNREACHED {
                *v = ((*v as i64) * numer as i64 / denom as i64) as i32;
            }
        }
    }

    fn relax(&mut self, source: &impl CostSource, rules: PathRules, mut order: u64) {
        while let Some(Reverse((dist, _, local))) = self.heap.pop() {
            let local = local as usize;
            if dist > self.values[local] {
                continue;
            }
            let here = self.point(local);
            for dir in rules.directions() {
                let Some((next, cost)) = step_cost(source, here, *dir, rules) else {
                    continue;
                };
                let Some(n) = self.local(next) else { continue };
                let nd = dist.saturating_add(cost as i32);
                if nd < self.values[n] {
                    self.values[n] = nd;
                    order += 1;
                    self.heap.push(Reverse((nd, order, n as u32)));
                }
            }
        }
    }

    /// The neighbour of `p` with the lowest value strictly below `p`'s own,
    /// or `None` if `p` is a goal, unreached, or boxed in.
    ///
    /// Ties break clockwise from north, so the same situation gives the
    /// same step every run. Occupancy is not consulted: a blocked step is
    /// the caller's to refuse, after which it may try the next best.
    pub fn descend(&self, p: Point) -> Option<Point> {
        self.best_neighbour(p, |a, b| a < b)
    }

    /// The neighbour with the highest value strictly above `p`'s own.
    pub fn ascend(&self, p: Point) -> Option<Point> {
        self.best_neighbour(p, |a, b| a > b)
    }

    /// Every neighbour of `p` with a value strictly below `p`'s own, best
    /// first, for callers that need a fallback when the best step is
    /// blocked.
    pub fn descents(&self, p: Point) -> Vec<Point> {
        let Some(here) = self.value(p) else { return Vec::new() };
        let mut out: Vec<(i32, usize, Point)> = Direction::ALL
            .iter()
            .enumerate()
            .filter_map(|(i, d)| {
                let n = p + d.offset();
                self.value(n).filter(|v| *v < here).map(|v| (v, i, n))
            })
            .collect();
        out.sort();
        out.into_iter().map(|(_, _, n)| n).collect()
    }

    fn best_neighbour(&self, p: Point, better: impl Fn(i32, i32) -> bool) -> Option<Point> {
        let here = self.value(p)?;
        let mut best: Option<(i32, Point)> = None;
        for d in Direction::ALL {
            let n = p + d.offset();
            let Some(v) = self.value(n) else { continue };
            if !better(v, here) {
                continue;
            }
            match best {
                Some((bv, _)) if !better(v, bv) => {}
                _ => best = Some((v, n)),
            }
        }
        best.map(|(_, n)| n)
    }

    /// Every reached cell with its value, row-major within the region.
    pub fn iter(&self) -> impl Iterator<Item = (Point, i32)> + '_ {
        self.values
            .iter()
            .enumerate()
            .filter(|(_, v)| **v != UNREACHED)
            .map(|(i, v)| (self.point(i), *v))
    }

    /// The reached cell with the lowest value, lowest point on a tie.
    pub fn minimum(&self) -> Option<(Point, i32)> {
        self.iter().min_by_key(|(p, v)| (*v, *p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::astar::AStar;
    use crate::terrain::fixtures::parse;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn p(x: i32, y: i32) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn values_grow_away_from_the_goal_and_stop_at_walls() {
        let (t, r) = parse(&["....#", "....#", "....#"]);
        let view = t.view(&r);
        let mut map = DijkstraMap::covering(&view);
        map.build(&view, [p(0, 1)], PathRules::default());
        assert_eq!(map.value(p(0, 1)), Some(0));
        assert_eq!(map.value(p(1, 1)), Some(100));
        assert_eq!(map.value(p(1, 0)), Some(141));
        assert_eq!(map.value(p(3, 1)), Some(300));
        assert_eq!(map.value(p(4, 1)), None);
        assert_eq!(map.value(p(9, 9)), None);
    }

    #[test]
    fn descending_reaches_the_goal_and_stops_there() {
        let (t, r) = parse(&["........", "..####..", "........"]);
        let view = t.view(&r);
        let mut map = DijkstraMap::covering(&view);
        map.build(&view, [p(7, 2)], PathRules::default());
        let mut here = p(0, 0);
        let mut steps = 0;
        while let Some(next) = map.descend(here) {
            here = next;
            steps += 1;
            assert!(steps < 50);
        }
        assert_eq!(here, p(7, 2));
        assert_eq!(map.descend(p(7, 2)), None);
    }

    #[test]
    fn multiple_goals_and_weights() {
        let (t, r) = parse(&["........"]);
        let view = t.view(&r);
        let mut map = DijkstraMap::covering(&view);
        map.build(&view, [p(0, 0), p(7, 0)], PathRules::default());
        assert_eq!(map.value(p(3, 0)), Some(300));
        assert_eq!(map.value(p(5, 0)), Some(200));
        map.build_weighted(&view, [(p(0, 0), 0), (p(7, 0), -500)], PathRules::default());
        assert_eq!(map.descend(p(3, 0)), Some(p(4, 0)), "the sweeter goal pulls harder");
    }

    #[test]
    fn a_region_bounds_the_flood() {
        let rows = vec![".".repeat(20); 20];
        let (t, r) = parse(&rows.iter().map(String::as_str).collect::<Vec<_>>());
        let view = t.view(&r);
        let mut map = DijkstraMap::new(Rect::new(5, 5, 6, 6));
        map.build(&view, [p(7, 7)], PathRules::default());
        assert_eq!(map.value(p(7, 7)), Some(0));
        assert!(map.value(p(10, 10)).is_some());
        assert_eq!(map.value(p(11, 11)), None);
        assert_eq!(map.value(p(0, 0)), None);
        assert_eq!(map.iter().count(), 36);
        map.reset(Rect::new(0, 0, 2, 2));
        assert_eq!(map.iter().count(), 0);
    }

    #[test]
    fn a_safety_map_leads_away_and_around() {
        // Fleeing from the goal at the corridor mouth should lead into the
        // far room, not into the dead end beside the threat.
        let (t, r) = parse(&["#########", "#...#...#", "#...#...#", "#.......#", "#########"]);
        let view = t.view(&r);
        let mut map = DijkstraMap::covering(&view);
        map.build(&view, [p(1, 1)], PathRules::default());
        map.scale(-12, 10);
        map.rescan(&view, PathRules::default());
        let (far, _) = map.minimum().unwrap();
        assert!(far.x >= 5, "safest point should be in the far room, got {far:?}");
        let mut here = p(2, 2);
        for _ in 0..30 {
            match map.descend(here) {
                Some(n) => here = n,
                None => break,
            }
        }
        assert!(here.x >= 5, "flight ended at {here:?}");
    }

    /// The two searches must agree: the cost A* finds from any start equals
    /// the Dijkstra value at that start when the goal is the same.
    #[test]
    fn astar_and_dijkstra_agree_on_every_reachable_cost() {
        let registry = crate::tile::TileRegistry::standard();
        let (wall, floor) = (registry.expect("wall"), registry.expect("floor"));
        let mut astar = AStar::new();
        for seed in 0..10u64 {
            let mut rng = StdRng::seed_from_u64(seed);
            let terrain = crate::terrain::Terrain::from_fn(20, 15, |_| {
                if rng.random_range(0..100) < 30 { wall } else { floor }
            });
            let view = terrain.view(&registry);
            let goal = (0..)
                .map(|_| p(rng.random_range(0..20), rng.random_range(0..15)))
                .find(|g| view.is_passable(*g))
                .unwrap();
            let mut map = DijkstraMap::covering(&view);
            map.build(&view, [goal], PathRules::default());
            for (start, _) in terrain.iter() {
                if !view.is_passable(start) {
                    continue;
                }
                let found = astar.find(&view, start, goal, PathRules::default()).map(|path| path.cost as i32);
                assert_eq!(found, map.value(start), "seed {seed} start {start:?}");
            }
        }
    }

    #[test]
    fn descents_lists_every_downhill_option_best_first() {
        let (t, r) = parse(&["...", "...", "..."]);
        let view = t.view(&r);
        let mut map = DijkstraMap::covering(&view);
        map.build(&view, [p(2, 2)], PathRules::default());
        let opts = map.descents(p(0, 0));
        assert_eq!(opts[0], p(1, 1));
        assert_eq!(opts.len(), 3);
        assert!(map.descents(p(2, 2)).is_empty());
        assert_eq!(map.ascend(p(2, 2)), Some(p(1, 1)));
    }
}
