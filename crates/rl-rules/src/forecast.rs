//! What a fight is likely to cost, as numbers a panel can print.
//!
//! An inspect panel that says "Deadly" has to have got that word from
//! somewhere, and the somewhere is arithmetic over the same mitigation
//! pipeline a real blow goes through. Putting it here rather than in the UI
//! means the forecast cannot drift from the fight: [`expected_damage`]
//! calls [`resolve`] with the average roll in place
//! of a real one, through the game's own damage stages.
//!
//! Nothing here rolls dice, so the same inputs always give the same
//! forecast and a panel may recompute it as often as it likes. Nothing here
//! knows what a combatant is called, either; the caller fills a
//! [`Combatant`] from whatever its components are.

use rl_core::DiceRoll;

use crate::content::Registry;
use crate::damage::{DamageKind, DamageKindId, DamageStage, Defender, Hit, Resistances, resolve};

/// One side of a duel, as the forecast reads it.
///
/// `speed` is the engine's percentage: 100 is normal, 200 acts twice as
/// often. Borrowed rather than owned because a caller builds one per frame
/// from live components and throws it away.
pub struct Combatant<'a> {
    /// Health remaining.
    pub health: i32,
    /// Flat armor, the same number [`Defender`] takes.
    pub armor: i32,
    /// Speed as a percentage of normal.
    pub speed: u32,
    /// What it resists.
    pub resists: &'a Resistances,
    /// Every roll one blow of its lands, the main one first.
    pub strikes: &'a [(DamageKindId, DiceRoll)],
}

impl Combatant<'_> {
    /// A combatant that neither strikes nor resists anything.
    pub fn unarmed(health: i32, armor: i32, speed: u32, resists: &Resistances) -> Combatant<'_> {
        Combatant { health, armor, speed, resists, strikes: &[] }
    }
}

/// What one blow by `attacker` takes off `defender`, on average, after the
/// game's own mitigation.
///
/// Fractional because dice are: a d4 that armor takes 1 from averages 1.5,
/// and rounding that to 1 or 2 before it is multiplied out is how a
/// forecast comes to be a blow wrong. Never negative, since a blow that
/// heals is not a blow the forecast can count.
pub fn expected_damage<A: Copy>(attacker: &Combatant<'_>, defender: &Combatant<'_>, kinds: &Registry<DamageKind>, stages: &[&dyn DamageStage<A>]) -> f32 {
    let shield = Defender { armor: defender.armor, blocked: false };
    attacker
        .strikes
        .iter()
        .map(|(kind, dice)| {
            // resolve takes integers, so the average is split into the two
            // rolls that bracket it and weighted by how often each comes up.
            let avg = dice.avg();
            let low = avg.floor() as i32;
            let share = avg - low as f32;
            let at = |amount: i32| {
                let hit: Hit<A> = Hit { attacker: None, credit: None, kind: *kind, amount, critical: false, status: None };
                resolve(&hit, &shield, defender.resists, kinds, stages).max(0) as f32
            };
            at(low) * (1.0 - share) + at(low + 1) * share
        })
        .sum()
}

/// How many blows `attacker` needs to fell `defender`, or `None` when it
/// never will because nothing it throws gets through.
pub fn blows_to_fell<A: Copy>(attacker: &Combatant<'_>, defender: &Combatant<'_>, kinds: &Registry<DamageKind>, stages: &[&dyn DamageStage<A>]) -> Option<u32> {
    let per_blow = expected_damage(attacker, defender, kinds, stages);
    if per_blow <= 0.0 || defender.health <= 0 {
        return None;
    }
    Some((defender.health as f32 / per_blow).ceil() as u32)
}

/// How many whole turns `blows` take an actor of `speed`, rounded up.
///
/// A turn is what an actor of speed 100 gets one action for, so a faster
/// actor fits more blows into the same span and reaches its target sooner.
pub fn turns_for(blows: u32, speed: u32) -> u32 {
    if speed == 0 {
        return u32::MAX;
    }
    ((blows as u64 * 100).div_ceil(speed as u64)) as u32
}

/// How a duel is likely to go for the side asking.
///
/// Derived from the two blow counts and nothing else, so a game that gives
/// its actors more health or better armor moves the boundaries without
/// touching this. Not a difficulty setting: it says who runs out of health
/// first at the current numbers, which is all a panel can honestly claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outlook {
    /// The subject falls in a third of the turns it needs, or cannot hurt
    /// the asker at all.
    Easy,
    /// The subject falls first, but not comfortably.
    Favorable,
    /// Within a turn either way.
    Even,
    /// The asker falls first.
    Grim,
    /// The asker falls in a third of the turns it needs, or cannot hurt the
    /// subject at all.
    Deadly,
}

impl Outlook {
    /// A one-word label. A game that wants its own words matches on the
    /// variant instead; these exist so a default panel has something to
    /// print.
    pub fn label(self) -> &'static str {
        match self {
            Outlook::Easy => "easy",
            Outlook::Favorable => "favorable",
            Outlook::Even => "even",
            Outlook::Grim => "grim",
            Outlook::Deadly => "deadly",
        }
    }
}

/// Both directions of a duel between `asker` and `subject`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duel {
    /// Turns the asker needs to fell the subject, `None` if it never will.
    pub turns_to_fell: Option<u32>,
    /// Turns the subject needs to fell the asker, `None` if it never will.
    pub turns_to_fall: Option<u32>,
    /// How that reads.
    pub outlook: Outlook,
}

/// Runs the duel both ways and reads the result.
pub fn duel<A: Copy>(asker: &Combatant<'_>, subject: &Combatant<'_>, kinds: &Registry<DamageKind>, stages: &[&dyn DamageStage<A>]) -> Duel {
    let to_fell = blows_to_fell(asker, subject, kinds, stages).map(|b| turns_for(b, asker.speed));
    let to_fall = blows_to_fell(subject, asker, kinds, stages).map(|b| turns_for(b, subject.speed));
    let outlook = match (to_fell, to_fall) {
        (None, None) => Outlook::Even,
        (Some(_), None) => Outlook::Easy,
        (None, Some(_)) => Outlook::Deadly,
        (Some(fell), Some(fall)) => {
            if fell * 3 <= fall {
                Outlook::Easy
            } else if fell < fall {
                Outlook::Favorable
            } else if fell == fall {
                Outlook::Even
            } else if fall * 3 <= fell {
                Outlook::Deadly
            } else {
                Outlook::Grim
            }
        }
    };
    Duel { turns_to_fell: to_fell, turns_to_fall: to_fall, outlook }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damage::SubtractArmor;

    fn kinds() -> Registry<DamageKind> {
        Registry::from_defs(vec![DamageKind::new("kinetic")]).unwrap()
    }

    fn armor_only() -> Vec<Box<dyn DamageStage<u32> + Send + Sync>> {
        vec![Box::new(SubtractArmor)]
    }

    fn stage_refs(v: &[Box<dyn DamageStage<u32> + Send + Sync>]) -> Vec<&dyn DamageStage<u32>> {
        v.iter().map(|s| s.as_ref() as &dyn DamageStage<u32>).collect()
    }

    #[test]
    fn armor_moves_the_average_by_its_own_size_and_not_by_a_rounding_step() {
        let kinds = kinds();
        let boxed = armor_only();
        let stages = stage_refs(&boxed);
        let none = Resistances::new();
        let strikes = [(kinds.expect("kinetic"), DiceRoll::new(1, 4))];
        let attacker = Combatant { health: 10, armor: 0, speed: 100, resists: &none, strikes: &strikes };
        let bare = Combatant::unarmed(10, 0, 100, &none);
        let armored = Combatant::unarmed(10, 1, 100, &none);
        let open = expected_damage::<u32>(&attacker, &bare, &kinds, &stages);
        let shielded = expected_damage::<u32>(&attacker, &armored, &kinds, &stages);
        assert!((open - 2.5).abs() < 0.01, "1d4 averages 2.5, got {open}");
        assert!((shielded - 1.5).abs() < 0.01, "one armor takes exactly one, got {shielded}");
    }

    #[test]
    fn a_blow_that_never_lands_never_fells_and_a_stalemate_reads_as_even() {
        let kinds = kinds();
        let boxed = armor_only();
        let stages = stage_refs(&boxed);
        let none = Resistances::new();
        let strikes = [(kinds.expect("kinetic"), DiceRoll::new(1, 2))];
        let biter = Combatant { health: 30, armor: 0, speed: 100, resists: &none, strikes: &strikes };
        let plated = Combatant::unarmed(30, 10, 100, &none);
        assert_eq!(blows_to_fell::<u32>(&biter, &plated, &kinds, &stages), None);
        let stalemate = duel::<u32>(&plated, &biter, &kinds, &stages);
        assert_eq!(stalemate.turns_to_fall, None, "the plated one is never felled");
        assert_eq!(stalemate.outlook, Outlook::Even, "neither side can end it, which is even and not a win");
        let armed = Combatant { health: 30, armor: 10, speed: 100, resists: &none, strikes: &strikes };
        assert_eq!(duel::<u32>(&armed, &biter, &kinds, &stages).outlook, Outlook::Easy, "armor it cannot pierce and a blow that lands");
    }

    #[test]
    fn speed_turns_blows_into_fewer_turns_and_never_rounds_one_away() {
        assert_eq!(turns_for(3, 100), 3);
        assert_eq!(turns_for(4, 200), 2);
        assert_eq!(turns_for(3, 200), 2, "one and a half turns is two");
        assert_eq!(turns_for(1, 50), 2);
        assert_eq!(turns_for(1, 0), u32::MAX, "an actor that never acts never arrives");
    }

    #[test]
    fn the_outlook_reads_the_two_counts_and_not_the_health_bars() {
        let kinds = kinds();
        let boxed = armor_only();
        let stages = stage_refs(&boxed);
        let none = Resistances::new();
        let hard = [(kinds.expect("kinetic"), DiceRoll { num: 1, sides: 8, bonus: 4 })];
        let soft = [(kinds.expect("kinetic"), DiceRoll::new(1, 2))];
        let strong = Combatant { health: 40, armor: 0, speed: 100, resists: &none, strikes: &hard };
        let weak = Combatant { health: 40, armor: 0, speed: 100, resists: &none, strikes: &soft };
        assert_eq!(duel::<u32>(&strong, &weak, &kinds, &stages).outlook, Outlook::Easy);
        assert_eq!(duel::<u32>(&weak, &strong, &kinds, &stages).outlook, Outlook::Deadly);
        assert_eq!(duel::<u32>(&strong, &strong, &kinds, &stages).outlook, Outlook::Even);
    }
}
