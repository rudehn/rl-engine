//! The party rail: one line a follower, with a health bar.
//!
//! A heading, then each follower in the order it joined: its glyph, its
//! name and whatever the game pushed onto its row, over the row as its
//! own health bar, as the nearby rail draws a row, in the tones the vitals
//! draw the player's own health in, so a follower at a quarter reads as
//! bad by the same rule the player does and by the same wash a foe beside
//! it does. One on another map is muted and says so after its name, and
//! keeps its bar.
//!
//! The bar was once a few cells at the row's right edge. Beside a name
//! and its order on a rail a third of the screen wide it was too short to
//! tell a quarter from a third, and it took the cells the order needed,
//! so it spans the row now and the words lie over it.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;

use crate::facet::Facets;
use crate::panel::nearby::{Beneath, clip_keeping, paint_beneath, paint_row};
use crate::panel::{clear, section};
use crate::tone::{Palette, Tones};
use crate::view::{PartyView, PartyViewPlugin, Row};

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
    /// The facets drawn on lines of their own beneath a follower's row,
    /// by key, rather than after its name, as on the nearby rail. Empty
    /// draws every facet after the name.
    pub beneath: Vec<String>,
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
    /// marked `elsewhere`: the engine's words for what the rows are,
    /// plain enough to ship with and to replace.
    pub fn new(rect: Rect) -> Self {
        Self(PartyLayout { rect, heading: "Party".into(), elsewhere: "elsewhere".into(), beneath: Vec::new() })
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

    /// Draws the facets under `key` beneath their follower's row rather
    /// than after its name: wrapped whole while the rail has room for every
    /// follower's so drawn, and each follower's on one clipped line when it
    /// has not, so every follower keeps its row.
    pub fn beneath(mut self, key: impl Into<String>) -> Self {
        self.0.beneath.push(key.into());
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
pub fn draw_party(mut terminal: ResMut<Terminal>, layout: Res<PartyLayout>, view: Res<PartyView>, palette: Res<Palette>, facets: Res<Facets>) {
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
    let beneath = Beneath::new(&layout.beneath, &facets, rect.width);
    let whole: i32 = view.rows.iter().map(|r| 1 + beneath.lines(r, false).len() as i32).sum();
    let compact = whole > rect.bottom() - y;
    for row in &view.rows {
        if y >= rect.bottom() {
            break;
        }
        draw_row(&mut terminal, Rect::new(rect.x, y, rect.width, 1), row, view.is_elsewhere(row), &layout, &beneath, &palette);
        let base = palette.get(Tones::SURFACE);
        y = paint_beneath(&mut terminal, rect, y + 1, rect.bottom(), &beneath.lines(row, compact), base, &palette);
    }
}

fn draw_row(terminal: &mut Terminal, line: Rect, row: &Row, elsewhere: bool, layout: &PartyLayout, beneath: &Beneath, palette: &Palette) {
    // What the game says of a follower, its order, is the row's point, so
    // the facets and the elsewhere mark are kept whole and the name gives
    // way: a long name read as `Dagdis Longsh… · fighting freely` still
    // says who, where `Dagdis Longshanks · fighti…` said neither what it
    // was doing nor that it was cut there.
    let mut tail = String::new();
    for facet in row.facets.iter().filter(|f| !beneath.takes(f)) {
        tail.push_str(" \u{00b7} ");
        tail.push_str(&facet.text);
    }
    if elsewhere && !layout.elsewhere.is_empty() {
        tail.push_str(&format!(" ({})", layout.elsewhere));
    }
    let text = clip_keeping(&row.label, &tail, (line.width - 2).max(0) as usize);
    let tone = if elsewhere { Tones::MUTED } else { Tones::TEXT };
    paint_row(terminal, line, row, palette.get(Tones::SURFACE), &text, tone, palette);
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

    fn bg(stage: &Stage, x: i32, y: i32) -> Color {
        stage.app.world().resource::<Terminal>().get(x, y).expect("on the screen").bg
    }

    /// The background a row's health washes `tone` over.
    fn washed(stage: &Stage, tone: crate::tone::ToneId) -> Color {
        let palette = stage.app.world().resource::<Palette>();
        palette.get(Tones::SURFACE).mix(&palette.get(tone), crate::panel::nearby::WASH)
    }

    #[test]
    fn with_nobody_following_the_rail_draws_nothing_not_even_its_heading() {
        let mut stage = Stage::new(PartyPanel::new(Rect::new(0, 0, 24, 6))).screen(24, 6);
        stage.actor("stranger", 's', 2, 0);
        stage.tick();
        assert!(stage.rows().iter().all(|r| r.is_empty()), "{:?}", stage.rows());
    }

    /// The row is the bar, as on the nearby rail: health washes it from
    /// the left, edge to edge at full, in the tone the vitals would read
    /// the same health in, and the name lies over it.
    #[test]
    fn a_follower_is_a_line_under_the_heading_and_its_row_is_its_health_bar_in_the_vitals_tones() {
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
        assert_eq!(&rows[2..5], ["h hale", "u hurt", "w worn"], "a glyph and a name, and no bar beside them");

        let surface = stage.app.world().resource::<Palette>().get(Tones::SURFACE);
        // Twenty-four cells: all of them at full, five at a fifth, twelve
        // at a half, each in its own tone and the rest the rail's own.
        for (y, tone, filled) in [(2, Tones::GOOD, 24), (3, Tones::BAD, 5), (4, Tones::NOTICE, 12)] {
            let wash = washed(&stage, tone);
            assert_eq!(bg(&stage, 0, y), wash, "row {y} reads as the vitals would read the same health");
            assert_eq!(bg(&stage, filled - 1, y), wash, "row {y} is washed to column {}", filled - 1);
            if filled < 24 {
                assert_eq!(bg(&stage, filled, y), surface, "and no further on row {y}");
            }
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
        assert_eq!(rows[2], "l left (below)");
        assert_eq!(bg(&stage, 29, 2), washed(&stage, Tones::GOOD), "and keeps its bar, edge to edge");
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
        assert_eq!(stage.rows()[2], "k kit \u{00b7} hold");
    }

    /// A name of twenty-four letters beside the longest order gives way to
    /// the order, which is kept whole; the name keeps its start.
    #[test]
    fn a_long_name_is_clipped_and_the_games_facet_after_it_kept_whole() {
        fn order(mut view: ResMut<PartyView>, mut facets: ResMut<crate::Facets>) {
            for row in view.rows_mut() {
                row.facets.push(facets.facet("order", "fighting freely"));
            }
        }
        let mut stage = Stage::new(PartyPanel::new(Rect::new(0, 0, 36, 4))).screen(36, 4);
        stage.app.add_systems(Update, order.in_set(crate::ViewSet::Annotate));
        party(&mut stage, "Dagdisorvan Longshankson", 'h', 2);
        stage.tick();
        let row = &stage.rows()[2];
        assert!(row.starts_with("h Dagdis"), "the name's start: {row:?}");
        assert!(row.ends_with("\u{2026} \u{00b7} fighting freely"), "the name cut, the order whole: {row:?}");
        assert_eq!(row.chars().count(), 36, "and the whole row is the name's, now the bar lies under it: {row:?}");
    }

    /// Pushes what each follower holds under `kit`, beneath its row, and
    /// its order after its name.
    fn kit_and_order(mut view: ResMut<PartyView>, mut facets: ResMut<crate::Facets>) {
        for row in view.rows_mut() {
            row.facets.push(facets.facet("order", "hold"));
            row.facets.push(facets.facet("kit", "wields a long spear with a hooked head").toned(Tones::MUTED));
        }
    }

    /// A facet the layout puts beneath the row is wrapped whole under the
    /// name in its own tone, and the one it does not stays after the name.
    #[test]
    fn a_facet_put_beneath_is_wrapped_under_the_name_and_the_rest_stay_after_it() {
        let mut stage = Stage::new(PartyPanel::new(Rect::new(0, 0, 24, 8)).beneath("kit")).screen(24, 8);
        stage.app.add_systems(Update, kit_and_order.in_set(crate::ViewSet::Annotate));
        party(&mut stage, "kit", 'k', 2);
        stage.tick();
        let rows = stage.rows();
        assert_eq!(&rows[2..6], ["k kit \u{00b7} hold", "  wields a long spear", "  with a hooked head", ""], "{rows:?}");
        let palette = stage.app.world().resource::<Palette>().clone();
        assert_eq!(fg(&stage, 2, 3), palette.get(Tones::MUTED), "in the facet's own tone");
        assert_eq!(bg(&stage, 23, 3), palette.get(Tones::SURFACE), "and the health wash is the row's alone");
    }

    /// With no room to wrap every follower's notes, each keeps one clipped
    /// line, so the second follower is not pushed off the rail by the
    /// first one's.
    #[test]
    fn with_no_room_to_wrap_every_followers_notes_each_keeps_one_clipped_line() {
        let mut stage = Stage::new(PartyPanel::new(Rect::new(0, 0, 24, 6)).beneath("kit")).screen(24, 6);
        stage.app.add_systems(Update, kit_and_order.in_set(crate::ViewSet::Annotate));
        party(&mut stage, "kit", 'k', 2);
        party(&mut stage, "tam", 't', 3);
        stage.tick();
        let rows = stage.rows();
        assert_eq!(
            &rows[2..6],
            ["k kit \u{00b7} hold", "  wields a long spear w\u{2026}", "t tam \u{00b7} hold", "  wields a long spear w\u{2026}"],
            "{rows:?}"
        );
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
