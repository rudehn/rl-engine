//! What a floor hides, and the log's words for finding it.
//!
//! `assets/hidden.ron` says how many of each a floor hides and how hard
//! each is to see; [`Hiding::openings`] turns it into the
//! [`HideOpenings`] pass `dungeon.rs`'s chain walls a floor's secret doors
//! and hidden rooms over with, and [`Hiding::laying`] into the
//! [`LayHidden`] pass that marks where its traps and loose flagstones go,
//! which [`stand_hidden`] stands up hidden on the floor's first arrival.
//!
//! Finding any of it is the engine's one roll, weighed by [`SPOTTING`] on
//! `PropsPlugin`, with the adventurer's own eye, `Keen`, kept by
//! [`crate::stats::keep_keen`]; [`tell_spotted`] says what was found. A
//! trap is also found by going off: [`show_sprung`] shows one the
//! adventurer set off or saw set off, since the engine's own trap stays
//! hidden when it springs.

use std::collections::BTreeSet;

use bevy::prelude::*;
use rand::Rng;
use rand::seq::{IndexedRandom, SliceRandom};
use rl_engine::prelude::*;
use rl_engine::rl_bevy::places::PlaceEntered;
use rl_engine::rl_core::Rect;
use rl_engine::rl_mapgen::dungeon::{ExitPoint, Room};
use rl_engine::rl_mapgen::passes::StartPoint;
use rl_engine::rl_mapgen::{Area, HideOpenings, Join, Opening, Secret, Surveyed};

use crate::dungeon::{FLOORS, MIN_DISTANCE_FROM_ENTRY};

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

/// How many traps a band of floors gets.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TrapCount {
    /// The first and the last floor of the band.
    floors: (u32, u32),
    /// The fewest and the most a floor of it gets.
    count: (u32, u32),
}

/// One kind of trap, and the floors it is laid on.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TrapKind {
    /// The prop it is.
    prop: String,
    /// The first floor it is laid on.
    from: u32,
    /// The last, when it stops short of the deepest.
    #[serde(default)]
    until: Option<u32>,
}

impl TrapKind {
    /// Whether it is laid on `floor`.
    fn on(&self, floor: u32) -> bool {
        floor >= self.from && self.until.is_none_or(|u| floor <= u)
    }
}

/// A floor's traps.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Traps {
    /// How many, by band.
    counts: Vec<TrapCount>,
    /// Which, by depth.
    kinds: Vec<TrapKind>,
}

/// A floor's loose flagstones.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Flagstones {
    /// The prop one is.
    prop: String,
    /// The fewest and the most a floor gets.
    count: (u32, u32),
}

/// What `assets/hidden.ron` says a floor hides: read once by `dungeon.rs`
/// to build the floors, and kept as a resource by the plugin to stand up
/// what they laid.
#[derive(Resource, Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hiding {
    /// Loops' openings walled over.
    secret_doors: SecretDoors,
    /// Dead-end rooms walled in.
    hidden_rooms: HiddenRooms,
    /// Traps on the corridors and doorways.
    traps: Traps,
    /// Loose flagstones in the rooms.
    flagstones: Flagstones,
}

impl Hiding {
    /// Reads `assets/hidden.ron`, and refuses a range whose fewest is past
    /// its most or a chance past certain, naming the field: a typo there
    /// would otherwise hide nothing, or panic on the first floor built.
    pub fn load() -> Self {
        let hiding: Self = ron::from_str(HIDDEN_RON).unwrap_or_else(|e| panic!("assets/hidden.ron: {e}"));
        let mut ranges =
            vec![("secret_doors", hiding.secret_doors.count), ("hidden_rooms", hiding.hidden_rooms.count), ("flagstones", hiding.flagstones.count)];
        ranges.extend(hiding.traps.counts.iter().flat_map(|c| [("a band of traps' floors", c.floors), ("a band of traps", c.count)]));
        ranges.extend(hiding.traps.kinds.iter().filter_map(|k| k.until.map(|u| ("a trap's floors", (k.from, u)))));
        if let Some((name, _)) = ranges.iter().find(|(_, (fewest, most))| fewest > most) {
            panic!("assets/hidden.ron: {name}: the fewest is past the most");
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

    /// Every prop this file lays, the traps' kinds in order and the
    /// flagstone last: what the spot [`FIRST_TAG`] plus its place in this
    /// list stands up.
    pub fn props(&self) -> Vec<&str> {
        self.traps.kinds.iter().map(|k| k.prop.as_str()).chain([self.flagstones.prop.as_str()]).collect()
    }

    /// The prop a spot tagged `tag` stands up, if this file laid it.
    pub fn prop_of(&self, tag: u32) -> Option<&str> {
        let index = tag.checked_sub(FIRST_TAG)?;
        self.props().get(usize::try_from(index).ok()?).copied()
    }

    /// The pass that lays `floor`'s traps and flagstones. `None` on the
    /// throne's floor, which hides nothing.
    pub fn laying(&self, floor: u32) -> Option<LayHidden> {
        if floor >= FLOORS {
            return None;
        }
        let traps = self.traps.counts.iter().find(|c| (c.floors.0..=c.floors.1).contains(&floor)).map_or((0, 0), |c| c.count);
        let kinds: Vec<char> = self.traps.kinds.iter().enumerate().filter(|(_, k)| k.on(floor)).map(|(i, _)| glyph(i)).collect();
        let traps = if kinds.is_empty() { (0, 0) } else { traps };
        Some(LayHidden { traps, kinds, flagstones: self.flagstones.count, flagstone: glyph(self.traps.kinds.len()) })
    }
}

/// The tag of the first thing [`Hiding::props`] names, as a stamp's mark:
/// the first character of a private-use plane, so no piece's or
/// dressing's legend glyph is ever read as one, and each is still a
/// `char` a mark can carry.
pub const FIRST_TAG: u32 = 0xF_0000;

/// The mark of [`Hiding::props`]'s `index`th.
fn glyph(index: usize) -> char {
    char::from_u32(FIRST_TAG + index as u32).expect("the private-use plane holds every prop the file names")
}

/// Whether a spot tagged `tag` is one [`LayHidden`] laid, for a printed
/// floor to draw it as a trap whatever it is.
pub fn is_hidden_tag(tag: u32) -> bool {
    (FIRST_TAG..FIRST_TAG + 0x1_0000).contains(&tag)
}

/// Lays a floor's traps and loose flagstones, each a one-cell stamp whose
/// mark is its [`FIRST_TAG`] tag, stood up hidden on the floor's first
/// arrival by [`stand_hidden`].
///
/// A trap stands where a body has to pass: a corridor, an area the survey
/// calls a chokepoint, or a doorway, an arch's own cell or the cell either
/// side of any opening. Never within [`MIN_DISTANCE_FROM_ENTRY`] of the way
/// in, so the first steps onto a floor are never a trap's; never on or
/// beside the way down, which is a way through and not a place to stop;
/// never on a door, deep or slow ground, or anything a piece or a dressing
/// laid; and never within [`TRAP_SPACING`] of another, so a corridor is not
/// a row of plates. Each is drawn evenly among the kinds the floor has
/// unlocked.
///
/// A flagstone stands in a room: first in a hidden room nothing else stands
/// in, so a pocket found behind a secret door is never empty while there is
/// a flagstone to give it, then anywhere a room's floor is plain.
///
/// Runs in [`Phase::Finish`], after the survey, the secrets and the
/// stamps, all of which it reads.
pub struct LayHidden {
    /// The fewest and the most traps.
    traps: (u32, u32),
    /// The marks of the kinds the floor has unlocked.
    kinds: Vec<char>,
    /// The fewest and the most flagstones.
    flagstones: (u32, u32),
    /// A flagstone's mark.
    flagstone: char,
}

/// The fewest cells, Chebyshev, between two traps.
pub const TRAP_SPACING: i32 = 3;

impl Pass<BaseContext> for LayHidden {
    fn name(&self) -> &'static str {
        "hidden things"
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let Some(ground) = Ground::read(ctx) else { return Ok(()) };
        let mut cells: Vec<Point> = ground.trap_cells().into_iter().collect();
        cells.shuffle(ctx.rng());
        let count = ctx.rng().random_range(self.traps.0..=self.traps.1) as usize;
        let mut laid: Vec<Point> = Vec::new();
        for p in cells {
            if laid.len() == count {
                break;
            }
            if laid.iter().all(|q| geometry::chebyshev(*q, p) >= TRAP_SPACING) {
                laid.push(p);
            }
        }
        for p in &laid {
            let kind = self.kinds[ctx.rng().random_range(0..self.kinds.len())];
            mark(ctx, kind, *p);
        }
        let count = ctx.rng().random_range(self.flagstones.0..=self.flagstones.1) as usize;
        let (pockets, mut rooms) = ground.flagstone_cells();
        let mut stones: Vec<Point> = Vec::new();
        for pocket in pockets {
            if let Some(p) = pocket.choose(ctx.rng()) {
                stones.push(*p);
            }
        }
        rooms.shuffle(ctx.rng());
        stones.extend(rooms);
        // A pocket's cell is a room's too, so each cell is taken once.
        let mut taken: BTreeSet<Point> = laid.into_iter().collect();
        for p in stones.into_iter().filter(|p| taken.insert(*p)).take(count) {
            mark(ctx, self.flagstone, p);
        }
        Ok(())
    }
}

/// Stamps a one-cell mark `glyph` at `at`, for no piece.
fn mark(ctx: &mut BaseContext, glyph: char, at: Point) {
    ctx.emit(Stamped { bounds: Rect::new(at.x, at.y, 1, 1), marks: vec![(glyph, at)], prefab: None });
}

/// What [`LayHidden`] reads of a floor: copied out of the context, so the
/// pass may draw from its stream while it reads.
struct Ground {
    /// Each cell plain enough to stand a hidden thing on, clear of every
    /// way and stamp.
    clear: BTreeSet<Point>,
    /// The cells farther than [`MIN_DISTANCE_FROM_ENTRY`] from the way in.
    far: BTreeSet<Point>,
    /// The floor's areas, none painted from a prefab.
    areas: Vec<Area>,
    /// Which of them is a room.
    room: Vec<bool>,
    /// Which a chokepoint.
    chokepoint: Vec<bool>,
    /// Which a hidden room: the dead end behind a secret.
    hidden: Vec<bool>,
    /// The cells of every opening between two areas, locked and sealed
    /// ones and a piece's own left out.
    doorways: Vec<Point>,
    /// The bounds of every stamp.
    stamps: Vec<Rect>,
}

impl Ground {
    /// Reads `ctx`; `None` for a floor the survey never measured.
    fn read(ctx: &BaseContext) -> Option<Self> {
        let survey = ctx.outputs().first::<Surveyed>()?.clone();
        let entry = ctx.outputs().first::<StartPoint>()?.0;
        let exit = ctx.outputs().first::<ExitPoint>().map(|e| e.0);
        let tables = ctx.tiles().tables();
        let stamps: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|s| s.bounds).collect();
        let marks: BTreeSet<Point> = ctx.outputs().iter::<Stamped>().flat_map(|s| s.marks.iter().map(|(_, p)| *p)).collect();
        let painted: BTreeSet<Point> = ctx.outputs().iter::<Area>().filter(|a| a.entries.is_some()).flat_map(|a| a.cells.iter().copied()).collect();
        let plain = |p: Point| {
            ctx.terrain().get(p).map(|t| t.index()).is_some_and(|i| {
                tables.walkable[i] && tables.move_cost[i] <= 100 && tables.opens[i].is_none() && tables.closes[i].is_none() && tables.burns[i].is_none()
            })
        };
        let clear: BTreeSet<Point> = ctx
            .terrain()
            .bounds()
            .cells()
            .filter(|p| plain(*p) && !marks.contains(p) && !painted.contains(p) && stamps.iter().all(|r| !r.contains(*p)))
            .filter(|p| *p != entry && exit.is_none_or(|e| geometry::chebyshev(*p, e) > 1))
            .collect();
        let far = clear.iter().copied().filter(|p| geometry::chebyshev(*p, entry) > MIN_DISTANCE_FROM_ENTRY).collect();
        let rooms: Vec<Rect> = ctx.outputs().iter::<Room>().map(|r| r.0).collect();
        let areas: Vec<Area> = ctx.outputs().iter::<Area>().filter(|a| a.entries.is_none()).cloned().collect();
        let room = areas.iter().map(|a| a.cells.iter().any(|c| rooms.iter().any(|r| r.contains(*c)))).collect();
        let facts = |a: &Area, read: fn(&rl_engine::rl_mapgen::AreaFacts) -> bool| survey.areas.get(a.index).is_some_and(read);
        let chokepoint = areas.iter().map(|a| facts(a, |f| f.chokepoint)).collect();
        let behind: BTreeSet<usize> = ctx.outputs().iter::<Secret>().flat_map(|s| [s.joins.0, s.joins.1]).collect();
        let hidden = areas.iter().map(|a| facts(a, |f| f.dead_end) && behind.contains(&a.index)).collect();
        // A piece's own entries were drawn by hand, and a lock or a seal
        // is already what keeps its doorway: neither is a trap's.
        let pieces: BTreeSet<usize> = ctx.outputs().iter::<Area>().filter(|a| a.entries.is_some()).map(|a| a.index).collect();
        let doorways = ctx
            .outputs()
            .iter::<Opening>()
            .filter(|o| !matches!(o.join, Join::Locked(_) | Join::Sealed) && !pieces.contains(&o.areas.0) && !pieces.contains(&o.areas.1))
            .flat_map(|o| o.cells.iter().copied())
            .collect();
        Some(Self { clear, far, areas, room, chokepoint, hidden, doorways, stamps })
    }

    /// Every cell a trap may stand on: a corridor's, a chokepoint's, or a
    /// doorway's or beside one, clear and far enough from the way in.
    fn trap_cells(&self) -> BTreeSet<Point> {
        let mut cells: BTreeSet<Point> = BTreeSet::new();
        for (i, area) in self.areas.iter().enumerate() {
            if !self.room[i] || self.chokepoint[i] {
                cells.extend(area.cells.iter().copied());
            }
        }
        for door in &self.doorways {
            cells.insert(*door);
            cells.extend(Direction::CARDINALS.iter().map(|d| *door + d.offset()));
        }
        cells.retain(|p| self.far.contains(p));
        cells
    }

    /// Where flagstones may go: one list for each hidden room no stamp
    /// stands in, and every clear cell of every room besides.
    fn flagstone_cells(&self) -> (Vec<Vec<Point>>, Vec<Point>) {
        let clear = |a: &Area| a.cells.iter().copied().filter(|p| self.clear.contains(p)).collect::<Vec<Point>>();
        let empty = |a: &Area| a.cells.iter().all(|p| self.stamps.iter().all(|r| !r.contains(*p)));
        let pockets = self.areas.iter().enumerate().filter(|(i, a)| self.hidden[*i] && empty(a)).map(|(_, a)| clear(a)).collect();
        let rooms = self.areas.iter().enumerate().filter(|(i, _)| self.room[*i]).flat_map(|(_, a)| clear(a)).collect();
        (pockets, rooms)
    }
}

/// Stands up every trap and flagstone [`LayHidden`] laid, on the arrival
/// that first finds the floor, each hidden as its prop's definition says.
/// A floor entered again keeps what the save or the run before kept: the
/// spots are only read on the first arrival, as every piece's are.
///
/// In `TurnSet::React`, beside the vaults' own marks.
pub fn stand_hidden(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, map: Res<WorldMap>, hiding: Res<Hiding>, registries: Res<Registries>) {
    for ev in entered.read().filter(|ev| ev.first) {
        let Some(place) = map.place(ev.map) else { continue };
        for spot in place.spots.iter().filter(|s| s.prefab.is_none()) {
            if let Some(name) = hiding.prop_of(spot.tag) {
                spawn_prop(&mut commands, &registries, registries.props.expect(name), spot.at, ev.map);
            }
        }
    }
}

/// A hidden trap that goes off is hidden no longer when the adventurer
/// set it off or saw it go off under someone else: a goblin caught in a
/// net in plain sight shows where the net was. One gone off out of sight
/// stays hidden, since nobody saw it.
///
/// In `TurnSet::React`, the pass the trap went off in.
pub fn show_sprung(
    mut commands: Commands,
    mut fired: MessageReader<Fired>,
    mut tell: MessageWriter<Tell>,
    registries: Res<Registries>,
    hidden: Query<&PropKind, With<Hidden>>,
    player: Query<(Entity, &Viewshed), With<Player>>,
    names: Query<&Name>,
) {
    let Ok((me, sight)) = player.single() else { return };
    for ev in fired.read().filter(|ev| ev.moment == Moments::ENTERED) {
        let (Ok(kind), Some(by)) = (hidden.get(ev.on), ev.by) else { continue };
        let what = &registries.props.get(kind.0).name;
        let line = if by == me {
            format!("You set off {}!", rl_engine::rl_core::noun::with_article(what))
        } else if sight.can_see(ev.at) {
            let who = names.get(by).map_or_else(|_| "something".to_string(), |n| rl_engine::rl_core::noun::with_the(n.as_str()));
            format!("{} goes off under {who}!", crate::effects::capital(&rl_engine::rl_core::noun::with_article(what)))
        } else {
            continue;
        };
        commands.entity(ev.on).remove::<Hidden>();
        tell.write(Tell::new(line, Tones::NOTICE));
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
    use crate::testing::{arena_on, arrive, clear_monsters, here, monster, pass_turns, said, settle, stand_on, sturdy};

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

    /// Every trap and flagstone `built` laid, by the prop it stands up.
    fn hidden_things<'a>(hiding: &'a Hiding, built: &rl_engine::rl_bevy::places::PlaceBuild) -> Vec<(&'a str, Point)> {
        built.spots.iter().filter(|s| s.prefab.is_none()).filter_map(|s| hiding.prop_of(s.tag).map(|name| (name, s.at))).collect()
    }

    /// Over a span of seeds: each floor's traps number what its band says,
    /// a kind is never laid above the floor it is unlocked on, and there is
    /// no trapdoor on floor nine or anything hidden on the throne's floor;
    /// a floor's flagstones number what the file says.
    #[test]
    fn traps_and_flagstones_by_depth_over_seeds() {
        let hiding = Hiding::load();
        let flagstone = hiding.flagstones.prop.as_str();
        let (mut short, mut floors) = (0, 0);
        for s in 0..24 {
            let undermere = Undermere::new(RunSeed(s));
            for floor in 1..=FLOORS {
                let built = undermere.build(map_of(floor), None).unwrap();
                let things = hidden_things(&hiding, &built);
                let traps: Vec<&str> = things.iter().filter(|(name, _)| *name != flagstone).map(|(name, _)| *name).collect();
                let stones = things.len() - traps.len();
                if floor == FLOORS {
                    assert!(things.is_empty(), "seed {s}: something hidden on the throne's floor");
                    continue;
                }
                let band = hiding.traps.counts.iter().find(|c| (c.floors.0..=c.floors.1).contains(&floor)).expect("every floor has a band");
                assert!(traps.len() as u32 <= band.count.1, "floor {floor}, seed {s}: {} traps", traps.len());
                floors += 1;
                short += usize::from((traps.len() as u32) < band.count.0);
                for name in &traps {
                    let kind = hiding.traps.kinds.iter().find(|k| k.prop == *name).expect("a kind the file names");
                    assert!(kind.on(floor), "floor {floor}, seed {s}: a {name} laid outside its floors");
                }
                assert!(floor != 9 || !traps.contains(&"trapdoor"), "seed {s}: a trapdoor on floor nine");
                assert!(stones as u32 <= hiding.flagstones.count.1, "floor {floor}, seed {s}: {stones} flagstones");
            }
        }
        assert!(short * 10 <= floors, "{short} of {floors} floors had fewer traps than their band's fewest");
    }

    /// Over a span of seeds: every trap stands on a cell a body can walk
    /// on, off the stairs and farther than six cells from the way in, and
    /// every trap and flagstone on a cell walked to from the way in once
    /// the floor's secrets are found.
    #[test]
    fn every_trap_is_on_a_walkable_cell_off_the_stairs_and_every_hidden_thing_is_reached_once_the_secrets_are_found() {
        let hiding = Hiding::load();
        for s in 0..24 {
            let undermere = Undermere::new(RunSeed(s));
            let tables = undermere.tiles().tables();
            for floor in 1..FLOORS {
                let built = undermere.build(map_of(floor), None).unwrap();
                let mut found = built.terrain.clone();
                for secret in &built.secrets {
                    found.set(secret.at, secret.is);
                }
                let open = |p: Point| found.get(p).is_some_and(|t| tables.passable[t.index()]);
                let mut reached = BTreeSet::from([built.entry]);
                let mut frontier = vec![built.entry];
                while let Some(p) = frontier.pop() {
                    for d in Direction::CARDINALS {
                        let n = p + d.offset();
                        if open(n) && reached.insert(n) {
                            frontier.push(n);
                        }
                    }
                }
                for (name, at) in hidden_things(&hiding, &built) {
                    assert!(reached.contains(&at), "floor {floor}, seed {s}: the {name} at {at:?} is not reached");
                    if name == hiding.flagstones.prop {
                        continue;
                    }
                    let tile = built.terrain.get(at).expect("on the map");
                    assert!(tables.walkable[tile.index()], "floor {floor}, seed {s}: the {name} at {at:?} is not on walkable ground");
                    assert!(at != built.entry && Some(at) != built.exit, "floor {floor}, seed {s}: the {name} is on the stairs");
                    assert!(geometry::chebyshev(at, built.entry) > MIN_DISTANCE_FROM_ENTRY, "floor {floor}, seed {s}: the {name} is by the way in");
                }
            }
        }
    }

    /// Stands a `name` at `at` on the place being read, hidden so no roll
    /// will ever find it, and says which it is.
    fn lay_unseeable(app: &mut App, name: &'static str, at: Point) -> Entity {
        use bevy::ecs::system::RunSystemOnce;
        let map = here(app);
        let trap = app
            .world_mut()
            .run_system_once(move |mut commands: Commands, registries: Res<Registries>| {
                spawn_prop(&mut commands, &registries, registries.props.expect(name), at, map)
            })
            .expect("it runs");
        app.world_mut().entity_mut(trap).insert(Hidden { spot: 0 });
        settle(app);
        trap
    }

    /// A monster setting off a hidden trap in the adventurer's sight shows
    /// it, and says so; one set off out of sight stays hidden.
    #[test]
    fn a_monster_springing_a_trap_in_sight_shows_it_and_out_of_sight_it_stays_hidden() {
        let (mut app, me, here_at) = arena_on(3, 3);
        let near = here_at.offset(2, 0);
        let net = lay_unseeable(&mut app, "net trap", near);
        let goblin = monster(&mut app, "goblin", near.offset(1, 0));
        app.world_mut().write_message(Stepped { actor: goblin, from: near.offset(1, 0), to: near });
        settle(&mut app);
        assert!(app.world().get::<Hidden>(net).is_none(), "the net is shown");
        assert!(said(&app).iter().any(|l| l == "A net trap goes off under the goblin!"), "and the log says so: {:?}", said(&app));
        let sight = app.world().get::<Viewshed>(me).expect("the adventurer sees").clone();
        let map = app.world().resource::<WorldMap>();
        let far =
            map.place(map.current()).expect("a floor").terrain.bounds().cells().find(|p| map.is_walkable(*p) && !sight.can_see(*p)).expect("somewhere unseen");
        let dart = lay_unseeable(&mut app, "dart plate", far);
        app.world_mut().write_message(Stepped { actor: goblin, from: far, to: far });
        settle(&mut app);
        assert!(app.world().get::<Hidden>(dart).is_some(), "a trap nobody saw go off stays hidden");
    }

    /// Spotting a hidden thing is said in the log by its name.
    #[test]
    fn a_hidden_thing_spotted_is_named_in_the_log() {
        let (mut app, _, here_at) = arena_on(3, 2);
        let glyph = lay_unseeable(&mut app, "alarm glyph", here_at.offset(1, 0));
        app.world_mut().entity_mut(glyph).insert(Hidden { spot: 100 });
        pass_turns(&mut app, 2);
        assert!(app.world().get::<Hidden>(glyph).is_none(), "spotted at a certainty");
        assert!(said(&app).iter().any(|l| l == "You spot an alarm glyph."), "{:?}", said(&app));
    }
}
