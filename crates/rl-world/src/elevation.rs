//! The height map: the layer everything else reads.
//!
//! Kept as both a coarse grid and the continuous fields it was sampled
//! from, so a chunk can ask for the height of any tile and get a value that
//! agrees with its region's band.

use rl_core::stats::{normalize_in_place, quantile, quantile_of_sorted, smoothstep};
use rl_core::{Grid, Point, RunSeed, SeedDomain};

use crate::noise::{Fbm, SampleSpace};

/// Tunable knobs for the height map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElevationConfig {
    /// Detail levels in the base landmass noise.
    pub continent_octaves: u32,
    /// Base landmass frequency. Higher means more, smaller landmasses.
    pub continent_frequency: f64,
    /// Detail levels in the ridge noise that carves mountain chains.
    pub ridge_octaves: u32,
    /// Ridge frequency. Higher means tighter, more numerous ranges.
    pub ridge_frequency: f64,
    /// How much the ridges lift the terrain they run through.
    pub ridge_strength: f64,
    /// Detail levels in the field that distorts sample positions.
    pub warp_octaves: u32,
    /// Frequency of that field.
    pub warp_frequency: f64,
    /// How far sample positions are distorted, which bends noise into
    /// coastlines with inlets instead of soap-bubble curves.
    pub warp_strength: f64,
    /// Detail levels in the field that wobbles the coast.
    pub coast_octaves: u32,
    /// Frequency of that field.
    pub coast_frequency: f64,
    /// Normalized radius at which the continental shelf starts dropping.
    pub coast_inner_radius: f64,
    /// Normalized radius by which it has dropped to nothing.
    pub coast_outer_radius: f64,
    /// How strongly noise wobbles that radius.
    pub coast_noise_strength: f64,
    /// Cells of guaranteed ocean around the border, so "you cannot walk off
    /// the world" is a property of the generator rather than of a seed.
    pub border_margin: f64,
    /// Share of the map above sea level.
    pub land_fraction: f32,
    /// Share of land that is hilly or higher.
    pub hill_fraction: f32,
    /// Share of land that is mountainous or higher.
    pub mountain_fraction: f32,
    /// Share of land that is bare summit.
    pub peak_fraction: f32,
}

impl Default for ElevationConfig {
    fn default() -> Self {
        Self {
            continent_octaves: 6,
            continent_frequency: 2.1,
            ridge_octaves: 5,
            ridge_frequency: 4.0,
            ridge_strength: 0.9,
            warp_octaves: 3,
            warp_frequency: 1.6,
            warp_strength: 0.28,
            coast_octaves: 4,
            coast_frequency: 2.6,
            coast_inner_radius: 0.52,
            coast_outer_radius: 1.02,
            coast_noise_strength: 0.30,
            border_margin: 3.0,
            land_fraction: 0.36,
            hill_fraction: 0.30,
            mountain_fraction: 0.12,
            peak_fraction: 0.035,
        }
    }
}

/// Where land stops being lowland, in land-height units (0 at the
/// waterline, 1 at the summit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReliefThresholds {
    /// At or above this, land is hills.
    pub hill: f32,
    /// At or above this, land is mountain.
    pub mountain: f32,
    /// At or above this, land is bare summit.
    pub peak: f32,
}

/// The continuous fields the height map is sampled from.
#[derive(Debug, Clone)]
struct Fields {
    continents: Fbm,
    ridges: Fbm,
    warp: Fbm,
    coast: Fbm,
}

/// A height map plus the thresholds that slice it into terrain bands.
#[derive(Debug, Clone)]
pub struct Elevation {
    /// Height per cell, normalized to `[0, 1]` across the whole map.
    pub height: Grid<f32>,
    /// Heights at or below this are water.
    pub sea_level: f32,
    /// Where this world's land turns to hill, mountain and summit.
    pub relief: ReliefThresholds,
    space: SampleSpace,
    fields: Fields,
    config: ElevationConfig,
    raw_min: f32,
    raw_max: f32,
}

impl Elevation {
    /// Builds the height map for a world of `width` by `height` cells.
    pub fn generate(width: i32, height: i32, seed: RunSeed, config: &ElevationConfig) -> Self {
        let space = SampleSpace::new(width, height);
        let d = |name: &[u8]| seed.derive(SeedDomain::new(name), 0);
        let fields = Fields {
            continents: Fbm::new(d(b"elevation.continents"), config.continent_octaves)
                .with_frequency(config.continent_frequency),
            ridges: Fbm::new(d(b"elevation.ridges"), config.ridge_octaves).with_frequency(config.ridge_frequency),
            warp: Fbm::new(d(b"elevation.warp"), config.warp_octaves).with_frequency(config.warp_frequency),
            coast: Fbm::new(d(b"elevation.coast"), config.coast_octaves).with_frequency(config.coast_frequency),
        };

        let mut field = Grid::from_fn(width, height, |p| raw_height(&space, &fields, config, p.x as f64 + 0.5, p.y as f64 + 0.5));
        let raw_min = field.cells().iter().copied().fold(f32::INFINITY, f32::min);
        let raw_max = field.cells().iter().copied().fold(f32::NEG_INFINITY, f32::max);
        normalize_in_place(field.cells_mut());

        let mut sorted = field.cells().to_vec();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let sea_level = quantile_of_sorted(&sorted, 1.0 - config.land_fraction);

        let mut elevation = Self {
            height: field,
            sea_level,
            relief: ReliefThresholds {
                hill: 1.0,
                mountain: 1.0,
                peak: 1.0,
            },
            space,
            fields,
            config: *config,
            raw_min,
            raw_max,
        };
        elevation.relief = elevation.relief_thresholds();
        elevation
    }

    fn relief_thresholds(&self) -> ReliefThresholds {
        let land: Vec<f32> = self
            .height
            .cells()
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.is_water_idx(*i))
            .map(|(_, h)| self.land_height_of(*h))
            .collect();
        ReliefThresholds {
            hill: quantile(&land, 1.0 - self.config.hill_fraction),
            mountain: quantile(&land, 1.0 - self.config.mountain_fraction),
            peak: quantile(&land, 1.0 - self.config.peak_fraction),
        }
    }

    /// The sampling space of the coarse grid.
    pub fn space(&self) -> &SampleSpace {
        &self.space
    }

    /// Whether the cell at `p` is under water. Out of bounds counts as water,
    /// because the border is ocean.
    pub fn is_water(&self, p: Point) -> bool {
        self.height.get(p).is_none_or(|h| *h <= self.sea_level)
    }

    /// As [`is_water`](Self::is_water), by flat index.
    pub fn is_water_idx(&self, idx: usize) -> bool {
        self.height[idx] <= self.sea_level
    }

    /// Height above sea level rescaled so the summit is 1. Zero for water.
    pub fn land_height(&self, p: Point) -> f32 {
        self.height.get(p).map(|h| self.land_height_of(*h)).unwrap_or(0.0)
    }

    /// Converts a normalized height into land height.
    pub fn land_height_of(&self, height: f32) -> f32 {
        let span = 1.0 - self.sea_level;
        if span <= f32::EPSILON {
            return 0.0;
        }
        ((height - self.sea_level) / span).max(0.0)
    }

    /// The normalized height of any point in fractional cell coordinates,
    /// from the same fields the grid was sampled from.
    ///
    /// A tile inside region `(rx, ry)` at local `(tx, ty)` with regions of
    /// `n` tiles is at `(rx + (tx + 0.5) / n, ry + (ty + 0.5) / n)`.
    /// Clamped to `[0, 1]` because a fine sample can fall just outside the
    /// coarse extremes.
    pub fn height_at(&self, fx: f64, fy: f64) -> f32 {
        let raw = raw_height(&self.space, &self.fields, &self.config, fx, fy);
        let span = self.raw_max - self.raw_min;
        if span <= f32::EPSILON {
            return 0.0;
        }
        ((raw - self.raw_min) / span).clamp(0.0, 1.0)
    }

    /// The normalized height of a tile, where each region is `tiles_per_cell`
    /// tiles across.
    pub fn height_at_tile(&self, tile: Point, tiles_per_cell: i32) -> f32 {
        let s = tiles_per_cell as f64;
        self.height_at((tile.x as f64 + 0.5) / s, (tile.y as f64 + 0.5) / s)
    }
}

fn raw_height(space: &SampleSpace, fields: &Fields, config: &ElevationConfig, fx: f64, fy: f64) -> f32 {
    let (nx, ny) = space.noise_point(fx, fy);
    let warped_x = nx + config.warp_strength * fields.warp.get(nx, ny);
    let warped_y = ny + config.warp_strength * fields.warp.get(nx + 31.7, ny - 17.3);
    let base = fields.continents.get_01(warped_x, warped_y);
    // Ridges are weighted by the base height so ranges rise out of the
    // interior rather than spiking out of open ocean.
    let ridge = fields.ridges.get_ridged(warped_x, warped_y) * base * base * config.ridge_strength;
    ((base + ridge) * continental_shelf(space, fields, config, fx, fy)) as f32
}

/// Falloff that sinks the map edges: a continent ringed by ocean rather
/// than land sliced off by the border. A squircle rather than a circle so a
/// wide map does not waste its corners.
fn continental_shelf(space: &SampleSpace, fields: &Fields, config: &ElevationConfig, fx: f64, fy: f64) -> f64 {
    let (dx, dy) = space.unit_offset(fx, fy);
    let mut distance = (dx.powi(4) + dy.powi(4)).powf(0.25);
    distance *= 1.0 + config.coast_noise_strength * fields.coast.get(dx * 0.5, dy * 0.5);
    let shelf = 1.0 - smoothstep(config.coast_inner_radius, config.coast_outer_radius, distance);
    let border = smoothstep(0.0, config.border_margin, space.cells_from_edge(fx, fy));
    shelf * border
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_core::Grid2D;

    #[test]
    fn land_fraction_is_stable_across_seeds() {
        let config = ElevationConfig::default();
        for seed in 1..=5u64 {
            let e = Elevation::generate(96, 64, RunSeed(seed), &config);
            let land = (0..e.height.len()).filter(|i| !e.is_water_idx(*i)).count() as f32 / e.height.len() as f32;
            assert!((land - config.land_fraction).abs() < 0.02, "seed {seed}: land {land}");
        }
    }

    #[test]
    fn the_border_is_always_ocean() {
        let e = Elevation::generate(80, 50, RunSeed(7), &ElevationConfig::default());
        for p in e.height.bounds().border() {
            assert!(e.is_water(p), "{p:?} is land on the border");
        }
        assert!(e.is_water(Point::new(-1, 0)));
    }

    #[test]
    fn relief_thresholds_are_ordered_and_binding() {
        let e = Elevation::generate(96, 64, RunSeed(3), &ElevationConfig::default());
        assert!(e.relief.hill < e.relief.mountain && e.relief.mountain < e.relief.peak);
        let land: Vec<f32> = (0..e.height.len()).filter(|i| !e.is_water_idx(*i)).map(|i| e.land_height_of(e.height[i])).collect();
        let hills = land.iter().filter(|h| **h >= e.relief.hill).count() as f32 / land.len() as f32;
        assert!((hills - 0.30).abs() < 0.03, "{hills}");
    }

    #[test]
    fn fine_sampling_matches_the_grid_at_cell_centres() {
        let e = Elevation::generate(64, 48, RunSeed(11), &ElevationConfig::default());
        for (p, h) in e.height.iter() {
            let fine = e.height_at(p.x as f64 + 0.5, p.y as f64 + 0.5);
            assert!((fine - h).abs() < 1e-5, "{p:?}: grid {h} vs fine {fine}");
            // Tile 8 of 16 has its centre at 8.5 / 16 into the cell.
            let tile = Point::new(p.x * 16 + 8, p.y * 16 + 8);
            let via_tile = e.height_at_tile(tile, 16);
            let direct = e.height_at(p.x as f64 + 8.5 / 16.0, p.y as f64 + 8.5 / 16.0);
            assert!((via_tile - direct).abs() < 1e-6);
        }
    }

    #[test]
    fn same_seed_same_world() {
        let a = Elevation::generate(40, 30, RunSeed(5), &ElevationConfig::default());
        let b = Elevation::generate(40, 30, RunSeed(5), &ElevationConfig::default());
        assert_eq!(a.height, b.height);
        assert_eq!(a.sea_level, b.sea_level);
    }
}
