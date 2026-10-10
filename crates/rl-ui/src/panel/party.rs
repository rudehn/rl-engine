//! The party rail: one line a follower, with a health bar.
//!
//! A heading, then each follower in the order it joined: its glyph, its
//! name and whatever the game pushed onto its row, and a bar at the right
//! in the tones the vitals draw the player's own health in, so a follower
//! at a quarter reads as bad by the same rule the player does. One on
//! another map is muted and says so after its name, and keeps its bar.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;

use crate::panel::nearby::clip_keeping;
use crate::panel::{bar, clear, section};
use crate::tone::{Palette, Tones};
use crate::view::{PartyView, PartyViewPlugin, Row, health_tone};

/// Where the party rail is drawn and what it says.
#[derive(Resource, Debug, Clone)]
pub struct PartyLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
    /// The heading over the rows, drawn only while someone follows. Empty
    /// draws none.
    pub heading: String,
    /// What a follower on another map is called after its name, in the
    /// game's own words. Empty says nothing, and the muted name alone
    /// marks it.
    pub elsewhere: String,
    /// Cells given to each health bar.
    pub bar_width: i32,
}

/// Draws [`PartyView`] as a rail.
///
/// Adds [`PartyViewPlugin`] if the game has not. With nobody following it
/// leaves its rectangle blank, heading and all, so a game can give the
/// party a place on the rail from the first turn without an empty heading
/// sitting there until someone joins.
pub struct PartyPanel(PartyLayout);

impl PartyPanel {
    /// A rail in `rect`, headed `Party`, with a follower on another map
    /// marked `elsewhere` and six cells for each bar: the engine's words
    /// for what the rows are, plain enough to ship with and to replace.
    pub fn new(rect: Rect) -> Self {
        Self(PartyLayout { rect, heading: "Party".into(), elsewhere: "elsewhere".into(), bar_width: 6 })
    }

    /// Replaces the heading. Empty draws none.
    pub fn heading(mut self, heading: impl Into<String>) -> Self {
        self.0.heading = heading.into();
        self
    }

    /// Replaces what a follower on another map is called.
    pub fn elsewhere(mut self, word: impl Into<String>) -> Self {
        self.0.elsewhere = word.into();
        self
    }

    /// Sets how many cells a bar gets.
    pub fn bars(mut self, width: i32) -> Self {
        self.0.bar_width = width;
        self
    }
}

impl Plugin for PartyPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<PartyViewPlugin>() {
            app.add_plugins(PartyViewPlugin);
        }
        app.insert_resource(self.0.clone()).add_systems(Update, draw_party.in_set(PresentSet::Chrome));
    }
}

/// Paints the party rail.
pub fn draw_party(mut terminal: ResMut<Terminal>, layout: Res<PartyLayout>, view: Res<PartyView>, palette: Res<Palette>) {
    let rect = layout.rect;
    if rect.width < 6 || rect.height < 1 {
        return;
    }
    // Cleared either way: the rail owns its rectangle, and the last
    // follower to leave must not leave its line behind.
    clear(&mut terminal, rect, &palette);
    if view.is_empty() {
        return;
    }
    let mut y = rect.y;
    if !layout.heading.is_empty() && rect.height > 2 {
        section(&mut terminal, rect, y, &layout.heading, None, &palette);
        y += 2;
    }
    for row in &view.rows {
        if y >= rect.bottom() {
            break;
        }
        draw_row(&mut terminal, rect, y, row, view.is_elsewhere(row), &layout, &palette);
        y += 1;
    }
}

fn draw_row(terminal: &mut Terminal, rect: Rect, y: i32, row: &Row, elsewhere: bool, layout: &PartyLayout, palette: &Palette) {
    let bg = palette.get(Tones::SURFACE);
    terminal.print_on(rect.x, y, &row.glyph.ch.to_string(), row.glyph.fg, bg);
    // The bar at the right edge, a space clear of the name, when the rail
    // has room for both it and a few letters of the name.
    let shows_bar = row.health.is_some() && layout.bar_width > 0 && rect.width >= layout.bar_width + 6;
    let text_end = if shows_bar { rect.right() - layout.bar_width - 1 } else { rect.right() };
    let mut name = row.label.clone();
    for facet in &row.facets {
        name.push_str(" \u{00b7} ");
        name.push_str(&facet.text);
    }
    let tail = if elsewhere && !layout.elsewhere.is_empty() { format!(" ({})", layout.elsewhere) } else { String::new() };
    let text = clip_keeping(&name, &tail, (text_end - rect.x - 2).max(0) as usize);
    let tone = if elsewhere { Tones::MUTED } else { Tones::TEXT };
    terminal.print_on(rect.x + 2, y, &text, palette.get(tone), bg);
    if let (true, Some((hp, max))) = (shows_bar, row.health) {
        bar(terminal, rect.right() - layout.bar_width, y, layout.bar_width, row.health_fraction(), health_tone(hp, max), palette);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use rl_bevy::prelude::*;

    fn party(stage: &mut Stage, name: &str, glyph: char, dx: i32) -> Entity {
        let me = stage.player;
        let one = stage.actor(name, glyph, dx, 0);
        stage.app.world_mut().entity_mut(one).insert(Leader(me));
        one
    }

    fn fg(stage: &Stage, x: i32, y: i32) -> Color {
        stage.app.world().resource::<Terminal>().get(x, y).expect("on the screen").fg
    }

    #[test]
    fn with_nobody_following_the_rail_draws_nothing_not_even_its_heading() {
        let mut stage = Stage::new(PartyPanel::new(Rect::new(0, 0, 24, 6))).screen(24, 6);
        stage.actor("stranger", 's', 2, 0);
        stage.tick();
        assert!(stage.rows().iter().all(|r| r.is_empty()), "{:?}", stage.rows());
    }

    #[test]
    fn a_follower_is_a_line_under_the_heading_with_a_health_bar_in_the_vitals_tones() {
        let mut stage = Stage::new(PartyPanel::new(Rect::new(0, 0, 24, 6))).screen(24, 6);
        let hale = party(&mut stage, "hale", 'h', 2);
        let hurt = party(&mut stage, "hurt", 'u', 3);
        let worn = party(&mut stage, "worn", 'w', 4);
        for (who, hp) in [(hale, 10), (hurt, 2), (worn, 5)] {
            stage.app.world_mut().get_mut::<Health>(who).unwrap().current = hp;
        }
        stage.tick();
        let rows = stage.rows();
        assert_eq!(rows[0], "Party");
        assert!(rows[1].starts_with('\u{2500}'), "underlined: {:?}", rows[1]);
        // The bar takes the last six cells, from column 18.
        assert_eq!(rows[2], format!("{:<18}{}", "h hale", "\u{2588}".repeat(6)));
        assert_eq!(rows[3], format!("{:<18}{}{}", "u hurt", "\u{2588}".repeat(2), "\u{2591}".repeat(4)), "a fifth, rounded up");
        assert_eq!(rows[4], format!("{:<18}{}{}", "w worn", "\u{2588}".repeat(3), "\u{2591}".repeat(3)));

        let palette = stage.app.world().resource::<Palette>().clone();
        for (y, tone) in [(2, Tones::GOOD), (3, Tones::BAD), (4, Tones::NOTICE)] {
            assert_eq!(fg(&stage, 18, y), palette.get(tone), "row {y} reads as the vitals would read the same health");
        }
    }

    #[test]
    fn a_follower_on_another_map_is_muted_and_marked_in_the_games_word_and_keeps_its_bar() {
        let panel = PartyPanel::new(Rect::new(0, 0, 30, 4)).heading("Company").elsewhere("below");
        let mut stage = Stage::new(panel).screen(30, 4);
        let left = party(&mut stage, "left", 'l', 2);
        stage.app.world_mut().entity_mut(left).insert(OnMap(MapId(3)));
        stage.tick();
        let rows = stage.rows();
        assert_eq!(rows[0], "Company");
        assert!(rows[2].starts_with("l left (below)"), "{:?}", rows[2]);
        assert!(rows[2].ends_with(&"\u{2588}".repeat(6)), "{:?}", rows[2]);
        let palette = stage.app.world().resource::<Palette>().clone();
        assert_eq!(fg(&stage, 2, 2), palette.get(Tones::MUTED));
    }

    #[test]
    fn a_game_facet_is_written_after_the_followers_name() {
        fn order(mut view: ResMut<PartyView>, mut facets: ResMut<crate::Facets>) {
            for row in view.rows_mut() {
                row.facets.push(facets.facet("order", "hold"));
            }
        }
        let mut stage = Stage::new(PartyPanel::new(Rect::new(0, 0, 24, 4))).screen(24, 4);
        stage.app.add_systems(Update, order.in_set(crate::ViewSet::Annotate));
        party(&mut stage, "kit", 'k', 2);
        stage.tick();
        assert!(stage.rows()[2].starts_with("k kit \u{00b7} hold "), "{:?}", stage.rows()[2]);
    }

    /// The rail clears what it drew once the last follower is gone, rather
    /// than leaving a line for someone no longer there.
    #[test]
    fn the_last_follower_leaving_blanks_the_rail() {
        let mut stage = Stage::new(PartyPanel::new(Rect::new(0, 0, 24, 4))).screen(24, 4);
        let kit = party(&mut stage, "kit", 'k', 2);
        stage.tick();
        assert_eq!(stage.rows()[0], "Party");
        stage.app.world_mut().entity_mut(kit).remove::<Leader>();
        stage.tick();
        assert!(stage.rows().iter().all(|r| r.is_empty()), "{:?}", stage.rows());
    }
}
