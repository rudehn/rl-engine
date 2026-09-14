//! Abilities: the loop that turns a definition into something that
//! happened.
//!
//! [`rl_rules::ability`] holds what an ability *is* and answers whether it
//! may be used. This module owns the rest: the [`Use`] action, the
//! resolver that gates it, pays for it, resolves its footprint and lands
//! its effects, and the state a use spends and sets.
//!
//! Effects are types, not a list, for the reason actions are: the engine
//! ships one per subsystem it owns, each living in the module that owns
//! the mechanic, and a game registers its own with
//! [`AddEffect::add_effect`]. There is no enum of effect kinds, no
//! `Custom { id }`, and no list anywhere for a new one to be added to. An
//! effect a game writes reaches the world through [`EffectWorld`], which
//! hands out the engine's own requests and, for anything the engine never
//! thought of, `Commands`.
//!
//! Cooldowns are absolute times on the turn queue's clock rather than
//! countdowns, so a save that restores the clock restores every cooldown
//! with it and nothing has to be ticked.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rand::Rng;
use rand::rngs::StdRng;
use rl_core::{Point, RunSeed, SeedDomain};
use rl_grid::{Footprint, footprint};
use rl_rules::ability::{AbilityDef, AbilityId, Aim, Blocked, Cost, Gates, Lookup, Purse, RawValue, Usable, aim_blocked, blocked};
use rl_rules::{Registry, Relation, StatId, Statuses, TagId};

use crate::combat::{CombatRules, Dead, Faction, Health};
use crate::components::{Blocks, MyTurn, Position, Viewshed};
use crate::items::{Equipped, Inventory, Stack, Tagged};
use crate::plugin::{ResolveSet, Turn, TurnSet};
use crate::status::{Afflict, Afflicted, Cure, StatBlock, StatRules};
use crate::turn::{Action, AddAction, Intent, Occupancy, Resolution, Turns};
use crate::world::WorldMap;

/// Spend a turn on an ability, pointed at a cell.
///
/// The aim is a cell rather than an actor because most shapes land on
/// ground: a ball bursts where it falls whether or not anyone was standing
/// there. An ability whose [`Aim`] does not need a cursor is aimed at the
/// user's own cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Use {
    /// Which ability.
    pub ability: AbilityId,
    /// Where it is pointed.
    pub aim: Point,
}
impl Action for Use {}

/// What an actor can use, and what lent it.
///
/// A map rather than a list: two wands granting the same ability is one
/// entry, and the source is what a charge is spent from. Rebuilt rather
/// than edited by [`refresh_known`], so an unequipped wand takes its
/// ability with it and nothing has to remember that it did.
#[derive(Component, Debug, Clone, Default)]
pub struct Known(BTreeMap<AbilityId, Option<Entity>>);

impl Known {
    /// An actor that knows nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `ability`, lent by `source` if something lent it. A second
    /// grant of the same ability keeps the first source, so an ability
    /// known innately is not made to depend on a wand.
    pub fn learn(&mut self, ability: AbilityId, source: Option<Entity>) {
        self.0.entry(ability).or_insert(source);
    }

    /// Forgets everything.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Whether `ability` is known.
    pub fn has(&self, ability: AbilityId) -> bool {
        self.0.contains_key(&ability)
    }

    /// What lent `ability`, if anything did.
    pub fn source_of(&self, ability: AbilityId) -> Option<Entity> {
        self.0.get(&ability).copied().flatten()
    }

    /// Every ability known, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (AbilityId, Option<Entity>)> + '_ {
        self.0.iter().map(|(a, s)| (*a, *s))
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing is known.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// What an actor has in its pools right now.
///
/// The stat says how large a pool may be; this says what is in it. Two
/// values rather than one because gear that raises maximum mana should not
/// also fill it. A `BTreeMap` and not a hash: gameplay paths in this engine
/// are ordered containers, so a run is reproducible from its seed.
#[derive(Component, Debug, Clone, Default)]
pub struct Pools(BTreeMap<StatId, i32>);

impl Pools {
    /// Empty pools: every one reads zero until it is filled.
    pub fn new() -> Self {
        Self::default()
    }

    /// What is in `stat`.
    pub fn get(&self, stat: StatId) -> i32 {
        self.0.get(&stat).copied().unwrap_or(0)
    }

    /// Sets `stat` to `value`.
    pub fn set(&mut self, stat: StatId, value: i32) {
        self.0.insert(stat, value);
    }

    /// Adds `amount`, never above `max`, and returns the new value.
    pub fn fill(&mut self, stat: StatId, amount: i32, max: i32) -> i32 {
        let now = (self.get(stat) + amount).min(max);
        self.set(stat, now);
        now
    }

    /// Takes `amount` off, never below zero.
    pub fn spend(&mut self, stat: StatId, amount: i32) {
        let now = (self.get(stat) - amount).max(0);
        self.set(stat, now);
    }

    /// Every pool with something in it, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (StatId, i32)> + '_ {
        self.0.iter().map(|(s, v)| (*s, *v))
    }
}

/// When each of an actor's abilities is ready again, on the turn queue's
/// clock.
#[derive(Component, Debug, Clone, Default)]
pub struct Cooldowns(BTreeMap<AbilityId, u32>);

impl Cooldowns {
    /// Nothing cooling.
    pub fn new() -> Self {
        Self::default()
    }

    /// When `ability` is ready, in hundredths on the turn clock.
    pub fn ready_at(&self, ability: AbilityId) -> u32 {
        self.0.get(&ability).copied().unwrap_or(0)
    }

    /// Marks `ability` unusable until `time`.
    pub fn set(&mut self, ability: AbilityId, time: u32) {
        self.0.insert(ability, time);
    }

    /// Every cooldown, for saving.
    pub fn iter(&self) -> impl Iterator<Item = (AbilityId, u32)> + '_ {
        self.0.iter().map(|(a, t)| (*a, *t))
    }
}

/// The abilities an item, an affix or a status lends whoever holds it.
#[derive(Component, Debug, Clone, Default)]
pub struct Grants(pub Vec<AbilityId>);

/// Uses left in whatever carries this.
///
/// On the item that granted the ability, not on the actor, so two wands
/// are two pools of charges and a used-up one is still a wand.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Charges {
    /// How many are left.
    pub left: u16,
    /// How many it holds when full.
    pub max: u16,
}

impl Charges {
    /// A full complement of `max`.
    pub fn full(max: u16) -> Self {
        Self { left: max, max }
    }
}

/// What happened when an ability was used.
#[derive(Message, Debug, Clone)]
pub enum AbilityEvent {
    /// It landed.
    Used {
        /// Who used it.
        user: Entity,
        /// Which.
        ability: AbilityId,
        /// Where it was pointed.
        aim: Point,
        /// Everyone under the footprint that the aim wanted there.
        targets: Vec<Entity>,
    },
    /// It could not be used, for every reason at once.
    Refused {
        /// Who tried.
        user: Entity,
        /// Which.
        ability: AbilityId,
        /// Why not.
        why: Vec<Blocked>,
    },
}

/// The stream abilities roll from.
///
/// Its own domain, so adding an ability does not shift the combat stream
/// and change every monster's rolls in a run that was going fine.
#[derive(Resource, Deref, DerefMut)]
pub struct AbilityRng(pub StdRng);

impl AbilityRng {
    /// The stream for `seed`.
    pub fn for_run(seed: RunSeed) -> Self {
        Self(seed.rng(SeedDomain::new(b"ability"), 0))
    }
}

/// One use, resolved: where it went and what was under it.
#[derive(Debug, Clone)]
pub struct Landing {
    /// Who used it.
    pub user: Entity,
    /// Which ability.
    pub ability: AbilityId,
    /// Where the user stood.
    pub origin: Point,
    /// Where it was pointed.
    pub aim: Point,
    /// Every cell it covers.
    pub cells: Vec<Point>,
    /// The cells a projectile flew through, landing included.
    pub path: Vec<Point>,
    /// Where a projectile stopped, if the shape had one.
    pub landed_at: Option<Point>,
    /// Everyone under the footprint the ability's [`Aim`] wanted there.
    /// An effect that means to hit whoever is standing in the fire reads
    /// this; one that means to change the ground reads `cells`.
    pub targets: Vec<Entity>,
}

/// What an effect may do to the world.
///
/// Requests, not changes: an effect asks for damage, a status or a move,
/// and the systems that already own those answer. That is what keeps an
/// ability's damage going through the same mitigation a sword's does.
/// `commands` is the escape hatch, and it is a real one: an effect a game
/// writes can do anything a system can, including write its own messages.
#[derive(bevy::ecs::system::SystemParam)]
pub struct EffectWorld<'w, 's> {
    /// Anything the engine did not think of.
    pub commands: Commands<'w, 's>,
    /// Damage, through the pipeline that mitigates it.
    pub damage: MessageWriter<'w, crate::combat::DamageEvent>,
    /// A status on.
    pub afflict: MessageWriter<'w, Afflict>,
    /// A status off.
    pub cure: MessageWriter<'w, Cure>,
    /// The ability stream, for an effect that rolls.
    pub rng: ResMut<'w, AbilityRng>,
    actors: Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>, Has<Blocks>)>,
    occupancy: ResMut<'w, Occupancy>,
    map: Res<'w, WorldMap>,
}

impl EffectWorld<'_, '_> {
    /// Where `e` is, if it is anywhere.
    pub fn position(&self, e: Entity) -> Option<Point> {
        self.actors.get(e).ok().map(|(p, _, _)| p.0)
    }

    /// Whether `who` can see the cell `p`, or `None` when it carries no
    /// viewshed and the question cannot be asked of it.
    ///
    /// Most non-players carry none: the minds read the player's viewshed
    /// as the oracle, since lines of sight are symmetric. So a monster is
    /// not held to a sight requirement it has no way to answer, and the
    /// tactic that chose the aim is trusted instead.
    ///
    /// Asked here rather than through a query of its own because this one
    /// already holds every viewshed, and two queries over the same
    /// component, one of them writing, is a system Bevy refuses to run.
    pub fn sight_of(&self, who: Entity, p: Point) -> Option<bool> {
        let (_, viewshed, _) = self.actors.get(who).ok()?;
        Some(viewshed?.can_see(p))
    }

    /// Whether `p` is somewhere an actor could stand right now.
    pub fn is_free(&self, p: Point) -> bool {
        self.map.is_walkable(p) && !self.occupancy.is_occupied(p)
    }

    /// Puts `e` on `p`, keeping the occupancy index and its sight
    /// straight. False when the cell will not take it.
    pub fn place(&mut self, e: Entity, p: Point) -> bool {
        if !self.is_free(p) {
            return false;
        }
        let Ok((mut pos, viewshed, blocks)) = self.actors.get_mut(e) else { return false };
        let from = pos.0;
        if from == p {
            return true;
        }
        if blocks {
            self.occupancy.relocate(e, from, p);
        }
        pos.0 = p;
        if let Some(mut v) = viewshed {
            v.dirty = true;
        }
        true
    }

    /// Slides `e` up to `cells` steps along the line from `from` to `to`,
    /// stopping at the last cell it can stand on. Returns where it ended.
    ///
    /// A push and a pull are the same walk with the ends swapped, so both
    /// stop at a wall rather than through it.
    pub fn slide(&mut self, e: Entity, from: Point, to: Point, cells: i32) -> Point {
        let (dx, dy) = ((to.x - from.x).signum(), (to.y - from.y).signum());
        if dx == 0 && dy == 0 {
            return from;
        }
        let mut at = from;
        for _ in 0..cells.max(0) {
            let next = at.offset(dx, dy);
            if !self.is_free(next) {
                break;
            }
            at = next;
        }
        if at != from {
            self.place(e, at);
        }
        at
    }
}

/// One thing an ability does when it lands.
///
/// A type per effect, registered with [`AddEffect::add_effect`], parsing
/// its own arguments out of the RON the ability named it with. The engine
/// ships one per subsystem it owns; a game's own sit beside them and the
/// resolver cannot tell the difference.
pub trait Effect: Send + Sync + 'static {
    /// What this does to `landing`.
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>);
}

/// An effect that knows how to build itself out of an ability's RON.
///
/// Separate from [`Effect`] so the trait a game writes stays object-safe
/// and the constructor can fail with a message naming what was wrong.
pub trait FromArgs: Effect + Sized {
    /// The name abilities call this effect by in RON.
    const KIND: &'static str;

    /// Builds one from the arguments, resolving any names through `look`.
    fn from_args(args: &RawValue, look: &dyn Lookup) -> Result<Self, String>;
}

/// How an effect is built, once its name has been matched.
type Builder = fn(&RawValue, &dyn Lookup) -> Result<Box<dyn Effect>, String>;

/// Every effect kind a game has registered.
///
/// Filled while the app is built and read once, when abilities load, so an
/// unknown effect name is a startup failure naming the ability rather than
/// a surprise the first time someone presses the key.
#[derive(Resource, Default)]
pub struct EffectKinds(BTreeMap<String, Builder>);

impl EffectKinds {
    /// Registers `E` under its own name.
    pub fn declare<E: FromArgs>(&mut self) {
        self.0.insert(E::KIND.to_string(), |args, look| E::from_args(args, look).map(|e| Box::new(e) as Box<dyn Effect>));
    }

    /// Whether `kind` is registered.
    pub fn has(&self, kind: &str) -> bool {
        self.0.contains_key(kind)
    }

    /// Every registered name, in order, for an error that lists them.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(|s| s.as_str())
    }
}

/// Registers an effect with the engine.
pub trait AddEffect {
    /// Registers `E` so abilities may name it in their RON.
    fn add_effect<E: FromArgs>(&mut self) -> &mut Self;
}

impl AddEffect for App {
    fn add_effect<E: FromArgs>(&mut self) -> &mut Self {
        self.init_resource::<EffectKinds>();
        self.world_mut().resource_mut::<EffectKinds>().declare::<E>();
        self
    }
}

/// One effect of one ability, built.
struct Built {
    chance: u8,
    effect: Box<dyn Effect>,
}

/// The abilities a game registered, with their effects built.
#[derive(Resource)]
pub struct Abilities {
    defs: Registry<AbilityDef>,
    /// Parallel to `defs` by id index.
    built: Vec<Vec<Built>>,
}

impl Abilities {
    /// Builds the effects of every ability in `defs` through `kinds`.
    ///
    /// Fails naming the ability, the effect and what was wrong with it,
    /// and lists every failure rather than the first.
    pub fn build(defs: Registry<AbilityDef>, kinds: &EffectKinds, look: &dyn Lookup) -> Result<Self, rl_rules::ContentError> {
        let mut errors = Vec::new();
        let mut built = Vec::new();
        for (_, def) in defs.iter() {
            let mut mine = Vec::new();
            for spec in &def.effects {
                match kinds.0.get(&spec.kind) {
                    None => {
                        let known: Vec<&str> = kinds.names().collect();
                        errors.push(format!("{}: no effect is registered as {:?}; registered: {}", def.name, spec.kind, known.join(", ")));
                    }
                    Some(build) => match build(&spec.args, look) {
                        Ok(effect) => mine.push(Built { chance: spec.chance, effect }),
                        Err(e) => errors.push(format!("{}: effect {:?}: {e}", def.name, spec.kind)),
                    },
                }
            }
            built.push(mine);
        }
        if !errors.is_empty() {
            return Err(rl_rules::ContentError::Invalid(errors));
        }
        Ok(Self { defs, built })
    }

    /// The definitions.
    pub fn defs(&self) -> &Registry<AbilityDef> {
        &self.defs
    }

    /// One definition.
    pub fn get(&self, id: AbilityId) -> &AbilityDef {
        self.defs.get(id)
    }

    /// The id named `name`, if there is one.
    pub fn id(&self, name: &str) -> Option<AbilityId> {
        self.defs.id(name)
    }

    /// The id named `name`. Panics naming it when there is none, for the
    /// setup code that would only unwrap.
    pub fn expect(&self, name: &str) -> AbilityId {
        self.defs.expect(name)
    }
}

/// What a user spends, as the resolver reads it.
type Spender = (&'static Known, Option<&'static mut Pools>, Option<&'static mut Cooldowns>, Option<&'static mut Health>);
/// What a user carries and how it feels, as the gate reads it.
type Bearing = (Option<&'static Inventory>, Option<&'static Equipped>, Option<&'static Afflicted>, Option<&'static StatBlock>);

/// What the resolver reads about the user, apart from where it stands.
#[derive(bevy::ecs::system::SystemParam)]
pub struct UserState<'w, 's> {
    users: Query<'w, 's, Spender, With<MyTurn>>,
    gear: Query<'w, 's, Bearing>,
    charges: Query<'w, 's, &'static mut Charges>,
    tagged: Query<'w, 's, (Option<&'static Tagged>, Option<&'static mut Stack>)>,
    stats: Res<'w, StatRules>,
    commands: Commands<'w, 's>,
}

/// A use being worked out: who, which, from where, and at what.
#[derive(Debug, Clone, Copy)]
pub struct Aimed<'a> {
    /// Who is using it.
    pub user: Entity,
    /// Which ability.
    pub ability: AbilityId,
    /// Its definition.
    pub def: &'a AbilityDef,
    /// Where the user stands.
    pub origin: Point,
    /// Where it is pointed: the user's own cell for an ability that needs
    /// no cursor.
    pub aim: Point,
    /// Whether the user can see `aim`, `None` when it carries no sight to
    /// ask. Most minds carry none and are trusted with their tactic's pick.
    pub sees_aim: Option<bool>,
}

/// Where a use lands, and whether the aim holds.
#[derive(Debug, Clone)]
pub struct Landed {
    /// The footprint and who is under it.
    pub landing: Landing,
    /// Every reason this aim would be refused, from
    /// [`aim_blocked`]; empty when it holds.
    pub refused: Vec<Blocked>,
}

/// Who is standing in a footprint and how they stand to the user.
///
/// Public because the resolver is not the only one asking. The targeting
/// cursor previews a use through the same [`Bystanders::land`], so the
/// cells it paints and the names in its banner are the ones that will be
/// hit: a preview that works that out some other way is a preview that
/// drifts from the rules.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Bystanders<'w, 's> {
    factions: Query<'w, 's, &'static Faction>,
    alive: Query<'w, 's, (), (With<Health>, Without<Dead>)>,
    rules: Option<Res<'w, CombatRules>>,
}

impl Bystanders<'_, '_> {
    /// How `other` stands towards `user`.
    ///
    /// The user is its own ally whatever the matrix says. Otherwise `None`
    /// where the question cannot be asked, because either side has no
    /// faction or the game registered no matrix, which is why an ability
    /// in a game with no factions should aim at [`Aim::Ground`].
    pub fn relation(&self, user: Entity, other: Entity) -> Option<Relation> {
        if user == other {
            return Some(Relation::Allied);
        }
        let rules = self.rules.as_ref()?;
        let a = self.factions.get(user).ok()?;
        let b = self.factions.get(other).ok()?;
        Some(rules.factions.relation(a.0, b.0))
    }

    /// Where `aimed` lands, who it hits, and why the aim would be refused.
    ///
    /// The one answer to the question. The resolver lands a use with it
    /// and the targeting cursor previews one with it, so the two cannot
    /// disagree. Who counts as hit is [`Aim::hits`] over every living
    /// actor under the footprint, the user included; what refuses an aim
    /// is [`aim_blocked`].
    pub fn land(&self, aimed: Aimed<'_>, map: &WorldMap, occupancy: &Occupancy) -> Landed {
        let Aimed { user, ability, def, origin, aim, sees_aim } = aimed;
        let stops = |p: Point| p != origin && (map.blocks_projectiles(p) || occupancy.is_occupied(p));
        let Footprint { cells, path, landing } = footprint(def.mode, origin, aim, map.window_tiles(), stops);
        let refused = aim_blocked(def, &cells, sees_aim);
        let mut targets = Vec::new();
        if def.aim == Aim::SelfOnly {
            // Whatever the shape, the only one it wants is the one using it.
            targets.push(user);
        } else {
            for cell in &cells {
                for who in occupancy.at(*cell) {
                    if !targets.contains(who) && self.alive.contains(*who) && def.aim.hits(self.relation(user, *who), *who == user) {
                        targets.push(*who);
                    }
                }
            }
        }
        Landed { landing: Landing { user, ability, origin, aim, cells, path, landed_at: landing, targets }, refused }
    }
}

/// The abilities a game registered, and the clock their cooldowns run on.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Catalog<'w> {
    abilities: Res<'w, Abilities>,
    turns: Res<'w, Turns>,
}

/// Resolves a use: gate, pay, aim, land.
///
/// A use refused costs the player nothing and keeps its turn, and costs
/// anyone else the turn, which is what stops a monster retrying an
/// ability it cannot afford forever. A use that lands on nobody still
/// costs the turn, the way a shot at nothing does.
pub fn resolve_abilities(
    mut intents: MessageReader<Intent<Use>>,
    mut events: MessageWriter<AbilityEvent>,
    mut resolution: Resolution,
    catalog: Catalog,
    mut state: UserState,
    bystanders: Bystanders,
    mut world: EffectWorld,
) {
    let Catalog { abilities, turns } = catalog;
    let now = turns.now();
    for intent in intents.read() {
        let user = intent.actor;
        let id = intent.action.ability;
        let Ok((known, _, _, _)) = state.users.get(user) else { continue };
        if !known.has(id) {
            continue;
        }
        let source = known.source_of(id);
        if !resolution.claim(user) {
            continue;
        }
        let def = abilities.get(id);
        let Some(origin) = world.position(user) else {
            resolution.failed(user, def.time);
            continue;
        };
        let aim = if def.aim.needs_cursor() { intent.action.aim } else { origin };
        let aimed = Aimed { user, ability: id, def, origin, aim, sees_aim: world.sight_of(user, aim) };
        let Landed { landing, refused } = bystanders.land(aimed, &world.map, &world.occupancy);

        let mut why = gate(user, id, def, source, &state, now);
        why.extend(refused);
        if !why.is_empty() {
            events.write(AbilityEvent::Refused { user, ability: id, why });
            resolution.failed(user, def.time);
            continue;
        }

        pay(user, def, source, &mut state);
        if def.cooldown > 0
            && let Ok((_, _, Some(mut cooldowns), _)) = state.users.get_mut(user)
        {
            cooldowns.set(id, now + def.cooldown);
        }

        let targets = landing.targets.clone();
        for built in &abilities.built[id.index()] {
            if built.chance < 100 && !world.rng.random_ratio(u32::from(built.chance), 100) {
                continue;
            }
            built.effect.apply(&landing, &mut world);
        }
        events.write(AbilityEvent::Used { user, ability: id, aim, targets });
        resolution.done(user, def.time);
    }
}

/// Every reason `def` may not be used by `user` right now.
fn gate(user: Entity, id: AbilityId, def: &AbilityDef, source: Option<Entity>, state: &UserState<'_, '_>, now: u32) -> Vec<Blocked> {
    // The caller has already found this actor holding a turn.
    let Ok((_, pools, cooldowns, health)) = state.users.get(user) else { return Vec::new() };
    let (inventory, equipped, afflicted, block) = state.gear.get(user).unwrap_or((None, None, None, None));

    let no_statuses = Statuses::default();
    let statuses = afflicted.map(|a| &a.0).unwrap_or(&no_statuses);
    let worn: Vec<(rl_rules::SlotId, TagId)> = equipped
        .map(|e| {
            e.0.worn()
                .flat_map(|(slot, item)| {
                    let tags = state.tagged.get(item).ok().and_then(|(t, _)| t).map(|t| t.0.as_slice()).unwrap_or(&[]);
                    tags.iter().map(move |tag| (slot, *tag))
                })
                .collect()
        })
        .unwrap_or_default();
    let no_stats = rl_rules::Stats::default();
    let block = block.map(|s| &s.0).unwrap_or(&no_stats);
    let stat = |s: StatId| block.value(s, &state.stats.0);
    let gates = Gates { statuses, worn: &worn, stat: &stat };

    let no_pools = Pools::default();
    let pools = pools.unwrap_or(&no_pools);
    let pool = |s: StatId| pools.get(s);
    let items = |tag: TagId| count_tagged(inventory, tag, state);
    let purse = Purse {
        pool: &pool,
        charges: source.and_then(|e| state.charges.get(e).ok()).map(|c| c.left),
        // No health component means nothing to spend it from, and a cost
        // in health should refuse rather than silently succeed.
        health: health.map(|h| h.hp).unwrap_or(0),
        items: &items,
    };
    blocked(def, &gates, &purse, now, cooldowns.map(|c| c.ready_at(id)).unwrap_or(0))
}

/// How many items carrying `tag` are in `inventory`, stacks counted.
fn count_tagged(inventory: Option<&Inventory>, tag: TagId, state: &UserState<'_, '_>) -> u16 {
    let Some(inventory) = inventory else { return 0 };
    let mut total: u32 = 0;
    for item in &inventory.items {
        let Ok((tags, stack)) = state.tagged.get(*item) else { continue };
        if !tags.is_some_and(|t| t.0.contains(&tag)) {
            continue;
        }
        total += stack.map(|s| s.count).unwrap_or(1);
    }
    total.min(u32::from(u16::MAX)) as u16
}

/// Spends everything `def` costs.
///
/// Called only once the gate has passed, so every cost here is affordable
/// and the all-or-nothing rule holds without a second check.
fn pay(user: Entity, def: &AbilityDef, source: Option<Entity>, state: &mut UserState<'_, '_>) {
    let carried: Vec<Entity> = state.gear.get(user).ok().and_then(|(i, _, _, _)| i).map(|i| i.items.clone()).unwrap_or_default();
    for cost in &def.costs {
        match *cost {
            Cost::Pool { stat, amount } => {
                if let Ok((_, Some(mut pools), _, _)) = state.users.get_mut(user) {
                    pools.spend(stat, amount);
                }
            }
            Cost::Health { amount } => {
                if let Ok((_, _, _, Some(mut health))) = state.users.get_mut(user) {
                    health.hp -= amount;
                }
            }
            Cost::Charge { amount } => {
                if let Some(mut charges) = source.and_then(|e| state.charges.get_mut(e).ok()) {
                    charges.left = charges.left.saturating_sub(amount);
                }
            }
            Cost::Item { tag, count } => spend_tagged(&carried, tag, count, state),
        }
    }
}

/// Takes `count` items carrying `tag` out of `carried`, drawing stacks
/// down before whole items.
///
/// An item spent to nothing is despawned, and the bag forgets it the way
/// it forgets any despawned item.
fn spend_tagged(carried: &[Entity], tag: TagId, count: u16, state: &mut UserState<'_, '_>) {
    let mut left = u32::from(count);
    let mut spent: Vec<Entity> = Vec::new();
    for item in carried {
        if left == 0 {
            break;
        }
        let Ok((tags, stack)) = state.tagged.get_mut(*item) else { continue };
        if !tags.is_some_and(|t| t.0.contains(&tag)) {
            continue;
        }
        match stack {
            Some(mut s) => {
                let taken = left.min(s.count);
                s.count -= taken;
                left -= taken;
                if s.count == 0 {
                    spent.push(*item);
                }
            }
            None => {
                left -= 1;
                spent.push(*item);
            }
        }
    }
    for item in spent {
        state.commands.entity(item).despawn();
    }
}

/// What the actor holding the turn knows, and which of it can be used.
///
/// A resource rather than a component so the gate runs once, for the one
/// actor whose turn it is, and so two very different readers can share it:
/// the minds in [`combat`](crate::combat), which take `usable` and never
/// learn how an ability is paid for, and a menu, which wants `refused` as
/// well so it can grey a row and say why.
///
/// Read through [`usable_by`](Self::usable_by) and [`why_for`](Self::why_for),
/// which answer only for the actor it was worked out for: it is rebuilt
/// every pass, so a reader holding last pass's actor would otherwise act
/// on someone else's abilities.
#[derive(Resource, Debug, Default)]
pub struct Offered {
    /// Who it was worked out for. The player as readily as a monster:
    /// what may be used is the same question for both.
    actor: Option<Entity>,
    /// What can be used now.
    usable: Vec<Usable>,
    /// What is known and cannot be used, with every reason.
    refused: Vec<(AbilityId, Vec<Blocked>)>,
}

impl Offered {
    /// Who holds the turn it was worked out for, if anyone does.
    pub fn actor(&self) -> Option<Entity> {
        self.actor
    }

    /// What `actor` can use now, empty when it was worked out for someone
    /// else.
    pub fn usable_by(&self, actor: Entity) -> &[Usable] {
        if self.actor == Some(actor) { &self.usable } else { &[] }
    }

    /// Why `actor` cannot use `ability`, empty when it can, when it does
    /// not know it, or when this was worked out for someone else.
    pub fn why_for(&self, actor: Entity, ability: AbilityId) -> &[Blocked] {
        if self.actor != Some(actor) {
            return &[];
        }
        self.refused.iter().find(|(id, _)| *id == ability).map(|(_, why)| why.as_slice()).unwrap_or(&[])
    }
}

/// Sorts the turn-holder's abilities into what it can use and what it
/// cannot, before anything decides.
///
/// The gate is the same one the resolver uses, so a tactic is never
/// offered an ability that would be refused, which is what keeps a
/// monster from spending its turn on something it cannot pay for, and a
/// menu never greys a row the resolver would have accepted.
pub fn offer_abilities(mut offered: ResMut<Offered>, abilities: Res<Abilities>, turns: Res<Turns>, state: UserState, holding: Query<Entity, With<MyTurn>>) {
    offered.actor = None;
    offered.usable.clear();
    offered.refused.clear();
    let Ok(actor) = holding.single() else { return };
    let Ok((known, _, _, _)) = state.users.get(actor) else { return };
    offered.actor = Some(actor);
    let now = turns.now();
    for (id, source) in known.iter() {
        let def = abilities.get(id);
        let why = gate(actor, id, def, source, &state, now);
        if why.is_empty() {
            offered.usable.push(Usable { ability: id, aim: def.aim, mode: def.mode });
        } else {
            offered.refused.push((id, why));
        }
    }
}

/// Rebuilds every actor's [`Known`] from what it is and what it wears.
///
/// Rebuilt rather than edited, the way gear modifiers are: an unequipped
/// wand takes its ability with it and nothing has to remember that it did.
/// An actor's own [`Grants`] come first, so a wand lending an ability the
/// actor already knows does not make it depend on the wand.
pub fn refresh_known(mut actors: Query<(&mut Known, Option<&Grants>, Option<&Equipped>)>, lent: Query<&Grants, Without<Known>>) {
    for (mut known, innate, equipped) in &mut actors {
        known.clear();
        for ability in innate.map(|g| g.0.as_slice()).unwrap_or(&[]) {
            known.learn(*ability, None);
        }
        let Some(equipped) = equipped else { continue };
        for (_, item) in equipped.0.worn() {
            let Ok(grants) = lent.get(item) else { continue };
            for ability in &grants.0 {
                known.learn(*ability, Some(item));
            }
        }
    }
}

/// Abilities: the use action, the state a use spends, and the seam every
/// effect is registered through.
///
/// Needs [`Abilities`], [`AbilityRng`] and [`StatRules`] before play
/// begins, and combat, since an ability's damage goes down the same
/// pipeline a sword's does. Register every effect an ability file names
/// with [`AddEffect::add_effect`] while the app is built, then build
/// [`Abilities`] from the loaded definitions.
///
/// The engine's own effects are not registered here: each belongs to the
/// module that owns its mechanic, and a game adds the ones its content
/// names with [`AddEngineEffects`] or one at a time.
///
/// Every [`Actor`](crate::components::Actor) is given an empty [`Known`],
/// [`Pools`] and [`Cooldowns`] the moment it is spawned, so an actor given
/// [`Grants`] alone can use what it was granted. On the actor rather than
/// on `Grants`, because an item that lends an ability is no actor and must
/// not come to know it.
pub struct AbilitiesPlugin;

impl Plugin for AbilitiesPlugin {
    fn build(&self, app: &mut App) {
        use crate::components::Actor;
        use crate::plugin::Needs;
        app.register_required_components::<Actor, Known>();
        app.register_required_components::<Actor, Pools>();
        app.register_required_components::<Actor, Cooldowns>();
        app.init_resource::<EffectKinds>()
            .init_resource::<Offered>()
            .add_message::<AbilityEvent>()
            .add_action::<Use>()
            .needs::<Abilities>("AbilitiesPlugin", "`Abilities::build(defs, &EffectKinds, &lookup)` over the defs `rl_rules::ability::load` returns")
            .needs::<AbilityRng>("AbilitiesPlugin", "`AbilityRng::for_run(seed)`, the stream abilities roll from")
            .needs::<StatRules>("AbilitiesPlugin", "`StatRules(registry)`, the stats ability costs and requirements name")
            .add_systems(Turn, offer_abilities.in_set(crate::plugin::DecideSet::Offer))
            .add_systems(Turn, resolve_abilities.in_set(ResolveSet::Act))
            .add_systems(Turn, refresh_known.in_set(TurnSet::React));
    }

    // In `finish` like every other plugin's, so the order a game lists its
    // plugins in never matters.
    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::combat::CombatPlugin>(app, "AbilitiesPlugin");
    }
}

/// Push everyone under the footprint away from the user.
///
/// Here rather than in the turn loop because the move goes through
/// [`EffectWorld::slide`], which is what keeps a shove out of a wall and
/// the occupancy index straight.
#[derive(Debug, Clone, Copy)]
pub struct Shove {
    /// How many cells.
    pub cells: i32,
}

impl Effect for Shove {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in landing.targets.clone() {
            let Some(at) = world.position(target) else { continue };
            let away = Point::new(at.x + (at.x - landing.origin.x).signum(), at.y + (at.y - landing.origin.y).signum());
            world.slide(target, at, away, self.cells);
        }
    }
}

impl FromArgs for Shove {
    const KIND: &'static str = "Shove";

    fn from_args(args: &RawValue, _look: &dyn Lookup) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            cells: i32,
        }
        let a: Args = rl_rules::ability::read_args(args)?;
        Ok(Self { cells: a.cells })
    }
}

/// Drag everyone under the footprint towards the user.
#[derive(Debug, Clone, Copy)]
pub struct Pull {
    /// How many cells.
    pub cells: i32,
}

impl Effect for Pull {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in landing.targets.clone() {
            let Some(at) = world.position(target) else { continue };
            world.slide(target, at, landing.origin, self.cells);
        }
    }
}

impl FromArgs for Pull {
    const KIND: &'static str = "Pull";

    fn from_args(args: &RawValue, _look: &dyn Lookup) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            cells: i32,
        }
        let a: Args = rl_rules::ability::read_args(args)?;
        Ok(Self { cells: a.cells })
    }
}

/// Move the user to where the ability landed.
///
/// Refused rather than approximated when the cell will not take it: a
/// blink that lands you inside a wall is worse than a blink that fizzles,
/// and the turn is spent either way.
#[derive(Debug, Clone, Copy, Default)]
pub struct Teleport;

impl Effect for Teleport {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let Some(to) = landing.landed_at.or(Some(landing.aim)) else { return };
        world.place(landing.user, to);
    }
}

impl FromArgs for Teleport {
    const KIND: &'static str = "Teleport";

    fn from_args(_args: &RawValue, _look: &dyn Lookup) -> Result<Self, String> {
        Ok(Self)
    }
}

/// The effects the engine ships, registered together.
///
/// A convenience, not a requirement: a game that wants three of them
/// registers three, and one that wants none registers none. Nothing is
/// registered by default, because an ability file naming an effect the
/// game did not ask for should fail at load rather than work by accident.
pub trait AddEngineEffects {
    /// Registers `Harm`, `Mend`, `Inflict`, `Cleanse`, `Shove`, `Pull` and
    /// `Teleport`.
    fn add_engine_effects(&mut self) -> &mut Self;
}

impl AddEngineEffects for App {
    fn add_engine_effects(&mut self) -> &mut Self {
        self.add_effect::<crate::combat::Harm>()
            .add_effect::<crate::combat::Mend>()
            .add_effect::<crate::status::Inflict>()
            .add_effect::<crate::status::Cleanse>()
            .add_effect::<Shove>()
            .add_effect::<Pull>()
            .add_effect::<Teleport>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatRng, CombatRules, DamageDealt, Faction};
    use crate::components::{Actor, Player, RevealsMap};
    use crate::status::StatusRules;
    use crate::world::{ChunkRulesRes, WorldRes};
    use rl_core::{Direction, RunSeed};
    use rl_grid::{TileId, TileRegistry};
    use rl_mapgen::Chain;
    use rl_mapgen::passes::Fill;
    use rl_rules::content::Registry;
    use rl_rules::faction::FactionDef;
    use rl_rules::{DamageKind, Factions, Relation, SlotDef, StatDef, StatusDef, TagDef};
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
            let tile = if around.here.band.0 == 0 { self.tiles.expect("wall") } else { self.tiles.expect("floor") };
            Chain::new().then(Fill { tile })
        }
    }

    /// Every registry an ability names, and the lookup over them.
    struct Content {
        stats: Registry<StatDef>,
        statuses: Registry<StatusDef>,
        tags: Registry<TagDef>,
        slots: Registry<SlotDef>,
        kinds: Registry<DamageKind>,
        factions: Registry<FactionDef>,
    }

    impl Content {
        fn new() -> Self {
            Self {
                stats: Registry::from_defs(vec![StatDef::new("mana", 20), StatDef::new("nerve", 5)]).unwrap(),
                statuses: Registry::from_defs(vec![StatusDef::new("scorched"), StatusDef::new("braced")]).unwrap(),
                tags: Registry::from_defs(vec![TagDef::new("powder"), TagDef::new("wand")]).unwrap(),
                slots: Registry::from_defs(vec![SlotDef::new("hand")]).unwrap(),
                kinds: Registry::from_defs(vec![DamageKind::new("fire")]).unwrap(),
                factions: Registry::from_defs(vec![FactionDef { name: "us".into() }, FactionDef { name: "them".into() }]).unwrap(),
            }
        }
    }

    impl rl_rules::ability::Lookup for Content {
        fn stat(&self, n: &str) -> Option<StatId> {
            self.stats.id(n)
        }
        fn status(&self, n: &str) -> Option<rl_rules::StatusId> {
            self.statuses.id(n)
        }
        fn tag(&self, n: &str) -> Option<TagId> {
            self.tags.id(n)
        }
        fn slot(&self, n: &str) -> Option<rl_rules::SlotId> {
            self.slots.id(n)
        }
        fn damage(&self, n: &str) -> Option<rl_rules::damage::DamageKindId> {
            self.kinds.id(n)
        }
    }

    const ABILITIES: &str = r#"#![enable(implicit_some)]
[
    (
        name: "bolt",
        mode: Bolt(range: 6),
        costs: [Pool("mana", 5)],
        effects: [(kind: "Harm", args: (kind: "fire", roll: "4"))],
    ),
    (
        name: "burst",
        aim: Ground,
        mode: Ball(range: 6, radius: 1),
        effects: [(kind: "Harm", args: (kind: "fire", roll: "3"))],
    ),
    (
        name: "brace",
        aim: SelfOnly,
        mode: Own,
        cooldown: 500,
        effects: [(kind: "Inflict", args: (status: "braced", turns: 4))],
    ),
    (
        name: "heave",
        mode: Adjacent,
        effects: [(kind: "Shove", args: (cells: 2))],
    ),
    (
        name: "volley",
        mode: Bolt(range: 6),
        costs: [Item("powder", 1)],
        effects: [(kind: "Harm", args: (kind: "fire", roll: "2"))],
    ),
    (
        name: "mark",
        aim: Ground,
        mode: Own,
        effects: [(kind: "Mark", args: (note: 7))],
    ),
    (
        name: "mend",
        aim: Ally,
        mode: Own,
        effects: [(kind: "Mend", args: (kind: "fire", roll: "5"))],
    ),
]"#;

    /// A game's own effect, doing something the engine has no word for.
    /// The whole point of the escape hatch: it reaches the world through
    /// `commands` and the resolver cannot tell it from an engine effect.
    #[derive(Component, Debug, Clone, Copy)]
    struct Marked(u32);

    #[derive(Debug, Clone, Copy)]
    struct Mark {
        note: u32,
    }

    impl Effect for Mark {
        fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
            world.commands.entity(landing.user).insert(Marked(self.note));
        }
    }

    impl FromArgs for Mark {
        const KIND: &'static str = "Mark";

        fn from_args(args: &RawValue, _look: &dyn Lookup) -> Result<Self, String> {
            #[derive(serde::Deserialize)]
            struct Args {
                note: u32,
            }
            let a: Args = rl_rules::ability::read_args(args)?;
            Ok(Self { note: a.note })
        }
    }

    fn app() -> (App, Point) {
        let mut app = crate::plugin::headless_app();
        app.add_plugins((
            crate::fov::FovPlugin,
            crate::world::StreamingPlugin,
            crate::combat::CombatPlugin,
            crate::status::StatusPlugin,
            crate::items::ItemsPlugin,
            AbilitiesPlugin,
        ));
        app.add_engine_effects().add_effect::<Mark>();

        let content = Content::new();
        let defs = rl_rules::ability::load(ABILITIES, &content).expect("the abilities load");
        let kinds = app.world().resource::<EffectKinds>();
        let abilities = Abilities::build(defs, kinds, &content).expect("the effects build");

        let mut factions = Factions::new(&content.factions);
        factions.set_mutual(content.factions.expect("us"), content.factions.expect("them"), Relation::Hostile);

        let tiles = TileRegistry::standard();
        let config = WorldConfig { region_size: 16, ..WorldConfig::regions(12, 10) };
        let world = WorldGraph::generate(RunSeed(5), config, &Flat);
        let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("land");
        let start = world.tile_origin(region).offset(8, 8);

        app.insert_resource(WorldMap::new(tiles.tables()));
        app.insert_resource(WorldRes(world));
        app.insert_resource(ChunkRulesRes(Box::new(Open { tiles })));
        app.insert_resource(CombatRules { kinds: content.kinds, factions });
        app.insert_resource(CombatRng::for_run(RunSeed(5)));
        app.insert_resource(AbilityRng::for_run(RunSeed(5)));
        app.insert_resource(StatusRules { defs: content.statuses });
        app.insert_resource(StatRules(content.stats));
        app.insert_resource(crate::items::Slots(content.slots));
        app.insert_resource(abilities);
        (app, start)
    }

    fn ability(app: &App, name: &str) -> AbilityId {
        app.world().resource::<Abilities>().expect(name)
    }

    fn caster(app: &mut App, at: Point, mana: i32, abilities: &[AbilityId]) -> Entity {
        let us = Faction(rl_rules::FactionId::from_raw(0));
        let mana_id = StatId::from_raw(0);
        let mut pools = Pools::new();
        pools.set(mana_id, mana);
        // Innate abilities go in `Grants`; `Known` is derived from it
        // every turn, so anything written straight into `Known` is gone by
        // the next pass.
        let grants = Grants(abilities.to_vec());
        let e = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(at), Viewshed::new(8), RevealsMap, Health::full(30), us, (grants, pools), Inventory::default()))
            .id();
        app.world_mut().resource_mut::<NextState<crate::state::EngineState>>().set(crate::state::EngineState::Playing);
        e
    }

    fn foe(app: &mut App, at: Point) -> Entity {
        let them = Faction(rl_rules::FactionId::from_raw(1));
        app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(20), them)).id()
    }

    fn friend(app: &mut App, at: Point) -> Entity {
        let us = Faction(rl_rules::FactionId::from_raw(0));
        app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(20), us)).id()
    }

    fn settle(app: &mut App) {
        app.update();
        app.update();
    }

    fn hp(app: &App, e: Entity) -> i32 {
        app.world().get::<Health>(e).expect("health").hp
    }

    /// The whole loop in one test: a bolt is paid for out of a pool, flies
    /// to what the aim wanted, and lands damage through the pipeline that
    /// mitigates it.
    #[test]
    fn a_bolt_spends_its_pool_and_harms_what_the_aim_wanted() {
        let (mut app, start) = app();
        let bolt = ability(&app, "bolt");
        let me = caster(&mut app, start, 20, &[bolt]);
        let them = foe(&mut app, start.offset(3, 0));
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: start.offset(3, 0) }));
        app.update();

        assert_eq!(hp(&app, them), 16, "four fire off twenty");
        assert_eq!(app.world().get::<Pools>(me).unwrap().get(StatId::from_raw(0)), 15, "five mana spent");
        assert!(app.world().get::<MyTurn>(me).is_some(), "the turn came back round");
        assert_eq!(app.world().resource::<Turns>().now(), 100, "and it cost one step");
    }

    /// A blast aimed at the ground hits whoever is standing in it; a bolt
    /// aimed at a foe never picks an ally out of the same cells.
    #[test]
    fn the_aim_decides_who_a_footprint_counts_as_a_target() {
        let (mut app, start) = app();
        let (bolt, burst) = (ability(&app, "bolt"), ability(&app, "burst"));
        let me = caster(&mut app, start, 20, &[bolt, burst]);
        let them = foe(&mut app, start.offset(4, 0));
        let ours = friend(&mut app, start.offset(4, 1));
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: burst, aim: start.offset(4, 0) }));
        app.update();
        assert_eq!(hp(&app, them), 17, "the ground does not take sides");
        assert_eq!(hp(&app, ours), 17, "and neither did we");

        app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: start.offset(4, 1) }));
        app.update();
        assert_eq!(hp(&app, ours), 17, "a bolt aimed at a foe passes an ally by");
    }

    /// An unaffordable ability is refused, costs the player no time, keeps
    /// its turn, and says every reason at once.
    #[test]
    fn an_ability_that_cannot_be_paid_for_costs_the_player_nothing() {
        let (mut app, start) = app();
        let bolt = ability(&app, "bolt");
        let me = caster(&mut app, start, 2, &[bolt]);
        let them = foe(&mut app, start.offset(3, 0));
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: start.offset(3, 0) }));
        app.update();

        assert_eq!(hp(&app, them), 20, "nothing landed");
        assert_eq!(app.world().get::<Pools>(me).unwrap().get(StatId::from_raw(0)), 2, "and nothing was spent");
        assert_eq!(app.world().resource::<Turns>().now(), 0, "no time passed");
        assert!(app.world().get::<MyTurn>(me).is_some(), "and the turn is still the player's");
        let refusals: Vec<Vec<Blocked>> = app
            .world_mut()
            .resource_mut::<Messages<AbilityEvent>>()
            .drain()
            .filter_map(|e| match e {
                AbilityEvent::Refused { why, .. } => Some(why),
                _ => None,
            })
            .collect();
        assert_eq!(refusals, vec![vec![Blocked::Cannot(Cost::Pool { stat: StatId::from_raw(0), amount: 5 })]]);
    }

    /// A cooldown is an absolute time on the turn clock, so it refuses
    /// while it is live and lets go on its own once the clock passes it.
    #[test]
    fn a_cooldown_refuses_a_second_use_until_the_clock_reaches_it() {
        let (mut app, start) = app();
        let brace = ability(&app, "brace");
        let me = caster(&mut app, start, 20, &[brace]);
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: brace, aim: start }));
        app.update();
        assert_eq!(app.world().get::<Cooldowns>(me).unwrap().ready_at(brace), 500, "five hundred from the moment of use, not from the end of it");
        assert!(app.world().get::<Afflicted>(me).unwrap().0.has(rl_rules::StatusId::from_raw(1)), "braced");

        app.world_mut().write_message(Intent::new(me, Use { ability: brace, aim: start }));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), 100, "the second use was refused and cost nothing");

        // Walk the clock past it. Each step is a hundredth of a step ×100.
        for _ in 0..5 {
            let dir = if app.world().resource::<Turns>().now() % 200 == 100 { Direction::East } else { Direction::West };
            app.world_mut().write_message(Intent::new(me, crate::turn::Step(dir)));
            app.update();
        }
        assert!(app.world().resource::<Turns>().now() >= 600);
        app.world_mut().write_message(Intent::new(me, Use { ability: brace, aim: start }));
        app.update();
        assert_eq!(app.world().get::<Cooldowns>(me).unwrap().ready_at(brace), 1100, "ready again, and set again");
    }

    /// A shove moves what it hits away from the user and stops at a wall
    /// rather than through it.
    #[test]
    fn a_shove_pushes_away_from_the_user_and_stops_at_what_it_cannot_pass() {
        let (mut app, start) = app();
        let heave = ability(&app, "heave");
        let me = caster(&mut app, start, 20, &[heave]);
        let them = foe(&mut app, start.offset(1, 0));
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: heave, aim: start.offset(1, 0) }));
        app.update();
        assert_eq!(app.world().get::<Position>(them).unwrap().0, start.offset(3, 0), "two cells further out");

        // A wall right behind it: the shove stops rather than passing.
        app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(4, 0), TileId(1));
        app.world_mut().write_message(Intent::new(me, Use { ability: heave, aim: start.offset(3, 0) }));
        app.update();
        assert_eq!(app.world().get::<Position>(them).unwrap().0, start.offset(3, 0), "the wall took it");
    }

    /// An effect the engine has never heard of reaches the world through
    /// `commands`, and the resolver cannot tell it from one of its own.
    #[test]
    fn an_effect_a_game_wrote_lands_beside_the_engines_own() {
        let (mut app, start) = app();
        let mark = ability(&app, "mark");
        let me = caster(&mut app, start, 20, &[mark]);
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: mark, aim: start }));
        app.update();
        assert_eq!(app.world().get::<Marked>(me).map(|m| m.0), Some(7));
    }

    /// An ability names an effect nobody registered: a failure at load,
    /// naming the ability and listing what is registered, rather than a
    /// surprise the first time a player presses the key.
    #[test]
    fn an_unregistered_effect_is_a_load_failure_that_names_the_ability() {
        let content = Content::new();
        let defs = rl_rules::ability::load(r#"[(name: "hex", mode: Own, effects: [(kind: "Curse", args: ())])]"#, &content).unwrap();
        let mut kinds = EffectKinds::default();
        kinds.declare::<crate::combat::Harm>();
        let Err(err) = Abilities::build(defs, &kinds, &content) else { panic!("it should not build") };
        let rl_rules::ContentError::Invalid(errs) = err else { panic!("expected a validation failure") };
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("hex") && errs[0].contains("Curse") && errs[0].contains("Harm"), "{errs:?}");
    }

    /// Damage from an ability goes down the same pipeline a sword's does,
    /// so a game that inserts a mitigation stage gets it on both.
    #[test]
    fn ability_damage_is_reported_like_any_other() {
        let (mut app, start) = app();
        let bolt = ability(&app, "bolt");
        let me = caster(&mut app, start, 20, &[bolt]);
        let them = foe(&mut app, start.offset(2, 0));
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: start.offset(2, 0) }));
        app.update();
        let dealt: Vec<DamageDealt> = app.world_mut().resource_mut::<Messages<DamageDealt>>().drain().collect();
        assert_eq!(dealt.len(), 1);
        assert_eq!(dealt[0].target, them);
        assert_eq!(dealt[0].dealt, 4);
        assert_eq!(dealt[0].hit.attacker, Some(me), "credited to the caster");
    }

    /// The resolver lands a use by the rule the preview and the minds read:
    /// a burst on the ground catches the thrower standing in it, and a bolt
    /// pointed at the caster's own feet has nowhere to fly, so it is refused
    /// and costs the player nothing.
    #[test]
    fn a_burst_catches_its_thrower_and_a_bolt_with_nowhere_to_fly_is_refused() {
        let (mut app, start) = app();
        let (bolt, burst) = (ability(&app, "bolt"), ability(&app, "burst"));
        let me = caster(&mut app, start, 20, &[bolt, burst]);
        let them = foe(&mut app, start.offset(1, 0));
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: burst, aim: start.offset(1, 0) }));
        app.update();
        assert_eq!(hp(&app, them), 17, "the foe it was thrown at");
        assert_eq!(hp(&app, me), 27, "and the thrower, one cell off the middle of it");

        app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: start }));
        app.update();
        assert_eq!(app.world().get::<Pools>(me).unwrap().get(StatId::from_raw(0)), 20, "nothing spent");
        assert_eq!(app.world().resource::<Turns>().now(), 100, "and no time passed for the bolt");
    }

    /// A mend is a negative hit down the same pipeline a blow takes, so it
    /// heals the user it was aimed at and stops at full health.
    #[test]
    fn a_mend_heals_the_user_and_stops_at_full() {
        let (mut app, start) = app();
        let mend = ability(&app, "mend");
        let me = caster(&mut app, start, 20, &[mend]);
        settle(&mut app);
        app.world_mut().get_mut::<Health>(me).expect("health").hp = 22;

        app.world_mut().write_message(Intent::new(me, Use { ability: mend, aim: start }));
        app.update();
        assert_eq!(hp(&app, me), 27, "five mended");

        app.world_mut().write_message(Intent::new(me, Use { ability: mend, aim: start }));
        app.update();
        assert_eq!(hp(&app, me), 30, "and no further than full");
    }

    /// The other half of owning the loop: a monster fires an ability the
    /// same way the player does, chosen by a tactic that never learns what
    /// the ability is.
    #[test]
    fn a_monster_casts_what_a_tactic_picked_for_it() {
        use rl_rules::ai::tactics::{Hunt, UseAbility};
        use rl_rules::{Brain, MovementProfile};
        use std::sync::Arc;

        let (mut app, start) = app();
        let bolt = ability(&app, "bolt");
        let me = caster(&mut app, start, 20, &[]);
        let brain = Brain::new().then(UseAbility::default()).then(Hunt);
        let them = foe(&mut app, start.offset(4, 0));
        let mut pools = Pools::new();
        pools.set(StatId::from_raw(0), 20);
        app.world_mut().entity_mut(them).insert((
            crate::combat::Mind(Arc::new(brain)),
            crate::combat::Perception(10),
            crate::combat::Profile(MovementProfile::default()),
            Grants(vec![bolt]),
            pools,
            Inventory::default(),
        ));
        settle(&mut app);

        let before = hp(&app, me);
        // Spend the player's turns waiting; the monster is due between them.
        for _ in 0..3 {
            app.world_mut().write_message(Intent::new(me, crate::turn::Wait));
            app.update();
        }
        let used: Vec<Entity> = app
            .world_mut()
            .resource_mut::<Messages<AbilityEvent>>()
            .drain()
            .filter_map(|e| match e {
                AbilityEvent::Used { user, .. } => Some(user),
                _ => None,
            })
            .collect();
        assert!(used.contains(&them), "the monster reached for its bolt: {used:?}");
        assert!(hp(&app, me) < before, "and it landed: {} then {}", before, hp(&app, me));
        assert!(app.world().get::<Pools>(them).unwrap().get(StatId::from_raw(0)) < 20, "it paid for it");
    }

    /// A monster is never offered an ability it could not pay for, which
    /// is what keeps it from spending its turn being refused.
    #[test]
    fn a_mind_is_never_offered_what_it_cannot_afford() {
        use rl_rules::ai::tactics::UseAbility;
        use rl_rules::{Brain, MovementProfile};
        use std::sync::Arc;

        let (mut app, start) = app();
        let bolt = ability(&app, "bolt");
        let me = caster(&mut app, start, 20, &[]);
        let them = foe(&mut app, start.offset(3, 0));
        app.world_mut().entity_mut(them).insert((
            crate::combat::Mind(Arc::new(Brain::new().then(UseAbility::default()))),
            crate::combat::Perception(10),
            crate::combat::Profile(MovementProfile::default()),
            // Granted alone: the plugin gives every actor empty pools, and
            // an empty pool is what this test is about.
            Grants(vec![bolt]),
            Inventory::default(),
        ));
        settle(&mut app);
        app.world_mut().write_message(Intent::new(me, crate::turn::Wait));
        app.update();

        assert!(app.world().resource::<Offered>().usable.is_empty(), "an empty pool is not an offer");
        let refusals = app.world_mut().resource_mut::<Messages<AbilityEvent>>().drain().filter(|e| matches!(e, AbilityEvent::Refused { .. })).count();
        assert_eq!(refusals, 0, "it was never offered, so it never asked");
    }
}
