//! Wiring: the sets, the plugins, and a headless app for tests.

use bevy::prelude::*;

use crate::knowledge::Knowledge;
use crate::state::EngineState;
use crate::turn::{ActionDone, ActionRefused, Intent, Occupancy, TurnEnd, Turns};
use crate::world::{WorldMap, WorldSettings};
use crate::{combat, fov, turn, world};

/// The stages of a frame while playing, in order.
///
/// Games put input and AI in `Decide`, their own action resolution in
/// `Resolve` alongside the engine's, and drawing in `Present`. The engine
/// never names a game system; games slot into these.
///
/// Streaming runs first so the window is loaded around wherever the player
/// ended the previous frame before anyone is dealt a turn on it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineSet {
    /// Move the loaded window with the player.
    Stream,
    /// Deal a turn.
    Schedule,
    /// Decide what to do with it: input for the player, AI for the rest.
    Decide,
    /// Apply the decisions.
    Resolve,
    /// Requeue and recover.
    Cleanup,
    /// Recompute sight.
    Fov,
    /// Draw.
    Present,
}

/// The engine's plugins, gated on [`EngineState::Playing`].
///
/// Insert [`WorldRes`](crate::world::WorldRes), [`ChunkRulesRes`](crate::world::ChunkRulesRes)
/// and a [`WorldMap`] before entering `Playing`; the streaming system does
/// the rest.
pub struct EnginePlugins;

impl Plugin for EnginePlugins {
    fn build(&self, app: &mut App) {
        app.init_state::<EngineState>()
            .init_resource::<Turns>()
            .init_resource::<Occupancy>()
            .init_resource::<WorldSettings>()
            .init_resource::<Knowledge>()
            .add_message::<Intent>()
            .add_message::<ActionDone>()
            .add_message::<ActionRefused>()
            .add_message::<TurnEnd>()
            .add_message::<combat::DamageEvent>()
            .add_message::<combat::DamageDealt>()
            .add_message::<world::ChunkLoaded>()
            .add_message::<combat::DeathEvent>()
            .init_resource::<combat::FlowFields>()
            .init_resource::<combat::DamageStages>()
            .configure_sets(
                Update,
                (
                    EngineSet::Stream,
                    EngineSet::Schedule,
                    EngineSet::Decide,
                    EngineSet::Resolve,
                    EngineSet::Cleanup,
                    EngineSet::Fov,
                    EngineSet::Present,
                )
                    .chain()
                    .run_if(in_state(EngineState::Playing))
                    .run_if(resource_exists::<WorldMap>),
            )
            .add_systems(Update, (turn::admit_new_actors, turn::schedule).chain().in_set(EngineSet::Schedule))
            // Combat is opt-in: a game that inserts no rules gets no combat
            // systems, and the walking demo stays a walking demo.
            .add_systems(Update, combat::decide_minds.in_set(EngineSet::Decide).run_if(combat_ready))
            .add_systems(
                Update,
                (turn::resolve_intents, combat::resolve_attacks.run_if(combat_ready), combat::apply_damage.run_if(combat_ready))
                    .chain()
                    .in_set(EngineSet::Resolve),
            )
            .add_systems(Update, (combat::process_deaths, turn::cleanup_turns, turn::forget_removed_blockers).chain().in_set(EngineSet::Cleanup))
            .add_systems(Update, world::stream_chunks.in_set(EngineSet::Stream))
            .add_systems(Update, fov::update_viewsheds.in_set(EngineSet::Fov));
    }
}

/// Whether the game has inserted what combat needs.
fn combat_ready(rules: Option<Res<combat::CombatRules>>, rng: Option<Res<combat::CombatRng>>) -> bool {
    rules.is_some() && rng.is_some()
}

/// A headless app with the engine plugins and no window, for tests in the
/// engine and in games.
pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin, EnginePlugins));
    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
    use crate::turn::Action;
    use crate::world::{ChunkRulesRes, WorldRes};
    use rl_core::{Direction, Point, RunSeed};
    use rl_grid::{TileId, TileProps, TileRegistry};
    use rl_mapgen::passes::Fill;
    use rl_mapgen::Chain;
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
            layers
                .bands
                .iter()
                .find(|(_, b)| b.0 == 1)
                .map(|(p, _)| vec![Site { kind: rl_world::SiteKindId(1), position: p }])
                .unwrap_or_default()
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
        let mut tiles = TileRegistry::standard();
        tiles.register(TileProps::floor("mud").move_cost(200)).unwrap();
        let config = WorldConfig { region_size: 16, ..WorldConfig::regions(12, 10) };
        let world = WorldGraph::generate(RunSeed(5), config, &Flat);
        app.insert_resource(WorldMap::new(16, tiles.tables()));
        app.insert_resource(WorldRes(world.clone()));
        app.insert_resource(ChunkRulesRes(Box::new(Open { tiles })));
        app.insert_resource(Knowledge::new(16));
        (app, world)
    }

    fn land_tile(world: &WorldGraph) -> Point {
        let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("land");
        world.tile_origin(region).offset(8, 8)
    }

    fn spawn_player(app: &mut App, at: Point) -> Entity {
        let e = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(at), Viewshed::new(6), RevealsMap, Speed(100)))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        e
    }

    fn intend(app: &mut App, actor: Entity, action: Action) {
        app.world_mut().write_message(Intent { actor, action });
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
        intend(&mut app, player, Action::Move(Direction::East));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start.offset(1, 0));
        assert_eq!(app.world().resource::<Turns>().now(), 0, "the clock waits for the requeue");
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some());
        assert_eq!(app.world().resource::<Turns>().now(), 100);
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
        intend(&mut app, player, Action::Move(Direction::East));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start);
        assert!(app.world().get::<MyTurn>(player).is_some());
        assert_eq!(app.world().resource::<Turns>().now(), 0);
    }

    #[test]
    fn a_stalled_non_player_is_charged_a_wait_within_the_frame_and_speed_scales_cost() {
        let (mut app, world) = app_with_world();
        let start = land_tile(&world);
        let player = spawn_player(&mut app, start);
        let fast = app.world_mut().spawn((Actor, Blocks, Position(start.offset(2, 0)), Speed(200))).id();
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player was admitted first and holds the turn");
        intend(&mut app, player, Action::Wait);
        app.update();
        // Frame 4: the other actor is dealt its turn, nobody decides for it,
        // and the recovery net charges it a wait before the frame ends.
        app.update();
        {
            let t = app.world().resource::<Turns>();
            assert!(app.world().get::<MyTurn>(fast).is_none());
            assert_eq!(t.now(), 0);
            assert_eq!(t.peek_time(), Some(50), "a wait at speed 200 costs 50");
        }
        // Frame 5: the clock advances to 50 and it happens again.
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), 50);
        assert!(app.world().get::<MyTurn>(player).is_none());
        // Frame 6: at 100 the player, requeued earlier, wins the tie.
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), 100);
        assert!(app.world().get::<MyTurn>(player).is_some());
        let _ = TurnEnd { turn: 0 };
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
            intend(&mut app, player, Action::Move(Direction::East));
            app.update();
            app.update();
        }
        let map = app.world().resource::<WorldMap>();
        assert_ne!(map.window(), first_window, "the window followed the player");
        assert!(map.is_loaded(app.world().get::<Position>(player).unwrap().0));
        // Walk back far enough that the edited region unloads, then return.
        for _ in 0..32 {
            intend(&mut app, player, Action::Move(Direction::East));
            app.update();
            app.update();
        }
        let map = app.world().resource::<WorldMap>();
        assert!(!map.is_loaded(edited), "the edited region left the window");
        assert_eq!(map.stored_deltas(), 1);
        for _ in 0..48 {
            intend(&mut app, player, Action::Move(Direction::West));
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
