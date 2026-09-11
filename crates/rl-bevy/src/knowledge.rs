//! What the player has seen.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use rl_core::{Grid2D, Point};
use rl_grid::BitGrid;

use crate::places::MapId;

/// Explored tiles and discovered sites, kept per region so they survive a
/// chunk being unloaded and never cost more than what was actually seen.
/// Explored tiles are per map: the current map's are read, the others'
/// kept aside.
#[derive(Resource, Debug, Default)]
pub struct Knowledge {
    region_size: i32,
    explored: BTreeMap<Point, BitGrid>,
    sites: BTreeSet<usize>,
    current: MapId,
    stash: BTreeMap<MapId, BTreeMap<Point, BitGrid>>,
}

impl Knowledge {
    /// Knowledge for a world whose regions are `region_size` tiles across.
    pub fn new(region_size: i32) -> Self {
        Self { region_size, explored: BTreeMap::new(), sites: BTreeSet::new(), current: MapId::SURFACE, stash: BTreeMap::new() }
    }

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

    fn split(&self, p: Point) -> (Point, Point) {
        let s = self.region_size;
        let region = Point::new(p.x.div_euclid(s), p.y.div_euclid(s));
        let local = Point::new(p.x.rem_euclid(s), p.y.rem_euclid(s));
        (region, local)
    }

    /// Whether the world tile `p` has been seen.
    pub fn is_explored(&self, p: Point) -> bool {
        let (region, local) = self.split(p);
        self.explored.get(&region).is_some_and(|g| g.contains(local))
    }

    /// Records that `p` has been seen.
    pub fn mark(&mut self, p: Point) {
        let (region, local) = self.split(p);
        let s = self.region_size;
        self.explored.entry(region).or_insert_with(|| BitGrid::new(s, s)).insert(local);
    }

    /// Whether any tile of `region` has been seen.
    pub fn region_touched(&self, region: Point) -> bool {
        self.explored.get(&region).is_some_and(|g| !g.is_clear())
    }

    /// How many tiles of `region` have been seen.
    pub fn region_explored_count(&self, region: Point) -> usize {
        self.explored.get(&region).map(|g| g.count()).unwrap_or(0)
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

    /// Total explored tiles.
    pub fn explored_count(&self) -> usize {
        self.explored.values().map(|g| g.count()).sum()
    }

    /// Whether the explored grid for a region exists at all.
    pub fn has_region(&self, region: Point) -> bool {
        self.explored.get(&region).is_some_and(|g| g.len() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_survive_across_regions_and_negative_coordinates() {
        let mut k = Knowledge::new(16);
        k.mark(Point::new(17, 3));
        k.mark(Point::new(-1, -1));
        assert!(k.is_explored(Point::new(17, 3)));
        assert!(k.is_explored(Point::new(-1, -1)));
        assert!(!k.is_explored(Point::new(18, 3)));
        assert!(k.region_touched(Point::new(1, 0)));
        assert!(k.region_touched(Point::new(-1, -1)));
        assert!(!k.region_touched(Point::new(0, 0)));
        assert_eq!(k.explored_count(), 2);
        assert!(k.discover_site(3));
        assert!(!k.discover_site(3));
        assert!(k.site_discovered(3));
        assert!(k.has_region(Point::new(1, 0)));
    }
}
