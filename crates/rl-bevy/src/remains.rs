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
//! continued save both go through.
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
use crate::components::{Actor, Blocks, Position};
use crate::minds::{Mind, Perception};

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
}
