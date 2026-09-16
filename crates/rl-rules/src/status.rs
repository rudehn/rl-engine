//! Statuses as data: a registered definition, and the instances an actor
//! carries with their remaining duration.
//!
//! What a status *does* is expressed as stat modifiers the engine can
//! apply and remove, plus an optional damage-over-time the game's damage
//! pipeline resolves. Anything richer is the game's, keyed by the id.
//!
//! [`load`] reads definitions from RON by name, resolving every stat and
//! damage kind through [`Names`], so a game authors statuses in the words
//! its other content uses and never mirrors this schema in a type of its
//! own.

use crate::content::{ContentError, Named, Registry};
use rl_core::Id;
use serde::{Deserialize, Serialize};

use crate::damage::DamageKindId;
use crate::names::Names;
use crate::stats::{Op, Source, StatId, Stats};

/// What happens when a status is applied to an actor that has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stacking {
    /// The new duration replaces the old if longer.
    Refresh,
    /// Durations add.
    Extend,
    /// A second instance sits beside the first.
    Stack,
    /// The second application is ignored.
    Ignore,
}

/// A stat change a status applies while it lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusModifier {
    /// The stat.
    pub stat: StatId,
    /// The change.
    pub op: Op,
}

/// A registered status.
///
/// Built by [`load`] from names, or by hand with the methods below, and
/// never deserialized as it stands: its ids index registries a content file
/// cannot see, and a number written in one is a modifier that lands on
/// another stat the day the stat list is reordered.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusDef {
    /// The name content refers to it by.
    pub name: String,
    /// How a repeat application behaves.
    pub stacking: Stacking,
    /// Stat changes while active.
    pub modifiers: Vec<StatusModifier>,
    /// Damage per turn as `(kind, amount)`, if any.
    pub tick_damage: Option<(DamageKindId, i32)>,
    /// Glyph for a badge, if the game wants one.
    pub badge: Option<char>,
}

fn refresh() -> Stacking {
    Stacking::Refresh
}

impl StatusDef {
    /// A status with no effects.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), stacking: Stacking::Refresh, modifiers: Vec::new(), tick_damage: None, badge: None }
    }

    /// Sets the stacking rule.
    pub fn stacking(mut self, s: Stacking) -> Self {
        self.stacking = s;
        self
    }

    /// Adds a stat modifier.
    pub fn modifies(mut self, stat: StatId, op: Op) -> Self {
        self.modifiers.push(StatusModifier { stat, op });
        self
    }

    /// Sets damage per turn. A negative amount mends each turn instead,
    /// through the same pipeline, so regeneration is a status like poison.
    pub fn ticks(mut self, kind: DamageKindId, amount: i32) -> Self {
        self.tick_damage = Some((kind, amount));
        self
    }
}

impl Named for StatusDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered status id.
pub type StatusId = Id<StatusDef>;

/// A status as authored, before its names are resolved.
#[derive(Debug, Deserialize)]
struct Authored {
    name: String,
    #[serde(default = "refresh")]
    stacking: Stacking,
    #[serde(default)]
    modifies: Vec<(String, Op)>,
    #[serde(default)]
    ticks: Option<(String, i32)>,
    #[serde(default)]
    badge: Option<char>,
}

impl Named for Authored {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Loads statuses from RON, resolving every stat and damage kind through
/// `names`.
///
/// One entry per status, and every field but `name` may be left out:
///
/// - `name`: unique; what a requirement, an effect or the game's code calls it.
/// - `stacking`: what a second application does. `Refresh`, the default,
///   keeps the longer duration; `Extend` adds them; `Stack` puts a second
///   instance beside the first; `Ignore` does nothing.
/// - `modifies`: stat changes while it lasts, each `(stat, op)` with `op`
///   one of `Add(n)`, `MulPct(n)`, `AtLeast(n)` or `AtMost(n)`.
/// - `ticks`: `(damage kind, amount)` dealt every whole turn. A negative
///   amount mends.
/// - `badge`: one character a panel may draw beside a health bar.
///
/// Reports every unknown name in the file at once.
pub fn load(text: &str, names: &Names<'_>) -> Result<Registry<StatusDef>, ContentError> {
    let authored: Registry<Authored> = Registry::from_ron_str(text)?;
    let mut errors = Vec::new();
    let mut defs = Vec::new();
    for (_, a) in authored.iter() {
        let mut modifiers = Vec::new();
        for (stat, op) in &a.modifies {
            match names.stat(stat) {
                Ok(stat) => modifiers.push(StatusModifier { stat, op: *op }),
                Err(e) => errors.push(format!("{}: {e}", a.name)),
            }
        }
        let tick_damage = a.ticks.as_ref().and_then(|(kind, amount)| match names.damage_kind(kind) {
            Ok(kind) => Some((kind, *amount)),
            Err(e) => {
                errors.push(format!("{}: {e}", a.name));
                None
            }
        });
        defs.push(StatusDef { name: a.name.clone(), stacking: a.stacking, modifiers, tick_damage, badge: a.badge });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

/// One status on one actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveStatus {
    /// Which status.
    pub id: StatusId,
    /// Whole turns left.
    pub turns: u32,
    /// Who applied it, for credit on a tick. Opaque.
    pub source: Option<u64>,
}

/// Damage one status dealt on a tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    /// Which status.
    pub status: StatusId,
    /// The damage kind.
    pub kind: DamageKindId,
    /// How much, before mitigation.
    pub amount: i32,
    /// Who applied the status, for credit.
    pub source: Option<u64>,
}

/// What one turn did to an actor's statuses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TickReport {
    /// Damage to resolve, in status order.
    pub ticks: Vec<Tick>,
    /// Statuses that ran out this turn.
    pub expired: Vec<StatusId>,
}

/// The statuses on one actor.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Statuses {
    active: Vec<ActiveStatus>,
}

/// The source under which a status's modifiers sit in [`Stats`]: unique
/// per status id and instance, so removal is exact.
fn source_tag(id: StatusId, instance: usize) -> Source {
    Source::Status { status: id, instance: instance.min(u16::MAX as usize) as u16 }
}

impl Statuses {
    /// No statuses.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `id` for `turns`, following the definition's stacking rule,
    /// and installs its modifiers into `stats`. Returns whether anything
    /// changed.
    pub fn apply(&mut self, id: StatusId, turns: u32, source: Option<u64>, defs: &Registry<StatusDef>, stats: &mut Stats) -> bool {
        let def = defs.get(id);
        let existing = self.active.iter_mut().find(|s| s.id == id);
        match (def.stacking, existing) {
            (Stacking::Ignore, Some(_)) => false,
            (Stacking::Refresh, Some(s)) => {
                let changed = turns > s.turns;
                s.turns = s.turns.max(turns);
                changed
            }
            (Stacking::Extend, Some(s)) => {
                s.turns = s.turns.saturating_add(turns);
                true
            }
            _ => {
                let instance = self.active.iter().filter(|s| s.id == id).count();
                let tag = source_tag(id, instance);
                for m in &def.modifiers {
                    stats.add(crate::stats::Modifier::new(m.stat, m.op, tag));
                }
                self.active.push(ActiveStatus { id, turns, source });
                true
            }
        }
    }

    /// Whether `id` is active.
    pub fn has(&self, id: StatusId) -> bool {
        self.active.iter().any(|s| s.id == id)
    }

    /// Every active status.
    pub fn iter(&self) -> impl Iterator<Item = &ActiveStatus> {
        self.active.iter()
    }

    /// Advances one turn: collects the damage each status deals, then
    /// expires what ran out and removes its modifiers.
    pub fn tick(&mut self, defs: &Registry<StatusDef>, stats: &mut Stats) -> TickReport {
        let mut ticks = Vec::new();
        for s in &self.active {
            if let Some((kind, amount)) = defs.get(s.id).tick_damage {
                ticks.push(Tick { status: s.id, kind, amount, source: s.source });
            }
        }
        let mut expired = Vec::new();
        let mut kept = Vec::with_capacity(self.active.len());
        let mut instance_of: Vec<(StatusId, usize)> = Vec::new();
        for s in self.active.drain(..) {
            let instance = instance_of.iter().filter(|(id, _)| *id == s.id).count();
            instance_of.push((s.id, instance));
            if s.turns <= 1 {
                stats.remove_source(source_tag(s.id, instance));
                expired.push(s.id);
            } else {
                kept.push(ActiveStatus { turns: s.turns - 1, ..s });
            }
        }
        // Surviving instances keep their original tags: removal above used
        // the instance index at the time of application, which the order of
        // `active` preserves.
        self.active = kept;
        TickReport { ticks, expired }
    }

    /// Removes `id` outright, modifiers included. Returns whether it was there.
    pub fn cure(&mut self, id: StatusId, stats: &mut Stats) -> bool {
        let mut removed = false;
        let mut instance = 0;
        self.active.retain(|s| {
            if s.id == id {
                stats.remove_source(source_tag(id, instance));
                instance += 1;
                removed = true;
                false
            } else {
                true
            }
        });
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::StatDef;

    fn world() -> (Registry<StatDef>, Registry<StatusDef>) {
        let stats = Registry::from_defs(vec![StatDef::new("speed", 100)]).unwrap();
        let speed = stats.expect("speed");
        let statuses = Registry::from_defs(vec![
            StatusDef::new("slowed").modifies(speed, Op::MulPct(50)),
            StatusDef::new("burning").ticks(DamageKindId::from_raw(0), 2).stacking(Stacking::Extend),
            StatusDef::new("poisoned").ticks(DamageKindId::from_raw(1), 1).stacking(Stacking::Stack),
        ])
        .unwrap();
        (stats, statuses)
    }

    #[test]
    fn a_status_modifies_stats_until_it_expires() {
        let (stat_defs, defs) = world();
        let speed = stat_defs.expect("speed");
        let slowed = defs.expect("slowed");
        let mut stats = Stats::new();
        let mut st = Statuses::new();
        assert!(st.apply(slowed, 2, None, &defs, &mut stats));
        assert_eq!(stats.value(speed, &stat_defs), 50);
        assert!(!st.apply(slowed, 1, None, &defs, &mut stats), "a shorter refresh changes nothing");
        assert!(st.apply(slowed, 3, None, &defs, &mut stats));
        for _ in 0..2 {
            assert!(st.tick(&defs, &mut stats).expired.is_empty());
        }
        assert_eq!(st.tick(&defs, &mut stats).expired, vec![slowed]);
        assert_eq!(stats.value(speed, &stat_defs), 100);
        assert!(!st.has(slowed));
    }

    #[test]
    fn ticks_extend_stack_and_cure() {
        let (_, defs) = world();
        let burning = defs.expect("burning");
        let poisoned = defs.expect("poisoned");
        let mut stats = Stats::new();
        let mut st = Statuses::new();
        st.apply(burning, 2, Some(9), &defs, &mut stats);
        st.apply(burning, 2, Some(9), &defs, &mut stats);
        assert_eq!(st.iter().next().unwrap().turns, 4, "extend adds");
        st.apply(poisoned, 3, None, &defs, &mut stats);
        st.apply(poisoned, 3, None, &defs, &mut stats);
        assert_eq!(st.iter().filter(|s| s.id == poisoned).count(), 2, "stack sits beside");
        let report = st.tick(&defs, &mut stats);
        assert_eq!(report.ticks.len(), 3);
        assert_eq!(report.ticks[0], Tick { status: burning, kind: DamageKindId::from_raw(0), amount: 2, source: Some(9) });
        assert!(st.cure(poisoned, &mut stats));
        assert!(!st.has(poisoned));
        assert!(!st.cure(poisoned, &mut stats));
    }

    fn vocabulary() -> (Registry<StatDef>, Registry<crate::damage::DamageKind>) {
        let stats = Registry::from_defs(vec![StatDef::new("speed", 100), StatDef::new("armor", 0)]).unwrap();
        let kinds = Registry::from_defs(vec![crate::damage::DamageKind::new("bite"), crate::damage::DamageKind::new("care")]).unwrap();
        (stats, kinds)
    }

    #[test]
    fn a_status_file_is_written_in_names_and_loaded_as_ids() {
        let (stats, kinds) = vocabulary();
        let names = Names::new().stats(&stats).damage_kinds(&kinds);
        let r = load(
            r#"#![enable(implicit_some)]
            [
                (name: "hasted", modifies: [("speed", MulPct(200))], badge: 'H'),
                (name: "venom", stacking: Extend, ticks: ("bite", 1)),
                (name: "mending", ticks: ("care", -2)),
                (name: "stunned", stacking: Ignore),
            ]"#,
            &names,
        )
        .expect("the file loads");
        let hasted = r.get(r.expect("hasted"));
        assert_eq!(hasted.modifiers, vec![StatusModifier { stat: stats.expect("speed"), op: Op::MulPct(200) }]);
        assert_eq!(hasted.badge, Some('H'));
        assert_eq!(hasted.stacking, Stacking::Refresh, "the default");
        assert_eq!(r.get(r.expect("venom")).tick_damage, Some((kinds.expect("bite"), 1)));
        assert_eq!(r.get(r.expect("mending")).tick_damage, Some((kinds.expect("care"), -2)), "a negative tick mends");
        assert_eq!(r.get(r.expect("stunned")).stacking, Stacking::Ignore);
    }

    #[test]
    fn every_unknown_name_in_a_status_file_is_reported_at_once() {
        let (stats, kinds) = vocabulary();
        let names = Names::new().stats(&stats).damage_kinds(&kinds);
        let bad = r#"#![enable(implicit_some)]
        [
            (name: "cursed", modifies: [("luck", Add(-1)), ("wisdom", Add(-1))]),
            (name: "burning", ticks: ("fire", 2)),
            (name: "fine"),
        ]"#;
        let Err(ContentError::Invalid(errs)) = load(bad, &names) else { panic!("a file of unknown names loaded") };
        assert_eq!(errs.len(), 3, "{errs:#?}");
        assert!(errs.contains(&"cursed: unknown stat \"luck\"".to_string()), "{errs:#?}");
        assert!(errs.contains(&"burning: unknown damage kind \"fire\"".to_string()), "{errs:#?}");
    }
}
