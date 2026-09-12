//! The engine's own state as a save.
//!
//! The scheduler's clock and queue, the surface's edits and every built
//! place, and what the player knows. Entities in the queue are written
//! as [`SaveId`]s, so capture the game's entities first and restore them
//! first: the queue only keeps entries whose ids were bound.

use bevy::prelude::*;
use rl_bevy::{Knowledge, KnowledgeSave, Occupancy, Turns, WorldMap, WorldMapSave};
use rl_core::RunSeed;
use serde::{Deserialize, Serialize};

use crate::remap::{EntityRemap, SaveId};

/// Everything the engine owns that a run needs back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSave {
    /// The run's seed, so the world regenerates the same.
    pub seed: RunSeed,
    /// The clock.
    pub now: u32,
    /// Who is waiting to act, and when.
    pub queue: Vec<(SaveId, u32)>,
    /// The world's edits and places, and which map is current.
    pub map: WorldMapSave,
    /// What the player has seen.
    pub knowledge: KnowledgeSave,
}

impl EngineSave {
    /// Captures the engine's state from `world`. Every actor in the queue
    /// is given a save id through `remap`, so call this after the game
    /// captured its own entities.
    pub fn capture(world: &mut World, seed: RunSeed, remap: &mut EntityRemap) -> Self {
        let (now, entries) = world.resource::<Turns>().export();
        // Whoever holds the turn is out of the queue; it goes back at the
        // front of the present so the restored run deals it first.
        let holders: Vec<Entity> = world.query_filtered::<Entity, With<rl_bevy::MyTurn>>().iter(world).collect();
        let queue = holders.into_iter().map(|e| (e, now)).chain(entries).map(|(e, t)| (remap.save_id(e), t)).collect();
        Self { seed, now, queue, map: world.resource::<WorldMap>().export(), knowledge: world.resource::<Knowledge>().export() }
    }

    /// Restores the engine's state into `world`. The game's entities must
    /// be spawned and bound in `remap` already; queue entries for ids
    /// not bound are dropped, and the occupancy index is rebuilt from
    /// whatever blocks stand on the current map.
    pub fn restore(&self, world: &mut World, remap: &EntityRemap) {
        world.resource_mut::<WorldMap>().import(self.map.clone());
        world.resource_mut::<Knowledge>().import(self.knowledge.clone());
        let entries = self.queue.iter().filter_map(|(id, t)| remap.entity(*id).map(|e| (e, *t)));
        world.resource_mut::<Turns>().import(self.now, entries);
        let current = self.map.current;
        let mut occupancy = Occupancy::default();
        occupancy.switch(current);
        let mut standing = world.query_filtered::<(Entity, &rl_bevy::Position, Option<&rl_bevy::OnMap>), With<rl_bevy::Blocks>>();
        for (e, pos, on) in standing.iter(world) {
            occupancy.insert_on(on.map(|m| m.0).unwrap_or(rl_bevy::MapId::SURFACE), pos.0, e);
        }
        world.insert_resource(occupancy);
        world.resource_mut::<rl_bevy::FlowFields>().invalidate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_bevy::prelude::*;
    use rl_core::{Direction, Point};
    use rl_grid::{TileId, TileRegistry};
    use rl_mapgen::Chain;
    use rl_world::{BandId, CellFacts, ChunkContext, ChunkRules, Layers, Site, Surroundings, WorldConfig, WorldGraph, WorldRules};

    struct Flat;
    impl WorldRules for Flat {
        fn classify(&self, f: &CellFacts) -> BandId {
            BandId(if f.is_sea { 0 } else { 1 })
        }
        fn road_friction(&self, _: BandId, _: &CellFacts) -> Option<f32> {
            None
        }
        fn settlements(&self, _: &Layers, _: u64) -> Vec<Site> {
            Vec::new()
        }
    }
    struct Open(TileRegistry);
    impl ChunkRules for Open {
        fn tiles(&self) -> &TileRegistry {
            &self.0
        }
        fn fill(&self, _: &Surroundings) -> TileId {
            self.0.expect("floor")
        }
        fn chain(&self, _: &WorldGraph, _: &Surroundings) -> Chain<ChunkContext> {
            Chain::new().then(rl_mapgen::passes::Fill { tile: self.0.expect("floor") })
        }
    }

    fn fresh() -> (App, Point) {
        let mut app = rl_bevy::plugin::headless_app();
        let tiles = TileRegistry::standard();
        let world = WorldGraph::generate(RunSeed(5), WorldConfig { region_size: 16, ..WorldConfig::regions(12, 10) }, &Flat);
        let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("land");
        let start = world.tile_origin(region).offset(8, 8);
        app.insert_resource(WorldMap::new(tiles.tables()));
        app.insert_resource(WorldRes(world));
        app.insert_resource(ChunkRulesRes(Box::new(Open(tiles))));
        (app, start)
    }

    #[test]
    fn the_engine_state_survives_a_round_trip_into_a_fresh_world() {
        let (mut app, start) = fresh();
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Speed(100))).id();
        let other = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Speed(200))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent { actor: player, action: Action::Move(Direction::East) });
        app.update();
        let edited = start.offset(0, 2);
        assert!(app.world_mut().resource_mut::<WorldMap>().set_tile(edited, TileId(1)));
        app.world_mut().resource_mut::<Knowledge>().discover_site(4);

        // Capture: the game numbers its entities, then the engine.
        let mut remap = EntityRemap::new();
        let (p_id, o_id) = (remap.save_id(player), remap.save_id(other));
        let save = EngineSave::capture(app.world_mut(), RunSeed(5), &mut remap);
        let text = crate::encode(1, &save).unwrap();
        let back: EngineSave = crate::decode(1, &text).unwrap();
        assert_eq!(back.now, save.now);
        assert!(back.queue.iter().any(|(id, _)| *id == p_id), "the player is in the saved queue");

        // Restore into a fresh app with fresh entities.
        let (mut app2, _) = fresh();
        let player2 = app2.world_mut().spawn((Actor, Player, Blocks, Position(start.offset(1, 0)), Viewshed::new(6), RevealsMap, Speed(100))).id();
        let other2 = app2.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Speed(200))).id();
        let mut remap2 = EntityRemap::new();
        remap2.bind(p_id, player2);
        remap2.bind(o_id, other2);
        back.restore(app2.world_mut(), &remap2);
        app2.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app2.update();
        let w = app2.world();
        assert_eq!(w.resource::<Turns>().now(), save.now);
        assert_eq!(w.resource::<WorldMap>().tile(edited), Some(TileId(1)), "the edit was replayed onto the streamed chunk");
        assert!(w.resource::<Knowledge>().site_discovered(4));
        assert!(w.resource::<Knowledge>().is_explored(start), "explored tiles came back");
        assert!(w.resource::<Occupancy>().is_occupied(start.offset(3, 0)), "the index was rebuilt");
        assert!(w.resource::<Turns>().contains(other2) || w.get::<MyTurn>(other2).is_some(), "the other actor is scheduled");
        assert!(w.get::<MyTurn>(player2).is_some(), "the player, who held the turn when saved, holds it again");
    }
}
