//! Each scroll in play: what reading it does where its reader stands, and
//! when it is plain enough to make its kind known.
//!
//! As `magic` and `draughts` do, these play the run that ships, the
//! adventurer reading things made by the armory as a floor's own are.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::content::DISCORDANT;
use crate::effects::FAR;
use crate::lore::Lore;
use crate::testing::{arena, arena_in_mereholt, arena_on, at, find, give, monster, name_of, pass_turns, read_on, said, settle, use_thing};

/// `name` found at `level` and put in `me`'s pack.
fn carried(app: &mut App, me: Entity, name: &str, level: i32) -> Entity {
    let thing = find(app, name, level);
    give(app, me, thing);
    thing
}

/// What `thing` has left of its charges, and the most it holds.
fn charges(app: &App, thing: Entity) -> (u16, u16) {
    let c = app.world().get::<Consumable>(thing).expect("charges");
    (c.left, c.max)
}

/// A goblin at `at` that stands where it is put, to be seen and not to
/// fight.
fn still_goblin(app: &mut App, at: Point) -> Entity {
    let e = monster(app, "goblin", at);
    app.world_mut().entity_mut(e).remove::<Mind>();
    e
}

/// A scroll of teleportation sends its reader to a free cell of the floor
/// at least twenty steps from where they stood, over a span of seeds.
#[test]
fn teleportation_sends_its_reader_far_from_where_they_stood_over_a_span_of_seeds() {
    for seed in 0..4 {
        let (mut app, me, here) = arena_on(50 + seed, 2);
        let scroll = carried(&mut app, me, "scroll of teleportation", 0);
        use_thing(&mut app, me, scroll);
        let gone = geometry::chebyshev(at(&app, me), here);
        assert!(gone >= FAR, "seed {seed}: sent {gone} off");
        assert!(app.world().resource::<WorldMap>().is_walkable(at(&app, me)), "seed {seed}: onto ground");
    }
}

/// A scroll of recharging fills every staff its reader carries and puts a
/// charge in every wand; read with neither in the pack, nothing seems to
/// happen and the scroll is as unknown as it was.
#[test]
fn recharging_fills_every_staff_and_adds_a_charge_to_every_wand_and_with_neither_shows_nothing() {
    let (mut app, me, _) = arena(51);
    let scroll = carried(&mut app, me, "scroll of recharging", 0);
    use_thing(&mut app, me, scroll);
    assert!(said(&app).iter().any(|l| l == "Nothing seems to happen."), "{:#?}", said(&app));
    assert!(!app.world().resource::<Lore>().knows("scroll of recharging"), "unknown still");
    let staff = carried(&mut app, me, "staff of firebolt", 2);
    let wand = carried(&mut app, me, "wand of beckoning", 0);
    for thing in [staff, wand] {
        app.world_mut().get_mut::<Consumable>(thing).expect("charges").left = 0;
    }
    let scroll = carried(&mut app, me, "scroll of recharging", 0);
    use_thing(&mut app, me, scroll);
    assert_eq!((charges(&app, staff), charges(&app, wand)), ((2, 2), (1, 2)), "the staff full, the wand a charge more");
    assert!(app.world().resource::<Lore>().knows("scroll of recharging"), "and known by it");
}

/// A scroll of magic mapping makes known every cell of the floor that can
/// be walked, seen or not, and the way down with it.
#[test]
fn magic_mapping_makes_the_whole_floor_known() {
    let (mut app, me, _) = arena_on(52, 1);
    let unknown = |app: &App| {
        let (map, known) = (app.world().resource::<WorldMap>(), app.world().resource::<Knowledge>());
        map.window_tiles().cells().filter(|p| map.is_walkable(*p) && !known.is_explored(*p)).count()
    };
    assert!(unknown(&app) > 0, "a floor just come down to is mostly unknown");
    let scroll = carried(&mut app, me, "scroll of magic mapping", 0);
    use_thing(&mut app, me, scroll);
    assert_eq!(unknown(&app), 0, "and once mapped, none of it is");
}

/// A scroll of discord read with two goblins in sight sets them on each
/// other, and on the adventurer, for ten turns: each on the discordant
/// side, each struck by the other; and after, back on the goblins' side.
#[test]
fn discord_sets_the_foes_in_sight_on_each_other_for_ten_turns_and_then_lets_them_go() {
    let (mut app, me, here) = arena_on(53, 1);
    let pair = [here.offset(3, 0), here.offset(4, 0)].map(|cell| {
        let e = monster(&mut app, "goblin", cell);
        app.world_mut().entity_mut(e).insert(Health::full(500));
        e
    });
    settle(&mut app);
    let scroll = carried(&mut app, me, "scroll of discord", 0);
    use_thing(&mut app, me, scroll);
    let side = |app: &App, e: Entity| app.world().get::<Faction>(e).map(|f| app.world().resource::<Registries>().factions.name(f.0).to_string());
    for goblin in pair {
        assert_eq!(side(&app, goblin).as_deref(), Some(DISCORDANT), "turned");
    }
    pass_turns(&mut app, 6);
    let hurt = pair.iter().filter(|g| app.world().get::<Health>(**g).is_some_and(|h| h.current < 500)).count();
    assert!(hurt >= 1, "they fought each other");
    pass_turns(&mut app, 8);
    for goblin in pair {
        assert_eq!(side(&app, goblin).as_deref(), Some("goblins"), "and are goblins again");
    }
}

/// A scroll of aggravate monsters makes every monster on the floor aware
/// of its reader, wherever it is, asleep or not.
#[test]
fn aggravate_monsters_makes_every_monster_on_the_floor_aware_of_the_reader() {
    let (mut app, me, here) = arena_on(54, 1);
    let far = still_goblin(&mut app, here.offset(5, 5));
    let world = app.world_mut();
    let floor: Vec<Entity> = world.query_filtered::<Entity, (With<crate::bestiary::Kind>, With<Aware>)>().iter(world).collect();
    assert!(!floor.is_empty(), "somebody to wake");
    let scroll = carried(&mut app, me, "scroll of aggravate monsters", 0);
    use_thing(&mut app, me, scroll);
    let knows = |app: &App, e: Entity| app.world().get::<Aware>(e).is_some_and(|a| a.knows(me));
    assert!(knows(&app, far), "the one nearby");
    let world = app.world_mut();
    let everyone: Vec<Entity> = world.query_filtered::<Entity, (With<crate::bestiary::Kind>, With<Aware>, With<Position>)>().iter(world).collect();
    assert!(everyone.iter().all(|e| knows(&app, *e)), "and every one on the floor");
}

/// A scroll of summon monsters calls up two or three of the floor's own
/// monsters beside its reader, each aware of them, over a span of seeds;
/// read in Mereholt, where none is ever found, nothing seems to happen.
#[test]
fn summon_monsters_calls_up_two_or_three_of_the_floors_monsters_beside_the_reader_over_a_span_of_seeds() {
    for seed in 0..4 {
        let (mut app, me, here) = arena_on(57 + seed, 1);
        let scroll = carried(&mut app, me, "scroll of summon monsters", 0);
        use_thing(&mut app, me, scroll);
        let world = app.world_mut();
        let came: Vec<(Point, bool)> = world
            .query_filtered::<(&Position, &Aware), With<crate::bestiary::Kind>>()
            .iter(world)
            .filter(|(p, _)| geometry::chebyshev(p.0, here) <= 2)
            .map(|(p, aware)| (p.0, aware.knows(me)))
            .collect();
        assert!((2..=3).contains(&came.len()), "seed {seed}: two or three beside the reader: {came:?}");
        assert!(came.iter().all(|(_, knows)| *knows), "seed {seed}: each knowing where they are");
    }
    let (mut app, me, _) = arena_in_mereholt(59);
    let scroll = carried(&mut app, me, "scroll of summon monsters", 0);
    use_thing(&mut app, me, scroll);
    assert!(said(&app).iter().any(|l| l == "Nothing seems to happen."), "{:#?}", said(&app));
}

/// What a scroll of summon monsters calls up knows where its reader is the
/// moment it comes, before it has had a turn to notice them itself.
#[test]
fn what_is_summoned_knows_where_its_reader_is_before_its_first_turn() {
    let (mut app, me, _) = arena_on(56, 1);
    let came = crate::effects::bodies::summon(app.world_mut(), me, 3);
    assert_eq!(came, 3);
    let world = app.world_mut();
    let knowing: Vec<bool> = world.query_filtered::<&Aware, With<crate::bestiary::Kind>>().iter(world).map(|a| a.knows(me)).collect();
    assert_eq!(knowing, [true; 3], "every one");
}

/// A scroll of discord read with no foe in sight does nothing anybody can
/// see: the log says so, and the scroll is as unknown as it was.
#[test]
fn discord_with_no_foe_in_sight_shows_nothing() {
    let (mut app, me, _) = arena_on(57, 1);
    let scroll = carried(&mut app, me, "scroll of discord", 0);
    use_thing(&mut app, me, scroll);
    assert!(said(&app).iter().any(|l| l == "Nothing seems to happen."), "{:#?}", said(&app));
    assert!(!app.world().resource::<Lore>().knows("scroll of discord"), "unknown still");
}

/// Every scroll is found unknown under a title of its own and is known
/// once read where what it does is plain: on floor one, beside a goblin to
/// set at odds and carrying a staff to refill, and a scroll that asks read
/// on the sword. Each says so in the log in its true name.
#[test]
fn every_scroll_is_found_unknown_under_its_title_and_known_once_read_where_it_is_plain() {
    let scrolls = [
        "scroll of enchanting",
        "scroll of identify",
        "scroll of teleportation",
        "scroll of remove curse",
        "scroll of recharging",
        "scroll of magic mapping",
        "scroll of discord",
        "scroll of aggravate monsters",
        "scroll of summon monsters",
    ];
    for name in scrolls {
        let (mut app, me, here) = arena_on(60, 1);
        still_goblin(&mut app, here.offset(3, 0));
        let staff = carried(&mut app, me, "staff of firebolt", 2);
        app.world_mut().get_mut::<Consumable>(staff).expect("charges").left = 0;
        let sword = carried(&mut app, me, "sword", 0);
        let scroll = carried(&mut app, me, name, 0);
        settle(&mut app);
        let face = app.world().resource::<Lore>().face(name).unwrap_or_else(|| panic!("{name} wears a title")).called.clone();
        assert!(face.starts_with("scroll titled "), "{name}: {face}");
        assert_eq!(name_of(&app, scroll), face, "{name} is called by its title");
        assert!(app.world().get::<Unknown>(scroll).is_some(), "{name} reads as unknown");
        let asks = app.world().get::<Triggers>(scroll).is_some_and(|t| t.asks().is_some());
        match asks {
            true => read_on(&mut app, me, scroll, sword),
            false => use_thing(&mut app, me, scroll),
        }
        assert!(app.world().resource::<Lore>().knows(name), "{name} is known once read: {:#?}", said(&app));
        let line = format!("That was {}.", rl_engine::rl_core::noun::with_article(name));
        assert!(said(&app).contains(&line), "{name}: {:#?}", said(&app));
    }
}

/// Two scrolls of a kind share a row of the pack, said as several under
/// their title, as two potions of a colour do: the engine keeps a title as
/// written and counts the word before it, `2 scrolls titled ...`.
#[test]
fn two_scrolls_of_a_kind_share_a_row_of_the_pack_under_their_title() {
    let (mut app, me, here) = arena(3);
    for _ in 0..2 {
        let scroll = find(&mut app, "scroll of identify", 0);
        crate::testing::lay(&mut app, scroll, here);
        crate::testing::take_up(&mut app, me);
    }
    let face = app.world().resource::<Lore>().face("scroll of identify").expect("a title").called.clone();
    let title = face.strip_prefix("scroll titled ").expect("titled");
    let rows = crate::testing::pack(&app, me);
    let titled: Vec<&(String, u32)> = rows.iter().filter(|(name, _)| *name == face).collect();
    assert_eq!(titled, [&(face.clone(), 2)], "one row of two: {rows:?}");
    assert_eq!(rl_engine::rl_core::noun::listed(&face, 2), format!("2 scrolls titled {title}"), "said as several under the title as written");
}
