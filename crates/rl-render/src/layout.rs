//! Where the cells go in a window of any size.
//!
//! The grid was once stretched to fit by the camera, which stretches a
//! picture drawn for another size: glyphs rasterized at fourteen pixels
//! and shown at twenty have stems of uneven width. So the grid is laid out
//! for the window it is in instead, and the glyphs are drawn at the size
//! they are shown.
//!
//! Pure arithmetic, with no `App`, so every property of it is a test over
//! a range of windows rather than something to look at.

use bevy::math::{UVec2, Vec2};

/// How a grid sits in a window, in physical pixels.
///
/// Physical, not logical, because the point is that every cell edge lies
/// on the display's own pixel grid: a cell of a fractional width leaves a
/// seam between two backgrounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fit {
    /// One cell's width and height.
    pub cell: UVec2,
    /// The blank border to the left of and above the grid.
    pub margin: UVec2,
}

/// What a float's rounding may cost before a floor, in pixels. A window of
/// exactly the native size must give exactly the declared cell, and
/// `19.999998` floors to nineteen. Far smaller than one pixel spread over
/// any grid a terminal has, so it never makes the grid overflow.
const ROUNDING: f32 = 1e-3;

/// The largest whole-pixel cells that fit `cols` by `rows` of them in
/// `window`, keeping `base_cell`'s shape as nearly as whole pixels allow,
/// and the margin that centres them.
///
/// Width and height are floored separately, so a cell's shape drifts from
/// the declared one by at most a pixel. A cell is never under one pixel,
/// which is what a window of no size at all, a minimized one, gets.
pub fn fit(cols: i32, rows: i32, base_cell: Vec2, window: UVec2, scale_factor: f32) -> Fit {
    let (cols, rows) = (cols.max(1) as f32, rows.max(1) as f32);
    let native = Vec2::new(cols * base_cell.x, rows * base_cell.y) * scale_factor;
    let zoom = (window.x as f32 / native.x).min(window.y as f32 / native.y);
    let cell = (base_cell * scale_factor * zoom + ROUNDING).floor().max(Vec2::ONE).as_uvec2();
    let used = UVec2::new(cell.x * cols as u32, cell.y * rows as u32);
    Fit { cell, margin: window.saturating_sub(used) / 2 }
}

impl Fit {
    /// A cell's size in logical pixels, the unit sprites are sized in.
    pub fn cell_logical(&self, scale_factor: f32) -> Vec2 {
        self.cell.as_vec2() / scale_factor
    }

    /// The centre of cell `(x, y)` in world space: logical pixels, the
    /// window's centre at the origin, row 0 at the top.
    pub fn center(&self, x: i32, y: i32, window: UVec2, scale_factor: f32) -> Vec2 {
        let half = window.as_vec2() * 0.5;
        let cell = self.cell.as_vec2();
        let px = Vec2::new(-half.x + self.margin.x as f32 + (x as f32 + 0.5) * cell.x, half.y - self.margin.y as f32 - (y as f32 + 0.5) * cell.y);
        px / scale_factor
    }

    /// The cell a point of the window falls in, of a grid `cols` by `rows`:
    /// [`center`](Self::center) run backwards. `at` is in physical pixels
    /// from the window's top-left, the way a cursor is reported. `None` in
    /// the margin round the grid and anywhere outside the window.
    pub fn cell_at(&self, at: Vec2, cols: i32, rows: i32) -> Option<(i32, i32)> {
        let inside = at - self.margin.as_vec2();
        if inside.x < 0.0 || inside.y < 0.0 {
            return None;
        }
        let cell = (inside / self.cell.as_vec2()).floor();
        let (x, y) = (cell.x as i32, cell.y as i32);
        (x < cols && y < rows).then_some((x, y))
    }

    /// The font size for these cells, in logical pixels: the declared one
    /// scaled as the cell's height was.
    pub fn font(&self, base_font: f32, base_cell: Vec2, scale_factor: f32) -> f32 {
        base_font * self.cell.y as f32 / (base_cell.y * scale_factor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: Vec2 = Vec2::new(10.0, 16.0);

    /// Window sizes from far smaller than the grid to far larger, at the
    /// scale factors displays come in.
    fn cases() -> impl Iterator<Item = (i32, i32, UVec2, f32)> {
        let grids = [(80, 40), (138, 55), (40, 12)];
        let factors = [1.0_f32, 1.25, 1.5, 2.0, 3.0];
        grids
            .into_iter()
            .flat_map(move |(cols, rows)| factors.into_iter().flat_map(move |sf| (0..60).map(move |i| (cols, rows, UVec2::new(97 + i * 61, 53 + i * 37), sf))))
    }

    #[test]
    fn the_grid_always_fits_inside_the_window_once_it_can_hold_a_pixel_per_cell() {
        for (cols, rows, window, sf) in cases() {
            let f = fit(cols, rows, BASE, window, sf);
            if window.x >= cols as u32 && window.y >= rows as u32 {
                assert!(f.margin.x + f.cell.x * cols as u32 <= window.x, "{cols}x{rows} {window} {sf}: {f:?}");
                assert!(f.margin.y + f.cell.y * rows as u32 <= window.y, "{cols}x{rows} {window} {sf}: {f:?}");
            }
        }
    }

    #[test]
    fn the_grid_is_centred_to_within_one_pixel() {
        for (cols, rows, window, sf) in cases() {
            let f = fit(cols, rows, BASE, window, sf);
            let (used_x, used_y) = (f.cell.x * cols as u32, f.cell.y * rows as u32);
            if used_x <= window.x && used_y <= window.y {
                assert!((window.x - used_x) - 2 * f.margin.x <= 1, "{window} {sf}: {f:?}");
                assert!((window.y - used_y) - 2 * f.margin.y <= 1, "{window} {sf}: {f:?}");
            }
        }
    }

    /// Every pixel of every cell reads back as that cell, at every window
    /// and scale: its four corners and its centre. And the pixel just
    /// outside the grid on each side reads as no cell.
    #[test]
    fn a_point_reads_back_as_the_cell_it_was_laid_out_in() {
        for (cols, rows, window, sf) in cases() {
            let f = fit(cols, rows, BASE, window, sf);
            if f.cell.x * cols as u32 > window.x || f.cell.y * rows as u32 > window.y {
                continue;
            }
            for (x, y) in [(0, 0), (cols - 1, 0), (0, rows - 1), (cols - 1, rows - 1), (cols / 2, rows / 3)] {
                let left = (f.margin.x + f.cell.x * x as u32) as f32;
                let top = (f.margin.y + f.cell.y * y as u32) as f32;
                let (right, bottom) = (left + f.cell.x as f32 - 0.5, top + f.cell.y as f32 - 0.5);
                for at in [
                    Vec2::new(left, top),
                    Vec2::new(right, top),
                    Vec2::new(left, bottom),
                    Vec2::new(right, bottom),
                    Vec2::new((left + right) / 2.0, (top + bottom) / 2.0),
                ] {
                    assert_eq!(f.cell_at(at, cols, rows), Some((x, y)), "{cols}x{rows} {window} {sf}: {at} of {f:?}");
                }
            }
            let (past_x, past_y) = ((f.margin.x + f.cell.x * cols as u32) as f32, (f.margin.y + f.cell.y * rows as u32) as f32);
            let inside = Vec2::new(f.margin.x as f32, f.margin.y as f32);
            assert_eq!(f.cell_at(Vec2::new(past_x, inside.y), cols, rows), None, "past the right edge: {f:?}");
            assert_eq!(f.cell_at(Vec2::new(inside.x, past_y), cols, rows), None, "past the bottom edge: {f:?}");
            assert_eq!(f.cell_at(Vec2::new(inside.x - 0.5, inside.y), cols, rows), None, "in the left margin: {f:?}");
            assert_eq!(f.cell_at(Vec2::new(inside.x, inside.y - 0.5), cols, rows), None, "in the top margin: {f:?}");
        }
    }

    #[test]
    fn a_cell_is_never_less_than_one_pixel_even_in_a_window_of_none() {
        let f = fit(80, 40, BASE, UVec2::ZERO, 2.0);
        assert_eq!(f.cell, UVec2::ONE);
        assert_eq!(f.margin, UVec2::ZERO);
    }

    #[test]
    fn the_native_window_reproduces_the_declared_cell_with_no_margin() {
        // The factors at which a cell of ten by sixteen is a whole number of
        // pixels; at 1.25 the display itself cannot show the declared cell.
        for sf in [1.0_f32, 1.5, 2.0, 3.0] {
            for (cols, rows) in [(80, 40), (138, 55), (100, 40)] {
                let window = UVec2::new((cols as f32 * BASE.x * sf) as u32, (rows as f32 * BASE.y * sf) as u32);
                let f = fit(cols, rows, BASE, window, sf);
                assert_eq!(f.cell, UVec2::new((BASE.x * sf) as u32, (BASE.y * sf) as u32), "{cols}x{rows} at {sf}");
                assert_eq!(f.margin, UVec2::ZERO, "{cols}x{rows} at {sf}");
                assert_eq!(f.font(14.0, BASE, sf), 14.0);
                assert_eq!(f.cell_logical(sf), BASE);
            }
        }
    }

    #[test]
    fn a_larger_window_never_gives_a_smaller_cell() {
        for sf in [1.0_f32, 2.0] {
            let mut last = UVec2::ZERO;
            for i in 0..200 {
                let f = fit(80, 40, BASE, UVec2::new(200 + i * 16, 160 + i * 13), sf);
                assert!(f.cell.x >= last.x && f.cell.y >= last.y, "step {i} at {sf}");
                last = f.cell;
            }
        }
    }

    #[test]
    fn the_first_cell_is_centred_where_it_always_was_at_the_native_size() {
        // Eight by four cells of ten by twenty: the same numbers
        // `terminal::tests` pins for `cell_center`.
        let (base, window) = (Vec2::new(10.0, 20.0), UVec2::new(80, 80));
        let f = fit(8, 4, base, window, 1.0);
        assert_eq!(f.center(0, 0, window, 1.0), Vec2::new(-35.0, 30.0));
        assert_eq!(f.center(7, 3, window, 1.0), Vec2::new(35.0, -30.0));
    }
}
