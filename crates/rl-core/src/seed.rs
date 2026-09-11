//! Seeds and the streams derived from them.
//!
//! One [`RunSeed`] is rolled per run, saved, and shown to the player. Every
//! random stream in the run derives from it through a [`SeedDomain`] and an
//! index, so the map, the monsters and the loot of one region are
//! independent of each other and all reshuffle together on a new run.
//! Nothing in the engine seeds itself from a bare constant.
//!
//! The generator behind a stream is [`rand`]'s `StdRng`. It is the same
//! algorithm on every platform, which `SmallRng` is not: a world seed must
//! produce the same terrain in a browser as on the desktop. Determinism is an
//! invariant within one build of a game, for replays and tests. It is not
//! promised across engine versions.
//!
//! Where iteration order could leak into a draw, use [`position_hash`] or
//! [`pair_hash`] instead of a stream: they depend only on their inputs, so
//! visiting cells in a different order cannot reroll them.

use rand::SeedableRng;
use rand::rngs::StdRng;

/// SplitMix64's finaliser. Avalanches every input bit across the output so
/// seeds that differ by one produce unrelated streams. `const` so domain
/// salts fold at compile time.
pub const fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a over a byte string, `const` so a domain can be a constant.
const fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        i += 1;
    }
    hash
}

/// The root seed for one run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RunSeed(pub u64);

impl RunSeed {
    /// The seed for one domain at one index (a region, a floor, a pass).
    ///
    /// The result depends on all three inputs, so a new run reshuffles every
    /// index of every domain at once, and two domains never share a stream.
    pub const fn derive(self, domain: SeedDomain, index: u64) -> u64 {
        mix64(mix64(self.0 ^ domain.salt()).wrapping_add(index))
    }

    /// The seed for a run-scoped domain with no index.
    pub const fn stream(self, domain: SeedDomain) -> u64 {
        self.derive(domain, 0)
    }

    /// A generator for one domain at one index.
    pub fn rng(self, domain: SeedDomain, index: u64) -> StdRng {
        StdRng::seed_from_u64(self.derive(domain, index))
    }

    /// A run seed from a raw entropy reading, avalanched so neighbouring
    /// readings give unrelated runs.
    pub const fn from_entropy(entropy: u64) -> Self {
        RunSeed(mix64(entropy))
    }

    /// A fresh run seed from the wall clock and a per-process counter.
    ///
    /// Either source alone has a degenerate case: a coarse or frozen clock
    /// repeats between two quick restarts, and a counter alone repeats across
    /// processes. Together they do not. Not available on wasm, which has no
    /// `SystemTime`; the Bevy layer reads the browser clock and calls
    /// [`RunSeed::from_entropy`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn fresh() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static RUNS_THIS_PROCESS: AtomicU64 = AtomicU64::new(0);
        let nth = RUNS_THIS_PROCESS.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0);
        RunSeed::from_entropy(nanos ^ mix64(nth))
    }
}

/// Which subsystem a derived seed belongs to.
///
/// Open, not an enum: a game declares its own domains as constants
/// (`const WEATHER: SeedDomain = SeedDomain::new(b"weather")`) without
/// editing the engine. The name is what keys the stream, so renaming a
/// domain rerolls it and reordering declarations does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeedDomain(u64);

impl SeedDomain {
    /// A domain keyed by a name.
    pub const fn new(name: &[u8]) -> Self {
        // Force the salt odd so no two domains can differ only in their
        // low bit and so a zero salt is impossible.
        SeedDomain(fnv1a(name) | 1)
    }

    /// The salt mixed into every seed derived under this domain.
    pub const fn salt(self) -> u64 {
        self.0
    }
}

/// A value in `[0, 2^64)` that depends only on `seed` and a position.
///
/// For draws that must not depend on visiting order: jittering a score per
/// cell, picking a tile variant, deciding which of several equally good
/// cells wins.
pub const fn position_hash(seed: u64, x: i32, y: i32) -> u64 {
    mix64(seed ^ mix64((x as u32 as u64) << 32 | (y as u32 as u64)))
}

/// A value that depends only on `seed` and an unordered pair of positions.
///
/// The seam primitive: two neighbouring chunks generated independently can
/// both compute this for their shared edge and agree, without negotiating,
/// where a road or river crosses it.
pub const fn pair_hash(seed: u64, a: (i32, i32), b: (i32, i32)) -> u64 {
    // Canonical order so (a, b) and (b, a) hash alike.
    let a_key = (a.1 as i64) << 32 | (a.0 as u32 as i64);
    let b_key = (b.1 as i64) << 32 | (b.0 as u32 as i64);
    let (first, second) = if a_key <= b_key { (a, b) } else { (b, a) };
    mix64(position_hash(seed, first.0, first.1) ^ mix64(position_hash(seed, second.0, second.1)))
}

/// `hash` reduced to `[0, bound)` without modulo bias worth caring about at
/// the bounds worldgen uses.
pub const fn hash_below(hash: u64, bound: u32) -> u32 {
    (((hash >> 32) * bound as u64) >> 32) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    const DOMAINS: &[SeedDomain] = &[
        SeedDomain::new(b"terrain"),
        SeedDomain::new(b"hydrology"),
        SeedDomain::new(b"sites"),
        SeedDomain::new(b"monsters"),
        SeedDomain::new(b"items"),
        SeedDomain::new(b"combat"),
    ];

    #[test]
    fn domains_never_collide_on_the_same_index() {
        let seed = RunSeed(12345);
        for index in 0..64u64 {
            let mut seen = Vec::new();
            for d in DOMAINS {
                let derived = seed.derive(*d, index);
                assert!(!seen.contains(&derived), "{d:?} collided at {index}");
                seen.push(derived);
            }
        }
    }

    #[test]
    fn indices_never_collide_within_a_domain() {
        let seed = RunSeed(999);
        let mut seen = Vec::new();
        for index in 0..256u64 {
            let derived = seed.derive(DOMAINS[0], index);
            assert!(!seen.contains(&derived), "index {index} collided");
            seen.push(derived);
        }
    }

    #[test]
    fn a_different_run_seed_moves_every_stream() {
        for index in 0..32u64 {
            for d in DOMAINS {
                assert_ne!(RunSeed(1).derive(*d, index), RunSeed(2).derive(*d, index));
            }
        }
    }

    #[test]
    fn the_same_run_seed_reproduces_every_stream() {
        for index in 0..32u64 {
            for d in DOMAINS {
                let mut a = RunSeed(777).rng(*d, index);
                let mut b = RunSeed(777).rng(*d, index);
                assert_eq!(a.random::<u64>(), b.random::<u64>());
            }
        }
    }

    #[test]
    fn domain_names_key_the_salt() {
        assert_eq!(SeedDomain::new(b"x"), SeedDomain::new(b"x"));
        assert_ne!(SeedDomain::new(b"x"), SeedDomain::new(b"y"));
        assert_eq!(SeedDomain::new(b"anything").salt() & 1, 1);
    }

    #[test]
    fn neighbouring_entropy_readings_produce_unrelated_seeds() {
        let base = 1_234_567_890u64;
        let mut seen = Vec::new();
        for step in 0..1000u64 {
            let seed = RunSeed::from_entropy(base + step);
            assert!(!seen.contains(&seed), "+{step} repeated a seed");
            seen.push(seed);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn consecutive_fresh_seeds_in_one_process_never_repeat() {
        let mut seen = Vec::new();
        for i in 0..500 {
            let seed = RunSeed::fresh();
            assert!(!seen.contains(&seed), "run {i} repeated");
            seen.push(seed);
        }
    }

    #[test]
    fn position_hash_is_order_free_and_position_sensitive() {
        assert_eq!(position_hash(1, 3, 4), position_hash(1, 3, 4));
        assert_ne!(position_hash(1, 3, 4), position_hash(1, 4, 3));
        assert_ne!(position_hash(1, 3, 4), position_hash(2, 3, 4));
        assert_ne!(position_hash(1, -1, 0), position_hash(1, 0, -1));
    }

    #[test]
    fn pair_hash_agrees_from_both_sides_of_a_seam() {
        assert_eq!(pair_hash(9, (3, 4), (3, 5)), pair_hash(9, (3, 5), (3, 4)));
        assert_ne!(pair_hash(9, (3, 4), (3, 5)), pair_hash(9, (3, 4), (4, 4)));
        assert_ne!(pair_hash(9, (3, 4), (3, 5)), pair_hash(10, (3, 4), (3, 5)));
    }

    #[test]
    fn hash_below_stays_in_range() {
        for i in 0..10_000u64 {
            assert!(hash_below(mix64(i), 7) < 7);
        }
        let hits = (0..10_000u64).filter(|i| hash_below(mix64(*i), 4) == 0).count();
        assert!((2_200..=2_800).contains(&hits), "bucket 0 got {hits} of 10000");
    }
}
