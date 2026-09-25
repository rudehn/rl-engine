//! The rules layer: shapes in the engine, vocabulary in the game.
//!
//! Nothing here names a stat, a damage type, a status, a faction, an
//! equipment slot, an item tag, an affix, a fact or a quest. The engine
//! ships the machinery; a game registers what exists and the numbers that
//! go with it.
//!
//! - [`content`]: registries and banded tables loaded from RON, which hand
//!   out the ids everything else is keyed by.
//! - [`stats`], [`damage`], [`status`], [`faction`], [`equip`] and
//!   [`affix`]: the modifier accumulator, the mitigation pipeline, the tick
//!   and expiry machinery, the relation matrix, the slot graph and the
//!   affix and enchant model.
//! - [`gas`] and [`fire`]: the rules a turn steps a gas and a fire by over a
//!   tile field; which gases there are is content, and what burns is the
//!   caller's to say.
//! - [`ability`]: what an actor can spend a turn on besides a step and a
//!   swing, as data: a shape, costs, requirements and a list of effects
//!   the layer above resolves. [`EffectSpec`] is one effect as any content
//!   file writes it, and [`TriggerSpec`] with its [`Area`] one trigger, the
//!   moment a prop or a thing answers and where its effects land.
//! - [`loot`]: where items turn up and how often, keyed by a game's own
//!   item ids: a banded table drawn by weight, by group and by tag, what
//!   the dead leave, and a place's scatter.
//! - [`prop`]: what stands on a map that is neither an actor nor an item,
//!   as data: a look, what it offers, what it holds, what sets it off and
//!   how hard it is to spot.
//! - [`role`]: a name and the monsters that fit it, drawn from a game's
//!   spawn table at a band, for a prefab's monster slot to ask for.
//! - [`prefab`]: one prefab, read from rows of glyphs and a legend naming a
//!   tile or a slot, a prop, an item row, a monster or a mark, every name
//!   resolved at load; the terrain and the stamp are `rl-mapgen`'s.
//! - [`ai`]: tactic-priority brains over Dijkstra maps.
//! - [`events`]: facts, counters and quests as data over what happened.
//! - [`balance`]: threat scoring and the spawn-band report, so content is
//!   checked from the command line.
//! - [`forecast`]: what a fight is likely to cost, run through the same
//!   mitigation a real blow goes through, for an inspect panel to print.
//!
//! One crate rather than five, because crate boundaries are drawn on
//! dependency weight and these all weigh the same: core, grid, serde and
//! ron. The modules keep the seams the crates had, and every item is also
//! re-exported at the root.
//!
//! Everything is pure. The Bevy layer turns these into components and
//! systems; a balance tool or a test runs them on plain values.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod ability;
pub mod affix;
pub mod ai;
pub mod balance;
pub mod content;
pub mod damage;
pub mod equip;
pub mod events;
pub mod faction;
pub mod fire;
pub mod forecast;
pub mod gas;
pub mod loot;
pub mod names;
pub mod prefab;
pub mod prop;
pub mod role;
pub mod stats;
pub mod status;

pub use ability::{AbilityDef, AbilityId, Aim, Area, Blocked, Cost, EffectSpec, Gates, Purse, Requirement, TriggerSpec, blocked, read_args};
pub use affix::{AffixDef, AffixId, AffixKind, Enchanted, EnhanceRule, Scaled, ScaledStrike, TagDef, TagId, roll_affixes};
pub use ai::{
    ActorView, Awareness, Brain, Choice, Decision, Fields, HearingStats, ItemView, Missile, MovementProfile, NoFields, NoticeStats, Sense, Snapshot,
    StealthStats, Tactic, TacticCtx, Vitals, Wits, notices,
};
pub use balance::{BandRow, Report, ThreatSubject, threat};
pub use content::{BandedEntry, BandedTable, ContentError, Named, Registry};
pub use damage::{DamageKind, DamageStage, Hit, Resistances, resolve};
pub use equip::{EquipError, EquipShape, Equipment, SlotDef, SlotId};
pub use events::{Change, CounterDef, CounterId, Fact, FactDef, FactKind, Ledger, Matcher, Need, Objective, QuestDef, QuestId, QuestState, Tally, Tracker};
pub use faction::{FactionId, Factions, Relation};
pub use fire::Tinder;
pub use forecast::{Combatant, Duel, Outlook, blows_to_fell, duel, expected_damage, turns_for};
pub use gas::{Breath, GasDef, GasId};
pub use loot::{DropRow, DropTable, Layout, LootRow, LootTable, Placed, ScatterRules, plan_scatter};
pub use names::{NameRef, Names};
pub use prefab::{Pick, PrefabDef, Slot};
pub use prop::{ContainerDef, ContentRoll, HiddenDef, OfferDef, PropDef, PropId, Stock};
pub use role::{RoleDef, RoleId};
pub use stats::{Modifier, Op, Source, StatDef, StatId, Stats};
pub use status::{ActiveStatus, Stacking, StatusDef, StatusId, Statuses, Tick, TickReport};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::ability::{AbilityDef, AbilityId, Aim, Area, Blocked, Cost, EffectSpec, Gates, Purse, Requirement, TriggerSpec, blocked, read_args};
    pub use crate::affix::{AffixDef, AffixId, AffixKind, Enchanted, EnhanceRule, Scaled, ScaledStrike, TagDef, TagId, roll_affixes};
    pub use crate::ai::tactics;
    pub use crate::ai::{
        ActorView, Awareness, Brain, Choice, Decision, Fields, HearingStats, ItemView, Missile, MovementProfile, NoFields, NoticeStats, Sense, Snapshot,
        StealthStats, Tactic, TacticCtx, Vitals, Wits, notices,
    };
    pub use crate::balance::{BandRow, Report, ThreatSubject, threat};
    pub use crate::content::{BandedEntry, BandedTable, ContentError, Named, Registry};
    pub use crate::damage::{DamageKind, DamageStage, Hit, Resistances, resolve};
    pub use crate::equip::{EquipError, EquipShape, Equipment, SlotDef, SlotId};
    pub use crate::events::{
        Change, CounterDef, CounterId, Fact, FactDef, FactKind, Ledger, Matcher, Need, Objective, QuestDef, QuestId, QuestState, Tally, Tracker,
    };
    pub use crate::faction::{FactionId, Factions, Relation};
    pub use crate::fire::Tinder;
    pub use crate::forecast::{Combatant, Duel, Outlook, blows_to_fell, duel, expected_damage, turns_for};
    pub use crate::gas::{Breath, GasDef, GasId};
    pub use crate::loot::{DropRow, DropTable, Layout, LootRow, LootTable, Placed, ScatterRules, plan_scatter};
    pub use crate::names::{NameRef, Names};
    pub use crate::prop::{ContainerDef, ContentRoll, HiddenDef, OfferDef, PropDef, PropId, Stock};
    pub use crate::stats::{Modifier, Op, Source, StatDef, StatId, Stats};
    pub use crate::status::{ActiveStatus, Stacking, StatusDef, StatusId, Statuses, Tick, TickReport};
}
