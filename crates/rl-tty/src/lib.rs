//! A game on the glyph terminal, played in a text terminal instead of a
//! window: locally, or over SSH.
//!
//! Every panel already draws into one buffer of cells, the
//! [`Terminal`](rl_render::Terminal), and the window is only one way of
//! showing it. This crate is another: it writes the cells that changed
//! since the last frame to a terminal, and turns the keys the terminal
//! reports back into the `ButtonInput<KeyCode>` every input system reads.
//! So a game, its panels and its keys run unchanged.
//!
//! - [`TtyPlugin`] is the whole of it, in place of the window's
//!   `TerminalPlugin`: the process's own terminal with
//!   [`TtyPlugin::new`], or any writer, an SSH channel, with
//!   [`TtyPlugin::to_writer`], whose keys and size arrive as [`TtyInput`]
//!   messages. `rl_engine::TtyRoguelikePlugins` adds it with the rest of
//!   what every game adds.
//! - [`keystroke`] turns a terminal's key event into the key a keyboard
//!   would have pressed, as a [`Keystroke`], and [`is_interrupt`] says
//!   whether it was Control-C.
//! - [`changes`] is the cells of a frame that differ from the last, and
//!   [`shown`] the glyph a terminal can draw in one column for a cell,
//!   drawn in as many [`Colors`] as the terminal has.

#![deny(missing_docs)]

mod keys;
mod paint;
mod plugin;

pub use keys::{Keystroke, is_interrupt, keystroke};
pub use paint::{Colors, changes, shown};
pub use plugin::{TtyInput, TtyPlugin};

/// The terminal library this crate speaks, re-exported so a game that
/// feeds it events from elsewhere, an SSH server say, names the same version.
pub use crossterm;
