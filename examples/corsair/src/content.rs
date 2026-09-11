//! Corsair's vocabulary: bands, tiles, sites, and how chunks are painted.
//!
//! Everything in here is game content. The engine knows none of these
//! names; it sees band ids, tile ids and site kind ids.

use bevy::prelude::Color;
use rand::Rng;
use rl_engine::rl_core::{Grid, Point};
use rl_engine::rl_grid::{TileId, TileProps, TileRegistry};
use rl_engine::rl_mapgen::passes::Scatter;
use rl_engine::rl_mapgen::{BuildContext, BuildError, Chain, Pass, Phase};
use rl_engine::rl_overworld::BandAppearance;
use rl_engine::rl_render::{Cell, TileAppearance};
use rl_engine::rl_world::WorldGraph;
use rl_engine::rl_world::chunk::{RiverChannel, RoadPave};
use rl_engine::rl_world::prelude::*;

pub const SEA: BandId = BandId(0);
pub const LAKE: BandId = BandId(1);
pub const BEACH: BandId = BandId(2);
pub const GRASS: BandId = BandId(3);
pub const JUNGLE: BandId = BandId(4);
pub const DUNES: BandId = BandId(5);
pub const MANGROVE: BandId = BandId(6);
pub const HILL: BandId = BandId(7);
pub const MOUNTAIN: BandId = BandId(8);
pub const VOLCANO: BandId = BandId(9);
pub const PORT: SiteKindId = SiteKindId(1);
pub const COVE: SiteKindId = SiteKindId(2);

pub fn band_name(b: BandId) -> &'static str {
    match b {
        SEA => "sea",
        LAKE => "lake",
        BEACH => "beach",
        GRASS => "scrub",
        JUNGLE => "jungle",
        DUNES => "dunes",
        MANGROVE => "mangrove",
        HILL => "hills",
        MOUNTAIN => "crags",
        VOLCANO => "volcano",
        _ => "?",
    }
}

/// The game's tiles and the rules that place them.
pub struct Content {
    tiles: TileRegistry,
    water: TileId,
    sand: TileId,
    grass: TileId,
    tree: TileId,
    rock: TileId,
    snow: TileId,
    road: TileId,
    plaza: TileId,
}

impl Content {
    pub fn new() -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("void")).unwrap();
        let water = tiles.register(TileProps::named("water")).unwrap();
        let sand = tiles.register(TileProps::floor("sand").move_cost(120)).unwrap();
        let grass = tiles.register(TileProps::floor("grass")).unwrap();
        let tree = tiles.register(TileProps::named("palm").opaque(true).blocks_projectiles(true)).unwrap();
        let rock = tiles.register(TileProps::wall("rock")).unwrap();
        let snow = tiles.register(TileProps::floor("marsh").move_cost(160)).unwrap();
        let road = tiles.register(TileProps::floor("road").move_cost(80)).unwrap();
        let plaza = tiles.register(TileProps::floor("dock")).unwrap();
        Self { tiles, water, sand, grass, tree, rock, snow, road, plaza }
    }

    pub fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    pub fn tile_appearance(&self) -> TileAppearance {
        let mut look = TileAppearance::new();
        look.set(self.water, Cell::new('~', Color::srgb(0.25, 0.45, 0.9)).on(Color::srgb(0.05, 0.12, 0.3)));
        look.set(self.sand, Cell::new('.', Color::srgb(0.85, 0.78, 0.5)));
        look.set(self.grass, Cell::new('.', Color::srgb(0.35, 0.65, 0.3)));
        look.set(self.tree, Cell::new('T', Color::srgb(0.2, 0.6, 0.25)));
        look.set(self.rock, Cell::new('#', Color::srgb(0.55, 0.52, 0.5)));
        look.set(self.snow, Cell::new('"', Color::srgb(0.3, 0.5, 0.4)));
        look.set(self.road, Cell::new('+', Color::srgb(0.7, 0.6, 0.45)));
        look.set(self.plaza, Cell::new('=', Color::srgb(0.6, 0.45, 0.3)));
        look
    }

    pub fn band_appearance(&self) -> BandAppearance {
        let mut look = BandAppearance::new();
        look.set(SEA, Cell::new('~', Color::srgb(0.2, 0.35, 0.7)).on(Color::srgb(0.03, 0.08, 0.2)));
        look.set(LAKE, Cell::new('=', Color::srgb(0.3, 0.55, 0.9)));
        look.set(BEACH, Cell::new(':', Color::srgb(0.85, 0.78, 0.5)));
        look.set(GRASS, Cell::new('.', Color::srgb(0.35, 0.65, 0.3)));
        look.set(JUNGLE, Cell::new('T', Color::srgb(0.15, 0.5, 0.2)));
        look.set(DUNES, Cell::new(',', Color::srgb(0.85, 0.7, 0.4)));
        look.set(MANGROVE, Cell::new('"', Color::srgb(0.3, 0.5, 0.4)));
        look.set(HILL, Cell::new('n', Color::srgb(0.55, 0.6, 0.35)));
        look.set(MOUNTAIN, Cell::new('A', Color::srgb(0.6, 0.55, 0.5)));
        look.set(VOLCANO, Cell::new('^', Color::srgb(0.95, 0.95, 1.0)));
        look
    }
}

impl WorldRules for Content {
    fn classify(&self, f: &CellFacts) -> BandId {
        if f.is_sea {
            return SEA;
        }
        if f.is_lake {
            return LAKE;
        }
        match f.relief {
            Relief::Peak => VOLCANO,
            Relief::Mountain => MOUNTAIN,
            Relief::Hill => HILL,
            _ if f.is_coast && f.moisture > 0.7 => MANGROVE,
            _ if f.is_coast => BEACH,
            _ if f.moisture < 0.2 => DUNES,
            _ if f.moisture > 0.5 => JUNGLE,
            _ => GRASS,
        }
    }

    fn road_friction(&self, band: BandId, _: &CellFacts) -> Option<f32> {
        match band {
            GRASS => Some(0.0),
            BEACH | DUNES => Some(0.3),
            JUNGLE => Some(0.6),
            HILL => Some(0.8),
            MANGROVE => Some(2.5),
            MOUNTAIN => Some(4.0),
            _ => None,
        }
    }

    fn settlements(&self, layers: &Layers, seed: u64) -> Vec<Site> {
        let mut sites = Vec::new();
        let rules = PlacementRules { cells_per_site: 90, min: 2, max: 60, jitter: 0.2, clearances: vec![Clearance { from: PORT, cells: 7 }] };
        place_scored(&mut sites, PORT, &rules, seed, layers.width(), layers.height(), |p| {
            let f = layers.facts(p).unwrap();
            if f.is_water() || f.relief != Relief::Lowland {
                return 0.0;
            }
            let water = if f.distance_to_water <= 2 { 1.0 } else { 0.4 };
            f.temperature * (0.5 + f.moisture) * water
        });
        sites
    }

    fn wilds(&self, layers: &Layers, _: &Roads, distance_to_road: &Grid<u16>, mut sites: Vec<Site>, seed: u64) -> Vec<Site> {
        let rules = PlacementRules {
            cells_per_site: 200,
            min: 0,
            max: 30,
            jitter: 0.2,
            clearances: vec![Clearance { from: PORT, cells: 4 }, Clearance { from: COVE, cells: 6 }],
        };
        place_scored(&mut sites, COVE, &rules, seed, layers.width(), layers.height(), |p| {
            let f = layers.facts(p).unwrap();
            if f.is_water() || !(1..=3).contains(&distance_to_road[p]) { 0.0 } else { 1.0 }
        });
        sites
    }
}

/// Paints a chunk from the height samples the engine put in the context,
/// so coastlines and rock run through it; the region's band decides the
/// ground in between.
struct Paint {
    water: TileId,
    sand: TileId,
    ground: TileId,
    rock: TileId,
}

impl Pass<ChunkContext> for Paint {
    fn name(&self) -> &'static str {
        "paint"
    }
    fn phase(&self) -> Phase {
        Phase::Ground
    }
    fn apply(&self, ctx: &mut ChunkContext) -> Result<(), BuildError> {
        let size = ctx.region_size();
        let rock_at = ctx.relief.hill * 1.15;
        for y in 0..size {
            for x in 0..size {
                let p = Point::new(x, y);
                let h = ctx.heights[p];
                let tile = if h <= ctx.sea_level {
                    self.water
                } else if h <= ctx.sea_level + 0.012 {
                    self.sand
                } else if ctx.land_height(p) >= rock_at {
                    self.rock
                } else {
                    self.ground
                };
                ctx.terrain_mut().set(p, tile);
            }
        }
        Ok(())
    }
}

/// A rough square of paving where a town will one day be.
struct Plaza {
    tile: TileId,
    half: i32,
}

impl Pass<ChunkContext> for Plaza {
    fn name(&self) -> &'static str {
        "plaza"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut ChunkContext) -> Result<(), BuildError> {
        let c = ctx.center();
        let jitter = ctx.rng().random_range(0..2);
        for p in rl_engine::rl_core::geometry::square(c, self.half + jitter) {
            ctx.terrain_mut().set(p, self.tile);
        }
        Ok(())
    }
}

impl ChunkRules for Content {
    fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    fn fill(&self, _: &Surroundings) -> TileId {
        self.grass
    }

    fn chain(&self, _: &WorldGraph, around: &Surroundings) -> Chain<ChunkContext> {
        let band = around.here.band;
        let ground = match band {
            DUNES | BEACH => self.sand,
            MANGROVE => self.snow,
            MOUNTAIN | VOLCANO => self.rock,
            _ => self.grass,
        };
        let mut chain = Chain::new().then(Paint { water: self.water, sand: self.sand, ground, rock: self.rock }).then(RiverChannel { water: self.water });
        let tree_pct = match band {
            JUNGLE => 35,
            MANGROVE => 20,
            GRASS | HILL => 5,
            BEACH => 3,
            _ => 0,
        };
        if tree_pct > 0 {
            chain = chain.then(Scatter { name: "trees", tile: self.tree, on: self.grass, chance_pct: tree_pct });
        }
        if band == HILL {
            chain = chain.then(Scatter { name: "boulders", tile: self.rock, on: self.grass, chance_pct: 6 });
        }
        if around.here.site.is_some() {
            chain = chain.then(Plaza { tile: self.plaza, half: 4 });
        }
        chain.then(RoadPave { tile: self.road })
    }
}
