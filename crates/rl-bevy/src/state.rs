//! The one state the engine gates on.

use bevy::prelude::*;

/// Whether the engine's gameplay loops run.
///
/// Every engine system is gated on [`EngineState::Playing`], so nothing
/// ticks in a menu and nothing crashes on a world that does not exist yet.
/// The game owns any finer state machine and drives this one from it.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EngineState {
    /// No world; nothing runs.
    #[default]
    Idle,
    /// A world is loaded and turns are being dealt.
    Playing,
}
