//! The targeting overlay: the footprint, painted over the map the frame
//! already drew.
//!
//! Not a framed box like the other panels. An aim is about the map, so
//! this reads back the cells [`draw_map`](rl_render::map_view::draw_map)
//! wrote and repaints their backgrounds, keeping every glyph where it is:
//! the monster under the cursor stays the monster, lit differently.
//!
//! Three colours and no more, because a fourth stops reading at a glance:
//! the flight the projectile takes, the cells it will hit, and the tone
//! for bad news on whatever is out of reach. The flight and the part of
//! it that is out of reach carry a pulse that runs from the user towards
//! the cursor, so the line reads as a direction and not as a wall; the
//! cells hit and the cursor hold still, since a burst that flickered
//! would be hard to count.

use bevy::color::Mix;
use bevy::prelude::*;
use rl_bevy::PresentSet;
use rl_core::{Point, Rect};
use rl_render::{MapView, Terminal};

use crate::panel::clip;
use crate::tone::{Palette, ToneId, Tones};
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

/// Cells per second the pulse travels along the line.
const PULSE_SPEED: f32 = 7.0;
/// Cells between one crest of the pulse and the next.
const PULSE_LENGTH: f32 = 7.0;
/// How far a trough dims a cell towards the surface: slight, so the line
/// is always a line.
const PULSE_DEPTH: f32 = 0.45;

/// How bright the `index`th cell of a line is at time `t`, from 0 at a
/// trough to 1 at a crest, the crests moving up the line as `t` grows.
pub fn pulse(index: usize, t: f32) -> f32 {
    let phase = (t * PULSE_SPEED - index as f32) * std::f32::consts::TAU / PULSE_LENGTH;
    phase.sin() * 0.5 + 0.5
}

/// The colour a cell of a line takes: `tone`, dimmed towards the surface
/// by how far from a crest it is.
pub fn pulsed(tone: ToneId, index: usize, t: f32, palette: &Palette) -> Color {
    palette.get(tone).mix(&palette.get(Tones::SURFACE), PULSE_DEPTH * (1.0 - pulse(index, t)))
}

/// Paints the footprint and the banner.
pub fn draw_target(
    mut terminal: ResMut<Terminal>,
    layout: Res<TargetLayout>,
    view: Res<TargetView>,
    map: Option<Res<MapView>>,
    time: Res<Time>,
    palette: Res<Palette>,
) {
    if !view.aiming() {
        return;
    }
    let Some(map) = map else { return };
    let t = time.elapsed_secs();
    // The flight first, then what is out of reach continuing its count so
    // the pulse runs on through the colour change, then the cells hit, so
    // a cell that is both lands as a hit rather than as the path it
    // arrived by.
    let flight: Vec<Point> = view.path.iter().filter(|p| !view.cells.contains(p)).copied().collect();
    for (i, cell) in flight.iter().enumerate() {
        tint(&mut terminal, &map, *cell, pulsed(Tones::NOTICE, i, t, &palette));
    }
    for (i, cell) in view.beyond.iter().enumerate() {
        tint(&mut terminal, &map, *cell, pulsed(Tones::BAD, flight.len() + i, t, &palette));
    }
    let ground = palette.get(if view.legal { Tones::SELECT } else { Tones::BAD });
    for cell in &view.cells {
        tint(&mut terminal, &map, *cell, ground);
    }
    // The cursor itself last and brightest, since a ball's burst can
    // cover it and a player needs to know where the keys are moving.
    tint(&mut terminal, &map, view.cursor, palette.get(Tones::TITLE));

    let rect = layout.rect;
    if rect.height < 1 || rect.width < 8 {
        return;
    }
    let bg = palette.get(Tones::SURFACE);
    terminal.fill(rect, rl_render::Cell::new(' ', bg).on(bg));
    let name = match (view.throwing, view.firing) {
        (Some(_), _) => format!("throw {}", view.what),
        (None, true) => "fire".to_string(),
        (None, false) => view.what.clone(),
    };
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
        let mut stage = Stage::new_with((AbilitiesPlugin, rl_bevy::ThrowingPlugin, TargetPanel::new(Rect::new(0, 0, 40, 1)).hints("[enter]")), |app| {
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

    /// The colour the `index`th cell of a line was painted on the last
    /// frame, from the same clock the panel read.
    fn lined(stage: &Stage, tone: ToneId, index: usize) -> Color {
        let t = stage.app.world().resource::<Time>().elapsed_secs();
        pulsed(tone, index, t, stage.app.world().resource::<Palette>())
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
        assert_eq!(bg_at(&stage, at.offset(1, 0)), Some(lined(&stage, Tones::NOTICE, 0)), "the flight to it, on its pulse");
        assert_eq!(bg_at(&stage, at.offset(0, 3)), Some(plain), "and nothing else moved");
        assert_eq!(stage.row(0), "bolt at them                    [enter]");
    }

    /// An aim past where the bolt reaches is refused, and the part of the
    /// line the bolt does not reach is painted as out of reach.
    #[test]
    fn an_aim_out_of_reach_is_refused_and_the_rest_of_the_line_is_red() {
        let mut stage = staged();
        let (bolt, _, _) = arm(&mut stage);
        stage.actor("them", 't', 2, 0);
        stage.tick();
        let at = stage.at;
        let user = stage.player;
        stage.app.world_mut().write_message(AimAt { user, ability: bolt });
        stage.tick();
        // Nine cells away, with nothing in between: a bolt of six stops at
        // six.
        stage.app.world_mut().resource_mut::<TargetView>().cursor = at.offset(-9, 0);
        stage.tick();
        let view = stage.app.world().resource::<TargetView>();
        assert!(!view.legal);
        assert_eq!(view.landing, Some(at.offset(-6, 0)));
        assert_eq!(view.beyond, (7..=9).map(|x| at.offset(-x, 0)).collect::<Vec<_>>(), "from past the stop to the cursor");
        assert_eq!(bg_at(&stage, at.offset(-3, 0)), Some(lined(&stage, Tones::NOTICE, 2)), "within reach is the flight");
        // Six cells of flight, the stop among them since nothing lands
        // there, so the first cell past it is the seventh of the line.
        assert_eq!(bg_at(&stage, at.offset(-6, 0)), Some(lined(&stage, Tones::NOTICE, 5)), "the stop is flight, not a hit");
        assert_eq!(bg_at(&stage, at.offset(-7, 0)), Some(lined(&stage, Tones::BAD, 6)), "past it is red, the pulse counting on");
        assert_eq!(bg_at(&stage, at.offset(-9, 0)), Some(stage.app.world().resource::<Palette>().get(Tones::TITLE)), "the cursor, over the refused shape");
        assert_eq!(stage.row(0), "bolt at nothing - out of reach  [enter]");
    }

    /// The pulse runs up the line: a crest at one cell is at the next a
    /// moment later, and it never dims a cell to the surface.
    #[test]
    fn the_pulse_travels_towards_the_cursor_and_never_goes_out() {
        let crest_at = |t: f32| (0..14).max_by(|a, b| pulse(*a, t).total_cmp(&pulse(*b, t))).unwrap();
        let first = crest_at(0.0);
        let later = crest_at(1.0 / PULSE_SPEED);
        assert_eq!(later, first + 1, "one cell further along, one cell-time later");
        for i in 0..14 {
            assert!(pulse(i, 0.37) >= 0.0 && pulse(i, 0.37) <= 1.0);
        }
        let palette = Palette::default();
        assert_ne!(pulsed(Tones::NOTICE, 3, 0.0, &palette), palette.get(Tones::SURFACE), "a trough is still a line");
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
        assert!(view.beyond.is_empty(), "it reaches; it only cannot be paid for");
    }
}
