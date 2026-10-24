//! Tier 0 of atglyph: the primitives every other crate is built on.
//!
//! Use this crate alone, or through `atglyph` as `atglyph::primitives`.
//! It is the grid, seed and dice vocabulary the other crates share, builds with no Bevy, and builds for `wasm32-unknown-unknown`,
//! so a tool, a server or a test can depend on `atglyph-core` and skip compiling Bevy.
//!
//! ```
//! use atglyph_core::{DiceRoll, RunSeed, SeedDomain};
//!
//! // One run seed, one named domain: the same stream every time.
//! let mut rng = RunSeed(7).rng(SeedDomain::new(b"demo"), 0);
//! let damage = DiceRoll::new(2, 6).roll(&mut rng);
//! assert!((2..=12).contains(&damage));
//! ```
//!
//! Nothing in here knows about Bevy, rendering, or any particular game.
//! Everything is a function of its arguments, which is what lets the rest of
//! the engine be tested in milliseconds and benchmarked without an `App`.
//!
//! What lives here:
//!
//! - [`Point`], [`GridRect`], [`Direction`]: addressing a grid.
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
pub use point::{GridRect, Point};
pub use seed::{PlaceSeed, RunSeed, SeedDomain};
pub use turn::{DequeueOutcome, TurnQueue};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::dice::DiceRoll;
    pub use crate::direction::Direction;
    pub use crate::geometry;
    pub use crate::grid::{Grid, Grid2D, Steps};
    pub use crate::id::{Id, Interner};
    pub use crate::point::{GridRect, Point};
    pub use crate::seed::{PlaceSeed, RunSeed, SeedDomain};
    pub use crate::turn::{BASE_ACTION_COST, DequeueOutcome, TurnQueue};
}
