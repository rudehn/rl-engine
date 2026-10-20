//! What a mind's decision to step becomes.

use rl_core::{Direction, Point};

/// What a decision to step into `to` is carried out as.
///
/// A mind decides only where it wants to be; whether that is a walk, a
/// door opened or a change of places with someone who gives way is the
/// world's to say, and this is the one place it is turned into an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepAs {
    /// Change places with the one standing there.
    Swap,
    /// Open the shut door there: the door is its own action.
    Open(Direction),
    /// Walk.
    Step(Direction),
    /// Nowhere to go, because `to` is where it already stands.
    Wait,
}

/// How a mind that decided to step from `from` to `to` does it.
///
/// Giving way comes before a door because a swap is the more specific
/// fact about the cell: someone stands there and yields. `gives_way` and
/// `door` are what the game gathered about `to`.
pub fn step_as(from: Point, to: Point, gives_way: bool, door: bool) -> StepAs {
    match Direction::between(from, to) {
        None => StepAs::Wait,
        Some(_) if gives_way => StepAs::Swap,
        Some(d) if door => StepAs::Open(d),
        Some(d) => StepAs::Step(d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HERE: Point = Point::new(3, 3);

    #[test]
    fn a_step_into_one_who_gives_way_is_a_swap_even_onto_a_door() {
        assert_eq!(step_as(HERE, HERE.offset(1, 0), true, false), StepAs::Swap);
        assert_eq!(step_as(HERE, HERE.offset(1, 0), true, true), StepAs::Swap);
    }

    #[test]
    fn a_step_onto_a_shut_door_is_the_turn_spent_opening_it() {
        assert_eq!(step_as(HERE, HERE.offset(0, -1), false, true), StepAs::Open(Direction::between(HERE, HERE.offset(0, -1)).unwrap()));
    }

    #[test]
    fn a_step_onto_an_open_cell_is_a_step_in_that_direction() {
        let to = HERE.offset(-1, 1);
        assert_eq!(step_as(HERE, to, false, false), StepAs::Step(Direction::between(HERE, to).unwrap()));
    }

    #[test]
    fn a_step_to_the_cell_it_already_stands_on_is_a_wait() {
        assert_eq!(step_as(HERE, HERE, false, false), StepAs::Wait);
        assert_eq!(step_as(HERE, HERE, true, true), StepAs::Wait);
    }
}
