//! A value per tile, stepped a whole turn at a time: fire, gas, and whatever
//! else spreads or fades over a grid.
//!
//! A rule reads the field as it stood and writes every cell's next value into
//! a second buffer, and the two swap. Nothing a step changes can move again
//! within the same step, so a fire cannot run across a map in one turn, and a
//! cloud spreads the same way whichever order its cells are visited in. The
//! second buffer is kept between steps, so a step allocates nothing.
//!
//! One type for every such thing, with the rule passed in: fire and gas are
//! two rules over it in `rl-rules`, and sound or scent would be a third.
//!
//! ```
//! use rl_core::Point;
//! use rl_grid::TileField;
//!
//! // Every cell takes the most of itself and its neighbours: a wave.
//! let mut field: TileField<u8> = TileField::new(5, 1);
//! field.set(Point::new(0, 0), 9);
//! field.step(|_, around| around.neighbours().map(|(_, v)| v).fold(around.here(), u8::max));
//! assert_eq!(field.cells(), &[9, 9, 0, 0, 0], "one step, one cell further, however it was visited");
//! ```

use rl_core::{Direction, Grid2D, Point};

/// A value per tile of a `width` by `height` grid, stepped by a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileField<T> {
    width: i32,
    height: i32,
    now: Vec<T>,
    next: Vec<T>,
}

impl<T: Copy + Default + PartialEq> TileField<T> {
    /// A field of `width` by `height` holding nothing.
    pub fn new(width: i32, height: i32) -> Self {
        let cells = (width.max(0) * height.max(0)) as usize;
        Self { width: width.max(0), height: height.max(0), now: vec![T::default(); cells], next: vec![T::default(); cells] }
    }

    /// The value at `p`, and nothing outside the field.
    pub fn get(&self, p: Point) -> T {
        self.checked_idx(p).map_or(T::default(), |i| self.now[i])
    }

    /// Sets the value at `p`. Returns whether `p` was inside.
    pub fn set(&mut self, p: Point, value: T) -> bool {
        self.update(p, |_| value)
    }

    /// Replaces the value at `p` with `f` of it. Returns whether `p` was inside.
    pub fn update(&mut self, p: Point, f: impl FnOnce(T) -> T) -> bool {
        let Some(i) = self.checked_idx(p) else { return false };
        self.now[i] = f(self.now[i]);
        true
    }

    /// Every value, row-major.
    pub fn cells(&self) -> &[T] {
        &self.now
    }

    /// Holds nothing anywhere.
    pub fn clear(&mut self) {
        self.now.fill(T::default());
    }

    /// Whether it holds nothing anywhere, so a step would change nothing a
    /// rule that leaves empty cells empty could produce.
    pub fn is_clear(&self) -> bool {
        self.now.iter().all(|v| *v == T::default())
    }

    /// Every cell holding something, with what, row-major.
    pub fn set_cells(&self) -> impl Iterator<Item = (Point, T)> + '_ {
        self.now.iter().enumerate().filter(|(_, v)| **v != T::default()).map(|(i, v)| (self.idx_point(i), *v))
    }

    /// One step: `rule` is asked every cell's next value, reading the field
    /// as it stood before the step through [`Around`].
    pub fn step(&mut self, mut rule: impl FnMut(Point, Around<'_, T>) -> T) {
        let (width, height) = (self.width, self.height);
        for y in 0..height {
            for x in 0..width {
                let at = Point::new(x, y);
                self.next[(y * width + x) as usize] = rule(at, Around { cells: &self.now, width, height, at });
            }
        }
        std::mem::swap(&mut self.now, &mut self.next);
    }

    /// The field under a window that moved by `shift` and is now `width` by
    /// `height`: every value keeps the cell it was on, and what falls outside
    /// the new bounds is dropped.
    pub fn reframe(&mut self, shift: Point, width: i32, height: i32) {
        let mut moved = TileField::new(width, height);
        for (p, v) in self.set_cells() {
            moved.set(p - shift, v);
        }
        *self = moved;
    }
}

impl<T> Grid2D for TileField<T> {
    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }
}

/// One cell and its neighbours, as they stood before a step.
#[derive(Debug, Clone, Copy)]
pub struct Around<'a, T> {
    cells: &'a [T],
    width: i32,
    height: i32,
    at: Point,
}

impl<'a, T: Copy + Default> Around<'a, T> {
    /// Where this cell is.
    pub fn point(&self) -> Point {
        self.at
    }

    /// This cell's value.
    pub fn here(&self) -> T {
        self.value(self.at)
    }

    /// The value one step in `direction`, and nothing past the edge.
    pub fn toward(&self, direction: Direction) -> T {
        self.value(self.at + direction.offset())
    }

    /// The eight neighbours inside the field, in [`Direction::ALL`] order.
    pub fn neighbours(&self) -> impl Iterator<Item = (Point, T)> + 'a {
        let Around { cells, width, height, at } = *self;
        Direction::ALL
            .into_iter()
            .map(move |d| at + d.offset())
            .filter(move |p| p.x >= 0 && p.y >= 0 && p.x < width && p.y < height)
            .map(move |p| (p, cells[(p.y * width + p.x) as usize]))
    }

    fn value(&self, p: Point) -> T {
        let inside = p.x >= 0 && p.y >= 0 && p.x < self.width && p.y < self.height;
        if inside { self.cells[(p.y * self.width + p.x) as usize] } else { T::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_reads_only_the_field_as_it_stood() {
        // A rule that copies the west neighbour: visited left to right, a
        // rule reading its own writes would carry the value to the far edge.
        let mut field: TileField<u8> = TileField::new(6, 1);
        field.set(Point::new(0, 0), 5);
        field.step(|_, around| around.toward(Direction::West));
        assert_eq!(field.cells(), &[0, 5, 0, 0, 0, 0]);
        field.step(|_, around| around.toward(Direction::West));
        assert_eq!(field.cells(), &[0, 0, 5, 0, 0, 0], "one cell a step, never more");
    }

    #[test]
    fn outside_the_field_there_is_nothing_and_writes_are_refused() {
        let mut field: TileField<u8> = TileField::new(3, 3);
        assert!(!field.set(Point::new(3, 0), 1));
        assert!(!field.update(Point::new(-1, 1), |v| v + 1));
        assert_eq!(field.get(Point::new(-1, -1)), 0);
        field.set(Point::new(0, 0), 4);
        let mut seen = Vec::new();
        field.step(|p, around| {
            if p == Point::new(0, 0) {
                seen = around.neighbours().collect();
            }
            around.here()
        });
        assert_eq!(seen.len(), 3, "a corner has three neighbours: {seen:?}");
        assert!(!field.is_clear());
        field.clear();
        assert!(field.is_clear());
    }

    #[test]
    fn reframing_keeps_each_value_on_its_cell_and_drops_what_leaves() {
        let mut field: TileField<u8> = TileField::new(4, 4);
        field.set(Point::new(1, 1), 7);
        field.set(Point::new(3, 3), 9);
        // The window moved two east and one south: what was at (3, 3) is now
        // at (1, 2), and what was at (1, 1) fell off the west edge.
        field.reframe(Point::new(2, 1), 4, 4);
        assert_eq!(field.set_cells().collect::<Vec<_>>(), vec![(Point::new(1, 2), 9)]);
    }
}
