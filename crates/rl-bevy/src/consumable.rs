//! What using a thing does, and what a use costs the thing.
//!
//! The third carrier of [`Effects`]: an ability is something an actor knows
//! how to do, a prop's trigger or offer is something the world does, and
//! this is something a thing in the bag does when it is used up. A stim, a
//! grenade you crush in your hand, a ration.
//!
//! **The dividing line, and why there is one.** An item that wants aiming,
//! a cooldown or a pool belongs to abilities: it carries
//! [`Grants`](crate::ability::Grants), the engine turns using it into using
//! what it lends, and everything abilities know about targeting, gating and
//! costs applies. An item whose use simply happens, to whoever used it,
//! where they stand, carries [`OnUse`] instead and never enters the ability
//! registry at all. Foundry proved why the line is worth drawing: its stim
//! and medkit were abilities for a day, which put two consumables on the
//! abilities screen beside the one thing the commando actually knew, and
//! spelled "the item is destroyed" as a cost of the ability rather than as
//! a fact about the item.
//!
//! **What is not here.** Nothing about wearing. What a worn thing does is
//! already declarative and needs no effects: [`Armor`](crate::combat::Armor),
//! [`Resists`](crate::combat::Resists), an attack, [`Bestows`](crate::items::Bestows)
//! for the registered stats, [`Grants`](crate::ability::Grants) for an
//! ability it lends while carried. An `on_equip` list of effects would be
//! one-shot where wearing is a standing state, and the two read the same in
//! content and behave nothing alike.

use std::sync::Arc;

use bevy::prelude::*;

use crate::ability::Charges;
use crate::effects::Effects;
use crate::items::{Item, ItemEvent, Stack};

/// What using this lands, on whoever used it, where they stand.
///
/// Behind an `Arc` because [`Effects`] is a list of boxed trait objects and
/// a game spawns many of one kind of item: the list is built once, when the
/// game reads its item file, and every stim spawned from that definition
/// carries a handle to the same one.
///
/// The user is the only target and their own cell the only cell, so an
/// effect that harms harms the user: that is what makes this the right
/// carrier for a thing you drink and the wrong one for a thing you throw.
/// A thrown thing is [`Throwable`](crate::throwing::Throwable), and a thing
/// you aim is an ability.
#[derive(Component, Clone)]
pub struct OnUse(pub Arc<Effects>);

impl OnUse {
    /// The list, built by the game and shared by every copy of the item.
    pub fn new(effects: Arc<Effects>) -> Self {
        Self(effects)
    }
}

/// An item that a use spends: one off its [`Charges`] when it counts them,
/// else one off its [`Stack`], else the item itself.
///
/// The same ladder [`Cost::Charge`](rl_rules::ability::Cost) walks, and for
/// the same reason: it is what makes a potion a potion and a wand a wand
/// without either having to say which it is. Absent, the thing survives
/// being used, which is what a tool is.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Consumable;

/// Lands what a used item does, and spends the item for it.
///
/// Reads [`ItemEvent::Used`], which the items resolver writes once it has
/// charged the turn and checked the thing is really in the bag, so this
/// system never has to ask whether the use was allowed: it only has to say
/// what it did. In the same phase as the resolver rather than in
/// `TurnSet::React`, so the damage and the statuses it asks for are
/// resolved by the pass that spent the turn, exactly as a prop's offer is.
pub fn land_uses(mut used: MessageReader<ItemEvent>, uses: Query<&OnUse>, mut spending: Spending, mut world: crate::ability::EffectWorld) {
    for ev in used.read() {
        let ItemEvent::Used { actor, item } = ev else { continue };
        let (actor, item) = (*actor, *item);
        let Ok(what) = uses.get(item) else { continue };
        // Where the user stands is where it happens. Nowhere at all is a
        // world too broken to land anything in.
        let Some(at) = world.position(actor) else { continue };
        what.0.land_on(actor, at, vec![actor], &mut world);
        spending.spend_one(item);
    }
}

/// What a use costs the thing that was used.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Spending<'w, 's> {
    commands: Commands<'w, 's>,
    consumable: Query<'w, 's, (), With<Consumable>>,
    counted: Query<'w, 's, &'static mut Charges>,
    stacks: Query<'w, 's, &'static mut Stack>,
    items: Query<'w, 's, (), With<Item>>,
}

impl Spending<'_, '_> {
    /// Takes one use off `item`, if a use costs it anything at all: a charge
    /// when it counts them, else one off its stack, else the item itself,
    /// despawned.
    ///
    /// A stack spent to nothing is despawned rather than left as a stack of
    /// zero, and the bag it was in forgets it in
    /// [`forget_removed_items`](crate::items::forget_removed_items), which
    /// is where everything that stops being an item is forgotten.
    fn spend_one(&mut self, item: Entity) {
        if self.consumable.get(item).is_err() {
            return;
        }
        if let Ok(mut charges) = self.counted.get_mut(item) {
            charges.left = charges.left.saturating_sub(1);
            return;
        }
        let left = match self.stacks.get_mut(item) {
            Ok(mut stack) => {
                stack.count = stack.count.saturating_sub(1);
                stack.count
            }
            Err(_) => 0,
        };
        if left == 0 && self.items.get(item).is_ok() {
            self.commands.entity(item).despawn();
        }
    }
}

/// Things in the bag that do something when they are used.
///
/// Opt-in, like every other subsystem: a game with no consumables adds
/// nothing and the components are simply never spawned. It rolls the
/// chances on its effects from the ability stream, the one every effect in
/// the engine is dealt from, so a medkit cannot shift a spell's dice.
pub struct ConsumablesPlugin;

impl Plugin for ConsumablesPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{Reads, ResolveSet, Turn};
        use crate::seed::AddStream;
        app
            // The stream every effect rolls from, whatever lands it. Asking
            // for it twice, with abilities or props in the same game, adds
            // nothing.
            .add_stream::<crate::ability::AbilityRng>("ConsumablesPlugin")
            // What an effect may write, as `PropsPlugin` registers the same
            // four for a trap's: a ration may mend, a stim may afflict, and
            // either is worth seeing. A game may have consumables and no
            // combat and no statuses, and then these queues stay empty and
            // nobody answers what nobody resolves.
            .reads::<crate::combat::DamageEvent>()
            .reads::<crate::status::Afflict>()
            .reads::<crate::status::Cure>()
            .reads::<crate::cue::Cued>()
            // After the resolver that says a use happened, in the phase it
            // happened in. Named directly because it is this crate's own
            // system in this crate's own phase, and the order is the whole
            // point: the message is written there and read here.
            .add_systems(Turn, land_uses.in_set(ResolveSet::Act).after(crate::items::resolve_items));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::items::ItemsPlugin>(app, "ConsumablesPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::{EffectKinds, FromArgs, Known};
    use crate::combat::Health;
    use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Viewshed};
    use crate::effects::{AddEngineEffects, Mend};
    use crate::items::{Inventory, ItemsPlugin, UseItem};
    use crate::plugin::headless_app;
    use crate::registries::Registries;
    use crate::seed::Seed;
    use crate::state::EngineState;
    use crate::testing::TEST_SEED;
    use crate::turn::Intent;
    use rl_rules::{DamageKind, Registry};

    /// A wounded player holding the first turn, with one thing in the bag
    /// that mends four when it is used.
    ///
    /// Built the way this crate's own item tests build theirs, on a real
    /// streamed surface with the state set to playing, because a use is an
    /// action and an action wants a turn to be dealt.
    fn rig(spent: bool, stack: Option<u32>, charges: Option<u16>) -> (App, Entity, Entity) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin, crate::combat::CombatPlugin, ItemsPlugin, ConsumablesPlugin));
        app.add_engine_effects();
        let kinds = Registry::from_defs(vec![DamageKind::new("care").unarmored()]).expect("one kind");
        let sides = Registry::from_defs(vec![rl_rules::faction::FactionDef::new("ours")]).expect("one side");
        app.insert_resource(crate::combat::CombatRules::new(&sides));
        app.insert_resource(Registries { damage_kinds: kinds, factions: sides, ..Registries::default() });
        app.insert_resource(Seed(TEST_SEED));
        let start = crate::testing::surface(&mut app);

        // One effect, built the way a game's loader builds one: through the
        // registered kinds, from the same `(kind, chance, args)` a file
        // writes.
        let effects = {
            let specs = vec![rl_rules::EffectSpec {
                kind: Mend::KIND.into(),
                chance: 100,
                args: rl_rules::ability::parse_args(r#"(kind: "care", roll: "4")"#).expect("the args parse"),
            }];
            let registries = app.world().resource::<Registries>().clone();
            let kinds = app.world().resource::<EffectKinds>();
            Effects::build(&specs, kinds, &registries.names()).expect("the mend builds")
        };
        let item = app.world_mut().spawn((Item, OnUse::new(Arc::new(effects)))).id();
        if spent {
            app.world_mut().entity_mut(item).insert(Consumable);
        }
        if let Some(count) = stack {
            app.world_mut().entity_mut(item).insert(Stack { key: 1, count });
        }
        if let Some(left) = charges {
            app.world_mut().entity_mut(item).insert(Charges::full(left));
        }
        let player = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Health::full(30), Inventory { items: vec![item] }))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds the turn a use needs");
        app.world_mut().get_mut::<Health>(player).expect("health").current = 10;
        (app, player, item)
    }

    /// Uses `item` and lets the pass that resolves it finish.
    fn use_it(app: &mut App, player: Entity, item: Entity) {
        app.world_mut().write_message(Intent::new(player, UseItem(item)));
        app.update();
        app.update();
    }

    /// Using it lands what it carries, on the one who used it, in the pass
    /// that spent the turn: no ability, nothing known, nothing aimed.
    #[test]
    fn using_a_thing_lands_what_it_carries_on_whoever_used_it() {
        let (mut app, player, item) = rig(false, None, None);
        use_it(&mut app, player, item);

        assert_eq!(app.world().get::<Health>(player).map(|h| h.current), Some(14), "the mend landed on the user");
        assert!(app.world().get_entity(item).is_ok(), "and a thing that is not consumable survives being used");
        assert!(app.world().get::<Known>(player).is_none_or(|k| k.iter().count() == 0), "using a thing is not knowing an ability");
    }

    /// A consumable walks the same ladder a charge cost walks: charges
    /// first, then the stack, then the thing itself.
    #[test]
    fn a_consumable_is_spent_off_its_charges_then_its_stack_then_itself() {
        let (mut app, player, wand) = rig(true, None, Some(2));
        use_it(&mut app, player, wand);
        assert_eq!(app.world().get::<Charges>(wand).map(|c| c.left), Some(1), "one charge off a thing that counts them");
        assert!(app.world().get_entity(wand).is_ok(), "and an empty wand is still a wand");

        let (mut app, player, stack) = rig(true, Some(2), None);
        use_it(&mut app, player, stack);
        assert_eq!(app.world().get::<Stack>(stack).map(|s| s.count), Some(1), "one off a stack that counts nothing else");

        let (mut app, player, last) = rig(true, Some(1), None);
        use_it(&mut app, player, last);
        assert!(app.world().get_entity(last).is_err(), "and the last of a stack goes with the use");
        assert!(app.world().get::<Inventory>(player).is_some_and(|b| b.items.is_empty()), "out of the bag with it");
    }

    /// A thing with nothing to land is not a use at all: the resolver still
    /// says it happened, and this leaves the item alone.
    #[test]
    fn a_thing_with_no_effects_is_left_to_whatever_the_game_makes_of_it() {
        let (mut app, player, _) = rig(false, None, None);
        let plain = app.world_mut().spawn((Item, Consumable, Stack { key: 2, count: 3 })).id();
        app.world_mut().get_mut::<Inventory>(player).expect("a bag").items.push(plain);
        use_it(&mut app, player, plain);
        assert_eq!(app.world().get::<Stack>(plain).map(|s| s.count), Some(3), "nothing to land, nothing spent");
    }
}
