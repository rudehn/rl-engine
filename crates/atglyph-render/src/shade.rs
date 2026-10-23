//! Colour for a map drawn the way Brogue draws one.
//!
//! Four things make the difference between a grid of flat glyphs and a
//! place with air in it, and each is one small function here:
//!
//! - every cell is its own shade of its tile, jittered by a hash of its
//!   position so a floor reads as stone rather than wallpaper;
//! - light colours the background as well as the glyph, so a torch paints
//!   the room it stands in, and a coloured light lends its own hue as well
//!   as filtering the tile's, so firelight warms green moss rather than
//!   only darkening it;
//! - flickering light and restless tiles drift over time, from a smooth
//!   noise that neighbouring cells share, so fire ripples instead of
//!   strobing;
//! - memory fades to a cold, desaturated blue, so what you saw and what
//!   you see never look alike.
//!
//! All of it is cosmetic. Nothing here is read by a gameplay rule, and the
//! only clock is the frame's, so a replayed seed plays identically however
//! it looks.

use atglyph_core::Point;
use atglyph_core::seed::{mix64, position_hash};
use atglyph_grid::{Light, Rgb};
use bevy::prelude::*;

/// How a tile's colours vary from cell to cell and over time. Applied to
/// both the glyph and the background, in step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vary {
    /// Brightness spread between cells, 0 to 1: at 0.2 each cell is drawn
    /// between 80% and 120% of the authored colour.
    pub brightness: f32,
    /// Spread per colour channel, independent of the others, 0 to 1: a
    /// mottled hue rather than a brighter or darker one.
    pub hue: f32,
    /// Brightness that drifts over time, 0 to 1, for water and anything
    /// else that should not sit still.
    pub shimmer: f32,
}

impl Vary {
    /// The authored colour everywhere, always.
    pub(crate) const NONE: Self = Self { brightness: 0.0, hue: 0.0, shimmer: 0.0 };

    /// A cell-to-cell spread in brightness and hue.
    pub const fn new(brightness: f32, hue: f32) -> Self {
        Self { brightness, hue, shimmer: 0.0 }
    }

    /// The same spread, drifting over time by `amount`.
    pub const fn shimmering(mut self, amount: f32) -> Self {
        self.shimmer = amount;
        self
    }
}

/// How a remembered tile is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Memory {
    /// Brightness kept, 0 to 1.
    pub brightness: f32,
    /// Colour kept, 0 for grey to 1 for the full hue.
    pub saturation: f32,
    /// What the kept colour is multiplied by: a cool tint reads as memory.
    pub tint: Color,
}

impl Default for Memory {
    fn default() -> Self {
        Self { brightness: 0.4, saturation: 0.3, tint: Color::srgb(0.7, 0.78, 1.0) }
    }
}

/// How light turns into colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shading {
    /// Brightness a seen but unlit tile keeps, 0 to 1: what dark sight or
    /// touch shows in the dark.
    pub dark_floor: f32,
    /// The light level, per channel, at which a tile shows its authored colour.
    pub full: u8,
    /// How far above its authored colour a brightly lit tile may go.
    pub max_gain: f32,
    /// How much of a light's own hue it lends a tile on top of filtering
    /// the tile's colour, in linear units at the `full` light level.
    ///
    /// Filtering alone can only take away, so a light's hue showed only on
    /// a tile that already had it: an orange fire on green moss darkened the
    /// green and read as a lantern. Only the hue is lent, the light's colour
    /// less its grey, because the grey part is what filtering already shows
    /// and lending it too would wash every lit tile toward white: a white or
    /// grey light shades exactly as it would at 0. Small, because it is
    /// added to every lit cell's dark background as much as to a bright glyph.
    pub warmth: f32,
    /// How fast flickering light moves, in noise cells per second.
    pub flicker_speed: f32,
    /// How fast restless tiles drift, in noise cells per second.
    pub shimmer_speed: f32,
    /// Salt for the per-cell jitter, so two games need not share a floor pattern.
    pub seed: u64,
}

impl Default for Shading {
    fn default() -> Self {
        Self { dark_floor: 0.12, full: 170, max_gain: 1.5, warmth: 0.04, flicker_speed: 5.0, shimmer_speed: 0.7, seed: 0x5eed_c010_0f5e }
    }
}

/// A value in `[-1, 1]` from 16 bits of `h` starting at `shift`.
fn unit(h: u64, shift: u32) -> f32 {
    ((h >> shift) & 0xFFFF) as f32 / 32767.5 - 1.0
}

/// Scales a colour's linear channels.
fn scale(c: Color, r: f32, g: f32, b: f32) -> Color {
    let l = c.to_linear();
    Color::linear_rgba(l.red * r, l.green * g, l.blue * b, l.alpha)
}

/// Smooth value noise in `[0, 1]` over the plane and time: cells a tile
/// or two apart move together, so a flame ripples rather than strobes.
pub(crate) fn wave(seed: u64, p: Point, t: f32) -> f32 {
    let (x, y) = (p.x as f32 * 0.45, p.y as f32 * 0.45);
    let (x0, y0, t0) = (x.floor(), y.floor(), t.floor());
    let (fx, fy, ft) = (x - x0, y - y0, t - t0);
    let s = |f: f32| f * f * (3.0 - 2.0 * f);
    let (sx, sy, st) = (s(fx), s(fy), s(ft));
    let corner = |dx: i32, dy: i32, dt: i64| -> f32 {
        let h = position_hash(seed ^ mix64((t0 as i64 + dt) as u64), x0 as i32 + dx, y0 as i32 + dy);
        (h >> 40) as f32 / (1u64 << 24) as f32
    };
    let lerp = |a: f32, b: f32, f: f32| a + (b - a) * f;
    let plane = |dt: i64| lerp(lerp(corner(0, 0, dt), corner(1, 0, dt), sx), lerp(corner(0, 1, dt), corner(1, 1, dt), sx), sy);
    lerp(plane(0), plane(1), st)
}

impl Shading {
    /// `c` as it is drawn at `p`: jittered per cell, drifting if `vary`
    /// shimmers, at time `t` seconds.
    pub(crate) fn vary(&self, c: Color, vary: Vary, p: Point, t: f32) -> Color {
        if vary == Vary::NONE {
            return c;
        }
        let h = position_hash(self.seed, p.x, p.y);
        let mut b = 1.0 + vary.brightness * unit(h, 0);
        if vary.shimmer > 0.0 {
            b *= 1.0 + vary.shimmer * (wave(self.seed ^ 0x51, p, t * self.shimmer_speed) * 2.0 - 1.0);
        }
        let hue = |shift| b * (1.0 + vary.hue * unit(h, shift));
        scale(c, hue(16), hue(32), hue(48))
    }

    /// `c` under `light` at `p`, at time `t` seconds: multiplied channel by
    /// channel by the light that lands, down to the dark floor and up to
    /// the gain cap, plus [`warmth`](Self::warmth)'s share of the light's
    /// hue, with the wavering part of the light dipping in time.
    ///
    /// Both parts only grow with the light, so a light a darkness has taken
    /// from never shades a tile brighter than the light did whole, and no
    /// light at all is the dark floor.
    pub fn light(&self, c: Color, light: Light, p: Point, t: f32) -> Color {
        let steady = if light.waver > 0 && light.intensity > 0 {
            let dip = light.waver as f32 * wave(self.seed ^ 0xf1, p, t * self.flicker_speed);
            (light.intensity as f32 - dip) / light.intensity as f32
        } else {
            1.0
        };
        let full = self.full.max(1) as f32;
        let f = |ch: u8| (self.dark_floor + (1.0 - self.dark_floor) * ch as f32 * steady / full).min(self.max_gain);
        let Rgb { r, g, b } = light.color;
        let grey = r.min(g).min(b);
        let lend = |ch: u8| self.warmth * (ch - grey) as f32 * steady / full;
        let l = c.to_linear();
        Color::linear_rgba(l.red * f(r) + lend(r), l.green * f(g) + lend(g), l.blue * f(b) + lend(b), l.alpha)
    }
}

impl Memory {
    /// `c` as remembered.
    pub fn recall(&self, c: Color) -> Color {
        let l = c.to_linear();
        let luma = 0.2126 * l.red + 0.7152 * l.green + 0.0722 * l.blue;
        let keep = |ch: f32| (luma + (ch - luma) * self.saturation) * self.brightness;
        let tint = self.tint.to_linear();
        Color::linear_rgba(keep(l.red) * tint.red, keep(l.green) * tint.green, keep(l.blue) * tint.blue, l.alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channels(c: Color) -> [f32; 3] {
        let l = c.to_linear();
        [l.red, l.green, l.blue]
    }

    #[test]
    fn jitter_is_a_function_of_position_alone() {
        let s = Shading::default();
        let v = Vary::new(0.2, 0.05);
        let grey = Color::srgb(0.5, 0.5, 0.5);
        let a = s.vary(grey, v, Point::new(3, 4), 0.0);
        assert_eq!(channels(a), channels(s.vary(grey, v, Point::new(3, 4), 99.0)), "no shimmer, no drift");
        assert_ne!(channels(a), channels(s.vary(grey, v, Point::new(4, 4), 0.0)));
        let base = channels(grey)[0];
        for x in 0..50 {
            let [r, _, _] = channels(s.vary(grey, v, Point::new(x, 0), 0.0));
            assert!(r >= base * 0.75 && r <= base * 1.26, "within the spread: {r} vs {base}");
        }
        assert_eq!(channels(s.vary(grey, Vary::NONE, Point::new(9, 9), 0.0)), channels(grey));
    }

    #[test]
    fn full_light_shows_the_authored_colour_and_darkness_the_floor() {
        let s = Shading::default();
        let c = Color::srgb(0.6, 0.4, 0.2);
        let lit = s.light(c, Light::white(s.full), Point::ZERO, 0.0);
        for (a, b) in channels(lit).iter().zip(channels(c)) {
            assert!((a - b).abs() < 1e-4);
        }
        let dark = s.light(c, Light::DARK, Point::ZERO, 0.0);
        assert!((channels(dark)[0] - channels(c)[0] * s.dark_floor).abs() < 1e-4);
        let amber = s.light(c, Light::new(200, Rgb::new(255, 120, 40)), Point::ZERO, 0.0);
        assert!(channels(amber)[0] / channels(c)[0] > channels(amber)[2] / channels(c)[2], "warm light warms");
    }

    /// A value in `[0, 1)` from `seed`, for spans of inputs without a
    /// generator: this crate draws nothing, so it has none to hand.
    fn draw(seed: u64) -> f32 {
        (mix64(seed) >> 40) as f32 / (1u64 << 24) as f32
    }

    fn byte(seed: u64) -> u8 {
        (mix64(seed) >> 56) as u8
    }

    #[test]
    fn a_coloured_light_warms_a_tile_with_little_of_its_colour() {
        let s = Shading::default();
        let filtered = Shading { warmth: 0.0, ..s };
        let moss = Color::srgb(0.08, 0.18, 0.13);
        let fire = Light::new(200, Rgb::new(255, 128, 51));
        let [r, g, b] = channels(s.light(moss, fire, Point::ZERO, 0.0));
        let [r0, g0, b0] = channels(filtered.light(moss, fire, Point::ZERO, 0.0));
        assert!(r > r0 * 2.0, "the fire lends the moss its red: {r} against {r0} by filtering alone");
        assert!(r > g, "and the moss reads warm, not green: {r}, {g}");
        assert!(g >= g0 && (b - b0).abs() < 1e-6, "a channel the fire has least of is lent nothing");
        let [mr, mg, _] = channels(moss);
        assert!(r0 / g0 > mr / mg, "filtering still tints as it did");
    }

    /// Over a span of grey and white lights, tiles, places and times: a
    /// light with no hue has none to lend, so it shades exactly as filtering
    /// alone did before a light lent anything.
    #[test]
    fn a_white_or_grey_light_shades_exactly_as_it_did() {
        let s = Shading::default();
        let filtered = Shading { warmth: 0.0, ..s };
        for i in 0..2000u64 {
            let tile = Color::srgb(draw(i * 7), draw(i * 7 + 1), draw(i * 7 + 2));
            let light = Light::white(byte(i * 7 + 3)).flickering(byte(i * 7 + 4));
            let (p, t) = (Point::new(i as i32 % 37, i as i32 / 37), draw(i * 7 + 5) * 50.0);
            assert_eq!(channels(s.light(tile, light, p, t)), channels(filtered.light(tile, light, p, t)), "{light:?}");
        }
    }

    /// Over a span of lights, darknesses, tiles and times: what a darkness
    /// leaves never shades a tile brighter than the whole light, and a
    /// darkness with no light under it is exactly unlit.
    #[test]
    fn a_darkened_light_never_shades_a_tile_brighter() {
        let s = Shading::default();
        for i in 0..4000u64 {
            let tile = Color::srgb(draw(i * 11), draw(i * 11 + 1), draw(i * 11 + 2));
            let hue = Rgb::new(byte(i * 11 + 3), byte(i * 11 + 4), byte(i * 11 + 5));
            let light = Light::new(byte(i * 11 + 6), hue).flickering(byte(i * 11 + 7));
            let shade = byte(i * 11 + 8);
            let (p, t) = (Point::new(i as i32 % 41, i as i32 / 41), draw(i * 11 + 9) * 50.0);
            let whole = channels(s.light(tile, light, p, t));
            let taken = channels(s.light(tile, light.darkened(shade), p, t));
            let unlit = channels(s.light(tile, Light::DARK, p, t));
            for ((w, d), u) in whole.iter().zip(taken).zip(unlit) {
                assert!(d <= w + 1e-6, "{light:?} under {shade}: {d} against {w}");
                assert!(d >= u - 1e-6, "never below unlit");
            }
        }
        let tile = Color::srgb(0.4, 0.5, 0.6);
        assert_eq!(channels(s.light(tile, Light::DARK.darkened(255), Point::ZERO, 0.0)), channels(s.light(tile, Light::DARK, Point::ZERO, 0.0)));
    }

    #[test]
    fn flicker_only_ever_dims_and_moves_with_time() {
        let s = Shading::default();
        let c = Color::srgb(0.5, 0.5, 0.5);
        let flame = Light::white(200).flickering(200);
        let steady = channels(s.light(c, Light::white(200), Point::new(2, 2), 0.0))[0];
        let samples: Vec<f32> = (0..40).map(|i| channels(s.light(c, flame, Point::new(2, 2), i as f32 * 0.1))[0]).collect();
        assert!(samples.iter().all(|v| *v <= steady + 1e-5));
        assert!(samples.iter().any(|v| (v - samples[0]).abs() > 1e-3), "it moves");
    }

    #[test]
    fn memory_is_darker_greyer_and_cooler() {
        let m = Memory::default();
        let c = Color::srgb(0.9, 0.5, 0.2);
        let [r, g, b] = channels(m.recall(c));
        let [r0, _, _] = channels(c);
        assert!(r < r0 * 0.5);
        assert!(r - b < (r0 - channels(c)[2]) * 0.5, "less saturated");
        assert!(b / r > channels(c)[2] / r0, "bluer");
        let _ = g;
    }

    #[test]
    fn wave_stays_in_range_and_is_smooth() {
        let mut prev = wave(1, Point::new(5, 5), 0.0);
        for i in 1..200 {
            let v = wave(1, Point::new(5, 5), i as f32 * 0.01);
            assert!((0.0..=1.0).contains(&v));
            assert!((v - prev).abs() < 0.1, "no jumps");
            prev = v;
        }
    }
}
