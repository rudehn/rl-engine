//! What the five artifacts do beyond what their own row in `items.ron`
//! already says through the engine's own fields: the two the engine has
//! no hook for at all.
//!
//! The Crown of Grubnak's armor, its wearer's wider notice and the
//! Choirmaster's staff's lost mending are all `items.ron`'s own `armor`
//! and `bestows`, folded by the engine's `fold_gear` on whoever wears
//! them; nothing here repeats what that already does. What is here is
//! the one half neither `bestows` nor a trigger reaches: an aura on
//! somebody other than the wearer, [`crown_aura`], and a charge nothing
//! in `items.ron`'s own `consumable` refills on its own clock,
//! [`refill_chalice`].

use atglyph::prelude::*;
use bevy::prelude::*;

use crate::armory::{Armory, ItemKind};
use crate::dungeon::floor_of;

/// The Crown of Grubnak's own name, as `items.ron` writes it: what
/// [`crown_aura`] looks for among what is worn.
pub const CROWN_OF_GRUBNAK: &str = "the Crown of Grubnak";

/// How much harder every ally within the crown's wearer's sight hits, on
/// every blow and every shot alike: the content sheet's own two.
pub const CROWN_BONUS: i32 = 2;

/// The [`Source::Game`] key the crown's aura tags its modifier with, an
/// eight-byte tag read off its own letters rather than a plain small
/// number: [`crate::strength::TOO_HEAVY`] is `Source::Game(1)`, which a
/// naive `Source::Game(1)` here collided with, stripping every pass what
/// a hammer too heavy or a mace too heavy had cost the wearer.
const CROWN_AURA: u64 = u64::from_be_bytes(*b"crownaur");

/// Gives every ally within the Crown of Grubnak's wearer's sight
/// [`CROWN_BONUS`] more on every blow and shot, and takes it back the
/// pass it falls out of sight, as the crown's own row says: "allies
/// within sight deal 2 more a blow."
///
/// Recomputed whole every pass rather than kept from one to the next:
/// nothing here logs or narrates, unlike an `Afflict`, so there is no
/// spurious line a edge-triggered version would exist to avoid, and a
/// flat strip-then-reapply is right through an equip, an unequip, or the
/// wearer's own death alike, with nothing left keyed to an entity that
/// may no longer be there to clean it up. "Allies" is the engine's own
/// [`Relation::Allied`], the adventurer's own side included, so a freed
/// captive or a raised body counts exactly as a hired sellsword would.
///
/// In `TurnSet::React`.
pub fn crown_aura(
    armory: Res<Armory>,
    registries: Res<Registries>,
    rules: Option<Res<CombatRules>>,
    items: Query<(Entity, &ItemKind), With<Item>>,
    wearers: Query<(Entity, &Equipped, &OnMap, &Faction, &Viewshed), With<Actor>>,
    mut bodies: Query<(Entity, &Position, &OnMap, &Faction, &mut StatBlock), With<Actor>>,
) {
    let (Some(rules), Some(attack), Some(crown)) = (rules, registries.stats.id(crate::content::ATTACK), armory.defs.id(CROWN_OF_GRUBNAK)) else { return };
    let crown_item = items.iter().find(|(_, k)| k.0 == crown).map(|(e, _)| e);
    let wearer = crown_item.and_then(|item| wearers.iter().find(|(_, equipped, ..)| equipped.0.contains(item)));
    for (body, pos, on, side, mut stats) in &mut bodies {
        let in_range = wearer.is_some_and(|(wearer_e, _, wearer_on, wearer_side, sight)| {
            body != wearer_e && on.0 == wearer_on.0 && sight.can_see(pos.0) && rules.factions.relation(side.0, wearer_side.0) == Relation::Allied
        });
        stats.0.remove_source(Source::Game(CROWN_AURA));
        if in_range {
            stats.0.add(Modifier::new(attack, Op::Add(CROWN_BONUS), Source::Game(CROWN_AURA)));
        }
    }
}

/// The Abbess's Chalice's own name, as `items.ron` writes it: what
/// [`refill_chalice`] looks for.
pub const ABBESS_CHALICE: &str = "the Abbess's Chalice";

/// Refills the Abbess's Chalice back to its one charge the moment a new
/// floor is first reached, as the chalice's own row says: "one charge,
/// refilled on each new floor." `items.ron`'s own `recharge` refills a
/// spent charge over turns; this is a charge the clock never touches and
/// only a floor's own arrival gives back.
///
/// Reads the same [`PlaceEntered`] [`crate::finds::lay`] lays a floor's
/// things from, filtered to the ten floors and never the town, which owns
/// nothing worth a chalice's refill. In `TurnSet::React`.
pub fn refill_chalice(mut entered: MessageReader<PlaceEntered>, armory: Res<Armory>, mut things: Query<(&ItemKind, &mut Consumable)>) {
    let Some(chalice) = armory.defs.id(ABBESS_CHALICE) else { return };
    if !entered.read().any(|ev| ev.first && floor_of(ev.map).is_some()) {
        return;
    }
    for (kind, mut held) in &mut things {
        if kind.0 == chalice {
            held.left = held.max;
        }
    }
}

#[cfg(test)]
mod tests {
    use atglyph::primitives::RunSeed;

    use super::*;
    use crate::allies::befriend;
    use crate::dungeon::map_of;
    use crate::testing::{arena, equip, give, headless, make, monster, pass_turns, stand_on};

    /// Every ally of the crown's wearer within sight deals two more a
    /// blow, and loses it the pass it steps out of sight, while the
    /// wearer's own blow is untouched.
    #[test]
    fn an_ally_in_the_crowns_wearers_sight_deals_two_more_a_blow_and_loses_it_out_of_sight() {
        let (mut app, me, here) = arena(1);
        let ally = monster(&mut app, "goblin", here.offset(1, 0));
        befriend(app.world_mut(), ally);
        let crown = make(&mut app, CROWN_OF_GRUBNAK);
        equip(&mut app, me, crown);
        pass_turns(&mut app, 1);
        let stat = app.world().resource::<Registries>().stats.expect(crate::content::ATTACK);
        let bonus = |app: &App, who: Entity| app.world().get::<StatBlock>(who).unwrap().0.value(stat, &app.world().resource::<Registries>().stats);
        assert_eq!(bonus(&app, ally), CROWN_BONUS, "beside its crowned ally");
        assert_eq!(bonus(&app, me), 0, "the crown gives its own wearer nothing");
        stand_on(&mut app, ally, here.offset(40, 40));
        pass_turns(&mut app, 1);
        assert_eq!(bonus(&app, ally), 0, "out of sight, the bonus is gone");
    }

    /// The crown widens every watcher's certain notice of its wearer by
    /// two cells: `items.ron`'s own `bestows: [("quiet", -2)]`, folded by
    /// the engine's `fold_gear` onto the adventurer's `quiet` exactly as
    /// it folds every other worn thing's.
    #[test]
    fn the_crown_widens_its_wearers_certain_notice_by_two_cells() {
        let (mut app, me, _) = arena(1);
        let crown = make(&mut app, CROWN_OF_GRUBNAK);
        equip(&mut app, me, crown);
        pass_turns(&mut app, 1);
        let quiet = app.world().get::<Stealth>(me).map(|s| s.0.quiet);
        assert_eq!(quiet, Some(-2), "two more cells of certain notice");
    }

    /// The Abbess's Chalice spends its one charge and is refilled the
    /// moment a new floor is reached, never before.
    #[test]
    fn the_abbesss_chalice_refills_its_one_charge_on_a_new_floor() {
        let mut app = headless(RunSeed(1));
        let me = crate::testing::adventurer(&mut app);
        let chalice = make(&mut app, ABBESS_CHALICE);
        give(&mut app, me, chalice);
        app.world_mut().get_mut::<Consumable>(chalice).unwrap().left = 0;
        pass_turns(&mut app, 1);
        assert_eq!(app.world().get::<Consumable>(chalice).map(|c| c.left), Some(0), "no new floor yet");
        crate::testing::arrive(&mut app, map_of(1));
        assert_eq!(app.world().get::<Consumable>(chalice).map(|c| c.left), Some(1), "a new floor refills it");
    }
}
