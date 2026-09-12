//! A roguelike engine for Rust and Bevy, re-exported from one crate.
//!
//! rl-engine is a workspace of small crates for turn-based grid
//! roguelikes: procedural dungeon and world generation, field of view, A*
//! and Dijkstra-map pathfinding, monster AI, combat, items, quests, saves
//! and an ASCII glyph renderer. This facade re-exports every one of them,
//! so a game depends on this crate and imports `rl_engine::prelude::*`.
//!
//! The tier 0 and 1 crates ([`rl_core`], [`rl_grid`], [`rl_mapgen`],
//! [`rl_world`], [`rl_rules`]) never
//! depend on Bevy. A tool, a server or a test that needs no window can
//! depend on one of them directly and skip compiling Bevy altogether.
//! The tier-2 crates ([`rl_bevy`], [`rl_render`], [`rl_ui`],
//! [`rl_overworld`], [`rl_save`]) are the Bevy plugins that run the loops.
//!
//! The repository README walks through a headless example, and the
//! `corsair`, `delve` and `lamplight` example games show the Bevy side end
//! to end.

#![deny(missing_docs)]

pub use rl_bevy;
pub use rl_core;
pub use rl_grid;
pub use rl_mapgen;
pub use rl_overworld;
pub use rl_render;
pub use rl_rules;
pub use rl_save;
pub use rl_ui;
pub use rl_world;

/// The curated set of names a game needs most of the time.
pub mod prelude {
    pub use rl_core::prelude::*;
    pub use rl_grid::prelude::*;
}

/// The README's code samples, compiled and run as doc-tests so the
/// front page of the repository cannot drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
pub struct ReadmeDoctests;
