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
//! round any [`Barrier`] present, and publishes an [`Area`] per piece and
//! an [`Opening`] per [`Join`];
//! [`OpenLoops`] then cuts the longest detours through, so there is more
//! than one way round. [`Lakes`] lays water or anything else over the
//! floor without ever parting it, and [`Bridges`] crosses it where
//! walking round is long. A [`Cave`] is a shape not built of rectangles,
//! drawn by the cellular automaton, grown and joined like any other, and
//! a [`PrefabShape`](prefab::PrefabShape) is a hand-drawn piece grown as an
//! area of its own, entered only through the entries its file draws.
//! [`Survey`] measures what was built, as a
//! [`Surveyed`] with an [`AreaFacts`] per area, and [`Require`] fails a
//! place that falls short, for [`Chain::run_retrying`] to build again.
//! [`HideOpenings`] walls over a loop's shortcut or a dead end's one door
//! and publishes each cell as a [`Secret`], never one the way down needs.
//! [`StampPicks`] places the picks a run's pool drew for a place in the
//! areas each [`Prefer`]s, the most secluded, a dead end, a junction or a
//! room behind a secret, a piece stamped and a dressing published as a
//! [`DressAt`], and fails the place on a required one that fits nowhere.
//! [`Dress`] lays each [`Dressing`], things along the walls, in the
//! corners, in the center, in rows or scattered at a density, in the area
//! a `DressAt` names and in every room nothing took, by its district,
//! never beside an opening and never cutting an area in two.
//!
//! Engine passes are generic over the context, so a game extends the
//! context with its own state and engine passes still run on it.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod accrete;
pub mod cave;
pub mod chain;
pub mod context;
pub mod dress;
pub mod dungeon;
pub mod lakes;
pub mod loops;
pub mod passes;
pub mod pool;
pub mod prefab;
pub mod secrets;
pub mod survey;
#[cfg(test)]
mod testing;
mod walk;

pub use accrete::{Accrete, Area, Barrier, District, Join, LaidBarrier, Opening, Piece, Shape};
pub use cave::Cave;
pub use chain::{BuildError, Chain, Pass, Phase};
pub use context::{BaseContext, BuildContext, Outputs};
pub use dress::{DistrictDressings, Dress, Dressed, Dressing};
pub use lakes::{Bridge, Bridges, Lake, Lakes};
pub use loops::OpenLoops;
pub use pool::{DressAt, Lay, PoolPick, Prefer, StampPicks};
pub use secrets::{HideOpenings, Secret};
pub use survey::{AreaFacts, Require, Survey, Surveyed};

/// The names most callers want in scope.
pub mod prelude {
    // The pass and the trait only. What a game builds one from, `Rectangle`
    // above all, is named where it is built: Bevy's prelude has a
    // `Rectangle` of its own, and `rl-rules` an `Area`.
    pub use crate::accrete::{Accrete, Shape};
    pub use crate::chain::{BuildError, Chain, Pass, Phase};
    pub use crate::context::{BaseContext, BuildContext, Outputs};
    pub use crate::dress::{DistrictDressings, Dress, Dressed, Dressing};
    pub use crate::dungeon;
    pub use crate::lakes::{Bridges, Lakes};
    pub use crate::loops::OpenLoops;
    pub use crate::passes;
    pub use crate::pool::{DressAt, Lay, PoolPick, Prefer, StampPicks};
    pub use crate::prefab::{Orient, Placement, Prefab, PrefabShape, StampEachRoom, StampOneOf, StampPrefab, Stamped};
    pub use crate::secrets::{HideOpenings, Secret};
    pub use crate::survey::{AreaFacts, Require, Survey, Surveyed};
}
