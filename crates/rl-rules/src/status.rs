//! Statuses as data: a registered definition, and the instances an actor
//! carries with their remaining duration.
//!
//! What a status *does* is expressed as stat modifiers the engine can
//! apply and remove, plus an optional damage-over-time the game's damage
//! pipeline resolves. Anything richer is the game's, keyed by the id.

use crate::content::{Named, Registry};
use rl_core::Id;
use serde::{Deserialize, Serialize};

use crate::stats::{Op, Stats};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusModifier {
    /// The stat, by name in RON and resolved to an id at load.
    pub stat: u32,
    /// The change.
    pub op: Op,
}

/// A registered status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusDef {
    /// The name content refers to it by.
    pub name: String,
    /// How a repeat application behaves.
    #[serde(default = "refresh")]
    pub stacking: Stacking,
    /// Stat changes while active, as raw stat ids.
    #[serde(default)]
    pub modifiers: Vec<StatusModifier>,
    /// Damage per turn as `(damage kind id, amount)`, if any.
    #[serde(default)]
    pub tick_damage: Option<(u32, i32)>,
    /// Glyph for a badge, if the game wants one.
    #[serde(default)]
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
    pub fn modifies(mut self, stat: u32, op: Op) -> Self {
        self.modifiers.push(StatusModifier { stat, op });
        self
    }

    /// Sets damage per turn.
    pub fn ticks(mut self, kind: u32, amount: i32) -> Self {
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
    /// The damage kind, as a raw id for the game's damage registry.
    pub kind: u32,
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

/// The high bits every status source tag carries.
const STATUS_TAG: u64 = 0x5747_0000_0000_0000;

/// The source tag under which a status's modifiers sit in [`Stats`]:
/// unique per status id so removal is exact.
fn source_tag(id: StatusId, instance: usize) -> u64 {
    STATUS_TAG | ((id.raw() as u64) << 16) | instance as u64
}

/// Whether a modifier source is a status's, so a game rebuilding its
/// gear modifiers can leave the statuses' in place.
pub fn is_status_source(source: u64) -> bool {
    source & 0xFFFF_0000_0000_0000 == STATUS_TAG
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
                    stats.add(crate::stats::Modifier::new(Id::from_raw(m.stat), m.op, tag));
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
        let speed = stats.expect("speed").raw();
        let statuses = Registry::from_defs(vec![
            StatusDef::new("slowed").modifies(speed, Op::MulPct(50)),
            StatusDef::new("burning").ticks(0, 2).stacking(Stacking::Extend),
            StatusDef::new("poisoned").ticks(1, 1).stacking(Stacking::Stack),
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
        assert_eq!(report.ticks[0], Tick { status: burning, kind: 0, amount: 2, source: Some(9) });
        assert!(st.cure(poisoned, &mut stats));
        assert!(!st.has(poisoned));
        assert!(!st.cure(poisoned, &mut stats));
    }

    #[test]
    fn statuses_load_from_ron() {
        let r: Registry<StatusDef> =
            Registry::from_ron_str(r#"[(name: "hasted", modifiers: [(stat: 2, op: MulPct(200))], badge: Some('H')), (name: "stunned", stacking: Ignore)]"#)
                .unwrap();
        assert_eq!(r.get(r.expect("hasted")).badge, Some('H'));
        assert_eq!(r.get(r.expect("stunned")).stacking, Stacking::Ignore);
    }
}
