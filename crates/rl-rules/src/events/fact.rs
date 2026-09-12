//! What happened, as a record.

use crate::content::Named;
use rl_core::Id;
use serde::{Deserialize, Serialize};

/// A registered kind of fact: "killed", "picked up", "entered".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactDef {
    /// The name content refers to it by.
    pub name: String,
}

impl FactDef {
    /// A kind called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Named for FactDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered fact kind.
pub type FactKind = Id<FactDef>;

/// One thing that happened.
///
/// Subject and object are opaque to the engine: a game puts a definition
/// id, a faction id, a depth, whatever the kind calls for. Amount is 1
/// unless the fact is about a quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    /// What kind of thing.
    pub kind: FactKind,
    /// Who or what it was about.
    pub subject: Option<u64>,
    /// What it was done to or with.
    pub object: Option<u64>,
    /// How much or how many.
    pub amount: i64,
}

impl Fact {
    /// A fact of `kind` with no subject, no object and an amount of one.
    pub const fn new(kind: FactKind) -> Self {
        Self { kind, subject: None, object: None, amount: 1 }
    }

    /// Sets the subject.
    pub const fn about(mut self, subject: u64) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Sets the object.
    pub const fn with(mut self, object: u64) -> Self {
        self.object = Some(object);
        self
    }

    /// Sets the amount.
    pub const fn amount(mut self, amount: i64) -> Self {
        self.amount = amount;
        self
    }
}

/// Picks facts out: a kind, and optionally a subject and an object that
/// must agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Matcher {
    /// The kind to match.
    pub kind: FactKind,
    /// The subject to match, or any.
    pub subject: Option<u64>,
    /// The object to match, or any.
    pub object: Option<u64>,
}

impl Matcher {
    /// Matches every fact of `kind`.
    pub const fn any(kind: FactKind) -> Self {
        Self { kind, subject: None, object: None }
    }

    /// Requires the subject.
    pub const fn about(mut self, subject: u64) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Requires the object.
    pub const fn with(mut self, object: u64) -> Self {
        self.object = Some(object);
        self
    }

    /// Whether `fact` is one of these.
    pub fn matches(&self, fact: &Fact) -> bool {
        fact.kind == self.kind && self.subject.is_none_or(|s| fact.subject == Some(s)) && self.object.is_none_or(|o| fact.object == Some(o))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matchers_narrow_by_what_they_name() {
        let killed = FactKind::from_raw(0);
        let seen = FactKind::from_raw(1);
        let f = Fact::new(killed).about(3).with(9);
        assert!(Matcher::any(killed).matches(&f));
        assert!(Matcher::any(killed).about(3).matches(&f));
        assert!(!Matcher::any(killed).about(4).matches(&f));
        assert!(Matcher::any(killed).about(3).with(9).matches(&f));
        assert!(!Matcher::any(killed).with(1).matches(&f));
        assert!(!Matcher::any(seen).matches(&f));
        assert!(!Matcher::any(killed).about(3).matches(&Fact::new(killed)), "a fact with no subject is not about anyone");
        assert_eq!(Fact::new(killed).amount, 1);
    }
}
