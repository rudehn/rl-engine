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
//! - [`fov`]: symmetric shadowcasting into a [`BitGrid`].
//! - [`AStar`]: point-to-point search with reusable scratch buffers.
//! - [`DijkstraMap`]: one flood, any number of consumers.
//! - [`SpatialGrid`]: who is standing where.
//! - [`region`]: flood fill and connected-component labelling.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod astar;
pub mod bitgrid;
pub mod dijkstra;
pub mod fov;
pub mod region;
pub mod spatial;
pub mod targeting;
pub mod terrain;
pub mod tile;

pub use astar::{AStar, PathRules};
pub use bitgrid::BitGrid;
pub use dijkstra::DijkstraMap;
pub use spatial::SpatialGrid;
pub use targeting::{Footprint, TargetMode, clear_shot, footprint};
pub use terrain::{CostSource, OpacitySource, Terrain, TerrainView};
pub use tile::{TileId, TileProps, TileRegistry, TileTables};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::astar::{AStar, PathRules};
    pub use crate::bitgrid::BitGrid;
    pub use crate::dijkstra::DijkstraMap;
    pub use crate::fov;
    pub use crate::region;
    pub use crate::spatial::SpatialGrid;
    pub use crate::targeting::{Footprint, TargetMode, clear_shot, footprint};
    pub use crate::terrain::{CostSource, OpacitySource, Terrain, TerrainView};
    pub use crate::tile::{TileId, TileProps, TileRegistry, TileTables};
}
