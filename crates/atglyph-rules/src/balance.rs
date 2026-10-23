//! Threat scoring and the spawn-band report.
//!
//! A game's monsters, items and spawn tables are its own types; the
//! engine cannot read them. What it can do is score anything that answers
//! [`ThreatSubject`] and roll a spawn table into a [`Report`] of what the
//! player meets at each band, so a content change is checked with a
//! command rather than a playthrough.
//!
//! Threat is effective hit points times damage per turn: how long a
//! thing takes to kill, times how much it hurts while it lives. Armor
//! counts as extra hit points at a fixed rate and speed scales the
//! damage. It is a ranking, not a simulation; two things with the same
//! score are worth the same worry, and that is all it claims.

use std::fmt::Write;

use crate::content::BandedTable;

/// Anything that can be scored.
pub trait ThreatSubject {
    /// Hit points.
    fn hp(&self) -> i32;
    /// Flat damage removed from each hit taken.
    fn armor(&self) -> i32;
    /// Average damage of one attack.
    fn damage_per_hit(&self) -> f32;
    /// Speed as a percentage of normal.
    fn speed_pct(&self) -> u32;
}

/// How many hit points a point of armor is worth.
pub const HP_PER_ARMOR: f32 = 4.0;

/// The threat score of `subject`.
pub fn threat(subject: &impl ThreatSubject) -> f32 {
    let eff_hp = subject.hp().max(0) as f32 + subject.armor().max(0) as f32 * HP_PER_ARMOR;
    let dps = subject.damage_per_hit().max(0.0) * subject.speed_pct() as f32 / 100.0;
    eff_hp * dps
}

/// One band of a report.
#[derive(Debug, Clone, PartialEq)]
pub struct BandRow {
    /// The band.
    pub band: i32,
    /// Weighted mean threat of one pick at this band.
    pub mean_threat: f32,
    /// The heaviest single entry that can appear.
    pub max_threat: f32,
    /// Weighted mean group size.
    pub mean_group: f32,
    /// The names that can appear, heaviest first.
    pub names: Vec<String>,
}

/// A spawn table rolled out band by band.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// One row per band asked for.
    pub rows: Vec<BandRow>,
    /// Bands with nothing to spawn.
    pub gaps: Vec<i32>,
}

impl Report {
    /// Scores `table` over `bands`, with `describe` giving each entry's
    /// name and threat.
    pub fn over<T>(table: &BandedTable<T>, bands: std::ops::RangeInclusive<i32>, describe: impl Fn(&T) -> (String, f32)) -> Self {
        let mut rows = Vec::new();
        let mut gaps = Vec::new();
        for band in bands {
            let entries: Vec<_> = table.at(band).collect();
            if entries.is_empty() {
                gaps.push(band);
                continue;
            }
            let total_weight: f32 = entries.iter().map(|e| e.weight as f32).sum();
            let mut named: Vec<(String, f32, f32, f32)> = entries
                .iter()
                .map(|e| {
                    let (name, t) = describe(&e.item);
                    let group = (e.min_group as f32 + e.max_group as f32) / 2.0;
                    (name, t, e.weight as f32, group)
                })
                .collect();
            named.sort_by(|a, b| b.1.total_cmp(&a.1));
            let mean_threat = named.iter().map(|(_, t, w, _)| t * w).sum::<f32>() / total_weight;
            let mean_group = named.iter().map(|(_, _, w, g)| g * w).sum::<f32>() / total_weight;
            let max_threat = named.first().map(|n| n.1).unwrap_or(0.0);
            rows.push(BandRow { band, mean_threat, max_threat, mean_group, names: named.into_iter().map(|n| n.0).collect() });
        }
        Self { rows, gaps }
    }

    /// The report as a text table.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "{:>5} {:>10} {:>10} {:>6}  who", "band", "mean", "max", "group");
        for r in &self.rows {
            let _ = writeln!(out, "{:>5} {:>10.1} {:>10.1} {:>6.1}  {}", r.band, r.mean_threat, r.max_threat, r.mean_group, r.names.join(", "));
        }
        if !self.gaps.is_empty() {
            let _ = writeln!(out, "gaps: {}", self.gaps.iter().map(|b| b.to_string()).collect::<Vec<_>>().join(", "));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::BandedEntry;

    struct Mob {
        hp: i32,
        armor: i32,
        dmg: f32,
        speed: u32,
    }
    impl ThreatSubject for Mob {
        fn hp(&self) -> i32 {
            self.hp
        }
        fn armor(&self) -> i32 {
            self.armor
        }
        fn damage_per_hit(&self) -> f32 {
            self.dmg
        }
        fn speed_pct(&self) -> u32 {
            self.speed
        }
    }

    #[test]
    fn threat_rewards_armor_and_speed() {
        let crab = Mob { hp: 4, armor: 2, dmg: 2.0, speed: 80 };
        let dog = Mob { hp: 8, armor: 0, dmg: 2.5, speed: 130 };
        assert!((threat(&crab) - 12.0 * 1.6).abs() < 1e-4);
        assert!(threat(&dog) > threat(&crab));
    }

    #[test]
    fn the_report_ranks_bands_and_finds_gaps() {
        let table =
            BandedTable::new(vec![BandedEntry::new("crab").bands(0, 2).weight(3).group(2, 4), BandedEntry::new("jaguar").bands(2, 5).weight(1).group(1, 1)]);
        let score = |name: &&str| (name.to_string(), if *name == "crab" { 10.0 } else { 100.0 });
        let r = Report::over(&table, 0..=7, score);
        assert_eq!(r.gaps, vec![6, 7]);
        assert_eq!(r.rows[0].names, vec!["crab"]);
        assert_eq!(r.rows[2].names, vec!["jaguar", "crab"], "heaviest first");
        assert!((r.rows[2].mean_threat - (10.0 * 3.0 + 100.0) / 4.0).abs() < 1e-4);
        assert!((r.rows[2].mean_group - (3.0 * 3.0 + 1.0) / 4.0).abs() < 1e-4);
        assert_eq!(r.rows[3].max_threat, 100.0);
        assert!(r.render().contains("gaps: 6, 7"));
    }
}
