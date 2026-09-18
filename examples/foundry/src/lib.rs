//! Foundry: a commando's fight down through the decks of a droid foundry,
//! alone against whatever the floor below is still assembling.
//!
//! A library as well as a binary, unlike Corsair and Delve, because a
//! later integration test can only reach `content` and `testing` through
//! a crate the `tests/` directory links against, not through `main.rs`.

#![deny(missing_docs)]

/// What one shot of an ammunition-fed weapon spends; fields only until
/// `ammo::spend_ammo` gives them behaviour.
pub mod ammo;
pub mod content;
pub mod decks;
/// The six weapons, six pieces of armor and slugs the decks are seeded
/// with, and the item entities they spawn as.
pub mod gear;
/// What a weapon that runs hot pays and sheds; fields only until
/// `heat`'s systems give them behaviour.
pub mod heat;
/// The run's start, called from `main.rs` and from `testing::headless`.
pub mod run;
/// The headless harness, always built: an integration test links the
/// plain library, never the `#[cfg(test)]` build only `cargo test`'s own
/// unit tests get.
pub mod testing;
