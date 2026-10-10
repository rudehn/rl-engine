//! The risk-and-reward pool's own mechanisms: whatever no effect already
//! says.
//!
//! A vault's key and a seal's lever never stand inside the piece they
//! open, so [`place_vault_fixtures`] drops each on a cell walkable from
//! the way in without passing any lock, once a floor is built, and
//! [`spawn_vault_marks`] fills it in on the arrival that finds it, the way
//! [`rl_engine::rl_bevy::prefabs::fill_prefabs`] fills an ordinary slot.
//! [`answer_pull_lever`] is what a pull does: the one seal on the floor
//! with no key is the one it lifts.
//!
//! The rest are a piece's own `Mark`s, read by [`spawn_vault_marks`] the
//! same way, each kept to its own piece by the prefab key a `Spot`
//! carries, since more than one of these pieces can stand on a floor and
//! their marks are not all spelled differently: caged altars whose
//! cages [`shut_other_cages`] slams once one is taken, a chained
//! captive [`spawn_vault_marks`] shackles in place, a flooding vault
//! [`start_flood`] and [`rise_floods`] fill, a rotten floor
//! [`trigger_rotten_floor`] drops its taker through, by the reusable
//! [`fall_through_floor`] and [`land_falling`] a later trap reuses, and a
//! collapsing gallery [`collapse_gallery_behind`] walls over.

use std::collections::{BTreeSet, VecDeque};

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::EffectRng;
use rl_engine::rl_bevy::places::PlaceEntered;
use rl_engine::rl_bevy::prefabs::Prefabs;
use rl_engine::rl_bevy::props::spawn_prop;
use rl_engine::rl_core::geometry;
use rl_engine::rl_grid::{Terrain, TileRegistry, TileTables};

use crate::armory::{Armory, spawn_item};
use crate::bestiary::{Bestiary, MonsterDef, spawn_monster_now};

/// The tile ids these mechanisms paint with at runtime, read once from
/// `Undermere::tiles` in `run::prepare`, since nothing after that call
/// may ask a tile by name directly.
#[derive(Resource, Debug, Clone, Copy)]
pub struct VaultTiles {
    /// Water risen just far enough to wade: a flooding vault's first
    /// stage.
    pub flooded: TileId,
    /// Water too deep to wade, at a flooding vault's far wall.
    pub deep: TileId,
    /// A hall's own wall, painted back in behind whoever lets a gallery
    /// fall in.
    pub wall: TileId,
}

impl VaultTiles {
    /// Reads the three tiles by name, panicking if `tiles` lacks one:
    /// every biome's own recipe shares `hall wall`, `flooded floor` and
    /// `deep water` (`dungeon.rs`'s own per-band tile lists), so this
    /// never fails once the game's own tiles load.
    pub fn new(tiles: &TileRegistry) -> Self {
        Self { flooded: tiles.expect("flooded floor"), deep: tiles.expect("deep water"), wall: tiles.expect("hall wall") }
    }
}

/// A sentinel tag `place_vault_fixtures` gives the locked vault's own
/// key: higher than any glyph a prefab's legend could carry, so it never
/// collides with one, and `Spot::prefab` is `None` for it, since no
/// piece's own legend names it.
const KEY_TAG: u32 = u32::MAX - 1;
/// As [`KEY_TAG`], for the sealed vault's own lever.
const LEVER_TAG: u32 = u32::MAX - 2;

/// After a floor builds, drops the locked vault's key or the sealed
/// vault's lever on a cell walkable from the way in without crossing any
/// lock: "elsewhere on the floor", read literally, since a piece behind a
/// lock or a seal grows nothing past it and so could never hold its own
/// way out. Nothing for a floor that drew neither.
///
/// One of each at most: the pool draws at most one sealed vault a run and
/// the lock the locked vault's every instance shares one key's tag, so
/// one key reachable on a floor that has one is enough, and a second
/// would only ever be a second copy of the one the first already is.
pub fn place_vault_fixtures(built: &mut rl_engine::rl_bevy::places::PlaceBuild, tiles: &TileRegistry, seed: RunSeed, map: MapId) {
    if built.locks.is_empty() {
        return;
    }
    let mut rng = seed.rng(SeedDomain::new(b"undermere.vaults"), u64::from(map.0));
    let locked: BTreeSet<Point> = built.locks.iter().map(|l| l.at).collect();
    let mut free: Vec<Point> =
        reachable_cells(built, tiles, &locked).into_iter().filter(|p| *p != built.entry && !built.spots.iter().any(|s| s.at == *p)).collect();
    free.sort_by_key(|p| (p.y, p.x));
    let mut want_key = built.locks.iter().any(|l| l.key.is_some());
    let mut want_lever = built.locks.iter().any(|l| l.key.is_none());
    while (want_key || want_lever) && !free.is_empty() {
        let at = free.remove(rng.random_range(0..free.len()));
        let tag = if want_key {
            want_key = false;
            KEY_TAG
        } else {
            want_lever = false;
            LEVER_TAG
        };
        built.spots.push(rl_engine::rl_bevy::places::Spot { tag, at, prefab: None });
    }
}

/// Every cell walked to from `built.entry`, through every passable cell
/// but a locked or sealed one: the same rule `Survey` draws a route by
/// (`passable`, with every `Lock`'s own cell taken out by hand, since a
/// locked door shares its tile with a plain one and carries nothing in
/// the tables that tells the two apart).
fn reachable_cells(built: &rl_engine::rl_bevy::places::PlaceBuild, tiles: &TileRegistry, locked: &BTreeSet<Point>) -> BTreeSet<Point> {
    let tables = tiles.tables();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    seen.insert(built.entry);
    queue.push_back(built.entry);
    while let Some(p) = queue.pop_front() {
        for n in [p.offset(1, 0), p.offset(-1, 0), p.offset(0, 1), p.offset(0, -1)] {
            if seen.contains(&n) || locked.contains(&n) {
                continue;
            }
            let Some(tile) = built.terrain.get(n) else { continue };
            if !tables.passable[tile.index()] {
                continue;
            }
            seen.insert(n);
            queue.push_back(n);
        }
    }
    seen
}

/// Every cell walked to from `from` through nothing but walkable floor:
/// a flooding vault's own room, bounded by its walls, which `walkable`
/// (never `passable`) stops at exactly as the room's own door does.
fn flood_fill_room(terrain: &Terrain, tables: &TileTables, from: Point) -> Vec<Point> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();
    seen.insert(from);
    queue.push_back(from);
    while let Some(p) = queue.pop_front() {
        for n in [p.offset(1, 0), p.offset(-1, 0), p.offset(0, 1), p.offset(0, -1)] {
            if seen.contains(&n) {
                continue;
            }
            let Some(tile) = terrain.get(n) else { continue };
            if !tables.walkable[tile.index()] {
                continue;
            }
            seen.insert(n);
            queue.push_back(n);
        }
    }
    seen.into_iter().collect()
}

/// A caged altar's own prize, one of three: taking any of them is what
/// [`shut_other_cages`] watches for.
#[derive(Component, Debug, Clone, Copy)]
pub struct CagedPrize(u8);

/// A caged altar's own gate, one of three, open until another of
/// [`CagedPrize`]'s three is taken.
#[derive(Component, Debug, Clone, Copy)]
pub struct CageGate(u8);

/// A flooding vault's own prize: taking it is what [`start_flood`]
/// watches for, and the point it stood at is where the flood begins.
#[derive(Component, Debug, Clone, Copy)]
pub struct FloodTrigger(Point);

/// A rotten floor's own prize: taking it is what [`trigger_rotten_floor`]
/// watches for, and the floor it stood on is the one its taker falls
/// from.
#[derive(Component, Debug, Clone, Copy)]
pub struct RottenFloorTrigger(MapId);

/// A collapsing gallery's own prize: taking it is what
/// [`collapse_gallery_behind`] watches for, and the two points are the
/// cells just inside each of the piece's two entries.
#[derive(Component, Debug, Clone, Copy)]
pub struct GalleryThresholds(Point, Point);

/// Marks a "Captives in Chains" captive as the pool's own, never part 3's
/// run-wide one, which `captives::held_at` reads this to tell apart: both
/// are shackled the identical way, but a run's own captive is drawn once,
/// by `dungeon::captive_floor`, and counting this one against it would
/// make a run look as if it drew two.
#[derive(Component, Debug, Clone, Copy)]
pub struct PoolCaptive;

/// The three things a caged altar holds, named rather than drawn, since
/// which of the three is taken is the choice the risk is about and not a
/// roll.
const CAGE_PRIZES: [&str; 3] = ["war axe", "wand of polymorph", "ring of reaping"];

/// Fills every `Mark` this module owns, on the arrival that finds it: the
/// locked vault's key or the sealed vault's lever on the sentinel spot
/// [`place_vault_fixtures`] left, caged altars' three prizes and three
/// gates, a chained captive shackled in place, a flooding vault's and a
/// rotten floor's own prize, and a collapsing gallery's prize and its two
/// thresholds. Kept to its own piece by the prefab key its spots carry,
/// since more than one of these may stand on a floor and share a glyph.
///
/// In [`TurnSet::React`](rl_engine::rl_bevy::plugin::TurnSet::React),
/// after `PrefabSet::Fill`, which this piece's ordinary slots already
/// filled; a `Mark` is the one slot it always skips.
#[allow(clippy::too_many_arguments)]
pub fn spawn_vault_marks(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    map: Res<WorldMap>,
    registries: Res<Registries>,
    armory: Res<Armory>,
    bestiary: Res<Bestiary>,
    prefabs: Res<Prefabs<MonsterDef>>,
) {
    let key_of = |name: &str| prefabs.defs().id(name).map(|id| id.raw());
    let (caged, captives, flooding, rotten, gallery) =
        (key_of("Caged Altars"), key_of("Captives in Chains"), key_of("The Flooding Vault"), key_of("The Rotten Floor"), key_of("The Collapsing Gallery"));
    for ev in entered.read().filter(|ev| ev.first) {
        let Some(place) = map.place(ev.map) else { continue };
        let mut gallery_points: Vec<Point> = Vec::new();
        let mut gallery_prize: Option<Entity> = None;
        for spot in &place.spots {
            match spot.tag {
                KEY_TAG => {
                    if let Some(def) = armory.defs.id("vault key") {
                        let key = spawn_item(&mut commands, &armory, def, &registries);
                        commands.entity(key).insert((Position(spot.at), OnMap(ev.map)));
                    }
                }
                LEVER_TAG => {
                    if let Some(prop) = registries.props.id("concealed lever") {
                        spawn_prop(&mut commands, &registries, prop, spot.at, ev.map);
                    }
                }
                _ if spot.prefab == caged => {
                    let Some(ch) = char::from_u32(spot.tag) else { continue };
                    if ('1'..='3').contains(&ch) {
                        let idx = ch as u8 - b'1';
                        if let Some(def) = armory.defs.id(CAGE_PRIZES[idx as usize]) {
                            let item = spawn_item(&mut commands, &armory, def, &registries);
                            commands.entity(item).insert((Position(spot.at), OnMap(ev.map), CagedPrize(idx)));
                        }
                    } else if ('4'..='6').contains(&ch) {
                        let idx = ch as u8 - b'4';
                        if let Some(prop) = registries.props.id("cage gate") {
                            let gate = spawn_prop(&mut commands, &registries, prop, spot.at, ev.map);
                            commands.entity(gate).insert(CageGate(idx));
                        }
                    }
                }
                _ if spot.prefab == captives => {
                    if char::from_u32(spot.tag) != Some('C') {
                        continue;
                    }
                    let Some(kind) = bestiary.defs.id("goblin") else { continue };
                    let (at, map_id) = (spot.at, ev.map);
                    commands.queue(move |world: &mut World| {
                        let captive = spawn_monster_now(world, kind, at, map_id);
                        let bundle = {
                            let registries = world.resource::<Registries>();
                            let verbs = world.resource::<Verbs>();
                            crate::captives::shackled(registries, verbs, at)
                        };
                        world.entity_mut(captive).insert((bundle, PoolCaptive));
                    });
                }
                _ if spot.prefab == flooding => {
                    if char::from_u32(spot.tag) != Some('P') {
                        continue;
                    }
                    if let Some(def) = armory.defs.id("potion of life") {
                        let item = spawn_item(&mut commands, &armory, def, &registries);
                        commands.entity(item).insert((Position(spot.at), OnMap(ev.map), FloodTrigger(spot.at)));
                    }
                }
                _ if spot.prefab == rotten => {
                    if char::from_u32(spot.tag) != Some('P') {
                        continue;
                    }
                    if let Some(def) = armory.defs.id("staff of firebolt") {
                        let item = spawn_item(&mut commands, &armory, def, &registries);
                        commands.entity(item).insert((Position(spot.at), OnMap(ev.map), RottenFloorTrigger(ev.map)));
                    }
                }
                _ if spot.prefab == gallery => match char::from_u32(spot.tag) {
                    Some('W') | Some('X') => gallery_points.push(spot.at),
                    Some('P') => {
                        if let Some(def) = armory.defs.id("wand of beckoning") {
                            let item = spawn_item(&mut commands, &armory, def, &registries);
                            commands.entity(item).insert((Position(spot.at), OnMap(ev.map)));
                            gallery_prize = Some(item);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        // The prize and its two thresholds may be read in any order, by
        // the rows they stand in, so the marker that needs both is
        // finished once the whole piece has been seen rather than as
        // each spot is.
        if let (Some(item), [a, b]) = (gallery_prize, gallery_points.as_slice()) {
            commands.entity(item).insert(GalleryThresholds(*a, *b));
        }
    }
}

/// Pulling the concealed lever lifts the one seal on the floor with no
/// key: the pool draws at most one sealed vault a run, so there is never
/// a second to tell it from. Nothing for a floor with none left to lift.
///
/// In `TurnSet::React`, where every game system answering `Interacted`
/// already reads from.
pub fn answer_pull_lever(mut done: MessageReader<Interacted>, verbs: Res<Verbs>, mut map: ResMut<WorldMap>) {
    let Some(pull) = verbs.get("pull") else { return };
    if !done.read().any(|ev| ev.verb == pull) {
        return;
    }
    let here = map.current();
    let Some(seal) = map.place(here).and_then(|p| p.locks.iter().find(|l| l.key.is_none())).map(|l| l.at) else { return };
    map.unlock(here, seal);
}

/// Taking any of the three caged altars' prizes shuts the other two's
/// gates for good: a [`Blocks`] they never had added, never taken off.
/// Which was taken is answered for; the gate beside it is left open,
/// since the one cage already reached is never the one that slams.
///
/// In `TurnSet::React`.
pub fn shut_other_cages(mut commands: Commands, mut picked: MessageReader<ItemEvent>, prizes: Query<&CagedPrize>, gates: Query<(Entity, &CageGate)>) {
    for ev in picked.read() {
        let ItemEvent::PickedUp { item, .. } = ev else { continue };
        let Ok(taken) = prizes.get(*item) else { continue };
        for (gate, guards) in &gates {
            if guards.0 != taken.0 {
                commands.entity(gate).insert(Blocks);
            }
        }
    }
}

/// How many turns a flooding vault takes to climb from its prize taken
/// to deep water at its own far wall.
pub const FLOOD_TURNS: u32 = 6;

/// A flooding vault mid-pour: the far third of its room, which
/// [`rise_floods`] drowns once `at_turn` comes round.
#[derive(Debug)]
struct RisingFlood {
    at_turn: u32,
    far: Vec<Point>,
}

/// Every flooding vault mid-pour, one entry per prize taken.
#[derive(Resource, Debug, Default)]
pub struct RisingFloods(Vec<RisingFlood>);

/// Taking a flooding vault's prize floods its own room: every cell
/// walkable from where the prize stood, bounded by the room's own walls,
/// turns to shallow water at once, and the far third of them, ordered by
/// the walk from the way in so the last is truest to "the far wall", turn
/// to deep water once [`FLOOD_TURNS`] have passed.
///
/// In `TurnSet::React`.
pub fn start_flood(
    mut picked: MessageReader<ItemEvent>,
    triggers: Query<&FloodTrigger>,
    mut map: ResMut<WorldMap>,
    tiles: Res<VaultTiles>,
    turns: Res<rl_engine::rl_bevy::turn::Turns>,
    mut rising: ResMut<RisingFloods>,
) {
    for ev in picked.read() {
        let ItemEvent::PickedUp { item, .. } = ev else { continue };
        let Ok(trigger) = triggers.get(*item) else { continue };
        let here = map.current();
        let Some(place) = map.place(here) else { continue };
        let tables = map.tables().clone();
        let mut cells = flood_fill_room(&place.terrain, &tables, trigger.0);
        let entry = place.entry;
        cells.sort_by_key(|p| geometry::chebyshev(*p, entry));
        let split = cells.len() * 2 / 3;
        let far = cells.split_off(split);
        for c in cells.iter().chain(far.iter()) {
            map.set_tile(*c, tiles.flooded);
        }
        rising.0.push(RisingFlood { at_turn: turns.turn_number() + FLOOD_TURNS, far });
    }
}

/// Carries every mid-pour flooding vault's far third to deep water once
/// its own `at_turn` has come round.
///
/// In `TurnSet::React`.
pub fn rise_floods(mut rising: ResMut<RisingFloods>, mut map: ResMut<WorldMap>, tiles: Res<VaultTiles>, turns: Res<rl_engine::rl_bevy::turn::Turns>) {
    let now = turns.turn_number();
    rising.0.retain_mut(|flood| {
        if now < flood.at_turn {
            return true;
        }
        for c in &flood.far {
            map.set_tile(*c, tiles.deep);
        }
        false
    });
}

/// Someone falling through the floor to the one below, as a trapdoor or
/// a rotten floor's own collapse sends them. Parallel to a potion of
/// descent's own `Sinking`, but never the way up or the way down's own
/// cell, which a fall should never land its taker on by chance.
#[derive(Component, Debug, Clone, Copy)]
pub struct FallingThrough {
    /// The place they fall to.
    pub to: MapId,
    /// What picks the cell they land on, among its free ones once they
    /// are there.
    pub roll: u64,
}

/// Drops `actor`, standing on `from`, through the floor to the one below
/// it: the rotten floor's own collapse, and the reusable half a later
/// trapdoor trap answers the same way down with, so it is written once.
/// Nothing on the deepest floor, since there is nowhere lower to fall to.
pub fn fall_through_floor(world: &mut World, actor: Entity, from: MapId, roll: u64) {
    use crate::dungeon::{FLOORS, floor_of, map_of};
    let Some(floor) = floor_of(from) else { return };
    if floor >= FLOORS {
        return;
    }
    let below = map_of(floor + 1);
    world.entity_mut(actor).insert(FallingThrough { to: below, roll });
    world.write_message(WarpRequest::into_place(actor, below));
}

/// Taking a rotten floor's prize drops its taker through, by
/// [`fall_through_floor`], the roll drawn from the effect stream as
/// anything else chancy in a landing's wake is.
///
/// In `TurnSet::React`.
pub fn trigger_rotten_floor(mut commands: Commands, mut picked: MessageReader<ItemEvent>, triggers: Query<&RottenFloorTrigger>) {
    for ev in picked.read() {
        let ItemEvent::PickedUp { actor, item, .. } = ev else { continue };
        let Ok(trigger) = triggers.get(*item) else { continue };
        let (actor, from) = (*actor, trigger.0);
        commands.queue(move |world: &mut World| {
            let roll = world.resource_mut::<EffectRng>().random::<u64>();
            fall_through_floor(world, actor, from, roll);
        });
    }
}

/// Puts whoever fell through a floor on a free cell of the one they fell
/// to, picked by the roll they fell with, once they have arrived, never
/// the way up or the way down's own cell: `effects/moving.rs`'s own
/// `land_sinking` shape, with the stairs taken out of what is free, for a
/// fall rather than a sinking.
///
/// In `TurnSet::React`, after the arrival's own reactions, by the
/// engine's warp.
pub fn land_falling(
    mut commands: Commands,
    falling: Query<(Entity, &FallingThrough, &OnMap)>,
    map: Res<WorldMap>,
    occupancy: Res<Occupancy>,
    stairs: Query<(&Position, &OnMap), With<crate::stairs::Way>>,
    mut warps: MessageWriter<WarpRequest>,
) {
    for (who, fall, on) in &falling {
        if on.0 != fall.to {
            continue;
        }
        commands.entity(who).remove::<FallingThrough>();
        let Some(place) = map.place(fall.to) else { continue };
        let on_stairs = |p: Point| stairs.iter().any(|(sp, sm)| sm.0 == fall.to && sp.0 == p);
        let open: Vec<Point> =
            place.terrain.bounds().cells().filter(|p| map.is_walkable_on(fall.to, *p) && !occupancy.is_occupied(*p) && !on_stairs(*p)).collect();
        if open.is_empty() {
            continue;
        }
        let at = open[(fall.roll % open.len() as u64) as usize];
        warps.write(WarpRequest { actor: who, to: Destination::Place { map: fall.to, arrive: Arrive::At(at) } });
    }
}

/// Taking a collapsing gallery's prize walls over whichever of its two
/// entries is farther from wherever its taker now stands: the way they
/// came, by the simplifying rule that a piece walked the length of
/// leaves its taker nearer the end they walked to than the one they
/// walked from.
///
/// In `TurnSet::React`.
pub fn collapse_gallery_behind(
    mut picked: MessageReader<ItemEvent>,
    triggers: Query<&GalleryThresholds>,
    positions: Query<&Position>,
    tiles: Res<VaultTiles>,
    mut map: ResMut<WorldMap>,
) {
    for ev in picked.read() {
        let ItemEvent::PickedUp { actor, item, .. } = ev else { continue };
        let (Ok(thresholds), Ok(at)) = (triggers.get(*item), positions.get(*actor)) else { continue };
        let (a, b) = (thresholds.0, thresholds.1);
        let far = if geometry::chebyshev(at.0, a) >= geometry::chebyshev(at.0, b) { a } else { b };
        map.set_tile(far, tiles.wall);
    }
}
