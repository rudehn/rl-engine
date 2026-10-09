//! Undermere: a delve from a lakeside town through the Drowned Gate, down
//! ten floors of a sunken kingdom to the thing that drowned it. This
//! binary opens the window, cuts the screen and adds the panels; the game
//! itself, and its docs, live in `lib.rs`.
//!
//! `cargo run -p undermere -- --seed 7`. For a picture of a deeper floor,
//! `UNDERMERE_START=5` starts the run on floor five instead of in Mereholt,
//! and for one of the pack, `UNDERMERE_PACK="ring of light:-2, potion of
//! extra healing*2"` puts those in it as if found.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::{Rect, RunSeed};
use undermere::plugin::UndermerePlugin;
use undermere::run::{ExtraPack, StartFloor};
use undermere::typeface::DEJAVU_SANS_MONO;

/// Terminal size in cells.
const COLS: i32 = 160;
const ROWS: i32 = 45;
/// How many cells across a map tile is drawn: two, so a tile is square on
/// cells half as wide as they are tall, and a pool of light, a blast and
/// the reach of sight are round on the map while the words keep a
/// terminal's proportions.
const ACROSS: i32 = 2;
/// A cell's size in pixels, at the window the game opens in: half as wide
/// as it is tall, so a tile two cells across is square. The engine's ten
/// by sixteen made a tile a quarter wider than it is tall, and a window of
/// 160 of them wider than a laptop's screen, which then drew the whole
/// grid smaller to fit.
const CELL: Vec2 = Vec2::new(8.0, 16.0);
/// The glyphs' height in pixels: as large as letters can be and still
/// stand eight pixels apart without touching, so text reads at a
/// terminal's spacing rather than spelled out like a banner.
const FONT: f32 = 13.0;
/// Rows given to the log along the bottom of the left.
const LOG_ROWS: i32 = 5;
/// Columns given to the rail down the right.
const RAIL: i32 = 34;
/// Rows the targeting box takes at the bottom of the rail: the frame, what
/// is aimed, the range, the target, the chance, and up to four lines of it.
const TARGET_ROWS: i32 = 10;
/// Rows the rail gives to vitals: its heading with the rule under it, the
/// name, health, armor, seen and lit, noise, and a last row for the
/// badges, blank most turns, which sets what is worn off from the gauges.
const VITALS_ROWS: i32 = 8;
/// Rows the rail gives to what is worn: the heading and its rule, one
/// apiece for the seven slots, a blank row setting the list off from what
/// is nearby under it, the gap the badges' row leaves above, and two more
/// for names that wrap. A name past nineteen cells wraps,
/// `unidentified chain` over `mail`, and a ring that clings says so on a
/// row of its own, so an unidentified ring that will not come off and
/// unidentified armor beside it take the three; past that a name is kept
/// to its row, and no slot is ever pushed off the rail.
const GEAR_ROWS: i32 = 12;

/// The screen, cut up once so every panel and the map agree on it.
///
/// The rail runs down the right; the log is the bottom of the left and
/// the map the rest of it, with a blank gutter between the two and the
/// rail. Everything else floats over the map, each cut whole tiles wide
/// from an even column of it, so a screen's frame never halves a tile.
struct Screen {
    map: Rect,
    gutter: Rect,
    log: Rect,
    vitals: Rect,
    gear: Rect,
    nearby: Rect,
    hint: Rect,
    inspect: Rect,
    target: Rect,
    pack: Rect,
    chest: Rect,
    here: Rect,
    controls: Rect,
    menu: Rect,
    settings: Rect,
    records: Rect,
    title_menu: Rect,
}

impl Screen {
    fn new() -> Self {
        let (left, rail) = panel::split_right(Rect::new(0, 0, COLS, ROWS), RAIL);
        // A column of nothing between the map and the rail, so a room on a
        // floor's east edge does not run its wall into the rail's words,
        // and the odd column besides when the left is an odd number of
        // cells: a tile is two cells, and half of one is no tile.
        let (left, gutter) = panel::split_right(left, 1 + (left.width - 1) % ACROSS);
        let (map, log) = panel::split_bottom(left, LOG_ROWS);
        let (vitals, below) = panel::split_top(rail, VITALS_ROWS);
        let (gear, nearby) = panel::split_top(below, GEAR_ROWS);
        // The last row of the rail says how to see the controls.
        let (nearby, hint) = panel::split_bottom(nearby, 1);
        // The targeting box sits over the bottom of the nearby list while
        // the cursor is up, where the eye already is when choosing what to
        // aim at, and never over the map, where it would hide what is
        // being aimed at; the rows above it stay readable.
        let (_, target) = panel::split_bottom(nearby, TARGET_ROWS);
        // Centred across the map: every width here is a multiple of four,
        // so the map's even width leaves an even margin either side.
        let centred = |w: i32, y: i32, h: i32| Rect::new(map.x + (map.width - w) / 2, map.y + y, w, h);
        Self {
            map,
            gutter,
            log,
            vitals,
            gear,
            nearby,
            hint,
            // Low on the left of the map; it moves to the top while the
            // cursor is under it.
            inspect: Rect::new(map.x + 2, map.bottom() - 12, 52, 10),
            target,
            // The pack's rows, a rule, and what the row picked out is: the
            // engine gives the rows half of it, so fifteen of them, enough
            // for a pack of potions, scrolls and rings to be seen whole. A
            // longer one scrolls, and the rules above and below the rows
            // count what is out of view.
            pack: centred(64, 4, 32),
            // What is in a chest, and the keys that take it.
            chest: centred(48, 6, 14),
            // What can be done here, when here is more than one thing.
            here: centred(40, 10, 6),
            controls: map.inflate(-2),
            // The most the menu may take: the run's own line, the seed and
            // turn, the record's score and its numbers, and the choices
            // under them, with a row to spare for the way back to the
            // title. The engine closes the frame under the last row, so
            // the shorter pause menu leaves no gap. Wide enough for the
            // words a run is won with, which stand whole in its top
            // border.
            menu: centred(56, 2, 34),
            // Where the menu stands, since the menu is what opens it.
            settings: centred(56, 2, 34),
            records: centred(72, 2, 36),
            // The title's rows: on the dark water under the middle of the
            // lake, where its picture leaves room for them.
            title_menu: undermere::title::menu(COLS),
        }
    }
}

/// Every panel, each in its own cut of `screen`: shared by `main` and by
/// the tests that read the screen back, so what a test reads is what the
/// window draws. The narrator that fills the log is added beside the
/// engine's plugins, as `testing::headless` adds it.
fn add_panels(app: &mut App, screen: &Screen) {
    // The gutter belongs to no panel, so it is painted blank with the
    // chrome each frame: nothing else would ever paint it again.
    let gutter = screen.gutter;
    app.add_systems(
        Update,
        (move |mut terminal: ResMut<Terminal>, palette: Res<Palette>| panel::clear(&mut terminal, gutter, &palette)).in_set(PresentSet::Chrome),
    );
    app.add_plugins((
        // A blow struck beside the adventurer is a thousand hundredths of
        // a step, half the gauge, so a fight next door reads in the bad
        // tone and a footstep in the next room barely stirs it. No turn
        // and position: the strip's last row is the badges'.
        VitalsPanel::new(screen.vitals).bars(16).heading("Vitals").noise("noise", 2000).without_whereabouts(),
        GearPanel::new(screen.gear),
        // A monster that knows of nothing sleeps, one walking to a noise
        // is searching, and one that has the adventurer is hunting; what
        // it is doing outranks what it knows when it is running, at its
        // post, or drifting with nothing better to do.
        NearbyPanel::new(screen.nearby)
            .titled("")
            .cursor(CursorStyle::ticks())
            .alerts(AlertWords::new("sleeping", "searching", "hunting"))
            .activities(ActivityWords::new().word("flee_when_hurt", "fleeing").word("keep_post", "guarding").word("wander", "wandering")),
        LogPanel::new(screen.log),
        InspectPanel::new(screen.inspect).hints("move \u{2022} tab next \u{2022} esc close").cursor(CursorStyle::ticks()),
        TargetPanel::new(screen.target).hints("[tab/shift-tab] cycle").cursor(CursorStyle::ticks()),
        InventoryPanel::new(screen.pack).title("Pack").called("pack").empty("Nothing but lint."),
        // What is in a chest, opened by walking into it.
        ContainerPanel::new(screen.chest).empty("Empty."),
        // What can be done here, when walking into it would be a guess.
        OffersPanel::new(screen.here).title("Here"),
        InteractKey,
        // Every key `input::declare_controls` and the engine's screens
        // declare, with the hint that opens it in the rail's last row.
        ControlsPanel::new(screen.controls).hint(screen.hint),
        GameMenuPanel::new(screen.menu).title("Undermere").died("The Undermere keeps you.").won("The Drowned King is dead, and the lake is still."),
    ));
    // What holds each monster in sight, said after its name on the rail.
    app.add_systems(Update, undermere::conditions::note_statuses.in_set(ViewSet::Annotate));
    // What the player may set, opened from the menu and from the title:
    // fullscreen is the engine's, declared by `RoguelikePlugins`. And
    // every run that has ended, best first, which the title opens.
    app.add_plugins((SettingsPanel::new(screen.settings), RecordsPanel::new(screen.records)));
    // ANCHOR: title
    // The screen before there is a run: the engine's menu, which holds the
    // run back until the player picks, over the night Undermere paints
    // under it. With it up, the pause and end menus offer the way back.
    app.add_plugins(TitlePanel::new(screen.title_menu)).add_systems(Update, undermere::title::paint_backdrop.in_set(TitleSet::Backdrop));
    // ANCHOR_END: title
}

/// Whether this run plays keys from a script of some kind: a replay, a
/// recording, or a capture.
fn scripted() -> bool {
    use rl_engine::rl_bevy::replay::{RECORD_VAR, REPLAY_VAR};
    std::env::var_os(REPLAY_VAR).is_some() || std::env::var_os(RECORD_VAR).is_some() || rl_engine::rl_render::capture::requested()
}

/// Where a run is saved: beside the executable, as `undermere.save.ron`,
/// unless the run is `scripted`, which saves in memory. A scripted run
/// plays keys against a seed of its own; starting it on the player's save
/// would play a different run, and its first arrival would write over the
/// player's.
fn saves_for(scripted: bool) -> rl_engine::rl_save::Saves {
    if scripted {
        rl_engine::rl_save::Saves::new(rl_engine::rl_save::MemoryBackend::default())
    } else {
        rl_engine::rl_save::Saves::platform_default("undermere")
    }
}

/// Draws each cloud a potion bursts into in its own colour, as
/// [`CLOUD_TINTS`](undermere::content::CLOUD_TINTS) says, where the
/// engine's map draws every gas nobody coloured in grey.
fn tint_clouds(app: &mut App) {
    use rl_engine::rl_render::map_view::FieldAppearance;
    let tints: Vec<_> = {
        let gases = &app.world().resource::<Registries>().gases;
        undermere::content::CLOUD_TINTS.iter().filter_map(|(name, (r, g, b))| gases.id(name).map(|gas| (gas, Color::srgb(*r, *g, *b)))).collect()
    };
    let mut fields = app.world_mut().resource_mut::<FieldAppearance>();
    for (gas, tint) in tints {
        fields.set_gas(gas, tint);
    }
}

fn main() -> AppExit {
    let mut seed = rl_engine::rl_bevy::replay::seed().unwrap_or_else(RunSeed::fresh);
    let args = rl_engine::rl_bevy::replay::args();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {}
        ["--seed", n] => seed = RunSeed(n.parse().expect("--seed takes a number")),
        _ => {
            eprintln!("usage: undermere [--seed N]");
            return AppExit::error();
        }
    }
    let screen = Screen::new();
    let mut app = App::new();
    // ANCHOR: typeface
    // The window, the terminal in the game's own face, the map and the
    // engine's base; the game's subsystems are `UndermerePlugin`'s, added
    // once there, since a second `CombatPlugin` would panic.
    app.add_plugins(RoguelikePlugins::new("Undermere", COLS, ROWS).cell(CELL).font(FONT).typeface(DEJAVU_SANS_MONO).map(screen.map).across(ACROSS))
        // ANCHOR_END: typeface
        .add_plugins(undermere::plugin::narrator())
        .insert_resource(Seed(seed))
        .insert_resource(saves_for(scripted()))
        // Remembered beside the save and apart from it. A scripted run's
        // backend is in memory, so a capture or a replay starts on the
        // defaults whatever the player chose.
        .add_plugins(SettingsSavePlugin)
        .add_plugins(UndermerePlugin);
    add_panels(&mut app, &screen);
    tint_clouds(&mut app);
    if let Some(floor) = std::env::var("UNDERMERE_START").ok().and_then(|f| f.parse().ok()) {
        app.insert_resource(StartFloor(floor));
    }
    if let Ok(pack) = std::env::var("UNDERMERE_PACK") {
        app.insert_resource(ExtraPack::parse(&pack));
    }
    app.run()
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};
    use rl_engine::rl_render::{MapView, MapViewPlugin, Terminal};
    use undermere::run::Adventurer;

    use super::*;

    /// The window's own screen drawn into a terminal no window shows, at
    /// the title it opens on: the same cut, the same panels, the same map
    /// view at the same width of tile, and the fullscreen setting the
    /// window's plugins declare, so a test reads back the text the player
    /// would see and the menus offer the rows theirs do.
    fn at_the_title(seed: RunSeed) -> App {
        let mut app = undermere::testing::headless(seed);
        let screen = Screen::new();
        app.add_plugins((KeyScriptPlugin, MapViewPlugin::new(screen.map).across(ACROSS), rl_engine::rl_render::FullscreenPlugin));
        app.insert_resource(Terminal::new(COLS, ROWS, Vec2::ONE));
        add_panels(&mut app, &screen);
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    /// The same screen a few frames into a run begun from its title, the
    /// way a player begins one: New Game, picked out with nothing saved.
    fn on_screen(seed: RunSeed) -> App {
        let mut app = at_the_title(seed);
        press(&mut app, KeyCode::Enter);
        for _ in 0..4 {
            app.update();
        }
        app
    }

    /// Every row of the screen with something on it, trimmed.
    fn written(app: &App) -> Vec<String> {
        (0..ROWS).map(|y| row(app, y).trim().to_string()).filter(|r| !r.is_empty()).collect()
    }

    /// How many adventurers stand anywhere.
    fn adventurers(app: &mut App) -> usize {
        app.world_mut().query_filtered::<(), With<Adventurer>>().iter(app.world()).count()
    }

    /// At launch the title stands over Undermere's picture and holds the
    /// run back: nothing is built and no adventurer stands anywhere. The
    /// engine's rows are drawn one to a line from the top of the dark the
    /// picture leaves for them, on one column, with New Game picked out,
    /// since nothing is saved for Continue to take.
    #[test]
    fn the_title_is_up_at_launch_and_no_run_has_begun() {
        let mut app = at_the_title(RunSeed(7));
        assert!(app.world().contains_resource::<HoldStart>(), "the run is held");
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Idle, "and nothing plays");
        assert!(!app.world().contains_resource::<WorldMap>(), "no place is built behind the title");
        assert_eq!(adventurers(&mut app), 0, "and no adventurer stands anywhere");

        let menu = Screen::new().title_menu;
        let rows: Vec<String> = (menu.y..menu.y + 5).map(|y| row(&app, y).chars().skip(menu.x as usize).take(menu.width as usize).collect()).collect();
        assert_eq!(rows.iter().map(|r| r.trim()).collect::<Vec<_>>(), ["Continue", "New Game", "Records", "Settings", "Exit"], "{rows:#?}");
        let column: Vec<_> = rows.iter().map(|r| r.chars().position(|c| c != ' ')).collect();
        assert!(column.iter().all(|c| *c == column[0]), "on one column: {rows:#?}");
        let title = app.world().resource::<TitleMenu>();
        assert_eq!(title.rows().get(title.selected), Some(&TitleRow::NewGame), "New Game picked out");
    }

    /// New Game, picked out first with nothing saved, begins the run with
    /// one adventurer, in Mereholt where it marks the arrival, and nothing
    /// of the title's menu is left on the screen.
    #[test]
    fn new_game_from_the_title_puts_the_adventurer_in_mereholt() {
        let mut app = at_the_title(RunSeed(7));
        assert!(written(&app).iter().any(|r| r.contains("New Game")) && adventurers(&mut app) == 0, "the title waits on New Game: {:#?}", written(&app));
        press(&mut app, KeyCode::Enter);
        undermere::testing::settle(&mut app);
        assert_eq!(adventurers(&mut app), 1, "one adventurer, begun once");
        let me = undermere::testing::adventurer(&mut app);
        let entry = app.world().resource::<WorldMap>().place(undermere::dungeon::TOWN).expect("Mereholt was built").entry;
        assert_eq!((undermere::testing::here(&app), undermere::testing::at(&app, me)), (undermere::dungeon::TOWN, entry), "in Mereholt, at its arrival");
        assert!(!written(&app).iter().any(|r| r.contains("New Game") || r.contains("Continue")), "and the title is gone: {:#?}", written(&app));
    }

    /// Where the adventurer is: the place, the cell and the turn.
    fn whereabouts(app: &mut App) -> (MapId, Point, u32) {
        let me = undermere::testing::adventurer(app);
        (undermere::testing::here(app), undermere::testing::at(app, me), app.world().resource::<Turns>().now())
    }

    /// The pause menu offers Main menu once there is a title to go back
    /// to, and taking it leaves the run for the title, written down as it
    /// stood. Continue, picked out on the title that comes back, carries
    /// the run on from the same cell of the same place on the same turn.
    #[test]
    fn leaving_for_the_main_menu_and_continuing_carries_the_run_on_from_where_it_was_left() {
        let mut app = on_screen(RunSeed(7));
        undermere::testing::pass_turns(&mut app, 3);
        let left = whereabouts(&mut app);
        press(&mut app, KeyCode::Escape);
        // Past Back to the run, A new run, This seed again and Settings.
        for _ in 0..4 {
            press(&mut app, KeyCode::ArrowDown);
        }
        press(&mut app, KeyCode::Enter);
        assert!(!app.world().contains_resource::<WorldMap>(), "the run is left");
        let title = app.world().resource::<TitleMenu>();
        assert_eq!(title.rows().get(title.selected), Some(&TitleRow::Continue), "for the title, which offers it back: {:#?}", written(&app));

        press(&mut app, KeyCode::Enter);
        undermere::testing::settle(&mut app);
        assert_eq!(adventurers(&mut app), 1, "one adventurer, restored once");
        assert_eq!(whereabouts(&mut app), left, "carried on where it was left");
    }

    /// A run that ended, left for the title from the screen it ended on,
    /// and New Game: the run begun is a new one, on a seed of its own, and
    /// its first line says so, rather than the Mereholt, the floors and
    /// the rolls of the run just played begun again from its first turn.
    #[test]
    fn new_game_after_a_run_ends_and_is_left_for_the_title_begins_on_a_fresh_seed() {
        let mut app = on_screen(RunSeed(7));
        let me = undermere::testing::adventurer(&mut app);
        undermere::testing::kill(&mut app, me, None);
        undermere::testing::settle(&mut app);
        assert_eq!(*app.world().resource::<State<EngineState>>().get(), EngineState::Over, "the run is over");
        // Up from A new run, round past Quit, to Main menu.
        press(&mut app, KeyCode::ArrowUp);
        press(&mut app, KeyCode::ArrowUp);
        press(&mut app, KeyCode::Enter);
        let title = app.world().resource::<TitleMenu>();
        assert_eq!(title.rows().get(title.selected), Some(&TitleRow::NewGame), "the title, New Game picked out: {:#?}", written(&app));

        press(&mut app, KeyCode::Enter);
        undermere::testing::settle(&mut app);
        let seed = app.world().resource::<Seed>().0.0;
        assert_ne!(seed, 7, "a seed of its own");
        let lines: Vec<String> = app.world().resource::<MessageLog>().iter().map(|line| line.text.clone()).collect();
        assert!(lines.iter().any(|line| line.starts_with(&format!("Seed {seed}."))), "the log names it: {lines:#?}");
        assert!(!lines.iter().any(|line| line.starts_with("Seed 7.")), "and not the run left: {lines:#?}");
    }

    /// Row `y` of the screen, as text.
    fn row(app: &App, y: i32) -> String {
        let t = app.world().resource::<Terminal>();
        (0..t.width()).map(|x| t.get(x, y).map_or(' ', |c| c.glyph)).collect()
    }

    /// The part of row `y` from column `x` on, trimmed at the end.
    fn from(app: &App, x: i32, y: i32) -> String {
        row(app, y).chars().skip(x as usize).collect::<String>().trim_end().to_string()
    }

    /// Where the adventurer is drawn: the cell their `@` stands in.
    fn drawn_at(app: &mut App) -> Point {
        let me = undermere::testing::adventurer(app);
        let at = undermere::testing::at(app, me);
        app.world().resource::<MapView>().to_screen(at).expect("the adventurer is on the map")
    }

    /// The rail down the right holds the vitals, what is worn and how to
    /// see the keys; the log holds the bottom rows of the left; the map
    /// holds the rest of the left, the adventurer on it; and nothing is
    /// drawn in the gutter between the map and the rail.
    #[test]
    fn the_rail_the_log_and_the_map_are_where_the_screen_cuts_them() {
        let mut app = on_screen(RunSeed(7));
        let screen = Screen::new();
        let rail = COLS - RAIL;
        assert!(from(&app, rail, 0).starts_with("Vitals"), "the rail's first row: {:?}", from(&app, rail, 0));
        let worn = (0..ROWS).find(|y| from(&app, rail, *y).starts_with("Worn")).expect("what is worn, on the rail");
        assert!(worn > 0 && worn < ROWS / 2, "under the vitals: row {worn}");
        assert!(from(&app, rail, ROWS - 1).ends_with("? controls"), "the rail's last row: {:?}", from(&app, rail, ROWS - 1));

        let log: Vec<String> = (screen.log.y..screen.log.bottom()).map(|y| from(&app, screen.log.x, y)).collect();
        assert_eq!((screen.log.x, screen.log.bottom()), (0, ROWS), "the log is the bottom of the left");
        assert!(log.iter().any(|l| l.starts_with("Seed 7.")), "the run's first line, in the log: {log:#?}");

        let at = drawn_at(&mut app);
        assert!(screen.map.contains(at) && !screen.log.contains(at), "the adventurer on the map, above the log: {at:?}");
        let cell = app.world().resource::<Terminal>().get(at.x, at.y).expect("on the terminal");
        assert_eq!(cell.glyph, '@', "drawn where the map view says");
        assert!(screen.map.right() < rail, "the map stops short of the rail");
        for x in screen.map.right()..rail {
            let column: String = (0..ROWS).map(|y| app.world().resource::<Terminal>().get(x, y).map_or('?', |c| c.glyph)).collect();
            assert_eq!(column.trim(), "", "the gutter at column {x} is blank");
        }
    }

    /// A map tile is two cells of the terminal: its glyph in the first
    /// cell of the pair, drawn centred across both, the second blank on the
    /// same ground, every pair starting an even number of cells into the
    /// map, so a step east is two cells east.
    #[test]
    fn a_map_tile_is_drawn_two_cells_wide() {
        let mut app = on_screen(RunSeed(7));
        let map = Screen::new().map;
        let at = drawn_at(&mut app);
        assert_eq!((at.x - map.x) % 2, 0, "the adventurer stands in the first cell of a pair: {at:?}");
        let me = undermere::testing::adventurer(&mut app);
        let east = undermere::testing::at(&app, me).offset(1, 0);
        assert_eq!(app.world().resource::<MapView>().to_screen(east), Some(Point::new(at.x + 2, at.y)), "a step east is two cells east");
        let terminal = app.world().resource::<Terminal>();
        let mut pairs = 0;
        for y in map.y..map.bottom() {
            for x in (map.x..map.right() - 1).step_by(2) {
                let (first, second) = (terminal.get(x, y).unwrap(), terminal.get(x + 1, y).unwrap());
                assert_eq!((second.glyph, second.bg), (' ', first.bg), "the second cell of the tile at ({x}, {y}) is blank on its ground");
                assert_eq!((terminal.span(x, y), terminal.span(x + 1, y)), (Some(2), Some(0)), "and the tile at ({x}, {y}) is one glyph across both");
                pairs += 1;
            }
        }
        assert_eq!(pairs, map.width / 2 * map.height, "every tile of the map is a pair");
    }

    /// A screen over the map is drawn a letter to a cell, as the rail and
    /// the log are, while the map around it stays a tile to two cells: the
    /// pack opened in Mereholt.
    #[test]
    fn a_screen_over_the_map_is_drawn_a_letter_to_a_cell() {
        let mut app = on_screen(RunSeed(7));
        press(&mut app, KeyCode::KeyI);
        app.update();
        let screen = Screen::new();
        let pack = screen.pack;
        assert!(from(&app, pack.x, pack.y).starts_with("\u{250c}\u{2500} Pack "), "the pack is up: {:?}", from(&app, pack.x, pack.y));
        let terminal = app.world().resource::<Terminal>();
        for at in pack.cells() {
            assert_eq!(terminal.span(at.x, at.y), Some(1), "the pack's cell at {at:?} is a cell of its own");
        }
        let beside = Point::new(pack.x - 2, pack.y + 1);
        assert_eq!(terminal.span(beside.x, beside.y), Some(2), "and the tile left of it is still two cells");
    }

    /// The opening screen says where the run is and how to see the keys:
    /// the seed's line, Mereholt's and the controls' in the log, and the
    /// key for them in the rail's last row.
    #[test]
    fn the_opening_screen_names_mereholt_in_the_log_and_the_way_to_the_controls_in_the_rail() {
        let app = on_screen(RunSeed(7));
        let log: Vec<String> = (ROWS - LOG_ROWS..ROWS).map(|y| row(&app, y)).collect();
        assert!(log.iter().any(|l| l.starts_with("Seed 7. Ten floors under the lake, the Drowned King waits.")), "{log:#?}");
        assert!(log.iter().any(|l| l.starts_with("Mereholt, at dusk, on the shore of the black lake.")), "{log:#?}");
        assert!(log.iter().any(|l| l.starts_with("Press ? for the controls.")), "{log:#?}");
        assert!(row(&app, ROWS - 1).trim_end().ends_with("? controls"), "the rail's last row: {:?}", row(&app, ROWS - 1));
    }

    /// A scroll not yet known that asks which thing to read it on has told
    /// that it is one of the scrolls that ask, so it is named the moment the
    /// pack turns to choosing for it, and the log says what it is; backed
    /// out of, it is still in the pack, known and unspent, and no turn went
    /// by. Played through the pack's own keys on the screen that ships.
    #[test]
    fn an_unknown_scroll_is_named_when_the_pack_asks_what_to_read_it_on_and_backing_out_spends_nothing() {
        let mut app = on_screen(RunSeed(3));
        let me = undermere::testing::adventurer(&mut app);
        let scroll = undermere::testing::find(&mut app, "scroll of enchanting", 0);
        undermere::testing::give(&mut app, me, scroll);
        app.update();
        let knows = |app: &App| app.world().resource::<undermere::lore::Lore>().knows("scroll of enchanting");
        assert!(!knows(&app) && app.world().get::<Unknown>(scroll).is_some(), "found, it is not yet known");
        let before = app.world().resource::<Turns>().now();
        press(&mut app, KeyCode::KeyI);
        let row = app.world().resource::<InventoryView>().rows.iter().position(|r| r.entity == scroll).expect("in the pack");
        app.world_mut().resource_mut::<InventoryMenu>().selected = row;
        press(&mut app, KeyCode::KeyU);
        let choosing = app.world().resource::<InventoryMenu>().choosing.map(|c| c.item);
        assert_eq!(choosing, Some(scroll), "the pack asks what to read it on");
        assert!(knows(&app), "and it is known");
        assert_eq!(app.world().get::<Name>(scroll).map(|n| n.as_str().to_string()).as_deref(), Some("scroll of enchanting"), "called what it is");
        assert!(written(&app).iter().any(|l| l.contains("It is a scroll of enchanting.")), "said in the log: {:#?}", written(&app));
        press(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<Modals>().any_open(), "backed out");
        let bag = app.world().get::<Inventory>(me).expect("a pack");
        assert!(bag.contains(scroll) && app.world().get::<Consumable>(scroll).is_some_and(|c| c.left == 1), "still in the pack, unspent");
        assert_eq!(app.world().resource::<Turns>().now(), before, "and no turn went by");
    }

    /// The nearby rail says what a monster is doing in Undermere's words:
    /// asleep while it knows of nothing and no tactic has moved it,
    /// searching once it has heard something, hunting once it has the
    /// adventurer, and, outranking what it knows, drifting, running or at
    /// its post. A rat on floor one, three tiles off on open ground, put in
    /// each state in turn while the adventurer holds the turn.
    #[test]
    fn the_rail_says_what_a_monster_is_doing_in_undermeres_words() {
        use rl_engine::rl_rules::Awareness;
        use undermere::testing::{arrive, at, clear_monsters, lay_floor, monster, settle};
        let mut app = on_screen(RunSeed(7));
        let me = arrive(&mut app, undermere::dungeon::map_of(1));
        clear_monsters(&mut app);
        let here = at(&app, me);
        lay_floor(&mut app, &(1..=3).map(|dx| here.offset(dx, 0)).collect::<Vec<_>>());
        let rat = monster(&mut app, "rat", here.offset(3, 0));
        settle(&mut app);
        let rail = |app: &App| (0..ROWS).map(|y| from(app, COLS - RAIL, y)).find(|r| r.contains("rat (")).unwrap_or_default();
        let alert = Awareness::Alert { at: here, stale_turns: 0 };
        for (doing, sees, heard, word) in [
            (None, None, None, "sleeping"),
            (Some("wander"), None, None, "wandering"),
            (None, None, Some(alert), "searching"),
            (Some("hunt"), Some(alert), None, "hunting"),
            (Some("flee_when_hurt"), Some(alert), None, "fleeing"),
            (Some("keep_post"), Some(alert), None, "guarding"),
        ] {
            let mut aware = Aware::default();
            aware.0.extend(sees.map(|s| (me, s)));
            app.world_mut().entity_mut(rat).insert((Doing(doing), aware, Heard(heard.unwrap_or(Awareness::Unaware))));
            app.update();
            assert!(rail(&app).ends_with(&format!("rat ({word})")), "{doing:?}, seen {sees:?}, heard {heard:?}: {:?}", rail(&app));
        }
    }

    /// Every glyph on the screen as it stands, kept beside the first
    /// screen it was seen on.
    fn see(app: &App, screen: &'static str, seen: &mut std::collections::BTreeMap<char, &'static str>) {
        let terminal = app.world().resource::<Terminal>();
        for cell in terminal.bounds().cells().filter_map(|at| terminal.get(at.x, at.y)) {
            seen.entry(cell.glyph).or_insert(screen);
        }
    }

    /// Opens the screen `modal` names over whatever is up, as its key or
    /// its menu row would, and draws it.
    fn open(app: &mut App, modal: impl Fn(&Modals) -> ModalId) {
        let id = modal(app.world().resource::<Modals>());
        app.world_mut().resource_mut::<Modals>().open(id);
        app.update();
    }

    /// Every glyph on every screen a run passes through is one Undermere's
    /// typeface has, read from the face's own character map: the title and
    /// its settings, Mereholt with the rail and the log, the pack, the look
    /// cursor, the menu, the controls, a floor, the screen the run ends on,
    /// and the records of it. The engine's panels draw their frames,
    /// rules, gauges and hints in glyphs past ASCII, and a glyph the face
    /// has not is drawn from whatever face the system falls back on, or as
    /// an empty box, and in a browser always as the box.
    #[test]
    fn every_glyph_on_every_screen_a_run_passes_through_is_one_the_typeface_has() {
        let mut seen = std::collections::BTreeMap::new();
        let mut app = at_the_title(RunSeed(7));
        see(&app, "the title", &mut seen);
        open(&mut app, |modals| settings_modal(modals).expect("a settings screen"));
        see(&app, "the settings", &mut seen);
        press(&mut app, KeyCode::Escape);
        press(&mut app, KeyCode::Enter);
        undermere::testing::settle(&mut app);
        see(&app, "Mereholt", &mut seen);
        for (key, screen) in [(KeyCode::KeyI, "the pack"), (KeyCode::KeyX, "the look cursor"), (KeyCode::Escape, "the menu")] {
            press(&mut app, key);
            see(&app, screen, &mut seen);
            press(&mut app, KeyCode::Escape);
        }
        open(&mut app, controls_modal);
        see(&app, "the controls", &mut seen);
        press(&mut app, KeyCode::Escape);
        undermere::testing::arrive(&mut app, undermere::dungeon::map_of(1));
        see(&app, "floor one", &mut seen);
        let me = undermere::testing::adventurer(&mut app);
        undermere::testing::kill(&mut app, me, None);
        see(&app, "the end of the run", &mut seen);
        open(&mut app, |modals| records_modal(modals).expect("a records screen"));
        see(&app, "the records", &mut seen);

        assert!(seen.contains_key(&'\u{2500}') && seen.contains_key(&'\u{250c}'), "the screens draw rules and frames: {seen:?}");
        let face = ttf_parser::Face::parse(DEJAVU_SANS_MONO, 0).expect("the typeface is a font");
        let lacking: Vec<_> = seen.iter().filter(|(glyph, _)| face.glyph_index(**glyph).is_none()).collect();
        assert!(lacking.is_empty(), "the typeface has no glyph for {lacking:?}");
    }

    /// The cell the screen's map view draws the world tile `p` in.
    fn tile_drawn(app: &App, p: Point) -> Option<rl_engine::rl_render::Cell> {
        let view = *app.world().resource::<MapView>();
        view.get(app.world().resource::<Terminal>(), p)
    }

    /// Whether the adventurer sees `p` now.
    fn sees(app: &mut App, p: Point) -> bool {
        let me = undermere::testing::adventurer(app);
        app.world().get::<Viewshed>(me).expect("eyes").can_see(p)
    }

    /// Stands the adventurer on the floor tile nearest `p` from which `p`
    /// is out of sight but still on the screen, the way a player walks a
    /// few rooms away from something; returns where.
    fn walk_out_of_sight_of(app: &mut App, p: Point) -> Point {
        let me = undermere::testing::adventurer(app);
        let place = app.world().resource::<WorldMap>().place(undermere::testing::here(app)).expect("a place").terrain.bounds();
        let mut cells: Vec<Point> = place.cells().filter(|c| rl_engine::rl_core::geometry::chebyshev(*c, p) > 4).collect();
        cells.sort_by_key(|c| (rl_engine::rl_core::geometry::chebyshev(*c, p), *c));
        for cell in cells {
            if !app.world().resource::<WorldMap>().is_walkable(cell) || app.world().resource::<Occupancy>().is_occupied(cell) {
                continue;
            }
            undermere::testing::stand_on(app, me, cell);
            if undermere::testing::at(app, me) == cell && !sees(app, p) && tile_drawn(app, p).is_some() {
                return cell;
            }
        }
        panic!("nowhere on the place is out of sight of {p:?} and on the screen with it");
    }

    /// The bug a playtest found: the stairs left behind vanished from the
    /// map the moment they were out of sight, since only tiles were
    /// remembered. Walked away from, they are drawn where they were seen,
    /// darker than in sight; and so they still are after going down a
    /// floor and coming back.
    #[test]
    fn stairs_walked_away_from_are_still_drawn_where_they_were_seen() {
        for s in 0..2 {
            let mut app = on_screen(RunSeed(s));
            let me = undermere::testing::arrive(&mut app, undermere::dungeon::map_of(2));
            let stairs = undermere::testing::at(&app, me);
            let seen = tile_drawn(&app, stairs).expect("on screen");
            let far = walk_out_of_sight_of(&mut app, stairs);
            let remembered = tile_drawn(&app, stairs).expect("on screen");
            assert_eq!(remembered.glyph, '<', "seed {s}: the way up, out of sight from {far:?}, is still drawn");
            let lum = |c: Color| c.to_linear().luminance();
            assert!(lum(remembered.fg) < lum(undermere::stairs::STAIRS) * 0.6, "seed {s}: and drawn as remembered, not as seen: {seen:?} then {remembered:?}");

            undermere::testing::arrive(&mut app, undermere::dungeon::map_of(3));
            app.world_mut().write_message(WarpRequest { actor: me, to: Destination::Place { map: undermere::dungeon::map_of(2), arrive: Arrive::At(far) } });
            undermere::testing::settle(&mut app);
            assert!(!sees(&mut app, stairs), "seed {s}");
            assert_eq!(tile_drawn(&app, stairs).map(|c| c.glyph), Some('<'), "seed {s}: down a floor and back up, the floor above remembers its stairs");
        }
    }

    /// The same screen a few frames into a run continued from `text`, the
    /// way a player continues one: the save in the slot at launch, and
    /// Continue, which the title picks out when there is one, taken.
    fn continued_on_screen(text: &str) -> App {
        let mut app = undermere::testing::headless(RunSeed(0));
        app.world().resource::<Saves>().persist(undermere::save::SLOT, text).expect("the memory backend takes it");
        let screen = Screen::new();
        app.add_plugins((KeyScriptPlugin, MapViewPlugin::new(screen.map).across(ACROSS), rl_engine::rl_render::FullscreenPlugin));
        app.insert_resource(Terminal::new(COLS, ROWS, Vec2::ONE));
        add_panels(&mut app, &screen);
        app.finish();
        app.cleanup();
        app.update();
        press(&mut app, KeyCode::Enter);
        for _ in 0..4 {
            app.update();
        }
        app
    }

    /// What the adventurer saw is saved with the run: continued, the stairs
    /// walked away from are drawn where they were seen.
    #[test]
    fn a_continued_run_still_draws_the_stairs_it_walked_away_from() {
        let mut app = on_screen(RunSeed(1));
        let me = undermere::testing::arrive(&mut app, undermere::dungeon::map_of(2));
        let stairs = undermere::testing::at(&app, me);
        walk_out_of_sight_of(&mut app, stairs);
        rl_engine::rl_save::save_run(app.world_mut()).expect("the run is saved");
        let text = app.world().resource::<Saves>().load(undermere::save::SLOT).expect("the slot reads").expect("a save");

        let mut continued = continued_on_screen(&text);
        assert_eq!(undermere::testing::here(&continued), undermere::dungeon::map_of(2), "continued where it was saved");
        assert!(!sees(&mut continued, stairs));
        assert_eq!(tile_drawn(&continued, stairs).map(|c| c.glyph), Some('<'));
    }

    /// The adventurer knows the town they live in, what stands in it as
    /// well as its walls: on the first frame of a run every prop of
    /// Mereholt on the screen is drawn, in sight or not.
    #[test]
    fn mereholt_is_drawn_whole_with_what_stands_in_it_from_the_first_frame() {
        for s in 0..2 {
            let mut app = on_screen(RunSeed(s));
            let town = undermere::dungeon::TOWN;
            let mut q = app.world_mut().query_filtered::<(&Position, &OnMap, &Glyph), With<Prop>>();
            let props: Vec<(Point, char)> = q.iter(app.world()).filter(|(_, on, _)| on.0 == town).map(|(p, _, g)| (p.0, g.ch)).collect();
            let unseen: Vec<(Point, char)> = props.into_iter().filter(|(p, _)| tile_drawn(&app, *p).is_some() && !sees(&mut app, *p)).collect();
            assert!(unseen.len() > 3, "seed {s}: some of the town's props are on the screen and out of sight: {unseen:?}");
            let drawn: Vec<(Point, char)> = unseen.iter().map(|(p, _)| (*p, tile_drawn(&app, *p).expect("on screen").glyph)).collect();
            assert_eq!(drawn, unseen, "seed {s}: and every one of them is drawn");
        }
    }

    /// The text inside a panel's frame on row `y`, from its left border to
    /// its right, the border and the padding cut off and the end trimmed.
    fn framed(app: &App, rect: Rect, y: i32) -> String {
        let line: String = row(app, y).chars().skip(rect.x as usize + 2).take(rect.width as usize - 4).collect();
        line.trim_end().to_string()
    }

    /// The pack opened on `thing`: its row in the list, and every line under
    /// the rule saying what it is, top to bottom, as the screen draws them.
    fn pack_on(app: &mut App, thing: Entity) -> (String, Vec<String>) {
        press(app, KeyCode::KeyI);
        let at = app.world().resource::<InventoryView>().rows.iter().position(|r| r.entity == thing).expect("in the pack");
        app.world_mut().resource_mut::<InventoryMenu>().selected = at;
        app.update();
        let pack = Screen::new().pack;
        let lines: Vec<String> = (pack.y + 1..pack.bottom() - 1).map(|y| framed(app, pack, y)).collect();
        let rule = lines.iter().position(|l| l.starts_with('\u{2500}')).expect("a rule under the rows");
        let listed = lines[..rule].iter().find(|l| l.contains(&undermere::testing::name_of(app, thing))).cloned().unwrap_or_default();
        let detail = lines[rule + 1..].iter().take_while(|l| !l.is_empty()).cloned().collect();
        press(app, KeyCode::Escape);
        (listed, detail)
    }

    /// The look cursor opened on `cell`: every line inside its panel, top to
    /// bottom, as the screen draws them, wherever it moved to.
    fn looking_at(app: &mut App, cell: Point) -> Vec<String> {
        press(app, KeyCode::KeyX);
        app.world_mut().resource_mut::<InspectView>().cursor = cell;
        app.update();
        app.update();
        let rect = app.world().resource::<rl_engine::rl_ui::panel::inspect::InspectPlacement>().0.expect("the panel is up");
        let lines = (rect.y + 1..rect.bottom() - 1).map(|y| framed(app, rect, y)).take_while(|l| !l.is_empty()).collect();
        press(app, KeyCode::Escape);
        lines
    }

    /// A found chain mail is called unidentified chain mail until its level
    /// is known; the pack says what any chain mail is at +0, that its
    /// numbers are shown so, and how long it must be worn to tell its
    /// level, counting down while it is worn; the first time it goes on,
    /// the log says so once; and worn long enough, it is +2 chain mail and
    /// reads its own numbers. Played on the screen that ships, in the
    /// pack's own frame.
    #[test]
    fn a_found_chain_mail_is_unidentified_and_says_how_long_until_its_level_is_known() {
        use undermere::testing::{equip, find, give, name_of, pass_turns, said, unequip};
        let mut app = on_screen(RunSeed(3));
        let me = undermere::testing::adventurer(&mut app);
        let mail = find(&mut app, "chain mail", 2);
        give(&mut app, me, mail);
        app.update();
        let (listed, detail) = pack_on(&mut app, mail);
        assert!(listed.starts_with("[ unidentified chain mail"), "{listed:?}");
        let short = "needs 13 strength, you have 12: -1 armor";
        let found = ["numbers shown at +0", "wearing it 200 turns will tell its level", "armor +3", "footfall +1", short, "goes on the body"];
        assert_eq!(detail, found, "what any chain mail is, and what wearing it will tell");

        let worn_for = |app: &App| said(app).iter().filter(|l| *l == "You will know its level once you have worn it 200 turns.").count();
        equip(&mut app, me, mail);
        assert_eq!(worn_for(&app), 1, "put on, the log says how long: {:#?}", said(&app));
        pass_turns(&mut app, 60);
        let (_, detail) = pack_on(&mut app, mail);
        // Sixty turns, and the turn it went on.
        assert_eq!(detail, ["numbers shown at +0", "level known in 139 more turns worn", "armor +3", "footfall +1", short, "worn on the body"]);
        unequip(&mut app, me, mail);
        let (_, detail) = pack_on(&mut app, mail);
        assert_eq!(detail[1], "wearing it 139 more turns will tell its level", "taken off, it keeps what it counted");
        equip(&mut app, me, mail);
        assert_eq!(worn_for(&app), 1, "and put on again, the log says nothing more");

        pass_turns(&mut app, 140);
        assert_eq!(name_of(&app, mail), "+2 chain mail");
        let (listed, detail) = pack_on(&mut app, mail);
        assert!(listed.starts_with("[ +2 chain mail"), "{listed:?}");
        assert_eq!(detail, ["armor +5", "footfall +1", short, "worn on the body"], "known, its own numbers and nothing more");
    }

    /// The look cursor over a found sword lying on the floor calls it
    /// unidentified and says what any sword strikes for at +0; over a
    /// potion of a kind not yet known, only that it is unknown, as before.
    #[test]
    fn the_look_cursor_says_what_a_found_things_kind_is_and_a_potion_not_yet_known_is_unknown() {
        use undermere::testing::{at, find, lay, lay_floor};
        let mut app = on_screen(RunSeed(3));
        let me = undermere::testing::adventurer(&mut app);
        let here = at(&app, me);
        let (east, west) = (here.offset(1, 0), here.offset(-1, 0));
        lay_floor(&mut app, &[east, west]);
        let sword = find(&mut app, "sword", 1);
        lay(&mut app, sword, east);
        let potion = find(&mut app, "potion of extra healing", 0);
        lay(&mut app, potion, west);
        app.update();
        let lines = looking_at(&mut app, east);
        assert_eq!(lines[0], ") unidentified sword", "{lines:#?}");
        assert_eq!(lines[2..], ["numbers shown at +0", "wearing it 200 turns will tell its level", "1d7+1 slash", "needs 12 strength"], "{lines:#?}");
        let lines = looking_at(&mut app, west);
        assert_eq!(lines[2..], ["unknown"], "a potion of a kind not yet known: {lines:#?}");
    }

    /// The kit's sword, known whole at nought, is called sword with no
    /// number, and the pack says the strength it asks in the muted tone;
    /// a war hammer found asks more than the adventurer has, and the pack
    /// says by how much and what it costs, in the bad tone.
    #[test]
    fn the_pack_says_what_a_thing_asks_of_strength_and_warns_when_the_adventurer_is_short() {
        use undermere::testing::{find, give};
        let mut app = on_screen(RunSeed(3));
        let me = undermere::testing::adventurer(&mut app);
        let sword =
            app.world().get::<Equipped>(me).and_then(|w| w.in_slot(app.world().resource::<Registries>().slots.expect("main hand"))).expect("the kit's sword");
        let (listed, detail) = pack_on(&mut app, sword);
        assert!(listed.starts_with(") sword") && listed.ends_with("main hand"), "{listed:?}");
        assert_eq!(detail, ["1d7+1 slash", "needs 12 strength", "worn on the main hand"]);

        let hammer = find(&mut app, "war hammer", 1);
        give(&mut app, me, hammer);
        app.update();
        let (listed, detail) = pack_on(&mut app, hammer);
        assert!(listed.starts_with(") unidentified war hammer"), "{listed:?}");
        let warning = "needs 20 strength, you have 12: -32 accuracy";
        assert!(detail.iter().any(|l| l == warning), "{detail:#?}");
        // The line as the screen draws it, in the bad tone.
        press(&mut app, KeyCode::KeyI);
        let at = app.world().resource::<InventoryView>().rows.iter().position(|r| r.entity == hammer).expect("in the pack");
        app.world_mut().resource_mut::<InventoryMenu>().selected = at;
        app.update();
        let pack = Screen::new().pack;
        let y = (pack.y..pack.bottom()).find(|y| framed(&app, pack, *y) == warning).expect("the warning is on the screen");
        let bad = app.world().resource::<Palette>().get(Tones::BAD);
        assert_eq!(app.world().resource::<Terminal>().get(pack.x + 2, y).map(|c| c.fg), Some(bad), "in the bad tone");
    }

    /// A ring of light found at -2 clings to the adventurer when it goes on,
    /// and the pack then says its level is below nought without saying how
    /// far, what any ring of light does for each level, how long until its
    /// level is known, and that it will not come off; its name says it is
    /// unidentified.
    #[test]
    fn a_cursed_ring_that_clung_says_its_level_is_below_nought_and_will_not_come_off() {
        use undermere::testing::{equip, find, said};
        let mut app = on_screen(RunSeed(3));
        let me = undermere::testing::adventurer(&mut app);
        let ring = find(&mut app, "ring of light", -2);
        equip(&mut app, me, ring);
        let lines = said(&app);
        assert!(lines.iter().any(|l| l == "The ring of light clings to you, and will not come off."), "{lines:#?}");
        let (listed, detail) = pack_on(&mut app, ring);
        assert!(listed.starts_with("= unidentified ring of light"), "{listed:?}");
        let slot = app.world().resource::<InventoryView>().rows.iter().find(|r| r.entity == ring).map(|r| r.slot_name.clone()).expect("worn");
        let shown = ["level below nought, how far not yet known", "level known in 199 more turns worn", "light +1 a level"];
        assert_eq!(detail[..3], shown, "{detail:#?}");
        assert_eq!(detail[3..], [format!("worn on the {slot}"), "will not come off".to_string()], "{detail:#?}");

        // The rail says it will not come off as the pack does, under the
        // whole name, and every slot is still drawn; and the vitals' armor
        // is plain, since no ring of light moves it.
        let gear = Screen::new().gear;
        let rail: Vec<String> = (gear.y..gear.bottom()).map(|y| from(&app, gear.x, y)).collect();
        let at = rail.iter().position(|r| r.starts_with(&slot)).expect("the ring's slot");
        assert_eq!(
            rail[at..at + 3],
            [format!("{slot:<12} = unidentified ring"), format!("{:15}of light", ""), format!("{:15}will not come off", "")],
            "{rail:#?}"
        );
        assert!(rail.iter().any(|r| r.starts_with("light")), "the last slot is still on the rail: {rail:#?}");
        let x = gear.x + 15;
        let bad = rl_engine::rl_ui::readable(app.world().resource::<Palette>().get(Tones::BAD), app.world().resource::<Palette>());
        assert_eq!(app.world().resource::<Terminal>().get(x, gear.y + at as i32 + 2).map(|c| c.fg), Some(bad), "in the bad tone");
        let vitals = Screen::new().vitals;
        let armor = (vitals.y..vitals.bottom()).map(|y| from(&app, vitals.x, y)).find(|r| r.contains("armor")).expect("the armor");
        assert!(armor.contains("armor 1") && !armor.contains("armor 1?"), "{armor:?}");
    }

    /// A run played from a script, a replay, a recording or a capture,
    /// saves somewhere of its own: it neither starts on the player's save
    /// nor writes over it.
    #[test]
    fn a_scripted_run_never_touches_the_players_save() {
        use rl_engine::rl_save::SaveBackend as _;
        let real = rl_engine::rl_save::Saves::platform_default("undermere");
        let before = real.load(undermere::save::SLOT).unwrap();
        let scripted = saves_for(true);
        scripted.persist(undermere::save::SLOT, "a scripted run").unwrap();
        assert_eq!(real.load(undermere::save::SLOT).unwrap(), before, "the player's slot is as it was");
        assert!(scripted.exists(undermere::save::SLOT), "and the scripted run has one of its own");
    }
}
