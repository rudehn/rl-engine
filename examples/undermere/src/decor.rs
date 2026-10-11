//! What a place has seen, beyond what the pool put there on purpose.
//!
//! Most of a lived-in floor's texture is not this module's: rubble in
//! the halls, moss in the caves, cobwebs in corners and bones in the
//! catacombs are `assets/dressings.ron`'s own "Collapsed Ceiling", "Moss
//! Garden", "Web-Choked Corners" and "Bone Scatter", laid by
//! `rl_mapgen::Dress` over whatever area the pool left for it, and ash
//! is the Forge of Embers' and the Ashen Galleries' own floor tile
//! besides. This module adds the two touches neither already gives a
//! floor: [`BloodTrail`], a body a dead end took and the blood that
//! still marks the doorway to it, and [`WallInscriptions`], a line of
//! the drowned kingdom's history carved where a wall stands over it.
//! Both read what [`rl_engine::rl_mapgen::Survey`] rated rather than any
//! piece of the pool, and both are laid in `Phase::Finish`, after
//! `rl_mapgen::Dress`, from `dungeon::Undermere::decor`.
//!
//! Neither ever stands on the route or over a cell a piece or a
//! dressing already claimed, and neither blocks: the body and the
//! inscription are both scenery, and the blood and the wall inscription
//! are both read off the floor they lie on, exactly as rubble and
//! bones already are.

use std::collections::BTreeMap;

use rand::Rng;
use rand::seq::SliceRandom;
use rl_engine::prelude::*;
use rl_engine::rl_mapgen::{Area, Opening};

use crate::dungeon::{pieces, slots, walkable, ways};

/// A body a dead end took, and the blood that still marks the doorway to
/// it: one dead end a floor's survey rated, reached by one opening,
/// walked from its threshold to the cell farthest in, at most
/// `max_cells` deep. Never an area a piece or a dressing already
/// claimed, and never the route.
pub struct BloodTrail {
    /// A stable name, so two floors' own trails draw different streams.
    pub name: &'static str,
    /// What the trail is smeared in.
    pub blood: TileId,
    /// The body at its end.
    pub body: Prefab,
    /// The chance, in a hundred, that a floor gets one at all: most
    /// floors do not, so the one that does reads as something that
    /// happened rather than wallpaper.
    pub chance_pct: u32,
    /// The farthest the trail ever walks from the doorway.
    pub max_cells: u32,
}

impl Pass<BaseContext> for BloodTrail {
    fn name(&self) -> &'static str {
        self.name
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        if ctx.rng().random_range(0..100) >= self.chance_pct {
            return Ok(());
        }
        // Cloned out of `ctx.outputs()` rather than borrowed from it, so
        // the borrow is gone before `ctx.terrain_mut()` and `ctx.rng()`
        // are needed again below.
        let Some(surveyed) = ctx.outputs().first::<rl_engine::rl_mapgen::Surveyed>().cloned() else { return Ok(()) };
        let areas: Vec<Area> = ctx.outputs().iter::<Area>().cloned().collect();
        let openings: Vec<Opening> = ctx.outputs().iter::<Opening>().cloned().collect();
        let tables = ctx.tiles().tables();
        let claimed = pieces(ctx);
        let marked = slots(ctx);
        let on_a_way = ways(ctx);

        // A doorway into a dead end neither the route nor a piece nor a
        // dressing has already claimed any cell of.
        let mut candidates: Vec<(&Opening, &Area)> = Vec::new();
        for o in &openings {
            for idx in [o.areas.0, o.areas.1] {
                let Some(facts) = surveyed.areas.get(idx) else { continue };
                let Some(area) = areas.iter().find(|a| a.index == idx) else { continue };
                if !facts.dead_end || area.cells.iter().any(|p| marked.contains(p) || claimed.iter().any(|r| r.contains(*p)) || on_a_way.contains(p)) {
                    continue;
                }
                candidates.push((o, area));
            }
        }
        if candidates.is_empty() {
            return Ok(());
        }
        // Sorted first, so the same seed draws the same trail whichever
        // order the chain's own passes gave these to `ctx.outputs()`.
        candidates.sort_by_key(|(o, a)| (o.cells[0].x, o.cells[0].y, a.index));
        let (opening, area) = candidates[ctx.rng().random_range(0..candidates.len())];

        // The area's own cell nearest the doorway: never the doorway
        // itself, which is a door or an arch and not a floor to paint
        // over.
        let mut threshold = None;
        for d in &opening.cells {
            for dir in Direction::CARDINALS {
                let n = *d + dir.offset();
                if area.cells.contains(&n) {
                    threshold = Some(n);
                    break;
                }
            }
            if threshold.is_some() {
                break;
            }
        }
        let Some(threshold) = threshold else { return Ok(()) };

        // Breadth-first into the dead end's own cells only, so the trail
        // never spills past the doorway it came through; the last cell
        // reached is where the body lies. `Point` has no `Ord` of its
        // own, so the parent each cell was reached from is kept by
        // `(x, y)` rather than in a `BTreeMap<Point, _>`.
        let mut parent: BTreeMap<(i32, i32), Point> = BTreeMap::new();
        let mut frontier = vec![threshold];
        let mut farthest = threshold;
        for _ in 0..self.max_cells {
            let mut next = Vec::new();
            for p in frontier {
                for d in Direction::CARDINALS {
                    let n = p + d.offset();
                    let key = (n.x, n.y);
                    if area.cells.contains(&n) && walkable(ctx, &tables, n) && !marked.contains(&n) && !parent.contains_key(&key) && n != threshold {
                        parent.insert(key, p);
                        next.push(n);
                        farthest = n;
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        let mut path = vec![farthest];
        while *path.last().expect("just pushed") != threshold {
            let prev = parent[&(path.last().expect("just pushed").x, path.last().expect("just pushed").y)];
            path.push(prev);
        }
        // `path` runs from the body back to the threshold; blood on
        // every cell between the two, the body on the first.
        for p in &path[1..] {
            ctx.terrain_mut().set(*p, self.blood);
        }
        StampPrefab { name: self.name, prefab: self.body.clone(), at: Placement::At(farthest), orient: Orient::Fixed }.apply(ctx)
    }
}

/// A line of the drowned kingdom's history, carved where a wall stands
/// over it: at most one a floor, on open floor beside a wall, never on
/// the route or over a cell a piece already claimed.
pub struct WallInscriptions {
    /// A stable name, so two floors' own draws differ.
    pub name: &'static str,
    /// The one-cell piece: a "wall inscription" prop on the floor.
    pub piece: Prefab,
    /// The chance, in a hundred, that a floor carves one at all.
    pub chance_pct: u32,
}

impl Pass<BaseContext> for WallInscriptions {
    fn name(&self) -> &'static str {
        self.name
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        if ctx.rng().random_range(0..100) >= self.chance_pct {
            return Ok(());
        }
        let tables = ctx.tiles().tables();
        let (on_a_way, claimed, marked) = (ways(ctx), pieces(ctx), slots(ctx));
        let beside_a_wall = |p: Point| Direction::CARDINALS.into_iter().any(|d| !walkable(ctx, &tables, p + d.offset()));
        let clear = |p: Point| {
            walkable(ctx, &tables, p) && !on_a_way.contains(&p) && !marked.contains(&p) && claimed.iter().all(|r| !r.contains(p)) && beside_a_wall(p)
        };
        let mut spots: Vec<Point> = ctx.terrain().bounds().cells().filter(|p| clear(*p)).collect();
        spots.shuffle(ctx.rng());
        let Some(at) = spots.into_iter().next() else { return Ok(()) };
        StampPrefab { name: self.name, prefab: self.piece.clone(), at: Placement::At(at), orient: Orient::Fixed }.apply(ctx)
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::dungeon::{FLOORS, Undermere, map_of};

    /// Over a span of seeds, some floor but never the throne's shows a
    /// blood trail, and not every floor does: `BloodTrail::chance_pct`
    /// is a chance, read on every floor's own decor, not a certainty.
    #[test]
    fn a_blood_trail_turns_up_on_some_floors_but_never_the_throne_over_a_span_of_seeds() {
        let (mut with_blood, mut total) = (0usize, 0usize);
        for s in 0..60u64 {
            let undermere = Undermere::new(RunSeed(s));
            let blood = undermere.tiles().id("blood").expect("blood is a registered tile");
            for f in 1..FLOORS {
                let built = undermere.build(map_of(f), None).expect("a floor still builds with its own decor");
                total += 1;
                if built.terrain.iter().any(|(_, t)| t == blood) {
                    with_blood += 1;
                }
            }
            let throne = undermere.build(map_of(FLOORS), None).expect("the throne still builds");
            assert!(!throne.terrain.iter().any(|(_, t)| t == blood), "seed {s}: the throne shows a blood trail");
        }
        assert!(with_blood > 0, "no floor in the span ever showed a blood trail");
        assert!(with_blood < total, "every floor in the span showed a blood trail");
    }
}
