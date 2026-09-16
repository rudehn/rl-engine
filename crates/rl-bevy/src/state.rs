//! The one state the engine gates on, and how a run ends and begins again.
//!
//! A run is the time between a [`NewRun`](crate::plugin::NewRun) and a
//! [`RunOver`]. The game says when it is over, by writing the message with
//! its outcome and its own words for it, and combat says so for it on the
//! player's death unless the game's [`CombatRules`](crate::combat::CombatRules)
//! say a death is not the end. The engine then leaves
//! [`EngineState::Playing`] for [`EngineState::Over`], where the world is
//! still drawn and nothing else runs, and keeps the [`Ending`] for whatever
//! screen or file wants to say what happened. A [`Restart`] tears the run
//! down and runs the game's start again, on a fresh seed or the same one.

use bevy::prelude::*;
use rl_core::RunSeed;

/// Whether the engine's gameplay loops run.
///
/// The turns, the input, the streaming, the light and the sight run only
/// while [`Playing`](Self::Playing). Drawing runs while playing and while
/// [`Over`](Self::Over), so the last frame of a run stays on screen under
/// whatever the game or the engine's menu draws over it. The game owns any
/// finer state machine and drives this one from it.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EngineState {
    /// No world; nothing runs.
    #[default]
    Idle,
    /// A world is loaded and turns are being dealt.
    Playing,
    /// The run has ended. The world is drawn and nothing in it moves.
    Over,
}

impl EngineState {
    /// Whether there is a world worth drawing.
    pub const fn shows_world(self) -> bool {
        matches!(self, EngineState::Playing | EngineState::Over)
    }
}

/// A run condition: true while a world is drawn, playing or over.
pub fn world_is_shown(state: Res<State<EngineState>>) -> bool {
    state.get().shows_world()
}

/// How a run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The player died, at the hands of `by` when something gets the credit.
    Died {
        /// Who gets the credit, if anyone does.
        by: Option<Entity>,
    },
    /// The game's victory condition was met.
    Won,
    /// The player gave the run up.
    Abandoned,
}

/// The run is over.
///
/// Written by the game when its own condition is met, and by combat on the
/// player's death when the rules say a death ends the run. `epitaph` is
/// the game's own line about it, in its own words: "The whale keeps you."
/// Empty leaves the words to whatever draws the ending.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct RunOver {
    /// How.
    pub outcome: Outcome,
    /// What the game says about it, or nothing.
    pub epitaph: String,
}

impl RunOver {
    /// The player died.
    pub fn died(by: Option<Entity>) -> Self {
        Self { outcome: Outcome::Died { by }, epitaph: String::new() }
    }

    /// The run was won.
    pub fn won() -> Self {
        Self { outcome: Outcome::Won, epitaph: String::new() }
    }

    /// With the game's own words.
    pub fn saying(mut self, epitaph: impl Into<String>) -> Self {
        self.epitaph = epitaph.into();
        self
    }
}

/// What the run that just ended came to. Present from the frame the run
/// ended until the next one begins.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct Ending {
    /// How it ended.
    pub outcome: Outcome,
    /// The game's words, possibly none.
    pub epitaph: String,
    /// The run's seed.
    pub seed: RunSeed,
    /// The whole turn it ended on.
    pub turn: u32,
}

/// Begin a new run: tear this one down and run the game's start again.
///
/// `seed` is the next run's, or `None` for a fresh one. Resolved at the end
/// of the frame it is written in; the new run's first turn is dealt two
/// frames later, after the state has gone through
/// [`EngineState::Idle`] so everything that watches for play beginning
/// sees it begin again.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Restart {
    /// The seed to start on, or a fresh one.
    pub seed: Option<RunSeed>,
}

impl Restart {
    /// A new run on a fresh seed.
    pub fn fresh() -> Self {
        Self { seed: None }
    }

    /// The same run again.
    pub fn on(seed: RunSeed) -> Self {
        Self { seed: Some(seed) }
    }
}

/// Ends the run for the first [`RunOver`] of the frame: keeps the
/// [`Ending`] and leaves [`EngineState::Playing`].
pub fn end_runs(
    mut overs: MessageReader<RunOver>,
    seed: Option<Res<crate::seed::Seed>>,
    turns: Res<crate::turn::Turns>,
    state: Res<State<EngineState>>,
    mut next: ResMut<NextState<EngineState>>,
    mut commands: Commands,
) {
    let Some(over) = overs.read().next() else { return };
    // A second ending in one frame, or one after the run is already over,
    // changes nothing: the first word is the last.
    if *state.get() != EngineState::Playing {
        return;
    }
    let seed = seed.map(|s| s.0).unwrap_or(RunSeed(0));
    commands.insert_resource(Ending { outcome: over.outcome, epitaph: over.epitaph.clone(), seed, turn: turns.turn_number() });
    next.set(EngineState::Over);
}
