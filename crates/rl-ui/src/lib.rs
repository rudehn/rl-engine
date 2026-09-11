//! Chrome: the message log, the status line, a framed list menu, and the
//! palette they share.
//!
//! Drawn onto the same glyph terminal as the map, in rows the game sets
//! aside for them. The log carries a category per entry so colouring is a
//! lookup, not a search through English. The menu is state plus a draw
//! call, so a game's inventory, shop or picker is a few lines of keys.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod log;
pub mod menu;
pub mod theme;

pub use log::{LogCategory, LogEntry, MessageLog};
pub use menu::{ListMenu, MenuRow, draw_frame, draw_menu};
pub use theme::Theme;

/// Where the log and status line are drawn.
#[derive(bevy::prelude::Resource, Debug, Clone, Copy)]
pub struct ChromeLayout {
    /// Terminal rows for the message log, top-most first.
    pub log_rows: rl_core::Rect,
    /// The terminal row for the status line.
    pub status_row: i32,
}

/// Draws the log and status line every frame.
pub struct ChromePlugin;

impl bevy::prelude::Plugin for ChromePlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        use bevy::prelude::*;
        app.init_resource::<MessageLog>()
            .init_resource::<Theme>()
            .init_resource::<StatusLine>()
            .add_systems(Update, draw_chrome.in_set(rl_bevy::EngineSet::Present).after(rl_render::map_view::draw_map));
    }
}

/// The text of the status line, set by the game.
#[derive(bevy::prelude::Resource, Debug, Clone, Default)]
pub struct StatusLine(pub String);

/// Draws the log and status line. Public so a game can order a modal
/// after it.
pub fn draw_chrome(
    mut terminal: bevy::prelude::ResMut<rl_render::Terminal>,
    layout: Option<bevy::prelude::Res<ChromeLayout>>,
    log: bevy::prelude::Res<MessageLog>,
    status: bevy::prelude::Res<StatusLine>,
    theme: bevy::prelude::Res<Theme>,
) {
    let Some(layout) = layout else { return };
    let rows = layout.log_rows;
    // Clear first: a shorter line must not leave the tail of a longer one.
    terminal.fill(rows, rl_render::Cell::new(' ', theme.text).on(theme.panel_bg));
    let mut y = rows.bottom() - 1;
    for entry in log.recent(rows.height as usize) {
        terminal.print_on(rows.x, y, &entry.text, theme.log_color(entry.category), theme.panel_bg);
        y -= 1;
    }
    let width = terminal.width();
    terminal.fill(rl_core::Rect::new(0, layout.status_row, width, 1), rl_render::Cell::new(' ', theme.text).on(theme.panel_bg));
    terminal.print_on(1, layout.status_row, &status.0, theme.text, theme.panel_bg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use rl_render::Terminal;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Terminal::new(40, 6, Vec2::ONE))
            .insert_resource(ChromeLayout { log_rows: rl_core::Rect::new(0, 3, 40, 3), status_row: 0 })
            .init_resource::<MessageLog>()
            .init_resource::<Theme>()
            .init_resource::<StatusLine>()
            .add_systems(Update, draw_chrome);
        app
    }

    fn row(app: &App, y: i32) -> String {
        let t = app.world().resource::<Terminal>();
        (0..40).map(|x| t.get(x, y).unwrap().glyph).collect::<String>().trim_end().to_string()
    }

    #[test]
    fn a_shorter_line_does_not_keep_the_tail_of_a_longer_one() {
        let mut app = app();
        app.world_mut().resource_mut::<MessageLog>().info("a long line that fills the row", 0);
        app.update();
        assert_eq!(row(&app, 5), "a long line that fills the row");
        app.world_mut().resource_mut::<MessageLog>().info("short", 0);
        app.update();
        assert_eq!(row(&app, 5), "short");
        assert_eq!(row(&app, 4), "a long line that fills the row", "the older line moved up intact");
        assert_eq!(row(&app, 3), "");
    }
}
