//! What the dead leave lying where they fell.
//!
//! Opt-in twice over. Without [`RemainsPlugin`] a death is what it has
//! always been: the actor leaves the world at once and is despawned at
//! the end of the frame. With it, only an actor spawned with
//! [`LeavesRemains`] stays, so a game leaves wrecks behind its robots and
//! nothing behind its ghosts without a second plugin.
//!
//! The remains are the dead actor itself, kept: whatever the game put on
//! that actor, its name, its look, its drop table, its side, is still on
//! the entity, under the same save kind the game already registered for
//! it. What the engine takes off is only what it put on and what means
//! "this is alive and acting": the turn queue, the occupancy index,
//! [`Actor`], `Blocks`, `Health`, and the mind, so a corpse never thinks,
//! never blocks a doorway and can never be struck a second time.
//!
//! Beside the body the engine keeps a twin of the actor as it lived, for a
//! game that stands bodies back up. [`keep_life`] takes it the moment the
//! actor dies, through [`take_twin`], which copies every component onto an
//! entity no ordinary query sees and names, through [`uncopied`], any it
//! could not. [`Life`] links the two, and the twin lasts exactly as long
//! as the body is remains. [`lay_down`] is the one lay-down a death and a
//! continued save both go through. [`revive`], or
//! [`ReviveCommands::revive`] from a system, stands a body back up from
//! its twin, on its own cell or the nearest free one within
//! [`STANDING_ROOM`], and writes [`Revived`].
//!
//! What the engine will not say is what remains *are*. There is no name,
//! no glyph, no rot timer, no loot, and no answer to whether they can be
//! searched, stripped, rebuilt or eaten: a game answers all of that from
//! its own components on the same entity, reacting to [`RemainsLeft`].
//! Whose they were is the one thing the engine knows, because sides are
//! its own, and it is the one thing it tells a mind, in
//! [`PropView`](rl_rules::ai::PropView): a body is a
//! [`Prop`](crate::props::Prop), so a mind that walks to wrecks and a mind
//! that walks to crates read one list, and every panel lists both the same
//! way. What it is called comes from [`RemainsNaming`], the one place the
//! wording lives.
//!
//! The engine never removes remains of its own accord. A game that wants a
//! body to fade despawns it, because how long the dead linger is a rule
//! about a world, not about an engine.

use bevy::ecs::component::ComponentId;
use bevy::ecs::entity::EntityCloner;
use bevy::ecs::entity_disabling::Disabled;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use rl_core::Point;

use crate::combat::{Dead, DeathEvent, Health};
use crate::components::{Actor, Blocks, MyTurn, Position, Viewshed};
use crate::items::{Equipped, Inventory};
use crate::minds::{Mind, Perception};
use crate::places::{MapId, OnMap};
use crate::turn::Occupancy;
use crate::world::WorldMap;

/// Spawned on an actor whose death should leave something behind.
///
/// The per-actor half of the opt-in: a death without it is despawned as
/// it always was, even with [`RemainsPlugin`] added. A game puts it on
/// whatever it wants bodies from, and leaves it off the rest.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct LeavesRemains;

/// What is left of an actor that died, on the entity that was the actor.
///
/// Carries when it died and who got the credit, which is what the engine
/// can know without a word about the world. Everything else a game wants
/// to know about a body it holds in its own components, on this same
/// entity.
#[derive(Component, Debug, Clone, Copy)]
#[component(on_remove = end_life)]
pub struct Remains {
    /// What the clock read when it died.
    pub since: u32,
    /// Who killed it, when anyone did and it is still in the world.
    pub credit: Option<Entity>,
}

/// The living actor as it was the moment it died, kept on a twin no
/// query sees, for a game to stand it back up from.
///
/// On the body rather than in [`Remains`], because the copy is taken the
/// pass the actor dies, before it is laid down, and a body despawned in
/// between must still take its twin with it. Its hook is that guarantee:
/// whatever takes this off the body, or despawns the body, despawns the
/// twin, so no path through the engine or a game leaves one behind.
#[derive(Component, Debug, Clone, Copy)]
#[component(on_remove = despawn_twin)]
pub struct Life(pub Entity);

fn despawn_twin(mut world: DeferredWorld, ctx: HookContext) {
    if let Some(twin) = world.get::<Life>(ctx.entity).map(|l| l.0) {
        world.commands().entity(twin).try_despawn();
    }
}

/// A body that stops being remains, however, stops keeping a life to
/// return to.
fn end_life(mut world: DeferredWorld, ctx: HookContext) {
    world.commands().entity(ctx.entity).try_remove::<Life>();
}

/// What `from` has that `to` does not, by name: the components a copy
/// from one to the other could not carry.
///
/// Bevy copies a component only if it is `Clone` and skips one that is
/// not without a word, which would leave a revived actor quietly short of
/// something. Named rather than counted, so the report says what to fix.
pub fn uncopied(world: &World, from: Entity, to: Entity) -> Vec<String> {
    let copied: Vec<ComponentId> = world.entity(to).archetype().components().to_vec();
    world
        .entity(from)
        .archetype()
        .components()
        .iter()
        .filter(|c| !copied.contains(c))
        .filter_map(|c| world.components().get_name(*c))
        .map(|name| name.to_string())
        .collect()
}

/// Copies `entity`, as it is this moment, onto a twin that is `Disabled`,
/// and links it with [`Life`].
///
/// Every component is copied and none is named, so one added to the
/// engine or to a game next year comes back from a revival without anyone
/// remembering to list it. A second call on a body that already has a twin
/// does nothing: a death takes one, and laying the body down afterwards
/// must not take a second of what is by then a body.
pub fn take_twin(world: &mut World, entity: Entity) {
    if world.get_entity(entity).is_err() || world.get::<Life>(entity).is_some() {
        return;
    }
    let twin = world.spawn(Disabled).id();
    EntityCloner::build_opt_out(world).clone_entity(entity, twin);
    let lost = uncopied(world, entity, twin);
    if !lost.is_empty() {
        error!("{} cannot come back to life: derive `Clone` on it", lost.join(", "));
    }
    world.entity_mut(entity).insert(Life(twin));
}

/// Keeps a twin of every actor that will leave remains, the moment it dies.
///
/// In `ResolveSet::Damage` straight after `apply_damage`, which is where
/// the death is written: before `TurnSet::React`, where a game answers a
/// death and may take its own components off, and before
/// `CleanupSet::Remove`, where the engine takes the actor out of the world.
/// The same test as [`leave_remains`] decides who, so every body has a
/// twin and nothing else does.
pub fn keep_life(mut commands: Commands, mut deaths: MessageReader<DeathEvent>, leaves: Query<(), With<LeavesRemains>>) {
    for death in deaths.read() {
        if death.was_player || leaves.get(death.entity).is_err() {
            continue;
        }
        let entity = death.entity;
        commands.queue(move |world: &mut World| take_twin(world, entity));
    }
}

/// A body was stood back up, this pass, by [`revive`].
///
/// For a game to say so, or to take away what it wants a revived actor to
/// have forgotten; the engine itself restores the actor as it died.
#[derive(Message, Debug, Clone, Copy)]
pub struct Revived {
    /// The actor, which is the entity that was the body.
    pub entity: Entity,
}

/// How far from its body a revived actor may stand when something stands
/// on the body: far enough to find room in a crowd, near enough that it
/// is plainly the same one getting up.
pub const STANDING_ROOM: i32 = 2;

/// Stands `body` back up with `health`, and says whether it could.
///
/// The body is made its twin again (see [`Life`]): every component the
/// twin has is put back with the twin's value, which is everything death
/// took off and everything a body is dressed in over the actor's own,
/// its name, its look, a game's value set on a body, named nowhere; and
/// every component the body gained as a body is taken off, `Remains` and
/// `Prop` and whatever the game added. Only what the world did to the
/// body stays the body's: where it lies, and what it carries. The twin's
/// bag lists items that may since be anywhere, so a looted body comes
/// back without what was taken, one whose bag was taken away comes back
/// with none, and a bag it gained as a body is emptied onto the floor
/// before it goes. In a game with items, death has already let fall
/// everything the actor carried and wore, so a revived actor stands up
/// with nothing, and what its gear lent is folded again from what it
/// wears now; the rules for a bag are for one a game kept or put on the
/// body. `MyTurn` is never put back; the turn queue deals it turns again
/// when it is admitted. An actor still dying is refused: it has a twin
/// from the pass it died in, but it is not remains until it is laid down.
///
/// Refused, with a warning, for anything that is not remains with a twin,
/// and for a body with someone on it and no free cell within
/// [`STANDING_ROOM`].
pub fn revive(world: &mut World, body: Entity, health: i32) -> bool {
    // `Life` goes on in the pass an actor dies and `Remains` only once it is
    // laid down, so an actor still dying has a twin and is not yet a body:
    // standing it up then would be undone by its own death a moment later.
    let Some(twin) = world.get::<Life>(body).filter(|_| world.get::<Remains>(body).is_some()).map(|l| l.0) else {
        warn!("{body:?} cannot be revived: it is not remains, or `RemainsPlugin` kept no twin of it");
        return false;
    };
    let Some(at) = standing_room(world, body) else {
        warn!("{body:?} cannot be revived: something stands on it and nothing within {STANDING_ROOM} cells is free");
        return false;
    };
    // What the world did to the body, which stays the body's.
    let the_bodys = [
        world.component_id::<Disabled>(),
        world.component_id::<Inventory>(),
        world.component_id::<Equipped>(),
        world.component_id::<MyTurn>(),
        world.component_id::<Position>(),
        world.component_id::<OnMap>(),
    ];
    let body_has: Vec<ComponentId> = world.entity(body).archetype().components().to_vec();
    let twin_has: Vec<ComponentId> = world.entity(twin).archetype().components().to_vec();
    let restored: Vec<ComponentId> = twin_has.iter().copied().filter(|c| !the_bodys.contains(&Some(*c))).collect();
    let gained: Vec<ComponentId> = body_has.iter().copied().filter(|c| !twin_has.contains(c)).collect();
    if world.component_id::<Inventory>().is_some_and(|bag| gained.contains(&bag)) {
        spill(world, body, at);
    }
    EntityCloner::build_opt_in(world).allow_by_ids(restored).clone_entity(twin, body);
    let max = world.get::<Health>(twin).map_or(health.max(1), |h| h.max);
    let mut stood = world.entity_mut(body);
    stood.remove_by_ids(&gained).insert((Health { current: health.clamp(1, max), max }, Position(at)));
    if let Some(mut sight) = stood.get_mut::<Viewshed>() {
        sight.dirty = true;
    }
    // What its gear lends is folded from what it wears, and the twin's
    // stats hold the bonuses of everything worn at death; what it wears now
    // is the body's, so the fold runs again from that.
    if let Some(mut worn) = stood.get_mut::<Equipped>() {
        worn.set_changed();
    }
    world.write_message(Revived { entity: body });
    true
}

/// Where a revived body stands: where it lies, or, when something now
/// stands there, the nearest free cell, ring by ring in reading order so
/// the same world always gives the same cell.
fn standing_room(world: &World, body: Entity) -> Option<Point> {
    let at = world.get::<Position>(body)?.0;
    let occupancy = world.resource::<Occupancy>();
    let on = world.get::<OnMap>(body).map(|m| m.0).unwrap_or(MapId::SURFACE);
    if on != occupancy.current() || !occupancy.is_occupied(at) {
        return Some(at);
    }
    let map = world.resource::<WorldMap>();
    (1..=STANDING_ROOM).find_map(|r| {
        (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
            .filter(|(dx, dy)| dx.abs().max(dy.abs()) == r)
            .map(|(dx, dy)| at.offset(dx, dy))
            .find(|p| map.is_walkable(*p) && !occupancy.is_occupied(*p))
    })
}

/// Empties a bag the body gained as a body onto the floor at `at`, so a
/// revival never destroys an item.
fn spill(world: &mut World, body: Entity, at: Point) {
    let items = world.get::<Inventory>(body).map(|b| b.items.clone()).unwrap_or_default();
    let map = world.get::<OnMap>(body).map(|m| m.0).unwrap_or(MapId::SURFACE);
    for item in items {
        if let Ok(mut thing) = world.get_entity_mut(item) {
            thing.insert((Position(at), OnMap(map)));
        }
    }
}

/// Stands a body up from inside a system, at the next sync point.
pub trait ReviveCommands {
    /// Queues [`revive`] of `body` with `health`.
    fn revive(&mut self, body: Entity, health: i32);
}

impl ReviveCommands for Commands<'_, '_> {
    fn revive(&mut self, body: Entity, health: i32) {
        self.queue(move |world: &mut World| {
            revive(world, body, health);
        });
    }
}

/// Lays `entity` down as remains: takes a twin if it has none, takes the
/// life off, makes it a prop and names it as what is left of it.
///
/// One function, used by a death and by a save continued, so a body from
/// a save has a twin as surely as one that just fell. On the second path
/// the twin is the living thing the game's own record just spawned, which
/// is all a save knows.
pub fn lay_down(world: &mut World, entity: Entity, since: u32, credit: Option<Entity>) {
    take_twin(world, entity);
    let Ok(mut body) = world.get_entity_mut(entity) else { return };
    body.remove::<WasLiving>().insert((crate::props::Prop, Remains { since, credit }));
    name_as_remains(world, entity);
}

/// An actor became remains, this pass.
///
/// The entity is the one that died, so a game reacting to this in
/// [`TurnSet::React`](crate::plugin::TurnSet::React) reads whatever it
/// spawned the actor with and adds whatever a body of its own needs.
#[derive(Message, Debug, Clone, Copy)]
pub struct RemainsLeft {
    /// The remains, which is the entity that was the actor.
    pub entity: Entity,
    /// Where they lie.
    pub at: Point,
}

/// How a body is named, once it is one.
///
/// A template with `{what}` standing for whatever the actor was called:
/// `"{what} remains"` makes a line droid's wreck a "line droid remains",
/// and a dungeon passes `"{what} corpse"`. The engine cannot invent the
/// wording and will not try; this is the one place a game says it, and
/// every panel and the log read the name that comes out of it.
///
/// A game that wants a name of its own per kind sets `Name` itself,
/// reacting to [`RemainsLeft`], and then owns setting it again when a save
/// is continued.
#[derive(Resource, Debug, Clone)]
pub struct RemainsNaming(pub String);

impl Default for RemainsNaming {
    fn default() -> Self {
        Self("{what} remains".into())
    }
}

impl RemainsNaming {
    /// What a body called `what` is called.
    pub fn name_for(&self, what: &str) -> String {
        self.0.replace("{what}", what)
    }
}

/// Names `entity` as what is left of what it was, if it is named at all.
///
/// Used twice: when an actor becomes remains, and when a save lays one
/// back down, because a game's record of a monster says what it was and
/// not what became of it.
pub fn name_as_remains(world: &mut World, entity: Entity) {
    let naming = world.get_resource::<RemainsNaming>().cloned().unwrap_or_default();
    let Some(was) = world.get::<Name>(entity).map(|n| n.as_str().to_string()) else { return };
    let becomes = naming.name_for(&was);
    if becomes != was {
        world.entity_mut(entity).insert(Name::new(becomes));
    }
}

/// What an actor stops being when it becomes remains.
///
/// One list, used twice: by [`leave_remains`] when it dies, and by the
/// save when a run is continued and a game has just spawned the thing
/// alive again from its own record of it. A body holds no turn (no
/// [`Actor`]), stands in nobody's way (no `Blocks`), thinks nothing (no
/// mind, no sight) and has nothing left to lose (no [`Health`]), so
/// nothing in the engine can deal it a turn or kill it twice. `Dead`
/// comes off with them, which is what keeps
/// [`bury_the_dead`](crate::combat::bury_the_dead) from despawning it at
/// the end of the frame.
///
/// What it noticed and what it heard come off too, and that is not
/// tidiness: a watcher is anything that carries `Notice` and is not
/// `Dead`, and remains are not `Dead` by design, so a body left with its
/// `Notice` went on watching the player, who stayed marked as seen with
/// every enemy on the deck dead. Its [`Post`](crate::minds::Post) comes
/// off as well, since a body keeps no cell and walks back to none.
pub type WasLiving = (
    Dead,
    Actor,
    Blocks,
    Health,
    Mind,
    Perception,
    crate::components::Viewshed,
    crate::stealth::Notice,
    crate::stealth::Aware,
    crate::noise::Hearing,
    crate::noise::Heard,
    crate::minds::Post,
);

/// Keeps the dead that leave remains, and takes the life off them.
///
/// In [`CleanupSet::Remove`](crate::plugin::CleanupSet::Remove) after
/// [`process_deaths`](crate::combat::process_deaths), which is where an
/// actor loses its turn, its cell in the index and its [`Position`]: the
/// position is put back here, at the cell it died on, because remains lie
/// where the actor fell rather than where a game would have to remember
/// it fell. Taking [`Dead`] off is what keeps
/// [`bury_the_dead`](crate::combat::bury_the_dead) from despawning it at
/// the end of the frame.
///
/// [`Health`] comes off rather than being left at zero: a body is not a
/// defender, and left with health it would answer a query the damage pass
/// makes and could be killed twice. A game that wants a wreck something
/// can shoot apart gives the wreck health of its own, which is a rule
/// about that game's wrecks.
pub fn leave_remains(
    mut commands: Commands,
    mut deaths: MessageReader<DeathEvent>,
    mut left: MessageWriter<RemainsLeft>,
    turns: Res<crate::turn::Turns>,
    leaves: Query<(), With<LeavesRemains>>,
) {
    for death in deaths.read() {
        if death.was_player || leaves.get(death.entity).is_err() {
            continue;
        }
        // A body is a prop: something standing in a cell that is neither an
        // actor nor an item, which is what lets a mind walk to one, a game
        // offer a verb on one, and every panel list one, with no second
        // mechanism for bodies. It lies where it fell, which death took off.
        commands.entity(death.entity).insert(Position(death.at));
        let (entity, since, credit) = (death.entity, turns.now(), death.credit);
        commands.queue(move |world: &mut World| lay_down(world, entity, since, credit));
        left.write(RemainsLeft { entity: death.entity, at: death.at });
    }
}

/// Remains: the dead stay where they fell, for a game to say what that
/// means.
///
/// Opt-in, and opt-in per actor with [`LeavesRemains`]. Needs
/// [`CombatPlugin`](crate::combat::CombatPlugin), since without deaths
/// there is nothing to leave.
///
/// What is left is a [`Prop`](crate::props::Prop), so whatever props can
/// do, a body can: be seen by a mind, offer a verb, be listed in a panel.
/// A game that wants either adds
/// [`PropsPlugin`](crate::props::PropsPlugin) beside this one; without it,
/// a body is simply something named lying on the floor.
pub struct RemainsPlugin;

impl RemainsPlugin {
    /// Sets how a body is named: a template with `{what}` for whatever the
    /// actor was called. The default is `"{what} remains"`.
    ///
    /// Inserted as [`RemainsNaming`], so a game may also replace it later.
    pub fn naming(template: impl Into<String>) -> impl Plugin {
        let template = template.into();
        move |app: &mut App| {
            app.insert_resource(RemainsNaming(template.clone()));
            app.add_plugins(RemainsPlugin);
        }
    }
}

impl Plugin for RemainsPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{CleanupSet, Turn};
        app.add_message::<RemainsLeft>()
            .add_message::<Revived>()
            .init_resource::<RemainsNaming>()
            .add_systems(Turn, keep_life.in_set(crate::plugin::ResolveSet::Damage).after(crate::combat::apply_damage))
            .add_systems(Turn, leave_remains.in_set(CleanupSet::Remove).after(crate::combat::process_deaths));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::combat::CombatPlugin>(app, "RemainsPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, DamageEvent, Faction, Health, Strikes};
    use crate::components::{Player, Viewshed};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::testing::{self, Sides};
    use bevy::ecs::entity_disabling::Disabled;
    use rl_core::DiceRoll;
    use rl_rules::Hit;

    /// A game's own component, which the game's own death takes off.
    #[derive(Component, Debug, Clone, PartialEq)]
    struct Patrol(i32);

    /// The game's answer to a death: its patrol ends.
    fn end_patrols(mut commands: Commands, mut deaths: MessageReader<DeathEvent>) {
        for death in deaths.read() {
            commands.entity(death.entity).remove::<Patrol>();
        }
    }

    /// A component a game forgot to make `Clone`.
    #[derive(Component)]
    struct Unclonable;

    /// Every twin in the world, which a query sees only by naming `Disabled`.
    fn twins(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<Disabled>>().iter(world).count()
    }

    /// An arena with combat and remains, and a world to stand in.
    fn arena() -> (App, Point, Sides) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, RemainsPlugin, crate::world::StreamingPlugin));
        let start = testing::surface(&mut app);
        let sides = testing::two_sides(&mut app);
        (app, start, sides)
    }

    /// Spawns something that can be killed, `at`, leaving remains or not.
    fn victim(app: &mut App, at: Point, sides: Sides, leaves: bool) -> Entity {
        let mut spawn = app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(4), Faction(sides.theirs)));
        if leaves {
            spawn.insert(LeavesRemains);
        }
        spawn.id()
    }

    /// Kills `who` outright and runs the frame its death is cleaned up in.
    fn kill(app: &mut App, who: Entity, sides: Sides) {
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        let hit = Hit::from_source(None, sides.kind, 99);
        app.world_mut().write_message(DamageEvent::new(who, hit));
        app.update();
        app.update();
    }

    /// The whole of the opt-in, in one property: the marker decides, and
    /// nothing else about the death changes.
    #[test]
    fn a_death_leaves_remains_where_it_fell_only_when_the_actor_was_marked_to_leave_them() {
        let (mut app, start, sides) = arena();
        let marked = victim(&mut app, start.offset(2, 0), sides, true);
        let unmarked = victim(&mut app, start.offset(3, 0), sides, false);
        kill(&mut app, marked, sides);
        kill(&mut app, unmarked, sides);

        let world = app.world();
        assert!(world.get_entity(marked).is_ok(), "the marked one was kept");
        assert_eq!(world.get::<Position>(marked).map(|p| p.0), Some(start.offset(2, 0)), "the remains lie where it fell");
        assert!(world.get::<Remains>(marked).is_some(), "and they are remains");
        assert!(world.get_entity(unmarked).is_err(), "the unmarked one was buried as it always was");
    }

    /// A body is not an actor: it holds no turn, blocks no doorway, and
    /// cannot be killed a second time.
    #[test]
    fn remains_take_no_turns_block_nothing_and_cannot_be_struck_again() {
        let (mut app, start, sides) = arena();
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        kill(&mut app, dead, sides);

        assert!(app.world().get::<Actor>(dead).is_none(), "no longer an actor");
        assert!(app.world().get::<Blocks>(dead).is_none(), "no longer in the way");
        assert!(app.world().get::<Health>(dead).is_none(), "nothing left to lose");
        assert!(app.world().get::<Dead>(dead).is_none(), "not waiting to be buried");
        assert!(!app.world().resource::<crate::turn::Turns>().contains(dead), "not in the queue");

        // A second blow on the same entity finds no defender, so no second
        // death is written and the remains are still there.
        let hit = Hit::from_source(None, sides.kind, 99);
        app.world_mut().write_message(DamageEvent::new(dead, hit));
        app.update();
        assert!(app.world().get_entity(dead).is_ok(), "the remains survived a blow aimed at them");
    }

    /// A body watches nobody. A watcher is anything that carries
    /// `Notice` and is not `Dead`, and remains are not `Dead` on purpose,
    /// so a body left with what it noticed went on seeing the player: a
    /// deck with every droid dead still read as a deck the commando was
    /// seen on.
    #[test]
    fn remains_watch_nobody_and_hear_nothing() {
        let (mut app, start, sides) = arena();
        app.add_plugins(crate::stealth::StealthPlugin);
        let player = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(sides.ours), crate::stealth::Stealth::default()))
            .id();
        let watcher = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(watcher).insert((crate::stealth::Notice::default(), crate::noise::Hearing(rl_rules::ai::HearingStats::default())));
        app.update();
        app.update();

        {
            let world = app.world_mut();
            let mut watchers = world.query_filtered::<Entity, (With<crate::stealth::Notice>, Without<Dead>)>();
            assert!(watchers.iter(world).any(|e| e == watcher), "it watches while it lives");
        }
        kill(&mut app, watcher, sides);
        let world = app.world_mut();
        let mut watchers = world.query_filtered::<Entity, (With<crate::stealth::Notice>, Without<Dead>)>();
        assert!(!watchers.iter(world).any(|e| e == watcher), "and watches nobody once it is a body");
        assert!(app.world().get::<crate::noise::Hearing>(watcher).is_none(), "nor hears anything");
        let _ = player;
    }

    /// The engine keeps the entity rather than spawning one, so whatever
    /// the game spawned the actor with is still there to be read.
    #[test]
    fn remains_keep_the_components_the_game_gave_the_actor() {
        #[derive(Component, Debug, Clone, Copy, PartialEq)]
        struct Kind(u32);

        let (mut app, start, sides) = arena();
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(dead).insert((Kind(7), Strikes(vec![(sides.kind, DiceRoll::flat(1))])));
        kill(&mut app, dead, sides);

        assert_eq!(app.world().get::<Kind>(dead), Some(&Kind(7)), "the game's own component came through untouched");
        assert_eq!(app.world().get::<Faction>(dead).map(|f| f.0), Some(sides.theirs), "and whose it was, which is what a mind is told");
    }

    /// Two deaths on one cell are two bodies: the engine merges nothing.
    #[test]
    fn two_deaths_on_one_cell_leave_two_remains() {
        let (mut app, start, sides) = arena();
        let cell = start.offset(2, 0);
        let first = victim(&mut app, cell, sides, true);
        kill(&mut app, first, sides);
        let second = victim(&mut app, cell, sides, true);
        kill(&mut app, second, sides);

        let mut lying = app.world_mut().query_filtered::<&Position, With<Remains>>();
        let at: Vec<Point> = lying.iter(app.world()).map(|p| p.0).collect();
        assert_eq!(at, vec![cell, cell], "both are still lying there");
    }

    /// A mind is told what it can see and no more: bodies are not a map
    /// of every death on the level. Told as props, because that is what a
    /// body is.
    #[test]
    fn a_mind_is_told_the_bodies_it_can_see_and_whose_they_were() {
        use crate::minds::{Mind, MindsPlugin, Perception, Thinking};
        use crate::plugin::{PerceiveSet, Turn};
        use crate::turn::{Intent, Wait};
        use std::sync::Arc;

        /// What the mind holding the turn was told about what stands about.
        #[derive(Resource, Default)]
        struct Told(Vec<rl_rules::ai::PropView<Entity>>);

        /// Reads the snapshot after the contributor filled it, which is
        /// what a game's own tactic would see.
        fn record(thinking: Res<Thinking>, mut told: ResMut<Told>) {
            if let Some(snapshot) = thinking.snapshot() {
                told.0 = snapshot.props.clone();
            }
        }

        let (mut app, start, sides) = arena();
        // Props are what a body is seen as, so the contributor is theirs.
        app.add_plugins((MindsPlugin, crate::props::PropsPlugin))
            .init_resource::<Told>()
            .add_systems(Turn, record.in_set(PerceiveSet::Annotate).after(crate::props::perceive_props));
        let near = victim(&mut app, start.offset(2, 0), sides, true);
        let far = victim(&mut app, start.offset(12, 0), sides, true);
        kill(&mut app, near, sides);
        kill(&mut app, far, sides);

        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), Health::full(30), Faction(sides.ours))).id();
        // A mind of its own beside the player, with nothing it wants to do.
        app.world_mut().spawn((
            Actor,
            Blocks,
            Position(start.offset(1, 1)),
            Health::full(5),
            Faction(sides.theirs),
            Perception(6),
            Mind(Arc::new(rl_rules::ai::Brain::new())),
        ));
        app.update();
        app.world_mut().write_message(Intent::new(player, Wait));
        app.update();

        let told = app.world().resource::<Told>().0.clone();
        assert_eq!(told.len(), 1, "one body in sight, one too far off: {told:?}");
        assert_eq!(told[0].id, near, "the one it could see");
        assert_eq!(told[0].side, Some(sides.theirs), "and whose it was, which is all the engine says");
        assert!(!told.iter().any(|r| r.id == far), "nothing of the one beyond its sight");
    }

    /// A body is named as what is left of what it was, from the one place
    /// the wording lives, so the nearby rail says "line droid remains"
    /// rather than listing a wreck as though it were still walking.
    #[test]
    fn a_body_is_named_as_what_is_left_of_what_it_was() {
        let (mut app, start, sides) = arena();
        let droid = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(droid).insert(Name::new("line droid"));
        kill(&mut app, droid, sides);
        assert_eq!(app.world().get::<Name>(droid).map(|n| n.as_str().to_string()), Some("line droid remains".into()));
        assert!(app.world().get::<crate::props::Prop>(droid).is_some(), "and it is a prop, like anything else standing in a cell");

        // A game says it its own way, and the engine says nothing of its own.
        let (mut app, start, sides) = arena();
        app.insert_resource(RemainsNaming("the corpse of a {what}".into()));
        let rat = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(rat).insert(Name::new("rat"));
        kill(&mut app, rat, sides);
        assert_eq!(app.world().get::<Name>(rat).map(|n| n.as_str().to_string()), Some("the corpse of a rat".into()));
    }

    /// The player is the game's to bury: a run ends on a death, and a
    /// corpse the game may still want to draw is not taken out from
    /// under it.
    #[test]
    fn the_players_death_leaves_the_player_to_the_game_marked_or_not() {
        let (mut app, start, sides) = arena();
        let player =
            app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(4), Faction(sides.ours), LeavesRemains)).id();
        kill(&mut app, player, sides);
        assert!(app.world().get::<Remains>(player).is_none(), "the engine left the player alone");
        assert!(app.world().get::<Health>(player).is_some(), "including its health, which the game may still show");
    }

    /// The copy is taken the moment the actor dies, before anything takes
    /// anything off it: a component the game's own death system removes is
    /// on the twin, and no query that does not ask for disabled entities
    /// ever sees the twin.
    #[test]
    fn a_dying_actor_is_kept_whole_on_a_twin_no_query_sees() {
        let (mut app, start, sides) = arena();
        app.add_systems(crate::plugin::Turn, end_patrols.in_set(crate::plugin::TurnSet::React));
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(dead).insert(Patrol(3));
        kill(&mut app, dead, sides);

        let twin = app.world().get::<Life>(dead).expect("the body keeps a life to return to").0;
        let world = app.world();
        assert_eq!(world.get::<Patrol>(twin), Some(&Patrol(3)), "what the game's death took off is on the twin");
        assert!(world.get::<Patrol>(dead).is_none(), "and off the body");
        assert!(world.get::<Actor>(twin).is_some() && world.get::<Health>(twin).is_some(), "the twin is the actor as it lived");
        let world = app.world_mut();
        let actors: Vec<Entity> = world.query_filtered::<Entity, With<Actor>>().iter(world).collect();
        assert!(!actors.contains(&twin), "no ordinary query sees the twin");
    }

    /// The twin lives exactly as long as its body is remains: despawning
    /// the body or taking `Remains` off it takes the twin too.
    #[test]
    fn no_twin_outlives_its_body_however_the_body_stops_being_remains() {
        let (mut app, start, sides) = arena();
        let despawned = victim(&mut app, start.offset(2, 0), sides, true);
        let stripped = victim(&mut app, start.offset(3, 0), sides, true);
        kill(&mut app, despawned, sides);
        kill(&mut app, stripped, sides);
        assert_eq!(twins(&mut app), 2, "one twin a body");

        app.world_mut().despawn(despawned);
        app.update();
        assert_eq!(twins(&mut app), 1, "a body despawned takes its twin");

        app.world_mut().entity_mut(stripped).remove::<Remains>();
        app.update();
        assert_eq!(twins(&mut app), 0, "and one that stops being remains does too");
        assert!(app.world().get::<Life>(stripped).is_none());
    }

    /// Bevy copies only what is `Clone` and skips the rest without a word,
    /// so the engine compares the two and names what did not come across.
    #[test]
    fn a_component_that_cannot_be_copied_is_named() {
        let (mut app, start, sides) = arena();
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(dead).insert(Unclonable);
        kill(&mut app, dead, sides);
        let twin = app.world().get::<Life>(dead).unwrap().0;
        let lost = uncopied(app.world(), dead, twin);
        assert!(lost.iter().any(|n| n.contains("Unclonable")), "{lost:?}");
        assert!(!lost.iter().any(|n| n.contains("Health")), "and nothing that was copied: {lost:?}");
    }

    /// A player at `start`: what streams the map in around it, and what the
    /// turns wait on, so a test that needs walkable ground or a turn dealt
    /// has both.
    fn watcher(app: &mut App, start: Point, sides: Sides) -> Entity {
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(sides.ours))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        player
    }

    /// The player waits once, so the turns run a round.
    fn take_a_turn(app: &mut App, player: Entity) {
        if app.world().get::<crate::components::MyTurn>(player).is_some() {
            app.world_mut().write_message(crate::turn::Intent::new(player, crate::turn::Wait));
        }
        app.update();
        app.update();
    }

    /// Stands `body` up with `health`, the way a game does, and runs the
    /// frame that admits it.
    fn stand_up(app: &mut App, body: Entity, health: i32) -> bool {
        let stood = revive(app.world_mut(), body, health);
        app.update();
        stood
    }

    /// A revived actor has exactly the components it had when it died,
    /// less none and plus none, whoever took what off in between.
    #[test]
    fn a_revived_actor_has_exactly_what_it_had_when_it_died() {
        let (mut app, start, sides) = arena();
        app.add_systems(crate::plugin::Turn, end_patrols.in_set(crate::plugin::TurnSet::React));
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(dead).insert((Patrol(3), Name::new("line droid")));
        // Playing, so everything play puts on an actor (its `OnMap`) is on
        // it before the set is taken.
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        let turn_state = app.world().components().component_id::<crate::components::MyTurn>();
        let set = |app: &App| -> Vec<ComponentId> {
            let mut ids: Vec<ComponentId> = app.world().entity(dead).archetype().components().iter().copied().filter(|c| Some(*c) != turn_state).collect();
            ids.sort();
            ids
        };
        let before = set(&app);
        kill(&mut app, dead, sides);
        assert!(stand_up(&mut app, dead, 3));

        assert_eq!(set(&app), before, "the same components it had alive");
        let world = app.world();
        assert_eq!(world.get::<Patrol>(dead), Some(&Patrol(3)), "the game's own, which its death took off, is back");
        assert_eq!(world.get::<Name>(dead).map(|n| n.as_str().to_string()), Some("line droid".into()), "and its own name, not a body's");
        assert_eq!(world.get::<Health>(dead).map(|h| (h.current, h.max)), Some((3, 4)), "with the health it was given");
        assert_eq!(twins(&mut app), 0, "and no twin left behind");
    }

    /// Stood up, it is an actor again in every way that matters: dealt
    /// turns, in the way, able to be hurt, and able to die and leave
    /// remains a second time.
    #[test]
    fn a_revived_actor_is_dealt_turns_blocks_and_can_die_again_leaving_remains_again() {
        let (mut app, start, sides) = arena();
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        kill(&mut app, dead, sides);
        let player = watcher(&mut app, start, sides);
        assert!(stand_up(&mut app, dead, 2));
        take_a_turn(&mut app, player);
        assert!(app.world().resource::<crate::turn::Turns>().contains(dead), "back in the queue");
        assert!(app.world().resource::<crate::turn::Occupancy>().is_occupied(start.offset(2, 0)), "and in the way");
        assert!(app.world().get::<Remains>(dead).is_none() && app.world().get::<crate::props::Prop>(dead).is_none(), "and no longer a body");

        kill(&mut app, dead, sides);
        assert!(app.world().get::<Remains>(dead).is_some(), "it died again, and lies there again");
        assert_eq!(twins(&mut app), 1, "with a twin of its second life");
    }

    /// A looted body comes back without what was taken, worn or carried;
    /// one whose bag was taken away comes back with none; and a bag it
    /// gained as a body is left on the floor rather than lost.
    #[test]
    fn a_revived_body_carries_only_what_it_still_holds_and_no_item_is_in_two_bags() {
        use crate::items::{Equipped, Inventory, Item};
        let (mut app, start, sides) = arena();
        let kept = app.world_mut().spawn(Item).id();
        let looted = app.world_mut().spawn(Item).id();
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        let mut worn = rl_rules::Equipment::with_slot_count(1);
        worn.equip(looted, &rl_rules::EquipShape::in_slot(rl_rules::SlotId::from_raw(0))).unwrap();
        app.world_mut().entity_mut(dead).insert((Inventory { items: vec![kept, looted] }, Equipped(worn)));
        kill(&mut app, dead, sides);
        // Looting, as `resolve_takes` does it.
        app.world_mut().get_mut::<Inventory>(dead).unwrap().remove(looted);
        app.world_mut().get_mut::<Equipped>(dead).unwrap().unequip(looted);
        assert!(stand_up(&mut app, dead, 2));
        assert_eq!(app.world().get::<Inventory>(dead).map(|b| b.items.clone()), Some(vec![kept]), "only what it still held");
        assert!(!app.world().get::<Equipped>(dead).unwrap().contains(looted), "and it wears nothing it lost");

        let bagless = victim(&mut app, start.offset(3, 0), sides, true);
        app.world_mut().entity_mut(bagless).insert(Inventory { items: vec![] });
        kill(&mut app, bagless, sides);
        app.world_mut().entity_mut(bagless).remove::<Inventory>();
        assert!(stand_up(&mut app, bagless, 2));
        assert!(app.world().get::<Inventory>(bagless).is_none(), "a bag taken away is not handed back from the twin");

        let gained = victim(&mut app, start.offset(4, 0), sides, true);
        kill(&mut app, gained, sides);
        let loot = app.world_mut().spawn(Item).id();
        app.world_mut().entity_mut(gained).insert(Inventory { items: vec![loot] });
        assert!(stand_up(&mut app, gained, 2));
        assert!(app.world().get::<Inventory>(gained).is_none(), "a bag gained as a body goes");
        assert_eq!(app.world().get::<Position>(loot).map(|p| p.0), Some(start.offset(4, 0)), "and what was in it is on the floor where it stood");
    }

    /// Something standing on a body when it stands up is not stood on: the
    /// body takes the nearest free cell.
    #[test]
    fn a_body_revived_where_someone_stands_stands_up_beside_them() {
        let (mut app, start, sides) = arena();
        let player = watcher(&mut app, start, sides);
        let at = start.offset(3, 0);
        let dead = victim(&mut app, at, sides, true);
        kill(&mut app, dead, sides);
        let blocker = app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(4), Faction(sides.ours))).id();
        take_a_turn(&mut app, player);
        assert!(stand_up(&mut app, dead, 2));
        let stood = app.world().get::<Position>(dead).unwrap().0;
        assert_ne!(stood, at, "not on top of whoever was there");
        assert!(rl_core::geometry::is_adjacent(stood, at), "but beside them");
        assert_eq!(app.world().get::<Position>(blocker).unwrap().0, at, "who did not move");
    }

    /// A body dressed as something else while it lay there, a look or a
    /// component of the game's own set to a body's value, stands up as the
    /// actor it was: the revived actor has the twin's values, not the
    /// body's, for everything but what it carries and where it lies.
    #[test]
    fn a_revived_actor_is_as_it_was_alive_whatever_its_body_was_dressed_as() {
        let (mut app, start, sides) = arena();
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        app.world_mut().entity_mut(dead).insert(Patrol(3));
        kill(&mut app, dead, sides);
        // A game dressing the body: the same component, a body's value.
        app.world_mut().entity_mut(dead).insert(Patrol(9));
        assert!(stand_up(&mut app, dead, 2));
        assert_eq!(app.world().get::<Patrol>(dead), Some(&Patrol(3)), "the actor's own value, not the body's");
    }

    /// Who died this pass, and what a game's answer got back from trying to
    /// revive each.
    #[derive(Resource, Default)]
    struct Tried(Vec<Entity>, Vec<bool>);

    fn note_the_dying(mut deaths: MessageReader<DeathEvent>, mut tried: ResMut<Tried>) {
        tried.0.extend(deaths.read().map(|d| d.entity));
    }

    /// A game that tries to stand a dying actor straight back up, in the
    /// pass it dies, before it is remains.
    fn revive_the_dying(world: &mut World) {
        let dying = std::mem::take(&mut world.resource_mut::<Tried>().0);
        for entity in dying {
            let stood = revive(world, entity, 2);
            world.resource_mut::<Tried>().1.push(stood);
        }
    }

    /// Something dying is not yet remains, whatever the engine has already
    /// kept of it: it is refused, and lies down as remains like any other.
    #[test]
    fn an_actor_in_the_pass_it_dies_is_not_yet_remains_and_cannot_be_revived() {
        let (mut app, start, sides) = arena();
        app.init_resource::<Tried>().add_systems(crate::plugin::Turn, (note_the_dying, revive_the_dying).chain().in_set(crate::plugin::TurnSet::React));
        watcher(&mut app, start, sides);
        let dead = victim(&mut app, start.offset(2, 0), sides, true);
        kill(&mut app, dead, sides);
        assert_eq!(app.world().resource::<Tried>().1, vec![false], "refused while it was dying");
        assert!(app.world().get::<Remains>(dead).is_some(), "and it lies down as remains");
    }

    /// In a game with items, death lets fall what the actor carried, so a
    /// revived actor stands up without it, and without what it lent: a
    /// bonus from armor it no longer wears does not come back with it.
    #[test]
    fn a_revived_actor_stands_up_without_what_death_dropped_or_the_bonus_it_gave() {
        use crate::items::{Bestows, Equipped, Inventory, Item, ItemsPlugin};
        use crate::status::StatBlock;
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, RemainsPlugin, crate::world::StreamingPlugin, ItemsPlugin));
        let start = testing::surface(&mut app);
        let sides = testing::two_sides(&mut app);
        let player = watcher(&mut app, start, sides);
        let stat = rl_rules::StatId::from_raw(0);
        let plate = app.world_mut().spawn((Item, Bestows(vec![(stat, rl_rules::Op::Add(2))]))).id();
        let at = start.offset(2, 0);
        let dead = victim(&mut app, at, sides, true);
        let mut worn = rl_rules::Equipment::with_slot_count(1);
        worn.equip(plate, &rl_rules::EquipShape::in_slot(rl_rules::SlotId::from_raw(0))).unwrap();
        app.world_mut().entity_mut(dead).insert((Inventory { items: vec![plate] }, Equipped(worn), StatBlock(rl_rules::Stats::new())));
        let from_gear = |app: &App| app.world().get::<StatBlock>(dead).map_or(0, |s| s.0.modifiers().iter().filter(|m| m.source.is_item()).count());
        take_a_turn(&mut app, player);
        assert_eq!(from_gear(&app), 1, "worn, the plate lends its bonus");

        kill(&mut app, dead, sides);
        assert!(stand_up(&mut app, dead, 2));
        take_a_turn(&mut app, player);
        assert_eq!(from_gear(&app), 0, "stood up without the plate, and without its bonus");
        assert!(app.world().get::<Inventory>(dead).is_none_or(|b| b.items.is_empty()), "carrying nothing");
        assert_eq!(app.world().get::<Position>(plate).map(|p| p.0), Some(at), "the plate lies where it fell");
    }

    /// Only remains can be stood up.
    #[test]
    fn nothing_but_remains_can_be_revived() {
        let (mut app, start, sides) = arena();
        let alive = victim(&mut app, start.offset(2, 0), sides, true);
        assert!(!revive(app.world_mut(), alive, 3), "a living actor has no life to return to");
    }
}
