//! The brain: tactics in priority order.

use std::any::Any;
use std::fmt::Debug;

use atglyph_core::{Point, Rect};
use rand::rngs::StdRng;

use crate::ability::AbilityId;
use crate::work::Work;

use crate::ai::snapshot::Snapshot;
use crate::ai::tactics::Station;

/// An action of the game's own, chosen by a tactic of the game's own.
///
/// Any type: the engine carries it back to the game in a box and the game
/// finds it again by type, so nothing is numbered and two games' choices
/// cannot collide. In the Bevy layer a type that is both an action and a
/// choice is routed to its own intent by `add_choice`, so the game writes
/// the tactic and nothing else. `name` is for the trace, and for
/// comparing two decisions in a test.
pub trait Choice: Any + Debug + Send + Sync {
    /// A stable name.
    fn name(&self) -> &'static str;
}

/// What an actor decided to do.
///
/// `PartialEq` compares a game's choice by its name, since a boxed trait
/// has no equality of its own and a test wants to say what was decided.
#[derive(Debug)]
pub enum Decision<A: Copy> {
    /// Step to an adjacent cell.
    Step(Point),
    /// Attack an adjacent actor.
    Attack(A),
    /// Use an ability, pointed at a cell. One of the engine's own, the
    /// way an attack is: the resolver that owns abilities answers it.
    Ability {
        /// Which.
        ability: AbilityId,
        /// Where it is pointed.
        aim: Point,
    },
    /// Do nothing this turn.
    Wait,
    /// Go through the way it stands on, to wherever that leads.
    GoThrough,
    /// Take everything lying where it stands.
    PickUp,
    /// Take the item lying where it stands and put it on, in one action.
    EquipFromGround(A),
    /// Put on an item it carries. One of the engine's own, answered by the
    /// resolver that answers the player's, so the slot it goes in and
    /// whatever it pushes off are the one rule for every wearer. When a
    /// mind arms is the game's: no engine tactic decides it.
    Equip(A),
    /// Throw a carried item at a cell.
    Throw {
        /// What.
        item: A,
        /// Where it is aimed.
        at: Point,
    },
    /// Use a carried item where it stands, on itself. One of the engine's
    /// own, answered by the resolver that answers the player's use of the
    /// same thing, so a mind is refused whatever the player would be.
    UseItem(A),
    /// Use a carried item whose use is pointed, at a cell: the sibling of
    /// [`UseItem`](Decision::UseItem), as the actions they stand for are.
    UseItemAt {
        /// What.
        item: A,
        /// Where it is pointed.
        at: Point,
    },
    /// Something of the game's own. The engine carries it back to the
    /// game, which turns it into the action it stands for, so a game's
    /// tactic can sit anywhere in the priority list beside the engine's.
    Own(Box<dyn Choice>),
    /// Begin work of many turns. The engine spends this turn as its first
    /// and every turn of the actor's after it on the rest, without asking
    /// the brain again, until it is done or broken.
    Work(Work<A>),
}

impl<A: Copy> Decision<A> {
    /// A decision of the game's own.
    pub fn own(choice: impl Choice) -> Self {
        Self::Own(Box::new(choice))
    }
}

impl<A: Copy + PartialEq> PartialEq for Decision<A> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Decision::Step(a), Decision::Step(b)) => a == b,
            (Decision::Attack(a), Decision::Attack(b)) => a == b,
            (Decision::Ability { ability: a, aim: p }, Decision::Ability { ability: b, aim: q }) => a == b && p == q,
            (Decision::Wait, Decision::Wait) | (Decision::PickUp, Decision::PickUp) | (Decision::GoThrough, Decision::GoThrough) => true,
            (Decision::EquipFromGround(a), Decision::EquipFromGround(b)) => a == b,
            (Decision::Equip(a), Decision::Equip(b)) => a == b,
            (Decision::Throw { item: a, at: p }, Decision::Throw { item: b, at: q }) => a == b && p == q,
            (Decision::UseItem(a), Decision::UseItem(b)) => a == b,
            (Decision::UseItemAt { item: a, at: p }, Decision::UseItemAt { item: b, at: q }) => a == b && p == q,
            (Decision::Own(a), Decision::Own(b)) => a.name() == b.name(),
            (Decision::Work(a), Decision::Work(b)) => a == b,
            _ => false,
        }
    }
}

/// The way toward, or away from, a set of cells, answered by whoever owns
/// the map.
///
/// A tactic never sees a flow field. It names the cells it wants to reach
/// or leave, and gets back the neighbouring cells that go that way, best
/// first, so the caller may build fields lazily, share one flood between
/// every mind that wants the same cells, and keep them in whatever
/// coordinates it likes. Fifty hunters after one player cost one flood.
pub trait Fields {
    /// The cells next to `from` that step down toward `goals`, best first;
    /// empty when there is no way or nothing to build a field over.
    fn descents_toward(&mut self, goals: &[Point], from: Point) -> Vec<Point>;

    /// The cells next to `from` that step away from `goals`, on a map that
    /// leads away from them and round corners rather than into them.
    fn descents_away(&mut self, goals: &[Point], from: Point) -> Vec<Point>;

    /// How far `from` is from the nearest of `goals` on foot, in hundredths
    /// of a step, the unit every cost is in: the value the field toward
    /// `goals` holds at `from`, so asking is free once a mind has walked
    /// down that field. The walk itself, never what a field adds to steer
    /// a mind, so a wary mind's price on a shown trap is no part of it and
    /// is answered from an unpriced field. `None` when there is no way, or
    /// no field at all, and a tactic then measures as the crow flies.
    ///
    /// Its own question rather than a count of descents, because the walk
    /// is what a follower loses time over: three cells off across a wall
    /// with a gap in it is a walk of five, and a diagonal stride costs a
    /// straight one and two fifths. `None` by default, so a game that does
    /// no pathing measures every gap as the crow flies.
    fn on_foot(&mut self, goals: &[Point], from: Point) -> Option<u32> {
        let _ = (goals, from);
        None
    }

    /// Whether a diagonal step may cut past the corner at `p`, by the rule
    /// the move resolver refuses one by, asked of both cells a diagonal
    /// squeezes between by every tactic that picks a neighbour itself.
    ///
    /// Its own question rather than [`TacticCtx::can_step`], because the
    /// two part at a closed door: a mind that opens doors may step into
    /// one, so it is a step, and no diagonal goes past one, so it is not a
    /// corner. Asked `can_step` alone, such a mind chose the diagonal past
    /// a door's corner, was refused it, and chose it again every turn.
    /// True by default, which leaves the corner to `can_step`, the rule for
    /// a mover that opens nothing; whoever owns a map with doors answers it
    /// from the map.
    fn lets_diagonal_past(&self, p: Point) -> bool {
        let _ = p;
        true
    }
}

/// No fields at all: every tactic falls back to stepping straight at or
/// away from what it sees. For a test, or a game that does no pathing.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFields;

impl Fields for NoFields {
    fn descents_toward(&mut self, _: &[Point], _: Point) -> Vec<Point> {
        Vec::new()
    }

    fn descents_away(&mut self, _: &[Point], _: Point) -> Vec<Point> {
        Vec::new()
    }
}

/// Everything a tactic may consult.
pub struct TacticCtx<'a, A: Copy> {
    /// What the actor knows.
    pub snapshot: &'a Snapshot<A>,
    /// The way toward or away from any cells, for this actor's movement
    /// class.
    pub fields: &'a mut dyn Fields,
    /// Whether `p` can be stepped onto right now: walkable and unoccupied.
    pub can_step: &'a dyn Fn(Point) -> bool,
    /// Whether `p` stops a projectile: a wall, or somebody standing.
    /// The same predicate the ability resolver uses, so what a tactic
    /// thinks an ability will cover is what it does cover.
    pub blocks_shot: &'a dyn Fn(Point) -> bool,
    /// Whether `p` stops a burst: a wall, and never somebody standing,
    /// whom a burst catches rather than stops at. The same predicate the
    /// ability resolver bursts by.
    pub blocks_burst: &'a dyn Fn(Point) -> bool,
    /// The tiles a shape may be resolved within.
    pub bounds: Rect,
    /// This actor's stream for the turn.
    pub rng: &'a mut StdRng,
}

impl<A: Copy> TacticCtx<'_, A> {
    /// The first cell the actor can step onto that leads toward `goals`,
    /// down the field, or `None` when there is no field or no free step.
    pub fn step_toward(&mut self, goals: &[Point]) -> Option<Point> {
        let me = self.snapshot.me.pos;
        self.fields.descents_toward(goals, me).into_iter().find(|p| (self.can_step)(*p))
    }

    /// The first cell the actor can step onto that leads away from `goals`.
    pub fn step_away_from(&mut self, goals: &[Point]) -> Option<Point> {
        let me = self.snapshot.me.pos;
        self.fields.descents_away(goals, me).into_iter().find(|p| (self.can_step)(*p))
    }
}

/// One way an actor might spend its turn.
pub trait Tactic<A: Copy>: Send + Sync {
    /// A stable name, for the trace of why an actor did something.
    fn name(&self) -> &'static str;

    /// A decision, or `None` to let the next tactic try.
    fn evaluate(&self, ctx: &mut TacticCtx<'_, A>) -> Option<Decision<A>>;

    /// Whose company a turn decided by this tactic keeps: the enemies it
    /// is closing on or watching, or the allies it stays beside. `None`
    /// for one that is about nobody, or about getting away.
    ///
    /// Read when one of them leaves by a way to another map, so a mind
    /// that travels goes after what it was after and not after what it
    /// was running from. A game's own tactic that hunts says so here.
    fn keeps_with(&self) -> Option<Station> {
        None
    }
}

/// An ordered list of tactics; the first that decides, wins.
pub struct Brain<A: Copy> {
    tactics: Vec<Box<dyn Tactic<A>>>,
}

impl<A: Copy> Default for Brain<A> {
    fn default() -> Self {
        Self { tactics: Vec::new() }
    }
}

impl<A: Copy> std::fmt::Debug for Brain<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.tactics.iter().map(|t| t.name())).finish()
    }
}

impl<A: Copy> Brain<A> {
    /// An empty brain, which always waits.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a tactic at the lowest priority so far.
    pub fn then(mut self, tactic: impl Tactic<A> + 'static) -> Self {
        self.tactics.push(Box::new(tactic));
        self
    }

    /// Appends `tactics`, in order, at the lowest priority so far.
    pub(crate) fn with_all(mut self, tactics: Vec<Box<dyn Tactic<A>>>) -> Self {
        self.tactics.extend(tactics);
        self
    }

    /// Decides, and says which tactic decided. `None` for the name means
    /// nothing fired and the actor waits.
    pub fn decide(&self, ctx: &mut TacticCtx<'_, A>) -> (Decision<A>, Option<&'static str>) {
        match self.deciding(ctx) {
            Some((decision, tactic)) => (decision, Some(tactic.name())),
            None => (Decision::Wait, None),
        }
    }

    /// Decides, and hands back the tactic that decided, for a caller that
    /// wants more of it than its name. `None` when nothing fired.
    pub fn deciding(&self, ctx: &mut TacticCtx<'_, A>) -> Option<(Decision<A>, &dyn Tactic<A>)> {
        self.tactics.iter().find_map(|t| t.evaluate(ctx).map(|d| (d, &**t)))
    }

    /// Whether any tactic here keeps the mind beside its allies, whichever
    /// tactic decides a given turn: a companion idling inside the distance
    /// it keeps is still a companion. A follower of one leader counts, since
    /// its leader is one of its allies and its allies are whom it keeps
    /// beside once the leader is gone.
    pub fn keeps_with_allies(&self) -> bool {
        self.tactics.iter().any(|t| matches!(t.keeps_with(), Some(Station::Allies | Station::Leader)))
    }

    /// The tactic names in priority order.
    #[cfg(test)]
    pub(crate) fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.tactics.iter().map(|t| t.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::work::WorkKindId;

    #[test]
    fn two_decisions_to_work_are_equal_only_when_the_work_is() {
        let kind = WorkKindId::from_raw(0);
        let a: Decision<u32> = Decision::Work(Work::new(kind, 4).on(7));
        assert_eq!(a, Decision::Work(Work::new(kind, 4).on(7)));
        assert_ne!(a, Decision::Work(Work::new(kind, 5).on(7)));
        assert_ne!(a, Decision::Work(Work::new(kind, 4).on(8)));
    }
}
