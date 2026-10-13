//! Keeping every run's record between launches.
//!
//! [`RunRecords`] is plain data, filled by `rl-bevy`'s `RecordsPlugin`
//! without knowing where, or whether, it is kept. This is a third slot,
//! apart from [`SETTINGS_SLOT`](crate::SETTINGS_SLOT) and a game's own
//! run: read once before the first frame, and written whenever a record
//! is filed.
//!
//! Not the versioned envelope `rl-save` otherwise writes a run through:
//! [`Versioned`](crate::Versioned) refuses a save whole when its version
//! differs, which suits a run's entities, saved by their own
//! game-specific shapes that cannot be read piecemeal. A list of records
//! is different: every [`RunRecord`] field is `#[serde(default)]`, so a
//! record a later build adds a field to, or an earlier one never wrote
//! one for, reads as far as it can. A slot that cannot be read at all is
//! warned about and left as an empty list, unlike a settings file that is
//! left untouched: a map of names can be recalled name by name regardless
//! of what else is wrong with it, but a list that will not parse at all
//! cannot be merged into entry by entry, so the next record filed simply
//! writes a fresh list over whatever was there.
//!
//! Apart from the run's own slot, and never forgotten with it: ending a
//! run, continuing one or giving one up leaves this slot alone.

use bevy::prelude::*;
use rl_bevy::plugin::Needs;
use rl_bevy::{RunRecord, RunRecords};

use crate::backend::{SaveBackend, Saves};

/// The slot the run records are remembered in.
pub const RECORDS_SLOT: &str = "records";

/// Recalls every run's record before the first frame, and writes the list
/// down again whenever it changes.
///
/// Opt-in: without it [`RunRecords`] holds only what has been filed since
/// the process started.
pub struct RecordsSavePlugin;

/// What the slot is known to hold, so a frame that filed nothing writes
/// nothing back.
#[derive(Resource, Debug, Default)]
struct Remembered(Vec<RunRecord>);

impl Plugin for RecordsSavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RunRecords>()
            .init_resource::<Remembered>()
            .needs::<Saves>("RecordsSavePlugin", "`Saves`, the backend records are remembered through, such as the one `RoguelikePlugins::saves(\"my-game\")` inserts, which keeps a scripted run in memory, or `Saves::platform_default(\"my-game\")`")
            .add_systems(Last, remember.run_if(resource_changed::<RunRecords>).run_if(resource_exists::<Saves>));
    }

    /// In `finish`, with `SettingsSavePlugin`'s own reason: every plugin
    /// has built by then, though nothing here depends on what any of them
    /// declared.
    fn finish(&self, app: &mut App) {
        let Some(saves) = app.world().get_resource::<Saves>() else { return };
        let records = match saves.load(RECORDS_SLOT) {
            Ok(Some(text)) => match ron::from_str::<Vec<RunRecord>>(&text) {
                Ok(records) => records,
                Err(e) => {
                    warn!("the run records could not be read, so the list starts empty and the next record filed replaces it: {e}");
                    Vec::new()
                }
            },
            Ok(None) => Vec::new(),
            Err(e) => {
                warn!("the run records could not be loaded, so the list starts empty and the next record filed replaces it: {e}");
                Vec::new()
            }
        };
        app.world_mut().resource_mut::<RunRecords>().records = records.clone();
        app.world_mut().resource_mut::<Remembered>().0 = records;
    }
}

/// Writes the list down when it is not what the slot holds.
fn remember(records: Res<RunRecords>, saves: Res<Saves>, mut remembered: ResMut<Remembered>) {
    if records.records == remembered.0 {
        return;
    }
    match ron::ser::to_string_pretty(&records.records, ron::ser::PrettyConfig::default()) {
        Ok(text) => match saves.persist(RECORDS_SLOT, &text) {
            Ok(()) => remembered.0 = records.records.clone(),
            Err(e) => warn!("the run records could not be remembered: {e}"),
        },
        Err(e) => warn!("the run records could not be written down: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MemoryBackend;
    use rl_bevy::{EngineState, RunOver, Seed};
    use rl_core::RunSeed;
    use std::sync::Arc;

    /// An app with `RecordsSavePlugin` and `backend` to remember records
    /// in, finished the way `App::run` finishes one.
    fn game(backend: &Arc<MemoryBackend>) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Saves(backend.clone()));
        app.add_plugins(RecordsSavePlugin);
        app.finish();
        app.cleanup();
        app
    }

    #[test]
    fn records_round_trip_through_their_slot() {
        let backend = Arc::new(MemoryBackend::default());
        let mut app = game(&backend);
        app.world_mut().resource_mut::<RunRecords>().file(RunRecord { score: 10, summary: "won".into(), ..Default::default() });
        app.update();

        let again = game(&backend);
        let records = &again.world().resource::<RunRecords>().records;
        assert_eq!(records.len(), 1, "recalled before any frame has run");
        assert_eq!(records[0].summary, "won");
        assert_eq!(records[0].score, 10);
    }

    #[test]
    fn a_record_from_a_build_with_more_fields_still_reads() {
        let backend = Arc::new(MemoryBackend::default());
        let text = r#"[(outcome: Won, summary: "Won on the tenth floor", score: 500, seed: 1, turn: 900, when: Some(1000), sections: [], from_a_later_build: "ignored")]"#;
        backend.persist(RECORDS_SLOT, text).unwrap();
        let app = game(&backend);
        let records = &app.world().resource::<RunRecords>().records;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].summary, "Won on the tenth floor");
        assert_eq!(records[0].score, 500);
    }

    /// A record an earlier build wrote, before the time it ended and the
    /// sections were kept, reads with no time and no sections, as the
    /// list's own docs promise, rather than failing the whole list.
    #[test]
    fn a_record_from_a_build_with_fewer_fields_still_reads() {
        let backend = Arc::new(MemoryBackend::default());
        let text = r#"[(outcome: Died, summary: "Slain on the third floor", score: 300, seed: 4, turn: 120)]"#;
        backend.persist(RECORDS_SLOT, text).unwrap();
        let app = game(&backend);
        let records = &app.world().resource::<RunRecords>().records;
        assert_eq!(records.len(), 1, "the list read: {records:?}");
        assert_eq!((records[0].summary.as_str(), records[0].score, records[0].turn), ("Slain on the third floor", 300, 120));
        assert_eq!((records[0].when, records[0].sections.len()), (None, 0), "what it never wrote reads as nothing");
    }

    #[test]
    fn a_slot_that_is_not_ron_is_warned_about_and_replaced_by_the_next_record() {
        let backend = Arc::new(MemoryBackend::default());
        backend.persist(RECORDS_SLOT, "this is not ron").unwrap();
        let mut app = game(&backend);
        assert!(app.world().resource::<RunRecords>().records.is_empty(), "unreadable, so the list starts empty");
        assert_eq!(backend.load(RECORDS_SLOT).unwrap().as_deref(), Some("this is not ron"), "not touched by the failed read itself");

        app.world_mut().resource_mut::<RunRecords>().file(RunRecord { score: 1, ..Default::default() });
        app.update();

        let text = backend.load(RECORDS_SLOT).unwrap().expect("written");
        assert_ne!(text, "this is not ron", "replaced by the next record filed");
        let parsed: Vec<RunRecord> = ron::from_str(&text).unwrap();
        assert_eq!(parsed.len(), 1);
    }

    #[test]
    fn records_are_never_deleted_with_the_run() {
        let backend = Arc::new(MemoryBackend::default());
        let mut app = rl_bevy::plugin::headless_app();
        rl_bevy::testing::surface(&mut app);
        app.insert_resource(Saves(backend.clone()));
        app.insert_resource(Seed(RunSeed(3)));
        app.add_plugins((crate::run::SavePlugin::new("run"), RecordsSavePlugin));
        app.finish();
        app.cleanup();

        crate::run::save_run(app.world_mut()).unwrap();
        app.world_mut().resource_mut::<RunRecords>().file(RunRecord { score: 10, ..Default::default() });
        app.update();
        assert!(backend.exists("run") && backend.exists(RECORDS_SLOT), "both slots hold something before the run ends");

        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(RunOver::won());
        app.update();
        app.update();

        assert!(!backend.exists("run"), "winning deleted the run's own slot");
        assert!(backend.exists(RECORDS_SLOT), "and left the records slot alone");
        assert_eq!(app.world().resource::<RunRecords>().records.len(), 1, "still in memory too");
    }
}
