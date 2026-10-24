//! What the screen a run ends on has to say, beyond the words the engine
//! already knows.
//!
//! [`Ending`] carries the outcome, the seed, the turn and the epitaph, and
//! the menu draws those itself. What it cannot know is what the run was
//! *about*: the take, the decks cleared, what the player was made of. That
//! arrives here as a section, pushed by the game in
//! [`ViewSet::Annotate`](crate::ViewSet) the way a facet is pushed onto a
//! row, and the ending screen draws whatever it finds.
//!
//! This used to be a file. A `Morgue` in `atglyph-save` took the same sections,
//! rendered them and wrote them to disk, and the menu's presenter composed
//! and filed it, which is how a UI crate came to depend on the save crate
//! for one screen. Nothing read the file in play, so the sections are shown
//! instead of stored, and the dependency is gone.

use atglyph_bevy::EngineApp;
use atglyph_bevy::{Ending, RunRecords};
use bevy::prelude::*;

/// One thing a game has to say about the run that just ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndingSection {
    /// What it is called, drawn as its own row.
    pub heading: String,
    /// What it says. Newlines are rows; the screen wraps and clips to its
    /// own rectangle, so a section longer than the screen is cut rather
    /// than scrolled.
    pub body: String,
}

/// What a game has added to the screen a run ends on.
///
/// Cleared and refilled every frame like every other view, so a game
/// pushes on each frame it wants the row rather than once, and a run that
/// starts again shows nothing until something pushes again.
#[derive(Resource, Debug, Default)]
pub struct EndingView {
    /// The run in one line, the summary of the record filed for it, which
    /// the records list shows it by; empty when there is none.
    pub summary: String,
    /// The sections, in the order they were pushed.
    pub sections: Vec<EndingSection>,
}

impl EndingView {
    /// Adds a section. Called from `ViewSet::Annotate`.
    pub fn section(&mut self, heading: impl Into<String>, body: impl Into<String>) -> &mut Self {
        self.sections.push(EndingSection { heading: heading.into(), body: body.into() });
        self
    }

    /// Whether the game added anything.
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

/// Empties [`EndingView`] so the frame's annotate systems refill it, then
/// adds the record `RecordsPlugin` filed for this run, when there is one:
/// its summary and its sections.
///
/// Almost nothing to collect on its own: everything the engine itself
/// knows about an ending is already in [`Ending`], which the menu reads
/// directly. The one thing worth collecting is [`RunRecords::this_run`],
/// so a game that records its run never pushes the same sections a second
/// time and the end screen and a records screen cannot disagree about
/// what the run came to. A game with no `RecordsPlugin` at all reads
/// `records` as `None` and gets exactly the old behaviour.
pub(crate) fn collect_ending(mut view: ResMut<EndingView>, ending: Option<Res<Ending>>, records: Option<Res<RunRecords>>) {
    view.summary.clear();
    view.sections.clear();
    // Nothing to say while the run is still going: a game that pushes
    // unconditionally still gets an empty screen until there is an ending
    // to draw it on.
    if ending.is_none() {
        view.sections.shrink_to_fit();
        return;
    }
    if let Some(record) = records.as_deref().and_then(|kept| kept.this_run.as_ref()) {
        view.summary.clone_from(&record.summary);
        view.sections.extend(record.sections.iter().map(|section| EndingSection { heading: section.heading.clone(), body: section.body.clone() }));
    }
}

/// Keeps [`EndingView`] current.
///
/// Added by [`GameMenuPanel`](crate::GameMenuPanel), which is the only
/// thing that draws it; a game that wants the sections without the menu
/// adds this itself and reads the view.
pub struct EndingViewPlugin;

impl Plugin for EndingViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EndingView>().add_systems(Update, collect_ending.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::UiPlugin>("EndingViewPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_bevy::{Outcome, RecordSection, RunRecord, RunRecords};
    use atglyph_core::RunSeed;
    use bevy::ecs::system::RunSystemOnce;

    /// `RecordsPlugin` files one record as `this_run` for the ending that
    /// just happened; `collect_ending` reads it so the end screen and a
    /// records screen never disagree about what the run came to.
    #[test]
    fn the_ending_screen_shows_the_sections_of_this_runs_record() {
        let mut app = App::new();
        app.init_resource::<EndingView>();
        app.insert_resource(Ending { outcome: Outcome::Won, epitaph: String::new(), seed: RunSeed(1), turn: 10 });
        app.insert_resource(RunRecords {
            records: Vec::new(),
            this_run: Some(RunRecord { sections: vec![RecordSection { heading: "Score".into(), body: "100".into() }], ..RunRecord::default() }),
        });
        app.world_mut().run_system_once(collect_ending).expect("the collector runs");
        let view = app.world().resource::<EndingView>();
        assert_eq!(view.sections, vec![EndingSection { heading: "Score".into(), body: "100".into() }]);
    }

    /// The record's one line comes with its sections, so the end screen
    /// says what the records list says of the run, and a new run, with no
    /// record yet, has none.
    #[test]
    fn the_ending_screen_shows_the_summary_of_this_runs_record() {
        let mut app = App::new();
        app.init_resource::<EndingView>();
        app.insert_resource(Ending { outcome: Outcome::Died { by: None }, epitaph: String::new(), seed: RunSeed(1), turn: 10 });
        app.insert_resource(RunRecords { records: Vec::new(), this_run: Some(RunRecord { summary: "Lost on the third day".into(), ..RunRecord::default() }) });
        app.world_mut().run_system_once(collect_ending).expect("the collector runs");
        assert_eq!(app.world().resource::<EndingView>().summary, "Lost on the third day");
        app.world_mut().resource_mut::<RunRecords>().this_run = None;
        app.world_mut().run_system_once(collect_ending).expect("the collector runs");
        assert_eq!(app.world().resource::<EndingView>().summary, "", "forgotten with the record");
    }

    /// A game with no `RecordsPlugin` at all: nothing to
    /// read, so the collector clears and adds nothing, as it always did.
    #[test]
    fn with_no_run_records_at_all_the_collector_only_clears() {
        let mut app = App::new();
        app.init_resource::<EndingView>();
        app.insert_resource(Ending { outcome: Outcome::Won, epitaph: String::new(), seed: RunSeed(1), turn: 10 });
        app.world_mut().run_system_once(collect_ending).expect("the collector runs");
        assert!(app.world().resource::<EndingView>().is_empty());
    }
}
