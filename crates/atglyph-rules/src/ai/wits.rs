//! What a mind is able to do, whatever its brain would like.
//!
//! A brain is what a monster prefers; its wits are what it can manage. The
//! engine's tactics and moves ask the wits before they act: a mindless thing
//! given [`FleeWhenHurt`](crate::ai::tactics::FleeWhenHurt) never runs, and
//! only a mind that opens doors walks or paths through one. So one brain can
//! serve a whole bestiary, and each kind's wits say how much of it it uses.
//!
//! A set of capabilities rather than a ladder of levels, because a level is
//! only a preset over them. [`Wits::MINDLESS`], [`Wits::ANIMAL`] and
//! [`Wits::SAPIENT`] are the three a game starts from, and a swarm that opens
//! doors but never runs is `Wits::MINDLESS.with(Wits::OPENS_DOORS)`, with no
//! enum anywhere to gain a variant.
//!
//! In content it is written as a preset or a list, and a list may mix the
//! two:
//!
//! ```
//! use atglyph_rules::ai::Wits;
//!
//! let animal: Wits = ron::from_str(r#""animal""#).unwrap();
//! assert!(animal.has(Wits::FLEES) && !animal.has(Wits::OPENS_DOORS));
//! let doorkeeper: Wits = ron::from_str(r#"["mindless", "opens_doors"]"#).unwrap();
//! assert_eq!(doorkeeper, Wits::MINDLESS.with(Wits::OPENS_DOORS));
//! ```

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// What a mind can do. See the [module docs](self).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Wits(u16);

impl Wits {
    /// Runs when hurt.
    pub const FLEES: Wits = Wits(1 << 0);
    /// Follows a trail it has lost to where its quarry was last seen.
    pub const SEARCHES: Wits = Wits(1 << 1);
    /// Opens and closes doors, and paths through a closed one.
    pub const OPENS_DOORS: Wits = Wits(1 << 2);
    /// Picks up what is worth having from where it lies.
    pub const PICKS_UP: Wits = Wits(1 << 3);
    /// Puts on gear that is better than what it wears.
    pub const EQUIPS: Wits = Wits(1 << 4);
    /// Throws what it carries.
    pub const THROWS: Wits = Wits(1 << 5);
    /// Goes through a way to another map after whoever it is hunting or
    /// keeping beside, when it stood next to them as they went. In no
    /// preset, because a monster that leaves the map it was put on is
    /// something a game says of a kind on purpose.
    pub const FOLLOWS_THROUGH: Wits = Wits(1 << 6);
    /// Uses what it carries, by [`UseCarried`](crate::ai::tactics::UseCarried):
    /// on itself what mends once it is hurt, and what is pointed at what
    /// its aim wants.
    pub const USES: Wits = Wits(1 << 7);
    /// Steps round a trap it can see: a cell under a shown thing that goes
    /// off when walked onto costs it as much as a long way round, so its
    /// ways lead round one wherever a reasonable way round exists, and
    /// over it only where none does, the way a player walks a corridor
    /// they cannot avoid. A sapient mind's, and not an animal's, since a
    /// beast blundering onto a plate it could have seen is what a player
    /// lures one there for. Any shown thing with an `entered` trigger that
    /// can still go off is priced, whatever it does, a healing glyph as
    /// much as a dart plate, since what a trigger lands is the game's.
    pub const WARY: Wits = Wits(1 << 8);
    /// Goes through a way to another map of its own accord: told of the
    /// ways it can see, so [`FleeWhenHurt`](crate::ai::tactics::FleeWhenHurt)
    /// runs for one to get away, and let through one it stands on. In no
    /// preset, for the reason [`Wits::FOLLOWS_THROUGH`] is not.
    ///
    /// Apart from following, because a game may want one without the
    /// other: a hireling that comes down the stairs behind the player and
    /// never runs down them alone.
    pub const FLEES_THROUGH: Wits = Wits(1 << 9);
    /// Both ways of leaving a map, [`Wits::FOLLOWS_THROUGH`] and
    /// [`Wits::FLEES_THROUGH`]: what content means by `travels`, so
    /// `["sapient", "travels"]` is a kind that follows and flees through.
    pub const TRAVELS: Wits = Wits(Self::FOLLOWS_THROUGH.0 | Self::FLEES_THROUGH.0);

    /// Nothing but what its brain does unconditionally: it strikes, hunts
    /// what it can perceive and wanders, and forgets you the moment it
    /// cannot see you.
    pub const MINDLESS: Wits = Wits(0);
    /// Runs when hurt and searches for what it lost, but a door is a wall
    /// and a knife on the floor is nothing to it.
    pub const ANIMAL: Wits = Wits(Self::FLEES.0 | Self::SEARCHES.0);
    /// Everything the engine knows how to ask about on the map it stands
    /// on: doors, picking up, wearing, throwing and using things, and
    /// stepping round a trap in plain sight. Leaving that map is
    /// [`Wits::FOLLOWS_THROUGH`] and [`Wits::FLEES_THROUGH`], which no
    /// preset has.
    pub const SAPIENT: Wits = Wits(Self::ANIMAL.0 | Self::OPENS_DOORS.0 | Self::PICKS_UP.0 | Self::EQUIPS.0 | Self::THROWS.0 | Self::USES.0 | Self::WARY.0);

    /// Whether every capability in `wit` is here.
    pub const fn has(self, wit: Wits) -> bool {
        self.0 & wit.0 == wit.0
    }

    /// Whether a mind of these wits takes anything up from where it lies:
    /// gear to put on, or something to throw, which it must both pick up
    /// and throw to want. What [`Scavenge`](crate::ai::tactics::Scavenge)
    /// fetches for, and so what a kind needs to carry and wear at all.
    pub const fn takes_things(self) -> bool {
        self.has(Wits::EQUIPS) || self.has(Wits::PICKS_UP.with(Wits::THROWS))
    }

    /// These wits with `wit` added.
    pub const fn with(self, wit: Wits) -> Wits {
        Wits(self.0 | wit.0)
    }

    /// These wits with `wit` taken away.
    pub const fn without(self, wit: Wits) -> Wits {
        Wits(self.0 & !wit.0)
    }

    /// The wits or preset called `name` in content, if there is one.
    pub(crate) fn named(name: &str) -> Option<Wits> {
        PRESETS.iter().chain(CAPABILITIES.iter()).chain(PAIRS.iter()).find(|(n, _)| *n == name).map(|(_, w)| *w)
    }

    /// The name of every capability here, in declaration order.
    pub(crate) fn names(self) -> impl Iterator<Item = &'static str> {
        CAPABILITIES.into_iter().filter(move |(_, w)| self.has(*w)).map(|(n, _)| n)
    }
}

/// Sapient: a mind nobody flagged does everything its brain asks, which is
/// what a game that has never heard of wits already expects of it.
impl Default for Wits {
    fn default() -> Self {
        Self::SAPIENT
    }
}

/// The presets by the names content writes them with.
const PRESETS: [(&str, Wits); 3] = [("mindless", Wits::MINDLESS), ("animal", Wits::ANIMAL), ("sapient", Wits::SAPIENT)];

/// Each capability by the name content writes it with.
const CAPABILITIES: [(&str, Wits); 10] = [
    ("flees", Wits::FLEES),
    ("searches", Wits::SEARCHES),
    ("opens_doors", Wits::OPENS_DOORS),
    ("picks_up", Wits::PICKS_UP),
    ("equips", Wits::EQUIPS),
    ("throws", Wits::THROWS),
    ("follows_through", Wits::FOLLOWS_THROUGH),
    ("uses", Wits::USES),
    ("wary", Wits::WARY),
    ("flees_through", Wits::FLEES_THROUGH),
];

/// Capabilities content names together under one word, read like any
/// other and never written back, since the list of what they stand for
/// says the same: `travels` is both ways of leaving a map, as it was when
/// it was one wit.
const PAIRS: [(&str, Wits); 1] = [("travels", Wits::TRAVELS)];

/// Every name content may use, for a message that lists them.
fn every_name() -> String {
    let presets: Vec<&str> = PRESETS.iter().map(|(n, _)| *n).collect();
    let wits: Vec<&str> = CAPABILITIES.iter().chain(PAIRS.iter()).map(|(n, _)| *n).collect();
    format!("the presets are {} and the wits {}", presets.join(", "), wits.join(", "))
}

/// A preset's name when these are exactly one, and otherwise the list, so a
/// saved or printed value reads the way an author would have written it.
impl Serialize for Wits {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match PRESETS.iter().find(|(_, w)| w == self) {
            Some((name, _)) => serializer.serialize_str(name),
            None => serializer.collect_seq(self.names()),
        }
    }
}

impl<'de> Deserialize<'de> for Wits {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(WitsVisitor)
    }
}

struct WitsVisitor;

impl<'de> Visitor<'de> for WitsVisitor {
    type Value = Wits;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "a preset or a list of wits; {}", every_name())
    }

    fn visit_str<E: de::Error>(self, name: &str) -> Result<Wits, E> {
        read_name(name)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut names: A) -> Result<Wits, A::Error> {
        let mut wits = Wits::MINDLESS;
        while let Some(name) = names.next_element::<String>()? {
            wits = wits.with(read_name(&name)?);
        }
        Ok(wits)
    }
}

/// The wits called `name`, or an error naming every one there is.
fn read_name<E: de::Error>(name: &str) -> Result<Wits, E> {
    Wits::named(name).ok_or_else(|| E::custom(format!("no wits called {name:?}; {}", every_name())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_preset_can_do_everything_the_one_below_it_can() {
        assert!(Wits::ANIMAL.has(Wits::MINDLESS));
        assert!(Wits::SAPIENT.has(Wits::ANIMAL));
        assert!(!Wits::ANIMAL.has(Wits::OPENS_DOORS));
        assert!(!Wits::MINDLESS.has(Wits::FLEES));
        assert!(Wits::SAPIENT.has(Wits::USES) && !Wits::ANIMAL.has(Wits::USES), "only a sapient mind uses what it carries");
        assert!(!Wits::SAPIENT.has(Wits::FOLLOWS_THROUGH) && !Wits::SAPIENT.has(Wits::FLEES_THROUGH), "and no preset leaves its map");
        assert!(Wits::SAPIENT.has(Wits::WARY) && !Wits::ANIMAL.has(Wits::WARY), "only a sapient mind steps round a trap it sees");
        assert_eq!(Wits::named("wary"), Some(Wits::WARY));
        assert_eq!(Wits::named("uses"), Some(Wits::USES));
        assert_eq!(Wits::SAPIENT.without(Wits::FLEES).with(Wits::FLEES), Wits::SAPIENT);
        assert_eq!(Wits::default(), Wits::SAPIENT, "a mind nobody flagged does what its brain asks");
    }

    #[test]
    fn wits_read_as_a_preset_or_a_list_and_write_back_the_same_way() {
        for (name, wits) in PRESETS {
            let text = format!("{name:?}");
            assert_eq!(ron::from_str::<Wits>(&text).unwrap(), wits);
            assert_eq!(ron::to_string(&wits).unwrap(), text, "a preset writes as its name");
        }
        let odd: Wits = ron::from_str(r#"["flees", "opens_doors"]"#).unwrap();
        assert_eq!(odd, Wits::FLEES.with(Wits::OPENS_DOORS));
        assert_eq!(ron::from_str::<Wits>(&ron::to_string(&odd).unwrap()).unwrap(), odd, "and anything else as the list");
        assert_eq!(ron::from_str::<Wits>("[]").unwrap(), Wits::MINDLESS);
    }

    /// `travels`, written before leaving a map was two wits, still means
    /// both, so nothing written with it changes meaning; and either half
    /// may be written alone.
    #[test]
    fn travels_in_content_is_both_following_and_fleeing_through_and_each_half_reads_alone() {
        let travels: Wits = ron::from_str(r#"["sapient", "travels"]"#).unwrap();
        assert!(travels.has(Wits::FOLLOWS_THROUGH) && travels.has(Wits::FLEES_THROUGH), "both halves");
        assert_eq!(travels, Wits::SAPIENT.with(Wits::TRAVELS));
        let follows: Wits = ron::from_str(r#"["sapient", "follows_through"]"#).unwrap();
        assert!(follows.has(Wits::FOLLOWS_THROUGH) && !follows.has(Wits::FLEES_THROUGH), "one half alone");
        assert_eq!(ron::from_str::<Wits>(&ron::to_string(&travels).unwrap()).unwrap(), travels, "and written back as its halves");
        assert_eq!(Wits::TRAVELS.without(Wits::FLEES_THROUGH), Wits::FOLLOWS_THROUGH, "taking one half off leaves the other");
    }

    #[test]
    fn a_wit_nobody_defined_is_refused_naming_every_one_there_is() {
        let err = ron::from_str::<Wits>(r#""cunning""#).unwrap_err().to_string();
        assert!(err.contains("cunning") && err.contains("sapient") && err.contains("opens_doors"), "{err}");
        assert!(ron::from_str::<Wits>(r#"["animal", "sneaky"]"#).is_err());
    }
}
