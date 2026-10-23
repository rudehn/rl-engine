//! Who is hostile to whom.

use std::collections::BTreeMap;

use crate::content::Named;
use rl_core::Id;
use serde::{Deserialize, Serialize};

/// A registered faction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactionDef {
    /// The name content refers to it by.
    pub name: String,
}

impl FactionDef {
    /// A faction called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Named for FactionDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered faction id.
pub type FactionId = Id<FactionDef>;

/// How one faction regards another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Relation {
    /// Attacks on sight.
    Hostile,
    /// Ignores.
    Neutral,
    /// Helps.
    Allied,
}

/// Who regards whom how: the relations a game named, and a rule for the
/// rest.
///
/// A side is allied with itself and neutral to every other until told
/// otherwise, so a side never named needs no entry, and the table needs no
/// registry to be built: a side made from a raw id, as a game with no
/// faction file makes them, is answered by the same rule. Relations need
/// not be symmetric: bandits may hunt merchants who merely fear them.
///
/// Only what was named is kept, in a `BTreeMap` rather than a matrix sized
/// to a registry, because the named pairs are few and a matrix both
/// demanded the registry up front and panicked on a side it was not sized
/// for; ordered so that a saved table reads the same every time.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Factions {
    named: BTreeMap<(FactionId, FactionId), Relation>,
}

impl Factions {
    /// No relation named: every side allied with itself and neutral to
    /// the rest.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets how `a` regards `b`.
    pub fn set(&mut self, a: FactionId, b: FactionId, r: Relation) {
        self.named.insert((a, b), r);
    }

    /// Sets the relation both ways.
    pub fn set_mutual(&mut self, a: FactionId, b: FactionId, r: Relation) {
        self.set(a, b, r);
        self.set(b, a, r);
    }

    /// How `a` regards `b`: as named, or else allied when they are the
    /// same side and neutral when they are not.
    pub fn relation(&self, a: FactionId, b: FactionId) -> Relation {
        match self.named.get(&(a, b)) {
            Some(&named) => named,
            None if a == b => Relation::Allied,
            None => Relation::Neutral,
        }
    }

    /// Whether `a` attacks `b` on sight.
    pub fn is_hostile(&self, a: FactionId, b: FactionId) -> bool {
        self.relation(a, b) == Relation::Hostile
    }

    /// Whether `a` helps `b`.
    #[cfg(test)]
    pub(crate) fn is_allied(&self, a: FactionId, b: FactionId) -> bool {
        self.relation(a, b) == Relation::Allied
    }
}

/// How side `a` regards side `b`, or `None` when there is nothing to ask.
///
/// A game with no faction table, or an actor that takes no side, has no
/// stance at all, and what that means differs by question (stealth reads
/// it as at odds, a swap as no objection), so the caller keeps its own
/// fallback rather than this picking one. Not symmetric: `a` to `b` and
/// `b` to `a` are two entries of the table.
pub fn stance(factions: Option<&Factions>, a: Option<FactionId>, b: Option<FactionId>) -> Option<Relation> {
    match (factions, a, b) {
        (Some(factions), Some(a), Some(b)) => Some(factions.relation(a, b)),
        _ => None,
    }
}

/// Whether side `a` has anything against side `b`: hostile by the table
/// when there is one and both take a side, and otherwise yes.
///
/// The question stealth asks of a watcher and the one it watches. A game
/// with no sides has nobody to be friends with, so the missing stance
/// reads as at odds; a question where it should read the other way, as a
/// swap does, asks [`stance`] and keeps its own fallback.
pub fn at_odds(factions: Option<&Factions>, a: Option<FactionId>, b: Option<FactionId>) -> bool {
    stance(factions, a, b).is_none_or(|r| r == Relation::Hostile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Registry;

    #[test]
    fn a_side_never_named_is_allied_with_itself_and_neutral_to_others() {
        let (a, b, far) = (FactionId::from_raw(0), FactionId::from_raw(1), FactionId::from_raw(4096));
        let mut f = Factions::new();
        assert_eq!(f.relation(far, far), Relation::Allied);
        assert_eq!(f.relation(a, far), Relation::Neutral);
        f.set(a, b, Relation::Hostile);
        assert_eq!((f.relation(a, b), f.relation(b, a)), (Relation::Hostile, Relation::Neutral));
        f.set(far, far, Relation::Hostile);
        assert!(f.is_hostile(far, far), "a side named hostile to itself is, as one at war within");
    }

    #[test]
    fn relations_default_and_can_be_asymmetric() {
        let r = Registry::from_defs(vec![FactionDef::new("navy"), FactionDef::new("pirates"), FactionDef::new("merchants")]).unwrap();
        let (navy, pirates, merchants) = (r.expect("navy"), r.expect("pirates"), r.expect("merchants"));
        let mut f = Factions::new();
        assert!(f.is_allied(navy, navy));
        assert_eq!(f.relation(navy, pirates), Relation::Neutral);
        f.set_mutual(navy, pirates, Relation::Hostile);
        f.set(pirates, merchants, Relation::Hostile);
        assert!(f.is_hostile(navy, pirates) && f.is_hostile(pirates, navy));
        assert!(f.is_hostile(pirates, merchants));
        assert!(!f.is_hostile(merchants, pirates), "merchants only fear them");
        let text = ron::to_string(&f).unwrap();
        let back: Factions = ron::from_str(&text).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn stance_is_none_when_the_rules_or_either_side_is_missing() {
        let defs = Registry::from_defs(vec![FactionDef::new("a"), FactionDef::new("b")]).unwrap();
        let (a, b) = (defs.expect("a"), defs.expect("b"));
        let f = Factions::new();
        assert_eq!(stance(None, Some(a), Some(b)), None);
        assert_eq!(stance(Some(&f), None, Some(b)), None);
        assert_eq!(stance(Some(&f), Some(a), None), None);
    }

    #[test]
    fn with_no_sides_everyone_is_at_odds() {
        let (a, b) = (FactionId::from_raw(0), FactionId::from_raw(1));
        let f = Factions::new();
        assert!(at_odds(None, Some(a), Some(b)), "no table");
        assert!(at_odds(None, Some(a), Some(a)), "no table, even one side with itself");
        assert!(at_odds(Some(&f), None, Some(b)), "the first takes no side");
        assert!(at_odds(Some(&f), Some(a), None), "the second takes no side");
        assert!(at_odds(None, None, None));
    }

    #[test]
    fn hostile_sides_are_at_odds_and_neutral_or_allied_ones_are_not() {
        let (a, b, c) = (FactionId::from_raw(0), FactionId::from_raw(1), FactionId::from_raw(2));
        let mut f = Factions::new();
        f.set(a, b, Relation::Hostile);
        f.set(a, c, Relation::Allied);
        assert!(at_odds(Some(&f), Some(a), Some(b)));
        assert!(!at_odds(Some(&f), Some(b), Some(a)), "neutral by the rule for the unnamed");
        assert!(!at_odds(Some(&f), Some(a), Some(c)));
        assert!(!at_odds(Some(&f), Some(a), Some(a)), "a side is allied with itself");
    }

    #[test]
    fn stance_reads_the_table_from_the_first_side_to_the_second() {
        let defs = Registry::from_defs(vec![FactionDef::new("a"), FactionDef::new("b")]).unwrap();
        let (a, b) = (defs.expect("a"), defs.expect("b"));
        let mut f = Factions::new();
        f.set(a, b, Relation::Hostile);
        f.set(b, a, Relation::Neutral);
        assert_eq!(stance(Some(&f), Some(a), Some(b)), Some(Relation::Hostile));
        assert_eq!(stance(Some(&f), Some(b), Some(a)), Some(Relation::Neutral));
    }
}
