//! The armory: the weapons, armor, potions, lights and gold of the
//! Undermere, loaded once from `items.ron` into the [`Armory`] resource,
//! found where `item_spawns.ron` says, and spawned as items an actor can
//! find, carry, wear and use.
//!
//! The [`Armory`] is Undermere's [`ItemMaker`]: the engine's loot decides
//! what lies on a floor, what a kill leaves and what a chest holds, and
//! asks the armory to make each thing. A pile of gold is rolled there,
//! since how much a pile holds depends on how deep it was found.
//!
//! An [`ItemDef`] is Foundry's item schema trimmed to what Undermere uses,
//! plus `lends`: the traits a worn thing gives its wearer, the same traits
//! a monster is written as, so a ring of the salamander and a salamander
//! are fire-blooded for the same reason. The names resolve here, at load,
//! and are folded once into what each thing lends: its armor and its
//! resistances join the thing's own, which the engine counts for whoever
//! wears it, and the rest is the thing's [`Lends`], which
//! [`wearing`](crate::wearing) puts on its wearer and takes off again.

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
/// an optional `look`, as Foundry writes one.
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
    /// What a fresh unit holds.
    pub charges: u16,
    /// What happens at the last.
    pub when_empty: WhenEmpty,
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
    /// True for a one-handed weapon that fits either hand.
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
        Ok(Self { defs, table, triggers, lent })
    }
}

/// Reads an items file against `registries` and the `traits` an item
/// lends, and checks each item makes sense on its own: every problem in
/// the file at once, each under the item it is in.
pub fn read(text: &str, registries: &Registries, traits: &Registry<TraitDef>) -> Result<Registry<ItemDef>, ContentError> {
    let defs: Registry<ItemDef> = registries.names().with("trait", traits).load(text)?;
    defs.validate(|d, _| validate_def(d, traits))?;
    Ok(defs)
}

/// One item's own shape, checked against nothing but itself and the
/// traits it lends: whether its slot claims make sense, whether a trigger
/// can ever come round, whether a worn thing can give what it lends, and
/// whether what it lends folds.
fn validate_def(d: &ItemDef, traits: &Registry<TraitDef>) -> Result<(), String> {
    if d.slot.is_none() && (d.either || !d.also.is_empty()) {
        return Err("either or also without a slot".into());
    }
    if d.either && !d.also.is_empty() {
        return Err("either with also".into());
    }
    if d.stack && d.slot.is_some() {
        return Err("a worn item cannot stack".into());
    }
    if d.pile.is_some() && !d.stack {
        return Err("a pile of a thing that does not stack is that many things lying apart".into());
    }
    if d.consumable.is_some_and(|c| c.charges == 0) {
        return Err("a consumable with no charges is spent before it is found".into());
    }
    if d.throw.is_none() && d.triggers.iter().any(|t| t.on == "land") {
        return Err("a land trigger on a thing that cannot be thrown never lands".into());
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

/// What wearing `d`, lending `lent`, is worth to a mind choosing between
/// the things it finds, for the engine's `GearScore`, or `None` for a thing
/// worth nothing to wear, a lantern or a torch.
///
/// In tenths of a point of damage a turn: a blow's average over what it
/// costs, so a quick dagger is worth less than a sword but not as little
/// as its roll alone says; half that for a shot, which a mind fighting
/// beside its enemy has no use for; and two points a turn for each point
/// of armor. Only the comparison matters, between a thing and what it
/// would displace, so the numbers need agree with nothing else.
fn gear_score(d: &ItemDef, lent: &Folded) -> Option<i32> {
    let blow = d.melee.map_or(0.0, |m| damage_a_turn(m.roll, d.cost));
    let shot = d.ranged.map_or(0.0, |r| damage_a_turn(r.roll, d.cost) / 2.0);
    let armor = (d.armor + lent.armor.unwrap_or(0)) as f32 * 2.0;
    let score = ((blow + shot + armor) * 10.0).round() as i32;
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

/// Where `d` is worn, or `None` for something never worn, such as a
/// potion.
///
/// `either` reaches for both hands directly, since a one-handed weapon
/// that fits either is not naming one slot at all; `also` folds each
/// extra slot a two-hander claims onto the one it starts in.
pub fn shape_of(d: &ItemDef, registries: &Registries) -> Option<EquipShape> {
    if d.either {
        let hands = [registries.slots.expect("main hand"), registries.slots.expect("off hand")];
        return Some(EquipShape::in_any(hands));
    }
    let mut shape = EquipShape::in_slot(d.slot?.id());
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
/// implies, placed nowhere: what it is called and drawn as, what it counts
/// as, what it does at its moments and what that costs it, and, for
/// something worn, its shape, what it is worth to wear, its armor, its
/// resistances and whatever it strikes or shoots with. A stackable item
/// spawns as a stack of one; the caller merges or grows it as it likes.
///
/// What a worn thing lends goes where the engine already reads a worn
/// thing, as far as it can: the armor and the resistances it lends join
/// its own, and the engine counts them for whoever wears it and for
/// nobody else. The rest, a status, sight in the dark and a light, is its
/// [`Lends`], which [`wearing`](crate::wearing) gives its wearer.
pub fn spawn_item(commands: &mut Commands, armory: &Armory, def: Id<ItemDef>, registries: &Registries) -> Entity {
    let d = armory.defs.get(def);
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
        e.insert(Consumable::new(c.charges, c.when_empty));
    }
    if let Some(shape) = shape_of(d, registries) {
        e.insert(Wearable(shape));
        // Worth something to a mind that puts things on, so a goblin
        // knows a sword from a dagger without knowing what either is.
        if let Some(score) = gear_score(d, lent) {
            e.insert(GearScore(score));
        }
    }
    let armor = d.armor + lent.armor.unwrap_or(0);
    if armor != 0 {
        e.insert(Armor(armor));
    }
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
    if let Some(melee) = d.melee {
        e.insert(MeleeAttack { cost: d.cost, look: melee.look, ..MeleeAttack::new(melee.kind.id(), melee.roll) });
    }
    if let Some(ranged) = d.ranged {
        e.insert(RangedAttack {
            cost: d.cost,
            look: ranged.look,
            effective: ranged.effective,
            ..RangedAttack::new(ranged.kind.id(), ranged.roll, ranged.range)
        });
    }
    if let Some(throw) = d.throw {
        e.insert(Throwable { effective: throw.effective, ..Throwable::new(throw.range, throw.strike.map(|(dice, kind)| (kind.id(), dice))) });
    }
    if let Some(light) = d.light {
        e.insert(light.source());
    }
    if d.stack {
        e.insert(Stack { key: def.index() as u64, count: 1 });
    }
    e.id()
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
        spawn_items(commands, self, def, count, registries)
    }

    fn id_of(&self, name: &str) -> Option<Id<ItemDef>> {
        self.defs.id(name)
    }

    fn table(&self) -> &LootTable<Id<ItemDef>> {
        &self.table
    }

    /// Four to six things lying loose on every floor, a first cut: what
    /// changes with the depth is what they are, which `item_spawns.ron`
    /// weights, rather than how many.
    fn scatter(&self) -> ScatterRules {
        ScatterRules::new().loose(4, 6)
    }

    /// Nothing lies loose in Mereholt: the town is where a run sets out
    /// from, and what is worth finding is under the lake.
    fn loose(&self, area: LootArea, planned: u32) -> u32 {
        match area {
            LootArea::Place(crate::dungeon::TOWN) => 0,
            _ => planned,
        }
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

    /// A tripwire over the skeleton's armory, by name, so a thing is added
    /// or dropped on purpose; and every floor has something to find.
    #[test]
    fn every_item_in_the_file_loads_and_every_floor_has_something_to_find() {
        let library = library();
        let skeleton = [
            "dagger",
            "sword",
            "axe",
            "mace",
            "spear",
            "war hammer",
            "bow",
            "arrow",
            "leather armor",
            "chain mail",
            "plate armor",
            "shield",
            "helm",
            "potion of healing",
            "potion of mending",
            "fire flask",
            "gold coin",
            "lantern",
            "torch",
        ];
        for name in skeleton {
            assert!(library.armory.defs.id(name).is_some(), "no item called {name:?}");
        }
        assert_eq!(library.armory.defs.len(), skeleton.len(), "an item this list does not name");
        assert!(library.armory.table.gaps(1..=10).is_empty(), "a floor with nothing to find");
    }

    #[test]
    fn a_weapons_numbers_reach_its_attack_and_a_two_hander_claims_the_off_hand() {
        let library = library();
        let (world, made) = spawned(&library, &["war hammer", "dagger", "bow"]);
        let kind = |name: &str| library.registries.damage_kinds.expect(name);
        let hammer = world.get::<MeleeAttack>(made[0]).expect("the hammer swings");
        assert_eq!((hammer.dice, hammer.kind, hammer.cost), (DiceRoll::new(2, 6), kind("crush"), Some(130)));
        assert_eq!(world.get::<Wearable>(made[0]).unwrap().0.slots().count(), 2, "main hand and off hand");
        let dagger = world.get::<MeleeAttack>(made[1]).expect("the dagger stabs");
        assert_eq!((dagger.dice, dagger.kind, dagger.cost), (DiceRoll::new(1, 4), kind("pierce"), Some(70)));
        let bow = world.get::<RangedAttack>(made[2]).expect("the bow shoots");
        assert_eq!((bow.dice, bow.range, bow.effective_range()), (DiceRoll::new(1, 6), 8, 4));
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
