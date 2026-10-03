//! The ways between places, and the line the log gives each place as it
//! is entered.
//!
//! A place's builder reports a way in and a way on; this stands a way
//! through on each the first time the place is entered, the way Foundry's
//! lifts are laid, so the way on is always as far from the arrival as the
//! place allows. In Mereholt the way on is the Drowned Gate, which leads to
//! floor one's way in; on floor one the way up is the gate's far side, and
//! leads back onto the gate; every other floor has stairs up at its way in
//! and down at its way on, and the tenth has no way down, since the throne
//! is where the Undermere ends.
//!
//! A way through is an entity, a [`Transition`] where the engine looks for
//! one when a body goes through, marked with the [`Way`] it is. Stairs are
//! drawn by a glyph of their own, as Foundry's lifts are. The gate is drawn
//! by the tile it stands on, which shimmers and glows violet on both its
//! sides, so it carries no glyph to hide the tile under.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::dungeon::{Biome, FLOORS, TOWN, floor_of, map_of};

/// What stairs are drawn in: a pale stone, so a way on reads against every
/// band's floor.
pub const STAIRS: Color = Color::srgb(0.92, 0.86, 0.62);

/// A way through: what a save writes down to draw one again, and how each
/// is told apart from the others standing on a place.
///
/// A closed list, since the ways are the game's own: where each leads is
/// its [`Transition`], which the engine keeps.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Way {
    /// Stairs up, at the way in of every floor but the first.
    Up,
    /// Stairs down, at a floor's way on.
    Down,
    /// The Drowned Gate: the town's way on, and floor one's way up.
    Gate,
}

impl Way {
    /// What the look cursor and the nearby rail call it.
    pub fn name(self) -> &'static str {
        match self {
            Way::Up => "stairs up",
            Way::Down => "stairs down",
            Way::Gate => "the Drowned Gate",
        }
    }

    /// How it is drawn, if not by the tile under it: the gate is its tile.
    pub fn glyph(self) -> Option<Glyph> {
        match self {
            Way::Up => Some(Glyph::new('<', STAIRS).on_layer(1)),
            Way::Down => Some(Glyph::new('>', STAIRS).on_layer(1)),
            Way::Gate => None,
        }
    }
}

/// Stands `way` at `at` on `map`, leading `to`.
fn lay(commands: &mut Commands, way: Way, at: Point, map: MapId, to: Destination) {
    let mut e = commands.spawn((Position(at), OnMap(map), Transition { to }, way, Name::new(way.name())));
    if let Some(glyph) = way.glyph() {
        e.insert(glyph);
    }
}

/// What the log says on entering `map`, the town or a floor: where the
/// adventurer is, and what the place is like.
pub fn place_line(map: MapId) -> String {
    let Some(floor) = floor_of(map) else {
        return "Mereholt, at dusk, on the shore of the black lake.".to_string();
    };
    let mood = match floor {
        1 => "Braziers still burn in the empty halls.",
        2 => "The masonry is older here, and cracked.",
        3 => "Something glows in the dark.",
        4 => "The caps grow thick here, and the air tastes of earth.",
        5 => "Candles burn for the dead.",
        6 => "The dead lie deeper here, and not all of them lie still.",
        7 => "Cold water stands in the nave.",
        8 => "Pale light falls through the water overhead.",
        9 => "The air shimmers over pools of fire.",
        _ => "Something waits on the throne.",
    };
    format!("Floor {floor}: {}. {mood}", Biome::of(floor).name())
}

/// Lays a place's ways through the first time it is entered, and names the
/// place in the log on every arrival, coming up as well as going down.
pub fn link_places(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, mut tell: MessageWriter<Tell>) {
    for ev in entered.read() {
        tell.write(Tell::new(place_line(ev.map), Tones::NOTICE));
        if !ev.first {
            continue;
        }
        let Some(floor) = floor_of(ev.map) else {
            // The town: the gate, its way on, leads to floor one's way in.
            if ev.map == TOWN
                && let Some(gate) = ev.exit
            {
                lay(&mut commands, Way::Gate, gate, ev.map, Destination::Place { map: map_of(1), arrive: Arrive::Entry });
            }
            continue;
        };
        // Floor one's way up is the gate's far side, onto the gate; every
        // other floor's goes up the stairs to the way down it came by.
        let up = match floor {
            1 => (Way::Gate, Destination::Place { map: TOWN, arrive: Arrive::Exit }),
            _ => (Way::Up, Destination::Place { map: map_of(floor - 1), arrive: Arrive::Exit }),
        };
        lay(&mut commands, up.0, ev.entry, ev.map, up.1);
        if let Some(down) = ev.exit.filter(|_| floor < FLOORS) {
            lay(&mut commands, Way::Down, down, ev.map, Destination::Place { map: map_of(floor + 1), arrive: Arrive::Entry });
        }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::{FLOORS, TOWN, map_of};
    use crate::testing::{arrive, go_through, headless, stand_on};

    /// Every way through on the map being read: where it stands, where it
    /// leads, and which way it is.
    fn ways_here(app: &mut App) -> Vec<(Point, Destination, Way)> {
        let here = app.world().resource::<WorldMap>().current();
        let mut q = app.world_mut().query::<(&Position, &OnMap, &Transition, &Way)>();
        let mut ways: Vec<(Point, Destination, Way)> = q.iter(app.world()).filter(|(_, on, _, _)| on.0 == here).map(|(p, _, t, w)| (p.0, t.to, *w)).collect();
        ways.sort_by_key(|(p, _, _)| *p);
        ways
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

    /// Down every stair from the town to the throne: each floor's way down
    /// leads to the next floor's way in, and each floor below the first has
    /// a way up to the one above's way down.
    #[test]
    fn every_way_down_leads_to_the_next_floor_and_every_way_up_back_to_the_last() {
        for s in 0..2 {
            let mut app = headless(RunSeed(s));
            let me = arrive(&mut app, TOWN);
            let gate = ways_here(&mut app)[0].0;
            stand_on(&mut app, me, gate);
            go_through(&mut app, me);
            for floor in 1..FLOORS {
                let down =
                    ways_here(&mut app).into_iter().find(|(_, _, w)| *w == Way::Down).unwrap_or_else(|| panic!("seed {s}: floor {floor} has a way down"));
                assert_eq!(down.1, Destination::Place { map: map_of(floor + 1), arrive: Arrive::Entry }, "seed {s}, floor {floor}");
                stand_on(&mut app, me, down.0);
                go_through(&mut app, me);
                assert_eq!(here(&app), map_of(floor + 1), "seed {s}: the way down from floor {floor}");
                let up =
                    ways_here(&mut app).into_iter().find(|(_, _, w)| *w == Way::Up).unwrap_or_else(|| panic!("seed {s}: floor {} has a way up", floor + 1));
                assert_eq!((up.0, up.1), (at(&app, me), Destination::Place { map: map_of(floor), arrive: Arrive::Exit }), "seed {s}, floor {}", floor + 1);
            }
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
        let (town, one) = (place_line(TOWN), place_line(map_of(1)));
        let said: Vec<&String> = lines.iter().filter(|l| **l == town || **l == one).collect();
        assert_eq!(said, [&town, &one, &town], "{lines:#?}");
        assert_eq!(place_line(map_of(3)), "Floor 3: the Fungal Deeps. Something glows in the dark.");
    }
}
