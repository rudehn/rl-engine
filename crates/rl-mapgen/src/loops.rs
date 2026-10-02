//! Cutting the longest detours through, so there is more than one way
//! round.

use std::collections::BTreeMap;

use rand::Rng;
use rl_core::geometry::manhattan;
use rl_core::{Direction, Grid, Grid2D, Point};
use rl_grid::{Terrain, TileId};

use crate::accrete::{Area, Join, Opening};
use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;
use crate::walk::{UNREACHED, steps_from};

/// Opens a door in the wall between two areas that touch but are far
/// apart on foot, longest detour first.
///
/// Accretion alone builds a tree, one way to anywhere. A candidate is a
/// wall cell with the floor of two different areas on opposite sides and
/// solid wall on the other two, not beside an opening already there, so a
/// loop never widens a way through. One candidate is drawn per pair of
/// areas. The detour is the walk between the two floor cells as the place
/// stands, measured again after each door, since one door shortens the
/// others.
///
/// Longest first is what makes a loop worth having: a door that saves
/// three steps is a second door, and one that saves sixty is another way
/// round. It reads [`Area`]s and does nothing on a map with none.
#[derive(Debug, Clone, Copy)]
pub struct OpenLoops {
    /// The tile a loop opens as.
    pub door: TileId,
    /// The shortest detour, in orthogonal steps, worth cutting through.
    pub min_detour: u32,
    /// The most loops to open.
    pub max: u32,
}

fn walkable_at(terrain: &Terrain, walkable: &[bool], p: Point) -> bool {
    terrain.get(p).is_some_and(|t| walkable[t.index()])
}

impl<C: BuildContext> Pass<C> for OpenLoops {
    fn name(&self) -> &'static str {
        "open_loops"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let walkable = ctx.tiles().tables().walkable;
        let bounds = ctx.terrain().bounds();
        let (w, h) = (bounds.width, bounds.height);
        if w < 3 || h < 3 {
            return Ok(());
        }
        let mut owner = Grid::<u32>::filled(w, h, 0);
        for a in ctx.outputs().iter::<Area>() {
            for c in &a.cells {
                owner.set(*c, a.index as u32 + 1);
            }
        }
        let joined: Vec<(usize, usize)> = ctx.outputs().iter::<Opening>().map(|o| (o.areas.0.min(o.areas.1), o.areas.0.max(o.areas.1))).collect();
        let taken: Vec<Point> = ctx.outputs().iter::<Opening>().flat_map(|o| o.cells.iter().copied()).collect();

        // Every wall cell between two areas not already joined, by pair. A
        // `BTreeMap` so the pairs are walked in one order on every run.
        let mut by_pair: BTreeMap<(usize, usize), Vec<(Point, Direction)>> = BTreeMap::new();
        for p in bounds.inflate(-1).cells() {
            let is_solid = |q: Point| !walkable_at(ctx.terrain(), &walkable, q);
            if !is_solid(p) || taken.iter().any(|t| manhattan(*t, p) <= 1) {
                continue;
            }
            for d in [Direction::East, Direction::South] {
                let (a, b) = (owner.get(p - d.offset()).copied().unwrap_or(0), owner.get(p + d.offset()).copied().unwrap_or(0));
                let walled = d.perpendicular().iter().all(|s| is_solid(p + s.offset()));
                if a != 0 && b != 0 && a != b && walled {
                    let (a, b) = (a as usize - 1, b as usize - 1);
                    if !joined.contains(&(a.min(b), a.max(b))) {
                        by_pair.entry((a, b)).or_default().push((p, d));
                    }
                }
            }
        }
        let mut candidates: Vec<((usize, usize), Point, Direction)> = Vec::new();
        for (pair, cells) in by_pair {
            let (p, d) = cells[ctx.rng().random_range(0..cells.len())];
            candidates.push((pair, p, d));
        }

        let mut opened: Vec<Point> = Vec::new();
        while (opened.len() as u32) < self.max {
            let open = |q: Point| walkable_at(ctx.terrain(), &walkable, q);
            // The first of equals, so a tie goes to the earlier pair.
            let mut best: Option<(u32, usize)> = None;
            for (i, (_, p, d)) in candidates.iter().enumerate() {
                if opened.iter().any(|o| manhattan(*o, *p) <= 1) {
                    continue;
                }
                let steps = steps_from(w, h, open, [*p - d.offset()]);
                let detour = steps.get(*p + d.offset()).copied().unwrap_or(UNREACHED);
                if best.is_none_or(|(b, _)| detour > b) {
                    best = Some((detour, i));
                }
            }
            let Some((detour, i)) = best else { break };
            if detour < self.min_detour {
                break;
            }
            let (areas, p, facing) = candidates.remove(i);
            ctx.terrain_mut().set(p, self.door);
            ctx.emit(Opening { cells: vec![p], facing, areas, join: Join::Door });
            opened.push(p);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accrete::tests::{accrete, rig, walkable};
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use rl_core::RunSeed;

    fn grown(seed: u64, w: i32, h: i32, loops: Option<OpenLoops>) -> BaseContext {
        let rig = rig();
        let chain = Chain::new().then(accrete(&rig, 3));
        let chain = match loops {
            Some(l) => chain.then(l),
            None => chain,
        };
        chain.run_retrying(|| BaseContext::blank(w, h, rig.tiles.clone(), rig.wall), RunSeed(seed), 1).unwrap()
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

    #[test]
    fn a_threshold_no_detour_reaches_opens_nothing() {
        let tree = grown(3, 84, 48, None);
        let none = grown(3, 84, 48, Some(OpenLoops { door: rig().door, min_detour: 100_000, max: 6 }));
        assert_eq!(tree.terrain(), none.terrain());
    }
}
