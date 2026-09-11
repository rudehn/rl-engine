//! The eight directions a grid is addressed in.

use serde::{Deserialize, Serialize};

use crate::point::Point;

/// A compass direction on the grid, including diagonals.
///
/// `y` grows downward, so [`Direction::North`] is negative on `y`.
/// The declaration order is clockwise from north and [`Direction::index`]
/// depends on it, so anything stored per direction is laid out the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Direction {
    /// Up.
    North,
    /// Up and right.
    NorthEast,
    /// Right.
    East,
    /// Down and right.
    SouthEast,
    /// Down.
    South,
    /// Down and left.
    SouthWest,
    /// Left.
    West,
    /// Up and left.
    NorthWest,
}

impl Direction {
    /// All eight, clockwise from north.
    pub const ALL: [Direction; 8] = [
        Direction::North,
        Direction::NorthEast,
        Direction::East,
        Direction::SouthEast,
        Direction::South,
        Direction::SouthWest,
        Direction::West,
        Direction::NorthWest,
    ];

    /// The four cardinals, clockwise from north.
    ///
    /// Anything that has to be walked along in a line wants these: a row of
    /// diagonal neighbours is a row nothing can walk between.
    pub const CARDINALS: [Direction; 4] = [Direction::North, Direction::East, Direction::South, Direction::West];

    /// The four diagonals, clockwise from north-east.
    pub const DIAGONALS: [Direction; 4] = [Direction::NorthEast, Direction::SouthEast, Direction::SouthWest, Direction::NorthWest];

    /// The one-cell step this direction represents.
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::NorthEast => (1, -1),
            Direction::East => (1, 0),
            Direction::SouthEast => (1, 1),
            Direction::South => (0, 1),
            Direction::SouthWest => (-1, 1),
            Direction::West => (-1, 0),
            Direction::NorthWest => (-1, -1),
        }
    }

    /// The one-cell step as a [`Point`].
    pub const fn offset(self) -> Point {
        let (dx, dy) = self.delta();
        Point::new(dx, dy)
    }

    /// Position in [`Direction::ALL`], for indexing anything held per direction.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// The direction at `index` in [`Direction::ALL`].
    ///
    /// # Panics
    /// Panics if `index >= 8`.
    pub const fn from_index(index: usize) -> Direction {
        Direction::ALL[index]
    }

    /// Whether the step moves on both axes.
    pub const fn is_diagonal(self) -> bool {
        let (dx, dy) = self.delta();
        dx != 0 && dy != 0
    }

    /// The direction a step corresponds to, or `None` for a zero step.
    ///
    /// Components are reduced to their signs first, so any offset maps to
    /// the direction it mostly points in.
    pub fn from_delta(dx: i32, dy: i32) -> Option<Direction> {
        let step = (dx.signum(), dy.signum());
        Direction::ALL.into_iter().find(|d| d.delta() == step)
    }

    /// The direction from `from` toward `to`, or `None` if they coincide.
    pub fn between(from: Point, to: Point) -> Option<Direction> {
        Direction::from_delta(to.x - from.x, to.y - from.y)
    }

    /// The direction facing back the way this one came.
    pub const fn opposite(self) -> Direction {
        Direction::ALL[(self.index() + 4) % 8]
    }

    /// One step clockwise (45 degrees).
    pub const fn rotate_cw(self) -> Direction {
        Direction::ALL[(self.index() + 1) % 8]
    }

    /// One step counter-clockwise (45 degrees).
    pub const fn rotate_ccw(self) -> Direction {
        Direction::ALL[(self.index() + 7) % 8]
    }

    /// The two directions square to this one, counter-clockwise first.
    pub const fn perpendicular(self) -> [Direction; 2] {
        [Direction::ALL[(self.index() + 6) % 8], Direction::ALL[(self.index() + 2) % 8]]
    }
}

/// A set of directions, held as one byte.
///
/// What a cell records about the roads through it: not "is there a road
/// here" but "which ways does it leave by". Two neighbouring cells agree
/// when each holds the [`Direction::opposite`] of the other's entry, which
/// makes a seam a checkable property.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DirectionSet(u8);

impl DirectionSet {
    /// The empty set.
    pub const NONE: DirectionSet = DirectionSet(0);

    /// Every direction.
    pub const ALL: DirectionSet = DirectionSet(u8::MAX);

    /// A set holding one direction.
    pub const fn only(direction: Direction) -> Self {
        DirectionSet(1 << direction.index())
    }

    /// A set from raw bits.
    pub const fn from_bits(bits: u8) -> Self {
        DirectionSet(bits)
    }

    /// Adds a direction.
    pub fn insert(&mut self, direction: Direction) {
        self.0 |= 1 << direction.index();
    }

    /// Removes a direction.
    pub fn remove(&mut self, direction: Direction) {
        self.0 &= !(1 << direction.index());
    }

    /// Whether the set holds a direction.
    pub const fn contains(self, direction: Direction) -> bool {
        self.0 & (1 << direction.index()) != 0
    }

    /// Whether the set is empty.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// How many directions the set holds.
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// The directions held, clockwise from north.
    pub fn iter(self) -> impl Iterator<Item = Direction> {
        Direction::ALL.into_iter().filter(move |d| self.contains(*d))
    }

    /// The raw bits.
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl FromIterator<Direction> for DirectionSet {
    fn from_iter<I: IntoIterator<Item = Direction>>(directions: I) -> Self {
        let mut set = DirectionSet::NONE;
        for d in directions {
            set.insert(d);
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_direction_is_a_distinct_single_step() {
        let deltas: std::collections::BTreeSet<(i32, i32)> = Direction::ALL.iter().map(|d| d.delta()).collect();
        assert_eq!(deltas.len(), 8);
        for d in Direction::ALL {
            let (dx, dy) = d.delta();
            assert!((dx, dy) != (0, 0));
            assert!(dx.abs() <= 1 && dy.abs() <= 1);
        }
    }

    #[test]
    fn north_is_up_on_a_screen_grid() {
        assert_eq!(Direction::North.delta(), (0, -1));
        assert_eq!(Direction::South.offset(), Point::new(0, 1));
    }

    #[test]
    fn deltas_round_trip_through_directions() {
        for d in Direction::ALL {
            let (dx, dy) = d.delta();
            assert_eq!(Direction::from_delta(dx, dy), Some(d));
        }
        assert_eq!(Direction::from_delta(0, 0), None);
        assert_eq!(Direction::from_delta(9, -4), Some(Direction::NorthEast));
    }

    #[test]
    fn between_two_points() {
        let o = Point::new(5, 5);
        assert_eq!(Direction::between(o, Point::new(5, 4)), Some(Direction::North));
        assert_eq!(Direction::between(o, Point::new(4, 6)), Some(Direction::SouthWest));
        assert_eq!(Direction::between(o, o), None);
    }

    #[test]
    fn indices_cover_every_slot_exactly_once() {
        let indices: Vec<usize> = Direction::ALL.iter().map(|d| d.index()).collect();
        assert_eq!(indices, (0..8).collect::<Vec<_>>());
        for d in Direction::ALL {
            assert_eq!(Direction::from_index(d.index()), d);
        }
    }

    #[test]
    fn cardinals_and_diagonals_partition_all() {
        assert!(Direction::CARDINALS.iter().all(|d| !d.is_diagonal()));
        assert!(Direction::DIAGONALS.iter().all(|d| d.is_diagonal()));
        assert_eq!(Direction::CARDINALS.len() + Direction::DIAGONALS.len(), 8);
    }

    #[test]
    fn opposites_face_back() {
        for d in Direction::ALL {
            assert_eq!(d.opposite().opposite(), d);
            let (dx, dy) = d.delta();
            let (ox, oy) = d.opposite().delta();
            assert_eq!((dx + ox, dy + oy), (0, 0));
        }
    }

    #[test]
    fn rotation_goes_round_the_clock() {
        assert_eq!(Direction::North.rotate_cw(), Direction::NorthEast);
        assert_eq!(Direction::NorthWest.rotate_cw(), Direction::North);
        assert_eq!(Direction::North.rotate_ccw(), Direction::NorthWest);
        for d in Direction::ALL {
            assert_eq!(d.rotate_cw().rotate_ccw(), d);
        }
    }

    #[test]
    fn perpendiculars_are_square_and_opposed() {
        for d in Direction::ALL {
            let [left, right] = d.perpendicular();
            assert_eq!(left.opposite(), right, "{d:?} turned unevenly");
            let (dx, dy) = d.delta();
            for t in [left, right] {
                let (tx, ty) = t.delta();
                assert_eq!(dx * tx + dy * ty, 0, "{t:?} is not square to {d:?}");
            }
        }
        assert_eq!(Direction::North.perpendicular(), [Direction::West, Direction::East]);
    }

    #[test]
    fn a_set_holds_exactly_what_was_put_in_it() {
        for d in Direction::ALL {
            let set = DirectionSet::only(d);
            assert_eq!(set.len(), 1);
            assert_eq!(set.iter().collect::<Vec<_>>(), vec![d]);
            for other in Direction::ALL {
                assert_eq!(set.contains(other), other == d);
            }
        }
        let mut set = DirectionSet::only(Direction::North);
        set.insert(Direction::North);
        assert_eq!(set.len(), 1);
        set.remove(Direction::North);
        assert!(set.is_empty());
    }

    #[test]
    fn a_set_iterates_clockwise_from_north() {
        let set: DirectionSet = [Direction::West, Direction::North, Direction::SouthEast].into_iter().collect();
        assert_eq!(set.iter().collect::<Vec<_>>(), vec![Direction::North, Direction::SouthEast, Direction::West]);
        assert_eq!(DirectionSet::ALL.len(), 8);
    }
}
