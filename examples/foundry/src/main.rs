//! Foundry: a commando fighting down three decks of a droid foundry, with
//! weapons that run hot or run dry, droids that shoot and raise the alarm,
//! and a reactor charge that ends in a choice of upgrade. This binary
//! opens the window, cuts the screen and adds the panels; the game itself,
//! and its docs, live in `lib.rs`.
//!
//! `cargo run -p foundry -- --seed 7`. For a screenshot of a deeper deck,
//! `FOUNDRY_START=3` starts the run on deck three instead of deck one.

use bevy::prelude::*;
use foundry::plugin::FoundryPlugin;
use foundry::run::StartDeck;
use foundry::upgrades::ChoicePanel;
use rl_engine::prelude::*;
use rl_engine::rl_core::{Rect, RunSeed};

/// Terminal size in cells.
const COLS: i32 = 100;
const ROWS: i32 = 40;
/// Rows given to the log at the bottom of the screen.
const LOG_ROWS: i32 = 4;
/// Columns given to the rail down the right.
const RAIL: i32 = 30;
/// Rows the rail gives to vitals and to gear; the rest is what is nearby.
const VITALS_ROWS: i32 = 7;
const GEAR_ROWS: i32 = 9;

/// The screen, cut up once so every panel and the map agree on it.
struct Screen {
    map: Rect,
    log: Rect,
    vitals: Rect,
    gear: Rect,
    nearby: Rect,
    hint: Rect,
    inspect: Rect,
    target: Rect,
    abilities: Rect,
    pack: Rect,
    controls: Rect,
    menu: Rect,
    choice: Rect,
    chest: Rect,
    here: Rect,
}

impl Screen {
    fn new() -> Self {
        let (left, rail) = panel::split_right(Rect::new(0, 0, COLS, ROWS), RAIL);
        let (map, log) = panel::split_bottom(left, LOG_ROWS);
        let (vitals, below) = panel::split_top(rail, VITALS_ROWS);
        let (gear, nearby) = panel::split_top(below, GEAR_ROWS);
        // The last row of the rail says how to see the controls.
        let (nearby, hint) = panel::split_bottom(nearby, 1);
        let centred = |w: i32, y: i32, h: i32| Rect::new(map.x + (map.width - w) / 2, map.y + y, w, h);
        Self {
            map,
            log,
            vitals,
            gear,
            nearby,
            hint,
            inspect: Rect::new(map.x + 2, map.bottom() - 12, map.width.min(52), 10),
            target: Rect::new(map.x, map.bottom() - 1, map.width, 1),
            // One ability, a rule, and what it does.
            abilities: centred(42, 3, 12),
            // The pack's rows, a rule, and what the row picked out is worth.
            pack: centred(56, 3, 16),
            controls: map.inflate(-2),
            // The most the menu may take: the ending's words, the seed and
            // turn, and three choices. The engine closes the frame under
            // the last row, so the shorter pause menu leaves no gap.
            menu: centred(44, 6, 12),
            // Three upgrades, a blank row, and what the one picked out does.
            choice: centred(60, 8, 7),
            // What is in the crate, and the keys that take it.
            chest: centred(46, 6, 14),
            // What can be done here, when here is more than one thing.
            here: centred(40, 10, 6),
        }
    }
}

/// Every panel, each in its own cut of `screen`: shared by `main` and by
/// the tests that read the screen back, so what a test reads is what the
/// window draws. The narrator that fills the log is added beside the
/// engine's plugins, as `testing::headless` adds it.
fn add_panels(app: &mut App, screen: &Screen) {
    app.add_plugins((
        // Three thousand hundredths of a step is the gauge's full: a shot
        // or a blow next door reads about a third of it, and a probe's
        // klaxon, at eight times a shot, pegs it and then falls away over
        // the turns after. Scaled to the shot alone, everything pegged and
        // the bar said only "something happened".
        VitalsPanel::new(screen.vitals).bars(12).heading("Vitals").noise("noise", 3000),
        // What is worn, with each blaster's heat as a facet on its row.
        GearPanel::new(screen.gear),
        // One mark for every cell a screen points at: the rail's tab
        // navigation, the look cursor and the targeting cursor all frame
        // the cell rather than washing it, so the commando reads them as
        // one thing and never loses what is standing there.
        // A droid does not sleep: one that knows of nothing is idle, one
        // walking to a noise is searching, and one that has the commando
        // is hunting.
        NearbyPanel::new(screen.nearby).titled("").headings("In sight", "On the deck").cursor(CursorStyle::ticks()).alerts(AlertWords::new(
            "idle",
            "searching",
            "hunting",
        )),
        LogPanel::new(screen.log),
        InspectPanel::new(screen.inspect).hints("move \u{2022} tab next \u{2022} esc close").cursor(CursorStyle::ticks()),
        TargetPanel::new(screen.target).hints("[enter] fire  [tab] next  [esc] back").cursor(CursorStyle::ticks()),
        AbilityPanel::new(screen.abilities).title("Abilities").called("abilities"),
        // ANCHOR: bags
        InventoryPanel::new(screen.pack).title("Pack").called("pack").empty("Nothing but dust."),
        // What is inside a crate or a wreck, opened by walking into it or
        // by the key that does what is here.
        ContainerPanel::new(screen.chest).empty("Stripped already."),
        // ANCHOR_END: bags
        // What can be done here, when walking into it would be a guess.
        OffersPanel::new(screen.here).title("Here"),
        InteractKey,
        // Every key `input::declare_controls` and the engine's screens
        // declare, with the hint that opens it in the rail's last row.
        ControlsPanel::new(screen.controls).hint(screen.hint),
        GameMenuPanel::new(screen.menu).title("Foundry").died("The foundry keeps you.").won("The first charge is set."),
        ChoicePanel(screen.choice),
    ));
}

fn main() -> AppExit {
    // Through the replay module, so a recorded run replays on its own seed.
    let mut seed = rl_engine::rl_bevy::replay::seed().unwrap_or_else(RunSeed::fresh);
    let args = rl_engine::rl_bevy::replay::args();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {}
        ["--seed", n] => seed = RunSeed(n.parse().expect("--seed takes a number")),
        _ => {
            eprintln!("usage: foundry [--seed N]");
            return AppExit::error();
        }
    }
    let screen = Screen::new();
    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("Foundry", COLS, ROWS).map(screen.map))
        .add_plugins((CombatPlugin, MindsPlugin, StatusPlugin, ItemsPlugin, ThrowingPlugin, LightingPlugin, StealthPlugin, FactsPlugin, AbilitiesPlugin))
        .add_plugins(NoisePlugin::new(foundry::droids::NOISE))
        .add_plugins(NarratorPlugin::default())
        // The engine's own effects: `Mend`, for `stims`.
        .add_engine_effects()
        .insert_resource(foundry::content::registries())
        .insert_resource(Seed(seed))
        .add_plugins(FoundryPlugin);
    add_panels(&mut app, &screen);
    if let Some(deck) = std::env::var("FOUNDRY_START").ok().and_then(|d| d.parse().ok()) {
        app.insert_resource(StartDeck(deck));
    }
    let abilities = {
        let world = app.world();
        let (kinds, registries) = (world.resource::<EffectKinds>(), world.resource::<Registries>());
        foundry::upgrades::load_abilities(kinds, registries)
    };
    app.insert_resource(abilities);
    app.run()
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::testing::KeyScriptPlugin;
    use rl_engine::rl_render::{MapViewPlugin, Terminal};

    use super::*;

    /// A headless run drawing the window's own screen into a terminal no
    /// window shows: the same cut, the same panels, the same map view, so
    /// a test reads back the text the player would see.
    fn on_screen(seed: RunSeed) -> App {
        let mut app = foundry::testing::headless(seed);
        let screen = Screen::new();
        app.add_plugins((KeyScriptPlugin, MapViewPlugin::new(screen.map)));
        app.insert_resource(Terminal::new(COLS, ROWS, Vec2::ONE));
        add_panels(&mut app, &screen);
        app.finish();
        app.cleanup();
        for _ in 0..4 {
            app.update();
        }
        app
    }

    /// Row `y` of the screen, as text.
    fn row(app: &App, y: i32) -> String {
        let t = app.world().resource::<Terminal>();
        (0..t.width()).map(|x| t.get(x, y).map_or(' ', |c| c.glyph)).collect()
    }

    #[test]
    fn the_opening_screen_names_the_first_deck_in_the_log_and_the_way_to_the_controls_in_the_rail() {
        let app = on_screen(RunSeed(7));
        let log: Vec<String> = (ROWS - LOG_ROWS..ROWS).map(|y| row(&app, y)).collect();
        assert!(log.iter().any(|l| l.starts_with("Deck 1: the upper assembly hall.")), "{log:#?}");
        assert!(log.iter().any(|l| l.starts_with("Press ? for the controls.")), "{log:#?}");
        assert!(row(&app, ROWS - 1).trim_end().ends_with("? controls"), "the rail's last row: {:?}", row(&app, ROWS - 1));
        assert!(row(&app, 0)[(COLS - RAIL) as usize..].starts_with("Vitals"));
    }

    /// The heat facet is the one thing on the rail the engine could not
    /// have drawn by itself: a blaster fired until it locks says so on its
    /// gear row, in full and in the palette's warning tone, however long
    /// the slot and the name ahead of it.
    #[test]
    fn a_blaster_fired_until_it_locks_reads_locked_in_the_warning_tone_on_the_gear_panel() {
        let mut app = on_screen(RunSeed(7));
        let (me, _) = foundry::testing::player_with_hand_blaster(&mut app);
        foundry::testing::fire_at_a_target(&mut app, me, 7);
        app.update();
        let y = (0..ROWS).find(|y| row(&app, *y).contains("main hand")).expect("a gear row for the main hand");
        let line = row(&app, y);
        assert!(line.trim_end().ends_with("\u{00b7} locked"), "{line:?}");
        // The last letter of "locked", wherever the row ends.
        let x = (0..COLS).rev().find(|x| app.world().resource::<Terminal>().get(*x, y).is_some_and(|c| c.glyph == 'd')).unwrap();
        let palette = app.world().resource::<Palette>();
        let bad = rl_engine::rl_ui::readable(palette.get(Tones::BAD), palette);
        assert_eq!(app.world().resource::<Terminal>().get(x, y).unwrap().fg, bad);
    }

    /// The log reads in the order things happened: a probe that spots the
    /// commando is logged noticing it, and only then sounding the alarm
    /// its noticing set off, in the log the player reads.
    #[test]
    fn a_probe_that_spots_the_commando_is_logged_noticing_it_before_the_alarm_it_sounds() {
        let mut app = on_screen(RunSeed(1));
        let (_, me) = foundry::testing::droid_facing_player(&mut app, "probe droid", 4);
        let lines = |app: &App| app.world().resource::<MessageLog>().iter().map(|e| e.text.clone()).collect::<Vec<_>>();
        for _ in 0..20 {
            if lines(&app).iter().any(|l| l == "The probe droid sounds an alarm.") {
                break;
            }
            app.world_mut().write_message(Intent::new(me, Wait));
            app.update();
        }
        let lines = lines(&app);
        let at = |what: &str| lines.iter().position(|l| l.contains(what)).unwrap_or_else(|| panic!("{what:?} not in {lines:#?}"));
        assert!(at("The probe droid notices you.") < at("The probe droid sounds an alarm."), "{lines:#?}");
    }

    #[test]
    fn a_new_run_puts_the_commando_on_deck_one() {
        let mut app = foundry::testing::headless(RunSeed(3));
        for _ in 0..5 {
            app.update();
        }
        let map = app.world().resource::<WorldMap>().current();
        assert_eq!(foundry::decks::deck_of(map), 1);
    }
}
