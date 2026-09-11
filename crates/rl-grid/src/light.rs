//! Light over a grid: point sources cast through the same symmetric
//! shadowcast as sight, blended into a field the field-of-view gate and
//! the renderer both read.
//!
//! Two channels, deliberately separate. `intensity` is what gameplay
//! reads: whether a tile is lit enough to be seen, how far a lit actor is
//! noticed from. `color` is what the renderer reads, already scaled by
//! the intensity so that two lights blend the way two lights do. Deriving
//! brightness from colour would make a red light count as darkness, so
//! nothing here ever does.
//!
//! Everything is integer, so a field is a deterministic function of tile
//! opacity and emitter placement, and emitters are sorted before they are
//! cast so the order they arrived in cannot leak into the result.

use rl_core::{Grid, Grid2D, Point, Rect};

use crate::bitgrid::BitGrid;
use crate::fov;
use crate::terrain::OpacitySource;

/// A colour, 0 to 255 per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// A colour.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Full white.
    pub const WHITE: Self = Self::new(255, 255, 255);

    /// This colour at `intensity / 255` of its strength.
    pub fn scaled(self, intensity: u8) -> Self {
        let s = |c: u8| ((c as u32 * intensity as u32) / 255) as u8;
        Self { r: s(self.r), g: s(self.g), b: s(self.b) }
    }

    /// Screen blend: brighter than either, never past 255.
    pub fn screen(self, other: Self) -> Self {
        Self { r: screen(self.r, other.r), g: screen(self.g, other.g), b: screen(self.b, other.b) }
    }
}

/// The light on one tile.
///
/// `color` is the colour as it lands: an emitter's hue already scaled by
/// the intensity that reached the tile, so [`Light::DARK`] is black and a
/// renderer multiplies pigment by it directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct Light {
    /// Brightness, 0 to 255. What gameplay reads.
    pub intensity: u8,
    /// The colour landing on the tile. What the renderer reads.
    pub color: Rgb,
}

impl Light {
    /// No light at all.
    pub const DARK: Self = Self { intensity: 0, color: Rgb::new(0, 0, 0) };

    /// Light of `intensity` in `color`, with the colour scaled to match.
    pub fn new(intensity: u8, color: Rgb) -> Self {
        Self { intensity, color: color.scaled(intensity) }
    }

    /// Uncoloured light: white at `intensity`.
    pub fn white(intensity: u8) -> Self {
        Self::new(intensity, Rgb::WHITE)
    }

    /// Both lights on one tile.
    pub fn screen(self, other: Self) -> Self {
        Self { intensity: screen(self.intensity, other.intensity), color: self.color.screen(other.color) }
    }
}

/// A point source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Emitter {
    /// Where it shines from.
    pub origin: Point,
    /// Brightness at the origin.
    pub intensity: u8,
    /// How far it reaches, in tiles; light is exactly zero at the rim.
    pub radius: i32,
    /// Its hue at full strength.
    pub color: Rgb,
}

impl Emitter {
    /// The light this emitter lands on a tile `d` away, before shadows.
    pub fn light_at(&self, d: i32) -> Light {
        Light::new(falloff(self.intensity, self.radius, d), self.color)
    }
}

/// Screen blend of two channels: `255 - (255 - a)(255 - b) / 255`.
///
/// Two lights are brighter than either and never exceed 255; `max` fails
/// the first, saturating addition the second.
#[inline]
pub fn screen(a: u8, b: u8) -> u8 {
    (255 - (((255 - a as u32) * (255 - b as u32)) / 255)) as u8
}

/// Brightness of a source of `intensity` and `radius` at distance `d`:
/// full within a third of the radius, then linear to exactly zero at the
/// rim, so the lit edge has nothing to alias against.
pub fn falloff(intensity: u8, radius: i32, d: i32) -> u8 {
    if d < 0 || d > radius {
        return 0;
    }
    let core = radius / 3;
    if d <= core {
        return intensity;
    }
    let span = (radius - core).max(1);
    let remaining = radius - d;
    ((intensity as i32 * remaining) / span) as u8
}

/// Rounded Euclidean distance, in whole tiles.
fn distance(a: Point, b: Point) -> i32 {
    let (dx, dy) = ((a.x - b.x) as i64, (a.y - b.y) as i64);
    // Round rather than floor, so a tile just past a whole number of
    // tiles counts as that number and the lit disc matches sight's.
    let d2 = (dx * dx + dy * dy) as u64;
    let root = d2.isqrt();
    if (root + 1) * (root + 1) - d2 <= d2 - root * root { root as i32 + 1 } else { root as i32 }
}

/// Light over a window of tiles.
#[derive(Debug, Clone)]
pub struct LightField {
    cells: Grid<Light>,
}

impl LightField {
    /// A dark field of `width` by `height`.
    pub fn new(width: i32, height: i32) -> Self {
        Self { cells: Grid::filled(width, height, Light::DARK) }
    }

    /// Everything dark again.
    pub fn clear(&mut self) {
        self.cells.fill(Light::DARK);
    }

    /// The light at `p`; out of bounds is dark.
    pub fn at(&self, p: Point) -> Light {
        self.cells.get(p).copied().unwrap_or(Light::DARK)
    }

    /// The light at an in-bounds index.
    pub fn get_idx(&self, idx: usize) -> Light {
        self.cells.cells()[idx]
    }

    /// The cells, row-major.
    pub fn cells(&self) -> &[Light] {
        self.cells.cells()
    }

    /// Adds `light` to `p`, screen-blended with what is there.
    pub fn add(&mut self, p: Point, light: Light) {
        if let Some(cell) = self.cells.get_mut(p) {
            *cell = cell.screen(light);
        }
    }

    /// Casts one emitter through `source`'s opacity into this field.
    /// `scratch` is the caller's viewshed buffer, reused across casts;
    /// it must match the field's shape.
    pub fn cast(&mut self, source: &impl OpacitySource, emitter: &Emitter, scratch: &mut BitGrid) {
        debug_assert_eq!((scratch.width(), scratch.height()), (self.width(), self.height()));
        if emitter.intensity == 0 || !self.in_bounds(emitter.origin) {
            return;
        }
        fov::compute(source, emitter.origin, emitter.radius, scratch);
        for p in scratch.iter() {
            let light = emitter.light_at(distance(emitter.origin, p));
            if light.intensity > 0 {
                self.add(p, light);
            }
        }
    }

    /// Casts every emitter, in an order that does not depend on the order
    /// given, so two runs over the same sources give the same field.
    pub fn cast_all(&mut self, source: &impl OpacitySource, emitters: &mut [Emitter], scratch: &mut BitGrid) {
        emitters.sort_unstable();
        for e in emitters.iter() {
            self.cast(source, e, scratch);
        }
    }

    /// Lights every cell of `area` with `light` directly, no shadows: for
    /// a lake of something glowing, where casting from every tile would
    /// cost a shadowcast each for no visible difference.
    pub fn flood(&mut self, area: Rect, light: Light) {
        for p in area.cells() {
            self.add(p, light);
        }
    }

    /// Sets this field to `a` screened with `b` and then `ambient`, cell
    /// by cell. All three fields must share a shape.
    pub fn compose(&mut self, a: &LightField, b: &LightField, ambient: Light) {
        debug_assert_eq!((a.width(), a.height()), (self.width(), self.height()));
        debug_assert_eq!((b.width(), b.height()), (self.width(), self.height()));
        for ((out, x), y) in self.cells.cells_mut().iter_mut().zip(a.cells()).zip(b.cells()) {
            *out = x.screen(*y).screen(ambient);
        }
    }
}

impl Grid2D for LightField {
    fn width(&self) -> i32 {
        self.cells.width()
    }

    fn height(&self) -> i32 {
        self.cells.height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::Terrain;
    use crate::tile::TileRegistry;
    use rand::seq::SliceRandom;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn falloff_is_full_in_the_core_and_exactly_zero_at_the_rim() {
        assert_eq!(falloff(200, 9, 0), 200);
        assert_eq!(falloff(200, 9, 3), 200);
        assert!(falloff(200, 9, 4) < 200);
        assert!(falloff(200, 9, 8) > 0);
        assert_eq!(falloff(200, 9, 9), 0);
        assert_eq!(falloff(200, 9, 10), 0);
        assert_eq!(falloff(200, 0, 0), 200, "a radius of zero lights its own tile");
        assert_eq!(falloff(200, 1, 1), 0);
    }

    #[test]
    fn screen_is_commutative_and_bounded() {
        for a in [0u8, 1, 100, 200, 255] {
            for b in [0u8, 7, 128, 255] {
                assert_eq!(screen(a, b), screen(b, a));
                assert!(screen(a, b) >= a.max(b));
            }
        }
        assert_eq!(screen(0, 0), 0);
        assert_eq!(screen(255, 40), 255);
        assert_eq!(screen(200, 200), 244);
    }

    #[test]
    fn a_lights_colour_is_scaled_by_its_intensity() {
        let l = Light::new(128, Rgb::new(255, 150, 40));
        assert_eq!(l.intensity, 128);
        assert_eq!(l.color, Rgb::new(128, 75, 20));
        assert_eq!(Light::new(0, Rgb::WHITE), Light::DARK);
    }

    #[test]
    fn two_colours_on_one_tile_blend_per_channel() {
        let amber = Light::new(170, Rgb::new(255, 160, 50));
        let blue = Light::new(110, Rgb::new(90, 170, 255));
        let both = amber.screen(blue);
        assert_eq!(both.color, Rgb::new(183, 149, 129));
        assert!(both.color.r > amber.color.r && both.color.b > amber.color.b, "warmer and paler than either");
        assert_eq!(both.intensity, screen(170, 110));
    }

    #[test]
    fn distance_rounds_to_whole_tiles() {
        let o = Point::ZERO;
        assert_eq!(distance(o, Point::new(3, 4)), 5);
        assert_eq!(distance(o, Point::new(1, 1)), 1);
        assert_eq!(distance(o, Point::new(2, 2)), 3);
        assert_eq!(distance(o, Point::new(7, 0)), 7);
    }

    fn room(width: i32, height: i32) -> (Terrain, TileRegistry) {
        let r = TileRegistry::standard();
        let (wall, floor) = (r.expect("wall"), r.expect("floor"));
        let t = Terrain::from_fn(width, height, |p| if p.x == 0 || p.y == 0 || p.x == width - 1 || p.y == height - 1 { wall } else { floor });
        (t, r)
    }

    #[test]
    fn a_wall_shadows_a_light_two_tiles_away() {
        let (mut t, r) = room(20, 9);
        let wall = r.expect("wall");
        for y in 1..8 {
            if y != 4 {
                t.set(Point::new(10, y), wall);
            }
        }
        // Wall at x=10 with a gap at y=4; light at (8,2) does not reach (12,2).
        let mut field = LightField::new(20, 9);
        let mut scratch = BitGrid::new(20, 9);
        field.cast(&t.view(&r), &Emitter { origin: Point::new(8, 2), intensity: 200, radius: 8, color: Rgb::WHITE }, &mut scratch);
        assert_eq!(field.at(Point::new(8, 2)).intensity, 200);
        assert!(field.at(Point::new(9, 2)).intensity > 0);
        assert_eq!(field.at(Point::new(12, 2)).intensity, 0, "behind the wall");
        assert_eq!(field.at(Point::new(8, 2) + Point::new(8, 0)).intensity, 0, "the rim is dark");
    }

    #[test]
    fn the_field_does_not_depend_on_emitter_order() {
        let (t, r) = room(30, 20);
        let view = t.view(&r);
        let mut rng = StdRng::seed_from_u64(9);
        let mut emitters: Vec<Emitter> = (0..12)
            .map(|i| Emitter {
                origin: Point::new(2 + (i * 5) % 26, 2 + (i * 7) % 16),
                intensity: 120 + i as u8 * 10,
                radius: 6,
                color: Rgb::new(255, 150, 40),
            })
            .collect();
        let mut scratch = BitGrid::new(30, 20);
        let mut first = LightField::new(30, 20);
        first.cast_all(&view, &mut emitters.clone(), &mut scratch);
        for _ in 0..5 {
            emitters.shuffle(&mut rng);
            let mut again = LightField::new(30, 20);
            again.cast_all(&view, &mut emitters.clone(), &mut scratch);
            assert_eq!(first.cells(), again.cells());
        }
    }

    #[test]
    fn compose_screens_the_layers_then_the_ambient() {
        let a = {
            let mut f = LightField::new(3, 1);
            f.add(Point::new(0, 0), Light::white(100));
            f
        };
        let b = {
            let mut f = LightField::new(3, 1);
            f.add(Point::new(0, 0), Light::white(100));
            f.add(Point::new(1, 0), Light::white(255));
            f
        };
        let mut out = LightField::new(3, 1);
        out.compose(&a, &b, Light::white(20));
        assert_eq!(out.at(Point::new(0, 0)).intensity, screen(screen(100, 100), 20));
        assert_eq!(out.at(Point::new(1, 0)).intensity, 255);
        assert_eq!(out.at(Point::new(2, 0)).intensity, 20);
        out.flood(Rect::new(0, 0, 3, 1), Light::white(255));
        assert_eq!(out.at(Point::new(2, 0)).intensity, 255);
    }
}
