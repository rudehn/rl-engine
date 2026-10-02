//! What the player has seen.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use rl_core::Point;
use rl_grid::BitGrid;

use crate::places::MapId;

/// Tiles are kept in square buckets this many tiles a side, allocated only
/// where something was seen, so knowledge never costs more than what was
/// actually looked at.
///
/// A storage detail and nothing else: a delve has no regions, and the
/// overworld's fog is kept apart, in the regions the world graph names.
/// It is part of the save shape, so a game bumps its save version if this
/// ever changes.
const BUCKET: i32 = 64;

/// A set of tiles of one map, of any extent: what has been seen of it.
///
/// What [`Knowledge`] keeps for the player and a
/// [`Charts`](crate::charts::Charts) chart keeps for whoever fills it.
/// Ordered, so two runs of a seed walk it in one order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TileSet(BTreeMap<Point, BitGrid>);

impl TileSet {
    fn split(p: Point) -> (Point, Point) {
        let bucket = Point::new(p.x.div_euclid(BUCKET), p.y.div_euclid(BUCKET));
        let local = Point::new(p.x.rem_euclid(BUCKET), p.y.rem_euclid(BUCKET));
        (bucket, local)
    }

    /// Whether `p` is in the set.
    pub fn contains(&self, p: Point) -> bool {
        let (bucket, local) = Self::split(p);
        self.0.get(&bucket).is_some_and(|g| g.contains(local))
    }

    /// Adds `p`.
    pub fn insert(&mut self, p: Point) {
        let (bucket, local) = Self::split(p);
        self.0.entry(bucket).or_insert_with(|| BitGrid::new(BUCKET, BUCKET)).insert(local);
    }

    /// How many tiles are in the set.
    pub fn count(&self) -> usize {
        self.0.values().map(|g| g.count()).sum()
    }

    /// Whether the set holds nothing.
    pub fn is_empty(&self) -> bool {
        self.0.values().all(|g| g.is_clear())
    }

    /// Every tile in the set, bucket by bucket, each row-major.
    pub fn iter(&self) -> impl Iterator<Item = Point> + '_ {
        self.0.iter().flat_map(|(bucket, grid)| grid.iter().map(move |local| Point::new(bucket.x * BUCKET + local.x, bucket.y * BUCKET + local.y)))
    }

    /// Adds every tile of `other`.
    pub fn union_with(&mut self, other: &TileSet) {
        for (bucket, grid) in &other.0 {
            match self.0.get_mut(bucket) {
                Some(mine) => mine.union_with(grid),
                None => {
                    self.0.insert(*bucket, grid.clone());
                }
            }
        }
    }

    /// The set as a save holds it: its buckets and the bits of each.
    pub fn export(&self) -> Vec<(Point, BitGrid)> {
        self.0.iter().map(|(p, g)| (*p, g.clone())).collect()
    }

    /// A set from what [`export`](Self::export) produced.
    pub fn import(buckets: Vec<(Point, BitGrid)>) -> Self {
        Self(buckets.into_iter().collect())
    }
}

/// Explored tiles, the surface regions the player has been in sight of,
/// and the sites discovered.
///
/// Explored tiles are per map: the current map's are read, the others'
/// kept aside. The regions and the sites are the surface's alone, so
/// going down a staircase never hides the overworld's fog.
#[derive(Resource, Debug, Default)]
pub struct Knowledge {
    explored: TileSet,
    touched: BTreeSet<Point>,
    sites: BTreeSet<usize>,
    current: MapId,
    stash: BTreeMap<MapId, TileSet>,
}

impl Knowledge {
    /// The map whose explored tiles are being read.
    pub fn current(&self) -> MapId {
        self.current
    }

    /// Swaps in the explored tiles of `map`, keeping the current ones aside.
    pub fn switch(&mut self, map: MapId) {
        if map == self.current {
            return;
        }
        let incoming = self.stash.remove(&map).unwrap_or_default();
        let outgoing = std::mem::replace(&mut self.explored, incoming);
        self.stash.insert(self.current, outgoing);
        self.current = map;
    }

    /// Whether the tile `p` of the current map has been seen.
    pub fn is_explored(&self, p: Point) -> bool {
        self.explored.contains(p)
    }

    /// Records that `p` has been seen on the current map.
    pub fn mark(&mut self, p: Point) {
        self.explored.insert(p);
    }

    /// Records that a surface region has been in sight. The caller names
    /// the region, because the world graph is what knows how big one is.
    pub fn touch_region(&mut self, region: Point) -> bool {
        self.touched.insert(region)
    }

    /// Whether any tile of a surface region has been seen, wherever the
    /// player happens to be standing now.
    pub fn region_touched(&self, region: Point) -> bool {
        self.touched.contains(&region)
    }

    /// Records that the site at index `site` has been discovered.
    pub fn discover_site(&mut self, site: usize) -> bool {
        self.sites.insert(site)
    }

    /// Whether the site at index `site` has been discovered.
    pub fn site_discovered(&self, site: usize) -> bool {
        self.sites.contains(&site)
    }

    /// Every discovered site index, ascending.
    pub fn discovered_sites(&self) -> impl Iterator<Item = usize> + '_ {
        self.sites.iter().copied()
    }

    /// Explored tiles of the current map.
    pub fn explored_count(&self) -> usize {
        self.explored.count()
    }

    /// Everything known, for saving.
    pub fn export(&self) -> KnowledgeSave {
        let mut explored: Vec<(MapId, Vec<(Point, BitGrid)>)> = self.stash.iter().map(|(m, e)| (*m, e.export())).collect();
        explored.push((self.current, self.explored.export()));
        KnowledgeSave { current: self.current, explored, touched: self.touched.iter().copied().collect(), sites: self.sites.iter().copied().collect() }
    }

    /// Replaces everything known with a save.
    pub fn import(&mut self, save: KnowledgeSave) {
        self.touched = save.touched.into_iter().collect();
        self.sites = save.sites.into_iter().collect();
        self.stash = save.explored.into_iter().map(|(m, e)| (m, TileSet::import(e))).collect();
        self.explored = self.stash.remove(&save.current).unwrap_or_default();
        self.current = save.current;
    }
}

/// What [`Knowledge::export`] produces.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeSave {
    /// The map being read.
    pub current: MapId,
    /// Explored tiles per map, in buckets.
    pub explored: Vec<(MapId, Vec<(Point, BitGrid)>)>,
    /// Surface regions that have been in sight.
    pub touched: Vec<Point>,
    /// Discovered site indexes.
    pub sites: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_survive_across_buckets_and_negative_coordinates() {
        let mut k = Knowledge::default();
        k.mark(Point::new(BUCKET + 1, 3));
        k.mark(Point::new(-1, -1));
        assert!(k.is_explored(Point::new(BUCKET + 1, 3)));
        assert!(k.is_explored(Point::new(-1, -1)));
        assert!(!k.is_explored(Point::new(BUCKET + 2, 3)));
        assert_eq!(k.explored_count(), 2);
        assert!(k.discover_site(3));
        assert!(!k.discover_site(3));
        assert!(k.site_discovered(3));
    }

    #[test]
    fn a_tile_set_unions_iterates_and_round_trips_across_buckets() {
        let mut a = TileSet::default();
        assert!(a.is_empty());
        a.insert(Point::new(3, 3));
        a.insert(Point::new(-1, BUCKET + 2));
        let mut b = TileSet::default();
        b.insert(Point::new(3, 4));
        b.insert(Point::new(-1, BUCKET + 2));
        b.insert(Point::new(BUCKET * 2, 0));
        a.union_with(&b);
        assert_eq!(a.count(), 4, "the one both held is counted once");
        let mut all: Vec<Point> = a.iter().collect();
        all.sort();
        let mut expected = vec![Point::new(-1, BUCKET + 2), Point::new(3, 3), Point::new(3, 4), Point::new(BUCKET * 2, 0)];
        expected.sort();
        assert_eq!(all, expected);
        assert_eq!(TileSet::import(a.export()), a);
        assert!(!a.contains(Point::new(4, 4)));
    }

    #[test]
    fn the_surface_fog_is_not_the_map_being_read() {
        let mut k = Knowledge::default();
        k.touch_region(Point::new(4, 2));
        k.mark(Point::new(9, 9));
        k.switch(MapId(3));
        assert!(k.region_touched(Point::new(4, 2)), "a place does not hide what the surface saw");
        assert!(!k.region_touched(Point::new(0, 0)));
        assert!(!k.is_explored(Point::new(9, 9)), "but its tiles are its own");
    }

    #[test]
    fn a_save_keeps_every_map_apart_and_remembers_the_surface() {
        let mut k = Knowledge::default();
        k.mark(Point::new(3, 3));
        k.touch_region(Point::new(1, 0));
        k.discover_site(2);
        k.switch(MapId(4));
        k.mark(Point::new(5, 5));

        let mut back = Knowledge::default();
        back.import(k.export());
        assert_eq!(back.current(), MapId(4));
        assert!(back.is_explored(Point::new(5, 5)));
        assert!(!back.is_explored(Point::new(3, 3)));
        back.switch(MapId::SURFACE);
        assert!(back.is_explored(Point::new(3, 3)));
        assert!(back.region_touched(Point::new(1, 0)));
        assert!(back.site_discovered(2));
    }
}
