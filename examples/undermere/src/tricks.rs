//! The bestiary's tricks that needed a game system of their own, named by
//! [`Tricks`] off a monster's traits at load: a pink jelly splitting when
//! struck, a monkey stealing and fleeing for it, a wraith unseen in a
//! dark cell, a vampire bat's erratic step, a salamander's trail of fire,
//! a golem turning magic back, a troll whose regeneration stops while it
//! burns, and a rat's own pack or a band's own leader, which are brain
//! shapes rather than anything reactive.
//!
//! `splitting` reads `DamageDealt` rather than the trait model's own
//! `struck`, which fires before a blow's damage is known; `fire-trail`
//! reads `Stepped`; `burns-badly`'s halted mending reads what is true of
//! a body every turn, the way `keep_fireproof` does. `shrouded` needs no
//! system here at all: the engine's own `Concealed`, put on at spawn,
//! keeps a wraith off the map, the sighted list and the look panel while
//! it stands out of the light and not beside the adventurer, decided again
//! every frame from where the light falls.
//! `stealing`'s taking and `withering`'s capped stacking are effects,
//! landed by the engine's actor triggers exactly as `venomous`'s bite is,
//! each registered in [`crate::content::add_game_effects`].
//!
//! The floor ten bosses' own: [`Spawns`] and [`Wells`] are counted off the engine's own
//! `ActionDone`, its own turns rather than the floor's clock, so a slow
//! spawner's trick comes no faster for standing beside a swift one. The
//! Mother Below's `spawning` opens a wall cell in her own sight into the
//! floor tile she herself stands on, so this reads no tile name of
//! `dungeon.rs`'s own; the Drowned King's `welling` is `call_drowned`
//! again, read as a forced turn rather than a chosen ability, over the
//! one water his hall holds, a black pool.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::EffectRng;
use rl_engine::rl_rules::ability::{RawValue, read_args};
use rl_engine::rl_rules::ai::brain::Tactic;
use rl_engine::rl_rules::ai::tactics::{Station, Ways};

use crate::bestiary::{Bestiary, Kind, spawn_monster_now};
use crate::bestiary_lore::MonsterLore;
use crate::effects::free_cells_near;
use crate::traits::TraitDef;

/// Which of the tricks needing a game system of its own a kind's traits
/// name, read once at load against the traits a monster lists rather
/// than by checking a name on every turn. Every other field the trait
/// model already folds; these are brain shapes and marker components
/// instead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tricks {
    /// `splitting`: splits in two when a blow leaves it alive.
    pub splits: bool,
    /// `stealing`: a landed blow takes a thing, then flees with it.
    pub steals: bool,
    /// `shrouded`: unseen while it stands in a dark cell.
    pub shrouds: bool,
    /// `flitting`: one step in three is random.
    pub flits: bool,
    /// `fire-trail`: its steps ignite the ground behind it.
    pub fire_trail: bool,
    /// `reflecting`: the engine's own `Reflects`, marked at spawn.
    pub reflects: bool,
    /// `packing`: keeps station on its own kind.
    pub packs: bool,
    /// `led`: keeps station on its one leader.
    pub led: bool,
    /// `burns-badly`: its regeneration stops while it burns.
    pub burns_badly: bool,
    /// `draining`: mends itself what a landed blow of its own dealt.
    pub drains: bool,
    /// `singing`: a choir plays while allies within `SING_RANGE` stand
    /// near it, the Choirmaster's own.
    pub sings: bool,
    /// `spawning`: a spawnling appears at a wall cell in its sight every
    /// `SPAWN_INTERVAL`th of its own turns, the Mother Below's own.
    pub spawns: bool,
    /// `welling`: a drowned thrall rises from the water within reach
    /// every `RISE_INTERVAL`th of its own turns, the Drowned King's own.
    pub wells: bool,
    /// `sleepy`: twice as likely to start a floor asleep as a plain kind.
    pub sleepy: bool,
    /// `wakeful`: never starts a floor asleep.
    pub wakeful: bool,
}

impl Tricks {
    /// Reads every trick off `listed`, the trait ids a monster names, in
    /// `traits`.
    pub fn from_listed(listed: &[Id<TraitDef>], traits: &Registry<TraitDef>) -> Self {
        let has = |name: &str| traits.id(name).is_some_and(|id| listed.contains(&id));
        Self {
            splits: has("splitting"),
            steals: has("stealing"),
            shrouds: has("shrouded"),
            flits: has("flitting"),
            fire_trail: has("fire-trail"),
            reflects: has("reflecting"),
            packs: has("packing"),
            led: has("led"),
            burns_badly: has("burns-badly"),
            drains: has("draining"),
            sings: has("singing"),
            spawns: has("spawning"),
            wells: has("welling"),
            sleepy: has("sleepy"),
            wakeful: has("wakeful"),
        }
    }

    /// Whether any of these needs a marker component at spawn, for a
    /// `TurnSet::React` system to find by: every one but `packs`, `led`
    /// and `flits`, which are brain shapes built once at load and nothing
    /// a running turn has to look up, and `shrouds`, whose `Concealed` is
    /// the engine's and is put on whatever this says.
    pub fn needs_marker(self) -> bool {
        self.splits || self.fire_trail || self.reflects || self.burns_badly || self.drains || self.sings || self.spawns || self.wells
    }
}

/// Whether the adventurer can see `at` right now: the one check every
/// trick's own narration below makes before naming what it did, so the
/// log never says what a trick is before the look panel would.
fn seen_by_player(world: &mut World, at: Point) -> bool {
    world.query_filtered::<&Viewshed, With<Player>>().iter(world).any(|v| v.can_see(at))
}

mod auras;
mod bodies;
mod bosses;
mod minds;

pub use auras::*;
pub use bodies::*;
pub use bosses::*;
pub use minds::*;
