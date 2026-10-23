//! Box-drawing and block characters drawn as shapes that fill their cell.
//!
//! A font draws `─` and `█` at its own size, narrower than the cell and
//! short of its top and bottom, so a frame drawn in them is a row of
//! dashes with its corners standing apart, and a bar is a row of blocks
//! with dark seams between them. A terminal emulator draws these two
//! Unicode blocks itself, a line running to the cell's edges where it
//! meets the next and a block filling the cell, and so does the glyph
//! terminal: every character from U+2500 to U+259F is drawn here, at the
//! cell's size in the display's own pixels, onto one sheet, and a cell
//! showing one shows its picture from the sheet in the cell's colour, as
//! a tileset's picture is shown.
//!
//! All 160 of them rather than the few the engine's own panels use, since
//! a game's title in box art or a tile drawn as a shade meets the same
//! seams, and a font that lacks them, as the one Bevy embeds for the
//! browser does, then no longer matters.
//!
//! A shade is a dot pattern, not a tint: gas thick enough to hide behind
//! is drawn in `░` and `▒`, and a tint of the same colour reads as a lit
//! or a bare tile rather than as haze. Its dots are about as wide as a
//! light line, so the pattern reads at any cell size and any display
//! scaling, on a grid of an even number of them each way, so a cell's
//! dots carry on into its neighbor's as if the pattern were one: a
//! quarter of the dots for `░`, every other one for `▒`, and all but a
//! quarter for `▓`.
//!
//! A glyph drawn across a run of cells, a map tile two cells wide, is
//! drawn once across the run with the strokes of one cell: a line keeps
//! its weight rather than doubling in width, a horizontal line runs edge
//! to edge so it meets the next tile's, a vertical one stands in the
//! middle of the tile as a glyph centred on it would, and a block or a
//! shade fills the tile as it fills a cell, since the run is one tile
//! drawn as one cell.
//!
//! Pure arithmetic on pixels, with no `App`, so every shape is a test.

use bevy::asset::RenderAssetUsages;
use bevy::image::{Image, ImageSampler, TextureAtlasLayout};
use bevy::math::UVec2;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

/// The first character drawn here.
const FIRST: u32 = 0x2500;
/// How many follow it: the Box Drawing block and the Block Elements block.
const COUNT: u32 = 160;
/// Columns of the sheet: sixteen, so a character's row and column are the
/// last two hex digits of its code.
const COLUMNS: u32 = 16;

/// Where `glyph` is on the sheet, or `None` when it is drawn by the font.
pub(crate) fn index(glyph: char) -> Option<usize> {
    let code = u32::from(glyph);
    (FIRST..FIRST + COUNT).contains(&code).then(|| (code - FIRST) as usize)
}

/// The sheet every character is drawn on for a run of `span` cells of
/// `cell` pixels, white where it is drawn so the sprite showing it takes
/// the cell's colour, and how it is cut up.
///
/// Sampled nearest: the sheet is drawn at the size it is shown, so a
/// filter would only blur a cell's edge into its neighbor's on the sheet.
pub(crate) fn sheet(cell: UVec2, span: u32) -> (Image, TextureAtlasLayout) {
    let tile = UVec2::new(cell.x * span.max(1), cell.y);
    let rows = COUNT.div_ceil(COLUMNS);
    let (width, height) = (tile.x * COLUMNS, tile.y * rows);
    let mut data = vec![0u8; (width * height * 4) as usize];
    for i in 0..COUNT {
        let glyph = char::from_u32(FIRST + i).expect("the two blocks are characters");
        let alpha = coverage(glyph, cell, span).expect("every character of the two blocks is drawn");
        let (left, top) = ((i % COLUMNS) * tile.x, (i / COLUMNS) * tile.y);
        for y in 0..tile.y {
            for x in 0..tile.x {
                let at = (((top + y) * width + left + x) * 4) as usize;
                data[at..at + 4].copy_from_slice(&[255, 255, 255, alpha[(y * tile.x + x) as usize]]);
            }
        }
    }
    let size = Extent3d { width, height, depth_or_array_layers: 1 };
    let mut image = Image::new(size, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::default());
    image.sampler = ImageSampler::nearest();
    (image, TextureAtlasLayout::from_grid(tile, COLUMNS, rows, None, None))
}

/// How much of each pixel `glyph` covers across a run of `span` cells of
/// `cell` pixels, from 0 to 255, row by row from the top; `None` for a
/// character drawn by the font.
pub(crate) fn coverage(glyph: char, cell: UVec2, span: u32) -> Option<Vec<u8>> {
    let code = u32::from(glyph);
    index(glyph)?;
    let mut canvas = Canvas::new(cell, span);
    match code {
        0x2580..=0x259F => canvas.block(code),
        0x2504..=0x250B | 0x254C..=0x254F => canvas.dashes(code),
        0x256D..=0x2570 => canvas.arc(code),
        0x2571..=0x2573 => canvas.diagonals(code),
        _ => canvas.lines(arms(code)),
    }
    Some(canvas.alpha)
}

/// How a line leaves the middle of a cell toward one edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    None,
    Light,
    Heavy,
    Double,
}

/// The four arms of a line character, up, right, down and left, read from
/// its row of [`ARMS`].
fn arms(code: u32) -> [Line; 4] {
    let row = ARMS[(code - FIRST) as usize].as_bytes();
    std::array::from_fn(|i| match row[i] {
        b'l' => Line::Light,
        b'h' => Line::Heavy,
        b'd' => Line::Double,
        _ => Line::None,
    })
}

/// The arms of U+2500 to U+257F, up, right, down and left: `l` light, `h`
/// heavy, `d` double, a space none. Empty for the dashed lines, the arcs
/// and the diagonals, which are drawn otherwise.
#[rustfmt::skip]
const ARMS: [&str; 128] = [
    // ─ ━ │ ┃ ┄ ┅ ┆ ┇
    " l l", " h h", "l l ", "h h ", "", "", "", "",
    // ┈ ┉ ┊ ┋ ┌ ┍ ┎ ┏
    "", "", "", "", " ll ", " hl ", " lh ", " hh ",
    // ┐ ┑ ┒ ┓ └ ┕ ┖ ┗
    "  ll", "  lh", "  hl", "  hh", "ll  ", "lh  ", "hl  ", "hh  ",
    // ┘ ┙ ┚ ┛ ├ ┝ ┞ ┟
    "l  l", "l  h", "h  l", "h  h", "lll ", "lhl ", "hll ", "llh ",
    // ┠ ┡ ┢ ┣ ┤ ┥ ┦ ┧
    "hlh ", "hhl ", "lhh ", "hhh ", "l ll", "l lh", "h ll", "l hl",
    // ┨ ┩ ┪ ┫ ┬ ┭ ┮ ┯
    "h hl", "h lh", "l hh", "h hh", " lll", " llh", " hll", " hlh",
    // ┰ ┱ ┲ ┳ ┴ ┵ ┶ ┷
    " lhl", " lhh", " hhl", " hhh", "ll l", "ll h", "lh l", "lh h",
    // ┸ ┹ ┺ ┻ ┼ ┽ ┾ ┿
    "hl l", "hl h", "hh l", "hh h", "llll", "lllh", "lhll", "lhlh",
    // ╀ ╁ ╂ ╃ ╄ ╅ ╆ ╇
    "hlll", "llhl", "hlhl", "hllh", "hhll", "llhh", "lhhl", "hhlh",
    // ╈ ╉ ╊ ╋ ╌ ╍ ╎ ╏
    "lhhh", "hlhh", "hhhl", "hhhh", "", "", "", "",
    // ═ ║ ╒ ╓ ╔ ╕ ╖ ╗
    " d d", "d d ", " dl ", " ld ", " dd ", "  ld", "  dl", "  dd",
    // ╘ ╙ ╚ ╛ ╜ ╝ ╞ ╟
    "ld  ", "dl  ", "dd  ", "l  d", "d  l", "d  d", "ldl ", "dld ",
    // ╠ ╡ ╢ ╣ ╤ ╥ ╦ ╧
    "ddd ", "l ld", "d dl", "d dd", " dld", " ldl", " ddd", "ld d",
    // ╨ ╩ ╪ ╫ ╬ ╭ ╮ ╯
    "dl l", "dd d", "ldld", "dldl", "dddd", "", "", "",
    // ╰ ╱ ╲ ╳ ╴ ╵ ╶ ╷
    "", "", "", "", "   l", "l   ", " l  ", "  l ",
    // ╸ ╹ ╺ ╻ ╼ ╽ ╾ ╿
    "   h", "h   ", " h  ", "  h ", " h l", "l h ", " l h", "h l ",
];

/// How wide a line of `line` weight is, where a light one is `light`.
fn weight(light: i32, line: Line) -> i32 {
    match line {
        Line::None => 0,
        Line::Light => light,
        Line::Heavy => 2 * light,
        Line::Double => 3 * light,
    }
}

/// A span of pixels along one axis, `start` up to but not including `end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Span {
    start: i32,
    end: i32,
}

impl Span {
    /// `width` pixels centred in `length`, floored toward the start, so a
    /// line in one cell lands on the same pixels as the same line in the
    /// next.
    fn centred(width: i32, length: i32) -> Self {
        let start = (length - width).div_euclid(2);
        Self { start, end: start + width }
    }

    fn union(self, other: Self) -> Self {
        Self { start: self.start.min(other.start), end: self.end.max(other.end) }
    }
}

/// One cell's pixels, and the line widths drawn in it.
struct Canvas {
    width: i32,
    height: i32,
    alpha: Vec<u8>,
    /// A light line's width: about a ninth of one cell, however many the
    /// run is, so a line keeps to the weight of a font's strokes at any
    /// cell size, and never under a pixel.
    light: i32,
    /// How many cells across the run is.
    span: i32,
    /// One cell's height and width, the unit a shade's dots are counted in.
    cell: (i32, i32),
}

impl Canvas {
    fn new(cell: UVec2, span: u32) -> Self {
        let span = span.max(1) as i32;
        let (cell_width, height) = (cell.x.max(1) as i32, cell.y.max(1) as i32);
        let width = cell_width * span;
        let light = ((cell_width as f32 / 9.0).round() as i32).max(1);
        Self { width, height, alpha: vec![0; (width * height) as usize], light, span, cell: (cell_width, height) }
    }

    /// How wide a line of `line` weight is: a heavy one twice a light one,
    /// a double one two light strokes a light stroke apart.
    fn weight(&self, line: Line) -> i32 {
        weight(self.light, line)
    }

    /// Covers the pixels in columns `x` and rows `y` by `alpha`, never
    /// lightening one already covered more.
    fn fill(&mut self, x: Span, y: Span, alpha: u8) {
        for py in y.start.max(0)..y.end.min(self.height) {
            for px in x.start.max(0)..x.end.min(self.width) {
                let at = (py * self.width + px) as usize;
                self.alpha[at] = self.alpha[at].max(alpha);
            }
        }
    }

    /// Uncovers the pixels in columns `x` and rows `y`.
    fn clear(&mut self, x: Span, y: Span) {
        for py in y.start.max(0)..y.end.min(self.height) {
            for px in x.start.max(0)..x.end.min(self.width) {
                self.alpha[(py * self.width + px) as usize] = 0;
            }
        }
    }

    /// Covers each pixel by the share of sixteen points spread over it that
    /// `inside` holds, so a curve or a slant has smooth edges where a
    /// rectangle has sharp ones.
    fn shape(&mut self, inside: impl Fn(f32, f32) -> bool) {
        const STEPS: i32 = 4;
        for py in 0..self.height {
            for px in 0..self.width {
                let mut hits = 0;
                for sy in 0..STEPS {
                    for sx in 0..STEPS {
                        let (x, y) = (px as f32 + (sx as f32 + 0.5) / STEPS as f32, py as f32 + (sy as f32 + 0.5) / STEPS as f32);
                        hits += i32::from(inside(x, y));
                    }
                }
                let alpha = (hits * 255 / (STEPS * STEPS)) as u8;
                let at = (py * self.width + px) as usize;
                self.alpha[at] = self.alpha[at].max(alpha);
            }
        }
    }

    fn all_x(&self) -> Span {
        Span { start: 0, end: self.width }
    }

    fn all_y(&self) -> Span {
        Span { start: 0, end: self.height }
    }

    /// The four arms of a line character, each from its edge to the middle
    /// and across whatever crosses it there, so a corner closes and a tee
    /// meets its stem.
    ///
    /// A double arm is drawn as a band three strokes wide with its middle
    /// stroke taken out again, which leaves the joins of `╔` `╠` and `╬`
    /// as their outlines. A single arm is drawn after, so one that ends at
    /// a double line, as `╒` ends, closes it; one that meets a double line
    /// running straight through, as `╟` meets it, stops at the near
    /// stroke instead of crossing the gap.
    fn lines(&mut self, [up, right, down, left]: [Line; 4]) {
        let (w, h) = (self.width, self.height);
        let middle = |length: i32| Span { start: length / 2, end: length / 2 };
        let light = self.light;
        let band = move |line: Line, length: i32| Span::centred(weight(light, line), length);
        // The width of whatever runs up and down through the middle, and
        // of whatever runs across it.
        let vertical = [up, down].into_iter().filter(|&l| l != Line::None).map(|l| band(l, w)).reduce(Span::union).unwrap_or(middle(w));
        let horizontal = [left, right].into_iter().filter(|&l| l != Line::None).map(|l| band(l, h)).reduce(Span::union).unwrap_or(middle(h));
        let (has_vertical, has_horizontal) = (up != Line::None || down != Line::None, left != Line::None || right != Line::None);
        // The middle stroke of a double line: its gap.
        let (gap_x, gap_y) = (Span::centred(self.light, w), Span::centred(self.light, h));

        if right == Line::Double {
            self.fill(Span { start: vertical.start, end: w }, band(right, h), 255);
        }
        if left == Line::Double {
            self.fill(Span { start: 0, end: vertical.end }, band(left, h), 255);
        }
        if up == Line::Double {
            self.fill(band(up, w), Span { start: 0, end: horizontal.end }, 255);
        }
        if down == Line::Double {
            self.fill(band(down, w), Span { start: horizontal.start, end: h }, 255);
        }
        if right == Line::Double {
            self.clear(Span { start: if has_vertical { gap_x.start } else { w / 2 }, end: w }, gap_y);
        }
        if left == Line::Double {
            self.clear(Span { start: 0, end: if has_vertical { gap_x.end } else { w / 2 } }, gap_y);
        }
        if up == Line::Double {
            self.clear(gap_x, Span { start: 0, end: if has_horizontal { gap_y.end } else { h / 2 } });
        }
        if down == Line::Double {
            self.clear(gap_x, Span { start: if has_horizontal { gap_y.start } else { h / 2 }, end: h });
        }

        // A double line running straight through, met from one side only.
        let through_vertical = up == Line::Double && down == Line::Double;
        let through_horizontal = left == Line::Double && right == Line::Double;
        let single = |line: Line| matches!(line, Line::Light | Line::Heavy);
        if single(right) {
            let start = if through_vertical && left == Line::None { gap_x.end } else { vertical.start };
            self.fill(Span { start, end: w }, band(right, h), 255);
        }
        if single(left) {
            let end = if through_vertical && right == Line::None { gap_x.start } else { vertical.end };
            self.fill(Span { start: 0, end }, band(left, h), 255);
        }
        if single(up) {
            let end = if through_horizontal && down == Line::None { gap_y.start } else { horizontal.end };
            self.fill(band(up, w), Span { start: 0, end }, 255);
        }
        if single(down) {
            let start = if through_horizontal && up == Line::None { gap_y.end } else { horizontal.start };
            self.fill(band(down, w), Span { start, end: h }, 255);
        }
    }

    /// A dashed line, its dashes centred in equal parts of the cell so the
    /// dashes of a row of cells are evenly spaced, and as many again for
    /// each further cell of a run, so a dash keeps its length.
    fn dashes(&mut self, code: u32) {
        // How many dashes, how heavy, and whether they run across.
        let (count, line, across) = match code {
            0x2504 => (3, Line::Light, true),
            0x2505 => (3, Line::Heavy, true),
            0x2506 => (3, Line::Light, false),
            0x2507 => (3, Line::Heavy, false),
            0x2508 => (4, Line::Light, true),
            0x2509 => (4, Line::Heavy, true),
            0x250A => (4, Line::Light, false),
            0x250B => (4, Line::Heavy, false),
            0x254C => (2, Line::Light, true),
            0x254D => (2, Line::Heavy, true),
            0x254E => (2, Line::Light, false),
            _ => (2, Line::Heavy, false),
        };
        let count = if across { count * self.span } else { count };
        let length = if across { self.width } else { self.height };
        let band = Span::centred(self.weight(line), if across { self.height } else { self.width });
        for i in 0..count {
            let (from, to) = (i * length / count, (i + 1) * length / count);
            let gap = ((to - from) / 3).max(1);
            let dash = Span { start: from + gap / 2, end: to - (gap - gap / 2) };
            if across { self.fill(dash, band, 255) } else { self.fill(band, dash, 255) }
        }
    }

    /// A rounded corner: a quarter circle from the middle of one edge to
    /// the middle of the next, run on straight to the edge on whichever
    /// side the cell is longer, so it meets a straight line there.
    fn arc(&mut self, code: u32) {
        // Which edges it joins: right or left, then down or up.
        let (rightward, downward) = match code {
            0x256D => (true, true),
            0x256E => (false, true),
            0x256F => (false, false),
            _ => (true, false),
        };
        let (w, h, light) = (self.width as f32, self.height as f32, self.light as f32);
        // The middles of the strokes it meets, where a straight line has them.
        let x_mid = Span::centred(self.light, self.width).start as f32 + light / 2.0;
        let y_mid = Span::centred(self.light, self.height).start as f32 + light / 2.0;
        let (sx, sy) = (if rightward { 1.0 } else { -1.0 }, if downward { 1.0 } else { -1.0 });
        let reach_x = if rightward { w - x_mid } else { x_mid };
        let reach_y = if downward { h - y_mid } else { y_mid };
        let radius = reach_x.min(reach_y);
        let (cx, cy) = (x_mid + sx * radius, y_mid + sy * radius);
        let half = light / 2.0;
        self.shape(|x, y| {
            let (dx, dy) = (x - cx, y - cy);
            let on_arc = dx * sx <= 0.0 && dy * sy <= 0.0 && ((dx * dx + dy * dy).sqrt() - radius).abs() <= half;
            let across = dx * sx >= 0.0 && (y - y_mid).abs() <= half;
            let along = dy * sy >= 0.0 && (x - x_mid).abs() <= half;
            on_arc || across || along
        });
    }

    /// The slants corner to corner, so a row of them is one unbroken line.
    fn diagonals(&mut self, code: u32) {
        let (w, h, half) = (self.width as f32, self.height as f32, self.light as f32 / 2.0);
        let length = (w * w + h * h).sqrt();
        let rising = code != 0x2572;
        let falling = code != 0x2571;
        self.shape(|x, y| {
            // Distance from the line through two opposite corners.
            let from_rising = (h * x + w * y - w * h).abs() / length;
            let from_falling = (h * x - w * y).abs() / length;
            (rising && from_rising <= half) || (falling && from_falling <= half)
        });
    }

    /// A shade: dots about a light line wide, a whole even number of them
    /// across each cell and down it, so a cell's pattern carries on into
    /// its neighbors' on every side. A quarter of them for `░`, on a grid
    /// two dots apart, every other one for `▒`, and all but that quarter
    /// for `▓`.
    fn shade(&mut self, code: u32) {
        let (cell_width, height) = self.cell;
        let per = |length: i32| 2 * (length / (2 * self.light)).max(1);
        let (across, down) = (per(cell_width) * self.span, per(height));
        for j in 0..down {
            for i in 0..across {
                let on = match code {
                    0x2591 => i % 2 == 0 && j % 2 == 0,
                    0x2592 => (i + j) % 2 == 0,
                    _ => !(i % 2 == 1 && j % 2 == 1),
                };
                if on {
                    let x = Span { start: i * self.width / across, end: (i + 1) * self.width / across };
                    let y = Span { start: j * self.height / down, end: (j + 1) * self.height / down };
                    self.fill(x, y, 255);
                }
            }
        }
    }

    /// U+2580 to U+259F: halves, eighths, quadrants and shades.
    fn block(&mut self, code: u32) {
        let (w, h) = (self.width, self.height);
        // Where a cell splits in half, the same for every character that
        // splits it, so a left half beside a right half is a whole cell.
        let (half_x, half_y) = (w / 2, h - h / 2);
        let eighths_x = |n: i32| (w * n / 8).max(1);
        let eighths_y = |n: i32| (h * n / 8).max(1);
        match code {
            0x2580 => self.fill(self.all_x(), Span { start: 0, end: half_y }, 255),
            0x2581..=0x2588 => {
                let n = (code - 0x2580) as i32;
                self.fill(self.all_x(), Span { start: h - eighths_y(n), end: h }, 255);
            }
            0x2589..=0x258F => {
                let n = (0x2590 - code) as i32;
                self.fill(Span { start: 0, end: eighths_x(n) }, self.all_y(), 255);
            }
            0x2590 => self.fill(Span { start: half_x, end: w }, self.all_y(), 255),
            0x2591..=0x2593 => self.shade(code),
            0x2594 => self.fill(self.all_x(), Span { start: 0, end: eighths_y(1) }, 255),
            0x2595 => self.fill(Span { start: w - eighths_x(1), end: w }, self.all_y(), 255),
            _ => {
                // Upper left, upper right, lower left, lower right.
                const QUADRANTS: [u8; 10] = [0b0100, 0b1000, 0b0001, 0b1101, 0b1001, 0b0111, 0b1011, 0b0010, 0b0110, 0b1110];
                let quadrants = QUADRANTS[(code - 0x2596) as usize];
                let (left, right) = (Span { start: 0, end: half_x }, Span { start: half_x, end: w });
                let (top, bottom) = (Span { start: 0, end: half_y }, Span { start: half_y, end: h });
                for (bit, x, y) in [(0b0001, left, top), (0b0010, right, top), (0b0100, left, bottom), (0b1000, right, bottom)] {
                    if quadrants & bit != 0 {
                        self.fill(x, y, 255);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: UVec2 = UVec2::new(20, 40);

    /// The rows of a drawn cell as text, `#` for a pixel covered more than
    /// half and `.` for one that is not, for an assertion to print.
    fn picture(glyph: char, size: UVec2) -> Vec<String> {
        let alpha = coverage(glyph, size, 1).unwrap();
        alpha.chunks(size.x as usize).map(|row| row.iter().map(|&a| if a > 127 { '#' } else { '.' }).collect()).collect()
    }

    fn picture_across(glyph: char, cell: UVec2, span: u32) -> Vec<String> {
        let alpha = coverage(glyph, cell, span).unwrap();
        alpha.chunks((cell.x * span) as usize).map(|row| row.iter().map(|&a| if a > 127 { '#' } else { '.' }).collect()).collect()
    }

    fn covered(glyph: char, size: UVec2, x: u32, y: u32) -> bool {
        coverage(glyph, size, 1).unwrap()[(y * size.x + x) as usize] > 127
    }

    /// The pixels a line covers along an edge of the cell, as spans.
    fn on_edge(glyph: char, size: UVec2, edge: &str) -> Vec<u32> {
        let along: Vec<(u32, u32)> = match edge {
            "top" => (0..size.x).map(|x| (x, 0)).collect(),
            "bottom" => (0..size.x).map(|x| (x, size.y - 1)).collect(),
            "left" => (0..size.y).map(|y| (0, y)).collect(),
            _ => (0..size.y).map(|y| (size.x - 1, y)).collect(),
        };
        along.into_iter().enumerate().filter(|&(_, (x, y))| covered(glyph, size, x, y)).map(|(i, _)| i as u32).collect()
    }

    #[test]
    fn exactly_the_box_drawing_and_block_elements_blocks_are_drawn_here() {
        assert_eq!(index('\u{2500}'), Some(0));
        assert_eq!(index('\u{259F}'), Some(159));
        for outside in ['\u{24FF}', '\u{25A0}', '#', '@', ' '] {
            assert_eq!(index(outside), None, "{outside:?} is the font's");
            assert_eq!(coverage(outside, CELL, 1), None);
        }
        for code in 0x2500..=0x259F {
            let glyph = char::from_u32(code).unwrap();
            let alpha = coverage(glyph, CELL, 1).unwrap();
            assert_eq!(alpha.len(), (CELL.x * CELL.y) as usize);
            assert!(alpha.iter().any(|&a| a > 0), "{glyph} draws something");
        }
    }

    /// The whole point: a horizontal line touches both side edges and a
    /// vertical one the top and bottom, at every cell size, so a frame is
    /// one line rather than a row of dashes.
    #[test]
    fn a_line_runs_to_the_edges_of_its_cell_at_any_size() {
        for size in [UVec2::new(8, 16), UVec2::new(10, 20), UVec2::new(9, 19), CELL, UVec2::new(40, 80)] {
            for glyph in ['─', '━', '═'] {
                let rows: Vec<usize> = picture(glyph, size).iter().enumerate().filter(|(_, r)| r.chars().all(|c| c == '#')).map(|(i, _)| i).collect();
                assert!(!rows.is_empty(), "{glyph} at {size}: some row is covered from edge to edge");
            }
            for glyph in ['│', '┃', '║'] {
                let columns = (0..size.x).filter(|&x| (0..size.y).all(|y| covered(glyph, size, x, y))).count();
                assert!(columns > 0, "{glyph} at {size}: some column is covered from top to bottom");
            }
        }
    }

    /// A line in one cell lands on the very pixels of the same line in the
    /// next, and a corner's arm on those of the line it meets, so a frame
    /// has no step at a corner.
    #[test]
    fn a_corner_meets_the_lines_beside_it_on_the_same_pixels() {
        for size in [UVec2::new(9, 19), UVec2::new(10, 20), CELL] {
            assert_eq!(on_edge('┌', size, "right"), on_edge('─', size, "left"), "{size}: ┌ to ─");
            assert_eq!(on_edge('┌', size, "bottom"), on_edge('│', size, "top"), "{size}: ┌ over │");
            assert_eq!(on_edge('┘', size, "left"), on_edge('─', size, "right"), "{size}: ─ to ┘");
            assert_eq!(on_edge('┘', size, "top"), on_edge('│', size, "bottom"), "{size}: │ over ┘");
            assert_eq!(on_edge('┬', size, "left"), on_edge('─', size, "right"), "{size}: ─ to ┬");
            assert_eq!(on_edge('╔', size, "right"), on_edge('═', size, "left"), "{size}: ╔ to ═");
            assert_eq!(on_edge('╔', size, "bottom"), on_edge('║', size, "top"), "{size}: ╔ over ║");
            assert_eq!(on_edge('╭', size, "right"), on_edge('─', size, "left"), "{size}: ╭ to ─");
            assert_eq!(on_edge('╭', size, "bottom"), on_edge('│', size, "top"), "{size}: ╭ over │");
        }
        // And a corner closes: the pixel where its two arms meet is covered.
        let picture = picture('┌', CELL);
        let row = picture.iter().position(|r| r.ends_with('#')).unwrap();
        assert!(picture[row].trim_start_matches('.').chars().all(|c| c == '#'), "┌'s arm runs unbroken from its stem: {:?}", picture[row]);
    }

    #[test]
    fn a_double_corner_is_two_nested_corners_and_a_double_tee_keeps_its_outer_line_whole() {
        assert_eq!(on_edge('╔', CELL, "right").len(), 2 * on_edge('─', CELL, "left").len(), "two strokes leave ╔ to the right");
        let left_of = |glyph: char| on_edge(glyph, CELL, "top").first().copied().unwrap();
        // ╠'s left stroke runs top to bottom unbroken.
        let x = left_of('╠');
        assert!((0..CELL.y).all(|y| covered('╠', CELL, x, y)), "the outer stroke of ╠ is whole");
        // ╟ meets its single arm at the near stroke, not across the gap.
        let gap_x = (CELL.x - Canvas::new(CELL, 1).light as u32) / 2;
        let y = CELL.y / 2;
        assert!(!covered('╟', CELL, gap_x, y), "╟'s arm stops at the near stroke");
        assert!(covered('╫', CELL, gap_x, y), "╫'s line crosses the gap");
    }

    #[test]
    fn the_full_block_fills_its_cell_and_the_eighths_and_halves_tile_it() {
        for size in [UVec2::new(9, 19), CELL] {
            assert!(coverage('█', size, 1).unwrap().iter().all(|&a| a == 255), "{size}: █ covers every pixel");
            let total = |glyph| coverage(glyph, size, 1).unwrap().iter().filter(|&&a| a == 255).count();
            let cell = (size.x * size.y) as usize;
            assert_eq!(total('▀') + total('▄'), cell, "{size}: an upper and a lower half are the whole cell");
            assert_eq!(total('▌') + total('▐'), cell, "{size}: so are a left and a right half");
            assert_eq!(total('▘') + total('▝') + total('▖') + total('▗'), cell, "{size}: and the four quadrants");
            let rising: Vec<usize> = ('▁'..='█').map(total).collect();
            assert!(rising.windows(2).all(|w| w[0] < w[1]), "{size}: each lower eighth more than the last: {rising:?}");
        }
    }

    /// A shade is dots, covering about a quarter, a half and three
    /// quarters of its cell, every one fully or not at all, at any size.
    #[test]
    fn a_shade_is_dots_covering_a_quarter_a_half_and_three_quarters() {
        for size in [UVec2::new(9, 19), UVec2::new(10, 20), CELL, UVec2::new(40, 80)] {
            for (glyph, share) in [('░', 0.25), ('▒', 0.5), ('▓', 0.75)] {
                let alpha = coverage(glyph, size, 1).unwrap();
                assert!(alpha.iter().all(|&a| a == 0 || a == 255), "{glyph} at {size}: dots, not a tint");
                let covered = alpha.iter().filter(|&&a| a == 255).count() as f32 / alpha.len() as f32;
                assert!((covered - share).abs() < 0.08, "{glyph} at {size}: {covered} of the cell, not about {share}");
            }
        }
        // About a light line wide: a cell twenty wide has ten dots across.
        let row: String = picture('▒', CELL)[0].clone();
        assert_eq!(row, "##..##..##..##..##..", "a light line's width a dot");
    }

    /// A shade's dots carry on into the next cell on every side: two cells
    /// side by side are the pattern a run two cells wide draws, and a cell
    /// over another alternates across the seam as it does inside.
    #[test]
    fn a_shade_tiles_into_its_neighbors_without_a_seam() {
        for size in [UVec2::new(9, 19), UVec2::new(10, 20), CELL] {
            for glyph in ['░', '▒', '▓'] {
                let one = picture(glyph, size);
                let two = picture_across(glyph, size, 2);
                let beside: Vec<String> = one.iter().map(|row| format!("{row}{row}")).collect();
                assert_eq!(two, beside, "{glyph} at {size}: two cells side by side are one pattern");
            }
            // Down the seam, the bottom dots of one cell and the top dots of
            // the one under it are the two phases of the pattern, as any two
            // rows of dots one above the other are.
            let medium = picture('▒', size);
            let (top, bottom) = (&medium[0], &medium[medium.len() - 1]);
            assert!(top.chars().zip(bottom.chars()).all(|(a, b)| a != b), "▒ at {size}: {top} over {bottom}");
            let light = picture('░', size);
            assert!(light[light.len() - 1].chars().all(|c| c == '.') && light[0].contains('#'), "░ at {size}: a row of dots then a row without");
        }
    }

    /// A glyph drawn across a run of two cells keeps one cell's strokes: a
    /// horizontal line runs the width of the run at the weight of one
    /// cell's, a vertical one stands in its middle no wider than in a cell,
    /// a double one keeps its strokes a light line apart, and a block fills
    /// the run.
    #[test]
    fn a_glyph_across_a_wide_run_keeps_the_strokes_of_one_cell() {
        let widest = |rows: &[String]| rows.iter().map(|r| r.matches('#').count()).max().unwrap_or(0);
        let two = picture_across('│', CELL, 2);
        assert_eq!(widest(&two), widest(&picture('│', CELL)), "a vertical stroke as wide in a run as in a cell");
        let mid = two[0].find('#').unwrap() as u32;
        assert_eq!(mid, CELL.x - 1, "and in the middle of the run");
        let rows_of = |rows: &[String]| rows.iter().filter(|r| r.contains('#')).count();
        assert_eq!(rows_of(&picture_across('─', CELL, 2)), rows_of(&picture('─', CELL)), "a horizontal line as heavy");
        assert!(picture_across('─', CELL, 2).iter().any(|r| r.chars().all(|c| c == '#')), "and edge to edge");
        assert_eq!(picture_across('║', CELL, 2)[0], format!("{}##..##{}", ".".repeat(17), ".".repeat(17)), "double bars a light line apart");
        assert!(coverage('█', CELL, 2).unwrap().iter().all(|&a| a == 255), "a block fills the run");
    }

    #[test]
    fn the_sheet_holds_every_character_at_its_index_in_white() {
        let (image, layout) = sheet(UVec2::new(10, 20), 1);
        assert_eq!((image.width(), image.height()), (160, 200));
        assert_eq!(layout.textures.len(), 160);
        let full = layout.textures[index('█').unwrap()];
        assert_eq!(full.size(), UVec2::new(10, 20));
        let data = image.data.as_ref().unwrap();
        let pixel = |x: u32, y: u32| &data[((y * 160 + x) * 4) as usize..][..4];
        assert_eq!(pixel(full.min.x, full.min.y), &[255, 255, 255, 255], "█ is covered at its corner");
        assert_eq!(pixel(0, 0)[3], 0, "─ is not, at its corner");
    }
}
