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
//!
//! How an attack looks belongs to what attacks, never to the kind of
//! damage it deals: two guns dealing one kind fly two colours, the way two
//! abilities do. A [`RangedAttack`] with a look flies it and, while
//! something watches the [`cue`](crate::cue)s, lands when it arrives, the
//! way a throw does; a [`MeleeAttack`] with one bursts on its target. One
//! with none is instant and unseen, which is every attack a game has not
//! given a look.
//!
//! An attack with an [`AttackShape`] strikes the foes it reaches beyond
//! its target as well, each with a roll of its own, as one swing. A blow
//! on a target that has not noticed its attacker lands harder, by the
//! percent [`CombatRules::unaware`] or the blow names, and only where
//! [`stealth`](crate::stealth) runs to say who has noticed whom.

use crate::app::EngineApp;
use atglyph_core::{DiceRoll, Point, geometry};
use atglyph_rules::ability::ParticleLook;
use atglyph_rules::damage::{DamageKindId, Defender, SubtractArmor};
use atglyph_rules::parts::{Melee, Ranged};
use atglyph_rules::{DamageStage, FactionId, Factions, Hit, Relation, Resistances, StatId, StatusId};
use bevy::prelude::*;

use crate::components::{Actor, Blocks, MyTurn, Player, Position};
use crate::cue::{AddAirborne, Airborne, Anchor, Cue, CueWait, Cued, LookOf};
use crate::items::{Equipped, Item};
use crate::places::{MapId, OnMap};
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

/// Takes no harm: every hit on it lands as nothing, after the stages. A
/// [`Heal`] still heals it, and so does a hit it absorbs.
///
/// A component rather than a stage, because a stage sees the hit and not
/// whom it lands on, and rather than a resistance of a hundred percent,
/// because resistances add up and past a hundred they absorb. For a dummy a
/// tutorial wants struck, an escort a scene keeps alive, or a debug mode.
/// The hit is still reported, with nothing dealt, so whatever narrates it
/// says it had no effect rather than saying nothing.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Invulnerable;

/// Which side an actor is on.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Faction(pub atglyph_rules::FactionId);

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
    /// [`BASE_ACTION_COST`](atglyph_core::turn::BASE_ACTION_COST), so a game
    /// that does not care about weapon speed writes nothing and every
    /// blow costs a turn.
    pub cost: Option<u32>,
    /// The colour a blow bursts in on whoever it struck. Only the colour:
    /// a burst's cells show the drawer's own burst glyphs, so the look's
    /// glyph is not drawn. `None` shows nothing, which is how a plain blow
    /// reads: the log says it, and the target's health bar moves.
    pub look: Option<ParticleLook>,
    /// Who else a blow strikes besides its target. `None` strikes the
    /// target alone, which is every blow a game has not given a shape.
    pub shape: Option<AttackShape>,
    /// What a blow on a target that has not noticed its wielder is worth,
    /// in percent of its roll, in place of [`CombatRules::unaware`]: a
    /// quick blade's five times over the house's three. `None` is the
    /// house's.
    pub unaware: Option<u32>,
}

impl MeleeAttack {
    /// A blow of `kind` rolling `dice`, costing an ordinary turn, showing
    /// nothing, striking its target alone and the unaware as the house
    /// says.
    ///
    /// A constructor rather than a literal, so a field that is only ever
    /// wanted by some games is added without touching every call site.
    pub const fn new(kind: DamageKindId, dice: DiceRoll) -> Self {
        Self { kind, dice, cost: None, look: None, shape: None, unaware: None }
    }

    /// The blow `part` describes, costing `cost` hundredths of a step, or
    /// an ordinary turn for `None`: the authored blow made a component.
    pub(crate) fn from_part(part: &Melee, cost: Option<u32>) -> Self {
        Self { cost, look: part.look, shape: part.shape, unaware: part.unaware, ..Self::new(part.kind.id(), part.roll) }
    }

    /// One blow costs `cost` hundredths of a step.
    pub const fn costing(mut self, cost: u32) -> Self {
        self.cost = Some(cost);
        self
    }

    /// A blow bursts on its target in `look`'s colour.
    pub const fn looking(mut self, look: ParticleLook) -> Self {
        self.look = Some(look);
        self
    }

    /// A blow strikes whoever else `shape` reaches as well as its target.
    pub const fn shaped(mut self, shape: AttackShape) -> Self {
        self.shape = Some(shape);
        self
    }

    /// A blow on a target that has not noticed its wielder lands at
    /// `percent` of its roll, whatever the house's percent: under a
    /// hundred too, for a blow that gains nothing by surprise.
    pub const fn unaware(mut self, percent: u32) -> Self {
        self.unaware = Some(percent);
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
    /// The furthest cell with no range penalty, for a hit model that reads
    /// one. `None` is a third of `range`, so only a weapon that cares says.
    pub effective: Option<i32>,
    /// What one shot with it costs, in hundredths of a step. `None` is
    /// [`BASE_ACTION_COST`](atglyph_core::turn::BASE_ACTION_COST). A shot that
    /// finds nothing in reach costs the ordinary turn rather than this,
    /// since what was spent was the aim.
    pub cost: Option<u32>,
    /// What flies from the shooter to the target. With one, a shot is seen
    /// to fly and, while something watches, lands when it arrives rather
    /// than when it is fired. `None` flies nothing and lands at once.
    pub look: Option<ParticleLook>,
    /// Who else a shot strikes besides its target. `None` strikes the
    /// target alone.
    ///
    /// No percent for the unaware, as [`MeleeAttack`] has: a shot is never
    /// multiplied for surprise, for the reason [`CombatRules::unaware`]
    /// gives.
    pub shape: Option<AttackShape>,
}

impl RangedAttack {
    /// A shot of `kind` rolling `dice` out to `range`, costing an ordinary
    /// turn, flying nothing and striking its target alone.
    pub const fn new(kind: DamageKindId, dice: DiceRoll, range: i32) -> Self {
        Self { kind, dice, range, effective: None, cost: None, look: None, shape: None }
    }

    /// The shot `part` describes, costing `cost` hundredths of a step, or
    /// an ordinary turn for `None`.
    pub(crate) fn from_part(part: &Ranged, cost: Option<u32>) -> Self {
        Self { cost, look: part.look, effective: part.effective, ..Self::new(part.kind.id(), part.roll, part.range) }
    }

    /// One shot costs `cost` hundredths of a step.
    pub const fn costing(mut self, cost: u32) -> Self {
        self.cost = Some(cost);
        self
    }

    /// A shot flies in `look`.
    pub const fn looking(mut self, look: ParticleLook) -> Self {
        self.look = Some(look);
        self
    }

    /// No range penalty out to `cells`.
    pub const fn effective_to(mut self, cells: i32) -> Self {
        self.effective = Some(cells);
        self
    }

    /// A shot strikes whoever else `shape` reaches as well as its target.
    pub const fn shaped(mut self, shape: AttackShape) -> Self {
        self.shape = Some(shape);
        self
    }

    /// The furthest cell with no range penalty: the one set, or a third of
    /// the range.
    pub const fn effective_range(&self) -> i32 {
        match self.effective {
            Some(cells) => cells,
            None => self.range / 3,
        }
    }
}

/// Who else an attack strikes besides the one it was made at: the
/// authored part, re-exported here beside the combat that plays it.
///
/// The whole is one swing: [`Struck`] is written once and the weapon fires
/// once, while the weapon's `hit` triggers land on each one struck.
pub use atglyph_rules::parts::AttackShape;

/// Extra rolls every hit carries, each its own [`DamageEvent`]: a flaming
/// blade's fire, a venomed edge's poison.
///
/// On an actor or on a worn item; every worn item's are added to the
/// wearer's own.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct BonusDamage(pub Vec<(DamageKindId, DiceRoll)>);

/// Who is hostile to whom, and which stats a blow reads.
///
/// A rule rather than content: the sides themselves are a registry in
/// [`Registries`], and this is how they regard each other. Built by naming
/// the pairs; a pair never named is worked out, a side allied with itself
/// and neutral to the rest, so the rules need no registry to be made:
///
/// ```
/// use atglyph_bevy::CombatRules;
/// use atglyph_rules::{Registry, faction::FactionDef};
///
/// let sides = Registry::from_defs(["player", "beasts", "navy", "pirates"].map(FactionDef::new).to_vec()).unwrap();
/// let side = |name| sides.expect(name);
/// let rules = CombatRules::new()
///     .hostile(side("player"), side("beasts"))
///     .hunts(side("navy"), side("pirates"));
/// assert!(rules.factions.is_hostile(side("beasts"), side("player")));
/// assert!(!rules.factions.is_hostile(side("pirates"), side("navy")), "the pirates would only rather not meet them");
/// ```
///
/// The stats are optional, and name the seam between the registered stats
/// and a blow: a status that hardens the skin or a ring that sharpens the
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
    /// The stat an attacker's accuracy is read from, if any. With none, the
    /// hit model is told there is no accuracy and uses its own default:
    /// 100 for `Percent`.
    pub accuracy: Option<StatId>,
    /// The stat a target's evasion is read from, if any.
    pub evasion: Option<StatId>,
    /// The stat whose value is added, in percent, to the hundred of every
    /// [`Heal`] an actor is given, if any.
    pub mending: Option<StatId>,
    /// What a blow on a target that has not noticed its attacker is worth,
    /// in percent of its roll, unless the blow names its own
    /// ([`MeleeAttack::unaware`]). `None` strikes the unaware as hard as
    /// anyone. See [`unaware`](Self::unaware) for who is unaware.
    pub unaware: Option<u32>,
    /// Whether the player's death ends the run, which it does unless the
    /// game says otherwise: one that revives, or plays on as a ghost, keeps
    /// the ending for itself.
    pub death_ends_run: bool,
    /// How far a thrower throws a thing with no range of its own, if
    /// throws reach at all: see [`reach`](Self::reach).
    pub reach: Option<ThrowReach>,
}

/// How far an actor throws a thing that names no range of its own: `base`,
/// plus its value of `stat` when one is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrowReach {
    /// The cells everyone reaches.
    pub base: i32,
    /// The stat whose value is added, if any.
    pub stat: Option<StatId>,
}

impl Default for CombatRules {
    fn default() -> Self {
        Self::new()
    }
}

impl CombatRules {
    /// Every side allied with itself and neutral to every other, until a
    /// pair is named, no stat read by a blow, no blow harder for surprise,
    /// and the player's death the end of the run.
    pub fn new() -> Self {
        Self {
            factions: Factions::new(),
            armor: None,
            attack: None,
            accuracy: None,
            evasion: None,
            mending: None,
            unaware: None,
            death_ends_run: true,
            reach: None,
        }
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

    /// A thing with no range of its own flies `base` cells from whoever
    /// throws it, plus the thrower's value of `stat`, when one is named;
    /// one with a range of its own keeps it. With no reach, a thing with no
    /// range of its own cannot be thrown.
    pub fn reach(mut self, base: i32, stat: Option<StatId>) -> Self {
        self.reach = Some(ThrowReach { base, stat });
        self
    }

    /// Reads `stat` as accuracy, for the hit model.
    pub fn accuracy_stat(mut self, stat: StatId) -> Self {
        self.accuracy = Some(stat);
        self
    }

    /// Reads `stat` as evasion, for the hit model.
    pub fn evasion_stat(mut self, stat: StatId) -> Self {
        self.evasion = Some(stat);
        self
    }

    /// Reads `stat` as how well an actor mends: its value is a percentage
    /// added to the hundred of every [`Heal`] it is given, never taking
    /// one below nothing. A thing that cannot be patched up carries a
    /// modifier of minus a hundred on it, and a status that halves every
    /// mend puts minus fifty there, with no kind of damage invented for
    /// either.
    pub fn mending_stat(mut self, stat: StatId) -> Self {
        self.mending = Some(stat);
        self
    }

    /// A blow on a target that has not noticed its attacker lands at
    /// `percent` of its roll, every roll it carries: 300 is three times.
    /// A blow that names its own percent lands at that instead.
    ///
    /// Who is unaware is stealth's to say, through
    /// [`Watchers::unaware`](crate::stealth::Watchers::unaware): while the
    /// [`StealthPlugin`](crate::stealth::StealthPlugin) runs, a target
    /// keeping an [`Aware`](crate::stealth::Aware) that holds nothing of an
    /// attacker it is at odds with and that carries
    /// [`Stealth`](crate::stealth::Stealth). Without the plugin nobody is,
    /// and this does nothing. What a `hit` trigger does is an effect, and
    /// lands as it would on anyone.
    ///
    /// Blows only. A shot on the unaware lands as it would on anyone,
    /// because what the surprise pays for is the walk up to the target
    /// unnoticed, and a shot from across the room never takes it.
    pub fn unaware(mut self, percent: u32) -> Self {
        self.unaware = Some(percent);
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

/// The mitigation pipeline, in order.
///
/// Defaults to [`SubtractArmor`] alone, which is what every game in this
/// repository and the template wrote by hand: armor that a blow does not
/// have to go through is not armor, and a game that inherited the empty
/// default got raw damage with no word about why. A game that wants
/// something else, resistances first, a crit that doubles, a block that
/// halves, inserts its own and this default is never built. A game that
/// really wants raw damage inserts `DamageStages(Vec::new())` and says so.
#[derive(Resource)]
pub struct DamageStages(pub Vec<Box<dyn DamageStage<Entity> + Send + Sync>>);

impl Default for DamageStages {
    fn default() -> Self {
        Self(vec![Box::new(SubtractArmor)])
    }
}

/// The stream combat rolls from.
///
/// Derived from the run's [`Seed`](crate::seed::Seed) by [`CombatPlugin`], so
/// a game never inserts it.
pub type CombatRng = crate::seed::RunRng<CombatDomain>;

/// The domain [`CombatRng`] draws under.
pub struct CombatDomain;

impl crate::seed::StreamDomain for CombatDomain {
    const DOMAIN: atglyph_core::SeedDomain = crate::seed::COMBAT;
}

/// Damage about to land on `target`.
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageEvent {
    /// Who takes it.
    pub target: Entity,
    /// The hit.
    pub hit: Hit<Entity>,
    /// How it got there, for whoever narrates it. Nothing in the pipeline
    /// reads it.
    pub reach: DamageRoute,
}

impl DamageEvent {
    /// Damage that did not travel as a weapon: an ability, a status, a
    /// fire. [`DamageRoute::Effect`], which is what most damage is.
    pub fn new(target: Entity, hit: Hit<Entity>) -> Self {
        Self { target, hit, reach: DamageRoute::Effect }
    }

    /// The same, arriving by `reach`.
    pub fn arriving(target: Entity, hit: Hit<Entity>, reach: DamageRoute) -> Self {
        Self { target, hit, reach }
    }
}

/// That an attack found something to strike with, and what: written once
/// per attack that lands a blow or fires a shot, before its damage, and
/// once however many a shaped one strikes, since it is one swing.
///
/// A game hangs what a weapon does to itself on this: heat, ammunition,
/// wear. It names the worn item rather than leaving the game to work out
/// which one the loadout would have chosen, which is the engine's decision
/// and would be copied, and drift, in every game that needed it.
///
/// Not the `struck` moment, which is
/// what the one struck answers, once for each blow that lands on it and
/// never for a miss.
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

/// How damage reached whoever took it, for whoever says what happened.
///
/// Closed, like the narrator's own `Phrase` and for the same reason: it
/// enumerates the ways the engine itself delivers damage, and a game that
/// invents another is writing its own words for it anyway. What reads it is
/// narration; nothing in the damage pipeline branches on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DamageRoute {
    /// A blow struck in reach.
    Melee,
    /// A shot down a line of fire.
    Shot,
    /// Something thrown.
    Thrown,
    /// An ability's effect: a bolt, a burst. Which one, so a narrator says
    /// what did it rather than call it a blow, since it was none.
    Ability(atglyph_rules::AbilityId),
    /// A thing's trigger, the thing being other than whoever set it off: a
    /// rod zapped, a flask broken where it was thrown, an edge's bite. The
    /// thing, for the same reason. A prop's own trap is its own user, and
    /// is [`Effect`](Self::Effect).
    Thing(Entity),
    /// Anything else that did not travel as a weapon: a status ticking, a
    /// fire, a trap. The default, because most damage is not a weapon and a
    /// subsystem that says nothing means this.
    #[default]
    Effect,
}

/// Damage that actually landed, after mitigation, for narration and
/// on-hit reactions. Zero means the hit was fully stopped; negative means
/// the target absorbed it, which a resistance past a hundred does.
///
/// Two figures, because a killing blow is bigger than what it took: a
/// narrator says the blow, "hits you for 30", and a tally of harm done
/// counts the health, the 5 the target had left.
///
/// Never a mend: health restored is a [`Heal`], reported as [`Healed`], so
/// a reader of this need not ask whether what it holds was a blow.
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageDealt {
    /// Who took it.
    pub target: Entity,
    /// The hit as it arrived.
    pub hit: Hit<Entity>,
    /// The blow as it landed, after mitigation and before the target's
    /// health is asked: a blow of 30 on a target with 5 left is 30.
    /// Negative for a hit absorbed, by what it restored, which at full
    /// health is nothing.
    pub dealt: i32,
    /// The health the target lost, never more than it had: a blow of 30 on
    /// a target with 5 left is 5. For a hit absorbed, the same negative as
    /// [`dealt`](Self::dealt), since what it restored is all that moved.
    pub lost: i32,
    /// How it got there, carried through from the [`DamageEvent`] so a
    /// narrator can tell a shot from a blow.
    pub reach: DamageRoute,
}

/// Asks for health to be restored: a medkit, a status that mends, a shrine.
///
/// Its own request and its own path to [`Health`], not a negative hit of
/// some kind. It was one once, and every game registered a damage kind
/// that was not one to carry it, set unarmored so plate did not stop a
/// medkit, while armor, resistances and every stage a game wrote had to
/// know to leave it alone. Nothing mitigates a mend; what scales it is
/// the target's own mending, [`CombatRules::mending_stat`].
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heal {
    /// Who is mended.
    pub target: Entity,
    /// How much is offered, before the target's mending. Below one offers
    /// nothing.
    pub amount: i32,
    /// Who did it, if anyone: the user of the thing, or whoever applied
    /// the status.
    pub by: Option<Entity>,
    /// The status whose tick this is, when it is one.
    pub status: Option<StatusId>,
}

impl Heal {
    /// `amount` for `target`, by nobody.
    pub fn new(target: Entity, amount: i32) -> Self {
        Self { target, amount, by: None, status: None }
    }

    /// The same, by `who`.
    pub fn by(mut self, who: Entity) -> Self {
        self.by = Some(who);
        self
    }

    /// The same, as one tick of `status`.
    pub fn of_status(mut self, status: StatusId) -> Self {
        self.status = Some(status);
        self
    }
}

/// Health that was restored, for narration and for a game's reactions.
///
/// Written for every [`Heal`] that found its target alive, with what it
/// restored rather than what it offered: a mend at full health restored
/// nothing and says so, so a log is not told every few turns that a
/// wearer who is whole has mended.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Healed {
    /// Who was mended.
    pub target: Entity,
    /// What health gained, nought at full health.
    pub restored: i32,
    /// Who did it, if anyone.
    pub by: Option<Entity>,
    /// The status whose tick it was, when it was one.
    pub status: Option<StatusId>,
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
type DefenderData = (&'static mut Health, &'static Position, Has<Player>, Has<Invulnerable>, Option<&'static OnMap>);

/// The combat components an actor or an item may carry, as a query asks
/// for them.
type Gear = (Option<&'static Armor>, Option<&'static MeleeAttack>, Option<&'static RangedAttack>, Option<&'static BonusDamage>);

/// A worn item's entity beside the same components, as [`Loadout::worn`]
/// yields them.
type WornRef<'a> = (Entity, Option<&'a Armor>, Option<&'a MeleeAttack>, Option<&'a RangedAttack>, Option<&'a BonusDamage>);

/// An attack a worn item cannot make right now, put by until whatever
/// benched it lifts the ban.
///
/// Taking the attack off the item entirely, rather than leaving it in
/// place behind some other flag, is what makes [`Loadout`] fall through to
/// the next worn item with no knowledge of why one went quiet: a
/// dual-wielded pair alternates because the benched one simply has
/// nothing to strike or shoot with. [`crate::ammo`] is the one reason the
/// engine itself knows (a shot with nothing left to spend); a game may
/// have reasons of its own, a weapon that runs hot among them, and reuses
/// this same holder rather than inventing a second one, since both answer
/// the same question a worn item's missing attack always asks: put by, or
/// simply never had one.
#[derive(Component, Debug, Clone, Copy)]
pub enum BenchedAttack {
    /// A blow, put by.
    Melee(MeleeAttack),
    /// A shot, put by.
    Ranged(RangedAttack),
}

/// What an actor fights with, summed at the moment it matters.
///
/// Three layers, added together: the actor's own [`Armor`], [`Resists`],
/// [`MeleeAttack`], [`RangedAttack`] and [`BonusDamage`]; the same components
/// on every item in its [`Equipped`] slots, in slot order; and the value of
/// the stats [`CombatRules`] names, read off its [`StatBlock`]. A worn
/// blow or shot replaces the actor's own, since a cutlass is swung in
/// place of a fist; armor, resistances and extra strikes add up.
///
/// The one answer to the question. The attack resolver strikes with it,
/// `apply_damage` defends with it, and the inspect forecast and the
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
    resists: Query<'w, 's, &'static Resists>,
    held: Query<'w, 's, &'static crate::status::StatusResists>,
    spent: Query<'w, 's, &'static crate::consumable::Consumable>,
    dry: Query<'w, 's, (), With<crate::ammo::Dry>>,
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

    /// Whether `item` has a charge to attack with: anything that counts no
    /// charges does, and an empty wand does not, so the shot it would fire
    /// is never offered to the player or to a mind. Its armor and what it
    /// resists still count: it is empty, not gone.
    fn charged(&self, item: Entity) -> bool {
        !self.spent.get(item).is_ok_and(|c| c.is_empty())
    }

    /// Whether `item` has gone dry: an ammunition-fed weapon
    /// `ammo::sync_ammo` most recently found
    /// nothing in its wielder's bag to feed it, the same "empty, not gone"
    /// standing as `charged`, above. [`melee_with`](Self::melee_with)
    /// and [`ranged_with`](Self::ranged_with) already skip it, because its
    /// attack came off with the ammunition and was put by as [`BenchedAttack`];
    /// this is the one place the engine answers "can this worn item act at
    /// all right now" by name, rather than leaving every reader to
    /// rediscover it from what is merely absent.
    pub fn dry(&self, item: Entity) -> bool {
        self.dry.get(item).is_ok()
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
        let worn = self.worn(who).filter_map(|(_, armor, ..)| armor).map(|a| a.0);
        atglyph_rules::arms::armor_total(own, worn, self.stat(who, |r| r.armor))
    }

    /// How much of a [`Heal`] reaches `who`, in percent: a hundred and the
    /// mending stat, never below nothing.
    pub fn mending(&self, who: Entity) -> i32 {
        atglyph_rules::arms::mending_pct(self.stat(who, |r| r.mending))
    }

    /// What `who` resists: its own [`Resists`], every worn item's and what
    /// the statuses on it lend, its
    /// `StatusResists`, added kind by kind,
    /// so a suit that resists a tenth of a bolt resists it for whoever
    /// wears it, and stops the moment it is taken off.
    pub fn resistances(&self, who: Entity) -> Resistances {
        let mut total = self.resists.get(who).map(|r| r.0.clone()).unwrap_or_default();
        if let Ok(held) = self.held.get(who) {
            total.plus(&held.0);
        }
        for (item, ..) in self.worn(who) {
            if let Ok(worn) = self.resists.get(item) {
                total.plus(&worn.0);
            }
        }
        total
    }

    /// The blow `who` strikes, as [`Loadout::melee`], with the worn item it
    /// comes from, or `None` when it is `who`'s own.
    pub fn melee_with(&self, who: Entity) -> Option<(Option<Entity>, MeleeAttack)> {
        let worn = self.worn(who).map(|(item, _, melee, ..)| (item, melee.copied(), self.charged(item)));
        let own = self.own.get(who).ok().and_then(|(_, melee, ..)| melee.copied());
        let (from, base) = atglyph_rules::arms::first_charged(worn, own)?;
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
        let worn = self.worn(who).map(|(item, _, _, ranged, _)| (item, ranged.copied(), self.charged(item)));
        let own = self.own.get(who).ok().and_then(|(_, _, ranged, _)| ranged.copied());
        let (from, base) = atglyph_rules::arms::first_charged(worn, own)?;
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
    pub(crate) fn strikes(&self, who: Entity) -> Vec<(DamageKindId, DiceRoll)> {
        let mut all: Vec<(DamageKindId, DiceRoll)> = self.own.get(who).ok().and_then(|(_, _, _, s)| s).map(|s| s.0.clone()).unwrap_or_default();
        for (_, _, _, _, strikes) in self.worn(who) {
            all.extend(strikes.map(|s| s.0.iter().copied()).into_iter().flatten());
        }
        all
    }

    /// Every roll one blow by `who` lands, the main one first: what the
    /// forecast in [`atglyph_rules::forecast`] takes. Empty for something that
    /// cannot strike.
    pub fn blows(&self, who: Entity) -> Vec<(DamageKindId, DiceRoll)> {
        let mut all: Vec<(DamageKindId, DiceRoll)> = self.melee(who).map(|m| (m.kind, m.dice)).into_iter().collect();
        all.extend(self.strikes(who));
        all
    }

    /// Every roll one shot by `who` lands, the shot itself first: what a
    /// forecast takes for a pair that is not adjacent. Empty for something
    /// with nothing to shoot.
    ///
    /// The ranged twin of [`blows`](Self::blows), and the extra strikes are
    /// in both because `resolve_attacks` adds them to a shot exactly as it
    /// adds them to a blow.
    pub fn shots(&self, who: Entity) -> Vec<(DamageKindId, DiceRoll)> {
        let Some(shot) = self.ranged(who) else { return Vec::new() };
        let mut all = vec![(shot.kind, shot.dice)];
        all.extend(self.strikes(who));
        all
    }

    /// What `who` can do to something at any distance, for
    /// [`atglyph_rules::forecast`]: both sets of rolls, both costs and the
    /// shot's reach, with the choice left to the distance.
    ///
    /// Takes the two `Vec`s the caller already built, because `Arms`
    /// borrows them and a forecast is recomputed every frame.
    pub fn arms<'a>(&self, who: Entity, blows: &'a [(DamageKindId, DiceRoll)], shots: &'a [(DamageKindId, DiceRoll)]) -> atglyph_rules::forecast::Arms<'a> {
        let shot = self.ranged(who);
        atglyph_rules::forecast::Arms {
            melee: blows,
            melee_cost: self.melee(who).and_then(|m| m.cost),
            ranged: shots,
            ranged_cost: shot.and_then(|r| r.cost),
            range: shot.map_or(0, |r| r.range),
        }
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
    fired: MessageWriter<'w, crate::effects::Fired>,
    /// What a worn thing does when its attack strikes, for the shot to carry.
    carried: Query<'w, 's, &'static crate::effects::Triggers>,
    /// The odds of the attack landing, from the game's hit model.
    marks: crate::accuracy::Marksmanship<'w, 's>,
    /// Where a rolled attack that missed is reported.
    missed: MessageWriter<'w, crate::accuracy::Missed>,
    /// Whoever a shape could reach: alive, with health to lose.
    alive: Query<'w, 's, (), (With<Health>, Without<Dead>)>,
    /// How each stands to the attacker, by the rule an ability's footprint
    /// asks, so a shape and a footprint never disagree about who is a foe.
    sides: crate::ability::Bystanders<'w, 's>,
    /// What a blow on the unaware is worth, unless the blow says.
    rules: Option<Res<'w, CombatRules>>,
    /// Who has noticed whom, for a blow on the unaware.
    watchers: crate::stealth::Watchers<'w, 's>,
}

/// What an attack is seen as, and the shots in the air while it is.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Shown<'w> {
    cues: MessageWriter<'w, Cued>,
    cue_wait: ResMut<'w, CueWait>,
    airborne: ResMut<'w, Airborne<ShotLanding>>,
}

/// A shot fired and not yet arrived: whom it is at, and every hit it
/// carries, the main one first, rolled when it was fired. A shot with a
/// shape is one of these for each it strikes.
///
/// Rolled at the trigger rather than on arrival, so the combat stream
/// draws in the same order whether or not anything watches, and a run
/// played in a window rolls what the same run played headless rolls.
#[derive(Debug, Clone)]
pub struct ShotLanding {
    target: Entity,
    /// Who made the attack.
    attacker: Entity,
    /// The worn item it was made with, whose `hit` triggers land on the
    /// target; `None` for a fist or a claw.
    with: Option<Entity>,
    /// That item's triggers, taken as it fired: a thing its last charge
    /// spent is gone before a watched shot lands, and what its hits carry
    /// lands all the same.
    carried: Option<crate::effects::Triggers>,
    /// The attacker's own triggers when it answers `hit`, taken as it
    /// struck for the same reason: a shooter killed while its shot flies is
    /// gone before it lands, and its venom lands all the same.
    own: Option<crate::effects::Triggers>,
    /// Who lands those should the shooter be gone: a [`Remnant`](crate::effects::Remnant)
    /// kept `Disabled`, so no query sees it, with what
    /// [`RemnantKeeps`](crate::effects::RemnantKeeps) takes copied from the
    /// shooter by [`keep_shooters`] in the pass it fired, its `Name` first.
    /// Without it the bite of a shooter killed while its shot flew landed
    /// from nobody, named nothing in the log and credited to an entity no
    /// longer there, where a `died` remnant is named as what it stands in
    /// for. Taken only for a shot in the air, the one that lands a pass
    /// later, and despawned unused whenever the shooter is still there or
    /// the shot lands nothing.
    stand_in: Option<Entity>,
    /// Whether it flew or was struck in reach, kept so the landing a pass
    /// later narrates as what it was.
    reach: DamageRoute,
    hits: Vec<Hit<Entity>>,
    /// Whether it was rolled and missed: it still flies, and lands nothing.
    missed: bool,
}

impl crate::cue::Lands for ShotLanding {}

/// The attack one blow or shot is made with, as [`Loadout`] chose it.
struct ChosenAttack {
    from: Option<Entity>,
    ranged: bool,
    kind: DamageKindId,
    dice: DiceRoll,
    cost: Option<u32>,
    look: Option<ParticleLook>,
    shape: Option<AttackShape>,
    /// What a blow on the unaware is worth, in percent: the blow's own or
    /// the house's, and never anything for a shot.
    unaware: Option<u32>,
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
/// An attack with a [`ParticleLook`] is seen: a shot flies from the shooter to
/// the target, and a blow bursts on the target. With something watching,
/// a shot's hits wait in [`Airborne`] until the flight has been
/// seen, the way a thrown knife does; a blow has no flight to wait for
/// and hurts at once. [`Struck`] is written as the attack is made either
/// way, since what a weapon does to itself happens at the trigger, not
/// at the target.
///
/// An attack with an [`AttackShape`] strikes its target and every foe the
/// shape reaches, the target rolled for first and the rest in the shape's
/// order, so an attack with none draws from the combat stream exactly what
/// it always did. It is still one swing: one [`Struck`], one `fire`
/// moment, one flight; but it lands on each, so the weapon's `hit`, the
/// attacker's own `hit` and the `struck` of the one it lands on are
/// reported once for every foe it strikes. A blow on a target that has not
/// noticed its attacker lands at the percent [`CombatRules::unaware`] or
/// the blow names, decided as it is rolled, before the blow wakes it.
///
/// One turn, one strike: an attack by an actor that already acted this
/// pass finds the turn spent, whatever spent it.
pub(crate) fn resolve_attacks(
    mut intents: MessageReader<Intent<Attack>>,
    mut damage: MessageWriter<DamageEvent>,
    mut resolution: Resolution,
    mut rng: ResMut<CombatRng>,
    mut arena: Arena,
    shown: Shown,
) {
    let Arena { map, occupancy, attackers, targets, loadout, struck, fired, carried, marks, missed, alive, sides, rules, watchers } = &mut arena;
    let Shown { mut cues, mut cue_wait, mut airborne } = shown;
    let house = rules.as_deref().and_then(|r| r.unaware);
    for intent in intents.read() {
        let (actor, target) = (intent.actor, intent.action.0);
        let Ok(pos) = attackers.get(actor) else { continue };
        if !resolution.claim(actor) {
            continue;
        }
        let Ok(target_pos) = targets.get(target) else {
            resolution.done(actor, atglyph_core::turn::BASE_ACTION_COST);
            continue;
        };
        // The gun is kept beside the attack it became, so the odds can read
        // its reach.
        let adjacent = geometry::is_adjacent(pos.0, target_pos.0);
        let gun = if adjacent { None } else { loadout.ranged_with(actor).filter(|(_, r)| line_of_fire(map, occupancy, pos.0, target_pos.0, r.range)) };
        let melee = if adjacent { loadout.melee_with(actor) } else { None };
        // `gun` is already filtered by line of fire, so it stands for both facts.
        let chosen = match atglyph_rules::arms::choose_reach(adjacent, gun.is_some(), melee.is_some(), gun.is_some()) {
            Some(atglyph_rules::arms::Reach::Melee) => melee.map(|(from, m)| ChosenAttack {
                from,
                ranged: false,
                kind: m.kind,
                dice: m.dice,
                cost: m.cost,
                look: m.look,
                shape: m.shape,
                unaware: m.unaware.or(house),
            }),
            Some(atglyph_rules::arms::Reach::Ranged) => {
                gun.map(|(from, r)| ChosenAttack { from, ranged: true, kind: r.kind, dice: r.dice, cost: r.cost, look: r.look, shape: r.shape, unaware: None })
            }
            None => None,
        };
        let Some(ChosenAttack { from, ranged, kind, dice, cost, look, shape, unaware }) = chosen else {
            resolution.done(actor, atglyph_core::turn::BASE_ACTION_COST);
            continue;
        };
        resolution.done(actor, cost.unwrap_or(atglyph_core::turn::BASE_ACTION_COST));
        struck.write(Struck { attacker: actor, target, with: from, ranged });
        // What the weapon does when it is used, whatever it strikes: a
        // wand's charge, a trigger on firing.
        if let Some(item) = from {
            fired.write(crate::effects::Fired { on: item, moment: crate::effects::TriggerKinds::FIRE, by: Some(actor), at: pos.0 });
        }
        // Rolled after the weapon has fired and before the damage, so a
        // miss still spends what firing spends and draws no damage dice.
        // Under `Certain` there are no odds and nothing is drawn.
        let attempt = match &gun {
            Some((_, gun)) => crate::accuracy::Attempt::Shot(gun),
            None => crate::accuracy::Attempt::Blow,
        };
        let mut struck_at = vec![(target, target_pos.0)];
        if let Some(shape) = shape {
            let foe = |other: Entity| other != actor && alive.contains(other) && sides.relation(actor, other) == Some(Relation::Hostile);
            struck_at.extend(reached(shape, pos.0, (target, target_pos.0), gun.map(|(_, r)| r.range), map, occupancy, foe));
        }
        let strikes = loadout.strikes(actor);
        let answers = |t: &&crate::effects::Triggers| t.on(crate::effects::TriggerKinds::HIT).next().is_some();
        let own = carried.get(actor).ok().filter(answers).cloned();
        let carried = from.and_then(|item| carried.get(item).ok()).filter(answers).cloned();
        let reach = if ranged { DamageRoute::Shot } else { DamageRoute::Melee };
        let mut landings = Vec::with_capacity(struck_at.len());
        for &(foe, at) in &struck_at {
            let wide = marks.odds(actor, foe, attempt, 0).is_some_and(|odds| !odds.roll(&mut **rng));
            // Read as the blow is rolled. The blow is what wakes the one it
            // lands on, and only once the turn's consequences are read, in
            // `TurnSet::React`, so every foe a sweep catches unaware is
            // struck as one.
            let worth = unaware.filter(|_| watchers.unaware(foe, actor)).unwrap_or(100);
            let mut hits = Vec::new();
            if !wide {
                // Floored where it is rolled: a blow that rolls below zero
                // has missed, and the pipeline would read a negative one as
                // a heal.
                hits.push(Hit::by(actor, kind, dice.roll_at_least(&mut **rng, 0)));
                hits.extend(strikes.iter().map(|(kind, dice)| Hit::by(actor, *kind, dice.roll_at_least(&mut **rng, 0))));
                for hit in &mut hits {
                    hit.amount = atglyph_rules::arms::scale_unaware(hit.amount, worth);
                }
            }
            let shot =
                ShotLanding { target: foe, attacker: actor, with: from, carried: carried.clone(), own: own.clone(), stand_in: None, reach, hits, missed: wide };
            landings.push((shot, at));
        }
        match (look, ranged) {
            (Some(look), true) => {
                // One flight, to the farthest a pierce runs on to; a sweep's
                // others stand round the shooter, and it flies to the target.
                let (to, to_at) = match shape {
                    Some(AttackShape::Pierce(_)) => struck_at.last().copied().unwrap_or((target, target_pos.0)),
                    _ => (target, target_pos.0),
                };
                cues.write(Cued { actor, cue: Cue::Flight { from: Anchor::on(actor, pos.0), to: Anchor::on(to, to_at), look: LookOf::Given(look) } });
                for (shot, at) in landings {
                    if let Some(shot) = airborne.launched(&mut cue_wait, shot) {
                        land(shot, at, &mut damage, fired, missed);
                    }
                }
                continue;
            }
            (Some(look), false) => {
                let on = struck_at.iter().map(|&(who, at)| Anchor::on(who, at)).collect();
                cues.write(Cued { actor, cue: Cue::Burst { on, look: LookOf::Given(look), from: None } });
            }
            (None, _) => {}
        }
        for (shot, at) in landings {
            land(shot, at, &mut damage, fired, missed);
        }
    }
}

/// Everyone besides the target that `shape` reaches from `from`, with
/// where each stands, in the order they are rolled for: clockwise from
/// north round the attacker for a sweep, outward along the line for a
/// pierce. Whoever `foe` refuses is passed over, and does not stop a
/// pierce either.
fn reached(
    shape: AttackShape,
    from: Point,
    (target, to): (Entity, Point),
    range: Option<i32>,
    map: &WorldMap,
    occupancy: &Occupancy,
    foe: impl Fn(Entity) -> bool,
) -> Vec<(Entity, Point)> {
    let cells: Vec<Point> = match shape {
        AttackShape::Sweep => atglyph_core::Direction::ALL.iter().map(|d| from + d.offset()).collect(),
        AttackShape::Pierce(n) => beyond(from, to, n, range, map),
    };
    cells.into_iter().flat_map(|cell| occupancy.at(cell).iter().map(move |who| (*who, cell))).filter(|&(who, _)| who != target && foe(who)).collect()
}

/// The cells up to `n` past `to` on the line from `from` through it, short
/// of the first that stops a shot and, for a shot, of its `range`.
///
/// The line is the one a beam through `to` draws, so it passes through the
/// target exactly and runs on as a shot aimed there would. Never past the
/// edge of what is loaded, which also keeps an enormous `n` from
/// overflowing the line's far end.
fn beyond(from: Point, to: Point, n: u32, range: Option<i32>, map: &WorldMap) -> Vec<Point> {
    let bounds = map.window_tiles();
    let near = geometry::chebyshev(from, to);
    let far = near + i32::try_from(n).unwrap_or(i32::MAX).min(bounds.width.max(bounds.height));
    let far = range.map_or(far, |range| far.min(range));
    if far <= near {
        return Vec::new();
    }
    let walls = |p: Point| map.blocks_projectiles(p);
    let line = atglyph_grid::footprint(atglyph_grid::TargetMode::Beam { range: far }, from, to, bounds, walls, |_| false);
    line.path.into_iter().skip_while(|p| *p != to).skip(1).take_while(|p| !walls(*p)).collect()
}

/// What a shot landing writes: its hits, the moment it struck, and a
/// remnant for a thing gone before it arrived.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Arriving<'w, 's> {
    damage: MessageWriter<'w, DamageEvent>,
    fired: MessageWriter<'w, crate::effects::Fired>,
    commands: Commands<'w, 's>,
    present: Query<'w, 's, (), With<crate::effects::Triggers>>,
    missed: MessageWriter<'w, crate::accuracy::Missed>,
}

/// Takes a stand-in for the shooter of every shot that went up this pass
/// answering `hit` and not yet missed, while the shooter is still there to
/// copy: what [`RemnantKeeps`](crate::effects::RemnantKeeps) lists, onto a
/// [`Remnant`](crate::effects::Remnant) kept `Disabled` so no query sees
/// it until `land_shots` needs it.
///
/// Its own system, after `resolve_attacks` in the same chain, rather than
/// a step inside it: the resolver is one of the busiest systems a game
/// meets, and giving it `Commands` widened what a game had to order
/// against it. Nothing in the air is taken for in a game without
/// [`EffectsPlugin`](crate::effects::EffectsPlugin), where no shooter
/// answers a moment.
pub(crate) fn keep_shooters(
    mut commands: Commands,
    mut airborne: ResMut<Airborne<ShotLanding>>,
    keeps: Option<Res<crate::effects::RemnantKeeps>>,
    there: Query<()>,
) {
    let Some(keeps) = keeps else { return };
    for shot in airborne.in_air_mut() {
        if shot.stand_in.is_some() || shot.own.is_none() || shot.missed || !there.contains(shot.attacker) {
            continue;
        }
        let remnant = crate::effects::Remnant { moment: crate::effects::TriggerKinds::HIT, by: Some(shot.attacker), at: Point::ZERO };
        let stand_in = commands.spawn((bevy::ecs::entity_disabling::Disabled, remnant)).id();
        keeps.copy(&mut commands, shot.attacker, stand_in);
        shot.stand_in = Some(stand_in);
    }
}

/// Lands every shot in the air, on the first pass after its flight has
/// been seen, on a target still standing: one killed or taken out of the
/// world while the shot flew is missed, not hurt twice or looked for.
/// Landing counts as progress, so the loop goes on to deal the next turn.
pub(crate) fn land_shots(
    mut airborne: ResMut<Airborne<ShotLanding>>,
    mut cue_wait: ResMut<CueWait>,
    mut turns: ResMut<Turns>,
    alive: Query<&Position, (With<Health>, Without<Dead>)>,
    mut arriving: Arriving,
) {
    for mut shot in airborne.landing(&mut cue_wait, &mut turns) {
        let stand_in = shot.stand_in.take();
        let Ok(at) = alive.get(shot.target) else {
            if let Some(unused) = stand_in {
                arriving.commands.entity(unused).despawn();
            }
            continue;
        };
        // A thing spent to nothing by firing is gone by now: what its hits
        // carry lands from a remnant in its place, as the shooter's doing.
        if let Some(item) = shot.with
            && !arriving.present.contains(item)
            && !shot.missed
        {
            if let Some(triggers) = shot.carried.take() {
                let remnant = crate::effects::Remnant { moment: crate::effects::TriggerKinds::HIT, by: Some(shot.attacker), at: at.0 };
                arriving.commands.spawn((triggers, remnant));
            }
            shot.with = None;
        }
        // And a shooter dead and gone: what its own hit carries lands from
        // its stand-in, named as it was and landing as itself, as a `died`
        // remnant does, so the log names the shooter and a foe the bite
        // kills is the shooter's kill. From a bare remnant, its doing still,
        // when nothing was kept of it.
        let gone = !arriving.present.contains(shot.attacker) && !shot.missed;
        match (gone.then(|| shot.own.take()).flatten(), stand_in) {
            (Some(triggers), Some(stand_in)) => {
                let remnant = crate::effects::Remnant { moment: crate::effects::TriggerKinds::HIT, by: Some(shot.attacker), at: at.0 };
                arriving.commands.entity(stand_in).insert((triggers, remnant, crate::effects::LandsAsItself)).remove::<bevy::ecs::entity_disabling::Disabled>();
            }
            (Some(triggers), None) => {
                let remnant = crate::effects::Remnant { moment: crate::effects::TriggerKinds::HIT, by: Some(shot.attacker), at: at.0 };
                arriving.commands.spawn((triggers, remnant));
            }
            (None, Some(unused)) => {
                arriving.commands.entity(unused).despawn();
            }
            (None, None) => {}
        }
        land(shot, at.0, &mut arriving.damage, &mut arriving.fired, &mut arriving.missed);
    }
}

/// Every hit an attack carries, down the damage pipeline, and its moments
/// at the cell the target stands on as it lands, so a trigger that bursts
/// bursts where the shot arrived: `hit` on the worn item it came from,
/// then `hit` on the attacker when it answers one, then `struck` on the
/// target. In that order, which is the order they land in, so a blade's
/// edge lands before the wielder's own venom and both before whatever the
/// one struck answers with. A miss lands none of that and is reported as
/// [`Missed`](crate::accuracy::Missed) instead, at the moment a hit would
/// have landed.
///
/// `struck` is reported as the blow lands, before its damage is applied
/// later in the pass, so a blow that kills still sets it off, and `died`
/// follows a pass later: an aura that burns whoever struck it burns the
/// one who killed it too. It is an attack's, a blow in reach or a shot;
/// a thrown thing, an ability, a trap or a fire strikes nobody.
fn land(
    shot: ShotLanding,
    at: Point,
    damage: &mut MessageWriter<DamageEvent>,
    fired: &mut MessageWriter<crate::effects::Fired>,
    missed: &mut MessageWriter<crate::accuracy::Missed>,
) {
    let (target, reach) = (shot.target, shot.reach);
    if shot.missed {
        missed.write(crate::accuracy::Missed { attacker: shot.attacker, target, with: shot.with, reach });
        return;
    }
    damage.write_batch(shot.hits.into_iter().map(|hit| DamageEvent::arriving(target, hit, reach)));
    let by = Some(shot.attacker);
    if let Some(item) = shot.with {
        fired.write(crate::effects::Fired { on: item, moment: crate::effects::TriggerKinds::HIT, by, at });
    }
    if shot.own.is_some() {
        fired.write(crate::effects::Fired { on: shot.attacker, moment: crate::effects::TriggerKinds::HIT, by, at });
    }
    fired.write(crate::effects::Fired { on: target, moment: crate::effects::TriggerKinds::STRUCK, by, at });
}

/// Where `mode`, sent from `from` at `aim`, goes on this map: a shot, a
/// throw, an ability and a thing's aimed use all ask this, so they agree
/// about what is in the way and where they come to rest.
///
/// Anyone standing in the way stops a flight, and so does anything that
/// stops projectiles; only the second is a wall, which
/// [`footprint`](atglyph_grid::footprint) brings nothing to rest inside. One
/// gathering rather than one per resolver because the copies drifted
/// apart: a shot came to rest inside the wall a throw stopped short of.
pub(crate) fn projectile(map: &WorldMap, occupancy: &Occupancy, mode: atglyph_grid::TargetMode, from: Point, aim: Point) -> atglyph_grid::Footprint {
    let walls = |p: Point| map.blocks_projectiles(p);
    atglyph_grid::footprint(mode, from, aim, map.window_tiles(), |p| walls(p) || occupancy.is_occupied(p), walls)
}

/// Where a shot from `from` at `to` flies within `range`: a bolt's flight,
/// the one a throw and a bolt ability take too.
///
/// It reaches `to`, or stops short of it at the first thing that stops
/// projectiles or anyone standing in between, and comes to rest in front
/// of a wall rather than inside it. [`line_of_fire`] is this reaching
/// `to`, and a targeting preview draws this same call, so what the player
/// is shown and what the resolver decides cannot disagree.
pub fn shot(map: &WorldMap, occupancy: &Occupancy, from: Point, to: Point, range: i32) -> atglyph_grid::Footprint {
    projectile(map, occupancy, atglyph_grid::TargetMode::Bolt { range }, from, to)
}

/// Whether a shot from `from` reaches `to` within `range`: nothing that
/// stops projectiles and nobody standing in between.
pub fn line_of_fire(map: &WorldMap, occupancy: &Occupancy, from: Point, to: Point, range: i32) -> bool {
    shot(map, occupancy, from, to, range).reached == Some(to)
}

/// Everything that decides how much of a hit lands: the stages, the kinds
/// they read, what the target wears, and whether the place it stands in
/// is peaceful.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Mitigation<'w, 's> {
    registries: Res<'w, Registries>,
    stages: Res<'w, DamageStages>,
    map: Res<'w, WorldMap>,
    loadout: Loadout<'w, 's>,
}

impl Mitigation<'_, '_> {
    /// What of `hit` lands on `target`, standing on `map`: negative for a
    /// hit it absorbs, nothing that harms an `invulnerable` one, and
    /// nothing at all in a peaceful place.
    fn lands(&self, hit: &Hit<Entity>, target: Entity, invulnerable: bool, map: MapId) -> i32 {
        let defender = Defender { armor: self.loadout.armor(target), blocked: false };
        let resists = self.loadout.resistances(target);
        let stages: Vec<&dyn DamageStage<Entity>> = self.stages.0.iter().map(|s| s.as_ref() as &dyn DamageStage<Entity>).collect();
        let amount = atglyph_rules::resolve(hit, &defender, &resists, &self.registries.damage_kinds, &stages);
        atglyph_rules::damage::mitigate(amount, invulnerable, self.map.is_peaceful(map))
    }
}

/// Runs the damage stages and applies what is left to health.
///
/// The armor and the resistances a blow meets are the target's
/// [`Loadout`]: its own, what it wears, and the armor stat. An
/// [`Invulnerable`] target keeps what it absorbs and nothing that harms it.
///
/// A target standing in a [`peaceful`](crate::places::PlaceBuild::peaceful)
/// place takes nothing either way: every hit on it lands as nothing, a
/// blow, a shot, a tick of fire or poison, and an absorbed one restores
/// nothing, and the hit is still reported with nothing dealt. Here, after
/// every stage, rather than as a stage of a game's own, since a stage is
/// handed a hit and a defender and never the place the blow lands in, and
/// so that no stage before it has to be taught to let the place alone.
/// A heal still heals there, and a status the same blow afflicts still
/// lands, since neither is a hit.
pub(crate) fn apply_damage(
    mut events: MessageReader<DamageEvent>,
    mut dealt: MessageWriter<DamageDealt>,
    mut deaths: MessageWriter<DeathEvent>,
    mitigation: Mitigation,
    mut targets: Query<DefenderData>,
) {
    for ev in events.read() {
        let Ok((mut health, pos, is_player, invulnerable, on)) = targets.get_mut(ev.target) else { continue };
        if health.current <= 0 {
            continue;
        }
        let amount = mitigation.lands(&ev.hit, ev.target, invulnerable, on.map_or(MapId::ZERO, |m| m.0));
        let hit = atglyph_rules::damage::apply_hit(atglyph_rules::Vitals { current: health.current, max: health.max }, amount);
        health.current = hit.current;
        dealt.write(DamageDealt { target: ev.target, hit: ev.hit, dealt: hit.dealt, lost: hit.lost, reach: ev.reach });
        if hit.died {
            deaths.write(DeathEvent { entity: ev.target, at: pos.0, credit: ev.hit.credit, was_player: is_player });
        }
    }
}

/// Restores what every [`Heal`] asks, scaled by the target's own mending
/// and stopped at its most, and reports it.
///
/// After [`apply_damage`] in the same pass, so a mend never stands between
/// an actor and the blow that killed it: the dead are not mended.
pub(crate) fn apply_healing(mut heals: MessageReader<Heal>, mut healed: MessageWriter<Healed>, loadout: Loadout, mut targets: Query<&mut Health>) {
    for heal in heals.read() {
        let Ok(mut health) = targets.get_mut(heal.target) else { continue };
        if health.current <= 0 {
            continue;
        }
        let before = health.current;
        health.current = atglyph_rules::damage::heal(atglyph_rules::Vitals { current: before, max: health.max }, heal.amount, loadout.mending(heal.target));
        healed.write(Healed { target: heal.target, restored: health.current - before, by: heal.by, status: heal.status });
    }
}

/// Dead, and waiting for the end of the frame to be despawned.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Dead;

/// Takes dead non-players out of the world, the queue and the index, and
/// marks them [`Dead`] for `bury_the_dead`.
pub(crate) fn process_deaths(
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
pub(crate) fn end_run_on_player_death(mut deaths: MessageReader<DeathEvent>, rules: Res<CombatRules>, mut over: MessageWriter<crate::state::RunOver>) {
    for death in deaths.read() {
        if death.was_player && rules.death_ends_run {
            over.write(crate::state::RunOver::died(death.credit));
        }
    }
}

/// Despawns whoever died this frame, once every system has seen them go.
pub(crate) fn bury_the_dead(mut commands: Commands, dead: Query<Entity, With<Dead>>) {
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
pub(crate) fn perceive_reach(mut thinking: ResMut<crate::minds::Thinking>, loadout: Loadout) {
    let Some(thinker) = thinking.actor() else { return };
    let reach = loadout.ranged(thinker).map(|r| r.range);
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.reach = reach;
    }
}

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        use crate::app::EngineApp;
        use crate::plugin::{CleanupSet, ResolveSet, Turn};
        app.add_message::<DamageEvent>()
            .add_message::<DamageDealt>()
            .add_message::<Heal>()
            .add_message::<Healed>()
            .add_message::<DeathEvent>()
            // What an attack made with a worn thing reports, for its `fire`
            // and `hit` triggers. A game may fight and have no effects, and
            // then the queue simply stays empty.
            .add_message::<crate::effects::Fired>()
            .add_message::<Struck>()
            .add_message::<crate::accuracy::Missed>()
            .init_resource::<DamageStages>()
            .init_resource::<crate::accuracy::HitRules>()
            .add_airborne::<ShotLanding>()
            .add_action::<Attack>()
            .needs::<CombatRules>("CombatPlugin", "`CombatRules::new()`, who is hostile to whom")
            .needs::<crate::registries::Registries>("CombatPlugin", "`Registries`, with the damage kinds a blow can deal")
            .add_stream::<CombatRng>("CombatPlugin")
            .add_systems(Turn, perceive_reach.in_set(crate::plugin::PerceiveSet::Annotate))
            .add_systems(
                Turn,
                (land_shots.in_set(crate::plugin::LandSet::Shot), resolve_attacks.in_set(ResolveSet::Act), keep_shooters.in_set(ResolveSet::Act)).chain(),
            )
            .add_systems(Turn, (apply_damage, apply_healing).chain().in_set(ResolveSet::Damage))
            .add_systems(Turn, end_run_on_player_death.in_set(crate::plugin::TurnSet::React))
            .add_systems(Turn, process_deaths.in_set(CleanupSet::Remove))
            // In `Last`, after everything that reads the frame's deaths has
            // run, which is the promise that the dead linger until the frame
            // ends, kept without naming any of those systems.
            .add_systems(Last, bury_the_dead.in_set(crate::plugin::EndOfFrame::Bury));
    }

    fn finish(&self, app: &mut App) {
        app.depends_on::<crate::plugin::CorePlugin>("CombatPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Viewshed;
    use crate::state::EngineState;
    use crate::testing::headless_app;
    use atglyph_grid::TileId;
    use atglyph_rules::Registry;
    use atglyph_rules::faction::FactionDef;

    fn arena() -> (App, Point, DamageKindId) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin));
        let start = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        (app, start, sides.kind)
    }

    #[test]
    fn a_shot_needs_a_clear_line_of_fire_and_carries_extra_strikes() {
        let (mut app, start, blunt) = arena();
        let (us, them) = (atglyph_rules::FactionId::from_raw(0), atglyph_rules::FactionId::from_raw(1));
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
                BonusDamage(vec![(blunt, DiceRoll::flat(2))]),
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

    /// A model that always misses, so a test reads a miss without a seed.
    struct Never;
    impl atglyph_rules::accuracy::HitModel for Never {
        fn odds(&self, _: &atglyph_rules::accuracy::Shot) -> Option<atglyph_rules::Odds> {
            Some(atglyph_rules::Odds::new(0, 100, Vec::new()))
        }
    }

    /// Counts what one attack wrote.
    #[derive(Resource, Default, Debug)]
    struct Wrote {
        struck: usize,
        damage: usize,
        missed: Vec<crate::accuracy::Missed>,
    }

    fn count(mut wrote: ResMut<Wrote>, mut s: MessageReader<Struck>, mut d: MessageReader<DamageEvent>, mut m: MessageReader<crate::accuracy::Missed>) {
        wrote.struck += s.read().count();
        wrote.damage += d.read().count();
        wrote.missed.extend(m.read().copied());
    }

    #[test]
    fn a_miss_fires_the_weapon_and_deals_nothing() {
        for adjacent in [true, false] {
            let (mut app, start, blunt) = arena();
            app.insert_resource(crate::accuracy::HitRules(Box::new(Never))).init_resource::<Wrote>().add_systems(PostUpdate, count);
            let player = app
                .world_mut()
                .spawn((
                    (Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(atglyph_rules::FactionId::from_raw(0))),
                    (MeleeAttack::new(blunt, DiceRoll::flat(3)), RangedAttack::new(blunt, DiceRoll::flat(3), 6)),
                ))
                .id();
            let gap = if adjacent { 1 } else { 4 };
            let target =
                app.world_mut().spawn((Actor, Blocks, Position(start.offset(gap, 0)), Health::full(20), Faction(atglyph_rules::FactionId::from_raw(1)))).id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            let before = app.world().resource::<Turns>().now();
            app.world_mut().write_message(Intent::new(player, Attack(target)));
            app.update();
            app.update();
            assert_eq!(app.world().get::<Health>(target).unwrap().current, 20, "nothing landed");
            let wrote = app.world().resource::<Wrote>();
            assert_eq!((wrote.struck, wrote.damage), (1, 0), "the weapon fired and hurt nobody");
            let reach = if adjacent { DamageRoute::Melee } else { DamageRoute::Shot };
            assert_eq!(wrote.missed, vec![crate::accuracy::Missed { attacker: player, target, with: None, reach }]);
            assert!(app.world().resource::<Turns>().now() > before, "a miss is a spent turn");
        }
    }

    /// Which moments a worn weapon's attack set off.
    #[derive(Resource, Default)]
    struct Moments {
        fire: bool,
        hit: bool,
    }

    fn moments(mut seen: ResMut<Moments>, mut fired: MessageReader<crate::effects::Fired>) {
        for f in fired.read() {
            seen.fire |= f.moment == crate::effects::TriggerKinds::FIRE;
            seen.hit |= f.moment == crate::effects::TriggerKinds::HIT;
        }
    }

    #[test]
    fn a_missed_shot_with_a_worn_weapon_fires_its_fire_moment_and_not_its_hit_moment() {
        let (mut app, player, _gun, target) = gunman(3, None, None);
        app.insert_resource(crate::accuracy::HitRules(Box::new(Never))).init_resource::<Moments>().add_systems(PostUpdate, moments);
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        app.update();
        let seen = app.world().resource::<Moments>();
        assert!(seen.fire, "the gun fired");
        assert!(!seen.hit, "and hit nothing");
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 20);
    }

    /// A call site names only what it cares about: `new` leaves every
    /// optional field unset, and each builder sets the one it names.
    #[test]
    fn a_new_attack_leaves_every_optional_field_unset_and_each_builder_sets_its_own() {
        let kind = DamageKindId::from_raw(0);
        let look = atglyph_rules::ability::ParticleLook { glyph: '*', color: atglyph_grid::Rgb::new(255, 80, 40) };
        let blow = MeleeAttack::new(kind, DiceRoll::flat(2));
        assert_eq!((blow.kind, blow.dice, blow.cost, blow.look, blow.shape, blow.unaware), (kind, DiceRoll::flat(2), None, None, None, None));
        let blow = blow.costing(70).looking(look).shaped(AttackShape::Sweep).unaware(500);
        assert_eq!((blow.cost, blow.look, blow.shape, blow.unaware), (Some(70), Some(look), Some(AttackShape::Sweep), Some(500)));

        let shot = RangedAttack::new(kind, DiceRoll::flat(3), 6);
        assert_eq!((shot.kind, shot.dice, shot.range, shot.cost, shot.look, shot.shape), (kind, DiceRoll::flat(3), 6, None, None, None));
        let shot = shot.costing(140).looking(look).shaped(AttackShape::Pierce(2));
        assert_eq!((shot.cost, shot.look, shot.shape), (Some(140), Some(look), Some(AttackShape::Pierce(2))));
    }

    #[derive(Resource, Default)]
    struct Seen {
        cues: Vec<Cued>,
        struck: Vec<Struck>,
        dealt: Vec<DamageDealt>,
    }

    /// Records every cue, every [`Struck`] and every [`DamageDealt`], for
    /// the reason [`hear`] does.
    fn see(mut cues: MessageReader<Cued>, mut struck: MessageReader<Struck>, mut dealt: MessageReader<DamageDealt>, mut seen: ResMut<Seen>) {
        seen.cues.extend(cues.read().cloned());
        seen.struck.extend(struck.read().copied());
        seen.dealt.extend(dealt.read().copied());
    }

    const LOOK: ParticleLook = ParticleLook { glyph: '*', color: atglyph_grid::Rgb::new(255, 80, 40) };

    /// A player armed with what `arm` makes of the one damage kind, facing
    /// a mindless target with twenty health `dx` cells east, with play
    /// begun and everything [`see`] records recorded. Watched when
    /// `watched`, the way a game that draws its cues is.
    fn duel<B: Bundle>(dx: i32, watched: bool, arm: impl FnOnce(DamageKindId) -> B) -> (App, Point, Entity, Entity) {
        let (mut app, start, kind) = arena();
        app.init_resource::<Seen>().add_systems(PostUpdate, see);
        let player = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(FactionId::from_raw(0)), arm(kind)))
            .id();
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(dx, 0)), Health::full(20), Faction(FactionId::from_raw(1)))).id();
        if watched {
            app.world_mut().resource_mut::<CueWait>().watch();
        }
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (app, start, player, target)
    }

    fn hp(app: &App, who: Entity) -> i32 {
        app.world().get::<Health>(who).unwrap().current
    }

    #[test]
    fn a_shot_with_a_look_cues_one_flight_from_the_shooter_to_its_target_in_that_look() {
        let (mut app, start, player, target) = duel(4, false, |kind| RangedAttack::new(kind, DiceRoll::flat(3), 6).looking(LOOK));
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        let flight = Cue::Flight { from: Anchor::on(player, start), to: Anchor::on(target, start.offset(4, 0)), look: LookOf::Given(LOOK) };
        assert_eq!(app.world().resource::<Seen>().cues, vec![Cued { actor: player, cue: flight }]);
    }

    /// A shot that names no look flies nothing and lands as it always did,
    /// even with something watching: nothing was cued, so nothing is waited on.
    #[test]
    fn a_shot_with_no_look_cues_nothing_and_lands_at_once_even_watched() {
        let (mut app, _, player, target) = duel(4, true, |kind| RangedAttack::new(kind, DiceRoll::flat(3), 6));
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert!(app.world().resource::<Seen>().cues.is_empty());
        assert_eq!(hp(&app, target), 17);
        assert!(!app.world().resource::<CueWait>().in_flight());
    }

    /// With something watching, the shot and its extra strikes wait in
    /// the air until the flight has been seen, and the trigger pull is
    /// reported when it happens, so heat and ammunition answer the shot
    /// rather than its arrival. Without a watcher it all lands at once.
    #[test]
    fn a_watched_shot_hurts_when_its_flight_has_been_seen_and_an_unwatched_one_at_once() {
        let arm = |kind| (RangedAttack::new(kind, DiceRoll::flat(3), 6).looking(LOOK), BonusDamage(vec![(kind, DiceRoll::flat(2))]));
        let (mut app, _, player, target) = duel(4, true, arm);
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(hp(&app, target), 20, "in the air");
        assert_eq!(app.world().resource::<Seen>().struck.len(), 1, "but fired");
        assert!(app.world().resource::<CueWait>().in_flight());
        assert_eq!(app.world().resource::<Turns>().now(), 0, "no turn is dealt while it flies");
        app.update();
        assert_eq!(hp(&app, target), 20, "nor does it land while the turns are held");

        app.world_mut().resource_mut::<CueWait>().release();
        app.update();
        assert_eq!(hp(&app, target), 20 - 3 - 2, "the shot and its strike, landed");
        assert!(!app.world().resource::<CueWait>().in_flight());
        assert_eq!(app.world().resource::<Seen>().struck.len(), 1, "and fired once");
        assert!(app.world().get::<MyTurn>(player).is_some(), "and the turn comes round");

        let (mut app, _, player, target) = duel(4, false, arm);
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(hp(&app, target), 15, "unwatched, it lands as it is fired");
    }

    /// A target that someone else kills while the shot flies, or that is
    /// gone altogether, is not hit by it: the shot lands on nothing and the
    /// turns go on.
    #[test]
    fn a_target_killed_or_gone_mid_flight_takes_nothing_from_the_shot() {
        let (mut app, start, player, target) = duel(4, true, |kind| RangedAttack::new(kind, DiceRoll::flat(3), 6).looking(LOOK));
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        let other = app.world_mut().spawn(Position(start.offset(0, 3))).id();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        app.world_mut().write_message(DamageEvent::new(target, Hit::by(other, kind, 99)));
        app.world_mut().resource_mut::<CueWait>().release();
        app.update();
        let dealt: Vec<(Option<Entity>, i32)> = app.world().resource::<Seen>().dealt.iter().map(|d| (d.hit.credit, d.dealt)).collect();
        assert_eq!(dealt, vec![(Some(other), 99)], "the other's blow killed it, and the shot found it dead");
        assert!(!app.world().resource::<CueWait>().in_flight());

        let (mut app, _, player, target) = duel(4, true, |kind| RangedAttack::new(kind, DiceRoll::flat(3), 6).looking(LOOK));
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        app.world_mut().resource_mut::<Turns>().remove(target);
        app.world_mut().resource_mut::<Occupancy>().remove(start.offset(4, 0), target);
        app.world_mut().despawn(target);
        app.world_mut().resource_mut::<CueWait>().release();
        app.update();
        assert!(app.world().resource::<Seen>().dealt.is_empty(), "gone, it takes nothing");
        assert!(!app.world().resource::<CueWait>().in_flight());
        assert!(app.world().get::<MyTurn>(player).is_some(), "and the turn comes round");
    }

    /// A shooter that dies and is gone before its shot arrives still hits:
    /// the shot left the barrel, and the hit it carries names a shooter
    /// nothing may assume is still in the world.
    #[test]
    fn a_shot_still_lands_when_its_shooter_is_dead_and_gone_before_it_arrives() {
        use atglyph_rules::ai::{Brain, tactics::ShootAtRange};
        let (mut app, start, kind) = arena();
        app.add_plugins(crate::minds::MindsPlugin);
        app.init_resource::<Seen>().add_systems(PostUpdate, see);
        let me = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(FactionId::from_raw(0)))).id();
        let shooter = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(4, 0)),
                Health::full(10),
                Faction(FactionId::from_raw(1)),
                crate::minds::Perception(8),
                RangedAttack::new(kind, DiceRoll::flat(3), 6).looking(LOOK),
                crate::minds::Mind(std::sync::Arc::new(Brain::new().then(ShootAtRange::default()))),
            ))
            .id();
        app.world_mut().resource_mut::<CueWait>().watch();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(me, crate::turn::Wait));
        app.update();
        assert!(app.world().resource::<CueWait>().in_flight(), "it fired");
        assert_eq!(hp(&app, me), 30);

        app.world_mut().resource_mut::<Turns>().remove(shooter);
        app.world_mut().resource_mut::<Occupancy>().remove(start.offset(4, 0), shooter);
        app.world_mut().despawn(shooter);
        app.world_mut().resource_mut::<CueWait>().release();
        app.update();
        assert_eq!(hp(&app, me), 27, "the shot arrived all the same");
        let credit: Vec<Option<Entity>> = app.world().resource::<Seen>().dealt.iter().map(|d| d.hit.credit).collect();
        assert_eq!(credit, vec![Some(shooter)], "credited to a shooter who is no longer anywhere");
        assert!(!app.world().resource::<CueWait>().in_flight());
        assert!(app.world().get::<MyTurn>(me).is_some(), "and the turn comes round");
    }

    /// A blow that names a look bursts on whoever it struck, and hurts at
    /// once, watched or not: nothing flies, so there is nothing to wait for.
    /// One that names none shows nothing.
    #[test]
    fn a_blow_with_a_look_bursts_on_its_target_and_one_without_shows_nothing() {
        let (mut app, start, player, target) = duel(1, true, |kind| MeleeAttack::new(kind, DiceRoll::flat(4)).looking(LOOK));
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        let burst = Cue::Burst { on: vec![Anchor::on(target, start.offset(1, 0))], look: LookOf::Given(LOOK), from: None };
        assert_eq!(app.world().resource::<Seen>().cues, vec![Cued { actor: player, cue: burst }]);
        assert_eq!(hp(&app, target), 16, "and it hurt at once");

        let (mut app, _, player, target) = duel(1, true, |kind| MeleeAttack::new(kind, DiceRoll::flat(4)));
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert!(app.world().resource::<Seen>().cues.is_empty());
        assert_eq!(hp(&app, target), 16);
    }

    /// The dead are not mended: a heal in the pass a blow killed in comes
    /// after the blow and restores nothing.
    #[test]
    fn a_heal_in_the_pass_that_killed_its_target_restores_nothing() {
        let (mut app, _, player, target) = duel(0, false, |kind| MeleeAttack::new(kind, DiceRoll::flat(1)));
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        app.world_mut().write_message(Heal::new(target, 50));
        app.world_mut().write_message(DamageEvent::new(target, Hit::by(player, kind, 999)));
        app.update();
        assert!(app.world_mut().resource_mut::<Messages<Healed>>().drain().next().is_none());
        assert!(app.world().get::<Health>(target).is_none_or(|h| h.current <= 0));
    }

    /// What `who` fights with, as a panel or a resolver would ask.
    fn loadout(app: &mut App, who: Entity) -> (i32, Option<DiceRoll>, Vec<(DamageKindId, DiceRoll)>) {
        let mut state: bevy::ecs::system::SystemState<Loadout> = bevy::ecs::system::SystemState::new(app.world_mut());
        let loadout = state.get(app.world()).expect("every input is optional");
        (loadout.armor(who), loadout.melee(who).map(|m| m.dice), loadout.strikes(who))
    }

    /// `Loadout::dry` answers by the item's own `Dry` marker, nothing else,
    /// so it reads the same whether or not the item is also worn.
    #[test]
    fn loadout_answers_dry_from_an_items_own_marker() {
        let mut app = headless_app();
        let marked = app.world_mut().spawn(crate::ammo::Dry).id();
        let plain = app.world_mut().spawn_empty().id();
        let mut state: bevy::ecs::system::SystemState<Loadout> = bevy::ecs::system::SystemState::new(app.world_mut());
        let loadout = state.get(app.world()).expect("every input is optional");
        assert!(loadout.dry(marked));
        assert!(!loadout.dry(plain));
    }

    /// Armor is three layers summed at the blow: the defender's own hide,
    /// the coat it wears, and the stat a status hardened. Nothing was
    /// copied onto the defender to make that so, and the coat's share
    /// leaves with the coat.
    #[test]
    fn a_blow_meets_the_defenders_own_armor_its_gear_and_the_armor_stat_together() {
        use atglyph_rules::stats::Op;
        use atglyph_rules::{EquipShape, Equipment, SlotId, StatDef, StatusDef};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin));
        let start = crate::testing::ground(&mut app);
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
        app.world_mut().write_message(crate::status::ApplyStatus { target, status: hardened, turns: 9, by: None, held_by: None });
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
        use atglyph_rules::{EquipShape, Equipment, SlotId, StatDef};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin));
        let start = crate::testing::ground(&mut app);
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
                BonusDamage(vec![(sides.kind, DiceRoll::flat(1))]),
                crate::items::Wearable(EquipShape::in_slot(hand)),
            ))
            .id();
        let pistol = app
            .world_mut()
            .spawn((
                crate::items::Item,
                RangedAttack::new(sides.kind, DiceRoll::flat(4), 5),
                BonusDamage(vec![(sides.kind, DiceRoll::flat(2))]),
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
                BonusDamage(vec![(sides.kind, DiceRoll::flat(3))]),
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
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin));
        let start = crate::testing::ground(&mut app);
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
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin));
        let start = crate::testing::ground(&mut app);
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
        assert_eq!(plain, atglyph_core::turn::BASE_ACTION_COST, "no cost stated is the ordinary cost");
    }

    /// Fires one ranged attack with `cost` and `range` at a target `offset`
    /// cells east, and answers what the turn cost.
    ///
    /// Mirrors [`melee_turn_cost`]: both actors start the clock at zero, so
    /// once the shot (or the miss) and the target's stranded wait have both
    /// been charged, the clock reads exactly what the shooter's turn cost.
    fn shot_turn_cost(cost: Option<u32>, range: i32, offset: i32) -> u32 {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin));
        let start = crate::testing::ground(&mut app);
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
        assert_eq!(missed, atglyph_core::turn::BASE_ACTION_COST, "out of range is a spent turn, not a free one");
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

    /// A gunman holding one gun in the main hand, and a target three cells
    /// east with twenty health and no armor. The gun deals `roll` and
    /// carries `consumable` and the triggers `triggers` names, if any.
    fn gunman(roll: i32, consumable: Option<crate::consumable::Consumable>, triggers: Option<&str>) -> (App, Entity, Entity, Entity) {
        use atglyph_rules::{EquipShape, Equipment, SlotId};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin, crate::effects::EffectsPlugin));
        crate::app::EngineApp::add_engine_effects(&mut app);
        let start = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let hand = SlotId::from_raw(0);
        let gun = app
            .world_mut()
            .spawn((crate::items::Item, RangedAttack::new(sides.kind, DiceRoll::flat(roll), 6), crate::items::Wearable(EquipShape::in_slot(hand))))
            .id();
        if let Some(c) = consumable {
            app.world_mut().entity_mut(gun).insert(c);
        }
        if let Some(text) = triggers {
            let specs: Vec<atglyph_rules::TriggerSpec> = atglyph_rules::Names::new().load_list(text).expect("the triggers parse");
            let world = app.world();
            let registries = world.resource::<Registries>();
            let built = crate::effects::Triggers::build(
                &specs,
                &[],
                world.resource::<crate::effects::TriggerKinds>(),
                world.resource::<crate::effects::EffectKinds>(),
                &registries.names(),
            )
            .expect("the triggers build");
            app.world_mut().entity_mut(gun).insert(built);
        }
        let mut equipment = Equipment::with_slot_count(1);
        equipment.equip(gun, &EquipShape::in_slot(hand)).unwrap();
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
                crate::items::Inventory { items: vec![gun] },
                Equipped(equipment),
            ))
            .id();
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(3, 0)), Health::full(20), Faction(sides.theirs))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (app, player, gun, target)
    }

    fn shoot(app: &mut App, player: Entity, target: Entity) {
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        app.update();
    }

    #[test]
    fn a_wand_spends_a_charge_for_every_shot_fired() {
        let wand = crate::consumable::Consumable::new(3, crate::consumable::WhenEmpty::Kept);
        let (mut app, player, gun, target) = gunman(1, Some(wand), None);
        shoot(&mut app, player, target);
        assert_eq!(app.world().get::<crate::consumable::Consumable>(gun).map(|c| c.left), Some(2), "one shot, one charge");
    }

    /// A hit trigger lands on whoever the shot struck, where they stand:
    /// a gun that deals nothing itself still hurts through what its hits
    /// carry.
    #[test]
    fn a_hit_trigger_lands_on_the_struck_actor_where_it_stands() {
        let (mut app, player, _, target) = gunman(0, None, Some(r#"[(on: "hit", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))])]"#));
        shoot(&mut app, player, target);
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 17);
        assert_eq!(app.world().get::<Health>(player).unwrap().current, 30, "and not on the shooter");
    }

    /// The last charge of a thing spent to nothing still does what its
    /// hits carry when the shot is watched: firing spends it, and it is
    /// gone before the shot lands a pass later, so what it would have done
    /// travels with the shot rather than being looked for on it.
    #[test]
    fn the_last_shot_of_a_spent_gun_still_lands_its_hit_trigger_when_watched() {
        let once = crate::consumable::Consumable::new(1, crate::consumable::WhenEmpty::Destroyed);
        let (mut app, player, gun, target) = gunman(0, Some(once), Some(r#"[(on: "hit", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))])]"#));
        let kind = app.world().get::<RangedAttack>(gun).unwrap().kind;
        app.world_mut().entity_mut(gun).insert(RangedAttack::new(kind, DiceRoll::flat(0), 6).looking(LOOK));
        app.world_mut().resource_mut::<CueWait>().watch();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert!(app.world().resource::<CueWait>().in_flight(), "the shot is in the air");
        assert!(app.world().get_entity(gun).is_err(), "and the gun, its one charge spent, is gone");
        app.world_mut().resource_mut::<CueWait>().release();
        app.update();
        app.update();
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 17, "what its hit carried landed all the same");
    }

    /// An empty wand has no shot, so neither the player nor a mind that
    /// asks the loadout ever chooses one.
    #[test]
    fn an_empty_kept_wand_offers_no_shot_so_nothing_ever_chooses_one() {
        let empty = crate::consumable::Consumable { left: 0, ..crate::consumable::Consumable::new(3, crate::consumable::WhenEmpty::Kept) };
        let (mut app, player, _, target) = gunman(5, Some(empty), None);
        let mut state: bevy::ecs::system::SystemState<Loadout> = bevy::ecs::system::SystemState::new(app.world_mut());
        let loadout = state.get(app.world()).expect("every input is optional");
        assert!(loadout.ranged(player).is_none(), "nothing to fire");
        shoot(&mut app, player, target);
        assert_eq!(app.world().get::<Health>(target).unwrap().current, 20, "and nothing was fired");
    }

    /// Sends one melee attack from a player adjacent to a target, worn or
    /// bare-handed, and returns the single [`Struck`] written for it.
    ///
    /// Mirrors [`melee_turn_cost`]'s setup, with `ItemsPlugin` added so a
    /// worn weapon can be equipped.
    fn struck_by_melee(worn: bool) -> Struck {
        use atglyph_rules::{EquipShape, Equipment, SlotId};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin));
        app.init_resource::<Heard>().add_systems(PostUpdate, hear);
        let start = crate::testing::ground(&mut app);
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
        use atglyph_rules::{EquipShape, Equipment, SlotId};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin));
        app.init_resource::<Heard>().add_systems(PostUpdate, hear);
        let start = crate::testing::ground(&mut app);
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

    /// An invulnerable actor is hit and takes nothing, and the hit is still
    /// reported, with nothing dealt, so a log says it had no effect; a heal
    /// still heals, since what it stops is harm.
    #[test]
    fn an_invulnerable_actor_takes_no_harm_and_is_still_healed() {
        let (mut app, start, blunt) = arena();
        let who = app.world_mut().spawn((Actor, Blocks, Position(start), Health { current: 10, max: 30 }, Invulnerable)).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.world_mut().write_message(DamageEvent::new(who, atglyph_rules::Hit::from_source(None, blunt, 999)));
        app.update();
        assert_eq!(app.world().get::<Health>(who).unwrap().current, 10, "nothing taken off, and not killed");
        let dealt: Vec<i32> = app.world_mut().resource_mut::<Messages<DamageDealt>>().drain().map(|d| d.dealt).collect();
        assert_eq!(dealt, vec![0], "the hit is reported, with nothing dealt");
        app.world_mut().write_message(Heal::new(who, 5));
        app.update();
        assert_eq!(app.world().get::<Health>(who).unwrap().current, 15, "a heal lands in full");
        app.world_mut().entity_mut(who).remove::<Invulnerable>();
        app.world_mut().write_message(DamageEvent::new(who, atglyph_rules::Hit::from_source(None, blunt, 4)));
        app.update();
        assert_eq!(app.world().get::<Health>(who).unwrap().current, 11, "and without it, a hit hurts again");
    }

    /// `apply_healing` hands the heal the target's own mending stat, not a
    /// constant: at minus fifty on it a heal of eight restores four.
    #[test]
    fn a_heal_restores_what_the_targets_own_mending_stat_allows() {
        use atglyph_rules::{Modifier, Op, StatDef};
        let (mut app, _, player, _) = duel(4, false, |kind| MeleeAttack::new(kind, DiceRoll::flat(1)));
        let stats = atglyph_rules::Registry::from_defs(vec![StatDef::new("mending", 0)]).unwrap();
        let mending = stats.expect("mending");
        app.world_mut().resource_mut::<Registries>().stats = stats;
        app.world_mut().resource_mut::<CombatRules>().mending = Some(mending);
        let mut block = StatBlock::default();
        block.0.add(Modifier::new(mending, Op::Add(-50), atglyph_rules::stats::ModifierSource::Game(7)));
        app.world_mut().entity_mut(player).insert(block);
        app.world_mut().get_mut::<Health>(player).unwrap().current = 10;
        app.world_mut().write_message(Heal::new(player, 8));
        app.update();
        assert_eq!(hp(&app, player), 14);
    }

    /// `Mitigation::lands` asks about the map the target stands on: a hit
    /// on an actor in a peaceful place is reported with nothing dealt and
    /// takes nothing.
    #[test]
    fn a_hit_on_an_actor_standing_in_a_peaceful_place_deals_nothing() {
        let (mut app, _, blunt) = arena();
        let floor = atglyph_grid::TileRegistry::standard().expect("floor");
        let place = crate::places::PlaceBuild {
            terrain: atglyph_grid::Terrain::filled(6, 6, floor),
            entry: Point::new(1, 1),
            exit: None,
            spots: Vec::new(),
            locks: Vec::new(),
            secrets: Vec::new(),
            ambient: atglyph_grid::Light::DARK,
            peaceful: true,
        };
        app.world_mut().resource_mut::<WorldMap>().install_place(MapId(1), place);
        let who = app.world_mut().spawn((Actor, Blocks, Position(Point::new(2, 2)), OnMap(MapId(1)), Health { current: 10, max: 30 })).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.world_mut().write_message(DamageEvent::new(who, atglyph_rules::Hit::from_source(None, blunt, 999)));
        app.update();
        assert_eq!(app.world().get::<Health>(who).unwrap().current, 10);
        let dealt: Vec<i32> = app.world_mut().resource_mut::<Messages<DamageDealt>>().drain().map(|d| d.dealt).collect();
        assert_eq!(dealt, vec![0]);
    }

    #[test]
    fn the_damage_pipeline_subtracts_armor_unless_the_game_said_otherwise() {
        // A game that names no pipeline gets the one every game wrote by
        // hand, rather than raw damage and no word about why.
        let stages = DamageStages::default();
        assert_eq!(stages.0.len(), 1, "one stage by default");

        let mut app = headless_app();
        app.add_plugins(CombatPlugin);
        assert_eq!(app.world().resource::<DamageStages>().0.len(), 1, "the plugin leaves the default in place");

        // And a game that inserted its own before the plugin keeps it.
        let mut own = headless_app();
        own.insert_resource(DamageStages(Vec::new()));
        own.add_plugins(CombatPlugin);
        assert!(own.world().resource::<DamageStages>().0.is_empty(), "a game that asked for raw damage still gets it");
    }

    /// The player's side.
    const OURS: FactionId = FactionId::from_raw(0);
    /// Its foes.
    const THEIRS: FactionId = FactionId::from_raw(1);
    /// Neutral to both.
    const BYSTANDERS: FactionId = FactionId::from_raw(2);

    /// A player armed with what `arm` makes of the one damage kind and,
    /// round it, whoever `around` places: each an offset from the player
    /// and a side, with a thousand health and no mind, so nothing struck
    /// dies of it. Three sides, the player's hostile to the second and
    /// neutral to the third; `plugins` adds what a test needs beside
    /// combat and the items a hand can hold, the run's seed is `seed`, and
    /// play has begun with everything [`see`] records recorded.
    fn crowd_with<B: Bundle>(
        plugins: impl FnOnce(&mut App),
        seed: u64,
        arm: impl FnOnce(DamageKindId) -> B,
        around: &[((i32, i32), FactionId)],
    ) -> (App, Point, Entity, Vec<Entity>) {
        let (mut app, start, kind) = arena();
        app.add_plugins((crate::items::ItemsPlugin, crate::effects::EffectsPlugin));
        crate::app::EngineApp::add_engine_effects(&mut app);
        plugins(&mut app);
        let sides = Registry::from_defs(["ours", "theirs", "bystanders"].map(FactionDef::new).to_vec()).unwrap();
        app.insert_resource(CombatRules::new().hostile(OURS, THEIRS)).insert_resource(crate::seed::Seed(atglyph_core::RunSeed(seed)));
        app.world_mut().resource_mut::<Registries>().factions = sides;
        app.init_resource::<Seen>().add_systems(PostUpdate, see);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), Health::full(30), Faction(OURS), arm(kind))).id();
        let mut others = Vec::new();
        for &((dx, dy), side) in around {
            others.push(app.world_mut().spawn((Actor, Blocks, Position(start.offset(dx, dy)), Health::full(1000), Faction(side))).id());
        }
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (app, start, player, others)
    }

    /// [`crowd_with`] and nothing more.
    fn crowd<B: Bundle>(seed: u64, arm: impl FnOnce(DamageKindId) -> B, around: &[((i32, i32), FactionId)]) -> (App, Point, Entity, Vec<Entity>) {
        crowd_with(|_| {}, seed, arm, around)
    }

    /// What each of `who` has lost of its thousand.
    fn taken(app: &App, who: &[Entity]) -> Vec<i32> {
        who.iter().map(|w| 1000 - hp(app, *w)).collect()
    }

    /// `player` attacks `target` and the turns run until it may act again.
    fn attack(app: &mut App, player: Entity, target: Entity) {
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
    }

    /// A sweep strikes every foe beside the attacker, each with a roll of
    /// its own, and passes over an ally and a bystander standing as close
    /// and a foe one step farther off.
    #[test]
    fn a_sweep_strikes_every_adjacent_foe_with_its_own_roll_and_no_ally() {
        let around = [((1, 0), THEIRS), ((-1, 1), THEIRS), ((0, -1), THEIRS), ((1, 1), OURS), ((-1, 0), BYSTANDERS), ((2, 0), THEIRS)];
        let mut apart = false;
        for seed in 0..6 {
            let (mut app, _, player, others) = crowd(seed, |kind| MeleeAttack::new(kind, DiceRoll::new(1, 100)).shaped(AttackShape::Sweep), &around);
            attack(&mut app, player, others[0]);
            let taken = taken(&app, &others);
            assert!(taken[..3].iter().all(|t| (1..=100).contains(t)), "seed {seed}: every foe beside the player was struck: {taken:?}");
            assert_eq!(taken[3..], [0, 0, 0], "seed {seed}: the ally, the bystander and the foe a step off were not");
            apart |= taken[0] != taken[1] || taken[1] != taken[2];
        }
        assert!(apart, "each foe is rolled for on its own, so the three do not take the same blow on every seed");
    }

    /// A pierce strikes its target and the foes in the next cells in line
    /// beyond it, as many cells as it names: it runs on past an ally
    /// without striking it, keeps to its line, diagonal or not, and stops
    /// at a wall.
    #[test]
    fn a_pierce_strikes_the_target_and_the_foes_in_the_next_cells_in_line() {
        let thrust = |kind| MeleeAttack::new(kind, DiceRoll::flat(5)).shaped(AttackShape::Pierce(2));
        let (mut app, _, player, others) = crowd(0, thrust, &[((1, 0), THEIRS), ((2, 0), OURS), ((3, 0), THEIRS), ((4, 0), THEIRS), ((2, 1), THEIRS)]);
        attack(&mut app, player, others[0]);
        assert_eq!(taken(&app, &others), [5, 0, 5, 0, 0], "the target and the foe two beyond; not the ally between, the foe three beyond, or one off the line");

        let (mut app, _, player, others) = crowd(0, thrust, &[((1, 1), THEIRS), ((2, 2), THEIRS), ((3, 3), THEIRS)]);
        attack(&mut app, player, others[0]);
        assert_eq!(taken(&app, &others), [5, 5, 5], "a diagonal is a line too");

        let (mut app, start, player, others) = crowd(0, thrust, &[((1, 0), THEIRS), ((3, 0), THEIRS)]);
        app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(2, 0), TileId(1));
        app.update();
        attack(&mut app, player, others[0]);
        assert_eq!(taken(&app, &others), [5, 0], "a wall between stops it");
    }

    /// A shot that pierces runs on through its target to the foes in line
    /// behind it, and never past its range.
    #[test]
    fn a_piercing_shot_strikes_the_foes_behind_its_target_and_none_past_its_range() {
        let gun = |kind| RangedAttack::new(kind, DiceRoll::flat(4), 4).shaped(AttackShape::Pierce(2));
        let (mut app, _, player, others) = crowd(0, gun, &[((3, 0), THEIRS), ((4, 0), THEIRS), ((5, 0), THEIRS)]);
        attack(&mut app, player, others[0]);
        assert_eq!(taken(&app, &others), [4, 4, 0], "the target and the foe behind it; the next is in the shape but past the range");
    }

    /// A watched shot that pierces flies once, from the shooter to the
    /// farthest foe it strikes, and lands on each when the flight has
    /// been seen.
    #[test]
    fn a_watched_piercing_shot_flies_to_the_farthest_foe_it_strikes_and_lands_on_each_once_seen() {
        let gun = |kind| RangedAttack::new(kind, DiceRoll::flat(4), 8).shaped(AttackShape::Pierce(1)).looking(LOOK);
        let (mut app, start, player, others) = crowd(0, gun, &[((3, 0), THEIRS), ((4, 0), THEIRS)]);
        app.world_mut().resource_mut::<CueWait>().watch();
        attack(&mut app, player, others[0]);
        assert_eq!(taken(&app, &others), [0, 0], "in the air");
        let flight = Cue::Flight { from: Anchor::on(player, start), to: Anchor::on(others[1], start.offset(4, 0)), look: LookOf::Given(LOOK) };
        assert_eq!(app.world().resource::<Seen>().cues, vec![Cued { actor: player, cue: flight }]);
        app.world_mut().resource_mut::<CueWait>().release();
        app.update();
        assert_eq!(taken(&app, &others), [4, 4], "both, once the flight was seen");
        assert!(!app.world().resource::<CueWait>().in_flight());
    }

    /// How often a worn weapon's `fire` moment went off, and where each of
    /// its `hit` moments did.
    #[derive(Resource, Default)]
    struct Tally {
        fire: usize,
        hit: Vec<Point>,
    }

    fn tally(mut tally: ResMut<Tally>, mut fired: MessageReader<crate::effects::Fired>) {
        for f in fired.read() {
            if f.moment == crate::effects::TriggerKinds::FIRE {
                tally.fire += 1;
            } else if f.moment == crate::effects::TriggerKinds::HIT {
                tally.hit.push(f.at);
            }
        }
    }

    /// Puts a thing striking with `attack` and carrying the triggers
    /// `triggers` writes in `who`'s one hand.
    fn wield(app: &mut App, who: Entity, attack: impl Bundle, triggers: &str) {
        use atglyph_rules::{EquipShape, Equipment, SlotId};
        let specs: Vec<atglyph_rules::TriggerSpec> = atglyph_rules::Names::new().load_list(triggers).expect("the triggers parse");
        let world = app.world();
        let names = world.resource::<Registries>().names();
        let built = crate::effects::Triggers::build(
            &specs,
            &[],
            world.resource::<crate::effects::TriggerKinds>(),
            world.resource::<crate::effects::EffectKinds>(),
            &names,
        )
        .expect("the triggers build");
        let hand = SlotId::from_raw(0);
        let item = app.world_mut().spawn((crate::items::Item, attack, built, crate::items::Wearable(EquipShape::in_slot(hand)))).id();
        let mut equipment = Equipment::with_slot_count(1);
        equipment.equip(item, &EquipShape::in_slot(hand)).unwrap();
        app.world_mut().entity_mut(who).insert((crate::items::Inventory { items: vec![item] }, Equipped(equipment)));
    }

    /// However many a shaped blow strikes, it is one swing: [`Struck`] is
    /// written once and the weapon's `fire` moment goes off once, while
    /// its `hit` triggers land on each foe struck, where it stands.
    #[test]
    fn a_shaped_blow_is_one_swing_and_its_hit_trigger_lands_on_each_foe_struck() {
        let ring = [((1, 0), THEIRS), ((0, 1), THEIRS), ((-1, -1), THEIRS)];
        let (mut app, start, player, foes) = crowd(0, |_| (), &ring);
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        let harm = r#"[(on: "hit", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))])]"#;
        wield(&mut app, player, MeleeAttack::new(kind, DiceRoll::flat(0)).shaped(AttackShape::Sweep), harm);
        app.init_resource::<Tally>().add_systems(PostUpdate, tally);
        attack(&mut app, player, foes[0]);
        assert_eq!(app.world().resource::<Seen>().struck.len(), 1, "one swing, one Struck");
        let tally = app.world().resource::<Tally>();
        assert_eq!(tally.fire, 1, "and the weapon fired once");
        let mut hit = tally.hit.clone();
        hit.sort();
        let mut stood: Vec<Point> = ring.iter().map(|((dx, dy), _)| start.offset(*dx, *dy)).collect();
        stood.sort();
        assert_eq!(hit, stood, "its hit moment went off where each foe stood");
        assert_eq!(taken(&app, &foes), [3, 3, 3], "and what it carries landed on each");
    }

    /// A shaped blow with a look is seen on everyone it struck: one burst,
    /// anchored once on each foe, the target first and the rest in the
    /// order the sweep went round, and on nobody it passed over.
    #[test]
    fn a_shaped_blow_with_a_look_bursts_once_on_each_foe_it_struck_and_on_nobody_else() {
        let around = [((1, 0), THEIRS), ((0, 1), THEIRS), ((-1, -1), THEIRS), ((1, 1), OURS), ((-1, 0), BYSTANDERS)];
        let (mut app, start, player, others) = crowd(0, |kind| MeleeAttack::new(kind, DiceRoll::flat(1)).shaped(AttackShape::Sweep).looking(LOOK), &around);
        attack(&mut app, player, others[0]);
        let on = [(0, (1, 0)), (1, (0, 1)), (2, (-1, -1))].map(|(i, (dx, dy))| Anchor::on(others[i], start.offset(dx, dy))).to_vec();
        let burst = Cue::Burst { on, look: LookOf::Given(LOOK), from: None };
        assert_eq!(app.world().resource::<Seen>().cues, vec![Cued { actor: player, cue: burst }]);
    }

    /// Strikes `target` once, with it made to know of `player` first or of
    /// nobody, and answers what it took.
    fn strike(app: &mut App, player: Entity, target: Entity, knows: bool) -> i32 {
        use atglyph_rules::ai::awareness::Awareness;
        let at = app.world().get::<Position>(player).unwrap().0;
        let mut aware = app.world_mut().get_mut::<crate::stealth::Aware>(target).unwrap();
        aware.0.clear();
        if knows {
            aware.0.insert(player, Awareness::Alert { at, stale_turns: 0 });
        }
        let before = hp(app, target);
        attack(app, player, target);
        before - hp(app, target)
    }

    /// A quiet player, armed to strike and to shoot for two, beside one
    /// foe that notices and three cells from another, in a game whose blows
    /// on the unaware land at three times; `plugins` says whether stealth
    /// runs.
    fn ambush(plugins: impl FnOnce(&mut App)) -> (App, Entity, Entity, Entity) {
        let arm = |kind| (MeleeAttack::new(kind, DiceRoll::flat(2)), RangedAttack::new(kind, DiceRoll::flat(2), 6), crate::stealth::Stealth::default());
        let (mut app, _, player, others) = crowd_with(plugins, 0, arm, &[((1, 0), THEIRS), ((0, 3), THEIRS)]);
        for watcher in &others {
            app.world_mut().entity_mut(*watcher).insert(crate::stealth::Notice::default());
        }
        app.world_mut().resource_mut::<CombatRules>().unaware = Some(300);
        (app, player, others[0], others[1])
    }

    /// A blow on a target that has not noticed its attacker lands
    /// multiplied, by the house's percent or by the blow's own where it
    /// names one, and not on a target that has, from an attacker that does
    /// not hide, or as a shot.
    #[test]
    fn an_unaware_target_takes_the_multiplied_blow_and_an_aware_one_does_not() {
        let (mut app, player, beside, across) = ambush(|app| {
            app.add_plugins((crate::minds::MindsPlugin, crate::stealth::StealthPlugin));
        });
        assert_eq!(strike(&mut app, player, beside, false), 6, "the house's three times on a target that never noticed");
        assert_eq!(strike(&mut app, player, beside, true), 2, "and nothing more on one that had");
        app.world_mut().get_mut::<MeleeAttack>(player).unwrap().unaware = Some(500);
        assert_eq!(strike(&mut app, player, beside, false), 10, "a blow's own five times over the house's three");
        app.world_mut().get_mut::<MeleeAttack>(player).unwrap().unaware = Some(100);
        assert_eq!(strike(&mut app, player, beside, false), 2, "and its own even when that is no more than a blow");
        assert_eq!(strike(&mut app, player, across, false), 2, "a shot on the unaware lands as it would on anyone");
        app.world_mut().get_mut::<MeleeAttack>(player).unwrap().unaware = None;
        app.world_mut().entity_mut(player).remove::<crate::stealth::Stealth>();
        assert_eq!(strike(&mut app, player, beside, false), 2, "and an attacker that does not hide is never unnoticed");
    }

    /// Without the stealth plugin nobody is unaware: nothing keeps an
    /// observer's memory, so one that holds nothing has not been caught
    /// off guard.
    #[test]
    fn without_the_stealth_plugin_no_one_is_unaware() {
        let (mut app, player, beside, _) = ambush(|_| {});
        assert_eq!(strike(&mut app, player, beside, false), 2);
    }

    /// Gives `who` the triggers `triggers` writes, as a game gives an actor
    /// what it answers.
    fn answers(app: &mut App, who: Entity, triggers: &str) {
        let specs: Vec<atglyph_rules::TriggerSpec> = atglyph_rules::Names::new().load_list(triggers).expect("the triggers parse");
        let world = app.world();
        let names = world.resource::<Registries>().names();
        let built = crate::effects::Triggers::build(
            &specs,
            &[],
            world.resource::<crate::effects::TriggerKinds>(),
            world.resource::<crate::effects::EffectKinds>(),
            &names,
        )
        .expect("the triggers build");
        app.world_mut().entity_mut(who).insert(built);
    }

    /// A harm of `roll` on `on`, landing over `area`.
    fn harming(on: &str, area: &str, roll: i32) -> String {
        format!(r#"[(on: "{on}", area: {area}, effects: [(kind: "Harm", args: (kind: "kinetic", roll: "{roll}"))])]"#)
    }

    /// Every death, copied out as it is written, beside what whoever gets
    /// the credit was called then.
    #[derive(Resource, Default)]
    struct Deaths(Vec<(DeathEvent, Option<String>)>);

    fn mourn(mut deaths: MessageReader<DeathEvent>, names: Query<&Name>, mut seen: ResMut<Deaths>) {
        for death in deaths.read() {
            let called = death.credit.and_then(|c| names.get(c).ok()).map(|n| n.as_str().to_string());
            seen.0.push((*death, called));
        }
    }

    /// Every `struck` and `died` moment, copied out as it is written.
    #[derive(Resource, Default)]
    struct Answered(Vec<crate::effects::Fired>);

    fn answered(mut fired: MessageReader<crate::effects::Fired>, mut seen: ResMut<Answered>) {
        let theirs = [crate::effects::TriggerKinds::STRUCK, crate::effects::TriggerKinds::DIED];
        seen.0.extend(fired.read().filter(|f| theirs.contains(&f.moment)).copied());
    }

    /// Runs the turns on until the player has waited `passes` times.
    fn wait(app: &mut App, player: Entity, passes: usize) {
        for _ in 0..passes {
            app.world_mut().write_message(Intent::new(player, crate::turn::Wait));
            app.update();
        }
    }

    /// An actor's own `hit` lands on whom its blow struck, where they
    /// stand, as its doing, and not on itself or on anyone beside.
    #[test]
    fn an_actors_hit_trigger_lands_on_whom_its_own_blow_struck_and_on_nobody_else() {
        let (mut app, _, player, others) = crowd(0, |kind| MeleeAttack::new(kind, DiceRoll::flat(1)), &[((1, 0), THEIRS), ((1, 1), THEIRS)]);
        answers(&mut app, player, &harming("hit", "Here", 3));
        attack(&mut app, player, others[0]);
        assert_eq!(taken(&app, &others), [4, 0], "the blow and the bite on the one struck, nothing on the one beside");
        assert_eq!(hp(&app, player), 30, "and nothing on the biter");
        let bite: Vec<_> = app.world().resource::<Seen>().dealt.iter().filter(|d| d.reach == DamageRoute::Effect).map(|d| (d.target, d.hit.credit)).collect();
        assert_eq!(bite, vec![(others[0], Some(player))], "credited to the biter");
    }

    /// A blow from a worn weapon by an actor that answers `hit` itself
    /// lands the weapon's `hit`, the wielder's own, and the `struck` of the
    /// one it landed on, each once and in that order, after the blow.
    #[test]
    fn a_weapons_hit_its_wielders_own_hit_and_the_struck_ones_struck_each_land_once_in_that_order() {
        let (mut app, _, player, others) = crowd(0, |_| (), &[((1, 0), THEIRS)]);
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        wield(&mut app, player, MeleeAttack::new(kind, DiceRoll::flat(1)), &harming("hit", "Here", 2));
        answers(&mut app, player, &harming("hit", "Here", 5));
        answers(&mut app, others[0], &harming("struck", "Burst(radius: 1)", 7));
        attack(&mut app, player, others[0]);
        let dealt: Vec<(Entity, i32, Option<Entity>)> = app.world().resource::<Seen>().dealt.iter().map(|d| (d.target, d.dealt, d.hit.credit)).collect();
        let (them, me) = (others[0], player);
        assert_eq!(
            dealt,
            vec![(them, 1, Some(me)), (them, 2, Some(me)), (them, 5, Some(me)), (me, 7, Some(them))],
            "the blow, the blade's edge, the wielder's venom, then what the one struck answered, as its own, round it"
        );
    }

    /// A shaped blow lands on each foe it strikes, so its attacker's own
    /// `hit` lands once on each, as a weapon's does.
    #[test]
    fn an_actors_hit_trigger_lands_once_on_each_foe_a_shaped_blow_strikes() {
        let ring = [((1, 0), THEIRS), ((0, 1), THEIRS), ((-1, -1), THEIRS), ((1, 1), OURS)];
        let (mut app, _, player, others) = crowd(0, |kind| MeleeAttack::new(kind, DiceRoll::flat(0)).shaped(AttackShape::Sweep), &ring);
        answers(&mut app, player, &harming("hit", "Here", 3));
        attack(&mut app, player, others[0]);
        assert_eq!(taken(&app, &others), [3, 3, 3, 0], "once on each foe swept, and not on the ally it passed over");
    }

    /// A blow that lands on an actor sets off its `struck` where it stands,
    /// as its own doing: an aura that burns whoever stands beside it burns
    /// the one who struck it, credited to it. A miss sets off nothing.
    #[test]
    fn an_actors_struck_trigger_lands_where_it_stands_as_its_own_doing_and_a_miss_sets_off_nothing() {
        let (mut app, _, player, others) = crowd(0, |kind| MeleeAttack::new(kind, DiceRoll::flat(0)), &[((1, 0), THEIRS)]);
        answers(&mut app, others[0], &harming("struck", "Burst(radius: 1)", 4));
        app.insert_resource(crate::accuracy::HitRules(Box::new(Never)));
        attack(&mut app, player, others[0]);
        assert_eq!((hp(&app, player), taken(&app, &others)), (30, vec![0]), "a miss lands on nobody, so nothing answers it");
        app.insert_resource(crate::accuracy::HitRules::default());
        wait(&mut app, player, 1);
        attack(&mut app, player, others[0]);
        assert_eq!(hp(&app, player), 26, "the aura burned the one who struck it");
        let burn: Vec<Option<Entity>> = app.world().resource::<Seen>().dealt.iter().filter(|d| d.target == player).map(|d| d.hit.credit).collect();
        assert_eq!(burn, vec![Some(others[0])], "as its own doing");
    }

    /// An actor's own trigger lands on everyone its area reaches but the
    /// actor it belongs to: an aura round the one struck spares it and
    /// burns the striker and its neighbour, and a bite that bursts round
    /// whom it struck spares the biter standing in the burst.
    #[test]
    fn an_actors_own_trigger_spares_its_bearer_and_lands_on_everyone_round_it() {
        let (mut app, _, player, others) = crowd(0, |kind| MeleeAttack::new(kind, DiceRoll::flat(0)), &[((1, 0), THEIRS), ((1, 1), THEIRS)]);
        answers(&mut app, others[0], &harming("struck", "Burst(radius: 1)", 4));
        answers(&mut app, player, &harming("hit", "Burst(radius: 1)", 2));
        attack(&mut app, player, others[0]);
        assert_eq!(taken(&app, &others), [2, 6], "the bearer of the aura takes only the bite, its neighbour both");
        assert_eq!(hp(&app, player), 26, "and the biter only the aura");
    }

    /// A blow that kills sets off `struck` as it lands, since its damage is
    /// applied only later in the pass, and `died` once it has: each once,
    /// in that order.
    #[test]
    fn a_blow_that_kills_sets_off_struck_and_then_died_each_once() {
        let (mut app, _, player, others) = crowd(0, |kind| MeleeAttack::new(kind, DiceRoll::flat(5)), &[((1, 0), THEIRS)]);
        let doomed = others[0];
        app.world_mut().entity_mut(doomed).insert(Health::full(1));
        answers(
            &mut app,
            doomed,
            r#"[(on: "struck", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "0"))]), (on: "died", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "0"))])]"#,
        );
        app.init_resource::<Answered>().add_systems(PostUpdate, answered);
        attack(&mut app, player, doomed);
        wait(&mut app, player, 2);
        let answered: Vec<_> = app.world().resource::<Answered>().0.iter().map(|f| (f.moment, f.by)).collect();
        let (struck, died) = (crate::effects::TriggerKinds::STRUCK, crate::effects::TriggerKinds::DIED);
        assert_eq!(answered, vec![(struck, Some(player)), (died, Some(player))], "each set off by the blow that killed it");
    }

    /// The actor whose `died` a test watches for.
    #[derive(Resource)]
    struct Doomed(Entity);

    /// Where a `died` moment was reported, by whom, and whether its actor
    /// still stood anywhere as it landed.
    #[derive(Resource, Default)]
    struct Landed(Vec<(Point, Option<Entity>, bool)>);

    fn landed(mut fired: MessageReader<crate::effects::Fired>, standing: Query<(), With<Position>>, doomed: Res<Doomed>, mut seen: ResMut<Landed>) {
        for f in fired.read().filter(|f| f.moment == crate::effects::TriggerKinds::DIED) {
            seen.0.push((f.at, f.by, standing.contains(doomed.0)));
        }
    }

    /// An actor that bursts when it dies bursts once, a pass after the
    /// blow, where it fell and with its body already out of the world, so
    /// nothing it does falls on itself. What the burst kills is its kill,
    /// credited to its stand-in, which is called what it was called; its
    /// own death stays credited to whoever struck it down, and is counted
    /// once.
    #[test]
    fn an_actors_died_trigger_lands_once_where_it_died_credited_to_it_after_its_body_is_gone() {
        let around = [((1, 0), THEIRS), ((2, 0), THEIRS), ((3, 0), THEIRS)];
        let (mut app, start, player, others) = crowd(0, |kind| MeleeAttack::new(kind, DiceRoll::flat(5)), &around);
        let (doomed, beside, past) = (others[0], others[1], others[2]);
        app.world_mut().entity_mut(doomed).insert((Health::full(1), Name::new("puffball")));
        app.world_mut().entity_mut(beside).insert(Health::full(10));
        answers(&mut app, doomed, &harming("died", "Burst(radius: 1)", 20));
        app.init_resource::<Deaths>().add_systems(PostUpdate, mourn);
        app.insert_resource(Doomed(doomed)).init_resource::<Landed>().add_systems(crate::plugin::Turn, landed.in_set(crate::plugin::ResolveSet::Fields));
        attack(&mut app, player, doomed);
        wait(&mut app, player, 3);
        assert_eq!(app.world().resource::<Landed>().0, vec![(start.offset(1, 0), Some(player), false)], "once, where it fell, from no body");
        let deaths: Vec<(Entity, Option<String>)> = app.world().resource::<Deaths>().0.iter().map(|(d, called)| (d.entity, called.clone())).collect();
        assert_eq!(
            deaths,
            vec![(doomed, None), (beside, Some("puffball".to_string()))],
            "its death is the player's, once, and the one its burst killed is its own, by its name"
        );
        let credits: Vec<Option<Entity>> = app.world().resource::<Deaths>().0.iter().map(|(d, _)| d.credit).collect();
        assert_eq!(credits[0], Some(player), "struck down by the player");
        assert!(credits[1].is_some_and(|c| c != doomed && c != player), "and its burst's kill is its stand-in's, not its body's or its killer's");
        assert_eq!(hp(&app, player), 10, "the one who struck it down stood in the burst too");
        assert_eq!(taken(&app, &[past]), [0], "and the one two cells off did not");
        let remnants = app.world_mut().query::<&crate::effects::Remnant>().iter(app.world()).count();
        assert_eq!(remnants, 0, "and what carried it is gone");
    }

    /// Who each [`DamageDealt`] was credited to, by name, read as it is
    /// written: a stand-in is gone by the end of its frame.
    #[derive(Resource, Default)]
    struct Credited(Vec<Option<String>>);

    fn credited(mut dealt: MessageReader<DamageDealt>, names: Query<&Name>, mut seen: ResMut<Credited>) {
        for d in dealt.read() {
            seen.0.push(d.hit.credit.and_then(|c| names.get(c).ok()).map(|n| n.as_str().to_string()));
        }
    }

    /// A shooter that answers `hit` and is dead and gone before its watched
    /// shot lands still lands it, from a stand-in named as it was: the shot
    /// is its doing, and the bite is its stand-in's, which the log names as
    /// it names a `died` remnant. Nothing of the stand-in is left after.
    #[test]
    fn a_shooter_gone_before_its_shot_lands_still_lands_its_own_hit_trigger() {
        use atglyph_rules::ai::{Brain, tactics::ShootAtRange};
        let (mut app, start, player, _) = crowd_with(|app| _ = app.add_plugins(crate::minds::MindsPlugin), 0, |_| (), &[]);
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        let shooter = app
            .world_mut()
            .spawn((
                (Actor, Blocks, Position(start.offset(4, 0)), Health::full(10), Faction(THEIRS), crate::minds::Perception(8)),
                RangedAttack::new(kind, DiceRoll::flat(0), 6).looking(LOOK),
                crate::minds::Mind(std::sync::Arc::new(Brain::new().then(ShootAtRange::default()))),
                Name::new("spitter"),
            ))
            .id();
        answers(&mut app, shooter, &harming("hit", "Here", 3));
        app.init_resource::<Credited>().add_systems(PostUpdate, credited);
        app.world_mut().resource_mut::<CueWait>().watch();
        wait(&mut app, player, 1);
        assert!(app.world().resource::<CueWait>().in_flight(), "it fired");
        assert_eq!(hp(&app, player), 30);
        app.world_mut().resource_mut::<Turns>().remove(shooter);
        app.world_mut().resource_mut::<Occupancy>().remove(start.offset(4, 0), shooter);
        app.world_mut().despawn(shooter);
        app.world_mut().resource_mut::<CueWait>().release();
        app.update();
        app.update();
        assert_eq!(hp(&app, player), 27, "its bite arrived with the shot");
        let credit: Vec<Option<Entity>> = app.world().resource::<Seen>().dealt.iter().filter(|d| d.target == player).map(|d| d.hit.credit).collect();
        assert_eq!(credit.len(), 2, "the shot and the bite");
        assert_eq!(credit[0], Some(shooter), "the shot its own doing");
        assert!(credit[1].is_some_and(|c| c != shooter), "and the bite its stand-in's");
        let names = &app.world().resource::<Credited>().0;
        assert_eq!(names.last(), Some(&Some("spitter".to_string())), "which is called what the shooter was called");
        let world = app.world_mut();
        let left = world
            .query_filtered::<Entity, (With<crate::effects::Remnant>, bevy::ecs::query::Allow<bevy::ecs::entity_disabling::Disabled>)>()
            .iter(world)
            .count();
        assert_eq!(left, 0, "and no stand-in is left over");
    }

    /// A shooter still standing when its watched shot lands bites as
    /// itself, and the stand-in taken for it as it fired is put away
    /// unused.
    #[test]
    fn a_shooter_still_there_when_its_shot_lands_leaves_no_stand_in() {
        use atglyph_rules::ai::{Brain, tactics::ShootAtRange};
        let (mut app, start, player, _) = crowd_with(|app| _ = app.add_plugins(crate::minds::MindsPlugin), 0, |_| (), &[]);
        let kind = app.world().resource::<Registries>().damage_kinds.expect("kinetic");
        let shooter = app
            .world_mut()
            .spawn((
                (Actor, Blocks, Position(start.offset(4, 0)), Health::full(10), Faction(THEIRS), crate::minds::Perception(8)),
                RangedAttack::new(kind, DiceRoll::flat(0), 6).looking(LOOK),
                crate::minds::Mind(std::sync::Arc::new(Brain::new().then(ShootAtRange::default()))),
            ))
            .id();
        answers(&mut app, shooter, &harming("hit", "Here", 3));
        app.world_mut().resource_mut::<CueWait>().watch();
        wait(&mut app, player, 1);
        assert!(app.world().resource::<CueWait>().in_flight(), "it fired");
        app.world_mut().resource_mut::<CueWait>().release();
        app.update();
        app.update();
        assert_eq!(hp(&app, player), 27, "its bite arrived with the shot");
        let credit: Vec<Option<Entity>> = app.world().resource::<Seen>().dealt.iter().filter(|d| d.target == player).map(|d| d.hit.credit).collect();
        assert!(credit.iter().all(|c| *c == Some(shooter)), "both its own doing: {credit:?}");
        let world = app.world_mut();
        let left = world
            .query_filtered::<Entity, (With<crate::effects::Remnant>, bevy::ecs::query::Allow<bevy::ecs::entity_disabling::Disabled>)>()
            .iter(world)
            .count();
        assert_eq!(left, 0, "and no stand-in is left over");
    }
}
