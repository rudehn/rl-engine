//! The three floors of the counting house, each a chain of engine passes.
//!
//! Rooms joined by corridors, a door on every threshold, and the way up
//! at the far end. The engine builds a floor the first time the stairs are
//! taken and keeps it; what stands in it, the lamps, the coin and the
//! watch, is `main.rs`'s to place when the floor is entered.

use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{Grid2D, RunSeed};
use rl_engine::rl_grid::{TileId, TileProps, TileRegistry};
use rl_engine::rl_mapgen::dungeon::{Doors, FarthestExit, RandomStart, Rooms};
use rl_engine::rl_mapgen::{BaseContext, BuildContext, BuildError, Chain, Pass, Phase};
use rl_engine::rl_render::TileAppearance;
use rl_engine::rl_world::WorldGraph;

/// How many floors the house has. The run starts at the bottom and climbs.
pub const FLOORS: u32 = 3;

/// How every tile looks, compiled in so the binary runs from anywhere.
const TILES_RON: &str = include_str!("../assets/tiles.ron");

/// The floor a map id is; maps count from one.
pub fn floor_of(map: MapId) -> u32 {
    map.0
}

/// The map id of a floor.
pub fn map_of(floor: u32) -> MapId {
    MapId(floor)
}

/// What a floor is called.
pub fn name_of(floor: u32) -> &'static str {
    match floor {
        1 => "the cellars",
        2 => "the counting floor",
        _ => "the strongroom",
    }
}

/// The house's tiles and how each floor is built.
pub struct House {
    tiles: TileRegistry,
    floor: TileId,
    carpet: TileId,
    door: TileId,
    window: TileId,
    seed: RunSeed,
}

impl House {
    pub fn new(seed: RunSeed) -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("wall")).unwrap();
        let floor = tiles.register(TileProps::floor("floor")).unwrap();
        // Carpet is floor that the strongroom is laid with; it looks
        // different and walks the same.
        let carpet = tiles.register(TileProps::floor("carpet")).unwrap();
        // A shut door stops sight and light. Anyone with hands opens it by
        // walking into it; a hound cannot, which is what a shut door is for.
        let door = tiles.register(TileProps::named("door").passable(true).opaque(true).blocks_projectiles(true).opens_to("open door")).unwrap();
        tiles.register(TileProps::floor("open door").closes_to("door")).unwrap();
        // The way out on the top floor: a window onto the roofs, which
        // stops nothing but a body.
        let window = tiles.register(TileProps::named("window").blocks_projectiles(true)).unwrap();
        Self { tiles, floor, carpet, door, window, seed }
    }

    pub fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    /// The tile the strongroom is laid with, for the coin that lies on it.
    pub fn carpet(&self) -> TileId {
        self.carpet
    }

    /// The window out, set into the wall by the strongroom's far end.
    pub fn window(&self) -> TileId {
        self.window
    }

    /// Each tile in full light, from `assets/tiles.ron`; a tile the file
    /// forgets stops the game at startup, by name.
    pub fn appearance(&self) -> TileAppearance {
        TileAppearance::load(TILES_RON, &self.tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"))
    }
}

impl PlaceRules for House {
    fn build(&self, map: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let floor = floor_of(map);
        let wall = self.tiles.expect("wall");
        let mut ctx = BaseContext::blank(64, 36, self.tiles.clone(), wall);
        // A stream per floor, so the strongroom is the same however long
        // the cellars took.
        let seed = RunSeed(self.seed.0 ^ (floor as u64) << 32);
        let rooms = match floor {
            // The cellars: many small rooms, a door on each.
            1 => Rooms { floor: self.floor, attempts: 60, min_size: 4, max_size: 8, min_rooms: 7 },
            // The counting floor: fewer, larger rooms, long sightlines.
            2 => Rooms { floor: self.floor, attempts: 40, min_size: 6, max_size: 11, min_rooms: 5 },
            // The strongroom: rooms enough to hide the coin among.
            _ => Rooms { floor: self.floor, attempts: 50, min_size: 5, max_size: 9, min_rooms: 6 },
        };
        // The stairs up are the far end of every floor, and on the top
        // floor the far end is the window out.
        let chain = Chain::new().then(rooms).then(Doors { door: self.door }).then(RandomStart).then(FarthestExit);
        // Carpet is laid last, over what the rooms left: a finish, which
        // follows the ways in and out rather than preceding them.
        let chain = if floor == FLOORS { chain.then(Carpeting { carpet: self.carpet, on: self.floor, chance_pct: 18 }) } else { chain };
        chain.run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }
}

/// Carpet laid over some of the floor after the rooms are carved and the
/// doors hung: a finish, since growth may not follow structures.
struct Carpeting {
    carpet: TileId,
    on: TileId,
    chance_pct: u32,
}

impl Pass<BaseContext> for Carpeting {
    fn name(&self) -> &'static str {
        "carpet"
    }
    fn phase(&self) -> Phase {
        Phase::Finish
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        use rand::Rng;
        for idx in 0..ctx.terrain().len() {
            if ctx.terrain().get_idx(idx) == self.on && ctx.rng().random_range(0..100) < self.chance_pct {
                ctx.terrain_mut().set_idx(idx, self.carpet);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every floor of every seed builds with a way in and a way on, well
    /// apart, and the strongroom has carpet for the coin to lie on.
    #[test]
    fn every_floor_of_every_seed_has_a_way_in_and_a_way_on() {
        for seed in [1u64, 7, 42, 99, 1234] {
            let house = House::new(RunSeed(seed));
            let tables = house.tiles().tables();
            for floor in 1..=FLOORS {
                let built = house.build(map_of(floor), None).unwrap_or_else(|e| panic!("seed {seed} floor {floor}: {e}"));
                let walkable = |p| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]);
                let exit = built.exit.expect("a way on");
                assert!(walkable(built.entry) && walkable(exit), "seed {seed} floor {floor}: entry or exit on a wall");
                assert_ne!(built.entry, exit, "seed {seed} floor {floor}: the way on is the way in");
                // Room to move, and so to be somewhere a lamp is not.
                let open = built.terrain.iter().filter(|(_, t)| tables.walkable[t.index()]).count();
                assert!(open >= 100, "seed {seed} floor {floor}: only {open} cells to move in");
                if floor == FLOORS {
                    assert!(built.terrain.iter().any(|(_, t)| t == house.carpet()), "seed {seed}: no carpet in the strongroom");
                }
            }
        }
    }
}
