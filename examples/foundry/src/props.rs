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
use rl_engine::rl_core::{Direction, Point, geometry};
use std::collections::BTreeSet;

use crate::decks::deck_of;
use crate::gear::spawn_item;

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

/// Whether a prop that blocks may stand on `p`: floor, and not a
/// doorway.
///
/// A crate in a hatch is a room sealed shut, and everything in it lost
/// for the run. A hatch is walkable floor here rather than a door the
/// engine opens, so nothing else would have stopped it. This is the cheap
/// half of the question; [`keeps_the_way_open`] is the half that is
/// actually true.
fn room_floor(map: &WorldMap, p: Point) -> bool {
    map.is_walkable(p) && !is_hatch(map, p)
}

/// Every cell a walker can reach from `from` with `blocked` standing in
/// the way, by the rule the move resolver walks by: eight ways, and never
/// a diagonal that cuts a corner.
///
/// A `BTreeSet`, as everything that walks a map here is.
fn reachable(map: &WorldMap, from: Point, blocked: &[Point]) -> BTreeSet<Point> {
    let mut seen = BTreeSet::new();
    let mut queue = vec![from];
    while let Some(p) = queue.pop() {
        if !seen.insert(p) {
            continue;
        }
        for d in Direction::ALL {
            let (dx, dy) = d.delta();
            let next = p + d.offset();
            if !map.is_walkable(next) || blocked.contains(&next) || seen.contains(&next) {
                continue;
            }
            // The corner rule, which is what makes a crate in a doorway a
            // wall: a walker may not squeeze past it diagonally when the
            // cells either side of the corner are not both floor.
            if dx != 0 && dy != 0 && !(map.is_walkable(p.offset(dx, 0)) && map.is_walkable(p.offset(0, dy))) {
                continue;
            }
            queue.push(next);
        }
    }
    seen
}

/// Whether something that blocks may stand at `at` without shutting
/// anything away.
///
/// The honest question, asked honestly: flood the deck as a walker walks
/// it, with what already stands in the way, and again with this as well.
/// If anything the walker could reach before it cannot reach after, the
/// crate goes somewhere else.
///
/// A count of walkable neighbours was the first attempt and it was not
/// enough: the cell below a two-wide store has three floor neighbours and
/// is still the one way in, because the diagonals either side of it cut a
/// corner the move resolver refuses.
fn keeps_the_way_open(map: &WorldMap, from: Point, blocked: &[Point], at: Point) -> bool {
    let before = reachable(map, from, blocked);
    if !before.contains(&at) {
        // Nothing can get there anyway, so nothing is shut away by it.
        return true;
    }
    let mut with = blocked.to_vec();
    with.push(at);
    let after = reachable(map, from, &with);
    before.iter().all(|p| *p == at || after.contains(p))
}

/// Whether `p` is one of the deck's hatches: the doorways, which are
/// walkable and opaque, named in `tiles.ron`.
fn is_hatch(map: &WorldMap, p: Point) -> bool {
    map.tile(p).and_then(|t| map.tables().names.get(t.index())).is_some_and(|name| name == "hatch")
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
        // rest of the crates wherever there is room floor to stand them
        // on. A mark that sits in a doorway is stood beside instead: the
        // builder put it where the room's contents go, not where a crate
        // may block the way in.
        // What already stands in the way, so each crate is judged against
        // the deck as the ones before it left it.
        let mut taken: Vec<Point> = Vec::new();
        let free_to_block = |p: Point, taken: &Vec<Point>| room_floor(&map, p) && !taken.contains(&p) && keeps_the_way_open(&map, ev.entry, taken, p);
        let stand = |commands: &mut Commands, kind, at: Point, taken: &mut Vec<Point>| {
            let spot = [at].into_iter().chain(geometry::square(at, 1)).find(|p| free_to_block(*p, taken));
            if let Some(spot) = spot {
                taken.push(spot);
                spawn_prop(commands, &registries, kind, spot, ev.map);
            }
        };
        for spot in place.spots.iter().filter(|s| s.tag == 'A' as u32) {
            stand(&mut commands, locker, spot.at, &mut taken);
        }
        let stores: Vec<Point> = place.spots.iter().filter(|s| s.tag == 'L' as u32).map(|s| s.at).collect();
        for at in &stores {
            stand(&mut commands, supply, *at, &mut taken);
        }
        let bounds = place.terrain.bounds();
        // A blocking prop must leave the deck whole; loose cable lies flat
        // and may go anywhere walkable, doorway included, since nothing is
        // shut by something you can walk over.
        let free = |rng: &mut dyn FnMut() -> Point, taken: &mut Vec<Point>, blocks: bool| -> Option<Point> {
            for _ in 0..64 {
                let p = rng();
                let ok = if blocks { free_to_block(p, taken) } else { map.is_walkable(p) && !taken.contains(&p) };
                if ok {
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
            if let Some(at) = free(&mut roll, &mut taken, true) {
                spawn_prop(&mut commands, &registries, supply, at, ev.map);
            }
        }
        for _ in 0..cables_for(deck) {
            if let Some(at) = free(&mut roll, &mut taken, false) {
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
    content: crate::gear::Content,
    mut bags: Query<&mut Inventory, With<Container>>,
) {
    let (armory, registries) = (content.armory(), content.registries());
    for ask in asks.read() {
        let Some(id) = armory.defs.id(&ask.item) else {
            warn!("props.ron asks for {:?}, which the armory has no definition for", ask.item);
            continue;
        };
        let mut items = Vec::new();
        for _ in 0..ask.count {
            items.push(spawn_item(&mut commands, &armory, id, registries));
        }
        if let Ok(mut bag) = bags.get_mut(ask.prop) {
            bag.items.extend(items);
        }
    }
}

/// Takes the keycard that opened a locker.
///
/// The engine decides whether a locked thing may be opened, and the
/// definition says what it wants; what happens to the key afterwards is
/// nobody's business but the game's. A card is spent opening one locker,
/// so a card found is a locker opened and no more, and a commando with
/// two lockers in sight has a choice to make.
///
/// In `TurnSet::React`, after the interaction the engine resolved.
pub fn spend_the_keycard(
    mut commands: Commands,
    mut done: MessageReader<Interacted>,
    mut tell: MessageWriter<Tell>,
    registries: Res<Registries>,
    props: Query<&PropKind>,
    mut bags: Query<&mut Inventory, With<Player>>,
    cards: Query<(&Tagged, Option<&Stack>)>,
) {
    let Some(keycard) = registries.tags.id("keycard") else { return };
    for ev in done.read() {
        if ev.verb != Verbs::OPEN {
            continue;
        }
        // Only a locker: a crate wants nothing and takes nothing.
        let Ok(kind) = props.get(ev.prop) else { continue };
        if registries.props.get(kind.0).container.as_ref().and_then(|c| c.locked) != Some(keycard) {
            continue;
        }
        let Ok(mut bag) = bags.get_mut(ev.actor) else { continue };
        let Some(card) = bag.items.iter().copied().find(|i| cards.get(*i).is_ok_and(|(t, _)| t.0.contains(&keycard))) else { continue };
        // One off the stack, or the card itself when it is the last.
        match cards.get(card).ok().and_then(|(_, stack)| stack).map(|s| s.count) {
            Some(count) if count > 1 => {
                commands.entity(card).entry::<Stack>().and_modify(|mut s| s.count -= 1);
            }
            _ => {
                bag.remove(card);
                commands.entity(card).despawn();
            }
        }
        tell.write(Tell::new("The keycard is spent in the lock.", Tones::MUTED));
    }
}

// ANCHOR: wreck
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
// ANCHOR_END: wreck

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_core::RunSeed;

    /// A crate in a doorway is a room sealed shut and everything in it
    /// lost for the run. Over a span of seeds and every deck: what the
    /// commando could reach before the props were put down, it can reach
    /// after.
    #[test]
    fn a_prop_that_blocks_never_seals_off_anything_the_deck_could_reach_over_a_span_of_seeds() {
        for seed in 1..=6u64 {
            let mut app = crate::testing::headless(RunSeed(seed));
            for deck in 1..=3 {
                crate::testing::arrive_on(&mut app, deck);
                let here = crate::decks::map_of(deck);
                let blockers: Vec<Point> = {
                    let world = app.world_mut();
                    let mut q = world.query_filtered::<(&Position, Option<&OnMap>), (With<Prop>, With<Blocks>)>();
                    q.iter(world).filter(|(_, on)| on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here).map(|(at, _)| at.0).collect()
                };
                let from = {
                    let world = app.world_mut();
                    let mut q = world.query_filtered::<&Position, With<Player>>();
                    q.single(world).expect("the commando stands somewhere").0
                };
                let map = app.world().resource::<WorldMap>();
                // The same flood the game places by, which is the one the
                // move resolver walks by: a diagonal that cuts a corner is
                // no way past a crate.
                let open = reachable(map, from, &[]);
                let with_props = reachable(map, from, &blockers);
                let lost: Vec<Point> = open.difference(&with_props).filter(|p| !blockers.contains(p)).copied().collect();
                assert!(lost.is_empty(), "seed {seed}, deck {deck}: props cut off {} cells, first {:?}", lost.len(), lost.first());
            }
        }
    }

    /// A live cable is electricity, and plate is no insulation: a commando
    /// in more armor than the cable's whole roll is hurt by it all the
    /// same. Only a resist, which the design keeps for an insulated suit,
    /// takes any of it off.
    #[test]
    fn a_live_cable_shocks_a_commando_through_any_amount_of_plate() {
        let mut app = crate::testing::headless(RunSeed(11));
        crate::testing::arrive_on(&mut app, 1);
        let registries = app.world().resource::<Registries>().clone();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let others: Vec<Entity> = app.world_mut().query_filtered::<Entity, (With<Actor>, Without<Player>)>().iter(app.world()).collect();
        for other in others {
            app.world_mut().entity_mut(other).despawn();
        }
        let at = app.world().get::<Position>(player).expect("the commando stands somewhere").0;
        let floor = app.world().resource::<WorldMap>().tile(at).expect("the commando's own tile is loaded");
        app.world_mut().resource_mut::<WorldMap>().set_tile(at.offset(1, 0), floor);
        let cable = registries.props.expect("live cable");
        let map = app.world().resource::<WorldMap>().current();
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        spawn_prop(&mut commands, &registries, cable, at.offset(1, 0), map);
        queue.apply(app.world_mut());
        // More than the cable's `1d4` could ever get through.
        app.world_mut().entity_mut(player).insert(Armor(10));
        let before = app.world().get::<Health>(player).unwrap().current;

        app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        crate::testing::settle(&mut app);

        assert_eq!(app.world().get::<Position>(player).unwrap().0, at.offset(1, 0), "stepped onto the cable");
        assert!(app.world().get::<Health>(player).unwrap().current < before, "and was shocked through ten points of plate");
    }

    /// A keycard opens one locker and is gone: the engine decides the
    /// locker may be opened, and Foundry takes the card for it.
    #[test]
    fn a_keycard_opens_one_locker_and_is_spent_doing_it() {
        let mut app = crate::testing::headless(RunSeed(11));
        crate::testing::arrive_on(&mut app, 1);
        let registries = app.world().resource::<Registries>().clone();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let at = app.world().get::<Position>(player).expect("the commando stands somewhere").0;

        // A locker beside the commando, and two cards in the bag.
        let locker = {
            let id = registries.props.expect("armory locker");
            let here = app.world().resource::<WorldMap>().current();
            let mut commands = app.world_mut().commands();
            spawn_prop(&mut commands, &registries, id, at.offset(1, 0), here)
        };
        app.world_mut().flush();
        let armory = crate::testing::armory_of(&app);
        let card = {
            let id = armory.defs.id("keycard").expect("the armory has keycards");
            let mut commands = app.world_mut().commands();
            let card = spawn_item(&mut commands, &armory, id, &registries);
            commands.entity(card).insert(Stack { key: 99, count: 2 });
            card
        };
        app.world_mut().flush();
        app.world_mut().entity_mut(player).insert(Inventory { items: vec![card] });
        app.update();

        let offer = {
            let verbs = app.world().resource::<Verbs>();
            let _ = verbs;
            app.world().resource::<OfferedHere>().find(player, locker, Verbs::OPEN).copied()
        };
        let offer = offer.expect("standing beside it, the locker offers to open");
        assert!(offer.refused.is_none(), "with a card in the bag, nothing refuses it");
        app.world_mut().write_message(Intent::new(player, Interact { prop: locker, verb: Verbs::OPEN }));
        crate::testing::settle(&mut app);

        let left = app.world().get::<Stack>(card).map(|s| s.count);
        assert_eq!(left, Some(1), "one card off the stack, spent in the lock");
    }

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
