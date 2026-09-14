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
//! Where each cursor is stays with it, because the two remember different
//! things: one a cell, the other a cell and what is being aimed. Only what
//! is done to that cell is here.

use bevy::prelude::*;
use rl_core::{Direction, Point, Rect, geometry};

use crate::keys::DirectionKeys;

/// The keys both cursors answer to.
///
/// One resource rather than one per cursor, so a game that moves "next"
/// off Tab moves it everywhere at once.
#[derive(Resource, Debug, Clone)]
pub struct CursorKeys {
    /// Opens and closes the look cursor.
    pub look: KeyCode,
    /// Jumps to the next thing worth pointing at.
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

/// Reads one frame's keys onto a cursor at `at`.
///
/// Close wins over confirm, and both over moving, so a frame with several
/// keys down never acts and moves at once. `candidates` is asked for only
/// when "next" was pressed, since working out what is in sight is the
/// expensive part of a cursor and most frames press nothing.
pub fn steer(
    at: &mut Point,
    input: &ButtonInput<KeyCode>,
    keys: &CursorKeys,
    steps: &DirectionKeys,
    bounds: Rect,
    candidates: impl FnOnce() -> Vec<Point>,
) -> Steer {
    if input.just_pressed(keys.close) {
        return Steer::Close;
    }
    if input.just_pressed(keys.confirm) || input.just_pressed(keys.also_confirm) {
        return Steer::Confirm;
    }
    let to = if input.just_pressed(keys.next) { next_of(&candidates(), *at) } else { steps.just_pressed(input).map(|step| stepped(*at, step, bounds)) };
    match to {
        Some(p) if p != *at => {
            *at = p;
            Steer::Moved
        }
        _ => Steer::Stay,
    }
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

    /// [`steer`], over this frame's keys.
    pub fn steer(&self, at: &mut Point, bounds: Rect, candidates: impl FnOnce() -> Vec<Point>) -> Steer {
        steer(at, &self.input, &self.keys, &self.steps, bounds, candidates)
    }
}

/// `cells` nearest to `origin` first, ties by position, without repeats.
///
/// The tie-break is by position rather than by whatever order the query
/// returned, so cycling twice from one place lands on the same second
/// thing in two runs of one seed.
pub fn ordered(origin: Point, cells: impl IntoIterator<Item = Point>) -> Vec<Point> {
    let mut out: Vec<Point> = cells.into_iter().collect();
    out.sort_by_key(|p| (geometry::chebyshev(origin, *p), p.x, p.y));
    out.dedup();
    out
}

/// The candidate after `at`, wrapping round the end.
///
/// The first when `at` is not one of them, which is what a cursor nudged
/// off a target with the direction keys should snap back to, and `None`
/// when there is nothing to cycle through.
pub fn next_of(cells: &[Point], at: Point) -> Option<Point> {
    match cells.iter().position(|p| *p == at) {
        Some(i) => cells.get((i + 1) % cells.len()).copied(),
        None => cells.first().copied(),
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
    fn candidates_come_back_nearest_first_and_once_each() {
        let origin = Point::new(5, 5);
        let cells = [Point::new(9, 5), Point::new(6, 5), Point::new(9, 5), Point::new(5, 7)];
        assert_eq!(ordered(origin, cells), vec![Point::new(6, 5), Point::new(5, 7), Point::new(9, 5)]);
        // Two at the same distance are ordered by position, so the second
        // press of the cycle key is the same thing in two runs.
        let tied = ordered(origin, [Point::new(5, 3), Point::new(3, 5)]);
        assert_eq!(tied, vec![Point::new(3, 5), Point::new(5, 3)]);
    }

    #[test]
    fn cycling_wraps_and_a_cursor_off_the_list_snaps_to_the_first() {
        let cells = [Point::new(1, 0), Point::new(2, 0), Point::new(3, 0)];
        assert_eq!(next_of(&cells, Point::new(1, 0)), Some(Point::new(2, 0)));
        assert_eq!(next_of(&cells, Point::new(3, 0)), Some(Point::new(1, 0)), "round the end");
        assert_eq!(next_of(&cells, Point::new(9, 9)), Some(Point::new(1, 0)), "nudged off, snapped back");
        assert_eq!(next_of(&[], Point::ZERO), None);
    }

    #[test]
    fn a_step_that_would_leave_the_window_leaves_the_cursor_alone() {
        let bounds = Rect::new(0, 0, 10, 10);
        assert_eq!(stepped(Point::new(5, 5), Direction::East, bounds), Point::new(6, 5));
        assert_eq!(stepped(Point::new(9, 5), Direction::East, bounds), Point::new(9, 5), "held at the edge");
        assert_eq!(stepped(Point::new(9, 5), Direction::NorthEast, bounds), Point::new(9, 5), "not slid along it");
        assert_eq!(stepped(Point::new(0, 0), Direction::NorthWest, bounds), Point::new(0, 0));
    }

    /// One frame with `keys` down, steered from `at`.
    fn frame(at: Point, keys: &[KeyCode], candidates: &[Point]) -> (Point, Steer, bool) {
        let mut input = ButtonInput::<KeyCode>::default();
        for key in keys {
            input.press(*key);
        }
        let mut at = at;
        let mut asked = false;
        let steer = steer(&mut at, &input, &CursorKeys::default(), &DirectionKeys::default(), Rect::new(0, 0, 10, 10), || {
            asked = true;
            candidates.to_vec()
        });
        (at, steer, asked)
    }

    #[test]
    fn closing_outranks_confirming_and_both_outrank_moving() {
        let keys = CursorKeys::default();
        let at = Point::new(5, 5);
        assert_eq!(frame(at, &[keys.close, keys.confirm, KeyCode::ArrowUp], &[]), (at, Steer::Close, false));
        assert_eq!(frame(at, &[keys.also_confirm, keys.next], &[Point::new(1, 1)]), (at, Steer::Confirm, false), "confirming never moves the aim first");
        assert_eq!(frame(at, &[], &[]), (at, Steer::Stay, false), "and nothing pressed is nothing done");
    }

    #[test]
    fn next_asks_for_candidates_and_a_step_does_not() {
        let keys = CursorKeys::default();
        let at = Point::new(5, 5);
        assert_eq!(frame(at, &[keys.next], &[Point::new(2, 2)]), (Point::new(2, 2), Steer::Moved, true));
        assert_eq!(frame(at, &[keys.next], &[]), (at, Steer::Stay, true), "nothing in sight to cycle to");
        assert_eq!(frame(at, &[KeyCode::ArrowLeft], &[Point::new(2, 2)]), (Point::new(4, 5), Steer::Moved, false));
        assert_eq!(frame(Point::new(0, 5), &[KeyCode::ArrowLeft], &[]), (Point::new(0, 5), Steer::Stay, false), "held at the edge is not a move");
    }
}
