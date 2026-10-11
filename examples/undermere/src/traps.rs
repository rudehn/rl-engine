//! What a floor hides, and the log's words for finding it.
//!
//! `assets/hidden.ron` says how many of each a floor hides and how hard
//! each is to see; [`Hiding::openings`] turns it into the
//! [`HideOpenings`] pass `dungeon.rs`'s chain walls a floor's secret doors
//! and hidden rooms over with. Finding any of it is the engine's one roll,
//! weighed as `plugin.rs` sets it on `PropsPlugin`, with the adventurer's
//! own eye, `Keen`, kept by [`crate::stats::keep_keen`]; [`tell_spotted`]
//! says what was found.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_mapgen::HideOpenings;

use crate::dungeon::FLOORS;

/// How the spotting roll weighs a chance: whole within two cells, half
/// out to seven and nothing beyond, so a secret is found by going near it
/// and not by glancing down a hall at it; and tripled on a turn spent
/// waiting, so the wait key the adventurer rests with is also how they
/// search a wall they suspect. There is no search key: walking past still
/// rolls, so nothing is found only by pressing one at every wall.
pub const SPOTTING: SpotRules = SpotRules { near: 2, far: 7, still: 3 };

/// `assets/hidden.ron`, compiled in so the binary runs from anywhere.
const HIDDEN_RON: &str = include_str!("../assets/hidden.ron");

/// A floor's secret doors: loops' openings walled over.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SecretDoors {
    /// The first floor with any.
    from: u32,
    /// The fewest and the most a floor tries for.
    count: (u32, u32),
    /// The chance a turn of spotting one, in percent.
    spot: u8,
}

/// A floor's hidden rooms: dead-end rooms' one doors walled over.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HiddenRooms {
    /// The fewest and the most a floor tries for.
    count: (u32, u32),
    /// The chance a turn of spotting one's door, in percent.
    spot: u8,
}

/// What `assets/hidden.ron` says a floor hides.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hiding {
    /// Loops' openings walled over.
    secret_doors: SecretDoors,
    /// Dead-end rooms walled in.
    hidden_rooms: HiddenRooms,
}

impl Hiding {
    /// Reads `assets/hidden.ron`, and refuses a range whose fewest is past
    /// its most or a chance past certain, naming the field: a typo there
    /// would otherwise hide nothing, or panic on the first floor built.
    pub fn load() -> Self {
        let hiding: Self = ron::from_str(HIDDEN_RON).unwrap_or_else(|e| panic!("assets/hidden.ron: {e}"));
        let ranges = [("secret_doors", hiding.secret_doors.count), ("hidden_rooms", hiding.hidden_rooms.count)];
        if let Some((name, _)) = ranges.iter().find(|(_, (fewest, most))| fewest > most) {
            panic!("assets/hidden.ron: {name}'s fewest is past its most");
        }
        if hiding.secret_doors.spot > 100 || hiding.hidden_rooms.spot > 100 {
            panic!("assets/hidden.ron: a chance is past a hundred");
        }
        hiding
    }

    /// The pass that walls `floor`'s secret doors and hidden rooms over,
    /// with at least `hidden_picks` rooms, one for each of the floor's
    /// pool picks that prefers a hidden one, so a hidden cache always has
    /// its room or the floor is grown again. `None` on the throne's floor,
    /// which hides nothing.
    pub fn openings(&self, floor: u32, hidden_picks: u32) -> Option<HideOpenings> {
        if floor >= FLOORS {
            return None;
        }
        let (doors, rooms) = (self.secret_doors, self.hidden_rooms);
        let loops = if floor >= doors.from { doors.count } else { (0, 0) };
        Some(HideOpenings {
            loops,
            rooms: (rooms.count.0.max(hidden_picks), rooms.count.1.max(hidden_picks)),
            loop_spot: doors.spot,
            room_spot: rooms.spot,
            at_least_rooms: hidden_picks,
        })
    }
}

/// Says in the log what the adventurer spotted: a hidden thing by its
/// name, a secret by what it turned out to be, a door or the causeway
/// under the water.
///
/// In `TurnSet::React`, the pass after the roll that found it.
pub fn tell_spotted(
    mut spotted: MessageReader<Spotted>,
    mut tell: MessageWriter<Tell>,
    registries: Res<Registries>,
    map: Res<WorldMap>,
    kinds: Query<&PropKind>,
) {
    for found in spotted.read() {
        let line = match found.what {
            Finding::Prop(prop) => {
                let Ok(kind) = kinds.get(prop) else { continue };
                format!("You spot {}.", rl_engine::rl_core::noun::with_article(&registries.props.get(kind.0).name))
            }
            Finding::Secret { is, .. } => match map.tables().names[is.index()].as_str() {
                "sunken causeway" => "You make out a causeway just under the water.".to_string(),
                _ if map.tables().opens[is.index()].is_some() => "You find a secret door.".to_string(),
                _ => "You find a hidden way through.".to_string(),
            },
        };
        tell.write(Tell::new(line, Tones::NOTICE));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rl_engine::rl_bevy::places::PlaceRules;
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::{Undermere, map_of};
    use crate::testing::{arrive, clear_monsters, here, pass_turns, stand_on, sturdy};

    /// The groups of `spot` among `secrets`: one per opening hidden.
    fn groups(secrets: &[rl_engine::rl_mapgen::Secret], spot: u8) -> usize {
        secrets.iter().filter(|s| s.spot == spot).map(|s| s.group).collect::<BTreeSet<_>>().len()
    }

    /// Over a span of seeds: no secret door on floor one, never more than
    /// three on any floor, and one or more on most floors from two; never
    /// more hidden rooms than two; nothing hidden on the throne's floor.
    #[test]
    fn secret_doors_and_hidden_rooms_by_depth_over_seeds() {
        let hiding = Hiding::load();
        let (door, room) = (hiding.secret_doors.spot, hiding.hidden_rooms.spot);
        assert_ne!(door, room, "a door and a room are told apart by their chance");
        let (mut floors, mut with_doors) = (0, 0);
        for s in 0..24 {
            let undermere = Undermere::new(RunSeed(s));
            for floor in 1..=FLOORS {
                let built = undermere.build(map_of(floor), None).unwrap();
                let (doors, rooms) = (groups(&built.secrets, door), groups(&built.secrets, room));
                assert!(rooms <= 2, "floor {floor}, seed {s}: {rooms} hidden rooms");
                match floor {
                    1 => assert_eq!(doors, 0, "seed {s}: a secret door on floor one"),
                    FLOORS => assert!(built.secrets.is_empty(), "seed {s}: something hidden on the throne's floor"),
                    _ => {
                        assert!(doors <= 3, "floor {floor}, seed {s}: {doors} secret doors");
                        floors += 1;
                        with_doors += usize::from(doors > 0);
                    }
                }
            }
        }
        assert!(with_doors * 10 >= floors * 8, "only {with_doors} of {floors} floors from two on have a secret door");
    }

    /// Standing beside a secret door and waiting, at the base chances with
    /// no ring, the adventurer finds it within twenty turns on at least
    /// nine seeds in ten: waiting is searching, and a wall suspected is a
    /// wall found.
    #[test]
    fn beside_a_secret_door_a_waiting_adventurer_finds_it_within_twenty_turns_on_nine_seeds_in_ten() {
        let door = Hiding::load().secret_doors.spot;
        let mut found = 0;
        for s in 0..10 {
            let mut app = crate::testing::headless(RunSeed(s));
            let me = (2..FLOORS)
                .find_map(|floor| {
                    let me = arrive(&mut app, map_of(floor));
                    let map = app.world().resource::<WorldMap>();
                    map.secrets(here(&app)).iter().any(|x| x.spot == door).then_some(me)
                })
                .unwrap_or_else(|| panic!("seed {s}: no floor with a secret door"));
            let map = app.world().resource::<WorldMap>();
            let (on, secret) = (here(&app), *map.secrets(here(&app)).iter().find(|x| x.spot == door).expect("just found"));
            let beside = Direction::CARDINALS
                .iter()
                .map(|d| secret.at + d.offset())
                .find(|p| map.is_walkable_on(on, *p))
                .unwrap_or_else(|| panic!("seed {s}: nothing to stand on beside {:?}", secret.at));
            sturdy(&mut app, me);
            clear_monsters(&mut app);
            stand_on(&mut app, me, beside);
            for _ in 0..20 {
                pass_turns(&mut app, 1);
                if !app.world().resource::<WorldMap>().secrets(on).iter().any(|x| x.group == secret.group) {
                    found += 1;
                    break;
                }
            }
        }
        assert!(found >= 9, "found on only {found} seeds in ten");
    }
}
