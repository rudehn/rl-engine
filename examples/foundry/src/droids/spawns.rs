//! Populating a deck on arrival: the first time down, and again on the
//! climb back up.
//!
//! `plan_population` is the pure half, tested without an `App` at all:
//! it draws groups from a [`BandedTable`] at a band and finds each
//! monster a spot at least [`MIN_DISTANCE_FROM_ENTRY`] tiles from the way
//! in, on whatever a `walkable` predicate the caller supplies allows, with
//! no two groups ever sharing a tile. [`populate_deck`] is the one line of
//! Bevy over it: reads a [`PlaceEntered`] for every arrival, plans against
//! the real map at the deck's own band on the way down and at the run's
//! deepest band on the way back up, and spawns what the plan says.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;

use super::{MonsterDef, Roster, spawn_monster};

/// Groups every deck gets before depth adds any more.
const BASE_GROUPS: u32 = 4;

/// Groups placed a deck aims for, on top of the four every deck gets.
const GROUPS_PER_DECK: u32 = 2;

/// No group is placed nearer the entry than this, in tiles, and no deck's
/// start nearer a stamped piece, so no guard at a post stands nearer
/// either.
pub(crate) const MIN_DISTANCE_FROM_ENTRY: i32 = 7;

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

/// Populates a deck on every arrival, the way `examples/delve` fills a
/// floor: follows `plan_population` against the deck's real terrain,
/// never on a cell a piece marked as a spot, and spawns what it plans. A
/// first arrival draws at the deck's own band. A revisit is the climb,
/// and the climb is drawn at the band of the deepest deck the run has
/// reached rather than the deck's own, the way NetHack's ascension run
/// and DCSS's Orb Run both make the way back the harder half: a deck one
/// revisited after the core holds what the deepest deck holds.
///
/// Draws from `Seed::stream(b"foundry.spawns", deck)` on a first arrival
/// and `Seed::stream(b"foundry.climb", deck)` on a revisit, never a combat
/// one: two decks drawing groups from the same stream would let deck
/// two's population depend on how many rooms deck one happened to roll
/// first, and a deck's climb population sharing its first-arrival stream
/// would make the climb the same fight all over again. The stream is
/// derived fresh from the deck on every arrival rather than kept in a
/// persistent generator, so a deck revisited twice draws the same groups
/// both times; a run has no reason to bounce between two decks, and the
/// alternative is a generator this system would have to keep per deck
/// forever just to guard against a case that never happens.
pub fn populate_deck(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, stock: Stock, deepest: Res<crate::climb::Deepest>) {
    let Stock { roster, map, seed, registries } = &stock;
    for ev in entered.read() {
        let Some(place) = map.place(ev.map) else { continue };
        let deck = crate::decks::deck_of(ev.map);
        // A first arrival is the deck's own band. A revisit is the climb,
        // and the climb is drawn at the deepest band the run reached, so
        // the way up is the harder half rather than a walk through decks
        // the commando already emptied.
        let band = if ev.first { deck } else { deepest.0 };
        let mut rng = seed.stream(if ev.first { b"foundry.spawns" } else { b"foundry.climb" }, u64::from(deck));
        let bounds = place.terrain.bounds();
        let target = BASE_GROUPS + band * GROUPS_PER_DECK;
        // Never on a spot a piece marked: a guard, a locker or the reactor
        // console stands there, or will, and the deck's own population
        // keeps off it even where the engine's slot drew nothing.
        let spots: Vec<Point> = place.spots.iter().map(|s| s.at).collect();
        let groups = plan_population(&roster.table, band as i32, bounds, ev.entry, target, &mut |p| map.is_walkable(p) && !spots.contains(&p), &mut rng);
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

    /// The climb is drawn at the deepest band the run reached, so a deck
    /// one revisited after the core holds what the core's neighbours hold
    /// rather than what deck one held on the way down. One table, sampled
    /// deeper, which is how NetHack's ascension run works.
    ///
    /// Two runs of one seed, alike but for how deep they went: what a
    /// revisit adds is what is measured, not what the deck holds, because
    /// every revisit adds somebody whatever the band is. Only the band
    /// tells the two runs apart.
    #[test]
    fn a_revisited_deck_repopulates_at_the_deepest_band_the_run_reached() {
        let added_after = |depth: u32| -> usize {
            let mut app = crate::testing::headless(RunSeed(5));
            crate::testing::arrive_on(&mut app, 1);
            let before = crate::testing::monsters_on(&mut app, 1);
            crate::testing::arrive_on(&mut app, depth);
            crate::testing::arrive_on(&mut app, 1);
            crate::testing::monsters_on(&mut app, 1) - before
        };
        let (shallow, deep) = (added_after(2), added_after(9));
        assert!(deep > shallow, "a climb from deck nine adds more than one from deck two: {shallow} then {deep}");
    }

    /// Two revisits at the same band draw the identical addition both
    /// times: the climb's rng is derived fresh from the deck on every
    /// arrival rather than kept in a generator the game carries forward,
    /// so a deck bounced back onto twice without the run going any
    /// deeper in between gets the same groups twice, not two different
    /// draws from the same stream.
    #[test]
    fn a_deck_revisited_twice_at_the_same_band_draws_the_same_addition_both_times() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::arrive_on(&mut app, 2);
        let before_any_revisit = crate::testing::monsters_on(&mut app, 1);
        crate::testing::arrive_on(&mut app, 1);
        let after_first_revisit = crate::testing::monsters_on(&mut app, 1);
        let first_addition = after_first_revisit - before_any_revisit;
        assert!(first_addition > 0, "a revisit at a deeper band adds somebody");
        // Deck two again, still no deeper than the run has already been,
        // so the climb band for deck one has not moved.
        crate::testing::arrive_on(&mut app, 2);
        crate::testing::arrive_on(&mut app, 1);
        let after_second_revisit = crate::testing::monsters_on(&mut app, 1);
        let second_addition = after_second_revisit - after_first_revisit;
        assert_eq!(second_addition, first_addition, "the same band drew a different addition the second time");
    }
}
