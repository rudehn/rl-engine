//! What fire and gas keep per map.
//!
//! A field covers the map readers read, in window-local cells, and every
//! other map's field is set aside and taken back up on return, the way a
//! place keeps its actors: leave while smoke hangs in a corridor and it hangs
//! there when you come back. On the streamed surface a field covers the
//! loaded window and moves with it, and what leaves the window is dropped,
//! since a fire nobody can see is not worth stepping or remembering.

use std::collections::BTreeMap;

use rl_core::{Point, Rect};
use rl_grid::TileField;
use serde::{Deserialize, Serialize};

use crate::places::MapId;
use crate::world::WorldMap;

/// A field for every map, the current one fitted to the loaded window.
#[derive(Debug, Clone)]
pub struct MapFields<T> {
    /// The map the field covers, `None` until it first follows one.
    map: Option<MapId>,
    window: Rect,
    field: TileField<T>,
    /// Every other map's cells that hold something, in world coordinates.
    aside: BTreeMap<MapId, Vec<(Point, T)>>,
}

/// One map's field as a save holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedField<T> {
    /// Which map.
    pub map: MapId,
    /// Every cell holding something, in world coordinates.
    pub cells: Vec<(Point, T)>,
}

impl<T: Copy + Default + PartialEq> Default for MapFields<T> {
    fn default() -> Self {
        Self { map: None, window: Rect::new(0, 0, 0, 0), field: TileField::new(0, 0), aside: BTreeMap::new() }
    }
}

impl<T: Copy + Default + PartialEq> MapFields<T> {
    /// Follows the map readers read: sets the field aside and takes up the
    /// next map's when the current map changes, and moves it with the window
    /// when the window moves.
    pub fn follow(&mut self, map: &WorldMap) {
        let (here, window) = (map.current(), map.window_tiles());
        if self.map != Some(here) {
            if let Some(left) = self.map {
                let cells = self.cells().collect::<Vec<_>>();
                if cells.is_empty() {
                    self.aside.remove(&left);
                } else {
                    self.aside.insert(left, cells);
                }
            }
            self.field = TileField::new(window.width, window.height);
            self.window = window;
            for (p, v) in self.aside.remove(&here).unwrap_or_default() {
                self.field.set(p - window.origin(), v);
            }
            self.map = Some(here);
        } else if window != self.window {
            self.field.reframe(window.origin() - self.window.origin(), window.width, window.height);
            self.window = window;
        }
    }

    /// The value at world cell `p` on the current map.
    pub fn get(&self, p: Point) -> T {
        self.field.get(p - self.window.origin())
    }

    /// Replaces the value at world cell `p` with `f` of it. Returns whether
    /// `p` is on the current map's field.
    pub fn update(&mut self, p: Point, f: impl FnOnce(T) -> T) -> bool {
        let origin = self.window.origin();
        self.field.update(p - origin, f)
    }

    /// Every cell on the current map holding something, in world coordinates.
    pub fn cells(&self) -> impl Iterator<Item = (Point, T)> + '_ {
        let origin = self.window.origin();
        self.field.set_cells().map(move |(p, v)| (p + origin, v))
    }

    /// Whether the current map's field holds nothing.
    pub fn is_clear(&self) -> bool {
        self.field.is_clear()
    }

    /// The world cell at the field's top-left.
    pub fn origin(&self) -> Point {
        self.window.origin()
    }

    /// The current map's field, in window-local cells, for a rule to step.
    pub fn field_mut(&mut self) -> &mut TileField<T> {
        &mut self.field
    }

    /// Every map's cells that hold something, the current map's included.
    pub fn export(&self) -> Vec<SavedField<T>> {
        let current = self.map.map(|map| SavedField { map, cells: self.cells().collect() });
        let aside = self.aside.iter().map(|(map, cells)| SavedField { map: *map, cells: cells.clone() });
        current.into_iter().chain(aside).filter(|s| !s.cells.is_empty()).collect()
    }

    /// Replaces everything with `saved`. Each map's field is laid out when
    /// the map is next followed, over whatever window it has then.
    pub fn import(&mut self, saved: impl IntoIterator<Item = SavedField<T>>) {
        *self = Self::default();
        for s in saved {
            self.aside.insert(s.map, s.cells);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::places::PlaceBuild;
    use rl_grid::{Terrain, TileRegistry};

    fn two_places() -> WorldMap {
        let tiles = TileRegistry::standard();
        let mut map = WorldMap::new(tiles.tables());
        for id in [1, 2] {
            let terrain = Terrain::filled(10, 8, tiles.expect("floor"));
            map.install_place(MapId(id), PlaceBuild { terrain, entry: Point::new(1, 1), exit: None, spots: Vec::new() });
        }
        map.switch_to(MapId(1));
        map
    }

    #[test]
    fn a_map_keeps_its_field_while_the_player_is_on_another() {
        let mut map = two_places();
        let mut fields: MapFields<u8> = MapFields::default();
        fields.follow(&map);
        fields.update(Point::new(3, 3), |_| 9);

        map.switch_to(MapId(2));
        fields.follow(&map);
        assert_eq!(fields.get(Point::new(3, 3)), 0, "the other map has nothing there");
        fields.update(Point::new(5, 5), |_| 4);

        map.switch_to(MapId(1));
        fields.follow(&map);
        assert_eq!(fields.get(Point::new(3, 3)), 9, "and the first kept its own");
        let mut saved = fields.export();
        saved.sort_by_key(|s| s.map);
        assert_eq!(
            saved,
            vec![SavedField { map: MapId(1), cells: vec![(Point::new(3, 3), 9)] }, SavedField { map: MapId(2), cells: vec![(Point::new(5, 5), 4)] }]
        );

        let mut restored: MapFields<u8> = MapFields::default();
        restored.import(saved);
        map.switch_to(MapId(2));
        restored.follow(&map);
        assert_eq!(restored.get(Point::new(5, 5)), 4, "a save puts each map's field back");
    }
}
