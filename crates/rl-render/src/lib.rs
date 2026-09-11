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

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod capture;
pub mod map_view;
pub mod shade;
pub mod terminal;

pub use capture::CapturePlugin;
pub use map_view::{Glyph, LightOverlay, MapView, MapViewPlugin, TileAppearance};
pub use shade::{Memory, Shading, Vary};
pub use terminal::{Cell, Terminal, TerminalPlugin};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::capture::CapturePlugin;
    pub use crate::map_view::{Glyph, LightOverlay, MapView, MapViewPlugin, TileAppearance};
    pub use crate::shade::{Memory, Shading, Vary};
    pub use crate::terminal::{Cell, Terminal, TerminalPlugin};
}
