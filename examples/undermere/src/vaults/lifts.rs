//! What taking a piece's prize sets off.
//!
//! Each prize is held by a container prop of its own kind, a cage or a
//! plinth or an altar, and the moment it is emptied is the moment the
//! engine marks it [`Emptied`]. Each system here answers one kind: the
//! other cages shut, the vermin pour from their holes, the water rises,
//! the planks give way, the gallery falls in behind. Whatever is set off
//! looks for the rest of its piece within [`REACH`] of the prize, since a
//! floor holds one of each piece at most and every piece is smaller.
//!
//! The emptied container is then stood down as its spent kind, which
//! holds nothing and sets off nothing, so the moment is answered once: a
//! continued run restores an emptied container as emptied, and it would
//! otherwise be answered again.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::EffectRng;
use rl_engine::rl_bevy::props::spawn_prop;
use rl_engine::rl_bevy::turn::Turns;
use rl_engine::rl_save::SaveableState;
use serde::{Deserialize, Serialize};

use super::{VaultTiles, walk_from};

/// How far round a prize the rest of its piece is looked for: farther than
/// any piece is long.
pub const REACH: i32 = 16;

/// The three cages of Caged Altars, each holding a thing of its own kind,
/// so which one is opened is a choice seen before it is made.
pub const CAGES: [&str; 3] = ["caged blade", "caged staff", "caged ring"];
/// A cage slammed shut on what it held.
pub const SHUT_CAGE: &str = "shut cage";
/// A cage opened and emptied.
pub const EMPTY_CAGE: &str = "empty cage";
/// The Rat Trap's prize, and the holes its vermin pour from.
pub const RAT_PLINTH: &str = "gnawed plinth";
/// A hole in the foot of a wall, which vermin pour from once a gnawed
/// plinth is emptied.
pub const RAT_HOLE: &str = "rat hole";
/// What pours from a rat hole, and how many from each.
pub const VERMIN: (&str, u32) = ("rat", 2);
/// The Flooding Vault's prize, and the grates its water pours through.
pub const FLOOD_ALTAR: &str = "drowned altar";
/// A grate in a flooding vault's floor, where its water comes in first.
pub const GRATE: &str = "iron grate";
/// The Rotten Floor's prize, on planks that give way once it is lifted.
pub const ROTTEN_PLINTH: &str = "teetering plinth";
/// The Collapsing Gallery's prize, and the lintels over its two ways in.
pub const KEYSTONE: &str = "keystone plinth";
/// The cracked lintel over a way into a collapsing gallery: the one
/// nearer the floor's way in falls once the keystone is lifted.
pub const LINTEL: &str = "cracked lintel";
/// What a plinth is once its prize is lifted.
pub const BARE_PLINTH: &str = "bare plinth";
/// What a drowned altar is once its prize is lifted.
pub const BARE_ALTAR: &str = "bare altar";

/// How many turns a flooding vault takes from its prize lifted to deep
/// water at its far wall.
pub const FLOOD_TURNS: u32 = 8;

/// A container just emptied: which, what kind, where.
type Lifted<'w, 's> = Query<'w, 's, (Entity, &'static PropKind, &'static Position, &'static OnMap), Added<Emptied>>;

/// Every other prop of a kind among `kinds`, with where it stands.
type Others<'w, 's> = Query<'w, 's, (Entity, &'static PropKind, &'static Position, &'static OnMap), Without<Emptied>>;

/// Stands `prop` down as `spent`, on the cell it stood on: what holds
/// nothing and sets off nothing any more.
fn stand_down(commands: &mut Commands, registries: &Registries, prop: Entity, spent: &str, at: Point, map: MapId) {
    commands.entity(prop).despawn();
    spawn_prop(commands, registries, registries.props.expect(spent), at, map);
}

/// Whether `kind` is the prop called `name`.
fn is(registries: &Registries, kind: &PropKind, name: &str) -> bool {
    registries.props.id(name) == Some(kind.0)
}

/// One cage of Caged Altars opened and emptied shuts the other two on what
/// they hold, for good: the choice of three is a choice of one.
///
/// In `TurnSet::React`.
pub fn shut_other_cages(mut commands: Commands, registries: Res<Registries>, lifted: Lifted, others: Others, bags: Query<&Inventory>) {
    for (taken, kind, at, on) in &lifted {
        if !CAGES.iter().any(|c| is(&registries, kind, c)) {
            continue;
        }
        for (cage, k, p, m) in &others {
            if m.0 != on.0 || geometry::chebyshev(p.0, at.0) > REACH || !CAGES.iter().any(|c| is(&registries, k, c)) {
                continue;
            }
            // What it held goes with it: shut in, out of reach for good.
            for item in bags.get(cage).map(|b| b.items.clone()).unwrap_or_default() {
                commands.entity(item).despawn();
            }
            stand_down(&mut commands, &registries, cage, SHUT_CAGE, p.0, m.0);
        }
        stand_down(&mut commands, &registries, taken, EMPTY_CAGE, at.0, on.0);
        commands.write_message(Tell::new("With a clang, the other cages slam shut!", Tones::BAD));
    }
}

/// A gnawed plinth emptied sets the vermin loose: each rat hole of its
/// piece lets out [`VERMIN`] beside it.
///
/// In `TurnSet::React`.
pub fn loose_the_vermin(mut commands: Commands, registries: Res<Registries>, lifted: Lifted, others: Others) {
    for (taken, kind, at, on) in &lifted {
        if !is(&registries, kind, RAT_PLINTH) {
            continue;
        }
        for (_, k, p, m) in &others {
            if m.0 == on.0 && geometry::chebyshev(p.0, at.0) <= REACH && is(&registries, k, RAT_HOLE) {
                let (hole, map) = (p.0, m.0);
                commands.queue(move |world: &mut World| crate::effects::bodies::hatch(world, VERMIN.0, VERMIN.1, hole, map));
            }
        }
        stand_down(&mut commands, &registries, taken, BARE_PLINTH, at.0, on.0);
        commands.write_message(Tell::new("The walls crack open, and the dark behind them squeals!", Tones::BAD));
    }
}

/// A teetering plinth emptied brings its planks down: every rotten plank
/// joined to it turns to a hole, and whoever stood on one falls through
/// to the floor below by [`fall_through`](crate::effects::moving::fall_through),
/// landing off its stairs; what lay on them falls out of reach.
///
/// In `TurnSet::React`.
pub fn give_way(mut commands: Commands, registries: Res<Registries>, lifted: Lifted, map: Res<WorldMap>, tiles: Res<VaultTiles>) {
    for (taken, kind, at, on) in &lifted {
        if !is(&registries, kind, ROTTEN_PLINTH) || on.0 != map.current() {
            continue;
        }
        // Every plank joined to the plinth's cell, which stands on one.
        let planks = walk_from(at.0, |p| geometry::chebyshev(p, at.0) <= REACH && map.tile(p) == Some(tiles.rotten_planks));
        let cells: Vec<Point> = planks.keys().copied().collect();
        commands.entity(taken).despawn();
        let (hole, here) = (tiles.hole, on.0);
        commands.queue(move |world: &mut World| drop_through(world, &cells, hole, here));
    }
}

/// Lays `hole` over `cells` of `map`, the place being read, and sends
/// everyone standing on one through to the floor below; things and props
/// on them go with the floor.
fn drop_through(world: &mut World, cells: &[Point], hole: TileId, map: MapId) {
    let on_them = |p: &Position, m: &OnMap| m.0 == map && cells.contains(&p.0);
    let fallers: Vec<Entity> =
        world.query_filtered::<(Entity, &Position, &OnMap), With<Health>>().iter(world).filter(|(_, p, m)| on_them(p, m)).map(|(e, ..)| e).collect();
    let things: Vec<Entity> = world
        .query_filtered::<(Entity, &Position, &OnMap), Or<(With<Item>, With<Prop>)>>()
        .iter(world)
        .filter(|(_, p, m)| on_them(p, m))
        .map(|(e, ..)| e)
        .collect();
    for thing in things {
        world.despawn(thing);
    }
    let mut tiles = world.resource_mut::<WorldMap>();
    for p in cells {
        tiles.set_tile(*p, hole);
    }
    for who in fallers {
        let roll = world.resource_mut::<EffectRng>().random::<u64>();
        crate::effects::moving::fall_through(world, who, roll);
    }
}

/// A keystone plinth emptied brings down the lintel over the nearer of
/// its gallery's two ways back, walked from the plinth: the short way,
/// which is the way most come, now fallen rock, so the way out is the long
/// one round. The gallery's two ways meet again at its door, so whichever
/// falls the other still leads out.
///
/// Whoever stands under the lintel leaps clear first, onto the nearest
/// free cell, rather than be buried standing in the rock; with nowhere
/// free to leap to, the lintel holds.
///
/// In `TurnSet::React`.
pub fn collapse_behind(
    mut commands: Commands,
    registries: Res<Registries>,
    lifted: Lifted,
    others: Others,
    tiles: Res<VaultTiles>,
    mut map: ResMut<WorldMap>,
    occupancy: Res<Occupancy>,
) {
    for (taken, kind, at, on) in &lifted {
        if !is(&registries, kind, KEYSTONE) || on.0 != map.current() {
            continue;
        }
        let walked = walk_from(at.0, |p| geometry::chebyshev(p, at.0) <= REACH && map.tile(p).is_some_and(|t| map.tables().walkable[t.index()]));
        let lintel = others
            .iter()
            .filter(|(_, k, p, m)| m.0 == on.0 && is(&registries, k, LINTEL) && walked.contains_key(&p.0))
            .min_by_key(|(_, _, p, _)| (walked[&p.0], p.0.y, p.0.x))
            .map(|(e, _, p, _)| (e, p.0));
        let lintel = lintel.filter(|(_, cell)| {
            let under: Vec<Entity> = occupancy.at(*cell).to_vec();
            let clear = crate::effects::free_cells_near(&map, &occupancy, *cell, under.len(), crate::effects::NEAR);
            if clear.len() < under.len() {
                return false;
            }
            for (who, to) in under.into_iter().zip(clear) {
                commands.write_message(WarpRequest { actor: who, to: Destination::Place { map: on.0, arrive: Arrive::At(to) }, after: None });
            }
            true
        });
        if let Some((lintel, cell)) = lintel {
            commands.entity(lintel).despawn();
            map.set_tile(cell, tiles.fallen_rock);
            commands.write_message(Tell::new("With a roar, the way you came falls in!", Tones::BAD));
        }
        stand_down(&mut commands, &registries, taken, BARE_PLINTH, at.0, on.0);
    }
}

/// One flooding vault filling: the turn its prize was lifted, and for each
/// cell of its room the turns after that it is wading-deep and, for its
/// far end, too deep to cross.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flood {
    /// The floor it fills, as its raw `MapId`.
    pub map: u32,
    /// The turn it began.
    pub began: u32,
    /// Each cell of the room and when the water reaches it.
    pub cells: Vec<FloodCell>,
}

/// One cell of a flooding vault's room, and when the water reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FloodCell {
    /// Where it is.
    pub at: Point,
    /// The turns after the flood began that it is wading-deep.
    pub wet: u32,
    /// The turns after which it is too deep to cross, for a cell of the
    /// far end; `None` nearer the door, which never goes deep.
    pub deep: Option<u32>,
    /// Its steps from the door, inside the room: the way out of the far
    /// end is down these, a step at a time.
    pub back: u32,
}

/// Every flooding vault still filling, saved with the run, so water
/// rising when a run is left rises on when it is continued, and on a
/// floor left mid-flood catches up when it is entered again.
#[derive(Resource, Debug, Default)]
pub struct RisingFloods(pub Vec<Flood>);

impl SaveableState for RisingFloods {
    type Saved = Vec<Flood>;

    fn capture(&self) -> Vec<Flood> {
        self.0.clone()
    }

    fn restore(&mut self, saved: Vec<Flood>) {
        self.0 = saved;
    }
}

/// A drowned altar emptied floods its room: the water comes in at the
/// room's grates and spreads a cell a turn, wading-deep, and the third of
/// the room farthest from its door is too deep to cross once
/// [`FLOOD_TURNS`] have passed, whatever was left lying there with it.
/// The room is every cell walked to from the altar short of a door.
///
/// In `TurnSet::React`.
pub fn start_flood(
    mut commands: Commands,
    registries: Res<Registries>,
    lifted: Lifted,
    others: Others,
    map: Res<WorldMap>,
    turns: Res<Turns>,
    mut floods: ResMut<RisingFloods>,
) {
    for (taken, kind, at, on) in &lifted {
        if !is(&registries, kind, FLOOD_ALTAR) || on.0 != map.current() {
            continue;
        }
        let grates: Vec<Point> = others
            .iter()
            .filter(|(_, k, p, m)| m.0 == on.0 && geometry::chebyshev(p.0, at.0) <= REACH && is(&registries, k, GRATE))
            .map(|(_, _, p, _)| p.0)
            .collect();
        floods.0.push(Flood { map: on.0.0, began: turns.turn_number(), cells: flood_plan(&map, at.0, &grates) });
        stand_down(&mut commands, &registries, taken, BARE_ALTAR, at.0, on.0);
        commands.write_message(Tell::new("Water gushes up through the grates!", Tones::BAD));
    }
}

/// Each cell of the room round `altar` on the place being read, the turns
/// until it is wading-deep, counted out from the nearest of `grates`, and
/// for the third of the room farthest from its door, [`FLOOD_TURNS`]
/// until it is too deep to cross.
pub fn flood_plan(map: &WorldMap, altar: Point, grates: &[Point]) -> Vec<FloodCell> {
    let tables = map.tables();
    let door = |p: Point| map.tile(p).is_some_and(|t| tables.opens[t.index()].is_some() || tables.closes[t.index()].is_some());
    let floor = |p: Point| geometry::chebyshev(p, altar) <= REACH && map.tile(p).is_some_and(|t| tables.walkable[t.index()]) && !door(p);
    let room: BTreeSet<Point> = walk_from(altar, |p| p == altar || floor(p)).into_keys().collect();
    let from = |starts: Vec<Point>| {
        let mut seen: BTreeMap<Point, u32> = starts.iter().map(|p| (*p, 0)).collect();
        let mut queue: VecDeque<Point> = starts.into_iter().collect();
        while let Some(p) = queue.pop_front() {
            let d = seen[&p];
            for n in Direction::CARDINALS.map(|dir| p + dir.offset()) {
                if room.contains(&n) && !seen.contains_key(&n) {
                    seen.insert(n, d + 1);
                    queue.push_back(n);
                }
            }
        }
        seen
    };
    let sources: Vec<Point> = grates.iter().copied().filter(|g| room.contains(g)).collect();
    let wet = from(if sources.is_empty() { vec![altar] } else { sources });
    let mouth: Vec<Point> = room.iter().copied().filter(|p| Direction::CARDINALS.iter().any(|d| door(*p + d.offset()))).collect();
    let back = from(if mouth.is_empty() { vec![altar] } else { mouth });
    let deepest = back.values().copied().max().unwrap_or(0);
    room.iter()
        .map(|p| {
            let steps = back.get(p).copied().unwrap_or(0);
            let far = deepest > 0 && steps * 3 >= deepest * 2;
            FloodCell { at: *p, wet: wet.get(p).copied().unwrap_or(0).min(FLOOD_TURNS - 1), deep: far.then_some(FLOOD_TURNS), back: steps }
        })
        .collect()
}

/// Carries every flood on the place being read on to where its turns have
/// brought it, a turn at a time or all at once for a floor entered again,
/// and forgets each once it has risen as far as it will.
///
/// Water too deep to cross never rises round somebody standing in it: the
/// cell they stand on stays wading-deep, and so does one channel from it
/// to the near part of the room, each step of it a step nearer the door,
/// so nobody is left in deep water or on an island of shallows, and the
/// flood is kept until they are out. The rest of the far end goes deep as
/// it would: holding all of it while anyone stood there let a taker who
/// lingered by the far wall loot it dry, and a monster parked there hold
/// the flood off for good. The near part never goes deep, and every cell
/// of it is nearer the door than any of the far end, so the channel ends
/// on a way out.
///
/// In `TurnSet::React`.
pub fn rise_floods(mut floods: ResMut<RisingFloods>, mut map: ResMut<WorldMap>, tiles: Res<VaultTiles>, turns: Res<Turns>, occupancy: Res<Occupancy>) {
    let (here, now) = (map.current(), turns.turn_number());
    floods.0.retain(|flood| {
        if flood.map != here.0 {
            return true;
        }
        let since = now.saturating_sub(flood.began);
        let held = channels(&flood.cells, since, &occupancy);
        for cell in &flood.cells {
            let want = match cell.deep {
                Some(d) if since >= d && !held.contains(&cell.at) => tiles.deep,
                Some(d) if since >= d => tiles.flooded,
                _ if since >= cell.wet => tiles.flooded,
                _ => continue,
            };
            if map.tile(cell.at) != Some(want) {
                map.set_tile(cell.at, want);
            }
        }
        since < FLOOD_TURNS || !held.is_empty()
    });
}

/// The cells of the far end `since` turns into a flood that are held
/// wading-deep: each one somebody stands on, and from each, one way out
/// of the far end, a step at a time to the neighbour a step nearer the
/// door, the first such in [`Direction::CARDINALS`] order. Taking the same
/// one every turn keeps the channel still, so a wader walking out along it
/// walks the rest of the same channel.
fn channels(cells: &[FloodCell], since: u32, occupancy: &Occupancy) -> BTreeSet<Point> {
    let far: BTreeMap<Point, u32> = cells.iter().filter(|c| c.deep.is_some_and(|d| since >= d)).map(|c| (c.at, c.back)).collect();
    let mut held = BTreeSet::new();
    for (&start, &steps) in far.iter().filter(|(p, _)| occupancy.is_occupied(**p)) {
        let (mut p, mut steps) = (start, steps);
        held.insert(p);
        while let Some(next) = Direction::CARDINALS.iter().map(|d| p + d.offset()).find(|n| far.get(n).is_some_and(|b| *b + 1 == steps)) {
            held.insert(next);
            (p, steps) = (next, steps - 1);
        }
    }
    held
}
