//! The integer-clock turn scheduler.
//!
//! [`TurnQueue`] holds every actor waiting to act, keyed by the clock
//! reading it is due at, with insertion order settling ties. It is generic
//! over the actor id so it needs no ECS to be correct, and its whole
//! ordering contract is tested here with ids made out of thin air.
//!
//! The clock is an integer in hundredths of a turn. An `f32` clock cannot
//! promise that the same inputs always give the same order once it has
//! accumulated enough additions to lose low bits, and a scheduler is the
//! last place to relax determinism.
//!
//! The Bevy layer owns the loop that drives this: it decides who is the
//! player, what an action costs, and what happens when the queue is empty.
//! Those are the parts the previous engine left to the game, and the reason
//! the game copied the queue instead of depending on it.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// What one ordinary action costs. A step costs exactly this, so game time
/// counts in hundredths of a turn and fractional costs stay whole numbers.
pub const BASE_ACTION_COST: u32 = 100;

/// When an actor that has just acted is next due.
///
/// Saturating rather than wrapping: a wrapped clock puts an actor in the
/// past and silently reorders everyone else, a saturated one stops visibly.
pub const fn reschedule_at(now: u32, cost: u32) -> u32 {
    now.saturating_add(cost)
}

/// The cost of an action at a given speed, where `speed_percent` of 100 is
/// normal, 200 is twice as fast (half the cost) and 50 is half speed.
///
/// Integer throughout; a speed of 0 is treated as 1 so nothing divides by
/// zero or waits forever.
pub const fn scaled_cost(base: u32, speed_percent: u32) -> u32 {
    let speed = if speed_percent == 0 { 1 } else { speed_percent } as u64;
    let cost = (base as u64 * 100).div_ceil(speed);
    if cost > u32::MAX as u64 { u32::MAX } else { cost as u32 }
}

#[derive(Debug, Clone, Copy)]
struct Entry<Id> {
    time: u32,
    insertion_order: u64,
    id: Id,
}

// Ordered on (time, insertion_order) only. The id is deliberately excluded:
// it carries no scheduling meaning and its bit order is arbitrary.
impl<Id> Ord for Entry<Id> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time).then(self.insertion_order.cmp(&other.insertion_order))
    }
}

impl<Id> PartialOrd for Entry<Id> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Hand-written over exactly the fields `Ord` compares, so `a == b` holds
// exactly when `cmp` says `Equal`, as Rust's ordering contract requires.
impl<Id> PartialEq for Entry<Id> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time && self.insertion_order == other.insertion_order
    }
}

impl<Id> Eq for Entry<Id> {}

/// Everyone waiting to act, and the clock they are waiting on.
#[derive(Debug, Clone)]
pub struct TurnQueue<Id> {
    entries: BinaryHeap<Reverse<Entry<Id>>>,
    next_insertion_order: u64,
    now: u32,
}

impl<Id> Default for TurnQueue<Id> {
    fn default() -> Self {
        Self { entries: BinaryHeap::new(), next_insertion_order: 0, now: 0 }
    }
}

/// Who acts next, from [`TurnQueue::dequeue_batch`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DequeueOutcome<Id> {
    /// The player is next and must be waited on for input.
    Player(Id),
    /// A batch of non-player actors due now, in schedule order.
    Batch(Vec<Id>),
    /// Nobody is due at the current clock reading.
    Idle,
}

impl<Id: Copy + Eq> TurnQueue<Id> {
    /// An empty queue with the clock at zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// What the clock reads.
    pub fn now(&self) -> u32 {
        self.now
    }

    /// Moves the clock forward to `time`.
    ///
    /// # Panics
    /// Panics if `time` is in the past. A clock asked to run backwards is not
    /// a value worth repairing: it can only mean the caller assembled the
    /// loop wrongly.
    pub fn set_now(&mut self, time: u32) {
        assert!(time >= self.now, "the clock cannot run backwards: asked to move from {} to {time}", self.now);
        self.now = time;
    }

    /// Schedules `id` to act once the clock reads `time`.
    pub fn insert_at(&mut self, id: Id, time: u32) {
        let insertion_order = self.next_insertion_order;
        self.next_insertion_order += 1;
        self.entries.push(Reverse(Entry { time, insertion_order, id }));
    }

    /// Schedules `id` to act at the current clock reading.
    pub fn insert_now(&mut self, id: Id) {
        self.insert_at(id, self.now);
    }

    /// Schedules `id` for `cost` units after now.
    pub fn insert_after(&mut self, id: Id, cost: u32) {
        self.insert_at(id, reschedule_at(self.now, cost));
    }

    /// When the next actor is due, without disturbing the queue.
    pub fn peek_time(&self) -> Option<u32> {
        self.entries.peek().map(|Reverse(e)| e.time)
    }

    /// If nobody is due now, moves the clock to whoever is nearest.
    /// Returns whether the clock moved.
    pub fn advance_to_next(&mut self) -> bool {
        match self.peek_time() {
            Some(t) if t > self.now => {
                self.now = t;
                true
            }
            _ => false,
        }
    }

    /// The earliest actor that is both due and still alive.
    ///
    /// `None` while the head is due later than the clock reads: that refusal
    /// is what stops one actor taking a second turn while another waits on
    /// its first. Liveness is asked of the caller, which is what lets the
    /// skip be proved with no world in sight. Dead entries are discarded as
    /// they are met, so the loop cannot spin.
    pub fn pop_due(&mut self, is_alive: impl Fn(Id) -> bool) -> Option<Id> {
        let id = self.peek_due(is_alive)?;
        self.entries.pop();
        Some(id)
    }

    /// The actor [`pop_due`](Self::pop_due) would return, left in place.
    ///
    /// Dead entries ahead of it are discarded, so a `peek_due` followed by
    /// `pop_due` with the same liveness answer pops exactly this actor. A
    /// loop that must stop short of dealing a turn, without changing the
    /// order of anyone tied with the head, looks here first.
    pub fn peek_due(&mut self, is_alive: impl Fn(Id) -> bool) -> Option<Id> {
        loop {
            match self.entries.peek() {
                Some(Reverse(head)) if head.time > self.now => return None,
                Some(Reverse(head)) if is_alive(head.id) => return Some(head.id),
                Some(_) => {
                    self.entries.pop();
                }
                None => return None,
            }
        }
    }

    /// Everyone due now, batched: the player alone, or up to `max_batch`
    /// non-player actors in schedule order.
    ///
    /// Strict schedule order. A batch stops when it reaches the player, and
    /// the player is returned alone on the next call; a player at the head
    /// is returned at once. Dead entries are discarded.
    pub fn dequeue_batch(&mut self, is_player: impl Fn(Id) -> bool, is_alive: impl Fn(Id) -> bool, max_batch: usize) -> DequeueOutcome<Id> {
        let mut batch = Vec::new();
        while let Some(Reverse(head)) = self.entries.peek() {
            if head.time > self.now {
                break;
            }
            if !is_alive(head.id) {
                self.entries.pop();
                continue;
            }
            if is_player(head.id) {
                if !batch.is_empty() {
                    break;
                }
                let Reverse(e) = self.entries.pop().expect("head was just seen");
                return DequeueOutcome::Player(e.id);
            }
            if batch.len() >= max_batch {
                break;
            }
            let Reverse(e) = self.entries.pop().expect("head was just seen");
            batch.push(e.id);
        }
        if batch.is_empty() { DequeueOutcome::Idle } else { DequeueOutcome::Batch(batch) }
    }

    /// Every waiting entry as `(id, time)`, in no particular order. For
    /// saving; the order is the heap's, not the schedule's.
    pub fn entries(&self) -> impl Iterator<Item = (Id, u32)> + '_ {
        self.entries.iter().map(|Reverse(e)| (e.id, e.time))
    }

    /// How many entries are waiting.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nobody is waiting.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Whether `id` is anywhere in the queue. A linear walk; it answers a
    /// yes-or-no question and never decides an order.
    pub fn contains(&self, id: Id) -> bool {
        self.entries.iter().any(|Reverse(e)| e.id == id)
    }

    /// Removes every entry for `id`. Returns how many were removed.
    pub fn remove(&mut self, id: Id) -> usize {
        let before = self.entries.len();
        let kept: Vec<Reverse<Entry<Id>>> = std::mem::take(&mut self.entries).into_iter().filter(|Reverse(e)| e.id != id).collect();
        self.entries = kept.into_iter().collect();
        before - self.entries.len()
    }

    /// Drops every entry but keeps the clock and the insertion counter.
    ///
    /// Leaving a map tears down the actors standing in it, not the hour of
    /// the day.
    pub fn clear_entries(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Q = TurnQueue<u32>;

    #[test]
    fn earliest_first_then_insertion_order() {
        let mut q = Q::new();
        q.insert_at(1, 50);
        q.insert_at(2, 10);
        q.insert_at(3, 10);
        q.set_now(50);
        assert_eq!(q.pop_due(|_| true), Some(2));
        assert_eq!(q.pop_due(|_| true), Some(3));
        assert_eq!(q.pop_due(|_| true), Some(1));
        assert_eq!(q.pop_due(|_| true), None);
    }

    #[test]
    fn nobody_acts_before_they_are_due() {
        let mut q = Q::new();
        q.insert_at(1, 100);
        assert_eq!(q.pop_due(|_| true), None);
        assert_eq!(q.peek_time(), Some(100));
        assert!(q.advance_to_next());
        assert_eq!(q.now(), 100);
        assert!(!q.advance_to_next());
        assert_eq!(q.pop_due(|_| true), Some(1));
    }

    #[test]
    fn dead_entries_are_skipped_without_spinning() {
        let mut q = Q::new();
        q.insert_now(1);
        q.insert_now(2);
        q.insert_now(3);
        assert_eq!(q.pop_due(|id| id == 3), Some(3));
        assert!(q.is_empty());
    }

    #[test]
    fn peeking_discards_the_dead_but_keeps_the_head_and_its_place() {
        let mut q = Q::new();
        q.insert_now(1);
        q.insert_now(2);
        q.insert_now(3);
        assert_eq!(q.peek_due(|id| id != 1), Some(2));
        assert_eq!(q.len(), 2, "the dead head was discarded, the live one kept");
        assert_eq!(q.peek_due(|_| true), Some(2));
        assert_eq!(q.pop_due(|_| true), Some(2), "peeking did not reorder the tie");
        assert_eq!(q.pop_due(|_| true), Some(3));
        q.insert_at(4, 50);
        assert_eq!(q.peek_due(|_| true), None, "due later is not due");
    }

    #[test]
    fn batches_stop_at_the_player_and_keep_schedule_order() {
        let mut q = Q::new();
        q.insert_now(1);
        q.insert_now(7); // the player
        q.insert_now(2);
        let is_player = |id: u32| id == 7;
        assert_eq!(q.dequeue_batch(is_player, |_| true, 64), DequeueOutcome::Batch(vec![1]));
        assert_eq!(q.dequeue_batch(is_player, |_| true, 64), DequeueOutcome::Player(7));
        assert_eq!(q.dequeue_batch(is_player, |_| true, 64), DequeueOutcome::Batch(vec![2]));
        assert_eq!(q.dequeue_batch(is_player, |_| true, 64), DequeueOutcome::Idle);
    }

    #[test]
    fn a_player_at_the_head_is_returned_at_once() {
        let mut q = Q::new();
        q.insert_now(7);
        q.insert_now(1);
        assert_eq!(q.dequeue_batch(|id| id == 7, |_| true, 64), DequeueOutcome::Player(7));
    }

    #[test]
    fn batches_are_capped_and_resume() {
        let mut q = Q::new();
        for id in 1..=5 {
            q.insert_now(id);
        }
        assert_eq!(q.dequeue_batch(|_| false, |_| true, 2), DequeueOutcome::Batch(vec![1, 2]));
        assert_eq!(q.dequeue_batch(|_| false, |_| true, 2), DequeueOutcome::Batch(vec![3, 4]));
        assert_eq!(q.dequeue_batch(|_| false, |_| true, 2), DequeueOutcome::Batch(vec![5]));
    }

    #[test]
    fn a_player_due_later_does_not_block_a_batch() {
        let mut q = Q::new();
        q.insert_at(7, 100);
        q.insert_now(1);
        assert_eq!(q.dequeue_batch(|id| id == 7, |_| true, 64), DequeueOutcome::Batch(vec![1]));
        assert_eq!(q.dequeue_batch(|id| id == 7, |_| true, 64), DequeueOutcome::Idle);
    }

    #[test]
    fn remove_and_contains() {
        let mut q = Q::new();
        q.insert_now(1);
        q.insert_at(1, 40);
        q.insert_now(2);
        assert!(q.contains(1));
        assert_eq!(q.remove(1), 2);
        assert!(!q.contains(1));
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn clearing_keeps_the_clock_and_the_tiebreak_counter() {
        let mut q = Q::new();
        q.insert_now(1);
        q.set_now(300);
        q.clear_entries();
        assert!(q.is_empty());
        assert_eq!(q.now(), 300);
        q.insert_now(2);
        q.insert_now(3);
        assert_eq!(q.pop_due(|_| true), Some(2));
    }

    #[test]
    #[should_panic(expected = "cannot run backwards")]
    fn the_clock_refuses_to_go_backwards() {
        let mut q = Q::new();
        q.set_now(10);
        q.set_now(5);
    }

    #[test]
    fn costs_scale_with_speed() {
        assert_eq!(scaled_cost(100, 100), 100);
        assert_eq!(scaled_cost(100, 200), 50);
        assert_eq!(scaled_cost(100, 50), 200);
        assert_eq!(scaled_cost(100, 150), 67);
        assert_eq!(scaled_cost(100, 0), 10_000);
        assert_eq!(reschedule_at(u32::MAX - 1, 100), u32::MAX);
    }

    #[test]
    fn insert_after_uses_the_clock() {
        let mut q = Q::new();
        q.set_now(250);
        q.insert_after(1, BASE_ACTION_COST);
        assert_eq!(q.peek_time(), Some(350));
    }
}
