//! Landmarks, placed by score.
//!
//! The engine does not know what a town is. It knows how to take a score
//! per cell, jitter it so the top cell does not always win, sort, and keep
//! the best candidates that stay clear of what is already placed. The game
//! supplies the kinds, the scores and the clearances.

use rl_core::{Point, seed};
use serde::{Deserialize, Serialize};

/// A game-defined kind of landmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct SiteKindId(pub u16);

/// A landmark on the world graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Site {
    /// What it is.
    pub kind: SiteKindId,
    /// Which region it occupies.
    pub position: Point,
}

/// A minimum spacing from an already-placed kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clearance {
    /// The kind to keep away from.
    pub from: SiteKindId,
    /// At least this many cells away.
    pub cells: u32,
}

/// How many of a kind to place and how they keep their distance.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementRules {
    /// One site per this many eligible cells.
    pub cells_per_site: u32,
    /// At least this many, if any cell is eligible at all.
    pub min: u32,
    /// At most this many.
    pub max: u32,
    /// How far the per-cell jitter can move a score, as a fraction.
    pub jitter: f32,
    /// Spacing from kinds already placed. A kind not named imposes nothing.
    pub clearances: Vec<Clearance>,
}

impl Default for PlacementRules {
    fn default() -> Self {
        Self { cells_per_site: 400, min: 1, max: 64, jitter: 0.15, clearances: Vec::new() }
    }
}

/// Scores every cell for `kind` and appends as many as the rules allow.
///
/// Candidates are built in row-major order and sorted stably, so equal
/// scores always break the same way. Jitter is derived from position, not
/// drawn from a stream, so visiting order cannot reroll it. Returns how
/// many were placed.
pub fn place_scored(
    sites: &mut Vec<Site>,
    kind: SiteKindId,
    rules: &PlacementRules,
    stream_seed: u64,
    width: i32,
    height: i32,
    score_at: impl Fn(Point) -> f32,
) -> usize {
    let mut eligible = 0u32;
    let mut candidates: Vec<(f32, Point)> = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let p = Point::new(x, y);
            let score = score_at(p);
            if score <= 0.0 {
                continue;
            }
            eligible += 1;
            candidates.push((score * jitter_at(stream_seed, p, rules.jitter), p));
        }
    }
    if eligible == 0 {
        return 0;
    }
    let target = (eligible / rules.cells_per_site.max(1)).clamp(rules.min, rules.max.max(rules.min));
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut placed = 0;
    for (_, p) in candidates {
        if placed >= target {
            break;
        }
        if has_clearance(sites, p, &rules.clearances) {
            sites.push(Site { kind, position: p });
            placed += 1;
        }
    }
    placed as usize
}

/// Whether `at` is far enough from everything already placed.
pub fn has_clearance(placed: &[Site], at: Point, clearances: &[Clearance]) -> bool {
    placed.iter().all(|site| {
        clearances.iter().filter(|c| c.from == site.kind).all(|c| rl_core::geometry::euclidean_sq(site.position, at) as i64 >= (c.cells as i64).pow(2))
    })
}

/// A per-cell multiplier near 1, from position alone.
fn jitter_at(stream_seed: u64, p: Point, jitter: f32) -> f32 {
    let roll = (seed::position_hash(stream_seed, p.x, p.y) >> 40) as f32 / (1u64 << 24) as f32;
    1.0 - jitter + 2.0 * jitter * roll
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOWN: SiteKindId = SiteKindId(1);
    const CAMP: SiteKindId = SiteKindId(2);

    #[test]
    fn placement_respects_target_and_clearance() {
        let rules = PlacementRules { cells_per_site: 10, min: 1, max: 100, jitter: 0.1, clearances: vec![Clearance { from: TOWN, cells: 2 }] };
        let mut sites = Vec::new();
        let n = place_scored(&mut sites, TOWN, &rules, 7, 20, 20, |_| 1.0);
        assert_eq!(n, 40, "400 eligible cells at one per ten");
        for (i, a) in sites.iter().enumerate() {
            for b in &sites[i + 1..] {
                assert!(rl_core::geometry::euclidean_sq(a.position, b.position) >= 4);
            }
        }
        let tight = PlacementRules { clearances: vec![Clearance { from: TOWN, cells: 8 }], ..rules };
        let mut few = Vec::new();
        let n = place_scored(&mut few, TOWN, &tight, 7, 20, 20, |_| 1.0);
        assert!((4..40).contains(&n), "clearance caps the count: {n}");
    }

    #[test]
    fn clearances_only_apply_to_named_kinds() {
        let mut sites = vec![Site { kind: TOWN, position: Point::new(5, 5) }];
        let near_town = PlacementRules { cells_per_site: 1, min: 1, max: 1, jitter: 0.0, clearances: vec![Clearance { from: CAMP, cells: 50 }] };
        let n = place_scored(&mut sites, CAMP, &near_town, 1, 10, 10, |p| if p == Point::new(5, 6) { 1.0 } else { 0.0 });
        assert_eq!(n, 1, "a clearance from camps does not keep a camp from a town");
        assert_eq!(place_scored(&mut sites, CAMP, &near_town, 1, 10, 10, |p| if p == Point::new(5, 7) { 1.0 } else { 0.0 }), 0);
    }

    #[test]
    fn the_best_cell_wins_without_jitter_and_nothing_is_placed_with_no_score() {
        let mut sites = Vec::new();
        let rules = PlacementRules { jitter: 0.0, max: 1, ..Default::default() };
        place_scored(&mut sites, TOWN, &rules, 1, 8, 8, |p| (p.x + p.y) as f32);
        assert_eq!(sites[0].position, Point::new(7, 7));
        assert_eq!(place_scored(&mut sites, CAMP, &rules, 1, 8, 8, |_| 0.0), 0);
    }

    #[test]
    fn placement_is_deterministic_and_seed_sensitive() {
        let run = |s: u64| {
            let mut sites = Vec::new();
            place_scored(&mut sites, TOWN, &PlacementRules { cells_per_site: 30, ..Default::default() }, s, 30, 30, |_| 1.0);
            sites
        };
        assert_eq!(run(3), run(3));
        assert_ne!(run(3), run(4));
    }
}
