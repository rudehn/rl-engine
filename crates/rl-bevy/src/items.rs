//! Items: on the ground, in a bag, or worn.
//!
//! An item is an entity. On the ground it has a [`Position`] and the
//! [`OnMap`] it lies on; in a bag it is listed in the carrier's
//! [`Inventory`] and has neither, since a carried thing goes wherever its
//! carrier does; worn, it is also claimed in the carrier's [`Equipped`]
//! slots. The engine resolves the moves between those three states and
//! charges a turn for each. What an item does when used is its `use`
//! trigger, landed by the effects subsystem, and what a use costs it is its
//! [`Consumable`](crate::consumable::Consumable); a use is reported both as
//! [`ItemEvent::Used`], for the game, and as the `use` moment, for the
//! triggers. An item never lends an ability.
//!
//! What wearing an item does is the item's to say and the engine's to
//! apply. Its combat components are read straight off it by
//! [`Loadout`](crate::combat::Loadout) whenever a blow is struck or met,
//! and what it [`Bestows`] on the registered stats is folded into the
//! wearer's [`StatBlock`] by [`fold_gear`] the moment its slots change.
//! Nothing is copied onto the wearer and nothing has to be remembered
//! when it comes off.
//!
//! Throwing one is [`throwing`](crate::throwing), which needs combat as well.

use bevy::prelude::*;
use rl_core::Point;
use rl_core::turn::BASE_ACTION_COST;
use rl_rules::stats::{Modifier, Op, Source};
use rl_rules::{EquipShape, Equipment, StatId};

use crate::combat::DeathEvent;
use crate::components::{Actor, MyTurn, Position};
use crate::minds::{Sight, Thinking};
use crate::places::{MapId, OnMap};
use crate::status::StatBlock;
use crate::throwing::Throwable;
use crate::turn::{Action, Intent, Resolution};
use crate::world::WorldMap;

/// An item.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Item;

/// What an actor carries, in pickup order. Worn items stay listed here.
#[derive(Component, Debug, Clone, Default)]
pub struct Inventory {
    /// The items.
    pub items: Vec<Entity>,
}

impl Inventory {
    /// Whether `item` is carried.
    pub fn contains(&self, item: Entity) -> bool {
        self.items.contains(&item)
    }

    /// Drops `item` from the list. Returns whether it was there.
    pub fn remove(&mut self, item: Entity) -> bool {
        let before = self.items.len();
        self.items.retain(|i| *i != item);
        before != self.items.len()
    }
}

/// What an actor wears.
#[derive(Component, Debug, Clone, Deref, DerefMut)]
pub struct Equipped(pub Equipment<Entity>);

/// Where an item goes when worn. Items without this cannot be equipped.
#[derive(Component, Debug, Clone)]
pub struct Wearable(pub EquipShape);

/// How much wearing an item is worth, in the game's own units, so a mind
/// can tell better gear from worse without knowing what armor is.
///
/// A mind weighs an item lying in sight against the worn items it would
/// displace, and one worth more than all of them together is worth putting
/// on. Gear without a score is never worth changing into; only the
/// comparison matters, so any scale the game likes will do.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct GearScore(pub i32);

/// An item's enchant level and affixes: what it is called and what a
/// game writes its combat components and [`Bestows`] from when it spawns
/// it.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
pub struct Enchant(pub rl_rules::Enchanted);

/// What an item does to its wearer's registered stats while it is worn.
///
/// An affix that sharpens the hand or a coat that hardens the skin is a
/// change to a stat, and this is where the item says so. The game writes
/// it when the item is spawned, enchant already applied, since an enchant
/// is rolled per instance; [`fold_gear`] puts every worn item's onto the
/// wearer's [`StatBlock`] tagged with the item, and takes them off again
/// with it. An item whose worth is a blow or armor carries a
/// [`MeleeAttack`](crate::combat::MeleeAttack) or an
/// [`Armor`](crate::combat::Armor) instead, which need no stat at all.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Bestows(pub Vec<(StatId, Op)>);

/// What an item counts as.
///
/// The affix model already rolls against tags; abilities read the same
/// ones to ask whether a shield is on the arm or a powder charge is in the
/// bag, so they live on the item rather than in a table only the game can
/// read.
#[derive(Component, Debug, Clone, Default, Deref, DerefMut)]
pub struct Tagged(pub Vec<rl_rules::TagId>);

/// A countable item: picking one up merges it into a carried item with the
/// same key instead of adding an entry. The key is the game's, usually
/// the definition id.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stack {
    /// What it is a stack of.
    pub key: u64,
    /// How many.
    pub count: u32,
}

/// What happened to an item, for narration and for the game's own
/// reactions. Written in [`TurnSet::Resolve`](crate::plugin::TurnSet::Resolve).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemEvent {
    /// `actor` took `item` off the ground. `merged_into` names the carried
    /// stack it joined, in which case `item` no longer exists.
    PickedUp {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
        /// The stack it was merged into, if any.
        merged_into: Option<Entity>,
    },
    /// `actor` put `item` on the ground at `at`, or let it fall there on
    /// dying.
    Dropped {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
        /// Where.
        at: Position,
    },
    /// `actor` put `item` on.
    Equipped {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
    },
    /// `actor` took `item` off, whether by choice or because something
    /// else took its slot.
    Unequipped {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
    },
    /// `actor` used `item`. The game decides what that means.
    Used {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
    },
    /// `actor` threw `item`, which came to rest at `at` after striking
    /// `struck`, if it met anyone on the way. A throw from a stack names the
    /// one that left the hand, which is an entity of its own from then on.
    Thrown {
        /// Who.
        actor: Entity,
        /// What.
        item: Entity,
        /// Where it came to rest.
        at: Position,
        /// Whoever it struck.
        struck: Option<Entity>,
    },
}

/// The actor holding the turn, as the item resolver sees it.
type Carrier<'w, 's> = Query<'w, 's, (&'static Position, &'static mut Inventory, Option<&'static mut Equipped>), With<MyTurn>>;

/// Items on the ground.
type Ground<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>), With<Item>>;

/// What the item resolver reads and moves.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ItemWorld<'w, 's> {
    carriers: Carrier<'w, 's>,
    ground: Ground<'w, 's>,
    stacks: Query<'w, 's, &'static Stack>,
    wearables: Query<'w, 's, &'static Wearable>,
    consumables: Query<'w, 's, &'static crate::consumable::Consumable>,
    map: Res<'w, WorldMap>,
}

/// Take everything lying on the actor's cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PickUp;
impl Action for PickUp {}

/// Put a carried item on the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropItem(pub Entity);
impl Action for DropItem {}

/// Put a carried item on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Equip(pub Entity);
impl Action for Equip {}

/// Take an item lying on the actor's cell and put it on, in one action.
///
/// It costs [`EQUIP_FROM_GROUND_COST`], half as much again as picking up or
/// putting on alone: quicker than doing both, dearer than either, so taking
/// up a sword in the middle of a fight is a real choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipFromGround(pub Entity);
impl Action for EquipFromGround {}

/// What [`EquipFromGround`] costs, in hundredths of a step.
pub const EQUIP_FROM_GROUND_COST: u32 = BASE_ACTION_COST * 3 / 2;

/// Take a worn item off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unequip(pub Entity);
impl Action for Unequip {}

/// Use a carried item.
///
/// The engine charges the turn, reports [`ItemEvent::Used`] and the `use`
/// moment, and the item's triggers do the rest. A use of an empty
/// [`Consumable`](crate::consumable::Consumable) is refused, and the
/// player keeps the turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseItem(pub Entity);
impl Action for UseItem {}

/// This module's actions as its resolver sees them, in one list, so that
/// one turn spends one of them whichever kind it is.
#[derive(Clone, Copy)]
enum Which {
    PickUp,
    Drop(Entity),
    Equip(Entity),
    EquipFromGround(Entity),
    Unequip(Entity),
    Use(Entity),
}

impl Which {
    /// What the action costs, done or failed.
    fn cost(self) -> u32 {
        match self {
            Which::EquipFromGround(_) => EQUIP_FROM_GROUND_COST,
            _ => BASE_ACTION_COST,
        }
    }
}

/// Every item intent written this pass.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ItemIntents<'w, 's> {
    pick_ups: MessageReader<'w, 's, Intent<PickUp>>,
    drops: MessageReader<'w, 's, Intent<DropItem>>,
    equips: MessageReader<'w, 's, Intent<Equip>>,
    from_ground: MessageReader<'w, 's, Intent<EquipFromGround>>,
    unequips: MessageReader<'w, 's, Intent<Unequip>>,
    uses: MessageReader<'w, 's, Intent<UseItem>>,
}

impl ItemIntents<'_, '_> {
    fn drain(&mut self) -> Vec<(Entity, Which)> {
        let mut out: Vec<(Entity, Which)> = self.pick_ups.read().map(|i| (i.actor, Which::PickUp)).collect();
        out.extend(self.drops.read().map(|i| (i.actor, Which::Drop(i.action.0))));
        out.extend(self.equips.read().map(|i| (i.actor, Which::Equip(i.action.0))));
        out.extend(self.from_ground.read().map(|i| (i.actor, Which::EquipFromGround(i.action.0))));
        out.extend(self.unequips.read().map(|i| (i.actor, Which::Unequip(i.action.0))));
        out.extend(self.uses.read().map(|i| (i.actor, Which::Use(i.action.0))));
        out
    }
}

/// Resolves pickups, drops, equips, unequips and uses for the actor
/// holding the turn. Each costs one action, and equipping from the ground
/// half as much again. An impossible one, such as picking up from bare
/// ground, is refused for the player and treated as a wait for anyone else,
/// like an impossible move.
pub fn resolve_items(
    mut commands: Commands,
    mut intents: ItemIntents,
    mut resolution: Resolution,
    world: ItemWorld,
    mut events: MessageWriter<ItemEvent>,
    mut fired: MessageWriter<crate::effects::Fired>,
) {
    let ItemWorld { mut carriers, ground, stacks, wearables, consumables, map } = world;
    let this_map = map.current();
    let lies_at = |item: Entity, at: Point| ground.get(item).is_ok_and(|(_, p, on)| p.0 == at && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == this_map);
    for (actor, which) in intents.drain() {
        if !resolution.claim(actor) {
            continue;
        }
        // An actor with no bag to act on has failed like any other
        // impossible item action, rather than holding a claimed turn.
        let ok = 'attempt: {
            match which {
                Which::PickUp => {
                    let Ok((pos, mut bag, _)) = carriers.get_mut(actor) else { break 'attempt false };
                    let here: Vec<Entity> =
                        ground.iter().filter(|(_, p, on)| p.0 == pos.0 && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == this_map).map(|(e, _, _)| e).collect();
                    for item in &here {
                        commands.entity(*item).remove::<(Position, OnMap)>();
                        let merged_into = stacks.get(*item).ok().and_then(|s| bag.items.iter().copied().find(|c| stacks.get(*c).is_ok_and(|t| t.key == s.key)));
                        match merged_into {
                            Some(into) => {
                                let add = stacks.get(*item).map(|s| s.count).unwrap_or(1);
                                commands.entity(into).entry::<Stack>().and_modify(move |mut s| s.count += add);
                                commands.entity(*item).despawn();
                            }
                            None => bag.items.push(*item),
                        }
                        events.write(ItemEvent::PickedUp { actor, item: *item, merged_into });
                    }
                    !here.is_empty()
                }
                Which::Drop(item) => {
                    let Ok((pos, mut bag, worn)) = carriers.get_mut(actor) else { break 'attempt false };
                    if !bag.remove(item) {
                        false
                    } else {
                        if let Some(mut worn) = worn
                            && worn.unequip(item)
                        {
                            events.write(ItemEvent::Unequipped { actor, item });
                        }
                        // On the map it is put down on, whichever it was picked up from.
                        commands.entity(item).insert((*pos, OnMap(this_map)));
                        events.write(ItemEvent::Dropped { actor, item, at: *pos });
                        true
                    }
                }
                Which::Equip(item) => {
                    let Ok((_, bag, worn)) = carriers.get_mut(actor) else { break 'attempt false };
                    match (worn, wearables.get(item)) {
                        (Some(mut worn), Ok(shape)) if bag.contains(item) => match worn.equip(item, &shape.0) {
                            Ok(displaced) => {
                                for other in displaced {
                                    events.write(ItemEvent::Unequipped { actor, item: other });
                                }
                                events.write(ItemEvent::Equipped { actor, item });
                                true
                            }
                            Err(e) => {
                                warn!("{actor:?} could not equip {item:?}: {e}");
                                false
                            }
                        },
                        _ => false,
                    }
                }
                Which::EquipFromGround(item) => {
                    let Ok((pos, mut bag, worn)) = carriers.get_mut(actor) else { break 'attempt false };
                    match (worn, wearables.get(item)) {
                        (Some(mut worn), Ok(shape)) if lies_at(item, pos.0) => match worn.equip(item, &shape.0) {
                            Ok(displaced) => {
                                commands.entity(item).remove::<(Position, OnMap)>();
                                bag.items.push(item);
                                events.write(ItemEvent::PickedUp { actor, item, merged_into: None });
                                for other in displaced {
                                    events.write(ItemEvent::Unequipped { actor, item: other });
                                }
                                events.write(ItemEvent::Equipped { actor, item });
                                true
                            }
                            Err(e) => {
                                warn!("{actor:?} could not equip {item:?} from the ground: {e}");
                                false
                            }
                        },
                        _ => false,
                    }
                }
                Which::Unequip(item) => {
                    let Ok((_, _, worn)) = carriers.get_mut(actor) else { break 'attempt false };
                    let taken_off = worn.is_some_and(|mut worn| worn.unequip(item));
                    if taken_off {
                        events.write(ItemEvent::Unequipped { actor, item });
                    }
                    taken_off
                }
                Which::Use(item) => {
                    let Ok((pos, bag, _)) = carriers.get_mut(actor) else { break 'attempt false };
                    // An empty wand is still a wand, and using one is a
                    // mistake the player keeps the turn for, as for any
                    // impossible item action.
                    let empty = consumables.get(item).is_ok_and(|c| c.is_empty());
                    if bag.contains(item) && !empty {
                        events.write(ItemEvent::Used { actor, item });
                        fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::USE, by: Some(actor), at: pos.0 });
                        true
                    } else {
                        false
                    }
                }
            }
        };
        if ok {
            resolution.done(actor, which.cost());
        } else {
            resolution.failed(actor, which.cost());
        }
    }
}

/// Folds what every worn item [`Bestows`] into its wearer's stats.
///
/// Rebuilt rather than edited, the way [`Known`](crate::ability::Known)
/// is: every modifier tagged [`Source::Item`] is dropped and each worn
/// item's put back, in slot order, so an item taken off takes its changes
/// with it and nothing has to remember that it did. A status's modifiers
/// carry their own tag and are left where they are.
///
/// Runs whenever [`Equipped`] changed, which is also the first pass after
/// a wearer is spawned with its slots filled: a run restored from a save
/// rebuilds its gear modifiers for nothing, and a save never has to write
/// them down. A game that changes what a worn item bestows touches the
/// wearer's `Equipped` to have it folded again.
pub fn fold_gear(mut wearers: Query<(&Equipped, &mut StatBlock), Changed<Equipped>>, items: Query<&Bestows, With<Item>>) {
    for (worn, mut stats) in &mut wearers {
        stats.0.retain_sources(|source| !source.is_item());
        for (_, item) in worn.0.worn() {
            let Ok(bestows) = items.get(item) else { continue };
            for (stat, op) in &bestows.0 {
                stats.0.add(Modifier::new(*stat, *op, Source::Item(item.to_bits())));
            }
        }
    }
}

/// Drops despawned items from every bag and every slot.
pub fn forget_removed_items(mut removed: RemovedComponents<Item>, mut carriers: Query<(&mut Inventory, Option<&mut Equipped>)>) {
    let gone: Vec<Entity> = removed.read().collect();
    if gone.is_empty() {
        return;
    }
    for (mut bag, worn) in &mut carriers {
        bag.items.retain(|i| !gone.contains(i));
        if let Some(mut worn) = worn {
            for item in &gone {
                worn.unequip(*item);
            }
        }
    }
}

/// Lets whatever a dead actor carried fall where it died.
///
/// Otherwise a monster that picked up a knife takes it out of the world
/// with it, since the dead are despawned and a carried item has no place of
/// its own. The player is left to the game, which may yet revive it.
pub fn drop_what_the_dead_carried(
    mut commands: Commands,
    mut deaths: MessageReader<DeathEvent>,
    map: Res<WorldMap>,
    mut carriers: Query<(&mut Inventory, Option<&mut Equipped>)>,
    mut events: MessageWriter<ItemEvent>,
) {
    for death in deaths.read() {
        if death.was_player {
            continue;
        }
        let Ok((mut bag, worn)) = carriers.get_mut(death.entity) else { continue };
        if let Some(mut worn) = worn {
            for item in &bag.items {
                worn.unequip(*item);
            }
        }
        let at = Position(death.at);
        for item in bag.items.drain(..) {
            commands.entity(item).insert((at, OnMap(map.current())));
            events.write(ItemEvent::Dropped { actor: death.entity, item, at });
        }
    }
}

/// An item lying about, as a mind weighs it.
type Lying = (Entity, &'static Position, Option<&'static OnMap>, Option<&'static Throwable>, Option<&'static Wearable>, Option<&'static GearScore>);

/// What a mind carries and wears, and what it might see lying about.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Belongings<'w, 's> {
    bags: Query<'w, 's, (Option<&'static Inventory>, Option<&'static Equipped>)>,
    missiles: Query<'w, 's, &'static Throwable>,
    scores: Query<'w, 's, &'static GearScore>,
    lying: Query<'w, 's, Lying, With<Item>>,
}

impl Belongings<'_, '_> {
    /// What `thinker` carries that it could throw.
    fn missiles(&self, thinker: Entity) -> Vec<rl_rules::Missile<Entity>> {
        let Ok((Some(bag), _)) = self.bags.get(thinker) else { return Vec::new() };
        bag.items.iter().filter_map(|item| self.missiles.get(*item).ok().map(|t| rl_rules::Missile { item: *item, range: t.range })).collect()
    }

    /// How much better `thinker` would be for wearing `item` in `shape`,
    /// scored `score`, than for what it would displace; `None` when it wears
    /// nothing or the item is not scored.
    fn gain(&self, thinker: Entity, item: Entity, shape: &Wearable, score: &GearScore) -> Option<i32> {
        let Ok((_, Some(worn))) = self.bags.get(thinker) else { return None };
        // Tried on a copy: what the real equip would displace, and nothing moved.
        let displaced = worn.0.clone().equip(item, &shape.0).ok()?;
        Some(score.0 - displaced.iter().map(|d| self.scores.get(*d).map_or(0, |s| s.0)).sum::<i32>())
    }
}

/// Tells the mind holding the turn what it carries to throw and what lies
/// in sight worth having.
///
/// Items' contribution to a mind's knowledge, in
/// [`PerceiveSet::Annotate`](crate::plugin::PerceiveSet::Annotate). Only a
/// mind with the wits to pick up or put on is told what lies about; what
/// it stands on it can feel, and anything else it has to see. How far its
/// shot reaches is combat's to tell, in
/// [`perceive_reach`](crate::combat::perceive_reach), since a gun a mind
/// was built with needs no items at all.
pub fn perceive_belongings(mut thinking: ResMut<Thinking>, sight: Sight, belongings: Belongings) {
    let Some(thinker) = thinking.actor() else { return };
    let missiles = belongings.missiles(thinker);
    let wits = thinking.snapshot().map(|s| s.wits).unwrap_or_default();
    let mut items = Vec::new();
    if wits.has(rl_rules::Wits::PICKS_UP) || wits.has(rl_rules::Wits::EQUIPS) {
        for (item, pos, on, throwable, wearable, score) in belongings.lying.iter() {
            let underfoot = pos.0 == thinking.at() && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == sight.current_map();
            if !underfoot && !sight.perceives(&thinking, pos.0, on) {
                continue;
            }
            let gain = wearable.zip(score).and_then(|(shape, score)| belongings.gain(thinker, item, shape, score));
            items.push(rl_rules::ItemView { id: item, pos: pos.0, throw_range: throwable.map(|t| t.range), gain });
        }
    }
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.missiles = missiles;
        snapshot.items.extend(items);
    }
}

/// Items: on the ground, in a bag, in a slot, the six actions that move
/// them between the three, and the fold of what worn items bestow.
///
/// Every [`Actor`] is given an empty [`StatBlock`] as it is spawned, the
/// way [`StatusPlugin`](crate::status::StatusPlugin) gives one, so a
/// wearer in a game with gear stats and no statuses still has somewhere
/// for them to land.
pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{CleanupSet, ResolveSet, Turn, TurnSet};
        use crate::turn::AddAction;
        // `try`, because the status plugin registers the same requirement
        // and the order a game lists its plugins in must not matter.
        let _ = app.try_register_required_components::<Actor, StatBlock>();
        app.add_message::<ItemEvent>()
            // Read to let the dead drop what they carried. A game without
            // combat has no deaths, and the reader reads nothing.
            .add_message::<DeathEvent>()
            .add_action::<PickUp>()
            .add_action::<DropItem>()
            .add_action::<Equip>()
            .add_action::<EquipFromGround>()
            .add_action::<Unequip>()
            .add_action::<UseItem>()
            // What a use reports, for the triggers it sets off. Registered
            // here as well as by the effects plugin, since a game may have
            // items and no effects, and then the queue simply stays empty.
            .add_message::<crate::effects::Fired>()
            .add_systems(Turn, perceive_belongings.in_set(crate::plugin::PerceiveSet::Annotate))
            .add_systems(Turn, resolve_items.in_set(ResolveSet::Act))
            // In the pass the slots changed in, so gear counts from the
            // moment it is worn.
            .add_systems(Turn, fold_gear.in_set(TurnSet::React))
            .add_systems(Turn, drop_what_the_dead_carried.in_set(CleanupSet::Remove))
            .add_systems(Turn, forget_removed_items.in_set(CleanupSet::Requeue));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "ItemsPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, Player, RevealsMap, Viewshed};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::Turns;
    use rl_rules::Registry;
    use rl_rules::SlotDef;

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        main: rl_rules::SlotId,
        off: rl_rules::SlotId,
    }

    fn rig() -> Rig {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, ItemsPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let slots = Registry::from_defs(vec![SlotDef::new("main"), SlotDef::new("off")]).unwrap();
        let (main, off) = (slots.expect("main"), slots.expect("off"));
        let player = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Inventory::default(), Equipped(Equipment::for_slots(&slots))))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        assert!(app.world().get::<MyTurn>(player).is_some());
        Rig { app, player, start, main, off }
    }

    fn act<A: Action>(rig: &mut Rig, action: A) -> Vec<ItemEvent> {
        rig.app.world_mut().write_message(Intent { actor: rig.player, action });
        rig.app.update();
        let mut events = rig.app.world_mut().resource_mut::<Messages<ItemEvent>>();
        events.drain().collect()
    }

    #[test]
    fn pick_up_carry_wear_and_drop() {
        let mut r = rig();
        let sword = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main)))).id();
        let axe = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main).and_claims(r.off)))).id();
        let events = act(&mut r, PickUp);
        assert_eq!(events.len(), 2);
        assert!(r.app.world().get::<Position>(sword).is_none(), "off the ground");
        assert!(r.app.world().get::<OnMap>(sword).is_none(), "and on no map while carried");
        assert_eq!(r.app.world().get::<Inventory>(r.player).unwrap().items, vec![sword, axe]);
        assert_eq!(r.app.world().resource::<Turns>().now(), 100, "picking up cost a turn");

        assert_eq!(act(&mut r, Equip(sword)), vec![ItemEvent::Equipped { actor: r.player, item: sword }]);
        assert_eq!(
            act(&mut r, Equip(axe)),
            vec![ItemEvent::Unequipped { actor: r.player, item: sword }, ItemEvent::Equipped { actor: r.player, item: axe }],
            "the two-hander pushed the sword out"
        );
        let worn = r.app.world().get::<Equipped>(r.player).unwrap();
        assert_eq!(worn.in_slot(r.off), Some(axe));
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().contains(sword), "still carried");

        let events = act(&mut r, DropItem(axe));
        assert_eq!(
            events,
            vec![ItemEvent::Unequipped { actor: r.player, item: axe }, ItemEvent::Dropped { actor: r.player, item: axe, at: Position(r.start) }]
        );
        assert_eq!(r.app.world().get::<Position>(axe).unwrap().0, r.start);
        assert!(!r.app.world().get::<Inventory>(r.player).unwrap().contains(axe));
        assert!(r.app.world().get::<Equipped>(r.player).unwrap().is_free(r.main));
    }

    #[test]
    fn equipping_from_the_ground_is_one_action_and_half_again() {
        let mut r = rig();
        let sword = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main)))).id();
        r.app.update();
        let events = act(&mut r, EquipFromGround(sword));
        assert_eq!(events, vec![ItemEvent::PickedUp { actor: r.player, item: sword, merged_into: None }, ItemEvent::Equipped { actor: r.player, item: sword }]);
        assert_eq!(r.app.world().get::<Equipped>(r.player).unwrap().in_slot(r.main), Some(sword), "worn");
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().contains(sword), "and carried");
        assert!(r.app.world().get::<Position>(sword).is_none(), "and off the ground");
        assert_eq!(r.app.world().resource::<Turns>().now(), EQUIP_FROM_GROUND_COST, "for a turn and a half: less than picking up and equipping on two");
    }

    #[test]
    fn equipping_from_the_ground_takes_only_what_lies_underfoot() {
        let mut r = rig();
        let beside = r.app.world_mut().spawn((Item, Position(r.start.offset(1, 0)), Wearable(EquipShape::in_slot(r.main)))).id();
        let plain = r.app.world_mut().spawn((Item, Position(r.start))).id();
        r.app.update();
        assert!(act(&mut r, EquipFromGround(beside)).is_empty(), "a step away is not underfoot");
        assert!(act(&mut r, EquipFromGround(plain)).is_empty(), "and what cannot be worn is not put on");
        assert_eq!(r.app.world().resource::<Turns>().now(), 0, "refusals cost nothing");
        assert_eq!(r.app.world().get::<Position>(beside).unwrap().0, r.start.offset(1, 0));
    }

    #[test]
    fn stacks_merge_and_impossible_actions_are_refused_for_free() {
        let mut r = rig();
        let coins = r.app.world_mut().spawn((Item, Position(r.start), Stack { key: 7, count: 5 })).id();
        act(&mut r, PickUp);
        let more = r.app.world_mut().spawn((Item, Position(r.start), Stack { key: 7, count: 3 })).id();
        let events = act(&mut r, PickUp);
        assert_eq!(events, vec![ItemEvent::PickedUp { actor: r.player, item: more, merged_into: Some(coins) }]);
        assert_eq!(r.app.world().get::<Stack>(coins).unwrap().count, 8);
        assert!(r.app.world().get_entity(more).is_err(), "the merged stack is gone");
        assert_eq!(r.app.world().get::<Inventory>(r.player).unwrap().items, vec![coins]);

        let before = r.app.world().resource::<Turns>().now();
        assert!(act(&mut r, PickUp).is_empty(), "nothing here");
        assert!(act(&mut r, Equip(coins)).is_empty(), "not wearable");
        assert_eq!(r.app.world().resource::<Turns>().now(), before, "refusals cost nothing");
        assert!(r.app.world().get::<MyTurn>(r.player).is_some());

        assert_eq!(act(&mut r, UseItem(coins)), vec![ItemEvent::Used { actor: r.player, item: coins }]);
        assert_eq!(r.app.world().resource::<Turns>().now(), before + 100, "using is the game's, the turn is ours");
    }

    #[test]
    fn a_despawned_item_leaves_the_bag_and_the_slots() {
        let mut r = rig();
        let sword = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main)))).id();
        act(&mut r, PickUp);
        act(&mut r, Equip(sword));
        r.app.world_mut().entity_mut(sword).despawn();
        r.app.update();
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().items.is_empty());
        assert!(r.app.world().get::<Equipped>(r.player).unwrap().is_free(r.main));
    }

    /// What a ring bestows is on the wearer's stats while it is worn and
    /// gone when it is not; a status's modifier on the same stat is left
    /// alone by both; and a wearer spawned already dressed, the way a
    /// restored run spawns one, has its gear folded on the first pass with
    /// nothing written down for it.
    #[test]
    fn what_worn_gear_bestows_is_folded_while_worn_and_a_status_is_left_alone() {
        use rl_rules::stats::Op;
        use rl_rules::{StatDef, StatusDef};
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin, crate::status::StatusPlugin, ItemsPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        let stats = Registry::from_defs(vec![StatDef::new("might", 10)]).unwrap();
        let might = stats.expect("might");
        let statuses = Registry::from_defs(vec![StatusDef::new("weak").modifies(might, Op::Add(-3))]).unwrap();
        let weak = statuses.expect("weak");
        {
            let mut registries = app.world_mut().resource_mut::<crate::registries::Registries>();
            registries.stats = stats.clone();
            registries.statuses = statuses;
        }
        let finger = rl_rules::SlotId::from_raw(0);
        let ring = app.world_mut().spawn((Item, Position(start), Wearable(EquipShape::in_slot(finger)), Bestows(vec![(might, Op::Add(5))]))).id();
        let player = app
            .world_mut()
            .spawn((
                Actor,
                Player,
                Blocks,
                Position(start),
                Viewshed::new(6),
                RevealsMap,
                crate::combat::Health::full(30),
                crate::combat::Faction(sides.ours),
                Inventory::default(),
                Equipped(Equipment::with_slot_count(1)),
            ))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        let might_of = |app: &App, who: Entity| app.world().get::<StatBlock>(who).map(|s| s.0.value(might, &stats));
        assert_eq!(might_of(&app, player), Some(10), "the plugin gave the player stats, and nothing is on them yet");

        app.world_mut().write_message(Intent::new(player, PickUp));
        app.update();
        app.world_mut().write_message(Intent::new(player, Equip(ring)));
        app.update();
        assert_eq!(might_of(&app, player), Some(15), "the ring's five, folded the pass it went on");
        app.world_mut().write_message(crate::status::Afflict { target: player, status: weak, turns: 9, by: None });
        app.update();
        assert_eq!(might_of(&app, player), Some(12), "and the status's three off");

        app.world_mut().write_message(Intent::new(player, Unequip(ring)));
        app.update();
        assert_eq!(might_of(&app, player), Some(7), "the ring's went with the ring, the status's stayed");
        let sources: Vec<Source> = app.world().get::<StatBlock>(player).unwrap().0.modifiers().iter().map(|m| m.source).collect();
        assert_eq!(sources, vec![Source::Status { status: weak, instance: 0 }]);

        // Dressed as it is spawned: what a save restores.
        let band = app.world_mut().spawn((Item, Bestows(vec![(might, Op::Add(2))]))).id();
        let mut worn = Equipment::with_slot_count(1);
        worn.equip(band, &EquipShape::in_slot(finger)).unwrap();
        let restored = app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(2, 0)),
                crate::combat::Health::full(10),
                crate::combat::Faction(sides.theirs),
                Inventory { items: vec![band] },
                Equipped(worn),
            ))
            .id();
        app.update();
        assert_eq!(might_of(&app, restored), Some(12), "folded on the first pass, from the slots alone");
    }
}
