//! The armory: the weapons, armor, potions, lights and gold of the
//! Undermere, loaded once from `items.ron` into the [`Armory`] resource,
//! found where `item_spawns.ron` says, and spawned as items an actor can
//! find, carry, wear and use.
//!
//! The [`Armory`] is Undermere's [`ItemMaker`]: [`finds`](crate::finds)
//! decides what lies on a floor, the engine's loot what a kill leaves and
//! what a chest holds, and both ask the armory to make each thing. A pile
//! of gold is rolled there, since how much a pile holds depends on how deep
//! it was found, and so is the level of a weapon, a piece of armor or a
//! ring, better the deeper and sometimes cursed.
//!
//! What sets one weapon apart from another is written on its line and
//! played by the engine: a blow's shape, an axe's sweep round its wielder
//! or a spear's thrust through to the foe behind, its own percent on the
//! unaware, a dagger's five times, what a blow costs, a mace's two turns,
//! and the tag a shot spends, a bow's arrows. What sets one coat apart is
//! its armor, the strength it asks and how much louder it makes every
//! step, which it gives its wearer's footfall as any worn thing gives a
//! stat.
//!
//! An [`ItemDef`] is Foundry's item schema trimmed to what Undermere uses,
//! plus `lends`: the traits a worn thing gives its wearer, the same traits
//! a monster is written as, so a ring of the salamander and a salamander
//! are fire-blooded for the same reason. The names resolve here, at load,
//! and are folded once into what each thing lends: its armor and its
//! resistances join the thing's own, which the engine counts for whoever
//! wears it, and the rest is the thing's [`Lends`], which
//! [`wearing`](crate::wearing) puts on its wearer and takes off again.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ability::Look;
use serde::Deserialize;

use crate::content::LightDef;
use crate::traits::{Folded, TraitDef};
use crate::wearing::Lends;

/// The items file, compiled in so the binary runs from anywhere.
const ITEMS_RON: &str = include_str!("../assets/items.ron");
/// Where each thing is found, compiled in beside it.
const ITEM_SPAWNS_RON: &str = include_str!("../assets/item_spawns.ron");

/// A blow as `items.ron` writes it: `(roll: "1d6", kind: "slash")`, with
/// an optional `look`, as Foundry writes one, and the `shape` and the
/// `unaware` percent that make an axe, a spear and a dagger more than
/// their rolls.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeleeDef {
    /// The damage roll, as `"NdS+B"`.
    pub roll: DiceRoll,
    /// The damage kind.
    pub kind: NameRef<DamageKind>,
    /// The colour it bursts in on whoever it strikes; absent, it shows
    /// nothing.
    #[serde(default)]
    pub look: Option<Look>,
    /// Who else a blow strikes: `Sweep`, every foe beside the wielder, or
    /// `Pierce(n)`, the next `n` cells behind the target; absent, the
    /// target alone.
    #[serde(default)]
    pub shape: Option<AttackShape>,
    /// What a blow on a foe that has not noticed the wielder is worth, in
    /// percent, in place of the house's
    /// [`UNAWARE`](crate::content::UNAWARE); absent, the house's.
    #[serde(default)]
    pub unaware: Option<u32>,
}

/// A shot as `items.ron` writes it: `(range: 8, roll: "1d6", kind:
/// "pierce")`, with an optional `effective` and `look`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RangedDef {
    /// The furthest cell it reaches.
    pub range: i32,
    /// The furthest cell with no range penalty; absent, a third of `range`.
    #[serde(default)]
    pub effective: Option<i32>,
    /// The damage roll, as `"NdS+B"`.
    pub roll: DiceRoll,
    /// The damage kind.
    pub kind: NameRef<DamageKind>,
    /// What flies from the shooter to the target; absent, a shot lands
    /// unseen.
    #[serde(default)]
    pub look: Option<Look>,
}

/// A throw as `items.ron` writes it: how far, and the blow it strikes, if
/// it strikes at all. A fire flask strikes nothing; its land trigger
/// bursts.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThrowDef {
    /// The furthest cell it reaches.
    pub range: i32,
    /// The furthest cell with no range penalty; absent, a third of `range`.
    #[serde(default)]
    pub effective: Option<i32>,
    /// The roll and damage kind it strikes whoever it hits with.
    #[serde(default)]
    pub strike: Option<(DiceRoll, NameRef<DamageKind>)>,
}

/// A thing's charges as `items.ron` writes them: what a use or a landing
/// costs it. No recharge yet, since nothing in Undermere refills.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsumableDef {
    /// What a fresh unit holds at level nought.
    pub charges: u16,
    /// What happens at the last.
    pub when_empty: WhenEmpty,
    /// Charges each level adds to what a fresh unit holds, for a thing
    /// that takes levels; absent, none.
    #[serde(default)]
    pub per_level: u16,
}

impl ConsumableDef {
    /// What a fresh unit holds at `level`: never fewer than one, since a
    /// thing with nothing to spend is spent before it is found.
    pub fn charges_at(&self, level: i32) -> u16 {
        (i32::from(self.charges) + i32::from(self.per_level) * level).clamp(1, i32::from(u16::MAX)) as u16
    }
}

/// One kind of item, as authored in `items.ron`.
///
/// Unknown fields are refused, so a field Foundry has and Undermere does
/// not, a `heat` or an `enchant`, fails the load naming it rather than
/// loading a thing that quietly does less than its line says.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    /// Unique; spawn tables and drops refer to it. One of it, singular,
    /// since the engine says a stack as "12 arrows".
    pub name: String,
    /// One character.
    pub glyph: char,
    /// `(r, g, b)` in `0..=1`.
    pub color: (f32, f32, f32),
    /// Where it is worn, if it is worn at all.
    #[serde(default)]
    pub slot: Option<NameRef<SlotDef>>,
    /// True for a thing that fits either slot of the pair its slot is one
    /// of: a one-handed weapon either hand, a ring either hand's finger.
    #[serde(default)]
    pub either: bool,
    /// Slots it also claims when worn, for a two-hander.
    #[serde(default)]
    pub also: Vec<NameRef<SlotDef>>,
    /// What it counts as, for a chest or a drop that asks by tag.
    #[serde(default)]
    pub tags: Vec<NameRef<TagDef>>,
    /// What the thing holds, landed by any of its triggers that names no
    /// list of its own.
    #[serde(default)]
    pub effects: Vec<EffectSpec>,
    /// What it does at its moments: used, thrown and landed.
    #[serde(default)]
    pub triggers: Vec<TriggerSpec>,
    /// What a use or a landing costs it; absent, nothing.
    #[serde(default)]
    pub consumable: Option<ConsumableDef>,
    /// Flat armor while worn.
    #[serde(default)]
    pub armor: i32,
    /// Percent removed per damage kind, while worn.
    #[serde(default)]
    pub resists: Vec<(NameRef<DamageKind>, i32)>,
    /// The blow it deals, while wielded.
    #[serde(default)]
    pub melee: Option<MeleeDef>,
    /// The shot it fires, while wielded.
    #[serde(default)]
    pub ranged: Option<RangedDef>,
    /// The tag of what one shot spends from its wielder's pack, for a
    /// weapon that shoots something it has to be fed: a bow's arrows.
    #[serde(default)]
    pub ammo: Option<NameRef<TagDef>>,
    /// How far it flies when thrown, and what it strikes; absent, it
    /// cannot be thrown.
    #[serde(default)]
    pub throw: Option<ThrowDef>,
    /// What one blow or shot with it costs, in hundredths of a step;
    /// absent, an ordinary turn.
    #[serde(default)]
    pub cost: Option<u32>,
    /// The light it sheds, where it lies and from whoever carries it.
    #[serde(default)]
    pub light: Option<LightDef>,
    /// The traits it gives whoever wears it, while it is worn.
    #[serde(default)]
    pub lends: Vec<NameRef<TraitDef>>,
    /// The strength it asks of whoever wears it; absent, none. Each point
    /// short costs a weapon's wielder four accuracy and armor's wearer a
    /// point of armor.
    #[serde(default)]
    pub strength: Option<i32>,
    /// What wearing it adds to its wearer's stats whatever its level: a
    /// shield's evasion.
    #[serde(default)]
    pub bestows: Vec<(NameRef<StatDef>, i32)>,
    /// What each of its levels adds to its wearer's stats while it is
    /// worn, and each level under nought takes away: a ring's whole worth.
    #[serde(default)]
    pub per_level: Vec<(NameRef<StatDef>, i32)>,
    /// Whether copies merge into one counted entry rather than one item
    /// each.
    #[serde(default)]
    pub stack: bool,
    /// How many make a pile where one is found, times the floor it is
    /// found on; absent, one.
    #[serde(default)]
    pub pile: Option<DiceRoll>,
}

impl Named for ItemDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// The item definitions and the table of what lies on which floor, loaded
/// once.
///
/// `Clone` but not `Debug`: what an item lands when it is used is a list
/// of boxed effects, shared by handle, and a trait object has nothing to
/// print.
#[derive(Resource, Clone)]
pub struct Armory {
    /// The item definitions, by id.
    pub defs: Registry<ItemDef>,
    /// What can be found on a floor, drawn by band, where the band is the
    /// floor's number: `item_spawns.ron`, through the engine's loader.
    pub table: LootTable<Id<ItemDef>>,
    /// Each definition's triggers, built once and shared by every item
    /// spawned from it.
    triggers: Vec<Triggers>,
    /// What each definition lends, its traits folded once, so a spawn
    /// reads plain values.
    lent: Vec<Folded>,
    /// Whether each definition takes levels, read once off its tags.
    levelled: Vec<bool>,
}

impl Armory {
    /// Loads `items.ron` and `item_spawns.ron` against `registries` and
    /// the `traits` an item lends, and builds every item's triggers
    /// against the effect `kinds` and `moments` registered for the run, so
    /// a flask naming an effect nobody registered is caught while the file
    /// is read rather than by a flask that quietly does nothing. Panics
    /// naming the file and every problem in it.
    pub fn load(registries: &Registries, traits: &Registry<TraitDef>, kinds: &EffectKinds, moments: &Moments) -> Self {
        let defs = load_defs(registries, traits);
        let names = registries.names();
        let tags = |d: &ItemDef| d.tags.iter().map(|t| t.id()).collect();
        let table = rl_engine::rl_rules::loot::load(ITEM_SPAWNS_RON, &names.clone().with("item", &defs), &defs, tags)
            .unwrap_or_else(|e| panic!("assets/item_spawns.ron: {e}"));
        Self::new(defs, table, registries, traits, kinds, moments).unwrap_or_else(|e| panic!("assets/items.ron: {e}"))
    }

    /// An armory of `defs`, found where `table` says, each definition's
    /// triggers built against the effect `kinds` and `moments` and what it
    /// lends folded through `traits`. Every trigger that cannot be built
    /// and every lend that does not fold is reported at once, each under
    /// its item.
    ///
    /// What [`load`](Self::load) builds the game's armory with, and what a
    /// test builds one of its own with, from items [`read`] out of a
    /// fixture: a thing the game does not have, made exactly as the
    /// game's own things are.
    pub fn new(
        defs: Registry<ItemDef>,
        table: LootTable<Id<ItemDef>>,
        registries: &Registries,
        traits: &Registry<TraitDef>,
        kinds: &EffectKinds,
        moments: &Moments,
    ) -> Result<Self, ContentError> {
        let names = registries.names();
        let (mut triggers, mut lent, mut errors) = (Vec::new(), Vec::new(), Vec::new());
        let levelled = defs.iter().map(|(_, d)| takes_levels(d, registries)).collect();
        for (_, d) in defs.iter() {
            triggers.push(match Triggers::build(&d.triggers, &d.effects, moments, kinds, &names) {
                Ok(built) => built,
                Err(mine) => {
                    errors.extend(mine.into_iter().map(|e| format!("{}: {e}", d.name)));
                    Triggers::default()
                }
            });
            let lends: Vec<_> = d.lends.iter().map(|t| t.id()).collect();
            lent.push(crate::traits::fold(&lends, traits).unwrap_or_else(|clash| {
                errors.push(format!("{}: {clash}", d.name));
                Folded::default()
            }));
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Ok(Self { defs, table, triggers, lent, levelled })
    }

    /// Whether `def` takes levels: a weapon, armor, a ring, a staff or a
    /// wand, and nothing that stacks.
    pub fn takes_levels(&self, def: Id<ItemDef>) -> bool {
        self.levelled.get(def.index()).copied().unwrap_or(false)
    }

    /// Whether `def`'s level is learned by wearing it, as well as by being
    /// told: a thing that takes levels and is worn. A staff or a wand is
    /// known as a whole once its kind is, since its level is its charges,
    /// which are seen being spent.
    pub fn learned_by_wear(&self, def: Id<ItemDef>) -> bool {
        self.takes_levels(def) && self.defs.get(def).slot.is_some()
    }

    /// The level a thing of `def` found on floor `band` is at: a ring at
    /// one to three, or, [`CURSED_RINGS_IN_TEN`] times in ten, cursed, at
    /// minus one to minus three, however deep; a weapon or a piece of armor
    /// cursed [`CURSED_GEAR_IN_FIVE`] times in five, at one of
    /// [`CURSED_GEAR_LEVELS`] below nought, and otherwise the more often
    /// better the deeper it lies, as [`better_by`] says; anything else at
    /// nought.
    ///
    /// Drawn from `rng`, the loot's own stream, and only for a thing whose
    /// level is rolled, so finding a potion draws nothing.
    pub fn found_level(&self, def: Id<ItemDef>, band: i32, registries: &Registries, rng: &mut impl rand::Rng) -> i32 {
        let tagged = |name: &str| self.defs.get(def).tags.iter().any(|t| Some(t.id()) == registries.tags.id(name));
        if !self.takes_levels(def) {
            return 0;
        }
        if tagged("ring") {
            let level = rng.random_range(RING_LEVELS);
            return if rng.random_ratio(CURSED_RINGS_IN_TEN, 10) { -level } else { level };
        }
        if !tagged("weapon") && !tagged("armor") {
            return 0;
        }
        if rng.random_ratio(CURSED_GEAR_IN_FIVE, 5) {
            return -rng.random_range(CURSED_GEAR_LEVELS);
        }
        better_by(band, rng.random_range(0..100))
    }
}

/// How much better than plain a weapon or a piece of armor found uncursed
/// on floor `floor` is, for a roll of `pct` out of a hundred: two better
/// under [`PLUS_TWO_PER_FLOOR`] percent a floor from floor
/// [`PLUS_TWO_FROM`], one better under [`PLUS_ONE_PER_FLOOR`] percent a
/// floor, and plain from there.
///
/// One roll, the two chances nested inside each other, so the chance of
/// finding a thing better at all is ten percent a floor however deep, and
/// a two better is a share of that rather than a second roll on top: on
/// floor four two in five are better, half of those by two, and from floor
/// ten every one is.
pub fn better_by(floor: i32, pct: u32) -> i32 {
    let floor = floor.max(0) as u32;
    if floor >= PLUS_TWO_FROM && pct < PLUS_TWO_PER_FLOOR * floor {
        2
    } else if pct < PLUS_ONE_PER_FLOOR * floor {
        1
    } else {
        0
    }
}

/// How many weapons and pieces of armor in five are found cursed, however
/// deep: one, so a find is worth putting on and not a thing put on blind.
pub const CURSED_GEAR_IN_FIVE: u32 = 1;

/// How far below nought a cursed weapon or piece of armor is found.
pub const CURSED_GEAR_LEVELS: std::ops::RangeInclusive<i32> = 1..=3;

/// The percent, for each floor down, of uncursed weapons and armor found at
/// least one better than plain: a tenth on floor one, all of them on ten.
pub const PLUS_ONE_PER_FLOOR: u32 = 10;

/// The percent, for each floor down, of uncursed weapons and armor found
/// two better, from [`PLUS_TWO_FROM`]: a fifth on floor four, half on ten.
pub const PLUS_TWO_PER_FLOOR: u32 = 5;

/// The shallowest floor a weapon or a piece of armor is found two better.
pub const PLUS_TWO_FROM: u32 = 4;

/// The levels a ring is found at, above nought or, cursed, below it.
pub const RING_LEVELS: std::ops::RangeInclusive<i32> = 1..=3;

/// How many rings in ten are found cursed: below nought, and stuck on
/// whoever puts one on.
pub const CURSED_RINGS_IN_TEN: u32 = 3;

/// The tags of the things that take levels: what a scroll of enchanting
/// makes better.
pub const LEVELLED_TAGS: [&str; 5] = ["weapon", "armor", "ring", "staff", "wand"];

/// What one level adds to every damage roll a weapon makes, its blow, its
/// shot and its thrown strike alike, and takes away below nought.
pub const DAMAGE_PER_LEVEL: i32 = 1;

/// What one level adds to the accuracy of whoever wields a weapon.
pub const ACCURACY_PER_LEVEL: i32 = 5;

/// What one level adds to a piece of armor's armor.
pub const ARMOR_PER_LEVEL: i32 = 1;

/// Whether `d` takes levels: it carries one of the [`LEVELLED_TAGS`] and
/// does not stack, since every copy in a stack is one thing to the bag and
/// a level is one thing's own.
fn takes_levels(d: &ItemDef, registries: &Registries) -> bool {
    !d.stack && d.tags.iter().any(|t| LEVELLED_TAGS.contains(&registries.tags.name(t.id())))
}

/// Reads an items file against `registries` and the `traits` an item
/// lends, and checks each item makes sense on its own: every problem in
/// the file at once, each under the item it is in.
pub fn read(text: &str, registries: &Registries, traits: &Registry<TraitDef>) -> Result<Registry<ItemDef>, ContentError> {
    let defs: Registry<ItemDef> = registries.names().with("trait", traits).load(text)?;
    defs.validate(|d, _| validate_def(d, traits, registries))?;
    Ok(defs)
}

/// One item's own shape, checked against nothing but itself, the traits
/// it lends and the registries it names: whether its slot claims make
/// sense, whether a trigger can ever come round, whether a worn thing can
/// give what it lends, whether what it lends folds, and whether what it
/// asks of a wearer and gives one by its level is on a thing that is worn
/// and takes levels.
fn validate_def(d: &ItemDef, traits: &Registry<TraitDef>, registries: &Registries) -> Result<(), String> {
    if d.slot.is_none() && (d.either || !d.also.is_empty()) {
        return Err("either or also without a slot".into());
    }
    if d.either && !d.also.is_empty() {
        return Err("either with also".into());
    }
    if let Some(slot) = d.slot.filter(|_| d.either)
        && pair_of(registries.slots.name(slot.id())).is_none()
    {
        return Err(format!("either, on the {} slot, which is one of no pair", registries.slots.name(slot.id())));
    }
    if d.stack && d.slot.is_some() {
        return Err("a worn item cannot stack".into());
    }
    if d.pile.is_some() && !d.stack {
        return Err("a pile of a thing that does not stack is that many things lying apart".into());
    }
    if d.consumable.is_some_and(|c| c.charges == 0 && c.per_level == 0) {
        return Err("a consumable with no charges is spent before it is found".into());
    }
    if d.slot.is_none() && (d.strength.is_some() || !d.bestows.is_empty() || !d.per_level.is_empty()) {
        return Err("asks strength of, or gives stats to, a wearer of a thing that is never worn".into());
    }
    if !takes_levels(d, registries) && (!d.per_level.is_empty() || d.consumable.is_some_and(|c| c.per_level != 0)) {
        return Err("gives something by its level, on a thing that takes no levels".into());
    }
    if d.throw.is_none() && d.triggers.iter().any(|t| t.on == "land") {
        return Err("a land trigger on a thing that cannot be thrown never lands".into());
    }
    if d.ammo.is_some() && d.ranged.is_none() {
        return Err("spends ammunition, on a thing that shoots nothing".into());
    }
    if d.slot.is_none() && d.triggers.iter().any(|t| t.on == "pulse") {
        return Err("a pulse trigger on a thing that is never worn never comes round".into());
    }
    if d.slot.is_none() && !d.lends.is_empty() {
        return Err("lends traits, on a thing that is never worn".into());
    }
    for lent in d.lends.iter().map(|t| traits.get(t.id())) {
        if let Some(field) = unlendable(lent) {
            return Err(format!("lends `{}`, whose `{field}` a worn thing cannot give", lent.name));
        }
    }
    // What it lends is folded onto a wearer the way a monster's traits are
    // folded, so two of them setting one value is the same mistake.
    let lends: Vec<_> = d.lends.iter().map(|t| t.id()).collect();
    let folded = crate::traits::fold(&lends, traits).map_err(|clash| clash.to_string())?;
    if folded.light.is_some() && d.light.is_some() {
        return Err("lends a light, and sheds one of its own: a thing has the one light".into());
    }
    Ok(())
}

/// The first value `t` sets that a worn thing cannot give its wearer, by
/// the name the traits file writes it under, or `None` when all of it can
/// be given.
///
/// A worn thing gives what it is or carries: armor, resistances, a light,
/// sight in the dark, a status held while it is worn. What a mind thinks
/// with, how fast and how far a body moves, sees, notices and hears, when
/// it runs and what it knows how to do are the body's own, read once when
/// it is made, and a thing that lent them would lend nothing at all.
fn unlendable(t: &TraitDef) -> Option<&'static str> {
    let set = [
        ("wits", t.wits.is_some()),
        ("speed", t.speed.is_some()),
        ("perception", t.perception.is_some()),
        ("flee_at", t.flee_at.is_some()),
        ("notice", t.notice.is_some()),
        ("hearing", t.hearing.is_some()),
        ("abilities", !t.abilities.is_empty()),
    ];
    set.into_iter().find_map(|(field, set)| set.then_some(field))
}

/// What wearing a thing that strikes `blow`, shoots `shot`, each every
/// `cost` hundredths of a step, and turns `armor`, is worth to a mind
/// choosing between the things it finds, for the engine's `GearScore`, or
/// `None` for a thing worth nothing to wear, a lantern or a torch.
///
/// In tenths of a point of damage a turn: a blow's average over what it
/// costs, so a quick dagger is worth less than a sword but not as little
/// as its roll alone says; half that for a shot, which a mind fighting
/// beside its enemy has no use for; and two points a turn for each point
/// of armor. Only the comparison matters, between a thing and what it
/// would displace, so the numbers need agree with nothing else. Read at
/// the thing's level, so a sword a level better is worth a level more.
fn gear_score(blow: Option<DiceRoll>, shot: Option<DiceRoll>, cost: Option<u32>, armor: i32) -> Option<i32> {
    let blow = blow.map_or(0.0, |roll| damage_a_turn(roll, cost));
    let shot = shot.map_or(0.0, |roll| damage_a_turn(roll, cost) / 2.0);
    let score = ((blow + shot + armor as f32 * 2.0) * 10.0).round() as i32;
    (score > 0).then_some(score)
}

/// What a blow of `roll` struck every `cost` hundredths of a step is worth,
/// in the tenths of a point of damage a turn every `GearScore` here is
/// counted in: the same measure for a sword lying on the floor and for a
/// goblin's own claws, so a mind's own blow is weighed against a weapon it
/// would strike with instead in the one unit.
pub fn blow_score(roll: DiceRoll, cost: Option<u32>) -> i32 {
    (damage_a_turn(roll, cost) * 10.0).round() as i32
}

/// The damage a turn a blow of `roll` deals when it costs `cost`.
fn damage_a_turn(roll: DiceRoll, cost: Option<u32>) -> f32 {
    roll.avg() * BASE_ACTION_COST as f32 / cost.unwrap_or(BASE_ACTION_COST) as f32
}

/// Every item definition in `items.ron`, checked, and nothing built from
/// them: what the bestiary reads its drops' names against. Panics naming
/// the file and every problem in it.
pub fn load_defs(registries: &Registries, traits: &Registry<TraitDef>) -> Registry<ItemDef> {
    read(ITEMS_RON, registries, traits).unwrap_or_else(|e| panic!("assets/items.ron: {e}"))
}

/// The other slot of the pair `slot` is one of, by name, or `None` for a
/// slot that goes alone: what `either` reaches for besides the slot named.
fn pair_of(slot: &str) -> Option<&'static str> {
    crate::content::PAIRS.iter().find_map(|(one, other)| match slot {
        s if s == *one => Some(*other),
        s if s == *other => Some(*one),
        _ => None,
    })
}

/// Where `d` is worn, or `None` for something never worn, such as a
/// potion.
///
/// `either` reaches for both slots of its slot's pair directly, since a
/// one-handed weapon that fits either hand, or a ring that fits either
/// finger, is not naming one slot at all; `also` folds each extra slot a
/// two-hander claims onto the one it starts in.
pub fn shape_of(d: &ItemDef, registries: &Registries) -> Option<EquipShape> {
    let slot = d.slot?.id();
    if d.either
        && let Some(other) = pair_of(registries.slots.name(slot))
    {
        return Some(EquipShape::in_any([slot, registries.slots.expect(other)]));
    }
    let mut shape = EquipShape::in_slot(slot);
    for also in &d.also {
        shape = shape.and_claims(also.id());
    }
    Some(shape)
}

/// Marks an item with the definition it was made from: what a save writes
/// down, and what play reads a worn thing's `lends` through.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemKind(pub Id<ItemDef>);

/// Spawns `def` as an item entity carrying every component its definition
/// implies, placed nowhere, at level nought: what it is called and drawn
/// as, what it counts as, what it does at its moments and what that costs
/// it, and, for something worn, its shape, what it is worth to wear, its
/// armor, its resistances, what it gives its wearer's stats and whatever
/// it strikes or shoots with. A stackable item spawns as a stack of one;
/// the caller merges or grows it as it likes.
///
/// What a worn thing lends goes where the engine already reads a worn
/// thing, as far as it can: the armor and the resistances it lends join
/// its own, and the engine counts them for whoever wears it and for
/// nobody else. The rest, a status, sight in the dark and a light, is its
/// [`Lends`], which [`wearing`](crate::wearing) gives its wearer.
///
/// It is spawned under its own name and as it is: what the adventurer
/// knows of it, and what it is called for that, is
/// [`lore`](crate::lore)'s, which names everything the pass it appears.
pub fn spawn_item(commands: &mut Commands, armory: &Armory, def: Id<ItemDef>, registries: &Registries) -> Entity {
    spawn_item_at(commands, armory, def, 0, registries)
}

/// Spawns `def` as [`spawn_item`] does, at `level`, which a thing that
/// takes no levels ignores.
///
/// The level is the engine's [`Enchant`], and everything it changes is
/// fitted by the one function a change of level runs again, so the level
/// is applied once, here, and never on top of itself: a save keeps the
/// level, and a continued run's `+2` is this function run again. A thing
/// made below nought is cursed, and [`Stuck`]: it will not come off once
/// it is put on, until its level is raised to nought or the curse is
/// lifted.
pub fn spawn_item_at(commands: &mut Commands, armory: &Armory, def: Id<ItemDef>, level: i32, registries: &Registries) -> Entity {
    let d = armory.defs.get(def);
    let levelled = armory.takes_levels(def);
    let level = if levelled { level } else { 0 };
    let lent = &armory.lent[def.index()];
    let (r, g, b) = d.color;
    let mut e = commands.spawn((Item, ItemKind(def), Name::new(d.name.clone()), Glyph::new(d.glyph, Color::srgb(r, g, b)).on_layer(2)));
    if !d.tags.is_empty() {
        e.insert(Tagged(d.tags.iter().map(|t| t.id()).collect()));
    }
    // What it does at its moments, a potion's use and a flask's landing,
    // built once by the armory and shared by every copy; the engine lands
    // them, and spends what doing it costs.
    let triggers = armory.triggers.get(def.index());
    if let Some(triggers) = triggers.filter(|t| !t.0.is_empty()) {
        e.insert(triggers.clone());
    }
    if let Some(every) = triggers.and_then(Triggers::pulse_every) {
        e.insert(Pulse::every(every));
    }
    if let Some(c) = d.consumable {
        e.insert(Consumable::new(c.charges_at(level), c.when_empty));
    }
    if let Some(shape) = shape_of(d, registries) {
        e.insert(Wearable(shape));
    }
    // What a shot spends is the engine's to take from the wielder's pack,
    // and to put the shot by while the pack has none.
    if let Some(tag) = d.ammo {
        e.insert(Ammo { tag: tag.id() });
    }
    if levelled {
        e.insert(Enchant(Enchanted { level, affixes: Vec::new() }));
        if level < 0 {
            e.insert(Stuck);
        }
    }
    fit(&mut e, armory, def, level, None, registries.stats.id(crate::content::ACCURACY));
    if !d.resists.is_empty() || lent.resists != Resistances::new() {
        let mut resists = Resistances::new();
        for (kind, pct) in &d.resists {
            resists.add(kind.id(), *pct);
        }
        resists.plus(&lent.resists);
        e.insert(Resists(resists));
    }
    if let Some(lends) = Lends::of(lent) {
        e.insert(lends);
    }
    if let Some(light) = d.light {
        e.insert(light.source());
    }
    if d.stack {
        e.insert(Stack { key: def.index() as u64, count: 1 });
    }
    e.id()
}

/// Puts on `e` every component of `def` that its level decides, as it is
/// at `level`, and takes off any it no longer has: its armor, its blow,
/// its shot, its thrown strike, what it gives its wearer's stats and what
/// it is worth to a mind that wears things.
///
/// The one place a level reaches a thing, run when the thing is made and
/// again whenever its level changes, and every number worked out from the
/// definition rather than from the thing as it stands: so a level is never
/// added to a number that already carries one. A weapon's level adds to
/// every damage roll it makes and to its wielder's `accuracy`, the stat
/// `accuracy` names; armor's to its armor; and each `per_level` stat a
/// level's worth. An attack the thing has put by while it cannot make it,
/// `stowed`, is changed where it is put by rather than put back, so
/// raising a dry bow does not load it.
fn fit(e: &mut EntityCommands, armory: &Armory, def: Id<ItemDef>, level: i32, stowed: Option<Stowed>, accuracy: Option<StatId>) {
    let d = armory.defs.get(def);
    let lent = &armory.lent[def.index()];
    let plus = |dice: DiceRoll| DiceRoll { bonus: dice.bonus + DAMAGE_PER_LEVEL * level, ..dice };
    let melee = d.melee.map(|m| MeleeAttack { cost: d.cost, look: m.look, shape: m.shape, unaware: m.unaware, ..MeleeAttack::new(m.kind.id(), plus(m.roll)) });
    let ranged = d.ranged.map(|r| RangedAttack { cost: d.cost, look: r.look, effective: r.effective, ..RangedAttack::new(r.kind.id(), plus(r.roll), r.range) });
    match (melee, stowed) {
        (Some(attack), Some(Stowed::Melee(_))) => {
            e.insert(Stowed::Melee(attack));
        }
        (Some(attack), _) => {
            e.insert(attack);
        }
        (None, _) => {
            e.remove::<MeleeAttack>();
        }
    }
    match (ranged, stowed) {
        (Some(attack), Some(Stowed::Ranged(_))) => {
            e.insert(Stowed::Ranged(attack));
        }
        (Some(attack), _) => {
            e.insert(attack);
        }
        (None, _) => {
            e.remove::<RangedAttack>();
        }
    }
    if let Some(throw) = d.throw {
        e.insert(Throwable { effective: throw.effective, ..Throwable::new(throw.range, throw.strike.map(|(dice, kind)| (kind.id(), plus(dice)))) });
    }
    let own = if d.armor != 0 { d.armor + ARMOR_PER_LEVEL * level } else { 0 };
    let armor = own + lent.armor.unwrap_or(0);
    if armor != 0 {
        e.insert(Armor(armor));
    } else {
        e.remove::<Armor>();
    }
    let mut gives: Vec<(StatId, Op)> = d.bestows.iter().map(|(stat, n)| (stat.id(), Op::Add(*n))).collect();
    gives.extend(d.per_level.iter().map(|(stat, n)| (stat.id(), Op::Add(n * level))));
    if let Some(accuracy) = accuracy.filter(|_| melee.is_some() || ranged.is_some()) {
        gives.push((accuracy, Op::Add(ACCURACY_PER_LEVEL * level)));
    }
    gives.retain(|(_, op)| *op != Op::Add(0));
    if gives.is_empty() {
        e.remove::<Bestows>();
    } else {
        e.insert(Bestows(gives));
    }
    // Worth something to a mind that puts things on, so a goblin knows a
    // sword from a dagger without knowing what either is.
    let score = d.slot.and_then(|_| gear_score(melee.map(|m| m.dice), ranged.map(|r| r.dice), d.cost, armor));
    match score {
        Some(score) => {
            e.insert(GearScore(score));
        }
        None => {
            e.remove::<GearScore>();
        }
    }
}

/// Raises `item`'s level by `by`, or lowers it for a `by` below nought,
/// and fits it to the new level by the function that made it, so its
/// armor, its blows and what it gives its wearer are its definition's at
/// the new level and never the old numbers with a level added twice.
///
/// The most it holds grows with the level, and what it has left by as
/// much. A cursed thing whose level reaches nought comes unstuck. A thing
/// someone wears has their stats folded again, since the engine folds a
/// wearer's stats when what is worn changes and a level is not that.
/// Returns the new level, or `None` for a thing that takes no levels,
/// which is left as it was.
pub fn level_up(world: &mut World, item: Entity, by: i32) -> Option<i32> {
    let def = world.get::<ItemKind>(item)?.0;
    let level = world.get::<Enchant>(item).map_or(0, |e| e.level) + by;
    let changed = world.resource_scope(|world, armory: Mut<Armory>| {
        if !armory.takes_levels(def) {
            return false;
        }
        let accuracy = world.resource::<Registries>().stats.id(crate::content::ACCURACY);
        let stowed = world.get::<Stowed>(item).copied();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, world);
        let mut e = commands.entity(item);
        e.insert(Enchant(Enchanted { level, affixes: Vec::new() }));
        fit(&mut e, &armory, def, level, stowed, accuracy);
        queue.apply(world);
        if let (Some(c), Some(mut held)) = (armory.defs.get(def).consumable, world.get_mut::<Consumable>(item)) {
            let max = c.charges_at(level);
            let grown = i32::from(max) - i32::from(held.max);
            held.left = (i32::from(held.left) + grown).clamp(0, i32::from(max)) as u16;
            held.max = max;
        }
        true
    });
    if !changed {
        return None;
    }
    if level >= 0 {
        world.entity_mut(item).remove::<Stuck>();
    }
    let wearer = world.query::<(Entity, &Equipped)>().iter(world).find(|(_, worn)| worn.contains(item)).map(|(wearer, _)| wearer);
    if let Some(mut worn) = wearer.and_then(|wearer| world.get_mut::<Equipped>(wearer)) {
        worn.set_changed();
    }
    Some(level)
}

/// Spawns `def` at `level` as [`spawn_item_at`] does, as a thing found
/// rather than set out with: one whose level its wearer learns by wearing
/// it is [`Unfamiliar`](crate::lore::Unfamiliar), so it reads `+?` until
/// it has been worn long enough or is identified.
pub fn spawn_found(commands: &mut Commands, armory: &Armory, def: Id<ItemDef>, level: i32, registries: &Registries) -> Entity {
    let thing = spawn_item_at(commands, armory, def, level, registries);
    if armory.learned_by_wear(def) {
        commands.entity(thing).insert(crate::lore::Unfamiliar::default());
    }
    thing
}

/// Spawns `count` of `def`: one stack of `count` for a thing that stacks,
/// and `count` of it otherwise, placed nowhere.
pub fn spawn_items(commands: &mut Commands, armory: &Armory, def: Id<ItemDef>, count: u32, registries: &Registries) -> Vec<Entity> {
    if armory.defs.get(def).stack {
        let stack = spawn_item(commands, armory, def, registries);
        commands.entity(stack).insert(Stack { key: def.index() as u64, count: count.max(1) });
        return vec![stack];
    }
    (0..count).map(|_| spawn_item(commands, armory, def, registries)).collect()
}

/// The engine's loot is made here: what lies on a floor when it is first
/// entered, what a kill leaves, and what a chest holds.
impl ItemMaker for Armory {
    type Def = ItemDef;

    /// Makes `count` of `def`, and for a thing with a `pile`, `count` piles
    /// in one stack: each its own roll, times the band it was found at, so
    /// a pile on floor eight is worth eight of one on floor one.
    ///
    /// What is found is not yet known for what it is: each thing that takes
    /// levels at the level [`found_level`](Self::found_level) rolls, and a
    /// thing whose level its wearer learns by wearing it marked
    /// [`Unfamiliar`](crate::lore::Unfamiliar), so it reads `+?` until it
    /// has been worn long enough or is identified.
    fn make(
        &self,
        commands: &mut Commands,
        registries: &Registries,
        def: Id<ItemDef>,
        count: u32,
        from: Provenance,
        rng: &mut rand::rngs::StdRng,
    ) -> Vec<Entity> {
        let count = match self.defs.get(def).pile {
            Some(pile) => (0..count).map(|_| pile.roll(rng).max(1) as u32 * from.band.max(1) as u32).sum(),
            None => count,
        };
        if !self.takes_levels(def) {
            return spawn_items(commands, self, def, count, registries);
        }
        (0..count).map(|_| spawn_found(commands, self, def, self.found_level(def, from.band, registries, rng), registries)).collect()
    }

    fn id_of(&self, name: &str) -> Option<Id<ItemDef>> {
        self.defs.id(name)
    }

    fn table(&self) -> &LootTable<Id<ItemDef>> {
        &self.table
    }

    /// Nothing for the engine to lay loose: what lies about on a floor is
    /// [`finds`](crate::finds)'s, drawn a kind at a time and then a thing
    /// of that kind, where the engine's scatter draws every thing from the
    /// one table at once and would find potions only as often as the table
    /// happens to hold them. Chests and the dead are still the engine's.
    fn scatter(&self) -> ScatterRules {
        ScatterRules::new()
    }

    /// A floor's band is its number, since floor `n` is `MapId(n)`. A place
    /// no row of the table reaches, the town, is band nothing rather than
    /// its number, so a pile of gold made there is a first floor's and not
    /// a hundredth's.
    fn band(&self, area: LootArea) -> i32 {
        match area {
            LootArea::Place(map) if self.table.gaps(map.0 as i32..=map.0 as i32).is_empty() => map.0 as i32,
            LootArea::Place(_) | LootArea::Region(_) => 0,
        }
    }
}

#[cfg(test)]
mod play;

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;
    use rand::SeedableRng;

    use super::*;
    use crate::content::Library;

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// Spawns `names` through the library's armory into a bare world, the
    /// way a test reaches into a world without a system of its own.
    fn spawned(library: &Library, names: &[&str]) -> (World, Vec<Entity>) {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        let armory = &library.armory;
        let made = names.iter().map(|n| spawn_item(&mut commands, armory, armory.defs.expect(n), &library.registries)).collect();
        queue.apply(&mut world);
        (world, made)
    }

    /// A tripwire over the armory, by name, so a thing is added or dropped
    /// on purpose: the skeleton's, and what knowing, enchanting, strength
    /// and curses are carried by; and every floor has something to find.
    #[test]
    fn every_item_in_the_file_loads_and_every_floor_has_something_to_find() {
        let library = library();
        let armory = [
            "dagger",
            "sword",
            "broadsword",
            "axe",
            "war axe",
            "mace",
            "war hammer",
            "spear",
            "war pike",
            "bow",
            "arrow",
            "dart",
            "javelin",
            "leather armor",
            "scale mail",
            "chain mail",
            "banded mail",
            "splint mail",
            "plate armor",
            "shield",
            "helm",
            "potion of healing",
            "potion of mending",
            "potion of strength",
            "fire flask",
            "scroll of enchanting",
            "scroll of identify",
            "scroll of remove curse",
            "ring of regeneration",
            "ring of stealth",
            "ring of light",
            "gold coin",
            "lantern",
            "torch",
        ];
        for name in armory {
            assert!(library.armory.defs.id(name).is_some(), "no item called {name:?}");
        }
        assert_eq!(library.armory.defs.len(), armory.len(), "an item this list does not name");
        assert!(library.armory.table.gaps(1..=10).is_empty(), "a floor with nothing to find");
    }

    /// What each weapon's line says reaches the blow it strikes: its roll
    /// and kind, how long a blow takes, who else it strikes and what it
    /// makes of the unaware; a two-hander claims the off hand, and the bow
    /// shoots as far as its line says and spends an arrow a shot.
    #[test]
    fn a_weapons_numbers_reach_its_attack_and_a_two_hander_claims_the_off_hand() {
        let library = library();
        let (world, made) = spawned(&library, &["war hammer", "dagger", "bow", "axe", "war pike", "sword"]);
        let kind = |name: &str| library.registries.damage_kinds.expect(name);
        let plus = |n, sides, bonus| DiceRoll { bonus, ..DiceRoll::new(n, sides) };
        let hammer = world.get::<MeleeAttack>(made[0]).expect("the hammer swings");
        assert_eq!((hammer.dice, hammer.kind, hammer.cost), (plus(3, 6, 2), kind("crush"), Some(200)));
        assert_eq!(world.get::<Wearable>(made[0]).unwrap().0.slots().count(), 2, "main hand and off hand");
        let dagger = world.get::<MeleeAttack>(made[1]).expect("the dagger stabs");
        assert_eq!((dagger.dice, dagger.kind, dagger.cost, dagger.unaware), (plus(1, 4, 1), kind("pierce"), None, Some(500)));
        let bow = world.get::<RangedAttack>(made[2]).expect("the bow shoots");
        assert_eq!((bow.dice, bow.range, bow.effective_range()), (plus(1, 6, 1), 8, 5));
        assert_eq!(world.get::<Ammo>(made[2]).map(|a| a.tag), library.registries.tags.id("arrow"), "and spends an arrow a shot");
        let shapes: Vec<_> = made[3..].iter().map(|e| world.get::<MeleeAttack>(*e).and_then(|m| m.shape)).collect();
        assert_eq!(shapes, [Some(AttackShape::Sweep), Some(AttackShape::Pierce(1)), None], "an axe sweeps, a pike runs on, a sword strikes its target");
    }

    /// Spawns `name` at `level` through the library's armory into a bare
    /// world, beside the registries a level is fitted against.
    fn at_level(library: &Library, name: &str, level: i32) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(library.registries.clone());
        world.insert_resource(library.armory.clone());
        let mut queue = CommandQueue::default();
        let armory = &library.armory;
        let made = spawn_item_at(&mut Commands::new(&mut queue, &world), armory, armory.defs.expect(name), level, &library.registries);
        queue.apply(&mut world);
        (world, made)
    }

    /// What `thing` gives its wearer's stat `stat`, summed.
    fn gives(world: &World, library: &Library, thing: Entity, stat: &str) -> i32 {
        let stat = library.registries.stats.expect(stat);
        world.get::<Bestows>(thing).map_or(0, |b| b.0.iter().filter(|(s, _)| *s == stat).map(|(_, op)| if let Op::Add(n) = op { *n } else { 0 }).sum())
    }

    /// A weapon's level adds one to its every damage roll and five to its
    /// wielder's accuracy, armor's adds a point of armor, a ring's adds its
    /// stat for each level and takes it away for each below nought, and a
    /// thing below nought is stuck. A shield turns a tenth of what is aimed
    /// at its wearer whatever its level.
    #[test]
    fn a_level_adds_to_a_weapons_rolls_and_aim_to_armors_armor_and_to_a_rings_stat_and_one_below_nought_is_stuck() {
        let library = library();
        let (world, sword) = at_level(&library, "sword", 2);
        assert_eq!(world.get::<MeleeAttack>(sword).map(|m| m.dice), Some(DiceRoll { bonus: 3, ..DiceRoll::new(1, 7) }), "its own one, and two");
        assert_eq!((gives(&world, &library, sword, "accuracy"), world.get::<Enchant>(sword).map(|e| e.level)), (10, Some(2)));
        let (world, mail) = at_level(&library, "chain mail", -1);
        assert_eq!((world.get::<Armor>(mail).map(|a| a.0), world.get::<Stuck>(mail).is_some()), (Some(2), true), "a cursed coat");
        let (world, ring) = at_level(&library, "ring of stealth", 2);
        assert_eq!((gives(&world, &library, ring, "stealth"), world.get::<Stuck>(ring).is_some()), (20, false));
        let (world, ring) = at_level(&library, "ring of stealth", -3);
        assert_eq!((gives(&world, &library, ring, "stealth"), world.get::<Stuck>(ring).is_some()), (-30, true));
        let (world, shield) = at_level(&library, "shield", 1);
        assert_eq!((gives(&world, &library, shield, "evasion"), world.get::<Armor>(shield).map(|a| a.0)), (10, Some(2)));
        let (world, potion) = at_level(&library, "potion of healing", 3);
        assert!(world.get::<Enchant>(potion).is_none(), "a potion takes no levels, whatever it is made at");
    }

    /// A level is fitted from the definition every time, never on top of
    /// the numbers a thing already has: raised by one and by one again,
    /// chain mail is two better, not three, and a cursed ring raised to
    /// nought comes unstuck.
    #[test]
    fn a_level_raised_is_fitted_from_the_definition_and_never_twice() {
        let library = library();
        let (mut world, mail) = at_level(&library, "chain mail", 0);
        assert_eq!(level_up(&mut world, mail, 1), Some(1));
        assert_eq!(level_up(&mut world, mail, 1), Some(2));
        assert_eq!(world.get::<Armor>(mail).map(|a| a.0), Some(5), "three and two");
        let (mut world, ring) = at_level(&library, "ring of light", -1);
        assert_eq!(level_up(&mut world, ring, 1), Some(0));
        assert!(world.get::<Stuck>(ring).is_none(), "at nought it comes unstuck");
        assert_eq!(gives(&world, &library, ring, "light"), 0);
        let (mut world, potion) = at_level(&library, "potion of healing", 0);
        assert_eq!(level_up(&mut world, potion, 1), None, "a potion takes no levels");
    }

    /// A thing whose charges grow with its level holds one more for each
    /// level it is raised, and has one more left: a rod of the test's own,
    /// as a staff or a wand will be written.
    #[test]
    fn a_thing_whose_charges_grow_with_its_level_holds_and_has_one_more_for_each_level_raised() {
        let library = library();
        let rod = r#"[(name: "rod", glyph: '/', color: (0.5, 0.5, 0.5), tags: ["staff"], consumable: (charges: 1, when_empty: Kept, per_level: 1),
                      triggers: [(on: "use", effects: [(kind: "Mend", args: (roll: "1d4"))])])]"#;
        let defs = read(rod, &library.registries, &library.traits).expect("the rod reads");
        let armory = Armory::new(defs, LootTable::default(), &library.registries, &library.traits, &crate::content::effect_kinds(), &Moments::default())
            .expect("and is made");
        let mut world = World::new();
        world.insert_resource(library.registries.clone());
        world.insert_resource(armory.clone());
        let mut queue = CommandQueue::default();
        let rod = spawn_item_at(&mut Commands::new(&mut queue, &world), &armory, armory.defs.expect("rod"), 2, &library.registries);
        queue.apply(&mut world);
        assert_eq!(world.get::<Consumable>(rod).map(|c| (c.left, c.max)), Some((3, 3)), "one and two");
        world.get_mut::<Consumable>(rod).expect("charges").left = 1;
        assert_eq!(level_up(&mut world, rod, 1), Some(3));
        assert_eq!(world.get::<Consumable>(rod).map(|c| (c.left, c.max)), Some((2, 4)), "one more held, one more left");
    }

    /// A ring is found at one to three, or cursed at minus one to minus
    /// three about three times in ten, however deep, over a span of seeds;
    /// a potion takes no levels and draws nothing from the stream, so
    /// finding one deals the floor's loot as it was dealt before levels
    /// were found.
    #[test]
    fn a_ring_is_found_at_one_to_three_or_cursed_about_three_times_in_ten_and_a_potion_draws_nothing() {
        use rand::RngCore;
        let library = library();
        let (ring, potion) = (library.armory.defs.expect("ring of light"), library.armory.defs.expect("potion of healing"));
        let mut cursed = 0;
        for seed in 0..1000 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
            let level = library.armory.found_level(ring, 1 + (seed % 10) as i32, &library.registries, &mut rng);
            assert!(RING_LEVELS.contains(&level.abs()), "seed {seed}: {level}");
            cursed += u32::from(level < 0);
            let (mut drew, mut fresh) = (rand::rngs::StdRng::seed_from_u64(seed), rand::rngs::StdRng::seed_from_u64(seed));
            assert_eq!(library.armory.found_level(potion, 10, &library.registries, &mut drew), 0);
            assert_eq!(drew.next_u64(), fresh.next_u64(), "seed {seed}: a potion draws nothing");
        }
        assert!((240..=360).contains(&cursed), "{cursed} in a thousand");
    }

    /// How many of two thousand `name`s found on floor `floor` came at each
    /// level from minus three to plus two.
    fn found_on(library: &Library, name: &str, floor: i32) -> [u32; 6] {
        let def = library.armory.defs.expect(name);
        let mut at = [0; 6];
        for seed in 0..2000 {
            let level = library.armory.found_level(def, floor, &library.registries, &mut rand::rngs::StdRng::seed_from_u64(seed));
            assert!((-3..=2).contains(&level), "{name} on floor {floor}, seed {seed}: {level}");
            at[(level + 3) as usize] += 1;
        }
        at
    }

    /// A weapon or a piece of armor is cursed one time in five, at minus
    /// one to minus three, however deep it is found; otherwise it is at
    /// least one better ten times in a hundred for every floor down, and
    /// from floor four two better five times in a hundred for every floor,
    /// over a span of seeds: on floor one a tenth of what is not cursed is
    /// better, on floor four two fifths and half of that two better, and
    /// on floor ten all of it, half two better.
    #[test]
    fn a_weapon_or_armor_found_deeper_is_more_often_better_and_one_in_five_is_cursed_over_a_span_of_seeds() {
        let library = library();
        // Of the uncursed, the percent at +1 and at +2, each within five.
        let near = |got: u32, of: u32, pct: u32| (got * 100).abs_diff(of * pct) <= of * 5;
        for name in ["sword", "chain mail"] {
            for (floor, one, two) in [(1, 10, 0), (4, 20, 20), (10, 50, 50)] {
                let at = found_on(&library, name, floor);
                let cursed: u32 = at[..3].iter().sum();
                assert!((340..=460).contains(&cursed), "{name} on floor {floor}: {cursed} of 2000 cursed, {at:?}");
                assert!(at[..3].iter().all(|n| n * 3 * 10 >= cursed * 8), "{name} on floor {floor}: each of minus one to three alike, {at:?}");
                let whole = 2000 - cursed;
                assert!(near(at[4], whole, one) && near(at[5], whole, two), "{name} on floor {floor}: {at:?}");
            }
        }
    }

    /// A thing found that takes levels and is worn is not known to be any
    /// level yet; one found that is not worn, or that takes none, is.
    #[test]
    fn what_is_found_and_worn_is_unfamiliar_and_nothing_else_is() {
        let library = library();
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let from = Provenance { found: Found::Scatter, band: 1 };
        let made: Vec<Entity> = ["sword", "ring of light", "potion of healing"]
            .iter()
            .flat_map(|n| library.armory.make(&mut commands, &library.registries, library.armory.defs.expect(n), 1, from, &mut rng))
            .collect();
        queue.apply(&mut world);
        let unfamiliar: Vec<bool> = made.iter().map(|e| world.get::<crate::lore::Unfamiliar>(*e).is_some()).collect();
        assert_eq!(unfamiliar, [true, true, false]);
    }

    /// What asks strength of a wearer or gives one stats, on a thing never
    /// worn, a level's worth on a thing that takes no levels, and `either`
    /// on a slot of no pair are refused, naming the item.
    #[test]
    fn strength_and_stats_on_a_thing_never_worn_and_levels_on_one_that_takes_none_are_refused() {
        let library = library();
        let bad = r#"[
            (name: "heavy draught", glyph: '!', color: (0.5, 0.5, 0.5), tags: ["potion"], strength: 14),
            (name: "lucky stone", glyph: '*', color: (0.5, 0.5, 0.5), slot: "head", per_level: [("stealth", 5)]),
            (name: "odd helm", glyph: '^', color: (0.5, 0.5, 0.5), slot: "head", either: true, tags: ["armor"], armor: 1),
        ]"#;
        let Err(ContentError::Invalid(errors)) = read(bad, &library.registries, &library.traits) else { panic!("it read") };
        assert_eq!(
            errors,
            [
                "heavy draught: asks strength of, or gives stats to, a wearer of a thing that is never worn",
                "lucky stone: gives something by its level, on a thing that takes no levels",
                "odd helm: either, on the head slot, which is one of no pair",
            ]
        );
    }

    /// A pile of gold is its roll times the floor it was found on, so a
    /// pile on floor eight is worth eight of one on floor one, over a span
    /// of seeds.
    #[test]
    fn a_pile_of_gold_is_its_roll_times_the_floor_it_lies_on_over_a_span_of_seeds() {
        let library = library();
        let gold = library.armory.defs.expect("gold coin");
        for seed in 0..40 {
            for floor in [1, 8] {
                let mut world = World::new();
                let mut queue = CommandQueue::default();
                let mut commands = Commands::new(&mut queue, &world);
                let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
                let from = Provenance { found: Found::Scatter, band: floor };
                let made = library.armory.make(&mut commands, &library.registries, gold, 1, from, &mut rng);
                queue.apply(&mut world);
                assert_eq!(made.len(), 1, "one pile, one stack");
                let coins = world.get::<Stack>(made[0]).expect("gold stacks").count as i32;
                assert!((3 * floor..=24 * floor).contains(&coins), "seed {seed}, floor {floor}: {coins}");
                assert_eq!(coins % floor, 0, "seed {seed}, floor {floor}: {coins} is not a pile times the floor");
            }
        }
    }

    /// A floor is drawn at its own number, and the town, which no row
    /// reaches, at nothing: a band of a hundred would make every pile of
    /// gold that turned up there a hundred floors deep.
    #[test]
    fn a_floor_is_drawn_at_its_number_and_the_town_at_nothing() {
        let library = library();
        let band = |map: u32| library.armory.band(LootArea::Place(MapId(map)));
        assert_eq!((1..=10).map(band).collect::<Vec<_>>(), (1..=10).collect::<Vec<_>>());
        assert_eq!(band(100), 0, "the town");
    }

    /// `lends` names traits, and a name that is no trait, a thing that is
    /// never worn lending anything, and two lent traits that set one value
    /// are all refused at load, naming the item.
    #[test]
    fn an_item_lends_traits_by_name_and_only_a_worn_thing_lends_any() {
        let library = library();
        let (registries, traits) = (&library.registries, &library.traits);
        let helm = r#"[(name: "owl helm", glyph: '^', color: (0.5, 0.6, 0.5), slot: "head", tags: ["armor"], armor: 1, lends: ["sees in the dark"])]"#;
        let defs = read(helm, registries, traits).expect("a helm lending a trait");
        assert_eq!(defs.get(defs.expect("owl helm")).lends.iter().map(|t| t.id()).collect::<Vec<_>>(), vec![traits.expect("sees in the dark")]);

        let bad = r#"[
            (name: "odd helm", glyph: '^', color: (0.5, 0.6, 0.5), slot: "head", lends: ["sees in the drak"]),
            (name: "loose charm", glyph: '"', color: (0.5, 0.6, 0.5), lends: ["swift"]),
            (name: "twisted helm", glyph: '^', color: (0.5, 0.6, 0.5), slot: "head", lends: ["thick-hided", "armored"]),
        ]"#;
        let Err(ContentError::Invalid(errors)) = read(bad, registries, traits) else { panic!("bad lends loaded") };
        assert_eq!(errors.len(), 1, "an unknown name is reported before anything is checked: {errors:?}");
        assert_eq!(errors[0], "odd helm: unknown trait \"sees in the drak\"");
        let Err(ContentError::Invalid(errors)) = read(&bad.replace("drak", "dark"), registries, traits) else { panic!("bad lends loaded") };
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors[0].starts_with("loose charm: "), "{errors:?}");
        assert_eq!(errors[1], "twisted helm: `armor` is set by both `thick-hided` and `armored`");
    }

    /// A lantern sheds its light where it lies and from whoever carries
    /// it, which is the engine's to do once the thing has a light at all.
    #[test]
    fn a_lantern_and_a_torch_shed_the_light_their_lines_write() {
        let library = library();
        let (world, made) = spawned(&library, &["lantern", "torch"]);
        for (e, name) in made.iter().zip(["lantern", "torch"]) {
            let def = library.armory.defs.get(library.armory.defs.expect(name));
            let light = def.light.expect("a light");
            assert_eq!(world.get::<LightSource>(*e), Some(&light.source()), "{name}");
        }
        assert!(world.get::<Throwable>(made[1]).is_some(), "a torch can be thrown into the dark");
    }

    /// A potion is used up by its use, and a fire flask flies and bursts
    /// where it lands: what each does is its trigger, built once.
    #[test]
    fn a_potion_is_spent_by_its_use_and_a_flask_flies_and_bursts_where_it_lands() {
        let library = library();
        let (world, made) = spawned(&library, &["potion of healing", "fire flask"]);
        for e in &made {
            assert!(world.get::<Triggers>(*e).is_some_and(|t| !t.0.is_empty()), "it does something");
            assert!(world.get::<Consumable>(*e).is_some(), "and is spent doing it");
        }
        assert!(world.get::<Throwable>(made[1]).is_some(), "a flask is thrown");
    }

    /// What a thing is worth to a mind that wears things: a sword more than
    /// a dagger, a war hammer more than a sword, plate more than leather,
    /// and a light nothing at all, so no goblin takes up a lantern for its
    /// worth.
    #[test]
    fn a_sword_is_worth_more_to_wear_than_a_dagger_and_a_lantern_nothing() {
        let library = library();
        let names = ["dagger", "sword", "war hammer", "leather armor", "plate armor", "lantern", "torch"];
        let (world, made) = spawned(&library, &names);
        let worth: Vec<Option<i32>> = made.iter().map(|e| world.get::<GearScore>(*e).map(|g| g.0)).collect();
        let [dagger, sword, hammer, leather, plate, lantern, torch] = worth[..] else { unreachable!() };
        assert!(dagger < sword && sword < hammer, "{worth:?}");
        assert!(leather < plate, "{worth:?}");
        assert_eq!((lantern, torch), (None, None), "{worth:?}");
    }

    /// A worn thing gives what it is or carries, and lending anything else,
    /// a pace or the wits a mind thinks with, is refused naming the trait and
    /// what it sets; so is a thing lending a light beside its own.
    #[test]
    fn a_thing_lending_what_a_worn_thing_cannot_give_is_refused() {
        let library = library();
        let (registries, traits) = (&library.registries, &library.traits);
        let bad = r#"#![enable(implicit_some)]
        [
            (name: "boots of haste", glyph: '[', color: (0.5, 0.5, 0.5), slot: "body", lends: ["swift"]),
            (name: "cap of wits", glyph: '^', color: (0.5, 0.5, 0.5), slot: "head", lends: ["sapient"]),
            (name: "glowing lantern", glyph: '(', color: (0.5, 0.5, 0.5), slot: "light", light: (radius: 3, color: (1.0, 1.0, 1.0)), lends: ["fungal glow"]),
        ]"#;
        let Err(ContentError::Invalid(errors)) = read(bad, registries, traits) else { panic!("it read") };
        assert_eq!(
            errors,
            [
                "boots of haste: lends `swift`, whose `speed` a worn thing cannot give",
                "cap of wits: lends `sapient`, whose `wits` a worn thing cannot give",
                "glowing lantern: lends a light, and sheds one of its own: a thing has the one light",
            ]
        );
    }

    /// What a ring lends that the engine reads off a worn thing joins the
    /// ring's own, and the rest is what the ring lends its wearer.
    #[test]
    fn a_things_lent_armor_and_resistances_are_its_own_and_the_rest_it_lends() {
        let library = library();
        let (registries, traits) = (&library.registries, &library.traits);
        let ring = r#"#![enable(implicit_some)]
        [(name: "stone ring", glyph: '=', color: (0.5, 0.5, 0.5), slot: "off hand", armor: 1, resists: [("cold", 10)],
          lends: ["armored", "fire-blooded", "regenerates", "sees in the dark"])]"#;
        let defs = read(ring, registries, traits).expect("the ring reads");
        let armory = Armory::new(defs, LootTable::default(), registries, traits, &crate::content::effect_kinds(), &Moments::default()).expect("and is made");
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let made = spawn_item(&mut Commands::new(&mut queue, &world), &armory, armory.defs.expect("stone ring"), registries);
        queue.apply(&mut world);
        let kind = |name: &str| registries.damage_kinds.expect(name);
        assert_eq!(world.get::<Armor>(made).map(|a| a.0), Some(5), "its own one and the four `armored` lends");
        let resists = &world.get::<Resists>(made).expect("resistances").0;
        assert_eq!((resists.get(kind("fire")), resists.get(kind("cold"))), (100, -40), "fire-blooded beside its own ten against cold");
        let lends = world.get::<Lends>(made).expect("what it lends a wearer");
        assert_eq!((lends.statuses.clone(), lends.dark_sight, lends.light), (vec![registries.statuses.expect("regenerating")], Some(6), None));
    }
}
