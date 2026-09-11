//! Noise fields and the coordinate space they are sampled in.
//!
//! Every layer that reads a noise field goes through [`SampleSpace`], so a
//! region and the tiles inside it sample the same continuous field at the
//! same place. That agreement is what lets a chunk be generated on its own
//! and still match the region it sits in.

use noise::{NoiseFn, Simplex};
use rand::{Rng, SeedableRng, rngs::StdRng};
use rl_core::Point;

/// Frequency multiplier between octaves.
const LACUNARITY: f64 = 2.0;
/// Amplitude multiplier between octaves. Halving keeps amplitudes exact
/// powers of two so the ridged shaping commutes with scaling.
const GAIN: f64 = 0.5;

/// Fractal Brownian motion: octaves of simplex noise with geometrically
/// decreasing amplitude and increasing frequency.
#[derive(Debug, Clone)]
pub struct Fbm {
    octaves: Vec<Simplex>,
    frequency: f64,
    normalizer: f64,
}

impl Fbm {
    /// A field with `octaves` levels of detail, each octave independently
    /// seeded so they do not correlate into grid-aligned repetition.
    ///
    /// # Panics
    /// Panics if `octaves` is zero.
    pub fn new(seed: u64, octaves: u32) -> Self {
        assert!(octaves > 0, "fbm needs at least one octave");
        let mut rng = StdRng::seed_from_u64(seed);
        let sources = (0..octaves).map(|_| Simplex::new(rng.random())).collect();
        let mut total = 0.0;
        let mut amplitude = 1.0;
        for _ in 0..octaves {
            total += amplitude;
            amplitude *= GAIN;
        }
        Self { octaves: sources, frequency: 1.0, normalizer: 1.0 / total }
    }

    /// Sets the frequency of the first octave. Higher means smaller features.
    pub fn with_frequency(mut self, frequency: f64) -> Self {
        self.frequency = frequency;
        self
    }

    fn accumulate(&self, x: f64, y: f64, shape: impl Fn(f64) -> f64) -> f64 {
        let mut sum = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = self.frequency;
        for octave in &self.octaves {
            sum += amplitude * shape(octave.get([x * frequency, y * frequency]));
            amplitude *= GAIN;
            frequency *= LACUNARITY;
        }
        sum * self.normalizer
    }

    /// A value in `[-1, 1]`.
    pub fn get(&self, x: f64, y: f64) -> f64 {
        self.accumulate(x, y, |v| v).clamp(-1.0, 1.0)
    }

    /// A value in `[0, 1]`.
    pub fn get_01(&self, x: f64, y: f64) -> f64 {
        self.get(x, y) * 0.5 + 0.5
    }

    /// A ridged value in `[0, 1]`: each octave folded about zero, which turns
    /// smooth blobs into creased lines and makes ranges rather than lumps.
    pub fn get_ridged(&self, x: f64, y: f64) -> f64 {
        self.accumulate(x, y, |v| {
            let folded = 1.0 - v.abs();
            folded * folded
        })
        .clamp(0.0, 1.0)
    }
}

/// Converts coordinates in one grid into the spaces generation samples in.
///
/// Coordinates are `f64` cell positions so the same space serves whole
/// regions (`x + 0.5` is a region's centre) and the tiles inside them
/// (`x + t / region_size`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleSpace {
    width: i32,
    height: i32,
    scale: f64,
}

impl SampleSpace {
    /// A sampling space for a grid of this size.
    ///
    /// # Panics
    /// Panics if either dimension is not positive.
    pub fn new(width: i32, height: i32) -> Self {
        assert!(width > 0 && height > 0, "grid must be non-empty");
        Self { width, height, scale: width.max(height) as f64 }
    }

    /// Width in cells.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Height in cells.
    pub fn height(&self) -> i32 {
        self.height
    }

    /// The point to sample a noise field at for a fractional cell position.
    ///
    /// Both axes are divided by the longer one so features stay round on a
    /// map that is not square.
    pub fn noise_point(&self, fx: f64, fy: f64) -> (f64, f64) {
        (fx / self.scale, fy / self.scale)
    }

    /// The sample point for the centre of cell `p`.
    pub fn noise_point_of(&self, p: Point) -> (f64, f64) {
        self.noise_point(p.x as f64 + 0.5, p.y as f64 + 0.5)
    }

    /// The sample point for a tile inside cell-space, where each cell is
    /// `cells_per_unit` tiles across.
    pub fn noise_point_fine(&self, tile: Point, tiles_per_cell: i32) -> (f64, f64) {
        let s = tiles_per_cell as f64;
        self.noise_point((tile.x as f64 + 0.5) / s, (tile.y as f64 + 0.5) / s)
    }

    /// Position in `[-1, 1]` on each axis independently, for a fractional cell.
    pub fn unit_offset(&self, fx: f64, fy: f64) -> (f64, f64) {
        (fx / self.width as f64 * 2.0 - 1.0, fy / self.height as f64 * 2.0 - 1.0)
    }

    /// Distance from the equator: 0 at the middle row, 1 at either pole.
    pub fn latitude(&self, fy: f64) -> f64 {
        (fy / self.height as f64 * 2.0 - 1.0).abs()
    }

    /// Cells between a fractional position and the nearest edge.
    pub fn cells_from_edge(&self, fx: f64, fy: f64) -> f64 {
        let dx = fx.min(self.width as f64 - fx);
        let dy = fy.min(self.height as f64 - fy);
        dx.min(dy).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_stays_in_range_and_varies() {
        let fbm = Fbm::new(1, 6).with_frequency(3.0);
        let mut min = f64::MAX;
        let mut max = f64::MIN;
        for i in 0..500 {
            let t = i as f64 * 0.017;
            let v = fbm.get(t, t * 1.7);
            assert!((-1.0..=1.0).contains(&v));
            assert!((0.0..=1.0).contains(&fbm.get_01(t, t * 1.7)));
            assert!((0.0..=1.0).contains(&fbm.get_ridged(t, t * 1.7)));
            min = min.min(v);
            max = max.max(v);
        }
        assert!(max - min > 0.2, "field is too flat: {min}..{max}");
    }

    #[test]
    fn seeds_pin_fields() {
        let a = Fbm::new(99, 4);
        let b = Fbm::new(99, 4);
        let c = Fbm::new(100, 4);
        assert_eq!(a.get(0.3, 0.7), b.get(0.3, 0.7));
        assert_ne!(a.get(0.3, 0.7), c.get(0.3, 0.7));
    }

    #[test]
    fn coarse_and_fine_sampling_agree() {
        // The centre of region (3, 2) is the same point as the centre tile of
        // that region at fine scale.
        let space = SampleSpace::new(10, 8);
        let coarse = space.noise_point_of(Point::new(3, 2));
        let fine = space.noise_point_fine(Point::new(3 * 64 + 32, 2 * 64 + 32), 64);
        assert!((coarse.0 - fine.0).abs() < 1e-2 && (coarse.1 - fine.1).abs() < 1e-2, "{coarse:?} vs {fine:?}");
    }

    #[test]
    fn noise_sampling_is_isotropic_on_a_wide_map() {
        let space = SampleSpace::new(200, 50);
        let (x0, y0) = space.noise_point(0.5, 0.5);
        let (x1, y1) = space.noise_point(1.5, 1.5);
        assert!(((x1 - x0) - (y1 - y0)).abs() < 1e-12);
    }

    #[test]
    fn latitude_and_edges() {
        let space = SampleSpace::new(10, 100);
        assert!(space.latitude(50.5) < 0.02);
        assert!(space.latitude(0.5) > 0.98);
        assert_eq!(space.cells_from_edge(0.0, 5.0), 0.0);
        assert_eq!(space.cells_from_edge(3.0, 5.0), 3.0);
        let (l, t) = space.unit_offset(0.0, 0.0);
        assert_eq!((l, t), (-1.0, -1.0));
    }
}
