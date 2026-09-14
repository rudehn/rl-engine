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
use rl_rules::Registry;
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

/// The stat definitions every [`StatBlock`] is read against.
///
/// Apart from [`StatusRules`] because a game may have stats and no
/// statuses, and because an ability's requirements need it whether or not
/// anything is ever afflicted.
#[derive(Resource, Debug, Clone, Deref)]
pub struct StatRules(pub Registry<rl_rules::StatDef>);

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
                damage.write(DamageEvent { target: entity, hit: Hit::from_source(credit, t.kind, t.amount) });
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

/// Statuses: what is afflicted, what it costs each turn, what cures it.
///
/// Ticks go through the damage pipeline, so combat comes with it, and
/// [`StatusRules`] must be in place before play begins.
///
/// Every [`Actor`] is given an empty [`Afflicted`] and [`StatBlock`] the
/// moment it is spawned, so a monster spawned without them still takes a
/// status rather than silently shrugging it off. Registered here and not
/// on `Actor` itself, so a game without statuses carries neither.
pub struct StatusPlugin;

impl Plugin for StatusPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{Needs, ResolveSet, Turn};
        // Before anything spawns, which is when Bevy allows it; an actor
        // spawned before this plugin was added is a setup Bevy refuses.
        app.register_required_components::<Actor, Afflicted>();
        app.register_required_components::<Actor, StatBlock>();
        app.add_message::<Afflict>()
            .add_message::<Cure>()
            .add_message::<StatusEvent>()
            .needs::<StatusRules>("StatusPlugin", "`StatusRules { defs }`, a registry of `StatusDef`s, which may be empty")
            .add_systems(Turn, (resolve_afflictions, tick_statuses).chain().in_set(ResolveSet::Effects));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::combat::CombatPlugin>(app, "StatusPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{Armor, CombatRng, CombatRules, DamageStages, Health};
    use crate::components::{Blocks, MyTurn, Player, Position, RevealsMap, Viewshed};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::Wait;
    use crate::turn::{Intent, Turns};
    use rl_core::RunSeed;
    use rl_rules::damage::{DamageKind, SubtractArmor};
    use rl_rules::faction::FactionDef;
    use rl_rules::{Factions, Op, Stacking, StatDef};

    #[derive(Resource, Default)]
    struct Heard(Vec<StatusEvent>);

    fn hear(mut events: MessageReader<StatusEvent>, mut heard: ResMut<Heard>) {
        heard.0.extend(events.read().copied());
    }

    #[test]
    fn a_status_ticks_damage_each_turn_modifies_stats_and_expires() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin, StatusPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let kinds = Registry::from_defs(vec![DamageKind::new("venom")]).unwrap();
        let venom_kind = kinds.expect("venom");
        let facs = Registry::from_defs(vec![FactionDef { name: "us".into() }]).unwrap();
        let stats = Registry::from_defs(vec![StatDef::new("armor", 0)]).unwrap();
        let armor_stat = stats.expect("armor");
        let defs = Registry::from_defs(vec![
            StatusDef::new("venom").ticks(venom_kind, 2).stacking(Stacking::Refresh),
            StatusDef::new("hearty").modifies(armor_stat, Op::Add(3)),
        ])
        .unwrap();
        let (venom, hearty) = (defs.expect("venom"), defs.expect("hearty"));
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
                Health::full(30),
                Armor(0),
                // No `Afflicted` and no `StatBlock`: the plugin gives every
                // actor both, and the statuses below must still land.
            ))
            .id();
        // Every status event, recorded by a reader: when a headless app
        // swaps its message buffers depends on wall time, so peeking at the
        // buffer after an update can miss a message that was written.
        app.init_resource::<Heard>().add_systems(PostUpdate, hear);
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
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
            assert_eq!(app.world().resource::<Turns>().turn_number(), turn);
            assert_eq!(app.world().get::<Health>(player).unwrap().hp, 30 - 2 * turn as i32, "turn {turn}");
        }
        let w = app.world();
        assert!(!w.get::<Afflicted>(player).unwrap().has(venom), "venom ran out");
        assert!(!w.get::<Afflicted>(player).unwrap().has(hearty));
        assert_eq!(w.get::<StatBlock>(player).unwrap().value(armor_stat, &stats), 0, "hearty's modifier left with it");
        let expired = &w.resource::<Heard>().0;
        assert!(expired.contains(&StatusEvent::Expired { target: player, status: venom }), "{expired:?}");
    }
}
