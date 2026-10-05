//! Statuses on actors: applied by request, ticked by the turn, expired
//! by the clock.
//!
//! The rules crate holds the model: a registered status with stat
//! modifiers and damage per turn, and the instances an actor carries.
//! This module gives every actor a [`StatBlock`] the modifiers land in
//! and an [`Afflicted`] list, resolves [`Afflict`] requests, and on every
//! whole turn ticks the statuses of the actors on the current map,
//! sending their damage through the same pipeline a blow goes through.
//! A status that holds takes its holder's turns: [`pass_held_turns`]
//! spends each as a wait before its mind or its player is asked.
//! Opt-in: a game that never adds [`StatusPlugin`] pays nothing, and the
//! statuses themselves are content, in [`Registries`].

use bevy::prelude::*;
use rl_core::turn::BASE_ACTION_COST;
use rl_rules::{Hit, Stats, StatusId, Statuses};

use crate::combat::{DamageEvent, Heal};
use crate::components::{Actor, MyTurn};
use crate::places::{MapId, OnMap};
use crate::registries::Registries;
use crate::turn::{Acting, ActionDone, TurnEnd};
use crate::world::WorldMap;

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
    /// The worn thing it lasts only while, or `None` for a status that runs
    /// its time whatever is worn.
    ///
    /// Held, it ends through the ordinary [`Cure`] in the pass its holder
    /// is not wearing that thing, however that came about; see
    /// [`end_unworn_holds`](crate::items::end_unworn_holds). One landed on
    /// somebody who is not wearing the thing, as an area use reaching a
    /// bystander, ends in the pass after it goes on. A refresh or an
    /// extension by a held request is held by the new thing, and one by an
    /// unheld request stays held by the old, which is why a held refresh
    /// reaching a bystander who already carries the same status unheld
    /// takes their instance over and cures it the very next pass: `held_by`
    /// is meant for a trigger that only ever lands on its own wearer, a
    /// `use` on oneself, not one that reaches an area or a `hit`.
    pub held_by: Option<Entity>,
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
    registries: Res<Registries>,
    mut actors: Query<(&mut Afflicted, &mut StatBlock)>,
) {
    for a in afflicts.read() {
        let Ok((mut statuses, mut stats)) = actors.get_mut(a.target) else { continue };
        let by = a.by.map(|e| e.to_bits());
        let changed = match a.held_by {
            Some(item) => statuses.0.apply_held(a.status, a.turns, by, item.to_bits(), &registries.statuses, &mut stats.0),
            None => statuses.0.apply(a.status, a.turns, by, &registries.statuses, &mut stats.0),
        };
        if changed {
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
/// the status, a status that mends becomes [`Heal`]s, and what ran out is
/// reported.
pub fn tick_statuses(
    mut ends: MessageReader<TurnEnd>,
    mut damage: MessageWriter<DamageEvent>,
    mut heals: MessageWriter<Heal>,
    mut events: MessageWriter<StatusEvent>,
    registries: Res<Registries>,
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
            let report = statuses.0.tick(&registries.statuses, &mut stats.0);
            for t in report.ticks {
                let credit = t.source.and_then(Entity::try_from_bits);
                damage.write(DamageEvent::new(entity, Hit::from_status(credit, t.status, t.kind, t.amount)));
            }
            for m in report.mends {
                let by = m.source.and_then(Entity::try_from_bits);
                heals.write(Heal { target: entity, amount: m.amount, by, status: Some(m.status) });
            }
            for status in report.expired {
                events.write(StatusEvent::Expired { target: entity, status });
            }
        }
    }
}

/// Spends the turn of whoever holds it while a status that
/// [`holds`](rl_rules::StatusDef::holds) is on them, as a wait costs,
/// before its mind or its player is asked.
///
/// In [`DecideSet::Sense`](crate::plugin::DecideSet::Sense), where work
/// claims a worker's turns, and for the player as for anyone. The decision
/// is claimed, so no mind perceives or chooses, and the turn is spent here
/// rather than by an intent resolved later, so a key the player pressed
/// finds it spent and comes to nothing whichever stage would have resolved
/// it. Taken whatever claimed the decision first, so a worker caught at
/// its work loses the turn rather than working it.
pub fn pass_held_turns(
    mut acting: ResMut<Acting>,
    mut spent: MessageWriter<ActionDone>,
    registries: Res<Registries>,
    holders: Query<(Entity, &Afflicted), With<MyTurn>>,
) {
    for (actor, statuses) in &holders {
        if statuses.holding(&registries.statuses).is_none() {
            continue;
        }
        acting.claim_decision(actor);
        if acting.claim_action(actor) {
            spent.write(ActionDone { actor, cost: BASE_ACTION_COST });
        }
    }
}

/// Statuses: what is afflicted, what it costs each turn, what cures it.
///
/// Ticks go through the damage pipeline, so combat comes with it, and
/// [`Registries`] must be in place before play begins, with the statuses in it.
///
/// Every [`Actor`] is given an empty [`Afflicted`] and [`StatBlock`] the
/// moment it is spawned, so a monster spawned without them still takes a
/// status rather than silently shrugging it off. Registered here and not
/// on `Actor` itself, so a game without statuses carries neither.
pub struct StatusPlugin;

impl Plugin for StatusPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{DecideSet, Needs, ResolveSet, Turn};
        // Before anything spawns, which is when Bevy allows it; an actor
        // spawned before this plugin was added is a setup Bevy refuses.
        // `try` for the stats, which the items plugin requires as well so
        // gear has somewhere to land, whichever of the two is added first.
        app.register_required_components::<Actor, Afflicted>();
        let _ = app.try_register_required_components::<Actor, StatBlock>();
        app.add_message::<Afflict>()
            .add_message::<Cure>()
            .add_message::<StatusEvent>()
            .needs::<Registries>("StatusPlugin", "`Registries`, with the statuses, which may be empty")
            .add_systems(Turn, pass_held_turns.in_set(DecideSet::Sense))
            .add_systems(Turn, (resolve_afflictions, tick_statuses).chain().in_set(ResolveSet::Effects));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::combat::CombatPlugin>(app, "StatusPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bump::Bump;
    use crate::combat::{Armor, CombatRules, DamageStages, Health};
    use crate::components::{Blocks, MyTurn, Player, Position, RevealsMap, Viewshed};
    use crate::minds::{Mind, Perception};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::Wait;
    use crate::turn::{Intent, Turns};
    use crate::work::{AddWork, WorkKinds, Working};
    use rl_core::turn::BASE_ACTION_COST;
    use rl_core::{Direction, Point, RunSeed};
    use rl_rules::ai::{Brain, Decision, Tactic, TacticCtx};
    use rl_rules::damage::{DamageKind, SubtractArmor};
    use rl_rules::faction::FactionDef;
    use rl_rules::work::Work;
    use rl_rules::{Op, Registry, Stacking, StatDef, StatusDef, UNTIL_CURED};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

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
        let facs = Registry::from_defs(vec![FactionDef::new("us")]).unwrap();
        let stats = Registry::from_defs(vec![StatDef::new("armor", 0)]).unwrap();
        let armor_stat = stats.expect("armor");
        let defs = Registry::from_defs(vec![
            StatusDef::new("venom").ticks(venom_kind, 2).stacking(Stacking::Refresh),
            StatusDef::new("hearty").modifies(armor_stat, Op::Add(3)),
        ])
        .unwrap();
        let (venom, hearty) = (defs.expect("venom"), defs.expect("hearty"));
        app.insert_resource(CombatRules::new(&facs));
        app.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
        app.insert_resource(crate::seed::Seed(RunSeed(5)));
        app.insert_resource(Registries { damage_kinds: kinds, factions: facs, stats: stats.clone(), statuses: defs, ..Default::default() });
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
        app.world_mut().write_message(Afflict { target: player, status: venom, turns: 3, by: None, held_by: None });
        app.world_mut().write_message(Afflict { target: player, status: hearty, turns: 2, by: None, held_by: None });
        app.update();
        {
            let w = app.world();
            assert!(w.get::<Afflicted>(player).unwrap().has(venom));
            assert_eq!(w.get::<StatBlock>(player).unwrap().value(armor_stat, &stats), 3, "hearty's modifier landed");
            assert_eq!(w.get::<Health>(player).unwrap().current, 30, "nothing ticks until a turn passes");
        }
        // Each wait is a whole turn: venom deals 2 for three turns, hearty
        // lasts two.
        for turn in 1..=3u32 {
            assert!(app.world().get::<MyTurn>(player).is_some());
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
            assert_eq!(app.world().resource::<Turns>().turn_number(), turn);
            assert_eq!(app.world().get::<Health>(player).unwrap().current, 30 - 2 * turn as i32, "turn {turn}");
        }
        let w = app.world();
        assert!(!w.get::<Afflicted>(player).unwrap().has(venom), "venom ran out");
        assert!(!w.get::<Afflicted>(player).unwrap().has(hearty));
        assert_eq!(w.get::<StatBlock>(player).unwrap().value(armor_stat, &stats), 0, "hearty's modifier left with it");
        let expired = &w.resource::<Heard>().0;
        assert!(expired.contains(&StatusEvent::Expired { target: player, status: venom }), "{expired:?}");
    }

    /// A status that mends restores its amount every whole turn, to no
    /// more than the most its holder has, and is reported as a heal by
    /// whoever applied it and never as damage dealt: there is no kind for
    /// armor or a resistance to be asked about.
    #[test]
    fn a_status_that_mends_restores_health_each_turn_and_stops_at_the_most() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin, StatusPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let facs = Registry::from_defs(vec![FactionDef::new("us")]).unwrap();
        let defs = Registry::from_defs(vec![StatusDef::new("mending").mends(4).boon()]).unwrap();
        let mending = defs.expect("mending");
        app.insert_resource(CombatRules::new(&facs));
        app.insert_resource(crate::seed::Seed(RunSeed(5)));
        app.insert_resource(Registries { factions: facs, statuses: defs, ..Default::default() });
        let player =
            app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Health { current: 21, max: 30 }, Armor(50))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Afflict { target: player, status: mending, turns: 3, by: Some(player), held_by: None });
        app.update();
        let mut healed = Vec::new();
        for want in [25, 29, 30] {
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
            assert_eq!(app.world().get::<Health>(player).unwrap().current, want);
            healed.extend(app.world_mut().resource_mut::<Messages<crate::combat::Healed>>().drain());
        }
        assert_eq!(healed.iter().map(|h| (h.restored, h.by, h.status)).collect::<Vec<_>>(), [4, 4, 1].map(|n| (n, Some(player), Some(mending))).to_vec());
        assert!(app.world_mut().resource_mut::<Messages<crate::combat::DamageDealt>>().drain().next().is_none(), "nothing was dealt");
    }

    /// A world with minds and statuses in it, and `snared`, a status that
    /// holds, the only one; whatever else a test adds, added before the app
    /// first runs; and a player to pass the turns, holding the first.
    fn held_arena(extra: impl FnOnce(&mut App)) -> (App, Point, Entity, StatusId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin, crate::combat::CombatPlugin, crate::minds::MindsPlugin, StatusPlugin));
        extra(&mut app);
        let start = crate::testing::surface(&mut app);
        crate::testing::two_sides(&mut app);
        let statuses = Registry::from_defs(vec![StatusDef::new("snared").holds()]).unwrap();
        let snared = statuses.expect("snared");
        app.world_mut().resource_mut::<Registries>().statuses = statuses;
        app.init_resource::<Spent>().add_systems(crate::plugin::Turn, record_spent.in_set(crate::plugin::TurnSet::Record));
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (app, start, player, snared)
    }

    /// Every turn spent, by whom and for how much, in the order spent.
    #[derive(Resource, Default)]
    struct Spent(Vec<(Entity, u32)>);

    fn record_spent(mut done: MessageReader<crate::turn::ActionDone>, mut spent: ResMut<Spent>) {
        spent.0.extend(done.read().map(|d| (d.actor, d.cost)));
    }

    /// What `who` spent its turns on, so far.
    fn spent_by(app: &App, who: Entity) -> Vec<u32> {
        app.world().resource::<Spent>().0.iter().filter(|(actor, _)| *actor == who).map(|(_, cost)| *cost).collect()
    }

    /// One player turn: a wait, if the player holds the turn, and a frame.
    fn pass(app: &mut App, player: Entity) {
        if app.world().get::<MyTurn>(player).is_some() {
            app.world_mut().write_message(Intent::new(player, Wait));
        }
        app.update();
    }

    /// Steps south whenever it is asked, and counts each asking, so a test
    /// sees both whether the mind was asked and whether it acted.
    struct Walk(Arc<AtomicU32>);

    impl Tactic<Entity> for Walk {
        fn name(&self) -> &'static str {
            "walk"
        }
        fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Some(Decision::Step(ctx.snapshot.me.pos.offset(0, 1)))
        }
    }

    /// An actor three cells from the player that walks whenever its mind
    /// is asked, and the count of the askings.
    fn walker(app: &mut App, start: Point) -> (Entity, Arc<AtomicU32>) {
        let asked = Arc::new(AtomicU32::new(0));
        let brain = Arc::new(Brain::new().then(Walk(asked.clone())));
        let walker = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(5), Perception(6), Mind(brain))).id();
        (walker, asked)
    }

    /// Put on before the walker's first turn, a snare of three turns takes
    /// exactly three of them, each spent as a wait costs, with the mind
    /// never asked; the turn after it runs out is the walker's own.
    #[test]
    fn a_held_actors_turns_pass_as_waits_and_its_mind_is_not_asked_until_the_status_runs_out() {
        let (mut app, start, player, snared) = held_arena(|_| {});
        let (walker, asked) = walker(&mut app, start);
        app.world_mut().write_message(Afflict { target: walker, status: snared, turns: 3, by: None, held_by: None });
        for _ in 0..3 {
            pass(&mut app, player);
        }
        assert!(!app.world().get::<Afflicted>(walker).unwrap().has(snared), "the snare has run out");
        assert_eq!(spent_by(&app, walker), vec![BASE_ACTION_COST; 3], "three turns lost, each spent as a wait");
        assert_eq!(asked.load(Ordering::Relaxed), 0, "and its mind asked on none of them");
        assert_eq!(app.world().get::<Position>(walker).unwrap().0, start.offset(3, 0), "nor did it move");
        pass(&mut app, player);
        assert_eq!(asked.load(Ordering::Relaxed), 1, "free, it is asked again");
        assert_eq!(app.world().get::<Position>(walker).unwrap().0, start.offset(3, 1), "and walks");
    }

    /// The one way a key reaches a player who is held: the status landed
    /// while the player held its turn, after the decide stage had passed
    /// it by, so the player still holds the turn when the next frame reads
    /// the keys. The key is spent with that turn, and the turns after it
    /// pass with no key at all.
    #[test]
    fn a_held_players_keys_do_nothing_and_its_turns_pass_until_the_status_runs_out() {
        let (mut app, start, player, snared) = held_arena(|_| {});
        app.world_mut().write_message(Afflict { target: player, status: snared, turns: 2, by: None, held_by: None });
        app.update();
        assert!(app.world().get::<Afflicted>(player).unwrap().has(snared), "caught");
        assert!(app.world().get::<MyTurn>(player).is_some(), "holding its turn");
        app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start, "the key did nothing");
        assert!(!app.world().get::<Afflicted>(player).unwrap().has(snared), "the turns passed until the snare ran out");
        assert_eq!(spent_by(&app, player), vec![BASE_ACTION_COST; 3], "the turn it was caught in and the two the snare lasted, each a wait");
        assert!(app.world().get::<MyTurn>(player).is_some(), "free, it holds a turn again");
        app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        app.update();
        assert_eq!(app.world().get::<Position>(player).unwrap().0, start.offset(1, 0), "and its keys work again");
    }

    #[test]
    fn a_status_that_holds_until_cured_frees_its_holder_on_the_turn_after_the_cure() {
        let (mut app, start, player, snared) = held_arena(|_| {});
        let (walker, asked) = walker(&mut app, start);
        app.world_mut().write_message(Afflict { target: walker, status: snared, turns: UNTIL_CURED, by: None, held_by: None });
        for _ in 0..3 {
            pass(&mut app, player);
        }
        assert_eq!(asked.load(Ordering::Relaxed), 0, "held for as long as it is on");
        app.world_mut().write_message(Cure { target: walker, status: snared });
        pass(&mut app, player);
        assert_eq!(asked.load(Ordering::Relaxed), 1, "cured, its next turn is its own");
        assert_eq!(app.world().get::<Position>(walker).unwrap().0, start.offset(3, 1));
    }

    /// The hold takes the turn whatever claimed the decision first, so a
    /// worker caught at its work loses the turn rather than working it.
    #[test]
    fn a_worker_caught_by_a_status_that_holds_does_no_work_on_the_turns_it_loses() {
        let (mut app, start, player, snared) = held_arena(|app| {
            app.add_plugins(crate::work::WorkPlugin).add_work("mending");
        });
        let kind = app.world().resource::<WorkKinds>().get("mending").expect("declared");
        let worker = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(5), Working(Work::new(kind, 4)))).id();
        app.world_mut().write_message(Afflict { target: worker, status: snared, turns: 3, by: None, held_by: None });
        for _ in 0..3 {
            pass(&mut app, player);
        }
        assert_eq!(app.world().get::<Working>(worker).map(|w| w.0.done), Some(0), "no work done on the turns it lost");
        for _ in 0..4 {
            pass(&mut app, player);
        }
        assert!(app.world().get::<Working>(worker).is_none(), "free, it works its four turns and is done");
    }
}
