//! What is worn, slot by slot, empty ones included.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;

use crate::panel::{clear, clip, frame, section};
use crate::tone::{Palette, Tones};
use crate::view::{GearView, GearViewPlugin};

/// Where the gear list is drawn.
#[derive(Resource, Debug, Clone)]
pub struct GearLayout {
    /// The terminal cells it occupies, border included.
    pub rect: Rect,
    /// The title in the top border. Empty draws no frame.
    pub title: String,
    /// The heading over the list. Empty draws no heading.
    pub heading: String,
    /// What an empty slot reads as.
    pub empty: String,
}

/// Draws [`GearView`].
///
/// Adds [`GearViewPlugin`] if the game has not.
pub struct GearPanel(GearLayout);

impl GearPanel {
    /// A gear list in `rect`.
    pub fn new(rect: Rect) -> Self {
        Self(GearLayout { rect, title: String::new(), heading: "Worn".into(), empty: "\u{2014}".into() })
    }

    /// Sets the title in the top border, which also turns the frame on.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Sets the heading over the list. Empty draws none.
    pub fn heading(mut self, heading: impl Into<String>) -> Self {
        self.0.heading = heading.into();
        self
    }

    /// Sets what an empty slot reads as.
    pub fn empty(mut self, empty: impl Into<String>) -> Self {
        self.0.empty = empty.into();
        self
    }
}

impl Plugin for GearPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<GearViewPlugin>() {
            app.add_plugins(GearViewPlugin);
        }
        app.insert_resource(self.0.clone()).add_systems(Update, draw_gear.in_set(PresentSet::Chrome));
    }
}

/// Paints the gear list.
pub fn draw_gear(mut terminal: ResMut<Terminal>, layout: Res<GearLayout>, view: Res<GearView>, palette: Res<Palette>) {
    let rect = layout.rect;
    if rect.width < 8 || rect.height < 2 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    let inner = if layout.title.is_empty() {
        rect
    } else {
        frame(&mut terminal, rect, &layout.title, "", &palette);
        Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2)
    };
    let bg = palette.get(Tones::SURFACE);
    let mut y = inner.y;
    if !layout.heading.is_empty() {
        section(&mut terminal, inner, y, &layout.heading, None, &palette);
        y += 2;
    }
    // A slot name column wide enough for the longest name, so the items
    // line up and the eye reads down one edge.
    let slot_width = view.slots.iter().map(|s| s.name.chars().count()).max().unwrap_or(0).min(inner.width as usize / 2);
    for slot in &view.slots {
        if y >= inner.bottom() {
            break;
        }
        terminal.print_on(inner.x, y, &clip(&slot.name, slot_width), palette.get(Tones::MUTED), bg);
        let x = inner.x + slot_width as i32 + 1;
        let room = (inner.right() - x).max(0) as usize;
        match &slot.item {
            Some(item) => {
                terminal.print_on(x, y, &item.glyph.ch.to_string(), item.glyph.fg, bg);
                let mut name = item.label.clone();
                for facet in &item.facets {
                    name.push_str(" \u{00b7} ");
                    name.push_str(&facet.text);
                }
                terminal.print_on(x + 2, y, &clip(&name, room.saturating_sub(2)), palette.get(Tones::TEXT), bg);
            }
            None => terminal.print_on(x, y, &clip(&layout.empty, room), palette.get(Tones::MUTED), bg),
        }
        y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_rules::{EquipShape, SlotDef};

    #[test]
    fn every_slot_gets_a_line_and_an_empty_one_reads_as_a_dash() {
        let slots = || rl_bevy::Slots(rl_rules::Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("body")]).unwrap());
        let hand = slots().expect("main hand");
        let mut stage = Stage::new_with(GearPanel::new(Rect::new(0, 0, 26, 6)), move |app| {
            app.insert_resource(slots());
        })
        .screen(26, 6);

        let blade = stage.app.world_mut().spawn((rl_bevy::Item, Name::new("rusty blade"), rl_render::Glyph::new('/', Color::WHITE))).id();
        let mut worn = rl_bevy::Equipped(rl_rules::Equipment::with_slot_count(2));
        worn.equip(blade, &EquipShape::in_slot(hand)).expect("the slot exists");
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(worn);
        stage.tick();

        let rows = stage.rows();
        assert_eq!(rows[0], "Worn");
        assert!(rows[1].starts_with('\u{2500}'), "underlined: {:?}", rows[1]);
        assert_eq!(rows[2], "main hand / rusty blade", "the slot name, the glyph, the item");
        assert_eq!(rows[3], "body      \u{2014}", "and an empty slot is still a line");
    }
}
