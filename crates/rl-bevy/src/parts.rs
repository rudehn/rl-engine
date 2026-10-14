//! Authored parts made components: what a game's own definition wrote in
//! [`rl_rules::parts`], put on the entity it spawns.
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

use std::sync::Arc;

use bevy::prelude::*;
use rl_core::DiceRoll;
use rl_grid::Glow;
use rl_rules::parts::{Charges, Level, Melee, Ranged, Throw};
use rl_rules::stats::Op;
use rl_rules::{EquipShape, Resistances, StatId, TagId};

use crate::ammo::Ammo;
use crate::combat::{Armor, MeleeAttack, RangedAttack, Resists, Stowed};
use crate::consumable::{Attuned, Consumable};
use crate::effects::{EffectBonus, MomentId, Moments, Triggers};
use crate::items::{Bestows, Equipped, Item, Pulse, Stack, Tagged, Wearable};
use crate::lighting::LightSource;
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
    pub throw: Option<Throw>,
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
/// [`rl_rules::parts`], and keeps it beside its definition; nothing here is
/// read from a file, so the field names in the file stay the game's. Every
/// part is optional and an empty one inserts nothing. What the engine does
/// not simulate stays off it: a name, a look, a price, what a thing asks of
/// its wearer or lends them besides stats, a game's own clock on it.
#[derive(Clone, Default)]
pub struct ItemParts {
    /// Where it is worn, its slots resolved with [`Wear::shape`](rl_rules::parts::Wear::shape).
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
    /// The light it sheds where it lies and from whoever carries it.
    pub light: Option<Glow>,
    /// What one level adds, for a thing that takes levels.
    pub level: Option<Level>,
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
        let moment = |on: MomentId| self.triggers.0.iter().any(|t| t.on == on);
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
        if moment(Moments::LAND) && self.attacks.throw.is_none() {
            wrong.push("a land trigger on a thing that cannot be thrown never lands".to_string());
        }
        if moment(Moments::PULSE) && self.wear.is_none() {
            wrong.push("a pulse trigger on a thing that is never worn never comes round".to_string());
        }
        if self.ammo.is_some() && self.attacks.ranged.is_none() {
            wrong.push("spends ammunition, on a thing that shoots nothing".to_string());
        }
        if let Some(level) = &self.level {
            if level.min.unwrap_or(0) > level.max.unwrap_or(i32::MAX) {
                wrong.push("a level whose lowest is above its highest".to_string());
            }
            if level.pulse.is_some() && !moment(Moments::PULSE) {
                wrong.push("a pulse in the level, on a thing with no pulse trigger".to_string());
            }
            if level.charges.is_some() && self.charges.is_none() {
                wrong.push("charges in the level, on a thing that holds none".to_string());
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
            throw: self.attacks.throw.map(|t| Throw { strike: t.strike.map(|(roll, kind)| (plus(roll), kind)), ..t }),
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
    pub fn bonus_at(&self, level: i32) -> EffectBonus {
        EffectBonus { amount: self.per(level, |l| l.amount), turns: self.per(level, |l| l.turns).max(0) as u32 }
    }

    /// Its pulse's period at `level`, or `None` for a thing with no pulse
    /// trigger: never under one hundredth, since a clock of nothing would
    /// come round forever in one pass.
    pub fn pulse_at(&self, level: i32) -> Option<u32> {
        let every = self.triggers.pulse_every()?;
        Some((i64::from(every) + i64::from(self.per(level, |l| l.pulse))).max(1) as u32)
    }

    /// What a fresh unit holds at `level`, or `None` for a thing with no
    /// charges: never fewer than one, since a thing with nothing to spend
    /// is spent before it is found.
    pub fn charges_at(&self, level: i32) -> Option<u16> {
        let c = self.charges?;
        Some((i32::from(c.charges) + self.per(level, |l| l.charges)).clamp(1, i32::from(u16::MAX)) as u16)
    }
}

/// What made an item: the parts it was spawned from, shared by every copy
/// of its kind, which [`level_up`] fits it again from.
#[derive(Component, Clone, Deref)]
pub struct MadeOf(pub Arc<ItemParts>);

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
/// [`Item`], [`MadeOf`], and where the parts say so, [`ItemLevel`],
/// [`Tagged`], [`Triggers`], [`Pulse`], [`Consumable`] and [`Attuned`],
/// [`Wearable`], [`Ammo`], [`Armor`], [`Resists`], the attacks,
/// [`Bestows`], [`EffectBonus`], [`LightSource`] and a [`Stack`] of one.
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
    if let Some(light) = parts.light {
        e.insert(LightSource(light));
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
/// pulse's period and what a fresh unit holds. Returns the new level, or
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

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;
    use rl_core::DiceRoll;
    use rl_rules::parts::AttackShape;

    use super::*;

    #[test]
    fn each_written_attack_part_becomes_its_component_at_the_cost_given_and_nothing_else_appears() {
        let kind = rl_rules::damage::DamageKindId::from_raw(0);
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
        let kind = rl_rules::damage::DamageKindId::from_raw(0);
        let stat = StatId::from_raw(0);
        let melee = Melee { roll: DiceRoll::new(1, 6), kind: kind.into(), look: None, shape: None, unaware: None };
        let throw = Throw { range: 5, effective: None, strike: Some((DiceRoll::new(1, 4), kind.into())) };
        let level = Level { min: Some(-2), max: Some(5), damage: Some(1), charges: Some(1), stats: vec![(stat.into(), 5)], ..Level::default() };
        Arc::new(ItemParts {
            attacks: Attacks { melee: Some(melee), throw: Some(throw), ..Attacks::default() },
            charges: Some(Charges { charges: 1, when_empty: rl_rules::WhenEmpty::Kept, recharge: Some(400), while_worn: false }),
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
        let stacked = ItemParts { stack: Some(1), wear: Some(EquipShape::in_slot(rl_rules::SlotId::from_raw(0))), ..(*blade()).clone() };
        let wrong = stacked.check();
        assert_eq!(wrong.len(), 2, "{wrong:?}");
        assert!(wrong[0].starts_with("a worn item cannot stack") && wrong[1].starts_with("a thing that stacks takes no level"), "{wrong:?}");
        assert!(blade().check().is_empty(), "{:?}", blade().check());
    }
}
