//! What the look cursor is over, and how a fight with it would go.
//!
//! Drawn in [`PresentSet::Overlay`], over the map, because the cursor is a
//! modal and a modal covers what it is about. The cursor itself is drawn
//! on the map cell it sits on, so the panel and the cursor never disagree
//! about what is being described.

use bevy::prelude::*;
use rl_bevy::{PresentSet, WorldMap};
use rl_core::Rect;
use rl_render::{Cell, MapView, Terminal};
use rl_rules::forecast::Outlook;

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
    /// The glyph the cursor is drawn as on the map.
    pub cursor: char,
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
            cursor: '\u{2588}',
        })
    }

    /// Sets the title in the top border.
    pub fn titled(mut self, title: impl Into<String>) -> Self {
        self.0.title = title.into();
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
    map: Option<Res<WorldMap>>,
) {
    if !modals.is_open(inspect_modal(&modals)) {
        return;
    }
    let _ = &map;
    if let Some(map_view) = map_view
        && let Some(screen) = map_view.to_screen(view.cursor)
    {
        let under = terminal.get(screen.x, screen.y).unwrap_or_default();
        terminal.set(screen.x, screen.y, Cell::new(layout.cursor, palette.get(Tones::SELECT)).on(under.fg));
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
    let Some(subject) = &view.subject else {
        terminal.print_on(inner.x, inner.y, &clip(&layout.empty, width), palette.get(Tones::MUTED), bg);
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
        terminal.print_on(inner.x, y, &clip(&format!("{} tiles away", subject.distance), width), palette.get(Tones::MUTED), bg);
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
