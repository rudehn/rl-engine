//! Every domain Foundry draws from, named once.
//!
//! A domain is the name a stream is keyed by, so a typo at a draw site would
//! be a new stream that nothing else rolls on; naming each here once, with
//! [`ALL`] checked for repeats and against the engine's own domains, means
//! two draws can only share a stream by naming the same constant.

use atglyph::primitives::SeedDomain;

/// The loose props a deck is dressed with.
pub const PROPS: SeedDomain = SeedDomain::new(b"foundry.props");
/// A deck's droids on the first arrival.
pub const SPAWNS: SeedDomain = SeedDomain::new(b"foundry.spawns");
/// A deck's droids on a revisit, the climb.
pub const CLIMB: SeedDomain = SeedDomain::new(b"foundry.climb");

/// Every domain above, by name.
pub const ALL: &[(&str, SeedDomain)] = &[("PROPS", PROPS), ("SPAWNS", SPAWNS), ("CLIMB", CLIMB)];

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph::ecs::ENGINE_DOMAINS;

    #[test]
    fn every_seed_domain_the_game_draws_from_is_distinct() {
        for (i, (name, domain)) in ALL.iter().enumerate() {
            for (other, theirs) in &ALL[i + 1..] {
                assert_ne!(name, other, "{name} is listed twice");
                assert_ne!(domain, theirs, "{name} and {other} share a stream");
            }
            for (engine, theirs) in ENGINE_DOMAINS {
                assert_ne!(domain, theirs, "{name} shares the engine's {engine} stream");
            }
        }
    }
}
