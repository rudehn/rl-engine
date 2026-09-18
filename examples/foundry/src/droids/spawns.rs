//! Populating a deck the first time it is entered.
//!
//! `plan_population` is the pure half, tested without an `App` at all:
//! it draws groups from a [`BandedTable`] at a deck's band and finds each
//! monster a spot at least [`MIN_DISTANCE_FROM_ENTRY`] tiles from the way
//! in, on whatever a `walkable` predicate the caller supplies allows, with
//! no two groups ever sharing a tile. [`populate_deck`] is the one line of
//! Bevy over it: reads a [`PlaceEntered`] for the first arrival, plans
//! against the real map, and spawns what the plan says.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;

use super::{MonsterDef, Roster, spawn_monster};

/// Groups every deck gets before depth adds any more.
const BASE_GROUPS: u32 = 4;

/// Groups placed a deck aims for, on top of the four every deck gets.
const GROUPS_PER_DECK: u32 = 2;

/// No group is placed nearer the entry than this, in tiles.
const MIN_DISTANCE_FROM_ENTRY: i32 = 7;

/// Draws up to `target` groups from `table` at `band`, each one or more
/// monsters at one random anchor within `bounds`, and keeps only the
/// monsters that land on a tile `walkable` allows, at least
/// [`MIN_DISTANCE_FROM_ENTRY`] from `entry`, and not already used by an
/// earlier group in this same call. Gives up after forty draws, so a deck
/// with nowhere left to stand one stops trying rather than spinning: on a
/// cramped or heavily built floor this can come in under `target`, never
/// over it. Returns each group as its own list of `(id, point)` pairs, so
/// a caller can tell how many groups landed, not just how many monsters.
fn plan_population(
    table: &BandedTable<Id<MonsterDef>>,
    band: i32,
    bounds: Rect,
    entry: Point,
    target: u32,
    walkable: &mut impl FnMut(Point) -> bool,
    rng: &mut impl Rng,
) -> Vec<Vec<(Id<MonsterDef>, Point)>> {
    let mut groups: Vec<Vec<(Id<MonsterDef>, Point)>> = Vec::new();
    let mut used: Vec<Point> = Vec::new();
    for _ in 0..40 {
        if groups.len() as u32 >= target {
            break;
        }
        let Some((id, count)) = table.pick_group(band, rng) else { break };
        let id = *id;
        let anchor = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
        let mut group = Vec::new();
        for p in geometry::square(anchor, 2) {
            if group.len() as u32 >= count {
                break;
            }
            if walkable(p) && geometry::chebyshev(p, entry) >= MIN_DISTANCE_FROM_ENTRY && !used.contains(&p) {
                group.push((id, p));
                used.push(p);
            }
        }
        if !group.is_empty() {
            groups.push(group);
        }
    }
    groups
}

/// What populating a deck reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Stock<'w> {
    roster: Res<'w, Roster>,
    map: Res<'w, WorldMap>,
    seed: Res<'w, Seed>,
    registries: Res<'w, Registries>,
}

/// Populates a deck the first time it is entered, the way `examples/delve`
/// fills a floor: follows `plan_population` against the deck's real
/// terrain and spawns what it plans. A revisit is not a first arrival, so
/// `PlaceEntered::first` being false leaves it alone: nobody new.
///
/// Draws from `Seed::stream(b"foundry.spawns", deck)`, the game's own
/// stream, never a combat one: two decks drawing groups from the same
/// stream would let deck two's population depend on how many rooms deck
/// one happened to roll first.
pub fn populate_deck(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, stock: Stock) {
    let Stock { roster, map, seed, registries } = &stock;
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let deck = crate::decks::deck_of(ev.map);
        let mut rng = seed.stream(b"foundry.spawns", deck as u64);
        let bounds = place.terrain.bounds();
        let target = BASE_GROUPS + deck * GROUPS_PER_DECK;
        let groups = plan_population(&roster.table, deck as i32, bounds, ev.entry, target, &mut |p| map.is_walkable(p), &mut rng);
        for (id, p) in groups.into_iter().flatten() {
            spawn_monster(&mut commands, roster, id, p, ev.map, registries);
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::droids::Kind;

    /// A generous open square with room for even the deepest deck's
    /// target, and no terrain to fall short on: `plan_population`'s own
    /// contract, tested with nothing but the predicate the caller passes,
    /// so a shortfall can only mean the logic, never a map.
    fn open_ground() -> (Rect, Point) {
        (Rect::new(0, 0, 80, 60), Point::new(40, 30))
    }

    #[test]
    fn open_ground_always_reaches_the_target_group_count_over_a_span_of_seeds() {
        // Spec: 4 plus twice the deck. Real terrain can fall short of this
        // when a deck is cramped or heavily built, which is why
        // `plan_population` gives up after forty draws rather than
        // spinning forever; open ground with nothing in the way never
        // should, so a shortfall here is the planner's fault, not the
        // map's.
        let roster = Roster::load(&crate::content::registries());
        let (bounds, entry) = open_ground();
        for deck in 1..=8u32 {
            let target = BASE_GROUPS + deck * GROUPS_PER_DECK;
            for s in 0..20 {
                let mut rng = rand::rngs::StdRng::seed_from_u64(s);
                let groups = plan_population(&roster.table, deck as i32, bounds, entry, target, &mut |_| true, &mut rng);
                assert_eq!(groups.len() as u32, target, "deck {deck}, seed {s}");
            }
        }
    }

    #[test]
    fn no_two_groups_ever_share_a_tile_over_a_span_of_seeds() {
        let roster = Roster::load(&crate::content::registries());
        let (bounds, entry) = open_ground();
        for s in 0..20 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(s);
            let groups = plan_population(&roster.table, 8, bounds, entry, BASE_GROUPS + 8 * GROUPS_PER_DECK, &mut |_| true, &mut rng);
            let mut seen: Vec<Point> = Vec::new();
            for (_, p) in groups.iter().flatten() {
                assert!(!seen.contains(p), "seed {s}: two groups on {p:?}");
                seen.push(*p);
            }
        }
    }

    #[test]
    fn every_placed_point_respects_the_walkable_predicate_and_the_minimum_distance_over_a_span_of_seeds() {
        let roster = Roster::load(&crate::content::registries());
        let (bounds, entry) = open_ground();
        for s in 0..20 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(s);
            // A checkerboard, so a point the predicate refused is easy to
            // tell from one it allowed.
            let groups = plan_population(&roster.table, 3, bounds, entry, BASE_GROUPS + 3 * GROUPS_PER_DECK, &mut |p| p.x % 2 == 0, &mut rng);
            for (_, p) in groups.iter().flatten() {
                assert_eq!(p.x % 2, 0, "seed {s}: {p:?} on a tile the predicate refused");
                assert!(geometry::chebyshev(*p, entry) >= MIN_DISTANCE_FROM_ENTRY, "seed {s}: {p:?} too close to {entry:?}");
            }
        }
    }

    #[test]
    fn the_same_seed_plans_the_same_population_every_time_over_a_span_of_seeds() {
        let roster = Roster::load(&crate::content::registries());
        let (bounds, entry) = open_ground();
        for s in 0..20 {
            let plan = || {
                let mut rng = rand::rngs::StdRng::seed_from_u64(s);
                plan_population(&roster.table, 3, bounds, entry, BASE_GROUPS + 3 * GROUPS_PER_DECK, &mut |_| true, &mut rng)
            };
            assert_eq!(plan(), plan(), "seed {s}: two runs of the same seed disagreed");
        }
    }

    #[test]
    fn every_spawn_on_a_real_deck_is_walkable_and_alone_on_its_tile_over_a_span_of_seeds() {
        // Distance from the entry is `plan_population`'s own property,
        // proven above against a fixed plan; a real deck's monsters can
        // take a wander step in the same early frames a test needs to
        // reach them (the scheduler deals every actor due before the
        // player its own next turn within one `app.update()`, and two are
        // spent just settling into the run), so a spawn that landed
        // exactly on the minimum could read one tile closer by the time
        // this looks. Walkability and never sharing a tile hold under a
        // step too, since the engine's own movement never steps an actor
        // onto a wall or another blocker.
        for s in 0..20 {
            let mut app = crate::testing::headless(RunSeed(s));
            app.update();
            app.update();
            let positions: Vec<Point> = {
                let world = app.world_mut();
                let mut q = world.query::<(&Kind, &Position, &OnMap)>();
                q.iter(world).filter(|(_, _, on)| on.0 == crate::decks::map_of(1)).map(|(_, p, _)| p.0).collect()
            };
            assert!(!positions.is_empty(), "seed {s}: deck one got nothing to spawn");
            let map = app.world().resource::<WorldMap>();
            for (i, p) in positions.iter().enumerate() {
                assert!(map.is_walkable(*p), "seed {s}: {p:?} sits on a wall");
                assert!(positions[i + 1..].iter().all(|q| q != p), "seed {s}: two monsters share {p:?}");
            }
        }
    }

    #[test]
    fn revisiting_a_deck_spawns_nobody_new() {
        let mut app = crate::testing::headless(RunSeed(4));
        app.update();
        app.update();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let count = |app: &mut App| -> usize {
            let world = app.world_mut();
            let mut q = world.query::<(&Kind, &OnMap)>();
            q.iter(world).filter(|(_, on)| on.0 == crate::decks::map_of(1)).count()
        };
        let first = count(&mut app);
        assert!(first > 0, "deck one starts with something to fight");
        app.world_mut().write_message(WarpRequest::into_place(player, crate::decks::map_of(2)));
        app.update();
        app.update();
        app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: crate::decks::map_of(1), arrive: Arrive::Entry } });
        app.update();
        app.update();
        assert_eq!(count(&mut app), first, "a second arrival on deck one added nobody");
    }
}
