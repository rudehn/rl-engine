//! A roguelike engine for Rust and Bevy, re-exported from one crate.
//!
//! rl-engine is a workspace of small crates for turn-based grid
//! roguelikes: procedural dungeon and world generation, field of view, A*
//! and Dijkstra-map pathfinding, monster AI, combat, items, quests, saves
//! and an ASCII glyph renderer. This facade re-exports every one of them,
//! so a game depends on this crate and imports `rl_engine::prelude::*`.
//!
//! The tier 0 and 1 crates ([`rl_core`], [`rl_grid`], [`rl_mapgen`],
//! [`rl_world`], [`rl_rules`]) never
//! depend on Bevy. A tool, a server or a test that needs no window can
//! depend on one of them directly and skip compiling Bevy altogether.
//! The tier-2 crates ([`rl_bevy`], [`rl_render`], [`rl_ui`],
//! [`rl_overworld`], [`rl_save`]) are the Bevy plugins that run the loops.
//!
//! The repository README walks through a headless example, and the
//! `corsair` and `delve` example games show the Bevy side end to end.

#![deny(missing_docs)]

pub use rl_bevy;
pub use rl_core;
pub use rl_grid;
pub use rl_mapgen;
pub use rl_overworld;
pub use rl_render;
pub use rl_rules;
pub use rl_save;
pub use rl_ui;
pub use rl_world;

/// The curated set of names a game needs most of the time.
/// Everything a game reaches for, in one glob.
///
/// ```
/// use bevy::prelude::*;
/// use rl_engine::prelude::*;
/// ```
///
/// `Rect` is deliberately left out. Bevy's prelude has a `Rect` of its
/// own, so a game that globs both would have to disambiguate every use of
/// the name; import [`rl_core::Rect`] where you want the grid one. That is
/// the only name the two preludes would have fought over, and the doc test
/// below is what keeps it that way.
///
/// ```
/// use bevy::prelude::*;
/// use rl_engine::prelude::*;
///
/// // One name from each crate the prelude covers, named rather than
/// // glob-imported, so a future collision with Bevy fails here.
/// fn takes(_: Point, _: Direction, _: TileId, _: Terrain, _: Chain<BaseContext>, _: WorldGraph) {}
/// fn rules(_: Registry<StatDef>, _: Brain<Entity>, _: Ledger, _: MovementProfile) {}
/// fn bevy_side(_: Position, _: Player, _: Health, _: Intent<Step>, _: MapId, _: EngineSet, _: PresentSet) {}
/// fn drawn(_: Glyph, _: Cell, _: Terminal, _: MapView, _: Palette, _: MessageLog, _: OverworldPlugin, _: Saves) {}
/// fn panels(_: NearbyView, _: VitalsView, _: GearView, _: InspectView, _: Row, _: Facet, _: ModalId, _: ViewSet) {}
/// ```
pub mod prelude {
    // Core, minus `Rect`: see the note above.
    pub use rl_bevy::prelude::*;
    pub use rl_core::prelude::{
        BASE_ACTION_COST, DequeueOutcome, DiceRoll, Direction, DirectionSet, DisjointSet, Grid, Grid2D, Id, Interner, Point, RunSeed, SeedDomain, Steps,
        TurnQueue, geometry,
    };
    pub use rl_grid::prelude::*;
    pub use rl_mapgen::prelude::*;
    pub use rl_overworld::prelude::*;
    pub use rl_render::prelude::*;
    pub use rl_rules::prelude::*;
    pub use rl_save::prelude::*;
    pub use rl_ui::prelude::*;
    pub use rl_world::prelude::*;
}

/// The README's code samples, compiled and run as doc-tests so the
/// front page of the repository cannot drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
pub struct ReadmeDoctests;
