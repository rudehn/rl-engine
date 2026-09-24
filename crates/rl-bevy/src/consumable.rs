//! What a use costs the thing that was used.
//!
//! A thing in the bag does what its triggers say, landed by the effects
//! subsystem: a stim's `use` trigger mends, a grenade's `land` trigger
//! bursts. This module is the other half, the thing's own economy:
//! [`Consumable`] counts its charges, [`spend_charges`] takes one off at
//! each moment that spends, and [`recharge_charges`] gives them back on the
//! turn clock for a thing that refills.
//!
//! **Why charges live on the thing, not on an ability.** An item never
//! lends an ability. A stim is not something the commando knows, and a
//! wand is a weapon that fires effects rather than damage; what either
//! costs to use is a fact about the thing, so it is written on the thing.
//!
//! **Which moments spend.** A use, a throw that comes to rest and an
//! attack made with the thing, by default; a game marks its own with
//! [`AddSpending::spends_on`]. A hit never spends: one shot can strike, and
//! a flaming blade is not used up by landing a blow.
//!
//! **What is not here.** Nothing about wearing. What a worn thing does is
//! declarative and needs no effects: [`Armor`](crate::combat::Armor),
//! [`Resists`](crate::combat::Resists), an attack, [`Bestows`](crate::items::Bestows)
//! for the registered stats. An `on_equip` trigger would be one-shot where
//! wearing is a standing state, and waits for a game that wants one.

use bevy::prelude::*;

use crate::effects::{Fired, MomentId, Moments};
use crate::items::Stack;
use crate::turn::Turns;

/// What happens to a thing when its last charge is spent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum WhenEmpty {
    /// It is gone: a stim, a grenade, a ration.
    Destroyed,
    /// It stays, empty, and does nothing it is spent by until it refills: a
    /// wand at zero is still a wand.
    Kept,
}

/// A thing's refill: one charge every `every` hundredths of a step, on the
/// turn clock, with `progress` counted towards the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recharge {
    /// Hundredths of a step per charge regained.
    pub every: u32,
    /// Hundredths counted towards the next charge.
    pub progress: u32,
}

/// Charges in a thing, spent at the moments that spend them.
///
/// `left` is the unit in hand. A stack of three stims is three units of one
/// charge each: spending the last charge of the unit in hand takes one off
/// the [`Stack`] and the next unit starts at `max`, and only the last unit
/// ever meets `when_empty`. Absent, a thing survives being used, which is
/// what a tool is.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Consumable {
    /// Charges left in the unit in hand.
    pub left: u16,
    /// What a fresh unit holds.
    pub max: u16,
    /// What happens when the last is spent.
    pub when_empty: WhenEmpty,
    /// How it refills, if it does.
    pub recharge: Option<Recharge>,
}

impl Consumable {
    /// A full one holding `max`.
    pub fn new(max: u16, when_empty: WhenEmpty) -> Self {
        Self { left: max, max, when_empty, recharge: None }
    }

    /// The same, regaining one charge every `every` hundredths of a step.
    pub fn recharging(mut self, every: u32) -> Self {
        self.recharge = Some(Recharge { every, progress: 0 });
        self
    }

    /// Whether there is nothing left to spend in the unit in hand.
    pub fn is_empty(&self) -> bool {
        self.left == 0
    }
}

/// The moments that spend a charge from whatever they happened to.
///
/// A resource rather than a rule written into [`spend_charges`], so a game
/// that reports a moment of its own can say whether it costs anything.
#[derive(Resource, Debug, Clone)]
pub struct SpendingMoments(Vec<MomentId>);

impl Default for SpendingMoments {
    fn default() -> Self {
        Self(vec![Moments::USE, Moments::LAND, Moments::FIRE])
    }
}

impl SpendingMoments {
    /// Whether `moment` spends.
    pub fn spends(&self, moment: MomentId) -> bool {
        self.0.contains(&moment)
    }
}

/// Marks a moment as one that spends a charge.
pub trait AddSpending {
    /// Makes `moment` spend a charge from what it happens to.
    fn spends_on(&mut self, moment: MomentId) -> &mut Self;
}

impl AddSpending for App {
    fn spends_on(&mut self, moment: MomentId) -> &mut Self {
        self.init_resource::<SpendingMoments>();
        let mut spending = self.world_mut().resource_mut::<SpendingMoments>();
        if !spending.0.contains(&moment) {
            spending.0.push(moment);
        }
        self
    }
}

/// Spends one charge from every consumable whose moment spends.
///
/// After [`land_triggers`](crate::effects::land_triggers), so what the
/// last charge did has landed before the thing is gone. A moment with no
/// triggers on the thing still spends, which is how a plain wand's ordinary
/// shot costs a charge. A thing spent to nothing is marked [`Spent`] and
/// despawned at the end of the pass by [`remove_spent`], and the bag it was
/// in forgets it in [`forget_removed_items`](crate::items::forget_removed_items),
/// where everything that stops being an item is forgotten.
pub fn spend_charges(
    mut commands: Commands,
    mut fired: MessageReader<Fired>,
    spending: Res<SpendingMoments>,
    mut things: Query<(&mut Consumable, Option<&mut Stack>)>,
) {
    for f in fired.read() {
        if !spending.spends(f.moment) {
            continue;
        }
        let Ok((mut c, stack)) = things.get_mut(f.on) else { continue };
        c.left = c.left.saturating_sub(1);
        if c.left > 0 {
            continue;
        }
        match stack {
            Some(mut s) if s.count > 1 => {
                s.count -= 1;
                c.left = c.max;
            }
            _ if c.when_empty == WhenEmpty::Destroyed => {
                commands.entity(f.on).insert(Spent);
            }
            _ => {}
        }
    }
}

/// A thing spent to nothing, gone at the end of the pass.
///
/// Not despawned the moment its last charge goes, because the pass is not
/// over: the log is written after every reaction, and a stim drunk to the
/// last or a grenade spent where it landed is named in it as what it was,
/// not as something nobody could make out. Empty, it does nothing more in
/// the meantime: it has no charge to use and no attack to fire.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Spent;

/// Despawns everything [`Spent`], in
/// [`CleanupSet::Remove`](crate::plugin::CleanupSet::Remove), after the
/// pass has been written down.
pub fn remove_spent(mut commands: Commands, spent: Query<Entity, With<Spent>>) {
    for e in &spent {
        commands.entity(e).despawn();
    }
}

/// Counts every refilling consumable's progress up by the time that passed
/// since the last pass, and gives back a charge for each full period.
///
/// Reads the clock rather than counting passes, because a pass is one
/// actor's turn and the clock is what a refill is written in. A clock that
/// went backwards, a new run, counts as no time.
pub fn recharge_charges(turns: Res<Turns>, mut last: Local<Option<u32>>, mut things: Query<&mut Consumable>) {
    let now = turns.now();
    let passed = now.saturating_sub(last.unwrap_or(now));
    *last = Some(now);
    if passed == 0 {
        return;
    }
    for mut c in &mut things {
        let (left, max) = (c.left, c.max);
        let Some(r) = c.recharge.as_mut() else { continue };
        if left >= max {
            r.progress = 0;
            continue;
        }
        let every = r.every.max(1);
        r.progress += passed;
        let gained = (r.progress / every).min(u32::from(max - left)) as u16;
        r.progress %= every;
        let full = left + gained >= max;
        if full {
            r.progress = 0;
        }
        c.left = (left + gained).min(max);
    }
}

/// What using, throwing and firing a thing costs it.
///
/// Opt-in, like every other subsystem: a game with no consumables adds
/// nothing and the component is simply never spawned. Adds
/// [`EffectsPlugin`](crate::effects::EffectsPlugin) if nothing has, since
/// what a spent thing did is landed there.
pub struct ConsumablesPlugin;

impl Plugin for ConsumablesPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{CleanupSet, ResolveSet, Turn, TurnSet};
        crate::effects::ensure(app);
        app.init_resource::<SpendingMoments>()
            // After the triggers land, in the same set, so the last charge's
            // effects are in before the thing goes.
            .add_systems(Turn, spend_charges.in_set(ResolveSet::Triggers).after(crate::effects::land_triggers))
            .add_systems(Turn, recharge_charges.in_set(TurnSet::React))
            .add_systems(Turn, remove_spent.in_set(CleanupSet::Remove));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::items::ItemsPlugin>(app, "ConsumablesPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ability::Known;
    use crate::combat::Health;
    use crate::components::{Actor, Blocks, MyTurn, Player, Position, RevealsMap, Viewshed};
    use crate::effects::{AddEngineEffects, EffectKinds, Moments, Triggers};
    use crate::items::{Inventory, Item, ItemsPlugin, UseItem};
    use crate::plugin::headless_app;
    use crate::registries::Registries;
    use crate::seed::Seed;
    use crate::state::EngineState;
    use crate::testing::TEST_SEED;
    use crate::turn::{ActionRefused, Intent, Turns, Wait};
    use rl_rules::{DamageKind, Registry};

    /// A trigger list that mends four on a use.
    const MEND_ON_USE: &str = r#"[(on: "use", effects: [(kind: "Mend", args: (kind: "care", roll: "4"))])]"#;

    /// A wounded player holding the first turn, with one thing in the bag
    /// carrying `triggers`, and `consumable` and a stack of `stack` if given.
    ///
    /// On a real streamed surface with the state set to playing, because a
    /// use is an action and an action wants a turn to be dealt.
    fn rig(consumable: Option<Consumable>, stack: Option<u32>, triggers: &str) -> (App, Entity, Entity) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin, crate::combat::CombatPlugin, ItemsPlugin, ConsumablesPlugin));
        app.add_engine_effects();
        let kinds = Registry::from_defs(vec![DamageKind::new("care").unarmored(), DamageKind::new("kinetic")]).expect("two kinds");
        let sides = Registry::from_defs(vec![rl_rules::faction::FactionDef::new("ours")]).expect("one side");
        app.insert_resource(crate::combat::CombatRules::new(&sides));
        app.insert_resource(Registries { damage_kinds: kinds, factions: sides, ..Registries::default() });
        app.insert_resource(Seed(TEST_SEED));
        let start = crate::testing::surface(&mut app);
        let triggers = {
            let specs: Vec<rl_rules::TriggerSpec> =
                ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME).from_str(triggers).expect("the triggers parse");
            let world = app.world();
            let registries = world.resource::<Registries>();
            Triggers::build(&specs, &[], world.resource::<Moments>(), world.resource::<EffectKinds>(), &registries.names()).expect("the triggers build")
        };
        let item = app.world_mut().spawn((Item, triggers)).id();
        if let Some(c) = consumable {
            app.world_mut().entity_mut(item).insert(c);
        }
        if let Some(count) = stack {
            app.world_mut().entity_mut(item).insert(Stack { key: 1, count });
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

    fn health(app: &App, who: Entity) -> i32 {
        app.world().get::<Health>(who).expect("health").current
    }

    /// Using it lands its use trigger, on the one who used it, in the pass
    /// that spent the turn: no ability, nothing known, nothing aimed. And a
    /// thing that is not consumable survives being used.
    #[test]
    fn a_use_lands_the_use_trigger_on_whoever_used_it_and_a_tool_survives() {
        let (mut app, player, item) = rig(None, None, MEND_ON_USE);
        use_it(&mut app, player, item);
        assert_eq!(health(&app, player), 14, "the mend landed on the user");
        assert!(app.world().get_entity(item).is_ok(), "not consumable, so it survives");
        assert!(app.world().get::<Known>(player).is_none_or(|k| k.is_empty()), "using a thing is not knowing an ability");
    }

    #[test]
    fn a_use_takes_one_charge_from_a_thing_that_holds_several() {
        let (mut app, player, item) = rig(Some(Consumable::new(3, WhenEmpty::Kept)), None, MEND_ON_USE);
        use_it(&mut app, player, item);
        assert_eq!(app.world().get::<Consumable>(item).map(|c| c.left), Some(2));
    }

    #[test]
    fn the_last_charge_of_a_stack_takes_one_off_the_stack_and_the_next_unit_starts_full() {
        let (mut app, player, item) = rig(Some(Consumable::new(1, WhenEmpty::Destroyed)), Some(3), MEND_ON_USE);
        use_it(&mut app, player, item);
        assert_eq!(app.world().get::<Stack>(item).map(|s| s.count), Some(2));
        assert_eq!(app.world().get::<Consumable>(item).map(|c| c.left), Some(1), "the next unit starts full");
    }

    #[test]
    fn the_last_of_a_destroyed_thing_is_gone_from_the_bag_and_the_last_of_a_kept_one_stays_at_nothing() {
        let (mut app, player, stim) = rig(Some(Consumable::new(1, WhenEmpty::Destroyed)), None, MEND_ON_USE);
        use_it(&mut app, player, stim);
        assert!(app.world().get_entity(stim).is_err(), "destroyed");
        assert!(app.world().get::<Inventory>(player).is_some_and(|b| b.items.is_empty()), "and out of the bag with it");
        let (mut app, player, wand) = rig(Some(Consumable::new(1, WhenEmpty::Kept)), None, MEND_ON_USE);
        use_it(&mut app, player, wand);
        assert_eq!(app.world().get::<Consumable>(wand).map(|c| c.left), Some(0), "kept, empty");
    }

    #[test]
    fn using_an_empty_thing_is_refused_the_turn_is_kept_and_nothing_lands() {
        let empty = Consumable { left: 0, ..Consumable::new(3, WhenEmpty::Kept) };
        let (mut app, player, wand) = rig(Some(empty), None, MEND_ON_USE);
        app.world_mut().resource_mut::<Messages<ActionRefused>>().clear();
        let before = app.world().resource::<Turns>().now();
        use_it(&mut app, player, wand);
        assert_eq!(app.world().resource::<Turns>().now(), before, "refused, so no time passed");
        assert_eq!(health(&app, player), 10, "and nothing landed");
        let refused = app.world_mut().resource_mut::<Messages<ActionRefused>>().drain().any(|r| r.actor == player);
        assert!(refused, "the player heard it was refused");
    }

    #[test]
    fn a_recharge_returns_a_charge_every_period_on_the_clock_and_keeps_the_remainder() {
        let empty = Consumable { left: 0, ..Consumable::new(3, WhenEmpty::Kept).recharging(250) };
        let (mut app, player, wand) = rig(Some(empty), None, MEND_ON_USE);
        for _ in 0..3 {
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
        }
        let c = app.world().get::<Consumable>(wand).copied().expect("still a wand");
        assert_eq!((c.left, c.recharge.map(|r| r.progress)), (1, Some(50)), "three turns of a hundred is one charge and fifty over");
    }

    #[test]
    fn a_thing_with_a_use_and_a_land_trigger_used_from_the_bag_answers_only_use() {
        let both = r#"[(on: "use", effects: [(kind: "Mend", args: (kind: "care", roll: "3"))]), (on: "land", effects: [(kind: "Harm", args: (kind: "kinetic", roll: "9"))])]"#;
        let (mut app, player, item) = rig(Some(Consumable::new(2, WhenEmpty::Kept)), None, both);
        use_it(&mut app, player, item);
        assert_eq!(health(&app, player), 13, "mended three, and the land trigger did not go off");
    }
}
