//! What an actor knows this turn.

use std::any::Any;
use std::fmt::Debug;

use crate::FactionId;
use crate::ability::Usable;
use crate::ai::Wits;
use rl_core::{Point, geometry};

/// Health as a mind reads it: what is left, and the most there can be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vitals {
    /// What is left.
    pub current: i32,
    /// The most.
    pub max: i32,
}

impl Vitals {
    /// Health as a percentage.
    pub fn pct(&self) -> i32 {
        if self.max <= 0 { 0 } else { (self.current as i64 * 100 / self.max as i64) as i32 }
    }
}

/// One actor as another sees it.
///
/// Health and side are optional: a prop with a mind, a civilian in a game
/// with no combat, or anyone in a game with no factions is still someone
/// a mind can see and step round. A tactic that needs either asks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActorView<A: Copy> {
    /// Who.
    pub id: A,
    /// Where.
    pub pos: Point,
    /// Its health, when it has any to lose.
    pub health: Option<Vitals>,
    /// Which side, when it takes one.
    pub faction: Option<FactionId>,
}

impl<A: Copy> ActorView<A> {
    /// Someone with no health and no side.
    pub fn at(id: A, pos: Point) -> Self {
        Self { id, pos, health: None, faction: None }
    }

    /// Health as a percentage, when it has health.
    pub fn hp_pct(&self) -> Option<i32> {
        self.health.map(|h| h.pct())
    }

    /// Whether it is below full health. False when its health is unknown,
    /// so a mind mends nothing it cannot see the wounds of.
    pub fn is_hurt(&self) -> bool {
        self.health.is_some_and(|h| h.current < h.max)
    }
}

/// Something the actor carries that it could throw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missile<A: Copy> {
    /// The item.
    pub item: A,
    /// How far it reaches.
    pub range: i32,
}

/// Something lying where the actor can see it, as far as a mind can tell
/// what it is worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemView<A: Copy> {
    /// The item.
    pub id: A,
    /// Where it lies.
    pub pos: Point,
    /// How far it reaches if thrown, when it can be.
    pub throw_range: Option<i32>,
    /// How much better off the actor would be wearing it than wearing what
    /// it would displace, when it can wear it at all. Above zero is better.
    pub gain: Option<i32>,
}

/// Something a game knows about the world that the engine has no word
/// for, pushed onto a [`Snapshot`] for the game's own tactic to read.
///
/// A scent, a post to return to, an alarm that was raised: any type at
/// all, found again by that type, so two games' senses cannot collide and
/// nothing is numbered. `Debug` so the trace of a decision can print it.
pub trait Sense: Debug + Send + Sync {
    /// Itself, for the downcast.
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + Debug + Send + Sync> Sense for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The world from one actor's point of view, built once per turn.
///
/// Not `Clone` or `PartialEq`: a game's senses are boxed by type, and
/// nothing copies or compares a snapshot once it is built.
#[derive(Debug)]
pub struct Snapshot<A: Copy> {
    /// The actor deciding.
    pub me: ActorView<A>,
    /// Hostile actors it can see, nearest first.
    pub enemies: Vec<ActorView<A>>,
    /// Friendly actors it can see, nearest first.
    pub allies: Vec<ActorView<A>>,
    /// Everyone else it can see, nearest first: neutrals, and everyone in
    /// a game with no sides. Factions are allied with themselves, so a
    /// bystander on the actor's own side is an ally, not one of these.
    pub others: Vec<ActorView<A>>,
    /// The cell it stepped from on its last turn, if it stepped, so a
    /// wanderer does not step straight back.
    pub came_from: Option<Point>,
    /// The abilities it could use this turn, already narrowed to what it
    /// can afford. Empty for an actor with none, which is most of them.
    pub usable: Vec<Usable>,
    /// Where the freshest enemy it knows about but cannot see was last
    /// seen: what a search walks toward. `None` for an actor that is
    /// tracking nothing, and always `None` in a game without stealth.
    pub last_known: Option<Point>,
    /// What the actor is able to do. A tactic that needs a capability asks
    /// here before it decides, whatever the brain it sits in would like.
    pub wits: Wits,
    /// What it carries that it could throw. Empty for an actor with no bag,
    /// which is most of them.
    pub missiles: Vec<Missile<A>>,
    /// How far the actor's own shot carries, if it has one. Read from
    /// whatever it wields, or failing that its own, so a mind that picks up
    /// a rifle can shoot without its brain changing.
    pub reach: Option<i32>,
    /// What lies where it can see, nearest first.
    pub items: Vec<ItemView<A>>,
    /// What the game knows that the engine does not, by type.
    pub senses: Vec<Box<dyn Sense>>,
}

impl<A: Copy + Ord> Snapshot<A> {
    /// Sorts enemies, allies, others and items nearest first, ties by position and
    /// then by identity, so two runs agree on what is "nearest" whatever
    /// order they were seen in: a pile of things on one cell is the normal
    /// case for items, and the first of them is what a scavenger takes.
    pub fn sort(&mut self) {
        let me = self.me.pos;
        let key = |v: &ActorView<A>| (geometry::chebyshev(me, v.pos), v.pos, v.id);
        self.enemies.sort_by_key(key);
        self.allies.sort_by_key(key);
        self.others.sort_by_key(key);
        self.items.sort_by_key(|i| (geometry::chebyshev(me, i.pos), i.pos, i.id));
    }
}

impl<A: Copy> Snapshot<A> {
    /// A snapshot with nothing in sight, of a mind with the default wits.
    pub fn alone(me: ActorView<A>) -> Self {
        Self {
            me,
            enemies: Vec::new(),
            allies: Vec::new(),
            others: Vec::new(),
            came_from: None,
            usable: Vec::new(),
            last_known: None,
            wits: Wits::default(),
            missiles: Vec::new(),
            reach: None,
            items: Vec::new(),
            senses: Vec::new(),
        }
    }

    /// Adds something the game knows. One per type: a second of the same
    /// type replaces the first, so a contributor that runs twice says one
    /// thing.
    pub fn add_sense<S: Sense + 'static>(&mut self, sense: S) {
        // Through the box: a `Box<dyn Sense>` is itself a `Sense`, and asked
        // directly it would answer with its own type.
        self.senses.retain(|s| !(**s).as_any().is::<S>());
        self.senses.push(Box::new(sense));
    }

    /// What the game knows of type `S`, if it said.
    pub fn sense<S: 'static>(&self) -> Option<&S> {
        self.senses.iter().find_map(|s| (**s).as_any().downcast_ref::<S>())
    }

    /// The nearest visible enemy.
    pub fn nearest_enemy(&self) -> Option<&ActorView<A>> {
        self.enemies.first()
    }

    /// The enemies standing next to the actor.
    pub fn adjacent_enemies(&self) -> impl Iterator<Item = &ActorView<A>> {
        self.enemies.iter().filter(move |e| geometry::is_adjacent(self.me.pos, e.pos))
    }
}
