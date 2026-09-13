//! The vitals strip: bars, badges, armor, turn and position.
//!
//! One row when the rectangle is one row tall, which is the status line
//! every game was building by hand; more rows when it is taller, which is
//! the top of a rail.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::Rect;
use rl_render::Terminal;

use crate::panel::{bar, clear, clip};
use crate::tone::{Palette, Tones};
use crate::view::{VitalsView, VitalsViewPlugin};

/// Where the vitals are drawn.
#[derive(Resource, Debug, Clone)]
pub struct VitalsLayout {
    /// The terminal cells it occupies.
    pub rect: Rect,
    /// Key hints printed at the right of the last row. The game's, since
    /// the engine does not know what a game bound.
    pub hints: String,
    /// Cells given to each bar.
    pub bar_width: i32,
    /// Whether to print the turn and the position.
    pub show_whereabouts: bool,
    /// A heading with a rule under it, over everything else. Empty draws
    /// none, which is what a one-row strip wants.
    pub heading: String,
}

/// Draws [`VitalsView`].
///
/// Adds [`VitalsViewPlugin`] if the game has not.
pub struct VitalsPanel(VitalsLayout);

impl VitalsPanel {
    /// Vitals in `rect`.
    pub fn new(rect: Rect) -> Self {
        Self(VitalsLayout { rect, hints: String::new(), bar_width: 10, show_whereabouts: true, heading: String::new() })
    }

    /// Sets the key hints at the right of the last row.
    pub fn hints(mut self, hints: impl Into<String>) -> Self {
        self.0.hints = hints.into();
        self
    }

    /// Sets how many cells a bar gets.
    pub fn bars(mut self, width: i32) -> Self {
        self.0.bar_width = width;
        self
    }

    /// Sets a heading with a rule under it. A panel two rows or taller
    /// reads as part of a rail with one and as floating text without.
    pub fn heading(mut self, heading: impl Into<String>) -> Self {
        self.0.heading = heading.into();
        self
    }

    /// Leaves the turn and position off, for a game that shows them
    /// somewhere else or not at all.
    pub fn without_whereabouts(mut self) -> Self {
        self.0.show_whereabouts = false;
        self
    }
}

impl Plugin for VitalsPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<VitalsViewPlugin>() {
            app.add_plugins(VitalsViewPlugin);
        }
        app.insert_resource(self.0.clone()).add_systems(Update, draw_vitals.in_set(PresentSet::Chrome));
    }
}

/// Paints the vitals.
pub fn draw_vitals(mut terminal: ResMut<Terminal>, layout: Res<VitalsLayout>, view: Res<VitalsView>, palette: Res<Palette>) {
    let rect = layout.rect;
    if rect.width < 8 || rect.height < 1 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    if view.entity.is_none() {
        return;
    }
    let bg = palette.get(Tones::SURFACE);
    let text = palette.get(Tones::TEXT);
    let muted = palette.get(Tones::MUTED);
    let mut y = rect.y;
    let bottom = rect.bottom();

    if !layout.heading.is_empty() && rect.height > 2 {
        crate::panel::section(&mut terminal, rect, y, &layout.heading, None, &palette);
        y += 2;
    }
    if !view.label.is_empty() && rect.height > 1 {
        terminal.print_on(rect.x, y, &clip(&view.label, rect.width as usize), palette.get(Tones::TITLE), bg);
        y += 1;
    }
    for gauge in &view.bars {
        if y >= bottom {
            break;
        }
        let reading = format!("{} {}/{}", gauge.label, gauge.value, gauge.max);
        terminal.print_on(rect.x, y, &clip(&reading, rect.width as usize), text, bg);
        let bar_x = rect.x + 1 + reading.chars().count() as i32;
        if bar_x + layout.bar_width < rect.right() {
            bar(&mut terminal, bar_x, y, layout.bar_width, gauge.fraction(), gauge.tone, &palette);
        }
        // A one-row strip keeps everything on the same line.
        if rect.height > 1 {
            y += 1;
        } else {
            let after = bar_x + layout.bar_width + 2;
            draw_tail(&mut terminal, rect, y, after, &view, &layout, &palette);
            return;
        }
    }
    if let Some(armor) = view.armor
        && y < bottom
    {
        terminal.print_on(rect.x, y, &format!("armor {armor}"), text, bg);
        y += 1;
    }
    if !view.badges.is_empty() && y < bottom {
        let badges: String = view.badges.iter().map(|b| b.text.as_str()).collect();
        terminal.print_on(rect.x, y, &clip(&badges, rect.width as usize), palette.get(Tones::NOTICE), bg);
        y += 1;
    }
    for facet in &view.facets {
        if y >= bottom {
            break;
        }
        terminal.print_on(rect.x, y, &clip(&facet.text, rect.width as usize), palette.get(facet.tone), bg);
        y += 1;
    }
    if layout.show_whereabouts && y < bottom {
        let where_ = format!("turn {}  ({}, {})", view.turn, view.position.x, view.position.y);
        terminal.print_on(rect.x, y, &clip(&where_, rect.width as usize), muted, bg);
    }
    if !layout.hints.is_empty() {
        let x = rect.right() - 1 - layout.hints.chars().count() as i32;
        terminal.print_on(x.max(rect.x), bottom - 1, &layout.hints, muted, bg);
    }
}

/// The rest of a one-row strip, after the health bar.
fn draw_tail(terminal: &mut Terminal, rect: Rect, y: i32, x: i32, view: &VitalsView, layout: &VitalsLayout, palette: &Palette) {
    let bg = palette.get(Tones::SURFACE);
    let mut parts: Vec<(String, bevy::prelude::Color)> = Vec::new();
    if let Some(armor) = view.armor {
        parts.push((format!("armor {armor}"), palette.get(Tones::TEXT)));
    }
    if !view.badges.is_empty() {
        parts.push((view.badges.iter().map(|b| b.text.as_str()).collect(), palette.get(Tones::NOTICE)));
    }
    for facet in &view.facets {
        parts.push((facet.text.clone(), palette.get(facet.tone)));
    }
    if layout.show_whereabouts {
        parts.push((format!("turn {}  ({}, {})", view.turn, view.position.x, view.position.y), palette.get(Tones::MUTED)));
    }
    let hints = layout.hints.chars().count() as i32;
    let limit = rect.right() - if hints > 0 { hints + 2 } else { 0 };
    let mut x = x;
    for (text, color) in parts {
        let width = text.chars().count() as i32;
        if x + width > limit {
            break;
        }
        terminal.print_on(x, y, &text, color, bg);
        x += width + 2;
    }
    if hints > 0 {
        terminal.print_on(rect.right() - 1 - hints, y, &layout.hints, palette.get(Tones::MUTED), bg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;

    #[test]
    fn a_one_row_strip_keeps_everything_on_the_line_and_ends_with_the_hints() {
        let stage = Stage::new(VitalsPanel::new(Rect::new(0, 0, 60, 1)).bars(6).hints("[q]uit")).screen(60, 3);
        let row = stage.row(0);
        assert!(row.starts_with("health 30/30"), "the reading first: {row:?}");
        assert!(row.contains('\u{2588}'), "then the bar: {row:?}");
        assert!(row.contains("armor 0"), "then armor: {row:?}");
        assert!(row.contains("turn 0"), "then the whereabouts: {row:?}");
        assert!(row.ends_with("[q]uit"), "hints last: {row:?}");
    }

    #[test]
    fn a_taller_panel_puts_the_name_first_and_a_line_per_reading() {
        let stage = Stage::new(VitalsPanel::new(Rect::new(0, 0, 24, 6)).bars(6)).screen(24, 6);
        let rows = stage.rows();
        assert_eq!(rows[0], "you", "the name has the top line");
        assert!(rows[1].starts_with("health 30/30"), "{:?}", rows[1]);
        assert_eq!(rows[2], "armor 0");
        assert!(rows[3].starts_with("turn 0"), "{:?}", rows[3]);
    }

    #[test]
    fn a_facet_a_game_pushed_is_drawn_in_the_tone_it_asked_for() {
        let mut stage = Stage::new(VitalsPanel::new(Rect::new(0, 0, 24, 6))).screen(24, 6);
        stage.app.add_systems(
            Update,
            (|mut view: ResMut<VitalsView>, mut facets: ResMut<crate::Facets>| {
                view.facets.push(facets.facet("here", "here: rum [g]").toned(crate::Tones::NOTICE));
            })
            .in_set(crate::ViewSet::Annotate),
        );
        stage.tick();
        let rows = stage.rows();
        assert_eq!(rows[3], "here: rum [g]", "after the engine's own readings: {rows:?}");
        let cell = stage.app.world().resource::<rl_render::Terminal>().get(0, 3).unwrap();
        let palette = stage.app.world().resource::<Palette>();
        assert_eq!(cell.fg, palette.get(crate::Tones::NOTICE), "in the tone the game asked for");
    }
}
