//! Reading a game's screen and log back in a test.
//!
//! A game's own tests build its real screen into a terminal no window
//! shows and assert on the text, and every game wrote the same two readers
//! for it and the same one for the log. These are the one copy, as
//! functions on an [`App`], so a game's test and the engine's panel tests
//! read a screen the same way.
//!
//! What builds the app is [`atglyph_bevy::testing`]'s, and the headless game is
//! the facade's, since it needs a save backend this crate does not know.

use atglyph_render::Terminal;
use bevy::prelude::*;

use crate::log::MessageLog;

/// Row `y` of the screen as text, every column of it, blanks and all, so a
/// test can cut a panel out of it by column.
pub fn row(app: &App, y: i32) -> String {
    let t = app.world().resource::<Terminal>();
    (0..t.width()).map(|x| t.get(x, y).map_or(' ', |c| c.glyph)).collect()
}

/// Every row of the screen with something on it, trimmed, top to bottom.
pub fn written(app: &App) -> Vec<String> {
    let height = app.world().resource::<Terminal>().height();
    (0..height).map(|y| row(app, y).trim().to_string()).filter(|r| !r.is_empty()).collect()
}

/// Every line of the log, oldest first.
pub fn said(app: &App) -> Vec<String> {
    app.world().resource::<MessageLog>().iter().map(|line| line.text.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_reads_back_row_by_row_and_the_log_line_by_line() {
        let mut app = App::new();
        let mut terminal = Terminal::new(6, 3, Vec2::ONE);
        terminal.print(1, 1, "hi", Color::WHITE);
        app.insert_resource(terminal).init_resource::<MessageLog>();
        app.world_mut().resource_mut::<MessageLog>().push_at("A line.", crate::tone::Tones::TEXT, 1);
        assert_eq!(row(&app, 1), " hi   ");
        assert_eq!(written(&app), vec!["hi".to_string()]);
        assert_eq!(said(&app), vec!["A line.".to_string()]);
    }
}
