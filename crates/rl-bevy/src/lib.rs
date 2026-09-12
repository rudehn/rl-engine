//! The Bevy layer: the loops the engine owns.
//!
//! Everything below tier 2 is data and algorithms. This crate is where they
//! run: the turn loop, chunk streaming, field of view, occupancy. A game
//! adds these plugins, spawns actors with the components here, and writes
//! its own systems into the named [`EngineSet`]s and [`TurnSet`]s.
//!
//! The rule from the previous engine's post-mortem: own the loop or leave
//! the subsystem out. The scheduler here advances the clock, deals turns,
//! requeues actors and recovers stalls. A game only decides what an actor
//! does with its turn.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod combat;
pub mod components;
pub mod events;
pub mod fov;
pub mod items;
pub mod knowledge;
pub mod lighting;
pub mod places;
pub mod plugin;
pub mod state;
pub mod status;
pub mod turn;
pub mod world;

pub use combat::{
    Armor, Attack, CombatPlugin, CombatRng, CombatRules, DamageDealt, DamageEvent, DamageStages, Dead, DeathEvent, Faction, FlowFields, Health, MeleeAttack,
    Mind, Perception, Profile, RangedAttack, Resists, Strikes, line_of_fire,
};
pub use components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
pub use events::{Counters, FactsPlugin, Happened, QuestChange, Quests};
pub use fov::FovPlugin;
pub use items::{DropItem, Enchant, Equip, Equipped, Inventory, Item, ItemEvent, ItemsPlugin, PickUp, Stack, Unequip, UseItem, Wearable};
pub use knowledge::{Knowledge, KnowledgeSave};
pub use lighting::{DarkSight, Fuel, LightEvent, LightSource, Lighting, LightingPlugin};
pub use places::{
    Arrive, Destination, GoThrough, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules, PlaceRulesRes, Spot, Transition, WarpRequest,
};
pub use plugin::{CorePlugin, EngineSet, PresentSet, ResolveSet, Turn, TurnSet};
pub use state::EngineState;
pub use status::{Afflict, Afflicted, Cure, StatBlock, StatusEvent, StatusPlugin, StatusRules};
pub use turn::{Acting, Action, ActionDone, ActionRefused, AddAction, Intent, Occupancy, Step, TurnEnd, Turns, Wait};
pub use world::{ChunkLoaded, ChunkRulesRes, PlaceMap, PlaceSave, StreamingPlugin, WindowView, WorldMap, WorldMapSave, WorldRes, WorldSettings};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::combat::{
        Armor, Attack, CombatPlugin, CombatRng, CombatRules, DamageDealt, DamageEvent, DamageStages, Dead, DeathEvent, Faction, FlowFields, Health,
        MeleeAttack, Mind, Perception, Profile, RangedAttack, Resists, Strikes, line_of_fire,
    };
    pub use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
    pub use crate::events::{Counters, FactsPlugin, Happened, QuestChange, Quests};
    pub use crate::fov::FovPlugin;
    pub use crate::items::{DropItem, Enchant, Equip, Equipped, Inventory, Item, ItemEvent, ItemsPlugin, PickUp, Stack, Unequip, UseItem, Wearable};
    pub use crate::knowledge::Knowledge;
    pub use crate::lighting::{DarkSight, Fuel, LightEvent, LightSource, Lighting, LightingPlugin};
    pub use crate::places::{
        Arrive, Destination, GoThrough, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules, PlaceRulesRes, Spot, Transition, WarpRequest,
    };
    pub use crate::plugin::{CorePlugin, EngineSet, PresentSet, ResolveSet, Turn, TurnSet};
    pub use crate::state::EngineState;
    pub use crate::status::{Afflict, Afflicted, Cure, StatBlock, StatusEvent, StatusPlugin, StatusRules};
    pub use crate::turn::{Acting, Action, ActionDone, ActionRefused, AddAction, Intent, Occupancy, Step, TurnEnd, Turns, Wait};
    pub use crate::world::{ChunkLoaded, ChunkRulesRes, PlaceMap, StreamingPlugin, WindowView, WorldMap, WorldRes, WorldSettings};
}
