//! Passes that need nothing but a terrain and a registry.
//!
//! Every tile these passes write is a parameter. They know nothing about
//! what a wall or grass is; the game tells them which ids to use.

use rand::Rng;
use rl_core::{Grid, Grid2D, Point, Steps};
use rl_grid::TileId;

use crate::chain::{BuildError, Pass, Phase};
use crate::context::BuildContext;

/// Sets every cell to one tile.
#[derive(Debug, Clone, Copy)]
pub struct Fill {
    /// The tile to fill with.
    pub tile: TileId,
}

impl<C: BuildContext> Pass<C> for Fill {
    fn name(&self) -> &'static str {
        "fill"
    }
    fn phase(&self) -> Phase {
        Phase::Ground
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        ctx.terrain_mut().fill(self.tile);
        Ok(())
    }
}

/// Writes one tile around the outer ring, so a map has an edge.
#[derive(Debug, Clone, Copy)]
pub struct Border {
    /// The tile for the ring.
    pub tile: TileId,
}

impl<C: BuildContext> Pass<C> for Border {
    fn name(&self) -> &'static str {
        "border"
    }
    fn phase(&self) -> Phase {
        Phase::Ground
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let bounds = ctx.terrain().bounds();
        let t = ctx.terrain_mut();
        for p in bounds.border() {
            t.set(p, self.tile);
        }
        Ok(())
    }
}

/// Replaces a share of `on` cells with `tile`, independently per cell.
#[derive(Debug, Clone, Copy)]
pub struct Scatter {
    /// A stable name, so two scatters in one chain draw different streams.
    pub name: &'static str,
    /// The tile to scatter.
    pub tile: TileId,
    /// Only cells currently holding this tile are candidates.
    pub on: TileId,
    /// Chance per candidate cell, in percent.
    pub chance_pct: u32,
}

impl<C: BuildContext> Pass<C> for Scatter {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Growth
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let cells = ctx.terrain().len();
        for idx in 0..cells {
            if ctx.terrain().get_idx(idx) == self.on && ctx.rng().random_range(0..100) < self.chance_pct {
                ctx.terrain_mut().set_idx(idx, self.tile);
            }
        }
        Ok(())
    }
}

/// A cave carved by cellular automata: random fill, then smoothing rounds
/// where a cell becomes `wall` if enough of its eight neighbours are.
#[derive(Debug, Clone, Copy)]
pub struct CellularCave {
    /// The solid tile.
    pub wall: TileId,
    /// The open tile.
    pub floor: TileId,
    /// Initial share of wall cells, in percent.
    pub fill_pct: u32,
    /// Smoothing rounds.
    pub rounds: u32,
    /// A cell becomes wall when at least this many of its eight neighbours
    /// are wall; out-of-bounds neighbours count as wall.
    pub threshold: u32,
}

impl Default for CellularCave {
    fn default() -> Self {
        Self { wall: TileId(0), floor: TileId(0), fill_pct: 45, rounds: 4, threshold: 5 }
    }
}

impl<C: BuildContext> Pass<C> for CellularCave {
    fn name(&self) -> &'static str {
        "cellular_cave"
    }
    fn phase(&self) -> Phase {
        Phase::Ground
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let (w, h) = (ctx.terrain().width(), ctx.terrain().height());
        let mut solid: Grid<bool> =
            Grid::from_fn(w, h, |p| p.x == 0 || p.y == 0 || p.x == w - 1 || p.y == h - 1 || ctx.rng().random_range(0..100) < self.fill_pct);
        let mut next = solid.clone();
        for _ in 0..self.rounds {
            for (p, cell) in next.iter_mut() {
                let walls = Steps::Eight.directions().iter().filter(|d| solid.get(p + d.offset()).copied().unwrap_or(true)).count() as u32;
                *cell = walls >= self.threshold;
            }
            solid.swap_with(&mut next);
        }
        let t = ctx.terrain_mut();
        for (p, s) in solid.iter() {
            t.set(p, if *s { self.wall } else { self.floor });
        }
        Ok(())
    }
}

/// Keeps only the largest connected open region, filling the rest with
/// `wall`. Fails if there is no open cell at all.
#[derive(Debug, Clone, Copy)]
pub struct KeepLargestRegion {
    /// The tile to fill discarded regions with.
    pub wall: TileId,
}

impl<C: BuildContext> Pass<C> for KeepLargestRegion {
    fn name(&self) -> &'static str {
        "keep_largest_region"
    }
    fn phase(&self) -> Phase {
        Phase::Connect
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        let terrain = ctx.terrain();
        let regions = rl_grid::region::label_regions(terrain, |i| tables.walkable[terrain.get_idx(i).index()], Steps::Eight);
        let keep = regions.largest().ok_or_else(|| BuildError::new("keep_largest_region", "no open cell to keep"))?;
        let t = ctx.terrain_mut();
        for idx in 0..t.len() {
            if let Some(label) = regions.label_idx(idx)
                && label != keep
            {
                t.set_idx(idx, self.wall);
            }
        }
        Ok(())
    }
}

/// Emits the first open cell found scanning outward from the centre, as a
/// [`StartPoint`]. Fails if there is none.
#[derive(Debug, Clone, Copy, Default)]
pub struct CentralStart;

/// Where a map's entrant should stand, emitted by [`CentralStart`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartPoint(pub Point);

impl<C: BuildContext> Pass<C> for CentralStart {
    fn name(&self) -> &'static str {
        "central_start"
    }
    fn phase(&self) -> Phase {
        Phase::Exits
    }
    fn apply(&self, ctx: &mut C) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        let center = ctx.terrain().bounds().center();
        let found =
            ctx.terrain().grid().nearest_from(center, |id| tables.walkable[id.index()]).ok_or_else(|| BuildError::new("central_start", "no walkable cell"))?;
        ctx.emit(StartPoint(found));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Chain;
    use crate::context::BaseContext;
    use rl_core::RunSeed;
    use rl_grid::TileRegistry;

    fn ctx() -> (BaseContext, TileId, TileId) {
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        (BaseContext::blank(40, 30, tiles, wall), wall, floor)
    }

    #[test]
    fn fill_and_border() {
        let (mut c, wall, floor) = ctx();
        Chain::new().then(Fill { tile: floor }).then(Border { tile: wall }).run(&mut c, RunSeed(1)).unwrap();
        assert_eq!(c.terrain().get(Point::new(0, 0)), Some(wall));
        assert_eq!(c.terrain().get(Point::new(5, 5)), Some(floor));
        assert_eq!(c.terrain().count(wall), 2 * 40 + 2 * 28);
    }

    #[test]
    fn scatter_hits_roughly_its_share_and_only_its_target() {
        let (mut c, wall, floor) = ctx();
        Chain::new()
            .then(Fill { tile: floor })
            .then(Border { tile: wall })
            .then(Scatter { name: "trees", tile: TileId(9), on: floor, chance_pct: 25 })
            .run(&mut c, RunSeed(3))
            .unwrap();
        let n = c.terrain().count(TileId(9));
        let candidates = 38 * 28;
        assert!((candidates / 6..candidates / 3).contains(&n), "{n} of {candidates}");
        assert_eq!(c.terrain().count(wall), 2 * 40 + 2 * 28, "the border was not touched");
    }

    #[test]
    fn a_cave_is_open_connected_and_deterministic() {
        let run = |seed: u64| {
            let (mut c, wall, floor) = ctx();
            Chain::new()
                .then(CellularCave { wall, floor, ..Default::default() })
                .then(KeepLargestRegion { wall })
                .then(CentralStart)
                .run(&mut c, RunSeed(seed))
                .unwrap();
            c
        };
        for seed in 1..=8 {
            let c = run(seed);
            let floor = c.tiles().expect("floor");
            let open = c.terrain().count(floor);
            assert!(open > 200, "seed {seed}: only {open} open cells");
            let tables = c.tiles().tables();
            let regions = rl_grid::region::label_regions(c.terrain(), |i| tables.walkable[c.terrain().get_idx(i).index()], Steps::Eight);
            assert_eq!(regions.count(), 1, "seed {seed}");
            let start = c.outputs().first::<StartPoint>().unwrap();
            assert_eq!(c.terrain().get(start.0), Some(floor));
        }
        assert_eq!(run(5).terrain(), run(5).terrain(), "same seed, same cave");
        assert_ne!(run(5).terrain(), run(6).terrain());
    }

    #[test]
    fn keep_largest_fails_on_a_solid_map() {
        let (mut c, wall, _) = ctx();
        let err = Chain::new().then(KeepLargestRegion { wall }).run(&mut c, RunSeed(1)).unwrap_err();
        assert_eq!(err.pass, "keep_largest_region");
    }
}
