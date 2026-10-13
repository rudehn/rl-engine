//! Quests: objectives over facts, unlocked by other quests.
//!
//! A quest definition is a list of objectives, each a [`Matcher`] and what
//! it needs: a total reached by adding matching amounts, or a latest
//! amount at least so large. A quest opens when every quest it comes
//! after is done, and is done when every objective is. The [`Tracker`]
//! holds the state and reports every change as it feeds facts, so a game
//! narrates, rewards or ends the run from the changes and nothing else.
//!
//! A game's quests are a file, read by [`load`] or [`load_winnable`]: the
//! schema is the engine's, and what an objective counts is the game's own
//! closed list, so an objective naming a fact nothing writes is refused
//! when the file is read rather than left open for ever.

use crate::content::{ContentError, Named, Registry};
use crate::names::Names;
use rl_core::Id;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::events::fact::{Fact, Matcher};

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
    /// What the run's ending says when finishing this wins it; empty for
    /// nothing, and never said of a quest that is not `victory`.
    #[serde(default)]
    pub ending: String,
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

/// A quest as a quest file writes it, over the game's own list of what
/// an objective may count.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuestRon<On> {
    name: String,
    title: String,
    text: String,
    #[serde(default)]
    after: Vec<String>,
    objectives: Vec<ObjectiveRon<On>>,
    #[serde(default)]
    victory: bool,
    #[serde(default)]
    ending: String,
}

impl<On> Named for QuestRon<On> {
    fn name(&self) -> &str {
        &self.name
    }
}

/// One objective as a quest file writes it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjectiveRon<On> {
    text: String,
    on: On,
    need: Need,
}

/// The quests in a quest file, or every problem with them at once.
///
/// `On` is the game's own list of what an objective counts, an enum in
/// the game's crate, and `matcher` turns one into the [`Matcher`] the
/// tracker counts facts with; a variant may carry what it is about, a
/// deck or a [`NameRef`](crate::NameRef) resolved through `names`. A
/// closed list rather than a fact named by a string, so a file counting
/// something the game never writes is refused here instead of read as a
/// quest that can never finish.
///
/// The file is a RON list of quests, `implicit_some` on:
///
/// ```text
/// name:       unique; another quest's `after` names it by this
/// title:      what the quest is called
/// text:       what it asks, in a line or two
/// after:      optional; names of quests that must be done before this one
///             opens; none when left out, and it is open from the start
/// objectives: a list of (text, on, need)
///   text:     what the objective asks
///   on:       what it counts: a variant of the game's `On`
///   need:     Total(n), matching amounts adding up to n | Latest(n), the
///             latest matching amount at least n
/// victory:    optional; true for a quest whose finishing wins the run
/// ending:     optional; what the run's ending says when this wins it
/// ```
///
/// Refused, every one reported together, each named by its quest: a quest
/// `after` one the file does not have, a quest with no objectives, and an
/// `ending` on a quest that wins nothing, which would never be said; and
/// whatever [`Names::load`] refuses, a duplicate name, an unknown field or
/// an `On` the game does not have. A file in which nothing wins is read:
/// a game with no way to win is a game, and [`load_winnable`] is the call
/// for one that has.
///
/// ```
/// use rl_rules::events::quest::load;
/// use rl_rules::{FactDef, Matcher, Names, Registry};
///
/// #[derive(serde::Deserialize)]
/// enum On {
///     RatSlain,
/// }
///
/// let facts = Registry::from_defs(vec![FactDef::new("rat_slain")]).unwrap();
/// let slain = facts.expect("rat_slain");
/// let text = r#"[(name: "rats", title: "Rats", text: "", objectives: [(text: "Kill three", on: RatSlain, need: Total(3))])]"#;
/// let quests = load(text, &Names::new(), |on: &On| match on {
///     On::RatSlain => Matcher::any(slain),
/// })
/// .unwrap();
/// assert_eq!(quests.get(quests.expect("rats")).objectives[0].required(), 3);
/// ```
pub fn load<On: DeserializeOwned>(text: &str, names: &Names<'_>, matcher: impl Fn(&On) -> Matcher) -> Result<Registry<QuestDef>, ContentError> {
    read(text, names, matcher, false)
}

/// [`load`], and a file in which no quest is marked `victory` refused
/// too, beside every other problem, since a run under it could only end
/// in death.
pub fn load_winnable<On: DeserializeOwned>(text: &str, names: &Names<'_>, matcher: impl Fn(&On) -> Matcher) -> Result<Registry<QuestDef>, ContentError> {
    read(text, names, matcher, true)
}

/// What [`load`] and [`load_winnable`] share.
fn read<On: DeserializeOwned>(text: &str, names: &Names<'_>, matcher: impl Fn(&On) -> Matcher, winnable: bool) -> Result<Registry<QuestDef>, ContentError> {
    let authored: Registry<QuestRon<On>> = names.load(text)?;
    let mut errors = Vec::new();
    for (_, q) in authored.iter() {
        errors.extend(q.after.iter().filter(|a| authored.id(a).is_none()).map(|a| format!("{}: after unknown quest {a:?}", q.name)));
        if q.objectives.is_empty() {
            errors.push(format!("{}: no objectives", q.name));
        }
        if !q.ending.is_empty() && !q.victory {
            errors.push(format!("{}: an ending, on a quest that wins nothing, would never be said", q.name));
        }
    }
    if winnable && !authored.iter().any(|(_, q)| q.victory) {
        errors.push("no quest is marked victory, so no run could be won".into());
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    let defs = authored
        .iter()
        .map(|(_, q)| QuestDef {
            name: q.name.clone(),
            title: q.title.clone(),
            text: q.text.clone(),
            after: q.after.iter().map(|a| QuestId::from_raw(authored.expect(a).raw())).collect(),
            objectives: q.objectives.iter().map(|o| Objective { text: o.text.clone(), on: matcher(&o.on), need: o.need }).collect(),
            victory: q.victory,
            ending: q.ending.clone(),
        })
        .collect();
    Registry::from_defs(defs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::fact::FactDef;

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
                ending: String::new(),
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
                ending: String::new(),
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

    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    enum On {
        Killed(u64),
        Entered,
    }

    fn matcher(facts: &Registry<FactDef>) -> impl Fn(&On) -> Matcher + '_ {
        |on| match on {
            On::Killed(what) => Matcher::any(facts.expect("killed")).about(*what),
            On::Entered => Matcher::any(facts.expect("entered")),
        }
    }

    #[test]
    fn a_quest_file_reads_into_definitions_with_its_order_its_matchers_and_its_ending() {
        let (facts, _) = quests();
        let text = r#"[
            (name: "legs", title: "Sea legs", text: "", objectives: [(text: "Kill three crabs", on: Killed(1), need: Total(3))]),
            (name: "hoard", title: "The hoard", text: "Find it.", after: ["legs"], objectives: [(text: "Find the vault", on: Entered, need: Latest(1))], victory: true, ending: "Rich at last."),
        ]"#;
        let quests = load_winnable(text, &Names::new(), matcher(&facts)).unwrap();
        let (legs, hoard) = (quests.expect("legs"), quests.expect("hoard"));
        assert_eq!(quests.get(legs).objectives[0].on, Matcher::any(facts.expect("killed")).about(1));
        assert_eq!(quests.get(hoard).after, vec![legs]);
        assert_eq!((quests.get(hoard).victory, quests.get(hoard).ending.as_str()), (true, "Rich at last."));
        assert_eq!((quests.get(legs).victory, quests.get(legs).ending.as_str()), (false, ""));
    }

    /// Every problem in the file comes back in one refusal, each named by
    /// its quest, so an author fixes the file once.
    #[test]
    fn every_problem_in_a_quest_file_is_reported_at_once() {
        let (facts, _) = quests();
        let text = r#"[
            (name: "lost", title: "Lost", text: "", after: ["nowhere", "legs"], objectives: []),
            (name: "legs", title: "Legs", text: "", objectives: [(text: "Kill", on: Killed(1), need: Total(1))], ending: "Said by nobody."),
        ]"#;
        let expected = [
            r#"lost: after unknown quest "nowhere""#,
            "lost: no objectives",
            "legs: an ending, on a quest that wins nothing, would never be said",
            "no quest is marked victory, so no run could be won",
        ];
        match load_winnable(text, &Names::new(), matcher(&facts)) {
            Err(ContentError::Invalid(errors)) => assert_eq!(errors, expected),
            other => panic!("refused for the wrong reason: {other:?}"),
        }
        match load(text, &Names::new(), matcher(&facts)) {
            Err(ContentError::Invalid(errors)) => assert_eq!(errors, expected[..3], "and a file that need not win is not refused for winning nothing"),
            other => panic!("refused for the wrong reason: {other:?}"),
        }
    }

    #[test]
    fn a_quest_file_that_wins_nothing_reads_unless_it_must_be_winnable() {
        let (facts, _) = quests();
        let text = r#"[(name: "legs", title: "Legs", text: "", objectives: [(text: "Kill", on: Killed(1), need: Total(1))])]"#;
        assert_eq!(load(text, &Names::new(), matcher(&facts)).map(|q| q.len()), Ok(1));
        assert!(load_winnable(text, &Names::new(), matcher(&facts)).is_err());
    }

    #[test]
    fn an_objective_counting_what_the_game_never_writes_is_refused_by_name() {
        let (facts, _) = quests();
        let text = r#"[(name: "rats", title: "Rats", text: "", objectives: [(text: "Kill rats", on: RatSlain, need: Total(1))], victory: true)]"#;
        let refused = load(text, &Names::new(), matcher(&facts)).expect_err("it is refused");
        assert!(refused.to_string().contains("RatSlain"), "naming what it counts: {refused}");
    }
}
