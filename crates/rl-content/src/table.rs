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

    /// Draws one row at `band` by weight, or `None` if nothing applies.
    pub fn pick(&self, band: i32, rng: &mut impl Rng) -> Option<&BandedEntry<T>> {
        let total: u64 = self.at(band).map(|e| e.weight as u64).sum();
        if total == 0 {
            return None;
        }
        let mut roll = rng.random_range(0..total);
        for e in self.at(band) {
            if (e.weight as u64) > roll {
                return Some(e);
            }
            roll -= e.weight as u64;
        }
        None
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
}
