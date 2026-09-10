//! Content as data: registries and tables loaded from RON.
//!
//! The engine never knows what a monster or an item is. It knows how to
//! load a list of named definitions, hand out dense typed ids for them,
//! refuse duplicates and unknown references at load time, and draw from
//! weighted tables banded by depth or distance. A game defines the
//! definition types and the validation rules; the RON is the theme.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod registry;
pub mod table;

pub use registry::{ContentError, Named, Registry};
pub use table::{BandedEntry, BandedTable};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::registry::{ContentError, Named, Registry};
    pub use crate::table::{BandedEntry, BandedTable};
}
