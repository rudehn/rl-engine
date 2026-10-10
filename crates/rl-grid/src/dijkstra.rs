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
    /// The rules the last flood stepped by, so descending takes only the
    /// steps it took.
    rules: PathRules,
    /// Whether a diagonal may pass beside each cell of the region, as the
    /// source said when the flood began, so descending asks the corner rule
    /// the flood stepped by without the source to hand. A cell's having a
    /// value is not the same answer either way: a source that prices a
    /// closed door reaches it and still lets nothing past its corner, and
    /// one that refuses a cell something stands in may still let a
    /// diagonal past it, as the move resolver does. When only the cells
    /// reached were recorded, the second was a diagonal the flood stepped
    /// and descending refused, and a mover whose cheapest way was that
    /// diagonal stood where it was for ever.
    past: Vec<bool>,
}

impl DijkstraMap {
    /// An unreached map over `region`.
    pub fn new(region: Rect) -> Self {
        let cells = region.area().max(0) as usize;
        Self { region, values: vec![UNREACHED; cells], heap: BinaryHeap::new(), rules: PathRules::default(), past: vec![false; cells] }
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
        self.past.clear();
        self.past.resize(cells, false);
        self.heap.clear();
    }

    fn local(&self, p: Point) -> Option<usize> {
        self.region.contains(p).then(|| ((p.y - self.region.y) * self.region.width + (p.x - self.region.x)) as usize)
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
        self.past.iter_mut().for_each(|p| *p = false);
        self.heap.clear();
        self.rules = rules;
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
        self.rules = rules;
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
        // Every cell, reached or not: the two a diagonal squeezes between
        // lie inside the rectangle of its ends, so inside the region.
        for local in 0..self.past.len() {
            self.past[local] = source.lets_diagonal_past(self.point(local));
        }
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
    ///
    /// Only a step the flood itself would take is offered. A diagonal past
    /// a wall's corner is often lower than going round, and a mover handed
    /// it is refused by the same corner rule every turn and never moves.
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
            .filter(|(_, d)| self.steps(p, **d))
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
        for d in Direction::ALL.into_iter().filter(|d| self.steps(p, *d)) {
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

    /// Whether the flood could have stepped from `p` toward `dir`.
    ///
    /// Asked of what the source said of each side as the flood began, so
    /// it is the flood's own corner test, asked without the terrain it was
    /// built over; a side outside the region lets nothing past.
    fn steps(&self, p: Point, dir: Direction) -> bool {
        if !dir.is_diagonal() {
            return true;
        }
        if !self.rules.diagonals {
            return false;
        }
        let (dx, dy) = dir.delta();
        let past = |q: Point| self.local(q).is_some_and(|i| self.past[i]);
        self.rules.cut_corners || (past(p.offset(dx, 0)) && past(p.offset(0, dy)))
    }

    /// Copies the values of a map of the same shape, whatever its region
    /// is, so a map can be re-addressed in another coordinate space.
    ///
    /// # Panics
    /// Panics if the shapes differ.
    pub fn copy_values_from(&mut self, other: &DijkstraMap) {
        assert_eq!(self.values.len(), other.values.len(), "maps differ in shape");
        self.values.copy_from_slice(&other.values);
        self.past.copy_from_slice(&other.past);
        self.rules = other.rules;
    }

    /// Lowers every cell to `other`'s value where that is the lower, and
    /// reaches what only `other` reached.
    ///
    /// A flood from several goals is, cell for cell, the lowest of the
    /// floods from each of them, so a caller that keeps one flood per goal
    /// can make the map toward any set of them without flooding again.
    /// Both must have stepped by the same rules for that to hold.
    ///
    /// # Panics
    /// Panics if the shapes differ.
    pub fn take_lower(&mut self, other: &DijkstraMap) {
        assert_eq!(self.values.len(), other.values.len(), "maps differ in shape");
        for (mine, theirs) in self.values.iter_mut().zip(&other.values) {
            *mine = (*mine).min(*theirs);
        }
        // What a cell lets past is the source's, not the flood's, so two
        // floods over one source say the same of every cell, and a map not
        // yet flooded, which says nothing lets a diagonal past, takes the
        // other's word.
        for (mine, theirs) in self.past.iter_mut().zip(&other.past) {
            *mine |= *theirs;
        }
    }

    /// Every reached cell with its value, row-major within the region.
    pub fn iter(&self) -> impl Iterator<Item = (Point, i32)> + '_ {
        self.values.iter().enumerate().filter(|(_, v)| **v != UNREACHED).map(|(i, v)| (self.point(i), *v))
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
    fn over_a_seed_range_a_flood_from_several_goals_is_the_lowest_of_the_floods_from_each() {
        let registry = crate::tile::TileRegistry::standard();
        let (wall, floor) = (registry.expect("wall"), registry.expect("floor"));
        for rules in [PathRules::default(), PathRules::CARDINAL] {
            for seed in 0..20u64 {
                let mut rng = StdRng::seed_from_u64(seed);
                let terrain = crate::terrain::Terrain::from_fn(20, 15, |_| if rng.random_range(0..100) < 35 { wall } else { floor });
                let view = terrain.view(&registry);
                let goals: Vec<Point> = (0..rng.random_range(2..5)).map(|_| p(rng.random_range(0..20), rng.random_range(0..15))).collect();
                let mut together = DijkstraMap::covering(&view);
                together.build(&view, goals.iter().copied(), rules);
                let mut lowest: Option<DijkstraMap> = None;
                for goal in &goals {
                    let mut one = DijkstraMap::covering(&view);
                    one.build(&view, [*goal], rules);
                    match lowest.as_mut() {
                        Some(lowest) => lowest.take_lower(&one),
                        None => lowest = Some(one),
                    }
                }
                let lowest = lowest.unwrap();
                for cell in (0..15).flat_map(|y| (0..20).map(move |x| p(x, y))) {
                    assert_eq!(lowest.value(cell), together.value(cell), "seed {seed}: {cell:?} under {rules:?}");
                    assert_eq!(lowest.descents(cell), together.descents(cell), "seed {seed}: the ways down from {cell:?} under {rules:?}");
                }
            }
        }
    }

    /// Floor everywhere, but `shut` costs its opening and lets nothing
    /// past its corner, as a closed door read by a mover that opens doors.
    struct Shut<'a> {
        view: crate::terrain::TerrainView<'a>,
        shut: Point,
    }

    impl Grid2D for Shut<'_> {
        fn width(&self) -> i32 {
            self.view.width()
        }

        fn height(&self) -> i32 {
            self.view.height()
        }
    }

    impl CostSource for Shut<'_> {
        fn cost_idx(&self, idx: usize) -> Option<u32> {
            if self.idx_point(idx) == self.shut { Some(200) } else { self.view.cost_idx(idx) }
        }

        fn lets_diagonal_past(&self, p: Point) -> bool {
            p != self.shut && self.is_passable(p)
        }
    }

    #[test]
    fn a_cell_that_lets_no_diagonal_past_is_gone_round_by_the_flood_and_the_search_alike() {
        // From (1, 1) the goal (2, 0) is one diagonal away, squeezing past
        // the shut cell (2, 1), so the way is north and then east.
        let (t, r) = parse(&["....", "....", "...."]);
        let source = Shut { view: t.view(&r), shut: p(2, 1) };
        let mut map = DijkstraMap::covering(&source);
        map.build(&source, [p(2, 0)], PathRules::default());
        assert_eq!(map.value(p(1, 1)), Some(200), "the diagonal is not a step the flood takes");
        assert_eq!(map.descents(p(1, 1)), vec![p(1, 0)], "nor one descending offers");
        let path = AStar::new().find(&source, p(1, 1), p(2, 0), PathRules::default()).expect("a way round");
        assert_eq!(path.steps, vec![p(1, 0), p(2, 0)], "and the search goes round it too");
    }

    /// What one cell of a [`Mixed`] source is: how it is entered and
    /// whether a diagonal passes beside it.
    #[derive(Clone, Copy)]
    enum Cell {
        /// Entered at a plain step, passed beside.
        Floor,
        /// Neither entered nor passed beside.
        Wall,
        /// Entered at its opening, never passed beside: a closed door.
        Shut,
        /// Never entered, passed beside: a thing standing in the way,
        /// whose corner the move resolver lets a diagonal past.
        Standing,
    }

    /// A source of cells each answering both questions its own way.
    struct Mixed {
        width: i32,
        cells: Vec<Cell>,
    }

    impl Mixed {
        fn parse(rows: &[&str]) -> Self {
            let cells = rows
                .iter()
                .flat_map(|r| r.chars())
                .map(|c| match c {
                    '#' => Cell::Wall,
                    '+' => Cell::Shut,
                    'o' => Cell::Standing,
                    _ => Cell::Floor,
                })
                .collect();
            Self { width: rows[0].len() as i32, cells }
        }

        fn random(width: i32, height: i32, rng: &mut impl Rng) -> Self {
            let cells = (0..width * height)
                .map(|_| match rng.random_range(0..100) {
                    0..15 => Cell::Wall,
                    15..20 => Cell::Shut,
                    20..40 => Cell::Standing,
                    _ => Cell::Floor,
                })
                .collect();
            Self { width, cells }
        }
    }

    impl Grid2D for Mixed {
        fn width(&self) -> i32 {
            self.width
        }

        fn height(&self) -> i32 {
            self.cells.len() as i32 / self.width
        }
    }

    impl CostSource for Mixed {
        fn cost_idx(&self, idx: usize) -> Option<u32> {
            match self.cells[idx] {
                Cell::Floor => Some(100),
                Cell::Shut => Some(200),
                Cell::Wall | Cell::Standing => None,
            }
        }

        fn lets_diagonal_past(&self, p: Point) -> bool {
            self.checked_idx(p).is_some_and(|i| matches!(self.cells[i], Cell::Floor | Cell::Standing))
        }
    }

    #[test]
    fn a_diagonal_past_two_things_that_cannot_be_entered_is_offered_by_descending_as_the_flood_took_it() {
        // The goal (1, 0) is one diagonal from (0, 1), past two things
        // standing in the way that a diagonal may pass beside. The flood
        // steps it; descending once refused it, since neither was reached,
        // and a mover standing at (0, 1) waited there for ever.
        let source = Mixed::parse(&["o.", ".o"]);
        let mut map = DijkstraMap::covering(&source);
        map.build(&source, [p(1, 0)], PathRules::default());
        assert_eq!(map.value(p(0, 1)), Some(141), "the flood takes the diagonal");
        assert_eq!(map.descend(p(0, 1)), Some(p(1, 0)), "and descending offers it");
    }

    #[test]
    fn over_a_seed_range_every_cell_the_flood_reaches_but_a_goal_has_a_step_down_the_flood_itself_would_take() {
        for rules in [PathRules::default(), PathRules::CARDINAL, PathRules { diagonals: true, cut_corners: true }] {
            for seed in 0..40u64 {
                let mut rng = StdRng::seed_from_u64(seed);
                let source = Mixed::random(20, 15, &mut rng);
                let goals: Vec<Point> = (0..rng.random_range(1..4)).map(|_| p(rng.random_range(0..20), rng.random_range(0..15))).collect();
                // A region short of the whole grid as well, so a flood
                // bounded to part of a map is held to the same.
                for region in [source.bounds(), Rect::new(2, 1, 15, 12)] {
                    let mut map = DijkstraMap::new(region);
                    map.build(&source, goals.iter().copied(), rules);
                    for (from, value) in map.iter().collect::<Vec<_>>() {
                        if value == 0 {
                            continue;
                        }
                        let down = map.descend(from).unwrap_or_else(|| panic!("seed {seed}: {from:?} at {value} has no step down under {rules:?}"));
                        let dir = Direction::between(from, down).expect("a neighbour");
                        assert!(step_cost(&source, from, dir, rules).is_some(), "seed {seed}: {from:?} to {down:?} is not a step under {rules:?}");
                        assert!(map.value(down) < Some(value), "seed {seed}: {from:?} to {down:?} is not closer");
                    }
                }
            }
        }
    }

    #[test]
    fn over_a_seed_range_every_descent_is_a_step_the_flood_itself_would_take() {
        let registry = crate::tile::TileRegistry::standard();
        let (wall, floor) = (registry.expect("wall"), registry.expect("floor"));
        for rules in [PathRules::default(), PathRules::CARDINAL] {
            for seed in 0..20u64 {
                let mut rng = StdRng::seed_from_u64(seed);
                let terrain = crate::terrain::Terrain::from_fn(20, 15, |_| if rng.random_range(0..100) < 35 { wall } else { floor });
                let view = terrain.view(&registry);
                let goal = (0..).map(|_| p(rng.random_range(0..20), rng.random_range(0..15))).find(|g| view.is_passable(*g)).unwrap();
                let mut map = DijkstraMap::covering(&view);
                map.build(&view, [goal], rules);
                for (from, _) in map.iter().collect::<Vec<_>>() {
                    for to in map.descents(from) {
                        let dir = Direction::between(from, to).expect("a neighbour");
                        assert!(step_cost(&view, from, dir, rules).is_some(), "seed {seed}: {from:?} to {to:?} is not a step under {rules:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn descending_round_a_wall_corner_goes_round_it_rather_than_through() {
        // From the far side of the corner at (1, 0), the diagonal onto the
        // row below is lower than going straight down, and is not a step.
        let (t, r) = parse(&[".#.", "..."]);
        let view = t.view(&r);
        let mut map = DijkstraMap::covering(&view);
        map.build(&view, [p(0, 1)], PathRules::default());
        assert_eq!(map.descend(p(2, 0)), Some(p(2, 1)), "straight down, since the diagonal squeezes past the wall");
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
            let terrain = crate::terrain::Terrain::from_fn(20, 15, |_| if rng.random_range(0..100) < 30 { wall } else { floor });
            let view = terrain.view(&registry);
            let goal = (0..).map(|_| p(rng.random_range(0..20), rng.random_range(0..15))).find(|g| view.is_passable(*g)).unwrap();
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
