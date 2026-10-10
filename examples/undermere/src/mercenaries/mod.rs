//! The swords for hire at the Sunken Lantern: one kind, its names, and
//! what each carries and asks.
//!
//! There is one kind, [`MERCENARY`] in `monsters.ron` with the trait of
//! the same name, and what makes one unlike the next is its kit and its
//! name, not its kind. The names are the `mercenary` set of `names.ron`,
//! [`NameSets::mercenary`](crate::townsfolk::NameSets::mercenary).
//! [`Mercenaries`] is `assets/mercenaries.ron` read against the
//! [`Armory`]: the [`Fee`], the parts of a kit, each a chance and rows the
//! engine's [`LootTable`] draws at a band, and the levels a thing that
//! takes them rolls on top of its kind's plain one. The band is the
//! deepest floor the run has reached, nought on the first evening, so a
//! later visit can find what the first could not.
//!
//! [`Mercenaries::kit`] and [`Mercenaries::fee`] are the pure half, tested
//! without an `App`: a kit at a band from whatever generator it is handed,
//! and what a kit costs. [`make_kit`] makes a kit's things to be carried.
//!
//! [`seat_candidates`] seats three of them at the tavern's tables on every
//! arrival in Mereholt, drawn from the run's [`TAVERN_STREAM`] at the
//! visit's number, which [`Tavern`] counts; the last visit's that nobody
//! hired are gone first. Each is a [`Candidate`] made by
//! [`spawn_candidate`]: the one kind on the townsfolk's side, with no mind,
//! wearing its kit and offering [`HIRE`] at its fee beside talk. `hire` is
//! the engine's priced offer, so a walk into a candidate opens the offers
//! screen, an adventurer short of the fee is refused there, and one who
//! takes it up has paid before the game hears of it: whatever hiring then
//! does answers the `Interacted` it sends, in `TurnSet::React`.
//! [`keep_hire_offered`] takes `hire` off every candidate while the party
//! is full. What a candidate holds is said as the adventurer knows it, by
//! the name each thing carries, when talked to by [`answer_candidates`]
//! and on the look panel by [`show_candidates`].
//!
//! Undermere's own rather than a keeper's [`Wares`](crate::shops::Wares):
//! a kit is a part at a time, each on its own chance, where a shelf is a
//! count of draws; and a kit's level may be below nought, which the
//! engine's [`LevelTable`] refuses for a find, so the levels are a loot
//! table whose item is the level. A part's rows are the engine's spawn
//! file, read by the reader `item_spawns.ron` is, so a row is written and
//! checked the one way.

mod kit;
mod tavern;

pub use kit::*;
pub use tavern::*;

#[cfg(doc)]
use crate::armory::Armory;
#[cfg(doc)]
use rl_engine::prelude::*;
#[cfg(doc)]
use rl_engine::rl_rules::loot::LootTable;

/// The one kind of hireling, by its name in `monsters.ron`.
pub const MERCENARY: &str = "mercenary";

/// The stream a visit's candidates are drawn from, the visit's number its
/// index: apart from every other draw, so who waits in the tavern never
/// moves what a floor holds or a shelf stocks.
pub const TAVERN_STREAM: &[u8] = b"undermere.tavern";

/// The glyph of the marks in `mereholt.ron` a candidate waits at, one
/// beside each of the tavern's tables: as many candidates a visit as there
/// are marks.
pub const TABLE: char = 'x';

/// The verb a candidate offers to be hired by.
pub const HIRE: &str = "hire";

/// What hiring takes, in hundredths of a step: one step, as talking does.
pub const HIRE_TIME: u32 = 100;

/// How many may follow the adventurer before no candidate offers
/// [`HIRE`]: a party of three is as many as a corridor fights in.
pub const PARTY: usize = 2;

/// The tone a mercenary's words are said in, its kind's own colour,
/// declared by [`declare_tone`].
pub const TONE: &str = "mercenary";

/// What `UNDERMERE_NEAR` names to stand the adventurer in the tavern beside
/// the candidates, for a photograph of them.
pub const NEAR_CANDIDATES: &str = "candidates";
