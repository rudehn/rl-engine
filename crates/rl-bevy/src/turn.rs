//! The turn loop.
//!
//! Four sets run in order every frame while playing: the scheduler deals a
//! turn, the game decides what to do with it, the engine resolves what it
//! knows how to resolve, and the scheduler puts the actor back. An actor
//! holding [`MyTurn`] is out of the queue until something reports
//! [`ActionDone`] or [`ActionRefused`] for it.

use bevy::prelude::*;
use rl_core::turn::{BASE_ACTION_COST, scaled_cost};
use rl_core::{Direction, Point, TurnQueue};
use rl_grid::SpatialGrid;

use crate::components::{Actor, Blocks, MyTurn, Player, Position, Speed, Viewshed};
use crate::world::WorldMap;

/// The scheduler.
#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub struct Turns {
    #[deref]
    queue: TurnQueue<Entity>,
    /// The last whole turn a [`TurnEnd`] was emitted for.
    last_turn: u32,
}

impl Turns {
    /// Whole turns elapsed.
    pub fn turn_number(&self) -> u32 {
        self.queue.now() / BASE_ACTION_COST
    }
}

/// Who is standing where. Only entities with [`Blocks`] are indexed.
#[derive(Resource, Debug, Default, Deref, DerefMut)]
pub struct Occupancy(pub SpatialGrid<Entity>);

/// What an actor does with its turn, as far as the engine knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Step one cell.
    Move(Direction),
    /// Do nothing for one action.
    Wait,
}

/// A decision, written in [`EngineSet::Decide`](crate::plugin::EngineSet::Decide)
/// for the actor holding [`MyTurn`].
#[derive(Message, Debug, Clone, Copy)]
pub struct Intent {
    /// Who.
    pub actor: Entity,
    /// What.
    pub action: Action,
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
/// forever. The engine's own emit site checks that; a game's must too.
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
/// full step without acting, so a distant crowd costs nothing per frame.
/// The clock advances at most once per frame, so a queue holding only
/// frozen actors moves time forward at one step per frame rather than
/// racing ahead inside the loop.
pub fn schedule(
    mut commands: Commands,
    mut turns: ResMut<Turns>,
    mut ends: MessageWriter<TurnEnd>,
    holding: Query<Entity, With<MyTurn>>,
    actors: Query<(&Position, Option<&Player>), With<Actor>>,
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
                let Ok((pos, _)) = actors.get(entity) else { continue };
                if !map.is_loaded(pos.0) {
                    debug!("actor {entity:?} at {:?} is outside the loaded window; frozen", pos.0);
                    turns.queue.insert_after(entity, BASE_ACTION_COST);
                    continue;
                }
                commands.entity(entity).insert(MyTurn);
                return;
            }
            None => {
                if advanced || !turns.queue.advance_to_next() {
                    return;
                }
                advanced = true;
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
    added_blockers: Query<(Entity, &Position), Added<Blocks>>,
) {
    for e in &added_actors {
        if !turns.queue.contains(e) {
            turns.queue.insert_now(e);
        }
    }
    for (e, pos) in &added_blockers {
        occupancy.insert(pos.0, e);
    }
}

/// The actor holding the turn, as the resolver sees it.
type TurnHolder<'w, 's> = Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>, Has<Blocks>, Has<Player>), With<MyTurn>>;

/// The actor holding the turn, as the cleanup sees it.
type Holding<'w, 's> = Query<'w, 's, (Entity, Option<&'static Speed>, Has<Player>), With<MyTurn>>;

/// Resolves the actions the engine understands: moves and waits.
///
/// A move into an unwalkable or occupied cell is refused for the player
/// and treated as a wait for anyone else.
pub fn resolve_intents(
    mut intents: MessageReader<Intent>,
    mut done: MessageWriter<ActionDone>,
    mut refused: MessageWriter<ActionRefused>,
    map: Res<WorldMap>,
    mut occupancy: ResMut<Occupancy>,
    mut actors: TurnHolder,
) {
    for intent in intents.read() {
        let Ok((mut pos, viewshed, blocks, is_player)) = actors.get_mut(intent.actor) else { continue };
        match intent.action {
            Action::Wait => {
                done.write(ActionDone { actor: intent.actor, cost: BASE_ACTION_COST });
            }
            Action::Move(dir) => {
                let target = pos.0 + dir.offset();
                let open = map.is_walkable(target) && !occupancy.is_occupied(target) && corner_ok(&map, pos.0, dir);
                if !open {
                    if is_player {
                        refused.write(ActionRefused { actor: intent.actor });
                    } else {
                        done.write(ActionDone { actor: intent.actor, cost: BASE_ACTION_COST });
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
                done.write(ActionDone { actor: intent.actor, cost });
            }
        }
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

/// Requeues actors that finished, keeps the turn of actors that were
/// refused, and recovers any non-player still holding a turn nobody used.
pub fn cleanup_turns(
    mut commands: Commands,
    mut turns: ResMut<Turns>,
    mut done: MessageReader<ActionDone>,
    mut refused: MessageReader<ActionRefused>,
    holding: Holding,
) {
    let mut handled: Vec<Entity> = Vec::new();
    for d in done.read() {
        if let Ok((e, speed, _)) = holding.get(d.actor) {
            let cost = scaled_cost(d.cost, speed.map(|s| s.0).unwrap_or(100));
            debug!("actor {e:?} finished an action costing {cost}");
            turns.queue.insert_after(e, cost);
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
