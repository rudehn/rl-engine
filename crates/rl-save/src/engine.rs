//! The engine's own state as a save.
//!
//! The scheduler's clock and queue, the surface's edits and every built
//! place, what the player knows, and what each saved entity had to spend on
//! abilities. Entities are written as [`SaveId`]s, so capture the game's
//! entities first and restore them first: the queue and the ability state
//! only keep entries whose ids were bound.

use bevy::prelude::*;
use rl_bevy::{Charges, Cooldowns, Knowledge, KnowledgeSave, Occupancy, Pools, Seed, Turns, WorldMap, WorldMapSave};
use rl_core::RunSeed;
use rl_rules::StatId;
use rl_rules::ability::AbilityId;
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
    /// What each saved entity had to spend on abilities. Beside the clock
    /// because a cooldown is an absolute time on it, so restoring the clock
    /// and these together restores every cooldown with nothing to tick.
    /// Empty in a save written before abilities were saved.
    #[serde(default)]
    pub abilities: Vec<(SaveId, AbilityState)>,
}

/// The ability state of one entity, as a save holds it.
///
/// Only what a use spends and sets. What an entity knows is rebuilt every
/// turn from what it is and wears, and what grants an ability is the game's
/// content, so neither is here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AbilityState {
    /// What was in each pool.
    pub pools: Vec<(StatId, i32)>,
    /// When each ability is ready again, on the saved clock.
    pub cooldowns: Vec<(AbilityId, u32)>,
    /// Charges left and the most it holds, on something that lends an
    /// ability.
    pub charges: Option<(u16, u16)>,
}

impl AbilityState {
    /// What `entity` has, or `None` when it has nothing worth saving: an
    /// actor with empty pools and nothing cooling needs no entry, since the
    /// abilities plugin gives every actor those empty.
    fn of(world: &World, entity: Entity) -> Option<Self> {
        let pools: Vec<(StatId, i32)> = world.get::<Pools>(entity).map(|p| p.iter().collect()).unwrap_or_default();
        let cooldowns: Vec<(AbilityId, u32)> = world.get::<Cooldowns>(entity).map(|c| c.iter().collect()).unwrap_or_default();
        let charges = world.get::<Charges>(entity).map(|c| (c.left, c.max));
        (!pools.is_empty() || !cooldowns.is_empty() || charges.is_some()).then_some(Self { pools, cooldowns, charges })
    }

    /// Puts this back on `entity`.
    fn restore(&self, world: &mut World, entity: Entity) {
        let Ok(mut target) = world.get_entity_mut(entity) else { return };
        if !self.pools.is_empty() {
            let mut pools = Pools::new();
            for (stat, amount) in &self.pools {
                pools.set(*stat, *amount);
            }
            target.insert(pools);
        }
        if !self.cooldowns.is_empty() {
            let mut cooldowns = Cooldowns::new();
            for (ability, ready_at) in &self.cooldowns {
                cooldowns.set(*ability, *ready_at);
            }
            target.insert(cooldowns);
        }
        if let Some((left, max)) = self.charges {
            target.insert(Charges { left, max });
        }
    }
}

impl EngineSave {
    /// Captures the engine's state from `world`, the run's [`Seed`] included.
    /// Every actor in the queue is given a save id through `remap`, so call
    /// this after the game captured its own entities.
    pub fn capture(world: &mut World, remap: &mut EntityRemap) -> Self {
        let seed = world.resource::<Seed>().0;
        // The game's own entities, read before the queue hands ids to actors
        // the game never saved and so could never restore.
        let abilities: Vec<(SaveId, AbilityState)> = remap.bound().filter_map(|(id, e)| AbilityState::of(world, e).map(|s| (id, s))).collect();
        let (now, entries) = world.resource::<Turns>().export();
        // Whoever holds the turn is out of the queue; it goes back at the
        // front of the present so the restored run deals it first.
        let holders: Vec<Entity> = world.query_filtered::<Entity, With<rl_bevy::MyTurn>>().iter(world).collect();
        let queue = holders.into_iter().map(|e| (e, now)).chain(entries).map(|(e, t)| (remap.save_id(e), t)).collect();
        Self { seed, now, queue, map: world.resource::<WorldMap>().export(), knowledge: world.resource::<Knowledge>().export(), abilities }
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
        for (id, state) in &self.abilities {
            if let Some(entity) = remap.entity(*id) {
                state.restore(world, entity);
            }
        }
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
    use rl_grid::TileId;

    fn fresh() -> (App, Point) {
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins((rl_bevy::FovPlugin, rl_bevy::StreamingPlugin));
        let start = rl_bevy::testing::surface(&mut app);
        (app, start)
    }

    #[test]
    fn the_engine_state_survives_a_round_trip_into_a_fresh_world() {
        let (mut app, start) = fresh();
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap)).id();
        let other = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Speed(200))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Step(Direction::East)));
        app.update();
        let edited = start.offset(0, 2);
        assert!(app.world_mut().resource_mut::<WorldMap>().set_tile(edited, TileId(1)));
        app.world_mut().resource_mut::<Knowledge>().discover_site(4);

        // Capture: the game numbers its entities, then the engine.
        let mut remap = EntityRemap::new();
        let (p_id, o_id) = (remap.save_id(player), remap.save_id(other));
        let save = {
            app.insert_resource(Seed(RunSeed(5)));
            EngineSave::capture(app.world_mut(), &mut remap)
        };
        let text = crate::encode(1, &save).unwrap();
        let back: EngineSave = crate::decode(1, &text).unwrap();
        assert_eq!(back.now, save.now);
        assert!(back.queue.iter().any(|(id, _)| *id == p_id), "the player is in the saved queue");

        // Restore into a fresh app with fresh entities.
        let (mut app2, _) = fresh();
        let player2 = app2.world_mut().spawn((Actor, Player, Blocks, Position(start.offset(1, 0)), Viewshed::new(6), RevealsMap)).id();
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

    /// What abilities spend is engine state, and the design promised it a
    /// place in the save beside the clock: pools keep what is in them, a
    /// cooldown set before saving is still live at the same clock after
    /// loading, and a wand keeps the charges it had left.
    #[test]
    fn ability_pools_cooldowns_and_charges_survive_a_round_trip() {
        let (mut app, start) = fresh();
        let mut pools = Pools::new();
        pools.set(rl_core::Id::from_raw(0), 12);
        let mut cooldowns = Cooldowns::new();
        cooldowns.set(rl_core::Id::from_raw(1), 700);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, pools, cooldowns)).id();
        let wand = app.world_mut().spawn(Charges { left: 2, max: 5 }).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();

        let mut remap = EntityRemap::new();
        let (p_id, w_id) = (remap.save_id(player), remap.save_id(wand));
        let save = {
            app.insert_resource(Seed(RunSeed(5)));
            EngineSave::capture(app.world_mut(), &mut remap)
        };
        let back: EngineSave = crate::decode(1, &crate::encode(1, &save).unwrap()).unwrap();

        let (mut app2, _) = fresh();
        let player2 = app2.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap)).id();
        let wand2 = app2.world_mut().spawn_empty().id();
        let mut remap2 = EntityRemap::new();
        remap2.bind(p_id, player2);
        remap2.bind(w_id, wand2);
        back.restore(app2.world_mut(), &remap2);

        let w = app2.world();
        let now = w.resource::<Turns>().now();
        assert_eq!(w.get::<Pools>(player2).map(|p| p.get(rl_core::Id::from_raw(0))), Some(12), "the pool kept what was in it");
        let ready = w.get::<Cooldowns>(player2).map(|c| c.ready_at(rl_core::Id::from_raw(1)));
        assert_eq!(ready, Some(700), "the cooldown kept its absolute time");
        assert!(ready.is_some_and(|t| t > now), "and is still live at the restored clock, {now}");
        assert_eq!(w.get::<Charges>(wand2).copied(), Some(Charges { left: 2, max: 5 }), "the wand kept its charges");
    }
}
