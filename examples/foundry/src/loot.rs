//! How much loot a deck gets, and where the engine puts it.
//!
//! The engine's `LootPlugin` owns the loop: it scatters a deck's floor on
//! the arrival that built it, leaves what a kill's kind carries where it
//! died, and fills crates and lockers, each through the
//! [`Armory`](crate::gear::Armory), Foundry's
//! [`ItemMaker`](rl_engine::rl_bevy::ItemMaker).
//! What is Foundry's is the numbers: [`scatter_rules`] puts one item beside
//! every armory mark, two beside every store mark, and `3 + deck` more
//! loose, and `item_spawns.ron` says what each deck may hold.

use rl_engine::rl_rules::ScatterRules;

/// How a deck's floor is laid out when it is first entered: one item
/// beside every `A` mark, where an armory locker stands, two beside every
/// `L`, where a supply crate does, and three more loose, plus one for each
/// deck down.
pub fn scatter_rules() -> ScatterRules {
    ScatterRules::new().at_spot('A' as u32, 1).at_spot('L' as u32, 2).loose(3, 3).per_band(1)
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;
    use rand::SeedableRng;
    use rl_engine::prelude::*;
    use rl_engine::rl_core::{Grid2D, RunSeed};
    use rl_engine::rl_rules::{Layout, plan_scatter};

    use super::*;

    /// Every kind a crate or a locker asks for is found at every deck it
    /// could stand on, offset included, so no container on any deck falls
    /// back to another deck's things or comes up empty.
    #[test]
    fn every_kind_a_container_asks_for_is_found_at_every_deck() {
        let registries = crate::content::registries();
        let armory = crate::testing::armory(&registries);
        let mut asked = 0;
        for (_, prop) in registries.props.iter() {
            for row in prop.container.iter().flat_map(|c| &c.contents) {
                let rl_engine::rl_rules::Stock::Tag(tag) = row.what else { continue };
                asked += 1;
                for deck in 1..=crate::decks::DECKS as i32 {
                    let band = deck + row.band;
                    assert_eq!(armory.table.band_for(tag, band), Some(band), "{} asks for {} at deck {deck}", prop.name, registries.tags.name(tag));
                }
            }
        }
        assert!(asked > 0, "the containers ask for kinds of thing");
    }

    /// The real thing, through the real deck: every armory locker the game
    /// puts down is stocked by the engine with a weapon of its own deck,
    /// so a locker on deck eight never holds deck one's blade and one on
    /// deck one never holds deck eight's rifle.
    #[test]
    fn an_armory_locker_holds_a_weapon_of_the_deck_it_stands_on() {
        for deck in [1u32, 4, 8] {
            let mut app = crate::testing::headless(RunSeed(3));
            crate::testing::arrive_on(&mut app, deck);
            app.update();
            let armory = app.world().resource::<crate::gear::Armory>().clone();
            let weapon = app.world().resource::<Registries>().tags.expect("weapon");
            let map = crate::decks::map_of(deck);
            let lockers: Vec<Vec<Entity>> = {
                let world = app.world_mut();
                let mut q = world.query::<(&Name, &Inventory, &OnMap)>();
                q.iter(world).filter(|(n, _, on)| n.as_str() == "armory locker" && on.0 == map).map(|(_, bag, _)| bag.items.clone()).collect()
            };
            assert!(!lockers.is_empty(), "deck {deck} has a locker");
            for held in lockers {
                let weapons: Vec<String> = held
                    .iter()
                    .filter_map(|i| app.world().get::<crate::gear::ItemKind>(*i))
                    .filter(|k| armory.defs.get(k.0).tags.iter().any(|t| t.id() == weapon))
                    .map(|k| armory.defs.get(k.0).name.clone())
                    .collect();
                assert_eq!(weapons.len(), 1, "deck {deck}: one weapon in a locker, {weapons:?}");
                let id = armory.defs.expect(&weapons[0]);
                assert!(armory.table.rows().iter().any(|r| r.item == id && r.applies(deck as i32)), "deck {deck}: {} is not a deck {deck} weapon", weapons[0]);
            }
        }
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
    fn every_armory_on_a_deck_is_flanked_by_something_and_every_store_by_two() {
        let mut app = crate::testing::headless(RunSeed(5));
        crate::testing::arrive_on(&mut app, 1);
        let (armories, stores) = crate::testing::items_beside_marks(&mut app);
        assert!(!armories.is_empty() && !stores.is_empty(), "deck one has both kinds of mark");
        assert!(armories.iter().all(|n| *n >= 1), "{armories:?}");
        assert!(stores.iter().all(|n| *n >= 2), "{stores:?}");
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

    /// The commando's half of the ranged ramp in `DESIGN.md`: deck one is
    /// walked with bare hands, a blade, or the one gun that runs dry, and
    /// an energy weapon is a deck two find. The weights alone would let a
    /// later band edit put a blaster back on deck one without anything
    /// saying so.
    #[test]
    fn deck_one_lays_out_no_energy_weapon_and_only_the_gun_that_runs_dry_over_a_span_of_seeds() {
        let mut decks_with_a_pistol = 0;
        let seeds = 40u64;
        for s in 0..seeds {
            let mut app = crate::testing::headless(RunSeed(s));
            crate::testing::arrive_on(&mut app, 1);
            let map = app.world().resource::<WorldMap>().current();
            let world = app.world_mut();
            let mut q = world.query_filtered::<(&Name, &OnMap), With<Item>>();
            let names: Vec<String> = q.iter(world).filter(|(_, on)| on.0 == map).map(|(n, _)| n.as_str().to_owned()).collect();
            for gun in ["hand blaster", "blaster carbine", "ion pistol"] {
                assert!(!names.iter().any(|n| n == gun), "seed {s}: a {gun} on deck one");
            }
            if names.iter().any(|n| n == "slug pistol") {
                decks_with_a_pistol += 1;
            }
        }
        // The design's "about two runs in five", at the weights in
        // `item_spawns.ron`. The bounds are loose on purpose, since the
        // rate is a weight to tune and not a promise.
        assert!(decks_with_a_pistol > 0, "the slug pistol is findable on deck one at all");
        assert!(decks_with_a_pistol < seeds, "and not on every deck, or the unarmed opening is one room long");
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
        let at = crate::testing::kill_with_a_guaranteed_drop(&mut app, "slug");
        let map = app.world().resource::<WorldMap>().current();
        let landed = {
            let world = app.world_mut();
            let mut q = world.query_filtered::<(&Position, &OnMap), With<Item>>();
            q.iter(world).any(|(p, on)| p.0 == at && on.0 == map)
        };
        assert!(landed, "no item landed at the death tile {at:?}");
    }

    /// Every real deck's own marks, which sit closer together than any
    /// synthetic layout (`decks::Foundry::stores` puts its two `L`s two
    /// tiles apart), over a span of seeds and with no `App`: every item on
    /// a walkable tile and none sharing one, and each mark's own items,
    /// which the plan lays first and in mark order, beside that mark.
    #[test]
    fn every_real_deck_scatters_walkable_items_beside_their_own_marks_over_a_span_of_seeds() {
        let registries = crate::content::registries();
        let armory = crate::testing::armory(&registries);
        let rules = scatter_rules();
        for s in 0..20 {
            let foundry = crate::decks::Foundry::new(RunSeed(s));
            let tables = foundry.tiles().tables();
            for deck in 1..=crate::decks::DECKS {
                let build = foundry.build(crate::decks::map_of(deck), None).unwrap_or_else(|e| panic!("deck {deck}, seed {s}: {e}"));
                let marks: Vec<(u32, Point)> = build.spots.iter().map(|sp| (sp.tag, sp.at)).collect();
                let on_marks: Vec<Point> = marks.iter().map(|(_, at)| *at).collect();
                // Where a locker or a crate stands, nothing lies.
                let mut free = |p: Point| build.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]) && !on_marks.contains(&p);
                let mut rng = rand::rngs::StdRng::seed_from_u64(s);
                let loose = rules.loose_count(deck as i32, &mut rng);
                let plan =
                    plan_scatter(&armory.table, deck as i32, &rules, Layout { marks: &marks, bounds: build.terrain.bounds() }, loose, &mut free, &mut rng);
                let mut seen: Vec<Point> = Vec::new();
                for p in &plan {
                    assert!(free(p.at), "deck {deck}, seed {s}: {:?} on a wall or a mark", p.at);
                    assert!(!seen.contains(&p.at), "deck {deck}, seed {s}: two items on {:?}", p.at);
                    seen.push(p.at);
                }
                let owed: Vec<Point> = rules
                    .spots
                    .iter()
                    .flat_map(|&(tag, n)| marks.iter().filter(move |(t, _)| *t == tag).flat_map(move |(_, at)| std::iter::repeat_n(*at, n as usize)))
                    .collect();
                assert!(plan.len() >= owed.len(), "deck {deck}, seed {s}: {} placed, {} owed to marks", plan.len(), owed.len());
                for (placed, mark) in plan.iter().zip(&owed) {
                    assert_eq!(
                        rl_engine::rl_core::geometry::chebyshev(placed.at, *mark),
                        1,
                        "deck {deck}, seed {s}: an item owed to {mark:?} lies at {:?}",
                        placed.at
                    );
                }
            }
        }
    }
}
