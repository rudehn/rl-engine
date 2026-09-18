//! Opens Foundry's window and loads its content; the crate's own docs,
//! and everything else, live in `lib.rs`.

use bevy::prelude::*;
use rl_engine::RoguelikePlugins;
use rl_engine::rl_bevy::plugin::{NewRun, Turn, TurnSet};

/// Columns and rows the terminal window opens with. The screen split
/// itself is a later task's, once there is something to draw in it.
const COLS: i32 = 100;
const ROWS: i32 = 40;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("Foundry", COLS, ROWS)).insert_resource(foundry::content::registries()).add_systems(NewRun, foundry::run::start);
    app.add_systems(Turn, foundry::gear::grant_dark_sight.in_set(TurnSet::React));
    app.run()
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::prelude::WorldMap;
    use rl_engine::rl_core::RunSeed;

    #[test]
    fn a_new_run_puts_the_commando_on_deck_one() {
        let mut app = foundry::testing::headless(RunSeed(3));
        for _ in 0..5 {
            app.update();
        }
        let map = app.world().resource::<WorldMap>().current();
        assert_eq!(foundry::decks::deck_of(map), 1);
    }
}
