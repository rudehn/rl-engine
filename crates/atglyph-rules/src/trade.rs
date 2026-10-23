//! What a deal comes to and what draws a purse down, as plain values.
//!
//! The Bevy layer gathers a trader's share, a thing's worth, tags and
//! whether it is worn, and applies what this returns; the price and the
//! order of refusals live here so a screen and the resolver strike every
//! deal by one rule and both are tested without an `App`.

/// Which way a deal went, from the customer's side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deal {
    /// The customer bought from the trader.
    Bought,
    /// The customer sold to the trader.
    Sold,
}

/// Why a deal was refused, for a screen to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeRefusal {
    /// Whoever pays carries less currency than the price: the customer
    /// buying, the trader buying back.
    Short,
    /// The trader does not buy a thing of this kind.
    NotBought,
    /// It has no worth, so it is neither sold nor bought.
    NoWorth,
    /// It is worn, and comes off before it changes hands.
    Worn,
}

/// What a deal needs to know of the thing changing hands, with the trader's
/// reading of its tags already folded into `bought`, so the rule needs
/// neither tags nor a trader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    /// Its worth, if it has one.
    pub worth: Option<u32>,
    /// Whether the trader buys a thing of its kind.
    pub bought: bool,
    /// Whether whoever holds it wears it.
    pub worn: bool,
}

/// What one of a thing worth `worth` is paid for at `pays_pct` hundredths,
/// rounded down and held to what a number holds.
///
/// Per one rather than per handful, so selling a stack whole pays what
/// selling it one at a time would.
fn pays(worth: u32, pays_pct: u32) -> u32 {
    (u64::from(worth) * u64::from(pays_pct) / 100).min(u64::from(u32::MAX)) as u32
}

/// What one of `piece` changes hands for going the way `deal` says: its
/// worth bought, the trader's share of it sold. `None` for a thing with no
/// worth, or sold and of a kind the trader does not buy, which has no price
/// to show.
///
/// Whether it is worn does not enter into it: a worn thing still has a
/// price, it is only not dealt until it comes off.
pub fn each(deal: Deal, pays_pct: u32, piece: Piece) -> Option<u32> {
    let worth = piece.worth?;
    match deal {
        Deal::Bought => Some(worth),
        Deal::Sold => piece.bought.then(|| pays(worth, pays_pct)),
    }
}

/// What `count` of `piece` would come to going the way `deal` says, or why
/// it would be refused, when whoever pays carries `purse`.
///
/// Refused for the first of: worn, sold and not bought, no worth, a short
/// purse. The price saturates, so a count no purse could meet is short
/// rather than wrapped into a bargain.
pub fn quote(deal: Deal, pays_pct: u32, piece: Piece, count: u32, purse: u32) -> Result<u32, TradeRefusal> {
    if piece.worn {
        return Err(TradeRefusal::Worn);
    }
    if deal == Deal::Sold && !piece.bought {
        return Err(TradeRefusal::NotBought);
    }
    let Some(each) = each(deal, pays_pct, piece) else { return Err(TradeRefusal::NoWorth) };
    let price = each.saturating_mul(count);
    if purse < price { Err(TradeRefusal::Short) } else { Ok(price) }
}

/// What to take from each of `pieces`, given as each with how much it
/// holds in the order carried, to make `amount`: whole pieces first and the
/// last split. Takes what there is, up to `amount`, and no more.
pub fn draw_down<I: Copy>(pieces: &[(I, u32)], amount: u32) -> Vec<(I, u32)> {
    let mut left = amount;
    let mut taken = Vec::new();
    for &(piece, held) in pieces {
        if left == 0 {
            break;
        }
        let take = left.min(held);
        taken.push((piece, take));
        left -= take;
    }
    taken
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEPT: Piece = Piece { worth: Some(7), bought: true, worn: false };

    #[test]
    fn a_sale_pays_the_share_of_each_ones_worth_rounded_down() {
        assert_eq!(each(Deal::Sold, 50, KEPT), Some(3), "half of seven is three, not four");
        assert_eq!(each(Deal::Sold, 100, KEPT), Some(7));
        assert_eq!(each(Deal::Sold, 0, KEPT), Some(0), "a share of nought is a price of nought, not a refusal");
        assert_eq!(quote(Deal::Sold, 50, KEPT, 3, 9), Ok(9), "three at three each, the same as selling them one at a time");
    }

    #[test]
    fn a_purchase_costs_the_whole_worth_whatever_the_traders_share() {
        assert_eq!(each(Deal::Bought, 50, KEPT), Some(7));
        let unbought = Piece { bought: false, ..KEPT };
        assert_eq!(each(Deal::Bought, 50, unbought), Some(7), "what a trader buys has no bearing on what it sells");
    }

    #[test]
    fn a_thing_with_no_worth_or_sold_of_a_kind_not_bought_has_no_price() {
        assert_eq!(each(Deal::Bought, 50, Piece { worth: None, ..KEPT }), None);
        assert_eq!(each(Deal::Sold, 50, Piece { worth: None, ..KEPT }), None);
        assert_eq!(each(Deal::Sold, 50, Piece { bought: false, ..KEPT }), None);
    }

    #[test]
    fn a_worn_thing_still_has_a_price_but_is_refused() {
        let worn = Piece { worn: true, ..KEPT };
        assert_eq!(each(Deal::Bought, 50, worn), Some(7));
        assert_eq!(quote(Deal::Bought, 50, worn, 1, 100), Err(TradeRefusal::Worn));
    }

    #[test]
    fn a_deal_is_refused_for_the_first_reason_in_order_worn_then_not_bought_then_no_worth_then_short() {
        let all = Piece { worth: None, bought: false, worn: true };
        assert_eq!(quote(Deal::Sold, 50, all, 1, 0), Err(TradeRefusal::Worn));
        assert_eq!(quote(Deal::Sold, 50, Piece { worn: false, ..all }, 1, 0), Err(TradeRefusal::NotBought));
        assert_eq!(quote(Deal::Bought, 50, Piece { worn: false, ..all }, 1, 0), Err(TradeRefusal::NoWorth));
        assert_eq!(quote(Deal::Sold, 50, Piece { worth: None, bought: true, worn: false }, 1, 0), Err(TradeRefusal::NoWorth));
        assert_eq!(quote(Deal::Bought, 50, KEPT, 2, 13), Err(TradeRefusal::Short), "fourteen asked of thirteen");
    }

    #[test]
    fn a_purse_that_meets_the_price_exactly_pays_it_and_a_huge_count_saturates_instead_of_wrapping() {
        assert_eq!(quote(Deal::Bought, 50, KEPT, 2, 14), Ok(14));
        assert_eq!(quote(Deal::Bought, 50, KEPT, u32::MAX, u32::MAX - 1), Err(TradeRefusal::Short));
        assert_eq!(quote(Deal::Bought, 50, KEPT, u32::MAX, u32::MAX), Ok(u32::MAX));
        assert_eq!(
            each(Deal::Sold, u32::MAX, Piece { worth: Some(u32::MAX), ..KEPT }),
            Some(u32::MAX),
            "a share past a hundred is held to what a number holds"
        );
    }

    #[test]
    fn drawing_down_takes_pieces_in_the_order_carried_and_splits_the_last() {
        assert_eq!(draw_down(&[('a', 4), ('b', 5), ('c', 9)], 7), [('a', 4), ('b', 3)]);
        assert_eq!(draw_down(&[('a', 4), ('b', 5)], 4), [('a', 4)], "an exact fit stops at the piece it empties");
    }

    #[test]
    fn drawing_down_more_than_is_held_takes_all_of_it_and_nought_takes_nothing() {
        assert_eq!(draw_down(&[('a', 4), ('b', 5)], 100), [('a', 4), ('b', 5)]);
        assert_eq!(draw_down(&[('a', 4)], 0), []);
        assert_eq!(draw_down::<char>(&[], 3), []);
    }
}
