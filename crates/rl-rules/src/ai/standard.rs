//! The standard chain: a brain assembled from what one kind can do, in
//! one order, with only the engine's own tactics and only those a game
//! asks for.
//!
//! Every game with monsters wrote the same chain by hand: run when hurt,
//! strike what is beside it, shoot what is not, hunt what it sees, search
//! where it lost it, walk back to its post and drift, each guarded by
//! whether the kind can. [`Standard`] is that chain as a game's choices:
//! which steps its chain holds, and every number a step needs, the chance
//! of reaching for an ability, of throwing, of a wanderer's step, and how
//! far a scavenger goes. [`Repertoire`] is what one kind can do, read from
//! its parts. [`Standard::brain`] puts the two together in this order:
//!
//! 1. whatever a game puts [`first`](StandardBrain::first);
//! 2. [`FleeWhenHurt`], for a kind that runs, unless the chain flees
//!    after its attacks;
//! 3. [`Keep::enemies`], for a kind that keeps a span from its enemies;
//! 4. [`UseAbility`], for a kind that knows one;
//! 5. [`MeleeAdjacent`], for a kind with a blow;
//! 6. [`ShootAtRange`], for a kind with a shot;
//! 7. [`ThrowAtRange`], for a kind whose wits throw;
//! 8. [`FleeWhenHurt`] here instead, when the chain flees after its
//!    attacks;
//! 9. [`Scavenge`], for a kind whose wits take things up;
//! 10. whatever a game puts [`before_hunt`](StandardBrain::before_hunt);
//! 11. [`Hunt`], or whatever a game puts
//!     [`instead_of_hunt`](StandardBrain::instead_of_hunt);
//! 12. [`SearchLastKnown`];
//! 13. [`KeepPost`];
//! 14. whatever a game puts [`before_wander`](StandardBrain::before_wander);
//! 15. [`Wander`].
//!
//! A step is in the brain only when the chain holds it and the kind can
//! take it, so a kind that never runs has no `FleeWhenHurt` in its brain
//! rather than one that checks a threshold of nothing every turn. Nothing
//! is held unless the game says: [`Standard::NONE`] is an empty chain, and
//! the engine holds no number of its own for any step, so a step whose
//! number a game never gave is not in its chain whatever its kinds can
//! do. Running first is the order a kind said to run when hurt keeps its
//! word by; a game whose kinds trade a blow before they run says so with
//! [`Standard::flee_after_attacks`].
//!
//! Using it is optional: a game that wants another order builds its
//! [`Brain`] by hand, as it always could.

use crate::ai::brain::{Brain, Tactic};
use crate::ai::tactics::{FleeWhenHurt, Hunt, Keep, KeepPost, MeleeAdjacent, Scavenge, SearchLastKnown, ShootAtRange, ThrowAtRange, UseAbility, Wander};
use crate::ai::wits::Wits;
use crate::parts::KeepAway;

/// What one kind of actor can do, as a chain reads it: what switches each
/// step of [`Standard`] on for that kind.
///
/// Read from a kind's parts by the layer that holds them, or written by
/// hand by a game that keeps none. Nothing by default: a kind that never
/// runs, keeps no span, knows no ability, strikes nothing, shoots nothing
/// and is mindless, so no step is on for it until a field says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repertoire {
    /// Percent health at or below which it runs; nought never runs.
    pub flee_at: i32,
    /// The span it keeps from its enemies.
    pub keep_away: Option<KeepAway>,
    /// Whether it knows any ability.
    pub abilities: bool,
    /// Whether it has a blow.
    pub melee: bool,
    /// Whether it has a shot.
    pub ranged: bool,
    /// What its mind is able to do, which says whether it throws and
    /// whether it takes things up.
    pub wits: Wits,
}

impl Default for Repertoire {
    fn default() -> Self {
        Self { flee_at: 0, keep_away: None, abilities: false, melee: false, ranged: false, wits: Wits::MINDLESS }
    }
}

/// Which steps of the standard chain a game's brains hold, and every
/// number a step needs: a game's choices, written once as a constant and
/// read for every kind.
///
/// Every field is off unless written, and [`NONE`](Self::NONE) is the
/// chain with nothing in it. A number is the game's rule, never the
/// engine's: how often a caster reaches for its ability, a thrower throws
/// and a wanderer steps, and how far a scavenger goes, so a step that needs
/// one is in the chain only once the game gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standard {
    /// [`FleeWhenHurt`] at the kind's own threshold.
    pub flee: bool,
    /// Whether it flees after its attacks rather than before them: a kind
    /// that trades a blow with what stands beside it before it runs.
    pub flee_after_attacks: bool,
    /// [`Keep::enemies`] at the kind's own span.
    pub keep_away: bool,
    /// [`UseAbility`], at this chance in percent on a turn one is ready.
    pub ability_chance: Option<u32>,
    /// [`MeleeAdjacent`].
    pub melee: bool,
    /// [`ShootAtRange`], on every turn there is a shot to take.
    pub shoot: bool,
    /// [`ThrowAtRange`], at this chance in percent on a turn a throw is
    /// there to be made.
    pub throw_chance: Option<u32>,
    /// [`Scavenge`], no further than this many steps.
    pub scavenge_reach: Option<i32>,
    /// [`Hunt`], unless a game puts something in its place.
    pub hunt: bool,
    /// [`SearchLastKnown`].
    pub search: bool,
    /// [`KeepPost`].
    pub keep_post: bool,
    /// [`Wander`], a step at this chance in percent on a turn it has
    /// nothing better to do.
    pub wander_chance: Option<u32>,
}

impl Standard {
    /// The chain with nothing in it, which a game's own writes over:
    /// `Standard { melee: true, hunt: true, ..Standard::NONE }`.
    pub const NONE: Standard = Standard {
        flee: false,
        flee_after_attacks: false,
        keep_away: false,
        ability_chance: None,
        melee: false,
        shoot: false,
        throw_chance: None,
        scavenge_reach: None,
        hunt: false,
        search: false,
        keep_post: false,
        wander_chance: None,
    };

    /// The brain this chain makes for a kind that can do what `can` says,
    /// ready for a game's own tactics at the points the
    /// [module docs](self) name, and then [`build`](StandardBrain::build).
    pub fn brain<A: Copy + 'static>(&self, can: Repertoire) -> StandardBrain<A> {
        StandardBrain { chain: *self, can, first: Vec::new(), before_hunt: Vec::new(), instead_of_hunt: Vec::new(), before_wander: Vec::new() }
    }
}

impl Default for Standard {
    fn default() -> Self {
        Self::NONE
    }
}

/// A standard chain for one kind, with a game's own tactics at the points
/// it names, before it is built into a [`Brain`].
///
/// Each point keeps the order its tactics were given in.
pub struct StandardBrain<A: Copy> {
    chain: Standard,
    can: Repertoire,
    first: Vec<Box<dyn Tactic<A>>>,
    before_hunt: Vec<Box<dyn Tactic<A>>>,
    instead_of_hunt: Vec<Box<dyn Tactic<A>>>,
    before_wander: Vec<Box<dyn Tactic<A>>>,
}

impl<A: Copy + 'static> StandardBrain<A> {
    /// `tactic` ahead of everything, running included: what nothing else a
    /// kind would do outranks, raising an alarm not yet raised or making
    /// off with what it stole.
    pub fn first(mut self, tactic: impl Tactic<A> + 'static) -> Self {
        self.first.push(Box::new(tactic));
        self
    }

    /// `tactic` after every attack and before the hunt: what a kind does
    /// once nothing is in reach, keeping beside its pack or its leader.
    pub fn before_hunt(mut self, tactic: impl Tactic<A> + 'static) -> Self {
        self.before_hunt.push(Box::new(tactic));
        self
    }

    /// `tactic` where [`Hunt`] would be, which is then left out: what a
    /// kind does with what it sees instead of closing on it, rebuilding a
    /// wreck or hanging at the span it keeps. Several go in the order
    /// given.
    pub fn instead_of_hunt(mut self, tactic: impl Tactic<A> + 'static) -> Self {
        self.instead_of_hunt.push(Box::new(tactic));
        self
    }

    /// `tactic` after the post is kept and before the drift: a route of
    /// the kind's own to walk when it has nothing else to do.
    pub fn before_wander(mut self, tactic: impl Tactic<A> + 'static) -> Self {
        self.before_wander.push(Box::new(tactic));
        self
    }

    /// The brain, in the order the [module docs](self) give.
    pub fn build(self) -> Brain<A> {
        let StandardBrain { chain, can, first, before_hunt, instead_of_hunt, before_wander } = self;
        let mut brain = Brain::new().with_all(first);
        let flee = (chain.flee && can.flee_at > 0).then_some(FleeWhenHurt { at_pct: can.flee_at });
        if let Some(flee) = flee.filter(|_| !chain.flee_after_attacks) {
            brain = brain.then(flee);
        }
        if let Some(keep) = can.keep_away.filter(|_| chain.keep_away) {
            brain = brain.then(Keep::enemies(keep.within, keep.no_closer));
        }
        if let Some(chance_pct) = chain.ability_chance.filter(|_| can.abilities) {
            brain = brain.then(UseAbility { chance_pct });
        }
        if chain.melee && can.melee {
            brain = brain.then(MeleeAdjacent);
        }
        if chain.shoot && can.ranged {
            brain = brain.then(ShootAtRange::default());
        }
        if let Some(chance_pct) = chain.throw_chance.filter(|_| can.wits.has(Wits::THROWS)) {
            brain = brain.then(ThrowAtRange { chance_pct });
        }
        if let Some(flee) = flee.filter(|_| chain.flee_after_attacks) {
            brain = brain.then(flee);
        }
        if let Some(reach) = chain.scavenge_reach.filter(|_| can.wits.takes_things()) {
            brain = brain.then(Scavenge { reach });
        }
        brain = brain.with_all(before_hunt);
        if !instead_of_hunt.is_empty() {
            brain = brain.with_all(instead_of_hunt);
        } else if chain.hunt {
            brain = brain.then(Hunt);
        }
        if chain.search {
            brain = brain.then(SearchLastKnown);
        }
        if chain.keep_post {
            brain = brain.then(KeepPost);
        }
        brain = brain.with_all(before_wander);
        if let Some(chance_pct) = chain.wander_chance {
            brain = brain.then(Wander { chance_pct });
        }
        brain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::tactics::Hover;

    /// A chain holding every step, at numbers of a test's own.
    const EVERY: Standard = Standard {
        flee: true,
        flee_after_attacks: false,
        keep_away: true,
        ability_chance: Some(40),
        melee: true,
        shoot: true,
        throw_chance: Some(50),
        scavenge_reach: Some(4),
        hunt: true,
        search: true,
        keep_post: true,
        wander_chance: Some(30),
    };

    /// A kind that can do everything a step asks of one.
    fn capable() -> Repertoire {
        Repertoire { flee_at: 50, keep_away: Some(KeepAway { within: 5, no_closer: 3 }), abilities: true, melee: true, ranged: true, wits: Wits::SAPIENT }
    }

    fn names(brain: &Brain<u32>) -> Vec<&'static str> {
        brain.names().collect()
    }

    #[test]
    fn every_step_a_capable_kind_can_take_comes_in_the_standard_order() {
        let brain: Brain<u32> = EVERY.brain(capable()).build();
        assert_eq!(
            names(&brain),
            [
                "flee_when_hurt",
                "shadow",
                "use_ability",
                "melee_adjacent",
                "shoot_at_range",
                "throw_at_range",
                "scavenge",
                "hunt",
                "search_last_known",
                "keep_post",
                "wander"
            ]
        );
    }

    #[test]
    fn a_step_is_in_only_when_the_chain_holds_it_and_the_kind_can_take_it() {
        let empty: Brain<u32> = Standard::NONE.brain(capable()).build();
        assert!(names(&empty).is_empty(), "nothing the game did not ask for: {:?}", names(&empty));
        let plain: Brain<u32> = EVERY.brain(Repertoire { melee: true, ..Repertoire::default() }).build();
        assert_eq!(names(&plain), ["melee_adjacent", "hunt", "search_last_known", "keep_post", "wander"], "a mindless biter that never runs");
    }

    #[test]
    fn a_chain_that_flees_after_its_attacks_trades_its_blows_first() {
        let chain = Standard { flee_after_attacks: true, ..EVERY };
        let brain: Brain<u32> = chain.brain(Repertoire { flee_at: 30, melee: true, ranged: true, ..Repertoire::default() }).build();
        assert_eq!(names(&brain), ["melee_adjacent", "shoot_at_range", "flee_when_hurt", "hunt", "search_last_known", "keep_post", "wander"]);
    }

    #[test]
    fn a_games_own_tactics_land_at_the_points_it_names_and_one_in_place_of_the_hunt_leaves_the_hunt_out() {
        let brain: Brain<u32> = EVERY
            .brain(Repertoire { melee: true, ..Repertoire::default() })
            .first(Wander { chance_pct: 1 })
            .before_hunt(Keep::allies(3, 1))
            .instead_of_hunt(Keep::enemies(6, 4))
            .instead_of_hunt(Hover)
            .before_wander(KeepPost)
            .build();
        assert_eq!(names(&brain), ["wander", "melee_adjacent", "follow", "shadow", "hover", "search_last_known", "keep_post", "keep_post", "wander"]);
    }
}
