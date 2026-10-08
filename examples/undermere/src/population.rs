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

/// Where a band or a group is placed: the map, the ground it may stand
/// on and the entry nothing may stand [`MIN_DISTANCE_FROM_ENTRY`] of.
/// Bundled so [`place_band`] takes seven arguments rather than nine, not
/// a ninth thing a band is placed on.
struct Site {
    map: MapId,
    bounds: Rect,
    entry: Point,
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
fn place_band(
    commands: &mut Commands,
    bestiary: &Bestiary,
    registries: &Registries,
    site: &Site,
    band: &Band,
    free: &mut impl FnMut(Point) -> bool,
    rng: &mut impl Rng,
) -> Option<Vec<Point>> {
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
    let leader = spawn_monster(commands, bestiary.defs.get(leader_id), cells[0], site.map, registries);
    for (follower, at) in followers.iter().zip(&cells[1..]) {
        let made = spawn_monster(commands, bestiary.defs.get(*follower), *at, site.map, registries);
        commands.entity(made).insert(Leader(leader));
    }
    Some(cells)
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
    let Stock { bestiary, map, seed, registries, ways } = &stock;
    for ev in entered.read().filter(|ev| ev.first) {
        let (Some(floor), Some(place)) = (floor_of(ev.map), map.place(ev.map)) else { continue };
        let mut rng = seed.stream(b"undermere.population", u64::from(floor));
        let mut marked: Vec<Point> = place.spots.iter().map(|spot| spot.at).collect();
        marked.extend(ways.iter().filter(|(_, on)| on.0 == ev.map).map(|(p, _)| p.0));
        let bounds = place.terrain.bounds();
        let site = Site { map: ev.map, bounds, entry: ev.entry };
        for band in BANDS.iter().filter(|b| (b.floors.0..=b.floors.1).contains(&floor)) {
            let mut free = |p: Point| map.is_walkable_on(ev.map, p) && !marked.contains(&p);
            if let Some(used) = place_band(&mut commands, bestiary, registries, &site, band, &mut free, &mut rng) {
                marked.extend(used);
            }
        }
        let mut free = |p: Point| map.is_walkable_on(ev.map, p) && !marked.contains(&p);
        let groups = plan(&bestiary.spawns, floor as i32, bounds, ev.entry, groups_on(floor), &mut free, &mut rng);
        for (id, at) in groups.into_iter().flatten() {
            spawn_monster(&mut commands, bestiary.defs.get(id), at, ev.map, registries);
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
pub fn lead_lair_bands(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, bestiary: Res<Bestiary>, posted: Query<(Entity, &Kind, &OnMap), With<Post>>) {
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
                // The throne's own six, the king and his court, on floor
                // ten alone; elsewhere nothing, unless this seed draws one
                // of task 8a's own lairs on this floor, whose boss and
                // entourage are posted the same way and never more than
                // five strong.
                let court = spawned.len() - people.len();
                if floor == FLOORS {
                    assert_eq!(court, 6, "seed {s}, floor {floor}: the king and his court");
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
}
