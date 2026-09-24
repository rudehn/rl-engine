//! Saving: the seam to storage, the versioning policy, entity remapping,
//! and the engine's own state as a save.
//!
//! The engine cannot know what a player is made of, so a game says what
//! each kind of thing it spawns is, through [`Saveable`], and the engine
//! walks the world: [`RunSave`] holds every kind's entries in the kind's
//! own words, the engine's state on each of them, the game's resources
//! through [`SaveableState`], and [`EngineSave`], the scheduler, the
//! world's edits and places, and what the player knows. Around it: a
//! [`SaveBackend`] with a file and a memory implementation (and a
//! `localStorage` one on wasm), a [`Versioned`] envelope whose version
//! must match exactly or the save is refused whole, an [`EntityRemap`]
//! that turns entity ids into stable [`SaveId`]s and back, [`SavePlugin`],
//! which keeps the [`Stash`] a turn behind the run and forgets the slot
//! when the run ends, and [`UnloadPlugin`], which writes the stash when
//! the page or the window is closed on it.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod backend;
pub mod engine;
pub mod remap;
pub mod run;
pub mod unload;
pub mod versioned;

pub use backend::{FileBackend, MemoryBackend, SaveBackend, SaveError, Saves};
pub use engine::EngineSave;
pub use remap::{EntityRemap, SaveId};
pub use run::{
    AddSaveable, EntityState, KindSave, RunSave, SavePlugin, SaveRegistry, SaveSlot, Saveable, SaveableState, forget_save, load_run, save_on_arrival, save_run,
};
pub use unload::{Stash, UnloadPlugin};
pub use versioned::{Versioned, decode, encode};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::backend::{FileBackend, MemoryBackend, SaveBackend, SaveError, Saves};
    pub use crate::engine::EngineSave;
    pub use crate::remap::{EntityRemap, SaveId};
    pub use crate::run::{AddSaveable, RunSave, SavePlugin, Saveable, SaveableState, load_run, save_run};
    pub use crate::unload::{Stash, UnloadPlugin};
    pub use crate::versioned::{Versioned, decode, encode};
}
