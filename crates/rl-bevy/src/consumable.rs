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
//! **Worn things.** Wearing is declarative and needs no effects:
//! [`Armor`](crate::combat::Armor), [`Resists`](crate::combat::Resists), an
//! attack, [`Bestows`](crate::items::Bestows). What is here is the one rule
//! about charges and wearing: an [`Attuned`] thing refills only while it is
//! worn and is emptied each time it is put on, so a charge is earned by
//! wearing the thing rather than by carrying it.

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

/// A worn thing whose charges come back only while it is worn, and which
/// is emptied each time it is put on.
///
/// The rule that makes swapping gear cost something: a worn thing that
/// hides its wearer cannot be carried charged and put on for the one turn
/// it is needed, nor kept charging in the bag while another plate is worn. What
/// it holds is earned by wearing it. Only a thing that can be worn means
/// anything by it.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Attuned;

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
/// shot costs a charge. A thing spent to nothing is marked [`Spent`]:
/// [`remove_spent`] takes it out of play at the end of the pass and
/// [`bury_spent`] despawns it at the end of the frame.
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

/// A thing spent to nothing: out of play at the end of the pass, gone at
/// the end of the frame.
///
/// Not despawned the moment its last charge goes, for the reason the dead
/// are not: the log is written and drawn after the turns, and a stim drunk
/// to the last or a grenade spent where it landed is named in it as what
/// it was, in its own colour, rather than as something nobody could make
/// out. Empty, it does nothing more in the meantime: it has no charge to
/// use and no attack to fire, and from the end of its pass it is in no bag,
/// no slot and no cell. A save leaves it out.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Spent;

/// Takes everything [`Spent`] out of play, in
/// [`CleanupSet::Remove`](crate::plugin::CleanupSet::Remove), after the
/// pass has been written down: no longer an [`Item`](crate::items::Item),
/// so every bag and slot forgets it, and off the map, so nothing draws it
/// or picks it up. What it is called and how it looks stay for the log.
pub fn remove_spent(mut commands: Commands, spent: Query<Entity, (With<Spent>, With<crate::items::Item>)>) {
    for e in &spent {
        commands.entity(e).remove::<(crate::items::Item, crate::components::Position, crate::places::OnMap)>();
    }
}

/// Despawns everything [`Spent`], in `Last`, once the frame's log has been
/// drawn, as [`bury_the_dead`](crate::combat::bury_the_dead) does the dead.
pub fn bury_spent(mut commands: Commands, spent: Query<Entity, With<Spent>>) {
    for e in &spent {
        commands.entity(e).despawn();
    }
}

/// Counts every refilling consumable's progress up by the time that passed
/// since the last pass, and gives back a charge for each full period.
///
/// Reads the clock rather than counting passes, because a pass is one
/// actor's turn and the clock is what a refill is written in. A clock that
/// went backwards, a new run, counts as no time. An [`Attuned`] thing that
/// nobody is wearing counts nothing.
pub fn recharge_charges(
    turns: Res<Turns>,
    mut last: Local<Option<u32>>,
    mut things: Query<(Entity, &mut Consumable, Has<Attuned>)>,
    wearers: Query<&crate::items::Equipped>,
) {
    let now = turns.now();
    let passed = now.saturating_sub(last.unwrap_or(now));
    *last = Some(now);
    if passed == 0 {
        return;
    }
    let worn: Vec<Entity> = wearers.iter().flat_map(|w| w.0.worn().map(|(_, item)| item)).collect();
    for (thing, mut c, attuned) in &mut things {
        if attuned && !worn.contains(&thing) {
            continue;
        }
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

/// Empties an [`Attuned`] thing, charges and progress both, whenever it is
/// put on.
///
/// After [`recharge_charges`] in the same set, so the pass it went on in
/// counts nothing towards it.
pub fn attune(mut events: MessageReader<crate::items::ItemEvent>, mut things: Query<&mut Consumable, With<Attuned>>) {
    for ev in events.read() {
        let crate::items::ItemEvent::Equipped { item, .. } = *ev else { continue };
        let Ok(mut c) = things.get_mut(item) else { continue };
        c.left = 0;
        if let Some(r) = c.recharge.as_mut() {
            r.progress = 0;
        }
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
            .add_systems(Turn, (recharge_charges, attune).chain().in_set(TurnSet::React))
            .add_systems(Turn, remove_spent.in_set(CleanupSet::Remove))
            // With the dead, before a restart tears the run down, so what
            // the last pass spent goes with the run it was spent in.
            .add_systems(Last, bury_spent.in_set(crate::plugin::EndOfFrame::Bury));
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
        rig_with(consumable, stack, triggers, |_| {})
    }

    /// [`rig`], with `extra` run on the app once its plugins and
    /// registries are in and before the thing's triggers are built, so a
    /// test can add a plugin and the content its triggers name.
    fn rig_with(consumable: Option<Consumable>, stack: Option<u32>, triggers: &str, extra: impl FnOnce(&mut App)) -> (App, Entity, Entity) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin, crate::combat::CombatPlugin, ItemsPlugin, ConsumablesPlugin));
        app.add_engine_effects();
        let kinds = Registry::from_defs(vec![DamageKind::new("care").unarmored(), DamageKind::new("kinetic")]).expect("two kinds");
        let sides = Registry::from_defs(vec![rl_rules::faction::FactionDef::new("ours")]).expect("one side");
        app.insert_resource(crate::combat::CombatRules::new(&sides));
        app.insert_resource(Registries { damage_kinds: kinds, factions: sides, ..Registries::default() });
        app.insert_resource(Seed(TEST_SEED));
        extra(&mut app);
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

    /// Gives the player one slot and makes `item` wearable in it.
    fn a_slot_for(app: &mut App, player: Entity, item: Entity) {
        let slot = rl_rules::SlotId::from_raw(0);
        app.world_mut().entity_mut(item).insert(crate::items::Wearable(rl_rules::EquipShape::in_slot(slot)));
        app.world_mut().entity_mut(player).insert(crate::items::Equipped(rl_rules::Equipment::with_slot_count(1)));
    }

    /// Puts `item` on through the engine's own intent.
    fn put_on(app: &mut App, player: Entity, item: Entity) {
        app.world_mut().write_message(Intent::new(player, crate::items::Equip(item)));
        app.update();
    }

    /// Takes `item` off through the engine's own intent.
    fn take_off(app: &mut App, player: Entity, item: Entity) {
        app.world_mut().write_message(Intent::new(player, crate::items::Unequip(item)));
        app.update();
    }

    /// Waits until the clock reads at least `until`.
    fn wait_until(app: &mut App, player: Entity, until: u32) {
        while app.world().resource::<Turns>().now() < until {
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
        }
    }

    /// A trigger list that mends one on each pulse.
    const MEND_ON_PULSE: &str = r#"[(on: "pulse", effects: [(kind: "Mend", args: (kind: "care", roll: "1"))])]"#;

    /// `text`'s triggers, built against the rig's moments and effect kinds,
    /// the way the rig builds its own item's.
    fn triggers_of(app: &App, text: &str) -> Triggers {
        let specs: Vec<rl_rules::TriggerSpec> =
            ron::Options::default().with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME).from_str(text).expect("the triggers parse");
        let world = app.world();
        Triggers::build(&specs, &[], world.resource::<Moments>(), world.resource::<EffectKinds>(), &world.resource::<Registries>().names())
            .expect("the triggers build")
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

    /// A worn thing's pulse mends its wearer once for every period it has
    /// been worn, counted from the moment it went on, and not at all while
    /// it sits in the bag.
    #[test]
    fn a_worn_pulse_lands_once_a_period_from_when_it_went_on_and_never_from_the_bag() {
        let (mut app, player, plate) = rig(None, None, MEND_ON_PULSE);
        app.world_mut().entity_mut(plate).insert(crate::items::Pulse::every(800));
        a_slot_for(&mut app, player, plate);
        wait_until(&mut app, player, 1000);
        assert_eq!(health(&app, player), 10, "ten turns in the bag and nothing");
        let on = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        wait_until(&mut app, player, on + 700);
        assert_eq!(health(&app, player), 10, "seven turns worn, not yet");
        wait_until(&mut app, player, on + 800);
        assert_eq!(health(&app, player), 11, "eight turns worn, one mended");
        wait_until(&mut app, player, on + 1600);
        assert_eq!(health(&app, player), 12, "and one more eight turns on");
    }

    /// Taking a worn thing off and putting it back starts its clock again:
    /// seven turns worn, off, and on again is not one turn from a mend.
    #[test]
    fn putting_a_pulsing_thing_back_on_starts_its_clock_again() {
        let (mut app, player, plate) = rig(None, None, MEND_ON_PULSE);
        app.world_mut().entity_mut(plate).insert(crate::items::Pulse::every(800));
        a_slot_for(&mut app, player, plate);
        let on = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        wait_until(&mut app, player, on + 700);
        take_off(&mut app, player, plate);
        let again = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        wait_until(&mut app, player, again + 700);
        assert_eq!(health(&app, player), 10, "the seven turns before it came off counted for nothing");
        wait_until(&mut app, player, again + 800);
        assert_eq!(health(&app, player), 11);
    }

    /// An attuned thing is empty the moment it is put on and refills only
    /// while it is worn: forty turns in the bag buy nothing, and a charge
    /// comes back one period after it went on.
    #[test]
    fn an_attuned_thing_is_empty_when_put_on_and_charges_only_while_worn() {
        let (mut app, player, plate) = rig(Some(Consumable::new(1, WhenEmpty::Kept).recharging(400)), None, MEND_ON_USE);
        app.world_mut().entity_mut(plate).insert(Attuned);
        a_slot_for(&mut app, player, plate);
        let on = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(0), "full in the bag, empty once on");
        wait_until(&mut app, player, on + 300);
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(0), "three turns worn, still charging");
        wait_until(&mut app, player, on + 400);
        assert_eq!(app.world().get::<Consumable>(plate).map(|c| c.left), Some(1), "four turns worn, ready");
    }

    /// Put on, off, and on again: each putting-on empties it, and the turns
    /// it spent in the bag between refilled nothing.
    #[test]
    fn an_attuned_thing_taken_off_gains_nothing_and_is_empty_again_when_put_back_on() {
        let (mut app, player, plate) = rig(Some(Consumable::new(1, WhenEmpty::Kept).recharging(400)), None, MEND_ON_USE);
        app.world_mut().entity_mut(plate).insert(Attuned);
        a_slot_for(&mut app, player, plate);
        let on = app.world().resource::<Turns>().now();
        put_on(&mut app, player, plate);
        wait_until(&mut app, player, on + 300);
        take_off(&mut app, player, plate);
        let off = app.world().resource::<Turns>().now();
        wait_until(&mut app, player, off + 2000);
        let c = app.world().get::<Consumable>(plate).copied().unwrap();
        assert_eq!((c.left, c.recharge.map(|r| r.progress)), (0, Some(300)), "twenty turns off the body counted for nothing");
        put_on(&mut app, player, plate);
        let c = app.world().get::<Consumable>(plate).copied().unwrap();
        // Progress starts over from nothing, not from the 300 it carried
        // into the bag, but the pass that puts it on is itself a whole
        // action's worth of the clock, credited the same way the first
        // test's own put-on pass is: reset to nothing, then one turn's
        // worth of worn time, not the three hundred left over from before.
        assert_eq!((c.left, c.recharge.map(|r| r.progress)), (0, Some(100)), "and back on, it starts over");
    }

    /// A thing that is not attuned refills in the bag as it always has.
    #[test]
    fn a_thing_that_is_not_attuned_refills_in_the_bag() {
        let empty = Consumable { left: 0, ..Consumable::new(1, WhenEmpty::Kept).recharging(400) };
        let (mut app, player, wand) = rig(Some(empty), None, MEND_ON_USE);
        wait_until(&mut app, player, 400);
        assert_eq!(app.world().get::<Consumable>(wand).map(|c| c.left), Some(1));
    }

    /// A wearer on a map that is not the current one has its pulse land on
    /// nobody: a pulse is landed by who stands on the wearer's cell, and on
    /// this map that is somebody else, standing at the same coordinates.
    #[test]
    fn a_pulse_on_a_wearer_elsewhere_lands_on_nobody_here() {
        let (mut app, player, _) = rig(None, None, MEND_ON_USE);
        let at = app.world().get::<Position>(player).unwrap().0;
        let pulsing = triggers_of(&app, MEND_ON_PULSE);
        let plate = app.world_mut().spawn((Item, pulsing, crate::items::Pulse::every(100))).id();
        let slot = rl_rules::SlotId::from_raw(0);
        let mut worn = rl_rules::Equipment::with_slot_count(1);
        worn.equip(plate, &rl_rules::EquipShape::in_slot(slot)).unwrap();
        // Elsewhere: on another map, at the player's own coordinates.
        app.world_mut().spawn((
            Actor,
            Position(at),
            crate::places::OnMap(crate::places::MapId(7)),
            Health::full(30),
            Inventory { items: vec![plate] },
            crate::items::Equipped(worn),
        ));
        wait_until(&mut app, player, 500);
        assert_eq!(health(&app, player), 10, "five of its pulses came round, and none of them landed on the player standing here");
    }

    /// A carrier's `EffectBonus` reaches what it lands: a mend of four
    /// with an amount of six mends ten.
    #[test]
    fn a_carriers_effectbonus_reaches_what_it_lands() {
        let (mut app, player, item) = rig(None, None, MEND_ON_USE);
        app.world_mut().entity_mut(item).insert(crate::effects::EffectBonus { turns: 0, amount: 6 });
        use_it(&mut app, player, item);
        assert_eq!(health(&app, player), 20, "four plus six");
    }

    /// Every status event, recorded by a reader, since a headless app may
    /// swap its message buffers before a test can peek at them.
    #[derive(Resource, Default)]
    struct Heard(Vec<crate::status::StatusEvent>);

    fn hear(mut events: MessageReader<crate::status::StatusEvent>, mut heard: ResMut<Heard>) {
        heard.0.extend(events.read().copied());
    }

    /// A worn thing whose use puts `hidden` on its wearer for twenty turns,
    /// held by the thing when `while_worn`, worn in the player's one slot
    /// and used once; with statuses on and every status event heard.
    fn used_while_worn(while_worn: bool) -> (App, Entity, Entity, rl_rules::StatusId) {
        let uses = format!(r#"[(on: "use", effects: [(kind: "Inflict", args: (status: "hidden", turns: 20, while_worn: {while_worn}))])]"#);
        let (mut app, player, plate) = rig_with(None, None, &uses, |app| {
            app.add_plugins(crate::status::StatusPlugin);
            app.world_mut().resource_mut::<Registries>().statuses = Registry::from_defs(vec![rl_rules::StatusDef::new("hidden")]).unwrap();
            app.init_resource::<Heard>().add_systems(PostUpdate, hear);
        });
        let hidden = app.world().resource::<Registries>().statuses.expect("hidden");
        a_slot_for(&mut app, player, plate);
        put_on(&mut app, player, plate);
        app.world_mut().write_message(Intent::new(player, UseItem(plate)));
        app.update();
        assert!(hides(&app, player, hidden), "used while worn, and it went on");
        (app, player, plate, hidden)
    }

    fn hides(app: &App, player: Entity, hidden: rl_rules::StatusId) -> bool {
        app.world().get::<crate::status::Afflicted>(player).is_some_and(|a| a.has(hidden))
    }

    fn cured(app: &App, player: Entity, hidden: rl_rules::StatusId) -> usize {
        let cure = crate::status::StatusEvent::Cured { target: player, status: hidden };
        app.world().resource::<Heard>().0.iter().filter(|e| **e == cure).count()
    }

    /// A status a worn thing holds lasts while it is worn and ends,
    /// cured, in the pass it is taken off, with nineteen of its twenty
    /// turns still to run.
    #[test]
    fn a_status_held_by_a_worn_thing_lasts_while_it_is_worn_and_is_cured_when_it_comes_off() {
        let (mut app, player, plate, hidden) = used_while_worn(true);
        let now = app.world().resource::<Turns>().now();
        wait_until(&mut app, player, now + 300);
        assert!(hides(&app, player, hidden), "three turns on, still worn, still held");
        take_off(&mut app, player, plate);
        assert!(!hides(&app, player, hidden), "off the body, and it ended");
        assert_eq!(cured(&app, player, hidden), 1, "through the cure, once");
    }

    /// The same status landed without `while_worn` runs its time whatever
    /// becomes of the thing.
    #[test]
    fn a_status_a_worn_thing_does_not_hold_outlasts_taking_it_off() {
        let (mut app, player, plate, hidden) = used_while_worn(false);
        take_off(&mut app, player, plate);
        let now = app.world().resource::<Turns>().now();
        wait_until(&mut app, player, now + 300);
        assert!(hides(&app, player, hidden), "off the body and still on");
        assert_eq!(cured(&app, player, hidden), 0);
    }

    /// Dropping the thing is taking it off, and ends what it holds.
    #[test]
    fn dropping_a_worn_thing_ends_what_it_holds() {
        let (mut app, player, plate, hidden) = used_while_worn(true);
        app.world_mut().write_message(Intent::new(player, crate::items::DropItem(plate)));
        app.update();
        assert!(!hides(&app, player, hidden), "on the floor, and it ended");
        assert_eq!(cured(&app, player, hidden), 1);
    }

    /// Putting another thing on in its slot displaces it, and ends what it
    /// holds, even though the wearer still carries it.
    #[test]
    fn a_worn_thing_displaced_by_another_in_its_slot_ends_what_it_holds() {
        let (mut app, player, plate, hidden) = used_while_worn(true);
        let slot = rl_rules::SlotId::from_raw(0);
        let other = app.world_mut().spawn((Item, crate::items::Wearable(rl_rules::EquipShape::in_slot(slot)))).id();
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(other);
        put_on(&mut app, player, other);
        assert!(app.world().get::<Inventory>(player).is_some_and(|b| b.contains(plate)), "still carried");
        assert!(!hides(&app, player, hidden), "but no longer worn, and it ended");
        assert_eq!(cured(&app, player, hidden), 1);
    }

    /// A thing that is gone, used up or despawned, holds nothing either.
    #[test]
    fn a_worn_thing_that_is_despawned_ends_what_it_holds() {
        let (mut app, player, plate, hidden) = used_while_worn(true);
        app.world_mut().despawn(plate);
        app.world_mut().write_message(Intent::new(player, Wait));
        app.update();
        assert!(!hides(&app, player, hidden), "gone, and it ended");
        assert_eq!(cured(&app, player, hidden), 1);
    }
}
