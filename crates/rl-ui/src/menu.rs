//! A framed list with a cursor: inventories, pickers, anything chosen
//! from a list.
//!
//! The widget is state plus a draw call. The game owns the state, fills
//! the rows from whatever it is listing, moves the cursor from its own
//! key bindings, and draws it into any rectangle of the terminal. Nothing
//! here reads input or knows what a row means.
//!
//! A menu is not a view: what is in a list is the game's question, not the
//! engine's. It is here because scrolling a cursor so the selected row
//! stays visible is the same arithmetic every time, and getting it wrong
//! is how a list ends up jumping a page when the cursor reaches the edge.

use rl_core::Rect;
use rl_render::{Cell, Terminal};

use crate::panel::{clip, frame};
use crate::tone::{Palette, ToneId, Tones};

/// One line of a menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    /// The text on the left.
    pub label: String,
    /// A short note on the right: "worn", "x12".
    pub tag: String,
    /// A longer line shown under the list while this row is selected.
    pub detail: String,
    /// How the label reads.
    pub tone: ToneId,
}

impl MenuRow {
    /// A plain row.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), tag: String::new(), detail: String::new(), tone: Tones::TEXT }
    }

    /// Sets the note on the right.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    /// Sets the detail line.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = detail.into();
        self
    }

    /// Sets the tone the label reads in.
    pub fn toned(mut self, tone: ToneId) -> Self {
        self.tone = tone;
        self
    }
}

/// A list with a cursor.
#[derive(Debug, Clone, Default)]
pub struct ListMenu {
    /// Drawn in the top border.
    pub title: String,
    /// The rows.
    pub rows: Vec<MenuRow>,
    /// Index of the row the cursor is on. Clamped by the movement methods
    /// and by [`set_rows`](Self::set_rows).
    pub selected: usize,
    /// Key hints drawn in the bottom border.
    pub hints: String,
    /// Shown instead of rows when there are none.
    pub empty: String,
}

impl ListMenu {
    /// An empty menu titled `title`.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), ..Self::default() }
    }

    /// Replaces the rows, keeping the cursor on the same index where it
    /// still exists.
    pub fn set_rows(&mut self, rows: Vec<MenuRow>) {
        self.rows = rows;
        self.clamp();
    }

    /// Moves the cursor by `delta`, wrapping at both ends.
    pub fn move_by(&mut self, delta: i32) {
        let n = self.rows.len();
        if n == 0 {
            self.selected = 0;
            return;
        }
        self.selected = (self.selected as i64 + delta as i64).rem_euclid(n as i64) as usize;
    }

    /// The row under the cursor.
    pub fn selected_row(&self) -> Option<&MenuRow> {
        self.rows.get(self.selected)
    }

    fn clamp(&mut self) {
        self.selected = self.selected.min(self.rows.len().saturating_sub(1));
    }
}

/// Draws `menu` framed inside `rect`: title in the top border, rows
/// scrolled to keep the cursor visible, the selected row's detail under
/// them, and the hints in the bottom border.
pub fn draw_menu(terminal: &mut Terminal, rect: Rect, menu: &ListMenu, palette: &Palette) {
    if rect.width < 4 || rect.height < 4 {
        return;
    }
    let surface = palette.get(Tones::SURFACE);
    terminal.fill(rect, Cell::new(' ', palette.get(Tones::TEXT)).on(surface));
    frame(terminal, rect, &menu.title, &menu.hints, palette);

    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    // The last inner row is the detail line, with a blank row above it.
    let list_rows = (inner.height - 2).max(1) as usize;
    if menu.rows.is_empty() {
        terminal.print_on(inner.x, inner.y, &menu.empty, palette.get(Tones::MUTED), surface);
        return;
    }
    let first = menu.selected.saturating_sub(list_rows - 1).min(menu.rows.len().saturating_sub(list_rows));
    for (i, row) in menu.rows.iter().enumerate().skip(first).take(list_rows) {
        let y = inner.y + (i - first) as i32;
        let selected = i == menu.selected;
        let bg = if selected { palette.get(Tones::SELECT) } else { surface };
        let fg = if selected { palette.get(Tones::TEXT) } else { palette.get(row.tone) };
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', fg).on(bg));
        let label = clip(&row.label, (inner.width as usize).saturating_sub(row.tag.chars().count() + 1));
        terminal.print_on(inner.x, y, &label, fg, bg);
        if !row.tag.is_empty() {
            let x = inner.right() - row.tag.chars().count() as i32;
            terminal.print_on(x, y, &row.tag, palette.get(Tones::MUTED), bg);
        }
    }
    if let Some(row) = menu.selected_row() {
        let detail = clip(&row.detail, inner.width as usize);
        terminal.print_on(inner.x, inner.bottom() - 1, &detail, palette.get(Tones::MUTED), surface);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_wraps_and_survives_a_shorter_list() {
        let mut m = ListMenu::new("bag");
        m.set_rows((0..5).map(|i| MenuRow::new(format!("row {i}"))).collect());
        m.move_by(-1);
        assert_eq!(m.selected, 4);
        m.move_by(1);
        assert_eq!(m.selected, 0);
        m.move_by(3);
        m.set_rows((0..2).map(|i| MenuRow::new(format!("row {i}"))).collect());
        assert_eq!(m.selected, 1, "clamped to the last row");
        m.set_rows(Vec::new());
        assert_eq!(m.selected_row(), None);
        m.move_by(1);
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn drawing_keeps_the_cursor_row_in_view() {
        let mut t = Terminal::new(40, 10, bevy::prelude::Vec2::ONE);
        let mut m = ListMenu::new("bag");
        m.hints = "q".into();
        m.set_rows((0..20).map(|i| MenuRow::new(format!("row {i}")).tag("t").detail(format!("detail {i}"))).collect());
        m.selected = 15;
        let rect = Rect::new(0, 0, 40, 10);
        draw_menu(&mut t, rect, &m, &Palette::default());
        let line = |y: i32| -> String { (0..40).map(|x| t.get(x, y).unwrap().glyph).collect() };
        assert!(line(0).contains(" bag "), "title in the top border: {:?}", line(0));
        assert!(line(9).contains(" q "), "hints in the bottom border: {:?}", line(9));
        let shown: Vec<String> = (1..7).map(line).collect();
        assert!(shown.iter().any(|l| l.contains("row 15")), "selected row visible: {shown:?}");
        assert!(!shown.iter().any(|l| l.contains("row 0 ")), "scrolled past the top");
        assert!(line(8).contains("detail 15"), "detail under the list: {:?}", line(8));
        assert_eq!(line(0).chars().next(), Some('\u{250c}'));
    }
}
