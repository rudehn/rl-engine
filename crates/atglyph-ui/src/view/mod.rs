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
pub mod container;
pub mod ending;
pub mod gear;
pub mod inspect;
pub mod inventory;
pub mod nearby;
pub mod offers;
pub mod party;
pub mod sheet;
pub mod target;
pub mod trade;
pub mod vitals;

pub use ability::{AbilityRow, AbilityView, AbilityViewPlugin};
pub use container::{ContainerView, ContainerViewPlugin, OpenContainer};
pub use ending::{EndingSection, EndingView, EndingViewPlugin};
pub use gear::{GearSlot, GearView, GearViewPlugin};
pub use inspect::{InspectView, InspectViewPlugin};
pub use inventory::{Asking, InventoryView, InventoryViewPlugin, ItemRow};
pub use nearby::{NearbyFollowers, NearbyStatuses, NearbyView, NearbyViewPlugin};
pub use offers::{OfferRow, OffersView, OffersViewPlugin};
pub use party::{PartyView, PartyViewPlugin};
pub use sheet::{Change, ResistLine, SheetView, SheetViewPlugin, StatLine, StatusLine, Strike, WornLine};
pub use target::{AimAt, AimFire, AimRange, AimThrow, AimUse, TargetView, TargetViewPlugin, target_modal};
pub use trade::{TradeRow, TradeView, TradeViewPlugin};
pub(crate) use vitals::health_tone;
pub use vitals::{VitalsView, VitalsViewPlugin};

use atglyph_render::Glyph;
use atglyph_rules::Relation;
use bevy::prelude::*;

use crate::facet::Facet;

/// How much more a body's numbers truly are than they seem to the player,
/// which the vitals, the look cursor's forecast and the targeting box take
/// off before they show them.
///
/// The engine counts what a body truly wears, since that is what a blow
/// meets, and has no idea of a thing whose numbers the player does not
/// know: whether a thing is known, and what it seems to be until it is, is
/// a game's. A game that hides some of a thing's numbers keeps this on an
/// actor before [`ViewSet::Collect`](crate::ViewSet::Collect), as
/// Undermere keeps on each wearer what the level of every worn thing not
/// yet known adds past its kind's plain numbers. Absent, or all nought, the
/// panels show the true numbers, which is every game without the idea.
///
/// Each figure is taken off the true one: `armor` off the vitals' armor
/// and the forecast's, `blow` and `shot` off the first roll of the blow and
/// the shot the forecast reckons with, and `accuracy` off the accuracy the
/// chance to hit is worked out from, where the game names an accuracy
/// stat. A view that took anything off says so with its `seeming` flag,
/// so the game can say why in its own words.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Seems {
    /// Armor that is truly there and does not seem to be.
    pub armor: i32,
    /// Damage the blow truly adds past what it seems to.
    pub blow: i32,
    /// Damage the shot truly adds past what it seems to.
    pub shot: i32,
    /// Accuracy truly lent past what it seems to be.
    pub accuracy: i32,
}

impl Seems {
    /// Whether nothing is taken off: the numbers shown are the true ones.
    pub(crate) fn is_true(&self) -> bool {
        *self == Self::default()
    }
}

/// Every named thing that carries tags, for finding what a piece of the
/// currency is called by [`currency_name`].
pub type NamedPieces<'w, 's> = Query<'w, 's, (Entity, &'static Name, &'static atglyph_bevy::Tagged)>;

/// What one piece of the trade slice's currency is called: the `Name` of
/// the first piece among `near`, else of any named piece in play, the
/// lowest entity first so the word never flickers between frames. Empty
/// with no currency, or no named piece anywhere.
///
/// One place for the word, so the trade screen and the offers screen
/// name the same currency the same way, and in the game's own word for a
/// piece rather than its tag's. Public so a game's own line about a
/// payment, as one an offer's answer writes, names it by the same rule.
pub fn currency_name(currency: Option<&atglyph_bevy::Currency>, near: impl IntoIterator<Item = Entity>, pieces: &NamedPieces) -> String {
    let Some(currency) = currency else { return String::new() };
    let is = |(_, _, tags): &(Entity, &Name, &atglyph_bevy::Tagged)| currency.is(Some(*tags));
    let nearby = near.into_iter().filter_map(|e| pieces.get(e).ok()).find(is);
    nearby.or_else(|| pieces.iter().filter(is).min_by_key(|(e, _, _)| *e)).map(|(_, name, _)| name.as_str().to_string()).unwrap_or_default()
}

/// `amount` of the currency called `name`, as every screen says it:
/// `1 scrip`, `25 scrips`, or the bare number when it has no name.
pub fn sum_of(name: &str, amount: u32) -> String {
    match (name.is_empty(), amount) {
        (true, _) => amount.to_string(),
        (false, 1) => format!("1 {name}"),
        (false, _) => atglyph_core::noun::listed(name, amount),
    }
}

/// What something in sight is doing about the player.
///
/// Three states and no more, because three is what the engine can say
/// without guessing: it has noticed you, it is going to look for you, or
/// it knows of nothing. A game that wants a finer reading, a
/// droid winding up or a beast feeding, pushes a [`Facet`].
///
/// What each is *called* is a panel's, not this: one game's monsters
/// sleep where another's stand idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alert {
    /// It knows of nothing: it has not noticed you and has heard nothing.
    Unaware,
    /// It is going to look without seeing you: at something it heard, or
    /// at where it last knew of you before you went unseen.
    Searching,
    /// It has noticed you.
    Hunting,
}

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
    /// What it is doing about the player: hunting, searching, or aware of
    /// nothing. `None` for something that neither notices nor hears, and
    /// in a game with neither stealth nor noise. In the view rather than a
    /// facet because the engine knows it and it reads the same in every
    /// game; what it is *called* is the panel's, since one game's monsters
    /// sleep and another's stand idle.
    pub alert: Option<Alert>,
    /// What it is working at, when it is at work. Written where the alert
    /// would be: what it is busy doing is what a player needs to know.
    pub work: Option<WorkRow>,
    /// The name of the tactic that decided its last turn, for a panel to
    /// say in the game's words what it is doing; `None` for a thing, for an
    /// actor with no mind, and for a turn no tactic decided.
    pub doing: Option<&'static str>,
    /// What the game added. Empty until an annotate system pushes.
    pub facets: Vec<Facet>,
}

/// What an actor is working at, as a panel reads it.
///
/// A field of the view rather than a facet, because the engine knows it
/// and every game with work would push the same one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkRow {
    /// The word the game declared the kind of work by.
    pub doing: String,
    /// What the work is done to, by name, if it is done to something.
    pub target: Option<String>,
    /// Of the worker's own turns, how many are left.
    pub left: u16,
}

/// Reads what an entity is working at, for any collector.
///
/// Empty in a game without work: no `Working` is ever put on anything, and
/// without `WorkKinds` there is no word to show.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Workings<'w, 's> {
    working: Query<'w, 's, &'static atglyph_bevy::Working>,
    kinds: Option<Res<'w, atglyph_bevy::WorkKinds>>,
    names: Query<'w, 's, &'static Name>,
}

impl Workings<'_, '_> {
    /// What `entity` is working at, if anything.
    pub(crate) fn row(&self, entity: Entity) -> Option<WorkRow> {
        let work = self.working.get(entity).ok()?.0;
        let kinds = self.kinds.as_deref()?;
        let target = work.target.and_then(|t| self.names.get(t).ok()).map(|n| n.as_str().to_string());
        Some(WorkRow { doing: kinds.name(work.kind).to_string(), target, left: work.left() })
    }
}

impl Row {
    /// A row for `entity` with nothing but a name and a glyph.
    pub fn new(entity: Entity, label: impl Into<String>, glyph: Glyph) -> Self {
        Self { entity, label: label.into(), glyph, distance: 0, relation: None, health: None, alert: None, work: None, doing: None, facets: Vec::new() }
    }

    /// The same row, `distance` tiles away.
    pub fn at(mut self, distance: i32) -> Self {
        self.distance = distance;
        self
    }

    /// The facet under `key`, if the game pushed one.
    pub fn facet(&self, key: &str) -> Option<&Facet> {
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
