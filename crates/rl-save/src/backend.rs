//! Where saves go.
//!
//! Four synchronous methods over named slots. A missing save is never an
//! error: `load` gives `None`, `delete` succeeds, `exists` says no. Only
//! a real I/O or storage failure is an `Err`, and callers surface those
//! rather than swallow them: a delete that silently failed leaves a
//! save the player can continue into after the death that should have
//! removed it.
//!
//! A slot is a plain name, ASCII letters, digits, `-` and `_`, and every
//! backend here refuses any other with [`SaveError::Slot`]. The file
//! backend needs the rule, since a slot becomes a file name and `../x` or
//! an absolute path would write outside its directory; the others keep it
//! so a slot a game's tests save to in memory is one the player's disk
//! takes too.

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
    /// The slot was not a plain name: empty, or holding something other
    /// than ASCII letters, digits, `-` and `_`.
    Slot(String),
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
            SaveError::Slot(s) => write!(f, "{s:?} is not a slot name"),
            SaveError::Version { found, expected } => write!(f, "save is version {found}, this build reads version {expected}"),
        }
    }
}

impl std::error::Error for SaveError {}

/// Whether `slot` is a name every backend takes: not empty, and nothing
/// but ASCII letters, digits, `-` and `_`.
///
/// Narrower than any one filesystem allows, so a slot that works on one
/// platform works on all of them: no separator, no dot to climb with or
/// to collide with the extension, no Unicode a filesystem might
/// normalize onto another slot's file. Case is allowed, and two slots
/// that differ only in it share a file where the filesystem folds case.
pub(crate) fn check_slot(slot: &str) -> Result<(), SaveError> {
    if !slot.is_empty() && slot.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { Ok(()) } else { Err(SaveError::Slot(slot.to_string())) }
}

/// Persistent storage for text saves, by slot name.
///
/// An implementation refuses a slot that is not a plain name, ASCII
/// letters, digits, `-` and `_`, with [`SaveError::Slot`], and says such a
/// slot does not exist.
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
    /// What a slot's file ends in: `save.ron` unless a game asked for
    /// something else.
    extension: String,
}

impl FileBackend {
    /// Saves under `dir`, created on first write.
    pub fn new(dir: impl Into<std::path::PathBuf>) -> Self {
        Self { dir: dir.into(), extension: "save.ron".into() }
    }

    /// The directory the running executable is in, or the working
    /// directory when that cannot be found.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn dir_beside_executable() -> std::path::PathBuf {
        std::env::current_exe().ok().and_then(|exe| exe.parent().map(|d| d.to_path_buf())).unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    /// Where `slot` is written, always a file directly inside the
    /// directory, or [`SaveError::Slot`] when `slot` is not a plain name.
    pub(crate) fn path(&self, slot: &str) -> Result<std::path::PathBuf, SaveError> {
        check_slot(slot)?;
        Ok(self.dir.join(format!("{slot}.{}", self.extension)))
    }
}

impl SaveBackend for FileBackend {
    fn persist(&self, slot: &str, text: &str) -> Result<(), SaveError> {
        let path = self.path(slot)?;
        std::fs::create_dir_all(&self.dir).map_err(SaveError::Io)?;
        std::fs::write(path, text).map_err(SaveError::Io)
    }

    fn load(&self, slot: &str) -> Result<Option<String>, SaveError> {
        match std::fs::read_to_string(self.path(slot)?) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SaveError::Io(e)),
        }
    }

    fn delete(&self, slot: &str) -> Result<(), SaveError> {
        match std::fs::remove_file(self.path(slot)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(SaveError::Io(e)),
        }
    }

    fn exists(&self, slot: &str) -> bool {
        self.path(slot).is_ok_and(|p| p.exists())
    }
}

/// Saves in memory, for tests and for a run that must not touch disk.
#[derive(Default)]
pub struct MemoryBackend {
    slots: Mutex<BTreeMap<String, String>>,
}

impl SaveBackend for MemoryBackend {
    fn persist(&self, slot: &str, text: &str) -> Result<(), SaveError> {
        check_slot(slot)?;
        self.slots.lock().expect("save slots").insert(slot.to_string(), text.to_string());
        Ok(())
    }

    fn load(&self, slot: &str) -> Result<Option<String>, SaveError> {
        check_slot(slot)?;
        Ok(self.slots.lock().expect("save slots").get(slot).cloned())
    }

    fn delete(&self, slot: &str) -> Result<(), SaveError> {
        check_slot(slot)?;
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

    fn key(&self, slot: &str) -> Result<String, SaveError> {
        check_slot(slot)?;
        Ok(format!("{}:{slot}", self.prefix))
    }
}

#[cfg(target_arch = "wasm32")]
impl SaveBackend for WebBackend {
    fn persist(&self, slot: &str, text: &str) -> Result<(), SaveError> {
        Self::storage()?.set_item(&self.key(slot)?, text).map_err(|_| SaveError::Storage)
    }

    fn load(&self, slot: &str) -> Result<Option<String>, SaveError> {
        Self::storage()?.get_item(&self.key(slot)?).map_err(|_| SaveError::Storage)
    }

    fn delete(&self, slot: &str) -> Result<(), SaveError> {
        Self::storage()?.remove_item(&self.key(slot)?).map_err(|_| SaveError::Storage)
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
    /// The platform's default for the game called `name`: files in
    /// `saves/<name>` beside the executable, created on the first save, or
    /// browser storage under `name` on wasm.
    ///
    /// A directory of the game's own, because games share a directory as
    /// readily as a browser origin: every example builds into one
    /// `target/debug`, and two games writing one `settings` slot, each with
    /// only its own keys, wipe each other's choices. Under `saves` rather
    /// than at `<name>` itself, which on the desktop is usually the
    /// executable.
    ///
    /// The first time the game runs with its directory, the run, settings
    /// and records an earlier build kept directly beside the executable
    /// are moved into it, so a player who updates the game keeps them.
    /// Where several games shared that directory, the shared settings and
    /// records go to whichever of them runs first; they were a mix of
    /// every game's already.
    ///
    /// # Panics
    ///
    /// When `name` is not a plain name, ASCII letters, digits, `-` and
    /// `_`: it names a directory, so a separator or `..` would put the
    /// saves somewhere else, and it is a constant of the game's, so it is
    /// the game's setup that is wrong.
    pub fn platform_default(name: &str) -> Self {
        if let Err(e) = check_slot(name) {
            panic!("a game's saves are named by a plain name: {e}");
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self(Arc::new(WebBackend::new(name)))
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let beside = FileBackend::dir_beside_executable();
            let dir = game_dir(&beside, name);
            if let Err(e) = adopt_saves_left_beside(&beside, &dir, &[name, crate::settings::SETTINGS_SLOT, crate::RECORDS_SLOT]) {
                bevy::log::warn!(
                    "the saves an earlier build left in {} could not all be moved into {}, so what was not stays there: {e}",
                    beside.display(),
                    dir.display()
                );
            }
            Self(Arc::new(FileBackend::new(dir)))
        }
    }

    /// Saves through `backend`.
    pub fn new(backend: impl SaveBackend + Send + Sync + 'static) -> Self {
        Self(Arc::new(backend))
    }
}

/// The directory the game called `name` saves in when its executable is in
/// `beside`.
#[cfg(not(target_arch = "wasm32"))]
fn game_dir(beside: &std::path::Path, name: &str) -> std::path::PathBuf {
    beside.join("saves").join(name)
}

/// Moves each of `slots` that an earlier build saved directly in `beside`
/// into `dir`, when `dir` does not exist yet, and nothing otherwise.
///
/// Only once: making `dir` is the claim, so of two launches at once one
/// moves the saves, and a launch after the game has saved in its own
/// directory never moves an older save over a newer one. Nothing is made
/// when nothing is left beside, so a game that never saves leaves no
/// directory. A slot that fails to move stays where it was and the rest
/// still move; the first failure is returned.
#[cfg(not(target_arch = "wasm32"))]
fn adopt_saves_left_beside(beside: &std::path::Path, dir: &std::path::Path, slots: &[&str]) -> Result<(), SaveError> {
    let (old, new) = (FileBackend::new(beside), FileBackend::new(dir));
    let left: Vec<&str> = slots.iter().copied().filter(|slot| old.exists(slot)).collect();
    if left.is_empty() {
        return Ok(());
    }
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).map_err(SaveError::Io)?;
    }
    match std::fs::create_dir(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Ok(()),
        Err(e) => return Err(SaveError::Io(e)),
    }
    let mut failed = None;
    for slot in left {
        match std::fs::rename(old.path(slot)?, new.path(slot)?) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => failed = failed.or(Some(SaveError::Io(e))),
        }
    }
    failed.map_or(Ok(()), Err)
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

    /// Two games built into one directory, as every example is in
    /// `target/debug`, each keep their own settings: a game writing its
    /// settings back with only its own keys must not wipe another's.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn two_games_beside_one_executable_keep_their_settings_apart() {
        let (a, b) = (format!("rl-save-game-a-{}", std::process::id()), format!("rl-save-game-b-{}", std::process::id()));
        let (first, second) = (Saves::platform_default(&a), Saves::platform_default(&b));
        first.persist(crate::settings::SETTINGS_SLOT, "(first: 1)").unwrap();
        let seen = second.load(crate::settings::SETTINGS_SLOT).unwrap();
        first.delete(crate::settings::SETTINGS_SLOT).unwrap();
        for name in [&a, &b] {
            let _ = std::fs::remove_dir(game_dir(&FileBackend::dir_beside_executable(), name));
        }
        assert_eq!(seen, None, "the second game never sees the first's settings");
    }

    /// A game is usually named as its executable is, and its directory
    /// must not be the executable's own path.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn a_game_named_like_its_executable_saves() {
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_stem().and_then(|n| n.to_str()).unwrap().to_string();
        assert!(check_slot(&name).is_ok(), "the test binary's name, {name:?}, is a plain name");
        let saves = Saves::platform_default(&name);
        let slot = format!("named-like-the-executable-{}", std::process::id());
        saves.persist(&slot, "x").expect("a game named like its executable saves");
        assert_eq!(saves.load(&slot).unwrap().as_deref(), Some("x"));
        saves.delete(&slot).unwrap();
        let _ = std::fs::remove_dir(game_dir(&FileBackend::dir_beside_executable(), &name));
    }

    /// A player who updates a game keeps what an earlier build saved
    /// beside the executable: the game's run, settings and records move
    /// into its own directory the first time it runs with one, another
    /// game's run stays, and once the directory is there nothing older is
    /// moved over what the game has saved since.
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn saves_an_earlier_build_left_beside_the_executable_move_into_the_games_directory_once() {
        let beside = std::env::temp_dir().join(format!("rl-save-adopt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&beside);
        let old = FileBackend::new(&beside);
        let ours = [("game", "a run"), (crate::settings::SETTINGS_SLOT, "settings"), (crate::RECORDS_SLOT, "records")];
        for (slot, text) in ours.into_iter().chain([("other", "another game's run")]) {
            old.persist(slot, text).unwrap();
        }
        let slots = ours.map(|(slot, _)| slot);
        let fresh = game_dir(&beside, "fresh");
        adopt_saves_left_beside(&beside, &fresh, &["fresh"]).unwrap();
        assert!(!fresh.exists(), "a game with nothing left beside gets no directory until it saves");

        let dir = game_dir(&beside, "game");
        adopt_saves_left_beside(&beside, &dir, &slots).unwrap();
        let new = FileBackend::new(&dir);
        for (slot, text) in ours {
            assert_eq!(new.load(slot).unwrap().as_deref(), Some(text), "{slot} moved into the game's directory");
            assert!(!old.exists(slot), "and is no longer beside the executable");
        }
        assert!(old.exists("other") && !new.exists("other"), "another game's run stays where it was");

        new.persist(crate::settings::SETTINGS_SLOT, "newer").unwrap();
        old.persist(crate::settings::SETTINGS_SLOT, "older").unwrap();
        adopt_saves_left_beside(&beside, &dir, &slots).unwrap();
        assert_eq!(new.load(crate::settings::SETTINGS_SLOT).unwrap().as_deref(), Some("newer"), "nothing is moved a second time");
        let _ = std::fs::remove_dir_all(&beside);
    }

    /// A slot is a plain name, never a path: one that would climb out of
    /// the save directory, or name a file anywhere else, is refused by
    /// every operation and writes nothing.
    #[test]
    fn a_slot_that_is_not_a_plain_name_is_refused_and_nothing_escapes_the_save_directory() {
        let root = std::env::temp_dir().join(format!("rl-save-escape-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let b = FileBackend::new(root.join("saves"));
        let outside = root.join("escape.save.ron");
        for slot in ["../escape", "", "a/b", "a.b", "/tmp/escape", "\\escape", " "] {
            assert!(matches!(b.persist(slot, "x"), Err(SaveError::Slot(_))), "{slot:?} is refused for writing");
            assert!(matches!(b.load(slot), Err(SaveError::Slot(_))), "{slot:?} is refused for reading");
            assert!(matches!(b.delete(slot), Err(SaveError::Slot(_))), "{slot:?} is refused for deleting");
            assert!(!b.exists(slot), "{slot:?} holds nothing");
        }
        assert!(!outside.exists(), "nothing was written beside the save directory");
        assert!(matches!(MemoryBackend::default().persist("../escape", "x"), Err(SaveError::Slot(_))), "memory refuses what a file would");
        b.persist("run-2_b", "x").expect("letters, digits, a dash and an underscore are a slot");
        let _ = std::fs::remove_dir_all(&root);
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
