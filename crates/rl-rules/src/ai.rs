//! Tactic-priority brains.
//!
//! A brain is an ordered list of [`Tactic`]s. Each turn the first tactic
//! that returns a decision wins; the trace of why an actor did something
//! is the name of the tactic that fired. Tactics see a [`Snapshot`] of
//! what the actor knows this turn and the shared [`DijkstraMap`](rl_grid::DijkstraMap)s for its
//! movement class, so fifty hunters cost one flood, not fifty searches.
//!
//! - [`awareness`]: [`NoticeStats`], [`StealthStats`], [`notices`] and [`Awareness`]: who has noticed whom.
//! - [`brain`]: [`Brain`], [`Tactic`], [`TacticCtx`] and [`Decision`].
//! - [`profile`]: [`MovementProfile`], the movement class flow fields are shared by.
//! - [`snapshot`]: [`Snapshot`] and [`ActorView`].
//! - [`tactics`]: the tactics every roguelike needs.
//!
//! A game adds its own by implementing [`Tactic`] and inserting it
//! anywhere in the list.

pub mod awareness;
pub mod brain;
pub mod profile;
pub mod snapshot;
pub mod tactics;

pub use awareness::{Awareness, NoticeStats, StealthStats, notices};
pub use brain::{Brain, Decision, Tactic, TacticCtx};
pub use profile::MovementProfile;
pub use snapshot::{ActorView, Snapshot};
