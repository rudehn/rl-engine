//! What is worn, slot by slot, empty ones included.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;

use crate::facet::Facet;
use crate::panel::{Segment, clear, clip, clip_rich, frame, print_rich, section};
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
                let label_x = x + 2;
                let lines = labelled(&item.label, slot.charges, &item.facets, room.saturating_sub(2), &palette);
                for (i, line) in lines.iter().enumerate() {
                    let row_y = y + i as i32;
                    if row_y >= inner.bottom() {
                        break;
                    }
                    print_rich(&mut terminal, label_x, row_y, line, palette.get(Tones::TEXT), bg, &palette);
                }
                y += lines.len() as i32;
                continue;
            }
            None => terminal.print_on(x, y, &clip(&layout.empty, room), palette.get(Tones::MUTED), bg),
        }
        y += 1;
    }
}

/// An item's name, followed on the same line by its charges, muted, and
/// then its facets, each in its own tone, when they all fit in `width`
/// cells. When they do not, the name is kept whole and the charges and
/// facets move to a second line, indented under where the name started:
/// the count and the warning are the words that matter in a fight, so they
/// are never the ones dropped for room. Only a name that alone overruns
/// `width` still gives way, clipped from the end as it always was.
fn labelled(label: &str, charges: Option<(u16, u16)>, facets: &[Facet], width: usize, palette: &Palette) -> Vec<Vec<Segment>> {
    let words: Vec<Segment> = charges
        .map(|(left, max)| (format!("{left}/{max}"), Some(palette.get(Tones::MUTED))))
        .into_iter()
        .chain(facets.iter().map(|f| (f.text.clone(), Some(palette.get(f.tone)))))
        .collect();
    if words.is_empty() {
        return vec![clip_rich(&[(label.to_string(), None)], width)];
    }
    let tail: Vec<Segment> = words.iter().cloned().flat_map(|w| [(" \u{00b7} ".to_string(), None), w]).collect();
    let tail_width: usize = tail.iter().map(|(r, _)| r.chars().count()).sum();
    let name_width = label.chars().count();
    if name_width + tail_width <= width {
        let runs: Vec<Segment> = std::iter::once((label.to_string(), None)).chain(tail).collect();
        return vec![clip_rich(&runs, width)];
    }
    if name_width > width {
        // The name alone cannot fit; there is no room for a second line to
        // mean anything either, so it gives way as before and the tail is
        // dropped rather than orphaned under a truncated name.
        return vec![vec![(clip(label, width), None)]];
    }
    // The name fits on its own; the tail, which is what a fight is decided
    // by, gets a line to itself instead of taking the name's room.
    let mut continuation: Vec<Segment> = Vec::new();
    for (i, word) in words.into_iter().enumerate() {
        if i > 0 {
            continuation.push((" \u{00b7} ".to_string(), None));
        }
        continuation.push(word);
    }
    vec![vec![(label.to_string(), None)], clip_rich(&continuation, width)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::prelude::*;
    use rl_render::Glyph;
    use rl_rules::{EquipShape, SlotDef};

    #[test]
    fn every_slot_gets_a_line_and_an_empty_one_reads_as_a_dash() {
        let slots = || rl_rules::Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("body")]).unwrap();
        let hand = slots().expect("main hand");
        let mut stage = Stage::new_with(GearPanel::new(Rect::new(0, 0, 26, 6)), move |app| {
            app.world_mut().resource_mut::<rl_bevy::Registries>().slots = slots();
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

    /// A wand's charges read on its row after its name, muted, the way a
    /// count is written everywhere else on the rail.
    #[test]
    fn a_worn_wands_row_reads_its_charges() {
        let mut stage = Stage::new_with(GearPanel::new(Rect::new(0, 0, 40, 6)), |app| {
            app.world_mut().resource_mut::<Registries>().slots = rl_rules::Registry::from_defs(vec![SlotDef::new("main hand")]).unwrap();
        });
        let player = stage.player;
        let hand = stage.app.world().resource::<Registries>().slots.expect("main hand");
        let wand = stage
            .app
            .world_mut()
            .spawn((Item, Name::new("wand"), Glyph::new('/', Color::WHITE), Consumable { left: 3, ..Consumable::new(5, WhenEmpty::Kept) }))
            .id();
        let mut worn = Equipped(rl_rules::Equipment::with_slot_count(1));
        worn.equip(wand, &rl_rules::EquipShape::in_slot(hand)).expect("the slot exists");
        stage.app.world_mut().entity_mut(player).insert(worn);
        stage.tick();
        stage.tick();
        let row = (0..6).map(|y| stage.row(y)).find(|r| r.contains("main hand")).expect("a row for the hand");
        assert!(row.trim_end().ends_with("wand \u{00b7} 3/5"), "{row:?}");
    }

    /// A facet is the game's word on the item and is kept whole: when the
    /// name and the facet together overrun the row, the name is kept
    /// whole too, and the facet moves to a line of its own under it,
    /// indented to the column the name started in, still in its own tone.
    /// So the enchant level at the end of a long name, the very thing the
    /// facet would otherwise have cut off, stays on screen.
    #[test]
    fn a_long_name_and_its_facet_wrap_to_a_line_under_it_rather_than_clip() {
        let slots = || rl_rules::Registry::from_defs(vec![SlotDef::new("main hand")]).unwrap();
        let hand = slots().expect("main hand");
        let mut stage = Stage::new_with(GearPanel::new(Rect::new(0, 0, 26, 4)), move |app| {
            app.world_mut().resource_mut::<rl_bevy::Registries>().slots = slots();
            app.add_systems(
                Update,
                (|mut view: ResMut<GearView>, mut facets: ResMut<crate::Facets>| {
                    for row in view.rows_mut() {
                        row.facets.push(facets.facet("state", "locked").toned(Tones::BAD));
                    }
                })
                .in_set(crate::ViewSet::Annotate),
            );
        })
        .screen(26, 4);
        let item = stage.app.world_mut().spawn((rl_bevy::Item, Name::new("very long name"), rl_render::Glyph::new('}', Color::WHITE))).id();
        let mut worn = rl_bevy::Equipped(rl_rules::Equipment::with_slot_count(1));
        worn.equip(item, &EquipShape::in_slot(hand)).expect("the slot exists");
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(worn);
        stage.tick();

        assert_eq!(stage.row(2), "main hand } very long name", "the whole name, none of it clipped");
        assert_eq!(stage.row(3), format!("{}locked", " ".repeat(12)), "the facet under the name's own column");
        let bad = crate::tone::readable(stage.app.world().resource::<Palette>().get(Tones::BAD), stage.app.world().resource::<Palette>());
        let cell = stage.app.world().resource::<Terminal>().get(12, 3).unwrap();
        assert_eq!(cell.fg, bad, "the facet is drawn in its own tone on the continuation line too");
    }

    /// A name wider than the whole row, with nothing worn to share it
    /// with, still gives way exactly as it always has: there is no room
    /// for a second line to mean anything either.
    #[test]
    fn a_name_wider_than_the_row_itself_still_clips_with_an_ellipsis() {
        let slots = || rl_rules::Registry::from_defs(vec![SlotDef::new("main hand")]).unwrap();
        let hand = slots().expect("main hand");
        let mut stage = Stage::new_with(GearPanel::new(Rect::new(0, 0, 26, 4)), move |app| {
            app.world_mut().resource_mut::<rl_bevy::Registries>().slots = slots();
            app.add_systems(
                Update,
                (|mut view: ResMut<GearView>, mut facets: ResMut<crate::Facets>| {
                    for row in view.rows_mut() {
                        row.facets.push(facets.facet("state", "locked").toned(Tones::BAD));
                    }
                })
                .in_set(crate::ViewSet::Annotate),
            );
        })
        .screen(26, 4);
        let item = stage
            .app
            .world_mut()
            .spawn((rl_bevy::Item, Name::new("an implausibly long name for a hand cannon"), rl_render::Glyph::new('}', Color::WHITE)))
            .id();
        let mut worn = rl_bevy::Equipped(rl_rules::Equipment::with_slot_count(1));
        worn.equip(item, &EquipShape::in_slot(hand)).expect("the slot exists");
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(worn);
        stage.tick();

        assert_eq!(stage.row(2), "main hand } an implausibl\u{2026}", "the name alone still gives way");
        assert_eq!(stage.row(3), "", "no orphaned facet under a name that could not fit either");
    }
}
