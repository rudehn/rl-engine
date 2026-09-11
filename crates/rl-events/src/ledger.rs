//! Named counters fed by facts.

use rl_content::{Named, Registry};
use rl_core::Id;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, Matcher};

/// A registered counter: "kills", "doubloons found", "turns below".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterDef {
    /// The name content refers to it by.
    pub name: String,
}

impl CounterDef {
    /// A counter called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Named for CounterDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered counter id.
pub type CounterId = Id<CounterDef>;

/// A rule: facts this matches add their amount to this counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    /// Which facts.
    pub on: Matcher,
    /// Which counter.
    pub counter: CounterId,
}

/// The counters and the rules that feed them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ledger {
    values: Vec<i64>,
    tallies: Vec<Tally>,
}

impl Ledger {
    /// A ledger with a zero for every counter in `counters`.
    pub fn new(counters: &Registry<CounterDef>) -> Self {
        Self { values: vec![0; counters.len()], tallies: Vec::new() }
    }

    /// Adds a rule.
    pub fn tally(mut self, on: Matcher, counter: CounterId) -> Self {
        self.push(Tally { on, counter });
        self
    }

    /// Adds a rule.
    pub fn push(&mut self, tally: Tally) {
        assert!(tally.counter.index() < self.values.len(), "counter {:?} was not registered", tally.counter);
        self.tallies.push(tally);
    }

    /// The value of `counter`.
    pub fn get(&self, counter: CounterId) -> i64 {
        self.values.get(counter.index()).copied().unwrap_or(0)
    }

    /// Adds `amount` to `counter` directly.
    pub fn add(&mut self, counter: CounterId, amount: i64) {
        if let Some(v) = self.values.get_mut(counter.index()) {
            *v += amount;
        }
    }

    /// Feeds a fact through every rule. Returns the counters it changed.
    pub fn feed(&mut self, fact: &Fact) -> Vec<CounterId> {
        let mut changed = Vec::new();
        for t in &self.tallies {
            if t.on.matches(fact) {
                self.values[t.counter.index()] += fact.amount;
                if !changed.contains(&t.counter) {
                    changed.push(t.counter);
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact::FactDef;

    #[test]
    fn tallies_sum_amounts_of_matching_facts() {
        let facts = Registry::from_defs(vec![FactDef::new("killed"), FactDef::new("picked up")]).unwrap();
        let counters = Registry::from_defs(vec![CounterDef::new("kills"), CounterDef::new("loot")]).unwrap();
        let (killed, picked) = (facts.expect("killed"), facts.expect("picked up"));
        let (kills, loot) = (counters.expect("kills"), counters.expect("loot"));
        let mut ledger = Ledger::new(&counters).tally(Matcher::any(killed), kills).tally(Matcher::any(picked).about(2), loot);
        assert_eq!(ledger.feed(&Fact::new(killed).about(1)), vec![kills]);
        assert_eq!(ledger.feed(&Fact::new(killed).about(5)), vec![kills]);
        assert_eq!(ledger.feed(&Fact::new(picked).about(2).amount(12)), vec![loot]);
        assert!(ledger.feed(&Fact::new(picked).about(3).amount(4)).is_empty(), "the wrong item");
        assert_eq!(ledger.get(kills), 2);
        assert_eq!(ledger.get(loot), 12);
        ledger.add(loot, -2);
        assert_eq!(ledger.get(loot), 10);
    }
}
