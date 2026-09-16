//! The components engine systems read and write.

use bevy::prelude::*;
use rl_core::Point;
use rl_grid::BitGrid;

/// Where an entity stands, in world tile coordinates.
///
/// Authoritative. The render layer derives transforms from it; nothing
/// writes a transform for a grid entity directly.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Deref, DerefMut)]
pub struct Position(pub Point);

impl Position {
    /// A position at `(x, y)`.
    pub const fn new(x: i32, y: i32) -> Self {
        Self(Point::new(x, y))
    }
}

/// Takes turns.
///
/// Requires [`Speed`], at its normal hundred, so an actor spawned without
/// one moves at normal speed rather than leaving every reader to decide
/// what a missing speed means.
#[derive(Component, Debug, Clone, Copy, Default)]
#[require(Speed)]
pub struct Actor;

/// The one actor the game waits on for input.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Player;

/// Occupies its cell: nothing else that blocks may enter it.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Blocks;

/// Speed as a percentage of normal: 100 is normal, 200 twice as fast.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Speed(pub u32);

impl Default for Speed {
    fn default() -> Self {
        Self(100)
    }
}

/// Worn by the actor whose turn it is. While an actor wears this it is out
/// of the queue; the scheduler puts it back when its action is done.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct MyTurn;

/// Marks the actor whose sight fills in the explored map.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct RevealsMap;

/// What an actor can see.
///
/// `line` is every tile the actor has an unobstructed line to; `visible`
/// is what it actually sees, which is the same set unless the game has
/// turned lighting on, and then only what is lit, within its dark sight
/// or adjacent. Both cover the loaded window, not the world, and `origin`
/// says which world tile their bit 0 is. Only the FOV system clears
/// `dirty`; anything that moves the actor or changes what blocks sight
/// or light sets it.
#[derive(Component, Debug, Clone)]
pub struct Viewshed {
    /// Sight radius in tiles.
    pub range: i32,
    /// Whether `visible` is stale.
    pub dirty: bool,
    /// The map's opacity epoch it was last cast under, so a viewshed cast
    /// before a door opened is stale whether or not its owner moved.
    pub epoch: u64,
    /// The world tile at the window's top-left.
    pub origin: Point,
    /// One bit per window tile: seen.
    pub visible: BitGrid,
    /// One bit per window tile: in line of sight, lit or not.
    pub line: BitGrid,
}

impl Viewshed {
    /// A viewshed of `range` that has never been computed.
    pub fn new(range: i32) -> Self {
        Self { range, dirty: true, epoch: 0, origin: Point::ZERO, visible: BitGrid::new(0, 0), line: BitGrid::new(0, 0) }
    }

    /// Whether the world tile `p` is currently seen.
    pub fn can_see(&self, p: Point) -> bool {
        self.visible.contains(p - self.origin)
    }

    /// Whether there is an unobstructed line to the world tile `p`,
    /// whether or not it is lit.
    pub fn in_line(&self, p: Point) -> bool {
        self.line.contains(p - self.origin)
    }

    /// Every visible world tile, row-major.
    pub fn iter(&self) -> impl Iterator<Item = Point> + '_ {
        self.visible.iter().map(move |local| local + self.origin)
    }
}
