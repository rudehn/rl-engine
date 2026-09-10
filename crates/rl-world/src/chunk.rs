//! The fine layer: a region's worth of tiles, generated on demand.
//!
//! A chunk is a pure function of the run seed, its region, and the eight
//! regions around it. Nothing about it is stored. Two neighbouring chunks
//! agree where a road or river crosses their shared edge because both
//! compute the crossing from the same unordered pair of regions.

use rand::rngs::StdRng;
use rl_core::{Direction, DirectionSet, Grid, Point, RunSeed, SeedDomain, geometry, seed};
use rl_core::Grid2D;
use rl_grid::{AStar, CostSource, PathRules, Terrain, TileId, TileRegistry};
use rl_mapgen::{BaseContext, BuildContext, BuildError, Chain, Outputs, Pass, Phase};

use crate::elevation::ReliefThresholds;
use crate::facts::{BandId, CellFacts};
use crate::graph::WorldGraph;
use crate::sites::SiteKindId;

/// Everything a chunk needs to know about one region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionFacts {
    /// Which region.
    pub region: Point,
    /// The game's band.
    pub band: BandId,
    /// The engine's facts.
    pub facts: CellFacts,
    /// The landmark here, if any.
    pub site: Option<SiteKindId>,
    /// Which ways a road leaves.
    pub roads: DirectionSet,
    /// Which ways a river enters and leaves.
    pub rivers: DirectionSet,
    /// Which way the river leaves, if one runs through.
    pub river_downstream: Option<Direction>,
}

/// A region together with the eight around it.
#[derive(Debug, Clone, PartialEq)]
pub struct Surroundings {
    /// The region the chunk is of.
    pub here: RegionFacts,
    neighbours: [Option<RegionFacts>; 8],
}

impl Surroundings {
    /// Builds a neighbourhood from a region and a lookup for those around it.
    pub fn new(here: RegionFacts, mut lookup: impl FnMut(Point) -> Option<RegionFacts>) -> Self {
        let mut neighbours = [None; 8];
        for d in Direction::ALL {
            neighbours[d.index()] = lookup(here.region + d.offset());
        }
        Self { here, neighbours }
    }

    /// The region this is a neighbourhood of.
    pub fn region(&self) -> Point {
        self.here.region
    }

    /// The neighbour in `d`, or `None` past the world edge.
    pub fn neighbour(&self, d: Direction) -> Option<&RegionFacts> {
        self.neighbours[d.index()].as_ref()
    }

    /// Every direction with a neighbour, and that neighbour, clockwise.
    pub fn neighbours(&self) -> impl Iterator<Item = (Direction, &RegionFacts)> {
        Direction::ALL.into_iter().filter_map(|d| self.neighbour(d).map(|n| (d, n)))
    }

    /// The directions whose neighbour satisfies `pred`, clockwise.
    pub fn directions_where(&self, pred: impl Fn(&RegionFacts) -> bool) -> DirectionSet {
        self.neighbours().filter(|(_, n)| pred(n)).map(|(d, _)| d).collect()
    }
}

/// Where a linear feature crosses the boundary between regions `a` and
/// `b`, as an offset along a boundary `span` cells long.
///
/// Both sides compute this from the same unordered pair, so they land on
/// the same cell without asking each other. Kept clear of the corners,
/// where a crossing would read as belonging to two edges.
pub fn crossing_offset(seam_seed: u64, a: Point, b: Point, span: i32) -> i32 {
    let hash = seed::pair_hash(seam_seed, a.tuple(), b.tuple());
    let margin = (span / 6).clamp(1, span.max(2) / 2);
    let free = (span - margin * 2).max(1);
    margin + seed::hash_below(hash, free as u32) as i32
}

impl WorldGraph {
    /// The seam seed every crossing in this world is derived from.
    pub fn seam_seed(&self) -> u64 {
        self.seed().derive(SeedDomain::new(b"world.seams"), 0)
    }

    /// Everything a chunk needs to know about region `p`.
    pub fn region_facts(&self, p: Point) -> Option<RegionFacts> {
        let layers = self.layers();
        let facts = layers.facts(p)?;
        Some(RegionFacts {
            region: p,
            band: layers.bands[p],
            facts,
            site: self.site_at(p).map(|s| s.kind),
            roads: self.roads().at(p),
            rivers: layers.hydrology.rivers[p],
            river_downstream: if layers.hydrology.is_river(p) { layers.hydrology.downstream[p] } else { None },
        })
    }

    /// Region `p` with its neighbours, or `None` off the map.
    pub fn surroundings(&self, p: Point) -> Option<Surroundings> {
        let here = self.region_facts(p)?;
        Some(Surroundings::new(here, |n| self.region_facts(n)))
    }

    /// The seed a region's chunk is built from: its own stream, so no
    /// chunk disturbs another or the world above.
    pub fn chunk_seed(&self, region: Point) -> RunSeed {
        let index = ((region.x as u32 as u64) << 32) | (region.y as u32 as u64);
        RunSeed(self.seed().derive(SeedDomain::new(b"chunk"), index))
    }

    /// Generates the chunk for `region` with the game's rules.
    pub fn build_chunk(&self, region: Point, rules: &(impl ChunkRules + ?Sized)) -> Result<(Terrain, Outputs), BuildError> {
        let around = self
            .surroundings(region)
            .ok_or_else(|| BuildError::new("chunk", format!("region {region:?} is off the world")))?;
        let size = self.region_size();
        let terrain = Terrain::filled(size, size, rules.fill(&around));
        let chain = rules.chain(self, &around);
        let origin = self.tile_origin(region);
        let elevation = &self.layers().elevation;
        let heights = Grid::from_fn(size, size, |p| elevation.height_at_tile(origin + p, size));
        let mut ctx = ChunkContext::new(terrain, rules.tiles().clone(), around, size, self.seam_seed())
            .with_heights(heights, elevation.sea_level, elevation.relief);
        chain.run(&mut ctx, self.chunk_seed(region))?;
        Ok(ctx.finish())
    }
}

/// What a game decides about a chunk.
pub trait ChunkRules {
    /// The registry chunk tiles refer to.
    fn tiles(&self) -> &TileRegistry;

    /// What a chunk of this region starts filled with.
    fn fill(&self, around: &Surroundings) -> TileId;

    /// The passes that build a chunk of this region. The world is there to
    /// read; the chunk's own height samples arrive in the context.
    fn chain(&self, world: &WorldGraph, around: &Surroundings) -> Chain<ChunkContext>;
}

/// The context chunk passes run over: a terrain, the neighbourhood, and
/// the height of every tile sampled from the same field the region was.
#[derive(Debug)]
pub struct ChunkContext {
    base: BaseContext,
    /// The region and its neighbours.
    pub around: Surroundings,
    /// Normalized height per tile, `[0, 1]` across the whole world.
    pub heights: Grid<f32>,
    /// Heights at or below this are water.
    pub sea_level: f32,
    /// Where land turns to hill, mountain and summit, in land-height units.
    pub relief: ReliefThresholds,
    region_size: i32,
    seam_seed: u64,
}

impl ChunkContext {
    /// A context for one chunk with flat heights; see
    /// [`with_heights`](Self::with_heights).
    pub fn new(terrain: Terrain, tiles: TileRegistry, around: Surroundings, region_size: i32, seam_seed: u64) -> Self {
        Self {
            base: BaseContext::new(terrain, tiles),
            around,
            heights: Grid::filled(region_size, region_size, 0.5),
            sea_level: 0.0,
            relief: ReliefThresholds {
                hill: 1.0,
                mountain: 1.0,
                peak: 1.0,
            },
            region_size,
            seam_seed,
        }
    }

    /// Installs the tile heights and the world's thresholds.
    pub fn with_heights(mut self, heights: Grid<f32>, sea_level: f32, relief: ReliefThresholds) -> Self {
        self.heights = heights;
        self.sea_level = sea_level;
        self.relief = relief;
        self
    }

    /// Whether the tile at `p` is under the sea.
    pub fn is_sea(&self, p: Point) -> bool {
        self.heights.get(p).is_none_or(|h| *h <= self.sea_level)
    }

    /// Height above sea level at `p`, summit at 1, zero over water.
    pub fn land_height(&self, p: Point) -> f32 {
        let h = self.heights.get(p).copied().unwrap_or(0.0);
        let span = 1.0 - self.sea_level;
        if span <= f32::EPSILON { 0.0 } else { ((h - self.sea_level) / span).max(0.0) }
    }

    /// Tiles per side.
    pub fn region_size(&self) -> i32 {
        self.region_size
    }

    /// The middle tile.
    pub fn center(&self) -> Point {
        Point::new(self.region_size / 2, self.region_size / 2)
    }

    /// The cell on this chunk's edge where a feature crossing toward the
    /// neighbour in `d` lands. Diagonals land on the corner.
    pub fn edge_cell(&self, d: Direction) -> Point {
        let last = self.region_size - 1;
        let here = self.around.region();
        let offset = crossing_offset(self.seam_seed, here, here + d.offset(), self.region_size);
        match d {
            Direction::North => Point::new(offset, 0),
            Direction::South => Point::new(offset, last),
            Direction::East => Point::new(last, offset),
            Direction::West => Point::new(0, offset),
            Direction::NorthEast => Point::new(last, 0),
            Direction::SouthEast => Point::new(last, last),
            Direction::SouthWest => Point::new(0, last),
            Direction::NorthWest => Point::new(0, 0),
        }
    }

    /// Takes the finished terrain and outputs apart.
    pub fn finish(self) -> (Terrain, Outputs) {
        self.base.finish()
    }
}

impl BuildContext for ChunkContext {
    fn terrain(&self) -> &Terrain {
        self.base.terrain()
    }
    fn terrain_mut(&mut self) -> &mut Terrain {
        self.base.terrain_mut()
    }
    fn tiles(&self) -> &TileRegistry {
        self.base.tiles()
    }
    fn rng(&mut self) -> &mut StdRng {
        self.base.rng()
    }
    fn set_rng(&mut self, rng: StdRng) {
        self.base.set_rng(rng)
    }
    fn emit<T: std::any::Any + Send>(&mut self, value: T) {
        self.base.emit(value)
    }
    fn outputs(&self) -> &Outputs {
        self.base.outputs()
    }
    fn outputs_mut(&mut self) -> &mut Outputs {
        self.base.outputs_mut()
    }
    fn take_snapshot(&mut self) {
        self.base.take_snapshot()
    }
}

/// The ground a linear feature is routed over inside a chunk: every tile
/// passable, priced by height and a little position-keyed wander so a
/// road leans and a river finds the valley.
struct Going<'a> {
    heights: &'a Grid<f32>,
    sea_level: f32,
    seed: u64,
    /// Cost per unit of land height, so rivers (high) hug low ground and
    /// roads (low) do not mind.
    height_weight: f32,
    /// Range of the per-cell wander added on top.
    wander: u32,
}

impl Grid2D for Going<'_> {
    fn width(&self) -> i32 {
        self.heights.width()
    }
    fn height(&self) -> i32 {
        self.heights.height()
    }
}

impl CostSource for Going<'_> {
    fn cost_idx(&self, idx: usize) -> Option<u32> {
        let p = self.heights.idx_point(idx);
        // The border is off limits to the search: a route that walked
        // along the edge would paint cells the neighbouring chunk never
        // agreed to. Crossings are appended by hand, one step out.
        if self.heights.bounds().is_border(p) {
            return None;
        }
        let h = self.heights[idx];
        let land = ((h - self.sea_level) / (1.0 - self.sea_level).max(f32::EPSILON)).max(0.0);
        let wander = seed::hash_below(seed::position_hash(self.seed, p.x, p.y), self.wander.max(1));
        Some(100 + (land * self.height_weight) as u32 + wander)
    }
}

/// The cell one step inside the chunk from a border cell, or the cell
/// itself if it is not on the border.
fn inset(bounds: rl_core::Rect, p: Point) -> Point {
    let mut q = p;
    if p.x == bounds.x {
        q.x += 1;
    } else if p.x == bounds.right() - 1 {
        q.x -= 1;
    }
    if p.y == bounds.y {
        q.y += 1;
    } else if p.y == bounds.bottom() - 1 {
        q.y -= 1;
    }
    q
}

/// The cells of a route from `from` to `to`, both included, four-connected
/// so a feature never slips diagonally between two tiles. Border cells
/// are only ever the first or last cell, so a crossing is exactly the cell
/// the neighbouring chunk expects.
fn route_in_chunk(ctx: &ChunkContext, from: Point, to: Point, height_weight: f32, wander: u32, salt: u64) -> Vec<Point> {
    let going = Going {
        heights: &ctx.heights,
        sea_level: ctx.sea_level,
        seed: ctx.seam_seed ^ salt,
        height_weight,
        wander,
    };
    let bounds = ctx.heights.bounds();
    let (a, b) = (inset(bounds, from), inset(bounds, to));
    let mut cells = Vec::new();
    if a != from {
        cells.push(from);
    }
    cells.push(a);
    match AStar::new().find(&going, a, b, PathRules::CARDINAL) {
        Some(path) => cells.extend(path.steps),
        None => cells.extend(geometry::line(a, b).skip(1)),
    }
    if b != to {
        cells.push(to);
    }
    cells
}

fn paint_cells(terrain: &mut Terrain, cells: &[Point], radius: i32, tile: TileId) {
    for p in cells {
        if radius == 0 {
            terrain.set(*p, tile);
        } else {
            for q in geometry::disc(*p, radius) {
                terrain.set(q, tile);
            }
        }
    }
}

/// Paves every road leaving the region from the centre to its crossing
/// cell, so roads meet in a junction at the middle. Routes are searched
/// over the tile heights with a little wander, so a road bends around a
/// rise rather than cutting a straight line.
#[derive(Debug, Clone, Copy)]
pub struct RoadPave {
    /// The road tile.
    pub tile: TileId,
}

impl Pass<ChunkContext> for RoadPave {
    fn name(&self) -> &'static str {
        "road_pave"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut ChunkContext) -> Result<(), BuildError> {
        let center = ctx.center();
        for d in ctx.around.here.roads.iter() {
            let edge = ctx.edge_cell(d);
            let cells = route_in_chunk(ctx, center, edge, 120.0, 60, 0x0AD5);
            paint_cells(ctx.terrain_mut(), &cells, 0, self.tile);
        }
        Ok(())
    }
}

/// Carves the river through the region: every upstream crossing runs to
/// the centre, and the centre runs on to the downstream crossing, each
/// searched over the tile heights so the channel keeps to low ground.
/// Width follows the river's width class.
#[derive(Debug, Clone, Copy)]
pub struct RiverChannel {
    /// The water tile.
    pub water: TileId,
}

impl Pass<ChunkContext> for RiverChannel {
    fn name(&self) -> &'static str {
        "river_channel"
    }
    fn phase(&self) -> Phase {
        Phase::Ground
    }
    fn apply(&self, ctx: &mut ChunkContext) -> Result<(), BuildError> {
        let here = ctx.around.here;
        if here.rivers.is_empty() {
            return Ok(());
        }
        let radius = (here.facts.river_width.max(1) as i32) - 1;
        // The river bends through the lowest ground near the middle rather
        // than the exact centre, so two rivers through neighbouring regions
        // do not all kink at the same spot.
        let center = ctx.center();
        let low = ctx
            .heights
            .nearest_from(center, |h| *h <= ctx.sea_level)
            .filter(|p| geometry::chebyshev(*p, center) < ctx.region_size() / 4)
            .unwrap_or(center);
        for d in here.rivers.iter() {
            if Some(d) == here.river_downstream {
                continue;
            }
            let edge = ctx.edge_cell(d);
            let cells = route_in_chunk(ctx, edge, low, 900.0, 30, 0x51E5);
            paint_cells(ctx.terrain_mut(), &cells, radius, self.water);
        }
        if let Some(d) = here.river_downstream {
            let edge = ctx.edge_cell(d);
            let cells = route_in_chunk(ctx, low, edge, 900.0, 30, 0x51E5);
            paint_cells(ctx.terrain_mut(), &cells, radius, self.water);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::demo::{Demo, SEA};
    use rl_core::Grid2D;
    use crate::graph::{WorldConfig, WorldGraph};
    use rl_mapgen::passes::Fill;

    struct Chunks {
        tiles: TileRegistry,
    }

    impl Chunks {
        fn new() -> Self {
            let mut tiles = TileRegistry::standard();
            tiles.register(rl_grid::TileProps::named("water").opaque(false)).unwrap();
            tiles.register(rl_grid::TileProps::floor("road")).unwrap();
            Self { tiles }
        }
    }

    impl ChunkRules for Chunks {
        fn tiles(&self) -> &TileRegistry {
            &self.tiles
        }
        fn fill(&self, around: &Surroundings) -> TileId {
            if around.here.band == SEA { self.tiles.expect("water") } else { self.tiles.expect("floor") }
        }
        fn chain(&self, _: &WorldGraph, _: &Surroundings) -> Chain<ChunkContext> {
            Chain::new()
                .then(Fill { tile: self.tiles.expect("floor") })
                .then(RiverChannel { water: self.tiles.expect("water") })
                .then(RoadPave { tile: self.tiles.expect("road") })
        }
    }

    fn world() -> WorldGraph {
        WorldGraph::generate(RunSeed(3), WorldConfig { region_size: 32, ..WorldConfig::regions(40, 30) }, &Demo)
    }

    #[test]
    fn crossing_offsets_agree_from_both_sides_and_avoid_corners() {
        for i in 0..200 {
            let a = Point::new(i % 17, i / 17);
            let b = a + Direction::East.offset();
            let x = crossing_offset(99, a, b, 64);
            assert_eq!(x, crossing_offset(99, b, a, 64));
            assert!((10..54).contains(&x), "{x}");
        }
        assert_eq!(crossing_offset(1, Point::new(0, 0), Point::new(1, 0), 2), 1);
    }

    #[test]
    fn neighbouring_chunks_agree_at_every_shared_edge_tile() {
        let w = world();
        let rules = Chunks::new();
        let mut checked = 0;
        for (p, set) in w.roads().directions().iter() {
            let d = match set.iter().find(|d| matches!(d, Direction::East | Direction::South)) {
                Some(d) => d,
                None => continue,
            };
            let q = p + d.offset();
            let (a, _) = w.build_chunk(p, &rules).unwrap();
            let (b, _) = w.build_chunk(q, &rules).unwrap();
            let n = w.region_size();
            for i in 0..n {
                let (pa, pb) = match d {
                    Direction::East => (Point::new(n - 1, i), Point::new(0, i)),
                    _ => (Point::new(i, n - 1), Point::new(i, 0)),
                };
                assert_eq!(a.get(pa), b.get(pb), "regions {p:?}/{q:?} disagree at {pa:?}");
            }
            checked += 1;
            if checked >= 12 {
                break;
            }
        }
        assert!(checked > 0, "no road seam to check");
    }

    #[test]
    fn a_river_region_carries_water_across_the_chunk() {
        let w = world();
        let rules = Chunks::new();
        let (p, _) = w
            .layers()
            .hydrology
            .rivers
            .iter()
            .find(|(_, s)| s.len() >= 2)
            .expect("a river cell with an upstream and a downstream");
        let (terrain, _) = w.build_chunk(p, &rules).unwrap();
        let water = rules.tiles.expect("water");
        assert!(terrain.count(water) >= w.region_size() as usize);
        let facts = w.region_facts(p).unwrap();
        for d in facts.rivers.iter() {
            let edge = ChunkContext::new(Terrain::filled(1, 1, TileId(0)), rules.tiles.clone(), w.surroundings(p).unwrap(), w.region_size(), w.seam_seed()).edge_cell(d);
            assert_eq!(terrain.get(edge), Some(water), "no water at the {d:?} crossing");
        }
    }

    #[test]
    fn chunks_are_deterministic_and_off_map_regions_fail() {
        let w = world();
        let rules = Chunks::new();
        let p = Point::new(10, 10);
        let (a, _) = w.build_chunk(p, &rules).unwrap();
        let (b, _) = w.build_chunk(p, &rules).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.width(), 32);
        assert!(w.build_chunk(Point::new(-1, 0), &rules).is_err());
        assert_ne!(w.chunk_seed(Point::new(1, 2)), w.chunk_seed(Point::new(2, 1)));
    }
}
