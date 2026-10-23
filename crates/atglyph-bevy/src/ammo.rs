//! What a shot spends, when it spends anything at all.
//!
//! [`Ammo`] names the tag a shot draws from; `spend_ammo` takes one off
//! the wielder's bag for every ranged [`Struck`](crate::combat::Struck) the
//! item caused. Whether a weapon is loaded or [`Dry`] is not tracked by
//! reacting to whichever events happen to touch it (equipping it, picking
//! something up): `sync_ammo` derives it fresh, every pass, from the one
//! fact that actually decides it, the wielder's bag right now, so no way
//! of changing that bag can leave a weapon's state stale. Going dry is the
//! same shape as a worn item running hot or otherwise losing its attack:
//! the attack comes off the item and is put by as [`BenchedAttack`], so
//! [`Loadout`](crate::combat::Loadout) falls through to the next worn
//! item exactly as it does for any other benched weapon, with no knowledge
//! of ammunition at all; [`Loadout::dry`](crate::combat::Loadout::dry) is
//! the one place it answers by name whether a worn thing is dry.
//!
//! An opt-in plugin, [`AmmoPlugin`], rather than part of [`CombatPlugin`](crate::combat::CombatPlugin):
//! a game with no ammunition-fed weapon (all blades, or stealth alone)
//! pays nothing for the idea, and one that has some, as a second
//! game with a bow does exactly as Foundry's blasters did, adds the one
//! plugin rather than writing this loop again.
//!
//! What a row says about a dry weapon is not here: `atglyph-ui`, which depends
//! on this crate rather than the other way round, reads [`Ammo`] straight
//! off the gear view's own entities, counts its tag in the wearer's bag,
//! and pushes the count, or the word when there is none, as a facet under
//! its row, since a magazine's count is a fact about an ammunition-fed
//! weapon and not about every row a panel might show (`atglyph-ui/src/lib.rs`'s
//! rules on a view versus a facet say why). What a game says in its own log when a weapon dries or reloads is
//! the game's own words too: `sync_ammo` writes [`AmmoChanged`] for every
//! item whose [`Dry`] it just set or cleared, so a game that wants to tell
//! its player reacts to that message rather than detecting the transition
//! itself.

use atglyph_rules::TagId;
use bevy::prelude::*;

use crate::combat::{BenchedAttack, RangedAttack};
use crate::items::{Inventory, Stack, Tagged};
use crate::plugin::{Turn, TurnSet};

/// What one shot of an ammunition-fed weapon spends: the tag its wielder's
/// bag is searched for, one at a time.
///
/// A weapon carries this instead of [`MeleeAttack`](crate::combat::MeleeAttack)
/// or [`RangedAttack`] running on some other economy, such as a game's own
/// heat, never both; a game's own content loader is the one place that can
/// say so, since it is the only reader of a game's item file that names
/// either.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ammo {
    /// The tag one shot spends, e.g. the tag a box of arrows carries.
    pub tag: TagId,
}

/// An ammunition-fed weapon `sync_ammo` most recently found nothing to
/// feed it: its [`RangedAttack`] is off, stowed in [`BenchedAttack`], and stays
/// that way until the same check finds its wielder carrying the tag again.
///
/// A marker rather than folded into [`BenchedAttack`] itself, since a weapon
/// stowed for want of ammunition and one a game stows for a reason of its
/// own share that one holder and need telling apart: a game's own system
/// for its own reason only ever queries what names its own cause, and
/// never this marker, so it can never see or act on a weapon ammunition
/// alone benched. `sync_ammo` reads it to know whether a weapon it finds
/// loaded again used to be dry, rather than needlessly restoring one that
/// was never stowed.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dry;

/// An ammunition-fed weapon's loaded state just flipped: `sync_ammo`
/// dried it up for want of its tag, or loaded it again because its
/// wielder's bag now holds some.
///
/// Written for every transition `sync_ammo` makes, whoever wields the item,
/// so a game that wants to say so, Foundry's own narration among them,
/// reacts to this rather than detecting the change itself from whether a
/// component was just added or removed.
#[derive(Message, Debug, Clone, Copy)]
pub struct AmmoChanged {
    /// The item whose loaded state changed.
    pub item: Entity,
    /// `true` once it has gone dry, `false` once it is loaded again.
    pub dry: bool,
}

/// Where ammunition's own economy runs, inside [`TurnSet::React`].
///
/// A named set rather than a game ordering itself after `spend_ammo` or
/// `sync_ammo` directly, which the engine's own rule against ordering a
/// system after another crate's function forbids: a game that wants its
/// own reaction to a shot or a bag changing settled first orders against
/// this set, and the two functions stay free to split or reorder without
/// breaking it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AmmoSet {
    /// `spend_ammo` then `sync_ammo`, chained: a shot's tag comes off
    /// the bag it fired from before every [`Ammo`] item's wielder's bag is
    /// read fresh to decide dry or loaded.
    Resolve,
}

/// Everything `spend_ammo` and `sync_ammo` read or write about an
/// item's ammunition and its wielder's bag.
///
/// One param struct rather than several loose queries on each system,
/// which is what a system this shape earns once it crosses clippy's
/// argument count rather than a sign either system is doing too much.
#[derive(bevy::ecs::system::SystemParam)]
pub struct AmmoWorld<'w, 's> {
    ammos: Query<'w, 's, &'static Ammo>,
    dry: Query<'w, 's, (), With<Dry>>,
    rangeds: Query<'w, 's, &'static RangedAttack>,
    stowed: Query<'w, 's, &'static BenchedAttack>,
    inventories: Query<'w, 's, &'static mut Inventory>,
    tagged: Query<'w, 's, &'static Tagged>,
    stacks: Query<'w, 's, &'static mut Stack>,
}

/// The item in `inv` carrying `tag` in a [`Stack`] with something left in
/// it, if any.
///
/// Used both to find what a shot spends and to ask whether a bag still has
/// anything of the tag left, since both questions are "does this bag hold
/// one", asked at different moments.
fn stack_of(inv: &Inventory, tag: TagId, tagged: &Query<&Tagged>, stacks: &Query<&mut Stack>) -> Option<Entity> {
    inv.items.iter().copied().find(|item| tagged.get(*item).is_ok_and(|t| t.0.contains(&tag)) && stacks.get(*item).is_ok_and(|s| s.count > 0))
}

/// Reacts to every ranged [`Struck`](crate::combat::Struck) whose weapon
/// runs on [`Ammo`]: takes one off the first matching [`Stack`] in the
/// attacker's bag, despawning it at zero. It does not need to dry the
/// weapon itself; `sync_ammo`, chained right after it, reads the bag this
/// left behind and dries whatever that bag can no longer feed, this weapon
/// included.
///
/// Melee and unarmed strikes never carry an item that could name [`Ammo`],
/// so this only ever looks at ranged hits; a `Struck` naming no item, an
/// attacker with no bag, or a weapon that is not ammunition-fed are all
/// skipped rather than treated as empty, since none of them is this
/// system's business.
pub(crate) fn spend_ammo(mut commands: Commands, mut struck: MessageReader<crate::combat::Struck>, mut world: AmmoWorld) {
    for ev in struck.read() {
        if !ev.ranged {
            continue;
        }
        let Some(item) = ev.with else { continue };
        let Ok(ammo) = world.ammos.get(item) else { continue };
        let Ok(mut inv) = world.inventories.get_mut(ev.attacker) else { continue };
        let Some(unit) = stack_of(&inv, ammo.tag, &world.tagged, &world.stacks) else { continue };
        let mut stack = world.stacks.get_mut(unit).expect("just found it");
        stack.count -= 1;
        if stack.count == 0 {
            inv.remove(unit);
            commands.entity(unit).despawn();
        }
    }
}

/// For every [`Ammo`] item in any actor's [`Inventory`], sets it dry or
/// loaded from that bag alone, idempotently: one already in the right
/// state is left untouched, and only a transition writes [`AmmoChanged`].
///
/// Run every pass rather than kept in step by reacting to whichever events
/// happen to touch a bag. Equipping a weapon, picking ammunition up, and
/// dropping it all did, once, each need their own listener, and a fourth
/// way to change a bag was a fourth listener to remember: two weapons
/// sharing one bag both fired the same last unit before a pickup-only
/// reload ever ran again, because nothing had reacted to the second
/// weapon's `Struck` at all, and dropping the whole stack left a loaded
/// weapon behind because nothing had reacted to a drop either. Deriving
/// the state from the bag directly has no such gap: whatever changed it,
/// the very next pass reads the bag as it now stands.
pub(crate) fn sync_ammo(mut commands: Commands, world: AmmoWorld, mut changed: MessageWriter<AmmoChanged>) {
    for inv in world.inventories.iter() {
        for &item in &inv.items {
            let Ok(ammo) = world.ammos.get(item) else { continue };
            let has_ammo = stack_of(inv, ammo.tag, &world.tagged, &world.stacks).is_some();
            let is_dry = world.dry.get(item).is_ok();
            if !has_ammo && !is_dry {
                let Ok(attack) = world.rangeds.get(item) else { continue };
                commands.entity(item).remove::<RangedAttack>().insert(BenchedAttack::Ranged(*attack)).insert(Dry);
                changed.write(AmmoChanged { item, dry: true });
            } else if has_ammo && is_dry {
                let Ok(BenchedAttack::Ranged(attack)) = world.stowed.get(item) else { continue };
                commands.entity(item).remove::<BenchedAttack>().remove::<Dry>().insert(*attack);
                changed.write(AmmoChanged { item, dry: false });
            }
        }
    }
}

/// Ammunition: opt-in, since most of a game's weapons never run on it.
///
/// `spend_ammo` and `sync_ammo`, chained in [`AmmoSet::Resolve`] inside
/// [`TurnSet::React`]: a game adds nothing else to make a tagged bag feed a
/// weapon and dry it when the tag runs out. Needs [`CombatPlugin`](crate::combat::CombatPlugin)
/// and [`ItemsPlugin`](crate::items::ItemsPlugin), for [`Struck`](crate::combat::Struck)
/// and the bag they both read.
pub struct AmmoPlugin;

impl Plugin for AmmoPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AmmoChanged>()
            .configure_sets(Turn, AmmoSet::Resolve.in_set(TurnSet::React))
            .add_systems(Turn, (spend_ammo, sync_ammo).chain().in_set(AmmoSet::Resolve));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::combat::CombatPlugin>(app, "AmmoPlugin");
        crate::plugin::depends_on::<crate::items::ItemsPlugin>(app, "AmmoPlugin");
    }
}

#[cfg(test)]
mod tests {
    use atglyph_rules::{EquipShape, Equipment, SlotId};
    use bevy::ecs::system::RunSystemOnce;

    use super::*;
    use crate::combat::Loadout;
    use crate::components::Player;
    use crate::items::Equipped;
    use crate::testing::headless_app;

    /// A unit of `tag`, stacked `count` high, carried by nobody yet.
    fn unit_of(app: &mut App, tag: TagId, count: u32) -> Entity {
        app.world_mut().spawn((Tagged(vec![tag]), Stack { key: 0, count })).id()
    }

    /// An ammunition-fed weapon, equipped on a fresh actor whose bag also
    /// holds `units` separate unit entities of `tag`, each stacked to
    /// `each`. Returns the actor and the weapon.
    fn wielder_with(app: &mut App, tag: TagId, units: &[u32]) -> (Entity, Entity) {
        app.add_message::<crate::combat::Struck>();
        app.add_message::<AmmoChanged>();
        let weapon = app
            .world_mut()
            .spawn((crate::items::Item, Ammo { tag }, RangedAttack::new(atglyph_rules::damage::DamageKindId::from_raw(0), atglyph_core::DiceRoll::flat(1), 6)))
            .id();
        let mut items = vec![weapon];
        items.extend(units.iter().map(|&count| unit_of(app, tag, count)));
        let actor = app.world_mut().spawn(Inventory { items }).id();
        let mut equipped = Equipped(Equipment::with_slot_count(1));
        equipped.equip(weapon, &EquipShape::in_slot(SlotId::from_raw(0))).expect("the slot exists");
        app.world_mut().entity_mut(actor).insert(equipped);
        (actor, weapon)
    }

    /// Runs `spend_ammo` then `sync_ammo` once, the two chained exactly as
    /// `AmmoPlugin` orders them, without needing the whole `Turn` schedule
    /// a bare mechanism test has no other reason to stand up.
    ///
    /// Clears `Struck` after: a persistently-registered reader advances its
    /// own cursor and never sees the same message twice, but
    /// `run_system_once` stands up a fresh one every call, with no cursor
    /// of its own to remember what a previous call already read, so a
    /// second call in the same test would spend an already-spent `Struck`
    /// all over again were it left in the buffer.
    fn resolve(app: &mut App) {
        app.world_mut().run_system_once(spend_ammo).unwrap();
        app.world_mut().run_system_once(sync_ammo).unwrap();
        app.world_mut().resource_mut::<Messages<crate::combat::Struck>>().clear();
    }

    /// Whether `Loadout::ranged` still finds a shot for `who`.
    fn has_shot(app: &mut App, who: Entity) -> bool {
        let mut state: bevy::ecs::system::SystemState<Loadout> = bevy::ecs::system::SystemState::new(app.world_mut());
        state.get(app.world()).expect("every input is optional").ranged(who).is_some()
    }

    #[test]
    fn a_shot_spends_one_of_the_tag_and_the_last_one_leaves_the_weapon_dry_and_loadout_finding_no_shot() {
        let mut app = headless_app();
        let tag = TagId::from_raw(0);
        let (actor, weapon) = wielder_with(&mut app, tag, &[2]);
        let target = app.world_mut().spawn_empty().id();
        assert!(has_shot(&mut app, actor), "loaded to start");

        app.world_mut().write_message(crate::combat::Struck { attacker: actor, target, with: Some(weapon), ranged: true });
        resolve(&mut app);
        assert!(has_shot(&mut app, actor), "one spent, one left");

        app.world_mut().write_message(crate::combat::Struck { attacker: actor, target, with: Some(weapon), ranged: true });
        resolve(&mut app);
        assert!(app.world().get::<RangedAttack>(weapon).is_none(), "dry");
        assert!(!has_shot(&mut app, actor), "loadout finds nothing to fire with");
    }

    #[test]
    fn a_dry_weapon_is_loaded_again_once_its_wielders_bag_gains_the_tag() {
        let mut app = headless_app();
        let tag = TagId::from_raw(0);
        let (actor, weapon) = wielder_with(&mut app, tag, &[]);
        resolve(&mut app);
        assert!(app.world().get::<RangedAttack>(weapon).is_none(), "dry from the start: nothing in the bag");

        let unit = unit_of(&mut app, tag, 3);
        app.world_mut().get_mut::<Inventory>(actor).unwrap().items.push(unit);
        resolve(&mut app);
        assert!(app.world().get::<RangedAttack>(weapon).is_some(), "loaded again");
    }

    #[test]
    fn two_items_sharing_one_bags_tag_never_spend_the_same_unit_of_it_twice() {
        // The bug this guards: with `spend_ammo` alone drying only the
        // weapon named on the `Struck` it reacted to, a second item the
        // first shot never touched stayed loaded one pass longer than the
        // bag did, and fired a unit that was already gone. `sync_ammo`'s
        // pass over every `Ammo` item in the bag, not just the one a
        // `Struck` names, is what closes that.
        let mut app = headless_app();
        let tag = TagId::from_raw(0);
        let (actor, first) = wielder_with(&mut app, tag, &[1]);
        let second = app
            .world_mut()
            .spawn((crate::items::Item, Ammo { tag }, RangedAttack::new(atglyph_rules::damage::DamageKindId::from_raw(0), atglyph_core::DiceRoll::flat(1), 6)))
            .id();
        app.world_mut().get_mut::<Inventory>(actor).unwrap().items.push(second);
        let target = app.world_mut().spawn_empty().id();

        app.world_mut().write_message(crate::combat::Struck { attacker: actor, target, with: Some(first), ranged: true });
        resolve(&mut app);
        assert!(app.world().get::<RangedAttack>(first).is_none(), "dry: the bag has nothing left");
        assert!(app.world().get::<RangedAttack>(second).is_none(), "dry too: it shares that same now-empty bag");
    }

    #[test]
    fn the_bag_losing_every_unit_of_the_tag_any_way_at_all_leaves_the_weapon_dry_before_its_next_shot() {
        // The bug this guards: a predecessor that only reacted to picking
        // ammunition up and to equipping a weapon never reacted to a drop
        // (an `ItemEvent::Dropped` it never listened for), so dropping a
        // stack of units left a loaded weapon pointed at an empty bag.
        // `sync_ammo` reading the bag itself, not any one event that might
        // have changed it, is what closes that; this test drops the unit
        // from the bag directly rather than through `DropItem`'s own turn
        // resolution, since what is under test is `sync_ammo` noticing the
        // bag changed at all, not how `DropItem` itself is resolved, which
        // `crate::items`'s own tests already cover.
        let mut app = headless_app();
        let tag = TagId::from_raw(0);
        let (actor, weapon) = wielder_with(&mut app, tag, &[3]);
        resolve(&mut app);
        assert!(app.world().get::<RangedAttack>(weapon).is_some(), "loaded to start");

        let unit = *app.world().get::<Inventory>(actor).unwrap().items.iter().find(|&&i| i != weapon).expect("the unit is in the bag too");
        app.world_mut().get_mut::<Inventory>(actor).unwrap().items.retain(|&i| i != unit);
        app.world_mut().despawn(unit);
        resolve(&mut app);
        assert!(app.world().get::<RangedAttack>(weapon).is_none(), "dry: the bag it drew from no longer has it");
    }

    #[test]
    fn sync_ammo_writes_ammochanged_once_a_weapon_dries_and_once_more_when_it_loads_again() {
        let mut app = headless_app();
        let tag = TagId::from_raw(0);
        let (actor, weapon) = wielder_with(&mut app, tag, &[1]);
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(crate::combat::Struck { attacker: actor, target, with: Some(weapon), ranged: true });
        resolve(&mut app);
        let read = |app: &mut App| -> Vec<AmmoChanged> { app.world_mut().resource_mut::<Messages<AmmoChanged>>().drain().collect() };
        let after_drying = read(&mut app);
        assert_eq!(after_drying.len(), 1, "one transition: this is the shot that dried it");
        assert_eq!((after_drying[0].item, after_drying[0].dry), (weapon, true));

        let unit = unit_of(&mut app, tag, 1);
        app.world_mut().get_mut::<Inventory>(actor).unwrap().items.push(unit);
        resolve(&mut app);
        let after_loading = read(&mut app);
        assert_eq!(after_loading.len(), 1, "one transition: loaded again");
        assert_eq!((after_loading[0].item, after_loading[0].dry), (weapon, false));
    }

    #[test]
    fn ammo_plugin_runs_spend_ammo_before_sync_ammo_so_a_second_item_on_the_same_bag_dries_the_same_pass() {
        // The wiring itself, not just the mechanism above:
        // `two_items_sharing_one_bags_tag_never_spend_the_same_unit_of_it_twice`
        // cannot show this ordering mattering, because the engine's own
        // turn loop (`crate::turn`) reruns the `Turn` schedule until
        // nothing more can progress within one `app.update()`, so even
        // with the chain reversed, `sync_ammo` gets a second pass to catch
        // up before that call returns. Running the real `Turn` schedule
        // exactly once isolates the guarantee `AmmoPlugin`'s chain is for.
        let mut app = headless_app();
        crate::testing::ground(&mut app);
        app.add_plugins(AmmoPlugin);
        let tag = TagId::from_raw(0);
        let (actor, first) = wielder_with(&mut app, tag, &[1]);
        let second = app
            .world_mut()
            .spawn((crate::items::Item, Ammo { tag }, RangedAttack::new(atglyph_rules::damage::DamageKindId::from_raw(0), atglyph_core::DiceRoll::flat(1), 6)))
            .id();
        app.world_mut().get_mut::<Inventory>(actor).unwrap().items.push(second);
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(crate::combat::Struck { attacker: actor, target, with: Some(first), ranged: true });
        app.world_mut().run_schedule(Turn);
        assert!(app.world().get::<RangedAttack>(first).is_none(), "first spent the only unit and is dry");
        assert!(app.world().get::<RangedAttack>(second).is_none(), "second shares that same now-empty bag and must dry in this same pass too");
    }

    /// `headless_app` deals no turns and queues no actor at all, so the
    /// `Player` marker here is only to prove `AmmoPlugin` needs none of
    /// that: ammunition is any actor's economy, not the player's alone.
    #[test]
    fn ammo_plugin_needs_no_player_and_no_turn_queue() {
        let mut app = headless_app();
        crate::testing::ground(&mut app);
        app.add_plugins(AmmoPlugin);
        let tag = TagId::from_raw(0);
        let (actor, weapon) = wielder_with(&mut app, tag, &[]);
        app.world_mut().entity_mut(actor).insert(Player);
        app.world_mut().run_schedule(Turn);
        app.world_mut().run_schedule(Turn);
        assert!(app.world().get::<RangedAttack>(weapon).is_none(), "dry: never fired, never any slug, and nothing panicked for want of a turn");
    }

    /// `AmmoPlugin` needs `ItemsPlugin` for the bag `spend_ammo` and
    /// `sync_ammo` both read, the same shape `ThrowingPlugin` declares for
    /// the same reason: a game that adds the one without the other is
    /// told so loudly, at `finish`, rather than failing quietly the first
    /// time a shot finds no bag to spend from.
    #[test]
    #[should_panic(expected = "AmmoPlugin needs")]
    fn ammo_plugin_without_items_plugin_says_so_at_build() {
        let mut app = headless_app();
        app.add_plugins((crate::combat::CombatPlugin, AmmoPlugin));
        app.finish();
    }
}
