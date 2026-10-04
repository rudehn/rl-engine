//! What a run is for: the quests in `assets/quests.ron`, the one fact they
//! count, and the run won when the quest marked `victory` is done.
//!
//! [`load`] reads the file into the engine's [`Quests`] and the [`Facts`]
//! its objectives count, as Foundry's `mission.rs` reads its charges;
//! [`report_bosses`] writes that fact whenever a monster marked `boss`
//! dies, and ends the run won in the same pass when the fact finishes the
//! quest marked `victory`. The win is the file's: a later part that
//! changes what wins a run, a boss drawn from a pool or a crown to carry
//! home, changes the file rather than a system that knows which death is
//! the last.
//!
//! Loaded once, before any run, as the content is: the engine forgets a
//! run's progress when the run ends, so the next run's quest is open again
//! without being read again.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::bestiary::{Bestiary, Kind};

/// The quests file, compiled in so the binary runs from anywhere.
const QUESTS_RON: &str = include_str!("../assets/quests.ron");

/// The fact kinds Undermere's quests count.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Facts {
    /// A monster marked `boss` died, about its definition's id.
    pub boss_slain: FactKind,
}

impl Facts {
    fn new() -> Self {
        let defs = Registry::from_defs(vec![FactDef::new("boss_slain")]).expect("one fact kind, named once");
        Self { boss_slain: defs.expect("boss_slain") }
    }
}

/// A quest as `quests.ron` writes it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct QuestRon {
    name: String,
    title: String,
    text: String,
    #[serde(default)]
    after: Vec<String>,
    objectives: Vec<ObjectiveRon>,
    #[serde(default)]
    victory: bool,
}

impl Named for QuestRon {
    fn name(&self) -> &str {
        &self.name
    }
}

/// One objective as `quests.ron` writes it.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ObjectiveRon {
    text: String,
    on: On,
    need: Need,
}

/// What an objective counts: a closed list of the facts Undermere writes,
/// so an objective naming one nothing writes is refused when the file is
/// read rather than left open for ever.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
enum On {
    /// A monster marked `boss` died, by whatever hand.
    BossSlain,
}

impl On {
    /// The matcher the tracker counts facts with.
    fn matcher(self, facts: &Facts) -> Matcher {
        match self {
            On::BossSlain => Matcher::any(facts.boss_slain),
        }
    }
}

/// Reads `quests.ron` into the engine's [`Quests`] and the facts it
/// counts. Panics naming the file and every problem in it, since a quest
/// file that does not read is a run that cannot be won.
pub fn load(registries: &Registries) -> (Quests, Facts) {
    let facts = Facts::new();
    let defs = read(QUESTS_RON, &registries.names(), &facts).unwrap_or_else(|e| panic!("assets/quests.ron: {e}"));
    (Quests::new(defs), facts)
}

/// The quests in `text`, every problem with them at once: a quest after
/// one the file does not have, a quest with nothing to do, and a file in
/// which no quest wins the run.
fn read(text: &str, names: &Names, facts: &Facts) -> Result<Registry<QuestDef>, ContentError> {
    let authored: Registry<QuestRon> = names.load(text)?;
    authored.validate(|q, all| {
        if let Some(a) = q.after.iter().find(|a| all.id(a).is_none()) {
            return Err(format!("after unknown quest {a:?}"));
        }
        if q.objectives.is_empty() {
            return Err("no objectives".into());
        }
        Ok(())
    })?;
    if !authored.iter().any(|(_, q)| q.victory) {
        return Err(ContentError::Invalid(vec!["no quest is marked victory, so no run could be won".into()]));
    }
    let defs = authored
        .iter()
        .map(|(_, q)| QuestDef {
            name: q.name.clone(),
            title: q.title.clone(),
            text: q.text.clone(),
            after: q.after.iter().map(|a| Id::from_raw(authored.expect(a).raw())).collect(),
            objectives: q.objectives.iter().map(|o| Objective { text: o.text.clone(), on: o.on.matcher(facts), need: o.need }).collect(),
            victory: q.victory,
        })
        .collect();
    Registry::from_defs(defs)
}

/// What reporting a boss's death reads and writes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Report<'w> {
    bestiary: Res<'w, Bestiary>,
    facts: Res<'w, Facts>,
    quests: Res<'w, Quests>,
    happened: MessageWriter<'w, Happened>,
    over: MessageWriter<'w, RunOver>,
}

/// Writes [`Facts::boss_slain`] for every monster marked `boss` that dies,
/// about its definition, whoever's hand it fell to, since a boss burned to
/// death is as dead as one cut down; and ends the run won, in the same
/// pass, when that fact finishes the quest marked `victory`.
///
/// The run is won the moment the boss dies. The engine feeds the tracker
/// only after the frame's turns, and the rest of this frame's turns are
/// the court's, so a run left to end on the tracker's report could be lost
/// to a knight's blow after the king had fallen. The fact is put to a copy
/// of the tracker here instead, so it is still the quest file that says
/// what wins, and the engine's own feeding of it later changes nothing it
/// has not already been told. With no words of its own: the menu's title
/// over the ending says the king is dead, and the run's record who slew
/// him.
///
/// In `TurnSet::React`, while the dead still stand where they fell and
/// carry their [`Kind`].
pub fn report_bosses(mut deaths: MessageReader<DeathEvent>, kinds: Query<&Kind>, report: Report) {
    let Report { bestiary, facts, quests, mut happened, mut over } = report;
    for death in deaths.read() {
        let Ok(kind) = kinds.get(death.entity) else { continue };
        if !bestiary.defs.get(kind.0).boss {
            continue;
        }
        let fact = Fact::new(facts.boss_slain).about(u64::from(kind.0.raw()));
        happened.write(Happened(fact));
        let mut tracker = quests.tracker.clone();
        if tracker.feed(&fact, &quests.defs).iter().any(|change| matches!(change, Change::QuestDone { victory: true, .. })) {
            over.write(RunOver::won());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped file reads, and its one quest is open from the start,
    /// wins the run, and asks for one boss.
    #[test]
    fn the_quest_file_reads_and_its_one_quest_wins_the_run() {
        let (quests, facts) = load(&crate::content::registries());
        assert_eq!(quests.defs.len(), 1);
        let (id, quest) = quests.defs.iter().next().expect("a quest");
        assert!(quest.victory, "slaying the king wins the run");
        assert_eq!(quests.tracker.state(id), QuestState::Open, "open from the start");
        assert_eq!(quest.objectives[0].on, Matcher::any(facts.boss_slain));
        assert_eq!(quests.tracker.progress(id, 0, &quests.defs), (0, 1), "one boss");
    }

    /// The run is won the moment the king dies: over in the very frame of
    /// the blow, before any later turn of that frame, a knight's among
    /// them, could take the win away, and not a frame later when the
    /// engine's tracker reports the quest done.
    #[test]
    fn the_run_is_won_in_the_frame_the_king_dies() {
        use rl_engine::rl_core::RunSeed;
        let mut app = crate::testing::headless(RunSeed(9));
        let me = crate::testing::arrive(&mut app, crate::dungeon::map_of(10));
        crate::testing::sturdy(&mut app, me);
        let king_id = app.world().resource::<Bestiary>().defs.expect("the Drowned King");
        let king = {
            let world = app.world_mut();
            world.query::<(Entity, &Kind)>().iter(world).find(|(_, k)| k.0 == king_id).map(|(e, _)| e).expect("the king on his throne")
        };
        let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
        app.world_mut().write_message(DamageEvent::new(king, rl_engine::rl_rules::Hit::by(me, crush, 100_000)));
        app.update();
        assert_eq!(app.world().get_resource::<Ending>().map(|e| e.outcome), Some(Outcome::Won), "won in the frame of the blow");
    }

    /// A file in which nothing wins the run is refused, since a run under
    /// it could only end in death; so is an objective counting something
    /// Undermere never writes.
    #[test]
    fn a_quest_file_that_wins_nothing_or_counts_what_nobody_writes_is_refused() {
        let registries = crate::content::registries();
        let facts = Facts::new();
        let nothing_wins = r#"[(name: "rats", title: "Rats", text: "", objectives: [(text: "Kill the rat king", on: BossSlain, need: Total(1))])]"#;
        match read(nothing_wins, &registries.names(), &facts) {
            Err(ContentError::Invalid(errors)) => assert_eq!(errors, ["no quest is marked victory, so no run could be won"]),
            Err(other) => panic!("refused for the wrong reason: {other}"),
            Ok(_) => panic!("it read"),
        }
        let unknown = r#"[(name: "rats", title: "Rats", text: "", objectives: [(text: "Kill rats", on: RatSlain, need: Total(1))], victory: true)]"#;
        match read(unknown, &registries.names(), &facts) {
            Err(e) => assert!(e.to_string().contains("RatSlain"), "refused naming what it counts: {e}"),
            Ok(_) => panic!("it read"),
        }
    }
}
