//! Work: an actor doing one thing across many turns.
//!
//! A mind begins work by deciding [`Decision::Work`](rl_rules::ai::Decision::Work),
//! which becomes a [`BeginWork`] intent; anything else begins it through
//! [`Works::begin`]. Either way the turn it is begun on is its first, and
//! the actor carries [`Working`] until it is done. On every turn after,
//! [`continue_work`] claims the actor's decision before any mind is asked
//! and writes a [`Toil`], so a busy actor never thinks, and
//! [`resolve_toil`] spends the turn as a wait costs and counts it. On the
//! last it writes [`WorkDone`], which is where a game says what finishing
//! means; [`resolve_begins`] is what turns a mind's decision into work.
//! [`WorkBegan`] is written when work starts. [`break_work`] breaks it off
//! with a [`WorkBroken`] and its [`BreakReason`] when the worker is hurt or
//! dies, when its target is gone or out of reach, or when another worker
//! finishes the same target; a game stops it with [`Works::stop`]. What a
//! kind of work is
//! called is interned by [`WorkKinds`] from the word a panel shows,
//! through [`AddWork::add_work`]. [`WorkPlugin`] is opt-in.

use bevy::prelude::*;
use rl_core::Interner;
use rl_core::turn::BASE_ACTION_COST;
use rl_rules::work::{Progress, Work, WorkKind, WorkKindId, in_reach};

use crate::combat::{DamageDealt, DeathEvent};
use crate::components::{MyTurn, Player, Position};
use crate::places::{MapId, OnMap};
use crate::plugin::{DecideSet, Reads, ResolveSet, Turn, TurnSet};
use crate::turn::{Acting, Action, AddAction, Intent, Resolution};

/// Every kind of work in play, by the word a panel shows for it.
///
/// Interned rather than free strings so a game compares a [`WorkDone`]'s
/// kind by id, and a save stores the word, so the order kinds were
/// declared in never reaches a save.
#[derive(Resource, Debug, Clone, Default)]
pub struct WorkKinds(Interner<WorkKind>);

impl WorkKinds {
    /// The id for `name`, assigning a new one if it is unseen.
    pub fn declare(&mut self, name: &str) -> WorkKindId {
        self.0.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<WorkKindId> {
        self.0.get(name)
    }

    /// The word behind `id`.
    pub fn name(&self, id: WorkKindId) -> &str {
        self.0.name(id)
    }
}

/// Declares a kind of work while the app is being built.
pub trait AddWork {
    /// Declares the kind of work called `name`, the word a panel shows
    /// while an actor does it. Look its id up with [`WorkKinds::get`].
    fn add_work(&mut self, name: &str) -> &mut Self;
}

impl AddWork for App {
    fn add_work(&mut self, name: &str) -> &mut Self {
        self.init_resource::<WorkKinds>();
        self.world_mut().resource_mut::<WorkKinds>().declare(name);
        self
    }
}

/// The work an actor is in the middle of.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Working(pub Work<Entity>);

/// Begin work: what a mind's [`Decision::Work`](rl_rules::ai::Decision::Work)
/// becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginWork(pub Work<Entity>);
impl Action for BeginWork {}

/// Spend a turn on the work in hand. Written by [`continue_work`] and
/// never by a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toil;
impl Action for Toil {}

/// Work was begun, this pass.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkBegan {
    /// Who.
    pub actor: Entity,
    /// What kind.
    pub kind: WorkKindId,
    /// On what, if anything.
    pub target: Option<Entity>,
}

/// Work was finished, this pass. What finishing means is the game's.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkDone {
    /// Who.
    pub actor: Entity,
    /// What kind.
    pub kind: WorkKindId,
    /// On what, if anything.
    pub target: Option<Entity>,
}

/// Why work was broken off.
///
/// Closed, because each is a thing the engine itself detects; a rule of
/// a game's own stops work with [`Works::stop`] and reads as `Stopped`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakReason {
    /// The worker was harmed: any damage that landed, even a hit healed in
    /// the same pass, since the hit is what breaks concentration.
    Hurt,
    /// The worker died. A killing blow is harm too, and reads as this.
    Died,
    /// The target is gone, or it and the worker are further apart than
    /// [`REACH`](rl_rules::work::REACH).
    OutOfReach,
    /// Another worker finished work on the same target.
    DoneByAnother,
    /// The game stopped it.
    Stopped,
}

/// Work was broken off, this pass, and everything done on it is lost.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkBroken {
    /// Who.
    pub actor: Entity,
    /// What kind.
    pub kind: WorkKindId,
    /// On what, if anything.
    pub target: Option<Entity>,
    /// Turns it had done, for a game that wants a half-done job remembered.
    pub done: u16,
    /// Why.
    pub reason: BreakReason,
}

/// Begins and stops work, for anything that is not a mind's decision.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Works<'w, 's> {
    commands: Commands<'w, 's>,
    began: MessageWriter<'w, WorkBegan>,
    done: MessageWriter<'w, WorkDone>,
    broken: MessageWriter<'w, WorkBroken>,
    working: Query<'w, 's, &'static Working>,
}

impl Works<'_, '_> {
    /// Begins `work` for `actor`, counting this turn as its first, so the
    /// caller is spending a turn on it. Work of one turn is done at once
    /// and leaves nothing working.
    pub fn begin(&mut self, actor: Entity, mut work: Work<Entity>) {
        self.began.write(WorkBegan { actor, kind: work.kind, target: work.target });
        match work.advance() {
            Progress::Finished => {
                self.done.write(WorkDone { actor, kind: work.kind, target: work.target });
            }
            Progress::Left(_) => {
                self.commands.entity(actor).insert(Working(work));
            }
        }
    }

    /// Stops `actor`'s work, if it has any, as a rule of the game's own.
    pub fn stop(&mut self, actor: Entity) {
        let Ok(work) = self.working.get(actor) else { return };
        self.commands.entity(actor).remove::<Working>();
        self.broken.write(WorkBroken { actor, kind: work.0.kind, target: work.0.target, done: work.0.done, reason: BreakReason::Stopped });
    }
}

/// Breaks off work for every reason the engine detects, once per worker,
/// the gravest reason first.
///
/// In `TurnSet::React`, which the turn loop runs after the whole resolve
/// chain: after damage has landed, and after `RemainsPlugin` has kept a
/// twin of anyone who died, so a worker stood back up comes back at its
/// work. Harm and death are read from the one place each lands, not
/// watched for; reach is asked of where things are now, so every way of
/// moving a worker or its target is covered by one question, and it is
/// asked every pass so a panel never shows work that can no longer be done.
pub fn break_work(
    mut commands: Commands,
    mut hurt: MessageReader<DamageDealt>,
    mut deaths: MessageReader<DeathEvent>,
    mut finished: MessageReader<WorkDone>,
    mut broken: MessageWriter<WorkBroken>,
    working: Query<(Entity, &Working, &Position, Option<&OnMap>)>,
    places: Query<(&Position, Option<&OnMap>)>,
) {
    fn note(reasons: &mut Vec<(Entity, BreakReason)>, actor: Entity, reason: BreakReason) {
        if !reasons.iter().any(|(a, _)| *a == actor) {
            reasons.push((actor, reason));
        }
    }
    let map_of = |on: Option<&OnMap>| on.map(|m| m.0).unwrap_or(MapId::SURFACE);
    let mut reasons: Vec<(Entity, BreakReason)> = Vec::new();
    for death in deaths.read() {
        note(&mut reasons, death.entity, BreakReason::Died);
    }
    for harm in hurt.read().filter(|h| h.dealt > 0) {
        note(&mut reasons, harm.target, BreakReason::Hurt);
    }
    let done: Vec<(Entity, Option<Entity>)> = finished.read().map(|d| (d.actor, d.target)).collect();
    for (actor, work, pos, on) in &working {
        let Some(target) = work.0.target else { continue };
        if done.iter().any(|(finisher, t)| *finisher != actor && *t == Some(target)) {
            note(&mut reasons, actor, BreakReason::DoneByAnother);
            continue;
        }
        let reachable = places.get(target).is_ok_and(|(there, target_on)| map_of(target_on) == map_of(on) && in_reach(pos.0, there.0));
        if !reachable {
            note(&mut reasons, actor, BreakReason::OutOfReach);
        }
    }
    for (actor, reason) in reasons {
        let Ok((_, work, _, _)) = working.get(actor) else { continue };
        commands.entity(actor).remove::<Working>();
        broken.write(WorkBroken { actor, kind: work.0.kind, target: work.0.target, done: work.0.done, reason });
    }
}

/// A non-player holding the turn with work in hand.
type AtWork<'w, 's> = Query<'w, 's, Entity, (With<Working>, With<MyTurn>, Without<Player>)>;

/// Carries on the work of whoever holds the turn, before any mind is
/// asked: claiming the decision is what keeps the perceive stage shut.
///
/// Not the player's: the player's own long actions wait for a slice of
/// their own, with a key to stop them.
pub fn continue_work(mut acting: ResMut<Acting>, mut toil: MessageWriter<Intent<Toil>>, working: AtWork) {
    for actor in &working {
        if acting.claim_decision(actor) {
            toil.write(Intent::new(actor, Toil));
        }
    }
}

/// Begins the work a mind decided on, spending the turn as a wait does.
pub fn resolve_begins(mut intents: MessageReader<Intent<BeginWork>>, mut resolution: Resolution, mut works: Works) {
    for intent in intents.read() {
        if resolution.claim(intent.actor) {
            works.begin(intent.actor, intent.action.0);
            resolution.done(intent.actor, BASE_ACTION_COST);
        }
    }
}

/// Spends a turn on the work in hand, as a wait costs, and finishes it on
/// its last.
pub fn resolve_toil(
    mut commands: Commands,
    mut intents: MessageReader<Intent<Toil>>,
    mut resolution: Resolution,
    mut working: Query<&mut Working>,
    mut done: MessageWriter<WorkDone>,
) {
    for intent in intents.read() {
        let Ok(mut work) = working.get_mut(intent.actor) else { continue };
        if !resolution.claim(intent.actor) {
            continue;
        }
        if work.0.advance() == Progress::Finished {
            commands.entity(intent.actor).remove::<Working>();
            done.write(WorkDone { actor: intent.actor, kind: work.0.kind, target: work.0.target });
        }
        resolution.done(intent.actor, BASE_ACTION_COST);
    }
}

/// Work: an actor doing one thing across many turns.
///
/// Opt-in. A mind that decides to work in a game without it is refused
/// by the sweeper, the way a blow is in a game without combat.
pub struct WorkPlugin;

impl Plugin for WorkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorkKinds>()
            .add_message::<WorkBegan>()
            .add_message::<WorkDone>()
            .add_message::<WorkBroken>()
            // Harm and death are combat's; without combat nothing is ever
            // hurt, and an empty queue says so.
            .reads::<DamageDealt>()
            .reads::<DeathEvent>()
            .add_systems(Turn, break_work.in_set(TurnSet::React))
            .add_action::<BeginWork>()
            .add_action::<Toil>()
            .add_systems(Turn, continue_work.in_set(DecideSet::Sense))
            .add_systems(Turn, (resolve_begins, resolve_toil).chain().in_set(ResolveSet::Act));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Health;
    use crate::components::{Actor, Blocks, Player, Position, Speed, Viewshed};
    use crate::minds::{Mind, MindsPlugin, Perception, Thinking};
    use crate::plugin::{PerceiveSet, Turn, TurnSet, headless_app};
    use crate::state::EngineState;
    use crate::turn::{Intent, Turns, Wait};
    use rl_core::Point;
    use rl_rules::ai::{Brain, Decision, Tactic, TacticCtx};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A world with work in it and every plugin a test here needs, all
    /// added before the app first runs; a player to hold the turns; and
    /// the word the work is called by.
    fn arena() -> (App, Point, Entity, WorkKindId) {
        let mut app = headless_app();
        app.add_plugins((
            crate::fov::FovPlugin,
            crate::world::StreamingPlugin,
            crate::combat::CombatPlugin,
            MindsPlugin,
            crate::remains::RemainsPlugin,
            WorkPlugin,
        ));
        app.add_work("mending");
        let kind = app.world().resource::<WorkKinds>().get("mending").expect("declared");
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        app.insert_resource(TheSides(sides));
        let player =
            app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), crate::combat::Faction(sides.ours))).id();
        app.init_resource::<Finished>().add_systems(Turn, record_finished.in_set(TurnSet::Cleanup));
        app.init_resource::<Broken>().add_systems(Turn, record_broken.in_set(TurnSet::Cleanup));
        app.init_resource::<Opened>().add_systems(Turn, count_openings.in_set(PerceiveSet::Annotate));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        (app, start, player, kind)
    }

    /// Every `WorkDone`, with the clock it was written at.
    #[derive(Resource, Default)]
    struct Finished(Vec<(WorkDone, u32)>);

    fn record_finished(mut done: MessageReader<WorkDone>, turns: Res<Turns>, mut seen: ResMut<Finished>) {
        for d in done.read() {
            seen.0.push((*d, turns.now()));
        }
    }

    /// One player turn: a wait, if the player holds the turn, and a frame.
    fn pass(app: &mut App, player: Entity) {
        if app.world().get::<crate::components::MyTurn>(player).is_some() {
            app.world_mut().write_message(Intent::new(player, Wait));
        }
        app.update();
    }

    #[test]
    fn work_ends_after_exactly_the_turns_it_needs_and_the_clock_it_took_scales_with_speed() {
        for speed in [50, 100, 200] {
            for needed in 1..=6u16 {
                let (mut app, start, player, kind) = arena();
                let worker = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Speed(speed), Health::full(5))).id();
                app.update();
                let began = app.world().resource::<Turns>().now();
                app.world_mut().entity_mut(worker).insert(Working(Work::new(kind, needed)));
                for _ in 0..(needed as usize * 4 + 8) {
                    pass(&mut app, player);
                }
                let finished = &app.world().resource::<Finished>().0;
                assert_eq!(finished.len(), 1, "one finish, speed {speed}, needed {needed}");
                let each = rl_core::turn::scaled_cost(rl_core::turn::BASE_ACTION_COST, speed);
                assert!(finished[0].1 - began <= needed as u32 * each, "{needed} turns at speed {speed} take no more than {needed} of its turns");
                assert!(finished[0].1 - began + each >= needed as u32 * each, "and no fewer");
                assert!(app.world().get::<Working>(worker).is_none(), "and nothing is left working");
            }
        }
    }

    /// Asks once and counts each asking, so a test sees whether the brain
    /// was consulted.
    struct Mend {
        kind: WorkKindId,
        needed: u16,
        asked: Arc<AtomicU32>,
    }

    impl Tactic<Entity> for Mend {
        fn name(&self) -> &'static str {
            "mend"
        }
        fn evaluate(&self, _: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
            self.asked.fetch_add(1, Ordering::Relaxed);
            Some(Decision::Work(Work::new(self.kind, self.needed)))
        }
    }

    #[derive(Resource, Default)]
    struct Opened(u32);

    /// Counts every time the perceive stage opens a snapshot.
    fn count_openings(mut thinking: ResMut<Thinking>, mut opened: ResMut<Opened>) {
        if thinking.snapshot_mut().is_some() {
            opened.0 += 1;
        }
    }

    #[test]
    fn a_mind_that_chose_work_is_not_asked_again_and_perceives_nothing_until_it_is_done() {
        let (mut app, start, player, kind) = arena();
        let asked = Arc::new(AtomicU32::new(0));
        let brain = Arc::new(Brain::new().then(Mend { kind, needed: 5, asked: asked.clone() }));
        app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(5), Perception(6), Mind(brain)));
        for _ in 0..20 {
            if !app.world().resource::<Finished>().0.is_empty() {
                break;
            }
            pass(&mut app, player);
        }
        assert_eq!(app.world().resource::<Finished>().0.len(), 1, "the work was finished");
        assert_eq!(asked.load(Ordering::Relaxed), 1, "and the brain was asked once, on the turn it chose the work");
        assert_eq!(app.world().resource::<Opened>().0, 1, "nor did it perceive anything while it worked");
        for _ in 0..3 {
            pass(&mut app, player);
        }
        assert!(asked.load(Ordering::Relaxed) >= 2, "done, it is asked again");
    }

    #[test]
    fn work_of_one_turn_is_done_on_the_turn_it_is_begun_and_leaves_nothing_working() {
        let (mut app, start, player, kind) = arena();
        let asked = Arc::new(AtomicU32::new(0));
        let brain = Arc::new(Brain::new().then(Mend { kind, needed: 1, asked }));
        let mender = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(5), Perception(6), Mind(brain))).id();
        pass(&mut app, player);
        pass(&mut app, player);
        assert!(!app.world().resource::<Finished>().0.is_empty(), "done at once");
        assert!(app.world().get::<Working>(mender).is_none(), "with no work left hanging");
    }

    /// The sides `arena` made, for a test that needs a faction or a damage
    /// kind.
    #[derive(Resource, Clone, Copy)]
    struct TheSides(crate::testing::Sides);

    /// Every `WorkBroken`, in order.
    #[derive(Resource, Default)]
    struct Broken(Vec<WorkBroken>);

    fn record_broken(mut broken: MessageReader<WorkBroken>, mut seen: ResMut<Broken>) {
        seen.0.extend(broken.read().copied());
    }

    /// A worker three cells from the player, working on a thing beside it,
    /// and the kind of damage to hurt it with.
    fn at_work() -> (App, Entity, Entity, Entity, rl_rules::damage::DamageKindId) {
        let (mut app, start, player, kind) = arena();
        let sides = app.world().resource::<TheSides>().0;
        let thing = app.world_mut().spawn(Position(start.offset(4, 0))).id();
        let worker = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(10), crate::combat::Faction(sides.theirs))).id();
        app.world_mut().entity_mut(worker).insert(Working(Work::new(kind, 50).on(thing)));
        app.update();
        (app, player, worker, thing, sides.kind)
    }

    fn reasons(app: &App) -> Vec<BreakReason> {
        app.world().resource::<Broken>().0.iter().map(|b| b.reason).collect()
    }

    #[test]
    fn a_worker_that_is_hurt_breaks_off_even_when_healed_in_the_same_pass() {
        let (mut app, player, worker, _, damage) = at_work();
        app.world_mut().write_message(crate::combat::DamageEvent::new(worker, rl_rules::Hit::from_source(None, damage, 2)));
        app.world_mut().write_message(crate::combat::DamageEvent::new(worker, rl_rules::Hit::from_source(None, damage, -2)));
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::Hurt]);
        assert!(app.world().get::<Working>(worker).is_none());
    }

    #[test]
    fn a_worker_killed_at_work_breaks_off_as_dead_not_hurt() {
        let (mut app, player, worker, _, damage) = at_work();
        app.world_mut().write_message(crate::combat::DamageEvent::new(worker, rl_rules::Hit::from_source(None, damage, 99)));
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::Died]);
    }

    #[test]
    fn a_worker_moved_out_of_reach_breaks_off_and_one_moved_within_reach_does_not() {
        let (mut app, player, worker, thing, _) = at_work();
        let there = app.world().get::<Position>(thing).unwrap().0;
        // Swapped to the other side of it, still beside it.
        app.world_mut().get_mut::<Position>(worker).unwrap().0 = there.offset(1, 1);
        pass(&mut app, player);
        assert!(reasons(&app).is_empty(), "still within reach");
        app.world_mut().get_mut::<Position>(worker).unwrap().0 = there.offset(3, 0);
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::OutOfReach], "shoved off");
    }

    #[test]
    fn work_on_a_target_carried_off_or_gone_breaks_off() {
        let (mut app, player, _, thing, _) = at_work();
        app.world_mut().get_mut::<Position>(thing).unwrap().0.x += 3;
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::OutOfReach], "carried off");

        let (mut app, player, _, thing, _) = at_work();
        app.world_mut().despawn(thing);
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::OutOfReach], "gone");
    }

    #[test]
    fn when_one_worker_finishes_a_target_every_other_on_it_breaks_off() {
        let (mut app, player, slow, thing, _) = at_work();
        let kind = app.world().resource::<WorkKinds>().get("mending").unwrap();
        let at = app.world().get::<Position>(thing).unwrap().0;
        let quick = app.world_mut().spawn((Actor, Blocks, Position(at.offset(0, 1)), Health::full(10))).id();
        app.world_mut().entity_mut(quick).insert(Working(Work::new(kind, 2).on(thing)));
        for _ in 0..4 {
            pass(&mut app, player);
        }
        let broken = &app.world().resource::<Broken>().0;
        assert_eq!(broken.len(), 1);
        assert_eq!((broken[0].actor, broken[0].reason), (slow, BreakReason::DoneByAnother));
    }

    #[derive(Resource)]
    struct StopNow(Entity);

    fn stop_it(stop: Option<Res<StopNow>>, mut works: Works) {
        if let Some(stop) = stop {
            works.stop(stop.0);
        }
    }

    #[test]
    fn a_game_can_stop_work_and_says_so() {
        let (mut app, player, worker, _, _) = at_work();
        app.add_systems(Turn, stop_it.in_set(TurnSet::React));
        app.insert_resource(StopNow(worker));
        pass(&mut app, player);
        assert_eq!(reasons(&app), vec![BreakReason::Stopped]);
        assert!(app.world().get::<Working>(worker).is_none());
    }

    /// The twin is taken before the work breaks, so an actor stood back up
    /// comes back doing what it was doing, and breaks off at once if what
    /// it was working on has gone.
    #[test]
    fn an_actor_revived_comes_back_at_the_work_it_died_doing() {
        let (mut app, player, worker, thing, damage) = at_work();
        app.world_mut().entity_mut(worker).insert(crate::remains::LeavesRemains);
        app.world_mut().write_message(crate::combat::DamageEvent::new(worker, rl_rules::Hit::from_source(None, damage, 99)));
        pass(&mut app, player);
        assert!(crate::remains::revive(app.world_mut(), worker, 5));
        assert!(app.world().get::<Working>(worker).is_some(), "back at its work");

        app.world_mut().despawn(thing);
        pass(&mut app, player);
        assert_eq!(reasons(&app).last(), Some(&BreakReason::OutOfReach), "which it drops once the thing is gone");
    }
}
