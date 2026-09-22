//! What the two cursors share: their keys, and how a key moves them.
//!
//! The look cursor and the targeting cursor open onto the nearest thing
//! worth pointing at, step with the direction keys, cycle through what is
//! in sight, stay inside the loaded window, and close on the same key. That
//! is one behaviour, so it is written once: [`CursorKeys`] is the one set of
//! bindings both answer to, and [`steer`] is the one reading of a frame's
//! keys, so a player who learns one cursor has learned the other and a fix
//! to either is a fix to both.
//!
//! Cycling steps through [`Sighting`]s, entities rather than cells, in the
//! order the nearby list prints them, and moves the [`Focus`] with it; a
//! step moves the focus onto whatever the list has on the new cell. So the
//! row the nearby panel highlights is always what the cursor is on, and
//! closing a cursor leaves picked out what it was last pointed at.
//!
//! Where each cursor is stays with it, because the two remember different
//! things: one a cell, the other a cell and what is being aimed. Only what
//! is done to that cell is here.
//!
//! How a cell is marked is here too, as [`CursorStyle`]: every screen that
//! points at a cell points at it the same two ways, so a game picks one
//! and picks it everywhere. [`mark`] is the drawing, one place, so the two
//! styles cannot drift apart.

use bevy::color::Mix;
use bevy::prelude::*;
use rl_core::{Direction, Point, Rect};
use rl_render::{Cell, Terminal};

use crate::controls::Repeats;
use crate::focus::{Focus, Sighting, cycle};
use crate::keys::DirectionKeys;
use crate::tone::{Palette, ToneId, Tones};

/// Seconds one pulse takes, whichever style is pulsing.
const PULSE_SECS: f32 = 0.9;

/// How bright a pulse is at time `t`, from 0 to 1 and back, so a cursor
/// breathes rather than blinks.
pub fn pulse(t: f32) -> f32 {
    (t * std::f32::consts::TAU / PULSE_SECS).sin() * 0.5 + 0.5
}

/// How a screen marks the cell it is pointing at.
///
/// Two ways, because a cell can be pointed at from outside it or filled
/// in, and which reads better depends on what is on the cell: a glow says
/// "this one" at a glance and is what a list's highlight wants; ticks
/// leave the cell's own glyph showing, which is what a cursor over
/// something worth reading wants.
///
/// Both take a [`ToneId`] rather than a colour, so a game that repaints
/// its palette repaints its cursors, and both may pulse toward a second
/// tone rather than sitting still.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorStyle {
    /// The cell itself glows: its background is washed with `tone`,
    /// leaving whatever stands there drawn over it.
    Glow {
        /// What it glows.
        tone: ToneId,
        /// What it breathes toward, when it breathes at all.
        pulse: Option<ToneId>,
    },
    /// Four marks on the cells around it, framing the cell and leaving
    /// the cell itself exactly as the map drew it.
    Ticks {
        /// Left, right, above, below.
        marks: [char; 4],
        /// What they are drawn in.
        tone: ToneId,
        /// What they breathe toward, when they breathe at all.
        pulse: Option<ToneId>,
    },
}

impl CursorStyle {
    /// A cell that glows `tone` and sits still.
    pub fn glow(tone: ToneId) -> Self {
        Self::Glow { tone, pulse: None }
    }

    /// The engine's four ASCII ticks, breathing between the select tone
    /// and the title tone: a dash either side, a bar above and below.
    ///
    /// ASCII because a browser build draws with whatever font it is
    /// given, and the box-drawing characters are the first to go missing.
    pub fn ticks() -> Self {
        Self::Ticks { marks: ['-', '-', '|', '|'], tone: Tones::SELECT, pulse: Some(Tones::TITLE) }
    }

    /// The same, in another tone.
    pub fn in_tone(self, tone: ToneId) -> Self {
        match self {
            Self::Glow { pulse, .. } => Self::Glow { tone, pulse },
            Self::Ticks { marks, pulse, .. } => Self::Ticks { marks, tone, pulse },
        }
    }

    /// The same, breathing toward `tone`, or sitting still with `None`.
    pub fn breathing(self, tone: Option<ToneId>) -> Self {
        match self {
            Self::Glow { tone: base, .. } => Self::Glow { tone: base, pulse: tone },
            Self::Ticks { marks, tone: base, .. } => Self::Ticks { marks, tone: base, pulse: tone },
        }
    }

    /// The colour it is drawn in at `t` seconds.
    fn color(&self, palette: &Palette, t: f32) -> Color {
        let (tone, breath) = match *self {
            Self::Glow { tone, pulse } | Self::Ticks { tone, pulse, .. } => (tone, pulse),
        };
        match breath {
            Some(other) => palette.get(tone).mix(&palette.get(other), pulse(t)),
            None => palette.get(tone),
        }
    }
}

/// Marks `cell` on the map in `style`, at `t` seconds for whatever is
/// pulsing.
///
/// The one drawing of a cursor, so a screen that points at a cell points
/// at it the way every other screen does.
pub fn mark(terminal: &mut Terminal, view: &rl_render::MapView, cell: Point, style: CursorStyle, palette: &Palette, t: f32) {
    let color = style.color(palette, t);
    match style {
        CursorStyle::Glow { .. } => {
            let Some(screen) = view.to_screen(cell) else { return };
            let Some(mut drawn) = terminal.get(screen.x, screen.y) else { return };
            drawn.bg = color;
            terminal.set(screen.x, screen.y, drawn);
        }
        CursorStyle::Ticks { marks, .. } => {
            // The cell itself is left alone: a cursor that covered it would
            // hide what it points at.
            let around = [(-1, 0), (1, 0), (0, -1), (0, 1)];
            for ((dx, dy), glyph) in around.into_iter().zip(marks) {
                let Some(screen) = view.to_screen(cell.offset(dx, dy)) else { continue };
                let under = terminal.get(screen.x, screen.y).unwrap_or_default();
                terminal.set(screen.x, screen.y, Cell::new(glyph, color).on(under.bg));
            }
        }
    }
}

/// The keys both cursors answer to.
///
/// One resource rather than one per cursor, so a game that moves "next"
/// off Tab moves it everywhere at once, the nearby list included.
#[derive(Resource, Debug, Clone)]
pub struct CursorKeys {
    /// Opens and closes the look cursor.
    pub look: KeyCode,
    /// Jumps to the next thing worth pointing at; with Shift held, to the
    /// one before.
    pub next: KeyCode,
    /// Puts a cursor away, spending nothing.
    pub close: KeyCode,
    /// Acts on the cursor: spends the turn on an aim.
    pub confirm: KeyCode,
    /// And so does this, because a player reaching for one reaches for
    /// the other.
    pub also_confirm: KeyCode,
}

impl Default for CursorKeys {
    fn default() -> Self {
        Self { look: KeyCode::KeyX, next: KeyCode::Tab, close: KeyCode::Escape, confirm: KeyCode::Enter, also_confirm: KeyCode::Space }
    }
}

/// What a frame's keys did to a cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Steer {
    /// Nothing: no key it answers to, or a move with nowhere to go.
    Stay,
    /// It moved, by a step or to the next candidate.
    Moved,
    /// Put it away.
    Close,
    /// Act on where it is.
    Confirm,
}

/// Whether Shift is held, which turns "next" into "previous".
pub fn shifted(input: &ButtonInput<KeyCode>) -> bool {
    input.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
}

/// Reads one frame's keys onto a cursor at `at`, moving `focus` with it.
///
/// Close wins over confirm, and both over moving, so a frame with several
/// keys down never acts and moves at once. `candidates` is asked for only
/// when the cursor moves, since working out what is in sight is the
/// expensive part of a cursor and most frames press nothing.
///
/// "Next" steps from the focus when the cursor sits on it, and from the
/// start of the list otherwise, so a cursor nudged off its target snaps
/// back to the first. Two things on one tile are two stops, and landing on
/// the second counts as a move although the cell is the same.
///
/// A direction key held down steps again at the pace `repeats` says, the
/// same pace a held key walks at.
pub fn steer(at: &mut Point, focus: &mut Focus, keyed: &Keyed<'_>, bounds: Rect, candidates: impl FnOnce() -> Vec<Sighting>) -> Steer {
    let Keyed { input, keys, steps, repeats } = *keyed;
    if input.just_pressed(keys.close) {
        return Steer::Close;
    }
    if input.just_pressed(keys.confirm) || input.just_pressed(keys.also_confirm) {
        return Steer::Confirm;
    }
    if input.just_pressed(keys.next) {
        let list = candidates();
        let from = focus.within(&list).filter(|s| s.at == *at).map(|s| s.entity);
        let Some(next) = cycle(&list, from, shifted(input)).copied() else { return Steer::Stay };
        let moved = next.at != *at || from != Some(next.entity);
        *at = next.at;
        focus.set(Some(next.entity));
        return if moved { Steer::Moved } else { Steer::Stay };
    }
    let Some(step) = steps.just_pressed(input).or_else(|| repeats.firing(false)) else { return Steer::Stay };
    let to = stepped(*at, step, bounds);
    if to == *at {
        return Steer::Stay;
    }
    *at = to;
    // Onto whatever the list has there, and off it on bare ground, so the
    // highlighted row never names something the cursor has left.
    focus.set(candidates().iter().find(|s| s.at == to).map(|s| s.entity));
    Steer::Moved
}

/// One frame's keys and the bindings they are read against, borrowed
/// together for [`steer`].
#[derive(Clone, Copy)]
pub struct Keyed<'a> {
    /// What is down.
    pub input: &'a ButtonInput<KeyCode>,
    /// The cursor's own keys.
    pub keys: &'a CursorKeys,
    /// The direction keys.
    pub steps: &'a DirectionKeys,
    /// Which direction a held key repeats this frame.
    pub repeats: &'a Repeats,
}

/// The keys a cursor system reads, borrowed together.
#[derive(bevy::ecs::system::SystemParam)]
pub struct CursorInput<'w> {
    input: Res<'w, ButtonInput<KeyCode>>,
    keys: Res<'w, CursorKeys>,
    steps: Res<'w, DirectionKeys>,
    repeats: Res<'w, Repeats>,
}

impl CursorInput<'_> {
    /// The bindings.
    pub fn keys(&self) -> &CursorKeys {
        &self.keys
    }

    /// Whether `key` went down this frame.
    pub fn just_pressed(&self, key: KeyCode) -> bool {
        self.input.just_pressed(key)
    }

    /// Whether Shift is held, which turns "next" into "previous".
    pub fn back(&self) -> bool {
        shifted(&self.input)
    }

    /// [`steer`], over this frame's keys.
    pub fn steer(&self, at: &mut Point, focus: &mut Focus, bounds: Rect, candidates: impl FnOnce() -> Vec<Sighting>) -> Steer {
        steer(at, focus, &Keyed { input: &self.input, keys: &self.keys, steps: &self.steps, repeats: &self.repeats }, bounds, candidates)
    }
}

/// `at` moved one step, or left where it was when that leaves `bounds`.
///
/// Stopping rather than clamping: a cursor at the edge that slides along
/// it when pushed outwards reads as the cursor moving on its own.
pub fn stepped(at: Point, step: Direction, bounds: Rect) -> Point {
    let moved = at + step.offset();
    if bounds.contains(moved) { moved } else { at }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_that_would_leave_the_window_leaves_the_cursor_alone() {
        let bounds = Rect::new(0, 0, 10, 10);
        assert_eq!(stepped(Point::new(5, 5), Direction::East, bounds), Point::new(6, 5));
        assert_eq!(stepped(Point::new(9, 5), Direction::East, bounds), Point::new(9, 5), "held at the edge");
        assert_eq!(stepped(Point::new(9, 5), Direction::NorthEast, bounds), Point::new(9, 5), "not slid along it");
        assert_eq!(stepped(Point::new(0, 0), Direction::NorthWest, bounds), Point::new(0, 0));
    }

    /// Sightings at each of `cells`, on distinct entities.
    fn sightings(cells: &[Point]) -> Vec<Sighting> {
        let mut world = World::new();
        cells.iter().map(|at| Sighting { entity: world.spawn_empty().id(), at: *at, actor: true, distance: 0 }).collect()
    }

    /// What one frame did to a cursor.
    struct Frame {
        at: Point,
        focus: Focus,
        steer: Steer,
        asked: bool,
    }

    /// One frame with `keys` down, steered from `at` with `focus`.
    fn frame(at: Point, focus: Focus, keys: &[KeyCode], candidates: &[Sighting]) -> Frame {
        let mut input = ButtonInput::<KeyCode>::default();
        for key in keys {
            input.press(*key);
        }
        let (mut at, mut focus, mut asked) = (at, focus, false);
        let keyed = Keyed { input: &input, keys: &CursorKeys::default(), steps: &DirectionKeys::default(), repeats: &Repeats::default() };
        let steer = steer(&mut at, &mut focus, &keyed, Rect::new(0, 0, 10, 10), || {
            asked = true;
            candidates.to_vec()
        });
        Frame { at, focus, steer, asked }
    }

    /// A direction key held down keeps the cursor stepping, at the pace a
    /// held key walks.
    #[test]
    fn a_held_direction_key_steps_the_cursor_again() {
        let input = ButtonInput::<KeyCode>::default();
        let mut repeats = Repeats::default();
        let pace = crate::controls::RepeatPace { delay: 0.1, every: 0.1 };
        repeats.advance(Some((Direction::East, false)), false, &pace, 0.2);
        let (mut at, mut focus) = (Point::new(5, 5), Focus::default());
        let keyed = Keyed { input: &input, keys: &CursorKeys::default(), steps: &DirectionKeys::default(), repeats: &repeats };
        let steer = steer(&mut at, &mut focus, &keyed, Rect::new(0, 0, 10, 10), Vec::new);
        assert_eq!((at, steer), (Point::new(6, 5), Steer::Moved));
    }

    #[test]
    fn closing_outranks_confirming_and_both_outrank_moving() {
        let keys = CursorKeys::default();
        let at = Point::new(5, 5);
        let some = sightings(&[Point::new(1, 1)]);
        let closed = frame(at, Focus::default(), &[keys.close, keys.confirm, KeyCode::ArrowUp], &some);
        assert_eq!((closed.at, closed.steer, closed.asked), (at, Steer::Close, false));
        let confirmed = frame(at, Focus::default(), &[keys.also_confirm, keys.next], &some);
        assert_eq!((confirmed.at, confirmed.steer, confirmed.asked), (at, Steer::Confirm, false), "confirming never moves the aim first");
        let idle = frame(at, Focus::default(), &[], &some);
        assert_eq!((idle.steer, idle.asked), (Steer::Stay, false), "and nothing pressed is nothing done");
    }

    #[test]
    fn next_steps_through_the_list_picks_out_what_it_lands_on_and_shift_goes_back() {
        let next = CursorKeys::default().next;
        let list = sightings(&[Point::new(2, 2), Point::new(3, 3), Point::new(4, 4)]);
        let first = frame(Point::new(5, 5), Focus::default(), &[next], &list);
        assert_eq!((first.at, first.focus.get(), first.steer, first.asked), (list[0].at, Some(list[0].entity), Steer::Moved, true));
        let second = frame(first.at, first.focus, &[next], &list);
        assert_eq!((second.at, second.focus.get()), (list[1].at, Some(list[1].entity)));
        let back = frame(first.at, first.focus, &[KeyCode::ShiftLeft, next], &list);
        assert_eq!(back.focus.get(), Some(list[2].entity), "Shift steps back, round the start");
        let none = frame(Point::new(5, 5), Focus::default(), &[next], &[]);
        assert_eq!((none.at, none.steer), (Point::new(5, 5), Steer::Stay), "nothing in sight to cycle to");
    }

    #[test]
    fn two_things_on_one_tile_are_two_stops() {
        let next = CursorKeys::default().next;
        let tile = Point::new(3, 3);
        let list = sightings(&[tile, tile]);
        let mut focus = Focus::default();
        focus.set(Some(list[0].entity));
        let on = frame(tile, focus, &[next], &list);
        assert_eq!((on.at, on.focus.get(), on.steer), (tile, Some(list[1].entity), Steer::Moved), "the same cell, the other thing");
    }

    #[test]
    fn a_step_moves_the_focus_onto_what_is_there_and_off_it_on_bare_ground() {
        let list = sightings(&[Point::new(6, 5)]);
        let onto = frame(Point::new(5, 5), Focus::default(), &[KeyCode::ArrowRight], &list);
        assert_eq!((onto.at, onto.focus.get(), onto.steer), (Point::new(6, 5), Some(list[0].entity), Steer::Moved));
        let off = frame(onto.at, onto.focus, &[KeyCode::ArrowRight], &list);
        assert_eq!((off.at, off.focus.get()), (Point::new(7, 5), None), "bare ground picks out nothing");
        let held = frame(Point::new(9, 5), Focus::default(), &[KeyCode::ArrowRight], &list);
        assert_eq!((held.steer, held.asked), (Steer::Stay, false), "held at the edge is not a move, and asks for nothing");
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;
    use crate::harness::Stage;
    use crate::panel::NearbyPanel;
    use rl_core::Rect;

    /// What each style does to the map: a glow washes the cell and leaves
    /// what stands there, ticks leave the cell alone and mark around it.
    #[test]
    fn a_glow_washes_the_cell_and_ticks_frame_it_without_covering_it() {
        let mut stage = Stage::new_with(NearbyPanel::new(Rect::new(40, 0, 20, 20)), |app| {
            app.add_plugins(rl_render::MapViewPlugin::new(Rect::new(0, 0, 40, 20)));
        })
        .screen(60, 24);
        stage.actor("crab", 'c', 2, 0);
        stage.tick();
        let at = stage.at.offset(2, 0);
        let (glow, ticks) = (CursorStyle::glow(Tones::SELECT), CursorStyle::ticks());

        let (palette, view) = {
            let world = stage.app.world();
            (world.resource::<Palette>().clone(), *world.resource::<rl_render::MapView>())
        };
        let screen = view.to_screen(at).expect("the crab is on screen");
        let left = view.to_screen(at.offset(-1, 0)).expect("and so is the cell beside it");

        let mut terminal = stage.app.world_mut().resource_mut::<Terminal>();
        let before = terminal.get(screen.x, screen.y).expect("a cell");
        mark(&mut terminal, &view, at, glow, &palette, 0.0);
        let after = terminal.get(screen.x, screen.y).expect("a cell");
        assert_eq!(after.glyph, before.glyph, "a glow leaves what stands there showing");
        assert_eq!(after.bg, palette.get(Tones::SELECT), "and washes the cell behind it");

        mark(&mut terminal, &view, at, ticks, &palette, 0.0);
        assert_eq!(terminal.get(screen.x, screen.y).map(|c| c.glyph), Some(before.glyph), "ticks never cover the cell");
        assert_eq!(terminal.get(left.x, left.y).map(|c| c.glyph), Some('-'), "they mark around it");
    }

    /// Pulsing is a second tone to breathe toward, and no pulse is the
    /// tone itself, whatever the clock reads.
    #[test]
    fn a_style_breathes_only_when_it_was_given_something_to_breathe_toward() {
        let mut stage = Stage::new(NearbyPanel::new(Rect::new(40, 0, 20, 20)));
        stage.tick();
        let palette = stage.app.world().resource::<Palette>().clone();
        let still = CursorStyle::glow(Tones::SELECT);
        assert_eq!(still.color(&palette, 0.0), still.color(&palette, 0.45), "a still cursor is the same at any moment");
        assert_eq!(still.color(&palette, 0.0), palette.get(Tones::SELECT));

        let breathing = still.breathing(Some(Tones::TITLE));
        assert_ne!(breathing.color(&palette, 0.0), breathing.color(&palette, PULSE_SECS / 4.0), "a breathing one is not");
        assert_eq!(
            CursorStyle::ticks().in_tone(Tones::BAD).color(&palette, 0.0),
            CursorStyle::glow(Tones::BAD).breathing(Some(Tones::TITLE)).color(&palette, 0.0)
        );
    }
}
