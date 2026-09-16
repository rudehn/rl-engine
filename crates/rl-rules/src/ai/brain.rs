//! The brain: tactics in priority order.

use std::any::Any;
use std::fmt::Debug;

use rand::rngs::StdRng;
use rl_core::{Point, Rect};

use crate::ability::AbilityId;

use crate::ai::snapshot::Snapshot;

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
    /// Take everything lying where it stands.
    PickUp,
    /// Take the item lying where it stands and put it on, in one action.
    EquipFromGround(A),
    /// Throw a carried item at a cell.
    Throw {
        /// What.
        item: A,
        /// Where it is aimed.
        at: Point,
    },
    /// Something of the game's own. The engine carries it back to the
    /// game, which turns it into the action it stands for, so a game's
    /// tactic can sit anywhere in the priority list beside the engine's.
    Own(Box<dyn Choice>),
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
            (Decision::Wait, Decision::Wait) | (Decision::PickUp, Decision::PickUp) => true,
            (Decision::EquipFromGround(a), Decision::EquipFromGround(b)) => a == b,
            (Decision::Throw { item: a, at: p }, Decision::Throw { item: b, at: q }) => a == b && p == q,
            (Decision::Own(a), Decision::Own(b)) => a.name() == b.name(),
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

    /// Decides, and says which tactic decided. `None` for the name means
    /// nothing fired and the actor waits.
    pub fn decide(&self, ctx: &mut TacticCtx<'_, A>) -> (Decision<A>, Option<&'static str>) {
        for t in &self.tactics {
            if let Some(d) = t.evaluate(ctx) {
                return (d, Some(t.name()));
            }
        }
        (Decision::Wait, None)
    }

    /// The tactic names in priority order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.tactics.iter().map(|t| t.name())
    }
}
