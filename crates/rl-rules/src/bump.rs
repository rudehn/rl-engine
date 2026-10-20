//! What walking into a cell comes to, as plain values.
//!
//! The Bevy layer gathers who stands in the cell, how they stand to the
//! walker and what they offer; the choice among a step, a door, a blow, a
//! swap, a thing taken up and a question lives here so it is tested without
//! an `App`. `A` is whoever stands there and `O` is one offer, so the rule
//! needs neither entities nor props.

use crate::faction::Relation;

/// What the thing in a cell offers whoever walked into it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offered<O> {
    /// Nothing, or nothing there offers at all.
    None,
    /// One plain thing, which the walk key comes to.
    One(O),
    /// More than one, or one that is refused, costs, or asks first: a
    /// question or an explanation, which a screen answers and a walk key
    /// cannot.
    Several,
}

/// What a bump comes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpOutcome<A, O> {
    /// Walk onto the open cell.
    Step,
    /// Open the door in the cell.
    Open,
    /// Strike the foe standing there.
    Attack(A),
    /// Change places with them.
    Swap(A),
    /// Take up the one thing they offer.
    Interact(O),
    /// Report an offer a screen must ask or explain; the turn is kept.
    Ask(A),
    /// Refuse: the cell holds someone the walker will not strike or pass.
    Refuse(A),
}

/// What an `offers` list comes to for a walk key.
///
/// Only one offer that is `plain` (no price, no refusal, nothing to ask
/// first) is something a movement key may answer: spending currency or
/// confirming is a choice that must never be made by walking into someone.
pub fn classify_offers<O: Copy>(offers: &[O], plain: impl Fn(&O) -> bool) -> Offered<O> {
    match offers {
        [] => Offered::None,
        [only] if plain(only) => Offered::One(*only),
        _ => Offered::Several,
    }
}

/// What walking into a cell comes to.
///
/// `someone` is whoever in it can be struck, `stance` how the walker
/// regards them (`None` when either has no side or no rules name one),
/// `gives_way` whether their side is one that is passed through, which only
/// counts for a neutral, `allies_swap` whether the game swaps with allies,
/// `offered` what the cell offers and `door` whether it holds a door.
///
/// A foe is struck and an ally walked past whatever either offers: the walk
/// key into a foe is a blow, and a companion with something to say is still
/// a companion to step round. Anyone else who offers is taken up or asked
/// about before they are refused. A question needs a name to carry, so a
/// caller whose cell holds something that offers but nobody to strike
/// passes that thing as `someone`, with a `stance` of `None`.
pub fn resolve_bump<A: Copy, O>(
    someone: Option<A>,
    stance: Option<Relation>,
    gives_way: bool,
    allies_swap: bool,
    offered: Offered<O>,
    door: bool,
) -> BumpOutcome<A, O> {
    let gives_way = gives_way && stance == Some(Relation::Neutral);
    if !matches!(stance, Some(Relation::Hostile | Relation::Allied)) && !gives_way {
        match (offered, someone) {
            (Offered::One(offer), _) => return BumpOutcome::Interact(offer),
            (Offered::Several, Some(what)) => return BumpOutcome::Ask(what),
            _ => {}
        }
    }
    match someone {
        Some(other) => match stance {
            Some(Relation::Hostile) => BumpOutcome::Attack(other),
            Some(Relation::Allied) if allies_swap => BumpOutcome::Swap(other),
            Some(Relation::Neutral) if gives_way => BumpOutcome::Swap(other),
            _ => BumpOutcome::Refuse(other),
        },
        None if door => BumpOutcome::Open,
        None => BumpOutcome::Step,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::faction::Relation::{Allied, Hostile, Neutral};

    fn plain(o: &(bool, u8)) -> bool {
        o.0
    }

    #[test]
    fn nothing_offered_is_none_one_plain_offer_is_one_and_anything_else_is_several() {
        assert_eq!(classify_offers::<(bool, u8)>(&[], plain), Offered::None);
        assert_eq!(classify_offers(&[(true, 1)], plain), Offered::One((true, 1)), "a plain offer is taken up by a walk key");
        assert_eq!(classify_offers(&[(false, 1)], plain), Offered::Several, "one that asks, costs or is refused is a question");
        assert_eq!(classify_offers(&[(true, 1), (true, 2)], plain), Offered::Several, "two cannot be chosen between");
    }

    fn bump(
        someone: Option<u8>,
        stance: Option<crate::faction::Relation>,
        gives_way: bool,
        allies_swap: bool,
        offered: Offered<char>,
        door: bool,
    ) -> BumpOutcome<u8, char> {
        resolve_bump(someone, stance, gives_way, allies_swap, offered, door)
    }

    #[test]
    fn empty_ground_is_a_step_and_a_door_there_is_opened() {
        assert_eq!(bump(None, None, false, false, Offered::None, false), BumpOutcome::Step);
        assert_eq!(bump(None, None, false, false, Offered::None, true), BumpOutcome::Open);
    }

    #[test]
    fn a_foe_is_struck_and_an_ally_is_swapped_with_only_when_the_rule_says_so_whatever_either_offers() {
        for offered in [Offered::None, Offered::One('x'), Offered::Several] {
            assert_eq!(bump(Some(7), Some(Hostile), false, false, offered, false), BumpOutcome::Attack(7));
            assert_eq!(bump(Some(7), Some(Allied), false, true, offered, false), BumpOutcome::Swap(7));
            assert_eq!(bump(Some(7), Some(Allied), false, false, offered, false), BumpOutcome::Refuse(7));
        }
    }

    #[test]
    fn someone_of_no_side_or_a_neutral_is_refused_unless_it_gives_way() {
        assert_eq!(bump(Some(7), None, false, true, Offered::None, false), BumpOutcome::Refuse(7));
        assert_eq!(bump(Some(7), Some(Neutral), false, true, Offered::None, false), BumpOutcome::Refuse(7));
        assert_eq!(bump(Some(7), Some(Neutral), true, false, Offered::None, false), BumpOutcome::Swap(7));
        assert_eq!(bump(Some(7), Some(Neutral), true, false, Offered::One('x'), false), BumpOutcome::Swap(7), "whatever it offers");
    }

    #[test]
    fn giving_way_means_nothing_to_someone_who_is_a_foe_or_has_no_stance() {
        assert_eq!(bump(Some(7), Some(Hostile), true, false, Offered::None, false), BumpOutcome::Attack(7));
        assert_eq!(bump(Some(7), None, true, false, Offered::None, false), BumpOutcome::Refuse(7));
    }

    #[test]
    fn one_plain_offer_from_a_non_foe_is_taken_up_and_several_is_asked_about() {
        assert_eq!(bump(Some(7), Some(Neutral), false, false, Offered::One('x'), false), BumpOutcome::Interact('x'));
        assert_eq!(bump(Some(7), Some(Neutral), false, false, Offered::Several, false), BumpOutcome::Ask(7));
        assert_eq!(bump(Some(7), None, false, false, Offered::Several, false), BumpOutcome::Ask(7));
    }

    #[test]
    fn something_that_offers_with_no_one_to_strike_is_taken_up_or_asked_about_before_a_step_or_a_door() {
        assert_eq!(bump(None, None, false, false, Offered::One('x'), true), BumpOutcome::Interact('x'));
        assert_eq!(bump(None, None, false, false, Offered::One('x'), false), BumpOutcome::Interact('x'));
    }
}
