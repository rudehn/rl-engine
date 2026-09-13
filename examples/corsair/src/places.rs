//! Smugglers' caves: the places under the coves.
//!
//! Every cove on the world map hides a cave mouth. The cave is two levels:
//! a winding cavern, then a dug-out vault with a treasure room. The engine
//! builds each level through [`PlaceRules`] the first time the player
//! goes down, and this module puts the stairs, the smugglers and the
//! treasure in on that first arrival.
//!
//! Caves are dark. The surface is lit by the sun, which is nothing but an
//! ambient light the size of the map; underground the ambient is nothing,
//! the player lights a lantern on the way down, and the smugglers carry
//! their own.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::{Grid2D, Point, RunSeed, SeedDomain, geometry};
use rl_engine::rl_grid::{Light, Rgb};
use rl_engine::rl_mapgen::dungeon::{Doors, ExitPoint, FarthestExit, RandomStart, Rooms};
use rl_engine::rl_mapgen::passes::{CellularCave, KeepLargestRegion, StartPoint};
use rl_engine::rl_mapgen::prefab::{Placement, Prefab, StampPrefab, Stamped};
use rl_engine::rl_mapgen::{BaseContext, BuildContext, BuildError, Chain};
use rl_engine::rl_render::Glyph;
use rl_engine::rl_ui::{MessageLog, Tones};
use rl_engine::rl_world::WorldGraph;

use crate::content::{COVE, Content};
use crate::items::Armory;
use crate::monsters::Bestiary;

/// How deep a cave goes.
pub const LEVELS: u32 = 2;

/// Spot tags the builder reports.
const TREASURE: u32 = 1;

/// The sun: warm white at the level a tile shows its authored colours.
pub fn daylight() -> Light {
    Light::new(170, Rgb::new(255, 250, 236))
}

/// What the player's lantern sheds underground.
const LANTERN: LightSource = LightSource::new(200, 8, Rgb::new(255, 205, 140)).flickering(30);

/// Keeps the light right for the map the player is on: the sun and no
/// lantern on the surface, darkness and a lit lantern below. Runs between
/// the turns and the light, so the frame a warp lands on is drawn in the
/// right light.
pub fn light_the_way(mut commands: Commands, map: Res<WorldMap>, mut lighting: ResMut<Lighting>, player: Query<(Entity, Has<LightSource>), With<Player>>) {
    let Ok((player, lit)) = player.single() else { return };
    let underground = !map.current().is_surface();
    let ambient = if underground { Light::DARK } else { daylight() };
    if lighting.ambient != ambient {
        lighting.ambient = ambient;
    }
    match (underground, lit) {
        (true, false) => {
            commands.entity(player).insert(LANTERN);
        }
        (false, true) => {
            commands.entity(player).remove::<LightSource>();
        }
        _ => {}
    }
}

/// The map id of a cave level under site `site`.
pub fn cave_id(site: usize, depth: u32) -> MapId {
    MapId(1 + site as u32 * LEVELS + depth)
}

/// The site and depth a cave map id names.
pub fn cave_of(map: MapId) -> Option<(usize, u32)> {
    let raw = map.0.checked_sub(1)?;
    Some(((raw / LEVELS) as usize, raw % LEVELS))
}

/// A cave level's name for the status line.
pub fn place_name(map: MapId) -> Option<String> {
    cave_of(map).map(|(_, depth)| if depth + 1 == LEVELS { "smugglers' vault".to_string() } else { format!("smugglers' cave, level {}", depth + 1) })
}

/// Builds cave levels.
pub struct Caves {
    content: Content,
}

impl Caves {
    pub fn new(content: Content) -> Self {
        Self { content }
    }
}

impl PlaceRules for Caves {
    fn build(&self, map: MapId, world: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let world = world.ok_or_else(|| BuildError::new("caves", "a cave lies under a world"))?;
        let (_, depth) = cave_of(map).ok_or_else(|| BuildError::new("caves", format!("{map:?} is not a cave")))?;
        let tiles = self.content.tiles().clone();
        let (rock, cave, door) = (tiles.expect("rock"), tiles.expect("cave"), tiles.expect("door"));
        let seed = RunSeed(world.seed().0 ^ (0xC0DE_u64 << 32) ^ (map.0 as u64));
        let mut ctx = BaseContext::blank(56, 36, tiles, rock);
        let last = depth + 1 == LEVELS;
        let chain = if last {
            let vault = Prefab::parse(
                &[
                    "#######", //
                    "#$...$#", //
                    "#.....#", //
                    "#$...$#", //
                    "###+###", //
                ],
                |c| match c {
                    '#' => Some(rock),
                    '.' => Some(cave),
                    '+' => Some(door),
                    _ => None,
                },
            )
            .map_err(|e| BuildError::new("caves", e))?;
            Chain::new()
                .then(Rooms { floor: cave, attempts: 40, min_size: 4, max_size: 9, min_rooms: 4 })
                .then(Doors { door })
                .then(StampPrefab { name: "vault", prefab: vault, at: Placement::AnyRoom })
                .then(RandomStart)
        } else {
            Chain::new()
                .then(CellularCave { wall: rock, floor: cave, ..Default::default() })
                .then(KeepLargestRegion { wall: rock })
                .then(RandomStart)
                .then(FarthestExit)
        };
        chain.run(&mut ctx, seed)?;
        let entry = ctx.outputs().first::<StartPoint>().ok_or_else(|| BuildError::new("caves", "no start"))?.0;
        let exit = ctx.outputs().first::<ExitPoint>().map(|e| e.0);
        let spots =
            ctx.outputs().iter::<Stamped>().flat_map(|s| s.marks.iter().filter(|(c, _)| *c == '$').map(|(_, p)| Spot { tag: TREASURE, at: *p })).collect();
        let (terrain, _) = ctx.finish();
        Ok(PlaceBuild { terrain, entry, exit, spots })
    }
}

/// Regions whose cave mouth has been placed.
#[derive(Resource, Default)]
pub struct Entrances(pub std::collections::BTreeSet<Point>);

/// Puts a cave mouth in the middle of each cove the first time it streams in.
pub fn mark_entrances(mut commands: Commands, mut loaded: MessageReader<ChunkLoaded>, mut done: ResMut<Entrances>, world: Res<WorldRes>) {
    for ev in loaded.read() {
        let Some(site) = world.site_index_at(ev.region) else { continue };
        if world.sites()[site].kind != COVE || !done.0.insert(ev.region) {
            continue;
        }
        let at = world.region_tiles(ev.region).center();
        commands.spawn((
            Position(at),
            Transition { to: Destination::Place { map: cave_id(site, 0), arrive: Arrive::Entry } },
            Glyph::new('>', Color::srgb(0.9, 0.9, 0.6)).on_layer(1),
        ));
    }
}

/// What populating a level needs.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Stock<'w> {
    bestiary: Res<'w, Bestiary>,
    armory: Res<'w, Armory>,
    map: Res<'w, WorldMap>,
    world: Res<'w, WorldRes>,
    occupancy: Res<'w, Occupancy>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
}

/// On the first arrival in a level: stairs, smugglers and treasure. On
/// every arrival: a line in the log.
pub fn populate_places(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, mut stock: Stock) {
    for ev in entered.read() {
        let Some((site, depth)) = cave_of(ev.map) else { continue };
        let name = place_name(ev.map).unwrap_or_default();
        let turn = stock.turns.turn_number();
        stock.log.push(
            if ev.first { format!("You climb down into the {name}. It smells of tar and rum.") } else { format!("You return to the {name}.") },
            Tones::NOTICE,
            turn,
        );
        if !ev.first {
            continue;
        }
        // The way back: up to the surface from the top level, up a level otherwise.
        let up = if depth == 0 {
            Destination::Surface(stock.world.region_tiles(stock.world.sites()[site].position).center())
        } else {
            Destination::Place { map: cave_id(site, depth - 1), arrive: Arrive::Exit }
        };
        commands.spawn((Position(ev.entry), Transition { to: up }, Glyph::new('<', Color::srgb(0.9, 0.9, 0.6)).on_layer(1)));
        if let Some(exit) = ev.exit {
            commands.spawn((
                Position(exit),
                Transition { to: Destination::Place { map: cave_id(site, depth + 1), arrive: Arrive::Entry } },
                Glyph::new('>', Color::srgb(0.9, 0.9, 0.6)).on_layer(1),
            ));
        }
        let Some(place) = stock.map.place(ev.map) else { continue };
        let mut rng = stock.world.seed().rng(SeedDomain::new(b"corsair.caves"), ev.map.0 as u64);
        // Treasure where the vault marks it: one piece of the smugglers'
        // own gear, then coin and drink.
        for (i, spot) in place.spots.iter().filter(|s| s.tag == TREASURE).enumerate() {
            let (id, n) = if i == 0 {
                let gear = ["cutlass", "boarding axe", "pistol", "buckler", "tricorne"];
                (stock.armory.defs.expect(gear[rng.random_range(0..gear.len())]), 1)
            } else if rng.random_bool(0.7) {
                (stock.armory.defs.expect("doubloons"), rng.random_range(20..=60))
            } else {
                (stock.armory.defs.expect("rum"), rng.random_range(1..=3))
            };
            let enchant = stock.armory.roll_quality(id, crate::items::Quality::HOARD, &mut rng);
            stock.armory.spawn_with(&mut commands, id, n, Some(spot.at), enchant);
        }
        // Smugglers, deeper as you go, never next to the stairs.
        let band = 6 + 6 * depth as i32;
        let bounds = place.terrain.bounds();
        for _ in 0..3 + depth {
            let Some((id, count)) = stock.bestiary.table.pick_group(band, &mut rng) else { continue };
            let id = *id;
            let anchor = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            let mut placed = 0;
            for p in geometry::square(anchor, 3) {
                if placed >= count {
                    break;
                }
                if !stock.map.is_walkable(p) || stock.occupancy.is_occupied(p) || geometry::chebyshev(p, ev.entry) < 8 {
                    continue;
                }
                stock.bestiary.spawn_underground(&mut commands, id, p);
                placed += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_world::WorldConfig;

    #[test]
    fn every_level_has_a_way_in_and_the_vault_has_treasure() {
        for seed in [3u64, 11, 42] {
            let content = Content::new();
            let mut config = WorldConfig::regions(24, 24);
            config.elevation.land_fraction = 0.22;
            let world = WorldGraph::generate(RunSeed(seed), config, &content);
            let tables = content.tiles().tables();
            let caves = Caves::new(content);
            for depth in 0..LEVELS {
                let map = cave_id(0, depth);
                let built = caves.build(map, Some(&world)).unwrap_or_else(|e| panic!("seed {seed} depth {depth}: {e}"));
                let walkable = |p: Point| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]);
                assert!(walkable(built.entry), "seed {seed} depth {depth}: entry on rock");
                if depth + 1 == LEVELS {
                    assert!(built.exit.is_none(), "the vault is the bottom");
                    assert_eq!(built.spots.len(), 4, "four treasure marks");
                    assert!(built.spots.iter().all(|s| walkable(s.at)));
                } else {
                    let exit = built.exit.expect("a cavern goes deeper");
                    assert!(walkable(exit));
                    assert!(geometry::chebyshev(built.entry, exit) > 10, "seed {seed}: stairs too close");
                }
                assert_eq!(cave_of(map), Some((0, depth)));
            }
        }
    }
}
