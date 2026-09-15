//! Every registry the engine's subsystems read, in one resource.
//!
//! Damage kinds, factions, stats, statuses, item tags and equipment slots
//! are content: a game fills them, and combat, statuses, abilities, items
//! and the panels read them. They live once, here, rather than in a
//! resource per subsystem the game copies its own tables into, which is how
//! two copies of one registry come to disagree about what an id means.
//!
//! [`Registries::names`] is the same tables as a [`Names`], so a game loads
//! its statuses, abilities and its own definitions against exactly what the
//! engine will read them with.

use bevy::prelude::*;
use rl_rules::damage::DamageKind;
use rl_rules::faction::FactionDef;
use rl_rules::{Names, Registry, SlotDef, StatDef, StatusDef, TagDef};

/// Every registry the engine reads, filled by the game before play begins.
///
/// A registry a game has no use for stays empty, and an empty one means
/// none: no statuses means no badges on a health bar, no slots means
/// nothing is worn. A plugin that reads one says so with `needs`, so a
/// game that forgot the whole resource is told when play begins.
///
/// Registries name each other, so a game fills them in order: the ones that
/// name nothing first, then the ones loaded through [`names`](Self::names).
///
/// ```
/// use rl_bevy::Registries;
/// use rl_rules::{DamageKind, Registry, StatDef, status};
///
/// let mut registries = Registries {
///     stats: Registry::from_defs(vec![StatDef::new("armor", 0)]).unwrap(),
///     damage_kinds: Registry::from_defs(vec![DamageKind::new("venom")]).unwrap(),
///     ..Default::default()
/// };
/// registries.statuses = status::load(r#"[(name: "poisoned", ticks: Some(("venom", 1)))]"#, &registries.names()).unwrap();
/// assert!(registries.statuses.id("poisoned").is_some());
/// ```
#[derive(Resource, Debug, Clone, Default)]
pub struct Registries {
    /// What damage can be.
    pub damage_kinds: Registry<DamageKind>,
    /// The sides an actor can be on. Who is hostile to whom is
    /// [`CombatRules`](crate::combat::CombatRules), which is a rule rather
    /// than content.
    pub factions: Registry<FactionDef>,
    /// The stats every [`StatBlock`](crate::status::StatBlock) is read against.
    pub stats: Registry<StatDef>,
    /// The statuses an actor can carry.
    pub statuses: Registry<StatusDef>,
    /// The tags an item can carry.
    pub tags: Registry<TagDef>,
    /// The equipment slots, in the order a panel lists them.
    pub slots: Registry<SlotDef>,
}

impl Registries {
    /// Every registry, to load content against.
    pub fn names(&self) -> Names<'_> {
        Names::new()
            .damage_kinds(&self.damage_kinds)
            .with("faction", &self.factions)
            .stats(&self.stats)
            .statuses(&self.statuses)
            .tags(&self.tags)
            .slots(&self.slots)
    }
}
