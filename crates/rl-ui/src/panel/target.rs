//! The targeting overlay: the footprint, painted over the map the frame
//! already drew.
//!
//! Not a framed box like the other panels. An aim is about the map, so
//! this reads back the cells [`draw_map`](rl_render::map_view::draw_map)
//! wrote and repaints their backgrounds, keeping every glyph where it is:
//! the monster under the cursor stays the monster, lit differently.
//!
//! Three colours and no more, because a fourth stops reading at a glance:
//! the cells that will be hit, the flight the projectile takes to get
//! there, and, when the aim is one the resolver would refuse, the whole
//! footprint in the tone for bad news.

use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::{Point, Rect};
use rl_render::{MapView, Terminal};

use crate::panel::clip;
use crate::tone::{Palette, Tones};
use crate::view::target::{TargetView, TargetViewPlugin};

/// Where the aim is described in words.
#[derive(Resource, Debug, Clone)]
pub struct TargetLayout {
    /// One row for the line naming the ability and what is under the
    /// cursor. Zero-height draws none and leaves only the footprint.
    pub rect: Rect,
    /// Key hints at the right of the row.
    pub hints: String,
}

/// Draws [`TargetView`]: the footprint on the map, and a line saying what
/// is being aimed at what.
///
/// Adds [`TargetViewPlugin`] if the game has not.
pub struct TargetPanel(TargetLayout);

impl TargetPanel {
    /// The banner in `rect`. Pass a zero-height rectangle for the
    /// footprint alone.
    pub fn new(rect: Rect) -> Self {
        Self(TargetLayout { rect, hints: String::new() })
    }

    /// Sets the key hints at the right of the row.
    pub fn hints(mut self, hints: impl Into<String>) -> Self {
        self.0.hints = hints.into();
        self
    }
}

impl Plugin for TargetPanel {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<TargetViewPlugin>() {
            app.add_plugins(TargetViewPlugin);
        }
        app.insert_resource(self.0.clone()).add_systems(Update, draw_target.in_set(PresentSet::Overlay));
    }
}

/// Paints the footprint and the banner.
pub fn draw_target(
    mut terminal: ResMut<Terminal>,
    layout: Res<TargetLayout>,
    view: Res<TargetView>,
    map: Option<Res<MapView>>,
    abilities: Option<Res<rl_bevy::Abilities>>,
    palette: Res<Palette>,
) {
    if !view.aiming() {
        return;
    }
    let Some(map) = map else { return };
    let ground = palette.get(if view.legal { Tones::SELECT } else { Tones::BAD });
    let flight = palette.get(Tones::NOTICE);

    // The flight first, so a cell that is both lands as a hit rather than
    // as the path it arrived by.
    for cell in view.path.iter().filter(|p| !view.cells.contains(p)) {
        tint(&mut terminal, &map, *cell, flight);
    }
    for cell in &view.cells {
        tint(&mut terminal, &map, *cell, ground);
    }
    // The cursor itself last and brightest, since a ball's burst can
    // cover it and a player needs to know where the keys are moving.
    if let Some(screen) = map.to_screen(view.cursor)
        && let Some(mut cell) = terminal.get(screen.x, screen.y)
    {
        cell.bg = palette.get(Tones::TITLE);
        terminal.set(screen.x, screen.y, cell);
    }

    let rect = layout.rect;
    if rect.height < 1 || rect.width < 8 {
        return;
    }
    let bg = palette.get(Tones::SURFACE);
    terminal.fill(rect, rl_render::Cell::new(' ', bg).on(bg));
    let name = abilities.as_deref().and_then(|a| view.ability.map(|id| a.get(id).name.clone())).unwrap_or_default();
    let at = match view.targets.as_slice() {
        [] => "nothing".to_string(),
        [one] if !one.label.is_empty() => one.label.clone(),
        [_] => "one of them".to_string(),
        many => format!("{} of them", many.len()),
    };
    // Red says no; the reason says why, because a cursor that refuses
    // without saying what is wrong is a cursor the player argues with.
    let line = match view.why.first() {
        None => format!("{name} at {at}"),
        Some(reason) => format!("{name} at {at} - {}", crate::view::ability::plain(reason)),
    };
    let tone = if view.legal { Tones::NOTICE } else { Tones::BAD };
    terminal.print_on(rect.x, rect.y, &clip(&line, rect.width as usize), palette.get(tone), bg);
    if !layout.hints.is_empty() {
        let x = rect.right() - 1 - layout.hints.chars().count() as i32;
        if x > rect.x + line.chars().count() as i32 {
            terminal.print_on(x, rect.y, &layout.hints, palette.get(Tones::MUTED), bg);
        }
    }
}

/// Repaints one cell's background, leaving whatever glyph is on it.
fn tint(terminal: &mut Terminal, map: &MapView, cell: Point, bg: Color) {
    let Some(screen) = map.to_screen(cell) else { return };
    let Some(mut drawn) = terminal.get(screen.x, screen.y) else { return };
    drawn.bg = bg;
    terminal.set(screen.x, screen.y, drawn);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Stage;
    use crate::view::target::AimAt;
    use crate::view::target::harness::{abilities, arm};
    use rl_bevy::{Abilities, AbilitiesPlugin, AddEngineEffects};

    /// A stage with the map drawn under the overlay, so the test sees the
    /// same cells a player would.
    fn staged() -> Stage {
        let mut stage = Stage::new_with((AbilitiesPlugin, TargetPanel::new(Rect::new(0, 0, 40, 1)).hints("[enter]")), |app| {
            app.add_engine_effects();
            abilities(app);
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 1, 40, 20)));
        });
        stage.tick();
        stage
    }

    fn bg_at(stage: &Stage, world: Point) -> Option<Color> {
        let map = stage.app.world().resource::<MapView>();
        let screen = map.to_screen(world)?;
        stage.app.world().resource::<Terminal>().get(screen.x, screen.y).map(|c| c.bg)
    }

    /// The footprint is painted over the map the frame already drew, and
    /// the banner says what is being aimed at what.
    #[test]
    fn the_footprint_is_tinted_over_the_map_and_the_banner_names_it() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("them", 't', 2, 0);
        stage.tick();
        let at = stage.at;
        let plain = bg_at(&stage, at.offset(0, 3)).expect("a cell off the footprint");

        let user = stage.player;
        stage.app.world_mut().write_message(AimAt { user, ability: bolt });
        stage.tick();

        let palette = stage.app.world().resource::<Palette>().clone();
        assert_eq!(bg_at(&stage, at.offset(2, 0)), Some(palette.get(Tones::TITLE)), "the cursor is brightest");
        assert_eq!(bg_at(&stage, at.offset(1, 0)), Some(palette.get(Tones::NOTICE)), "the flight to it");
        assert_eq!(bg_at(&stage, at.offset(0, 3)), Some(plain), "and nothing else moved");
        assert_eq!(stage.row(0), "bolt at them                    [enter]");
    }

    /// An aim the resolver would refuse is painted in the tone for bad
    /// news, so it reads as wrong before the turn is spent.
    #[test]
    fn an_illegal_aim_is_painted_as_one() {
        let mut stage = staged();
        arm(&mut stage);
        let dear = stage.app.world().resource::<Abilities>().expect("dear");
        stage.actor("them", 't', 2, 0);
        stage.tick();

        let user = stage.player;
        stage.app.world_mut().write_message(AimAt { user, ability: dear });
        stage.tick();

        let at = stage.at;
        let palette = stage.app.world().resource::<Palette>().clone();
        assert_eq!(bg_at(&stage, at.offset(2, 0)), Some(palette.get(Tones::TITLE)), "the cursor is still the cursor");
        assert_eq!(stage.row(0), "dear at them - cannot pay       [enter]");
        // The landing cell is the one the footprint covers, and an
        // illegal aim paints it in the bad tone rather than the select.
        let view = stage.app.world().resource::<TargetView>();
        assert!(!view.legal);
        assert_ne!(palette.get(Tones::BAD), palette.get(Tones::SELECT), "the two tones are told apart");
    }

    /// Nothing is drawn while nothing is being aimed.
    #[test]
    fn a_closed_cursor_draws_nothing() {
        let mut stage = staged();
        arm(&mut stage);
        stage.tick();
        assert_eq!(stage.row(0), "", "no banner");
    }
}
