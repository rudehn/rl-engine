//! A grid of tile ids, and the two questions algorithms ask of it.

use rl_core::{Grid, Grid2D, Point};
use serde::{Deserialize, Serialize};

use crate::tile::{TileId, TileRegistry, TileTables};

/// The terrain of one map or chunk: which tile is where.
///
/// Holds ids only. Semantics come from the [`TileRegistry`] through a
/// [`TerrainView`], so the same terrain can be read under different rules
/// (a swimmer's costs, a smoke overlay, a viewer's knowledge) without
/// copying it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Terrain {
    tiles: Grid<TileId>,
}

impl Terrain {
    /// A terrain of `width` by `height` filled with `fill`.
    pub fn filled(width: i32, height: i32, fill: TileId) -> Self {
        Self {
            tiles: Grid::filled(width, height, fill),
        }
    }

    /// A terrain built by evaluating `f` at every cell.
    pub fn from_fn(width: i32, height: i32, f: impl FnMut(Point) -> TileId) -> Self {
        Self {
            tiles: Grid::from_fn(width, height, f),
        }
    }

    /// The tile at `p`, or `None` if out of bounds.
    pub fn get(&self, p: Point) -> Option<TileId> {
        self.tiles.get(p).copied()
    }

    /// The tile at a flat index.
    pub fn get_idx(&self, idx: usize) -> TileId {
        self.tiles[idx]
    }

    /// Sets the tile at `p`. Returns whether it was in bounds.
    pub fn set(&mut self, p: Point, id: TileId) -> bool {
        self.tiles.set(p, id)
    }

    /// Sets the tile at a flat index.
    pub fn set_idx(&mut self, idx: usize, id: TileId) {
        self.tiles[idx] = id;
    }

    /// Sets every cell to `id`.
    pub fn fill(&mut self, id: TileId) {
        self.tiles.fill(id);
    }

    /// The underlying grid.
    pub fn grid(&self) -> &Grid<TileId> {
        &self.tiles
    }

    /// The underlying grid, mutably.
    pub fn grid_mut(&mut self) -> &mut Grid<TileId> {
        &mut self.tiles
    }

    /// Iterates `(point, tile)` row-major.
    pub fn iter(&self) -> impl Iterator<Item = (Point, TileId)> + '_ {
        self.tiles.iter().map(|(p, t)| (p, *t))
    }

    /// How many cells hold `id`.
    pub fn count(&self, id: TileId) -> usize {
        self.tiles.cells().iter().filter(|t| **t == id).count()
    }

    /// A view that answers opacity and cost questions from `registry`.
    pub fn view<'a>(&'a self, registry: &TileRegistry) -> TerrainView<'a> {
        TerrainView {
            terrain: self,
            tables: registry.tables(),
        }
    }
}

impl Grid2D for Terrain {
    fn width(&self) -> i32 {
        self.tiles.width()
    }

    fn height(&self) -> i32 {
        self.tiles.height()
    }
}

/// Answers "does this cell block sight" for FOV and lighting.
///
/// Out-of-bounds cells are opaque, so scans stop at the edge.
pub trait OpacitySource: Grid2D {
    /// Whether the in-bounds cell at `idx` blocks sight.
    fn is_opaque_idx(&self, idx: usize) -> bool;

    /// Whether `p` blocks sight; out of bounds counts as opaque.
    fn is_opaque(&self, p: Point) -> bool {
        match self.checked_idx(p) {
            Some(idx) => self.is_opaque_idx(idx),
            None => true,
        }
    }
}

/// Answers "what does it cost to enter this cell" for pathfinding.
///
/// `None` means impassable. Costs are in hundredths of a normal step, the
/// same unit as the turn clock, so a path's cost is the time it takes.
pub trait CostSource: Grid2D {
    /// The entry cost of the in-bounds cell at `idx`, or `None` if impassable.
    fn cost_idx(&self, idx: usize) -> Option<u32>;

    /// The entry cost of `p`; out of bounds is impassable.
    fn cost(&self, p: Point) -> Option<u32> {
        self.checked_idx(p).and_then(|idx| self.cost_idx(idx))
    }

    /// Whether `p` can be entered at all.
    fn is_passable(&self, p: Point) -> bool {
        self.cost(p).is_some()
    }
}

/// A terrain read through a registry's flag tables.
///
/// This is the plain reading: walkable tiles cost their move cost, opaque
/// tiles block sight. Wrap it in a newtype and override one method to
/// overlay something on top.
///
/// ```
/// use rl_core::{Grid2D, Point};
/// use rl_grid::{CostSource, Terrain, TerrainView, TileRegistry};
///
/// /// A pather that refuses to enter a set of hazardous cells.
/// struct AvoidHazards<'a> { base: TerrainView<'a>, hazards: &'a [usize] }
///
/// impl Grid2D for AvoidHazards<'_> {
///     fn width(&self) -> i32 { self.base.width() }
///     fn height(&self) -> i32 { self.base.height() }
/// }
///
/// impl CostSource for AvoidHazards<'_> {
///     fn cost_idx(&self, idx: usize) -> Option<u32> {
///         if self.hazards.contains(&idx) { None } else { self.base.cost_idx(idx) }
///     }
/// }
///
/// let registry = TileRegistry::standard();
/// let terrain = Terrain::filled(3, 3, registry.expect("floor"));
/// let view = AvoidHazards { base: terrain.view(&registry), hazards: &[4] };
/// assert!(view.is_passable(Point::new(0, 0)));
/// assert!(!view.is_passable(Point::new(1, 1)));
/// ```
#[derive(Debug, Clone)]
pub struct TerrainView<'a> {
    terrain: &'a Terrain,
    tables: TileTables,
}

impl<'a> TerrainView<'a> {
    /// The terrain being viewed.
    pub fn terrain(&self) -> &'a Terrain {
        self.terrain
    }

    /// The flag tables in use.
    pub fn tables(&self) -> &TileTables {
        &self.tables
    }

    /// Whether the cell at `idx` is walkable now.
    pub fn is_walkable_idx(&self, idx: usize) -> bool {
        self.tables.walkable[self.terrain.get_idx(idx).index()]
    }

    /// Whether `p` is walkable now; out of bounds is not.
    pub fn is_walkable(&self, p: Point) -> bool {
        self.checked_idx(p).is_some_and(|i| self.is_walkable_idx(i))
    }

    /// Whether the cell at `idx` is passable eventually.
    pub fn is_passable_eventually_idx(&self, idx: usize) -> bool {
        self.tables.passable[self.terrain.get_idx(idx).index()]
    }

    /// Whether the cell at `idx` stops projectiles.
    pub fn blocks_projectiles_idx(&self, idx: usize) -> bool {
        self.tables.blocks_projectiles[self.terrain.get_idx(idx).index()]
    }
}

impl Grid2D for TerrainView<'_> {
    fn width(&self) -> i32 {
        self.terrain.width()
    }

    fn height(&self) -> i32 {
        self.terrain.height()
    }
}

impl OpacitySource for TerrainView<'_> {
    fn is_opaque_idx(&self, idx: usize) -> bool {
        self.tables.opaque[self.terrain.get_idx(idx).index()]
    }
}

impl CostSource for TerrainView<'_> {
    fn cost_idx(&self, idx: usize) -> Option<u32> {
        let id = self.terrain.get_idx(idx).index();
        self.tables.walkable[id].then(|| self.tables.move_cost[id])
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    //! ASCII map fixtures shared by the algorithm tests.
    use super::*;

    /// Parses rows of `#` (wall) and `.` (floor); `~` is a slow floor.
    pub fn parse(rows: &[&str]) -> (Terrain, TileRegistry) {
        let mut registry = TileRegistry::standard();
        let slow = registry.register(crate::tile::TileProps::floor("mud").move_cost(300)).unwrap();
        let wall = registry.expect("wall");
        let floor = registry.expect("floor");
        let height = rows.len() as i32;
        let width = rows[0].len() as i32;
        let terrain = Terrain::from_fn(width, height, |p| {
            match rows[p.y as usize].as_bytes()[p.x as usize] {
                b'#' => wall,
                b'~' => slow,
                _ => floor,
            }
        });
        (terrain, registry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_reads_flags_through_the_registry() {
        let (terrain, registry) = fixtures::parse(&["#.#", "#~#", "###"]);
        let view = terrain.view(&registry);
        assert!(view.is_opaque(Point::new(0, 0)));
        assert!(!view.is_opaque(Point::new(1, 0)));
        assert!(view.is_opaque(Point::new(-1, 0)), "out of bounds is opaque");
        assert_eq!(view.cost(Point::new(1, 0)), Some(100));
        assert_eq!(view.cost(Point::new(1, 1)), Some(300));
        assert_eq!(view.cost(Point::new(0, 0)), None);
        assert_eq!(view.cost(Point::new(9, 9)), None);
        assert!(view.is_walkable(Point::new(1, 0)));
        assert!(!view.is_walkable(Point::new(5, 5)));
    }

    #[test]
    fn terrain_edits_and_counts() {
        let registry = TileRegistry::standard();
        let mut t = Terrain::filled(4, 3, registry.expect("wall"));
        assert!(t.set(Point::new(1, 1), registry.expect("floor")));
        assert!(!t.set(Point::new(4, 1), registry.expect("floor")));
        assert_eq!(t.count(registry.expect("floor")), 1);
        assert_eq!(t.get(Point::new(1, 1)), Some(registry.expect("floor")));
        assert_eq!(t.get(Point::new(4, 4)), None);
        assert_eq!(t.len(), 12);
    }
}
