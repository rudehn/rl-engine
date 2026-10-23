//! The armory: the weapons, armor, staffs, wands, rings, potions, scrolls,
//! lights and gold of the Undermere, loaded once from `items.ron` into the
//! [`Armory`] resource, found where `item_spawns.ron` says, and spawned as
//! items an actor can find, carry, wear, zap and use.
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
//! stat. What sets one staff apart is the bolt its use is aimed as, what
//! the bolt lands, and what its level makes of it: the charges it holds,
//! how soon a spent one comes back, and how much harder a deeper staff's
//! bolt lands. A wand is a staff that never refills.
//!
//! An [`ItemDef`] is Undermere's own, its fields typed with the engine's
//! authored parts wherever the engine plays what they say, a blow, a
//! wear, charges, a level, so the armory builds each line's
//! [`ItemParts`] once and the engine spawns a thing from them; besides
//! those, a price, the strength a thing asks, where it is found and
//! `lends`: the traits a worn thing gives its wearer, the same traits a
//! monster is written as, so a ring of the salamander and a salamander are
//! fire-blooded for the same reason. The names resolve here, at load, and
//! are folded once into what each thing lends, all of it the thing's own
//! parts: its armor and its resistances join the thing's own, which the
//! engine counts for whoever wears it, and the statuses it holds, the
//! sight in the dark it lends and a light lit while it is worn are the
//! parts the engine lends a wearer, as [`wearing`](crate::wearing) says.
//!
//! What a level adds is a [`Level`] part, written by each line over the
//! default its tag has here, [`level_defaults`]: a weapon a damage and five
//! accuracy, armor a point of armor, a staff or a wand a charge. The engine
//! applies one linearly and nothing more; that a thing below nought is
//! cursed and will not come off, how a level is found, and how a staff's
//! level quickens its refill are Undermere's, here and in
//! [`staffs`](crate::staffs).

use std::sync::Arc;

use atglyph::prelude::*;
use atglyph::rules::parts::{self, Level};
use bevy::prelude::*;
use serde::Deserialize;

use crate::lore::{CurseShown, Lore, Unfamiliar};
use crate::traits::{Folded, TraitDef};

/// The items file, compiled in so the binary runs from anywhere.
const ITEMS_RON: &str = include_str!("../assets/items.ron");
/// Where each thing is found, compiled in beside it.
const ITEM_SPAWNS_RON: &str = include_str!("../assets/item_spawns.ron");

/// One kind of item, as authored in `items.ron`.
///
/// Unknown fields are refused, so a field Foundry has and Undermere does
/// not, a `heat` or a `dark_sight`, fails the load naming it rather than
/// loading a thing that quietly does less than its line says.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemDef {
    /// Unique; spawn tables and drops refer to it. One of it, singular,
    /// since the engine says a stack as "12 arrows".
    pub name: String,
    /// Whether the name is said with no `a`, a stuff: `chain mail`. The
    /// engine's narrator reads it as [`MassNoun`] on the thing.
    #[serde(default)]
    pub mass: bool,
    /// One character.
    pub glyph: char,
    /// `(r, g, b)` in `0..=1`.
    pub color: (f32, f32, f32),
    /// The price of one at level nought, from the spec's tables; absent on
    /// gold, which is not for sale. [`keep_worth`] turns it into the
    /// engine's `Worth`, current on the thing rather than saved.
    #[serde(default)]
    pub price: Option<u32>,
    /// Where it is worn, if it is worn at all: a one-handed weapon either
    /// hand, a ring either hand's finger, a two-hander both hands.
    #[serde(default)]
    pub wear: Option<parts::Wear>,
    /// What it counts as, for a chest or a drop that asks by tag, and what
    /// its level adds when its line writes nothing of its own.
    #[serde(default)]
    pub tags: Vec<NameRef<TagDef>>,
    /// What the thing holds, landed by any of its triggers that names no
    /// list of its own.
    #[serde(default)]
    pub effects: Vec<EffectSpec>,
    /// What it does at its moments: used, thrown and landed.
    #[serde(default)]
    pub triggers: Vec<TriggerSpec>,
    /// What a use or a landing costs it, and, for a staff, how long a
    /// charge takes to come back at level one; absent, nothing.
    #[serde(default)]
    pub consumable: Option<parts::Charges>,
    /// Flat armor while worn.
    #[serde(default)]
    pub armor: i32,
    /// Percent removed per damage kind, while worn.
    #[serde(default)]
    pub resists: Vec<(NameRef<DamageKind>, i32)>,
    /// The blow it deals, while wielded: an axe's sweep, a spear's thrust
    /// through, a dagger's own percent on the unaware.
    #[serde(default)]
    pub melee: Option<parts::Melee>,
    /// The shot it fires, while wielded.
    #[serde(default)]
    pub ranged: Option<parts::Ranged>,
    /// The tag of what one shot spends from its wielder's pack, for a
    /// weapon that shoots something it has to be fed: a bow's arrows.
    #[serde(default)]
    pub ammo: Option<NameRef<TagDef>>,
    /// How far it flies when thrown, and what it strikes; absent, it
    /// cannot be thrown.
    #[serde(default)]
    pub throw: Option<parts::ThrowDef>,
    /// What one blow or shot with it costs, in hundredths of a step;
    /// absent, an ordinary turn.
    #[serde(default)]
    pub cost: Option<u32>,
    /// The light it sheds, where it lies and from whoever carries it.
    #[serde(default)]
    pub light: Option<Glow>,
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
    /// The most each named stat of its wearer's comes to while it is
    /// worn, whatever else is worn or held: the Choirmaster's Staff's
    /// regeneration held at nought. A cap rather than a large amount
    /// taken away, since no amount stays past whatever a ring and armor of
    /// regeneration might add up to, and a cut of mending only ever
    /// rounded a single point's heal away.
    #[serde(default)]
    pub caps: Vec<(NameRef<StatDef>, i32)>,
    /// What each of its levels adds, written over what its tag's level adds
    /// in [`level_defaults`]: a ring's whole worth, a staff's power. Only
    /// on a thing that takes levels.
    #[serde(default)]
    pub level: Option<Level>,
    /// The levels a thing of it is found at, drawn evenly from the first to
    /// the second: how many charges a wand turns up with. Absent, its
    /// kind's own rule, [`found_level`](Armory::found_level)'s.
    #[serde(default)]
    pub found_at: Option<(i32, i32)>,
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
    /// Each definition's parts, built once and shared by every item spawned
    /// from it: its triggers, its wear, everything it lends folded into
    /// them, and its level, its tag's default written over by its line.
    parts: Vec<Arc<ItemParts>>,
    /// Each definition's plain level, read once off its tags and its
    /// `found_at`: `None` for one that takes no levels.
    plain: Vec<Option<i32>>,
    /// Whether each definition is an artifact, read once off its tags.
    artifacts: Vec<bool>,
    /// Whether each definition's use, aimed at somewhere, shows what it is
    /// with nobody under it, read once off its use trigger.
    shows_unstruck: Vec<bool>,
    /// Whether each definition, thrown, shows what it is where it lands
    /// with nobody there, read once off its land triggers.
    lands_seen: Vec<bool>,
}

impl Armory {
    /// Loads `items.ron` and `item_spawns.ron` against `registries` and
    /// the `traits` an item lends, and builds every item's triggers
    /// against the effect `kinds` and `moments` registered for the run, so
    /// a potion naming an effect nobody registered is caught while the file
    /// is read rather than by a potion that quietly does nothing. Panics
    /// naming the file and every problem in it.
    pub fn load(registries: &Registries, traits: &Registry<TraitDef>, kinds: &EffectKinds, moments: &Moments) -> Self {
        let defs = load_defs(registries, traits);
        let names = registries.names();
        let tags = |d: &ItemDef| d.tags.iter().map(|t| t.id()).collect();
        let table = atglyph::rules::loot::load(ITEM_SPAWNS_RON, &names.clone().with("item", &defs), &defs, tags)
            .unwrap_or_else(|e| panic!("assets/item_spawns.ron: {e}"));
        Self::new(defs, table, registries, traits, kinds, moments).unwrap_or_else(|e| panic!("assets/items.ron: {e}"))
    }

    /// An armory of `defs`, found where `table` says, each definition's
    /// parts built: its triggers against the effect `kinds` and `moments`,
    /// what it lends folded through `traits`, and its level over its tag's.
    /// Every trigger that cannot be built, every lend that does not fold
    /// and every part the engine would never play is reported at once, each
    /// under its item.
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
        let defaults = level_defaults(registries);
        let (mut parts, mut errors) = (Vec::new(), Vec::new());
        let plain = defs.iter().map(|(_, d)| plain_level(d, registries)).collect();
        let artifacts = defs.iter().map(|(_, d)| d.tags.iter().any(|t| registries.tags.name(t.id()) == "artifact")).collect();
        let shows_unstruck = defs.iter().map(|(_, d)| shows_unstruck(d)).collect();
        let lands_seen = defs.iter().map(|(_, d)| lands_seen(d)).collect();
        for (id, d) in defs.iter() {
            let mut wrong = Vec::new();
            let triggers = Triggers::build(&d.triggers, &d.effects, moments, kinds, &names).unwrap_or_else(|mine| {
                wrong.extend(mine);
                Triggers::default()
            });
            let lends: Vec<_> = d.lends.iter().map(|t| t.id()).collect();
            let folded = crate::traits::fold(&lends, traits).unwrap_or_else(|clash| {
                wrong.push(clash.to_string());
                Folded::default()
            });
            let made = parts_of(d, id, &folded, triggers, &defaults, registries).unwrap_or_else(|mine| {
                wrong.extend(mine);
                ItemParts::default()
            });
            wrong.extend(made.check());
            errors.extend(wrong.into_iter().map(|e| format!("{}: {e}", d.name)));
            parts.push(Arc::new(made));
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Ok(Self { defs, table, parts, plain, artifacts, shows_unstruck, lands_seen })
    }

    /// The parts a thing of `def` is made of: what the engine spawns it
    /// from and fits its level from.
    pub fn parts(&self, def: Id<ItemDef>) -> &Arc<ItemParts> {
        &self.parts[def.index()]
    }

    /// What one level of `def` adds, its tag's written over by its line, or
    /// `None` for a thing that takes no levels.
    pub fn level_of(&self, def: Id<ItemDef>) -> Option<&Level> {
        self.parts(def).level.as_ref()
    }

    /// Whether a thing of `def`, thrown, shows what it is where it lands
    /// with nobody there: a burst seen where it breaks, or the ground
    /// changed, a fire lit or a cloud given off. A draught that only does
    /// something to whoever it breaks on shows nothing on bare floor.
    pub fn lands_seen(&self, def: Id<ItemDef>) -> bool {
        self.lands_seen.get(def.index()).copied().unwrap_or(false)
    }

    /// Whether `def`'s use, aimed at somewhere and landing on nobody, still
    /// shows what it is: one that moves its user to where it is aimed, or
    /// one that changes the ground it lands on, a bolt of fire burning.
    /// What a zap that strikes nobody is known by, or not.
    pub fn shows_unstruck(&self, def: Id<ItemDef>) -> bool {
        self.shows_unstruck.get(def.index()).copied().unwrap_or(false)
    }

    /// Hundredths of a step a spent charge of a `def` at `level` takes to
    /// come back, before its carrier's pace: what its line writes, divided
    /// by the level above it, a level below one counted as one, so a staff
    /// two better refills twice as fast. `None` for a thing that does not
    /// refill.
    ///
    /// Undermere's rule rather than a level's, since it divides where a
    /// level only adds: the engine spawns a thing at its line's pace and
    /// [`spawn_item_at`], [`level_up`] and [`staffs`](crate::staffs) keep it
    /// at this one.
    pub fn recharge_at(&self, def: Id<ItemDef>, level: i32) -> Option<u32> {
        self.defs.get(def).consumable.and_then(|c| c.recharge).map(|every| every / level.max(1) as u32)
    }

    /// The armor a thing of `def` gives at `level`: its own and whatever it
    /// lends, and a level's worth more for each level when its level adds
    /// armor. The one rule [`Armor`] is written by when a thing is made or
    /// its level changes, and what a thing whose level is not known yet
    /// seems to give, at nought.
    pub fn armor_at(&self, def: Id<ItemDef>, level: i32) -> i32 {
        self.parts(def).armor_at(level)
    }

    /// What a thing of `def` at `level` is worth to a mind that puts things
    /// on, the engine's `GearScore`, by `gear_score` at that level's blow,
    /// shot and armor; `None` for a thing never worn or worth nothing worn.
    /// The one rule a thing's score is written by when it is made or its
    /// level changes, and what a thing whose level is not known yet seems
    /// worth, at nought.
    pub fn score_at(&self, def: Id<ItemDef>, level: i32) -> Option<i32> {
        let d = self.defs.get(def);
        let attacks = self.parts(def).attacks_at(level);
        d.wear.as_ref().and_then(|_| gear_score(attacks.melee.map(|m| m.roll), attacks.ranged.map(|r| r.roll), d.cost, self.armor_at(def, level)))
    }

    /// Whether a thing of `def`'s level moves its wearer's armor: a level
    /// that adds armor, or one that adds to the stat `armor` names. What
    /// the vitals' mark asks of a thing whose kind is known and whose level
    /// is not, and a fact of the kind alone.
    pub fn level_moves_armor(&self, def: Id<ItemDef>, armor: Option<StatId>) -> bool {
        self.armor_at(def, 1) != self.armor_at(def, 0) || self.level_of(def).is_some_and(|l| l.stats.iter().any(|(stat, _)| Some(stat.id()) == armor))
    }

    /// What a thing of `def` at `level` gives its wearer's stats: its own,
    /// a level's worth of each stat its level adds to, a weapon's accuracy
    /// among them, and the most each stat it caps comes to; nothing that
    /// comes to nought. The one rule [`Bestows`] is written by when a thing
    /// is made or its level changes, and what a thing whose level is not
    /// known yet seems to give, at nought.
    pub fn gives_at(&self, def: Id<ItemDef>, level: i32) -> Vec<(StatId, Op)> {
        self.parts(def).bestows_at(level)
    }

    /// What one level of `def` adds to the stat `stat`, nought for one its
    /// level does not name: what a weapon's level is worth in accuracy.
    pub fn per_level(&self, def: Id<ItemDef>, stat: Option<StatId>) -> i32 {
        self.level_of(def).and_then(|l| l.stats.iter().find(|(s, _)| Some(s.id()) == stat)).map_or(0, |(_, n)| *n)
    }

    /// Whether `def` takes levels: a weapon, armor, a ring, a staff or a
    /// wand, and nothing that stacks.
    pub fn takes_levels(&self, def: Id<ItemDef>) -> bool {
        self.level_of(def).is_some()
    }

    /// The level a thing of `def` is at plain, or `None` for a thing that
    /// takes no levels: the least a found one of its kind is at uncursed,
    /// so a ring sold does something and a staff sold has charges. Never
    /// below nought.
    ///
    /// The one definition both a shelf's stocking and a thing's worth read:
    /// a ware is stocked at it and better, and a level is worth something
    /// only above it, so a plain staff costs its `items.ron` price as a
    /// plain sword does.
    pub fn plain_level(&self, def: Id<ItemDef>) -> Option<i32> {
        self.plain.get(def.index()).copied().flatten()
    }

    /// What one of `def` at `level` is worth with its kind and its level
    /// both told, as a keeper names a ware and a hireling names its kit:
    /// `None` for a def with no price, gold, or a level below nought, which
    /// nobody pays for; else its price, with [`WORTH_PER_LEVEL`] a level
    /// above its kind's [plain level](Self::plain_level). The arithmetic
    /// of a thing's `Worth`, with what the adventurer does not know of it
    /// left to [`keep_worth`]; a hireling's fee reads it without a `Lore`,
    /// since what a hireling says it carries is told, not learned.
    pub fn told_worth(&self, def: Id<ItemDef>, level: i32) -> Option<u32> {
        let price = self.defs.get(def).price?;
        let plain = self.plain_level(def).unwrap_or(0);
        if level < 0 {
            return None;
        }
        Some(price + WORTH_PER_LEVEL * (level - plain).max(0) as u32)
    }

    /// Whether `def` is an artifact, a mini-boss's own: bought by the curio
    /// and never sold.
    pub fn is_artifact(&self, def: Id<ItemDef>) -> bool {
        self.artifacts.get(def.index()).copied().unwrap_or(false)
    }

    /// Whether `def`'s level is learned by wearing it, as well as by being
    /// told: a thing that takes levels and is worn. A staff or a wand is
    /// known as a whole once its kind is, since its level is its charges,
    /// which are seen being spent.
    pub fn learned_by_wear(&self, def: Id<ItemDef>) -> bool {
        self.takes_levels(def) && self.defs.get(def).wear.is_some()
    }

    /// The level a thing of `def` found on floor `band` is at: a ring at
    /// one to three, or, [`CURSED_RINGS_IN_TEN`] times in ten, cursed, at
    /// minus one to minus three, however deep; a weapon or a piece of armor
    /// cursed [`CURSED_GEAR_IN_FIVE`] times in five, at one of
    /// [`CURSED_GEAR_LEVELS`] below nought, and otherwise the more often
    /// better the deeper it lies, as [`better_by`] says; a thing whose line
    /// says where it is found, a wand, there, and a staff at [`STAFF_LEVEL`]
    /// and better deeper as a weapon is; anything else at nought.
    ///
    /// Drawn from `rng`, the loot's own stream, and only for a thing whose
    /// level is rolled, so finding a potion draws nothing.
    pub fn found_level(&self, def: Id<ItemDef>, band: i32, registries: &Registries, rng: &mut impl atglyph::rand::Rng) -> i32 {
        let tagged = |name: &str| self.defs.get(def).tags.iter().any(|t| Some(t.id()) == registries.tags.id(name));
        if !self.takes_levels(def) {
            return 0;
        }
        if let Some((low, high)) = self.defs.get(def).found_at {
            return rng.random_range(low..=high);
        }
        if tagged("staff") {
            return STAFF_LEVEL + better_by(band, rng.random_range(0..100));
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

/// What one level adds to what a fresh staff or wand holds: a staff two
/// better zaps twice more before it needs time, and a scroll of
/// enchanting puts a charge into a wand.
pub const CHARGES_PER_LEVEL: i32 = 1;

/// What one level the adventurer knows adds to a thing's `Worth`, above
/// its def's own price at its plain level.
pub const WORTH_PER_LEVEL: u32 = 100;

/// The level a staff is found at on the first floors, its power and its
/// charges both: two, as Brogue's are, so the first staff found is worth
/// zapping twice before it needs time.
pub const STAFF_LEVEL: i32 = 2;

/// The effects that change the cells they land on whoever stands there,
/// by the names content writes them under: a zap or a throw landing one on
/// nobody is still seen to do something.
pub const GROUND_EFFECTS: [&str; 3] = ["Ignite", "Emit", "Darkness"];

/// Whether `d`'s use, when it is aimed, shows what it is with nobody under
/// it: aimed at the ground, which is a use that moves its user there, or
/// landing an effect that changes the ground.
fn shows_unstruck(d: &ItemDef) -> bool {
    d.triggers.iter().filter(|t| t.on == "use").any(|t| {
        let at_ground = t.targeting.is_some_and(|targeting| targeting.aim == atglyph::rules::ability::Aim::Ground);
        let effects = t.effects.as_deref().unwrap_or(&d.effects);
        at_ground || effects.iter().any(|e| GROUND_EFFECTS.contains(&e.kind.as_str()))
    })
}

/// Whether `d`, thrown, shows what it is where it lands with nobody there:
/// a land trigger seen to burst, or landing an effect that changes the
/// ground.
fn lands_seen(d: &ItemDef) -> bool {
    d.triggers.iter().filter(|t| t.on == "land").any(|t| {
        let effects = t.effects.as_deref().unwrap_or(&d.effects);
        t.look.is_some() || effects.iter().any(|e| GROUND_EFFECTS.contains(&e.kind.as_str()))
    })
}

/// Whether `d` takes levels: it carries one of the [`LEVELLED_TAGS`] and
/// does not stack, since every copy in a stack is one thing to the bag and
/// a level is one thing's own.
fn takes_levels(d: &ItemDef, registries: &Registries) -> bool {
    !d.stack && d.tags.iter().any(|t| LEVELLED_TAGS.contains(&registries.tags.name(t.id())))
}

/// What a level adds to a thing of each of the [`LEVELLED_TAGS`] whose
/// line writes nothing of its own, by tag: a weapon [`DAMAGE_PER_LEVEL`]
/// on every damage roll and [`ACCURACY_PER_LEVEL`] to its wielder's
/// accuracy, armor [`ARMOR_PER_LEVEL`], a staff and a wand
/// [`CHARGES_PER_LEVEL`]; and how far below nought one goes, a weapon, a
/// piece of armor and a ring as far as one is found cursed, a staff and a
/// wand never.
///
/// Undermere's rule for what a `+2` is, written once here so `items.ron`
/// does not repeat it on every line; a line's own `level` is written over
/// it. A ring's level adds nothing by default: each ring says what its
/// level is worth.
pub fn level_defaults(registries: &Registries) -> Vec<(TagId, Level)> {
    let accuracy = registries.stats.id(crate::content::ACCURACY);
    let cursed = |worst: i32| Level { min: Some(-worst), ..Level::default() };
    let gear = *CURSED_GEAR_LEVELS.end();
    let rows = [
        ("weapon", Level { damage: Some(DAMAGE_PER_LEVEL), stats: accuracy.map(|a| (a.into(), ACCURACY_PER_LEVEL)).into_iter().collect(), ..cursed(gear) }),
        ("armor", Level { armor: Some(ARMOR_PER_LEVEL), ..cursed(gear) }),
        ("ring", cursed(*RING_LEVELS.end())),
        ("staff", Level { charges: Some(CHARGES_PER_LEVEL), ..Level::default() }),
        ("wand", Level { charges: Some(CHARGES_PER_LEVEL), ..Level::default() }),
    ];
    rows.into_iter().filter_map(|(tag, level)| registries.tags.id(tag).map(|tag| (tag, level))).collect()
}

/// The parts a thing of `d`, the definition `id`, is made of: its own,
/// with what `lent` folded into them, armor and resistances added to its
/// own, the statuses it holds, its sight in the dark and a light lit
/// while worn, its
/// `triggers` as the armory built them, and for a thing that takes levels
/// its line's level written over its tags' `defaults`. Every reason it
/// cannot be, its wear on a slot of no pair among them.
fn parts_of(
    d: &ItemDef,
    id: Id<ItemDef>,
    lent: &Folded,
    triggers: Triggers,
    defaults: &[(TagId, Level)],
    registries: &Registries,
) -> Result<ItemParts, Vec<String>> {
    let wear = d.wear.as_ref().map(|w| w.shape(&registries.slots)).transpose().map_err(|e| vec![e])?;
    let level = takes_levels(d, registries).then(|| {
        let tagged = defaults.iter().filter(|(tag, _)| d.tags.iter().any(|t| t.id() == *tag));
        let default = tagged.fold(Level::default(), |under, (_, over)| over.over(&under));
        d.level.as_ref().map_or(default.clone(), |own| own.over(&default))
    });
    let mut resists = parts::resistances(&d.resists);
    resists.plus(&lent.resists);
    let stats = |list: &[(NameRef<StatDef>, i32)]| list.iter().map(|(stat, n)| (stat.id(), *n)).collect();
    Ok(ItemParts {
        wear,
        tags: d.tags.iter().map(|t| t.id()).collect(),
        triggers,
        charges: d.consumable,
        armor: d.armor + lent.armor.unwrap_or(0),
        resists,
        bestows: stats(&d.bestows),
        caps: stats(&d.caps),
        attacks: Attacks { melee: d.melee, ranged: d.ranged, throw: d.throw, cost: d.cost },
        ammo: d.ammo.map(|t| t.id()),
        stack: d.stack.then_some(id.index() as u64),
        light: d.light.or(lent.light.map(|light| light.0)),
        light_when: if lent.light.is_some() { LightWhen::Worn } else { LightWhen::Carried },
        holds: lent.statuses.clone(),
        dark_sight: lent.dark_sight,
        level,
    })
}

/// `d`'s plain level, for [`Armory::plain_level`]: its lowest `found_at`
/// for a wand, [`STAFF_LEVEL`] for a staff, the least of [`RING_LEVELS`]
/// for a ring and nought for the rest, or `None` for a thing that takes
/// no levels.
fn plain_level(d: &ItemDef, registries: &Registries) -> Option<i32> {
    if !takes_levels(d, registries) {
        return None;
    }
    let tagged = |name: &str| d.tags.iter().any(|t| registries.tags.name(t.id()) == name);
    let plain = match d.found_at {
        Some((lowest, _)) => lowest,
        None if tagged("staff") => STAFF_LEVEL,
        None if tagged("ring") => *RING_LEVELS.start(),
        None => 0,
    };
    Some(plain.max(0))
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
/// it lends and the registries it names: whether its wear fits the slots,
/// whether a worn thing can give what it lends, whether what it lends
/// folds, and whether what it asks of a wearer and gives one by its level
/// is on a thing that is worn and takes levels. What the engine would never
/// play, a worn stack or a land trigger on a thing never thrown, the
/// engine's own check of its parts says when the armory builds them.
fn validate_def(d: &ItemDef, traits: &Registry<TraitDef>, registries: &Registries) -> Result<(), String> {
    if let Some(wear) = &d.wear {
        wear.shape(&registries.slots)?;
    }
    if d.pile.is_some() && !d.stack {
        return Err("a pile of a thing that does not stack is that many things lying apart".into());
    }
    let level_stats = d.level.as_ref().is_some_and(|l| !l.stats.is_empty());
    if d.wear.is_none() && (d.strength.is_some() || !d.bestows.is_empty() || !d.caps.is_empty() || level_stats) {
        return Err("asks strength of, or gives stats to, a wearer of a thing that is never worn".into());
    }
    if !takes_levels(d, registries) && d.level.is_some() {
        return Err("gives something by its level, on a thing that takes no levels".into());
    }
    if d.level.as_ref().is_some_and(|l| l.armor.is_some()) && d.armor == 0 {
        return Err("armor in its level, on a thing with no armor of its own".into());
    }
    if let Some((low, high)) = d.found_at
        && (!takes_levels(d, registries) || low > high)
    {
        return Err(format!("found at levels {low} to {high}, on a thing that takes none or a range that is empty"));
    }
    // A zap is seen by its flight: one that flies as nothing is a turn spent
    // and a foe hurt by nothing the player saw leave their hands.
    if d.triggers.iter().any(|t| t.targeting.is_some() && t.look.is_none()) {
        return Err("a use aimed at somewhere, with no look for what flies there".into());
    }
    if d.wear.is_none() && !d.lends.is_empty() {
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
/// it runs, what it knows how to do and what its own kind does while it
/// holds a status are the body's own, read once when it is made, and a
/// thing that lent them would lend nothing at all.
fn unlendable(t: &TraitDef) -> Option<&'static str> {
    let set = [
        ("wits", t.wits.is_some()),
        ("speed", t.speed.is_some()),
        ("perception", t.perception.is_some()),
        ("flee_at", t.flee_at.is_some()),
        ("notice", t.notice.is_some()),
        ("hearing", t.hearing.is_some()),
        ("abilities", !t.abilities.is_empty()),
        ("while_holding", !t.while_holding.is_empty()),
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
/// What a worn thing lends is in its parts, so the engine gives it to
/// whoever wears it and to nobody else, as [`wearing`](crate::wearing)
/// says.
///
/// It is spawned under its own name and as it is: what the adventurer
/// knows of it, and what it is called for that, is
/// [`lore`](crate::lore)'s, which names everything the pass it appears.
pub fn spawn_item(commands: &mut Commands, armory: &Armory, def: Id<ItemDef>) -> Entity {
    spawn_item_at(commands, armory, def, 0)
}

/// Spawns `def` as [`spawn_item`] does, at `level`, which a thing that
/// takes no levels ignores and one that does holds within its level's
/// bounds.
///
/// Everything the engine plays is its parts' and the engine's
/// [`insert_parts`], which applies the level once, here, and never on top
/// of itself: a save keeps the level, and a continued run's `+2` is this
/// function run again. What is Undermere's goes on beside them: its name
/// and look, what it is worth to a mind that wears things, a staff's pace
/// at its level, what it lends, and its curse: a thing made below nought
/// is [`Stuck`], and will not come off once it is put on, until its level
/// is raised to nought or the curse is lifted.
pub fn spawn_item_at(commands: &mut Commands, armory: &Armory, def: Id<ItemDef>, level: i32) -> Entity {
    let d = armory.defs.get(def);
    let (r, g, b) = d.color;
    let mut e = commands.spawn((ItemKind(def), Name::new(d.name.clone()), Glyph::new(d.glyph, Color::srgb(r, g, b)).on_layer(2)));
    let level = insert_parts(&mut e, armory.parts(def), level);
    if d.mass {
        e.insert(MassNoun);
    }
    if armory.takes_levels(def) && level < 0 {
        e.insert(Stuck);
    }
    // Worth something to a mind that puts things on, so a goblin knows a
    // sword from a dagger without knowing what either is.
    if let Some(score) = armory.score_at(def, level) {
        e.insert(GearScore(score));
    }
    // A staff refills at its level's pace, which its carrier's wisdom
    // quickens: `staffs` keeps the pace current from here on.
    if let Some(every) = armory.recharge_at(def, level) {
        e.entry::<Consumable>().and_modify(move |mut held| {
            if let Some(pace) = held.recharge.as_mut() {
                pace.every = every;
            }
        });
    }
    e.id()
}

/// Raises `item`'s level by `by`, or lowers it for a `by` below nought,
/// and fits it to the new level: the engine's
/// [`level_up`](atglyph::ecs::parts::level_up) fits everything its
/// parts say from the parts it was made of, so its armor, its blows and
/// what it gives its wearer are its definition's at the new level and
/// never the old numbers with a level added twice, and what it holds grows
/// with the level and what it has left by as much; Undermere's own
/// follows it, what it is worth to a mind and a staff's pace.
///
/// A cursed thing whose level reaches nought comes unstuck, which is
/// Undermere's curse and not the engine's level. Returns the new level, or
/// `None` for a thing that takes no levels, which is left as it was.
pub fn level_up(world: &mut World, item: Entity, by: i32) -> Option<i32> {
    let def = world.get::<ItemKind>(item)?.0;
    let level = atglyph::ecs::parts::level_up(world, item, by)?;
    let armory = world.resource::<Armory>();
    let (score, pace) = (armory.score_at(def, level), armory.recharge_at(def, level));
    let mut e = world.entity_mut(item);
    match score {
        Some(score) => e.insert(GearScore(score)),
        None => e.remove::<GearScore>(),
    };
    if let (Some(every), Some(mut held)) = (pace, e.get_mut::<Consumable>())
        && let Some(recharge) = held.recharge.as_mut()
    {
        recharge.every = every;
    }
    if level >= 0 {
        e.remove::<Stuck>();
    }
    Some(level)
}

/// Spawns `def` at `level` as [`spawn_item_at`] does, as a thing found
/// rather than set out with: one whose level its wearer learns by wearing
/// it is [`Unfamiliar`], so it is called by its
/// kind alone, with no level, until it has been worn long enough or is
/// identified.
pub fn spawn_found(commands: &mut Commands, armory: &Armory, def: Id<ItemDef>, level: i32) -> Entity {
    let thing = spawn_item_at(commands, armory, def, level);
    if armory.learned_by_wear(def) {
        commands.entity(thing).insert(crate::lore::Unfamiliar::default());
    }
    thing
}

/// Spawns `count` of `def`: one stack of `count` for a thing that stacks,
/// and `count` of it otherwise, placed nowhere.
pub fn spawn_items(commands: &mut Commands, armory: &Armory, def: Id<ItemDef>, count: u32) -> Vec<Entity> {
    if armory.defs.get(def).stack {
        let stack = spawn_item(commands, armory, def);
        commands.entity(stack).insert(Stack { key: def.index() as u64, count: count.max(1) });
        return vec![stack];
    }
    (0..count).map(|_| spawn_item(commands, armory, def)).collect()
}

/// What one of `def` at `level` is worth, as the adventurer knows it, for
/// the engine's `Worth`: `None` for a def with no price at all, gold; for
/// a kind not known; for a curse's sign shown; or for a level known to be
/// below nought. Else the def's price, with [`WORTH_PER_LEVEL`] a level
/// above its kind's [plain level](Armory::plain_level) once the level is
/// known, an `unfamiliar` one counted as plain, as a shop that tells
/// nothing of a level the adventurer cannot read either. Above plain
/// rather than above nought, since a staff is never at nought: its price
/// is what a plain one costs, as a sword's is.
///
/// A thing a keeper carries is `shelved`, and its kind is named whether
/// the adventurer knows it or not, since keepers name what they sell: so
/// an unknown potion on a shelf is for sale at its kind's price, where
/// one in the pack is worth nothing to anybody.
fn worth_of(armory: &Armory, lore: &Lore, def: Id<ItemDef>, level: i32, unfamiliar: bool, cursed: bool, shelved: bool) -> Option<u32> {
    if cursed || !(shelved || lore.knows(&armory.defs.get(def).name)) {
        return None;
    }
    let level = if unfamiliar { armory.plain_level(def).unwrap_or(0) } else { level };
    armory.told_worth(def, level)
}

/// A thing whose worth may have changed: one just made, one whose level
/// changed, one just found unfamiliar, and one whose curse just showed.
type Repriced = (With<ItemKind>, Or<(Added<ItemKind>, Changed<ItemLevel>, Added<Unfamiliar>, Added<CurseShown>)>);

/// What every thing's price, its level and its `Worth` answer to.
type Priced<'a> = (Entity, &'a ItemKind, Option<&'a ItemLevel>, Has<Unfamiliar>, Has<CurseShown>, Option<&'a Worth>);

/// Keeps every thing's `Worth` current from its def's price, the level the
/// adventurer knows of it and what they know of its kind: run whenever a
/// thing's price might have moved rather than saved, since only the game
/// knows what a level, a kind or a curse's sign is worth, as
/// [`Bestows`] is kept current rather than saved. A kind just learned
/// reprices every thing of it at once, so the check runs for everything
/// whenever `lore` has changed and not only for what else moved, and so
/// does a keeper's bag changing, since what a keeper carries is priced by
/// its kind's name and what changed hands is priced again on its new side.
///
/// An artifact a keeper carries has no `Worth`: the curio pays a quarter
/// of its price for one, the 400 the price list says, and never sells it
/// back, since an artifact is never sold.
pub fn keep_worth(
    mut commands: Commands,
    lore: Res<Lore>,
    armory: Res<Armory>,
    changed: Query<(), Repriced>,
    mut familiar: RemovedComponents<Unfamiliar>,
    shelves: Query<Ref<Inventory>, With<Trader>>,
    things: Query<Priced>,
) {
    let learned = familiar.read().count() > 0;
    let restocked = shelves.iter().any(|bag| bag.is_changed());
    if !(lore.is_changed() || learned || restocked || !changed.is_empty()) {
        return;
    }
    let shelved: Vec<Entity> = shelves.iter().flat_map(|bag| bag.items.clone()).collect();
    for (thing, kind, enchant, unfamiliar, cursed, had) in &things {
        let level = enchant.map_or(0, |l| l.0);
        let on_shelf = shelved.contains(&thing);
        let worth = if on_shelf && armory.is_artifact(kind.0) { None } else { worth_of(&armory, &lore, kind.0, level, unfamiliar, cursed, on_shelf) };
        if worth == had.map(|w| w.0) {
            continue;
        }
        match worth {
            Some(worth) => commands.entity(thing).insert(Worth(worth)),
            None => commands.entity(thing).remove::<Worth>(),
        };
    }
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
    /// [`Unfamiliar`], so it is called by its kind
    /// alone until it has been worn long enough or is identified.
    fn make(
        &self,
        commands: &mut Commands,
        registries: &Registries,
        def: Id<ItemDef>,
        count: u32,
        from: Provenance,
        rng: &mut atglyph::rand::rngs::StdRng,
    ) -> Vec<Entity> {
        let count = match self.defs.get(def).pile {
            Some(pile) => (0..count).map(|_| pile.roll(rng).max(1) as u32 * from.band.max(1) as u32).sum(),
            None => count,
        };
        if !self.takes_levels(def) {
            return spawn_items(commands, self, def, count);
        }
        (0..count).map(|_| spawn_found(commands, self, def, self.found_level(def, from.band, registries, rng))).collect()
    }

    /// Made again as [`spawn_item_at`] makes it, and never unfamiliar: a
    /// save puts back what the adventurer had learned of it.
    fn make_at(&self, commands: &mut Commands, _: &Registries, def: Id<ItemDef>, level: i32) -> Entity {
        spawn_item_at(commands, self, def, level)
    }

    fn id_of(&self, name: &str) -> Option<Id<ItemDef>> {
        self.defs.id(name)
    }

    fn name_of(&self, def: Id<ItemDef>) -> &str {
        self.defs.name(def)
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
            LootArea::Place(_) => 0,
        }
    }
}

#[cfg(test)]
mod draughts;
#[cfg(test)]
mod magic;
#[cfg(test)]
mod play;
#[cfg(test)]
mod scrolls;

#[cfg(test)]
mod tests {
    use atglyph::primitives::RunSeed;
    use atglyph::rand::SeedableRng;
    use bevy::ecs::world::CommandQueue;

    use super::*;
    use crate::content::Library;
    use crate::testing::{adventurer, equip, familiar, find, headless, make, name_of, said, settle};

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
        let made = names.iter().map(|n| spawn_item(&mut commands, armory, armory.defs.expect(n))).collect();
        queue.apply(&mut world);
        (world, made)
    }

    /// Every coat of body armor is a stuff, said with no `a`, `chain mail`
    /// and `leather armor`, and the shield, the helm and every weapon are
    /// counted one at a time.
    #[test]
    fn body_armor_is_said_with_no_article_and_a_shield_or_a_sword_with_one() {
        let library = library();
        let body = library.registries.slots.expect("body");
        let coats: Vec<&str> =
            library.armory.defs.iter().filter(|(_, d)| d.wear.as_ref().is_some_and(|w| w.slot.id() == body)).map(|(_, d)| d.name.as_str()).collect();
        assert_eq!(coats.len(), 6, "{coats:?}");
        let counted = ["shield", "helm", "sword", "bow", "ring of light"];
        let (world, made) = spawned(&library, &[coats.as_slice(), &counted].concat());
        let (stuffs, ones) = made.split_at(coats.len());
        assert!(stuffs.iter().all(|e| world.get::<MassNoun>(*e).is_some()), "{coats:?} are each a stuff");
        assert!(ones.iter().all(|e| world.get::<MassNoun>(*e).is_none()), "{counted:?} are each counted");
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
            "potion of extra healing",
            "potion of strength",
            "potion of life",
            "potion of haste",
            "potion of fire immunity",
            "potion of darkness",
            "potion of descent",
            "potion of incineration",
            "potion of paralysis",
            "potion of confusion",
            "scroll of enchanting",
            "scroll of identify",
            "scroll of remove curse",
            "scroll of teleportation",
            "scroll of recharging",
            "scroll of magic mapping",
            "scroll of discord",
            "scroll of aggravate monsters",
            "scroll of summon monsters",
            "staff of firebolt",
            "staff of lightning",
            "staff of poison",
            "staff of slowness",
            "staff of blinking",
            "staff of conjuration",
            "wand of domination",
            "wand of teleport other",
            "wand of polymorph",
            "wand of beckoning",
            "wand of plenty",
            "ring of regeneration",
            "ring of stealth",
            "ring of light",
            "ring of wisdom",
            "ring of reaping",
            "ring of awareness",
            "gold coin",
            "lantern",
            "torch",
            "waystone",
            "the Crown of Grubnak",
            "the Widow's Fang",
            "the Choirmaster's Staff",
            "the Abbess's Chalice",
            "the Forgemaster's Hammer",
            // The risk-and-reward pool's key.
            "vault key",
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
        assert_eq!((bow.dice, bow.range, bow.effective_range()), (plus(2, 4, 1), 10, 7));
        assert_eq!(world.get::<Ammo>(made[2]).map(|a| a.tag), library.registries.tags.id("arrow"), "and spends an arrow a shot");
        let shapes: Vec<_> = made[3..].iter().map(|e| world.get::<MeleeAttack>(*e).and_then(|m| m.shape)).collect();
        assert_eq!(shapes, [Some(AttackShape::Sweep), Some(AttackShape::Pierce(1)), None], "an axe sweeps, a pike runs on, a sword strikes its target");
    }

    /// An arrow is ammunition and nothing else: it has no throw, so the
    /// throwing key never offers it. A bow with arrows out-damages the best
    /// thing thrown, a javelin, shoots further and is true further, so the
    /// bow is the ranged choice and darts and javelins the stopgap.
    #[test]
    fn arrows_are_never_thrown_and_a_bow_out_damages_and_out_ranges_a_javelin_and_a_dart() {
        let library = library();
        let (world, made) = spawned(&library, &["arrow", "bow", "javelin", "dart"]);
        assert!(world.get::<Throwable>(made[0]).is_none(), "an arrow has no throw");
        assert!(library.armory.defs.get(library.armory.defs.expect("arrow")).stack, "and stacks as ammunition");
        let bow = world.get::<RangedAttack>(made[1]).expect("the bow shoots");
        for thrown in &made[2..] {
            let throw = world.get::<Throwable>(*thrown).expect("a javelin and a dart are thrown");
            let (_, dice) = throw.strike.expect("and strike");
            assert!(bow.dice.avg() > dice.avg(), "the bow's {} against {}", bow.dice.avg(), dice.avg());
            assert!(bow.range > throw.range, "the bow reaches {} against {}", bow.range, throw.range);
            assert!(bow.effective_range() > throw.effective.unwrap_or(throw.range / 3), "and is true further");
        }
    }

    /// Spawns `name` at `level` through the library's armory into a bare
    /// world, beside the registries a level is fitted against.
    fn at_level(library: &Library, name: &str, level: i32) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(library.registries.clone());
        world.insert_resource(library.armory.clone());
        let mut queue = CommandQueue::default();
        let armory = &library.armory;
        let made = spawn_item_at(&mut Commands::new(&mut queue, &world), armory, armory.defs.expect(name), level);
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
        assert_eq!((gives(&world, &library, sword, "accuracy"), world.get::<ItemLevel>(sword).map(|l| l.0)), (10, Some(2)));
        let (world, mail) = at_level(&library, "chain mail", -1);
        assert_eq!((world.get::<Armor>(mail).map(|a| a.0), world.get::<Stuck>(mail).is_some()), (Some(2), true), "a cursed coat");
        let (world, ring) = at_level(&library, "ring of stealth", 2);
        assert_eq!((gives(&world, &library, ring, "stealth"), world.get::<Stuck>(ring).is_some()), (20, false));
        let (world, ring) = at_level(&library, "ring of stealth", -3);
        assert_eq!((gives(&world, &library, ring, "stealth"), world.get::<Stuck>(ring).is_some()), (-30, true));
        let (world, shield) = at_level(&library, "shield", 1);
        assert_eq!((gives(&world, &library, shield, "evasion"), world.get::<Armor>(shield).map(|a| a.0)), (10, Some(2)));
        let (world, potion) = at_level(&library, "potion of healing", 3);
        assert!(world.get::<ItemLevel>(potion).is_none(), "a potion takes no levels, whatever it is made at");
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
        let rod = r#"[(name: "rod", glyph: '/', color: (0.5, 0.5, 0.5), tags: ["staff"], consumable: (charges: 1, when_empty: Kept),
                      triggers: [(on: "use", effects: [(kind: "Mend", args: (roll: "1d4"))])])]"#;
        let defs = read(rod, &library.registries, &library.traits).expect("the rod reads");
        let armory = Armory::new(defs, LootTable::default(), &library.registries, &library.traits, &crate::content::effect_kinds(), &Moments::default())
            .expect("and is made");
        let mut world = World::new();
        world.insert_resource(library.registries.clone());
        world.insert_resource(armory.clone());
        let mut queue = CommandQueue::default();
        let rod = spawn_item_at(&mut Commands::new(&mut queue, &world), &armory, armory.defs.expect("rod"), 2);
        queue.apply(&mut world);
        assert_eq!(world.get::<Consumable>(rod).map(|c| (c.left, c.max)), Some((3, 3)), "one and two");
        world.get_mut::<Consumable>(rod).expect("charges").left = 1;
        assert_eq!(level_up(&mut world, rod, 1), Some(3));
        assert_eq!(world.get::<Consumable>(rod).map(|c| (c.left, c.max)), Some((2, 4)), "one more held, one more left");
    }

    /// A ring is found at one to three, cursed below nought or not, however
    /// deep, over a span of seeds; a potion takes no levels and draws
    /// nothing from the stream, so finding one deals the floor's loot as it
    /// was dealt before levels were found. How often a ring is cursed is
    /// [`CURSED_RINGS_IN_TEN`], read where it is written rather than
    /// counted off a thousand draws.
    #[test]
    fn a_ring_is_found_at_one_to_three_cursed_or_not_and_a_potion_draws_nothing() {
        use atglyph::rand::RngCore;
        let library = library();
        let (ring, potion) = (library.armory.defs.expect("ring of light"), library.armory.defs.expect("potion of healing"));
        let mut cursed = std::collections::BTreeSet::new();
        for seed in 0..100 {
            let mut rng = atglyph::rand::rngs::StdRng::seed_from_u64(seed);
            let level = library.armory.found_level(ring, 1 + (seed % 10) as i32, &library.registries, &mut rng);
            assert!(RING_LEVELS.contains(&level.abs()), "seed {seed}: {level}");
            cursed.insert(level < 0);
            let (mut drew, mut fresh) = (atglyph::rand::rngs::StdRng::seed_from_u64(seed), atglyph::rand::rngs::StdRng::seed_from_u64(seed));
            assert_eq!(library.armory.found_level(potion, 10, &library.registries, &mut drew), 0);
            assert_eq!(drew.next_u64(), fresh.next_u64(), "seed {seed}: a potion draws nothing");
        }
        assert_eq!(cursed.len(), 2, "rings are found both cursed and not");
    }

    /// A weapon or a piece of armor found uncursed is better by exactly
    /// the shares [`better_by`] gives, counted over every roll of a hundred
    /// rather than sampled: on floor one a tenth of it is one better, on
    /// floor four two fifths and half of that two better, and on floor ten
    /// all of it, half two better. One cursed is one to three below nought,
    /// and everything found lies between minus three and plus two, over a
    /// span of seeds; how often one is cursed is [`CURSED_GEAR_IN_FIVE`].
    #[test]
    fn a_weapon_or_armor_found_deeper_is_more_often_better_by_the_shares_better_by_gives() {
        for (floor, one, two) in [(1, 10, 0), (4, 20, 20), (10, 50, 50)] {
            let count = |level: i32| (0..100).filter(|pct| better_by(floor, *pct) == level).count();
            assert_eq!((count(1), count(2)), (one, two), "floor {floor}: of a hundred uncursed, how many one and two better");
        }
        let library = library();
        for name in ["sword", "chain mail"] {
            let def = library.armory.defs.expect(name);
            let mut cursed = std::collections::BTreeSet::new();
            for seed in 0..200 {
                let level = library.armory.found_level(def, 1 + (seed % 10) as i32, &library.registries, &mut atglyph::rand::rngs::StdRng::seed_from_u64(seed));
                assert!((-3..=2).contains(&level), "{name}, seed {seed}: {level}");
                if level < 0 {
                    cursed.insert(level);
                }
            }
            assert_eq!(cursed, std::collections::BTreeSet::from([-3, -2, -1]), "{name}: each of one to three below nought when cursed");
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
        let mut rng = atglyph::rand::rngs::StdRng::seed_from_u64(1);
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
            (name: "lucky stone", glyph: '*', color: (0.5, 0.5, 0.5), wear: (slot: "head"), level: (stats: [("stealth", 5)])),
            (name: "odd helm", glyph: '^', color: (0.5, 0.5, 0.5), wear: (slot: "head", either: true), tags: ["armor"], armor: 1),
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

    /// A zap that flies as nothing, a level's power on a thing that takes
    /// no levels, a wand found at levels that run backwards and a charge
    /// that comes back in no time are each refused, naming the thing.
    #[test]
    fn an_unseen_zap_a_power_without_levels_a_backwards_find_and_an_instant_recharge_are_refused() {
        let library = library();
        let bad = r#"[
            (name: "dark rod", glyph: '~', color: (0.5, 0.5, 0.5), tags: ["wand"], consumable: (charges: 1, when_empty: Kept),
             triggers: [(on: "use", targeting: (mode: Bolt(range: 8)), effects: [(kind: "Beckon")])]),
            (name: "strong draught", glyph: '!', color: (0.5, 0.5, 0.5), tags: ["potion"], level: (amount: 1)),
            (name: "odd wand", glyph: '~', color: (0.5, 0.5, 0.5), tags: ["wand"], found_at: (2, 1)),
        ]"#;
        let Err(ContentError::Invalid(errors)) = read(bad, &library.registries, &library.traits) else { panic!("it read") };
        assert_eq!(
            errors,
            [
                "dark rod: a use aimed at somewhere, with no look for what flies there",
                "strong draught: gives something by its level, on a thing that takes no levels",
                "odd wand: found at levels 2 to 1, on a thing that takes none or a range that is empty",
            ]
        );
        // What the engine would never play is its own check, made as the
        // armory builds each thing's parts.
        let quick = r#"[(name: "quick staff", glyph: '/', color: (0.5, 0.5, 0.5), tags: ["staff"], consumable: (charges: 1, when_empty: Kept, recharge: 0))]"#;
        let defs = read(quick, &library.registries, &library.traits).expect("the staff reads");
        let made = Armory::new(defs, LootTable::default(), &library.registries, &library.traits, &crate::content::effect_kinds(), &Moments::default());
        let Err(ContentError::Invalid(errors)) = made else { panic!("it was made") };
        assert_eq!(errors, ["quick staff: a charge that comes back in no time at all is never spent"]);
    }

    /// A staff's level is its power, its charges and its pace: a staff of
    /// firebolt two better holds two more charges, refills twice as fast and
    /// adds four to its bolt's roll, fitted again whenever its level moves,
    /// and found at two, or better deeper.
    #[test]
    fn a_staffs_level_is_its_charges_its_pace_and_its_power_and_it_is_found_at_two_or_better_deeper() {
        let library = library();
        let staff = library.armory.defs.expect("staff of firebolt");
        let (mut world, made) = at_level(&library, "staff of firebolt", 2);
        let read = |world: &World| {
            let c = world.get::<Consumable>(made).expect("charges");
            (c.max, c.recharge.map(|r| r.every), world.get::<EffectBonus>(made).map(|b| b.amount))
        };
        assert_eq!(read(&world), (2, Some(25_000), Some(4)), "two charges, two hundred and fifty turns a charge, four on the roll");
        level_up(&mut world, made, 2);
        assert_eq!(read(&world), (4, Some(12_500), Some(8)), "two better: four, half the time, eight");
        let mut rng = atglyph::rand::rngs::StdRng::seed_from_u64(1);
        let levels: Vec<i32> = (0..200).map(|_| library.armory.found_level(staff, 1, &library.registries, &mut rng)).collect();
        assert!(levels.iter().all(|l| *l >= STAFF_LEVEL), "never below two: {levels:?}");
        let deep: Vec<i32> = (0..200).map(|_| library.armory.found_level(staff, 10, &library.registries, &mut rng)).collect();
        assert!(deep.iter().all(|l| *l > STAFF_LEVEL), "on the throne's floor, always better");
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
                let mut rng = atglyph::rand::rngs::StdRng::seed_from_u64(seed);
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
        let helm = r#"[(name: "owl helm", glyph: '^', color: (0.5, 0.6, 0.5), wear: (slot: "head"), tags: ["armor"], armor: 1, lends: ["sees in the dark"])]"#;
        let defs = read(helm, registries, traits).expect("a helm lending a trait");
        assert_eq!(defs.get(defs.expect("owl helm")).lends.iter().map(|t| t.id()).collect::<Vec<_>>(), vec![traits.expect("sees in the dark")]);

        let bad = r#"[
            (name: "odd helm", glyph: '^', color: (0.5, 0.6, 0.5), wear: (slot: "head"), lends: ["sees in the drak"]),
            (name: "loose charm", glyph: '"', color: (0.5, 0.6, 0.5), lends: ["swift"]),
            (name: "twisted helm", glyph: '^', color: (0.5, 0.6, 0.5), wear: (slot: "head"), lends: ["thick-hided", "armored"]),
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
            assert_eq!(world.get::<LightSource>(*e), Some(&LightSource(light)), "{name}");
        }
        assert!(world.get::<Throwable>(made[1]).is_some(), "a torch can be thrown into the dark");
    }

    /// Every potion is used up by its use or by where it lands when it is
    /// thrown: what each does drunk is its `use` trigger, built once, and
    /// one that does something where it breaks has a `land` trigger too,
    /// which a potion wasted thrown has not.
    #[test]
    fn every_potion_is_spent_drunk_or_thrown_and_only_one_that_does_something_where_it_breaks_lands() {
        let library = library();
        let potions = [
            ("potion of healing", true),
            ("potion of extra healing", true),
            ("potion of strength", false),
            ("potion of life", false),
            ("potion of haste", true),
            ("potion of fire immunity", true),
            ("potion of darkness", true),
            ("potion of descent", false),
            ("potion of incineration", true),
            ("potion of paralysis", true),
            ("potion of confusion", true),
        ];
        let (world, made) = spawned(&library, &potions.map(|(name, _)| name));
        for ((name, lands), e) in potions.iter().zip(&made) {
            let triggers = world.get::<Triggers>(*e).unwrap_or_else(|| panic!("{name} does something"));
            assert!(triggers.on(Moments::USE).next().is_some(), "{name} is drunk");
            assert_eq!(triggers.on(Moments::LAND).next().is_some(), *lands, "{name}: whether it does something where it breaks");
            assert!(world.get::<Consumable>(*e).is_some() && world.get::<Throwable>(*e).is_some(), "{name} is spent, and can be thrown");
        }
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
            (name: "boots of haste", glyph: '[', color: (0.5, 0.5, 0.5), wear: (slot: "body"), lends: ["swift"]),
            (name: "cap of wits", glyph: '^', color: (0.5, 0.5, 0.5), wear: (slot: "head"), lends: ["sapient"]),
            (name: "glowing lantern", glyph: '(', color: (0.5, 0.5, 0.5), wear: (slot: "light"), light: (intensity: 200, radius: 3), lends: ["fungal glow"]),
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
        [(name: "stone ring", glyph: '=', color: (0.5, 0.5, 0.5), wear: (slot: "off hand"), armor: 1, resists: [("cold", 10)],
          lends: ["armored", "fire-blooded", "regenerates", "sees in the dark"])]"#;
        let defs = read(ring, registries, traits).expect("the ring reads");
        let armory = Armory::new(defs, LootTable::default(), registries, traits, &crate::content::effect_kinds(), &Moments::default()).expect("and is made");
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let made = spawn_item(&mut Commands::new(&mut queue, &world), &armory, armory.defs.expect("stone ring"));
        queue.apply(&mut world);
        let kind = |name: &str| registries.damage_kinds.expect(name);
        assert_eq!(world.get::<Armor>(made).map(|a| a.0), Some(5), "its own one and the four `armored` lends");
        let resists = &world.get::<Resists>(made).expect("resistances").0;
        assert_eq!((resists.get(kind("fire")), resists.get(kind("cold"))), (100, -40), "fire-blooded beside its own ten against cold");
        let held = world.get::<HeldWhileWorn>(made).map(|h| h.0.clone());
        let (sight, lit) = (world.get::<WornDarkSight>(made).map(|d| d.0), world.get::<LitWhenWorn>(made).is_some());
        assert_eq!((held, sight, lit), (Some(vec![registries.statuses.expect("regenerating")]), Some(6), false), "what it lends a wearer, and no light");
    }

    /// A weapon found rather than set out with reads `+?`, its level not
    /// yet known, and is worth no more than its def's price at nought, as
    /// a shop that tells nothing of a level cannot price what it does not
    /// say either; true whichever seed dealt this run its faces.
    #[test]
    fn a_thing_found_whose_level_is_not_yet_known_is_worth_its_nought_price_over_a_span_of_seeds() {
        for seed in 0..5 {
            let mut app = headless(RunSeed(seed));
            adventurer(&mut app);
            let sword = find(&mut app, "sword", 3);
            settle(&mut app);
            assert_eq!(app.world().get::<Worth>(sword).copied(), Some(Worth(60)), "seed {seed}: {}", name_of(&app, sword));
        }
    }

    /// A ring nobody has named yet is worth nothing to a keeper, and
    /// neither is one whose curse has shown, even once wearing it has
    /// made its kind known: a curse's sign, not its kind, is what a
    /// keeper will not buy.
    #[test]
    fn an_unknown_kind_and_a_shown_curse_have_no_worth() {
        let mut app = headless(RunSeed(8));
        let me = adventurer(&mut app);
        let unnamed = find(&mut app, "ring of light", 2);
        settle(&mut app);
        assert!(app.world().get::<Worth>(unnamed).is_none(), "a kind nobody has named yet");

        let cursed = find(&mut app, "ring of stealth", -1);
        equip(&mut app, me, cursed);
        assert_eq!(name_of(&app, cursed), "unidentified ring of stealth", "{:#?}", said(&app));
        assert!(app.world().get::<CurseShown>(cursed).is_some(), "its curse has shown: {:#?}", said(&app));
        assert!(app.world().get::<Worth>(cursed).is_none(), "worn and its kind known, but its curse has shown");
    }

    /// Once a level is known, by wearing a thing long enough, every level
    /// above nought it carries adds a hundred to what it is worth, over
    /// its def's own price.
    #[test]
    fn knowing_a_level_raises_worth_a_hundred_a_level() {
        let mut app = headless(RunSeed(9));
        let me = adventurer(&mut app);
        let sword = find(&mut app, "sword", 3);
        equip(&mut app, me, sword);
        assert_eq!(app.world().get::<Worth>(sword).copied(), Some(Worth(60)), "not yet known, counted at nought");
        familiar(&mut app, sword);
        assert_eq!(name_of(&app, sword), "+3 sword", "{:#?}", said(&app));
        assert_eq!(app.world().get::<Worth>(sword).copied(), Some(Worth(60 + 300)), "known, three levels of a hundred each");
    }

    /// An artifact wears no face, so its kind is known from the first, at
    /// the one price every artifact asks, which the curio buys at a
    /// quarter.
    #[test]
    fn an_artifact_is_worth_sixteen_hundred() {
        let mut app = headless(RunSeed(10));
        adventurer(&mut app);
        let crown = make(&mut app, "the Crown of Grubnak");
        settle(&mut app);
        assert_eq!(app.world().get::<Worth>(crown).copied(), Some(Worth(1600)));
    }

    /// Gold is the currency and never for sale, so it carries no price
    /// and therefore no `Worth` at all.
    #[test]
    fn gold_has_no_worth() {
        let mut app = headless(RunSeed(11));
        adventurer(&mut app);
        let gold = make(&mut app, "gold coin");
        settle(&mut app);
        assert!(app.world().get::<Worth>(gold).is_none());
    }
}
