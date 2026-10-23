//! Facets: the notes a game adds to a row the engine built.
//!
//! A view can only carry what the engine knows, and the engine does not
//! know what a weapon is, what an alert sentry looks like, or which of a
//! game's twelve resistances matters. Adding a field per game would put
//! game vocabulary in an engine type, which is the thing the whole crate
//! refuses to do.
//!
//! So a row carries a list of facets instead. The engine fills the row; a
//! game's system runs in [`ViewSet::Annotate`](crate::ViewSet::Annotate)
//! and pushes what it knows; the panel prints them in the order they were
//! pushed, in whatever colour their tone resolves to. Nothing in the middle
//! has to learn a word.
//!
//! Facets are an escape hatch and their use is a signal: if two games push
//! the same key, the field belongs in the view.

use crate::tone::ToneId;

/// One note on a row: what it is about, what it says, and how it reads.
///
/// The key is a plain name, so a panel can pick facets out rather than
/// printing all of them: a narrow rail may want only the one keyed
/// `wielding`. It is `&'static str` because every key is a word the game's
/// code names, and a name to compare is all a key is for:
///
/// ```
/// # use rl_ui::{Facet, Tones};
/// # let mut row_facets = Vec::new();
/// row_facets.push(Facet::new("wielding", "cutlass").toned(Tones::MUTED));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facet {
    /// Which kind of note.
    pub key: &'static str,
    /// The words, already in the game's language.
    pub text: String,
    /// Its semantic role, which the palette turns into a colour.
    pub tone: ToneId,
}

impl Facet {
    /// A facet under `key` reading `text`, in the ordinary text tone.
    pub fn new(key: &'static str, text: impl Into<String>) -> Self {
        Self { key, text: text.into(), tone: crate::tone::Tones::TEXT }
    }

    /// The same facet in `tone`.
    pub fn toned(mut self, tone: ToneId) -> Self {
        self.tone = tone;
        self
    }
}
