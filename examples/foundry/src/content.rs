//! What Foundry's world is made of: its damage kinds, slots, sides and
//! the two ways a body takes a hit.
//!
//! Every name here is content the engine knows nothing about; the engine
//! sees ids. The two resistance profiles are the design's damage table,
//! kept in one place so a droid and a commando cannot drift from it.

use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::DiceRoll;
use rl_engine::rl_rules::ability::Look;
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::gas::GasDef;
use rl_engine::rl_rules::{DamageKind, NameRef, Registry, Resistances, SlotDef, StatusDef, TagDef};
use serde::Deserialize;

/// What one turn of `mending` gives back.
///
/// The medkit's half of the medical pair: a stim closes a wound now, and
/// a medkit gives twice as much at this a turn, which the commando has to
/// stay alive to collect. Here rather than in `statuses.ron`, which
/// Foundry has none of, and read by `assets/abilities.ron` only through
/// how long it inflicts the status for, which
/// `a_medkit_is_worth_twice_a_stim_spread_over_ten_turns` holds to.
pub const MEND_PER_TURN: i32 = 2;

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

/// A blow as a content file writes it: `(roll: "1d6", kind: "kinetic")`,
/// with an optional `look`.
///
/// Named fields rather than a tuple, so the look sits inside the attack
/// it belongs to, and a droid that punches and shoots says which of the
/// two a look is for. The look is the engine's own, written the way an
/// ability's is in `abilities.ron`: a look is one thing wherever it is.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct MeleeDef {
    /// The damage roll, as `"NdS+B"`.
    pub roll: DiceRoll,
    /// The damage kind.
    pub kind: NameRef<DamageKind>,
    /// The colour it bursts in on whoever it strikes; absent, it shows
    /// nothing.
    #[serde(default)]
    pub look: Option<Look>,
}

impl MeleeDef {
    /// The engine's attack for it, at the ordinary cost.
    pub fn attack(&self) -> MeleeAttack {
        MeleeAttack { look: self.look, ..MeleeAttack::new(self.kind.id(), self.roll) }
    }
}

/// A shot as a content file writes it: `(range: 5, roll: "1d6", kind:
/// "energy")`, with an optional `look`, for the reasons [`MeleeDef`] has
/// named fields.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct RangedDef {
    /// The furthest cell it reaches.
    pub range: i32,
    /// The damage roll, as `"NdS+B"`.
    pub roll: DiceRoll,
    /// The damage kind.
    pub kind: NameRef<DamageKind>,
    /// What flies from the shooter to the target; absent, a shot lands
    /// unseen.
    #[serde(default)]
    pub look: Option<Look>,
}

impl RangedDef {
    /// The engine's attack for it, at the ordinary cost.
    pub fn attack(&self) -> RangedAttack {
        RangedAttack { look: self.look, ..RangedAttack::new(self.kind.id(), self.roll, self.range) }
    }
}

/// The resistance table for `profile`, as percentages removed.
pub fn resistances(profile: Profile, registries: &Registries) -> Resistances {
    let k = &registries.damage_kinds;
    let (kinetic, energy, ion, electricity) = (k.expect("kinetic"), k.expect("energy"), k.expect("ion"), k.expect("electricity"));
    let mut r = Resistances::new();
    let (a, b, c, d) = match profile {
        Profile::Chassis => (50, 0, -100, 0),
        Profile::Organic => (0, 25, 75, 0),
    };
    r.set(kinetic, a);
    r.set(energy, b);
    r.set(ion, c);
    r.set(electricity, d);
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
        // Raw current: a live cable, a capacitor let go. Plate conducts
        // it, so only a resist takes any off, and it is not ion: flesh and
        // chassis take it alike, and it jams nothing.
        DamageKind::new("electricity").unarmored(),
        // Fire, and what an incendiary burns with. Unarmored for the reason
        // electricity is: plate heats through, and a tick of `scorched`
        // that armor could stop would never tick at all.
        DamageKind::new("thermal").unarmored(),
        // The kind a `Mend` is dealt as, like Corsair's and Delve's; no
        // profile resists it, so a heal lands in full.
        DamageKind::new("care").unarmored(),
    ])
    .unwrap();
    // Mending is what a medkit's gel does: two a turn through the same
    // pipeline a wound comes in by, so plate and resistances have nothing
    // to say about it and the commando reads the gain in the log a blow
    // reads in. `Stacking::Refresh` is the default and is the rule that
    // matters here: a second medkit puts the clock back to ten turns
    // rather than mending four a turn, so a pack of them is a longer
    // recovery and never a faster one.
    let care = damage_kinds.expect("care");
    // Scorched is what standing in fire leaves on whoever stood there, a
    // point a turn for as long as `run::start`'s `FireRules` says.
    let thermal = damage_kinds.expect("thermal");
    let statuses = Registry::from_defs(vec![
        StatusDef { badge: Some('~'), ..StatusDef::new("sensors down") },
        StatusDef { badge: Some('+'), ..StatusDef::new("mending").ticks(care, -MEND_PER_TURN) },
        StatusDef { badge: Some('^'), ..StatusDef::new("scorched").ticks(thermal, 1) },
    ])
    .unwrap();
    // Smoke is the one gas: what a smoke grenade throws and what anything
    // burning gives off, thick enough to hide in while it hangs.
    let gases = Registry::from_defs(vec![GasDef::new("smoke").spread(55).fade(9).veils_at(70)]).unwrap();
    let mut registries = Registries {
        damage_kinds,
        statuses,
        gases,
        factions: Registry::from_defs(vec![FactionDef::new("commando"), FactionDef::new("droids"), FactionDef::new("vermin")]).unwrap(),
        tags: Registry::from_defs(vec![TagDef::new("weapon"), TagDef::new("armor"), TagDef::new("slug"), TagDef::new("keycard")]).unwrap(),
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
    };
    // Props name the tags above, so they are loaded once those exist, the
    // way the statuses and the armory are.
    registries.props = crate::props::load(&registries);
    registries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_damage_table_is_the_designs_to_the_percent() {
        // Spec section 5: a droid shrugs off half of a slug and takes double
        // from ion; flesh takes a quarter less from a bolt and nearly nothing
        // from ion; electricity hurts both alike, which is what keeps it a
        // different idea from ion. A profile that drifts from this
        // rebalances every fight.
        let r = registries();
        let k = &r.damage_kinds;
        let (kinetic, energy, ion, electricity) = (k.expect("kinetic"), k.expect("energy"), k.expect("ion"), k.expect("electricity"));
        let table = |p| {
            let t = resistances(p, &r);
            (t.get(kinetic), t.get(energy), t.get(ion), t.get(electricity))
        };
        assert_eq!(table(Profile::Chassis), (50, 0, -100, 0));
        assert_eq!(table(Profile::Organic), (0, 25, 75, 0));
    }

    /// The design's table of what each attack flies as, to the byte: a
    /// blaster's bolt, an ion pistol's charge, a slug. Walks every item
    /// and every monster in the files, so a new gun without a row here, or
    /// a blade or a bite given a look, fails it.
    #[test]
    fn every_attack_in_the_files_flies_the_look_the_design_gives_it_and_no_blow_bursts() {
        use bevy::ecs::world::CommandQueue;
        use bevy::prelude::*;
        use rl_engine::rl_core::{Point, RunSeed};
        use rl_engine::rl_grid::Rgb;
        let bolt = Some(('*', Rgb::new(255, 77, 38)));
        let slug = Some(('o', Rgb::new(242, 204, 115)));
        let table = [
            ("hand blaster", bolt),
            ("blaster carbine", bolt),
            ("ion pistol", Some(('~', Rgb::new(89, 166, 255)))),
            ("slug pistol", slug),
            ("slug rifle", slug),
            ("heavy repeater", bolt),
            ("line droid", None),
            ("probe droid", None),
            ("trooper droid", bolt),
            ("heavy droid", bolt),
        ];
        let flies = |name: &str| table.iter().find(|(n, _)| *n == name).and_then(|(_, look)| *look);
        let mut app = crate::testing::headless(RunSeed(1));
        let registries = app.world().resource::<Registries>().clone();
        let armory = crate::testing::armory_of(&app);
        let roster = crate::droids::Roster::load(&registries);
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world_mut());
        let mut spawned: Vec<(String, Entity)> =
            armory.defs.iter().map(|(id, d)| (d.name.clone(), crate::gear::spawn_item(&mut commands, &armory, id, &registries))).collect();
        let map = MapId::SURFACE;
        spawned.extend(
            roster.defs.iter().map(|(id, d)| (d.name.clone(), crate::droids::spawn_monster(&mut commands, &roster, id, Point::ZERO, map, &registries))),
        );
        queue.apply(app.world_mut());
        for (name, e) in spawned {
            let fired = app.world().get::<RangedAttack>(e).and_then(|r| r.look).map(|l| (l.glyph, l.color));
            assert_eq!(fired, flies(&name), "{name}");
            assert_eq!(app.world().get::<MeleeAttack>(e).and_then(|m| m.look), None, "{name} bursts on nothing yet");
        }
    }

    #[test]
    fn every_slot_the_armor_names_is_registered() {
        let r = registries();
        for slot in ["main hand", "off hand", "head", "torso", "arms", "legs"] {
            assert!(r.slots.id(slot).is_some(), "{slot}");
        }
    }
}
