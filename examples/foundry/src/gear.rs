//! Gear: the weapons, the armor, the slugs, the medical pair and the
//! grenades that arm the foundry's decks, loaded once from `items.ron` into
//! the [`Armory`] resource, found where `item_spawns.ron` says, and spawned
//! as items an actor can find, carry, wear and use.
//!
//! The [`Armory`] is Foundry's [`ItemMaker`]: the engine's `LootPlugin`
//! decides what lies on a deck, what a kill leaves and what a crate holds,
//! and asks the armory to make each thing, rolling a level from
//! `levels.ron` at the band it is found at for a thing that names an
//! `enchant`: a found plate is better the deeper it lay.
//!
//! An [`ItemDef`] is the file's own vocabulary: everything a game needs
//! to know about a weapon or a suit of plate, named rather than typed, so
//! a new weapon is a line in the file and nothing here changes. What
//! wielding or wearing one does is read straight off the spawned entity
//! by the engine's own combat and equip machinery; nothing is copied onto
//! whoever carries it.
//!
//! [`Heat`] and [`Ammo`] are declared in their own modules with no
//! behaviour yet; this module only attaches them to the weapons whose
//! entry names one, so later work on either does not touch the loader.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_bevy::{ItemMaker, LootArea, Provenance};
use rl_engine::rl_core::{DiceRoll, Id};
use rl_engine::rl_render::Glyph;
use rl_engine::rl_rules::{
    AffixDef, DamageKind, Enchanted, EquipShape, LevelTable, LootTable, NameRef, Named, Registry, Resistances, ScatterRules, SlotDef, TagDef, TagId,
};
use rl_engine::rl_rules::{EffectSpec, TriggerSpec};
use serde::Deserialize;

use crate::ammo::Ammo;
use crate::content::{MeleeDef, RangedDef};
use crate::heat::Heat;

const ITEMS_RON: &str = include_str!("../assets/items.ron");
const ITEM_SPAWNS_RON: &str = include_str!("../assets/item_spawns.ron");
const LEVELS_RON: &str = include_str!("../assets/levels.ron");

/// One kind of item, as authored in `items.ron`.
#[derive(Debug, Clone, Deserialize)]
pub struct ItemDef {
    /// Unique; spawn tables and, later, drop tables refer to it.
    pub name: String,
    /// One character.
    pub glyph: char,
    /// `(r, g, b)` in `0..=1`.
    pub color: (f32, f32, f32),
    /// Where it is worn, if it is worn at all.
    #[serde(default)]
    pub slot: Option<NameRef<SlotDef>>,
    /// True for a one-handed weapon that fits either hand, so two can be
    /// dual wielded.
    #[serde(default)]
    pub either: bool,
    /// Slots it also claims when worn, for a two-hander.
    #[serde(default)]
    pub also: Vec<NameRef<SlotDef>>,
    /// What it counts as: a weapon, armor, or a slug, for abilities and
    /// drops that ask by tag rather than by name.
    #[serde(default)]
    pub tags: Vec<NameRef<TagDef>>,
    /// What the thing holds, landed by any of its triggers that names no
    /// list of its own: written once, delivered by each.
    #[serde(default)]
    pub effects: Vec<EffectSpec>,
    /// What it does at its moments: used, thrown and landed, fired, hit.
    #[serde(default)]
    pub triggers: Vec<TriggerSpec>,
    /// What a use, a landing or a shot costs it; absent, nothing, which is
    /// what a tool is.
    #[serde(default)]
    pub consumable: Option<ConsumableDef>,
    /// Flat armor while worn.
    #[serde(default)]
    pub armor: i32,
    /// Percent removed per damage kind, while worn.
    #[serde(default)]
    pub resists: Vec<(NameRef<DamageKind>, i32)>,
    /// The roll and damage kind a blow deals, while wielded.
    #[serde(default)]
    pub melee: Option<MeleeDef>,
    /// The range, roll and damage kind a shot deals, while wielded.
    #[serde(default)]
    pub ranged: Option<RangedDef>,
    /// How far it flies when thrown, and the blow it strikes whoever it
    /// hits, if any; absent, it cannot be thrown at all.
    #[serde(default)]
    pub throw: Option<ThrowDef>,
    /// What one blow or shot costs, in hundredths of a step. Absent is
    /// [`BASE_ACTION_COST`](rl_engine::rl_core::turn::BASE_ACTION_COST).
    #[serde(default)]
    pub cost: Option<u32>,
    /// Heat one shot adds, and heat a quiet turn sheds, for a weapon that
    /// runs hot rather than on ammunition.
    #[serde(default)]
    pub heat: Option<(u32, u32)>,
    /// The tag one shot spends, for a weapon that runs on ammunition
    /// rather than on heat.
    #[serde(default)]
    pub ammo: Option<NameRef<TagDef>>,
    /// Tiles seen without light, while worn.
    #[serde(default)]
    pub dark_sight: Option<i32>,
    /// A clock that comes round while it is worn, for a thing whose `pulse`
    /// trigger does something on it.
    #[serde(default)]
    pub pulse: Option<PulseDef>,
    /// True for a thing whose charges come back only while it is worn, and
    /// which is empty each time it is put on.
    #[serde(default)]
    pub attuned: bool,
    /// How far a thing found about the decks may be enchanted; absent, it
    /// is always plain.
    #[serde(default)]
    pub enchant: Option<EnchantDef>,
    /// Whether copies merge into one counted entry rather than one item
    /// each.
    #[serde(default)]
    pub stack: bool,
}

/// A thing's charges as `items.ron` writes them.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ConsumableDef {
    /// What a fresh unit holds.
    pub charges: u16,
    /// What happens at the last.
    pub when_empty: WhenEmpty,
    /// Hundredths of a step per charge regained, for a thing that refills.
    #[serde(default)]
    pub recharge: Option<u32>,
}

/// A throw as `items.ron` writes it: how far, and the blow it strikes, if
/// it strikes at all. A grenade strikes nothing; its land trigger bursts.
#[derive(Debug, Clone, Copy, Deserialize)]
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

/// A worn thing's clock as `items.ron` writes it: hundredths of a step per
/// pulse at `+0`, what each level adds, and the shortest period any level
/// reaches.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct PulseDef {
    /// Hundredths per pulse at `+0`.
    pub every: u32,
    /// Hundredths added per level; negative for a clock that quickens.
    pub per_level: i32,
    /// The shortest period any level reaches.
    pub fastest: u32,
}

impl PulseDef {
    /// The period at `level`.
    pub fn at(&self, level: i32) -> u32 {
        (self.every as i32 + self.per_level * level.max(0)).max(self.fastest as i32).max(1) as u32
    }
}

/// How far a thing may be enchanted where it is found.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct EnchantDef {
    /// The most any one of it is found at.
    pub most: i32,
}

impl Named for ItemDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Grants [`DarkSight`] while worn: the rangefinder helmet's sensor
/// suite, and whatever else ever names one.
///
/// A component on the item rather than a field the game copies onto the
/// wearer, so [`sync_dark_sight`](crate::droids::sync_dark_sight) can read
/// it off whatever is currently equipped without tracking which slot it
/// came from, and fold it together with a monster's own native radar and
/// whether either is jammed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WornDarkSight(pub i32);

/// The item definitions and the table of what lies on which deck, loaded
/// once, in `PreStartup`, by [`load_armory`].
///
/// `Clone` but not `Debug`: what an item lands when it is used is a list of
/// boxed effects, shared by handle, and a trait object has nothing to print.
#[derive(Resource, Clone)]
pub struct Armory {
    /// The item definitions, by id.
    pub defs: Registry<ItemDef>,
    /// What can be found on a deck, drawn by band, where the band is the
    /// deck number: `item_spawns.ron`, through the engine's loader.
    pub table: LootTable<Id<ItemDef>>,
    /// How good a found thing is at each deck, for the things that name an
    /// `enchant`: `levels.ron`.
    pub levels: LevelTable,
    /// Each definition's triggers, built once and shared by every item
    /// spawned from it.
    ///
    /// Built here rather than per item because an effect is a boxed trait
    /// object read out of RON: parsing a medkit's gel once a medkit is
    /// parsing it for every medkit on ten decks.
    triggers: Vec<Triggers>,
}

/// One item's own shape, checked against nothing but itself: whether its
/// slot claims make sense, and whether it names a weapon that runs on both
/// of the two economies at once, which neither `heat_on_struck` nor
/// `spend_ammo` is written to expect on the same entity.
fn validate_def(d: &ItemDef, _: &Registry<ItemDef>) -> Result<(), String> {
    if d.slot.is_none() && !d.also.is_empty() {
        return Err("also without a slot".into());
    }
    if d.stack && d.slot.is_some() {
        return Err("a worn item cannot stack".into());
    }
    if d.either && !d.also.is_empty() {
        return Err("either with also".into());
    }
    if d.heat.is_some() && d.ammo.is_some() {
        return Err("a weapon cannot run on both heat and ammo".into());
    }
    if d.consumable.is_some_and(|c| c.charges == 0) {
        return Err("a consumable with no charges is spent before it is found".into());
    }
    if d.throw.is_none() && d.triggers.iter().any(|t| t.on == "land") {
        return Err("a land trigger on a thing that cannot be thrown never lands".into());
    }
    if d.slot.is_none() && (d.pulse.is_some() || d.attuned || d.enchant.is_some()) {
        return Err("a pulse, an attunement or an enchant on a thing that is never worn does nothing".into());
    }
    if d.pulse.is_some() != d.triggers.iter().any(|t| t.on == "pulse") {
        return Err("a pulse needs a pulse trigger to do something, and a pulse trigger needs a pulse to come round".into());
    }
    if d.attuned && d.consumable.and_then(|c| c.recharge).is_none() {
        return Err("attuned, and nothing that refills".into());
    }
    if d.enchant.is_some_and(|e| e.most < 1) {
        return Err("an enchant that reaches no level".into());
    }
    Ok(())
}

impl Armory {
    /// Loads `items.ron` against `registries`, validates it, builds every
    /// item's triggers against the effect `kinds` and `moments` registered
    /// for the run, and builds the spawn table from `item_spawns.ron`;
    /// panics with every problem either file has, since a broken item file
    /// is a game that cannot start.
    ///
    /// The triggers are built here rather than when an item is spawned, so
    /// a grenade naming an effect or a moment nobody registered is caught
    /// while the file is read rather than by a grenade that quietly does
    /// nothing in the middle of a run.
    pub fn load(registries: &Registries, kinds: &EffectKinds, moments: &Moments) -> Self {
        let names = registries.names();
        let defs = load_defs(registries);
        let table =
            rl_engine::rl_rules::loot::load(ITEM_SPAWNS_RON, &names.clone().with("item", &defs), &defs, |d: &ItemDef| d.tags.iter().map(|t| t.id()).collect())
                .unwrap_or_else(|e| panic!("assets/item_spawns.ron: {e}"));
        let levels = rl_engine::rl_rules::loot::load_levels(LEVELS_RON).unwrap_or_else(|e| panic!("assets/levels.ron: {e}"));
        // ANCHOR: triggers
        // Each definition's triggers, built once against the moments and
        // effect kinds the run registered, and every problem in the file
        // named at once: a grenade that lands nothing is a typo.
        let mut triggers = Vec::new();
        let mut errors = Vec::new();
        for (_, d) in defs.iter() {
            triggers.push(match Triggers::build(&d.triggers, &d.effects, moments, kinds, &names) {
                Ok(t) => t,
                Err(mine) => {
                    errors.extend(mine.into_iter().map(|e| format!("{}: {e}", d.name)));
                    Triggers::default()
                }
            });
        }
        assert!(errors.is_empty(), "assets/items.ron: {}", errors.join("; "));
        // ANCHOR_END: triggers
        Self { defs, table, levels, triggers }
    }
}

/// Every item definition in `items.ron`, checked, and nothing built from
/// them: what the monster roster reads its drops' names against, as the
/// armory reads its own.
pub fn load_defs(registries: &Registries) -> Registry<ItemDef> {
    let defs: Registry<ItemDef> = registries.names().load(ITEMS_RON).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
    defs.validate(validate_def).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
    defs
}

/// The effect kinds a Foundry app has, for loading an [`Armory`] where no
/// app is running, as `foundry --prefabs` does and a test with no world
/// does: whatever `add_engine_effects` declares, and the `Ignite` and
/// `Emit` that `FirePlugin` and `GasPlugin` declare for the grenades,
/// which is what `main.rs` gives the real load. The throwaway `App` is
/// there for that and nothing else.
pub fn effect_kinds() -> EffectKinds {
    let mut app = App::new();
    app.add_engine_effects().add_effect::<rl_engine::rl_bevy::Ignite>().add_effect::<rl_engine::rl_bevy::Emit>();
    std::mem::take(&mut app.world_mut().resource_mut::<EffectKinds>())
}

/// Loads the [`Armory`] once, in `PreStartup`, before any run begins or is
/// continued, against the effects and moments the app registered.
///
/// Once rather than per use: every deck's scatter, every kill and every
/// crate used to read `items.ron` and build its triggers afresh, and the
/// save refreshed every turn needs it too.
pub fn load_armory(mut commands: Commands, registries: Res<Registries>, kinds: Res<EffectKinds>, moments: Res<Moments>) {
    commands.insert_resource(Armory::load(&registries, &kinds, &moments));
}

/// The armory and the registries it was read against, for the systems that
/// spawn items: one parameter rather than two, because every such system
/// wants both.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Content<'w> {
    armory: Res<'w, Armory>,
    registries: Res<'w, Registries>,
}

impl Content<'_> {
    /// The armory.
    pub fn armory(&self) -> &Armory {
        &self.armory
    }

    /// The registries, for whatever else a spawn needs them for.
    pub fn registries(&self) -> &Registries {
        &self.registries
    }
}

/// Where `d` is worn, or `None` for something never worn, such as a
/// stack of slugs.
///
/// `either` reaches for the named hands directly, since a one-handed
/// weapon that fits either one is not naming a single slot at all; `also`
/// folds each extra slot a two-hander claims onto the one it starts in.
pub fn shape_of(d: &ItemDef, registries: &Registries) -> Option<EquipShape> {
    if d.either {
        let main = registries.slots.expect("main hand");
        let off = registries.slots.expect("off hand");
        return Some(EquipShape::in_any([main, off]));
    }
    let mut shape = EquipShape::in_slot(d.slot?.id());
    for also in &d.also {
        shape = shape.and_claims(also.id());
    }
    Some(shape)
}

/// Marks an item with the definition it was made from, which is what a
/// save writes down and a continued run makes it again from.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemKind(pub Id<ItemDef>);

/// Spawns `id` as an item entity carrying every component its definition
/// implies: what it is called and drawn as, what it counts as, and, for
/// something worn, its shape, its armor, its resistances and whatever it
/// strikes or shoots with, and the `Heat` or `Ammo` it runs on. A
/// stackable item spawns as a stack of one; the caller merges or grows it
/// as it likes.
///
/// At `level`, which names it `cloak plate +2`, lands its effects at that
/// level and writes its clock with the level applied; a thing that names
/// no `enchant` is plain whatever `level` says, and one that does is held
/// between plain and its `most`, so a save written before the file
/// lowered a `most` never brings back a thing the file no longer allows.
pub fn spawn_item_at(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>, level: i32, registries: &Registries) -> Entity {
    let d = armory.defs.get(id);
    let level = d.enchant.map_or(0, |e| level.clamp(0, e.most));
    let enchanted = Enchanted { level, affixes: Vec::new() };
    let name = enchanted.display_name(&d.name, &Registry::<AffixDef>::default());
    let mut e = commands.spawn((Item, ItemKind(id), Name::new(name), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(2)));
    if d.enchant.is_some() {
        e.insert(Enchant(enchanted));
    }
    if !d.tags.is_empty() {
        e.insert(Tagged(d.tags.iter().map(|t| t.id()).collect::<Vec<TagId>>()));
    }
    // ANCHOR: use
    // What it does at its moments, a stim's use and a grenade's landing,
    // built once by the armory and shared by every copy; the engine lands
    // them. And what doing it costs the thing, which the engine spends.
    if let Some(triggers) = armory.triggers.get(id.index()).filter(|t| !t.0.is_empty()) {
        e.insert(triggers.clone());
    }
    if let Some(c) = d.consumable {
        let charges = Consumable::new(c.charges, c.when_empty);
        e.insert(match c.recharge {
            Some(every) => charges.recharging(every),
            None => charges,
        });
    }
    // ANCHOR_END: use
    if let Some(shape) = shape_of(d, registries) {
        e.insert(Wearable(shape));
    }
    if d.armor != 0 {
        e.insert(Armor(d.armor));
    }
    if !d.resists.is_empty() {
        let mut r = Resistances::new();
        for (kind, pct) in &d.resists {
            r.set(kind.id(), *pct);
        }
        e.insert(Resists(r));
    }
    if let Some(melee) = d.melee {
        e.insert(MeleeAttack { cost: d.cost, ..melee.attack() });
    }
    if let Some(ranged) = d.ranged {
        e.insert(RangedAttack { cost: d.cost, ..ranged.attack() });
    }
    if let Some(throw) = d.throw {
        e.insert(Throwable { effective: throw.effective, ..Throwable::new(throw.range, throw.strike.map(|(dice, kind)| (kind.id(), dice))) });
    }
    if let Some((per_shot, vent)) = d.heat {
        e.insert(Heat::new(per_shot, vent));
    }
    if let Some(tag) = d.ammo {
        e.insert(Ammo { tag: tag.id() });
    }
    if let Some(n) = d.dark_sight {
        e.insert(WornDarkSight(n));
    }
    if let Some(pulse) = d.pulse {
        e.insert(Pulse::every(pulse.at(level)));
    }
    if d.attuned {
        e.insert(Attuned);
    }
    if d.stack {
        e.insert(Stack { key: id.index() as u64, count: 1 });
    }
    e.id()
}

/// Spawns `id` plain, at `+0`: what the cheat menu puts in the pack, what
/// a stack is made as, and what a test hands the commando.
pub fn spawn_item(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>, registries: &Registries) -> Entity {
    spawn_item_at(commands, armory, id, 0, registries)
}

/// Spawns `count` of `id`: one stack of `count` for a thing that stacks,
/// and `count` of it otherwise, placed nowhere.
pub fn spawn_items(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>, count: u32, registries: &Registries) -> Vec<Entity> {
    if armory.defs.get(id).stack {
        let stack = spawn_item(commands, armory, id, registries);
        commands.entity(stack).insert(Stack { key: id.index() as u64, count: count.max(1) });
        return vec![stack];
    }
    (0..count).map(|_| spawn_item(commands, armory, id, registries)).collect()
}

// ANCHOR: maker
/// The engine's loot is made here: what lies on a deck when it is first
/// entered, what a kill leaves, and what a crate or a locker holds. A
/// deck's band is its number, and Foundry has no streamed surface. The
/// armory makes each thing, rolling a level from `levels.ron` at the band
/// it is found at for a thing that names an `enchant`.
impl ItemMaker for Armory {
    type Def = ItemDef;

    fn make(
        &self,
        commands: &mut Commands,
        registries: &Registries,
        def: Id<ItemDef>,
        count: u32,
        from: Provenance,
        rng: &mut rand::rngs::StdRng,
    ) -> Vec<Entity> {
        let d = self.defs.get(def);
        if d.stack {
            return spawn_items(commands, self, def, count, registries);
        }
        // Each its own roll: two plates from one crate are two finds.
        (0..count)
            .map(|_| {
                let level = d.enchant.map_or(0, |e| self.levels.roll(from.band, rng).min(e.most));
                spawn_item_at(commands, self, def, level, registries)
            })
            .collect()
    }

    fn id_of(&self, name: &str) -> Option<Id<ItemDef>> {
        self.defs.id(name)
    }

    fn table(&self) -> &LootTable<Id<ItemDef>> {
        &self.table
    }

    fn scatter(&self) -> ScatterRules {
        crate::loot::scatter_rules()
    }

    fn band(&self, area: LootArea) -> i32 {
        match area {
            LootArea::Place(map) => crate::decks::deck_of(map) as i32,
            LootArea::Region(_) => 0,
        }
    }
}
// ANCHOR_END: maker

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;
    use rl_engine::rl_core::{Point, RunSeed};

    use super::*;

    /// Spawns two items by name, through a fresh `Armory` and a raw
    /// `CommandQueue`, the way a test reaches into the world without a
    /// system of its own.
    fn spawn_two(app: &mut App, a: &str, b: &str) -> (Entity, Entity) {
        let registries = app.world().resource::<Registries>().clone();
        let armory = crate::testing::armory_of(app);
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let ea = spawn_item(&mut commands, &armory, armory.defs.expect(a), &registries);
        let eb = spawn_item(&mut commands, &armory, armory.defs.expect(b), &registries);
        queue.apply(app.world_mut());
        (ea, eb)
    }

    /// Spawns `name`, puts it in the player's bag, and equips it through
    /// the engine's own `Equip` intent, so wearing it fires the same
    /// `ItemEvent` the real game reacts to.
    fn wear(app: &mut App, name: &str) -> (Entity, Entity) {
        app.update();
        app.update();
        let registries = app.world().resource::<Registries>().clone();
        let armory = crate::testing::armory_of(app);
        let id = armory.defs.expect(name);
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let item = spawn_item(&mut commands, &armory, id, &registries);
        queue.apply(app.world_mut());
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(item);
        app.world_mut().write_message(Intent::new(player, Equip(item)));
        app.update();
        (player, item)
    }

    /// Takes `item` off `player` through the engine's own `Unequip` intent.
    fn unwear(app: &mut App, player: Entity, item: Entity) {
        app.world_mut().write_message(Intent::new(player, Unequip(item)));
        app.update();
    }

    #[test]
    fn every_item_loads_and_every_spawn_band_on_the_first_three_decks_has_something() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        assert_eq!(armory.defs.len(), 24, "sixteen things to carry, a slug, a keycard, a stim, a medkit and four grenades");
        assert!(armory.table.gaps(1..=3).is_empty(), "a deck with nothing to find");
    }

    #[test]
    fn a_weapons_file_numbers_reach_its_attack_and_a_two_hander_claims_the_off_hand() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (axe, carbine) = spawn_two(&mut app, "mono-axe", "blaster carbine");
        let world = app.world();
        let melee = world.get::<MeleeAttack>(axe).expect("the axe swings");
        assert_eq!(melee.cost, Some(140));
        let ranged = world.get::<RangedAttack>(carbine).expect("the carbine fires");
        assert_eq!((ranged.range, ranged.cost), (12, Some(100)));
        let shape = &world.get::<Wearable>(axe).unwrap().0;
        assert_eq!(shape.slots().count(), 2, "main hand and off hand");
    }

    /// The slug rifle is the pistol's economy at a rifle's reach: a slug a
    /// shot and dry on an empty bag, but a line past what the lamp shows,
    /// so it kills what the commando cannot yet see.
    #[test]
    fn a_slug_rifle_spends_a_slug_a_shot_and_reaches_past_the_lamp() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, rifle) = crate::testing::slug_gun_with(&mut app, "slug rifle", 2);
        let reach = app.world().get::<RangedAttack>(rifle).expect("loaded, so it has a shot").range;
        assert!(reach > crate::light::SHOULDER_LAMP.radius, "{reach} tiles, no further than the lamp");
        let struck = crate::testing::fire_at_a_target(&mut app, player, 3);
        assert_eq!(struck.len(), 2, "two slugs, two shots; the third finds nothing to fire");
        assert!(app.world().get::<RangedAttack>(rifle).is_none(), "dry");
    }

    /// The heavy repeater runs hot the fast way: its whole burst goes out
    /// in half the time a carbine's does, and then it is locked like any
    /// other energy weapon.
    #[test]
    fn a_heavy_repeater_gets_its_burst_off_in_half_the_time_a_carbine_takes() {
        let burst = |name: &str| {
            let mut app = crate::testing::headless(RunSeed(1));
            crate::testing::settle(&mut app);
            let player = crate::testing::empty_handed(&mut app);
            let gun = crate::testing::equip_new(&mut app, player, name);
            let before = crate::testing::clock(&app);
            let mut shots = 0;
            while app.world().get::<RangedAttack>(gun).is_some() {
                assert!(shots < 10, "{name} never locked");
                shots += crate::testing::fire_at_a_target(&mut app, player, 1).len();
            }
            (shots, crate::testing::clock(&app) - before)
        };
        let (repeater_shots, repeater_time) = burst("heavy repeater");
        let (carbine_shots, carbine_time) = burst("blaster carbine");
        assert_eq!(repeater_shots, carbine_shots, "the same five shots before it locks");
        assert!(repeater_time * 2 <= carbine_time, "{repeater_time} against the carbine's {carbine_time}");
    }

    /// What a suit resists is resisted by whoever wears it: twenty points
    /// of energy on a commando in composite plate meet flesh's quarter and
    /// the plate's tenth together, seven off, and then the plate's two
    /// points of armor, so eleven land. Nothing is copied onto the wearer
    /// to make that so, and the tenth leaves with the plate.
    #[test]
    fn composite_plate_resists_its_tenth_of_a_bolt_on_top_of_what_flesh_resists() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, plate) = wear(&mut app, "composite plate");
        let energy = app.world().resource::<Registries>().damage_kinds.expect("energy");
        let bolt = |app: &mut App| {
            app.world_mut().entity_mut(player).insert(Health::full(100));
            app.world_mut().write_message(DamageEvent::new(player, rl_engine::rl_rules::Hit::from_source(None, energy, 20)));
            app.update();
            100 - health(app, player)
        };
        assert_eq!(bolt(&mut app), 20 - 7 - 2, "a quarter and a tenth resisted, then two points of plate");
        unwear(&mut app, player, plate);
        assert_eq!(bolt(&mut app), 20 - 5, "and taken off, flesh's quarter alone");
    }

    #[test]
    fn wearing_the_rangefinder_gives_dark_sight_and_taking_it_off_takes_it_away() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, helmet) = wear(&mut app, "rangefinder helmet");
        assert_eq!(app.world().get::<DarkSight>(player).map(|d| d.0), Some(6));
        unwear(&mut app, player, helmet);
        assert_eq!(app.world().get::<DarkSight>(player), None);
    }

    /// The reviewer's own reproduction, Fix round 1, finding 1: a jam that
    /// stored the radius it took away and a `grant_dark_sight` that wrote
    /// `DarkSight` from gear alone fought over the same component the
    /// moment a jammed wearer took the helmet off, and the wearer kept
    /// radar for good with no helmet on. `DarkSight` derived fresh every
    /// pass by `sync_dark_sight` has nothing left to fight over: with the
    /// helmet gone, there is nothing for it to read once the jam clears.
    #[test]
    fn a_jammed_wearer_who_takes_the_helmet_off_has_no_dark_sight_once_the_jam_clears() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, helmet) = wear(&mut app, "rangefinder helmet");
        assert_eq!(app.world().get::<DarkSight>(player).map(|d| d.0), Some(6));
        crate::testing::hit(&mut app, player, "ion", 1);
        assert_eq!(app.world().get::<DarkSight>(player), None, "jammed");
        unwear(&mut app, player, helmet);
        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(app.world().get::<DarkSight>(player), None, "no helmet, so nothing to see by once the jam clears either");
    }

    #[test]
    fn a_jammed_wearer_who_swaps_helmets_gets_the_new_ones_radius_once_the_jam_clears() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, old_helmet) = wear(&mut app, "rangefinder helmet");
        crate::testing::hit(&mut app, player, "ion", 1);
        assert_eq!(app.world().get::<DarkSight>(player), None, "jammed");
        unwear(&mut app, player, old_helmet);
        // A second sensor helmet, spawned by hand at a radius nothing in
        // the roster names, so a leftover 6 from the old one could never
        // be mistaken for the new one's own 9.
        let head = app.world().resource::<Registries>().slots.expect("head");
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let new_helmet = commands.spawn((Item, Wearable(EquipShape::in_slot(head)), WornDarkSight(9))).id();
        queue.apply(app.world_mut());
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(new_helmet);
        app.world_mut().write_message(Intent::new(player, Equip(new_helmet)));
        app.update();
        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(app.world().get::<DarkSight>(player).map(|d| d.0), Some(9), "the new helmet's own radius, not the old one's");
    }

    /// A definition with fields left at their most inert: no slot, no
    /// attack, no economy, nothing to spawn with. Tests that care about one
    /// property override just that field, rather than restating all
    /// twenty-three every time.
    fn blank_def(name: &str) -> ItemDef {
        ItemDef {
            name: name.to_string(),
            glyph: '?',
            color: (0.0, 0.0, 0.0),
            slot: None,
            either: false,
            also: Vec::new(),
            tags: Vec::new(),
            effects: Vec::new(),
            triggers: Vec::new(),
            consumable: None,
            armor: 0,
            resists: Vec::new(),
            melee: None,
            ranged: None,
            throw: None,
            cost: None,
            heat: None,
            ammo: None,
            dark_sight: None,
            pulse: None,
            attuned: false,
            enchant: None,
            stack: false,
        }
    }

    /// The commando, wounded by `hurt`, alone on a quiet deck one: the
    /// droids and the rats are taken off it, because the only thing that
    /// may move this player's health over the turns a test waits is what
    /// the test gave them.
    fn alone_and_wounded(seed: u64, hurt: i32) -> (App, Entity) {
        let mut app = crate::testing::headless(RunSeed(seed));
        crate::testing::arrive_on(&mut app, 1);
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let others: Vec<Entity> = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<Entity, (With<Actor>, Without<Player>)>();
            q.iter(world).collect()
        };
        for other in others {
            app.world_mut().entity_mut(other).despawn();
        }
        // Put at the wound rather than dealt one: `testing::hit` writes the
        // report a blow ends in, which reacts but never takes a point off,
        // and a real blow would drag armor, resistances and a profile in
        // between the test and the thing it is measuring.
        {
            let mut health = app.world_mut().get_mut::<Health>(player).expect("the commando has health");
            health.current = health.max - hurt;
        }
        (app, player)
    }

    /// Puts `count` of `name` in the player's bag and returns the stack.
    fn carry(app: &mut App, player: Entity, name: &str, count: u32) -> Entity {
        let registries = app.world().resource::<Registries>().clone();
        let armory = crate::testing::armory_of(app);
        let id = armory.defs.expect(name);
        let item = {
            let mut queue = CommandQueue::default();
            let mut commands = Commands::new(&mut queue, app.world_mut());
            let item = spawn_item(&mut commands, &armory, id, &registries);
            commands.entity(item).insert(Stack { key: id.index() as u64, count });
            queue.apply(app.world_mut());
            item
        };
        app.world_mut().entity_mut(player).insert(Inventory { items: vec![item] });
        app.update();
        item
    }

    /// What the commando has left.
    fn health(app: &App, who: Entity) -> i32 {
        app.world().get::<Health>(who).expect("the commando has health").current
    }

    /// A stim closes a wound on the spot and the shot is gone with it.
    ///
    /// Nothing of Foundry's own runs here: the item's `use` trigger lands
    /// the mend where the commando stands, and its one charge takes one off
    /// the stack. The roll is `2d4+5`, so the
    /// gain is between seven and thirteen whatever the dice say, which is
    /// what makes ten its midpoint and the medkit's twenty twice it.
    #[test]
    fn a_stim_closes_a_wound_at_once_and_the_shot_is_spent() {
        let (mut app, player) = alone_and_wounded(3, 20);
        let stim = carry(&mut app, player, "stim", 2);
        let before = health(&app, player);

        app.world_mut().write_message(Intent::new(player, UseItem(stim)));
        crate::testing::settle(&mut app);

        let gained = health(&app, player) - before;
        assert!((7..=13).contains(&gained), "2d4+5 closed at once, not {gained}");
        assert_eq!(app.world().get::<Stack>(stim).map(|s| s.count), Some(1), "one shot off the stack, spent using it");
    }

    /// A thing in the bag is not something the commando knows how to do.
    ///
    /// The stim and the medkit were abilities for a day, granted by the item
    /// that carried them, and the abilities screen listed both beside the
    /// one thing the commando had actually been taught. What a used thing
    /// does is on the thing now, so `Known` is what the run has learned and
    /// nothing else.
    #[test]
    fn carrying_the_medical_pair_teaches_the_commando_nothing() {
        let (mut app, player) = alone_and_wounded(6, 10);
        carry(&mut app, player, "stim", 1);
        app.update();
        assert!(app.world().get::<Known>(player).is_none_or(|k| k.iter().count() == 0), "a stim in the bag is not an ability");
        assert!(!crate::testing::knows(&app, player, "stims"), "and the upgrade's own is still unlearned");
    }

    /// A medkit is worth twice a stim and pays it two a turn over ten,
    /// which is ten turns of a deck the commando has to live through.
    ///
    /// Ten turns exactly: the tenth mends and the eleventh does not, so
    /// the gain is `MEND_PER_TURN * 10` and no more. The status is the
    /// engine's own, ticking through the damage pipeline a wound arrives
    /// by, so plate and resistances have nothing to say about it.
    #[test]
    fn a_medkit_is_worth_twice_a_stim_spread_over_ten_turns() {
        let (mut app, player) = alone_and_wounded(4, 25);
        let medkit = carry(&mut app, player, "medkit", 1);
        let before = health(&app, player);
        let mending = app.world().resource::<Registries>().statuses.expect("mending");

        app.world_mut().write_message(Intent::new(player, UseItem(medkit)));
        crate::testing::settle(&mut app);
        assert!(app.world().get::<Afflicted>(player).is_some_and(|a| a.0.has(mending)), "the gel is working");
        assert_eq!(health(&app, player) - before, crate::content::MEND_PER_TURN, "the first of the ten turns is the turn it is used");

        crate::testing::pass_turns(&mut app, 9);
        let gained = health(&app, player) - before;
        assert_eq!(gained, crate::content::MEND_PER_TURN * 10, "two a turn for ten turns, twice a stim's ten");
        assert!(!app.world().get::<Afflicted>(player).is_some_and(|a| a.0.has(mending)), "and it is done");

        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(health(&app, player) - before, gained, "nothing after the ten");
        assert!(app.world().get_entity(medkit).is_err(), "the last kit off the stack goes with it");
    }

    /// A second medkit starts the ten turns again rather than mending
    /// twice as fast: `mending` refreshes, which is the engine's default
    /// and the rule that keeps a pack of kits a longer recovery instead of
    /// a bigger one.
    #[test]
    fn a_second_medkit_starts_the_ten_turns_again_rather_than_doubling_the_rate() {
        let (mut app, player) = alone_and_wounded(5, 25);
        let kits = carry(&mut app, player, "medkit", 2);
        let before = health(&app, player);

        app.world_mut().write_message(Intent::new(player, UseItem(kits)));
        crate::testing::settle(&mut app);
        crate::testing::pass_turns(&mut app, 2);
        // The turn it went on and the two after it: three turns of gel.
        let one_kit = health(&app, player) - before;
        assert_eq!(one_kit, crate::content::MEND_PER_TURN * 3, "two a turn from the turn it went on");

        app.world_mut().write_message(Intent::new(player, UseItem(kits)));
        crate::testing::settle(&mut app);
        crate::testing::pass_turns(&mut app, 2);
        assert_eq!(health(&app, player) - before, one_kit + crate::content::MEND_PER_TURN * 3, "still two a turn with the second kit in, not four");
    }

    /// Throws one of the grenades `player` carries at `aim`, through the
    /// engine's own throw action, the way the pack's throw key does.
    fn throw_grenade(app: &mut App, player: Entity, grenade: Entity, aim: Point) {
        app.world_mut().write_message(Intent::new(player, Throw { item: grenade, at: aim }));
        crate::testing::settle(app);
    }

    /// Every item lying on `at`.
    fn lying_at(app: &mut App, at: Point) -> Vec<Entity> {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(Entity, &Position), With<Item>>();
        q.iter(world).filter(|(_, p)| p.0 == at).map(|(e, _)| e).collect()
    }

    fn at(app: &App, e: Entity) -> Point {
        app.world().get::<Position>(e).expect("it stands somewhere").0
    }

    /// A frag grenade is thrown and lands like a blast: the droid it is
    /// thrown at is hurt, the grenade that did it is one fewer in the bag,
    /// and nothing is left lying where it burst.
    #[test]
    fn a_frag_grenade_bursts_on_the_droid_it_is_thrown_at_and_is_spent_doing_it() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_down_a_lane(&mut app, "line droid", 4, 4);
        let frags = carry(&mut app, player, "frag grenade", 2);
        let before = health(&app, droid);
        let aim = at(&app, droid);
        throw_grenade(&mut app, player, frags, aim);
        assert!(app.world().get::<Health>(droid).is_none_or(|h| h.current < before), "the blast reached it");
        assert_eq!(app.world().get::<Stack>(frags).map(|s| s.count), Some(1), "one grenade off the stack");
        assert!(lying_at(&mut app, aim).is_empty(), "and the one thrown is spent, not lying there");
    }

    /// An incendiary is a fire the commando starts: the deck plate it lands
    /// on has nothing to burn, and burns anyway for the turns it names.
    #[test]
    fn an_incendiary_leaves_the_deck_it_lands_on_burning() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_down_a_lane(&mut app, "line droid", 4, 4);
        let grenade = carry(&mut app, player, "incendiary grenade", 1);
        let aim = at(&app, droid);
        throw_grenade(&mut app, player, grenade, aim);
        assert!(app.world().resource::<Fire>().is_burning(aim), "alight where it landed");
        crate::testing::pass_turns(&mut app, 1);
        assert!(app.world().resource::<Fire>().is_burning(aim), "and still burning a turn later");
    }

    /// An ion grenade blinds every radar in the burst at once, not only the
    /// one it was thrown at.
    #[test]
    fn an_ion_grenade_jams_every_radar_in_its_burst() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (first, player) = crate::testing::droid_down_a_lane(&mut app, "probe droid", 4, 4);
        let beside = at(&app, first).offset(0, 1);
        let floor = app.world().resource::<WorldMap>().tile(at(&app, first)).expect("the lane is loaded");
        app.world_mut().resource_mut::<WorldMap>().set_tile(beside, floor);
        let registries = app.world().resource::<Registries>().clone();
        let roster = crate::droids::Roster::load(&registries);
        let map = app.world().resource::<WorldMap>().current();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let second = crate::droids::spawn_monster(&mut commands, &roster, roster.defs.expect("probe droid"), beside, map, &registries);
        queue.apply(app.world_mut());
        let grenade = carry(&mut app, player, "ion grenade", 1);
        for probe in [first, second] {
            assert!(app.world().get::<DarkSight>(probe).is_some(), "radar up before the throw");
        }
        let aim = at(&app, first);
        throw_grenade(&mut app, player, grenade, aim);
        for probe in [first, second] {
            assert_eq!(app.world().get::<DarkSight>(probe), None, "{probe:?} jammed");
        }
    }

    /// A smoke grenade is a burst of smoke big enough to fight in, and does
    /// no harm. On open deck it hides a room's worth of cells the moment it
    /// lands, still hides some of it nine turns on, and then thins away.
    #[test]
    fn a_smoke_grenade_hides_a_room_s_worth_of_deck_for_ten_turns_then_thins() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_down_a_lane(&mut app, "line droid", 5, 5);
        crate::testing::clear_droids(&mut app, &[droid]);
        let (aim, before) = (at(&app, droid), health(&app, droid));
        let floor = app.world().resource::<WorldMap>().tile(aim).expect("the lane is loaded");
        {
            let mut map = app.world_mut().resource_mut::<WorldMap>();
            for dy in -9..=9 {
                for dx in -9..=9 {
                    map.set_tile(aim.offset(dx, dy), floor);
                }
            }
        }
        let grenade = carry(&mut app, player, "smoke grenade", 1);
        throw_grenade(&mut app, player, grenade, aim);
        let registries = app.world().resource::<Registries>();
        let smoke = registries.gases.expect("smoke");
        let veils_at = registries.gases.get(smoke).veils_at.expect("smoke hides what is behind it");
        let hidden = |app: &App| app.world().resource::<Gases>().cells(smoke).filter(|(_, c)| *c >= veils_at).count();
        let landed = hidden(&app);
        assert!(landed >= 25, "a room's worth hidden where it landed, not {landed} cells");
        assert_eq!(health(&app, droid), before, "and nobody hurt by it");
        // Out of the way, so nine turns of waiting in the open are nine
        // turns nobody is shot in.
        app.world_mut().despawn(droid);
        crate::testing::pass_turns(&mut app, 9);
        assert!(hidden(&app) > 0, "still hiding some of the deck nine turns on");
        crate::testing::pass_turns(&mut app, 40);
        assert_eq!(app.world().resource::<Gases>().cells(smoke).count(), 0, "and thinned away to nothing");
    }

    /// A stim is used, never thrown: a use trigger, and no throw in the file,
    /// so the pack offers the one and not the other.
    #[test]
    fn a_stim_has_a_use_trigger_and_no_throw() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let stim = armory.defs.get(armory.defs.expect("stim"));
        assert!(stim.throw.is_none());
        assert_eq!(stim.triggers.iter().map(|t| t.on.as_str()).collect::<Vec<_>>(), vec!["use"]);
    }

    #[test]
    fn the_plates_load_with_their_clock_their_charge_and_their_level() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let nanite = armory.defs.get(armory.defs.expect("nanite plate"));
        assert_eq!(nanite.pulse.map(|p| (p.every, p.per_level, p.fastest)), Some((1000, -100, 100)));
        assert_eq!(nanite.enchant.map(|e| e.most), Some(9));
        let cloak = armory.defs.get(armory.defs.expect("cloak plate"));
        assert!(cloak.attuned);
        assert_eq!(cloak.consumable.and_then(|c| c.recharge), Some(4000));
    }

    #[test]
    fn a_pulse_or_an_attunement_or_an_enchant_on_a_thing_never_worn_fails_to_validate() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let mut d = blank_def("loose pulse");
        d.pulse = Some(PulseDef { every: 800, per_level: 0, fastest: 800 });
        assert!(validate_def(&d, &armory.defs).is_err(), "a pulse on nothing worn");
        let mut d = blank_def("loose attunement");
        d.attuned = true;
        assert!(validate_def(&d, &armory.defs).is_err(), "attuned and never worn");
        let mut d = blank_def("loose enchant");
        d.enchant = Some(EnchantDef { most: 2 });
        assert!(validate_def(&d, &armory.defs).is_err(), "an enchant on nothing worn");
        // An enchant on a stack is refused either way it is written: worn,
        // a worn thing cannot stack; not worn, the enchant does nothing.
        let mut d = blank_def("a stack of fine things");
        (d.enchant, d.stack) = (Some(EnchantDef { most: 2 }), true);
        assert!(validate_def(&d, &armory.defs).is_err(), "an enchanted stack, not worn");
        d.slot = Some(rl_engine::rl_core::Id::from_raw(0).into());
        assert!(validate_def(&d, &armory.defs).is_err(), "an enchanted stack, worn");
    }

    /// Worn, each still has to hold up its own end: a clock with nothing
    /// to do on it, a pulse trigger with no clock to fire it, an attunement
    /// on a thing that never refills, and an enchant that reaches no level
    /// are each a typo in the file rather than a plate.
    #[test]
    fn a_worn_pulse_without_its_trigger_or_an_attunement_with_nothing_to_refill_fails_to_validate() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let worn = |name: &str| ItemDef { slot: Some(rl_engine::rl_core::Id::from_raw(0).into()), ..blank_def(name) };
        let mut d = worn("idle clock");
        d.pulse = Some(PulseDef { every: 800, per_level: 0, fastest: 800 });
        assert!(validate_def(&d, &armory.defs).is_err(), "a pulse and no pulse trigger");
        let mut d = worn("no clock");
        d.triggers = armory.defs.get(armory.defs.expect("nanite plate")).triggers.clone();
        assert!(validate_def(&d, &armory.defs).is_err(), "a pulse trigger and no pulse");
        let mut d = worn("never refills");
        d.attuned = true;
        d.consumable = Some(ConsumableDef { charges: 1, when_empty: WhenEmpty::Kept, recharge: None });
        assert!(validate_def(&d, &armory.defs).is_err(), "attuned and nothing that refills");
        let mut d = worn("plain for good");
        d.enchant = Some(EnchantDef { most: 0 });
        assert!(validate_def(&d, &armory.defs).is_err(), "an enchant that reaches no level");
        let mut d = worn("a fine plate");
        d.enchant = Some(EnchantDef { most: 3 });
        assert!(validate_def(&d, &armory.defs).is_ok(), "and a worn enchant that reaches a level is fine");
    }

    /// The nanite plate knits a point back every ten turns worn, counted
    /// from when it went on.
    #[test]
    fn the_nanite_plate_mends_a_point_every_ten_turns_worn() {
        let (mut app, player) = alone_and_wounded(11, 10);
        let before = health(&app, player);
        crate::testing::equip_new(&mut app, player, "nanite plate");
        crate::testing::pass_turns(&mut app, 8);
        assert_eq!(health(&app, player), before, "not yet");
        crate::testing::pass_turns(&mut app, 2);
        assert_eq!(health(&app, player), before + 1, "a point in ten turns");
    }

    /// Every `DamageDealt` on anyone, recorded as it is written: a headless
    /// app rotates its message buffers on wall time, so draining the queue
    /// afterwards can find it already empty and prove nothing.
    #[derive(Resource, Default)]
    struct Dealt(Vec<(Entity, i32)>);

    fn record_dealt(mut dealt: MessageReader<DamageDealt>, mut out: ResMut<Dealt>) {
        out.0.extend(dealt.read().map(|d| (d.target, d.dealt)));
    }

    /// Worn at full health it mends nothing and says nothing: a whole
    /// commando's log is not a list of mends.
    #[test]
    fn the_nanite_plate_on_a_whole_commando_says_nothing() {
        let (mut app, player) = alone_and_wounded(12, 0);
        app.init_resource::<Dealt>().add_systems(PostUpdate, record_dealt);
        crate::testing::equip_new(&mut app, player, "nanite plate");
        crate::testing::pass_turns(&mut app, 21);
        let mine: Vec<i32> = app.world().resource::<Dealt>().0.iter().filter(|(t, _)| *t == player).map(|(_, d)| *d).collect();
        assert_eq!(mine.len(), 2, "two pulses came round in twenty-one turns: {mine:?}");
        assert!(mine.iter().all(|d| *d == 0), "and each mended nothing, so the narrator says nothing: {mine:?}");
    }

    /// A `+2` nanite plate comes round every eight turns, a `+9` one every
    /// turn, and nothing past `+9` any faster.
    #[test]
    fn a_nanite_plate_quickens_with_its_level_to_every_turn_at_plus_nine() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let nanite = armory.defs.get(armory.defs.expect("nanite plate")).pulse.unwrap();
        assert_eq!((nanite.at(0), nanite.at(2), nanite.at(9), nanite.at(12)), (1000, 800, 100, 100));
    }

    /// Every attacker's swing, recorded as it is written: a miss spends no
    /// health, and a test that read health would take a miss for a droid
    /// that never swung.
    #[derive(Resource, Default)]
    struct Swung(Vec<Entity>);

    fn record_swings(mut struck: MessageReader<Struck>, mut swung: ResMut<Swung>) {
        swung.0.extend(struck.read().map(|s| s.attacker));
    }

    /// The cloak plate is empty when it goes on and charged forty turns
    /// later; used then, a droid alert to the commando one step away never
    /// swings at them while it lasts.
    #[test]
    fn the_cloak_plate_hides_the_commando_from_an_adjacent_droid_once_it_has_charged() {
        let (mut app, player) = alone_and_wounded(15, 0);
        let plate = crate::testing::equip_new(&mut app, player, "cloak plate");
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(0), "empty when it went on");
        crate::testing::pass_turns(&mut app, 40);
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(1), "charged after forty turns worn");
        // Only now the droid, so forty turns of waiting are not forty turns
        // of being hit.
        let (droid, _) = crate::testing::droid_facing_player(&mut app, "line droid", 1);
        crate::testing::alert(&mut app, droid, player);
        app.init_resource::<Swung>().add_systems(PostUpdate, record_swings);
        app.world_mut().write_message(Intent::new(player, UseItem(plate)));
        crate::testing::settle(&mut app);
        assert!(app.world().get::<Unseen>(player).is_some(), "cloaked");
        crate::testing::pass_turns(&mut app, 3);
        assert!(!app.world().resource::<Swung>().0.contains(&droid), "never swung at while cloaked");
    }

    /// How many turns the commando stays unseen after using a charged
    /// cloak plate at `level`, alone on a quiet deck so nothing ends it
    /// early, counted in waits until `Unseen` is gone.
    fn turns_cloaked_by_a_plate_at(level: i32) -> u32 {
        let (mut app, player) = alone_and_wounded(13, 0);
        let registries = app.world().resource::<Registries>().clone();
        let armory = crate::testing::armory_of(&app);
        let plate = {
            let mut queue = CommandQueue::default();
            let mut commands = Commands::new(&mut queue, app.world_mut());
            let plate = spawn_item_at(&mut commands, &armory, armory.defs.expect("cloak plate"), level, &registries);
            queue.apply(app.world_mut());
            plate
        };
        let named = if level == 0 { "cloak plate".to_string() } else { format!("cloak plate +{level}") };
        assert_eq!(app.world().get::<Name>(plate).map(|n| n.as_str().to_string()), Some(named));
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(plate);
        app.world_mut().write_message(Intent::new(player, Equip(plate)));
        app.update();
        app.world_mut().get_mut::<Consumable>(plate).unwrap().left = 1;
        app.world_mut().write_message(Intent::new(player, UseItem(plate)));
        crate::testing::settle(&mut app);
        assert!(app.world().get::<Unseen>(player).is_some(), "unseen once the plate is used");
        let mut turns = 0;
        while app.world().get::<Unseen>(player).is_some() {
            assert!(turns < 60, "a +{level} cloak that never wore off");
            crate::testing::pass_turns(&mut app, 1);
            turns += 1;
        }
        turns
    }

    /// Each level of a cloak plate hides the commando two turns longer: a
    /// `+2` lasts four turns past a plain one's ten.
    ///
    /// Counted as waits after the use settles, the plain plate's ten come
    /// to nine: the status ticks once in the pass it lands, which is the
    /// use's own turn, so the tenth turn is the one it was used on.
    #[test]
    fn a_cloak_plates_level_adds_two_turns_each() {
        let plain = turns_cloaked_by_a_plate_at(0);
        let plus_two = turns_cloaked_by_a_plate_at(2);
        assert_eq!(plain, 9, "a plain cloak's ten turns, the first of them the use's own");
        assert_eq!(plus_two, plain + 4, "two more turns for each of two levels");
    }

    /// The commando's `@` fades the pass the cloak goes on and is white
    /// again the pass it wears off.
    #[test]
    fn the_commando_is_drawn_faded_while_unseen() {
        let (mut app, player) = alone_and_wounded(14, 0);
        let fg = |app: &App| app.world().get::<Glyph>(player).map(|g| g.fg);
        assert_eq!(fg(&app), Some(crate::run::COMMANDO));
        let cloaked = app.world().resource::<Registries>().statuses.expect("cloaked");
        app.world_mut().write_message(Afflict { target: player, status: cloaked, turns: 2, by: None });
        crate::testing::pass_turns(&mut app, 1);
        assert_eq!(fg(&app), Some(crate::run::COMMANDO_UNSEEN), "faded while unseen");
        crate::testing::pass_turns(&mut app, 3);
        assert_eq!(fg(&app), Some(crate::run::COMMANDO), "and white once it wore off");
    }

    /// A stim is no attack and leaves a cloaked commando unseen; a frag
    /// grenade thrown at a droid is one, and the commando is seen again in
    /// the pass it bursts, through the same throw the pack's key makes.
    #[test]
    fn a_stim_leaves_the_commando_cloaked_and_a_grenade_thrown_at_a_droid_ends_it() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (droid, player) = crate::testing::droid_down_a_lane(&mut app, "line droid", 4, 4);
        let frags = carry(&mut app, player, "frag grenade", 1);
        crate::testing::pick(&mut app, player, crate::upgrades::Upgrade::Stims);
        let stims = app.world().resource::<Abilities>().expect("stims");
        let cloaked = app.world().resource::<Registries>().statuses.expect("cloaked");
        app.world_mut().write_message(Afflict { target: player, status: cloaked, turns: 10, by: None });
        crate::testing::pass_turns(&mut app, 1);
        assert!(app.world().get::<Unseen>(player).is_some(), "cloaked");

        let here = at(&app, player);
        app.world_mut().write_message(Intent::new(player, Use { ability: stims, aim: here }));
        crate::testing::settle(&mut app);
        let cooling = app.world().get::<Cooldowns>(player).map(|c| c.ready_at(stims));
        assert!(cooling.is_some_and(|t| t > 0), "the stim was taken, so it is cooling: {cooling:?}");
        assert!(app.world().get::<Unseen>(player).is_some(), "a stim in the arm is not an attack");

        let aim = at(&app, droid);
        throw_grenade(&mut app, player, frags, aim);
        assert!(app.world().get::<Unseen>(player).is_none(), "a grenade at a droid is");
    }

    /// What `make` hands back, each thing with the level it was made at,
    /// read off its `Enchant`, and its name.
    fn made(armory: &Armory, registries: &Registries, name: &str, count: u32, band: i32, rng: &mut rand::rngs::StdRng) -> Vec<(Option<i32>, String)> {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let from = Provenance { found: rl_engine::rl_bevy::Found::Container, band };
        let things = {
            let mut commands = Commands::new(&mut queue, &world);
            armory.make(&mut commands, registries, armory.defs.expect(name), count, from, rng)
        };
        queue.apply(&mut world);
        things
            .into_iter()
            .map(|e| (world.get::<Enchant>(e).map(|x| x.0.level), world.get::<Name>(e).map(|n| n.as_str().to_string()).unwrap_or_default()))
            .collect()
    }

    /// A cloak plate found at band 10, over a span of seeds, is always
    /// `+2` to `+5`, every one of the four turns up, and it carries its
    /// level on it and in its name.
    #[test]
    fn a_plate_made_at_band_ten_is_plus_two_to_plus_five_and_says_so() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..200 {
            let mut rng = rand::SeedableRng::seed_from_u64(seed);
            let [(level, name)] = made(&armory, &r, "cloak plate", 1, 10, &mut rng).try_into().expect("one plate");
            let level = level.expect("an enchantable thing carries its level");
            assert!((2..=5).contains(&level), "seed {seed}: +{level} at band 10");
            assert_eq!(name, format!("cloak plate +{level}"));
            seen.insert(level);
        }
        assert_eq!(seen.into_iter().collect::<Vec<_>>(), vec![2, 3, 4, 5], "every level the row names turns up");
    }

    /// Two plates from one `make` are two finds, each rolled for itself.
    #[test]
    fn two_plates_made_together_roll_their_levels_apart() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let differ = (0..50).any(|seed| {
            let mut rng = rand::SeedableRng::seed_from_u64(seed);
            let pair = made(&armory, &r, "cloak plate", 2, 10, &mut rng);
            pair[0].0 != pair[1].0
        });
        assert!(differ, "fifty pairs and never two levels: the pair shares one roll");
    }

    /// A thing's `most` caps what the table hands it: a plate that reaches
    /// only `+2` is never found better, even where the row runs to `+5`.
    #[test]
    fn a_things_most_caps_the_level_it_is_made_at() {
        let r = crate::content::registries();
        let mut armory = crate::testing::armory(&r);
        let defs = armory
            .defs
            .iter()
            .map(|(_, d)| {
                let mut d = d.clone();
                if d.name == "cloak plate" {
                    d.enchant = Some(EnchantDef { most: 2 });
                }
                d
            })
            .collect();
        armory.defs = Registry::from_defs(defs).expect("the same names");
        let levels: Vec<i32> = (0..100)
            .map(|seed| {
                let mut rng = rand::SeedableRng::seed_from_u64(seed);
                made(&armory, &r, "cloak plate", 1, 10, &mut rng)[0].0.expect("a level")
            })
            .collect();
        assert!(levels.iter().all(|l| *l == 2), "capped at +2: {levels:?}");
    }

    /// A thing that names no enchant, or a stack, is made plain and draws
    /// nothing from the stream, so adding enchants to plates did not shift
    /// what every other find on a deck rolls.
    #[test]
    fn a_plain_or_stacking_thing_is_made_without_drawing_from_the_stream() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let (mut a, mut b): (rand::rngs::StdRng, rand::rngs::StdRng) = (rand::SeedableRng::seed_from_u64(4), rand::SeedableRng::seed_from_u64(4));
        let pistols = made(&armory, &r, "slug pistol", 2, 10, &mut a);
        let stims = made(&armory, &r, "stim", 3, 10, &mut a);
        assert!(pistols.iter().chain(&stims).all(|(level, _)| level.is_none()), "plain: {pistols:?} {stims:?}");
        assert_eq!(rand::Rng::random::<u64>(&mut a), rand::Rng::random::<u64>(&mut b), "the stream is where it was");
    }

    /// A level past a thing's `most`, such as one a save wrote before the
    /// file lowered it, comes back at the most the file allows.
    #[test]
    fn a_level_past_most_is_spawned_at_most() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let plate = {
            let mut commands = Commands::new(&mut queue, &world);
            spawn_item_at(&mut commands, &armory, armory.defs.expect("cloak plate"), 12, &r)
        };
        queue.apply(&mut world);
        assert_eq!(world.get::<Enchant>(plate).map(|e| e.0.level), Some(9));
        assert_eq!(world.get::<Name>(plate).map(|n| n.as_str().to_string()), Some("cloak plate +9".to_string()));
    }

    #[test]
    fn a_weapon_naming_both_heat_and_ammo_fails_to_validate() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let mut d = blank_def("double economy");
        d.heat = Some((10, 10));
        d.ammo = Some(rl_engine::rl_core::Id::from_raw(0).into());
        assert!(validate_def(&d, &armory.defs).is_err(), "heat and ammo on the same weapon");
    }
}
