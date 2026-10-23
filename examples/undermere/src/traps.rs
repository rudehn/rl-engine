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
//! hidden when it springs. [`secret_kinds`] tells what each group of a
//! floor's secrets hides, a [`SecretKind`], by what finding it opens.

use std::collections::{BTreeMap, BTreeSet};

use atglyph::ecs::places::PlaceEntered;
use atglyph::grid::{Terrain, TileId, TileTables};
use atglyph::mapgen::dungeon::{ExitPoint, Room};
use atglyph::mapgen::passes::StartPoint;
use atglyph::mapgen::{Area, HideOpenings, Join, Opening, Secret, Surveyed};
use atglyph::prelude::*;
use atglyph::primitives::Rect;
use atglyph::rand::Rng;
use atglyph::rand::seq::{IndexedRandom, SliceRandom};
use bevy::prelude::*;

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
    /// The names of `assets/layout.ron`'s passage shapes, the corridors a
    /// trap may stand in: read from the layout rather than written here,
    /// so a corridor shape added there takes traps without a word here.
    #[serde(skip)]
    passages: BTreeSet<String>,
}

impl Hiding {
    /// Reads `assets/hidden.ron`, and refuses a range whose fewest is past
    /// its most or a chance past certain, naming the field: a typo there
    /// would otherwise hide nothing, or panic on the first floor built.
    pub fn load() -> Self {
        let mut hiding: Self = ron::from_str(HIDDEN_RON).unwrap_or_else(|e| panic!("assets/hidden.ron: {e}"));
        hiding.passages = crate::dungeon::layout()
            .shapes
            .into_iter()
            .filter(|shape| matches!(shape.kind, atglyph::rules::layout::ShapeKind::Passage { .. }))
            .map(|shape| shape.name)
            .collect();
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
        Some(LayHidden { traps, kinds, flagstones: self.flagstones.count, flagstone: glyph(self.traps.kinds.len()), passages: self.passages.clone() })
    }
}

/// What a group of secret cells hides, told by what finding it opens
/// rather than by how hard it is to see, so a chance retuned in
/// `assets/hidden.ron` never makes a door read as a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecretKind {
    /// A loop's door walled over: shut, nothing is cut off.
    Door,
    /// A dead-end room's one door walled over: shut, the room is cut off.
    Room,
    /// A drowned reliquary's causeway, hidden as the deep water round it.
    Causeway,
}

/// Each group of `secrets` on a place whose tiles are `terrain` and whose
/// way in is `entry`, by what it hides. Each secret is `(at, is, group)`,
/// where it stands, the tile it really is, and the opening it belongs to,
/// as the build's and the place's secrets both say.
///
/// A causeway is told by its tile. A door from a room by a walk from the
/// way in with every other secret found: a room's door, shut alone, cuts
/// cells off the walk, which a loop's door never does, since the engine
/// hides a loop's door only where shutting it cuts nothing off. Locks are
/// walked through, as the engine hides no door to a room behind one.
pub fn secret_kinds(terrain: &Terrain, entry: Point, secrets: &[(Point, TileId, u32)], tables: &TileTables) -> BTreeMap<u32, SecretKind> {
    let groups: BTreeSet<u32> = secrets.iter().map(|(_, _, g)| *g).collect();
    let walk = |shut: Option<u32>| {
        let tile = |p: Point| match secrets.iter().find(|(at, ..)| *at == p) {
            Some((_, is, g)) if Some(*g) != shut => Some(*is),
            _ => terrain.get(p),
        };
        let mut reached = BTreeSet::from([entry]);
        let mut frontier = vec![entry];
        while let Some(p) = frontier.pop() {
            for d in Direction::CARDINALS {
                let n = p + d.offset();
                if tile(n).is_some_and(|t| tables.passable[t.index()]) && reached.insert(n) {
                    frontier.push(n);
                }
            }
        }
        reached
    };
    let whole = walk(None);
    groups
        .into_iter()
        .map(|g| {
            let cells: Vec<&(Point, TileId, u32)> = secrets.iter().filter(|(.., group)| *group == g).collect();
            let kind = if cells.iter().any(|(_, is, _)| tables.names[is.index()] == crate::vaults::CAUSEWAY_TILE) {
                SecretKind::Causeway
            } else {
                let shut = walk(Some(g));
                let lost = whole.iter().any(|p| !shut.contains(p) && !cells.iter().any(|(at, ..)| at == p));
                if lost { SecretKind::Room } else { SecretKind::Door }
            };
            (g, kind)
        })
        .collect()
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
    /// The shapes that are corridors, by name.
    passages: BTreeSet<String>,
}

/// The fewest cells, Chebyshev, between two traps.
pub const TRAP_SPACING: i32 = 3;

impl Pass for LayHidden {
    fn name(&self) -> &'static str {
        "hidden things"
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let Some(ground) = Ground::read(ctx, &self.passages) else { return Ok(()) };
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
    /// Which a corridor: grown as one of the layout's passage shapes.
    corridor: Vec<bool>,
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
    /// Reads `ctx`, whose corridors are the areas grown as one of
    /// `passages`; `None` for a floor the survey never measured.
    fn read(ctx: &BaseContext, passages: &BTreeSet<String>) -> Option<Self> {
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
        let facts = |a: &Area, read: fn(&atglyph::mapgen::AreaFacts) -> bool| survey.areas.get(a.index).is_some_and(read);
        let chokepoint = areas.iter().map(|a| facts(a, |f| f.chokepoint)).collect();
        let corridor = areas.iter().map(|a| passages.contains(&a.shape)).collect();
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
        Some(Self { clear, far, areas, room, corridor, chokepoint, hidden, doorways, stamps })
    }

    /// Every cell a trap may stand on: a corridor's, a chokepoint's, or a
    /// doorway's or beside one, clear and far enough from the way in. A
    /// corridor is an area grown as a passage, and never just any area
    /// that is no room, which on a floor of caves is every cave.
    fn trap_cells(&self) -> BTreeSet<Point> {
        let mut cells: BTreeSet<Point> = BTreeSet::new();
        for (i, area) in self.areas.iter().enumerate() {
            if self.corridor[i] || self.chokepoint[i] {
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
/// Said in the past tense, "You stepped on a hidden dart plate!", because
/// a game's line is read after the pass's own events: the dart's blow is
/// already in the log above it, and "You set off a dart plate!" under
/// "The dart plate hits you for 4." read as a second dart.
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
            format!("You stepped on {}!", atglyph::primitives::noun::with_article(&format!("hidden {what}")))
        } else if sight.can_see(ev.at) {
            let who = names.get(by).map_or_else(|_| "something".to_string(), |n| atglyph::primitives::noun::with_the(n.as_str()));
            format!("{} went off under {who}!", crate::effects::capital(&atglyph::primitives::noun::with_article(&format!("hidden {what}"))))
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
                format!("You spot {}.", atglyph::primitives::noun::with_article(&registries.props.get(kind.0).name))
            }
            Finding::Secret { is, .. } => match map.tables().names[is.index()].as_str() {
                crate::vaults::CAUSEWAY_TILE => "You make out a causeway just under the water.".to_string(),
                _ if map.tables().opens[is.index()].is_some() => "You find a secret door.".to_string(),
                _ => "You find a hidden way through.".to_string(),
            },
        };
        tell.write(Tell::new(line, Tones::NOTICE));
    }
}

/// What a floor `built` hides keeps the promises [`Hiding::openings`] and
/// [`LayHidden`] make with the shipped file, for the run's backstop over
/// every floor of a span of seeds: no secret door on floor one and none
/// past three, no more than two hidden rooms, nothing hidden on the
/// throne's floor; no more traps than the floor's band, each of a kind laid
/// on the floor, no trapdoor on floor nine, no more flagstones than the
/// file's most; every trap on walkable ground, off the stairs and past the
/// population's distance from the way in, and every trap and flagstone on a
/// cell walked to once the secrets are found; and every secret told by what
/// finding it opens, a door with the secret doors' chance and a room with
/// the hidden rooms'. Hands back what each secret was told as, so the
/// span can be held to having told both. `at` names the floor and seed.
#[cfg(test)]
pub(crate) fn hidden_keeps_its_promises(
    hiding: &Hiding,
    built: &atglyph::ecs::places::PlaceBuild,
    floor: u32,
    tables: &TileTables,
    at: &str,
) -> Vec<SecretKind> {
    let secrets: Vec<(Point, TileId, u32)> = built.secrets.iter().map(|x| (x.at, x.is, x.group)).collect();
    let kinds = secret_kinds(&built.terrain, built.entry, &secrets, tables);
    let count = |kind: SecretKind| kinds.values().filter(|k| **k == kind).count();
    let things: Vec<(&str, Point)> = built.spots.iter().filter(|s| s.prefab.is_none()).filter_map(|s| hiding.prop_of(s.tag).map(|name| (name, s.at))).collect();
    if floor == FLOORS {
        assert!(built.secrets.is_empty() && things.is_empty(), "{at}: something hidden on the throne's floor");
        return Vec::new();
    }
    assert!(count(SecretKind::Room) <= 2, "{at}: {} hidden rooms", count(SecretKind::Room));
    let doors = count(SecretKind::Door);
    assert!(doors <= 3 && (floor > 1 || doors == 0), "{at}: {doors} secret doors");
    let flagstone = hiding.flagstones.prop.as_str();
    let traps: Vec<&str> = things.iter().filter(|(name, _)| *name != flagstone).map(|(name, _)| *name).collect();
    let band = hiding.traps.counts.iter().find(|c| (c.floors.0..=c.floors.1).contains(&floor)).expect("every floor has a band of traps");
    assert!(traps.len() as u32 <= band.count.1, "{at}: {} traps", traps.len());
    for name in &traps {
        let kind = hiding.traps.kinds.iter().find(|k| k.prop == *name).expect("a kind the file names");
        assert!(kind.on(floor), "{at}: a {name} laid outside its floors");
    }
    assert!(floor != 9 || !traps.contains(&"trapdoor"), "{at}: a trapdoor on floor nine");
    assert!((things.len() - traps.len()) as u32 <= hiding.flagstones.count.1, "{at}: {} flagstones", things.len() - traps.len());
    let mut found = built.terrain.clone();
    for secret in &built.secrets {
        found.set(secret.at, secret.is);
    }
    let mut reached = BTreeSet::from([built.entry]);
    let mut frontier = vec![built.entry];
    while let Some(p) = frontier.pop() {
        for d in Direction::CARDINALS {
            let n = p + d.offset();
            if found.get(n).is_some_and(|t| tables.passable[t.index()]) && reached.insert(n) {
                frontier.push(n);
            }
        }
    }
    for (name, cell) in &things {
        assert!(reached.contains(cell), "{at}: the {name} at {cell:?} is not reached");
        if *name == flagstone {
            continue;
        }
        let tile = built.terrain.get(*cell).expect("on the map");
        assert!(tables.walkable[tile.index()], "{at}: the {name} at {cell:?} is not on walkable ground");
        assert!(*cell != built.entry && Some(*cell) != built.exit, "{at}: the {name} is on the stairs");
        assert!(geometry::chebyshev(*cell, built.entry) > MIN_DISTANCE_FROM_ENTRY, "{at}: the {name} is by the way in");
    }
    let (door, room) = (hiding.secret_doors.spot, hiding.hidden_rooms.spot);
    for (group, kind) in &kinds {
        let spot = built.secrets.iter().find(|x| x.group == *group).expect("a cell of it").spot;
        let want = match kind {
            SecretKind::Door => door,
            SecretKind::Room => room,
            SecretKind::Causeway => crate::vaults::CAUSEWAY_SPOT,
        };
        assert!(door == room || spot == want, "{at}: group {group} told as a {kind:?}");
    }
    kinds.into_values().collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use atglyph::primitives::RunSeed;

    use super::*;
    use crate::dungeon::map_of;
    use crate::testing::{arena_on, here, monster, pass_turns, said, seeds, settle};

    /// What `assets/hidden.ron` asks of each floor, read with no floor
    /// built: no secret door before floor two and at least one asked for on
    /// every floor from there, never more than three; never more than two
    /// hidden rooms; a band of traps on every floor above the throne, never
    /// emptier than nothing on the floors past the first; no trapdoor laid
    /// on floor nine, above the throne nothing falls into; and every trap
    /// kind laid on some floor. What the passes then lay within those
    /// numbers is theirs and their tests', and the run's backstop holds the
    /// floors to them.
    #[test]
    fn the_hidden_file_asks_each_floor_for_what_its_depth_should_hide() {
        let hiding = Hiding::load();
        let (doors, rooms) = (hiding.secret_doors, hiding.hidden_rooms);
        assert!(doors.from >= 2 && doors.count.0 >= 1 && doors.count.1 <= 3, "secret doors from floor {} at {:?}", doors.from, doors.count);
        assert!(rooms.count.1 <= 2, "hidden rooms at {:?}", rooms.count);
        for floor in 1..FLOORS {
            let band = hiding.traps.counts.iter().find(|c| (c.floors.0..=c.floors.1).contains(&floor));
            let band = band.unwrap_or_else(|| panic!("floor {floor} has no band of traps"));
            assert!(floor == 1 || band.count.1 > 0, "floor {floor} lays no trap");
            assert!(hiding.traps.kinds.iter().any(|k| k.on(floor)) || band.count.1 == 0, "floor {floor} asks for traps of no kind");
        }
        let trapdoor = hiding.traps.kinds.iter().find(|k| k.prop == "trapdoor").expect("a trapdoor kind");
        assert!(!trapdoor.on(FLOORS - 1), "a trapdoor on floor nine drops a body onto the throne's floor");
        for kind in &hiding.traps.kinds {
            assert!((1..FLOORS).any(|f| kind.on(f)), "{} is laid on no floor", kind.prop);
        }
    }

    /// [`LayHidden`] on a made-up floor, two caves and the corridor between
    /// them, over a span of seeds: never more traps than its most, each of
    /// a kind it was handed, on plain walkable ground in the corridor or a
    /// doorway, off the way in by more than the population's distance, off
    /// the way down and the cells beside it, and no two within
    /// [`TRAP_SPACING`] of each other; never more flagstones than its most,
    /// each on a cell no trap took.
    #[test]
    fn lay_hidden_never_lays_more_than_asked_and_only_where_a_trap_may_stand() {
        use atglyph::mapgen::{AreaFacts, BaseContext, Chain, Phase};
        let tiles = crate::dungeon::registry();
        let ground = tiles.expect("hall floor");
        let passages = BTreeSet::from(["corridor".to_string()]);
        let (entry, exit) = (Point::new(1, 2), Point::new(58, 9));
        let kinds = vec![glyph(0), glyph(2)];
        let flagstone = glyph(5);
        for s in seeds(0..40) {
            let mut ctx = BaseContext::blank(60, 12, tiles.clone(), ground);
            let block = |x0: i32, x1: i32| -> Vec<Point> { (x0..x1).flat_map(|x| (1..11).map(move |y| Point::new(x, y))).collect() };
            let area = |index: usize, shape: &str, cells: Vec<Point>| {
                let (x0, x1) = (cells.iter().map(|p| p.x).min().unwrap_or(0), cells.iter().map(|p| p.x).max().unwrap_or(0));
                let bounds = Rect::new(x0, 1, x1 - x0 + 1, 10);
                Area { index, shape: shape.into(), district: "d".into(), cells, bounds, inner: bounds, entries: None }
            };
            let corridor: Vec<Point> = (15..45).map(|x| Point::new(x, 5)).collect();
            ctx.emit(area(0, "cave", block(0, 15)));
            ctx.emit(area(1, "corridor", corridor.clone()));
            ctx.emit(area(2, "cave", block(45, 60)));
            ctx.emit(Room(Rect::new(0, 1, 15, 10)));
            ctx.emit(Room(Rect::new(45, 1, 15, 10)));
            ctx.emit(Opening { cells: vec![Point::new(14, 5)], facing: Direction::East, areas: (0, 1), join: Join::Arch });
            ctx.emit(Opening { cells: vec![Point::new(45, 5)], facing: Direction::East, areas: (1, 2), join: Join::Arch });
            let facts = |dead_end: bool| AreaFacts { openings: 1, dead_end, chokepoint: false, junction: false, seclusion: 0 };
            ctx.emit(Surveyed { areas: vec![facts(true), facts(false), facts(true)], loops: 0, open_pct: 100, route: None });
            ctx.emit(StartPoint(entry));
            ctx.emit(ExitPoint(exit));
            let pass = LayHidden { traps: (2, 5), kinds: kinds.clone(), flagstones: (1, 3), flagstone, passages: passages.clone() };
            assert_eq!(<LayHidden as Pass>::phase(&pass), Phase::Finish);
            Chain::new().then(pass).run(&mut ctx, RunSeed(s)).expect("it lays");
            let laid: Vec<(char, Point)> = ctx.outputs().iter::<Stamped>().flat_map(|st| st.marks.clone()).collect();
            let traps: Vec<Point> = laid.iter().filter(|(c, _)| *c != flagstone).map(|(_, p)| *p).collect();
            let stones: Vec<Point> = laid.iter().filter(|(c, _)| *c == flagstone).map(|(_, p)| *p).collect();
            assert!(traps.len() <= 5 && stones.len() <= 3, "seed {s}: {} traps and {} flagstones", traps.len(), stones.len());
            for (c, p) in laid.iter().filter(|(c, _)| *c != flagstone) {
                assert!(kinds.contains(c), "seed {s}: a trap of a kind it was not handed");
                let beside_a_mouth = [Point::new(14, 5), Point::new(45, 5)].iter().any(|m| geometry::chebyshev(*m, *p) <= 1);
                assert!(corridor.contains(p) || beside_a_mouth, "seed {s}: a trap at {p:?}, in a cave");
                assert!(geometry::chebyshev(*p, entry) > MIN_DISTANCE_FROM_ENTRY && geometry::chebyshev(*p, exit) > 1, "seed {s}: a trap at {p:?} by a way");
            }
            for (i, a) in traps.iter().enumerate() {
                assert!(traps[i + 1..].iter().all(|b| geometry::chebyshev(*a, *b) >= TRAP_SPACING), "seed {s}: two traps within {TRAP_SPACING}");
            }
            assert!(stones.iter().all(|p| !traps.contains(p)), "seed {s}: a flagstone under a trap");
        }
    }

    /// Standing beside a secret door and waiting, at the base chances with
    /// no ring, the adventurer finds it within twenty turns on at least
    /// nine runs in ten: waiting is searching, and a wall suspected is a
    /// wall found. Worked from the numbers the game plays by rather than
    /// sampled: the door's spot percent from `hidden.ron`, weighed by
    /// [`SPOTTING`] for a finder one cell off who waits, with no keenness,
    /// and compounded over twenty turns. That the engine rolls that chance
    /// once a turn, once for a door's whole group, and that waiting
    /// multiplies it, is `atglyph-bevy`'s `props` tests: the secret in sight
    /// found at its chance, the waited turn that multiplies it, and the
    /// group found as one.
    #[test]
    fn beside_a_secret_door_a_waiting_adventurer_finds_it_within_twenty_turns_on_nine_runs_in_ten() {
        let hiding = Hiding::load();
        let (chance, out_of) = SPOTTING.chance(hiding.secret_doors.spot, 0, 1, true).expect("a door beside the finder can be spotted");
        let (chance, out_of) = (f64::from(chance), f64::from(out_of));
        let found_within_twenty = 1.0 - (1.0 - chance / out_of).powi(20);
        assert!(found_within_twenty >= 0.9, "a waiting adventurer finds a door beside them in twenty turns {found_within_twenty:.3} of the time");
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
        assert!(said(&app).iter().any(|l| l == "A hidden net trap went off under the goblin!"), "and the log says so: {:?}", said(&app));
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

    /// Where `who` stands: the map and the cell.
    fn whereabouts(app: &App, who: Entity) -> (MapId, Point) {
        (app.world().get::<OnMap>(who).expect("on a map").0, app.world().get::<Position>(who).expect("somewhere").0)
    }

    /// Stepping on a trapdoor drops the adventurer through it, by its own
    /// trigger, onto a cell of the floor below off its stairs.
    #[test]
    fn a_trapdoor_stepped_on_drops_the_adventurer_to_the_floor_below() {
        let (mut app, me, here_at) = arena_on(3, 3);
        let trapdoor = here_at.offset(1, 0);
        lay_unseeable(&mut app, "trapdoor", trapdoor);
        app.world_mut().write_message(Intent::new(me, Step(Direction::East)));
        for _ in 0..3 {
            settle(&mut app);
        }
        let (on, at) = whereabouts(&app, me);
        assert_eq!(on, map_of(4), "fell through to floor four");
        let map = app.world().resource::<WorldMap>();
        let place = map.place(on).expect("built by the fall");
        assert!(at != place.entry && Some(at) != place.exit && map.is_walkable_on(on, at), "on open ground off the stairs, at {at:?}");
    }

    /// A monster stepping on a trapdoor falls through it too, by the same
    /// trigger, and leaves the floor.
    #[test]
    fn a_trapdoor_stepped_on_drops_a_monster_to_the_floor_below() {
        let (mut app, _, here_at) = arena_on(3, 3);
        let trapdoor = here_at.offset(2, 0);
        lay_unseeable(&mut app, "trapdoor", trapdoor);
        let goblin = monster(&mut app, "goblin", trapdoor);
        app.world_mut().write_message(Stepped { actor: goblin, from: trapdoor.offset(1, 0), to: trapdoor });
        for _ in 0..3 {
            settle(&mut app);
        }
        assert_eq!(whereabouts(&app, goblin).0, map_of(4), "the goblin fell through to floor four");
    }

    /// Nothing falls into the throne's hall: a trapdoor on the floor above
    /// it holds whoever steps on it, who stays where they stepped.
    #[test]
    fn nothing_falls_through_to_the_thrones_floor() {
        let (mut app, me, here_at) = arena_on(3, FLOORS - 1);
        let trapdoor = here_at.offset(1, 0);
        lay_unseeable(&mut app, "trapdoor", trapdoor);
        app.world_mut().write_message(Intent::new(me, Step(Direction::East)));
        for _ in 0..3 {
            settle(&mut app);
        }
        assert_eq!(whereabouts(&app, me), (map_of(FLOORS - 1), trapdoor), "still on floor nine, on the trapdoor");
    }

    /// A loose flagstone found and lifted always has something under it:
    /// its gold is at least a coin, whatever else it rolls.
    #[test]
    fn a_loose_flagstone_always_holds_a_coin_at_least() {
        let library = crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default());
        let registries = &library.registries;
        let def = registries.props.get(registries.props.expect("loose flagstone"));
        let rows = &def.container.as_ref().expect("a container").contents;
        assert!(rows.iter().any(|r| r.min >= 1), "a row that always rolls something: {rows:?}");
    }

    /// On a floor of caves, a trap may stand only in a corridor, a
    /// chokepoint or a doorway, never anywhere in a cave: two caves that
    /// are neither, joined by a corridor that is not a chokepoint either,
    /// take traps in the corridor and at its two mouths and nowhere else.
    #[test]
    fn on_a_floor_of_caves_a_trap_stands_only_in_a_corridor_a_chokepoint_or_a_doorway() {
        use atglyph::mapgen::{AreaFacts, BaseContext};
        let hiding = Hiding::load();
        assert!(
            hiding.passages.contains("corridor") && !hiding.passages.contains("cave"),
            "the layout's corridors and none of its caves: {:?}",
            hiding.passages
        );
        let tiles = crate::dungeon::registry();
        let ground = tiles.expect("hall floor");
        let mut ctx = BaseContext::blank(40, 12, tiles, ground);
        let block = |x0: i32, x1: i32| -> Vec<Point> { (x0..x1).flat_map(|x| (1..11).map(move |y| Point::new(x, y))).collect() };
        let corridor: Vec<Point> = (15..25).map(|x| Point::new(x, 5)).collect();
        let area = |index: usize, shape: &str, cells: Vec<Point>| {
            let bounds = Rect::new(cells[0].x, 1, cells.last().map_or(1, |p| p.x - cells[0].x + 1), 10);
            Area { index, shape: shape.into(), district: "d".into(), cells, bounds, inner: bounds, entries: None }
        };
        ctx.emit(area(0, "cave", block(0, 15)));
        ctx.emit(area(1, "corridor", corridor.clone()));
        ctx.emit(area(2, "cave", block(25, 40)));
        let mouths = [Point::new(14, 5), Point::new(25, 5)];
        ctx.emit(Opening { cells: vec![mouths[0]], facing: Direction::East, areas: (0, 1), join: Join::Arch });
        ctx.emit(Opening { cells: vec![mouths[1]], facing: Direction::East, areas: (1, 2), join: Join::Arch });
        let facts = |dead_end: bool| AreaFacts { openings: 1, dead_end, chokepoint: false, junction: false, seclusion: 0 };
        ctx.emit(Surveyed { areas: vec![facts(true), facts(false), facts(true)], loops: 0, open_pct: 100, route: None });
        ctx.emit(StartPoint(Point::new(1, 1)));
        let cells = Ground::read(&ctx, &hiding.passages).expect("surveyed").trap_cells();
        assert!(corridor.iter().all(|p| cells.contains(p)), "the corridor takes traps");
        for p in &cells {
            let beside_a_mouth = mouths.iter().any(|m| m == p || Direction::CARDINALS.iter().any(|d| *m + d.offset() == *p));
            assert!(corridor.contains(p) || beside_a_mouth, "a trap may stand at {p:?}, inside a cave");
        }
    }
}
