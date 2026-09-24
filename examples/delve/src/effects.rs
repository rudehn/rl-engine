//! The one effect the delve adds to the engine's seven.
//!
//! The engine owns pools but ships no effect that fills one, because what
//! refills mana is a question about a game's economy and not about
//! abilities. `Drain` is the delve's answer, and this file is the whole of
//! its side of that boundary: a struct that reads its arguments, and a
//! function that asks the world for something through `commands`. It never
//! touches the resolver, the gate, the targeting or the turn, and the RON
//! that names it looks exactly like the RON that names `Harm`.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::StatId;
use rl_engine::rl_rules::ability::{RawValue, read_args};

/// Pour into the user's pool, up to what its stats allow, whenever the
/// ability lands on something.
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
            // The stat says how large the pool may be, so a delver with more
            // mana to hold can drain further into it.
            let max = {
                let defs = &w.resource::<Registries>().stats;
                w.get::<StatBlock>(user).map(|s| s.0.value(pool, defs)).unwrap_or(0)
            };
            if let Some(mut pools) = w.get_mut::<Pools>(user) {
                pools.fill(pool, amount, max);
            }
        });
        // What was drained is seen to come back: a flight from the one it
        // was taken from to the user, after the burst that took it.
        // Only for an ability: the knack has a look, and a trap or a prop
        // landing this same effect has none to fly.
        if let Some(&target) = landing.targets.first()
            && let Some(at) = world.position(target)
            && let Some(ability) = landing.ability()
        {
            let look = LookOf::Ability(ability);
            world.cues.write(Cued { actor: user, cue: Cue::Flight { from: Anchor::on(target, at), to: Anchor::on(user, landing.origin), look } });
        }
    }

    fn describe(&self, registries: &Registries) -> String {
        format!("gives you back {} {}", self.amount, registries.stats.name(self.pool))
    }
}

impl FromArgs for Drain {
    const KIND: &'static str = "Drain";

    fn from_args(args: &RawValue, names: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        struct Args {
            pool: String,
            amount: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { pool: names.stat(&a.pool)?, amount: a.amount })
    }
}
