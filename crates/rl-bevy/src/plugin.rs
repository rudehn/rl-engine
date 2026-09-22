//! Wiring: the sets, the plugins, and a headless app for tests.

use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use rl_core::RunSeed;

use crate::components::{Actor, MyTurn, Player, Position};
use crate::cue::TurnHold;
use crate::knowledge::Knowledge;
use crate::state::{EngineState, Restart, RunOver};
use crate::turn::{Acting, ActionDone, ActionRefused, AddAction, Occupancy, TurnEnd, Turns};
use crate::world::{WorldMap, WorldSettings};
use crate::{places, turn};

/// The stages of a frame while playing, in order. All in `Update`.
///
/// Games read the player's keys in `Input` and draw in `Present`. The
/// engine never names a game system; games slot into these.
///
/// Streaming runs first so the window is loaded around wherever the player
/// ended the previous frame before anyone is dealt a turn on it. Input runs
/// once per frame, before the turns, so a key pressed this frame becomes
/// one [`Intent`](crate::turn::Intent) however many passes the turn loop takes.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineSet {
    /// Step the loaded window with the player.
    Stream,
    /// Read the player's input, if the player holds [`MyTurn`].
    Input,
    /// Run the [`Turn`] schedule until the player holds a turn or nothing moves.
    Turns,
    /// Recast light, if the game has turned it on.
    Light,
    /// Recompute sight.
    Fov,
    /// Draw.
    Present,
}

/// The order the frame is drawn in.
///
/// Every set is inside [`EngineSet::Present`]. Drawing is layered, and the
/// layers belong to different crates, so they are named here rather than
/// each crate ordering itself after another crate's function.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresentSet {
    /// Words first: log lines and the status line the chrome will draw,
    /// and anything else a game works out once a frame.
    Narrate,
    /// The map view.
    Map,
    /// The status line and the log over it.
    Chrome,
    /// Whatever covers the map: the overworld screen, menus, modals.
    Overlay,
}

/// One pass of the turn loop: deal, decide, resolve, requeue.
///
/// Runs inside [`EngineSet::Turns`] as many times per frame as it takes for
/// every actor due before the player's next turn to act, so a player step
/// costs one frame however many monsters are awake. Systems here must be
/// safe to run several times in a frame: a `just_pressed` check is not,
/// which is why player input lives in [`EngineSet::Input`] instead.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Turn;

/// Where a game begins a run: the schedule its start system goes in.
///
/// Run once at startup and again after every [`Restart`], with the
/// previous run torn down and the [`Seed`](crate::seed::Seed) set for the
/// next. A game puts here what it used to put in `Startup`: the rules, the
/// map, the player, the warp in and the flip to [`EngineState::Playing`].
/// Nothing of the old run is left when it runs, so the same system starts
/// the first run and the tenth.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NewRun;

/// Where a game forgets a run: the schedule run before the engine tears one
/// down for a [`Restart`].
///
/// The engine despawns everything that stands, lies or acts on a map and
/// resets what it keeps about the run; a game puts here whatever it keeps
/// of its own, such as which regions it has populated or a flag that
/// resumes a save, so the next run does not inherit it.
#[derive(ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EndRun;

/// The stages of one [`Turn`] pass, in order.
///
/// Games put AI for their own kinds of actors in `Decide` and their own
/// action resolution in `Resolve` alongside the engine's.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TurnSet {
    /// Deal a turn.
    Schedule,
    /// Decide what to do with it: minds for the non-player actors.
    Decide,
    /// Apply the decisions.
    Resolve,
    /// Refuse whatever no resolver claimed, so an action with no resolver
    /// reads as a refusal and a warning rather than a frozen game.
    Sweep,
    /// Where a game answers what just happened: the drink that heals, the
    /// bite that poisons, the loot the dead leave, the floor that fills
    /// the first time it is entered.
    ///
    /// Inside the pass, so an effect lands before the next actor acts. A
    /// reaction that writes a request rather than a change, an affliction
    /// or a warp, has it resolved on the next pass, which is still before
    /// the player acts again. Only reactions to what the turn produced
    /// belong here: a system that scans the world every frame belongs in
    /// [`PresentSet::Narrate`] instead, since this runs once per pass and
    /// a frame may hold hundreds.
    React,
    /// What the pass's sounds reached: every noise made in it, the
    /// engine's and a game's, flooded and heard at once.
    ///
    /// After [`React`](TurnSet::React) rather than in it, so a shout a game
    /// writes there is heard in the same pass whichever order the executor
    /// ran the two in, and a replay cannot tell them apart. Empty unless
    /// the game added [`NoisePlugin`](crate::noise::NoisePlugin).
    Listen,
    /// What the pass did, read for the record: every event it raised and
    /// every answer a game gave in [`React`](TurnSet::React), read in one
    /// place once they are all in.
    ///
    /// Its own phase rather than a reader in `React`, because a reader
    /// there races the game's own reactions: whichever the executor ran
    /// first decides whether a game's line lands in this pass or trails
    /// into the next, behind that pass's events. Before
    /// [`Cleanup`](TurnSet::Cleanup), so the dead still stand where they
    /// fell. The narrator's collector runs here.
    Record,
    /// Requeue and recover.
    Cleanup,
}

/// The most passes one frame may run. A frame that hits it has a queue that
/// never reaches the player; the rest waits for the next frame rather than
/// stalling the window.
const MAX_PASSES: usize = 512;

/// Runs [`Turn`] passes until the player holds a turn, a pass changes
/// nothing, or a pass cued something worth waiting for.
///
/// The player holding a turn means the game's input system gets the next
/// frame; a pass that neither dealt, advanced nor requeued means the queue
/// is idle. Both leave at least one pass run, so an [`Intent`](crate::turn::Intent) written in
/// [`EngineSet::Input`] is always resolved in the same frame. A
/// [`TurnHold`] stops the loop after the pass that raised it and runs no
/// pass at all until it is let go: nobody holds a turn while it is up,
/// since the act that raised it was the last one dealt, so nothing a
/// player presses meanwhile could have been resolved anyway.
pub fn run_turns(world: &mut World) {
    if world.resource::<TurnHold>().is_held() {
        return;
    }
    for pass in 0..MAX_PASSES {
        world.resource_mut::<Turns>().progress = false;
        world.run_schedule(Turn);
        let player_holds = world.query_filtered::<(), (With<Player>, With<MyTurn>)>().iter(world).next().is_some();
        if player_holds || !world.resource::<Turns>().progress || world.resource::<TurnHold>().is_held() {
            return;
        }
        if pass + 1 == MAX_PASSES {
            debug!("turn loop hit {MAX_PASSES} passes in one frame; the rest waits");
        }
    }
}

/// The engine's core, and the only plugin every game needs.
///
/// The state, the system sets, the turn schedule and the loop that runs
/// it, the clock, the occupancy index, the map and its places, the run's
/// beginning and end, and the actions that need nothing else: step, wait,
/// open and close a door, go through what stands here, and bump, which
/// comes to one of the others. Everything else is a plugin of its own, and
/// a game adds the ones it wants: nothing turns itself on because a
/// resource happens to exist.
///
/// Needs a [`WorldMap`] before play begins, and checks what every other
/// plugin said it needs at the same moment, through [`Requirements`].
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<EngineState>()
            .init_resource::<Turns>()
            .init_resource::<Occupancy>()
            .init_resource::<Acting>()
            .init_resource::<WorldSettings>()
            .init_resource::<Knowledge>()
            .init_resource::<crate::minds::FlowFields>()
            .init_resource::<crate::minds::Thinking>()
            .init_resource::<TurnHold>()
            .add_message::<crate::cue::Cued>()
            .add_message::<ActionDone>()
            .add_message::<ActionRefused>()
            .add_message::<TurnEnd>()
            .add_message::<turn::Stepped>()
            .add_message::<RunOver>()
            .add_message::<Restart>()
            .add_message::<places::WarpRequest>()
            .add_message::<places::MapChanged>()
            .add_message::<places::PlaceEntered>()
            .add_message::<crate::doors::DoorEvent>()
            .add_message::<crate::bump::Bumped>()
            // A bump may come to a blow, so the message it would write exists
            // whether or not the game added combat; nothing resolves it then.
            .add_message::<turn::Intent<crate::combat::Attack>>()
            .init_schedule(Turn)
            .init_schedule(NewRun)
            .init_schedule(EndRun)
            // The loops run only while playing; drawing runs while there is a
            // world to draw, which a run that is over still is.
            .configure_sets(Update, (EngineSet::Stream, EngineSet::Input, EngineSet::Turns, EngineSet::Light, EngineSet::Fov, EngineSet::Present).chain())
            .configure_sets(
                Update,
                (EngineSet::Stream, EngineSet::Input, EngineSet::Turns, EngineSet::Light, EngineSet::Fov).run_if(in_state(EngineState::Playing)),
            )
            .configure_sets(Update, EngineSet::Present.run_if(crate::state::world_is_shown))
            .configure_sets(Update, (PresentSet::Narrate, PresentSet::Map, PresentSet::Chrome, PresentSet::Overlay).chain().in_set(EngineSet::Present))
            .configure_sets(
                Turn,
                (TurnSet::Schedule, TurnSet::Decide, TurnSet::Resolve, TurnSet::Sweep, TurnSet::React, TurnSet::Listen, TurnSet::Record, TurnSet::Cleanup)
                    .chain(),
            )
            // `Sense` is in the chain, where its own doc always said it
            // was: everything after reads what the mind sees now. It was
            // left out, so `minds::sense` ran unordered against the whole
            // pass and a mind decided on sight that may or may not have
            // been recast yet. Foundry's ambiguity test found it the moment
            // a prop held a `Viewshed` in the same pass.
            .configure_sets(
                Turn,
                (DecideSet::Sense, DecideSet::Notice, DecideSet::Offer, DecideSet::Perceive, DecideSet::Minds, DecideSet::Game).chain().in_set(TurnSet::Decide),
            )
            .configure_sets(Turn, DecideSet::Perceive.run_if(crate::minds::a_mind_holds_the_turn))
            .configure_sets(Turn, (PerceiveSet::Begin, PerceiveSet::Roster, PerceiveSet::Filter, PerceiveSet::Annotate).chain().in_set(DecideSet::Perceive))
            .configure_sets(
                Turn,
                (ResolveSet::Redirect, ResolveSet::Travel, ResolveSet::Act, ResolveSet::Fields, ResolveSet::Effects, ResolveSet::Damage)
                    .chain()
                    .in_set(TurnSet::Resolve),
            )
            .configure_sets(Turn, (FieldSet::Fire, FieldSet::Gas).chain().in_set(ResolveSet::Fields))
            .configure_sets(Turn, (LandSet::Ability, LandSet::Throw, LandSet::Shot).chain().in_set(ResolveSet::Act))
            .configure_sets(Turn, (CleanupSet::Remove, CleanupSet::Requeue).chain().in_set(TurnSet::Cleanup))
            .add_action::<turn::Step>()
            .add_action::<turn::Wait>()
            .add_action::<crate::bump::Bump>()
            .add_action::<crate::bump::Swap>()
            .add_message::<crate::bump::Swapped>()
            // The message and not the action: a bump into a prop that
            // offers one thing comes to an interaction, so the writer must
            // exist in every game, while the sweeper and the resolver are
            // `PropsPlugin`'s, which is what decides whether props run at
            // all. The minds register `Intent<Attack>` the same way.
            .add_message::<turn::Intent<crate::props::Interact>>()
            .add_action::<places::GoThrough>()
            .add_action::<crate::doors::Open>()
            .add_action::<crate::doors::Close>()
            .add_systems(Update, run_turns.in_set(EngineSet::Turns))
            .add_systems(Turn, (turn::start_pass, places::tag_new_positions, turn::admit_new_actors, turn::schedule).chain().in_set(TurnSet::Schedule))
            .add_systems(Turn, crate::bump::redirect_bumps.in_set(ResolveSet::Redirect))
            .add_systems(
                Turn,
                (
                    turn::resolve_moves,
                    crate::bump::resolve_swaps,
                    turn::resolve_waits,
                    crate::doors::resolve_opens,
                    crate::doors::resolve_closes,
                    places::resolve_warps,
                )
                    .chain()
                    .in_set(ResolveSet::Travel),
            )
            .add_systems(Turn, (turn::cleanup_turns, turn::forget_removed_blockers).chain().in_set(CleanupSet::Requeue))
            .add_systems(Turn, crate::cue::hold_for_cues.in_set(TurnSet::Cleanup))
            .needs::<WorldMap>("CorePlugin", "`WorldMap::new(tiles.tables())`, the map every engine system reads")
            .add_systems(OnEnter(EngineState::Playing), check_requirements)
            // Ambiguous with everything on purpose: it reads the state and
            // whether a map exists, and warns, so no order changes it, and a
            // game's exclusive system in `Update` should not have to say so.
            .add_systems(Update, warn_if_play_never_began.ambiguous_with_all())
            // The run's life: begun at startup, ended by the first `RunOver`,
            // torn down and begun again by a `Restart`.
            .add_systems(Startup, begin_first_run)
            .add_systems(PostUpdate, crate::state::end_runs)
            .add_systems(Last, restart_runs)
            .add_systems(OnEnter(EngineState::Idle), begin_pending_run);
        // Run on one thread. [`Turn`] is dozens of small systems, and it
        // runs once per actor turn rather than once per frame, so the
        // multi-threaded executor's per-system handoff is paid tens of
        // thousands of times a second and buys nothing: none of these
        // systems is big enough to be worth a thread.
        //
        // Measured by `benches/passes.rs`: a pass of the fifty-two systems
        // a fighting game adds cost 91 microseconds multi-threaded and 4.3
        // single-threaded, and the whole of that gap was dispatch, since a
        // build whose actors carry no `Mind` cost the same 91 as one whose
        // minds were really deciding. End to end, one player turn with a
        // hundred and twenty-eight awake minds went from 14.4 milliseconds
        // to 1.9.
        //
        // A game with a genuinely heavy system of its own in the pass can
        // put the multi-threaded executor back with the same call.
        app.edit_schedule(Turn, |schedule| {
            schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
        });
    }
}

/// Runs the game's start for the first time.
fn begin_first_run(world: &mut World) {
    world.run_schedule(NewRun);
}

/// The run a [`Restart`] asked for, waiting for the state to have left the
/// old one.
#[derive(Resource, Debug, Clone, Copy)]
struct PendingRun {
    seed: Option<RunSeed>,
}

/// Answers the frame's [`Restart`], if there was one: the game forgets the
/// run, the engine tears it down, and the state goes back to
/// [`EngineState::Idle`] so the next run is begun through the same door
/// the first was.
pub fn restart_runs(world: &mut World) {
    let asked: Option<Restart> = world.resource_mut::<Messages<Restart>>().drain().last();
    let Some(restart) = asked else { return };
    world.run_schedule(EndRun);
    clear_run(world);
    world.insert_resource(PendingRun { seed: restart.seed });
    world.resource_mut::<NextState<EngineState>>().set(EngineState::Idle);
}

/// Begins the run a [`Restart`] asked for, once the state has come round to
/// idle: sets the seed and runs the game's start.
fn begin_pending_run(world: &mut World) {
    let Some(PendingRun { seed }) = world.remove_resource::<PendingRun>() else { return };
    let previous = world.get_resource::<crate::seed::Seed>().map(|s| s.0).unwrap_or(RunSeed(0));
    let seed = seed.unwrap_or_else(|| next_run_seed(previous));
    world.insert_resource(crate::seed::Seed(seed));
    world.run_schedule(NewRun);
}

/// The seed for a run no [`Restart`] named, given the one the last run used.
///
/// Off wasm the wall clock and a per-process counter give one. On wasm
/// there is no `SystemTime`, and no engine crate reads entropy of its own,
/// so the next run is derived from the last instead: two runs in a session
/// differ, and a page opened on one seed plays the same sequence of runs.
fn next_run_seed(previous: RunSeed) -> RunSeed {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = previous;
        RunSeed::fresh()
    }
    #[cfg(target_arch = "wasm32")]
    {
        RunSeed::from_entropy(previous.derive(rl_core::SeedDomain::new(b"restart"), 0))
    }
}

/// Takes the run out of the world: everything that stands, lies or acts on
/// a map is despawned, and everything the engine keeps about a run is
/// reset. The [`WorldMap`] is removed rather than emptied, so a start that
/// forgets to make one is told so by the same check as the first time.
///
/// What a game keeps of its own is its own to forget, in [`EndRun`].
pub fn clear_run(world: &mut World) {
    let doomed: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<Position>, With<places::OnMap>, With<crate::items::Item>, With<Actor>, With<crate::combat::Dead>)>>()
        .iter(world)
        .collect();
    for e in doomed {
        world.despawn(e);
    }
    world.insert_resource(Turns::default());
    world.insert_resource(Occupancy::default());
    world.insert_resource(Acting::default());
    world.insert_resource(Knowledge::default());
    world.resource_mut::<TurnHold>().reset();
    world.resource_mut::<crate::minds::FlowFields>().invalidate();
    world.remove_resource::<WorldMap>();
    world.remove_resource::<crate::state::Ending>();
    let resets = world.remove_resource::<RunResets>().unwrap_or_default();
    for entry in &resets.0 {
        (entry.reset)(world);
    }
    world.insert_resource(resets);
}

/// What a run's teardown puts back, as the plugins that own it declared.
///
/// A list rather than a line per subsystem in [`clear_run`], so a plugin
/// that keeps something of the run says so where the resource is created
/// and a plugin nobody added leaves nothing to put back. Filled through
/// [`ResetsOnNewRun`].
#[derive(Resource, Default)]
pub struct RunResets(Vec<RunReset>);

/// One resource a run's teardown puts back.
struct RunReset {
    resource: &'static str,
    /// Boxed rather than a plain `fn`, because the constructor is a value
    /// the caller chose and cannot be monomorphised into one. Called once
    /// per run ending, so the indirection costs nothing that matters.
    reset: Box<dyn Fn(&mut World) + Send + Sync>,
}

/// Declares a resource the engine puts back when a run ends.
///
/// The counterpart of [`Needs`] for what a subsystem keeps rather than
/// what it requires: a `Fire` field, an airborne queue, what a mind was
/// offered. A resource that is not there when the run ends is left alone,
/// so this is safe to declare for something the game inserts.
///
/// What [`CorePlugin`] owns itself, the clock, the occupancy index and
/// what has acted, is put back by [`clear_run`] directly, since it is
/// there in every game and its absence would be a broken world rather than
/// a plugin nobody added.
pub trait ResetsOnNewRun {
    /// Puts `R` back to its default when a run ends, if it is there.
    fn reset_on_new_run<R: Resource + Default>(&mut self) -> &mut Self;

    /// Puts `R` back to what `make` returns, for a resource whose empty
    /// state is not its `Default`, or which has none.
    fn reset_on_new_run_with<R: Resource>(&mut self, make: fn() -> R) -> &mut Self;
}

impl ResetsOnNewRun for App {
    fn reset_on_new_run<R: Resource + Default>(&mut self) -> &mut Self {
        self.reset_on_new_run_with::<R>(R::default)
    }

    fn reset_on_new_run_with<R: Resource>(&mut self, make: fn() -> R) -> &mut Self {
        let resource = std::any::type_name::<R>();
        self.init_resource::<RunResets>();
        let mut list = self.world_mut().resource_mut::<RunResets>();
        // Once: a plugin added twice, or two plugins that share a
        // resource, must not put it back twice.
        if !list.0.iter().any(|e| e.resource == resource) {
            let reset = move |world: &mut World| {
                if world.contains_resource::<R>() {
                    world.insert_resource(make());
                }
            };
            list.0.push(RunReset { resource, reset: Box::new(reset) });
        }
        self
    }
}

/// The stages of [`TurnSet::Decide`], in order.
///
/// What an actor has noticed is settled first, so the mind that then
/// chooses is choosing on this turn's knowledge. A game that gave its
/// brains a decision of its own answers it in [`DecideSet::Game`], where
/// the choice has been made and written but nothing has acted on it yet.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DecideSet {
    /// The sight of the mind about to decide, recast if it moved or the
    /// map changed, so everything after reads what it sees now.
    Sense,
    /// Who has noticed whom, for the actor about to decide. Empty unless
    /// the game added [`StealthPlugin`](crate::stealth::StealthPlugin).
    Notice,
    /// What the actor about to decide may choose from, worked out before
    /// anything chooses: the abilities it can use, when the game added
    /// [`AbilitiesPlugin`](crate::ability::AbilitiesPlugin).
    Offer,
    /// What the mind about to decide knows, filled into
    /// [`Thinking`](crate::minds::Thinking) by every plugin that knows
    /// something a mind should, in [`PerceiveSet`] order. Runs only while a
    /// mind holds the turn.
    Perceive,
    /// The engine's minds, deciding for everyone but the player.
    Minds,
    /// The game's answer to whatever its own tactics chose.
    Game,
}

/// The stages of [`DecideSet::Perceive`], in order.
///
/// Phases rather than one set, because the contributors are not
/// independent: stealth takes hiders out of the enemies combat put in, and
/// what is annotated is annotated onto what survived. Two contributors in
/// one phase never write the same list, and the snapshot is sorted once
/// after all of them, so the order the executor ran a phase in cannot
/// reach a tactic.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PerceiveSet {
    /// The snapshot opened for the mind holding the turn.
    Begin,
    /// Who is seen, sorted into sides.
    Roster,
    /// What the mind cannot act on taken out again.
    Filter,
    /// Everything else a mind may know: what it carries and sees lying
    /// about, what it may use, where not to step, and a game's own senses.
    Annotate,
}

/// The stages of [`TurnSet::Resolve`], in order.
///
/// Moving first, then every other action, then fire and gas over the map,
/// then what ticks because a turn passed, then the damage all of it
/// produced. Fields before the ticks, so a status that fire or gas puts on
/// whoever stands in it lands and bites on the turn they stood there. Named because the systems
/// that fill them come from different plugins, which cannot chain
/// themselves together, and no plugin orders itself after another's
/// function. One turn is one action whichever set resolves it: the first
/// resolver to [`claim`](crate::turn::Resolution::claim) an actor spends
/// its turn.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolveSet {
    /// Where one intent becomes another before anything claims the turn:
    /// a bump becomes a step, an opening or a blow. An alternate action of
    /// a game's own reads its intent here, writes the one it comes to, and
    /// claims nothing.
    Redirect,
    /// Going somewhere: a step, a wait, a door, and any warp a reaction
    /// asked for, so a place a warp builds exists before anything acts in it.
    Travel,
    /// Every other action: a strike, a drink, an ability, a game's own.
    Act,
    /// What spreads over the map because a turn passed: fire, then gas, in
    /// [`FieldSet`] order.
    Fields,
    /// What a turn costs whoever is standing in it: statuses, fuel.
    Effects,
    /// The damage the pass produced, applied once.
    Damage,
}

/// The stages of [`ResolveSet::Fields`], in order.
///
/// Fire before gas, so a fire that burns a vapour away and gives off smoke
/// has that smoke spread on the same turn.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldSet {
    /// Fire catches, spreads and burns out.
    Fire,
    /// Gas spreads, fades and is breathed.
    Gas,
}

/// The landings in [`ResolveSet::Act`], in order: what was put in the air
/// on an earlier pass and comes down on this one.
///
/// A fixed order because each writes damage, and the first hit to take a
/// target to nothing is the one credited with the kill: two things landing
/// on one pass would otherwise settle who killed what by however the
/// scheduler happened to run them. Abilities, then throws, then shots, the
/// order the engine gained them in; nothing about the rules prefers one.
/// Every landing runs before its own resolver, so what lands is settled
/// before anything new is launched.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LandSet {
    /// An ability's projectile.
    Ability,
    /// A thrown item.
    Throw,
    /// A shot.
    Shot,
}

/// The stages of [`TurnSet::Cleanup`], in order.
///
/// The dead leave the queue and the index before the turns are requeued,
/// so an actor killed this pass is never dealt another.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CleanupSet {
    /// Take out whatever this pass killed.
    Remove,
    /// Requeue what acted, keep the turn of what was refused, recover what
    /// nobody moved, and forget what was despawned.
    Requeue,
}

/// Says that this plugin reads a message another plugin writes, and that
/// an empty queue is a perfectly good answer.
///
/// Bevy refuses a reader for a message nobody registered, so a system that
/// reads what an optional plugin writes panics in every game that left
/// that plugin out: a vitals panel that reads what the player heard breaks
/// a game with no noise, a prop that bursts breaks one with no combat.
/// Registering it here says the reading is optional and means it: with the
/// writer's plugin added the queue fills, without it the queue stays
/// empty, and the reader asks either way.
///
/// The opposite of [`Needs`]. `needs` is for what a plugin cannot work
/// without, and fails loudly when play begins; this is for what it can
/// work without, and says so once, where the reading is written.
///
/// ```
/// # use bevy::prelude::*;
/// # use rl_bevy::plugin::Reads;
/// # #[derive(Message)]
/// # struct Heard;
/// # let mut app = App::new();
/// // A panel that shows what was heard, in a game that may not have noise.
/// app.reads::<Heard>();
/// ```
pub trait Reads {
    /// Registers `M` unless it is already registered, because this plugin
    /// reads it and an empty queue is an answer.
    fn reads<M: Message>(&mut self) -> &mut Self;
}

impl Reads for App {
    fn reads<M: Message>(&mut self) -> &mut Self {
        // `add_message` is itself idempotent; this is about saying why, in
        // one place, rather than five plugins each explaining themselves.
        self.add_message::<M>()
    }
}

/// Everything the added plugins cannot work without, checked together when
/// play begins.
///
/// A plugin records what it needs while the app is built, with a hint at
/// how a game makes it. On entering [`EngineState::Playing`] one check reads
/// them all, and if anything is missing it panics once, listing every piece
/// with its hint, so a game's setup is fixed in one run rather than one
/// crash at a time. A missing rule table is a mistake in the setup, and
/// saying so beats a subsystem quietly doing nothing all run.
#[derive(Resource, Default)]
pub struct Requirements(Vec<Requirement>);

/// One thing one plugin needs.
struct Requirement {
    plugin: &'static str,
    resource: &'static str,
    hint: &'static str,
    present: fn(&World) -> bool,
}

impl Requirements {
    /// Every requirement `world` does not meet, one line each: who needs
    /// what, and how to make it.
    pub fn missing(&self, world: &World) -> Vec<String> {
        self.0.iter().filter(|r| !(r.present)(world)).map(|r| format!("{} needs {}: {}", r.plugin, r.resource, r.hint)).collect()
    }
}

/// Declares what a plugin cannot work without.
pub trait Needs {
    /// Records that `plugin` needs `R` inserted before play begins, with
    /// `hint` saying how a game makes one.
    fn needs<R: Resource>(&mut self, plugin: &'static str, hint: &'static str) -> &mut Self;
}

impl Needs for App {
    fn needs<R: Resource>(&mut self, plugin: &'static str, hint: &'static str) -> &mut Self {
        fn present<R: Resource>(world: &World) -> bool {
            world.contains_resource::<R>()
        }
        let resource = std::any::type_name::<R>();
        self.init_resource::<Requirements>();
        let mut list = self.world_mut().resource_mut::<Requirements>();
        if !list.0.iter().any(|r| r.plugin == plugin && r.resource == resource) {
            list.0.push(Requirement { plugin, resource, hint, present: present::<R> });
        }
        self
    }
}

/// Panics once, listing every requirement the game's setup left unmet.
pub fn check_requirements(world: &World) {
    let Some(requirements) = world.get_resource::<Requirements>() else { return };
    let missing = requirements.missing(world);
    if !missing.is_empty() {
        let pieces = if missing.len() == 1 { "one piece is" } else { "some pieces are" };
        panic!("play cannot begin: {pieces} missing.\n  {}\nInsert each before setting EngineState::Playing.", missing.join("\n  "));
    }
}

/// Warns once when a world has been built and play never began.
///
/// Every engine system waits on [`EngineState::Playing`], so a game that
/// built its map and forgot to set the state gets a still window and no
/// other sign of what is wrong.
fn warn_if_play_never_began(mut frames: Local<u32>, state: Res<State<EngineState>>, map: Option<Res<WorldMap>>) {
    if *state.get() != EngineState::Idle || map.is_none() {
        *frames = 0;
        return;
    }
    *frames += 1;
    if *frames == 120 {
        warn!("a WorldMap was inserted but EngineState is still Idle after 120 frames; nothing runs until the game sets EngineState::Playing");
    }
}

/// Asserts that a plugin this one depends on was added too.
pub fn depends_on<P: Plugin>(app: &App, plugin: &'static str) {
    assert!(app.is_plugin_added::<P>(), "{plugin} needs {} added as well", std::any::type_name::<P>());
}

/// A headless app with the engine plugins and no window, for tests in the
/// engine and in games.
pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin, CorePlugin));
    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
    use crate::turn::{Action, Intent, Step, Wait};
    use rl_core::{Direction, Point};
    use rl_grid::{TileId, TileProps, TileRegistry};

    fn app_with_world() -> (App, Point) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin));
        let mut tiles = TileRegistry::standard();
        tiles.register(TileProps::floor("mud").move_cost(200)).unwrap();
        let start = crate::testing::surface_with(&mut app, tiles);
        (app, start)
    }

    fn spawn_player(app: &mut App, at: Point) -> Entity {
        let e = app.world_mut().spawn((Actor, Player, Blocks, Position(at), Viewshed::new(6), RevealsMap)).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        e
    }

    fn intend<A: Action>(app: &mut App, actor: Entity, action: A) {
        app.world_mut().write_message(Intent::new(actor, action));
    }

    #[test]
    fn the_player_is_dealt_a_turn_and_walks() {
        let (mut app, start) = app_with_world();
        let player = spawn_player(&mut app, start);
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds the first turn");
        assert!(app.world().resource::<WorldMap>().is_loaded(start), "the window loaded around the player");
        intend(&mut app, player, Step(Direction::East));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start.offset(1, 0));
        assert_eq!(app.world().resource::<Turns>().now(), 100, "the clock ran on to the player's next turn within the frame");
        assert!(app.world().get::<MyTurn>(player).is_some(), "and dealt it");
        assert_eq!(app.world().resource::<Occupancy>().at(start.offset(1, 0)), &[player]);
    }

    #[test]
    fn a_refused_move_costs_no_time_and_keeps_the_turn() {
        let (mut app, start) = app_with_world();
        let player = spawn_player(&mut app, start);
        app.update();
        app.update();
        app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(1, 0), TileId(1));
        intend(&mut app, player, Step(Direction::East));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start);
        assert!(app.world().get::<MyTurn>(player).is_some());
        assert_eq!(app.world().resource::<Turns>().now(), 0);
    }

    #[test]
    fn everyone_due_before_the_player_acts_in_the_frame_the_player_did() {
        let (mut app, start) = app_with_world();
        let player = spawn_player(&mut app, start);
        // No mind, so nobody decides for it: the recovery net charges it a
        // wait each time it is dealt a turn. At speed 200 a wait costs 50.
        let fast = app.world_mut().spawn((Actor, Blocks, Position(start.offset(2, 0)), Speed(200))).id();
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player was admitted first and holds the turn");
        intend(&mut app, player, Wait);
        app.update();
        // One frame: the player waited (100), the other actor was dealt a
        // turn at 0 and at 50, and the clock reached the player's turn at
        // 100, where the player wins the tie as the earlier insertion.
        let t = app.world().resource::<Turns>();
        assert_eq!(t.now(), 100);
        assert_eq!(t.peek_time(), Some(100), "the other actor is due at 100 too, behind the player");
        assert!(app.world().get::<MyTurn>(player).is_some());
        assert!(app.world().get::<MyTurn>(fast).is_none());
        let mut ends = app.world_mut().resource_mut::<Messages<TurnEnd>>();
        assert_eq!(ends.drain().map(|e| e.turn).collect::<Vec<_>>(), vec![1], "one whole turn passed");
    }

    #[test]
    fn an_idle_frame_runs_one_pass_and_a_key_moves_the_player_once() {
        let (mut app, start) = app_with_world();
        let player = spawn_player(&mut app, start);
        app.update();
        app.update();
        // Two intents for the same turn: only the first resolves, the second
        // finds the player no longer holding the turn it was written for.
        intend(&mut app, player, Step(Direction::East));
        intend(&mut app, player, Step(Direction::East));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start.offset(1, 0));
        // Idle frames leave the clock alone.
        app.update();
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), 100);
        assert!(app.world().get::<MyTurn>(player).is_some());
    }

    /// Every missing piece is reported together, each with how to make it,
    /// so a game's setup is fixed in one run rather than one crash at a time.
    #[test]
    fn every_missing_requirement_is_listed_at_once_with_how_to_make_it() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin));
        let missing = app.world().resource::<Requirements>().missing(app.world());
        assert_eq!(missing.len(), 4, "the map, the rules, the registries and the seed: {missing:#?}");
        assert!(missing.iter().any(|m| m.starts_with("CorePlugin needs") && m.contains("WorldMap::new")), "{missing:#?}");
        assert!(missing.iter().any(|m| m.starts_with("CombatPlugin needs") && m.contains("CombatRules::new")), "{missing:#?}");
        assert!(missing.iter().any(|m| m.starts_with("CombatPlugin needs") && m.contains("Seed(RunSeed(n))")), "{missing:#?}");
        assert!(missing.iter().any(|m| m.starts_with("CombatPlugin needs") && m.contains("`Registries`")), "{missing:#?}");

        // A plugin added twice, or two asking for the same thing, is one line
        // per plugin rather than a list that repeats itself.
        app.needs::<WorldMap>("CorePlugin", "again");
        assert_eq!(app.world().resource::<Requirements>().missing(app.world()).len(), 4);
    }

    #[test]
    #[should_panic(expected = "CombatPlugin needs")]
    fn combat_without_its_rules_says_so_when_play_begins() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
    }

    #[test]
    #[should_panic(expected = "StatusPlugin needs")]
    fn a_plugin_without_the_plugin_it_depends_on_says_so_at_build() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin, CorePlugin, crate::status::StatusPlugin));
        app.finish();
    }

    /// An action of a game's own. The engine has never heard of it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Shout;
    impl Action for Shout {}

    /// How many shouts the game's own resolver heard.
    #[derive(Resource, Default)]
    struct Heard(u32);

    fn resolve_shouts(mut intents: MessageReader<Intent<Shout>>, mut resolution: crate::turn::Resolution, mut heard: ResMut<Heard>) {
        for intent in intents.read() {
            if !resolution.claim(intent.actor) {
                continue;
            }
            heard.0 += 1;
            resolution.done(intent.actor, rl_core::turn::BASE_ACTION_COST);
        }
    }

    /// A game's action that can never be done.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Leap;
    impl Action for Leap {}

    fn resolve_leaps(mut intents: MessageReader<Intent<Leap>>, mut resolution: crate::turn::Resolution) {
        for intent in intents.read() {
            if resolution.claim(intent.actor) {
                resolution.failed(intent.actor, rl_core::turn::BASE_ACTION_COST);
            }
        }
    }

    /// A game's decider that tries a leap for every monster dealt a turn.
    fn leap_every_turn(mut leaps: MessageWriter<Intent<Leap>>, holding: Query<Entity, (With<MyTurn>, Without<Player>)>) {
        for monster in &holding {
            leaps.write(Intent::new(monster, Leap));
        }
    }

    /// The rule for a failure lives in `Resolution`, so a game's resolver
    /// inherits it: the player keeps its turn at no cost, and a monster is
    /// charged instead of refused, so it cannot ask again forever.
    #[test]
    fn a_failed_action_keeps_the_players_turn_and_charges_a_monster() {
        let (mut app, start) = app_with_world();
        app.add_action::<Leap>().add_systems(Turn, (leap_every_turn.in_set(DecideSet::Game), resolve_leaps.in_set(ResolveSet::Act)));
        let player = spawn_player(&mut app, start);
        let monster = app.world_mut().spawn((Actor, Blocks, Position(start.offset(2, 0)))).id();
        app.update();
        app.update();
        let mut refused: Vec<Entity> = Vec::new();

        intend(&mut app, player, Leap);
        app.update();
        refused.extend(app.world_mut().resource_mut::<Messages<ActionRefused>>().drain().map(|r| r.actor));
        assert_eq!(app.world().resource::<Turns>().now(), 0, "the player's failed leap cost nothing");
        assert!(app.world().get::<MyTurn>(player).is_some(), "and the turn is still the player's");

        intend(&mut app, player, Wait);
        app.update();
        refused.extend(app.world_mut().resource_mut::<Messages<ActionRefused>>().drain().map(|r| r.actor));
        assert_eq!(app.world().resource::<Turns>().now(), 100, "the monster's failed leap was charged, so the clock reached the player again");
        assert!(app.world().get::<MyTurn>(player).is_some());
        assert_eq!(refused, vec![player], "only the player was ever refused, never {monster:?}");
    }

    #[test]
    fn a_game_action_the_engine_never_heard_of_spends_the_turn() {
        let (mut app, start) = app_with_world();
        app.init_resource::<Heard>().add_action::<Shout>().add_systems(Turn, resolve_shouts.in_set(TurnSet::Resolve));
        let player = spawn_player(&mut app, start);
        app.update();
        app.update();

        intend(&mut app, player, Shout);
        app.update();
        assert_eq!(app.world().resource::<Heard>().0, 1, "the game's resolver saw it");
        assert_eq!(app.world().resource::<Turns>().now(), 100, "and it cost a turn");
    }

    #[test]
    fn an_action_nobody_resolves_is_refused_rather_than_left_to_hang() {
        let (mut app, start) = app_with_world();
        // Registered, and no resolver: the mistake a game makes.
        app.add_action::<Shout>();
        let player = spawn_player(&mut app, start);
        app.update();
        app.update();

        let before = app.world().resource::<Turns>().now();
        intend(&mut app, player, Shout);
        app.update();
        let refused: Vec<Entity> = app.world_mut().resource_mut::<Messages<ActionRefused>>().drain().map(|r| r.actor).collect();
        assert_eq!(refused, vec![player], "the sweep refused it");
        assert_eq!(app.world().resource::<Turns>().now(), before, "no time passed");
        assert!(app.world().get::<MyTurn>(player).is_some(), "and the player still holds the turn");
    }

    #[test]
    fn walking_across_a_region_boundary_streams_the_window_and_keeps_edits() {
        let (mut app, start) = app_with_world();
        let player = spawn_player(&mut app, start);
        app.update();
        app.update();
        let first_window = app.world().resource::<WorldMap>().window();
        let edited = start.offset(0, 1);
        assert!(app.world_mut().resource_mut::<WorldMap>().set_tile(edited, TileId(1)));
        // Walk 16 tiles east, one region over.
        for _ in 0..16 {
            intend(&mut app, player, Step(Direction::East));
            app.update();
            app.update();
        }
        let map = app.world().resource::<WorldMap>();
        assert_ne!(map.window(), first_window, "the window followed the player");
        assert!(map.is_loaded(app.world().get::<Position>(player).unwrap().0));
        // Walk back far enough that the edited region unloads, then return.
        for _ in 0..32 {
            intend(&mut app, player, Step(Direction::East));
            app.update();
            app.update();
        }
        let map = app.world().resource::<WorldMap>();
        assert!(!map.is_loaded(edited), "the edited region left the window");
        assert_eq!(map.stored_deltas(), 1);
        for _ in 0..48 {
            intend(&mut app, player, Step(Direction::West));
            app.update();
            app.update();
        }
        let map = app.world().resource::<WorldMap>();
        assert_eq!(map.tile(edited), Some(TileId(1)), "the edit was replayed on reload");
    }

    #[test]
    fn sight_is_computed_and_reveals_the_map() {
        let (mut app, start) = app_with_world();
        let player = spawn_player(&mut app, start);
        app.update();
        app.update();
        let v = app.world().get::<Viewshed>(player).unwrap();
        assert!(!v.dirty);
        assert!(v.can_see(start));
        assert!(v.can_see(start.offset(5, 0)));
        assert!(!v.can_see(start.offset(7, 0)), "beyond range");
        let k = app.world().resource::<Knowledge>();
        assert!(k.is_explored(start.offset(3, 3)));
        assert!(k.explored_count() > 50);
    }

    /// One walled hall, whatever map is asked for, entered at (2, 2).
    struct Hall(TileRegistry);

    impl crate::places::PlaceRules for Hall {
        fn build(&self, _: crate::places::MapId, _: Option<&rl_world::WorldGraph>) -> Result<crate::places::PlaceBuild, rl_mapgen::BuildError> {
            let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
            let terrain = rl_grid::Terrain::from_fn(12, 8, |p| if p.x == 0 || p.y == 0 || p.x == 11 || p.y == 7 { wall } else { floor });
            Ok(crate::places::PlaceBuild { terrain, entry: Point::new(2, 2), exit: None, spots: Vec::new() })
        }
    }

    /// Stands three actors beside the entry the first time a place is
    /// entered, in `TurnSet::React`, the way a game populates one.
    fn populate(mut commands: Commands, mut entered: MessageReader<crate::places::PlaceEntered>) {
        for ev in entered.read().filter(|ev| ev.first) {
            for dx in [2, 4, 6] {
                commands.spawn((Actor, Blocks, Position(ev.entry.offset(dx, 2))));
            }
        }
    }

    /// Whether each actor dealt a turn was the player, in the order dealt,
    /// read inside the pass that dealt it.
    #[derive(Resource, Default)]
    struct Dealt(Vec<bool>);

    fn record_deals(mut dealt: ResMut<Dealt>, fresh: Query<Has<Player>, Added<MyTurn>>) {
        dealt.0.extend(fresh.iter());
    }

    #[test]
    fn a_player_spawned_as_an_actor_and_warped_into_a_place_takes_the_first_turn_there() {
        let mut app = headless_app();
        let tiles = TileRegistry::standard();
        app.insert_resource(crate::world::WorldMap::new(tiles.tables()));
        app.insert_resource(crate::places::PlaceRulesRes(Box::new(Hall(tiles))));
        app.init_resource::<Dealt>();
        app.add_systems(Turn, (populate.in_set(TurnSet::React), record_deals.in_set(TurnSet::Decide)));
        // Spawned the way a game starts a run: an actor at the origin of a
        // surface it has none of, and a warp onto its first place.
        let player = spawn_player(&mut app, Point::ZERO);
        app.world_mut().write_message(crate::places::WarpRequest::into_place(player, crate::places::MapId(1)));
        app.update();
        app.update();
        let dealt = &app.world().resource::<Dealt>().0;
        assert_eq!(dealt.first(), Some(&true), "{dealt:?}: someone was dealt a turn before the player");
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds the first turn");
        assert_eq!(app.world().resource::<Turns>().now(), 0, "and holds it at the clock's start, not frozen a step or two ahead");
    }

    /// A resource one plugin keeps of the run, declared the way a
    /// subsystem declares its own.
    #[derive(Resource, Default, PartialEq, Debug)]
    struct Kept(u32);

    /// One whose empty state is not its `Default`.
    #[derive(Resource, PartialEq, Debug)]
    struct Banked(u32);

    impl Banked {
        fn empty() -> Self {
            Self(7)
        }
    }

    #[test]
    fn a_resource_that_registered_a_reset_is_put_back_when_the_run_ends_and_one_that_did_not_is_left() {
        let mut app = headless_app();
        app.insert_resource(crate::world::WorldMap::new(TileRegistry::standard().tables()));
        app.insert_resource(Kept(3)).insert_resource(Banked(3));
        app.reset_on_new_run::<Kept>().reset_on_new_run_with::<Banked>(Banked::empty);
        // Something nobody declared, to show the list is what decides.
        #[derive(Resource, Default, PartialEq, Debug)]
        struct Undeclared(u32);
        app.insert_resource(Undeclared(3));

        clear_run(app.world_mut());

        assert_eq!(app.world().resource::<Kept>(), &Kept(0), "put back to its default");
        assert_eq!(app.world().resource::<Banked>(), &Banked(7), "put back to what its own constructor says empty is");
        assert_eq!(app.world().resource::<Undeclared>(), &Undeclared(3), "nobody declared it, so nobody puts it back");
    }

    #[test]
    fn a_reset_declared_twice_is_run_once_and_a_resource_that_is_not_there_is_not_made() {
        let mut app = headless_app();
        app.insert_resource(crate::world::WorldMap::new(TileRegistry::standard().tables()));
        app.reset_on_new_run::<Kept>().reset_on_new_run::<Kept>();
        assert_eq!(app.world().resource::<RunResets>().0.len(), 1, "declared twice, listed once");

        clear_run(app.world_mut());
        assert!(app.world().get_resource::<Kept>().is_none(), "a resource the game never inserted is not conjured by its reset");
    }
}
