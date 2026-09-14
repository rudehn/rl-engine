//! Where a content file's names are looked up.
//!
//! A definition in RON names other content: an ability names the stat it
//! spends and the status it requires, a status names the stat it modifies
//! and the damage kind it ticks, an affix names tags, stats and kinds. Every
//! such name is resolved once, at load, so nothing compares strings during
//! play and a typo fails at startup saying what was wrong.
//!
//! [`Names`] borrows whichever registries a game has, and every loader in
//! this crate reads through it. A game fills it rather than implementing
//! anything, which is why there is no lookup per game and no RON-facing
//! mirror of an engine type.

use rl_core::Id;

use crate::affix::{TagDef, TagId};
use crate::content::Registry;
use crate::damage::{DamageKind, DamageKindId};
use crate::equip::{SlotDef, SlotId};
use crate::stats::{StatDef, StatId};
use crate::status::{StatusDef, StatusId};

/// The registries a content file's names resolve against, borrowed for one
/// load.
///
/// Any of them may be left out. A name looked up in a registry that was
/// never given is reported as unknown, and says so, rather than panicking:
/// a game with no statuses loads abilities that name none, and one that
/// names a status anyway is told there is nothing to find it in.
///
/// Loads chain, because content names content: statuses name stats, and
/// abilities name statuses.
///
/// ```
/// use rl_rules::{DamageKind, Names, Registry, StatDef, ability, status};
///
/// let stats = Registry::from_defs(vec![StatDef::new("mana", 20)]).unwrap();
/// let kinds = Registry::from_defs(vec![DamageKind::new("fire")]).unwrap();
/// let names = Names::new().stats(&stats).damage_kinds(&kinds);
///
/// let statuses = status::load(r#"[(name: "burning", ticks: Some(("fire", 2)))]"#, &names).unwrap();
/// let names = names.statuses(&statuses);
/// let abilities = ability::load(r#"[(name: "ignite", mode: Adjacent, costs: [Pool("mana", 4)])]"#, &names).unwrap();
///
/// assert_eq!(statuses.get(statuses.expect("burning")).tick_damage, Some((kinds.expect("fire"), 2)));
/// assert_eq!(abilities.len(), 1);
/// assert!(names.tag("powder").is_err(), "no tags were given");
/// ```
#[derive(Clone, Copy, Default)]
pub struct Names<'a> {
    stats: Option<&'a Registry<StatDef>>,
    statuses: Option<&'a Registry<StatusDef>>,
    tags: Option<&'a Registry<TagDef>>,
    slots: Option<&'a Registry<SlotDef>>,
    damage_kinds: Option<&'a Registry<DamageKind>>,
}

impl<'a> Names<'a> {
    /// Nothing to look anything up in.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stats, for a cost, a requirement or a modifier.
    pub fn stats(mut self, stats: &'a Registry<StatDef>) -> Self {
        self.stats = Some(stats);
        self
    }

    /// Statuses, for a requirement or an effect that inflicts one.
    pub fn statuses(mut self, statuses: &'a Registry<StatusDef>) -> Self {
        self.statuses = Some(statuses);
        self
    }

    /// Item tags, for an item cost, a requirement or an affix.
    pub fn tags(mut self, tags: &'a Registry<TagDef>) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Equipment slots, for a requirement.
    pub fn slots(mut self, slots: &'a Registry<SlotDef>) -> Self {
        self.slots = Some(slots);
        self
    }

    /// Damage kinds, for a tick, a strike or an effect that deals one.
    pub fn damage_kinds(mut self, damage_kinds: &'a Registry<DamageKind>) -> Self {
        self.damage_kinds = Some(damage_kinds);
        self
    }

    /// The stat named `name`, or why there is none.
    pub fn stat(&self, name: &str) -> Result<StatId, String> {
        find(self.stats, "stat", name)
    }

    /// The status named `name`, or why there is none.
    pub fn status(&self, name: &str) -> Result<StatusId, String> {
        find(self.statuses, "status", name)
    }

    /// The tag named `name`, or why there is none.
    pub fn tag(&self, name: &str) -> Result<TagId, String> {
        find(self.tags, "tag", name)
    }

    /// The slot named `name`, or why there is none.
    pub fn slot(&self, name: &str) -> Result<SlotId, String> {
        find(self.slots, "slot", name)
    }

    /// The damage kind named `name`, or why there is none.
    pub fn damage_kind(&self, name: &str) -> Result<DamageKindId, String> {
        find(self.damage_kinds, "damage kind", name)
    }
}

/// One lookup, with the message a content author reads when it fails.
fn find<T>(registry: Option<&Registry<T>>, what: &str, name: &str) -> Result<Id<T>, String> {
    match registry {
        Some(r) => r.id(name).ok_or_else(|| format!("unknown {what} {name:?}")),
        None => Err(format!("unknown {what} {name:?}: no {what} registry was given to look it up in")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_resolves_to_the_id_its_registry_issued() {
        let stats = Registry::from_defs(vec![StatDef::new("mana", 10), StatDef::new("nerve", 5)]).unwrap();
        let names = Names::new().stats(&stats);
        assert_eq!(names.stat("nerve"), Ok(stats.expect("nerve")));
        assert_eq!(names.stat("wisdom"), Err("unknown stat \"wisdom\"".to_string()));
    }

    #[test]
    fn a_registry_that_was_never_given_is_named_in_the_failure() {
        let names = Names::new();
        let why = names.status("burning").expect_err("there are no statuses");
        assert!(why.starts_with("unknown status \"burning\""), "{why}");
        assert!(why.contains("no status registry was given"), "{why}");
    }
}
