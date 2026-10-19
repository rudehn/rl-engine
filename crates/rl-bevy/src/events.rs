//! Facts as messages, and the quest tracker as a resource.
//!
//! A game writes a [`Happened`] for each outcome it wants quests and
//! counters to hear about. If it inserted a [`Quests`] resource, the
//! engine feeds every fact through the tracker and reports each change as
//! a [`QuestChange`]; if it inserted a [`Counters`] resource, the ledger is
//! fed too. Both are opt-in: a game with no quests inserts neither and
//! pays nothing. Both are a run's, and forgotten when it ends.
//!
//! Facts are fed inside the turn, in [`FactSet`], straight after
//! [`TurnSet::React`](crate::plugin::TurnSet::React) where a game writes
//! them, so a fact finishes its quest in the pass that raised it; and the
//! engine ends the run won there, with a [`RunOver`], when the quest it
//! finished is marked `victory`, before any later pass of the frame can
//! end it otherwise. A fact written outside a pass, by a system in
//! `Update`, is fed once the frame's turns are done.
//!
//! A game's quests are a file, read with
//! [`rl_rules::events::quest::load`]; the game keeps only its list of what
//! an objective counts.

use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use rl_rules::Registry;
use rl_rules::{Change, Fact, Ledger, QuestDef, Tracker};

use crate::state::RunOver;

/// Something happened that facts should record.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Deref)]
pub struct Happened(pub Fact);

/// The quest definitions and where every quest stands.
#[derive(Resource)]
pub struct Quests {
    /// The definitions.
    pub defs: Registry<QuestDef>,
    /// The state.
    pub tracker: Tracker,
}

impl Quests {
    /// Fresh state over `defs`.
    pub fn new(defs: Registry<QuestDef>) -> Self {
        let tracker = Tracker::new(&defs);
        Self { defs, tracker }
    }
}

/// Named counters fed by facts.
#[derive(Resource, Deref, DerefMut)]
pub struct Counters(pub Ledger);

/// A quest moved. Written in the pass whose fact moved it, in
/// [`FactSet`], so a system later in the same pass, in
/// [`TurnSet::Record`](crate::plugin::TurnSet::Record), reads it there,
/// and one in `Update` reads it once the frame's turns are done.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Deref)]
pub struct QuestChange(pub Change);

/// Where facts are fed to the tracker and the ledger inside a pass:
/// straight after [`TurnSet::React`](crate::plugin::TurnSet::React), where
/// a game answers a death or a verb with a [`Happened`], and before
/// [`TurnSet::Listen`](crate::plugin::TurnSet::Listen) and the record, so
/// the pass's reactions all land first and the record reads the quest as
/// the pass left it.
#[derive(bevy::ecs::schedule::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FactSet;

/// How far the facts have been fed, one cursor for the feeding in the pass
/// and the one after the frame's turns, so neither feeds a fact the other
/// already has.
#[derive(Resource, Default)]
pub(crate) struct FactsRead(MessageCursor<Happened>);

/// Feeds every fact not yet fed to the tracker and the ledger, and ends
/// the run won when one finishes a quest marked `victory`, saying the
/// quest's `ending`.
///
/// In the pass, in [`FactSet`], so a victory is the first word on the
/// run's end: the run ends for the first [`RunOver`] of the frame, and a
/// blow in a later pass of the frame, which a mind still strikes, comes
/// after it. Again in `PostUpdate`, for a fact written outside the turns,
/// where no pass would feed it until the next one ran, which a held turn
/// may put off for longer than a message is kept.
pub(crate) fn track_facts(
    facts: Res<Messages<Happened>>,
    mut read: ResMut<FactsRead>,
    quests: Option<ResMut<Quests>>,
    counters: Option<ResMut<Counters>>,
    mut changes: MessageWriter<QuestChange>,
    mut over: MessageWriter<RunOver>,
) {
    let (mut quests, mut counters) = (quests, counters);
    for fact in read.0.read(&facts) {
        if let Some(q) = quests.as_deref_mut() {
            let Quests { defs, tracker } = q;
            for change in tracker.feed(&fact.0, defs) {
                if let Change::QuestDone { quest, victory: true } = change {
                    over.write(RunOver::won().saying(defs.get(quest).ending.clone()));
                }
                changes.write(QuestChange(change));
            }
        }
        if let Some(c) = counters.as_deref_mut() {
            c.0.feed(&fact.0);
        }
    }
}

/// Forgets what the run that is ending fed: every counter back at nought
/// with the rules that feed it kept, and every quest where a fresh tracker
/// over the same definitions puts it.
///
/// In [`EndRun`](crate::plugin::EndRun) rather than left to each game's
/// start, and not `reset_on_new_run`, whose constructor is a plain `fn`
/// that cannot keep a game's own counters and quests: a game that inserts
/// them once, with the rest of its content, would otherwise begin its
/// second run in a launch with the first one's counts, and a quest
/// counting them could finish before the run had done anything. A game
/// that inserts them afresh in `NewRun` loses nothing, and a run left for
/// the title has been written down before its teardown reaches here, so
/// Continue gives back whatever this forgets.
fn forget_facts(counters: Option<ResMut<Counters>>, quests: Option<ResMut<Quests>>) {
    if let Some(mut counters) = counters {
        counters.0.reset();
    }
    if let Some(mut quests) = quests {
        let Quests { defs, tracker } = &mut *quests;
        *tracker = Tracker::new(defs);
    }
}

/// Facts: what happened, tallied into counters and matched against
/// quests, and the run won when a quest marked `victory` is done. Needs
/// one of [`Quests`] or [`Counters`] before play begins; a game that
/// wants both inserts both, and either may be inserted once for every
/// run, since both are forgotten when a run ends.
pub struct FactsPlugin;

impl Plugin for FactsPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{Turn, TurnSet};
        use crate::state::EngineState;
        app.add_message::<Happened>()
            .add_message::<QuestChange>()
            .init_resource::<FactsRead>()
            .configure_sets(Turn, FactSet.after(TurnSet::React).before(TurnSet::Listen))
            // Either will do: a game may track quests, counters, or both.
            .add_systems(OnEnter(EngineState::Playing), |quests: Option<Res<Quests>>, counters: Option<Res<Counters>>| {
                assert!(quests.is_some() || counters.is_some(), "FactsPlugin needs Quests or Counters inserted before EngineState::Playing");
            })
            .add_systems(Turn, track_facts.in_set(FactSet))
            .add_systems(PostUpdate, track_facts.before(crate::state::end_runs).run_if(in_state(EngineState::Playing)))
            .add_systems(crate::plugin::EndRun, forget_facts);
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "FactsPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use rl_rules::{FactDef, FactKind, Matcher, Objective, QuestId, QuestState};

    /// A game that tracks a quest of two kills and counts its kills, both
    /// inserted once, before any run, as a game loading its content up
    /// front would: the fact kind and the counter, for feeding and reading.
    fn tracking() -> (App, FactKind, rl_rules::CounterId) {
        let mut app = headless_app();
        app.add_plugins(FactsPlugin);
        let facts = Registry::from_defs(vec![FactDef::new("killed")]).unwrap();
        let killed: FactKind = facts.expect("killed");
        let defs = Registry::from_defs(vec![QuestDef {
            name: "two".into(),
            title: "Two kills".into(),
            text: String::new(),
            after: vec![],
            objectives: vec![Objective::count("Kill two", Matcher::any(killed), 2)],
            victory: true,
            ending: "Two down.".into(),
        }])
        .unwrap();
        let counters = Registry::from_defs(vec![rl_rules::CounterDef::new("kills")]).unwrap();
        let kills = counters.expect("kills");
        app.insert_resource(Quests::new(defs));
        app.insert_resource(Counters(Ledger::new(&counters).tally(Matcher::any(killed), kills)));
        (app, killed, kills)
    }

    /// Begins play: a run's start, as far as facts care.
    fn play(app: &mut App) {
        // Play needs a map, and says so if there is none; facts need nothing
        // on it.
        app.insert_resource(crate::world::WorldMap::new(rl_grid::TileRegistry::standard().tables()));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
    }

    #[test]
    fn facts_move_quests_and_counters_between_frames() {
        let (mut app, killed, kills) = tracking();
        play(&mut app);
        app.world_mut().write_message(Happened(Fact::new(killed).about(1)));
        app.update();
        app.world_mut().write_message(Happened(Fact::new(killed).about(1)));
        app.update();
        let changes: Vec<Change> = app.world_mut().resource_mut::<Messages<QuestChange>>().drain().map(|c| c.0).collect();
        let quest = QuestId::from_raw(0);
        assert!(changes.contains(&Change::QuestDone { quest, victory: true }), "{changes:?}");
        assert_eq!(app.world().resource::<Quests>().tracker.state(quest), QuestState::Done);
        assert_eq!(app.world().resource::<Counters>().get(kills), 2);
    }

    /// A run begun after another in the same launch starts from nothing:
    /// the counters the last run fed are back at nought and its quest is
    /// open again, while the rules that feed them and the quests themselves
    /// stand, so the new run's first kill counts one and not three.
    #[test]
    fn a_run_begun_after_another_starts_with_empty_counters_and_its_quests_open() {
        let (mut app, killed, kills) = tracking();
        play(&mut app);
        for _ in 0..2 {
            app.world_mut().write_message(Happened(Fact::new(killed).about(1)));
            app.update();
        }
        let quest = QuestId::from_raw(0);
        assert_eq!(app.world().resource::<Quests>().tracker.state(quest), QuestState::Done, "the first run finished its quest");
        app.world_mut().write_message(crate::state::Restart::fresh());
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Counters>().get(kills), 0, "the next run has counted nothing");
        let quests = app.world().resource::<Quests>();
        assert_eq!((quests.tracker.state(quest), quests.tracker.progress(quest, 0, &quests.defs)), (QuestState::Open, (0, 2)), "and its quest is open");
        play(&mut app);
        app.world_mut().write_message(Happened(Fact::new(killed).about(1)));
        app.update();
        assert_eq!(app.world().resource::<Counters>().get(kills), 1, "the rules still feed the counter, from nought");
        let quests = app.world().resource::<Quests>();
        assert_eq!(quests.tracker.progress(quest, 0, &quests.defs), (1, 2), "and the quest counts again");
    }

    /// Where the blow that would lose the run stands: armed by the pass
    /// that wins it, and struck in the next.
    #[derive(Resource, Default)]
    struct Blow {
        armed: bool,
        struck: bool,
    }

    /// The player's own act finishes the victory quest: a fact written in
    /// `React`, as a game answers a boss's death.
    fn fell_the_boss(
        mut done: MessageReader<crate::turn::ActionDone>,
        players: Query<(), With<crate::components::Player>>,
        mut happened: MessageWriter<Happened>,
        mut blow: ResMut<Blow>,
        facts: Res<Killed>,
    ) {
        for d in done.read() {
            if players.contains(d.actor) && !blow.armed {
                happened.write(Happened(Fact::new(facts.0).about(1)));
                happened.write(Happened(Fact::new(facts.0).about(1)));
                blow.armed = true;
            }
        }
    }

    /// The pass after: a blow that kills the player, as a monster's later
    /// turn in the same frame would.
    fn strike_back(mut blow: ResMut<Blow>, mut over: MessageWriter<crate::state::RunOver>) {
        if blow.armed && !blow.struck {
            blow.struck = true;
            over.write(crate::state::RunOver::died(None));
        }
    }

    #[derive(Resource)]
    struct Killed(FactKind);

    /// The race a win in its own pass is for. The player's turn finishes
    /// the quest marked victory, and a later pass of the same frame, an
    /// actor quicker than the player taking its turns before the player's
    /// next, brings a blow that would end the run lost. The quest was won
    /// first, so the run is won, with the quest's own words.
    #[test]
    fn a_run_won_in_one_pass_stays_won_though_a_later_pass_of_the_frame_would_lose_it() {
        let (mut app, killed, _) = tracking();
        app.insert_resource(Killed(killed)).init_resource::<Blow>();
        app.add_systems(crate::plugin::Turn, (strike_back, fell_the_boss).chain().in_set(crate::plugin::TurnSet::React));
        let start = crate::testing::ground(&mut app);
        use crate::components::{Actor, Blocks, MyTurn, Player, Position, Speed};
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start))).id();
        app.world_mut().spawn((Actor, Blocks, Position(start.offset(2, 0)), Speed(200)));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds the first turn");
        app.world_mut().write_message(crate::turn::Intent::new(player, crate::turn::Wait));
        app.update();
        assert!(app.world().resource::<Blow>().struck, "the blow came in the frame the quest was won");
        let ending = app.world().get_resource::<crate::state::Ending>().expect("the run ended in that frame");
        assert_eq!((ending.outcome, ending.epitaph.as_str()), (crate::state::Outcome::Won, "Two down."), "won all the same, in the quest's own words");
    }
}
