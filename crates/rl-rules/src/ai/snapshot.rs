//! What an actor knows this turn.

use crate::FactionId;
use crate::ability::Usable;
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
}

impl<A: Copy> Snapshot<A> {
    /// A snapshot with nothing in sight.
    pub fn alone(me: ActorView<A>) -> Self {
        Self { me, enemies: Vec::new(), allies: Vec::new(), came_from: None, usable: Vec::new(), last_known: None }
    }

    /// Sorts enemies and allies nearest first, ties by position, so two
    /// runs agree on who is "nearest".
    pub fn sort(&mut self) {
        let me = self.me.pos;
        let key = |v: &ActorView<A>| (geometry::chebyshev(me, v.pos), v.pos);
        self.enemies.sort_by_key(key);
        self.allies.sort_by_key(key);
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
