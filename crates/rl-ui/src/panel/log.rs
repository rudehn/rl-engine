//! The message log, newest at the bottom.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;

use crate::log::MessageLog;
use crate::panel::{clear, clip};
use crate::tone::{Palette, Tones};

/// Where the log is drawn.
#[derive(Resource, Debug, Clone)]
pub struct LogLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
    /// Newest line at the bottom, which is where a player's eye rests
    /// after an action. `false` puts it at the top.
    pub newest_last: bool,
}

/// Draws [`MessageLog`]. Initialises the log if nothing else has.
pub struct LogPanel(LogLayout);

impl LogPanel {
    /// A log in `rect`, newest at the bottom.
    pub fn new(rect: Rect) -> Self {
        Self(LogLayout { rect, newest_last: true })
    }

    /// Puts the newest line at the top instead.
    pub fn newest_first(mut self) -> Self {
        self.0.newest_last = false;
        self
    }
}

impl Plugin for LogPanel {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0.clone()).add_systems(Update, draw_log.in_set(PresentSet::Chrome));
    }
}

/// Paints the log.
pub fn draw_log(mut terminal: ResMut<Terminal>, layout: Res<LogLayout>, log: Res<MessageLog>, palette: Res<Palette>) {
    let rect = layout.rect;
    if rect.width < 4 || rect.height < 1 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    let bg = palette.get(Tones::SURFACE);
    for (i, entry) in log.recent(rect.height as usize).enumerate() {
        let y = if layout.newest_last { rect.bottom() - 1 - i as i32 } else { rect.y + i as i32 };
        terminal.print_on(rect.x, y, &clip(&entry.display(), rect.width as usize), palette.get(entry.tone), bg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use crate::tone::Tones;

    #[test]
    fn the_newest_line_is_at_the_bottom_and_a_shorter_one_leaves_no_tail() {
        let mut stage = Stage::new(LogPanel::new(Rect::new(0, 0, 40, 3))).screen(40, 3);
        stage.app.world_mut().resource_mut::<MessageLog>().info("a long line that fills the row", 0);
        stage.tick();
        assert_eq!(stage.row(2), "a long line that fills the row");
        stage.app.world_mut().resource_mut::<MessageLog>().info("short", 0);
        stage.tick();
        assert_eq!(stage.row(2), "short");
        assert_eq!(stage.row(1), "a long line that fills the row", "the older line moved up intact");
        assert_eq!(stage.row(0), "");
    }

    #[test]
    fn a_repeated_line_shows_its_count_rather_than_filling_the_panel() {
        let mut stage = Stage::new(LogPanel::new(Rect::new(0, 0, 40, 3))).screen(40, 3);
        for _ in 0..3 {
            stage.app.world_mut().resource_mut::<MessageLog>().bad("the crab nips you", 1);
        }
        stage.tick();
        assert_eq!(stage.row(2), "the crab nips you (x3)");
        assert_eq!(stage.row(1), "", "one line, not three");
    }

    #[test]
    fn a_line_is_drawn_in_the_tone_it_was_logged_in() {
        let mut stage = Stage::new(LogPanel::new(Rect::new(0, 0, 40, 3))).screen(40, 3);
        stage.app.world_mut().resource_mut::<MessageLog>().bad("bad news", 1);
        stage.tick();
        let cell = stage.app.world().resource::<rl_render::Terminal>().get(0, 2).unwrap();
        let palette = stage.app.world().resource::<Palette>();
        assert_eq!(cell.fg, palette.get(Tones::BAD));
        assert_ne!(cell.fg, palette.get(Tones::TEXT));
    }
}
