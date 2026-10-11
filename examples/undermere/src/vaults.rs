//! What the risk-and-reward pieces do that no effect says.
//!
//! A piece is a prefab in `assets/prefabs/risk/` or `assets/prefabs/reward/`
//! whose props, items and monsters the engine lays as it lays any piece's.
//! What is left is this module's, in three parts.
//!
//! When a floor is built, [`place_vault_fixtures`] gives a locked vault's
//! lock its own floor's key, drops that key and a sealed vault's lever on
//! cells walked to from the way in without passing a lock, and hides a
//! drowned reliquary's causeway as the deep water round it, an engine
//! [`Secret`] spotted as any hidden door is.
//!
//! When a floor is first entered, [`spawn_vault_marks`] fills each piece's
//! `Mark`s by one vocabulary, whichever piece drew them: [`SLEEPER`] a
//! sleeper of the floor's own, [`GIANT`] an ogre asleep, [`GOBLIN`] a goblin
//! asleep, [`CAPTIVE`] a captive of the floor's own shackled, and
//! [`CAUSEWAY`] the causeway already hidden at build. A sleeper keeps the
//! cell it was laid on as its post, as a piece's guard does.
//!
//! In play, [`lifts`] answers a prize taken: the other cages shut, the
//! vermin pour out, the water rises, the floor gives way, the gallery
//! falls in behind. [`answer_pull_lever`] lifts a floor's seal and
//! [`spend_vault_key`] leaves a key in the lock it opened; what a
//! reward piece's own book or a dead adventurer's own wall offers to be
//! read is [`crate::writings`]'s. What the glyphs, the bone circle, the
//! vents and a fall do is an effect's, in [`crate::effects`]:
//! `WakeFloor`, `Rise`, `Rouse` and `Fall`.
//!
//! Everything here is told apart by what a prop is and where it stands,
//! which the save already keeps, rather than by a component of the game's
//! own, so a run continued mid-vault does what the run left would have
//! done; the two things that are not, a key's floor and a flood still
//! rising, the save keeps by name.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::doors::DoorEvent;
use rl_engine::rl_bevy::places::{PlaceBuild, PlaceEntered, Spot};
use rl_engine::rl_bevy::prefabs::Prefabs;
use rl_engine::rl_bevy::props::spawn_prop;
use rl_engine::rl_bevy::world::PlaceMap;
use rl_engine::rl_grid::{TileRegistry, TileTables};
use rl_engine::rl_rules::prefab::Slot;

use crate::armory::{Armory, spawn_item};
use crate::bestiary::{Bestiary, MonsterDef, spawn_monster};
use crate::dungeon::{MIN_DISTANCE_FROM_ENTRY, floor_of};

pub mod lifts;
#[cfg(test)]
mod tests;

/// A piece's mark for a sleeper of the floor's own, drawn from the floor's
/// table as the population draws one: a hoard's band, a last stand's
/// killers, a vestibule's waiting dead.
pub const SLEEPER: char = 'z';
/// A piece's mark for an ogre asleep: the sleeping giant across his narrow
/// way, an ogre whatever the floor's table holds.
pub const GIANT: char = 'Z';
/// A piece's mark for a goblin asleep: a stockade's sleepers, goblins
/// whatever the floor's table holds.
pub const GOBLIN: char = 'G';
/// A piece's mark for a captive of the floor's own, shackled as part 3's
/// run captive is and freed the same way.
pub const CAPTIVE: char = 'C';
/// A piece's mark for a cell of a sunken causeway: hidden at build as the
/// deep water round it and found as any secret is.
pub const CAUSEWAY: char = 'c';

/// The ogre a [`GIANT`] mark stands.
const GIANT_KIND: &str = "ogre";
/// The goblin a [`GOBLIN`] mark stands.
const GOBLIN_KIND: &str = "goblin";

/// How likely a turn in sight finds a sunken causeway, in percent: the
/// unit a hidden prop's chance is given in, and as hard to see as the
/// floor's hidden plates.
pub const CAUSEWAY_SPOT: u8 = 10;

/// The areas a causeway's [`Secret`](rl_engine::rl_mapgen::Secret)
/// joins: none a reader can look up. The causeway is hidden once the
/// build's areas have been flattened into its spots, so there is no area
/// index left to name, and an index past any a floor grows says so where
/// `(0, 0)` would name the floor's first area as joined to its island.
pub const JOINS_NO_AREA: (usize, usize) = (usize::MAX, usize::MAX);

/// How far a vault's key or lever lies from anything a piece laid: farther
/// than any piece's own cell is from the nearest thing it lays.
const PIECE_CLEAR: i32 = 4;

/// The spot a locked vault's key is dropped at: past every glyph a
/// legend could carry, so no piece's mark is ever read as it.
pub(crate) const KEY_TAG: u32 = u32::MAX - 1;
/// As [`KEY_TAG`], for a sealed vault's lever.
pub(crate) const LEVER_TAG: u32 = u32::MAX - 2;

/// The item a locked vault's key is, `assets/items.ron`'s.
pub const KEY_ITEM: &str = "vault key";
/// The prop a sealed vault's lever is, hidden, offering [`PULL_VERB`].
pub const LEVER: &str = "concealed lever";
/// What a lever is once pulled: the same lever, spent.
pub const PULLED_LEVER: &str = "pulled lever";
/// What a lever offers, declared by [`crate::plugin::UndermerePlugin`].
pub const PULL_VERB: &str = "pull";

/// The tile ids the pieces' mechanisms paint with in play, read once from
/// `Undermere::tiles` in `run::prepare`, since nothing after that call
/// may ask a tile by name.
#[derive(Resource, Debug, Clone, Copy)]
pub struct VaultTiles {
    /// Water a body wades through, a flooding vault's first.
    pub flooded: TileId,
    /// Water nobody crosses, a flooding vault's far end once it has risen.
    pub deep: TileId,
    /// A collapsing gallery's way back, fallen in.
    pub fallen_rock: TileId,
    /// What a rotten floor leaves when it gives way.
    pub hole: TileId,
    /// The planks a rotten floor is laid of.
    pub rotten_planks: TileId,
    /// A sealed vault's gate once its lever is pulled.
    pub raised_gate: TileId,
    /// A locked vault's door once its key has opened it.
    pub open_vault_door: TileId,
}

impl VaultTiles {
    /// Reads each by name; every one is registered in `dungeon.rs`, so a
    /// missing one is a typo there, which every test that starts a run
    /// finds.
    pub fn new(tiles: &TileRegistry) -> Self {
        Self {
            flooded: tiles.expect("flooded floor"),
            deep: tiles.expect("deep water"),
            fallen_rock: tiles.expect("fallen rock"),
            hole: tiles.expect("hole"),
            rotten_planks: tiles.expect("rotten planks"),
            raised_gate: tiles.expect("raised gate"),
            open_vault_door: tiles.expect("open vault door"),
        }
    }
}

/// Finishes a floor's vaults once it is built: a locked vault's lock asks
/// for its own floor's [`vault_key`](crate::content::vault_key), so a key
/// opens its own vault and no other; the key and a sealed vault's lever
/// are dropped where `fixture_cell` says; and every [`CAUSEWAY`] mark is
/// hidden as the deep water round it, found as any secret is. Nothing on a
/// place that is no floor or drew none of them.
pub fn place_vault_fixtures(built: &mut PlaceBuild, tiles: &TileRegistry, prefabs: &Prefabs<MonsterDef>, seed: RunSeed, map: MapId) {
    let Some(floor) = floor_of(map) else { return };
    hide_causeways(built, tiles, prefabs);
    if built.locks.is_empty() {
        return;
    }
    let key = crate::content::tags().id(&crate::content::vault_key(floor));
    for lock in built.locks.iter_mut().filter(|l| l.key.is_some()) {
        lock.key = key;
    }
    let mut rng = seed.rng(SeedDomain::new(b"undermere.vaults.fixtures"), u64::from(map.0));
    let locked: Vec<Point> = built.locks.iter().filter(|l| l.key.is_some()).map(|l| l.at).collect();
    let sealed: Vec<Point> = built.locks.iter().filter(|l| l.key.is_none()).map(|l| l.at).collect();
    for (shut, tag, by_wall) in [(locked, KEY_TAG, false), (sealed, LEVER_TAG, true)] {
        let Some(&door) = shut.first() else { continue };
        if let Some(at) = fixture_cell(built, tiles, prefabs, door, by_wall, &mut rng) {
            built.spots.push(Spot { tag, at, prefab: None });
        }
    }
}

/// Where a vault's key or lever lies: on a cell walked to from the way in
/// without passing any lock or seal, since a piece behind one grows
/// nothing past it and could never hold its own way out; at least
/// [`MIN_DISTANCE_FROM_ENTRY`] from the way in and off the way down; on
/// plain ground [`PIECE_CLEAR`] from anything a piece laid; and drawn
/// among the third of
/// such cells walked farthest from the vault's own door, so finding it is
/// a search of the floor rather than a step beside the door. A lever
/// stands against a wall, as a lever is set in one. `None` on a floor
/// with no such cell, which leaves the vault shut for good rather than
/// its fixture somewhere it cannot be reached.
fn fixture_cell(built: &PlaceBuild, tiles: &TileRegistry, prefabs: &Prefabs<MonsterDef>, door: Point, by_wall: bool, rng: &mut impl Rng) -> Option<Point> {
    let tables = tiles.tables();
    let shut: BTreeSet<Point> = built.locks.iter().map(|l| l.at).collect();
    let passable = |p: Point| !shut.contains(&p) && built.terrain.get(p).is_some_and(|t| tables.passable[t.index()]);
    let reached = walk_from(built.entry, passable);
    let from_door = walk_from(door, |p| p == door || passable(p));
    // Ground a body crosses at a step's cost, no doorway and nothing that
    // burns or gives way: what a key lies on and a lever stands beside.
    let plain = |p: Point| {
        built.terrain.get(p).map(|t| t.index()).is_some_and(|i| {
            tables.walkable[i]
                && tables.move_cost[i] <= 100
                && tables.opens[i].is_none()
                && tables.closes[i].is_none()
                && tables.burns[i].is_none()
                && tables.names[i] != "rotten planks"
        })
    };
    // A spot a piece laid, and not a dressing's slot: a key or a lever
    // keeps off every piece, so none lies in a vault that floods or on
    // planks that fall.
    let pieces = |s: &Spot| s.prefab.is_some_and(|k| prefabs.defs().try_get(Id::from_raw(k)).is_some());
    let mut cells: Vec<(u32, Point)> = reached
        .keys()
        .copied()
        .filter(|p| geometry::chebyshev(*p, built.entry) >= MIN_DISTANCE_FROM_ENTRY && Some(*p) != built.exit && plain(*p))
        .filter(|p| !built.spots.iter().any(|s| s.at == *p || (pieces(s) && geometry::chebyshev(s.at, *p) <= PIECE_CLEAR)))
        .filter(|p| !by_wall || Direction::CARDINALS.iter().any(|d| built.terrain.get(*p + d.offset()).is_some_and(|t| !tables.passable[t.index()])))
        .filter_map(|p| from_door.get(&p).map(|d| (*d, p)))
        .collect();
    cells.sort();
    let far = cells.split_off(cells.len() * 2 / 3);
    (!far.is_empty()).then(|| far[rng.random_range(0..far.len())].1)
}

/// How many steps each cell `open` admits is from `from`, one orthogonal
/// step at a time: the strictest reading of the movers' rule, so a cell
/// it reaches is one every mover reaches too. A `BTreeMap` for the same
/// cells in the same order on every run.
fn walk_from(from: Point, open: impl Fn(Point) -> bool) -> BTreeMap<Point, u32> {
    let mut seen = BTreeMap::from([(from, 0)]);
    let mut queue = VecDeque::from([from]);
    while let Some(p) = queue.pop_front() {
        let d = seen[&p];
        for dir in Direction::CARDINALS {
            let n = p + dir.offset();
            if open(n) && !seen.contains_key(&n) {
                seen.insert(n, d + 1);
                queue.push_back(n);
            }
        }
    }
    seen
}

/// Every cell of `place` walked to from its way in with each lock and seal
/// still shut taken for a wall and each cell a secret hides taken for what
/// it really is: everywhere a body put down could walk back from. A fall
/// and a far step land only among these, so nobody is ever set down
/// behind a vault door whose key lies outside it, under a gate lifted
/// only from without, or past a way back fallen in. A secret counts as
/// found, since whoever lands beside one finds it as anyone does; a lock
/// once opened is off the place, and the vault behind it is then as open
/// as any room.
pub fn open_to_the_way_in(place: &PlaceMap, tables: &TileTables) -> BTreeSet<Point> {
    let tile = |p: Point| place.secrets.iter().find(|s| s.at == p).map(|s| s.is).or_else(|| place.terrain.get(p));
    let passable = |p: Point| !place.locks.iter().any(|l| l.at == p) && tile(p).is_some_and(|t| tables.passable[t.index()]);
    walk_from(place.entry, passable).into_keys().collect()
}

/// Hides every [`CAUSEWAY`] mark a piece laid as the deep water round it,
/// an engine secret whose real tile is the sunken causeway, so the island
/// it leads to is reached once it is spotted and the mark itself is never
/// filled. One group for the whole causeway, numbered past every group the
/// floor already hid, so one sighting finds all of it.
fn hide_causeways(built: &mut PlaceBuild, tiles: &TileRegistry, prefabs: &Prefabs<MonsterDef>) {
    let is_causeway = |s: &Spot| s.tag == CAUSEWAY as u32 && s.prefab.is_some_and(|key| matches!(prefabs.slot(key, CAUSEWAY), Some(Slot::Mark)));
    let cells: Vec<Point> = built.spots.iter().filter(|s| is_causeway(s)).map(|s| s.at).collect();
    if cells.is_empty() {
        return;
    }
    built.spots.retain(|s| !is_causeway(s));
    let (deep, causeway) = (tiles.expect("deep water"), tiles.expect("sunken causeway"));
    let group = built.secrets.iter().map(|s| s.group + 1).max().unwrap_or(0);
    for at in cells {
        built.terrain.set(at, deep);
        built.secrets.push(rl_engine::rl_mapgen::Secret { at, is: causeway, spot: CAUSEWAY_SPOT, group, joins: JOINS_NO_AREA });
    }
}

/// A locked vault's key, and which floor's vault it opens: what the save
/// keeps of it beside its name, since the tag the lock asks for is put on
/// by [`key_tags`] from this rather than written in `assets/items.ron`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultKey {
    /// The floor whose vault it opens.
    pub floor: u32,
}

/// The tag `floor`'s key carries, [`crate::content::vault_key`] looked up.
pub fn key_tags(registries: &Registries, floor: u32) -> Tagged {
    Tagged(registries.tags.id(&crate::content::vault_key(floor)).into_iter().collect())
}

/// What filling a floor's marks reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Filling<'w> {
    map: Res<'w, WorldMap>,
    registries: Res<'w, Registries>,
    armory: Res<'w, Armory>,
    bestiary: Res<'w, Bestiary>,
    prefabs: Res<'w, Prefabs<MonsterDef>>,
    verbs: Res<'w, Verbs>,
    seed: Res<'w, Seed>,
}

/// Fills every mark of this module's own on the arrival that first finds
/// a floor: the key and the lever [`place_vault_fixtures`] dropped, and
/// each piece's [`SLEEPER`], [`GIANT`], [`GOBLIN`] and [`CAPTIVE`]. A
/// sleeper is drawn from the floor's own table, on its own stream, so a
/// floor's ordinary people are the same whatever its pieces hold.
///
/// In `TurnSet::React`, after `PrefabSet::Fill`, which has filled every
/// slot a piece's legend names; a `Mark` is the one it leaves.
pub fn spawn_vault_marks(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, fill: Filling) {
    for ev in entered.read().filter(|ev| ev.first) {
        let (Some(place), Some(floor)) = (fill.map.place(ev.map), floor_of(ev.map)) else { continue };
        let mut rng = fill.seed.stream(b"undermere.vaults.marks", u64::from(ev.map.0));
        for spot in &place.spots {
            let mark = spot.prefab.zip(char::from_u32(spot.tag)).filter(|(key, glyph)| matches!(fill.prefabs.slot(*key, *glyph), Some(Slot::Mark)));
            match (spot.tag, mark.map(|(_, glyph)| glyph)) {
                (KEY_TAG, _) => {
                    let Some(def) = fill.armory.defs.id(KEY_ITEM) else { continue };
                    let key = spawn_item(&mut commands, &fill.armory, def, &fill.registries);
                    commands.entity(key).insert((Position(spot.at), OnMap(ev.map), VaultKey { floor }, key_tags(&fill.registries, floor)));
                }
                (LEVER_TAG, _) => {
                    spawn_prop(&mut commands, &fill.registries, fill.registries.props.expect(LEVER), spot.at, ev.map);
                }
                (_, Some(SLEEPER)) => {
                    let Some(id) = fill.bestiary.pick_cast(floor, &fill.registries, &mut rng) else { continue };
                    let made = spawn_monster(&mut commands, fill.bestiary.defs.get(id), spot.at, ev.map, &fill.registries, true);
                    commands.entity(made).insert(Post(spot.at));
                }
                (_, Some(glyph @ (GIANT | GOBLIN))) => {
                    let Some(id) = fill.bestiary.defs.id(if glyph == GIANT { GIANT_KIND } else { GOBLIN_KIND }) else { continue };
                    let made = spawn_monster(&mut commands, fill.bestiary.defs.get(id), spot.at, ev.map, &fill.registries, true);
                    commands.entity(made).insert(Post(spot.at));
                }
                (_, Some(CAPTIVE)) => {
                    let Some(id) = fill.bestiary.pick_cast(floor, &fill.registries, &mut rng) else { continue };
                    let made = spawn_monster(&mut commands, fill.bestiary.defs.get(id), spot.at, ev.map, &fill.registries, false);
                    commands.entity(made).insert(crate::captives::shackled(&fill.registries, &fill.verbs, spot.at));
                    spawn_prop(&mut commands, &fill.registries, fill.registries.props.expect("shackles"), spot.at, ev.map);
                }
                _ => {}
            }
        }
    }
}

/// Pulling a floor's concealed lever lifts every seal on it, the gate
/// raised where it stood, and leaves the lever pulled: a floor draws one
/// sealed vault at most, so there is never a second seal its lever could
/// be meant for. Pulling one already pulled is pulling nothing, since a
/// pulled lever offers nothing.
///
/// In `TurnSet::React`, where every answer to an `Interacted` is.
pub fn answer_pull_lever(
    mut commands: Commands,
    mut done: MessageReader<Interacted>,
    verbs: Res<Verbs>,
    registries: Res<Registries>,
    props: Query<(&PropKind, &Position, &OnMap)>,
    tiles: Res<VaultTiles>,
    mut map: ResMut<WorldMap>,
) {
    let (Some(pull), Some(lever)) = (verbs.get(PULL_VERB), registries.props.id(LEVER)) else { return };
    for ev in done.read().filter(|ev| ev.verb == pull) {
        let Ok((kind, at, on)) = props.get(ev.target) else { continue };
        if kind.0 != lever {
            continue;
        }
        let here = map.current();
        let seals: Vec<Point> = map.place(here).map(|p| p.locks.iter().filter(|l| l.key.is_none()).map(|l| l.at).collect()).unwrap_or_default();
        for seal in &seals {
            map.unlock(here, *seal);
            map.set_tile(*seal, tiles.raised_gate);
        }
        commands.entity(ev.target).despawn();
        spawn_prop(&mut commands, &registries, registries.props.expect(PULLED_LEVER), at.0, on.0);
        let line = if seals.is_empty() { "The lever moves, and nothing answers." } else { "Somewhere, stone grinds against stone." };
        commands.write_message(Tell::new(line, Tones::NOTICE));
    }
}

/// A vault's door opened by its key keeps the key: the opener's key to
/// this floor's vault is taken from their bag as the door swings, so a key
/// is spent on the one lock it was cut for. A door opened again after,
/// with no lock left on it, takes nothing.
///
/// In `TurnSet::React`.
pub fn spend_vault_key(
    mut commands: Commands,
    mut doors: MessageReader<DoorEvent>,
    map: Res<WorldMap>,
    tiles: Res<VaultTiles>,
    keys: Query<&VaultKey>,
    mut bags: Query<&mut Inventory>,
) {
    let Some(floor) = floor_of(map.current()) else { return };
    for ev in doors.read() {
        let DoorEvent::Opened { actor, at } = ev else { continue };
        if map.tile(*at) != Some(tiles.open_vault_door) {
            continue;
        }
        let Ok(mut bag) = bags.get_mut(*actor) else { continue };
        let Some(key) = bag.items.iter().copied().find(|i| keys.get(*i).is_ok_and(|k| k.floor == floor)) else { continue };
        bag.items.retain(|i| *i != key);
        commands.entity(key).despawn();
        commands.write_message(Tell::new("The key turns, and stays in the lock.", Tones::NOTICE));
    }
}
