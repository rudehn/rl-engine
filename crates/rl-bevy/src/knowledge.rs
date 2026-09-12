//! What the player has seen.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use rl_core::Point;
use rl_grid::BitGrid;

use crate::places::MapId;

/// Explored tiles are kept in square buckets this many tiles a side,
/// allocated only where something was seen, so knowledge never costs
/// more than what was actually looked at.
///
/// A storage detail and nothing else: a delve has no regions, and the
/// overworld's fog is kept apart, in the regions the world graph names.
/// It is part of the save shape, so a game bumps its save version if this
/// ever changes.
const BUCKET: i32 = 64;

/// Explored tiles, the surface regions the player has been in sight of,
/// and the sites discovered.
///
/// Explored tiles are per map: the current map's are read, the others'
/// kept aside. The regions and the sites are the surface's alone, so
/// going down a staircase never hides the overworld's fog.
#[derive(Resource, Debug, Default)]
pub struct Knowledge {
    explored: BTreeMap<Point, BitGrid>,
    touched: BTreeSet<Point>,
    sites: BTreeSet<usize>,
    current: MapId,
    stash: BTreeMap<MapId, BTreeMap<Point, BitGrid>>,
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

    fn split(p: Point) -> (Point, Point) {
        let bucket = Point::new(p.x.div_euclid(BUCKET), p.y.div_euclid(BUCKET));
        let local = Point::new(p.x.rem_euclid(BUCKET), p.y.rem_euclid(BUCKET));
        (bucket, local)
    }

    /// Whether the tile `p` of the current map has been seen.
    pub fn is_explored(&self, p: Point) -> bool {
        let (bucket, local) = Self::split(p);
        self.explored.get(&bucket).is_some_and(|g| g.contains(local))
    }

    /// Records that `p` has been seen on the current map.
    pub fn mark(&mut self, p: Point) {
        let (bucket, local) = Self::split(p);
        self.explored.entry(bucket).or_insert_with(|| BitGrid::new(BUCKET, BUCKET)).insert(local);
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
        self.explored.values().map(|g| g.count()).sum()
    }

    /// Everything known, for saving.
    pub fn export(&self) -> KnowledgeSave {
        let mut explored: Vec<(MapId, Vec<(Point, BitGrid)>)> =
            self.stash.iter().map(|(m, e)| (*m, e.iter().map(|(p, g)| (*p, g.clone())).collect())).collect();
        explored.push((self.current, self.explored.iter().map(|(p, g)| (*p, g.clone())).collect()));
        KnowledgeSave { current: self.current, explored, touched: self.touched.iter().copied().collect(), sites: self.sites.iter().copied().collect() }
    }

    /// Replaces everything known with a save.
    pub fn import(&mut self, save: KnowledgeSave) {
        self.touched = save.touched.into_iter().collect();
        self.sites = save.sites.into_iter().collect();
        self.stash = save.explored.into_iter().map(|(m, e)| (m, e.into_iter().collect())).collect();
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
