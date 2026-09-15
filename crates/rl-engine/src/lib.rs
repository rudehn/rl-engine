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
//!
//! A game starts from [`RoguelikePlugins`], the window, the terminal and
//! the plugins every game adds, and then names the subsystems it wants.

#![deny(missing_docs)]

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy::window::WindowResolution;

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

/// What every game on the glyph terminal adds before its own plugins, in
/// one group.
///
/// Bevy's defaults, with a window sized to the terminal and nearest-pixel
/// sampling so glyphs stay sharp; the [`TerminalPlugin`](rl_render::TerminalPlugin)
/// grid; the engine's [`CorePlugin`](rl_bevy::CorePlugin) and
/// [`FovPlugin`](rl_bevy::FovPlugin); the map, drawn in [`map`](Self::map)'s
/// rectangle; [`UiPlugin`](rl_ui::UiPlugin), the base every panel needs; and
/// [`CapturePlugin`](rl_render::CapturePlugin), which does nothing unless
/// `RL_CAPTURE` is set.
///
/// Only what every game adds, and nothing that is a subsystem: combat,
/// items, statuses, lighting, streaming and the rest stay plugins a game
/// names, because a subsystem is opt-in. Anything here can still be
/// switched off or replaced the way Bevy's own groups allow.
///
/// ```no_run
/// use bevy::prelude::*;
/// use rl_engine::prelude::*;
/// use rl_engine::rl_core::Rect;
///
/// App::new()
///     .add_plugins(RoguelikePlugins::new("Warren", 80, 40).map(Rect::new(0, 1, 80, 34)).build().disable::<CapturePlugin>())
///     .add_plugins((CombatPlugin, MindsPlugin))
///     .run();
/// ```
#[derive(Debug, Clone)]
pub struct RoguelikePlugins {
    title: String,
    cols: i32,
    rows: i32,
    cell: Vec2,
    font: f32,
    map: Option<rl_core::Rect>,
}

impl RoguelikePlugins {
    /// A window titled `title`, `cols` by `rows` cells of ten by sixteen
    /// pixels, with the map filling it.
    pub fn new(title: impl Into<String>, cols: i32, rows: i32) -> Self {
        Self { title: title.into(), cols, rows, cell: Vec2::new(10.0, 16.0), font: 14.0, map: None }
    }

    /// Each cell's size in pixels.
    pub fn cell(mut self, size: Vec2) -> Self {
        self.cell = size;
        self
    }

    /// The glyph height in pixels, a little under the cell's.
    pub fn font(mut self, size: f32) -> Self {
        self.font = size;
        self
    }

    /// The terminal cells the map is drawn in; the rest is left to panels.
    pub fn map(mut self, viewport: rl_core::Rect) -> Self {
        self.map = Some(viewport);
        self
    }
}

impl PluginGroup for RoguelikePlugins {
    fn build(self) -> PluginGroupBuilder {
        let (width, height) = ((self.cols as f32 * self.cell.x) as u32, (self.rows as f32 * self.cell.y) as u32);
        let window = Window { title: self.title, resolution: WindowResolution::new(width, height), ..default() };
        let map = self.map.unwrap_or(rl_core::Rect::new(0, 0, self.cols, self.rows));
        PluginGroupBuilder::start::<Self>()
            .add_group(
                DefaultPlugins.set(WindowPlugin { primary_window: Some(rl_render::capture::prepare(window)), ..default() }).set(ImagePlugin::default_nearest()),
            )
            .add(rl_render::TerminalPlugin { width: self.cols, height: self.rows, cell_size: self.cell, font_size: self.font })
            .add(rl_bevy::CorePlugin)
            .add(rl_bevy::FovPlugin)
            .add(rl_render::MapViewPlugin::new(map))
            .add(rl_ui::UiPlugin)
            .add(rl_render::CapturePlugin)
    }
}

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
/// fn rules(_: Registry<StatDef>, _: NameRef<StatDef>, _: Names, _: Brain<Entity>, _: Ledger, _: MovementProfile) {}
/// fn bevy_side(_: Position, _: Player, _: Health, _: Intent<Step>, _: MapId, _: EngineSet, _: PresentSet) {}
/// fn drawn(_: Glyph, _: Cell, _: Terminal, _: MapView, _: Palette, _: MessageLog, _: OverworldPlugin, _: Saves) {}
/// fn panels(_: NearbyView, _: VitalsView, _: GearView, _: InspectView, _: Row, _: Facet, _: ModalId, _: ViewSet) {}
/// ```
pub mod prelude {
    pub use crate::RoguelikePlugins;
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
