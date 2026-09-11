//! Prints a world, or one chunk of it, as ASCII.
//!
//! ```sh
//! cargo run -p rl-world --example world_dump -- --seed 7 --regions 96x48
//! cargo run -p rl-world --example world_dump -- --seed 7 --region 40,20
//! ```
//!
//! The bands, site kinds and tiles here are example content, not engine
//! vocabulary: this file is what a game's `WorldRules` and `ChunkRules`
//! look like.

use rl_core::{Grid, Grid2D, Point, RunSeed};
use rl_grid::{TileId, TileProps, TileRegistry};
use rl_mapgen::passes::Fill;
use rl_mapgen::{BuildContext, BuildError, Chain, Pass, Phase};
use rl_world::WorldGraph;
use rl_world::chunk::{RiverChannel, RoadPave};
use rl_world::prelude::*;

const SEA: BandId = BandId(0);
const LAKE: BandId = BandId(1);
const BEACH: BandId = BandId(2);
const GRASS: BandId = BandId(3);
const FOREST: BandId = BandId(4);
const DESERT: BandId = BandId(5);
const TUNDRA: BandId = BandId(6);
const HILL: BandId = BandId(7);
const MOUNTAIN: BandId = BandId(8);
const PEAK: BandId = BandId(9);
const TOWN: SiteKindId = SiteKindId(1);
const CAMP: SiteKindId = SiteKindId(2);

struct ExampleWorld;

impl WorldRules for ExampleWorld {
    fn classify(&self, f: &CellFacts) -> BandId {
        if f.is_sea {
            return SEA;
        }
        if f.is_lake {
            return LAKE;
        }
        match f.relief {
            Relief::Peak => PEAK,
            Relief::Mountain => MOUNTAIN,
            Relief::Hill => HILL,
            _ if f.is_coast && f.temperature > 0.3 => BEACH,
            _ if f.temperature < 0.25 => TUNDRA,
            _ if f.moisture < 0.2 && f.temperature > 0.6 => DESERT,
            _ if f.moisture > 0.55 => FOREST,
            _ => GRASS,
        }
    }

    fn road_friction(&self, band: BandId, _: &CellFacts) -> Option<f32> {
        match band {
            GRASS => Some(0.0),
            BEACH | DESERT => Some(0.3),
            FOREST => Some(0.6),
            HILL => Some(0.8),
            TUNDRA => Some(1.0),
            MOUNTAIN => Some(4.0),
            _ => None,
        }
    }

    fn settlements(&self, layers: &Layers, seed: u64) -> Vec<Site> {
        let mut sites = Vec::new();
        let rules = PlacementRules { cells_per_site: 90, min: 2, max: 60, jitter: 0.2, clearances: vec![Clearance { from: TOWN, cells: 7 }] };
        place_scored(&mut sites, TOWN, &rules, seed, layers.width(), layers.height(), |p| {
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
            clearances: vec![Clearance { from: TOWN, cells: 4 }, Clearance { from: CAMP, cells: 6 }],
        };
        place_scored(&mut sites, CAMP, &rules, seed, layers.width(), layers.height(), |p| {
            let f = layers.facts(p).unwrap();
            if f.is_water() || !(1..=3).contains(&distance_to_road[p]) {
                return 0.0;
            }
            1.0
        });
        sites
    }
}

fn band_glyph(b: BandId) -> char {
    match b {
        SEA => '~',
        LAKE => '=',
        BEACH => ':',
        GRASS => '.',
        FOREST => 'T',
        DESERT => ',',
        TUNDRA => '_',
        HILL => 'n',
        MOUNTAIN => 'A',
        PEAK => '^',
        _ => '?',
    }
}

/// The tiles a chunk is painted with.
#[derive(Clone, Copy)]
struct Paint {
    water: TileId,
    sand: TileId,
    grass: TileId,
    rock: TileId,
}

/// Paints tiles from the height samples the engine put in the context, so
/// coastlines and rock run through the chunk at tile resolution.
struct PaintFromHeight(Paint);

impl Pass<ChunkContext> for PaintFromHeight {
    fn name(&self) -> &'static str {
        "paint_from_height"
    }
    fn phase(&self) -> Phase {
        Phase::Ground
    }
    fn apply(&self, ctx: &mut ChunkContext) -> Result<(), BuildError> {
        let size = ctx.region_size();
        let hill = ctx.relief.hill;
        for y in 0..size {
            for x in 0..size {
                let p = Point::new(x, y);
                let h = ctx.heights[p];
                let tile = if h <= ctx.sea_level {
                    self.0.water
                } else if h <= ctx.sea_level + 0.012 {
                    self.0.sand
                } else if ctx.land_height(p) >= hill {
                    self.0.rock
                } else {
                    self.0.grass
                };
                ctx.terrain_mut().set(p, tile);
            }
        }
        Ok(())
    }
}

struct Chunks {
    tiles: TileRegistry,
    paint: Paint,
    road: TileId,
}

impl ChunkRules for Chunks {
    fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }
    fn fill(&self, _: &Surroundings) -> TileId {
        self.paint.grass
    }
    fn chain(&self, _: &WorldGraph, _: &Surroundings) -> Chain<ChunkContext> {
        Chain::new()
            .then(Fill { tile: self.paint.grass })
            .then(PaintFromHeight(self.paint))
            .then(RiverChannel { water: self.paint.water })
            .then(RoadPave { tile: self.road })
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seed = 7u64;
    let mut regions = (96, 48);
    let mut region: Option<Point> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                seed = args[i + 1].parse().expect("seed");
                i += 2;
            }
            "--regions" => {
                let (w, h) = args[i + 1].split_once('x').expect("WxH");
                regions = (w.parse().expect("w"), h.parse().expect("h"));
                i += 2;
            }
            "--region" if args[i + 1] == "auto" => {
                region = Some(Point::new(-1, -1));
                i += 2;
            }
            "--region" => {
                let (x, y) = args[i + 1].split_once(',').expect("X,Y");
                region = Some(Point::new(x.parse().expect("x"), y.parse().expect("y")));
                i += 2;
            }
            other => panic!("unknown argument {other}"),
        }
    }

    let start = std::time::Instant::now();
    let world = WorldGraph::generate(RunSeed(seed), WorldConfig::regions(regions.0, regions.1), &ExampleWorld);
    let took = start.elapsed();
    // `--region auto` picks the first region that has both a road and a river.
    let region = region.map(|r| {
        if r.x >= 0 {
            return r;
        }
        world
            .roads()
            .directions()
            .iter()
            .find(|(p, set)| !set.is_empty() && world.layers().hydrology.is_river(*p))
            .or_else(|| world.roads().directions().iter().find(|(_, set)| !set.is_empty()))
            .map(|(p, _)| p)
            .expect("a region with a road")
    });

    match region {
        None => {
            let l = world.layers();
            for y in 0..world.height() {
                let mut line = String::new();
                for x in 0..world.width() {
                    let p = Point::new(x, y);
                    let c = if let Some(s) = world.site_at(p) {
                        if s.kind == TOWN { 'O' } else { 'c' }
                    } else if !world.roads().at(p).is_empty() {
                        '+'
                    } else if l.hydrology.is_river(p) {
                        'r'
                    } else {
                        band_glyph(l.bands[p])
                    };
                    line.push(c);
                }
                println!("{line}");
            }
            let towns = world.sites().iter().filter(|s| s.kind == TOWN).count();
            let land = (0..l.bands.len()).filter(|i| !l.elevation.is_water_idx(*i)).count();
            println!(
                "seed {seed}: {}x{} regions, {} land, {} lake, {} towns, {} camps, {} road cells, {} river cells, fingerprint {:016x}, {:?}",
                world.width(),
                world.height(),
                land,
                l.hydrology.lakes.count(),
                towns,
                world.sites().len() - towns,
                world.roads().length(),
                l.hydrology.river_length(),
                world.fingerprint(),
                took
            );
        }
        Some(r) => {
            let mut tiles = TileRegistry::standard();
            let water = tiles.register(TileProps::named("water")).unwrap();
            let sand = tiles.register(TileProps::floor("sand")).unwrap();
            let grass = tiles.register(TileProps::floor("grass")).unwrap();
            let rock = tiles.register(TileProps::wall("rock")).unwrap();
            let road = tiles.register(TileProps::floor("road")).unwrap();
            let rules = Chunks { tiles, paint: Paint { water, sand, grass, rock }, road };
            let start = std::time::Instant::now();
            let (terrain, _) = world.build_chunk(r, &rules).expect("chunk");
            let took_chunk = start.elapsed();
            let facts = world.region_facts(r).expect("region");
            println!("region {:?}: band {:?} relief {:?} roads {:?} rivers {:?}", r, facts.band, facts.facts.relief, facts.roads.bits(), facts.rivers.bits());
            for y in 0..terrain.height() {
                let mut line = String::new();
                for x in 0..terrain.width() {
                    let t = terrain.get(Point::new(x, y)).unwrap();
                    line.push(match t {
                        t if t == water => '~',
                        t if t == sand => ':',
                        t if t == grass => '.',
                        t if t == rock => '#',
                        t if t == road => '+',
                        _ => '?',
                    });
                }
                println!("{line}");
            }
            println!("chunk in {took_chunk:?}");
        }
    }
}
