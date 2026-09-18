//! Items on the decks, and items the dead leave behind.
//!
//! [`plan_scatter`] is the pure half, tested without an `App` at all: it
//! places one item at every `A` mark, two at every `L` (the mark itself
//! and a free floor tile beside it), and a handful more on random floor
//! tiles well clear of any mark, all drawn from a [`BandedTable`] at a
//! deck's band. [`scatter_on_arrival`] is the one line of Bevy over it,
//! the way `droids::spawns::populate_deck` is over `plan_population`:
//! reads a [`PlaceEntered`] for the first arrival, plans against the real
//! map, and spawns what the plan says.
//!
//! [`roll_drops`] is the other pure half: what a dead monster's own drop
//! table pays out. [`drop_on_death`] rolls it from [`Drops`], the game's
//! own stream, on every [`DeathEvent`], never the engine's combat one, so
//! a kill never nudges a single later blow in the run.

use bevy::prelude::*;
use rand::Rng;
use rand::rngs::StdRng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;

use crate::decks::deck_of;
use crate::droids::{Kind, Roster};
use crate::gear::{Armory, ItemDef};

/// No random item lands nearer than this, in Chebyshev tiles, to any mark
/// the same deck also guarantees an item to. Any closer and it would sit
/// inside the very square a store's own second item is allowed to land
/// in, so a scatter that drew a loose item there could never be told
/// apart from the guaranteed one.
const MIN_DISTANCE_FROM_MARKS: i32 = 2;

/// The stream every kill's drop rolls come from: seeded once, from
/// `Seed::stream(b"foundry.drops", 0)`, when the run starts, and never
/// reseeded, so one kill's roll picks up wherever the last one left off
/// rather than every kill starting over from the same point the way a
/// fresh [`Seed::stream`] call would.
///
/// Its own resource rather than a stream a game system derives on the
/// spot: a game's draws are `Seed::stream` kept from where they last
/// stood, and a persistent generator is the only way a second kill's roll
/// can differ from the first one's. Never the engine's own `CombatRng`
/// (`rl_engine::rl_bevy::CombatRng`): the whole point of a stream of its
/// own is that a kill's loot can never shift a blow that has not
/// happened yet.
#[derive(Resource)]
pub struct Drops(pub StdRng);

/// Rolls each of `drops` in the order given: one roll of `0..100` per
/// entry, keeping the ones that land under their own percentage. An entry
/// this misses is simply left out, never retried, so a monster with two
/// rows can drop both, one, or neither on the same death.
pub fn roll_drops(drops: &[(Id<ItemDef>, u32)], rng: &mut impl Rng) -> Vec<Id<ItemDef>> {
    drops.iter().filter(|(_, pct)| rng.random_range(0..100) < *pct).map(|(id, _)| *id).collect()
}

/// A deck's guaranteed loot spots and the floor they sit on, bundled so
/// [`plan_scatter`] takes one fewer argument than clippy's own limit on a
/// function's parameter list starts complaining about.
#[derive(Clone, Copy)]
struct Layout<'a> {
    /// The floor's own bounds, for a random loose item's anchor.
    bounds: Rect,
    /// Where an `A` mark guarantees exactly one item.
    armories: &'a [Point],
    /// Where an `L` mark guarantees two: the mark itself, and a free
    /// floor tile beside it.
    stores: &'a [Point],
}

/// Plans a deck's whole scatter the moment it is entered: one item at
/// each of `layout.armories`, two at each of `layout.stores` (the mark
/// itself, and a free floor tile beside it, [`geometry::square`] radius
/// one), and `extra` more on random floor tiles at least
/// [`MIN_DISTANCE_FROM_MARKS`] from every mark and from each other. Every
/// item is drawn from `table` at `band`. A random slot gives up after
/// forty draws, so a deck with nowhere left to stand one stops trying
/// rather than spinning, the same way `droids::spawns::plan_population`
/// does; a mark the table cannot fill is simply left bare rather than
/// panicking.
///
/// A store's own free tile also keeps [`MIN_DISTANCE_FROM_MARKS`] from
/// every *other* mark, not just clear of `used`: a room's own two `L`s
/// can sit as close as two Chebyshev tiles apart
/// (`decks::Foundry::stores`), close enough that a tile beside one can
/// also sit right next to the other, and a caller counting what landed
/// near each mark could never then tell whose item it actually was.
fn plan_scatter(
    table: &BandedTable<Id<ItemDef>>,
    band: i32,
    layout: &Layout,
    extra: u32,
    walkable: &mut impl FnMut(Point) -> bool,
    rng: &mut impl Rng,
) -> Vec<(Id<ItemDef>, Point)> {
    let Layout { bounds, armories, stores } = *layout;
    let mut placed: Vec<(Id<ItemDef>, Point)> = Vec::new();
    let mut used: Vec<Point> = Vec::new();
    let marks: Vec<Point> = armories.iter().chain(stores).copied().collect();

    for &mark in armories {
        if let Some(id) = table.pick(band, rng).map(|e| e.item) {
            placed.push((id, mark));
            used.push(mark);
        }
    }
    for &mark in stores {
        if let Some(id) = table.pick(band, rng).map(|e| e.item) {
            placed.push((id, mark));
            used.push(mark);
        }
        let beside = geometry::square(mark, 1).find(|&p| {
            p != mark && walkable(p) && !used.contains(&p) && marks.iter().all(|&m| m == mark || geometry::chebyshev(p, m) >= MIN_DISTANCE_FROM_MARKS)
        });
        if let Some(beside) = beside
            && let Some(id) = table.pick(band, rng).map(|e| e.item)
        {
            placed.push((id, beside));
            used.push(beside);
        }
    }

    for _ in 0..extra {
        for _ in 0..40 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            let clear = walkable(p) && !used.contains(&p) && marks.iter().all(|m| geometry::chebyshev(*m, p) >= MIN_DISTANCE_FROM_MARKS);
            if !clear {
                continue;
            }
            if let Some(id) = table.pick(band, rng).map(|e| e.item) {
                placed.push((id, p));
                used.push(p);
            }
            break;
        }
    }
    placed
}

/// What scattering a deck's loot the moment it is entered reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Scatter<'w> {
    map: Res<'w, WorldMap>,
    seed: Res<'w, Seed>,
    registries: Res<'w, Registries>,
}

/// Scatters a deck's loot the moment it is entered, the way `populate_deck`
/// seeds its monsters: one item at every `A` mark, two at every `L`, and
/// `3 + deck` more loose, all planned by [`plan_scatter`] against the
/// deck's real terrain. Draws from `Seed::stream(b"foundry.scatter",
/// deck)`, the game's own stream and never [`Drops`]: a deck's own layout
/// must never depend on how many kills happened to land before it was
/// ever built. A revisit is not a first arrival, so nothing is scattered
/// twice.
pub fn scatter_on_arrival(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, scatter: Scatter) {
    let Scatter { map, seed, registries } = &scatter;
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let armory = Armory::load(registries);
        let deck = deck_of(ev.map);
        let mut rng = seed.stream(b"foundry.scatter", deck as u64);
        let armories: Vec<Point> = place.spots.iter().filter(|s| s.tag == 'A' as u32).map(|s| s.at).collect();
        let stores: Vec<Point> = place.spots.iter().filter(|s| s.tag == 'L' as u32).map(|s| s.at).collect();
        let layout = Layout { bounds: place.terrain.bounds(), armories: &armories, stores: &stores };
        let plan = plan_scatter(&armory.table, deck as i32, &layout, 3 + deck, &mut |p| map.is_walkable(p), &mut rng);
        for (id, p) in plan {
            let item = crate::gear::spawn_item(&mut commands, &armory, id, registries);
            commands.entity(item).insert((Position(p), OnMap(ev.map)));
        }
    }
}

/// Drops whatever the dead actor's kind carries, at the spot it died, on
/// its own map: on each [`DeathEvent`], looks up the dead entity's
/// [`Kind`] in the [`Roster`] (absent for the player, and for anything
/// else with no kind at all) and rolls its `drops` with [`Drops`],
/// [`roll_drops`]'s own stream, never the combat one.
///
/// Reads `Kind` and `OnMap` off the dying entity while both are still
/// there to read: the engine takes a dead non-player out of the turn
/// queue and its own occupancy at once, but leaves every component but
/// `Actor`, `Blocks` and `Position` on it until `bury_the_dead` despawns
/// it at the very end of the frame, which [`TurnSet::React`] runs well
/// before.
pub fn drop_on_death(
    mut commands: Commands,
    mut deaths: MessageReader<DeathEvent>,
    mut drops: ResMut<Drops>,
    registries: Res<Registries>,
    roster: Res<Roster>,
    kinds: Query<(&Kind, &OnMap)>,
) {
    let mut armory: Option<Armory> = None;
    for d in deaths.read() {
        let Ok((kind, on_map)) = kinds.get(d.entity) else { continue };
        let def = roster.defs.get(kind.0);
        if def.drops.is_empty() {
            continue;
        }
        let armory = armory.get_or_insert_with(|| Armory::load(&registries));
        let table: Vec<(Id<ItemDef>, u32)> = def.drops.iter().map(|(name, pct)| (armory.defs.expect(name), *pct)).collect();
        for id in roll_drops(&table, &mut drops.0) {
            let item = crate::gear::spawn_item(&mut commands, armory, id, &registries);
            commands.entity(item).insert((Position(d.at), OnMap(on_map.0)));
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use super::*;

    /// Two armories and two stores on an open floor, far enough apart that
    /// no mark's own radius ever brushes another's. Kept as the raw marks
    /// too, since a property test reads them back out of `plan`'s result.
    fn synthetic_marks() -> (Vec<Point>, Vec<Point>) {
        (vec![Point::new(10, 10), Point::new(60, 40)], vec![Point::new(30, 30), Point::new(50, 10)])
    }

    #[test]
    fn a_ten_percent_drop_lands_near_one_time_in_ten_over_many_deaths() {
        // A property over many rolls rather than one lucky seed: the
        // design's rate, within a margin a correct roll never leaves.
        let armory = crate::gear::Armory::load(&crate::content::registries());
        let slugs = armory.defs.expect("slugs");
        let mut rng = rand::rngs::StdRng::seed_from_u64(4);
        let hits = (0..10_000).filter(|_| !roll_drops(&[(slugs, 10)], &mut rng).is_empty()).count();
        assert!((850..=1150).contains(&hits), "{hits} drops in ten thousand");
    }

    #[test]
    fn drops_never_draw_from_the_combat_stream() {
        // The randomness rule: a game's draws come from its own stream. A
        // kill that rolled loot from the combat stream would shift every
        // later blow in the run.
        let (with_loot, without) = crate::testing::combat_rolls_across_a_kill(RunSeed(8));
        assert_eq!(with_loot, without);
    }

    #[test]
    fn every_armory_on_a_deck_holds_something_and_every_store_holds_two() {
        let mut app = crate::testing::headless(RunSeed(5));
        crate::testing::arrive_on(&mut app, 1);
        let (armories, stores) = crate::testing::items_at_marks(&mut app);
        assert!(armories.iter().all(|n| *n == 1));
        assert!(stores.iter().all(|n| *n == 2));
    }

    #[test]
    fn open_ground_always_scatters_the_full_count_over_a_span_of_seeds() {
        let armory = crate::gear::Armory::load(&crate::content::registries());
        let (armories, stores) = synthetic_marks();
        let layout = Layout { bounds: Rect::new(0, 0, 80, 60), armories: &armories, stores: &stores };
        for s in 0..20 {
            for deck in 1..=3u32 {
                let mut rng = rand::rngs::StdRng::seed_from_u64(s);
                let plan = plan_scatter(&armory.table, deck as i32, &layout, 3 + deck, &mut |_| true, &mut rng);
                let expect = armories.len() + stores.len() * 2 + (3 + deck) as usize;
                assert_eq!(plan.len(), expect, "deck {deck}, seed {s}");
            }
        }
    }

    #[test]
    fn every_armory_mark_gets_one_item_and_every_store_mark_gets_two_over_a_span_of_seeds() {
        let armory = crate::gear::Armory::load(&crate::content::registries());
        let (armories, stores) = synthetic_marks();
        let layout = Layout { bounds: Rect::new(0, 0, 80, 60), armories: &armories, stores: &stores };
        for s in 0..20 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(s);
            let plan = plan_scatter(&armory.table, 2, &layout, 5, &mut |_| true, &mut rng);
            for &mark in &armories {
                assert_eq!(plan.iter().filter(|(_, p)| *p == mark).count(), 1, "seed {s}: {mark:?}");
            }
            for &mark in &stores {
                let near = plan.iter().filter(|(_, p)| geometry::chebyshev(*p, mark) <= 1).count();
                assert_eq!(near, 2, "seed {s}: {mark:?}");
            }
        }
    }

    #[test]
    fn no_two_items_ever_share_a_tile_over_a_span_of_seeds() {
        let armory = crate::gear::Armory::load(&crate::content::registries());
        let (armories, stores) = synthetic_marks();
        let layout = Layout { bounds: Rect::new(0, 0, 80, 60), armories: &armories, stores: &stores };
        for s in 0..20 {
            let mut rng = rand::rngs::StdRng::seed_from_u64(s);
            let plan = plan_scatter(&armory.table, 3, &layout, 8, &mut |_| true, &mut rng);
            let mut seen: Vec<Point> = Vec::new();
            for (_, p) in &plan {
                assert!(!seen.contains(p), "seed {s}: two items on {p:?}");
                seen.push(*p);
            }
        }
    }

    #[test]
    fn the_same_seed_plans_the_same_scatter_every_time_over_a_span_of_seeds() {
        let armory = crate::gear::Armory::load(&crate::content::registries());
        let (armories, stores) = synthetic_marks();
        let layout = Layout { bounds: Rect::new(0, 0, 80, 60), armories: &armories, stores: &stores };
        for s in 0..20 {
            let plan = || {
                let mut rng = rand::rngs::StdRng::seed_from_u64(s);
                plan_scatter(&armory.table, 2, &layout, 6, &mut |_| true, &mut rng)
            };
            assert_eq!(plan(), plan(), "seed {s}: two runs of the same seed disagreed");
        }
    }
}
