//! Stats and the modifiers that stack on them.
//!
//! A stat is a registered id. A modifier is one of four operations on one
//! stat. An actor's [`Stats`] holds base values and every modifier from
//! gear, statuses and traits, and computes the final value on demand:
//! adds, then multipliers compounded, then floors and caps. Adding a stat
//! is a registry entry, not a code change.

use crate::content::{Named, Registry};
use rl_core::Id;
use serde::{Deserialize, Serialize};

/// A registered stat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatDef {
    /// The name content refers to it by.
    pub name: String,
    /// The value an actor has with nothing set.
    #[serde(default)]
    pub base: i32,
    /// The lowest final value, if any.
    #[serde(default)]
    pub min: Option<i32>,
    /// The highest final value, if any.
    #[serde(default)]
    pub max: Option<i32>,
}

impl StatDef {
    /// A stat with a base and no bounds.
    pub fn new(name: impl Into<String>, base: i32) -> Self {
        Self { name: name.into(), base, min: None, max: None }
    }

    /// Bounds the final value.
    pub fn clamp(mut self, min: i32, max: i32) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }
}

impl Named for StatDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered stat id.
pub type StatId = Id<StatDef>;

/// What a modifier does to a stat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    /// Adds to the base. Every add is summed.
    Add(i32),
    /// Multiplies by a percentage, 100 being no change. Multipliers
    /// compound after the adds.
    MulPct(i32),
    /// The final value is at least this.
    AtLeast(i32),
    /// The final value is at most this.
    AtMost(i32),
}

/// One change to one stat, tagged with where it came from so it can be
/// removed when the source goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Modifier {
    /// Which stat.
    pub stat: StatId,
    /// What it does.
    pub op: Op,
    /// Who applied it: an item, a status, a trait. Opaque to the engine.
    pub source: u64,
}

impl Modifier {
    /// A modifier from `source`.
    pub const fn new(stat: StatId, op: Op, source: u64) -> Self {
        Self { stat, op, source }
    }
}

/// An actor's base values and active modifiers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Stats {
    base: Vec<Option<i32>>,
    modifiers: Vec<Modifier>,
}

impl Stats {
    /// Stats with nothing set: every stat reads its definition's base.
    pub fn new() -> Self {
        Self::default()
    }

    /// Overrides the base of `stat` for this actor.
    pub fn set_base(&mut self, stat: StatId, value: i32) {
        let i = stat.index();
        if self.base.len() <= i {
            self.base.resize(i + 1, None);
        }
        self.base[i] = Some(value);
    }

    /// This actor's base for `stat`, or the definition's.
    pub fn base(&self, stat: StatId, defs: &Registry<StatDef>) -> i32 {
        self.base.get(stat.index()).copied().flatten().unwrap_or_else(|| defs.get(stat).base)
    }

    /// Adds a modifier.
    pub fn add(&mut self, m: Modifier) {
        self.modifiers.push(m);
    }

    /// Removes every modifier from `source`. Returns how many went.
    pub fn remove_source(&mut self, source: u64) -> usize {
        let before = self.modifiers.len();
        self.modifiers.retain(|m| m.source != source);
        before - self.modifiers.len()
    }

    /// Removes every modifier whose source fails `keep`. Returns how many went.
    pub fn retain_sources(&mut self, keep: impl Fn(u64) -> bool) -> usize {
        let before = self.modifiers.len();
        self.modifiers.retain(|m| keep(m.source));
        before - self.modifiers.len()
    }

    /// Every active modifier.
    pub fn modifiers(&self) -> &[Modifier] {
        &self.modifiers
    }

    /// The final value of `stat`: base plus adds, times each multiplier
    /// in turn, then floors and caps, then the definition's bounds.
    pub fn value(&self, stat: StatId, defs: &Registry<StatDef>) -> i32 {
        let mut total = self.base(stat, defs) as i64;
        let mine = || self.modifiers.iter().filter(move |m| m.stat == stat);
        for m in mine() {
            if let Op::Add(n) = m.op {
                total += n as i64;
            }
        }
        for m in mine() {
            if let Op::MulPct(pct) = m.op {
                total = total * pct as i64 / 100;
            }
        }
        for m in mine() {
            match m.op {
                Op::AtLeast(n) => total = total.max(n as i64),
                Op::AtMost(n) => total = total.min(n as i64),
                _ => {}
            }
        }
        let def = defs.get(stat);
        if let Some(min) = def.min {
            total = total.max(min as i64);
        }
        if let Some(max) = def.max {
            total = total.min(max as i64);
        }
        total.clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> Registry<StatDef> {
        Registry::from_defs(vec![StatDef::new("hp", 10), StatDef::new("armor", 0).clamp(0, 90), StatDef::new("speed", 100)]).unwrap()
    }

    #[test]
    fn adds_then_multipliers_then_bounds() {
        let d = defs();
        let hp = d.expect("hp");
        let mut s = Stats::new();
        assert_eq!(s.value(hp, &d), 10);
        s.add(Modifier::new(hp, Op::Add(5), 1));
        s.add(Modifier::new(hp, Op::MulPct(200), 2));
        s.add(Modifier::new(hp, Op::Add(-3), 3));
        assert_eq!(s.value(hp, &d), 24, "(10 + 5 - 3) * 2");
        s.add(Modifier::new(hp, Op::AtMost(20), 4));
        assert_eq!(s.value(hp, &d), 20);
        assert_eq!(s.remove_source(2), 1);
        assert_eq!(s.value(hp, &d), 12);
    }

    #[test]
    fn per_actor_base_and_definition_bounds() {
        let d = defs();
        let armor = d.expect("armor");
        let mut s = Stats::new();
        s.set_base(armor, 50);
        s.add(Modifier::new(armor, Op::Add(70), 9));
        assert_eq!(s.value(armor, &d), 90, "capped by the definition");
        s.add(Modifier::new(armor, Op::Add(-500), 10));
        assert_eq!(s.value(armor, &d), 0);
        assert_eq!(s.base(d.expect("speed"), &d), 100);
    }

    #[test]
    fn stats_load_from_ron() {
        let r: Registry<StatDef> = Registry::from_ron_str(r#"[(name: "hp", base: 20), (name: "dodge", min: Some(0), max: Some(75))]"#).unwrap();
        assert_eq!(r.get(r.expect("dodge")).max, Some(75));
        assert_eq!(r.get(r.expect("hp")).base, 20);
    }
}
