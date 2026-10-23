//! Authored parts made components: what a game's own definition wrote in
//! [`atglyph_rules::parts`], put on the entity it spawns.
//!
//! The engine owns no definition. A game reads its file into its own type,
//! and where a field is one of the engine's parts it hands the part here
//! rather than writing the component itself, so a blow written the same in
//! two games is made the same in both. Using this is optional: a game that
//! inserts `MeleeAttack` by hand gets exactly what it inserted.
//!
//! [`Attacks`] is a blow, a shot and a throw, for a monster as for a
//! weapon. [`ItemParts`] is every part of one kind of item, which a game
//! builds once from its definition; [`insert_parts`] spawns a thing of it
//! at a level, and [`level_up`] fits a thing to a new level from the same
//! parts, so a level is applied once and never on top of itself. What a
//! level adds is the [`Level`] part's and linear; how a level is found,
//! what one is worth and what one below nought means are a game's.
//!
//! [`ActorParts`] is the same for one kind of actor: its health, side,
//! sight, wits, notice, hearing, attacks, light, abilities, the statuses
//! it holds for life and what its stats do while it holds one, its
//! triggers, resistances and drops, whether it
//! leaves remains, and the brain it decides with. [`ActorParts::insert`]
//! puts every one written on a spawned actor; one not written puts
//! nothing, and a game's defaults are its own, laid under each kind with
//! [`ActorParts::over`]. [`ActorParts::repertoire`] is what a kind can do,
//! which `atglyph_rules::ai::Standard` builds its brain from. [`innate_statuses`]
//! is the statuses an actor holds from its first turn, for a game that
//! holds one more of its own.

use std::sync::Arc;

use atglyph_core::{DiceRoll, Id};
use atglyph_grid::Glow;
use atglyph_rules::parts::{Charges, KeepAway, Level, Melee, Ranged, ThrowDef};
use atglyph_rules::stats::Op;
use atglyph_rules::{
    AbilityId, Brain, DropTable, EquipShape, FactionId, HearingStats, NoticeStats, Registry, Repertoire, Resistances, StatId, Stats, StatusDef, StatusId,
    Statuses, TagId, UNTIL_CURED, Wits,
};
use bevy::prelude::*;

use crate::ability::Grants;
use crate::ammo::Ammo;
use crate::combat::{Armor, Faction, Health, MeleeAttack, RangedAttack, Resists, Stowed};
use crate::components::Speed;
use crate::consumable::{Attuned, Consumable};
use crate::effects::{EffectBonus, TriggerKindId, TriggerKinds, Triggers};
use crate::items::{Bestows, Equipped, Item, LitWhenWorn, Pulse, Stack, Tagged, Wearable, WornStatuses};
use crate::lighting::{DarkSight, LightSource, OwnDarkSight, WornDarkSight};
use crate::loot::Drops;
use crate::minds::{Intelligence, Mind, Perception};
use crate::noise::Hearing;
use crate::registries::Registries;
use crate::remains::LeavesRemains;
use crate::status::{ActiveStatuses, StatBlock, WhileHolding};
use crate::stealth::Notice;
use crate::throwing::Throwable;

/// What something strikes, shoots and is thrown as: the attack parts of
/// one definition, a weapon's or a monster's, with what one blow or shot
/// costs.
///
/// One cost for the blow and the shot alike, because a cost is how quick
/// the thing is in the hand, not a fact about either attack; a throw costs
/// what throwing does. Every part is optional and each absent one inserts
/// nothing, so a monster that only bites is `Attacks { melee, ..default }`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attacks {
    /// The blow, while wielded or, on an actor, its own.
    pub melee: Option<Melee>,
    /// The shot.
    pub ranged: Option<Ranged>,
    /// How it flies when thrown, for an item.
    pub throw: Option<ThrowDef>,
    /// What one blow or shot costs, in hundredths of a step; `None` is an
    /// ordinary turn.
    pub cost: Option<u32>,
}

impl Attacks {
    /// Inserts on `e` the components these parts describe: a
    /// [`MeleeAttack`], a [`RangedAttack`] and a [`Throwable`], each only
    /// where its part is written.
    pub fn insert(&self, e: &mut EntityCommands) {
        if let Some(melee) = &self.melee {
            e.insert(MeleeAttack::from_part(melee, self.cost));
        }
        if let Some(ranged) = &self.ranged {
            e.insert(RangedAttack::from_part(ranged, self.cost));
        }
        if let Some(throw) = &self.throw {
            e.insert(Throwable::from_part(throw));
        }
    }
}

/// Every part of one kind of item, as a game's own definition wrote them,
/// resolved and built once at load: what [`insert_parts`] spawns a thing
/// from and [`level_up`] fits it again from.
///
/// A game builds one per definition from its own fields, typed with
/// [`atglyph_rules::parts`], and keeps it beside its definition; nothing here is
/// read from a file, so the field names in the file stay the game's. Every
/// part is optional and an empty one inserts nothing. What the engine does
/// not simulate stays off it: a name, a look, a price, what a thing asks of
/// its wearer or lends them besides stats, a game's own clock on it.
#[derive(Clone, Default)]
pub struct ItemParts {
    /// Where it is worn, its slots resolved with [`Wear::shape`](atglyph_rules::parts::Wear::shape).
    pub wear: Option<EquipShape>,
    /// What it counts as.
    pub tags: Vec<TagId>,
    /// What it does at its moments, built once and shared by every copy.
    pub triggers: Triggers,
    /// What a use, a landing or a shot costs it.
    pub charges: Option<Charges>,
    /// Flat armor while worn, at level nought.
    pub armor: i32,
    /// Percent removed per damage kind, while worn.
    pub resists: Resistances,
    /// Added to its wearer's stats while worn, whatever its level.
    pub bestows: Vec<(StatId, i32)>,
    /// The most each named stat of its wearer's comes to while it is worn.
    pub caps: Vec<(StatId, i32)>,
    /// Its blow, its shot, its throw and what a blow or a shot costs, at
    /// level nought.
    pub attacks: Attacks,
    /// The tag of what one shot spends from its wielder's bag.
    pub ammo: Option<TagId>,
    /// The key copies merge under, for a thing that stacks; the game's,
    /// usually its definition's index.
    pub stack: Option<u64>,
    /// The light it sheds, where it lies and from whoever carries it, or
    /// only from whoever wears it, as [`light_when`](Self::light_when) says.
    pub light: Option<Glow>,
    /// When `light` shines: [`LightWhen::Carried`], the default, wherever
    /// it is, or [`LightWhen::Worn`] only while it is worn.
    pub light_when: LightWhen,
    /// Statuses held on the wearer, until cured, while this is worn. Empty
    /// holds none.
    pub worn_statuses: Vec<StatusId>,
    /// Tiles the wearer sees without light while this is worn, at level
    /// nought. `None` lends none.
    pub dark_sight: Option<i32>,
    /// What one level adds, for a thing that takes levels.
    pub level: Option<Level>,
}

/// When a thing's light shines.
///
/// A choice of two rather than a flag, so what a part says reads as what
/// it means: a torch lights the room from a pack or the floor, and a ring
/// that glows only on the finger is lit by being put on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LightWhen {
    /// Wherever it is: lying on the ground, or from whoever carries it.
    #[default]
    Carried,
    /// Only while it is worn, from its wearer; dark on the ground and in a
    /// pack.
    Worn,
}

impl ItemParts {
    /// Every way these parts contradict each other or the engine, each a
    /// sentence a game puts under the item's name: what nothing in the
    /// engine would ever play, a stack that takes levels or a pulse on a
    /// thing never worn.
    ///
    /// Only what the engine knows to be wrong: whether an amount is worth
    /// writing, whether a level should grow armor, is the game's to check.
    pub fn check(&self) -> Vec<String> {
        let mut wrong = Vec::new();
        let moment = |on: TriggerKindId| self.triggers.0.iter().any(|t| t.on == on);
        if self.stack.is_some() && self.wear.is_some() {
            wrong.push("a worn item cannot stack".to_string());
        }
        if self.stack.is_some() && self.level.is_some() {
            wrong.push("a thing that stacks takes no level: every copy in a stack is one thing to the bag, and a level is one thing's own".to_string());
        }
        if let Some(c) = self.charges {
            if c.charges == 0 && self.level.as_ref().and_then(|l| l.charges).unwrap_or(0) <= 0 {
                wrong.push("a consumable with no charges is spent before it is found".to_string());
            }
            if c.recharge == Some(0) {
                wrong.push("a charge that comes back in no time at all is never spent".to_string());
            }
            if c.while_worn && self.wear.is_none() {
                wrong.push("a charge that refills only while worn, on a thing that is never worn".to_string());
            }
        }
        if moment(TriggerKinds::LAND) && self.attacks.throw.is_none() {
            wrong.push("a land trigger on a thing that cannot be thrown never lands".to_string());
        }
        if moment(TriggerKinds::PULSE) && self.wear.is_none() {
            wrong.push("a pulse trigger on a thing that is never worn never comes round".to_string());
        }
        if self.wear.is_none() && (!self.worn_statuses.is_empty() || self.dark_sight.is_some() || self.light_when == LightWhen::Worn) {
            wrong.push("lends its wearer a status, sight in the dark or a light, on a thing that is never worn".to_string());
        }
        if self.light_when == LightWhen::Worn && self.light.is_none() {
            wrong.push("a light that shines while worn, on a thing that sheds none".to_string());
        }
        if self.ammo.is_some() && self.attacks.ranged.is_none() {
            wrong.push("spends ammunition, on a thing that shoots nothing".to_string());
        }
        if let Some(level) = &self.level {
            if level.min.unwrap_or(0) > level.max.unwrap_or(i32::MAX) {
                wrong.push("a level whose lowest is above its highest".to_string());
            }
            if level.pulse.is_some() && !moment(TriggerKinds::PULSE) {
                wrong.push("a pulse in the level, on a thing with no pulse trigger".to_string());
            }
            if level.charges.is_some() && self.charges.is_none() {
                wrong.push("charges in the level, on a thing that holds none".to_string());
            }
            if level.dark_sight.is_some() && self.dark_sight.is_none() {
                wrong.push("dark sight in the level, on a thing that lends none".to_string());
            }
        }
        wrong
    }

    /// The level a thing of these parts asked to be at `level` is at:
    /// `level` within its level part's bounds, or `None` for a thing that
    /// takes no level.
    pub fn level_at(&self, level: i32) -> Option<i32> {
        self.level.as_ref().map(|part| part.clamp(level))
    }

    /// `amount` of the level part, `level` times over: nought for an
    /// amount not written and for a thing that takes no level.
    fn per(&self, level: i32, amount: impl Fn(&Level) -> Option<i32>) -> i32 {
        self.level.as_ref().and_then(amount).unwrap_or(0) * level
    }

    /// Its armor at `level`.
    pub fn armor_at(&self, level: i32) -> i32 {
        self.armor + self.per(level, |l| l.armor)
    }

    /// Its blow, its shot and its throw at `level`, each damage roll a
    /// level's damage stronger.
    pub fn attacks_at(&self, level: i32) -> Attacks {
        let damage = self.per(level, |l| l.damage);
        let plus = |roll: DiceRoll| DiceRoll { bonus: roll.bonus + damage, ..roll };
        Attacks {
            melee: self.attacks.melee.map(|m| Melee { roll: plus(m.roll), ..m }),
            ranged: self.attacks.ranged.map(|r| Ranged { roll: plus(r.roll), ..r }),
            throw: self.attacks.throw.map(|t| ThrowDef { strike: t.strike.map(|(roll, kind)| (plus(roll), kind)), ..t }),
            cost: self.attacks.cost,
        }
    }

    /// What it gives its wearer's stats at `level`: what it bestows, each
    /// stat its level part names a level's worth, and each cap, leaving out
    /// whatever adds nought.
    pub fn bestows_at(&self, level: i32) -> Vec<(StatId, Op)> {
        let mut gives: Vec<(StatId, Op)> = self.bestows.iter().map(|(stat, n)| (*stat, Op::Add(*n))).collect();
        if let Some(part) = &self.level {
            gives.extend(part.stats.iter().map(|(stat, n)| (stat.id(), Op::Add(n * level))));
        }
        gives.extend(self.caps.iter().map(|(stat, most)| (*stat, Op::AtMost(*most))));
        gives.retain(|(_, op)| *op != Op::Add(0));
        gives
    }

    /// What its effects land stronger by at `level`. Turns below nought are
    /// none, since a status cannot be inflicted for less than nothing.
    pub(crate) fn bonus_at(&self, level: i32) -> EffectBonus {
        EffectBonus { amount: self.per(level, |l| l.amount), turns: self.per(level, |l| l.turns).max(0) as u32 }
    }

    /// Its pulse's period at `level`, or `None` for a thing with no pulse
    /// trigger: never under one hundredth, since a clock of nothing would
    /// come round forever in one pass.
    pub(crate) fn pulse_at(&self, level: i32) -> Option<u32> {
        let every = self.triggers.pulse_every()?;
        Some((i64::from(every) + i64::from(self.per(level, |l| l.pulse))).max(1) as u32)
    }

    /// The sight in the dark it lends at `level`, or `None` for a thing that
    /// lends none.
    pub(crate) fn dark_sight_at(&self, level: i32) -> Option<i32> {
        self.dark_sight.map(|tiles| tiles + self.per(level, |l| l.dark_sight))
    }

    /// What a fresh unit holds at `level`, or `None` for a thing with no
    /// charges: never fewer than one, since a thing with nothing to spend
    /// is spent before it is found.
    pub(crate) fn charges_at(&self, level: i32) -> Option<u16> {
        let c = self.charges?;
        Some((i32::from(c.charges) + self.per(level, |l| l.charges)).clamp(1, i32::from(u16::MAX)) as u16)
    }
}

/// What made an item: the parts it was spawned from, shared by every copy
/// of its kind, which [`level_up`] fits it again from.
#[derive(Component, Clone, Deref)]
pub(crate) struct MadeOf(pub Arc<ItemParts>);

/// An item's level: `+N`, or below nought.
///
/// On a thing whose parts take a level, and only there. Everything the
/// level changes is already applied to the thing's components, so nothing
/// reads this to play: it is what a game names and prices a thing by, what
/// a save keeps, and what [`level_up`] adds to.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemLevel(pub i32);

/// Inserts on `e` every component `parts` describe, at `level`, and
/// returns the level it is at: `level` within the level part's bounds, or
/// nought for a thing that takes none, whatever `level` says.
///
/// [`Item`], `MadeOf`, and where the parts say so, [`ItemLevel`],
/// [`Tagged`], [`Triggers`], [`Pulse`], [`Consumable`] and [`Attuned`],
/// [`Wearable`], [`Ammo`], [`Armor`], [`Resists`], the attacks,
/// [`Bestows`], [`EffectBonus`], [`LightSource`], or [`LitWhenWorn`] for a
/// light that shines only while worn, [`WornStatuses`], [`WornDarkSight`]
/// and a [`Stack`] of one.
/// A name, a look and anything else the game keeps are the game's to add
/// beside these.
pub fn insert_parts(e: &mut EntityCommands, parts: &Arc<ItemParts>, level: i32) -> i32 {
    let level = parts.level_at(level);
    e.insert((Item, MadeOf(parts.clone())));
    if let Some(level) = level {
        e.insert(ItemLevel(level));
    }
    let level = level.unwrap_or(0);
    if !parts.tags.is_empty() {
        e.insert(Tagged(parts.tags.clone()));
    }
    if !parts.triggers.0.is_empty() {
        e.insert(parts.triggers.clone());
    }
    if let Some(every) = parts.pulse_at(level) {
        e.insert(Pulse::every(every));
    }
    if let (Some(c), Some(max)) = (parts.charges, parts.charges_at(level)) {
        let held = Consumable::new(max, c.when_empty);
        e.insert(match c.recharge {
            Some(every) => held.recharging(every),
            None => held,
        });
        if c.while_worn {
            e.insert(Attuned);
        }
    }
    if let Some(shape) = &parts.wear {
        e.insert(Wearable(shape.clone()));
    }
    if let Some(tag) = parts.ammo {
        e.insert(Ammo { tag });
    }
    if parts.resists != Resistances::new() {
        e.insert(Resists(parts.resists.clone()));
    }
    match (parts.light, parts.light_when) {
        (Some(light), LightWhen::Carried) => drop(e.insert(LightSource(light))),
        (Some(light), LightWhen::Worn) => drop(e.insert(LitWhenWorn(LightSource(light)))),
        (None, _) => {}
    }
    if !parts.worn_statuses.is_empty() {
        e.insert(WornStatuses(parts.worn_statuses.clone()));
    }
    if let Some(tiles) = parts.dark_sight_at(level) {
        e.insert(WornDarkSight(tiles));
    }
    if let Some(key) = parts.stack {
        e.insert(Stack { key, count: 1 });
    }
    let armor = parts.armor_at(level);
    if armor != 0 {
        e.insert(Armor(armor));
    }
    parts.attacks_at(level).insert(e);
    let gives = parts.bestows_at(level);
    if !gives.is_empty() {
        e.insert(Bestows(gives));
    }
    let bonus = parts.bonus_at(level);
    if bonus != EffectBonus::default() {
        e.insert(bonus);
    }
    level
}

/// Raises `item`'s level by `by`, or lowers it for a `by` below nought,
/// within its level part's bounds, and fits every component the level
/// decides to the new level from the parts it was made of: its armor, its
/// attacks, what it bestows, what its effects land stronger by, its
/// pulse's period, what a fresh unit holds and the sight in the dark it
/// lends. Returns the new level, or
/// `None` for a thing that takes none, which is left as it was.
///
/// Fitted from the parts rather than from the thing as it stands, so a
/// level is never added to a number that already carries one. What it has
/// left grows or shrinks with what it holds, and its refill keeps its pace
/// and its progress, since how fast a level refills is a game's rule. An
/// attack the thing has put by while it cannot make it is changed where it
/// is put by rather than put back, so raising a dry bow does not load it.
/// A wearer has their stats folded again, as when what they wear changes.
///
/// Nothing else is touched: a thing that will not come off still will not,
/// whatever its level, since what a level below nought means is a game's.
pub fn level_up(world: &mut World, item: Entity, by: i32) -> Option<i32> {
    let parts = world.get::<MadeOf>(item)?.0.clone();
    let was = world.get::<ItemLevel>(item).map_or(0, |l| l.0);
    let level = parts.level_at(was.saturating_add(by))?;
    let stowed = world.get::<Stowed>(item).copied();
    let attacks = parts.attacks_at(level);
    let mut e = world.entity_mut(item);
    e.insert(ItemLevel(level));
    match (attacks.melee.map(|m| MeleeAttack::from_part(&m, attacks.cost)), stowed) {
        (Some(blow), Some(Stowed::Melee(_))) => e.insert(Stowed::Melee(blow)),
        (Some(blow), _) => e.insert(blow),
        (None, _) => e.remove::<MeleeAttack>(),
    };
    match (attacks.ranged.map(|r| RangedAttack::from_part(&r, attacks.cost)), stowed) {
        (Some(shot), Some(Stowed::Ranged(_))) => e.insert(Stowed::Ranged(shot)),
        (Some(shot), _) => e.insert(shot),
        (None, _) => e.remove::<RangedAttack>(),
    };
    if let Some(throw) = attacks.throw {
        e.insert(Throwable::from_part(&throw));
    }
    match parts.armor_at(level) {
        0 => e.remove::<Armor>(),
        armor => e.insert(Armor(armor)),
    };
    let gives = parts.bestows_at(level);
    if gives.is_empty() {
        e.remove::<Bestows>();
    } else {
        e.insert(Bestows(gives));
    }
    match parts.bonus_at(level) {
        bonus if bonus == EffectBonus::default() => e.remove::<EffectBonus>(),
        bonus => e.insert(bonus),
    };
    if let Some(tiles) = parts.dark_sight_at(level) {
        e.insert(WornDarkSight(tiles));
    }
    if let (Some(every), Some(mut pulse)) = (parts.pulse_at(level), e.get_mut::<Pulse>()) {
        pulse.every = every;
    }
    if let (Some(max), Some(mut held)) = (parts.charges_at(level), e.get_mut::<Consumable>()) {
        let grown = i32::from(max) - i32::from(held.max);
        held.left = (i32::from(held.left) + grown).clamp(0, i32::from(max)) as u16;
        held.max = max;
    }
    let wearer = world.query::<(Entity, &Equipped)>().iter(world).find(|(_, worn)| worn.contains(item)).map(|(wearer, _)| wearer);
    if let Some(mut worn) = wearer.and_then(|wearer| world.get_mut::<Equipped>(wearer)) {
        worn.set_changed();
    }
    Some(level)
}

/// Every part of one kind of actor, as a game's own definition wrote them,
/// resolved and built once at load: what [`ActorParts::insert`] puts on
/// each one spawned.
///
/// The actor's [`ItemParts`]. A game fills one per definition from its own
/// fields, after whatever its own model folds, a list of traits or a
/// template, has made them plain values, and keeps it beside the
/// definition. `I` is the game's item definition, which what it drops
/// names.
///
/// Every field is optional and absent means no mechanic, never one the
/// engine chose: no health is nothing to strike down, no notice is a mind
/// that never notices anyone trying not to be seen, no remains is a death
/// that leaves nothing. A number written is inserted as written, nought
/// included, so a game that writes `armor: Some(0)` gets an `Armor(0)`.
/// What a game wants on every kind unless a kind says otherwise is its own
/// default, written once as another `ActorParts` and laid under each with
/// [`over`](Self::over).
///
/// What the engine does not simulate stays off it: a name, a look, what a
/// kind is worth to a score, a boss's death ending the run, a trick of the
/// game's own. Those are the game's to insert beside these.
pub struct ActorParts<I: Send + Sync + 'static> {
    /// Its health, full; absent, it has no [`Health`] and nothing strikes
    /// it down.
    pub hp: Option<i32>,
    /// Flat armor.
    pub armor: Option<i32>,
    /// Its side, which the faction matrix reads.
    pub faction: Option<FactionId>,
    /// Percent of normal speed; absent, the hundred every [`Actor`](crate::components::Actor) is
    /// spawned with.
    pub speed: Option<u32>,
    /// How far it sees, in tiles; absent, [`DEFAULT_PERCEPTION`](crate::minds::DEFAULT_PERCEPTION).
    pub perception: Option<i32>,
    /// Percent health at or below which it runs: what a brain's
    /// `FleeWhenHurt` is built with, not a component.
    pub flee_at: Option<i32>,
    /// What its mind is able to do; absent, whatever a [`Mind`] is given
    /// when it names none.
    pub wits: Option<Wits>,
    /// How it notices what is trying not to be seen.
    pub notice: Option<NoticeStats>,
    /// How it hears.
    pub hearing: Option<HearingStats>,
    /// Tiles it sees with no light at all, by itself: its [`OwnDarkSight`],
    /// which what it wears is folded with into its [`DarkSight`].
    pub dark_sight: Option<i32>,
    /// Its blow and its shot, and what each costs.
    pub attacks: Attacks,
    /// The span it holds from its enemies: what a brain's `Keep::enemies`
    /// is built with, not a component.
    pub keep_away: Option<KeepAway>,
    /// The light it sheds.
    pub light: Option<Glow>,
    /// The abilities it knows.
    pub abilities: Vec<AbilityId>,
    /// The statuses it holds for life, on from its first turn.
    pub statuses: Vec<StatusId>,
    /// What its stats do while it holds a status, by status: the kind's
    /// rule and not the status's, a body whose mending stops while it
    /// burns. Empty changes nothing.
    pub while_holding: Vec<(StatusId, Vec<(StatId, Op)>)>,
    /// What it does at its own moments, built once and shared by every one
    /// of its kind.
    pub triggers: Triggers,
    /// Percent removed per damage kind.
    pub resists: Resistances,
    /// What it leaves when it dies, each row rolled on its own.
    pub drops: DropTable<Id<I>>,
    /// Whether its death leaves remains; absent, it leaves nothing.
    pub leaves_remains: Option<bool>,
    /// The brain it decides with, built once from the rest; absent, it has
    /// no mind and never takes a turn of its own.
    pub mind: Option<Arc<Brain<Entity>>>,
}

impl<I: Send + Sync + 'static> Default for ActorParts<I> {
    fn default() -> Self {
        Self {
            hp: None,
            armor: None,
            faction: None,
            speed: None,
            perception: None,
            flee_at: None,
            wits: None,
            notice: None,
            hearing: None,
            dark_sight: None,
            attacks: Attacks::default(),
            keep_away: None,
            light: None,
            abilities: Vec::new(),
            statuses: Vec::new(),
            while_holding: Vec::new(),
            triggers: Triggers::default(),
            resists: Resistances::new(),
            drops: DropTable::default(),
            leaves_remains: None,
            mind: None,
        }
    }
}

impl<I: Send + Sync + 'static> Clone for ActorParts<I> {
    fn clone(&self) -> Self {
        Self {
            hp: self.hp,
            armor: self.armor,
            faction: self.faction,
            speed: self.speed,
            perception: self.perception,
            flee_at: self.flee_at,
            wits: self.wits,
            notice: self.notice,
            hearing: self.hearing,
            dark_sight: self.dark_sight,
            attacks: self.attacks,
            keep_away: self.keep_away,
            light: self.light,
            abilities: self.abilities.clone(),
            statuses: self.statuses.clone(),
            while_holding: self.while_holding.clone(),
            triggers: self.triggers.clone(),
            resists: self.resists.clone(),
            drops: self.drops.clone(),
            leaves_remains: self.leaves_remains,
            mind: self.mind.clone(),
        }
    }
}

/// By hand rather than derived, since a derive would ask `I`, which only
/// names what it drops, to be printable itself.
impl<I: Send + Sync + 'static> std::fmt::Debug for ActorParts<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActorParts")
            .field("hp", &self.hp)
            .field("armor", &self.armor)
            .field("faction", &self.faction)
            .field("speed", &self.speed)
            .field("perception", &self.perception)
            .field("flee_at", &self.flee_at)
            .field("wits", &self.wits)
            .field("notice", &self.notice)
            .field("hearing", &self.hearing)
            .field("dark_sight", &self.dark_sight)
            .field("attacks", &self.attacks)
            .field("keep_away", &self.keep_away)
            .field("light", &self.light)
            .field("abilities", &self.abilities)
            .field("statuses", &self.statuses)
            .field("while_holding", &self.while_holding)
            .field("triggers", &self.triggers)
            .field("resists", &self.resists)
            .field("drops", &self.drops)
            .field("leaves_remains", &self.leaves_remains)
            .field("mind", &self.mind)
            .finish()
    }
}

impl<I: Send + Sync + 'static> ActorParts<I> {
    /// These parts written over `under`, a game's defaults: each value this
    /// writes, and `under`'s where this writes none, and every list
    /// `under`'s and then this one's, the resistances summed, so what every
    /// kind has and what this kind adds both hold.
    ///
    /// The default is the game's, never the engine's: one game's monsters
    /// notice by its own rule unless a kind says otherwise, another's all
    /// leave remains, and a third writes none at all.
    pub fn over(self, under: &ActorParts<I>) -> ActorParts<I> {
        fn both<T: Clone + PartialEq>(under: &[T], mine: Vec<T>) -> Vec<T> {
            let mut all = under.to_vec();
            all.extend(mine.into_iter().filter(|t| !under.contains(t)));
            all
        }
        let mut resists = under.resists.clone();
        resists.plus(&self.resists);
        let mut triggers = under.triggers.clone();
        triggers.0.extend(self.triggers.0);
        let mut drops = under.drops.clone();
        drops.0.extend(self.drops.0);
        let mut while_holding = under.while_holding.clone();
        while_holding.extend(self.while_holding);
        ActorParts {
            hp: self.hp.or(under.hp),
            armor: self.armor.or(under.armor),
            faction: self.faction.or(under.faction),
            speed: self.speed.or(under.speed),
            perception: self.perception.or(under.perception),
            flee_at: self.flee_at.or(under.flee_at),
            wits: self.wits.or(under.wits),
            notice: self.notice.or(under.notice),
            hearing: self.hearing.or(under.hearing),
            dark_sight: self.dark_sight.or(under.dark_sight),
            attacks: Attacks {
                melee: self.attacks.melee.or(under.attacks.melee),
                ranged: self.attacks.ranged.or(under.attacks.ranged),
                throw: self.attacks.throw.or(under.attacks.throw),
                cost: self.attacks.cost.or(under.attacks.cost),
            },
            keep_away: self.keep_away.or(under.keep_away),
            light: self.light.or(under.light),
            abilities: both(&under.abilities, self.abilities),
            statuses: both(&under.statuses, self.statuses),
            while_holding,
            triggers,
            resists,
            drops,
            leaves_remains: self.leaves_remains.or(under.leaves_remains),
            mind: self.mind.or_else(|| under.mind.clone()),
        }
    }

    /// What a kind of these parts can do, as a standard chain reads it:
    /// whether it runs and at what, the span it keeps, whether it knows an
    /// ability, strikes or shoots, and its wits, the sapient a mind is
    /// given when it names none where no wits are written, as the engine
    /// would play it.
    pub fn repertoire(&self) -> Repertoire {
        Repertoire {
            flee_at: self.flee_at.unwrap_or(0),
            keep_away: self.keep_away,
            abilities: !self.abilities.is_empty(),
            melee: self.attacks.melee.is_some(),
            ranged: self.attacks.ranged.is_some(),
            wits: self.wits.unwrap_or_default(),
        }
    }

    /// Every way these parts contradict themselves, each a sentence opening
    /// with the field it is about, which a game puts under the kind's name:
    /// what nothing in the engine would ever play as written.
    ///
    /// Only what the engine knows to be wrong: whether a kind should run,
    /// or how far it should see, is the game's to check.
    pub fn check(&self) -> Vec<String> {
        let mut wrong = Vec::new();
        if self.hp.is_some_and(|hp| hp < 1) {
            wrong.push("hp: health of nothing is dead before it is spawned".to_string());
        }
        if self.speed == Some(0) {
            wrong.push("speed: a speed of nothing never acts".to_string());
        }
        if let Some(pct) = self.flee_at.filter(|pct| !(0..=100).contains(pct)) {
            wrong.push(format!("flee_at: {pct} is not a share of health, which runs from 0 to 100"));
        }
        if let Some(keep) = self.keep_away.filter(|k| k.no_closer > k.within) {
            wrong.push(format!("keep_away: no closer than {} and within {}, which no cell is", keep.no_closer, keep.within));
        }
        if self.attacks.ranged.is_some_and(|shot| shot.range < 1) {
            wrong.push("ranged: a shot reaches one tile or more".to_string());
        }
        if self.leaves_remains == Some(true) && self.hp.is_none() {
            wrong.push("leaves_remains: remains, with no health to lose".to_string());
        }
        wrong
    }

    /// Inserts on `e` every component these parts describe, each only
    /// where its part is written: [`Health`], full, [`Armor`],
    /// [`Faction`], [`Speed`], [`Perception`], [`Intelligence`],
    /// [`Notice`], [`Hearing`], [`DarkSight`] with its [`OwnDarkSight`], the attacks,
    /// [`LightSource`], [`Grants`], what it holds for life in [`ActiveStatuses`]
    /// and [`StatBlock`], `WhileHolding`, [`Triggers`], [`Resists`], [`Drops`],
    /// [`LeavesRemains`] and [`Mind`]. `registries` holds the statuses it
    /// holds.
    ///
    /// Not [`Actor`](crate::components::Actor) or where it stands: a game spawns those with its own
    /// marker, its name and its look, and hands the entity here, so a
    /// thing spawned by hand and one spawned from parts are the same kind
    /// of entity.
    pub fn insert(&self, e: &mut EntityCommands, registries: &Registries) {
        if let Some(hp) = self.hp {
            e.insert(Health::full(hp));
        }
        if let Some(armor) = self.armor {
            e.insert(Armor(armor));
        }
        if let Some(faction) = self.faction {
            e.insert(Faction(faction));
        }
        if let Some(speed) = self.speed {
            e.insert(Speed(speed));
        }
        if let Some(cells) = self.perception {
            e.insert(Perception(cells));
        }
        if let Some(mind) = &self.mind {
            e.insert(Mind(mind.clone()));
        }
        if let Some(wits) = self.wits {
            e.insert(Intelligence(wits));
        }
        if let Some(notice) = self.notice {
            e.insert(Notice(notice));
        }
        if let Some(hearing) = self.hearing {
            e.insert(Hearing(hearing));
        }
        if let Some(cells) = self.dark_sight {
            e.insert((DarkSight(cells), OwnDarkSight(cells)));
        }
        self.attacks.insert(e);
        if let Some(light) = self.light {
            e.insert(LightSource(light));
        }
        if !self.abilities.is_empty() {
            e.insert(Grants(self.abilities.clone()));
        }
        if !self.statuses.is_empty() {
            e.insert(innate_statuses(&self.statuses, &registries.statuses));
        }
        if !self.while_holding.is_empty() {
            e.insert(WhileHolding(self.while_holding.clone()));
        }
        if !self.triggers.0.is_empty() {
            e.insert(self.triggers.clone());
        }
        if self.resists != Resistances::new() {
            e.insert(Resists(self.resists.clone()));
        }
        if !self.drops.0.is_empty() {
            e.insert(Drops::<I>(self.drops.clone()));
        }
        if self.leaves_remains == Some(true) {
            e.insert(LeavesRemains);
        }
    }
}

/// `statuses` held for life, [`UNTIL_CURED`], applied in order onto a
/// fresh [`Statuses`] and the [`Stats`] they change: what an actor that
/// holds them from its first turn carries.
///
/// Put on rather than asked for, so they are there from the first turn
/// and never announced: a thing that regenerates did not just start to.
/// Public for a game that holds one more at a spawn of its own choosing,
/// a sleeper among a floor's people, and so builds the pair itself from
/// the parts' statuses and its own.
pub fn innate_statuses(statuses: &[StatusId], registry: &Registry<StatusDef>) -> (ActiveStatuses, StatBlock) {
    let (mut held, mut stats) = (Statuses::new(), Stats::new());
    for status in statuses {
        held.apply(*status, UNTIL_CURED, None, registry, &mut stats);
    }
    (ActiveStatuses(held), StatBlock(stats))
}

#[cfg(test)]
mod tests {
    use atglyph_core::DiceRoll;
    use atglyph_rules::parts::AttackShape;
    use bevy::ecs::world::CommandQueue;

    use super::*;
    use crate::components::Actor;

    #[test]
    fn each_written_attack_part_becomes_its_component_at_the_cost_given_and_nothing_else_appears() {
        let kind = atglyph_rules::damage::DamageKindId::from_raw(0);
        let melee = Melee { roll: DiceRoll::new(1, 6), kind: kind.into(), look: None, shape: Some(AttackShape::Sweep), unaware: Some(500) };
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let e = {
            let mut commands = Commands::new(&mut queue, &world);
            let mut e = commands.spawn_empty();
            Attacks { melee: Some(melee), cost: Some(200), ..Default::default() }.insert(&mut e);
            e.id()
        };
        queue.apply(&mut world);
        let blow = world.get::<MeleeAttack>(e).copied().expect("a blow");
        assert_eq!(blow, MeleeAttack::new(kind, DiceRoll::new(1, 6)).costing(200).shaped(AttackShape::Sweep).unaware(500));
        assert!(world.get::<RangedAttack>(e).is_none() && world.get::<Throwable>(e).is_none(), "no part, no component");
    }

    /// A blade that takes a level: a damage a level on its blow and its
    /// throw, an accuracy stat a level, charges a level, from minus two to
    /// plus five.
    fn blade() -> Arc<ItemParts> {
        let kind = atglyph_rules::damage::DamageKindId::from_raw(0);
        let stat = StatId::from_raw(0);
        let melee = Melee { roll: DiceRoll::new(1, 6), kind: kind.into(), look: None, shape: None, unaware: None };
        let throw = ThrowDef { range: 5, effective: None, strike: Some((DiceRoll::new(1, 4), kind.into())) };
        let level = Level { min: Some(-2), max: Some(5), damage: Some(1), charges: Some(1), stats: vec![(stat.into(), 5)], ..Level::default() };
        Arc::new(ItemParts {
            attacks: Attacks { melee: Some(melee), throw: Some(throw), ..Attacks::default() },
            charges: Some(Charges { charges: 1, when_empty: atglyph_rules::WhenEmpty::Kept, recharge: Some(400), while_worn: false }),
            armor: 1,
            level: Some(level),
            ..ItemParts::default()
        })
    }

    fn spawned(world: &mut World, parts: &Arc<ItemParts>, level: i32) -> (Entity, i32) {
        let mut queue = CommandQueue::default();
        let made = {
            let mut commands = Commands::new(&mut queue, world);
            let mut e = commands.spawn_empty();
            let at = insert_parts(&mut e, parts, level);
            (e.id(), at)
        };
        queue.apply(world);
        made
    }

    /// Its level, its blow's bonus, its armor, its thrown strike's bonus,
    /// what a fresh unit holds and what it bestows.
    type Fitted = (Option<i32>, i32, i32, Option<i32>, u16, Vec<(StatId, Op)>);

    /// What a thing at a level carries, in one line to compare.
    fn read(world: &World, e: Entity) -> Fitted {
        let blow = world.get::<MeleeAttack>(e).or_else(|| match world.get::<Stowed>(e) {
            Some(Stowed::Melee(blow)) => Some(blow),
            _ => None,
        });
        (
            world.get::<ItemLevel>(e).map(|l| l.0),
            blow.map_or(0, |b| b.dice.bonus),
            world.get::<Armor>(e).map_or(0, |a| a.0),
            world.get::<Throwable>(e).and_then(|t| t.strike).map(|(_, roll)| roll.bonus),
            world.get::<Consumable>(e).map_or(0, |c| c.max),
            world.get::<Bestows>(e).map(|b| b.0.clone()).unwrap_or_default(),
        )
    }

    #[test]
    fn a_level_adds_each_written_amount_once_a_level_and_nothing_it_does_not_write() {
        let mut world = World::new();
        let stat = StatId::from_raw(0);
        let (plus_two, at) = spawned(&mut world, &blade(), 2);
        assert_eq!(at, 2);
        assert_eq!(read(&world, plus_two), (Some(2), 2, 1, Some(2), 3, vec![(stat, Op::Add(10))]), "armor is not written, so it does not grow");
        let (plain, _) = spawned(&mut world, &blade(), 0);
        assert_eq!(read(&world, plain), (Some(0), 0, 1, Some(0), 1, vec![]), "a stat at nought is no stat");
        let (below, at) = spawned(&mut world, &blade(), -9);
        assert_eq!((at, read(&world, below).1), (-2, -2), "held at the lowest the line allows");
    }

    #[test]
    fn a_thing_with_no_level_part_takes_no_level_whatever_it_is_asked_at() {
        let mut world = World::new();
        let parts = Arc::new(ItemParts { level: None, ..(*blade()).clone() });
        let (e, at) = spawned(&mut world, &parts, 4);
        assert_eq!((at, read(&world, e)), (0, (None, 0, 1, Some(0), 1, vec![])));
        assert_eq!(level_up(&mut world, e, 1), None, "and none is added to it");
    }

    #[test]
    fn levelling_up_fits_the_thing_from_its_parts_so_two_steps_are_one_jump_and_a_stowed_blow_stays_stowed() {
        let mut world = World::new();
        let (stepped, _) = spawned(&mut world, &blade(), 0);
        let (jumped, _) = spawned(&mut world, &blade(), 2);
        world.get_mut::<Consumable>(stepped).unwrap().left = 0;
        let blow = world.entity_mut(stepped).take::<MeleeAttack>().unwrap();
        world.entity_mut(stepped).insert(Stowed::Melee(blow));
        assert_eq!(level_up(&mut world, stepped, 1), Some(1));
        assert_eq!(level_up(&mut world, stepped, 1), Some(2));
        assert_eq!(read(&world, stepped), read(&world, jumped), "never a level on top of a level");
        assert!(world.get::<MeleeAttack>(stepped).is_none(), "a blow put by stays put by");
        assert_eq!(
            world.get::<Consumable>(stepped).map(|c| (c.left, c.recharge.map(|r| r.every))),
            Some((2, Some(400))),
            "what it holds grew by two, and its pace is the game's"
        );
        assert_eq!(level_up(&mut world, stepped, 10), Some(5), "held at the highest the line allows");
    }

    #[test]
    fn parts_the_engine_would_never_play_are_each_named() {
        let stacked = ItemParts { stack: Some(1), wear: Some(EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0))), ..(*blade()).clone() };
        let wrong = stacked.check();
        assert_eq!(wrong.len(), 2, "{wrong:?}");
        assert!(wrong[0].starts_with("a worn item cannot stack") && wrong[1].starts_with("a thing that stacks takes no level"), "{wrong:?}");
        assert!(blade().check().is_empty(), "{:?}", blade().check());
    }

    /// What spawning `parts` puts on an actor, by component name, sorted.
    fn actor_components(parts: &ActorParts<()>, registries: &Registries) -> (World, Entity, Vec<String>) {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let e = {
            let mut commands = Commands::new(&mut queue, &world);
            let mut e = commands.spawn(Actor);
            parts.insert(&mut e, registries);
            e.id()
        };
        queue.apply(&mut world);
        let mut names: Vec<String> =
            world.inspect_entity(e).unwrap().map(|c| c.name().to_string().rsplit("::").next().unwrap_or_default().to_string()).collect();
        names.sort();
        (world, e, names)
    }

    #[test]
    fn an_actor_with_no_parts_written_is_an_actor_and_nothing_else() {
        let (_, _, names) = actor_components(&ActorParts::default(), &Registries::default());
        assert_eq!(names, ["Actor", "Speed"], "only what every actor requires: no health, notice, hearing, remains or mind");
    }

    #[test]
    fn each_written_actor_part_becomes_its_component_as_written_nought_included() {
        let statuses = Registry::from_defs(vec![atglyph_rules::StatusDef::new("regenerating")]).unwrap();
        let registries = Registries { statuses, ..Registries::default() };
        let parts: ActorParts<()> = ActorParts {
            hp: Some(9),
            armor: Some(0),
            notice: Some(NoticeStats::default()),
            statuses: vec![registries.statuses.expect("regenerating")],
            leaves_remains: Some(true),
            ..ActorParts::default()
        };
        let (world, e, names) = actor_components(&parts, &registries);
        assert!(names.iter().any(|n| n == "LeavesRemains") && names.iter().any(|n| n == "Notice"), "{names:?}");
        assert_eq!(world.get::<Armor>(e).map(|a| a.0), Some(0), "a nought written is a nought inserted");
        assert_eq!(world.get::<Health>(e).map(|h| (h.current, h.max)), Some((9, 9)));
        let held = &world.get::<ActiveStatuses>(e).expect("what it holds").0;
        assert!(held.has(registries.statuses.expect("regenerating")), "held from its first turn");
        assert!(world.get::<Resists>(e).is_none() && world.get::<Hearing>(e).is_none(), "nothing not written");
    }

    #[test]
    fn a_kind_written_over_a_games_defaults_keeps_its_own_values_and_adds_its_lists_to_theirs() {
        let (a, b) = (AbilityId::from_raw(0), AbilityId::from_raw(1));
        let defaults: ActorParts<()> =
            ActorParts { notice: Some(NoticeStats::default()), leaves_remains: Some(true), abilities: vec![a], ..ActorParts::default() };
        let quiet = NoticeStats { certain: 3, ..NoticeStats::default() };
        let kind: ActorParts<()> = ActorParts { notice: Some(quiet), leaves_remains: Some(false), abilities: vec![b, a], ..ActorParts::default() };
        let made = kind.over(&defaults);
        assert_eq!((made.notice, made.leaves_remains), (Some(quiet), Some(false)), "the kind's own, false included");
        assert_eq!(made.abilities, [a, b], "the defaults' then the kind's, each once");
        let plain: ActorParts<()> = ActorParts::default().over(&defaults);
        assert_eq!((plain.notice, plain.leaves_remains), (Some(NoticeStats::default()), Some(true)));
    }

    #[test]
    fn what_a_kind_can_do_is_read_from_its_parts_and_unwritten_wits_are_the_ones_a_mind_is_given() {
        let blow = Melee { roll: DiceRoll::new(1, 4), kind: atglyph_rules::damage::DamageKindId::from_raw(0).into(), look: None, shape: None, unaware: None };
        let parts: ActorParts<()> = ActorParts { flee_at: Some(25), attacks: Attacks { melee: Some(blow), ..Attacks::default() }, ..ActorParts::default() };
        let can = parts.repertoire();
        assert_eq!((can.flee_at, can.melee, can.ranged, can.abilities), (25, true, false, false));
        assert_eq!(can.wits, Intelligence::default().0, "what the engine gives a mind that names none");
    }

    #[test]
    fn actor_parts_the_engine_would_never_play_are_each_named() {
        let parts: ActorParts<()> =
            ActorParts { flee_at: Some(120), keep_away: Some(KeepAway { within: 2, no_closer: 4 }), leaves_remains: Some(true), ..ActorParts::default() };
        assert_eq!(parts.check().len(), 3, "{:?}", parts.check());
        assert!(ActorParts::<()> { hp: Some(4), leaves_remains: Some(true), ..ActorParts::default() }.check().is_empty());
    }

    /// A helmet that lends a status, a light while worn and sight in the
    /// dark: six tiles, and one more a level, up to three.
    fn helmet() -> Arc<ItemParts> {
        Arc::new(ItemParts {
            wear: Some(EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0))),
            dark_sight: Some(6),
            worn_statuses: vec![StatusId::from_raw(0)],
            light: Some(Glow::new(100, 3, atglyph_grid::Rgb::WHITE)),
            light_when: LightWhen::Worn,
            level: Some(Level { max: Some(3), dark_sight: Some(1), ..Level::default() }),
            ..ItemParts::default()
        })
    }

    /// What a thing lends its wearer goes on the thing as the components
    /// the engine lends from, its sight at its level, and a level fitted
    /// later fits the sight from the parts again.
    #[test]
    fn a_thing_that_lends_sight_a_status_and_a_light_while_worn_carries_each_and_a_level_grows_its_sight() {
        let mut world = World::new();
        let (e, level) = spawned(&mut world, &helmet(), 2);
        assert_eq!(level, 2);
        assert_eq!(world.get::<WornDarkSight>(e), Some(&WornDarkSight(8)), "six and two levels' one each");
        assert_eq!(world.get::<WornStatuses>(e).map(|h| h.0.clone()), Some(vec![StatusId::from_raw(0)]));
        assert!(world.get::<LightSource>(e).is_none() && world.get::<LitWhenWorn>(e).is_some(), "lit only once worn");
        level_up(&mut world, e, 1);
        assert_eq!(world.get::<WornDarkSight>(e), Some(&WornDarkSight(9)), "fitted again from the parts");
    }

    /// What a thing lends a wearer, on a thing never worn, a light lit when
    /// worn with no light, and dark sight in a level on a thing that lends
    /// none are each named as what the engine would never play.
    #[test]
    fn what_a_thing_lends_that_nothing_would_play_is_named() {
        let loose = ItemParts { dark_sight: Some(3), ..ItemParts::default() };
        assert_eq!(loose.check().len(), 1, "{:?}", loose.check());
        let dark = ItemParts {
            wear: Some(EquipShape::in_slot(atglyph_rules::SlotId::from_raw(0))),
            light_when: LightWhen::Worn,
            level: Some(Level { dark_sight: Some(1), ..Level::default() }),
            ..ItemParts::default()
        };
        assert_eq!(dark.check().len(), 2, "{:?}", dark.check());
        assert!(helmet().check().is_empty(), "{:?}", helmet().check());
    }
}
