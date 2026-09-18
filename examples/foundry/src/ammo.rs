//! What a shot spends, when it spends anything at all.
//!
//! [`Ammo`] names the tag a shot draws from; [`spend_ammo`] takes one off
//! the wielder's bag for every ranged [`Struck`] the item caused, and
//! running out is the same shape as running hot: the attack comes off the
//! item and is put by as [`Stowed`](crate::heat::Stowed), reusing Task 6's
//! holder rather than inventing a second one, so the engine's own
//! [`Loadout`](rl_engine::rl_bevy::Loadout) falls through to the next
//! worn item exactly as it does for a locked weapon. [`Dry`] is what tells
//! the two economies apart on the same component: a weapon a shot could
//! unstow (heat, once it has vented) from one a shot cannot (empty,
//! until its wielder carries the tag again), which is what keeps
//! [`vent_heat`](crate::heat::vent_heat) from ever touching this one; it
//! only reads entities with [`Heat`](crate::heat::Heat), and a weapon
//! never carries both, enforced in `gear::Armory::load`.
//!
//! [`reload`] is the other half: a weapon that comes on with nothing to
//! feed it goes dry the instant it is worn, so the first shot is never
//! free, and a dry weapon already worn comes back the instant its
//! wielder picks up more of its tag. Both are reactions to
//! [`ItemEvent`], not a poll, so a bag that never changes costs nothing
//! to check.

use bevy::prelude::*;
use rl_engine::rl_bevy::{Inventory, ItemEvent, RangedAttack, Stack, Struck, Tagged, Turns};
use rl_engine::rl_rules::TagId;
use rl_engine::rl_ui::MessageLog;

use crate::heat::Stowed;

/// What one shot of an ammunition-fed weapon spends: the tag its
/// wielder's bag is searched for, one at a time.
///
/// A weapon carries this instead of [`Heat`](crate::heat::Heat), never
/// both; `gear::Armory::load` is the one place that can say so, since it
/// is the only reader of the file that names either.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ammo {
    /// The tag one shot spends, e.g. the tag a box of slugs carries.
    pub tag: TagId,
}

/// An ammunition-fed weapon with nothing left to feed it: [`spend_ammo`]
/// set this the shot its last round left the bag empty, and [`reload`]
/// clears it the moment its wielder carries the tag again.
///
/// A marker rather than folded into [`Stowed`] itself, since the two
/// systems that read it need to tell a dry weapon from a merely
/// overheated one sharing the same holder: [`vent_heat`](crate::heat::vent_heat)
/// only ever sees a [`Heat`](crate::heat::Heat) weapon in the first place,
/// so it can never unstow this one by accident, but this still keeps the
/// two reasons a weapon has nothing worn on it visibly distinct on the
/// entity itself.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dry;

/// Everything [`spend_ammo`] and [`reload`] read or write about an item's
/// ammunition, its wielder's bag, and the shot a dry weapon has put by.
///
/// One param struct rather than eight loose queries on each system, which
/// is what a system this shape earns once it crosses clippy's argument
/// count rather than a sign either system is doing too much.
#[derive(bevy::ecs::system::SystemParam)]
pub struct AmmoWorld<'w, 's> {
    ammos: Query<'w, 's, &'static Ammo>,
    dry: Query<'w, 's, (), With<Dry>>,
    rangeds: Query<'w, 's, &'static RangedAttack>,
    stowed: Query<'w, 's, &'static Stowed>,
    inventories: Query<'w, 's, &'static mut Inventory>,
    tagged: Query<'w, 's, &'static Tagged>,
    stacks: Query<'w, 's, &'static mut Stack>,
    names: Query<'w, 's, &'static Name>,
}

/// The item in `inv` carrying `tag` in a [`Stack`] with something left in
/// it, if any.
///
/// Used both to find what a shot spends and to ask whether a bag still
/// has anything of the tag left, since both questions are "does this
/// bag hold one", asked at different moments.
fn stack_of(inv: &Inventory, tag: TagId, tagged: &Query<&Tagged>, stacks: &Query<&mut Stack>) -> Option<Entity> {
    inv.items.iter().copied().find(|item| tagged.get(*item).is_ok_and(|t| t.contains(&tag)) && stacks.get(*item).is_ok_and(|s| s.count > 0))
}

/// Takes `item`'s `RangedAttack` off and stows it dry, so nothing later
/// finds a shot to fire from it until [`reload`] gives it back.
fn go_dry(commands: &mut Commands, item: Entity, attack: RangedAttack, name: &str, turns: &Turns, log: &mut MessageLog) {
    commands.entity(item).remove::<RangedAttack>().insert(Stowed::Ranged(attack)).insert(Dry);
    log.bad(format!("{name} runs dry."), turns.turn_number());
}

/// Reacts to every ranged [`Struck`] whose weapon runs on [`Ammo`]: takes
/// one off the first matching [`Stack`] in the attacker's bag, despawning
/// it at zero, and when nothing of the tag is left, takes the weapon's
/// [`RangedAttack`] off and stows it dry.
///
/// Melee and unarmed strikes never carry an item that could name [`Ammo`],
/// so this only ever looks at ranged hits; a `Struck` naming no item, an
/// attacker with no bag, or a weapon that is not ammunition-fed are all
/// skipped rather than treated as empty, since none of them is this
/// system's business.
pub fn spend_ammo(mut commands: Commands, mut struck: MessageReader<Struck>, mut world: AmmoWorld, turns: Res<Turns>, mut log: ResMut<MessageLog>) {
    for ev in struck.read() {
        if !ev.ranged {
            continue;
        }
        let Some(item) = ev.with else { continue };
        let Ok(ammo) = world.ammos.get(item) else { continue };
        let Ok(mut inv) = world.inventories.get_mut(ev.attacker) else { continue };
        if let Some(slug) = stack_of(&inv, ammo.tag, &world.tagged, &world.stacks) {
            let mut stack = world.stacks.get_mut(slug).expect("just found it");
            stack.count -= 1;
            if stack.count == 0 {
                inv.remove(slug);
                commands.entity(slug).despawn();
            }
        }
        if stack_of(&inv, ammo.tag, &world.tagged, &world.stacks).is_some() {
            continue;
        }
        let Ok(attack) = world.rangeds.get(item) else { continue };
        let name = world.names.get(item).map(Name::as_str).unwrap_or("it").to_string();
        go_dry(&mut commands, item, *attack, &name, &turns, &mut log);
    }
}

/// Reacts to [`ItemEvent::Equipped`] and [`ItemEvent::PickedUp`], the two
/// moments a bag's ammunition and a weapon's need of it can fall out of
/// step: putting on a weapon with nothing to feed it, and taking up
/// something that might feed one already worn.
///
/// Equipping an [`Ammo`] weapon whose wielder carries none of its tag
/// dries it on the spot, the same way [`spend_ammo`] would after a shot
/// that found the bag empty, so a weapon taken up dry never gets its
/// first shot for free. Picking anything up re-checks every [`Dry`]
/// weapon the picker already wears, since the item this event names might
/// be gear rather than ammunition and the bag is cheap to re-read either
/// way; the one that now finds its tag gets its [`RangedAttack`] back and
/// loses [`Dry`].
pub fn reload(mut commands: Commands, mut events: MessageReader<ItemEvent>, world: AmmoWorld, turns: Res<Turns>, mut log: ResMut<MessageLog>) {
    for ev in events.read() {
        match *ev {
            ItemEvent::Equipped { actor, item } => {
                let Ok(ammo) = world.ammos.get(item) else { continue };
                let Ok(attack) = world.rangeds.get(item) else { continue };
                let Ok(inv) = world.inventories.get(actor) else { continue };
                if stack_of(inv, ammo.tag, &world.tagged, &world.stacks).is_some() {
                    continue;
                }
                let name = world.names.get(item).map(Name::as_str).unwrap_or("it").to_string();
                go_dry(&mut commands, item, *attack, &name, &turns, &mut log);
            }
            ItemEvent::PickedUp { actor, .. } => {
                let Ok(inv) = world.inventories.get(actor) else { continue };
                for &item in &inv.items {
                    let Ok(ammo) = world.ammos.get(item) else { continue };
                    if world.dry.get(item).is_err() {
                        continue;
                    }
                    if stack_of(inv, ammo.tag, &world.tagged, &world.stacks).is_none() {
                        continue;
                    }
                    let Ok(Stowed::Ranged(attack)) = world.stowed.get(item) else { continue };
                    commands.entity(item).remove::<Stowed>().remove::<Dry>().insert(*attack);
                    let name = world.names.get(item).map(Name::as_str).unwrap_or("it");
                    log.notice(format!("{name} is loaded again."), turns.turn_number());
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use rl_engine::rl_bevy::RangedAttack;
    use rl_engine::rl_core::RunSeed;

    use super::*;

    #[test]
    fn each_shot_spends_one_slug_and_the_last_slug_leaves_the_weapon_dry() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, pistol) = crate::testing::slug_pistol_with(&mut app, 2);
        let struck = crate::testing::fire_at_a_target(&mut app, player, 3);
        assert_eq!(struck.len(), 2, "two slugs, two shots; the third finds nothing to fire");
        assert!(app.world().get::<RangedAttack>(pistol).is_none(), "dry");
    }

    #[test]
    fn a_dry_weapon_fires_again_once_its_wielder_picks_up_slugs() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, pistol) = crate::testing::slug_pistol_with(&mut app, 0);
        assert!(app.world().get::<RangedAttack>(pistol).is_none(), "dry from the start: no free first shot");
        crate::testing::give_slugs(&mut app, player, 3);
        app.update();
        assert!(app.world().get::<RangedAttack>(pistol).is_some(), "loaded again");
    }

    #[test]
    fn vent_heat_never_touches_a_weapon_stowed_dry_for_want_of_ammunition() {
        // `vent_heat` only ever queries entities with `Heat`, and no item
        // in the file carries both `Heat` and `Ammo` (Armory::load refuses
        // to load one that does), so a dry weapon should never be in its
        // path at all. Spelled out anyway, on a bare entity with no `Heat`
        // whatsoever, so a future change to either system that widened
        // that query would be caught here rather than in play.
        let mut app = crate::testing::headless(RunSeed(1));
        let item = app
            .world_mut()
            .spawn((Dry, Stowed::Ranged(RangedAttack { kind: rl_engine::rl_core::Id::from_raw(0), dice: Default::default(), range: 6, cost: None })))
            .id();
        app.world_mut().write_message(rl_engine::rl_bevy::TurnEnd { turn: 1 });
        app.world_mut().run_system_once(crate::heat::vent_heat).unwrap();
        assert!(app.world().get::<RangedAttack>(item).is_none(), "still dry: vent_heat has nothing here to vent");
        assert!(app.world().get::<Stowed>(item).is_some(), "still stowed");
    }
}
