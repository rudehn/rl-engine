//! Where someone stands: a staff of blinking's step of several cells, a
//! wand of teleport other's, which sends what it strikes far off, and a
//! wand of beckoning's, which pulls it close.
//!
//! Each moves through [`EffectWorld::place`] or [`EffectWorld::slide`], so
//! nobody lands in a wall or on someone else, and the index of who stands
//! where and the mover's sight are kept right.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ability::{RawValue, read_args};

use super::FAR;

/// A free cell of the place being read, drawn from `rng`, at least
/// [`FAR`] steps from `from`; with none that far, one of the furthest
/// there are. `None` on a place with nowhere free at all.
pub fn far_cell(world: &mut EffectWorld<'_, '_>, from: Point) -> Option<Point> {
    let open: Vec<Point> = world.map().window_tiles().cells().filter(|p| *p != from && world.is_free(*p)).collect();
    let furthest = open.iter().map(|p| geometry::chebyshev(*p, from)).max()?;
    let far = FAR.min(furthest);
    let choices: Vec<Point> = open.into_iter().filter(|p| geometry::chebyshev(*p, from) >= far).collect();
    let pick = world.rng.random_range(0..choices.len());
    Some(choices[pick])
}

/// Moves the user along the line they aimed it down, as far as its reach
/// and no further, onto the last cell of the line they can stand on: a
/// staff of blinking's. Written `(kind: "Blink", args: (reach: 2,
/// per_level: 2))`: the reach in cells, and what each of the thing's
/// [`EffectBonus`] amount adds to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blink {
    /// The furthest it goes with nothing added.
    pub reach: i32,
    /// Cells added for each of its carrier's bonus amount.
    pub per_level: i32,
}

impl Blink {
    /// The furthest it goes with `bonus` added.
    pub fn reach_with(&self, bonus: EffectBonus) -> i32 {
        self.reach + self.per_level * bonus.amount
    }
}

impl Effect for Blink {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let reach = self.reach_with(landing.bonus).max(0) as usize;
        let line = landing.path.iter().copied().filter(|p| *p != landing.origin).take(reach);
        let mut to = None;
        for p in line {
            if !world.is_free(p) {
                break;
            }
            to = Some(p);
        }
        if let Some(to) = to {
            world.place(landing.user, to);
        }
    }

    fn describe(&self, _: &Registries, bonus: EffectBonus) -> String {
        format!("blinks you up to {} cells", self.reach_with(bonus))
    }
}

impl FromArgs for Blink {
    const KIND: &'static str = "Blink";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            reach: i32,
            per_level: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { reach: a.reach, per_level: a.per_level })
    }
}

/// Sends everyone it struck to a free cell of the floor far from whoever
/// sent them: a wand of teleport other's. Takes no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TeleportOther;

impl Effect for TeleportOther {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in landing.targets.iter().copied().filter(|t| *t != landing.user) {
            if let Some(to) = far_cell(world, landing.origin) {
                world.place(target, to);
            }
        }
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "sends it far away".to_string()
    }
}

impl FromArgs for TeleportOther {
    const KIND: &'static str = "TeleportOther";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Pulls everyone it struck along the line to whoever sent it, to the
/// nearest cell beside them it can stand on: a wand of beckoning's. Takes
/// no arguments.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Beckon;

impl Effect for Beckon {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        for target in landing.targets.iter().copied().filter(|t| *t != landing.user) {
            let Some(at) = world.position(target) else { continue };
            let steps = geometry::chebyshev(at, landing.origin) - 1;
            if steps > 0 {
                world.slide(target, at, landing.origin, steps);
            }
        }
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "pulls it to your side".to_string()
    }
}

impl FromArgs for Beckon {
    const KIND: &'static str = "Beckon";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        no_args(args, Self::KIND).map(|_| Self)
    }
}

/// Refuses any argument at all, for an effect that takes none: a field
/// written for one is a typo for another effect, or a number that would
/// quietly do nothing.
pub fn no_args(args: &RawValue, kind: &str) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Nothing {}
    let text = args.get_ron().trim();
    if text.is_empty() || text == "()" {
        return Ok(());
    }
    read_args::<Nothing>(args).map(|_| ()).map_err(|e| format!("{kind} takes no arguments: {e}"))
}
