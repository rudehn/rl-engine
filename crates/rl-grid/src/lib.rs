//! Tier 1 of rl-engine: everything that runs over a grid of tiles.
//!
//! - [`TileId`], [`TileProps`], [`TileRegistry`]: what a tile *is* lives in a
//!   registry the game populates. Engine code asks the registry and never
//!   matches on a tile enum, which is what lets a game add force fields or
//!   ice without forking anything.
//! - [`Terrain`]: a grid of tile ids for one map or chunk.
//! - [`OpacitySource`] and [`CostSource`]: the two questions every grid
//!   algorithm asks. Implement them on a newtype that wraps a terrain view to
//!   overlay smoke, hazards or knowledge without touching the map.
//! - [`BitGrid`]: one bit per cell, for viewsheds and visited sets.
//! - [`TileField`]: a value per cell stepped a turn at a time by a rule that
//!   reads the field as it stood, for fire, gas and whatever else spreads.
//! - [`fov`]: symmetric shadowcasting into a [`BitGrid`].
//! - [`light`]: point sources cast through the same shadows into a
//!   [`LightField`] of intensity and colour.
//! - [`AStar`]: point-to-point search with reusable scratch buffers.
//! - [`DijkstraMap`]: one flood, any number of consumers.
//! - [`SpatialGrid`]: who is standing where.
//! - [`region`]: flood fill and connected-component labelling.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod astar;
pub mod bitgrid;
pub mod dijkstra;
pub mod field;
pub mod fov;
pub mod light;
pub mod region;
pub mod spatial;
pub mod targeting;
pub mod terrain;
pub mod tile;

pub use astar::{AStar, PathRules};
pub use bitgrid::BitGrid;
pub use dijkstra::DijkstraMap;
pub use field::{Around, TileField};
pub use light::{Emitter, Light, LightField, Rgb};
pub use spatial::SpatialGrid;
pub use targeting::{Footprint, TargetMode, burst, clear_shot, footprint};
pub use terrain::{CostSource, OpacitySource, Terrain, TerrainView};
pub use tile::{Burn, Kindling, TileId, TileProps, TileRegistry, TileTables};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::astar::{AStar, PathRules};
    pub use crate::bitgrid::BitGrid;
    pub use crate::dijkstra::DijkstraMap;
    pub use crate::field::{Around, TileField};
    pub use crate::fov;
    pub use crate::light::{Emitter, Light, LightField, Rgb};
    pub use crate::region;
    pub use crate::spatial::SpatialGrid;
    pub use crate::targeting::{Footprint, TargetMode, burst, clear_shot, footprint};
    pub use crate::terrain::{CostSource, OpacitySource, Terrain, TerrainView};
    pub use crate::tile::{Burn, Kindling, TileId, TileProps, TileRegistry, TileTables};
}
