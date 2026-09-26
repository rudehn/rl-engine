//! The targeting overlay: the footprint, painted over the map the frame
//! already drew, and a box saying what the aim is worth.
//!
//! The footprint is not framed like the other panels. An aim is about the
//! map, so this reads back the cells
//! [`draw_map`](rl_render::map_view::draw_map) wrote and repaints their
//! backgrounds, keeping every glyph where it is: the monster under the
//! cursor stays the monster, lit differently.
//!
//! The box is where the words go: what is aimed, the range against how far
//! it reaches, the target, and the chance to hit with every line the game's
//! hit model gave for it, or the reason the aim is refused in its place. It
//! is drawn only while the cursor is up, so a game sets it over the bottom
//! of its rail, where the eye already is when choosing what to aim at, and
//! the rows above it stay readable.
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

use crate::cursor::CursorStyle;
use crate::panel::nearby::relation_tone;
use crate::panel::{clear, clip, frame, odds_tone};
use crate::tone::{Palette, ToneId, Tones};
use crate::view::target::{TargetView, TargetViewPlugin};

/// Where the aim is described in words.
#[derive(Resource, Debug, Clone)]
pub struct TargetLayout {
    /// The box the aim is described in. Under three rows tall draws none,
    /// leaving only the footprint.
    pub rect: Rect,
    /// Key hints, in the box's bottom border.
    pub hints: String,
    /// How the cell being aimed at is marked, over the footprint. A glow
    /// in the title tone by default, which is what a cell about to be
    /// struck wants; ticks leave it showing what stands there.
    pub cursor: CursorStyle,
}

/// Draws [`TargetView`]: the footprint on the map, and a box saying what
/// is being aimed at what and the chance it lands.
///
/// Adds [`TargetViewPlugin`] if the game has not.
pub struct TargetPanel(TargetLayout);

impl TargetPanel {
    /// The box in `rect`, eight rows or more to hold a shot's lines. Pass a
    /// zero-height rectangle for the footprint alone.
    pub fn new(rect: Rect) -> Self {
        Self(TargetLayout { rect, hints: String::new(), cursor: CursorStyle::glow(Tones::TITLE) })
    }

    /// Sets the key hints in the bottom border, clipped to it.
    pub fn hints(mut self, hints: impl Into<String>) -> Self {
        self.0.hints = hints.into();
        self
    }

    /// Sets how the cell being aimed at is marked.
    pub fn cursor(mut self, style: CursorStyle) -> Self {
        self.0.cursor = style;
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

/// Paints the footprint and the box.
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
    // cover it and a player needs to know where the keys are moving. The
    // same mark the look cursor uses, so aiming and looking point the
    // same way.
    crate::cursor::mark(&mut terminal, &map, view.cursor, layout.cursor, &palette, t);

    let rect = layout.rect;
    if rect.height < 3 || rect.width < 8 {
        return;
    }
    clear(&mut terminal, rect, &palette);
    frame(&mut terminal, rect, "Targeting", &layout.hints, &palette);
    let bg = palette.get(Tones::SURFACE);
    let inner = rect.inflate(-1);
    let width = inner.width as usize;
    let mut y = inner.y;
    let mut line = |terminal: &mut Terminal, text: &str, tone: ToneId| {
        if y < inner.bottom() {
            terminal.print_on(inner.x, y, &clip(text, width), palette.get(tone), bg);
            y += 1;
        }
    };
    let name = match (view.throwing, view.firing, view.what.is_empty()) {
        (Some(_), _, _) => format!("throw {}", view.what),
        (None, true, true) => "fire".to_string(),
        (None, true, false) => format!("fire {}", view.what),
        (None, false, _) => view.what.clone(),
    };
    line(&mut terminal, &name, Tones::TITLE);
    if let Some(span) = view.span {
        line(&mut terminal, &format!("Range: {} / {}", span.distance, span.max), Tones::TEXT);
    }
    // The target's name in the tone of what it is to the aimer, the way the
    // nearby list colours it, so a friend under the cursor reads as one.
    let (at, at_tone) = match view.targets.as_slice() {
        [] => ("nothing".to_string(), Tones::MUTED),
        [one] if !one.label.is_empty() => (one.label.clone(), relation_tone(one.relation)),
        [_] => ("one of them".to_string(), Tones::TEXT),
        many => (format!("{} of them", many.len()), Tones::TEXT),
    };
    if y < inner.bottom() {
        // Both halves clipped to the box, so a narrow one keeps its border.
        let prefix = clip("Target: ", width);
        terminal.print_on(inner.x, y, &prefix, palette.get(Tones::TEXT), bg);
        let used = prefix.chars().count();
        if used < width {
            terminal.print_on(inner.x + used as i32, y, &clip(&at, width - used), palette.get(at_tone), bg);
        }
        y += 1;
    }
    // Red says no, and says why, where the chance would be: a cursor that
    // refuses without saying what is wrong is a cursor the player argues
    // with, and a refused aim has no chance worth printing.
    let mut line = |terminal: &mut Terminal, text: &str, tone: ToneId| {
        if y < inner.bottom() {
            terminal.print_on(inner.x, y, &clip(text, width), palette.get(tone), bg);
            y += 1;
        }
    };
    match (view.why.first(), &view.odds) {
        (Some(reason), _) => line(&mut terminal, crate::view::ability::plain(reason), Tones::BAD),
        (None, Some(odds)) => {
            let percent = odds.percent();
            line(&mut terminal, &format!("Chance to hit: {percent}%"), odds_tone(percent));
            for reason in &odds.lines {
                line(&mut terminal, &format!("  {:+} {}", reason.value, reason.label), Tones::MUTED);
            }
        }
        (None, None) => {}
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
    use crate::view::target::harness::{abilities, arm};
    use crate::view::target::{AimAt, AimFire};
    use rl_bevy::{Abilities, AbilitiesPlugin, AddEngineEffects};

    /// A stage with the map drawn under the overlay, so the test sees the
    /// same cells a player would.
    fn staged() -> Stage {
        let mut stage = Stage::new_with((AbilitiesPlugin, rl_bevy::ThrowingPlugin, TargetPanel::new(Rect::new(0, 0, 26, 10)).hints("[tab] next")), |app| {
            app.add_engine_effects();
            abilities(app);
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 10, 40, 20)));
        });
        stage.tick();
        stage
    }

    /// The box's inner rows, the frame stripped and trailing spaces trimmed.
    fn boxed(stage: &Stage, height: i32) -> Vec<String> {
        (1..height - 1).map(|y| stage.row(y).chars().skip(1).take(24).collect::<String>().trim_end().to_string()).collect()
    }

    /// A stage with the box, a shooter whose gun reaches twelve and three of
    /// them without penalty, and the game's odds a percent roll.
    fn shooter(height: i32) -> Stage {
        let mut stage = Stage::new_with(TargetPanel::new(Rect::new(0, 0, 26, height)), |app| {
            app.insert_resource(rl_bevy::HitRules(Box::new(rl_rules::Percent::new(5, 16, 30))));
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 12, 40, 20)));
        });
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(rl_bevy::RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12).effective_to(3));
        stage.actor("droid", 'd', 5, 0);
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
    /// the box says what is being aimed at what.
    #[test]
    fn the_footprint_is_tinted_over_the_map_and_the_box_names_it() {
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
        assert_eq!(boxed(&stage, 10)[..4], ["bolt", "Range: 2 / 6", "Target: them", ""], "an ability is never rolled, so no chance");
        assert!(stage.row(9).contains("[tab] next"), "the hints are in the bottom border: {:?}", stage.row(9));
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
        assert_eq!(boxed(&stage, 10)[..4], ["bolt", "Range: 9 / 6", "Target: nothing", "out of reach"]);
    }

    #[test]
    fn a_shot_shows_its_chance_and_every_line_behind_it() {
        let mut stage = shooter(10);
        let user = stage.player;
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        assert_eq!(boxed(&stage, 10)[..5], ["fire", "Range: 5 / 12", "Target: droid", "Chance to hit: 90%", "  -10 for range"]);
        let palette = stage.app.world().resource::<Palette>().clone();
        assert_eq!(stage.app.world().resource::<Terminal>().get(1, 4).map(|c| c.fg), Some(palette.get(Tones::GOOD)), "ninety is a good bet");
        assert_eq!(stage.app.world().resource::<Terminal>().get(9, 3).map(|c| c.fg), Some(palette.get(Tones::BAD)), "a foe's name in the hostile tone");
    }

    #[test]
    fn more_lines_than_rows_are_clipped_inside_the_frame() {
        // Six rows: the frame and four inside, which the name, the range,
        // the target and the chance fill, so the range line is cut.
        let mut stage = shooter(6);
        let user = stage.player;
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        assert_eq!(boxed(&stage, 6), ["fire", "Range: 5 / 12", "Target: droid", "Chance to hit: 90%"]);
        assert!(stage.row(5).starts_with('\u{2514}'), "the bottom border is where it belongs: {:?}", stage.row(5));
        let below: String = stage.row(6).chars().take(26).collect();
        assert!(!below.contains("range") && !below.contains('\u{2502}'), "and nothing spilled below it: {below:?}");
    }

    #[test]
    fn a_box_too_narrow_for_the_target_line_clips_it_inside_the_frame() {
        let mut stage = Stage::new_with(TargetPanel::new(Rect::new(0, 0, 9, 8)), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 12, 40, 20)));
        });
        let (user, kind) = (stage.player, stage.kind);
        stage.app.world_mut().entity_mut(user).insert(rl_bevy::RangedAttack::new(kind, rl_core::DiceRoll::flat(2), 12));
        stage.actor("droid", 'd', 5, 0);
        stage.tick();
        stage.app.world_mut().write_message(AimFire { user });
        stage.tick();
        let line = |y: i32| stage.row(y).chars().take(10).collect::<String>();
        let target = (1..7).find(|y| line(*y).contains("Targ")).expect("a target line");
        assert_eq!(line(target).chars().nth(8), Some('\u{2502}'), "the right border is left standing: {:?}", line(target));
        assert!(line(target).chars().nth(9).is_none_or(|c| c == ' '), "and nothing is drawn past it: {:?}", line(target));
    }

    #[test]
    fn with_the_cursor_down_the_box_draws_nothing() {
        let stage = shooter(10);
        assert!(stage.rows()[..10].iter().all(|r| !r.contains("Targeting")), "{:?}", &stage.rows()[..10]);
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
        assert_eq!(boxed(&stage, 10)[..4], ["dear", "Range: 2 / 6", "Target: them", "cannot pay"]);
        // The landing cell is the one the footprint covers, and an
        // illegal aim paints it in the bad tone rather than the select.
        let view = stage.app.world().resource::<TargetView>();
        assert!(!view.legal);
        assert!(view.beyond.is_empty(), "it reaches; it only cannot be paid for");
    }
}
