//! Headless tools over content: the balance checker.
//!
//! A game's monsters, items and spawn tables are its own types; the
//! engine cannot read them. What it can do is score anything that answers
//! [`ThreatSubject`] and roll a spawn table into a report of what the
//! player meets at each band, so a content change is checked with a
//! command rather than a playthrough.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod balance;

pub use balance::{BandRow, Report, ThreatSubject, threat};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::balance::{BandRow, Report, ThreatSubject, threat};
}
