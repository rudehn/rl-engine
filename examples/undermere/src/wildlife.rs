//! The small life of the place, a moth or a blind cave rat, given way
//! past.
//!
//! Wildlife is neutral to every side, so a bump into it is neither a blow
//! nor a swap: the engine refuses it, as it refuses any bump into
//! somebody who is neither friend nor foe. A moth flitting in a corridor
//! one cell wide then held the adventurer on the near side of it for as
//! long as it stayed there, a thousand turns and more in the soak, with
//! no key that would move it. So every one of them offers [`PAST_VERB`]
//! through the engine's own actor offers, which a bump into it takes up
//! at once, the only thing on offer, and [`slip_past`] changes the two
//! places: a step, as walking on would have been.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::bestiary::Kind;

/// What wildlife offers whoever walks into it, declared with
/// [`AddVerb::add_verb`] by [`crate::plugin::UndermerePlugin`].
pub const PAST_VERB: &str = "slip past";

/// A monster just come into the world that offers nothing yet, and its side.
type Newcomers<'w, 's> = Query<'w, 's, (Entity, &'static Faction), (Added<Kind>, Without<Offers>)>;

/// Puts [`PAST_VERB`] on every monster of the wildlife side as it comes
/// into the world, a floor's own or a summoned one, and on one a save
/// stood up again, since an offer is not the engine's to keep.
///
/// In `TurnSet::React`, before any bump into it can be read: a creature
/// spawned in a pass is standing by the next.
pub fn offer_a_way_past(mut commands: Commands, registries: Res<Registries>, verbs: Res<Verbs>, new: Newcomers) {
    let Some(past) = verbs.get(PAST_VERB) else { return };
    let wildlife = registries.factions.expect(crate::content::WILDLIFE);
    for (who, side) in &new {
        if side.0 == wildlife {
            commands.entity(who).insert(Offers(vec![Offering::new(past, BASE_ACTION_COST)]));
        }
    }
}

/// Answers [`PAST_VERB`]: whoever took it up and the creature change
/// places, both see anew, and the index of who stands where follows them.
/// Nothing when they are not a step apart or the step is a diagonal past a
/// corner, which a step could not have taken either.
///
/// In `TurnSet::React`, the set every game system answering `Interacted`
/// already reads from.
pub fn slip_past(
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    map: Res<WorldMap>,
    mut occupancy: ResMut<Occupancy>,
    mut movers: Query<(&mut Position, &OnMap, Option<&mut Viewshed>)>,
) {
    let Some(past) = verbs.get(PAST_VERB) else { return };
    for ev in done.read().filter(|ev| ev.verb == past) {
        let Ok([(mut mine, on, my_sight), (mut theirs, _, their_sight)]) = movers.get_many_mut([ev.actor, ev.target]) else { continue };
        let (from, to) = (mine.0, theirs.0);
        let step = to - from;
        let squeezed = step.x != 0 && step.y != 0 && !(map.lets_diagonal_past(from.offset(step.x, 0)) && map.lets_diagonal_past(from.offset(0, step.y)));
        if geometry::chebyshev(from, to) != 1 || squeezed {
            continue;
        }
        let here = on.0;
        occupancy.remove_on(here, from, ev.actor);
        occupancy.remove_on(here, to, ev.target);
        occupancy.insert_on(here, to, ev.actor);
        occupancy.insert_on(here, from, ev.target);
        (mine.0, theirs.0) = (to, from);
        for sight in [my_sight, their_sight].into_iter().flatten() {
            sight.into_inner().dirty = true;
        }
    }
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
