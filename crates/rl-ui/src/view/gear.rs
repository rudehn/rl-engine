//! What the player is wearing, slot by slot, filled or not.
//!
//! An empty slot is as much of the panel as a full one: what you are not
//! wearing is the thing you go looking for. So the view lists every slot
//! the game registered, in registration order, and leaves the item `None`
//! where nothing is worn.
//!
//! The slot names come from the slots in [`Registries`], which the game
//! inserts. The engine
//! stores what is worn by slot id, and an id is an index; printing it needs
//! the registry the index came from.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_render::Glyph;
use rl_rules::SlotId;

use crate::view::Row;

/// One equipment slot and what fills it.
#[derive(Debug, Clone, PartialEq)]
pub struct GearSlot {
    /// Which slot.
    pub slot: SlotId,
    /// Its registered name.
    pub name: String,
    /// What is worn in it, if anything.
    pub item: Option<Row>,
}

/// Every slot, in the order the game registered them.
#[derive(Resource, Debug, Default)]
pub struct GearView {
    /// The slots.
    pub slots: Vec<GearSlot>,
}

impl GearView {
    /// The slot called `name`, if the game registered one.
    pub fn by_name(&self, name: &str) -> Option<&GearSlot> {
        self.slots.iter().find(|s| s.name == name)
    }

    /// Every filled slot.
    pub fn worn(&self) -> impl Iterator<Item = (&GearSlot, &Row)> {
        self.slots.iter().filter_map(|s| s.item.as_ref().map(|i| (s, i)))
    }

    /// Every row, for an annotate system that wants to note what a piece
    /// of gear does.
    pub fn rows_mut(&mut self) -> impl Iterator<Item = &mut Row> {
        self.slots.iter_mut().filter_map(|s| s.item.as_mut())
    }
}

/// Keeps [`GearView`] current.
///
/// Needs [`Registries`], whose slots the worn ids index into.
pub struct GearViewPlugin;

impl Plugin for GearViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GearView>()
            .needs::<Registries>("GearViewPlugin", "`Registries`, with the equipment slots the worn ids index into")
            .add_systems(Update, collect_gear.in_set(crate::ViewSet::Collect));
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<crate::UiPlugin>(app, "GearViewPlugin");
    }
}

/// Fills [`GearView`] from what the player wears.
pub fn collect_gear(
    mut view: ResMut<GearView>,
    registries: Res<Registries>,
    player: Query<&Equipped, With<Player>>,
    items: Query<(Option<&Name>, Option<&Glyph>, Option<&Stack>)>,
) {
    view.slots.clear();
    let worn = player.single().ok();
    for (slot, def) in registries.slots.iter() {
        let item = worn.and_then(|w| w.in_slot(slot)).map(|entity| {
            let (name, glyph, stack) = items.get(entity).unwrap_or((None, None, None));
            let shown = name.map(|n| rl_core::noun::listed(n.as_str(), stack.map_or(1, |s| s.count))).unwrap_or_default();
            Row::new(entity, shown, glyph.copied().unwrap_or(Glyph::new('?', Color::WHITE)))
        });
        view.slots.push(GearSlot { slot, name: def.name.clone(), item });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_rules::{EquipShape, SlotDef};

    fn slots() -> rl_rules::Registry<SlotDef> {
        rl_rules::Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("body")]).unwrap()
    }

    #[test]
    fn every_slot_is_a_row_and_an_empty_one_is_as_visible_as_a_full_one() {
        let hand = slots().expect("main hand");
        let mut stage = Stage::new_with(GearViewPlugin, |app| {
            app.world_mut().resource_mut::<Registries>().slots = slots();
        });
        let player = stage.player;

        let blade = stage.app.world_mut().spawn((Item, Name::new("a rusty blade"), Glyph::new('/', Color::WHITE))).id();
        let mut worn = Equipped(rl_rules::Equipment::with_slot_count(2));
        worn.equip(blade, &EquipShape::in_slot(hand)).expect("the slot exists");
        stage.app.world_mut().entity_mut(player).insert(worn);
        stage.tick();

        let view = stage.app.world().resource::<GearView>();
        assert_eq!(view.slots.len(), 2, "both slots are rows");
        let hand_row = view.by_name("main hand").expect("the slot is listed by name");
        assert_eq!(hand_row.item.as_ref().map(|i| i.label.as_str()), Some("a rusty blade"));
        assert_eq!(hand_row.item.as_ref().map(|i| i.glyph.ch), Some('/'));
        let body = view.by_name("body").expect("listed");
        assert!(body.item.is_none(), "an empty slot is still a row");
        assert_eq!(view.worn().count(), 1);
    }

    #[test]
    fn a_stack_of_more_than_one_says_how_many() {
        let hand = slots().expect("main hand");
        let mut stage = Stage::new_with(GearViewPlugin, |app| {
            app.world_mut().resource_mut::<Registries>().slots = slots();
        });
        let player = stage.player;

        let darts = stage.app.world_mut().spawn((Item, Name::new("dart"), Glyph::new('|', Color::WHITE), Stack { key: 1, count: 7 })).id();
        let mut worn = Equipped(rl_rules::Equipment::with_slot_count(2));
        worn.equip(darts, &EquipShape::in_slot(hand)).expect("the slot exists");
        stage.app.world_mut().entity_mut(player).insert(worn);
        stage.tick();

        let view = stage.app.world().resource::<GearView>();
        assert_eq!(view.by_name("main hand").unwrap().item.as_ref().unwrap().label, "7 darts", "the count first, and the name for many");
    }
}
