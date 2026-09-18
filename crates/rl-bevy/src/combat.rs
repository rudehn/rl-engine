//! Health, attacks, damage and deaths.
//!
//! The engine owns the loop: an attack intent becomes a [`DamageEvent`],
//! the game's damage stages mitigate it, health drops, a [`DeathEvent`]
//! fires, and dead non-players are removed from the world, the queue and
//! the occupancy index. What a hit is worth and what a death means beyond
//! that are the game's, read from the same messages.
//!
//! What an actor fights with is summed at the moment of the blow by
//! [`Loadout`], from the actor's own components, the components of what it
//! wears, and the stats [`CombatRules`] names. Nothing is copied onto the
//! actor when it dresses, so a game never rebuilds a wearer's attack from
//! its gear, and the resolver, the forecast and the character sheet read
//! one answer.
//!
//! Nothing here decides who strikes or knows what else a turn could be
//! spent on: the [`minds`](crate::minds) choose for monsters, and an
//! ability's damage arrives as the same [`DamageEvent`] a sword's does.

use bevy::prelude::*;
use rand::rngs::StdRng;
use rl_core::{DiceRoll, Point, RunSeed, SeedDomain, geometry};
use rl_rules::ability::Look;
use rl_rules::damage::{DamageKindId, Defender};
use rl_rules::faction::FactionDef;
use rl_rules::{DamageStage, FactionId, Factions, Hit, Registry, Relation, Resistances, StatId};

use crate::components::{Actor, Blocks, MyTurn, Player, Position};
use crate::items::{Equipped, Item};
use crate::registries::Registries;
use crate::status::StatBlock;
use crate::turn::{Action, Intent, Occupancy, Resolution, Turns};
use crate::world::WorldMap;

/// Hit points: what is left, and the most there can be.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Health {
    /// What is left.
    pub current: i32,
    /// The most.
    pub max: i32,
}

impl Health {
    /// Full health of `max`.
    pub const fn full(max: i32) -> Self {
        Self { current: max, max }
    }
}

/// Flat armor for the damage stages.
///
/// On an actor, its own hide; on an item, what wearing it adds. The two are
/// summed by [`Loadout`] at the moment a blow lands, so a jerkin is an item
/// with `Armor(1)` and nothing has to copy that onto whoever puts it on.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Armor(pub i32);

/// Resistances for the damage stages.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
pub struct Resists(pub Resistances);

/// Which side an actor is on.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Faction(pub rl_rules::FactionId);

/// What a blow does.
///
/// On an actor, its own strike: teeth, claws, a fist. On an item, what
/// wielding it strikes with; a worn item that carries one replaces the
/// wearer's own for as long as it is worn, which is what [`Loadout`]
/// answers.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeleeAttack {
    /// Damage kind.
    pub kind: DamageKindId,
    /// Damage roll.
    pub dice: DiceRoll,
    /// What one blow with it costs, in hundredths of a step. `None` is
    /// [`BASE_ACTION_COST`](rl_core::turn::BASE_ACTION_COST), so a game
    /// that does not care about weapon speed writes nothing and every
    /// blow costs a turn.
    pub cost: Option<u32>,
    /// What a blow looks like as it lands: a burst on whoever it struck.
    /// `None` shows nothing, which is how a plain blow reads: the log says
    /// it, and the target's health bar moves.
    pub look: Option<Look>,
}

impl MeleeAttack {
    /// A blow of `kind` rolling `dice`, costing an ordinary turn and
    /// showing nothing.
    ///
    /// A constructor rather than a literal, so a field that is only ever
    /// wanted by some games is added without touching every call site.
    pub const fn new(kind: DamageKindId, dice: DiceRoll) -> Self {
        Self { kind, dice, cost: None, look: None }
    }

    /// One blow costs `cost` hundredths of a step.
    pub const fn costing(mut self, cost: u32) -> Self {
        self.cost = Some(cost);
        self
    }

    /// A blow bursts on its target in `look`.
    pub const fn looking(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

/// What a shot does, and how far it reaches. An attack on a target that is
/// not adjacent uses this, if the line of fire is clear.
///
/// On an actor or on a worn item, the way [`MeleeAttack`] is.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangedAttack {
    /// Damage kind.
    pub kind: DamageKindId,
    /// Damage roll.
    pub dice: DiceRoll,
    /// Furthest cell it reaches.
    pub range: i32,
    /// What one shot with it costs, in hundredths of a step. `None` is
    /// [`BASE_ACTION_COST`](rl_core::turn::BASE_ACTION_COST). A shot that
    /// finds nothing in reach costs the ordinary turn rather than this,
    /// since what was spent was the aim.
    pub cost: Option<u32>,
    /// What flies from the shooter to the target. With one, a shot is seen
    /// to fly and, while something watches, lands when it arrives rather
    /// than when it is fired. `None` flies nothing and lands at once.
    pub look: Option<Look>,
}

impl RangedAttack {
    /// A shot of `kind` rolling `dice` out to `range`, costing an ordinary
    /// turn and flying nothing.
    pub const fn new(kind: DamageKindId, dice: DiceRoll, range: i32) -> Self {
        Self { kind, dice, range, cost: None, look: None }
    }

    /// One shot costs `cost` hundredths of a step.
    pub const fn costing(mut self, cost: u32) -> Self {
        self.cost = Some(cost);
        self
    }

    /// A shot flies in `look`.
    pub const fn looking(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

/// Extra rolls every hit carries, each its own [`DamageEvent`]: a flaming
/// blade's fire, a venomed edge's poison.
///
/// On an actor or on a worn item; every worn item's are added to the
/// wearer's own.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Strikes(pub Vec<(DamageKindId, DiceRoll)>);

/// Who is hostile to whom, and which stats a blow reads.
///
/// A rule rather than content: the sides themselves are a registry in
/// [`Registries`], and this is the matrix
/// over them. Built by naming the pairs:
///
/// ```
/// use rl_bevy::CombatRules;
/// use rl_rules::{Registry, faction::FactionDef};
///
/// let sides = Registry::from_defs(["player", "beasts", "navy", "pirates"].map(FactionDef::new).to_vec()).unwrap();
/// let side = |name| sides.expect(name);
/// let rules = CombatRules::new(&sides)
///     .hostile(side("player"), side("beasts"))
///     .hunts(side("navy"), side("pirates"));
/// assert!(rules.factions.is_hostile(side("beasts"), side("player")));
/// assert!(!rules.factions.is_hostile(side("pirates"), side("navy")), "the pirates would only rather not meet them");
/// ```
///
/// The stats are optional, and name the seam between the registered stats
/// and a blow: a status that hardens the skin or an affix that sharpens the
/// hand puts a modifier on a stat, and [`Loadout`] adds the stat's value to
/// the armor or the roll only when the game says which stat that is. A
/// game with no stats registry names neither and its armor is components
/// alone.
#[derive(Resource)]
pub struct CombatRules {
    /// Who hates whom.
    pub factions: Factions,
    /// The stat whose value is added to a defender's armor, if any.
    pub armor: Option<StatId>,
    /// The stat whose value is added to the roll of the blow and the shot,
    /// if any.
    pub attack: Option<StatId>,
    /// Whether the player's death ends the run, which it does unless the
    /// game says otherwise: one that revives, or plays on as a ghost, keeps
    /// the ending for itself.
    pub death_ends_run: bool,
}

impl CombatRules {
    /// Every side in `sides` allied with itself and neutral to every other,
    /// until a pair is named, no stat read by a blow, and the player's death
    /// the end of the run.
    pub fn new(sides: &Registry<FactionDef>) -> Self {
        Self { factions: Factions::new(sides), armor: None, attack: None, death_ends_run: true }
    }

    /// The player's death does not end the run; the game says when it ends.
    pub fn death_is_not_the_end(mut self) -> Self {
        self.death_ends_run = false;
        self
    }

    /// Reads `stat` as armor: its value is added to whatever a defender
    /// and its gear already have.
    pub fn armor_stat(mut self, stat: StatId) -> Self {
        self.armor = Some(stat);
        self
    }

    /// Reads `stat` as an attack bonus: its value is added to the roll of
    /// the blow and the shot.
    pub fn attack_stat(mut self, stat: StatId) -> Self {
        self.attack = Some(stat);
        self
    }

    /// `a` and `b` attack each other on sight.
    pub fn hostile(mut self, a: FactionId, b: FactionId) -> Self {
        self.factions.set_mutual(a, b, Relation::Hostile);
        self
    }

    /// `hunter` attacks `prey` on sight, and `prey` does not return it: a
    /// grudge need not be mutual.
    pub fn hunts(mut self, hunter: FactionId, prey: FactionId) -> Self {
        self.factions.set(hunter, prey, Relation::Hostile);
        self
    }

    /// `a` and `b` help each other.
    pub fn allied(mut self, a: FactionId, b: FactionId) -> Self {
        self.factions.set_mutual(a, b, Relation::Allied);
        self
    }
}

/// The mitigation pipeline, in order. Empty means damage lands raw.
#[derive(Resource, Default)]
pub struct DamageStages(pub Vec<Box<dyn DamageStage<Entity> + Send + Sync>>);

/// The stream combat rolls from.
///
/// Derived from the run's [`Seed`](crate::seed::Seed) by [`CombatPlugin`], so
/// a game never inserts it.
#[derive(Resource, Deref, DerefMut)]
pub struct CombatRng(pub StdRng);

impl crate::seed::Stream for CombatRng {
    fn for_run(seed: RunSeed) -> Self {
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

/// That an attack found something to strike with, and what: written once
/// per attack that lands a blow or fires a shot, before its damage.
///
/// A game hangs what a weapon does to itself on this: heat, ammunition,
/// wear. It names the worn item rather than leaving the game to work out
/// which one the loadout would have chosen, which is the engine's decision
/// and would be copied, and drift, in every game that needed it.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Struck {
    /// Who attacked.
    pub attacker: Entity,
    /// At whom.
    pub target: Entity,
    /// The worn item the attack came from; `None` for the attacker's own,
    /// a fist or a claw, which has nothing to heat or to spend.
    pub with: Option<Entity>,
    /// Whether it was a shot rather than a blow in reach.
    pub ranged: bool,
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
type DefenderData = (&'static mut Health, &'static Position, Option<&'static Resists>, Has<Player>);

/// The combat components an actor or an item may carry, as a query asks
/// for them.
type Gear = (Option<&'static Armor>, Option<&'static MeleeAttack>, Option<&'static RangedAttack>, Option<&'static Strikes>);

/// A worn item's entity beside the same components, as [`Loadout::worn`]
/// yields them.
type WornRef<'a> = (Entity, Option<&'a Armor>, Option<&'a MeleeAttack>, Option<&'a RangedAttack>, Option<&'a Strikes>);

/// What an actor fights with, summed at the moment it matters.
///
/// Three layers, added together: the actor's own [`Armor`],
/// [`MeleeAttack`], [`RangedAttack`] and [`Strikes`]; the same components
/// on every item in its [`Equipped`] slots, in slot order; and the value of
/// the stats [`CombatRules`] names, read off its [`StatBlock`]. A worn
/// blow or shot replaces the actor's own, since a cutlass is swung in
/// place of a fist; armor and extra strikes add up.
///
/// The one answer to the question. The attack resolver strikes with it,
/// [`apply_damage`] defends with it, and the inspect forecast and the
/// character sheet read it, so what a panel says a fight will cost is
/// what the fight costs. A game never copies a worn item's numbers onto
/// its wearer, which is the fold every game with gear used to write and
/// get a detail of wrong.
///
/// Every input is optional: an entity that wears nothing is its own
/// components, and a game that registered no stats or no combat rules
/// adds no stat.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Loadout<'w, 's> {
    own: Query<'w, 's, Gear>,
    worn: Query<'w, 's, Gear, With<Item>>,
    equipped: Query<'w, 's, &'static Equipped>,
    stats: Query<'w, 's, &'static StatBlock>,
    rules: Option<Res<'w, CombatRules>>,
    registries: Option<Res<'w, Registries>>,
}

impl Loadout<'_, '_> {
    /// What `who` wears, in slot order, each item's entity beside its
    /// combat components.
    fn worn(&self, who: Entity) -> impl Iterator<Item = WornRef<'_>> + '_ {
        self.equipped.get(who).ok().into_iter().flat_map(|e| e.0.worn()).filter_map(|(_, item)| {
            let (armor, melee, ranged, strikes) = self.worn.get(item).ok()?;
            Some((item, armor, melee, ranged, strikes))
        })
    }

    /// The value of the stat `pick` names on `who`, or zero when the game
    /// named none, `who` has no stats, or there is no registry to read them
    /// against.
    fn stat(&self, who: Entity, pick: impl Fn(&CombatRules) -> Option<StatId>) -> i32 {
        let (Some(rules), Some(registries)) = (self.rules.as_deref(), self.registries.as_deref()) else { return 0 };
        let Some(stat) = pick(rules) else { return 0 };
        self.stats.get(who).map_or(0, |s| s.0.value(stat, &registries.stats))
    }

    /// The flat armor a blow on `who` meets: its own, every worn item's,
    /// and the armor stat.
    pub fn armor(&self, who: Entity) -> i32 {
        let own = self.own.get(who).ok().and_then(|(armor, ..)| armor).map_or(0, |a| a.0);
        let worn: i32 = self.worn(who).filter_map(|(_, armor, ..)| armor).map(|a| a.0).sum();
        own + worn + self.stat(who, |r| r.armor)
    }

    /// The blow `who` strikes, as [`Loadout::melee`], with the worn item it
    /// comes from, or `None` when it is `who`'s own.
    pub fn melee_with(&self, who: Entity) -> Option<(Option<Entity>, MeleeAttack)> {
        let wielded = self.worn(who).find_map(|(item, _, melee, ..)| melee.copied().map(|m| (Some(item), m)));
        let (from, base) = wielded.or_else(|| self.own.get(who).ok().and_then(|(_, melee, ..)| melee.copied()).map(|m| (None, m)))?;
        let bonus = self.stat(who, |r| r.attack);
        Some((from, MeleeAttack { dice: DiceRoll { bonus: base.dice.bonus + bonus, ..base.dice }, ..base }))
    }

    /// The blow `who` strikes with: the first worn item's in slot order,
    /// or its own, with the attack stat added to the roll. `None` for
    /// something that cannot strike at all.
    pub fn melee(&self, who: Entity) -> Option<MeleeAttack> {
        self.melee_with(who).map(|(_, m)| m)
    }

    /// The shot `who` fires, as [`Loadout::ranged`], with the worn item it
    /// comes from, or `None` when it is `who`'s own.
    pub fn ranged_with(&self, who: Entity) -> Option<(Option<Entity>, RangedAttack)> {
        let wielded = self.worn(who).find_map(|(item, _, _, ranged, _)| ranged.copied().map(|r| (Some(item), r)));
        let (from, base) = wielded.or_else(|| self.own.get(who).ok().and_then(|(_, _, ranged, _)| ranged.copied()).map(|r| (None, r)))?;
        let bonus = self.stat(who, |r| r.attack);
        Some((from, RangedAttack { dice: DiceRoll { bonus: base.dice.bonus + bonus, ..base.dice }, ..base }))
    }

    /// The shot `who` fires: the first worn item's in slot order, or its
    /// own, with the attack stat added to the roll. `None` for something
    /// with nothing to shoot with.
    pub fn ranged(&self, who: Entity) -> Option<RangedAttack> {
        self.ranged_with(who).map(|(_, r)| r)
    }

    /// The extra rolls every hit by `who` carries: its own, then each worn
    /// item's in slot order.
    pub fn strikes(&self, who: Entity) -> Vec<(DamageKindId, DiceRoll)> {
        let mut all: Vec<(DamageKindId, DiceRoll)> = self.own.get(who).ok().and_then(|(_, _, _, s)| s).map(|s| s.0.clone()).unwrap_or_default();
        for (_, _, _, _, strikes) in self.worn(who) {
            all.extend(strikes.map(|s| s.0.iter().copied()).into_iter().flatten());
        }
        all
    }

    /// Every roll one blow by `who` lands, the main one first: what the
    /// forecast in [`rl_rules::forecast`] takes. Empty for something that
    /// cannot strike.
    pub fn blows(&self, who: Entity) -> Vec<(DamageKindId, DiceRoll)> {
        let mut all: Vec<(DamageKindId, DiceRoll)> = self.melee(who).map(|m| (m.kind, m.dice)).into_iter().collect();
        all.extend(self.strikes(who));
        all
    }
}

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
    attackers: Query<'w, 's, &'static Position, With<MyTurn>>,
    targets: Query<'w, 's, &'static Position, With<Health>>,
    loadout: Loadout<'w, 's>,
    struck: MessageWriter<'w, Struck>,
}

/// Turns attack intents into damage events: a melee strike on an
/// adjacent target, a shot on a distant one with a clear line of fire,
/// each followed by the attacker's extra strikes. A shot at nothing in
/// reach still costs the turn.
///
/// What is struck with is the attacker's [`Loadout`], so a worn blade is
/// swung and a worn pistol fired without either being copied onto the
/// wearer.
///
/// One turn, one strike: an attack by an actor that already acted this
/// pass finds the turn spent, whatever spent it.
pub fn resolve_attacks(
    mut intents: MessageReader<Intent<Attack>>,
    mut damage: MessageWriter<DamageEvent>,
    mut resolution: Resolution,
    mut rng: ResMut<CombatRng>,
    mut arena: Arena,
) {
    let Arena { map, occupancy, attackers, targets, loadout, struck } = &mut arena;
    for intent in intents.read() {
        let target = intent.action.0;
        let Ok(pos) = attackers.get(intent.actor) else { continue };
        if !resolution.claim(intent.actor) {
            continue;
        }
        let Ok(target_pos) = targets.get(target) else {
            resolution.done(intent.actor, rl_core::turn::BASE_ACTION_COST);
            continue;
        };
        let weapon = if geometry::is_adjacent(pos.0, target_pos.0) {
            loadout.melee_with(intent.actor).map(|(from, m)| (from, false, m.kind, m.dice, m.cost))
        } else {
            loadout
                .ranged_with(intent.actor)
                .filter(|(_, r)| line_of_fire(map, occupancy, pos.0, target_pos.0, r.range))
                .map(|(from, r)| (from, true, r.kind, r.dice, r.cost))
        };
        let mut spent = rl_core::turn::BASE_ACTION_COST;
        if let Some((from, ranged, kind, dice, cost)) = weapon {
            spent = cost.unwrap_or(rl_core::turn::BASE_ACTION_COST);
            struck.write(Struck { attacker: intent.actor, target, with: from, ranged });
            // Floored where it is rolled: a blow that rolls below zero has
            // missed, and the pipeline would read a negative one as a heal.
            let amount = dice.roll_at_least(&mut **rng, 0);
            damage.write(DamageEvent { target, hit: Hit::by(intent.actor, kind, amount) });
            for (kind, dice) in loadout.strikes(intent.actor) {
                let amount = dice.roll_at_least(&mut **rng, 0);
                damage.write(DamageEvent { target, hit: Hit::by(intent.actor, kind, amount) });
            }
        }
        resolution.done(intent.actor, spent);
    }
}

/// Where a shot from `from` at `to` flies within `range`: the cells it
/// passes through, landing included, and the cell it stops in.
///
/// It stops at `to`, or short of it at the first thing that stops
/// projectiles or anyone standing in between. [`line_of_fire`] is this
/// landing on `to`, and a targeting preview draws this same call, so what
/// the player is shown and what the resolver decides cannot disagree.
pub fn shot(map: &WorldMap, occupancy: &Occupancy, from: Point, to: Point, range: i32) -> rl_grid::Footprint {
    rl_grid::footprint(rl_grid::TargetMode::Bolt { range }, from, to, map.window_tiles(), |p| {
        p != to && (map.blocks_projectiles(p) || occupancy.is_occupied(p))
    })
}

/// Whether a shot from `from` reaches `to` within `range`: nothing that
/// stops projectiles and nobody standing in between.
pub fn line_of_fire(map: &WorldMap, occupancy: &Occupancy, from: Point, to: Point, range: i32) -> bool {
    shot(map, occupancy, from, to, range).landing == Some(to)
}

/// Runs the damage stages and applies what is left to health.
///
/// The armor a blow meets is the target's [`Loadout`]: its own, what it
/// wears, and the armor stat.
pub fn apply_damage(
    mut events: MessageReader<DamageEvent>,
    mut dealt: MessageWriter<DamageDealt>,
    mut deaths: MessageWriter<DeathEvent>,
    registries: Res<Registries>,
    stages: Res<DamageStages>,
    loadout: Loadout,
    mut targets: Query<DefenderData>,
) {
    for ev in events.read() {
        let Ok((mut health, pos, resist, is_player)) = targets.get_mut(ev.target) else { continue };
        if health.current <= 0 {
            continue;
        }
        let defender = Defender { armor: loadout.armor(ev.target), blocked: false };
        let none = Resistances::new();
        let stage_refs: Vec<&dyn DamageStage<Entity>> = stages.0.iter().map(|s| s.as_ref() as &dyn DamageStage<Entity>).collect();
        let amount = rl_rules::resolve(&ev.hit, &defender, resist.map(|r| &r.0).unwrap_or(&none), &registries.damage_kinds, &stage_refs);
        health.current = (health.current - amount).min(health.max);
        dealt.write(DamageDealt { target: ev.target, hit: ev.hit, dealt: amount });
        if health.current <= 0 {
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

/// Ends the run on the player's death, when the rules say a death does.
///
/// Inside the turn, so the run is over before the next actor acts: the
/// monster that would have struck the corpse never gets the turn.
pub fn end_run_on_player_death(mut deaths: MessageReader<DeathEvent>, rules: Res<CombatRules>, mut over: MessageWriter<crate::state::RunOver>) {
    for death in deaths.read() {
        if death.was_player && rules.death_ends_run {
            over.write(crate::state::RunOver::died(death.credit));
        }
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
/// Needs [`CombatRules`], [`Registries`] for
/// the damage kinds, and the run's [`Seed`](crate::seed::Seed) before play
/// begins, and derives [`CombatRng`] from the seed. Monsters that
/// choose whom to strike come with [`MindsPlugin`](crate::minds::MindsPlugin).
pub struct CombatPlugin;

/// Tells the mind holding the turn how far its own shot reaches.
///
/// Combat's contribution to a mind's knowledge, in
/// [`PerceiveSet::Annotate`](crate::plugin::PerceiveSet::Annotate), and
/// combat's rather than items': [`Loadout::ranged`] is the shot the
/// resolver fires, whether a worn gun's or the actor's own, and a mind
/// built with a gun needs no items to fire it. Reach asks no wits, since
/// firing what is wielded takes none, so it is read for every mind,
/// mindless or not.
pub fn perceive_reach(mut thinking: ResMut<crate::minds::Thinking>, loadout: Loadout) {
    let Some(thinker) = thinking.actor() else { return };
    let reach = loadout.ranged(thinker).map(|r| r.range);
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.reach = reach;
    }
}

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{CleanupSet, Needs, ResolveSet, Turn};
        use crate::seed::AddStream;
        use crate::turn::AddAction;
        app.add_message::<DamageEvent>()
            .add_message::<DamageDealt>()
            .add_message::<DeathEvent>()
            .add_message::<Struck>()
            .init_resource::<DamageStages>()
            .add_action::<Attack>()
            .needs::<CombatRules>("CombatPlugin", "`CombatRules::new(&sides)`, who is hostile to whom")
            .needs::<crate::registries::Registries>("CombatPlugin", "`Registries`, with the damage kinds a blow can deal")
            .add_stream::<CombatRng>("CombatPlugin")
            .add_systems(Turn, perceive_reach.in_set(crate::plugin::PerceiveSet::Annotate))
            .add_systems(Turn, resolve_attacks.in_set(ResolveSet::Act))
            .add_systems(Turn, apply_damage.in_set(ResolveSet::Damage))
            .add_systems(Turn, end_run_on_player_death.in_set(crate::plugin::TurnSet::React))
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
                RangedAttack::new(blunt, DiceRoll::flat(3), 6),
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
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 20 - 3 - 2, "the shot and the extra strike both landed");
        // A wall in between stops the next shot; the turn is still spent.
        app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(2, 0), TileId(1));
        app.update();
        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 15, "the wall took the shot");
        assert!(app.world().resource::<Turns>().now() > before);
        // Out of range is no shot either.
        app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(2, 0), TileId(0));
        let far = app.world_mut().spawn((Actor, Blocks, Position(start.offset(7, 0)), Health::full(20), Faction(them))).id();
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Attack(far)));
        app.update();
        assert_eq!(app.world().get::<Health>(far).unwrap().current, 20);
    }

    /// A call site names only what it cares about: `new` leaves every
    /// optional field unset, and each builder sets the one it names.
    #[test]
    fn a_new_attack_leaves_every_optional_field_unset_and_each_builder_sets_its_own() {
        let kind = DamageKindId::from_raw(0);
        let look = rl_rules::ability::Look { glyph: '*', color: rl_grid::Rgb::new(255, 80, 40) };
        let blow = MeleeAttack::new(kind, DiceRoll::flat(2));
        assert_eq!((blow.kind, blow.dice, blow.cost, blow.look), (kind, DiceRoll::flat(2), None, None));
        let blow = blow.costing(70).looking(look);
        assert_eq!((blow.cost, blow.look), (Some(70), Some(look)));

        let shot = RangedAttack::new(kind, DiceRoll::flat(3), 6);
        assert_eq!((shot.kind, shot.dice, shot.range, shot.cost, shot.look), (kind, DiceRoll::flat(3), 6, None, None));
        let shot = shot.costing(140).looking(look);
        assert_eq!((shot.cost, shot.look), (Some(140), Some(look)));
    }

    /// What `who` fights with, as a panel or a resolver would ask.
    fn loadout(app: &mut App, who: Entity) -> (i32, Option<DiceRoll>, Vec<(DamageKindId, DiceRoll)>) {
        let mut state: bevy::ecs::system::SystemState<Loadout> = bevy::ecs::system::SystemState::new(app.world_mut());
        let loadout = state.get(app.world()).expect("every input is optional");
        (loadout.armor(who), loadout.melee(who).map(|m| m.dice), loadout.strikes(who))
    }

    /// Armor is three layers summed at the blow: the defender's own hide,
    /// the coat it wears, and the stat a status hardened. Nothing was
    /// copied onto the defender to make that so, and the coat's share
    /// leaves with the coat.
    #[test]
    fn a_blow_meets_the_defenders_own_armor_its_gear_and_the_armor_stat_together() {
        use rl_rules::stats::Op;
        use rl_rules::{EquipShape, Equipment, SlotId, StatDef, StatusDef};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::status::StatusPlugin, crate::items::ItemsPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let stats = Registry::from_defs(vec![StatDef::new("armor", 0)]).unwrap();
        let armor_stat = stats.expect("armor");
        let statuses = Registry::from_defs(vec![StatusDef::new("hardened").modifies(armor_stat, Op::Add(3))]).unwrap();
        let hardened = statuses.expect("hardened");
        {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.stats = stats;
            registries.statuses = statuses;
            app.world_mut().resource_mut::<CombatRules>().armor = Some(armor_stat);
        }
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(sides.ours),
                MeleeAttack::new(sides.kind, DiceRoll::flat(10)),
            ))
            .id();
        let slot = SlotId::from_raw(0);
        let coat = app.world_mut().spawn((crate::items::Item, Armor(2), crate::items::Wearable(EquipShape::in_slot(slot)))).id();
        let mut worn = Equipment::with_slot_count(1);
        worn.equip(coat, &EquipShape::in_slot(slot)).unwrap();
        let target = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(1, 0)),
                Health::full(50),
                Armor(1),
                Faction(sides.theirs),
                crate::items::Inventory { items: vec![coat] },
                crate::items::Equipped(worn),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(crate::status::Afflict { target, status: hardened, turns: 9, by: None });
        app.update();
        assert_eq!(loadout(&mut app, target).0, 1 + 2 + 3, "hide, coat and status");

        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 50 - (10 - 6), "ten less six armor");
        assert_eq!(app.world().get::<Armor>(target).map(|a| a.0), Some(1), "and its own hide is still all it carries");

        app.world_mut().get_mut::<crate::items::Equipped>(target).unwrap().unequip(coat);
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 46 - (10 - 4), "the coat's share left with the coat");
    }

    /// A worn blade is swung in place of the fist, a worn pistol fired, and
    /// every worn item's extra strikes land beside the blow, with the attack
    /// stat on the roll only when the game names one.
    #[test]
    fn a_worn_blade_replaces_the_fist_and_worn_strikes_add_up() {
        use rl_rules::{EquipShape, Equipment, SlotId, StatDef};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let stats = Registry::from_defs(vec![StatDef::new("attack", 2)]).unwrap();
        let attack_stat = stats.expect("attack");
        app.world_mut().resource_mut::<Registries>().stats = stats;
        let (hand, off) = (SlotId::from_raw(0), SlotId::from_raw(1));
        let blade = app
            .world_mut()
            .spawn((
                crate::items::Item,
                MeleeAttack::new(sides.kind, DiceRoll::new(2, 6)),
                Strikes(vec![(sides.kind, DiceRoll::flat(1))]),
                crate::items::Wearable(EquipShape::in_slot(hand)),
            ))
            .id();
        let pistol = app
            .world_mut()
            .spawn((
                crate::items::Item,
                RangedAttack::new(sides.kind, DiceRoll::flat(4), 5),
                Strikes(vec![(sides.kind, DiceRoll::flat(2))]),
                crate::items::Wearable(EquipShape::in_slot(off)),
            ))
            .id();
        let mut worn = Equipment::with_slot_count(2);
        worn.equip(blade, &EquipShape::in_slot(hand)).unwrap();
        worn.equip(pistol, &EquipShape::in_slot(off)).unwrap();
        let fist = MeleeAttack::new(sides.kind, DiceRoll::new(1, 3));
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(sides.ours),
                fist,
                Strikes(vec![(sides.kind, DiceRoll::flat(3))]),
                crate::items::Inventory { items: vec![blade, pistol] },
                crate::items::Equipped(worn),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();

        let (_, blow, strikes) = loadout(&mut app, player);
        assert_eq!(blow, Some(DiceRoll::new(2, 6)), "the blade, not the fist, and no bonus while no stat is named");
        assert_eq!(strikes.iter().map(|(_, d)| d.bonus).collect::<Vec<_>>(), vec![3, 1, 2], "its own strike, then the blade's, then the pistol's");

        app.world_mut().resource_mut::<CombatRules>().attack = Some(attack_stat);
        let (_, blow, _) = loadout(&mut app, player);
        assert_eq!(blow, Some(DiceRoll { num: 2, sides: 6, bonus: 2 }), "the attack stat on the roll");

        // Fired: the pistol's flat four plus two, and the three extra strikes.
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(40), Faction(sides.theirs))).id();
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 40 - (4 + 2) - (3 + 1 + 2));

        app.world_mut().get_mut::<crate::items::Equipped>(player).unwrap().unequip(blade);
        let (_, blow, strikes) = loadout(&mut app, player);
        assert_eq!(blow, Some(DiceRoll { num: 1, sides: 3, bonus: 2 }), "the fist again, still with the stat");
        assert_eq!(strikes.len(), 2, "the blade's strike went with it");
    }

    /// Charges one melee blow with `cost` and answers what the turn cost.
    ///
    /// Both actors start the clock at zero, so once the player's blow and
    /// the target's stranded wait (it has no mind to act with) have both
    /// been charged, the clock reads exactly what the player's turn cost:
    /// [`run_turns`](crate::plugin::run_turns) keeps running passes until
    /// the player holds a turn again, which happens the instant the clock
    /// reaches it.
    fn melee_turn_cost(cost: Option<u32>) -> u32 {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(sides.ours),
                MeleeAttack { cost, ..MeleeAttack::new(sides.kind, DiceRoll::flat(1)) },
            ))
            .id();
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(1, 0)), Health::full(20), Faction(sides.theirs))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        app.world().resource::<Turns>().now()
    }

    /// A weapon's cost and the actor's speed compose by scaling the cost
    /// once, not by pre-scaling the weapon and applying speed a second time:
    /// `scaled_cost(70, 200)` is 35, and nothing in the resolver or the
    /// scheduler may charge less or more than that single scaling gives.
    #[test]
    fn a_weapons_cost_and_the_actors_speed_compose_by_scaling_the_cost_once() {
        use crate::components::Speed;
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(sides.ours),
                Speed(200),
                MeleeAttack::new(sides.kind, DiceRoll::flat(1)).costing(70),
            ))
            .id();
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(1, 0)), Health::full(20), Faction(sides.theirs))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), 35, "scaled_cost(70, 200) is 35: the weapon's cost scaled by speed exactly once");
    }

    #[test]
    fn a_blow_costs_what_its_weapon_says_and_an_ordinary_turn_when_it_says_nothing() {
        // Identical blows but for what the weapon charges: 70 hundredths of a
        // step against 140, and nothing stated. A weapon that charges half as
        // much comes round twice as often, which is the whole of weapon speed.
        let quick = melee_turn_cost(Some(70));
        let heavy = melee_turn_cost(Some(140));
        let plain = melee_turn_cost(None);
        assert_eq!(quick, 70, "the weapon's cost is what the turn charges");
        assert_eq!(heavy, 140);
        assert_eq!(plain, rl_core::turn::BASE_ACTION_COST, "no cost stated is the ordinary cost");
    }

    /// Fires one ranged attack with `cost` and `range` at a target `offset`
    /// cells east, and answers what the turn cost.
    ///
    /// Mirrors [`melee_turn_cost`]: both actors start the clock at zero, so
    /// once the shot (or the miss) and the target's stranded wait have both
    /// been charged, the clock reads exactly what the shooter's turn cost.
    fn shot_turn_cost(cost: Option<u32>, range: i32, offset: i32) -> u32 {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(sides.ours),
                RangedAttack { cost, ..RangedAttack::new(sides.kind, DiceRoll::flat(1), range) },
            ))
            .id();
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(offset, 0)), Health::full(20), Faction(sides.theirs))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        app.world().resource::<Turns>().now()
    }

    #[test]
    fn a_shot_charges_the_weapons_cost_and_a_shot_at_nothing_still_costs_a_turn() {
        // A slow weapon costs more than a fast one, and a shot with no
        // line of fire costs the ordinary turn: the shooter spent it aiming.
        let slow = shot_turn_cost(Some(140), 6, 4);
        let fast = shot_turn_cost(Some(80), 6, 4);
        let missed = shot_turn_cost(Some(140), 3, 4);
        assert_eq!(slow, 140);
        assert_eq!(fast, 80);
        assert_eq!(missed, rl_core::turn::BASE_ACTION_COST, "out of range is a spent turn, not a free one");
    }

    #[derive(Resource, Default)]
    struct Heard(Vec<Struck>);

    /// Copies every [`Struck`] into [`Heard`], the way [`status::hear`] does
    /// for [`StatusEvent`](crate::status::StatusEvent): a headless app swaps
    /// its message buffers on wall time, so peeking at the buffer after an
    /// update can miss what a reader added this run would have caught.
    fn hear(mut events: MessageReader<Struck>, mut heard: ResMut<Heard>) {
        heard.0.extend(events.read().copied());
    }

    /// Sends one melee attack from a player adjacent to a target, worn or
    /// bare-handed, and returns the single [`Struck`] written for it.
    ///
    /// Mirrors [`melee_turn_cost`]'s setup, with `ItemsPlugin` added so a
    /// worn weapon can be equipped.
    fn struck_by_melee(worn: bool) -> Struck {
        use rl_rules::{EquipShape, Equipment, SlotId};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin, crate::world::StreamingPlugin));
        app.init_resource::<Heard>().add_systems(PostUpdate, hear);
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let mut player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(sides.ours)));
        if !worn {
            player.insert(MeleeAttack::new(sides.kind, DiceRoll::flat(1)));
        }
        let player = player.id();
        if worn {
            let slot = SlotId::from_raw(0);
            let blade = app
                .world_mut()
                .spawn((crate::items::Item, MeleeAttack::new(sides.kind, DiceRoll::flat(1)), crate::items::Wearable(EquipShape::in_slot(slot))))
                .id();
            let mut equipment = Equipment::with_slot_count(1);
            equipment.equip(blade, &EquipShape::in_slot(slot)).unwrap();
            app.world_mut().entity_mut(player).insert((crate::items::Inventory { items: vec![blade] }, Equipped(equipment)));
        }
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(1, 0)), Health::full(20), Faction(sides.theirs))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        let struck = app.world().resource::<Heard>().0.clone();
        assert_eq!(struck.len(), 1, "one attack, one Struck");
        struck[0]
    }

    /// Equips two ranged items in slot order on a player three tiles from a
    /// clear-line target, fires one attack, and returns the [`Struck`]
    /// written along with both item entities in slot order.
    fn struck_by_two_guns() -> (Struck, Entity, Entity) {
        use rl_rules::{EquipShape, Equipment, SlotId};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin, crate::world::StreamingPlugin));
        app.init_resource::<Heard>().add_systems(PostUpdate, hear);
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let (first_slot, second_slot) = (SlotId::from_raw(0), SlotId::from_raw(1));
        let first = app
            .world_mut()
            .spawn((crate::items::Item, RangedAttack::new(sides.kind, DiceRoll::flat(3), 6), crate::items::Wearable(EquipShape::in_slot(first_slot))))
            .id();
        let second = app
            .world_mut()
            .spawn((crate::items::Item, RangedAttack::new(sides.kind, DiceRoll::flat(3), 6), crate::items::Wearable(EquipShape::in_slot(second_slot))))
            .id();
        let mut equipment = Equipment::with_slot_count(2);
        equipment.equip(first, &EquipShape::in_slot(first_slot)).unwrap();
        equipment.equip(second, &EquipShape::in_slot(second_slot)).unwrap();
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(8),
                Health::full(30),
                Faction(sides.ours),
                crate::items::Inventory { items: vec![first, second] },
                Equipped(equipment),
            ))
            .id();
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(20), Faction(sides.theirs))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        let struck = app.world().resource::<Heard>().0.clone();
        assert_eq!(struck.len(), 1, "one attack, one Struck");
        (struck[0], first, second)
    }

    #[test]
    fn a_blow_names_the_worn_weapon_it_came_from_and_nothing_when_it_came_from_the_attacker() {
        // Worn: the blow names the item. Bare-handed: it names nothing. A game
        // that heats or spends a weapon learns which one from this, rather than
        // working out for itself which item the engine would have picked.
        let (worn, bare) = (struck_by_melee(true), struck_by_melee(false));
        assert!(worn.with.is_some(), "a worn weapon's blow names it");
        assert!(!worn.ranged);
        assert_eq!(bare.with, None, "a bare-handed blow names nothing");
    }

    #[test]
    fn a_shot_names_the_item_it_was_fired_from_and_the_one_loadout_would_pick() {
        // Two ranged items worn: the shot names whichever the loadout picks,
        // which is the first in slot order, so the report and the resolver can
        // never disagree about which weapon fired.
        let (struck, first, _second) = struck_by_two_guns();
        assert!(struck.ranged);
        assert_eq!(struck.with, Some(first));
    }
}
