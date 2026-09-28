//! Foundry: a commando fighting down ten decks of a droid foundry, with
//! weapons that run hot or run dry, droids that shoot and raise the alarm,
//! and a reactor charge that ends in a choice of upgrade. This binary
//! opens the window, cuts the screen and adds the panels; the game itself,
//! and its docs, live in `lib.rs`.
//!
//! `cargo run -p foundry -- --seed 7`. For a screenshot of a deeper deck,
//! `FOUNDRY_START=3` starts the run on deck three instead of deck one.
//! `cargo run -p foundry -- --prefabs` prints, instead of playing, what
//! every drawn slot of every piece finds on every deck: `✓` where the draw
//! lands on the deck asked, `~N` where it falls back to deck `N`, and `✗`
//! where it finds nothing.

use bevy::prelude::*;
use foundry::cheats::CheatPanel;
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
/// Rows the targeting box takes at the bottom of the rail: the frame, what
/// is aimed, the range, the target, the chance, and up to four lines of it.
const TARGET_ROWS: i32 = 10;
/// Rows the rail gives to vitals and to gear; the rest is what is nearby.
/// Vitals is its heading with the rule under it, the name, health, armor,
/// seen and lit, noise, and a last row for the badges: blank most turns,
/// which sets the gear off from the gauges as the blank under the gear
/// sets it off from the nearby list.
const VITALS_ROWS: i32 = 8;
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
    cheats: Rect,
    search: Rect,
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
        // The targeting box sits over the bottom of the nearby list while
        // the cursor is up, where the eye already is when choosing what to
        // aim at; the rows above it stay readable.
        let (_, target) = panel::split_bottom(nearby, TARGET_ROWS);
        let centred = |w: i32, y: i32, h: i32| Rect::new(map.x + (map.width - w) / 2, map.y + y, w, h);
        Self {
            map,
            log,
            vitals,
            gear,
            nearby,
            hint,
            inspect: Rect::new(map.x + 2, map.bottom() - 12, map.width.min(52), 10),
            target,
            // Stims and the four grenades, a rule, and what the one picked
            // out does: how it is aimed, what it costs and every effect, which
            // for an incendiary is two.
            abilities: centred(56, 3, 18),
            // The pack's rows, a rule, and what the row picked out is worth.
            pack: centred(56, 3, 16),
            controls: map.inflate(-2),
            // The most the menu may take: the ending's words, the seed and
            // turn, and three choices. The engine closes the frame under
            // the last row, so the shorter pause menu leaves no gap.
            menu: centred(44, 6, 12),
            // Three upgrades, a blank row, and what the one picked out does.
            choice: centred(60, 8, 7),
            // Six cheats, a blank row, and what the one picked out does.
            cheats: centred(56, 6, 10),
            // A page of the armory, and the keys along the bottom.
            search: centred(56, 6, 18),
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
        // the bar said only "something happened". No turn and position:
        // the strip's last row is the badges', and a line that showed only
        // while no status did would come and go.
        VitalsPanel::new(screen.vitals).bars(12).heading("Vitals").noise("noise", 3000).without_whereabouts(),
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
        TargetPanel::new(screen.target).hints("[tab/shift-tab] cycle").cursor(CursorStyle::ticks()),
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
        GameMenuPanel::new(screen.menu).title("Foundry").died("The foundry keeps you.").won("The core is charged, and you are on the lift."),
        ChoicePanel(screen.choice),
        CheatPanel { menu: screen.cheats, search: screen.search },
    ));
}

/// Whether this run plays keys from a script of some kind: a replay, a
/// recording, or a capture.
fn scripted() -> bool {
    use rl_engine::rl_bevy::replay::{RECORD_VAR, REPLAY_VAR};
    std::env::var_os(REPLAY_VAR).is_some() || std::env::var_os(RECORD_VAR).is_some() || rl_engine::rl_render::capture::requested()
}

/// Where a run is saved: beside the executable, as `foundry.save.ron`,
/// unless the run is `scripted`, which saves in memory. A scripted run
/// plays keys against a seed of its own; starting it on the player's save
/// would play a different run, and its first arrival would write over the
/// player's.
fn saves_for(scripted: bool) -> rl_engine::rl_save::Saves {
    if scripted { rl_engine::rl_save::Saves::new(rl_engine::rl_save::MemoryBackend::default()) } else { rl_engine::rl_save::Saves::platform_default("foundry") }
}

fn main() -> AppExit {
    // Through the replay module, so a recorded run replays on its own seed.
    let mut seed = rl_engine::rl_bevy::replay::seed().unwrap_or_else(RunSeed::fresh);
    let args = rl_engine::rl_bevy::replay::args();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {}
        ["--seed", n] => seed = RunSeed(n.parse().expect("--seed takes a number")),
        ["--prefabs"] => {
            print!("{}", foundry::prefabs::coverage_report().render());
            return AppExit::Success;
        }
        _ => {
            eprintln!("usage: foundry [--seed N | --prefabs]");
            return AppExit::error();
        }
    }
    let screen = Screen::new();
    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("Foundry", COLS, ROWS).map(screen.map))
        .add_plugins((CombatPlugin, MindsPlugin, StatusPlugin, ItemsPlugin, ThrowingPlugin, LightingPlugin, StealthPlugin, FactsPlugin, AbilitiesPlugin))
        // Fire and smoke, for the grenades: each registers the effect its
        // grenade names, `Ignite` and `Emit`, before the abilities load.
        .add_plugins((FirePlugin, GasPlugin))
        // Hearing, which registers the `Noise` every grenade is heard by.
        .add_plugins(NoisePlugin::new(foundry::droids::NOISE))
        .add_plugins(foundry::plugin::narrator())
        // The engine's own effects: `Mend`, for `stims`, and `Harm` for
        // the grenades.
        .add_engine_effects()
        .insert_resource(foundry::content::registries())
        .insert_resource(Seed(seed))
        .insert_resource(saves_for(scripted()))
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
    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};
    use rl_engine::rl_render::{MapViewPlugin, Terminal};
    use rl_engine::rl_ui::{InventoryMenu, InventoryView, inventory_modal};

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
        // Where the first charge is set, which holds of the ten-deck
        // foundry wherever the run starts.
        assert!(log.iter().any(|l| l.starts_with("Seed 7. The drop ship is gone. The first reactor is on deck three.")), "{log:#?}");
        assert!(log.iter().any(|l| l.starts_with("Deck 1: the upper assembly hall.")), "{log:#?}");
        assert!(log.iter().any(|l| l.starts_with("Press ? for the controls.")), "{log:#?}");
        assert!(row(&app, ROWS - 1).trim_end().ends_with("? controls"), "the rail's last row: {:?}", row(&app, ROWS - 1));
        assert!(row(&app, 0)[(COLS - RAIL) as usize..].starts_with("Vitals"));
    }

    /// The targeting box sits at the bottom of the rail, over the nearby
    /// list and above the controls hint, only while the cursor is up, and
    /// its first row names the gun being fired.
    #[test]
    fn the_targeting_box_is_framed_at_the_bottom_of_the_rail_while_aiming() {
        let mut app = on_screen(RunSeed(7));
        let (me, _) = foundry::testing::player_with_hand_blaster(&mut app);
        let rail = |app: &App, y: i32| row(app, y).chars().skip((COLS - RAIL) as usize).collect::<String>().trim_end().to_string();
        let hint = ROWS - 1;
        let top = hint - TARGET_ROWS;
        assert!(!rail(&app, top).contains("Targeting"), "no box before the cursor is up: {:?}", rail(&app, top));

        app.world_mut().write_message(AimFire { user: me });
        app.update();
        app.update();
        assert!(rail(&app, top).starts_with("\u{250c}\u{2500} Targeting "), "the top border: {:?}", rail(&app, top));
        assert!(rail(&app, top + 1).starts_with("\u{2502}fire hand blaster"), "what is aimed: {:?}", rail(&app, top + 1));
        assert!(
            rail(&app, hint - 1).starts_with("\u{2514}") && rail(&app, hint - 1).contains("[tab/shift-tab] cycle"),
            "the bottom border: {:?}",
            rail(&app, hint - 1)
        );
        // The controls hint goes while a screen is open; the box stops above
        // its row rather than drawing over it.
        assert!(!rail(&app, hint).contains('\u{2502}') && !rail(&app, hint).contains('\u{2514}'), "the box stops above the hint row: {:?}", rail(&app, hint));
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

    /// A status's badge has a row of its own on the vitals strip, under
    /// the noise gauge and above the worn gear, so the cloak's `%` is on
    /// screen for as long as the commando is cloaked; with no badge the
    /// row is blank, never the turn and position, which would come and go
    /// with every status.
    #[test]
    fn a_status_badge_is_drawn_on_the_vitals_strip_above_the_worn_gear() {
        let mut app = on_screen(RunSeed(7));
        let rail = |app: &App, y: i32| row(app, y).chars().skip((COLS - RAIL) as usize).collect::<String>().trim_end().to_string();
        assert_eq!(rail(&app, VITALS_ROWS - 1), "", "blank with no badge to show");
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let cloaked = app.world().resource::<Registries>().statuses.expect("cloaked");
        app.world_mut().write_message(Afflict { target: me, status: cloaked, turns: 10, by: None, held_by: None });
        app.update();
        app.update();
        assert_eq!(rail(&app, VITALS_ROWS - 1), "%", "the badge, on the last row of the strip");
        assert!(rail(&app, VITALS_ROWS).starts_with("Worn"), "and the gear still under it: {:?}", rail(&app, VITALS_ROWS));
    }

    /// The pack says the cloak plate's cloak lasts only while it is worn,
    /// under the plate, in the words the effect describes itself with.
    #[test]
    fn the_pack_says_the_cloak_plate_cloaks_only_while_worn() {
        let mut app = on_screen(RunSeed(7));
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let plate = foundry::testing::equip_new(&mut app, me, "cloak plate");
        app.update();
        let view = app.world().resource::<InventoryView>();
        let row = view.rows.iter().find(|r| r.entity == plate).expect("the plate in the pack");
        assert!(row.used.contains(&"use: cloaked for 10 turns while worn".to_string()), "{:?}", row.used);
    }

    /// `name` made at `level` and put in the commando's pack, and a pass
    /// run so the pack's rows are read again.
    fn in_the_pack(app: &mut App, name: &str, level: i32) -> Entity {
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let registries = app.world().resource::<Registries>().clone();
        let armory = foundry::testing::armory_of(app);
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let item = {
            let mut commands = Commands::new(&mut queue, app.world());
            foundry::gear::spawn_item_at(&mut commands, &armory, armory.defs.expect(name), level, &registries)
        };
        queue.apply(app.world_mut());
        app.world_mut().get_mut::<Inventory>(me).expect("the commando has a pack").items.push(item);
        app.update();
        app.update();
        item
    }

    /// The pack row of `item`, as the pack draws it.
    fn pack_row(app: &App, item: Entity) -> rl_engine::rl_ui::ItemRow {
        app.world().resource::<InventoryView>().rows.iter().find(|r| r.entity == item).expect("in the pack").clone()
    }

    /// A slug rifle with no slugs to feed it still says in the pack what
    /// it shoots, at its level, and that it is dry: the shot put by while
    /// it cannot fire is the one a player weighing a `+2` wants to read.
    #[test]
    fn the_pack_says_what_a_dry_gun_shoots_at_its_level_and_that_it_is_dry() {
        let mut app = on_screen(RunSeed(7));
        let rifle = in_the_pack(&mut app, "slug rifle", 2);
        assert!(app.world().get::<foundry::ammo::Dry>(rifle).is_some(), "no slugs in the pack, so it is dry");
        let row = pack_row(&app, rifle);
        assert_eq!(row.shot.map(|s| s.dice.to_string()).as_deref(), Some("1d10+5"), "damage: 2, so four more at +2");
        assert!(row.facets.iter().any(|f| f.text == "dry"), "{:?}", row.facets);
    }

    /// A rangefinder helmet says in the pack how far it sees in the dark,
    /// with its level's tiles added, since that is what it is for.
    #[test]
    fn the_pack_says_how_far_a_rangefinder_sees_in_the_dark_at_its_level() {
        let mut app = on_screen(RunSeed(7));
        let plain = in_the_pack(&mut app, "rangefinder helmet", 0);
        let fine = in_the_pack(&mut app, "rangefinder helmet", 2);
        let sees = |row: rl_engine::rl_ui::ItemRow| row.facets.iter().map(|f| f.text.clone()).find(|t| t.contains("dark"));
        assert_eq!(sees(pack_row(&app, plain)).as_deref(), Some("sees 6 in the dark"));
        assert_eq!(sees(pack_row(&app, fine)).as_deref(), Some("sees 8 in the dark"));
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

    /// `t` is the pack, opened on the first thing in it that flies:
    /// which of several is meant is the player's to pick, and the pack's
    /// own `t` throws the row picked out.
    #[test]
    fn t_opens_the_pack_on_what_can_be_thrown_and_t_again_throws_it() {
        let mut app = on_screen(RunSeed(5));
        let me = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let blade = foundry::testing::equip_new(&mut app, me, "monoblade");
        app.update();
        app.world_mut().resource_mut::<Messages<AimThrow>>().clear();

        press(&mut app, KeyCode::KeyT);
        let modals = app.world().resource::<Modals>();
        assert!(modals.is_top(inventory_modal(modals)), "the pack is up");
        let picked = app.world().resource::<InventoryMenu>().selected;
        assert_eq!(app.world().resource::<InventoryView>().rows[picked].entity, blade, "on the blade");
        assert!(app.world_mut().resource_mut::<Messages<AimThrow>>().drain().next().is_none(), "and nothing is thrown yet");

        press(&mut app, KeyCode::KeyT);
        let asked: Vec<Entity> = app.world_mut().resource_mut::<Messages<AimThrow>>().drain().map(|a| a.item).collect();
        assert_eq!(asked, vec![blade]);
    }

    /// A run played from a script, a replay, a recording or a capture,
    /// saves somewhere of its own: it neither starts on the player's save
    /// nor writes over it.
    #[test]
    fn a_scripted_run_never_touches_the_players_save() {
        use rl_engine::rl_save::SaveBackend as _;
        let real = rl_engine::rl_save::Saves::platform_default("foundry");
        let before = real.load(foundry::save::SLOT).unwrap();
        let scripted = saves_for(true);
        scripted.persist(foundry::save::SLOT, "a scripted run").unwrap();
        assert_eq!(real.load(foundry::save::SLOT).unwrap(), before, "the player's slot is as it was");
        assert!(scripted.exists(foundry::save::SLOT), "and the scripted run has one of its own");
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
