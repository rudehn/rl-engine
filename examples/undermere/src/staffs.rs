//! What a staff asks of time: a spent charge comes back at its level's
//! pace, quickened by its carrier's wisdom and hurried along by every blow
//! they land while they wear a ring of reaping.
//!
//! The engine refills a thing's charges on the turn clock, one every
//! `Recharge::every` hundredths of a step, counting `progress` towards the
//! next; a staff's line says how long a charge takes at level one and the
//! armory sets the pace by its level. [`keep_pace`] keeps that pace
//! current for every staff, quicker for one the adventurer carries by
//! their [`WISDOM`] stat, read every pass, so a ring of wisdom put on or
//! taken off reaches the pace at once and a staff dropped goes back to its
//! own; and [`reap`] adds to every staff they carry that is not full what
//! each blow they land is worth by their [`REAPING`] stat. Both touch only
//! numbers the engine already counts with, so a refill is the engine's as
//! every other is.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::armory::{Armory, ItemKind};
use crate::content::{REAPING, WISDOM};
use crate::run::Adventurer;

/// The slowest a curse of wisdom makes a staff refill, in percent of its
/// own pace: a quarter, so a ring of wisdom found at minus three is a
/// staff that refills four times slower and not one that never does.
pub const SLOWEST_PACE: i32 = 25;

/// Hundredths of a step a charge takes to come back for a staff whose own
/// pace is `base`, carried by someone whose wisdom stat is `wisdom`: that
/// many percent faster, a ring of wisdom's three tenths a level, and never
/// slower than [`SLOWEST_PACE`] percent of its own.
pub fn paced(base: u32, wisdom: i32) -> u32 {
    let pace = (100 + wisdom).max(SLOWEST_PACE) as u32;
    (base * 100 / pace).max(1)
}

/// Keeps every staff refilling at its level's pace, and the ones the
/// adventurer carries at that pace quickened by their wisdom, as [`paced`]
/// says.
///
/// Every pass, in `TurnSet::React`, derived from the staff's line, its
/// level and the carrier's stat rather than adjusted, so wisdom is never
/// applied twice and a staff that changes hands or levels is at the right
/// pace the pass after. Only the adventurer's wisdom counts, since nobody
/// else wears a ring.
pub fn keep_pace(
    adventurer: Query<(&Inventory, &StatBlock), With<Adventurer>>,
    mut staffs: Query<(Entity, &ItemKind, Option<&ItemLevel>, &mut Consumable)>,
    armory: Res<Armory>,
    registries: Res<Registries>,
) {
    let stat = registries.stats.id(WISDOM);
    let carrier = adventurer.single().ok();
    for (staff, kind, level, mut held) in &mut staffs {
        let Some(base) = armory.recharge_at(kind.0, level.map_or(0, |l| l.0)) else { continue };
        let wisdom = match (carrier, stat) {
            (Some((bag, stats)), Some(stat)) if bag.items.contains(&staff) => stats.0.value(stat, &registries.stats),
            _ => 0,
        };
        let every = paced(base, wisdom);
        if held.recharge.is_some_and(|pace| pace.every != every)
            && let Some(pace) = held.recharge.as_mut()
        {
            pace.every = every;
        }
    }
}

/// Puts what each blow the adventurer landed this pass is worth towards
/// the next charge of every staff they carry that is not full: their
/// reaping stat in turns a blow, a ring of reaping's twenty-five a level,
/// and taken away for a cursed one.
///
/// A blow is a swing struck in reach that met at least one target, as the
/// damage pipeline reports it, armor or none: one however many foes a
/// sweep or a pierce catches and however many extra strikes it carries,
/// since the engine makes one swing of it and reports it once as
/// `Struck`, and a staff's own bolt is no blow, so a staff never hurries
/// itself. The adventurer acts at most once a pass, so a pass's blows that
/// landed are one swing. In `TurnSet::React`, after the pass's damage; the
/// engine turns what is counted into a charge on the next pass that time
/// goes by in.
pub fn reap(
    mut dealt: MessageReader<DamageDealt>,
    adventurer: Query<(Entity, &Inventory, &StatBlock), With<Adventurer>>,
    mut staffs: Query<&mut Consumable, With<ItemKind>>,
    registries: Res<Registries>,
) {
    let (Ok((me, bag, stats)), Some(stat)) = (adventurer.single(), registries.stats.id(REAPING)) else {
        dealt.clear();
        return;
    };
    // Every hit read, so none is left over for the next pass, and one
    // swing counted however many of them it dealt.
    let landed = dealt.read().filter(|d| d.hit.attacker == Some(me) && d.reach == Reach::Melee).count() > 0;
    let blows = i64::from(landed);
    let worth = blows * i64::from(stats.0.value(stat, &registries.stats)) * i64::from(BASE_ACTION_COST);
    if worth == 0 {
        return;
    }
    for item in &bag.items {
        let Ok(mut held) = staffs.get_mut(*item) else { continue };
        if held.left >= held.max || held.recharge.is_none() {
            continue;
        }
        if let Some(pace) = held.recharge.as_mut() {
            pace.progress = (i64::from(pace.progress) + worth).max(0) as u32;
        }
    }
}
