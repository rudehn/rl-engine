//! The rules layer: shapes in the engine, vocabulary in the game.
//!
//! Use this crate alone, or through `atglyph` as `atglyph::rules`.
//! It holds the rules a roguelike runs on, with the vocabulary left to the game, builds with no Bevy, and builds for `wasm32-unknown-unknown`,
//! so a tool, a server or a test can depend on `atglyph-rules` and skip compiling Bevy.
//!
//! ```
//! use atglyph_rules::{Named, Registry};
//!
//! struct Def {
//!     name: &'static str,
//!     hp: u32,
//! }
//!
//! impl Named for Def {
//!     fn name(&self) -> &str {
//!         self.name
//!     }
//! }
//!
//! // Definitions in order; each gets a dense id, and a name finds it.
//! let defs = Registry::from_defs(vec![Def { name: "small", hp: 3 }, Def { name: "large", hp: 12 }]).unwrap();
//! let large = defs.expect("large");
//! assert_eq!(defs.get(large).hp, 12);
//! assert_eq!(defs.name(large), "large");
//! assert!(defs.id("missing").is_none());
//! ```
//!
//! Nothing here names a stat, a damage type, a status, a faction, an
//! equipment slot, an item tag, a fact or a quest. The engine ships the
//! machinery; a game registers what exists and the numbers that go with
//! it.
//!
//! - [`content`]: registries and banded tables loaded from RON, which hand
//!   out the ids everything else is keyed by.
//! - [`stats`], [`damage`], [`status`], [`faction`], [`equip`] and
//!   [`tag`]: the modifier accumulator, the mitigation pipeline, the tick
//!   and expiry machinery, the relation matrix, the slot graph and its
//!   pairs, and what a thing counts as and who carries it.
//! - [`gas`] and [`fire`]: the rules a turn steps a gas and a fire by over a
//!   tile field; which gases there are is content, and what burns is the
//!   caller's to say.
//! - [`ability`]: what an actor can spend a turn on besides a step and a
//!   swing, as data: a shape, costs, requirements and a list of effects
//!   the layer above resolves. [`EffectSpec`] is one effect as any content
//!   file writes it, and [`TriggerSpec`] with its [`Area`] one trigger, the
//!   moment a prop or a thing answers and where its effects land.
//!   [`Targeting`] is how a use is pointed, an ability's or a thing's own
//!   aimed use, so both are refused by the one rule, and [`Asks`] what a
//!   thing's use asks for when it is used on another of its user's things.
//! - [`parts`]: what a content file writes for something the engine
//!   already simulates, a blow, a shot, a throw, where a thing is worn,
//!   its charges and what a level of it adds, in one shape any game's
//!   own definition can type a field with, so two games' files read alike
//!   without either owning the other's schema.
//! - [`loot`]: where items turn up and how often, keyed by a game's own
//!   item ids: a banded table drawn by weight, by group and by tag, what
//!   the dead leave, a place's scatter, and how good a found thing is by
//!   band.
//! - [`namegen`]: names drawn from syllables, a set of pools and weighted
//!   patterns per kind of name in one RON file, every drawn name checked
//!   against rejections and drawn again a bounded number of times.
//! - [`tiles`]: the light a tile sheds, read from the line of the tiles
//!   file that also gives its look, so a game lights its tiles without a
//!   renderer.
//! - [`prop`]: what stands on a map that is neither an actor nor an item,
//!   as data: a look, what it offers, what it holds, what sets it off and
//!   how hard it is to spot.
//! - [`role`]: a name and the monsters that fit it, drawn from a game's
//!   spawn table at a band, for a prefab's monster slot to ask for.
//! - [`prefab`]: one prefab, read from rows of glyphs and a legend naming a
//!   tile or a slot, a prop, an item row, a monster or a mark, every name
//!   but a fixed item's resolved at load; the terrain and the stamp are
//!   `atglyph-mapgen`'s.
//! - [`dressing`]: what fills an area of any size, as a prefab's legend
//!   and rules for where each glyph goes, along walls, in corners, in the
//!   center, in rows or scattered, at a density, every name resolved at
//!   load; the dressing is `atglyph-mapgen`'s.
//! - [`layout`]: the shapes a place is grown from, the districts that
//!   favor some of them and the recipes a place names, as one file with
//!   every name resolved at load; the growing is `atglyph-mapgen`'s.
//! - [`pool`]: the pieces a run may hold, by floor, tag, weight, limit,
//!   group and budget, as one file with every name resolved at load, and
//!   a whole run's picks drawn from it at once; the laying is
//!   `atglyph-mapgen`'s.
//! - [`grow`]: a layout file as the passes that grow a place, a pool's
//!   picks as the pieces and stamps it is given, and the dressing files as
//!   the pass that dresses it, through a [`PrefabLibrary`], so a place is
//!   grown from its files with no Bevy in the build.
//! - [`ai`]: tactic-priority brains over Dijkstra maps, and the standard
//!   chain a game builds one from a kind's parts with.
//! - [`work`]: one thing an actor does across many turns, counted in its
//!   own turns, and the one cell it reaches its target from.
//! - [`events`]: facts, counters and quests as data over what happened.
//! - [`balance`]: threat scoring and the spawn-band report, so content is
//!   checked from the command line.
//! - [`forecast`]: what a fight is likely to cost, run through the same
//!   mitigation a real blow goes through, for an inspect panel to print.
//! - [`arms`]: which weapon an attack is made with, the first worn weapon
//!   that has a charge, the armor a blow meets, how much of a heal
//!   arrives and what a blow on the unaware is worth.
//! - [`bump`]: what walking into a cell comes to, a step, a door, a blow, a
//!   swap, a thing taken up or a question, chosen on plain values.
//! - [`charges`]: what spending a charge and counting a refill period come
//!   to, as arithmetic on counts and hundredths.
//! - [`effects`]: who an effect is done by when a thing lands.
//! - [`items`]: what picking up, dropping, wearing and using a thing comes
//!   to, decided on facts the resolver gathers.
//! - [`trade`]: what a deal comes to, why it is refused and which pieces a
//!   purse is drawn down by.
//! - [`travel`]: who goes through a way behind whoever took it.
//! - [`accuracy`]: whether an attack lands, as a [`HitModel`] the game
//!   chooses turning plain facts into [`Odds`] and the lines behind them.
//!
//! One crate rather than five, because crate boundaries are drawn on
//! dependency weight and these all weigh the same: core, grid, serde and
//! ron. The modules keep the seams the crates had. The older items are also
//! re-exported at the root; the rule functions of [`arms`], [`bump`],
//! [`charges`], [`effects`], [`items`], [`tiles`], [`trade`] and [`travel`], and the
//! free functions the other modules gained with them, are reached through
//! their module path.
//!
//! Everything is pure. The Bevy layer turns these into components and
//! systems; a balance tool or a test runs them on plain values.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod ability;
pub mod accuracy;
pub mod ai;
pub mod arms;
pub mod balance;
pub mod bump;
pub mod charges;
pub mod content;
pub mod damage;
pub mod dressing;
pub mod effects;
pub mod equip;
pub mod events;
pub mod faction;
pub mod fire;
pub mod forecast;
pub mod gas;
pub mod grow;
pub mod items;
pub mod layout;
pub mod loot;
pub mod namegen;
pub mod names;
pub mod parts;
pub mod pool;
pub mod prefab;
pub mod prop;
pub mod role;
pub mod stats;
pub mod status;
pub mod tag;
pub mod tiles;
pub mod trade;
pub mod travel;
pub mod work;

pub use ability::{AbilityDef, AbilityId, Aim, Area, Asks, Blocked, Cost, EffectSpec, Gates, Purse, Requirement, Targeting, TriggerSpec, blocked, read_args};
pub use accuracy::{Certain, Delivery, HitModel, Line, Odds, Percent, PercentLabels, Shot};
pub use ai::{
    ActorView, Awareness, Brain, Carried, Choice, Decision, Fields, HearingStats, ItemView, Missile, MovementProfile, NoFields, NoticeStats, Repertoire, Sense,
    Snapshot, Standard, StandardBrain, StealthStats, Tactic, TacticCtx, Vitals, Wits, notices,
};
pub use balance::{BandRow, Report, ThreatSubject, threat};
pub use content::{BandedEntry, BandedTable, ContentError, Named, Registry, load_spawns};
pub use damage::{DamageKind, DamageStage, Hit, Resistances, resolve};
pub use dressing::DressingDef;
pub use equip::{EquipError, EquipShape, Equipment, SlotDef, SlotId};
pub use events::{Change, CounterDef, CounterId, Fact, FactDef, FactKind, Ledger, Matcher, Need, Objective, QuestDef, QuestId, QuestState, Tally, Tracker};
pub use faction::{FactionId, Factions, Relation};
pub use fire::Tinder;
pub use forecast::{Combatant, Duel, Outlook, duel};
pub use gas::{Breath, GasDef, GasId};
pub use layout::LayoutDef;
pub use loot::{DropRow, DropTable, Layout, LevelRow, LevelTable, LootRow, LootTable, Placed, ScatterRules, plan_scatter};
pub use namegen::{NameBook, NameSet};
pub use names::{NameRef, Names};
pub use parts::{AttackShape, KeepAway, WhenEmpty};
pub use prefab::{Coverage, CoverageRow, Legend, Legended, Pick, PrefabDef, PrefabLibrary, Reach, Slot};
pub use prop::{ContainerDef, ContentRoll, HiddenDef, OfferDef, PropDef, PropId, Stock};
pub use role::RoleDef;
pub use stats::{Modifier, Op, Source, StatDef, StatId, Stats};
pub use status::{ActiveStatus, Mended, SLEEPER_REACH, Stacking, StatusDef, StatusId, Statuses, Tick, TickReport, UNTIL_CURED};
pub use tag::{TagDef, TagId};
pub use work::{BreakReason, Work, WorkKind, WorkKindId};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::ability::{
        AbilityDef, AbilityId, Aim, Area, Asks, Blocked, Cost, EffectSpec, Gates, Purse, Requirement, Targeting, TriggerSpec, blocked, read_args,
    };
    pub use crate::ai::tactics;
    pub use crate::ai::{
        ActorView, Awareness, Brain, Carried, Choice, Decision, Fields, HearingStats, ItemView, Missile, MovementProfile, NoFields, NoticeStats, Repertoire,
        Sense, Snapshot, Standard, StandardBrain, StealthStats, Tactic, TacticCtx, Vitals, Wits, notices,
    };
    pub use crate::balance::{BandRow, Report, ThreatSubject, threat};
    pub use crate::content::{BandedEntry, BandedTable, ContentError, Named, Registry, load_spawns};
    pub use crate::damage::{DamageKind, DamageStage, Hit, Resistances, resolve};
    pub use crate::equip::{EquipError, EquipShape, Equipment, SlotDef, SlotId};
    pub use crate::events::{
        Change, CounterDef, CounterId, Fact, FactDef, FactKind, Ledger, Matcher, Need, Objective, QuestDef, QuestId, QuestState, Tally, Tracker,
    };
    pub use crate::faction::{FactionId, Factions, Relation};
    pub use crate::fire::Tinder;
    pub use crate::forecast::{Combatant, Duel, Outlook, duel};

    pub use crate::gas::{Breath, GasDef, GasId};
    pub use crate::loot::{DropRow, DropTable, Layout, LevelRow, LevelTable, LootRow, LootTable, Placed, ScatterRules, plan_scatter};
    pub use crate::namegen::{NameBook, NameSet};

    pub use crate::names::{NameRef, Names};
    pub use crate::prop::{ContainerDef, ContentRoll, HiddenDef, OfferDef, PropDef, PropId, Stock};
    pub use crate::stats::{Modifier, Op, Source, StatDef, StatId, Stats};
    pub use crate::status::{ActiveStatus, Mended, SLEEPER_REACH, Stacking, StatusDef, StatusId, Statuses, Tick, TickReport, UNTIL_CURED};
    pub use crate::tag::{TagDef, TagId};
}
