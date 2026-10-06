//! What is seen and heard: a potion of darkness's pool of dark round
//! whoever it breaks on.
//!
//! What darkness does to the light is the status's, `shrouded`, which puts
//! its holder's lantern out and makes them the harder to notice, and the
//! cloud's, which shrouds whoever breathes it and hides what is behind its
//! thickest; this puts both down where it lands.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::GasId;
use rl_engine::rl_rules::ability::{RawValue, read_args};

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
            world.afflict.write(Afflict { target: *target, status: self.status, turns, by: Some(landing.user), held_by: None });
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
