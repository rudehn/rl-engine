//! The loaded part of the world, streamed as the player moves.
//!
//! The whole world never exists as tiles. A window of regions around the
//! player is generated on demand and kept; regions that leave the window
//! are dropped, and only their edits survive, as a delta replayed when
//! they load again. Everything that reads tiles reads through
//! [`WorldMap`], in world coordinates, and gets `None` outside the window.
//!
//! While the player is in a place (see [`places`](crate::places)), the
//! same readers read the place's bounded terrain instead, in the place's
//! own coordinates, and the surface window waits untouched underneath.
//!
//! The surface owns everything regional, and it is optional. A game with
//! no world graph, a dungeon delve, never names a region size:
//! [`WorldMap::new`] takes the tile tables and nothing else, and how big
//! a region is stays the world graph's business, read from it when the
//! first window loads.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_core::{Grid2D, Point, Rect};
use rl_grid::{BitGrid, CostSource, OpacitySource, TileId, TileTables};
use rl_mapgen::Outputs;
use rl_world::{ChunkRules, WorldGraph};

use crate::components::{Player, Position, Viewshed};
use crate::places::{MapId, PlaceBuild, Spot};

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

/// The streamed surface: the window of chunks around the player, and the
/// edits of chunks that have left it.
///
/// Everything that counts in regions lives here, which is what keeps
/// regions out of a delve's vocabulary: with no world graph nothing ever
/// loads, and every method answers `None`.
#[derive(Debug)]
struct Surface {
    /// Tiles per region, learned from the world graph the first time a
    /// window loads. `None` until then, and forever in a delve.
    region_size: Option<i32>,
    window: Rect,
    chunks: Vec<Option<Chunk>>,
    deltas: BTreeMap<Point, Vec<(usize, TileId)>>,
}

impl Default for Surface {
    fn default() -> Self {
        Self { region_size: None, window: Rect::new(0, 0, 0, 0), chunks: Vec::new(), deltas: BTreeMap::new() }
    }
}

impl Surface {
    /// Whether a window has ever loaded. A window of nothing but ocean
    /// still counts, so an all-unbuilt window is not reloaded every frame.
    fn is_windowed(&self) -> bool {
        !self.chunks.is_empty()
    }

    fn slot(&self, region: Point) -> Option<usize> {
        self.window.contains(region).then(|| ((region.y - self.window.y) * self.window.width + (region.x - self.window.x)) as usize)
    }

    fn locate(&self, p: Point) -> Option<(usize, usize)> {
        let s = self.region_size?;
        let slot = self.slot(Point::new(p.x.div_euclid(s), p.y.div_euclid(s)))?;
        let local = (p.y.rem_euclid(s) * s + p.x.rem_euclid(s)) as usize;
        Some((slot, local))
    }

    fn tile(&self, p: Point) -> Option<TileId> {
        let (slot, local) = self.locate(p)?;
        self.chunks[slot].as_ref().map(|c| c.terrain.get_idx(local))
    }

    /// Writes a tile and records the edit, answering what was there, or
    /// `None` if `p` is not loaded.
    fn set_tile(&mut self, p: Point, id: TileId) -> Option<TileId> {
        let (slot, local) = self.locate(p)?;
        let chunk = self.chunks[slot].as_mut()?;
        let was = chunk.terrain.get_idx(local);
        if was != id {
            chunk.terrain.set_idx(local, id);
            chunk.delta.push((local, id));
        }
        Some(was)
    }

    fn is_loaded(&self, p: Point) -> bool {
        self.locate(p).is_some_and(|(slot, _)| self.chunks[slot].is_some())
    }

    /// The window in world tiles; empty until one loads.
    fn window_tiles(&self) -> Rect {
        match self.region_size {
            Some(s) => Rect::new(self.window.x * s, self.window.y * s, self.window.width * s, self.window.height * s),
            None => Rect::new(0, 0, 0, 0),
        }
    }

    /// Moves the window to cover `regions`, generating what is new and
    /// dropping what left. Edits to dropped chunks are kept as deltas.
    fn load(&mut self, regions: Rect, world: &WorldGraph, rules: &dyn ChunkRules) -> Result<Vec<Point>, rl_mapgen::BuildError> {
        if regions == self.window && self.is_windowed() {
            return Ok(Vec::new());
        }
        // The world graph is the only thing that knows how big a region
        // is; nobody had to say so to build the map.
        self.region_size = Some(world.region_size());
        let mut next: Vec<Option<Chunk>> = Vec::with_capacity(regions.area().max(0) as usize);
        let mut loaded = Vec::new();
        for region in regions.cells() {
            match self.slot(region).and_then(|slot| self.chunks[slot].take()) {
                Some(chunk) => next.push(Some(chunk)),
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
        let window = self.window;
        for (slot, chunk) in self.chunks.drain(..).enumerate() {
            if let Some(chunk) = chunk
                && !chunk.delta.is_empty()
            {
                let region = window.origin() + Point::new(slot as i32 % window.width, slot as i32 / window.width);
                self.deltas.insert(region, chunk.delta);
            }
        }
        self.window = regions;
        self.chunks = next;
        Ok(loaded)
    }

    /// Every edit, from chunks in the window and out of it alike.
    fn edits(&self) -> Vec<(Point, Vec<(usize, TileId)>)> {
        let mut deltas: Vec<(Point, Vec<(usize, TileId)>)> = self.deltas.iter().map(|(r, d)| (*r, d.clone())).collect();
        for (slot, chunk) in self.chunks.iter().enumerate() {
            if let Some(chunk) = chunk
                && !chunk.delta.is_empty()
            {
                let region = self.window.origin() + Point::new(slot as i32 % self.window.width, slot as i32 / self.window.width);
                deltas.push((region, chunk.delta.clone()));
            }
        }
        deltas
    }
}

/// A built place, kept whole.
#[derive(Debug)]
pub struct PlaceMap {
    /// The tiles, edits included.
    pub terrain: rl_grid::Terrain,
    /// Where an arrival from above stands.
    pub entry: Point,
    /// Where an arrival from below stands, if it goes deeper.
    pub exit: Option<Point>,
    /// The builder's points of interest.
    pub spots: Vec<Spot>,
}

/// The map every reader reads: the streamed surface, when the game has
/// one, and every built place, one of which may be the current map.
#[derive(Resource, Debug)]
pub struct WorldMap {
    tables: TileTables,
    /// Bumped every time the window moves or the current map changes, so
    /// viewsheds know to recompute.
    generation: u64,
    /// Bumped every time an edit changes what blocks sight, so viewsheds
    /// and light know to recompute without anyone moving.
    opacity_epoch: u64,
    /// Bumped every time an edit changes where an actor may walk or what a
    /// step costs, so flow fields know a door opened while nobody moved.
    cost_epoch: u64,
    current: MapId,
    surface: Surface,
    places: BTreeMap<MapId, PlaceMap>,
    /// Cells of the current window that hide what is behind them for a
    /// reason other than their tile, such as thick smoke, and the world cell
    /// at its top-left.
    veil: (Point, BitGrid),
}

impl WorldMap {
    /// An empty map: nothing streamed, no place built. A game with a
    /// world graph gets its surface on the first stream, region size and
    /// all; a delve never gets one and never asks for one.
    pub fn new(tables: TileTables) -> Self {
        Self {
            tables,
            generation: 0,
            opacity_epoch: 0,
            cost_epoch: 0,
            current: MapId::SURFACE,
            surface: Surface::default(),
            places: BTreeMap::new(),
            veil: (Point::ZERO, BitGrid::new(0, 0)),
        }
    }

    /// Hides what is behind every cell in `cells` on the current map, on top
    /// of what its tile hides, replacing the veil set before: thick smoke,
    /// rewritten every turn by what makes it.
    ///
    /// Field of view and light both read opacity through the map, so a veil
    /// stops sight and light alike, and changing it moves the opacity epoch
    /// so every viewshed and the light are recast without anyone moving. The
    /// veil is lifted whenever the current map or the window changes, until
    /// whatever set it sets it again.
    pub fn set_veil(&mut self, cells: impl IntoIterator<Item = Point>) {
        let window = self.window_tiles();
        let mut veil = BitGrid::new(window.width, window.height);
        for p in cells {
            veil.insert(p - window.origin());
        }
        let (was_origin, was) = &self.veil;
        let moved = *was_origin != window.origin() && !(was.is_clear() && veil.is_clear());
        if moved || was.count() != veil.count() || was.iter().ne(veil.iter()) {
            self.opacity_epoch += 1;
        }
        self.veil = (window.origin(), veil);
    }

    /// Lifts the veil, when the map or window under it changes.
    fn lift_veil(&mut self) {
        if !self.veil.1.is_clear() {
            self.opacity_epoch += 1;
        }
        self.veil = (Point::ZERO, BitGrid::new(0, 0));
    }

    /// The map every reader reads right now.
    pub fn current(&self) -> MapId {
        self.current
    }

    /// The place being read, if the current map is one.
    fn active_place(&self) -> Option<&PlaceMap> {
        (!self.current.is_surface()).then(|| self.places.get(&self.current)).flatten()
    }

    /// Whether `map` has been built.
    pub fn has_place(&self, map: MapId) -> bool {
        self.places.contains_key(&map)
    }

    /// A built place, current or not.
    pub fn place(&self, map: MapId) -> Option<&PlaceMap> {
        self.places.get(&map)
    }

    /// Keeps a freshly built place. Does not switch to it.
    pub fn install_place(&mut self, map: MapId, build: PlaceBuild) {
        assert!(!map.is_surface(), "the surface is not a place");
        self.places.insert(map, PlaceMap { terrain: build.terrain, entry: build.entry, exit: build.exit, spots: build.spots });
    }

    /// Makes `map` the one readers read: the surface, or a built place.
    /// Returns whether it existed.
    pub fn switch_to(&mut self, map: MapId) -> bool {
        if !map.is_surface() && !self.places.contains_key(&map) {
            return false;
        }
        if self.current != map {
            self.current = map;
            self.generation += 1;
            self.lift_veil();
        }
        true
    }

    /// The loaded surface window, in regions. Empty in a game that
    /// streams nothing.
    pub fn window(&self) -> Rect {
        self.surface.window
    }

    /// The loaded window, in world tiles; a place's whole bounds while
    /// the player is in one.
    pub fn window_tiles(&self) -> Rect {
        match self.active_place() {
            Some(place) => place.terrain.bounds(),
            None => self.surface.window_tiles(),
        }
    }

    /// Changes whenever the window moves or the current map changes.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Changes whenever an edit changes what blocks sight.
    pub fn opacity_epoch(&self) -> u64 {
        self.opacity_epoch
    }

    /// Changes whenever an edit changes where an actor may walk or what a
    /// step costs.
    pub fn cost_epoch(&self) -> u64 {
        self.cost_epoch
    }

    /// The flag tables tiles are read through.
    pub fn tables(&self) -> &TileTables {
        &self.tables
    }

    /// The tile at `p`, or `None` outside the current map.
    pub fn tile(&self, p: Point) -> Option<TileId> {
        match self.active_place() {
            Some(place) => place.terrain.get(p),
            None => self.surface.tile(p),
        }
    }

    /// Sets the tile at `p`, recording a surface edit so it survives the
    /// chunk being unloaded. Returns whether `p` was there to write.
    pub fn set_tile(&mut self, p: Point, id: TileId) -> bool {
        let was = if self.current.is_surface() {
            self.surface.set_tile(p, id)
        } else {
            let Some(place) = self.places.get_mut(&self.current) else { return false };
            let was = place.terrain.get(p);
            if !place.terrain.set(p, id) {
                return false;
            }
            was
        };
        let Some(was) = was else { return false };
        let (was, now) = (was.index(), id.index());
        if self.tables.opaque[was] != self.tables.opaque[now] {
            self.opacity_epoch += 1;
        }
        if self.tables.walkable[was] != self.tables.walkable[now] || self.tables.move_cost[was] != self.tables.move_cost[now] {
            self.cost_epoch += 1;
        }
        true
    }

    /// Whether an actor can stand on `p` now.
    pub fn is_walkable(&self, p: Point) -> bool {
        self.tile(p).is_some_and(|t| self.tables.walkable[t.index()])
    }

    /// What `p` becomes when opened, if it is something that opens.
    pub fn opens(&self, p: Point) -> Option<TileId> {
        self.tile(p).and_then(|t| self.tables.opens[t.index()])
    }

    /// What `p` becomes when closed, if it is something that closes.
    pub fn closes(&self, p: Point) -> Option<TileId> {
        self.tile(p).and_then(|t| self.tables.closes[t.index()])
    }

    /// Whether `p` blocks sight: its tile does, or the veil over it. Unloaded
    /// tiles do.
    pub fn is_opaque(&self, p: Point) -> bool {
        self.tile(p).is_none_or(|t| self.tables.opaque[t.index()]) || self.veil.1.contains(p - self.veil.0)
    }

    /// Whether `p` stops a projectile. Unloaded tiles do.
    pub fn blocks_projectiles(&self, p: Point) -> bool {
        self.tile(p).is_none_or(|t| self.tables.blocks_projectiles[t.index()])
    }

    /// Entry cost of `p`, or `None` if not walkable or not loaded.
    pub fn cost(&self, p: Point) -> Option<u32> {
        let t = self.tile(p)?;
        self.tables.walkable[t.index()].then(|| self.tables.move_cost[t.index()])
    }

    /// Whether `p` is inside the current map.
    pub fn is_loaded(&self, p: Point) -> bool {
        match self.active_place() {
            Some(place) => place.terrain.bounds().contains(p),
            None => self.surface.is_loaded(p),
        }
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
        WindowView { map: self, opening: false }
    }

    /// The same view for a mover that opens what it walks into: a closed
    /// door costs the turn spent opening it and then the step onto what it
    /// opens into, so a flood routes through it when that is shorter.
    pub fn opening_view(&self) -> WindowView<'_> {
        WindowView { map: self, opening: true }
    }

    /// Moves the surface window to cover `regions`, generating what is
    /// new and dropping what left. The region size comes from `world`.
    pub fn load_window(&mut self, regions: Rect, world: &WorldGraph, rules: &dyn ChunkRules) -> Result<Vec<Point>, rl_mapgen::BuildError> {
        let loaded = self.surface.load(regions, world, rules)?;
        self.generation += 1;
        self.lift_veil();
        Ok(loaded)
    }

    /// Every edit to the surface, loaded chunks included, and every built
    /// place, for saving.
    pub fn export(&self) -> WorldMapSave {
        let places =
            self.places.iter().map(|(id, p)| (*id, PlaceSave { terrain: p.terrain.clone(), entry: p.entry, exit: p.exit, spots: p.spots.clone() })).collect();
        WorldMapSave { current: self.current, deltas: self.surface.edits(), places }
    }

    /// Restores edits and places from a save and switches to its current
    /// map. Loaded chunks are dropped so the next stream replays the
    /// edits onto fresh generation.
    pub fn import(&mut self, save: WorldMapSave) {
        self.surface = Surface { deltas: save.deltas.into_iter().collect(), ..Surface::default() };
        self.places = save.places.into_iter().map(|(id, p)| (id, PlaceMap { terrain: p.terrain, entry: p.entry, exit: p.exit, spots: p.spots })).collect();
        self.current = MapId::SURFACE;
        self.switch_to(save.current);
        self.generation += 1;
    }

    /// Number of regions currently loaded.
    pub fn loaded_count(&self) -> usize {
        self.surface.chunks.iter().filter(|c| c.is_some()).count()
    }

    /// Number of unloaded regions with edits kept.
    pub fn stored_deltas(&self) -> usize {
        self.surface.deltas.len()
    }
}

/// A place as saved.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaceSave {
    /// The tiles.
    pub terrain: rl_grid::Terrain,
    /// The entry.
    pub entry: Point,
    /// The exit.
    pub exit: Option<Point>,
    /// The builder's spots.
    pub spots: Vec<Spot>,
}

/// What [`WorldMap::export`] produces.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldMapSave {
    /// The map being read.
    pub current: MapId,
    /// Edits to surface regions.
    pub deltas: Vec<(Point, Vec<(usize, TileId)>)>,
    /// Every built place.
    pub places: Vec<(MapId, PlaceSave)>,
}

/// The loaded window as a grid in window-local coordinates.
#[derive(Debug, Clone, Copy)]
pub struct WindowView<'a> {
    map: &'a WorldMap,
    /// Whether a tile that opens costs its opening rather than blocking.
    opening: bool,
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
        let p = self.world(idx);
        let opened = || {
            let into = self.map.opens(p)?.index();
            self.map.tables.walkable[into].then(|| rl_core::turn::BASE_ACTION_COST + self.map.tables.move_cost[into])
        };
        self.map.cost(p).or_else(|| if self.opening { opened() } else { None })
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
    world: Option<Res<WorldRes>>,
    rules: Option<Res<ChunkRulesRes>>,
    settings: Res<WorldSettings>,
    player: Query<&Position, With<Player>>,
    mut viewsheds: Query<&mut Viewshed>,
    mut loaded: MessageWriter<ChunkLoaded>,
) {
    let Ok(pos) = player.single() else { return };
    // A game with no surface, places only, streams nothing.
    let (Some(world), Some(rules)) = (world, rules) else { return };
    if !map.current().is_surface() {
        return;
    }
    let wanted = desired_window(world.region_of_tile(pos.0), settings.window_radius, &world);
    if wanted == map.window() && map.surface.is_windowed() {
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

/// Chunk streaming: the window of the world kept loaded around the
/// player.
///
/// Needs a [`WorldRes`] and a [`ChunkRulesRes`] before play begins. A
/// game with no surface leaves this one out.
pub struct StreamingPlugin;

impl Plugin for StreamingPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{EngineSet, Needs};
        app.add_message::<ChunkLoaded>()
            .needs::<WorldRes>("StreamingPlugin", "`WorldRes(WorldGraph::generate(seed, config, &rules))`, the surface to stream")
            .needs::<ChunkRulesRes>("StreamingPlugin", "`ChunkRulesRes(Box::new(rules))`, how a chunk of the surface is built")
            .add_systems(Update, stream_chunks.in_set(EngineSet::Stream));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "StreamingPlugin");
    }
}
