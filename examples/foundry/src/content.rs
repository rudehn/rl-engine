//! What Foundry's world is made of: its damage kinds, slots, sides and
//! the two ways a body takes a hit.
//!
//! Every name here is content the engine knows nothing about; the engine
//! sees ids. The two resistance profiles are the design's damage table,
//! kept in one place so a droid and a commando cannot drift from it.

use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::{DamageKind, Registry, Resistances, SlotDef, StatusDef, TagDef};

/// How a body takes a hit.
///
/// `Deserialize` by hand, from a plain string, so a monster's file writes
/// `profile: "chassis" | "organic"` the way every other name in content is
/// written; the derived enum deserializer RON gives a fieldless enum reads
/// a bare, unquoted identifier instead, which is not this file's style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// Plated: shrugs off half a slug, takes a bolt in full, and is undone by ion.
    Chassis,
    /// Flesh in plate: a bolt is blunted, ion barely registers.
    Organic,
}

impl<'de> serde::Deserialize<'de> for Profile {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ProfileVisitor;
        impl serde::de::Visitor<'_> for ProfileVisitor {
            type Value = Profile;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "\"chassis\" or \"organic\"")
            }

            fn visit_str<E: serde::de::Error>(self, name: &str) -> Result<Profile, E> {
                match name {
                    "chassis" => Ok(Profile::Chassis),
                    "organic" => Ok(Profile::Organic),
                    other => Err(E::custom(format!("no profile called {other:?}; expected \"chassis\" or \"organic\""))),
                }
            }
        }
        deserializer.deserialize_str(ProfileVisitor)
    }
}

/// The resistance table for `profile`, as percentages removed.
pub fn resistances(profile: Profile, registries: &Registries) -> Resistances {
    let k = &registries.damage_kinds;
    let (kinetic, energy, ion) = (k.expect("kinetic"), k.expect("energy"), k.expect("ion"));
    let mut r = Resistances::new();
    let (a, b, c) = match profile {
        Profile::Chassis => (50, 0, -100),
        Profile::Organic => (0, 25, 75),
    };
    r.set(kinetic, a);
    r.set(energy, b);
    r.set(ion, c);
    r
}

/// Every registry Foundry's content names, filled before anything is
/// loaded against them.
///
/// Assembled the way `delve`'s `registries` is: one `Registry::from_defs`
/// per table, then the struct literal.
pub fn registries() -> Registries {
    let damage_kinds = Registry::from_defs(vec![
        DamageKind::new("kinetic"),
        DamageKind::new("energy"),
        // Plate does not stop a charge.
        DamageKind::new("ion").unarmored(),
        // The kind a `Mend` is dealt as, like Corsair's and Delve's; no
        // profile resists it, so a heal lands in full.
        DamageKind::new("care").unarmored(),
    ])
    .unwrap();
    let statuses = Registry::from_defs(vec![StatusDef { badge: Some('~'), ..StatusDef::new("sensors down") }]).unwrap();
    Registries {
        damage_kinds,
        statuses,
        factions: Registry::from_defs(vec![FactionDef::new("commando"), FactionDef::new("droids"), FactionDef::new("vermin")]).unwrap(),
        tags: Registry::from_defs(vec![TagDef::new("weapon"), TagDef::new("armor"), TagDef::new("slug")]).unwrap(),
        slots: Registry::from_defs(vec![
            SlotDef::new("main hand"),
            SlotDef::new("off hand"),
            SlotDef::new("head"),
            SlotDef::new("torso"),
            SlotDef::new("arms"),
            SlotDef::new("legs"),
        ])
        .unwrap(),
        ..Registries::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_damage_table_is_the_designs_to_the_percent() {
        // Spec section 5: a droid shrugs off half of a slug and takes double
        // from ion; flesh takes a quarter less from a bolt and nearly nothing
        // from ion. A profile that drifts from this rebalances every fight.
        let r = registries();
        let (kinetic, energy, ion) = (r.damage_kinds.expect("kinetic"), r.damage_kinds.expect("energy"), r.damage_kinds.expect("ion"));
        let chassis = resistances(Profile::Chassis, &r);
        assert_eq!((chassis.get(kinetic), chassis.get(energy), chassis.get(ion)), (50, 0, -100));
        let organic = resistances(Profile::Organic, &r);
        assert_eq!((organic.get(kinetic), organic.get(energy), organic.get(ion)), (0, 25, 75));
    }

    #[test]
    fn every_slot_the_armor_names_is_registered() {
        let r = registries();
        for slot in ["main hand", "off hand", "head", "torso", "arms", "legs"] {
            assert!(r.slots.id(slot).is_some(), "{slot}");
        }
    }
}
