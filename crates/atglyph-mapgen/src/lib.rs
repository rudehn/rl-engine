//! The generation pipeline: a chain of named, phased passes over a context.
//!
//! Use this crate alone, or through `atglyph` as `atglyph::mapgen`.
//! It builds places from a chain of seeded passes, builds with no Bevy, and builds for `wasm32-unknown-unknown`,
//! so a tool, a server or a test can depend on `atglyph-mapgen` and skip compiling Bevy.
//! Its example imports from the crates it builds on, so used alone it lists them at the same version:
//!
//! ```toml
//! [dependencies]
//! atglyph-core = "0.4.0"
//! atglyph-grid = "0.4.0"
//! atglyph-mapgen = "0.4.0"
//! ```
//!
//! ```
//! use atglyph_core::{PlaceSeed, RunSeed};
//! use atglyph_grid::TileRegistry;
//! use atglyph_mapgen::dungeon::{RandomStart, Rooms};
//! use atglyph_mapgen::passes::StartPoint;
//! use atglyph_mapgen::{BaseContext, Chain};
//!
//! // Solid wall, rooms carved from it, and a start placed in one.
//! let tiles = TileRegistry::standard();
//! let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
//! let mut map = BaseContext::blank(40, 24, tiles, wall);
//! let seed = PlaceSeed::of(RunSeed(7), 0);
//! Chain::new().then(Rooms { floor, ..Default::default() }).then(RandomStart).run(&mut map, seed).unwrap();
//!
//! let view = map.terrain().view(map.tiles());
//! let start = map.outputs().first::<StartPoint>().unwrap().0;
//! assert!(view.is_walkable(start), "the start is a walkable cell");
//! assert!(map.terrain().count(floor) > 0);
//! ```
//!
//! Every generated thing in the engine, a dungeon floor or a cave, is built by running a [`Chain`] of [`Pass`]es over a
//! [`BaseContext`]. Three rules make the chain composable:
//!
//! - **A pass's name keys its random stream.** Inserting, removing or
//!   reordering a pass cannot change what any other pass draws.
//! - **Phase order is mandatory.** A chain whose phases go backwards is
//!   refused when it is assembled, not discovered in a screenshot.
//! - **Passes can fail.** A pass that cannot place what it was asked to
//!   says so, and the caller retries with a different index.
//!
//! A place can also be grown: [`Accrete`] attaches each [`Shape`]'s
//! [`Piece`] to a wall of what already stands, biased by [`District`]s,
//! round any [`Barrier`] present, and publishes an [`Area`] per piece and
//! an [`Opening`] per [`Join`];
//! [`OpenLoops`] then cuts the longest detours through, so there is more
//! than one way round. [`Lakes`] lays water or anything else over the
//! floor without ever parting it, and [`Bridges`] crosses it where
//! walking round is long. A [`Cave`] is a shape not built of rectangles,
//! drawn by the cellular automaton, grown and joined like any other, and
//! a [`PrefabShape`](prefab::PrefabShape) is a hand-drawn piece grown as an
//! area of its own, entered only through the entries its file draws.
//! A [`FirstPiece`] is one laid before anything grows, at a placement, for
//! a place to grow round, and published as a [`Landmark`] whose
//! [`Sides`] the way in and the way down may keep to, as
//! [`Stairs`](dungeon::Stairs) says.
//! [`Survey`] measures what was built, as a
//! [`Surveyed`] with an [`AreaFacts`] per area, and [`Require`] fails a
//! place that falls short, for [`Chain::run_retrying`] to build again.
//! [`HideOpenings`] walls over a loop's shortcut or a dead end's one door
//! and publishes each cell as a [`SecretCell`], never one the way down needs.
//! [`StampPicks`] places the picks a run's pool drew for a place in the
//! areas each [`Prefer`]s, the most secluded, a dead end, a junction or a
//! room behind a secret, a piece stamped and a dressing published as a
//! [`DressAt`], and fails the place on a required one that fits nowhere.
//! [`Dress`] lays each [`Dressing`], things along the walls, in the
//! corners, in the center, in rows or scattered at a density, in the area
//! a `DressAt` names and in every room nothing took, by its district,
//! never beside an opening and never cutting an area in two.
//!
//! The values a content file writes, a [`Rotation`](prefab::Rotation), a
//! [`Mirror`](prefab::Mirror), a [`Placement`](prefab::Placement), a
//! [`Prefer`], a [`Stairs`](dungeon::Stairs), a [`Spread`](dress::Spread),
//! a [`DistrictDressings`] and a [`Barrier`], deserialize, so `atglyph-rules` reads its files
//! straight into them rather than into copies it would translate; this
//! crate reads no file itself.
//!
//! Every pass, the engine's and a game's, works on one [`BaseContext`];
//! what a game's pass needs beyond it, it carries in its own fields.
//!
//! [`place`] is what a finished chain becomes: a game's [`PlaceRules`]
//! build each [`MapId`] from its own seed into a [`PlaceBuild`], and a
//! [`Ladder`] says which floors stand one under another and where their
//! stairs go, so a terminal game or a tool builds the same places as the
//! Bevy framework, which re-exports them.
//!
//! [`testing`] plays a generation test over a range of seeds on every
//! core, for this crate's tests and a game's alike.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod accrete;
pub mod cave;
pub mod chain;
pub mod context;
pub mod dress;
pub mod dungeon;
pub mod lakes;
pub mod loops;
pub mod passes;
pub mod place;
pub mod pool;
pub mod prefab;
pub mod secrets;
pub mod survey;
pub mod testing;
mod walk;

pub use accrete::{Accrete, Area, Barrier, District, FirstPiece, Join, LaidBarrier, Landmark, Opening, Piece, Shape, Side, Sides};
pub use cave::Cave;
pub use chain::{BuildError, Chain, Pass, Phase};
pub use context::{BaseContext, Outputs};
pub use dress::{DistrictDressings, Dress, Dressed, Dressing};
pub use lakes::{Bridge, Bridges, Lake, Lakes};
pub use loops::OpenLoops;
pub use place::{Climb, Ladder, LadderEnd, LadderStair, Lock, MapId, PlaceBuild, PlaceRules, Spot, StairLook};
pub use pool::{DressAt, Lay, PoolPick, Prefer, StampPicks};
pub use secrets::{HideOpenings, SecretCell};
pub use survey::{AreaFacts, Require, Survey, Surveyed};

/// The names most callers want in scope.
pub mod prelude {
    // The pass and the trait only. What a game builds one from, `Rectangle`
    // above all, is named where it is built: Bevy's prelude has a
    // `Rectangle` of its own.
    pub use crate::accrete::{Accrete, Shape};
    pub use crate::chain::{BuildError, Chain, Pass, Phase};
    pub use crate::context::{BaseContext, Outputs};
    pub use crate::dress::{DistrictDressings, Dress, Dressed, Dressing};
    pub use crate::dungeon;
    pub use crate::lakes::{Bridges, Lakes};
    pub use crate::loops::OpenLoops;
    pub use crate::passes;
    pub use crate::place::{Ladder, MapId, PlaceBuild, PlaceRules, StairLook};
    pub use crate::pool::{DressAt, Lay, PoolPick, Prefer, StampPicks};
    pub use crate::prefab::{Orient, Placement, Prefab, PrefabShape, StampEachRoom, StampOneOf, StampPrefab, Stamped};
    pub use crate::secrets::{HideOpenings, SecretCell};
    pub use crate::survey::{AreaFacts, Require, Survey, Surveyed};
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph_core::Point;
    use dress::Spread;
    use dungeon::Stairs;
    use prefab::{Mirror, Placement, Rotation};

    fn read<T: serde::de::DeserializeOwned>(text: &str) -> T {
        ron::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"))
    }

    /// Every spelling a prefab, pool or dressing file writes, read into
    /// the type the passes take, as `atglyph-rules` reads it: a variant renamed
    /// here would break every file that writes it.
    #[test]
    fn each_file_spelling_of_a_turn_mirror_prefer_and_stairs_reads_as_before() {
        assert_eq!([read::<Rotation>("None"), read("Half"), read("Quarter")], [Rotation::None, Rotation::Half, Rotation::Quarter]);
        assert_eq!([read::<Mirror>("None"), read("LeftRight"), read("UpDown"), read("Both")], [Mirror::None, Mirror::LeftRight, Mirror::UpDown, Mirror::Both]);
        assert_eq!(
            [read::<Prefer>("Secluded"), read("DeadEnd"), read("Hidden"), read("Junction"), read("Anywhere")],
            [Prefer::Secluded, Prefer::DeadEnd, Prefer::Hidden, Prefer::Junction, Prefer::Anywhere]
        );
        assert_eq!([read::<Stairs>("Free"), read("Beyond"), read("Within('>')")], [Stairs::Free, Stairs::Beyond, Stairs::Within('>')]);
        assert_eq!(
            read::<Vec<Placement>>("[Center, Edge, Anywhere, Span, At((x: 2, y: 3)), InRoom(1), AnyRoom]"),
            [
                Placement::Center,
                Placement::Edge,
                Placement::Anywhere,
                Placement::Span,
                Placement::At(Point::new(2, 3)),
                Placement::InRoom(1),
                Placement::AnyRoom
            ]
        );
        assert_eq!(
            read::<Vec<Spread>>("[Walls, Corners, Center, Rows(aisle: 2), Scatter]"),
            [Spread::Walls, Spread::Corners, Spread::Center, Spread::Rows { aisle: 2 }, Spread::Scatter]
        );
        let list: DistrictDressings = read(r#"(district: "halls", dressings: [("dust", 1)])"#);
        assert_eq!(list, DistrictDressings::new("halls", vec![("dust".into(), 1)]), "nothing is 0 when left out");
        assert!(ron::from_str::<DistrictDressings>(r#"(district: "halls", dressings: [], extra: 1)"#).is_err(), "an unknown field");
        let barrier: Barrier = read("(from: (50, 0), to: (50, 70), chance_pct: 100, jitter_pct: 5)");
        assert_eq!(barrier, Barrier { from: (50, 0), to: (50, 70), chance_pct: 100, jitter_pct: 5 });
        assert!(ron::from_str::<Barrier>("(from: (50, 0), to: (50, 70), chance: 100, jitter_pct: 5)").is_err(), "an unknown field");
        assert!(ron::from_str::<Rotation>("Sideways").is_err(), "a turn no file may write");
    }
}
