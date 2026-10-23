//! The cells of a frame as the bytes a terminal draws them from.
//!
//! A frame writes only the cells that changed since the last one, each
//! as a move, its colours when they differ from the cell written before
//! it, and its glyph: a whole screen of escape codes every frame would
//! fill an SSH link thirty times a second for a game that mostly stands
//! still.

use std::collections::BTreeSet;

use bevy::color::{Color, ColorToComponents};
use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::{Color as Ink, Print, SetBackgroundColor, SetForegroundColor};
use rl_render::{Cell, Terminal};
use unicode_width::UnicodeWidthChar;

/// How many colours the terminal draws in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colors {
    /// Any colour, as the cell has it: most terminals made since about
    /// 2016 draw them, and say so in `COLORTERM`.
    True,
    /// The 256 of xterm's palette, each cell's colours drawn in the nearest
    /// of them, for a terminal that would draw a colour it does not know
    /// as something else entirely.
    Ansi256,
}

impl Colors {
    /// What the environment says the terminal draws: [`Colors::True`] when
    /// `COLORTERM` is `truecolor` or `24bit`, which the terminals that
    /// draw any colour set, and [`Colors::Ansi256`] otherwise.
    pub fn from_env() -> Self {
        match std::env::var("COLORTERM").as_deref() {
            Ok("truecolor" | "24bit") => Self::True,
            _ => Self::Ansi256,
        }
    }

    /// `color` as this many colours can draw it.
    fn ink(self, color: Color) -> Ink {
        let [r, g, b] = color.to_srgba().to_f32_array_no_alpha().map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
        match self {
            Self::True => Ink::Rgb { r, g, b },
            Self::Ansi256 => Ink::AnsiValue(ansi256(r, g, b)),
        }
    }
}

/// The cells of `now` to write: every one when nothing was written
/// before, or when what was is another size, and otherwise those that
/// differ from `last`, each with its column and row.
pub fn changes(last: Option<&[Cell]>, now: &Terminal) -> Vec<(u16, u16, Cell)> {
    let cells = now.cells();
    let width = now.width() as usize;
    let at = |(i, cell): (usize, &Cell)| ((i % width) as u16, (i / width) as u16, *cell);
    match last {
        Some(last) if last.len() == cells.len() => cells.iter().enumerate().filter(|&(i, cell)| *cell != last[i]).map(at).collect(),
        _ => cells.iter().enumerate().map(at).collect(),
    }
}

/// `c` when a terminal draws it one column wide, and `?` otherwise.
///
/// A glyph two columns wide, an ideograph or most emoji, pushes every
/// cell after it on its row one column right, and a control character or
/// a combining mark draws nothing or moves the cursor, so either would
/// tear the rest of the row. The window draws them in their cell; the
/// terminal cannot, and a question mark in the cell says where.
pub fn shown(c: char) -> char {
    if c.width() == Some(1) { c } else { '?' }
}

/// Writes `cells` to `out`, each moved by `origin`, the corner of the
/// terminal the grid is drawn from, and in `colors`; each glyph a
/// terminal cannot draw in one column is added to `odd`, which is
/// returned so the caller warns once for each.
pub(crate) fn paint(out: &mut Vec<u8>, cells: &[(u16, u16, Cell)], origin: (u16, u16), colors: Colors, odd: &mut BTreeSet<char>) -> Vec<char> {
    let mut fresh = Vec::new();
    let (mut fg, mut bg, mut cursor) = (None, None, None);
    for &(x, y, cell) in cells {
        let at = (origin.0 + x, origin.1 + y);
        if cursor != Some(at) {
            let _ = queue!(out, MoveTo(at.0, at.1));
        }
        let (ink, fill) = (colors.ink(cell.fg), colors.ink(cell.bg));
        if fg != Some(ink) {
            let _ = queue!(out, SetForegroundColor(ink));
            fg = Some(ink);
        }
        if bg != Some(fill) {
            let _ = queue!(out, SetBackgroundColor(fill));
            bg = Some(fill);
        }
        let glyph = shown(cell.glyph);
        if glyph != cell.glyph && odd.insert(cell.glyph) {
            fresh.push(cell.glyph);
        }
        let _ = queue!(out, Print(glyph));
        cursor = Some((at.0 + 1, at.1));
    }
    fresh
}

/// The nearest of xterm's 256 colours to `(r, g, b)`: the nearer of the
/// 6 by 6 by 6 cube's colour and the grey ramp's, by squared distance.
/// The first sixteen are left out, since every terminal shows them in a
/// theme of its own.
pub(crate) fn ansi256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let step = |v: u8| match v {
        0..48 => 0,
        48..115 => 1,
        _ => (v - 35) / 40,
    };
    let distance = |(r2, g2, b2): (u8, u8, u8)| {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        d(r, r2) + d(g, g2) + d(b, b2)
    };
    let (ri, gi, bi) = (step(r), step(g), step(b));
    let cube = (LEVELS[ri as usize], LEVELS[gi as usize], LEVELS[bi as usize]);
    let mean = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;
    let grey_step = (mean.saturating_sub(3) / 10).min(23) as u8;
    let grey_level = 8 + 10 * grey_step;
    if distance((grey_level, grey_level, grey_level)) < distance(cube) { 232 + grey_step } else { 16 + 36 * ri + 6 * gi + bi }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::Vec2;

    fn terminal_of(glyph: char) -> Terminal {
        let mut terminal = Terminal::new(4, 3, Vec2::ONE);
        terminal.fill(terminal.bounds(), Cell::new(glyph, Color::WHITE));
        terminal
    }

    #[test]
    fn with_nothing_written_before_every_cell_is() {
        let terminal = terminal_of('.');
        let all = changes(None, &terminal);
        assert_eq!(all.len(), 12);
        assert_eq!((all[5].0, all[5].1), (1, 1), "each with its column and row");
    }

    #[test]
    fn only_the_cells_that_changed_are_written() {
        let mut terminal = terminal_of('.');
        let last = terminal.cells().to_vec();
        terminal.put(2, 1, '@', Color::WHITE);
        assert_eq!(changes(Some(&last), &terminal), vec![(2, 1, Cell::new('@', Color::WHITE))]);
        assert!(changes(Some(terminal.cells()), &terminal).is_empty(), "and none when nothing did");
    }

    #[test]
    fn a_glyph_two_columns_wide_is_drawn_as_a_question_mark() {
        for wide in ['\u{4e2d}', '\u{1f600}', '\u{7}', '\u{301}'] {
            assert_eq!(shown(wide), '?', "{wide:?}");
        }
        for narrow in ['@', '#', '\u{2500}', '\u{2588}', '\u{b7}'] {
            assert_eq!(shown(narrow), narrow, "{narrow:?}");
        }
        let mut odd = BTreeSet::new();
        let mut out = Vec::new();
        let wide = Cell::new('\u{4e2d}', Color::WHITE);
        let fresh = paint(&mut out, &[(0, 0, wide), (1, 0, wide)], (0, 0), Colors::True, &mut odd);
        assert_eq!(fresh, vec!['\u{4e2d}'], "named once, to warn about once");
        assert_eq!(String::from_utf8(out).unwrap().matches('?').count(), 2);
    }

    #[test]
    fn a_colour_is_drawn_in_the_nearest_of_256_where_the_terminal_has_no_more() {
        assert_eq!(ansi256(0, 0, 0), 16);
        assert_eq!(ansi256(255, 255, 255), 231);
        assert_eq!(ansi256(255, 0, 0), 196);
        assert_eq!(ansi256(128, 128, 128), 244, "a grey is the ramp's");
        assert_eq!(ansi256(95, 135, 175), 16 + 36 + 2 * 6 + 3, "a colour of the cube is itself");
    }

    #[test]
    fn a_row_written_left_to_right_moves_the_cursor_once_and_sets_each_colour_once() {
        let cells: Vec<_> = (0..3).map(|x| (x, 0, Cell::new('#', Color::WHITE))).collect();
        let mut out = Vec::new();
        paint(&mut out, &cells, (10, 5), Colors::True, &mut BTreeSet::new());
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.matches("\x1b[6;11H").count(), 1, "one move, to the origin: {text:?}");
        assert_eq!(text.matches('H').count(), 1, "and no other");
        assert_eq!(text.matches("38;2").count(), 1, "one foreground");
        assert!(text.ends_with("###"));
    }
}
