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

use bevy::prelude::*;
use rl_core::{Id, Interner};

use crate::tone::ToneId;

/// The type [`FacetId`] indexes. Never constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FacetKey;

/// A registered facet key.
pub type FacetId = Id<FacetKey>;

/// The facet keys a game has named.
///
/// Keys exist so a panel can pick facets out rather than printing all of
/// them: a narrow rail may want only the one keyed `wielding`.
#[derive(Resource, Debug, Clone, Default)]
pub struct Facets(Interner<FacetKey>);

impl Facets {
    /// The id for `name`, assigning a new one if it is unseen.
    pub fn declare(&mut self, name: &str) -> FacetId {
        self.0.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<FacetId> {
        self.0.get(name)
    }

    /// The name behind `key`.
    ///
    /// # Panics
    /// Panics if `key` did not come from this registry.
    pub fn name(&self, key: FacetId) -> &str {
        self.0.name(key)
    }

    /// How many keys exist.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing has been declared.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every key, with its name.
    pub fn iter(&self) -> impl Iterator<Item = (FacetId, &str)> {
        self.0.iter()
    }

    /// The facet under `key` reading `text`, declaring the key if it is
    /// new. The one-liner an annotate system wants:
    ///
    /// ```
    /// # use rl_ui::{Facets, Tones};
    /// # let mut facets = Facets::default();
    /// # let mut row_facets = Vec::new();
    /// row_facets.push(facets.facet("wielding", "cutlass").toned(Tones::MUTED));
    /// ```
    pub fn facet(&mut self, key: &str, text: impl Into<String>) -> Facet {
        Facet::new(self.declare(key), text)
    }
}

/// One note on a row: what it is about, what it says, and how it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facet {
    /// Which kind of note.
    pub key: FacetId,
    /// The words, already in the game's language.
    pub text: String,
    /// Its semantic role, which the palette turns into a colour.
    pub tone: ToneId,
}

impl Facet {
    /// A facet in the ordinary text tone.
    pub fn new(key: FacetId, text: impl Into<String>) -> Self {
        Self { key, text: text.into(), tone: crate::tone::Tones::TEXT }
    }

    /// The same facet in `tone`.
    pub fn toned(mut self, tone: ToneId) -> Self {
        self.tone = tone;
        self
    }
}
