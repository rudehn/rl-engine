//! Weighted draws banded by a number: depth, distance from town, danger.

use rand::Rng;
use serde::{Deserialize, Serialize};

/// One row of a banded table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BandedEntry<T> {
    /// What can be drawn.
    pub item: T,
    /// Lowest band this row applies to, inclusive.
    #[serde(default = "band_min")]
    pub min_band: i32,
    /// Highest band this row applies to, inclusive.
    #[serde(default = "band_max")]
    pub max_band: i32,
    /// Relative chance among the rows that apply.
    #[serde(default = "one")]
    pub weight: u32,
    /// Smallest group drawn with the item.
    #[serde(default = "one")]
    pub min_group: u32,
    /// Largest group drawn with the item.
    #[serde(default = "one")]
    pub max_group: u32,
}

fn band_min() -> i32 {
    i32::MIN
}
fn band_max() -> i32 {
    i32::MAX
}
fn one() -> u32 {
    1
}

impl<T> BandedEntry<T> {
    /// A row that applies everywhere with weight 1 and a group of one.
    pub fn new(item: T) -> Self {
        Self { item, min_band: i32::MIN, max_band: i32::MAX, weight: 1, min_group: 1, max_group: 1 }
    }

    /// Restricts the row to `min..=max`.
    pub fn bands(mut self, min: i32, max: i32) -> Self {
        self.min_band = min;
        self.max_band = max;
        self
    }

    /// Sets the weight.
    pub fn weight(mut self, weight: u32) -> Self {
        self.weight = weight;
        self
    }

    /// Sets the group size range.
    pub fn group(mut self, min: u32, max: u32) -> Self {
        self.min_group = min;
        self.max_group = max.max(min);
        self
    }

    /// Whether the row applies at `band`.
    pub fn applies(&self, band: i32) -> bool {
        (self.min_band..=self.max_band).contains(&band)
    }
}

/// Rows that apply at a band, drawn by weight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BandedTable<T> {
    entries: Vec<BandedEntry<T>>,
}

impl<T> Default for BandedTable<T> {
    fn default() -> Self {
        Self { entries: Vec::new() }
    }
}

impl<T> BandedTable<T> {
    /// A table from rows.
    pub fn new(entries: Vec<BandedEntry<T>>) -> Self {
        Self { entries }
    }

    /// Adds a row.
    pub fn push(&mut self, entry: BandedEntry<T>) {
        self.entries.push(entry);
    }

    /// Every row.
    pub fn entries(&self) -> &[BandedEntry<T>] {
        &self.entries
    }

    /// The rows that apply at `band`.
    pub fn at(&self, band: i32) -> impl Iterator<Item = &BandedEntry<T>> {
        self.entries.iter().filter(move |e| e.applies(band))
    }

    /// Whether any row applies at `band`.
    pub fn covers(&self, band: i32) -> bool {
        self.at(band).any(|e| e.weight > 0)
    }

    /// Draws one row at `band` by weight among those `keep` accepts, or
    /// `None` if none applies there. No fallback: ask
    /// [`band_where`](Self::band_where) first for the band to draw at.
    pub fn pick_where(&self, band: i32, keep: impl Fn(&T) -> bool, rng: &mut impl Rng) -> Option<&BandedEntry<T>> {
        let kept = || self.at(band).filter(|e| keep(&e.item));
        let total: u64 = kept().map(|e| e.weight as u64).sum();
        if total == 0 {
            return None;
        }
        let mut roll = rng.random_range(0..total);
        for e in kept() {
            if (e.weight as u64) > roll {
                return Some(e);
            }
            roll -= e.weight as u64;
        }
        None
    }

    /// The band a draw among the rows `keep` accepts is made at, the way
    /// `LootTable::band_for` answers for a tag: `band` itself when a kept row
    /// with weight applies there, else the deepest band shallower than it
    /// that one covers, so a request past the table's end gets its deepest,
    /// else the shallowest band deeper. `None` when no kept row has weight.
    pub fn band_where(&self, band: i32, keep: impl Fn(&T) -> bool) -> Option<i32> {
        let kept: Vec<&BandedEntry<T>> = self.entries.iter().filter(|e| e.weight > 0 && keep(&e.item)).collect();
        if kept.is_empty() {
            return None;
        }
        if kept.iter().any(|e| e.applies(band)) {
            return Some(band);
        }
        let shallower = kept.iter().map(|e| e.max_band).filter(|deepest| *deepest < band).max();
        shallower.or_else(|| kept.iter().map(|e| e.min_band).filter(|shallowest| *shallowest > band).min())
    }

    /// Draws one row at `band` by weight, or `None` if nothing applies.
    pub fn pick(&self, band: i32, rng: &mut impl Rng) -> Option<&BandedEntry<T>> {
        self.pick_where(band, |_| true, rng)
    }

    /// Draws one row and a group size within its range.
    pub fn pick_group(&self, band: i32, rng: &mut impl Rng) -> Option<(&T, u32)> {
        let e = self.pick(band, rng)?;
        let n = if e.max_group > e.min_group { rng.random_range(e.min_group..=e.max_group) } else { e.min_group };
        Some((&e.item, n))
    }

    /// The bands in `range` that no row covers, for a guard test.
    pub fn gaps(&self, range: std::ops::RangeInclusive<i32>) -> Vec<i32> {
        range.filter(|b| !self.covers(*b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};

    fn table() -> BandedTable<&'static str> {
        BandedTable::new(vec![
            BandedEntry::new("rat").bands(1, 5).weight(3).group(2, 4),
            BandedEntry::new("wolf").bands(3, 10).weight(1),
            BandedEntry::new("bear").bands(8, 10).weight(2),
        ])
    }

    fn rows() -> BandedTable<&'static str> {
        BandedTable::new(vec![
            BandedEntry::new("rat").bands(1, 10).weight(5),
            BandedEntry::new("crab").bands(2, 7).weight(3),
            BandedEntry::new("heavy").bands(3, 8).weight(2),
            BandedEntry::new("ghost").bands(1, 10).weight(0),
        ])
    }

    #[test]
    fn rows_apply_by_band_and_weights_bias_the_draw() {
        let t = table();
        let mut rng = StdRng::seed_from_u64(1);
        let mut rats = 0;
        for _ in 0..1000 {
            if t.pick(4, &mut rng).unwrap().item == "rat" {
                rats += 1;
            }
        }
        assert!((650..850).contains(&rats), "{rats}");
        assert_eq!(t.pick(1, &mut rng).unwrap().item, "rat");
        assert!(t.pick(11, &mut rng).is_none());
        assert_eq!(t.gaps(0..=12), vec![0, 11, 12]);
        assert!(t.covers(9));
    }

    #[test]
    fn groups_stay_in_range() {
        let t = table();
        let mut rng = StdRng::seed_from_u64(2);
        for _ in 0..200 {
            let (item, n) = t.pick_group(2, &mut rng).unwrap();
            assert_eq!(*item, "rat");
            assert!((2..=4).contains(&n));
        }
    }

    #[test]
    fn loads_from_ron_with_defaults() {
        let t: BandedTable<String> = ron::from_str(r#"[(item: "a"), (item: "b", min_band: 5, weight: 0)]"#).unwrap();
        assert!(t.entries()[0].applies(i32::MAX));
        assert_eq!(t.entries()[1].weight, 0);
        assert!(!t.covers(6) || t.entries()[0].applies(6));
    }

    #[test]
    fn a_restricted_draw_never_returns_a_row_it_was_told_to_leave_out() {
        let table = rows();
        for s in 0..500 {
            let mut rng = StdRng::seed_from_u64(s);
            let e = table.pick_where(5, |n| *n != "rat", &mut rng).unwrap();
            assert_ne!(e.item, "rat", "seed {s}");
            assert_ne!(e.item, "ghost", "seed {s}: a row with no weight is never drawn");
        }
    }

    #[test]
    fn an_unrestricted_draw_is_the_plain_draw_over_a_span_of_seeds() {
        let table = rows();
        for s in 0..500 {
            let (mut a, mut b) = (StdRng::seed_from_u64(s), StdRng::seed_from_u64(s));
            assert_eq!(table.pick(4, &mut a).map(|e| e.item), table.pick_where(4, |_| true, &mut b).map(|e| e.item), "seed {s}");
        }
    }

    #[test]
    fn a_restricted_band_is_exact_where_a_kept_row_applies_and_falls_back_shallower_first() {
        let table = rows();
        let heavy_or_crab = |n: &&str| *n == "heavy" || *n == "crab";
        assert_eq!(table.band_where(5, heavy_or_crab), Some(5), "both apply at five");
        assert_eq!(table.band_where(12, heavy_or_crab), Some(8), "past the deepest, the deepest");
        assert_eq!(table.band_where(1, heavy_or_crab), Some(2), "shallower than all, the shallowest");
        assert_eq!(table.band_where(5, |n| *n == "ghost"), None, "a row with no weight covers nothing");
        assert_eq!(table.band_where(5, |n| *n == "nobody"), None);
    }
}
