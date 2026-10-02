//! Drawing: a glyph terminal, and the map view drawn onto it.
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
//! [`Pointer`] is the cell the mouse is over, read off the same layout the
//! grid is drawn with, and through a [`MapView`] the map tile drawn there.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod capture;
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
pub use fullscreen::{FULLSCREEN, FullscreenPlugin};
pub use layout::{Fit, fit};
pub use map_view::{Glyph, LightOverlay, MapView, MapViewPlugin, TileAppearance};
pub use particles::{Animation, Beat, Burst, ParticleStyle, Particles, ParticlesPlugin, Spark, Trail};
pub use pointer::Pointer;
pub use shade::{Memory, Shading, Vary};
pub use terminal::{Cell, Terminal, TerminalPlugin, WideCells};
pub use tileset::Tileset;

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::capture::CapturePlugin;
    pub use crate::fullscreen::FullscreenPlugin;
    pub use crate::map_view::{Glyph, LightOverlay, MapView, MapViewPlugin, TileAppearance};
    pub use crate::particles::{Animation, Beat, Burst, ParticleStyle, Particles, ParticlesPlugin, Trail};
    pub use crate::pointer::Pointer;
    pub use crate::shade::{Memory, Shading, Vary};
    pub use crate::terminal::{Cell, Terminal, TerminalPlugin, WideCells};
    pub use crate::tileset::Tileset;
}
