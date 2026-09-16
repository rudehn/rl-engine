//! What the player carries, item by item, with what each one is worth.
//!
//! The list every game with items writes: the bag in pickup order, which of
//! it is worn and where, how many of a stack there are, and the numbers on
//! each thing. The numbers are the item's own components, the ones
//! [`Loadout`] reads when a blow is struck: a blade's [`MeleeAttack`], a
//! coat's [`Armor`], what an affix [`Bestows`], how far a knife flies. So an
//! item spawned with them is described for free, in the registries' names,
//! and a game says nothing twice.
//!
//! What an item does when used is the game's, so it is not here: a game
//! pushes a [`Facet`] in [`ViewSet::Annotate`](crate::ViewSet) for "restores
//! 8 health when drunk", and the panel prints it under the row.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_render::Glyph;
use rl_rules::SlotId;
use rl_rules::stats::Op;

use crate::facet::Facet;
use crate::view::sheet::Strike;

/// One carried item, as a bag screen reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemRow {
    /// Which.
    pub entity: Entity,
    /// What the game called it, from its [`Name`]; empty for an item with
    /// none, which is listed rather than hidden because it is carried
    /// whether or not it was named.
    pub label: String,
    /// How it is drawn on the map, if it is.
    pub glyph: Option<Glyph>,
    /// How many, for a stack; one otherwise.
    pub count: u32,
    /// The slot it is worn in, if it is worn.
    pub slot: Option<SlotId>,
    /// That slot's registered name, empty when not worn.
    pub slot_name: String,
    /// Where it would go if put on: the registered names of the slots it
    /// may take, empty for something that cannot be worn.
    pub goes_on: Vec<String>,
    /// How far it flies if thrown, when it can be.
    pub throw_range: Option<i32>,
    /// What it strikes for when thrown, when a throw is a blow.
    pub thrown: Option<Strike>,
    /// Flat armor while worn.
    pub armor: i32,
    /// The blow it is swung with, if wielding it strikes.
    pub blow: Option<Strike>,
    /// The shot it fires, if wielding it shoots.
    pub shot: Option<Strike>,
    /// The extra rolls every hit carries while it is worn.
    pub strikes: Vec<Strike>,
    /// What it does to registered stats while worn, by the stat's name.
    pub bestows: Vec<(String, Op)>,
    /// What it counts as, by the tags' registered names.
    pub tags: Vec<String>,
    /// What the game added.
    pub facets: Vec<Facet>,
}

impl ItemRow {
    /// Whether it is on.
    pub fn worn(&self) -> bool {
        self.slot.is_some()
    }

    /// Whether it can be put on.
    pub fn wearable(&self) -> bool {
        !self.goes_on.is_empty()
    }
}

/// The player's bag.
#[derive(Resource, Debug, Default)]
pub struct InventoryView {
    /// Whose bag, while there is a player with one.
    pub entity: Option<Entity>,
    /// The items, in pickup order, worn or not.
    pub rows: Vec<ItemRow>,
}

impl InventoryView {
    /// The row at `index`, for a menu cursor.
    pub fn row(&self, index: usize) -> Option<&ItemRow> {
        self.rows.get(index)
    }

    /// The row for `item`, for a game annotating one it recognises.
    pub fn row_of(&mut self, item: Entity) -> Option<&mut ItemRow> {
        self.rows.iter_mut().find(|r| r.entity == item)
    }
}

/// Keeps [`InventoryView`] current.
///
/// Needs nothing. A player with no [`Inventory`] lists nothing, and the
/// names on each row come from [`Registries`] when the game inserted one
/// and are empty otherwise.
pub struct InventoryViewPlugin;

impl Plugin for InventoryViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryView>().add_systems(Update, collect_inventory.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "InventoryViewPlugin");
    }
}

/// How an item looks and where it goes.
type Looks =
    (Option<&'static Name>, Option<&'static Glyph>, Option<&'static Stack>, Option<&'static Wearable>, Option<&'static Throwable>, Option<&'static Tagged>);
/// What an item does when worn: the same components [`Loadout`] reads.
type Arms = (Option<&'static Armor>, Option<&'static MeleeAttack>, Option<&'static RangedAttack>, Option<&'static Strikes>, Option<&'static Bestows>);

/// Fills [`InventoryView`] from the player's bag.
pub fn collect_inventory(
    mut view: ResMut<InventoryView>,
    registries: Option<Res<Registries>>,
    player: Query<(Entity, &Inventory, Option<&Equipped>), With<Player>>,
    items: Query<(Looks, Arms), With<Item>>,
) {
    view.rows.clear();
    view.entity = None;
    let Ok((entity, bag, worn)) = player.single() else { return };
    view.entity = Some(entity);
    let registries = registries.as_deref();
    let kind_name = |kind| registries.map(|r| r.damage_kinds.name(kind).to_string()).unwrap_or_default();
    let slot_name = |slot| registries.map(|r| r.slots.name(slot).to_string()).unwrap_or_default();
    let strike = |(kind, dice): (rl_rules::damage::DamageKindId, rl_core::DiceRoll), range: Option<i32>| Strike { kind: kind_name(kind), dice, range };
    for &item in &bag.items {
        let Ok(((name, glyph, stack, wearable, throwable, tagged), (armor, melee, ranged, strikes, bestows))) = items.get(item) else { continue };
        let slot = worn.and_then(|w| w.slot_of(item));
        view.rows.push(ItemRow {
            entity: item,
            label: name.map(|n| n.as_str().to_string()).unwrap_or_default(),
            glyph: glyph.copied(),
            count: stack.map_or(1, |s| s.count),
            slot,
            slot_name: slot.map(slot_name).unwrap_or_default(),
            goes_on: wearable.map(|w| w.0.any_of.iter().map(|s| slot_name(*s)).collect()).unwrap_or_default(),
            throw_range: throwable.map(|t| t.range),
            thrown: throwable.and_then(|t| t.strike.map(|s| strike(s, Some(t.range)))),
            armor: armor.map_or(0, |a| a.0),
            blow: melee.map(|m| strike((m.kind, m.dice), None)),
            shot: ranged.map(|r| strike((r.kind, r.dice), Some(r.range))),
            strikes: strikes.map(|s| s.0.iter().map(|hit| strike(*hit, None)).collect()).unwrap_or_default(),
            bestows: bestows
                .map(|b| b.0.iter().map(|(stat, op)| (registries.map(|r| r.stats.name(*stat).to_string()).unwrap_or_default(), *op)).collect())
                .unwrap_or_default(),
            tags: tagged.map(|t| t.0.iter().map(|tag| registries.map(|r| r.tags.name(*tag).to_string()).unwrap_or_default()).collect()).unwrap_or_default(),
            facets: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_core::DiceRoll;
    use rl_rules::content::Registry;
    use rl_rules::{EquipShape, Equipment, SlotDef, StatDef};

    /// A row carries the item's own numbers in the registries' names, and
    /// says where it is worn and where it could be.
    #[test]
    fn the_bag_is_listed_in_pickup_order_with_what_each_thing_is_worth() {
        let mut stage = Stage::new_with(InventoryViewPlugin, |app| {
            let mut registries = app.world_mut().resource_mut::<Registries>();
            registries.stats = Registry::from_defs(vec![StatDef::new("might", 10)]).unwrap();
            registries.slots = Registry::from_defs(vec![SlotDef::new("hand"), SlotDef::new("head")]).unwrap();
        });
        let (player, kind) = (stage.player, stage.kind);
        let (hand, head, might) = {
            let r = stage.app.world().resource::<Registries>();
            (r.slots.expect("hand"), r.slots.expect("head"), r.stats.expect("might"))
        };
        let blade = stage
            .app
            .world_mut()
            .spawn((
                Item,
                Name::new("a blade"),
                Wearable(EquipShape::in_slot(hand)),
                MeleeAttack { kind, dice: DiceRoll::new(1, 6) },
                Strikes(vec![(kind, DiceRoll::flat(1))]),
                Bestows(vec![(might, Op::Add(2))]),
            ))
            .id();
        let hat = stage.app.world_mut().spawn((Item, Name::new("a hat"), Wearable(EquipShape::in_slot(head)), Armor(1))).id();
        let knives = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("knives"), Stack { key: 1, count: 3 }, Throwable { range: 5, strike: Some((kind, DiceRoll::new(1, 4))) }))
            .id();
        let mut worn = Equipped(Equipment::with_slot_count(2));
        worn.equip(blade, &EquipShape::in_slot(hand)).unwrap();
        stage.app.world_mut().entity_mut(player).insert((Inventory { items: vec![knives, blade, hat] }, worn));
        stage.tick();

        let view = stage.app.world().resource::<InventoryView>();
        assert_eq!(view.entity, Some(player));
        let labels: Vec<&str> = view.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, vec!["knives", "a blade", "a hat"], "pickup order, worn or not");

        let knives = &view.rows[0];
        assert_eq!((knives.count, knives.throw_range), (3, Some(5)));
        assert_eq!(knives.thrown.as_ref().map(|s| (s.kind.as_str(), s.range)), Some(("kinetic", Some(5))));
        assert!(!knives.wearable() && !knives.worn());

        let blade = &view.rows[1];
        assert_eq!((blade.slot, blade.slot_name.as_str()), (Some(hand), "hand"), "worn, and where");
        assert_eq!(blade.blow.as_ref().map(|s| s.dice), Some(DiceRoll::new(1, 6)));
        assert_eq!(blade.strikes.len(), 1);
        assert_eq!(blade.bestows, vec![("might".to_string(), Op::Add(2))], "by the stat's name");

        let hat = &view.rows[2];
        assert!(!hat.worn() && hat.wearable());
        assert_eq!((hat.goes_on.as_slice(), hat.armor), (["head".to_string()].as_slice(), 1));
    }
}
