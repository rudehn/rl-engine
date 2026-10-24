//! The swords for hire at the Sunken Lantern: one kind, its names, what
//! each carries and asks, and what one hired is told to do.
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
//! arrival in Mereholt, drawn from the run's [`TAVERN`](crate::seeds::TAVERN) at the
//! visit's number, which [`Tavern`] counts; the last visit's that nobody
//! hired are gone first. Each is a [`Candidate`] made by
//! [`spawn_candidate`]: the one kind on the townsfolk's side, with no mind,
//! wearing its kit and offering [`HIRE`] at its fee beside talk. `hire` is
//! the engine's priced offer, so a walk into a candidate opens the offers
//! screen, an adventurer short of the fee is refused there, and one who
//! takes it up has paid before the game hears of it, in the `Interacted`
//! that [`answer_hire`] answers. [`keep_hire_offered`] takes `hire` off
//! every candidate while the party is full. What a candidate holds is said
//! as the adventurer knows it, by the name each thing carries, when talked
//! to by [`answer_candidates`] and on the look panel by
//! [`show_candidates`].
//!
//! [`hire`] makes a candidate a [`Mercenary`]: on the adventurer's side by
//! [`befriend`](crate::allies::befriend), following them by `Leader`, and
//! offering [`GIVE`] and [`DISMISS`]. It thinks with one of the three
//! brains of [`MercenaryBrains`], built once at load, one for each
//! [`Order`]: follow, hold, fight freely. A hire is sapient, so the
//! engine's `Wits::WARY` walks it round a trap the adventurer has found,
//! and over one in a corridor with no way round. [`order_party`] cycles every
//! mercenary's order at once on the order key, without spending a turn,
//! and [`show_orders`] says the order on the look panel and the party
//! panel, and on the look panel that it is in the adventurer's pay and
//! what it holds.
//!
//! A mercenary never tells the adventurer what they do not know. What it
//! was made with is its [`OwnKit`], which it knows whole and uses freely;
//! anything else it uses only once the adventurer knows it, since
//! [`know_as_the_adventurer_does`] trims what its mind is told it could
//! use and counts gear at what it seems worth, by [`SeenWorth`]. A kind it
//! uses where the adventurer sees it, by [`Witnessed`], is learned as if
//! they had used it, and a curse it puts on there is seen to cling. `give`
//! is the engine's choosing offer: the pack asks which thing to hand over
//! before anything is spent, among what [`give_choices`] names, and
//! [`answer_give`] moves the one chosen; [`PutOnBetter`] puts on whatever
//! in its pack it is better off wearing.
//!
//! [`DISMISS`] asks first, as the engine's asking offer, and costs nothing
//! until the yes, which [`answer_dismiss`] answers by [`dismiss`]: the
//! mercenary sets down whatever it was given and waits to be hired again,
//! a [`Candidate`] on the townsfolk's side asking the fee of the kit it
//! keeps. One
//! that dies is gone for good: the tally lists it among the fallen,
//! [`strip_the_fallen`] takes what made it the party's off its body and
//! marks it [`Fallen`] by name, which the save keeps, and
//! [`say_who_fell`] has the log say it as the adventurer's own loss.
//!
//! The party goes where the adventurer goes, from beside them: on their
//! own `Travelled`, through a stair, the gate or a waystone's call alike,
//! [`follow_through`] sends after them each mercenary near the cell they
//! left with a line to it, onto a free cell beside where they landed, by a
//! warp after them that the narrator tells as following. One left behind,
//! by distance, a wall or the hold order, waits frozen where it stood,
//! still theirs. None flees through a way of its own accord: [`obey`]
//! keeps the engine's wit for it off every mercenary.
//!
//! The run's save keeps a [`Mercenary`] whole, a candidate's name and fee,
//! whose kit is its own and the [`Tavern`]'s visits; the engine keeps who
//! follows whom, in what order and the post one holds. What is not saved
//! is made again: [`serve_again`] gives a mercenary its offers and the
//! brain for its order, and [`wait_again`] gives a candidate its offers.
//!
//! Undermere's own rather than a keeper's [`Wares`](crate::shops::Wares):
//! a kit is a part at a time, each on its own chance, where a shelf is a
//! count of draws; and a kit's level may be below nought, which the
//! engine's [`LevelTable`] refuses for a find, so the levels are a loot
//! table whose item is the level. A part's rows are the engine's spawn
//! file, read by the reader `item_spawns.ron` is, so a row is written and
//! checked the one way.

#[cfg(test)]
mod fixtures;
mod hired;
mod kit;
mod knowing;
mod leaving;
mod orders;
#[cfg(test)]
mod saved;
mod tavern;
mod travel;

pub use hired::*;
pub use kit::*;
pub use knowing::*;
pub use leaving::*;
pub use orders::*;
pub use tavern::*;
pub use travel::*;

#[cfg(doc)]
use crate::armory::Armory;
#[cfg(doc)]
use atglyph::prelude::all::*;
#[cfg(doc)]
use atglyph::rules::loot::LootTable;

/// The one kind of hireling, by its name in `monsters.ron`.
pub const MERCENARY: &str = "mercenary";

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
