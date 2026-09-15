//! The controls screen: every key the game answers to, on one screen.
//!
//! Drawn from the [`Controls`] registry, so it lists what the game reads
//! and nothing else: a key the game stops reading leaves the screen when
//! its declaration goes, and a cursor key a game rebinds is listed as
//! rebound. Groups come in the order they were declared, each a heading
//! and its rows, flowed into as many columns as fit and then into pages.
//!
//! The one hand-typed hint left on a screen is the one that opens this:
//! [`ControlsPanel::hint`] prints it wherever the game has a spare corner,
//! from the key that really opens the screen.

use crate::modal::AddModal;
use bevy::prelude::*;
use rl_bevy::{EngineSet, PresentSet};
use rl_core::Rect;
use rl_render::Terminal;

use crate::controls::{Bindings, ControlInput, Controls, ControlsKeys, EngineKey, key_name};
use crate::modal::{ModalId, Modals};
use crate::panel::{clear, clip, frame};
use crate::tone::{Palette, Tones};

/// The name the controls screen's modal is declared under.
pub const CONTROLS_MODAL: &str = "controls";

/// Cells between two columns of controls.
const GUTTER: i32 = 3;
/// The widest the key column is allowed to grow before the actions are
/// pushed off the right of a narrow screen: room for the direction
/// families, `arrows hjklyubn numpad`, which are the widest keys a game
/// is likely to list.
const KEYS_WIDTH_CAP: usize = 24;

/// Which page of the screen is showing.
#[derive(Resource, Debug, Default)]
pub struct ControlsScreen {
    /// Zero-based.
    pub page: usize,
}

/// Where the screen and its hint are drawn.
#[derive(Resource, Debug, Clone)]
pub struct ControlsLayout {
    /// The terminal cells the screen occupies, border included.
    pub rect: Rect,
    /// The title in the top border.
    pub title: String,
    /// One row the hint is printed in, right-aligned, while nothing is
    /// open. `None` prints no hint.
    pub hint: Option<Rect>,
    /// The word after the key in the hint.
    pub hint_word: String,
}

/// Draws the [`Controls`] registry as a screen, and the hint that opens it.
pub struct ControlsPanel(ControlsLayout);

impl ControlsPanel {
    /// A screen in `rect`.
    pub fn new(rect: Rect) -> Self {
        Self(ControlsLayout { rect, title: "Controls".into(), hint: None, hint_word: "controls".into() })
    }

    /// Sets the title in the top border.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Prints the hint that opens the screen, right-aligned in `rect`, while
    /// nothing is open: the key and a word, `? controls` by default.
    pub fn hint(mut self, rect: Rect) -> Self {
        self.0.hint = Some(rect);
        self
    }

    /// Replaces the word after the key in the hint.
    pub fn hint_word(mut self, word: impl Into<String>) -> Self {
        self.0.hint_word = word.into();
        self
    }
}

impl Plugin for ControlsPanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<Controls>().init_resource::<ControlsKeys>().init_resource::<ControlsScreen>().insert_resource(self.0.clone());
        app.add_modal(CONTROLS_MODAL);
        app.add_systems(Update, controls_keys.in_set(EngineSet::Input))
            .add_systems(Update, draw_hint.in_set(PresentSet::Chrome))
            .add_systems(Update, draw_controls.in_set(PresentSet::Overlay));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "ControlsPanel");
        // After the game's own, so its groups are listed first.
        app.world_mut().resource_mut::<Controls>().add("Screens", "show the controls", EngineKey::ShowControls);
    }
}

/// The id of the controls screen's modal, for a game gating its own
/// systems.
///
/// # Panics
/// Panics if [`ControlsPanel`] was not added.
pub fn controls_modal(modals: &Modals) -> ModalId {
    modals.get(CONTROLS_MODAL).expect("ControlsPanel declares the controls modal")
}

/// Opens, closes and pages the screen.
pub fn controls_keys(keys: ControlInput, layout: Res<ControlsLayout>, mut screen: ResMut<ControlsScreen>, mut modals: ResMut<Modals>) {
    let modal = controls_modal(&modals);
    let binds = keys.bindings();
    if binds.help.toggle.just_pressed(keys.input()) && (modals.is_top(modal) || !modals.any_open()) {
        modals.toggle(modal);
        screen.page = 0;
        return;
    }
    if !modals.is_top(modal) {
        return;
    }
    if keys.input().just_pressed(binds.help.close) {
        modals.close_one(modal);
        return;
    }
    let pages = paginate(&lines(keys.controls(), &binds), inner_of(layout.rect)).len().max(1);
    if keys.input().just_pressed(binds.help.next_page) {
        screen.page = (screen.page + 1) % pages;
    }
    if keys.input().just_pressed(binds.help.prev_page) {
        screen.page = (screen.page + pages - 1) % pages;
    }
}

/// One row of the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    /// A group's name.
    Heading(String),
    /// The keys and what they do.
    Row(String, String),
    /// The gap between groups.
    Blank,
}

/// Every line of the screen, in declaration order, groups gathered under
/// the heading where each was first declared.
///
/// A control nothing binds, such as a log key in a game with no
/// scrollback, is left out.
fn lines(controls: &Controls, bindings: &Bindings) -> Vec<Line> {
    let mut groups: Vec<(&str, Vec<(String, String)>)> = Vec::new();
    for (_, control) in controls.iter() {
        let label = bindings.label(&control.keys);
        if label.is_empty() {
            continue;
        }
        let row = (label, control.action.clone());
        match groups.iter_mut().find(|(name, _)| *name == control.group) {
            Some((_, rows)) => rows.push(row),
            None => groups.push((&control.group, vec![row])),
        }
    }
    let keys_width = groups.iter().flat_map(|(_, rows)| rows.iter().map(|(k, _)| k.chars().count())).max().unwrap_or(0).min(KEYS_WIDTH_CAP);
    let mut out = Vec::new();
    for (i, (group, rows)) in groups.into_iter().enumerate() {
        if i > 0 {
            out.push(Line::Blank);
        }
        out.push(Line::Heading(group.to_string()));
        for (keys, action) in rows {
            out.push(Line::Row(format!("{keys:<keys_width$}"), action));
        }
    }
    out
}

/// The cells a line takes.
fn width_of(line: &Line) -> usize {
    match line {
        Line::Heading(text) => text.chars().count(),
        Line::Row(keys, action) => keys.chars().count() + 2 + action.chars().count(),
        Line::Blank => 0,
    }
}

/// `lines` cut into columns of `inner.height`, and the columns into pages
/// of as many as fit `inner.width`.
///
/// A heading never ends a column, and no column opens on a blank: both
/// read as a slip of the hand.
fn paginate(lines: &[Line], inner: Rect) -> Vec<Vec<Vec<Line>>> {
    let height = inner.height.max(1) as usize;
    let column_width = lines.iter().map(width_of).max().unwrap_or(0) as i32;
    let per_page = ((inner.width + GUTTER) / (column_width + GUTTER).max(1)).max(1) as usize;
    let mut columns: Vec<Vec<Line>> = vec![Vec::new()];
    for line in lines {
        let column = columns.last_mut().expect("one column to start");
        let full = column.len() >= height;
        let orphaned = matches!(line, Line::Heading(_)) && column.len() + 2 > height;
        if full || orphaned {
            if *line == Line::Blank {
                continue;
            }
            columns.push(vec![line.clone()]);
        } else if !(column.is_empty() && *line == Line::Blank) {
            column.push(line.clone());
        }
    }
    for column in &mut columns {
        while column.last() == Some(&Line::Blank) {
            column.pop();
        }
    }
    columns.retain(|c| !c.is_empty());
    columns.chunks(per_page).map(|page| page.to_vec()).collect()
}

/// The cells inside the frame.
fn inner_of(rect: Rect) -> Rect {
    Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2)
}

/// Prints the hint that opens the screen, while nothing is open.
///
/// The row is cleared every frame either way, since a hint painted once
/// would stay under whatever screen opened over it.
pub fn draw_hint(mut terminal: ResMut<Terminal>, layout: Res<ControlsLayout>, keys: Res<ControlsKeys>, palette: Res<Palette>, modals: Res<Modals>) {
    let Some(rect) = layout.hint else { return };
    if rect.height < 1 {
        return;
    }
    let bg = palette.get(Tones::SURFACE);
    terminal.fill(rect, rl_render::Cell::new(' ', bg).on(bg));
    if modals.any_open() {
        return;
    }
    let text = format!("{} {}", keys.toggle.label(), layout.hint_word);
    let x = rect.right() - text.chars().count() as i32;
    terminal.print_on(x.max(rect.x), rect.y, &text, palette.get(Tones::MUTED), bg);
}

/// Paints the screen, while it is open.
pub fn draw_controls(
    mut terminal: ResMut<Terminal>,
    layout: Res<ControlsLayout>,
    keys: ControlInput,
    screen: Res<ControlsScreen>,
    palette: Res<Palette>,
    modals: Res<Modals>,
) {
    if !modals.is_open(controls_modal(&modals)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 12 || rect.height < 4 {
        return;
    }
    let binds = keys.bindings();
    let inner = inner_of(rect);
    let pages = paginate(&lines(keys.controls(), &binds), inner);
    let page = screen.page.min(pages.len().saturating_sub(1));
    let title = if pages.len() > 1 { format!("{} {}/{}", layout.title, page + 1, pages.len()) } else { layout.title.clone() };
    let hints = if pages.len() > 1 {
        format!("{} {} page \u{2022} {} close", key_name(binds.help.prev_page), key_name(binds.help.next_page), key_name(binds.help.close))
    } else {
        format!("{} close", key_name(binds.help.close))
    };
    clear(&mut terminal, rect, &palette);
    frame(&mut terminal, rect, &title, &hints, &palette);

    let bg = palette.get(Tones::SURFACE);
    let Some(columns) = pages.get(page) else {
        terminal.print_on(inner.x, inner.y, "Nothing is bound.", palette.get(Tones::MUTED), bg);
        return;
    };
    let column_width = columns.iter().flatten().map(width_of).max().unwrap_or(0) as i32;
    for (c, column) in columns.iter().enumerate() {
        let x = inner.x + c as i32 * (column_width + GUTTER);
        for (r, line) in column.iter().enumerate() {
            let y = inner.y + r as i32;
            match line {
                Line::Heading(text) => terminal.print_on(x, y, text, palette.get(Tones::TITLE), bg),
                Line::Row(keys, action) => {
                    terminal.print_on(x, y, keys, palette.get(Tones::NOTICE), bg);
                    let at = x + keys.chars().count() as i32 + 2;
                    terminal.print_on(at, y, &clip(action, (inner.right() - at).max(0) as usize), palette.get(Tones::TEXT), bg);
                }
                Line::Blank => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::{AddControls, Chord};
    use crate::harness::Stage;

    /// A screen over a game that declared its own walking and two keys,
    /// with the look cursor for an engine control.
    fn stage(rect: Rect) -> Stage {
        Stage::new_with((crate::InspectViewPlugin, ControlsPanel::new(rect).hint(Rect::new(0, 39, 40, 1))), |app| {
            app.add_control("Move", "walk", crate::controls::Keys::Directions { shift: false });
            app.add_control("Act", "pick up", [KeyCode::KeyG, KeyCode::Comma]);
            app.add_control("Act", "smother the brand", Chord::shift(KeyCode::KeyL));
        })
        .screen(60, 40)
    }

    fn open(stage: &mut Stage) {
        stage.chord(&[KeyCode::ShiftLeft, KeyCode::Slash]);
    }

    #[test]
    fn the_hint_names_the_key_that_opens_the_screen_and_goes_while_it_is_open() {
        let mut stage = stage(Rect::new(0, 0, 40, 30));
        assert_eq!(stage.row(39), format!("{:>40}", "? controls"), "right-aligned in its row");
        open(&mut stage);
        assert!(stage.app.world().resource::<Modals>().any_open());
        assert_eq!(stage.row(39), "", "the hint is not needed while the screen is up");
        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open());
        assert!(stage.row(39).ends_with("? controls"));
    }

    /// The row inside the frame that says `action`, its border and margin
    /// cut off, so an assertion reads what the player reads.
    fn row_for<'a>(rows: &'a [String], action: &str) -> &'a str {
        let row = rows.iter().find(|r| r.contains(action)).unwrap_or_else(|| panic!("no {action:?} in {rows:#?}"));
        row.strip_prefix("\u{2502} ").unwrap_or_else(|| panic!("inside the frame: {row:?}"))
    }

    #[test]
    fn the_screen_lists_the_games_keys_under_their_headings_and_the_engines_beside_them() {
        let mut stage = stage(Rect::new(0, 0, 50, 30));
        open(&mut stage);
        let rows = stage.rows();
        let find = |needle: &str| rows.iter().position(|r| r.contains(needle)).unwrap_or_else(|| panic!("no {needle:?} in {rows:#?}"));
        assert!(rows[0].contains(" Controls "), "one page: {:?}", rows[0]);
        assert!(find("Move") < find("walk"), "the heading first");
        assert!(row_for(&rows, "walk").starts_with("arrows hjklyubn numpad  walk"), "{:?}", row_for(&rows, "walk"));
        assert!(find("walk") < find("Act"), "then the next group");
        assert!(row_for(&rows, "pick up").starts_with("g ,"), "{:?}", row_for(&rows, "pick up"));
        assert!(row_for(&rows, "smother the brand").starts_with("L "), "{:?}", row_for(&rows, "smother the brand"));
        assert!(row_for(&rows, "look around").starts_with("x "), "the engine's own key, from its resource: {:?}", row_for(&rows, "look around"));
        assert!(row_for(&rows, "show the controls").starts_with("? "), "{:?}", row_for(&rows, "show the controls"));
        assert_eq!(rows.iter().filter(|r| r.contains("next thing in sight")).count(), 1, "declared by two plugins, listed once");
        assert!(rows.iter().all(|r| r.chars().count() <= 50), "nothing runs past the frame: {rows:#?}");
    }

    #[test]
    fn a_rebound_engine_key_is_listed_as_rebound() {
        let mut stage = stage(Rect::new(0, 0, 50, 30));
        stage.app.world_mut().resource_mut::<crate::CursorKeys>().look = KeyCode::KeyV;
        open(&mut stage);
        let rows = stage.rows();
        assert!(row_for(&rows, "look around").starts_with("v "), "{:?}", row_for(&rows, "look around"));
    }

    #[test]
    fn a_screen_too_short_for_one_column_flows_into_columns_and_then_pages() {
        // Sixteen lines of controls over ten rows of screen is two columns,
        // and at sixty cells wide only one column of forty fits a page.
        let mut narrow = stage(Rect::new(0, 0, 60, 12));
        open(&mut narrow);
        let first = narrow.rows();
        assert!(first[0].contains(" Controls 1/2 "), "{:?}", first[0]);
        assert!(first[11].contains("page"), "the hints say how to turn it: {:?}", first[11]);
        assert!(first.iter().any(|r| r.contains("walk")), "{first:#?}");
        narrow.press(KeyCode::ArrowRight);
        let second = narrow.rows();
        assert!(second[0].contains(" Controls 2/2 "), "{:?}", second[0]);
        assert!(second.iter().any(|r| r.contains("show the controls")), "{second:#?}");
        assert_ne!(first, second);
        narrow.press(KeyCode::ArrowRight);
        assert!(narrow.rows()[0].contains(" Controls 1/2 "), "round again");

        // Wide enough for both columns, it is one page, the game's groups
        // first however late the game declared them.
        let mut wide = stage(Rect::new(0, 0, 120, 12)).screen(120, 12);
        open(&mut wide);
        let rows = wide.rows();
        assert!(rows[0].contains(" Controls \u{2500}"), "{:?}", rows[0]);
        assert!(rows[1].starts_with("\u{2502} Move"), "{:?}", rows[1]);
        assert!(rows.iter().any(|r| r.contains("walk")) && rows.iter().any(|r| r.contains("show the controls")), "{rows:#?}");
    }

    #[test]
    fn a_heading_never_ends_a_column_and_no_column_opens_blank() {
        let lines =
            vec![Line::Heading("A".into()), Line::Row("a".into(), "one".into()), Line::Blank, Line::Heading("B".into()), Line::Row("b".into(), "two".into())];
        let pages = paginate(&lines, Rect::new(0, 0, 80, 3));
        let columns: Vec<Vec<Line>> = pages.into_iter().flatten().collect();
        assert_eq!(columns.len(), 2, "{columns:#?}");
        assert_eq!(columns[0], lines[..2], "the blank and B's heading moved on together, and the blank was dropped");
        assert_eq!(columns[1], lines[3..]);
        let pages = paginate(&lines, Rect::new(0, 0, 80, 4));
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].len(), 2, "still two columns: B and its row would not both fit under the blank");
        assert_eq!(paginate(&lines, Rect::new(0, 0, 80, 5)), vec![vec![lines.clone()]], "and at five rows it is one column");
    }
}
