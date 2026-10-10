//! A shackled monster of the floor, freed to follow.
//!
//! `population::place_captive` stands
//! one a run, neutral and anchored by a one-waypoint `PatrolRoute`, the
//! universal brain's own `Patrol`, offering [`FREE_VERB`]
//! through the engine's own actor offers: a long action of
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

/// What makes a monster standing at `at` a captive: the captive side,
/// neutral to everyone, a one-waypoint `PatrolRoute` on its own cell, the
/// universal brain's own `Patrol` keeping it there, and the [`FREE_VERB`]
/// it offers. The one account of it, put on by
/// `population::place_captive` when a
/// floor is first stood and again by the save when a run that left one
/// shackled is continued, since neither the side nor the offer is the
/// engine's to keep.
pub fn shackled(registries: &Registries, verbs: &Verbs, at: Point) -> impl Bundle + use<> {
    let side = registries.factions.expect(crate::content::CAPTIVE);
    let free_verb = verbs.get(FREE_VERB).expect("UndermerePlugin declares the free verb");
    (Faction(side), crate::tricks::PatrolRoute { waypoints: vec![at], at: 0 }, Offers(vec![Offering::new(free_verb, FREE_TIME)]))
}

/// Where a shackled captive is held, the one waypoint [`shackled`] gave
/// it, or `None` for anything not shackled: what the save keeps of a
/// captive, since the rest of [`shackled`] follows from it.
pub fn held_at(world: &World, captive: Entity) -> Option<Point> {
    // Never the risk-and-reward pool's own "Captives in Chains", shackled
    // exactly the same way for the same reason, but a second, independent
    // source: the run's own captive, which this answers for, is part 3's
    // alone and the one `dungeon::captive_floor` draws.
    if world.get::<crate::vaults::PoolCaptive>(captive).is_some() {
        return None;
    }
    let free_verb = world.resource::<Verbs>().get(FREE_VERB)?;
    let offers = world.get::<Offers>(captive)?;
    if !offers.0.iter().any(|o| o.verb == free_verb) {
        return None;
    }
    world.get::<crate::tricks::PatrolRoute>(captive).and_then(|route| route.waypoints.first().copied())
}

/// Puts `captive` on the adventurer's own side for good, as
/// [`fight_for`] says, takes the offer to be freed off it, then overrides
/// what it decides with to follow
/// `adventurer` by name: `Leader(adventurer)` and the kind's own
/// [`crate::bestiary::MonsterDef::follow_mind`], over the ordinary ally
/// mind's "whoever of my side is nearest". Also gives it
/// [`Wits::TRAVELS`], so it goes down the stairs after the adventurer when
/// it stood beside them as they left, the engine's own rule for anyone
/// who travels: most of what a captive's own kind may be does not carry
/// that wit by default, and a follower who could not leave the floor it
/// was freed on would be no follower at all.
pub fn free(world: &mut World, captive: Entity, adventurer: Entity) {
    if world.get::<Kind>(captive).is_none() {
        return;
    }
    let Some(side) = world.get::<Faction>(adventurer).map(|f| f.0) else { return };
    fight_for(world, captive, side);
    world.entity_mut(captive).remove::<(crate::tricks::PatrolRoute, Offers)>().insert(Leader(adventurer));
    follow(world, captive);
}

/// Shapes a freed captive to follow its `Leader` by name: its kind's own
/// [`crate::bestiary::MonsterDef::follow_mind`] over the ordinary ally
/// mind, and [`Wits::TRAVELS`]. The one account of a follower, shared by
/// [`free`] and the save's restore of one freed before the run was saved,
/// since neither the mind nor the wit is the engine's to keep, and a
/// follower restored with the mind and not the wit stayed behind on the
/// first stairs after a continue. Nothing for an entity that is no
/// monster of the bestiary's.
pub fn follow(world: &mut World, captive: Entity) {
    let Some(kind) = world.get::<Kind>(captive).map(|k| k.0) else { return };
    let mind = world.resource::<Bestiary>().defs.get(kind).follow_mind();
    let mut e = world.entity_mut(captive);
    e.insert(mind);
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

/// What a captive is doing, as the rail's activity words name it: not one
/// of the engine's tactics, so `main.rs` gives it a word of its own.
pub const SHACKLED: &str = "shackled";

/// Says a captive is shackled where the rail says what a monster is doing,
/// `vampire bat (shackled)`, in the plain text of something neither friend
/// nor foe rather than muted as one unaware of the adventurer: read off the
/// [`FREE_VERB`] it offers, so it says so until the moment it is freed.
/// Without it a captive read as one more monster of the floor, and nothing
/// on the screen said it was the one to walk up to.
///
/// In `ViewSet::Annotate`.
pub fn note_captives(mut nearby: ResMut<NearbyView>, verbs: Res<Verbs>, offers: Query<&Offers>) {
    let Some(free_verb) = verbs.get(FREE_VERB) else { return };
    for row in &mut nearby.actors {
        if offers.get(row.entity).is_ok_and(|offers| offers.0.iter().any(|o| o.verb == free_verb)) {
            (row.doing, row.alert) = (Some(SHACKLED), None);
        }
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
                // Never the risk-and-reward pool's own "Captives in
                // Chains", shackled the identical way but a second,
                // independent source now: see `held_at`'s own doc.
                let count = world
                    .query_filtered::<(&OnMap, &Offers), Without<crate::vaults::PoolCaptive>>()
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

    /// A run continued on the floor whose captive is still shackled finds
    /// it still shackled where it was held: neutral, anchored, and offering
    /// to be freed, rather than an ordinary monster of its kind.
    #[test]
    fn a_captive_left_shackled_is_still_shackled_once_the_run_is_continued() {
        let s = 0u64;
        let mut app = headless(RunSeed(s));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        let floor = captive_floor(RunSeed(s));
        arrive(&mut app, map_of(floor));
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");
        let mut again = crate::testing::continued(&text);
        let captive_side = again.world().resource::<Registries>().factions.expect(crate::content::CAPTIVE);
        let world = again.world_mut();
        let held: Vec<(Entity, Point)> = world.query::<(Entity, &Position)>().iter(world).map(|(e, p)| (e, p.0)).collect();
        let shackled: Vec<(Point, Option<Point>, Option<FactionId>)> = held
            .iter()
            .filter_map(|(e, at)| held_at(again.world(), *e).map(|anchor| (*at, Some(anchor), again.world().get::<Faction>(*e).map(|f| f.0))))
            .collect();
        assert_eq!(shackled.len(), 1, "the one captive, still offering to be freed: {shackled:?}");
        let (at, anchor, side) = shackled[0];
        assert_eq!((Some(at), side), (anchor, Some(captive_side)), "held where it stood, and neither friend nor foe");
    }

    /// A captive killed in its shackles offers nothing from its body, and
    /// a run continued after holds no captive at all, rather than one
    /// shackled again over a corpse.
    #[test]
    fn a_captive_killed_in_its_shackles_offers_nothing_and_is_not_shackled_again_on_a_continue() {
        use crate::testing::kill;
        let s = 0u64;
        let mut app = headless(RunSeed(s));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        let floor = captive_floor(RunSeed(s));
        arrive(&mut app, map_of(floor));
        let everyone = |app: &mut App| {
            let mut all = app.world_mut().query::<Entity>();
            all.iter(app.world()).collect::<Vec<_>>()
        };
        let captive = everyone(&mut app).into_iter().find(|e| held_at(app.world(), *e).is_some()).expect("the run's own captive");
        kill(&mut app, captive, Some(me));
        assert!(app.world().get::<Remains>(captive).is_some(), "its body lies where it was held");
        assert_eq!(held_at(app.world(), captive), None, "and offers to be freed no longer");
        rl_engine::rl_save::save_run(app.world_mut()).expect("the run is written down");
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("a save");
        let mut again = crate::testing::continued(&text);
        let shackled: Vec<Entity> = everyone(&mut again).into_iter().filter(|e| held_at(again.world(), *e).is_some()).collect();
        assert!(shackled.is_empty(), "nothing shackled again in the run continued: {shackled:?}");
    }

    /// The rail says the captive is shackled, in plain text, until it is
    /// freed, and says nothing of the kind once it is.
    #[test]
    fn the_rail_says_a_captive_is_shackled_until_it_is_freed() {
        use crate::testing::{settle, stand_on};
        let s = 0u64;
        let mut app = headless(RunSeed(s));
        app.add_plugins(NearbyViewPlugin).add_systems(Update, note_captives.in_set(ViewSet::Annotate));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        let floor = captive_floor(RunSeed(s));
        arrive(&mut app, map_of(floor));
        let free_verb = app.world().resource::<Verbs>().get(FREE_VERB).expect("declared");
        let world = app.world_mut();
        let (captive, at) = world
            .query::<(Entity, &Position, &Offers)>()
            .iter(world)
            .find(|(_, _, offers)| offers.0.iter().any(|o| o.verb == free_verb))
            .map(|(e, p, _)| (e, p.0))
            .expect("the run's own captive");
        stand_on(&mut app, me, at.offset(1, 0));
        settle(&mut app);
        let doing = |app: &App| app.world().resource::<NearbyView>().actors.iter().find(|r| r.entity == captive).map(|r| (r.doing, r.alert));
        assert_eq!(doing(&app), Some((Some(SHACKLED), None)), "shackled, and neither unaware nor hunting");
        free(app.world_mut(), captive, me);
        settle(&mut app);
        assert_ne!(doing(&app).and_then(|(d, _)| d), Some(SHACKLED), "freed, it is shackled no longer");
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
        let (follower, leader) = world
            .query::<(Entity, &Kind, &OnMap, Option<&Leader>, &crate::allies::Ally)>()
            .iter(world)
            .find(|(_, k, on, ..)| k.0 == captive_kind && on.0 == map_of(floor + 1))
            .map(|(e, _, _, leader, _)| (e, leader.copied()))
            .expect("the freed captive continues on the floor it came down to, still an ally");
        assert_eq!(leader.map(|l| l.0), Some(new_me), "still led by the adventurer's own new entity");
        assert!(again.world().get::<Intelligence>(follower).is_some_and(|w| w.0.has(Wits::TRAVELS)), "and can still leave with them");

        // And down the next stairs beside them, the continued run's own:
        // the wit to travel kept is the wit used. At peace with the floor,
        // so the follower's one turn beside them is spent keeping with
        // them rather than going after whatever of the floor it can see.
        let peace = CombatRules::new(&again.world().resource::<Registries>().factions);
        again.insert_resource(peace);
        let world = again.world_mut();
        let down = world
            .query::<(&Position, &OnMap, &crate::stairs::Way)>()
            .iter(world)
            .find(|(_, on, w)| on.0 == map_of(floor + 1) && **w == crate::stairs::Way::Down)
            .map(|(p, ..)| p.0)
            .expect("the floor it came down to has a way down");
        crate::testing::stand_on(&mut again, follower, down.offset(-1, 0));
        crate::testing::stand_on(&mut again, new_me, down);
        crate::testing::pass_turns(&mut again, 1);
        crate::testing::go_through(&mut again, new_me);
        let on = |app: &App, who: Entity| app.world().get::<OnMap>(who).map(|m| m.0);
        assert_eq!(on(&again, new_me), Some(map_of(floor + 2)), "the adventurer went down again");
        assert_eq!(on(&again, follower), on(&again, new_me), "and the captive freed before the save came down beside them");
    }
}
