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
pub mod bump;
pub mod combat;
pub mod components;
pub mod consumable;
pub mod cue;
pub mod doors;
pub mod effects;
pub mod events;
pub mod fields;
pub mod fire;
pub mod fov;
pub mod gas;
pub mod items;
pub mod knowledge;
pub mod lighting;
pub mod minds;
pub mod noise;
pub mod places;
pub mod plugin;
pub mod props;
pub mod registries;
pub mod remains;
pub mod replay;
pub mod seed;
pub mod state;
pub mod status;
pub mod stealth;
pub mod testing;
pub mod throwing;
pub mod turn;
pub mod world;

pub use ability::{Abilities, AbilitiesPlugin, AbilityEvent, Aimed, Bystanders, Cooldowns, Grants, Known, Landed, Offered, Pools, Use};
pub use bump::{Bump, BumpRules, Bumped, OnAlly, Swap, Swapped};
pub use combat::{
    Armor, Attack, CombatPlugin, CombatRng, CombatRules, DamageDealt, DamageEvent, DamageStages, Dead, DeathEvent, Faction, Health, Invulnerable, Loadout,
    MeleeAttack, RangedAttack, Reach, Resists, Strikes, Struck, line_of_fire, shot,
};
pub use components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
pub use consumable::{AddSpending, Consumable, ConsumablesPlugin, Recharge, SpendingMoments, Spent, WhenEmpty, remove_spent};
pub use cue::{AddAirborne, Airborne, Anchor, Cue, Cued, Lands, LookOf, TurnHold};
pub use doors::{Close, DoorEvent, Open};
pub use effects::{
    AddEffect, AddEngineEffects, AddMoment, Cleanse, Effect, EffectKinds, EffectRng, EffectWorld, Effects, EffectsPlugin, Emit, Fired, FromArgs, Harm, Ignite,
    Inflict, Landing, LandsAsItself, Mend, Moment, MomentId, Moments, Pull, Remnant, Shove, Source, Teleport, Trigger, Triggers, area_cells, land_triggers,
    report_remnants,
};
pub use events::{Counters, FactsPlugin, Happened, QuestChange, Quests};
pub use fields::{MapFields, SavedField};
pub use fire::{Burning, FIRE_GLOW, Fire, FireEvent, FirePlugin, FireRules, Flammable, Kindle};
pub use fov::FovPlugin;
pub use gas::{Breathed, GasPlugin, Gases, Release, Vents};
pub use items::{
    Bestows, DropItem, EQUIP_FROM_GROUND_COST, Enchant, Equip, EquipFromGround, Equipped, GearScore, Inventory, Item, ItemEvent, ItemsPlugin, PickUp, Stack,
    Tagged, Unequip, UseItem, Wearable,
};
pub use knowledge::{Knowledge, KnowledgeSave};
pub use lighting::{DarkSight, Fuel, LightEvent, LightSource, Lighting, LightingPlugin};
pub use minds::{
    AddChoice, CameFrom, DEFAULT_PERCEPTION, FlowFields, Intelligence, Mind, MindChose, MindRng, MindsPlugin, Perception, Profile, Sight, Thinking,
    a_mind_holds_the_turn,
};
pub use noise::{AddSound, Earshot, Footfall, Heard, Hearing, MakeNoise, NoiseHeard, NoisePlugin, NoiseRules, NoiseRunning, Sound, SoundId, Sounds};
pub use places::{
    Arrive, Destination, GoThrough, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules, PlaceRulesRes, Spot, Transition, WarpRequest,
};
pub use plugin::{
    CleanupSet, CorePlugin, DecideSet, EndRun, EngineSet, FieldSet, LandSet, Needs, NewRun, PerceiveSet, PresentSet, Reads, Requirements, ResetsOnNewRun,
    ResolveSet, RunResets, Turn, TurnSet, clear_run, depends_on,
};
pub use props::{
    AddVerb, Container, Emptied, FillContainer, Hidden, Interact, Interacted, Offer, OfferedHere, PendingFires, Prop, PropEffects, PropKind, PropRng, PropSet,
    PropsPlugin, Refused, Spotted, Stocked, Take, Verb, VerbId, Verbs, spawn_prop,
};
pub use registries::Registries;
pub use remains::{LeavesRemains, Remains, RemainsLeft, RemainsNaming, RemainsPlugin, WasLiving};
pub use replay::{Pressed, Recording};
pub use seed::{AddStream, Seed, Stream};
pub use state::{Ending, EngineState, Outcome, Restart, RunOver, world_is_shown};
pub use status::{Afflict, Afflicted, Cure, StatBlock, StatusEvent, StatusPlugin};
pub use stealth::{Aware, Notice, Noticed, Stealth, StealthPlugin, StealthRng, StealthRunning, Watchers};
pub use throwing::{Flight, Throw, Throwable, ThrowingPlugin, flight};
pub use turn::{Acting, Action, ActionDone, ActionRefused, AddAction, Intent, Occupancy, Resolution, Step, Stepped, TurnEnd, Turns, Wait};
pub use world::{ChunkLoaded, ChunkRulesRes, PlaceMap, PlaceSave, StreamingPlugin, WindowView, WorldMap, WorldMapSave, WorldRes, WorldSettings};

/// The names a game writes.
///
/// What a game spawns, sends, reads and registers, and the traits whose
/// methods it calls. Not what other engine crates build on, such as
/// `Bystanders` and `Offered`, nor the effects a game names only in RON:
/// those stay at the crate root, and an effect in the prelude was a name
/// that collided with a game's own `Shove` action.
pub mod prelude {
    pub use crate::ability::{Abilities, AbilitiesPlugin, AbilityEvent, Cooldowns, Grants, Known, Pools, Use};
    pub use crate::bump::{Bump, BumpRules, Bumped, OnAlly, Swap, Swapped};
    pub use crate::combat::{
        Armor, Attack, CombatPlugin, CombatRules, DamageDealt, DamageEvent, DamageStages, Dead, DeathEvent, Faction, Health, Invulnerable, Loadout,
        MeleeAttack, RangedAttack, Reach, Resists, Strikes, Struck, line_of_fire, shot,
    };
    pub use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
    pub use crate::consumable::{AddSpending, Consumable, ConsumablesPlugin, WhenEmpty};
    pub use crate::cue::{AddAirborne, Airborne, Anchor, Cue, Cued, Lands, LookOf, TurnHold};
    pub use crate::doors::{Close, DoorEvent, Open};
    pub use crate::effects::{
        AddEffect, AddEngineEffects, AddMoment, Effect, EffectKinds, EffectWorld, Effects, EffectsPlugin, Fired, FromArgs, Landing, Moments, Triggers,
    };
    pub use crate::events::{Counters, FactsPlugin, Happened, QuestChange, Quests};
    pub use crate::fire::{Burning, Fire, FireEvent, FirePlugin, FireRules, Flammable, Kindle};
    pub use crate::fov::FovPlugin;
    pub use crate::gas::{Breathed, GasPlugin, Gases, Release, Vents};
    pub use crate::items::{
        Bestows, DropItem, Enchant, Equip, EquipFromGround, Equipped, GearScore, Inventory, Item, ItemEvent, ItemsPlugin, PickUp, Stack, Tagged, Unequip,
        UseItem, Wearable,
    };
    pub use crate::knowledge::Knowledge;
    pub use crate::lighting::{DarkSight, Fuel, LightEvent, LightSource, Lighting, LightingPlugin};
    pub use crate::minds::{AddChoice, Intelligence, Mind, MindChose, MindsPlugin, Perception, Profile, Thinking};
    pub use crate::noise::{AddSound, Footfall, Heard, Hearing, MakeNoise, NoiseHeard, NoisePlugin, NoiseRules, SoundId, Sounds};
    pub use crate::places::{
        Arrive, Destination, GoThrough, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules, PlaceRulesRes, Spot, Transition, WarpRequest,
    };
    pub use crate::plugin::{
        CleanupSet, CorePlugin, DecideSet, EndRun, EngineSet, FieldSet, LandSet, Needs, NewRun, PerceiveSet, PresentSet, Reads, ResetsOnNewRun, ResolveSet,
        Turn, TurnSet, depends_on,
    };
    pub use crate::props::{
        AddVerb, Container, Emptied, FillContainer, Hidden, Interact, Interacted, Offer, OfferedHere, Prop, PropKind, PropSet, PropsPlugin, Refused, Spotted,
        Take, VerbId, Verbs, spawn_prop,
    };
    pub use crate::registries::Registries;
    pub use crate::remains::{LeavesRemains, Remains, RemainsLeft, RemainsNaming, RemainsPlugin};
    pub use crate::seed::{AddStream, Seed};
    pub use crate::state::{Ending, EngineState, Outcome, Restart, RunOver};
    pub use crate::status::{Afflict, Afflicted, Cure, StatBlock, StatusEvent, StatusPlugin};
    pub use crate::stealth::{Aware, Notice, Noticed, Stealth, StealthPlugin, Watchers};
    pub use crate::throwing::{Throw, Throwable, ThrowingPlugin};
    pub use crate::turn::{Acting, Action, ActionDone, ActionRefused, AddAction, Intent, Occupancy, Resolution, Step, Stepped, TurnEnd, Turns, Wait};
    pub use crate::world::{ChunkLoaded, ChunkRulesRes, StreamingPlugin, WorldMap, WorldRes, WorldSettings};
}
