//! Where a game's saves are kept when it inserts none of its own, asked of
//! the environment as a game's run asks it.
//!
//! Its own test binary, and the one test in it, because it sets the
//! variable a replay is named by: the environment is the whole process's,
//! and a test beside it reading the variable while this one held it set
//! would see a run it never asked for.

use bevy::prelude::*;
use rl_bevy::replay::REPLAY_VAR;
use rl_save::{SaveBackend, SavePlugin, Saves};

/// A game no game is called, by this process, so a run of the tests
/// writing its slot beside the test binary touches nothing a game reads.
fn name() -> String {
    format!("rl-save-scripted-{}", std::process::id())
}

/// What a fresh app saving the game called `name` wrote `text` to its run
/// slot through.
fn saved_by_a_run(name: &str, text: &str) {
    let mut app = rl_bevy::plugin::headless_app();
    app.add_plugins(SavePlugin::new(name));
    app.world().resource::<Saves>().persist(name, text).expect("the backend takes it");
}

/// A run played from a script, a replay, a recording or a capture, saves
/// in memory and leaves the player's save as it was; a run the player
/// plays saves where the player's saves are kept, so the next run
/// continues it.
#[test]
fn a_scripted_run_saves_in_memory_and_a_played_one_where_the_players_saves_are_kept() {
    assert!(!rl_bevy::replay::scripted(), "nothing in the test's environment names a script");
    let name = name();
    let platform = Saves::platform_default(&name);
    saved_by_a_run(&name, "a played run");
    assert_eq!(platform.load(&name).unwrap().as_deref(), Some("a played run"), "the played run is where the player's saves are kept");
    platform.delete(&name).unwrap();

    // SAFETY: the one test in this binary, so no other thread reads the
    // environment while it is written.
    unsafe { std::env::set_var(REPLAY_VAR, "run.ron") };
    let scripted = rl_bevy::replay::scripted();
    saved_by_a_run(&name, "a scripted run");
    // SAFETY: as above.
    unsafe { std::env::remove_var(REPLAY_VAR) };
    assert!(scripted, "a replay is a scripted run");
    assert_eq!(platform.load(&name).unwrap(), None, "and the player's slot never saw it");
}
