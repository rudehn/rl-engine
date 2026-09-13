//! Panels for a roguelike, that a game may take, restyle, or leave.
//!
//! # How a panel is built here
//!
//! Every panel splits three ways, and the split is the whole point:
//!
//! | Layer | What it is | Where |
//! |---|---|---|
//! | **View** | a resource of plain data: rows, bars, numbers | [`view`] |
//! | **Collector** | the system that rebuilds the view each frame | [`view`], in [`ViewSet::Collect`] |
//! | **Presenter** | one way of drawing it | [`panel`], in [`PresentSet`](rl_bevy::PresentSet) |
//!
//! The query is the half worth sharing. "Every actor in the viewshed,
//! nearest first, with a health fraction and a relation" is the same
//! sentence in every roguelike; a gold-ruled rail with small-caps headings
//! is one game's taste. So the engine owns the first and offers the second.
//!
//! One view may have more than one presenter: [`LogPanel`] draws the last
//! few lines along the bottom of the map and [`ScrollbackPanel`] draws all
//! of them on a screen, over the same [`MessageLog`], and neither knows the
//! other exists.
//!
//! A game picks its level of involvement, cheapest first:
//!
//! 1. Add the panel. `app.add_plugins(NearbyPanel::new(rect))` and it draws
//!    itself, pulling its view plugin in behind it.
//! 2. Change the [`Palette`]. Every panel restyles at once, including tones
//!    the engine never heard of.
//! 3. Add facts the engine cannot know, by pushing a [`Facet`] onto a row
//!    in [`ViewSet::Annotate`].
//! 4. Add the view plugin alone and draw it yourself, from the same data.
//! 5. Add neither. Nothing here runs unless it is asked for.
//!
//! # Adding a panel to a game
//!
//! ```
//! # use bevy::prelude::*;
//! # use rl_core::Rect;
//! # use rl_ui::{LogPanel, NearbyPanel, UiPlugin, VitalsPanel, panel};
//! # let mut app = App::new();
//! let screen = Rect::new(0, 0, 100, 40);
//! let (left, rail) = panel::split_right(screen, 24);
//! let (map, log) = panel::split_bottom(left, 5);
//! let (status, _map) = panel::split_top(map, 1);
//!
//! app.add_plugins(UiPlugin)
//!     .add_plugins(VitalsPanel::new(status).hints("[i]nventory [q]uit"))
//!     .add_plugins(LogPanel::new(log))
//!     .add_plugins(NearbyPanel::new(rail));
//! ```
//!
//! # Adding a panel to the engine
//!
//! Write it in this order and the seams come out right:
//!
//! 1. A view struct in [`view`], holding [`Row`]s, [`Bar`]s and numbers.
//!    No `Color`, no [`Rect`](rl_core::Rect), no string the game did not
//!    supply. Anything a game might phrase differently is a [`Facet`].
//! 2. A collector system in [`ViewSet::Collect`], and a plugin that adds
//!    it and declares what it needs with [`rl_bevy::needs`].
//! 3. Arithmetic that is really about the rules, in `rl-rules` where it can
//!    be tested without an `App`. [`rl_rules::forecast`] is the example: the
//!    inspect panel's numbers come from the same mitigation pipeline a real
//!    blow goes through.
//! 4. A presenter in [`panel`], as a plugin holding its rectangle. Colours
//!    come from the [`Palette`] by [`ToneId`], never as literals.
//! 5. A test per layer: rows for a hand-built world, exact terminal text
//!    for the drawing.
//!
//! # Rules
//!
//! - **No theme words.** No engine type, doc or constant says weapon, spell
//!   or monster. A game's vocabulary reaches a panel as a [`Name`] on an
//!   entity or a [`Facet`] on a row.
//! - **No colour in a view, and no literal in a panel.** A widget that took
//!   a `Color` would be a widget every game forked.
//! - **Never order a system after another crate's function.** Use
//!   [`ViewSet`] and [`PresentSet`](rl_bevy::PresentSet).
//! - **A panel owns no state.** If it needs to remember something, that is
//!   a view or a [`ListMenu`] the game holds.
//!
//! [`Name`]: rl_bevy::Label

#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[cfg(test)]
pub(crate) mod harness;

pub mod facet;
pub mod keys;
pub mod log;
pub mod menu;
pub mod modal;
pub mod panel;
pub mod tone;
pub mod view;

pub use facet::{Facet, FacetId, FacetKey, Facets};
pub use keys::DirectionKeys;
pub use log::{LogEntry, MessageLog};
pub use menu::{ListMenu, MenuRow, draw_menu};
pub use modal::{Modal, ModalId, Modals, modal_is, modal_open, no_modal};
pub use panel::{GearPanel, InspectPanel, LogPanel, NearbyPanel, Scrollback, ScrollbackKeys, ScrollbackPanel, VitalsPanel};
pub use tone::{Palette, Tone, ToneId, Tones};
pub use view::{
    Bar, GearSlot, GearView, GearViewPlugin, InspectKeys, InspectView, InspectViewPlugin, NearbyView, NearbyViewPlugin, Row, VitalsView, VitalsViewPlugin,
};

use bevy::prelude::*;

/// When a view is filled, and when a game may add to it.
///
/// Both run inside [`PresentSet::Narrate`](rl_bevy::PresentSet::Narrate),
/// which is the frame's "work out what to say" phase, before anything
/// draws. A game's annotate system names this set rather than ordering
/// itself after a collector function, so the engine may split a collector
/// in two without breaking it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewSet {
    /// The engine rebuilds every view from the world.
    Collect,
    /// The game pushes what the engine cannot know.
    Annotate,
}

/// The base every other plugin in this crate needs: tones, the palette,
/// facet keys, the modal stack and the direction bindings.
///
/// Adds no systems that draw and no views. A game adds this once and then
/// the panels it wants.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tones>()
            .init_resource::<Palette>()
            .init_resource::<Facets>()
            .init_resource::<Modals>()
            .init_resource::<DirectionKeys>()
            .configure_sets(Update, (ViewSet::Collect, ViewSet::Annotate).chain().in_set(rl_bevy::PresentSet::Narrate))
            .add_systems(OnEnter(rl_bevy::EngineState::Playing), tone::report_unset_tones);
        // The one facet key the engine itself pushes: a status badge.
        app.world_mut().resource_mut::<Facets>().declare("badge");
    }

    fn finish(&self, app: &mut App) {
        rl_bevy::depends_on::<rl_bevy::CorePlugin>(app, "UiPlugin");
    }
}

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::facet::{Facet, FacetId, Facets};
    pub use crate::keys::DirectionKeys;
    pub use crate::log::{LogEntry, MessageLog};
    pub use crate::menu::{ListMenu, MenuRow, draw_menu};
    pub use crate::modal::{ModalId, Modals, modal_is, modal_open, no_modal};
    // The module itself, for `panel::split_right` and the drawing
    // helpers a game writing its own presenter reaches for.
    pub use crate::panel;
    pub use crate::panel::{GearPanel, InspectPanel, LogPanel, NearbyPanel, Scrollback, ScrollbackKeys, ScrollbackPanel, VitalsPanel};
    pub use crate::tone::{Palette, ToneId, Tones};
    pub use crate::view::{Bar, GearView, GearViewPlugin, InspectView, InspectViewPlugin, NearbyView, NearbyViewPlugin, Row, VitalsView, VitalsViewPlugin};
    pub use crate::{UiPlugin, ViewSet};
}
