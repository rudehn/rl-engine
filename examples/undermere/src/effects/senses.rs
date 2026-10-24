//! What is seen and known: a potion of darkness's pool of dark round
//! whoever it breaks on, and a scroll of magic mapping's floor known end
//! to end.
//!
//! What darkness does to the light is the status's, `shrouded`, which puts
//! its holder's lantern out and makes them the harder to notice, and the
//! cloud's, which shrouds whoever breathes it and hides what is behind its
//! thickest; this puts both down where it lands. What the adventurer knows
//! of a floor is the engine's [`Knowledge`], the cells they have seen,
//! which the map draws as remembered; magic mapping marks the rest.

use atglyph::prelude::all::*;
use atglyph::rules::GasId;
use atglyph::rules::ability::{RawValue, read_args};
use bevy::prelude::*;

use crate::content::{DARKNESS, SHROUDED};

/// Darkness where it lands: everyone there `shrouded` for `turns` turns,
/// and `amount` of the darkness cloud given off on every cell of it, so
/// the drinker stands in a dark that hides them and a thrown one leaves a
/// dark to hide in. Written `(kind: "Darkness", args: (turns: 40, amount:
/// 150))`; the status and the cloud are the game's own, `shrouded` and
/// `darkness`, looked up as the file is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Darkness {
    /// Whole turns its holder is shrouded, before any bonus.
    pub turns: u32,
    /// How much of the cloud on each cell, a full cell being 255.
    pub amount: u16,
    /// `shrouded`.
    status: StatusId,
    /// `darkness`.
    gas: GasId,
}

impl Effect for Darkness {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let turns = self.turns + landing.bonus.turns;
        for target in &landing.targets {
            world.afflict.write(ApplyStatus { target: *target, status: self.status, turns, by: Some(landing.user), held_by: None });
        }
        for cell in &landing.cells {
            world.commands.write_message(Release { gas: self.gas, at: *cell, amount: self.amount });
        }
    }

    fn describe(&self, _: &Registries, bonus: EffectBonus) -> String {
        format!("darkness that hides you for {} turns", self.turns + bonus.turns)
    }
}

impl FromArgs for Darkness {
    const KIND: &'static str = "Darkness";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            turns: u32,
            amount: u16,
        }
        let a: Args = read_args(args)?;
        Ok(Self { turns: a.turns, amount: a.amount, status: names.status(SHROUDED)?, gas: names.gas(DARKNESS)? })
    }
}

/// The whole layout of the floor its user stands on made known to them, as
/// if they had seen it all: every cell that can be walked and every wall
/// round one, and nothing of who or what is there. A scroll of magic
/// mapping's. Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MagicMapping;

impl Effect for MagicMapping {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let user = landing.user;
        world.commands.queue(move |world: &mut World| magic_map(world, user));
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "the floor's whole layout known".to_string()
    }
}

impl FromArgs for MagicMapping {
    const KIND: &'static str = "MagicMapping";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        super::moving::no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Marks known every cell of the floor being read that can be walked, and
/// every cell beside one, its walls and its doors, as [`MagicMapping`]
/// says. Only the player's knowing is kept, so a monster reading one maps
/// nothing.
pub fn magic_map(world: &mut World, reader: Entity) {
    if world.get::<Player>(reader).is_none() {
        return;
    }
    let map = world.resource::<WorldMap>();
    let cells: Vec<Point> = map.window_tiles().cells().filter(|p| map.is_loaded(*p)).collect();
    let layout: Vec<Point> = cells.iter().copied().filter(|p| geometry::square(*p, 1).any(|q| map.is_walkable(q))).collect();
    let mut known = world.resource_mut::<Knowledge>();
    for cell in layout {
        known.mark(cell);
    }
    world.write_message(Tell::new("A map of the floor forms in your mind.", Tones::GOOD));
}
