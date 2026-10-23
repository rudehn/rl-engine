//! Each mechanism in its own test, on a floor a run lays its piece on.

use super::lifts::*;
use super::*;
use crate::dungeon::{drawn, drawn_from, map_of};
use crate::testing::{adventurer, arrive, at, clear_monsters, give, headless, make, pass_turns, settle, stand_on, sturdy, take_up};

/// A run that draws `piece`, its adventurer standing on the floor it lays
/// it, sturdy and alone: the floor's own monsters cleared, so what a test
/// reads is what the piece did.
fn on_the_floor_of(piece: &str) -> (App, Entity, MapId) {
    let (seed, floor) = drawn(&[piece]);
    let mut app = headless(seed);
    let me = adventurer(&mut app);
    sturdy(&mut app, me);
    let map = map_of(floor);
    arrive(&mut app, map);
    clear_monsters(&mut app);
    (app, me, map)
}

/// Every prop called `name` on `map`, with where it stands, in row order.
fn props_called(app: &mut App, map: MapId, name: &str) -> Vec<(Entity, Point)> {
    let id = app.world().resource::<Registries>().props.expect(name);
    let world = app.world_mut();
    let mut found: Vec<(Entity, Point)> = world
        .query::<(Entity, &PropKind, &Position, &OnMap)>()
        .iter(world)
        .filter(|(_, k, _, on)| k.0 == id && on.0 == map)
        .map(|(e, _, p, _)| (e, p.0))
        .collect();
    found.sort_by_key(|(_, p)| (p.y, p.x));
    found
}

/// A cell beside `of` a body may stand on, nobody standing there.
fn beside(app: &App, of: Point) -> Point {
    let (map, occupancy) = (app.world().resource::<WorldMap>(), app.world().resource::<Occupancy>());
    Direction::ALL.iter().map(|d| of + d.offset()).find(|p| map.is_walkable(*p) && !occupancy.is_occupied(*p)).expect("a free cell beside it")
}

/// The way from `from` to the cell `to` beside it.
fn towards(from: Point, to: Point) -> Direction {
    *Direction::ALL.iter().find(|d| from + d.offset() == to).expect("beside it")
}

/// Opens the container `prop` from beside it and takes all it holds, a
/// turn each, as the player would.
fn open_and_take(app: &mut App, me: Entity, prop: Entity) {
    let open = app.world().resource::<Verbs>().get("open").expect("the engine's own");
    app.world_mut().write_message(Intent::new(me, Interact { target: prop, verb: open, thing: None }));
    settle(app);
    app.world_mut().write_message(Intent::new(me, Take { from: prop, item: None }));
    settle(app);
    settle(app);
}

/// The name of the tile at `p` on `map`.
fn tile_at(app: &App, map: MapId, p: Point) -> String {
    let world_map = app.world().resource::<WorldMap>();
    let t = world_map.place(map).and_then(|place| place.terrain.get(p)).expect("on the map");
    world_map.tables().names[t.index()].clone()
}

/// Opening one of Caged Altars' three cages and taking what it holds shuts
/// the other two for good, on what they hold, and leaves the one opened
/// empty: the choice of three is a choice of one.
#[test]
fn taking_one_caged_thing_shuts_the_other_cages() {
    let (mut app, me, map) = on_the_floor_of("Caged Altars");
    let cages: Vec<(Entity, Point)> = CAGES.iter().flat_map(|c| props_called(&mut app, map, c)).collect();
    assert_eq!(cages.len(), 3, "three cages, one of each");
    let (cage, there) = cages[0];
    let next = beside(&app, there);
    stand_on(&mut app, me, next);
    open_and_take(&mut app, me, cage);
    assert!(!app.world().get::<Inventory>(me).expect("a pack").items.is_empty(), "the taker holds what the cage held");
    let shut: Vec<Point> = props_called(&mut app, map, SHUT_CAGE).into_iter().map(|(_, p)| p).collect();
    let mut others: Vec<Point> = cages[1..].iter().map(|(_, p)| *p).collect();
    others.sort_by_key(|p| (p.y, p.x));
    assert_eq!(shut, others, "the other two shut where they stood");
    assert_eq!(props_called(&mut app, map, EMPTY_CAGE).into_iter().map(|(_, p)| p).collect::<Vec<_>>(), vec![there], "the one opened stands empty");
    assert!(CAGES.iter().all(|c| props_called(&mut app, map, c).is_empty()), "and no cage is left to open");
}

/// Lifting a flooding vault's prize floods its room: wading-deep at once at
/// its grates, and too deep to cross at the wall farthest from its door
/// within the vault's own turns.
#[test]
fn the_flood_reaches_deep_water_at_the_vaults_far_wall_within_its_turns() {
    let (mut app, me, map) = on_the_floor_of("The Flooding Vault");
    let (altar, there) = props_called(&mut app, map, FLOOD_ALTAR)[0];
    let grates: Vec<Point> = props_called(&mut app, map, GRATE).into_iter().map(|(_, p)| p).collect();
    let plan = flood_plan(app.world().resource::<WorldMap>(), there, &grates);
    let far = plan.iter().filter(|c| c.deep.is_some()).map(|c| c.at).collect::<Vec<_>>();
    assert!(!far.is_empty() && far.len() < plan.len(), "a far end and a near one");
    let next = beside(&app, there);
    stand_on(&mut app, me, next);
    open_and_take(&mut app, me, altar);
    assert!(grates.iter().any(|g| tile_at(&app, map, *g) == "flooded floor"), "the water comes up through the grates at once");
    assert!(far.iter().all(|p| tile_at(&app, map, *p) != "deep water"), "and is not deep yet");
    pass_turns(&mut app, FLOOD_TURNS + 1);
    for p in &far {
        assert_eq!(tile_at(&app, map, *p), "deep water", "{p:?}, at the far wall, is too deep to cross");
    }
    let near: Vec<&Point> = plan.iter().filter(|c| c.deep.is_none()).map(|c| &c.at).collect();
    assert!(near.iter().all(|p| tile_at(&app, map, **p) == "flooded floor"), "and the way back to the door is waded");
}

/// Lifting a rotten floor's prize brings its planks down, and its taker,
/// standing on them, lands on an open cell of the floor below, never on
/// its stairs.
#[test]
fn the_rotten_floor_drops_its_taker_onto_an_open_cell_of_the_floor_below_off_its_stairs() {
    let (mut app, me, map) = on_the_floor_of("The Rotten Floor");
    let (plinth, there) = props_called(&mut app, map, ROTTEN_PLINTH)[0];
    let next = beside(&app, there);
    assert_eq!(tile_at(&app, map, next), "rotten planks", "the taker stands on the planks");
    stand_on(&mut app, me, next);
    open_and_take(&mut app, me, plinth);
    settle(&mut app);
    let below = map_of(crate::dungeon::floor_of(map).expect("a floor") + 1);
    assert_eq!(app.world().get::<OnMap>(me).map(|m| m.0), Some(below), "fallen to the floor below");
    let landed = at(&app, me);
    let world_map = app.world().resource::<WorldMap>();
    let place = world_map.place(below).expect("built by the fall");
    assert!(world_map.is_walkable_on(below, landed), "on open ground");
    assert!(landed != place.entry && Some(landed) != place.exit, "and off the stairs, at {landed:?}");
    assert_eq!(tile_at(&app, map, next), "hole", "the planks fell in behind");
    assert!(!app.world().get::<Inventory>(me).expect("a pack").items.is_empty(), "holding what the plinth held");
}

/// A locked vault's door is refused to a bare hand and to the key of
/// another floor's vault, and opens for the key found on its own floor,
/// which stays in the lock.
#[test]
fn the_key_opens_its_own_vault_and_no_other() {
    let (mut app, me, map) = on_the_floor_of("The Locked Vault");
    let floor = crate::dungeon::floor_of(map).expect("a floor");
    let (door, entry) = {
        let place = app.world().resource::<WorldMap>().place(map).expect("built");
        (place.locks.iter().find(|l| l.key.is_some()).expect("the vault's lock").at, place.entry)
    };
    let outside = {
        let world_map = app.world().resource::<WorldMap>();
        let tables = world_map.tables();
        let reached = walk_from(entry, |p| p != door && world_map.tile(p).is_some_and(|t| tables.passable[t.index()]));
        Direction::CARDINALS.iter().map(|d| door + d.offset()).find(|n| reached.contains_key(n) && world_map.is_walkable(*n)).expect("a way up to the door")
    };
    let open = |app: &mut App| {
        stand_on(app, me, outside);
        app.world_mut().write_message(Intent::new(me, Open(towards(outside, door))));
        settle(app);
        app.world().resource::<WorldMap>().lock(door).is_none()
    };
    assert!(!open(&mut app), "no hand opens it");
    let other = make(&mut app, KEY_ITEM);
    let elsewhere = if floor == 1 { 2 } else { 1 };
    let tags = key_tags(app.world().resource::<Registries>(), elsewhere);
    app.world_mut().entity_mut(other).insert((VaultKey { floor: elsewhere }, tags));
    give(&mut app, me, other);
    assert!(!open(&mut app), "nor the key to another floor's vault");
    let own = {
        let world = app.world_mut();
        world
            .query::<(&VaultKey, &Position, &OnMap)>()
            .iter(world)
            .find(|(k, _, on)| k.floor == floor && on.0 == map)
            .map(|(_, p, _)| p.0)
            .expect("its key lies on its floor")
    };
    stand_on(&mut app, me, own);
    take_up(&mut app, me);
    assert!(open(&mut app), "its own key opens it");
    assert_eq!(tile_at(&app, map, door), "open vault door");
    let pack = &app.world().get::<Inventory>(me).expect("a pack").items;
    assert_eq!(pack.iter().filter(|i| app.world().get::<VaultKey>(**i).is_some()).count(), 1, "the key stayed in the lock, the other floor's still carried");
}

/// Pulling a sealed vault's lever, once it is spotted, lifts the seal on
/// its floor and raises the gate, and the lever stays pulled.
#[test]
fn the_lever_raises_its_floors_seal() {
    let (mut app, me, map) = on_the_floor_of("The Sealed Vault");
    let gate = app.world().resource::<WorldMap>().place(map).expect("built").locks.iter().find(|l| l.key.is_none()).expect("the vault's seal").at;
    let (lever, there) = props_called(&mut app, map, LEVER)[0];
    app.world_mut().entity_mut(lever).remove::<atglyph::ecs::props::Hidden>();
    let next = beside(&app, there);
    stand_on(&mut app, me, next);
    let pull = app.world().resource::<Verbs>().get(PULL_VERB).expect("declared");
    app.world_mut().write_message(Intent::new(me, Interact { target: lever, verb: pull, thing: None }));
    settle(&mut app);
    assert!(app.world().resource::<WorldMap>().lock(gate).is_none(), "the seal is lifted");
    assert_eq!(tile_at(&app, map, gate), "raised gate");
    assert_eq!(props_called(&mut app, map, PULLED_LEVER).into_iter().map(|(_, p)| p).collect::<Vec<_>>(), vec![there], "the lever stays pulled");
}

/// Steps `me` from beside `onto` onto it, as the player walks, so whatever
/// it sets off when entered goes off.
fn step_onto(app: &mut App, me: Entity, onto: Point) {
    let from = beside(app, onto);
    stand_on(app, me, from);
    app.world_mut().write_message(Intent::new(me, Step(towards(from, onto))));
    settle(app);
    assert_eq!(at(app, me), onto, "stepped onto {onto:?}");
}

/// The sleepers `count` of `name` laid asleep on `map` wherever the floor
/// is open farthest from `near`: the floor's own, for a test of what wakes
/// them.
fn sleepers(app: &mut App, map: MapId, near: Point, name: &str, count: usize) -> Vec<Entity> {
    let cells: Vec<Point> = {
        let world_map = app.world().resource::<WorldMap>();
        let place = world_map.place(map).expect("built");
        let mut open: Vec<Point> =
            place.terrain.bounds().cells().filter(|p| world_map.is_walkable_on(map, *p) && !app.world().resource::<Occupancy>().is_occupied(*p)).collect();
        open.sort_by_key(|p| std::cmp::Reverse(geometry::chebyshev(*p, near)));
        open.into_iter().take(count).collect()
    };
    let asleep = app.world().resource::<Registries>().statuses.expect(crate::content::ASLEEP);
    let made: Vec<Entity> = cells.into_iter().map(|p| crate::testing::monster(app, name, p)).collect();
    for who in &made {
        app.world_mut().write_message(ApplyStatus { target: *who, status: asleep, turns: 1000, by: None, held_by: None });
    }
    settle(app);
    made
}

/// Whether `who` sleeps.
fn sleeps(app: &App, who: Entity) -> bool {
    let asleep = app.world().resource::<Registries>().statuses.expect(crate::content::ASLEEP);
    app.world().get::<ActiveStatuses>(who).is_some_and(|a| a.has(asleep))
}

/// Crossing a warded hall's glyph wakes every sleeper on the floor, the
/// farthest as surely as the nearest.
#[test]
fn a_glyph_crossed_wakes_every_sleeper_on_the_floor() {
    let (mut app, me, map) = on_the_floor_of("The Warded Hall");
    let (_, glyph) = props_called(&mut app, map, "warding glyph")[0];
    let far = sleepers(&mut app, map, glyph, "goblin", 3);
    assert!(far.iter().all(|s| sleeps(&app, *s)), "asleep before");
    step_onto(&mut app, me, glyph);
    for s in &far {
        assert!(!sleeps(&app, *s), "{s:?}, far across the floor, wakes");
    }
}

/// Stepping on any cell of a bone circle stands every body of its ring up
/// at once, where each lay, and leaves none lying.
#[test]
fn the_circles_bodies_rise_when_it_is_crossed() {
    let (mut app, me, map) = on_the_floor_of("The Bone Circle");
    let bones: Vec<Point> = props_called(&mut app, map, "circle bones").into_iter().map(|(_, p)| p).collect();
    assert_eq!(bones.len(), 4, "a body at each corner of the ring");
    let (_, sigil) = props_called(&mut app, map, "grave sigil")[0];
    step_onto(&mut app, me, sigil);
    assert!(props_called(&mut app, map, "circle bones").is_empty(), "no body left lying");
    let skeleton = app.world().resource::<crate::bestiary::Bestiary>().defs.expect("skeleton");
    let world = app.world_mut();
    let risen: Vec<Point> = world
        .query::<(&crate::bestiary::Kind, &Position, &OnMap)>()
        .iter(world)
        .filter(|(k, _, on)| k.0 == skeleton && on.0 == map)
        .map(|(_, p, _)| p.0)
        .collect();
    assert_eq!(risen.len(), bones.len(), "every body stands up");
    // Risen where it lay, and a step from it at most once it has had a turn.
    assert!(risen.iter().all(|p| bones.iter().any(|b| geometry::chebyshev(*p, *b) <= 1)), "{risen:?} from {bones:?}");
}

/// Stepping on one of a bone circle's own bodies stands every body up,
/// the one stepped on beside the stepper rather than on their cell: the
/// stepper stands where they stepped, nothing shares their cell, and
/// every body rises.
#[test]
fn a_body_of_the_circle_stepped_on_rises_beside_its_stepper() {
    let (mut app, me, map) = on_the_floor_of("The Bone Circle");
    let bones: Vec<Point> = props_called(&mut app, map, "circle bones").into_iter().map(|(_, p)| p).collect();
    let corner = bones[0];
    step_onto(&mut app, me, corner);
    assert_eq!(at(&app, me), corner, "the stepper stands on the corner it stepped on");
    let skeleton = app.world().resource::<crate::bestiary::Bestiary>().defs.expect("skeleton");
    let world = app.world_mut();
    let risen: Vec<Point> = world
        .query::<(&crate::bestiary::Kind, &Position, &OnMap)>()
        .iter(world)
        .filter(|(k, _, on)| k.0 == skeleton && on.0 == map)
        .map(|(_, p, _)| p.0)
        .collect();
    assert_eq!(risen.len(), bones.len(), "every body stands up");
    assert!(!risen.contains(&corner), "nothing rose onto the stepper's own cell: {risen:?}");
    assert_eq!(risen.iter().collect::<BTreeSet<_>>().len(), risen.len(), "nor two onto one cell: {risen:?}");
}

/// Lifting a collapsing gallery's prize brings the short way back down in
/// fallen rock, and the long way round still leads to the floor's way in.
#[test]
fn the_gallerys_way_back_falls_in() {
    let (mut app, me, map) = on_the_floor_of("The Collapsing Gallery");
    let (plinth, there) = props_called(&mut app, map, KEYSTONE)[0];
    let lintels: Vec<Point> = props_called(&mut app, map, LINTEL).into_iter().map(|(_, p)| p).collect();
    assert_eq!(lintels.len(), 2, "a lintel over each way");
    let next = beside(&app, there);
    stand_on(&mut app, me, next);
    open_and_take(&mut app, me, plinth);
    let fallen: Vec<&Point> = lintels.iter().filter(|p| tile_at(&app, map, **p) == "fallen rock").collect();
    assert_eq!(fallen.len(), 1, "one way back falls in");
    let walked = walk_from(next, |p| {
        tile_at(&app, map, p) != "fallen rock"
            && app.world().resource::<WorldMap>().tile(p).is_some_and(|t| app.world().resource::<WorldMap>().tables().passable[t.index()])
    });
    let entry = app.world().resource::<WorldMap>().place(map).expect("built").entry;
    assert!(walked.contains_key(&entry), "and the other still leads out to the way in");
}

/// A run continued mid-flood floods on, and a vault's key carried through
/// the save still opens its own vault: what the vaults set going outlives
/// the run being left.
#[test]
fn a_flood_rising_and_a_key_carried_outlive_a_continued_run() {
    let (mut app, me, map) = on_the_floor_of("The Flooding Vault");
    let (altar, there) = props_called(&mut app, map, FLOOD_ALTAR)[0];
    let grates: Vec<Point> = props_called(&mut app, map, GRATE).into_iter().map(|(_, p)| p).collect();
    let far: Vec<Point> = flood_plan(app.world().resource::<WorldMap>(), there, &grates).into_iter().filter(|c| c.deep.is_some()).map(|c| c.at).collect();
    let next = beside(&app, there);
    stand_on(&mut app, me, next);
    open_and_take(&mut app, me, altar);
    let key = make(&mut app, KEY_ITEM);
    let tags = key_tags(app.world().resource::<Registries>(), 3);
    app.world_mut().entity_mut(key).insert((VaultKey { floor: 3 }, tags));
    give(&mut app, me, key);
    pass_turns(&mut app, 1);
    atglyph::save::save_run(app.world_mut()).expect("the run is written down");
    let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("written");
    let mut again = crate::testing::continued(&text);
    pass_turns(&mut again, FLOOD_TURNS + 1);
    assert!(far.iter().all(|p| tile_at(&again, map, *p) == "deep water"), "the water rose on after the run was continued");
    let me = adventurer(&mut again);
    let carried: Vec<Entity> = again.world().get::<Inventory>(me).expect("a pack").items.clone();
    let key = carried.iter().find(|i| again.world().get::<VaultKey>(**i).is_some()).expect("the key carried through the save");
    assert_eq!(again.world().get::<VaultKey>(*key), Some(&VaultKey { floor: 3 }));
    let want = again.world().resource::<Registries>().tags.id(&crate::content::vault_key(3));
    assert_eq!(again.world().get::<Tagged>(*key).map(|t| t.0.clone()), Some(want.into_iter().collect()), "tagged for its own vault again");
}

/// A piece's marks are filled as the vocabulary says: a guarded hoard's
/// band sleeps at its posts, and captives in chains are shackled, offering
/// to be freed, under a dozing gaoler.
#[test]
fn a_pieces_sleepers_sleep_at_their_posts_and_its_captives_are_shackled() {
    for piece in ["The Guarded Hoard", "Captives in Chains"] {
        let (seed, floor) = drawn(&[piece]);
        let mut app = headless(seed);
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        let map = map_of(floor);
        arrive(&mut app, map);
        let marks: Vec<(char, Point)> = {
            let world = app.world();
            let (place, prefabs) = (world.resource::<WorldMap>().place(map).expect("built"), world.resource::<Prefabs<MonsterDef>>());
            let key = prefabs.defs().id(piece).expect("a piece").raw();
            place.spots.iter().filter(|s| s.prefab == Some(key)).filter_map(|s| char::from_u32(s.tag).map(|c| (c, s.at))).collect()
        };
        let at_mark = |app: &mut App, p: Point| {
            let world = app.world_mut();
            world
                .query_filtered::<(Entity, &Position, &OnMap), With<crate::bestiary::Kind>>()
                .iter(world)
                .find(|(_, at, on)| at.0 == p && on.0 == map)
                .map(|(e, ..)| e)
        };
        assert!(marks.iter().any(|(g, _)| [SLEEPER, CAPTIVE].contains(g)), "{piece}: marks to fill");
        let wildlife = app.world().resource::<Registries>().factions.expect(crate::content::WILDLIFE);
        for (glyph, p) in marks.into_iter().filter(|(g, _)| [SLEEPER, CAPTIVE].contains(g)) {
            let Some(who) = at_mark(&mut app, p) else { panic!("{piece}: nobody at its {glyph:?} at {p:?}") };
            let kind = app.world().get::<crate::bestiary::Kind>(who).expect("a monster").0;
            let side = app.world().resource::<crate::bestiary::Bestiary>().defs.get(kind).faction;
            assert_ne!(side, wildlife, "{piece}: its {glyph:?} is small life");
            match glyph {
                SLEEPER => {
                    assert!(sleeps(&app, who), "{piece}: its sleeper sleeps");
                    assert_eq!(app.world().get::<Post>(who).map(|post| post.0), Some(p), "{piece}: at its post");
                }
                CAPTIVE => assert_eq!(crate::captives::held_at(app.world(), who), Some(p), "{piece}: shackled where it was laid"),
                _ => {}
            }
        }
    }
}

/// The mark vocabulary is the risk-and-reward and reward pieces' alone on
/// the floors: a lair's or a flavor piece's mark spelt the same is never
/// filled as a sleeper, a captive or a causeway, since none of them marks
/// one. Mereholt's own marks are its people's posts, `townsfolk.ron`'s,
/// and may spell the same, since [`spawn_vault_marks`] fills a floor's
/// marks and never the town's.
#[test]
fn only_a_vault_piece_marks_a_sleeper_a_captive_or_a_causeway() {
    let undermere = crate::dungeon::Undermere::new(RunSeed(0));
    let vaults = undermere.vault_pieces();
    for (_, def) in undermere.prefabs().defs().iter().filter(|(_, def)| def.name != "Mereholt") {
        for glyph in [SLEEPER, GIANT, GOBLIN, CAPTIVE, CAUSEWAY] {
            if matches!(def.slot(glyph), Some(Slot::Mark)) {
                assert!(vaults.contains(&def.name), "{}: marks {glyph:?}, the vaults' own", def.name);
            }
        }
    }
}

/// A drowned reliquary's causeway is one secret: waited beside, the whole
/// of it is found at once, every cell of it the causeway it was, and the
/// log says so.
#[test]
fn the_sunken_causeway_is_found_whole_from_beside_one_cell_of_it() {
    let (mut app, me, map) = on_the_floor_of("The Drowned Reliquary");
    let causeway = app.world().resource::<WorldMap>().tables().names.iter().position(|n| n == "sunken causeway").expect("a tile");
    let cells: Vec<atglyph::ecs::Secret> = app.world().resource::<WorldMap>().secrets(map).iter().filter(|s| s.is.index() == causeway).copied().collect();
    assert!(cells.len() > 1, "a causeway of more than one cell");
    assert!(cells.iter().all(|s| s.group == cells[0].group), "one group");
    let world_map = app.world().resource::<WorldMap>();
    let (shore, _) = cells
        .iter()
        .find_map(|s| Direction::CARDINALS.iter().map(|d| s.at + d.offset()).find(|p| world_map.is_walkable(*p)).map(|p| (p, s.at)))
        .expect("a causeway cell beside the shore");
    stand_on(&mut app, me, shore);
    for _ in 0..20 {
        pass_turns(&mut app, 1);
        if app.world().resource::<WorldMap>().secrets(map).iter().all(|s| s.group != cells[0].group) {
            break;
        }
    }
    assert!(app.world().resource::<WorldMap>().secrets(map).iter().all(|s| s.group != cells[0].group), "found within twenty turns");
    for s in &cells {
        assert_eq!(tile_at(&app, map, s.at), "sunken causeway", "every cell of it at once");
    }
    let said = crate::testing::said(&app);
    assert_eq!(said.iter().filter(|l| *l == "You make out a causeway just under the water.").count(), 1, "said once: {said:?}");
}

/// Whoever falls through the floor above a locked vault lands where they
/// can walk back to the way in, whatever the fall's roll: never inside
/// the vault, whose key lies outside it and whose door no hand opens from
/// within. Each roll tried is one that drew a cell of the vault while
/// every free cell off the stairs was a landing.
#[test]
fn a_fall_never_lands_inside_a_locked_vault() {
    let (seed, floor) = drawn_from(&["The Locked Vault"], 2);
    let mut app = crate::testing::headless(seed);
    let me = adventurer(&mut app);
    sturdy(&mut app, me);
    let map = map_of(floor);
    arrive(&mut app, map);
    clear_monsters(&mut app);
    let (inside, rolls) = {
        let (world_map, occupancy) = (app.world().resource::<WorldMap>(), app.world().resource::<Occupancy>());
        let place = world_map.place(map).expect("built");
        let open = open_to_the_way_in(place, world_map.tables());
        let free: Vec<Point> = place
            .terrain
            .bounds()
            .cells()
            .filter(|p| world_map.is_walkable_on(map, *p) && !occupancy.is_occupied(*p) && *p != place.entry && Some(*p) != place.exit)
            .collect();
        let inside: BTreeSet<Point> = free.iter().copied().filter(|p| !open.contains(p)).collect();
        let rolls: Vec<u64> = (0..free.len() as u64).filter(|i| inside.contains(&free[*i as usize])).collect();
        (inside, rolls)
    };
    assert!(!rolls.is_empty(), "the vault has room to land in");
    for roll in rolls {
        arrive(&mut app, map_of(floor - 1));
        let fell = crate::effects::moving::fall_through(app.world_mut(), me, roll);
        assert!(fell, "a floor below to fall to");
        settle(&mut app);
        settle(&mut app);
        assert_eq!(crate::testing::here(&app), map, "fell to the floor below");
        assert!(!inside.contains(&at(&app, me)), "roll {roll} landed at {:?}, inside the locked vault", at(&app, me));
    }
}

/// On a floor made for it, a hall and a vault shut behind a locked door:
/// the way in reaches every cell of the hall and none of the vault, and no
/// fall and no far step from anywhere in the hall lands in the vault, the
/// one rule [`open_to_the_way_in`] keeps for both draws. The run's
/// backstop asks the same of every real floor that lays a lock.
#[test]
fn nothing_lands_behind_a_lock_on_a_floor_made_for_it() {
    use atglyph::ecs::places::Lock;
    let tiles = crate::dungeon::registry();
    let (wall, floor, door) = (tiles.expect("stone wall"), tiles.expect("hall floor"), tiles.expect("vault door"));
    let terrain = atglyph::grid::Terrain::from_fn(16, 7, |p| match (p.x, p.y) {
        (0 | 15, _) | (_, 0 | 6) => wall,
        (10, 3) => door,
        (10, _) => wall,
        _ => floor,
    });
    let lock = Point::new(10, 3);
    let built = PlaceBuild {
        terrain,
        entry: Point::new(1, 1),
        exit: None,
        spots: Vec::new(),
        locks: vec![Lock { at: lock, key: None }],
        secrets: Vec::new(),
        ambient: atglyph::grid::Light::DARK,
        peaceful: false,
    };
    let map = map_of(3);
    let tables = tiles.tables();
    let mut world = WorldMap::new(tables.clone());
    world.install_place(map, built);
    let place = world.place(map).expect("installed");
    let open = open_to_the_way_in(place, &tables);
    for p in place.terrain.bounds().cells().filter(|p| world.is_walkable_on(map, *p) && *p != lock) {
        assert_eq!(open.contains(&p), p.x < 10, "{p:?}: open to the way in only on the hall's side");
    }
    let built = PlaceBuild {
        terrain: place.terrain.clone(),
        entry: place.entry,
        exit: None,
        spots: Vec::new(),
        locks: vec![Lock { at: lock, key: None }],
        secrets: Vec::new(),
        ambient: atglyph::grid::Light::DARK,
        peaceful: false,
    };
    assert!(nothing_lands_behind_a_lock(&tables, built, map, "the made-up floor"), "it lays a lock");
}

/// On every floor, over many draws, whoever is cast in a part, a vault's
/// sleeper or captive, a patrol or the run's own captive, is never one of
/// the floor's small life, though the floor's own table draws them; and
/// every floor still has someone to cast.
#[test]
fn nobody_cast_in_a_part_is_small_life_on_any_floor() {
    use atglyph::rand::SeedableRng;
    let library = crate::content::Library::load(&crate::content::effect_kinds(), &TriggerKinds::default());
    let (bestiary, registries) = (&library.bestiary, &library.registries);
    let wildlife = registries.factions.expect(crate::content::WILDLIFE);
    let small = |id: &Id<MonsterDef>| bestiary.defs.get(*id).faction == wildlife;
    assert!(bestiary.spawns.entries().iter().any(|row| small(&row.item)), "the table draws small life for the floors' own people");
    for floor in 1..crate::dungeon::FLOORS {
        let mut rng = atglyph::rand::rngs::StdRng::seed_from_u64(u64::from(floor));
        for _ in 0..2000 {
            let cast = bestiary.pick_cast(floor, registries, &mut rng).unwrap_or_else(|| panic!("floor {floor}: nobody to cast"));
            assert!(!small(&cast), "floor {floor}: {} cast in a part", bestiary.defs.get(cast).name);
        }
    }
}

/// Emptying a Rat Trap's gnawed plinth lets two rats out beside each of its
/// holes, each on a cell of its own, and the log says the walls crack.
#[test]
fn the_rat_traps_vermin_pour_from_every_hole_when_its_prize_is_taken() {
    let (mut app, me, map) = on_the_floor_of("The Rat Trap");
    let holes: Vec<Point> = props_called(&mut app, map, RAT_HOLE).into_iter().map(|(_, p)| p).collect();
    assert_eq!(holes.len(), 3, "a hole in three of its corners");
    let (plinth, there) = props_called(&mut app, map, RAT_PLINTH)[0];
    let next = beside(&app, there);
    stand_on(&mut app, me, next);
    open_and_take(&mut app, me, plinth);
    let rat = app.world().resource::<crate::bestiary::Bestiary>().defs.expect(VERMIN.0);
    let world = app.world_mut();
    let rats: Vec<Point> =
        world.query::<(&crate::bestiary::Kind, &Position, &OnMap)>().iter(world).filter(|(k, _, on)| k.0 == rat && on.0 == map).map(|(_, p, _)| p.0).collect();
    assert_eq!(rats.len() as u32, holes.len() as u32 * VERMIN.1, "two from each hole: {rats:?}");
    assert_eq!(rats.iter().collect::<BTreeSet<_>>().len(), rats.len(), "each on a cell of its own");
    assert!(rats.iter().all(|r| holes.iter().any(|h| geometry::chebyshev(*r, *h) <= crate::effects::NEAR + 1)), "each beside a hole: {rats:?}");
    assert!(crate::testing::said(&app).iter().any(|l| l.contains("squeals")), "the log says so");
}

/// Stepping on a paralysis vestibule's vent rouses the sleepers of the
/// room beyond it, every one of the piece's own.
#[test]
fn the_paralysis_vestibules_vent_rouses_the_sleepers_beyond_it() {
    let piece = "The Paralysis Vestibule";
    let (seed, floor) = drawn(&[piece]);
    let mut app = headless(seed);
    let me = adventurer(&mut app);
    sturdy(&mut app, me);
    let map = map_of(floor);
    arrive(&mut app, map);
    let marks: Vec<Point> = {
        let world = app.world();
        let (place, prefabs) = (world.resource::<WorldMap>().place(map).expect("built"), world.resource::<Prefabs<MonsterDef>>());
        let key = prefabs.defs().id(piece).expect("a piece").raw();
        place.spots.iter().filter(|s| s.prefab == Some(key) && s.tag == SLEEPER as u32).map(|s| s.at).collect()
    };
    let world = app.world_mut();
    let waiting: Vec<Entity> = world
        .query_filtered::<(Entity, &Position, &OnMap), With<crate::bestiary::Kind>>()
        .iter(world)
        .filter(|(_, p, on)| on.0 == map && marks.contains(&p.0))
        .map(|(e, ..)| e)
        .collect();
    assert_eq!(waiting.len(), marks.len(), "a sleeper on each mark");
    assert!(waiting.iter().all(|s| sleeps(&app, *s)), "asleep before");
    // The vestibule's own vent, the one nearest its sleepers: from floor
    // six a paralysis vent is also one of the floor's traps, laid on its
    // corridors and doorways, and one of those may be found first.
    let (_, vent) = props_called(&mut app, map, "paralysis vent")
        .into_iter()
        .min_by_key(|(_, at)| marks.iter().map(|m| geometry::chebyshev(*at, *m)).min())
        .expect("the vestibule's vent");
    step_onto(&mut app, me, vent);
    for s in &waiting {
        assert!(!sleeps(&app, *s), "{s:?}, in the room beyond, is roused");
    }
}

/// Stepping on a tinder room's ember plate sets its straw alight round the
/// plate at once, and the fire runs over the straw to the far side of the
/// room, by the brazier, within a few turns.
#[test]
fn the_tinder_rooms_ember_plate_sets_the_straw_alight() {
    let (mut app, me, map) = on_the_floor_of("The Tinder Room");
    let (_, plate) = props_called(&mut app, map, "ember plate")[0];
    let (_, brazier) = props_called(&mut app, map, "brazier").into_iter().find(|(_, p)| geometry::chebyshev(*p, plate) == 2).expect("the room's brazier");
    let far: Vec<Point> = Direction::ALL.iter().map(|d| brazier + d.offset()).filter(|p| geometry::chebyshev(*p, plate) == 2).collect();
    assert!(far.iter().all(|p| tile_at(&app, map, *p) == "straw"), "straw beside the brazier");
    step_onto(&mut app, me, plate);
    let burning = |app: &App, p: Point| app.world().resource::<atglyph::ecs::fire::Fire>().is_burning(p);
    let near: Vec<Point> = Direction::ALL.iter().map(|d| plate + d.offset()).filter(|p| tile_at(&app, map, *p) == "straw").collect();
    assert!(near.iter().any(|p| burning(&app, *p)), "the straw round the plate catches at once");
    let mut reached = false;
    for _ in 0..6 {
        pass_turns(&mut app, 1);
        reached |= far.iter().any(|p| burning(&app, *p) || tile_at(&app, map, *p) != "straw");
    }
    assert!(reached, "the fire runs over the straw to the brazier's side");
}

/// Whoever stands under a collapsing gallery's lintel when it falls leaps
/// clear onto open ground rather than being buried standing in the rock.
#[test]
fn whoever_stands_under_the_lintel_leaps_clear_of_the_fall() {
    let (mut app, me, map) = on_the_floor_of("The Collapsing Gallery");
    let (plinth, there) = props_called(&mut app, map, KEYSTONE)[0];
    let lintels: Vec<Point> = props_called(&mut app, map, LINTEL).into_iter().map(|(_, p)| p).collect();
    let under: Vec<Entity> = lintels.iter().map(|p| crate::testing::still(&mut app, "goblin", *p)).collect();
    let next = beside(&app, there);
    stand_on(&mut app, me, next);
    open_and_take(&mut app, me, plinth);
    settle(&mut app);
    let fallen: Vec<Point> = lintels.iter().copied().filter(|p| tile_at(&app, map, *p) == "fallen rock").collect();
    assert_eq!(fallen.len(), 1, "one way back falls in");
    for who in under {
        let stands = at(&app, who);
        assert!(!fallen.contains(&stands), "{who:?} buried in the rock at {stands:?}");
        assert!(app.world().resource::<WorldMap>().is_walkable_on(map, stands), "{who:?} on open ground at {stands:?}");
    }
}

/// The far end of `plan` held wading-deep on the floor now, with each
/// cell's steps from the door.
fn shallow_far_end(app: &App, map: MapId, plan: &[FloodCell]) -> Vec<(Point, u32)> {
    plan.iter().filter(|c| c.deep.is_some() && tile_at(app, map, c.at) != "deep water").map(|c| (c.at, c.back)).collect()
}

/// A wader at the far wall of a flooding vault is never stranded: deep
/// water never rises under it, and it keeps one wading-deep channel out
/// to the near part of the room, a step nearer the door at every step,
/// until it has gone.
#[test]
fn a_wader_at_the_far_wall_keeps_a_channel_out_of_the_flood() {
    let (mut app, me, map) = on_the_floor_of("The Flooding Vault");
    let (altar, there) = props_called(&mut app, map, FLOOD_ALTAR)[0];
    let grates: Vec<Point> = props_called(&mut app, map, GRATE).into_iter().map(|(_, p)| p).collect();
    let plan = flood_plan(app.world().resource::<WorldMap>(), there, &grates);
    let far = plan.iter().filter(|c| c.deep.is_some() && !grates.contains(&c.at)).max_by_key(|c| c.back).copied().expect("a far end");
    let wader = crate::testing::still(&mut app, "goblin", far.at);
    let next = beside(&app, there);
    stand_on(&mut app, me, next);
    open_and_take(&mut app, me, altar);
    pass_turns(&mut app, FLOOD_TURNS + 2);
    assert_eq!(at(&app, wader), far.at, "the wader stands where it was put");
    let mut channel = shallow_far_end(&app, map, &plan);
    channel.sort_by_key(|(_, back)| *back);
    assert_eq!(channel.last().map(|(p, _)| *p), Some(far.at), "wading-deep under the wader, and nothing of the far end farther out");
    for pair in channel.windows(2) {
        let ((a, near), (b, out)) = (pair[0], pair[1]);
        assert!(out == near + 1 && geometry::manhattan(a, b) == 1, "{b:?} steps on to {a:?}, a step nearer the door: one channel, not a pool");
    }
    let (mouth, steps) = channel[0];
    let ashore = plan.iter().find(|c| c.deep.is_none() && c.back + 1 == steps && geometry::manhattan(c.at, mouth) == 1);
    assert!(ashore.is_some_and(|c| tile_at(&app, map, c.at) == "flooded floor"), "the channel comes out on the waded near part");
    app.world_mut().despawn(wader);
    pass_turns(&mut app, 2);
    assert!(shallow_far_end(&app, map, &plan).is_empty(), "and all of the far end is deep once it has gone");
}

/// The water a wader holds back is its one channel out: every other cell
/// of a flooding vault's far end goes too deep to cross on time, so
/// lingering by the far wall saves nothing lying there, and a monster
/// standing in the far end does not hold the flood off.
#[test]
fn deep_water_rises_everywhere_off_a_waders_channel() {
    let (mut app, me, map) = on_the_floor_of("The Flooding Vault");
    let (altar, there) = props_called(&mut app, map, FLOOD_ALTAR)[0];
    let grates: Vec<Point> = props_called(&mut app, map, GRATE).into_iter().map(|(_, p)| p).collect();
    let plan = flood_plan(app.world().resource::<WorldMap>(), there, &grates);
    let far = plan.iter().filter(|c| c.deep.is_some() && !grates.contains(&c.at)).max_by_key(|c| c.back).copied().expect("a far end");
    let edge = plan.iter().filter(|c| c.deep.is_some()).map(|c| c.back).min().expect("a far end");
    let wader = crate::testing::still(&mut app, "goblin", far.at);
    let next = beside(&app, there);
    stand_on(&mut app, me, next);
    open_and_take(&mut app, me, altar);
    pass_turns(&mut app, FLOOD_TURNS + 2);
    assert_eq!(at(&app, wader), far.at, "the wader stands where it was put");
    let held = shallow_far_end(&app, map, &plan);
    let room = plan.iter().filter(|c| c.deep.is_some()).count();
    assert_eq!(held.len() as u32, far.back - edge + 1, "one cell a step held, of the far end's {room}");
    assert!(held.len() < room, "the rest of the far end is deep");
}

/// The prop `name` of `map`, and where it stands; the first, in row order.
fn the_one(app: &mut App, map: MapId, name: &str) -> (Entity, Point) {
    *props_called(app, map, name).first().unwrap_or_else(|| panic!("a {name} on the floor"))
}

/// Takes up `verb` on `prop` from beside it, with `thing` chosen first
/// when the offer chooses one.
fn take_up_on(app: &mut App, me: Entity, prop: Entity, at: Point, verb: &str, thing: Option<Entity>) {
    let next = beside(app, at);
    stand_on(app, me, next);
    let verb = app.world().resource::<Verbs>().get(verb).expect("declared");
    app.world_mut().write_message(Intent::new(me, Interact { target: prop, verb, thing }));
    settle(app);
}

/// A healing spring drunk from heals its drinker whole, where it stands,
/// and runs dry: drunk from once, never twice.
#[test]
fn a_healing_spring_heals_its_drinker_whole_once() {
    use super::wells::*;
    let (mut app, me, map) = on_the_floor_of("Healing Spring");
    let (spring, at) = the_one(&mut app, map, SPRING);
    // An adventurer's own health rather than the sturdy test one's, which
    // no draught is written to fill.
    let full = 120;
    app.world_mut().entity_mut(me).insert(Health::full(full));
    app.world_mut().get_mut::<Health>(me).expect("health").current = 1;
    take_up_on(&mut app, me, spring, at, DRINK_VERB, None);
    assert_eq!(app.world().get::<Health>(me).expect("health").current, full, "healed whole");
    assert!(props_called(&mut app, map, SPRING).is_empty(), "the spring is gone");
    assert_eq!(props_called(&mut app, map, DRY_SPRING).len(), 1, "run dry where it stood");
}

/// A cold forge's anvil tempers a worn weapon a level, chosen among what
/// is worn and nothing carried besides, and goes cold.
#[test]
fn the_cold_forges_anvil_tempers_a_worn_weapon_once() {
    use super::wells::*;
    let (mut app, me, map) = on_the_floor_of("The Cold Forge");
    let (anvil, at) = the_one(&mut app, map, ANVIL);
    assert!(app.world().get::<atglyph::ecs::props::Scenery>(anvil).is_none(), "listed with what can be acted on");
    let hand = app.world().resource::<Registries>().slots.expect("main hand");
    let sword = app.world().get::<Equipped>(me).and_then(|worn| worn.in_slot(hand)).expect("the sword they set out with");
    let carried = make(&mut app, "potion of descent");
    give(&mut app, me, carried);
    let among = app.world().resource::<WellChoices>().temper;
    let choices = app.world_mut().run_system_with(among, OfferChooser { taker: me, offerer: anvil, verb: Verbs::OPEN }).expect("it runs");
    assert!(choices.contains(&sword) && !choices.contains(&carried), "the sword worn and not the potion carried: {choices:?}");
    let before = app.world().get::<ItemLevel>(sword).map_or(0, |l| l.0);
    take_up_on(&mut app, me, anvil, at, TEMPER_VERB, Some(sword));
    assert_eq!(app.world().get::<ItemLevel>(sword).map_or(0, |l| l.0), before + 1, "tempered a level");
    assert!(props_called(&mut app, map, ANVIL).is_empty() && props_called(&mut app, map, COLD_ANVIL).len() == 1, "and the anvil is cold");
}

/// A moonwell drunk from tells what one carried thing is, and darkens.
#[test]
fn a_moonwell_tells_what_one_carried_thing_is_once() {
    use super::wells::*;
    let (mut app, me, map) = on_the_floor_of("Moonwell");
    let (well, at) = the_one(&mut app, map, MOONWELL);
    let potion = make(&mut app, "potion of descent");
    give(&mut app, me, potion);
    assert!(!app.world().resource::<crate::lore::Lore>().knows("potion of descent"), "not known before");
    take_up_on(&mut app, me, well, at, DRINK_VERB, Some(potion));
    assert!(app.world().resource::<crate::lore::Lore>().knows("potion of descent"), "known after");
    assert!(props_called(&mut app, map, MOONWELL).is_empty() && props_called(&mut app, map, DARK_MOONWELL).len() == 1, "and the well is dark");
}
