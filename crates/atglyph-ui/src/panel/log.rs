//! The message log, newest at the bottom.

use atglyph_bevy::PresentSet;
use atglyph_core::GridRect;
use atglyph_render::Terminal;
use bevy::prelude::*;

use crate::log::LogEntry;
use crate::log::MessageLog;
use crate::panel::{Segment, clear, print_rich, wrap_rich};
use crate::tone::ToneId;
use crate::tone::{Palette, Tones};

/// Where the log is drawn.
#[derive(Resource, Debug, Clone)]
pub(crate) struct LogLayout {
    /// The terminal cells it occupies.
    pub rect: GridRect,
    /// Newest line at the bottom, which is where a player's eye rests
    /// after an action. `false` puts it at the top.
    pub newest_last: bool,
}

/// Draws [`MessageLog`]. Initialises the log if nothing else has.
pub struct LogPanel(LogLayout);

impl LogPanel {
    /// A log in `rect`, newest at the bottom.
    pub fn new(rect: GridRect) -> Self {
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

/// Paints the log: as many of the newest lines as fit whole, each wrapped
/// onto the rows it needs.
///
/// Wrapped rather than clipped, since the end of a line is as often what it
/// was for as its start: how much a blow took, what a shield caught. A
/// line is shown whole or not at all, so the strip never starts partway
/// through a sentence; only one line taller than the whole strip is cut,
/// showing its start and ending where it was cut.
pub(crate) fn draw_log(mut terminal: ResMut<Terminal>, layout: Res<LogLayout>, log: Res<MessageLog>, palette: Res<Palette>) {
    let rect = layout.rect;
    if rect.width < 4 || rect.height < 1 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    let bg = palette.get(Tones::SURFACE);
    let rows = fitted(log.recent(rect.height as usize), rect.width as usize, rect.height as usize);
    // Newest first, each line's rows in reading order.
    let ordered: Vec<(&Vec<Segment>, ToneId)> = if layout.newest_last {
        rows.iter().rev().flat_map(|(lines, tone)| lines.iter().map(|l| (l, *tone))).collect()
    } else {
        rows.iter().flat_map(|(lines, tone)| lines.iter().map(|l| (l, *tone))).collect()
    };
    let top = if layout.newest_last { rect.bottom() - ordered.len() as i32 } else { rect.y };
    for (i, (line, tone)) in ordered.into_iter().enumerate() {
        print_rich(&mut terminal, rect.x, top + i as i32, line, palette.get(tone), bg, &palette);
    }
}

/// The newest `entries` that fit whole in `height` rows of `width`, newest
/// first, each as its wrapped rows and its tone; the newest alone cut to
/// the strip, with an ellipsis, when it is taller than all of it.
fn fitted<'a>(entries: impl Iterator<Item = &'a LogEntry>, width: usize, height: usize) -> Vec<(Vec<Vec<Segment>>, ToneId)> {
    let mut shown = Vec::new();
    let mut used = 0;
    for entry in entries {
        let mut lines = wrap_rich(&entry.display(), &entry.spans, width);
        if used + lines.len() > height {
            if used == 0 {
                lines.truncate(height);
                if let Some(last) = lines.last_mut() {
                    cut(last, width);
                }
                shown.push((lines, entry.tone));
            }
            break;
        }
        used += lines.len();
        shown.push((lines, entry.tone));
    }
    shown
}

/// Ends `line` with an ellipsis, in place of its last character when it
/// fills `width`.
fn cut(line: &mut Vec<Segment>, width: usize) {
    let len: usize = line.iter().map(|(text, _)| text.chars().count()).sum();
    if len >= width
        && let Some((text, _)) = line.last_mut()
    {
        text.pop();
    }
    line.push(("\u{2026}".to_string(), None));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use crate::tone::Tones;

    #[test]
    fn the_newest_line_is_at_the_bottom_and_a_shorter_one_leaves_no_tail() {
        let mut stage = Stage::new(LogPanel::new(GridRect::new(0, 0, 40, 3))).screen(40, 3);
        stage.app.world_mut().resource_mut::<MessageLog>().push_at("a long line that fills the row", Tones::TEXT, 0);
        stage.tick();
        assert_eq!(stage.row(2), "a long line that fills the row");
        stage.app.world_mut().resource_mut::<MessageLog>().push_at("short", Tones::TEXT, 0);
        stage.tick();
        assert_eq!(stage.row(2), "short");
        assert_eq!(stage.row(1), "a long line that fills the row", "the older line moved up intact");
        assert_eq!(stage.row(0), "");
    }

    /// A line too long for the strip wraps onto the rows it needs, its
    /// names keeping their colours across the break, and the line before it
    /// moves up whole.
    #[test]
    fn a_long_line_wraps_onto_the_rows_it_needs_and_keeps_its_colours() {
        let mut stage = Stage::new(LogPanel::new(GridRect::new(0, 0, 20, 4))).screen(20, 4);
        let green = Color::srgb(0.2, 0.9, 0.3);
        let mut log = stage.app.world_mut().resource_mut::<MessageLog>();
        log.push_at("you wait", Tones::TEXT, 0);
        log.push_spans("the giant spider bites you hard", vec![crate::log::Span { start: 10, len: 6, color: green }], Tones::BAD, 1);
        stage.tick();
        assert_eq!(stage.rows(), ["", "you wait", "the giant spider", "bites you hard"]);
        let t = stage.app.world().resource::<atglyph_render::Terminal>();
        assert_eq!(t.get(10, 2).unwrap().fg, green, "'spider' in its own green");
        let palette = stage.app.world().resource::<Palette>();
        assert_eq!(t.get(0, 3).unwrap().fg, palette.get(Tones::BAD), "the wrapped row in the line's tone");
    }

    /// The oldest line shown is shown whole or not at all, so the strip
    /// never starts partway through a sentence; newest first, the same.
    #[test]
    fn a_line_that_does_not_fit_whole_is_left_out_rather_than_cut() {
        for newest_first in [false, true] {
            let panel = LogPanel::new(GridRect::new(0, 0, 20, 2));
            let panel = if newest_first { panel.newest_first() } else { panel };
            let mut stage = Stage::new(panel).screen(20, 2);
            let mut log = stage.app.world_mut().resource_mut::<MessageLog>();
            log.push_at("the giant spider bites you hard", Tones::BAD, 1);
            log.push_at("you wait", Tones::TEXT, 1);
            stage.tick();
            let rows = if newest_first { ["you wait", ""] } else { ["", "you wait"] };
            assert_eq!(stage.rows(), rows, "newest first: {newest_first}");
        }
    }

    /// A line longer than the whole strip shows its start, ending where it
    /// was cut, rather than nothing.
    #[test]
    fn a_line_taller_than_the_strip_shows_its_start_and_where_it_was_cut() {
        let mut stage = Stage::new(LogPanel::new(GridRect::new(0, 0, 20, 1))).screen(20, 1);
        stage.app.world_mut().resource_mut::<MessageLog>().push_at("the giant spider bites you hard", Tones::BAD, 1);
        stage.tick();
        assert_eq!(stage.rows(), ["the giant spider\u{2026}"]);
    }

    #[test]
    fn a_repeated_line_shows_its_count_rather_than_filling_the_panel() {
        let mut stage = Stage::new(LogPanel::new(GridRect::new(0, 0, 40, 3))).screen(40, 3);
        for _ in 0..3 {
            stage.app.world_mut().resource_mut::<MessageLog>().push_at("the crab nips you", Tones::BAD, 1);
        }
        stage.tick();
        assert_eq!(stage.row(2), "the crab nips you (x3)");
        assert_eq!(stage.row(1), "", "one line, not three");
    }

    /// A name carries the colour of what it names, lifted to readable; the
    /// rest of the line stays in its tone.
    #[test]
    fn a_span_is_drawn_in_its_own_colour_and_the_rest_in_the_tone() {
        let mut stage = Stage::new(LogPanel::new(GridRect::new(0, 0, 40, 3))).screen(40, 3);
        let green = Color::srgb(0.2, 0.9, 0.3);
        stage.app.world_mut().resource_mut::<MessageLog>().push_spans(
            "the slime nips you",
            vec![crate::log::Span { start: 4, len: 5, color: green }],
            Tones::BAD,
            1,
        );
        stage.tick();
        let t = stage.app.world().resource::<atglyph_render::Terminal>();
        let palette = stage.app.world().resource::<Palette>();
        assert_eq!(t.get(0, 2).unwrap().fg, palette.get(Tones::BAD), "'the' in the tone");
        assert_eq!(t.get(4, 2).unwrap().fg, green, "'slime' in its own green");
        assert_eq!(t.get(10, 2).unwrap().fg, palette.get(Tones::BAD), "'nips' in the tone again");
    }

    #[test]
    fn a_line_is_drawn_in_the_tone_it_was_logged_in() {
        let mut stage = Stage::new(LogPanel::new(GridRect::new(0, 0, 40, 3))).screen(40, 3);
        stage.app.world_mut().resource_mut::<MessageLog>().push_at("bad news", Tones::BAD, 1);
        stage.tick();
        let cell = stage.app.world().resource::<atglyph_render::Terminal>().get(0, 2).unwrap();
        let palette = stage.app.world().resource::<Palette>();
        assert_eq!(cell.fg, palette.get(Tones::BAD));
        assert_ne!(cell.fg, palette.get(Tones::TEXT));
    }
}
