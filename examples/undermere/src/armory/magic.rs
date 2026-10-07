//! Each staff, wand and ring in play, and the effects they carry: what a
//! zap strikes and does, what it spends and when that comes back, what a
//! ring's level gives its wearer, and when each is plain enough to make
//! its kind known.
//!
//! As `play` does for the weapons, these play the run that ships: the
//! adventurer zapping at monsters made as a floor's own are, from things
//! made by the armory as a floor's own are, so a line in `items.ron` is
//! held to what it does.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::allies::{Ally, Fades};
use crate::bestiary::Kind;
use crate::lore::Lore;
use crate::testing::{arena, at, find, give, lost, monster, name_of, pass_turns, said, settle, still, use_thing};

/// `me` zaps `thing`, out of their pack, at `cell`, as the cursor's
/// confirm does, and the turns run until they may act again.
fn zap(app: &mut App, me: Entity, thing: Entity, cell: Point) {
    app.world_mut().write_message(Intent::new(me, UseItemAt { item: thing, at: cell }));
    settle(app);
}

/// What `thing` has left of its charges, and the most it holds.
fn charges(app: &App, thing: Entity) -> (u16, u16) {
    let c = app.world().get::<Consumable>(thing).expect("charges");
    (c.left, c.max)
}

/// `name` found at `level` and put in `me`'s pack.
fn carried(app: &mut App, me: Entity, name: &str, level: i32) -> Entity {
    let thing = find(app, name, level);
    give(app, me, thing);
    thing
}

/// A staff of firebolt zapped at a goblin burns it and the ground under
/// it, and spends one of its two charges; the charge comes back once its
/// recharge, five hundred turns over its level of two, has gone by, and
/// not before.
#[test]
fn a_staff_of_firebolt_zapped_at_a_goblin_burns_it_and_spends_a_charge_that_comes_back_over_its_recharge() {
    let (mut app, me, here) = arena(1);
    let goblin = still(&mut app, "goblin", here.offset(3, 0));
    let staff = carried(&mut app, me, "staff of firebolt", 2);
    assert_eq!(charges(&app, staff), (2, 2), "a staff found at two holds two");
    zap(&mut app, me, staff, here.offset(3, 0));
    assert!(lost(&app, goblin) > 0, "the bolt burns the goblin");
    assert!(app.world().resource::<Fire>().is_burning(here.offset(3, 0)), "and the ground under it");
    assert_eq!(charges(&app, staff), (1, 2), "one charge spent");
    pass_turns(&mut app, 240);
    assert_eq!(charges(&app, staff), (1, 2), "not back after two hundred and forty turns");
    pass_turns(&mut app, 12);
    assert_eq!(charges(&app, staff), (2, 2), "back after two hundred and fifty");
}

/// A goblin of `health` out of its ten, with its own mind, at `at`: a foe
/// that fights and moves, for a zap whose point is what it does next.
fn goblin(app: &mut App, at: Point, health: i32) -> Entity {
    let e = monster(app, "goblin", at);
    app.world_mut().entity_mut(e).insert(Health { current: health, max: 10 });
    e
}

/// A `name` at `at` with `health` out of its kind's ten, standing where it
/// is put with no mind to move it: a target for a zap whose point is what
/// it does to the target.
fn held(app: &mut App, name: &str, at: Point, health: i32) -> Entity {
    let e = monster(app, name, at);
    app.world_mut().entity_mut(e).remove::<Mind>().insert(Health { current: health, max: 10 });
    e
}

/// The faction called `name`.
fn side(app: &App, name: &str) -> Faction {
    Faction(app.world().resource::<Registries>().factions.expect(name))
}

/// A wand of domination zapped at a goblin beaten to a fifth of its health
/// binds it for certain: it takes the adventurer's side, follows them
/// when they walk off, and strikes a rat that comes at it.
#[test]
fn a_wand_of_domination_makes_a_beaten_goblin_follow_the_adventurer_and_fight_beside_them() {
    let (mut app, me, here) = arena(2);
    let foe = goblin(&mut app, here.offset(2, 0), 2);
    let wand = carried(&mut app, me, "wand of domination", 0);
    zap(&mut app, me, wand, here.offset(2, 0));
    assert!(app.world().get::<Ally>(foe).is_some(), "bound");
    assert_eq!(app.world().get::<Faction>(foe).copied(), Some(side(&app, "adventurer")), "on the adventurer's side");
    assert!(said(&app).iter().any(|l| l == "The goblin is bound to your will."), "{:#?}", said(&app));
    crate::testing::stand_on(&mut app, me, here.offset(-5, 0));
    pass_turns(&mut app, 8);
    let gap = geometry::chebyshev(at(&app, foe), at(&app, me));
    assert!(gap <= 3, "it followed, and stands {gap} off");
    let beside = at(&app, foe).offset(0, 1);
    let rat = still(&mut app, "rat", beside);
    pass_turns(&mut app, 4);
    assert!(lost(&app, rat) > 0, "and struck the rat that came at it");
}

/// The chance a foe is bound: certain at a quarter of its health or under,
/// and otherwise what it has lost, so a goblin at full health is never
/// bound and one at half is bound half the time.
#[test]
fn domination_is_certain_at_a_quarter_of_a_foes_health_and_otherwise_as_likely_as_it_is_hurt() {
    use crate::effects::minds::chance;
    let at = |current| chance(Health { current, max: 20 });
    assert_eq!([at(20), at(10), at(6), at(5), at(1)], [0, 50, 70, 100, 100]);
}

/// A wand of polymorph makes a goblin at half its health into some other
/// kind of monster, at half of that kind's health: over a span of seeds,
/// never a goblin and never at another share. The goblin is gone, and the
/// one thing alive on the ground besides the adventurer is what it became.
/// What it may become is every kind found anywhere but its own, and never
/// the king nor a blade, which are found nowhere.
#[test]
fn polymorph_keeps_the_share_of_health_and_never_gives_back_the_same_kind_over_a_span_of_seeds() {
    let library = crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default());
    let defs = &library.bestiary.defs;
    let shapes = crate::effects::bodies::shapes(&library.bestiary, defs.expect("goblin"));
    let named: Vec<&str> = shapes.iter().map(|k| defs.get(*k).name.as_str()).collect();
    for never in ["goblin", "the Drowned King", "spectral blade"] {
        assert!(!named.contains(&never), "never a {never}: {named:?}");
    }
    assert!(named.len() >= 10, "and anything else under the lake: {named:?}");
    for seed in 0..6 {
        let (mut app, me, here) = arena(10 + seed);
        let cell = here.offset(2, 0);
        let foe = held(&mut app, "goblin", cell, 5);
        let wand = carried(&mut app, me, "wand of polymorph", 0);
        zap(&mut app, me, wand, cell);
        let world = app.world_mut();
        let now: Vec<(Entity, String, Health)> = world
            .query_filtered::<(Entity, &Name, &Health), (With<Kind>, With<Position>)>()
            .iter(world)
            .map(|(e, n, h)| (e, n.as_str().to_string(), *h))
            .collect();
        let [(made, name, health)] = now.as_slice() else { panic!("seed {seed}: one thing alive: {now:?}") };
        assert_ne!(*made, foe, "seed {seed}: a new body");
        assert_ne!(name, "goblin", "seed {seed}: something other than a goblin");
        assert_eq!(health.current, (health.max + 1) / 2, "seed {seed}: half of a {name}'s {}", health.max);
    }
}

/// A wand of plenty zapped at an ally gives the adventurer a second ally
/// beside it, each with half the first's health; zapped at a foe, it makes
/// two foes the same way.
#[test]
fn plenty_on_an_ally_gives_a_second_ally_and_on_a_foe_a_second_foe_each_at_half_its_health() {
    let (mut app, me, here) = arena(3);
    held(&mut app, "goblin", here.offset(-2, 0), 8);
    let wand = carried(&mut app, me, "wand of plenty", 0);
    zap(&mut app, me, wand, here.offset(-2, 0));
    // Made an ally just before the zap, so it has had no turn to walk off.
    let friend = held(&mut app, "goblin", here.offset(2, 0), 10);
    crate::allies::befriend(app.world_mut(), friend);
    let wand = carried(&mut app, me, "wand of plenty", 0);
    zap(&mut app, me, wand, here.offset(2, 0));
    let world = app.world_mut();
    let mut goblins: Vec<(bool, i32)> =
        world.query::<(&Name, &Health, Has<Ally>)>().iter(world).filter(|(n, ..)| n.as_str() == "goblin").map(|(_, h, ally)| (ally, h.current)).collect();
    goblins.sort();
    assert_eq!(goblins, [(false, 4), (false, 4), (true, 5), (true, 5)], "two allies at five, two foes at four");
}

/// A staff of blinking a level one reaches four cells, two and two a
/// level: aimed five off down open ground, it sets the adventurer down on
/// the fourth, and aimed three off, on the third.
#[test]
fn a_staff_of_blinking_moves_its_user_down_the_line_as_far_as_its_level_reaches() {
    let (mut app, me, here) = arena(4);
    let staff = carried(&mut app, me, "staff of blinking", 1);
    zap(&mut app, me, staff, here.offset(5, 0));
    assert_eq!(at(&app, me), here.offset(4, 0), "four cells, and no further");
    let staff = carried(&mut app, me, "staff of blinking", 1);
    zap(&mut app, me, staff, here.offset(4, -3));
    assert_eq!(at(&app, me), here.offset(4, -3), "where it was aimed, when that is within reach");
}

/// A wand of teleport other sends the goblin it strikes far off, a wand
/// of beckoning pulls one from four cells off to the adventurer's side,
/// and each says what it does in its own words.
#[test]
fn teleport_other_sends_a_foe_far_off_and_beckoning_pulls_one_to_the_adventurers_side() {
    let (mut app, me, here) = arena(5);
    let far = held(&mut app, "goblin", here.offset(2, 0), 10);
    let wand = carried(&mut app, me, "wand of teleport other", 0);
    zap(&mut app, me, wand, here.offset(2, 0));
    let gone = geometry::chebyshev(at(&app, far), here);
    assert!(gone >= 10, "sent {gone} off");
    let near = held(&mut app, "goblin", here.offset(-4, 0), 10);
    let wand = carried(&mut app, me, "wand of beckoning", 0);
    zap(&mut app, me, wand, here.offset(-4, 0));
    assert_eq!(at(&app, near), here.offset(-1, 0), "pulled to the adventurer's side");
}

/// A staff of conjuration at level two calls up three spectral blades
/// beside the adventurer, on their side, each striking a level harder than
/// the kind's own blow, and fifteen turns later every one has faded.
#[test]
fn a_staff_of_conjuration_calls_up_a_blade_a_level_and_one_more_that_fade_after_fifteen_turns() {
    let (mut app, me, here) = arena(6);
    let staff = carried(&mut app, me, "staff of conjuration", 2);
    use_thing(&mut app, me, staff);
    let blades = |app: &mut App| {
        let world = app.world_mut();
        let mut found = world.query_filtered::<(&Name, &Position, &MeleeAttack, &Faction, Has<Fades>), With<Ally>>();
        found.iter(world).map(|(n, p, m, f, fades)| (n.as_str().to_string(), p.0, m.dice.bonus, *f, fades)).collect::<Vec<_>>()
    };
    let called = blades(&mut app);
    assert_eq!(called.len(), 3, "three blades: {called:?}");
    for (name, cell, bonus, faction, fades) in &called {
        assert_eq!((name.as_str(), *bonus, *faction, *fades), ("spectral blade", 2, side(&app, "adventurer"), true));
        assert!(geometry::chebyshev(*cell, here) <= 4, "by the adventurer: {cell:?}");
    }
    pass_turns(&mut app, 16);
    assert!(blades(&mut app).is_empty(), "every one faded");
    assert!(said(&app).iter().any(|l| l == "The spectral blade fades away."), "{:#?}", said(&app));
}

/// The staffs and wands, by name, and whether each is zapped at somewhere
/// rather than used where its user stands.
const ZAPPED: [(&str, bool); 11] = [
    ("staff of firebolt", true),
    ("staff of lightning", true),
    ("staff of poison", true),
    ("staff of slowness", true),
    ("staff of blinking", true),
    ("staff of conjuration", false),
    ("wand of domination", true),
    ("wand of teleport other", true),
    ("wand of polymorph", true),
    ("wand of beckoning", true),
    ("wand of plenty", true),
];

/// Every staff and wand is found unknown, called by its face, and is known
/// once zapped at a goblin, or used, for the one that is not aimed: what
/// each does to whoever it strikes is plain to see, and a blink is plain
/// to whoever it moves. Each says so in the log in its true name.
#[test]
fn every_staff_and_wand_is_found_unknown_under_its_face_and_known_once_zapped_at_a_goblin() {
    for (name, aimed) in ZAPPED {
        let (mut app, me, here) = arena(7);
        let thing = carried(&mut app, me, name, 2);
        settle(&mut app);
        let face = app.world().resource::<Lore>().face(name).unwrap_or_else(|| panic!("{name} wears a face")).called.clone();
        assert_eq!(name_of(&app, thing), face, "{name} is called by its face");
        assert!(app.world().get::<Unknown>(thing).is_some(), "{name} reads as unknown");
        held(&mut app, "goblin", here.offset(3, 0), 10);
        match aimed {
            true => zap(&mut app, me, thing, here.offset(if name == "staff of blinking" { -3 } else { 3 }, 0)),
            false => use_thing(&mut app, me, thing),
        }
        assert!(app.world().resource::<Lore>().knows(name), "{name} is known once used: {:#?}", said(&app));
        let line = format!("That was {}.", rl_engine::rl_core::noun::with_article(name));
        assert!(said(&app).contains(&line), "{name}: {:#?}", said(&app));
    }
}

/// A wand of polymorph zapped at bare floor strikes nobody and shows
/// nothing: its charge is spent, the log says nothing seemed to happen,
/// and the wand is as unknown as it was. A staff of firebolt zapped there
/// sets the floor alight, which shows what it is.
#[test]
fn a_zap_that_strikes_nobody_shows_nothing_and_teaches_nothing_unless_it_burns_the_ground() {
    let (mut app, me, here) = arena(8);
    let wand = carried(&mut app, me, "wand of polymorph", 0);
    zap(&mut app, me, wand, here.offset(3, 0));
    let lore = app.world().resource::<Lore>().clone();
    assert!(!lore.knows("wand of polymorph"), "still unknown");
    assert!(said(&app).iter().any(|l| l == "Nothing seems to happen."), "{:#?}", said(&app));
    assert_eq!(charges(&app, wand), (2, 3), "and the charge spent");
    let staff = carried(&mut app, me, "staff of firebolt", 2);
    zap(&mut app, me, staff, here.offset(-3, 0));
    assert!(app.world().resource::<Lore>().knows("staff of firebolt"), "fire on bare floor is plain");
}

/// Whether `name` is known, and whether the log said nothing seemed to
/// happen.
fn learned(app: &App, name: &str) -> (bool, bool) {
    (app.world().resource::<Lore>().knows(name), said(app).iter().any(|l| l == "Nothing seems to happen."))
}

/// A wand of plenty zapped at a goblin walled in on every side but the
/// adventurer's has nowhere to put a second, and a wand of beckoning zapped
/// at a goblin already beside the adventurer pulls it nowhere: each shows
/// nothing, says so, and is as unknown as it was.
#[test]
fn plenty_with_no_room_and_beckoning_at_a_foe_beside_you_show_nothing_and_teach_nothing() {
    for name in ["wand of plenty", "wand of beckoning"] {
        let (mut app, me, here) = arena(17);
        let cell = here.offset(1, 0);
        let walls: Vec<Point> = geometry::square(cell, 1).filter(|p| *p != cell && *p != here).collect();
        crate::testing::wall_off(&mut app, &walls);
        let goblin = held(&mut app, "goblin", cell, 10);
        let wand = carried(&mut app, me, name, 0);
        zap(&mut app, me, wand, cell);
        assert_eq!(learned(&app, name), (false, true), "{name}: {:#?}", said(&app));
        assert_eq!(at(&app, goblin), cell, "{name}: the goblin is where it was");
        let world = app.world_mut();
        assert_eq!(world.query_filtered::<(), With<Kind>>().iter(world).count(), 1, "{name}: and alone");
    }
}

/// With something watching, a zap is seen to fly and comes down the pass
/// after it was made, and what it showed is weighed then: a staff of
/// firebolt at a goblin is not known until the bolt lands, and is then;
/// a wand of beckoning at a goblin beside the adventurer, which the bolt
/// shows to pull nowhere as it lands, and a staff of poison at bare floor,
/// weighed as striking nobody when it was made, are not known even once
/// they have landed, and the log says nothing seemed to happen.
#[test]
fn a_zap_seen_to_fly_is_known_by_what_it_showed_when_it_lands() {
    let zaps = [
        ("staff of firebolt", Point::new(3, 0), true, true),
        ("wand of beckoning", Point::new(1, 0), true, false),
        ("staff of poison", Point::new(3, 0), false, false),
    ];
    for (name, cell, goblin, plain) in zaps {
        let (mut app, me, here) = arena(18);
        let cell = here.offset(cell.x, cell.y);
        if goblin {
            held(&mut app, "goblin", cell, 10);
        }
        let thing = carried(&mut app, me, name, 2);
        app.world_mut().resource_mut::<TurnHold>().watch();
        zap(&mut app, me, thing, cell);
        assert!(app.world().resource::<TurnHold>().in_flight(), "{name}: in the air");
        assert!(!app.world().resource::<Lore>().knows(name), "{name}: not known while it flies");
        for _ in 0..4 {
            app.world_mut().resource_mut::<TurnHold>().release();
            app.update();
        }
        assert!(!app.world().resource::<TurnHold>().in_flight(), "{name}: landed");
        assert_eq!(learned(&app, name), (plain, !plain), "{name}: {:#?}", said(&app));
    }
}

/// Each of Undermere's own effects refuses an argument it does not name,
/// and one that takes arguments refuses being written without them, so a
/// typo in `items.ron` stops the game at load rather than doing nothing.
#[test]
fn the_games_own_effects_refuse_an_argument_they_do_not_name() {
    let kinds = crate::content::effect_kinds();
    let registries = crate::content::registries();
    let build = |kind: &str, args: &str| {
        let args = rl_engine::rl_rules::ability::parse_args(args).expect("ron");
        kinds.build(kind, &args, &registries.names()).map(|_| ())
    };
    for kind in ["TeleportOther", "Beckon", "Polymorph", "Plenty", "Dominate"] {
        assert!(build(kind, "()").is_ok(), "{kind} as written");
        assert!(build(kind, "(cells: 2)").is_err(), "{kind} with an argument it does not name");
    }
    assert!(build("Blink", "(reach: 2, per_level: 2)").is_ok(), "a blink as written");
    assert!(build("Blink", "(reach: 2)").is_err(), "a blink with no per_level");
    assert!(build("Conjure", "(blades: 1, turns: 15)").is_ok(), "a conjuring as written");
    assert!(build("Conjure", "(blades: 1, turns: 15, sharp: 2)").is_err(), "a conjuring with a word it does not read");
}

/// A goblin bound to the adventurer and the blades they conjured are
/// carried on by a continued run: the goblin still on their side, and
/// each blade still to fade at the moment it was going to.
#[test]
fn a_continued_run_keeps_a_bound_goblin_on_the_adventurers_side_and_a_conjured_blade_fading() {
    use crate::save::SLOT;
    let (mut app, me, here) = arena(9);
    goblin(&mut app, here.offset(2, 0), 2);
    let wand = carried(&mut app, me, "wand of domination", 0);
    zap(&mut app, me, wand, here.offset(2, 0));
    let staff = carried(&mut app, me, "staff of conjuration", 1);
    use_thing(&mut app, me, staff);
    let allies = |app: &mut App| {
        let world = app.world_mut();
        let mut found = world.query_filtered::<(&Name, &Faction, Option<&Fades>), (With<Ally>, With<Kind>, With<Position>)>();
        let mut list: Vec<(String, Faction, Option<u32>)> = found.iter(world).map(|(n, f, fades)| (n.as_str().to_string(), *f, fades.map(|f| f.at))).collect();
        list.sort_by_key(|(name, _, at)| (name.clone(), *at));
        list
    };
    let before = allies(&mut app);
    assert_eq!(before.len(), 3, "the goblin and two blades: {before:?}");
    app.world().resource::<Saves>().delete(SLOT).expect("the slot can be emptied");
    crate::testing::arrive(&mut app, crate::dungeon::map_of(1));
    let text = app.world().resource::<Saves>().load(SLOT).expect("the slot reads").expect("the arrival wrote the run down");
    let mut carried_on = crate::testing::continued(&text);
    assert_eq!(allies(&mut carried_on), before, "each on the adventurer's side, the blades to fade when they were going to");
}

/// Every ring is found unknown, called by its stone, and known the moment
/// it is put on, its work plain on its wearer, with a line in the log in
/// its true name.
#[test]
fn every_ring_is_found_unknown_under_its_stone_and_known_once_put_on() {
    let rings = ["ring of regeneration", "ring of stealth", "ring of light", "ring of wisdom", "ring of reaping", "ring of awareness"];
    for name in rings {
        let (mut app, me, _) = arena(16);
        let ring = find(&mut app, name, 1);
        give(&mut app, me, ring);
        settle(&mut app);
        let face = app.world().resource::<Lore>().face(name).unwrap_or_else(|| panic!("{name} wears a stone")).called.clone();
        assert_eq!(name_of(&app, ring), face, "{name} is called by its stone");
        assert!(app.world().get::<Unknown>(ring).is_some(), "{name} reads as unknown");
        crate::testing::equip(&mut app, me, ring);
        assert!(app.world().resource::<Lore>().knows(name), "{name} is known once on: {:#?}", said(&app));
        let line = format!("It is {}.", rl_engine::rl_core::noun::with_article(name));
        assert!(said(&app).contains(&line), "{name}: {:#?}", said(&app));
    }
}

/// `name` found at `level` and put on, by `Equip`, as the pack's key does.
fn worn(app: &mut App, me: Entity, name: &str, level: i32) -> Entity {
    let ring = find(app, name, level);
    crate::testing::equip(app, me, ring);
    ring
}

/// The hundredths a charge of `staff` takes to come back, as the engine
/// counts it this pass.
fn pace(app: &App, staff: Entity) -> u32 {
    app.world().get::<Consumable>(staff).and_then(|c| c.recharge).expect("a staff refills").every
}

/// A ring of wisdom a level quickens every staff its wearer carries by
/// three tenths: a staff of firebolt at two refills in a hundred and
/// ninety-three turns rather than two hundred and fifty, and at its own
/// pace again once the ring is off. A ring of reaping two better puts
/// fifty turns towards the next charge for every blow that lands, so ten
/// blows at a goblin bring back a staff's charge that would have taken
/// five hundred turns.
#[test]
fn a_ring_of_wisdom_quickens_a_staffs_recharge_and_a_ring_of_reapings_blows_shorten_it() {
    let (mut app, me, here) = arena(11);
    let staff = carried(&mut app, me, "staff of firebolt", 2);
    still(&mut app, "goblin", here.offset(3, 0));
    zap(&mut app, me, staff, here.offset(3, 0));
    let wisdom = worn(&mut app, me, "ring of wisdom", 1);
    assert_eq!(pace(&app, staff), 25_000 * 100 / 130, "three tenths faster");
    pass_turns(&mut app, 195);
    assert_eq!(charges(&app, staff), (2, 2), "back inside two hundred turns");
    crate::testing::unequip(&mut app, me, wisdom);
    pass_turns(&mut app, 1);
    assert_eq!(pace(&app, staff), 25_000, "and at its own pace with the ring off");

    let (mut app, me, here) = arena(12);
    let staff = carried(&mut app, me, "staff of firebolt", 1);
    let goblin = still(&mut app, "goblin", here.offset(1, 0));
    zap(&mut app, me, staff, here.offset(1, 0));
    worn(&mut app, me, "ring of reaping", 2);
    assert_eq!(charges(&app, staff), (0, 1), "spent");
    for _ in 0..10 {
        app.world_mut().write_message(Intent::new(me, Attack(goblin)));
        settle(&mut app);
    }
    pass_turns(&mut app, 1);
    assert_eq!(charges(&app, staff), (1, 1), "ten blows brought back what five hundred turns would have");
}

/// A ring of regeneration a level gives back ten health a hundred turns,
/// and three levels thirty, where nothing else heals by itself; one cursed
/// two below nought gives back none, and takes half off every heal.
#[test]
fn a_ring_of_regeneration_gives_ten_health_back_a_hundred_turns_a_level_and_a_cursed_one_halves_every_heal() {
    for (level, turns, back) in [(1, 100, 10), (3, 50, 15), (-2, 100, 0)] {
        let (mut app, me, _) = arena(13);
        worn(&mut app, me, "ring of regeneration", level);
        app.world_mut().entity_mut(me).insert(Health { current: 10, max: 40 });
        pass_turns(&mut app, turns);
        let now = app.world().get::<Health>(me).expect("health").current;
        assert!((10 + back - 1..=10 + back).contains(&now), "a {level:+} ring, {turns} turns: {now} of 40");
    }
    let (mut app, me, _) = arena(14);
    worn(&mut app, me, "ring of regeneration", -2);
    app.world_mut().entity_mut(me).insert(Health { current: 10, max: 40 });
    app.world_mut().write_message(Heal::new(me, 20));
    settle(&mut app);
    assert_eq!(app.world().get::<Health>(me).map(|h| h.current), Some(20), "half of a heal of twenty");
}

/// A ring of awareness two better adds thirty to the chance a turn of
/// spotting a hidden thing, on top of what its definition gives, and takes
/// it away again when it comes off.
#[test]
fn a_ring_of_awareness_puts_fifteen_a_level_on_the_chance_of_spotting_what_is_hidden() {
    let (mut app, me, _) = arena(15);
    let barrel = app.world().resource::<Registries>().props.expect("barrel");
    let secret = app.world_mut().spawn((PropKind(barrel), Hidden { spot: 0 })).id();
    let ring = worn(&mut app, me, "ring of awareness", 2);
    pass_turns(&mut app, 1);
    assert_eq!(app.world().get::<Hidden>(secret).map(|h| h.spot), Some(30), "thirty a turn");
    crate::testing::unequip(&mut app, me, ring);
    pass_turns(&mut app, 1);
    assert_eq!(app.world().get::<Hidden>(secret).map(|h| h.spot), Some(0), "and the barrel's own nought without it");
}
