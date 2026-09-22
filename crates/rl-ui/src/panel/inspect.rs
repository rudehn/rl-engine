//! What the look cursor is over, and how a fight with it would go.
//!
//! Drawn in [`PresentSet::Overlay`], over the map, because the cursor is a
//! modal and a modal covers what it is about. The cursor is drawn as
//! four ticks on the cells around the one it sits on, a dash either side
//! and a bar above and below, pulsing, so the cell itself shows what is
//! there, and the panel and the cursor never disagree about what is being
//! described. The ticks are ASCII because a browser build draws with
//! Bevy's built-in font alone, which has no arrows worth the name.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::{MapView, Terminal};
use rl_rules::forecast::Outlook;

use crate::cursor::CursorStyle;
use crate::modal::Modals;
use crate::panel::nearby::relation_tone;
use crate::panel::{bar, clear, clip, frame};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::inspect::inspect_modal;
use crate::view::{InspectView, InspectViewPlugin};

/// Where the inspect panel is drawn.
#[derive(Resource, Debug, Clone)]
pub struct InspectLayout {
    /// The terminal cells it occupies, border included.
    pub rect: Rect,
    /// The title in the top border.
    pub title: String,
    /// The key hints in the bottom border.
    pub hints: String,
    /// What is shown when the cursor is over nothing.
    pub empty: String,
    /// How the cell the cursor sits on is marked. Four ASCII ticks round
    /// it by default, breathing, so the cell keeps its own glyph.
    pub cursor: CursorStyle,
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
            title: "Looking at".into(),
            hints: "move \u{2022} tab next \u{2022} esc close".into(),
            empty: "Nothing here.".into(),
            cursor: CursorStyle::ticks(),
        })
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
        app.insert_resource(self.0.clone()).add_systems(Update, draw_inspect.in_set(PresentSet::Overlay));
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

/// Paints the cursor and the panel, while the cursor is open.
pub fn draw_inspect(
    mut terminal: ResMut<Terminal>,
    layout: Res<InspectLayout>,
    view: Res<InspectView>,
    palette: Res<Palette>,
    modals: Res<Modals>,
    map_view: Option<Res<MapView>>,
    time: Res<Time>,
) {
    if !modals.is_open(inspect_modal(&modals)) {
        return;
    }
    // However the game marks a cell it is pointing at, drawn where the
    // cursor is: one style, shared with the nearby rail's highlight and
    // the targeting cursor, so a player learns one mark.
    if let Some(map_view) = map_view {
        crate::cursor::mark(&mut terminal, &map_view, view.cursor, layout.cursor, &palette, time.elapsed_secs());
    }

    let rect = layout.rect;
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
        let whereabouts = if ground.is_empty() { format!("{} tiles away", subject.distance) } else { format!("{} tiles away, on {ground}", subject.distance) };
        terminal.print_on(inner.x, y, &clip(&whereabouts, width), palette.get(Tones::MUTED), bg);
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
    for facet in &view.facets {
        if y >= inner.bottom() {
            break;
        }
        terminal.print_on(inner.x, y, &clip(&facet.text, width), palette.get(facet.tone), bg);
        y += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::CursorKeys;
    use crate::harness::Stage;
    use rl_core::Point;

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

    /// The ground is named whether or not something stands on it.
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
}
