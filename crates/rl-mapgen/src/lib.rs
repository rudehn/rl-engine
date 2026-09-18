//! The generation pipeline: a chain of named, phased passes over a context.
//!
//! Every generated thing in the engine, a dungeon floor or a chunk of the
//! open world, is built by running a [`Chain`] of [`Pass`]es over a
//! [`BuildContext`]. Three rules make the chain composable:
//!
//! - **A pass's name keys its random stream.** Inserting, removing or
//!   reordering a pass cannot change what any other pass draws.
//! - **Phase order is mandatory.** A chain whose phases go backwards is
//!   refused when it is assembled, not discovered in a screenshot.
//! - **Passes can fail.** A pass that cannot place what it was asked to
//!   says so, and the caller retries with a different index.
//!
//! Engine passes are generic over the context, so a game extends the
//! context with its own state and engine passes still run on it.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod chain;
pub mod context;
pub mod dungeon;
pub mod passes;
pub mod prefab;

pub use chain::{BuildError, Chain, Pass, Phase};
pub use context::{BaseContext, BuildContext, Outputs};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::chain::{BuildError, Chain, Pass, Phase};
    pub use crate::context::{BaseContext, BuildContext, Outputs};
    pub use crate::dungeon;
    pub use crate::passes;
    pub use crate::prefab::{Orient, Placement, Prefab, StampOneOf, StampPrefab, Stamped};
}
