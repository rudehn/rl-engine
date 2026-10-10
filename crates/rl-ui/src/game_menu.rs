//! The menu: pause, a new run, the same run again, quit, and the screen a
//! run ends on.
//!
//! Two screens that are one. While playing, the menu key opens it over the
//! map with a way back into the run. When the run is over, the engine
//! opens it itself with no way back, under the game's own words for the
//! ending, the seed and the turn. Both offer a new run on a fresh seed,
//! the same seed again, and quitting, and each of those is one message:
//! [`Restart`], or `AppExit`. Nothing here knows how a game starts; the
//! game's start runs again in [`NewRun`] the way it ran the first time.
//!
//! A game with a title, a [`TitlePanel`](crate::TitlePanel), is offered
//! the way back to it as well, one more message, [`LeaveRun`]: the run is
//! torn down as a restart tears it down, and the title raised in place of
//! another run. A game that saves writes a run still in play down first,
//! so Continue picks it up where it was left; a run that is over is only
//! left.
//!
//! The ending screen shows the game's own words as well as the engine's:
//! the run's own line, the summary of the record filed for it, under the
//! title, and whatever sections are on [`EndingView`], the record's and
//! any a game pushed in `ViewSet::Annotate`, each under its heading,
//! clipped to the rectangle the game gave the menu.
//!
//! It used to write them to a file instead, through a `Morgue` in
//! `rl-save`, composed and filed by this presenter. Nothing read the file
//! in play, a presenter has no business doing file I/O, and it was the one
//! reason `rl-ui` depended on `rl-save` at all.

use crate::modal::AddModal;
use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_bevy::{Ending, Outcome, Restart, world_is_shown};
use rl_core::{Direction, Rect};
use rl_render::{Cell, Terminal};

use crate::controls::{AddControls, ControlInput, EngineKey, key_name};
use crate::modal::{ModalId, Modals};
use crate::panel::title::TitleMenu;
use crate::panel::{clear, clip, frame, wrap};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::{EndingView, SheetView};

/// The name the menu's modal is declared under.
pub const GAME_MENU_MODAL: &str = "menu";

/// The key that opens and closes the menu while playing.
#[derive(Resource, Debug, Clone)]
pub struct MenuKeys {
    /// Opens the menu with nothing else open, and closes it again.
    pub toggle: KeyCode,
}

impl Default for MenuKeys {
    fn default() -> Self {
        Self { toggle: KeyCode::Escape }
    }
}

/// Where the menu is drawn and what it says.
#[derive(Resource, Debug, Clone)]
pub struct MenuLayout {
    /// The most of the terminal it may occupy; it is drawn from the
    /// top-left down only as far as its rows need.
    pub rect: Rect,
    /// The top border while playing.
    pub title: String,
    /// The top border when the run ended in death.
    pub died: String,
    /// When it ended in victory.
    pub won: String,
    /// When it was given up.
    pub abandoned: String,
}

/// Which row the cursor is on.
#[derive(Resource, Debug, Default)]
pub struct GameMenu {
    /// The row picked out.
    pub selected: usize,
}

/// One thing the menu offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuItem {
    /// Back into the run.
    Resume,
    /// A new run on a fresh seed.
    NewRun,
    /// The same run again.
    SameSeed,
    /// The settings screen, when a game has one with something on it.
    Settings,
    /// Back to the title, in a game with one: the run is left with
    /// [`LeaveRun`], written down first by a game that saves while it is
    /// still in play, and the title raised in its place.
    MainMenu,
    /// Leave.
    Quit,
}

impl MenuItem {
    /// What is offered while `playing`, or once the run is over, with the
    /// settings screen when there is one with a row on it, and the way back
    /// to the `title` when the game has one.
    pub fn offered(playing: bool, settings: bool, title: bool) -> Vec<MenuItem> {
        let mut items = Vec::new();
        if playing {
            items.push(MenuItem::Resume);
        }
        items.extend([MenuItem::NewRun, MenuItem::SameSeed]);
        if settings {
            items.push(MenuItem::Settings);
        }
        if title {
            items.push(MenuItem::MainMenu);
        }
        items.push(MenuItem::Quit);
        items
    }

    fn label(self) -> &'static str {
        match self {
            MenuItem::Resume => "Back to the run",
            MenuItem::NewRun => "A new run",
            MenuItem::SameSeed => "This seed again",
            MenuItem::Settings => "Settings",
            MenuItem::MainMenu => "Main menu",
            MenuItem::Quit => "Quit",
        }
    }
}

/// The menu, and the screen a run ends on.
///
/// Declares the `menu` modal and adds [`EndingViewPlugin`](crate::EndingViewPlugin),
/// so a game has somewhere to push what it wants the ending screen to say.
pub struct GameMenuPanel(MenuLayout);

impl GameMenuPanel {
    /// The menu in `rect`, from its top-left corner down only as far as
    /// its rows need: the rectangle is the most it may take, so one size
    /// serves both the short pause menu and the ending's longer screen.
    pub fn new(rect: Rect) -> Self {
        Self(MenuLayout { rect, title: "Menu".into(), died: "You died.".into(), won: "You have won.".into(), abandoned: "The run is over.".into() })
    }

    /// The top border while playing.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// The top border over a death.
    pub fn died(mut self, words: impl Into<String>) -> Self {
        self.0.died = words.into();
        self
    }

    /// The top border over a victory.
    pub fn won(mut self, words: impl Into<String>) -> Self {
        self.0.won = words.into();
        self
    }
}

impl Plugin for GameMenuPanel {
    fn build(&self, app: &mut App) {
        app.add_modal(GAME_MENU_MODAL);
        app.add_message::<Restart>().add_message::<LeaveRun>().add_message::<AppExit>();
        app.insert_resource(self.0.clone()).init_resource::<GameMenu>().init_resource::<MenuKeys>();
        // Before the game's input and outside the engine's sets, since the
        // sets do not run once the run is over and the menu must.
        app.add_plugins(crate::view::EndingViewPlugin);
        app.add_systems(Update, menu_keys.before(EngineSet::Input).run_if(world_is_shown))
            .add_systems(Update, draw_game_menu.in_set(PresentSet::Overlay))
            .add_systems(OnEnter(EngineState::Over), run_ended);
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "GameMenuPanel");
        app.add_control(crate::focus::SCREENS_GROUP, "open the menu", EngineKey::OpenMenu);
    }
}

/// The id of the menu's modal, for a game gating its own systems.
///
/// # Panics
/// Panics if [`GameMenuPanel`] was not added.
pub fn game_menu_modal(modals: &Modals) -> ModalId {
    modals.get(GAME_MENU_MODAL).expect("GameMenuPanel declares the menu modal")
}

/// The settings screen's modal, when a menu should offer it: the panel was
/// added, and something is declared for it to list. A screen with no rows
/// is not offered, which is what a build on the web gets. The title's
/// Settings row asks the same.
pub(crate) fn settings_offered(modals: &Modals, settings: Option<&Settings>) -> Option<ModalId> {
    crate::panel::settings::settings_modal(modals).filter(|_| settings.is_some_and(|s| !s.is_empty()))
}

/// What the menu writes when a row is chosen.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Choices<'w> {
    restarts: MessageWriter<'w, Restart>,
    leaves: MessageWriter<'w, LeaveRun>,
    exits: MessageWriter<'w, AppExit>,
}

/// What decides whether the menu opens: its key, and what is picked out in
/// the nearby list when a game has one, which the close key lets go of
/// before it opens the menu. And what decides the rows it offers once it
/// is open: a settings screen with a row on it, and a title to go back to.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Opening<'w> {
    binds: Res<'w, MenuKeys>,
    focus: Option<Res<'w, crate::focus::Focus>>,
    sighted: Option<Res<'w, crate::focus::Sighted>>,
    settings: Option<Res<'w, Settings>>,
    title_menu: Option<Res<'w, TitleMenu>>,
}

impl Opening<'_> {
    /// Whether something in sight is picked out.
    fn picked_out(&self) -> bool {
        self.focus.as_deref().zip(self.sighted.as_deref()).is_some_and(|(focus, sighted)| focus.within(sighted.list()).is_some())
    }
}

/// Opens, closes, walks and chooses.
///
/// With nothing open and something in sight picked out, the close key is
/// the nearby list's, which lets go of it: a player who pressed Tab to look
/// down the list backs out of it the way they back out of a cursor, and a
/// second press, with nothing picked out, opens the menu.
pub fn menu_keys(
    keys: ControlInput,
    opening: Opening,
    state: Res<State<EngineState>>,
    seed: Option<Res<Seed>>,
    mut menu: ResMut<GameMenu>,
    mut modals: ResMut<Modals>,
    mut choices: Choices,
) {
    let modal = game_menu_modal(&modals);
    let playing = *state.get() == EngineState::Playing;
    let input = keys.input();
    let letting_go = !modals.any_open() && input.just_pressed(keys.bindings().cursor.close) && opening.picked_out();
    if playing && input.just_pressed(opening.binds.toggle) && (modals.is_top(modal) || !modals.any_open()) && !letting_go {
        modals.toggle(modal);
        menu.selected = 0;
        return;
    }
    if !modals.is_top(modal) {
        return;
    }
    // A screen that closed over the menu this frame did so on a key that
    // is still down; it is not the menu's.
    if modals.closing() {
        return;
    }
    let settings = settings_offered(&modals, opening.settings.as_deref());
    let items = MenuItem::offered(playing, settings.is_some(), opening.title_menu.is_some());
    let bindings = keys.bindings();
    match bindings.directions.just_pressed(input) {
        Some(Direction::North) => menu.selected = (menu.selected + items.len() - 1) % items.len(),
        Some(Direction::South) => menu.selected = (menu.selected + 1) % items.len(),
        _ => {}
    }
    if !(input.just_pressed(bindings.cursor.confirm) || input.just_pressed(bindings.cursor.also_confirm)) {
        return;
    }
    match items[menu.selected.min(items.len() - 1)] {
        MenuItem::Resume => modals.close_one(modal),
        MenuItem::NewRun => {
            choices.restarts.write(Restart::fresh());
        }
        MenuItem::SameSeed => {
            choices.restarts.write(Restart { seed: seed.map(|s| s.0) });
        }
        MenuItem::Settings => {
            if let Some(settings) = settings {
                modals.open(settings);
            }
        }
        MenuItem::MainMenu => {
            choices.leaves.write(LeaveRun);
        }
        MenuItem::Quit => {
            choices.exits.write(AppExit::Success);
        }
    }
}

/// Opens the menu over the ending, with no way back into the run.
///
/// Composing and filing an obituary used to happen here too. It does not
/// any more: what a game wants said goes on [`EndingView`] and is drawn,
/// which is a presenter's business, and nothing is written to disk.
pub fn run_ended(mut modals: ResMut<Modals>, mut menu: ResMut<GameMenu>) {
    let modal = game_menu_modal(&modals);
    modals.close_all();
    modals.open(modal);
    menu.selected = 0;
}

/// The sheet as lines of text: what the player was made of.
///
/// The ending screen no longer adds this by itself, because a game that
/// wants it can say so and one that does not should not pay for it. A game
/// that does pushes it as a section:
///
/// ```
/// # use bevy::prelude::*;
/// # use rl_ui::{EndingView, SheetView, describe_sheet};
/// fn what_you_were(mut ending: ResMut<EndingView>, sheet: Option<Res<SheetView>>) {
///     if let Some(sheet) = sheet {
///         ending.section("Character", describe_sheet(&sheet));
///     }
/// }
/// ```
pub fn describe_sheet(sheet: &SheetView) -> String {
    let mut lines = Vec::new();
    if !sheet.label.is_empty() {
        lines.push(sheet.label.clone());
    }
    if let Some((hp, max)) = sheet.health {
        lines.push(format!("health {hp}/{max}"));
    }
    if let Some(armor) = sheet.armor {
        lines.push(format!("armor {armor}"));
    }
    for stat in &sheet.stats {
        lines.push(format!("{} {}", stat.name, stat.value));
    }
    for strike in &sheet.strikes {
        lines.push(match strike.range {
            Some(range) => format!("shoots {} {} to {range}", strike.dice, strike.kind),
            None => format!("strikes {} {}", strike.dice, strike.kind),
        });
    }
    for worn in &sheet.worn {
        if let Some(item) = &worn.item {
            lines.push(format!("{}: {item}", worn.name));
        }
    }
    lines.join("\n")
}

/// Lines for `sections`, each under its own heading: a blank line, the
/// heading in the title tone, then its body wrapped to `width`.
///
/// Shared by this screen and the records screen's detail view
/// (`crate::panel::records`), so a game's own account of a run reads the
/// same wherever it is shown, and `pub(crate)` rather than private because
/// that is the one caller outside this file.
pub(crate) fn section_lines<'a>(sections: impl Iterator<Item = (&'a str, &'a str)>, width: usize) -> Vec<(String, ToneId)> {
    let mut lines = Vec::new();
    for (heading, body) in sections {
        lines.push((String::new(), Tones::MUTED));
        lines.push((clip(heading, width), Tones::TITLE));
        for line in body.lines() {
            lines.extend(wrap(line, width).into_iter().map(|line| (line, Tones::TEXT)));
        }
    }
    lines
}

/// What the screen is drawn from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct MenuScreen<'w> {
    layout: Res<'w, MenuLayout>,
    menu: Res<'w, GameMenu>,
    modals: Res<'w, Modals>,
    state: Res<'w, State<EngineState>>,
    ending: Option<Res<'w, Ending>>,
    view: Res<'w, EndingView>,
    settings: Option<Res<'w, Settings>>,
    title_menu: Option<Res<'w, TitleMenu>>,
    keys: ControlInput<'w>,
    palette: Res<'w, Palette>,
}

/// Paints the menu while it is open: the ending, if there is one, and the
/// choices under it.
pub fn draw_game_menu(mut terminal: ResMut<Terminal>, screen: MenuScreen) {
    let MenuScreen { layout, menu, modals, state, ending, view, settings, title_menu, keys, palette } = &screen;
    if !modals.is_open(game_menu_modal(modals)) {
        return;
    }
    // The settings screen stands in for the menu that opened it. The two
    // are as tall as their rows and seldom the same height, so a menu left
    // drawn shows its last rows under the other's frame.
    if crate::panel::settings::settings_modal(modals).is_some_and(|settings| modals.is_open(settings)) {
        return;
    }
    let rect = layout.rect;
    if rect.width < 12 || rect.height < 6 {
        return;
    }
    let playing = *state.get() == EngineState::Playing;
    let title = match ending.as_deref().filter(|_| !playing).map(|e| e.outcome) {
        Some(Outcome::Died { .. }) => &layout.died,
        Some(Outcome::Won) => &layout.won,
        Some(Outcome::Abandoned) => &layout.abandoned,
        None => &layout.title,
    };
    let bindings = keys.bindings();
    let hints = format!("\u{2191}\u{2193} pick \u{2022} {} choose", key_name(bindings.cursor.confirm));
    let width = (rect.width - 4).max(0) as usize;
    // What the ending says above the choices, worked out first so the
    // frame can close right under the last row: `layout.rect` is the most
    // the menu may take, not a box to fill, so the pause menu's four rows
    // do not trail the empty rows the ending's words needed.
    let mut words: Vec<(String, ToneId)> = Vec::new();
    if let Some(ending) = ending.as_deref().filter(|_| !playing) {
        if !ending.epitaph.is_empty() {
            words.extend(wrap(&ending.epitaph, width).into_iter().map(|line| (line, Tones::TEXT)));
        }
        // The run's own line, which the records list will show it by, said
        // here first: the title is what any ending in this outcome says,
        // and this is what this one came to. Only when there is one, as
        // with the epitaph: a game that files no record has no line, and
        // an empty one would wrap to a blank row.
        if !view.summary.is_empty() {
            words.extend(wrap(&view.summary, width).into_iter().map(|line| (line, Tones::TEXT)));
        }
        words.push((clip(&format!("Seed {}, turn {}.", ending.seed.0, ending.turn), width), Tones::MUTED));
        // Then whatever the game had to say, each section under its own
        // heading, the way the records screen draws one too: clipped by
        // the rectangle like everything else here, since the menu is
        // drawn only as far down as its rows reach and a game that wants
        // more room gives it a taller one.
        words.extend(section_lines(view.sections.iter().map(|section| (section.heading.as_str(), section.body.as_str())), width));
    }
    let items = MenuItem::offered(playing, settings_offered(modals, settings.as_deref()).is_some(), title_menu.is_some());
    // The choices are the way off this screen, so it is the words that
    // give way when both do not fit: an ending long enough to push Quit
    // under the frame left a row the cursor could reach and nobody could
    // see. Cut from the end, where the game's last section is, with a
    // muted ellipsis saying words were cut and the empty row under the
    // words kept, so the last of them never butts against the first
    // choice; and never down to a heading with none of its lines, which
    // reads as a section with nothing in it.
    let room = ((rect.height - 2).max(0) as usize).saturating_sub(items.len());
    let spacer = usize::from(!words.is_empty());
    if words.len() + spacer > room {
        words.truncate(room.saturating_sub(2));
        while words.last().is_some_and(|(line, tone)| line.is_empty() || *tone == Tones::TITLE) {
            words.pop();
        }
        words.push(("\u{2026}".to_string(), Tones::MUTED));
    }
    if spacer > 0 {
        words.push((String::new(), Tones::MUTED));
    }
    words.truncate(room);
    let height = ((words.len() + items.len()) as i32 + 2).min(rect.height);
    let rect = Rect::new(rect.x, rect.y, rect.width, height);
    clear(&mut terminal, rect, palette);
    frame(&mut terminal, rect, title, &hints, palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let surface = palette.get(Tones::SURFACE);
    let mut y = inner.y;
    for (line, tone) in &words {
        if y >= inner.bottom() {
            break;
        }
        terminal.print_on(inner.x, y, line, palette.get(*tone), surface);
        y += 1;
    }
    for (i, item) in items.iter().enumerate() {
        if y >= inner.bottom() {
            break;
        }
        let selected = i == menu.selected.min(items.len() - 1);
        let bg = if selected { palette.get(Tones::SELECT) } else { surface };
        terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', palette.get(Tones::TEXT)).on(bg));
        terminal.print_on(inner.x, y, &clip(item.label(), width), palette.get(Tones::TEXT), bg);
        y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use crate::log::MessageLog;
    use rl_core::DiceRoll;

    fn staged() -> Stage {
        let mut stage = Stage::new((GameMenuPanel::new(Rect::new(0, 0, 40, 12)), crate::SheetViewPlugin)).screen(40, 12);
        stage.tick();
        stage
    }

    /// The text inside the frame on row `y`, border and padding cut off.
    fn inside(stage: &Stage, y: i32) -> String {
        stage.row(y).trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string()
    }

    #[test]
    fn escape_opens_the_menu_over_the_run_and_the_first_row_takes_you_back() {
        let mut stage = staged();
        assert!(stage.rows().iter().all(|r| r.is_empty()));
        stage.press(KeyCode::Escape);
        assert!(stage.row(0).contains(" Menu "), "{:?}", stage.row(0));
        assert_eq!(inside(&stage, 1), "Back to the run");
        assert_eq!(inside(&stage, 4), "Quit");
        assert!(stage.app.world().resource::<Modals>().any_open(), "the world does not have the keys");
        stage.press(KeyCode::Enter);
        assert!(!stage.app.world().resource::<Modals>().any_open(), "resumed");
        stage.press(KeyCode::Escape);
        stage.press(KeyCode::Escape);
        assert!(!stage.app.world().resource::<Modals>().any_open(), "the same key closes it again");
    }

    /// With nothing open and something in sight picked out, escape lets
    /// go of it rather than opening the menu, and the next escape, with
    /// nothing picked out, opens the menu.
    #[test]
    fn escape_lets_go_of_what_is_picked_out_before_it_opens_the_menu() {
        let mut stage = Stage::new((GameMenuPanel::new(Rect::new(0, 0, 40, 12)), crate::NearbyViewPlugin)).screen(40, 12);
        stage.actor("them", 't', 2, 0);
        stage.tick();
        stage.press(KeyCode::Tab);
        assert!(stage.app.world().resource::<crate::focus::Focus>().get().is_some(), "picked out");
        stage.press(KeyCode::Escape);
        assert_eq!(stage.app.world().resource::<crate::focus::Focus>().get(), None, "let go of");
        assert!(!stage.app.world().resource::<Modals>().any_open(), "and no menu over it");
        stage.press(KeyCode::Escape);
        assert!(stage.app.world().resource::<Modals>().any_open(), "then the menu");
    }

    /// The menu is only as tall as what it says: its rectangle is the most
    /// it may take, so a pause menu of four rows closes right under the
    /// fourth instead of trailing empty rows the ending screen needed, and
    /// nothing below it is painted over.
    #[test]
    fn the_frame_closes_under_the_last_row_and_leaves_the_rest_of_its_rectangle_alone() {
        let mut stage = staged();
        stage.press(KeyCode::Escape);
        assert_eq!(inside(&stage, 4), "Quit");
        assert!(stage.row(5).starts_with('\u{2514}'), "the bottom border is the next row: {:?}", stage.row(5));
        assert!(stage.rows()[6..].iter().all(|r| r.is_empty()), "and the rest of the rectangle is untouched: {:?}", stage.rows());
    }

    /// A new run and the same seed again are each one message; quitting is
    /// the app's exit.
    #[test]
    fn the_rows_ask_for_a_fresh_run_the_same_seed_or_the_exit() {
        let mut stage = staged();
        let seed = stage.app.world().resource::<Seed>().0;
        stage.app.world_mut().resource_mut::<MessageLog>().info("an old run's line", 0);
        stage.press(KeyCode::Escape);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        // The restart is answered the same frame, at the end of it: the run
        // is torn down, the log forgotten, and the old player gone. A game's
        // start would then fill the world again on a fresh seed.
        stage.tick();
        assert!(stage.app.world().resource::<MessageLog>().is_empty(), "the log was forgotten");
        assert!(stage.app.world().get::<Position>(stage.player).is_none(), "the old player is gone");
        assert!(!stage.app.world().resource::<Modals>().any_open(), "and the menu went with the run");
        assert_ne!(stage.app.world().resource::<Seed>().0, seed, "a fresh seed");

        // The same seed again keeps the seed.
        let mut stage = staged();
        stage.press(KeyCode::Escape);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::ArrowDown);
        stage.press(KeyCode::Enter);
        stage.tick();
        assert_eq!(stage.app.world().resource::<Seed>().0, seed, "the same seed");
        assert!(stage.app.world().get::<Position>(stage.player).is_none(), "but a new run all the same");

        let mut stage = staged();
        stage.press(KeyCode::Escape);
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::Enter);
        assert_eq!(stage.app.world_mut().resource_mut::<Messages<AppExit>>().drain().count(), 1, "quit");
    }

    /// A game's own word on the run, pushed the way a facet is.
    fn note_the_haul(mut view: ResMut<EndingView>) {
        view.section("The haul", "nine bones");
    }

    /// Ends the run in the player's death: a blow the player cannot
    /// survive, from something beside them, and the frames for it to land.
    fn die(stage: &mut Stage) {
        strike_the_last_blow(stage);
        for _ in 0..4 {
            stage.tick();
        }
        assert_eq!(*stage.app.world().resource::<State<EngineState>>().get(), EngineState::Over);
    }

    /// Writes a blow the player cannot survive, from something beside
    /// them, leaving the frames it lands in to the test.
    fn strike_the_last_blow(stage: &mut Stage) {
        let (player, kind, theirs) = (stage.player, stage.kind, stage.theirs);
        stage.app.world_mut().get_mut::<Health>(player).unwrap().current = 1;
        let ogre = stage
            .app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(stage.at.offset(1, 0)),
                Health::full(9),
                Faction(theirs),
                MeleeAttack::new(kind, DiceRoll::flat(30)),
                Name::new("ogre"),
            ))
            .id();
        stage.tick();
        stage.app.world_mut().resource_mut::<MessageLog>().bad("You feel a chill.", 0);
        // The harness has no minds, so the ogre's blow is written for it.
        stage.app.world_mut().write_message(DamageEvent::new(player, rl_rules::Hit::by(ogre, kind, 30)));
        stage.app.world_mut().write_message(Intent::new(player, Wait));
    }

    /// The player's death ends the run: the menu opens by itself under the
    /// game's words with no way back, and the game's sections are on it.
    /// A game that files no record has no line of its own to say, so the
    /// seed stands first under the title, with no blank row above it.
    #[test]
    fn the_players_death_opens_the_ending_and_shows_what_the_game_had_to_say() {
        let mut stage = Stage::new((GameMenuPanel::new(Rect::new(0, 0, 44, 20)).died("You are dead."), crate::SheetViewPlugin)).screen(44, 20);
        stage.app.add_systems(Update, note_the_haul.in_set(crate::ViewSet::Annotate));
        die(&mut stage);
        assert!(stage.row(0).contains(" You are dead. "), "{:?}", stage.row(0));
        assert!(inside(&stage, 1).starts_with("Seed 5, turn "), "the seed first under the title: {:?}", inside(&stage, 1));
        let body: Vec<String> = (1..19).map(|y| inside(&stage, y)).collect();
        assert!(body.contains(&"The haul".to_string()), "the game's heading is on the screen: {body:?}");
        assert!(body.contains(&"nine bones".to_string()), "and its words under it: {body:?}");
        assert!(body.contains(&"A new run".to_string()) && !body.contains(&"Back to the run".to_string()), "no way back: {body:?}");
        assert!(body.iter().all(|l| !l.contains("morgue")), "nothing is written anywhere: {body:?}");
        stage.press(KeyCode::Escape);
        assert!(stage.app.world().resource::<Modals>().any_open(), "escape does not leave an ending");
    }

    /// An ending with more to say than its frame holds keeps every choice
    /// in it, Quit last, and gives up the end of what it had to say
    /// instead.
    #[test]
    fn an_ending_too_long_for_its_frame_keeps_every_choice_and_cuts_its_words() {
        let mut stage = Stage::new((GameMenuPanel::new(Rect::new(0, 0, 44, 12)).died("You are dead."), crate::SheetViewPlugin)).screen(44, 12);
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<EndingView>| {
                let body = (1..=12).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n");
                view.section("The long of it", &body);
            })
            .in_set(crate::ViewSet::Annotate),
        );
        die(&mut stage);
        let body: Vec<String> = (1..11).map(|y| inside(&stage, y)).collect();
        assert_eq!(
            body[body.len() - 5..],
            ["\u{2026}", "", "A new run", "This seed again", "Quit"],
            "every choice at the foot, a blank over them and a sign words were cut: {body:?}"
        );
        assert!(body.contains(&"The long of it".to_string()) && !body.contains(&"line 12".to_string()), "and the words cut short above them: {body:?}");
    }

    /// A cut that would leave a heading with none of its lines takes the
    /// heading too: the ending stops at the last section it can show
    /// something of.
    #[test]
    fn a_cut_ending_never_keeps_a_heading_without_a_line_of_its_own() {
        let mut stage = Stage::new((GameMenuPanel::new(Rect::new(0, 0, 44, 13)).died("You are dead."), crate::SheetViewPlugin)).screen(44, 13);
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<EndingView>| {
                view.section("First", "one");
                view.section("Second", "two\nthree");
            })
            .in_set(crate::ViewSet::Annotate),
        );
        die(&mut stage);
        let body: Vec<String> = (1..12).map(|y| inside(&stage, y)).collect();
        let quit = body.iter().position(|l| l == "Quit").expect("every choice");
        assert!(body.contains(&"one".to_string()), "the first section whole: {body:?}");
        assert!(!body.contains(&"Second".to_string()), "and no heading left bare: {body:?}");
        assert_eq!(body[quit - 4..=quit], ["\u{2026}", "", "A new run", "This seed again", "Quit"], "{body:?}");
    }

    /// The run's own line, the summary of the record filed for it, stands
    /// under the frame's title and the epitaph, above the seed: what the
    /// records list will say of the run, said here first.
    #[test]
    fn the_ending_says_the_runs_own_line_under_its_title() {
        let mut stage = Stage::new((GameMenuPanel::new(Rect::new(0, 0, 44, 20)).died("You are dead."), crate::SheetViewPlugin)).screen(44, 20);
        stage.app.add_systems(Update, (|mut view: ResMut<EndingView>| view.summary = "Eaten on the second floor".into()).in_set(crate::ViewSet::Annotate));
        die(&mut stage);
        assert!(stage.row(0).contains(" You are dead. "), "{:?}", stage.row(0));
        assert_eq!(inside(&stage, 1), "Eaten on the second floor", "first under the title");
        assert!(inside(&stage, 2).starts_with("Seed 5, turn "), "then the seed: {:?}", inside(&stage, 2));
    }

    /// A record a game writes as its run ends, the frame the state turns
    /// over, is on the first frame of the screen the run ends on: filed
    /// before anything draws, not on a later frame or an earlier one by
    /// whichever order the executor happens to run the two in.
    #[test]
    fn the_first_frame_of_the_ending_shows_the_runs_record() {
        let mut stage = Stage::new((GameMenuPanel::new(Rect::new(0, 0, 44, 20)).died("You are dead."), RecordsPlugin)).screen(44, 20);
        let summary = || "Eaten on the second floor".to_string();
        stage.app.add_systems(OnEnter(EngineState::Over), move |mut records: MessageWriter<RecordRun>| {
            records.write(RecordRun { score: 12, summary: summary(), sections: vec![RecordSection { heading: "Score".into(), body: "12".into() }] });
        });
        strike_the_last_blow(&mut stage);
        for _ in 0..8 {
            if *stage.app.world().resource::<State<EngineState>>().get() == EngineState::Over {
                break;
            }
            stage.tick();
        }
        assert!(stage.row(0).contains(" You are dead. "), "the ending is drawn: {:?}", stage.row(0));
        let body: Vec<String> = (1..19).map(|y| inside(&stage, y)).collect();
        assert_eq!(body[0], summary(), "the record's line on the first frame: {body:?}");
        assert!(body.contains(&"Score".to_string()) && body.contains(&"12".to_string()), "and its sections: {body:?}");
    }

    /// How many runs the game's start began.
    #[derive(Resource, Default)]
    struct Begun(u32);

    /// The menu over a run in a game with a title, both in `rect`, and the
    /// game's start counted.
    fn titled(rect: Rect) -> Stage {
        let mut stage = Stage::new((GameMenuPanel::new(rect), crate::TitlePanel::new(rect))).screen(rect.width, rect.height);
        // The stage plays its run without the title's door, which would
        // have lowered the hold the panel raised; lowered here, so only
        // the leaving can raise it again.
        stage.app.world_mut().remove_resource::<HoldStart>();
        stage.app.init_resource::<Begun>().add_systems(NewRun, |mut begun: ResMut<Begun>| begun.0 += 1);
        stage
    }

    /// The run left for the title: torn down, the start held for the
    /// title, and no other run begun in its place.
    fn assert_left_for_the_title(stage: &Stage) {
        let world = stage.app.world();
        assert_eq!(*world.resource::<State<EngineState>>().get(), EngineState::Idle);
        assert!(world.contains_resource::<HoldStart>(), "the start is held for the title");
        assert!(!world.contains_resource::<WorldMap>(), "the run was torn down");
        assert!(world.get::<Position>(stage.player).is_none(), "the player with it");
        assert_eq!(world.resource::<Begun>().0, 0, "and no other run was begun");
    }

    #[test]
    fn with_a_title_the_pause_menu_offers_main_menu_and_taking_it_leaves_the_run() {
        let mut stage = titled(Rect::new(0, 0, 40, 12));
        stage.press(KeyCode::Escape);
        let rows: Vec<String> = (1..6).map(|y| inside(&stage, y)).collect();
        assert_eq!(rows, ["Back to the run", "A new run", "This seed again", "Main menu", "Quit"], "the way back to the title, above Quit");
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::Enter);
        assert_left_for_the_title(&stage);
        assert!(stage.rows().iter().any(|row| row.contains("New Game")), "and the title is up in its place: {:#?}", stage.rows());
    }

    #[test]
    fn without_a_title_no_main_menu_row_is_offered() {
        let mut stage = staged();
        stage.press(KeyCode::Escape);
        let rows: Vec<String> = (1..5).map(|y| inside(&stage, y)).collect();
        assert_eq!(rows, ["Back to the run", "A new run", "This seed again", "Quit"]);
        assert!(!MenuItem::offered(false, true, false).contains(&MenuItem::MainMenu), "nor on the screen a run ends on");
    }

    /// A run that is over has nothing to write down, so the end screen's
    /// way back only leaves it, and the title is up in its place.
    #[test]
    fn taking_main_menu_on_the_end_screen_leaves_the_run_and_begins_no_other() {
        let mut stage = titled(Rect::new(0, 0, 44, 20));
        die(&mut stage);
        let body: Vec<String> = (1..19).map(|y| inside(&stage, y)).collect();
        let rows: Vec<&str> = body.iter().map(String::as_str).filter(|row| ["A new run", "This seed again", "Main menu", "Quit"].contains(row)).collect();
        assert_eq!(rows, ["A new run", "This seed again", "Main menu", "Quit"], "{body:?}");
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::Enter);
        assert_left_for_the_title(&stage);
        assert!(stage.rows().iter().any(|row| row.contains("New Game")), "and the title is up in its place: {:#?}", stage.rows());
    }
}
