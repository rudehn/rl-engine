//! The records screen: every run that has ended, best score first, and
//! what the game said about the one picked out.
//!
//! A modal the engine draws end to end, in two parts that share one
//! screen rather than two modals on the stack: the list, best score
//! first, and the detail view over the record confirmed from it, showing
//! the run's seed and turn and the sections [`RecordsPlugin`] filed for
//! it, as the ending screen shows them. Confirm opens the detail; close
//! goes back a level from it, and closes the screen from the list.
//! `RecordsScreen` is which of the two is showing, not two resources,
//! because only one is ever on screen at once.
//!
//! The list is drawn in whatever rectangle the game gave it and scrolled
//! to keep the row picked out in view, the way [`ScrollbackPanel`](crate::panel::ScrollbackPanel)
//! scrolls the log: a list a hundred runs deep must not grow the box to
//! fit it. The detail view scrolls the same way, over sections a game may
//! have written at any length.
//!
//! Declares the `records` modal, so [`TitlePanel`](crate::TitlePanel)
//! offers a row for it once there is a record to show; see `title.rs` for
//! how it is opened. Reads [`RunRecords`] as optional data: a game that
//! added this screen but never added [`RecordsPlugin`] sees an empty list
//! rather than a panic, the same as a game that has not recorded a run
//! yet.

use atglyph_bevy::prelude::*;
use atglyph_core::{Direction, GridRect};
use atglyph_render::{Cell, Terminal};
use bevy::prelude::*;

use crate::app::UiApp;
use crate::controls::{ControlInput, key_name};
use crate::game_menu::section_lines;
use crate::modal::{ModalId, Modals};
use crate::panel::{clear, clip, frame};
use crate::tone::{Palette, ToneId, Tones};

/// The name the records screen's modal is declared under. [`TitlePanel`](crate::TitlePanel)
/// looks for a modal of this same name to decide whether to offer a
/// Records row at all.
pub(crate) const RECORDS_MODAL: &str = "records";

/// Where the screen is drawn and what it says.
#[derive(Resource, Debug, Clone)]
pub(crate) struct RecordsLayout {
    /// The terminal cells it occupies, border included. Fixed rather than
    /// shrunk to the records there are: the list can run to a hundred
    /// entries, so it scrolls inside this rectangle rather than growing to
    /// swallow the screen.
    pub rect: GridRect,
    /// Drawn in the top border while the list is showing.
    pub title: String,
    /// Shown in place of the list when there are no records yet.
    pub empty: String,
}

/// Which row the list's cursor is on, whether the detail view over it is
/// open, and how far that view is scrolled.
///
/// The screen's own state, since a view holds no cursor. One resource for
/// both halves of the screen rather than two, because only one is ever
/// drawn: `open` says which.
#[derive(Resource, Debug, Default)]
pub(crate) struct RecordsScreen {
    /// The row picked out in the list, indexing [`RunRecords::records`]
    /// best score first. Clamped to the records there are when the keys
    /// or the drawing read it.
    pub selected: usize,
    /// Whether the picked record's sections are shown in place of the
    /// list.
    pub open: bool,
    /// Lines scrolled down the detail view, while `open`. Forgotten when
    /// the detail view closes, so it always opens at the top.
    pub scroll: usize,
}

/// The records screen: [`RunRecords`] read and drawn, nothing owned.
///
/// Declares the `records` modal; a game adds this once, over the title or
/// anywhere else it offers a way to read the list.
pub struct RecordsPanel(RecordsLayout);

impl RecordsPanel {
    /// The screen in `rect`.
    pub fn new(rect: GridRect) -> Self {
        Self(RecordsLayout { rect, title: "Records".into(), empty: "No runs recorded yet.".into() })
    }

    /// Sets the top border's title over the list.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Sets what is shown in place of the list when there are no records.
    pub fn empty(mut self, empty: impl Into<String>) -> Self {
        self.0.empty = empty.into();
        self
    }
}

impl Plugin for RecordsPanel {
    fn build(&self, app: &mut App) {
        app.add_modal(RECORDS_MODAL);
        app.insert_resource(self.0.clone()).init_resource::<RecordsScreen>();
        // Before the input phase and outside the engine's sets, as
        // `SettingsPanel`'s keys are: this screen is read from the title,
        // before a world exists for those sets to gate on.
        app.add_systems(Update, records_keys.before(EngineSet::Input)).add_systems(Update, draw_records.after(EngineSet::Present));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::UiPlugin>("RecordsPanel");
    }
}

/// The id of the records screen's modal, when [`RecordsPanel`] was added.
///
/// `None` rather than a panic, the way [`settings_modal`](crate::panel::settings::settings_modal)
/// reads: a title or a menu offers the row only when this is `Some`.
pub fn records_modal(modals: &Modals) -> Option<ModalId> {
    modals.get(RECORDS_MODAL)
}

/// Opens, closes and walks the list; opens and scrolls the detail view
/// over the record confirmed from it, and closes it back to the list.
///
/// Guards itself exactly as `settings_keys` does rather than by system
/// ordering against whoever opens this screen: a frame that opens the
/// modal this same frame is never read again here, whether this system
/// ran before the open (and so saw nothing on top yet) or after it (and so
/// sees [`Modals::just_opened`]). Either way the key that opened the
/// screen is never also its first move.
pub(crate) fn records_keys(
    keys: ControlInput,
    records: Option<Res<RunRecords>>,
    layout: Res<RecordsLayout>,
    mut screen: ResMut<RecordsScreen>,
    mut modals: ResMut<Modals>,
) {
    let Some(modal) = records_modal(&modals) else { return };
    let top = modals.is_top(modal);
    if top && modals.just_opened() {
        screen.selected = 0;
        screen.open = false;
        screen.scroll = 0;
        return;
    }
    if !top || modals.closing() {
        return;
    }
    let (input, bindings) = (keys.input(), keys.bindings());
    let direction = bindings.directions.just_pressed(input);
    let close = input.just_pressed(bindings.cursor.close);
    let confirm = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    let count = records.as_deref().map_or(0, |kept| kept.records.len());

    if screen.open {
        if let Some(record) = records.as_deref().and_then(|kept| kept.records.get(screen.selected.min(count.saturating_sub(1)))) {
            let width = (layout.rect.width - 4).max(0) as usize;
            let page = (layout.rect.height - 2).max(1) as usize;
            let furthest = detail_lines(record, width).len().saturating_sub(page);
            match direction {
                Some(Direction::North) => screen.scroll = screen.scroll.saturating_sub(1),
                Some(Direction::South) => screen.scroll = (screen.scroll + 1).min(furthest),
                _ => {}
            }
        }
        if close {
            screen.open = false;
            screen.scroll = 0;
        }
        return;
    }

    if count == 0 {
        if close {
            modals.close_one(modal);
        }
        return;
    }
    let picked = screen.selected.min(count - 1);
    screen.selected = match direction {
        Some(Direction::North) => (picked + count - 1) % count,
        Some(Direction::South) => (picked + 1) % count,
        _ => picked,
    };
    if close {
        modals.close_one(modal);
        return;
    }
    if confirm {
        screen.open = true;
        screen.scroll = 0;
    }
}

/// What the screen is drawn from.
#[derive(bevy::ecs::system::SystemParam)]
struct Screen<'w> {
    layout: Res<'w, RecordsLayout>,
    screen: Res<'w, RecordsScreen>,
    modals: Res<'w, Modals>,
    records: Option<Res<'w, RunRecords>>,
    keys: ControlInput<'w>,
    palette: Res<'w, Palette>,
}

/// Paints the screen while its modal is open: the list, or the picked
/// record's sections in its place.
///
/// `terminal` is optional, as `SettingsPanel`'s is: this screen is usable
/// from a title with no window, where there is nothing to draw on.
fn draw_records(terminal: Option<ResMut<Terminal>>, screen: Screen) {
    let Screen { layout, screen, modals, records, keys, palette } = &screen;
    let Some(mut terminal) = terminal else { return };
    if !records_modal(modals).is_some_and(|modal| modals.is_open(modal)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 16 || rect.height < 4 {
        return;
    }
    clear(&mut terminal, rect, palette);
    let bindings = keys.bindings();
    let close = key_name(bindings.cursor.close);
    let kept: &[RunRecord] = match records.as_deref() {
        Some(kept) => &kept.records,
        None => &[],
    };
    if screen.open
        && let Some(record) = kept.get(screen.selected.min(kept.len().saturating_sub(1)))
    {
        let hints = format!("\u{2191}\u{2193} scroll \u{2022} {close} back");
        draw_detail(&mut terminal, rect, record, screen.scroll, &hints, palette);
        return;
    }
    let confirm = key_name(bindings.cursor.confirm);
    let hints = if kept.is_empty() { close } else { format!("\u{2191}\u{2193} pick \u{2022} {confirm} open \u{2022} {close} close") };
    draw_list(&mut terminal, rect, layout, kept, screen.selected, &hints, palette);
}

/// The list, best score first: each row clipped to the rectangle, scrolled
/// to keep the picked row in view.
fn draw_list(terminal: &mut Terminal, rect: GridRect, layout: &RecordsLayout, records: &[RunRecord], picked: usize, hints: &str, palette: &Palette) {
    frame(terminal, rect, &layout.title, hints, palette);
    let inner = GridRect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let width = inner.width.max(0) as usize;
    if records.is_empty() {
        terminal.print_on(inner.x, inner.y, &clip(&layout.empty, width), palette.get(Tones::MUTED), surface);
        return;
    }
    let list_rows = inner.height.max(1) as usize;
    let picked = picked.min(records.len() - 1);
    let first = picked.saturating_sub(list_rows.saturating_sub(1)).min(records.len().saturating_sub(list_rows));
    let text = palette.get(Tones::TEXT);
    for (i, record) in records.iter().enumerate().skip(first).take(list_rows) {
        let y = inner.y + (i - first) as i32;
        let bg = if i == picked { palette.get(Tones::SELECT) } else { surface };
        terminal.fill(GridRect::new(inner.x, y, inner.width, 1), Cell::new(' ', text).on(bg));
        terminal.print_on(inner.x, y, &clip(&row_text(i + 1, record), width), text, bg);
    }
}

/// The detail view's lines for `record`, `width` wide: the seed and the
/// turn the run ended on, as the ending screen says them under the run's
/// summary, then each of its sections under its heading as the ending
/// screen draws them.
///
/// The seed and the turn are the engine's half of the record, which a
/// game's own sections leave out so the ending screen does not say them
/// twice. One list for the keys and the drawing both, so the view scrolls
/// exactly as far as there is to read.
fn detail_lines(record: &RunRecord, width: usize) -> Vec<(String, ToneId)> {
    let mut lines = vec![(clip(&format!("Seed {}, turn {}.", record.seed, record.turn), width), Tones::MUTED)];
    lines.extend(section_lines(record.sections.iter().map(|s| (s.heading.as_str(), s.body.as_str())), width));
    lines
}

/// The picked record in full under its summary, the frame's title:
/// [`detail_lines`], scrolled by `scroll` lines once they run past the
/// rectangle.
fn draw_detail(terminal: &mut Terminal, rect: GridRect, record: &RunRecord, scroll: usize, hints: &str, palette: &Palette) {
    frame(terminal, rect, &record.summary, hints, palette);
    let inner = GridRect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let width = inner.width.max(0) as usize;
    let lines = detail_lines(record, width);
    let page = inner.height.max(1) as usize;
    for (row, (line, tone)) in lines.iter().skip(scroll).take(page).enumerate() {
        terminal.print_on(inner.x, inner.y + row as i32, line, palette.get(*tone), surface);
    }
}

/// One row of the list: its rank, its score, the game's summary and the
/// date it ended, as `rank. score  summary  date`, with no date when the
/// record does not carry one (recorded on wasm, or by a build before this
/// field existed).
fn row_text(rank: usize, record: &RunRecord) -> String {
    let mut text = format!("{rank}. {}  {}", record.score, record.summary);
    if let Some(when) = record.when {
        text.push_str("  ");
        text.push_str(&civil_date(when));
    }
    text
}

/// The civil, proleptic-Gregorian date `seconds` since the Unix epoch
/// falls on, as `YYYY-MM-DD`.
///
/// Howard Hinnant's days-from-civil arithmetic
/// (<http://howardhinnant.github.io/date_algorithms.html>): a calculation
/// this small does not earn a calendar dependency of its own.
fn civil_date(seconds: u64) -> String {
    let z = (seconds / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_bevy::testing::press;
    use atglyph_render::Terminal;

    fn record(score: i64, summary: &str) -> RunRecord {
        RunRecord { score, summary: summary.into(), ..RunRecord::default() }
    }

    /// An app with [`RecordsPanel`] added and its screen already open over
    /// `records`.
    fn opened(records: RunRecords) -> App {
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((atglyph_bevy::testing::KeyScriptPlugin, crate::UiPlugin, RecordsPanel::new(GridRect::new(0, 0, 50, 10))));
        app.insert_resource(Terminal::new(50, 10, Vec2::ONE));
        app.insert_resource(records);
        app.finish();
        app.cleanup();
        app.update();
        let modal = records_modal(app.world().resource::<Modals>()).expect("RecordsPanel declares it");
        app.world_mut().resource_mut::<Modals>().open(modal);
        app.update();
        app
    }

    fn rows(app: &App) -> Vec<String> {
        let t = app.world().resource::<Terminal>();
        (0..t.height()).map(|y| (0..t.width()).map(|x| t.get(x, y).unwrap_or_default().glyph).collect::<String>().trim_end().to_string()).collect()
    }

    fn screen(app: &App) -> &RecordsScreen {
        app.world().resource::<RecordsScreen>()
    }

    #[test]
    fn the_records_screen_lists_records_best_first_with_their_scores() {
        let dated = RunRecord { when: Some(1_704_067_200), ..record(100, "Won on the tenth floor") };
        let plain = record(50, "Died on floor three");
        let app = opened(RunRecords { records: vec![dated.clone(), plain.clone()], this_run: None });
        let text = rows(&app).join("\n");
        assert_eq!(row_text(1, &dated), "1. 100  Won on the tenth floor  2024-01-01");
        assert_eq!(row_text(2, &plain), "2. 50  Died on floor three");
        assert!(text.contains(&row_text(1, &dated)), "{text}");
        assert!(text.contains(&row_text(2, &plain)), "{text}");
    }

    #[test]
    fn confirm_opens_the_record_picked_out_and_close_goes_back_to_the_list() {
        let mut app = opened(RunRecords { records: vec![record(100, "Won on the tenth floor"), record(50, "Died on floor three")], this_run: None });
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(screen(&app).selected, 1);
        press(&mut app, KeyCode::Enter);
        assert!(screen(&app).open, "confirm opened the detail view");
        assert!(rows(&app).join("\n").contains("Died on floor three"), "the record picked out, not the first");
        press(&mut app, KeyCode::Escape);
        assert!(!screen(&app).open, "close went back to the list");
        assert!(app.world().resource::<Modals>().any_open(), "the screen itself is still open");
    }

    /// The detail view says the run's seed and the turn it ended on first
    /// under its summary, as the ending screen does, and the game's own
    /// sections after them: a game's record leaves both to the engine.
    #[test]
    fn the_detail_view_says_the_seed_and_the_turn_under_the_summary() {
        let score = RecordSection { heading: "Score".into(), body: "100".into() };
        let kept = RunRecord { seed: 77, turn: 900, sections: vec![score], ..record(100, "Won on the tenth floor") };
        let mut app = opened(RunRecords { records: vec![kept], this_run: None });
        press(&mut app, KeyCode::Enter);
        let rows = rows(&app);
        assert!(rows[0].contains("Won on the tenth floor"), "the summary in the frame's title: {rows:#?}");
        assert!(rows[1].contains("Seed 77, turn 900."), "the seed and the turn first under it: {rows:#?}");
        assert!(rows[3].contains("Score") && rows[4].contains("100"), "then the record's own sections: {rows:#?}");
    }

    /// A record longer than the screen scrolls down to its last line and
    /// no further: the keys count the seed's line with the sections, as
    /// the drawing does.
    #[test]
    fn a_record_longer_than_the_screen_scrolls_to_its_last_line_and_stops() {
        let body = (1..=20).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
        let long = RunRecord { sections: vec![RecordSection { heading: "The run".into(), body }], ..record(10, "Died on floor three") };
        let mut app = opened(RunRecords { records: vec![long], this_run: None });
        press(&mut app, KeyCode::Enter);
        for _ in 0..30 {
            press(&mut app, KeyCode::ArrowDown);
        }
        let rows = rows(&app);
        assert!(rows[8].contains("line 20"), "the last line on the last row: {rows:#?}");
        // The seed, a blank, the heading and twenty lines, eight to a page.
        assert_eq!(screen(&app).scroll, 23 - 8, "and scrolled no further than that");
    }

    #[test]
    fn close_on_the_list_closes_the_screen() {
        let mut app = opened(RunRecords { records: vec![record(10, "Won on the tenth floor")], this_run: None });
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<Modals>().any_open(), "close on the list closes the whole screen");
    }

    #[test]
    fn with_no_records_the_screen_says_so() {
        let app = opened(RunRecords::default());
        assert!(rows(&app).join("\n").contains("No runs recorded yet."));
    }

    #[test]
    fn the_title_offers_records_once_there_is_one() {
        let rect = GridRect::new(0, 0, 40, 12);
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((atglyph_bevy::testing::KeyScriptPlugin, crate::UiPlugin, crate::TitlePanel::new(rect), RecordsPanel::new(rect)));
        app.insert_resource(Terminal::new(40, 12, Vec2::ONE));
        app.insert_resource(atglyph_bevy::SavedRun::Empty);
        app.finish();
        app.cleanup();
        app.update();

        let menu = |app: &App| app.world().resource::<crate::TitleMenu>().rows();
        assert_eq!(menu(&app), vec![crate::TitleRow::NewGame, crate::TitleRow::Exit], "nothing kept yet, so Records cannot be taken");

        app.insert_resource(RunRecords { records: vec![record(12, "Won on the tenth floor")], this_run: None });
        app.update();
        assert_eq!(menu(&app), vec![crate::TitleRow::NewGame, crate::TitleRow::Records, crate::TitleRow::Exit], "the row can be taken now");

        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        assert!(!app.world().resource::<RecordsScreen>().open, "the Enter that opened the screen from the title did not also confirm its first row");
        let modals = app.world().resource::<Modals>();
        assert!(records_modal(modals).is_some_and(|screen| modals.is_top(screen)), "Enter opened the real records screen");
        assert!(rows(&app).join("\n").contains("Won on the tenth floor"), "showing the kept record");
    }

    #[test]
    fn epoch_seconds_read_as_the_civil_date_they_fall_on() {
        assert_eq!(civil_date(0), "1970-01-01", "the epoch itself");
        assert_eq!(civil_date(1_704_067_200), "2024-01-01");
    }
}
