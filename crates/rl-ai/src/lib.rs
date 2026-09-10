//! Tactic-priority brains.
//!
//! A brain is an ordered list of [`Tactic`]s. Each turn the first tactic
//! that returns a decision wins; the trace of why an actor did something
//! is the name of the tactic that fired. Tactics see a [`Snapshot`] of
//! what the actor knows this turn and the shared [`DijkstraMap`](rl_grid::DijkstraMap)s for its
//! movement class, so fifty hunters cost one flood, not fifty searches.
//!
//! The engine ships the tactics every roguelike needs. A game adds its own
//! by implementing [`Tactic`] and inserting it anywhere in the list.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod brain;
pub mod profile;
pub mod snapshot;
pub mod tactics;

pub use brain::{Brain, Decision, Tactic, TacticCtx};
pub use profile::MovementProfile;
pub use snapshot::{ActorView, Snapshot};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::brain::{Brain, Decision, Tactic, TacticCtx};
    pub use crate::profile::MovementProfile;
    pub use crate::snapshot::{ActorView, Snapshot};
    pub use crate::tactics;
}
