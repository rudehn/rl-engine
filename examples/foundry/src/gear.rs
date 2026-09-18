//! Gear: the six weapons, six pieces of armor and the slugs that arm the
//! foundry's decks, loaded once from `items.ron` and spawned as items an
//! actor can find, carry and wear.
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
use serde::Deserialize;

use crate::ammo::Ammo;
use crate::heat::Heat;

const ITEMS_RON: &str = include_str!("../assets/items.ron");

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
    /// Flat armor while worn.
    #[serde(default)]
    pub armor: i32,
    /// Percent removed per damage kind, while worn.
    #[serde(default)]
    pub resists: Vec<(NameRef<DamageKind>, i32)>,
    /// The roll and damage kind a blow deals, while wielded.
    #[serde(default)]
    pub melee: Option<(DiceRoll, NameRef<DamageKind>)>,
    /// The range, roll and damage kind a shot deals, while wielded.
    #[serde(default)]
    pub ranged: Option<(i32, DiceRoll, NameRef<DamageKind>)>,
    /// The range, roll and damage kind it strikes with when thrown; absent,
    /// it cannot be thrown at all.
    #[serde(default)]
    pub throw: Option<(i32, DiceRoll, NameRef<DamageKind>)>,
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
    /// `(min deck, max deck, weight)` for lying on a deck.
    #[serde(default)]
    pub spawn: Option<(i32, i32, u32)>,
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

/// The item definitions and the table of what lies on which deck.
#[derive(Debug, Clone)]
pub struct Armory {
    /// The item definitions, by id.
    pub defs: Registry<ItemDef>,
    /// What can be found on a deck, drawn by band, where the band is the
    /// deck number.
    pub table: BandedTable<Id<ItemDef>>,
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
    Ok(())
}

impl Armory {
    /// Loads `items.ron` against `registries`, validates it, and builds
    /// the spawn table; panics with every problem the file has, since a
    /// broken item file is a game that cannot start.
    pub fn load(registries: &Registries) -> Self {
        let defs: Registry<ItemDef> = registries.names().load(ITEMS_RON).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
        defs.validate(validate_def).unwrap_or_else(|e| panic!("assets/items.ron: {e}"));
        let mut table = BandedTable::default();
        for (id, d) in defs.iter() {
            if let Some((lo, hi, w)) = d.spawn {
                table.push(BandedEntry::new(id).bands(lo, hi).weight(w));
            }
        }
        Self { defs, table }
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
    if let Some((dice, kind)) = d.melee {
        e.insert(MeleeAttack { kind: kind.id(), dice, cost: d.cost });
    }
    if let Some((range, dice, kind)) = d.ranged {
        e.insert(RangedAttack { kind: kind.id(), dice, range, cost: d.cost });
    }
    if let Some((range, dice, kind)) = d.throw {
        e.insert(Throwable { range, strike: Some((kind.id(), dice)) });
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
    use rl_engine::rl_core::RunSeed;

    use super::*;

    /// Spawns two items by name, through a fresh `Armory` and a raw
    /// `CommandQueue`, the way a test reaches into the world without a
    /// system of its own.
    fn spawn_two(app: &mut App, a: &str, b: &str) -> (Entity, Entity) {
        let registries = app.world().resource::<Registries>().clone();
        let armory = Armory::load(&registries);
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
        let armory = Armory::load(&registries);
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
        let armory = Armory::load(&r);
        assert_eq!(armory.defs.len(), 13);
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
            spawn: None,
        }
    }

    #[test]
    fn a_weapon_naming_both_heat_and_ammo_fails_to_validate() {
        let r = crate::content::registries();
        let armory = Armory::load(&r);
        let mut d = blank_def("double economy");
        d.heat = Some((10, 10));
        d.ammo = Some(rl_engine::rl_core::Id::from_raw(0).into());
        assert!(validate_def(&d, &armory.defs).is_err(), "heat and ammo on the same weapon");
    }
}
