//! What a mover can do, as a small set of flags.
//!
//! Two actors with the same profile see the same costs and share one
//! Dijkstra map. The engine names no flag; a game declares its own bits
//! and interprets them in its cost source.

use serde::{Deserialize, Serialize};

/// A bit set of movement capabilities, game-defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct MovementProfile(pub u32);

impl MovementProfile {
    /// No capabilities beyond walking.
    pub const WALKER: MovementProfile = MovementProfile(0);

    /// Whether every bit in `flag` is set.
    pub const fn has(self, flag: MovementProfile) -> bool {
        self.0 & flag.0 == flag.0
    }

    /// This profile with `flag` added.
    pub const fn with(self, flag: MovementProfile) -> MovementProfile {
        MovementProfile(self.0 | flag.0)
    }
}
