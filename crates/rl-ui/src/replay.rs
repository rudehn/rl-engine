//! Recording a run's keys, and playing a recorded run back.
//!
//! The file is [`Recording`], in `rl-bevy`; this is the plugin that writes
//! it and reads it. Set [`RECORD_VAR`] to a path and every frame that
//! presses something is written down with the turn clock it was read at;
//! set [`REPLAY_VAR`](rl_bevy::replay::REPLAY_VAR) to a recording and its keys are pressed again, each
//! when the clock reads what it read then, one frame apiece, and the
//! keyboard is the player's again once the last has been pressed. Both at
//! once records the replay and whatever the player does after it.
//!
//! What is recorded is what the game reads: the keys that went down, with
//! Shift, after the engine's held-key repeat has been turned into presses,
//! so a walk that was a held key replays as the steps it was. A replay
//! never presses while the turns are held for something to be seen, since
//! a key then would skip it, and what it skipped decides which clock the
//! key is read at.
//!
//! A replay that reaches a key whose clock the game has already passed has
//! drifted: something rolled or decided differently than it did when
//! recorded. It stops there and says which key, since pressing on would
//! only bury where the difference began.

use std::collections::VecDeque;
use std::path::PathBuf;

use bevy::input::InputSystems;
use bevy::prelude::*;
use rl_bevy::prelude::{MyTurn, Player};
use rl_bevy::replay::{Pressed, RECORD_VAR, Recording};
use rl_bevy::{EngineSet, Seed, TurnHold, Turns};

use crate::controls::Repeats;
use crate::cursor::shifted;
use crate::keys::DirectionKeys;

/// Records to [`RECORD_VAR`] and plays [`REPLAY_VAR`](rl_bevy::replay::REPLAY_VAR), when either is set.
///
/// [`RoguelikePlugins`](https://docs.rs/rl-engine) adds one reading the
/// environment; a test names its file and its recording outright.
#[derive(Default, Debug, Clone)]
pub struct ReplayPlugin {
    record_to: Option<PathBuf>,
    play: Option<Recording>,
    from_env: bool,
}

impl ReplayPlugin {
    /// Reads [`RECORD_VAR`] and [`REPLAY_VAR`](rl_bevy::replay::REPLAY_VAR) when the app is built.
    pub fn from_env() -> Self {
        Self { from_env: true, ..Self::default() }
    }

    /// Writes the run to `path` as it is played.
    pub fn record_to(path: impl Into<PathBuf>) -> Self {
        Self { record_to: Some(path.into()), ..Self::default() }
    }

    /// Plays `recording` from the first frame of play.
    pub fn play(recording: Recording) -> Self {
        Self { play: Some(recording), ..Self::default() }
    }
}

impl Plugin for ReplayPlugin {
    fn build(&self, app: &mut App) {
        let (mut record_to, mut play) = (self.record_to.clone(), self.play.clone());
        if self.from_env {
            record_to = record_to.or_else(|| std::env::var_os(RECORD_VAR).map(PathBuf::from));
            play = play.or_else(rl_bevy::replay::replaying);
        }
        if let Some(path) = record_to {
            app.insert_resource(Recorder { path, recording: Recording { seed: 0, args: rl_bevy::replay::args(), keys: Vec::new() } })
                .add_systems(Update, record.in_set(EngineSet::Input));
        }
        if let Some(recording) = play {
            info!("replaying {} keys from seed {}", recording.keys.len(), recording.seed);
            app.insert_resource(Replay { keys: recording.keys.into(), held: Vec::new(), played: 0 }).add_systems(PreUpdate, play_back.after(InputSystems));
        }
    }
}

/// The run so far, and where it is written.
#[derive(Resource, Debug)]
pub struct Recorder {
    // Written only where there is a filesystem to write to.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    path: PathBuf,
    recording: Recording,
}

impl Recorder {
    /// What has been recorded so far.
    pub fn recording(&self) -> &Recording {
        &self.recording
    }
}

/// What the recorder reads each frame.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Frame<'w> {
    input: Res<'w, ButtonInput<KeyCode>>,
    repeats: Res<'w, Repeats>,
    directions: Res<'w, DirectionKeys>,
    turns: Res<'w, Turns>,
    seed: Option<Res<'w, Seed>>,
}

/// Writes down this frame's keys, if any, at the turn clock the game
/// reads them at, and the file with them.
pub fn record(frame: Frame, mut recorder: ResMut<Recorder>) {
    let mut keys: Vec<KeyCode> = frame.input.get_just_pressed().copied().collect();
    let mut shift = shifted(&frame.input);
    // A repeat is a press the game never saw as one: written as the key
    // it stands for, so it replays as a press.
    if let Some((direction, with_shift)) = frame.repeats.firing_any()
        && let Some((key, _)) = frame.directions.0.iter().find(|(_, d)| *d == direction)
    {
        keys.push(*key);
        shift |= with_shift;
    }
    if keys.is_empty() {
        return;
    }
    recorder.recording.seed = frame.seed.as_deref().map(|s| s.0.0).unwrap_or(0);
    recorder.recording.keys.push(Pressed { clock: frame.turns.now(), keys, shift });
    // A browser has no filesystem to write the recording to, and
    // `Recording::save` is not compiled there.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = recorder.path.clone();
        if let Err(e) = recorder.recording.save(&path) {
            error!("{RECORD_VAR}: {e}");
        }
    }
}

/// The keys still to press, and the ones down this frame.
#[derive(Resource, Debug)]
pub struct Replay {
    keys: VecDeque<Pressed>,
    held: Vec<KeyCode>,
    played: usize,
}

impl Replay {
    /// Whether every key has been pressed.
    pub fn is_done(&self) -> bool {
        self.keys.is_empty()
    }

    /// How many have been pressed.
    pub fn played(&self) -> usize {
        self.played
    }
}

/// Presses the next recorded keys when the clock reads what it read then,
/// lets go of the last frame's, and stops with a report when the game has
/// gone past a recorded clock.
///
/// Only while the player holds a turn: that is when a key was read, and a
/// frame the world is still being built in, a monster's pass, or a hold
/// on the turns is a frame the key would fall on the floor.
pub fn play_back(
    mut replay: ResMut<Replay>,
    mut input: ResMut<ButtonInput<KeyCode>>,
    turns: Res<Turns>,
    hold: Option<Res<TurnHold>>,
    player: Query<(), (With<Player>, With<MyTurn>)>,
) {
    for key in std::mem::take(&mut replay.held) {
        input.release(key);
    }
    let Some(next) = replay.keys.front() else { return };
    if player.is_empty() || hold.as_deref().is_some_and(TurnHold::is_held) {
        return;
    }
    let now = turns.now();
    if now < next.clock {
        return;
    }
    if now > next.clock {
        error!("replay drifted: key {} was recorded at clock {}, but the game is at {}. The keyboard is yours.", replay.played + 1, next.clock, now);
        replay.keys.clear();
        return;
    }
    let Some(next) = replay.keys.pop_front() else { return };
    let mut down = next.keys;
    if next.shift && !down.iter().any(|k| matches!(k, KeyCode::ShiftLeft | KeyCode::ShiftRight)) {
        down.push(KeyCode::ShiftLeft);
    }
    for key in &down {
        input.press(*key);
    }
    replay.held = down;
    replay.played += 1;
    if replay.keys.is_empty() {
        info!("replay done: {} keys played. The keyboard is yours.", replay.played);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::prelude::*;
    use rl_core::Direction;

    /// The smallest game: `h` steps west, `.` waits.
    fn walk(
        keys: Res<ButtonInput<KeyCode>>,
        player: Query<Entity, (With<Player>, With<MyTurn>)>,
        mut steps: MessageWriter<Intent<Step>>,
        mut waits: MessageWriter<Intent<Wait>>,
    ) {
        let Ok(me) = player.single() else { return };
        if keys.just_pressed(KeyCode::KeyH) {
            steps.write(Intent::new(me, Step(Direction::West)));
        } else if keys.just_pressed(KeyCode::Period) {
            waits.write(Intent::new(me, Wait));
        }
    }

    fn game(plugin: ReplayPlugin) -> Stage {
        Stage::new_with(plugin, |app| {
            app.add_systems(Update, walk.in_set(EngineSet::Input));
        })
    }

    /// A run recorded and played back ends where it ended, at the clock it
    /// ended at, with every key stamped by the clock it was read at.
    #[test]
    fn a_recorded_run_plays_back_to_the_same_place_and_clock() {
        let dir = std::env::temp_dir().join("rl-replay-round-trip");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.ron");

        let mut recorded = game(ReplayPlugin::record_to(&path));
        for key in [KeyCode::KeyH, KeyCode::KeyH, KeyCode::Period, KeyCode::KeyH] {
            recorded.press(key);
        }
        let ended = (recorded.app.world().get::<Position>(recorded.player).unwrap().0, recorded.app.world().resource::<Turns>().now());
        assert_eq!(ended.0, recorded.at.offset(-3, 0), "three steps west");
        let recording = Recording::load(&path).unwrap();
        assert_eq!(recording.keys.iter().map(|p| p.clock).collect::<Vec<_>>(), vec![0, 100, 200, 300], "each key at the clock it was read at");

        let mut replayed = game(ReplayPlugin::play(recording));
        for _ in 0..12 {
            replayed.tick();
        }
        assert!(replayed.app.world().resource::<Replay>().is_done());
        assert_eq!((replayed.app.world().get::<Position>(replayed.player).unwrap().0, replayed.app.world().resource::<Turns>().now()), ended);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A recording whose clocks the game has passed is a run that drifted,
    /// and the replay stops rather than pressing on.
    #[test]
    fn a_replay_that_has_drifted_stops_and_gives_the_keyboard_back() {
        let recording = Recording {
            seed: 0,
            args: vec![],
            keys: vec![Pressed { clock: 0, keys: vec![KeyCode::KeyH], shift: false }, Pressed { clock: 50, keys: vec![KeyCode::KeyH], shift: false }],
        };
        let mut stage = game(ReplayPlugin::play(recording));
        for _ in 0..6 {
            stage.tick();
        }
        let replay = stage.app.world().resource::<Replay>();
        assert!(replay.is_done() && replay.played() == 1, "the first key went, the second was never at a clock the game reached");
        assert_eq!(stage.app.world().get::<Position>(stage.player).unwrap().0, stage.at.offset(-1, 0));
    }
}
