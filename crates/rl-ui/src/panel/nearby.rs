//! The nearby rail: who is in sight, and what is lying about.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;
use rl_rules::Relation;

use crate::panel::{bar, clear, clip, frame, section};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::{NearbyView, NearbyViewPlugin, Row};

/// Where the nearby rail is drawn and what the headings say.
#[derive(Resource, Debug, Clone)]
pub struct NearbyLayout {
    /// The terminal cells it occupies, border included.
    pub rect: Rect,
    /// The title in the top border. Empty draws no frame.
    pub title: String,
    /// The heading over the actors.
    pub actors: String,
    /// The heading over the things on the ground.
    pub things: String,
    /// Cells given to each health bar.
    pub bar_width: i32,
}

/// Draws [`NearbyView`] as a rail.
///
/// Adds [`NearbyViewPlugin`] if the game has not, because a panel with no
/// view behind it can only draw nothing.
pub struct NearbyPanel(NearbyLayout);

impl NearbyPanel {
    /// A rail in `rect`, with the engine's default headings.
    pub fn new(rect: Rect) -> Self {
        Self(NearbyLayout { rect, title: "Nearby".into(), actors: "In sight".into(), things: "On the ground".into(), bar_width: 6 })
    }

    /// Replaces the title in the top border. Empty draws no frame, which
    /// is what a rail flush against the map wants.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Replaces the two headings.
    pub fn headings(mut self, actors: impl Into<String>, things: impl Into<String>) -> Self {
        self.0.actors = actors.into();
        self.0.things = things.into();
        self
    }

    /// Sets how many cells a health bar gets.
    pub fn bars(mut self, width: i32) -> Self {
        self.0.bar_width = width;
        self
    }
}

impl Plugin for NearbyPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<NearbyViewPlugin>() {
            app.add_plugins(NearbyViewPlugin);
        }
        app.insert_resource(self.0.clone()).add_systems(Update, draw_nearby.in_set(PresentSet::Chrome));
    }
}

/// The tone a row reads in: what it is to the player, or plain text when
/// it takes no side.
pub fn relation_tone(relation: Option<Relation>) -> ToneId {
    match relation {
        Some(Relation::Hostile) => Tones::BAD,
        Some(Relation::Allied) => Tones::GOOD,
        _ => Tones::TEXT,
    }
}

/// Paints the rail.
pub fn draw_nearby(mut terminal: ResMut<Terminal>, layout: Res<NearbyLayout>, view: Res<NearbyView>, palette: Res<Palette>) {
    let rect = layout.rect;
    if rect.width < 6 || rect.height < 4 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    let inner = if layout.title.is_empty() {
        rect
    } else {
        frame(&mut terminal, rect, &layout.title, "", &palette);
        Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2)
    };
    let mut y = inner.y;
    let bottom = inner.bottom();
    for (heading, rows, count) in [(&layout.actors, &view.actors, Some(view.threats())), (&layout.things, &view.things, None)] {
        if rows.is_empty() || y + 3 > bottom {
            continue;
        }
        section(&mut terminal, inner, y, heading, count, &palette);
        y += 2;
        for row in rows {
            if y >= bottom {
                break;
            }
            draw_row(&mut terminal, inner, y, row, layout.bar_width, &palette);
            y += 1;
        }
        y += 1;
    }
}

fn draw_row(terminal: &mut Terminal, inner: Rect, y: i32, row: &Row, bar_width: i32, palette: &Palette) {
    let bg = palette.get(Tones::SURFACE);
    terminal.print_on(inner.x, y, &row.glyph.ch.to_string(), row.glyph.fg, bg);
    let has_bar = row.health.is_some() && inner.width > bar_width + 4;
    let name_width = if has_bar { inner.width - bar_width - 3 } else { inner.width - 2 };
    let mut name = row.label.clone();
    for facet in &row.facets {
        name.push_str(" \u{00b7} ");
        name.push_str(&facet.text);
    }
    // Something that has not noticed you reads muted, so the names in its
    // colour are the ones hunting you; one that has, carries a mark.
    let tone = if row.aware == Some(false) { Tones::MUTED } else { relation_tone(row.relation) };
    if row.aware == Some(true) {
        terminal.print_on(inner.x + 1, y, "!", palette.get(Tones::BAD), bg);
    }
    terminal.print_on(inner.x + 2, y, &clip(&name, name_width.max(0) as usize), palette.get(tone), bg);
    if has_bar {
        let tone = match row.health_fraction() {
            f if f <= 0.25 => Tones::BAD,
            f if f <= 0.5 => Tones::NOTICE,
            _ => Tones::GOOD,
        };
        bar(terminal, inner.right() - bar_width, y, bar_width, row.health_fraction(), tone, palette);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;

    #[test]
    fn the_rail_prints_a_heading_a_glyph_a_name_and_a_bar() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        stage.actor("crab", 'c', 2, 0);
        stage.thing("rum", '!', 1, 0);
        stage.tick();

        let rows = stage.rows();
        assert_eq!(rows[0], "In sight               1", "the heading carries the threat count");
        assert!(rows[1].starts_with('\u{2500}'), "underlined: {:?}", rows[1]);
        assert!(rows[2].starts_with("c crab"), "glyph then name: {:?}", rows[2]);
        assert!(rows[2].contains('\u{2588}'), "a full health bar: {:?}", rows[2]);
        assert_eq!(rows[4], "On the ground", "the second heading has no count");
        assert!(rows[6].starts_with("! rum"), "{:?}", rows[6]);
        assert!(!rows[6].contains('\u{2588}'), "a thing on the floor has no health bar");
    }

    #[test]
    fn a_facet_is_printed_after_the_name_and_a_long_row_is_clipped_not_wrapped() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        stage.actor("crab", 'c', 2, 0);
        stage.tick();
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<NearbyView>, mut facets: ResMut<crate::Facets>| {
                for row in view.rows_mut() {
                    row.facets.push(facets.facet("wielding", "a very long cutlass indeed"));
                }
            })
            .in_set(crate::ViewSet::Annotate),
        );
        stage.tick();

        let row = stage.rows()[2].clone();
        assert!(row.contains("crab \u{00b7} a very"), "the facet follows the name: {row:?}");
        assert!(row.chars().count() <= 24, "clipped to the panel, never wrapped: {row:?}");
        assert!(row.contains('\u{2026}'), "and says it was clipped: {row:?}");
    }

    #[test]
    fn an_empty_rail_draws_no_headings_at_all() {
        let stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        assert!(stage.rows().iter().all(|r| r.is_empty()), "nothing in sight is nothing drawn: {:?}", stage.rows());
    }
}
