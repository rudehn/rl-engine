//! Who is hostile to whom.

use crate::content::{Named, Registry};
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

/// A dense relation matrix. Factions are allied with themselves and
/// neutral to everyone else until told otherwise. Relations need not be
/// symmetric: bandits may hunt merchants who merely fear them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Factions {
    count: usize,
    relations: Vec<Relation>,
}

impl Factions {
    /// Relations for `registry`'s factions.
    pub fn new(registry: &Registry<FactionDef>) -> Self {
        let n = registry.len();
        let mut relations = vec![Relation::Neutral; n * n];
        for i in 0..n {
            relations[i * n + i] = Relation::Allied;
        }
        Self { count: n, relations }
    }

    /// Sets how `a` regards `b`.
    pub fn set(&mut self, a: FactionId, b: FactionId, r: Relation) {
        self.relations[a.index() * self.count + b.index()] = r;
    }

    /// Sets the relation both ways.
    pub fn set_mutual(&mut self, a: FactionId, b: FactionId, r: Relation) {
        self.set(a, b, r);
        self.set(b, a, r);
    }

    /// How `a` regards `b`.
    pub fn relation(&self, a: FactionId, b: FactionId) -> Relation {
        self.relations[a.index() * self.count + b.index()]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relations_default_and_can_be_asymmetric() {
        let r = Registry::from_defs(vec![FactionDef::new("navy"), FactionDef::new("pirates"), FactionDef::new("merchants")]).unwrap();
        let (navy, pirates, merchants) = (r.expect("navy"), r.expect("pirates"), r.expect("merchants"));
        let mut f = Factions::new(&r);
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
        let f = Factions::new(&defs);
        assert_eq!(stance(None, Some(a), Some(b)), None);
        assert_eq!(stance(Some(&f), None, Some(b)), None);
        assert_eq!(stance(Some(&f), Some(a), None), None);
    }

    #[test]
    fn stance_reads_the_table_from_the_first_side_to_the_second() {
        let defs = Registry::from_defs(vec![FactionDef::new("a"), FactionDef::new("b")]).unwrap();
        let (a, b) = (defs.expect("a"), defs.expect("b"));
        let mut f = Factions::new(&defs);
        f.set(a, b, Relation::Hostile);
        f.set(b, a, Relation::Neutral);
        assert_eq!(stance(Some(&f), Some(a), Some(b)), Some(Relation::Hostile));
        assert_eq!(stance(Some(&f), Some(b), Some(a)), Some(Relation::Neutral));
    }
}
