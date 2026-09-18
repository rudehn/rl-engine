//! Foundry: a commando's fight down through the decks of a droid foundry,
//! alone against whatever the floor below is still assembling.
//!
//! A library as well as a binary, unlike Corsair and Delve, because a
//! later integration test can only reach `content` and `testing` through
//! a crate the `tests/` directory links against, not through `main.rs`.

#![deny(missing_docs)]

pub mod content;
pub mod decks;
/// The run's start, called from `main.rs` and from `testing::headless`.
pub mod run;
/// The headless harness, always built: an integration test links the
/// plain library, never the `#[cfg(test)]` build only `cargo test`'s own
/// unit tests get.
pub mod testing;
