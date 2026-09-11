//! Quests: objectives over facts, unlocked by other quests.
//!
//! A quest definition is a list of objectives, each a [`Matcher`] and what
//! it needs: a total reached by adding matching amounts, or a latest
//! amount at least so large. A quest opens when every quest it comes
//! after is done, and is done when every objective is. The [`Tracker`]
//! holds the state and reports every change as it feeds facts, so a game
//! narrates, rewards or ends the run from the changes and nothing else.

use rl_content::{Named, Registry};
use rl_core::Id;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, Matcher};

/// What an objective needs of the facts it matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Need {
    /// The amounts of matching facts, added up, reach this.
    Total(i64),
    /// The most recent matching fact's amount is at least this.
    Latest(i64),
}

/// One step of a quest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Objective {
    /// What the player is told to do.
    pub text: String,
    /// Which facts count.
    pub on: Matcher,
    /// How much of them.
    pub need: Need,
}

impl Objective {
    /// An objective needing `n` matching facts in total.
    pub fn count(text: impl Into<String>, on: Matcher, n: i64) -> Self {
        Self { text: text.into(), on, need: Need::Total(n) }
    }

    /// An objective needing a matching fact of amount at least `n`.
    pub fn reach(text: impl Into<String>, on: Matcher, n: i64) -> Self {
        Self { text: text.into(), on, need: Need::Latest(n) }
    }

    /// The number the progress is measured against.
    pub fn required(&self) -> i64 {
        match self.need {
            Need::Total(n) | Need::Latest(n) => n,
        }
    }
}

/// A quest, as defined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestDef {
    /// The name content refers to it by.
    pub name: String,
    /// What the player sees.
    pub title: String,
    /// The pitch.
    pub text: String,
    /// Quests that must be done before this one opens.
    pub after: Vec<QuestId>,
    /// The steps, all of which must be done.
    pub objectives: Vec<Objective>,
    /// Whether finishing this ends the run in victory.
    pub victory: bool,
}

impl Named for QuestDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered quest id.
pub type QuestId = Id<QuestDef>;

/// Where a quest stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestState {
    /// Waiting on the quests it comes after.
    Locked,
    /// In progress.
    Open,
    /// Finished.
    Done,
}

/// Something a fact changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// An objective's progress moved without finishing it.
    Progress {
        /// Which quest.
        quest: QuestId,
        /// Which objective, by index.
        objective: usize,
    },
    /// An objective finished.
    ObjectiveDone {
        /// Which quest.
        quest: QuestId,
        /// Which objective, by index.
        objective: usize,
    },
    /// A quest finished. `victory` echoes the definition.
    QuestDone {
        /// Which.
        quest: QuestId,
        /// Whether the run is won.
        victory: bool,
    },
    /// A quest opened because the ones before it are done.
    QuestOpened {
        /// Which.
        quest: QuestId,
    },
}

/// The state of every quest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tracker {
    states: Vec<QuestState>,
    progress: Vec<Vec<i64>>,
}

impl Tracker {
    /// Fresh state: quests with nothing before them are open, the rest
    /// locked.
    pub fn new(quests: &Registry<QuestDef>) -> Self {
        let states = quests.iter().map(|(_, q)| if q.after.is_empty() { QuestState::Open } else { QuestState::Locked }).collect();
        let progress = quests.iter().map(|(_, q)| vec![0; q.objectives.len()]).collect();
        Self { states, progress }
    }

    /// Where `quest` stands.
    pub fn state(&self, quest: QuestId) -> QuestState {
        self.states[quest.index()]
    }

    /// Progress on an objective, against what it requires.
    pub fn progress(&self, quest: QuestId, objective: usize, quests: &Registry<QuestDef>) -> (i64, i64) {
        (self.progress[quest.index()][objective], quests.get(quest).objectives[objective].required())
    }

    /// Whether an objective is finished.
    pub fn objective_done(&self, quest: QuestId, objective: usize, quests: &Registry<QuestDef>) -> bool {
        let (have, need) = self.progress(quest, objective, quests);
        have >= need
    }

    /// Every quest in `state`, in definition order.
    pub fn in_state(&self, state: QuestState) -> impl Iterator<Item = QuestId> + '_ {
        self.states.iter().enumerate().filter(move |(_, s)| **s == state).map(|(i, _)| QuestId::from_raw(i as u32))
    }

    /// Feeds one fact to every open quest and reports what changed, in
    /// the order it happened: progress, objectives done, quests done,
    /// quests opened.
    pub fn feed(&mut self, fact: &Fact, quests: &Registry<QuestDef>) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut finished = Vec::new();
        for (quest, def) in quests.iter() {
            if self.states[quest.index()] != QuestState::Open {
                continue;
            }
            for (i, obj) in def.objectives.iter().enumerate() {
                if !obj.on.matches(fact) || self.objective_done(quest, i, quests) {
                    continue;
                }
                let slot = &mut self.progress[quest.index()][i];
                match obj.need {
                    Need::Total(_) => *slot += fact.amount,
                    Need::Latest(_) => *slot = fact.amount,
                }
                if *slot >= obj.required() {
                    changes.push(Change::ObjectiveDone { quest, objective: i });
                } else {
                    changes.push(Change::Progress { quest, objective: i });
                }
            }
            if (0..def.objectives.len()).all(|i| self.objective_done(quest, i, quests)) {
                self.states[quest.index()] = QuestState::Done;
                changes.push(Change::QuestDone { quest, victory: def.victory });
                finished.push(quest);
            }
        }
        if !finished.is_empty() {
            for (quest, def) in quests.iter() {
                if self.states[quest.index()] == QuestState::Locked && def.after.iter().all(|a| self.states[a.index()] == QuestState::Done) {
                    self.states[quest.index()] = QuestState::Open;
                    changes.push(Change::QuestOpened { quest });
                }
            }
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact::FactDef;

    fn quests() -> (Registry<FactDef>, Registry<QuestDef>) {
        let facts = Registry::from_defs(vec![FactDef::new("killed"), FactDef::new("carrying"), FactDef::new("entered")]).unwrap();
        let (killed, carrying, entered) = (facts.expect("killed"), facts.expect("carrying"), facts.expect("entered"));
        let quests = Registry::from_defs(vec![
            QuestDef {
                name: "legs".into(),
                title: "Sea legs".into(),
                text: String::new(),
                after: vec![],
                objectives: vec![Objective::count("Kill three crabs", Matcher::any(killed).about(1), 3)],
                victory: false,
            },
            QuestDef {
                name: "hoard".into(),
                title: "The hoard".into(),
                text: String::new(),
                after: vec![QuestId::from_raw(0)],
                objectives: vec![
                    Objective::count("Find the vault", Matcher::any(entered).about(9), 1),
                    Objective::reach("Carry 100 doubloons", Matcher::any(carrying).about(7), 100),
                ],
                victory: true,
            },
        ])
        .unwrap();
        (facts, quests)
    }

    #[test]
    fn quests_progress_finish_and_unlock_in_order() {
        let (facts, quests) = quests();
        let (killed, carrying, entered) = (facts.expect("killed"), facts.expect("carrying"), facts.expect("entered"));
        let (legs, hoard) = (quests.expect("legs"), quests.expect("hoard"));
        let mut t = Tracker::new(&quests);
        assert_eq!(t.state(legs), QuestState::Open);
        assert_eq!(t.state(hoard), QuestState::Locked);

        assert!(t.feed(&Fact::new(entered).about(9), &quests).is_empty(), "a locked quest hears nothing");
        assert!(t.feed(&Fact::new(killed).about(2), &quests).is_empty(), "the wrong crab");
        assert_eq!(t.feed(&Fact::new(killed).about(1), &quests), vec![Change::Progress { quest: legs, objective: 0 }]);
        t.feed(&Fact::new(killed).about(1), &quests);
        assert_eq!(t.progress(legs, 0, &quests), (2, 3));
        assert_eq!(
            t.feed(&Fact::new(killed).about(1), &quests),
            vec![Change::ObjectiveDone { quest: legs, objective: 0 }, Change::QuestDone { quest: legs, victory: false }, Change::QuestOpened { quest: hoard }]
        );
        assert_eq!(t.state(hoard), QuestState::Open);
        assert!(t.feed(&Fact::new(killed).about(1), &quests).is_empty(), "a done quest hears nothing");

        // Latest: carrying 40 then 100 is done; dropping to 20 after would not undo it.
        assert_eq!(t.feed(&Fact::new(carrying).about(7).amount(40), &quests), vec![Change::Progress { quest: hoard, objective: 1 }]);
        assert_eq!(t.progress(hoard, 1, &quests), (40, 100));
        assert_eq!(t.feed(&Fact::new(carrying).about(7).amount(100), &quests), vec![Change::ObjectiveDone { quest: hoard, objective: 1 }]);
        assert!(t.feed(&Fact::new(carrying).about(7).amount(20), &quests).is_empty());
        assert_eq!(
            t.feed(&Fact::new(entered).about(9), &quests),
            vec![Change::ObjectiveDone { quest: hoard, objective: 0 }, Change::QuestDone { quest: hoard, victory: true }]
        );
        assert_eq!(t.in_state(QuestState::Done).collect::<Vec<_>>(), vec![legs, hoard]);
    }
}
