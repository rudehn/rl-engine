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
    use rl_engine::prelude::*;
    use rl_engine::rl_core::RunSeed;

    /// What the decks hand out gets better the deeper it is, read from the
    /// level rows with no draw: deck one is plain but for a `+1` about one
    /// find in ten, deck nine is never plain, and `+5` waits for the last
    /// row, which a crate past the last deck reads too. The weighted draw
    /// itself is `LevelTable::roll`'s, tested where it is written.
    #[test]
    fn enchant_levels_climb_from_a_rare_plus_one_on_deck_one_to_plus_five_on_the_last() {
        let registries = crate::content::registries();
        let armory = crate::testing::armory(&registries);
        let levels = |band: i32| -> Vec<(i32, u32)> { armory.levels.row_for(band).expect("a row for every band").levels.clone() };
        let share = |band: i32, level: i32| -> f64 {
            let row = levels(band);
            let total: u32 = row.iter().map(|(_, w)| w).sum();
            f64::from(row.iter().filter(|(l, _)| *l == level).map(|(_, w)| w).sum::<u32>()) / f64::from(total)
        };
        assert!(levels(1).iter().all(|(l, _)| (0..=1).contains(l)), "deck one is plain or +1");
        assert!((0.05..=0.16).contains(&share(1, 1)), "and +1 is an uncommon surprise, about one in ten: {}", share(1, 1));
        assert!(levels(9).iter().all(|(l, _)| (1..=4).contains(l)), "deck nine is never plain, and +5 waits for the last");
        assert!(levels(10).iter().any(|(l, _)| *l == 5) && levels(10).iter().all(|(l, _)| (2..=5).contains(l)), "the last deck reaches +5");
        assert_eq!(levels(40), levels(10), "a crate past the last deck reads the last row");
    }

    /// A container whose kind the item table does not have at some deck it
    /// could stand on is refused naming the container, the kind and the
    /// decks, so no crate or locker on any deck falls back to another
    /// deck's things or comes up empty; the shipped table, which the
    /// armory's load holds to it, has every one.
    #[test]
    fn a_container_asking_for_a_kind_the_table_lacks_on_some_deck_is_refused_naming_it() {
        let registries = crate::content::registries();
        let armory = crate::testing::armory(&registries);
        assert_eq!(crate::gear::container_problems(&armory.table, &registries), Vec::<String>::new(), "the shipped table");
        let empty = rl_engine::rl_rules::LootTable::default();
        let wrong = crate::gear::container_problems(&empty, &registries);
        assert!(wrong.iter().any(|w| w.starts_with("armory locker asks for weapon") && w.ends_with("decks [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]")), "{wrong:#?}");
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
        app.world_mut().write_message(WarpRequest {
            actor: player,
            to: Destination::Place { map: crate::decks::map_of(1), arrive: Arrive::Entry },
            after: None,
        });
        app.update();
        app.update();
        assert_eq!(count(&mut app), first, "a second arrival on deck one scattered nothing new");
    }

    /// The commando's half of the ranged ramp in `DESIGN.md`: deck one is
    /// walked with bare hands, a blade, or the one gun that runs dry, and
    /// an energy weapon is a deck two find, read from the tables with no
    /// deck built: nothing deck one
    /// scatters or stocks a container with, at the offset each container
    /// row asks, is an energy weapon, and the slug pistol, the gun that
    /// runs dry, is among what it may scatter. How often a run finds the
    /// pistol there is its weight's to tune, not a promise to sample.
    #[test]
    fn deck_one_lays_out_no_energy_weapon_and_may_lay_the_gun_that_runs_dry() {
        let registries = crate::content::registries();
        let armory = crate::testing::armory(&registries);
        let mut bands = vec![1];
        for (_, prop) in registries.props.iter() {
            bands.extend(prop.container.iter().flat_map(|c| &c.contents).map(|row| 1 + row.band));
        }
        for band in bands {
            let found: Vec<&str> =
                armory.table.rows().iter().filter(|r| r.weight > 0 && r.applies(band)).map(|r| armory.defs.get(r.item).name.as_str()).collect();
            for gun in ["hand blaster", "blaster carbine", "ion pistol"] {
                assert!(!found.contains(&gun), "a {gun} found at band {band}, which deck one's floor or a container on it draws at");
            }
        }
        let first: Vec<&str> = armory.table.rows().iter().filter(|r| r.weight > 0 && r.applies(1)).map(|r| armory.defs.get(r.item).name.as_str()).collect();
        assert!(first.contains(&"slug pistol"), "the slug pistol is findable on deck one at all: {first:?}");
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
}
