//! Statuses on actors: applied by request, ticked by the turn, expired
//! by the clock.
//!
//! The rules crate holds the model: a registered status with stat
//! modifiers and damage per turn, and the instances an actor carries.
//! This module gives every actor a [`StatBlock`] the modifiers land in
//! and an [`Afflicted`] list, resolves [`Afflict`] requests, and on every
//! whole turn ticks the statuses of the actors on the current map,
//! sending their damage through the same pipeline a blow goes through.
//! Opt-in: a game that inserts no [`StatusRules`] pays nothing.

use bevy::prelude::*;
use rl_content::Registry;
use rl_core::Id;
use rl_rules::{Hit, Stats, StatusDef, StatusId, Statuses};

use crate::combat::DamageEvent;
use crate::components::Actor;
use crate::places::{MapId, OnMap};
use crate::turn::TurnEnd;
use crate::world::WorldMap;

/// The status definitions.
#[derive(Resource)]
pub struct StatusRules {
    /// What exists.
    pub defs: Registry<StatusDef>,
}

/// An actor's stats: base values and every modifier from gear, statuses
/// and whatever else the game folds in.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
pub struct StatBlock(pub Stats);

/// The statuses an actor carries.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
pub struct Afflicted(pub Statuses);

/// A request to put a status on an actor. Resolved in
/// [`TurnSet::Resolve`](crate::plugin::TurnSet::Resolve), before damage.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Afflict {
    /// Who.
    pub target: Entity,
    /// Which.
    pub status: StatusId,
    /// For how many whole turns.
    pub turns: u32,
    /// Who did it, for credit on the damage it deals.
    pub by: Option<Entity>,
}

/// A request to take a status off an actor.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cure {
    /// Who.
    pub target: Entity,
    /// Which.
    pub status: StatusId,
}

/// What happened to a status.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusEvent {
    /// It was put on, or refreshed or extended.
    Applied {
        /// Who.
        target: Entity,
        /// Which.
        status: StatusId,
    },
    /// Its time ran out.
    Expired {
        /// Who.
        target: Entity,
        /// Which.
        status: StatusId,
    },
    /// It was taken off.
    Cured {
        /// Who.
        target: Entity,
        /// Which.
        status: StatusId,
    },
}

/// Applies and removes statuses as asked.
pub fn resolve_afflictions(
    mut afflicts: MessageReader<Afflict>,
    mut cures: MessageReader<Cure>,
    mut events: MessageWriter<StatusEvent>,
    rules: Res<StatusRules>,
    mut actors: Query<(&mut Afflicted, &mut StatBlock)>,
) {
    for a in afflicts.read() {
        let Ok((mut statuses, mut stats)) = actors.get_mut(a.target) else { continue };
        if statuses.0.apply(a.status, a.turns, a.by.map(|e| e.to_bits()), &rules.defs, &mut stats.0) {
            events.write(StatusEvent::Applied { target: a.target, status: a.status });
        }
    }
    for c in cures.read() {
        let Ok((mut statuses, mut stats)) = actors.get_mut(c.target) else { continue };
        if statuses.0.cure(c.status, &mut stats.0) {
            events.write(StatusEvent::Cured { target: c.target, status: c.status });
        }
    }
}

/// Once per whole turn, ticks every afflicted actor on the current map:
/// damage over time becomes [`DamageEvent`]s credited to whoever applied
/// the status, and what ran out is reported.
pub fn tick_statuses(
    mut ends: MessageReader<TurnEnd>,
    mut damage: MessageWriter<DamageEvent>,
    mut events: MessageWriter<StatusEvent>,
    rules: Res<StatusRules>,
    map: Res<WorldMap>,
    mut actors: Query<(Entity, &mut Afflicted, &mut StatBlock, Option<&OnMap>), With<Actor>>,
) {
    let turns = ends.read().count();
    if turns == 0 {
        return;
    }
    let here = map.current();
    for (entity, mut statuses, mut stats, on) in &mut actors {
        if on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here {
            continue;
        }
        for _ in 0..turns {
            let report = statuses.0.tick(&rules.defs, &mut stats.0);
            for t in report.ticks {
                let credit = t.source.and_then(Entity::try_from_bits);
                damage.write(DamageEvent { target: entity, hit: Hit::from_source(credit, Id::from_raw(t.kind), t.amount) });
            }
            for status in report.expired {
                events.write(StatusEvent::Expired { target: entity, status });
            }
        }
    }
}

/// Whether the game inserted status rules.
pub fn statuses_ready(rules: Option<Res<StatusRules>>) -> bool {
    rules.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{Armor, CombatRng, CombatRules, DamageStages, Health};
    use crate::components::{Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
    use crate::knowledge::Knowledge;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::{Action, Intent, Turns};
    use crate::world::{ChunkRulesRes, WorldRes};
    use rl_core::RunSeed;
    use rl_grid::{TileId, TileRegistry};
    use rl_mapgen::Chain;
    use rl_rules::damage::{DamageKind, SubtractArmor};
    use rl_rules::faction::FactionDef;
    use rl_rules::{Factions, Op, Stacking, StatDef};
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

    #[test]
    fn a_status_ticks_damage_each_turn_modifies_stats_and_expires() {
        let mut app = headless_app();
        let tiles = TileRegistry::standard();
        let world = WorldGraph::generate(RunSeed(5), WorldConfig { region_size: 16, ..WorldConfig::regions(12, 10) }, &Flat);
        let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("land");
        let start = world.tile_origin(region).offset(8, 8);
        let kinds = Registry::from_defs(vec![DamageKind::new("venom")]).unwrap();
        let venom_kind = kinds.expect("venom");
        let facs = Registry::from_defs(vec![FactionDef { name: "us".into() }]).unwrap();
        let stats = Registry::from_defs(vec![StatDef::new("armor", 0)]).unwrap();
        let armor_stat = stats.expect("armor");
        let defs = Registry::from_defs(vec![
            StatusDef::new("venom").ticks(venom_kind.raw(), 2).stacking(Stacking::Refresh),
            StatusDef::new("hearty").modifies(armor_stat.raw(), Op::Add(3)),
        ])
        .unwrap();
        let (venom, hearty) = (defs.expect("venom"), defs.expect("hearty"));
        app.insert_resource(WorldMap::new(16, tiles.tables()));
        app.insert_resource(WorldRes(world));
        app.insert_resource(ChunkRulesRes(Box::new(Open(tiles))));
        app.insert_resource(Knowledge::new(16));
        app.insert_resource(CombatRules { kinds, factions: Factions::new(&facs) });
        app.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
        app.insert_resource(CombatRng::for_run(RunSeed(5)));
        app.insert_resource(StatusRules { defs });
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(6),
                RevealsMap,
                Speed(100),
                Health::full(30),
                Armor(0),
                StatBlock::default(),
                Afflicted::default(),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Afflict { target: player, status: venom, turns: 3, by: None });
        app.world_mut().write_message(Afflict { target: player, status: hearty, turns: 2, by: None });
        app.update();
        {
            let w = app.world();
            assert!(w.get::<Afflicted>(player).unwrap().has(venom));
            assert_eq!(w.get::<StatBlock>(player).unwrap().value(armor_stat, &stats), 3, "hearty's modifier landed");
            assert_eq!(w.get::<Health>(player).unwrap().hp, 30, "nothing ticks until a turn passes");
        }
        // Each wait is a whole turn: venom deals 2 for three turns, hearty
        // lasts two.
        for turn in 1..=3u32 {
            assert!(app.world().get::<MyTurn>(player).is_some());
            app.world_mut().write_message(Intent { actor: player, action: Action::Wait });
            app.update();
            assert_eq!(app.world().resource::<Turns>().turn_number(), turn);
            assert_eq!(app.world().get::<Health>(player).unwrap().hp, 30 - 2 * turn as i32, "turn {turn}");
        }
        let w = app.world();
        assert!(!w.get::<Afflicted>(player).unwrap().has(venom), "venom ran out");
        assert!(!w.get::<Afflicted>(player).unwrap().has(hearty));
        assert_eq!(w.get::<StatBlock>(player).unwrap().value(armor_stat, &stats), 0, "hearty's modifier left with it");
        let expired: Vec<StatusEvent> = w.resource::<Messages<StatusEvent>>().iter_current_update_messages().copied().collect();
        assert!(expired.contains(&StatusEvent::Expired { target: player, status: venom }), "{expired:?}");
    }
}
