//! Facts as messages, and the quest tracker as a resource.
//!
//! A game writes a [`Happened`] for each outcome it wants quests and
//! counters to hear about. If it inserted a [`Quests`] resource, the
//! engine feeds every fact through the tracker after the frame's systems
//! and reports each change as a [`QuestChange`]; if it inserted a
//! [`Counters`] resource, the ledger is fed too. Both are opt-in: a game
//! with no quests inserts neither and pays nothing.

use bevy::prelude::*;
use rl_content::Registry;
use rl_events::{Change, Fact, Ledger, QuestDef, Tracker};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use rl_events::{FactDef, FactKind, Matcher, Objective, QuestId, QuestState};

    #[test]
    fn facts_move_quests_and_counters_between_frames() {
        let mut app = headless_app();
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
        let counters = Registry::from_defs(vec![rl_events::CounterDef::new("kills")]).unwrap();
        let kills = counters.expect("kills");
        app.insert_resource(Quests::new(defs));
        app.insert_resource(Counters(Ledger::new(&counters).tally(Matcher::any(killed), kills)));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
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
}
