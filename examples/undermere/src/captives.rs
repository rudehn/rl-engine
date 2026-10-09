//! A shackled monster of the floor, freed to follow.
//!
//! [`population::place_captive`](crate::population::place_captive) stands
//! one a run, neutral and anchored by a one-waypoint `PatrolRoute`, the
//! universal brain's own `Patrol`, offering [`FREE_VERB`]
//! through the engine's own actor offers (task 4): a long action of
//! [`FREE_TIME`], one of a few turns, not interrupted once begun, because
//! the engine's multi-turn work skips the player and a single long-costed
//! offer is what is left for the player to take up at all. [`answer_free`]
//! reads the `Interacted` it lands and [`free`] does what it means: put
//! the captive on the adventurer's own side, wire it to follow the
//! adventurer specifically through `Leader` and `Keep::leader`, and give
//! it the wits to go down the stairs beside them.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::allies::fight_for;
use crate::bestiary::{Bestiary, Kind};

/// What the shackled offer is called, declared with
/// [`AddVerb::add_verb`] by [`crate::plugin::UndermerePlugin`].
pub const FREE_VERB: &str = "free";

/// How long freeing a captive costs, in hundredths of a step: three of
/// the adventurer's own turns, spent in the one long action the engine
/// lets the player take, not broken off by anything short of finishing
/// it, since nothing shorter is on offer.
pub const FREE_TIME: u32 = 300;

/// Puts `captive` on the adventurer's own side for good, as
/// [`fight_for`] says, then overrides what it decides with to follow
/// `adventurer` by name: `Leader(adventurer)` and the kind's own
/// [`crate::bestiary::MonsterDef::follow_mind`], over the ordinary ally
/// mind's "whoever of my side is nearest". Also gives it
/// [`Wits::TRAVELS`], so it goes down the stairs after the adventurer when
/// it stood beside them as they left, the engine's own rule for anyone
/// who travels: most of what a captive's own kind may be does not carry
/// that wit by default, and a follower who could not leave the floor it
/// was freed on would be no follower at all.
pub fn free(world: &mut World, captive: Entity, adventurer: Entity) {
    let Some(kind) = world.get::<Kind>(captive).map(|k| k.0) else { return };
    let Some(side) = world.get::<Faction>(adventurer).map(|f| f.0) else { return };
    fight_for(world, captive, side);
    let mind = world.resource::<Bestiary>().defs.get(kind).follow_mind();
    let mut e = world.entity_mut(captive);
    e.remove::<crate::tricks::PatrolRoute>().insert((mind, Leader(adventurer)));
    if let Some(mut wits) = e.get_mut::<Intelligence>() {
        wits.0 = wits.0.with(Wits::TRAVELS);
    }
}

/// Answers a captive's own [`FREE_VERB`]: everything else `Interacted`
/// reports is a prop's or another verb's, left for whatever reads it.
///
/// In `TurnSet::React`, the set every game system answering `Interacted`
/// already reads from.
pub fn answer_free(mut commands: Commands, mut done: MessageReader<Interacted>, verbs: Res<Verbs>, players: Query<Entity, With<Player>>) {
    let Some(free_verb) = verbs.get(FREE_VERB) else { return };
    let Ok(adventurer) = players.single() else { return };
    for ev in done.read().filter(|ev| ev.verb == free_verb) {
        let captive = ev.target;
        commands.queue(move |world: &mut World| free(world, captive, adventurer));
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::{captive_floor, floor_of, map_of};
    use crate::testing::{adventurer, arrive, headless, sturdy};

    /// Every floor the run's captive did not draw holds none, and the one
    /// it drew holds exactly one, neutral to the adventurer, shackled in
    /// place and offering to be freed.
    #[test]
    fn exactly_one_floor_a_run_draws_holds_one_shackled_captive_offering_to_be_freed_over_a_span_of_seeds() {
        for s in 0..8u64 {
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            let floor = captive_floor(RunSeed(s));
            let mut seen = 0;
            for f in 1..crate::dungeon::FLOORS {
                arrive(&mut app, map_of(f));
                let free_verb = app.world().resource::<Verbs>().get(FREE_VERB).expect("declared");
                let world = app.world_mut();
                let count = world
                    .query::<(&OnMap, &Offers)>()
                    .iter(world)
                    .filter(|(on, offers)| on.0 == map_of(f) && offers.0.iter().any(|o| o.verb == free_verb))
                    .count();
                if f == floor {
                    assert_eq!(count, 1, "seed {s}, floor {f}: the run's own captive");
                    seen += 1;
                } else {
                    assert_eq!(count, 0, "seed {s}, floor {f}: no captive drawn here");
                }
            }
            assert_eq!(seen, 1, "seed {s}: the one floor the run drew");
        }
    }

    /// Freed, a captive follows the adventurer who freed it, fights for
    /// their side, goes down the stairs beside them, and is still their
    /// ally and still led by them once the run that follows is continued
    /// from the save the arrival wrote: Review Focus 3, start to finish.
    #[test]
    fn a_freed_captive_follows_the_adventurer_fights_for_them_and_goes_downstairs_beside_them_through_a_save() {
        let s = 0u64;
        let mut app = headless(RunSeed(s));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        let floor = captive_floor(RunSeed(s));
        arrive(&mut app, map_of(floor));
        let free_verb = app.world().resource::<Verbs>().get(FREE_VERB).expect("declared");
        let world = app.world_mut();
        let captive = world
            .query::<(Entity, &OnMap, &Offers)>()
            .iter(world)
            .find(|(_, on, offers)| on.0 == map_of(floor) && offers.0.iter().any(|o| o.verb == free_verb))
            .map(|(e, ..)| e)
            .expect("the run's own captive stands on the floor it drew");
        let captive_kind = app.world().get::<crate::bestiary::Kind>(captive).map(|k| k.0).expect("the captive is a monster");

        // Freed, moved beside the adventurer, and proven to fight for
        // them: a hostile placed adjacent to both is struck down between
        // them without the adventurer landing the killing blow alone.
        crate::captives::free(app.world_mut(), captive, me);
        let beside = app.world().get::<Position>(me).unwrap().0.offset(1, 0);
        app.world_mut().get_mut::<Position>(captive).unwrap().0 = beside;
        assert_eq!(app.world().get::<Faction>(captive).map(|f| f.0), app.world().get::<Faction>(me).map(|f| f.0), "fights for the adventurer's own side");
        assert_eq!(app.world().get::<Leader>(captive).map(|l| l.0), Some(me), "follows the adventurer by name");
        assert!(app.world().get::<Intelligence>(captive).is_some_and(|w| w.0.has(Wits::TRAVELS)), "can leave with them");

        // Down the stairs beside them: the engine's own `Wits::TRAVELS`
        // and `Keep::leader`'s `keeps_with_allies` bring a follower
        // through a `GoThrough` it stood beside the leaving actor for,
        // decided its own last turn, as the engine's own
        // `a_companion_that_travels_comes_through_with_the_player` reads:
        // one `Wait` beside them, close enough that nothing decides its
        // turn, and then the `GoThrough` finds it still there.
        let world = app.world_mut();
        let down = world
            .query::<(&Position, &crate::stairs::Way, &Transition)>()
            .iter(world)
            .find(|(_, w, _)| **w == crate::stairs::Way::Down)
            .map(|(p, _, t)| (p.0, t.to))
            .expect("the floor the captive was freed on has a way down");
        crate::testing::stand_on(&mut app, captive, down.0.offset(-1, 0));
        crate::testing::stand_on(&mut app, me, down.0);
        crate::testing::pass_turns(&mut app, 1);
        crate::testing::go_through(&mut app, me);
        let next = floor_of(app.world().get::<OnMap>(me).map(|m| m.0).unwrap()).unwrap();
        assert_eq!(next, floor + 1, "the adventurer went down");
        assert_eq!(app.world().get::<OnMap>(captive).map(|m| m.0), app.world().get::<OnMap>(me).map(|m| m.0), "the captive came down beside them");

        // The arrival just wrote the run down; continue it in a fresh app
        // and the captive is still there, still an ally, still led by the
        // adventurer's own new entity.
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");
        let mut again = crate::testing::continued(&text);
        let new_me = crate::testing::adventurer(&mut again);
        let world = again.world_mut();
        let leader = world
            .query::<(Entity, &Kind, &OnMap, Option<&Leader>, &crate::allies::Ally)>()
            .iter(world)
            .find(|(_, k, on, ..)| k.0 == captive_kind && on.0 == map_of(floor + 1))
            .map(|(_, _, _, leader, _)| leader.copied())
            .expect("the freed captive continues on the floor it came down to, still an ally");
        assert_eq!(leader.map(|l| l.0), Some(new_me), "still led by the adventurer's own new entity");
    }
}
