//! Tier 1 of atglyph: everything that runs over a grid of tiles.
//!
//! Use this crate alone, or through `atglyph` as `atglyph::grid`.
//! It runs field of view, light, pathfinding and flood fills over a grid of tiles, builds with no Bevy, and builds for `wasm32-unknown-unknown`,
//! so a tool, a server or a test can depend on `atglyph-grid` and skip compiling Bevy.
//!
//! ```
//! use atglyph_core::Point;
//! use atglyph_grid::{BitGrid, TileRegistry, Terrain, fov};
//!
//! // Seven by three, with a wall down the middle column.
//! let tiles = TileRegistry::standard();
//! let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
//! let terrain = Terrain::from_fn(7, 3, |p| if p.x == 3 { wall } else { floor });
//! let view = terrain.view(&tiles);
//!
//! let mut seen = BitGrid::new(7, 3);
//! fov::compute(&view, Point::new(1, 1), 8, &mut seen);
//! assert!(seen.contains(Point::new(2, 1)), "the floor before the wall is seen");
//! assert!(!seen.contains(Point::new(5, 1)), "a cell behind the wall is not");
//! ```
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
//!   [`LightField`] of intensity and colour, and the [`LightBand`] an
//!   intensity falls in. A [`Glow`] is what one sheds, the one light
//!   description a tile kind, a prop kind, a game's content and the Bevy
//!   layer's light source all carry.
//! - [`AStar`]: point-to-point search with reusable scratch buffers.
//! - [`DijkstraMap`]: one flood, any number of consumers.
//! - [`SpatialGrid`]: who is standing where.
//! - [`landing`]: where a thing that is pushed or thrown comes to rest.
//! - [`slide`]: where a push or pull along a line ends.
//! - [`region`]: flood fill and connected-component labelling.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod astar;
pub mod bitgrid;
pub mod dijkstra;
pub mod field;
pub mod fov;
pub mod landing;
pub mod light;
pub mod region;
pub mod slide;
pub mod spatial;
pub mod targeting;
pub mod terrain;
pub mod tile;

pub use astar::{AStar, PathRules};
pub use bitgrid::BitGrid;
pub use dijkstra::DijkstraMap;
pub use field::{Around, TileField};
pub use light::{Emitter, Glow, Light, LightBand, LightField, Rgb};
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
    pub use crate::light::{Emitter, Glow, Light, LightBand, LightField, Rgb};

    pub use crate::spatial::SpatialGrid;
    pub use crate::targeting::{Footprint, TargetMode, burst, clear_shot, footprint};
    pub use crate::terrain::{CostSource, OpacitySource, Terrain, TerrainView};
    pub use crate::tile::{Burn, Kindling, TileId, TileProps, TileRegistry, TileTables};
}
