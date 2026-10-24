//! A shackled monster of the floor, freed to follow.
//!
//! `population::place_captive` stands
//! one a run, neutral and anchored by a one-waypoint `PatrolRoute`, the
//! universal brain's own `Patrol`, offering [`FREE_VERB`]
//! through the engine's own actor offers. Taking it up sets the adventurer
//! to work at the shackles, [`answer_free`] beginning the engine's `Work`
//! of the kind [`FREEING`], [`FREE_TURNS`] turns long, the offer's own turn
//! the first. The engine spends the rest for them and breaks it off as it
//! breaks off any of the player's work: a blow that lands, a foe coming
//! into sight, a key pressed or a turn spent on anything else, though a
//! walk into the captive again, pressed or held, presses on with the work
//! rather than leaving it off and beginning it over. Broken off,
//! the captive is still bound and [`answer_freeing`] says so; done, it
//! calls [`free`], which does what freeing means: put the captive on the
//! adventurer's own side, wire it to follow the adventurer specifically
//! through `Leader` and `Keep::leader`, and give it the wits to go down
//! the stairs beside them.

use atglyph::ecs::{BreakReason, WorkBroken, Works};
use atglyph::prelude::all::*;
use atglyph::primitives::turn::BASE_ACTION_COST;
use atglyph::rules::work::Work;
use bevy::prelude::*;

use crate::allies::fight_for;
use crate::bestiary::{Bestiary, Kind};

/// What the shackled offer is called, declared with
/// [`EngineApp::add_verb`](atglyph::ecs::EngineApp::add_verb) by [`crate::plugin::UndermerePlugin`].
pub const FREE_VERB: &str = "free";

/// The kind of work freeing a captive is, as the panels name it.
pub const FREEING: &str = "freeing a captive";

/// How many of the adventurer's turns freeing a captive takes, the offer's
/// own the first: long enough that a fight nearby is a reason to leave it
/// off, short enough to be worth starting in a quiet room.
pub const FREE_TURNS: u16 = 3;

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
    (Faction(side), crate::tricks::PatrolRoute { waypoints: vec![at], at: 0 }, Offers(vec![Offering::new(free_verb, BASE_ACTION_COST)]))
}

/// Where a shackled captive is held, the one waypoint [`shackled`] gave
/// it, or `None` for anything not shackled: what the save keeps of a
/// captive, since the rest of [`shackled`] follows from it.
pub fn held_at(world: &World, captive: Entity) -> Option<Point> {
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
/// [`Wits::TRAVELS`], both halves, so it goes down the stairs after the
/// adventurer when it stood beside them as they left, the engine's own
/// rule for anyone who follows through, and runs down one when hurt as a
/// kind that travels does: most of what a captive's own kind may be does not carry
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
/// mind, and [`Wits::TRAVELS`], following and fleeing through. The one
/// account of a follower, shared by
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

/// Answers a captive's own [`FREE_VERB`] by setting whoever took it up to
/// work at the shackles, [`FREE_TURNS`] turns of [`FREEING`] on the
/// captive, the offer's own turn the first: everything else `Interacted`
/// reports is a prop's or another verb's, left for whatever reads it.
///
/// In `TurnSet::React`, the set every game system answering `Interacted`
/// already reads from.
pub fn answer_free(mut done: MessageReader<Interacted>, verbs: Res<Verbs>, kinds: Res<WorkKinds>, mut works: Works, mut tell: MessageWriter<Tell>) {
    let (Some(free_verb), Some(freeing)) = (verbs.get(FREE_VERB), kinds.get(FREEING)) else { return };
    for ev in done.read().filter(|ev| ev.verb == free_verb) {
        works.begin(ev.actor, Work::new(freeing, FREE_TURNS).on(ev.target));
        tell.write(Tell::new("You set to work on the shackles.", Tones::NOTICE));
    }
}

/// Answers the work [`answer_free`] began: finished, the captive is
/// [`free`]d to follow the adventurer; broken off, by a blow, a foe in
/// sight, a key or the game, the log says the captive is still bound,
/// and it still offers to be freed. Nothing for a death, which says
/// enough.
///
/// In `TurnSet::React`, where the engine finishes work and breaks it off.
pub fn answer_freeing(
    mut commands: Commands,
    mut done: MessageReader<WorkDone>,
    mut broken: MessageReader<WorkBroken>,
    kinds: Res<WorkKinds>,
    players: Query<Entity, With<Player>>,
    mut tell: MessageWriter<Tell>,
) {
    let Some(freeing) = kinds.get(FREEING) else { return };
    let Ok(adventurer) = players.single() else { return };
    for ev in done.read().filter(|d| d.kind == freeing) {
        let Some(captive) = ev.target else { continue };
        commands.queue(move |world: &mut World| free(world, captive, adventurer));
    }
    for _ in broken.read().filter(|b| b.kind == freeing && b.reason != BreakReason::Died && b.actor == adventurer) {
        tell.write(Tell::new("You leave off at the shackles, and the captive is still bound.", Tones::BAD));
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
    use std::collections::BTreeMap;

    use atglyph::primitives::RunSeed;

    use super::*;
    use crate::dungeon::{captive_floor, floor_of, map_of};
    use crate::testing::{adventurer, arrive, headless, sturdy};

    /// The arena on floor two with a goblin shackled beside the
    /// adventurer, as `population::place_captive` stands the run's own.
    fn beside_a_captive(seed: u64) -> (App, Entity, Entity) {
        let (mut app, me, here) = crate::testing::arena_on(seed, 2);
        let at = here.offset(1, 0);
        let captive = crate::testing::monster(&mut app, "goblin", at);
        let shackles = shackled(app.world().resource::<Registries>(), app.world().resource::<Verbs>(), at);
        app.world_mut().entity_mut(captive).insert(shackles);
        crate::testing::settle(&mut app);
        (app, me, captive)
    }

    /// The adventurer takes up the captive's offer to be freed, as a walk
    /// into it or the interact key does.
    fn begin_freeing(app: &mut App, me: Entity, captive: Entity) {
        let verb = app.world().resource::<Verbs>().get(FREE_VERB).expect("declared");
        app.world_mut().write_message(Intent::new(me, Interact { target: captive, verb, thing: None }));
        app.update();
    }

    /// A blow that lands while the adventurer works at the shackles breaks
    /// the freeing off: the captive is still bound, still offering to be
    /// freed, and the log says so.
    #[test]
    fn a_blow_while_freeing_a_captive_breaks_it_off_and_leaves_the_captive_bound() {
        for s in 0..3 {
            let (mut app, me, captive) = beside_a_captive(s);
            begin_freeing(&mut app, me, captive);
            let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
            app.world_mut().write_message(DamageEvent::new(me, atglyph::rules::Hit::from_source(None, crush, 50)));
            for _ in 0..20 {
                app.update();
            }
            assert!(held_at(app.world(), captive).is_some(), "seed {s}: still shackled");
            assert!(app.world().get::<Leader>(captive).is_none(), "seed {s}: and following nobody");
            assert!(crate::testing::said(&app).iter().any(|l| l.contains("still bound")), "seed {s}: {:#?}", crate::testing::said(&app));
        }
    }

    /// Every work broken off in the run, kept for a test to read.
    #[derive(Resource, Default)]
    struct Broken(Vec<WorkBroken>);

    fn record_broken(mut broken: MessageReader<WorkBroken>, mut seen: ResMut<Broken>) {
        seen.0.extend(broken.read().copied());
    }

    /// A foe coming into sight while the adventurer works at the shackles
    /// breaks the freeing off before it can strike, and the captive stays
    /// bound.
    #[test]
    fn a_foe_coming_into_sight_while_freeing_a_captive_breaks_it_off_and_leaves_the_captive_bound() {
        for s in 0..3 {
            let (mut app, me, captive) = beside_a_captive(s);
            app.init_resource::<Broken>().add_systems(Turn, record_broken.in_set(TurnSet::Cleanup));
            begin_freeing(&mut app, me, captive);
            let here = crate::testing::at(&app, me);
            crate::testing::monster(&mut app, "goblin", here.offset(-3, 0));
            for _ in 0..20 {
                app.update();
            }
            let reasons: Vec<BreakReason> = app.world().resource::<Broken>().0.iter().map(|b| b.reason).collect();
            assert_eq!(reasons, [BreakReason::FoeSighted], "seed {s}: broken off by the sight of it");
            assert!(held_at(app.world(), captive).is_some(), "seed {s}: still shackled");
            assert!(crate::testing::said(&app).iter().any(|l| l.contains("still bound")), "seed {s}: {:#?}", crate::testing::said(&app));
        }
    }

    /// A shackled captive that takes things up stays on its chain with a
    /// blade lying three steps off: a goblin on seed 2's floor two walked off
    /// it to fetch the sword the adventurer set down, and the freeing broke
    /// off with the captive out of reach.
    #[test]
    fn a_shackled_captive_never_leaves_its_chain_to_fetch_what_lies_near() {
        for s in 0..3 {
            let (mut app, _me, captive) = beside_a_captive(s);
            let held = crate::testing::at(&app, captive);
            let blade = crate::testing::make(&mut app, "sword");
            let lying = held.offset(0, 2);
            crate::testing::lay_floor(&mut app, &[lying, held.offset(0, 1)]);
            crate::testing::lay(&mut app, blade, lying);
            crate::testing::pass_turns(&mut app, 10);
            assert_eq!(crate::testing::at(&app, captive), held, "seed {s}: still on its chain");
        }
    }

    /// Taken up in a quiet room, freeing a captive takes the adventurer
    /// three turns, the offer's own the first, as the single long action it
    /// once was did, and then the captive follows them on their side and
    /// offers to be freed no longer.
    #[test]
    fn freeing_a_captive_takes_three_turns_and_then_it_follows_the_adventurer() {
        for s in 0..3 {
            let (mut app, me, captive) = beside_a_captive(s);
            let before = app.world().resource::<Turns>().now();
            begin_freeing(&mut app, me, captive);
            assert!(held_at(app.world(), captive).is_some(), "seed {s}: still shackled on the first turn");
            for _ in 0..20 {
                app.update();
            }
            assert_eq!(app.world().resource::<Turns>().now() - before, 3 * BASE_ACTION_COST, "seed {s}: three turns spent");
            assert_eq!(held_at(app.world(), captive), None, "seed {s}: shackled no longer");
            assert_eq!(app.world().get::<Leader>(captive).map(|l| l.0), Some(me), "seed {s}: following the adventurer");
            assert_eq!(app.world().get::<Faction>(captive).map(|f| f.0), app.world().get::<Faction>(me).map(|f| f.0), "seed {s}: on their side");
        }
    }

    /// A run continued on the floor whose captive is still shackled finds
    /// it still shackled where it was held: neutral, anchored, and offering
    /// to be freed, rather than an ordinary monster of its kind. Every
    /// captive is asked it, the run's own and any a piece on the floor
    /// holds, so a floor that draws a piece of captives beside the run's
    /// own is no different.
    #[test]
    fn a_captive_left_shackled_is_still_shackled_once_the_run_is_continued() {
        let s = 0u64;
        let mut app = headless(RunSeed(s));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        let floor = captive_floor(RunSeed(s));
        arrive(&mut app, map_of(floor));
        let before = shackled_on(&mut app);
        assert!(!before.is_empty(), "the run's own captive, shackled on its floor");
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");
        let mut again = crate::testing::continued(&text);
        let captive_side = again.world().resource::<Registries>().factions.expect(crate::content::CAPTIVE);
        let after = shackled_on(&mut again);
        assert_eq!(after.keys().collect::<Vec<_>>(), before.keys().collect::<Vec<_>>(), "every captive, still offering to be freed");
        for (at, (anchor, side)) in after {
            assert_eq!((Some(at), side), (anchor, Some(captive_side)), "held where it stood, and neither friend nor foe");
        }
    }

    /// Every captive still offering to be freed, by where it stands: the
    /// cell it is held at and its side.
    fn shackled_on(app: &mut App) -> BTreeMap<Point, (Option<Point>, Option<FactionId>)> {
        let world = app.world_mut();
        let standing: Vec<(Entity, Point)> = world.query::<(Entity, &Position)>().iter(world).map(|(e, p)| (e, p.0)).collect();
        standing
            .into_iter()
            .filter_map(|(e, at)| held_at(app.world(), e).map(|anchor| (at, (Some(anchor), app.world().get::<Faction>(e).map(|f| f.0)))))
            .collect()
    }

    /// A captive killed in its shackles offers nothing from its body, and
    /// a run continued after holds no captive where it was held, rather
    /// than one shackled again over a corpse.
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
        let held = held_at(app.world(), captive).expect("shackled");
        kill(&mut app, captive, Some(me));
        assert!(app.world().get::<Remains>(captive).is_some(), "its body lies where it was held");
        assert_eq!(held_at(app.world(), captive), None, "and offers to be freed no longer");
        atglyph::save::save_run(app.world_mut()).expect("the run is written down");
        let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("a save");
        let mut again = crate::testing::continued(&text);
        let shackled: Vec<Point> = everyone(&mut again).into_iter().filter_map(|e| held_at(again.world(), e)).collect();
        assert!(!shackled.contains(&held), "nothing shackled again where it was killed in the run continued: {shackled:?}");
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

        // Down the stairs beside them: the engine's own `Wits::FOLLOWS_THROUGH`
        // and `Keep::leader`'s `keeps_with_allies` bring a follower
        // through a `GoThrough` it stood beside the leaving actor for,
        // decided its own last turn, as the engine's own
        // `a_companion_that_travels_comes_through_with_the_player` reads:
        // one `Wait` beside them, close enough that nothing decides its
        // turn, and then the `GoThrough` finds it still there.
        let world = app.world_mut();
        let down = world
            .query::<(&Position, &Stair, &Transition)>()
            .iter(world)
            .find(|(_, w, _)| w.climb == Climb::Down)
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
        let peace = CombatRules::new();
        again.insert_resource(peace);
        let world = again.world_mut();
        let down = world
            .query::<(&Position, &OnMap, &Stair)>()
            .iter(world)
            .find(|(_, on, w)| on.0 == map_of(floor + 1) && w.climb == Climb::Down)
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
