//! Each potion in play, drunk and thrown: what it does to its drinker or
//! to whoever it breaks on, what the statuses it puts on do while they
//! last, and when each is plain enough to make its kind known.
//!
//! As `play` and `magic` do, these play the run that ships: things made by
//! the armory as a floor's own are, drunk and thrown by the adventurer
//! through the pack's own actions.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::content::{CONFUSED, DARKNESS, FIREPROOF, HASTED, PARALYSED, SHROUDED, SPEED, STEALTH};
use crate::dungeon::map_of;
use crate::lore::Lore;
use crate::testing::{adventurer, arena, arrive, at, find, give, headless, here, monster, pass_turns, said, settle, sturdy, use_thing, wall_off};

/// `name` found and put in `me`'s pack.
fn carried(app: &mut App, me: Entity, name: &str) -> Entity {
    let thing = find(app, name, 0);
    give(app, me, thing);
    thing
}

/// Whether `who` holds the status called `name`.
fn holds(app: &App, who: Entity, name: &str) -> bool {
    let status = app.world().resource::<Registries>().statuses.expect(name);
    app.world().get::<Afflicted>(who).is_some_and(|a| a.0.has(status))
}

/// `who`'s stat called `name`, as everything that reads it reads it.
fn stat(app: &App, who: Entity, name: &str) -> i32 {
    let registries = app.world().resource::<Registries>();
    app.world().get::<StatBlock>(who).expect("stats").0.value(registries.stats.expect(name), &registries.stats)
}

/// `me` throws `thing`, out of their pack, at `cell`, and the turns run
/// until it has landed and they may act again.
fn throw(app: &mut App, me: Entity, thing: Entity, cell: Point) {
    app.world_mut().write_message(Intent::new(me, Throw { item: thing, at: cell }));
    settle(app);
    settle(app);
}

/// A potion of paralysis thrown at a goblin bursts into a cloud that holds
/// it: paralysed, it loses its turns, and stands where it stood though the
/// adventurer is in reach of a few steps; the adventurer, out of the cloud,
/// is not held.
#[test]
fn a_potion_of_paralysis_thrown_holds_whoever_breathes_its_cloud() {
    let (mut app, me, here) = arena(21);
    let goblin = monster(&mut app, "goblin", here.offset(5, 0));
    let potion = carried(&mut app, me, "potion of paralysis");
    throw(&mut app, me, potion, here.offset(5, 0));
    let stood = at(&app, goblin);
    pass_turns(&mut app, 2);
    assert!(holds(&app, goblin, PARALYSED), "the goblin breathed it and is held");
    assert_eq!(at(&app, goblin), stood, "and has not moved a step towards the adventurer");
    assert!(!holds(&app, me, PARALYSED), "the adventurer, out of the cloud, is not");
}

/// Puts `status` on `who` for `turns` turns, as a cloud or a bolt would.
fn afflict(app: &mut App, who: Entity, status: &str, turns: u32) {
    let status = app.world().resource::<Registries>().statuses.expect(status);
    app.world_mut().write_message(Afflict { target: who, status, turns, by: None, held_by: None });
    settle(app);
}

/// Confused, the adventurer's every walk east goes a random way, and so
/// does a goblin's every step towards them: over a span of seeds, eight
/// walks east never come to eight steps east and go more than one way, and
/// a goblin three steps off, which unconfused closes in with every step it
/// takes, takes one that does not.
#[test]
fn confusion_turns_every_step_a_random_way_for_the_adventurer_and_a_monster_alike_over_a_span_of_seeds() {
    for seed in 0..4 {
        let (mut app, me, here) = arena(30 + seed);
        afflict(&mut app, me, CONFUSED, 40);
        let mut ways = Vec::new();
        for _ in 0..8 {
            let before = at(&app, me);
            app.world_mut().write_message(Intent::new(me, Bump(Direction::East)));
            settle(&mut app);
            ways.push(Direction::between(before, at(&app, me)));
        }
        assert_ne!(at(&app, me), here.offset(8, 0), "seed {seed}: eight walks east never went eight steps east: {ways:?}");
        assert!(ways.iter().all(Option::is_some), "seed {seed}: and every one went somewhere, never into a wall: {ways:?}");
        let mut taken: Vec<_> = ways.iter().flatten().copied().collect();
        taken.sort_by_key(|d| d.index());
        taken.dedup();
        assert!(taken.len() > 1, "seed {seed}: the steps went more than one way: {ways:?}");

        let (mut app, me, here) = arena(40 + seed);
        let goblin = monster(&mut app, "goblin", here.offset(-3, 0));
        afflict(&mut app, goblin, CONFUSED, 40);
        let mut astray = 0;
        for _ in 0..8 {
            let gap = geometry::chebyshev(at(&app, goblin), at(&app, me));
            let from = at(&app, goblin);
            pass_turns(&mut app, 1);
            let moved = at(&app, goblin) != from;
            if moved && geometry::chebyshev(at(&app, goblin), at(&app, me)) >= gap {
                astray += 1;
            }
        }
        assert!(astray > 0, "seed {seed}: the goblin took a step that did not close on the adventurer");
    }
}

/// A confused adventurer with one way open walks that way every time, and
/// never into the walls round it, whatever way they meant: a stumble goes
/// a way that can be walked.
#[test]
fn a_confused_step_only_ever_goes_a_way_that_can_be_walked() {
    let (mut app, me, here) = arena(25);
    let walls: Vec<Point> = geometry::square(here, 1).filter(|p| *p != here && *p != here.offset(0, -1)).collect();
    wall_off(&mut app, &walls);
    afflict(&mut app, me, CONFUSED, 40);
    app.world_mut().write_message(Intent::new(me, Bump(Direction::East)));
    settle(&mut app);
    assert_eq!(at(&app, me), here.offset(0, -1), "the one way open");
}

/// A potion of descent sinks its drinker through the floor to a free cell
/// of the floor below, and is known by it; on the tenth floor, the deepest,
/// there is nowhere to sink to: the drinker stays, the log says nothing
/// seemed to happen, and the potion is as unknown as it was.
#[test]
fn descent_sinks_the_drinker_to_the_floor_below_and_does_nothing_on_floor_ten() {
    let mut app = headless(rl_engine::rl_core::RunSeed(5));
    let me = adventurer(&mut app);
    sturdy(&mut app, me);
    arrive(&mut app, map_of(3));
    let potion = carried(&mut app, me, "potion of descent");
    use_thing(&mut app, me, potion);
    settle(&mut app);
    assert_eq!(here(&app), map_of(4), "sunk to the floor below");
    assert!(app.world().resource::<WorldMap>().is_walkable(at(&app, me)), "onto a cell one can stand on");
    let world = app.world_mut();
    let mut ways = world.query::<(&Position, &OnMap, &crate::stairs::Way)>();
    let up = ways.iter(world).find(|(_, on, way)| on.0 == map_of(4) && **way == crate::stairs::Way::Up).map(|(p, ..)| p.0);
    assert_ne!(Some(at(&app, me)), up, "somewhere on it, not at the stairs the floor is come down by");
    assert!(app.world().resource::<Lore>().knows("potion of descent"), "and known by it");

    let mut app = headless(rl_engine::rl_core::RunSeed(5));
    let me = adventurer(&mut app);
    sturdy(&mut app, me);
    arrive(&mut app, map_of(10));
    let stood = at(&app, me);
    let potion = carried(&mut app, me, "potion of descent");
    use_thing(&mut app, me, potion);
    settle(&mut app);
    assert_eq!((here(&app), at(&app, me)), (map_of(10), stood), "nowhere below the deepest floor");
    assert!(said(&app).iter().any(|l| l == "Nothing seems to happen."), "{:#?}", said(&app));
    assert!(!app.world().resource::<Lore>().knows("potion of descent"), "and the potion is as unknown as it was");
}

/// The potions that are good for whoever drinks them do what their lines
/// say: haste doubles the drinker's speed for fifteen turns, fire immunity
/// keeps every point of fire off for fifty, darkness puts the lantern out,
/// makes its drinker the harder to notice and leaves them in a cloud of
/// dark, and life is ten more health for good, and all of it.
#[test]
fn haste_fire_immunity_darkness_and_life_do_to_their_drinker_what_their_lines_say() {
    let (mut app, me, here) = arena(22);
    let haste = carried(&mut app, me, "potion of haste");
    use_thing(&mut app, me, haste);
    assert!(holds(&app, me, HASTED) && stat(&app, me, SPEED) == 100, "twice as fast");
    // Two waits to a turn while it lasts.
    pass_turns(&mut app, 32);
    assert!(!holds(&app, me, HASTED) && stat(&app, me, SPEED) == 0, "for fifteen turns");

    let fireproof = carried(&mut app, me, "potion of fire immunity");
    use_thing(&mut app, me, fireproof);
    let burn = |app: &mut App| {
        let fire = app.world().resource::<Registries>().damage_kinds.expect("fire");
        let before = app.world().get::<Health>(me).expect("health").current;
        app.world_mut().write_message(DamageEvent::new(me, rl_engine::rl_rules::Hit::from_source(None, fire, 10)));
        settle(app);
        before - app.world().get::<Health>(me).expect("health").current
    };
    assert!(holds(&app, me, FIREPROOF));
    assert_eq!(burn(&mut app), 0, "fire does nothing");
    pass_turns(&mut app, 51);
    assert_eq!(burn(&mut app), 10, "and burns again once it wears off");

    let dark = carried(&mut app, me, "potion of darkness");
    use_thing(&mut app, me, dark);
    let lantern = |app: &mut App| {
        let world = app.world_mut();
        let def = world.resource::<crate::armory::Armory>().defs.expect(crate::run::LANTERN);
        let mut lights = world.query::<(&crate::armory::ItemKind, &LightSource)>();
        lights.iter(world).find(|(kind, _)| kind.0 == def).map(|(_, l)| l.intensity)
    };
    assert!(holds(&app, me, SHROUDED), "shrouded");
    assert_eq!(lantern(&mut app), Some(0), "the lantern out");
    assert_eq!(stat(&app, me, STEALTH), 30, "thirty harder to notice");
    let darkness = app.world().resource::<Registries>().gases.expect(DARKNESS);
    assert!(app.world().resource::<Gases>().at(darkness, here) > 0, "in a cloud of dark");
    pass_turns(&mut app, 41);
    assert!(!holds(&app, me, SHROUDED) && lantern(&mut app) == Some(crate::content::LIGHT_INTENSITY), "and lit again after forty turns");

    app.world_mut().entity_mut(me).insert(Health { current: 10, max: 40 });
    let life = carried(&mut app, me, "potion of life");
    use_thing(&mut app, me, life);
    assert_eq!(app.world().get::<Health>(me).copied(), Some(Health { current: 50, max: 50 }), "ten more for good, and all of it");
}

/// A potion of healing thrown at a hurt goblin mends whoever it breaks
/// on, by its roll of at least seven, and one of incineration drunk bursts
/// into flame round its drinker, who is in it.
#[test]
fn a_thrown_healing_potion_mends_whoever_it_breaks_on_and_incineration_drunk_burns_its_drinker() {
    let (mut app, me, here) = arena(23);
    let hurt = monster(&mut app, "goblin", here.offset(3, 0));
    app.world_mut().entity_mut(hurt).remove::<Mind>().insert(Health { current: 2, max: 10 });
    let potion = carried(&mut app, me, "potion of healing");
    throw(&mut app, me, potion, here.offset(3, 0));
    assert!(app.world().get::<Health>(hurt).is_some_and(|h| h.current >= 9), "mended by seven at least");

    let health = |app: &App| app.world().get::<Health>(me).expect("health").current;
    let before = health(&app);
    let fire = carried(&mut app, me, "potion of incineration");
    use_thing(&mut app, me, fire);
    assert!(health(&app) < before, "the drinker is burnt");
    let burning = app.world().resource::<Fire>();
    assert!(burning.is_burning(here) && burning.is_burning(here.offset(2, 0)), "and the ground round them, two cells out, alight");
}

/// A kobold carrying a potion of paralysis never throws it, for a monster
/// throws only what strikes; carrying darts, it throws those.
#[test]
fn a_monster_throws_darts_it_carries_and_never_a_potion() {
    for (name, thrown) in [("potion of paralysis", false), ("dart", true)] {
        let (mut app, me, here) = arena(24);
        let kobold = monster(&mut app, "kobold", here.offset(4, 0));
        let thing = find(&mut app, name, 0);
        give(&mut app, kobold, thing);
        pass_turns(&mut app, 6);
        let still_held = app.world().get::<Inventory>(kobold).is_some_and(|bag| bag.items.contains(&thing));
        assert_eq!(!still_held, thrown, "{name}: thrown, or kept");
        assert!(!holds(&app, me, PARALYSED), "{name}: and the adventurer is never held by it");
    }
}

/// A potion that only does something to whoever it breaks on, thrown at
/// bare floor, shows nothing: it is as unknown as it was and the log does
/// not name it. The same thrown at a goblin is seen to land on it, and is
/// known by it.
#[test]
fn a_thrown_potion_that_breaks_on_nobody_teaches_nothing_and_one_that_breaks_on_a_goblin_does() {
    for name in ["potion of healing", "potion of extra healing", "potion of haste", "potion of fire immunity"] {
        let (mut app, me, here) = arena(26);
        // Faces dealt afresh with nothing known, the kit's healing among
        // them, as if the adventurer had set out with none.
        let world = app.world();
        let lore = Lore::draw(world.resource::<crate::lore::Faces>(), &world.resource::<crate::armory::Armory>().defs, world.resource::<Seed>());
        app.world_mut().insert_resource(lore);
        let potion = carried(&mut app, me, name);
        throw(&mut app, me, potion, here.offset(3, 0));
        let line = format!("That was {}.", rl_engine::rl_core::noun::with_article(name));
        assert!(!app.world().resource::<Lore>().knows(name), "{name} broken on bare floor is not learned");
        assert!(!said(&app).contains(&line), "{name}: {:#?}", said(&app));
        let goblin = monster(&mut app, "goblin", here.offset(-3, 0));
        app.world_mut().entity_mut(goblin).remove::<Mind>();
        let potion = carried(&mut app, me, name);
        throw(&mut app, me, potion, here.offset(-3, 0));
        assert!(app.world().resource::<Lore>().knows(name), "{name} broken on a goblin is learned: {:#?}", said(&app));
        assert!(said(&app).contains(&line), "{name}: {:#?}", said(&app));
    }
}

/// The potions, by name.
const POTIONS: [&str; 11] = [
    "potion of healing",
    "potion of extra healing",
    "potion of strength",
    "potion of life",
    "potion of haste",
    "potion of fire immunity",
    "potion of darkness",
    "potion of descent",
    "potion of incineration",
    "potion of paralysis",
    "potion of confusion",
];

/// Every potion but the kit's healing is found unknown, called by its
/// face, and once drunk on the first floor is known, every one of them
/// plain to whoever drinks it there, with a line in the log in its true
/// name.
#[test]
fn every_potion_is_found_unknown_under_its_face_and_known_once_drunk() {
    for name in POTIONS.into_iter().filter(|n| *n != "potion of healing") {
        let mut app = headless(rl_engine::rl_core::RunSeed(9));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        arrive(&mut app, map_of(1));
        let potion = carried(&mut app, me, name);
        settle(&mut app);
        let face = app.world().resource::<Lore>().face(name).unwrap_or_else(|| panic!("{name} wears a face")).called.clone();
        assert_eq!(crate::testing::name_of(&app, potion), face, "{name} is called by its face");
        assert!(app.world().get::<Unknown>(potion).is_some(), "{name} reads as unknown");
        use_thing(&mut app, me, potion);
        assert!(app.world().resource::<Lore>().knows(name), "{name} is known once drunk: {:#?}", said(&app));
        let line = format!("That was {}.", rl_engine::rl_core::noun::with_article(name));
        assert!(said(&app).contains(&line), "{name}: {:#?}", said(&app));
    }
}
