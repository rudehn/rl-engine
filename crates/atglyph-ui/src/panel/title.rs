//! The title screen's menu: what a player picks before there is a run.
//!
//! The one screen before there is a world. [`TitlePanel`] holds the first
//! run back with [`HoldStart`] and offers what can be done about it:
//! continue the saved run, begin a new one, read the records, change the
//! settings, or leave. Every game with a title writes this menu, and a
//! second game would copy the first one's, so it is the engine's; what a
//! game keeps is its picture, painted in [`TitleSet::Backdrop`] under the
//! rows the panel draws in [`TitleSet::Menu`].
//!
//! It knows nothing of storage, which is `atglyph-save`'s, a crate this one may
//! not depend on. What the slot holds is [`SavedRun`], kept by whoever
//! keeps the slot, and a game with none keeps no slot, so its title has no
//! Continue to offer; Continue asks the game's start to restore it with [`ContinueRun`],
//! and New Game over a save gives it up with [`AbandonRun`], both through
//! [`begin_run`], the door every run begins by. A run left with
//! [`LeaveRun`] raises the hold again, and the menu with it.

use atglyph_bevy::prelude::*;
use atglyph_core::{Direction, GridRect};
use atglyph_render::{Cell, Terminal};
use bevy::prelude::*;

use crate::controls::ControlInput;
use crate::modal::{ModalId, Modals};
use crate::panel::records::records_modal;
use crate::panel::settings::{SettingsLayout, rows_drawn, settings_keys, settings_modal};
use crate::panel::{clear, clip};
use crate::tone::{Palette, Tones};

/// One row the title can draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleRow {
    /// The saved run, carried on: taken only over a run this build can
    /// read.
    Continue,
    /// A run begun afresh, asked about first over anything saved.
    NewGame,
    /// The records screen: drawn once a game added one, and taken once
    /// there is a record for it to show.
    Records,
    /// The settings screen, when it has a setting to list.
    Settings,
    /// Leave.
    Exit,
}

impl TitleRow {
    /// Every row, in the order they are drawn.
    pub(crate) const ALL: [TitleRow; 5] = [TitleRow::Continue, TitleRow::NewGame, TitleRow::Records, TitleRow::Settings, TitleRow::Exit];

    /// What the row says, in the game's `words`.
    fn label(self, words: &TitleWords) -> &str {
        match self {
            TitleRow::Continue => &words.continue_run,
            TitleRow::NewGame => &words.new_game,
            TitleRow::Records => &words.records,
            TitleRow::Settings => &words.settings,
            TitleRow::Exit => &words.exit,
        }
    }
}

/// What the title says: each row's words, and the question New Game asks
/// over a save with its two answers.
///
/// The game's to change with [`TitlePanel::words`], as [`GameMenuPanel`](crate::GameMenuPanel)'s
/// are: the rest of a title is the game's own, and words in another
/// voice than its picture's would read as the engine's.
#[derive(Clone, Debug)]
pub struct TitleWords {
    /// The row that carries the saved run on.
    pub continue_run: String,
    /// The row that begins a run afresh.
    pub new_game: String,
    /// The row that opens the records screen.
    pub records: String,
    /// The row that opens the settings screen.
    pub settings: String,
    /// The row that leaves.
    pub exit: String,
    /// The question New Game asks over a save.
    pub abandon: String,
    /// The answer that gives the save up for a new run.
    pub yes: String,
    /// The answer that keeps it, picked out when the question is asked.
    pub no: String,
}

impl Default for TitleWords {
    fn default() -> Self {
        Self {
            continue_run: "Continue".into(),
            new_game: "New Game".into(),
            records: "Records".into(),
            settings: "Settings".into(),
            exit: "Exit".into(),
            abandon: "Abandon the run in progress?".into(),
            yes: "Yes".into(),
            no: "No".into(),
        }
    }
}

/// The menu as it stands: the row picked out, the question while it is
/// asked, and whether a screen stands over it.
///
/// Refilled by `title_keys` once a frame while the title is up, before
/// anything draws, so the rows a key takes are the rows drawn.
#[derive(Resource, Debug, Default)]
pub struct TitleMenu {
    /// Which of [`rows`](Self::rows) is picked out.
    pub selected: usize,
    /// The question New Game asks over a save, while it is asked: `Some`,
    /// with whether Yes is picked out.
    pub confirm: Option<bool>,
    /// Whether a screen stands over the menu, as the keys last saw it: the
    /// keys are that screen's, and the menu is not drawn under it.
    pub covered: bool,
    /// Every row drawn, and whether it can be taken, as the keys last saw
    /// them.
    drawn: Vec<(TitleRow, bool)>,
}

impl TitleMenu {
    /// The rows that can be taken, in the order they are drawn, as the
    /// keys last saw them: [`selected`](Self::selected) counts these, so a
    /// row that cannot be taken is never the one picked out. Empty until
    /// the title's first frame.
    pub fn rows(&self) -> Vec<TitleRow> {
        self.drawn.iter().filter(|(_, takes)| *takes).map(|(row, _)| *row).collect()
    }

    /// Every row drawn, in order, and whether it can be taken now, as the
    /// keys last saw them, the rows drawn dim included, which
    /// [`rows`](Self::rows) leaves out. The tests' alone: the panel always
    /// draws its own rows, so no game draws the menu another way and needs
    /// the dim ones, and what a game reads is `rows` and `selected`.
    #[cfg(test)]
    pub(crate) fn drawn(&self) -> &[(TitleRow, bool)] {
        &self.drawn
    }
}

/// Where a title is drawn, in order: the game's picture, then the menu
/// over it.
///
/// Both run in `Update` only while [`title_is_up`], before the engine's
/// input phase: outside every set that waits on play, since the whole of a
/// title is the time before a run exists.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TitleSet {
    /// The game's picture, painted on a terminal cleared to the palette's
    /// surface just before, so a picture paints only what it draws and a
    /// game with none has a blank screen under its menu.
    Backdrop,
    /// The panel's rows, over the picture.
    Menu,
}

/// A run condition: whether the title is up, which is while the start is
/// held and no world stands.
pub fn title_is_up(hold: Option<Res<HoldStart>>, state: Res<State<EngineState>>) -> bool {
    hold.is_some() && *state.get() == EngineState::Idle
}

/// Where the menu is drawn and what it says.
#[derive(Resource, Debug, Clone)]
struct TitleLayout {
    /// The rectangle the rows are drawn in, from its top down.
    rect: GridRect,
    /// What the rows and the question say.
    words: TitleWords,
}

/// Where the game put the settings screen for a run, kept while the title
/// has it standing over its own menu instead.
#[derive(Resource, Default)]
struct SettingsAside(Option<GridRect>);

/// The title screen's menu, and the hold on the first run while it is up.
///
/// Inserts [`HoldStart`], so the engine begins no run at startup, and
/// [`TitleMenu`], whose presence is also how [`GameMenuPanel`](crate::GameMenuPanel)
/// knows to offer the way back here from a run. The keys are read ahead of
/// [`TitleSet::Backdrop`], the rows drawn in [`TitleSet::Menu`], and a
/// screen opened from a row, the settings or the records, draws over both.
/// Every run, the title's or a restart's, begins on a blank terminal, so
/// nothing of the picture outlasts the title.
pub struct TitlePanel(TitleLayout);

impl TitlePanel {
    /// The menu in `menu`: its rows drawn from the top down, centred
    /// across it, and the question New Game asks in their place.
    pub fn new(menu: GridRect) -> Self {
        Self(TitleLayout { rect: menu, words: TitleWords::default() })
    }

    /// The game's words for the rows and the question.
    pub fn words(mut self, words: TitleWords) -> Self {
        self.0.words = words;
        self
    }
}

impl Plugin for TitlePanel {
    fn build(&self, app: &mut App) {
        app.add_message::<AppExit>();
        app.insert_resource(self.0.clone()).init_resource::<TitleMenu>().init_resource::<SettingsAside>().init_resource::<HoldStart>();
        // Before the input phase, which holds the exclusive key handlers
        // that conflict with everything they are not ordered against, and
        // so before the frame's presenters and the screens drawn after
        // them, which stand over the title.
        app.configure_sets(Update, (TitleSet::Backdrop, TitleSet::Menu).chain().before(EngineSet::Input).run_if(title_is_up));
        // The keys after a screen's own, so the key that closes a screen
        // over the menu is that screen's and the menu is drawn again the
        // frame it closes; after the pause menu's, which is never up with
        // the title but writes the same stack and the same exit, so the two
        // are ordered even in a game with no settings screen between them;
        // and after the repeats are moved on, which every key is read
        // through here. Each with a condition of its own rather than one
        // for the three: a row that begins a run does so in a command
        // applied before the clearing, and a clearing after the run's start
        // would paint the title's surface over the blank terminal the run
        // begins on.
        app.add_systems(
            Update,
            (stand_settings_over_title, title_keys, clear_title_screen)
                .chain()
                .after(settings_keys)
                .after(crate::game_menu::menu_keys)
                .after(crate::controls::advance_repeats)
                .before(TitleSet::Backdrop)
                .distributive_run_if(title_is_up),
        )
        .add_systems(Update, draw_title_menu.in_set(TitleSet::Menu))
        .add_systems(PrepareRun, give_back_for_the_run);
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::UiPlugin>("TitlePanel");
    }
}

/// What decides which rows the title can offer: what the run's slot
/// holds, if the game keeps one, the settings declared, and the records
/// kept.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct TitleReads<'w> {
    saved: Option<Res<'w, SavedRun>>,
    settings: Option<Res<'w, Settings>>,
    records: Option<Res<'w, RunRecords>>,
}

impl TitleReads<'_> {
    /// Every row the title draws, and whether it can be taken now.
    ///
    /// A row is drawn when the game has what it opens, and taken only when
    /// there is something there: Continue when the game keeps a save
    /// slot, to say a run can be continued one day, and taken over a run
    /// this build can read; Records once a records screen was declared,
    /// and taken once there is a record; Settings when the settings screen
    /// has a setting to list. Whether a game keeps a slot or has anything
    /// to set no frame changes, so a row that could never be taken is not
    /// drawn at all.
    fn rows(&self, modals: &Modals) -> Vec<(TitleRow, bool)> {
        let records = records_modal(modals).map(|_| self.records.as_deref().is_some_and(|kept| !kept.records.is_empty()));
        let settings = crate::game_menu::settings_offered(modals, self.settings.as_deref()).is_some();
        TitleRow::ALL
            .into_iter()
            .filter_map(|row| match row {
                TitleRow::Continue => self.saved.as_deref().map(|saved| (row, *saved == SavedRun::Readable)),
                TitleRow::NewGame | TitleRow::Exit => Some((row, true)),
                TitleRow::Records => records.map(|kept| (row, kept)),
                TitleRow::Settings => settings.then_some((row, true)),
            })
            .collect()
    }

    /// Whether the slot holds a run, one this build can read or not, which
    /// a new run would replace: what New Game asks about first.
    fn holds_a_run(&self) -> bool {
        self.saved.as_deref().is_some_and(|saved| *saved != SavedRun::Empty)
    }
}

/// Reads the title's keys: the direction keys walk the rows that can be
/// taken, round from the last to the first; the confirm key takes the
/// picked row; the close key leaves. Read through
/// [`DirectionKeys`](crate::DirectionKeys) and [`CursorKeys`](crate::CursorKeys)
/// as every menu here is, so a key a game rebinds is rebound here too.
///
/// Refills [`TitleMenu`] first, whatever the keys do, so the drawing reads
/// the rows the keys read. An ordinary system rather than an exclusive one:
/// beginning a run runs the `NewRun` schedule, which wants the whole world,
/// so it is queued as a command, applied before anything draws.
pub(crate) fn title_keys(
    keys: ControlInput,
    reads: TitleReads,
    mut menu: ResMut<TitleMenu>,
    mut modals: ResMut<Modals>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    menu.drawn = reads.rows(&modals);
    // A screen over the menu has the keys, and the key that closed one
    // this frame is still down and was that screen's.
    menu.covered = modals.top().is_some();
    if menu.covered || modals.closing() {
        return;
    }
    let (input, bindings) = (keys.input(), keys.bindings());
    let direction = bindings.directions.just_pressed(input);
    let take = input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm);
    let close = input.just_pressed(bindings.cursor.close);
    // The question has the keys while it is asked: any direction moves
    // between its two answers, the close key is No, and No, taken, puts it
    // away with New Game still picked out.
    if let Some(yes) = menu.confirm {
        if direction.is_some() {
            menu.confirm = Some(!yes);
        } else if close || (take && !yes) {
            menu.confirm = None;
        } else if take {
            lower(&mut menu, &mut commands, abandon_and_begin);
        }
        return;
    }
    // Never empty: New Game and Exit can always be taken.
    let rows = menu.rows();
    let (count, picked) = (rows.len(), menu.selected.min(rows.len() - 1));
    menu.selected = match direction {
        Some(Direction::North) => (picked + count - 1) % count,
        Some(Direction::South) => (picked + 1) % count,
        _ => picked,
    };
    if close {
        exit.write(AppExit::Success);
        return;
    }
    if !take {
        return;
    }
    match rows[menu.selected] {
        TitleRow::Continue => {
            commands.insert_resource(ContinueRun);
            lower(&mut menu, &mut commands, begin_run);
        }
        // Over anything saved, even what this build cannot read, New Game
        // asks first: a run given up by a slip of the key is gone for good.
        TitleRow::NewGame if reads.holds_a_run() => menu.confirm = Some(false),
        TitleRow::NewGame => lower(&mut menu, &mut commands, begin_run),
        TitleRow::Records => {
            let screen = records_modal(&modals);
            cover(&mut menu, &mut modals, screen);
        }
        TitleRow::Settings => {
            let screen = settings_modal(&modals);
            cover(&mut menu, &mut modals, screen);
        }
        TitleRow::Exit => {
            exit.write(AppExit::Success);
        }
    }
}

/// Opens `screen` over the menu, which hands it the keys from this frame
/// on. The screen reads nothing more in the frame it opens, so the key
/// that took the row is not read again as one of its own.
fn cover(menu: &mut TitleMenu, modals: &mut Modals, screen: Option<ModalId>) {
    if let Some(screen) = screen {
        modals.open(screen);
        menu.covered = true;
    }
}

/// Puts the title down for a run that `begin` begins once the frame's
/// commands are applied. The menu forgets its place, so whenever the title
/// next comes up its first row is the one picked out, whatever the slot
/// then holds.
fn lower(menu: &mut TitleMenu, commands: &mut Commands, begin: fn(&mut World)) {
    menu.selected = 0;
    menu.confirm = None;
    commands.queue(begin);
}

/// Gives the saved run up and begins a new one, in one command: whoever
/// keeps the slot observes [`AbandonRun`] and forgets it before the trigger
/// returns, so the new run's start never sees the old run's save.
fn abandon_and_begin(world: &mut World) {
    world.trigger(AbandonRun);
    begin_run(world);
}

/// Stands the settings screen over the title's menu while the title is up,
/// keeping where the game put it for a run.
///
/// Where the game put it for a run is where a run's screens go, often
/// across the title's picture, and the player is looking at the menu.
/// Centred across the menu, as wide as the game made it, from the menu's
/// top, and risen only as far as its rows need to end on the terminal: it
/// does not scroll, so a row run past the terminal's foot would be a
/// setting the player could not reach from the title, and a screen risen
/// further would stand over the picture rather than the menu. Never
/// taller than the game made it. Done once each time the title comes up;
/// a run's beginning puts it back.
fn stand_settings_over_title(
    title: Res<TitleLayout>,
    layout: Option<ResMut<SettingsLayout>>,
    declared: Option<Res<Settings>>,
    terminal: Option<Res<Terminal>>,
    mut aside: ResMut<SettingsAside>,
) {
    let Some(mut layout) = layout else { return };
    if aside.0.is_some() {
        return;
    }
    let (menu, in_run) = (title.rect, layout.rect);
    aside.0 = Some(in_run);
    let Some(terminal) = terminal else {
        layout.rect = GridRect::new((menu.x + (menu.width - in_run.width) / 2).max(0), menu.y, in_run.width, in_run.height);
        return;
    };
    let (columns, rows) = (terminal.width(), terminal.height());
    let width = in_run.width.min(columns);
    let x = (menu.x + (menu.width - width) / 2).clamp(0, columns - width);
    // As tall as its rows, as the screen draws itself, never taller than
    // the game made it, and on the terminal from its top row to its foot.
    let tall = declared.as_deref().map_or(in_run.height, rows_drawn).min(in_run.height).min(rows);
    let y = menu.y.min(rows - tall).max(0);
    layout.rect = GridRect::new(x, y, width, in_run.height.min(rows - y));
}

/// Gives back what the title borrowed, as a run begins: the settings
/// screen goes where the game put it for a run, and the terminal is left
/// blank, as a launch with no title leaves it. A run's panels draw only
/// their own rectangles, and a cell none of them covers would show the
/// title's picture for the whole run.
///
/// In [`PrepareRun`], so however a run begins, from the title or a
/// restart, fresh or continued, it begins the same way.
fn give_back_for_the_run(mut aside: ResMut<SettingsAside>, settings: Option<ResMut<SettingsLayout>>, terminal: Option<ResMut<Terminal>>) {
    if let (Some(rect), Some(mut settings)) = (aside.0.take(), settings) {
        settings.rect = rect;
    }
    if let Some(mut terminal) = terminal {
        let whole = terminal.bounds();
        terminal.fill(whole, Cell::default());
    }
}

/// Clears the whole terminal to the palette's surface, ahead of the
/// picture.
///
/// The title has the window to itself, and a run left for it would leave
/// its last frame under a picture that paints less than the whole
/// terminal, or under a menu with no picture at all.
fn clear_title_screen(terminal: Option<ResMut<Terminal>>, palette: Res<Palette>) {
    let Some(mut terminal) = terminal else { return };
    let whole = terminal.bounds();
    clear(&mut terminal, whole, &palette);
}

/// Paints the menu over the picture: the rows as the keys last saw them,
/// or the question in their place while it is asked.
///
/// Nothing while a screen stands over the menu: the two are seldom the
/// same height, and a menu left drawn under one shows its last rows below
/// the other's frame.
fn draw_title_menu(terminal: Option<ResMut<Terminal>>, title: Res<TitleLayout>, menu: Res<TitleMenu>, palette: Res<Palette>) {
    let Some(mut terminal) = terminal else { return };
    if menu.covered {
        return;
    }
    match menu.confirm {
        Some(yes) => draw_question(&mut terminal, &title, yes, &palette),
        None => draw_rows(&mut terminal, &title, &menu, &palette),
    }
}

/// The rows from the top of the rectangle down, on one band as wide as the
/// widest word and a cell either side, centred across the rectangle, so
/// the words line up and nothing moves as the pick does.
///
/// The picked row is the band in the selection tone, as a picked row is in
/// every menu here; a row that cannot be taken is dim; the rest are drawn
/// over the picture, on whatever it painted under them.
fn draw_rows(terminal: &mut Terminal, title: &TitleLayout, menu: &TitleMenu, palette: &Palette) {
    let rect = title.rect;
    let picked = menu.rows().get(menu.selected).copied();
    let widest = menu.drawn.iter().map(|(row, _)| row.label(&title.words).chars().count()).max().unwrap_or(0);
    let band = (widest as i32 + 2).min(rect.width);
    let (x, room) = (rect.x + (rect.width - band) / 2, (band - 2).max(0) as usize);
    let (text, select) = (palette.get(Tones::TEXT), palette.get(Tones::SELECT));
    for (y, (row, takes)) in (rect.y..rect.bottom()).zip(&menu.drawn) {
        let label = clip(row.label(&title.words), room);
        if picked == Some(*row) {
            terminal.fill(GridRect::new(x, y, band, 1), Cell::new(' ', text).on(select));
            terminal.print_on(x + 1, y, &label, text, select);
        } else {
            terminal.print(x + 1, y, &label, palette.get(if *takes { Tones::TEXT } else { Tones::MUTED }));
        }
    }
}

/// The question New Game asks over a save, where the rows stood: the
/// question centred on the top row, and two rows down its two answers, No
/// first, each a cell wider either side, so the picked one reads as the
/// picked row of a menu does.
fn draw_question(terminal: &mut Terminal, title: &TitleLayout, yes: bool, palette: &Palette) {
    let (rect, words) = (title.rect, &title.words);
    let (text, select) = (palette.get(Tones::TEXT), palette.get(Tones::SELECT));
    let question = clip(&words.abandon, rect.width.max(0) as usize);
    terminal.print(rect.x + (rect.width - question.chars().count() as i32) / 2, rect.y, &question, text);
    let y = rect.y + 2;
    if y >= rect.bottom() {
        return;
    }
    let answers = [(&words.no, !yes), (&words.yes, yes)];
    // Each answer's band, and two cells between them.
    let width = answers.iter().map(|(word, _)| word.chars().count() as i32 + 2).sum::<i32>() + 2;
    let mut x = rect.x + (rect.width - width) / 2;
    for (word, picked) in answers {
        let cells = word.chars().count() as i32 + 2;
        if picked {
            terminal.fill(GridRect::new(x, y, cells, 1), Cell::new(' ', text).on(select));
            terminal.print_on(x + 1, y, word, text, select);
        } else {
            terminal.print(x + 1, y, word, text);
        }
        x += cells + 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::UiApp;
    use crate::panel::records::RECORDS_MODAL;
    use crate::panel::settings::SettingsPanel;
    use crate::tone::ToneId;
    use atglyph_bevy::ContinueSaved;
    use atglyph_bevy::testing::{KeyScript, press};
    use bevy::ecs::system::RunSystemOnce;

    /// What the tests watch a game do about its runs: the runs its start
    /// began and how many of those were asked to continue the saved one,
    /// the saved runs given up, and how many had been given up by the time
    /// the last run's start ran.
    #[derive(Resource, Default)]
    struct Seen {
        begun: u32,
        continued: u32,
        abandoned: u32,
        abandoned_before_the_start: u32,
    }

    /// A game's start, as much of it as a title can tell apart.
    fn start(mut seen: ResMut<Seen>) {
        seen.begun += 1;
        seen.abandoned_before_the_start = seen.abandoned;
    }

    /// The save crate's continuing, as much of it as a title can tell
    /// apart: a run begun, from the save, and the ask taken away.
    fn resume(mut commands: Commands, mut seen: ResMut<Seen>) {
        commands.remove_resource::<ContinueRun>();
        seen.begun += 1;
        seen.continued += 1;
    }

    /// Whoever keeps the save slot, as much of it as a title can tell.
    fn forget(_: On<AbandonRun>, mut seen: ResMut<Seen>) {
        seen.abandoned += 1;
    }

    /// An engine app with `panel` up over a slot holding `saved`, or with
    /// no slot at all, as in a game that saves nothing, a small terminal to
    /// draw on, `setup` run before the first frame, and that frame run.
    fn titled(panel: TitlePanel, saved: Option<SavedRun>, setup: impl FnOnce(&mut App)) -> App {
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((atglyph_bevy::testing::KeyScriptPlugin, crate::UiPlugin, panel));
        app.insert_resource(Terminal::new(40, 12, Vec2::ONE));
        if let Some(saved) = saved {
            app.insert_resource(saved);
        }
        app.init_resource::<Seen>().add_systems(NewRun, start).add_systems(ContinueSaved, resume).add_observer(forget);
        setup(&mut app);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    /// The title over the whole of the small terminal, over a slot holding
    /// `saved`.
    fn title_over(saved: SavedRun, setup: impl FnOnce(&mut App)) -> App {
        titled(TitlePanel::new(GridRect::new(0, 0, 40, 12)), Some(saved), setup)
    }

    /// A settings screen with one setting on it, `glow`, which is `Off`.
    fn with_settings(app: &mut App) {
        app.add_plugins(SettingsPanel::new(GridRect::new(0, 0, 40, 12)));
        app.add_setting(Setting::new("glow", "Display", "Glow", ["Off", "On"]));
    }

    fn glow(app: &App) -> String {
        let settings = app.world().resource::<Settings>();
        settings.choice(settings.find("glow").expect("declared")).to_string()
    }

    fn seen(app: &App) -> &Seen {
        app.world().resource::<Seen>()
    }

    fn menu(app: &App) -> &TitleMenu {
        app.world().resource::<TitleMenu>()
    }

    /// The row picked out.
    fn picked(app: &App) -> TitleRow {
        let (rows, picked) = (menu(app).rows(), menu(app).selected);
        *rows.get(picked).unwrap_or_else(|| panic!("row {picked} of {rows:?} is picked out"))
    }

    /// How many times the app was asked to exit since this was last asked.
    fn exits(app: &mut App) -> usize {
        app.world_mut().resource_mut::<Messages<AppExit>>().drain().count()
    }

    /// Every row of the terminal as text, trailing blanks trimmed.
    fn screen(app: &App) -> Vec<String> {
        let terminal = app.world().resource::<Terminal>();
        (0..terminal.height())
            .map(|y| (0..terminal.width()).map(|x| terminal.get(x, y).unwrap_or_default().glyph).collect::<String>().trim_end().to_string())
            .collect()
    }

    /// What the screen reads: every row with something on it, trimmed.
    fn drawn(app: &App) -> Vec<String> {
        screen(app).iter().map(|row| row.trim().to_string()).filter(|row| !row.is_empty()).collect()
    }

    /// The cell `text` begins in, on the first row that shows it.
    fn cell_of(app: &App, text: &str) -> Cell {
        let rows = screen(app);
        let y = rows.iter().position(|row| row.contains(text)).unwrap_or_else(|| panic!("{text:?} is drawn: {rows:#?}"));
        let x = rows[y][..rows[y].find(text).expect("on this row")].chars().count();
        app.world().resource::<Terminal>().get(x as i32, y as i32).expect("on the terminal")
    }

    fn tone(app: &App, tone: ToneId) -> Color {
        app.world().resource::<Palette>().get(tone)
    }

    #[test]
    fn the_title_holds_the_first_run_back() {
        let mut app = title_over(SavedRun::Empty, |_| {});
        app.update();
        assert_eq!(seen(&app).begun, 0, "the engine began no run at startup");
        assert!(app.world_mut().run_system_once(title_is_up).expect("the condition runs"), "and the title is up");
    }

    /// A game with no save slot, no `SavePlugin`, has nothing it could
    /// ever continue, and that cannot change while it runs: Continue is
    /// left out rather than drawn dim, as Settings is with nothing to set,
    /// and New Game has no save to ask about.
    #[test]
    fn a_game_that_keeps_no_save_slot_is_offered_no_continue() {
        let mut app = titled(TitlePanel::new(GridRect::new(0, 0, 40, 12)), None, |_| {});
        assert_eq!(drawn(&app), ["New Game", "Exit"], "no Continue row, dim or otherwise");
        assert_eq!(picked(&app), TitleRow::NewGame);
        press(&mut app, KeyCode::Enter);
        assert_eq!((menu(&app).confirm, seen(&app).begun), (None, 1), "New Game begins without asking");
    }

    /// A slot that holds nothing yet could hold a run once one is left for
    /// the title, so Continue is drawn dim, to say a run can be continued.
    #[test]
    fn with_an_empty_slot_the_title_picks_new_game_out_first_and_draws_continue_dim() {
        let app = title_over(SavedRun::Empty, |_| {});
        assert_eq!(menu(&app).rows(), [TitleRow::NewGame, TitleRow::Exit], "Continue cannot be taken");
        assert_eq!(picked(&app), TitleRow::NewGame, "New Game is picked out");
        assert_eq!(drawn(&app), ["Continue", "New Game", "Exit"], "Continue is drawn all the same, to say a run can be continued one day");
        assert_eq!(cell_of(&app, "Continue").fg, tone(&app, Tones::MUTED), "and drawn dim");
    }

    /// A game drawing the menu its own way reads what the panel draws:
    /// every row in order and whether it can be taken now, the dim ones
    /// included, which the takeable `rows` leave out.
    #[test]
    fn a_game_drawing_its_own_menu_reads_every_row_and_whether_it_can_be_taken() {
        let app = title_over(SavedRun::Empty, |_| {});
        assert_eq!(menu(&app).drawn(), [(TitleRow::Continue, false), (TitleRow::NewGame, true), (TitleRow::Exit, true)]);
    }

    #[test]
    fn with_a_readable_save_the_title_picks_continue_out_first() {
        let app = title_over(SavedRun::Readable, |_| {});
        assert_eq!(menu(&app).rows(), [TitleRow::Continue, TitleRow::NewGame, TitleRow::Exit]);
        assert_eq!(picked(&app), TitleRow::Continue);
        assert_eq!(cell_of(&app, "Continue").bg, tone(&app, Tones::SELECT), "and it is drawn picked out");
    }

    #[test]
    fn with_an_unreadable_save_continue_is_not_offered_and_new_game_still_asks() {
        let mut app = title_over(SavedRun::Unreadable, |_| {});
        assert_eq!(menu(&app).rows(), [TitleRow::NewGame, TitleRow::Exit], "nothing this build can read to continue");
        assert_eq!(cell_of(&app, "Continue").fg, tone(&app, Tones::MUTED));
        press(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).confirm, Some(false), "New Game asks before it replaces what it cannot read, with No picked out");
        assert_eq!((seen(&app).begun, seen(&app).abandoned), (0, 0), "and the key that asked gave nothing up and began nothing");
    }

    #[test]
    fn the_direction_keys_skip_a_row_that_cannot_be_taken_and_wrap() {
        let mut app = title_over(SavedRun::Empty, |app| {
            // A records screen with no record on it yet, so its row is
            // drawn and cannot be taken, and a settings screen that can.
            app.add_modal(RECORDS_MODAL);
            with_settings(app);
        });
        assert_eq!(drawn(&app), ["Continue", "New Game", "Records", "Settings", "Exit"]);
        let mut walked = vec![picked(&app)];
        for key in [KeyCode::ArrowDown, KeyCode::ArrowDown, KeyCode::ArrowDown, KeyCode::ArrowUp, KeyCode::ArrowUp, KeyCode::ArrowUp] {
            press(&mut app, key);
            walked.push(picked(&app));
        }
        let (new_game, settings, exit) = (TitleRow::NewGame, TitleRow::Settings, TitleRow::Exit);
        assert_eq!(walked, [new_game, settings, exit, new_game, exit, settings, new_game], "past Records both ways, and round past Continue");
    }

    #[test]
    fn the_title_reads_the_keys_a_game_rebound() {
        let mut app = title_over(SavedRun::Empty, |app| {
            app.insert_resource(crate::DirectionKeys::none().bind(KeyCode::KeyS, atglyph_core::Direction::South));
            app.insert_resource(crate::CursorKeys { confirm: KeyCode::KeyE, close: KeyCode::Backspace, ..crate::CursorKeys::default() });
        });
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(picked(&app), TitleRow::NewGame, "an arrow the game unbound moves nothing");
        press(&mut app, KeyCode::KeyS);
        assert_eq!(picked(&app), TitleRow::Exit, "the key it bound does");
        press(&mut app, KeyCode::Escape);
        assert_eq!(exits(&mut app), 0, "a close key the game replaced leaves nothing");
        press(&mut app, KeyCode::KeyE);
        assert_eq!(exits(&mut app), 1, "and its own confirm key takes the row");
    }

    #[test]
    fn continue_inserts_continue_run_and_begins_the_run() {
        let mut app = title_over(SavedRun::Readable, |_| {});
        press(&mut app, KeyCode::Enter);
        assert_eq!((seen(&app).begun, seen(&app).continued), (1, 1), "one run begun, the saved one continued");
        assert!(!app.world().contains_resource::<HoldStart>(), "and the hold lowered, so the title is down");
    }

    #[test]
    fn new_game_with_nothing_saved_begins_the_run_without_asking() {
        let mut app = title_over(SavedRun::Empty, |_| {});
        assert_eq!(seen(&app).begun, 0, "held until asked");
        press(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).confirm, None, "nothing to give up, so nothing to ask");
        assert_eq!((seen(&app).begun, seen(&app).continued, seen(&app).abandoned), (1, 0, 0), "one fresh run");
    }

    /// A run's panels draw only their own rectangles, so a cell none of
    /// them covers, a gutter between two, would show the title's picture
    /// for the whole run; the run begins on the terminal a launch without a
    /// title begins on.
    #[test]
    fn a_run_begun_from_the_title_begins_on_a_blank_terminal() {
        let mut app = title_over(SavedRun::Empty, |_| {});
        assert!(drawn(&app).contains(&"New Game".to_string()), "the title is drawn");
        press(&mut app, KeyCode::Enter);
        assert_eq!(seen(&app).begun, 1);
        let terminal = app.world().resource::<Terminal>();
        let blank = terminal.bounds().cells().all(|at| terminal.get(at.x, at.y) == Some(Cell::default()));
        assert!(blank, "nothing of the title is left: {:#?}", screen(&app));
    }

    #[test]
    fn new_game_over_a_save_asks_with_no_picked_out_and_no_puts_the_question_away() {
        let mut app = title_over(SavedRun::Readable, |_| {});
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).confirm, Some(false), "it asks, with No picked out");
        assert_eq!(drawn(&app), ["Abandon the run in progress?", "No    Yes"], "in place of the rows");
        assert_eq!(cell_of(&app, "No").bg, tone(&app, Tones::SELECT), "No drawn picked out");
        press(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).confirm, None, "No puts the question away");
        assert_eq!(picked(&app), TitleRow::NewGame, "with New Game still picked out");
        assert_eq!(drawn(&app), ["Continue", "New Game", "Exit"], "and the rows back");
        assert_eq!((seen(&app).begun, seen(&app).abandoned), (0, 0), "the save neither given up nor left");
    }

    #[test]
    fn the_close_key_answers_the_question_no_and_does_not_exit() {
        let mut app = title_over(SavedRun::Readable, |_| {});
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).confirm, Some(false), "asked");
        press(&mut app, KeyCode::Escape);
        assert_eq!(menu(&app).confirm, None, "the close key is No");
        assert_eq!(exits(&mut app), 0, "and not the title's own close key, which exits");
        assert_eq!(seen(&app).abandoned, 0);
    }

    #[test]
    fn yes_triggers_abandon_and_begins_one_run() {
        let mut app = title_over(SavedRun::Readable, |_| {});
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(menu(&app).confirm, Some(true), "a direction moves to Yes");
        press(&mut app, KeyCode::Enter);
        let after = seen(&app);
        assert_eq!((after.abandoned, after.begun, after.continued), (1, 1, 0), "the save given up, and one fresh run begun");
        assert_eq!(after.abandoned_before_the_start, 1, "given up before the new run's start ran, so the start never saw it");
        press(&mut app, KeyCode::Enter);
        assert_eq!(seen(&app).begun, 1, "the title is down, and its keys begin nothing more");
    }

    #[test]
    fn the_key_that_opens_settings_is_not_read_again_by_the_settings_screen() {
        let mut app = title_over(SavedRun::Empty, with_settings);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(picked(&app), TitleRow::Settings);
        press(&mut app, KeyCode::Enter);
        let modals = app.world().resource::<Modals>();
        assert!(settings_modal(modals).is_some_and(|screen| modals.is_top(screen)), "Enter opened the settings screen");
        assert_eq!(glow(&app), "Off", "and changed nothing on it");
    }

    /// One frame of `key`, read by the title's keys and the settings
    /// screen's in the order given.
    fn frame(app: &mut App, key: KeyCode, title_first: bool) {
        app.world_mut().resource_mut::<Modals>().begin_frame();
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
        let world = app.world_mut();
        if title_first {
            world.run_system_once(title_keys).expect("the title's keys run");
            world.run_system_once(settings_keys).expect("the screen's keys run");
        } else {
            world.run_system_once(settings_keys).expect("the screen's keys run");
            world.run_system_once(title_keys).expect("the title's keys run");
        }
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.clear();
    }

    /// The title's keys and a screen's are two systems, so the key that
    /// opens the screen must not also change its first row, and the key
    /// that closes it must not also be the title's close key, which exits,
    /// whichever of the two reads first.
    #[test]
    fn the_title_and_the_settings_screen_share_no_key_whichever_reads_first() {
        for (open_title_first, close_title_first) in [(true, true), (true, false), (false, true), (false, false)] {
            let orders = format!("opened with the title first: {open_title_first}, closed with it first: {close_title_first}");
            let mut app = title_over(SavedRun::Empty, with_settings);
            app.world_mut().resource_mut::<TitleMenu>().selected = 1;
            assert_eq!(picked(&app), TitleRow::Settings, "{orders}");

            frame(&mut app, KeyCode::Enter, open_title_first);
            let modals = app.world().resource::<Modals>();
            assert!(settings_modal(modals).is_some_and(|screen| modals.is_top(screen)), "{orders}");
            assert_eq!(glow(&app), "Off", "{orders}");

            frame(&mut app, KeyCode::Escape, close_title_first);
            assert_eq!(app.world().resource::<Modals>().top(), None, "the close key put the screen away: {orders}");
            assert_eq!(exits(&mut app), 0, "{orders}");
            assert!(app.world_mut().run_system_once(title_is_up).expect("the condition runs"), "{orders}");
        }
    }

    #[test]
    fn a_screen_over_the_title_has_its_keys_until_it_closes_and_its_close_key_does_not_exit() {
        let mut app = title_over(SavedRun::Empty, with_settings);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        assert_eq!(picked(&app), TitleRow::Settings, "the screen took the direction, not the title");
        assert_eq!(glow(&app), "On", "and the confirm key");
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<Modals>().any_open(), "the close key put the screen away");
        assert_eq!(exits(&mut app), 0, "and did not exit, as it does from the title itself");
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(picked(&app), TitleRow::Exit, "the title has its keys back");
    }

    /// A records screen with a record to show: the row can be taken, and
    /// it opens that screen over the menu as Settings opens its own.
    #[test]
    fn with_a_record_kept_the_records_row_opens_the_records_screen_over_the_menu() {
        let mut app = title_over(SavedRun::Empty, |app| {
            app.add_modal(RECORDS_MODAL);
            app.insert_resource(RunRecords { records: vec![RunRecord { score: 12, ..RunRecord::default() }], this_run: None });
        });
        assert_eq!(menu(&app).rows(), [TitleRow::NewGame, TitleRow::Records, TitleRow::Exit]);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(picked(&app), TitleRow::Records);
        press(&mut app, KeyCode::Enter);
        let modals = app.world().resource::<Modals>();
        assert!(records_modal(modals).is_some_and(|screen| modals.is_top(screen)), "Enter opened the records screen");
        assert!(menu(&app).covered, "which stands over the menu");
        assert_eq!(seen(&app).begun, 0, "and began nothing");
    }

    #[test]
    fn close_on_the_title_exits() {
        let mut app = title_over(SavedRun::Empty, |_| {});
        press(&mut app, KeyCode::Escape);
        assert_eq!(exits(&mut app), 1);
    }

    #[test]
    fn the_exit_row_exits() {
        let mut app = title_over(SavedRun::Empty, |_| {});
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(picked(&app), TitleRow::Exit);
        press(&mut app, KeyCode::Enter);
        assert_eq!(exits(&mut app), 1);
    }

    #[test]
    fn the_settings_screen_stands_over_the_menu_while_the_title_is_up_and_goes_back_when_a_run_begins() {
        let in_run = GridRect::new(28, 6, 44, 13);
        let mut app = titled(TitlePanel::new(GridRect::new(30, 30, 40, 8)), Some(SavedRun::Empty), |app| {
            app.insert_resource(Terminal::new(100, 40, Vec2::ONE));
            app.add_plugins(SettingsPanel::new(in_run));
            app.add_setting(Setting::new("glow", "Display", "Glow", ["Off", "On"]));
        });
        // Centred on the menu, from its top, as wide as the game made it,
        // and stopping at the terminal's foot rather than running past it:
        // its four rows fit under the menu's top, so it has no need to rise.
        let over_the_menu = GridRect::new(28, 30, 44, 10);
        assert_eq!(app.world().resource::<SettingsLayout>().rect, over_the_menu);
        press(&mut app, KeyCode::Enter);
        assert_eq!(seen(&app).begun, 1, "New Game began a run");
        assert_eq!(app.world().resource::<SettingsLayout>().rect, in_run, "and the screen is back where the game put it for one");
        // A run left for the title holds the start again, and the screen
        // goes back over the menu with it.
        app.world_mut().insert_resource(HoldStart);
        app.update();
        assert_eq!(app.world().resource::<SettingsLayout>().rect, over_the_menu);
    }

    /// A settings screen taller than the room under the menu rises until
    /// its last row is on the terminal: it does not scroll, so a row cut at
    /// the terminal's foot would be a setting the player could not reach
    /// from the title. Nine settings under one heading, in a frame, are
    /// twelve rows, two more than the ten from the menu's top down.
    #[test]
    fn a_settings_screen_too_tall_for_the_room_under_the_menu_rises_to_show_every_row() {
        let in_run = GridRect::new(28, 6, 44, 13);
        let mut app = titled(TitlePanel::new(GridRect::new(30, 30, 40, 8)), Some(SavedRun::Empty), |app| {
            app.insert_resource(Terminal::new(100, 40, Vec2::ONE));
            app.add_plugins(SettingsPanel::new(in_run));
            for n in 1..=9 {
                app.add_setting(Setting::new(format!("s{n}"), "Display", format!("Setting {n}"), ["Off", "On"]));
            }
        });
        assert_eq!(app.world().resource::<SettingsLayout>().rect, GridRect::new(28, 28, 44, 12), "two rows up, and as tall as its rows");
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        let rows = screen(&app);
        assert!(rows[38].contains("Setting 9"), "the last setting on the last row inside the frame: {rows:#?}");
        assert!(rows[39].trim_start().starts_with('\u{2514}'), "and the frame closed on the terminal's last row: {rows:#?}");
    }

    /// The words line up on one column, so they do not shift as the pick
    /// moves, and the band the picked one is drawn on is as wide as the
    /// widest word and a cell either side.
    #[test]
    fn the_rows_line_up_centred_across_the_menu_in_the_games_own_words() {
        let words = TitleWords { new_game: "Descend".into(), ..TitleWords::default() };
        let app = titled(TitlePanel::new(GridRect::new(0, 0, 40, 12)).words(words), Some(SavedRun::Empty), |_| {});
        let rows = screen(&app);
        // The widest word is eight cells, so the band is ten, from column
        // fifteen, and every word starts at sixteen.
        assert_eq!(rows[..4], ["                Continue", "                Descend", "                Exit", ""]);
        let terminal = app.world().resource::<Terminal>();
        let select = tone(&app, Tones::SELECT);
        assert!((15..25).all(|x| terminal.get(x, 1).expect("on the terminal").bg == select), "the picked row's band");
        assert!(terminal.get(14, 1).expect("on the terminal").bg != select && terminal.get(25, 1).expect("on the terminal").bg != select, "and no wider");
    }

    #[test]
    fn the_menu_is_not_drawn_while_a_screen_stands_over_it_and_is_back_the_frame_that_screen_closes() {
        let mut app = title_over(SavedRun::Empty, with_settings);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        let rows = screen(&app).join("\n");
        assert!(rows.contains("Glow"), "the settings screen is drawn:\n{rows}");
        assert!(!rows.contains("New Game") && !rows.contains("Exit"), "and the menu is not, under it:\n{rows}");
        app.world_mut().resource_mut::<KeyScript>().press(KeyCode::Escape);
        app.update();
        let rows = screen(&app).join("\n");
        assert!(rows.contains("New Game") && !rows.contains("Glow"), "the frame the screen closes, the menu is back:\n{rows}");
    }
}
