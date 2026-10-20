//! A hired party and the tavern's candidates through a save: what each
//! test saves, continues in a fresh app, and finds there.

use std::sync::Arc;

use bevy::prelude::*;
use rl_engine::prelude::*;

use super::fixtures::{beside, in_mereholt, offered, take, waiting};
use super::*;
use crate::dungeon::{TOWN, map_of};
use crate::save::SLOT;
use crate::testing::{adventurer, arrive, at, continued, name_of, pass_turns, set_gold, stand_on, worn};
use crate::townsfolk::GivenNames;

/// The run written down where it stands, read back as text.
fn saved(app: &mut App) -> String {
    rl_engine::rl_save::save_run(app.world_mut()).expect("the run is written down");
    written(app)
}

/// What the slot holds now.
fn written(app: &App) -> String {
    app.world().resource::<Saves>().load(SLOT).expect("the slot reads").expect("a save")
}

/// What `who` carries, by name, each with whether it is its own kit,
/// sorted.
fn kit(app: &App, who: Entity) -> Vec<(String, bool)> {
    let world = app.world();
    let mut kit: Vec<(String, bool)> =
        world.get::<Inventory>(who).expect("a pack").items.iter().map(|i| (name_of(app, *i), world.get::<OwnKit>(*i).is_some())).collect();
    kit.sort();
    kit
}

/// Whether `who` thinks with the brain for `order`.
fn thinks(app: &App, who: Entity, order: Order) -> bool {
    let mind = app.world().get::<Mind>(who).expect("a mind");
    Arc::ptr_eq(&mind.0, &app.world().resource::<MercenaryBrains>().mind(order).0)
}

/// Puts `who` under `order`, as the order key does for the party.
fn order(app: &mut App, who: Entity, order: Order) {
    app.world_mut().get_mut::<Mercenary>(who).expect("a mercenary").order = order;
    obey(app.world_mut(), who, order);
}

/// One of the party as a test compares it across a save: who it is and
/// what it was hired for and told, where it stands and on which map,
/// whether it follows the adventurer and fights on their side, with the
/// brain for its order and never the wit to travel alone, what it offers,
/// what it carries and wears, and the post it holds.
#[derive(Debug, PartialEq)]
struct Member {
    hired: Mercenary,
    on: MapId,
    at: Point,
    theirs: bool,
    their_side: bool,
    obeys: bool,
    flees_through: bool,
    offers: Vec<(String, Option<u32>)>,
    kit: Vec<(String, bool)>,
    worn: Vec<Option<String>>,
    post: Option<Point>,
}

/// Every mercenary of the run, in the order it joined.
fn party(app: &mut App) -> Vec<Member> {
    let me = adventurer(app);
    let world = app.world_mut();
    let mut hired: Vec<(Joined, Entity)> = world.query_filtered::<(&Joined, Entity), With<Mercenary>>().iter(world).map(|(j, e)| (*j, e)).collect();
    hired.sort();
    let world = app.world();
    hired
        .into_iter()
        .map(|(_, who)| {
            let mercenary = world.get::<Mercenary>(who).expect("hired").clone();
            let order = mercenary.order;
            Member {
                hired: mercenary,
                on: world.get::<OnMap>(who).expect("on a map").0,
                at: at(app, who),
                theirs: world.get::<Leader>(who) == Some(&Leader(me)),
                their_side: world.get::<crate::allies::Ally>(who).is_some() && world.get::<Faction>(who) == world.get::<Faction>(me),
                obeys: thinks(app, who, order),
                flees_through: world.get::<Intelligence>(who).is_some_and(|i| i.0.has(Wits::FLEES_THROUGH)),
                offers: offered(app, who),
                kit: kit(app, who),
                worn: worn(app, who),
                post: world.get::<Post>(who).map(|p| p.0),
            }
        })
        .collect()
}

/// The mercenary of the run called `name`.
fn called(app: &mut App, name: &str) -> Entity {
    let world = app.world_mut();
    world.query::<(Entity, &Mercenary)>().iter(world).find(|(_, m)| m.name == name).map(|(e, _)| e).unwrap_or_else(|| panic!("nobody hired is called {name}"))
}

/// One candidate as a test compares it across a save: where it waits,
/// its name and fee, what it offers, what it carries and wears, and
/// whether it has a mind, which a candidate never has.
type Seated = (Point, String, u32, Vec<(String, Option<u32>)>, Vec<(String, bool)>, Vec<Option<String>>, bool);

/// Every candidate waiting, by where it stands.
fn seated(app: &mut App) -> Vec<Seated> {
    waiting(app)
        .into_iter()
        .map(|c| {
            let world = app.world();
            let fee = world.get::<Candidate>(c).expect("a candidate").fee;
            (at(app, c), name_of(app, c), fee, offered(app, c), kit(app, c), worn(app, c), world.get::<Mind>(c).is_some())
        })
        .collect()
}

/// Down to the first floor and home again: one more visit.
fn visit(app: &mut App) {
    arrive(app, map_of(1));
    arrive(app, TOWN);
}

/// Two hired out of the tavern, one under the follow order and one told
/// to fight freely, are saved and continued: the
/// same two, in the order they joined, each still the adventurer's on
/// their side with the brain for its order and no wit to travel alone,
/// offering `give` and `dismiss`, carrying and wearing what it did with
/// its own kit still its own, and hired for the fee it was. The third
/// candidate still offers no `hire`, since the party is full.
/// One seed: the save of a hired party is wiring, and the seed-varying candidates round-trip in the three-candidates test below.
#[test]
fn a_hired_party_survives_a_save_and_continues_with_its_leader_join_order_orders_offers_own_kit_and_fee() {
    let seed = 0;
    let (mut app, me) = in_mereholt(seed);
    set_gold(&mut app, me, 10_000);
    let candidates = waiting(&mut app);
    take(&mut app, me, candidates[1], HIRE);
    take(&mut app, me, candidates[0], HIRE);
    order(&mut app, candidates[0], Order::FightFreely);
    let before = party(&mut app);
    assert_eq!(before.len(), 2, "seed {seed}: two hired");
    assert!(before.iter().all(|m| m.kit.iter().any(|(_, own)| *own)), "seed {seed}: each with its own kit: {before:?}");
    let hires = |app: &App, who: Entity| offered(app, who).iter().any(|(v, _)| v == HIRE);
    assert!(!hires(&app, candidates[2]), "seed {seed}: the party is full");

    let mut again = continued(&saved(&mut app));
    assert_eq!(party(&mut again), before, "seed {seed}");
    let third = waiting(&mut again);
    assert_eq!(third.len(), 1, "seed {seed}: the third still waits");
    assert!(!hires(&again, third[0]), "seed {seed}: and still offers no hire: {:?}", offered(&again, third[0]));
}

/// A mercenary told to hold, the adventurer gone off across the town,
/// comes back from a save on the post it held, under the hold order, and
/// keeps to it.
#[test]
fn a_mercenary_on_hold_keeps_its_post_through_a_load() {
    let (mut app, me) = in_mereholt(1);
    let start = at(&app, me);
    set_gold(&mut app, me, 10_000);
    let brenna = waiting(&mut app)[0];
    take(&mut app, me, brenna, HIRE);
    order(&mut app, brenna, Order::Hold);
    let post = at(&app, brenna);
    stand_on(&mut app, me, start);
    let name = app.world().get::<Mercenary>(brenna).expect("hired").name.clone();

    let mut again = continued(&saved(&mut app));
    let brenna = called(&mut again, &name);
    assert_eq!(again.world().get::<Post>(brenna), Some(&Post(post)), "the post it held, not where a restore first stands it");
    assert_eq!(again.world().get::<Mercenary>(brenna).map(|m| m.order), Some(Order::Hold));
    assert!(thinks(&again, brenna, Order::Hold), "the hold brain");
    pass_turns(&mut again, 5);
    assert_eq!(at(&again, brenna), post, "and it keeps to it");
}

/// A run saved by its arrival in Mereholt, over a span of seeds, finds
/// the same three candidates at the same tables when continued: the same
/// names, fees and kits, each thing of a kit still its carrier's own,
/// worn as it was, offering `hire` at its fee and `talk`, with no mind.
#[test]
fn a_run_saved_in_the_tavern_finds_the_same_three_candidates_with_the_same_kits_and_fees_offering_hire_and_talk_over_a_span_of_seeds() {
    for seed in 0..4 {
        let (mut app, _) = in_mereholt(seed);
        let before = seated(&mut app);
        assert_eq!(before.len(), 3, "seed {seed}");
        for (.., fee, offers, kit, _, mind) in &before {
            assert_eq!(offers, &[(HIRE.to_string(), Some(*fee)), (crate::townsfolk::TALK.to_string(), None)], "seed {seed}");
            assert!(!kit.is_empty() && kit.iter().all(|(_, own)| *own) && !mind, "seed {seed}: {kit:?}");
        }
        let mut again = continued(&written(&app));
        assert_eq!(seated(&mut again), before, "seed {seed}");
    }
}

/// The tavern's count of visits is the run's and is saved with it: the visit after a continue seats the three the run
/// would have seated had it never been saved.
/// One seed: the count is a plain integer, and the seed only picks which three it seats.
#[test]
fn the_visit_count_survives_so_the_next_visit_seats_what_it_would_have() {
    let seed = 0;
    let (mut app, _) = in_mereholt(seed);
    visit(&mut app);
    let mut again = continued(&written(&app));
    assert_eq!(again.world().resource::<Tavern>(), app.world().resource::<Tavern>(), "seed {seed}");
    visit(&mut app);
    visit(&mut again);
    assert_eq!(seated(&mut again), seated(&mut app), "seed {seed}");
}

/// Every name the run has given is saved with it, so the candidates of the visits after a continue repeat none of
/// them, and none of each other's.
/// One seed: the given names are a set saved whole, and the seed only picks which.
#[test]
fn the_names_the_run_has_given_survive_so_no_later_candidate_repeats_one() {
    let seed = 0;
    let (app, _) = in_mereholt(seed);
    let given = app.world().resource::<GivenNames>().clone();
    let mut again = continued(&written(&app));
    assert_eq!(again.world().resource::<GivenNames>(), &given, "seed {seed}");
    let mut later: Vec<String> = Vec::new();
    for _ in 0..4 {
        visit(&mut again);
        later.extend(seated(&mut again).into_iter().map(|s| s.1));
    }
    assert!(later.iter().all(|n| !given.has(n)), "seed {seed}: none given before the save: {later:?}");
    let mut distinct = later.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), later.len(), "seed {seed}: none twice: {later:?}");
}

/// A mercenary left behind in Mereholt, the run saved by the adventurer's
/// arrival on the first floor, is found where it was left, still theirs
/// and under its order, and comes along the next time they leave from
/// beside it.
/// One seed: the leaving and the return are the town's hand-drawn geometry, not the draw.
#[test]
fn a_mercenary_left_behind_on_another_floor_is_found_there_still_in_the_party_and_follows_again_on_return() {
    let seed = 0;
    let (mut app, me) = in_mereholt(seed);
    let start = at(&app, me);
    set_gold(&mut app, me, 10_000);
    let brenna = waiting(&mut app)[0];
    take(&mut app, me, brenna, HIRE);
    stand_on(&mut app, me, start);
    let left = at(&app, brenna);
    assert!(geometry::chebyshev(start, left) > LEASH, "seed {seed}: the adventurer leaves from out of its reach");
    arrive(&mut app, map_of(1));
    assert_eq!(app.world().get::<OnMap>(brenna).map(|m| m.0), Some(TOWN), "seed {seed}: left behind");
    let name = app.world().get::<Mercenary>(brenna).expect("hired").name.clone();

    let mut again = continued(&written(&app));
    let me = adventurer(&mut again);
    let brenna = called(&mut again, &name);
    assert_eq!((again.world().get::<OnMap>(brenna).map(|m| m.0), at(&again, brenna)), (Some(TOWN), left), "seed {seed}: where it was left");
    assert_eq!(again.world().get::<Leader>(brenna), Some(&Leader(me)), "seed {seed}: still theirs");
    assert!(thinks(&again, brenna, Order::Follow), "seed {seed}: under its order");
    arrive(&mut again, TOWN);
    let by = beside(&mut again, brenna);
    stand_on(&mut again, me, by);
    arrive(&mut again, map_of(1));
    assert_eq!(again.world().get::<OnMap>(brenna).map(|m| m.0), Some(map_of(1)), "seed {seed}: it came along");
}

/// A hired mercenary cut down lies as the corpse of its name, not as its
/// kind's, and a continue lays it back down by that name, not as
/// "mercenary corpse".
#[test]
fn a_fallen_mercenarys_body_is_the_corpse_of_its_name_and_still_is_after_a_continue() {
    let (mut app, me, here) = crate::testing::arena_on(5, 1);
    let brenna = super::fixtures::hired(&mut app, me, here.offset(2, 0), "Brenna");
    let name = "Brenna".to_string();
    let goblin = crate::testing::monster(&mut app, "goblin", here.offset(-2, 0));
    crate::testing::kill(&mut app, brenna, Some(goblin));
    let body = format!("the corpse of {name}");
    assert_eq!(name_of(&app, brenna), body, "named as its own on the floor");

    let mut again = continued(&saved(&mut app));
    let world = again.world_mut();
    let bodies: Vec<String> = world
        .query_filtered::<&Name, With<rl_engine::rl_bevy::Remains>>()
        .iter(world)
        .map(|n| n.as_str().to_string())
        .filter(|n| n.contains("corpse"))
        .collect();
    assert!(bodies.contains(&body), "laid back down by its name: {bodies:?}");
    assert!(!bodies.iter().any(|n| n.contains("mercenary")), "and never by its kind: {bodies:?}");
}
