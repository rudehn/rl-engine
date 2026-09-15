//! A run written down as its seed, its arguments and every key, so it can
//! be played back.
//!
//! "Sometimes it gets stuck" is a report nobody can act on; the same
//! report with a file attached is a bug with a reproduction. The turn
//! loop, the minds and every roll are deterministic given the seed, so a
//! run is its seed plus the player's keys, and that is all a
//! [`Recording`] holds. It is written as RON, small enough to paste.
//!
//! The keys are the ones the game reads, `ButtonInput<KeyCode>` after the
//! engine's held-key repeat has been turned into presses, and each is
//! stamped with the turn clock it was read at. The clock is what keeps a
//! replay in step: a key is played when the clock reads what it read when
//! the key was pressed, one entry per frame, so nothing in the file
//! depends on how fast the frames came, and a replay that has drifted
//! from the recording stops and says at which key, rather than pressing
//! on into a world that is not the one recorded.
//!
//! The recorder and the player are `ReplayPlugin` in `rl-ui`, which knows
//! the direction keys and the repeat. What lives here is the file, and
//! what a game reads before it has an app: [`args`] and [`seed`], which
//! answer from the recording named by [`REPLAY_VAR`] when there is one
//! and from the command line otherwise, so a game that reads its
//! arguments through them replays with the arguments it was recorded
//! with.

use bevy::prelude::*;
use rl_core::RunSeed;
use serde::{Deserialize, Serialize};

/// The environment variable naming the file to write the run to.
pub const RECORD_VAR: &str = "RL_RECORD";
/// The environment variable naming the file to play the run from.
pub const REPLAY_VAR: &str = "RL_REPLAY";

/// One frame's keys, and the turn clock they were read at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pressed {
    /// [`Turns::now`](crate::turn::Turns) when the keys were read.
    pub clock: u32,
    /// The keys that went down that frame.
    pub keys: Vec<KeyCode>,
    /// Whether Shift was down with them.
    #[serde(default)]
    pub shift: bool,
}

/// A run: what it was seeded with, what it was run with, and every key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Recording {
    /// The run's seed.
    pub seed: u64,
    /// The command line it was run with, the program's name left off.
    pub args: Vec<String>,
    /// Every frame that pressed something, in order.
    pub keys: Vec<Pressed>,
}

impl Recording {
    /// Reads a recording from `path`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        ron::from_str(&text).map_err(|e| format!("{} is not a recording: {e}", path.display()))
    }

    /// Writes the recording to `path`, whole, so a run cut short by a
    /// crash or a kill has everything up to its last key on disk.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), String> {
        let path = path.as_ref();
        let text = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).map_err(|e| format!("cannot write a recording: {e}"))?;
        std::fs::write(path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }
}

/// The recording [`REPLAY_VAR`] names, if it names one.
///
/// # Panics
/// Panics when the variable is set and the file cannot be read, since a
/// replay that quietly started a fresh run is a replay that never
/// happened.
#[cfg(not(target_arch = "wasm32"))]
pub fn replaying() -> Option<Recording> {
    let path = std::env::var_os(REPLAY_VAR)?;
    Some(Recording::load(&path).unwrap_or_else(|e| panic!("{REPLAY_VAR}: {e}")))
}

/// Nothing to replay from: a browser has no environment.
#[cfg(target_arch = "wasm32")]
pub fn replaying() -> Option<Recording> {
    None
}

/// The arguments this run was started with: the recording's when
/// replaying, the command line's otherwise, the program's name left off.
/// What a game reads its own flags from, so a replay runs with the flags
/// it was recorded under.
pub fn args() -> Vec<String> {
    match replaying() {
        Some(recording) => recording.args,
        None => own_args(),
    }
}

/// The seed the recording was made with, when replaying.
pub fn seed() -> Option<RunSeed> {
    replaying().map(|r| RunSeed(r.seed))
}

/// The command line, the program's name left off.
#[cfg(not(target_arch = "wasm32"))]
pub fn own_args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

/// A browser has no command line.
#[cfg(target_arch = "wasm32")]
pub fn own_args() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_round_trips_through_its_file() {
        let recording = Recording {
            seed: 7,
            args: vec!["--seed".into(), "7".into(), "--floor".into(), "2".into()],
            keys: vec![
                Pressed { clock: 0, keys: vec![KeyCode::Digit1], shift: false },
                Pressed { clock: 0, keys: vec![KeyCode::Enter], shift: false },
                Pressed { clock: 100, keys: vec![KeyCode::KeyH], shift: false },
            ],
        };
        let dir = std::env::temp_dir().join("rl-recording-round-trip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.ron");
        recording.save(&path).unwrap();
        assert_eq!(Recording::load(&path).unwrap(), recording);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
