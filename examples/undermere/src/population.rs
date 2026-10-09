//! A floor's people, put down the first time the adventurer arrives on
//! it, as Foundry fills a deck.
//!
//! [`plan`] is the pure half, tested without an `App`: it draws groups
//! from the bestiary's spawn table at a band and stands each round a cell
//! of its own, at least [`MIN_DISTANCE_FROM_ENTRY`] from the way in, on
//! whatever the caller's `free` allows, with no two monsters on one cell.
//! [`populate`] is the Bevy over it: it plans every floor on its
//! first arrival against the floor as built, off every cell a piece marked,
//! and spawns what the plan says, each monster made by
//! [`spawn_monster`] exactly as a piece's own are.
//!
//! The town is never populated: Mereholt has nobody to meet yet, and
//! nothing that fights belongs there. A floor is populated once, on the
//! arrival that built it; the way back up is the way the adventurer came,
//! and what is left on a floor is what they left.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;

use crate::bestiary::{Bestiary, Kind, MonsterDef, spawn_monster};
use crate::dungeon::{FLOORS, MIN_DISTANCE_FROM_ENTRY, floor_of};
use crate::stairs::Way;

/// Groups every floor draws before its depth adds any, a first cut: four,
/// and one more for each floor down, so floor one holds five groups and
/// floor nine thirteen.
pub const BASE_GROUPS: u32 = 4;

/// Groups the throne's floor draws besides its court: few, so the king
/// and his knights are the fight that matters there and the way to them
/// is not a fourteenth floor of the dead. What they are is the spawn
/// table's floor ten, the drowned, the wights and the ogres of the floors
/// above, and never a knight, who is the throne's alone.
pub const THRONE_GROUPS: u32 = 4;

/// The percent chance a plain monster starts a floor asleep, by floor:
/// about half on floor one, falling to about a sixth by floor nine, the
/// controller's own span, interpolated by hand between those two and
/// rounded to the nearest whole percent. Index `0` is floor one.
const SLEEP_PCT: [u32; 9] = [50, 46, 42, 38, 33, 29, 25, 21, 17];

/// The percent chance a monster of `floor` starts asleep: [`SLEEP_PCT`]'s
/// own, doubled and capped at a hundred for a `sleepy` kind, and nought,
/// whatever `floor` says, for a `wakeful` one. Pure, so a test can check
/// the share it draws without an `App`.
pub fn sleep_chance(floor: u32, sleepy: bool, wakeful: bool) -> u32 {
    if wakeful {
        return 0;
    }
    let base = SLEEP_PCT.get(floor as usize - 1).copied().unwrap_or(0);
    if sleepy { (base * 2).min(100) } else { base }
}

/// The most draws a floor makes before it gives up finding room, so one
/// with nowhere left to stand a group stops trying rather than spinning:
/// three times what the deepest floor that asks most asks for. A draw is
/// wasted only when its anchor's whole square is already taken, so a floor
/// comes in short of its groups only where its ground is nearly all taken.
const MAX_DRAWS: u32 = 40;

/// How far round its anchor a group stands, in tiles: a pack of jackals
/// within a square five across, together but not in a line.
const GROUP_SPREAD: i32 = 2;

/// What [`plan`] puts down: each group its own list of who stands where.
pub type Groups = Vec<Vec<(Id<MonsterDef>, Point)>>;

/// How many groups floor `floor` draws: [`BASE_GROUPS`] and the floor's
/// number, but the throne's [`THRONE_GROUPS`].
pub fn groups_on(floor: u32) -> u32 {
    if floor == FLOORS { THRONE_GROUPS } else { BASE_GROUPS + floor }
}

/// Plans a floor's population: up to `target` groups drawn from `table` at
/// `band`, each as many of its kind as the table's group rolls, stood
/// round an anchor drawn from the cells one could stand on.
///
/// A monster stands only inside `bounds`, where `free` allows, at least
/// [`MIN_DISTANCE_FROM_ENTRY`] from `entry`, and never where another of
/// the plan already stands. The anchor is one of those cells, so a group
/// lands round it unless its whole square is taken, and a group that finds
/// nowhere is no group, and another is drawn. Gives up after forty draws,
/// so on ground that is nearly all taken this comes in under `target`,
/// never over it. Each group comes back as its own list, so a caller can
/// tell how many groups landed and not only how many monsters.
pub fn plan(
    table: &BandedTable<Id<MonsterDef>>,
    band: i32,
    bounds: Rect,
    entry: Point,
    target: u32,
    free: &mut impl FnMut(Point) -> bool,
    rng: &mut impl Rng,
) -> Groups {
    // Every cell one could stand on, kept sorted, so whether a cell is still
    // open is a search, and a cell taken is gone for every later group.
    let mut open: Vec<Point> = bounds.cells().filter(|p| geometry::chebyshev(*p, entry) >= MIN_DISTANCE_FROM_ENTRY && free(*p)).collect();
    open.sort_unstable();
    let mut groups: Groups = Vec::new();
    for _ in 0..MAX_DRAWS {
        if groups.len() as u32 >= target || open.is_empty() {
            break;
        }
        let Some((id, count)) = table.pick_group(band, rng) else { break };
        let id = *id;
        let anchor = open[rng.random_range(0..open.len())];
        let mut group = Vec::new();
        for p in geometry::square(anchor, GROUP_SPREAD) {
            if group.len() as u32 >= count {
                break;
            }
            if let Ok(at) = open.binary_search(&p) {
                open.remove(at);
                group.push((id, p));
            }
        }
        if !group.is_empty() {
            groups.push(group);
        }
    }
    groups
}

/// What populating a floor reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Stock<'w, 's> {
    bestiary: Res<'w, Bestiary>,
    map: Res<'w, WorldMap>,
    seed: Res<'w, Seed>,
    registries: Res<'w, Registries>,
    /// Every way on every map, read once a floor so a group never stands
    /// on the stairs it leads to or from: the entry is already kept clear
    /// by its own distance, but nothing else kept a monster off the down
    /// stairs the adventurer has yet to reach.
    ways: Query<'w, 's, (&'static Position, &'static OnMap), With<Way>>,
    /// The clock, for a feeding ground's own scripted remains to carry a
    /// time of death as real as a blow's.
    turns: Res<'w, Turns>,
    /// Interned verbs, for the captive's own "free" offer.
    verbs: Res<'w, Verbs>,
}

/// A leader and the followers it is drawn with, one band to a floor's own
/// band of floors: the dar trio, always all three, and a goblin war band,
/// a goblin leading two of its own kinds. Placed by [`place_band`] before
/// [`plan`]'s own random groups, so neither ever lands on the band's
/// cells, and never the camp, the fire or the watch round a goblin band,
/// which is a lived-in dungeon's and not this.
struct Band {
    /// The first and last floor it comes together on, both included.
    floors: (u32, u32),
    /// Leads; never named among `followers`, and never `led` itself.
    leader: &'static str,
    /// Drawn `count` of, without repeats, `led` every one.
    followers: &'static [&'static str],
    /// How many of `followers` come, besides the leader.
    count: usize,
}

/// The dar trio, floors eight and nine, the blademaster leading; a goblin
/// war band, floors three to six, a goblin leading two of archer,
/// conjurer or mystic, so a war band never doubles one kind.
const BANDS: &[Band] = &[
    Band { floors: (3, 6), leader: "goblin", followers: &["goblin archer", "goblin conjurer", "goblin mystic"], count: 2 },
    Band { floors: (8, 9), leader: "dar blademaster", followers: &["dar priestess", "dar battlemage"], count: 2 },
];

/// A beast that feeds on another kind's remains, one feeding ground to a
/// floor's own band of floors, never both of the same floor: the content
/// sheet names no pairing, so each is chosen for a biome it fits, a
/// scavenger or a grave-eater over something smaller or already dead by
/// nature's own hand of the lake's floors.
struct Feeder {
    /// The first and last floor it comes together on, both included.
    floors: (u32, u32),
    /// What feeds.
    beast: &'static str,
    /// What it feeds on, laid down as the engine's own remains rather
    /// than a scenery corpse, so a necromancer could in principle raise
    /// what a feeding ground leaves exactly as it would raise any other
    /// body.
    prey: &'static str,
}

/// Jackals over a rat on the upper halls, an ogre over a goblin in the
/// caves, a ghoul, which the sheet already has eating the dead, over a
/// skeleton in the crypts and the drowned halls, a troll over a fire imp
/// at the forge: one feeder a floor's band may draw, chosen by hand for
/// what already stands there.
const FEEDERS: &[Feeder] = &[
    Feeder { floors: (1, 2), beast: "jackal", prey: "rat" },
    Feeder { floors: (3, 4), beast: "ogre", prey: "goblin" },
    Feeder { floors: (5, 7), beast: "ghoul", prey: "skeleton" },
    Feeder { floors: (8, 9), beast: "troll", prey: "fire imp" },
];

/// The percent chance a floor whose band has a [`Feeder`] draws one at
/// all: not every floor of a band shows the same scene twice, which is
/// the pillar "no two runs show everything" read down to one flourish.
const FEEDING_GROUND_PCT: u32 = 50;

/// How the feeding ground's own corpse is dressed, a bone-pale glyph
/// rather than whatever the live kind looked like: [`lay_down`] keeps a
/// body's glyph as it was in life, since the engine does not say what
/// remains look like, and task 10's own scene wants its prey to read as
/// dead on sight rather than as the animal it was standing beside its
/// own feeder.
const REMAINS_GLYPH: char = '%';
/// Bone-pale, the remains' own colour.
const REMAINS_COLOR: (f32, f32, f32) = (0.85, 0.82, 0.7);

/// Stands `feeder`'s beast beside its prey's own remains, both free cells
/// near one another, the prey laid down as the engine's own remains
/// rather than spawned already dead, so a necromancer's `raising` finds
/// the same body a real death would have left. The beast keeps its own
/// cell as a one-waypoint `PatrolRoute`, the universal brain's own
/// `Patrol`, so it stands over its meal until something worth fighting
/// for reaches it; not the engine's own `Post`, since `lead_lair_bands`
/// reads every `Post` on a lair's floor as that lair's own entourage.
///
/// `None` for a beast or a prey this build's bestiary does not have by
/// that name, or for ground with no room for both at once.
fn place_feeding_ground(
    commands: &mut Commands,
    stocked: &Stocked,
    site: &Site,
    feeder: &Feeder,
    now: u32,
    free: &mut impl FnMut(Point) -> bool,
    rng: &mut impl Rng,
) -> Option<Vec<Point>> {
    let Stocked { bestiary, registries, .. } = stocked;
    let (beast_id, prey_id) = (bestiary.defs.id(feeder.beast)?, bestiary.defs.id(feeder.prey)?);
    let open: Vec<Point> = site.bounds.cells().filter(|p| geometry::chebyshev(*p, site.entry) >= MIN_DISTANCE_FROM_ENTRY && free(*p)).collect();
    let remains_at = *open.get(rng.random_range(0..open.len().max(1)))?;
    let beast_at = geometry::square(remains_at, 1).find(|p| *p != remains_at && open.contains(p))?;
    let prey = spawn_monster(commands, bestiary.defs.get(prey_id), remains_at, site.map, registries, false);
    commands.queue(move |world: &mut World| rl_engine::rl_bevy::remains::lay_down(world, prey, now, None));
    commands.entity(prey).insert((Glyph::new(REMAINS_GLYPH, Color::srgb(REMAINS_COLOR.0, REMAINS_COLOR.1, REMAINS_COLOR.2)).on_layer(5),));
    let beast = spawn_monster(commands, bestiary.defs.get(beast_id), beast_at, site.map, registries, false);
    // Not the engine's own `Post`: `lead_lair_bands` reads every `Post`
    // on a lair's floor as that lair's own entourage, and a feeding
    // ground's beast is not a mini-boss's. A route of one waypoint is its
    // own cell, held by the universal brain's own `Patrol`, which nothing
    // else ever reads a floor's own placements by.
    commands.entity(beast).insert(crate::tricks::PatrolRoute { waypoints: vec![beast_at], at: 0 });
    Some(vec![remains_at, beast_at])
}

/// The percent chance a floor draws a patrol at all: not every floor
/// shows one, the same reasoning [`FEEDING_GROUND_PCT`] gives.
const PATROL_PCT: u32 = 60;

/// How many waypoints a patrol's own loop walks: the engine keeps no
/// list of a built floor's rooms once it stands, so three points spread
/// across the open ground stand in for "room to room" well enough for a
/// watcher to read as a loop rather than a wander.
const PATROL_WAYPOINTS: usize = 3;

/// How far apart two of a patrol's own waypoints must stand, so three
/// points drawn at random still spread across the floor rather than
/// bunching in one corner.
const PATROL_SPREAD: i32 = 6;

/// Draws a monster of `floor`'s own band to walk a loop of
/// [`PATROL_WAYPOINTS`] points across its open ground, each at least
/// [`PATROL_SPREAD`] from every other, with [`crate::tricks::PatrolRoute`]
/// on it and its own `Patrol` tactic, already in every kind's shared
/// brain, carrying it from one to the next.
///
/// `None` for a floor whose table draws nothing, or one with too little
/// open ground to spread even two waypoints across.
fn place_patrol(commands: &mut Commands, stocked: &Stocked, site: &Site, free: &mut impl FnMut(Point) -> bool, rng: &mut impl Rng) -> Option<Vec<Point>> {
    if !rng.random_ratio(PATROL_PCT, 100) {
        return None;
    }
    let Stocked { bestiary, registries, .. } = stocked;
    let (id, _) = bestiary.spawns.pick_group(site.floor as i32, rng)?;
    let open: Vec<Point> = site.bounds.cells().filter(|p| geometry::chebyshev(*p, site.entry) >= MIN_DISTANCE_FROM_ENTRY && free(*p)).collect();
    if open.is_empty() {
        return None;
    }
    let mut waypoints = vec![open[rng.random_range(0..open.len())]];
    for _ in 1..PATROL_WAYPOINTS {
        let candidates: Vec<Point> = open.iter().filter(|p| waypoints.iter().all(|w| geometry::chebyshev(**p, *w) >= PATROL_SPREAD)).copied().collect();
        if candidates.is_empty() {
            break;
        }
        waypoints.push(candidates[rng.random_range(0..candidates.len())]);
    }
    if waypoints.len() < 2 {
        return None;
    }
    let walker = spawn_monster(commands, bestiary.defs.get(*id), waypoints[0], site.map, registries, false);
    commands.entity(walker).insert(crate::tricks::PatrolRoute { waypoints: waypoints.clone(), at: 0 });
    Some(waypoints)
}

/// Stands a monster of `floor`'s own band shackled and neutral, offering
/// [`crate::captives::FREE_VERB`] through the engine's own actor offers,
/// with shackles lying at its feet: task 10's own captive, the one a run
/// draws drawn by [`crate::dungeon::captive_floor`].
///
/// Held in place by a one-waypoint `PatrolRoute`, the universal brain's
/// own `Patrol`, exactly as a feeding ground's beast is rather than the
/// engine's own `Post`: `lead_lair_bands` reads every `Post` on a lair's
/// own floor as that lair's entourage, and a captive drawn by the
/// population is not. Put on [`crate::content::CAPTIVE`], neutral to
/// every side, rather than left on whatever faction its kind's row
/// names: a shackled monster answers to nobody until it is freed, when
/// [`crate::captives::free`] takes the route off and lets it move.
///
/// `None` for a floor whose table draws nothing, or ground with no free
/// cell at all.
fn place_captive(commands: &mut Commands, stocked: &Stocked, site: &Site, free: &mut impl FnMut(Point) -> bool, rng: &mut impl Rng) -> Option<Vec<Point>> {
    let Stocked { bestiary, registries, verbs } = stocked;
    let (id, _) = bestiary.spawns.pick_group(site.floor as i32, rng)?;
    let open: Vec<Point> = site.bounds.cells().filter(|p| geometry::chebyshev(*p, site.entry) >= MIN_DISTANCE_FROM_ENTRY && free(*p)).collect();
    let at = *open.get(rng.random_range(0..open.len().max(1)))?;
    let captive = spawn_monster(commands, bestiary.defs.get(*id), at, site.map, registries, false);
    let side = registries.factions.expect(crate::content::CAPTIVE);
    let free_verb = verbs.get(crate::captives::FREE_VERB).expect("UndermerePlugin declares the free verb");
    commands.entity(captive).insert((
        Faction(side),
        crate::tricks::PatrolRoute { waypoints: vec![at], at: 0 },
        Offers(vec![Offering::new(free_verb, crate::captives::FREE_TIME)]),
    ));
    let shackles = registries.props.expect("shackles");
    spawn_prop(commands, registries, shackles, at, site.map);
    Some(vec![at])
}

/// Where a band or a group is placed: the map, the ground it may stand
/// on, the entry nothing may stand [`MIN_DISTANCE_FROM_ENTRY`] of, and the
/// floor's own number. Bundled so a placing function takes the one
/// argument rather than four, not a fourth or fifth thing a band, a
/// feeding ground, a patrol or a captive is placed on.
struct Site {
    map: MapId,
    bounds: Rect,
    entry: Point,
    floor: u32,
}

/// What every one of task 10's placing functions reads from the
/// bestiary's own content, bundled for the same reason [`Site`] is: a
/// function reads only the field its own scene needs, and clippy's limit
/// on a function's own arguments is never the reason one of them is
/// shaped as it is.
struct Stocked<'a> {
    bestiary: &'a Bestiary,
    registries: &'a Registries,
    verbs: &'a Verbs,
}

/// Stands one `band` together on free cells near an anchor drawn the same
/// way [`plan`]'s own groups are, the leader first and the rest of
/// `band.count`'s own followers, each wired with the engine's own
/// `Leader` to the one it leads, and says which cells it used, for the
/// caller to keep `plan`'s own groups off them.
///
/// `None` for a leader or a follower this build's bestiary does not have
/// by that name, or for a floor with no room for the whole band at once:
/// a leader with nobody beside it is no band, and `plan`'s own groups
/// fill in whatever room is left.
///
/// Who starts asleep is [`sleep_chance`]'s own roll for `floor`, one draw
/// each, unless `camp` says this band is dressed as a goblin camp round
/// its fire: then the leader alone keeps the watch and every follower
/// sleeps, whatever its own kind's share would have said.
fn place_band(
    commands: &mut Commands,
    stocked: &Stocked,
    site: &Site,
    band: &Band,
    free: &mut impl FnMut(Point) -> bool,
    rng: &mut impl Rng,
    camp: bool,
) -> Option<Vec<Point>> {
    let Stocked { bestiary, registries, .. } = stocked;
    let leader_id = bestiary.defs.id(band.leader)?;
    let mut followers: Vec<Id<MonsterDef>> = band.followers.iter().filter_map(|name| bestiary.defs.id(name)).collect();
    if followers.len() != band.followers.len() {
        return None;
    }
    // A Fisher-Yates shuffle over the registered ids, so two of three is
    // drawn without repeats and the one left out is as likely to be any
    // of them.
    for i in (1..followers.len()).rev() {
        followers.swap(i, rng.random_range(0..=i));
    }
    followers.truncate(band.count);
    let open: Vec<Point> = site.bounds.cells().filter(|p| geometry::chebyshev(*p, site.entry) >= MIN_DISTANCE_FROM_ENTRY && free(*p)).collect();
    if open.is_empty() {
        return None;
    }
    let anchor = open[rng.random_range(0..open.len())];
    let wanted = 1 + followers.len();
    let mut cells = vec![anchor];
    for p in geometry::square(anchor, GROUP_SPREAD) {
        if cells.len() >= wanted {
            break;
        }
        if p != anchor && open.contains(&p) {
            cells.push(p);
        }
    }
    if cells.len() < wanted {
        return None;
    }
    let leader_def = bestiary.defs.get(leader_id);
    let leader_asleep = !camp && rng.random_ratio(sleep_chance(site.floor, leader_def.tricks.sleepy, leader_def.tricks.wakeful), 100);
    let leader = spawn_monster(commands, leader_def, cells[0], site.map, registries, leader_asleep);
    for (follower, at) in followers.iter().zip(&cells[1..]) {
        let def = bestiary.defs.get(*follower);
        let asleep = camp || rng.random_ratio(sleep_chance(site.floor, def.tricks.sleepy, def.tricks.wakeful), 100);
        let made = spawn_monster(commands, def, *at, site.map, registries, asleep);
        commands.entity(made).insert(Leader(leader));
    }
    Some(cells)
}

/// A free cell close to `cells[0]`, outside every cell `cells` already
/// holds, for a goblin camp's own fire to stand on beside its band
/// without sharing a blocking monster's cell. `None` on ground too
/// cramped to spare one.
fn camp_fire_cell(cells: &[Point], free: &mut impl FnMut(Point) -> bool) -> Option<Point> {
    let anchor = cells[0];
    [1, 2].into_iter().find_map(|radius| geometry::square(anchor, radius).find(|p| !cells.contains(p) && free(*p)))
}

/// Populates every floor on the arrival that built it: a band of its own,
/// the dar trio or a goblin war band wherever [`BANDS`] names its floor,
/// then [`groups_on`]'s number of ordinary groups, drawn at its own band,
/// planned against the floor as built and off every cell a piece marked,
/// where a chest, a brazier or the king's court stands or will stand even
/// where the engine's slot drew nothing, off every way the floor has, so
/// a group never stands on the stairs themselves, and off a band's own
/// cells, so neither ever lands on the other's.
///
/// Draws from `Seed::stream(b"undermere.population", floor)`, a stream for
/// each floor alone, so what stands on floor two never depends on how many
/// draws floor one happened to make, or on whether it was entered first.
pub fn populate(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, stock: Stock) {
    let Stock { bestiary, map, seed, registries, ways, turns, verbs } = &stock;
    for ev in entered.read().filter(|ev| ev.first) {
        let (Some(floor), Some(place)) = (floor_of(ev.map), map.place(ev.map)) else { continue };
        let mut rng = seed.stream(b"undermere.population", u64::from(floor));
        let mut marked: Vec<Point> = place.spots.iter().map(|spot| spot.at).collect();
        marked.extend(ways.iter().filter(|(_, on)| on.0 == ev.map).map(|(p, _)| p.0));
        let bounds = place.terrain.bounds();
        let site = Site { map: ev.map, bounds, entry: ev.entry, floor };
        let stocked = Stocked { bestiary, registries, verbs };
        for band in BANDS.iter().filter(|b| (b.floors.0..=b.floors.1).contains(&floor)) {
            // Half the time a goblin war band is placed at all, it comes
            // together as a camp round its own fire instead of standing
            // in the open: the controller's own share for task 10's camps.
            let camp = band.leader == "goblin" && rng.random_ratio(1, 2);
            let mut free = |p: Point| map.is_walkable_on(ev.map, p) && !marked.contains(&p);
            if let Some(used) = place_band(&mut commands, &stocked, &site, band, &mut free, &mut rng, camp) {
                if camp {
                    let mut free = |p: Point| map.is_walkable_on(ev.map, p) && !marked.contains(&p);
                    if let Some(fire_at) = camp_fire_cell(&used, &mut free) {
                        let fire = registries.props.expect("fire");
                        spawn_prop(&mut commands, registries, fire, fire_at, ev.map);
                        marked.push(fire_at);
                    }
                }
                marked.extend(used);
            }
        }
        if let Some(feeder) = FEEDERS.iter().find(|f| (f.floors.0..=f.floors.1).contains(&floor))
            && rng.random_ratio(FEEDING_GROUND_PCT, 100)
        {
            let mut free = |p: Point| map.is_walkable_on(ev.map, p) && !marked.contains(&p);
            if let Some(used) = place_feeding_ground(&mut commands, &stocked, &site, feeder, turns.now(), &mut free, &mut rng) {
                marked.extend(used);
            }
        }
        {
            let mut free = |p: Point| map.is_walkable_on(ev.map, p) && !marked.contains(&p);
            if let Some(used) = place_patrol(&mut commands, &stocked, &site, &mut free, &mut rng) {
                marked.extend(used);
            }
        }
        if floor == crate::dungeon::captive_floor(seed.0) {
            let mut free = |p: Point| map.is_walkable_on(ev.map, p) && !marked.contains(&p);
            if let Some(used) = place_captive(&mut commands, &stocked, &site, &mut free, &mut rng) {
                marked.extend(used);
            }
        }
        // Its own stream, not `rng`'s current position: task 10's own
        // bands, camps, feeding grounds, patrols and captives all draw
        // from `rng` ahead of this, floor by floor and run by run, and
        // `plan` drawing from the same position they left it at would
        // mean every ordinary group's own kind and place depends on
        // whether this floor drew a camp or a captive, which is no
        // business of either. Isolating it here is what keeps a floor's
        // ordinary people exactly who they were before any of that
        // existed, for whichever seed happens to land one.
        let mut free = |p: Point| map.is_walkable_on(ev.map, p) && !marked.contains(&p);
        let mut plan_rng = seed.stream(b"undermere.population.plan", u64::from(floor));
        let groups = plan(&bestiary.spawns, floor as i32, bounds, ev.entry, groups_on(floor), &mut free, &mut plan_rng);
        for (id, at) in groups.into_iter().flatten() {
            let def = bestiary.defs.get(id);
            let asleep = rng.random_ratio(sleep_chance(floor, def.tricks.sleepy, def.tricks.wakeful), 100);
            spawn_monster(&mut commands, def, at, ev.map, registries, asleep);
        }
    }
}

/// Wires every entourage member a lair's own prefab just stood up to the
/// one mini-boss among them with the engine's own `Leader`, the moment a
/// floor's arrival fills its slots: task 8a's own band, brought together
/// the same way [`place_band`]'s already are, but by the engine's prefab
/// slots rather than by this module's own placement. Read by
/// [`MonsterDef::miniboss`] rather than a name, since nothing else `Post`
/// marks is ever a monster but a slot's own, and the throne's court
/// stands no miniboss among it to lead them.
///
/// After [`PrefabSet::Fill`], in the same pass, so a lair's band is whole
/// before anything decides a turn.
pub fn lead_lair_bands(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    bestiary: Res<Bestiary>,
    posted: Query<(Entity, &Kind, &OnMap), With<Post>>,
) {
    for ev in entered.read().filter(|ev| ev.first) {
        let (mut boss, mut followers) = (None, Vec::new());
        for (e, kind, on) in &posted {
            if on.0 != ev.map {
                continue;
            }
            if bestiary.defs.get(kind.0).miniboss {
                boss = Some(e);
            } else {
                followers.push(e);
            }
        }
        if let Some(boss) = boss {
            for follower in followers {
                commands.entity(follower).insert(Leader(boss));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_rules::prefab::Slot;

    use super::*;
    use crate::dungeon::{TOWN, map_of};
    use crate::testing::{adventurer, arrive, headless, sturdy};

    /// A floor's worth of open ground, larger than any floor, with nothing
    /// in the way: what `plan` does with no map to fall short on.
    fn open_ground() -> (Rect, Point) {
        (Rect::new(0, 0, 80, 50), Point::new(40, 25))
    }

    fn bestiary() -> Bestiary {
        crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default()).bestiary
    }

    /// Four groups and one more for each floor down, to floor nine's
    /// thirteen, and the throne's floor four: what every floor draws where
    /// nothing is in the way.
    #[test]
    fn on_open_ground_every_floor_draws_four_groups_and_one_more_for_each_floor_down_but_the_throne_over_a_span_of_seeds() {
        let bestiary = bestiary();
        let (bounds, entry) = open_ground();
        assert_eq!((1..=FLOORS).map(groups_on).collect::<Vec<_>>(), [5, 6, 7, 8, 9, 10, 11, 12, 13, 4]);
        for floor in 1..=FLOORS {
            for s in 0..20 {
                let mut rng = rand::rngs::StdRng::seed_from_u64(s);
                let groups = plan(&bestiary.spawns, floor as i32, bounds, entry, groups_on(floor), &mut |_| true, &mut rng);
                assert_eq!(groups.len() as u32, groups_on(floor), "floor {floor}, seed {s}");
                assert!(groups.iter().all(|g| !g.is_empty()), "floor {floor}, seed {s}: an empty group");
            }
        }
    }

    /// The throne's floor draws few groups, and only of what walks floors
    /// seven to nine, the drowned, the wights and the ogres, so the
    /// court is the fight that matters there: never a knight of it out on
    /// the floor, and never more than four groups.
    #[test]
    fn the_throne_floor_draws_at_most_four_groups_of_what_walks_floors_seven_to_nine_over_a_span_of_seeds() {
        let bestiary = bestiary();
        let (bounds, entry) = open_ground();
        let deep: Vec<Id<MonsterDef>> = (7..=9).flat_map(|band| bestiary.spawns.at(band).map(|row| row.item)).collect();
        let court = bestiary.defs.expect("skeleton knight");
        let mut kinds: Vec<Id<MonsterDef>> = Vec::new();
        for s in 0..40 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(s);
            let groups = plan(&bestiary.spawns, FLOORS as i32, bounds, entry, groups_on(FLOORS), &mut |_| true, &mut rng);
            assert!(!groups.is_empty() && groups.len() <= 4, "seed {s}: {} groups", groups.len());
            for (id, _) in groups.iter().flatten() {
                assert!(*id != court, "seed {s}: a knight of the court out on the floor");
                assert!(deep.contains(id), "seed {s}: a {} on the throne's floor", bestiary.defs.get(*id).name);
                if !kinds.contains(id) {
                    kinds.push(*id);
                }
            }
        }
        assert!(kinds.len() >= 3, "and mixed: {} kinds over every seed", kinds.len());
    }

    #[test]
    fn nobody_is_planned_near_the_way_in_off_the_ground_or_on_anothers_cell_over_a_span_of_seeds() {
        let bestiary = bestiary();
        let (bounds, entry) = open_ground();
        for s in 0..20 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(s);
            // A checkerboard, so a cell the caller refused is easy to tell
            // from one it allowed.
            let groups = plan(&bestiary.spawns, 9, bounds, entry, 13, &mut |p| (p.x + p.y) % 2 == 0, &mut rng);
            let mut seen: Vec<Point> = Vec::new();
            for (_, p) in groups.iter().flatten() {
                assert_eq!((p.x + p.y) % 2, 0, "seed {s}: {p:?} on a cell the caller refused");
                assert!(geometry::chebyshev(*p, entry) >= MIN_DISTANCE_FROM_ENTRY, "seed {s}: {p:?} too near {entry:?}");
                assert!(bounds.contains(*p), "seed {s}: {p:?} off the floor");
                assert!(!seen.contains(p), "seed {s}: two on {p:?}");
                seen.push(*p);
            }
        }
    }

    #[test]
    fn the_same_seed_plans_the_same_people_every_time_over_a_span_of_seeds() {
        let bestiary = bestiary();
        let (bounds, entry) = open_ground();
        for s in 0..20 {
            let plan_it = || plan(&bestiary.spawns, 5, bounds, entry, 9, &mut |_| true, &mut rand::rngs::StdRng::seed_from_u64(s));
            assert_eq!(plan_it(), plan_it(), "seed {s}");
        }
    }

    /// Where every monster stood the moment it was made, by place, and
    /// what it is: what the population put down, before anybody took a
    /// step.
    #[derive(Resource, Default)]
    struct Spawned(Vec<(MapId, Point, Id<MonsterDef>)>);

    fn watch(app: &mut App) {
        app.init_resource::<Spawned>();
        app.add_observer(|add: On<Add, Kind>, monsters: Query<(&Position, &OnMap, &Kind)>, mut spawned: ResMut<Spawned>| {
            if let Ok((at, on, kind)) = monsters.get(add.entity) {
                spawned.0.push((on.0, at.0, kind.0));
            }
        });
    }

    fn spawned_on(app: &App, map: MapId) -> Vec<Point> {
        app.world().resource::<Spawned>().0.iter().filter(|(on, _, _)| *on == map).map(|(_, at, _)| *at).collect()
    }

    /// What was made on `map` at `at`, by name.
    fn made_at(app: &App, map: MapId, at: Point) -> String {
        let bestiary = app.world().resource::<Bestiary>();
        let made = app.world().resource::<Spawned>().0.iter().find(|(on, p, _)| *on == map && *p == at).map(|(_, _, kind)| *kind);
        made.map_or_else(String::new, |kind| bestiary.defs.get(kind).name.clone())
    }

    /// Every floor, on the arrival that built it, has its people: as many
    /// groups as [`groups_on`] says, every monster at least the distance
    /// from the way in, none on a cell a piece marked, and on floor ten the
    /// throne's court at its own posts besides, with no knight of it among
    /// the floor's own people. A floor entered again gets nobody new.
    #[test]
    fn a_floor_is_populated_on_first_arrival_away_from_the_entry() {
        for s in 0..3 {
            let mut app = headless(RunSeed(s));
            watch(&mut app);
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            for floor in 1..=FLOORS {
                arrive(&mut app, map_of(floor));
                let world = app.world();
                let place = world.resource::<WorldMap>().place(map_of(floor)).expect("built on arrival");
                let prefabs = world.resource::<Prefabs<MonsterDef>>();
                let posts: Vec<Point> = place
                    .spots
                    .iter()
                    .filter(|spot| {
                        let slot = spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, glyph)| prefabs.slot(key, glyph));
                        matches!(slot, Some(Slot::Monster { .. }))
                    })
                    .map(|spot| spot.at)
                    .collect();
                let marked: Vec<Point> = place.spots.iter().map(|spot| spot.at).collect();
                let spawned = spawned_on(&app, map_of(floor));
                for p in &spawned {
                    assert!(geometry::chebyshev(*p, place.entry) >= MIN_DISTANCE_FROM_ENTRY, "seed {s}, floor {floor}: one made at {p:?}, by the way in");
                }
                let people: Vec<Point> = spawned.iter().copied().filter(|p| !posts.contains(p)).collect();
                assert!(people.iter().all(|p| !marked.contains(p)), "seed {s}, floor {floor}: one of the people on a piece's mark");
                assert!(people.len() as u32 >= groups_on(floor), "seed {s}, floor {floor}: {} people for {} groups", people.len(), groups_on(floor));
                // The throne's own, on floor ten alone: six where this
                // run draws the Drowned King, the king and his five
                // knights, or one alone where it draws the Mother Below
                // or the Ember Wyrm, task 9's own two with no entourage.
                // Elsewhere nothing, unless this seed draws one of task
                // 8a's own lairs on this floor, whose boss and entourage
                // are posted the same way and never more than five
                // strong.
                let court = spawned.len() - people.len();
                if floor == FLOORS {
                    assert!(court == 1 || court == 6, "seed {s}, floor {floor}: the drawn boss alone, or the king and his court: {court}");
                } else {
                    assert!(court == 0 || (1..=5).contains(&court), "seed {s}, floor {floor}: an unexpected court of {court}");
                }
                let knights = people.iter().filter(|p| made_at(&app, map_of(floor), **p) == "skeleton knight").count();
                assert_eq!(knights, 0, "seed {s}, floor {floor}: a knight of the court out among the floor's own people");
            }
            let before = spawned_on(&app, map_of(1)).len();
            arrive(&mut app, map_of(1));
            assert_eq!(spawned_on(&app, map_of(1)).len(), before, "seed {s}: nobody new on floor one the second time");
        }
    }

    /// Mereholt has nobody to fight and nothing lying about, the first time
    /// and every time after, where floor one has both.
    #[test]
    fn the_town_is_never_populated_and_never_scattered() {
        let on = |app: &mut App, map: MapId| {
            let world = app.world_mut();
            let monsters = world.query::<(&Kind, &OnMap)>().iter(world).filter(|(_, on)| on.0 == map).count();
            let lying = world.query_filtered::<&OnMap, (With<Item>, With<Position>)>().iter(world).filter(|on| on.0 == map).count();
            (monsters, lying)
        };
        for s in 0..4 {
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            assert_eq!(on(&mut app, TOWN), (0, 0), "seed {s}: on the run's first arrival");
            arrive(&mut app, map_of(1));
            let (monsters, lying) = on(&mut app, map_of(1));
            assert!(monsters > 0 && lying as u32 >= crate::finds::things_on(1), "seed {s}: floor one has its people and its things: {monsters} and {lying}");
            arrive(&mut app, TOWN);
            assert_eq!(on(&mut app, TOWN), (0, 0), "seed {s}: and on coming back");
        }
    }

    /// Every entity of one of `ids`, standing on `map` and given a
    /// `Leader`, paired with who leads it: both a band's placement and
    /// the ordinary spawn table can put a kind on a floor, so this reads
    /// every one of them and leaves telling a band's own from the
    /// table's to the `Leader` each actually carries, rather than to
    /// which the query happens to find first.
    fn led_among(world: &mut World, map: MapId, ids: &[Id<MonsterDef>]) -> Vec<(Entity, Entity)> {
        let mut q = world.query::<(Entity, &Kind, &OnMap, Option<&Leader>)>();
        q.iter(world)
            .filter(|(_, k, on, leader)| on.0 == map && ids.contains(&k.0) && leader.is_some())
            .map(|(e, _, _, leader)| (e, leader.unwrap().0))
            .collect()
    }

    /// Review Focus 5's placement half, and the brief's own: the dar come
    /// as a band of three, one of each, the blademaster leading, so both
    /// of its followers are given its own `Leader` the moment the floor
    /// that can draw all three is entered, whatever else the ordinary
    /// spawn table also drew of any of the three.
    #[test]
    fn the_dar_trio_is_placed_together_the_blademaster_leading_over_a_span_of_seeds() {
        for s in 0..6u64 {
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            let floor = map_of(8);
            arrive(&mut app, floor);
            let (blademaster_id, followers) = {
                let bestiary = app.world().resource::<Bestiary>();
                (bestiary.defs.expect("dar blademaster"), [bestiary.defs.expect("dar priestess"), bestiary.defs.expect("dar battlemage")])
            };
            let led = led_among(app.world_mut(), floor, &followers);
            assert_eq!(led.len(), 2, "seed {s}: the priestess and the battlemage, led: {led:?}");
            let leader = led[0].1;
            assert!(led.iter().all(|(_, l)| *l == leader), "seed {s}: both led by the same one");
            assert_eq!(app.world().get::<Kind>(leader).map(|k| k.0), Some(blademaster_id), "seed {s}: led by the blademaster");
        }
    }

    /// The brief's other band: a goblin leads two of archer, conjurer or
    /// mystic, drawn without repeats, each given the goblin's own
    /// `Leader`, whatever else the ordinary spawn table also drew.
    #[test]
    fn a_goblin_war_band_is_placed_together_a_goblin_leading_two_of_its_own_kinds_over_a_span_of_seeds() {
        for s in 0..6u64 {
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            let floor = map_of(4);
            arrive(&mut app, floor);
            let (goblin_id, band) = {
                let bestiary = app.world().resource::<Bestiary>();
                (
                    bestiary.defs.expect("goblin"),
                    [bestiary.defs.expect("goblin archer"), bestiary.defs.expect("goblin conjurer"), bestiary.defs.expect("goblin mystic")],
                )
            };
            let led = led_among(app.world_mut(), floor, &band);
            assert_eq!(led.len(), 2, "seed {s}: two of the three, led: {led:?}");
            let leader = led[0].1;
            assert!(led.iter().all(|(_, l)| *l == leader), "seed {s}: both led by the same goblin");
            assert_eq!(app.world().get::<Kind>(leader).map(|k| k.0), Some(goblin_id), "seed {s}: led by a goblin");
        }
    }

    /// Task 8a's own: whichever lair a floor draws holds its boss, found
    /// by `MonsterDef::miniboss` rather than by name, and every other
    /// monster the lair's own slots stood is led by it, `lead_lair_bands`
    /// run the moment the arrival filled them. The Widow's own entourage
    /// is egg sacs, props rather than monsters, so she alone is let
    /// stand with none `Leader`'d to her.
    #[test]
    fn whichever_lair_a_floor_draws_holds_its_boss_and_every_monster_its_own_slots_stood_follows_it() {
        let mut checked = 0;
        for s in 0..20u64 {
            let undermere = crate::dungeon::Undermere::new(RunSeed(s));
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            for (boss, floor) in undermere.lairs() {
                let name = crate::dungeon::MINI_BOSSES[boss].name;
                let map = map_of(floor);
                arrive(&mut app, map);
                let world = app.world_mut();
                let present: Vec<(Entity, Id<MonsterDef>)> =
                    world.query::<(Entity, &Kind, &OnMap)>().iter(world).filter(|(_, _, on)| on.0 == map).map(|(e, k, _)| (e, k.0)).collect();
                let led: Vec<(Entity, Entity)> =
                    world.query::<(Entity, &Leader, &OnMap)>().iter(world).filter(|(_, _, on)| on.0 == map).map(|(e, l, _)| (e, l.0)).collect();
                let bestiary = world.resource::<Bestiary>();
                let Some((boss_entity, kind)) = present.iter().find(|(_, k)| bestiary.defs.get(*k).miniboss) else {
                    continue;
                };
                assert_eq!(bestiary.defs.get(*kind).name, name, "seed {s}, floor {floor}: the lair's own boss by name");
                let followers: Vec<Entity> = led.iter().filter(|(_, leader)| leader == boss_entity).map(|(e, _)| *e).collect();
                if name == "the Widow" {
                    assert!(followers.is_empty(), "seed {s}: the Widow's own entourage is egg sacs, never led monsters: {followers:?}");
                } else {
                    assert!(!followers.is_empty() && followers.len() <= 4, "seed {s}, floor {floor}: {} followers of {name}", followers.len());
                }
                checked += 1;
            }
        }
        assert!(checked > 0, "some lair was drawn and checked over the span");
    }

    /// [`sleep_chance`]'s own hand-written numbers: half on floor one,
    /// about a sixth by floor nine, doubled and capped at a hundred for a
    /// sleepy kind, and nought for a wakeful one whatever the floor.
    #[test]
    fn sleep_chance_falls_with_depth_doubles_for_sleepy_and_is_nought_for_wakeful() {
        assert_eq!(sleep_chance(1, false, false), 50, "about half on floor one");
        assert_eq!(sleep_chance(9, false, false), 17, "about a sixth by floor nine");
        assert!(sleep_chance(1, false, false) > sleep_chance(9, false, false), "falls with depth");
        for floor in 1..=9 {
            assert_eq!(
                sleep_chance(floor, true, false),
                (sleep_chance(floor, false, false) * 2).min(100),
                "floor {floor}: sleepy doubles its floor's own share"
            );
            assert_eq!(sleep_chance(floor, false, true), 0, "floor {floor}: wakeful never sleeps");
            assert_eq!(sleep_chance(floor, true, true), 0, "floor {floor}: wakeful outranks sleepy");
        }
    }

    /// Rolled against [`sleep_chance`]'s own percent over a span of seeds,
    /// a plain kind's own share of sleepers lands within a few points of
    /// what the floor's own number says, on three floors spread across the
    /// span the controller's numbers cover.
    #[test]
    fn a_plain_kinds_own_share_of_sleepers_matches_sleep_chance_over_a_span_of_seeds() {
        for floor in [1u32, 5, 9] {
            let pct = sleep_chance(floor, false, false);
            let trials: u32 = 4000;
            let mut asleep: u32 = 0;
            let mut rng = rand::rngs::StdRng::seed_from_u64(u64::from(floor));
            for _ in 0..trials {
                if rng.random_ratio(pct, 100) {
                    asleep += 1;
                }
            }
            let share = asleep * 100 / trials;
            assert!(share.abs_diff(pct) <= 4, "floor {floor}: rolled {share}%, wanted about {pct}%");
        }
    }

    /// Task 10's camps: half the floors a goblin war band is placed on
    /// floors three to six dress it round a fire, every follower asleep
    /// and the leader alone keeping the watch, found somewhere over a
    /// span of seeds since the dressing itself is a coin flip.
    #[test]
    fn a_goblin_camp_holds_a_fire_its_followers_asleep_and_its_leader_keeping_the_watch_over_a_span_of_seeds() {
        let mut camps = 0;
        for s in 0..40u64 {
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            let map = map_of(4);
            arrive(&mut app, map);
            let fire_id = app.world().resource::<Registries>().props.expect("fire");
            let world = app.world_mut();
            let has_fire = world.query::<(&PropKind, &OnMap)>().iter(world).any(|(k, on)| k.0 == fire_id && on.0 == map);
            if !has_fire {
                continue;
            }
            let (goblin_id, band) = {
                let bestiary = app.world().resource::<Bestiary>();
                (
                    bestiary.defs.expect("goblin"),
                    [bestiary.defs.expect("goblin archer"), bestiary.defs.expect("goblin conjurer"), bestiary.defs.expect("goblin mystic")],
                )
            };
            // Led by a plain goblin, never Grubnak the goblin king: his
            // own honor guard can draw the same three kinds, on the same
            // floor range, and this is the war band's camp alone.
            let led: Vec<(Entity, Entity)> =
                led_among(app.world_mut(), map, &band).into_iter().filter(|(_, leader)| app.world().get::<Kind>(*leader).map(|k| k.0) == Some(goblin_id)).collect();
            assert_eq!(led.len(), 2, "seed {s}: two followers, the camp's own band");
            let leader = led[0].1;
            assert!(led.iter().all(|(_, l)| *l == leader), "seed {s}: both led by the same goblin");
            let world = app.world();
            let asleep_id = world.resource::<Registries>().statuses.expect(crate::content::ASLEEP);
            let sleeps = |e: Entity| world.get::<Afflicted>(e).is_some_and(|a| a.has(asleep_id));
            for (follower, _) in &led {
                assert!(sleeps(*follower), "seed {s}: a camp's own follower sleeps");
            }
            assert!(!sleeps(leader), "seed {s}: the leader keeps the watch, awake");
            camps += 1;
        }
        assert!(camps > 0, "some seed over the span dressed its goblin band as a camp");
    }

    /// Task 10's feeding grounds: a beast stands over its prey's own
    /// remains, a real body the engine's own `Remains` carries rather
    /// than a scenery corpse, found somewhere over a span of seeds since
    /// a floor draws one only [`FEEDING_GROUND_PCT`] of the time.
    #[test]
    fn a_feeding_ground_stands_a_beast_over_its_preys_own_engine_remains_over_a_span_of_seeds() {
        let mut found = 0;
        for s in 0..40u64 {
            let mut app = headless(RunSeed(s));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            let map = map_of(1);
            arrive(&mut app, map);
            let (jackal, rat) = {
                let bestiary = app.world().resource::<Bestiary>();
                (bestiary.defs.expect("jackal"), bestiary.defs.expect("rat"))
            };
            let world = app.world_mut();
            let beasts: Vec<Entity> =
                world.query::<(Entity, &Kind, &OnMap)>().iter(world).filter(|(_, k, on)| k.0 == jackal && on.0 == map).map(|(e, ..)| e).collect();
            let Some(&beast) = beasts.iter().find(|e| world.get::<crate::tricks::PatrolRoute>(**e).is_some()) else { continue };
            let rats: Vec<Entity> =
                world.query::<(Entity, &Kind, &OnMap)>().iter(world).filter(|(_, k, on)| k.0 == rat && on.0 == map).map(|(e, ..)| e).collect();
            let remains = rats.into_iter().find(|e| world.get::<Remains>(*e).is_some());
            let Some(remains) = remains else { continue };
            assert!(world.get::<Actor>(remains).is_none(), "seed {s}: remains are not an actor any more");
            assert!(world.get::<crate::tricks::PatrolRoute>(beast).is_some(), "seed {s}: the beast keeps its own cell over its meal");
            found += 1;
        }
        assert!(found > 0, "some seed over the span drew a feeding ground on floor one");
    }
}
