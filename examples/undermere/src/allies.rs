//! Who fights beside the adventurer: a foe a wand of domination bound to
//! them, the second a wand of plenty made of one, and the blades a staff of
//! conjuration calls up for a few turns.
//!
//! An ally is a monster on the adventurer's side of the faction matrix,
//! which every mind and every blow reads live, so turning one is changing
//! its [`Faction`] and nothing else of the rules; it thinks with its kind's
//! [`ally_mind`](crate::bestiary::MonsterDef::ally_mind), which keeps
//! station on the adventurer and fights what comes near. [`Ally`] marks it
//! so, for the save to keep and for whatever tells a companion from a foe.
//! A conjured thing [`Fades`] when its time is up, by [`fade`], and goes
//! as [`vanish`] takes it: out of play without a death, since nothing
//! killed it.

use atglyph::prelude::*;
use bevy::prelude::*;

use crate::bestiary::{Bestiary, Kind};

/// A monster fighting for the adventurer, for good: dominated, conjured,
/// or made of one by a wand of plenty. Not what [`fight_for`] puts on one
/// conjured to fight for a side that is not the adventurer's, a goblin
/// conjurer's own blades: the save keeps this one, by it, as whom the
/// adventurer fights beside, and a goblin's blades are no such thing.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ally;

/// A conjured thing, and the moment on the turn clock it fades at.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fades {
    /// Hundredths of a step on the turn clock.
    pub at: u32,
}

/// Puts `monster` on `side` for good: `side`'s own ally mind of its kind,
/// so it keeps station on `side`'s own and fights what comes near rather
/// than wandering or fetching. [`Ally`] goes with it only when `side` is
/// the adventurer's, which is what the save reads and what tells a
/// companion from a foe; a goblin conjurer's blades fight for the goblins
/// and are no companion of the adventurer's, however this puts them there.
/// What either had noticed of whoever it used to hunt is forgotten, since
/// it hunts for `side` now. Nothing for an entity that is no monster of
/// the bestiary's.
pub fn fight_for(world: &mut World, monster: Entity, side: FactionId) {
    let Some(kind) = world.get::<Kind>(monster).map(|k| k.0) else { return };
    let mind = world.resource::<Bestiary>().defs.get(kind).ally_mind();
    let adventurer = world.resource::<Registries>().factions.expect("adventurer");
    let mut e = world.entity_mut(monster);
    e.insert((Faction(side), mind));
    if side == adventurer {
        e.insert(Ally);
    }
    if let Some(mut aware) = e.get_mut::<Aware>() {
        aware.0.clear();
    }
}

/// Puts `monster` on the adventurer's side for good, as [`fight_for`]
/// says: a wand of domination's foe, a wand of plenty's second, a save's
/// own restored companion. [`fight_for`] itself is for a conjurer whose
/// own side is not necessarily the adventurer's.
pub fn befriend(world: &mut World, monster: Entity) {
    let adventurer = world.resource::<Registries>().factions.expect("adventurer");
    fight_for(world, monster, adventurer);
}

/// Takes `actor` out of play without a death, as the engine takes out the
/// dead: off the index of who stands where and out of the turn queue now,
/// and despawned at the end of the frame with them, so nothing is dropped,
/// tallied or said for it. Whatever it carried is left lying where it
/// stood, so a goblin turned into something else does not take a sword
/// with it into nothing.
pub fn vanish(world: &mut World, actor: Entity) {
    let Some(at) = world.get::<Position>(actor).map(|p| p.0) else { return };
    let map = world.get::<OnMap>(actor).map_or(MapId::ZERO, |m| m.0);
    if world.get::<Blocks>(actor).is_some() {
        world.resource_mut::<Occupancy>().remove(at, actor);
    }
    world.resource_mut::<Turns>().remove(actor);
    let carried: Vec<Entity> = world.get::<Inventory>(actor).map(|bag| bag.items.clone()).unwrap_or_default();
    for item in carried {
        world.entity_mut(item).insert((Position(at), OnMap(map)));
    }
    let mut e = world.entity_mut(actor);
    e.remove::<(Actor, Blocks, Position, Inventory, Equipped)>().insert(Dead);
}

/// Takes out every conjured thing whose time is up, and says so of each
/// the adventurer can see.
///
/// Straight after `TurnSet::React`, inside the pass, so a blade's last
/// turn is the one its time ends in, and after the reactions rather than
/// among them, since it reads the whole world and would be unordered
/// against every one. Read off the turn clock rather than counted, so a
/// long action counts every turn it took.
pub fn fade(world: &mut World) {
    let now = world.resource::<Turns>().now();
    let mut fading = world.query::<(Entity, &Fades, &Name, &Position)>();
    let done: Vec<(Entity, String, Point)> =
        fading.iter(world).filter(|(_, fades, ..)| fades.at <= now).map(|(e, _, name, at)| (e, name.as_str().to_string(), at.0)).collect();
    let mut sight = world.query_filtered::<&Viewshed, With<Player>>();
    let seen: Vec<bool> = done.iter().map(|(_, _, at)| sight.iter(world).any(|v| v.can_see(*at))).collect();
    for ((thing, name, _), seen) in done.into_iter().zip(seen) {
        vanish(world, thing);
        if seen {
            let line = format!("The {name} fades away.");
            world.write_message(Tell::new(line, Tones::MUTED));
        }
    }
}
