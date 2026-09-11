//! The coarse layers sampled per tile, so a chunk is painted from the
//! same fields the region was banded from instead of from one band.
//!
//! Height already has a continuous form: the region grid was sampled from
//! noise, and a tile asks the noise directly. Climate and the water
//! distance have no such form, so they are interpolated between region
//! centres, and the position they are read at is warped by a low
//! frequency noise so a biome edge wanders instead of running along the
//! region grid. Everything is a pure function of world position, which is
//! what keeps two chunks agreeing at their seam.

use rl_core::{Grid, Grid2D, Point, RunSeed, SeedDomain};

use crate::facts::{CellFacts, Relief};
use crate::graph::{Layers, WorldGraph};
use crate::noise::{Fbm, SampleSpace};

/// The value of `grid` at a fractional cell position, interpolated
/// between the four nearest cell centres and clamped at the edges.
pub fn bilinear(grid: &Grid<f32>, fx: f64, fy: f64) -> f32 {
    let (w, h) = (grid.width(), grid.height());
    if w == 0 || h == 0 {
        return 0.0;
    }
    // Cell centres sit at i + 0.5; clamp so the edge cells extend outward.
    let x = (fx - 0.5).clamp(0.0, (w - 1) as f64);
    let y = (fy - 0.5).clamp(0.0, (h - 1) as f64);
    let (x0, y0) = (x.floor() as i32, y.floor() as i32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (tx, ty) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32);
    let at = |x: i32, y: i32| grid[Point::new(x, y)];
    let top = at(x0, y0) * (1.0 - tx) + at(x1, y0) * tx;
    let bottom = at(x0, y1) * (1.0 - tx) + at(x1, y1) * tx;
    top * (1.0 - ty) + bottom * ty
}

/// The noise fields behind per-tile sampling.
#[derive(Debug, Clone)]
pub struct FineFields {
    warp_x: Fbm,
    warp_y: Fbm,
    clumps: Fbm,
    /// How far, in regions, the warp may move a sample.
    pub warp_cells: f64,
    region_size: i32,
}

impl FineFields {
    /// Fields for a world of `space` regions of `region_size` tiles.
    pub fn new(seed: RunSeed, space: &SampleSpace, region_size: i32) -> Self {
        let d = |name: &[u8]| seed.derive(SeedDomain::new(name), 0);
        // The warp wobbles over a couple of regions; the clumps over a
        // dozen tiles.
        let regions = space.width().max(space.height()) as f64;
        let warp_frequency = regions / 2.0;
        let clump_frequency = regions * region_size as f64 / 12.0;
        Self {
            warp_x: Fbm::new(d(b"fine.warp.x"), 3).with_frequency(warp_frequency),
            warp_y: Fbm::new(d(b"fine.warp.y"), 3).with_frequency(warp_frequency),
            clumps: Fbm::new(d(b"fine.clumps"), 2).with_frequency(clump_frequency),
            warp_cells: 0.4,
            region_size,
        }
    }

    /// The fractional cell position a tile's climate is read at: its own
    /// position, moved by the warp.
    pub fn warped(&self, space: &SampleSpace, tile: Point) -> (f64, f64) {
        let s = self.region_size as f64;
        let (fx, fy) = ((tile.x as f64 + 0.5) / s, (tile.y as f64 + 0.5) / s);
        let (nx, ny) = space.noise_point(fx, fy);
        (fx + self.warp_x.get(nx, ny) * self.warp_cells, fy + self.warp_y.get(nx, ny) * self.warp_cells)
    }

    /// A slow noise over tiles in `[0, 1]`: where vegetation gathers and
    /// where it thins.
    pub fn clump_at(&self, space: &SampleSpace, tile: Point) -> f32 {
        let (nx, ny) = space.noise_point_fine(tile, self.region_size);
        self.clumps.get_01(nx, ny) as f32
    }
}

impl Layers {
    /// Temperature and moisture at a fractional cell position.
    pub fn climate_at(&self, fx: f64, fy: f64) -> (f32, f32) {
        (bilinear(&self.climate.temperature, fx, fy), bilinear(&self.climate.moisture, fx, fy))
    }

    /// Cells of land to the nearest water at a fractional cell position.
    pub fn distance_to_water_at(&self, fx: f64, fy: f64) -> f32 {
        bilinear(&self.distance_to_water_f, fx, fy)
    }
}

impl WorldGraph {
    /// Everything the engine knows about one tile: the height from the
    /// continuous field, climate interpolated between regions at a
    /// warped position, relief and coast from those, and the region's
    /// lake and river flags.
    pub fn tile_facts(&self, tile: Point) -> CellFacts {
        let layers = self.layers();
        let elevation = &layers.elevation;
        let space = elevation.space();
        let size = self.region_size();
        let region = self.region_of_tile(tile);
        let coarse = layers.facts(region).unwrap_or_else(CellFacts::sea);
        let height = elevation.height_at_tile(tile, size);
        let is_sea = height <= elevation.sea_level;
        let land_height = elevation.land_height_of(height);
        let (wx, wy) = self.fine().warped(space, tile);
        let (temperature, moisture) = layers.climate_at(wx, wy);
        let distance = layers.distance_to_water_at(wx, wy);
        let relief = if is_sea {
            Relief::Lowland
        } else {
            let t = elevation.relief;
            if land_height >= t.peak {
                Relief::Peak
            } else if land_height >= t.mountain {
                Relief::Mountain
            } else if land_height >= t.hill {
                Relief::Hill
            } else {
                Relief::Lowland
            }
        };
        CellFacts {
            is_sea,
            is_lake: coarse.is_lake,
            river_width: coarse.river_width,
            relief,
            height,
            land_height,
            temperature: (temperature - land_height * self.config().climate.lapse_rate * 0.25).clamp(0.0, 1.0),
            moisture,
            distance_to_water: distance.round() as u16,
            latitude: space.latitude((tile.y as f64 + 0.5) / size as f64) as f32,
            is_coast: !is_sea && distance < 1.0,
        }
    }

    /// Where vegetation gathers, in `[0, 1]`, for `tile`.
    pub fn clump_at(&self, tile: Point) -> f32 {
        self.fine().clump_at(self.layers().elevation.space(), tile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_hits_centres_and_blends_between_them() {
        let g = Grid::from_fn(2, 2, |p| (p.x + 2 * p.y) as f32);
        assert_eq!(bilinear(&g, 0.5, 0.5), 0.0);
        assert_eq!(bilinear(&g, 1.5, 0.5), 1.0);
        assert_eq!(bilinear(&g, 1.0, 0.5), 0.5);
        assert_eq!(bilinear(&g, 1.0, 1.0), 1.5);
        assert_eq!(bilinear(&g, -3.0, 9.0), 2.0, "clamped to the edge cells");
    }
}
