//! What is worn, slot by slot, empty ones included.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;

use crate::facet::Facet;
use crate::panel::{Segment, clear, clip, clip_rich, frame, print_rich, section, wrap};
use crate::tone::{Palette, Tones};
use crate::view::{GearView, GearViewPlugin};

/// Where the gear list is drawn.
#[derive(Resource, Debug, Clone)]
pub(crate) struct GearLayout {
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

/// What the rail says of a worn thing that will not come off, in the bad
/// tone: the bag's words for it, so one mark means one thing everywhere.
pub(crate) const STUCK_WORDS: &str = "will not come off";

/// Paints the gear list.
///
/// A slot whose name and words need more than one row takes the rows the
/// panel has to spare, slot by slot from the top, and never so many that a
/// slot below it is pushed out of the panel: what is not worn is as much
/// the list as what is.
pub(crate) fn draw_gear(mut terminal: ResMut<Terminal>, layout: Res<GearLayout>, view: Res<GearView>, palette: Res<Palette>) {
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
    for (i, slot) in view.slots.iter().enumerate() {
        if y >= inner.bottom() {
            break;
        }
        // The rows this slot may take: its own, and whatever the slots
        // after it leave spare.
        let spare = (inner.bottom() - y) as usize - (view.slots.len() - i).min((inner.bottom() - y) as usize);
        let room_for = 1 + spare;
        terminal.print_on(inner.x, y, &clip(&slot.name, slot_width), palette.get(Tones::MUTED), bg);
        let x = inner.x + slot_width as i32 + 1;
        let room = (inner.right() - x).max(0) as usize;
        match &slot.item {
            Some(item) => {
                terminal.print_on(x, y, &item.glyph.ch.to_string(), item.glyph.fg, bg);
                let label_x = x + 2;
                // A claimed slot (a two-hander's off hand) is the same
                // item worn twice over: whatever a game noted about it
                // already reads on the slot it was equipped into, so a
                // second copy here would only say it again.
                let (charges, facets, stuck): (Option<(u16, u16)>, &[Facet], bool) =
                    if slot.primary { (slot.charges, &item.facets, slot.stuck) } else { (None, &[], false) };
                let words: Vec<Segment> = charges
                    .map(|(left, max)| (format!("{left}/{max}"), Some(palette.get(Tones::MUTED))))
                    .into_iter()
                    .chain(facets.iter().map(|f| (f.text.clone(), Some(palette.get(f.tone)))))
                    .chain(stuck.then(|| (STUCK_WORDS.to_string(), Some(palette.get(Tones::BAD)))))
                    .collect();
                let lines = labelled(&item.label, words, room.saturating_sub(2), room_for);
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

/// An item's name, followed on the same line by `words`, its charges,
/// what the game noted and whether it will come off, each in its own
/// colour, when they all fit in `width` cells, and in no more than `lines`
/// rows.
///
/// A name too long for the row wraps onto the rows under it, on its
/// spaces, and the words follow it on its last row or on one of their
/// own, all indented under where the name started: a name is what the
/// player reads the rail for, and `unidentified ring of` with the rest cut
/// off reads as a different thing. The words are the ones that matter in
/// a fight, so they are never the ones dropped for room. Only when there
/// are not the rows to spare does a name give way, kept to one row with
/// the words on the next, and a name that alone overruns `width` clipped
/// from the end and the words dropped, rather than orphaned under a
/// truncated name.
fn labelled(label: &str, words: Vec<Segment>, width: usize, lines: usize) -> Vec<Vec<Segment>> {
    let joined = |words: &[Segment]| -> Vec<Segment> {
        let mut runs: Vec<Segment> = Vec::new();
        for (i, word) in words.iter().cloned().enumerate() {
            if i > 0 {
                runs.push((" \u{00b7} ".to_string(), None));
            }
            runs.push(word);
        }
        runs
    };
    let tail: Vec<Segment> = words.iter().cloned().flat_map(|w| [(" \u{00b7} ".to_string(), None), w]).collect();
    let tail_width: usize = tail.iter().map(|(r, _)| r.chars().count()).sum();
    // Wrapped: the name on as many rows as it takes, and the words after
    // its last row when they fit there, on a row of their own when not.
    let mut wrapped: Vec<Vec<Segment>> = wrap(label, width).into_iter().map(|line| vec![(line, None)]).collect();
    if wrapped.is_empty() {
        wrapped.push(vec![(String::new(), None)]);
    }
    if !words.is_empty() {
        let last = wrapped.last_mut().expect("a row at least");
        if last[0].0.chars().count() + tail_width <= width {
            last.extend(tail.iter().cloned());
        } else {
            wrapped.push(clip_rich(&joined(&words), width));
        }
    }
    if wrapped.len() <= lines.max(1) {
        return wrapped;
    }
    // No rows to spare for that: the name kept to one row as it was
    // before names wrapped.
    let name_width = label.chars().count();
    let kept = if name_width > width {
        vec![vec![(clip(label, width), None)]]
    } else if words.is_empty() || name_width + tail_width <= width {
        vec![clip_rich(&std::iter::once((label.to_string(), None)).chain(tail).collect::<Vec<_>>(), width)]
    } else {
        vec![vec![(label.to_string(), None)], clip_rich(&joined(&words), width)]
    };
    kept.into_iter().take(lines.max(1)).collect()
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
    /// So the level at the end of a long name, the very thing the
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

    /// A worn thing that will not come off says so on the rail in the
    /// bag's words and the bag's bad tone, after its name, so a ring that
    /// clings reads as one here as it does in the pack; the same thing once
    /// freed says nothing more.
    #[test]
    fn a_stuck_worn_thing_says_it_will_not_come_off_in_the_bad_tone() {
        let slots = || rl_rules::Registry::from_defs(vec![SlotDef::new("finger")]).unwrap();
        let finger = slots().expect("finger");
        let mut stage = Stage::new_with(GearPanel::new(Rect::new(0, 0, 40, 4)), move |app| {
            app.world_mut().resource_mut::<rl_bevy::Registries>().slots = slots();
        })
        .screen(40, 4);
        let ring = stage.app.world_mut().spawn((rl_bevy::Item, Name::new("grey ring"), rl_render::Glyph::new('=', Color::WHITE), rl_bevy::Stuck)).id();
        let mut worn = rl_bevy::Equipped(rl_rules::Equipment::with_slot_count(1));
        worn.equip(ring, &EquipShape::in_slot(finger)).expect("the slot exists");
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(worn);
        stage.tick();

        assert_eq!(stage.row(2), "finger = grey ring \u{00b7} will not come off");
        let palette = stage.app.world().resource::<Palette>();
        let bad = crate::tone::readable(palette.get(Tones::BAD), palette);
        let at = stage.row(2).chars().position(|c| c == 'w').expect("the words") as i32;
        assert_eq!(stage.app.world().resource::<Terminal>().get(at, 2).unwrap().fg, bad, "in the bad tone");

        stage.app.world_mut().entity_mut(ring).remove::<rl_bevy::Stuck>();
        stage.tick();
        assert_eq!(stage.row(2), "finger = grey ring", "freed, it says nothing more");
    }

    /// A name too long for its row wraps on its spaces onto the rows the
    /// panel has spare, under where it started, with its words after it;
    /// and never onto a row a slot below it needs, so with none to spare
    /// it is kept to one row as before and every slot is still drawn.
    #[test]
    fn a_long_name_wraps_onto_spare_rows_and_never_pushes_a_slot_out() {
        let slots = || rl_rules::Registry::from_defs(vec![SlotDef::new("finger"), SlotDef::new("light")]).unwrap();
        let finger = slots().expect("finger");
        let staged = |height: i32| {
            let mut stage = Stage::new_with(GearPanel::new(Rect::new(0, 0, 26, height)), move |app| {
                app.world_mut().resource_mut::<rl_bevy::Registries>().slots = slots();
            })
            .screen(26, height);
            let ring = stage
                .app
                .world_mut()
                .spawn((rl_bevy::Item, Name::new("unidentified ring of light"), rl_render::Glyph::new('=', Color::WHITE), rl_bevy::Stuck))
                .id();
            let mut worn = rl_bevy::Equipped(rl_rules::Equipment::with_slot_count(2));
            worn.equip(ring, &EquipShape::in_slot(finger)).expect("the slot exists");
            let player = stage.player;
            stage.app.world_mut().entity_mut(player).insert(worn);
            stage.tick();
            stage
        };
        let roomy = staged(7);
        let indent = " ".repeat(9);
        assert_eq!(
            (2..7).map(|y| roomy.row(y)).collect::<Vec<_>>(),
            [
                "finger = unidentified ring".to_string(),
                format!("{indent}of light"),
                format!("{indent}will not come off"),
                "light  \u{2014}".to_string(),
                String::new()
            ],
            "the whole name, its words under it, and the slot below"
        );
        let tight = staged(4);
        assert_eq!([tight.row(2), tight.row(3)], ["finger = unidentified rin\u{2026}".to_string(), "light  \u{2014}".to_string()], "no row to spare");
    }

    /// A name wider than the whole row, with nothing worn to share it
    /// with, and no rows to spare for it to wrap onto, still gives way
    /// exactly as it always has: there is no room for a second line to
    /// mean anything either.
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

    /// A two-hander's facet, pushed onto both the row the engine built for
    /// its own slot and the row it built for the slot the item only
    /// claims, reads once: on the hand it was equipped into. The claimed
    /// hand still names the weapon, just not what it is out of, so `dry`
    /// is not said twice for one gun.
    #[test]
    fn a_two_handers_facet_reads_once_on_the_hand_it_was_equipped_into() {
        let slots = || rl_rules::Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("off hand")]).unwrap();
        let (main, off) = {
            let s = slots();
            (s.expect("main hand"), s.expect("off hand"))
        };
        let mut stage = Stage::new_with(GearPanel::new(Rect::new(0, 0, 30, 6)), move |app| {
            app.world_mut().resource_mut::<rl_bevy::Registries>().slots = slots();
            app.add_systems(
                Update,
                (|mut view: ResMut<GearView>, mut facets: ResMut<crate::Facets>| {
                    for row in view.rows_mut() {
                        row.facets.push(facets.facet("ammo", "dry").toned(Tones::BAD));
                    }
                })
                .in_set(crate::ViewSet::Annotate),
            );
        })
        .screen(30, 6);
        let rifle = stage.app.world_mut().spawn((rl_bevy::Item, Name::new("slug rifle"), rl_render::Glyph::new('}', Color::WHITE))).id();
        let mut worn = rl_bevy::Equipped(rl_rules::Equipment::with_slot_count(2));
        worn.equip(rifle, &EquipShape::in_slot(main).and_claims(off)).expect("the slot exists");
        let player = stage.player;
        stage.app.world_mut().entity_mut(player).insert(worn);
        stage.tick();

        assert_eq!(stage.row(2), "main hand } slug rifle \u{00b7} dry", "the primary hand carries the facet");
        assert_eq!(stage.row(3), "off hand  } slug rifle", "the claimed hand names the gun and stops there");
    }
}
