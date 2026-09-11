//! World structure for a continuous open world.
//!
//! A world is two layers. The coarse layer, [`WorldGraph`], is a grid of
//! regions: elevation, hydrology, climate, a band per region chosen by the
//! game, sites and roads. It generates in milliseconds and is what the
//! overworld screen draws. The fine layer is chunks: one region's worth of
//! tiles, generated on demand from the region, its neighbours and the run
//! seed, and stitched at the seams by arithmetic rather than negotiation.
//!
//! The engine owns the mechanisms: noise sampling that agrees at both
//! scales, quantile banding, priority-flood hydrology, scored placement, the
//! road router, and the seam primitive. The game owns the vocabulary: which
//! bands exist, how they cost a road, where its site kinds go, and what
//! tiles a chunk is painted with. Those come in through [`WorldRules`] and
//! [`ChunkRules`].

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod chunk;
pub mod climate;
pub mod elevation;
pub mod facts;
pub mod fine;
pub mod graph;
pub mod hydrology;
pub mod noise;
pub mod roads;
pub mod sites;

pub use chunk::{ChunkContext, ChunkRules, RegionFacts, Surroundings, crossing_offset};
pub use climate::{Climate, ClimateConfig};
pub use elevation::{Elevation, ElevationConfig};
pub use facts::{BandId, CellFacts, Classifier, Relief};
pub use fine::{FineFields, bilinear};
pub use graph::{Layers, WorldConfig, WorldGraph, WorldRules};
pub use hydrology::{Hydrology, HydrologyConfig};
pub use noise::{Fbm, SampleSpace};
pub use roads::{RoadConfig, Roads, Route};
pub use sites::{Clearance, PlacementRules, Site, SiteKindId, place_scored};

/// The names most callers want in scope.
pub mod prelude {
    pub use crate::chunk::{ChunkContext, ChunkRules, RegionFacts, Surroundings};
    pub use crate::facts::{BandId, CellFacts, Classifier, Relief};
    pub use crate::graph::{Layers, WorldConfig, WorldGraph, WorldRules};
    pub use crate::roads::{RoadConfig, Roads};
    pub use crate::sites::{Clearance, PlacementRules, Site, SiteKindId, place_scored};
}
