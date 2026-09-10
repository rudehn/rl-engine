//! Chrome: the message log, the status line, and the palette they share.
//!
//! Drawn onto the same glyph terminal as the map, in rows the game sets
//! aside for them. The log carries a category per entry so colouring is a
//! lookup, not a search through English.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod log;
pub mod theme;

pub use log::{LogCategory, LogEntry, MessageLog};
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

fn draw_chrome(
    mut terminal: bevy::prelude::ResMut<rl_render::Terminal>,
    layout: Option<bevy::prelude::Res<ChromeLayout>>,
    log: bevy::prelude::Res<MessageLog>,
    status: bevy::prelude::Res<StatusLine>,
    theme: bevy::prelude::Res<Theme>,
) {
    let Some(layout) = layout else { return };
    let rows = layout.log_rows;
    let mut y = rows.bottom() - 1;
    for entry in log.recent(rows.height as usize) {
        terminal.print_on(rows.x, y, &entry.text, theme.log_color(entry.category), theme.panel_bg);
        y -= 1;
    }
    let width = terminal.width();
    terminal.fill(rl_core::Rect::new(0, layout.status_row, width, 1), rl_render::Cell::new(' ', theme.text).on(theme.panel_bg));
    terminal.print_on(1, layout.status_row, &status.0, theme.text, theme.panel_bg);
}
