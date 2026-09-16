//! What an actor knows this turn.

use crate::FactionId;
use crate::ability::Usable;
use crate::ai::Wits;
use rl_core::{Point, geometry};

/// One actor as another sees it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActorView<A: Copy> {
    /// Who.
    pub id: A,
    /// Where.
    pub pos: Point,
    /// Current health.
    pub hp: i32,
    /// Maximum health.
    pub max_hp: i32,
    /// Which side.
    pub faction: FactionId,
}

impl<A: Copy> ActorView<A> {
    /// Health as a percentage.
    pub fn hp_pct(&self) -> i32 {
        if self.max_hp <= 0 { 0 } else { (self.hp as i64 * 100 / self.max_hp as i64) as i32 }
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

/// The world from one actor's point of view, built once per turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot<A: Copy> {
    /// The actor deciding.
    pub me: ActorView<A>,
    /// Hostile actors it can see, nearest first.
    pub enemies: Vec<ActorView<A>>,
    /// Friendly actors it can see, nearest first.
    pub allies: Vec<ActorView<A>>,
    /// The cell it came from last turn, if it moved.
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
    /// What lies where it can see, nearest first.
    pub items: Vec<ItemView<A>>,
}

impl<A: Copy + Ord> Snapshot<A> {
    /// Sorts enemies, allies and items nearest first, ties by position and
    /// then by identity, so two runs agree on what is "nearest" whatever
    /// order they were seen in: a pile of things on one cell is the normal
    /// case for items, and the first of them is what a scavenger takes.
    pub fn sort(&mut self) {
        let me = self.me.pos;
        let key = |v: &ActorView<A>| (geometry::chebyshev(me, v.pos), v.pos, v.id);
        self.enemies.sort_by_key(key);
        self.allies.sort_by_key(key);
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
            came_from: None,
            usable: Vec::new(),
            last_known: None,
            wits: Wits::default(),
            missiles: Vec::new(),
            items: Vec::new(),
        }
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
