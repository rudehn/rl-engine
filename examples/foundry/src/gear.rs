//! Gear: the weapons, the armor, the slugs, the medical pair and the
//! grenades that arm the foundry's decks, loaded once from `items.ron`,
//! found where `item_spawns.ron` says, and spawned as items an actor can
//! find, carry, wear and use.
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
use rl_engine::rl_core::{DiceRoll, Id};
use rl_engine::rl_render::Glyph;
use rl_engine::rl_rules::{BandedEntry, BandedTable, DamageKind, EquipShape, NameRef, Named, Registry, Resistances, SlotDef, TagDef, TagId};
use rl_engine::rl_rules::{EffectSpec, TriggerSpec};
use serde::Deserialize;

use crate::ammo::Ammo;
use crate::content::{MeleeDef, RangedDef};
use crate::heat::Heat;

const ITEMS_RON: &str = include_str!("../assets/items.ron");
const ITEM_SPAWNS_RON: &str = include_str!("../assets/item_spawns.ron");

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
    /// The roll and damage kind it strikes whoever it hits with.
    #[serde(default)]
    pub strike: Option<(DiceRoll, NameRef<DamageKind>)>,
}

impl Named for ItemDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// One row of `item_spawns.ron`: an item, the decks it lies about on, and
/// how often.
///
/// Its own file rather than a field of [`ItemDef`], for the reason
/// [`MonsterSpawn`](crate::droids::MonsterSpawn) is: what a thing is and
/// where it is found are read and tuned apart.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ItemSpawn {
    /// Which item.
    pub item: NameRef<ItemDef>,
    /// The first and last deck it applies on, both included.
    pub decks: (i32, i32),
    /// How often, against every other row that applies on a deck.
    pub weight: u32,
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

/// The item definitions and the table of what lies on which deck.
///
/// `Clone` but not `Debug`: what an item lands when it is used is a list of
/// boxed effects, shared by handle, and a trait object has nothing to print.
#[derive(Clone)]
pub struct Armory {
    /// The item definitions, by id.
    pub defs: Registry<ItemDef>,
    /// What can be found on a deck, drawn by band, where the band is the
    /// deck number.
    pub table: BandedTable<Id<ItemDef>>,
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
        let defs: Registry<ItemDef> = names.load(ITEMS_RON).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
        defs.validate(validate_def).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
        let rows: Vec<ItemSpawn> = names.clone().with("item", &defs).load_list(ITEM_SPAWNS_RON).unwrap_or_else(|e| panic!("assets/item_spawns.ron: {e}"));
        let table = BandedTable::new(rows.iter().map(|r| BandedEntry::new(r.item.id()).bands(r.decks.0, r.decks.1).weight(r.weight)).collect());
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
        Self { defs, table, triggers }
    }
}

/// The three tables an armory is read against, for the systems that spawn
/// items: what items exist, what kinds of effect a trigger may land, and
/// what moments one may answer.
///
/// One parameter rather than three, because every system that spawns an
/// item wants all three and none of them wants any of the three alone.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Content<'w> {
    registries: Res<'w, Registries>,
    kinds: Res<'w, EffectKinds>,
    moments: Res<'w, Moments>,
}

impl Content<'_> {
    /// The armory, read fresh from `items.ron`.
    pub fn armory(&self) -> Armory {
        Armory::load(&self.registries, &self.kinds, &self.moments)
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

/// Spawns `id` as an item entity carrying every component its definition
/// implies: what it is called and drawn as, what it counts as, and, for
/// something worn, its shape, its armor, its resistances and whatever it
/// strikes or shoots with. `Heat` and `Ammo` are attached the same way
/// Tasks 6 and 7 will give them behaviour for. A stackable item spawns as
/// a stack of one; the caller merges or grows it as it likes.
pub fn spawn_item(commands: &mut Commands, armory: &Armory, id: Id<ItemDef>, registries: &Registries) -> Entity {
    let d = armory.defs.get(id);
    let mut e = commands.spawn((Item, Name::new(d.name.clone()), Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(2)));
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
        e.insert(Throwable { range: throw.range, strike: throw.strike.map(|(dice, kind)| (kind.id(), dice)) });
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
    if d.stack {
        e.insert(Stack { key: id.index() as u64, count: 1 });
    }
    e.id()
}

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
        assert_eq!(armory.defs.len(), 22, "fourteen things to carry, a slug, a keycard, a stim, a medkit and four grenades");
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
        assert_eq!((ranged.range, ranged.cost), (9, Some(100)));
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
    /// fifteen every time.
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
    fn a_weapon_naming_both_heat_and_ammo_fails_to_validate() {
        let r = crate::content::registries();
        let armory = crate::testing::armory(&r);
        let mut d = blank_def("double economy");
        d.heat = Some((10, 10));
        d.ammo = Some(rl_engine::rl_core::Id::from_raw(0).into());
        assert!(validate_def(&d, &armory.defs).is_err(), "heat and ammo on the same weapon");
    }
}
