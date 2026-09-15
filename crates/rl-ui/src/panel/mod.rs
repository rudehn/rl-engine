//! Panels: one way of drawing each view, on the glyph terminal.
//!
//! Every panel here is a plugin holding the rectangle it draws in and the
//! title over it, and nothing else. It reads a view, reads the
//! [`Palette`], and writes cells. It owns no state, makes
//! no decisions a game might want to make differently, and can be left out
//! entirely: a game that adds [`NearbyViewPlugin`](crate::NearbyViewPlugin)
//! and not [`NearbyPanel`] gets the data and draws its own.
//!
//! That is the whole theming story. Three ways out, cheapest first:
//!
//! 1. Change the [`Palette`], including tones the engine
//!    never heard of. Every panel restyles at once.
//! 2. Keep the view, drop the panel, draw it yourself.
//! 3. Add neither plugin.
//!
//! The drawing helpers below are public because a game taking the second
//! way out should not have to rewrite a box-drawing routine to do it.

pub mod ability;
pub mod controls;
pub mod gear;
pub mod inspect;
pub mod log;
pub mod nearby;
pub mod scrollback;
pub mod sheet;
pub mod target;
pub mod vitals;

pub use ability::{AbilityLayout, AbilityMenu, AbilityPanel, ability_modal};
pub use controls::{CONTROLS_MODAL, ControlsLayout, ControlsPanel, ControlsScreen, controls_modal};
pub use gear::GearPanel;
pub use inspect::InspectPanel;
pub use log::LogPanel;
pub use nearby::NearbyPanel;
pub use scrollback::{SCROLLBACK_MODAL, Scrollback, ScrollbackKeys, ScrollbackPanel, scrollback_modal};
pub use sheet::{SHEET_MODAL, SheetKeys, SheetLayout, SheetPanel, plain_op, sheet_modal};
pub use target::{TargetLayout, TargetPanel};
pub use vitals::VitalsPanel;

use rl_core::{Point, Rect};
use rl_render::{Cell, MapView, Terminal};

use crate::tone::{Palette, ToneId, Tones};

/// Clears `rect` to the surface colour. Every panel starts here: a shorter
/// line must not leave the tail of a longer one behind it.
pub fn clear(terminal: &mut Terminal, rect: Rect, palette: &Palette) {
    terminal.fill(rect, Cell::new(' ', palette.get(Tones::TEXT)).on(palette.get(Tones::SURFACE)));
}

/// Draws a single-line box around the edge of `rect`, with `title` in the
/// top border and `hints` in the bottom one. Either may be empty.
pub fn frame(terminal: &mut Terminal, rect: Rect, title: &str, hints: &str, palette: &Palette) {
    if rect.width < 2 || rect.height < 2 {
        return;
    }
    let line = palette.get(Tones::FRAME);
    let (x0, y0, x1, y1) = (rect.x, rect.y, rect.right() - 1, rect.bottom() - 1);
    for x in x0 + 1..x1 {
        terminal.put(x, y0, '\u{2500}', line);
        terminal.put(x, y1, '\u{2500}', line);
    }
    for y in y0 + 1..y1 {
        terminal.put(x0, y, '\u{2502}', line);
        terminal.put(x1, y, '\u{2502}', line);
    }
    terminal.put(x0, y0, '\u{250c}', line);
    terminal.put(x1, y0, '\u{2510}', line);
    terminal.put(x0, y1, '\u{2514}', line);
    terminal.put(x1, y1, '\u{2518}', line);
    if !title.is_empty() {
        terminal.print_on(x0 + 2, y0, &format!(" {title} "), palette.get(Tones::TITLE), palette.get(Tones::SURFACE));
    }
    if !hints.is_empty() {
        let text = format!(" {hints} ");
        let x = x1 - 1 - text.chars().count() as i32;
        terminal.print_on(x.max(x0 + 1), y1, &text, palette.get(Tones::MUTED), palette.get(Tones::SURFACE));
    }
}

/// Draws a section heading at `y`, underlined to the width of `rect`.
///
/// `count` is printed right-aligned on the same row when it is `Some`,
/// which is how a heading carries a live tally without a second row.
pub fn section(terminal: &mut Terminal, rect: Rect, y: i32, title: &str, count: Option<usize>, palette: &Palette) {
    let bg = palette.get(Tones::SURFACE);
    terminal.print_on(rect.x, y, title, palette.get(Tones::TITLE), bg);
    if let Some(n) = count {
        let text = n.to_string();
        terminal.print_on(rect.right() - text.chars().count() as i32, y, &text, palette.get(Tones::MUTED), bg);
    }
    for x in rect.x..rect.right() {
        terminal.put(x, y + 1, '\u{2500}', palette.get(Tones::FRAME));
    }
}

/// Draws a bar `width` cells wide, filled to `fraction` in `tone`.
///
/// Rounds the filled length up, so anything left at all shows as at least
/// one cell: a bar that reads empty while its owner is alive is a bar that
/// lies at the moment it matters most.
///
/// Drawn on whatever background its cells already have, so a bar on a
/// highlighted row stays part of the highlight.
pub fn bar(terminal: &mut Terminal, x: i32, y: i32, width: i32, fraction: f32, tone: ToneId, palette: &Palette) {
    let filled = if fraction <= 0.0 { 0 } else { ((fraction.clamp(0.0, 1.0) * width as f32).ceil() as i32).min(width).max(1) };
    for i in 0..width {
        let (glyph, color) = if i < filled { ('\u{2588}', palette.get(tone)) } else { ('\u{2591}', palette.get(Tones::MUTED)) };
        let under = terminal.get(x + i, y).map_or(palette.get(Tones::SURFACE), |cell| cell.bg);
        terminal.set(x + i, y, Cell::new(glyph, color).on(under));
    }
}

/// Repaints the background of the map cell showing world `cell` in `tone`,
/// keeping whatever glyph the map drew there, so a highlight marks a
/// monster without hiding it. Nothing happens for a cell the map view is
/// not showing.
pub fn tint(terminal: &mut Terminal, map: &MapView, cell: Point, tone: ToneId, palette: &Palette) {
    let Some(screen) = map.to_screen(cell) else { return };
    let Some(mut drawn) = terminal.get(screen.x, screen.y) else { return };
    drawn.bg = palette.get(tone);
    terminal.set(screen.x, screen.y, drawn);
}

/// Cuts `text` to `width` characters, with an ellipsis when it did not
/// fit. Panels are narrow and a name that runs off the edge reads as a
/// different name.
pub fn clip(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text.chars().count() <= width {
        return text.to_string();
    }
    let mut out: String = text.chars().take(width.saturating_sub(1)).collect();
    out.push('\u{2026}');
    out
}

/// Breaks `text` into lines of at most `width` characters, on spaces.
///
/// A word longer than the whole width is cut rather than allowed to run
/// off the edge, since a line that runs off is a line that overwrites
/// whatever a panel drew beside it. Used by the scrollback, where losing
/// the end of a sentence is worse than spending a second row on it, and
/// public because a game writing its own presenter wants the same.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut len = 0;
    for word in text.split_whitespace() {
        let word_len = word.chars().count();
        if word_len > width {
            if len > 0 {
                lines.push(std::mem::take(&mut line));
                len = 0;
            }
            for chunk in word.chars().collect::<Vec<_>>().chunks(width) {
                lines.push(chunk.iter().collect());
            }
            continue;
        }
        if len > 0 && len + 1 + word_len > width {
            lines.push(std::mem::take(&mut line));
            len = 0;
        }
        if len > 0 {
            line.push(' ');
            len += 1;
        }
        line.push_str(word);
        len += word_len;
    }
    if len > 0 {
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Splits `rect` into everything left of a column `width` wide and that
/// column. For a game laying a rail beside the map.
pub fn split_right(rect: Rect, width: i32) -> (Rect, Rect) {
    let width = width.clamp(0, rect.width);
    (Rect::new(rect.x, rect.y, rect.width - width, rect.height), Rect::new(rect.right() - width, rect.y, width, rect.height))
}

/// Splits `rect` into everything above a strip `height` tall and that
/// strip. For a game laying a log under the map.
pub fn split_bottom(rect: Rect, height: i32) -> (Rect, Rect) {
    let height = height.clamp(0, rect.height);
    (Rect::new(rect.x, rect.y, rect.width, rect.height - height), Rect::new(rect.x, rect.bottom() - height, rect.width, height))
}

/// Splits `rect` into a strip `height` tall and everything below it. For a
/// game laying a status line over the map.
pub fn split_top(rect: Rect, height: i32) -> (Rect, Rect) {
    let height = height.clamp(0, rect.height);
    (Rect::new(rect.x, rect.y, rect.width, height), Rect::new(rect.x, rect.y + height, rect.width, rect.height - height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Vec2;

    fn row(t: &Terminal, y: i32, width: i32) -> String {
        (0..width).map(|x| t.get(x, y).unwrap().glyph).collect()
    }

    #[test]
    fn a_bar_with_anything_left_shows_at_least_one_cell_and_a_full_one_fills() {
        let palette = Palette::default();
        let mut t = Terminal::new(10, 3, Vec2::ONE);
        bar(&mut t, 0, 0, 10, 0.01, Tones::GOOD, &palette);
        assert_eq!(row(&t, 0, 10).matches('\u{2588}').count(), 1, "one hit point is not no hit points");
        bar(&mut t, 0, 1, 10, 1.0, Tones::GOOD, &palette);
        assert_eq!(row(&t, 1, 10).matches('\u{2588}').count(), 10);
        bar(&mut t, 0, 2, 10, 0.0, Tones::GOOD, &palette);
        assert_eq!(row(&t, 2, 10).matches('\u{2588}').count(), 0, "dead is empty");
    }

    #[test]
    fn wrapping_breaks_on_spaces_and_cuts_only_a_word_that_never_fits() {
        assert_eq!(wrap("the crab nips you", 20), vec!["the crab nips you"]);
        assert_eq!(wrap("the crab nips you", 9), vec!["the crab", "nips you"]);
        assert_eq!(wrap("", 10), vec![""], "an empty line is still a line");
        assert_eq!(wrap("antidisestablishmentarianism", 10), vec!["antidisest", "ablishment", "arianism"]);
        assert_eq!(wrap("you hit antidisestablishmentarianism", 10), vec!["you hit", "antidisest", "ablishment", "arianism"]);
        assert!(wrap("anything", 0).is_empty());
    }

    #[test]
    fn a_name_too_long_for_the_column_ends_in_an_ellipsis() {
        assert_eq!(clip("cutlass", 10), "cutlass");
        assert_eq!(clip("cutlass", 7), "cutlass");
        assert_eq!(clip("cutlass", 5), "cutl\u{2026}");
        assert_eq!(clip("cutlass", 0), "");
    }

    #[test]
    fn splitting_a_rectangle_loses_no_cells_and_survives_a_width_it_cannot_give() {
        let whole = Rect::new(0, 0, 100, 40);
        let (left, right) = split_right(whole, 24);
        assert_eq!(left.width + right.width, whole.width);
        assert_eq!(right.x, 76);
        let (top, bottom) = split_bottom(whole, 5);
        assert_eq!(top.height + bottom.height, whole.height);
        assert_eq!(bottom.y, 35);
        let (all, none) = split_right(whole, 500);
        assert_eq!(all.width, 0);
        assert_eq!(none.width, 100);
    }
}
