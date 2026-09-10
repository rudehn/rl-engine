//! Distribution helpers that keep worlds comparable across seeds.
//!
//! Noise amplitude varies from seed to seed, so fixed thresholds give one
//! world a drowned archipelago and the next a single supercontinent.
//! Cutting on quantiles fixes the proportion of each band and lets the
//! noise decide only where it goes.

/// The value at quantile `q` in `[0, 1]` of `values`. `0.0` for an empty slice.
pub fn quantile(values: &[f32], q: f32) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    quantile_of_sorted(&sorted, q)
}

/// The value at quantile `q` of an ascending slice, linearly interpolated.
pub fn quantile_of_sorted(sorted: &[f32], q: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let position = q.clamp(0.0, 1.0) * (sorted.len() - 1) as f32;
    let low = position.floor() as usize;
    let high = (low + 1).min(sorted.len() - 1);
    let t = position - low as f32;
    sorted[low] + (sorted[high] - sorted[low]) * t
}

/// Rescales `values` so the smallest becomes 0 and the largest 1.
/// A constant slice is left untouched rather than dividing by zero.
pub fn normalize_in_place(values: &mut [f32]) {
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &v in values.iter() {
        min = min.min(v);
        max = max.max(v);
    }
    let span = max - min;
    if span <= f32::EPSILON {
        return;
    }
    for v in values.iter_mut() {
        *v = (*v - min) / span;
    }
}

/// Replaces each selected value with its rank in `[0, 1]`, leaving the rest.
///
/// Gives a layer a flat distribution over a subset, so the same share of
/// every world's land is desert and the same share is rainforest.
pub fn rank_normalize_where(values: &mut [f32], selected: impl Fn(usize) -> bool) {
    let mut order: Vec<usize> = (0..values.len()).filter(|&i| selected(i)).collect();
    if order.len() < 2 {
        return;
    }
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let last = (order.len() - 1) as f32;
    for (rank, &index) in order.iter().enumerate() {
        values[index] = rank as f32 / last;
    }
}

/// Hermite interpolation between `edge0` and `edge1`, clamped to `[0, 1]`.
pub fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    if (edge1 - edge0).abs() < f64::EPSILON {
        return if x < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantile_picks_the_expected_cut() {
        let values = [0.0, 1.0, 2.0, 3.0, 4.0];
        assert_eq!(quantile(&values, 0.0), 0.0);
        assert_eq!(quantile(&values, 0.5), 2.0);
        assert_eq!(quantile(&values, 1.0), 4.0);
        assert_eq!(quantile(&[], 0.5), 0.0);
    }

    #[test]
    fn quantile_splits_a_slice_by_proportion() {
        let values: Vec<f32> = (0..1000).map(|i| i as f32).collect();
        let cut = quantile(&values, 0.7);
        let above = values.iter().filter(|&&v| v > cut).count();
        assert!((above as f32 / 1000.0 - 0.3).abs() < 0.01);
    }

    #[test]
    fn normalize_maps_onto_the_unit_interval_and_leaves_constants() {
        let mut values = [3.0, 5.0, 7.0];
        normalize_in_place(&mut values);
        assert_eq!(values, [0.0, 0.5, 1.0]);
        let mut flat = [2.0, 2.0];
        normalize_in_place(&mut flat);
        assert_eq!(flat, [2.0, 2.0]);
    }

    #[test]
    fn rank_normalize_only_touches_selected_entries() {
        let mut values = [9.0, 0.5, 8.0, 0.1];
        rank_normalize_where(&mut values, |i| i % 2 == 0);
        assert_eq!(values, [1.0, 0.5, 0.0, 0.1]);
    }

    #[test]
    fn smoothstep_is_clamped_and_monotonic() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 2.0), 1.0);
        assert!(smoothstep(0.0, 1.0, 0.25) < smoothstep(0.0, 1.0, 0.75));
    }
}
