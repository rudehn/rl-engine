//! Each kind of weapon and armor in play: whom a blow strikes, how long it
//! takes, what it does to the unaware, what a shot spends, and how loud a
//! step is in what the adventurer wears.
//!
//! The armory's own tests read a thing as it is made; these play the run
//! that ships, the adventurer swinging at monsters made as a floor's own
//! are, so a weapon's line in `items.ron` is held to what it does.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ai::hearing::HearingStats;

use crate::testing::{arena, equip, give, lost, make, monster, settle, still};

/// `me` attacks `foe`, a blow beside them or a shot at one further off,
/// and the turns run until they may act again.
fn strike(app: &mut App, me: Entity, foe: Entity) {
    app.world_mut().write_message(Intent::new(me, Attack(foe)));
    settle(app);
}

/// An axe swings round the adventurer: every foe beside them is struck,
/// and an ally beside them and a foe a step further off are not. A war
/// axe swings the same, in both hands.
#[test]
fn an_axe_sweeps_every_foe_beside_the_adventurer_and_spares_an_ally() {
    for weapon in ["axe", "war axe"] {
        let (mut app, me, here) = arena(1);
        let foes = [(1, 0), (0, 1), (-1, -1)].map(|(dx, dy)| still(&mut app, "goblin", here.offset(dx, dy)));
        let friend = still(&mut app, "goblin", here.offset(1, 1));
        let side = app.world().resource::<Registries>().factions.expect("adventurer");
        app.world_mut().entity_mut(friend).insert(Faction(side));
        let far = still(&mut app, "goblin", here.offset(2, 0));
        let axe = make(&mut app, weapon);
        equip(&mut app, me, axe);
        strike(&mut app, me, foes[0]);
        let struck: Vec<bool> = foes.iter().map(|foe| lost(&app, *foe) > 0).collect();
        assert_eq!(struck, [true, true, true], "{weapon}: every foe beside the adventurer");
        assert_eq!((lost(&app, friend), lost(&app, far)), (0, 0), "{weapon}: not the ally beside them, nor the foe a step off");
    }
}

/// A spear runs on through the foe it is thrust at to the one standing
/// behind, and no further; a war pike the same, in both hands.
#[test]
fn a_spear_runs_on_through_its_target_to_the_one_behind_and_no_further() {
    for weapon in ["spear", "war pike"] {
        let (mut app, me, here) = arena(2);
        let line = [1, 2, 3].map(|dx| still(&mut app, "goblin", here.offset(dx, 0)));
        let spear = make(&mut app, weapon);
        equip(&mut app, me, spear);
        strike(&mut app, me, line[0]);
        let struck: Vec<bool> = line.iter().map(|foe| lost(&app, *foe) > 0).collect();
        assert_eq!(struck, [true, true, false], "{weapon}: the target and the one behind it, and not the one behind that");
    }
}

/// A mace's blow takes two of the adventurer's turns where a sword's
/// takes one: the clock stands two hundred further on before they act
/// again, and a hundred after a sword's. A war hammer is as slow.
#[test]
fn a_mace_takes_two_turns_a_blow_where_a_sword_takes_one() {
    for (weapon, took) in [("sword", 100), ("mace", 200), ("war hammer", 200)] {
        let (mut app, me, here) = arena(3);
        let foe = still(&mut app, "goblin", here.offset(1, 0));
        let held = make(&mut app, weapon);
        equip(&mut app, me, held);
        assert!(app.world().get::<MyTurn>(me).is_some(), "{weapon}: the adventurer holds the turn");
        let before = app.world().resource::<Turns>().now();
        strike(&mut app, me, foe);
        assert!(app.world().get::<MyTurn>(me).is_some(), "{weapon}: and holds it again");
        assert_eq!(app.world().resource::<Turns>().now() - before, took, "{weapon}");
    }
}

/// Of `each` fresh `victim`s on each of `seeds`' runs, none of which has
/// noticed the adventurer, how many died of one blow of `weapon`, and of
/// how many struck.
fn killed_unaware(weapon: &str, victim: &str, seeds: std::ops::Range<u64>, each: u32) -> (u32, u32) {
    let (mut killed, mut struck) = (0, 0);
    for seed in seeds {
        let (mut app, me, here) = arena(seed);
        let blade = make(&mut app, weapon);
        equip(&mut app, me, blade);
        for _ in 0..each {
            let foe = monster(&mut app, victim, here.offset(1, 0));
            let unaware = app.world_mut().run_system_once(move |watchers: Watchers| watchers.unaware(foe, me)).expect("the watchers");
            assert!(unaware, "seed {seed}: the {victim} has not noticed the adventurer yet");
            strike(&mut app, me, foe);
            struck += 1;
            // Dead, not only despawned: every monster now leaves remains
            // for `raising` to find, so a killed one still answers
            // `get_entity` as its own body, with its `Health` gone rather
            // than its whole entity, and `Dead` itself lasts only the one
            // pass between dying and being laid down as one.
            match app.world().get_entity(foe) {
                Ok(standing) if standing.get::<Health>().is_some() => {
                    app.world_mut().despawn(foe);
                    settle(&mut app);
                }
                _ => killed += 1,
            }
        }
    }
    (killed, struck)
}

/// The dagger's own five times on the unaware outranks the house's three:
/// a rat that has not noticed the adventurer dies of one stab every time,
/// and so does a goblin, which a sword's blow on the unaware leaves
/// standing about one time in four, over a span of seeds.
#[test]
fn a_dagger_kills_the_unaware_in_one_blow_far_more_often_than_a_sword_over_a_span_of_seeds() {
    let (rats, stabbed) = killed_unaware("dagger", "rat", 0..2, 8);
    assert_eq!(rats, stabbed, "every rat stabbed unaware dies of it");
    let (goblins, stabbed) = killed_unaware("dagger", "goblin", 0..4, 10);
    assert_eq!(goblins, stabbed, "and every goblin");
    let (goblins, cut) = killed_unaware("sword", "goblin", 0..4, 10);
    let standing = cut - goblins;
    assert!((cut / 8..=cut / 2).contains(&standing), "a sword's blow on the unaware leaves about one goblin in four standing: {standing} of {cut}");
}

/// What the steps made in a pass carried, and who heard them.
#[derive(Resource, Default)]
struct Noises {
    made: Vec<MakeNoise>,
    heard: Vec<NoiseHeard>,
}

fn listen(mut made: MessageReader<MakeNoise>, mut heard: MessageReader<NoiseHeard>, mut noises: ResMut<Noises>) {
    noises.made.extend(made.read().copied());
    noises.heard.extend(heard.read().copied());
}

/// `me` steps `dir`, and the turns run until they may act again.
fn step(app: &mut App, me: Entity, dir: Direction) {
    app.world_mut().write_message(Intent::new(me, Bump(dir)));
    settle(app);
}

/// The heavier the armor, the louder every step taken in it: leather adds
/// nothing to the two steps of loudness anyone's step carries, scale and
/// chain mail one, banded and splint mail two and plate three; and a
/// listener four steps off hears the adventurer walk in banded mail or
/// plate, and never in leather or chain.
#[test]
fn armor_steps_louder_the_heavier_and_a_listener_hears_plate_where_it_would_not_hear_leather() {
    let (mut app, me, here) = arena(4);
    let ear = still(&mut app, "rat", here.offset(5, 0));
    app.world_mut().entity_mut(ear).insert(Hearing(HearingStats { threshold: 0, memory: 6 }));
    app.init_resource::<Noises>().add_systems(PostUpdate, listen);
    let mut steps = Vec::new();
    for armor in ["leather armor", "scale mail", "chain mail", "banded mail", "splint mail", "plate armor"] {
        let coat = make(&mut app, armor);
        equip(&mut app, me, coat);
        *app.world_mut().resource_mut::<Noises>() = Noises::default();
        step(&mut app, me, Direction::East);
        let noises = app.world().resource::<Noises>();
        let mine: Vec<i32> = noises.made.iter().filter(|n| n.sound == Sounds::STEP && n.maker == Some(me)).map(|n| n.loudness).collect();
        let heard = noises.heard.iter().any(|h| h.listener == ear && h.maker == Some(me));
        steps.push((armor, mine, heard));
        step(&mut app, me, Direction::West);
    }
    let expected = [
        ("leather armor", vec![2], false),
        ("scale mail", vec![3], false),
        ("chain mail", vec![3], false),
        ("banded mail", vec![4], true),
        ("splint mail", vec![4], true),
        ("plate armor", vec![5], true),
    ];
    assert_eq!(steps, expected);
}

/// The bow spends an arrow a shot and is dry without: with none it never
/// draws, two arrows are two shots, and the third finds nothing to shoot.
#[test]
fn the_bow_spends_an_arrow_a_shot_and_is_dry_without() {
    let (mut app, me, here) = arena(5);
    let target = still(&mut app, "goblin", here.offset(3, 0));
    let bow = make(&mut app, "bow");
    equip(&mut app, me, bow);
    assert!(app.world().get::<Dry>(bow).is_some(), "no arrows, nothing to draw");
    let arrows = make(&mut app, "arrow");
    app.world_mut().get_mut::<Stack>(arrows).expect("arrows stack").count = 2;
    give(&mut app, me, arrows);
    settle(&mut app);
    assert!(app.world().get::<Dry>(bow).is_none(), "two arrows in the pack");
    strike(&mut app, me, target);
    let first = lost(&app, target);
    assert!(first > 0, "the first arrow struck");
    assert_eq!(app.world().get::<Stack>(arrows).map(|s| s.count), Some(1), "and was spent");
    strike(&mut app, me, target);
    let second = lost(&app, target);
    assert!(second > first, "the second struck");
    assert!(app.world().get::<Stack>(arrows).is_none_or(|s| s.count == 0), "and was the last");
    assert!(app.world().get::<Dry>(bow).is_some(), "so the bow is dry");
    strike(&mut app, me, target);
    assert_eq!(lost(&app, target), second, "and a third finds nothing to shoot");
}
