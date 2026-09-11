//! A framed list with a cursor: inventories, pickers, anything chosen
//! from a list.
//!
//! The widget is state plus a draw call. The game owns the state, fills
//! the rows from whatever it is listing, moves the cursor from its own
//! key bindings, and draws it into any rectangle of the terminal. Nothing
//! here reads input or knows what a row means.

use rl_core::Rect;
use rl_render::{Cell, Terminal};

use crate::log::LogCategory;
use crate::theme::Theme;

/// One line of a menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuRow {
    /// The text on the left.
    pub label: String,
    /// A short note on the right: "worn", "x12".
    pub tag: String,
    /// A longer line shown under the list while this row is selected.
    pub detail: String,
    /// Colours the label like a log line of this category.
    pub category: LogCategory,
}

impl MenuRow {
    /// A plain row.
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), tag: String::new(), detail: String::new(), category: LogCategory::Info }
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

    /// Sets the colour category.
    pub fn category(mut self, category: LogCategory) -> Self {
        self.category = category;
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
pub fn draw_menu(terminal: &mut Terminal, rect: Rect, menu: &ListMenu, theme: &Theme) {
    if rect.width < 4 || rect.height < 4 {
        return;
    }
    terminal.fill(rect, Cell::new(' ', theme.text).on(theme.panel_bg));
    draw_frame(terminal, rect, theme);
    let title = format!(" {} ", menu.title);
    terminal.print_on(rect.x + 2, rect.y, &title, theme.title, theme.panel_bg);
    if !menu.hints.is_empty() {
        let hints = format!(" {} ", menu.hints);
        let x = rect.right() - 2 - hints.chars().count() as i32;
        terminal.print_on(x.max(rect.x + 1), rect.bottom() - 1, &hints, theme.muted, theme.panel_bg);
    }

    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    // The last inner row is the detail line, with a blank row above it.
    let list_rows = (inner.height - 2).max(1) as usize;
    if menu.rows.is_empty() {
        terminal.print_on(inner.x, inner.y, &menu.empty, theme.muted, theme.panel_bg);
        return;
    }
    let first = menu.selected.saturating_sub(list_rows - 1).min(menu.rows.len().saturating_sub(list_rows));
    for (i, row) in menu.rows.iter().enumerate().skip(first).take(list_rows) {
        let y = inner.y + (i - first) as i32;
        let selected = i == menu.selected;
        let bg = if selected { theme.highlight_bg } else { theme.panel_bg };
        let fg = if selected { theme.text } else { theme.log_color(row.category) };
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', fg).on(bg));
        let label = clip(&row.label, inner.width as usize - row.tag.chars().count() - 1);
        terminal.print_on(inner.x, y, &label, fg, bg);
        if !row.tag.is_empty() {
            let x = inner.right() - row.tag.chars().count() as i32;
            terminal.print_on(x, y, &row.tag, theme.muted, bg);
        }
    }
    if let Some(row) = menu.selected_row() {
        let detail = clip(&row.detail, inner.width as usize);
        terminal.print_on(inner.x, inner.bottom() - 1, &detail, theme.muted, theme.panel_bg);
    }
}

/// Draws a single-line box around the edge of `rect`.
pub fn draw_frame(terminal: &mut Terminal, rect: Rect, theme: &Theme) {
    let (x0, y0, x1, y1) = (rect.x, rect.y, rect.right() - 1, rect.bottom() - 1);
    for x in x0 + 1..x1 {
        terminal.put(x, y0, '\u{2500}', theme.frame);
        terminal.put(x, y1, '\u{2500}', theme.frame);
    }
    for y in y0 + 1..y1 {
        terminal.put(x0, y, '\u{2502}', theme.frame);
        terminal.put(x1, y, '\u{2502}', theme.frame);
    }
    terminal.put(x0, y0, '\u{250c}', theme.frame);
    terminal.put(x1, y0, '\u{2510}', theme.frame);
    terminal.put(x0, y1, '\u{2514}', theme.frame);
    terminal.put(x1, y1, '\u{2518}', theme.frame);
}

fn clip(s: &str, width: usize) -> String {
    s.chars().take(width).collect()
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
        draw_menu(&mut t, rect, &m, &Theme::default());
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
