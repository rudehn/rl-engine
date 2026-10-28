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
//!
//! - [`plugin`] and [`turn`]: the wiring and the loop, [`CorePlugin`]
//!   with every set and schedule a game's systems go in, and the turn
//!   itself, its actions, intents, clock and occupancy.
//! - [`app`]: [`EngineApp`], every method a game and a plugin call on the
//!   `App` while it is put together.
//! - [`components`]: what every actor is made of, its position, its sight
//!   and its speed.
//! - [`state`], [`title`], [`seed`] and [`replay`]: a run's life, how it
//!   ends and begins again, its start held for a title, the seed every
//!   stream derives from, and a run written down and played back.
//! - [`world`], [`places`], [`knowledge`] and [`fov`]: the map every reader
//!   reads, the places it is made of and travel between them, what the
//!   player has seen, and who sees what.
//! - [`lighting`], [`doors`], [`secrets`], [`props`] and [`work`]: what is
//!   on a map besides its actors and items, and what an actor does there
//!   across many turns.
//! - [`fields`], [`fire`] and [`gas`]: what spreads over the map, stepped
//!   by the rules in `atglyph-rules`.
//! - [`combat`], [`accuracy`], [`bump`], [`status`], [`ability`],
//!   [`effects`], [`throwing`] and [`ammo`]: fighting, from a blow to an
//!   ability's effects landing where it was aimed.
//! - [`items`], [`consumable`], [`loot`], [`parts`], [`prefabs`] and
//!   [`registries`]: things, what a use costs them, where they turn up,
//!   the authored parts an actor or an item is made from, and the
//!   registries those are read against.
//! - [`minds`], [`stealth`] and [`noise`]: what decides everyone else's
//!   turn, who has noticed whom, and what was heard.
//! - [`cue`] and [`settings`]: what a turn did that is worth showing, with
//!   the wait while it is shown, and what a player chooses once.
//! - [`events`], [`records`], [`remains`] and [`trade`]: facts and quests,
//!   what a run comes to, what the dead leave, and trade between two
//!   actors.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod ability;
pub mod accuracy;
#[cfg(test)]
mod ambiguity;
pub mod ammo;
pub mod app;
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
#[doc(hidden)]
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
pub use app::EngineApp;
pub use atglyph_grid::LightBand;
pub use bump::{Bump, BumpRules, Bumped, OnAlly, Swapped};
pub use combat::{
    Armor, Attack, AttackShape, BenchedAttack, BonusDamage, CombatPlugin, CombatRng, CombatRules, DamageDealt, DamageEvent, DamageRoute, DamageStages, Dead,
    DeathEvent, Faction, Heal, Healed, Health, Invulnerable, Loadout, MeleeAttack, RangedAttack, Resists, Struck, ThrowReach, line_of_fire, shot,
};
pub use components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
pub use consumable::{Attuned, Consumable, Recharge, SpendingMoments, Spent, WhenEmpty};
pub use cue::{Airborne, Anchor, Cue, CueWait, Cued, Lands, LookOf};
pub use doors::{Close, DoorEvent, Open};
pub use effects::{
    Cleanse, Effect, EffectBonus, EffectKinds, EffectRng, EffectWorld, Effects, EffectsPlugin, Emit, Fired, FiredOn, FromArgs, Harm, Ignite, Inflict, Landing,
    LandsAsItself, Mend, Noise, Pull, Remnant, RemnantKeeps, Shove, Source, Trigger, TriggerKind, TriggerKindId, TriggerKinds, Triggers, area_cells,
};
pub use events::{Counters, FactSet, FactsPlugin, Happened, QuestChange, Quests};
pub use fields::SavedField;
pub use fire::{Burning, Fire, FireEvent, FirePlugin, FireRules, Flammable, Kindle};
pub use fov::FovPlugin;
pub use gas::{Breathed, GasPlugin, Gases, Release, Vents};
pub use items::{
    Bestows, DropItem, Equip, EquipFromGround, Equipped, GearScore, Inventory, Item, ItemEvent, ItemRules, ItemsPlugin, LitWhenWorn, OwnBlowScore, PickUp,
    Stack, StackLimit, Stuck, Tagged, Unequip, Unknown, UseItem, UseItemAt, UseOn, Wearable, WornStatuses, WornTimer, may_use, may_use_on,
};
pub use knowledge::{Knowledge, KnowledgeSave, Recalled};
pub use lighting::{
    Concealed, DarkSight, DarkSightSuppressed, Fuel, LightEvent, LightSource, Lighting, LightingPlugin, OwnDarkSight, PlayerSight, WornDarkSight,
};
pub use loot::{Drops, Found, ItemMaker, LootArea, LootPlugin, LootRng, LootSet, Provenance};
pub use minds::{
    CameFrom, DEFAULT_PERCEPTION, FlowFields, Intelligence, JoinStamp, LastTactic, Leader, Mind, MindChose, MindsPlugin, Perception, Post, Profile, Sight,
    Thinking,
};
pub use noise::{Footfall, Heard, Hearing, MakeNoise, NoiseHeard, NoisePlugin, NoiseRules, NoiseRunning, Sound, SoundId, SoundNames, Sounds};
pub use parts::{ActorParts, Attacks, ItemLevel, ItemParts, LightWhen, innate_statuses, insert_parts};
pub use places::{
    Arrive, Climb, Destination, GoThrough, Ladder, LadderEnd, LadderStair, Lock, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules, PlaceRulesRes,
    SpawnPlayer, Spot, Stair, StairLook, Transition, Travelled, WarpRequest,
};
pub use plugin::{
    CleanupSet, ContinueSaved, CorePlugin, DecideSet, EndOfFrame, EndRun, EngineSet, FieldSet, LandSet, NewRun, PerceiveSet, PrepareRun, PresentSet, ReactSet,
    ResolveSet, RunResumed, Turn, TurnSet, clear_run,
};
pub use prefabs::{ActorMaker, PrefabPlugin, PrefabSet, Prefabs};
pub use props::{
    AsksFirst, Container, Emptied, FillContainer, Finding, Hidden, Interact, Interacted, Offer, OfferChooser, OfferFilter, OfferedHere, Offering, Offers,
    PendingFires, Prop, PropEffects, PropKind, PropRng, PropSet, PropsPlugin, Refused, Scenery, SpotBonus, SpotRules, Spotted, Stocked, Take, Verb, VerbId,
    Verbs, spawn_prop,
};
pub use records::{RecordOutcome, RecordRun, RecordSection, RecordsPlugin, RunRecord, RunRecords};
pub use registries::Registries;
pub use remains::{LeavesRemains, Remains, RemainsLeft, RemainsNaming, RemainsPlugin, RevivalRules, RevivalTwin, ReviveCommands, Revived, revive};
pub use replay::{Pressed, Recording};
pub use secrets::Secret;
pub use seed::{ENGINE_DOMAINS, LaunchArgs, RunRng, Seed, Stream, StreamDomain};
pub use settings::{Setting, SettingId, Settings};
pub use state::{Ending, EngineState, Outcome, Restart, RunOver, world_is_shown};
pub use status::{ActiveStatuses, ApplyStatus, RemoveStatus, SideWas, StatBlock, StatusEvent};
pub use stealth::{Aware, Notice, Noticed, Stealth, StealthPlugin, Unseen, Watchers};
pub use throwing::{Flight, Throw, ThrowAs, Throwable, ThrowingPlugin, Throws, flight};
pub use title::{AbandonRun, ContinueRun, HoldStart, LeaveRun, SavedRun, begin_afresh, begin_run};
pub use trade::{Buy, Currency, Deal, Parties, Sell, TRADE, TradePlugin, TradeRefusal, TradeRefused, Traded, Trader, Trading, Ware, Worth, pay};
pub use turn::{Acting, Action, ActionDone, ActionRefused, Intent, Occupancy, Resolution, Speeds, Step, Stepped, TurnEnd, TurnRules, Turns, Wait};
pub use work::{BeginWork, BreakReason, StopWork, WorkBegan, WorkBroken, WorkDone, WorkKinds, WorkPace, Working, Works};
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
    pub use crate::app::EngineApp;
    pub use crate::bump::{Bump, BumpRules, Bumped, OnAlly, Swapped};

    pub use crate::combat::{
        Armor, Attack, AttackShape, BenchedAttack, BonusDamage, CombatPlugin, CombatRules, DamageDealt, DamageEvent, DamageRoute, DamageStages, Dead,
        DeathEvent, Faction, Heal, Healed, Health, Invulnerable, Loadout, MeleeAttack, RangedAttack, Resists, Struck, line_of_fire, shot,
    };
    pub use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Speed, Viewshed};
    pub use crate::consumable::{Attuned, Consumable, WhenEmpty};
    pub use crate::cue::{Airborne, Anchor, Cue, CueWait, Cued, Lands, LookOf};
    pub use crate::doors::{Close, DoorEvent, Open};

    pub use crate::effects::{Effect, EffectBonus, EffectKinds, EffectWorld, Effects, EffectsPlugin, Fired, FromArgs, Landing, TriggerKinds, Triggers};
    pub use crate::events::{Counters, FactSet, FactsPlugin, Happened, QuestChange, Quests};
    pub use crate::fire::{Burning, Fire, FireEvent, FirePlugin, FireRules, Flammable, Kindle};

    pub use crate::fov::FovPlugin;
    pub use crate::gas::{Breathed, GasPlugin, Gases, Release, Vents};

    pub use crate::items::{
        Bestows, DropItem, Equip, EquipFromGround, Equipped, GearScore, Inventory, Item, ItemEvent, ItemRules, ItemsPlugin, LitWhenWorn, OwnBlowScore, PickUp,
        Stack, StackLimit, Stuck, Tagged, Unequip, Unknown, UseItem, UseItemAt, UseOn, Wearable, WornStatuses, WornTimer,
    };

    pub use crate::knowledge::Knowledge;
    pub use crate::lighting::{
        Concealed, DarkSight, DarkSightSuppressed, Fuel, LightEvent, LightSource, Lighting, LightingPlugin, OwnDarkSight, PlayerSight, WornDarkSight,
    };

    pub use crate::loot::{Drops, Found, ItemMaker, LootArea, LootPlugin, LootSet, Provenance};
    pub use crate::minds::{Intelligence, JoinStamp, LastTactic, Leader, Mind, MindChose, MindsPlugin, Perception, Post, Profile, Thinking};

    pub use crate::noise::{Footfall, Heard, Hearing, MakeNoise, NoiseHeard, NoisePlugin, NoiseRules, SoundId, SoundNames, Sounds};
    pub use crate::parts::{ActorParts, Attacks, ItemLevel, ItemParts, LightWhen, innate_statuses, insert_parts};

    pub use crate::places::{
        Arrive, Climb, Destination, GoThrough, Ladder, LadderEnd, Lock, MapChanged, MapId, OnMap, PlaceBuild, PlaceEntered, PlaceRules, PlaceRulesRes,
        SpawnPlayer, Spot, Stair, StairLook, Transition, Travelled, WarpRequest,
    };
    pub use crate::plugin::{
        CleanupSet, CorePlugin, DecideSet, EndOfFrame, EndRun, EngineSet, FieldSet, LandSet, NewRun, PerceiveSet, PrepareRun, PresentSet, ReactSet, ResolveSet,
        RunResumed, Turn, TurnSet,
    };
    pub use crate::prefabs::{ActorMaker, PrefabPlugin, PrefabSet, Prefabs};
    pub use crate::props::{
        AsksFirst, Container, Emptied, FillContainer, Finding, Hidden, Interact, Interacted, Offer, OfferChooser, OfferFilter, OfferedHere, Offering, Offers,
        Prop, PropKind, PropSet, PropsPlugin, Refused, Scenery, SpotBonus, SpotRules, Spotted, Take, VerbId, Verbs, spawn_prop,
    };
    pub use crate::records::{RecordOutcome, RecordRun, RecordSection, RecordsPlugin, RunRecord, RunRecords};

    pub use crate::registries::Registries;
    pub use crate::remains::{LeavesRemains, Remains, RemainsLeft, RemainsNaming, RemainsPlugin, RevivalRules, ReviveCommands, Revived};

    pub use crate::seed::{LaunchArgs, Seed};
    pub use crate::settings::{Setting, SettingId, Settings};
    pub use crate::state::{Ending, EngineState, Outcome, Restart, RunOver};
    pub use crate::status::{ActiveStatuses, ApplyStatus, RemoveStatus, SideWas, StatBlock, StatusEvent};

    pub use crate::stealth::{Aware, Notice, Noticed, Stealth, StealthPlugin, Unseen, Watchers};
    pub use crate::throwing::{Throw, ThrowAs, Throwable, ThrowingPlugin, Throws};
    pub use crate::title::{AbandonRun, ContinueRun, HoldStart, LeaveRun, SavedRun, begin_run};
    pub use crate::trade::{Buy, Currency, Deal, Parties, Sell, TRADE, TradePlugin, TradeRefusal, TradeRefused, Traded, Trader, Trading, Ware, Worth};

    pub use crate::turn::{Acting, Action, ActionDone, ActionRefused, Intent, Occupancy, Resolution, Speeds, Step, Stepped, TurnEnd, TurnRules, Turns, Wait};

    pub use crate::work::{WorkDone, WorkKinds, WorkPace, Working};

    pub use crate::world::WorldMap;
    pub use atglyph_grid::LightBand;
}
