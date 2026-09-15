//! Where saves go.
//!
//! Four synchronous methods over named slots. A missing save is never an
//! error: `load` gives `None`, `delete` succeeds, `exists` says no. Only
//! a real I/O or storage failure is an `Err`, and callers surface those
//! rather than swallow them: a delete that silently failed leaves a
//! save the player can continue into after the death that should have
//! removed it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use bevy::prelude::Resource;

/// Why a backend operation failed.
#[derive(Debug)]
pub enum SaveError {
    /// The filesystem said no.
    Io(std::io::Error),
    /// Browser storage was unavailable or full.
    Storage,
    /// The save could not be written as text.
    Encode(String),
    /// The save could not be read back.
    Decode(String),
    /// The save was written by a different version of the schema.
    Version {
        /// What the save says.
        found: u32,
        /// What this build expects.
        expected: u32,
    },
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Io(e) => write!(f, "save I/O failed: {e}"),
            SaveError::Storage => write!(f, "browser storage is unavailable"),
            SaveError::Encode(e) => write!(f, "save could not be encoded: {e}"),
            SaveError::Decode(e) => write!(f, "save could not be decoded: {e}"),
            SaveError::Version { found, expected } => write!(f, "save is version {found}, this build reads version {expected}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Persistent storage for text saves, by slot name.
pub trait SaveBackend {
    /// Writes `text` to `slot`, replacing what was there.
    fn persist(&self, slot: &str, text: &str) -> Result<(), SaveError>;
    /// Reads `slot`, or `None` if nothing was ever saved there.
    fn load(&self, slot: &str) -> Result<Option<String>, SaveError>;
    /// Removes `slot`. Succeeds when there was nothing to remove.
    fn delete(&self, slot: &str) -> Result<(), SaveError>;
    /// Whether `slot` holds a save.
    fn exists(&self, slot: &str) -> bool;
}

/// Saves as files in a directory, one per slot.
pub struct FileBackend {
    dir: std::path::PathBuf,
}

impl FileBackend {
    /// Saves under `dir`, created on first write.
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Saves next to the running executable.
    pub fn beside_executable() -> Self {
        let dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|d| d.to_path_buf())).unwrap_or_else(|| std::path::PathBuf::from("."));
        Self::new(dir)
    }

    fn path(&self, slot: &str) -> std::path::PathBuf {
        self.dir.join(format!("{slot}.save.ron"))
    }
}

impl SaveBackend for FileBackend {
    fn persist(&self, slot: &str, text: &str) -> Result<(), SaveError> {
        std::fs::create_dir_all(&self.dir).map_err(SaveError::Io)?;
        std::fs::write(self.path(slot), text).map_err(SaveError::Io)
    }

    fn load(&self, slot: &str) -> Result<Option<String>, SaveError> {
        match std::fs::read_to_string(self.path(slot)) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SaveError::Io(e)),
        }
    }

    fn delete(&self, slot: &str) -> Result<(), SaveError> {
        match std::fs::remove_file(self.path(slot)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(SaveError::Io(e)),
        }
    }

    fn exists(&self, slot: &str) -> bool {
        self.path(slot).exists()
    }
}

/// Saves in memory, for tests and for a run that must not touch disk.
#[derive(Default)]
pub struct MemoryBackend {
    slots: Mutex<BTreeMap<String, String>>,
}

impl SaveBackend for MemoryBackend {
    fn persist(&self, slot: &str, text: &str) -> Result<(), SaveError> {
        self.slots.lock().expect("save slots").insert(slot.to_string(), text.to_string());
        Ok(())
    }

    fn load(&self, slot: &str) -> Result<Option<String>, SaveError> {
        Ok(self.slots.lock().expect("save slots").get(slot).cloned())
    }

    fn delete(&self, slot: &str) -> Result<(), SaveError> {
        self.slots.lock().expect("save slots").remove(slot);
        Ok(())
    }

    fn exists(&self, slot: &str) -> bool {
        self.slots.lock().expect("save slots").contains_key(slot)
    }
}

/// Saves in the browser's `localStorage`, one key per slot.
#[cfg(target_arch = "wasm32")]
pub struct WebBackend {
    prefix: String,
}

#[cfg(target_arch = "wasm32")]
impl WebBackend {
    /// Keys are `prefix:slot`.
    pub fn new(prefix: impl Into<String>) -> Self {
        Self { prefix: prefix.into() }
    }

    fn storage() -> Result<web_sys::Storage, SaveError> {
        web_sys::window().and_then(|w| w.local_storage().ok().flatten()).ok_or(SaveError::Storage)
    }

    fn key(&self, slot: &str) -> String {
        format!("{}:{slot}", self.prefix)
    }
}

#[cfg(target_arch = "wasm32")]
impl SaveBackend for WebBackend {
    fn persist(&self, slot: &str, text: &str) -> Result<(), SaveError> {
        Self::storage()?.set_item(&self.key(slot), text).map_err(|_| SaveError::Storage)
    }

    fn load(&self, slot: &str) -> Result<Option<String>, SaveError> {
        Self::storage()?.get_item(&self.key(slot)).map_err(|_| SaveError::Storage)
    }

    fn delete(&self, slot: &str) -> Result<(), SaveError> {
        Self::storage()?.remove_item(&self.key(slot)).map_err(|_| SaveError::Storage)
    }

    fn exists(&self, slot: &str) -> bool {
        matches!(self.load(slot), Ok(Some(_)))
    }
}

/// The backend a game saves through, as a resource, so no system
/// branches on the platform.
///
/// Shared rather than owned, so the unload bridge's handler, which runs
/// outside the app, writes through the same backend the game does.
#[derive(Resource)]
pub struct Saves(pub Arc<dyn SaveBackend + Send + Sync>);

impl Saves {
    /// The platform's default: files beside the executable, or browser
    /// storage under `name` on wasm.
    pub fn platform_default(name: &str) -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self(Arc::new(WebBackend::new(name)))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = name;
            Self(Arc::new(FileBackend::beside_executable()))
        }
    }

    /// Saves through `backend`.
    pub fn new(backend: impl SaveBackend + Send + Sync + 'static) -> Self {
        Self(Arc::new(backend))
    }
}

impl SaveBackend for Saves {
    fn persist(&self, slot: &str, text: &str) -> Result<(), SaveError> {
        self.0.persist(slot, text)
    }
    fn load(&self, slot: &str) -> Result<Option<String>, SaveError> {
        self.0.load(slot)
    }
    fn delete(&self, slot: &str) -> Result<(), SaveError> {
        self.0.delete(slot)
    }
    fn exists(&self, slot: &str) -> bool {
        self.0.exists(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_round_trip_and_a_missing_save_is_not_an_error() {
        let dir = std::env::temp_dir().join(format!("rl-save-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let b = FileBackend::new(&dir);
        assert!(!b.exists("a"));
        assert!(matches!(b.load("a"), Ok(None)));
        assert!(b.delete("a").is_ok(), "deleting nothing is fine");
        b.persist("a", "(x: 1)").unwrap();
        assert!(b.exists("a"));
        assert_eq!(b.load("a").unwrap().as_deref(), Some("(x: 1)"));
        b.delete("a").unwrap();
        assert!(!b.exists("a"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn memory_slots_are_independent() {
        let b = MemoryBackend::default();
        b.persist("one", "1").unwrap();
        b.persist("two", "2").unwrap();
        assert_eq!(b.load("one").unwrap().as_deref(), Some("1"));
        b.delete("one").unwrap();
        assert!(!b.exists("one") && b.exists("two"));
    }
}
