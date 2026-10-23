//! A game's tests, without a window: one module for all of it.
//!
//! Every game's tests stood the same app up, ran it the same way and read
//! it back the same way, and every game wrote its own copy. The copies are
//! here: [`rl_bevy::testing`] for building a world and driving it, with the
//! helpers that put a thing in a hand or a player on a cell, and
//! [`rl_ui::testing`] for reading the screen and the log back, both
//! re-exported, and beside them the headless game itself, which needs a
//! save backend and so lives in the facade, the one crate that knows every
//! other.
//!
//! None of it names a game's content. A helper a game's content shapes,
//! one that makes a thing by the game's own name or kills with the game's
//! own kind of harm, stays in the game, built on these.

pub use rl_bevy::testing::*;
pub use rl_ui::testing::*;

use bevy::prelude::*;
use rl_core::RunSeed;
use rl_save::{MemoryBackend, SaveBackend, Saves};

/// The engine with no window: the headless app, sight, the run's seed and
/// saves kept in memory, so a test never touches the disk and each starts
/// with an empty slot of its own.
///
/// What a game adds next is what its `main` adds after the window's own
/// plugins, in the same order, so a test runs the systems the player does
/// in the order the player's build added them.
pub fn windowless(seed: RunSeed) -> App {
    let mut app = rl_bevy::testing::headless_app();
    app.add_plugins(rl_bevy::FovPlugin);
    app.insert_resource(rl_bevy::Seed(seed)).insert_resource(Saves::new(MemoryBackend::default()));
    app
}

/// A headless game: [`windowless`], the UI base with the game's
/// `narrator`, and then `game`, the game's own plugin. The run begins on
/// the first frames, as it does for a player who picked New Game.
pub fn headless<N, G>(seed: RunSeed, narrator: impl bevy::app::Plugins<N>, game: impl bevy::app::Plugins<G>) -> App {
    let mut app = windowless(seed);
    app.add_plugins((rl_ui::UiPlugin, narrator));
    app.add_plugins(game);
    app
}

/// `app` with the run in `text` continued: the save put in `slot`,
/// [`ContinueRun`](rl_bevy::ContinueRun) asked for as a title's Continue
/// asks for it, and the first frames run so the continued run stands.
pub fn continued(mut app: App, slot: &str, text: &str) -> App {
    app.world().resource::<Saves>().persist(slot, text).expect("the memory backend takes it");
    app.insert_resource(rl_bevy::ContinueRun);
    settle(&mut app);
    settle(&mut app);
    app
}
