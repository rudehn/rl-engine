//! Every domain Undermere draws from, named once.
//!
//! A domain is the name a stream is keyed by, so a typo at a draw site would
//! be a new stream that nothing else rolls on; naming each here once, with
//! [`ALL`] checked for repeats and against the engine's own domains, means
//! two draws can only share a stream by naming the same constant.

use rl_engine::rl_core::SeedDomain;

/// The run's picks from the pool, every floor's at once.
pub const POOL: SeedDomain = SeedDomain::new(b"undermere.pool");
/// Which floor holds the captive.
pub const CAPTIVE: SeedDomain = SeedDomain::new(b"undermere.captive");
/// Which of the final bosses sits the throne.
pub const FINAL_BOSS: SeedDomain = SeedDomain::new(b"undermere.final_boss");
/// Which floors hold a mini-boss's lair.
pub const LAIRS: SeedDomain = SeedDomain::new(b"undermere.lairs");
/// The recipe a floor's biome grows by, indexed by floor.
pub const RECIPE: SeedDomain = SeedDomain::new(b"undermere.recipe");
/// The biome a floor draws, indexed by floor.
pub const BIOME: SeedDomain = SeedDomain::new(b"undermere.biome");
/// The item faces of the run, one index per family.
pub const LORE: SeedDomain = SeedDomain::new(b"undermere.lore");
/// The run's names for Mereholt's people and the swords for hire.
pub const NAMES: SeedDomain = SeedDomain::new(b"undermere.names");
/// A visit's candidates in the tavern, the visit's number its index.
pub const TAVERN: SeedDomain = SeedDomain::new(b"undermere.tavern");
/// A shop's stocking, the stocking's number its index.
pub const WARES: SeedDomain = SeedDomain::new(b"undermere.wares");
/// The party a photograph starts with.
pub const PARTY: SeedDomain = SeedDomain::new(b"undermere.start-party");
// Drawn from the run's seed, by floor or once: what the run decides
// before any floor is built, or what belongs to the run and no one place.
// Below, drawn from a place's own seed: what one floor holds, so building
// or entering floors in any order changes none of them.

/// Where a floor's vault key and lever lie.
pub const VAULT_FIXTURES: SeedDomain = SeedDomain::new(b"undermere.vaults.fixtures");
/// Who sleeps at a vault's marks.
pub const VAULT_MARKS: SeedDomain = SeedDomain::new(b"undermere.vaults.marks");
/// Which lines a floor's writings read.
pub const WRITINGS: SeedDomain = SeedDomain::new(b"undermere.writings");
/// What lies loose on a floor.
pub const FINDS: SeedDomain = SeedDomain::new(b"undermere.finds");
/// Who stands on a floor: three streams of it, by index, the ordinary
/// groups, the bands and the rest.
pub const POPULATION: SeedDomain = SeedDomain::new(b"undermere.population");

/// Every domain above, by name.
pub const ALL: &[(&str, SeedDomain)] = &[
    ("POOL", POOL),
    ("CAPTIVE", CAPTIVE),
    ("FINAL_BOSS", FINAL_BOSS),
    ("LAIRS", LAIRS),
    ("RECIPE", RECIPE),
    ("BIOME", BIOME),
    ("LORE", LORE),
    ("NAMES", NAMES),
    ("TAVERN", TAVERN),
    ("WARES", WARES),
    ("PARTY", PARTY),
    ("VAULT_FIXTURES", VAULT_FIXTURES),
    ("VAULT_MARKS", VAULT_MARKS),
    ("WRITINGS", WRITINGS),
    ("FINDS", FINDS),
    ("POPULATION", POPULATION),
];

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_bevy::ENGINE_DOMAINS;

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
