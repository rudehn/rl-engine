//! The message log, newest at the bottom.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;

use crate::log::MessageLog;
use crate::panel::{clear, clip_rich, print_rich, wrap_rich};
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
        // One row per entry, clipped rather than wrapped: the strip is for
        // the last few things, and the scrollback for reading them whole.
        let runs = clip_rich(&wrap_rich(&entry.display(), &entry.spans, usize::MAX).remove(0), rect.width as usize);
        print_rich(&mut terminal, rect.x, y, &runs, palette.get(entry.tone), bg, &palette);
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

    /// A name carries the colour of what it names, lifted to readable; the
    /// rest of the line stays in its tone.
    #[test]
    fn a_span_is_drawn_in_its_own_colour_and_the_rest_in_the_tone() {
        let mut stage = Stage::new(LogPanel::new(Rect::new(0, 0, 40, 3))).screen(40, 3);
        let green = Color::srgb(0.2, 0.9, 0.3);
        stage.app.world_mut().resource_mut::<MessageLog>().push_spans(
            "the slime nips you",
            vec![crate::log::Span { start: 4, len: 5, color: green }],
            Tones::BAD,
            1,
        );
        stage.tick();
        let t = stage.app.world().resource::<rl_render::Terminal>();
        let palette = stage.app.world().resource::<Palette>();
        assert_eq!(t.get(0, 2).unwrap().fg, palette.get(Tones::BAD), "'the' in the tone");
        assert_eq!(t.get(4, 2).unwrap().fg, green, "'slime' in its own green");
        assert_eq!(t.get(10, 2).unwrap().fg, palette.get(Tones::BAD), "'nips' in the tone again");
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
