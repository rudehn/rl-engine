//! The rules layer: shapes in the engine, vocabulary in the game.
//!
//! Nothing here names a stat, a damage type, a status, a faction or an
//! equipment slot. The engine ships the accumulator, the mitigation
//! pipeline, the tick and expiry machinery, the relation matrix and the
//! slot graph; a game registers what exists and the numbers that go with
//! it.
//!
//! Everything is pure. The Bevy layer turns these into components and
//! systems; a balance tool or a test runs them on plain values.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod damage;
pub mod equip;
pub mod faction;
pub mod stats;
pub mod status;

pub use damage::{DamageKind, DamageStage, Hit, Resistances, resolve};
pub use equip::{EquipError, EquipShape, Equipment, SlotDef, SlotId};
pub use faction::{FactionId, Factions, Relation};
pub use stats::{Modifier, Op, StatDef, StatId, Stats};
pub use status::{ActiveStatus, Stacking, StatusDef, StatusId, Statuses, Tick, TickReport};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::damage::{DamageKind, DamageStage, Hit, Resistances, resolve};
    pub use crate::equip::{EquipError, EquipShape, Equipment, SlotDef, SlotId};
    pub use crate::faction::{FactionId, Factions, Relation};
    pub use crate::stats::{Modifier, Op, StatDef, StatId, Stats};
    pub use crate::status::{ActiveStatus, Stacking, StatusDef, StatusId, Statuses, Tick, TickReport};
}
