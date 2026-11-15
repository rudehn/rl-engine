//! What a use would do, and what that is worth to the mind making it.
//!
//! An effect says what it would do to whoever it lands on and to its user,
//! as a [`Impact`] each: damage or a heal in hundredths of a point, a status
//! given or taken off, nothing, or that it cannot tell. The layer that
//! holds the effects asks them before a mind decides and hands the answers
//! over on the [`Snapshot`](crate::ai::Snapshot) as a [`Prediction`] per
//! ability and actor, so the tactic that weighs them stays a plain function
//! of what it was handed.
//!
//! A [`Valuation`] turns a change into a score, by who it lands on as the
//! mind sees them, a [`Party`]: harm to a foe is worth something, harm to
//! oneself costs. The engine's own is [`StandardValuation`]; a game gives
//! a brain its own, so one caster can prize its buffs and another its
//! damage, and two of one kind can judge differently.
//!
//! Scores are in hundredths of a point of health, the unit a change is
//! written in, so a game's own valuation weighs a status against so many
//! points of damage.

use std::fmt::Debug;

use crate::FactionId;
use crate::StatusId;
use crate::ai::snapshot::{ActorView, Vitals};
use crate::damage::Resistances;

/// What a use would do to one actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impact {
    /// Takes this much health, on average, in hundredths of a point.
    Damage(i32),
    /// Gives this much back, on average, in hundredths of a point.
    Heal(i32),
    /// Puts this status on, a boon to whoever holds it or a bane.
    Status {
        /// Which.
        id: StatusId,
        /// Whether holding it is good for its holder.
        boon: bool,
    },
    /// Takes this status off, a boon it held or a bane.
    Cleanse {
        /// Which.
        id: StatusId,
        /// Whether holding it was good for its holder.
        boon: bool,
    },
    /// Nothing at all: a mend on the unhurt, a status on its holder or on
    /// one immune to it, a blow on one immune to its kind.
    Nothing,
    /// Something this cannot tell: what every effect that says nothing of
    /// itself does, and what a mind weighs by its footprint alone.
    Unknown,
}

/// What one effect would do to the actor it lands on and to its user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prospect {
    /// To whoever it lands on.
    pub on_target: Impact,
    /// To whoever used it.
    pub on_user: Impact,
}

impl Prospect {
    /// Nothing known of what it does.
    pub const UNKNOWN: Prospect = Prospect { on_target: Impact::Unknown, on_user: Impact::Unknown };

    /// Nothing done to anyone.
    pub const NOTHING: Prospect = Prospect { on_target: Impact::Nothing, on_user: Impact::Nothing };

    /// `change` to whoever it lands on, and nothing to its user.
    pub fn on(change: Impact) -> Self {
        Self { on_target: change, on_user: Impact::Nothing }
    }

    /// Whether either half is unknown.
    pub fn is_unknown(&self) -> bool {
        self.on_target == Impact::Unknown || self.on_user == Impact::Unknown
    }
}

/// One effect of an ability as a mind weighs it: the chance it lands, in
/// percent, and what it would do when it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expected {
    /// Percent chance it lands.
    pub chance: u8,
    /// What it does when it lands.
    pub outcome: Prospect,
}

/// What one ability would do to one actor the mind can see, itself
/// included, effect by effect in the ability's order.
#[derive(Debug, Clone, PartialEq)]
pub struct Prediction<A: Copy> {
    /// Which ability.
    pub ability: crate::AbilityId,
    /// On whom.
    pub on: A,
    /// Each of its effects.
    pub effects: Vec<Expected>,
}

/// Who a change lands on, as the mind making the use sees them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Party {
    /// One hostile to it.
    Foe,
    /// One allied to it.
    Ally,
    /// One neither.
    Neutral,
    /// Itself.
    Myself,
}

/// Whoever a change lands on, as a valuation reads them.
#[derive(Debug, Clone, Copy)]
pub struct Recipient<'a> {
    /// Its health, when it has any.
    pub health: Option<Vitals>,
    /// Its side, when it takes one.
    pub faction: Option<FactionId>,
    /// What it resists.
    pub resists: &'a Resistances,
    /// The statuses it holds.
    pub statuses: &'a [StatusId],
    /// The statuses it can never be given.
    pub immune_to: &'a [StatusId],
}

impl<A: Copy> ActorView<A> {
    /// This actor as a valuation reads it.
    pub fn recipient(&self) -> Recipient<'_> {
        Recipient { health: self.health, faction: self.faction, resists: &self.resists, statuses: &self.statuses, immune_to: &self.immune_to }
    }
}

/// What a change is worth to the mind making it, by who it lands on.
///
/// Positive is worth doing, negative is a cost. Each method answers one
/// kind of change; [`worth`](Self::worth) picks the one a change asks.
pub trait Valuation: Debug + Send + Sync {
    /// `hundredths` of damage to `who`.
    fn damage(&self, hundredths: i32, to: Party, who: &Recipient<'_>) -> i32;
    /// `hundredths` of health given back to `who`.
    fn heal(&self, hundredths: i32, to: Party, who: &Recipient<'_>) -> i32;
    /// Status `id` put on `who`.
    fn status(&self, id: StatusId, boon: bool, to: Party, who: &Recipient<'_>) -> i32;
    /// Status `id` taken off `who`.
    fn cleanse(&self, id: StatusId, boon: bool, to: Party, who: &Recipient<'_>) -> i32;

    /// What `change` is worth, or `None` when it is [`Impact::Unknown`].
    fn worth(&self, change: Impact, to: Party, who: &Recipient<'_>) -> Option<i32> {
        Some(match change {
            Impact::Damage(h) => self.damage(h, to, who),
            Impact::Heal(h) => self.heal(h, to, who),
            Impact::Status { id, boon } => self.status(id, boon, to, who),
            Impact::Cleanse { id, boon } => self.cleanse(id, boon, to, who),
            Impact::Nothing => 0,
            Impact::Unknown => return None,
        })
    }
}

/// What a status given or taken is worth by [`StandardValuation`], in
/// hundredths: three points of health, about one good blow early on.
pub const STATUS_WORTH: i32 = 300;

/// The engine's valuation: hurt foes and help friends, never the reverse.
///
/// Damage is worth what it takes, up to what the recipient has left, so a
/// killing blow is not worth more for overkill: plus to a foe, half again
/// as much off to an ally or the user, so a burst that catches a friend
/// must catch far more foes, and as much off to a bystander. A heal is
/// worth what it gives back, up to what the recipient is missing, and
/// more the more it is missing, up to twice as much at its last point:
/// plus to an ally or the user, minus to a foe, nothing to a bystander. A
/// status is [`STATUS_WORTH`], plus where a boon lands on a friend or a
/// bane on a foe and minus the other way round; taking one off is the same
/// with the sign turned. A bystander's boon is nothing, its bane a cost.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StandardValuation;

impl StandardValuation {
    /// What is left of `who`, in hundredths, when it has health to lose.
    fn left(who: &Recipient<'_>) -> Option<i32> {
        who.health.map(|h| h.current.max(0).saturating_mul(100))
    }

    /// What `who` is missing, in hundredths, and as a share of its most
    /// in percent, when it has health.
    fn missing(who: &Recipient<'_>) -> Option<(i32, i32)> {
        who.health.map(|h| {
            let missing = (h.max - h.current).max(0);
            let pct = if h.max <= 0 { 0 } else { (missing as i64 * 100 / h.max as i64) as i32 };
            (missing.saturating_mul(100), pct)
        })
    }

    /// Whether `to` is on the user's side: an ally or the user.
    fn friend(to: Party) -> bool {
        matches!(to, Party::Ally | Party::Myself)
    }
}

impl Valuation for StandardValuation {
    fn damage(&self, hundredths: i32, to: Party, who: &Recipient<'_>) -> i32 {
        let taken = Self::left(who).map_or(hundredths, |left| hundredths.min(left)).max(0);
        match to {
            Party::Foe => taken,
            Party::Neutral => -taken,
            Party::Ally | Party::Myself => -(taken * 3 / 2),
        }
    }

    fn heal(&self, hundredths: i32, to: Party, who: &Recipient<'_>) -> i32 {
        let Some((missing, pct)) = Self::missing(who) else { return 0 };
        let given = hundredths.min(missing).max(0);
        match to {
            Party::Foe => -given,
            Party::Neutral => 0,
            Party::Ally | Party::Myself => given * (100 + pct) / 100,
        }
    }

    fn status(&self, _: StatusId, boon: bool, to: Party, _: &Recipient<'_>) -> i32 {
        match (to, boon) {
            (Party::Neutral, true) => 0,
            (Party::Neutral, false) => -STATUS_WORTH,
            (to, boon) if Self::friend(to) == boon => STATUS_WORTH,
            _ => -STATUS_WORTH,
        }
    }

    fn cleanse(&self, id: StatusId, boon: bool, to: Party, who: &Recipient<'_>) -> i32 {
        // Taking a status off is worth what putting it on would cost.
        -self.status(id, boon, to, who)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(current: i32, max: i32) -> ActorView<u32> {
        ActorView { health: Some(Vitals { current, max }), ..ActorView::at(1, atglyph_core::Point::ZERO) }
    }

    fn worth(change: Impact, to: Party, who: &ActorView<u32>) -> i32 {
        StandardValuation.worth(change, to, &who.recipient()).unwrap()
    }

    #[test]
    fn damage_is_worth_what_it_takes_from_a_foe_and_costs_more_on_a_friend() {
        let fresh = at(10, 10);
        assert_eq!(worth(Impact::Damage(500), Party::Foe, &fresh), 500);
        assert_eq!(worth(Impact::Damage(500), Party::Ally, &fresh), -750, "half again as much off a friend");
        assert_eq!(worth(Impact::Damage(500), Party::Myself, &fresh), -750);
        assert_eq!(worth(Impact::Damage(500), Party::Neutral, &fresh), -500);
        assert_eq!(worth(Impact::Damage(500), Party::Foe, &at(2, 10)), 200, "no more than it has left");
    }

    #[test]
    fn a_heal_is_worth_more_the_more_its_recipient_is_missing_and_nothing_at_full() {
        assert_eq!(worth(Impact::Heal(500), Party::Myself, &at(10, 10)), 0, "nothing to give back");
        assert_eq!(worth(Impact::Heal(500), Party::Myself, &at(9, 10)), 110, "a point given back, at a tenth missing");
        assert_eq!(worth(Impact::Heal(500), Party::Myself, &at(2, 10)), 900, "all of it, at four fifths missing");
        assert!(worth(Impact::Heal(500), Party::Ally, &at(2, 10)) > worth(Impact::Heal(500), Party::Ally, &at(5, 10)));
        assert_eq!(worth(Impact::Heal(500), Party::Foe, &at(2, 10)), -500, "healing a foe is a cost");
        assert_eq!(worth(Impact::Heal(500), Party::Neutral, &at(2, 10)), 0);
    }

    #[test]
    fn a_status_is_worth_its_flat_amount_by_whether_it_helps_whoever_it_lands_on() {
        let (id, who) = (StatusId::from_raw(0), at(10, 10));
        let status = |boon, to| worth(Impact::Status { id, boon }, to, &who);
        let cleanse = |boon, to| worth(Impact::Cleanse { id, boon }, to, &who);
        assert_eq!((status(false, Party::Foe), status(true, Party::Foe)), (STATUS_WORTH, -STATUS_WORTH), "a bane on a foe, not a boon");
        assert_eq!((status(true, Party::Myself), status(false, Party::Ally)), (STATUS_WORTH, -STATUS_WORTH), "a boon on a friend, not a bane");
        assert_eq!((status(true, Party::Neutral), status(false, Party::Neutral)), (0, -STATUS_WORTH));
        assert_eq!((cleanse(false, Party::Ally), cleanse(true, Party::Foe)), (STATUS_WORTH, STATUS_WORTH), "a bane off a friend, a boon off a foe");
        assert_eq!(cleanse(false, Party::Foe), -STATUS_WORTH);
    }

    #[test]
    fn nothing_is_worth_nothing_and_unknown_is_unweighed() {
        let who = at(10, 10);
        assert_eq!(StandardValuation.worth(Impact::Nothing, Party::Foe, &who.recipient()), Some(0));
        assert_eq!(StandardValuation.worth(Impact::Unknown, Party::Foe, &who.recipient()), None);
        assert!(Prospect::UNKNOWN.is_unknown() && !Prospect::NOTHING.is_unknown());
        assert!(Prospect { on_target: Impact::Damage(1), on_user: Impact::Unknown }.is_unknown());
    }
}
