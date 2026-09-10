//! The loaded part of the world, streamed as the player moves.
//!
//! The whole world never exists as tiles. A window of regions around the
//! player is generated on demand and kept; regions that leave the window
//! are dropped, and only their edits survive, as a delta replayed when
//! they load again. Everything that reads tiles reads through
//! [`WorldMap`], in world coordinates, and gets `None` outside the window.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_core::{Grid2D, Point, Rect};
use rl_grid::{CostSource, OpacitySource, TileId, TileTables};
use rl_mapgen::Outputs;
use rl_world::{ChunkRules, WorldGraph};

use crate::components::{Player, Position, Viewshed};

/// The coarse world.
#[derive(Resource, Deref)]
pub struct WorldRes(pub WorldGraph);

/// How the game builds chunks.
#[derive(Resource)]
pub struct ChunkRulesRes(pub Box<dyn ChunkRules + Send + Sync>);

/// How much of the world stays loaded.
#[derive(Resource, Debug, Clone, Copy)]
pub struct WorldSettings {
    /// Regions kept loaded on each side of the player's region. 1 gives a
    /// 3x3 window, 2 a 5x5.
    pub window_radius: i32,
}

impl Default for WorldSettings {
    fn default() -> Self {
        Self { window_radius: 1 }
    }
}

/// One loaded region's tiles, and whether they differ from generation.
#[derive(Debug, Clone)]
struct Chunk {
    terrain: rl_grid::Terrain,
    delta: Vec<(usize, TileId)>,
}

/// Every loaded chunk, addressable in world tile coordinates.
#[derive(Resource, Debug)]
pub struct WorldMap {
    region_size: i32,
    window: Rect,
    chunks: Vec<Option<Chunk>>,
    deltas: BTreeMap<Point, Vec<(usize, TileId)>>,
    tables: TileTables,
    /// Bumped every time the window moves, so viewsheds know to recompute.
    generation: u64,
}

impl WorldMap {
    /// An empty map with no window loaded.
    pub fn new(region_size: i32, tables: TileTables) -> Self {
        Self {
            region_size,
            window: Rect::new(0, 0, 0, 0),
            chunks: Vec::new(),
            deltas: BTreeMap::new(),
            tables,
            generation: 0,
        }
    }

    /// Tiles per region.
    pub fn region_size(&self) -> i32 {
        self.region_size
    }

    /// The loaded window, in regions.
    pub fn window(&self) -> Rect {
        self.window
    }

    /// The loaded window, in world tiles.
    pub fn window_tiles(&self) -> Rect {
        let s = self.region_size;
        Rect::new(self.window.x * s, self.window.y * s, self.window.width * s, self.window.height * s)
    }

    /// Changes whenever the window moves.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The flag tables tiles are read through.
    pub fn tables(&self) -> &TileTables {
        &self.tables
    }

    /// The region a world tile lies in.
    pub fn region_of(&self, p: Point) -> Point {
        Point::new(p.x.div_euclid(self.region_size), p.y.div_euclid(self.region_size))
    }

    fn chunk_slot(&self, region: Point) -> Option<usize> {
        self.window
            .contains(region)
            .then(|| ((region.y - self.window.y) * self.window.width + (region.x - self.window.x)) as usize)
    }

    fn locate(&self, p: Point) -> Option<(usize, usize)> {
        let region = self.region_of(p);
        let slot = self.chunk_slot(region)?;
        let s = self.region_size;
        let local = (p.y.rem_euclid(s) * s + p.x.rem_euclid(s)) as usize;
        Some((slot, local))
    }

    /// The tile at world `p`, or `None` outside the loaded window.
    pub fn tile(&self, p: Point) -> Option<TileId> {
        let (slot, local) = self.locate(p)?;
        self.chunks[slot].as_ref().map(|c| c.terrain.get_idx(local))
    }

    /// Sets the tile at world `p`, recording the edit so it survives the
    /// chunk being unloaded. Returns whether `p` was loaded.
    pub fn set_tile(&mut self, p: Point, id: TileId) -> bool {
        let Some((slot, local)) = self.locate(p) else { return false };
        let Some(chunk) = self.chunks[slot].as_mut() else { return false };
        if chunk.terrain.get_idx(local) == id {
            return true;
        }
        chunk.terrain.set_idx(local, id);
        chunk.delta.push((local, id));
        true
    }

    /// Whether an actor can stand on `p` now.
    pub fn is_walkable(&self, p: Point) -> bool {
        self.tile(p).is_some_and(|t| self.tables.walkable[t.index()])
    }

    /// Whether `p` blocks sight. Unloaded tiles do.
    pub fn is_opaque(&self, p: Point) -> bool {
        self.tile(p).is_none_or(|t| self.tables.opaque[t.index()])
    }

    /// Entry cost of `p`, or `None` if not walkable or not loaded.
    pub fn cost(&self, p: Point) -> Option<u32> {
        let t = self.tile(p)?;
        self.tables.walkable[t.index()].then(|| self.tables.move_cost[t.index()])
    }

    /// Whether `p` is inside the loaded window.
    pub fn is_loaded(&self, p: Point) -> bool {
        self.locate(p).is_some_and(|(slot, _)| self.chunks[slot].is_some())
    }

    /// World coordinates of window-local `local`.
    pub fn to_world(&self, local: Point) -> Point {
        local + self.window_tiles().origin()
    }

    /// Window-local coordinates of world `p`, or `None` outside the window.
    pub fn to_local(&self, p: Point) -> Option<Point> {
        let w = self.window_tiles();
        w.contains(p).then(|| p - w.origin())
    }

    /// A view of the window that grid algorithms can run over, in
    /// window-local coordinates.
    pub fn view(&self) -> WindowView<'_> {
        WindowView { map: self }
    }

    /// Moves the window to cover `regions`, generating what is new and
    /// dropping what left. Edits to dropped chunks are kept as deltas.
    pub fn load_window(&mut self, regions: Rect, world: &WorldGraph, rules: &dyn ChunkRules) -> Result<Vec<Point>, rl_mapgen::BuildError> {
        if regions == self.window && !self.chunks.is_empty() {
            return Ok(Vec::new());
        }
        let mut next: Vec<Option<Chunk>> = Vec::with_capacity(regions.area().max(0) as usize);
        let mut loaded = Vec::new();
        let mut kept = 0;
        for region in regions.cells() {
            match self.chunk_slot(region).and_then(|slot| self.chunks[slot].take()) {
                Some(chunk) => {
                    kept += 1;
                    next.push(Some(chunk));
                }
                None => {
                    if !world.layers().contains(region) {
                        next.push(None);
                        continue;
                    }
                    let (mut terrain, _outputs): (rl_grid::Terrain, Outputs) = world.build_chunk(region, rules)?;
                    let delta = self.deltas.remove(&region).unwrap_or_default();
                    for (idx, id) in &delta {
                        terrain.set_idx(*idx, *id);
                    }
                    next.push(Some(Chunk { terrain, delta }));
                    loaded.push(region);
                }
            }
        }
        // Whatever was not taken has left the window.
        for (slot, chunk) in self.chunks.drain(..).enumerate() {
            if let Some(chunk) = chunk
                && !chunk.delta.is_empty()
            {
                let region = self.window.origin() + Point::new(slot as i32 % self.window.width, slot as i32 / self.window.width);
                self.deltas.insert(region, chunk.delta);
            }
        }
        let _ = kept;
        self.window = regions;
        self.chunks = next;
        self.generation += 1;
        Ok(loaded)
    }

    /// Number of regions currently loaded.
    pub fn loaded_count(&self) -> usize {
        self.chunks.iter().filter(|c| c.is_some()).count()
    }

    /// Number of unloaded regions with edits kept.
    pub fn stored_deltas(&self) -> usize {
        self.deltas.len()
    }
}

/// The loaded window as a grid in window-local coordinates.
#[derive(Debug, Clone, Copy)]
pub struct WindowView<'a> {
    map: &'a WorldMap,
}

impl WindowView<'_> {
    /// The map this views.
    pub fn map(&self) -> &WorldMap {
        self.map
    }

    fn world(&self, idx: usize) -> Point {
        self.map.to_world(self.idx_point(idx))
    }
}

impl Grid2D for WindowView<'_> {
    fn width(&self) -> i32 {
        self.map.window_tiles().width
    }

    fn height(&self) -> i32 {
        self.map.window_tiles().height
    }
}

impl OpacitySource for WindowView<'_> {
    fn is_opaque_idx(&self, idx: usize) -> bool {
        self.map.is_opaque(self.world(idx))
    }
}

impl CostSource for WindowView<'_> {
    fn cost_idx(&self, idx: usize) -> Option<u32> {
        self.map.cost(self.world(idx))
    }
}

/// A region's chunk was generated and entered the window. Fires once per
/// load, so a game can populate a region the first time it sees it and
/// remember not to again.
#[derive(Message, Debug, Clone, Copy)]
pub struct ChunkLoaded {
    /// Which region.
    pub region: Point,
}

/// The window the player should have loaded.
pub fn desired_window(player_region: Point, radius: i32, world: &WorldGraph) -> Rect {
    let wanted = Rect::new(player_region.x - radius, player_region.y - radius, 2 * radius + 1, 2 * radius + 1);
    // Clamp to the world so the window never asks for regions that do not
    // exist; the world's border is ocean, so nothing walkable is lost.
    let bounds = Rect::new(0, 0, world.width(), world.height());
    wanted.intersection(&bounds).unwrap_or(wanted)
}

/// Keeps the loaded window centred on the player and marks every viewshed
/// stale when it moves.
pub fn stream_chunks(
    mut map: ResMut<WorldMap>,
    world: Res<WorldRes>,
    rules: Res<ChunkRulesRes>,
    settings: Res<WorldSettings>,
    player: Query<&Position, With<Player>>,
    mut viewsheds: Query<&mut Viewshed>,
    mut loaded: MessageWriter<ChunkLoaded>,
) {
    let Ok(pos) = player.single() else { return };
    let region = map.region_of(pos.0);
    let wanted = desired_window(region, settings.window_radius, &world);
    if wanted == map.window() && !map.chunks.is_empty() {
        return;
    }
    match map.load_window(wanted, &world, rules.0.as_ref()) {
        Err(e) => {
            error!("chunk generation failed: {e}");
            return;
        }
        Ok(regions) => {
            for region in regions {
                loaded.write(ChunkLoaded { region });
            }
        }
    }
    for mut v in &mut viewsheds {
        v.dirty = true;
    }
}
