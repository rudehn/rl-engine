//! Saving: the seam to storage, the versioning policy, entity remapping,
//! and the engine's own state as a save.
//!
//! The engine cannot know what a player is made of, so a game says what
//! each kind of thing it spawns is, through [`Saveable`], or for an item
//! it makes through its `ItemMaker`, through [`SaveableItem`], which the
//! engine writes as its definition and level and makes again at that
//! level before the game's own extra goes back on, or for an actor it
//! makes through its `ActorMaker`, through [`SaveableActor`], written as
//! its definition and made again before the game's extra, and the engine
//! walks the world: [`RunSave`] holds every kind's entries in the kind's
//! own words, the engine's state on each of them, the game's resources
//! through [`SaveableState`], and [`EngineSave`], the scheduler, the
//! world's edits and places, and what the player knows. Around it: a
//! [`SaveBackend`] with a file and a memory implementation (and a
//! `localStorage` one on wasm), a [`Versioned`] envelope whose version
//! must match exactly or the save is refused whole, an [`EntityRemap`]
//! that turns entity ids into stable [`SaveId`]s and back, and
//! [`SavePlugin`], which inserts the backend, in memory for a scripted
//! run, keeps the [`Stash`] a turn behind the run and writes it when the
//! page or the window is closed on it, forgets the slot when the run ends
//! and keeps a title told what the slot holds.
//!
//! Apart from all of that, `SavePlugin` remembers what a player chose in
//! the [`settings`] slot, [`SETTINGS_SLOT`], which no run's ending
//! forgets, and in a game that keeps records every run's [record](records)
//! the same way, in [`RECORDS_SLOT`].

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod backend;
pub mod engine;
pub mod records;
pub mod remap;
pub mod run;
pub mod settings;
pub mod unload;
pub mod versioned;

pub use backend::{FileBackend, MemoryBackend, SaveBackend, SaveError, Saves};
pub use engine::EngineSave;
pub use records::RECORDS_SLOT;
pub use remap::{EntityRemap, SaveId};
pub use run::{
    AddSaveable, EntityState, KindSave, RunSave, SavePlugin, SaveRegistry, SaveSlot, Saveable, SaveableActor, SaveableItem, SaveableState, SavedActor,
    SavedBurning, SavedItem, forget_save, load_run, save_on_arrival, save_on_leaving, save_run,
};
pub use settings::SETTINGS_SLOT;
pub use unload::Stash;
pub use versioned::{Versioned, decode, encode};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::backend::{FileBackend, MemoryBackend, SaveBackend, SaveError, Saves};
    pub use crate::engine::EngineSave;
    pub use crate::remap::{EntityRemap, SaveId};
    pub use crate::run::{AddSaveable, RunSave, SavePlugin, Saveable, SaveableActor, SaveableItem, SaveableState, SavedActor, SavedItem, load_run, save_run};
    pub use crate::unload::Stash;
    pub use crate::versioned::{Versioned, decode, encode};
}
