//! What spending a charge and counting a period come to, with no world.
//!
//! A charge is a count and a clock is a count of hundredths, so what a use
//! leaves behind and how much a refill has earned are arithmetic on
//! integers. They live here so a rule about the last charge of a stack, or a
//! refill that lands exactly on full, is tested on numbers and not by
//! playing turns in an `App`.

use crate::parts::WhenEmpty;

/// What spending one charge leaves a thing as.
///
/// The caller applies it: the rule never touches the stack or the entity, so
/// a game's own container can use the same answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spent {
    /// It holds this many charges still.
    Left(u16),
    /// That was the unit's last charge and the stack has another: one comes
    /// off the stack and the next starts full.
    NextUnit,
    /// That was the last charge of the last unit and the thing is gone.
    Destroyed,
    /// That was the last charge and the thing stays, at nothing, until it
    /// refills.
    Empty,
}

/// What spending one charge from `left` comes to.
///
/// `stack` is the units held, if the thing stacks. A spend at nought is the
/// last charge again rather than an underflow: a thing that spends without
/// being asked whether it can is still resolved the same way.
pub fn spend(left: u16, stack: Option<u32>, when: WhenEmpty) -> Spent {
    let left = left.saturating_sub(1);
    if left > 0 {
        return Spent::Left(left);
    }
    match stack {
        Some(count) if count > 1 => Spent::NextUnit,
        _ if when == WhenEmpty::Destroyed => Spent::Destroyed,
        _ => Spent::Empty,
    }
}

/// How many whole periods of `every` have run once `passed` is added to
/// `progress`, and the progress left over.
///
/// A period of nought counts as one hundredth, so a badly written content
/// file fires every step and does not divide by nothing.
pub fn periods(progress: u32, every: u32, passed: u32) -> (u32, u32) {
    let every = every.max(1);
    let total = progress.saturating_add(passed);
    (total / every, total % every)
}

/// The charges and progress of a refilling thing after `passed` hundredths.
///
/// A full thing holds no progress, so the clock does not bank time towards a
/// charge it has no room for, and one that fills with time to spare starts
/// its next period from nothing for the same reason.
pub fn recharge(left: u16, max: u16, progress: u32, every: u32, passed: u32) -> (u16, u32) {
    if left >= max {
        return (left, 0);
    }
    let (fired, rest) = periods(progress, every, passed);
    let gained = fired.min(u32::from(max - left)) as u16;
    let now = left + gained;
    (now, if now >= max { 0 } else { rest })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_use_takes_one_charge_from_a_thing_that_holds_several() {
        assert_eq!(spend(3, None, WhenEmpty::Kept), Spent::Left(2));
    }

    #[test]
    fn the_last_charge_of_a_stack_takes_one_off_the_stack_and_the_next_unit_starts_full() {
        assert_eq!(spend(1, Some(3), WhenEmpty::Destroyed), Spent::NextUnit);
        assert_eq!(spend(1, Some(2), WhenEmpty::Kept), Spent::NextUnit);
    }

    #[test]
    fn the_last_of_a_unit_alone_is_gone_when_destroyed_and_stays_at_nothing_when_kept() {
        assert_eq!(spend(1, None, WhenEmpty::Destroyed), Spent::Destroyed);
        assert_eq!(spend(1, Some(1), WhenEmpty::Destroyed), Spent::Destroyed);
        assert_eq!(spend(1, None, WhenEmpty::Kept), Spent::Empty);
    }

    #[test]
    fn a_spend_at_nothing_is_the_last_charge_again_and_does_not_underflow() {
        assert_eq!(spend(0, None, WhenEmpty::Kept), Spent::Empty);
        assert_eq!(spend(0, None, WhenEmpty::Destroyed), Spent::Destroyed);
    }

    #[test]
    fn periods_count_whole_periods_and_keep_the_remainder() {
        assert_eq!(periods(0, 800, 700), (0, 700));
        assert_eq!(periods(700, 800, 100), (1, 0));
        assert_eq!(periods(0, 800, 1600), (2, 0));
        assert_eq!(periods(50, 250, 600), (2, 150));
    }

    #[test]
    fn a_period_of_nothing_fires_once_a_hundredth_and_never_divides_by_zero() {
        assert_eq!(periods(0, 0, 3), (3, 0));
    }

    #[test]
    fn a_recharge_returns_a_charge_every_period_on_the_clock_and_keeps_the_remainder() {
        // Three turns of a hundred on a period of 250: one charge and fifty over.
        assert_eq!(recharge(0, 3, 0, 250, 300), (1, 50));
        assert_eq!(recharge(1, 3, 50, 250, 100), (1, 150));
        assert_eq!(recharge(1, 3, 150, 250, 100), (2, 0));
    }

    #[test]
    fn a_recharge_never_passes_the_maximum_and_a_full_thing_banks_no_time() {
        assert_eq!(recharge(2, 3, 0, 100, 900), (3, 0));
        assert_eq!(recharge(3, 3, 40, 100, 900), (3, 0));
        assert_eq!(recharge(2, 3, 0, 100, 150), (3, 0));
        assert_eq!(recharge(2, 3, 0, 400, 300), (2, 300));
    }
}
