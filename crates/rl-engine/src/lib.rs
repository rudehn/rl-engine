//! Facade over the rl-engine workspace.
//!
//! Games depend on this crate and import `rl_engine::prelude::*`.
//! Each tier-1 crate is also usable on its own for tools that need no Bevy.

#![deny(missing_docs)]

pub use rl_ai;
pub use rl_bevy;
pub use rl_content;
pub use rl_core;
pub use rl_events;
pub use rl_grid;
pub use rl_mapgen;
pub use rl_overworld;
pub use rl_render;
pub use rl_rules;
pub use rl_ui;
pub use rl_world;

/// The curated set of names a game needs most of the time.
pub mod prelude {
    pub use rl_core::prelude::*;
    pub use rl_grid::prelude::*;
}
