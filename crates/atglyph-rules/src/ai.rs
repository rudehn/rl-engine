//! Tactic-priority brains.
//!
//! A brain is an ordered list of [`Tactic`]s. Each turn the first tactic
//! that returns a decision wins; the trace of why an actor did something
//! is the name of the tactic that fired. Tactics see a [`Snapshot`] of
//! what the actor knows this turn and ask [`Fields`] for the way toward or
//! away from any cells, shared by movement class and goal, so fifty hunters
//! after one player cost one flood, not fifty searches.
//!
//! - [`awareness`]: [`NoticeStats`], [`StealthStats`], [`notices`] and [`Awareness`]: who has noticed whom.
//! - [`hearing`]: [`HearingStats`], and how far a sound carries: [`carries`](hearing::carries), [`left_after`](hearing::left_after) and [`heard`](hearing::heard).
//! - [`brain`]: [`Brain`], [`Tactic`], [`TacticCtx`], [`Decision`], [`Choice`], a decision of the game's own, and [`Fields`], the way toward or away from any cells.
//! - [`profile`]: [`MovementProfile`], the movement class flow fields are shared by.
//! - [`recent`]: [`Recent`](recent::Recent), a bounded cache that forgets what was asked for longest ago.
//! - [`roster`]: [`sort_roster`], who a mind sees by how it regards them, and the [`Trail`] it follows.
//! - [`step`]: [`StepAs`] and [`step_as`], what a decision to step becomes.
//! - [`snapshot`]: [`Snapshot`], [`ActorView`], what it carries and sees lying about, [`Missile`], [`Carried`], [`ItemView`] and [`PropView`], and a game's own [`Sense`].
//! - [`tactics`]: the tactics every roguelike needs.
//! - [`standard`]: [`Standard`], the chain every game with monsters wrote
//!   by hand, as a game's choices of which steps it holds and every number
//!   a step needs, built for one kind from its [`Repertoire`] into a
//!   [`StandardBrain`] with a game's own tactics at named points.
//! - [`wits`]: [`Wits`], what a mind is able to do whatever its brain would
//!   like, as capabilities with mindless, animal and sapient presets.
//!
//! A game adds its own by implementing [`Tactic`] and inserting it
//! anywhere in the list.

pub mod awareness;
pub mod brain;
pub mod hearing;
pub mod profile;
pub mod recent;
pub mod roster;
pub mod snapshot;
pub mod standard;
pub mod step;
pub mod tactics;
pub mod wits;

pub use awareness::{Awareness, NoticeStats, StealthStats, notices};
pub use brain::{Brain, Choice, Decision, Fields, NoFields, Tactic, TacticCtx};
pub use hearing::HearingStats;
pub use profile::MovementProfile;
pub use roster::{Trail, sort_roster};
pub use snapshot::{ActorView, Carried, ItemView, Missile, PropView, Sense, Snapshot, Vitals};
pub use standard::{Repertoire, Standard, StandardBrain};
pub use step::{StepAs, step_as};
pub use wits::Wits;
