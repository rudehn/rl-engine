//! Health, attacks, damage and deaths, and the minds that decide them.
//!
//! The engine owns the loop: an attack intent becomes a [`DamageEvent`],
//! the game's damage stages mitigate it, health drops, a [`DeathEvent`]
//! fires, and dead non-players are removed from the world, the queue and
//! the occupancy index. What a hit is worth and what a death means beyond
//! that are the game's, read from the same messages.

use std::collections::BTreeMap;
use std::sync::Arc;

use bevy::prelude::*;
use rand::rngs::StdRng;
use rl_core::{DiceRoll, Direction, Point, RunSeed, SeedDomain, geometry};
use rl_grid::{DijkstraMap, PathRules};
use rl_rules::Registry;
use rl_rules::damage::{DamageKind, DamageKindId, Defender};
use rl_rules::{ActorView, Brain, Decision, MovementProfile, Snapshot, TacticCtx};
use rl_rules::{DamageStage, Factions, Hit, Resistances};

use crate::components::{Actor, Blocks, MyTurn, Player, Position, Viewshed};
use crate::lighting::{DarkSight, Lighting, perceives};
use crate::places::{MapId, OnMap};
use crate::turn::{Acting, Action, Intent, Occupancy, Resolution, Step, Turns, Wait};
use crate::world::WorldMap;

/// Hit points.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    /// Current.
    pub hp: i32,
    /// Maximum.
    pub max: i32,
}

impl Health {
    /// Full health of `max`.
    pub const fn full(max: i32) -> Self {
        Self { hp: max, max }
    }
}

/// Flat armor for the damage stages.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Armor(pub i32);

/// Resistances for the damage stages.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
pub struct Resists(pub Resistances);

/// Which side an actor is on.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Faction(pub rl_rules::FactionId);

/// What an actor's unarmed strike does.
#[derive(Component, Debug, Clone, Copy)]
pub struct MeleeAttack {
    /// Damage kind.
    pub kind: DamageKindId,
    /// Damage roll.
    pub dice: DiceRoll,
}

/// What an actor's shot does, and how far it reaches. An attack on a
/// target that is not adjacent uses this, if the line of fire is clear.
#[derive(Component, Debug, Clone, Copy)]
pub struct RangedAttack {
    /// Damage kind.
    pub kind: DamageKindId,
    /// Damage roll.
    pub dice: DiceRoll,
    /// Furthest cell it reaches.
    pub range: i32,
}

/// Extra rolls every hit by this actor carries, each its own
/// [`DamageEvent`]: a flaming blade's fire, a venomed edge's poison.
#[derive(Component, Debug, Clone, Default)]
pub struct Strikes(pub Vec<(DamageKindId, DiceRoll)>);

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
pub struct Mind(pub Arc<Brain<Entity>>);

/// The registries combat reads.
#[derive(Resource)]
pub struct CombatRules {
    /// Damage kinds.
    pub kinds: Registry<DamageKind>,
    /// Who hates whom.
    pub factions: Factions,
}

/// The mitigation pipeline, in order. Empty means damage lands raw.
#[derive(Resource, Default)]
pub struct DamageStages(pub Vec<Box<dyn DamageStage<Entity> + Send + Sync>>);

/// The combat stream for the run.
#[derive(Resource, Deref, DerefMut)]
pub struct CombatRng(pub StdRng);

impl CombatRng {
    /// The stream for `seed`.
    pub fn for_run(seed: RunSeed) -> Self {
        Self(seed.rng(SeedDomain::new(b"combat"), 0))
    }
}

/// Damage about to land on `target`.
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageEvent {
    /// Who takes it.
    pub target: Entity,
    /// The hit.
    pub hit: Hit<Entity>,
}

/// Damage that actually landed, after mitigation, for narration and
/// on-hit reactions. Zero means the hit was fully stopped; negative healed.
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageDealt {
    /// Who took it.
    pub target: Entity,
    /// The hit as it arrived.
    pub hit: Hit<Entity>,
    /// What health lost.
    pub dealt: i32,
}

/// An actor's health reached zero. A non-player is taken out of the
/// world at once (no position, no turns, no cell) and despawned at the
/// end of the frame, so the game's systems can still read what it was
/// while they react to this; the player is left for the game.
#[derive(Message, Debug, Clone, Copy)]
pub struct DeathEvent {
    /// Who died.
    pub entity: Entity,
    /// Where. The entity is despawned before the game's systems run, so
    /// anything it leaves behind goes here.
    pub at: Point,
    /// Who gets the credit.
    pub credit: Option<Entity>,
    /// Whether it was the player.
    pub was_player: bool,
}

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
/// A defender as the damage system sees it.
type DefenderData = (&'static mut Health, &'static Position, Option<&'static Armor>, Option<&'static Resists>, Has<Player>);

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
    offered: Option<Res<'w, crate::ability::Offered>>,
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

/// Strike an actor: adjacent with a melee weapon, at range with a ranged
/// one down a clear line of fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attack(pub Entity);
impl Action for Attack {}

/// What a mind writes when it decides.
#[derive(bevy::ecs::system::SystemParam)]
pub struct MindIntents<'w> {
    moves: MessageWriter<'w, Intent<Step>>,
    abilities: MessageWriter<'w, Intent<crate::ability::Use>>,
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
            intents.abilities.write(Intent::new(thinker, crate::ability::Use { ability, aim }));
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

/// Where an attack happens: the map, who stands on it, and who strikes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Arena<'w, 's> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    attackers: Query<'w, 's, AttackerData, With<MyTurn>>,
    targets: Query<'w, 's, &'static Position, With<Health>>,
}

/// An attacker as the attack resolver sees it.
type AttackerData = (&'static Position, Option<&'static MeleeAttack>, Option<&'static RangedAttack>, Option<&'static Strikes>);

/// Turns attack intents into damage events: a melee strike on an
/// adjacent target, a shot on a distant one with a clear line of fire,
/// each followed by the attacker's extra strikes. A shot at nothing in
/// reach still costs the turn.
///
/// One turn, one strike: an attack by an actor that already acted this
/// pass finds the turn spent, whatever spent it.
pub fn resolve_attacks(
    mut intents: MessageReader<Intent<Attack>>,
    mut damage: MessageWriter<DamageEvent>,
    mut resolution: Resolution,
    mut rng: ResMut<CombatRng>,
    arena: Arena,
) {
    let Arena { map, occupancy, attackers, targets } = arena;
    for intent in intents.read() {
        let target = intent.action.0;
        let Ok((pos, melee, ranged, strikes)) = attackers.get(intent.actor) else { continue };
        if !resolution.claim(intent.actor) {
            continue;
        }
        let Ok(target_pos) = targets.get(target) else {
            resolution.done(intent.actor, rl_core::turn::BASE_ACTION_COST);
            continue;
        };
        let weapon = if geometry::is_adjacent(pos.0, target_pos.0) {
            melee.map(|m| (m.kind, m.dice))
        } else {
            ranged.filter(|r| line_of_fire(&map, &occupancy, pos.0, target_pos.0, r.range)).map(|r| (r.kind, r.dice))
        };
        if let Some((kind, dice)) = weapon {
            // Floored where it is rolled: a blow that rolls below zero has
            // missed, and the pipeline would read a negative one as a heal.
            let amount = dice.roll_at_least(&mut **rng, 0);
            damage.write(DamageEvent { target, hit: Hit::by(intent.actor, kind, amount) });
            for (kind, dice) in strikes.map(|s| s.0.as_slice()).unwrap_or(&[]) {
                let amount = dice.roll_at_least(&mut **rng, 0);
                damage.write(DamageEvent { target, hit: Hit::by(intent.actor, *kind, amount) });
            }
        }
        resolution.done(intent.actor, rl_core::turn::BASE_ACTION_COST);
    }
}

/// Whether a shot from `from` reaches `to` within `range`: nothing that
/// stops projectiles and nobody standing in between.
pub fn line_of_fire(map: &WorldMap, occupancy: &Occupancy, from: Point, to: Point, range: i32) -> bool {
    rl_grid::clear_shot(from, to, range, map.window_tiles(), |p| p != to && (map.blocks_projectiles(p) || occupancy.is_occupied(p)))
}

/// Runs the damage stages and applies what is left to health.
pub fn apply_damage(
    mut events: MessageReader<DamageEvent>,
    mut dealt: MessageWriter<DamageDealt>,
    mut deaths: MessageWriter<DeathEvent>,
    rules: Res<CombatRules>,
    stages: Res<DamageStages>,
    mut targets: Query<DefenderData>,
) {
    for ev in events.read() {
        let Ok((mut health, pos, armor, resist, is_player)) = targets.get_mut(ev.target) else { continue };
        if health.hp <= 0 {
            continue;
        }
        let defender = Defender { armor: armor.map(|a| a.0).unwrap_or(0), blocked: false };
        let none = Resistances::new();
        let stage_refs: Vec<&dyn DamageStage<Entity>> = stages.0.iter().map(|s| s.as_ref() as &dyn DamageStage<Entity>).collect();
        let amount = rl_rules::resolve(&ev.hit, &defender, resist.map(|r| &r.0).unwrap_or(&none), &rules.kinds, &stage_refs);
        health.hp = (health.hp - amount).min(health.max);
        dealt.write(DamageDealt { target: ev.target, hit: ev.hit, dealt: amount });
        if health.hp <= 0 {
            deaths.write(DeathEvent { entity: ev.target, at: pos.0, credit: ev.hit.credit, was_player: is_player });
        }
    }
}

/// Dead, and waiting for the end of the frame to be despawned.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Dead;

/// Takes dead non-players out of the world, the queue and the index, and
/// marks them [`Dead`] for [`bury_the_dead`].
pub fn process_deaths(
    mut commands: Commands,
    mut deaths: MessageReader<DeathEvent>,
    mut turns: ResMut<Turns>,
    mut occupancy: ResMut<Occupancy>,
    positions: Query<(&Position, Has<Blocks>)>,
) {
    for d in deaths.read() {
        if d.was_player {
            continue;
        }
        if let Ok((pos, blocks)) = positions.get(d.entity)
            && blocks
        {
            occupancy.remove(pos.0, d.entity);
        }
        turns.remove(d.entity);
        commands.entity(d.entity).remove::<(Actor, Blocks, Position)>().insert(Dead);
    }
}

/// Despawns whoever died this frame, once every system has seen them go.
pub fn bury_the_dead(mut commands: Commands, dead: Query<Entity, With<Dead>>) {
    for e in &dead {
        commands.entity(e).despawn();
    }
}

/// Combat: health, factions, strikes down a line of fire, the damage
/// pipeline, deaths, and the minds that decide for everyone but the
/// player.
///
/// Needs [`CombatRules`] and [`CombatRng`] before play begins, and the
/// minds read the player's viewshed, so the field of view comes with it.
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{CleanupSet, Needs, ResolveSet, Turn};
        use crate::turn::AddAction;
        // The minds may choose an ability, so the message they would
        // write it into exists whether or not the game added abilities.
        // Registering it twice is what `add_message` is built for.
        app.add_message::<Intent<crate::ability::Use>>()
            .add_message::<DamageEvent>()
            .add_message::<DamageDealt>()
            .add_message::<DeathEvent>()
            .add_message::<MindChose>()
            .init_resource::<DamageStages>()
            .add_action::<Attack>()
            .needs::<CombatRules>("CombatPlugin", "`CombatRules { kinds, factions }`, a registry of damage kinds and a faction matrix")
            .needs::<CombatRng>("CombatPlugin", "`CombatRng::for_run(seed)`, the stream combat rolls from")
            .add_systems(Turn, decide_minds.in_set(crate::plugin::DecideSet::Minds))
            .add_systems(Turn, resolve_attacks.in_set(ResolveSet::Act))
            .add_systems(Turn, apply_damage.in_set(ResolveSet::Damage))
            .add_systems(Turn, process_deaths.in_set(CleanupSet::Remove))
            // In `Last`, after everything that reads the frame's deaths has
            // run, which is the promise that the dead linger until the frame
            // ends, kept without naming any of those systems.
            .add_systems(Last, bury_the_dead);
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "CombatPlugin");
        crate::plugin::depends_on::<crate::fov::FovPlugin>(app, "CombatPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::world::{ChunkRulesRes, WorldRes};
    use rl_grid::{TileId, TileRegistry};
    use rl_mapgen::Chain;
    use rl_mapgen::passes::Fill;
    use rl_rules::ai::tactics::{Hunt, MeleeAdjacent};
    use rl_rules::damage::SubtractArmor;
    use rl_rules::faction::FactionDef;
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
            Chain::new().then(Fill { tile: self.0.expect("floor") })
        }
    }

    fn arena() -> (App, Point, DamageKindId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin));
        let tiles = TileRegistry::standard();
        let world = WorldGraph::generate(RunSeed(5), WorldConfig { region_size: 16, ..WorldConfig::regions(12, 10) }, &Flat);
        let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("land");
        let start = world.tile_origin(region).offset(8, 8);
        let kinds = Registry::from_defs(vec![DamageKind::new("blunt")]).unwrap();
        let blunt = kinds.expect("blunt");
        let facs = Registry::from_defs(vec![FactionDef { name: "us".into() }, FactionDef { name: "them".into() }]).unwrap();
        let mut factions = Factions::new(&facs);
        factions.set_mutual(facs.expect("us"), facs.expect("them"), rl_rules::Relation::Hostile);
        app.insert_resource(WorldMap::new(tiles.tables()));
        app.insert_resource(WorldRes(world));
        app.insert_resource(ChunkRulesRes(Box::new(Open(tiles))));
        app.insert_resource(CombatRules { kinds, factions });
        app.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
        app.insert_resource(CombatRng::for_run(RunSeed(5)));
        (app, start, blunt)
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
    fn a_shot_needs_a_clear_line_of_fire_and_carries_extra_strikes() {
        let (mut app, start, blunt) = arena();
        let (us, them) = (rl_rules::FactionId::from_raw(0), rl_rules::FactionId::from_raw(1));
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
                RangedAttack { kind: blunt, dice: DiceRoll::flat(3), range: 6 },
                Strikes(vec![(blunt, DiceRoll::flat(2))]),
            ))
            .id();
        // A target four cells east with no mind: it never moves.
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(4, 0)), Health::full(20), Faction(them))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(app.world().get::<Health>(target).unwrap().hp, 20 - 3 - 2, "the shot and the extra strike both landed");
        // A wall in between stops the next shot; the turn is still spent.
        app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(2, 0), TileId(1));
        app.update();
        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(app.world().get::<Health>(target).unwrap().hp, 15, "the wall took the shot");
        assert!(app.world().resource::<Turns>().now() > before);
        // Out of range is no shot either.
        app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(2, 0), TileId(0));
        let far = app.world_mut().spawn((Actor, Blocks, Position(start.offset(7, 0)), Health::full(20), Faction(them))).id();
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Attack(far)));
        app.update();
        assert_eq!(app.world().get::<Health>(far).unwrap().hp, 20);
    }
}

/// Damage everyone the ability's aim wanted under its footprint.
///
/// Through [`DamageEvent`] rather than onto health directly, so a
/// fireball is mitigated by the same armor, resistances and stages a
/// sword is, and a game that inserts a stage gets it on both at once.
#[derive(Debug, Clone, Copy)]
pub struct Harm {
    /// What kind of damage.
    pub kind: DamageKindId,
    /// How much, rolled per target.
    pub roll: DiceRoll,
}

impl crate::ability::Effect for Harm {
    fn apply(&self, landing: &crate::ability::Landing, world: &mut crate::ability::EffectWorld<'_, '_>) {
        for target in &landing.targets {
            let amount = self.roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent { target: *target, hit: Hit::by(landing.user, self.kind, amount) });
        }
    }
}

impl crate::ability::FromArgs for Harm {
    const KIND: &'static str = "Harm";

    fn from_args(args: &rl_rules::ability::RawValue, look: &dyn rl_rules::ability::Lookup) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            kind: String,
            roll: String,
        }
        let a: Args = rl_rules::ability::read_args(args)?;
        Ok(Self { kind: look.damage(&a.kind).ok_or_else(|| format!("unknown damage kind {:?}", a.kind))?, roll: a.roll.parse().map_err(|e| format!("{e}"))? })
    }
}

/// Heal everyone under the footprint.
///
/// Negative damage of a named kind, so resistance to it is a game's to
/// define: a construct that resists the kind a medkit deals cannot be
/// patched up, and nothing in the engine had to learn the word undead.
#[derive(Debug, Clone, Copy)]
pub struct Mend {
    /// The kind healing counts as.
    pub kind: DamageKindId,
    /// How much, rolled per target.
    pub roll: DiceRoll,
}

impl crate::ability::Effect for Mend {
    fn apply(&self, landing: &crate::ability::Landing, world: &mut crate::ability::EffectWorld<'_, '_>) {
        for target in &landing.targets {
            let amount = self.roll.roll_at_least(&mut **world.rng, 0);
            world.damage.write(DamageEvent { target: *target, hit: Hit::by(landing.user, self.kind, -amount) });
        }
    }
}

impl crate::ability::FromArgs for Mend {
    const KIND: &'static str = "Mend";

    fn from_args(args: &rl_rules::ability::RawValue, look: &dyn rl_rules::ability::Lookup) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            kind: String,
            roll: String,
        }
        let a: Args = rl_rules::ability::read_args(args)?;
        Ok(Self { kind: look.damage(&a.kind).ok_or_else(|| format!("unknown damage kind {:?}", a.kind))?, roll: a.roll.parse().map_err(|e| format!("{e}"))? })
    }
}
