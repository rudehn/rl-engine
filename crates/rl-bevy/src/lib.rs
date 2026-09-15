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

pub mod ability;
pub mod combat;
pub mod components;
pub mod effects;
pub mod events;
pub mod fov;
pub mod items;
pub mod knowledge;
pub mod lighting;
pub mod minds;
pub mod places;
pub mod plugin;
pub mod seed;
pub mod state;
pub mod status;
pub mod stealth;
pub mod testing;
pub mod turn;
pub mod world;

pub use ability::{
    Abilities, AbilitiesPlugin, AbilityEvent, AbilityRng, AddEffect, Aimed, Bystanders, Charges, Cooldowns, Effect, EffectKinds, EffectWorld, FromArgs, Grants,
    Known, Landed, Landing, Offered, Pools, Use,
};
pub use combat::{
    Armor, Attack, CombatPlugin, CombatRng, CombatRules, DamageDealt, DamageEvent, DamageStages, Dead, DeathEvent, Faction, Health, MeleeAttack, RangedAttack,
    Resists, Strikes, line_of_fire,
};
pub use components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
pub use effects::{AddEngineEffects, Cleanse, Harm, Inflict, Mend, Pull, Shove, Teleport};
pub use events::{Counters, FactsPlugin, Happened, QuestChange, Quests};
pub use fov::FovPlugin;
pub use items::{DropItem, Enchant, Equip, Equipped, Inventory, Item, ItemEvent, ItemsPlugin, PickUp, Slots, Stack, Tagged, Unequip, UseItem, Wearable};
pub use knowledge::{Knowledge, KnowledgeSave};
pub use lighting::{DarkSight, Fuel, LightEvent, LightSource, Lighting, LightingPlugin};
pub use minds::{FlowFields, Mind, MindChose, MindsPlugin, Perception, Profile};
pub use places::{
    Arrive, Destination, GoThrough, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules, PlaceRulesRes, Spot, Transition, WarpRequest,
};
pub use plugin::{CleanupSet, CorePlugin, DecideSet, EngineSet, Needs, PresentSet, Requirements, ResolveSet, Turn, TurnSet, depends_on};
pub use seed::{AddStream, Seed, Stream};
pub use state::EngineState;
pub use status::{Afflict, Afflicted, Cure, StatBlock, StatRules, StatusEvent, StatusPlugin, StatusRules};
pub use stealth::{Aware, Notice, Noticed, Stealth, StealthPlugin, StealthRunning, Watchers};
pub use turn::{Acting, Action, ActionDone, ActionRefused, AddAction, Intent, Occupancy, Resolution, Step, TurnEnd, Turns, Wait};
pub use world::{ChunkLoaded, ChunkRulesRes, PlaceMap, PlaceSave, StreamingPlugin, WindowView, WorldMap, WorldMapSave, WorldRes, WorldSettings};

/// The names a game writes.
///
/// What a game spawns, sends, reads and registers, and the traits whose
/// methods it calls. Not what other engine crates build on, such as
/// `Bystanders` and `Offered`, nor the effects a game names only in RON:
/// those stay at the crate root, and an effect in the prelude was a name
/// that collided with a game's own `Shove` action.
pub mod prelude {
    pub use crate::ability::{
        Abilities, AbilitiesPlugin, AbilityEvent, AbilityRng, AddEffect, Charges, Cooldowns, Effect, EffectKinds, EffectWorld, FromArgs, Grants, Known,
        Landing, Pools, Use,
    };
    pub use crate::combat::{
        Armor, Attack, CombatPlugin, CombatRng, CombatRules, DamageDealt, DamageEvent, DamageStages, Dead, DeathEvent, Faction, Health, MeleeAttack,
        RangedAttack, Resists, Strikes, line_of_fire,
    };
    pub use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
    pub use crate::effects::AddEngineEffects;
    pub use crate::events::{Counters, FactsPlugin, Happened, QuestChange, Quests};
    pub use crate::fov::FovPlugin;
    pub use crate::items::{
        DropItem, Enchant, Equip, Equipped, Inventory, Item, ItemEvent, ItemsPlugin, PickUp, Slots, Stack, Tagged, Unequip, UseItem, Wearable,
    };
    pub use crate::knowledge::Knowledge;
    pub use crate::lighting::{DarkSight, Fuel, LightEvent, LightSource, Lighting, LightingPlugin};
    pub use crate::minds::{Mind, MindChose, MindsPlugin, Perception, Profile};
    pub use crate::places::{
        Arrive, Destination, GoThrough, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules, PlaceRulesRes, Spot, Transition, WarpRequest,
    };
    pub use crate::plugin::{CleanupSet, CorePlugin, DecideSet, EngineSet, Needs, PresentSet, ResolveSet, Turn, TurnSet, depends_on};
    pub use crate::seed::{AddStream, Seed};
    pub use crate::state::EngineState;
    pub use crate::status::{Afflict, Afflicted, Cure, StatBlock, StatRules, StatusEvent, StatusPlugin, StatusRules};
    pub use crate::stealth::{Aware, Notice, Noticed, Stealth, StealthPlugin, Watchers};
    pub use crate::turn::{Acting, Action, ActionDone, ActionRefused, AddAction, Intent, Occupancy, Resolution, Step, TurnEnd, Turns, Wait};
    pub use crate::world::{ChunkLoaded, ChunkRulesRes, StreamingPlugin, WorldMap, WorldRes, WorldSettings};
}
