//! What the look cursor is over, and how a fight with it would go.
//!
//! Drawn in [`PresentSet::Overlay`], over the map, because the cursor is a
//! modal and a modal covers what it is about. The cursor is drawn as
//! four ticks on the cells around the one it sits on, a dash either side
//! and a bar above and below, pulsing, so the cell itself shows what is
//! there, and the panel and the cursor never disagree about what is being
//! described. The ticks are ASCII because a browser build draws with
//! Bevy's built-in font alone, which has no arrows worth the name.
//!
//! The panel sits over the map, so it moves out of the way of what it
//! describes: when the cursor or a tick round it would fall under the
//! panel, the panel is drawn in its other place, the game's
//! [`InspectPanel::else_at`] or its rectangle mirrored top for bottom in
//! the map. It stays there until the cursor goes under it again rather
//! than going back as soon as it can, because a panel that swapped sides
//! on every step across the middle of the map would be read in two
//! places. A fresh look starts where the game put it.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::{Point, Rect};
use rl_render::{MapView, Terminal};
use rl_rules::forecast::Outlook;

use crate::cursor::{CursorMap, CursorStyle};
use crate::modal::Modals;
use crate::panel::nearby::relation_tone;
use crate::panel::{bar, clear, clip, frame, hides, other_place, wrap};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::inspect::inspect_modal;
use crate::view::{InspectView, InspectViewPlugin, WorkRow};

/// Where the inspect panel is drawn.
#[derive(Resource, Debug, Clone)]
pub(crate) struct InspectLayout {
    /// The terminal cells it occupies, border included.
    pub rect: Rect,
    /// Where it goes while the cursor is under `rect`. `None` mirrors
    /// `rect` top for bottom within the map, which suits a panel along
    /// either edge; a game names a place when its panel is not there.
    pub elsewhere: Option<Rect>,
    /// The title in the top border.
    pub title: String,
    /// The key hints in the bottom border.
    pub hints: String,
    /// What is shown when the cursor is over nothing.
    pub empty: String,
    /// How the cell the cursor sits on is marked. Four ASCII ticks round
    /// it by default, breathing, so the cell keeps its own glyph.
    pub cursor: CursorStyle,
    /// What someone at work on something is said to be doing: `{doing}`
    /// is the kind's word, `{target}` what it is working on, `{left}` the
    /// turns left counted ("4 turns", "1 turn") and `{n}` the bare number.
    /// The first letter is capitalised.
    pub working: String,
    /// The same, for work done to nothing in particular.
    pub working_alone: String,
    /// The line under the facets that counts those the rectangle has no
    /// room for, `{n}` standing for how many.
    pub more: String,
}

/// Draws [`InspectView`] and the cursor on the map.
///
/// Adds [`InspectViewPlugin`] if the game has not.
pub struct InspectPanel(InspectLayout);

impl InspectPanel {
    /// An inspect panel in `rect`.
    pub fn new(rect: Rect) -> Self {
        Self(InspectLayout {
            rect,
            elsewhere: None,
            title: "Looking at".into(),
            hints: "move \u{2022} tab next \u{2022} esc close".into(),
            empty: "Nothing here.".into(),
            cursor: CursorStyle::ticks(),
            working: "{doing} the {target}, {left} left".into(),
            working_alone: "{doing}, {left} left".into(),
            more: "and {n} more".into(),
        })
    }

    /// Sets where the panel goes while the cursor is under its rectangle,
    /// in place of the mirror of it.
    pub fn else_at(mut self, rect: Rect) -> Self {
        self.0.elsewhere = Some(rect);
        self
    }

    /// Sets how work is described: with a target, and without one.
    pub fn working(mut self, with_target: impl Into<String>, alone: impl Into<String>) -> Self {
        self.0.working = with_target.into();
        self.0.working_alone = alone.into();
        self
    }

    /// Sets the title in the top border.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
        self
    }

    /// Sets how the cell being looked at is marked.
    pub fn cursor(mut self, style: CursorStyle) -> Self {
        self.0.cursor = style;
        self
    }

    /// Sets the key hints in the bottom border.
    pub fn hints(mut self, hints: impl Into<String>) -> Self {
        self.0.hints = hints.into();
        self
    }

    /// Sets the line counting the facets left out for want of room, with
    /// `{n}` for how many.
    pub fn more(mut self, more: impl Into<String>) -> Self {
        self.0.more = more.into();
        self
    }

    /// Sets what is shown when the cursor is over nothing.
    pub fn empty(mut self, empty: impl Into<String>) -> Self {
        self.0.empty = empty.into();
        self
    }
}

impl Plugin for InspectPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<InspectViewPlugin>() {
            app.add_plugins(InspectViewPlugin);
        }
        app.insert_resource(self.0.clone())
            .init_resource::<InspectPlacement>()
            .add_systems(Update, (place_inspect, draw_inspect).chain().in_set(PresentSet::Overlay));
    }
}

/// The tone an outlook reads in.
///
/// A game that wants a fifth colour for its worst tier declares a tone and
/// writes its own panel; this is the honest four-way reading of two blow
/// counts.
pub(crate) fn outlook_tone(outlook: Outlook) -> ToneId {
    match outlook {
        Outlook::Easy => Tones::GOOD,
        Outlook::Favorable => Tones::GOOD,
        Outlook::Even => Tones::NOTICE,
        Outlook::Grim => Tones::BAD,
        Outlook::Deadly => Tones::BAD,
    }
}

/// Where the inspect panel is drawn this frame: `None` while the look
/// cursor is down, and otherwise its rectangle or the place it moved to.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct InspectPlacement(pub Option<Rect>);

/// Picks where the panel goes this frame, moving it off the cursor.
///
/// Without a [`MapView`] there is no cursor on screen to keep clear, so
/// the panel stays where the game put it.
pub(crate) fn place_inspect(
    layout: Res<InspectLayout>,
    view: Res<InspectView>,
    modals: Res<Modals>,
    map_view: Option<Res<MapView>>,
    mut placed: ResMut<InspectPlacement>,
) {
    if !modals.is_open(inspect_modal(&modals)) {
        placed.0 = None;
        return;
    }
    let Some(map_view) = map_view else {
        placed.0 = Some(layout.rect);
        return;
    };
    // Recomputed every frame rather than kept, so a map resized while the
    // panel is moved takes it to the new mirror.
    let elsewhere = other_place(layout.rect, layout.elsewhere, map_view.viewport);
    let moved = placed.0.is_some_and(|rect| rect != layout.rect);
    let (here, there) = if moved { (elsewhere, layout.rect) } else { (layout.rect, elsewhere) };
    let marked = marked(&map_view, view.cursor);
    let moved = moved != (hides(here, &marked) && !hides(there, &marked));
    placed.0 = Some(if moved { elsewhere } else { layout.rect });
}

/// One line saying what someone is working at, from a template.
pub(crate) fn working_line(template: &str, work: &WorkRow) -> String {
    let left = if work.left == 1 { "1 turn".to_string() } else { format!("{} {}", work.left, rl_core::noun::plural("turn")) };
    let line = template
        .replace("{doing}", &work.doing)
        .replace("{target}", work.target.as_deref().unwrap_or(""))
        .replace("{left}", &left)
        .replace("{n}", &work.left.to_string());
    let mut chars = line.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => line,
    }
}

/// Paints the cursor and the panel, while the cursor is open.
pub(crate) fn draw_inspect(
    mut terminal: ResMut<Terminal>,
    layout: Res<InspectLayout>,
    placed: Res<InspectPlacement>,
    view: Res<InspectView>,
    palette: Res<Palette>,
    map: CursorMap,
    time: Res<Time>,
) {
    let Some(rect) = placed.0 else { return };
    // However the game marks a cell it is pointing at, drawn where the
    // cursor is: one style, shared with the nearby rail's highlight and
    // the targeting cursor, so a player learns one mark.
    map.mark(&mut terminal, view.cursor, layout.cursor, &palette, time.elapsed_secs());

    if rect.width < 12 || rect.height < 4 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    frame(&mut terminal, rect, &layout.title, &layout.hints, &palette);
    let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width - 4, rect.height - 2);
    let bg = palette.get(Tones::SURFACE);
    let width = inner.width.max(0) as usize;
    // The ground, always, so a burnt cell says what it burnt to.
    let mut ground = view.ground.clone();
    if view.burning {
        ground.push_str(", burning");
    }
    if let Some(gas) = &view.gas {
        ground.push_str(&format!(", in {gas}"));
    }
    let bottom = inner.bottom();
    let Some(subject) = &view.subject else {
        terminal.print_on(inner.x, inner.y, &clip(&layout.empty, width), palette.get(Tones::MUTED), bg);
        if !ground.is_empty() && inner.y + 1 < bottom {
            terminal.print_on(inner.x, inner.y + 1, &clip(&ground, width), palette.get(Tones::TEXT), bg);
        }
        return;
    };
    let mut y = inner.y;
    terminal.print_on(inner.x, y, &subject.glyph.ch.to_string(), subject.glyph.fg, bg);
    terminal.print_on(inner.x + 2, y, &clip(&subject.label, width.saturating_sub(2)), palette.get(relation_tone(subject.relation)), bg);
    y += 1;
    if let Some((hp, max)) = subject.health
        && y < inner.bottom()
    {
        let reading = format!("health {hp}/{max}");
        terminal.print_on(inner.x, y, &clip(&reading, width), palette.get(Tones::TEXT), bg);
        let x = inner.x + reading.chars().count() as i32 + 1;
        if x + 10 < inner.right() {
            bar(&mut terminal, x, y, 10, subject.health_fraction(), crate::view::health_tone(hp, max), &palette);
        }
        y += 1;
    }
    if y < inner.bottom() {
        terminal.print_on(inner.x, y, &clip(&whereabouts(subject.distance, &ground), width), palette.get(Tones::MUTED), bg);
        y += 1;
    }
    // A thing not yet known is no more than its name here, as in the bag,
    // and what the game says it seems in place of the bare word.
    if view.unknown {
        let said: Vec<(&str, ToneId)> = match view.seems.as_slice() {
            [] => vec![("unknown", Tones::MUTED)],
            seems => seems.iter().map(|f| (f.text.as_str(), f.tone)).collect(),
        };
        for (text, tone) in said.into_iter().take((inner.bottom() - y).max(0) as usize) {
            terminal.print_on(inner.x, y, &clip(text, width), palette.get(tone), bg);
            y += 1;
        }
    }
    if let Some(work) = &subject.work
        && y < inner.bottom()
    {
        let template = if work.target.is_some() { &layout.working } else { &layout.working_alone };
        terminal.print_on(inner.x, y, &clip(&working_line(template, work), width), palette.get(Tones::NOTICE), bg);
        y += 1;
    }
    if let Some(duel) = view.duel
        && y + 1 < inner.bottom()
    {
        y += 1;
        terminal.print_on(inner.x, y, "outlook ", palette.get(Tones::TEXT), bg);
        terminal.print_on(inner.x + 8, y, duel.outlook.label(), palette.get(outlook_tone(duel.outlook)), bg);
        y += 1;
        if y < inner.bottom() {
            let turns = |t: Option<u32>| t.map(|t| t.to_string()).unwrap_or_else(|| "never".into());
            let reading = format!("you fell it in {}, it fells you in {}", turns(duel.turns_to_fell), turns(duel.turns_to_fall));
            terminal.print_on(inner.x, y, &clip(&reading, width), palette.get(Tones::MUTED), bg);
            y += 1;
        }
    }
    // The chance the player's own attack lands, and every line the model
    // gave for it, the same the targeting box prints.
    if let Some(odds) = &view.odds
        && y < inner.bottom()
    {
        let percent = odds.percent();
        terminal.print_on(inner.x, y, &clip(&format!("Chance to hit: {percent}%"), width), palette.get(crate::panel::odds_tone(percent)), bg);
        y += 1;
        for line in &odds.lines {
            if y >= inner.bottom() {
                break;
            }
            terminal.print_on(inner.x, y, &clip(&format!("  {:+} {}", line.value, line.label), width), palette.get(Tones::MUTED), bg);
            y += 1;
        }
    }
    // What a game says about it, each facet wrapped onto as many lines as
    // it takes: a monster's traits are sentences, and a clipped one reads
    // as a different trait. A facet is drawn whole or not at all, and what
    // is left out is counted on the last row rather than cut mid-word.
    let rows = (inner.bottom() - y).max(0) as usize;
    let shown = fitting(view.facets.iter().map(|f| wrap(&f.text, width).len()), rows);
    for facet in &view.facets[..shown] {
        for line in wrap(&facet.text, width) {
            terminal.print_on(inner.x, y, &line, palette.get(facet.tone), bg);
            y += 1;
        }
    }
    let left = view.facets.len() - shown;
    if left > 0 && y < inner.bottom() {
        terminal.print_on(inner.x, y, &clip(&layout.more.replace("{n}", &left.to_string()), width), palette.get(Tones::MUTED), bg);
    }
}

/// How many of a run of facets, each so many lines tall, are drawn whole
/// in `rows`: as many as fit, leaving a row for the line that counts the
/// rest whenever any are left out.
fn fitting(heights: impl ExactSizeIterator<Item = usize>, rows: usize) -> usize {
    let count = heights.len();
    let (mut shown, mut used) = (0, 0);
    for (i, h) in heights.enumerate() {
        let room = if i + 1 == count { rows } else { rows.saturating_sub(1) };
        if used + h > room {
            break;
        }
        shown += 1;
        used += h;
    }
    shown
}

/// The terminal cells the cursor must keep in sight: every cell of its own
/// tile and of the four beside it, where ticks go, which on a map of wide
/// tiles is more than each tile's first. A glowing cursor marks only its
/// own, but the tiles round it are what a player reads it against, so it
/// keeps them too.
fn marked(map: &MapView, cursor: Point) -> Vec<Point> {
    [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().flat_map(|(dx, dy)| map.cells(cursor.offset(dx, dy))).collect()
}

/// How far off the thing under the cursor is, and what it stands on when
/// the ground has a name: `1 tile away`, `3 tiles away, on floor`, and
/// `here, on floor` for what lies where the player stands.
fn whereabouts(distance: i32, ground: &str) -> String {
    let away = match distance {
        0 => "here".to_string(),
        1 => "1 tile away".to_string(),
        _ => format!("{distance} tiles away"),
    };
    if ground.is_empty() { away } else { format!("{away}, on {ground}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::CursorKeys;
    use crate::harness::Stage;

    /// A stage with the map drawn under the panel, so the pointers land on
    /// real cells.
    fn staged() -> Stage {
        Stage::new_with(InspectPanel::new(Rect::new(0, 21, 40, 8)), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)));
        })
        .screen(40, 30)
    }

    fn glyph_at(stage: &Stage, world: Point) -> Option<char> {
        let map = stage.app.world().resource::<MapView>();
        let screen = map.to_screen(world)?;
        stage.app.world().resource::<Terminal>().get(screen.x, screen.y).map(|c| c.glyph)
    }

    /// The cell looked at keeps its own glyph; the four around it frame
    /// it in plain ASCII, on the pulse the clock says.
    #[test]
    fn the_cursor_is_four_ascii_ticks_around_the_cell_and_the_cell_keeps_its_glyph() {
        let mut stage = staged();
        stage.actor("crab", 'c', 2, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        let at = stage.at.offset(2, 0);
        assert_eq!(glyph_at(&stage, at), Some('c'), "still the crab");
        assert_eq!(glyph_at(&stage, at.offset(-1, 0)), Some('-'), "a dash to the left");
        assert_eq!(glyph_at(&stage, at.offset(1, 0)), Some('-'), "and to the right");
        assert_eq!(glyph_at(&stage, at.offset(0, -1)), Some('|'), "a bar above");
        assert_eq!(glyph_at(&stage, at.offset(0, 1)), Some('|'), "and below");
        let t = stage.app.world().resource::<Time>().elapsed_secs();
        let palette = stage.app.world().resource::<Palette>();
        let expected = palette.get(Tones::SELECT).mix(&palette.get(Tones::TITLE), crate::cursor::pulse(t));
        let map = stage.app.world().resource::<MapView>();
        let screen = map.to_screen(at.offset(-1, 0)).unwrap();
        assert_eq!(stage.app.world().resource::<Terminal>().get(screen.x, screen.y).map(|c| c.fg), Some(expected), "on the pulse");
        assert!((0.0..=1.0).contains(&crate::cursor::pulse(0.37)));
    }

    /// On a map of tiles two cells wide the ticks stand on the four tiles
    /// round the cursor, each drawn across its tile as a tile's own glyph
    /// is, so they stand as far from the cell looked at on either side.
    #[test]
    fn on_a_map_of_wide_tiles_the_ticks_stand_centred_on_the_four_tiles_round_the_cursor() {
        let mut stage = Stage::new_with(InspectPanel::new(Rect::new(0, 21, 40, 8)), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)).across(2));
        })
        .screen(40, 30);
        stage.actor("crab", 'c', 2, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        let at = stage.at.offset(2, 0);
        let (view, terminal) = (stage.app.world().resource::<MapView>(), stage.app.world().resource::<Terminal>());
        for (dx, dy, glyph) in [(0, 0, 'c'), (-1, 0, '-'), (1, 0, '-'), (0, -1, '|'), (0, 1, '|')] {
            let first = view.to_screen(at.offset(dx, dy)).expect("on the map");
            let drawn = (terminal.get(first.x, first.y).map(|c| c.glyph), terminal.span(first.x, first.y));
            assert_eq!(drawn, (Some(glyph), Some(2)), "the tile ({dx}, {dy}) from the crab: {glyph:?} across both its cells");
        }
    }

    /// On a map of tiles two cells wide every cell of the tiles round the
    /// cursor is kept in sight: a panel covering only the second cell of
    /// the tile beside it moves off it, as one over the first would.
    #[test]
    fn on_a_map_of_wide_tiles_a_panel_over_half_of_a_tile_round_the_cursor_moves_off_it() {
        let mut stage = Stage::new_with(InspectPanel::new(Rect::new(23, 5, 14, 10)).else_at(Rect::new(23, 12, 14, 8)), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)).across(2));
        })
        .screen(40, 20);
        let east = stage.app.world().resource::<MapView>().cells(stage.at.offset(1, 0)).collect::<Vec<_>>();
        assert_eq!(east, [Point::new(22, 10), Point::new(23, 10)], "the tile east of the player, the second of its cells under the panel");
        stage.press(CursorKeys::default().look);
        assert_eq!(title_row(&stage), Some(12), "the panel is drawn in its other place");
    }

    /// The panel over the bottom of the map, where every game puts it.
    fn overlaid() -> Stage {
        Stage::new_with(InspectPanel::new(Rect::new(1, 12, 30, 7)), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)));
        })
        .screen(40, 20)
    }

    fn title_row(stage: &Stage) -> Option<i32> {
        stage.rows().iter().position(|r| r.contains("Looking at")).map(|y| y as i32)
    }

    /// Something under the panel is looked at with the panel moved off it,
    /// so the cell and all four ticks show.
    #[test]
    fn looking_at_something_under_the_panel_moves_the_panel_to_the_other_side_of_the_map() {
        let mut stage = overlaid();
        stage.press(CursorKeys::default().look);
        assert_eq!(title_row(&stage), Some(12), "where the game put it, while the cursor is clear of it");
        stage.actor("crab", 'c', 0, 5);
        stage.tick();
        stage.press(CursorKeys::default().next);
        let at = stage.at.offset(0, 5);
        assert_eq!(glyph_at(&stage, at), Some('c'), "the crab shows");
        assert_eq!(glyph_at(&stage, at.offset(0, 1)), Some('|'), "and so does the tick below it");
        assert_eq!(title_row(&stage), Some(1), "mirrored top for bottom in the map");
    }

    /// The panel stays on the side it moved to until the cursor goes under
    /// it there, so it does not jump with every step across the middle.
    #[test]
    fn the_panel_stays_where_it_moved_to_until_the_cursor_goes_under_it_again() {
        let mut stage = overlaid();
        let crab = stage.actor("crab", 'c', 0, 5);
        stage.tick();
        stage.press(CursorKeys::default().look);
        stage.press(CursorKeys::default().next);
        assert_eq!(title_row(&stage), Some(1), "moved off the crab");
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::ArrowUp);
        stage.press(KeyCode::ArrowUp);
        assert_eq!(title_row(&stage), Some(1), "clear of the panel where it is now, so it stays");
        for _ in 0..6 {
            stage.press(KeyCode::ArrowUp);
        }
        assert_eq!(title_row(&stage), Some(12), "under it at the top, so back to the bottom");
        stage.press(CursorKeys::default().next);
        assert_eq!(title_row(&stage), Some(1), "back on the crab, so at the top");
        stage.press(CursorKeys::default().close);
        stage.app.world_mut().despawn(crab);
        stage.press(CursorKeys::default().look);
        assert_eq!(title_row(&stage), Some(12), "a fresh look on the player's own tile starts where the game put it");
    }

    /// A game that wants the panel somewhere other than the mirror says so.
    #[test]
    fn a_game_can_name_where_the_panel_goes_instead() {
        let mut stage = Stage::new_with(InspectPanel::new(Rect::new(1, 12, 30, 7)).else_at(Rect::new(8, 3, 30, 7)), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)));
        })
        .screen(40, 20);
        stage.actor("crab", 'c', 0, 5);
        stage.tick();
        stage.press(CursorKeys::default().look);
        stage.press(CursorKeys::default().next);
        assert_eq!(title_row(&stage), Some(3));
    }

    /// The ground is named whether or not something stands on it.
    #[test]
    fn the_chance_to_hit_and_what_shaped_it_are_printed_under_the_forecast() {
        let mut stage = Stage::new_with(InspectPanel::new(Rect::new(0, 21, 40, 12)), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)));
            app.insert_resource(rl_bevy::HitRules(Box::new(rl_rules::Percent::new(5, 16, 30))));
        })
        .screen(40, 34);
        let (player, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(player).insert(rl_bevy::RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(3));
        stage.actor("droid", 'd', 5, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        let rows: Vec<String> = stage.rows().iter().map(|r| r.trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string()).collect();
        assert!(rows.iter().any(|r| r == "Chance to hit: 90%"), "{rows:#?}");
        assert!(rows.iter().any(|r| r == "  -10 for range"), "{rows:#?}");
    }

    #[test]
    fn the_panel_names_the_ground_under_the_cursor() {
        let mut stage = staged();
        stage.press(CursorKeys::default().look);
        assert_eq!(stage.row(22).trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end(), "Nothing here.");
        assert_eq!(stage.row(23).trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end(), "floor", "what the ground is called");
        stage.actor("crab", 'c', 2, 0);
        stage.tick();
        stage.press(CursorKeys::default().next);
        let rows = stage.rows();
        assert!(rows.iter().any(|r| r.contains("2 tiles away, on floor")), "{rows:#?}");
    }

    /// A thing not yet known under the cursor is named, placed and said
    /// to be unknown, and nothing more.
    #[test]
    fn a_thing_not_yet_known_is_said_to_be_unknown_under_where_it_lies() {
        let mut stage = staged();
        let vial = stage.thing("murky vial", '!', 2, 0);
        stage.app.world_mut().entity_mut(vial).insert(rl_bevy::Unknown);
        stage.tick();
        stage.press(CursorKeys::default().look);
        let inner = |y: i32| stage.row(y).trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string();
        assert_eq!([inner(22), inner(23), inner(24), inner(25)], ["! murky vial", "2 tiles away, on floor", "unknown", ""]);
    }

    /// A stage looking at a crab the game says `facets` about, in a panel
    /// `height` rows tall and thirty wide, and the panel's inner rows.
    fn looked_at_with(facets: &'static [&'static str], height: i32) -> (Stage, Vec<String>) {
        let mut stage = Stage::new_with(InspectPanel::new(Rect::new(0, 21, 30, height)), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)));
        })
        .screen(40, 21 + height);
        let annotate = move |mut view: ResMut<InspectView>| {
            if view.subject.is_some() {
                view.facets.extend(facets.iter().map(|text| crate::Facet::new("trait", *text)));
            }
        };
        stage.app.add_systems(Update, annotate.in_set(crate::ViewSet::Annotate));
        stage.actor("crab", 'c', 2, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        stage.press(CursorKeys::default().next);
        let inner = (22..20 + height).map(|y| stage.row(y).trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string()).collect();
        (stage, inner)
    }

    /// Six traits, each too long for one line: thirty wide holds a line of
    /// twenty-six inside the frame and its margins.
    const TRAITS: &[&str] = &[
        "hunts in packs and keeps near its kind",
        "sees in the dark as well as by day",
        "spits venom that lingers in the blood",
        "spins webs that hold whoever walks in",
        "flees when hurt and comes back healed",
        "never sleeps, and hears a pin drop",
    ];

    /// A monster with five traits needs them all: each wraps onto as many
    /// lines as it takes, on spaces, and what the rectangle has no room for
    /// is counted on a muted line of its own rather than cut off mid-word.
    #[test]
    fn five_long_facets_wrap_within_the_rectangle_and_a_sixth_says_more() {
        let (_, tall) = looked_at_with(TRAITS, 30);
        let first = tall.iter().position(|r| r.starts_with("hunts")).expect("the first trait is drawn");
        // Exactly the rows the header, five traits of two lines each and
        // the line saying more take.
        let height = first as i32 + 10 + 1 + 2;
        let (stage, rows) = looked_at_with(TRAITS, height);
        let traits = &rows[first..first + 10];
        for (i, text) in TRAITS[..5].iter().enumerate() {
            let lines = &traits[2 * i..2 * i + 2];
            assert!(lines.iter().all(|l| !l.is_empty() && l.chars().count() <= 26), "two lines inside the frame: {lines:?}");
            assert_eq!(lines.join(" "), *text, "the whole trait, broken on a space");
        }
        assert_eq!(rows[first + 10], "and 1 more", "the sixth, counted: {rows:#?}");
        assert_eq!(rows.len(), first + 11, "on the last row inside the frame");
        let palette = stage.app.world().resource::<Palette>().clone();
        let more_y = 22 + first as i32 + 10;
        assert_eq!(stage.app.world().resource::<Terminal>().get(2, more_y).map(|c| c.fg), Some(palette.get(Tones::MUTED)), "in the panel's muted tone");
        assert!(stage.row(more_y + 1).starts_with('\u{2514}'), "and the bottom border under it");

        let (_, roomy) = looked_at_with(TRAITS, height + 1);
        assert_eq!(roomy[first + 10..].join(" "), TRAITS[5], "with room for the sixth, it is drawn and nothing says more");
    }

    /// What the game says a thing not yet known seems is printed in place
    /// of the bare `unknown`, a line apiece in its own tone, with what else
    /// it added under it.
    #[test]
    fn what_a_game_says_a_thing_not_yet_known_seems_is_said_in_place_of_unknown() {
        let mut stage = staged();
        let vial = stage.thing("vial", '!', 2, 0);
        stage.app.world_mut().entity_mut(vial).insert(rl_bevy::Unknown);
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<InspectView>| {
                view.seems.push(crate::Facet::new("seems", "strength not yet known").toned(Tones::BAD));
                view.facets.push(crate::Facet::new("holds", "holds a draught"));
            })
            .in_set(crate::ViewSet::Annotate),
        );
        stage.tick();
        stage.press(CursorKeys::default().look);
        let inner = |y: i32| stage.row(y).trim_start_matches("\u{2502} ").trim_end_matches('\u{2502}').trim_end().to_string();
        assert_eq!([inner(22), inner(23), inner(24), inner(25)], ["! vial", "2 tiles away, on floor", "strength not yet known", "holds a draught"]);
        let palette = stage.app.world().resource::<Palette>();
        assert_eq!(stage.app.world().resource::<Terminal>().get(2, 24).map(|c| c.fg), Some(palette.get(Tones::BAD)), "in its own tone");
    }

    /// The bar beside a looked-at monster's health reads in the tone the
    /// vitals, the nearby rail and the party rail give the same health, so
    /// a crab at a fifth is bad here as it is everywhere else.
    #[test]
    fn a_looked_at_monsters_health_bar_reads_in_the_tone_every_other_rail_gives_the_same_health() {
        let mut stage = staged();
        let palette = stage.app.world().resource::<Palette>().clone();
        let crab = stage.actor("crab", 'c', 2, 0);
        stage.tick();
        stage.press(CursorKeys::default().look);
        for (hp, tone) in [(10, Tones::GOOD), (5, Tones::NOTICE), (2, Tones::BAD)] {
            stage.app.world_mut().get_mut::<rl_bevy::Health>(crab).unwrap().current = hp;
            stage.tick();
            let reading = format!("health {hp}/10");
            let y = stage.rows().iter().position(|r| r.contains(&reading)).unwrap_or_else(|| panic!("{reading} drawn: {:#?}", stage.rows())) as i32;
            let terminal = stage.app.world().resource::<Terminal>();
            let filled = (0..40).find_map(|x| terminal.get(x, y).filter(|c| c.glyph == '\u{2588}')).expect("a filled bar cell");
            assert_eq!(filled.fg, palette.get(tone), "at {hp} of 10");
        }
    }

    #[test]
    fn one_tile_off_is_one_tile_and_any_other_distance_is_tiles() {
        assert_eq!(whereabouts(1, "deck"), "1 tile away, on deck");
        assert_eq!(whereabouts(2, ""), "2 tiles away");
    }

    /// What lies where the player stands, the stairs they arrived by or
    /// what they dropped, is here, not nought tiles away.
    #[test]
    fn something_underfoot_is_here() {
        assert_eq!(whereabouts(0, "ash"), "here, on ash");
        assert_eq!(whereabouts(0, ""), "here");
    }

    #[test]
    fn the_working_line_reads_as_a_sentence_and_counts_turns_right() {
        let work = |target: Option<&str>, left| crate::view::WorkRow { doing: "mending".into(), target: target.map(String::from), left };
        let layout = InspectPanel::new(Rect::new(0, 0, 30, 12)).0;
        assert_eq!(working_line(&layout.working, &work(Some("rag doll"), 4)), "Mending the rag doll, 4 turns left");
        assert_eq!(working_line(&layout.working, &work(Some("rag doll"), 1)), "Mending the rag doll, 1 turn left");
        assert_eq!(working_line(&layout.working_alone, &work(None, 2)), "Mending, 2 turns left");
        assert_eq!(working_line("{doing}: {n} cycles", &work(None, 2)), "Mending: 2 cycles", "a game's own template, with the bare number");
    }
}
