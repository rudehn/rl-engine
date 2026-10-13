//! Where a run given [`RoguelikePlugins::saves`] saves, asked of the
//! environment as a game's run asks it.
//!
//! Its own test binary, and the one test in it, because it sets the
//! variable a replay is named by: the environment is the whole process's,
//! and a test beside it reading the variable while this one held it set
//! would see a run it never asked for.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::replay::REPLAY_VAR;

/// A slot no game saves in, by this process, so a run of the tests
/// writing it beside the test binary touches nothing a game reads.
fn slot() -> String {
    format!("rl-engine-player-saves-{}", std::process::id())
}

/// What a fresh app with the player's saves of `name` wrote `text` to.
fn saved_by_a_run(name: &str, text: &str) {
    let mut app = App::new();
    app.add_plugins(PlayerSaves::new(name));
    app.world().resource::<Saves>().persist(&slot(), text).expect("the backend takes it");
}

/// A run played from a script saves in memory and leaves the player's
/// save as it was; a run the player plays saves where the player's saves
/// are kept, so the next run continues it.
#[test]
fn a_scripted_run_saves_in_memory_and_a_played_one_where_the_players_saves_are_kept() {
    assert!(!rl_engine::scripted(), "nothing in the test's environment names a script");
    let platform = Saves::platform_default("warren");
    saved_by_a_run("warren", "a played run");
    assert_eq!(platform.load(&slot()).unwrap().as_deref(), Some("a played run"), "the played run is where the player's saves are kept");
    platform.delete(&slot()).unwrap();

    // SAFETY: the one test in this binary, so no other thread reads the
    // environment while it is written.
    unsafe { std::env::set_var(REPLAY_VAR, "run.ron") };
    let scripted = rl_engine::scripted();
    saved_by_a_run("warren", "a scripted run");
    // SAFETY: as above.
    unsafe { std::env::remove_var(REPLAY_VAR) };
    assert!(scripted, "a replay is a scripted run");
    assert_eq!(platform.load(&slot()).unwrap(), None, "and the player's slot never saw it");
}
