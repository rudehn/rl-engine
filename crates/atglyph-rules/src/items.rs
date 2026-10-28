//! What picking up, dropping, wearing and using a thing does: the rule
//! the items resolver applies, over facts it gathers.
//!
//! The resolver in the layer above reads the world into an [`ItemFacts`],
//! asks [`resolve_item`], and applies what comes back: it moves things
//! between the ground and the bag, merges stacks, writes the events and
//! charges the turn. What is decided here is only what a turn of item work
//! comes to, which needs no world and is tested without one, and which a
//! front end with no `App` of its own can ask as well.
//!
//! The item handle is generic, as [`Equipment`]'s is, so the rule names no
//! entity and its cases are written on plain numbers.

use atglyph_core::Point;

use crate::equip::{EquipError, EquipShape, Equipment};

/// One item action an actor asks for, on the things `I` names.
///
/// The actions as one list, so one turn spends one of them whichever kind
/// it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAsk<I> {
    /// Take everything lying on the actor's cell.
    PickUp,
    /// Put a carried thing on the ground.
    Drop(I),
    /// Put a carried thing on.
    Equip(I),
    /// Take a thing lying on the actor's cell and put it on, in one action.
    EquipFromGround(I),
    /// Take a worn thing off.
    Unequip(I),
    /// Use a carried thing where its user stands.
    Use(I),
    /// Use a carried thing whose use is aimed, pointed at a cell.
    UseAt(I, Point),
    /// Use a carried thing on another of its user's things.
    UseOn(I, I),
}

/// What the resolver read of the world for one [`ItemAsk`]: plain facts,
/// so the rule asks no query.
///
/// A fact an ask does not read may be left at its default; each field
/// says which asks read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemFacts<'a, I> {
    /// Everything lying on the actor's cell, in the order the ground lists
    /// it. Read by [`ItemAsk::PickUp`].
    pub here: Vec<I>,
    /// Whether the actor carries the thing asked about. Read by
    /// [`ItemAsk::Drop`] and [`ItemAsk::Equip`].
    pub carried: bool,
    /// Whether the thing asked about lies on the actor's cell, on the map
    /// it stands on. Read by [`ItemAsk::EquipFromGround`].
    pub underfoot: bool,
    /// Where the thing asked about goes when worn, `None` for a thing that
    /// cannot be. Read by both equips.
    pub shape: Option<&'a EquipShape>,
    /// The worn things that will not come off. Read by every ask that takes
    /// something off: a drop, an unequip, and an equip that would push
    /// something off.
    pub stuck: Vec<I>,
    /// Whether the thing may be used now, by
    /// [`may_use`](crate::equip::may_use). Read by every use.
    pub usable: bool,
    /// Whether the thing's use is pointed somewhere, so it is made with
    /// [`ItemAsk::UseAt`]. Read by [`ItemAsk::Use`] and [`ItemAsk::UseAt`].
    pub pointed: bool,
    /// Whether the thing's use asks for another of its user's things, so
    /// it is made with [`ItemAsk::UseOn`]. Read by [`ItemAsk::Use`] and
    /// [`ItemAsk::UseOn`].
    pub asks: bool,
    /// Whether the cell a [`ItemAsk::UseAt`] is pointed at is one its
    /// targeting refuses.
    pub aim_refused: bool,
    /// Whether the thing a [`ItemAsk::UseOn`] is made on is one the use
    /// does not take.
    pub choice_refused: bool,
}

impl<I> Default for ItemFacts<'_, I> {
    fn default() -> Self {
        Self {
            here: Vec::new(),
            carried: false,
            underfoot: false,
            shape: None,
            stuck: Vec::new(),
            usable: false,
            pointed: false,
            asks: false,
            aim_refused: false,
            choice_refused: false,
        }
    }
}

/// What a granted [`ItemAsk`] did, one step of it, in the order the steps
/// are reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemOutcome<I> {
    /// Taken off the ground into the bag.
    PickedUp(I),
    /// Put out of the bag onto the ground.
    Dropped(I),
    /// Put on.
    Equipped(I),
    /// Taken off, by asking or by something put on in its place.
    Unequipped(I),
    /// Used.
    Used(I),
}

/// Why an [`ItemAsk`] was refused. A refused ask changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemRefusal<I> {
    /// A pick-up on a cell with nothing on it.
    NothingHere,
    /// A drop or an equip of a thing the actor does not carry.
    NotCarried,
    /// An equip from the ground of a thing not lying under the actor.
    NotUnderfoot,
    /// An equip of a thing that cannot be worn.
    NotWearable,
    /// An equip or an unequip by an actor with no slots at all.
    NoSlots,
    /// An unequip of a thing not worn.
    NotWorn,
    /// The ask would take off a worn thing that will not come off: this
    /// one.
    WouldNotComeOff(I),
    /// An equip the equipment refused for its shape: a shape naming no
    /// slot, or a slot the wearer does not have.
    CannotGoOn(EquipError),
    /// A use of a thing that may not be used now.
    Unusable,
    /// A plain use of a thing whose use is pointed somewhere.
    Pointed,
    /// A plain use of a thing whose use asks for another thing.
    Asks,
    /// A use pointed at a cell of a thing whose use is not pointed.
    NotPointed,
    /// A use made on a thing of a thing whose use asks for nothing.
    AsksNothing,
    /// A pointed use aimed where its targeting refuses.
    AimRefused,
    /// A use made on a thing it does not take.
    ChoiceRefused,
}

/// What `ask` comes to, given `facts` about it and what the actor wears,
/// `worn`, which a granted equip, unequip or drop of a worn thing changes.
///
/// Granted, the steps it took in the order they are reported: an equip
/// that displaces things reports each coming off before the new one goes
/// on, and one from the ground reports the pick-up first. Refused, why,
/// with `worn` untouched. A worn thing in `facts.stuck` refuses whatever
/// would take it off, and an equip takes, of the slots its shape allows,
/// one whose occupant may come off, by [`Equipment::equip_keeping`].
pub fn resolve_item<I: Copy + Eq>(ask: ItemAsk<I>, facts: &ItemFacts<'_, I>, worn: Option<&mut Equipment<I>>) -> Result<Vec<ItemOutcome<I>>, ItemRefusal<I>> {
    use ItemOutcome as Out;
    use ItemRefusal as No;
    let stuck_on = |worn: &Option<&mut Equipment<I>>, item: I| facts.stuck.contains(&item) && worn.as_ref().is_some_and(|w| w.contains(item));
    match ask {
        ItemAsk::PickUp if facts.here.is_empty() => Err(No::NothingHere),
        ItemAsk::PickUp => Ok(facts.here.iter().map(|i| Out::PickedUp(*i)).collect()),
        ItemAsk::Drop(item) => {
            // Dropped, a worn thing comes off, which a stuck one will not.
            if stuck_on(&worn, item) {
                return Err(No::WouldNotComeOff(item));
            }
            if !facts.carried {
                return Err(No::NotCarried);
            }
            let mut out = Vec::new();
            if worn.is_some_and(|w| w.unequip(item)) {
                out.push(Out::Unequipped(item));
            }
            out.push(Out::Dropped(item));
            Ok(out)
        }
        ItemAsk::Equip(item) => {
            let (Some(worn), Some(shape)) = (worn, facts.shape) else { return Err(refuse_equip(facts.shape)) };
            if !facts.carried {
                return Err(No::NotCarried);
            }
            put_on(worn, item, shape, &facts.stuck, Vec::new())
        }
        ItemAsk::EquipFromGround(item) => {
            let (Some(worn), Some(shape)) = (worn, facts.shape) else { return Err(refuse_equip(facts.shape)) };
            if !facts.underfoot {
                return Err(No::NotUnderfoot);
            }
            put_on(worn, item, shape, &facts.stuck, vec![Out::PickedUp(item)])
        }
        ItemAsk::Unequip(item) => {
            if stuck_on(&worn, item) {
                return Err(No::WouldNotComeOff(item));
            }
            match worn.map(|w| w.unequip(item)) {
                Some(true) => Ok(vec![Out::Unequipped(item)]),
                Some(false) => Err(No::NotWorn),
                None => Err(No::NoSlots),
            }
        }
        // A thing whose use is pointed somewhere is used at a cell: used
        // here it would land around its user, which its targeting says it
        // does not. One that asks for a thing is used on one, which this
        // does not name.
        ItemAsk::Use(item) => match () {
            _ if !facts.usable => Err(No::Unusable),
            _ if facts.pointed => Err(No::Pointed),
            _ if facts.asks => Err(No::Asks),
            _ => Ok(vec![Out::Used(item)]),
        },
        ItemAsk::UseAt(item, _) => match () {
            _ if !facts.pointed => Err(No::NotPointed),
            _ if !facts.usable => Err(No::Unusable),
            _ if facts.aim_refused => Err(No::AimRefused),
            _ => Ok(vec![Out::Used(item)]),
        },
        ItemAsk::UseOn(item, _) => match () {
            _ if !facts.asks => Err(No::AsksNothing),
            _ if !facts.usable => Err(No::Unusable),
            _ if facts.choice_refused => Err(No::ChoiceRefused),
            _ => Ok(vec![Out::Used(item)]),
        },
    }
}

/// Why an equip with no slots or no shape is refused: a thing that cannot
/// be worn is that whoever asks, so it is named first.
fn refuse_equip<I>(shape: Option<&EquipShape>) -> ItemRefusal<I> {
    if shape.is_none() { ItemRefusal::NotWearable } else { ItemRefusal::NoSlots }
}

/// Puts `item` on over whatever may come off, after the steps `first`.
fn put_on<I: Copy + Eq>(
    worn: &mut Equipment<I>,
    item: I,
    shape: &EquipShape,
    stuck: &[I],
    mut first: Vec<ItemOutcome<I>>,
) -> Result<Vec<ItemOutcome<I>>, ItemRefusal<I>> {
    match worn.equip_keeping(item, shape, |i| stuck.contains(&i)) {
        Ok(displaced) => {
            first.extend(displaced.into_iter().map(ItemOutcome::Unequipped));
            first.push(ItemOutcome::Equipped(item));
            Ok(first)
        }
        // A kept slot always has its keeper in it; the arm for none is
        // the equipment's own error, never reached.
        Err(EquipError::Kept(slot)) => Err(worn.in_slot(slot).map_or(ItemRefusal::CannotGoOn(EquipError::Kept(slot)), ItemRefusal::WouldNotComeOff)),
        Err(e) => Err(ItemRefusal::CannotGoOn(e)),
    }
}

/// How `count` arriving units of a kind fill the stacks of it already
/// carried, each given as how many it holds and the most it may, in bag
/// order: what each one takes, and what is left over to lie in the bag as
/// a stack of its own.
///
/// In bag order, so a part stack fills before a later one starts; a stack
/// with no limit takes everything that reaches it, and one already past its
/// limit takes nothing, since the limit is what a merge keeps to, not a
/// rule that empties a stack a game made bigger.
pub fn fill_stacks(held: &[(u32, Option<u32>)], count: u32) -> (Vec<u32>, u32) {
    let mut left = count;
    let takes = held
        .iter()
        .map(|&(has, limit)| {
            let room = limit.map_or(u32::MAX, |most| most.saturating_sub(has));
            let take = room.min(left);
            left -= take;
            take
        })
        .collect();
    (takes, left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equip::SlotId;
    use ItemOutcome::*;
    use ItemRefusal as No;

    const MAIN: SlotId = SlotId::from_raw(0);
    const OFF: SlotId = SlotId::from_raw(1);

    /// Two hands, empty.
    fn hands() -> Equipment<u32> {
        Equipment::with_slot_count(2)
    }

    /// Two hands, `main` holding 1 and `off` 2.
    fn full_hands() -> Equipment<u32> {
        let mut worn = hands();
        worn.equip(1, &EquipShape::in_slot(MAIN)).unwrap();
        worn.equip(2, &EquipShape::in_slot(OFF)).unwrap();
        worn
    }

    fn carried<'a>() -> ItemFacts<'a, u32> {
        ItemFacts { carried: true, ..ItemFacts::default() }
    }

    #[test]
    fn a_pick_up_takes_everything_underfoot_in_the_order_it_lies() {
        let facts = ItemFacts { here: vec![5, 3, 9], ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::PickUp, &facts, None), Ok(vec![PickedUp(5), PickedUp(3), PickedUp(9)]));
    }

    #[test]
    fn a_pick_up_on_bare_ground_is_refused() {
        assert_eq!(resolve_item(ItemAsk::PickUp, &ItemFacts::default(), Some(&mut hands())), Err(No::NothingHere));
    }

    #[test]
    fn a_carried_thing_not_worn_is_dropped() {
        let mut worn = full_hands();
        assert_eq!(resolve_item(ItemAsk::Drop(7), &carried(), Some(&mut worn)), Ok(vec![Dropped(7)]));
        assert_eq!(worn, full_hands(), "nothing came off");
        assert_eq!(resolve_item(ItemAsk::Drop(7), &carried(), None), Ok(vec![Dropped(7)]), "by a carrier wearing nothing too");
    }

    #[test]
    fn a_worn_thing_dropped_comes_off_before_it_is_dropped() {
        let mut worn = full_hands();
        assert_eq!(resolve_item(ItemAsk::Drop(1), &carried(), Some(&mut worn)), Ok(vec![Unequipped(1), Dropped(1)]));
        assert!(worn.is_free(MAIN));
    }

    #[test]
    fn a_stuck_thing_worn_is_not_dropped() {
        let mut worn = full_hands();
        let facts = ItemFacts { stuck: vec![1], ..carried() };
        assert_eq!(resolve_item(ItemAsk::Drop(1), &facts, Some(&mut worn)), Err(No::WouldNotComeOff(1)));
        assert_eq!(worn, full_hands(), "still worn");
        assert_eq!(
            resolve_item(ItemAsk::Drop(7), &ItemFacts { stuck: vec![7], ..carried() }, Some(&mut hands())),
            Ok(vec![Dropped(7)]),
            "one not worn holds nothing on"
        );
    }

    #[test]
    fn a_thing_not_carried_is_not_dropped() {
        assert_eq!(resolve_item(ItemAsk::Drop(7), &ItemFacts::default(), Some(&mut hands())), Err(No::NotCarried));
    }

    #[test]
    fn a_carried_wearable_thing_is_put_on() {
        let shape = EquipShape::in_slot(MAIN);
        let mut worn = hands();
        let facts = ItemFacts { shape: Some(&shape), ..carried() };
        assert_eq!(resolve_item(ItemAsk::Equip(7), &facts, Some(&mut worn)), Ok(vec![Equipped(7)]));
        assert_eq!(worn.in_slot(MAIN), Some(7));
    }

    #[test]
    fn equipping_over_two_things_takes_both_off_before_putting_it_on() {
        let shape = EquipShape::in_slot(MAIN).and_claims(OFF);
        let mut worn = full_hands();
        let facts = ItemFacts { shape: Some(&shape), ..carried() };
        assert_eq!(resolve_item(ItemAsk::Equip(7), &facts, Some(&mut worn)), Ok(vec![Unequipped(1), Unequipped(2), Equipped(7)]));
        assert_eq!((worn.in_slot(MAIN), worn.in_slot(OFF)), (Some(7), Some(7)));
    }

    #[test]
    fn nothing_put_on_over_a_stuck_thing_pushes_it_off() {
        let (shield, pike) = (EquipShape::in_slot(OFF), EquipShape::in_slot(MAIN).and_claims(OFF));
        for shape in [&shield, &pike] {
            let mut worn = full_hands();
            let facts = ItemFacts { shape: Some(shape), stuck: vec![2], ..carried() };
            assert_eq!(resolve_item(ItemAsk::Equip(7), &facts, Some(&mut worn)), Err(No::WouldNotComeOff(2)));
            assert_eq!(worn, full_hands(), "nothing moved");
        }
    }

    #[test]
    fn a_thing_that_goes_in_either_slot_takes_the_one_whose_occupant_may_come_off() {
        let either = EquipShape::in_any([OFF, MAIN]);
        let mut worn = full_hands();
        let facts = ItemFacts { shape: Some(&either), stuck: vec![2], ..carried() };
        assert_eq!(resolve_item(ItemAsk::Equip(7), &facts, Some(&mut worn)), Ok(vec![Unequipped(1), Equipped(7)]));
        assert_eq!((worn.in_slot(MAIN), worn.in_slot(OFF)), (Some(7), Some(2)));
    }

    #[test]
    fn a_thing_not_carried_not_wearable_or_with_no_slots_to_go_in_is_not_put_on() {
        let shape = EquipShape::in_slot(MAIN);
        let wearable = ItemFacts { shape: Some(&shape), ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::Equip(7), &wearable, Some(&mut hands())), Err(No::NotCarried));
        assert_eq!(resolve_item(ItemAsk::Equip(7), &carried(), Some(&mut hands())), Err(No::NotWearable));
        assert_eq!(resolve_item(ItemAsk::Equip(7), &ItemFacts { carried: true, ..wearable }, None), Err(No::NoSlots));
    }

    #[test]
    fn a_shape_naming_a_slot_the_wearer_lacks_is_refused_by_the_equipment() {
        let shape = EquipShape::in_slot(SlotId::from_raw(5));
        let mut worn = full_hands();
        let facts = ItemFacts { shape: Some(&shape), ..carried() };
        assert_eq!(resolve_item(ItemAsk::Equip(7), &facts, Some(&mut worn)), Err(No::CannotGoOn(EquipError::UnknownSlot(SlotId::from_raw(5)))));
        assert_eq!(worn, full_hands());
    }

    #[test]
    fn equipping_from_the_ground_picks_it_up_before_anything_comes_off() {
        let shape = EquipShape::in_slot(MAIN);
        let mut worn = full_hands();
        let facts = ItemFacts { shape: Some(&shape), underfoot: true, ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::EquipFromGround(7), &facts, Some(&mut worn)), Ok(vec![PickedUp(7), Unequipped(1), Equipped(7)]));
        assert_eq!(worn.in_slot(MAIN), Some(7));
    }

    #[test]
    fn only_a_wearable_thing_underfoot_is_equipped_from_the_ground() {
        let shape = EquipShape::in_slot(MAIN);
        let beside = ItemFacts { shape: Some(&shape), carried: true, ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::EquipFromGround(7), &beside, Some(&mut hands())), Err(No::NotUnderfoot), "carried is not underfoot");
        let plain = ItemFacts { underfoot: true, ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::EquipFromGround(7), &plain, Some(&mut hands())), Err(No::NotWearable));
    }

    #[test]
    fn a_stuck_thing_is_not_pushed_off_by_a_thing_from_the_ground() {
        let pike = EquipShape::in_slot(MAIN).and_claims(OFF);
        let mut worn = full_hands();
        let facts = ItemFacts { shape: Some(&pike), underfoot: true, stuck: vec![2], ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::EquipFromGround(7), &facts, Some(&mut worn)), Err(No::WouldNotComeOff(2)), "and is not picked up either");
        assert_eq!(worn, full_hands());
    }

    #[test]
    fn a_worn_thing_comes_off() {
        let mut worn = full_hands();
        assert_eq!(resolve_item(ItemAsk::Unequip(1), &ItemFacts::default(), Some(&mut worn)), Ok(vec![Unequipped(1)]));
        assert!(worn.is_free(MAIN) && !worn.is_free(OFF));
    }

    #[test]
    fn a_stuck_thing_worn_does_not_come_off() {
        let mut worn = full_hands();
        let facts = ItemFacts { stuck: vec![1], ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::Unequip(1), &facts, Some(&mut worn)), Err(No::WouldNotComeOff(1)));
        assert_eq!(worn, full_hands());
    }

    #[test]
    fn a_thing_not_worn_is_not_taken_off() {
        assert_eq!(resolve_item(ItemAsk::Unequip(7), &ItemFacts::default(), Some(&mut full_hands())), Err(No::NotWorn));
        assert_eq!(resolve_item(ItemAsk::Unequip(7), &ItemFacts::default(), None), Err(No::NoSlots));
    }

    #[test]
    fn a_usable_thing_used_plainly_is_used() {
        let facts = ItemFacts { usable: true, ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::Use(7), &facts, None), Ok(vec![Used(7)]));
    }

    #[test]
    fn a_plain_use_is_refused_for_a_thing_unusable_pointed_or_asking() {
        let usable = ItemFacts { usable: true, ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::Use(7), &ItemFacts::default(), None), Err(No::Unusable));
        assert_eq!(resolve_item(ItemAsk::Use(7), &ItemFacts { pointed: true, ..usable.clone() }, None), Err(No::Pointed));
        assert_eq!(resolve_item(ItemAsk::Use(7), &ItemFacts { asks: true, ..usable }, None), Err(No::Asks));
    }

    #[test]
    fn a_pointed_use_aimed_where_it_may_go_is_used() {
        let facts = ItemFacts { usable: true, pointed: true, ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::UseAt(7, Point::new(3, 4)), &facts, None), Ok(vec![Used(7)]));
    }

    /// A thing the player does not know is gathered as aimed where it may
    /// go, whatever its targeting says, and usable even when empty; the
    /// rule reads only the facts, so it is this case.
    #[test]
    fn a_pointed_use_is_refused_for_a_thing_not_pointed_unusable_or_aimed_where_it_may_not_go() {
        let at = Point::new(3, 4);
        let pointed = ItemFacts { usable: true, pointed: true, ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::UseAt(7, at), &ItemFacts { usable: true, ..ItemFacts::default() }, None), Err(No::NotPointed));
        assert_eq!(resolve_item(ItemAsk::UseAt(7, at), &ItemFacts { usable: false, ..pointed.clone() }, None), Err(No::Unusable));
        assert_eq!(resolve_item(ItemAsk::UseAt(7, at), &ItemFacts { aim_refused: true, ..pointed }, None), Err(No::AimRefused));
    }

    #[test]
    fn a_use_made_on_a_thing_it_takes_is_used() {
        let facts = ItemFacts { usable: true, asks: true, ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::UseOn(7, 8), &facts, None), Ok(vec![Used(7)]));
    }

    #[test]
    fn a_use_made_on_a_thing_is_refused_for_one_asking_nothing_unusable_or_a_choice_it_does_not_take() {
        let asks = ItemFacts { usable: true, asks: true, ..ItemFacts::default() };
        assert_eq!(resolve_item(ItemAsk::UseOn(7, 8), &ItemFacts { usable: true, ..ItemFacts::default() }, None), Err(No::AsksNothing));
        assert_eq!(resolve_item(ItemAsk::UseOn(7, 8), &ItemFacts { usable: false, ..asks.clone() }, None), Err(No::Unusable));
        assert_eq!(resolve_item(ItemAsk::UseOn(7, 8), &ItemFacts { choice_refused: true, ..asks }, None), Err(No::ChoiceRefused));
    }

    #[test]
    fn an_arrival_fills_the_carried_stack_to_its_limit_and_leaves_the_rest() {
        assert_eq!(fill_stacks(&[(45, Some(50))], 10), (vec![5], 5));
    }

    #[test]
    fn an_arrival_fills_part_stacks_in_bag_order() {
        assert_eq!(fill_stacks(&[(48, Some(50)), (30, Some(50))], 5), (vec![2, 3], 0));
    }

    #[test]
    fn a_stack_with_no_limit_takes_the_whole_arrival() {
        assert_eq!(fill_stacks(&[(5, None), (2, None)], 3), (vec![3, 0], 0));
    }

    #[test]
    fn a_stack_already_past_its_limit_takes_nothing() {
        assert_eq!(fill_stacks(&[(60, Some(50))], 4), (vec![0], 4));
    }

    #[test]
    fn with_nothing_carried_the_whole_arrival_is_left() {
        assert_eq!(fill_stacks(&[], 4), (vec![], 4));
    }
}
