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
//! [`tricks`] is the bestiary's own tricks the trait model's folding
//! cannot carry alone: a game system of their own, each a `TurnSet::React`
//! reader or a brain shape read off [`tricks::Tricks`].
//!
//! The places are [`dungeon`]'s: Mereholt, one hand-drawn piece, and the
//! ten floors under it, each a chain of the engine's passes varied by its
//! band. [`prefabs`] loads the pieces they are stamped from, [`stairs`]
//! lays the ways between them and names each in the log, [`population`]
//! puts each floor's people down the first time it is entered and
//! [`finds`] its things, drawn a kind at a time with a scroll of
//! enchanting besides, and [`light`] lights them and dims the lantern as
//! the adventurer descends.
//!
//! The run is [`run`]'s: the adventurer and what they set out with, and a
//! run carried on from a save, which [`save`] says how to write down.
//! [`quest`] is what the run is for, the Drowned King's death, and the win
//! when he dies; [`tally`] counts what the run does on the way, and when
//! it ends scores it, line by line, and writes the record the end screen
//! and the records screen show. [`wearing`] gives a wearer what a worn
//! thing lends, [`input`] turns keys into intents, and [`plugin`] is every
//! one of these systems in one list, which the binary and [`testing`]'s
//! headless world both add, so a test plays the game that ships.
//!
//! What the adventurer finds is what makes them: [`lore`] is what they
//! know of it, the faces a run's potions, scrolls, staffs, wands and rings
//! wear until they are known and the levels of what they wear until they
//! are familiar with it, and [`seeming`] what a fight looks like to them
//! meanwhile; [`enchant`] is a thing made better by a level,
//! and the curse a level below nought holds it on by; [`strength`] is what
//! heavy gear asks of its wearer; and [`stats`] reads the stats the engine
//! does not. [`effects`] is what Undermere's staffs, wands, potions and
//! scrolls do that the engine's effects do not, [`conditions`] what the
//! statuses they put on do that the engine's statuses do not, [`staffs`]
//! how soon a staff's spent charge comes back, and [`allies`] is who a
//! wand or a staff of theirs puts on the adventurer's side.
//!
//! Before any of it, [`title`] paints the screen the game opens on: the
//! black lake at night under the engine's title menu. Every glyph of it
//! is drawn in the face [`typeface`] holds, which the game ships, so it
//! looks the same on every platform.

#![deny(missing_docs)]

pub mod allies;
pub mod armory;
pub mod bestiary;
pub mod conditions;
pub mod content;
pub mod damage;
pub mod dungeon;
pub mod effects;
pub mod enchant;
pub mod finds;
pub mod input;
pub mod light;
pub mod lore;
pub mod plugin;
pub mod population;
pub mod prefabs;
pub mod quest;
pub mod run;
pub mod save;
pub mod seeming;
pub mod staffs;
pub mod stairs;
pub mod stats;
pub mod strength;
pub mod tally;
pub mod testing;
pub mod title;
pub mod traits;
pub mod tricks;
pub mod typeface;
pub mod wearing;
