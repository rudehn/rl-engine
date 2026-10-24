//! Work: an actor doing one thing across many turns.
//!
//! A mind begins work by deciding [`Decision::Work`](atglyph_rules::ai::Decision::Work),
//! which becomes a [`BeginWork`] intent; anything else begins it through
//! [`Works::begin`]. Either way the turn it is begun on is its first, and
//! the actor carries [`Working`] until it is done. On every turn after,
//! `continue_work` claims the actor's decision before any mind is asked
//! and writes a `Toil`, so a busy actor never thinks, and
//! `resolve_toil` spends the turn as a wait costs and counts it. On the
//! last it writes [`WorkDone`], which is where a game says what finishing
//! means; `resolve_begins` is what turns a mind's decision into work.
//! [`WorkBegan`] is written when work starts. `break_work` breaks it off
//! with a [`WorkBroken`] and its [`BreakReason`] when the worker is hurt or
//! dies, when its target is gone or out of reach, or when another worker
//! finishes the same target; a game stops it with [`Works::stop`], or by
//! writing a [`StopWork`], which `answer_stops` reads at the head of
//! every pass. What a kind of work is called is interned by [`WorkKinds`]
//! from the word a panel shows, through [`EngineApp::add_work`].
//!
//! The player works by the same [`Working`], begun by [`Works::begin`]
//! from whatever the player did, and `work_for_player` spends its turns
//! for it, at most one each [`WorkPace`], as `pace_player_work` allows,
//! so they pass without a key and slowly enough to be watched. Spending
//! one of them on anything else leaves the work off, as
//! [`BreakReason::LeftOff`]; so does a key, which is `atglyph-ui`'s to read
//! and which it answers with [`Works::leave_off`], all but a direction
//! key toward the work's own target, which presses on with it. A foe
//! coming into the player's sight breaks it as [`BreakReason::FoeSighted`]
//! before another turn of it is spent, and `PlayerAtWork` keeps who was
//! already in sight.
//!
//! Part of [`CorePlugin`](crate::plugin::CorePlugin): what work a game has
//! is the kinds it declares, and a game that declares none has no work to
//! do, so there is nothing to turn on.

use std::time::Duration;

use atglyph_core::Interner;
use atglyph_core::turn::BASE_ACTION_COST;
use atglyph_rules::Relation;
use atglyph_rules::faction::stance;
pub use atglyph_rules::work::BreakReason;
use atglyph_rules::work::{Progress, Work, WorkKind, WorkKindId, break_reason, in_reach, newly_sighted};
use bevy::prelude::*;

use crate::app::EngineApp;
use crate::combat::{CombatRules, DamageDealt, Dead, DeathEvent, Faction};
use crate::components::{Actor, MyTurn, Player, Position};
use crate::lighting::{Concealed, PlayerSight};
use crate::places::{MapId, OnMap};
use crate::plugin::{DecideSet, EngineSet, ResolveSet, Turn, TurnSet};
use crate::stealth::Unseen;
use crate::turn::{Acting, Action, ActionDone, Intent, Resolution};

/// Every kind of work in play, by the word a panel shows for it.
///
/// Interned rather than free strings so a game compares a [`WorkDone`]'s
/// kind by id, and a save stores the word, so the order kinds were
/// declared in never reaches a save.
#[derive(Resource, Debug, Clone, Default)]
pub struct WorkKinds(Interner<WorkKind>);

impl WorkKinds {
    /// The id for `name`, assigning a new one if it is unseen.
    pub(crate) fn declare(&mut self, name: &str) -> WorkKindId {
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

    /// Whether no kind has been declared, so nothing in the game can work.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The body of [`EngineApp::add_work`](crate::app::EngineApp::add_work).
pub(crate) fn add_work<'a>(app: &'a mut App, name: &str) -> &'a mut App {
    app.init_resource::<WorkKinds>();
    app.world_mut().resource_mut::<WorkKinds>().declare(name);
    app
}

/// The work an actor is in the middle of.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Working(pub Work<Entity>);

/// Begin work: what a mind's [`Decision::Work`](atglyph_rules::ai::Decision::Work)
/// becomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BeginWork(pub Work<Entity>);
impl Action for BeginWork {}

/// Spend a turn on the work in hand. Written by [`continue_work`] and
/// never by a game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Toil;
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

    /// Stops `actor`'s work, if it has any, as a rule of the game's own,
    /// which reads as [`BreakReason::Stopped`].
    ///
    /// At once, so a stop written before the turns run this frame is
    /// answered before another turn of the work is spent. A [`StopWork`]
    /// says the same from anywhere a message can be written. Not
    /// [`Works::leave_off`], which is the worker's own choice: a reader of
    /// [`WorkBroken`] tells a game's interruption from the player walking
    /// away by the reason alone.
    pub fn stop(&mut self, actor: Entity) {
        self.break_off(actor, BreakReason::Stopped);
    }

    /// `actor` leaves its work off of its own accord, if it has any: what
    /// a key pressed while the player works comes to, and read as
    /// [`BreakReason::LeftOff`] rather than a rule of the game's.
    pub fn leave_off(&mut self, actor: Entity) {
        self.break_off(actor, BreakReason::LeftOff);
    }

    fn break_off(&mut self, actor: Entity, reason: BreakReason) {
        let Ok(work) = self.working.get(actor) else { return };
        self.commands.entity(actor).remove::<Working>();
        self.broken.write(WorkBroken { actor, kind: work.0.kind, target: work.0.target, done: work.0.done, reason });
    }
}

/// Breaks off `actor`'s work, if it has any, as [`BreakReason::Stopped`]:
/// a rule of the game's own, written as a message.
///
/// The hook for anything a game adds that should break work off, such as
/// a noise heard or a shout from a friend, from any system that can write
/// a message. The work system reads it at the head of every pass, before
/// any turn of work is spent in it, so a stop written in one pass's
/// `TurnSet::React` or in a frame's drawing is answered before the next
/// turn of the work. [`Works::stop`] is the same, at once.
///
/// The message rather than [`Works`] for code that holds the `World`
/// rather than a system's parameters, such as a function a key or a use
/// calls, where building the `SystemParam` would be the whole of the work.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopWork(pub Entity);

/// Breaks off the work of everyone a [`StopWork`] names.
///
/// At the head of [`DecideSet::Sense`], before [`continue_work`] claims a
/// mind's turn for it and long before [`work_for_player`] spends the
/// player's, so a stop is never a turn late.
pub(crate) fn answer_stops(mut stops: MessageReader<StopWork>, mut works: Works) {
    // Once each: the work comes off when the commands run, so a second
    // stop for the same worker in one read would find it still there.
    let mut stopped: Vec<Entity> = Vec::new();
    for StopWork(actor) in stops.read() {
        if !stopped.contains(actor) {
            stopped.push(*actor);
            works.stop(*actor);
        }
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
pub(crate) fn break_work(
    mut commands: Commands,
    mut hurt: MessageReader<DamageDealt>,
    mut deaths: MessageReader<DeathEvent>,
    mut finished: MessageReader<WorkDone>,
    mut broken: MessageWriter<WorkBroken>,
    working: Query<(Entity, &Working, &Position, Option<&OnMap>)>,
    places: Query<(&Position, Option<&OnMap>)>,
) {
    let map_of = |on: Option<&OnMap>| on.map(|m| m.0).unwrap_or(MapId::ZERO);
    let died: Vec<Entity> = deaths.read().map(|d| d.entity).collect();
    let harmed: Vec<Entity> = hurt.read().filter(|h| h.dealt > 0).map(|h| h.target).collect();
    let done: Vec<(Entity, Option<Entity>)> = finished.read().map(|d| (d.actor, d.target)).collect();
    // Broken in the order death, harm, then the workers' own, so the
    // messages read the same as they always have.
    let mut actors: Vec<Entity> = Vec::new();
    for actor in died.iter().chain(&harmed).copied().chain(working.iter().map(|w| w.0)) {
        if !actors.contains(&actor) {
            actors.push(actor);
        }
    }
    let mut reasons: Vec<(Entity, BreakReason)> = Vec::new();
    for actor in actors {
        let Ok((_, work, pos, on)) = working.get(actor) else { continue };
        let (done_by_another, reachable) = match work.0.target {
            None => (false, true),
            Some(target) => (
                done.iter().any(|(finisher, t)| *finisher != actor && *t == Some(target)),
                places.get(target).is_ok_and(|(there, target_on)| map_of(target_on) == map_of(on) && in_reach(pos.0, there.0)),
            ),
        };
        if let Some(reason) = break_reason(died.contains(&actor), harmed.contains(&actor), done_by_another, reachable) {
            reasons.push((actor, reason));
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
/// Not the player's: [`work_for_player`] spends those, later in the pass,
/// so that whatever the player asked for this frame is resolved first.
pub(crate) fn continue_work(mut acting: ResMut<Acting>, mut toil: MessageWriter<Intent<Toil>>, working: AtWork) {
    for actor in &working {
        if acting.claim_decision(actor) {
            toil.write(Intent::new(actor, Toil));
        }
    }
}

/// The least wall-clock time between two turns of the player's work.
///
/// A held status spends its holder's turns as fast as the loop runs, and
/// that is right for turns nobody chose. Work is chosen, can be stopped by
/// a key, and is broken off by what happens while it goes on, so its
/// turns are paced to be watched: the world is seen to move around the
/// player, and a key pressed to stop it lands before the work is over.
/// The pace decides only when a turn is spent, never what it does, so a
/// replay keyed by the turn clock plays the same at any pace.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkPace(pub Duration);

impl WorkPace {
    /// One turn of the work each `every` of the wall clock, at most.
    pub const fn every(every: Duration) -> Self {
        Self(every)
    }

    /// One turn of the work a frame: what a test, or a run nobody watches,
    /// wants.
    pub const fn every_frame() -> Self {
        Self(Duration::ZERO)
    }
}

impl Default for WorkPace {
    /// Twenty turns a second: ten turns of work are half a second to
    /// watch, and a key lands well inside them.
    fn default() -> Self {
        Self::every(Duration::from_millis(50))
    }
}

/// The engine's hand on the player's work: whether a turn of it is due,
/// and which foes were already in sight.
///
/// Kept by [`pace_player_work`] and [`work_for_player`]; never saved,
/// since every part of it is put back from the world. A work that was
/// begun this session starts with whoever was in sight when it was begun;
/// one that was not, a run loaded mid-work, starts with nobody, so a foe
/// in sight on its first turn breaks it, since nobody can say whether
/// that foe came into sight before the save or after the work began.
#[derive(Resource, Debug, Default)]
pub(crate) struct PlayerAtWork {
    /// The player whose work the rest is about.
    worker: Option<Entity>,
    /// Wall-clock time waited since the last turn of it.
    waited: Duration,
    /// Whether the pace has allowed the next turn.
    due: bool,
    /// The foes in sight at the last turn of it.
    seen: Vec<Entity>,
}

/// Lets the player's work have a turn once its [`WorkPace`] has passed.
///
/// Once a frame, before [`EngineSet::Input`], where a key would have asked
/// for the turn: a pass may run hundreds of times a frame, and a turn
/// allowed there would spend the whole work in one frame. Before the set
/// rather than in it, so a game's own input systems, an exclusive one
/// among them, need no order against it. With nobody at work it
/// forgets everything, so the next work begins from nothing.
pub(crate) fn pace_player_work(time: Res<Time>, pace: Res<WorkPace>, mut hand: ResMut<PlayerAtWork>, workers: Query<(), (With<Player>, With<Working>)>) {
    if workers.is_empty() {
        *hand = PlayerAtWork::default();
        return;
    }
    if !hand.due {
        hand.waited += time.delta();
        hand.due = hand.waited >= pace.0;
    }
}

/// Anyone who might stand in the player's sight.
type Bystanders<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>, Option<&'static Faction>, Has<Concealed>), (With<Actor>, Without<Dead>, Without<Unseen>)>;

/// What the player at work looks up to see: its own side, what it sees,
/// the sides' rules and whoever stands about.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Lookout<'w, 's> {
    sides: Query<'w, 's, Option<&'static Faction>, With<Player>>,
    sight: PlayerSight<'w, 's>,
    rules: Option<Res<'w, CombatRules>>,
    bystanders: Bystanders<'w, 's>,
}

impl Lookout<'_, '_> {
    /// Every foe `me` sees, in a fixed order, or `None` when `me` is not
    /// the player: an actor alive and not [`Unseen`], shown by
    /// [`PlayerSight`], so one concealed in the dark is no foe in sight, and
    /// at odds with the player either way round. With no rules, or with no
    /// side on either, anyone is at odds with anyone, as stealth reads it:
    /// a game with no sides has nobody to be friends with.
    fn foes(&self, me: Entity) -> Option<Vec<Entity>> {
        let mine = self.sides.get(me).ok()?;
        let rules = self.rules.as_deref();
        let mut foes: Vec<Entity> = self
            .bystanders
            .iter()
            .filter(|(other, at, on, theirs, concealed)| {
                let (factions, mine, theirs) = (rules.map(|r| &r.factions), mine.map(|f| f.0), theirs.map(|f| f.0));
                let odds =
                    stance(factions, mine, theirs).zip(stance(factions, theirs, mine)).is_none_or(|(x, y)| x == Relation::Hostile || y == Relation::Hostile);
                *other != me && odds && self.sight.shows(at.0, *on, *concealed)
            })
            .map(|(other, ..)| other)
            .collect();
        foes.sort();
        Some(foes)
    }
}

/// The player at work, and whether it holds the turn.
type PlayerWorking<'w, 's> = Query<'w, 's, (Entity, &'static mut Working, Has<MyTurn>), With<Player>>;

/// What [`work_for_player`] spends a turn with or breaks the work off by.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct PlayerToil<'w, 's> {
    commands: Commands<'w, 's>,
    acting: ResMut<'w, Acting>,
    spent: MessageWriter<'w, ActionDone>,
    done: MessageWriter<'w, WorkDone>,
    broken: MessageWriter<'w, WorkBroken>,
}

impl PlayerToil<'_, '_> {
    fn break_off(&mut self, actor: Entity, work: &Work<Entity>, reason: BreakReason) {
        self.commands.entity(actor).remove::<Working>();
        self.broken.write(WorkBroken { actor, kind: work.kind, target: work.target, done: work.done, reason });
    }
}

/// Spends the turn the player holds on its work, when nothing else has,
/// and breaks the work off when something else has or a foe has come into
/// sight.
///
/// After every resolver and before the sweep, so whatever the player asked
/// for this frame, a game's action or the engine's, has been answered
/// first: a turn the player spent on anything else, or an ask refused,
/// leaves the work off as [`BreakReason::LeftOff`] and the player's own
/// act stands, so no key the player meant is swallowed by the work. A
/// turn something claimed the decision for, as a status that holds does,
/// is lost to the work as it is to a mind's, and leaves it on.
///
/// Otherwise the player looks up before working: a foe in sight that was
/// not in sight at the last turn of it breaks the work as
/// [`BreakReason::FoeSighted`], and the player keeps the turn, unspent, to
/// answer the foe. Then, when [`pace_player_work`] has let a turn be due,
/// it is spent as a wait costs and counted, and on its last
/// [`WorkDone`] is written, as [`resolve_toil`] does for a mind; until
/// then the player holds the turn and the frame ends, so the world is
/// drawn between every two turns of the work.
pub(crate) fn work_for_player(
    mut hand: ResMut<PlayerAtWork>,
    mut began: MessageReader<WorkBegan>,
    mut hurt: MessageReader<DamageDealt>,
    mut toil: PlayerToil,
    mut workers: PlayerWorking,
    lookout: Lookout,
) {
    let mut fresh: Vec<Entity> = Vec::new();
    for b in began.read() {
        // Read off the player rather than its work, which the commands of
        // whatever began it may not have put on yet.
        if let Some(foes) = lookout.foes(b.actor) {
            // Begun with whoever was in sight then, which the player chose
            // to begin it in front of.
            hand.worker = Some(b.actor);
            hand.seen = foes;
            fresh.push(b.actor);
        }
    }
    let harmed: Vec<Entity> = hurt.read().filter(|h| h.dealt > 0).map(|h| h.target).collect();
    for (me, mut work, holds) in &mut workers {
        if hand.worker != Some(me) {
            // Work this session never saw begun: a run loaded mid-work.
            hand.worker = Some(me);
            hand.seen.clear();
        }
        // The turn it was begun on is its first, spent by the beginning.
        if !holds || fresh.contains(&me) || toil.acting.has_decided(me) {
            continue;
        }
        // Hurt in this very pass, by a trap or a fire on the player's own
        // turn: no turn of work, and [`break_work`] breaks it off as hurt
        // once the pass reaches `React`.
        if harmed.contains(&me) {
            continue;
        }
        if toil.acting.has_acted(me) || toil.acting.was_refused(me) {
            toil.break_off(me, &work.0, BreakReason::LeftOff);
            continue;
        }
        let now = lookout.foes(me).unwrap_or_default();
        let sighted = newly_sighted(&hand.seen, &now);
        hand.seen = now;
        if sighted {
            toil.break_off(me, &work.0, BreakReason::FoeSighted);
            continue;
        }
        if !hand.due {
            continue;
        }
        hand.due = false;
        hand.waited = Duration::ZERO;
        toil.acting.claim_decision(me);
        toil.acting.claim_action(me);
        if work.0.advance() == Progress::Finished {
            toil.commands.entity(me).remove::<Working>();
            toil.done.write(WorkDone { actor: me, kind: work.0.kind, target: work.0.target });
        }
        toil.spent.write(ActionDone { actor: me, cost: BASE_ACTION_COST });
    }
}

/// Begins the work a mind decided on, spending the turn as a wait does.
pub(crate) fn resolve_begins(mut intents: MessageReader<Intent<BeginWork>>, mut resolution: Resolution, mut works: Works) {
    for intent in intents.read() {
        if resolution.claim(intent.actor) {
            works.begin(intent.actor, intent.action.0);
            resolution.done(intent.actor, BASE_ACTION_COST);
        }
    }
}

/// Spends a turn on the work in hand, as a wait costs, and finishes it on
/// its last.
pub(crate) fn resolve_toil(
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

/// Work: an actor doing one thing across many turns, the player's own
/// long actions among them, as [`CorePlugin`](crate::plugin::CorePlugin)
/// registers it.
pub(crate) fn build(app: &mut App) {
    app.init_resource::<WorkKinds>()
        .init_resource::<WorkPace>()
        .init_resource::<PlayerAtWork>()
        .reset_on_new_run::<PlayerAtWork>()
        .add_message::<WorkBegan>()
        .add_message::<WorkDone>()
        .add_message::<WorkBroken>()
        .add_message::<StopWork>()
        // Harm and death are combat's; without combat nothing is ever
        // hurt, and an empty queue says so.
        .reads::<DamageDealt>()
        .reads::<DeathEvent>()
        .add_systems(Turn, break_work.in_set(crate::plugin::ReactSet::Settle))
        .add_action::<BeginWork>()
        .add_action::<Toil>()
        .add_systems(Turn, (answer_stops, continue_work).chain().in_set(DecideSet::Sense))
        .add_systems(Turn, (resolve_begins, resolve_toil).chain().in_set(ResolveSet::Act))
        // Between the resolvers and the sweep rather than in a set of
        // its own: after everything the player could have asked for,
        // and before anything that would refuse it.
        .add_systems(Turn, work_for_player.after(TurnSet::Resolve).before(TurnSet::Sweep))
        .add_systems(Update, pace_player_work.before(EngineSet::Input));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Health;
    use crate::components::{Actor, Blocks, Player, Position, Speed, Viewshed};
    use crate::minds::{Mind, MindsPlugin, Perception, Thinking};
    use crate::plugin::{PerceiveSet, Turn, TurnSet};
    use crate::state::EngineState;
    use crate::testing::headless_app;
    use crate::turn::{Intent, Turns, Wait};
    use atglyph_core::Point;
    use atglyph_rules::ai::{Brain, Decision, Tactic, TacticCtx};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A world with work in it and every plugin a test here needs, all
    /// added before the app first runs; a player to hold the turns; and
    /// the word the work is called by.
    fn arena() -> (App, Point, Entity, WorkKindId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin, MindsPlugin, crate::remains::RemainsPlugin));
        app.add_work("mending");
        let kind = app.world().resource::<WorkKinds>().get("mending").expect("declared");
        let start = crate::testing::ground(&mut app);
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
                let each = atglyph_core::turn::scaled_cost(atglyph_core::turn::BASE_ACTION_COST, speed);
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
    struct TheSides(crate::testing::TwoSides);

    /// Every `WorkBroken`, in order.
    #[derive(Resource, Default)]
    struct Broken(Vec<WorkBroken>);

    fn record_broken(mut broken: MessageReader<WorkBroken>, mut seen: ResMut<Broken>) {
        seen.0.extend(broken.read().copied());
    }

    /// A worker three cells from the player, working on a thing beside it,
    /// and the kind of damage to hurt it with.
    fn at_work() -> (App, Entity, Entity, Entity, atglyph_rules::damage::DamageKindId) {
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
        app.world_mut().write_message(crate::combat::DamageEvent::new(worker, atglyph_rules::Hit::from_source(None, damage, 2)));
        app.world_mut().write_message(crate::combat::Heal::new(worker, 2));
        pass(&mut app, player);
        assert_eq!(app.world().get::<Health>(worker).map(|h| h.current), Some(10), "mended back to whole in the pass it was hurt in");
        assert_eq!(reasons(&app), vec![BreakReason::Hurt]);
        assert!(app.world().get::<Working>(worker).is_none());
    }

    #[test]
    fn a_worker_killed_at_work_breaks_off_as_dead_not_hurt() {
        let (mut app, player, worker, _, damage) = at_work();
        app.world_mut().write_message(crate::combat::DamageEvent::new(worker, atglyph_rules::Hit::from_source(None, damage, 99)));
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
        app.world_mut().write_message(crate::combat::DamageEvent::new(worker, atglyph_rules::Hit::from_source(None, damage, 99)));
        pass(&mut app, player);
        assert!(crate::remains::revive(app.world_mut(), worker, 5));
        assert!(app.world().get::<Working>(worker).is_some(), "back at its work");

        app.world_mut().despawn(thing);
        pass(&mut app, player);
        assert_eq!(reasons(&app).last(), Some(&BreakReason::OutOfReach), "which it drops once the thing is gone");
    }

    /// The player at work on `needed` turns of mending, begun as a game
    /// begins it, by an act that spends the player's turn; a turn a frame;
    /// and the clock when it was begun.
    fn player_at_work(needed: u16) -> (App, Point, Entity, u32) {
        let (mut app, start, player, kind) = arena();
        app.insert_resource(WorkPace::every_frame());
        let began = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, BeginWork(Work::new(kind, needed))));
        app.update();
        assert!(app.world().get::<Working>(player).is_some(), "the player took the work up");
        (app, start, player, began)
    }

    /// Turns the player's work has done.
    fn done(app: &App, player: Entity) -> Option<u16> {
        app.world().get::<Working>(player).map(|w| w.0.done)
    }

    /// Frames, with no key pressed and nothing written for the player.
    fn frames(app: &mut App, n: usize) {
        for _ in 0..n {
            app.update();
        }
    }

    #[test]
    fn the_players_work_spends_one_turn_a_frame_without_a_key_and_is_done_after_exactly_the_turns_it_needs() {
        for needed in 2..=6u16 {
            let (mut app, _, player, began) = player_at_work(needed);
            for turn in 2..needed {
                app.update();
                assert_eq!(done(&app, player), Some(turn), "{needed}: one turn a frame, never the whole work in one");
            }
            app.update();
            assert!(app.world().get::<Working>(player).is_none(), "{needed}: done, and nothing is left working");
            let finished = &app.world().resource::<Finished>().0;
            assert_eq!(finished.len(), 1, "{needed}: one finish");
            assert_eq!(finished[0].0.actor, player);
            assert_eq!(finished[0].1 - began, (needed as u32 - 1) * BASE_ACTION_COST, "{needed}: a wait's cost a turn, the first spent beginning it");
            frames(&mut app, 3);
            assert!(app.world().get::<MyTurn>(player).is_some(), "{needed}: and the player then holds its turn and waits for a key");
            assert_eq!(app.world().resource::<Turns>().now() - began, needed as u32 * BASE_ACTION_COST, "{needed}: with no turn spent for it");
        }
    }

    #[test]
    fn the_players_work_waits_out_its_pace_between_turns() {
        let (mut app, _, player, _) = player_at_work(20);
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(20)));
        app.insert_resource(WorkPace::every(std::time::Duration::from_millis(50)));
        // A frame for the manual clock to take over from the real one.
        app.update();
        let from = done(&app, player).unwrap();
        frames(&mut app, 9);
        assert_eq!(done(&app, player), Some(from + 3), "a turn each 50 ms at 20 ms a frame: three in nine frames, whatever the phase");
    }

    #[test]
    fn a_stop_written_while_the_player_works_breaks_it_off_before_another_turn_is_spent() {
        let (mut app, _, player, _) = player_at_work(10);
        frames(&mut app, 2);
        let clock = app.world().resource::<Turns>().now();
        app.world_mut().write_message(StopWork(player));
        frames(&mut app, 3);
        assert_eq!(reasons(&app), vec![BreakReason::Stopped]);
        assert_eq!(app.world().resource::<Broken>().0[0].done, 3, "the turns it had done, and none more");
        assert_eq!(app.world().resource::<Turns>().now(), clock, "nothing spent since");
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds its turn");
    }

    #[test]
    fn a_blow_that_lands_on_the_player_at_work_breaks_it_off_before_another_turn_of_it() {
        let (mut app, _, player, _) = player_at_work(10);
        frames(&mut app, 2);
        let damage = app.world().resource::<TheSides>().0.kind;
        app.world_mut().write_message(crate::combat::DamageEvent::new(player, atglyph_rules::Hit::from_source(None, damage, 2)));
        frames(&mut app, 3);
        assert_eq!(reasons(&app), vec![BreakReason::Hurt]);
        assert_eq!(app.world().resource::<Broken>().0[0].done, 3, "the pass it landed in was no turn of work");
        assert!(app.world().get::<Working>(player).is_none());
    }

    #[test]
    fn a_foe_stepping_into_the_players_sight_breaks_its_work_off_and_the_player_keeps_the_turn() {
        let (mut app, start, player, kind) = arena();
        app.insert_resource(WorkPace::every_frame());
        let sides = app.world().resource::<TheSides>().0;
        // In sight when the work is taken up: the player chose to begin
        // in front of it.
        app.world_mut().spawn((Actor, Blocks, Position(start.offset(-3, 0)), Health::full(5), crate::combat::Faction(sides.theirs)));
        // A friend and a foe, both out of sight.
        let friend = app.world_mut().spawn((Actor, Blocks, Position(start.offset(0, 12)), Health::full(5), crate::combat::Faction(sides.ours))).id();
        let foe = app.world_mut().spawn((Actor, Blocks, Position(start.offset(12, 0)), Health::full(5), crate::combat::Faction(sides.theirs))).id();
        app.world_mut().write_message(Intent::new(player, BeginWork(Work::new(kind, 20))));
        frames(&mut app, 3);
        assert!(reasons(&app).is_empty(), "a foe already in sight leaves it on");
        app.world_mut().get_mut::<Position>(friend).unwrap().0 = start.offset(0, 3);
        frames(&mut app, 2);
        assert!(reasons(&app).is_empty(), "and so does a friend coming into sight");
        let worked = done(&app, player).unwrap();
        app.world_mut().get_mut::<Position>(foe).unwrap().0 = start.offset(4, 0);
        let clock = app.world().resource::<Turns>().now();
        frames(&mut app, 3);
        assert_eq!(reasons(&app), vec![BreakReason::FoeSighted]);
        assert_eq!(app.world().resource::<Broken>().0[0].done, worked, "before another turn of it");
        assert_eq!(app.world().resource::<Turns>().now(), clock, "and the turn is the player's, unspent");
        assert!(app.world().get::<MyTurn>(player).is_some());
    }

    #[test]
    fn the_player_spending_a_turn_of_work_on_anything_else_leaves_it_off_and_does_that_instead() {
        let (mut app, _, player, _) = player_at_work(10);
        frames(&mut app, 2);
        let clock = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Wait));
        app.update();
        assert_eq!(reasons(&app), vec![BreakReason::LeftOff]);
        assert_eq!(app.world().resource::<Broken>().0[0].done, 3, "the wait was not a turn of work");
        assert_eq!(app.world().resource::<Turns>().now(), clock + BASE_ACTION_COST, "and the wait was spent");
    }

    #[test]
    fn a_status_that_holds_the_player_takes_its_turns_from_the_work_and_leaves_it_on() {
        let (mut app, _, player, _) = player_at_work(10);
        app.insert_resource(SpendTheirs(player));
        app.add_systems(Turn, spend_theirs.in_set(DecideSet::Sense));
        frames(&mut app, 4);
        assert!(reasons(&app).is_empty(), "lost turns are not a choice");
        assert_eq!(done(&app, player), Some(1), "and none of them was work");
    }

    /// Whose turns [`spend_theirs`] takes, as a status that holds takes them.
    #[derive(Resource)]
    struct SpendTheirs(Entity);

    fn spend_theirs(who: Res<SpendTheirs>, mut acting: ResMut<Acting>, mut spent: MessageWriter<ActionDone>, holding: Query<(), With<MyTurn>>) {
        if holding.contains(who.0) && acting.claim_decision(who.0) && acting.claim_action(who.0) {
            spent.write(ActionDone { actor: who.0, cost: BASE_ACTION_COST });
        }
    }

    /// What a run loaded mid-work has: the player carrying the work, and
    /// nothing of this session's to say it was ever begun.
    #[test]
    fn work_the_player_was_saved_at_carries_on_and_a_foe_in_sight_when_it_does_breaks_it() {
        let (mut app, start, player, kind) = arena();
        app.insert_resource(WorkPace::every_frame());
        app.world_mut().entity_mut(player).insert(Working(Work { kind, target: None, done: 4, needed: 6 }));
        frames(&mut app, 3);
        assert!(app.world().get::<Working>(player).is_none(), "carried on from where it was");
        assert_eq!(app.world().resource::<Finished>().0.len(), 1, "and done");

        let (mut app, _, player, kind) = arena();
        app.insert_resource(WorkPace::every_frame());
        let sides = app.world().resource::<TheSides>().0;
        app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(5), crate::combat::Faction(sides.theirs)));
        app.world_mut().entity_mut(player).insert(Working(Work { kind, target: None, done: 4, needed: 6 }));
        frames(&mut app, 2);
        assert_eq!(reasons(&app), vec![BreakReason::FoeSighted], "nobody can say the foe was there before the save");
    }

    #[test]
    fn a_mind_works_on_beside_the_player_and_is_never_broken_off_by_what_comes_into_sight() {
        let (mut app, start, player, kind) = arena();
        app.insert_resource(WorkPace::every_frame());
        let sides = app.world().resource::<TheSides>().0;
        let mender = app
            .world_mut()
            .spawn((Actor, Blocks, Position(start.offset(-12, 0)), Health::full(5), Viewshed::new(8), crate::combat::Faction(sides.theirs)))
            .id();
        app.world_mut().entity_mut(mender).insert(Working(Work::new(kind, 6)));
        app.world_mut().write_message(Intent::new(player, BeginWork(Work::new(kind, 20))));
        app.update();
        app.world_mut().get_mut::<Position>(mender).unwrap().0 = start.offset(-6, 0);
        frames(&mut app, 2);
        assert_eq!(reasons(&app), vec![BreakReason::FoeSighted], "the player looks up");
        for _ in 0..8 {
            pass(&mut app, player);
        }
        let finished: Vec<Entity> = app.world().resource::<Finished>().0.iter().map(|(d, _)| d.actor).collect();
        assert_eq!(finished, vec![mender], "the mender, with the player in its sight, works on to the end");
    }
}
