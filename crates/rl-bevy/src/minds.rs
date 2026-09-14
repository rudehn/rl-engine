//! Minds: what decides the turn of everyone but the player.
//!
//! A [`Mind`] holds a brain, a priority list of tactics from
//! [`rl_rules::ai`]. On its turn the engine builds the snapshot its tactics
//! read (who it perceives, sorted into allies and enemies by the faction
//! matrix, the trail it is on, and what it may use) and turns the decision
//! into the intent of the action that answers it: a step, an attack, a
//! wait, an ability, or a number of the game's own.
//!
//! Its own plugin rather than part of combat, because this is the one place
//! every action a monster can choose meets. Combat resolves a blow whoever
//! struck it and abilities resolve a use whoever chose it; only a mind has to
//! know that both exist, so neither of them has to know about the other.

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use rand::rngs::StdRng;
use rl_core::{Direction, Point, geometry};
use rl_grid::{DijkstraMap, PathRules};
use rl_rules::{ActorView, Brain, Decision, MovementProfile, Snapshot, TacticCtx};

use crate::ability::{Offered, Use};
use crate::combat::{Attack, CombatRng, CombatRules, Faction, Health};
use crate::components::{Actor, MyTurn, Player, Position, Viewshed};
use crate::lighting::{DarkSight, Lighting, perceives};
use crate::places::{MapId, OnMap};
use crate::turn::{Acting, Intent, Occupancy, Step, Turns, Wait};
use crate::world::WorldMap;

/// How far a non-player notices things, in tiles. Sight is symmetric,
/// so a monster sees the player exactly when the player sees it and it
/// is within this range.
#[derive(Component, Debug, Clone, Copy)]
pub struct Perception(pub i32);

/// The movement class an actor paths with.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Profile(pub MovementProfile);

/// The brain deciding a non-player's turns. Shared, since most monsters of
/// a kind think alike.
#[derive(Component, Clone)]
#[component(on_add = report_mind_without_plugin)]
pub struct Mind(pub Arc<Brain<Entity>>);

/// One approach map per movement class, rebuilt when the player moves.
#[derive(Resource, Default)]
pub struct FlowFields {
    built_at: Option<Point>,
    approach: BTreeMap<MovementProfile, DijkstraMap>,
    escape: BTreeMap<MovementProfile, DijkstraMap>,
}

impl FlowFields {
    fn ensure(&mut self, profile: MovementProfile, player: Point, map: &WorldMap) {
        if self.built_at != Some(player) {
            self.approach.clear();
            self.escape.clear();
            self.built_at = Some(player);
        }
        if self.approach.contains_key(&profile) {
            return;
        }
        let view = map.view();
        let Some(local) = map.to_local(player) else { return };
        let mut approach = DijkstraMap::covering(&view);
        approach.build(&view, [local], PathRules::default());
        let mut escape = approach.clone();
        escape.scale(-12, 10);
        escape.rescan(&view, PathRules::default());
        self.approach.insert(profile, approach);
        self.escape.insert(profile, escape);
    }

    /// The approach map for `profile`, if built this turn.
    pub fn approach(&self, profile: MovementProfile) -> Option<&DijkstraMap> {
        self.approach.get(&profile)
    }

    /// Forgets every map, so the next mind rebuilds them: the player
    /// changed maps, or the terrain changed under everyone.
    pub fn invalidate(&mut self) {
        self.built_at = None;
        self.approach.clear();
        self.escape.clear();
    }
}

/// What a mind reads about any actor.
type ActorData = (Entity, &'static Position, &'static Health, &'static Faction, Option<&'static Perception>, Option<&'static OnMap>);
/// The mind holding the turn.
type MindData = (Entity, &'static Mind, Option<&'static Profile>);

/// Everyone a mind might see, and the mind whose turn it is.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Sight<'w, 's> {
    player: Query<'w, 's, (&'static Position, &'static Viewshed), With<Player>>,
    actors: Query<'w, 's, ActorData, With<Actor>>,
    minds: Query<'w, 's, MindData, (With<MyTurn>, Without<Player>)>,
    lighting: Option<Res<'w, Lighting>>,
    dark: Query<'w, 's, &'static DarkSight>,
    /// Who is hiding, and what the thinker has noticed of them. Both empty
    /// in a game without stealth, and then everything is seen on sight.
    hidden: Query<'w, 's, (), With<crate::stealth::Stealth>>,
    aware: Query<'w, 's, &'static crate::stealth::Aware>,
    stealth: crate::stealth::StealthRunning<'w>,
}

/// Whether an observer at `from` perceives `to`: a line through the
/// player's viewshed, then light, dark sight or adjacency.
///
/// Lines of sight are symmetric and non-players carry no viewshed, so the
/// player's is the oracle: the observer has a line to the player if the
/// player has one to it, and to anyone else if the player has one to them
/// both. Light is not symmetric, so what it then perceives along that line
/// is whatever is lit, within its dark sight, or adjacent. Shared by the
/// minds and by noticing, so the two can never disagree about who could be
/// seen.
pub fn perceivable(player_pos: Point, player_sight: &Viewshed, lighting: Option<&Lighting>, from: Point, dark_sight: i32, to: Point) -> bool {
    let in_line = player_sight.in_line(from) && (to == player_pos || player_sight.in_line(to));
    in_line && perceives(lighting, from, dark_sight, to)
}

/// The shared state a mind reads and the stream it draws from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct MindWorld<'w> {
    fields: ResMut<'w, FlowFields>,
    /// What the ability layer narrowed down for this mind, when the game
    /// added it. Absent in a game with no abilities, and then no tactic
    /// is ever offered one.
    offered: Option<Res<'w, Offered>>,
    rng: ResMut<'w, CombatRng>,
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    rules: Res<'w, CombatRules>,
    turns: Res<'w, Turns>,
}

/// A mind chose something of the game's own: whatever number its tactic
/// returned, and who chose it.
///
/// Written in [`DecideSet::Minds`](crate::plugin::DecideSet::Minds) and
/// answered by the game in [`DecideSet::Game`](crate::plugin::DecideSet::Game),
/// which is where it turns the number into one of its own actions. The
/// actor's decision is already claimed, so nothing else will decide for
/// it this pass.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MindChose {
    /// Who chose.
    pub actor: Entity,
    /// What, in the game's own numbering.
    pub choice: u32,
}

/// What a mind writes when it decides.
#[derive(bevy::ecs::system::SystemParam)]
pub struct MindIntents<'w> {
    moves: MessageWriter<'w, Intent<Step>>,
    abilities: MessageWriter<'w, Intent<Use>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    chose: MessageWriter<'w, MindChose>,
}

/// Lets every non-player holding a turn decide it.
///
/// A game that decides for an actor itself claims it in
/// [`TurnSet::Decide`](crate::plugin::TurnSet::Decide), and the mind
/// leaves that actor alone, so a monster can take an action the engine
/// has never heard of.
pub fn decide_minds(mut intents: MindIntents, mut acting: ResMut<Acting>, mut world: MindWorld, sight: Sight) {
    let Ok((player_pos, player_sight)) = sight.player.single() else { return };
    let Ok((thinker, mind, profile)) = sight.minds.single() else { return };
    let Ok((_, my_pos, my_hp, my_faction, perception, _)) = sight.actors.get(thinker) else { return };
    let MindWorld { fields, offered, rng, map, occupancy, rules, turns } = &mut world;
    let (fields, rng, map, occupancy, rules, turns) = (&mut **fields, &mut **rng, &**map, &**occupancy, &**rules, &**turns);
    let actors = &sight.actors;
    let profile = profile.map(|p| p.0).unwrap_or_default();
    let reach = perception.map(|p| p.0).unwrap_or(8);

    let me = ActorView { id: thinker, pos: my_pos.0, hp: my_hp.hp, max_hp: my_hp.max, faction: my_faction.0 };
    let mut snapshot = Snapshot::alone(me);
    let dark_sight = sight.dark.get(thinker).map(|d| d.0).unwrap_or(0);
    let lighting = sight.lighting.as_deref();
    let here = map.current();
    // With stealth running, what this mind has noticed; without it, `None`
    // and everything it can perceive is seen.
    let aware = sight.aware.get(thinker).ok().filter(|_| sight.stealth.get());
    for (e, pos, hp, faction, _, on) in actors.iter() {
        if e == thinker || on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here || geometry::chebyshev(pos.0, my_pos.0) > reach {
            continue;
        }
        if !perceivable(player_pos.0, player_sight, lighting, my_pos.0, dark_sight, pos.0) {
            continue;
        }
        let view = ActorView { id: e, pos: pos.0, hp: hp.hp, max_hp: hp.max, faction: faction.0 };
        if rules.factions.is_hostile(my_faction.0, faction.0) {
            // A hider it has not noticed is not an enemy it can act on.
            if aware.is_some_and(|a| sight.hidden.contains(e) && !a.knows(e)) {
                continue;
            }
            snapshot.enemies.push(view);
        } else if rules.factions.is_allied(my_faction.0, faction.0) {
            snapshot.allies.push(view);
        }
    }
    snapshot.sort();
    // The freshest trail it is on but cannot see the end of: what a search
    // walks toward.
    if let Some(aware) = aware {
        snapshot.last_known = aware
            .0
            .iter()
            .filter(|(subject, _)| !snapshot.enemies.iter().any(|e| e.id == **subject))
            .filter_map(|(_, state)| Some((state.stale_turns()?, state.last_known()?)))
            .min_by_key(|(stale, at)| (*stale, *at))
            .map(|(_, at)| at);
    }
    if let Some(offered) = offered.as_deref() {
        snapshot.usable = offered.usable_by(thinker).to_vec();
    }

    // The shared flow fields are built toward the player. Where stealth is
    // running, a mind that has not seen the player must not descend them,
    // or it would walk straight to a player it never noticed.
    let player_seen = snapshot.enemies.iter().any(|e| e.pos == player_pos.0);
    let maps_allowed = aware.is_none() || player_seen;
    let wants_maps = !snapshot.enemies.is_empty() && maps_allowed;
    if wants_maps {
        fields.ensure(profile, player_pos.0, map);
    }
    let origin = map.window_tiles().origin();
    // Maps are window-local; translate through a local copy of the
    // decision so tactics stay in world coordinates.
    let approach = fields.approach.get(&profile);
    let escape = fields.escape.get(&profile);
    let can_step = |p: Point| map.is_walkable(p) && !occupancy.is_occupied(p);
    // The predicate the ability resolver uses, so what a tactic thinks a
    // shape will cover is what it does cover.
    let blocks_shot = |p: Point| map.blocks_projectiles(p) || occupancy.is_occupied(p);
    let mut turn_rng: StdRng =
        rand::SeedableRng::seed_from_u64(rl_core::seed::position_hash(turns.now() as u64 ^ rand::RngCore::next_u64(&mut rng.0), my_pos.0.x, my_pos.0.y));
    let shifted = |m: &DijkstraMap| shift_map(m, origin);
    let approach_world = approach.filter(|_| maps_allowed).map(shifted);
    let escape_world = escape.filter(|_| maps_allowed).map(shifted);
    let mut ctx = TacticCtx {
        snapshot: &snapshot,
        approach: approach_world.as_ref(),
        escape: escape_world.as_ref(),
        can_step: &can_step,
        blocks_shot: &blocks_shot,
        bounds: map.window_tiles(),
        rng: &mut turn_rng,
    };
    let (decision, _which) = mind.0.decide(&mut ctx);
    if !acting.claim_decision(thinker) {
        return;
    }
    match decision {
        Decision::Step(to) => match Direction::between(my_pos.0, to) {
            Some(d) => {
                intents.moves.write(Intent::new(thinker, Step(d)));
            }
            None => {
                intents.waits.write(Intent::new(thinker, Wait));
            }
        },
        Decision::Attack(target) => {
            intents.attacks.write(Intent::new(thinker, Attack(target)));
        }
        Decision::Ability { ability, aim } => {
            intents.abilities.write(Intent::new(thinker, Use { ability, aim }));
        }
        Decision::Wait => {
            intents.waits.write(Intent::new(thinker, Wait));
        }
        // The game's own: hand the number back and let it act.
        Decision::Game(choice) => {
            intents.chose.write(MindChose { actor: thinker, choice });
        }
    }
}

/// A copy of a window-local map re-addressed in world coordinates.
fn shift_map(m: &DijkstraMap, origin: Point) -> DijkstraMap {
    let r = m.region();
    let mut out = DijkstraMap::new(rl_core::Rect::new(r.x + origin.x, r.y + origin.y, r.width, r.height));
    out.copy_values_from(m);
    out
}

/// Minds: every non-player carrying a [`Mind`] decides its own turn.
///
/// Needs combat, for the faction matrix a mind sorts friend from foe by and
/// the stream it rolls from, and the field of view, because the player's
/// viewshed is the line-of-sight oracle. With
/// [`AbilitiesPlugin`](crate::ability::AbilitiesPlugin) added as well, a mind
/// is offered what it may use; without it, never.
///
/// A game that decides every monster's turn with systems of its own leaves
/// this out. One that spawns a [`Mind`] without it is told so, once, rather
/// than left watching monsters that never move.
pub struct MindsPlugin;

impl Plugin for MindsPlugin {
    fn build(&self, app: &mut App) {
        // The minds may choose an ability, so the message they would write
        // it into exists whether or not the game added abilities. Registering
        // it twice is what `add_message` is built for.
        app.init_resource::<MindsRunning>()
            .add_message::<Intent<Use>>()
            .add_message::<MindChose>()
            .add_systems(crate::plugin::Turn, decide_minds.in_set(crate::plugin::DecideSet::Minds));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::combat::CombatPlugin>(app, "MindsPlugin");
        crate::plugin::depends_on::<crate::fov::FovPlugin>(app, "MindsPlugin");
    }
}

/// Present while [`MindsPlugin`] is added, so a [`Mind`] spawned without it
/// can say so.
#[derive(Resource, Default)]
struct MindsRunning;

/// Set once a [`Mind`] without [`MindsPlugin`] has been reported, so the
/// report is made once and not per monster.
#[derive(Resource, Default)]
struct ToldMindsAreOff;

/// Reports the first [`Mind`] added to a world with no [`MindsPlugin`].
fn report_mind_without_plugin(mut world: DeferredWorld, _: HookContext) {
    if world.contains_resource::<MindsRunning>() || world.contains_resource::<ToldMindsAreOff>() {
        return;
    }
    world.commands().init_resource::<ToldMindsAreOff>();
    error!("an actor was given a `Mind`, but `MindsPlugin` was not added, so nothing will ever decide its turns; add `MindsPlugin` beside `CombatPlugin`");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{Armor, CombatPlugin, MeleeAttack};
    use crate::components::Blocks;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::Resolution;
    use rl_core::DiceRoll;
    use rl_rules::ai::tactics::{Hunt, MeleeAdjacent};
    use rl_rules::damage::DamageKindId;

    fn arena() -> (App, Point, DamageKindId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, MindsPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        (app, start, sides.kind)
    }

    /// A tactic of the game's own, at the top of the priority list.
    struct Shove;
    impl rl_rules::ai::Tactic<Entity> for Shove {
        fn name(&self) -> &'static str {
            "shove"
        }
        fn evaluate(&self, ctx: &mut rl_rules::ai::TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
            ctx.snapshot.enemies.first().map(|_| Decision::Game(SHOVE))
        }
    }

    /// The game's number for a shove.
    const SHOVE: u32 = 7;

    /// The game's action, which the engine has never heard of.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Shoved(Entity);
    impl crate::turn::Action for Shoved {}

    #[derive(Resource, Default)]
    struct Shoves(u32);

    /// The game's half of the decision.
    fn answer_the_choice(mut chose: MessageReader<MindChose>, mut shoves: MessageWriter<Intent<Shoved>>, players: Query<Entity, With<Player>>) {
        for c in chose.read() {
            if c.choice == SHOVE
                && let Ok(player) = players.single()
            {
                shoves.write(Intent::new(c.actor, Shoved(player)));
            }
        }
    }

    fn resolve_shoves(mut intents: MessageReader<Intent<Shoved>>, mut resolution: Resolution, mut count: ResMut<Shoves>) {
        for intent in intents.read() {
            if !resolution.claim(intent.actor) {
                continue;
            }
            count.0 += 1;
            resolution.done(intent.actor, rl_core::turn::BASE_ACTION_COST);
        }
    }

    #[test]
    fn a_mind_can_choose_an_action_the_engine_never_heard_of() {
        use crate::turn::AddAction;
        let (mut app, start, blunt) = arena();
        app.init_resource::<Shoves>()
            .add_action::<Shoved>()
            .add_systems(crate::plugin::Turn, answer_the_choice.in_set(crate::plugin::DecideSet::Game))
            .add_systems(crate::plugin::Turn, resolve_shoves.in_set(crate::plugin::ResolveSet::Act));
        let us = rl_rules::FactionId::from_raw(0);
        let them = rl_rules::FactionId::from_raw(1);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(us),
                MeleeAttack { kind: blunt, dice: DiceRoll::flat(1) },
            ))
            .id();
        // Shove first, strike second: the game's tactic outranks the
        // engine's, which is what the priority list is for.
        let brain = Arc::new(Brain::new().then(Shove).then(MeleeAdjacent));
        app.world_mut().spawn((
            Actor,
            Blocks,
            Position(start.offset(1, 0)),
            Health::full(5),
            Faction(them),
            Perception(8),
            MeleeAttack { kind: blunt, dice: DiceRoll::flat(3) },
            Mind(brain),
        ));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);

        for _ in 0..6 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
        }
        assert!(app.world().resource::<Shoves>().0 > 0, "the monster shoved");
        assert_eq!(app.world().get::<Health>(player).unwrap().hp, 30, "and never struck, because shoving outranks it");
    }

    #[test]
    fn a_monster_hunts_strikes_and_dies() {
        let (mut app, start, blunt) = arena();
        let us = rl_rules::FactionId::from_raw(0);
        let them = rl_rules::FactionId::from_raw(1);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(us),
                Armor(1),
                MeleeAttack { kind: blunt, dice: DiceRoll::flat(50) },
            ))
            .id();
        let brain = Arc::new(Brain::new().then(MeleeAdjacent).then(Hunt));
        let monster = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(4, 0)),
                Health::full(5),
                Faction(them),
                Perception(8),
                MeleeAttack { kind: blunt, dice: DiceRoll::flat(3) },
                Mind(brain),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        // The player waits; the monster closes and strikes.
        let mut hits = 0;
        for _ in 0..40 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Wait));
            }
            app.update();
            let hp = app.world().get::<Health>(player).unwrap().hp;
            if hp < 30 {
                hits = 30 - hp;
                break;
            }
        }
        assert_eq!(hits, 2, "3 damage minus 1 armor");
        let mpos = app.world().get::<Position>(monster).unwrap().0;
        assert!(geometry::is_adjacent(mpos, start), "the monster closed in: {mpos:?}");
        // The player strikes back and the monster is removed.
        for _ in 0..10 {
            if app.world().get::<MyTurn>(player).is_some() {
                app.world_mut().write_message(Intent::new(player, Attack(monster)));
            }
            app.update();
            if app.world().get_entity(monster).is_err() {
                break;
            }
        }
        assert!(app.world().get_entity(monster).is_err(), "the monster despawned by the end of the frame");
        assert!(!app.world().resource::<Occupancy>().is_occupied(mpos));
        assert!(!app.world().resource::<Turns>().contains(monster));
    }

    #[test]
    fn a_mind_spawned_without_the_plugin_is_reported_once() {
        let mut without = headless_app();
        without.add_plugins((crate::fov::FovPlugin, CombatPlugin));
        without.world_mut().spawn(Mind(Arc::new(Brain::new())));
        without.world_mut().spawn(Mind(Arc::new(Brain::new())));
        without.update();
        assert!(without.world().contains_resource::<ToldMindsAreOff>(), "a mind nothing will run is reported");

        let mut with = headless_app();
        with.add_plugins((crate::fov::FovPlugin, CombatPlugin, MindsPlugin));
        with.world_mut().spawn(Mind(Arc::new(Brain::new())));
        with.update();
        assert!(!with.world().contains_resource::<ToldMindsAreOff>(), "and one the plugin runs is not");
    }
}
