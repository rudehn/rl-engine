//! What the engine knows about a region, handed to the game to name.

use serde::{Deserialize, Serialize};

/// A game-defined terrain band: biome, zone, whatever the game calls it.
///
/// Opaque to the engine. The game's [`Classifier`] assigns one per region
/// and the game's tables give it meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct BandId(pub u16);

/// How high a region sits, cut from this world's own distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Relief {
    /// Below sea level.
    Water,
    /// Land below the hill threshold.
    Lowland,
    /// Hilly.
    Hill,
    /// Mountainous.
    Mountain,
    /// Bare summit.
    Peak,
}

/// Everything the engine can say about one region, theme-free.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellFacts {
    /// Under the sea.
    pub is_sea: bool,
    /// A lake formed by hydrology.
    pub is_lake: bool,
    /// River width class, 0 where there is no river.
    pub river_width: u8,
    /// Height band.
    pub relief: Relief,
    /// Raw height in `[0, 1]` across the whole map.
    pub height: f32,
    /// Height above sea level, summit at 1, zero over water.
    pub land_height: f32,
    /// 0 polar, 1 equatorial.
    pub temperature: f32,
    /// 0 arid, 1 saturated.
    pub moisture: f32,
    /// Cells of land to the nearest water (sea, lake or river). 0 on water.
    pub distance_to_water: u16,
    /// 0 at the equator, 1 at a pole.
    pub latitude: f32,
    /// Land with sea in one of its eight neighbours.
    pub is_coast: bool,
}

impl CellFacts {
    /// Ordinary temperate lowland with no water anywhere near, for
    /// fixtures and tests.
    pub const fn plain() -> Self {
        Self {
            is_sea: false,
            is_lake: false,
            river_width: 0,
            relief: Relief::Lowland,
            height: 0.5,
            land_height: 0.3,
            temperature: 0.6,
            moisture: 0.5,
            distance_to_water: 10,
            latitude: 0.3,
            is_coast: false,
        }
    }

    /// Open sea.
    pub const fn sea() -> Self {
        Self {
            is_sea: true,
            relief: Relief::Water,
            height: 0.1,
            land_height: 0.0,
            moisture: 1.0,
            distance_to_water: 0,
            ..Self::plain()
        }
    }

    /// Sea or lake.
    pub fn is_water(&self) -> bool {
        self.is_sea || self.is_lake
    }

    /// Whether a river runs through.
    pub fn has_river(&self) -> bool {
        self.river_width > 0
    }
}

/// Names a region's band from the engine's facts about it.
///
/// This is where a game's vocabulary enters: a fantasy game answers
/// "tundra", a pirate game "reef", a sci-fi game "irradiated waste".
pub trait Classifier {
    /// The band for a region with these facts.
    fn classify(&self, facts: &CellFacts) -> BandId;
}

impl<F: Fn(&CellFacts) -> BandId> Classifier for F {
    fn classify(&self, facts: &CellFacts) -> BandId {
        self(facts)
    }
}
