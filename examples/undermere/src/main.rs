//! Undermere: a delve from a lakeside town through the Drowned Gate, down
//! ten floors of a sunken kingdom to the thing that drowned it. This
//! binary opens the window, cuts the screen and adds the panels; the game
//! itself, and its docs, live in `lib.rs`.
//!
//! `cargo run -p undermere -- --seed 7`. For a picture of a deeper floor,
//! `UNDERMERE_START=5` starts the run on floor five instead of in Mereholt,
//! `UNDERMERE_NEAR=camp` stands the adventurer a few steps short of what
//! it names there (`camp`, `captive`, `lair`, `throne` or a monster's
//! name); without `UNDERMERE_START`, `UNDERMERE_NEAR=smith` stands them
//! beside that person in Mereholt instead (any role of `townsfolk.ron`,
//! `smith`, `priest`, `gatewarden`, `child`...), one step from their shop
//! or their talk. For one of the pack, `UNDERMERE_PACK="ring of light:-2,
//! potion of extra healing*2"` puts those in it as if found.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::{Rect, RunSeed};
use undermere::plugin::UndermerePlugin;
use undermere::run::{ExtraPack, StartFloor, StartNear};
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
/// apiece for the seven slots, and a blank row setting the list off from
/// what is nearby under it, the gap the badges' row leaves above. Nothing
/// Undermere wears says anything beside its name yet, so no slot wraps.
const GEAR_ROWS: i32 = 10;

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
    shop: Rect,
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
            // A keeper's shelf beside the pack, wide enough for a long
            // name and its price on either side and for the longest
            // greeting under the keeper's name, the apothecary's: the
            // frame, that line, the headings and their rule, fifteen rows,
            // as many as the pack shows, and the purses at the foot.
            shop: centred(108, 4, 22),
            // What can be done here, when here is more than one thing:
            // wide enough for the temple's longest rite with its turn and
            // its price, `unbind what you wear (1 turn, 60 gold coins)`.
            here: centred(48, 10, 6),
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
        // A monster that knows of nothing is unaware, one walking to a
        // noise is searching, and one that has the adventurer is hunting:
        // not "sleeping" for the first, since a monster that truly sleeps
        // says so by its own status and one awake and unaware does not
        // sleep. What
        // it is doing outranks what it knows when it is running, at its
        // post, or drifting with nothing better to do. The things below
        // are everything in sight that is not someone, stairs and a gate
        // mark as well as what lies about, so not "on the ground".
        NearbyPanel::new(screen.nearby)
            .titled("")
            .headings("In sight", "Around you")
            .cursor(CursorStyle::ticks())
            .alerts(AlertWords::new("unaware", "searching", "hunting"))
            .activities(
                ActivityWords::new()
                    .word("flee_when_hurt", "fleeing")
                    .word("keep_post", "guarding")
                    .word("wander", "wandering")
                    .word(undermere::conditions::SLEEPING, "asleep")
                    .word(undermere::captives::SHACKLED, "shackled"),
            ),
        LogPanel::new(screen.log),
        InspectPanel::new(screen.inspect).hints("move \u{2022} tab next \u{2022} esc close").cursor(CursorStyle::ticks()),
        TargetPanel::new(screen.target).hints("[tab/shift-tab] cycle").cursor(CursorStyle::ticks()),
        InventoryPanel::new(screen.pack).title("Pack").called("pack").empty("Nothing but lint."),
        // What is in a chest, opened by walking into it.
        ContainerPanel::new(screen.chest).empty("Empty."),
        // A keeper's shelf and the pack, opened by walking into the keeper.
        TradePanel::new(screen.shop),
        // What can be done here, when walking into it would be a guess.
        OffersPanel::new(screen.here).title("Here"),
        InteractKey,
        // Every key `input::declare_controls` and the engine's screens
        // declare, with the hint that opens it in the rail's last row.
        ControlsPanel::new(screen.controls).hint(screen.hint),
        GameMenuPanel::new(screen.menu).title("Undermere").died("The Undermere keeps you.").won("The throne is empty, and the lake is still."),
    ));
    // What holds each monster in sight, said after its name on the rail.
    app.add_systems(Update, undermere::conditions::note_statuses.in_set(ViewSet::Annotate));
    // A captive said shackled on the rail, until it is freed.
    app.add_systems(Update, undermere::captives::note_captives.in_set(ViewSet::Annotate));
    // A keeper's wares by their true names, in the shop's own colour, and
    // the keeper's greeting under their name.
    app.add_systems(Update, (undermere::shops::shelf_names, undermere::townsfolk::greet).in_set(ViewSet::Annotate));
    // A person of Mereholt's role beside their name, on the look panel in
    // place of the forecast the game's own seeming may have redone, and on
    // the rail in place of what they know of the adventurer.
    app.add_systems(Update, undermere::townsfolk::name_roles.after(undermere::seeming::seem_forecast).in_set(ViewSet::Annotate));
    // The temple's rites by what they do, not "name the priest", and every
    // keeper's trade as "trade with" them, not "trade the priest".
    app.add_systems(Update, (undermere::temple::rite_rows, undermere::shops::trade_rows).in_set(ViewSet::Annotate));
    // What is known of a monster's traits on the look panel: every plain
    // one in its own words, and a "?" for every trick not yet seen.
    app.add_systems(Update, undermere::bestiary_lore::annotate_inspect.in_set(ViewSet::Annotate));
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
    if let Ok(near) = std::env::var("UNDERMERE_NEAR") {
        app.insert_resource(StartNear(near));
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
        // Mereholt is at peace with the adventurer, so a blow struck there
        // is dropped before it reaches Health; floor one is not.
        let me = undermere::testing::arrive(&mut app, undermere::dungeon::map_of(1));
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
        assert!(log.iter().any(|l| l.starts_with("Seed 7. Ten floors under the lake, something waits on a drowned throne.")), "{log:#?}");
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
            (None, None, None, "unaware"),
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

    /// The rail beside the smith says who is in sight with no count of
    /// foes, since there are none, and on a floor heads what else is in
    /// sight `Around you`, not `On the ground`, which it is not.
    #[test]
    fn the_rail_counts_no_foes_at_nought_and_heads_its_things_around_you() {
        use undermere::testing::{arrive, at, clear_monsters, lay, lay_floor, make, settle, stand_on};
        let mut app = on_screen(RunSeed(7));
        let me = undermere::testing::adventurer(&mut app);
        let smith = undermere::testing::townsperson(&mut app, "smith");
        let beside = at(&app, smith).offset(0, -1);
        stand_on(&mut app, me, beside);
        let rail = |app: &App| (0..ROWS).map(|y| from(app, COLS - RAIL, y)).collect::<Vec<_>>();
        let lines = rail(&app);
        let heading = lines.iter().position(|r| r.trim() == "In sight").unwrap_or_else(|| panic!("a bare heading: {lines:#?}"));
        let name = undermere::testing::name_of(&app, smith);
        assert!(lines[heading + 2..].iter().any(|r| r.contains(&name)), "over the smith, {name}: {lines:#?}");

        arrive(&mut app, undermere::dungeon::map_of(1));
        clear_monsters(&mut app);
        let here = at(&app, me);
        lay_floor(&mut app, &[here.offset(1, 0), here.offset(2, 0)]);
        let potion = make(&mut app, "potion of healing");
        lay(&mut app, potion, here.offset(2, 0));
        settle(&mut app);
        let lines = rail(&app);
        assert!(lines.iter().any(|r| r.trim() == "Around you"), "{lines:#?}");
        assert!(!lines.iter().any(|r| r.contains("On the ground")), "{lines:#?}");
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

    /// Every keeper's greeting, quoted as the trade screen says it, fits
    /// the line under their name whole: the frame and its margins take
    /// two cells a side.
    #[test]
    fn every_keepers_greeting_fits_the_trade_screens_line() {
        let line = (Screen::new().shop.width - 4) as usize;
        let app = on_screen(RunSeed(0));
        for role in app.world().resource::<undermere::townsfolk::Townsfolk>().roles() {
            let Some(greeting) = &role.greeting else { continue };
            assert!(greeting.chars().count() + 2 <= line, "the {}'s greeting is {} long, the line {line}", role.role, greeting.chars().count() + 2);
        }
    }

    /// Walking into a keeper with the key that walks opens their shop on
    /// the screen, and the press that opened it does nothing more there:
    /// nothing is bought or sold, and the cursor stands where the screen
    /// opens, on the shelf's first row, where the next press of that key
    /// moves it a row down.
    #[test]
    fn walking_into_a_keeper_opens_the_shop_and_the_same_press_does_not_act_on_its_first_frame() {
        let mut app = on_screen(RunSeed(7));
        let me = undermere::testing::adventurer(&mut app);
        let smith = undermere::testing::townsperson(&mut app, "smith");
        let at = undermere::testing::at(&app, smith);
        undermere::testing::stand_on(&mut app, me, at.offset(0, -1));
        let (mine, theirs) = (undermere::testing::pack(&app, me), undermere::testing::pack(&app, smith));
        press(&mut app, KeyCode::ArrowDown);
        undermere::testing::settle(&mut app);
        let modals = app.world().resource::<Modals>();
        assert!(modals.is_open(trade_modal(modals)), "the smith's shop is up");
        assert_eq!(app.world().resource::<Trading>().open().map(|p| (p.customer, p.trader)), Some((me, smith)), "a trade with the smith");
        let name = undermere::testing::name_of(&app, smith);
        assert!(written(&app).iter().any(|r| r.contains(&name)), "the screen names the smith, {name}: {:#?}", written(&app));
        let greeting = "\"Steel, leather, and arrows by the bundle. I buy what's straight and sell what's sharp.\"";
        assert!(written(&app).iter().any(|r| r.contains(greeting)), "and their greeting reads whole under it: {:#?}", written(&app));
        let menu = app.world().resource::<rl_engine::rl_ui::panel::trade::TradeMenu>();
        assert_eq!((menu.side, menu.wares, menu.pack), (rl_engine::rl_ui::TradeSide::Wares, 0, 0), "the cursor where the screen opens");
        assert_eq!(undermere::testing::pack(&app, me), mine, "nothing bought or sold");
        assert_eq!(undermere::testing::pack(&app, smith), theirs, "and the shelf as it was");
        assert_eq!(undermere::testing::at(&app, me), at.offset(0, -1), "and nobody stepped");
        // The same key pressed again, on the screen, moves the cursor: so
        // the first press would have shown, had it reached the screen.
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(app.world().resource::<rl_engine::rl_ui::panel::trade::TradeMenu>().wares, 1, "the next press moves down a row");
    }

    /// Walking into the priest asks rather than pays: the offers screen
    /// opens listing `trade` and the three rites, each rite at its price,
    /// and the press that opened it spends nothing, takes nothing up and
    /// moves nobody, since the walk key never spends gold; nor does the log
    /// call the priest in the way.
    #[test]
    fn walking_into_the_priest_opens_the_offers_screen_with_each_rites_price_and_pays_nothing() {
        let mut app = on_screen(RunSeed(7));
        let me = undermere::testing::adventurer(&mut app);
        let priest = undermere::testing::townsperson(&mut app, "priest");
        let at = undermere::testing::at(&app, priest);
        let town = undermere::dungeon::TOWN;
        let taken: Vec<Point> = {
            let world = app.world_mut();
            world.query_filtered::<&Position, With<Blocks>>().iter(world).map(|p| p.0).collect()
        };
        let sides = [((0, -1), KeyCode::ArrowDown), ((0, 1), KeyCode::ArrowUp), ((-1, 0), KeyCode::ArrowRight), ((1, 0), KeyCode::ArrowLeft)];
        let map = app.world().resource::<WorldMap>();
        let (from, key) = sides
            .into_iter()
            .map(|((dx, dy), key)| (at.offset(dx, dy), key))
            .find(|(cell, _)| map.is_walkable_on(town, *cell) && !taken.contains(cell))
            .expect("a side of the priest to walk in from");
        undermere::testing::stand_on(&mut app, me, from);
        let (mine, theirs) = (undermere::testing::pack(&app, me), undermere::testing::pack(&app, priest));
        press(&mut app, key);
        undermere::testing::settle(&mut app);
        let modals = app.world().resource::<Modals>();
        assert!(modals.is_open(offers_modal(modals)), "the offers screen is up: {:#?}", written(&app));
        assert!(!modals.is_open(trade_modal(modals)), "and not the shop");
        let view = app.world().resource::<rl_engine::rl_ui::view::offers::OffersView>();
        let rows: Vec<(String, Option<u32>)> = view.rows.iter().map(|r| (r.label.clone(), r.price)).collect();
        let trade = format!("trade with {}", undermere::testing::name_of(&app, priest));
        let offered = [(trade.as_str(), None), ("mend your wounds", Some(30)), ("unbind what you wear", Some(60)), ("name what you wear", Some(60))]
            .map(|(v, p)| (v.to_string(), p));
        assert_eq!(rows, offered.to_vec(), "trade and the rites at their prices");
        for rite in ["mend your wounds (1 turn, 30 gold coins)", "unbind what you wear - needs 60 gold coins", "name what you wear - needs 60 gold coins"] {
            assert!(written(&app).iter().any(|r| r.contains(rite)), "the screen lists {rite:?} whole: {:#?}", written(&app));
        }
        assert_eq!(undermere::testing::pack(&app, me), mine, "nothing paid");
        assert_eq!(undermere::testing::pack(&app, priest), theirs, "and nothing taken");
        assert_eq!(undermere::testing::at(&app, me), from, "and nobody stepped");
        let said = undermere::testing::said(&app);
        assert!(!said.iter().any(|line| line.contains("in the way")), "the priest is asked about, not in the way: {said:#?}");
    }
}
