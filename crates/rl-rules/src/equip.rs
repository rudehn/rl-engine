//! Equipment slots and what occupies them.
//!
//! A slot is a registered id, so a game with hands, a head and a back has
//! three slots and a game with hardpoints on a hull has twelve. An item
//! says where it goes as an [`EquipShape`]: the slots it may take, and the
//! slots it also claims once it is there. A two-handed weapon takes the
//! main hand and also claims the off hand; a ring takes whichever finger
//! is free. [`Equipment`] keeps the claims and reports what each equip
//! displaced, so the game can put those items back in the bag.
//!
//! Generic over the item handle: the Bevy layer uses entities, a balance
//! tool can use definition ids, and the tests here use integers.

use rl_content::{Named, Registry};
use rl_core::Id;
use serde::{Deserialize, Serialize};

/// A registered equipment slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotDef {
    /// The name content refers to it by.
    pub name: String,
}

impl SlotDef {
    /// A slot called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Named for SlotDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered slot id.
pub type SlotId = Id<SlotDef>;

/// Where an item goes when worn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquipShape {
    /// Slots the item may take. The first free one is taken; if none is
    /// free, the first is taken and its occupant displaced.
    pub any_of: Vec<SlotId>,
    /// Slots the item also claims wherever it went, displacing whatever
    /// holds them. A two-hander lists the off hand here.
    pub also: Vec<SlotId>,
}

impl EquipShape {
    /// An item that goes in exactly `slot`.
    pub fn in_slot(slot: SlotId) -> Self {
        Self {
            any_of: vec![slot],
            also: Vec::new(),
        }
    }

    /// An item that goes in any of `slots`.
    pub fn in_any(slots: impl IntoIterator<Item = SlotId>) -> Self {
        Self {
            any_of: slots.into_iter().collect(),
            also: Vec::new(),
        }
    }

    /// Also claims `slot`.
    pub fn and_claims(mut self, slot: SlotId) -> Self {
        self.also.push(slot);
        self
    }

    /// Every slot the shape names.
    pub fn slots(&self) -> impl Iterator<Item = SlotId> + '_ {
        self.any_of.iter().chain(self.also.iter()).copied()
    }
}

/// Why an item could not be equipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipError {
    /// The shape names no slot to take.
    NoSlot,
    /// The shape names a slot this equipment does not have.
    UnknownSlot(SlotId),
}

impl std::fmt::Display for EquipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EquipError::NoSlot => write!(f, "the item names no slot to go in"),
            EquipError::UnknownSlot(s) => write!(f, "slot {} does not exist here", s.raw()),
        }
    }
}

impl std::error::Error for EquipError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Claim<I> {
    item: I,
    /// Whether this is the slot the item was equipped into, as opposed to
    /// one it also claims.
    primary: bool,
}

/// What one wearer has on, by slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Equipment<I> {
    slots: Vec<Option<Claim<I>>>,
}

impl<I: Copy + Eq> Equipment<I> {
    /// Empty equipment with one slot per definition in `slots`.
    pub fn for_slots(slots: &Registry<SlotDef>) -> Self {
        Self::with_slot_count(slots.len())
    }

    /// Empty equipment with `count` slots.
    pub fn with_slot_count(count: usize) -> Self {
        Self { slots: vec![None; count] }
    }

    /// How many slots there are.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Puts `item` on. Returns the items it displaced, each once, in slot
    /// order. An item already worn is taken off first, so equipping it
    /// again moves it rather than doubling it.
    pub fn equip(&mut self, item: I, shape: &EquipShape) -> Result<Vec<I>, EquipError> {
        let primary = *shape.any_of.first().ok_or(EquipError::NoSlot)?;
        for slot in shape.slots() {
            if slot.index() >= self.slots.len() {
                return Err(EquipError::UnknownSlot(slot));
            }
        }
        self.unequip(item);
        let primary = shape.any_of.iter().copied().find(|s| self.slots[s.index()].is_none()).unwrap_or(primary);
        let mut displaced = Vec::new();
        for slot in std::iter::once(primary).chain(shape.also.iter().copied()) {
            if let Some(claim) = self.slots[slot.index()]
                && !displaced.contains(&claim.item)
            {
                displaced.push(claim.item);
            }
        }
        for other in &displaced {
            self.unequip(*other);
        }
        self.slots[primary.index()] = Some(Claim { item, primary: true });
        for slot in &shape.also {
            self.slots[slot.index()] = Some(Claim { item, primary: false });
        }
        Ok(displaced)
    }

    /// Takes `item` off, freeing every slot it claimed. Returns whether it
    /// was worn.
    pub fn unequip(&mut self, item: I) -> bool {
        let mut found = false;
        for slot in &mut self.slots {
            if slot.is_some_and(|c| c.item == item) {
                *slot = None;
                found = true;
            }
        }
        found
    }

    /// Whether `item` is worn.
    pub fn contains(&self, item: I) -> bool {
        self.slots.iter().flatten().any(|c| c.item == item)
    }

    /// What holds `slot`, whether as its own slot or a claimed one.
    pub fn in_slot(&self, slot: SlotId) -> Option<I> {
        self.slots.get(slot.index()).copied().flatten().map(|c| c.item)
    }

    /// The slot `item` was equipped into.
    pub fn slot_of(&self, item: I) -> Option<SlotId> {
        self.slots
            .iter()
            .position(|s| s.is_some_and(|c| c.item == item && c.primary))
            .map(|i| SlotId::from_raw(i as u32))
    }

    /// Every worn item with the slot it was equipped into, in slot order.
    pub fn worn(&self) -> impl Iterator<Item = (SlotId, I)> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.filter(|c| c.primary).map(|c| (SlotId::from_raw(i as u32), c.item)))
    }

    /// Whether `slot` is free.
    pub fn is_free(&self, slot: SlotId) -> bool {
        self.in_slot(slot).is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hands_and_fingers() -> (Registry<SlotDef>, SlotId, SlotId, SlotId, SlotId) {
        let r = Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("off hand"), SlotDef::new("left ring"), SlotDef::new("right ring")]).unwrap();
        (r.clone(), r.expect("main hand"), r.expect("off hand"), r.expect("left ring"), r.expect("right ring"))
    }

    #[test]
    fn a_ring_takes_the_free_finger_then_displaces_the_first() {
        let (r, _, _, left, right) = hands_and_fingers();
        let mut eq: Equipment<u32> = Equipment::for_slots(&r);
        let ring = EquipShape::in_any([left, right]);
        assert_eq!(eq.equip(1, &ring), Ok(vec![]));
        assert_eq!(eq.equip(2, &ring), Ok(vec![]));
        assert_eq!(eq.in_slot(left), Some(1));
        assert_eq!(eq.in_slot(right), Some(2));
        assert_eq!(eq.equip(3, &ring), Ok(vec![1]), "no finger free: the first is displaced");
        assert_eq!(eq.in_slot(left), Some(3));
        assert!(!eq.contains(1));
    }

    #[test]
    fn a_two_hander_claims_both_hands_and_a_shield_takes_it_off() {
        let (r, main, off, _, _) = hands_and_fingers();
        let mut eq: Equipment<u32> = Equipment::for_slots(&r);
        let sword = EquipShape::in_slot(main);
        let shield = EquipShape::in_slot(off);
        let greatsword = EquipShape::in_slot(main).and_claims(off);
        eq.equip(1, &sword).unwrap();
        eq.equip(2, &shield).unwrap();
        assert_eq!(eq.equip(3, &greatsword), Ok(vec![1, 2]), "both hands were displaced, in slot order");
        assert_eq!(eq.in_slot(main), Some(3));
        assert_eq!(eq.in_slot(off), Some(3));
        assert_eq!(eq.slot_of(3), Some(main), "the off hand is a claim, not where it lives");
        assert_eq!(eq.worn().collect::<Vec<_>>(), vec![(main, 3)], "worn lists it once");
        assert_eq!(eq.equip(2, &shield), Ok(vec![3]), "a shield displaces the two-hander whole");
        assert!(eq.is_free(main));
        assert_eq!(eq.in_slot(off), Some(2));
    }

    #[test]
    fn unequip_frees_every_claim_and_re_equipping_moves() {
        let (r, main, off, _, _) = hands_and_fingers();
        let mut eq: Equipment<u32> = Equipment::for_slots(&r);
        eq.equip(3, &EquipShape::in_slot(main).and_claims(off)).unwrap();
        assert!(eq.unequip(3));
        assert!(!eq.unequip(3), "already off");
        assert!(eq.is_free(main) && eq.is_free(off));
        eq.equip(1, &EquipShape::in_slot(main)).unwrap();
        assert_eq!(eq.equip(1, &EquipShape::in_slot(off)), Ok(vec![]), "moving an item displaces nothing, least of all itself");
        assert!(eq.is_free(main));
        assert_eq!(eq.in_slot(off), Some(1));
    }

    #[test]
    fn bad_shapes_are_refused_before_anything_changes() {
        let (r, main, _, _, _) = hands_and_fingers();
        let mut eq: Equipment<u32> = Equipment::for_slots(&r);
        eq.equip(1, &EquipShape::in_slot(main)).unwrap();
        assert_eq!(eq.equip(2, &EquipShape { any_of: vec![], also: vec![] }), Err(EquipError::NoSlot));
        let bogus = SlotId::from_raw(9);
        assert_eq!(eq.equip(2, &EquipShape::in_slot(main).and_claims(bogus)), Err(EquipError::UnknownSlot(bogus)));
        assert_eq!(eq.in_slot(main), Some(1), "the refused equip touched nothing");
    }
}
