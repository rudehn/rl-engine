//! Rivers and lakes, by priority flood.
//!
//! Every land cell is given a downstream neighbour such that following
//! downstream always reaches the sea. Depressions are filled to their spill
//! point and become lakes. Rain then flows downhill and accumulates; the
//! cells carrying the most flow are rivers, chosen by proportion of land so
//! every world has the same share of river, not the same threshold.
//!
//! Rivers are stored the same way roads are, as a [`DirectionSet`] per cell
//! saying which ways water enters and leaves. That is what a chunk reads to
//! know which edges its channel crosses. Flow is four-connected so a
//! crossing is always on an edge, never a corner: a corner belongs to four
//! chunks, and only an edge can be agreed between two.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use rl_core::stats::quantile;
use rl_core::{Direction, DirectionSet, Grid, Grid2D, Point, Steps};
use rl_grid::BitGrid;

use crate::elevation::Elevation;

/// Tunable knobs for hydrology.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HydrologyConfig {
    /// Share of land cells that carry a river.
    pub river_fraction: f32,
    /// Share of land cells that are lake. The deepest-filled depressions
    /// become lakes first, so a world with no basins has no lakes.
    pub lake_fraction: f32,
    /// A depression shallower than this never counts as a lake, however
    /// few basins the world has. In normalized height units.
    pub min_lake_depth: f32,
    /// Number of river width classes.
    pub width_classes: u8,
}

impl Default for HydrologyConfig {
    fn default() -> Self {
        Self {
            river_fraction: 0.035,
            lake_fraction: 0.02,
            min_lake_depth: 0.002,
            width_classes: 3,
        }
    }
}

/// Where the water goes.
#[derive(Debug, Clone)]
pub struct Hydrology {
    /// The neighbour each land cell drains into. `None` for sea cells.
    pub downstream: Grid<Option<Direction>>,
    /// Height after depressions are filled to their spill point.
    pub filled: Grid<f32>,
    /// Upstream area draining through each cell, in cells.
    pub accumulation: Grid<f32>,
    /// Which ways a river enters and leaves each cell. Empty where none.
    pub rivers: Grid<DirectionSet>,
    /// River width class, 1 to `width_classes`; 0 where there is no river.
    pub width: Grid<u8>,
    /// Filled depressions.
    pub lakes: BitGrid,
}

impl Hydrology {
    /// Builds hydrology for a height map.
    pub fn generate(elevation: &Elevation, config: &HydrologyConfig) -> Self {
        let height = &elevation.height;
        let (w, h) = (height.width(), height.height());
        let cells = height.len();

        // Priority flood: pop the lowest unprocessed cell, raise its
        // unprocessed neighbours to at least its own filled height, and point
        // them at it. Every land cell ends up draining monotonically to sea.
        let mut filled = height.clone();
        let mut downstream: Grid<Option<Direction>> = Grid::filled(w, h, None);
        let mut processed = BitGrid::new(w, h);
        let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
        for idx in 0..cells {
            if elevation.is_water_idx(idx) {
                processed.insert_idx(idx);
                heap.push(Reverse((height[idx].to_bits(), idx)));
            }
        }
        const STEP: f32 = 1e-6;
        while let Some(Reverse((_, idx))) = heap.pop() {
            let here = height.idx_point(idx);
            for d in Direction::CARDINALS {
                let n = here + d.offset();
                let Some(nidx) = height.checked_idx(n) else { continue };
                if processed.get_idx(nidx) {
                    continue;
                }
                processed.insert_idx(nidx);
                let raised = height[nidx].max(filled[idx] + STEP);
                filled[nidx] = raised;
                downstream[nidx] = Some(d.opposite());
                heap.push(Reverse((raised.to_bits(), nidx)));
            }
        }

        // Lakes are the deepest-filled share of land, cut by proportion so
        // the same fraction of every world is lake and only the seed decides
        // where. A floor keeps a world of shallow dimples lake-free.
        let depths: Vec<f32> = (0..cells)
            .filter(|i| !elevation.is_water_idx(*i))
            .map(|i| filled[i] - height[i])
            .collect();
        let lake_depth = quantile(&depths, 1.0 - config.lake_fraction).max(config.min_lake_depth);
        let mut lakes = BitGrid::new(w, h);
        for idx in 0..cells {
            if !elevation.is_water_idx(idx) && filled[idx] - height[idx] > lake_depth {
                lakes.insert_idx(idx);
            }
        }

        // Rain: one unit per land cell, passed downstream in decreasing
        // filled order so every contributor is summed before its receiver.
        let mut order: Vec<usize> = (0..cells).filter(|i| !elevation.is_water_idx(*i)).collect();
        order.sort_by(|a, b| filled[*b].total_cmp(&filled[*a]).then(a.cmp(b)));
        let mut accumulation = Grid::filled(w, h, 0.0f32);
        for idx in order {
            accumulation[idx] += 1.0;
            if let Some(d) = downstream[idx] {
                let n = height.idx_point(idx) + d.offset();
                let nidx = height.point_idx(n);
                let carried = accumulation[idx];
                accumulation[nidx] += carried;
            }
        }

        // Rivers are the top slice of land by accumulation, lakes excluded.
        let land_flow: Vec<f32> = (0..cells)
            .filter(|i| !elevation.is_water_idx(*i) && !lakes.get_idx(*i))
            .map(|i| accumulation[i])
            .collect();
        let threshold = quantile(&land_flow, 1.0 - config.river_fraction).max(2.0);
        let is_river = |i: usize| !elevation.is_water_idx(i) && !lakes.get_idx(i) && accumulation[i] >= threshold;

        let mut rivers = Grid::filled(w, h, DirectionSet::NONE);
        for idx in 0..cells {
            if !is_river(idx) {
                continue;
            }
            let Some(d) = downstream[idx] else { continue };
            rivers[idx].insert(d);
            let n = height.idx_point(idx) + d.offset();
            let nidx = height.point_idx(n);
            if is_river(nidx) {
                rivers[nidx].insert(d.opposite());
            }
        }

        let river_flow: Vec<f32> = (0..cells).filter(|i| is_river(*i)).map(|i| accumulation[i]).collect();
        let classes = config.width_classes.max(1);
        let cuts: Vec<f32> = (1..classes).map(|c| quantile(&river_flow, c as f32 / classes as f32)).collect();
        let width = Grid::from_fn(w, h, |p| {
            let i = height.point_idx(p);
            if !is_river(i) {
                return 0;
            }
            1 + cuts.iter().filter(|c| accumulation[i] >= **c).count() as u8
        });

        Self {
            downstream,
            filled,
            accumulation,
            rivers,
            width,
            lakes,
        }
    }

    /// Whether a river runs through `p`.
    pub fn is_river(&self, p: Point) -> bool {
        self.rivers.get(p).is_some_and(|s| !s.is_empty())
    }

    /// Whether `p` is a lake cell.
    pub fn is_lake(&self, p: Point) -> bool {
        self.lakes.contains(p)
    }

    /// Whether `p` is lake or river.
    pub fn is_fresh_water(&self, p: Point) -> bool {
        self.is_lake(p) || self.is_river(p)
    }

    /// How many cells carry a river.
    pub fn river_length(&self) -> usize {
        self.rivers.cells().iter().filter(|s| !s.is_empty()).count()
    }

    /// The neighbours water reaches `p` from.
    pub fn upstream_of(&self, p: Point) -> impl Iterator<Item = Point> + '_ {
        self.downstream.neighbours(p, Steps::Eight).filter(move |n| {
            self.downstream[*n].is_some_and(|d| *n + d.offset() == p)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elevation::ElevationConfig;
    use rl_core::RunSeed;

    fn world(seed: u64) -> (Elevation, Hydrology) {
        let e = Elevation::generate(96, 64, RunSeed(seed), &ElevationConfig::default());
        let h = Hydrology::generate(&e, &HydrologyConfig::default());
        (e, h)
    }

    #[test]
    fn every_land_cell_drains_to_the_sea_without_looping() {
        for seed in 1..=4 {
            let (e, h) = world(seed);
            for (p, _) in e.height.iter() {
                if e.is_water(p) {
                    assert_eq!(h.downstream[p], None);
                    continue;
                }
                let mut here = p;
                let mut steps = 0;
                while let Some(d) = h.downstream[here] {
                    let next = here + d.offset();
                    assert!(h.filled[next] < h.filled[here], "seed {seed}: uphill at {here:?}");
                    here = next;
                    steps += 1;
                    assert!(steps < 10_000, "seed {seed}: loop from {p:?}");
                }
                assert!(e.is_water(here), "seed {seed}: {p:?} drains to land {here:?}");
            }
        }
    }

    #[test]
    fn river_share_matches_the_config_and_rivers_are_connected() {
        for seed in 1..=4 {
            let (e, h) = world(seed);
            let land = (0..e.height.len()).filter(|i| !e.is_water_idx(*i)).count();
            let share = h.river_length() as f32 / land as f32;
            assert!((share - 0.035).abs() < 0.015, "seed {seed}: river share {share}");
            for (p, set) in h.rivers.iter() {
                for d in set.iter() {
                    let n = p + d.offset();
                    let downstream_here = h.downstream[p] == Some(d);
                    if downstream_here {
                        assert!(e.is_water(n) || h.is_lake(n) || h.rivers[n].contains(d.opposite()), "seed {seed}: {p:?} flows {d:?} into nothing");
                    } else {
                        assert!(h.rivers[n].contains(d.opposite()), "seed {seed}: {p:?} has an upstream link {d:?} nobody claims");
                    }
                }
            }
        }
    }

    #[test]
    fn widths_grow_downstream_and_lakes_are_land_depressions() {
        let (e, h) = world(2);
        for (p, set) in h.rivers.iter() {
            if set.is_empty() {
                continue;
            }
            assert!(h.width[p] >= 1 && h.width[p] <= 3);
            if let Some(d) = h.downstream[p] {
                let n = p + d.offset();
                if h.is_river(n) {
                    assert!(h.width[n] >= h.width[p], "{p:?} narrows downstream");
                }
            }
        }
        for p in h.lakes.iter() {
            assert!(!e.is_water(p));
            assert!(h.filled[p] > e.height[p]);
        }
    }

    #[test]
    fn lake_share_follows_the_config_across_seeds() {
        for seed in 1..=6 {
            let (e, h) = world(seed);
            let land = (0..e.height.len()).filter(|i| !e.is_water_idx(*i)).count() as f32;
            let share = h.lakes.count() as f32 / land;
            assert!(share <= 0.025, "seed {seed}: lake share {share}");
        }
    }
}
