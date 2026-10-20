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
//! triggers. A use lands on its user where they stand, or, for a thing
//! whose `use` trigger carries a targeting, at a cell it is pointed at with
//! [`UseItemAt`], refused by the rule an ability's aim is, or, for one whose
//! `use` trigger asks for another of its user's things, on the thing chosen
//! with [`UseOn`], refused by [`may_use_on`]. An item never lends an
//! ability: the aimed use is still the thing's own trigger. A thing that
//! can be worn is used only while it is worn.
//!
//! Two markers are the game's to put on and take off. [`Unknown`] keeps a
//! thing's numbers and what it does off every panel and changes nothing it
//! does; [`Stuck`] keeps a worn thing on, refusing whatever would take it
//! off and saying so with [`ItemEvent::WouldNotComeOff`].
//!
//! What wearing an item does is the item's to say and the engine's to
//! apply. Its combat components are read straight off it by
//! [`Loadout`](crate::combat::Loadout) whenever a blow is struck or met,
//! and what it [`Bestows`] on the registered stats is folded into the
//! wearer's [`StatBlock`] by [`fold_gear`] the moment its slots change.
//! Nothing is copied onto the wearer and nothing has to be remembered
//! when it comes off. A status a worn thing's trigger put on its wearer
//! may be held by the thing, and `end_unworn_holds` cures it in the pass
//! the thing is no longer worn.
//!
//! What else a worn thing lends is kept in
//! [`ReactSet::Lend`](crate::plugin::ReactSet::Lend), read every pass off
//! what is worn now: the statuses its [`HeldWhileWorn`] holds on its wearer
//! (`hold_worn_statuses`), the sight in the dark its
//! [`WornDarkSight`] lends, folded with the wearer's own
//! [`OwnDarkSight`] into its [`DarkSight`] unless a game has it
//! [`DarkSightSuppressed`] (`lend_dark_sight`), and a light that shines
//! only while worn, its [`LitWhenWorn`] (`light_worn_things`).
//!
//! Throwing one is [`throwing`](crate::throwing), which needs combat as well.

use bevy::prelude::*;
use rl_core::Point;
use rl_core::turn::BASE_ACTION_COST;
use rl_rules::stats::{Modifier, Op, Source};
use rl_rules::{EquipShape, Equipment, StatId, StatusId};

use crate::ability::{Aimed, Bystanders};
use crate::combat::DeathEvent;
use crate::components::{MyTurn, Player, Position, Viewshed};
use crate::effects::Triggers;
use crate::lighting::{DarkSight, DarkSightSuppressed, LightSource, OwnDarkSight, WornDarkSight};
use crate::minds::{Sight, Thinking};
use crate::places::{MapId, OnMap};
use crate::status::{Afflicted, Cure, StatBlock};
use crate::throwing::Throwable;
use crate::turn::{Action, Intent, Occupancy, Resolution, Turns};
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
/// displace, and a thing whose blow it would strike with in place of its
/// own against its [`OwnBlowScore`] as well; one worth more than all of
/// that together is worth putting on. Gear without a score is never worth
/// changing into; only the comparison matters, so any scale the game likes
/// will do.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct GearScore(pub i32);

/// How much the blow an actor strikes with by itself is worth, in the
/// game's [`GearScore`] units: what a mind gives up when it puts on a thing
/// with a blow and strikes with that instead.
///
/// The engine strikes with the first worn thing that has a blow, and with
/// an actor's own only while it wears none, so a worn blow replaces the
/// actor's own rather than adding to it. A mind weighing a thing whose
/// blow would become the one it strikes with counts this beside what the
/// thing displaces, so an actor with a heavy blow of its own does not
/// trade it for a lighter one. Without it an actor's own blow is worth
/// nothing, which is right for one that barely strikes at all and was the
/// only rule before.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct OwnBlowScore(pub i32);

/// What an item does to its wearer's registered stats while it is worn.
///
/// A ring that sharpens the eye or a coat that hardens the skin is a
/// change to a stat, and this is where the item says so. It is written
/// when the item is spawned, its level already applied, since a level is
/// one thing's own, by the game or by
/// [`insert_parts`](crate::parts::insert_parts); [`fold_gear`] puts every worn item's onto the
/// wearer's [`StatBlock`] tagged with the item, and takes them off again
/// with it. An item whose worth is a blow or armor carries a
/// [`MeleeAttack`](crate::combat::MeleeAttack) or an
/// [`Armor`](crate::combat::Armor) instead, which need no stat at all.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct Bestows(pub Vec<(StatId, Op)>);

/// What an item counts as.
///
/// Abilities read tags to ask whether a shield is on the arm or a powder
/// charge is in the bag, and loot and ammunition draw and spend by them,
/// so they live on the item rather than in a table only the game can
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

/// A thing not yet known: shown by its name and where it is worn, and
/// nothing else, until the game says it is known.
///
/// The bag, the look cursor's panel and the targeting box leave out what
/// its triggers do, its numbers and its charges, and say it is unknown in
/// their place, so identification is a game's choice of name and this
/// marker rather than a redaction every game writes over every panel. It
/// changes nothing the thing does: an unknown draught drunk does what it
/// does, and an unknown blade strikes for what it strikes for. The game's
/// to put on, take off and keep in its own save, since what makes a thing
/// known, a use, a reading, a long wear, is the game's.
///
/// Nor is its use refused, or its aim previewed, by anything only its
/// kind, its reach or its charges would explain, since a refusal for
/// nothing tells as much as a panel: [`may_use`] lets it be used empty,
/// to spend the turn and land nothing; [`UseItemAt`] is refused for no
/// reach, sight or shape of its targeting, and lands by that targeting
/// where its flight stops; and the targeting cursor draws it as a plain
/// line to the cell pointed at and cycles to anyone. What is refused still
/// is what does not depend on the thing: one not carried, or worn to be
/// used and not worn.
///
/// It is the player's not knowing and nobody else's. Anyone else using a
/// thing, a mind using what it carries, knows what it carries, and is
/// aimed and refused by everything the thing truly is, as for a thing
/// known.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Unknown;

/// A thing that will not come off while it is worn.
///
/// Taking it off, dropping it, throwing it and putting on anything that
/// would push it off are each refused, as an impossible item action is, and
/// reported as [`ItemEvent::WouldNotComeOff`] so the log can say why. Only
/// being worn holds it: in the bag it is put on, dropped and thrown as
/// anything else is, and dying lets it fall. What frees it is the game's,
/// whose own effect takes this off, and so is keeping that in its save.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Stuck;

/// A worn thing's own clock: its `pulse` moment comes round every `every`
/// hundredths of a step it is worn, with `progress` counted towards the
/// next.
///
/// What a pulse does is the thing's `pulse` trigger, landed on whoever
/// stands on the wearer's cell, which is the wearer: a plate that knits
/// wounds is a pulse that mends. The clock runs only while the thing is
/// worn and starts again from nothing each time it is put on, so a wearer
/// cannot wear it most of a period, swap, and swap back for a pulse on the
/// next turn. `every` is written when the thing is spawned, with the
/// thing's level already applied, as [`Bestows`] is.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pulse {
    /// Hundredths of a step between pulses.
    pub every: u32,
    /// Hundredths counted towards the next.
    pub progress: u32,
}

impl Pulse {
    /// A clock coming round every `every` hundredths, with nothing counted yet.
    pub fn every(every: u32) -> Self {
        Self { every, progress: 0 }
    }
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
    /// `actor` could not get `item` off, because it is [`Stuck`]: taking
    /// it off, dropping or throwing it, or putting on something that would
    /// have pushed it off, was refused. The refusal's reason, for the log;
    /// the action itself was refused as any impossible one is.
    WouldNotComeOff {
        /// Who.
        actor: Entity,
        /// What stayed on.
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

/// The actor holding the turn, as the item resolver sees it: where it
/// stands, what it carries and wears, and what it sees, for a use it
/// points somewhere.
type Carrier<'w, 's> = Query<'w, 's, (&'static Position, &'static mut Inventory, Option<&'static mut Equipped>, Option<&'static Viewshed>), With<MyTurn>>;

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
    /// What a thing does, for how its use is pointed.
    triggers: Query<'w, 's, &'static Triggers>,
    /// Who stands where, and how they stand to a user: what an aimed use is
    /// refused by, through the call the targeting cursor previews it by.
    occupancy: Res<'w, Occupancy>,
    bystanders: Bystanders<'w, 's>,
    /// What will not come off while it is worn.
    stuck: Query<'w, 's, (), With<Stuck>>,
    /// What a thing counts as, for whether a use may be made on it.
    tagged: Query<'w, 's, &'static Tagged>,
    /// What the player does not know yet, which the player's use is
    /// refused for nothing only what it is would explain.
    unknown: Query<'w, 's, (), With<Unknown>>,
    /// The one user a thing is ever unknown to: anyone else knows what it
    /// carries.
    players: Query<'w, 's, (), With<Player>>,
    /// What a use made on a thing was made on, for its landing.
    fired_on: MessageWriter<'w, crate::effects::FiredOn>,
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
/// It costs `EQUIP_FROM_GROUND_COST`, half as much again as picking up or
/// putting on alone: quicker than doing both, dearer than either, so taking
/// up a sword in the middle of a fight is a real choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquipFromGround(pub Entity);
impl Action for EquipFromGround {}

/// What [`EquipFromGround`] costs, in hundredths of a step.
pub(crate) const EQUIP_FROM_GROUND_COST: u32 = BASE_ACTION_COST * 3 / 2;

/// Take a worn item off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unequip(pub Entity);
impl Action for Unequip {}

/// Use a carried item.
///
/// The engine charges the turn, reports [`ItemEvent::Used`] and the `use`
/// moment, and the item's triggers do the rest. A use of an empty
/// [`Consumable`](crate::consumable::Consumable) is refused, and the
/// player keeps the turn. A thing that can be worn is used only while it
/// is worn, and a use of one in the bag is refused the same way. A thing
/// whose use is aimed is used with [`UseItemAt`], and refused here: used
/// where its user stands it would land somewhere its targeting says it does
/// not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseItem(pub Entity);
impl Action for UseItem {}

/// Use a carried thing whose use is aimed, pointed at a cell: a bolt sent
/// from the pack at a foe.
///
/// [`UseItem`]'s sibling rather than a cell on it, since every thing used
/// where it stands, which is most of them, would otherwise carry a cell it
/// never reads. Refused, and the player keeps the turn, for everything
/// that refuses a [`UseItem`], for a thing whose use is not aimed, and for
/// an aim the use's own [`Targeting`](rl_rules::Targeting) refuses: a bolt
/// with nowhere to fly, a cell out of reach or out of sight. That is
/// [`Bystanders::land`], the call the targeting cursor previews the use by,
/// so a use the cursor shows as legal is one this takes. Reported as the
/// `use` moment at the cell it was pointed at, where its trigger lands.
///
/// A thing the player does not know yet, [`Unknown`], is refused the
/// player for none of what its targeting says, which a refusal would tell:
/// pointed at a cell it wants seen and is not, it is made at the farthest
/// seen cell on the line there, by [`made_at`](rl_rules::ability::made_at),
/// and anywhere else where it was pointed; its trigger then lands by its
/// true targeting, stopping at its reach along the line and catching nobody
/// its aim does not want. Anyone else is refused it as a thing known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseItemAt {
    /// What is used.
    pub item: Entity,
    /// Where it is pointed.
    pub at: Point,
}
impl Action for UseItemAt {}

/// Use a carried thing whose use asks for another of its user's own
/// things, on the one chosen: a polish worked into a coat.
///
/// [`UseItem`]'s sibling, as [`UseItemAt`] is, for the same reason. Refused,
/// and the player keeps the turn, for everything that refuses a
/// [`UseItem`], for a thing whose use asks for nothing, and for a choice
/// [`may_use_on`] refuses, the rule the bag offers its choices by. Reported
/// as the `use` moment where its user stands, beside a
/// [`FiredOn`](crate::effects::FiredOn) naming `on`, which the trigger's
/// landing carries to its effects as [`Landing::on`](crate::effects::Landing::on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseOn {
    /// What is used.
    pub item: Entity,
    /// What it is used on.
    pub on: Entity,
}
impl Action for UseOn {}

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
    UseAt(Entity, Point),
    UseOn(Entity, Entity),
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
    uses_at: MessageReader<'w, 's, Intent<UseItemAt>>,
    uses_on: MessageReader<'w, 's, Intent<UseOn>>,
}

impl ItemIntents<'_, '_> {
    fn drain(&mut self) -> Vec<(Entity, Which)> {
        let mut out: Vec<(Entity, Which)> = self.pick_ups.read().map(|i| (i.actor, Which::PickUp)).collect();
        out.extend(self.drops.read().map(|i| (i.actor, Which::Drop(i.action.0))));
        out.extend(self.equips.read().map(|i| (i.actor, Which::Equip(i.action.0))));
        out.extend(self.from_ground.read().map(|i| (i.actor, Which::EquipFromGround(i.action.0))));
        out.extend(self.unequips.read().map(|i| (i.actor, Which::Unequip(i.action.0))));
        out.extend(self.uses.read().map(|i| (i.actor, Which::Use(i.action.0))));
        out.extend(self.uses_at.read().map(|i| (i.actor, Which::UseAt(i.action.item, i.action.at))));
        out.extend(self.uses_on.read().map(|i| (i.actor, Which::UseOn(i.action.item, i.action.on))));
        out
    }
}

/// Whether `item` may be used out of `bag` by whoever wears `worn`, how its
/// use is pointed aside: carried, worn when it is `wearable`, and with a
/// charge to spend when it is `consumable`, unless it is `unknown` to its
/// user.
///
/// The one rule the items resolver refuses a use by, public so the
/// targeting cursor opens no aim the resolver would refuse before looking
/// at where it was pointed, and the minds are told of nothing it would
/// refuse. An empty thing is still a thing, and using one is a mistake the
/// player keeps the turn for, as for any impossible item action, but only
/// one whose emptiness its user knows: a thing the player does not know
/// yet, [`Unknown`], is used empty, spends the turn and lands nothing,
/// since a use refused for nothing would tell the player it was empty. A
/// mind knows what it carries, so for its use `unknown` is false. A thing
/// that can be worn is used by wearing it, known or not, since where it
/// goes is shown: one used from the bottom of the bag would let a wearer
/// keep one plate on and spend another's charge.
pub fn may_use(
    item: Entity,
    bag: &Inventory,
    worn: Option<&Equipped>,
    consumable: Option<&crate::consumable::Consumable>,
    wearable: bool,
    unknown: bool,
) -> bool {
    let known_empty = !unknown && consumable.is_some_and(|c| c.is_empty());
    let on_the_body = worn.is_some_and(|w| w.slot_of(item).is_some());
    rl_rules::equip::may_use(bag.contains(item), known_empty, wearable, on_the_body)
}

/// Whether `item`'s use, which `asks` for one of its user's things, may be
/// made on `on` out of `bag`, `on` counting as `tagged`: carried, not the
/// thing used, and carrying the tag the use asks for when it asks for one.
///
/// The one rule the items resolver refuses a choice by, public so the bag
/// offers exactly the things the resolver would take; whether `item` may
/// be used at all is [`may_use`]'s. Worn or not, known or not, a thing
/// carried may be chosen: a coat is polished on its wearer's back.
pub fn may_use_on(asks: &rl_rules::Asks, item: Entity, on: Entity, bag: &Inventory, tagged: Option<&Tagged>) -> bool {
    let counts = asks.tag.is_none_or(|tag| tagged.is_some_and(|t| t.contains(&tag.id())));
    on != item && bag.contains(on) && counts
}

/// Resolves pickups, drops, equips, unequips and uses, aimed somewhere,
/// made on a thing or neither, for the actor holding the turn. Each costs
/// one action, and equipping from the ground half as much again. An
/// impossible one, such as picking up from bare ground or a use aimed where
/// it cannot reach, is refused for the player and treated as a wait for
/// anyone else, like an impossible move.
///
/// A worn [`Stuck`] thing is not taken off, dropped, or pushed off by what
/// is put on over it, each refused that way and reported as
/// [`ItemEvent::WouldNotComeOff`]. Putting on a thing that may go in more
/// than one slot takes one whose occupant may come off, by
/// [`Equipment::equip_keeping`].
pub fn resolve_items(
    mut commands: Commands,
    mut intents: ItemIntents,
    mut resolution: Resolution,
    world: ItemWorld,
    mut events: MessageWriter<ItemEvent>,
    mut fired: MessageWriter<crate::effects::Fired>,
) {
    let ItemWorld { mut carriers, ground, stacks, wearables, consumables, map, triggers, occupancy, bystanders, stuck, tagged, unknown, players, mut fired_on } =
        world;
    let this_map = map.current();
    let lies_at = |item: Entity, at: Point| ground.get(item).is_ok_and(|(_, p, on)| p.0 == at && on.map(|m| m.0).unwrap_or(MapId::ZERO) == this_map);
    // Whether `actor` does not know `item`, which only the player ever
    // does: `Unknown` is the player's ignorance, so a mind using what it
    // carries is refused for everything the thing truly is.
    let unknown_to = |actor: Entity, item: Entity| unknown.contains(item) && players.contains(actor);
    let usable = |actor: Entity, item: Entity, bag: &Inventory, worn: Option<&Equipped>| {
        may_use(item, bag, worn, consumables.get(item).ok(), wearables.contains(item), unknown_to(actor, item))
    };
    // How `item`'s use is pointed, when it is pointed somewhere, and what
    // it asks for, when it is made on a thing.
    let targeting_of = |item: Entity| triggers.get(item).ok().and_then(Triggers::targeting);
    let asks_of = |item: Entity| triggers.get(item).ok().and_then(Triggers::asks);
    // Whether a worn thing stays on.
    let stays = |item: Entity| stuck.contains(item);
    for (actor, which) in intents.drain() {
        if !resolution.claim(actor) {
            continue;
        }
        // An actor with no bag to act on has failed like any other
        // impossible item action, rather than holding a claimed turn.
        let ok = 'attempt: {
            match which {
                Which::PickUp => {
                    let Ok((pos, mut bag, _, _)) = carriers.get_mut(actor) else { break 'attempt false };
                    let here: Vec<Entity> =
                        ground.iter().filter(|(_, p, on)| p.0 == pos.0 && on.map(|m| m.0).unwrap_or(MapId::ZERO) == this_map).map(|(e, _, _)| e).collect();
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
                    let Ok((pos, mut bag, worn, _)) = carriers.get_mut(actor) else { break 'attempt false };
                    // Dropped, a worn thing comes off, which a stuck one will not.
                    if stays(item) && worn.as_ref().is_some_and(|w| w.contains(item)) {
                        events.write(ItemEvent::WouldNotComeOff { actor, item });
                        break 'attempt false;
                    }
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
                    let Ok((_, bag, worn, _)) = carriers.get_mut(actor) else { break 'attempt false };
                    match (worn, wearables.get(item)) {
                        (Some(mut worn), Ok(shape)) if bag.contains(item) => match worn.equip_keeping(item, &shape.0, stays) {
                            Ok(displaced) => {
                                for other in displaced {
                                    events.write(ItemEvent::Unequipped { actor, item: other });
                                }
                                events.write(ItemEvent::Equipped { actor, item });
                                true
                            }
                            Err(rl_rules::EquipError::Kept(slot)) => {
                                if let Some(held) = worn.in_slot(slot) {
                                    events.write(ItemEvent::WouldNotComeOff { actor, item: held });
                                }
                                false
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
                    let Ok((pos, mut bag, worn, _)) = carriers.get_mut(actor) else { break 'attempt false };
                    match (worn, wearables.get(item)) {
                        (Some(mut worn), Ok(shape)) if lies_at(item, pos.0) => match worn.equip_keeping(item, &shape.0, stays) {
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
                            Err(rl_rules::EquipError::Kept(slot)) => {
                                if let Some(held) = worn.in_slot(slot) {
                                    events.write(ItemEvent::WouldNotComeOff { actor, item: held });
                                }
                                false
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
                    let Ok((_, _, worn, _)) = carriers.get_mut(actor) else { break 'attempt false };
                    if stays(item) && worn.as_ref().is_some_and(|w| w.contains(item)) {
                        events.write(ItemEvent::WouldNotComeOff { actor, item });
                        break 'attempt false;
                    }
                    let taken_off = worn.is_some_and(|mut worn| worn.unequip(item));
                    if taken_off {
                        events.write(ItemEvent::Unequipped { actor, item });
                    }
                    taken_off
                }
                Which::Use(item) => {
                    let Ok((pos, bag, worn, _)) = carriers.get(actor) else { break 'attempt false };
                    // A thing whose use is pointed somewhere is used at a
                    // cell: used here it would land around its user, which
                    // its targeting says it does not. One that asks for a thing is
                    // used on one, which this does not name.
                    if usable(actor, item, bag, worn) && targeting_of(item).is_none() && asks_of(item).is_none() {
                        events.write(ItemEvent::Used { actor, item });
                        fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::USE, by: Some(actor), at: pos.0 });
                        true
                    } else {
                        false
                    }
                }
                Which::UseAt(item, at) => {
                    let Ok((pos, bag, worn, sight)) = carriers.get(actor) else { break 'attempt false };
                    let Some(targeting) = targeting_of(item) else { break 'attempt false };
                    // Refused as an ability's aim is, before anything is
                    // spent, and by the one call the cursor previews it by:
                    // nowhere to go, out of reach, or out of sight. A thing
                    // the player does not know is refused the player for
                    // none of them, since each would tell what it is: made
                    // where it may still be pointed, it lands by its true
                    // targeting, stopping at its reach, and on nobody its
                    // aim does not want.
                    let source = crate::effects::Source::Trigger { on: item, moment: crate::effects::Moments::USE };
                    let (at, refused) = if unknown_to(actor, item) {
                        (rl_rules::ability::made_at(targeting, pos.0, at, |p| sight.is_none_or(|v| v.can_see(p))), false)
                    } else {
                        let aimed = Aimed { user: actor, source, targeting, origin: pos.0, aim: at, sees_aim: sight.map(|v| v.can_see(at)) };
                        (at, !bystanders.land(aimed, &map, &occupancy).refused.is_empty())
                    };
                    if usable(actor, item, bag, worn) && !refused {
                        events.write(ItemEvent::Used { actor, item });
                        // At the cell it was pointed at, which is where an
                        // aimed trigger lands; it flies from its user.
                        fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::USE, by: Some(actor), at });
                        true
                    } else {
                        false
                    }
                }
                Which::UseOn(item, on) => {
                    let Ok((pos, bag, worn, _)) = carriers.get(actor) else { break 'attempt false };
                    let Some(asks) = asks_of(item) else { break 'attempt false };
                    if usable(actor, item, bag, worn) && may_use_on(asks, item, on, bag, tagged.get(on).ok()) {
                        events.write(ItemEvent::Used { actor, item });
                        // Where its user stands, as any use that is not
                        // aimed, and made on the thing chosen.
                        fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::USE, by: Some(actor), at: pos.0 });
                        fired_on.write(crate::effects::FiredOn { carrier: item, moment: crate::effects::Moments::USE, on });
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

/// Counts each worn [`Pulse`] on by the time that passed since the last
/// pass, and reports its `pulse` moment for each full period, on the thing,
/// by its wearer, at the wearer's cell.
///
/// In [`ResolveSet::Triggers`](crate::plugin::ResolveSet::Triggers) before
/// the triggers land, so a pulse lands in the pass whose clock brought it.
/// Reads the clock rather than counting passes, as a recharge does, and a
/// clock that went backwards, a new run, counts as no time. Only wearers on
/// the current map: a moment lands on whoever stands on its cell here, and
/// a wearer on another map would mend a stranger at the same coordinates.
pub(crate) fn pulse_worn(
    turns: Res<Turns>,
    mut last: Local<Option<u32>>,
    map: Res<WorldMap>,
    wearers: Query<(Entity, &Position, Option<&OnMap>, &Equipped)>,
    mut pulses: Query<&mut Pulse>,
    mut fired: MessageWriter<crate::effects::Fired>,
) {
    let now = turns.now();
    let passed = now.saturating_sub(last.unwrap_or(now));
    *last = Some(now);
    if passed == 0 {
        return;
    }
    let here = map.current();
    for (wearer, pos, on, worn) in &wearers {
        if on.map_or(MapId::ZERO, |m| m.0) != here {
            continue;
        }
        for (_, item) in worn.0.worn() {
            let Ok(mut pulse) = pulses.get_mut(item) else { continue };
            let (times, rest) = rl_rules::charges::periods(pulse.progress, pulse.every, passed);
            pulse.progress = rest;
            for _ in 0..times {
                fired.write(crate::effects::Fired { on: item, moment: crate::effects::Moments::PULSE, by: Some(wearer), at: pos.0 });
            }
        }
    }
}

/// Starts a [`Pulse`] from nothing whenever its thing is put on.
///
/// In [`TurnSet::React`](crate::plugin::TurnSet::React), after the pass
/// counted whatever time it counted, so the thing's first period is a whole
/// one from the pass it went on in.
pub(crate) fn restart_pulses(mut events: MessageReader<ItemEvent>, mut pulses: Query<&mut Pulse>) {
    for ev in events.read() {
        let ItemEvent::Equipped { item, .. } = *ev else { continue };
        if let Ok(mut pulse) = pulses.get_mut(item) {
            pulse.progress = 0;
        }
    }
}

/// Ends every status a worn thing holds on anyone not wearing that very
/// thing, through the ordinary [`Cure`].
///
/// A status is held when the [`Afflict`](crate::status::Afflict) that put
/// it on named a holder, which an `Inflict` written `while_worn` does for
/// the thing whose trigger landed it. Nothing here asks how the thing came
/// off: taken off, dropped, displaced by another in its slot, used up or
/// despawned, the question is only whether the holder still wears it, so
/// a way of losing a thing added later is covered without a line here.
/// A cure takes a status off whole, so a status that stacks loses an
/// unheld instance beside the held one; no game stacks a held status, and
/// a cure that picked one instance is a change to the status model this
/// does not need yet.
///
/// In [`ResolveSet::Effects`](crate::plugin::ResolveSet::Effects) before
/// the statuses resolve, so in the pass the thing comes off, which
/// resolved in [`ResolveSet::Act`](crate::plugin::ResolveSet::Act), the
/// cure lands, [`StatusEvent::Cured`](crate::status::StatusEvent) is
/// written, and a status that hid its holder stops hiding it before the
/// next actor decides. Here rather than in the status plugin because
/// wearing is this plugin's to know; statuses never learn what an item
/// is. A game with items and no statuses has no [`Afflicted`] to look at
/// and the queue it writes stays empty.
pub(crate) fn end_unworn_holds(holders: Query<(Entity, &Afflicted, Option<&Equipped>)>, items: Query<(), With<Item>>, mut cure: MessageWriter<Cure>) {
    for (who, afflicted, worn) in &holders {
        for status in afflicted.iter() {
            let Some(bits) = status.held_by else { continue };
            let item = Entity::try_from_bits(bits);
            let worn = item.is_some_and(|item| items.contains(item) && worn.is_some_and(|w| w.slot_of(item).is_some()));
            if !worn {
                cure.write(Cure { target: who, status: status.id });
            }
        }
    }
}

/// On an item: the statuses it holds on whoever wears it, until cured,
/// for as long as it is worn, which `hold_worn_statuses` puts on and
/// `end_unworn_holds` takes off. Put on by
/// [`insert_parts`](crate::parts::insert_parts) from
/// [`ItemParts::holds`](crate::parts::ItemParts::holds).
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
pub struct HeldWhileWorn(pub Vec<StatusId>);

/// On an item: the light it sheds only while it is worn, which
/// `light_worn_things` puts on it as its [`LightSource`] while it is and
/// takes off when it is not. Put on by
/// [`insert_parts`](crate::parts::insert_parts) for a light that is
/// [`LightWhen::Worn`](crate::parts::LightWhen::Worn).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct LitWhenWorn(pub LightSource);

/// On an actor whose [`DarkSight`] [`lend_dark_sight`] set from what it
/// wears with no [`OwnDarkSight`] of its own: what lets taking the last
/// such thing off take the sight away again, while an actor the fold never
/// touched keeps a `DarkSight` a game set by hand.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LentDarkSight;

/// Each worn thing once, though a two-hander fills two slots.
fn worn_once(equipped: &Equipped) -> Vec<Entity> {
    let mut worn: Vec<Entity> = Vec::new();
    for (_, item) in equipped.0.worn() {
        if !worn.contains(&item) {
            worn.push(item);
        }
    }
    worn
}

/// Puts each worn thing's [`HeldWhileWorn`] statuses on its wearer, held by
/// the thing, [`UNTIL_CURED`](rl_rules::UNTIL_CURED), unless a thing it
/// wears that holds them already does.
///
/// Asked for through [`Afflict`](crate::status::Afflict) every pass rather
/// than when the thing goes on, so a thing worn however it came to be, put
/// on, handed over or put back by a save, is read as it stands; what takes
/// the status off again is [`end_unworn_holds`], the pass the thing is no
/// longer worn. A wearer that holds no statuses at all is lent none.
pub(crate) fn hold_worn_statuses(
    wearers: Query<(Entity, &Equipped, &Afflicted)>,
    holders: Query<&HeldWhileWorn>,
    mut afflict: MessageWriter<crate::status::Afflict>,
) {
    for (wearer, equipped, afflicted) in &wearers {
        let worn: Vec<(Entity, &HeldWhileWorn)> = worn_once(equipped).into_iter().filter_map(|item| Some((item, holders.get(item).ok()?))).collect();
        let held_by_what_it_wears = |by: u64| worn.iter().any(|(holder, _)| holder.to_bits() == by);
        for (item, held) in &worn {
            for status in &held.0 {
                if !afflicted.iter().any(|s| s.id == *status && s.held_by.is_some_and(held_by_what_it_wears)) {
                    afflict.write(crate::status::Afflict { target: wearer, status: *status, turns: rl_rules::UNTIL_CURED, by: None, held_by: Some(*item) });
                }
            }
        }
    }
}

/// An actor [`lend_dark_sight`] keeps: its own sight, what it wears, the
/// sight it reads now, whether a game has suppressed it, and whether the
/// fold lent it what it reads.
type Seer = (Entity, Option<&'static OwnDarkSight>, Option<&'static Equipped>, Option<&'static DarkSight>, Has<DarkSightSuppressed>, Has<LentDarkSight>);
/// Anyone with sight of its own, something worn that might lend some, or
/// sight the fold lent it to take back.
type MaySee = Or<(With<OwnDarkSight>, With<Equipped>, With<LentDarkSight>)>;

/// Keeps every actor's [`DarkSight`] at the best of its [`OwnDarkSight`]
/// and the best [`WornDarkSight`] among what it wears, or none while it is
/// [`DarkSightSuppressed`] or when that comes to nothing.
///
/// Derived every pass from what stands now rather than answered as a thing
/// goes on or comes off, so a thing swapped while the sight was suppressed
/// is read as it stands the pass the suppression ends. An actor with no
/// sight of its own that wears nothing lending any, and was lent none, is
/// never touched, so a game that sets `DarkSight` by hand keeps it.
/// Idempotent: an actor already reading right is left alone, so nothing
/// recasts for it.
///
/// In [`ReactSet::Lend`](crate::plugin::ReactSet::Lend), where a game that
/// suppresses sight in its own reactions orders those before it, so the
/// sight is folded as that pass leaves them.
pub(crate) fn lend_dark_sight(mut commands: Commands, seers: Query<Seer, MaySee>, lending: Query<&WornDarkSight>) {
    for (seer, own, equipped, now, suppressed, lent) in &seers {
        let worn = equipped.and_then(|e| worn_once(e).into_iter().filter_map(|item| lending.get(item).ok()).map(|s| s.0).max());
        if own.is_none() && worn.is_none() && !lent {
            continue;
        }
        let best = rl_rules::equip::dark_sight(own.map(|o| o.0), worn, suppressed);
        let mut e = commands.entity(seer);
        if now.map(|d| d.0) != best {
            match best {
                Some(tiles) => drop(e.insert(DarkSight(tiles))),
                None => drop(e.remove::<DarkSight>()),
            }
        }
        // Marked while what it wears is what it sees by, so the sight goes
        // with the thing; an actor with sight of its own needs no mark, its
        // own is always what is left.
        let lends = own.is_none() && worn.is_some();
        if lends != lent {
            if lends {
                e.insert(LentDarkSight);
            } else {
                e.remove::<LentDarkSight>();
            }
        }
    }
}

/// Lights every [`LitWhenWorn`] thing while someone wears it, its light its
/// [`LightSource`] shed from its wearer, and puts it out when nobody does.
///
/// A component put on and taken off rather than a rule in the cast, so the
/// light is recast the way any light lit or put out is, and what lights a
/// carried thing needs no second case. Leaves alone what already reads
/// right.
pub(crate) fn light_worn_things(mut commands: Commands, wearers: Query<&Equipped>, things: Query<(Entity, &LitWhenWorn, Option<&LightSource>)>) {
    let mut worn: Vec<Entity> = Vec::new();
    for equipped in &wearers {
        worn.extend(equipped.0.worn().map(|(_, item)| item));
    }
    for (thing, lit, now) in &things {
        match (worn.contains(&thing), now) {
            (true, Some(now)) if *now == lit.0 => {}
            (true, _) => drop(commands.entity(thing).insert(lit.0)),
            (false, Some(_)) => drop(commands.entity(thing).remove::<LightSource>()),
            (false, None) => {}
        }
    }
}

/// Drops despawned items from every bag and every slot.
pub(crate) fn forget_removed_items(mut removed: RemovedComponents<Item>, mut carriers: Query<(&mut Inventory, Option<&mut Equipped>)>) {
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
pub(crate) fn drop_what_the_dead_carried(
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
    blows: Query<'w, 's, (), (With<Item>, With<crate::combat::MeleeAttack>)>,
    own: Query<'w, 's, &'static OwnBlowScore>,
    /// What will not come off, which no gain pushes off.
    stuck: Query<'w, 's, (), With<Stuck>>,
    /// What a carried thing does when used, how it is pointed and whether
    /// it asks for another thing, and what its use spends.
    uses: Query<'w, 's, &'static Triggers>,
    charges: Query<'w, 's, &'static crate::consumable::Consumable>,
    wearables: Query<'w, 's, (), With<Wearable>>,
}

impl Belongings<'_, '_> {
    /// What `thinker` carries that it could throw.
    fn missiles(&self, thinker: Entity) -> Vec<rl_rules::Missile<Entity>> {
        let Ok((Some(bag), _)) = self.bags.get(thinker) else { return Vec::new() };
        bag.items.iter().filter_map(|item| self.missiles.get(*item).ok().map(|t| rl_rules::Missile { item: *item, range: t.range })).collect()
    }

    /// What `thinker` carries that it could use, by [`may_use`], the rule
    /// the resolver refuses a use by, asked as one that knows what it
    /// carries: a thing the player has not learnt is empty is empty to the
    /// mind that carries it. A thing with no `use` trigger is the game's to
    /// answer and one whose use asks for another thing has nothing to
    /// choose that thing for a mind, so neither is told of.
    fn carried(&self, thinker: Entity) -> Vec<rl_rules::Carried<Entity>> {
        let Ok((Some(bag), worn)) = self.bags.get(thinker) else { return Vec::new() };
        let usable = |item: Entity| may_use(item, bag, worn, self.charges.get(item).ok(), self.wearables.contains(item), false);
        let told = |item: Entity| {
            let does = self.uses.get(item).ok()?;
            let uses = || does.on(crate::effects::Moments::USE);
            (uses().next().is_some() && does.asks().is_none() && usable(item)).then(|| rl_rules::Carried {
                item,
                targeting: does.targeting(),
                mends: uses().any(|t| t.effects.mends()),
            })
        };
        bag.items.iter().filter_map(|item| told(*item)).collect()
    }

    /// How much better `thinker` would be for wearing `item` in `shape`,
    /// scored `score`, than for what it would displace and, for a thing
    /// whose blow it would strike with where it strikes with its own now,
    /// than for its [`OwnBlowScore`]; `None` when it wears nothing, the
    /// item is not scored, or putting it on would push off a [`Stuck`]
    /// thing, which the resolver would refuse.
    fn gain(&self, thinker: Entity, item: Entity, shape: &Wearable, score: &GearScore) -> Option<i32> {
        let Ok((_, Some(worn))) = self.bags.get(thinker) else { return None };
        let own_blow = self.own.get(thinker).map_or(0, |o| o.0);
        rl_rules::equip::gain(
            &worn.0,
            item,
            &shape.0,
            |other| self.stuck.contains(other),
            score.0,
            |other| (self.scores.get(other).map_or(0, |s| s.0), self.blows.contains(other)),
            own_blow,
        )
    }
}

/// Tells the mind holding the turn what it carries to throw and to use,
/// and what lies in sight worth having.
///
/// Items' contribution to a mind's knowledge, in
/// [`PerceiveSet::Annotate`](crate::plugin::PerceiveSet::Annotate). Only a
/// mind with the wits to pick up or put on is told what lies about; what
/// it stands on it can feel, and anything else it has to see. Only one
/// with the wits to use things is told what it could use. How far its
/// shot reaches is combat's to tell, in
/// [`perceive_reach`](crate::combat::perceive_reach), since a gun a mind
/// was built with needs no items at all.
pub fn perceive_belongings(mut thinking: ResMut<Thinking>, sight: Sight, belongings: Belongings) {
    let Some(thinker) = thinking.actor() else { return };
    let missiles = belongings.missiles(thinker);
    let wits = thinking.snapshot().map(|s| s.wits).unwrap_or_default();
    let carried = if wits.has(rl_rules::Wits::USES) { belongings.carried(thinker) } else { Vec::new() };
    let mut items = Vec::new();
    if wits.has(rl_rules::Wits::PICKS_UP) || wits.has(rl_rules::Wits::EQUIPS) {
        for (item, pos, on, throwable, wearable, score) in belongings.lying.iter() {
            let underfoot = pos.0 == thinking.at() && on.map(|m| m.0).unwrap_or(MapId::ZERO) == sight.current_map();
            if !underfoot && !sight.perceives(&thinking, pos.0, on) {
                continue;
            }
            let gain = wearable.zip(score).and_then(|(shape, score)| belongings.gain(thinker, item, shape, score));
            items.push(rl_rules::ItemView { id: item, pos: pos.0, throw_range: throwable.map(|t| t.range), gain });
        }
    }
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.missiles = missiles;
        snapshot.carried = carried;
        snapshot.items.extend(items);
    }
}

/// Items: on the ground, in a bag, in a slot, the actions that move them
/// between the three and use them, and the fold of what worn items bestow.
///
/// What worn gear lends lands on the [`StatBlock`] every
/// [`Actor`](crate::components::Actor) is given as it is spawned, by
/// [`CorePlugin`](crate::plugin::CorePlugin)'s statuses.
///
/// A worn [`Pulse`] reports its `pulse` moment on its own clock, and a
/// status a worn thing holds ends when it is no longer worn, in
/// `end_unworn_holds`. What a use costs the thing used, its
/// [`Consumable`](crate::consumable::Consumable) charges, is part of it
/// too: see [`consumable`](crate::consumable).
pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{CleanupSet, ResolveSet, Turn, TurnSet};
        use crate::turn::AddAction;
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
            .add_action::<UseItemAt>()
            .add_action::<UseOn>()
            // What a use reports, for the triggers it sets off. Registered
            // here as well as by the effects plugin, since a game may have
            // items and no effects, and then the queue simply stays empty.
            .add_message::<crate::effects::Fired>()
            .add_message::<crate::effects::FiredOn>()
            .add_systems(Turn, perceive_belongings.in_set(crate::plugin::PerceiveSet::Annotate))
            .add_systems(Turn, resolve_items.in_set(ResolveSet::Act))
            // Before the triggers land, so a pulse lands in the pass that
            // brought it; a game with no effects has nothing to land it.
            .add_systems(Turn, pulse_worn.in_set(ResolveSet::Triggers).before(crate::effects::land_triggers))
            // What ending a held status writes, registered here as well as
            // by the status plugin, since a game may have items and no
            // statuses, and then nobody is afflicted and nothing is written.
            .add_message::<Cure>()
            // Before the statuses resolve, so a held status ends in the pass
            // its thing came off; see `end_unworn_holds`.
            .add_systems(Turn, end_unworn_holds.in_set(ResolveSet::Effects).before(crate::status::resolve_afflictions))
            // In the pass the slots changed in, so gear counts from the
            // moment it is worn.
            .add_systems(Turn, fold_gear.in_set(crate::plugin::ReactSet::Settle))
            // What else a worn thing lends, as what is worn stands once the
            // pass's slots have settled.
            .add_systems(Turn, (hold_worn_statuses, lend_dark_sight, light_worn_things).chain().in_set(crate::plugin::ReactSet::Lend))
            .add_systems(Turn, restart_pulses.in_set(TurnSet::React))
            .add_systems(Turn, drop_what_the_dead_carried.in_set(CleanupSet::Remove))
            .add_systems(Turn, forget_removed_items.in_set(CleanupSet::Requeue));
        crate::consumable::build(app);
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
        app.add_plugins((crate::fov::FovPlugin, ItemsPlugin));
        let start = crate::testing::ground(&mut app);
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

    /// A thing that is worn is used by wearing it: in the bag its use is
    /// refused and costs nothing, and once it is on the use goes through.
    #[test]
    fn a_wearable_thing_is_used_only_while_it_is_worn() {
        let mut r = rig();
        let plate = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main)))).id();
        act(&mut r, PickUp);
        let before = r.app.world().resource::<Turns>().now();
        assert!(act(&mut r, UseItem(plate)).is_empty(), "in the bag, it is not used");
        assert_eq!(r.app.world().resource::<Turns>().now(), before, "and the refusal is free");
        act(&mut r, Equip(plate));
        assert_eq!(act(&mut r, UseItem(plate)), vec![ItemEvent::Used { actor: r.player, item: plate }], "worn, it is");
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
        app.add_plugins((crate::fov::FovPlugin, crate::combat::CombatPlugin, ItemsPlugin));
        let start = crate::testing::ground(&mut app);
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
        app.world_mut().write_message(crate::status::Afflict { target: player, status: weak, turns: 9, by: None, held_by: None });
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

    /// A stuck thing stays on: taking it off and dropping it are each
    /// refused for nothing and said, and once the game frees it, it comes
    /// off as anything does.
    #[test]
    fn a_stuck_thing_is_neither_taken_off_nor_dropped_and_says_why() {
        let mut r = rig();
        let ring = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main)), Stuck)).id();
        act(&mut r, PickUp);
        act(&mut r, Equip(ring));
        let before = r.app.world().resource::<Turns>().now();
        let held = vec![ItemEvent::WouldNotComeOff { actor: r.player, item: ring }];
        assert_eq!(act(&mut r, Unequip(ring)), held, "taking it off is refused, and says why");
        assert_eq!(act(&mut r, DropItem(ring)), held, "and so is dropping it");
        assert_eq!(r.app.world().get::<Equipped>(r.player).unwrap().in_slot(r.main), Some(ring), "still worn");
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().contains(ring), "and carried");
        assert_eq!(r.app.world().resource::<Turns>().now(), before, "for nothing");
        r.app.world_mut().entity_mut(ring).remove::<Stuck>();
        assert_eq!(act(&mut r, Unequip(ring)), vec![ItemEvent::Unequipped { actor: r.player, item: ring }], "freed, it comes off");
    }

    /// Nothing put on over a stuck thing pushes it off: another shield and
    /// a two-hander are each refused the hand a stuck shield holds, the
    /// two-hander out of the bag and off the ground alike, and a thing that
    /// goes in either hand takes the one whose occupant may come off.
    #[test]
    fn nothing_put_on_over_a_stuck_thing_pushes_it_off() {
        let mut r = rig();
        let shield = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.off)), Stuck)).id();
        let pike = r.app.world_mut().spawn((Item, Position(r.start), Wearable(EquipShape::in_slot(r.main).and_claims(r.off)))).id();
        act(&mut r, PickUp);
        act(&mut r, Equip(shield));
        let before = r.app.world().resource::<Turns>().now();
        let held = vec![ItemEvent::WouldNotComeOff { actor: r.player, item: shield }];
        let buckler = r.app.world_mut().spawn((Item, Wearable(EquipShape::in_slot(r.off)))).id();
        r.app.world_mut().get_mut::<Inventory>(r.player).unwrap().items.push(buckler);
        assert_eq!(act(&mut r, Equip(buckler)), held, "another shield would take its hand");
        assert_eq!(act(&mut r, Equip(pike)), held, "and so would the two-hander");
        act(&mut r, DropItem(pike));
        let before_ground = r.app.world().resource::<Turns>().now();
        assert_eq!(act(&mut r, EquipFromGround(pike)), held, "and off the ground");
        assert_eq!(r.app.world().get::<Position>(pike).map(|p| p.0), Some(r.start), "where it is left lying");
        assert_eq!(r.app.world().resource::<Turns>().now(), before_ground, "each refused for nothing");
        assert!(before < before_ground, "the drop between them was a turn");
        let worn = r.app.world().get::<Equipped>(r.player).unwrap();
        assert_eq!((worn.in_slot(r.main), worn.in_slot(r.off)), (None, Some(shield)), "the shield is where it was");

        let sword = r.app.world_mut().spawn((Item, Wearable(EquipShape::in_slot(r.main)))).id();
        let knife = r.app.world_mut().spawn((Item, Wearable(EquipShape::in_any([r.off, r.main])))).id();
        r.app.world_mut().get_mut::<Inventory>(r.player).unwrap().items.extend([sword, knife]);
        act(&mut r, Equip(sword));
        let swapped = vec![ItemEvent::Unequipped { actor: r.player, item: sword }, ItemEvent::Equipped { actor: r.player, item: knife }];
        assert_eq!(act(&mut r, Equip(knife)), swapped, "the knife takes the sword's hand, not the shield's");
    }

    /// Waits one turn for the rig's player, so a pass runs.
    fn wait(r: &mut Rig) {
        act(r, crate::turn::Wait);
    }

    fn sight(r: &Rig, who: Entity) -> Option<i32> {
        r.app.world().get::<DarkSight>(who).map(|d| d.0)
    }

    /// A thing in the rig's player's bag, worn in the main hand, with `extra`.
    fn carried(r: &mut Rig, extra: impl Bundle) -> Entity {
        let main = r.main;
        let thing = r.app.world_mut().spawn((Item, Wearable(EquipShape::in_slot(main)), extra)).id();
        r.app.world_mut().get_mut::<Inventory>(r.player).unwrap().items.push(thing);
        thing
    }

    /// A wearer sees in the dark by the best of its own sight and what it
    /// wears, by nothing while a game suppresses it, and by its own again
    /// once the thing comes off.
    #[test]
    fn a_wearer_sees_in_the_dark_by_the_best_of_its_own_and_what_it_wears_and_by_nothing_while_suppressed() {
        let mut r = rig();
        r.app.world_mut().entity_mut(r.player).insert((OwnDarkSight(2), DarkSight(2)));
        let goggles = carried(&mut r, WornDarkSight(5));
        wait(&mut r);
        assert_eq!(sight(&r, r.player), Some(2), "carried, its own");
        act(&mut r, Equip(goggles));
        assert_eq!(sight(&r, r.player), Some(5), "worn, the goggles' in the pass they go on");
        r.app.world_mut().entity_mut(r.player).insert(DarkSightSuppressed);
        wait(&mut r);
        assert_eq!(sight(&r, r.player), None, "suppressed, none at all");
        r.app.world_mut().entity_mut(r.player).remove::<DarkSightSuppressed>();
        wait(&mut r);
        assert_eq!(sight(&r, r.player), Some(5), "and the goggles' again");
        act(&mut r, Unequip(goggles));
        assert_eq!(sight(&r, r.player), Some(2), "taken off, its own");
    }

    /// A wearer with no sight of its own loses what it wore with the thing,
    /// and an actor the fold never lent anything keeps a sight set by hand.
    #[test]
    fn sight_lent_by_a_worn_thing_goes_with_it_and_a_sight_set_by_hand_is_left_alone() {
        let mut r = rig();
        let rat = r.app.world_mut().spawn((Actor, Blocks, Position(r.start.offset(3, 0)), DarkSight(9))).id();
        let goggles = carried(&mut r, WornDarkSight(5));
        act(&mut r, Equip(goggles));
        assert_eq!(sight(&r, r.player), Some(5));
        act(&mut r, Unequip(goggles));
        assert_eq!(sight(&r, r.player), None, "nothing of its own to fall back on");
        wait(&mut r);
        assert_eq!(sight(&r, rat), Some(9), "set by hand and never lent, it is kept");
    }

    /// A worn thing holds its statuses on its wearer, until cured, for as
    /// long as it is worn, and the pass it comes off they end.
    #[test]
    fn a_worn_thing_holds_its_statuses_on_its_wearer_until_it_comes_off() {
        let mut r = rig();
        let statuses = Registry::from_defs(vec![rl_rules::StatusDef::new("warded")]).unwrap();
        let warded = statuses.expect("warded");
        r.app.insert_resource(crate::registries::Registries { statuses, ..Default::default() });
        let charm = carried(&mut r, HeldWhileWorn(vec![warded]));
        let held = |r: &Rig| r.app.world().get::<Afflicted>(r.player).unwrap().iter().find(|s| s.id == warded).map(|s| (s.turns, s.held_by));
        wait(&mut r);
        assert_eq!(held(&r), None, "carried, it holds nothing");
        act(&mut r, Equip(charm));
        wait(&mut r);
        assert_eq!(held(&r), Some((rl_rules::UNTIL_CURED, Some(charm.to_bits()))), "worn, held by the charm until cured");
        wait(&mut r);
        assert_eq!(r.app.world().get::<Afflicted>(r.player).unwrap().iter().filter(|s| s.id == warded).count(), 1, "and asked for once");
        act(&mut r, Unequip(charm));
        wait(&mut r);
        assert_eq!(held(&r), None, "taken off, it ended");
    }

    /// A light lit only while worn is dark in the bag, lit on the wearer,
    /// and dark again once taken off.
    #[test]
    fn a_light_lit_while_worn_shines_only_while_it_is_worn() {
        let mut r = rig();
        let glow = crate::lighting::LightSource::new(200, 4, rl_grid::Rgb::WHITE);
        let lamp = carried(&mut r, LitWhenWorn(glow));
        wait(&mut r);
        assert_eq!(r.app.world().get::<LightSource>(lamp), None, "dark in the bag");
        act(&mut r, Equip(lamp));
        assert_eq!(r.app.world().get::<LightSource>(lamp), Some(&glow), "lit, worn");
        act(&mut r, Unequip(lamp));
        assert_eq!(r.app.world().get::<LightSource>(lamp), None, "dark again");
    }
}

/// A thing used at somewhere: the resolver's refusals and where the use
/// lands, through the engine's own intents.
#[cfg(test)]
mod aimed {
    use super::*;
    use crate::combat::{CombatPlugin, Faction, Health};
    use crate::components::{Actor, Blocks, Player, RevealsMap, Viewshed};
    use crate::consumable::{Consumable, WhenEmpty};
    use crate::cue::{Anchor, Cue, Cued, LookOf, TurnHold};
    use crate::effects::{AddEngineEffects, EffectBonus, EffectKinds, Moments, Triggers};
    use crate::plugin::headless_app;
    use crate::registries::Registries;
    use crate::state::EngineState;
    use crate::testing::Sides;

    /// A rod that throws a bolt of four reaching six, at a foe.
    const BOLT: &str = r#"[(on: "use", targeting: (mode: Bolt(range: 6)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "4"))])]"#;

    /// A player holding the first turn, with combat, the engine's effects
    /// and charges, and two sides at war.
    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        sides: Sides,
    }

    impl Rig {
        fn new() -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, ItemsPlugin, crate::effects::EffectsPlugin));
            app.add_engine_effects();
            let start = crate::testing::ground(&mut app);
            let sides = crate::testing::two_sides(&mut app);
            let player = app
                .world_mut()
                .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap, Health::full(30), Faction(sides.ours), Inventory::default()))
                .id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds the turn a use needs");
            Self { app, player, start, sides }
        }

        /// A thing in the player's bag doing `triggers`, with three charges
        /// kept when spent.
        fn carry(&mut self, triggers: &str) -> Entity {
            let specs: Vec<rl_rules::TriggerSpec> = rl_rules::Names::new().load_list(triggers).expect("the triggers parse");
            let built = {
                let world = self.app.world();
                let names = world.resource::<Registries>().names();
                Triggers::build(&specs, &[], world.resource::<Moments>(), world.resource::<EffectKinds>(), &names).expect("the triggers build")
            };
            let thing = self.app.world_mut().spawn((Item, Name::new("rod"), built, Consumable::new(3, WhenEmpty::Kept))).id();
            self.app.world_mut().get_mut::<Inventory>(self.player).expect("a bag").items.push(thing);
            thing
        }

        /// Someone standing `dx, dy` off the player, on `side`.
        fn stand(&mut self, dx: i32, dy: i32, side: rl_rules::FactionId) -> Entity {
            let at = self.start.offset(dx, dy);
            let e = self.app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(20), Faction(side))).id();
            self.app.update();
            e
        }

        fn foe(&mut self, dx: i32, dy: i32) -> Entity {
            let theirs = self.sides.theirs;
            self.stand(dx, dy, theirs)
        }

        /// Writes `action` for the player and runs the frame that resolves
        /// it, returning what the items resolver reported.
        fn act<A: Action>(&mut self, action: A) -> Vec<ItemEvent> {
            let player = self.player;
            self.app.world_mut().write_message(Intent::new(player, action));
            self.app.update();
            self.app.world_mut().resource_mut::<Messages<ItemEvent>>().drain().collect()
        }

        fn hp(&self, who: Entity) -> i32 {
            self.app.world().get::<Health>(who).expect("health").current
        }

        fn charges(&self, thing: Entity) -> u16 {
            self.app.world().get::<Consumable>(thing).expect("charges").left
        }

        fn now(&self) -> u32 {
            self.app.world().resource::<Turns>().now()
        }

        fn cues(&mut self) -> Vec<Cue> {
            self.app.world_mut().resource_mut::<Messages<Cued>>().drain().map(|c| c.cue).collect()
        }
    }

    /// The whole of it: the use is pointed at a foe three cells off, lands
    /// its bolt there and not on whoever used it, spends a charge and the
    /// turn, and is reported as a use like any other.
    #[test]
    fn an_aimed_use_lands_on_the_footprint_where_it_was_pointed_and_not_on_its_user() {
        let mut rig = Rig::new();
        let rod = rig.carry(BOLT);
        let foe = rig.foe(3, 0);
        let at = rig.start.offset(3, 0);
        let events = rig.act(UseItemAt { item: rod, at });
        assert_eq!(events, vec![ItemEvent::Used { actor: rig.player, item: rod }], "a use like any other");
        assert_eq!(rig.hp(foe), 16, "the bolt landed where it was pointed");
        assert_eq!(rig.hp(rig.player), 30, "and not on its user");
        assert_eq!(rig.charges(rod), 2, "for one charge");
        assert_eq!(rig.now(), 100, "and the turn");
    }

    /// Who a use's footprint catches is the ability rule: a burst on the
    /// ground takes no sides, and a bolt aimed at foes passes an ally by.
    #[test]
    fn the_aim_of_a_use_decides_who_its_footprint_catches_as_an_abilitys_does() {
        let mut rig = Rig::new();
        let burst = rig.carry(
            r#"[(on: "use", targeting: (mode: Ball(range: 6, radius: 1), aim: Ground), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))])]"#,
        );
        let bolt = rig.carry(BOLT);
        let foe = rig.foe(4, 0);
        let ours = rig.sides.ours;
        let ally = rig.stand(4, 1, ours);
        rig.act(UseItemAt { item: burst, at: rig.start.offset(4, 0) });
        assert_eq!((rig.hp(foe), rig.hp(ally)), (17, 17), "the ground takes no sides");
        rig.act(UseItemAt { item: bolt, at: rig.start.offset(4, 1) });
        assert_eq!(rig.hp(ally), 17, "a bolt aimed at foes passes an ally by");
        assert_eq!(rig.charges(bolt), 2, "though it was used");
    }

    /// What a thing's level adds to what it lands, it adds to a use aimed
    /// somewhere as to one that lands on its user.
    #[test]
    fn an_aimed_use_lands_with_its_things_effect_bonus() {
        let mut rig = Rig::new();
        let rod = rig.carry(BOLT);
        rig.app.world_mut().entity_mut(rod).insert(EffectBonus { turns: 0, amount: 2 });
        let foe = rig.foe(3, 0);
        rig.act(UseItemAt { item: rod, at: rig.start.offset(3, 0) });
        assert_eq!(rig.hp(foe), 14, "four and the bonus's two");
    }

    /// An aim its own targeting refuses is refused before anything is
    /// spent: a bolt pointed at its user's feet has nowhere to fly, and one
    /// pointed past its reach would land somewhere nobody chose.
    #[test]
    fn an_aim_the_use_would_refuse_costs_the_player_nothing() {
        let mut rig = Rig::new();
        let rod = rig.carry(BOLT);
        let far = rig.foe(8, 0);
        assert!(rig.act(UseItemAt { item: rod, at: rig.start }).is_empty(), "nowhere to fly");
        assert!(rig.act(UseItemAt { item: rod, at: rig.start.offset(8, 0) }).is_empty(), "out of reach");
        assert_eq!(rig.hp(far), 20);
        assert_eq!((rig.charges(rod), rig.now()), (3, 0), "nothing spent, no time passed");
        assert!(rig.app.world().get::<MyTurn>(rig.player).is_some(), "and the turn is still the player's");
    }

    /// A thing is used the way its use is written: one aimed somewhere is
    /// not used without a cell, and one that lands on its user is not
    /// pointed anywhere, each refused for nothing; and the second, used as
    /// it always was, lands on its user where they stand.
    #[test]
    fn a_use_is_refused_unless_it_is_made_the_way_the_thing_is_used() {
        let mut rig = Rig::new();
        let rod = rig.carry(BOLT);
        let draught = rig.carry(r#"[(on: "use", effects: [(kind: "Mend", args: (roll: "5"))])]"#);
        rig.app.world_mut().get_mut::<Health>(rig.player).expect("health").current = 20;
        let foe = rig.foe(1, 0);
        assert!(rig.act(UseItem(rod)).is_empty(), "an aimed thing used with no cell");
        assert!(rig.act(UseItemAt { item: draught, at: rig.start.offset(1, 0) }).is_empty(), "a draught pointed at a foe");
        assert_eq!((rig.charges(rod), rig.charges(draught), rig.now()), (3, 3, 0), "both refused for nothing");
        assert_eq!(rig.act(UseItem(draught)), vec![ItemEvent::Used { actor: rig.player, item: draught }]);
        assert_eq!((rig.hp(rig.player), rig.hp(foe)), (25, 20), "an unaimed use lands on its user, as it always did");
    }

    /// With something watching, a use aimed somewhere is in the air until
    /// its flight, in the trigger's own look, has been seen: the charge and
    /// the turn are spent on the use, nothing is hurt and nobody is dealt a
    /// turn until the hold lets go, and then it lands, bursts in the same
    /// look, and is waited on again before the turn comes round.
    #[test]
    fn watched_an_aimed_use_flies_in_its_triggers_look_and_lands_once_the_flight_is_seen() {
        let mut rig = Rig::new();
        let rod = rig.carry(
            r#"[(on: "use", targeting: (mode: Bolt(range: 6)), look: (glyph: '*', color: (r: 255, g: 80, b: 0)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "4"))])]"#,
        );
        let look = rl_rules::ability::Look { glyph: '*', color: rl_grid::Rgb::new(255, 80, 0) };
        let foe = rig.foe(3, 0);
        let at = rig.start.offset(3, 0);
        rig.cues();
        rig.app.world_mut().resource_mut::<TurnHold>().watch();

        rig.act(UseItemAt { item: rod, at });
        assert_eq!(rig.hp(foe), 20, "in the air");
        let flight = Cue::Flight { from: Anchor::on(rig.player, rig.start), to: Anchor::on(foe, at), look: LookOf::Given(look) };
        assert_eq!(rig.cues(), vec![flight], "the flight, in the trigger's look, and nothing else yet");
        let hold = *rig.app.world().resource::<TurnHold>();
        assert!(hold.is_held() && hold.in_flight(), "held for the flight, with one in the air");
        assert_eq!(rig.charges(rod), 2, "spent on the use");
        rig.app.update();
        assert_eq!(rig.hp(foe), 20, "still, while held");

        rig.app.world_mut().resource_mut::<TurnHold>().release();
        rig.app.update();
        assert_eq!(rig.hp(foe), 16, "seen to fly, it lands");
        let burst = rig.cues();
        let [Cue::Burst { from, look: shown, .. }] = burst.as_slice() else { panic!("and bursts where it did: {burst:?}") };
        assert_eq!((*from, *shown), (Some(Anchor::on(foe, at)), LookOf::Given(look)), "on whoever it landed on, in the same look");
        let hold = *rig.app.world().resource::<TurnHold>();
        assert!(hold.is_held() && !hold.in_flight(), "held again for the burst, nothing in the air");
        assert!(rig.app.world().get::<MyTurn>(rig.player).is_none(), "no turn dealt while the burst is seen");

        rig.app.world_mut().resource_mut::<TurnHold>().release();
        rig.app.update();
        assert!(rig.app.world().get::<MyTurn>(rig.player).is_some(), "then the turn comes round");
    }

    /// Watched, a use a reflecting body turns back is seen to fly to that
    /// body and back to its user, as a reflected ability is, since both are
    /// seen by the one landing; one that is not turned back flies once.
    /// Over a span of rolls, so both outcomes are seen.
    #[test]
    fn watched_an_aimed_use_turned_back_is_seen_to_fly_to_the_body_and_back() {
        use rand::SeedableRng;
        let (mut reflected, mut landed) = (0, 0);
        for seed in 0..20 {
            let mut rig = Rig::new();
            rig.app.world_mut().resource_mut::<crate::ability::ReflectRng>().0 = rand::rngs::StdRng::seed_from_u64(seed);
            let rod = rig.carry(
                r#"[(on: "use", targeting: (mode: Bolt(range: 6)), look: (glyph: '*', color: (r: 255, g: 80, b: 0)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "4"))])]"#,
            );
            let look = LookOf::Given(rl_rules::ability::Look { glyph: '*', color: rl_grid::Rgb::new(255, 80, 0) });
            let at = rig.start.offset(3, 0);
            let foe = rig.foe(3, 0);
            rig.app.world_mut().entity_mut(foe).insert(crate::ability::Reflects);
            rig.cues();
            rig.app.world_mut().resource_mut::<TurnHold>().watch();

            rig.act(UseItemAt { item: rod, at });
            let (user, body) = (Anchor::on(rig.player, rig.start), Anchor::on(foe, at));
            let out = Cue::Flight { from: user, to: body, look };
            let turned = !rig.app.world_mut().resource_mut::<Messages<crate::ability::Reflected>>().drain().collect::<Vec<_>>().is_empty();
            if turned {
                reflected += 1;
                assert_eq!(rig.cues(), vec![out, Cue::Flight { from: body, to: user, look }], "seed {seed}: out to the body, and back");
            } else {
                landed += 1;
                assert_eq!(rig.cues(), vec![out], "seed {seed}: once, to the body it lands on");
            }
        }
        assert!(reflected > 0 && landed > 0, "both outcomes over twenty seeds: {reflected} reflected, {landed} landed");
    }

    /// A trigger with no look shows nothing, aimed or not, so a use with no
    /// look is not waited on and lands as it is made.
    #[test]
    fn an_aimed_use_with_no_look_is_not_waited_on() {
        let mut rig = Rig::new();
        let rod = rig.carry(BOLT);
        let foe = rig.foe(3, 0);
        rig.app.world_mut().resource_mut::<TurnHold>().watch();
        rig.act(UseItemAt { item: rod, at: rig.start.offset(3, 0) });
        assert_eq!(rig.hp(foe), 16, "landed as it was made");
        assert!(!rig.app.world().resource::<TurnHold>().in_flight(), "with nothing in the air");
    }

    /// A burst on the ground reaching six, landing three on all round it.
    const LOB: &str =
        r#"[(on: "use", targeting: (mode: Ball(range: 6, radius: 1), aim: Ground), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "3"))])]"#;

    /// A thing not yet known pointed past its reach is not refused, which
    /// would tell its reach: it is used and spent, and lands where its own
    /// flight stops, at its reach along the line it was pointed down.
    #[test]
    fn an_unknown_thing_pointed_past_its_reach_is_used_and_lands_at_its_reach() {
        let mut rig = Rig::new();
        let lob = rig.carry(LOB);
        rig.app.world_mut().entity_mut(lob).insert(Unknown);
        let (near, pointed) = (rig.foe(7, 0), rig.foe(9, 0));
        let events = rig.act(UseItemAt { item: lob, at: rig.start.offset(9, 0) });
        assert_eq!(events, vec![ItemEvent::Used { actor: rig.player, item: lob }], "used, not refused");
        assert_eq!((rig.hp(near), rig.hp(pointed)), (17, 20), "burst at its reach of six, beside the near one, short of the one pointed at");
        assert_eq!((rig.charges(lob), rig.now()), (2, 100), "for a charge and the turn");
    }

    /// A thing not yet known whose use wants a foe, pointed at a friend or
    /// at bare ground, is used and spent and lands on nobody, as its aim
    /// says, rather than being refused for what its aim would tell.
    #[test]
    fn an_unknown_foe_only_thing_pointed_at_a_friend_or_bare_ground_lands_on_nobody_spent() {
        let mut rig = Rig::new();
        let rod = rig.carry(BOLT);
        rig.app.world_mut().entity_mut(rod).insert(Unknown);
        let ours = rig.sides.ours;
        let friend = rig.stand(3, 0, ours);
        assert_eq!(rig.act(UseItemAt { item: rod, at: rig.start.offset(3, 0) }), vec![ItemEvent::Used { actor: rig.player, item: rod }]);
        assert_eq!((rig.hp(friend), rig.charges(rod)), (20, 2), "at a friend: nobody hurt, a charge spent");
        let before = rig.now();
        assert_eq!(rig.act(UseItemAt { item: rod, at: rig.start.offset(0, 4) }), vec![ItemEvent::Used { actor: rig.player, item: rod }]);
        assert_eq!((rig.charges(rod), rig.now() - before), (1, 100), "at bare ground: a charge and the turn");
    }

    /// An empty thing its user knows is refused for nothing, but one not
    /// yet known is used, since a refusal would say it was empty: the use
    /// spends the turn, lands nothing, and is reported as any use is.
    #[test]
    fn an_empty_unknown_thing_is_used_for_a_turn_and_lands_nothing() {
        let mut rig = Rig::new();
        let (known, unknown) = (rig.carry(BOLT), rig.carry(BOLT));
        for rod in [known, unknown] {
            rig.app.world_mut().entity_mut(rod).insert(Consumable { left: 0, ..Consumable::new(3, WhenEmpty::Kept) });
        }
        rig.app.world_mut().entity_mut(unknown).insert(Unknown);
        let foe = rig.foe(3, 0);
        let at = rig.start.offset(3, 0);
        assert!(rig.act(UseItemAt { item: known, at }).is_empty(), "a known empty rod is refused");
        assert_eq!(rig.now(), 0, "for nothing");
        assert_eq!(rig.act(UseItemAt { item: unknown, at }), vec![ItemEvent::Used { actor: rig.player, item: unknown }], "an unknown one is used");
        assert_eq!((rig.hp(foe), rig.charges(unknown), rig.now()), (20, 0, 100), "landing nothing, still empty, for the turn");
    }

    /// A bolt reaching twelve, which wants its cell seen.
    const LONG: &str = r#"[(on: "use", targeting: (mode: Bolt(range: 12)), effects: [(kind: "Harm", args: (kind: "kinetic", roll: "4"))])]"#;

    /// Pointed at a cell out of its user's sight, which its targeting wants
    /// seen, a known thing is refused for nothing and one not yet known is
    /// used, stopping at the farthest cell on the line it could have been
    /// pointed at: its user sees eight cells, so a bolt reaching twelve
    /// pointed eleven off comes down at eight, short of a foe at ten.
    #[test]
    fn an_unknown_thing_pointed_out_of_sight_stops_at_the_farthest_cell_it_may_go() {
        let mut rig = Rig::new();
        let (known, unknown) = (rig.carry(LONG), rig.carry(LONG));
        rig.app.world_mut().entity_mut(unknown).insert(Unknown);
        let beyond = rig.foe(10, 0);
        let at = rig.start.offset(11, 0);
        assert!(rig.act(UseItemAt { item: known, at }).is_empty(), "out of sight, a known bolt is refused");
        assert_eq!(rig.now(), 0, "for nothing");
        assert_eq!(rig.act(UseItemAt { item: unknown, at }), vec![ItemEvent::Used { actor: rig.player, item: unknown }], "an unknown one is used");
        assert_eq!((rig.hp(beyond), rig.charges(unknown)), (20, 2), "and comes down short of what its user cannot see");
    }

    /// `ReflectRng` is derived by whichever of `AbilitiesPlugin` or
    /// `EffectsPlugin` adds the stream first; this rig carries neither
    /// `AbilitiesPlugin` nor a game that adds it, only items and
    /// `EffectsPlugin`, so a `Reflects` body
    /// must still turn an aimed use back on its user about half the time,
    /// rather than never, which is what an unset stream would silently do.
    #[test]
    fn an_aimed_use_reflects_off_a_reflecting_body_with_no_abilities_plugin_in_the_app() {
        let (mut users_hurt, mut bodies_hurt) = (0, 0);
        for seed in 0..40u64 {
            let mut rig = Rig::new();
            rig.app.world_mut().resource_mut::<crate::seed::Seed>().0 = rl_core::RunSeed(seed);
            rig.app.update();
            let rod = rig.carry(BOLT);
            let foe = rig.foe(3, 0);
            rig.app.world_mut().entity_mut(foe).insert(crate::ability::Reflects);
            let at = rig.start.offset(3, 0);
            rig.act(UseItemAt { item: rod, at });
            let (user_hurt, body_hurt) = (rig.hp(rig.player) < 30, rig.hp(foe) < 20);
            assert_ne!(user_hurt, body_hurt, "seed {seed}: exactly one of the user and the reflecting body is hurt");
            users_hurt += user_hurt as u32;
            bodies_hurt += body_hurt as u32;
        }
        assert!(users_hurt > 0 && bodies_hurt > 0, "both outcomes over forty seeds: {users_hurt} reflected, {bodies_hurt} landed");
    }
}

/// A thing used on another of its user's things: the resolver's refusals
/// and what the landing is made on, through the engine's own intents.
#[cfg(test)]
mod chosen {
    use super::*;
    use crate::components::{Actor, Blocks, Player, RevealsMap};
    use crate::consumable::{Consumable, WhenEmpty};
    use crate::effects::{AddEffect, AddEngineEffects, Effect, EffectKinds, EffectWorld, FromArgs, Landing, Moments};
    use crate::plugin::headless_app;
    use crate::registries::Registries;
    use crate::state::EngineState;

    /// What a game's own effect leaves on the thing a use was made on.
    #[derive(Component)]
    struct Polished;

    /// A game's own effect, as one that raises a thing's level is: it reads
    /// what the use was made on off the landing.
    struct Polish;

    impl Effect for Polish {
        fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
            if let Some(on) = landing.on {
                world.commands.entity(on).insert(Polished);
            }
        }
    }

    impl FromArgs for Polish {
        const KIND: &'static str = "Polish";
        fn from_args(_: &rl_rules::ability::RawValue, _: &rl_rules::Names<'_>) -> Result<Self, String> {
            Ok(Self)
        }
    }

    /// A polish whose use asks for a thing counted as kit.
    const POLISH: &str = r#"[(on: "use", asks: (prompt: "Polish which?", tag: "kit"), effects: [(kind: "Polish")])]"#;

    /// A player holding the first turn, with the engine's effects, the
    /// game's polish among them, charges, and one tag, `kit`.
    struct Rig {
        app: App,
        player: Entity,
        start: Point,
    }

    impl Rig {
        fn new() -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, ItemsPlugin, crate::effects::EffectsPlugin));
            app.add_engine_effects();
            app.add_effect::<Polish>().init_resource::<Registries>().insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
            app.world_mut().resource_mut::<Registries>().tags = rl_rules::Registry::from_defs(vec![rl_rules::TagDef::new("kit")]).unwrap();
            let start = crate::testing::ground(&mut app);
            let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap, Inventory::default())).id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            assert!(app.world().get::<MyTurn>(player).is_some(), "the player holds the turn a use needs");
            Self { app, player, start }
        }

        /// A thing in the player's bag doing `triggers`, with three charges
        /// kept when spent.
        fn carry(&mut self, triggers: &str) -> Entity {
            let built = {
                let world = self.app.world();
                let names = world.resource::<Registries>().names();
                let specs: Vec<rl_rules::TriggerSpec> = names.load_list(triggers).expect("the triggers parse");
                Triggers::build(&specs, &[], world.resource::<Moments>(), world.resource::<EffectKinds>(), &names).expect("the triggers build")
            };
            let thing = self.app.world_mut().spawn((Item, built, Consumable::new(3, WhenEmpty::Kept))).id();
            self.app.world_mut().get_mut::<Inventory>(self.player).expect("a bag").items.push(thing);
            thing
        }

        /// A plain thing in the bag, counted as kit or not.
        fn thing(&mut self, kit: bool) -> Entity {
            let tags = if kit { vec![self.app.world().resource::<Registries>().tags.expect("kit")] } else { Vec::new() };
            let thing = self.app.world_mut().spawn((Item, Tagged(tags))).id();
            self.app.world_mut().get_mut::<Inventory>(self.player).expect("a bag").items.push(thing);
            thing
        }

        fn act<A: Action>(&mut self, action: A) -> Vec<ItemEvent> {
            let player = self.player;
            self.app.world_mut().write_message(Intent::new(player, action));
            self.app.update();
            self.app.world_mut().resource_mut::<Messages<ItemEvent>>().drain().collect()
        }

        fn charges(&self, thing: Entity) -> u16 {
            self.app.world().get::<Consumable>(thing).expect("charges").left
        }

        fn now(&self) -> u32 {
            self.app.world().resource::<Turns>().now()
        }

        fn polished(&self, thing: Entity) -> bool {
            self.app.world().get::<Polished>(thing).is_some()
        }
    }

    /// The whole of it: a polish used on a coat lands with the coat as what
    /// it was made on, for the game's effect to find, and spends a charge
    /// and the turn like any other use.
    #[test]
    fn a_use_made_on_a_thing_lands_on_that_thing_for_a_charge_and_the_turn() {
        let mut rig = Rig::new();
        let polish = rig.carry(POLISH);
        let coat = rig.thing(true);
        assert_eq!(rig.act(UseOn { item: polish, on: coat }), vec![ItemEvent::Used { actor: rig.player, item: polish }], "a use like any other");
        assert!(rig.polished(coat), "the game's own effect found the coat on the landing");
        assert_eq!((rig.charges(polish), rig.now()), (2, 100), "for a charge and the turn");
    }

    /// A choice the use would not take is refused before anything is
    /// spent: a thing without the tag it asks for, the thing used itself,
    /// a thing lying on the ground; nor is a thing that asks used on
    /// nothing, or one that asks for nothing used on a thing.
    #[test]
    fn a_use_made_on_what_it_does_not_ask_for_is_refused_for_nothing() {
        let mut rig = Rig::new();
        let polish = rig.carry(POLISH);
        let draught = rig.carry(r#"[(on: "use", effects: [(kind: "Polish")])]"#);
        let coat = rig.thing(true);
        let stone = rig.thing(false);
        let kit = rig.app.world().resource::<Registries>().tags.expect("kit");
        let lying = rig.app.world_mut().spawn((Item, Tagged(vec![kit]), Position(rig.start))).id();
        assert!(rig.act(UseOn { item: polish, on: stone }).is_empty(), "not kit");
        assert!(rig.act(UseOn { item: polish, on: polish }).is_empty(), "not itself");
        assert!(rig.act(UseOn { item: polish, on: lying }).is_empty(), "not carried");
        assert!(rig.act(UseItem(polish)).is_empty(), "not on nothing");
        assert!(rig.act(UseOn { item: draught, on: coat }).is_empty(), "nor a use that asks for nothing on a thing");
        assert_eq!((rig.charges(polish), rig.charges(draught), rig.now()), (3, 3, 0), "nothing spent, no time passed");
        assert!(!rig.polished(coat) && !rig.polished(stone) && !rig.polished(lying));
        assert!(rig.app.world().get::<MyTurn>(rig.player).is_some(), "and the turn is still the player's");
    }

    /// A thing not yet known is used as what it is: its real trigger lands.
    #[test]
    fn a_thing_not_yet_known_still_does_what_it_does() {
        let mut rig = Rig::new();
        let polish = rig.carry(POLISH);
        rig.app.world_mut().entity_mut(polish).insert(Unknown);
        let coat = rig.thing(true);
        rig.act(UseOn { item: polish, on: coat });
        assert!(rig.polished(coat));
    }
}
