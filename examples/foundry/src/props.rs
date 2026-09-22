//! What stands on the decks: the crates, the consoles, the loose cable,
//! and the wreck a droid leaves.
//!
//! All of it is the engine's props, so this module is short on purpose:
//! [`load`] reads `assets/props.ron`, [`place_on_arrival`] says where one
//! stands, [`fill_containers`] answers the engine's ask for what goes
//! inside, and [`wreck_the_dead`] makes a droid's remains a kind of prop
//! so it can be gone through. Nothing here knows what opening a crate
//! does, because the engine does.
//!
//! The one prop Foundry answers itself is the reactor console, in
//! `mission`, because reporting a fact the quest tracker counts is the
//! one thing no effect can say.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::Point;

use crate::decks::deck_of;
use crate::gear::{Armory, spawn_item};

const PROPS_RON: &str = include_str!("../assets/props.ron");

/// Loads `props.ron` against `registries`, which must already hold the
/// tags a locked thing names. Panics listing every problem, since a
/// broken content file is a game that cannot start.
pub fn load(registries: &Registries) -> Registry<PropDef> {
    rl_engine::rl_rules::prop::load(PROPS_RON, &registries.names()).expect("props.ron")
}

/// How many supply crates a deck gets: one per store, and one more the
/// deeper it goes, since a deeper deck is a longer walk from the last
/// crate.
fn crates_for(deck: u32, stores: usize) -> usize {
    stores + deck as usize
}

/// How many lengths of loose cable a deck gets. None on deck one: the
/// first deck teaches the lamp and the droids, and a hidden thing that
/// hurts belongs after that.
fn cables_for(deck: u32) -> usize {
    deck.saturating_sub(1) as usize * 2
}

/// Puts Foundry's props on a deck the first time it is entered, beside
/// the loot and the droids: crates in the stores, cable on open floor.
///
/// Reads the same [`PlaceEntered`] as `loot::scatter_on_arrival` and
/// `droids::populate_deck`, and runs after both, in the chain `plugin`
/// builds a deck in: three systems that all spawn, left unordered, hand
/// their commands in whatever order they finish in, and the fingerprint
/// tripwire reads a run by spawn order.
pub fn place_on_arrival(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, map: Res<WorldMap>, seed: Res<Seed>, registries: Res<Registries>) {
    let (Some(supply), Some(locker), Some(cable)) =
        (registries.props.id("supply crate"), registries.props.id("armory locker"), registries.props.id("live cable"))
    else {
        return;
    };
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let deck = deck_of(ev.map);
        let mut rng = seed.stream(b"foundry.props", deck as u64);
        // A locker at every armory mark, a crate in every store, and the
        // rest of the crates wherever there is floor to stand them on.
        for spot in place.spots.iter().filter(|s| s.tag == 'A' as u32) {
            spawn_prop(&mut commands, &registries, locker, spot.at, ev.map);
        }
        let stores: Vec<Point> = place.spots.iter().filter(|s| s.tag == 'L' as u32).map(|s| s.at).collect();
        let mut taken: Vec<Point> = stores.clone();
        for at in &stores {
            spawn_prop(&mut commands, &registries, supply, *at, ev.map);
        }
        let bounds = place.terrain.bounds();
        let free = |rng: &mut dyn FnMut() -> Point, taken: &mut Vec<Point>| -> Option<Point> {
            for _ in 0..64 {
                let p = rng();
                if map.is_walkable(p) && !taken.contains(&p) {
                    taken.push(p);
                    return Some(p);
                }
            }
            None
        };
        let mut roll = || {
            use rand::Rng;
            Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()))
        };
        for _ in stores.len()..crates_for(deck, stores.len()) {
            if let Some(at) = free(&mut roll, &mut taken) {
                spawn_prop(&mut commands, &registries, supply, at, ev.map);
            }
        }
        for _ in 0..cables_for(deck) {
            if let Some(at) = free(&mut roll, &mut taken) {
                spawn_prop(&mut commands, &registries, cable, at, ev.map);
            }
        }
    }
}

/// Puts into each container what the engine asked for.
///
/// The engine rolled how many of what, from its own stream, and asks by
/// name; spawning is Foundry's, because only Foundry has an armory. This
/// is the whole of the seam.
pub fn fill_containers(
    mut commands: Commands,
    mut asks: MessageReader<FillContainer>,
    registries: Res<Registries>,
    mut bags: Query<&mut Inventory, With<Container>>,
) {
    let armory = Armory::load(&registries);
    for ask in asks.read() {
        let Some(id) = armory.defs.id(&ask.item) else {
            warn!("props.ron asks for {:?}, which the armory has no definition for", ask.item);
            continue;
        };
        let mut items = Vec::new();
        for _ in 0..ask.count {
            items.push(spawn_item(&mut commands, &armory, id, &registries));
        }
        if let Ok(mut bag) = bags.get_mut(ask.prop) {
            bag.items.extend(items);
        }
    }
}

/// Makes a droid's remains a wreck: something to go through.
///
/// The engine kept the dead droid and named it from the remains
/// template, so it is already a prop lying where it fell. What it cannot
/// know is what a Foundry wreck looks like or that it is worth opening,
/// which is one kind in `props.ron` and one component here.
pub fn wreck_the_dead(mut commands: Commands, mut left: MessageReader<RemainsLeft>, registries: Res<Registries>) {
    let Some(id) = registries.props.id("wreckage") else { return };
    for ev in left.read() {
        // The glyph comes off with it, so the renderer dresses the wreck
        // from `props.ron` rather than leaving it drawn as the droid that
        // walked: a `%` on the deck reads as something broken.
        commands.entity(ev.entity).remove::<Glyph>().insert(PropKind(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deck gets more crates the deeper it is, and cable only below the
    /// first: the numbers are a judgement, and this is where the judgement
    /// is written down rather than buried in a loop.
    #[test]
    fn a_deeper_deck_gets_more_crates_and_the_first_deck_no_cable() {
        assert_eq!(crates_for(1, 1), 2, "deck one: the store's crate and one more");
        assert_eq!(crates_for(3, 1), 4, "deck three: two more than that");
        assert_eq!(cables_for(1), 0, "nothing hidden and harmful on the first deck");
        assert!(cables_for(3) > cables_for(2), "and more of it the deeper it goes");
    }
}
