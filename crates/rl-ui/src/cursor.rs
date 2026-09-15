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

use bevy::prelude::*;
use rl_core::{Direction, Point, Rect};

use crate::focus::{Focus, Sighting, cycle};
use crate::keys::DirectionKeys;

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
pub fn steer(
    at: &mut Point,
    focus: &mut Focus,
    input: &ButtonInput<KeyCode>,
    keys: &CursorKeys,
    steps: &DirectionKeys,
    bounds: Rect,
    candidates: impl FnOnce() -> Vec<Sighting>,
) -> Steer {
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
    let Some(step) = steps.just_pressed(input) else { return Steer::Stay };
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

/// The keys a cursor system reads, borrowed together.
#[derive(bevy::ecs::system::SystemParam)]
pub struct CursorInput<'w> {
    input: Res<'w, ButtonInput<KeyCode>>,
    keys: Res<'w, CursorKeys>,
    steps: Res<'w, DirectionKeys>,
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
        steer(at, focus, &self.input, &self.keys, &self.steps, bounds, candidates)
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
        let steer = steer(&mut at, &mut focus, &input, &CursorKeys::default(), &DirectionKeys::default(), Rect::new(0, 0, 10, 10), || {
            asked = true;
            candidates.to_vec()
        });
        Frame { at, focus, steer, asked }
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
