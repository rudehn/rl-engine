//! Undermere: a delve from a lakeside town through the Drowned Gate, down
//! ten floors of a sunken kingdom to the thing that drowned it.
//!
//! A library as well as a binary, as Foundry is, so a test reaches the
//! content and the game it plays through the crate the binary links,
//! rather than through `main.rs`. `examples/undermere/DESIGN.md` is what
//! the game is and why.
//!
//! The content is RON in `assets/`, compiled in, and read in the order
//! its names need: [`content`] fills the registries every file names
//! into and reads the abilities, [`traits`] reads the traits a monster is
//! written as, [`armory`] the items, and [`bestiary`] the monsters, each
//! one its core and its traits folded together at load, so a spawn reads
//! plain values. [`content::Library`] is that order, written once.
//!
//! The places are [`dungeon`]'s: Mereholt, one hand-drawn piece, and the
//! ten floors under it, each a chain of the engine's passes varied by its
//! band. [`prefabs`] loads the pieces they are stamped from, [`stairs`]
//! lays the ways between them and names each in the log, and [`light`]
//! lights them. [`testing`] is a headless world with the places in it,
//! for the tests.

#![deny(missing_docs)]

pub mod armory;
pub mod bestiary;
pub mod content;
pub mod dungeon;
pub mod light;
pub mod prefabs;
pub mod stairs;
pub mod testing;
pub mod traits;
