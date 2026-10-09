//! Measuring a grown place, and holding it to what its recipe asked for.
//!
//! [`Survey`] decides nothing. It is in the engine because every game
//! that puts a thing somewhere for where that is would otherwise measure
//! the same facts again, differently. [`Require`] is what makes a loose
//! generator safe: a place that falls short fails, and the chain builds
//! another.

use rl_core::{Direction, Grid2D, Point};

use crate::accrete::{Area, Opening};
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::dungeon::ExitPoint;
use crate::passes::StartPoint;
use crate::walk::{UNREACHED, steps_from};

/// What the survey found out about one area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaFacts {
    /// How many openings lead into it.
    pub openings: u32,
    /// One way in: somewhere a player went on purpose.
    pub dead_end: bool,
    /// Without it, some two other areas have no way between them.
    pub chokepoint: bool,
    /// Three ways or more lead out of it.
    pub junction: bool,
    /// Orthogonal steps from its nearest opening to the nearest cell of
    /// the shortest route from the start to the exit, or to the start
    /// where there is no exit. What is far from the direct way is what a
    /// player chose to go and find. `u32::MAX` for an area the route
    /// cannot be walked to from, which a place grown and surveyed in one
    /// chain never has, but one a later pass walled off does.
    pub seclusion: u32,
}

/// What the survey found out about a place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surveyed {
    /// One per [`Area`], at the area's index.
    pub areas: Vec<AreaFacts>,
    /// The independent loops among areas and openings: openings less
    /// areas plus one.
    pub loops: u32,
    /// The share of the map that can be walked on, in a hundred.
    pub open_pct: u32,
    /// Orthogonal steps from the start to the exit; `None` with no exit.
    pub route: Option<u32>,
}

/// Measures the place and emits one [`Surveyed`]. Runs once the start and
/// the exit are known; fails without a start, without an area, or when
/// the exit cannot be walked to.
#[derive(Debug, Clone, Copy, Default)]
pub struct Survey;

impl<C: BuildContext> Pass<C> for Survey {
    fn name(&self) -> &'static str {
        "survey"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let err = |reason: &str| BuildError::new("survey", reason);
        let start = ctx.outputs().first::<StartPoint>().ok_or_else(|| err("no start point emitted before it"))?.0;
        let exit = ctx.outputs().first::<ExitPoint>().map(|e| e.0);
        let areas: Vec<&Area> = ctx.outputs().iter::<Area>().collect();
        let openings: Vec<&Opening> = ctx.outputs().iter::<Opening>().collect();
        if areas.is_empty() {
            return Err(err("no area emitted before it"));
        }
        let walkable = ctx.tiles().tables().walkable;
        let bounds = ctx.terrain().bounds();
        let (w, h) = (bounds.width, bounds.height);
        let open = |p: Point| ctx.terrain().get(p).is_some_and(|t| walkable[t.index()]);

        // The shortest route, walked down from the start to the exit.
        let mut route_cells = vec![start];
        let mut route = None;
        if let Some(exit) = exit {
            let to_exit = steps_from(w, h, open, [exit]);
            let at = |p: Point| to_exit.get(p).copied().unwrap_or(UNREACHED);
            if at(start) == UNREACHED {
                return Err(err("the exit cannot be walked to from the start"));
            }
            route = Some(at(start));
            let mut here = start;
            while here != exit {
                here = Direction::CARDINALS
                    .iter()
                    .map(|d| here + d.offset())
                    .find(|n| at(*n) != UNREACHED && at(*n) + 1 == at(here))
                    .expect("a reached cell has a neighbour one step nearer");
                route_cells.push(here);
            }
        }
        let off_route = steps_from(w, h, open, route_cells);

        let joins = |o: &Opening, i: usize| o.areas.0 == i || o.areas.1 == i;
        let chokepoints = articulations(areas.len(), &openings);
        let facts = (0..areas.len())
            .map(|i| {
                let doors: Vec<Point> = openings.iter().filter(|o| joins(o, i)).flat_map(|o| o.cells.iter().copied()).collect();
                let count = openings.iter().filter(|o| joins(o, i)).count() as u32;
                let from: &[Point] = if doors.is_empty() { &areas[i].cells } else { &doors };
                let seclusion = from.iter().map(|p| off_route.get(*p).copied().unwrap_or(UNREACHED)).min().unwrap_or(UNREACHED);
                let chokepoint = chokepoints[i];
                AreaFacts { openings: count, dead_end: count == 1, chokepoint, junction: count >= 3, seclusion }
            })
            .collect();

        let open_cells = bounds.cells().filter(|p| open(*p)).count() as u32;
        let loops = (openings.len() as u32 + 1).saturating_sub(areas.len() as u32);
        let surveyed = Surveyed { areas: facts, loops, open_pct: open_cells * 100 / (w * h) as u32, route };
        ctx.emit(surveyed);
        Ok(())
    }
}

/// Which of `count` areas, joined by `openings`, part two others when
/// taken away: one depth-first walk, numbering each area as it is reached
/// and keeping the earliest number reachable from below it without going
/// back up through it. An area some child cannot get above is the only
/// way to that child.
fn articulations(count: usize, openings: &[&Opening]) -> Vec<bool> {
    const UNSEEN: usize = usize::MAX;
    let mut beside: Vec<Vec<usize>> = vec![Vec::new(); count];
    for o in openings {
        beside[o.areas.0].push(o.areas.1);
        beside[o.areas.1].push(o.areas.0);
    }
    let (mut found, mut reached, mut lowest, mut clock) = (vec![false; count], vec![UNSEEN; count], vec![0; count], 0);
    for root in 0..count {
        if reached[root] != UNSEEN {
            continue;
        }
        reached[root] = clock;
        lowest[root] = clock;
        clock += 1;
        let mut children_of_root = 0;
        // Each entry: an area, the area it was reached from, and how many
        // of its neighbours have been looked at.
        let mut stack = vec![(root, UNSEEN, 0)];
        while let Some(&(area, from, looked)) = stack.last() {
            if let Some(&n) = beside[area].get(looked) {
                stack.last_mut().expect("just read").2 += 1;
                if reached[n] == UNSEEN {
                    reached[n] = clock;
                    lowest[n] = clock;
                    clock += 1;
                    children_of_root += usize::from(area == root);
                    stack.push((n, area, 0));
                } else if n != from {
                    lowest[area] = lowest[area].min(reached[n]);
                }
            } else {
                stack.pop();
                if let Some(&(up, _, _)) = stack.last() {
                    lowest[up] = lowest[up].min(lowest[area]);
                    found[up] |= up != root && lowest[area] >= reached[up];
                }
            }
        }
        found[root] = children_of_root > 1;
    }
    found
}

/// Fails the build when the place falls short of what its recipe asked
/// for, naming the first bound it missed and both numbers.
#[derive(Debug, Clone)]
pub struct Require {
    /// The open share, at least and at most, in a hundred.
    pub open_pct: (u32, u32),
    /// The fewest areas drawn by each named shape.
    pub areas: Vec<(String, u32)>,
    /// The fewest loops.
    pub loops: u32,
    /// The largest share of areas, in a hundred, that may be dead ends.
    pub dead_end_pct: u32,
    /// The shortest walk from the start to the exit, in orthogonal steps.
    /// More than nought fails a place with no exit.
    pub route: u32,
}

impl<C: BuildContext> Pass<C> for Require {
    fn name(&self) -> &'static str {
        "require"
    }
    fn phase(&self) -> Phase {
        Phase::Finish
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let fail = |reason: String| Err(BuildError::new("require", reason));
        let Some(s) = ctx.outputs().first::<Surveyed>() else {
            return fail("no survey emitted before it".to_string());
        };
        if s.open_pct < self.open_pct.0 || s.open_pct > self.open_pct.1 {
            return fail(format!("{}% of the map is open, outside {}% to {}%", s.open_pct, self.open_pct.0, self.open_pct.1));
        }
        for (shape, fewest) in &self.areas {
            let found = ctx.outputs().iter::<Area>().filter(|a| a.shape == *shape).count() as u32;
            if found < *fewest {
                return fail(format!("{found} areas drawn by {shape:?}, fewer than {fewest}"));
            }
        }
        if s.loops < self.loops {
            return fail(format!("{} loops, fewer than {}", s.loops, self.loops));
        }
        let dead = s.areas.iter().filter(|a| a.dead_end).count() as u32 * 100 / s.areas.len().max(1) as u32;
        if dead > self.dead_end_pct {
            return fail(format!("{dead}% of areas are a dead end, more than {}%", self.dead_end_pct));
        }
        match s.route {
            Some(route) if route < self.route => fail(format!("the exit is {route} steps from the start, nearer than {}", self.route)),
            None if self.route > 0 => fail(format!("there is no exit, and one was asked for {} steps away", self.route)),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accrete::{Join, bounds_of, largest_rect};
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use rl_core::RunSeed;
    use rl_grid::TileRegistry;

    /// A context drawn from rows: `#` solid, a digit the floor of that
    /// area, `+` an opening between the areas on either side of it.
    fn drawn(rows: &[&str], start: Point, exit: Option<Point>) -> BaseContext {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let (w, h) = (rows[0].len() as i32, rows.len() as i32);
        let mut ctx = BaseContext::blank(w, h, tiles, wall);
        let at = |x: i32, y: i32| rows.get(y as usize).and_then(|r| r.as_bytes().get(x as usize)).copied().unwrap_or(b'#');
        let mut cells: std::collections::BTreeMap<u8, Vec<Point>> = Default::default();
        for y in 0..h {
            for x in 0..w {
                let c = at(x, y);
                if c != b'#' {
                    ctx.terrain_mut().set(Point::new(x, y), floor);
                }
                if c.is_ascii_digit() {
                    cells.entry(c - b'0').or_default().push(Point::new(x, y));
                }
            }
        }
        for (i, cells) in &cells {
            let b = bounds_of(cells);
            ctx.emit(Area { index: *i as usize, shape: "s".into(), district: "d".into(), inner: largest_rect(cells, b), bounds: b, cells: cells.clone() });
        }
        for y in 0..h {
            for x in 0..w {
                if at(x, y) == b'+' {
                    let (l, r, u, d) = (at(x - 1, y), at(x + 1, y), at(x, y - 1), at(x, y + 1));
                    let (facing, a, b) = if l.is_ascii_digit() && r.is_ascii_digit() { (Direction::East, l, r) } else { (Direction::South, u, d) };
                    ctx.emit(Opening { cells: vec![Point::new(x, y)], facing, areas: ((a - b'0') as usize, (b - b'0') as usize), join: Join::Door });
                }
            }
        }
        ctx.emit(StartPoint(start));
        if let Some(e) = exit {
            ctx.emit(ExitPoint(e));
        }
        ctx
    }

    fn surveyed(mut ctx: BaseContext) -> Surveyed {
        Chain::new().then(Survey).run(&mut ctx, RunSeed(0)).unwrap();
        ctx.outputs().first::<Surveyed>().unwrap().clone()
    }

    // Area 0 on the left holds the start, 1 in the middle, 2 on the right
    // holds the exit, and 3 hangs below the middle with one way in.
    const LINE: [&str; 7] = ["#############", "#000+111+222#", "#000#111#222#", "#####+#######", "#####3#######", "#####3#######", "#############"];

    #[test]
    fn a_dead_end_a_chokepoint_and_a_junction_are_each_what_the_drawing_says() {
        let s = surveyed(drawn(&LINE, Point::new(1, 1), Some(Point::new(11, 1))));
        assert_eq!(s.areas.iter().map(|a| a.openings).collect::<Vec<_>>(), vec![1, 3, 1, 1]);
        assert_eq!(s.areas.iter().map(|a| a.dead_end).collect::<Vec<_>>(), vec![true, false, true, true]);
        assert_eq!(s.areas.iter().map(|a| a.chokepoint).collect::<Vec<_>>(), vec![false, true, false, false], "only the middle parts the others");
        assert_eq!(s.areas.iter().map(|a| a.junction).collect::<Vec<_>>(), vec![false, true, false, false]);
        assert_eq!(s.loops, 0);
        assert_eq!(s.route, Some(10));
    }

    #[test]
    fn an_area_off_the_route_is_as_secluded_as_the_walk_to_its_door_and_one_on_it_is_not() {
        let s = surveyed(drawn(&LINE, Point::new(1, 1), Some(Point::new(11, 1))));
        assert_eq!(s.areas[1].seclusion, 0, "the route runs through its openings");
        assert_eq!(s.areas[3].seclusion, 2, "its door is two steps below the route along the top row");
    }

    #[test]
    fn a_ring_of_four_areas_has_one_loop_and_no_chokepoint() {
        let ring = ["#########", "#000+111#", "#000#111#", "##+###+##", "#333+222#", "#333#222#", "#########"];
        let s = surveyed(drawn(&ring, Point::new(1, 1), Some(Point::new(7, 5))));
        assert_eq!(s.loops, 1);
        assert!(s.areas.iter().all(|a| !a.chokepoint && !a.dead_end));
    }

    #[test]
    fn with_no_exit_seclusion_is_the_walk_from_the_start() {
        let s = surveyed(drawn(&LINE, Point::new(1, 1), None));
        assert_eq!(s.route, None);
        assert_eq!(s.areas[0].seclusion, 3, "from the start to its one opening");
    }

    /// A start walled off from the exit is an error, never a panic on a
    /// distance nothing reached.
    #[test]
    fn a_start_that_cannot_walk_to_the_exit_is_an_error_by_name() {
        let cut = ["#######", "#00#11#", "#######"];
        let mut ctx = drawn(&cut, Point::new(1, 1), Some(Point::new(5, 1)));
        let err = Chain::new().then(Survey).run(&mut ctx, RunSeed(0)).unwrap_err();
        assert_eq!(err.pass, "survey");
    }

    #[test]
    fn each_requirement_fails_by_name_with_both_numbers_and_all_met_pass() {
        let run = |r: Require| {
            let mut ctx = drawn(&LINE, Point::new(1, 1), Some(Point::new(11, 1)));
            Chain::new().then(Survey).then(r).run(&mut ctx, RunSeed(0))
        };
        let met = || Require { open_pct: (10, 60), areas: vec![("s".into(), 4)], loops: 0, dead_end_pct: 75, route: 10 };
        run(met()).unwrap();
        let reason = |r: Require| run(r).unwrap_err().reason;
        assert!(reason(Require { open_pct: (50, 60), ..met() }).contains("open"));
        assert!(reason(Require { open_pct: (1, 5), ..met() }).contains("open"));
        assert!(reason(Require { areas: vec![("s".into(), 5)], ..met() }).contains("\"s\""));
        assert!(reason(Require { loops: 1, ..met() }).contains("loop"));
        assert!(reason(Require { dead_end_pct: 50, ..met() }).contains("dead end"));
        let r = reason(Require { route: 11, ..met() });
        assert!(r.contains("10") && r.contains("11"), "{r}");
    }

    #[test]
    fn a_grown_place_surveys_and_adding_the_survey_changes_no_cell() {
        use crate::accrete::tests::{accrete, rig};
        use crate::dungeon::{FarthestExit, RandomStart};
        use crate::loops::OpenLoops;
        let rig = rig();
        for s in 0..20 {
            let make = || BaseContext::blank(84, 48, rig.tiles.clone(), rig.wall);
            let base = || Chain::new().then(accrete(&rig, 3)).then(OpenLoops { door: rig.door, min_detour: 20, max: 6 }).then(RandomStart).then(FarthestExit);
            let without = base().run_retrying(make, RunSeed(s), 1).unwrap();
            let with = base().then(Survey).run_retrying(make, RunSeed(s), 1).unwrap();
            assert_eq!(without.terrain(), with.terrain(), "seed {s}");
            let surveyed = with.outputs().first::<Surveyed>().unwrap();
            assert_eq!(surveyed.areas.len(), with.outputs().iter::<Area>().count());
            assert!(surveyed.route.is_some_and(|r| r > 0), "seed {s}");
        }
    }
}
