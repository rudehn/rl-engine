//! Saving: the seam to storage, the versioning policy, entity remapping,
//! and the engine's own state as a save.
//!
//! A game's save is its own struct; the engine cannot know what a player
//! is made of. What the engine ships is everything around that struct:
//! a [`SaveBackend`] with a file and a memory implementation (and a
//! `localStorage` one on wasm), a [`Versioned`] envelope whose version
//! must match exactly or the save is refused whole, an [`EntityRemap`]
//! that turns entity ids into stable [`SaveId`]s and back, and
//! [`EngineSave`], the scheduler, the world's edits and places, and what
//! the player knows, captured from and restored into a Bevy world. A game
//! captures its entities, asks for the engine's state, and writes one
//! versioned blob.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod backend;
pub mod engine;
pub mod remap;
pub mod versioned;

pub use backend::{FileBackend, MemoryBackend, SaveBackend, SaveError, Saves};
pub use engine::EngineSave;
pub use remap::{EntityRemap, SaveId};
pub use versioned::{Versioned, decode, encode};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::backend::{FileBackend, MemoryBackend, SaveBackend, SaveError, Saves};
    pub use crate::engine::EngineSave;
    pub use crate::remap::{EntityRemap, SaveId};
    pub use crate::versioned::{Versioned, decode, encode};
}
