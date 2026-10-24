//! Drawing: a glyph terminal, and the map view drawn onto it.
//!
//! Part of `atglyph`: a game depends on `atglyph` and reaches this crate as `atglyph::render`.
//!
//! Game code writes cells into a [`Terminal`] back buffer as if it were a
//! console. The plugin diffs it against what is on screen and touches only
//! the entities whose cell changed, so a turn-based frame where nothing
//! moved costs nothing.
//!
//! The backing store is one sprite and one text entity per cell, which is
//! fine at a hundred columns and will be replaced by an instanced quad
//! grid when a bigger terminal is wanted. Nothing above this module will
//! notice the change.
//!
//! `Pointer` is the cell the mouse is over, read off the same layout the
//! grid is drawn with, and through a [`MapView`] the map tile drawn there.
//!
//! - [`terminal`]: the glyph grid a game and its panels write cells into,
//!   and the plugin that draws its back buffer.
//! - [`layout`] and [`pointer`](mod@pointer): where the cells go in a window of any
//!   size, and which cell the mouse is over.
//! - [`map_view`]: the map drawn from the player's point of view, each
//!   tile's look and the actors over it.
//! - [`shade`], [`looks`] and [`fields`]: colour, light and memory the way
//!   Brogue draws them, tile looks read from a file, and fire and gas
//!   blended in over a moment.
//! - [`particles`]: what flies, bursts and fades over the map after a turn.
//! - [`tileset`]: pictures drawn in place of glyphs.
//! - [`fullscreen`] and [`capture`]: filling the screen, as a setting, and
//!   a game photographing its own window.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod capture;
mod cell_art;
pub mod fields;
pub mod fullscreen;
pub mod layout;
pub mod looks;
pub mod map_view;
pub mod particles;
pub mod pointer;
pub mod shade;
pub mod terminal;
pub mod tileset;

pub use capture::CapturePlugin;
pub use fullscreen::FullscreenPlugin;
pub use map_view::{DrawnActors, Glyph, LightOverlay, MapView, MapViewPlugin, TileAppearance};
pub use particles::{ParticleStyle, Particles, ParticlesPlugin};
pub use shade::{Memory, Shading, Vary};
pub use terminal::{Cell, Terminal, TerminalPlugin};
pub use tileset::Tileset;

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::capture::CapturePlugin;
    pub use crate::fullscreen::FullscreenPlugin;
    pub use crate::map_view::{DrawnActors, Glyph, LightOverlay, MapView, MapViewPlugin, TileAppearance};

    pub use crate::particles::{ParticleStyle, Particles, ParticlesPlugin};

    pub use crate::shade::{Memory, Shading, Vary};
    pub use crate::terminal::{Cell, Terminal, TerminalPlugin};
    pub use crate::tileset::Tileset;
}
