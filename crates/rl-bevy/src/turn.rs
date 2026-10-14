//! The turn loop.
//!
//! Four sets make one pass of the [`Turn`](crate::plugin::Turn) schedule:
//! the scheduler deals a turn, minds decide what to do with it, the engine
//! resolves what it knows how to resolve, and the scheduler puts the actor
//! back. The runner in [`plugin`](crate::plugin) repeats the pass within a
//! frame until the player holds a turn or nothing moves, so every monster
//! due before the player's next turn acts in the frame the player did. An
//! actor holding [`MyTurn`] is out of the queue until something reports
//! [`ActionDone`] or [`ActionRefused`] for it, and is put back as far ahead
//! as its action costs at its speed, as [`Speeds`] reads it: its own
//! [`Speed`], and the stat [`TurnRules`] names, which is what makes haste
//! and slowness statuses.

use bevy::prelude::*;
use rl_core::turn::{BASE_ACTION_COST, scaled_cost};
use rl_core::{Direction, Point, TurnQueue};
use rl_grid::SpatialGrid;
use rl_rules::StatId;

use crate::components::{Actor, Blocks, MyTurn, Player, Position, Speed, Viewshed};
use crate::cue::TurnHold;
use crate::places::{MapId, OnMap};
use crate::registries::Registries;
use crate::status::StatBlock;
use crate::world::WorldMap;

/// The scheduler.
#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub struct Turns {
    #[deref]
    queue: TurnQueue<Entity>,
    /// The last whole turn a [`TurnEnd`] was emitted for.
    last_turn: u32,
    /// Whether the current pass dealt, advanced or requeued anything. The
    /// runner clears it before a pass and stops when a pass leaves it clear.
    pub(crate) progress: bool,
    /// Actors spawned somewhere play is not, waiting to be admitted until
    /// it is: see [`admit_new_actors`]. In the order they were spawned.
    arriving: Vec<Entity>,
}

impl Turns {
    /// Whole turns elapsed.
    pub fn turn_number(&self) -> u32 {
        self.queue.now() / BASE_ACTION_COST
    }

    /// Marks this pass as having done something, so the runner deals
    /// another rather than reading it as an idle queue.
    ///
    /// For a resolver whose work lands a pass later than the turn that
    /// started it: see [`Airborne`](crate::cue::Airborne), whose landings
    /// are the only ones in the engine.
    pub fn made_progress(&mut self) {
        self.progress = true;
    }

    /// The clock and every waiting entry, for saving.
    pub fn export(&self) -> (u32, Vec<(Entity, u32)>) {
        (self.queue.now(), self.queue.entries().collect())
    }

    /// Replaces the clock and the queue with a saved one. Entries keep
    /// their times; ties among them fall back to the order given.
    pub fn import(&mut self, now: u32, entries: impl IntoIterator<Item = (Entity, u32)>) {
        self.queue = TurnQueue::new();
        self.queue.set_now(now);
        for (e, time) in entries {
            self.queue.insert_at(e, time);
        }
        self.last_turn = self.turn_number();
    }
}

/// How fast the turns run while nobody is waited on.
///
/// The loop stops for a player holding a turn, which is the only pace a
/// game somebody plays needs: the world moves when the player does. A run
/// nobody plays has nothing to stop for, and unpaced it is over in a few
/// frames. A pace is game time allowed per second of the wall clock: the
/// loop earns it each frame and spends it as the turn clock advances, so a
/// fight is watched at the speed it is set to whatever the frame rate.
///
/// Passes that do not move the clock are free, so everyone due at one
/// reading acts in one frame, and a pass that jumps the clock a long way
/// is paid off over the frames after it rather than refused. The pace only
/// decides how many passes a frame runs, never what a pass does, so the
/// same seed plays the same run at any pace, stopped and started or not.
///
/// A spent pace stops turns being dealt and nothing else: every frame
/// still runs a pass, so a turn already dealt is resolved, a player's
/// intent is answered in the frame it was written, and a warp or a
/// reaction asked for while the turns are stopped is still answered.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pace {
    /// Hundredths of a step per second, or `None` for as fast as the loop
    /// runs.
    rate: Option<u32>,
    /// Game time earned and not yet spent, in millionths of a hundredth of
    /// a step, so a frame of a few milliseconds earns a whole number.
    credit: i64,
}

impl Pace {
    /// The most a slow frame earns, in microseconds of wall clock: a hitch
    /// is not answered by a burst of turns nobody saw.
    const MOST_EARNED_MICROS: i64 = 250_000;

    /// As fast as the loop runs, which is the default and what a game
    /// somebody plays wants.
    pub const fn unpaced() -> Self {
        Self { rate: None, credit: 0 }
    }

    /// `hundredths` of a step of game time for each second watched: 100 is
    /// one ordinary step a second.
    pub const fn per_second(hundredths: u32) -> Self {
        Self { rate: Some(hundredths), credit: 0 }
    }

    /// No turn is dealt until the pace is changed.
    pub const fn stopped() -> Self {
        Self::per_second(0)
    }

    /// Hundredths of a step per second, or `None` when unpaced.
    pub fn rate(&self) -> Option<u32> {
        self.rate
    }

    /// Whether no turn is dealt at all.
    pub fn is_stopped(&self) -> bool {
        self.rate == Some(0)
    }

    /// Earns what `elapsed` of the wall clock allows.
    pub(crate) fn earn(&mut self, elapsed: std::time::Duration) {
        let Some(rate) = self.rate else { return };
        let micros = i64::try_from(elapsed.as_micros()).unwrap_or(i64::MAX).min(Self::MOST_EARNED_MICROS);
        // Never above what one capped frame earns: a pace left idle, with
        // nothing to deal, does not save up a burst for when there is.
        self.credit = (self.credit + i64::from(rate) * micros).min(i64::from(rate) * Self::MOST_EARNED_MICROS);
    }

    /// Pays for the turn clock having advanced by `hundredths`.
    pub(crate) fn spend(&mut self, hundredths: u32) {
        if self.rate.is_some() {
            self.credit -= i64::from(hundredths) * 1_000_000;
        }
    }

    /// Whether the pace allows no more game time this frame.
    pub(crate) fn is_spent(&self) -> bool {
        self.rate.is_some() && self.credit <= 0
    }
}

/// Who is standing where on the current map. Only entities with
/// [`Blocks`] are indexed. Other maps' indexes are kept aside and swapped
/// in when the player goes there.
#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub struct Occupancy {
    #[deref]
    grid: SpatialGrid<Entity>,
    current: MapId,
    stash: std::collections::BTreeMap<MapId, SpatialGrid<Entity>>,
}

impl Occupancy {
    /// The map the index is of.
    pub fn current(&self) -> MapId {
        self.current
    }

    /// Indexes `e` at `p` on `map`, whether or not that is the current map.
    pub fn insert_on(&mut self, map: MapId, p: Point, e: Entity) {
        if map == self.current {
            self.grid.insert(p, e);
        } else {
            self.stash.entry(map).or_default().insert(p, e);
        }
    }

    /// Takes `e` off `p` on `map`, whether or not that is the current map.
    pub fn remove_on(&mut self, map: MapId, p: Point, e: Entity) {
        if map == self.current {
            self.grid.remove(p, e);
        } else if let Some(grid) = self.stash.get_mut(&map) {
            grid.remove(p, e);
        }
    }

    /// Whether anything stands on `p` on `map`, whether or not that is the
    /// current map.
    pub fn is_occupied_on(&self, map: MapId, p: Point) -> bool {
        if map == self.current { self.grid.is_occupied(p) } else { self.stash.get(&map).is_some_and(|grid| grid.is_occupied(p)) }
    }

    /// Swaps in the index for `map`, keeping the current one aside.
    pub fn switch(&mut self, map: MapId) {
        if map == self.current {
            return;
        }
        let incoming = self.stash.remove(&map).unwrap_or_default();
        let outgoing = std::mem::replace(&mut self.grid, incoming);
        self.stash.insert(self.current, outgoing);
        self.current = map;
    }
}

/// One thing an actor can do with its turn.
///
/// The engine ships the actions every roguelike needs, each owned by the
/// module that owns the mechanic: [`Step`] and [`Wait`] here,
/// [`Attack`](crate::combat::Attack) in combat, the item actions in
/// items, [`Open`](crate::doors::Open) and [`Close`](crate::doors::Close)
/// in doors, [`GoThrough`](crate::places::GoThrough) in places. A game
/// adds its own by implementing this on a type of its own, registering it
/// with [`AddAction::add_action`], and resolving it in
/// [`TurnSet::Resolve`](crate::plugin::TurnSet::Resolve). There is no
/// list of actions anywhere for a new one to be added to.
///
/// An action may instead be an alternate: one that comes to another. It is
/// read in [`ResolveSet::Redirect`](crate::plugin::ResolveSet::Redirect),
/// the intent it comes to is written, and nothing is claimed, so the
/// resolver of that intent spends the turn. [`Bump`](crate::bump::Bump),
/// which comes to a step, an opening or a blow, is the engine's own.
pub trait Action: Send + Sync + 'static {}

/// A decision for the actor holding [`MyTurn`]: written by the game's input
/// system in [`EngineSet::Input`](crate::plugin::EngineSet::Input) for the
/// player, and by minds in [`TurnSet::Decide`](crate::plugin::TurnSet::Decide)
/// for everyone else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Intent<A: Action> {
    /// Who.
    pub actor: Entity,
    /// What.
    pub action: A,
}

impl<A: Action> Message for Intent<A> {}

impl<A: Action> Intent<A> {
    /// An intent for `actor` to do `action`.
    pub fn new(actor: Entity, action: A) -> Self {
        Self { actor, action }
    }
}

/// Step one cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step(pub Direction);
impl Action for Step {}

/// Do nothing for one action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wait;
impl Action for Wait {}

/// Who has already chosen and who has already acted, this pass.
///
/// One turn is one action. A resolver claims the actor before it resolves,
/// so a second intent in the same pass finds the turn spent, whatever kind
/// of action it is. A game's own decider claims in
/// [`TurnSet::Decide`](crate::plugin::TurnSet::Decide) so the minds leave
/// that actor alone.
#[derive(Resource, Debug, Default)]
pub struct Acting {
    decided: Vec<Entity>,
    acted: Vec<Entity>,
    /// Players refused by [`Resolution::refuse`] this pass: answered, so
    /// the sweep does not refuse them again, but not claimed, so another
    /// intent from the same press may still spend the turn.
    refused: Vec<Entity>,
}

impl Acting {
    /// Claims the right to choose for `actor` this pass. False if
    /// something already chose.
    pub fn claim_decision(&mut self, actor: Entity) -> bool {
        Self::mark(&mut self.decided, actor)
    }

    /// Whether something has already chosen for `actor` this pass.
    pub fn has_decided(&self, actor: Entity) -> bool {
        self.decided.contains(&actor)
    }

    /// Claims the actor's turn for resolution. False if it already acted.
    pub fn claim_action(&mut self, actor: Entity) -> bool {
        Self::mark(&mut self.acted, actor)
    }

    /// Whether `actor` has already acted this pass.
    pub fn has_acted(&self, actor: Entity) -> bool {
        self.acted.contains(&actor)
    }

    /// Whether the player `actor` asked for something this pass that was
    /// refused by [`Resolution::refuse`]: answered without its turn being
    /// spent, so [`has_acted`](Self::has_acted) does not see it, though
    /// the player did ask.
    pub fn was_refused(&self, actor: Entity) -> bool {
        self.refused.contains(&actor)
    }

    fn mark(list: &mut Vec<Entity>, actor: Entity) -> bool {
        if list.contains(&actor) {
            return false;
        }
        list.push(actor);
        true
    }
}

/// How a resolver spends a turn: the claim that keeps one turn to one
/// action, and the outcome that puts the actor back in the queue.
///
/// Every resolver has one shape, the engine's and a game's alike: claim
/// the actor holding the turn, attempt the action, say how it went. The
/// rule for a failure lives here and nowhere else. The player keeps its
/// turn and may try something else, and anyone else is charged and moves
/// on, because a monster handed a free retry asks again forever. A
/// resolver that wrote [`ActionRefused`] itself had to remember that; one
/// that calls [`failed`](Self::failed) cannot get it wrong.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Resolution<'w, 's> {
    acting: ResMut<'w, Acting>,
    finished: MessageWriter<'w, ActionDone>,
    refusals: MessageWriter<'w, ActionRefused>,
    holding: Query<'w, 's, (), With<MyTurn>>,
    players: Query<'w, 's, (), With<Player>>,
}

impl Resolution<'_, '_> {
    /// Claims `actor`'s turn for this resolver. False when it holds no turn
    /// or something already spent this one, and the intent is then stale
    /// or a second choice: skip it.
    pub fn claim(&mut self, actor: Entity) -> bool {
        self.holding.contains(actor) && self.acting.claim_action(actor)
    }

    /// Refuses an intent that found nothing to act on, without claiming
    /// the player's turn: a game may read one key as two asks, a way
    /// through and what is here, and the press should still do the one
    /// that found something. The player is noted as refused for the pass,
    /// once, so [`sweep_unclaimed`] does not refuse the same ask again.
    /// Anyone else is claimed and charged as by [`failed`](Self::failed),
    /// so a monster's refusal is never a second action beside another of
    /// the same pass.
    pub fn refuse(&mut self, actor: Entity, cost: u32) {
        if !self.holding.contains(actor) || self.acting.has_acted(actor) {
            return;
        }
        if self.players.contains(actor) {
            if Acting::mark(&mut self.acting.refused, actor) {
                self.refusals.write(ActionRefused { actor });
            }
        } else if self.acting.claim_action(actor) {
            self.finished.write(ActionDone { actor, cost });
        }
    }

    /// Whether something already spent `actor`'s turn this pass.
    pub fn spent(&self, actor: Entity) -> bool {
        self.acting.has_acted(actor)
    }

    /// Whether `actor` was answered this pass, its turn spent or refused
    /// by [`refuse`](Self::refuse), so a sweep owes it nothing.
    fn answered(&self, actor: Entity) -> bool {
        self.acting.has_acted(actor) || self.acting.refused.contains(&actor)
    }

    /// The action happened and owes `cost` hundredths of a step, before
    /// speed.
    pub fn done(&mut self, actor: Entity, cost: u32) {
        self.finished.write(ActionDone { actor, cost });
    }

    /// The action could not be done. The player keeps its turn at no
    /// cost; anyone else is charged `cost`, what the attempt would have
    /// taken, and moves on.
    pub fn failed(&mut self, actor: Entity, cost: u32) {
        if self.players.contains(actor) {
            self.refusals.write(ActionRefused { actor });
        } else {
            self.finished.write(ActionDone { actor, cost });
        }
    }
}

/// Forgets the pass that just ended. First thing in every pass.
pub fn start_pass(mut acting: ResMut<Acting>) {
    acting.decided.clear();
    acting.acted.clear();
    acting.refused.clear();
}

/// An actor finished an action and owes `cost` of game time.
#[derive(Message, Debug, Clone, Copy)]
pub struct ActionDone {
    /// Who.
    pub actor: Entity,
    /// In hundredths of a normal step, before speed.
    pub cost: u32,
}

/// An actor tried something that could not be done. No time passes, and
/// the actor keeps its turn.
///
/// Only ever written for the player: a non-player handed a free retry loops
/// forever. Write it through [`Resolution::failed`], which checks that.
#[derive(Message, Debug, Clone, Copy)]
pub struct ActionRefused {
    /// Who.
    pub actor: Entity,
}

/// An actor stepped from one cell to the next under its own power.
///
/// Written by the move resolver for every step it lets through, and only
/// then: a warp, a shove or a swap is not a step. What reads it is what
/// cares that someone walked, such as the noise their feet made.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stepped {
    /// Who.
    pub actor: Entity,
    /// From where.
    pub from: Point,
    /// To where.
    pub to: Point,
}

/// A whole turn has passed. Per-turn simulations subscribe to this.
#[derive(Message, Debug, Clone, Copy)]
pub struct TurnEnd {
    /// Whole turns elapsed.
    pub turn: u32,
}

/// Whether an actor at `pos` on `on` stands where play is: on the current
/// map, inside its loaded window. Anywhere else it can neither act nor be
/// seen, and an unset [`OnMap`] is the surface.
fn where_play_is(map: &WorldMap, pos: Point, on: Option<&OnMap>) -> bool {
    on.map(|m| m.0).unwrap_or(MapId::SURFACE) == map.current() && map.is_loaded(pos)
}

/// What stops a turn being dealt though the queue has one due.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Brakes<'w> {
    hold: Res<'w, TurnHold>,
    pace: Res<'w, Pace>,
}

impl Brakes<'_> {
    /// Whether nothing is to be dealt this pass.
    ///
    /// While something flies: what it does when it lands comes before
    /// whatever the next actor would do. And while the pace has no game
    /// time left to give: the rest of the pass still runs, so what was
    /// asked for is answered, but nobody new is given a turn and the clock
    /// stays where it is.
    fn on(&self) -> bool {
        self.hold.in_flight() || self.pace.is_spent()
    }
}

/// Deals the next turn if nobody holds one.
///
/// Actors that were admitted and have since been left behind, on another
/// map or outside the loaded window, are frozen: they are put back for a
/// full step without acting, so a distant crowd costs nothing per pass.
/// An actor that has never been where play is was never admitted at all;
/// see [`admit_new_actors`].
/// The clock advances at most once per pass, so a queue holding only
/// frozen actors moves time forward one step at a time rather than racing
/// ahead inside the loop.
pub fn schedule(
    mut commands: Commands,
    mut turns: ResMut<Turns>,
    mut ends: MessageWriter<TurnEnd>,
    holding: Query<Entity, With<MyTurn>>,
    actors: Query<(&Position, Option<&OnMap>), With<Actor>>,
    map: Res<WorldMap>,
    brakes: Brakes,
) {
    if !holding.is_empty() || brakes.on() {
        return;
    }
    let mut advanced = false;
    // Bounded: an actor frozen outside the window is requeued each pass,
    // so a queue of only frozen actors would otherwise spin.
    for _ in 0..64 {
        match turns.queue.pop_due(|e| actors.contains(e)) {
            Some(entity) => {
                let Ok((pos, on)) = actors.get(entity) else { continue };
                if !where_play_is(&map, pos.0, on) {
                    debug!("actor {entity:?} at {:?} is on another map or outside the loaded window; frozen", pos.0);
                    turns.queue.insert_after(entity, BASE_ACTION_COST);
                    continue;
                }
                commands.entity(entity).insert(MyTurn);
                turns.progress = true;
                return;
            }
            None => {
                if advanced || !turns.queue.advance_to_next() {
                    return;
                }
                advanced = true;
                turns.progress = true;
                let turn = turns.queue.now() / BASE_ACTION_COST;
                if turn > turns.last_turn {
                    turns.last_turn = turn;
                    ends.write(TurnEnd { turn });
                }
            }
        }
    }
}

/// Every actor, as admission asks whether it has arrived: whether it is
/// the player, and where it stands, if anywhere yet.
type Arrivals<'w, 's> = Query<'w, 's, (Has<Player>, Option<&'static Position>, Option<&'static OnMap>), With<Actor>>;

/// Puts newly spawned actors into the queue and the occupancy index.
///
/// An actor is admitted when it first stands where play is, on the current
/// map inside the loaded window, which for most is the pass after it was
/// spawned. One spawned anywhere else waits, out of the queue, and is
/// admitted the pass it arrives: a player spawned at the origin and warped
/// onto its first map as a run begins, or a crowd placed on a map nobody
/// has gone to yet. Admitted at once and left to [`schedule`], such an
/// actor would be frozen a step forward each time it came due before it
/// got there, and the player would arrive a step or two behind the
/// monsters its own arrival put there.
///
/// Actors admitted in one pass go in with the player first and the rest in
/// the order of where they stand, by row and then column. A query's
/// iteration order follows the archetypes as they happened to be created,
/// which a new component anywhere in the engine can reorder, and an
/// entity's number is not the order it was spawned in, since Bevy hands a
/// freed number to the next spawn, so a light made a moment earlier would
/// renumber every monster after it; the first turn of a run must depend on
/// neither. Two that block never share a cell, and the number settles only
/// two that do not, admitted together where they stand together.
pub fn admit_new_actors(
    mut turns: ResMut<Turns>,
    mut occupancy: ResMut<Occupancy>,
    map: Res<WorldMap>,
    added_actors: Query<Entity, Added<Actor>>,
    actors: Arrivals,
    added_blockers: Query<(Entity, &Position, Option<&OnMap>), Added<Blocks>>,
) {
    let mut waiting = std::mem::take(&mut turns.arriving);
    waiting.extend(added_actors.iter());
    let mut fresh: Vec<(bool, i32, i32, Entity)> = Vec::new();
    for e in waiting {
        // Despawned, no longer an actor, already queued, or seen twice:
        // nothing to admit.
        let Ok((player, pos, on)) = actors.get(e) else { continue };
        if turns.queue.contains(e) || fresh.iter().any(|(.., f)| *f == e) || turns.arriving.contains(&e) {
            continue;
        }
        match pos {
            Some(pos) if where_play_is(&map, pos.0, on) => fresh.push((!player, pos.0.y, pos.0.x, e)),
            _ => turns.arriving.push(e),
        }
    }
    fresh.sort();
    for (.., e) in fresh {
        turns.queue.insert_now(e);
    }
    let here = occupancy.current();
    for (e, pos, on) in &added_blockers {
        occupancy.insert_on(on.map(|m| m.0).unwrap_or(here), pos.0, e);
    }
}

/// The actor holding the turn, as the resolver sees it.
type TurnHolder<'w, 's> = Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>, Has<Blocks>), With<MyTurn>>;

/// The actor holding the turn, as the cleanup sees it.
type Holding<'w, 's> = Query<'w, 's, (Entity, Has<Player>), With<MyTurn>>;

/// Resolves a move.
///
/// A move into an unwalkable or occupied cell is refused for the player
/// and treated as a wait for anyone else, which is what keeps a blocked
/// monster from retrying forever. A shut door is unwalkable: opening one
/// is [`Open`](crate::doors::Open), which a [`Bump`](crate::bump::Bump)
/// comes to when that is what is in the way.
pub fn resolve_moves(
    mut intents: MessageReader<Intent<Step>>,
    mut resolution: Resolution,
    map: Res<WorldMap>,
    mut occupancy: ResMut<Occupancy>,
    mut actors: TurnHolder,
    mut stepped: MessageWriter<Stepped>,
) {
    for intent in intents.read() {
        if !resolution.claim(intent.actor) {
            debug!("actor {:?} holds no turn or already acted this pass; dropped {:?}", intent.actor, intent.action);
            continue;
        }
        let Ok((mut pos, viewshed, blocks)) = actors.get_mut(intent.actor) else {
            resolution.failed(intent.actor, BASE_ACTION_COST);
            continue;
        };
        let dir = intent.action.0;
        let target = pos.0 + dir.offset();
        if occupancy.is_occupied(target) || !corner_ok(&map, pos.0, dir) || !map.is_walkable(target) {
            resolution.failed(intent.actor, BASE_ACTION_COST);
            continue;
        }
        let cost = map.cost(target).unwrap_or(BASE_ACTION_COST);
        let cost = if dir.is_diagonal() { cost * 1414 / 1000 } else { cost };
        if blocks {
            occupancy.relocate(intent.actor, pos.0, target);
        }
        stepped.write(Stepped { actor: intent.actor, from: pos.0, to: target });
        pos.0 = target;
        if let Some(mut v) = viewshed {
            v.dirty = true;
        }
        resolution.done(intent.actor, cost);
    }
}

/// A diagonal step may squeeze only between two orthogonals that let it
/// past, by [`WorldMap::lets_diagonal_past`], the rule every view a mind
/// paths over answers too.
pub(crate) fn corner_ok(map: &WorldMap, from: Point, dir: Direction) -> bool {
    if !dir.is_diagonal() {
        return true;
    }
    let (dx, dy) = dir.delta();
    map.lets_diagonal_past(from.offset(dx, 0)) && map.lets_diagonal_past(from.offset(0, dy))
}

/// Resolves a wait: the turn passes and nothing else happens.
pub fn resolve_waits(mut intents: MessageReader<Intent<Wait>>, mut resolution: Resolution) {
    for intent in intents.read() {
        if resolution.claim(intent.actor) {
            resolution.done(intent.actor, BASE_ACTION_COST);
        }
    }
}

/// Refuses an action nobody resolved.
///
/// Registered once per action type by [`AddAction::add_action`]. Without
/// it a game that registers an action and forgets its resolver leaves the
/// player holding a turn nothing will ever spend, which reads as a frozen
/// game rather than a mistake.
pub fn sweep_unclaimed<A: Action>(mut intents: MessageReader<Intent<A>>, mut resolution: Resolution) {
    for intent in intents.read() {
        if resolution.answered(intent.actor) {
            continue;
        }
        warn!("nothing resolved {} for actor {:?}", std::any::type_name::<A>(), intent.actor);
        resolution.failed(intent.actor, BASE_ACTION_COST);
    }
}

/// Which stat makes an actor faster or slower than its own [`Speed`].
///
/// A rule rather than content, as the stats a blow reads are
/// [`CombatRules`](crate::combat::CombatRules)': the stat is a registry
/// entry, and this says it is the one the turn loop reads, so haste and
/// slowness are statuses that put a modifier on it, and a worn thing that
/// quickens its wearer bestows one. Optional data: without it, or with no
/// stat named, every actor is as fast as its own `Speed`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TurnRules {
    /// The stat whose value is a percentage added to the hundred of every
    /// actor's [`Speed`], if any.
    pub speed: Option<StatId>,
}

impl TurnRules {
    /// No stat read: every actor as fast as its own [`Speed`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads `stat` as how much faster an actor is than its own [`Speed`]:
    /// its value is a percentage added to the hundred of it, so a hundred
    /// is twice as fast and minus fifty half as fast, whatever the actor's
    /// own speed. A stat whose base is nought, then, so an actor with
    /// nothing on it is as fast as it always was; the stat's own bounds are
    /// where a game says how fast is fastest and how slow is slowest.
    pub fn speed_stat(mut self, stat: StatId) -> Self {
        self.speed = Some(stat);
        self
    }
}

/// How fast every actor is, as the turn loop reads it: its own [`Speed`],
/// with the stat [`TurnRules`] names on top.
///
/// The one reader, so the queue and whatever shows a speed, a sheet or a
/// forecast, never disagree about who is hasted.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Speeds<'w, 's> {
    rules: Option<Res<'w, TurnRules>>,
    registries: Option<Res<'w, Registries>>,
    actors: Query<'w, 's, (Option<&'static Speed>, Option<&'static StatBlock>)>,
}

impl Speeds<'_, '_> {
    /// `who`'s speed in percent of normal: its own [`Speed`], a hundred
    /// when it has none, with the value of the stat [`TurnRules`] names
    /// added as a percentage of it, and nothing added when no stat is
    /// named or `who` has no stats.
    ///
    /// Never below one, the slowest the queue deals a turn to: an actor
    /// that is not to act at all is held by a status, not slowed to
    /// nothing.
    pub fn of(&self, who: Entity) -> u32 {
        let Ok((speed, stats)) = self.actors.get(who) else { return Speed::default().0 };
        let own = speed.map_or(Speed::default().0, |s| s.0);
        let percent = match (self.rules.as_deref().and_then(|r| r.speed), self.registries.as_deref(), stats) {
            (Some(stat), Some(registries), Some(stats)) => stats.value(stat, &registries.stats),
            _ => 0,
        };
        quickened(own, percent)
    }
}

/// `speed` with `percent` of it added: a hundred doubles it, minus fifty
/// halves it, and nothing takes it below one, as
/// [`scaled_cost`] already reads a speed of nothing.
fn quickened(speed: u32, percent: i32) -> u32 {
    let scaled = i64::from(speed) * (100 + i64::from(percent)) / 100;
    scaled.clamp(1, i64::from(u32::MAX)) as u32
}

/// Requeues actors that finished, keeps the turn of actors that were
/// refused, and recovers any non-player still holding a turn nobody used.
///
/// One turn is requeued once: a second [`ActionDone`] for the same actor
/// in one pass is ignored, so two resolvers answering two intents for one
/// holder cannot put it in the queue twice. Every cost is scaled by the
/// actor's speed as [`Speeds`] reads it, so a stat [`TurnRules`] names
/// counts wherever a turn is put back.
pub fn cleanup_turns(
    mut commands: Commands,
    mut turns: ResMut<Turns>,
    mut done: MessageReader<ActionDone>,
    mut refused: MessageReader<ActionRefused>,
    holding: Holding,
    speeds: Speeds,
) {
    let mut handled: Vec<Entity> = Vec::new();
    for d in done.read() {
        if handled.contains(&d.actor) {
            continue;
        }
        if let Ok((e, _)) = holding.get(d.actor) {
            let cost = scaled_cost(d.cost, speeds.of(e));
            debug!("actor {e:?} finished an action costing {cost}");
            turns.queue.insert_after(e, cost);
            turns.progress = true;
            commands.entity(e).remove::<MyTurn>();
            handled.push(e);
        }
    }
    for r in refused.read() {
        // The actor keeps its turn: nothing to do but note it was seen.
        handled.push(r.actor);
    }
    // A non-player still holding a turn after Decide and Resolve had their
    // say is stranded. Charge it a wait rather than freeze the game.
    for (e, is_player) in &holding {
        if is_player || handled.contains(&e) {
            continue;
        }
        let cost = scaled_cost(BASE_ACTION_COST, speeds.of(e));
        debug!("actor {e:?} held a turn nobody used; charged a wait");
        turns.queue.insert_after(e, cost);
        turns.progress = true;
        commands.entity(e).remove::<MyTurn>();
    }
}

/// Drops despawned actors from the occupancy index.
pub fn forget_removed_blockers(mut occupancy: ResMut<Occupancy>, mut removed: RemovedComponents<Blocks>) {
    let gone: Vec<Entity> = removed.read().collect();
    if gone.is_empty() {
        return;
    }
    let stale: Vec<(Point, Entity)> = occupancy.iter().filter(|(_, e)| gone.contains(e)).collect();
    for (p, e) in stale {
        occupancy.remove(p, e);
    }
}

/// Registers an action with the engine.
pub trait AddAction {
    /// Registers `A` as an action: its intents become a message, and an
    /// intent nobody resolves is refused rather than left to hang. Once:
    /// two plugins that both register the same action, as combat and the
    /// minds both do for a blow, get one sweeper between them.
    fn add_action<A: Action>(&mut self) -> &mut Self;
}

impl AddAction for App {
    fn add_action<A: Action>(&mut self) -> &mut Self {
        // The message alone is not proof of a sweeper: a plugin may
        // register the message so it can write intents it does not resolve,
        // as the minds do, so the sweeper is what is remembered.
        if self.world().contains_resource::<Swept<A>>() {
            return self;
        }
        self.insert_resource(Swept::<A>(std::marker::PhantomData));
        self.add_message::<Intent<A>>().add_systems(crate::plugin::Turn, sweep_unclaimed::<A>.in_set(crate::plugin::TurnSet::Sweep))
    }
}

/// Present once `A` has a sweeper, so a second registration adds none.
#[derive(Resource)]
struct Swept<A: Action>(std::marker::PhantomData<A>);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Health;
    use crate::plugin::{Turn, TurnSet, headless_app};
    use crate::registries::Registries;
    use crate::state::EngineState;
    use crate::status::Afflict;
    use rl_rules::{Op, Registry, StatDef, StatusDef, StatusId, UNTIL_CURED};

    /// A world with statuses in it and a stat, `quickness`, that `hasted`
    /// adds a hundred to and `slowed` takes fifty from, read as speed when
    /// `rules` says so; and a player to pass the turns, holding the first.
    fn quick_arena(rules: bool) -> (App, Point, Entity, [StatusId; 2]) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin, crate::combat::CombatPlugin));
        let start = crate::testing::surface(&mut app);
        crate::testing::two_sides(&mut app);
        let stats = Registry::from_defs(vec![StatDef::new("quickness", 0)]).unwrap();
        let quickness = stats.expect("quickness");
        let statuses =
            Registry::from_defs(vec![StatusDef::new("hasted").modifies(quickness, Op::Add(100)), StatusDef::new("slowed").modifies(quickness, Op::Add(-50))])
                .unwrap();
        let both = [statuses.expect("hasted"), statuses.expect("slowed")];
        {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.stats = stats;
            registries.statuses = statuses;
        }
        if rules {
            app.insert_resource(TurnRules::new().speed_stat(quickness));
        }
        app.init_resource::<Dealt>().add_systems(Turn, record_deals.in_set(TurnSet::Decide));
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (app, start, player, both)
    }

    /// Every turn dealt: to whom, and when.
    #[derive(Resource, Default)]
    struct Dealt(Vec<(Entity, u32)>);

    fn record_deals(fresh: Query<Entity, Added<MyTurn>>, turns: Res<Turns>, mut dealt: ResMut<Dealt>) {
        dealt.0.extend(fresh.iter().map(|e| (e, turns.now())));
    }

    /// How many turns `who` was dealt before the clock read `before`.
    fn dealt_before(app: &App, who: Entity, before: u32) -> usize {
        app.world().resource::<Dealt>().0.iter().filter(|(e, at)| *e == who && *at < before).count()
    }

    fn afflict(app: &mut App, target: Entity, status: StatusId) {
        app.world_mut().write_message(Afflict { target, status, turns: UNTIL_CURED, by: None, held_by: None });
    }

    /// Nobody decides for the three, so each is charged a wait for every
    /// turn it is dealt, at the speed the loop reads: over twenty of the
    /// player's turns the hasted one is dealt twice the turns of the one
    /// with nothing on it, and the slowed one half, whatever speed the
    /// three share, give or take the turn a rounded cost moves.
    #[test]
    fn a_speed_stat_of_a_hundred_doubles_how_often_an_actor_acts_and_one_of_minus_fifty_halves_it() {
        for speed in [50, 75, 100, 150, 200] {
            let (mut app, start, player, [hasted, slowed]) = quick_arena(true);
            let [plain, quick, slow] = [2, 3, 4].map(|dx| app.world_mut().spawn((Actor, Blocks, Position(start.offset(dx, 2)), Speed(speed))).id());
            afflict(&mut app, quick, hasted);
            afflict(&mut app, slow, slowed);
            for _ in 0..20 {
                app.world_mut().write_message(Intent::new(player, Wait));
                app.update();
            }
            let span = 20 * BASE_ACTION_COST;
            assert_eq!(app.world().resource::<Turns>().now(), span, "speed {speed}: twenty of the player's turns");
            let [plain, quick, slow] = [plain, quick, slow].map(|e| dealt_before(&app, e, span));
            assert!(plain >= 10, "speed {speed}: dealt {plain}");
            assert!(quick.abs_diff(2 * plain) <= 1, "speed {speed}: the hasted one was dealt {quick} against {plain}");
            assert!((2 * slow).abs_diff(plain) <= 1, "speed {speed}: the slowed one was dealt {slow} against {plain}");
        }
    }

    /// The player's own wait is spent by its resolver rather than charged
    /// by the recovery net, and is scaled the same way; and a stat no rule
    /// names is only a number, whatever is on it.
    #[test]
    fn the_stat_the_rules_name_scales_what_the_players_wait_costs_and_one_they_do_not_name_changes_nothing() {
        for (rules, which, next) in [(false, 0, 100), (true, 0, 50), (true, 1, 200)] {
            let (mut app, _, player, statuses) = quick_arena(rules);
            afflict(&mut app, player, statuses[which]);
            app.update();
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
            assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds its next turn");
            assert_eq!(app.world().resource::<Turns>().now(), next, "named {rules}, status {which}");
        }
    }

    #[test]
    fn a_speed_stat_is_a_percentage_of_the_actors_own_speed_and_never_takes_it_below_the_slowest_there_is() {
        assert_eq!(quickened(100, 0), 100);
        assert_eq!(quickened(150, 100), 300);
        assert_eq!(quickened(150, -50), 75);
        assert_eq!(quickened(100, -100), 1, "an actor that is not to act is held, not slowed to nothing");
        assert_eq!(quickened(100, -500), 1);
        assert_eq!(quickened(0, 0), 1, "as `scaled_cost` reads a speed of nothing");
        assert_eq!(quickened(u32::MAX, 100), u32::MAX, "and nothing wraps");
    }
}
