//! The way back out: how deep the run has been, and the lift it ends on.
//!
//! Four things that only exist because the run goes back up, and that are
//! meaningless apart. [`Deepest`] is the run's depth memory, which is what
//! the climb's population is drawn at rather than the deck's own band.
//! [`LiftOut`] is the lift on deck one the commando came down on, planted
//! with no `Transition`, so the engine refuses a `GoThrough` on it and
//! leaves the player its turn; [`answer_the_lift_out`] is the game
//! answering that refusal, which is the whole of the run's victory.

use bevy::prelude::*;
use rl_engine::prelude::*;

/// The deepest deck this run has stood on, one at the start.
///
/// The climb's difficulty is drawn at this band rather than at the deck's
/// own, the way NetHack's ascension run takes its monsters from the
/// deepest level reached: a deck one revisited after the core holds what
/// deck ten holds. Only ever raised, so walking back up does not walk it
/// back down.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deepest(pub u32);

impl Default for Deepest {
    fn default() -> Self {
        Self(1)
    }
}

/// Raises [`Deepest`] on every arrival that goes deeper than the run has
/// been. Reads `PlaceEntered` rather than the player's `OnMap`, since the
/// arrival is the event the rest of the deck's setup hangs off too.
pub fn remember_depth(mut entered: MessageReader<PlaceEntered>, mut deepest: ResMut<Deepest>) {
    for ev in entered.read() {
        deepest.0 = deepest.0.max(crate::decks::deck_of(ev.map));
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;

    /// Climbing back up does not make the run shallower: the climb is
    /// drawn at the deepest band, so a memory that fell back to the
    /// current deck would make the way up easier than the way down.
    #[test]
    fn the_run_remembers_its_deepest_deck_and_climbing_back_up_does_not_lower_it() {
        let mut app = crate::testing::headless(RunSeed(3));
        crate::testing::arrive_on(&mut app, 3);
        assert_eq!(app.world().resource::<Deepest>().0, 3);
        crate::testing::arrive_on(&mut app, 1);
        assert_eq!(app.world().resource::<Deepest>().0, 3, "deck one again, but the run has been to three");
    }
}
