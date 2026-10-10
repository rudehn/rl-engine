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
//! A place can also be grown: [`Accrete`] attaches each [`Shape`]'s
//! [`Piece`] to a wall of what already stands, biased by [`District`]s,
//! and publishes an [`Area`] per piece and an [`Opening`] per [`Join`];
//! [`OpenLoops`] then cuts the longest detours through, so there is more
//! than one way round. A [`Cave`] is a shape not built of rectangles,
//! drawn by the cellular automaton, grown and joined like any other.
//! [`Survey`] measures what was built, as a
//! [`Surveyed`] with an [`AreaFacts`] per area, and [`Require`] fails a
//! place that falls short, for [`Chain::run_retrying`] to build again.
//!
//! Engine passes are generic over the context, so a game extends the
//! context with its own state and engine passes still run on it.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod accrete;
pub mod cave;
pub mod chain;
pub mod context;
pub mod dungeon;
pub mod loops;
pub mod passes;
pub mod prefab;
pub mod survey;
mod walk;

pub use accrete::{Accrete, Area, District, Join, Opening, Piece, Shape};
pub use cave::Cave;
pub use chain::{BuildError, Chain, Pass, Phase};
pub use context::{BaseContext, BuildContext, Outputs};
pub use loops::OpenLoops;
pub use survey::{AreaFacts, Require, Survey, Surveyed};

/// The names most callers want in scope.
pub mod prelude {
    // The pass and the trait only. What a game builds one from, `Rectangle`
    // above all, is named where it is built: Bevy's prelude has a
    // `Rectangle` of its own, and `rl-rules` an `Area`.
    pub use crate::accrete::{Accrete, Shape};
    pub use crate::chain::{BuildError, Chain, Pass, Phase};
    pub use crate::context::{BaseContext, BuildContext, Outputs};
    pub use crate::dungeon;
    pub use crate::loops::OpenLoops;
    pub use crate::passes;
    pub use crate::prefab::{Orient, Placement, Prefab, StampEachRoom, StampOneOf, StampPrefab, Stamped};
    pub use crate::survey::{AreaFacts, Require, Survey, Surveyed};
}
