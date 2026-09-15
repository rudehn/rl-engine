//! The five floors of the whale, each a chain of engine passes.
//!
//! This is the whole of a floor builder: a tile registry, one chain per
//! floor, and [`PlaceBuild::from_context`] to read the entry, the exit and
//! the prefab marks back out. The engine builds a floor the first time
//! the stairs are taken and keeps it after that.

use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{Grid2D, RunSeed};
use rl_engine::rl_grid::{Light, Rgb};
use rl_engine::rl_grid::{TileId, TileProps, TileRegistry};
use rl_engine::rl_mapgen::dungeon::{Bsp, Doors, FarthestExit, RandomStart, Rooms};
use rl_engine::rl_mapgen::passes::{CellularCave, KeepLargestRegion, Scatter};
use rl_engine::rl_mapgen::prefab::{Placement, Prefab, StampPrefab};
use rl_engine::rl_mapgen::{BaseContext, BuildError, Chain};
use rl_engine::rl_render::{Cell, TileAppearance, Vary};
use rl_engine::rl_world::WorldGraph;

/// How deep the whale goes.
pub const FLOORS: u32 = 5;

/// The floor a map id is; maps count from one.
pub fn floor_of(map: MapId) -> u32 {
    map.0
}

/// The map id of a floor.
pub fn map_of(floor: u32) -> MapId {
    MapId(floor)
}

/// The light everywhere on a floor before anything glows: grey daylight
/// through the propped jaw in the Maw, nothing at all deeper in.
pub fn ambient_of(floor: u32) -> Light {
    match floor {
        1 => Light::new(40, Rgb::new(190, 205, 255)),
        _ => Light::DARK,
    }
}

/// What a floor is called.
pub fn name_of(floor: u32) -> &'static str {
    match floor {
        1 => "the Maw",
        2 => "the Gullet",
        3 => "the Stomach",
        4 => "the Ribcage",
        _ => "the Heart",
    }
}

/// The whale's tiles and how each floor is built.
pub struct Whale {
    tiles: TileRegistry,
    flesh: TileId,
    blubber: TileId,
    tooth: TileId,
    bile: TileId,
    bone: TileId,
    sinew: TileId,
    tallow: TileId,
    cinder: TileId,
    seed: RunSeed,
}

impl Whale {
    pub fn new(seed: RunSeed) -> Self {
        let mut tiles = TileRegistry::new();
        let blubber = tiles.register(TileProps::wall("blubber")).unwrap();
        let flesh = tiles.register(TileProps::floor("flesh")).unwrap();
        let tooth = tiles.register(TileProps::wall("tooth")).unwrap();
        let bile = tiles.register(TileProps::floor("bile").move_cost(200)).unwrap();
        let bone = tiles.register(TileProps::wall("bone")).unwrap();
        // Curtains of sinew burn away and leave the doorway open.
        let sinew = tiles.register(TileProps::floor("sinew").opaque(true).burns(45, 4, "flesh")).unwrap();
        // Slicks of fat: quick to catch, and what is left is cinder.
        let tallow = tiles.register(TileProps::floor("tallow").burns(75, 3, "cinder")).unwrap();
        let cinder = tiles.register(TileProps::floor("cinder")).unwrap();
        Self { tiles, flesh, blubber, tooth, bile, bone, sinew, tallow, cinder, seed }
    }

    pub fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    /// Each tile in full light, both colours, and how it varies from cell
    /// to cell. The brand's light and the dark do the rest.
    pub fn appearance(&self) -> TileAppearance {
        let mut look = TileAppearance::new();
        let flesh = Vary::new(0.28, 0.07);
        let bone = Vary::new(0.14, 0.03);
        look.set_varied(self.blubber, Cell::new('#', Color::srgb(0.88, 0.58, 0.58)).on(Color::srgb(0.52, 0.25, 0.29)), flesh);
        look.set_varied(self.flesh, Cell::new('.', Color::srgb(0.88, 0.6, 0.58)).on(Color::srgb(0.3, 0.13, 0.15)), flesh);
        look.set_varied(self.tooth, Cell::new('^', Color::srgb(1.0, 0.97, 0.88)).on(Color::srgb(0.56, 0.52, 0.45)), bone);
        look.set_varied(self.bile, Cell::new('~', Color::srgb(0.78, 0.98, 0.32)).on(Color::srgb(0.2, 0.3, 0.05)), Vary::new(0.2, 0.05).shimmering(0.25));
        look.set_varied(self.bone, Cell::new('#', Color::srgb(0.96, 0.94, 0.86)).on(Color::srgb(0.62, 0.6, 0.52)), bone);
        look.set_varied(self.sinew, Cell::new('+', Color::srgb(1.0, 0.66, 0.62)).on(Color::srgb(0.46, 0.15, 0.18)), flesh);
        look.set_varied(self.tallow, Cell::new('"', Color::srgb(1.0, 0.94, 0.7)).on(Color::srgb(0.46, 0.38, 0.22)), Vary::new(0.18, 0.05));
        look.set_varied(self.cinder, Cell::new(',', Color::srgb(0.52, 0.46, 0.44)).on(Color::srgb(0.13, 0.1, 0.1)), Vary::new(0.25, 0.04));
        look
    }

    /// The tile that glows: bile gives off its own faint green light.
    pub fn bile(&self) -> TileId {
        self.bile
    }

    /// The heart chamber: the warden at the `W`, sinew curtains for doors.
    fn heart(&self) -> Result<Prefab, BuildError> {
        Prefab::parse(
            &[
                "  ###########  ", //
                " ##.........## ", //
                "##...........##", //
                "#.....###.....#", //
                "#....#...#....#", //
                "+....#.W.#....+", //
                "#....#...#....#", //
                "#.....#+#.....#", //
                "##...........##", //
                " ##.........## ", //
                "  #####+#####  ", //
            ],
            |c| match c {
                '#' => Some(self.bone),
                '.' => Some(self.flesh),
                '+' => Some(self.sinew),
                _ => None,
            },
        )
        .map_err(|e| BuildError::new("heart", e))
    }
}

use bevy::prelude::Color;

/// Bile or fat spilled on the floor after the rooms are carved: a scatter
/// that runs in the finish phase, since growth may not follow structures.
struct Spill {
    name: &'static str,
    tile: TileId,
    on: TileId,
    chance_pct: u32,
}

impl rl_engine::rl_mapgen::Pass<BaseContext> for Spill {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> rl_engine::rl_mapgen::Phase {
        rl_engine::rl_mapgen::Phase::Finish
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        use rand::Rng;
        use rl_engine::rl_mapgen::BuildContext;
        for idx in 0..ctx.terrain().len() {
            if ctx.terrain().get_idx(idx) == self.on && ctx.rng().random_range(0..100) < self.chance_pct {
                ctx.terrain_mut().set_idx(idx, self.tile);
            }
        }
        Ok(())
    }
}

impl PlaceRules for Whale {
    fn build(&self, map: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let floor = floor_of(map);
        // The ribcage is walled in bone; everywhere else is blubber.
        let wall = if floor == 4 { self.bone } else { self.blubber };
        let open = self.flesh;
        let mut ctx = BaseContext::blank(70, 40, self.tiles.clone(), wall);
        let seed = RunSeed(self.seed.0 ^ (floor as u64) << 32);
        let chain = match floor {
            // The Maw: a cavern studded with teeth.
            1 => Chain::new()
                .then(CellularCave { wall, floor: open, ..Default::default() })
                .then(Scatter { name: "teeth", tile: self.tooth, on: open, chance_pct: 3 })
                .then(KeepLargestRegion { wall })
                .then(RandomStart)
                .then(FarthestExit)
                .then(Spill { name: "tallow", tile: self.tallow, on: open, chance_pct: 8 }),
            // The Gullet: a long throat of passages, slick with fat.
            2 => Chain::new().then(Bsp { floor: open, min_leaf: 9, padding: 1 }).then(RandomStart).then(FarthestExit).then(Spill {
                name: "tallow",
                tile: self.tallow,
                on: open,
                chance_pct: 14,
            }),
            // The Stomach: chambers pooled with bile.
            3 => Chain::new()
                .then(Rooms { floor: open, attempts: 40, min_size: 5, max_size: 11, min_rooms: 5 })
                .then(Doors { door: self.sinew })
                .then(RandomStart)
                .then(FarthestExit)
                .then(Spill { name: "pools", tile: self.bile, on: open, chance_pct: 12 }),
            // The Ribcage: tight bone-walled rooms.
            4 => Chain::new()
                .then(Rooms { floor: open, attempts: 60, min_size: 4, max_size: 8, min_rooms: 6 })
                .then(Doors { door: self.sinew })
                .then(RandomStart)
                .then(FarthestExit)
                .then(Spill { name: "tallow", tile: self.tallow, on: open, chance_pct: 6 }),
            // The Heart: one chamber, and what beats in it.
            _ => Chain::new()
                .then(Rooms { floor: open, attempts: 60, min_size: 15, max_size: 18, min_rooms: 2 })
                .then(StampPrefab { name: "heart", prefab: self.heart()?, at: Placement::AnyRoom })
                .then(RandomStart),
        };
        chain.run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_floor_builds_with_a_way_in_and_the_heart_holds_the_warden() {
        for seed in [1u64, 2, 3] {
            let whale = Whale::new(RunSeed(seed));
            let tables = whale.tiles().tables();
            for floor in 1..=FLOORS {
                let built = whale.build(map_of(floor), None).unwrap_or_else(|e| panic!("seed {seed} floor {floor}: {e}"));
                let walkable = |p| built.terrain.get(p).is_some_and(|t: TileId| tables.walkable[t.index()]);
                assert!(walkable(built.entry), "seed {seed} floor {floor}: entry in a wall");
                if floor < FLOORS {
                    assert!(built.exit.is_some_and(walkable), "seed {seed} floor {floor}: no stairs down");
                } else {
                    assert!(built.exit.is_none());
                    assert_eq!(built.spots.iter().filter(|s| s.tag == 'W' as u32).count(), 1, "one warden");
                }
            }
        }
    }
}
