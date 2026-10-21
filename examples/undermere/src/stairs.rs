//! The ways between places, the line the log gives each place as it is
//! entered, and the town the adventurer knows by heart.
//!
//! The floors are a ladder, [`ladder`], and the engine lays its stairs:
//! up at a floor's way in and down at its way on, when the floor is built,
//! so the way on is always as far from the arrival as the place allows,
//! and the tenth has no way down, since the throne is where the Undermere
//! ends. Stairs are drawn by a glyph of their own and kept on the map from
//! memory once seen, as every still thing the map draws is.
//!
//! The rest is Undermere's own. In Mereholt the way on is the Drowned
//! Gate, which leads to floor one's way in, or to the gate mark a waystone
//! left, by [`waystone`](crate::waystone); on floor one the way up is the
//! gate's far side, the top of the ladder, and leads back onto the gate.
//! Each is a [`Transition`] marked [`Gate`], laid by [`link_places`] the
//! first time its place is entered. The gate is drawn by the tile it
//! stands on, which shimmers and glows violet on both its sides, so it
//! carries no glyph to hide the tile under.
//!
//! Mereholt is where the adventurer lives, so its first arrival writes the
//! whole town into what they remember, [`know_the_town`], where a floor
//! below is learned only as it is seen.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::dungeon::{FLOORS, TOWN, floor_of, map_of};
use crate::light::BiomeDraws;

/// What stairs are drawn in: a pale stone, so a way on reads against every
/// band's floor.
pub const STAIRS: Rgb = Rgb::new(235, 219, 158);

/// The floors, top first, as the engine lays stairs between them: stairs
/// up `<` and down `>` in [`STAIRS`]' pale stone. Floor one's way up is
/// the gate, which Undermere lays, and the tenth has nothing below it.
pub fn ladder() -> Ladder {
    let (up, down) = (StairLook::new('<', STAIRS, "stairs up"), StairLook::new('>', STAIRS, "stairs down"));
    Ladder::new((1..=FLOORS).map(map_of), up, down).top(LadderEnd::Game)
}

/// The Drowned Gate: the town's way on, and floor one's way up. What a
/// save writes down to stand one again; where each leads is its
/// [`Transition`], which the engine keeps, and what it is called is
/// [`Gate::NAME`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gate;

impl Gate {
    /// What the look cursor and the nearby rail call it.
    pub const NAME: &str = "the Drowned Gate";
}

/// Any way through standing on a place: a stair the engine laid, or the
/// gate.
pub type WayThrough = Or<(With<Stair>, With<Gate>)>;

/// Stands the gate at `at` on `map`, leading `to`.
fn lay(commands: &mut Commands, at: Point, map: MapId, to: Destination) {
    commands.spawn((Position(at), OnMap(map), Transition { to }, Gate, Name::new(Gate::NAME)));
}

/// What the log says on entering `map`, the town or a floor: where the
/// adventurer is, and what the place is like. A floor's is its drawn
/// biome's, read off `draws`, its log name and its line for the floor,
/// since what a floor is like is its biome's, the Cellars' first floor no
/// hall of braziers; every floor has a draw, so no floor goes unnamed.
pub fn place_line(map: MapId, draws: &BiomeDraws) -> String {
    let Some(floor) = floor_of(map) else {
        return "Mereholt, at dusk, on the shore of the black lake.".to_string();
    };
    let drawn = draws.of(floor);
    format!("Floor {floor}: {}. {}", drawn.name, drawn.mood(floor))
}

/// Writes every cell of Mereholt into what the adventurer remembers, and
/// what stands in it, so the town is there whole from the first frame
/// rather than the slivers of it in sight, which read as walls strewn
/// about.
///
/// Remembered, not seen: what is in view is still what sight and the light
/// allow, and the rest is drawn as memory draws anything, the town's props
/// and things with it, which the map takes in at its next look since they
/// are put down in the same pass as this. Written into the town's own
/// memory whatever place is being read, since a warp later in the same
/// pass may already have moved the adventurer on.
pub fn know_the_town(map: &WorldMap, knowledge: &mut Knowledge) {
    let Some(town) = map.place(TOWN) else { return };
    let reading = knowledge.current();
    knowledge.switch(TOWN);
    for p in town.terrain.bounds().cells() {
        knowledge.learn(p);
    }
    knowledge.switch(reading);
}

/// Lays the gate the first time Mereholt or floor one is entered, and names
/// the place in the log on every arrival, coming up as well as going down.
/// The first arrival in a new stratum says the stratum's own crossing line
/// ahead of the place's, once.
/// The stairs between the floors are the engine's, laid from [`ladder`]
/// when each floor is built.
///
/// On the run's first arrival anywhere, which is Mereholt in every real
/// run and a deeper floor in a photograph begun there, it also says how to
/// see the keys, under the place's own line: said when the run is begun,
/// it would be written before the arrival and read as though it came
/// first, and said on the town's first arrival instead, it would be said
/// again on coming home to a town first seen mid-run. The first arrival is
/// the one that finds no other place visited, which the save keeps, so a
/// continued run never says it twice. And the adventurer knows the town
/// they live in, [`know_the_town`].
pub fn link_places(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    lines: ArrivalLines,
    map: Res<WorldMap>,
    mut knowledge: ResMut<Knowledge>,
    marks: crate::waystone::GateMarks,
) {
    let ArrivalLines { help, mut tell, draws } = lines;
    for ev in entered.read() {
        if ev.first
            && let Some(stratum) = crate::dungeon::crossed_into(ev.map, |m| map.place(m).is_some_and(|p| p.visited))
        {
            tell.write(Tell::new(stratum.crossing, Tones::NOTICE));
        }
        tell.write(Tell::new(place_line(ev.map, &draws), Tones::NOTICE));
        if !ev.first {
            continue;
        }
        let visited = std::iter::once(TOWN).chain((1..=FLOORS).map(map_of)).filter(|m| map.place(*m).is_some_and(|p| p.visited)).count();
        if visited == 1 {
            tell.write(Tell::new(format!("Press {} for the controls.", help.toggle.label()), Tones::MUTED));
        }
        // The town: the gate, its way on, leading to floor one's way in or
        // to a gate mark left before the town was ever entered, and the
        // whole of it known.
        if ev.map == TOWN {
            if let Some(gate) = ev.exit {
                lay(&mut commands, gate, ev.map, crate::waystone::gate_to(marks.mark()));
            }
            know_the_town(&map, &mut knowledge);
        }
        // Floor one's way up, the top of the ladder: the gate's far side,
        // onto the gate.
        if floor_of(ev.map) == Some(1) {
            lay(&mut commands, ev.entry, ev.map, Destination::Place { map: TOWN, arrive: Arrive::Exit });
        }
    }
}

/// What [`link_places`] says on an arrival from: the keys it names on the
/// run's first, the log it writes to, and each floor's drawn biome, whose
/// name its line carries.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ArrivalLines<'w> {
    help: Res<'w, ControlsKeys>,
    tell: MessageWriter<'w, Tell>,
    draws: Res<'w, BiomeDraws>,
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::{FLOORS, TOWN, map_of};
    use crate::testing::{Way, arrive, go_through, headless, stand_on};

    /// Every way through on `map`: where it stands, where it leads, and
    /// which way it is.
    fn ways_on(app: &mut App, map: MapId) -> Vec<(Point, Destination, Way)> {
        crate::testing::ways(app.world_mut(), map).into_iter().map(|(p, way, to)| (p, to, way)).collect()
    }

    /// Every way through on the map being read.
    fn ways_here(app: &mut App) -> Vec<(Point, Destination, Way)> {
        let here = app.world().resource::<WorldMap>().current();
        ways_on(app, here)
    }

    /// The registered name of the tile at `p` on the map being read.
    fn tile_at(app: &App, p: Point) -> String {
        let map = app.world().resource::<WorldMap>();
        let tile = map.tile(p).expect("a tile");
        map.tables().names[tile.index()].clone()
    }

    fn here(app: &App) -> MapId {
        app.world().resource::<WorldMap>().current()
    }

    fn at(app: &App, me: Entity) -> Point {
        app.world().get::<Position>(me).expect("standing somewhere").0
    }

    #[test]
    fn the_gate_leads_to_floor_one_and_floor_ones_way_up_leads_back_to_the_gate() {
        for s in 0..3 {
            let mut app = headless(RunSeed(s));
            let me = arrive(&mut app, TOWN);
            let ways = ways_here(&mut app);
            assert_eq!(ways.len(), 1, "seed {s}: the town has one way through, the gate: {ways:?}");
            let (gate, to, way) = ways[0];
            assert_eq!((way, to), (Way::Gate, Destination::Place { map: map_of(1), arrive: Arrive::Entry }), "seed {s}");
            assert_eq!(tile_at(&app, gate), "gate", "seed {s}: the gate stands on the gate's tile");

            stand_on(&mut app, me, gate);
            go_through(&mut app, me);
            assert_eq!(here(&app), map_of(1), "seed {s}: the gate leads to floor one");
            let entry = app.world().resource::<WorldMap>().place(map_of(1)).unwrap().entry;
            assert_eq!(at(&app, me), entry, "seed {s}: at its way in");
            let back = ways_here(&mut app).into_iter().find(|(p, _, _)| *p == entry).expect("a way back where the gate let out");
            assert_eq!((back.2, back.1), (Way::Gate, Destination::Place { map: TOWN, arrive: Arrive::Exit }), "seed {s}");
            assert_eq!(tile_at(&app, entry), "gate", "seed {s}: floor one's way up is the gate's other side");

            go_through(&mut app, me);
            assert_eq!(here(&app), TOWN, "seed {s}: floor one's way up leads back to Mereholt");
            assert_eq!(at(&app, me), gate, "seed {s}: onto the gate");
        }
    }

    /// Every stair is laid looking as the ladder says, a pale stone `<`
    /// or `>` on no actor, which the map draws and keeps in memory once
    /// seen, and still does in a continued run; `main.rs`'s own test walks
    /// away from one and reads it drawn from memory.
    #[test]
    fn every_stair_keeps_the_look_the_ladder_gives_it_through_a_save() {
        let looks = |app: &mut App| {
            let world = app.world_mut();
            let mut looks: Vec<(char, Rgb, String)> =
                world.query_filtered::<&Stair, Without<Actor>>().iter(world).map(|s| (s.look.glyph, s.look.color, s.look.name.clone())).collect();
            looks.sort();
            looks.dedup();
            looks
        };
        let ladder = ladder();
        let expected = vec![(ladder.up.glyph, ladder.up.color, ladder.up.name.clone()), (ladder.down.glyph, ladder.down.color, ladder.down.name.clone())];
        let mut app = headless(RunSeed(2));
        arrive(&mut app, map_of(2));
        assert_eq!(looks(&mut app), expected);
        let text = app.world().resource::<rl_engine::rl_save::Saves>().load(crate::save::SLOT).expect("reads").expect("saved on arrival");
        let mut carried_on = crate::testing::continued(&text);
        assert_eq!(looks(&mut carried_on), expected, "continued");
    }

    /// The run's backstop on foot: from Mereholt's gate down every stair to
    /// the throne, over a span of seeds, every floor is joined to the next
    /// as the ladder says and is never cut off by what its arrival stood
    /// on it. Mereholt's one way is the gate to floor one; every floor has
    /// exactly one way up and, but the tenth, one way down, and nothing
    /// else that leads anywhere, floor one's way up the gate onto
    /// Mereholt's; nobody is made on either way, the way down stands free
    /// to land on once whoever walked onto it since is gone, and taking it
    /// lands on the next floor's way in, with a way back up underfoot.
    /// Whatever the arrival stood that blocks, a camp's fire first among
    /// them, never stands between the way in and the way down, doors
    /// opened: seed 49's fire, lit in a corridor on floor five, once held
    /// every delve of it there for good. A run continued from the save the
    /// last arrival wrote has every floor's ways again, where they stood
    /// and leading where they led, once. The ladder and the population
    /// each own their half on a map of their own; this is the real floors,
    /// walked.
    #[test]
    fn every_floor_walked_down_to_is_joined_to_the_next_and_never_cut_off_by_what_stands_on_it_through_a_save_over_a_span_of_seeds() {
        crate::testing::par_seeds(crate::testing::seeds(0..50), |s| {
            let mut app = headless(RunSeed(s));
            app.init_resource::<Made>();
            app.add_observer(|add: On<Add, crate::bestiary::Kind>, monsters: Query<(&Position, &OnMap)>, mut made: ResMut<Made>| {
                if let Ok((at, on)) = monsters.get(add.entity) {
                    made.0.push((on.0, at.0));
                }
            });
            let me = arrive(&mut app, TOWN);
            crate::testing::sturdy(&mut app, me);
            let gate = app.world().resource::<WorldMap>().place(TOWN).expect("Mereholt").exit.expect("the gate's cell");
            assert_eq!(ways_on(&mut app, TOWN), [(gate, Destination::Place { map: map_of(1), arrive: Arrive::Entry }, Way::Gate)], "seed {s}: Mereholt");
            stand_on(&mut app, me, gate);
            go_through(&mut app, me);
            let mut seen = Vec::new();
            for floor in 1..=FLOORS {
                assert_eq!(here(&app), map_of(floor), "seed {s}: the way down from floor {}", floor - 1);
                let place = app.world().resource::<WorldMap>().place(map_of(floor)).expect("built");
                let entry = place.entry;
                assert_eq!(at(&app, me), entry, "seed {s}, floor {floor}: arrived at its way in");
                let up = match floor {
                    1 => (entry, Destination::Place { map: TOWN, arrive: Arrive::Exit }, Way::Gate),
                    _ => (entry, Destination::Place { map: map_of(floor - 1), arrive: Arrive::Exit }, Way::Up),
                };
                let mut expected = vec![up];
                let down = place.exit;
                if floor < FLOORS {
                    let exit = down.unwrap_or_else(|| panic!("seed {s}, floor {floor}: a floor above the throne has a way on"));
                    expected.push((exit, Destination::Place { map: map_of(floor + 1), arrive: Arrive::Entry }, Way::Down));
                }
                expected.sort_by_key(|(p, _, _)| *p);
                let ways = ways_on(&mut app, map_of(floor));
                assert_eq!(ways, expected, "seed {s}, floor {floor}");
                seen.push(ways);
                let Some(down) = down else { continue };
                nothing_standing_cuts_the_way_in_off_from_the_way_down(&mut app, map_of(floor), down, &format!("seed {s}, floor {floor}"));
                let made_on_a_way = app.world().resource::<Made>().0.iter().any(|(on, p)| *on == map_of(floor) && (*p == entry || *p == down));
                assert!(!made_on_a_way, "seed {s}, floor {floor}: one made on a way");
                // A monster that walked onto the stairs since is play, one
                // the adventurer would fight there: seed 19's ogre wandered
                // onto floor seven's way down on the arrival's own turn.
                // Taken off, so what the landing finds is the way itself.
                step_off(&mut app, me, map_of(floor), down);
                stand_on(&mut app, me, down);
                assert_eq!(at(&app, me), down, "seed {s}, floor {floor}: the way down stood free to land on");
                go_through(&mut app, me);
            }
            let text = app.world().resource::<rl_engine::rl_save::Saves>().load(crate::save::SLOT).expect("reads").expect("saved on arrival");
            let mut again = crate::testing::continued(&text);
            for floor in 1..=FLOORS {
                assert_eq!(ways_on(&mut again, map_of(floor)), seen[floor as usize - 1], "seed {s}, floor {floor}, continued");
            }
            assert_eq!(ways_on(&mut again, TOWN).len(), 1, "seed {s}: Mereholt's gate, continued");
        });
    }

    /// Where every monster was made, before anybody took a step.
    #[derive(Resource, Default)]
    struct Made(Vec<(MapId, Point)>);

    /// Takes every actor but `me` off `cell` on `map`, and runs a frame so
    /// the turn's cleanup drops them from who stands where.
    fn step_off(app: &mut App, me: Entity, map: MapId, cell: Point) {
        let world = app.world_mut();
        let there: Vec<Entity> = world
            .query_filtered::<(Entity, &Position, &OnMap), With<Actor>>()
            .iter(world)
            .filter(|(e, p, on)| *e != me && on.0 == map && p.0 == cell)
            .map(|(e, ..)| e)
            .collect();
        for e in there {
            world.despawn(e);
        }
        app.update();
    }

    /// The way down at `down` on `map` is reached from the way in round
    /// every prop that blocks, doors opened.
    fn nothing_standing_cuts_the_way_in_off_from_the_way_down(app: &mut App, map: MapId, down: Point, at: &str) {
        let world = app.world_mut();
        let blocked: Vec<Point> =
            world.query_filtered::<(&Position, &OnMap), (With<Prop>, With<Blocks>)>().iter(world).filter(|(_, on)| on.0 == map).map(|(p, _)| p.0).collect();
        let world_map = app.world().resource::<WorldMap>();
        let entry = world_map.place(map).expect("built").entry;
        let mut reached = vec![entry];
        let mut seen: std::collections::BTreeSet<Point> = reached.iter().copied().collect();
        while let Some(p) = reached.pop() {
            for next in geometry::square(p, 1) {
                let door = world_map.tile(next).is_some_and(|t| world_map.tables().names[t.index()] == "door");
                if (world_map.is_walkable_on(map, next) || door) && !blocked.contains(&next) && seen.insert(next) {
                    reached.push(next);
                }
            }
        }
        assert!(seen.contains(&down), "{at}: the way down at {down:?} is cut off from the way in at {entry:?}");
    }

    #[test]
    fn floor_ten_has_no_way_down() {
        for s in 0..3 {
            let mut app = headless(RunSeed(s));
            arrive(&mut app, map_of(FLOORS));
            let ways = ways_here(&mut app);
            let entry = app.world().resource::<WorldMap>().place(map_of(FLOORS)).unwrap().entry;
            assert_eq!(
                ways,
                [(entry, Destination::Place { map: map_of(FLOORS - 1), arrive: Arrive::Exit }, Way::Up)],
                "seed {s}: the last floor's one way is back up"
            );
        }
    }

    /// Enter on the gate, the way a player takes it, goes through to floor
    /// one, at its way in.
    #[test]
    fn walking_through_the_gate_arrives_on_floor_one() {
        use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};
        for s in 0..3 {
            let mut app = headless(RunSeed(s));
            app.add_plugins(KeyScriptPlugin);
            let me = crate::testing::adventurer(&mut app);
            let gate = ways_here(&mut app).into_iter().find(|(_, _, w)| *w == Way::Gate).expect("Mereholt's gate").0;
            stand_on(&mut app, me, gate);
            assert_eq!(at(&app, me), gate, "seed {s}: on the gate");
            press(&mut app, KeyCode::Enter);
            assert_eq!(here(&app), map_of(1), "seed {s}: through the gate is floor one");
            let entry = app.world().resource::<WorldMap>().place(map_of(1)).unwrap().entry;
            assert_eq!(at(&app, me), entry, "seed {s}: at its way in");
        }
    }

    /// The run's first arrival names Mereholt and then, under it, how to
    /// see the keys: said in the arrival rather than when the run is begun,
    /// so it reads below the town's line rather than above it.
    #[test]
    fn the_log_names_mereholt_and_then_says_how_to_see_the_keys() {
        let mut app = headless(RunSeed(1));
        crate::testing::settle(&mut app);
        let lines: Vec<String> = app.world().resource::<MessageLog>().iter().map(|e| e.text.clone()).collect();
        let line = |text: &str| lines.iter().position(|l| l == text).unwrap_or_else(|| panic!("{text:?} not in {lines:#?}"));
        let town = place_line(TOWN, app.world().resource::<BiomeDraws>());
        assert!(line(&town) < line("Press ? for the controls."), "{lines:#?}");
    }

    /// How to see the keys is said once a run, on the first arrival in
    /// Mereholt, and never again however the adventurer comes back: up
    /// through the gate, or home by a waystone.
    #[test]
    fn the_controls_line_is_said_once_a_run_however_often_mereholt_is_entered() {
        let mut app = headless(RunSeed(3));
        let me = arrive(&mut app, TOWN);
        let gate = ways_here(&mut app)[0].0;
        stand_on(&mut app, me, gate);
        go_through(&mut app, me);
        go_through(&mut app, me);
        assert_eq!(here(&app), TOWN, "up through the gate");
        stand_on(&mut app, me, gate);
        go_through(&mut app, me);
        let stone = crate::testing::make(&mut app, crate::waystone::WAYSTONE);
        crate::testing::give(&mut app, me, stone);
        crate::testing::use_thing(&mut app, me, stone);
        for _ in 0..40 {
            app.update();
        }
        assert_eq!(here(&app), TOWN, "home by a waystone");
        let said = crate::testing::said(&app);
        assert_eq!(said.iter().filter(|l| *l == "Press ? for the controls.").count(), 1, "{said:#?}");
        let town = place_line(TOWN, app.world().resource::<BiomeDraws>());
        assert_eq!(said.iter().filter(|l| **l == town).count(), 3, "while the town's own line is said on every arrival");

        // A run begun below, as a photograph's is, says it on that first
        // floor, and not again on the town's first arrival by a waystone.
        let mut app = headless(RunSeed(3));
        app.insert_resource(crate::run::StartFloor(2));
        let me = crate::testing::adventurer(&mut app);
        assert_eq!(here(&app), map_of(2), "begun on floor two");
        // Nobody to break the call off: a jackal near the stair on some
        // seeds is a fight, not what this asks.
        crate::testing::clear_monsters(&mut app);
        let stone = crate::testing::make(&mut app, crate::waystone::WAYSTONE);
        crate::testing::give(&mut app, me, stone);
        crate::testing::use_thing(&mut app, me, stone);
        for _ in 0..40 {
            app.update();
        }
        assert_eq!(here(&app), TOWN, "home by a waystone, the town's first arrival");
        let said = crate::testing::said(&app);
        let controls = said.iter().position(|l| l == "Press ? for the controls.").expect("said on the first floor");
        let floor_two = place_line(map_of(2), app.world().resource::<BiomeDraws>());
        assert_eq!(said[controls - 1], floor_two, "under floor two's line: {said:#?}");
        assert_eq!(said.iter().filter(|l| *l == "Press ? for the controls.").count(), 1, "once: {said:#?}");
    }

    /// The adventurer lives in Mereholt, so the first arrival there knows
    /// the whole of it, every cell of the town remembered from the first
    /// frame, while sight and light still decide what is seen: the town is
    /// known, not all in view. A floor below is learned as it is seen, and
    /// the town is as well known on the way back up.
    #[test]
    fn the_whole_of_mereholt_is_remembered_on_arrival() {
        for s in 0..3 {
            let mut app = headless(RunSeed(s));
            let me = crate::testing::adventurer(&mut app);
            let town = app.world().resource::<WorldMap>().place(TOWN).expect("Mereholt was built").terrain.bounds();
            let unknown = |app: &App| {
                let knowledge = app.world().resource::<Knowledge>();
                town.cells().filter(|p| !knowledge.is_explored(*p)).count()
            };
            assert_eq!((here(&app), unknown(&app)), (TOWN, 0), "seed {s}: every cell of Mereholt is known on arrival");
            let sight = app.world().get::<Viewshed>(me).expect("eyes");
            assert!(town.cells().any(|p| !sight.can_see(p)), "seed {s}: known, not seen: some of the town is out of sight");

            arrive(&mut app, map_of(1));
            let floor = app.world().resource::<WorldMap>().place(map_of(1)).expect("floor one").terrain.bounds();
            let known = app.world().resource::<Knowledge>().explored_count();
            assert!(known > 0 && known < floor.area() as usize / 2, "seed {s}: floor one is learned as it is seen, {known} cells of it so far");
            arrive(&mut app, TOWN);
            assert_eq!(unknown(&app), 0, "seed {s}: and Mereholt is as well known coming back");
        }
    }

    /// The log names the place on every arrival, not only the first, so
    /// coming back up to the town says where the adventurer is too.
    #[test]
    fn every_arrival_tells_the_places_line() {
        let mut app = headless(RunSeed(1));
        let me = arrive(&mut app, TOWN);
        let gate = ways_here(&mut app)[0].0;
        stand_on(&mut app, me, gate);
        go_through(&mut app, me);
        go_through(&mut app, me);
        let lines: Vec<String> = app.world().resource::<MessageLog>().iter().map(|e| e.text.clone()).collect();
        let draws = app.world().resource::<BiomeDraws>().clone();
        let (town, one) = (place_line(TOWN, &draws), place_line(map_of(1), &draws));
        let said: Vec<&String> = lines.iter().filter(|l| **l == town || **l == one).collect();
        assert_eq!(said, [&town, &one, &town], "{lines:#?}");
        // Floor one's line is one of its three biomes' own first lines,
        // as `assets/biomes.ron` writes them.
        let firsts = [
            "Floor 1: the Upper Halls. Braziers still burn in the empty halls.",
            "Floor 1: the Cellars. Barrels and racks stand in the dust.",
            "Floor 1: the Watch Barracks. Bunks stand in rows for a watch that never came back.",
        ];
        assert!(firsts.contains(&one.as_str()), "{one}");
    }
}
