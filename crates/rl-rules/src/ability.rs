//! Abilities: the second thing an actor can spend a turn on.
//!
//! The first is an attack, which the engine owns end to end. An ability is
//! that same sentence with every part named by data: a shape from
//! [`rl_grid::TargetMode`], a list of costs, a list of requirements, a
//! time, a cooldown, and a list of effects. A game writes them in RON and
//! never compiles anything to add one.
//!
//! Nothing here names a spell, mana or a cooldown's flavour. A cost is an
//! amount against a registered stat, so mana, stamina, power cells, nerve,
//! heat and powder are the same mechanism wearing different names, and the
//! engine cannot tell them apart.
//!
//! This module decides and never acts. It answers "may this be used, and
//! why not" over borrowed views of the user, and leaves paying, targeting
//! and landing to the layer that owns the world. What an effect *does*
//! lives in the Bevy layer, because doing it means writing to the world;
//! here an effect is a name, a chance and the RON its constructor will
//! parse.

use ron::extensions::Extensions;
use ron::options::Options;
use serde::Deserialize;

pub use ron::value::RawValue;

use rl_core::{Id, Point};
use rl_grid::TargetMode;

use crate::affix::TagId;
use crate::content::{ContentError, Named, Registry};
use crate::equip::SlotId;
use crate::faction::Relation;
use crate::names::Names;
use crate::stats::StatId;
use crate::status::{StatusId, Statuses};

/// What an ability wants under its footprint.
///
/// A closed set, deliberately, and not the closed taxonomy the plan
/// forbids: it enumerates the questions the faction matrix can answer
/// about a cell, the way [`TargetMode`] enumerates the shapes the geometry
/// can draw. A sixth value would mean [`Relation`] had grown a fourth.
///
/// It exists so a mind can fire an ability it cannot understand. A tactic
/// scores a footprint by how many of the right things are in it and never
/// asks what the ability does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum Aim {
    /// The user, and no cursor.
    SelfOnly,
    /// Something the relation matrix calls hostile.
    #[default]
    Foe,
    /// Something it calls allied, the user included.
    Ally,
    /// A cell, whoever happens to be on it.
    Ground,
    /// Any actor at all.
    Anyone,
}

impl Aim {
    /// Whether an actor at `relation` to the user is what this ability is
    /// looking for. `None` is a cell with nobody on it.
    pub fn wants(self, relation: Option<Relation>) -> bool {
        match (self, relation) {
            (Aim::Ground, _) => true,
            (Aim::SelfOnly, _) => false,
            (_, None) => false,
            (Aim::Foe, Some(r)) => r == Relation::Hostile,
            (Aim::Ally, Some(r)) => r == Relation::Allied,
            (Aim::Anyone, Some(_)) => true,
        }
    }

    /// Whether using this needs somewhere to point. False only for
    /// [`Aim::SelfOnly`], which is the whole reason the value exists: a
    /// cloak or a battle cry should not open a cursor.
    pub fn needs_cursor(self) -> bool {
        self != Aim::SelfOnly
    }

    /// Whether an actor standing under the footprint is hit.
    ///
    /// `relation` is how the actor stands to the user, `None` when either
    /// takes no side. The user is its own ally whatever the matrix says, so
    /// a spray aimed at allies mends whoever sprays it and a burst on the
    /// ground burns whoever stands in it, the thrower included, while an
    /// ability aimed at a foe never catches its user. One rule for the
    /// resolver, the preview and the mind that scores a footprint.
    pub fn hits(self, relation: Option<Relation>, is_user: bool) -> bool {
        match self {
            Aim::SelfOnly => is_user,
            _ => self.wants(if is_user { Some(Relation::Allied) } else { relation }),
        }
    }

    /// Whether an actor is worth pointing this at: what a mind aims for and
    /// what the player's cursor opens on, so the two make the same choice.
    ///
    /// Narrower than [`hits`](Self::hits). An ability aimed at allies is
    /// worth pointing only at the hurt, and one aimed at the ground or at
    /// anyone only at a foe, because nothing else would want either pointed
    /// at it. Anything a footprint hits that is not worth hitting is harm.
    pub fn worth_aiming_at(self, relation: Option<Relation>, is_user: bool, hurt: bool) -> bool {
        match self {
            Aim::SelfOnly => is_user,
            Aim::Ally => hurt && self.hits(relation, is_user),
            Aim::Foe | Aim::Ground | Aim::Anyone => !is_user && relation == Some(Relation::Hostile),
        }
    }
}

/// What a use spends. All or nothing: a use that cannot pay every cost
/// pays none of them.
///
/// Closed for the same reason [`Aim`] is. It enumerates what the engine
/// can decrement, and each arm is a subsystem it already owns. A game that
/// wants a sixth kind of fuel registers a stat and uses [`Cost::Pool`],
/// which is what every genre's fuel turns out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// An amount off a registered stat held as a pool.
    Pool {
        /// Which pool.
        stat: StatId,
        /// How much.
        amount: i32,
    },
    /// Charges off whatever granted the ability.
    Charge {
        /// How many.
        amount: u16,
    },
    /// Health, for the abilities that ought to hurt to use.
    Health {
        /// How much.
        amount: i32,
    },
    /// Items in the bag carrying a tag: a powder charge, a reagent, cash.
    Item {
        /// Which tag.
        tag: TagId,
        /// How many.
        count: u16,
    },
}

/// What must be true of the user before an ability may be used.
///
/// Each arm reads a table the engine already owns, so a shield bash can
/// learn it needs a shield and a shiv can learn it needs to be unseen
/// without either idea entering the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    /// The user carries this status.
    Has(StatusId),
    /// The user does not.
    Lacks(StatusId),
    /// Something with this tag is equipped, in any slot.
    Wielding(TagId),
    /// Something with this tag is equipped in this slot.
    InSlot(SlotId, TagId),
    /// A stat is above a threshold.
    Above(StatId, i32),
}

/// One effect an ability lands, as the definition holds it.
///
/// The arguments stay unparsed here because tier 1 does not know what
/// effects exist; the layer that registered the effect parses them once,
/// at load, and a typo fails at startup naming the ability rather than at
/// the moment a player presses the key.
#[derive(Debug)]
pub struct EffectSpec {
    /// The registered name of the effect.
    pub kind: String,
    /// Percentage chance it lands, rolled per use.
    pub chance: u8,
    /// The RON its constructor will read.
    pub args: Box<RawValue>,
}

impl Clone for EffectSpec {
    fn clone(&self) -> Self {
        // RawValue has no Clone, and re-parsing text that parsed once
        // cannot fail.
        Self { kind: self.kind.clone(), chance: self.chance, args: parse_args(self.args.get_ron()).expect("it parsed once") }
    }
}

/// An ability, as the engine reads it.
#[derive(Debug, Clone)]
pub struct AbilityDef {
    /// The name content refers to it by.
    pub name: String,
    /// What it wants under its footprint.
    pub aim: Aim,
    /// The shape. Range lives inside the shape.
    pub mode: TargetMode,
    /// Whether the aim must be a cell the user can see. Off for a grenade
    /// over a wall.
    pub sight: bool,
    /// Everything that must be true of the user.
    pub requires: Vec<Requirement>,
    /// Everything a use spends.
    pub costs: Vec<Cost>,
    /// What the turn costs, in hundredths of a step, like every other
    /// clock in the engine.
    pub time: u32,
    /// Hundredths before it may be used again, counted from the moment
    /// of use rather than from the end of it, so a cooldown at or below
    /// `time` is no cooldown at all. Zero is none.
    pub cooldown: u32,
    /// What lands, in order.
    pub effects: Vec<EffectSpec>,
}

impl Named for AbilityDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered ability id.
pub type AbilityId = Id<AbilityDef>;

/// Why an ability may not be used, one entry per reason.
///
/// Every reason, not the first: a panel that greys a row wants to say all
/// of what is wrong with it, and a player who is both out of mana and
/// missing a shield should be told both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// An unmet requirement.
    Needs(Requirement),
    /// An unaffordable cost.
    Cannot(Cost),
    /// Used too recently; ready at this time on the turn clock.
    Cooling {
        /// Hundredths on the same clock the turn queue runs on.
        until: u32,
    },
    /// Nothing the ability aims at is in reach. Only the layer that can
    /// resolve a footprint produces this; the gate here never does.
    NoTarget,
}

/// What the gate reads about a user.
///
/// Borrowed rather than owned, and filled per call from whatever the
/// caller's components are, the way [`Combatant`](crate::forecast::Combatant)
/// is.
pub struct Gates<'a> {
    /// The statuses the user carries.
    pub statuses: &'a Statuses,
    /// Every slot and tag pair the user has equipped.
    pub worn: &'a [(SlotId, TagId)],
    /// The user's final stat values.
    pub stat: &'a dyn Fn(StatId) -> i32,
}

/// What the gate reads about what a user can spend.
pub struct Purse<'a> {
    /// Current value of a pool. Not the stat's maximum: the stat says how
    /// large the pool may be, this says what is in it.
    pub pool: &'a dyn Fn(StatId) -> i32,
    /// Charges left on whatever granted the ability, if it is limited.
    pub charges: Option<u16>,
    /// Health remaining. A cost may not reduce it below one.
    pub health: i32,
    /// How many items carrying a tag the user has.
    pub items: &'a dyn Fn(TagId) -> u16,
}

/// Every reason `def` may not be used right now, empty when it may.
///
/// `ready_at` is when the cooldown expires on the turn queue's clock and
/// `now` is that clock, so a cooldown is an absolute time rather than a
/// countdown: restoring the clock from a save restores every cooldown with
/// it, and nothing has to be ticked.
pub fn blocked(def: &AbilityDef, gates: &Gates<'_>, purse: &Purse<'_>, now: u32, ready_at: u32) -> Vec<Blocked> {
    let mut out = Vec::new();
    if ready_at > now {
        out.push(Blocked::Cooling { until: ready_at });
    }
    for r in &def.requires {
        let met = match *r {
            Requirement::Has(s) => gates.statuses.has(s),
            Requirement::Lacks(s) => !gates.statuses.has(s),
            Requirement::Wielding(tag) => gates.worn.iter().any(|(_, t)| *t == tag),
            Requirement::InSlot(slot, tag) => gates.worn.iter().any(|(s, t)| *s == slot && *t == tag),
            Requirement::Above(stat, n) => (gates.stat)(stat) > n,
        };
        if !met {
            out.push(Blocked::Needs(*r));
        }
    }
    for c in &def.costs {
        let afford = match *c {
            Cost::Pool { stat, amount } => (purse.pool)(stat) >= amount,
            // No charges at all means the ability was not granted by
            // something that counts them, so there is nothing to spend.
            Cost::Charge { amount } => purse.charges.is_some_and(|have| have >= amount),
            Cost::Health { amount } => purse.health > amount,
            Cost::Item { tag, count } => (purse.items)(tag) >= count,
        };
        if !afford {
            out.push(Blocked::Cannot(*c));
        }
    }
    out
}

/// Every reason this particular aim would be refused, as opposed to the
/// ability: [`blocked`] answers whether it may be used at all, this whether
/// it may be used *here*.
///
/// `cells` is the footprint the aim resolved to and `sees_aim` whether the
/// user can see the cell it is pointed at, `None` for a user with no sight
/// to ask, which is a mind trusted with its tactic's choice. A shape with
/// nowhere to go, such as a bolt pointed at the caster's own feet, is
/// refused rather than paid for, so the player keeps the turn and a
/// preview can say so before it is spent. An ability aimed at its user
/// needs no footprint and no sight.
pub fn aim_blocked(def: &AbilityDef, cells: &[Point], sees_aim: Option<bool>) -> Vec<Blocked> {
    let pointed = def.aim.needs_cursor();
    let nowhere = pointed && cells.is_empty();
    let unseen = pointed && def.sight && sees_aim == Some(false);
    if nowhere || unseen { vec![Blocked::NoTarget] } else { Vec::new() }
}

/// An ability an actor could use this turn, as a mind reads it.
///
/// Everything a tactic needs to pick a target and score it, and nothing
/// about what the ability does. The layer that owns the world narrows the
/// list to what is affordable, permitted and off cooldown before the brain
/// runs, so a tactic never has to know how to pay for anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usable {
    /// Which ability.
    pub ability: AbilityId,
    /// What it wants under it.
    pub aim: Aim,
    /// The shape it covers.
    pub mode: TargetMode,
}

/// Parses one effect's arguments, with the same RON extensions the rest of
/// a content file is written with.
///
/// A raw value re-parses from its own text, and extensions declared at the
/// top of the file do not reach it, so an argument list would otherwise
/// follow different rules from the entry containing it.
pub fn parse_args(text: &str) -> Result<Box<RawValue>, ContentError> {
    RawValue::from_boxed_ron(text.into()).map_err(|e| ContentError::Parse(e.to_string()))
}

/// Reads one effect's arguments as `T`, under the file's extensions.
///
/// The loader hands this to whoever registered the effect, so a game's
/// effect parses its own arguments into its own type and gets a RON error
/// with a position when they are wrong.
pub fn read_args<T: serde::de::DeserializeOwned>(args: &RawValue) -> Result<T, String> {
    Options::default().with_default_extension(Extensions::IMPLICIT_SOME).from_str(args.get_ron()).map_err(|e| e.to_string())
}

/// An ability as authored, before its names are resolved.
#[derive(Debug, Deserialize)]
struct Authored {
    name: String,
    #[serde(default)]
    aim: Aim,
    mode: TargetMode,
    #[serde(default = "yes")]
    sight: bool,
    #[serde(default)]
    requires: Vec<RequirementRon>,
    #[serde(default)]
    costs: Vec<CostRon>,
    #[serde(default = "one_step")]
    time: u32,
    #[serde(default)]
    cooldown: u32,
    #[serde(default)]
    effects: Vec<EffectRon>,
}

#[derive(Debug, Deserialize)]
enum CostRon {
    Pool(String, i32),
    Charge(u16),
    Health(i32),
    Item(String, u16),
}

#[derive(Debug, Deserialize)]
enum RequirementRon {
    Has(String),
    Lacks(String),
    Wielding(String),
    InSlot(String, String),
    Above(String, i32),
}

#[derive(Debug, Deserialize)]
struct EffectRon {
    kind: String,
    #[serde(default = "hundred")]
    chance: u8,
    #[serde(default = "nothing")]
    args: Box<RawValue>,
}

fn yes() -> bool {
    true
}

fn one_step() -> u32 {
    rl_core::turn::BASE_ACTION_COST
}

fn hundred() -> u8 {
    100
}

fn nothing() -> Box<RawValue> {
    parse_args("()").expect("the unit is valid ron")
}

impl Named for Authored {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Loads abilities from RON, resolving every name through `names`.
///
/// Reports every problem in the file at once rather than the first, so a
/// content file with three typos names three.
pub fn load(text: &str, names: &Names<'_>) -> Result<Registry<AbilityDef>, ContentError> {
    let authored: Registry<Authored> = Registry::from_ron_str(text)?;
    let mut errors = Vec::new();
    let mut defs = Vec::new();
    for (_, a) in authored.iter() {
        let mut costs = Vec::new();
        for c in &a.costs {
            match resolve_cost(c, names) {
                Ok(c) => costs.push(c),
                Err(e) => errors.push(format!("{}: {e}", a.name)),
            }
        }
        let mut requires = Vec::new();
        for r in &a.requires {
            match resolve_requirement(r, names) {
                Ok(r) => requires.push(r),
                Err(e) => errors.push(format!("{}: {e}", a.name)),
            }
        }
        let mut effects = Vec::new();
        for e in &a.effects {
            if e.chance > 100 {
                errors.push(format!("{}: effect {:?} has a chance of {}, above 100", a.name, e.kind, e.chance));
            }
            match parse_args(e.args.get_ron()) {
                Ok(args) => effects.push(EffectSpec { kind: e.kind.clone(), chance: e.chance, args }),
                Err(err) => errors.push(format!("{}: effect {:?} arguments: {err}", a.name, e.kind)),
            }
        }
        if a.time == 0 {
            errors.push(format!("{}: a use that costs no time is a use that can be repeated forever", a.name));
        }
        defs.push(AbilityDef { name: a.name.clone(), aim: a.aim, mode: a.mode, sight: a.sight, requires, costs, time: a.time, cooldown: a.cooldown, effects });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

fn resolve_cost(c: &CostRon, names: &Names<'_>) -> Result<Cost, String> {
    Ok(match c {
        CostRon::Pool(name, amount) => Cost::Pool { stat: names.stat(name)?, amount: *amount },
        CostRon::Charge(amount) => Cost::Charge { amount: *amount },
        CostRon::Health(amount) => Cost::Health { amount: *amount },
        CostRon::Item(name, count) => Cost::Item { tag: names.tag(name)?, count: *count },
    })
}

fn resolve_requirement(r: &RequirementRon, names: &Names<'_>) -> Result<Requirement, String> {
    Ok(match r {
        RequirementRon::Has(name) => Requirement::Has(names.status(name)?),
        RequirementRon::Lacks(name) => Requirement::Lacks(names.status(name)?),
        RequirementRon::Wielding(name) => Requirement::Wielding(names.tag(name)?),
        RequirementRon::InSlot(slot, tag) => Requirement::InSlot(names.slot(slot)?, names.tag(tag)?),
        RequirementRon::Above(name, n) => Requirement::Above(names.stat(name)?, *n),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affix::TagDef;
    use crate::damage::DamageKind;
    use crate::equip::SlotDef;
    use crate::stats::StatDef;
    use crate::status::StatusDef;

    /// The five registries an ability names, and the lookup over them.
    struct World {
        stats: Registry<StatDef>,
        statuses: Registry<StatusDef>,
        tags: Registry<TagDef>,
        slots: Registry<SlotDef>,
        kinds: Registry<DamageKind>,
    }

    impl World {
        fn new() -> Self {
            Self {
                stats: Registry::from_defs(vec![StatDef::new("mana", 20), StatDef::new("grit", 3)]).unwrap(),
                statuses: Registry::from_defs(vec![StatusDef::new("spotted"), StatusDef::new("braced")]).unwrap(),
                tags: Registry::from_defs(vec![TagDef::new("shield"), TagDef::new("powder")]).unwrap(),
                slots: Registry::from_defs(vec![SlotDef::new("off hand")]).unwrap(),
                kinds: Registry::from_defs(vec![DamageKind::new("fire")]).unwrap(),
            }
        }
    }

    impl World {
        fn names(&self) -> Names<'_> {
            Names::new().stats(&self.stats).statuses(&self.statuses).tags(&self.tags).slots(&self.slots).damage_kinds(&self.kinds)
        }
    }

    const RON: &str = r#"#![enable(implicit_some)]
[
    (
        name: "fireball",
        mode: Ball(range: 8, radius: 2),
        costs: [Pool("mana", 12)],
        cooldown: 300,
        effects: [(kind: "Harm", args: (kind: "fire", roll: "6d6"))],
    ),
    (
        name: "shield bash",
        mode: Adjacent,
        requires: [InSlot("off hand", "shield")],
        costs: [Pool("grit", 1)],
        effects: [(kind: "Shove", chance: 60, args: (cells: 1))],
    ),
    (
        name: "brace",
        aim: SelfOnly,
        mode: Own,
        effects: [(kind: "Afflict", args: (status: "braced", turns: 5))],
    ),
]"#;

    fn defs() -> (World, Registry<AbilityDef>) {
        let w = World::new();
        let r = load(RON, &w.names()).expect("the file loads");
        (w, r)
    }

    /// Nothing an author writes is an id: every name in the file is
    /// resolved through the game's own registries at load.
    #[test]
    fn a_file_of_names_becomes_a_registry_of_ids() {
        let (w, r) = defs();
        let fireball = r.get(r.expect("fireball"));
        assert_eq!(fireball.mode, TargetMode::Ball { range: 8, radius: 2 });
        assert_eq!(fireball.costs, vec![Cost::Pool { stat: w.stats.expect("mana"), amount: 12 }]);
        assert_eq!(fireball.aim, Aim::Foe, "the default");
        assert!(fireball.sight, "the default");
        assert_eq!(fireball.time, rl_core::turn::BASE_ACTION_COST, "the default");
        assert_eq!(fireball.effects[0].chance, 100, "the default");
        let bash = r.get(r.expect("shield bash"));
        assert_eq!(bash.requires, vec![Requirement::InSlot(w.slots.expect("off hand"), w.tags.expect("shield"))]);
        assert_eq!(bash.effects[0].chance, 60);
        assert_eq!(r.get(r.expect("brace")).aim, Aim::SelfOnly);
    }

    /// An effect's arguments are the game's to parse, under the same RON
    /// rules as the file they sit in.
    #[test]
    fn effect_arguments_are_read_by_whoever_registered_the_effect() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Harm {
            kind: String,
            roll: String,
            #[serde(default)]
            crit: Option<bool>,
        }
        let (_, r) = defs();
        let fireball = r.get(r.expect("fireball"));
        let harm: Harm = read_args(&fireball.effects[0].args).expect("the arguments read");
        assert_eq!(harm, Harm { kind: "fire".into(), roll: "6d6".into(), crit: None });
        let wrong: Result<Harm, String> = read_args(&r.get(r.expect("brace")).effects[0].args);
        assert!(wrong.is_err(), "an Afflict's arguments are not a Harm's");
    }

    /// A content file with several mistakes reports all of them, because
    /// fixing one at a time is how a morning goes.
    #[test]
    fn every_unknown_name_in_a_file_is_reported_at_once() {
        let w = World::new();
        let bad = r#"[
            (name: "a", mode: Own, costs: [Pool("wisdom", 1)]),
            (name: "b", mode: Own, requires: [Has("burning")], costs: [Item("gunpowder", 1)]),
            (name: "c", mode: Own, time: 0),
            (name: "d", mode: Own, effects: [(kind: "Harm", chance: 140, args: ())]),
        ]"#;
        let err = load(bad, &w.names()).expect_err("it does not load");
        let ContentError::Invalid(errs) = err else { panic!("expected a validation failure, got {err:?}") };
        assert_eq!(errs.len(), 5, "{errs:#?}");
        assert!(errs.iter().any(|e| e.contains("unknown stat \"wisdom\"")), "{errs:#?}");
        assert!(errs.iter().any(|e| e.contains("unknown status \"burning\"")), "{errs:#?}");
        assert!(errs.iter().any(|e| e.contains("unknown tag \"gunpowder\"")), "{errs:#?}");
        assert!(errs.iter().any(|e| e.contains("costs no time")), "{errs:#?}");
        assert!(errs.iter().any(|e| e.contains("above 100")), "{errs:#?}");
    }

    fn purse<'a>(pool: &'a dyn Fn(StatId) -> i32, items: &'a dyn Fn(TagId) -> u16) -> Purse<'a> {
        Purse { pool, charges: Some(2), health: 10, items }
    }

    /// The gate answers with every reason at once, so a panel can print
    /// all of what is wrong with a row rather than the first thing.
    #[test]
    fn the_gate_reports_every_reason_a_use_is_refused() {
        let (w, r) = defs();
        let bash = r.get(r.expect("shield bash"));
        let mana = w.stats.expect("mana");
        let grit = w.stats.expect("grit");
        let empty = Statuses::default();
        let none: &dyn Fn(StatId) -> i32 = &|_| 0;
        let no_items: &dyn Fn(TagId) -> u16 = &|_| 0;

        // Nothing worn, no grit, and used a moment ago.
        let gates = Gates { statuses: &empty, worn: &[], stat: &|_| 0 };
        let reasons = blocked(bash, &gates, &purse(none, no_items), 100, 400);
        assert_eq!(reasons.len(), 3, "{reasons:?}");
        assert!(reasons.contains(&Blocked::Cooling { until: 400 }));
        assert!(reasons.contains(&Blocked::Needs(Requirement::InSlot(w.slots.expect("off hand"), w.tags.expect("shield")))));
        assert!(reasons.contains(&Blocked::Cannot(Cost::Pool { stat: grit, amount: 1 })));

        // A shield on the arm, grit in the tank, and the cooldown expired.
        let worn = [(w.slots.expect("off hand"), w.tags.expect("shield"))];
        let gates = Gates { statuses: &empty, worn: &worn, stat: &|_| 0 };
        let full: &dyn Fn(StatId) -> i32 = &|s| if s == grit { 3 } else { 20 };
        assert!(blocked(bash, &gates, &purse(full, no_items), 400, 400).is_empty(), "ready at exactly now is ready");
        assert_ne!(mana, grit, "the two pools are told apart by id, not by name");
    }

    /// Health is the one cost that may not be paid in full: an ability
    /// that would kill its user is refused rather than lethal.
    #[test]
    fn a_cost_in_health_may_never_be_the_last_of_it() {
        let w = World::new();
        let r = load(r#"[(name: "blood pact", mode: Own, costs: [Health(10)])]"#, &w.names()).unwrap();
        let def = r.get(r.expect("blood pact"));
        let empty = Statuses::default();
        let gates = Gates { statuses: &empty, worn: &[], stat: &|_| 0 };
        let pool: &dyn Fn(StatId) -> i32 = &|_| 0;
        let items: &dyn Fn(TagId) -> u16 = &|_| 0;
        let at = |health| blocked(def, &gates, &Purse { pool, charges: None, health, items }, 0, 0);
        assert!(at(11).is_empty(), "eleven pays ten and lives");
        assert_eq!(at(10).len(), 1, "ten would leave nothing");
        assert_eq!(at(3).len(), 1);
    }

    /// An ability granted by nothing that counts charges cannot pay a
    /// charge, which is different from having run out.
    #[test]
    fn a_charge_cost_needs_something_that_counts_charges() {
        let w = World::new();
        let r = load(r#"[(name: "flare", mode: Bolt(range: 5), costs: [Charge(1)])]"#, &w.names()).unwrap();
        let def = r.get(r.expect("flare"));
        let empty = Statuses::default();
        let gates = Gates { statuses: &empty, worn: &[], stat: &|_| 0 };
        let pool: &dyn Fn(StatId) -> i32 = &|_| 0;
        let items: &dyn Fn(TagId) -> u16 = &|_| 0;
        let with = |charges| blocked(def, &gates, &Purse { pool, charges, health: 10, items }, 0, 0);
        assert!(with(Some(1)).is_empty());
        assert_eq!(with(Some(0)).len(), 1, "out of charges");
        assert_eq!(with(None).len(), 1, "nothing to spend charges from");
    }

    /// What a mind needs from an ability it cannot understand.
    #[test]
    fn aim_says_what_belongs_under_a_footprint() {
        assert!(Aim::Foe.wants(Some(Relation::Hostile)));
        assert!(!Aim::Foe.wants(Some(Relation::Allied)));
        assert!(!Aim::Foe.wants(None), "an empty cell is not a foe");
        assert!(Aim::Ally.wants(Some(Relation::Allied)));
        assert!(Aim::Anyone.wants(Some(Relation::Neutral)));
        assert!(Aim::Ground.wants(None), "a cell is a target in itself");
        assert!(!Aim::SelfOnly.wants(Some(Relation::Hostile)));
        assert!(!Aim::SelfOnly.needs_cursor());
        assert!(Aim::Ground.needs_cursor());
    }

    #[test]
    fn the_user_stands_in_its_own_footprint_as_its_own_ally() {
        assert!(Aim::Ally.hits(Some(Relation::Hostile), true), "an ally to itself whatever the matrix says");
        assert!(Aim::Ground.hits(None, true), "the ground does not step aside for the thrower");
        assert!(Aim::Anyone.hits(None, true));
        assert!(!Aim::Foe.hits(None, true), "a foe-aimed shape never catches its user");
        assert!(!Aim::Foe.hits(Some(Relation::Allied), false), "nor an ally");
        assert!(Aim::SelfOnly.hits(None, true));
        assert!(!Aim::SelfOnly.hits(Some(Relation::Allied), false), "and a self ability nobody else");
    }

    #[test]
    fn what_is_worth_aiming_at_is_always_something_the_footprint_hits() {
        assert!(Aim::Ally.worth_aiming_at(None, true, true), "a hurt user mends itself");
        assert!(!Aim::Ally.worth_aiming_at(Some(Relation::Allied), false, false), "a whole ally is not worth a heal");
        assert!(Aim::Ground.worth_aiming_at(Some(Relation::Hostile), false, false));
        assert!(!Aim::Ground.worth_aiming_at(Some(Relation::Allied), false, true), "an ally is hit only by mistake");
        assert!(!Aim::Anyone.worth_aiming_at(None, true, true), "and so is the thrower");
        for aim in [Aim::SelfOnly, Aim::Foe, Aim::Ally, Aim::Ground, Aim::Anyone] {
            for relation in [None, Some(Relation::Hostile), Some(Relation::Neutral), Some(Relation::Allied)] {
                for (is_user, hurt) in [(false, false), (false, true), (true, false), (true, true)] {
                    if aim.worth_aiming_at(relation, is_user, hurt) {
                        assert!(aim.hits(relation, is_user), "{aim:?} aims at {relation:?} (user: {is_user}) and would pass it by");
                    }
                }
            }
        }
    }

    #[test]
    fn an_aim_with_nowhere_to_go_or_out_of_sight_is_refused_once() {
        let world = World::new();
        let defs = load(
            r#"[
                (name: "bolt", mode: Bolt(range: 6), effects: []),
                (name: "lob", mode: Bolt(range: 6), sight: false, effects: []),
                (name: "steel", aim: SelfOnly, mode: Bolt(range: 6), effects: []),
            ]"#,
            &world.names(),
        )
        .unwrap();
        let (bolt, lob, steel) = (defs.get(defs.expect("bolt")), defs.get(defs.expect("lob")), defs.get(defs.expect("steel")));
        let some = [Point::new(3, 0)];
        assert_eq!(aim_blocked(bolt, &[], Some(true)), vec![Blocked::NoTarget], "nowhere to fly");
        assert_eq!(aim_blocked(bolt, &some, Some(false)), vec![Blocked::NoTarget], "out of sight");
        assert_eq!(aim_blocked(bolt, &[], Some(false)), vec![Blocked::NoTarget], "both, and still said once");
        assert!(aim_blocked(bolt, &some, None).is_empty(), "a user with no sight to ask is trusted");
        assert!(aim_blocked(lob, &some, Some(false)).is_empty(), "an ability that needs no sight");
        assert!(aim_blocked(steel, &[], Some(false)).is_empty(), "and one aimed at its user needs neither");
    }
}
