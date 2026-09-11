//! Temperature and moisture, the two axes a game's bands are usually cut from.

use std::collections::VecDeque;

use rl_core::stats::rank_normalize_where;
use rl_core::{Grid, Grid2D, Point, RunSeed, SeedDomain, Steps};

use crate::elevation::Elevation;
use crate::noise::Fbm;

/// Tunable knobs for climate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClimateConfig {
    /// How much colder the summits are than the shore, as a fraction of the
    /// full temperature range.
    pub lapse_rate: f32,
    /// Amplitude of the regional wobble on the latitude gradient.
    pub temperature_noise: f32,
    /// Frequency of that wobble.
    pub temperature_frequency: f64,
    /// Frequency of the base moisture field.
    pub moisture_frequency: f64,
    /// Detail levels in the moisture field.
    pub moisture_octaves: u32,
    /// Cells from water over which coastal humidity decays.
    pub coastal_reach: f32,
    /// Weight of proximity to water against the raw moisture noise.
    pub coastal_weight: f32,
}

impl Default for ClimateConfig {
    fn default() -> Self {
        Self {
            lapse_rate: 0.62,
            temperature_noise: 0.10,
            temperature_frequency: 2.4,
            moisture_frequency: 3.2,
            moisture_octaves: 5,
            coastal_reach: 9.0,
            coastal_weight: 0.42,
        }
    }
}

/// Per-cell climate, each field in `[0, 1]`.
#[derive(Debug, Clone)]
pub struct Climate {
    /// 0 is polar, 1 is equatorial.
    pub temperature: Grid<f32>,
    /// 0 is desert-dry, 1 is saturated.
    pub moisture: Grid<f32>,
    /// Cells of land between here and the nearest water; 0 on water.
    pub distance_to_water: Grid<u16>,
}

impl Climate {
    /// Builds the climate layers. `is_water` says which cells count as
    /// water for humidity: the sea, and any lakes and rivers hydrology made.
    pub fn generate(elevation: &Elevation, is_water: impl Fn(usize) -> bool, seed: RunSeed, config: &ClimateConfig) -> Self {
        let space = *elevation.space();
        let (width, height) = (elevation.height.width(), elevation.height.height());
        let d = |name: &[u8]| seed.derive(SeedDomain::new(name), 0);

        let wobble = Fbm::new(d(b"climate.temperature"), 4).with_frequency(config.temperature_frequency);
        let temperature = Grid::from_fn(width, height, |p| {
            // A cosine of latitude gives wide tropics and wide ice caps with a
            // fast-changing temperate band between.
            let base = (space.latitude(p.y as f64 + 0.5) * std::f64::consts::FRAC_PI_2).cos() as f32;
            let (fx, fy) = space.noise_point_of(p);
            let regional = wobble.get(fx, fy) as f32 * config.temperature_noise;
            let altitude_penalty = elevation.land_height(p) * config.lapse_rate;
            (base + regional - altitude_penalty).clamp(0.0, 1.0)
        });

        let distance_to_water = distance_field(&elevation.height, &is_water);

        let noise = Fbm::new(d(b"climate.moisture"), config.moisture_octaves).with_frequency(config.moisture_frequency);
        let mut moisture = Grid::from_fn(width, height, |p| {
            if is_water(elevation.height.point_idx(p)) {
                return 1.0;
            }
            let (fx, fy) = space.noise_point_of(p);
            let base = noise.get_01(fx, fy) as f32;
            // Humidity blown off the water thins out inland, which puts
            // deserts in continental interiors instead of scattering them.
            let inland = distance_to_water[p] as f32 / config.coastal_reach;
            let coastal = (-inland).exp();
            base * (1.0 - config.coastal_weight) + coastal * config.coastal_weight
        });
        // Flatten over land so every world gets the same share of dry and wet.
        rank_normalize_where(moisture.cells_mut(), |i| !is_water(i));

        Self { temperature, moisture, distance_to_water }
    }
}

/// Four-way breadth-first distance from every `source` cell. Saturates at
/// `u16::MAX` for cells no source reaches.
pub fn distance_field<T>(grid: &Grid<T>, source: &impl Fn(usize) -> bool) -> Grid<u16> {
    let mut distance = Grid::filled(grid.width(), grid.height(), u16::MAX);
    let mut queue = VecDeque::new();
    for idx in 0..grid.len() {
        if source(idx) {
            distance[idx] = 0;
            queue.push_back(idx);
        }
    }
    while let Some(idx) = queue.pop_front() {
        let next = distance[idx].saturating_add(1);
        let p: Point = grid.idx_point(idx);
        for n in grid.neighbour_indices(p, Steps::Four) {
            if distance[n] > next {
                distance[n] = next;
                queue.push_back(n);
            }
        }
    }
    distance
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elevation::ElevationConfig;

    fn sample() -> (Elevation, Climate) {
        let e = Elevation::generate(96, 64, RunSeed(20_260_728), &ElevationConfig::default());
        let c = Climate::generate(&e, |i| e.is_water_idx(i), RunSeed(20_260_728), &ClimateConfig::default());
        (e, c)
    }

    #[test]
    fn everything_is_in_the_unit_interval() {
        let (_, c) = sample();
        for v in c.temperature.cells().iter().chain(c.moisture.cells()) {
            assert!((0.0..=1.0).contains(v));
        }
    }

    #[test]
    fn the_poles_are_colder_than_the_equator() {
        let (_, c) = sample();
        let row = |y: i32| (0..96).map(|x| c.temperature[Point::new(x, y)]).sum::<f32>() / 96.0;
        assert!(row(0) < row(32));
        assert!(row(63) < row(32));
    }

    #[test]
    fn water_is_saturated_and_land_moisture_is_flat() {
        let (e, c) = sample();
        let mut land: Vec<f32> = Vec::new();
        for i in 0..e.height.len() {
            if e.is_water_idx(i) {
                assert_eq!(c.moisture[i], 1.0);
                assert_eq!(c.distance_to_water[i], 0);
            } else {
                land.push(c.moisture[i]);
                assert!(c.distance_to_water[i] > 0);
            }
        }
        let dry = land.iter().filter(|m| **m < 0.25).count() as f32 / land.len() as f32;
        assert!((dry - 0.25).abs() < 0.02, "{dry}");
    }

    #[test]
    fn distance_field_counts_steps() {
        let g: Grid<u8> = Grid::new(5, 1);
        let d = distance_field(&g, &|i| i == 0);
        assert_eq!(d.cells(), &[0, 1, 2, 3, 4]);
        let none = distance_field(&g, &|_| false);
        assert!(none.cells().iter().all(|v| *v == u16::MAX));
    }
}
