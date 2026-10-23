//! The small life of the place, a moth or a blind cave rat, given way
//! past.
//!
//! Wildlife is neutral to every side, so a bump into it is neither a blow
//! nor a swap, and the engine refuses a bump into somebody who is neither
//! friend nor foe. A moth flitting in a corridor one cell wide then held
//! the adventurer on the near side of it for as long as it stayed there, a
//! thousand turns and more in the soak, with no key that would move it. So
//! the wildlife side is one the engine's [`BumpRules`] say gives way: a
//! bump into one changes the two places, a step's time as walking on
//! would have been, and a monster's step into one does the same, so a moth
//! holds up nobody.

use atglyph::prelude::*;

/// How a bump goes in the Undermere: an ally is swapped with, and so is
/// wildlife, by the adventurer and by every mind.
pub fn bump_rules(registries: &Registries) -> BumpRules {
    BumpRules::new().swap_allies().give_way(registries.factions.expect(crate::content::WILDLIFE)).minds_too()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{arena, at, monster, settle, wall_off};

    /// A moth standing in a corridor one cell wide is walked past: the
    /// bump into it changes the two places, where it once held the
    /// adventurer there for good; and it is a step, a turn spent.
    #[test]
    fn a_moth_in_a_corridor_one_cell_wide_is_walked_past_and_not_a_wall() {
        let (mut app, me, here) = arena(0);
        let sides: Vec<Point> = (-1..=4).flat_map(|x| [here.offset(x, -1), here.offset(x, 1)]).collect();
        wall_off(&mut app, &sides);
        let moth = monster(&mut app, "moth", here.offset(1, 0));
        app.world_mut().entity_mut(moth).remove::<Mind>();
        settle(&mut app);
        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(me, Bump(Direction::East)));
        settle(&mut app);
        assert_eq!(at(&app, me), here.offset(1, 0), "past the moth");
        assert_eq!(at(&app, moth), here, "which stands where they stood");
        assert!(app.world().resource::<Turns>().now() > before, "and it took a turn");
        app.world_mut().write_message(Intent::new(me, Bump(Direction::East)));
        settle(&mut app);
        assert_eq!(at(&app, me), here.offset(2, 0), "and on down the corridor");
    }
}
