//! Tier 0 of rl-engine: the primitives every other crate is built on.
//!
//! Nothing in here knows about Bevy, rendering, or any particular game.
//! Everything is a function of its arguments, which is what lets the rest of
//! the engine be tested in milliseconds and benchmarked without an `App`.
//!
//! What lives here:
//!
//! - [`Point`], [`Rect`], [`Direction`]: addressing a grid.
//! - [`Grid<T>`] and the [`Grid2D`] trait: dense row-major storage and the
//!   index arithmetic every map type shares.
//! - [`geometry`]: distances, Bresenham lines, discs, cones, squares.
//! - [`RunSeed`] and [`SeedDomain`]: every random stream in a run derives
//!   from one root seed through a named domain, so subsystems cannot
//!   reshuffle one another; [`PlaceSeed`] is one place's share of it.
//! - [`DiceRoll`]: `NdS+B` notation parsed once at load.
//! - [`Id<T>`] and [`Interner<T>`]: cheap typed ids for content.
//! - [`noun`]: a thing's name said of one or of several, `a pebble` or
//!   `5 pebbles`, from the one name a game gives it.
//! - [`TurnQueue`]: the integer-clock scheduler.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod dice;
pub mod direction;
pub mod geometry;
pub mod grid;
pub mod id;
pub mod noun;
pub mod point;
pub mod seed;
pub mod turn;

pub use dice::DiceRoll;
pub use direction::Direction;
pub use grid::{Grid, Grid2D, Steps};
pub use id::{Id, Interner};
pub use point::{Point, Rect};
pub use seed::{PlaceSeed, RunSeed, SeedDomain};
pub use turn::{DequeueOutcome, TurnQueue};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::dice::DiceRoll;
    pub use crate::direction::Direction;
    pub use crate::geometry;
    pub use crate::grid::{Grid, Grid2D, Steps};
    pub use crate::id::{Id, Interner};
    pub use crate::point::{Point, Rect};
    pub use crate::seed::{PlaceSeed, RunSeed, SeedDomain};
    pub use crate::turn::{BASE_ACTION_COST, DequeueOutcome, TurnQueue};
}
