//! Wiring: the sets, the plugins, and a headless app for tests.

use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;

use crate::components::{MyTurn, Player};
use crate::knowledge::Knowledge;
use crate::state::EngineState;
use crate::turn::{Acting, ActionDone, ActionRefused, AddAction, Occupancy, TurnEnd, Turns};
use crate::world::{WorldMap, WorldSettings};
use crate::{combat, places, turn};

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
    /// Requeue and recover.
    Cleanup,
}

/// The most passes one frame may run. A frame that hits it has a queue that
/// never reaches the player; the rest waits for the next frame rather than
/// stalling the window.
const MAX_PASSES: usize = 512;

/// Runs [`Turn`] passes until the player holds a turn or a pass changes
/// nothing.
///
/// The player holding a turn means the game's input system gets the next
/// frame; a pass that neither dealt, advanced nor requeued means the queue
/// is idle. Both leave at least one pass run, so an [`Intent`](crate::turn::Intent) written in
/// [`EngineSet::Input`] is always resolved in the same frame.
pub fn run_turns(world: &mut World) {
    for pass in 0..MAX_PASSES {
        world.resource_mut::<Turns>().progress = false;
        world.run_schedule(Turn);
        let player_holds = world.query_filtered::<(), (With<Player>, With<MyTurn>)>().iter(world).next().is_some();
        if player_holds || !world.resource::<Turns>().progress {
            return;
        }
        if pass + 1 == MAX_PASSES {
            debug!("turn loop hit {MAX_PASSES} passes in one frame; the rest waits");
        }
    }
}

/// The engine's plugins, gated on [`EngineState::Playing`].
///
/// Insert [`WorldRes`](crate::world::WorldRes), [`ChunkRulesRes`](crate::world::ChunkRulesRes)
/// and a [`WorldMap`] before entering `Playing`; the streaming system does
/// the rest.
/// The engine's core, and the only plugin every game needs.
///
/// The state, the system sets, the turn schedule and the loop that runs
/// it, the clock, the occupancy index, the map and its places, and the
/// three actions that need nothing else: step, wait, and go through what
/// stands here. Everything else is a plugin of its own, and a game adds
/// the ones it wants: nothing turns itself on because a resource happens
/// to exist.
pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<EngineState>()
            .init_resource::<Turns>()
            .init_resource::<Occupancy>()
            .init_resource::<Acting>()
            .init_resource::<WorldSettings>()
            .init_resource::<Knowledge>()
            .init_resource::<combat::FlowFields>()
            .add_message::<ActionDone>()
            .add_message::<ActionRefused>()
            .add_message::<TurnEnd>()
            .add_message::<places::WarpRequest>()
            .add_message::<places::MapChanged>()
            .add_message::<places::PlaceEntered>()
            .init_schedule(Turn)
            .configure_sets(
                Update,
                (EngineSet::Stream, EngineSet::Input, EngineSet::Turns, EngineSet::Light, EngineSet::Fov, EngineSet::Present)
                    .chain()
                    .run_if(in_state(EngineState::Playing))
                    .run_if(resource_exists::<WorldMap>),
            )
            .configure_sets(Update, (PresentSet::Narrate, PresentSet::Map, PresentSet::Chrome, PresentSet::Overlay).chain().in_set(EngineSet::Present))
            .configure_sets(Turn, (TurnSet::Schedule, TurnSet::Decide, TurnSet::Resolve, TurnSet::Sweep, TurnSet::React, TurnSet::Cleanup).chain())
            .configure_sets(Turn, (ResolveSet::Act, ResolveSet::Effects, ResolveSet::Damage).chain().in_set(TurnSet::Resolve))
            .add_action::<turn::Step>()
            .add_action::<turn::Wait>()
            .add_action::<places::GoThrough>()
            .add_systems(Update, run_turns.in_set(EngineSet::Turns))
            .add_systems(Turn, (turn::start_pass, places::tag_new_positions, turn::admit_new_actors, turn::schedule).chain().in_set(TurnSet::Schedule))
            .add_systems(Turn, (turn::resolve_moves, turn::resolve_waits, places::resolve_warps).chain().in_set(ResolveSet::Act))
            .add_systems(Turn, (turn::cleanup_turns, turn::forget_removed_blockers).chain().in_set(TurnSet::Cleanup));
    }
}

/// The stages of [`TurnSet::Resolve`], in order.
///
/// Actions first, then what ticks because a turn passed, then the damage
/// both of them produced. Named because the systems that fill them come
/// from different plugins, which cannot chain themselves together.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolveSet {
    /// Actions: a step, a strike, a drink, a door.
    Act,
    /// What a turn costs whoever is standing in it: statuses, fuel.
    Effects,
    /// The damage the pass produced, applied once.
    Damage,
}

/// Asserts, when play begins, that a plugin has what it cannot work
/// without. A missing rule table is a mistake in the game's setup, and it
/// says so rather than quietly doing nothing all run.
pub(crate) fn needs<R: Resource>(plugin: &'static str) -> impl Fn(Option<Res<R>>) {
    move |res: Option<Res<R>>| {
        assert!(res.is_some(), "{plugin} needs {} inserted before EngineState::Playing", std::any::type_name::<R>());
    }
}

/// Asserts that a plugin this one depends on was added too.
pub(crate) fn depends_on<P: Plugin>(app: &App, plugin: &'static str) {
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
    use crate::world::{ChunkRulesRes, WorldRes};
    use rl_core::{Direction, Point, RunSeed};
    use rl_grid::{TileId, TileProps, TileRegistry};
    use rl_mapgen::Chain;
    use rl_mapgen::passes::Fill;
    use rl_world::{BandId, CellFacts, ChunkContext, ChunkRules, Layers, Site, Surroundings, WorldConfig, WorldGraph, WorldRules};

    struct Flat;

    impl WorldRules for Flat {
        fn classify(&self, f: &CellFacts) -> BandId {
            BandId(if f.is_sea { 0 } else { 1 })
        }
        fn road_friction(&self, band: BandId, _: &CellFacts) -> Option<f32> {
            (band.0 == 1).then_some(0.0)
        }
        fn settlements(&self, layers: &Layers, _: u64) -> Vec<Site> {
            // One town on the first land region found, so discovery can be tested.
            layers.bands.iter().find(|(_, b)| b.0 == 1).map(|(p, _)| vec![Site { kind: rl_world::SiteKindId(1), position: p }]).unwrap_or_default()
        }
    }

    struct Open {
        tiles: TileRegistry,
    }

    impl ChunkRules for Open {
        fn tiles(&self) -> &TileRegistry {
            &self.tiles
        }
        fn fill(&self, _: &Surroundings) -> TileId {
            self.tiles.expect("floor")
        }
        fn chain(&self, _: &WorldGraph, around: &Surroundings) -> Chain<ChunkContext> {
            // Sea regions are solid so there is something unwalkable to bump.
            let tile = if around.here.band.0 == 0 { self.tiles.expect("wall") } else { self.tiles.expect("floor") };
            Chain::new().then(Fill { tile })
        }
    }

    fn app_with_world() -> (App, WorldGraph) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin));
        let mut tiles = TileRegistry::standard();
        tiles.register(TileProps::floor("mud").move_cost(200)).unwrap();
        let config = WorldConfig { region_size: 16, ..WorldConfig::regions(12, 10) };
        let world = WorldGraph::generate(RunSeed(5), config, &Flat);
        app.insert_resource(WorldMap::new(tiles.tables()));
        app.insert_resource(WorldRes(world.clone()));
        app.insert_resource(ChunkRulesRes(Box::new(Open { tiles })));
        (app, world)
    }

    fn land_tile(world: &WorldGraph) -> Point {
        let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("land");
        world.tile_origin(region).offset(8, 8)
    }

    fn spawn_player(app: &mut App, at: Point) -> Entity {
        let e = app.world_mut().spawn((Actor, Player, Blocks, Position(at), Viewshed::new(6), RevealsMap, Speed(100))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        e
    }

    fn intend<A: Action>(app: &mut App, actor: Entity, action: A) {
        app.world_mut().write_message(Intent::new(actor, action));
    }

    #[test]
    fn the_player_is_dealt_a_turn_and_walks() {
        let (mut app, world) = app_with_world();
        let start = land_tile(&world);
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
        let (mut app, world) = app_with_world();
        let start = land_tile(&world);
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
        let (mut app, world) = app_with_world();
        let start = land_tile(&world);
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
        let (mut app, world) = app_with_world();
        let start = land_tile(&world);
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

    fn resolve_shouts(
        mut intents: MessageReader<Intent<Shout>>,
        mut done: MessageWriter<ActionDone>,
        mut acting: ResMut<crate::turn::Acting>,
        mut heard: ResMut<Heard>,
    ) {
        for intent in intents.read() {
            if !acting.claim_action(intent.actor) {
                continue;
            }
            heard.0 += 1;
            done.write(ActionDone { actor: intent.actor, cost: rl_core::turn::BASE_ACTION_COST });
        }
    }

    #[test]
    fn a_game_action_the_engine_never_heard_of_spends_the_turn() {
        let (mut app, world) = app_with_world();
        app.init_resource::<Heard>().add_action::<Shout>().add_systems(Turn, resolve_shouts.in_set(TurnSet::Resolve));
        let start = land_tile(&world);
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
        let (mut app, world) = app_with_world();
        // Registered, and no resolver: the mistake a game makes.
        app.add_action::<Shout>();
        let start = land_tile(&world);
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
        let (mut app, world) = app_with_world();
        let start = land_tile(&world);
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
        let (mut app, world) = app_with_world();
        let start = land_tile(&world);
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
}
