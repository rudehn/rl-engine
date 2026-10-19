//! The run's start as something a screen can hold, and what the run's
//! save slot holds.
//!
//! The engine begins the first run at startup by running [`NewRun`]. A
//! game with a title screen wants nothing begun until the player picks,
//! so while [`HoldStart`] is present the engine begins nothing, and the
//! screen begins the run through [`begin_run`], the same `NewRun` the
//! engine runs at startup and after a restart: no game's start system asks
//! whether a title is up. [`LeaveRun`] tears a run down the way a restart
//! does and holds the start again, which is the way back from a run to its
//! title.
//!
//! What a title offers depends on what was saved, and saving is
//! `rl-save`'s, which `rl-ui` may not depend on. So the words between them
//! live here, in the crate both depend on, as settings do: [`SavedRun`] is
//! what the slot holds, [`ContinueRun`] asks a game's start to restore the
//! saved run, and [`AbandonRun`] asks whoever keeps the slot to forget it.

use bevy::prelude::*;

use crate::plugin::NewRun;

/// Holds the run back while a title screen stands.
///
/// Present at startup, the engine begins no run: no map is built, the
/// state stays [`EngineState::Idle`](crate::state::EngineState::Idle) and
/// the screen has the window to itself. [`begin_run`] takes it away and
/// [`LeaveRun`] puts it back. A resource rather than a flag a game's start
/// checks, so the start is written once and runs the same way whoever
/// asked for it.
#[derive(Resource, Default)]
pub struct HoldStart;

/// Begins a run: lowers the [`HoldStart`], if a title raised one, and runs
/// the game's [`NewRun`].
///
/// The one door a title uses, and the one the engine uses itself at
/// startup and after a restart. `NewRun` rather than a
/// [`Restart`](crate::state::Restart), since a restart tears a run down and
/// at a title there is none. It begins on whatever
/// [`Seed`](crate::seed::Seed) stands, the game's own at launch or the
/// fresh one [`LeaveRun`] set, so it has no seed to choose. Exclusive, so
/// a screen that reads its keys in a system queues it as a command.
pub fn begin_run(world: &mut World) {
    world.remove_resource::<HoldStart>();
    world.run_schedule(NewRun);
}

/// What the run's save slot holds, read by a title: whether Continue can
/// be offered, and whether New Game must ask before a run is given up.
///
/// Plain data, so a screen reads it knowing nothing of storage. Inserted
/// and kept by whoever keeps the slot, `rl-save`'s `SavePlugin`, from
/// before the first frame. A game that saves nothing has none, and that is
/// the word for it: there is no slot, so nothing could ever be continued
/// and a title offers no Continue at all, where an [`Empty`](Self::Empty)
/// slot could hold a run once one is left for the title.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavedRun {
    /// Nothing: there is no run to continue.
    #[default]
    Empty,
    /// A run this build can read and continue.
    Readable,
    /// Something this build cannot read, of another version or damaged. It
    /// cannot be continued, and it is still a run a new one would replace,
    /// so a title asks before beginning one over it.
    Unreadable,
}

/// Asks the next run's start to continue the saved run rather than begin
/// afresh.
///
/// A title inserts it before [`begin_run`]. A game's [`NewRun`] systems
/// restore the saved run while it is present, and whichever restores it
/// removes it, so the start after that, a restart's, begins afresh.
#[derive(Resource, Default)]
pub struct ContinueRun;

/// Asks whoever keeps the run's save slot to forget it: the run in
/// progress is given up for a new one.
///
/// Triggered on the world rather than written as a message, so it is
/// answered before the trigger returns: a title triggers it and begins the
/// new run in the same command, and the new run's start never sees the
/// old one's slot. Whoever keeps the slot observes it, and `rl-save`'s
/// observer expects `Saves` to be present, which it is wherever
/// [`SavedRun`] holds something, the only time a title asks.
#[derive(Event, Debug, Clone, Copy)]
pub struct AbandonRun;

/// Leaves the run for the title: the run is torn down as a
/// [`Restart`](crate::state::Restart) tears it down, and the start is held
/// with [`HoldStart`] rather than another run begun.
///
/// Answered at the end of the frame it is written in, in
/// [`EndOfFrame::Restart`](crate::plugin::EndOfFrame::Restart). Whoever
/// keeps the save slot writes a run still in play down first, in
/// [`EndOfFrame::Save`](crate::plugin::EndOfFrame::Save), so Continue picks
/// it up where it was left; a run that is over is only left. It wins over
/// a restart asked for in the same frame, whose new run would bury the one
/// just written down, and with no run standing it changes nothing.
///
/// The run's seed goes with it: the next run the title begins is on a
/// fresh one, set as a restart without a seed sets it, since New Game on
/// the seed just played would be that run again from its first turn.
/// Continue is unaffected, since a game's restore builds the world again
/// from the seed its save holds.
#[derive(Message, Debug, Clone, Copy)]
pub struct LeaveRun;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, Player, Position, RevealsMap, Viewshed};
    use crate::plugin::{EndRun, headless_app};
    use crate::seed::Seed;
    use crate::state::{EngineState, Restart};
    use crate::world::WorldMap;
    use rl_core::RunSeed;

    /// How many runs the game's start has begun and its end has forgotten,
    /// and the seed each run was begun on.
    #[derive(Resource, Default)]
    struct Runs {
        begun: u32,
        forgotten: u32,
        seeds: Vec<u64>,
    }

    fn begin(mut runs: ResMut<Runs>, seed: Res<Seed>) {
        runs.begun += 1;
        runs.seeds.push(seed.0.0);
    }

    fn forget(mut runs: ResMut<Runs>) {
        runs.forgotten += 1;
    }

    /// An engine app launched on seed five, whose game counts its starts
    /// and its ends, where a game's start and its forgetting would run.
    fn counted() -> App {
        let mut app = headless_app();
        app.insert_resource(Seed(RunSeed(5))).init_resource::<Runs>().add_systems(NewRun, begin).add_systems(EndRun, forget);
        app
    }

    fn runs(app: &App) -> (u32, u32) {
        let runs = app.world().resource::<Runs>();
        (runs.begun, runs.forgotten)
    }

    /// A run standing on the test surface, a player in it, played.
    fn playing() -> App {
        let mut app = counted();
        app.add_plugins(crate::fov::FovPlugin);
        let start = crate::testing::ground(&mut app);
        app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app
    }

    fn state(app: &App) -> EngineState {
        *app.world().resource::<State<EngineState>>().get()
    }

    #[test]
    fn with_the_start_held_startup_begins_no_run() {
        let mut app = counted();
        app.init_resource::<HoldStart>();
        app.update();
        app.update();
        assert_eq!(runs(&app).0, 0, "nothing begun while the start is held");
        begin_run(app.world_mut());
        assert_eq!(runs(&app).0, 1, "the title's door begins it");
        assert!(!app.world().contains_resource::<HoldStart>(), "and lowers the hold, so the title is down while the run comes up");
    }

    #[test]
    fn without_a_hold_startup_begins_the_run_as_before() {
        let mut app = counted();
        app.update();
        app.update();
        assert_eq!(runs(&app).0, 1);
    }

    #[test]
    fn leaving_a_run_tears_it_down_and_holds_the_start() {
        let mut app = playing();
        assert_eq!(runs(&app), (1, 0), "begun at startup, as every run is");
        app.world_mut().write_message(LeaveRun);
        app.update();
        // The state comes round to idle on the next frame, which is where a
        // restart would begin its new run.
        app.update();
        assert_eq!(state(&app), EngineState::Idle);
        assert!(!app.world().contains_resource::<WorldMap>(), "the run was torn down");
        assert!(app.world_mut().query::<&Player>().iter(app.world()).next().is_none(), "the player with it");
        assert!(app.world().contains_resource::<HoldStart>(), "and the start is held for the title");
        assert_eq!(runs(&app), (1, 1), "the game forgot the run, and no other was begun");
    }

    /// The title's New Game after a run was left for it, from the screen a
    /// run ends on or from the pause menu: the run it begins is a new one,
    /// on a seed of its own as a restart's is, and not the run just left
    /// played again from its first turn.
    #[test]
    fn a_run_begun_after_one_was_left_is_on_a_fresh_seed() {
        let mut app = playing();
        app.world_mut().write_message(LeaveRun);
        app.update();
        app.update();
        begin_run(app.world_mut());
        let seeds = &app.world().resource::<Runs>().seeds;
        assert_eq!(seeds.len(), 2, "two runs begun: {seeds:?}");
        assert_eq!(seeds[0], 5, "the first on the seed the launch named");
        assert_ne!(seeds[1], seeds[0], "and the second on a seed of its own");
    }

    /// A restart and a leaving in one frame leave: the run left is written
    /// down to be continued, and a new run would bury it.
    #[test]
    fn leaving_wins_over_a_restart_asked_in_the_same_frame() {
        let mut app = playing();
        app.world_mut().write_message(Restart::fresh());
        app.world_mut().write_message(LeaveRun);
        app.update();
        app.update();
        assert_eq!(state(&app), EngineState::Idle);
        assert!(app.world().contains_resource::<HoldStart>());
        assert_eq!(runs(&app), (1, 1), "torn down once, and nothing begun");
    }

    /// With no run standing there is nothing to leave: a title already up
    /// stays up, and nothing a game keeps of a run is forgotten, the save
    /// the title offers to continue included.
    #[test]
    fn leaving_with_no_run_standing_changes_nothing() {
        let mut app = counted();
        app.init_resource::<HoldStart>();
        app.update();
        app.world_mut().write_message(LeaveRun);
        app.update();
        app.update();
        assert_eq!(runs(&app), (0, 0), "nothing begun and nothing forgotten");
        assert!(app.world().contains_resource::<HoldStart>());
    }
}
