//! Views: what a panel would have had to query for itself.
//!
//! A view is a resource holding plain data, rebuilt every frame by a
//! collector system in [`ViewSet::Collect`](crate::ViewSet::Collect) from
//! the components the engine already owns. It carries no colour, no
//! rectangle and no string the game did not supply, so it is equally
//! serviceable to the terminal panels in [`panel`](crate::panel), to a
//! game's own drawing, and to a test that never opens a window.
//!
//! The split exists because the query is the reusable half. "Every actor in
//! the viewshed, nearest first, with a health fraction and a relation" is
//! the same sentence in every roguelike; how it looks on screen is not.
//!
//! Each view has its own plugin, and adding one is the whole opt-in.
//! Nothing here runs unless a game asks for it.

pub mod ability;
pub mod gear;
pub mod inspect;
pub mod nearby;
pub mod sheet;
pub mod target;
pub mod vitals;

pub use ability::{AbilityRow, AbilityView, AbilityViewPlugin};
pub use gear::{GearSlot, GearView, GearViewPlugin};
pub use inspect::{InspectView, InspectViewPlugin};
pub use nearby::{NearbyView, NearbyViewPlugin};
pub use sheet::{Change, ResistLine, SheetView, SheetViewPlugin, StatLine, StatusLine, Strike, WornLine};
pub use target::{AimAt, AimFire, AimThrow, TargetView, TargetViewPlugin, target_modal};
pub use vitals::{VitalsView, VitalsViewPlugin};

use bevy::prelude::*;
use rl_render::Glyph;
use rl_rules::Relation;

use crate::facet::{Facet, FacetId};

/// One entity, as a panel reads it.
///
/// Built by a collector from components; a game's annotate system may push
/// facets onto it afterwards. `relation` and `health` are optional because
/// a thing on the floor has neither.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Which entity, so a panel can select or highlight it and a game's
    /// annotate system can look up whatever else it knows.
    pub entity: Entity,
    /// What the game called it, from its [`Name`].
    pub label: String,
    /// Its own glyph and colour, which are content and not theme: a green
    /// slime is green in every palette.
    pub glyph: Glyph,
    /// Chebyshev tiles from whoever is looking, zero when that means
    /// nothing.
    pub distance: i32,
    /// How it stands to the one looking, if it takes sides.
    pub relation: Option<Relation>,
    /// Current and maximum health, if it has any.
    pub health: Option<(i32, i32)>,
    /// Whether it has noticed the player: `None` for something that does not
    /// notice at all, or in a game without stealth. In the view rather than
    /// a facet because the engine knows it and it reads the same in every
    /// game.
    pub aware: Option<bool>,
    /// What the game added. Empty until an annotate system pushes.
    pub facets: Vec<Facet>,
}

impl Row {
    /// A row for `entity` with nothing but a name and a glyph.
    pub fn new(entity: Entity, label: impl Into<String>, glyph: Glyph) -> Self {
        Self { entity, label: label.into(), glyph, distance: 0, relation: None, health: None, aware: None, facets: Vec::new() }
    }

    /// The same row, `distance` tiles away.
    pub fn at(mut self, distance: i32) -> Self {
        self.distance = distance;
        self
    }

    /// The facet under `key`, if the game pushed one.
    pub fn facet(&self, key: FacetId) -> Option<&Facet> {
        self.facets.iter().find(|f| f.key == key)
    }

    /// Health as a fraction from 0 to 1, or 1 for something without any.
    pub fn health_fraction(&self) -> f32 {
        match self.health {
            Some((hp, max)) if max > 0 => (hp.max(0) as f32 / max as f32).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }
}

/// A labelled quantity with a maximum: health, a resource, a fuel gauge.
///
/// The engine fills one for health and a game pushes whatever else it
/// tracks, so a panel draws bars it has never heard of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bar {
    /// What it measures.
    pub label: String,
    /// Where it stands.
    pub value: i32,
    /// Where full is.
    pub max: i32,
    /// How the filled part reads.
    pub tone: crate::tone::ToneId,
}

impl Bar {
    /// A bar reading `value` of `max`.
    pub fn new(label: impl Into<String>, value: i32, max: i32, tone: crate::tone::ToneId) -> Self {
        Self { label: label.into(), value, max, tone }
    }

    /// How full, from 0 to 1. A bar with no maximum reads empty.
    pub fn fraction(&self) -> f32 {
        if self.max <= 0 { 0.0 } else { (self.value.max(0) as f32 / self.max as f32).clamp(0.0, 1.0) }
    }
}

/// What every collector needs to name an entity: its label and its glyph.
///
/// An entity without both is skipped rather than drawn as a blank, because
/// a nameless row is a spawn that forgot its [`Name`] and a silent blank
/// row is the hardest kind of that to notice.
pub type Named<'a> = (&'a Label, &'a Glyph);
