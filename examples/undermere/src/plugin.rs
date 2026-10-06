//! `UndermerePlugin`: every one of Undermere's own systems and resources,
//! and the engine's plugins the game plays with, in one place.
//!
//! A system registered in the binary and again in `testing::headless` is
//! two places to keep in step, and nothing stops them drifting apart, so
//! a test would exercise a different game from the one that ships. This
//! is the one list instead: the binary adds it beside `RoguelikePlugins`
//! and its panels, `testing::headless` adds it beside the few engine
//! plugins a window would have brought, and neither registers a game
//! system of its own.
//!
//! Foundry lists the engine's gameplay plugins in its binary and again in
//! its harness; here they are part of the game, since which subsystems
//! run is as much the game as its own systems are. What stays outside is
//! what a window brings, `CorePlugin`, `FovPlugin` and `UiPlugin` among
//! them, the narrator, which the binary and the harness add from
//! [`narrator`], the run's `Seed` and the `Saves` it is written to, and
//! drawing into a rectangle of the screen, which a headless test has no
//! screen to cut. The title is one of those: the engine's `TitlePanel`,
//! with [`title`](crate::title)'s picture painted under it, holds the
//! first run back, so the binary adds it with the panels and a headless
//! run, which has none, begins at startup.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::plugin::{CleanupSet, NewRun, ResetsOnNewRun, Turn, TurnSet};
use rl_engine::rl_bevy::{FirePlugin, LootPlugin, LootSet, NoiseRules, PrefabPlugin, PrefabSet, PropsPlugin};

use crate::armory::Armory;
use crate::bestiary::Bestiary;
use crate::content::Library;

/// What a sound carries, in steps of loudness over open ground: a step two
/// tiles, so a keen-eared monster hears the adventurer coming and nothing
/// else does; a blow ten, so a fight is heard down the corridor; a door
/// or a thrown thing landing six; and three taken off by a shut door.
pub const NOISE: NoiseRules = NoiseRules { step: 2, strike: 10, door: 6, landing: 6, door_muffle: 3 };

/// The narrator as Undermere words it, for the binary and
/// `testing::headless` both, so a test reads the log the player reads.
///
/// The engine's own words, as they stand: the adventurer is `you`, a
/// monster is named by its kind and the Drowned King by his name, and a
/// monster's ability as it is written. The one place to change them.
pub fn narrator() -> NarratorPlugin {
    NarratorPlugin::default()
}

/// Undermere's own systems, and the engine's plugins it plays with.
pub struct UndermerePlugin;

impl Plugin for UndermerePlugin {
    fn build(&self, app: &mut App) {
        // The engine's subsystems a run plays with: fighting and minds,
        // statuses, things carried, worn, thrown and drunk, light and the
        // dark, being noticed and heard, what monsters breathe and cast,
        // fire, the props the places stand, the arrows a bow spends, the
        // slots the pieces fill, loot, the facts the quest counts, and the
        // record a run ends in.
        app.add_plugins((CombatPlugin, MindsPlugin, StatusPlugin, ItemsPlugin, ThrowingPlugin, ConsumablesPlugin, LightingPlugin, StealthPlugin))
            .add_plugins((AbilitiesPlugin, FirePlugin, NoisePlugin::new(NOISE), PropsPlugin, AmmoPlugin))
            .add_plugins((PrefabPlugin::<Bestiary, Armory>::default(), LootPlugin::<Armory>::default()))
            .add_plugins((FactsPlugin, RecordsPlugin))
            // `Harm`, `Mend` and `Inflict`, for the potions, the flask and
            // the breaths, beside the `Ignite` fire declares.
            .add_engine_effects();
        // And Undermere's own, for the scrolls and the potion of strength:
        // a thing's level, what is known of it, a curse, a strength.
        crate::content::add_game_effects(app);
        // The content, read once before any run begins against the
        // effects and moments every plugin above declared: no run changes
        // what a sword or a goblin is. A file that does not read stops the
        // game here, by name.
        let library = {
            let world = app.world();
            Library::load(world.resource::<EffectKinds>(), world.resource::<Moments>())
        };
        // What the run is for, read once with the rest: the engine forgets
        // a run's progress when it ends, so the next run's quest is open
        // again without the file being read again.
        let (quests, facts) = crate::quest::load(&library.registries);
        app.insert_resource(quests).insert_resource(facts);
        app.insert_resource(library.registries).insert_resource(library.abilities).insert_resource(library.armory).insert_resource(library.bestiary);
        app.insert_resource(library.faces);
        // What the adventurer knows of the run's things, dealt out afresh
        // by every run's start and given back by a continued run's save;
        // nothing between two runs, so the next one's faces are its own.
        app.init_resource::<crate::lore::Lore>().reset_on_new_run::<crate::lore::Lore>();

        // The run's start, and a continued run after the fresh start has
        // stood aside.
        app.add_systems(NewRun, crate::run::start);
        app.add_systems(NewRun, crate::run::resume.after(crate::run::start));
        // What a run is saved as, and when: on the way out and on every
        // arrival.
        crate::save::register(app);
        // The run's story in numbers, nought at the start and put back to
        // nought when the run ends, so the next run in the same launch has
        // been nowhere and found nothing; and the stacks that left the
        // world since the reactions last read them, which a teardown
        // leaves nothing of.
        app.init_resource::<crate::tally::Tally>().reset_on_new_run::<crate::tally::Tally>();
        app.init_resource::<crate::tally::Departed>().reset_on_new_run::<crate::tally::Departed>();
        app.add_observer(crate::tally::note_departure);
        // What a turn did, counted and answered inside it: the fighting and
        // what was found and drunk into the tally, and a boss's death into
        // the fact the quest counts, and the win when it finishes the
        // quest. Chained, and after the arrivals' chain, which raises the
        // tally's deepest floor, so nothing writes the tally in an order
        // the scheduler picks.
        app.add_systems(
            Turn,
            (crate::tally::count_the_fighting, crate::tally::count_what_is_found, crate::quest::report_bosses)
                .chain()
                .in_set(TurnSet::React)
                .after(LootSet::Scatter),
        );
        // The run written down the frame it is over, however it ended.
        app.add_systems(OnEnter(EngineState::Over), crate::tally::record);
        // A place fills the moment it is first entered: the engine's slots
        // in `PrefabSet::Fill`, then the depth, the ways through and the
        // place's line in the log, the floor's people, the fixtures' light
        // and the floor's things, before the engine's loot in
        // `LootSet::Scatter`, each reading the same `PlaceEntered` through
        // its own cursor. After the slots, so the people keep off every
        // cell a piece marked; the things after the ways through, which
        // they keep off. Chained, though no two of them share a cell:
        // systems that spawn, left unordered, queue their commands in
        // whatever order they finish in, and a place's entities would come
        // out in a different order from one run to the next.
        app.add_systems(
            Turn,
            (crate::tally::remember_depth, crate::stairs::link_places, crate::population::populate, crate::light::light_fixtures, crate::finds::lay)
                .chain()
                .in_set(TurnSet::React)
                .after(PrefabSet::Fill)
                .before(LootSet::Scatter),
        );
        // What the adventurer wears and carries, kept in step every pass:
        // what a worn thing lends, then how far the lantern reaches where
        // the adventurer stands. Chained, since both light things. Beside
        // them what too heavy a thing costs, and how sneaky the stealth
        // stat makes the adventurer, each reading its own numbers.
        app.add_systems(Turn, (crate::wearing::lend, crate::light::dim_the_lantern).chain().in_set(TurnSet::React));
        app.add_systems(Turn, (crate::strength::keep_strength, crate::stats::keep_stealth).in_set(TurnSet::React));
        // How loud every step is, read off the footfall stat at the start
        // of every pass, before anyone steps in it: what is worn is folded
        // onto the stat in the reactions, unordered against these.
        app.add_systems(Turn, crate::stats::keep_footfall.in_set(TurnSet::Schedule));
        // What the adventurer's hands showed of their things, then a curse
        // closing on them, then the turns they have worn what they do not
        // know yet: chained, since each says so in the log, and the lines
        // land in this order.
        app.add_systems(Turn, (crate::lore::learn_by_use, crate::enchant::bind_on_wear, crate::lore::grow_familiar).chain().in_set(TurnSet::React));
        // Every thing called what is known of it: after the narrator has
        // read the pass, so nothing is told of under a name it had not yet
        // earned, and before the spent are taken out; at the start of every
        // pass, for what came into the world between the passes, a
        // continued run's things, before anything in the pass is told of;
        // and before the frame is drawn, for a frame no pass runs in.
        app.add_systems(Turn, crate::lore::keep_names.in_set(TurnSet::Cleanup).before(CleanupSet::Remove));
        app.add_systems(Turn, crate::lore::keep_names.in_set(TurnSet::Schedule));
        app.add_systems(Update, crate::lore::keep_names.after(EngineSet::Turns).before(EngineSet::Present));
        // Each place's ambient, set between the turns and the light, so the
        // frame the stairs land on is drawn in the new place's light.
        app.add_systems(Update, crate::light::set_ambient.after(EngineSet::Turns).before(EngineSet::Light).run_if(in_state(EngineState::Playing)));
        // The keys, declared once so the controls screen lists what the
        // input reads. `f` and `t` hand the engine's targeting cursor an
        // `AimFire` or an `AimThrow`; registering both here is a no-op
        // beside the cursor's own plugin, and lets a headless test with no
        // cursor press the keys all the same.
        crate::input::declare_controls(app);
        app.add_message::<AimFire>().add_message::<AimThrow>();
        app.add_systems(Update, crate::input::player_input.in_set(EngineSet::Input));
    }

    /// Every line Undermere says from inside a turn is a `Tell`, which
    /// only the engine's narrator collects and speaks.
    fn finish(&self, app: &mut App) {
        rl_engine::rl_bevy::plugin::depends_on::<rl_engine::rl_ui::NarrationViewPlugin>(app, "UndermerePlugin");
    }
}
