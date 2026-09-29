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
//! [`WorkBegan`] is written when work starts. What a kind of work is
//! called is interned by [`WorkKinds`] from the word a panel shows,
//! through [`AddWork::add_work`]. [`WorkPlugin`] is opt-in.

use bevy::prelude::*;
use rl_core::Interner;
use rl_core::turn::BASE_ACTION_COST;
use rl_rules::work::{Progress, Work, WorkKind, WorkKindId};

use crate::components::{MyTurn, Player};
use crate::plugin::{DecideSet, ResolveSet, Turn};
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

/// Begins work, for anything that is not a mind's decision.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Works<'w, 's> {
    commands: Commands<'w, 's>,
    began: MessageWriter<'w, WorkBegan>,
    done: MessageWriter<'w, WorkDone>,
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
        let player =
            app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), crate::combat::Faction(sides.ours))).id();
        app.init_resource::<Finished>().add_systems(Turn, record_finished.in_set(TurnSet::Cleanup));
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
}
