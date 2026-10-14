//! What a run comes to, once it is over, and the list of every run that
//! has ended one.
//!
//! A game writes [`RecordRun`] once, when its own victory or ending
//! condition is met, carrying its score, its one-line summary and the
//! sections it wants shown. [`RecordsPlugin`] completes it into a
//! [`RunRecord`] from what only the engine knows, the outcome, the seed
//! and the turn, so the game never repeats numbers the engine already has
//! and the end screen and a records screen read the one record rather
//! than two accounts of the same run. Filed into [`RunRecords`], best
//! score first, and kept apart as `this_run` until the run it belongs to
//! is torn down.
//!
//! `rl-save`'s `SavePlugin` keeps the list between launches, in a
//! slot of its own.

use bevy::ecs::message::MessageCursor;
use bevy::prelude::*;
use rl_core::RunSeed;
use serde::{Deserialize, Serialize};

use crate::plugin::{EndRun, EngineSet};
use crate::seed::Seed;
use crate::state::{Ending, Outcome};

/// How a recorded run ended.
///
/// The engine's own three; a game's finer reasons are its summary's words,
/// not another variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RecordOutcome {
    /// The game's victory condition was met.
    Won,
    /// The player died. The default, for a record whose bytes do not say.
    #[default]
    Died,
    /// The player gave the run up.
    Abandoned,
}

/// One part of a run's own account of itself, shown in full by an end
/// screen and a records screen both, so neither disagrees with the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordSection {
    /// What the section is, such as "Score" or "The run".
    pub heading: String,
    /// The section's words, the game's own.
    pub body: String,
}

/// What a run came to, once it was over.
///
/// Every field defaulted, so a record a later build adds a field to, and
/// one an earlier build never wrote a field for, both still read: the list
/// is kept a hundred runs deep, long past a single build's lifetime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct RunRecord {
    /// How the run ended.
    pub outcome: RecordOutcome,
    /// The game's one line about it: "Won on the tenth floor".
    pub summary: String,
    /// The game's score. The list is filed by it, highest first.
    pub score: i64,
    /// The run's seed.
    pub seed: u64,
    /// The whole turn it ended on.
    pub turn: u64,
    /// Seconds since the Unix epoch when it ended, or `None` on wasm,
    /// where the engine has no wall clock to read, and for a record
    /// written before this field existed.
    pub when: Option<u64>,
    /// The game's own account of the run.
    pub sections: Vec<RecordSection>,
}

/// The most records [`RunRecords::file`] keeps. Long enough that filling
/// it is something to be proud of, short enough that the list stays a
/// screen or two.
pub const MAX_RECORDS: usize = 100;

/// Every run that has ended, best score first, and the one just filed for
/// the run now ending.
///
/// `this_run` is read by an end screen, and by a records screen opened
/// from it, so neither draws its own copy of what the run came to; it is
/// forgotten when the run is torn down, in [`EndRun`], while `records`
/// stands until `rl-save`'s `SavePlugin` writes it, which never
/// deletes it.
#[derive(Resource, Default)]
pub struct RunRecords {
    /// Every run that has ended, best score first.
    pub records: Vec<RunRecord>,
    /// The record filed for the run that just ended, if one has, until the
    /// next run begins.
    pub this_run: Option<RunRecord>,
}

impl RunRecords {
    /// Files `record`, best score first; a tie keeps the newer record,
    /// this one, ahead of whatever already matched it. Drops whatever
    /// falls past [`MAX_RECORDS`].
    pub fn file(&mut self, record: RunRecord) {
        let position = self.records.iter().position(|kept| record.score >= kept.score).unwrap_or(self.records.len());
        self.records.insert(position, record);
        self.records.truncate(MAX_RECORDS);
    }
}

/// What a game writes once, when its run ends, for [`RecordsPlugin`] to
/// complete into a [`RunRecord`].
///
/// Score, summary and sections are the game's own words; the outcome, the
/// seed and the turn are the engine's, read off [`Ending`] and [`Seed`]
/// rather than asked of the game a second time.
#[derive(Message, Debug, Clone)]
pub struct RecordRun {
    /// The game's score for the run.
    pub score: i64,
    /// The game's one line about how it ended.
    pub summary: String,
    /// The game's own account of the run.
    pub sections: Vec<RecordSection>,
}

/// Completes every [`RecordRun`] a game writes into a [`RunRecord`] and
/// keeps the list.
///
/// Opt-in: a game that never writes `RecordRun` keeps no records.
/// `rl-save`'s `SavePlugin` is what keeps the list between
/// launches; without it the list lasts only the process.
pub struct RecordsPlugin;

impl Plugin for RecordsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RecordRun>()
            .init_resource::<RunRecords>()
            .init_resource::<Waiting>()
            // Before anything draws, so the screen a run ends on shows its
            // record from its first frame rather than from whichever frame
            // the executor first ran the filing ahead of the drawing in;
            // and after the frame's play, sight and all, so a game's own
            // system between the turns and the drawing is ordered against
            // it by the engine's phases rather than left to the executor.
            .add_systems(Update, record_runs.after(EngineSet::Fov).before(EngineSet::Present))
            // Not `reset_on_new_run::<RunRecords>()`: that puts the whole
            // resource back to its default, forgetting every filed record
            // along with `this_run`. Only the field is cleared here, and
            // in `EndRun` rather than `NewRun`, so the record stands
            // through every frame the ended run's own screen reads it in
            // and is gone before the next run begins.
            .add_systems(EndRun, forget_the_run);
    }
}

/// The [`RecordRun`]s a game wrote that wait on their run's [`Ending`],
/// and how far the messages have been read.
///
/// A record is completed from the ending of the run it was written in, so
/// one whose run is torn down before it ends, by a restart or a leaving,
/// has no ending coming. A resource rather than the filing system's own
/// state, so the teardown can read what is left and forget it; kept by the
/// filing system alone, it waited on and was filed against the next run's
/// ending, under that run's outcome, seed and turn.
#[derive(Resource, Default)]
struct Waiting {
    /// Where the filing and the teardown have read the messages up to, one
    /// cursor for the two, so neither reads a record the other has.
    read: MessageCursor<RecordRun>,
    /// Read and not yet filed.
    records: Vec<RecordRun>,
}

impl Waiting {
    /// Takes every record written since the last look.
    fn take(&mut self, written: &Messages<RecordRun>) {
        let Self { read, records } = self;
        records.extend(read.read(written).cloned());
    }

    /// Completes every waiting record from `ending` and `seed` and files it.
    fn file(&mut self, ending: &Ending, seed: Option<&Seed>, kept: &mut RunRecords) {
        let seed = seed.map_or(RunSeed(0), |s| s.0);
        for run in self.records.drain(..) {
            let record = RunRecord {
                outcome: match ending.outcome {
                    Outcome::Won => RecordOutcome::Won,
                    Outcome::Died { .. } => RecordOutcome::Died,
                    Outcome::Abandoned => RecordOutcome::Abandoned,
                },
                summary: run.summary,
                score: run.score,
                seed: seed.0,
                turn: u64::from(ending.turn),
                when: seconds_since_epoch(),
                sections: run.sections,
            };
            kept.this_run = Some(record.clone());
            kept.file(record);
        }
    }
}

/// Completes each waiting [`RecordRun`] once the [`Ending`] it waits for
/// exists, and files the result.
///
/// A game may write `RecordRun` from the same pass that writes the
/// [`RunOver`](crate::state::RunOver) its ending answers to, which
/// `end_runs` (`crate::state::end_runs`, in `PostUpdate`) has not yet
/// turned into an `Ending` that same frame. Rather than drop what arrived
/// too early, it waits and is completed the first frame `Ending` exists,
/// however many frames that takes, as long as its run stands.
fn record_runs(
    written: Res<Messages<RecordRun>>,
    mut waiting: ResMut<Waiting>,
    ending: Option<Res<Ending>>,
    seed: Option<Res<Seed>>,
    mut kept: ResMut<RunRecords>,
) {
    waiting.take(&written);
    if let Some(ending) = ending {
        waiting.file(&ending, seed.as_deref(), &mut kept);
    }
}

/// Now, as seconds since the Unix epoch.
///
/// `SystemTime::now` compiles on wasm but panics there, where there is no
/// wall clock to read, so a record made there carries `None` instead.
fn seconds_since_epoch() -> Option<u64> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs())
    }
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
}

/// Forgets the run's records as it is torn down: which record was this
/// run's, so the next run does not show an old ending before one of its
/// own exists, and every record still waiting, so none is filed against
/// the next run's ending.
///
/// A run that ended still has its `Ending` here, before the teardown takes
/// it, so a record written late in the frame its run is left is filed
/// rather than lost. One whose run never ended cannot be completed, and
/// is dropped with a warning: a game that records a run given up ends it
/// with [`RunOver`](crate::state::RunOver) first.
fn forget_the_run(
    written: Res<Messages<RecordRun>>,
    mut waiting: ResMut<Waiting>,
    ending: Option<Res<Ending>>,
    seed: Option<Res<Seed>>,
    mut kept: ResMut<RunRecords>,
) {
    waiting.take(&written);
    match ending {
        Some(ending) => waiting.file(&ending, seed.as_deref(), &mut kept),
        None if !waiting.records.is_empty() => {
            warn!("{} run record(s) dropped: written for a run torn down before it ended, with no Ending to complete them from", waiting.records.len());
            waiting.records.clear();
        }
        None => {}
    }
    kept.this_run = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Restart;

    /// Whatever reads the records while the frame is drawn is ordered after
    /// the filing, so an end screen shows its run's record on its first
    /// frame. A pair left unordered still passes a test that watches the
    /// screen whenever the executor happens to run them in the right order,
    /// so this asks the schedule itself.
    #[test]
    fn a_record_is_filed_before_anything_draws() {
        fn draws(_: Res<RunRecords>) {}
        let mut app = crate::plugin::headless_app();
        app.add_plugins(RecordsPlugin).add_systems(Update, draws.in_set(EngineSet::Present));
        let (files, reads) = (IntoSystem::into_system(record_runs).system_type(), IntoSystem::into_system(draws).system_type());
        app.world_mut().schedule_scope(Update, |world, schedule| {
            schedule.initialize(world).expect("Update builds");
            let types: Vec<_> = schedule.systems().expect("just built").map(|(key, system)| (key, system.system_type())).collect();
            let type_of = |key| types.iter().find(|(k, _)| *k == key).map(|(_, t)| *t);
            let left_to_chance = schedule.graph().conflicting_systems().iter().any(|(a, b, _)| {
                let pair = (type_of(*a), type_of(*b));
                pair == (Some(files), Some(reads)) || pair == (Some(reads), Some(files))
            });
            assert!(!left_to_chance, "filing a record and drawing it are left to the executor's order");
        });
    }

    fn record(score: i64) -> RunRecord {
        RunRecord { score, ..Default::default() }
    }

    #[test]
    fn a_record_is_filed_best_score_first() {
        let mut records = RunRecords::default();
        records.file(record(50));
        records.file(record(100));
        records.file(record(75));
        assert_eq!(records.records.iter().map(|r| r.score).collect::<Vec<_>>(), vec![100, 75, 50]);
    }

    #[test]
    fn a_tie_files_the_newer_record_first() {
        let mut records = RunRecords::default();
        records.file(RunRecord { score: 10, summary: "first".into(), ..Default::default() });
        records.file(RunRecord { score: 10, summary: "second".into(), ..Default::default() });
        assert_eq!(records.records.iter().map(|r| r.summary.as_str()).collect::<Vec<_>>(), vec!["second", "first"]);
    }

    #[test]
    fn the_list_keeps_the_best_hundred() {
        let mut records = RunRecords::default();
        for score in 1..=150 {
            records.file(record(score));
        }
        assert_eq!(records.records.len(), MAX_RECORDS);
        assert_eq!(records.records.first().unwrap().score, 150, "the best score stands first");
        assert_eq!(records.records.last().unwrap().score, 51, "the hundred best, not the first hundred filed");
    }

    #[test]
    fn a_record_run_written_before_the_ending_waits_for_it() {
        let mut app = crate::plugin::headless_app();
        app.add_plugins(RecordsPlugin);
        app.world_mut().write_message(RecordRun { score: 42, summary: "won".into(), sections: Vec::new() });
        app.update();
        {
            let records = app.world().resource::<RunRecords>();
            assert!(records.this_run.is_none(), "no Ending yet, so nothing is filed");
            assert!(records.records.is_empty());
        }
        app.world_mut().insert_resource(Ending { outcome: Outcome::Won, epitaph: String::new(), seed: RunSeed(1), turn: 10 });
        app.update();
        let records = app.world().resource::<RunRecords>();
        assert_eq!(records.records.len(), 1, "filed once the ending exists");
        assert_eq!(records.this_run.as_ref().map(|r| r.score), Some(42));
    }

    /// A record still waiting for its ending when its run is torn down, by
    /// a restart before the ending came, goes with the run: the next run's
    /// ending files its own record and not the last run's under its name.
    #[test]
    fn a_record_still_waiting_when_its_run_is_torn_down_is_not_filed_against_the_next() {
        let mut app = crate::plugin::headless_app();
        app.add_plugins(RecordsPlugin);
        app.insert_resource(Seed(RunSeed(1)));
        app.world_mut().write_message(RecordRun { score: 1, summary: "the run torn down".into(), sections: Vec::new() });
        app.update();
        app.world_mut().write_message(Restart::fresh());
        app.update();
        app.update();
        app.world_mut().insert_resource(Ending { outcome: Outcome::Won, epitaph: String::new(), seed: RunSeed(2), turn: 3 });
        app.world_mut().write_message(RecordRun { score: 2, summary: "the next run".into(), sections: Vec::new() });
        app.update();
        let records = app.world().resource::<RunRecords>();
        assert_eq!(records.records.iter().map(|r| r.summary.as_str()).collect::<Vec<_>>(), ["the next run"]);
        assert_eq!(records.this_run.as_ref().map(|r| r.summary.as_str()), Some("the next run"));
    }

    /// The same for a record written in the frame its run is torn down,
    /// after that frame's filing has run: a run that never ended drops it,
    /// and one that did files it under its own ending before the teardown
    /// takes the ending away.
    #[test]
    fn a_record_written_as_its_run_is_torn_down_is_filed_only_under_that_runs_ending() {
        for (ended, filed) in [(false, vec!["the next run"]), (true, vec!["the next run", "the run left"])] {
            let mut app = crate::plugin::headless_app();
            app.add_plugins(RecordsPlugin);
            app.insert_resource(Seed(RunSeed(1)));
            if ended {
                app.world_mut().insert_resource(Ending { outcome: Outcome::Died { by: None }, epitaph: String::new(), seed: RunSeed(1), turn: 9 });
            }
            let late = |mut records: MessageWriter<RecordRun>, mut once: Local<bool>| {
                if !std::mem::replace(&mut *once, true) {
                    records.write(RecordRun { score: 1, summary: "the run left".into(), sections: Vec::new() });
                }
            };
            app.add_systems(Last, late.before(crate::plugin::EndOfFrame::Restart));
            app.world_mut().write_message(Restart::fresh());
            app.update();
            app.update();
            app.world_mut().insert_resource(Ending { outcome: Outcome::Won, epitaph: String::new(), seed: RunSeed(2), turn: 3 });
            app.world_mut().write_message(RecordRun { score: 2, summary: "the next run".into(), sections: Vec::new() });
            app.update();
            let records = app.world().resource::<RunRecords>();
            let summaries: Vec<&str> = records.records.iter().map(|r| r.summary.as_str()).collect();
            assert_eq!(summaries, filed, "the run left had ended: {ended}");
            let left = records.records.iter().find(|r| r.summary == "the run left");
            assert!(left.is_none_or(|r| r.outcome == RecordOutcome::Died && r.turn == 9), "under its own ending: {left:?}");
        }
    }

    #[test]
    fn a_record_takes_its_outcome_seed_and_turn_from_the_ending() {
        let mut app = crate::plugin::headless_app();
        app.add_plugins(RecordsPlugin);
        app.insert_resource(Seed(RunSeed(77)));
        app.world_mut().insert_resource(Ending { outcome: Outcome::Died { by: None }, epitaph: String::new(), seed: RunSeed(999), turn: 123 });
        app.world_mut().write_message(RecordRun { score: 7, summary: "died".into(), sections: Vec::new() });
        app.update();
        let record = app.world().resource::<RunRecords>().this_run.clone().expect("filed");
        assert_eq!(record.outcome, RecordOutcome::Died);
        assert_eq!(record.seed, 77, "from the live Seed, not Ending's own copy");
        assert_eq!(record.turn, 123);
    }

    #[test]
    fn this_run_is_forgotten_when_a_new_run_begins() {
        let mut app = crate::plugin::headless_app();
        app.add_plugins(RecordsPlugin);
        app.insert_resource(Seed(RunSeed(1)));
        app.world_mut().insert_resource(Ending { outcome: Outcome::Won, epitaph: String::new(), seed: RunSeed(1), turn: 5 });
        app.world_mut().write_message(RecordRun { score: 1, summary: "won".into(), sections: Vec::new() });
        app.update();
        assert!(app.world().resource::<RunRecords>().this_run.is_some());

        app.world_mut().write_message(Restart::fresh());
        app.update();
        app.update();
        let records = app.world().resource::<RunRecords>();
        assert!(records.this_run.is_none(), "forgotten once the run is torn down");
        assert_eq!(records.records.len(), 1, "but not the record itself");
    }
}
