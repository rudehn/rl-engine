//! Who someone is: a wand of polymorph's foe made into another kind of
//! thing, a wand of plenty's made into two, and a staff of conjuration's
//! blades called up out of nothing to fight for whoever called them.
//!
//! Each needs a monster's kind and health, which [`EffectWorld`] does not
//! hold, so each queues its work on the effect's commands and does it with
//! the whole world in hand, making what it makes by
//! [`spawn_monster_now`], as a floor's own monsters are made. The Drowned
//! King is the one thing none of them reaches: a run is won by killing
//! him, not by making him a rat or two kings.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::EffectRng;
use rl_engine::rl_rules::ability::{RawValue, read_args};

use super::moving::no_args;
use super::{free_cells_near, the};
use rl_engine::rl_bevy::Life as EngineLife;
use rl_engine::rl_bevy::revive;

use crate::allies::{Ally, Fades, befriend, fight_for, vanish};
use crate::bestiary::{Bestiary, Kind, MonsterDef, spawn_monster_now};

/// The name of the kind a staff of conjuration calls up, as `monsters.ron`
/// writes it.
pub const SPECTRAL_BLADE: &str = "spectral blade";

/// What `who` is, read for a change: its kind, unless that is the boss or
/// it is no monster at all, its health, where it stands, and whether it
/// fights for the adventurer.
struct Body {
    kind: Id<MonsterDef>,
    health: Health,
    at: Point,
    map: MapId,
    ally: bool,
    name: String,
}

/// `who` as a body a change can be made to, or `None`, with the reason
/// said in the log, for the one thing nothing changes.
fn body(world: &mut World, who: Entity) -> Option<Body> {
    let kind = world.get::<Kind>(who)?.0;
    let name = world.get::<Name>(who).map_or_else(String::new, |n| n.as_str().to_string());
    if world.resource::<Bestiary>().defs.get(kind).boss {
        world.write_message(Tell::new(format!("{} is unchanged.", the(&name)), Tones::MUTED));
        return None;
    }
    let health = *world.get::<Health>(who)?;
    let at = world.get::<Position>(who)?.0;
    let map = world.get::<OnMap>(who).map_or(MapId::SURFACE, |m| m.0);
    Some(Body { kind, health, at, map, ally: world.get::<Ally>(who).is_some(), name })
}

/// `share` of `of`'s health as a part of `max`: the same share of the
/// whole, never less than one point, so a change never kills.
fn same_share(of: Health, max: i32) -> i32 {
    let share = (i64::from(max) * i64::from(of.current.max(0)) + i64::from(of.max.max(1)) / 2) / i64::from(of.max.max(1));
    (share as i32).clamp(1, max.max(1))
}

/// Makes everyone it struck into a monster of another kind, drawn from
/// every kind that turns up anywhere under the lake, at the same share of
/// its health it had: a wand of polymorph's. What it carried is left lying
/// where it stood, and one that fought for the adventurer still does.
/// Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Polymorph;

impl Effect for Polymorph {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let landing = landing.clone();
        world.commands.queue(move |world: &mut World| {
            let changed = landing.targets.iter().filter(|target| polymorph(world, **target)).count();
            if changed == 0 {
                super::showed_nothing(world, &landing);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "makes it something else".to_string()
    }
}

impl FromArgs for Polymorph {
    const KIND: &'static str = "Polymorph";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// What a monster of the kind `from` may be made into: every kind that
/// turns up anywhere under the lake but its own, in the bestiary's order.
/// The spawn table is the list, so the king, his knights and a conjured
/// blade, which turn up nowhere, are never what a foe becomes.
pub fn shapes(bestiary: &Bestiary, from: Id<MonsterDef>) -> Vec<Id<MonsterDef>> {
    let mut kinds: Vec<Id<MonsterDef>> = bestiary.spawns.entries().iter().map(|row| row.item).filter(|k| *k != from).collect();
    kinds.sort_by_key(|k| k.index());
    kinds.dedup();
    kinds
}

/// Makes `target` a monster of another kind, as [`Polymorph`] says, and
/// says whether it did: not the king, nor a kind with nothing to become.
pub fn polymorph(world: &mut World, target: Entity) -> bool {
    let Some(was) = body(world, target) else { return false };
    let kinds = shapes(world.resource::<Bestiary>(), was.kind);
    if kinds.is_empty() {
        return false;
    }
    let pick = world.resource_mut::<EffectRng>().random_range(0..kinds.len());
    let kind = kinds[pick];
    vanish(world, target);
    let made = spawn_monster_now(world, kind, was.at, was.map);
    let max = world.resource::<Bestiary>().defs.get(kind).hp;
    world.entity_mut(made).insert(Health { current: same_share(was.health, max), max });
    if was.ally {
        befriend(world, made);
    }
    let now = world.resource::<Bestiary>().defs.get(kind).name.clone();
    let line = format!("{} turns into {}!", the(&was.name), rl_engine::rl_core::noun::with_article(&now));
    world.write_message(Tell::new(line, Tones::NOTICE));
    true
}

/// Makes everyone it struck into two, each with half the health the one
/// had, the second beside the first and on its side: a foe made two, and
/// an ally made a second ally. Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Plenty;

impl Effect for Plenty {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let landing = landing.clone();
        world.commands.queue(move |world: &mut World| {
            let split = landing.targets.iter().filter(|target| plenty(world, **target)).count();
            if split == 0 {
                super::showed_nothing(world, &landing);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "makes it two, each half as hale".to_string()
    }
}

impl FromArgs for Plenty {
    const KIND: &'static str = "Plenty";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Makes `target` two, as [`Plenty`] says, and says whether it did: not
/// with no free cell beside it for a second, which shows nothing.
pub fn plenty(world: &mut World, target: Entity) -> bool {
    let Some(was) = body(world, target) else { return false };
    let beside = free_cells_near(world.resource::<WorldMap>(), world.resource::<Occupancy>(), was.at, 1, 1);
    let Some(at) = beside.first().copied() else { return false };
    let half = Health { current: (was.health.current + 1) / 2, max: was.health.max };
    world.entity_mut(target).insert(half);
    let made = spawn_monster_now(world, was.kind, at, was.map);
    world.entity_mut(made).insert(half);
    if let Some(side) = world.get::<Faction>(target).copied() {
        world.entity_mut(made).insert(side);
    }
    if was.ally {
        befriend(world, made);
    }
    world.write_message(Tell::new(format!("{} splits in two!", the(&was.name)), Tones::NOTICE));
    true
}

/// Monsters of the floor its user stands on called up on the free cells
/// nearest them, knowing just where they are: a scroll of summon monsters'.
/// Written `(kind: "Summon", args: (fewest: 2, most: 3))`, how many, each
/// drawn from the floor's own spawn table as its people were. Where no
/// monster is ever found, in Mereholt, it shows nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Summon {
    /// The fewest that come.
    pub fewest: u32,
    /// The most that come.
    pub most: u32,
}

impl Effect for Summon {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let count = world.rng.random_range(self.fewest..=self.most);
        let landing = landing.clone();
        world.commands.queue(move |world: &mut World| {
            if summon(world, landing.user, count) == 0 {
                super::showed_nothing(world, &landing);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("{} to {} monsters appear beside you", self.fewest, self.most)
    }
}

impl FromArgs for Summon {
    const KIND: &'static str = "Summon";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            fewest: u32,
            most: u32,
        }
        let a: Args = read_args(args)?;
        if a.fewest > a.most {
            return Err(format!("summons {} to {}, a range that is empty", a.fewest, a.most));
        }
        Ok(Self { fewest: a.fewest, most: a.most })
    }
}

/// Calls up `count` monsters of `reader`'s floor beside them, aware of
/// them, as [`Summon`] says, and says so. Returns how many came.
pub fn summon(world: &mut World, reader: Entity, count: u32) -> usize {
    let Some(at) = world.get::<Position>(reader).map(|p| p.0) else { return 0 };
    let map = world.get::<OnMap>(reader).map_or(MapId::SURFACE, |m| m.0);
    let band = map.0 as i32;
    let cells = free_cells_near(world.resource::<WorldMap>(), world.resource::<Occupancy>(), at, count as usize, super::NEAR);
    let mut came = Vec::new();
    for cell in cells {
        let kind = world.resource_scope(|world, mut rng: Mut<EffectRng>| world.resource::<Bestiary>().spawns.pick(band, &mut rng.0).map(|row| row.item));
        let Some(kind) = kind else { break };
        let made = spawn_monster_now(world, kind, cell, map);
        if let Some(mut aware) = world.get_mut::<Aware>(made) {
            let mut knows = aware.of(reader);
            knows.alerted_to(at);
            aware.0.insert(reader, knows);
        }
        came.push(world.resource::<Bestiary>().defs.get(kind).name.clone());
    }
    if !came.is_empty() && world.get::<Player>(reader).is_some() {
        let line = match came.as_slice() {
            [one] => format!("{} appears beside you!", super::capital(&rl_engine::rl_core::noun::with_article(one))),
            _ => "Monsters appear all round you!".to_string(),
        };
        world.write_message(Tell::new(line, Tones::BAD));
    }
    came.len()
}

/// Raises the user's greatest health by `by` for good and heals them whole:
/// a potion of life's. Written `(kind: "Life", args: (by: 10))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Life {
    /// Health added to the most there can be.
    pub by: i32,
}

impl Effect for Life {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (user, by) = (landing.user, self.by);
        world.commands.queue(move |world: &mut World| {
            let Some(mut health) = world.get_mut::<Health>(user) else { return };
            health.max += by;
            health.current = health.max;
            if world.get::<Player>(user).is_some() {
                world.write_message(Tell::new("You feel more alive than you ever have.", Tones::GOOD));
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("{} more health for good, and healed whole", self.by)
    }
}

impl FromArgs for Life {
    const KIND: &'static str = "Life";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            by: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { by: a.by })
    }
}

/// Calls up spectral blades on the free cells nearest its user, on the
/// user's own side, for a few turns: a staff of conjuration's, and a
/// goblin conjurer's trait of the same name. Written `(kind: "Conjure",
/// args: (blades: 1, turns: 15))`: one blade more for each of its
/// carrier's [`EffectBonus`] amount, and that amount added to every
/// blade's blow, so a staff a level better calls one more and each
/// strikes harder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conjure {
    /// How many come with nothing added.
    pub blades: u32,
    /// How many whole turns they last.
    pub turns: u32,
}

impl Conjure {
    /// How many come with `bonus` added.
    pub fn blades_with(&self, bonus: EffectBonus) -> u32 {
        (self.blades as i32 + bonus.amount).max(1) as u32
    }
}

impl Effect for Conjure {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (user, count, sharper, turns) = (landing.user, self.blades_with(landing.bonus), landing.bonus.amount, self.turns);
        world.commands.queue(move |world: &mut World| conjure(world, user, count, sharper, turns));
    }

    fn describe(&self, _: &Registries, bonus: EffectBonus) -> String {
        format!("{} spectral blades fight beside you for {} turns", self.blades_with(bonus), self.turns)
    }
}

impl FromArgs for Conjure {
    const KIND: &'static str = "Conjure";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            blades: u32,
            turns: u32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { blades: a.blades, turns: a.turns })
    }
}

/// Calls up `count` spectral blades beside `user`, each blow `sharper`, on
/// `user`'s own side until `turns` whole turns from now, as [`Conjure`]
/// says: the adventurer's own staff calls up a companion, and a goblin
/// conjurer's trait calls up one more goblin blade, never a rival on the
/// adventurer's side of its own fight.
///
/// `user` with no side of its own, which nothing under the lake is, shows
/// nothing rather than calling up a blade nobody's mind can read a side
/// for.
pub fn conjure(world: &mut World, user: Entity, count: u32, sharper: i32, turns: u32) {
    let Some(blade) = world.resource::<Bestiary>().defs.id(SPECTRAL_BLADE) else { return };
    let Some(side) = world.get::<Faction>(user).copied() else { return };
    let Some(at) = world.get::<Position>(user).map(|p| p.0) else { return };
    let map = world.get::<OnMap>(user).map_or(MapId::SURFACE, |m| m.0);
    let fades = Fades { at: world.resource::<Turns>().now() + turns * BASE_ACTION_COST };
    let cells = free_cells_near(world.resource::<WorldMap>(), world.resource::<Occupancy>(), at, count as usize, super::NEAR);
    for cell in &cells {
        let made = spawn_monster_now(world, blade, *cell, map);
        fight_for(world, made, side.0);
        let mut e = world.entity_mut(made);
        e.insert(fades);
        if let Some(mut blow) = e.get_mut::<MeleeAttack>() {
            blow.dice.bonus += sharper;
        }
    }
    // Only the adventurer's own casting is told: a goblin's blades spring
    // up where the goblin stands, which the adventurer sees for themselves
    // if they are watching, and "beside you" would be a lie said of a
    // goblin's own conjuring.
    if world.get::<Player>(user).is_none() {
        return;
    }
    let line = match cells.len() {
        0 => return,
        1 => "A spectral blade springs up beside you.".to_string(),
        n => format!("{n} spectral blades spring up beside you."),
    };
    world.write_message(Tell::new(line, Tones::GOOD));
}

/// How far from the one who raises it a body may lie and still be
/// raised, and still be sought by [`raise_dead`]: `raising`'s own number.
pub const RAISE_REACH: i32 = 6;

/// Stands the nearest body in sight and within [`RAISE_REACH`] back up as
/// its user's own ally, for good: `raising`'s ability, `raise dead`,
/// built on the engine's own `revive`, which gives a raised body back its
/// own health and everything else it died with, and [`fight_for`], which
/// is the "a raised body becomes an ally of the raiser through a
/// `befriend` generalised to any side" the research called for. Takes no
/// arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RaiseDead;

impl Effect for RaiseDead {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (user, landing) = (landing.user, landing.clone());
        world.commands.queue(move |world: &mut World| {
            if !raise_dead(world, user) {
                super::showed_nothing(world, &landing);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("a body within {RAISE_REACH} cells in sight rises as its own ally")
    }

    // Only with a body to raise, so a necromancer with none in reach does
    // something else with its turn rather than raising nothing.
    fn would_act(&self, landing: &Landing, world: &World) -> bool {
        nearest_body(world, landing.user).is_some()
    }
}

impl FromArgs for RaiseDead {
    const KIND: &'static str = "RaiseDead";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// A body stood back up by [`raise_dead`] or [`raise_ally`]: its second
/// death drops nothing and counts nothing on the tally, since both were
/// paid out by its first. Without it a mini-boss raised by the
/// Choirmaster's Staff and killed again dropped its artifact again and
/// scored again, as often as the staff could raise it.
///
/// The save keeps it, so a continued run's raised body is no fresh one.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Raised;

/// Takes off a body the engine's own `revive` just stood up everything
/// that does not outlast a death, and marks it [`Raised`]: what it drops,
/// which its first death already dropped; sleep, which a blow on a
/// sleeper did not end since the blow killed it; what it stole, which it
/// let fall where it died; an offer to be freed, a patrol, a post and a
/// leader of its old life; and a link to whoever spawned it. `revive`
/// gives back everything the actor died with, which is right for what it
/// is and wrong for what it was doing.
pub fn forget_its_life(world: &mut World, body: Entity) {
    use rl_engine::rl_bevy::Drops;
    world
        .entity_mut(body)
        .remove::<(Drops<crate::armory::ItemDef>, crate::tricks::Stole, Offers, crate::tricks::PatrolRoute, crate::tricks::Spawned, Post, Leader)>();
    world.entity_mut(body).insert(Raised);
    // Quietly, as the twin put it back: a body that rises is not news of
    // a sleeper waking.
    let Some(asleep) = world.resource::<Registries>().statuses.id(crate::content::ASLEEP) else { return };
    if let Ok((mut held, mut stats)) = world.query::<(&mut Afflicted, &mut StatBlock)>().get_mut(world, body) {
        held.0.cure(asleep, &mut stats.0);
    }
}

/// The body [`raise_dead`] would stand up for `raiser`, and where it lies:
/// the nearest within [`RAISE_REACH`] that `raiser` can see, by steps, then
/// row, then column. Read-only, so [`RaiseDead`] asks it before a turn is
/// spent as well as when the turn lands.
fn nearest_body(world: &World, raiser: Entity) -> Option<(Entity, Point)> {
    let at = world.get::<Position>(raiser)?.0;
    let map = world.get::<OnMap>(raiser).map_or(MapId::SURFACE, |m| m.0);
    let sight = world.get::<Viewshed>(raiser)?;
    let mut remains = world.try_query_filtered::<(Entity, &Position, Option<&OnMap>), With<Remains>>()?;
    let mut bodies: Vec<(Entity, Point)> = remains
        .iter(world)
        .filter(|(_, pos, on)| on.map_or(MapId::SURFACE, |m| m.0) == map && geometry::chebyshev(pos.0, at) <= RAISE_REACH && sight.can_see(pos.0))
        .map(|(e, pos, _)| (e, pos.0))
        .collect();
    bodies.sort_by_key(|(e, pos)| (geometry::chebyshev(*pos, at), pos.y, pos.x, e.index()));
    bodies.into_iter().next()
}

/// Stands the nearest body within [`RAISE_REACH`] of `raiser`, in its
/// sight, back up on `raiser`'s own side, as [`RaiseDead`] says, and says
/// whether it found one to raise: nothing with no body `raiser` can see
/// and reach, nor one something already stands on with nowhere within
/// the engine's own standing room free.
///
/// Nearest first, by steps, then row, then column, so two runs of one
/// seed agree on which body a necromancer with more than one in reach
/// raises.
pub fn raise_dead(world: &mut World, raiser: Entity) -> bool {
    let Some((body, body_at)) = nearest_body(world, raiser) else { return false };
    let twin = world.get::<EngineLife>(body).map(|life| life.0);
    let max = twin.and_then(|twin| world.get::<Health>(twin)).map_or(1, |h| h.max);
    if !revive(world, body, max) {
        return false;
    }
    forget_its_life(world, body);
    if let Some(side) = world.get::<Faction>(raiser).copied() {
        fight_for(world, body, side.0);
    }
    // Told only where the adventurer themselves can see it rise: `raiser`'s
    // own sight found the body, which is not the same thing, and a body
    // raised in a room the adventurer cannot see is not something they
    // learn `raising` from by reading the log.
    if world.query_filtered::<&Viewshed, With<Player>>().iter(world).any(|v| v.can_see(body_at)) {
        let name = world.get::<Name>(body).map_or_else(String::new, |n| n.as_str().to_string());
        world.write_message(Tell::new(format!("{} rises to fight for its raiser!", the(&name)), Tones::BAD));
    }
    true
}

/// Stands the body at the cell a use was pointed at back up on its own
/// user's side for `turns` whole turns, then lets it [`Fades`] away as a
/// conjured thing does: the Choirmaster's staff's own "aimed at a body
/// within 6", which chooses which body rather than [`raise_dead`]'s own
/// nearest. Written `(kind: "RaiseAlly", args: (turns: 30))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaiseAlly {
    /// How many whole turns it fights before it fades.
    pub turns: u32,
}

impl Effect for RaiseAlly {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (user, at, turns) = (landing.user, landing.landed_at.unwrap_or(landing.aim), self.turns);
        let told = landing.clone();
        world.commands.queue(move |world: &mut World| {
            if !raise_ally(world, user, at, turns) {
                super::showed_nothing(world, &told);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("the body at the cell aimed at rises on your side for {} turns", self.turns)
    }
}

impl FromArgs for RaiseAlly {
    const KIND: &'static str = "RaiseAlly";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            turns: u32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { turns: a.turns })
    }
}

/// Stands the body standing at `at`, on `raiser`'s own map, back up on
/// `raiser`'s own side for `turns` whole turns before it [`Fades`], as
/// [`RaiseAlly`] says: the engine's own `revive` gives it back its health,
/// [`fight_for`] turns it, and the same clock a conjured blade fades by
/// takes it back out of play once its time runs out, since nothing kills
/// it a second time. Returns whether a body stood at `at` to raise.
pub fn raise_ally(world: &mut World, raiser: Entity, at: Point, turns: u32) -> bool {
    let map = world.get::<OnMap>(raiser).map_or(MapId::SURFACE, |m| m.0);
    let body = world
        .query_filtered::<(Entity, &Position, Option<&OnMap>), With<Remains>>()
        .iter(world)
        .find(|(_, pos, on)| pos.0 == at && on.map_or(MapId::SURFACE, |m| m.0) == map)
        .map(|(e, ..)| e);
    let Some(body) = body else { return false };
    let twin = world.get::<EngineLife>(body).map(|life| life.0);
    let max = twin.and_then(|twin| world.get::<Health>(twin)).map_or(1, |h| h.max);
    if !revive(world, body, max) {
        return false;
    }
    forget_its_life(world, body);
    if let Some(side) = world.get::<Faction>(raiser).copied() {
        fight_for(world, body, side.0);
    }
    let fades = Fades { at: world.resource::<Turns>().now() + turns * BASE_ACTION_COST };
    world.entity_mut(body).insert(fades);
    if world.query_filtered::<&Viewshed, With<Player>>().iter(world).any(|v| v.can_see(at)) {
        let name = world.get::<Name>(body).map_or_else(String::new, |n| n.as_str().to_string());
        world.write_message(Tell::new(format!("{} rises to fight for its raiser!", the(&name)), Tones::GOOD));
    }
    true
}

/// A noise at its wielder's own cell, as loud as a blow: the Widow's
/// Fang's own "it whispers", landed by its `pulse` trigger every sixty
/// turns rather than by anything the wielder does. Lets whatever has ears
/// hear exactly what a blow sounds like, with nobody having struck one, so
/// standing still and quiet is no longer quiet with it on the hip. Takes
/// no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Whisper;

impl Effect for Whisper {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let user = landing.user;
        world.commands.queue(move |world: &mut World| {
            let Some(at) = world.get::<Position>(user).map(|p| p.0) else { return };
            world.write_message(MakeNoise { at, loudness: crate::plugin::NOISE.strike, sound: Sounds::STRIKE, maker: Some(user) });
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "a noise at its wielder's own cell, as loud as a blow".to_string()
    }
}

impl FromArgs for Whisper {
    const KIND: &'static str = "Whisper";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Hastens every ally the user can see, itself included, for `turns`:
/// Grubnak's `war cry`. `aim: SelfOnly` and `mode: Own`, the same reason
/// `conjure blades` takes neither: its own sight decides who it hits,
/// never a cursor, and an ordinary `Aim::Ally` bolt only ever finds the
/// one nearest. Written `(kind: "Rally", args: (turns: 4))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rally {
    /// How many whole turns the haste lasts.
    pub turns: u32,
}

impl Effect for Rally {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (user, turns) = (landing.user, self.turns);
        world.commands.queue(move |world: &mut World| rally(world, user, turns));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("hastens every ally in sight for {} turns", self.turns)
    }
}

impl FromArgs for Rally {
    const KIND: &'static str = "Rally";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            turns: u32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { turns: a.turns })
    }
}

/// Hastens every actor on `user`'s own side that `user` can see, `user`
/// included, for `turns`, as [`Rally`] says. Nothing with no side of its
/// own, nor with no sight to rally by.
pub fn rally(world: &mut World, user: Entity, turns: u32) {
    let Some(hasted) = world.resource::<Registries>().statuses.id(crate::content::HASTED) else { return };
    let map = world.get::<OnMap>(user).map_or(MapId::SURFACE, |m| m.0);
    let Some(sight) = world.get::<Viewshed>(user).cloned() else { return };
    let side = world.get::<Faction>(user).copied();
    // Collected before `CombatRules` is borrowed, so the query's own
    // borrow of `world` is done with by the time the relation is read,
    // `CombatRules` not being `Clone` to hold onto instead.
    let seen: Vec<(Entity, Option<Faction>)> = world
        .query_filtered::<(Entity, &Position, Option<&OnMap>, Option<&Faction>), With<Actor>>()
        .iter(world)
        .filter(|(_, pos, on, _)| on.map_or(MapId::SURFACE, |m| m.0) == map && sight.can_see(pos.0))
        .map(|(e, _, _, theirs)| (e, theirs.copied()))
        .collect();
    let rules = world.get_resource::<CombatRules>();
    let allies: Vec<Entity> = seen
        .into_iter()
        .filter(|(e, theirs)| {
            *e == user
                || matches!((rules, side, theirs), (Some(rules), Some(mine), Some(theirs)) if rules.factions.relation(theirs.0, mine.0) == Relation::Allied)
        })
        .map(|(e, _)| e)
        .collect();
    for ally in allies {
        world.write_message(Afflict { target: ally, status: hasted, turns, by: Some(user), held_by: None });
    }
}

/// Stands up to `count` drowned thralls on cells of standing water within
/// `range` of `user`, wading tiles rather than the lake's own deep water
/// or the throne's black pools, which stop a body rather than letting one
/// rise: the Drowned Abbess's `call the drowned`. Written `(kind:
/// "CallDrowned", args: (count: 2, range: 6))`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallDrowned {
    /// How many rise, at most.
    pub count: u32,
    /// How far from the caller a cell of water may lie.
    pub range: i32,
}

impl Effect for CallDrowned {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (user, count, range) = (landing.user, self.count, self.range);
        let landing = landing.clone();
        world.commands.queue(move |world: &mut World| {
            if call_drowned(world, user, count, range).0 == 0 {
                super::showed_nothing(world, &landing);
            }
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        match self.count {
            1 => format!("a drowned thrall rises from water within {}", self.range),
            n => format!("up to {n} drowned thralls rise from water within {}", self.range),
        }
    }
}

impl FromArgs for CallDrowned {
    const KIND: &'static str = "CallDrowned";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            count: u32,
            range: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { count: a.count, range: a.range })
    }
}

/// A tile's move cost that marks it as standing water: the wading tiles
/// `dungeon.rs` builds with its own `wading` helper, the shallows, the
/// flooded floor and the black pools, every one of them twice the cost of
/// bare ground and none other. Shared rather than read off a name, since
/// `call_drowned` has no list of tile names of its own to keep in step
/// with the biomes that add one.
const WADING_MOVE_COST: u32 = 200;

/// Stands up to `count` drowned thralls on free cells of standing water
/// within `range` of `caller`, nearest rows first so two runs of one seed
/// agree, as [`CallDrowned`] says. Returns where each rose, and whether
/// the adventurer saw any of it: the caller, or the water one rose from,
/// in their sight, since a thrall rising from a pool across the hall from
/// a caller out of sight is still seen to rise.
pub fn call_drowned(world: &mut World, caller: Entity, count: u32, range: i32) -> (usize, bool) {
    let Some(thrall) = world.resource::<Bestiary>().defs.id("drowned thrall") else { return (0, false) };
    let Some(at) = world.get::<Position>(caller).map(|p| p.0) else { return (0, false) };
    let map = world.get::<OnMap>(caller).map_or(MapId::SURFACE, |m| m.0);
    let worldmap = world.resource::<WorldMap>();
    let tables = worldmap.tables();
    let occupancy = world.resource::<Occupancy>();
    let mut cells: Vec<Point> = geometry::square(at, range)
        .filter(|p| *p != at && geometry::chebyshev(*p, at) <= range)
        .filter(|p| worldmap.tile(*p).is_some_and(|t| tables.walkable[t.index()] && tables.move_cost[t.index()] == WADING_MOVE_COST))
        .filter(|p| !occupancy.is_occupied(*p))
        .collect();
    cells.sort_by_key(|p| (p.y, p.x));
    cells.truncate(count as usize);
    let n = cells.len();
    let seen = n > 0 && world.query_filtered::<&Viewshed, With<Player>>().iter(world).any(|v| v.can_see(at) || cells.iter().any(|c| v.can_see(*c)));
    for cell in cells {
        spawn_monster_now(world, thrall, cell, map);
    }
    if seen {
        let line = match n {
            1 => "A drowned thrall rises from the water!".to_string(),
            _ => format!("{n} drowned thralls rise from the water!"),
        };
        world.write_message(Tell::new(line, Tones::BAD));
    }
    (n, seen)
}

/// Hatches an egg sac into two cave spiders beside `at`: the Widow's own,
/// landed either when a sac is struck (its one health spent, the engine's
/// `destroyed` moment) or when something steps beside it, which
/// `crate::tricks::hatch_beside` watches for. Written `(kind: "Hatch",
/// args: (monster: "cave spider", count: 2))`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hatch {
    /// The kind that hatches out.
    pub monster: String,
    /// How many.
    pub count: u32,
}

impl Effect for Hatch {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (at, map, monster, count) = (landing.origin, world.map().current(), self.monster.clone(), self.count);
        world.commands.queue(move |world: &mut World| {
            hatch(world, &monster, count, at, map);
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("{} {} hatch out", self.count, self.monster)
    }
}

impl FromArgs for Hatch {
    const KIND: &'static str = "Hatch";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            monster: String,
            count: u32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { monster: a.monster, count: a.count })
    }
}

/// Spawns up to `count` of the monster named `monster` on free cells
/// nearest `at` on `map`: an egg sac's hatch, struck or stepped beside.
/// Nothing with no monster of that name, which a content typo would
/// otherwise leave as a sac that hatches nothing rather than a load-time
/// failure; `Hatch`'s own name is read against the bestiary at this
/// moment rather than resolved at load, since a prop's trigger effects
/// are built against the engine's registered kinds, which know nothing of
/// a game's own monsters.
pub fn hatch(world: &mut World, monster: &str, count: u32, at: Point, map: MapId) {
    let Some(kind) = world.resource::<Bestiary>().defs.id(monster) else { return };
    let cells = free_cells_near(world.resource::<WorldMap>(), world.resource::<Occupancy>(), at, count as usize, super::NEAR);
    for cell in cells {
        spawn_monster_now(world, kind, cell, map);
    }
}

/// Every prop called `prop` within `radius` of where it lands stands up
/// as a `monster`, knowing where whoever set it off stands: a bone
/// circle's dead rising when it is crossed, every body of the ring at
/// once, whichever of its cells was stepped on. Written `(kind: "Rise",
/// args: (prop: "circle bones", monster: "skeleton", radius: 2))`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rise {
    /// The props that rise.
    pub prop: String,
    /// What each rises as.
    pub monster: String,
    /// How far from where it lands, in cells.
    pub radius: i32,
}

impl Effect for Rise {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (at, map, rise, by) = (landing.origin, world.map().current(), self.clone(), landing.targets.clone());
        world.commands.queue(move |world: &mut World| rise.raise(world, at, map, &by));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("every {} near it rises as a {}", self.prop, self.monster)
    }
}

impl Rise {
    /// Takes every one of [`Rise::prop`] within [`Rise::radius`] of `at`
    /// on `map` off the map and stands a [`Rise::monster`] where it lay,
    /// aware of each of `by`. Nothing with no monster of that name, read
    /// against the bestiary now, as [`hatch`] reads its own.
    pub fn raise(&self, world: &mut World, at: Point, map: MapId, by: &[Entity]) {
        let Some(kind) = world.resource::<Bestiary>().defs.id(&self.monster) else { return };
        let Some(prop) = world.resource::<Registries>().props.id(&self.prop) else { return };
        let bodies: Vec<(Entity, Point)> = {
            let mut q = world.query::<(Entity, &PropKind, &Position, &OnMap)>();
            q.iter(world)
                .filter(|(_, k, p, on)| k.0 == prop && on.0 == map && rl_engine::rl_core::geometry::chebyshev(p.0, at) <= self.radius)
                .map(|(e, _, p, _)| (e, p.0))
                .collect()
        };
        let seen: Vec<(Entity, Point)> = by.iter().filter_map(|e| world.get::<Position>(*e).map(|p| (*e, p.0))).collect();
        for (body, cell) in &bodies {
            world.despawn(*body);
            let risen = spawn_monster_now(world, kind, *cell, map);
            if let Some(mut aware) = world.get_mut::<Aware>(risen) {
                for (who, there) in &seen {
                    let mut knows = aware.of(*who);
                    knows.alerted_to(*there);
                    aware.0.insert(*who, knows);
                }
            }
        }
        if !bodies.is_empty() && by.iter().any(|e| world.get::<Player>(*e).is_some()) {
            world.write_message(Tell::new("The bones of the circle stir, and stand.", Tones::BAD));
        }
    }
}

impl FromArgs for Rise {
    const KIND: &'static str = "Rise";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            prop: String,
            monster: String,
            radius: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { prop: a.prop, monster: a.monster, radius: a.radius })
    }
}
