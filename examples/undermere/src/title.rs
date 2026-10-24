//! The title screen's picture: the black lake at night, Mereholt's lit
//! windows on its near shore and the Drowned Gate on its island, under a
//! moon and UNDERMERE lettered across the stars.
//!
//! The menu over it is the engine's. `main.rs` adds `TitlePanel` with
//! [`menu`]'s rectangle beside the other panels, and [`paint_backdrop`] in
//! `TitleSet::Backdrop` with it. The panel holds the run back until the
//! player picks, and offers Continue, New Game, Records, Settings and
//! Exit, each only when it can be taken, on the dark water the picture
//! leaves clear for them. What is Undermere's here is the picture, and
//! where it leaves room for the menu.
//!
//! **The art.** Painted the way Brogue paints a floor: every cell its own
//! shade, the solid things in background colour and the glyphs for what
//! glints on them, since a glyph at this size never quite fills its cell
//! and a shape built of them shows the seams. Layered back to front, the
//! sky, the stars, the far shore, the moon, the lake, the word, Mereholt,
//! the gate, the mist, onto a canvas the terminal is written from once at
//! the end, so a layer of light can add to what is already there. The
//! shapes are `art`'s, the word and the town drawn as marks and the gate
//! as its stones, so a change to the picture is a change to one const.
//! Four things move, and only four, because a title that never settles
//! is tiring to look at: the stars twinkle, the gate's glow breathes, the
//! mist drifts, and the moon's path shimmers on the water.
//!
//! **The glyphs.** Only ones the game's typeface has: DejaVu Sans Mono,
//! from [`typeface`](crate::typeface), whose own character map a test
//! reads at every moment of the picture's motion. A glyph the face lacked
//! would be drawn as an empty box in the night.

use atglyph::primitives::{GridRect, Point};
use atglyph::render::{Cell, Terminal};
use bevy::prelude::*;

/// Where the engine's menu is drawn on a terminal `width` across: on the
/// near water under the middle of the lake, which the picture leaves dark
/// so the rows read over nothing. Six rows, the five the title can offer
/// and one spare, and wide enough for the question New Game asks over a
/// save.
pub fn menu(width: i32) -> GridRect {
    GridRect::new(width / 2 - MENU_WIDTH / 2, rows::MENU, MENU_WIDTH, 6)
}

/// Paints the picture, the whole terminal of it, as it stands `t` seconds
/// after launch.
///
/// In `TitleSet::Backdrop`, which runs only while the title is up, and
/// before the engine draws its menu over the picture. Paints nothing
/// without a terminal, which a headless test may leave out.
pub fn paint_backdrop(terminal: Option<ResMut<Terminal>>, time: Res<Time>) {
    let Some(mut terminal) = terminal else { return };
    let canvas = picture(terminal.width(), terminal.height(), time.elapsed_secs());
    canvas.draw(&mut terminal);
}

/// How wide the menu's rectangle is: the question New Game asks over a
/// save, with room either side.
const MENU_WIDTH: i32 = 48;

/// Every row the picture is laid out from, top to bottom, so nothing is
/// placed by counting on the screen.
mod rows {
    /// The top of UNDERMERE, which is seven rows tall.
    pub const TITLE: i32 = 3;
    /// The tip of Mereholt's tower, the top of the town's art.
    pub const TOWN: i32 = 11;
    /// The far shore's waterline: the sky above it, the lake from it down.
    pub const HORIZON: i32 = 22;
    /// Where Mereholt's quay meets the water.
    pub const QUAY: i32 = 27;
    /// Where the gate's island meets the water, nearer than the town.
    pub const ISLAND: i32 = 29;
    /// The first row the mist reaches, over the far shore.
    pub const MIST_TOP: i32 = 19;
    /// The last row the mist and the moon's path reach, two clear of the
    /// menu.
    pub const MIST_FLOOR: i32 = 32;
    /// The top of the engine's menu.
    pub const MENU: i32 = 35;
}

/// One cell of the picture while it is painted: its glyph, and both its
/// colours as plain red, green and blue, so a layer of light or mist can
/// mix into what the layers under it left.
#[derive(Clone, Copy, Debug)]
struct Paint {
    glyph: char,
    fg: Vec3,
    bg: Vec3,
}

/// The picture, a [`Paint`] to a cell, written to the terminal once it is
/// finished.
struct Canvas {
    width: i32,
    height: i32,
    cells: Vec<Paint>,
}

impl Canvas {
    /// A canvas of blank black cells, every one of which the sky or the
    /// lake paints over.
    fn new(width: i32, height: i32) -> Self {
        let blank = Paint { glyph: ' ', fg: Vec3::ZERO, bg: Vec3::ZERO };
        Self { width, height, cells: vec![blank; (width.max(0) * height.max(0)) as usize] }
    }

    /// The cell at `(x, y)`, or `None` off the canvas.
    fn at(&mut self, x: i32, y: i32) -> Option<&mut Paint> {
        let inside = (0..self.width).contains(&x) && (0..self.height).contains(&y);
        inside.then(|| &mut self.cells[(y * self.width + x) as usize])
    }

    /// Writes every cell to `terminal`, which is the canvas's size.
    fn draw(&self, terminal: &mut Terminal) {
        for y in 0..self.height {
            for x in 0..self.width {
                let paint = self.cells[(y * self.width + x) as usize];
                terminal.set(x, y, Cell::new(paint.glyph, colour(paint.fg)).on(colour(paint.bg)));
            }
        }
    }
}

/// A colour of the canvas as the terminal takes it.
fn colour(rgb: Vec3) -> Color {
    let rgb = rgb.clamp(Vec3::ZERO, Vec3::ONE);
    Color::srgb(rgb.x, rgb.y, rgb.z)
}

/// The colours the picture is painted in: deep blues and violets for the
/// night, warm light for the town, and the gate's own violet.
///
/// Its own colours rather than the palette's tones, because these are the
/// scene's own. The menu over it reads the game's `Palette` like every
/// other panel, so a game that repaints its tones repaints the words and
/// not the night.
mod ink {
    use bevy::math::Vec3;

    /// The sky overhead, all but black.
    pub const SKY_TOP: Vec3 = Vec3::new(0.010, 0.014, 0.045);
    /// The sky at the far shore, where the last of the dusk is violet.
    pub const SKY_LOW: Vec3 = Vec3::new(0.100, 0.076, 0.190);
    /// What the moon lights in the air around it.
    pub const HALO: Vec3 = Vec3::new(0.150, 0.165, 0.270);
    /// A star at its brightest: three kinds, cold, white and warm.
    pub const STARS: [Vec3; 3] = [Vec3::new(0.78, 0.85, 1.00), Vec3::new(0.96, 0.96, 0.98), Vec3::new(1.00, 0.90, 0.72)];
    /// The moon's face.
    pub const MOON: Vec3 = Vec3::new(0.94, 0.93, 0.84);
    /// The darker seas on it.
    pub const MARE: Vec3 = Vec3::new(0.70, 0.72, 0.74);
    /// The far shore, a shade off the sky behind it.
    pub const HILLS: Vec3 = Vec3::new(0.026, 0.030, 0.066);
    /// The lake under the far shore, holding the dusk.
    pub const WATER_FAR: Vec3 = Vec3::new(0.050, 0.052, 0.120);
    /// The lake at the viewer's feet: the black of the name.
    pub const WATER_NEAR: Vec3 = Vec3::new(0.008, 0.012, 0.032);
    /// A ripple catching the sky.
    pub const RIPPLE: Vec3 = Vec3::new(0.16, 0.18, 0.30);
    /// The moon on the water.
    pub const GLINT: Vec3 = Vec3::new(0.92, 0.93, 0.86);
    /// The word, where the moon catches its top.
    pub const WORD_TOP: Vec3 = Vec3::new(0.86, 0.88, 0.98);
    /// The word at its foot, gone over to violet.
    pub const WORD_FOOT: Vec3 = Vec3::new(0.46, 0.42, 0.74);
    /// A roof in the moon's shadow, darker than the sky behind it.
    pub const ROOF: Vec3 = Vec3::new(0.030, 0.030, 0.052);
    /// A roof's slope and edge turned to the moon, which catch its light.
    pub const ROOF_LIT: Vec3 = Vec3::new(0.200, 0.210, 0.310);
    /// A wall in the moon's shadow.
    pub const WALL: Vec3 = Vec3::new(0.042, 0.040, 0.060);
    /// A wall turned to the moon, which sets one house off from the next.
    pub const WALL_LIT: Vec3 = Vec3::new(0.115, 0.115, 0.160);
    /// The quay's stones and the tower's.
    pub const STONEWORK: Vec3 = Vec3::new(0.070, 0.070, 0.095);
    /// The land the town stands on, and the trees on it.
    pub const LAND: Vec3 = Vec3::new(0.018, 0.024, 0.034);
    /// A lit window.
    pub const WINDOW: Vec3 = Vec3::new(1.00, 0.74, 0.34);
    /// The frame across a lit window.
    pub const SASH: Vec3 = Vec3::new(0.30, 0.16, 0.07);
    /// A standing stone the glow is in front of.
    pub const STONE: Vec3 = Vec3::new(0.150, 0.150, 0.200);
    /// A near stone, standing dark against the glow.
    pub const STONE_NEAR: Vec3 = Vec3::new(0.050, 0.048, 0.075);
    /// The island's turf.
    pub const TURF: Vec3 = Vec3::new(0.030, 0.050, 0.055);
    /// The glow's heart, in the gate's doorway.
    pub const HEART: Vec3 = Vec3::new(0.62, 0.62, 1.00);
    /// The light the glow throws on what is round it.
    pub const GLOW: Vec3 = Vec3::new(0.50, 0.20, 0.90);
    /// The mist, as the night leaves it.
    pub const MIST: Vec3 = Vec3::new(0.36, 0.38, 0.52);
}

/// A number from 0 to 1 that depends on nothing but its inputs.
///
/// The picture's dice: which cell has a star, how bright a window burns.
/// A hash rather than a stream, because the title is outside a run and has
/// no seed to draw from, and a picture that is the same every time the
/// game is opened is the point.
fn hashed(a: i32, b: i32) -> f32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B9) ^ (b as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h % 1000) as f32 / 1000.0
}

/// Noise from 0 to 1 that varies smoothly over the plane: [`hashed`] at
/// the whole points, eased between them, for what has no edges, the mist,
/// the far shore and the seas on the moon.
fn noise(x: f32, y: f32) -> f32 {
    let (i, j) = (x.floor() as i32, y.floor() as i32);
    let ease = |f: f32| f * f * (3.0 - 2.0 * f);
    let (u, v) = (ease(x - x.floor()), ease(y - y.floor()));
    let top = hashed(i, j) + (hashed(i + 1, j) - hashed(i, j)) * u;
    let bottom = hashed(i, j + 1) + (hashed(i + 1, j + 1) - hashed(i, j + 1)) * u;
    top + (bottom - top) * v
}

/// A cell's own shade: `rgb` lifted or dimmed by up to `by` of itself,
/// the same way every frame, so a wash of one colour reads as a surface.
fn grain(rgb: Vec3, x: i32, y: i32, by: f32) -> Vec3 {
    rgb * (1.0 + (hashed(x * 7 + 3, y * 13 + 5) - 0.5) * 2.0 * by)
}

/// A cell's centre in pixels, with the terminal's cells eight across and
/// sixteen down, so a distance is a distance on the screen and a circle
/// is round.
fn pixel(x: i32, y: i32) -> Vec2 {
    Vec2::new(x as f32 * 8.0 + 4.0, y as f32 * 16.0 + 8.0)
}

/// The whole picture `t` seconds in, painted back to front.
fn picture(width: i32, height: i32, t: f32) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    let glow = breath(t);
    sky(&mut canvas);
    stars(&mut canvas, t);
    far_shore(&mut canvas);
    moon(&mut canvas);
    lake(&mut canvas);
    moon_path(&mut canvas, t);
    word(&mut canvas);
    town(&mut canvas);
    gate(&mut canvas, glow);
    mist(&mut canvas, t, glow);
    canvas
}

/// How brightly the gate glows `t` seconds in, from a little under half
/// its light to all of it and back, one slow breath every seven seconds.
fn breath(t: f32) -> f32 {
    0.72 + 0.28 * (t * std::f32::consts::TAU / 7.0).sin()
}

/// Where the moon's centre stands on a terminal `width` across, in
/// pixels: over the middle of the far shore, under the word, at the
/// middle of a row, so with [`MOON_RADIUS`] its top and its foot fall on
/// the edges of rows.
fn moon_centre(width: i32) -> Vec2 {
    Vec2::new(width as f32 * 4.0, 15.5 * 16.0)
}

/// The moon's radius in pixels: three and a half rows, so it is seven
/// whole rows tall.
const MOON_RADIUS: f32 = 56.0;

/// The sky: the last of the dusk at the far shore going up to black, and
/// the air round the moon lit by it.
fn sky(canvas: &mut Canvas) {
    let moon = moon_centre(canvas.width);
    for y in 0..rows::HORIZON.min(canvas.height) {
        let up = 1.0 - y as f32 / rows::HORIZON as f32;
        let dusk = ink::SKY_LOW.lerp(ink::SKY_TOP, up.powf(0.6));
        for x in 0..canvas.width {
            let halo = ink::HALO * (-(pixel(x, y).distance(moon) - MOON_RADIUS).max(0.0) / 95.0).exp() * 0.6;
            let bg = grain(dusk + halo, x, y, 0.07);
            if let Some(cell) = canvas.at(x, y) {
                *cell = Paint { glyph: ' ', fg: bg, bg };
            }
        }
    }
}

/// The stars, each twinkling at its own slow pace, from a little under
/// half its light to all of it over five to fifteen seconds. Most are
/// faint; the few large ones flare from a cross to a spark at the top of
/// their twinkle. Fewer near the moon, which drowns them.
fn stars(canvas: &mut Canvas, t: f32) {
    let moon = moon_centre(canvas.width);
    for y in 0..rows::HORIZON {
        for x in (0..canvas.width).filter(|x| is_star(*x, y)) {
            let size = hashed(x + 101, y).powi(3);
            let pace = 0.4 + hashed(x, y + 211) * 0.85;
            let twinkle = 0.6 + 0.4 * (t * pace + hashed(x + 307, y) * std::f32::consts::TAU).sin();
            let drowned = ((pixel(x, y).distance(moon) - MOON_RADIUS * 1.2) / 140.0).clamp(0.0, 1.0);
            let light = (0.3 + 0.7 * size) * twinkle * drowned;
            let tint = ink::STARS[(hashed(x + 13, y + 17) * 3.0) as usize % 3];
            let Some(cell) = canvas.at(x, y) else { continue };
            cell.glyph = match size {
                s if s > 0.75 && twinkle > 0.93 => '✦',
                s if s > 0.75 => '+',
                s if s > 0.35 => '•',
                _ => '·',
            };
            cell.fg = cell.bg.lerp(tint, light);
        }
    }
}

/// Whether a star shines at `(x, y)`: about one cell in sixteen of the
/// sky, above the row the far shore hides.
fn is_star(x: i32, y: i32) -> bool {
    y < rows::HORIZON - 1 && hashed(x, y * 31 + 7) <= 0.06
}

/// The far shore: low wooded hills black against the dusk along the
/// waterline, lowest under the moon and rising toward either side, so
/// they hold it between them, their crests a shade lighter where the moon
/// is behind them.
fn far_shore(canvas: &mut Canvas) {
    let horizon = rows::HORIZON as f32 * 16.0;
    let moon = moon_centre(canvas.width);
    for x in 0..canvas.width {
        let fx = x as f32;
        let aside = ((pixel(x, 0).x - moon.x).abs() / moon.x).min(1.0);
        let crest = horizon - (4.0 + 44.0 * aside.powf(1.4) + 18.0 * noise(fx * 0.04, 3.0) + 8.0 * noise(fx * 0.13, 9.0) + 4.0 * noise(fx * 0.6, 17.0));
        for y in 0..rows::HORIZON.min(canvas.height) {
            let cover = ((y as f32 * 16.0 + 16.0 - crest) / 16.0).clamp(0.0, 1.0);
            if cover <= 0.0 {
                continue;
            }
            let rim = if cover < 1.0 { ink::HALO * (1.0 - pixel(x, y).distance(moon) / 260.0).max(0.0) * 0.5 } else { Vec3::ZERO };
            let Some(cell) = canvas.at(x, y) else { continue };
            cell.bg = cell.bg.lerp(grain(ink::HILLS, x, y, 0.1) + rim, cover);
            if cover > 0.3 {
                cell.glyph = ' ';
            }
        }
    }
}

/// The moon, low over the far shore: a disc a little dimmer at its edge,
/// with its seas on it, painted in whole cells' backgrounds.
///
/// Its top and its foot fall on the edges of rows, so its first and last
/// rows are mostly moon rather than a row of grey cells across it, which
/// read as a dark band round the disc; and a cell the edge crosses is
/// moon or sky by how much of it the disc covers, mixed only where that
/// is near half, so the edge is clean rather than smudged. A half-block
/// glyph would split a cell finer, but a face draws its blocks to its own
/// line, which at this size is a little shorter than a cell, so a moon of
/// them would show a seam between its rows.
fn moon(canvas: &mut Canvas) {
    let centre = moon_centre(canvas.width);
    let (cx, cy) = ((centre.x / 8.0) as i32, (centre.y / 16.0) as i32);
    let reach = (MOON_RADIUS / 8.0) as i32 + 1;
    for y in cy - reach / 2 - 1..=cy + reach / 2 + 1 {
        for x in cx - reach..=cx + reach {
            // How much of the cell is the moon's, on a four by four grid.
            let inside = (0..16).filter(|i| {
                Vec2::new(x as f32 * 8.0 + (i % 4) as f32 * 2.0 + 1.0, y as f32 * 16.0 + (i / 4) as f32 * 4.0 + 2.0).distance(centre) <= MOON_RADIUS
            });
            let cover = inside.count() as f32 / 16.0;
            let cover = ((cover - 0.2) / 0.6).clamp(0.0, 1.0);
            if cover == 0.0 {
                continue;
            }
            let at = pixel(x, y) - centre;
            let seas = noise(at.x / 22.0 + 5.0, at.y / 22.0 + 5.0);
            let edge = (at.length() / MOON_RADIUS).min(1.0);
            let face = ink::MOON.lerp(ink::MARE, ((seas - 0.42) * 2.4).clamp(0.0, 1.0) * 0.7) * (1.0 - 0.16 * edge * edge);
            let Some(cell) = canvas.at(x, y) else { continue };
            *cell = Paint { glyph: ' ', fg: face, bg: cell.bg.lerp(face, cover) };
        }
    }
}

/// The water the menu stands on, and a margin round it: the picture
/// leaves it dark and puts no glyph in it, so the rows read over nothing.
fn clear_water(width: i32) -> GridRect {
    menu(width).inflate(2)
}

/// The lake: the dusk held under the far shore going to black at the
/// foot of the screen, the moon's light in it round its path, and still
/// but for its ripples, finer and fainter the farther off they are, and
/// none where the menu stands.
fn lake(canvas: &mut Canvas) {
    let depth = (canvas.height - rows::HORIZON).max(1) as f32;
    let clear = clear_water(canvas.width);
    let moon = moon_centre(canvas.width);
    let mirrored = Vec2::new(moon.x, 2.0 * rows::HORIZON as f32 * 16.0 - moon.y);
    for y in rows::HORIZON..canvas.height {
        let near = (y - rows::HORIZON) as f32 / depth;
        let water = ink::WATER_FAR.lerp(ink::WATER_NEAR, near.powf(0.55));
        // The far shore, upside down along the waterline.
        let shore = if y == rows::HORIZON { 0.35 } else { 0.0 };
        for x in 0..canvas.width {
            let shine = ink::HALO * (-pixel(x, y).distance(mirrored) / 150.0).exp() * 0.35 * (1.0 - near);
            let bg = grain(water.lerp(ink::HILLS, shore) + shine, x, y, 0.06);
            let mut paint = Paint { glyph: ' ', fg: bg, bg };
            if !clear.contains(Point::new(x, y)) && hashed(x * 3 + 1, y * 5 + 2) < 0.03 + 0.04 * (1.0 - near) {
                paint.glyph = if near < 0.35 { '─' } else { '~' };
                paint.fg = bg.lerp(ink::RIPPLE, 0.6 - 0.35 * near);
            }
            if let Some(cell) = canvas.at(x, y) {
                *cell = paint;
            }
        }
    }
}

/// The moon's path on the water: under the moon from the far shore to
/// the last row the mist reaches, narrow and bright where it starts, and
/// wider and more broken as it nears, as light on rippled water is. The
/// water under it is lifted a little, and the light itself is glints, a
/// ripple to a cell, long and joined far off and short dashes near.
///
/// It shimmers `t` seconds in: the glints come and go as swells roll in
/// toward the viewer, two rows a second, and the path's edges swell
/// and narrow with them, so the light breaks up and gathers again without
/// the path ever leaving the moon or the rows it lies on.
fn moon_path(canvas: &mut Canvas, t: f32) {
    let moon = moon_centre(canvas.width);
    let length = (rows::MIST_FLOOR - rows::HORIZON + 1) as f32;
    for y in rows::HORIZON..=rows::MIST_FLOOR {
        let down = (y - rows::HORIZON) as f32;
        let swell = (t * 1.3 - down * 0.8).sin();
        let half = (3.5 + down * 1.4) * (1.0 + SWAY * swell);
        let strength = (1.0 - down / length).powf(0.8);
        let reach = half as i32 + 1;
        let middle = (moon.x / 8.0) as i32;
        for x in middle - reach..=middle + reach {
            let off = ((pixel(x, y).x - moon.x) / 8.0 / half).abs();
            if off >= 1.0 {
                continue;
            }
            let column = (1.0 - off * off) * strength;
            let Some(cell) = canvas.at(x, y) else { continue };
            cell.bg = cell.bg.lerp(ink::HALO * 1.4, column * 0.5);
            // Whether this cell catches the moon now: its own ripple, as
            // likely to as the path is bright here on the whole, but more
            // so under a swell drifting in over it and less between, so a
            // glint lasts half a second or so and its neighbours light as it
            // goes out.
            let swells = 0.2 + 1.6 * noise(x as f32 * 0.45, down * 0.9 - t * 1.8);
            let chance = (column * 0.75 + 0.2 * strength) * swells;
            let ripple = hashed(x * 5 + 3, y * 17);
            if ripple > chance {
                continue;
            }
            let bright = (1.0 - ripple / chance).clamp(0.0, 1.0);
            cell.glyph = match (down < 4.0, hashed(x, y * 3) < 0.25 + 0.5 * bright) {
                (true, true) => '━',
                (true, false) => '─',
                (false, true) => '~',
                (false, false) => '-',
            };
            cell.fg = cell.bg.lerp(ink::GLINT, (0.45 + 0.55 * column) * (0.75 + 0.25 * bright));
        }
    }
}

/// How far the moon's path swells past its width and narrows short of
/// it as the swells roll in, a part of its width either way.
const SWAY: f32 = 0.12;

/// UNDERMERE across the sky over the moon: lit silver from above and gone
/// over to violet at its foot, each stroke a shade brighter along its top
/// edge and darker along its bottom one, so the letters stand off the sky.
fn word(canvas: &mut Canvas) {
    for stroke in strokes(canvas.width) {
        let mut rgb = ink::WORD_TOP.lerp(ink::WORD_FOOT, stroke.row as f32 / 6.0);
        if stroke.top {
            rgb *= 1.1;
        }
        if stroke.foot {
            rgb *= 0.82;
        }
        if let Some(cell) = canvas.at(stroke.x, stroke.y) {
            *cell = Paint { glyph: ' ', fg: rgb, bg: grain(rgb, stroke.x, stroke.y, 0.035) };
        }
    }
}

/// One cell of the word: where it stands, how far down its letter, and
/// whether it is the top or the foot of a stroke.
struct Stroke {
    x: i32,
    y: i32,
    row: usize,
    top: bool,
    foot: bool,
}

/// Every cell of UNDERMERE, the word centred across a terminal `width`
/// across, a letter at a time from the left.
fn strokes(width: i32) -> Vec<Stroke> {
    let letters: Vec<&[&str; 7]> = art::WORD.iter().map(|c| art::letter(*c)).collect();
    let span: i32 = letters.iter().map(|l| l[0].chars().count() as i32).sum::<i32>() + art::GAP * (letters.len() as i32 - 1);
    let mut left = (width - span) / 2;
    let mut strokes = Vec::new();
    for letter in letters {
        let solid = |col: usize, row: i32| (0..7).contains(&row) && letter[row as usize].chars().nth(col) == Some('#');
        for (row, line) in letter.iter().enumerate() {
            for (col, _) in line.chars().enumerate().filter(|(_, mark)| *mark == '#') {
                let (top, foot) = (!solid(col, row as i32 - 1), !solid(col, row as i32 + 1));
                strokes.push(Stroke { x: left + col as i32, y: rows::TITLE + row as i32, row, top, foot });
            }
        }
        left += letter[0].chars().count() as i32 + art::GAP;
    }
    strokes
}

/// Mereholt on the near shore: tall gabled houses shoulder to shoulder
/// along the quay with the tower behind them, lamps in their windows, and
/// each lamp's light on the wall round it and, broken by the ripples, on
/// the water under the quay.
fn town(canvas: &mut Canvas) {
    let mut lamps = Vec::new();
    for (row, line) in art::TOWN.iter().enumerate() {
        for (col, mark) in line.chars().enumerate() {
            let (x, y) = (col as i32, rows::TOWN + row as i32);
            let warmth = 0.75 + 0.25 * hashed(x * 5, y * 3);
            let (rgb, cover, glyph) = match mark {
                'H' => (ink::LAND, 1.0, None),
                't' => (ink::LAND * 1.4, 1.0, None),
                'R' => (ink::ROOF, 1.0, None),
                'r' => (ink::ROOF_LIT, 1.0, None),
                '/' => (ink::ROOF, 0.5, None),
                '\\' => (ink::ROOF_LIT, 0.5, None),
                'W' => (ink::WALL, 1.0, None),
                'w' => (ink::WALL_LIT, 1.0, None),
                'T' => (ink::STONEWORK, 1.0, None),
                // The quay's top course catches the moon.
                'Q' if row > 0 && art::TOWN[row - 1].chars().nth(col) != Some('Q') => (ink::STONEWORK * 1.8, 1.0, None),
                'Q' => (ink::STONEWORK, 1.0, None),
                'o' | 'D' => (ink::WINDOW * warmth, 1.0, Some(('┼', ink::SASH))),
                'x' => (ink::WALL * 0.7, 1.0, Some(('┼', ink::WALL * 1.6))),
                _ => continue,
            };
            if matches!(mark, 'o' | 'D') {
                lamps.push((x, y, warmth));
            }
            let Some(cell) = canvas.at(x, y) else { continue };
            cell.bg = cell.bg.lerp(grain(rgb, x, y, 0.08), cover);
            (cell.glyph, cell.fg) = glyph.unwrap_or((' ', cell.bg));
        }
    }
    for (x, y, warmth) in lamps {
        // The wall round a window, warmed by it.
        for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            if let Some(cell) = canvas.at(x + dx, y + dy).filter(|c| c.glyph == ' ') {
                cell.bg += ink::WINDOW * 0.05 * warmth;
            }
        }
        // Its light on the water, a broken streak under it that fades as
        // it comes nearer.
        for y in rows::QUAY + 1..rows::QUAY + 8 {
            let fade = 1.0 - (y - rows::QUAY) as f32 / 8.0;
            if hashed(x * 11, y * 7) < 0.45 {
                continue;
            }
            if let Some(cell) = canvas.at(x, y) {
                cell.bg = cell.bg.lerp(ink::WINDOW * warmth, 0.08 * fade);
                cell.glyph = if hashed(x, y * 5) < 0.5 { '~' } else { '-' };
                cell.fg = cell.bg.lerp(ink::WINDOW * warmth, 0.55 * fade);
            }
        }
    }
}

/// The Drowned Gate on its island: a ring of standing stones round a
/// gate of two stones and a lintel, the glow in the gate's doorway and
/// its light on everything near, the stones behind lit full on, the
/// near ones dark against it but at their edges, and the glow again on
/// the water under the island.
fn gate(canvas: &mut Canvas, glow: f32) {
    let ring = canvas.width - art::RING_FROM_RIGHT;
    let heart = gate_heart(canvas.width);
    // The glow's light on the air, the water and the island's turf, laid
    // before the stones, which take theirs by the way they face.
    for at in glow_reach(canvas.width).cells() {
        let light = glow_light(pixel(at.x, at.y), heart, glow);
        if let Some(cell) = canvas.at(at.x, at.y) {
            cell.bg += ink::GLOW * light * 0.5;
            cell.fg += ink::GLOW * light * 0.5;
        }
    }
    island(canvas, ring, heart, glow);
    for stone in art::RING.iter().filter(|s| !s.near) {
        standing_stone(canvas, ring, stone, heart, glow);
    }
    // The doorway, and the heart of the glow in it: near white at the
    // middle going to violet at the stones, a little brighter with each
    // breath, over whatever the doorway shows of the island behind.
    let door = art::DOOR;
    for y in door.y..door.bottom() {
        for x in ring + door.x..ring + door.right() {
            let off = (pixel(x, y) - heart) / Vec2::new(door.width as f32 * 4.0, door.height as f32 * 8.0);
            let core = (1.0 - off.length()).max(0.0);
            if let Some(cell) = canvas.at(x, y) {
                let rgb = cell.bg + ink::GLOW * glow * 0.3 + ink::HEART * core.powf(1.4) * glow;
                *cell = Paint { glyph: ' ', fg: rgb, bg: rgb };
            }
        }
    }
    for stone in art::RING.iter().filter(|s| s.near) {
        standing_stone(canvas, ring, stone, heart, glow);
    }
    // The glow on the water, broken by the ripples like the moon's.
    for y in rows::ISLAND + 1..rows::MIST_FLOOR {
        let fade = 1.0 - (y - rows::ISLAND) as f32 / (rows::MIST_FLOOR - rows::ISLAND) as f32;
        for x in ring + door.x - 1..ring + door.right() + 1 {
            if hashed(x * 7, y * 5 + 1) < 0.4 {
                continue;
            }
            if let Some(cell) = canvas.at(x, y) {
                cell.bg += ink::GLOW * glow * 0.22 * fade;
                cell.glyph = '─';
                cell.fg = cell.bg.lerp(ink::HEART, 0.6 * fade * glow);
            }
        }
    }
}

/// Everything the gate's glow lights, on a terminal `width` across: the
/// island, its stones, the air over them and the water under them, and
/// nothing past it, so the breath is seen nowhere else.
fn glow_reach(width: i32) -> GridRect {
    let (ring, top) = (width - art::RING_FROM_RIGHT, rows::HORIZON - 9);
    GridRect::new(ring - 34, top, 68, rows::MIST_FLOOR - top)
}

/// Where the glow's heart is, in pixels: the middle of the doorway.
fn gate_heart(width: i32) -> Vec2 {
    let door = art::DOOR;
    let ring = width - art::RING_FROM_RIGHT;
    Vec2::new((ring + door.x) as f32 * 8.0 + door.width as f32 * 4.0, door.y as f32 * 16.0 + door.height as f32 * 8.0)
}

/// How much of the glow reaches `at`, in pixels, from the heart: a bright
/// pool round the gate and a long faint reach past it, both rising and
/// falling with its breath.
fn glow_light(at: Vec2, heart: Vec2, glow: f32) -> f32 {
    let d = at.distance(heart);
    glow * (0.6 * (-(d / 80.0).powi(2)).exp() + 0.25 * (-d / 170.0).exp())
}

/// The island: a low mound of turf on the water, rising to its middle,
/// with the water dark under it where it is mirrored.
fn island(canvas: &mut Canvas, ring: i32, heart: Vec2, glow: f32) {
    let reach = art::ISLAND_HALF_WIDTH;
    for x in ring - reach..=ring + reach {
        let across = (x - ring) as f32 / reach as f32;
        let rise = 5.2 * (1.0 - across * across).max(0.0).powf(0.6);
        let top = rows::ISLAND as f32 + 1.0 - rise;
        for y in top.floor() as i32..=rows::ISLAND {
            let cover = (y as f32 + 1.0 - top).clamp(0.0, 1.0);
            let light = glow_light(pixel(x, y), heart, glow);
            let turf = grain(ink::TURF, x, y, 0.15) + ink::GLOW * light * 0.7;
            if let Some(cell) = canvas.at(x, y) {
                cell.bg = cell.bg.lerp(turf, cover);
                if cover > 0.5 {
                    (cell.glyph, cell.fg) = if hashed(x * 3, y) < 0.3 { ('"', turf * 1.6) } else { (' ', turf) };
                }
            }
        }
        // The island upside down on the water: a few rows darker.
        for y in rows::ISLAND + 1..rows::ISLAND + 1 + (rise * 0.6) as i32 {
            if let Some(cell) = canvas.at(x, y) {
                cell.bg *= 0.7;
            }
        }
    }
}

/// One stone of the ring, `stone` placed against the ring's middle
/// column: grey where the glow does not reach it, violet where it does,
/// the near stones dark but for the edge toward the glow, and every stone
/// a hair of moonlight down its left side, which faces the moon.
fn standing_stone(canvas: &mut Canvas, ring: i32, stone: &art::Stone, heart: Vec2, glow: f32) {
    for row in 0..stone.height {
        let y = stone.foot - row;
        // Its top leans, and a worn top is a cell narrower each side.
        let shift = stone.lean * row / stone.height.max(1);
        let crown = i32::from(stone.worn && row == stone.height - 1);
        for col in crown..stone.width - crown {
            let x = ring + stone.x + col + shift;
            let at = pixel(x, y);
            let light = glow_light(at, heart, glow);
            // The edge toward the glow takes it full on; a far stone's
            // face, which looks across the ring at the glow, takes some;
            // a near stone's face, turned from it to the viewer, next to
            // none.
            let toward = (col == crown && heart.x < at.x) || (col == stone.width - 1 - crown && heart.x > at.x);
            let facing = match (stone.near, toward) {
                (_, true) => 0.95,
                (false, false) => 0.35,
                (true, false) => 0.05,
            };
            let mut rgb = if stone.near { ink::STONE_NEAR } else { ink::STONE } + ink::GLOW * light * facing;
            // The moon is off to the left: a hair of it down that side.
            if col == crown {
                rgb += ink::HALO * 0.35;
            }
            let rgb = grain(rgb, x, y, 0.12);
            if let Some(cell) = canvas.at(x, y) {
                *cell = Paint { glyph: if hashed(x * 13, y * 7) < 0.15 { '░' } else { ' ' }, fg: rgb * 1.25, bg: rgb };
            }
        }
    }
}

/// The mist, drifting left to right low over the water: a bank along the
/// far shore and a thinner one over the near water, the near one the
/// quicker, as nearer things seem to move, and both gone two rows above
/// the menu. It takes the light of what it passes: silver over the moon's
/// path, violet round the gate.
fn mist(canvas: &mut Canvas, t: f32, glow: f32) {
    let moon = moon_centre(canvas.width);
    let heart = gate_heart(canvas.width);
    for y in rows::MIST_TOP..=rows::MIST_FLOOR {
        let fy = y as f32;
        let far = (-((fy - 21.5) / 1.1).powi(2)).exp();
        let near = (-((fy - 28.0) / 1.4).powi(2)).exp() * ((rows::MIST_FLOOR as f32 + 1.0 - fy) / 3.0).clamp(0.0, 1.0);
        for x in 0..canvas.width {
            let fx = x as f32;
            // How thick a bank is here: long wisps drifting at `speed`
            // cells a second, a row or so deep, the gaps between them
            // clear.
            let bank = |speed: f32, seed: f32| {
                let wisp = noise((fx - t * speed) * 0.06 + seed, fy * 0.3 + seed) * 0.65 + noise((fx - t * speed * 1.4) * 0.16 + seed, fy * 0.55 + seed) * 0.35;
                ((wisp - 0.4) / 0.35).clamp(0.0, 1.0)
            };
            let thick = far * bank(0.6, 0.0) * 0.32 + near * bank(1.4, 31.0) * 0.26;
            if thick <= 0.005 {
                continue;
            }
            let at = pixel(x, y);
            let silver = ink::HALO * (-(at.x - moon.x).abs() / 120.0).exp() * 0.8;
            let lit = ink::MIST + silver + ink::GLOW * glow_light(at, heart, glow) * 0.9;
            if let Some(cell) = canvas.at(x, y) {
                cell.bg = cell.bg.lerp(lit, thick);
                cell.fg = cell.fg.lerp(lit, thick);
            }
        }
    }
}

/// The shapes the picture is drawn from: the word's letters and the town
/// as rows of marks, a cell to a mark and a space whatever is behind, and
/// the gate as the stones it is built of.
mod art {
    /// The word, a letter at a time.
    pub const WORD: [char; 9] = ['U', 'N', 'D', 'E', 'R', 'M', 'E', 'R', 'E'];

    /// Cells between one letter and the next.
    pub const GAP: i32 = 3;

    /// One letter of the word, seven rows tall: each stem two cells wide
    /// and each bar a row deep, so a stroke is as thick across as down,
    /// and every edge on a cell's edge. A half cell on a diagonal reads as
    /// a grey smudge at this size rather than as half a stroke.
    pub fn letter(which: char) -> &'static [&'static str; 7] {
        match which {
            'U' => &["##        ##", "##        ##", "##        ##", "##        ##", "##        ##", "##        ##", "  ########  "],
            'N' => &["###      ##", "####     ##", "## ##    ##", "##  ##   ##", "##   ##  ##", "##    ## ##", "##     ####"],
            'D' => &["##########  ", "##        ##", "##        ##", "##        ##", "##        ##", "##        ##", "##########  "],
            'E' => &["##########", "##        ", "##        ", "########  ", "##        ", "##        ", "##########"],
            'R' => &["##########  ", "##        ##", "##        ##", "##########  ", "##    ##    ", "##      ##  ", "##        ##"],
            'M' => &["###        ###", "####      ####", "## ##    ## ##", "##  ##  ##  ##", "##   ####   ##", "##    ##    ##", "##          ##"],
            _ => &["", "", "", "", "", "", ""],
        }
    }

    /// Mereholt, from the tower's tip down to the quay, its left edge on
    /// the screen's. `H` is the land and `t` a tree on it; `R` and `r` a
    /// roof turned from the moon and to it, `/` and `\` half of each where
    /// a slope crosses its cell; `W` and `w` a wall the same two ways; `T`
    /// the tower's stone and `Q` the quay's; `o` a lit window, `D` a lit
    /// door and `x` a dark window.
    pub const TOWN: [&str; 17] = [
        "                     /\\                                 ",
        "                    /Rr\\                                ",
        "            /\\     /RRrr\\         /\\                    ",
        "  /\\       /Rr\\   /RRRrrr\\       /Rr\\                   ",
        " /Rr\\     /Rowr\\    TTTT        /RWwr\\                  ",
        "/RWwr\\ttt/RWWwwr\\   TooT       /RWowwr\\                 ",
        "RWWowr\\t/RWWWwwwr\\  T/\\T /RRRR/RWWWwwwr\\    /\\          ",
        "WWWwwwr\\WoWWxWWoWw  /Rr\\ rrrr/RWWWWwwwwr\\  /Rr\\         ",
        "WWWWWWWwWWWWWWWWWwH/Rxwr\\WWWWWoWWoWWxWWow /Rowr\\   /\\   ",
        "oWWxWWowWoWWoWWxWw/RWWwwr\\xWWWWWWWWWWWWWw/RWWwwr\\ /Rr\\  ",
        "WWWWWWWwWWWWWWWWWwWWWWWWWwWWWWoWWxWWoWWowWWWWWWWw/RWwr\\ ",
        "xWWoWWowWxWWoWWoWwWoWWoWWwWWWWWWWWWWWWWWwWxWWoWWwWWWWWWw",
        "WWWWWWWwWWWWWWWWWwWWWWWWWwWWWWxWWoWWoWWxwWWWWWWWwWoWWxWw",
        "oWWWWWWwWWWWWWWWWwWxWWWWWwWWWWWWWWWWWWWWwWWWWWWWwWWWWWWw",
        "WWWDWWWwWWWWDWWWWwWWWWWWWwWWWWWWWWWDWWWWwWWWDWWWwWxxxWWw",
        "QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ",
        "QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ      ",
    ];

    /// How far in from the screen's right edge the ring's middle column
    /// stands.
    pub const RING_FROM_RIGHT: i32 = 31;

    /// How far the island reaches either side of the ring's middle.
    pub const ISLAND_HALF_WIDTH: i32 = 26;

    /// A stone of the gate: its leftmost column against the ring's middle,
    /// the row of its foot, how wide and tall it stands and how far its
    /// top leans, whether its top is worn round, and whether it stands
    /// between the viewer and the glow.
    pub struct Stone {
        pub x: i32,
        pub foot: i32,
        pub width: i32,
        pub height: i32,
        pub lean: i32,
        pub worn: bool,
        pub near: bool,
    }

    /// The gate's doorway against the ring's middle column, where its
    /// glow is: between the two stones that hold the lintel.
    pub const DOOR: atglyph::primitives::GridRect = atglyph::primitives::GridRect { x: -2, y: 17, width: 5, height: 8 };

    /// The ring as it is seen from Mereholt's shore: a ring of stones on
    /// the island seen from low across the water, so the far ones stand
    /// higher in the gaps the near ones leave, and in its middle the gate,
    /// two stones and a lintel. The far stones and the gate are painted
    /// before the glow in the doorway, the near ones after it.
    pub const RING: [Stone; 12] = [
        Stone { x: -13, foot: 24, width: 3, height: 5, lean: 0, worn: true, near: false },
        Stone { x: 11, foot: 24, width: 3, height: 5, lean: 0, worn: true, near: false },
        Stone { x: -22, foot: 26, width: 3, height: 6, lean: 0, worn: true, near: false },
        Stone { x: 19, foot: 26, width: 3, height: 4, lean: 0, worn: true, near: false },
        Stone { x: -5, foot: 24, width: 3, height: 8, lean: 0, worn: false, near: false },
        Stone { x: 3, foot: 24, width: 3, height: 8, lean: 0, worn: false, near: false },
        Stone { x: -6, foot: 16, width: 13, height: 1, lean: 0, worn: true, near: false },
        Stone { x: -18, foot: 28, width: 4, height: 7, lean: 0, worn: true, near: true },
        Stone { x: -10, foot: 29, width: 4, height: 7, lean: 0, worn: true, near: true },
        Stone { x: 6, foot: 29, width: 4, height: 5, lean: 0, worn: true, near: true },
        Stone { x: 14, foot: 28, width: 4, height: 7, lean: 1, worn: true, near: true },
        Stone { x: 21, foot: 29, width: 5, height: 1, lean: 0, worn: true, near: true },
    ];
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    /// Undermere's screen in cells.
    const COLS: i32 = 160;
    const ROWS: i32 = 45;

    /// What no picture paints: a cell the test lays under the picture, so
    /// any cell still showing it afterwards is a cell the picture left.
    fn unpainted() -> Cell {
        Cell::new('?', Color::srgb(1.0, 0.0, 1.0)).on(Color::srgb(1.0, 0.0, 1.0))
    }

    /// The system itself, run `t` seconds in over a terminal of
    /// [`unpainted`] cells.
    fn painted(t: f32) -> Terminal {
        let mut world = World::new();
        let mut terminal = Terminal::new(COLS, ROWS, Vec2::new(8.0, 16.0));
        terminal.fill(terminal.bounds(), unpainted());
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(t));
        world.insert_resource(terminal);
        world.insert_resource(time);
        world.run_system_once(paint_backdrop).expect("the system runs");
        world.remove_resource::<Terminal>().expect("the terminal is still there")
    }

    /// The picture is the whole screen: at every moment of a minute of
    /// its motion, every cell has had its glyph and both its colours
    /// painted, so no corner of the window shows what was there before,
    /// a run's last frame or the engine's surface, as a hole in the night.
    #[test]
    fn the_backdrop_paints_every_cell_of_the_terminal() {
        let before = unpainted();
        for tenth in (0..600).step_by(7) {
            let terminal = painted(tenth as f32 / 10.0);
            let left: Vec<_> = terminal
                .bounds()
                .cells()
                .filter(|at| terminal.get(at.x, at.y).is_some_and(|c| c.glyph == before.glyph || c.fg == before.fg || c.bg == before.bg))
                .collect();
            assert!(left.is_empty(), "{} seconds in, {} cells are left unpainted, the first {:?}", tenth as f32 / 10.0, left.len(), left.first());
        }
    }

    /// Every glyph the picture draws, at every moment of a minute of its
    /// motion, is one the typeface has, read from the face's own character
    /// map. A glyph it has not is drawn from whatever face the system falls
    /// back on, or as an empty box, and in a browser always as the box.
    #[test]
    fn every_glyph_the_picture_draws_is_one_the_typeface_has() {
        let face = crate::typeface::face();
        for tenth in (0..600).step_by(3) {
            let terminal = painted(tenth as f32 / 10.0);
            let drawn: std::collections::BTreeSet<char> = terminal.bounds().cells().filter_map(|at| terminal.get(at.x, at.y)).map(|c| c.glyph).collect();
            let lacking: Vec<char> = drawn.into_iter().filter(|g| face.glyph_index(*g).is_none()).collect();
            assert!(lacking.is_empty(), "{} seconds in, the picture draws {lacking:?}, which the typeface has not", tenth as f32 / 10.0);
        }
    }

    /// How bright `colour` looks, from 0 for black to 1 for white.
    fn brightness(colour: Color) -> f32 {
        let c = colour.to_srgba();
        0.2126 * c.red + 0.7152 * c.green + 0.0722 * c.blue
    }

    /// The engine draws its menu over the picture, and the rows read
    /// because what is under them is clean dark water: at every moment of
    /// a minute of the picture's motion, no cell of the menu's rectangle
    /// holds a glyph or anything brighter than the lake's far water.
    #[test]
    fn the_menu_stands_on_clean_dark_water_at_every_moment_of_the_motion() {
        let (menu, dusk) = (menu(COLS), brightness(colour(ink::WATER_FAR)));
        for tenth in (0..600).step_by(3) {
            let terminal = painted(tenth as f32 / 10.0);
            let lit: Vec<_> = menu.cells().filter(|at| terminal.get(at.x, at.y).is_some_and(|c| c.glyph != ' ' || brightness(c.bg) >= dusk)).collect();
            assert!(lit.is_empty(), "{} seconds in, the picture lights the menu's water at {lit:?}", tenth as f32 / 10.0);
        }
    }

    /// UNDERMERE is whole at every moment: every cell of every letter is
    /// drawn in the word's light, bright and bare of any glyph, and the
    /// same as at launch, so no star, mist or roof is ever over it.
    #[test]
    fn the_word_is_whole_at_every_moment_of_the_motion() {
        let strokes = strokes(COLS);
        assert_eq!(
            strokes.len(),
            art::WORD.iter().map(|c| art::letter(*c).concat().matches('#').count()).sum::<usize>(),
            "every mark of the word is laid on the screen"
        );
        let launch = painted(0.0);
        for tenth in (0..600).step_by(11) {
            let terminal = painted(tenth as f32 / 10.0);
            for Stroke { x, y, .. } in &strokes {
                let cell = terminal.get(*x, *y).expect("the word is on the screen");
                assert!(cell.glyph == ' ' && brightness(cell.bg) > 0.3, "{} seconds in, the word at ({x}, {y}) is {cell:?}", tenth as f32 / 10.0);
                assert_eq!(Some(cell), launch.get(*x, *y), "{} seconds in, the word at ({x}, {y}) has changed", tenth as f32 / 10.0);
            }
        }
    }

    /// Four things move and only four: between launch and any moment of
    /// the first minute, every cell that has changed is a star, in the
    /// gate's reach, in the moon's path, or in the rows the mist drifts
    /// through. The word, the moon, the town, the near water and the
    /// menu's dark are still.
    #[test]
    fn only_the_stars_the_glow_the_mist_and_the_moons_path_move() {
        let (launch, reach, path) = (painted(0.0), glow_reach(COLS), path_reach(COLS));
        let mut moved = 0;
        for tenth in (1..600).step_by(13) {
            let terminal = painted(tenth as f32 / 10.0);
            for at in terminal.bounds().cells().filter(|at| terminal.get(at.x, at.y) != launch.get(at.x, at.y)) {
                moved += 1;
                let mist = (rows::MIST_TOP..=rows::MIST_FLOOR).contains(&at.y);
                assert!(
                    is_star(at.x, at.y) || reach.contains(at) || path.contains(at) || mist,
                    "{} seconds in, ({}, {}) moved",
                    tenth as f32 / 10.0,
                    at.x,
                    at.y
                );
            }
        }
        assert!(moved > 0, "and the picture does move");
    }

    /// Everything the moon's path lights at its widest, on a terminal `width`
    /// across, so its shimmer is seen nowhere else.
    fn path_reach(width: i32) -> GridRect {
        let middle = (moon_centre(width).x / 8.0) as i32;
        let reach = ((3.5 + (rows::MIST_FLOOR - rows::HORIZON) as f32 * 1.4) * (1.0 + SWAY)) as i32 + 1;
        GridRect::new(middle - reach, rows::HORIZON, reach * 2 + 1, rows::MIST_FLOOR - rows::HORIZON + 1)
    }

    /// The glints on the moon's path, as a set of cells, at `t` seconds:
    /// the path alone, on a blank canvas, so no ripple of the lake's is
    /// counted as one.
    fn glints(t: f32) -> std::collections::BTreeSet<(i32, i32)> {
        let mut canvas = Canvas::new(COLS, ROWS);
        moon_path(&mut canvas, t);
        path_reach(COLS).cells().filter(|at| canvas.at(at.x, at.y).is_some_and(|c| c.glyph != ' ')).map(|at| (at.x, at.y)).collect()
    }

    /// The moon's path shimmers and stays a path: from one moment to the
    /// next a share of its glints go out and others light, but at every
    /// moment of a minute every row of it from the far shore down still
    /// holds some, so the light breaks up without the path ever going.
    #[test]
    fn the_moons_path_shimmers_without_ever_going_out() {
        let mut before = glints(0.0);
        for tenth in (5..600).step_by(5) {
            let now = glints(tenth as f32 / 10.0);
            for y in rows::HORIZON..=rows::MIST_FLOOR {
                assert!(now.iter().any(|g| g.1 == y), "{} seconds in, row {y} of the path is dark", tenth as f32 / 10.0);
            }
            let changed = now.symmetric_difference(&before).count();
            assert!(changed * 10 >= now.len(), "{} seconds in, only {changed} of {} glints changed in half a second", tenth as f32 / 10.0, now.len());
            before = now;
        }
    }
}
