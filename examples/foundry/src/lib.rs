//! Foundry: a commando's fight down through the decks of a droid foundry,
//! alone against whatever the floor below is still assembling.
//!
//! A library as well as a binary, unlike Corsair and Delve, because a
//! later integration test can only reach `content` and `testing` through
//! a crate the `tests/` directory links against, not through `main.rs`.

#![deny(missing_docs)]

pub mod ammo;
pub mod content;
pub mod decks;
pub mod droids;
pub mod gear;
pub mod heat;
pub mod input;
pub mod lifts;
pub mod light;
pub mod loot;
pub mod mission;
pub mod plugin;
pub mod run;
pub mod testing;
pub mod upgrades;
