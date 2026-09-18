//! Foundry: a commando's fight down through the decks of a droid foundry,
//! alone against whatever the floor below is still assembling.
//!
//! This opens the window and loads Foundry's content; the run itself,
//! its decks and its systems, is later tasks'.

mod content;
#[cfg(test)]
mod testing;

use bevy::prelude::*;
use rl_engine::RoguelikePlugins;

/// Columns and rows the terminal window opens with. The screen split
/// itself is a later task's, once there is something to draw in it.
const COLS: i32 = 100;
const ROWS: i32 = 40;

fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("Foundry", COLS, ROWS)).insert_resource(content::registries());
    app.run()
}
