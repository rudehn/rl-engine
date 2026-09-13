//! The five effects the engine does not ship, one per genre.
//!
//! This file is the honest half of the claim that abilities are data. Nine
//! of the eighteen abilities in `assets/` are built entirely out of effects
//! the engine owns; these five are not, because draining a pool, robbing a
//! pocket, turning a sentry, planting a banner and filling a street with
//! smoke are game vocabulary and the engine has no business knowing any of
//! them.
//!
//! What matters is the size of each: a struct that parses its own
//! arguments and a function that asks the world for something. None of
//! them touches the resolver, the gate, the targeting or the turn, and the
//! RON that names them looks exactly like the RON that names `Harm`.
//!
//! Each reaches the world differently on purpose. `Drain` writes to a
//! component the engine owns, `Plunder` moves one of this game's between
//! two entities, `Hack` replaces one, `Banner` spawns an entity, and
//! `Smoke` edits the map itself. All five go through `commands`, which is
//! the whole of the escape hatch.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::StatId;
use rl_engine::rl_rules::ability::{Lookup, RawValue, read_args};

use crate::{Coin, Prop};

/// Take health off the target and put the same into the user's pool.
///
/// The engine owns [`Pools`] but ships no effect that fills one, because
/// what refills a pool is a question about a game's economy.
#[derive(Debug, Clone, Copy)]
pub struct Drain {
    pool: StatId,
    amount: i32,
}

impl Effect for Drain {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        if landing.targets.is_empty() {
            return;
        }
        let (user, pool, amount) = (landing.user, self.pool, self.amount);
        world.commands.queue(move |w: &mut World| {
            // The stat says how large the pool may be, so a caster with
            // gear that raises maximum mana can drain further into it.
            let max = {
                let defs = &w.resource::<StatRules>().0;
                w.get::<StatBlock>(user).map(|s| s.0.value(pool, defs)).unwrap_or(0)
            };
            if let Some(mut pools) = w.get_mut::<Pools>(user) {
                pools.fill(pool, amount, max);
            }
        });
    }
}

impl FromArgs for Drain {
    const KIND: &'static str = "Drain";

    fn from_args(args: &RawValue, look: &dyn Lookup) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            pool: String,
            amount: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { pool: look.stat(&a.pool).ok_or_else(|| format!("unknown stat {:?}", a.pool))?, amount: a.amount })
    }
}

/// Move a share of the target's coin into the user's purse.
#[derive(Debug, Clone, Copy)]
pub struct Plunder {
    share: u32,
}

impl Effect for Plunder {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (user, share) = (landing.user, self.share.min(100));
        for target in landing.targets.clone() {
            world.commands.queue(move |w: &mut World| {
                let taken = w.get::<Coin>(target).map(|c| c.0 * share / 100).unwrap_or(0);
                if taken == 0 {
                    return;
                }
                if let Some(mut theirs) = w.get_mut::<Coin>(target) {
                    theirs.0 -= taken;
                }
                let mut ours = w.entity_mut(user);
                let held = ours.get::<Coin>().map(|c| c.0).unwrap_or(0);
                ours.insert(Coin(held + taken));
            });
        }
    }
}

impl FromArgs for Plunder {
    const KIND: &'static str = "Plunder";

    fn from_args(args: &RawValue, _look: &dyn Lookup) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            share: u32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { share: a.share })
    }
}

/// Turn the target: it joins the user's side for the rest of the run.
///
/// A faction is an engine component, but deciding that a sentry can be
/// talked round is not an engine idea, so nothing ships that does it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Hack;

impl Effect for Hack {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let user = landing.user;
        for target in landing.targets.clone() {
            world.commands.queue(move |w: &mut World| {
                let Some(side) = w.get::<Faction>(user).copied() else { return };
                w.entity_mut(target).insert(side);
            });
        }
    }
}

impl FromArgs for Hack {
    const KIND: &'static str = "Hack";

    fn from_args(_args: &RawValue, _look: &dyn Lookup) -> Result<Self, String> {
        Ok(Self)
    }
}

/// Plant a standard where the ability landed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Banner;

impl Effect for Banner {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let at = landing.landed_at.unwrap_or(landing.aim);
        world.commands.spawn((Position(at), Prop, Glyph::new('|', Color::srgb(0.9, 0.8, 0.3)).on_layer(1)));
    }
}

impl FromArgs for Banner {
    const KIND: &'static str = "Banner";

    fn from_args(_args: &RawValue, _look: &dyn Lookup) -> Result<Self, String> {
        Ok(Self)
    }
}

/// Fill the footprint with smoke, which blocks sight where it settles.
///
/// The only one of the five that edits the map, and the one that shows how
/// far the escape hatch reaches: `Commands::queue` takes a closure with the
/// whole world, so an effect needing a resource the engine never handed it
/// is still one function long. The tile comes from the game's own
/// [`SmokeTile`], because a tile id is not something an ability file
/// should have to name.
#[derive(Debug, Clone, Copy, Default)]
pub struct Smoke;

/// The tile smoke lays down.
#[derive(Resource, Debug, Clone, Copy)]
pub struct SmokeTile(pub TileId);

impl Effect for Smoke {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let cells = landing.cells.clone();
        world.commands.queue(move |w: &mut World| {
            let Some(tile) = w.get_resource::<SmokeTile>().copied() else { return };
            let mut map = w.resource_mut::<WorldMap>();
            for p in cells {
                if map.is_walkable(p) {
                    map.set_tile(p, tile.0);
                }
            }
        });
    }
}

impl FromArgs for Smoke {
    const KIND: &'static str = "Smoke";

    fn from_args(_args: &RawValue, _look: &dyn Lookup) -> Result<Self, String> {
        Ok(Self)
    }
}
