//! The brain: tactics in priority order.

use rand::rngs::StdRng;
use rl_core::{Point, Rect};
use rl_grid::DijkstraMap;

use crate::ability::AbilityId;

use crate::ai::snapshot::Snapshot;

/// What an actor decided to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Something of the game's own, in the game's own numbering, the way
    /// a map's spots are tagged. The engine carries the number back to
    /// the game and lets it decide what the actor actually does, so a
    /// game's tactic can sit anywhere in the priority list beside the
    /// engine's.
    Game(u32),
}

/// Everything a tactic may consult.
pub struct TacticCtx<'a, A: Copy> {
    /// What the actor knows.
    pub snapshot: &'a Snapshot<A>,
    /// Costs to the nearest enemy for this actor's movement class, if the
    /// caller built one. Descending it approaches.
    pub approach: Option<&'a DijkstraMap>,
    /// The safety map for this class, if built. Descending it escapes.
    pub escape: Option<&'a DijkstraMap>,
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
