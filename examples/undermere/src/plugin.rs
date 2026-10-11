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
        // fire, the clouds potions burst into, the props the places stand,
        // the arrows a bow spends, the slots the pieces fill, loot, the
        // facts the quest counts, and the record a run ends in.
        app.add_plugins((CombatPlugin, MindsPlugin, StatusPlugin, ItemsPlugin, ThrowingPlugin, ConsumablesPlugin, LightingPlugin, StealthPlugin))
            .add_plugins((AbilitiesPlugin, FirePlugin, GasPlugin, NoisePlugin::new(NOISE), PropsPlugin::spotting(crate::traps::SPOTTING), AmmoPlugin))
            // The run's captive: the one verb `PropsPlugin` does not already
            // know, offered by an actor rather than a prop.
            .add_verb(crate::captives::FREE_VERB)
            // A sealed vault's concealed lever, and what a reward piece
            // offers to be read.
            .add_verb(crate::vaults::PULL_VERB)
            .add_verb(crate::vaults::READ_VERB)
            .add_plugins((PrefabPlugin::<Bestiary, Armory>::default(), LootPlugin::<Armory>::default()))
            .add_plugins((FactsPlugin, RecordsPlugin))
            // What the dead leave lying where they fell, for `raising` to
            // find within sight and reach: every monster but the king, who
            // never dies in a way anything could be left to raise.
            .add_plugins(RemainsPlugin::naming("{what} corpse"));
        // Which kind a death's remnant stands for, so a burst of spores
        // where a myconid fell is learned as the myconid's.
        rl_engine::rl_bevy::KeepOnRemnant::keep_on_remnant::<crate::bestiary::KindTag>(app);
        // The engine's effects Undermere names, for the potions, the staffs
        // and the breaths, beside the `Ignite` fire declares and the `Emit`
        // the clouds do; and Undermere's own, for the scrolls, the potions,
        // the staffs and the wands: a thing's level, what is known of it, a
        // curse, a strength, where someone stands, what they are, whose
        // side, what they see and what their things hold.
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
        // What the adventurer knows of the run's monsters: which kind's
        // tricks they have seen land, the same kind of thing and the same
        // run-to-run life as `Lore`.
        app.init_resource::<crate::bestiary_lore::MonsterLore>().reset_on_new_run::<crate::bestiary_lore::MonsterLore>();
        // Every flooding vault still filling: none at a run's start, and
        // what a continued run's save says.
        app.init_resource::<crate::vaults::lifts::RisingFloods>().reset_on_new_run::<crate::vaults::lifts::RisingFloods>();

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
        // What the rings give that the engine does not read itself: health
        // back over time, a staff's pace and what a blow puts towards its
        // next charge, and how easily what is hidden is spotted. Chained, so
        // what a blow puts towards a charge is put at the pace this pass's
        // wisdom keeps.
        app.add_systems(
            Turn,
            (crate::stats::regenerate, crate::staffs::keep_pace, crate::staffs::reap, crate::stats::keep_keen).chain().in_set(TurnSet::React),
        );
        // How loud every step is, read off the footfall stat at the start
        // of every pass, before anyone steps in it: what is worn is folded
        // onto the stat in the reactions, unordered against these.
        app.add_systems(Turn, crate::stats::keep_footfall.in_set(TurnSet::Schedule));
        // What the adventurer's hands showed of their things, then a curse
        // closing on them, then how long wearing a thing tells its level,
        // then the turns they have worn what they do not know yet: chained,
        // since each says so in the log, and the lines land in this order.
        app.add_systems(
            Turn,
            (crate::lore::learn_by_use, crate::enchant::bind_on_wear, crate::lore::tell_unfamiliar, crate::lore::grow_familiar).chain().in_set(TurnSet::React),
        );
        // What a zap at somewhere strikes, weighed between the actions that
        // make one and the triggers that land it, so a wand zapped at
        // nobody is not learned by it; and what any use showed nothing of,
        // which the reactions above read and empty.
        app.init_resource::<crate::effects::ShowedNothing>();
        app.add_systems(Turn, crate::lore::weigh_landings.in_set(TurnSet::Resolve).after(ResolveSet::Act).before(ResolveSet::Triggers));
        // Who fights beside the adventurer: a conjured blade gone when its
        // time is up, and a walk into an ally a change of places with it.
        app.add_systems(Turn, crate::allies::fade.in_set(TurnSet::React));
        app.insert_resource(BumpRules::new().swap_allies());
        // What the potions' statuses do that the engine's do not: a
        // confused step turned about once everyone has chosen and before
        // anything resolves it, and fire immunity kept on whoever holds it.
        // What a monster throws: only what strikes, and whether it has
        // stolen something and should run with it.
        app.add_systems(Turn, (crate::bestiary::throw_what_strikes, crate::tricks::sense_stole).in_set(DecideSet::Perceive).after(PerceiveSet::Annotate));
        // The lived-in dungeon: a patrolling mind's next
        // waypoint, sensed beside the engine's own post and leader, and
        // never for an actor with no route of its own.
        app.add_systems(Turn, crate::tricks::sense_patrols.in_set(PerceiveSet::Annotate));
        app.add_stream::<crate::conditions::Stumbles>("UndermerePlugin");
        app.add_systems(Turn, crate::conditions::stumble.in_set(DecideSet::Game));
        app.add_systems(Turn, (crate::conditions::keep_fireproof, crate::conditions::keep_discord).in_set(TurnSet::React));
        // The bestiary's own tricks that read what a turn did rather than
        // the trait model's fold: a blow that splits what it left alive, a
        // blow that mends its own drainer, a step that leaves fire behind
        // it, a troll's regeneration
        // halted while it burns, a Choirmaster's choir kept over its own
        // side, the Widow's egg sacs stepped beside, the Mother Below's
        // young torn from the walls and gone the pass she dies, and the
        // Drowned King's own pool.
        app.add_systems(
            Turn,
            (
                crate::tricks::judge_splits,
                crate::tricks::judge_draining,
                crate::tricks::trail_fire,
                crate::tricks::keep_regen_halted,
                crate::tricks::sing_choir,
                crate::tricks::hatch_beside,
                crate::tricks::spawn_young,
                crate::tricks::die_with_mother,
                crate::tricks::rise_from_the_pool,
            )
                .in_set(TurnSet::React),
        );
        // A shackled captive's own "free".
        app.add_systems(Turn, crate::captives::answer_free.in_set(TurnSet::React));
        // What the adventurer spotted, said in the log.
        app.add_systems(Turn, crate::traps::tell_spotted.in_set(TurnSet::React));
        // The risk-and-reward pieces' own: the marks a piece leaves for the
        // game to fill, after the engine's own `PrefabSet::Fill` has filled
        // every slot beside them, a lever and a vault's key, and what
        // lifting a prize sets off.
        app.add_systems(Turn, crate::vaults::spawn_vault_marks.in_set(TurnSet::React).after(PrefabSet::Fill));
        app.add_systems(Turn, (crate::vaults::answer_pull_lever, crate::vaults::spend_vault_key, crate::vaults::answer_read).in_set(TurnSet::React));
        {
            use crate::vaults::lifts::*;
            app.add_systems(
                Turn,
                (shut_other_cages, loose_the_vermin, give_way, collapse_behind, start_flood, rise_floods.after(start_flood)).in_set(TurnSet::React),
            );
        }
        // What is known of a kind's tricks, learned off the same messages
        // the tricks above, an ability's own landing and the engine's own
        // actor triggers already answer with: each only once its effect
        // lands where the adventurer can see it.
        app.add_systems(
            Turn,
            (
                crate::bestiary_lore::learn_from_abilities,
                crate::bestiary_lore::learn_from_triggers,
                crate::bestiary_lore::learn_from_statuses,
                crate::bestiary_lore::learn_fire_trail,
                crate::bestiary_lore::learn_draining,
                crate::bestiary_lore::learn_reflecting,
            )
                .in_set(TurnSet::React),
        );
        // The shriek a scroll of aggravate monsters lets out.
        app.add_sound(crate::effects::minds::SHRIEK_SOUND);
        // A drinker of descent set down on the floor below once it stands:
        // after the arrival has built and filled it.
        app.add_systems(Turn, crate::effects::moving::land_sinking.in_set(TurnSet::React).after(crate::finds::lay));
        // Every thing called what is known of it: after the narrator has
        // read the pass, so nothing is told of under a name it had not yet
        // earned, and before the spent are taken out; at the start of every
        // pass, for what came into the world between the passes, a
        // continued run's things, before anything in the pass is told of;
        // and before the frame is drawn, for a frame no pass runs in.
        app.add_systems(Turn, crate::lore::keep_names.in_set(TurnSet::Cleanup).before(CleanupSet::Remove));
        app.add_systems(Turn, crate::lore::keep_names.in_set(TurnSet::Schedule));
        app.add_systems(Update, crate::lore::keep_names.after(EngineSet::Turns).before(EngineSet::Present));
        // A scroll that asks which thing to read it on is named as it asks:
        // after the keys that turn the pack to choosing, before the names.
        app.add_systems(Update, crate::lore::learn_by_asking.after(EngineSet::Input).before(EngineSet::Turns));
        // What the panels count of a worn thing not yet known: its kind's
        // plain numbers, over the engine's true ones, for the vitals' armor,
        // the look cursor's forecast and the targeting box's chance; and
        // what the pack and the look cursor say of a thing whose kind is
        // known and whose level is not. Each reads its view when the game
        // draws it and does nothing where it does not.
        app.add_systems(
            Update,
            (crate::seeming::seem_armor, crate::seeming::seem_forecast, crate::seeming::seem_aim, crate::seeming::seem_pack, crate::seeming::seem_looked_at)
                .in_set(ViewSet::Annotate),
        );
        // What a thing asks of the adventurer's strength, under the pack's
        // row and the look cursor's: after what a thing not yet known
        // seems, so the line reads under its numbers.
        app.add_systems(
            Update,
            crate::strength::note_strength.after(crate::seeming::seem_pack).after(crate::seeming::seem_looked_at).in_set(ViewSet::Annotate),
        );
        // Each place's ambient, set between the turns and the light, so the
        // frame the stairs land on is drawn in the new place's light; and
        // beside it a camp's fire lit, so the frame it is stood is lit.
        app.add_systems(
            Update,
            (crate::light::set_ambient, crate::light::light_loose_props)
                .after(EngineSet::Turns)
                .before(EngineSet::Light)
                .run_if(in_state(EngineState::Playing)),
        );
        // A photograph's ask to stand near something on the floor it
        // starts on, which a real run never makes.
        app.add_systems(
            Update,
            crate::run::stand_near.after(EngineSet::Turns).run_if(resource_exists::<crate::run::StartNear>).run_if(in_state(EngineState::Playing)),
        );
        // The keys, declared once so the controls screen lists what the
        // input reads. `f` and `t` hand the engine's targeting cursor an
        // `AimFire` or an `AimThrow`; registering both here is a no-op
        // beside the cursor's own plugin, and lets a headless test with no
        // cursor press the keys all the same.
        crate::input::declare_controls(app);
        app.add_message::<AimFire>().add_message::<AimThrow>();
        app.add_systems(Update, crate::input::player_input.in_set(EngineSet::Input));
        // A lair's own band, led the moment its prefab's slots are
        // filled. Its own call rather than folded into the chain above,
        // since nothing in that chain's own order depends on this nor
        // this on it.
        app.add_systems(Turn, crate::population::lead_lair_bands.in_set(TurnSet::React).after(PrefabSet::Fill));
        // The two artifacts' own clocks: the Crown of Grubnak's aura on
        // whoever stands beside its wearer, recomputed whole every pass,
        // and the Abbess's Chalice refilled the moment a new floor is
        // reached. Neither is a worn thing's own `bestows` or a trigger
        // can reach: one lands on somebody other than the wearer, the
        // other refills on a clock no `recharge` ticks.
        app.add_systems(Turn, crate::artifacts::crown_aura.in_set(TurnSet::React));
        app.add_systems(Turn, crate::artifacts::refill_chalice.in_set(TurnSet::React).after(PrefabSet::Fill));
    }

    /// Every line Undermere says from inside a turn is a `Tell`, which
    /// only the engine's narrator collects and speaks.
    fn finish(&self, app: &mut App) {
        rl_engine::rl_bevy::plugin::depends_on::<rl_engine::rl_ui::NarrationViewPlugin>(app, "UndermerePlugin");
    }
}
