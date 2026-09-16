//! Abilities: the loop that turns a definition into something that
//! happened.
//!
//! [`rl_rules::ability`] holds what an ability *is* and answers whether it
//! may be used. This module owns the rest: the [`Use`] action, the
//! resolver that gates it, pays for it, resolves its footprint and lands
//! its effects, and the state a use spends and sets.
//!
//! Effects are types, not a list, for the reason actions are: the engine
//! ships one per subsystem it owns, in [`effects`](crate::effects), and a
//! game registers its own with
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
use rl_rules::ability::{AbilityDef, AbilityId, Aim, Blocked, Cost, Gates, Purse, RawValue, Usable, aim_blocked, blocked};
use rl_rules::{Names, Registry, Relation, StatId, Statuses, TagId};

use crate::combat::{CombatRules, Dead, Faction, Health};
use crate::components::{Blocks, MyTurn, Position, Viewshed};
use crate::cue::{Anchor, Cue, Cued, LookOf, TurnHold};
use crate::items::{Equipped, Inventory, Item, Stack, Tagged, UseItem};
use crate::plugin::{ResolveSet, Turn, TurnSet};
use crate::registries::Registries;
use crate::status::{Afflict, Afflicted, Cure, StatBlock};
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

/// The abilities an actor knows of itself, or an item lends whoever holds
/// it.
///
/// On an item it lends while the item is carried, worn or not: a wand in
/// the bag is a wand. An ability that should work only while its item is
/// worn says so with [`Requirement::Wielding`](rl_rules::ability::Requirement),
/// which is what the requirement is for.
///
/// This is also how a consumable is written. A potion is an item that
/// grants an ability costing [`Cost::Charge`]: the ability's effects are
/// what drinking does, and the charge is spent from the potion, so a
/// potion is a line of RON and no game writes a use system. Using the item
/// itself, through [`UseItem`], comes to using what it grants.
#[derive(Component, Debug, Clone, Default)]
pub struct Grants(pub Vec<AbilityId>);

/// Uses left in whatever carries this.
///
/// On the item that granted the ability, not on the actor, so two wands
/// are two pools of charges and a used-up one is still a wand. An item
/// with no `Charges` is spent whole by a [`Cost::Charge`]: one off its
/// [`Stack`], or the item itself, despawned. That is what makes a potion a
/// potion and a wand a wand, and nothing else has to say which is which.
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
/// and change every monster's rolls in a run that was going fine. Derived
/// from the run's [`Seed`](crate::seed::Seed) by [`AbilitiesPlugin`].
#[derive(Resource, Deref, DerefMut)]
pub struct AbilityRng(pub StdRng);

impl crate::seed::Stream for AbilityRng {
    fn for_run(seed: RunSeed) -> Self {
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
    /// What is worth seeing, for whatever draws. The flight and the burst
    /// of the use itself are cued before any effect runs, so a cue an
    /// effect adds plays after them.
    pub cues: MessageWriter<'w, Cued>,
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

    /// What this does, in a few words for a menu, with every id named
    /// through `registries`: `3d6 fire`, `scorched for 4 turns`. Empty
    /// means the menu says nothing about it, which is the default so an
    /// effect a game writes in a hurry still loads.
    fn describe(&self, registries: &crate::registries::Registries) -> String {
        let _ = registries;
        String::new()
    }
}

/// An effect that knows how to build itself out of an ability's RON.
///
/// Separate from [`Effect`] so the trait a game writes stays object-safe
/// and the constructor can fail with a message naming what was wrong.
pub trait FromArgs: Effect + Sized {
    /// The name abilities call this effect by in RON.
    const KIND: &'static str;

    /// Builds one from the arguments, resolving any names through `names`.
    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String>;
}

/// How an effect is built, once its name has been matched.
type Builder = fn(&RawValue, &Names<'_>) -> Result<Box<dyn Effect>, String>;

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
        self.0.insert(E::KIND.to_string(), |args, names| E::from_args(args, names).map(|e| Box::new(e) as Box<dyn Effect>));
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
    /// Loads abilities from RON and builds their effects, every name resolved
    /// through `names`: [`rl_rules::ability::load`] then [`build`](Self::build),
    /// for a game whose abilities are one file.
    ///
    /// Fails at whichever step finds something wrong, listing everything it
    /// found: every unknown name, or every effect that would not build.
    pub fn load(text: &str, kinds: &EffectKinds, names: &Names<'_>) -> Result<Self, rl_rules::ContentError> {
        Self::build(rl_rules::ability::load(text, names)?, kinds, names)
    }

    /// Builds the effects of every ability in `defs` through `kinds`.
    ///
    /// Fails naming the ability, the effect and what was wrong with it,
    /// and lists every failure rather than the first.
    pub fn build(defs: Registry<AbilityDef>, kinds: &EffectKinds, names: &Names<'_>) -> Result<Self, rl_rules::ContentError> {
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
                    Some(build) => match build(&spec.args, names) {
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

    /// What `id` does, one line per effect that has something to say,
    /// with its chance in front when it is not certain: what a menu lists
    /// under an ability.
    pub fn describe(&self, id: AbilityId, registries: &crate::registries::Registries) -> Vec<String> {
        self.built[id.index()]
            .iter()
            .filter_map(|b| {
                let what = b.effect.describe(registries);
                if what.is_empty() {
                    None
                } else if b.chance >= 100 {
                    Some(what)
                } else {
                    Some(format!("{}% chance of {what}", b.chance))
                }
            })
            .collect()
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
    items: Query<'w, 's, (), With<Item>>,
    registries: Res<'w, Registries>,
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
        let mut refused = aim_blocked(def, &cells, sees_aim);
        // A projectile that stopped short of where it was pointed, at its
        // range or on whatever stood in the way, is refused rather than
        // landed where it stopped: nobody chose that spot.
        if def.aim.needs_cursor() && landing.is_some_and(|landed| landed != aim) {
            // First among the reasons: an aim that stops short has no
            // target under it either, and out of reach is the one to act on.
            refused.insert(0, Blocked::OutOfReach);
        }
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

/// The abilities a game registered, the clock their cooldowns run on, and
/// the uses in the air.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Catalog<'w> {
    abilities: Res<'w, Abilities>,
    turns: Res<'w, Turns>,
    hold: ResMut<'w, TurnHold>,
    airborne: ResMut<'w, Airborne>,
}

/// Uses that have been cast and are flying, to land on the first pass
/// after their flight has been seen.
#[derive(Resource, Debug, Default)]
pub struct Airborne(Vec<Landing>);

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
    let Catalog { abilities, turns, mut hold, mut airborne } = catalog;
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

        // The turn is spent on the cast. With something watching, a
        // projectile is seen to fly before it lands: what it does waits in
        // the air for the first pass after the flight has been seen.
        if let Some(flight) = flight_of(&landing, &world) {
            world.cues.write(Cued { actor: user, cue: flight });
            if hold.is_watched() {
                hold.launch();
                airborne.0.push(landing);
                resolution.done(user, def.time);
                continue;
            }
        }
        land(landing, &abilities, &mut world, &mut events);
        resolution.done(user, def.time);
    }
}

/// Lands every use in the air. A pass runs only while nothing is held,
/// so whatever is in the air when one runs has been seen to fly; landing
/// counts as progress, so the loop goes on to deal the next turn once the
/// burst has been seen in turn.
pub fn land_abilities(
    abilities: Res<Abilities>,
    mut hold: ResMut<TurnHold>,
    mut airborne: ResMut<Airborne>,
    mut turns: ResMut<Turns>,
    mut world: EffectWorld,
    mut events: MessageWriter<AbilityEvent>,
) {
    for landing in std::mem::take(&mut airborne.0) {
        land(landing, &abilities, &mut world, &mut events);
        hold.land();
        turns.progress = true;
    }
}

/// Lands a use: the burst over the footprint, the effects, and the report.
fn land(landing: Landing, abilities: &Abilities, world: &mut EffectWorld<'_, '_>, events: &mut MessageWriter<AbilityEvent>) {
    let (user, id) = (landing.user, landing.ability);
    if let Some(burst) = burst_of(&landing, world) {
        world.cues.write(Cued { actor: user, cue: burst });
    }
    for built in &abilities.built[id.index()] {
        if built.chance < 100 && !world.rng.random_ratio(u32::from(built.chance), 100) {
            continue;
        }
        built.effect.apply(&landing, world);
    }
    events.write(AbilityEvent::Used { user, ability: id, aim: landing.aim, targets: landing.targets });
}

/// `cell` as a cue anchors it: the target standing on it, so a burst on
/// someone knocked back goes with them and a bolt at someone walking away
/// still lands on them, or the cell itself when nobody does.
fn anchor_of(landing: &Landing, world: &EffectWorld<'_, '_>, cell: Point) -> Anchor {
    landing.targets.iter().find(|t| world.position(**t) == Some(cell)).map(|t| Anchor::on(*t, cell)).unwrap_or(Anchor::cell(cell))
}

/// The flight to where a projectile stopped, for a shape that has one.
fn flight_of(landing: &Landing, world: &EffectWorld<'_, '_>) -> Option<Cue> {
    let stop = landing.landed_at.filter(|stop| *stop != landing.origin)?;
    Some(Cue::Flight { from: Anchor::on(landing.user, landing.origin), to: anchor_of(landing, world, stop), look: LookOf::Ability(landing.ability) })
}

/// The burst over the footprint, for a shape that covers anything.
fn burst_of(landing: &Landing, world: &EffectWorld<'_, '_>) -> Option<Cue> {
    if landing.cells.is_empty() {
        return None;
    }
    Some(Cue::Burst { on: landing.cells.iter().map(|c| anchor_of(landing, world, *c)).collect(), look: LookOf::Ability(landing.ability) })
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
    let stat = |s: StatId| block.value(s, &state.registries.stats);
    let gates = Gates { statuses, worn: &worn, stat: &stat };

    let no_pools = Pools::default();
    let pools = pools.unwrap_or(&no_pools);
    let pool = |s: StatId| pools.get(s);
    let items = |tag: TagId| count_tagged(inventory, tag, state);
    let purse = Purse {
        pool: &pool,
        charges: source.map(|e| charges_of(e, state)),
        // No health component means nothing to spend it from, and a cost
        // in health should refuse rather than silently succeed.
        health: health.map(|h| h.current).unwrap_or(0),
        items: &items,
    };
    blocked(def, &gates, &purse, now, cooldowns.map(|c| c.ready_at(id)).unwrap_or(0))
}

/// What a charge costs `source` from: its [`Charges`] when it counts them,
/// else its stack, else itself, which is one use.
fn charges_of(source: Entity, state: &UserState<'_, '_>) -> u16 {
    if let Ok(charges) = state.charges.get(source) {
        return charges.left;
    }
    match state.tagged.get(source) {
        Ok((_, Some(stack))) => stack.count.min(u32::from(u16::MAX)) as u16,
        _ => 1,
    }
}

/// Spends `amount` charges from `source`, by the same rule
/// [`charges_of`] counts them: off its [`Charges`], else off its stack,
/// else the item itself, and an item spent to nothing is despawned.
fn spend_charges(source: Entity, amount: u16, state: &mut UserState<'_, '_>) {
    if let Ok(mut charges) = state.charges.get_mut(source) {
        charges.left = charges.left.saturating_sub(amount);
        return;
    }
    let left = match state.tagged.get_mut(source) {
        Ok((_, Some(mut stack))) => {
            stack.count = stack.count.saturating_sub(u32::from(amount));
            stack.count
        }
        _ => 0,
    };
    if left == 0 && state.items.contains(source) {
        state.commands.entity(source).despawn();
    }
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
                    health.current -= amount;
                }
            }
            Cost::Charge { amount } => {
                if let Some(source) = source {
                    spend_charges(source, amount, state);
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

/// Tells the mind holding the turn what it may use: what the gate offered
/// it, in [`PerceiveSet::Annotate`](crate::plugin::PerceiveSet::Annotate),
/// so a tactic is never offered an ability the resolver would refuse.
pub fn perceive_abilities(mut thinking: ResMut<crate::minds::Thinking>, offered: Res<Offered>) {
    let Some(thinker) = thinking.actor() else { return };
    let usable = offered.usable_by(thinker).to_vec();
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.usable = usable;
    }
}

/// An actor as [`refresh_known`] reads it: what it knows, what it is
/// granted of itself, and what it wears and carries.
type Learner = (&'static mut Known, Option<&'static Grants>, Option<&'static Equipped>, Option<&'static Inventory>);

/// Rebuilds every actor's [`Known`] from what it is, wears and carries.
///
/// Rebuilt rather than edited, the way gear modifiers are: a wand put down
/// takes its ability with it and nothing has to remember that it did. An
/// actor's own [`Grants`] come first, so a wand lending an ability the
/// actor already knows does not make it depend on the wand; then what is
/// worn, then the rest of the bag, so a charge is spent from what is in
/// hand before what is in the pack.
pub fn refresh_known(mut actors: Query<Learner>, lent: Query<&Grants, Without<Known>>) {
    for (mut known, innate, equipped, carried) in &mut actors {
        known.clear();
        for ability in innate.map(|g| g.0.as_slice()).unwrap_or(&[]) {
            known.learn(*ability, None);
        }
        let worn = equipped.into_iter().flat_map(|e| e.0.worn().map(|(_, item)| item));
        let bag = carried.into_iter().flat_map(|bag| bag.items.iter().copied());
        for item in worn.chain(bag) {
            let Ok(grants) = lent.get(item) else { continue };
            for ability in &grants.0 {
                known.learn(*ability, Some(item));
            }
        }
    }
}

/// Turns using an item that lends an ability into using that ability.
///
/// An alternate action, in the shape of [`Bump`](crate::bump::Bump): read
/// in [`ResolveSet::Redirect`], it writes a [`Use`] of the first ability
/// the item grants, aimed at the user's own cell, and claims nothing, so
/// the ability resolver gates, pays and lands it as if the ability had
/// been called on by name, with the charge spent from the item. The item
/// resolver leaves such a use alone. An aimed ability used this way lands
/// on the user's feet and is refused; a screen that offers the item opens
/// the targeting cursor on it instead, through
/// [`Known::source_of`]. A use of an item that grants nothing is the
/// game's, reported as [`ItemEvent::Used`](crate::items::ItemEvent::Used)
/// as before.
pub fn redirect_item_uses(
    mut intents: MessageReader<Intent<UseItem>>,
    lends: Query<&Grants, With<Item>>,
    carriers: Query<(&Position, &Inventory), With<MyTurn>>,
    mut uses: MessageWriter<Intent<Use>>,
) {
    for intent in intents.read() {
        let Ok((pos, bag)) = carriers.get(intent.actor) else { continue };
        let item = intent.action.0;
        if !bag.contains(item) {
            continue;
        }
        let Some(ability) = lends.get(item).ok().and_then(|g| g.0.first().copied()) else { continue };
        uses.write(Intent::new(intent.actor, Use { ability, aim: pos.0 }));
    }
}

/// Abilities: the use action, the state a use spends, and the seam every
/// effect is registered through.
///
/// Needs [`Abilities`], [`Registries`] and the run's [`Seed`](crate::seed::Seed)
/// before play begins, and combat, since an ability's damage goes down the same
/// pipeline a sword's does. Register every effect an ability file names
/// with [`AddEffect::add_effect`] while the app is built, then build
/// [`Abilities`] from the loaded definitions.
///
/// The engine's own effects are not registered here: they live in
/// [`effects`](crate::effects), and a game adds the ones its content names
/// with [`AddEngineEffects`](crate::effects::AddEngineEffects) or one at a
/// time.
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
        use crate::seed::AddStream;
        app.register_required_components::<Actor, Known>();
        app.register_required_components::<Actor, Pools>();
        app.register_required_components::<Actor, Cooldowns>();
        app.init_resource::<EffectKinds>()
            .init_resource::<Offered>()
            .init_resource::<Airborne>()
            .add_message::<AbilityEvent>()
            .add_action::<Use>()
            .needs::<Abilities>("AbilitiesPlugin", "`Abilities::load(ron, &EffectKinds, &names)`, the game's abilities with their effects built")
            .add_stream::<AbilityRng>("AbilitiesPlugin")
            .needs::<Registries>("AbilitiesPlugin", "`Registries`, with the stats an ability's costs and requirements name")
            .add_systems(Turn, offer_abilities.in_set(crate::plugin::DecideSet::Offer))
            .add_systems(Turn, perceive_abilities.in_set(crate::plugin::PerceiveSet::Annotate))
            .add_systems(Turn, redirect_item_uses.in_set(ResolveSet::Redirect))
            // What has landed, then what is cast this pass.
            .add_systems(Turn, (land_abilities, resolve_abilities).chain().in_set(ResolveSet::Act))
            .add_systems(Turn, refresh_known.in_set(TurnSet::React));
    }

    // In `finish` like every other plugin's, so the order a game lists its
    // plugins in never matters.
    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::combat::CombatPlugin>(app, "AbilitiesPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatRules, DamageDealt, Faction};
    use crate::components::{Actor, Player, RevealsMap};
    use crate::effects::AddEngineEffects;
    use rl_core::{Direction, RunSeed};
    use rl_grid::TileId;
    use rl_rules::content::Registry;
    use rl_rules::faction::FactionDef;
    use rl_rules::{DamageKind, SlotDef, StatDef, StatusDef, TagDef};

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
                factions: Registry::from_defs(vec![FactionDef::new("us"), FactionDef::new("them")]).unwrap(),
            }
        }

        fn names(&self) -> Names<'_> {
            Names::new().stats(&self.stats).statuses(&self.statuses).tags(&self.tags).slots(&self.slots).damage_kinds(&self.kinds)
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
    (
        name: "quaff",
        aim: SelfOnly,
        mode: Own,
        costs: [Charge(1)],
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

        fn from_args(args: &RawValue, _names: &Names<'_>) -> Result<Self, String> {
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
            crate::minds::MindsPlugin,
            crate::status::StatusPlugin,
            crate::items::ItemsPlugin,
            AbilitiesPlugin,
        ));
        app.add_engine_effects().add_effect::<Mark>();

        let content = Content::new();
        let abilities = Abilities::load(ABILITIES, app.world().resource::<EffectKinds>(), &content.names()).expect("the abilities load and build");

        let combat = CombatRules::new(&content.factions).hostile(content.factions.expect("us"), content.factions.expect("them"));

        let start = crate::testing::surface(&mut app);
        app.insert_resource(combat);
        app.insert_resource(crate::seed::Seed(RunSeed(5)));
        app.insert_resource(Registries {
            damage_kinds: content.kinds,
            factions: content.factions,
            stats: content.stats,
            statuses: content.statuses,
            tags: content.tags,
            slots: content.slots,
            gases: Default::default(),
        });
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
        app.world().get::<Health>(e).expect("health").current
    }

    fn cues(app: &mut App) -> Vec<Cue> {
        app.world_mut().resource_mut::<Messages<Cued>>().drain().map(|c| c.cue).collect()
    }

    /// With something watching, a bolt is in the air until its flight has
    /// been seen: nothing is hurt and nobody is dealt a turn until the hold
    /// lets go, and then it lands, burns, and is waited on again for its
    /// burst before the turn comes round.
    #[test]
    fn watched_a_bolt_lands_only_after_its_flight_has_been_seen() {
        let (mut app, start) = app();
        let bolt = ability(&app, "bolt");
        let me = caster(&mut app, start, 20, &[bolt]);
        let them = foe(&mut app, start.offset(3, 0));
        settle(&mut app);
        app.world_mut().resource_mut::<TurnHold>().watch();

        app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: start.offset(3, 0) }));
        app.update();
        assert_eq!(hp(&app, them), 20, "in the air");
        assert!(matches!(cues(&mut app).as_slice(), [Cue::Flight { .. }]), "the flight, and nothing else yet");
        let hold = *app.world().resource::<TurnHold>();
        assert!(hold.is_held() && hold.in_flight(), "held for the flight, with one in the air");
        assert_eq!(app.world().get::<Pools>(me).unwrap().get(StatId::from_raw(0)), 15, "paid on the cast");
        app.update();
        assert_eq!(hp(&app, them), 20, "still, while held");

        app.world_mut().resource_mut::<TurnHold>().release();
        app.update();
        assert_eq!(hp(&app, them), 16, "seen to fly, it lands");
        assert!(matches!(cues(&mut app).as_slice(), [Cue::Burst { .. }]), "and bursts where it did");
        let hold = *app.world().resource::<TurnHold>();
        assert!(hold.is_held() && !hold.in_flight(), "held again for the burst, nothing in the air");
        assert!(app.world().get::<MyTurn>(me).is_none(), "no turn dealt while the burst is seen");

        app.world_mut().resource_mut::<TurnHold>().release();
        app.update();
        assert!(app.world().get::<MyTurn>(me).is_some(), "then the turn comes round");
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
        let defs = rl_rules::ability::load(r#"[(name: "hex", mode: Own, effects: [(kind: "Curse", args: ())])]"#, &content.names()).unwrap();
        let mut kinds = EffectKinds::default();
        kinds.declare::<crate::effects::Harm>();
        let Err(err) = Abilities::build(defs, &kinds, &content.names()) else { panic!("it should not build") };
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
        app.world_mut().get_mut::<Health>(me).expect("health").current = 22;

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
            crate::minds::Mind(Arc::new(brain)),
            crate::minds::Perception(10),
            crate::minds::Profile(MovementProfile::default()),
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
            crate::minds::Mind(Arc::new(Brain::new().then(UseAbility::default()))),
            crate::minds::Perception(10),
            crate::minds::Profile(MovementProfile::default()),
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

    /// A potion is an item that grants an ability costing a charge: using
    /// the item uses the ability, the charge comes off the stack, the last
    /// one takes the bottle with it, and a wand that counts its charges is
    /// still a wand at zero.
    #[test]
    fn using_an_item_that_grants_an_ability_uses_it_and_spends_the_item() {
        let (mut app, start) = app();
        let quaff = ability(&app, "quaff");
        let me = caster(&mut app, start, 0, &[]);
        let potions = app.world_mut().spawn((Item, Grants(vec![quaff]), Stack { key: 1, count: 2 })).id();
        let wand = app.world_mut().spawn((Item, Grants(vec![quaff]), Charges::full(1))).id();
        app.world_mut().get_mut::<Inventory>(me).unwrap().items = vec![potions, wand];
        app.world_mut().get_mut::<Health>(me).unwrap().current = 10;
        settle(&mut app);
        assert!(app.world().get::<Known>(me).unwrap().has(quaff), "carried, not worn, and known");
        assert_eq!(app.world().get::<Known>(me).unwrap().source_of(quaff), Some(potions), "spent from the first thing in the bag that lends it");

        app.world_mut().write_message(Intent::new(me, UseItem(potions)));
        app.update();
        assert_eq!(hp(&app, me), 15, "drunk");
        assert_eq!(app.world().get::<Stack>(potions).map(|s| s.count), Some(1), "one off the stack");
        assert_eq!(app.world().resource::<Turns>().now(), 100, "for the ability's time");
        assert!(app.world_mut().resource_mut::<Messages<crate::items::ItemEvent>>().drain().next().is_none(), "and nothing for the game to answer");

        app.world_mut().write_message(Intent::new(me, UseItem(potions)));
        app.update();
        assert_eq!(hp(&app, me), 20);
        assert!(app.world().get_entity(potions).is_err(), "the last one took the bottle");
        assert_eq!(app.world().get::<Inventory>(me).unwrap().items, vec![wand], "and the bag forgot it");
        assert_eq!(app.world().get::<Known>(me).unwrap().source_of(quaff), Some(wand), "the wand lends it now");

        app.world_mut().write_message(Intent::new(me, Use { ability: quaff, aim: start }));
        app.update();
        assert_eq!(hp(&app, me), 25);
        assert_eq!(app.world().get::<Charges>(wand).copied(), Some(Charges { left: 0, max: 1 }), "a wand at zero is still a wand");
        let clock = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(me, UseItem(wand)));
        app.update();
        assert_eq!((hp(&app, me), app.world().resource::<Turns>().now()), (25, clock), "and an empty one is refused for free");
    }
}
