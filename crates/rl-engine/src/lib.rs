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
//! `corsair`, `delve`, `heist` and `foundry` example games show the Bevy side
//! end to end.
//!
//! A game starts from [`RoguelikePlugins`], the window, the terminal and
//! the plugins every game adds, and then names the subsystems it wants.
//! A game that saves names itself with [`RoguelikePlugins::saves`], and
//! [`PlayerSaves`] keeps a [`scripted`] run off the player's save.

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
/// grid, drawn in the game's own [`typeface`](Self::typeface) when it
/// ships one, and the [`FullscreenPlugin`](rl_render::FullscreenPlugin) that
/// declares filling the screen as a setting; the engine's [`CorePlugin`](rl_bevy::CorePlugin) and
/// [`FovPlugin`](rl_bevy::FovPlugin); the map, drawn in [`map`](Self::map)'s
/// rectangle, and the [`ParticlesPlugin`](rl_render::ParticlesPlugin) that
/// plays flights and bursts over it; [`UiPlugin`](rl_ui::UiPlugin), the base
/// every panel needs; and
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
    across: i32,
    typeface: Option<Typeface>,
    saves: Option<String>,
}

/// A typeface's bytes, which a derived `Debug` would print every one of:
/// shown by how many there are instead.
#[derive(Clone)]
struct Typeface(std::borrow::Cow<'static, [u8]>);

impl std::fmt::Debug for Typeface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Typeface({} bytes)", self.0.len())
    }
}

impl RoguelikePlugins {
    /// A window titled `title`, `cols` by `rows` cells of ten by sixteen
    /// pixels, with the map filling it.
    pub fn new(title: impl Into<String>, cols: i32, rows: i32) -> Self {
        Self { title: title.into(), cols, rows, cell: Vec2::new(10.0, 16.0), font: 14.0, map: None, across: 1, typeface: None, saves: None }
    }

    /// Saves the run where the player's saves are kept, under `name`, by
    /// adding [`PlayerSaves`]: in memory when the run is [`scripted`].
    /// Without it nothing inserts [`Saves`](rl_save::Saves), and a game
    /// that saves inserts its own.
    pub fn saves(mut self, name: impl Into<String>) -> Self {
        self.saves = Some(name.into());
        self
    }

    /// Draws every glyph in `face`, the bytes of a TrueType or OpenType
    /// file the game ships, rather than in the system's monospace family,
    /// which is another face on every platform and only ASCII in a
    /// browser: a `&'static` slice compiled in, or bytes read at run time,
    /// as [`TerminalPlugin::typeface`](rl_render::TerminalPlugin::typeface)
    /// takes them.
    pub fn typeface(mut self, face: impl Into<std::borrow::Cow<'static, [u8]>>) -> Self {
        self.typeface = Some(Typeface(face.into()));
        self
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

    /// Draws each map tile `cells` cells across: two, for square tiles on
    /// cells half as wide as they are tall, so text keeps a terminal's
    /// proportions and the map keeps a board's.
    pub fn across(mut self, cells: i32) -> Self {
        self.across = cells.max(1);
        self
    }
}

impl PluginGroup for RoguelikePlugins {
    fn build(self) -> PluginGroupBuilder {
        let (width, height) = ((self.cols as f32 * self.cell.x) as u32, (self.rows as f32 * self.cell.y) as u32);
        // On the web the canvas is a fixed size unless it is told to follow
        // its parent, which leaves it overflowing whatever it is embedded in.
        // The field does nothing off the web.
        let window = Window { title: self.title, resolution: WindowResolution::new(width, height), fit_canvas_to_parent: true, ..default() };
        let map = self.map.unwrap_or(rl_core::Rect::new(0, 0, self.cols, self.rows));
        let mut terminal = rl_render::TerminalPlugin::new(self.cols, self.rows, self.cell, self.font);
        if let Some(Typeface(face)) = self.typeface {
            terminal = terminal.typeface(face);
        }
        let group = PluginGroupBuilder::start::<Self>()
            .add_group(
                DefaultPlugins.set(WindowPlugin { primary_window: Some(rl_render::capture::prepare(window)), ..default() }).set(ImagePlugin::default_nearest()),
            )
            .add(terminal)
            // Filling the screen, as a setting the renderer declares.
            .add(rl_render::FullscreenPlugin)
            .add(rl_bevy::CorePlugin)
            .add(rl_bevy::FovPlugin)
            .add(rl_render::MapViewPlugin::new(map).across(self.across))
            // What flies and bursts over the map for a moment after a
            // turn: presentation of the map, not a subsystem, so it comes
            // with the map view.
            .add(rl_render::ParticlesPlugin)
            .add(rl_ui::UiPlugin)
            // A run written down and played back, when the environment
            // asks: `RL_RECORD=run.ron` and `RL_REPLAY=run.ron`.
            .add(rl_ui::ReplayPlugin::from_env())
            .add(rl_render::CapturePlugin);
        match self.saves {
            Some(name) => group.add(PlayerSaves::new(name)),
            None => group,
        }
    }
}

/// Whether this run plays keys from a script rather than from the player:
/// a replay or a recording, named by [`REPLAY_VAR`](rl_bevy::replay::REPLAY_VAR)
/// and [`RECORD_VAR`](rl_bevy::replay::RECORD_VAR), or a
/// [capture](rl_render::capture::requested).
///
/// Read from the environment, so it can be asked before there is an app.
pub fn scripted() -> bool {
    use rl_bevy::replay::{RECORD_VAR, REPLAY_VAR};
    std::env::var_os(REPLAY_VAR).is_some() || std::env::var_os(RECORD_VAR).is_some() || rl_render::capture::requested()
}

/// Inserts the [`Saves`](rl_save::Saves) a run is kept in: the platform's
/// default under the game's name, files beside the executable or browser
/// storage, unless the run is [`scripted`], which saves in memory.
///
/// A scripted run plays keys against a seed of its own, so starting it on
/// the player's save would play a different run, and its first save, an
/// arrival or the window closing, would write over the player's. Every
/// game that saves needs that guard and none should have to remember it,
/// so it is the engine's; [`RoguelikePlugins::saves`] adds this.
#[derive(Debug, Clone)]
pub struct PlayerSaves {
    name: String,
}

impl PlayerSaves {
    /// The saves of the game called `name`, which names its browser
    /// storage.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Where a run that is, or is not, `scripted` saves: what the plugin
    /// inserts, given [`scripted`]'s answer.
    pub fn backend(&self, scripted: bool) -> rl_save::Saves {
        if scripted { rl_save::Saves::new(rl_save::MemoryBackend::default()) } else { rl_save::Saves::platform_default(&self.name) }
    }
}

impl Plugin for PlayerSaves {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.backend(scripted()));
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
/// fn title(_: HoldStart, _: SavedRun, _: ContinueRun, _: AbandonRun, _: LeaveRun) {}
/// let _begin: fn(&mut World) = begin_run;
/// fn records(_: RunRecord, _: RunRecords, _: RecordOutcome, _: RecordSection, _: RecordRun, _: RecordsPlugin, _: RecordsPanel) {}
/// fn drawn(_: Glyph, _: Cell, _: Terminal, _: MapView, _: Palette, _: MessageLog, _: OverworldPlugin, _: Saves) {}
/// // Bevy's own names a game uses beside the engine's: the shape a mesh
/// // is made from, which a grown place's shapes must not shadow.
/// fn bevy_own(_: Rectangle, _: Circle) {}
/// fn panels(_: NearbyView, _: VitalsView, _: GearView, _: InspectView, _: Row, _: Facet, _: ModalId, _: ViewSet) {}
/// fn title_screen(_: TitlePanel, _: TitleWords, _: TitleMenu, _: TitleRow, _: TitleSet) {}
/// let _up = title_is_up;
/// ```
pub mod prelude {
    pub use crate::{PlayerSaves, RoguelikePlugins};
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

#[cfg(test)]
mod tests {
    use rl_save::SaveBackend as _;

    use super::*;

    /// A slot no game saves in, so a test writing it beside the test
    /// binary touches nothing a game reads.
    const SLOT: &str = "rl-engine-test-slot";

    /// A run played from a script, a replay, a recording or a capture,
    /// saves somewhere of its own: it neither starts on the player's save
    /// nor writes over it.
    #[test]
    fn a_scripted_run_never_touches_the_players_save() {
        let real = rl_save::Saves::platform_default("warren");
        let before = real.load(SLOT).unwrap();
        let scripted = PlayerSaves::new("warren").backend(true);
        scripted.persist(SLOT, "a scripted run").unwrap();
        assert_eq!(real.load(SLOT).unwrap(), before, "the player's slot is as it was");
        assert!(scripted.exists(SLOT), "and the scripted run has one of its own");
    }

    /// Naming the game's saves is what adds them, and a game that names
    /// none inserts its own.
    #[test]
    fn the_players_saves_are_added_only_when_a_game_names_them() {
        assert!(RoguelikePlugins::new("Warren", 80, 40).saves("warren").build().contains::<PlayerSaves>());
        assert!(!RoguelikePlugins::new("Warren", 80, 40).build().contains::<PlayerSaves>());
    }
}

/// The README's code samples, compiled and run as doc-tests so the
/// front page of the repository cannot drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
pub struct ReadmeDoctests;
