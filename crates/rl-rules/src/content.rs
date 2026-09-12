//! Content as data: registries and tables loaded from RON.
//!
//! The engine never knows what a monster or an item is. It knows how to
//! load a list of named definitions, hand out dense typed ids for them,
//! refuse duplicates and unknown references at load time, and draw from
//! weighted tables banded by depth or distance. A game defines the
//! definition types and the validation rules; the RON is the theme.
//!
//! - [`Registry`] holds the definitions, [`Named`] is what a definition
//!   answers to be registered, and [`ContentError`] is what loading refuses.
//! - [`BandedTable`] draws from [`BandedEntry`]s by band.

pub mod registry;
pub mod table;

pub use registry::{ContentError, Named, Registry};
pub use table::{BandedEntry, BandedTable};
