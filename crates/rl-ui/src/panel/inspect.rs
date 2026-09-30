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

use crate::cursor::CursorStyle;
use crate::modal::Modals;
use crate::panel::nearby::relation_tone;
use crate::panel::{bar, clear, clip, frame};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::inspect::inspect_modal;
use crate::view::{InspectView, InspectViewPlugin, WorkRow};

/// Where the inspect panel is drawn.
#[derive(Resource, Debug, Clone)]
pub struct InspectLayout {
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
pub fn outlook_tone(outlook: Outlook) -> ToneId {
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
pub fn place_inspect(
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
    let elsewhere = layout.elsewhere.unwrap_or_else(|| mirrored(layout.rect, map_view.viewport));
    let moved = placed.0.is_some_and(|rect| rect != layout.rect);
    let (here, there) = if moved { (elsewhere, layout.rect) } else { (layout.rect, elsewhere) };
    let marked = marked(&map_view, view.cursor);
    let moved = moved != (hides(here, &marked) && !hides(there, &marked));
    placed.0 = Some(if moved { elsewhere } else { layout.rect });
}

/// One line saying what someone is working at, from a template.
pub fn working_line(template: &str, work: &WorkRow) -> String {
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
pub fn draw_inspect(
    mut terminal: ResMut<Terminal>,
    layout: Res<InspectLayout>,
    placed: Res<InspectPlacement>,
    view: Res<InspectView>,
    palette: Res<Palette>,
    map_view: Option<Res<MapView>>,
    time: Res<Time>,
) {
    let Some(rect) = placed.0 else { return };
    // However the game marks a cell it is pointing at, drawn where the
    // cursor is: one style, shared with the nearby rail's highlight and
    // the targeting cursor, so a player learns one mark.
    if let Some(map_view) = map_view {
        crate::cursor::mark(&mut terminal, &map_view, view.cursor, layout.cursor, &palette, time.elapsed_secs());
    }

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
            bar(&mut terminal, x, y, 10, subject.health_fraction(), Tones::GOOD, &palette);
        }
        y += 1;
    }
    if y < inner.bottom() {
        terminal.print_on(inner.x, y, &clip(&whereabouts(subject.distance, &ground), width), palette.get(Tones::MUTED), bg);
        y += 1;
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
    for facet in &view.facets {
        if y >= inner.bottom() {
            break;
        }
        terminal.print_on(inner.x, y, &clip(&facet.text, width), palette.get(facet.tone), bg);
        y += 1;
    }
}

/// `rect` turned top for bottom within `map`, so a panel along the
/// bottom edge lands along the top one the same distance in.
fn mirrored(rect: Rect, map: Rect) -> Rect {
    Rect::new(rect.x, map.y + map.bottom() - rect.bottom(), rect.width, rect.height)
}

/// The terminal cells the cursor must keep in sight: its own and the four
/// beside it, where ticks go. A glowing cursor marks only its own, but the
/// cells round it are what a player reads it against, so it keeps them
/// too.
fn marked(map: &MapView, cursor: Point) -> Vec<Point> {
    [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().filter_map(|(dx, dy)| map.to_screen(cursor.offset(dx, dy))).collect()
}

fn hides(rect: Rect, cells: &[Point]) -> bool {
    cells.iter().any(|&p| rect.contains(p))
}

/// How far off the thing under the cursor is, and what it stands on when
/// the ground has a name: `1 tile away`, `3 tiles away, on floor`.
fn whereabouts(distance: i32, ground: &str) -> String {
    let away = if distance == 1 { "1 tile away".to_string() } else { format!("{distance} tiles away") };
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

    #[test]
    fn one_tile_off_is_one_tile_and_any_other_distance_is_tiles() {
        assert_eq!(whereabouts(1, "deck"), "1 tile away, on deck");
        assert_eq!(whereabouts(2, ""), "2 tiles away");
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
