//! What a mover can do, as a small set of flags.
//!
//! Two actors with the same profile see the same costs and share one
//! Dijkstra map. The engine names no flag; a game declares its own bits
//! and interprets them in its cost source.

use serde::{Deserialize, Serialize};

/// A bit set of movement capabilities, game-defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct MovementProfile(pub u32);

impl MovementProfile {}
