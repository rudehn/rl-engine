//! Items on the decks, and items the dead leave behind.
//!
//! `plan_scatter` is the pure half, tested without an `App` at all: it
//! places one item at every `A` mark, two at every `L` (the mark itself
//! and a free floor tile beside it), and a handful more on random floor
//! tiles, all drawn from a [`BandedTable`] at a deck's band.
//! [`scatter_on_arrival`] is the one line of Bevy over it, the way
//! `droids::spawns::populate_deck` is over `plan_population`: reads a
//! [`PlaceEntered`] for the first arrival, plans against the real map,
//! and spawns what the plan says.
//!
//! [`roll_drops`] is the other pure half: what a dead monster's drop
//! table pays out. [`drop_on_death`] rolls it from [`Drops`], never the
//! engine's combat stream, so a kill never nudges a later blow.

use bevy::prelude::*;
use rand::Rng;
use rand::rngs::StdRng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;

use crate::decks::deck_of;
use crate::droids::{Kind, Roster};
use crate::gear::{Armory, ItemDef};

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
/// `plan_scatter` takes one fewer argument than clippy's own limit on a
/// function's parameter list starts complaining about.
///
/// `pub(crate)`, with `plan_scatter` and [`Origin`]: `testing::loot`'s
/// `items_at_marks` recomputes the exact same plan `scatter_on_arrival`
/// already made, to read off each item's origin rather than guess one
/// from where it landed.
#[derive(Clone, Copy)]
pub(crate) struct Layout<'a> {
    /// The floor's own bounds, for a random loose item's anchor.
    pub(crate) bounds: Rect,
    /// Where an `A` mark guarantees exactly one item.
    pub(crate) armories: &'a [Point],
    /// Where an `L` mark guarantees two: the mark itself, and a free
    /// floor tile beside it.
    pub(crate) stores: &'a [Point],
}

/// How many loose items a deck's own scatter adds, on top of what its
/// marks guarantee. Named once so [`scatter_on_arrival`] and a test never
/// drift apart on the design's `3 + deck`.
pub(crate) fn extra_loose_items(deck: u32) -> u32 {
    3 + deck
}

/// Where in the plan one placed item came from, so a test can attribute
/// each one to the mark that earned it instead of guessing from where it
/// landed: two marks close enough together that a free tile beside one
/// also sits within a step of the other would make a guess by proximity
/// wrong, and a real store's own two `L`s can sit exactly that close
/// (`decks::Foundry::stores`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    /// The guaranteed item at the `i`th of `layout.armories`.
    Armory(usize),
    /// One of the two guaranteed items at the `i`th of `layout.stores`:
    /// the mark itself and the free tile beside it both count here.
    Store(usize),
    /// One of the `extra` loose items, tied to no particular mark.
    Loose,
}

/// Plans a deck's whole scatter the moment it is entered: one item at
/// each of `layout.armories`, two at each of `layout.stores` (the mark
/// itself, and a free floor tile beside it, [`geometry::square`] radius
/// one), and `extra` more on random floor tiles, none of them sharing a
/// tile with another. Every item is drawn from `table` at `band`. A
/// random slot gives up after forty draws, so a deck with nowhere left to
/// stand one stops trying rather than spinning, the same way
/// `droids::spawns::plan_population` does; a mark the table cannot fill
/// is simply left bare rather than panicking.
pub(crate) fn plan_scatter(
    table: &BandedTable<Id<ItemDef>>,
    band: i32,
    layout: &Layout,
    extra: u32,
    walkable: &mut impl FnMut(Point) -> bool,
    rng: &mut impl Rng,
) -> Vec<(Id<ItemDef>, Point, Origin)> {
    let Layout { bounds, armories, stores } = *layout;
    let mut placed: Vec<(Id<ItemDef>, Point, Origin)> = Vec::new();
    let mut used: Vec<Point> = Vec::new();

    for (i, &mark) in armories.iter().enumerate() {
        if let Some(id) = table.pick(band, rng).map(|e| e.item) {
            placed.push((id, mark, Origin::Armory(i)));
            used.push(mark);
        }
    }
    for (i, &mark) in stores.iter().enumerate() {
        if let Some(id) = table.pick(band, rng).map(|e| e.item) {
            placed.push((id, mark, Origin::Store(i)));
            used.push(mark);
        }
        let beside = geometry::square(mark, 1).find(|&p| p != mark && walkable(p) && !used.contains(&p));
        if let Some(beside) = beside
            && let Some(id) = table.pick(band, rng).map(|e| e.item)
        {
            placed.push((id, beside, Origin::Store(i)));
            used.push(beside);
        }
    }

    for _ in 0..extra {
        for _ in 0..40 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            if !walkable(p) || used.contains(&p) {
                continue;
            }
            if let Some(id) = table.pick(band, rng).map(|e| e.item) {
                placed.push((id, p, Origin::Loose));
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
/// `3 + deck` more loose, all planned by `plan_scatter` against the
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
        let plan = plan_scatter(&armory.table, deck as i32, &layout, extra_loose_items(deck), &mut |p| map.is_walkable(p), &mut rng);
        for (id, p, _origin) in plan {
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
    /// no mark's own radius ever brushes another's.
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
    fn revisiting_a_deck_scatters_nothing_new() {
        let mut app = crate::testing::headless(RunSeed(4));
        crate::testing::arrive_on(&mut app, 1);
        let count = |app: &mut App| -> usize {
            let map = crate::decks::map_of(1);
            let world = app.world_mut();
            let mut q = world.query_filtered::<&OnMap, With<Item>>();
            q.iter(world).filter(|on| on.0 == map).count()
        };
        let first = count(&mut app);
        assert!(first > 0, "deck one starts with something to find");
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        app.world_mut().write_message(WarpRequest::into_place(player, crate::decks::map_of(2)));
        app.update();
        app.update();
        app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: crate::decks::map_of(1), arrive: Arrive::Entry } });
        app.update();
        app.update();
        assert_eq!(count(&mut app), first, "a second arrival on deck one scattered nothing new");
    }

    #[test]
    fn every_scattered_item_on_a_live_deck_is_walkable_over_a_span_of_seeds() {
        for s in 0..20u64 {
            let mut app = crate::testing::headless(RunSeed(s));
            crate::testing::arrive_on(&mut app, 1);
            let map = app.world().resource::<WorldMap>().current();
            let positions: Vec<Point> = {
                let world = app.world_mut();
                let mut q = world.query_filtered::<(&Position, &OnMap), With<Item>>();
                q.iter(world).filter(|(_, on)| on.0 == map).map(|(p, _)| p.0).collect()
            };
            assert!(!positions.is_empty(), "seed {s}: nothing scattered");
            let wm = app.world().resource::<WorldMap>();
            for p in &positions {
                assert!(wm.is_walkable(*p), "seed {s}: {p:?} on a wall");
            }
        }
    }

    #[test]
    fn a_real_kill_drops_its_guaranteed_item_at_the_death_tile() {
        let mut app = crate::testing::headless(RunSeed(2));
        let at = crate::testing::kill_with_a_guaranteed_drop(&mut app, "slugs");
        let map = app.world().resource::<WorldMap>().current();
        let landed = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<(&Position, &OnMap), With<Item>>();
            q.iter(world).any(|(p, on)| p.0 == at && on.0 == map)
        };
        assert!(landed, "no item landed at the death tile {at:?}");
    }

    /// Covers the full count, one item per armory and two per store by
    /// origin, and no two items ever sharing a tile, in one pass over the
    /// same plans: all three properties of the same generated data,
    /// rather than three separate plans that could drift apart.
    #[test]
    fn open_ground_satisfies_every_mark_by_origin_with_no_shared_tile_over_a_span_of_seeds() {
        let armory = crate::gear::Armory::load(&crate::content::registries());
        let (armories, stores) = synthetic_marks();
        let layout = Layout { bounds: Rect::new(0, 0, 80, 60), armories: &armories, stores: &stores };
        for s in 0..20 {
            for deck in 1..=3u32 {
                let mut rng = rand::rngs::StdRng::seed_from_u64(s);
                let plan = plan_scatter(&armory.table, deck as i32, &layout, extra_loose_items(deck), &mut |_| true, &mut rng);
                let expect = armories.len() + stores.len() * 2 + extra_loose_items(deck) as usize;
                assert_eq!(plan.len(), expect, "deck {deck}, seed {s}");
                for i in 0..armories.len() {
                    let n = plan.iter().filter(|(_, _, o)| *o == Origin::Armory(i)).count();
                    assert_eq!(n, 1, "deck {deck}, seed {s}: armory {i}");
                }
                for i in 0..stores.len() {
                    let n = plan.iter().filter(|(_, _, o)| *o == Origin::Store(i)).count();
                    assert_eq!(n, 2, "deck {deck}, seed {s}: store {i}");
                }
                let mut seen: Vec<Point> = Vec::new();
                for (_, p, _) in &plan {
                    assert!(!seen.contains(p), "deck {deck}, seed {s}: two items on {p:?}");
                    seen.push(*p);
                }
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

    /// The test above uses marks spread far apart on open ground; a real
    /// deck's own tighter layout (`decks::Foundry::stores` places its two
    /// `L`s only two Chebyshev tiles apart) needs its own real terrain
    /// and walkability to exercise, no `App` involved.
    #[test]
    fn every_real_deck_scatters_walkable_items_by_origin_over_a_span_of_seeds() {
        let armory = crate::gear::Armory::load(&crate::content::registries());
        for s in 0..20 {
            let foundry = crate::decks::Foundry::new(RunSeed(s));
            let tables = foundry.tiles().tables();
            for deck in 1..=crate::decks::DECKS {
                let build = foundry.build(crate::decks::map_of(deck), None).unwrap_or_else(|e| panic!("deck {deck}, seed {s}: {e}"));
                let armories: Vec<Point> = build.spots.iter().filter(|sp| sp.tag == 'A' as u32).map(|sp| sp.at).collect();
                let stores: Vec<Point> = build.spots.iter().filter(|sp| sp.tag == 'L' as u32).map(|sp| sp.at).collect();
                let layout = Layout { bounds: build.terrain.bounds(), armories: &armories, stores: &stores };
                let mut walkable = |p: Point| build.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]);
                let mut rng = rand::rngs::StdRng::seed_from_u64(s);
                let plan = plan_scatter(&armory.table, deck as i32, &layout, extra_loose_items(deck), &mut walkable, &mut rng);
                let mut seen: Vec<Point> = Vec::new();
                for (_, p, _) in &plan {
                    assert!(walkable(*p), "deck {deck}, seed {s}: {p:?} on a wall");
                    assert!(!seen.contains(p), "deck {deck}, seed {s}: two items on {p:?}");
                    seen.push(*p);
                }
                for i in 0..armories.len() {
                    let n = plan.iter().filter(|(_, _, o)| *o == Origin::Armory(i)).count();
                    assert_eq!(n, 1, "deck {deck}, seed {s}: armory {i}");
                }
                for i in 0..stores.len() {
                    let n = plan.iter().filter(|(_, _, o)| *o == Origin::Store(i)).count();
                    assert_eq!(n, 2, "deck {deck}, seed {s}: store {i}");
                }
            }
        }
    }
}
