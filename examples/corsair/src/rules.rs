//! Everything Corsair's content is written in, loaded in the order it names
//! itself.
//!
//! The vocabulary comes first: damage kinds, sides, stats, item tags and
//! slots, the registries that name nothing. Then what is written in those
//! words, each file loaded against everything before it: statuses, then the
//! abilities, then the items and their affixes, which name the ability a
//! bottle lends, then the monsters, which name all of them, and last the
//! ledger's tasks. A name in any file becomes an id
//! as it loads, so a typo stops the game at startup saying which file and
//! which entry, and nothing looks a name up again during play.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{Point, RunSeed};
use rl_engine::rl_rules::damage::DamageKind;
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::{Registry, SlotDef, StatDef, TagDef};

use crate::items::Armory;
use crate::monsters::Bestiary;
use crate::quests::Facts;

/// Every table Corsair loads, before any of it is handed to the world.
pub struct Loaded {
    /// The engine's registries: kinds, sides, stats, statuses, tags, slots.
    pub registries: Registries,
    /// Who is at war with whom.
    pub combat: CombatRules,
    /// The items and their affixes.
    pub armory: Armory,
    /// The abilities, with their effects built.
    pub abilities: Abilities,
    /// The monsters and their spawn table.
    pub bestiary: Bestiary,
    /// The ledger's tasks.
    pub quests: Quests,
    /// The facts the tasks are written over.
    pub facts: Facts,
}

/// Loads every table against `effects`, the effect kinds registered while
/// the app was built; panics naming every problem in whichever file has one.
pub fn load(seed: RunSeed, home: Point, effects: &EffectKinds) -> Loaded {
    let mut registries = Registries {
        damage_kinds: Registry::from_defs(vec![
            DamageKind::new("cutlass"),
            DamageKind::new("pistol"),
            DamageKind::new("bite"),
            DamageKind::new("claw"),
            DamageKind::new("fist"),
            DamageKind::new("fire"),
            // What a swig mends with: nothing in the way of it.
            DamageKind::new("care").unarmored(),
        ])
        .unwrap(),
        factions: Registry::from_defs(vec![FactionDef::new("player"), FactionDef::new("beasts"), FactionDef::new("cutthroats"), FactionDef::new("navy")])
            .unwrap(),
        stats: Registry::from_defs(vec![StatDef::new("armor", 0).clamp(0, 20), StatDef::new("attack", 0)]).unwrap(),
        tags: Registry::from_defs(["weapon", "blade", "gun", "armor", "shield", "hat", "powder", "rum"].map(TagDef::new).to_vec()).unwrap(),
        slots: Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("off hand"), SlotDef::new("body"), SlotDef::new("head")]).unwrap(),
        ..default()
    };
    registries.statuses = crate::statuses::load(&registries.names());
    let abilities = crate::abilities::load(&registries.names(), effects);
    // The engine's moments are all an item here answers; Corsair declares
    // none of its own.
    let armory = Armory::load(seed, home, &registries, effects, &Moments::default());
    let bestiary = Bestiary::load(seed, home, &registries.names().with("item", &armory.defs).with("ability", abilities.defs()), registries.slots.len());
    let (quests, facts) = crate::quests::load(&bestiary, &armory, &registries);
    let combat = relations(&registries);
    Loaded { registries, combat, armory, abilities, bestiary, quests, facts }
}

/// Who is at war with whom, and which stats a blow reads: "armor" is what
/// a status like hearty and an affix like Stout harden, "attack" what Sharp
/// adds to a swing.
fn relations(registries: &Registries) -> CombatRules {
    let sides = &registries.factions;
    let (player, beasts, cutthroats, navy) = (sides.expect("player"), sides.expect("beasts"), sides.expect("cutthroats"), sides.expect("navy"));
    CombatRules::new(sides)
        .hostile(player, beasts)
        .hostile(player, cutthroats)
        .hostile(player, navy)
        .hostile(beasts, cutthroats)
        .hostile(beasts, navy)
        // The navy hunts pirates; pirates would rather not meet the navy.
        .hunts(navy, cutthroats)
        .armor_stat(registries.stats.expect("armor"))
        .attack_stat(registries.stats.expect("attack"))
}

/// The engine's effects and Corsair's own, for loading outside the app: the
/// balance report and the tests.
pub fn effect_kinds() -> EffectKinds {
    let mut app = App::new();
    app.add_engine_effects().add_effect::<crate::abilities::Plunder>();
    app.world_mut().remove_resource::<EffectKinds>().expect("registering an effect makes the table")
}
