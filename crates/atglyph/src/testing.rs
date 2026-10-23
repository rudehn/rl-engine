//! A game's tests, without a window: one module for all of it.
//!
//! Every game's tests stood the same app up, ran it the same way and read
//! it back the same way, and every game wrote its own copy. The copies are
//! here: the headless game itself, [`windowless`], [`headless`] and
//! [`continued`], which need a save backend and so live in the facade, the
//! one crate that knows every other; the helpers that drive it, put a
//! thing in a hand or a player on a cell and read the world back; the
//! readers of the screen and the log; the seed ranges a generation test
//! plays; and [`ambiguity`], the check that a game's systems touching the
//! same data are ordered.
//!
//! These names are the engine's API, stable as any other: each changes
//! only with a version, and its changelog says how. They are listed one by
//! one rather than re-exported whole, so nothing reaches a game's tests by
//! accident. `atglyph_bevy::testing` and `atglyph_ui::testing`, where most
//! of them are written, are the engine crates' shared fixtures, a ground to
//! stand on and two sides at war among them: hidden, and outside semver.
//! A game builds its own ground on a run of its own.
//!
//! None of it names a game's content. A helper a game's content shapes,
//! one that makes a thing by the game's own name or kills with the game's
//! own kind of harm, stays in the game, built on these.

pub use atglyph_bevy::testing::ambiguity;
pub use atglyph_bevy::testing::{
    KeyScript, KeyScriptPlugin, QUICK, at, equip, first_seeds, give, go_through, here, kill, lay, lay_tile, let_time_pass, name_of, pack, par_seeds,
    pass_turns, player, press, put_down, seeds, settle, stand_on, take_up, unequip, wall_off,
};
pub use atglyph_ui::testing::{row, said, written};

use atglyph_core::RunSeed;
use atglyph_save::{MemoryBackend, SaveBackend, Saves};
use bevy::prelude::*;

/// The engine with no window: the headless app, sight, the run's seed and
/// saves kept in memory, so a test never touches the disk and each starts
/// with an empty slot of its own.
///
/// What a game adds next is what its `main` adds after the window's own
/// plugins, in the same order, so a test runs the systems the player does
/// in the order the player's build added them.
pub fn windowless(seed: RunSeed) -> App {
    let mut app = atglyph_bevy::testing::headless_app();
    app.add_plugins(atglyph_bevy::FovPlugin);
    app.insert_resource(atglyph_bevy::Seed(seed)).insert_resource(Saves::new(MemoryBackend::default()));
    app
}

/// A headless game: [`windowless`], the UI base with the game's
/// `narrator`, and then `game`, the game's own plugin. The run begins on
/// the first frames, as it does for a player who picked New Game.
pub fn headless<N, G>(seed: RunSeed, narrator: impl bevy::app::Plugins<N>, game: impl bevy::app::Plugins<G>) -> App {
    let mut app = windowless(seed);
    app.add_plugins((atglyph_ui::UiPlugin, narrator));
    app.add_plugins(game);
    app
}

/// `app` with the run in `text` continued: the save put in `slot`,
/// [`ContinueRun`](atglyph_bevy::ContinueRun) asked for as a title's Continue
/// asks for it, and the first frames run so the continued run stands.
pub fn continued(mut app: App, slot: &str, text: &str) -> App {
    app.world().resource::<Saves>().persist(slot, text).expect("the memory backend takes it");
    app.insert_resource(atglyph_bevy::ContinueRun);
    settle(&mut app);
    settle(&mut app);
    app
}
