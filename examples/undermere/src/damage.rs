//! Undermere's own stage of mitigation: run after the engine's
//! `SubtractArmor`, so armor absorbs at most half of each blow.
//!
//! Part 2's soak found that armor, which subtracts flat and is enchanted a
//! point a level, made floors one to eight nearly harmless to a delve that
//! put its scrolls there. [`HalveArmor`] puts back whatever armor took past
//! half of a blow's raw amount, rounded down, so plate and scrolls still
//! matter and nothing is ever immune. `run::prepare` is where it joins the
//! pipeline, after `SubtractArmor`, in the place the design names.
//!
//! Mereholt's peace is not a stage: the town's build marks it
//! [`peaceful`](atglyph::ecs::PlaceBuild::peaceful), and the engine
//! lands every hit on anyone standing in it as nothing, a blow, a shot, a
//! tick of fire or of poison alike, after every stage here has answered.

use atglyph::prelude::all::*;
use atglyph::rules::damage::{DamageKind, DamageStage, Defender, Hit, Resistances};

/// Armor's floor against an armored kind: never more than half of the
/// blow's own roll, rounded down, taken off by armor. A blow of 1 always
/// deals 1, and a blow of 9 always deals at least 5, however much armor
/// stands against it.
///
/// Reads `hit.amount`, the roll before any stage touched it, rather than
/// what `ApplyResistance` already left: the half the design means is the
/// blow's whole, the attacker's own roll, not what a defender's own
/// resistance took off first. A kind that ignores armor is untouched, as
/// `SubtractArmor` leaves it.
///
/// Never above what resistance alone left of the blow, though: the floor
/// puts back what armor took, and a defender resisting more than half
/// had less than the floor left before armor touched it, so a floor of the
/// whole roll's half would have made its armor raise the damage it took.
pub struct HalveArmor;

impl<A: Copy> DamageStage<A> for HalveArmor {
    fn apply(&self, hit: &Hit<A>, _defender: &Defender, resistances: &Resistances, kinds: &Registry<DamageKind>, amount: i32) -> i32 {
        if hit.amount <= 0 || !kinds.get(hit.kind).armored {
            return amount;
        }
        let resisted = hit.amount - (i64::from(hit.amount) * i64::from(resistances.get(hit.kind)) / 100) as i32;
        let floor = (hit.amount - hit.amount / 2).min(resisted);
        amount.max(floor)
    }
}

#[cfg(test)]
mod tests {
    use atglyph::primitives::RunSeed;
    use atglyph::rules::damage::{ApplyResistance, SubtractArmor, resolve};
    use bevy::prelude::App;

    use super::*;
    use crate::dungeon::{TOWN, map_of};
    use crate::testing::{adventurer, arrive, headless, here, settle, sturdy};

    fn kinds() -> Registry<DamageKind> {
        Registry::from_defs(vec![DamageKind::new("slash"), DamageKind::new("fire").unarmored()]).unwrap()
    }

    /// Armor never takes more than half of a blow, rounded down, however
    /// much of it stands between the blow and its target: a blow of 1
    /// always deals 1, and a blow of 9 always deals at least 5.
    #[test]
    fn armor_never_absorbs_more_than_half_a_blow_rounded_down() {
        let k = kinds();
        let slash = k.expect("slash");
        let stages: [&dyn DamageStage<u32>; 3] = [&ApplyResistance, &SubtractArmor, &HalveArmor];
        for armor in [0, 1, 2, 3, 5, 10, 50, 1000] {
            let defender = Defender { armor, blocked: false };
            let r = Resistances::new();
            let one = resolve(&Hit::by(1u32, slash, 1), &defender, &r, &k, &stages);
            assert_eq!(one, 1, "a blow of 1 against armor {armor}");
            let nine = resolve(&Hit::by(1u32, slash, 9), &defender, &r, &k, &stages);
            assert!(nine >= 5, "a blow of 9 against armor {armor} dealt {nine}, less than half");
        }
    }

    /// An unarmored kind, fire, is never touched by the armor floor: armor
    /// never applied to it in the first place, so there is nothing for the
    /// floor to put back.
    #[test]
    fn an_unarmored_kind_is_untouched_by_the_armor_floor() {
        let k = kinds();
        let fire = k.expect("fire");
        let stages: [&dyn DamageStage<u32>; 3] = [&ApplyResistance, &SubtractArmor, &HalveArmor];
        let defender = Defender { armor: 1000, blocked: false };
        let amount = resolve(&Hit::by(1u32, fire, 9), &defender, &Resistances::new(), &k, &stages);
        assert_eq!(amount, 9, "armor never touches fire, so the floor has nothing to restore");
    }

    /// Armor never raises what a blow deals: against a defender resisting
    /// more than half of it, the floor is what resistance alone left, so
    /// the same blow with armor deals no more than without, for every
    /// resistance from none to immune and past it.
    #[test]
    fn armor_never_makes_a_resisted_blow_deal_more() {
        let k = kinds();
        let slash = k.expect("slash");
        let stages: [&dyn DamageStage<u32>; 3] = [&ApplyResistance, &SubtractArmor, &HalveArmor];
        for pct in (0..=150).step_by(10) {
            let mut r = Resistances::new();
            r.set(slash, pct);
            for roll in 1..=20 {
                let bare = resolve(&Hit::by(1u32, slash, roll), &Defender { armor: 0, blocked: false }, &r, &k, &stages);
                for armor in [1, 3, 10] {
                    let armored = resolve(&Hit::by(1u32, slash, roll), &Defender { armor, blocked: false }, &r, &k, &stages);
                    assert!(armored <= bare, "resist {pct}, roll {roll}, armor {armor}: {armored} armored against {bare} bare");
                }
            }
        }
    }

    /// With no armor at all, the floor changes nothing: `SubtractArmor`
    /// already left the whole blow standing.
    #[test]
    fn with_no_armor_the_floor_leaves_a_blow_exactly_as_it_was() {
        let k = kinds();
        let slash = k.expect("slash");
        let stages: [&dyn DamageStage<u32>; 3] = [&ApplyResistance, &SubtractArmor, &HalveArmor];
        let defender = Defender { armor: 0, blocked: false };
        let amount = resolve(&Hit::by(1u32, slash, 7), &defender, &Resistances::new(), &k, &stages);
        assert_eq!(amount, 7);
    }

    /// A blow lands nowhere in Mereholt: a hit that would otherwise cost
    /// the adventurer health leaves them exactly as whole as it found
    /// them. The same hit, against the same stages, still costs them
    /// something once they have gone down to the first floor, where
    /// armor and the roll decide how much rather than the stage.
    #[test]
    fn a_blow_harms_nobody_in_mereholt_but_the_same_blow_still_harms_below() {
        let mut app = headless(RunSeed(0));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        assert_eq!(here(&app), TOWN, "a fresh run sets out from Mereholt");
        let slash = app.world().resource::<Registries>().damage_kinds.expect("slash");
        let health = |app: &App| app.world().get::<Health>(me).map(|h| h.current);
        let full = health(&app);
        app.world_mut().write_message(DamageEvent::new(me, Hit::by(me, slash, 50)));
        settle(&mut app);
        assert_eq!(health(&app), full, "nothing is harmed in Mereholt");

        arrive(&mut app, map_of(1));
        app.world_mut().write_message(DamageEvent::new(me, Hit::by(me, slash, 50)));
        settle(&mut app);
        assert!(health(&app) < full, "the same blow still harms once they have left for the floor below");
    }

    /// A firebolt, a fire-kind hit, is stopped the same way a blow is:
    /// nothing in Mereholt, and still a hit for its whole roll below,
    /// since fire goes through armor and resistance starts at nought.
    #[test]
    fn a_firebolt_harms_nobody_in_mereholt_but_the_same_fire_still_harms_below() {
        let mut app = headless(RunSeed(1));
        let me = adventurer(&mut app);
        sturdy(&mut app, me);
        assert_eq!(here(&app), TOWN, "a fresh run sets out from Mereholt");
        let fire = app.world().resource::<Registries>().damage_kinds.expect("fire");
        let health = |app: &App| app.world().get::<Health>(me).map(|h| h.current);
        let full = health(&app);
        app.world_mut().write_message(DamageEvent::new(me, Hit::by(me, fire, 50)));
        settle(&mut app);
        assert_eq!(health(&app), full, "a firebolt does nothing in Mereholt");

        arrive(&mut app, map_of(1));
        app.world_mut().write_message(DamageEvent::new(me, Hit::by(me, fire, 50)));
        settle(&mut app);
        assert_eq!(health(&app), full.map(|h| h - 50), "and still burns for its whole roll once they have left");
    }
}
