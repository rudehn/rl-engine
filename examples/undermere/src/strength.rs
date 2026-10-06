//! What heavy gear asks of its wearer, and what it costs to wear it short.
//!
//! A weapon or a piece of armor may name a `strength` in `items.ron`. For
//! each point the adventurer's strength falls short of it, a weapon costs
//! them [`SHORT_ACCURACY`] accuracy and armor [`SHORT_ARMOR`] armor, a
//! body, a shield and a helm each counted, and none costing more than it
//! gives. [`keep_strength`] keeps those costs on the adventurer's own
//! stats, as modifiers of the game's own, worked out afresh every pass
//! from what they wear and how strong they are now, so putting a hammer
//! down, drinking a potion of strength or a continued run all read right
//! at once with nothing to remember.
//!
//! Strength itself is a stat whose base is the adventurer's twelve; the
//! potion of strength's [`Strengthen`] raises the base, for good. Only the
//! adventurer pays: a goblin's strength is never read, and what it wears
//! it wears as well as anyone.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_rules::ability::RawValue;
use rl_engine::rl_rules::stats::Source;

use crate::armory::{Armory, ItemKind};
use crate::content::{ACCURACY, ARMOR, STRENGTH};
use crate::run::Adventurer;

/// Accuracy each point of strength short costs a weapon's wielder.
pub const SHORT_ACCURACY: i32 = 4;

/// Armor each point of strength short costs a piece of armor's wearer.
pub const SHORT_ARMOR: i32 = 1;

/// What the costs of wearing too heavy a thing are tagged with on the
/// wearer's stats: the game's own, so the engine's fold of what is worn
/// and its statuses leave them alone, and nothing but [`keep_strength`]
/// takes them off.
pub const TOO_HEAVY: Source = Source::Game(1);

/// What wearing `worn` costs a wearer of `strength`, in accuracy and in
/// armor: the shortfall of each thing that asks more strength than that,
/// a weapon's in accuracy and armor's in armor, each thing once however
/// many slots it fills.
///
/// A piece of armor never costs more than the armor it gives, read off
/// the piece as it is, its level counted: armor subtracts from every blow,
/// so armor below nothing would add to every blow, and too heavy a coat is
/// no coat rather than one that draws blows.
pub fn shortfall(worn: &[Entity], strength: i32, things: &Query<(&ItemKind, Option<&Armor>)>, armory: &Armory) -> (i32, i32) {
    let (mut accuracy, mut armor) = (0, 0);
    for (kind, gives) in worn.iter().filter_map(|item| things.get(*item).ok()) {
        let d = armory.defs.get(kind.0);
        let short = d.strength.map_or(0, |needs| (needs - strength).max(0));
        if d.melee.is_some() || d.ranged.is_some() {
            accuracy += SHORT_ACCURACY * short;
        }
        if d.armor != 0 {
            armor += (SHORT_ARMOR * short).min(gives.map_or(0, |a| a.0).max(0));
        }
    }
    (accuracy, armor)
}

/// Keeps what the adventurer's too-heavy gear costs them on their stats:
/// [`TOO_HEAVY`] modifiers on accuracy and armor, as [`shortfall`] works
/// them out from what they wear and their strength as it stands.
///
/// Every pass in `TurnSet::React`, deriving the costs rather than answering
/// each way they could change, as the light a worn thing lends is kept:
/// gear put on, taken off, pushed off or restored from a save, a strength
/// raised or lowered, the next pass reads it as it stands. Idempotent:
/// costs that already read right are left alone, so the stats change only
/// when the costs do.
pub fn keep_strength(
    registries: Res<Registries>,
    armory: Res<Armory>,
    mut wearers: Query<(&Equipped, &mut StatBlock), With<Adventurer>>,
    kinds: Query<(&ItemKind, Option<&Armor>)>,
) {
    let stat = |name: &str| registries.stats.expect(name);
    let (strength, accuracy, armor) = (stat(STRENGTH), stat(ACCURACY), stat(ARMOR));
    for (equipped, mut stats) in &mut wearers {
        let mut worn: Vec<Entity> = Vec::new();
        for (_, item) in equipped.0.worn() {
            if !worn.contains(&item) {
                worn.push(item);
            }
        }
        let (less_accuracy, less_armor) = shortfall(&worn, stats.0.value(strength, &registries.stats), &kinds, &armory);
        let mut costs: Vec<Modifier> = Vec::new();
        if less_accuracy > 0 {
            costs.push(Modifier::new(accuracy, Op::Add(-less_accuracy), TOO_HEAVY));
        }
        if less_armor > 0 {
            costs.push(Modifier::new(armor, Op::Add(-less_armor), TOO_HEAVY));
        }
        let now: Vec<Modifier> = stats.0.modifiers().iter().copied().filter(|m| m.source == TOO_HEAVY).collect();
        if now != costs {
            stats.0.remove_source(TOO_HEAVY);
            for cost in costs {
                stats.0.add(cost);
            }
        }
    }
}

/// Raise the strength of everyone under the footprint by `by`, for good:
/// the potion of strength's. A base raised, not a modifier added, since
/// nothing ever takes it back off.
#[derive(Debug, Clone, Copy)]
pub struct Strengthen {
    /// How much.
    pub by: i32,
}

impl Effect for Strengthen {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        let (targets, by) = (landing.targets.clone(), self.by);
        world.commands.queue(move |world: &mut World| {
            world.resource_scope(|world, registries: Mut<Registries>| {
                let Some(strength) = registries.stats.id(STRENGTH) else { return };
                for target in targets {
                    let Some(mut stats) = world.get_mut::<StatBlock>(target) else { continue };
                    let base = stats.0.base(strength, &registries.stats);
                    stats.0.set_base(strength, base + by);
                    if world.get::<Player>(target).is_some() {
                        let (line, tone) = if by >= 0 { ("You feel stronger.", Tones::GOOD) } else { ("You feel weaker.", Tones::BAD) };
                        world.write_message(Tell::new(line, tone));
                    }
                }
            });
        });
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        format!("strength {:+} for good", self.by)
    }
}

impl FromArgs for Strengthen {
    const KIND: &'static str = "Strengthen";

    fn from_args(args: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Args {
            by: i32,
        }
        let a: Args = read_args(args)?;
        Ok(Self { by: a.by })
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::testing::{adventurer, at, clear_monsters, equip, find, give, headless, lay_floor, make, monster, said, use_thing};

    /// `who`'s value of the stat `name`.
    fn stat(app: &App, who: Entity, name: &str) -> i32 {
        let registries = app.world().resource::<Registries>();
        app.world().get::<StatBlock>(who).expect("stats").0.value(registries.stats.expect(name), &registries.stats)
    }

    /// The chance, in percent, of `me`'s blow landing on `foe`, as the
    /// hit model reads it.
    fn chance(app: &mut App, me: Entity, foe: Entity) -> Option<u32> {
        app.world_mut().run_system_once(move |marks: Marksmanship| marks.odds(me, foe, Attempt::Blow).map(|o| o.percent())).expect("the odds")
    }

    /// A war hammer asks twenty of an adventurer of twelve: eight short,
    /// it costs thirty-two accuracy, so a blow that would always land
    /// lands sixty-eight times in a hundred. A potion of strength gives a
    /// point back for good, and four of the accuracy with it.
    #[test]
    fn strength_short_of_a_war_hammers_costs_accuracy_and_a_potion_of_strength_gives_it_back() {
        let mut app = headless(RunSeed(4));
        let me = adventurer(&mut app);
        clear_monsters(&mut app);
        let beside = at(&app, me).offset(1, 0);
        lay_floor(&mut app, &[beside]);
        let rat = monster(&mut app, "rat", beside);
        assert_eq!((stat(&app, me, ACCURACY), chance(&mut app, me, rat)), (100, Some(100)), "the sword asks no more than twelve");
        let hammer = make(&mut app, "war hammer");
        equip(&mut app, me, hammer);
        assert_eq!((stat(&app, me, STRENGTH), stat(&app, me, ACCURACY), chance(&mut app, me, rat)), (12, 68, Some(68)), "eight short");
        let potion = find(&mut app, "potion of strength", 0);
        give(&mut app, me, potion);
        use_thing(&mut app, me, potion);
        assert_eq!((stat(&app, me, STRENGTH), stat(&app, me, ACCURACY), chance(&mut app, me, rat)), (13, 72, Some(72)), "seven short");
        let lines = said(&app);
        assert!(lines.iter().any(|l| l == "You feel stronger."), "{lines:#?}");
    }

    /// Armor too heavy costs a point of armor for each point short, a
    /// piece at a time, and never more than the piece gives: plate that
    /// asks nineteen of twelve is no armor at all, not armor that draws
    /// blows, and a shield asking twelve costs nothing.
    #[test]
    fn armor_too_heavy_costs_a_point_a_point_short_and_never_more_than_it_gives() {
        let mut app = headless(RunSeed(4));
        let me = adventurer(&mut app);
        let armor = |app: &mut App| app.world_mut().run_system_once(move |loadout: Loadout| loadout.armor(me)).expect("a loadout");
        assert_eq!(armor(&mut app), 1, "leather, which asks ten");
        let mail = make(&mut app, "chain mail");
        equip(&mut app, me, mail);
        assert_eq!((armor(&mut app), stat(&app, me, ARMOR)), (2, -1), "chain mail's three, a point short");
        let plate = make(&mut app, "plate armor");
        equip(&mut app, me, plate);
        assert_eq!((armor(&mut app), stat(&app, me, ARMOR)), (0, -6), "plate's six, seven short, and no less than nothing");
        let shield = make(&mut app, "shield");
        equip(&mut app, me, shield);
        assert_eq!(armor(&mut app), 1, "and a shield's one, which asks twelve");
    }

    /// The costs are worked out afresh from what is worn: taken off, the
    /// hammer costs nothing.
    #[test]
    fn taking_off_what_is_too_heavy_gives_back_what_it_cost() {
        let mut app = headless(RunSeed(4));
        let me = adventurer(&mut app);
        let hammer = make(&mut app, "war hammer");
        equip(&mut app, me, hammer);
        assert_eq!(stat(&app, me, ACCURACY), 68);
        crate::testing::unequip(&mut app, me, hammer);
        assert_eq!(stat(&app, me, ACCURACY), 100);
    }
}
