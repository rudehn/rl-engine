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
//! use rl_rules::ai::Wits;
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

    /// Nothing but what its brain does unconditionally: it strikes, hunts
    /// what it can perceive and wanders, and forgets you the moment it
    /// cannot see you.
    pub const MINDLESS: Wits = Wits(0);
    /// Runs when hurt and searches for what it lost, but a door is a wall.
    pub const ANIMAL: Wits = Wits(Self::FLEES.0 | Self::SEARCHES.0);
    /// Everything the engine knows how to ask about.
    pub const SAPIENT: Wits = Wits(Self::ANIMAL.0 | Self::OPENS_DOORS.0);

    /// Whether every capability in `wit` is here.
    pub const fn has(self, wit: Wits) -> bool {
        self.0 & wit.0 == wit.0
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
    pub fn named(name: &str) -> Option<Wits> {
        PRESETS.iter().chain(CAPABILITIES.iter()).find(|(n, _)| *n == name).map(|(_, w)| *w)
    }

    /// The name of every capability here, in declaration order.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
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
const CAPABILITIES: [(&str, Wits); 3] = [("flees", Wits::FLEES), ("searches", Wits::SEARCHES), ("opens_doors", Wits::OPENS_DOORS)];

/// Every name content may use, for a message that lists them.
fn every_name() -> String {
    let presets: Vec<&str> = PRESETS.iter().map(|(n, _)| *n).collect();
    let wits: Vec<&str> = CAPABILITIES.iter().map(|(n, _)| *n).collect();
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

    #[test]
    fn a_wit_nobody_defined_is_refused_naming_every_one_there_is() {
        let err = ron::from_str::<Wits>(r#""cunning""#).unwrap_err().to_string();
        assert!(err.contains("cunning") && err.contains("sapient") && err.contains("opens_doors"), "{err}");
        assert!(ron::from_str::<Wits>(r#"["animal", "sneaky"]"#).is_err());
    }
}
