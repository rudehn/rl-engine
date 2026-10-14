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
use rl_engine::rl_bevy::plugin::{CleanupSet, NewRun, ReactSet, ResetsOnNewRun, Turn, TurnSet};
use rl_engine::rl_bevy::{AmmoSet, FactSet, FirePlugin, LootPlugin, LootSet, NoiseRules, PrefabPlugin, PrefabSet, PropsPlugin};

use crate::armory::Armory;
use crate::bestiary::Bestiary;
use crate::content::Library;

#[cfg(test)]
mod ambiguity;

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

/// The engine's subsystems a run plays with, and nothing of Undermere's
/// own: fighting and minds, statuses, things carried, worn, thrown and
/// drunk, light and the dark, being noticed and heard, what monsters
/// breathe and cast, fire, the clouds potions burst into, the props the
/// places stand, the arrows a bow spends, the slots the pieces fill,
/// loot, the facts the quest counts, the record a run ends in, what the
/// dead leave, the adventurer's long actions and trading.
///
/// [`UndermerePlugin`] adds these before any system of its own, and the
/// ordering test builds the game with these and without the plugin, so
/// whatever the plugin adds beyond them is the game's to order.
pub fn add_engine(app: &mut App) {
    app.add_plugins((CombatPlugin, MindsPlugin, StatusPlugin, ItemsPlugin, ThrowingPlugin, ConsumablesPlugin, LightingPlugin, StealthPlugin))
        .add_plugins((AbilitiesPlugin, FirePlugin, GasPlugin, NoisePlugin::new(NOISE), PropsPlugin::spotting(crate::traps::SPOTTING), AmmoPlugin))
        .add_plugins((PrefabPlugin::<Bestiary, Armory>::default(), LootPlugin::<Armory>::default()))
        .add_plugins((FactsPlugin, RecordsPlugin))
        // What the dead leave lying where they fell, for `raising` to
        // find within sight and reach: every monster but the king, who
        // never dies in a way anything could be left to raise. A kind's
        // body is a "goblin corpse", and one with a name of its own,
        // a mercenary or Grubnak, "the corpse of Brenna".
        .add_plugins(RemainsPlugin::naming(RemainsNaming::new("{what} corpse").proper("the corpse of {what}")))
        // The engine's multi-turn work, the adventurer's long actions:
        // the waystone's kneeling and freeing a captive, each spent and
        // broken off by the engine and answered by `waystone` and
        // `captives`.
        .add_plugins(WorkPlugin)
        // Trading, paid in the `Currency` the plugin inserts once the
        // content is read.
        .add_plugins(TradePlugin);
}

/// The order of Undermere's own answers inside `TurnSet::React`, so no two
/// that touch the same thing run in whichever order the scheduler picks,
/// which any system added anywhere could change.
///
/// A place filled first, on the arrival that finds it, after the engine's
/// prefab slots; then the engine's own settling of worn gear, sleep,
/// charges, ammunition and work; then what the pass did answered, after
/// the engine's loot too, since a lever or a flood edits the map loot is
/// laid on; then what the adventurer is derived again from the stats as
/// they now stand; and last the tally counting all of it. Each step's
/// own systems are chained, in the order the step reads in.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReactStep {
    /// A place filled the moment it is first entered.
    Fill,
    /// What the pass did, answered: work done, offers taken up, the map
    /// changed, the tricks and what is learned of them.
    Answer,
    /// What the adventurer is, kept from the stats as they stand.
    Keep,
    /// The run's story in numbers, and the quest's.
    Count,
}

/// Undermere's own systems, and the engine's plugins it plays with.
pub struct UndermerePlugin;

impl Plugin for UndermerePlugin {
    fn build(&self, app: &mut App) {
        add_engine(app);
        // The run's captive: the one verb `PropsPlugin` does not already
        // know, offered by an actor rather than a prop.
        app.add_verb(crate::captives::FREE_VERB)
            // A sealed vault's concealed lever, and what a reward piece,
            // a flavor piece or a wall carving offers to be read.
            .add_verb(crate::vaults::PULL_VERB)
            .add_verb(crate::vaults::wells::DRINK_VERB)
            .add_verb(crate::vaults::wells::TEMPER_VERB)
            .add_verb(crate::writings::READ_VERB)
            .add_work(crate::waystone::CALLING)
            .add_work(crate::captives::FREEING);
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
        // Trading, paid in the one tag every piece of gold carries, so the
        // engine counts a purse and a price without knowing a coin from
        // anything else Undermere calls an item.
        app.insert_resource(Currency(library.registries.tags.expect("gold")));
        // The gold carried, said on the rail in the coin's own colour.
        let (r, g, b) = library.armory.defs.get(library.armory.defs.expect(crate::finds::GOLD)).color;
        app.add_tone(crate::shops::GOLD_TONE, Color::srgb(r, g, b));
        for (name, (r, g, b)) in [crate::doors::MARK_TONE, crate::doors::MARK_PULSE_TONE].into_iter().zip(crate::doors::MARK_COLORS) {
            app.add_tone(name, Color::srgb(r, g, b));
        }
        crate::mercenaries::declare_tone(app, &library.bestiary);
        // The three brains a hired mercenary thinks with, one an order,
        // built once from its kind beside the kind's own ally mind.
        let mercenary = library.bestiary.defs.get(library.bestiary.defs.expect(crate::mercenaries::MERCENARY));
        app.insert_resource(crate::mercenaries::MercenaryBrains::new(mercenary));
        app.insert_resource(library.registries).insert_resource(library.abilities).insert_resource(library.armory).insert_resource(library.bestiary);
        app.insert_resource(library.faces);
        // What Mereholt's keepers may stock, read once with the rest: each
        // stocking is drawn from it, never written back.
        app.insert_resource(library.wares);
        // The temple's rites, read with the rest, each a verb the priest
        // offers: declared here, before any priest is made to offer it.
        for rite in library.rites.iter() {
            app.add_verb(&rite.verb);
        }
        app.insert_resource(library.rites);
        // Mereholt's people and the names they are given, read with the
        // rest: who the people are never changes, and who fills each role
        // is drawn by every run's start. Talking is a verb a person
        // offers, and each role's lines are said in a tone of its own.
        app.add_verb(crate::townsfolk::TALK);
        crate::townsfolk::declare_tones(app, &library.townsfolk);
        app.insert_resource(library.townsfolk).insert_resource(library.names).insert_resource(library.rumours);
        app.init_resource::<crate::townsfolk::Folk>().reset_on_new_run::<crate::townsfolk::Folk>();
        app.init_resource::<crate::townsfolk::GivenNames>().reset_on_new_run::<crate::townsfolk::GivenNames>();
        // The swords for hire at the Sunken Lantern: what they may carry
        // and ask, read with the rest; `hire`, the verb a candidate offers
        // beside talk, and `give` and `dismiss`, a hired one's; and how
        // many visits the run has made, which says which three wait there.
        // The tone they speak in is their kind's, declared above while the
        // bestiary is still to hand.
        app.add_verb(crate::mercenaries::HIRE).add_verb(crate::mercenaries::GIVE).add_verb(crate::mercenaries::DISMISS);
        app.insert_resource(library.mercenaries);
        app.init_resource::<crate::mercenaries::Tavern>().reset_on_new_run::<crate::mercenaries::Tavern>();
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
        // The lines `assets/writings.ron` holds, read once; and which of
        // them a run has already given out, none at a run's start, and
        // what a continued run's save says.
        app.insert_resource(crate::writings::load());
        app.init_resource::<crate::writings::WritingsProgress>().reset_on_new_run::<crate::writings::WritingsProgress>();
        // Undermere's reactions in their steps, ordered against the
        // engine's own phases: see `ReactStep`.
        app.configure_sets(Turn, (ReactStep::Fill, ReactStep::Answer, ReactStep::Keep, ReactStep::Count).chain().in_set(TurnSet::React))
            .configure_sets(Turn, ReactStep::Fill.after(PrefabSet::Fill).before(ReactSet::Settle).before(AmmoSet::Resolve))
            .configure_sets(Turn, ReactStep::Answer.after(ReactSet::Settle).after(AmmoSet::Resolve).after(LootSet::Scatter));
        // The run's start, and a continued run after the fresh start has
        // stood aside.
        app.add_systems(NewRun, crate::run::start);
        app.add_systems(NewRun, crate::run::resume.after(crate::run::start));
        // A continued run lays its bodies back down in `resume`, with the
        // glyph they lived with: drawn as corpses once it has.
        app.add_systems(NewRun, crate::corpses::dress_the_dead.after(crate::run::resume));
        // The doors the adventurer has walked through, kept with the run,
        // noted among the answers, and the marks the key with the input
        // puts on the ones not yet, drawn over the map.
        app.init_resource::<crate::doors::Passed>().reset_on_new_run::<crate::doors::Passed>();
        app.init_resource::<crate::doors::DoorTiles>().init_resource::<crate::doors::DoorMarks>().reset_on_new_run::<crate::doors::DoorMarks>();
        app.add_systems(Update, crate::doors::draw_door_marks.in_set(PresentSet::Overlay));
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
        // Which stocking Mereholt's shelves are, begun again with each run.
        app.init_resource::<crate::shops::Shelves>().reset_on_new_run::<crate::shops::Shelves>();
        app.add_observer(crate::tally::note_departure);
        // What a turn did, counted inside it: the fighting and what was
        // found and drunk into the tally, and a boss's death into the fact
        // the quest counts, and the win when it finishes the quest. Last,
        // after every answer, so what an answer hands over or kills is
        // counted the pass it happens.
        app.add_systems(
            Turn,
            (crate::tally::count_the_fighting, crate::tally::count_what_is_found, crate::quest::report_bosses).chain().in_set(ReactStep::Count),
        );
        // The run written down the frame it is over, however it ended.
        app.add_systems(OnEnter(EngineState::Over), crate::tally::record);
        // A place fills the moment it is first entered: the engine's slots
        // in `PrefabSet::Fill`, then the depth, the ways through and the
        // place's line in the log, the floor's people, Mereholt's people at
        // their marks, the keepers' shelves filled or restocked, the
        // tavern's candidates seated on every arrival in Mereholt, and the
        // floor's things, before the engine's loot in
        // `LootSet::Scatter`, each reading the same `PlaceEntered` through
        // its own cursor. After the slots, so the people keep off every
        // cell a piece marked; the things after the ways through, which
        // they keep off. Chained, though no two of them share a cell:
        // systems that spawn, left unordered, queue their commands in
        // whatever order they finish in, and a place's entities would come
        // out in a different order from one run to the next.
        app.add_systems(
            Turn,
            (
                crate::tally::remember_depth,
                crate::stairs::link_places,
                crate::population::populate,
                crate::townsfolk::gather,
                crate::shops::keep_shops,
                crate::mercenaries::seat_candidates,
                crate::finds::lay,
            )
                .chain()
                .in_set(ReactStep::Fill)
                .before(LootSet::Scatter),
        );
        // What the adventurer and those around them are, kept in step every
        // pass from the stats as they stand, after the engine has folded
        // what is worn onto them. First what puts a status or a stat on
        // somebody: what a worn thing lends, fire immunity and discord kept
        // on whoever holds them, a Choirmaster's choir, a troll's
        // regeneration halted while it burns, the Crown of Grubnak's aura.
        // Then what reads the stats those leave: what too heavy a thing
        // costs, how far the lantern reaches, how sneaky the stealth stat
        // makes the adventurer, and what the rings give that the engine
        // does not read itself, health back over time, a staff's pace and
        // what a blow puts towards its next charge, at the pace this pass's
        // wisdom keeps, and how easily what is hidden is spotted.
        app.add_systems(
            Turn,
            (
                crate::wearing::lend,
                crate::conditions::keep_fireproof,
                crate::conditions::keep_discord,
                crate::tricks::sing_choir,
                crate::tricks::keep_regen_halted,
                crate::artifacts::crown_aura,
                crate::strength::keep_strength,
                crate::light::dim_the_lantern,
                crate::stats::keep_stealth,
                crate::stats::regenerate,
                crate::staffs::keep_pace,
                crate::staffs::reap,
                crate::stats::keep_keen,
            )
                .chain()
                .in_set(ReactStep::Keep),
        );
        // How loud every step is, read off the footfall stat at the start
        // of every pass, before anyone steps in it: what is worn is folded
        // onto the stat in the reactions, unordered against these.
        app.add_systems(Turn, crate::stats::keep_footfall.in_set(TurnSet::Schedule));
        // What a zap at somewhere strikes, weighed between the actions that
        // make one and the triggers that land it, so a wand zapped at
        // nobody is not learned by it; and what any use showed nothing of,
        // which the reactions above read and empty.
        app.init_resource::<crate::effects::ShowedNothing>();
        app.add_systems(Turn, crate::lore::weigh_landings.in_set(TurnSet::Resolve).after(ResolveSet::Act).before(ResolveSet::Triggers));
        // A walk into an ally or into wildlife a change of places with it,
        // by the engine's bump rules.
        let bumps = crate::wildlife::bump_rules(app.world().resource::<Registries>());
        app.insert_resource(bumps);
        // What a hired mercenary knows of what it carries and sees; what a
        // monster throws, only what strikes; and whether it has stolen
        // something and should run with it. In that order, so what it
        // throws and runs with is chosen among what it knows, and all three
        // after every annotation and before the engine weighs the
        // snapshot, so it is weighed as the game rewrote it.
        app.add_systems(
            Turn,
            (crate::mercenaries::know_as_the_adventurer_does, crate::bestiary::throw_what_strikes, crate::tricks::sense_stole)
                .chain()
                .in_set(DecideSet::Perceive)
                .after(PerceiveSet::Annotate)
                .before(PerceiveSet::Weigh),
        );
        // The lived-in dungeon: a patrolling mind's next
        // waypoint, sensed beside the engine's own post and leader, and
        // never for an actor with no route of its own.
        app.add_systems(Turn, crate::tricks::sense_patrols.in_set(PerceiveSet::Annotate));
        app.add_stream::<crate::conditions::Stumbles>("UndermerePlugin");
        app.add_systems(Turn, crate::conditions::stumble.in_set(DecideSet::Game));
        add_answers(app);
        // What a floor hides: its traps and flagstones stood up on its
        // first arrival. A trap seen going off, and what the adventurer
        // spotted, are among the answers.
        app.insert_resource(crate::traps::Hiding::load());
        app.add_systems(Turn, crate::traps::stand_hidden.in_set(ReactStep::Fill));
        // The risk-and-reward pieces' own: the marks a piece leaves for the
        // game to fill, after the engine's own `PrefabSet::Fill` has filled
        // every slot beside them. A lever, a vault's key and what lifting a
        // prize sets off are among the answers.
        app.add_systems(Turn, crate::vaults::spawn_vault_marks.in_set(ReactStep::Fill));
        // A cold forge's anvil and a moonwell choose a thing first, among
        // what these name, asked as the offer is taken up.
        let wells = crate::vaults::wells::WellChoices {
            temper: app.register_system(crate::vaults::wells::temper_choices),
            tell: app.register_system(crate::vaults::wells::tell_choices),
        };
        app.insert_resource(wells);
        app.add_systems(Turn, crate::vaults::wells::offer_at_wells.in_set(TurnSet::Schedule).before(NamesKept));
        // The lived-in dungeon's own: a readable prop's line, drawn once
        // on the arrival that finds it, and a floor's own feeling, said
        // the same arrival.
        app.add_systems(Turn, (crate::writings::assign_writings, crate::writings::tell_floor_feeling).chain().in_set(ReactStep::Fill).after(crate::finds::lay));
        // Two answers that read the whole world, and so would be unordered
        // against every reaction: a conjured blade gone when its time is
        // up, and a rite answered. After the reactions and before the
        // facts, so what either does is counted as a fact this pass.
        app.add_systems(Turn, (crate::allies::fade, crate::temple::answer_rites).chain().after(TurnSet::React).before(FactSet));
        // A hired mercenary that falls taken out of the party once the tally has counted
        // it, and its death said as the adventurer's own loss.
        app.init_resource::<crate::mercenaries::Fell>();
        app.add_systems(Turn, crate::mercenaries::strip_the_fallen.in_set(ReactStep::Count).after(crate::tally::count_the_fighting));
        app.add_systems(Update, (crate::mercenaries::say_who_fell, crate::mercenaries::say_who_came).chain().in_set(ViewSet::Annotate));
        // What a hired mercenary knows of what it carries and sees: its own
        // kit whole, and anything else only as the adventurer knows it; a
        // thing it is better off wearing put on by the engine's own rule;
        // and what `give` chooses among, asked as the offer is taken up.
        app.add_choice::<crate::mercenaries::PutOn>();
        let among = app.register_system(crate::mercenaries::give_choices);
        app.insert_resource(crate::mercenaries::GiveAmong(among));
        app.add_systems(Turn, crate::mercenaries::put_on.in_set(ResolveSet::Redirect));
        app.add_systems(Turn, crate::mercenaries::release_own_kit.in_set(CleanupSet::Requeue));
        // `hire` kept off while the party is full, counted at the start of
        // every pass.
        app.add_systems(Turn, crate::mercenaries::keep_hire_offered.in_set(TurnSet::Schedule));
        // Mereholt's people: a wanderer kept to its beat beside the
        // engine's own senses, and a lamplighter's round laid at the start
        // of a pass once the lamps stand.
        app.add_systems(Turn, crate::townsfolk::sense_beats.in_set(PerceiveSet::Annotate));
        app.add_systems(Turn, crate::townsfolk::walk_rounds.in_set(TurnSet::Schedule));
        // A waystone refused in Mereholt before its use is spent. The call
        // answered, or said to be broken off, is among the answers.
        app.add_systems(Turn, crate::waystone::refuse_at_home.in_set(ResolveSet::Travel));
        // The shriek a scroll of aggravate monsters lets out.
        app.add_sound(crate::effects::minds::SHRIEK_SOUND);
        // A drinker of descent set down on the floor below once it stands:
        // after the arrival has built and filled it.
        app.add_systems(Turn, crate::effects::moving::land_sinking.in_set(ReactStep::Fill).after(crate::finds::lay));
        // Every thing called what is known of it: after the narrator has
        // read the pass, so nothing is told of under a name it had not yet
        // earned, and before the spent are taken out; at the start of every
        // pass, for what came into the world between the passes, a
        // continued run's things, before anything in the pass is told of;
        // and before the frame is drawn, for a frame no pass runs in.
        app.add_systems(Turn, crate::lore::keep_names.in_set(TurnSet::Cleanup).before(CleanupSet::Remove));
        app.add_systems(Turn, crate::lore::keep_names.in_set(TurnSet::Schedule).in_set(NamesKept));

        // Every thing's `Worth` current with what is known of it, at the
        // same three moments as its name, since a shop or a sale reads it
        // no differently from how the log reads a name.
        app.add_systems(Turn, crate::armory::keep_worth.in_set(TurnSet::Cleanup).before(CleanupSet::Remove));
        app.add_systems(Turn, crate::armory::keep_worth.in_set(TurnSet::Schedule));

        // Between the turns and the light, behind the photograph's asks
        // below, and chained, since four of these read the whole world: a
        // scroll that asks which thing to read it on named as it asks, in
        // the frame the keys turned the pack to choosing; a thing bought
        // from a keeper known for what the shelf named it, the deal struck
        // before the turns; what a deal hands the adventurer marked as
        // theirs, not a find, before the next pickup could count it; and
        // then the frame's names and worths, so the pack calls a thing by
        // its name the frame it lands there.
        app.add_systems(
            Update,
            (crate::lore::learn_by_asking, crate::shops::learn_by_buying, crate::tally::count_deals, crate::lore::keep_names, crate::armory::keep_worth)
                .chain()
                .in_set(AfterTurns)
                .after(crate::run::stand_near),
        );
        // What the panels count of a worn thing not yet known: its kind's
        // plain numbers, over the engine's true ones, for the vitals' armor,
        // the look cursor's forecast and the targeting box's chance; and
        // what the pack and the look cursor say of a thing whose kind is
        // known and whose level is not. Each reads its view when the game
        // draws it and does nothing where it does not.
        // Then what a thing asks of the adventurer's strength, under the
        // pack's row and the look cursor's, so the line reads under its
        // numbers; and the gold carried, on the vitals under the gauges,
        // wherever the game draws them. Chained, since a row's facets read
        // in the order they are pushed.
        app.add_systems(
            Update,
            (
                crate::seeming::seem_armor,
                crate::seeming::seem_forecast,
                crate::seeming::seem_aim,
                crate::seeming::seem_pack,
                crate::seeming::seem_looked_at,
                crate::strength::note_strength,
                crate::shops::count_gold,
            )
                .chain()
                .in_set(ViewSet::Annotate),
        );
        // A photograph's ask to stand near something on the floor it
        // starts on, which a real run never makes.
        app.configure_sets(Update, AfterTurns.after(EngineSet::Turns).before(EngineSet::Light));
        app.add_systems(
            Update,
            crate::run::stand_near.in_set(AfterTurns).run_if(resource_exists::<crate::run::StartNear>).run_if(in_state(EngineState::Playing)),
        );
        // And its ask for a party, once that stand has been taken: before
        // `stand_near` in the frame, so the frame that asks the warp does
        // not hire beside the cell the adventurer is about to leave.
        app.add_systems(
            Update,
            crate::run::hire_party
                .in_set(AfterTurns)
                .before(crate::run::stand_near)
                .run_if(resource_exists::<crate::run::StartParty>)
                .run_if(not(resource_exists::<crate::run::StartNear>))
                .run_if(in_state(EngineState::Playing)),
        );
        // The keys, declared once so the controls screen lists what the
        // input reads. `f` and `t` hand the engine's targeting cursor an
        // `AimFire` or an `AimThrow`; registering both here is a no-op
        // beside the cursor's own plugin, and lets a headless test with no
        // cursor press the keys all the same.
        crate::input::declare_controls(app);
        app.add_message::<AimFire>().add_message::<AimThrow>();
        // A door marked, the adventurer's keys, the party's orders: chained,
        // since each may open a screen.
        app.add_systems(Update, (crate::doors::mark_doors, crate::input::player_input, crate::mercenaries::order_party).chain().in_set(EngineSet::Input));
        // A lair's own band, led the moment its prefab's slots are
        // filled. Its own call rather than folded into the chain above,
        // since nothing in that chain's own order depends on this nor
        // this on it.
        app.add_systems(Turn, crate::population::lead_lair_bands.in_set(ReactStep::Fill));
        // The two artifacts' own clocks: the Crown of Grubnak's aura on
        // whoever stands beside its wearer, recomputed whole every pass,
        // and the Abbess's Chalice refilled the moment a new floor is
        // reached. Neither is a worn thing's own `bestows` or a trigger
        // can reach: one lands on somebody other than the wearer, the
        // other refills on a clock no `recharge` ticks.
        app.add_systems(Turn, crate::artifacts::refill_chalice.in_set(ReactStep::Fill));
    }

    /// Every line Undermere says from inside a turn is a `Tell`, which
    /// only the engine's narrator collects and speaks.
    fn finish(&self, app: &mut App) {
        rl_engine::rl_bevy::plugin::depends_on::<rl_engine::rl_ui::NarrationViewPlugin>(app, "UndermerePlugin");
    }
}

/// Undermere's frame between the turns and the light: the photograph's
/// asks, then what a deal hands over, then every thing's name and worth,
/// so what is drawn and lit is the world as the turns and the asks left
/// it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct AfterTurns;

/// Where every thing is called what is known of it at the start of a
/// pass, for what is offered there to be named first.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct NamesKept;

/// Undermere's answers to what a pass did, in [`ReactStep::Answer`], in
/// the order they are written here.
///
/// Work done or broken off first: a shackled captive's own "free", the
/// work at the shackles begun and answered, and a waystone's call. Then
/// every offer taken up: a vault's lever and key, a moonwell or an
/// anvil, something read, somebody talked to, a mercenary hired, given
/// to or sent away. Then the map changed by what was done: a door walked
/// through, a trap sprung or spotted, a lift's cage, its vermin, its
/// floor giving way or flooding, and the party after the adventurer
/// through every way and warp. Then the bestiary's own tricks that read
/// what a turn did: a blow that splits what it left alive or mends its
/// own drainer, a step that leaves fire, the Widow's egg sacs, the Mother
/// Below's young and the Drowned King's pool. Then what is learned: what
/// the adventurer's hands showed of their things, a curse closing on
/// them, how long wearing a thing tells its level, the turns they have
/// worn what they do not know, and what is known of a kind's tricks, each
/// only once its effect lands where the adventurer can see it. Last,
/// every body the turn left, drawn as a corpse of its kind.
///
/// One chain, since most of them write the log, the map, a bag or what
/// the adventurer knows, and an order the scheduler picked would say the
/// pass differently from one build to the next.
fn add_answers(app: &mut App) {
    use crate::vaults::lifts::*;
    let work = (crate::captives::answer_freeing, crate::captives::answer_free, crate::waystone::answer_the_call, crate::waystone::broken_off).chain();
    let offers = (
        crate::vaults::answer_pull_lever,
        crate::vaults::spend_vault_key,
        crate::vaults::wells::answer_wells,
        crate::writings::answer_read,
        crate::townsfolk::answer_talk,
        crate::mercenaries::answer_candidates,
        crate::mercenaries::answer_hire,
        crate::mercenaries::answer_give,
        crate::mercenaries::answer_dismiss,
    )
        .chain();
    let map = (
        crate::doors::note_doors_passed,
        crate::traps::show_sprung,
        crate::traps::tell_spotted,
        shut_other_cages,
        loose_the_vermin,
        give_way,
        collapse_behind,
        start_flood,
        rise_floods,
        crate::mercenaries::follow_through,
    )
        .chain();
    let tricks = (
        crate::tricks::judge_splits,
        crate::tricks::judge_draining,
        crate::tricks::trail_fire,
        crate::tricks::hatch_beside,
        crate::tricks::spawn_young,
        crate::tricks::die_with_mother,
        crate::tricks::rise_from_the_pool,
    )
        .chain();
    let learned = (
        crate::lore::learn_by_use,
        crate::enchant::bind_on_wear,
        crate::lore::tell_unfamiliar,
        crate::lore::grow_familiar,
        crate::bestiary_lore::learn_from_abilities,
        crate::bestiary_lore::learn_from_triggers,
        crate::bestiary_lore::learn_from_statuses,
        crate::bestiary_lore::learn_fire_trail,
        crate::bestiary_lore::learn_draining,
        crate::bestiary_lore::learn_reflecting,
    )
        .chain();
    app.add_systems(Turn, (work, offers, map, tricks, learned, crate::corpses::dress_the_dead).chain().in_set(ReactStep::Answer));
}
