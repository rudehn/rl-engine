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

use bevy::prelude::*;
use rand::rngs::StdRng;
use rl_core::{DiceRoll, Point, RunSeed, SeedDomain, geometry};
use rl_rules::ability::Look;
use rl_rules::damage::{DamageKindId, Defender, SubtractArmor};
use rl_rules::faction::FactionDef;
use rl_rules::{DamageStage, FactionId, Factions, Hit, Registry, Relation, Resistances, StatId};

use crate::components::{Actor, Blocks, MyTurn, Player, Position};
use crate::cue::{AddAirborne, Airborne, Anchor, Cue, Cued, LookOf, TurnHold};
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

/// Takes no harm: every hit on it lands as nothing, after the stages, and a
/// heal still heals.
///
/// A component rather than a stage, because a stage sees the hit and not
/// whom it lands on, and rather than a resistance of a hundred percent,
/// because resistances add up and past a hundred they heal. For a dummy a
/// tutorial wants struck, an escort a scene keeps alive, or a debug mode.
/// The hit is still reported, with nothing dealt, so whatever narrates it
/// says it had no effect rather than saying nothing.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Invulnerable;

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
    /// The colour a blow bursts in on whoever it struck. Only the colour:
    /// a burst's cells show the drawer's own burst glyphs, so the look's
    /// glyph is not drawn. `None` shows nothing, which is how a plain blow
    /// reads: the log says it, and the target's health bar moves.
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

    /// A blow bursts on its target in `look`'s colour.
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
    /// How it got there, for whoever narrates it. Nothing in the pipeline
    /// reads it.
    pub reach: Reach,
}

impl DamageEvent {
    /// Damage that did not travel as a weapon: an ability, a status, a
    /// fire. [`Reach::Effect`], which is what most damage is.
    pub fn new(target: Entity, hit: Hit<Entity>) -> Self {
        Self { target, hit, reach: Reach::Effect }
    }

    /// The same, arriving by `reach`.
    pub fn arriving(target: Entity, hit: Hit<Entity>, reach: Reach) -> Self {
        Self { target, hit, reach }
    }
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

/// How damage reached whoever took it, for whoever says what happened.
///
/// Closed, like the narrator's own `Phrase` and for the same reason: it
/// enumerates the ways the engine itself delivers damage, and a game that
/// invents a fifth is writing its own words for it anyway. What reads it is
/// narration; nothing in the damage pipeline branches on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Reach {
    /// A blow struck in reach.
    Melee,
    /// A shot down a line of fire.
    Shot,
    /// Something thrown.
    Thrown,
    /// Anything that did not travel as a weapon: an ability, a status
    /// ticking, a fire. The default, because most damage is not a weapon
    /// and a subsystem that says nothing means this.
    #[default]
    Effect,
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
    /// How it got there, carried through from the [`DamageEvent`] so a
    /// narrator can tell a shot from a blow.
    pub reach: Reach,
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
type DefenderData = (&'static mut Health, &'static Position, Has<Player>, Has<Invulnerable>);

/// The combat components an actor or an item may carry, as a query asks
/// for them.
type Gear = (Option<&'static Armor>, Option<&'static MeleeAttack>, Option<&'static RangedAttack>, Option<&'static Strikes>);

/// A worn item's entity beside the same components, as [`Loadout::worn`]
/// yields them.
type WornRef<'a> = (Entity, Option<&'a Armor>, Option<&'a MeleeAttack>, Option<&'a RangedAttack>, Option<&'a Strikes>);

/// What an actor fights with, summed at the moment it matters.
///
/// Three layers, added together: the actor's own [`Armor`], [`Resists`],
/// [`MeleeAttack`], [`RangedAttack`] and [`Strikes`]; the same components
/// on every item in its [`Equipped`] slots, in slot order; and the value of
/// the stats [`CombatRules`] names, read off its [`StatBlock`]. A worn
/// blow or shot replaces the actor's own, since a cutlass is swung in
/// place of a fist; armor, resistances and extra strikes add up.
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
    resists: Query<'w, 's, &'static Resists>,
    spent: Query<'w, 's, &'static crate::consumable::Consumable>,
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

    /// What `who` resists: its own [`Resists`] and every worn item's, added
    /// kind by kind, so a suit that resists a tenth of a bolt resists it for
    /// whoever wears it, and stops the moment it is taken off.
    pub fn resistances(&self, who: Entity) -> Resistances {
        let mut total = self.resists.get(who).map(|r| r.0.clone()).unwrap_or_default();
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
        let wielded = self.worn(who).filter(|(item, ..)| self.charged(*item)).find_map(|(item, _, melee, ..)| melee.copied().map(|m| (Some(item), m)));
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
        let wielded = self.worn(who).filter(|(item, ..)| self.charged(*item)).find_map(|(item, _, _, ranged, _)| ranged.copied().map(|r| (Some(item), r)));
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
    /// [`rl_rules::forecast`]: both sets of rolls, both costs and the
    /// shot's reach, with the choice left to the distance.
    ///
    /// Takes the two `Vec`s the caller already built, because `Arms`
    /// borrows them and a forecast is recomputed every frame.
    pub fn arms<'a>(&self, who: Entity, blows: &'a [(DamageKindId, DiceRoll)], shots: &'a [(DamageKindId, DiceRoll)]) -> rl_rules::forecast::Arms<'a> {
        let shot = self.ranged(who);
        rl_rules::forecast::Arms {
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
}

/// What an attack is seen as, and the shots in the air while it is.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Shown<'w> {
    cues: MessageWriter<'w, Cued>,
    hold: ResMut<'w, TurnHold>,
    airborne: ResMut<'w, Airborne<ShotLanding>>,
}

/// A shot fired and not yet arrived: whom it is at, and every hit it
/// carries, the main one first, rolled when it was fired.
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
    /// Whether it flew or was struck in reach, kept so the landing a pass
    /// later narrates as what it was.
    reach: Reach,
    hits: Vec<Hit<Entity>>,
}

impl crate::cue::Lands for ShotLanding {}

/// The weapon one attack is made with, as [`Loadout`] chose it.
struct Weapon {
    from: Option<Entity>,
    ranged: bool,
    kind: DamageKindId,
    dice: DiceRoll,
    cost: Option<u32>,
    look: Option<Look>,
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
/// An attack with a [`Look`] is seen: a shot flies from the shooter to
/// the target, and a blow bursts on the target. With something watching,
/// a shot's hits wait in [`Airborne`] until the flight has been
/// seen, the way a thrown knife does; a blow has no flight to wait for
/// and hurts at once. [`Struck`] is written as the attack is made either
/// way, since what a weapon does to itself happens at the trigger, not
/// at the target.
///
/// One turn, one strike: an attack by an actor that already acted this
/// pass finds the turn spent, whatever spent it.
pub fn resolve_attacks(
    mut intents: MessageReader<Intent<Attack>>,
    mut damage: MessageWriter<DamageEvent>,
    mut resolution: Resolution,
    mut rng: ResMut<CombatRng>,
    mut arena: Arena,
    shown: Shown,
) {
    let Arena { map, occupancy, attackers, targets, loadout, struck, fired, carried } = &mut arena;
    let Shown { mut cues, mut hold, mut airborne } = shown;
    for intent in intents.read() {
        let (actor, target) = (intent.actor, intent.action.0);
        let Ok(pos) = attackers.get(actor) else { continue };
        if !resolution.claim(actor) {
            continue;
        }
        let Ok(target_pos) = targets.get(target) else {
            resolution.done(actor, rl_core::turn::BASE_ACTION_COST);
            continue;
        };
        let weapon = if geometry::is_adjacent(pos.0, target_pos.0) {
            loadout.melee_with(actor).map(|(from, m)| Weapon { from, ranged: false, kind: m.kind, dice: m.dice, cost: m.cost, look: m.look })
        } else {
            loadout.ranged_with(actor).filter(|(_, r)| line_of_fire(map, occupancy, pos.0, target_pos.0, r.range)).map(|(from, r)| Weapon {
                from,
                ranged: true,
                kind: r.kind,
                dice: r.dice,
                cost: r.cost,
                look: r.look,
            })
        };
        let Some(Weapon { from, ranged, kind, dice, cost, look }) = weapon else {
            resolution.done(actor, rl_core::turn::BASE_ACTION_COST);
            continue;
        };
        resolution.done(actor, cost.unwrap_or(rl_core::turn::BASE_ACTION_COST));
        struck.write(Struck { attacker: actor, target, with: from, ranged });
        // What the weapon does when it is used, whatever it strikes: a
        // wand's charge, a trigger on firing.
        if let Some(item) = from {
            fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::FIRE, by: Some(actor), at: pos.0 });
        }
        // Floored where it is rolled: a blow that rolls below zero has
        // missed, and the pipeline would read a negative one as a heal.
        let mut hits = vec![Hit::by(actor, kind, dice.roll_at_least(&mut **rng, 0))];
        hits.extend(loadout.strikes(actor).into_iter().map(|(kind, dice)| Hit::by(actor, kind, dice.roll_at_least(&mut **rng, 0))));
        let carried = from.and_then(|item| carried.get(item).ok()).filter(|t| t.on(crate::effects::Moments::HIT).next().is_some()).cloned();
        let shot = ShotLanding { target, attacker: actor, with: from, carried, reach: if ranged { Reach::Shot } else { Reach::Melee }, hits };
        match (look, ranged) {
            (Some(look), true) => {
                let to = Anchor::on(target, target_pos.0);
                cues.write(Cued { actor, cue: Cue::Flight { from: Anchor::on(actor, pos.0), to, look: LookOf::Given(look) } });
                let Some(shot) = airborne.launched(&mut hold, shot) else { continue };
                land(shot, target_pos.0, &mut damage, fired);
                continue;
            }
            (Some(look), false) => {
                cues.write(Cued { actor, cue: Cue::Burst { on: vec![Anchor::on(target, target_pos.0)], look: LookOf::Given(look), from: None } });
            }
            (None, _) => {}
        }
        land(shot, target_pos.0, &mut damage, fired);
    }
}

/// What a shot landing writes: its hits, the moment it struck, and a
/// remnant for a thing gone before it arrived.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Arriving<'w, 's> {
    damage: MessageWriter<'w, DamageEvent>,
    fired: MessageWriter<'w, crate::effects::Fired>,
    commands: Commands<'w, 's>,
    present: Query<'w, 's, (), With<crate::effects::Triggers>>,
}

/// Lands every shot in the air, on the first pass after its flight has
/// been seen, on a target still standing: one killed or taken out of the
/// world while the shot flew is missed, not hurt twice or looked for.
/// Landing counts as progress, so the loop goes on to deal the next turn.
pub fn land_shots(
    mut airborne: ResMut<Airborne<ShotLanding>>,
    mut hold: ResMut<TurnHold>,
    mut turns: ResMut<Turns>,
    alive: Query<&Position, (With<Health>, Without<Dead>)>,
    mut arriving: Arriving,
) {
    for mut shot in airborne.landing(&mut hold, &mut turns) {
        let Ok(at) = alive.get(shot.target) else { continue };
        // A thing spent to nothing by firing is gone by now: what its hits
        // carry lands from a remnant in its place, as the shooter's doing.
        if let Some(item) = shot.with
            && !arriving.present.contains(item)
        {
            if let Some(triggers) = shot.carried.take() {
                let remnant = crate::effects::Remnant { moment: crate::effects::Moments::HIT, by: Some(shot.attacker), at: at.0 };
                arriving.commands.spawn((triggers, remnant));
            }
            shot.with = None;
        }
        land(shot, at.0, &mut arriving.damage, &mut arriving.fired);
    }
}

/// Every hit an attack carries, down the damage pipeline, and the `hit`
/// moment on the worn item it came from, at the cell the target stands on
/// as it lands, so a trigger that bursts bursts where the shot arrived.
fn land(shot: ShotLanding, at: Point, damage: &mut MessageWriter<DamageEvent>, fired: &mut MessageWriter<crate::effects::Fired>) {
    let (target, reach) = (shot.target, shot.reach);
    damage.write_batch(shot.hits.into_iter().map(|hit| DamageEvent::arriving(target, hit, reach)));
    if let Some(item) = shot.with {
        fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::HIT, by: Some(shot.attacker), at });
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
    rl_grid::footprint(
        rl_grid::TargetMode::Bolt { range },
        from,
        to,
        map.window_tiles(),
        |p| p != to && (map.blocks_projectiles(p) || occupancy.is_occupied(p)),
        |_| false,
    )
}

/// Whether a shot from `from` reaches `to` within `range`: nothing that
/// stops projectiles and nobody standing in between.
pub fn line_of_fire(map: &WorldMap, occupancy: &Occupancy, from: Point, to: Point, range: i32) -> bool {
    shot(map, occupancy, from, to, range).landing == Some(to)
}

/// Runs the damage stages and applies what is left to health.
///
/// The armor and the resistances a blow meets are the target's
/// [`Loadout`]: its own, what it wears, and the armor stat. An [`Invulnerable`] target keeps whatever
/// heals it and nothing that harms it.
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
        let Ok((mut health, pos, is_player, invulnerable)) = targets.get_mut(ev.target) else { continue };
        if health.current <= 0 {
            continue;
        }
        let defender = Defender { armor: loadout.armor(ev.target), blocked: false };
        let resists = loadout.resistances(ev.target);
        let stage_refs: Vec<&dyn DamageStage<Entity>> = stages.0.iter().map(|s| s.as_ref() as &dyn DamageStage<Entity>).collect();
        let amount = rl_rules::resolve(&ev.hit, &defender, &resists, &registries.damage_kinds, &stage_refs);
        let amount = if invulnerable { amount.min(0) } else { amount };
        health.current = (health.current - amount).min(health.max);
        dealt.write(DamageDealt { target: ev.target, hit: ev.hit, dealt: amount, reach: ev.reach });
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
            // What an attack made with a worn thing reports, for its `fire`
            // and `hit` triggers. A game may fight and have no effects, and
            // then the queue simply stays empty.
            .add_message::<crate::effects::Fired>()
            .add_message::<Struck>()
            .init_resource::<DamageStages>()
            .add_airborne::<ShotLanding>()
            .add_action::<Attack>()
            .needs::<CombatRules>("CombatPlugin", "`CombatRules::new(&sides)`, who is hostile to whom")
            .needs::<crate::registries::Registries>("CombatPlugin", "`Registries`, with the damage kinds a blow can deal")
            .add_stream::<CombatRng>("CombatPlugin")
            .add_systems(Turn, perceive_reach.in_set(crate::plugin::PerceiveSet::Annotate))
            .add_systems(Turn, (land_shots.in_set(crate::plugin::LandSet::Shot), resolve_attacks.in_set(ResolveSet::Act)).chain())
            .add_systems(Turn, apply_damage.in_set(ResolveSet::Damage))
            .add_systems(Turn, end_run_on_player_death.in_set(crate::plugin::TurnSet::React))
            .add_systems(Turn, process_deaths.in_set(CleanupSet::Remove))
            // In `Last`, after everything that reads the frame's deaths has
            // run, which is the promise that the dead linger until the frame
            // ends, kept without naming any of those systems.
            .add_systems(Last, bury_the_dead.in_set(crate::plugin::EndOfFrame::Bury));
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

    const LOOK: Look = Look { glyph: '*', color: rl_grid::Rgb::new(255, 80, 40) };

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
            app.world_mut().resource_mut::<TurnHold>().watch();
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
        assert!(!app.world().resource::<TurnHold>().in_flight());
    }

    /// With something watching, the shot and its extra strikes wait in
    /// the air until the flight has been seen, and the trigger pull is
    /// reported when it happens, so heat and ammunition answer the shot
    /// rather than its arrival. Without a watcher it all lands at once.
    #[test]
    fn a_watched_shot_hurts_when_its_flight_has_been_seen_and_an_unwatched_one_at_once() {
        let arm = |kind| (RangedAttack::new(kind, DiceRoll::flat(3), 6).looking(LOOK), Strikes(vec![(kind, DiceRoll::flat(2))]));
        let (mut app, _, player, target) = duel(4, true, arm);
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert_eq!(hp(&app, target), 20, "in the air");
        assert_eq!(app.world().resource::<Seen>().struck.len(), 1, "but fired");
        assert!(app.world().resource::<TurnHold>().in_flight());
        assert_eq!(app.world().resource::<Turns>().now(), 0, "no turn is dealt while it flies");
        app.update();
        assert_eq!(hp(&app, target), 20, "nor does it land while the turns are held");

        app.world_mut().resource_mut::<TurnHold>().release();
        app.update();
        assert_eq!(hp(&app, target), 20 - 3 - 2, "the shot and its strike, landed");
        assert!(!app.world().resource::<TurnHold>().in_flight());
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
        app.world_mut().resource_mut::<TurnHold>().release();
        app.update();
        let dealt: Vec<(Option<Entity>, i32)> = app.world().resource::<Seen>().dealt.iter().map(|d| (d.hit.credit, d.dealt)).collect();
        assert_eq!(dealt, vec![(Some(other), 99)], "the other's blow killed it, and the shot found it dead");
        assert!(!app.world().resource::<TurnHold>().in_flight());

        let (mut app, _, player, target) = duel(4, true, |kind| RangedAttack::new(kind, DiceRoll::flat(3), 6).looking(LOOK));
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        app.world_mut().resource_mut::<Turns>().remove(target);
        app.world_mut().resource_mut::<Occupancy>().remove(start.offset(4, 0), target);
        app.world_mut().despawn(target);
        app.world_mut().resource_mut::<TurnHold>().release();
        app.update();
        assert!(app.world().resource::<Seen>().dealt.is_empty(), "gone, it takes nothing");
        assert!(!app.world().resource::<TurnHold>().in_flight());
        assert!(app.world().get::<MyTurn>(player).is_some(), "and the turn comes round");
    }

    /// A shooter that dies and is gone before its shot arrives still hits:
    /// the shot left the barrel, and the hit it carries names a shooter
    /// nothing may assume is still in the world.
    #[test]
    fn a_shot_still_lands_when_its_shooter_is_dead_and_gone_before_it_arrives() {
        use rl_rules::ai::{Brain, tactics::ShootAtRange};
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
        app.world_mut().resource_mut::<TurnHold>().watch();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(me, crate::turn::Wait));
        app.update();
        assert!(app.world().resource::<TurnHold>().in_flight(), "it fired");
        assert_eq!(hp(&app, me), 30);

        app.world_mut().resource_mut::<Turns>().remove(shooter);
        app.world_mut().resource_mut::<Occupancy>().remove(start.offset(4, 0), shooter);
        app.world_mut().despawn(shooter);
        app.world_mut().resource_mut::<TurnHold>().release();
        app.update();
        assert_eq!(hp(&app, me), 27, "the shot arrived all the same");
        let credit: Vec<Option<Entity>> = app.world().resource::<Seen>().dealt.iter().map(|d| d.hit.credit).collect();
        assert_eq!(credit, vec![Some(shooter)], "credited to a shooter who is no longer anywhere");
        assert!(!app.world().resource::<TurnHold>().in_flight());
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

    /// A gunman holding one gun in the main hand, and a target three cells
    /// east with twenty health and no armor. The gun deals `roll` and
    /// carries `consumable` and the triggers `triggers` names, if any.
    fn gunman(roll: i32, consumable: Option<crate::consumable::Consumable>, triggers: Option<&str>) -> (App, Entity, Entity, Entity) {
        use rl_rules::{EquipShape, Equipment, SlotId};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::items::ItemsPlugin, crate::consumable::ConsumablesPlugin, crate::world::StreamingPlugin));
        crate::effects::AddEngineEffects::add_engine_effects(&mut app);
        let start = crate::testing::surface(&mut app);
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
            let specs: Vec<rl_rules::TriggerSpec> = rl_rules::Names::new().load_list(text).expect("the triggers parse");
            let world = app.world();
            let registries = world.resource::<Registries>();
            let built = crate::effects::Triggers::build(
                &specs,
                &[],
                world.resource::<crate::effects::Moments>(),
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
        app.world_mut().resource_mut::<TurnHold>().watch();
        app.world_mut().write_message(Intent::new(player, Attack(target)));
        app.update();
        assert!(app.world().resource::<TurnHold>().in_flight(), "the shot is in the air");
        assert!(app.world().get_entity(gun).is_err(), "and the gun, its one charge spent, is gone");
        app.world_mut().resource_mut::<TurnHold>().release();
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

    /// An invulnerable actor is hit and takes nothing, and the hit is still
    /// reported, with nothing dealt, so a log says it had no effect; a heal
    /// still heals, since what it stops is harm.
    #[test]
    fn an_invulnerable_actor_takes_no_harm_and_is_still_healed() {
        let (mut app, start, blunt) = arena();
        let who = app.world_mut().spawn((Actor, Blocks, Position(start), Health { current: 10, max: 30 }, Invulnerable)).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.world_mut().write_message(DamageEvent::new(who, rl_rules::Hit::from_source(None, blunt, 999)));
        app.update();
        assert_eq!(app.world().get::<Health>(who).unwrap().current, 10, "nothing taken off, and not killed");
        let dealt: Vec<i32> = app.world_mut().resource_mut::<Messages<DamageDealt>>().drain().map(|d| d.dealt).collect();
        assert_eq!(dealt, vec![0], "the hit is reported, with nothing dealt");
        app.world_mut().write_message(DamageEvent::new(who, rl_rules::Hit::from_source(None, blunt, -5)));
        app.update();
        assert_eq!(app.world().get::<Health>(who).unwrap().current, 15, "a heal lands in full");
        app.world_mut().entity_mut(who).remove::<Invulnerable>();
        app.world_mut().write_message(DamageEvent::new(who, rl_rules::Hit::from_source(None, blunt, 4)));
        app.update();
        assert_eq!(app.world().get::<Health>(who).unwrap().current, 11, "and without it, a hit hurts again");
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
}
