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
/// Line droids, probe droids, heavy droids and coolant rats: the roster
/// loaded from `monsters.ron`, the spawn that gives one a brain, the
/// probe's alarm and the jam an ion hit leaves on radar.
pub mod droids;
/// The six weapons, six pieces of armor and slugs the decks are seeded
/// with, and the item entities they spawn as.
pub mod gear;
/// What a weapon that runs hot pays and sheds; fields only until
/// `heat`'s systems give them behaviour.
pub mod heat;
/// Items on the decks, scattered the moment each is first entered, and
/// items the dead leave behind.
pub mod loot;
/// The reactor console, `SetCharge`, and the mission it completes: deck
/// three's one objective, and the hinge into `upgrades`.
pub mod mission;
/// `FoundryPlugin`, the one list of Foundry's own systems that `main.rs`
/// and `testing::headless` both add.
pub mod plugin;
/// The run's start, added to [`plugin::FoundryPlugin`] in [`NewRun`](rl_engine::rl_bevy::plugin::NewRun).
pub mod run;
/// The headless harness, always built: an integration test links the
/// plain library, never the `#[cfg(test)]` build only `cargo test`'s own
/// unit tests get.
pub mod testing;
/// The one pick the first slice offers once its charge is set: three
/// permanent upgrades, and the screen that lets the player choose one.
pub mod upgrades;
