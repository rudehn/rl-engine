//! Facts, counters and quests: gameplay outcomes as data a game can
//! subscribe to.
//!
//! The engine's systems report what happened as typed messages. A game
//! turns the ones it cares about into [`Fact`]s: a registered kind, a
//! subject and an object in the game's own numbering, and an amount.
//! Everything downstream is data over facts: a [`Matcher`] picks facts
//! out, a [`Ledger`] tallies them into named counters, and a [`Tracker`]
//! advances quests whose objectives are matchers with a required count.
//! A quest, an achievement or a generated victory condition is then a
//! definition, not a system.
//!
//! Nothing here names a fact, a counter or a quest. Pure; the Bevy layer
//! wraps it in a resource and a message.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod fact;
pub mod ledger;
pub mod quest;

pub use fact::{Fact, FactDef, FactKind, Matcher};
pub use ledger::{CounterDef, CounterId, Ledger, Tally};
pub use quest::{Change, Need, Objective, QuestDef, QuestId, QuestState, Tracker};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::fact::{Fact, FactDef, FactKind, Matcher};
    pub use crate::ledger::{CounterDef, CounterId, Ledger, Tally};
    pub use crate::quest::{Change, Need, Objective, QuestDef, QuestId, QuestState, Tracker};
}
