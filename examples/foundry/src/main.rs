//! Opens Foundry's window and loads its content; the crate's own docs,
//! and everything else, live in `lib.rs`.

use bevy::prelude::*;
use rl_engine::RoguelikePlugins;

/// Columns and rows the terminal window opens with. The screen split
/// itself is a later task's, once there is something to draw in it.
const COLS: i32 = 100;
const ROWS: i32 = 40;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("Foundry", COLS, ROWS)).insert_resource(foundry::content::registries());
    app.run()
}
