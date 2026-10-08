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
