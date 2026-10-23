//! The Bevy layer: the loops the engine owns.
//!
//! Part of `atglyph`: a game depends on `atglyph` and reaches this crate as `atglyph::ecs`.
//!
//! Everything below tier 2 is data and algorithms. This crate is where they
//! run: the turn loop, field of view, occupancy. A game
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
pub mod accuracy;
#[cfg(test)]
mod ambiguity;
pub mod ammo;
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
pub mod loot;
pub mod minds;
pub mod noise;
pub mod parts;
pub mod places;
pub mod plugin;
pub mod prefabs;
pub mod props;
pub mod records;
pub mod registries;
pub mod remains;
pub mod replay;
pub mod secrets;
pub mod seed;
pub mod settings;
pub mod state;
pub mod status;
pub mod stealth;
pub mod testing;
pub mod throwing;
pub mod title;
pub mod trade;
pub mod turn;
pub mod work;
pub mod world;

pub use ability::{
    Abilities, AbilitiesPlugin, AbilityEvent, Aimed, Bystanders, Cooldowns, Grants, Known, Landed, Pools, Reflected, Reflects, UsableAbilities, Use,
};
pub use accuracy::{Attempt, HitRules, Marksmanship, Missed};
pub use ammo::{Ammo, AmmoChanged, AmmoPlugin, AmmoSet, Dry};
pub use atglyph_grid::LightBand;
pub use bump::{Bump, BumpRules, Bumped, OnAlly, Swapped};
pub use combat::{
    Armor, Attack, AttackShape, CombatPlugin, CombatRng, CombatRules, DamageDealt, DamageEvent, DamageRoute, DamageStages, Dead, DeathEvent, Faction, Heal,
    Healed, Health, Invulnerable, Loadout, MeleeAttack, RangedAttack, Resists, Stowed, Strikes, Struck, line_of_fire, shot,
};
pub use components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
pub use consumable::{AddSpending, Attuned, Consumable, Recharge, SpendingMoments, Spent, WhenEmpty};
pub use cue::{AddAirborne, Airborne, Anchor, Cue, CueWait, Cued, Lands, LookOf};
pub use doors::{Close, DoorEvent, Open};
pub use effects::{
    AddEffect, AddEngineEffects, AddTriggerKind, Cleanse, Effect, EffectBonus, EffectKinds, EffectRng, EffectWorld, Effects, EffectsPlugin, Emit, Fired,
    FiredOn, FromArgs, Harm, Ignite, Inflict, KeepOnRemnant, Landing, LandsAsItself, Mend, Noise, Pull, Remnant, RemnantKeeps, Shove, Source, Trigger,
    TriggerKind, TriggerKindId, TriggerKinds, Triggers, area_cells,
};
pub use events::{Counters, FactSet, FactsPlugin, Happened, QuestChange, Quests};
pub use fields::SavedField;
pub use fire::{Burning, Fire, FireEvent, FirePlugin, FireRules, Flammable, Kindle};
pub use fov::FovPlugin;
pub use gas::{Breathed, GasPlugin, Gases, Release, Vents};
pub use items::{
    Bestows, DropItem, Equip, EquipFromGround, Equipped, GearScore, Inventory, Item, ItemEvent, ItemRules, ItemsPlugin, LitWhenWorn, OwnBlowScore, PickUp,
    Pulse, Stack, Stuck, Tagged, Unequip, Unknown, UseItem, UseItemAt, UseOn, Wearable, WornStatuses, may_use, may_use_on,
};
pub use knowledge::{Knowledge, KnowledgeSave, Recalled};
pub use lighting::{
    Concealed, DarkSight, DarkSightSuppressed, Fuel, LightEvent, LightSource, Lighting, LightingPlugin, OwnDarkSight, PlayerSight, WornDarkSight,
};
pub use loot::{Drops, Found, ItemMaker, LootArea, LootPlugin, LootRng, LootSet, Provenance};
pub use minds::{
    AddChoice, CameFrom, DEFAULT_PERCEPTION, Doing, FlowFields, Intelligence, JoinStamp, Leader, Mind, MindChose, MindsPlugin, Perception, Post, Profile,
    Sight, Thinking,
};
pub use noise::{AddSound, Footfall, Heard, Hearing, MakeNoise, NoiseHeard, NoisePlugin, NoiseRules, NoiseRunning, Sound, SoundId, SoundNames, Sounds};
pub use parts::{ActorParts, Attacks, ItemLevel, ItemParts, LightWhen, innate_statuses, insert_parts};
pub use places::{
    AddPlaces, Arrive, Climb, Destination, GoThrough, Ladder, LadderEnd, Lock, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules, PlaceRulesRes,
    SpawnPlayer, Spot, Stair, StairLook, Transition, Travelled, WarpRequest,
};
pub use plugin::{
    CleanupSet, CorePlugin, DecideSet, EndOfFrame, EndRun, EngineSet, FieldSet, LandSet, Needs, NewRun, PerceiveSet, PresentSet, ReactSet, Reads,
    ResetsOnNewRun, ResolveSet, Turn, TurnSet, clear_run, depends_on,
};
pub use prefabs::{ActorMaker, PrefabPlugin, PrefabSet, Prefabs};
pub use props::{
    AddVerb, Among, AsksFirst, Chooser, Container, Emptied, FillContainer, Finding, Hidden, Interact, Interacted, Keen, Offer, OfferedHere, Offering, Offers,
    PendingFires, Prop, PropEffects, PropKind, PropRng, PropSet, PropsPlugin, Refused, Scenery, SpotRules, Spotted, Stocked, Take, Verb, VerbId, Verbs,
    spawn_prop,
};
pub use records::{RecordOutcome, RecordRun, RecordSection, RecordsPlugin, RunRecord, RunRecords};
pub use registries::Registries;
pub use remains::{LeavesRemains, Life, Remains, RemainsLeft, RemainsNaming, RemainsPlugin, RevivalRules, ReviveCommands, Revived, revive};
pub use replay::{Pressed, Recording};
pub use secrets::Secret;
pub use seed::{AddStream, ENGINE_DOMAINS, RunRng, Seed, Stream, StreamDomain};
pub use settings::{AddSettings, Setting, SettingId, Settings};
pub use state::{Ending, EngineState, Outcome, Restart, RunOver, world_is_shown};
pub use status::{ActiveStatuses, ApplyStatus, RemoveStatus, SideWas, StatBlock, StatusEvent};
pub use stealth::{Aware, Notice, Noticed, Stealth, StealthPlugin, Unseen, Watchers};
pub use throwing::{Flight, Throw, Throwable, ThrowingPlugin, flight};
pub use title::{AbandonRun, ContinueRun, HoldStart, LeaveRun, SavedRun, begin_run};
pub use trade::{Buy, Currency, Deal, Parties, Sell, TRADE, TradePlugin, TradeRefusal, Traded, Trader, Trading, Ware, Worth, pay};
pub use turn::{Acting, Action, ActionDone, ActionRefused, AddAction, Intent, Occupancy, Resolution, Speeds, Step, Stepped, TurnEnd, TurnRules, Turns, Wait};
pub use work::{AddWork, BeginWork, BreakReason, WorkBegan, WorkBroken, WorkDone, WorkKinds, WorkPace, Working, Works};
pub use world::{PlaceMap, PlaceSave, WindowView, WorldMap, WorldMapSave};

/// The names a game writes.
///
/// What a game spawns, sends, reads and registers, and the traits whose
/// methods it calls. Not what other engine crates build on, such as
/// `Bystanders` and `UsableAbilities`, nor the effects a game names only in RON:
/// those stay at the crate root, and an effect in the prelude was a name
/// that collided with a game's own `Shove` action.
pub mod prelude {
    pub use crate::ability::{Abilities, AbilitiesPlugin, AbilityEvent, Cooldowns, Grants, Known, Pools, Reflected, Reflects, Use};
    pub use crate::accuracy::{Attempt, HitRules, Marksmanship, Missed};
    pub use crate::ammo::{Ammo, AmmoChanged, AmmoPlugin, AmmoSet, Dry};
    pub use crate::bump::{Bump, BumpRules, Bumped, OnAlly, Swapped};

    pub use crate::combat::{
        Armor, Attack, AttackShape, CombatPlugin, CombatRules, DamageDealt, DamageEvent, DamageRoute, DamageStages, Dead, DeathEvent, Faction, Heal, Healed,
        Health, Invulnerable, Loadout, MeleeAttack, RangedAttack, Resists, Stowed, Strikes, Struck, line_of_fire, shot,
    };
    pub use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
    pub use crate::consumable::{AddSpending, Attuned, Consumable, WhenEmpty};
    pub use crate::cue::{AddAirborne, Airborne, Anchor, Cue, CueWait, Cued, Lands, LookOf};
    pub use crate::doors::{Close, DoorEvent, Open};

    pub use crate::effects::{
        AddEffect, AddEngineEffects, AddTriggerKind, Effect, EffectBonus, EffectKinds, EffectWorld, Effects, EffectsPlugin, Fired, FromArgs, Landing,
        TriggerKinds, Triggers,
    };
    pub use crate::events::{Counters, FactSet, FactsPlugin, Happened, QuestChange, Quests};
    pub use crate::fire::{Burning, Fire, FireEvent, FirePlugin, FireRules, Flammable, Kindle};

    pub use crate::fov::FovPlugin;
    pub use crate::gas::{Breathed, GasPlugin, Gases, Release, Vents};

    pub use crate::items::{
        Bestows, DropItem, Equip, EquipFromGround, Equipped, GearScore, Inventory, Item, ItemEvent, ItemRules, ItemsPlugin, LitWhenWorn, OwnBlowScore, PickUp,
        Pulse, Stack, Stuck, Tagged, Unequip, Unknown, UseItem, UseItemAt, UseOn, Wearable, WornStatuses,
    };

    pub use crate::knowledge::Knowledge;
    pub use crate::lighting::{
        Concealed, DarkSight, DarkSightSuppressed, Fuel, LightEvent, LightSource, Lighting, LightingPlugin, OwnDarkSight, PlayerSight, WornDarkSight,
    };

    pub use crate::loot::{Drops, Found, ItemMaker, LootArea, LootPlugin, LootSet, Provenance};
    pub use crate::minds::{AddChoice, Doing, Intelligence, JoinStamp, Leader, Mind, MindChose, MindsPlugin, Perception, Post, Profile, Thinking};

    pub use crate::noise::{AddSound, Footfall, Heard, Hearing, MakeNoise, NoiseHeard, NoisePlugin, NoiseRules, SoundId, SoundNames, Sounds};
    pub use crate::parts::{ActorParts, Attacks, ItemLevel, ItemParts, LightWhen, innate_statuses, insert_parts};

    pub use crate::places::{
        AddPlaces, Arrive, Climb, Destination, GoThrough, Ladder, LadderEnd, Lock, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules,
        PlaceRulesRes, SpawnPlayer, Spot, Stair, StairLook, Transition, Travelled, WarpRequest,
    };
    pub use crate::plugin::{
        CleanupSet, CorePlugin, DecideSet, EndOfFrame, EndRun, EngineSet, FieldSet, LandSet, Needs, NewRun, PerceiveSet, PresentSet, ReactSet, Reads,
        ResetsOnNewRun, ResolveSet, Turn, TurnSet, depends_on,
    };
    pub use crate::prefabs::{ActorMaker, PrefabPlugin, PrefabSet, Prefabs};
    pub use crate::props::{
        AddVerb, Among, AsksFirst, Chooser, Container, Emptied, FillContainer, Finding, Hidden, Interact, Interacted, Keen, Offer, OfferedHere, Offering,
        Offers, Prop, PropKind, PropSet, PropsPlugin, Refused, Scenery, SpotRules, Spotted, Take, VerbId, Verbs, spawn_prop,
    };
    pub use crate::records::{RecordOutcome, RecordRun, RecordSection, RecordsPlugin, RunRecord, RunRecords};

    pub use crate::registries::Registries;
    pub use crate::remains::{LeavesRemains, Remains, RemainsLeft, RemainsNaming, RemainsPlugin, RevivalRules, ReviveCommands, Revived};

    pub use crate::seed::{AddStream, Seed};
    pub use crate::settings::{AddSettings, Setting, SettingId, Settings};
    pub use crate::state::{Ending, EngineState, Outcome, Restart, RunOver};
    pub use crate::status::{ActiveStatuses, ApplyStatus, RemoveStatus, SideWas, StatBlock, StatusEvent};

    pub use crate::stealth::{Aware, Notice, Noticed, Stealth, StealthPlugin, Unseen, Watchers};
    pub use crate::throwing::{Throw, Throwable, ThrowingPlugin};
    pub use crate::title::{AbandonRun, ContinueRun, HoldStart, LeaveRun, SavedRun, begin_run};
    pub use crate::trade::{Buy, Currency, Deal, Parties, Sell, TRADE, TradePlugin, TradeRefusal, Traded, Trader, Trading, Ware, Worth};

    pub use crate::turn::{
        Acting, Action, ActionDone, ActionRefused, AddAction, Intent, Occupancy, Resolution, Speeds, Step, Stepped, TurnEnd, TurnRules, Turns, Wait,
    };

    pub use crate::work::{AddWork, WorkDone, WorkKinds, WorkPace, Working};

    pub use crate::world::WorldMap;
    pub use atglyph_grid::LightBand;
}
