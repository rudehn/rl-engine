//! The turn loop.
//!
//! Four sets make one pass of the [`Turn`](crate::plugin::Turn) schedule:
//! the scheduler deals a turn, minds decide what to do with it, the engine
//! resolves what it knows how to resolve, and the scheduler puts the actor
//! back. The runner in [`plugin`](crate::plugin) repeats the pass within a
//! frame until the player holds a turn or nothing moves, so every monster
//! due before the player's next turn acts in the frame the player did. An
//! actor holding [`MyTurn`] is out of the queue until something reports
//! [`ActionDone`] or [`ActionRefused`] for it.

use bevy::prelude::*;
use rl_core::turn::{BASE_ACTION_COST, scaled_cost};
use rl_core::{Direction, Point, TurnQueue};
use rl_grid::SpatialGrid;

use crate::components::{Actor, Blocks, MyTurn, Player, Position, Speed, Viewshed};
use crate::places::{MapId, OnMap};
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
}

impl Turns {
    /// Whole turns elapsed.
    pub fn turn_number(&self) -> u32 {
        self.queue.now() / BASE_ACTION_COST
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
/// items, [`GoThrough`](crate::places::GoThrough) in places. A game adds its own
/// by implementing this on a type of its own, registering it with
/// [`AddAction::add_action`], and resolving it in
/// [`TurnSet::Resolve`](crate::plugin::TurnSet::Resolve). There is no
/// list of actions anywhere for a new one to be added to.
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

    /// Whether something already spent `actor`'s turn this pass.
    pub fn spent(&self, actor: Entity) -> bool {
        self.acting.has_acted(actor)
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

/// A whole turn has passed. Per-turn simulations subscribe to this.
#[derive(Message, Debug, Clone, Copy)]
pub struct TurnEnd {
    /// Whole turns elapsed.
    pub turn: u32,
}

/// Deals the next turn if nobody holds one.
///
/// Actors outside the loaded window are frozen: they are put back for a
/// full step without acting, so a distant crowd costs nothing per pass.
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
) {
    if !holding.is_empty() {
        return;
    }
    let mut advanced = false;
    // Bounded: an actor frozen outside the window is requeued each pass,
    // so a queue of only frozen actors would otherwise spin.
    for _ in 0..64 {
        match turns.queue.pop_due(|e| actors.contains(e)) {
            Some(entity) => {
                let Ok((pos, on)) = actors.get(entity) else { continue };
                let here = on.map(|m| m.0).unwrap_or(MapId::SURFACE) == map.current();
                if !here || !map.is_loaded(pos.0) {
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

/// Puts newly spawned actors into the queue and the occupancy index.
pub fn admit_new_actors(
    mut turns: ResMut<Turns>,
    mut occupancy: ResMut<Occupancy>,
    added_actors: Query<Entity, Added<Actor>>,
    added_blockers: Query<(Entity, &Position, Option<&OnMap>), Added<Blocks>>,
) {
    for e in &added_actors {
        if !turns.queue.contains(e) {
            turns.queue.insert_now(e);
        }
    }
    let here = occupancy.current();
    for (e, pos, on) in &added_blockers {
        occupancy.insert_on(on.map(|m| m.0).unwrap_or(here), pos.0, e);
    }
}

/// The actor holding the turn, as the resolver sees it.
type TurnHolder<'w, 's> =
    Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>, Has<Blocks>, Option<&'static crate::minds::Intelligence>), With<MyTurn>>;

/// The actor holding the turn, as the cleanup sees it.
type Holding<'w, 's> = Query<'w, 's, (Entity, Option<&'static Speed>, Has<Player>), With<MyTurn>>;

/// Resolves a move.
///
/// A move into an unwalkable or occupied cell is refused for the player
/// and treated as a wait for anyone else, which is what keeps a blocked
/// monster from retrying forever. A move into a closed door opens it
/// instead, for an actor with the wits to, and spends the turn where it
/// stands; see [`doors`](crate::doors).
pub fn resolve_moves(
    mut intents: MessageReader<Intent<Step>>,
    mut resolution: Resolution,
    mut map: ResMut<WorldMap>,
    mut occupancy: ResMut<Occupancy>,
    mut actors: TurnHolder,
    mut doors: MessageWriter<crate::doors::DoorEvent>,
) {
    for intent in intents.read() {
        if !resolution.claim(intent.actor) {
            debug!("actor {:?} holds no turn or already acted this pass; dropped {:?}", intent.actor, intent.action);
            continue;
        }
        let Ok((mut pos, viewshed, blocks, intelligence)) = actors.get_mut(intent.actor) else {
            resolution.failed(intent.actor, BASE_ACTION_COST);
            continue;
        };
        let dir = intent.action.0;
        let target = pos.0 + dir.offset();
        if occupancy.is_occupied(target) || !corner_ok(&map, pos.0, dir) {
            resolution.failed(intent.actor, BASE_ACTION_COST);
            continue;
        }
        if !map.is_walkable(target) {
            match map.opens(target) {
                Some(open) if crate::doors::works_doors(intelligence) => {
                    map.set_tile(target, open);
                    doors.write(crate::doors::DoorEvent::Opened { actor: intent.actor, at: target });
                    resolution.done(intent.actor, BASE_ACTION_COST);
                }
                _ => resolution.failed(intent.actor, BASE_ACTION_COST),
            }
            continue;
        }
        let cost = map.cost(target).unwrap_or(BASE_ACTION_COST);
        let cost = if dir.is_diagonal() { cost * 1414 / 1000 } else { cost };
        if blocks {
            occupancy.relocate(intent.actor, pos.0, target);
        }
        pos.0 = target;
        if let Some(mut v) = viewshed {
            v.dirty = true;
        }
        resolution.done(intent.actor, cost);
    }
}

/// A diagonal step may not squeeze between two unwalkable orthogonals.
fn corner_ok(map: &WorldMap, from: Point, dir: Direction) -> bool {
    if !dir.is_diagonal() {
        return true;
    }
    let (dx, dy) = dir.delta();
    map.is_walkable(from.offset(dx, 0)) && map.is_walkable(from.offset(0, dy))
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
        if resolution.spent(intent.actor) {
            continue;
        }
        warn!("nothing resolved {} for actor {:?}", std::any::type_name::<A>(), intent.actor);
        resolution.failed(intent.actor, BASE_ACTION_COST);
    }
}

/// Requeues actors that finished, keeps the turn of actors that were
/// refused, and recovers any non-player still holding a turn nobody used.
///
/// One turn is requeued once: a second [`ActionDone`] for the same actor
/// in one pass is ignored, so two resolvers answering two intents for one
/// holder cannot put it in the queue twice.
pub fn cleanup_turns(
    mut commands: Commands,
    mut turns: ResMut<Turns>,
    mut done: MessageReader<ActionDone>,
    mut refused: MessageReader<ActionRefused>,
    holding: Holding,
) {
    let mut handled: Vec<Entity> = Vec::new();
    for d in done.read() {
        if handled.contains(&d.actor) {
            continue;
        }
        if let Ok((e, speed, _)) = holding.get(d.actor) {
            let cost = scaled_cost(d.cost, speed.map(|s| s.0).unwrap_or(100));
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
    for (e, speed, is_player) in &holding {
        if is_player || handled.contains(&e) {
            continue;
        }
        let cost = scaled_cost(BASE_ACTION_COST, speed.map(|s| s.0).unwrap_or(100));
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
    /// intent nobody resolves is refused rather than left to hang.
    fn add_action<A: Action>(&mut self) -> &mut Self;
}

impl AddAction for App {
    fn add_action<A: Action>(&mut self) -> &mut Self {
        self.add_message::<Intent<A>>().add_systems(crate::plugin::Turn, sweep_unclaimed::<A>.in_set(crate::plugin::TurnSet::Sweep))
    }
}
