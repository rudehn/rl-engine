//! The coarse layer: one cell per region of the world.

use std::collections::BTreeMap;

use rl_core::{Grid, Grid2D, Point, Rect, RunSeed, SeedDomain, Steps};
use rl_grid::BitGrid;

use crate::climate::{Climate, ClimateConfig};
use crate::elevation::{Elevation, ElevationConfig};
use crate::facts::{BandId, CellFacts, Relief};
use crate::hydrology::{Hydrology, HydrologyConfig};
use crate::roads::{RoadConfig, Roads};
use crate::sites::Site;

/// Everything that shapes a world, seed aside.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldConfig {
    /// Regions across.
    pub regions_wide: i32,
    /// Regions down.
    pub regions_high: i32,
    /// Tiles per region on each axis.
    pub region_size: i32,
    /// Height map settings.
    pub elevation: ElevationConfig,
    /// River and lake settings.
    pub hydrology: HydrologyConfig,
    /// Climate settings.
    pub climate: ClimateConfig,
    /// Road network settings.
    pub roads: RoadConfig,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            regions_wide: 64,
            regions_high: 64,
            region_size: 64,
            elevation: ElevationConfig::default(),
            hydrology: HydrologyConfig::default(),
            climate: ClimateConfig::default(),
            roads: RoadConfig::default(),
        }
    }
}

impl WorldConfig {
    /// The defaults at a given region count.
    pub fn regions(wide: i32, high: i32) -> Self {
        Self { regions_wide: wide, regions_high: high, ..Default::default() }
    }
}

/// The per-region layers of a world, before and after banding.
#[derive(Debug, Clone)]
pub struct Layers {
    /// Heights and the sea line.
    pub elevation: Elevation,
    /// Rivers and lakes.
    pub hydrology: Hydrology,
    /// Temperature and moisture.
    pub climate: Climate,
    /// The game's band per region. Empty until classification runs.
    pub bands: Grid<BandId>,
    coast: BitGrid,
}

impl Layers {
    fn new(elevation: Elevation, hydrology: Hydrology, climate: Climate) -> Self {
        let (w, h) = (elevation.height.width(), elevation.height.height());
        let mut coast = BitGrid::new(w, h);
        for (p, _) in elevation.height.iter() {
            if !elevation.is_water(p) && elevation.height.neighbours(p, Steps::Eight).any(|n| elevation.is_water(n)) {
                coast.insert(p);
            }
        }
        Self { elevation, hydrology, climate, bands: Grid::filled(w, h, BandId::default()), coast }
    }

    /// Regions across.
    pub fn width(&self) -> i32 {
        self.bands.width()
    }

    /// Regions down.
    pub fn height(&self) -> i32 {
        self.bands.height()
    }

    /// Whether `p` is inside the world.
    pub fn contains(&self, p: Point) -> bool {
        self.bands.in_bounds(p)
    }

    /// Sea or lake.
    pub fn is_water(&self, p: Point) -> bool {
        self.elevation.is_water(p) || self.hydrology.is_lake(p)
    }

    /// The height band of `p`.
    pub fn relief(&self, p: Point) -> Relief {
        if self.elevation.is_water(p) {
            return Relief::Water;
        }
        let lh = self.elevation.land_height(p);
        let r = &self.elevation.relief;
        if lh >= r.peak {
            Relief::Peak
        } else if lh >= r.mountain {
            Relief::Mountain
        } else if lh >= r.hill {
            Relief::Hill
        } else {
            Relief::Lowland
        }
    }

    /// Everything the engine knows about `p`, or `None` off the map.
    pub fn facts(&self, p: Point) -> Option<CellFacts> {
        if !self.contains(p) {
            return None;
        }
        Some(CellFacts {
            is_sea: self.elevation.is_water(p),
            is_lake: self.hydrology.is_lake(p),
            river_width: self.hydrology.width[p],
            relief: self.relief(p),
            height: self.elevation.height[p],
            land_height: self.elevation.land_height(p),
            temperature: self.climate.temperature[p],
            moisture: self.climate.moisture[p],
            distance_to_water: self.climate.distance_to_water[p],
            latitude: self.elevation.space().latitude(p.y as f64 + 0.5) as f32,
            is_coast: self.coast.contains(p),
        })
    }

    /// The band of `p`.
    pub fn band(&self, p: Point) -> Option<BandId> {
        self.bands.get(p).copied()
    }
}

/// What a game decides about a world's structure.
pub trait WorldRules {
    /// Names the band for a region.
    fn classify(&self, facts: &CellFacts) -> BandId;

    /// How hard a region is to build a road across, additive on a flat
    /// step; `None` where no road can go.
    fn road_friction(&self, band: BandId, facts: &CellFacts) -> Option<f32>;

    /// Places the sites the road network is built from. `seed` is a stream
    /// seed for [`place_scored`](crate::sites::place_scored).
    fn settlements(&self, layers: &Layers, seed: u64) -> Vec<Site>;

    /// Places everything else, after the roads exist, so a kind that wants
    /// to sit near or far from a road can ask. Default: nothing more.
    fn wilds(&self, layers: &Layers, roads: &Roads, distance_to_road: &Grid<u16>, sites: Vec<Site>, seed: u64) -> Vec<Site> {
        let _ = (layers, roads, distance_to_road, seed);
        sites
    }
}

/// A generated world's coarse structure.
#[derive(Debug, Clone)]
pub struct WorldGraph {
    seed: RunSeed,
    config: WorldConfig,
    layers: Layers,
    sites: Vec<Site>,
    site_index: BTreeMap<Point, usize>,
    roads: Roads,
}

impl WorldGraph {
    /// Generates a world. The same seed, config and rules always produce
    /// the same world.
    pub fn generate(seed: RunSeed, config: WorldConfig, rules: &impl WorldRules) -> Self {
        let (w, h) = (config.regions_wide, config.regions_high);
        let elevation = Elevation::generate(w, h, seed, &config.elevation);
        let hydrology = Hydrology::generate(&elevation, &config.hydrology);
        let climate = {
            let wet = |i: usize| {
                let p = elevation.height.idx_point(i);
                elevation.is_water_idx(i) || hydrology.is_lake(p) || hydrology.is_river(p)
            };
            Climate::generate(&elevation, wet, seed, &config.climate)
        };
        let mut layers = Layers::new(elevation, hydrology, climate);
        layers.bands = Grid::from_fn(w, h, |p| rules.classify(&layers.facts(p).expect("in bounds")));

        let stream = |name: &[u8]| seed.derive(SeedDomain::new(name), 0);
        let towns = rules.settlements(&layers, stream(b"world.settlements"));
        let friction = Grid::from_fn(w, h, |p| {
            let facts = layers.facts(p).expect("in bounds");
            rules.road_friction(layers.bands[p], &facts)
        });
        let roads = Roads::generate(&layers.elevation, &friction, &towns, seed, &config.roads);
        let distance_to_road = roads.distance_field();
        let sites = rules.wilds(&layers, &roads, &distance_to_road, towns, stream(b"world.wilds"));
        let site_index = sites.iter().enumerate().map(|(i, s)| (s.position, i)).collect();

        Self { seed, config, layers, sites, site_index, roads }
    }

    /// The run seed.
    pub fn seed(&self) -> RunSeed {
        self.seed
    }

    /// The config used.
    pub fn config(&self) -> &WorldConfig {
        &self.config
    }

    /// The per-region layers.
    pub fn layers(&self) -> &Layers {
        &self.layers
    }

    /// Regions across.
    pub fn width(&self) -> i32 {
        self.config.regions_wide
    }

    /// Regions down.
    pub fn height(&self) -> i32 {
        self.config.regions_high
    }

    /// Tiles per region on each axis.
    pub fn region_size(&self) -> i32 {
        self.config.region_size
    }

    /// The whole world in tiles.
    pub fn tile_bounds(&self) -> Rect {
        Rect::new(0, 0, self.width() * self.region_size(), self.height() * self.region_size())
    }

    /// The region a world tile lies in.
    pub fn region_of_tile(&self, tile: Point) -> Point {
        Point::new(tile.x.div_euclid(self.region_size()), tile.y.div_euclid(self.region_size()))
    }

    /// The world tile at a region's top-left corner.
    pub fn tile_origin(&self, region: Point) -> Point {
        Point::new(region.x * self.region_size(), region.y * self.region_size())
    }

    /// The tiles a region covers.
    pub fn region_tiles(&self, region: Point) -> Rect {
        let o = self.tile_origin(region);
        Rect::new(o.x, o.y, self.region_size(), self.region_size())
    }

    /// Every landmark.
    pub fn sites(&self) -> &[Site] {
        &self.sites
    }

    /// The landmark in region `p`, if any.
    pub fn site_at(&self, p: Point) -> Option<&Site> {
        self.site_index.get(&p).map(|i| &self.sites[*i])
    }

    /// The index into [`sites`](Self::sites) of the landmark in region `p`.
    pub fn site_index_at(&self, p: Point) -> Option<usize> {
        self.site_index.get(&p).copied()
    }

    /// The roads.
    pub fn roads(&self) -> &Roads {
        &self.roads
    }

    /// A hash of everything a player could see, as a tripwire for
    /// accidental non-determinism. Float layers are quantized so the last
    /// bit of a rounding difference does not trip it.
    pub fn fingerprint(&self) -> u64 {
        let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
        let mut eat = |byte: u8| {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        };
        let l = &self.layers;
        for idx in 0..l.bands.len() {
            eat(l.bands[idx].0 as u8);
            eat((l.elevation.height[idx] * 255.0) as u8);
            eat((l.climate.temperature[idx] * 255.0) as u8);
            eat((l.climate.moisture[idx] * 255.0) as u8);
            eat(l.hydrology.rivers[idx].bits());
            eat(self.roads.directions()[idx].bits());
        }
        for s in &self.sites {
            eat(s.kind.0 as u8);
            for b in s.position.x.to_le_bytes().iter().chain(s.position.y.to_le_bytes().iter()) {
                eat(*b);
            }
        }
        hash
    }
}

#[cfg(test)]
pub(crate) mod demo {
    //! A tiny theme for the tests: enough bands and rules to make a world.
    use super::*;
    use crate::sites::{Clearance, PlacementRules, SiteKindId, place_scored};

    pub const SEA: BandId = BandId(0);
    pub const LAKE: BandId = BandId(1);
    pub const PLAIN: BandId = BandId(2);
    pub const FOREST: BandId = BandId(3);
    pub const HILL: BandId = BandId(4);
    pub const MOUNTAIN: BandId = BandId(5);
    pub const PEAK: BandId = BandId(6);
    pub const TOWN: SiteKindId = SiteKindId(1);
    pub const CAMP: SiteKindId = SiteKindId(2);

    pub struct Demo;

    impl WorldRules for Demo {
        fn classify(&self, f: &CellFacts) -> BandId {
            if f.is_sea {
                SEA
            } else if f.is_lake {
                LAKE
            } else {
                match f.relief {
                    Relief::Peak => PEAK,
                    Relief::Mountain => MOUNTAIN,
                    Relief::Hill => HILL,
                    _ if f.moisture > 0.55 => FOREST,
                    _ => PLAIN,
                }
            }
        }

        fn road_friction(&self, band: BandId, _: &CellFacts) -> Option<f32> {
            match band {
                PLAIN => Some(0.0),
                FOREST => Some(0.6),
                HILL => Some(0.8),
                MOUNTAIN => Some(4.0),
                _ => None,
            }
        }

        fn settlements(&self, layers: &Layers, seed: u64) -> Vec<Site> {
            let mut sites = Vec::new();
            let rules = PlacementRules { cells_per_site: 60, min: 2, max: 40, jitter: 0.2, clearances: vec![Clearance { from: TOWN, cells: 6 }] };
            place_scored(&mut sites, TOWN, &rules, seed, layers.width(), layers.height(), |p| {
                let f = layers.facts(p).unwrap();
                if f.is_water() || f.relief != Relief::Lowland {
                    return 0.0;
                }
                let water = if f.distance_to_water <= 2 { 1.0 } else { 0.5 };
                f.temperature * (0.5 + f.moisture) * water
            });
            sites
        }

        fn wilds(&self, layers: &Layers, _: &Roads, distance_to_road: &Grid<u16>, mut sites: Vec<Site>, seed: u64) -> Vec<Site> {
            let rules = PlacementRules {
                cells_per_site: 150,
                min: 0,
                max: 20,
                jitter: 0.2,
                clearances: vec![Clearance { from: TOWN, cells: 4 }, Clearance { from: CAMP, cells: 5 }],
            };
            place_scored(&mut sites, CAMP, &rules, seed, layers.width(), layers.height(), |p| {
                let f = layers.facts(p).unwrap();
                if f.is_water() || distance_to_road[p] > 3 {
                    return 0.0;
                }
                1.0 + distance_to_road[p] as f32
            });
            sites
        }
    }
}

#[cfg(test)]
mod tests {
    use super::demo::*;
    use super::*;

    fn world(seed: u64) -> WorldGraph {
        WorldGraph::generate(RunSeed(seed), WorldConfig::regions(48, 32), &Demo)
    }

    #[test]
    fn a_world_has_a_useful_spread_of_bands_sites_and_roads() {
        for seed in 1..=3 {
            let w = world(seed);
            let mut counts = BTreeMap::new();
            for b in w.layers().bands.cells() {
                *counts.entry(*b).or_insert(0usize) += 1;
            }
            assert!(counts.len() >= 5, "seed {seed}: bands {counts:?}");
            assert!(w.sites().iter().filter(|s| s.kind == TOWN).count() >= 2);
            assert!(w.roads().length() > 0, "seed {seed}: no roads");
            for s in w.sites() {
                assert_eq!(w.site_at(s.position), Some(s));
                assert!(!w.layers().is_water(s.position));
            }
        }
    }

    #[test]
    fn same_seed_same_world_different_seed_different_world() {
        assert_eq!(world(7).fingerprint(), world(7).fingerprint());
        assert_ne!(world(7).fingerprint(), world(8).fingerprint());
    }

    #[test]
    fn tile_and_region_arithmetic_round_trip() {
        let w = world(1);
        assert_eq!(w.region_size(), 64);
        assert_eq!(w.region_of_tile(Point::new(130, 64)), Point::new(2, 1));
        assert_eq!(w.tile_origin(Point::new(2, 1)), Point::new(128, 64));
        assert!(w.region_tiles(Point::new(2, 1)).contains(Point::new(191, 127)));
        assert!(!w.region_tiles(Point::new(2, 1)).contains(Point::new(192, 127)));
        assert_eq!(w.tile_bounds().width, 48 * 64);
    }

    #[test]
    fn facts_are_consistent_with_layers() {
        let w = world(2);
        let l = w.layers();
        for (p, _) in l.bands.iter() {
            let f = l.facts(p).unwrap();
            assert_eq!(f.is_sea, l.elevation.is_water(p));
            assert_eq!(f.has_river(), l.hydrology.is_river(p));
            if f.is_coast {
                assert!(!f.is_sea);
            }
            assert!((0.0..=1.0).contains(&f.latitude));
        }
        assert!(l.facts(Point::new(-1, 0)).is_none());
    }
}
