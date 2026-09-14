//! Health, attacks, damage and deaths.
//!
//! The engine owns the loop: an attack intent becomes a [`DamageEvent`],
//! the game's damage stages mitigate it, health drops, a [`DeathEvent`]
//! fires, and dead non-players are removed from the world, the queue and
//! the occupancy index. What a hit is worth and what a death means beyond
//! that are the game's, read from the same messages.
//!
//! Nothing here decides who strikes or knows what else a turn could be
//! spent on: the [`minds`](crate::minds) choose for monsters, and an
//! ability's damage arrives as the same [`DamageEvent`] a sword's does.

use bevy::prelude::*;
use rand::rngs::StdRng;
use rl_core::{DiceRoll, Point, RunSeed, SeedDomain, geometry};
use rl_rules::Registry;
use rl_rules::damage::{DamageKind, DamageKindId, Defender};
use rl_rules::{DamageStage, Factions, Hit, Resistances};

use crate::components::{Actor, Blocks, MyTurn, Player, Position};
use crate::turn::{Action, Intent, Occupancy, Resolution, Turns};
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

/// A defender as the damage system sees it.
type DefenderData = (&'static mut Health, &'static Position, Option<&'static Armor>, Option<&'static Resists>, Has<Player>);

/// Strike an actor: adjacent with a melee weapon, at range with a ranged
/// one down a clear line of fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attack(pub Entity);
impl Action for Attack {}

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
/// pipeline and deaths.
///
/// Needs [`CombatRules`] and [`CombatRng`] before play begins. Monsters that
/// choose whom to strike come with [`MindsPlugin`](crate::minds::MindsPlugin).
pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{CleanupSet, Needs, ResolveSet, Turn};
        use crate::turn::AddAction;
        app.add_message::<DamageEvent>()
            .add_message::<DamageDealt>()
            .add_message::<DeathEvent>()
            .init_resource::<DamageStages>()
            .add_action::<Attack>()
            .needs::<CombatRules>("CombatPlugin", "`CombatRules { kinds, factions }`, a registry of damage kinds and a faction matrix")
            .needs::<CombatRng>("CombatPlugin", "`CombatRng::for_run(seed)`, the stream combat rolls from")
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Viewshed;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use rl_grid::TileId;

    fn arena() -> (App, Point, DamageKindId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        (app, start, sides.kind)
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
