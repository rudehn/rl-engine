//! One bit per cell.
//!
//! Viewsheds, visited sets and explored maps want membership tests in a
//! tight loop and deterministic iteration. A hash set gives neither.

use rl_core::{Grid2D, Point};
use serde::{Deserialize, Serialize};

/// A dense grid of bits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitGrid {
    width: i32,
    height: i32,
    words: Vec<u64>,
}

impl BitGrid {
    /// An all-clear grid.
    pub fn new(width: i32, height: i32) -> Self {
        let cells = (width.max(0) as usize) * (height.max(0) as usize);
        Self { width, height, words: vec![0; cells.div_ceil(64)] }
    }

    /// Clears every bit.
    pub fn clear(&mut self) {
        self.words.iter_mut().for_each(|w| *w = 0);
    }

    /// Whether the bit at a flat index is set.
    pub fn get_idx(&self, idx: usize) -> bool {
        self.words[idx / 64] & (1u64 << (idx % 64)) != 0
    }

    /// Sets the bit at a flat index.
    pub fn set_idx(&mut self, idx: usize, on: bool) {
        let mask = 1u64 << (idx % 64);
        if on {
            self.words[idx / 64] |= mask;
        } else {
            self.words[idx / 64] &= !mask;
        }
    }

    /// Sets the bit at a flat index on.
    pub fn insert_idx(&mut self, idx: usize) {
        self.set_idx(idx, true);
    }

    /// Whether the bit at `p` is set; out of bounds is clear.
    pub fn contains(&self, p: Point) -> bool {
        self.checked_idx(p).is_some_and(|i| self.get_idx(i))
    }

    /// Sets the bit at `p`. Returns whether `p` was in bounds.
    pub fn set(&mut self, p: Point, on: bool) -> bool {
        match self.checked_idx(p) {
            Some(i) => {
                self.set_idx(i, on);
                true
            }
            None => false,
        }
    }

    /// Sets the bit at `p` on. Returns whether `p` was in bounds.
    pub fn insert(&mut self, p: Point) -> bool {
        self.set(p, true)
    }

    /// Number of set bits.
    pub fn count(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Whether no bit is set.
    pub fn is_clear(&self) -> bool {
        self.words.iter().all(|w| *w == 0)
    }

    /// Every set index, ascending.
    pub fn iter_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.words.iter().enumerate().flat_map(|(wi, word)| {
            let mut bits = *word;
            std::iter::from_fn(move || {
                if bits == 0 {
                    return None;
                }
                let bit = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                Some(wi * 64 + bit)
            })
        })
    }

    /// Every set cell, row-major.
    pub fn iter(&self) -> impl Iterator<Item = Point> + '_ {
        self.iter_indices().map(|i| self.idx_point(i))
    }

    /// Sets every bit that is set in `other`. Shapes must match.
    pub fn union_with(&mut self, other: &BitGrid) {
        debug_assert_eq!((self.width, self.height), (other.width, other.height));
        for (a, b) in self.words.iter_mut().zip(&other.words) {
            *a |= *b;
        }
    }
}

impl Grid2D for BitGrid {
    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_set_clear_and_count() {
        let mut g = BitGrid::new(10, 7);
        assert!(g.is_clear());
        assert!(g.insert(Point::new(9, 6)));
        assert!(g.insert(Point::new(0, 0)));
        assert!(!g.insert(Point::new(10, 0)));
        assert!(g.contains(Point::new(9, 6)));
        assert!(!g.contains(Point::new(1, 0)));
        assert!(!g.contains(Point::new(-1, 0)));
        assert_eq!(g.count(), 2);
        g.set(Point::new(0, 0), false);
        assert_eq!(g.count(), 1);
        g.clear();
        assert!(g.is_clear());
    }

    #[test]
    fn iteration_is_row_major_and_crosses_word_boundaries() {
        let mut g = BitGrid::new(100, 3);
        for p in [Point::new(70, 2), Point::new(3, 0), Point::new(99, 0), Point::new(0, 1)] {
            g.insert(p);
        }
        assert_eq!(g.iter().collect::<Vec<_>>(), vec![Point::new(3, 0), Point::new(99, 0), Point::new(0, 1), Point::new(70, 2)]);
        assert_eq!(g.iter_indices().count(), 4);
    }

    #[test]
    fn union_merges() {
        let mut a = BitGrid::new(8, 8);
        let mut b = BitGrid::new(8, 8);
        a.insert(Point::new(1, 1));
        b.insert(Point::new(2, 2));
        a.union_with(&b);
        assert_eq!(a.count(), 2);
    }
}
