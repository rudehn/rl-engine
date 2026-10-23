//! The map every reader reads: every built place, one of which is current.
//!
//! Everything that reads tiles reads through [`WorldMap`], in the current
//! place's own coordinates (see [`places`](crate::places)), and gets `None`
//! outside it or while no place is current.

use std::collections::BTreeMap;

use atglyph_core::{Grid2D, Point, Rect};
use atglyph_grid::{BitGrid, CostSource, OpacitySource, TileId, TileTables};
use bevy::prelude::*;

use crate::places::{Lock, MapId, PlaceBuild, Spot};
use crate::secrets::Secret;

/// A built place, kept whole.
#[derive(Debug)]
pub struct PlaceMap {
    /// The tiles, edits included.
    pub terrain: atglyph_grid::Terrain,
    /// Where an arrival from above stands.
    pub entry: Point,
    /// Where an arrival from below stands, if it goes deeper.
    pub exit: Option<Point>,
    /// The builder's points of interest.
    pub spots: Vec<Spot>,
    /// Every lock and seal not yet opened or lifted.
    pub locks: Vec<Lock>,
    /// Every cell still hiding what it is, walled over in `terrain`.
    pub secrets: Vec<Secret>,
    /// The light everywhere in it before anything is lit, as its builder
    /// chose.
    pub ambient: atglyph_grid::Light,
    /// Whether nothing in it is harmed, as its builder chose.
    pub peaceful: bool,
    /// Whether the player has arrived here yet. A place is built for
    /// whoever gets there first, and a monster that ran down the stairs
    /// ahead of the player builds the floor it lands on; the game still
    /// fills that floor the first time the player sees it.
    pub visited: bool,
    /// Whether the light its tiles shed by themselves has been put up
    /// since it was built or restored. Never saved: that light is the
    /// tiles', so a restored place puts it up again from them, the way its
    /// building did.
    lit: bool,
}

/// The map every reader reads: every built place, one of which may be the
/// current map.
#[derive(Resource, Debug)]
pub struct WorldMap {
    tables: TileTables,
    /// Bumped every time the current map changes, so viewsheds know to
    /// recompute.
    generation: u64,
    /// Bumped every time an edit changes what blocks sight, so viewsheds
    /// and light know to recompute without anyone moving.
    opacity_epoch: u64,
    /// Bumped every time an edit changes where an actor may walk or what a
    /// step costs, so flow fields know a door opened while nobody moved.
    cost_epoch: u64,
    current: MapId,
    places: BTreeMap<MapId, PlaceMap>,
    /// Cells of the current map that hide what is behind them for a
    /// reason other than their tile, such as thick smoke, and the world cell
    /// at its top-left.
    veil: (Point, BitGrid),
}

impl WorldMap {
    /// An empty map: no place built, and none current.
    pub fn new(tables: TileTables) -> Self {
        Self { tables, generation: 0, opacity_epoch: 0, cost_epoch: 0, current: MapId::ZERO, places: BTreeMap::new(), veil: (Point::ZERO, BitGrid::new(0, 0)) }
    }

    /// Hides what is behind every cell in `cells` on the current map, on top
    /// of what its tile hides, replacing the veil set before: thick smoke,
    /// rewritten every turn by what makes it.
    ///
    /// Field of view and light both read opacity through the map, so a veil
    /// stops sight and light alike, and changing it moves the opacity epoch
    /// so every viewshed and the light are recast without anyone moving. The
    /// veil is lifted whenever the current map changes, until
    /// whatever set it sets it again.
    pub(crate) fn set_veil(&mut self, cells: impl IntoIterator<Item = Point>) {
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

    /// Lifts the veil, when the map under it changes.
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
        self.places.get(&self.current)
    }

    /// Whether nothing on `map` is harmed: a built place its builder made
    /// [`peaceful`](PlaceBuild::peaceful).
    pub fn is_peaceful(&self, map: MapId) -> bool {
        self.places.get(&map).is_some_and(|p| p.peaceful)
    }

    /// Whether `map` has been built.
    pub(crate) fn has_place(&self, map: MapId) -> bool {
        self.places.contains_key(&map)
    }

    /// A built place, current or not.
    pub fn place(&self, map: MapId) -> Option<&PlaceMap> {
        self.places.get(&map)
    }

    /// The lock on `at` in the map being read, if one is still shut there.
    pub fn lock(&self, at: Point) -> Option<Lock> {
        self.places.get(&self.current)?.locks.iter().find(|l| l.at == at).copied()
    }

    /// Takes the lock on `at` in `map` off, and hands it back: how a game
    /// lifts a seal, and how an opener carrying the key spends a lock.
    /// What the cell then is stays its tile's business, so a game lifting
    /// a seal that does not open sets the tile it becomes itself.
    ///
    /// The cost epoch moves when the current map's lock goes, since the
    /// opening view routed nothing through it until now.
    pub fn unlock(&mut self, map: MapId, at: Point) -> Option<Lock> {
        let locks = &mut self.places.get_mut(&map)?.locks;
        let i = locks.iter().position(|l| l.at == at)?;
        let lock = locks.remove(i);
        if map == self.current {
            self.cost_epoch += 1;
        }
        Some(lock)
    }

    /// Keeps a freshly built place. Does not switch to it.
    pub fn install_place(&mut self, map: MapId, build: PlaceBuild) {
        let secrets = build.secrets.into_iter().map(|s| Secret::hidden(s, build.terrain.get(s.at).unwrap_or(s.is))).collect();
        let place = PlaceMap {
            terrain: build.terrain,
            entry: build.entry,
            exit: build.exit,
            spots: build.spots,
            locks: build.locks,
            secrets,
            ambient: build.ambient,
            peaceful: build.peaceful,
            visited: false,
            lit: false,
        };
        self.places.insert(map, place);
    }

    /// Every cell of `map` still hiding what it is, in the order its builder
    /// hid them. None once every one is found.
    pub fn secrets(&self, map: MapId) -> &[Secret] {
        self.places.get(&map).map_or(&[], |p| p.secrets.as_slice())
    }

    /// Finds the secret on `at` in `map` and every other cell of its
    /// group, lays each one's real tile, and hands them back; empty when
    /// nothing is hidden there. How the spotting roll finds one, and how a
    /// game finds one by any other means.
    ///
    /// The whole group, because a mouth wider than a cell is one opening:
    /// half of one found would show a wall with a hole in it. When `map`
    /// is the one being read the opacity epoch moves if what a cell hides
    /// lets sight through, and the cost epoch always, even where no tile
    /// changed how it is walked, since a found door is one a mind may
    /// now path through by opening it, which the opening view did not route
    /// through while it was wall.
    ///
    /// A cell no longer showing the tile it hid behind, because a game
    /// changed it since, keeps the change and is found all the same: the
    /// door a found mouth would have been is not laid over a cell dug
    /// out or built over.
    pub fn find_secret(&mut self, map: MapId, at: Point) -> Vec<Secret> {
        let Some(place) = self.places.get_mut(&map) else { return Vec::new() };
        let Some(group) = place.secrets.iter().find(|s| s.at == at).map(|s| s.group) else { return Vec::new() };
        let (found, kept): (Vec<Secret>, Vec<Secret>) = std::mem::take(&mut place.secrets).into_iter().partition(|s| s.group == group);
        place.secrets = kept;
        let mut sight_changed = false;
        for secret in &found {
            if place.terrain.get(secret.at) != Some(secret.shows) {
                continue;
            }
            place.terrain.set(secret.at, secret.is);
            sight_changed |= self.tables.opaque[secret.shows.index()] != self.tables.opaque[secret.is.index()];
        }
        if map == self.current {
            self.opacity_epoch += u64::from(sight_changed);
            self.cost_epoch += 1;
        }
        found
    }

    /// Every built place whose tiles' own light is not up yet: each one
    /// built or restored since the lighting last looked.
    pub(crate) fn unlit_places(&self) -> impl Iterator<Item = MapId> + '_ {
        self.places.iter().filter(|(_, p)| !p.lit).map(|(id, _)| *id)
    }

    /// Notes that the light of `map`'s tiles is up.
    pub(crate) fn mark_lit(&mut self, map: MapId) {
        if let Some(place) = self.places.get_mut(&map) {
            place.lit = true;
        }
    }

    /// Marks `map` as somewhere the player has arrived, and answers
    /// whether this was the first time.
    pub(crate) fn visit(&mut self, map: MapId) -> bool {
        self.places.get_mut(&map).is_some_and(|place| !std::mem::replace(&mut place.visited, true))
    }

    /// Whether `p` on `map` can be walked on, whether or not `map` is the
    /// one being read: an arrival on a map nobody is looking at still has
    /// to land on floor. False where the tile is not known.
    pub fn is_walkable_on(&self, map: MapId, p: Point) -> bool {
        let tile = self.places.get(&map).and_then(|place| place.terrain.get(p));
        tile.is_some_and(|t| self.tables.walkable[t.index()])
    }

    /// Makes the built place `map` the one readers read. Returns whether it
    /// existed.
    pub fn switch_to(&mut self, map: MapId) -> bool {
        if !self.places.contains_key(&map) {
            return false;
        }
        if self.current != map {
            self.current = map;
            self.generation += 1;
            self.lift_veil();
        }
        true
    }

    /// The current place's whole bounds; empty while no place is current.
    pub fn window_tiles(&self) -> Rect {
        self.active_place().map_or(Rect::new(0, 0, 0, 0), |place| place.terrain.bounds())
    }

    /// Changes whenever the current map changes.
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    /// Changes whenever an edit changes what blocks sight.
    pub(crate) fn opacity_epoch(&self) -> u64 {
        self.opacity_epoch
    }

    /// Changes whenever an edit changes where an actor may walk or what a
    /// step costs.
    pub(crate) fn cost_epoch(&self) -> u64 {
        self.cost_epoch
    }

    /// The flag tables tiles are read through.
    pub fn tables(&self) -> &TileTables {
        &self.tables
    }

    /// The tile at `p`, or `None` outside the current map.
    pub fn tile(&self, p: Point) -> Option<TileId> {
        self.active_place()?.terrain.get(p)
    }

    /// Sets the tile at `p` of the current map. Returns whether `p` was
    /// there to write.
    pub fn set_tile(&mut self, p: Point, id: TileId) -> bool {
        let Some(place) = self.places.get_mut(&self.current) else { return false };
        let Some(was) = place.terrain.get(p) else { return false };
        if !place.terrain.set(p, id) {
            return false;
        }
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

    /// Whether a diagonal step may pass beside `p`: only beside something
    /// an actor could stand on now, so never past a wall's corner nor a
    /// closed door's.
    ///
    /// The one corner rule, read by the move resolver and by every view a
    /// flood or a search runs over, [`Self::opening_view`] included. That
    /// view prices a closed door at its opening, so a door can be entered
    /// on it; asked whether a diagonal slips past the door's corner, it
    /// answers as the move resolver will, or a mind plans a step it is
    /// refused every turn and never moves.
    pub(crate) fn lets_diagonal_past(&self, p: Point) -> bool {
        self.is_walkable(p)
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
    pub(crate) fn is_opaque(&self, p: Point) -> bool {
        self.tile(p).is_none_or(|t| self.tables.opaque[t.index()]) || self.veil.1.contains(p - self.veil.0)
    }

    /// Whether `p` stops a projectile. Unloaded tiles do.
    pub(crate) fn blocks_projectiles(&self, p: Point) -> bool {
        self.tile(p).is_none_or(|t| self.tables.blocks_projectiles[t.index()])
    }

    /// Entry cost of `p`, or `None` if not walkable or not loaded.
    pub(crate) fn cost(&self, p: Point) -> Option<u32> {
        let t = self.tile(p)?;
        self.tables.walkable[t.index()].then(|| self.tables.move_cost[t.index()])
    }

    /// Whether `p` is inside the current map.
    pub fn is_loaded(&self, p: Point) -> bool {
        self.active_place().is_some_and(|place| place.terrain.bounds().contains(p))
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
    /// opens into, so a flood routes through it when that is shorter. A
    /// door under a [`Lock`] is wall here, key or no key, so no mind is
    /// led to one.
    pub fn opening_view(&self) -> WindowView<'_> {
        WindowView { map: self, opening: true }
    }

    /// Every built place, for saving.
    pub fn export(&self) -> WorldMapSave {
        let places = self
            .places
            .iter()
            .map(|(id, p)| {
                (
                    *id,
                    PlaceSave {
                        terrain: p.terrain.clone(),
                        entry: p.entry,
                        exit: p.exit,
                        spots: p.spots.clone(),
                        locks: p.locks.clone(),
                        secrets: p.secrets.clone(),
                        ambient: p.ambient,
                        peaceful: p.peaceful,
                        visited: p.visited,
                    },
                )
            })
            .collect();
        WorldMapSave { current: self.current, places }
    }

    /// Restores places from a save and switches to its current map.
    pub fn import(&mut self, save: WorldMapSave) {
        self.places = save
            .places
            .into_iter()
            .map(|(id, p)| {
                let mut secrets = p.secrets;
                for s in secrets.iter_mut().filter(|s| s.shows == Secret::unread()) {
                    s.shows = p.terrain.get(s.at).unwrap_or(s.is);
                }
                let place = PlaceMap {
                    terrain: p.terrain,
                    entry: p.entry,
                    exit: p.exit,
                    spots: p.spots,
                    locks: p.locks,
                    secrets,
                    ambient: p.ambient,
                    peaceful: p.peaceful,
                    visited: p.visited,
                    lit: false,
                };
                (id, place)
            })
            .collect();
        self.current = MapId::ZERO;
        self.switch_to(save.current);
        self.generation += 1;
    }
}

/// A place as saved.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlaceSave {
    /// The tiles.
    pub terrain: atglyph_grid::Terrain,
    /// The entry.
    pub entry: Point,
    /// The exit.
    pub exit: Option<Point>,
    /// The builder's spots.
    pub spots: Vec<Spot>,
    /// Every lock and seal still shut; none in a save written before
    /// places had locks.
    #[serde(default)]
    pub locks: Vec<Lock>,
    /// Every cell still hiding what it is; none in a save written before
    /// places had secrets, whose walls then stay walls.
    #[serde(default)]
    pub secrets: Vec<Secret>,
    /// The light everywhere in it before anything is lit; dark in a save
    /// written before places carried their own.
    #[serde(default)]
    pub ambient: atglyph_grid::Light,
    /// Whether nothing in it is harmed; not, in a save written before a
    /// place could be peaceful.
    #[serde(default)]
    pub peaceful: bool,
    /// Whether the player had arrived there. A save written before anyone
    /// but the player travelled holds only places the player built by
    /// arriving, so one with no word on it reads as visited.
    #[serde(default = "visited_by_default")]
    pub visited: bool,
}

fn visited_by_default() -> bool {
    true
}

/// What [`WorldMap::export`] produces.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldMapSave {
    /// The map being read.
    pub current: MapId,
    /// Every built place.
    pub places: Vec<(MapId, PlaceSave)>,
}

/// The current place as a grid in its own coordinates.
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
        // A locked door or a seal is a wall to every mind: none is asked
        // whether it carries the key, so none walks up to a door it would
        // then be refused at every turn.
        let opened = || {
            if self.map.lock(p).is_some() {
                return None;
            }
            let into = self.map.opens(p)?.index();
            self.map.tables.walkable[into].then(|| atglyph_core::turn::BASE_ACTION_COST + self.map.tables.move_cost[into])
        };
        self.map.cost(p).or_else(|| if self.opening { opened() } else { None })
    }

    fn lets_diagonal_past(&self, p: Point) -> bool {
        self.checked_idx(p).is_some_and(|idx| self.map.lets_diagonal_past(self.world(idx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_grid::{Terrain, TileRegistry};

    /// A place holding a door locked to tag 5 and a seal, as built.
    fn locked_place() -> (WorldMap, MapId, [Lock; 2]) {
        let tiles = TileRegistry::standard();
        let locks = [Lock { at: Point::new(3, 1), key: Some(5) }, Lock { at: Point::new(6, 1), key: None }];
        let build = PlaceBuild {
            terrain: Terrain::filled(9, 3, tiles.expect("floor")),
            entry: Point::new(1, 1),
            exit: None,
            spots: Vec::new(),
            locks: locks.to_vec(),
            secrets: Vec::new(),
            ambient: atglyph_grid::Light::DARK,
            peaceful: false,
        };
        let mut map = WorldMap::new(tiles.tables());
        map.install_place(MapId(1), build);
        map.switch_to(MapId(1));
        (map, MapId(1), locks)
    }

    #[test]
    fn a_place_s_locks_and_seals_come_back_from_a_save_and_a_save_without_them_loads_with_none() {
        let (map, place, [lock, seal]) = locked_place();
        let text = ron::to_string(&map.export()).unwrap();
        let mut back = WorldMap::new(TileRegistry::standard().tables());
        back.import(ron::from_str(&text).unwrap());
        assert_eq!((back.lock(lock.at), back.lock(seal.at)), (Some(lock), Some(seal)), "both come back where they were");
        assert_eq!(back.unlock(place, seal.at), Some(seal), "and a seal is still the game's to lift");

        // A save written before places had locks has no word on them.
        let start = text.find("locks:[").expect("the save names its locks");
        let end = start + text[start..].find(']').expect("the list ends") + 1;
        let old = format!("{}{}", &text[..start], text[end..].trim_start_matches(','));
        assert!(!old.contains("locks"), "{old}");
        let mut older = WorldMap::new(TileRegistry::standard().tables());
        older.import(ron::from_str(&old).unwrap());
        assert_eq!((older.lock(lock.at), older.lock(seal.at)), (None, None), "it loads, with nothing locked");
    }

    /// A peaceful place stays peaceful through a save, and one that is not
    /// stays at war; a save written before a place could be peaceful loads
    /// every place at war, as every place was.
    #[test]
    fn a_place_s_peace_comes_back_from_a_save_and_a_save_without_it_loads_none() {
        let tiles = TileRegistry::standard();
        let build = |peaceful| PlaceBuild {
            terrain: Terrain::filled(4, 4, tiles.expect("floor")),
            entry: Point::new(1, 1),
            exit: None,
            spots: Vec::new(),
            locks: Vec::new(),
            secrets: Vec::new(),
            ambient: atglyph_grid::Light::DARK,
            peaceful,
        };
        let mut map = WorldMap::new(tiles.tables());
        map.install_place(MapId(1), build(true));
        map.install_place(MapId(2), build(false));
        let text = ron::to_string(&map.export()).unwrap();
        let mut back = WorldMap::new(tiles.tables());
        back.import(ron::from_str(&text).unwrap());
        assert_eq!((back.is_peaceful(MapId(1)), back.is_peaceful(MapId(2)), back.is_peaceful(MapId::ZERO)), (true, false, false));

        let old = text.replace(",peaceful:true", "").replace(",peaceful:false", "");
        assert!(!old.contains("peaceful"), "{old}");
        let mut older = WorldMap::new(tiles.tables());
        older.import(ron::from_str(&old).unwrap());
        assert!(!older.is_peaceful(MapId(1)), "it loads, with no place at peace");
    }
}
