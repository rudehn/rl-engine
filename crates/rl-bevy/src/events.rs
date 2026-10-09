//! Facts as messages, and the quest tracker as a resource.
//!
//! A game writes a [`Happened`] for each outcome it wants quests and
//! counters to hear about. If it inserted a [`Quests`] resource, the
//! engine feeds every fact through the tracker after the frame's systems
//! and reports each change as a [`QuestChange`]; if it inserted a
//! [`Counters`] resource, the ledger is fed too. Both are opt-in: a game
//! with no quests inserts neither and pays nothing. Both are a run's, and
//! forgotten when it ends.

use bevy::prelude::*;
use rl_rules::Registry;
use rl_rules::{Change, Fact, Ledger, QuestDef, Tracker};

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

/// A quest moved. Written after the frame's game systems, read by them
/// the next frame.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Deref)]
pub struct QuestChange(pub Change);

/// Feeds the frame's facts to the tracker and the ledger.
pub fn track_facts(
    mut facts: MessageReader<Happened>,
    quests: Option<ResMut<Quests>>,
    counters: Option<ResMut<Counters>>,
    mut changes: MessageWriter<QuestChange>,
) {
    let (mut quests, mut counters) = (quests, counters);
    for fact in facts.read() {
        if let Some(q) = quests.as_deref_mut() {
            let Quests { defs, tracker } = q;
            for change in tracker.feed(&fact.0, defs) {
                changes.write(QuestChange(change));
            }
        }
        if let Some(c) = counters.as_deref_mut() {
            c.0.feed(&fact.0);
        }
    }
}

/// Whether a game inserted anything that listens to facts.
pub fn anyone_listening(quests: Option<Res<Quests>>, counters: Option<Res<Counters>>) -> bool {
    quests.is_some() || counters.is_some()
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
/// quests. Needs one of [`Quests`] or [`Counters`] before play begins;
/// a game that wants both inserts both, and either may be inserted once
/// for every run, since both are forgotten when a run ends.
pub struct FactsPlugin;

impl Plugin for FactsPlugin {
    fn build(&self, app: &mut App) {
        use crate::state::EngineState;
        app.add_message::<Happened>()
            .add_message::<QuestChange>()
            // Either will do: a game may track quests, counters, or both.
            .add_systems(OnEnter(EngineState::Playing), |quests: Option<Res<Quests>>, counters: Option<Res<Counters>>| {
                assert!(quests.is_some() || counters.is_some(), "FactsPlugin needs Quests or Counters inserted before EngineState::Playing");
            })
            .add_systems(PostUpdate, track_facts.run_if(in_state(EngineState::Playing)))
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
}
