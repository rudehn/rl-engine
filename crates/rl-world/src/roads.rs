//! The roads between sites.
//!
//! The first layer that is structure rather than terrain. Three ideas do
//! the work: slope, not height, decides where a road goes; roads prefer
//! existing roads, so later routes snake onto earlier ones and meet at
//! junctions; and a spanning tree is not a network, so shortcuts are added
//! wherever the tree makes a journey disproportionately long.
//!
//! The search runs over (cell, entry direction) with a quadratic turn cost,
//! which is why roads read as built rather than found. Costs are integer
//! fixed point so two runs cannot disagree over a float compare. Steps are
//! four-connected at region scale so every crossing between two regions is
//! on a shared edge, where the two chunks can agree on it; a corner belongs
//! to four chunks and cannot be agreed between two.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use rl_core::stats::quantile;
use rl_core::{Direction, DirectionSet, DisjointSet, Grid, Grid2D, Point, RunSeed, SeedDomain, Steps};

use crate::elevation::Elevation;
use crate::noise::Fbm;
use crate::sites::Site;

/// Tunable knobs for the road network.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoadConfig {
    /// How much a climb of the world's typical steepness adds to a step.
    pub slope_weight: f32,
    /// What a step onto ground already carrying a road costs, as a
    /// fraction of what it would otherwise cost.
    pub reuse_discount: f32,
    /// How much the going varies from place to place, as a fraction.
    pub wander: f32,
    /// Frequency of that variation.
    pub wander_frequency: f64,
    /// Detail levels in it.
    pub wander_octaves: u32,
    /// A pair the network already joins gets a direct road when going round
    /// costs at least this many times going direct.
    pub shortcut_ratio: f32,
    /// Quantile of per-cell climbs taken as the world's typical steepness.
    pub slope_quantile: f32,
    /// What a right-angle bend costs, as a multiple of a flat step.
    pub turn_weight: f32,
}

impl Default for RoadConfig {
    fn default() -> Self {
        Self {
            slope_weight: 9.0,
            reuse_discount: 0.15,
            wander: 0.55,
            wander_frequency: 9.0,
            wander_octaves: 3,
            shortcut_ratio: 1.7,
            slope_quantile: 0.5,
            turn_weight: 2.0,
        }
    }
}

/// A road between two sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    /// Index into the sites list, where the road starts.
    pub from: usize,
    /// Index into the sites list, where it ends.
    pub to: usize,
    /// Every cell the road runs through, inclusive.
    pub cells: Vec<Point>,
    /// The journey cost, comparable within one world.
    pub cost: u32,
}

/// Every road in a world.
#[derive(Debug, Clone)]
pub struct Roads {
    directions: Grid<DirectionSet>,
    routes: Vec<Route>,
}

impl Roads {
    /// A world with no roads.
    pub fn empty(width: i32, height: i32) -> Self {
        Self { directions: Grid::filled(width, height, DirectionSet::NONE), routes: Vec::new() }
    }

    /// Which ways a road leaves `p`. Empty where there is none, or off map.
    pub fn at(&self, p: Point) -> DirectionSet {
        self.directions.get(p).copied().unwrap_or_default()
    }

    /// Every road, trunk roads before shortcuts.
    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    /// The per-cell direction grid.
    pub fn directions(&self) -> &Grid<DirectionSet> {
        &self.directions
    }

    /// How many cells carry a road.
    pub fn length(&self) -> usize {
        self.directions.cells().iter().filter(|s| !s.is_empty()).count()
    }

    /// The sites directly joined to `site` by one road.
    pub fn neighbours(&self, site: usize) -> impl Iterator<Item = usize> + '_ {
        self.routes.iter().filter_map(move |r| {
            if r.from == site {
                Some(r.to)
            } else if r.to == site {
                Some(r.from)
            } else {
                None
            }
        })
    }

    /// Cells between each cell and the nearest road; 0 on a road.
    pub fn distance_field(&self) -> Grid<u16> {
        let g = &self.directions;
        let mut distance = Grid::filled(g.width(), g.height(), u16::MAX);
        let mut queue = VecDeque::new();
        for idx in 0..g.len() {
            if !g[idx].is_empty() {
                distance[idx] = 0;
                queue.push_back(idx);
            }
        }
        while let Some(idx) = queue.pop_front() {
            let next = distance[idx].saturating_add(1);
            for n in g.neighbour_indices(g.idx_point(idx), Steps::Four) {
                if distance[n] > next {
                    distance[n] = next;
                    queue.push_back(n);
                }
            }
        }
        distance
    }

    /// Builds the network joining `sites` over `friction`.
    ///
    /// `friction` is additive per cell on top of a flat step, `None` where
    /// no road can go. The game derives it from its bands.
    pub fn generate(elevation: &Elevation, friction: &Grid<Option<f32>>, sites: &[Site], seed: RunSeed, config: &RoadConfig) -> Self {
        let (width, height) = (friction.width(), friction.height());
        if sites.len() < 2 {
            return Self::empty(width, height);
        }
        let space = *elevation.space();
        let going = Fbm::new(seed.derive(SeedDomain::new(b"roads.wander"), 0), config.wander_octaves).with_frequency(config.wander_frequency);
        let terrain = Terrain {
            elevation,
            friction,
            wander: Grid::from_fn(width, height, |p| {
                let (fx, fy) = space.noise_point_of(p);
                1.0 + (going.get_01(fx, fy) as f32 - 0.5) * 2.0 * config.wander
            }),
            slope_unit: slope_unit(elevation, config),
            config: *config,
        };

        let region = rl_grid::region::label_regions(friction, |i| friction[i].is_some(), Steps::Four);
        let mut candidates: Vec<Candidate> = neighbouring_pairs(sites, &region)
            .into_iter()
            .filter_map(|(a, b)| {
                let (_, cost) = route(&terrain, sites[a].position, sites[b].position, None)?;
                Some(Candidate { a, b, cost })
            })
            .collect();
        candidates.sort_by_key(|c| (c.cost, c.a, c.b));

        let mut groups = DisjointSet::new(sites.len());
        let mut accepted: Vec<Candidate> = Vec::new();
        let mut rejected: Vec<Candidate> = Vec::new();
        for c in candidates {
            if groups.union(c.a, c.b) {
                accepted.push(c);
            } else {
                rejected.push(c);
            }
        }
        for c in rejected {
            let Some(around) = journey(&accepted, sites.len(), c.a, c.b) else { continue };
            if around as f32 >= config.shortcut_ratio * c.cost as f32 {
                accepted.push(c);
            }
        }

        let mut directions = Grid::filled(width, height, DirectionSet::NONE);
        let mut routes = Vec::new();
        for c in accepted {
            let Some((cells, cost)) = route(&terrain, sites[c.a].position, sites[c.b].position, Some(&directions)) else {
                continue;
            };
            for pair in cells.windows(2) {
                let d = Direction::between(pair[0], pair[1]).expect("a route steps one cell at a time");
                directions[pair[0]].insert(d);
                directions[pair[1]].insert(d.opposite());
            }
            routes.push(Route { from: c.a, to: c.b, cells, cost });
        }
        Self { directions, routes }
    }
}

/// How steep an ordinary step is in this world, so a fixed slope cost
/// behaves the same on a flat seed and a mountainous one.
fn slope_unit(elevation: &Elevation, config: &RoadConfig) -> f32 {
    let h = &elevation.height;
    let mut climbs = Vec::new();
    for (p, _) in h.iter() {
        if elevation.is_water(p) {
            continue;
        }
        for d in [Direction::East, Direction::South] {
            let n = p + d.offset();
            if !h.in_bounds(n) || elevation.is_water(n) {
                continue;
            }
            climbs.push((h[n] - h[p]).abs());
        }
    }
    quantile(&climbs, config.slope_quantile).max(f32::EPSILON)
}

const SCALE: f32 = 256.0;

struct Terrain<'a> {
    elevation: &'a Elevation,
    friction: &'a Grid<Option<f32>>,
    wander: Grid<f32>,
    slope_unit: f32,
    config: RoadConfig,
}

impl Terrain<'_> {
    fn step(&self, from: Point, to: Point, roads: Option<&Grid<DirectionSet>>) -> Option<u32> {
        let friction = (*self.friction.get(to)?)?;
        let climb = (self.elevation.height[to] - self.elevation.height[from]).abs();
        let slope = self.config.slope_weight * (climb / self.slope_unit);
        let mut cost = (1.0 + friction + slope) * self.wander[to] * SCALE;
        if roads.is_some_and(|r| !r[to].is_empty()) {
            cost *= self.config.reuse_discount;
        }
        Some((cost as u32).max(1))
    }

    fn turn(&self, from: Direction, to: Direction) -> u32 {
        let eighths = turn_between(from, to);
        (self.config.turn_weight * (eighths * eighths) as f32 * SCALE) as u32
    }

    fn floor(&self, reuse: bool) -> f32 {
        let wander = self.wander.cells().iter().copied().fold(f32::INFINITY, f32::min);
        let discount = if reuse { self.config.reuse_discount } else { 1.0 };
        (wander * discount * SCALE).max(1.0)
    }
}

fn turn_between(from: Direction, to: Direction) -> u32 {
    let (a, b) = (from.index() as i32, to.index() as i32);
    (a - b).rem_euclid(8).min((b - a).rem_euclid(8)) as u32
}

/// A* over (cell, entry direction) with a Manhattan heuristic scaled by the
/// cheapest admissible step.
fn route(terrain: &Terrain, from: Point, to: Point, roads: Option<&Grid<DirectionSet>>) -> Option<(Vec<Point>, u32)> {
    let (width, height) = (terrain.friction.width(), terrain.friction.height());
    let mut best = Grid::filled(width, height, [u32::MAX; 8]);
    let mut came_from = Grid::filled(width, height, [u8::MAX; 8]);
    let mut done = Grid::filled(width, height, [false; 8]);
    let floor = terrain.floor(roads.is_some());
    let guess = |p: Point| (rl_core::geometry::manhattan(p, to) as f32 * floor) as u32;

    let mut queue = BinaryHeap::new();
    for facing in Direction::ALL {
        best[from][facing.index()] = 0;
        queue.push(Reverse((guess(from), 0u32, from, facing.index() as u8)));
    }
    while let Some(Reverse((_, cost, at, facing))) = queue.pop() {
        if at == to {
            let mut cells = vec![at];
            let mut step = (at, facing);
            loop {
                let prev_dir = came_from[step.0][step.1 as usize];
                if prev_dir == u8::MAX {
                    break;
                }
                // The parent is one step back along the direction we arrived by.
                let arrived_by = Direction::from_index(step.1 as usize);
                let prev = step.0 + arrived_by.opposite().offset();
                cells.push(prev);
                step = (prev, prev_dir);
            }
            cells.reverse();
            return Some((cells, cost));
        }
        if done[at][facing as usize] {
            continue;
        }
        done[at][facing as usize] = true;
        let heading = Direction::from_index(facing as usize);
        for d in Direction::CARDINALS {
            let next = at + d.offset();
            if !terrain.friction.in_bounds(next) {
                continue;
            }
            let Some(step) = terrain.step(at, next, roads) else { continue };
            let total = cost.saturating_add(step).saturating_add(terrain.turn(heading, d));
            let slot = d.index();
            if total < best[next][slot] {
                best[next][slot] = total;
                came_from[next][slot] = facing;
                queue.push(Reverse((total.saturating_add(guess(next)), total, next, slot as u8)));
            }
        }
    }
    None
}

struct Candidate {
    a: usize,
    b: usize,
    cost: u32,
}

/// The relative neighbourhood graph: a pair is kept unless a third site in
/// the same region lies nearer to both than they are to each other.
fn neighbouring_pairs(sites: &[Site], region: &rl_grid::region::Regions) -> Vec<(usize, usize)> {
    let sep = |a: usize, b: usize| rl_core::geometry::euclidean_sq(sites[a].position, sites[b].position) as i64;
    let at = |s: usize| region.label(sites[s].position);
    let mut pairs = Vec::new();
    for a in 0..sites.len() {
        for b in (a + 1)..sites.len() {
            if at(a).is_none() || at(a) != at(b) {
                continue;
            }
            let span = sep(a, b);
            let blocked = (0..sites.len()).filter(|c| *c != a && *c != b && at(*c) == at(a)).any(|c| sep(a, c) < span && sep(b, c) < span);
            if !blocked {
                pairs.push((a, b));
            }
        }
    }
    pairs
}

/// Cheapest journey over the roads accepted so far, Dijkstra on the site graph.
fn journey(accepted: &[Candidate], sites: usize, from: usize, to: usize) -> Option<u32> {
    let mut best = vec![u32::MAX; sites];
    best[from] = 0;
    let mut queue = BinaryHeap::from([Reverse((0u32, from))]);
    while let Some(Reverse((cost, at))) = queue.pop() {
        if at == to {
            return Some(cost);
        }
        if cost > best[at] {
            continue;
        }
        for edge in accepted {
            let next = if edge.a == at {
                edge.b
            } else if edge.b == at {
                edge.a
            } else {
                continue;
            };
            let total = cost.saturating_add(edge.cost);
            if total < best[next] {
                best[next] = total;
                queue.push(Reverse((total, next)));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elevation::ElevationConfig;
    use crate::sites::SiteKindId;

    fn flat_world(seed: u64) -> (Elevation, Grid<Option<f32>>) {
        let e = Elevation::generate(60, 40, RunSeed(seed), &ElevationConfig::default());
        let friction = Grid::from_fn(60, 40, |p| if e.is_water(p) { None } else { Some(0.0) });
        (e, friction)
    }

    fn towns(e: &Elevation, n: usize) -> Vec<Site> {
        let mut sites = Vec::new();
        crate::sites::place_scored(
            &mut sites,
            SiteKindId(1),
            &crate::sites::PlacementRules {
                cells_per_site: 1,
                min: 1,
                max: n as u32,
                jitter: 0.2,
                clearances: vec![crate::sites::Clearance { from: SiteKindId(1), cells: 8 }],
            },
            9,
            60,
            40,
            |p| if e.is_water(p) { 0.0 } else { 1.0 },
        );
        sites
    }

    #[test]
    fn every_town_reachable_by_land_is_joined_and_roads_agree_at_seams() {
        for seed in 1..=3 {
            let (e, friction) = flat_world(seed);
            let sites = towns(&e, 8);
            let roads = Roads::generate(&e, &friction, &sites, RunSeed(seed), &RoadConfig::default());
            let region = rl_grid::region::label_regions(&friction, |i| friction[i].is_some(), Steps::Four);
            let mut joined = DisjointSet::new(sites.len());
            for r in roads.routes() {
                joined.union(r.from, r.to);
                assert_eq!(r.cells.first(), Some(&sites[r.from].position));
                assert_eq!(r.cells.last(), Some(&sites[r.to].position));
                for w in r.cells.windows(2) {
                    assert_eq!(rl_core::geometry::manhattan(w[0], w[1]), 1, "roads step four ways");
                }
            }
            for a in 0..sites.len() {
                for b in 0..sites.len() {
                    if region.connected(sites[a].position, sites[b].position) {
                        assert!(joined.same(a, b), "seed {seed}: towns {a} and {b} share land but no road");
                    }
                }
            }
            for (p, set) in roads.directions().iter() {
                for d in set.iter() {
                    assert!(roads.at(p + d.offset()).contains(d.opposite()), "seam broken at {p:?}");
                }
            }
        }
    }

    #[test]
    fn roads_bend_rather_than_zigzag_and_are_deterministic() {
        let (e, friction) = flat_world(4);
        let sites = towns(&e, 8);
        let a = Roads::generate(&e, &friction, &sites, RunSeed(4), &RoadConfig::default());
        let b = Roads::generate(&e, &friction, &sites, RunSeed(4), &RoadConfig::default());
        assert_eq!(a.directions(), b.directions());
        let mut steps = 0;
        let mut sharp = 0;
        for r in a.routes() {
            for w in r.cells.windows(3) {
                let d1 = Direction::between(w[0], w[1]).unwrap();
                let d2 = Direction::between(w[1], w[2]).unwrap();
                steps += 1;
                if turn_between(d1, d2) >= 2 {
                    sharp += 1;
                }
            }
        }
        assert!(steps > 0);
        // Four-way, so every bend is a right angle; the turn cost should
        // still keep them rare compared with a staircase, which turns on
        // every step.
        assert!((sharp as f32) / (steps as f32) < 0.25, "{sharp} bends in {steps} steps");
    }

    #[test]
    fn distance_field_and_neighbours() {
        let (e, friction) = flat_world(2);
        let sites = towns(&e, 5);
        let roads = Roads::generate(&e, &friction, &sites, RunSeed(2), &RoadConfig::default());
        let field = roads.distance_field();
        for r in roads.routes() {
            for c in &r.cells {
                assert_eq!(field[*c], 0);
            }
            assert!(roads.neighbours(r.from).any(|n| n == r.to));
        }
        assert!(roads.length() > 0);
        assert_eq!(Roads::empty(3, 3).length(), 0);
    }

    #[test]
    fn turn_between_is_the_short_way_round() {
        assert_eq!(turn_between(Direction::North, Direction::North), 0);
        assert_eq!(turn_between(Direction::North, Direction::East), 2);
        assert_eq!(turn_between(Direction::North, Direction::South), 4);
        assert_eq!(turn_between(Direction::NorthWest, Direction::North), 1);
    }
}
