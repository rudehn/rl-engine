//! The Bevy layer: the loops the engine owns.
//!
//! Everything below tier 2 is data and algorithms. This crate is where they
//! run: the turn loop, chunk streaming, field of view, occupancy. A game
//! adds these plugins, spawns actors with the components here, and writes
//! its own systems into the named [`EngineSet`]s.
//!
//! The rule from the previous engine's post-mortem: own the loop or leave
//! the subsystem out. The scheduler here advances the clock, deals turns,
//! requeues actors and recovers stalls. A game only decides what an actor
//! does with its turn.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod combat;
pub mod components;
pub mod fov;
pub mod knowledge;
pub mod plugin;
pub mod state;
pub mod turn;
pub mod world;

pub use combat::{Armor, CombatRng, CombatRules, DamageEvent, DamageStages, DeathEvent, Faction, FlowFields, Health, MeleeAttack, Mind, Perception, Profile, Resists};
pub use components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
pub use knowledge::Knowledge;
pub use plugin::{EnginePlugins, EngineSet};
pub use state::EngineState;
pub use turn::{Action, ActionDone, ActionRefused, Intent, Occupancy, TurnEnd, Turns};
pub use world::{ChunkRulesRes, WindowView, WorldMap, WorldRes, WorldSettings};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::combat::{Armor, CombatRng, CombatRules, DamageEvent, DamageStages, DeathEvent, Faction, FlowFields, Health, MeleeAttack, Mind, Perception, Profile, Resists};
    pub use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
    pub use crate::knowledge::Knowledge;
    pub use crate::plugin::{EnginePlugins, EngineSet};
    pub use crate::state::EngineState;
    pub use crate::turn::{Action, ActionDone, ActionRefused, Intent, Occupancy, TurnEnd, Turns};
    pub use crate::world::{ChunkRulesRes, WindowView, WorldMap, WorldRes, WorldSettings};
}
