//! The log, all of it, on a screen you can scroll and filter.
//!
//! The strip along the bottom of the map shows the last four lines because
//! that is all there is room for. Everything before them is still in the
//! [`MessageLog`], and a player who looked away for a fight wants to read
//! back. So: a second presenter over the same view.
//!
//! That is the whole point of the split made concrete. [`LogPanel`] and
//! this draw the same resource two ways, and neither knows the other
//! exists. What this one adds is state of its own - where the cursor is
//! and what it is filtered to - and that lives in [`Scrollback`], not in
//! the log.
//!
//! Lines are wrapped rather than clipped here. On the strip a cut line is
//! a cut line; on a screen you opened to read, losing the end of a
//! sentence is worse than spending a second row on it.
//!
//! [`LogPanel`]: crate::panel::LogPanel

use crate::controls::{AddControls, EngineKey};
use crate::modal::AddModal;
use bevy::prelude::*;
use rl_bevy::{EngineSet, PresentSet};
use rl_core::Rect;
use rl_render::Terminal;

use crate::log::MessageLog;
use crate::modal::{ModalId, Modals};
use crate::panel::{clear, frame, wrap};
use crate::tone::{Palette, ToneId, Tones};

/// The name the scrollback's modal is declared under.
pub const SCROLLBACK_MODAL: &str = "scrollback";

/// Where the reader is in the history, and what they are reading.
///
/// `offset` counts lines up from the newest, so zero is the bottom and
/// opening the screen always shows what just happened.
#[derive(Resource, Debug, Default)]
pub struct Scrollback {
    /// Lines scrolled up from the newest.
    pub offset: usize,
    /// Show only this tone. `None` shows everything.
    pub filter: Option<ToneId>,
}

/// Keys the scrollback answers to.
#[derive(Resource, Debug, Clone)]
pub struct ScrollbackKeys {
    /// Opens and closes the screen.
    pub toggle: KeyCode,
    /// Closes it.
    pub close: KeyCode,
    /// One line back.
    pub up: KeyCode,
    /// One line on.
    pub down: KeyCode,
    /// A screenful back.
    pub page_up: KeyCode,
    /// A screenful on.
    pub page_down: KeyCode,
    /// Cycles the tone filter through the tones the log actually holds.
    pub filter: KeyCode,
}

impl Default for ScrollbackKeys {
    fn default() -> Self {
        Self {
            toggle: KeyCode::KeyP,
            close: KeyCode::Escape,
            up: KeyCode::ArrowUp,
            down: KeyCode::ArrowDown,
            page_up: KeyCode::PageUp,
            page_down: KeyCode::PageDown,
            filter: KeyCode::Tab,
        }
    }
}

/// Where the scrollback is drawn and what its borders say.
#[derive(Resource, Debug, Clone)]
pub struct ScrollbackLayout {
    /// The terminal cells it occupies, border included.
    pub rect: Rect,
    /// The title in the top border. The filter's name is appended to it
    /// while one is on, so the reader can see why lines are missing.
    pub title: String,
    /// The key hints in the bottom border.
    pub hints: String,
    /// Shown when the log, or the filter, has nothing to show.
    pub empty: String,
    /// Whether to rule off each turn with its number.
    pub turn_rules: bool,
}

/// Draws [`MessageLog`] as a scrollable screen.
///
/// Initialises the log if nothing else has, so a game may add this
/// without [`LogPanel`](crate::panel::LogPanel).
pub struct ScrollbackPanel(ScrollbackLayout);

impl ScrollbackPanel {
    /// A scrollback in `rect`.
    pub fn new(rect: Rect) -> Self {
        Self(ScrollbackLayout {
            rect,
            title: "Messages".into(),
            hints: "\u{2191}\u{2193} pgup/pgdn \u{2022} tab filter \u{2022} esc close".into(),
            empty: "Nothing has happened yet.".into(),
            turn_rules: true,
        })
    }

    /// Sets the title in the top border.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Sets the key hints in the bottom border.
    pub fn hints(mut self, hints: impl Into<String>) -> Self {
        self.0.hints = hints.into();
        self
    }

    /// Sets what is shown when there is nothing to show.
    pub fn empty(mut self, empty: impl Into<String>) -> Self {
        self.0.empty = empty.into();
        self
    }

    /// Leaves the per-turn rules out, for a game whose log does not read
    /// as a sequence of turns.
    pub fn without_turn_rules(mut self) -> Self {
        self.0.turn_rules = false;
        self
    }
}

impl Plugin for ScrollbackPanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<Scrollback>().init_resource::<ScrollbackKeys>().insert_resource(self.0.clone());
        app.add_modal(SCROLLBACK_MODAL);
        app.add_systems(Update, scrollback_keys.in_set(EngineSet::Input)).add_systems(Update, draw_scrollback.in_set(PresentSet::Overlay));
    }

    fn finish(&self, app: &mut App) {
        // Declared once the game has declared its own, so the controls
        // screen lists the game's groups first.
        app.add_control("Log", "read the whole log", EngineKey::OpenLog);
        app.add_control("Log", "scroll it", EngineKey::ScrollLog);
        app.add_control("Log", "filter it by tone", EngineKey::FilterLog);
    }
}

/// The id of the scrollback's modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`ScrollbackPanel`] was not added.
pub fn scrollback_modal(modals: &Modals) -> ModalId {
    modals.get(SCROLLBACK_MODAL).expect("ScrollbackPanel declares the scrollback modal")
}

/// One drawn row: either a message or the rule that starts a turn.
enum Line {
    /// A turn boundary, drawn as a rule carrying the number.
    Rule(u32),
    /// One wrapped line of one entry.
    Text(String, ToneId),
}

/// Opens, closes, scrolls and filters.
pub fn scrollback_keys(
    keys: Res<ButtonInput<KeyCode>>,
    binds: Res<ScrollbackKeys>,
    layout: Res<ScrollbackLayout>,
    log: Res<MessageLog>,
    mut scrollback: ResMut<Scrollback>,
    mut modals: ResMut<Modals>,
) {
    let modal = scrollback_modal(&modals);
    if keys.just_pressed(binds.toggle) && (modals.is_top(modal) || !modals.any_open()) {
        modals.toggle(modal);
        // Always open at the bottom: what just happened is what the
        // reader came for.
        scrollback.offset = 0;
        return;
    }
    if !modals.is_top(modal) {
        return;
    }
    if keys.just_pressed(binds.close) {
        modals.close_one(modal);
        return;
    }
    let page = (layout.rect.height - 2).max(1) as usize;
    if keys.just_pressed(binds.filter) {
        scrollback.filter = next_filter(&log, scrollback.filter);
        scrollback.offset = 0;
        return;
    }
    let lines = lines_of(&log, &scrollback, layout.turn_rules, (layout.rect.width - 4).max(1) as usize).len();
    // Never past the oldest line, and never below the newest: scrolling
    // into blank space reads as a bug, not as the end of the log.
    let furthest = lines.saturating_sub(page);
    let step = |offset: usize, by: usize, back: bool| if back { (offset + by).min(furthest) } else { offset.saturating_sub(by) };
    if keys.just_pressed(binds.up) {
        scrollback.offset = step(scrollback.offset, 1, true);
    }
    if keys.just_pressed(binds.down) {
        scrollback.offset = step(scrollback.offset, 1, false);
    }
    if keys.just_pressed(binds.page_up) {
        scrollback.offset = step(scrollback.offset, page, true);
    }
    if keys.just_pressed(binds.page_down) {
        scrollback.offset = step(scrollback.offset, page, false);
    }
}

/// The next tone to filter to, cycling through only the tones the log
/// holds and back to showing everything.
///
/// Only the tones present, because cycling through nine roles a game
/// never logs in is a key nobody presses twice.
fn next_filter(log: &MessageLog, current: Option<ToneId>) -> Option<ToneId> {
    let mut present: Vec<ToneId> = Vec::new();
    for entry in log.iter() {
        if !present.contains(&entry.tone) {
            present.push(entry.tone);
        }
    }
    present.sort_by_key(|t| t.raw());
    match current {
        None => present.first().copied(),
        Some(tone) => match present.iter().position(|t| *t == tone) {
            Some(i) if i + 1 < present.len() => Some(present[i + 1]),
            _ => None,
        },
    }
}

/// Every drawn line, oldest first, wrapped to `width` and filtered.
fn lines_of(log: &MessageLog, scrollback: &Scrollback, turn_rules: bool, width: usize) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut last_turn = None;
    for entry in log.iter() {
        if scrollback.filter.is_some_and(|t| t != entry.tone) {
            continue;
        }
        if turn_rules && last_turn != Some(entry.turn) {
            lines.push(Line::Rule(entry.turn));
            last_turn = Some(entry.turn);
        }
        for line in wrap(&entry.display(), width) {
            lines.push(Line::Text(line, entry.tone));
        }
    }
    lines
}

/// Paints the scrollback, while it is open.
pub fn draw_scrollback(
    mut terminal: ResMut<Terminal>,
    layout: Res<ScrollbackLayout>,
    log: Res<MessageLog>,
    scrollback: Res<Scrollback>,
    tones: Res<Tones>,
    palette: Res<Palette>,
    modals: Res<Modals>,
) {
    if !modals.is_open(scrollback_modal(&modals)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 8 || rect.height < 4 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    let title = match scrollback.filter {
        Some(tone) => format!("{} \u{2022} {}", layout.title, tones.name(tone)),
        None => layout.title.clone(),
    };
    frame(&mut terminal, rect, &title, &layout.hints, &palette);

    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let bg = palette.get(Tones::SURFACE);
    let width = inner.width.max(0) as usize;
    let lines = lines_of(&log, &scrollback, layout.turn_rules, width);
    if lines.is_empty() {
        terminal.print_on(inner.x, inner.y, &layout.empty, palette.get(Tones::MUTED), bg);
        return;
    }
    let page = inner.height.max(1) as usize;
    // The window ends `offset` lines above the newest, so the bottom row
    // is the newest line when the offset is zero.
    let end = lines.len().saturating_sub(scrollback.offset);
    let start = end.saturating_sub(page);
    for (row, line) in lines[start..end].iter().enumerate() {
        let y = inner.y + row as i32;
        match line {
            Line::Rule(turn) => {
                let label = format!("turn {turn} ");
                let used = label.chars().count() as i32;
                terminal.print_on(inner.x, y, &label, palette.get(Tones::MUTED), bg);
                for x in inner.x + used..inner.right() {
                    terminal.put(x, y, '\u{2500}', palette.get(Tones::FRAME));
                }
            }
            Line::Text(text, tone) => terminal.print_on(inner.x, y, text, palette.get(*tone), bg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_render::Terminal;

    /// A stage with a scrollback and `n` numbered lines already logged.
    fn stage(n: u32) -> Stage {
        let mut stage = Stage::new(ScrollbackPanel::new(Rect::new(0, 0, 40, 8)).without_turn_rules()).screen(40, 8);
        for i in 0..n {
            stage.app.world_mut().resource_mut::<MessageLog>().info(format!("line {i}"), 0);
        }
        stage.tick();
        stage
    }

    fn open(stage: &mut Stage) {
        stage.press(ScrollbackKeys::default().toggle);
    }

    /// The rows inside the border, blank ones dropped.
    ///
    /// The panel draws its content two cells in from the frame, so the
    /// assertions below read the same slice the drawing writes.
    fn body(stage: &Stage) -> Vec<String> {
        let width = stage.app.world().resource::<Terminal>().width() as usize;
        stage.rows()[1..stage.rows().len() - 1]
            .iter()
            .map(|row| {
                let cells: Vec<char> = format!("{row:<width$}").chars().collect();
                cells[2..width - 2].iter().collect::<String>().trim_end().to_string()
            })
            .filter(|r| !r.is_empty())
            .collect()
    }

    #[test]
    fn it_draws_nothing_until_it_is_opened_and_opens_at_the_newest_line() {
        let mut stage = stage(20);
        assert!(stage.rows().iter().all(|r| r.is_empty()), "a closed screen draws nothing");
        open(&mut stage);
        let rows = body(&stage);
        assert_eq!(rows.last().unwrap(), "line 19", "the bottom row is the newest line");
        assert_eq!(rows.first().unwrap(), "line 14", "six rows of it");
        assert!(stage.row(0).contains(" Messages "), "titled: {:?}", stage.row(0));
    }

    #[test]
    fn scrolling_up_walks_back_and_stops_at_the_oldest_line() {
        let mut stage = stage(20);
        open(&mut stage);
        stage.press(ScrollbackKeys::default().up);
        assert_eq!(body(&stage).last().unwrap(), "line 18");
        stage.press(ScrollbackKeys::default().page_up);
        assert_eq!(body(&stage).last().unwrap(), "line 12", "a page is the six rows it draws");
        for _ in 0..10 {
            stage.press(ScrollbackKeys::default().page_up);
        }
        assert_eq!(body(&stage).first().unwrap(), "line 0", "and never past the oldest");
        assert_eq!(stage.app.world().resource::<Scrollback>().offset, 14);
    }

    #[test]
    fn scrolling_down_stops_at_the_newest_rather_than_into_blank_space() {
        let mut stage = stage(20);
        open(&mut stage);
        stage.press(ScrollbackKeys::default().page_up);
        for _ in 0..10 {
            stage.press(ScrollbackKeys::default().page_down);
        }
        assert_eq!(stage.app.world().resource::<Scrollback>().offset, 0);
        assert_eq!(body(&stage).last().unwrap(), "line 19");
    }

    #[test]
    fn the_filter_cycles_only_the_tones_the_log_holds_and_back_to_all() {
        let mut stage = Stage::new(ScrollbackPanel::new(Rect::new(0, 0, 40, 8)).without_turn_rules()).screen(40, 8);
        {
            let mut log = stage.app.world_mut().resource_mut::<MessageLog>();
            log.info("a plain line", 0);
            log.bad("a bad line", 0);
            log.info("another plain line", 0);
        }
        stage.tick();
        open(&mut stage);
        assert_eq!(body(&stage).len(), 3, "everything at first");

        stage.press(ScrollbackKeys::default().filter);
        assert_eq!(stage.app.world().resource::<Scrollback>().filter, Some(Tones::TEXT));
        assert_eq!(body(&stage), vec!["a plain line", "another plain line"]);
        assert!(stage.row(0).contains("text"), "the title says why lines are missing: {:?}", stage.row(0));

        stage.press(ScrollbackKeys::default().filter);
        assert_eq!(body(&stage), vec!["a bad line"], "notice is never offered: nothing logged it");

        stage.press(ScrollbackKeys::default().filter);
        assert_eq!(stage.app.world().resource::<Scrollback>().filter, None, "and back to all of it");
        assert_eq!(body(&stage).len(), 3);
    }

    #[test]
    fn a_line_too_wide_for_the_screen_is_wrapped_and_not_cut() {
        let mut stage = Stage::new(ScrollbackPanel::new(Rect::new(0, 0, 24, 8)).without_turn_rules()).screen(24, 8);
        stage.app.world_mut().resource_mut::<MessageLog>().info("the cutthroat runs you through with a rusty blade", 0);
        stage.tick();
        open(&mut stage);
        let rows = body(&stage);
        assert!(rows.len() > 1, "wrapped onto more than one row: {rows:?}");
        assert!(rows.join(" ").contains("rusty blade"), "and nothing was lost: {rows:?}");
        assert!(rows.iter().all(|r| r.chars().count() <= 20), "inside the border: {rows:?}");
    }

    #[test]
    fn each_turn_is_ruled_off_with_its_number() {
        let mut stage = Stage::new(ScrollbackPanel::new(Rect::new(0, 0, 40, 8))).screen(40, 8);
        {
            let mut log = stage.app.world_mut().resource_mut::<MessageLog>();
            log.info("you step east", 4);
            log.bad("the crab nips you", 5);
        }
        stage.tick();
        open(&mut stage);
        let rows = body(&stage);
        assert_eq!(rows[0], "turn 4 ".to_string() + &"\u{2500}".repeat(29));
        assert_eq!(rows[1], "you step east");
        assert!(rows[2].starts_with("turn 5 \u{2500}"), "a new turn rules off: {rows:?}");
        assert_eq!(rows[3], "the crab nips you");
    }

    #[test]
    fn an_empty_log_says_so_rather_than_drawing_a_blank_box() {
        let mut stage = stage(0);
        open(&mut stage);
        assert_eq!(body(&stage), vec!["Nothing has happened yet."]);
    }
}
