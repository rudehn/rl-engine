//! What a shot spends, when it spends anything at all.
//!
//! [`Ammo`] names the tag a shot draws from; [`spend_ammo`] takes one off
//! the wielder's bag for every ranged [`Struck`] the item caused. Whether
//! a weapon is loaded or [`Dry`] is not tracked by reacting to whichever
//! events happen to touch it (equipping it, picking something up):
//! [`sync_ammo`] derives it fresh, every pass, from the one fact that
//! actually decides it, the wielder's bag right now, so no way of
//! changing that bag can leave a weapon's state stale. Going dry is the
//! same shape as running hot: the attack comes off the item and is put
//! by as [`Stowed`](crate::heat::Stowed), reusing Task 6's holder rather
//! than inventing a second one, so the engine's own
//! [`Loadout`](rl_engine::rl_bevy::Loadout) falls through to the next
//! worn item exactly as it does for a locked weapon.

use bevy::prelude::*;
use rl_engine::rl_bevy::{Equipped, Inventory, Player, RangedAttack, Stack, Struck, Tagged, Turns};
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

/// An ammunition-fed weapon [`sync_ammo`] most recently found nothing to
/// feed it: its `RangedAttack` is off, stowed in [`Stowed::Ranged`], and
/// stays that way until the same check finds its wielder carrying the tag
/// again.
///
/// A marker rather than folded into `Stowed` itself, since a weapon
/// stowed for want of ammunition and one stowed for having overheated
/// share that one holder and need telling apart: [`vent_heat`](crate::heat::vent_heat)
/// only ever queries entities with [`Heat`](crate::heat::Heat), and no
/// item carries both (`gear::Armory::load` refuses one that tries), so it
/// can never see this marker at all, let alone act on it. `sync_ammo`
/// reads it to know whether a weapon it finds loaded again used to be
/// dry, rather than needlessly restoring one that was never stowed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dry;

/// Everything [`spend_ammo`] and [`sync_ammo`] read or write about an
/// item's ammunition, its wielder's bag, and the shot a dry weapon has
/// put by.
///
/// One param struct rather than several loose queries on each system,
/// which is what a system this shape earns once it crosses clippy's
/// argument count rather than a sign either system is doing too much.
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
    player: Query<'w, 's, &'static Equipped, With<Player>>,
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

/// Reacts to every ranged [`Struck`] whose weapon runs on [`Ammo`]: takes
/// one off the first matching [`Stack`] in the attacker's bag, despawning
/// it at zero. It does not need to dry the weapon itself; [`sync_ammo`],
/// chained right after it, reads the bag this left behind and dries
/// whatever that bag can no longer feed, this weapon included.
///
/// Melee and unarmed strikes never carry an item that could name [`Ammo`],
/// so this only ever looks at ranged hits; a `Struck` naming no item, an
/// attacker with no bag, or a weapon that is not ammunition-fed are all
/// skipped rather than treated as empty, since none of them is this
/// system's business.
pub fn spend_ammo(mut commands: Commands, mut struck: MessageReader<Struck>, mut world: AmmoWorld) {
    for ev in struck.read() {
        if !ev.ranged {
            continue;
        }
        let Some(item) = ev.with else { continue };
        let Ok(ammo) = world.ammos.get(item) else { continue };
        let Ok(mut inv) = world.inventories.get_mut(ev.attacker) else { continue };
        let Some(slug) = stack_of(&inv, ammo.tag, &world.tagged, &world.stacks) else { continue };
        let mut stack = world.stacks.get_mut(slug).expect("just found it");
        stack.count -= 1;
        if stack.count == 0 {
            inv.remove(slug);
            commands.entity(slug).despawn();
        }
    }
}

/// For every [`Ammo`] item in any actor's [`Inventory`], sets it dry or
/// loaded from that bag alone, idempotently: one already in the right
/// state is left untouched.
///
/// Run every pass rather than kept in step by reacting to whichever
/// events happen to touch a bag. Equipping a weapon, picking ammunition
/// up, and dropping it all did, once, each need their own listener, and
/// a fourth way to change a bag was a fourth listener to remember: two
/// slug pistols sharing one bag both fired the same last slug before a
/// pickup-only reload ever ran again, because nothing had reacted to the
/// second pistol's `Struck` at all, and dropping the whole stack left a
/// loaded pistol behind because nothing had reacted to `Dropped` either.
/// Deriving the state from the bag directly has no such gap: whatever
/// changed it, the very next pass reads the bag as it now stands.
///
/// Only a weapon the player wears says so in the log: a pistol at the
/// bottom of the pack, or in a droid's hand, going dry is nothing the
/// player can act on, and reads as noise between the lines that matter.
pub fn sync_ammo(mut commands: Commands, world: AmmoWorld, turns: Res<Turns>, mut log: ResMut<MessageLog>) {
    for inv in world.inventories.iter() {
        for &item in &inv.items {
            let Ok(ammo) = world.ammos.get(item) else { continue };
            let has_ammo = stack_of(inv, ammo.tag, &world.tagged, &world.stacks).is_some();
            let is_dry = world.dry.get(item).is_ok();
            let name = || world.names.get(item).map(Name::as_str).unwrap_or("weapon");
            let yours = world.player.iter().any(|e| e.0.worn().any(|(_, worn)| worn == item));
            if !has_ammo && !is_dry {
                let Ok(attack) = world.rangeds.get(item) else { continue };
                commands.entity(item).remove::<RangedAttack>().insert(Stowed::Ranged(*attack)).insert(Dry);
                if yours {
                    log.bad(format!("Your {} runs dry.", name()), turns.turn_number());
                }
            } else if has_ammo && is_dry {
                let Ok(Stowed::Ranged(attack)) = world.stowed.get(item) else { continue };
                commands.entity(item).remove::<Stowed>().remove::<Dry>().insert(*attack);
                if yours {
                    log.notice(format!("Your {} is loaded again.", name()), turns.turn_number());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use rl_engine::rl_bevy::{DropItem, Intent, RangedAttack};
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
    fn two_dual_wielded_slug_pistols_never_fire_the_same_slug_twice() {
        // The bug this guards: with `spend_ammo` alone drying only the
        // weapon named on the `Struck` it reacted to, an off-hand pistol
        // the first shot never touched stayed loaded one pass longer than
        // the bag did, and fired a slug that was already gone.
        // `sync_ammo`'s pass over every `Ammo` item in the bag, not just
        // the one a `Struck` names, is what closes that.
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, first, second) = crate::testing::dual_slug_pistols_with(&mut app, 1);
        let struck = crate::testing::fire_at_a_target(&mut app, player, 2);
        assert_eq!(struck.len(), 1, "one slug between two pistols is one shot, not two");
        assert!(app.world().get::<RangedAttack>(first).is_none(), "dry: the bag has nothing left");
        assert!(app.world().get::<RangedAttack>(second).is_none(), "dry: the bag has nothing left, whichever hand fired");
    }

    #[test]
    fn dropping_every_slug_leaves_the_pistol_dry_before_its_next_shot() {
        // The bug this guards: `sync_ammo`'s predecessor only reacted to
        // `ItemEvent::Equipped` and `ItemEvent::PickedUp`, so dropping a
        // stack of slugs (an `ItemEvent::Dropped` it never listened for)
        // left a loaded pistol pointed at an empty bag.
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, pistol) = crate::testing::slug_pistol_with(&mut app, 3);
        assert!(app.world().get::<RangedAttack>(pistol).is_some(), "loaded to start");
        let slugs = *app.world().get::<Inventory>(player).unwrap().items.iter().find(|&&i| i != pistol).expect("the slug stack is in the bag too");
        app.world_mut().write_message(Intent::new(player, DropItem(slugs)));
        app.update();
        assert!(app.world().get::<RangedAttack>(pistol).is_none(), "dry: the bag it drew from is on the floor now");
        let struck = crate::testing::fire_at_a_target(&mut app, player, 1);
        assert!(struck.is_empty(), "nothing to fire with");
    }

    #[test]
    fn foundry_plugin_runs_spend_ammo_before_sync_ammo_so_a_second_weapon_on_the_same_bag_dries_the_same_pass() {
        // The wiring itself, not just the mechanism above:
        // `two_dual_wielded_slug_pistols_never_fire_the_same_slug_twice`
        // cannot show this ordering mattering, because the engine's own
        // turn loop (crates/rl-bevy/src/plugin.rs) reruns the `Turn`
        // schedule until nothing more can progress within one
        // `app.update()`, so even with the chain reversed, `sync_ammo`
        // gets a second pass to catch up before that call returns.
        // Forcing exactly one `Turn` pass, the way heat's own wiring test
        // does, isolates the guarantee `FoundryPlugin`'s chain is for:
        // within that one pass, a second weapon fed from the same bag as
        // the one that just fired must see the bag `spend_ammo` already
        // emptied, not the bag as it stood before this pass's shot.
        let mut app = crate::testing::headless(RunSeed(1));
        app.update();
        app.update();
        let tag: TagId = rl_engine::rl_core::Id::from_raw(0);
        let attack = RangedAttack { kind: rl_engine::rl_core::Id::from_raw(0), dice: Default::default(), range: 6, cost: None };
        let slugs = app.world_mut().spawn((Tagged(vec![tag]), Stack { key: 0, count: 1 })).id();
        let a = app.world_mut().spawn((Ammo { tag }, attack)).id();
        let b = app.world_mut().spawn((Ammo { tag }, attack)).id();
        let attacker = app.world_mut().spawn(Inventory { items: vec![a, b, slugs] }).id();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(Struck { attacker, target, with: Some(a), ranged: true });
        app.world_mut().run_schedule(rl_engine::rl_bevy::plugin::Turn);
        assert!(app.world().get::<RangedAttack>(a).is_none(), "a spent the only slug and is dry");
        assert!(app.world().get::<RangedAttack>(b).is_none(), "b shares that same now-empty bag and must dry in this same pass too");
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
