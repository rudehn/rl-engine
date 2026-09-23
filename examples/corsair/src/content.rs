//! Corsair's vocabulary: bands, tiles, sites, and how chunks are painted.
//!
//! Everything in here is game content. The engine knows none of these
//! names; it sees band ids, tile ids and site kind ids.

use bevy::prelude::Color;
use rand::Rng;
use rl_engine::rl_core::{Grid, Grid2D, Point};
use rl_engine::rl_grid::{TileId, TileProps, TileRegistry};
use rl_engine::rl_mapgen::passes::ScatterBy;
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

/// How every tile looks, compiled in so the binary runs from anywhere.
const TILES_RON: &str = include_str!("../assets/tiles.ron");

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
#[derive(Clone)]
pub struct Content {
    tiles: TileRegistry,
    water: TileId,
    sand: TileId,
    grass: TileId,
    tree: TileId,
    rock: TileId,
    marsh: TileId,
    road: TileId,
    plaza: TileId,
    door: TileId,
    timber: TileId,
    plank: TileId,
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
        let marsh = tiles.register(TileProps::floor("marsh").move_cost(160)).unwrap();
        let road = tiles.register(TileProps::floor("road").move_cost(80)).unwrap();
        let plaza = tiles.register(TileProps::floor("dock")).unwrap();
        tiles.register(TileProps::floor("cave")).unwrap();
        // Shut until someone with hands opens it: a crab or a dog is kept out,
        // a cutthroat is not.
        let door = tiles.register(TileProps::named("door").passable(true).opaque(true).blocks_projectiles(true).opens_to("open door")).unwrap();
        let timber = tiles.register(TileProps::wall("timber")).unwrap();
        let plank = tiles.register(TileProps::floor("plank")).unwrap();
        // Registered last, so a saved map's tile ids still mean what they meant.
        tiles.register(TileProps::floor("open door").closes_to("door")).unwrap();
        Self { tiles, water, sand, grass, tree, rock, marsh, road, plaza, door, timber, plank }
    }

    pub fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    /// How each tile looks in full light, from `assets/tiles.ron`; a tile
    /// the file forgets stops the game at startup, by name.
    pub fn tile_appearance(&self) -> TileAppearance {
        TileAppearance::load(TILES_RON, &self.tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"))
    }

    // ANCHOR: bands
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
    // ANCHOR_END: bands
}

/// The band for a set of facts, whether they are a region's or one tile's.
pub fn classify_facts(f: &CellFacts) -> BandId {
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

impl WorldRules for Content {
    fn classify(&self, f: &CellFacts) -> BandId {
        classify_facts(f)
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
            cells_per_site: 40,
            min: 3,
            max: 30,
            jitter: 0.2,
            clearances: vec![Clearance { from: PORT, cells: 4 }, Clearance { from: COVE, cells: 6 }],
        };
        // A cove is a hidden bit of coast: away from the roads, better
        // still under hills.
        place_scored(&mut sites, COVE, &rules, seed, layers.width(), layers.height(), |p| {
            let f = layers.facts(p).unwrap();
            if f.is_water() || !f.is_coast {
                return 0.0;
            }
            let quiet = if distance_to_road[p] >= 2 { 1.0 } else { 0.3 };
            let hilly = if f.relief == Relief::Hill { 1.5 } else { 1.0 };
            quiet * hilly
        });
        sites
    }
}

/// Paints a chunk tile by tile from the facts the engine sampled for it:
/// water and shore from the height, rock from the relief, and the ground
/// between from the same classification the region got, so a forest edge
/// follows the moisture field rather than the region grid.
struct Paint {
    water: TileId,
    sand: TileId,
    grass: TileId,
    marsh: TileId,
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
                    match classify_facts(&ctx.facts[p]) {
                        DUNES | BEACH => self.sand,
                        MANGROVE => self.marsh,
                        MOUNTAIN | VOLCANO => self.rock,
                        _ => self.grass,
                    }
                };
                ctx.terrain_mut().set(p, tile);
            }
        }
        Ok(())
    }
}

/// Huts around the plaza of a port: timber walls, a plank floor, and a
/// door facing the square.
struct Huts {
    wall: TileId,
    floor: TileId,
    door: TileId,
    water: TileId,
    plaza_half: i32,
}

impl Pass<ChunkContext> for Huts {
    fn name(&self) -> &'static str {
        "huts"
    }
    fn phase(&self) -> Phase {
        Phase::Structures
    }
    fn apply(&self, ctx: &mut ChunkContext) -> Result<(), BuildError> {
        use rl_engine::rl_core::Rect;
        let c = ctx.center();
        let keep_out = rl_engine::rl_core::geometry::square(c, self.plaza_half + 1).collect::<Vec<_>>();
        let mut huts: Vec<Rect> = Vec::new();
        for _ in 0..40 {
            if huts.len() >= 6 {
                break;
            }
            let w = ctx.rng().random_range(4..=6);
            let h = ctx.rng().random_range(4..=5);
            let dx = ctx.rng().random_range(-13..=13 - w);
            let dy = ctx.rng().random_range(-11..=11 - h);
            let hut = Rect::new(c.x + dx, c.y + dy, w, h);
            let clear = hut
                .inflate(1)
                .cells()
                .all(|p| ctx.terrain().get(p).is_some_and(|t| t != self.water) && !keep_out.contains(&p) && !ctx.terrain().bounds().is_border(p))
                && huts.iter().all(|other| !other.too_close(&hut, 1));
            if !clear {
                continue;
            }
            for p in hut.cells() {
                let tile = if hut.is_border(p) { self.wall } else { self.floor };
                ctx.terrain_mut().set(p, tile);
            }
            // The door is on the side that faces the plaza.
            let (cx, cy) = (hut.center().x, hut.center().y);
            let door = if (c.x - cx).abs() > (c.y - cy).abs() {
                Point::new(if c.x > cx { hut.right() - 1 } else { hut.x }, cy)
            } else {
                Point::new(cx, if c.y > cy { hut.bottom() - 1 } else { hut.y })
            };
            ctx.terrain_mut().set(door, self.door);
            huts.push(hut);
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
        let mut chain = Chain::new()
            .then(Paint { water: self.water, sand: self.sand, grass: self.grass, marsh: self.marsh, rock: self.rock })
            .then(RiverChannel { water: self.water });
        // Trees follow the moisture, gathered into thickets and clearings
        // by the clump field; the shore and the hills stay sparse.
        chain = chain.then(ScatterBy {
            name: "trees",
            tile: self.tree,
            on: self.grass,
            chance_pct: Box::new(|ctx: &ChunkContext, p: Point| {
                let f = &ctx.facts[p];
                let base = ((f.moisture - 0.25) * 70.0).clamp(0.0, 40.0);
                let shaped = base * (0.3 + ctx.clumps[p] * 1.4) * if f.relief == Relief::Hill { 0.4 } else { 1.0 };
                shaped.round() as u32
            }),
        });
        chain = chain.then(ScatterBy {
            name: "boulders",
            tile: self.rock,
            on: self.grass,
            chance_pct: Box::new(|ctx: &ChunkContext, p: Point| if ctx.facts[p].relief == Relief::Hill { 6 } else { 0 }),
        });
        if around.here.site.is_some() {
            chain = chain.then(Plaza { tile: self.plaza, half: 4 });
        }
        if around.here.site == Some(PORT) {
            chain = chain.then(Huts { wall: self.timber, floor: self.plank, door: self.door, water: self.water, plaza_half: 5 });
        }
        chain.then(RoadPave { tile: self.road })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_world::WorldConfig;

    #[test]
    fn every_world_has_ports_and_coves() {
        for seed in 1..=6 {
            let content = Content::new();
            let mut config = WorldConfig::regions(64, 64);
            config.elevation.land_fraction = 0.22;
            config.elevation.continent_frequency = 3.2;
            let world = WorldGraph::generate(RunSeed(seed), config, &content);
            let ports = world.sites().iter().filter(|s| s.kind == PORT).count();
            let coves = world.sites().iter().filter(|s| s.kind == COVE).count();
            assert!(ports >= 2, "seed {seed}: {ports} ports");
            assert!(coves >= 3, "seed {seed}: {coves} coves");
        }
    }

    #[test]
    fn land_chunks_mix_their_ground_instead_of_painting_one_block() {
        let content = Content::new();
        let mut config = WorldConfig::regions(32, 32);
        config.elevation.land_fraction = 0.22;
        let world = WorldGraph::generate(RunSeed(7), config, &content);
        let (mut land, mut mixed) = (0, 0);
        for (region, band) in world.layers().bands.iter() {
            if matches!(*band, SEA | LAKE | MOUNTAIN | VOLCANO) {
                continue;
            }
            let (terrain, _) = world.build_chunk(region, &content).unwrap();
            let kinds = [content.grass, content.sand, content.marsh].iter().filter(|t| terrain.count(**t) > 40).count();
            land += 1;
            if kinds >= 2 {
                mixed += 1;
            }
        }
        assert!(land > 10, "{land} land regions");
        assert!(mixed * 100 / land >= 30, "{mixed} of {land} land chunks mix their ground");
    }

    #[test]
    fn a_port_chunk_has_huts_with_doors_and_a_walkable_middle() {
        let content = Content::new();
        let mut config = WorldConfig::regions(32, 32);
        config.elevation.land_fraction = 0.22;
        let world = WorldGraph::generate(RunSeed(7), config, &content);
        let port = world.sites().iter().find(|s| s.kind == PORT).expect("a port");
        let (terrain, _) = world.build_chunk(port.position, &content).unwrap();
        let tables = content.tiles().tables();
        assert!(terrain.count(content.door) >= 2, "{} doors", terrain.count(content.door));
        assert!(terrain.count(content.timber) >= 2 * 12, "{} timber", terrain.count(content.timber));
        let c = terrain.bounds().center();
        assert!(tables.walkable[terrain.get(c).unwrap().index()], "the plaza centre is open");
        let open_door = content.tiles.expect("open door");
        let (shut, open) = (content.door.index(), open_door.index());
        assert!(!tables.walkable[shut] && tables.opaque[shut], "a shut door stops a step and blocks sight");
        assert_eq!(tables.opens[shut], Some(open_door), "until it is opened");
        assert!(tables.walkable[open] && !tables.opaque[open] && tables.closes[open] == Some(content.door), "and open, it is walked through and shut again");
    }
}

#[cfg(test)]
mod dump {
    use super::*;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_world::WorldConfig;

    /// Prints a 2x2 neighbourhood of chunks around the first port, for
    /// eyeballing how the ground blends across region edges:
    /// `cargo test -p corsair dump -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_a_neighbourhood() {
        let content = Content::new();
        let mut config = WorldConfig::regions(64, 64);
        config.elevation.land_fraction = 0.22;
        config.elevation.continent_frequency = 3.2;
        let world = WorldGraph::generate(RunSeed(7), config, &content);
        let port = world.sites().iter().find(|s| s.kind == PORT).expect("a port").position;
        let size = world.region_size();
        let glyph = |t: TileId| content.tile_appearance().lit(t).glyph;
        let chunks: Vec<Vec<_>> = (0..2).map(|dy| (0..2).map(|dx| world.build_chunk(port.offset(dx, dy), &content).unwrap().0).collect()).collect();
        for row_of_chunks in &chunks {
            for y in 0..size {
                let row: String = row_of_chunks.iter().flat_map(|chunk| (0..size).map(move |x| glyph(chunk.get(Point::new(x, y)).unwrap()))).collect();
                println!("{row}");
            }
        }
    }
}
