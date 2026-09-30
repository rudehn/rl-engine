//! Work: one thing an actor does across many turns.
//!
//! [`Work`] is how far along it is, counted in the worker's own turns and
//! never in clock time, so a worker twice as fast finishes in half the
//! clock without this module knowing speed exists. [`Work::advance`]
//! counts a turn and answers with [`Progress`]. [`in_reach`] is the one
//! test of whether work can still be done: a worker within [`REACH`] of
//! its target, asked of where both are now rather than how they got
//! there. What a kind of work is called is a [`WorkKindId`], interned from
//! the word a panel shows, typed by [`WorkKind`], so there is no list of
//! kinds to add to.

use rl_core::{Id, Point, geometry};

/// What a kind of work is called, as an interned name. Never constructed;
/// it only types [`WorkKindId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkKind {}

/// A kind of work, interned from the word a panel shows for it.
pub type WorkKindId = Id<WorkKind>;

/// How far from its target work can be done, in cells, Chebyshev: beside
/// it or on it. Nothing asks for work from further off.
pub const REACH: i32 = 1;

/// Whether a worker at `worker` can work on a target at `target`.
pub fn in_reach(worker: Point, target: Point) -> bool {
    geometry::chebyshev(worker, target) <= REACH
}

/// Where a piece of work stands after a turn of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Progress {
    /// Not done; this many of the worker's turns are left.
    Left(u16),
    /// That was the last turn it needed.
    Finished,
}

/// One thing an actor is doing across many turns, and how far along it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Work<A> {
    /// What kind of work, which is also what a panel calls it.
    pub kind: WorkKindId,
    /// What it is done to, if anything: work on a target breaks when the
    /// target is gone or out of [`REACH`].
    pub target: Option<A>,
    /// Turns worked so far.
    pub done: u16,
    /// Turns it takes, never fewer than one.
    pub needed: u16,
}

impl<A> Work<A> {
    /// Work of `kind` taking `needed` of the worker's turns, on nothing.
    /// Zero is taken as one, since work that takes no turns is not work.
    pub fn new(kind: WorkKindId, needed: u16) -> Self {
        Self { kind, target: None, done: 0, needed: needed.max(1) }
    }

    /// The same work, done to `target`.
    pub fn on(mut self, target: A) -> Self {
        self.target = Some(target);
        self
    }

    /// Turns still to work.
    pub fn left(&self) -> u16 {
        self.needed - self.done
    }

    /// Counts one turn of work.
    pub fn advance(&mut self) -> Progress {
        self.done = (self.done + 1).min(self.needed);
        if self.done == self.needed { Progress::Finished } else { Progress::Left(self.left()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_finishes_on_exactly_the_turn_it_needs_and_not_before() {
        for needed in 1..=40u16 {
            let mut work: Work<()> = Work::new(WorkKindId::from_raw(0), needed);
            for turn in 1..needed {
                assert_eq!(work.advance(), Progress::Left(needed - turn), "turn {turn} of {needed}");
            }
            assert_eq!(work.advance(), Progress::Finished, "turn {needed} of {needed}");
            assert_eq!(work.left(), 0);
        }
    }

    #[test]
    fn work_that_asks_for_no_turns_takes_one() {
        let mut work: Work<()> = Work::new(WorkKindId::from_raw(0), 0);
        assert_eq!(work.needed, 1);
        assert_eq!(work.advance(), Progress::Finished);
    }

    #[test]
    fn a_target_is_in_reach_on_it_or_beside_it_and_not_a_cell_further() {
        let c = Point::new(5, 5);
        for dx in -1..=1 {
            for dy in -1..=1 {
                assert!(in_reach(c, c.offset(dx, dy)), "({dx}, {dy})");
            }
        }
        assert!(!in_reach(c, c.offset(2, 0)));
        assert!(!in_reach(c, c.offset(2, 2)));
    }
}
