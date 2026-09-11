//! Where an attack or an ability lands.
//!
//! A [`TargetMode`] names a shape; [`footprint`] resolves it on a grid
//! into the cells it covers and the path a projectile took to get there,
//! stopping at whatever the caller says blocks. Blocking is a closure so
//! walls, closed doors and standing actors are all the caller's to name.
//! Nothing here knows what an ability does with the cells.

use rl_core::{Point, Rect, geometry};

/// The shape of a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetMode {
    /// The user's own cell.
    Own,
    /// One adjacent cell.
    Adjacent,
    /// A projectile towards the aim, stopping at the first blocker or at
    /// `range`.
    Bolt {
        /// Furthest cell it reaches.
        range: i32,
    },
    /// A projectile that bursts where it lands into a disc.
    Ball {
        /// Furthest cell it reaches.
        range: i32,
        /// Radius of the burst.
        radius: i32,
    },
    /// A line through the aim to `range`, passing blockers only if they
    /// do not stop it; every cell along it.
    Beam {
        /// Furthest cell it reaches.
        range: i32,
    },
    /// A cone from the user towards the aim.
    Cone {
        /// How far it reaches.
        length: i32,
    },
}

/// What a target mode resolved to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Footprint {
    /// The cells affected, in no particular order, without duplicates.
    pub cells: Vec<Point>,
    /// The cells a projectile passed through before landing, origin
    /// excluded, landing included. Empty for shapes without flight.
    pub path: Vec<Point>,
    /// Where a projectile landed: the aim, or the blocker it hit.
    pub landing: Option<Point>,
}

/// Resolves `mode` from `origin` towards `aim` within `bounds`, with
/// `blocks(p)` saying which cells stop a projectile. A blocker is where a
/// bolt lands and is included; the cell past it is not.
pub fn footprint(mode: TargetMode, origin: Point, aim: Point, bounds: Rect, blocks: impl Fn(Point) -> bool) -> Footprint {
    let inside = |p: Point| bounds.contains(p);
    match mode {
        TargetMode::Own => Footprint { cells: vec![origin], path: Vec::new(), landing: Some(origin) },
        TargetMode::Adjacent => {
            let cell = if geometry::is_adjacent(origin, aim) { aim } else { step_towards(origin, aim) };
            Footprint { cells: vec![cell], path: vec![cell], landing: Some(cell) }
        }
        TargetMode::Bolt { range } => {
            let (path, landing) = fly(origin, aim, range, &inside, &blocks);
            Footprint { cells: landing.into_iter().collect(), path, landing }
        }
        TargetMode::Ball { range, radius } => {
            let (path, landing) = fly(origin, aim, range, &inside, &blocks);
            let cells = landing.map(|c| geometry::disc(c, radius).filter(|p| inside(*p)).collect()).unwrap_or_default();
            Footprint { cells, path, landing }
        }
        TargetMode::Beam { range } => {
            let far = extend(origin, aim, range);
            let mut path = Vec::new();
            for p in geometry::line(origin, far).skip(1) {
                if !inside(p) || geometry::chebyshev(origin, p) > range {
                    break;
                }
                path.push(p);
                if blocks(p) {
                    break;
                }
            }
            let landing = path.last().copied();
            Footprint { cells: path.clone(), path, landing }
        }
        TargetMode::Cone { length } => {
            let cells: Vec<Point> = geometry::cone(origin, aim, length).filter(|p| inside(*p) && *p != origin).collect();
            Footprint { cells, path: Vec::new(), landing: None }
        }
    }
}

/// Flies a projectile from `origin` towards `aim`, up to `range`, stopping
/// at the first blocker. Returns the cells flown through and where it
/// landed.
fn fly(origin: Point, aim: Point, range: i32, inside: &impl Fn(Point) -> bool, blocks: &impl Fn(Point) -> bool) -> (Vec<Point>, Option<Point>) {
    if aim == origin {
        return (Vec::new(), None);
    }
    let far = extend(origin, aim, range);
    let mut path = Vec::new();
    for p in geometry::line(origin, far).skip(1) {
        if !inside(p) || geometry::chebyshev(origin, p) > range {
            break;
        }
        path.push(p);
        if blocks(p) || p == aim {
            break;
        }
    }
    let landing = path.last().copied();
    (path, landing)
}

/// The point `range` cells past `origin` along the ray through `aim`.
fn extend(origin: Point, aim: Point, range: i32) -> Point {
    let (dx, dy) = (aim.x - origin.x, aim.y - origin.y);
    let d = dx.abs().max(dy.abs());
    if d == 0 || d >= range {
        return aim;
    }
    // Scale the direction so the far end is at least `range` away.
    let k = (range + d - 1) / d;
    Point::new(origin.x + dx * k, origin.y + dy * k)
}

/// The neighbour of `from` in the direction of `to`.
fn step_towards(from: Point, to: Point) -> Point {
    Point::new(from.x + (to.x - from.x).signum(), from.y + (to.y - from.y).signum())
}

/// Whether a projectile from `origin` reaches `target` within `range`
/// with nothing in between that `blocks`. The target cell itself may
/// block: a standing actor is what a bolt is for.
pub fn clear_shot(origin: Point, target: Point, range: i32, bounds: Rect, blocks: impl Fn(Point) -> bool) -> bool {
    let fp = footprint(TargetMode::Bolt { range }, origin, target, bounds, blocks);
    fp.landing == Some(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Rect {
        Rect::new(0, 0, 20, 20)
    }

    #[test]
    fn a_bolt_stops_at_the_first_blocker_or_the_aim_or_its_range() {
        let o = Point::new(2, 2);
        let wall = |p: Point| p == Point::new(6, 2);
        let hit = footprint(TargetMode::Bolt { range: 10 }, o, Point::new(9, 2), bounds(), wall);
        assert_eq!(hit.landing, Some(Point::new(6, 2)), "the wall took it");
        assert_eq!(hit.path, vec![Point::new(3, 2), Point::new(4, 2), Point::new(5, 2), Point::new(6, 2)]);
        let aimed = footprint(TargetMode::Bolt { range: 10 }, o, Point::new(4, 2), bounds(), wall);
        assert_eq!(aimed.landing, Some(Point::new(4, 2)), "it stops at the aim");
        let short = footprint(TargetMode::Bolt { range: 2 }, o, Point::new(9, 2), bounds(), wall);
        assert_eq!(short.landing, Some(Point::new(4, 2)), "out of range");
        assert!(clear_shot(o, Point::new(5, 2), 5, bounds(), wall));
        assert!(!clear_shot(o, Point::new(7, 2), 5, bounds(), wall), "the wall is in the way");
        assert!(clear_shot(o, Point::new(6, 2), 5, bounds(), wall), "the blocker itself can be the target");
        assert_eq!(footprint(TargetMode::Bolt { range: 5 }, o, o, bounds(), wall).landing, None);
    }

    #[test]
    fn a_ball_bursts_where_it_lands_and_a_beam_runs_through() {
        let o = Point::new(2, 2);
        let ball = footprint(TargetMode::Ball { range: 6, radius: 1 }, o, Point::new(6, 2), bounds(), |_| false);
        assert_eq!(ball.landing, Some(Point::new(6, 2)));
        assert!(ball.cells.contains(&Point::new(6, 2)) && ball.cells.contains(&Point::new(7, 2)) && ball.cells.contains(&Point::new(6, 3)));
        assert!(!ball.cells.contains(&Point::new(8, 2)));
        let beam = footprint(TargetMode::Beam { range: 6 }, o, Point::new(4, 2), bounds(), |_| false);
        assert_eq!(beam.cells.len(), 6, "past the aim to the range: {:?}", beam.cells);
        assert_eq!(beam.landing, Some(Point::new(8, 2)));
        let edge = footprint(TargetMode::Beam { range: 30 }, o, Point::new(3, 2), bounds(), |_| false);
        assert_eq!(edge.landing, Some(Point::new(19, 2)), "clipped at the bounds");
    }

    #[test]
    fn own_adjacent_and_cone_shapes() {
        let o = Point::new(5, 5);
        assert_eq!(footprint(TargetMode::Own, o, o, bounds(), |_| false).cells, vec![o]);
        let adj = footprint(TargetMode::Adjacent, o, Point::new(9, 9), bounds(), |_| false);
        assert_eq!(adj.cells, vec![Point::new(6, 6)], "the first step towards a far aim");
        let cone = footprint(TargetMode::Cone { length: 3 }, o, Point::new(8, 5), bounds(), |_| false);
        assert!(cone.cells.contains(&Point::new(7, 5)));
        assert!(!cone.cells.contains(&o));
        assert!(!cone.cells.contains(&Point::new(3, 5)), "nothing behind");
    }
}
