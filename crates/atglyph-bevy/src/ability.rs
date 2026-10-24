//! Abilities: the loop that turns a definition into something that
//! happened.
//!
//! [`atglyph_rules::ability`] holds what an ability *is* and answers whether it
//! may be used. This module owns the rest: the [`Use`] action, the
//! resolver that gates it, pays for it, resolves its footprint and lands
//! its effects, and the state a use spends and sets.
//!
//! Effects are types, not a list, for the reason actions are: the engine
//! ships one per subsystem it owns, in [`effects`](crate::effects), and a
//! game registers its own with
//! [`EngineApp::add_effect`]. There is no enum of effect kinds, no
//! `Custom { id }`, and no list anywhere for a new one to be added to. An
//! effect a game writes reaches the world through [`EffectWorld`], which
//! hands out the engine's own requests and, for anything the engine never
//! thought of, `Commands`.
//!
//! Cooldowns are absolute times on the turn queue's clock rather than
//! countdowns, so a save that restores the clock restores every cooldown
//! with it and nothing has to be ticked.

use std::collections::{BTreeMap, BTreeSet};

use atglyph_core::Point;
use atglyph_grid::{Footprint, TargetMode, burst};
use atglyph_rules::ability::{AbilityDef, AbilityId, Aim, Blocked, Cost, Gates, Purse, Targeting, Usable, aim_blocked, blocked};
use atglyph_rules::{Names, Registry, Relation, StatId, Statuses, TagId};
use bevy::prelude::*;
use rand::Rng;

use crate::app::EngineApp;
use crate::combat::{CombatRules, Dead, Faction, Health};
use crate::components::MyTurn;
use crate::cue::{AddAirborne, Airborne, CueWait, Cued, LookOf};
use crate::effects::{EffectBonus, EffectKinds, EffectWorld, Landing, Source};
use crate::items::{Equipped, Inventory, Stack, Tagged};
use crate::plugin::{ResolveSet, Turn, TurnSet};
use crate::registries::Registries;
use crate::status::{ActiveStatuses, StatBlock};
use crate::turn::{Action, Intent, Occupancy, Resolution, Turns};
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

/// What an actor can use.
///
/// A set rather than a list: an ability granted twice is known once.
/// Rebuilt rather than edited by `refresh_known`, from the actor's own
/// [`Grants`], so a grant taken away takes its ability with it and nothing
/// has to remember that it did.
#[derive(Component, Debug, Clone, Default)]
pub struct Known(BTreeSet<AbilityId>);

impl Known {
    /// An actor that knows nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `ability`.
    pub(crate) fn learn(&mut self, ability: AbilityId) {
        self.0.insert(ability);
    }

    /// Forgets everything.
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    /// Whether `ability` is known.
    pub fn has(&self, ability: AbilityId) -> bool {
        self.0.contains(&ability)
    }

    /// Every ability known, in id order.
    pub fn iter(&self) -> impl Iterator<Item = AbilityId> + '_ {
        self.0.iter().copied()
    }

    /// Whether nothing is known.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
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

    /// Takes `amount` off, never below zero.
    pub(crate) fn spend(&mut self, stat: StatId, amount: i32) {
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

/// The abilities an actor knows of itself.
///
/// On the actor, never on an item: an item does what its triggers say and
/// lends nothing, so what an actor knows reads as what it is and has
/// learned. An upgrade that teaches something pushes onto this. [`Known`]
/// is rebuilt from it every pass.
#[derive(Component, Debug, Clone, Default)]
pub struct Grants(pub Vec<AbilityId>);

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

/// The abilities a game registered, with their effects built.
#[derive(Resource)]
pub struct Abilities {
    defs: Registry<AbilityDef>,
    /// Parallel to `defs` by id index.
    built: Vec<crate::effects::Effects>,
}

impl Abilities {
    /// Loads abilities from RON and builds their effects, every name resolved
    /// through `names`: [`atglyph_rules::ability::load`] then [`build`](Self::build),
    /// for a game whose abilities are one file.
    ///
    /// Fails at whichever step finds something wrong, listing everything it
    /// found: every unknown name, or every effect that would not build.
    pub fn load(text: &str, kinds: &EffectKinds, names: &Names<'_>) -> Result<Self, atglyph_rules::ContentError> {
        Self::build(atglyph_rules::ability::load(text, names)?, kinds, names)
    }

    /// Builds the effects of every ability in `defs` through `kinds`.
    ///
    /// Fails naming the ability, the effect and what was wrong with it,
    /// and lists every failure rather than the first.
    pub fn build(defs: Registry<AbilityDef>, kinds: &EffectKinds, names: &Names<'_>) -> Result<Self, atglyph_rules::ContentError> {
        let mut errors = Vec::new();
        let mut built = Vec::new();
        for (_, def) in defs.iter() {
            match crate::effects::Effects::build(&def.effects, kinds, names) {
                Ok(effects) => built.push(effects),
                Err(mine) => {
                    errors.extend(mine.into_iter().map(|e| format!("{}: {e}", def.name)));
                    built.push(crate::effects::Effects::default());
                }
            }
        }
        if !errors.is_empty() {
            return Err(atglyph_rules::ContentError::Invalid(errors));
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
        self.built[id.index()].describe(registries, EffectBonus::default())
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
type Bearing = (Option<&'static Inventory>, Option<&'static Equipped>, Option<&'static ActiveStatuses>, Option<&'static StatBlock>);

/// What the resolver reads about the user, apart from where it stands.
#[derive(bevy::ecs::system::SystemParam)]
pub struct UserState<'w, 's> {
    users: Query<'w, 's, Spender, With<MyTurn>>,
    gear: Query<'w, 's, Bearing>,
    tagged: Query<'w, 's, (Option<&'static Tagged>, Option<&'static mut Stack>)>,
    registries: Res<'w, Registries>,
    commands: Commands<'w, 's>,
}

/// A use being worked out: who, what, pointed how, from where, and at what.
///
/// What is pointed is an ability or a thing's own aimed `use` trigger, and
/// nothing past [`Bystanders::land`] can tell which: both are a
/// [`Targeting`], so a bolt sent from the pack and a bolt an actor knows
/// are landed, previewed and refused by one rule.
#[derive(Debug, Clone, Copy)]
pub struct Aimed {
    /// Who is using it.
    pub user: Entity,
    /// What it is, for the landing to say: an ability, or the trigger of
    /// the thing used.
    pub source: Source,
    /// How it is pointed: its shape and reach, what it wants under it, and
    /// whether its cell must be seen.
    pub targeting: Targeting,
    /// Where the user stands.
    pub origin: Point,
    /// Where it is pointed: the user's own cell for an ability that needs
    /// no cursor.
    pub aim: Point,
    /// Whether the user can see `aim`, `None` when it carries no sight to
    /// ask. Most minds carry none and are trusted with their tactic's pick.
    pub sees_aim: Option<bool>,
}

impl Aimed {
    /// The ability `ability`, defined by `def`, pointed by `user` from
    /// `origin` at `aim`.
    pub fn ability(user: Entity, ability: AbilityId, def: &AbilityDef, origin: Point, aim: Point, sees_aim: Option<bool>) -> Self {
        Self { user, source: Source::Ability(ability), targeting: def.targeting(), origin, aim, sees_aim }
    }
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

/// Turns an aimed use or an ability back on whoever aimed it, `percent`
/// times in a hundred: a body too dense or too strange for either to
/// bite, and a trick any game can give a body of its own.
///
/// Read by [`Bystanders::land`] rather than by a game reacting afterwards,
/// because nothing fires between a use being pointed and its effects
/// landing for a game to turn the aim back in: [`Bystanders::land`] is the
/// one place, shared by an ability and a thing's aimed use alike, that
/// turns a footprint into the targets its effects will run on. The
/// alternative, letting every effect ask whether its own target reflects,
/// would be answered differently by every effect a game or the engine
/// ever writes.
///
/// The chance is the body's own, a rule value the game sets rather than a
/// number the engine fixes: one golem may turn half of what is thrown at
/// it and another every bolt. [`Reflects::ALWAYS`] and [`Reflects::NEVER`]
/// are also how a test asks for one outcome instead of counting both
/// over a span of seeds.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reflects {
    /// The chance in a hundred that a use aimed at this body is turned
    /// back. Past 100 it is read as 100, a certainty, rather than refused.
    pub percent: u8,
}

impl Reflects {
    /// Turns back everything aimed at it as a foe.
    pub const ALWAYS: Reflects = Reflects { percent: 100 };
    /// Turns back nothing: the component stays, so whatever reads it as a
    /// trait of the body still does, but no roll ever comes up.
    pub const NEVER: Reflects = Reflects { percent: 0 };

    /// A body that turns a use back `percent` times in a hundred, held to
    /// a hundred.
    pub fn percent(percent: u8) -> Reflects {
        Reflects { percent: percent.min(100) }
    }
}

impl Default for Reflects {
    /// One time in two, the chance every reflecting body had before the
    /// chance was its own.
    fn default() -> Self {
        Reflects { percent: 50 }
    }
}

/// Written each time `Bystanders::reflect` turns a landing back: the
/// body [`Reflects`] marked, and whoever aimed the use, now the one it
/// lands on instead.
///
/// Nothing else says a reflection happened: the harm it causes is reported
/// the way any other harm is, by [`crate::combat::DamageDealt`], with
/// nothing to say why the caster is the one hurt. This is that reason, for
/// a narrator to word and for a game to learn a reflecting body by, read
/// beside the landing rather than folded into it, since a preview or a
/// refused aim must not write one.
#[derive(Message, Debug, Clone, Copy)]
pub struct Reflected {
    /// The body [`Reflects`] marked, which turned the landing back.
    pub by: Entity,
    /// Whoever aimed it, now the one the landing lands on.
    pub onto: Entity,
    /// What was turned back: an ability, a trigger, or a prop's offer.
    pub source: Source,
}

/// Something aimed came down: an ability, or a thing's own aimed use, with
/// whom it landed on and who turned it back.
///
/// Written for both, in the pass each lands, so a rule about anything a
/// user points at someone reads one report and cannot treat a bolt sent
/// from the pack differently from a bolt an actor knows: stealth's
/// [`reveal_attackers`](crate::stealth::reveal_attackers) is that rule.
/// [`AbilityEvent::Used`] says the same of an ability for the game and the
/// log, and stays the report a game reads; an aimed use had no report of
/// whom it landed on before this, since [`ItemEvent::Used`](crate::items::ItemEvent::Used)
/// is written when the use is made and the landing is worked out later,
/// when its trigger lands.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub(crate) struct LandedOn {
    /// Who aimed it.
    pub user: Entity,
    /// What it was: an ability, or the `use` trigger of the thing used.
    pub source: Source,
    /// Everyone it landed on, the user too when it was turned back or
    /// caught them.
    pub targets: Vec<Entity>,
    /// Every body that turned its share back on the user, as
    /// [`Landing::turned_by`] holds them.
    pub turned_by: Vec<Entity>,
}

impl LandedOn {
    /// What `landing` reports as it comes down.
    pub(crate) fn of(landing: &Landing) -> Self {
        Self { user: landing.user, source: landing.source, targets: landing.targets.clone(), turned_by: landing.turned_by.clone() }
    }

    /// Whether it was aimed at anyone but its user: it landed on another,
    /// or another turned it back.
    ///
    /// A body that turned it back was aimed at as surely as one it landed
    /// on, so a bolt reflected onto its own caster still counts.
    pub(crate) fn reached_another(&self) -> bool {
        !self.turned_by.is_empty() || self.targets.iter().any(|t| *t != self.user)
    }
}

/// The roll [`Reflects`] spends, its own stream so adding or rolling for a
/// reflecting body never shifts a single roll combat or an effect already
/// drew: the same reasoning [`crate::minds::MindRng`] is kept apart for.
pub(crate) type ReflectRng = crate::seed::RunRng<ReflectDomain>;

/// The domain [`ReflectRng`] draws under.
pub(crate) struct ReflectDomain;

impl crate::seed::StreamDomain for ReflectDomain {
    const DOMAIN: atglyph_core::SeedDomain = crate::seed::REFLECT;
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
    reflects: Query<'w, 's, &'static Reflects>,
    // `Option`: nothing here needs the stream derived until a body
    // actually carries `Reflects`, so a game that has added neither
    // `AbilitiesPlugin` nor `EffectsPlugin`'s own `add_stream` for it, and
    // marked nobody `Reflects`, never misses it.
    rng: Option<ResMut<'w, ReflectRng>>,
    // `Option` for the same reason as `rng`: `combat::Arena` and the items
    // resolver build a `Bystanders` with neither `AbilitiesPlugin` nor
    // `EffectsPlugin` added, and nothing here writes one until a body
    // actually reflects, which the stream above already requires both of
    // to roll for.
    reflected: Option<MessageWriter<'w, Reflected>>,
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
    /// The one answer to the question, for an ability and for a thing's
    /// aimed use alike. The resolvers land a use with it and the targeting
    /// cursor previews one with it, so the two cannot disagree. Who counts
    /// as hit is [`Aim::hits`] over every living actor under the footprint,
    /// the user included; what refuses an aim is [`aim_blocked`]. The
    /// landing adds nothing to what it lands, and a thing that lands its
    /// effects stronger sets its own bonus on it.
    ///
    /// Never reflects: `Self::reflect` is a second, explicit step, since
    /// this same call also answers the targeting cursor's preview and a
    /// refused aim, where nothing should be spent or decided yet, and a
    /// landing only one of the resolvers actually carries out.
    pub fn land(&self, aimed: Aimed, map: &WorldMap, occupancy: &Occupancy) -> Landed {
        let Aimed { user, source, targeting, origin, aim, sees_aim } = aimed;
        let Footprint { cells, path, reached, landing } = crate::combat::projectile(map, occupancy, targeting.mode, origin, aim);
        let mut refused = aim_blocked(targeting, &cells, sees_aim);
        // A projectile that stopped short of where it was pointed, at its
        // range or on whatever stood in the way, is refused rather than
        // landed where it stopped: nobody chose that spot. One that reached
        // a wall it was aimed at did get there, and lands in front of it.
        if targeting.aim.needs_cursor() && reached.is_some_and(|got| got != aim) {
            // First among the reasons: an aim that stops short has no
            // target under it either, and out of reach is the one to act on.
            refused.insert(0, Blocked::OutOfReach);
        }
        let mut targets = Vec::new();
        if targeting.aim == Aim::SelfOnly {
            // Whatever the shape, the only one it wants is the one using it.
            targets.push(user);
        } else {
            for cell in &cells {
                for who in occupancy.at(*cell) {
                    if !targets.contains(who) && self.alive.contains(*who) && targeting.aim.hits(self.relation(user, *who), *who == user) {
                        targets.push(*who);
                    }
                }
            }
        }
        Landed {
            landing: Landing {
                user,
                source,
                origin,
                aim,
                cells,
                path,
                landed_at: landing,
                targets,
                bonus: EffectBonus::default(),
                on: None,
                turned_by: Vec::new(),
            },
            refused,
        }
    }

    /// Turns back, each at its own chance, every target of `landing` but
    /// its own user that carries [`Reflects`]: the body that turns it is
    /// spared, and its share lands on the user instead, never listing the
    /// user twice.
    ///
    /// Only its own share, because the body turns back what reached it and
    /// nothing else: the rest of a burst lands where it was aimed, on
    /// everyone else it caught, and the ground it covers stays there. When
    /// a use turned back leaves nobody caught where it was aimed, as a bolt
    /// at one body does, the whole of it lands on its user, its cells the
    /// shape `mode` draws on the user's own cell, as if the user had been
    /// aimed at: a bolt that lights its cell lights the user's, and a ball
    /// bursts round them. Swapping only the targets then left the ground
    /// effects at the original aim, a turned-back bolt still setting the
    /// reflecting body's cell alight. Where a projectile stopped is kept,
    /// since that is where it was turned: its flight is seen to go there
    /// and come back to the user, and the burst goes off on the user once
    /// it has.
    ///
    /// Every body that turns its share back is kept in the landing's
    /// [`turned_by`](Landing::turned_by), for how it is seen to fly and for
    /// the [`LandedOn`] written when it comes down.
    ///
    /// The second, explicit step [`Self::land`] itself never takes: called
    /// only once an aim is no longer refused and a resolver is about to
    /// carry it out, never by the targeting cursor's preview of the same
    /// aim, so looking at a reflecting body costs nothing and decides
    /// nothing before the use is actually made.
    ///
    /// Only a use aimed at the body as a foe is turned back: one aimed
    /// [`Aim::Foe`], or aimed at the ground or at anyone by a user hostile
    /// to it. A mend or a ward aimed at allies lands as it would, since a
    /// body turns back what is thrown at it and not what is offered to it;
    /// turning back half of every mend an ally aimed at a reflecting
    /// companion made it the one companion nobody could heal.
    ///
    /// Writes a [`Reflected`] for every target it turns back; a target
    /// left standing where it was aimed writes none.
    pub(crate) fn reflect(&mut self, landing: &mut Landing, targeting: Targeting, map: &WorldMap) {
        let user = landing.user;
        let source = landing.source;
        let mut landed: Vec<Entity> = Vec::with_capacity(landing.targets.len());
        let (mut turned, mut stayed) = (false, false);
        for target in landing.targets.drain(..) {
            let thrown = match targeting.aim {
                Aim::Foe => true,
                Aim::Ground | Aim::Anyone => self.relation(user, target) == Some(Relation::Hostile),
                Aim::Ally | Aim::SelfOnly => false,
            };
            // At the default chance the roll is `random_bool(0.5)` exactly,
            // so a body inserted as `Reflects::default()` draws as the unit
            // marker did.
            let chance = self.reflects.get(target).ok().map(|r| f64::from(r.percent.min(100)) / 100.0);
            let reflected = target != user && thrown && chance.is_some_and(|p| self.rng.as_mut().is_some_and(|rng| rng.0.random_bool(p)));
            if reflected {
                landing.turned_by.push(target);
                if let Some(writer) = self.reflected.as_mut() {
                    writer.write(Reflected { by: target, onto: user, source });
                }
            }
            turned |= reflected;
            stayed |= !reflected && target != user;
            let onto = if reflected { user } else { target };
            if !landed.contains(&onto) {
                landed.push(onto);
            }
        }
        landing.targets = landed;
        if turned && !stayed {
            let at = landing.origin;
            landing.cells = match targeting.mode {
                TargetMode::Ball { radius, .. } => burst(at, radius, map.window_tiles(), |p| map.blocks_projectiles(p)),
                _ => vec![at],
            };
            landing.aim = at;
        }
    }
}

/// The abilities a game registered, the clock their cooldowns run on, and
/// the uses in the air.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Catalog<'w> {
    abilities: Res<'w, Abilities>,
    turns: Res<'w, Turns>,
    cue_wait: ResMut<'w, CueWait>,
    airborne: ResMut<'w, Airborne<Landing>>,
}

impl crate::cue::Lands for Landing {}

/// Resolves a use: gate, pay, aim, land.
///
/// A use refused costs the player nothing and keeps its turn, and costs
/// anyone else the turn, which is what stops a monster retrying an
/// ability it cannot afford forever. A use that lands on nobody still
/// costs the turn, the way a shot at nothing does.
pub(crate) fn resolve_abilities(
    mut intents: MessageReader<Intent<Use>>,
    mut reports: Reports,
    mut resolution: Resolution,
    catalog: Catalog,
    mut state: UserState,
    mut bystanders: Bystanders,
    mut world: EffectWorld,
) {
    let Catalog { abilities, turns, mut cue_wait, mut airborne } = catalog;
    let now = turns.now();
    for intent in intents.read() {
        let user = intent.actor;
        let id = intent.action.ability;
        let Ok((known, _, _, _)) = state.users.get(user) else { continue };
        if !known.has(id) {
            continue;
        }
        if !resolution.claim(user) {
            continue;
        }
        let def = abilities.get(id);
        let Some(origin) = world.position(user) else {
            resolution.failed(user, def.time);
            continue;
        };
        let aim = if def.aim.needs_cursor() { intent.action.aim } else { origin };
        let aimed = Aimed::ability(user, id, def, origin, aim, world.sight_of(user, aim));
        let Landed { mut landing, refused } = bystanders.land(aimed, world.map(), world.occupancy());

        let mut why = gate(user, id, def, &state, now);
        why.extend(refused);
        if !why.is_empty() {
            reports.events.write(AbilityEvent::Refused { user, ability: id, why });
            resolution.failed(user, def.time);
            continue;
        }
        // Not refused, and about to be cast: the one moment a reflecting
        // body's turn is actually spent, never the gate above or the
        // cursor's own look at the same aim.
        bystanders.reflect(&mut landing, def.targeting(), world.map());

        pay(user, def, &mut state);
        if def.cooldown > 0
            && let Ok((_, _, Some(mut cooldowns), _)) = state.users.get_mut(user)
        {
            cooldowns.set(id, now + def.cooldown);
        }

        // The turn is spent on the cast. With something watching, a
        // projectile is seen to fly before it lands: what it does waits in
        // the air for the first pass after the flight has been seen.
        let flights = landing.flights(&world, LookOf::Ability(id));
        let landing = if flights.is_empty() {
            landing
        } else {
            for flight in flights {
                world.cues.write(Cued { actor: user, cue: flight });
            }
            match airborne.launched(&mut cue_wait, landing) {
                None => {
                    resolution.done(user, def.time);
                    continue;
                }
                Some(landing) => landing,
            }
        };
        land(landing, &abilities, &mut world, &mut reports);
        resolution.done(user, def.time);
    }
}

/// Lands every use in the air. A pass runs only while nothing is held,
/// so whatever is in the air when one runs has been seen to fly; landing
/// counts as progress, so the loop goes on to deal the next turn once the
/// burst has been seen in turn.
pub(crate) fn land_abilities(
    abilities: Res<Abilities>,
    mut cue_wait: ResMut<CueWait>,
    mut airborne: ResMut<Airborne<Landing>>,
    mut turns: ResMut<Turns>,
    mut world: EffectWorld,
    mut reports: Reports,
) {
    for landing in airborne.landing(&mut cue_wait, &mut turns) {
        land(landing, &abilities, &mut world, &mut reports);
    }
}

/// What a use is reported by: refused or landed, for the game, and what it
/// came down on, for every rule about anything aimed.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Reports<'w> {
    events: MessageWriter<'w, AbilityEvent>,
    landed: MessageWriter<'w, LandedOn>,
}

/// Lands a use: the burst over the footprint, the effects, and the reports.
fn land(landing: Landing, abilities: &Abilities, world: &mut EffectWorld<'_, '_>, reports: &mut Reports<'_>) {
    let user = landing.user;
    let Some(id) = landing.ability() else { return };
    if let Some(burst) = landing.burst(world, LookOf::Ability(id)) {
        world.cues.write(Cued { actor: user, cue: burst });
    }
    abilities.built[id.index()].land(&landing, world);
    reports.landed.write(LandedOn::of(&landing));
    reports.events.write(AbilityEvent::Used { user, ability: id, aim: landing.aim, targets: landing.targets });
}

/// Every reason `def` may not be used by `user` right now.
fn gate(user: Entity, id: AbilityId, def: &AbilityDef, state: &UserState<'_, '_>, now: u32) -> Vec<Blocked> {
    // The caller has already found this actor holding a turn.
    let Ok((_, pools, cooldowns, health)) = state.users.get(user) else { return Vec::new() };
    let (inventory, equipped, afflicted, block) = state.gear.get(user).unwrap_or((None, None, None, None));

    let no_statuses = Statuses::default();
    let statuses = afflicted.map(|a| &a.0).unwrap_or(&no_statuses);
    let worn: Vec<(atglyph_rules::SlotId, TagId)> = equipped
        .map(|e| {
            e.0.worn()
                .flat_map(|(slot, item)| {
                    let tags = state.tagged.get(item).ok().and_then(|(t, _)| t).map(|t| t.0.as_slice()).unwrap_or(&[]);
                    tags.iter().map(move |tag| (slot, *tag))
                })
                .collect()
        })
        .unwrap_or_default();
    let no_stats = atglyph_rules::Stats::default();
    let block = block.map(|s| &s.0).unwrap_or(&no_stats);
    let stat = |s: StatId| block.value(s, &state.registries.stats);
    let gates = Gates { statuses, worn: &worn, stat: &stat };

    let no_pools = Pools::default();
    let pools = pools.unwrap_or(&no_pools);
    let pool = |s: StatId| pools.get(s);
    let items = |tag: TagId| count_tagged(inventory, tag, state);
    let purse = Purse {
        pool: &pool,
        // No health component means nothing to spend it from, and a cost
        // in health should refuse rather than silently succeed.
        health: health.map(|h| h.current).unwrap_or(0),
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
fn pay(user: Entity, def: &AbilityDef, state: &mut UserState<'_, '_>) {
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
/// Read through `usable_by` and [`why_for`](Self::why_for),
/// which answer only for the actor it was worked out for: it is rebuilt
/// every pass, so a reader holding last pass's actor would otherwise act
/// on someone else's abilities.
#[derive(Resource, Debug, Default)]
pub struct UsableAbilities {
    /// Who it was worked out for. The player as readily as a monster:
    /// what may be used is the same question for both.
    actor: Option<Entity>,
    /// What can be used now.
    usable: Vec<Usable>,
    /// What is known and cannot be used, with every reason.
    refused: Vec<(AbilityId, Vec<Blocked>)>,
}

impl UsableAbilities {
    /// What `actor` can use now, empty when it was worked out for someone
    /// else.
    pub(crate) fn usable_by(&self, actor: Entity) -> &[Usable] {
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
pub(crate) fn offer_abilities(
    mut offered: ResMut<UsableAbilities>,
    abilities: Res<Abilities>,
    turns: Res<Turns>,
    state: UserState,
    holding: Query<Entity, With<MyTurn>>,
) {
    offered.actor = None;
    offered.usable.clear();
    offered.refused.clear();
    let Ok(actor) = holding.single() else { return };
    let Ok((known, _, _, _)) = state.users.get(actor) else { return };
    offered.actor = Some(actor);
    let now = turns.now();
    for id in known.iter() {
        let def = abilities.get(id);
        let why = gate(actor, id, def, &state, now);
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
pub(crate) fn perceive_abilities(mut thinking: ResMut<crate::minds::Thinking>, offered: Res<UsableAbilities>) {
    let Some(thinker) = thinking.actor() else { return };
    let usable = offered.usable_by(thinker).to_vec();
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.usable = usable;
    }
}

/// Takes from what the mind holding the turn may use every ability aimed
/// at its user alone whose effects would find nothing to act on, by
/// [`Effect::would_act`](crate::effects::Effect::would_act), once
/// [`PerceiveSet::Annotate`](crate::plugin::PerceiveSet::Annotate), where
/// [`perceive_abilities`] tells it what it may use, is done.
///
/// Only those aimed at the user alone. One pointed somewhere is weighed
/// by whoever its footprint would land on, which already says whether it
/// has anything to act on; one aimed at its user leaves a tactic nothing
/// to weigh, and was cast whenever a foe was in sight, standing up no body
/// where none lay. Asked of the landing the resolver would land, on the
/// user's own cell, so its cells are the resolver's too: empty for a shape
/// that flies, which aimed at its own user goes nowhere, and an effect
/// asked reads the targets instead. What the menu offers the player is untouched: the
/// player is offered what they can afford and judges what it is worth.
pub(crate) fn weigh_self_abilities(world: &mut World) {
    let Some(thinking) = world.get_resource::<crate::minds::Thinking>() else { return };
    let (Some(thinker), Some(usable)) = (thinking.actor(), thinking.snapshot().map(|s| s.usable.clone())) else { return };
    if !usable.iter().any(|u| u.aim == Aim::SelfOnly) {
        return;
    }
    let Some(origin) = world.get::<crate::components::Position>(thinker).map(|p| p.0) else { return };
    let kept: Vec<Usable> = {
        let world: &World = world;
        let (Some(abilities), Some(map), Some(occupancy)) =
            (world.get_resource::<Abilities>(), world.get_resource::<WorldMap>(), world.get_resource::<Occupancy>())
        else {
            return;
        };
        usable
            .into_iter()
            .filter(|u| {
                if u.aim != Aim::SelfOnly {
                    return true;
                }
                let cells = crate::combat::projectile(map, occupancy, u.mode, origin, origin).cells;
                let source = Source::Ability(u.ability);
                let landing = Landing {
                    user: thinker,
                    source,
                    origin,
                    aim: origin,
                    cells,
                    path: Vec::new(),
                    landed_at: None,
                    targets: vec![thinker],
                    bonus: EffectBonus::default(),
                    on: None,
                    turned_by: Vec::new(),
                };
                abilities.built[u.ability.index()].would_act(&landing, world)
            })
            .collect()
    };
    if let Some(snapshot) = world.resource_mut::<crate::minds::Thinking>().snapshot_mut() {
        snapshot.usable = kept;
    }
}

/// Rebuilds every actor's [`Known`] from its own [`Grants`].
///
/// Rebuilt rather than edited, the way gear modifiers are: a grant taken
/// away takes its ability with it and nothing has to remember that it did.
pub(crate) fn refresh_known(mut actors: Query<(&mut Known, Option<&Grants>)>) {
    for (mut known, grants) in &mut actors {
        known.clear();
        for ability in grants.map(|g| g.0.as_slice()).unwrap_or(&[]) {
            known.learn(*ability);
        }
    }
}

/// Abilities: the use action, the state a use spends, and the seam every
/// effect is registered through.
///
/// Needs [`Abilities`], [`Registries`] and the run's [`Seed`](crate::seed::Seed)
/// before play begins, and combat, since an ability's damage goes down the same
/// pipeline a sword's does. Register every effect an ability file names
/// with [`EngineApp::add_effect`] while the app is built, then build
/// [`Abilities`] from the loaded definitions.
///
/// The engine's own effects are not registered here: they live in
/// [`effects`](crate::effects), and a game adds the ones its content names
/// with [`EngineApp::add_engine_effects`] or one at a
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
        use crate::app::EngineApp;
        use crate::components::Actor;
        crate::effects::ensure(app);
        app.register_required_components::<Actor, Known>();
        app.register_required_components::<Actor, Pools>();
        app.register_required_components::<Actor, Cooldowns>();
        app.add_stream::<ReflectRng>("AbilitiesPlugin");
        app.init_resource::<UsableAbilities>()
            .reset_on_new_run::<UsableAbilities>()
            .add_airborne::<Landing>()
            .add_message::<AbilityEvent>()
            .add_action::<Use>()
            .needs::<Abilities>("AbilitiesPlugin", "`Abilities::load(ron, &EffectKinds, &names)`, the game's abilities with their effects built")
            .needs::<Registries>("AbilitiesPlugin", "`Registries`, with the stats an ability's costs and requirements name")
            .add_systems(Turn, offer_abilities.in_set(crate::plugin::DecideSet::Offer))
            .add_systems(Turn, perceive_abilities.in_set(crate::plugin::PerceiveSet::Annotate))
            // After every annotator rather than among them: it reads the
            // whole world, so it runs alone, and once the snapshot is
            // whole, before any mind decides on it.
            .add_systems(Turn, weigh_self_abilities.in_set(crate::plugin::PerceiveSet::Weigh))
            // What has landed, then what is cast this pass.
            .add_systems(Turn, (land_abilities.in_set(crate::plugin::LandSet::Ability), resolve_abilities.in_set(ResolveSet::Act)).chain())
            .add_systems(Turn, refresh_known.in_set(TurnSet::React));
    }

    // In `finish` like every other plugin's, so the order a game lists its
    // plugins in never matters.
    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::combat::CombatPlugin>("AbilitiesPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::EngineApp;
    use crate::combat::{CombatRules, DamageDealt, Faction};
    use crate::components::{Actor, Player, RevealsMap};
    use crate::components::{Blocks, Position, Viewshed};
    use crate::cue::{Anchor, Cue};
    use crate::effects::{Effect, FromArgs};
    use crate::items::Item;
    use crate::places::{Arrive, Destination, MapId, Transition};
    use atglyph_core::{Direction, RunSeed};
    use atglyph_grid::TileId;
    use atglyph_rules::ability::RawValue;
    use atglyph_rules::content::Registry;
    use atglyph_rules::faction::FactionDef;
    use atglyph_rules::{DamageKind, SlotDef, StatDef, StatusDef, TagDef};

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
        effects: [(kind: "Mend", args: (roll: "5"))],
    ),
    (
        name: "quaff",
        aim: SelfOnly,
        mode: Own,
        effects: [(kind: "Mend", args: (roll: "5"))],
    ),
    (
        name: "flare",
        mode: Bolt(range: 6),
        effects: [(kind: "Harm", args: (kind: "fire", roll: "4")), (kind: "Paint", args: ())],
    ),
    (
        name: "flareball",
        mode: Ball(range: 6, radius: 1),
        effects: [(kind: "Harm", args: (kind: "fire", roll: "4")), (kind: "Paint", args: ())],
    ),
    (
        name: "probe",
        aim: Ground,
        mode: Bolt(range: 6),
        effects: [(kind: "Probe", args: ())],
    ),
    (
        name: "salve",
        aim: Ally,
        mode: Bolt(range: 6),
        effects: [(kind: "Mend", args: (roll: "5"))],
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
            let a: Args = atglyph_rules::ability::read_args(args)?;
            Ok(Self { note: a.note })
        }
    }

    /// Whether `EffectWorld::is_transition` said the aimed cell was one,
    /// for `is_transition_is_true_only_at_a_transitions_own_cell` to read
    /// back.
    #[derive(Component, Debug, Clone, Copy)]
    struct Probed(bool);

    #[derive(Debug, Clone, Copy)]
    struct Probe;

    impl Effect for Probe {
        fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
            let found = world.is_transition(landing.aim);
            world.commands.entity(landing.user).insert(Probed(found));
        }
    }

    impl FromArgs for Probe {
        const KIND: &'static str = "Probe";

        fn from_args(_args: &RawValue, _names: &Names<'_>) -> Result<Self, String> {
            Ok(Self)
        }
    }

    /// Where a use landed its cells and on whom, as the effects saw it,
    /// for the reflection tests to read back: a stand-in for any effect
    /// that changes the ground, such as fire or gas.
    #[derive(Resource, Debug, Clone, Default)]
    struct Painted {
        cells: Vec<Point>,
        targets: Vec<Entity>,
    }

    #[derive(Debug, Clone, Copy)]
    struct Paint;

    impl Effect for Paint {
        fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
            let painted = Painted { cells: landing.cells.clone(), targets: landing.targets.clone() };
            world.commands.insert_resource(painted);
        }
    }

    impl FromArgs for Paint {
        const KIND: &'static str = "Paint";

        fn from_args(_args: &RawValue, _names: &Names<'_>) -> Result<Self, String> {
            Ok(Self)
        }
    }

    fn app() -> (App, Point) {
        let mut app = crate::testing::headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin, crate::minds::MindsPlugin, crate::items::ItemsPlugin, AbilitiesPlugin));
        app.add_engine_effects().add_effect::<Mark>().add_effect::<Probe>().add_effect::<Paint>();

        let content = Content::new();
        let abilities = Abilities::load(ABILITIES, app.world().resource::<EffectKinds>(), &content.names()).expect("the abilities load and build");

        let combat = CombatRules::new().hostile(content.factions.expect("us"), content.factions.expect("them"));

        let start = crate::testing::ground(&mut app);
        app.insert_resource(combat);
        app.insert_resource(crate::seed::Seed(RunSeed(5)));
        app.insert_resource(Registries {
            damage_kinds: content.kinds,
            factions: content.factions,
            stats: content.stats,
            statuses: content.statuses,
            tags: content.tags,
            slots: content.slots,
            ..Default::default()
        });
        app.insert_resource(abilities);
        (app, start)
    }

    fn ability(app: &App, name: &str) -> AbilityId {
        app.world().resource::<Abilities>().expect(name)
    }

    fn caster(app: &mut App, at: Point, mana: i32, abilities: &[AbilityId]) -> Entity {
        let us = Faction(atglyph_rules::FactionId::from_raw(0));
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
        let them = Faction(atglyph_rules::FactionId::from_raw(1));
        app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(20), them)).id()
    }

    fn friend(app: &mut App, at: Point) -> Entity {
        let us = Faction(atglyph_rules::FactionId::from_raw(0));
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
        app.world_mut().resource_mut::<CueWait>().watch();

        app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: start.offset(3, 0) }));
        app.update();
        assert_eq!(hp(&app, them), 20, "in the air");
        assert!(matches!(cues(&mut app).as_slice(), [Cue::Flight { .. }]), "the flight, and nothing else yet");
        let cue_wait = *app.world().resource::<CueWait>();
        assert!(cue_wait.is_waiting() && cue_wait.in_flight(), "held for the flight, with one in the air");
        assert_eq!(app.world().get::<Pools>(me).unwrap().get(StatId::from_raw(0)), 15, "paid on the cast");
        app.update();
        assert_eq!(hp(&app, them), 20, "still, while held");

        app.world_mut().resource_mut::<CueWait>().release();
        app.update();
        assert_eq!(hp(&app, them), 16, "seen to fly, it lands");
        let burst = cues(&mut app);
        let [Cue::Burst { from, .. }] = burst.as_slice() else { panic!("and bursts where it did: {burst:?}") };
        assert_eq!(*from, Some(Anchor::on(them, start.offset(3, 0))), "spreading out from where it landed, on whoever it landed on");
        let cue_wait = *app.world().resource::<CueWait>();
        assert!(cue_wait.is_waiting() && !cue_wait.in_flight(), "held again for the burst, nothing in the air");
        assert!(app.world().get::<MyTurn>(me).is_none(), "no turn dealt while the burst is seen");

        app.world_mut().resource_mut::<CueWait>().release();
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

    /// A body's chance to turn a use back is one in two unless the game
    /// says otherwise.
    #[test]
    fn the_default_reflection_chance_is_one_half() {
        assert_eq!(Reflects::default().percent, 50);
    }

    /// A percent no roll could mean is held to a certainty rather than
    /// handed to the roll, which would refuse it.
    #[test]
    fn a_percent_past_a_hundred_is_held_to_a_hundred() {
        assert_eq!(Reflects::percent(250), Reflects::ALWAYS);
    }

    /// A body marked [`Reflects`] turns a bolt aimed at it back on whoever
    /// cast it at its chance: a body that always does hurts the caster and
    /// is spared, and one that never does is hurt and spares the caster,
    /// so on every cast exactly one of them is hurt, never both and never
    /// neither. A percent past a hundred written into the field is a
    /// certainty too, never a roll refused. The chance between is `rand`'s
    /// own arithmetic.
    #[test]
    fn a_reflecting_body_turns_a_bolt_back_on_its_caster_at_its_chance() {
        for (reflects, turned) in [(Reflects::ALWAYS, true), (Reflects::NEVER, false), (Reflects { percent: 250 }, true)] {
            let (mut app, start) = app();
            let bolt = ability(&app, "bolt");
            let me = caster(&mut app, start, 20, &[bolt]);
            let them = foe(&mut app, start.offset(3, 0));
            app.world_mut().entity_mut(them).insert(reflects);
            settle(&mut app);

            app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: start.offset(3, 0) }));
            app.update();

            let (me_hurt, them_hurt) = (hp(&app, me) < 30, hp(&app, them) < 20);
            assert_eq!((me_hurt, them_hurt), (turned, !turned), "{reflects:?}: the caster hurt when turned back, the body when not");
        }
    }

    /// Watched, a bolt turned back is seen to fly to the body that turned it
    /// and back to its caster, and lands only once both have been seen,
    /// bursting on the caster; one that is not turned back flies once, to
    /// the body, and bursts there, as it always did. The picture then
    /// agrees with the log's line that it was reflected. One app for each
    /// outcome.
    #[test]
    fn a_reflected_bolt_is_seen_to_fly_to_the_body_that_turned_it_and_back_to_its_caster() {
        for (reflects, turned) in [(Reflects::ALWAYS, true), (Reflects::NEVER, false)] {
            let (mut app, start) = app();
            let bolt = ability(&app, "bolt");
            let me = caster(&mut app, start, 20, &[bolt]);
            let there = start.offset(3, 0);
            let them = foe(&mut app, there);
            app.world_mut().entity_mut(them).insert(reflects);
            settle(&mut app);
            app.world_mut().resource_mut::<CueWait>().watch();
            cues(&mut app);

            app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: there }));
            app.update();
            let written = !app.world_mut().resource_mut::<Messages<Reflected>>().drain().collect::<Vec<_>>().is_empty();
            assert_eq!(written, turned, "{reflects:?}: a Reflected written exactly when it is turned back");
            let (caster, body, look) = (Anchor::on(me, start), Anchor::on(them, there), LookOf::Ability(bolt));
            let out = Cue::Flight { from: caster, to: body, look };
            let flown = cues(&mut app);
            assert_eq!((hp(&app, me), hp(&app, them)), (30, 20), "{reflects:?}: nothing lands while it is in the air");
            if turned {
                assert_eq!(flown, vec![out, Cue::Flight { from: body, to: caster, look }], "out to the body that turned it, and back");
            } else {
                assert_eq!(flown, vec![out], "one flight, to the body it lands on");
            }

            app.world_mut().resource_mut::<CueWait>().release();
            app.update();
            let burst = cues(&mut app);
            let [Cue::Burst { from, .. }] = burst.as_slice() else { panic!("{reflects:?}: a burst where it landed: {burst:?}") };
            let (hurt, burst_from) = if turned { (me, caster) } else { (them, body) };
            assert_eq!(*from, Some(burst_from), "{reflects:?}: bursting on whoever it came down on");
            assert!(hp(&app, hurt) < if turned { 30 } else { 20 }, "{reflects:?}: and hurting them");
        }
    }

    /// What came down reaches another when it landed on anyone but its user
    /// or was turned back by anyone, and not when it caught its user alone,
    /// so a bolt reflected onto its own caster still counts as aimed at
    /// another.
    #[test]
    fn landed_on_reaches_another_by_landing_on_them_or_being_turned_back_by_them() {
        let mut world = World::new();
        let (me, them) = (world.spawn_empty().id(), world.spawn_empty().id());
        let report = |targets: Vec<Entity>, turned_by: Vec<Entity>| LandedOn { user: me, source: Source::Ability(AbilityId::from_raw(0)), targets, turned_by };
        assert!(report(vec![them], vec![]).reached_another(), "landed on another");
        assert!(report(vec![me], vec![them]).reached_another(), "turned back onto its user by another");
        assert!(!report(vec![me], vec![]).reached_another(), "its user alone");
        assert!(!report(vec![], vec![]).reached_another(), "nobody at all");
    }

    /// A reflection writes a [`Reflected`] naming the reflecting body and
    /// the caster it turned the bolt back onto; a bolt that lands where it
    /// was aimed writes none. One app for each outcome.
    #[test]
    fn a_reflection_writes_what_turned_back_and_a_landing_writes_none() {
        for reflects in [Reflects::ALWAYS, Reflects::NEVER] {
            let (mut app, start) = app();
            let bolt = ability(&app, "bolt");
            let me = caster(&mut app, start, 20, &[bolt]);
            let them = foe(&mut app, start.offset(3, 0));
            app.world_mut().entity_mut(them).insert(reflects);
            settle(&mut app);

            app.world_mut().write_message(Intent::new(me, Use { ability: bolt, aim: start.offset(3, 0) }));
            app.update();

            let reflected: Vec<Reflected> = app.world_mut().resource_mut::<Messages<Reflected>>().drain().collect();
            if reflects == Reflects::NEVER {
                assert!(hp(&app, them) < 20, "never reflecting, the body is hurt");
                assert!(reflected.is_empty(), "and a landing that was not reflected writes no Reflected");
            } else {
                assert_eq!(hp(&app, them), 20, "always reflecting, the body is spared");
                assert_eq!(reflected.len(), 1, "and a reflection writes exactly one Reflected");
                assert_eq!(reflected[0].by, them, "by names the reflecting body");
                assert_eq!(reflected[0].onto, me, "onto names the caster it turned back onto");
            }
        }
    }

    /// A reflecting body turns back only what is aimed at it as a foe: a
    /// mend aimed at it by an ally, and a blast an ally aims at the ground
    /// it stands on, land on it even when it turns back everything a foe
    /// aims, and never come back on the one who used them.
    #[test]
    fn a_reflecting_body_never_turns_back_what_an_ally_aims_at_it() {
        let (mut app, start) = app();
        let (salve, burst) = (ability(&app, "salve"), ability(&app, "burst"));
        let me = caster(&mut app, start, 20, &[salve, burst]);
        let there = start.offset(4, 0);
        let ours = friend(&mut app, there);
        app.world_mut().entity_mut(ours).insert((Reflects::ALWAYS, Health { current: 10, max: 20 }));
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: salve, aim: there }));
        app.update();
        assert_eq!(hp(&app, ours), 15, "an ally's mend lands on the reflecting body");

        settle(&mut app);
        app.world_mut().write_message(Intent::new(me, Use { ability: burst, aim: there }));
        app.update();
        assert_eq!(hp(&app, ours), 12, "and so does an ally's blast on its ground");
        assert_eq!(hp(&app, me), 30, "and neither came back on the one who used it");
    }

    /// A bolt turned back lands whole on its caster: the cells its ground
    /// effects run over are the caster's own, never the reflecting body's,
    /// and a bolt that is not turned back lands both on the body it was
    /// aimed at. One app for each outcome.
    #[test]
    fn a_reflected_bolt_lands_its_cells_where_its_caster_stands() {
        for (reflects, turned) in [(Reflects::ALWAYS, true), (Reflects::NEVER, false)] {
            let (mut app, start) = app();
            let flare = ability(&app, "flare");
            let me = caster(&mut app, start, 20, &[flare]);
            let there = start.offset(3, 0);
            let them = foe(&mut app, there);
            app.world_mut().entity_mut(them).insert(reflects);
            settle(&mut app);

            app.world_mut().write_message(Intent::new(me, Use { ability: flare, aim: there }));
            app.update();

            let painted = app.world().get_resource::<Painted>().expect("the flare landed").clone();
            assert_eq!(hp(&app, me) < 30, turned, "{reflects:?}: the caster hurt exactly when it is turned back");
            if turned {
                assert_eq!(painted.cells, vec![start], "a reflected bolt lands its cells on its caster");
                assert_eq!(painted.targets, vec![me], "and its caster is its only target");
            } else {
                assert_eq!(painted.cells, vec![there], "a bolt not turned back lands on the body it was aimed at");
                assert_eq!(painted.targets, vec![them], "and hits it");
            }
        }
    }

    /// A ball that catches only the body that turns it back lands whole on
    /// its caster, as a bolt does: it bursts round the caster, as if the
    /// caster had been aimed at, and the caster is its only target.
    #[test]
    fn a_ball_turned_back_by_the_only_body_it_caught_bursts_round_its_caster() {
        let (mut app, start) = app();
        let ball = ability(&app, "flareball");
        let me = caster(&mut app, start, 20, &[ball]);
        let there = start.offset(4, 0);
        let them = foe(&mut app, there);
        app.world_mut().entity_mut(them).insert(Reflects::ALWAYS);
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: ball, aim: there }));
        app.update();

        assert!(hp(&app, me) < 30, "the ball is turned back onto its caster");
        let painted = app.world().get_resource::<Painted>().expect("the ball landed").clone();
        let mut cells = painted.cells.clone();
        cells.sort_by_key(|p| (p.y, p.x));
        let mut around: Vec<Point> = atglyph_core::geometry::disc(start, 1).collect();
        around.sort_by_key(|p| (p.y, p.x));
        assert_eq!(cells, around, "a ball turned back by its only target bursts round its caster");
        assert_eq!(painted.targets, vec![me], "and lands on its caster alone");
        assert_eq!(hp(&app, them), 20, "sparing the body that turned it");
    }

    /// A body in a burst turns back only its own share: the two others it
    /// caught are hurt where it was aimed, as is the caster in the
    /// reflecting body's place, and the reflecting body is spared; the
    /// ground stays where it was aimed. One app for each outcome.
    #[test]
    fn a_body_in_a_burst_turns_back_only_its_own_share() {
        for (reflects, turned) in [(Reflects::ALWAYS, true), (Reflects::NEVER, false)] {
            let (mut app, start) = app();
            let ball = ability(&app, "flareball");
            let me = caster(&mut app, start, 20, &[ball]);
            let there = start.offset(4, 0);
            let them = foe(&mut app, there);
            let (above, below) = (foe(&mut app, there.offset(0, -1)), foe(&mut app, there.offset(0, 1)));
            app.world_mut().entity_mut(them).insert(reflects);
            settle(&mut app);

            app.world_mut().write_message(Intent::new(me, Use { ability: ball, aim: there }));
            app.update();

            assert!(hp(&app, above) < 20 && hp(&app, below) < 20, "{reflects:?}: the two others are hurt where it was aimed either way");
            let painted = app.world().get_resource::<Painted>().expect("the ball landed").clone();
            assert!(painted.cells.contains(&there), "{reflects:?}: and the ground stays where it was aimed");
            assert_eq!(hp(&app, me) < 30, turned, "{reflects:?}: the caster hurt in its place exactly when it is turned back");
            if turned {
                assert_eq!(hp(&app, them), 20, "the reflecting body is spared its share");
            } else {
                assert!(hp(&app, them) < 20, "not turned back, the body is hurt with the rest");
            }
        }
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
        assert!(app.world().get::<ActiveStatuses>(me).unwrap().0.has(atglyph_rules::StatusId::from_raw(1)), "braced");

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

    /// `EffectWorld::is_transition` says a cell carrying a `Transition` is
    /// one, and nowhere else is, so an effect that must never move anyone
    /// onto a floor's own way through, a blink among them, can tell the
    /// one cell it must refuse from the rest of the open ground.
    #[test]
    fn is_transition_is_true_only_at_a_transitions_own_cell() {
        let (mut app, start) = app();
        let there = start.offset(2, 0);
        app.world_mut().spawn((Position(there), Transition { to: Destination::Place { map: MapId::ZERO, arrive: Arrive::At(Point::ZERO) } }));
        let probe = ability(&app, "probe");
        let me = caster(&mut app, start, 20, &[probe]);
        settle(&mut app);

        app.world_mut().write_message(Intent::new(me, Use { ability: probe, aim: there }));
        app.update();
        assert_eq!(app.world().get::<Probed>(me).map(|p| p.0), Some(true), "a transition stands at the aimed cell");

        app.world_mut().write_message(Intent::new(me, Use { ability: probe, aim: start.offset(5, 0) }));
        app.update();
        assert_eq!(app.world().get::<Probed>(me).map(|p| p.0), Some(false), "nothing stands at this one");
    }

    /// An ability names an effect nobody registered: a failure at load,
    /// naming the ability and listing what is registered, rather than a
    /// surprise the first time a player presses the key.
    #[test]
    fn an_unregistered_effect_is_a_load_failure_that_names_the_ability() {
        let content = Content::new();
        let defs = atglyph_rules::ability::load(r#"[(name: "hex", mode: Own, effects: [(kind: "Curse", args: ())])]"#, &content.names()).unwrap();
        let mut kinds = EffectKinds::default();
        kinds.declare::<crate::effects::Harm>();
        let Err(err) = Abilities::build(defs, &kinds, &content.names()) else { panic!("it should not build") };
        let atglyph_rules::ContentError::Invalid(errs) = err else { panic!("expected a validation failure") };
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
        use atglyph_rules::ai::tactics::{Hunt, UseAbility};
        use atglyph_rules::{Brain, MovementProfile};
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

    /// A monster does not cast an ability aimed at itself alone whose
    /// effects would find nothing to act on: a mend at full health, with a
    /// foe in sight, is left alone, and cast once it is hurt.
    #[test]
    fn a_mind_casts_a_self_aimed_ability_only_when_it_has_something_to_act_on() {
        use atglyph_rules::ai::tactics::{Hunt, UseAbility};
        use atglyph_rules::{Brain, MovementProfile};
        use std::sync::Arc;

        let (mut app, start) = app();
        let quaff = ability(&app, "quaff");
        let me = caster(&mut app, start, 20, &[]);
        let them = foe(&mut app, start.offset(4, 0));
        app.world_mut().entity_mut(them).insert((
            crate::minds::Mind(Arc::new(Brain::new().then(UseAbility::default()).then(Hunt))),
            crate::minds::Perception(10),
            crate::minds::Profile(MovementProfile::default()),
            Grants(vec![quaff]),
            Inventory::default(),
        ));
        settle(&mut app);
        let used_by_them = |app: &mut App| {
            for _ in 0..3 {
                app.world_mut().write_message(Intent::new(me, crate::turn::Wait));
                app.update();
            }
            app.world_mut().resource_mut::<Messages<AbilityEvent>>().drain().any(|e| matches!(e, AbilityEvent::Used { user, .. } if user == them))
        };
        assert!(!used_by_them(&mut app), "whole, with a foe in sight, it has nothing to mend");

        app.world_mut().get_mut::<Health>(them).unwrap().current = 5;
        assert!(used_by_them(&mut app), "hurt, it mends");
    }

    /// A monster is never offered an ability it could not pay for, which
    /// is what keeps it from spending its turn being refused.
    #[test]
    fn a_mind_is_never_offered_what_it_cannot_afford() {
        use atglyph_rules::ai::tactics::UseAbility;
        use atglyph_rules::{Brain, MovementProfile};
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

        assert!(app.world().resource::<UsableAbilities>().usable.is_empty(), "an empty pool is not an offer");
        let refusals = app.world_mut().resource_mut::<Messages<AbilityEvent>>().drain().filter(|e| matches!(e, AbilityEvent::Refused { .. })).count();
        assert_eq!(refusals, 0, "it was never offered, so it never asked");
    }

    /// What an actor knows is what it is, never what it carries: a thing in
    /// the bag with `Grants` on it teaches nothing, worn or not.
    #[test]
    fn an_actor_knows_its_own_grants_and_nothing_its_bag_carries() {
        let (mut app, start) = app();
        let quaff = ability(&app, "quaff");
        let bolt = ability(&app, "bolt");
        let me = caster(&mut app, start, 0, &[bolt]);
        let charm = app.world_mut().spawn((Item, Grants(vec![quaff]))).id();
        app.world_mut().get_mut::<Inventory>(me).unwrap().items = vec![charm];
        settle(&mut app);
        let known = app.world().get::<Known>(me).unwrap();
        assert!(known.has(bolt), "its own grant");
        assert!(!known.has(quaff), "and nothing the charm in its bag carries");
    }
}
