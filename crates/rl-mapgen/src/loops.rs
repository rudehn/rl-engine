//! Cutting the longest detours through, so there is more than one way
//! round.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use rand::Rng;
use rl_core::{Direction, Grid, Grid2D, Point};
use rl_grid::{BitGrid, Terrain, TileId};

use crate::accrete::{Area, Join, Opening};
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::dungeon::DoorWalk;
use crate::walk::{UNREACHED, steps_between};

/// Opens a door in the wall between two areas that touch but are far
/// apart on foot, longest detour first.
///
/// Accretion alone builds a tree, one way to anywhere. A candidate is a
/// wall cell with the floor of two different areas on opposite sides and
/// solid wall on the other two, not beside an opening already there, so a
/// loop never widens a way through. One candidate is drawn per pair of
/// areas, whichever side of a wall each lies on, so no pair gains two
/// doors. The detour is the walk between the two floor cells as the place
/// stands, through every cell the registry marks passable, a shut door
/// included, and never through an opening joined locked or sealed, as the
/// exit is drawn; measured again after each door, since one door shortens
/// the others.
///
/// Longest first is what makes a loop worth having: a door that saves
/// three steps is a second door, and one that saves sixty is another way
/// round. It reads [`Area`]s and does nothing on a map with none, and
/// never cuts into one painted from a prefab, whose [`Area::entries`] are
/// its only ways in.
#[derive(Debug, Clone, Copy)]
pub struct OpenLoops {
    /// The tile a loop opens as.
    pub door: TileId,
    /// The shortest detour, in orthogonal steps, worth cutting through.
    pub min_detour: u32,
    /// The most loops to open.
    pub max: u32,
}

fn passable_at(terrain: &Terrain, passable: &[bool], p: Point) -> bool {
    terrain.get(p).is_some_and(|t| passable[t.index()])
}

impl<C: BuildContext> Pass<C> for OpenLoops {
    fn name(&self) -> &'static str {
        "open_loops"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let passable = ctx.tiles().tables().passable;
        let bounds = ctx.terrain().bounds();
        let (w, h) = (bounds.width, bounds.height);
        if w < 3 || h < 3 {
            return Ok(());
        }
        let mut owner = Grid::<u32>::filled(w, h, 0);
        // A painted piece is entered only at its entries, by its own join,
        // so no wall of one is a candidate.
        for a in ctx.outputs().iter::<Area>().filter(|a| a.entries.is_none()) {
            for c in &a.cells {
                owner.set(*c, a.index as u32 + 1);
            }
        }
        // A pair is the two areas whichever is named first: a piece that
        // wraps another's corner meets it along two walls, and is still
        // one pair. A `BTreeSet` and a `BTreeMap` so both are walked in
        // one order on every run.
        let pair = |a: usize, b: usize| (a.min(b), a.max(b));
        let joined: BTreeSet<(usize, usize)> = ctx.outputs().iter::<Opening>().map(|o| pair(o.areas.0, o.areas.1)).collect();
        // Every opening's cells and the cells beside them: where a door
        // would widen a way through that is already there.
        let mut barred = BitGrid::new(w, h);
        for o in ctx.outputs().iter::<Opening>() {
            for c in &o.cells {
                bar(&mut barred, *c);
            }
        }

        // Every wall cell between two areas not already joined, by pair.
        let mut by_pair: BTreeMap<(usize, usize), Vec<Candidate>> = BTreeMap::new();
        for p in bounds.inflate(-1).cells() {
            let is_solid = |q: Point| !passable_at(ctx.terrain(), &passable, q);
            if !is_solid(p) || barred.contains(p) {
                continue;
            }
            for facing in [Direction::East, Direction::South] {
                let (a, b) = (owner.get(p - facing.offset()).copied().unwrap_or(0), owner.get(p + facing.offset()).copied().unwrap_or(0));
                let walled = facing.perpendicular().iter().all(|s| is_solid(p + s.offset()));
                if a != 0 && b != 0 && a != b && walled {
                    let areas = (a as usize - 1, b as usize - 1);
                    if !joined.contains(&pair(areas.0, areas.1)) {
                        by_pair.entry(pair(areas.0, areas.1)).or_default().push(Candidate { at: p, facing, areas, detour: 0 });
                    }
                }
            }
        }
        let mut candidates: Vec<Candidate> = Vec::new();
        for (_, cells) in by_pair {
            candidates.push(cells[ctx.rng().random_range(0..cells.len())]);
        }

        // The walk between a candidate's two sides as the place stands,
        // by the rule the exit is drawn by: through shut doors and never
        // through a lock. A side a later pass has built over is no way
        // through at all.
        let rule = DoorWalk::new(ctx, true);
        let measure = |terrain: &Terrain, c: &Candidate| {
            let open = |q: Point| rule.open(terrain, q);
            steps_between(w, h, open, c.at - c.facing.offset(), c.at + c.facing.offset())
        };
        for c in &mut candidates {
            c.detour = measure(ctx.terrain(), c);
        }
        candidates.retain(|c| c.detour != UNREACHED);

        let mut opened = 0;
        while opened < self.max {
            candidates.retain(|c| !barred.contains(c.at));
            // A door only ever shortens a walk, so a detour measured before
            // the last door is a ceiling on what it is now. Longest ceiling
            // first, each measured again, until the longest found is no
            // shorter than every ceiling left: that one is the longest, and
            // most candidates were never measured twice. `sort_by_key` is
            // stable, so equal ceilings keep the order of their pairs.
            candidates.sort_by_key(|c| Reverse(c.detour));
            let mut best: Option<usize> = None;
            for i in 0..candidates.len() {
                if best.is_some_and(|b| candidates[b].detour >= candidates[i].detour) {
                    break;
                }
                candidates[i].detour = measure(ctx.terrain(), &candidates[i]);
                if best.is_none_or(|b| candidates[i].detour > candidates[b].detour) {
                    best = Some(i);
                }
            }
            let Some(best) = best else { break };
            if candidates[best].detour < self.min_detour {
                break;
            }
            let door = candidates.remove(best);
            ctx.terrain_mut().set(door.at, self.door);
            bar(&mut barred, door.at);
            ctx.emit(Opening { cells: vec![door.at], facing: door.facing, areas: door.areas, join: Join::Door });
            opened += 1;
        }
        Ok(())
    }
}

/// Bars `p` and the four cells beside it from becoming a door.
fn bar(barred: &mut BitGrid, p: Point) {
    barred.insert(p);
    for d in Direction::CARDINALS {
        barred.insert(p + d.offset());
    }
}

/// A wall cell that could become a door: where, which way through, the
/// area behind it and the area ahead, and the walk a door would save, as
/// last measured.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    at: Point,
    facing: Direction,
    areas: (usize, usize),
    detour: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accrete::Accrete;
    use crate::accrete::tests::{accrete, rig, walkable};
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use crate::walk::steps_from;
    use rl_core::RunSeed;

    fn grown(seed: u64, w: i32, h: i32, loops: Option<OpenLoops>) -> BaseContext {
        let rig = rig();
        let chain = Chain::new().then(accrete(&rig, 3));
        let chain = match loops {
            Some(l) => chain.then(l),
            None => chain,
        };
        chain.run_retrying(|| BaseContext::blank(w, h, rig.tiles.clone(), rig.wall), RunSeed(seed), 20).unwrap()
    }

    fn pass() -> OpenLoops {
        OpenLoops { door: rig().door, min_detour: 20, max: 6 }
    }

    #[test]
    fn opening_loops_never_lengthens_a_walk_and_shortens_some() {
        for (w, h, seeds) in [(84, 48, 0..30u64), (160, 90, 0..6)] {
            let mut shortened = 0;
            for s in seeds {
                let (tree, looped) = (grown(s, w, h, None), grown(s, w, h, Some(pass())));
                let start = tree.outputs().first::<Area>().unwrap().cells[0];
                let before = steps_from(w, h, |p| walkable(&tree, p), [start]);
                let after = steps_from(w, h, |p| walkable(&looped, p), [start]);
                for (p, b) in before.iter() {
                    if *b != UNREACHED {
                        let a = *after.get(p).unwrap();
                        assert!(a <= *b, "{w}x{h} seed {s}: {p:?} went from {b} steps to {a}");
                        shortened += usize::from(a < *b);
                    }
                }
            }
            assert!(shortened > 0, "{w}x{h}: no walk on any seed got shorter");
        }
    }

    #[test]
    fn with_a_threshold_most_places_gain_a_loop_and_none_gains_more_than_the_cap() {
        let mut with_loops = 0;
        for s in 0..40 {
            let ctx = grown(s, 84, 48, Some(pass()));
            let (areas, openings) = (ctx.outputs().iter::<Area>().count(), ctx.outputs().iter::<Opening>().count());
            let loops = openings + 1 - areas;
            assert!(loops <= 6, "seed {s}: {loops} loops past the cap of 6");
            with_loops += usize::from(loops > 0);
        }
        assert!(with_loops >= 30, "{with_loops} of 40 places gained a loop");
    }

    /// A loop is never opened beside another opening, so a wall never
    /// gains a two-wide gap nobody asked for.
    #[test]
    fn no_loop_is_opened_orthogonally_beside_another_opening() {
        for s in 0..40 {
            let ctx = grown(s, 84, 48, Some(pass()));
            let all: Vec<&Opening> = ctx.outputs().iter::<Opening>().collect();
            for (i, a) in all.iter().enumerate() {
                for b in &all[i + 1..] {
                    if a.join == Join::Door || b.join == Join::Door {
                        for p in &a.cells {
                            for q in &b.cells {
                                assert!(rl_core::geometry::manhattan(*p, *q) > 1, "seed {s}: openings at {p:?} and {q:?} sit side by side");
                            }
                        }
                    }
                }
            }
        }
    }

    /// A game's shut door is passable but not walkable this moment. Grown
    /// with one, a place gains its loops as it does with a door that is
    /// floor: the detour is walked through the doors, not stopped at them.
    #[test]
    fn a_detour_through_shut_doors_is_measured_and_cut() {
        let rig = rig();
        let shut = rig.tiles.expect("door_closed");
        let mut with_loops = 0;
        for s in 0..40 {
            let grow = Accrete { door: shut, ..accrete(&rig, 3) };
            let chain = Chain::new().then(grow).then(OpenLoops { door: shut, ..pass() });
            let ctx = chain.run_retrying(|| BaseContext::blank(84, 48, rig.tiles.clone(), rig.wall), RunSeed(s), 20).unwrap();
            let (areas, openings) = (ctx.outputs().iter::<Area>().count(), ctx.outputs().iter::<Opening>().count());
            with_loops += usize::from(openings + 1 > areas);
        }
        assert!(with_loops >= 30, "{with_loops} of 40 places behind shut doors gained a loop");
    }

    #[test]
    fn a_threshold_no_detour_reaches_opens_nothing() {
        let tree = grown(3, 84, 48, None);
        let none = grown(3, 84, 48, Some(OpenLoops { door: rig().door, min_detour: 100_000, max: 6 }));
        assert_eq!(tree.terrain(), none.terrain());
    }

    /// One way through per pair of areas, whichever side of the wall each
    /// lies on: a piece that wraps another's corner touches it along two
    /// walls, and still gets one door.
    #[test]
    fn no_pair_of_areas_is_joined_twice() {
        let mut bent = 0;
        for (w, h, seeds) in [(84, 48, 0..200u64), (160, 90, 0..12)] {
            for s in seeds {
                let ctx = grown(s, w, h, Some(OpenLoops { door: rig().door, min_detour: 4, max: 40 }));
                let mut pairs: Vec<(usize, usize)> = ctx.outputs().iter::<Opening>().map(|o| (o.areas.0.min(o.areas.1), o.areas.0.max(o.areas.1))).collect();
                let all = pairs.len();
                pairs.sort();
                pairs.dedup();
                assert_eq!(pairs.len(), all, "{w}x{h} seed {s}: a pair of areas has two ways through");
                bent += 1;
            }
        }
        assert!(bent > 0);
    }

    /// A door is cut from an area into the one beyond it: its facing leads
    /// from the first of its areas to the second, as a join's does.
    #[test]
    fn a_loop_faces_from_its_first_area_into_its_second() {
        for s in 0..40 {
            let ctx = grown(s, 84, 48, Some(pass()));
            let areas: Vec<&Area> = ctx.outputs().iter::<Area>().collect();
            for o in ctx.outputs().iter::<Opening>() {
                let at = o.cells[0];
                assert!(areas[o.areas.0].cells.contains(&(at - o.facing.offset())), "seed {s}: {at:?}");
                assert!(areas[o.areas.1].cells.contains(&(at + o.facing.offset())), "seed {s}: {at:?}");
            }
        }
    }
}
