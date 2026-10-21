//! Where an attack or an ability lands.
//!
//! A [`TargetMode`] names a shape; [`footprint`] resolves it on a grid
//! into the cells it covers and the path a projectile took to get there,
//! stopping at whatever the caller says blocks, and [`burst`] spreads a
//! ball's burst up to the walls. Blocking is a closure so walls, closed
//! doors and standing actors are all the caller's to name.
//! Nothing here knows what an ability does with the cells.
//!
//! Where a projectile comes to rest is decided here once, for every
//! flight: on whatever stopped it, at the aim or the end of its range, and
//! in front of a wall, never inside one. A bolt, a ball, a shot and a
//! thrown thing all fly this one way, so none of them lands where another
//! would not.

use rl_core::{Point, Rect, geometry};
use serde::{Deserialize, Serialize};

/// The shape of a target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// The cells a projectile passed through, origin excluded and the
    /// cell it comes to rest in included: a bolt or a ball never enters a
    /// wall it stopped against, and a beam's path is every cell it covers,
    /// the wall it ends on included. Empty for shapes without flight.
    pub path: Vec<Point>,
    /// The farthest cell a projectile reached: the aim, the end of its
    /// range, or the first cell that stopped it, which may be a wall. What
    /// a resolver compares with the aim to ask whether it got there, so a
    /// shape aimed at a wall reaches what it was aimed at, as one aimed at
    /// a body does.
    pub reached: Option<Point>,
    /// Where a projectile comes to rest: where it was [`reached`], unless
    /// that is a wall, and then the last cell it flew through before it,
    /// or the origin when the wall was the first cell. A ball bursts here,
    /// and a shot, a bolt and a thrown thing all lie here.
    ///
    /// Apart from `reached` because the two answer different questions: a
    /// spell sent at a wall did get where it was pointed, and still has
    /// nowhere inside the wall to land its effects.
    ///
    /// [`reached`]: Self::reached
    pub landing: Option<Point>,
}

/// Resolves `mode` from `origin` towards `aim` within `bounds`, with
/// `blocks(p)` saying which cells stop a projectile and `walls(p)` which
/// are walls. A blocker is what a projectile reaches and the cell past it
/// is not; a projectile comes to rest on a blocker that is not a wall, and
/// in front of one that is.
///
/// Two predicates because the two stop different things: a projectile
/// stops on whoever stands in its way, and a burst goes round them and
/// stops only at a wall, which is also the one thing nothing comes to rest
/// inside. See [`burst`].
pub fn footprint(mode: TargetMode, origin: Point, aim: Point, bounds: Rect, blocks: impl Fn(Point) -> bool, walls: impl Fn(Point) -> bool) -> Footprint {
    let inside = |p: Point| bounds.contains(p);
    match mode {
        TargetMode::Own => Footprint { cells: vec![origin], path: Vec::new(), reached: Some(origin), landing: Some(origin) },
        TargetMode::Adjacent => {
            let cell = if geometry::is_adjacent(origin, aim) { aim } else { step_towards(origin, aim) };
            Footprint { cells: vec![cell], path: vec![cell], reached: Some(cell), landing: Some(cell) }
        }
        TargetMode::Bolt { range } => {
            let Flown { path, reached, landing } = fly(origin, aim, range, &inside, &blocks, &walls);
            // A bolt that fell at its sender's feet went nowhere and covers
            // nothing, rather than the one who sent it.
            Footprint { cells: landing.filter(|l| *l != origin).into_iter().collect(), path, reached, landing }
        }
        TargetMode::Ball { range, radius } => {
            // A ball that hit a wall bursts where it came to rest, on the
            // near side, rather than inside the wall and out of both faces
            // of it; one that never left its sender bursts on them.
            let Flown { path, reached, landing } = fly(origin, aim, range, &inside, &blocks, &walls);
            let cells = landing.map(|c| burst(c, radius, bounds, &walls)).unwrap_or_default();
            Footprint { cells, path, reached, landing }
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
            // A beam covers the blocker it ends on, wall or body, but like
            // any flight comes to rest in front of a wall, so whatever is
            // placed where it landed is placed on open ground.
            let reached = path.last().copied();
            let landing = reached.map(|end| if walls(end) { path.iter().rev().nth(1).copied().unwrap_or(origin) } else { end });
            Footprint { cells: path.clone(), path, reached, landing }
        }
        TargetMode::Cone { length } => {
            let cells: Vec<Point> = geometry::cone(origin, aim, length).filter(|p| inside(*p) && *p != origin).collect();
            Footprint { cells, path: Vec::new(), reached: None, landing: None }
        }
    }
}

/// The cells a burst of `radius` around `center` reaches within `bounds`:
/// every cell of the disc with a straight line back to the centre that no
/// wall interrupts, the wall itself excluded.
///
/// A wall is what stops it and nothing else: an actor in the way is caught
/// in it rather than shielding whoever stands behind, which is the
/// difference between a burst and a projectile. A burst centred on a wall
/// reaches only that cell. The line is the integer one [`geometry::line`]
/// draws, so which cells a corner shadows is the same on every run.
pub fn burst(center: Point, radius: i32, bounds: Rect, walls: impl Fn(Point) -> bool) -> Vec<Point> {
    if walls(center) {
        return if bounds.contains(center) { vec![center] } else { Vec::new() };
    }
    geometry::disc(center, radius).filter(|p| bounds.contains(*p) && geometry::line(center, *p).skip(1).all(|c| !walls(c))).collect()
}

/// Where a projectile flew: the [`Footprint`] fields a flight answers.
struct Flown {
    path: Vec<Point>,
    reached: Option<Point>,
    landing: Option<Point>,
}

/// Flies a projectile from `origin` towards `aim`, up to `range`, stopping
/// at the first blocker, and brings it to rest: on what it reached, or in
/// front of it when that is a wall.
///
/// The origin is never asked whether it blocks, since the flight leaves it
/// before anything is looked at, so a caller's `blocks` need not spare the
/// one who sent it.
fn fly(origin: Point, aim: Point, range: i32, inside: &impl Fn(Point) -> bool, blocks: &impl Fn(Point) -> bool, walls: &impl Fn(Point) -> bool) -> Flown {
    if aim == origin {
        return Flown { path: Vec::new(), reached: None, landing: None };
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
    let reached = path.last().copied();
    // A wall is not somewhere to come to rest, nor a cell the projectile
    // flew through: it falls in the last open cell, or at its sender's feet.
    if reached.is_some_and(walls) {
        path.pop();
    }
    let landing = reached.map(|_| path.last().copied().unwrap_or(origin));
    Flown { path, reached, landing }
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
    let fp = footprint(TargetMode::Bolt { range }, origin, target, bounds, blocks, |_| false);
    fp.reached == Some(target)
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
        let hit = footprint(TargetMode::Bolt { range: 10 }, o, Point::new(9, 2), bounds(), wall, |_| false);
        assert_eq!(hit.landing, Some(Point::new(6, 2)), "the wall took it");
        assert_eq!(hit.path, vec![Point::new(3, 2), Point::new(4, 2), Point::new(5, 2), Point::new(6, 2)]);
        let aimed = footprint(TargetMode::Bolt { range: 10 }, o, Point::new(4, 2), bounds(), wall, |_| false);
        assert_eq!(aimed.landing, Some(Point::new(4, 2)), "it stops at the aim");
        let short = footprint(TargetMode::Bolt { range: 2 }, o, Point::new(9, 2), bounds(), wall, |_| false);
        assert_eq!(short.landing, Some(Point::new(4, 2)), "out of range");
        assert!(clear_shot(o, Point::new(5, 2), 5, bounds(), wall));
        assert!(!clear_shot(o, Point::new(7, 2), 5, bounds(), wall), "the wall is in the way");
        assert!(clear_shot(o, Point::new(6, 2), 5, bounds(), wall), "the blocker itself can be the target");
        assert_eq!(footprint(TargetMode::Bolt { range: 5 }, o, o, bounds(), wall, |_| false).landing, None);
    }

    #[test]
    fn a_ball_bursts_where_it_lands_and_a_beam_runs_through() {
        let o = Point::new(2, 2);
        let ball = footprint(TargetMode::Ball { range: 6, radius: 1 }, o, Point::new(6, 2), bounds(), |_| false, |_| false);
        assert_eq!(ball.landing, Some(Point::new(6, 2)));
        assert!(ball.cells.contains(&Point::new(6, 2)) && ball.cells.contains(&Point::new(7, 2)) && ball.cells.contains(&Point::new(6, 3)));
        assert!(!ball.cells.contains(&Point::new(8, 2)));
        let beam = footprint(TargetMode::Beam { range: 6 }, o, Point::new(4, 2), bounds(), |_| false, |_| false);
        assert_eq!(beam.cells.len(), 6, "past the aim to the range: {:?}", beam.cells);
        assert_eq!(beam.landing, Some(Point::new(8, 2)));
        let edge = footprint(TargetMode::Beam { range: 30 }, o, Point::new(3, 2), bounds(), |_| false, |_| false);
        assert_eq!(edge.landing, Some(Point::new(19, 2)), "clipped at the bounds");
        let wall = |p: Point| p == Point::new(5, 2);
        let walled = footprint(TargetMode::Beam { range: 6 }, o, Point::new(4, 2), bounds(), wall, wall);
        assert_eq!(walled.cells.last(), Some(&Point::new(5, 2)), "a beam covers the wall it ends on");
        assert_eq!((walled.reached, walled.landing), (Some(Point::new(5, 2)), Some(Point::new(4, 2))), "and comes to rest in front of it");
    }

    /// A burst stops at walls: a cell behind one, with no straight line
    /// back to the centre that misses it, is out of the burst, and so is
    /// the wall itself. Whoever stands in the way stops nothing: a burst
    /// is not a projectile.
    #[test]
    fn a_burst_does_not_reach_behind_a_wall() {
        let c = Point::new(5, 5);
        let wall = |p: Point| p.x == 7 && (3..=7).contains(&p.y);
        let cells = burst(c, 3, bounds(), wall);
        assert!(cells.contains(&c) && cells.contains(&Point::new(6, 5)), "this side of the wall");
        assert!(!cells.contains(&Point::new(7, 5)), "not the wall");
        assert!(!cells.contains(&Point::new(8, 5)), "and not behind it");
        assert!(cells.contains(&Point::new(3, 5)), "the open side reaches its whole radius");
        assert_eq!(burst(c, 3, bounds(), |_| false).len(), rl_core::geometry::disc(c, 3).count(), "in the open, the whole disc");
    }

    /// A ball that flies into a wall bursts on the near side of it, where
    /// it hit, and never inside the wall and out the other side.
    #[test]
    fn a_ball_that_hits_a_wall_bursts_on_the_near_side() {
        let o = Point::new(2, 5);
        let wall = |p: Point| p.x == 7;
        let ball = footprint(TargetMode::Ball { range: 10, radius: 1 }, o, Point::new(9, 5), bounds(), wall, wall);
        assert_eq!(ball.reached, Some(Point::new(7, 5)), "it stopped on the wall");
        assert_eq!(ball.landing, Some(Point::new(6, 5)), "and came to rest in front of it");
        assert!(ball.cells.contains(&Point::new(6, 5)), "where it burst");
        assert!(ball.cells.iter().all(|p| p.x < 7), "nowhere in or past the wall: {:?}", ball.cells);
    }

    /// A bolt comes to rest where a ball would burst: on a body it
    /// reached, and in front of a wall it reached, which is no cell it
    /// flew through; one stopped by a wall beside its sender falls at
    /// their feet and covers nobody.
    #[test]
    fn a_bolt_rests_on_a_body_and_in_front_of_a_wall() {
        let o = Point::new(2, 5);
        let wall = |p: Point| p.x == 7;
        let body = |p: Point| p == Point::new(5, 5);
        let walled = footprint(TargetMode::Bolt { range: 10 }, o, Point::new(9, 5), bounds(), wall, wall);
        assert_eq!((walled.reached, walled.landing), (Some(Point::new(7, 5)), Some(Point::new(6, 5))));
        assert_eq!(walled.path.last(), Some(&Point::new(6, 5)), "the flight ends where it rests");
        assert_eq!(walled.cells, vec![Point::new(6, 5)]);
        let aimed = footprint(TargetMode::Bolt { range: 10 }, o, Point::new(7, 5), bounds(), wall, wall);
        assert_eq!((aimed.reached, aimed.landing), (Some(Point::new(7, 5)), Some(Point::new(6, 5))), "aimed at the wall, it reaches it");
        let struck = footprint(TargetMode::Bolt { range: 10 }, o, Point::new(9, 5), bounds(), |p| wall(p) || body(p), wall);
        assert_eq!((struck.reached, struck.landing), (Some(Point::new(5, 5)), Some(Point::new(5, 5))), "on the body");
        let close = footprint(TargetMode::Bolt { range: 10 }, Point::new(6, 5), Point::new(9, 5), bounds(), wall, wall);
        assert_eq!((close.landing, close.path.len(), close.cells.len()), (Some(Point::new(6, 5)), 0, 0), "at its sender's feet");
    }

    #[test]
    fn own_adjacent_and_cone_shapes() {
        let o = Point::new(5, 5);
        assert_eq!(footprint(TargetMode::Own, o, o, bounds(), |_| false, |_| false).cells, vec![o]);
        let adj = footprint(TargetMode::Adjacent, o, Point::new(9, 9), bounds(), |_| false, |_| false);
        assert_eq!(adj.cells, vec![Point::new(6, 6)], "the first step towards a far aim");
        let cone = footprint(TargetMode::Cone { length: 3 }, o, Point::new(8, 5), bounds(), |_| false, |_| false);
        assert!(cone.cells.contains(&Point::new(7, 5)));
        assert!(!cone.cells.contains(&o));
        assert!(!cone.cells.contains(&Point::new(3, 5)), "nothing behind");
    }
}
