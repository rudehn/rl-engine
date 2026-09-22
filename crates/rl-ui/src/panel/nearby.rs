//! The nearby rail: who is in sight, and what is lying about.
//!
//! The row picked out is drawn on the selection tone, and so is the map
//! tile it names while no cursor is up, so a player stepping down the list
//! with Tab sees which glyph each row is. A cursor draws its own mark on
//! the map, and the rail leaves the map to it.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::{Cell, MapView, Terminal};
use rl_rules::Relation;

use crate::cursor::CursorStyle;
use crate::modal::Modals;
use crate::panel::{clear, clip, frame, section};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::{Alert, NearbyView, NearbyViewPlugin, Row};

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
    /// What each state is called, in the game's own words. An empty word
    /// says nothing at all for that state, which is what a game that
    /// wants only "hunting" written does with the other two.
    pub alerts: AlertWords,
    /// How the cell of the row picked out is marked on the map. A glow by
    /// default, since the rail's own highlight is a glow and the two read
    /// as one thing.
    pub cursor: CursorStyle,
}

/// What each [`Alert`] is called on a row, in the game's own words.
///
/// The engine knows the three states and will not name them: one game's
/// monsters sleep where another's stand idle, and a droid does neither.
/// The defaults are plain enough to ship with and plain enough to
/// replace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertWords {
    /// What something that knows of nothing is called.
    pub unaware: String,
    /// What something on its way to a noise is called.
    pub searching: String,
    /// What something that has noticed you is called.
    pub hunting: String,
}

impl Default for AlertWords {
    fn default() -> Self {
        Self { unaware: "unaware".into(), searching: "searching".into(), hunting: "hunting".into() }
    }
}

impl AlertWords {
    /// The three, in order.
    pub fn new(unaware: impl Into<String>, searching: impl Into<String>, hunting: impl Into<String>) -> Self {
        Self { unaware: unaware.into(), searching: searching.into(), hunting: hunting.into() }
    }

    /// What `alert` is called, or `None` where the game left it unsaid.
    pub fn get(&self, alert: Alert) -> Option<&str> {
        let word = match alert {
            Alert::Unaware => &self.unaware,
            Alert::Searching => &self.searching,
            Alert::Hunting => &self.hunting,
        };
        (!word.is_empty()).then_some(word.as_str())
    }
}

/// Draws [`NearbyView`] as a rail.
///
/// Adds [`NearbyViewPlugin`] if the game has not, because a panel with no
/// view behind it can only draw nothing.
pub struct NearbyPanel(NearbyLayout);

impl NearbyPanel {
    /// A rail in `rect`, with the engine's default headings.
    pub fn new(rect: Rect) -> Self {
        Self(NearbyLayout {
            rect,
            title: "Nearby".into(),
            actors: "In sight".into(),
            things: "On the ground".into(),
            alerts: AlertWords::default(),
            cursor: CursorStyle::glow(Tones::SELECT),
        })
    }

    /// Replaces the title in the top border. Empty draws no frame, which
    /// is what a rail flush against the map wants.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Sets how the cell of the row picked out is marked on the map.
    pub fn cursor(mut self, style: CursorStyle) -> Self {
        self.0.cursor = style;
        self
    }

    /// Replaces the two headings.
    pub fn headings(mut self, actors: impl Into<String>, things: impl Into<String>) -> Self {
        self.0.actors = actors.into();
        self.0.things = things.into();
        self
    }

    /// Sets what each state is called on a row.
    pub fn alerts(mut self, words: AlertWords) -> Self {
        self.0.alerts = words;
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

/// Paints the rail, and the tile of the row picked out.
pub fn draw_nearby(
    mut terminal: ResMut<Terminal>,
    layout: Res<NearbyLayout>,
    view: Res<NearbyView>,
    palette: Res<Palette>,
    modals: Res<Modals>,
    map: Option<Res<MapView>>,
    time: Res<Time>,
) {
    if let (Some(focused), Some(map)) = (view.focused, map.as_deref())
        && !modals.any_open()
    {
        // The same mark the look cursor and the targeting cursor use, so
        // the row picked out and the cell it stands on read as one thing.
        crate::cursor::mark(&mut terminal, map, focused.at, layout.cursor, &palette, time.elapsed_secs());
    }
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
        // Keep the row picked out on the rail: when it falls below the rows
        // that fit, the list starts late enough to show it last.
        let room = (bottom - y).max(0) as usize;
        let skip = rows.iter().position(|r| view.is_focused(r)).map_or(0, |i| (i + 1).saturating_sub(room));
        for row in rows.iter().skip(skip) {
            if y >= bottom {
                break;
            }
            draw_row(&mut terminal, inner, y, row, view.is_focused(row), &layout.alerts, &palette);
            y += 1;
        }
        y += 1;
    }
}

fn draw_row(terminal: &mut Terminal, inner: Rect, y: i32, row: &Row, focused: bool, words: &AlertWords, palette: &Palette) {
    // The row is the bar: health fills it from the left in the health's
    // own tone, mixed into the row's background rather than painted over
    // it, so a wounded thing reads at a glance without a column of its
    // own and without shouting.
    let base = palette.get(if focused { Tones::SELECT } else { Tones::SURFACE });
    terminal.fill(Rect::new(inner.x, y, inner.width, 1), Cell::new(' ', base).on(base));
    let filled = match row.health {
        Some(_) => ((inner.width as f32 * row.health_fraction()).round() as i32).clamp(0, inner.width),
        None => 0,
    };
    let hurt = palette.get(match row.health_fraction() {
        f if f <= 0.25 => Tones::BAD,
        f if f <= 0.5 => Tones::NOTICE,
        _ => Tones::GOOD,
    });
    let wash = base.mix(&hurt, 0.35);
    if filled > 0 {
        terminal.fill(Rect::new(inner.x, y, filled, 1), Cell::new(' ', base).on(wash));
    }
    let bg_at = |x: i32| if x < inner.x + filled { wash } else { base };

    terminal.print_on(inner.x, y, &row.glyph.ch.to_string(), row.glyph.fg, bg_at(inner.x));
    let mut name = row.label.clone();
    for facet in &row.facets {
        name.push_str(" \u{00b7} ");
        name.push_str(&facet.text);
    }
    // What it is doing about you, in the panel's own words and after the
    // name: a state a player reads rather than a mark they learn.
    if let Some(word) = row.alert.and_then(|alert| words.get(alert)) {
        name.push_str(" (");
        name.push_str(word);
        name.push(')');
    }
    // Something that knows of nothing reads muted, so the names in their
    // own colour are the ones that know you are there.
    let tone = if row.alert == Some(Alert::Unaware) { Tones::MUTED } else { relation_tone(row.relation) };
    let text = clip(&name, (inner.width - 2).max(0) as usize);
    for (i, ch) in text.chars().enumerate() {
        let x = inner.x + 2 + i as i32;
        terminal.print_on(x, y, &ch.to_string(), palette.get(tone), bg_at(x));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::CursorKeys;
    use crate::harness::Stage;

    #[test]
    fn the_rail_prints_a_heading_a_glyph_a_name_and_the_row_itself_is_the_health_bar() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("")).screen(24, 10);
        stage.actor("crab", 'c', 2, 0);
        stage.thing("rum", '!', 1, 0);
        stage.tick();

        let rows = stage.rows();
        assert_eq!(rows[0], "In sight               1", "the heading carries the threat count");
        assert!(rows[1].starts_with('\u{2500}'), "underlined: {:?}", rows[1]);
        assert!(rows[2].starts_with("c crab"), "glyph then name: {:?}", rows[2]);
        assert_eq!(rows[4], "On the ground", "the second heading has no count");
        assert!(rows[6].starts_with("! rum"), "{:?}", rows[6]);

        // The row is the bar: a crab at full health is washed edge to
        // edge, and a bottle on the floor, which has no health, is not
        // washed at all.
        let (surface, good) = {
            let palette = stage.app.world().resource::<Palette>();
            (palette.get(Tones::SURFACE), palette.get(Tones::GOOD))
        };
        let full = surface.mix(&good, 0.35);
        for x in [0, 12, 23] {
            assert_eq!(bg(&stage, x, 2), full, "the crab's row is washed at column {x}");
        }
        assert_eq!(bg(&stage, 12, 6), surface, "a thing on the floor has no health to show");
    }

    #[test]
    fn one_coming_to_look_at_a_sound_carries_a_question_and_one_that_has_seen_you_the_bang_instead() {
        let rules = rl_bevy::NoiseRules { step: 0, strike: 0, door: 0, landing: 0, door_muffle: 0 };
        let panel = NearbyPanel::new(Rect::new(0, 0, 24, 10)).titled("");
        let mut stage = Stage::new((panel, rl_bevy::NoisePlugin::new(rules))).screen(24, 10);
        let rat = stage.actor("rat", 'r', 2, 0);
        stage.app.world_mut().entity_mut(rat).insert(rl_bevy::Hearing::default());
        let at = stage.at;
        stage.app.world_mut().get_mut::<rl_bevy::Heard>(rat).unwrap().0 = rl_rules::Awareness::Alert { at, stale_turns: 0 };
        stage.tick();
        assert!(stage.rows()[2].contains("rat (searching)"), "{:?}", stage.rows()[2]);

        // And one that has seen you is hunting, whatever it heard.
        let mut row = stage.app.world().resource::<NearbyView>().actors[0].clone();
        row.alert = Some(Alert::Hunting);
        let palette = stage.app.world().resource::<Palette>().clone();
        let mut terminal = Terminal::new(24, 1, bevy::math::Vec2::ONE);
        draw_row(&mut terminal, Rect::new(0, 0, 24, 1), 0, &row, false, &AlertWords::default(), &palette);
        let said: String = (0..24).filter_map(|x| terminal.get(x, 0).map(|c| c.glyph)).collect();
        assert!(said.contains("(hunting)"), "{said:?}");
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

    /// The background of the terminal cell at `x, y`.
    fn bg(stage: &Stage, x: i32, y: i32) -> Color {
        stage.app.world().resource::<Terminal>().get(x, y).expect("on the screen").bg
    }

    #[test]
    fn the_row_picked_out_is_drawn_on_the_selection_bar_and_all_and_so_is_its_tile() {
        let mut stage = Stage::new_with(NearbyPanel::new(Rect::new(40, 0, 24, 10)).titled(""), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)));
        })
        .screen(64, 20);
        stage.actor("crab", 'c', 2, 0);
        stage.actor("gull", 'g', 3, 0);
        stage.tick();
        let (select, surface) = {
            let palette = stage.app.world().resource::<Palette>();
            (palette.get(Tones::SELECT), palette.get(Tones::SURFACE))
        };
        let tile = stage.app.world().resource::<MapView>().to_screen(stage.at.offset(3, 0)).expect("the gull is on the map");
        // Unpicked and unhurt, so the row carries its own health wash
        // rather than the selection.
        let unpicked = surface.mix(&stage.app.world().resource::<Palette>().get(Tones::GOOD), 0.35);
        assert_eq!(bg(&stage, 45, 3), unpicked, "nothing is picked out yet");

        stage.press(CursorKeys::default().next);
        stage.press(CursorKeys::default().next);
        assert!(stage.rows()[3].contains("g gull"), "{:?}", stage.rows()[3]);
        // Picked out and unhurt: the selection is what the health washes
        // over, so the row reads as picked whatever its health.
        let picked = select.mix(&stage.app.world().resource::<Palette>().get(Tones::GOOD), 0.35);
        for x in [40, 45, 63] {
            assert_eq!(bg(&stage, x, 3), picked, "the gull's row, edge to edge, at column {x}");
        }
        assert_eq!(bg(&stage, 45, 2), unpicked, "and not the crab's");
        assert_eq!(bg(&stage, tile.x, tile.y), select, "the gull's tile on the map");
        assert_eq!(stage.app.world().resource::<Terminal>().get(tile.x, tile.y).map(|c| c.glyph), Some('g'), "still showing the gull");

        stage.press(CursorKeys::default().close);
        assert_eq!(bg(&stage, 45, 3), unpicked, "let go, and back to its own health wash");
        assert_ne!(bg(&stage, tile.x, tile.y), select);
    }

    #[test]
    fn a_row_picked_out_below_the_rail_scrolls_the_list_to_show_it() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(0, 0, 24, 5)).titled("")).screen(24, 5);
        for (i, name) in ["ant", "bee", "cat", "dog", "eel"].into_iter().enumerate() {
            stage.actor(name, name.chars().next().unwrap(), i as i32 + 1, 0);
        }
        stage.tick();
        assert!(stage.rows()[4].starts_with("c cat"), "three rows fit: {:?}", stage.rows());
        for _ in 0..5 {
            stage.press(CursorKeys::default().next);
        }
        let rows = stage.rows();
        assert!(rows[4].starts_with("e eel"), "the eel is shown last: {rows:?}");
        assert!(rows[2].starts_with("c cat"), "and the list starts late enough: {rows:?}");
    }
}
