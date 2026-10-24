//! A roguelike engine for Rust and Bevy, re-exported from one crate.
//!
//! Start a game from the template, which plays from the first build:
//!
//! ```sh
//! cargo install cargo-generate
//! cargo generate --git https://github.com/rudehn/rl-engine --tag v0.3.0 templates/starter --name my-game
//! ```
//!
//! The generated game is one file: a floor of rooms lit by a torch, goblins
//! that notice the player by sight and hunt where they last saw them,
//! potions, a dagger and a coat to find, a status row and a log, in a window or, with `cargo run -- --tty`, in a
//! text terminal. [The guide](https://rudehn.github.io/rl-engine/) builds
//! the same game a step at a time, the first six steps playable in the
//! browser.
//!
//! atglyph is a workspace of small crates for turn-based grid
//! roguelikes: procedural dungeon generation, field of view, A*
//! and Dijkstra-map pathfinding, monster AI, combat, items, quests, saves
//! and an ASCII glyph renderer. This facade re-exports every one of them,
//! so a game depends on this crate and imports `atglyph::prelude::*`.
//!
//! Four of the crates never depend on Bevy: `atglyph-core`
//! (`atglyph::primitives`), `atglyph-grid` (`atglyph::grid`),
//! `atglyph-mapgen` (`atglyph::mapgen`) and `atglyph-rules`
//! (`atglyph::rules`). A tool, a server or a test that needs no window can
//! depend on one of them directly and skip compiling Bevy altogether; each
//! opens its own docs with an example that needs none. The tier-2 crates,
//! `atglyph::ecs`, `atglyph::render`, `atglyph::ui` and `atglyph::save`,
//! are the Bevy plugins that run the loops, and a game reaches them only
//! through this crate.
//!
//! The repository README walks through a headless example, and the
//! `foundry` and `undermere` example games show the Bevy side end to end.
//!
//! A game starts from [`RoguelikePlugins`], the window, the terminal and
//! the plugins every game adds, and then names the subsystems it wants; or
//! from [`TtyRoguelikePlugins`], the same with a text terminal in place of
//! the window, played locally or over SSH. A
//! game that saves names itself with
//! [`SavePlugin::new`](atglyph_save::SavePlugin::new), which also keeps a
//! [scripted](atglyph_bevy::replay::scripted) run off the player's save.
//!
//! [`testing`] is the same game without a window, for its tests: the
//! headless app with the UI base, the game's narrator and saves kept in
//! memory, and every helper the engine's own tests drive and read one with.
//!
//! `atglyph::bevy` and `atglyph::rand` re-export the Bevy and the `rand`
//! the engine is built on, so a game draws from `atglyph::rand` and lists no
//! `rand` of its own. A game still lists `bevy` in its Cargo.toml, at the
//! same version (`0.19`), because Bevy's derive macros look for it in the
//! game's own manifest and emit paths that do not resolve without it.
//!
//! ```
//! use atglyph::rand::{Rng, SeedableRng, rngs::StdRng};
//! let mut rng = StdRng::seed_from_u64(7);
//! let roll: i32 = rng.random_range(1..=6);
//! assert!((1..=6).contains(&roll));
//! let _app = atglyph::bevy::app::App::new();
//! ```

#![deny(missing_docs)]

pub mod testing;

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy::window::WindowResolution;

pub use bevy;
pub use rand;

pub use atglyph_bevy as ecs;
pub use atglyph_core as primitives;
pub use atglyph_grid as grid;
pub use atglyph_mapgen as mapgen;
pub use atglyph_render as render;
pub use atglyph_rules as rules;
pub use atglyph_save as save;
#[cfg(not(target_arch = "wasm32"))]
pub use atglyph_tty as tty;
pub use atglyph_ui as ui;

/// What every game on the glyph terminal adds before its own plugins, in
/// one group.
///
/// Bevy's defaults, with a window sized to the terminal and nearest-pixel
/// sampling so glyphs stay sharp; the [`TerminalPlugin`](atglyph_render::TerminalPlugin)
/// grid, drawn in the game's own [`typeface`](Self::typeface) when it
/// ships one, and the [`FullscreenPlugin`](atglyph_render::FullscreenPlugin) that
/// declares filling the screen as a setting; the engine's [`CorePlugin`](atglyph_bevy::CorePlugin) and
/// [`FovPlugin`](atglyph_bevy::FovPlugin); the map, drawn in [`map`](Self::map)'s
/// rectangle, and the [`ParticlesPlugin`](atglyph_render::ParticlesPlugin) that
/// plays flights and bursts over it; [`UiPlugin`](atglyph_ui::UiPlugin), the base
/// every panel needs; and
/// [`CapturePlugin`](atglyph_render::CapturePlugin), which does nothing unless
/// `ATGLYPH_CAPTURE` is set.
///
/// Only what every game adds, and nothing that is a subsystem: combat,
/// items, statuses, lighting, streaming and the rest stay plugins a game
/// names, because a subsystem is opt-in. Anything here can still be
/// switched off or replaced the way Bevy's own groups allow.
///
/// ```no_run
/// use bevy::prelude::*;
/// use atglyph::prelude::*;
///
/// App::new()
///     .add_plugins(RoguelikePlugins::new("Warren", 80, 40).map(GridRect::new(0, 1, 80, 34)).build().disable::<CapturePlugin>())
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
    map: Option<atglyph_core::GridRect>,
    across: i32,
    typeface: Option<Typeface>,
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
        Self { title: title.into(), cols, rows, cell: Vec2::new(10.0, 16.0), font: 14.0, map: None, across: 1, typeface: None }
    }

    /// Draws every glyph in `face`, the bytes of a TrueType or OpenType
    /// file the game ships, rather than in the system's monospace family,
    /// which is another face on every platform and only ASCII in a
    /// browser: a `&'static` slice compiled in, or bytes read at run time,
    /// as [`TerminalPlugin::typeface`](atglyph_render::TerminalPlugin::typeface)
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
    pub fn map(mut self, viewport: atglyph_core::GridRect) -> Self {
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
        let map = self.map.unwrap_or(atglyph_core::GridRect::new(0, 0, self.cols, self.rows));
        let mut terminal = atglyph_render::TerminalPlugin::new(self.cols, self.rows, self.cell, self.font);
        if let Some(Typeface(face)) = self.typeface {
            terminal = terminal.typeface(face);
        }
        PluginGroupBuilder::start::<Self>()
            .add_group(
                DefaultPlugins
                    .set(WindowPlugin { primary_window: Some(atglyph_render::capture::prepare(window)), ..default() })
                    .set(ImagePlugin::default_nearest()),
            )
            .add(terminal)
            // Filling the screen, as a setting the renderer declares.
            .add(atglyph_render::FullscreenPlugin)
            .add(atglyph_bevy::CorePlugin)
            .add(atglyph_bevy::FovPlugin)
            .add(atglyph_render::MapViewPlugin::new(map).across(self.across))
            // What flies and bursts over the map for a moment after a
            // turn: presentation of the map, not a subsystem, so it comes
            // with the map view.
            .add(atglyph_render::ParticlesPlugin)
            .add(atglyph_ui::UiPlugin)
            // A run written down and played back, when the environment
            // asks: `ATGLYPH_RECORD=run.ron` and `ATGLYPH_REPLAY=run.ron`.
            .add(atglyph_ui::ReplayPlugin::from_env())
            .add(atglyph_render::CapturePlugin)
    }
}

/// What every game on the glyph terminal adds before its own plugins, for
/// a text terminal rather than a window: the one the game was started in,
/// at a desk or at the end of an SSH login.
///
/// The engine's half of [`RoguelikePlugins`], [`CorePlugin`](atglyph_bevy::CorePlugin),
/// [`FovPlugin`](atglyph_bevy::FovPlugin), the map in [`map`](Self::map)'s
/// rectangle with its [`ParticlesPlugin`](atglyph_render::ParticlesPlugin),
/// [`UiPlugin`](atglyph_ui::UiPlugin) and the [`ReplayPlugin`](atglyph_ui::ReplayPlugin),
/// over Bevy's `MinimalPlugins` running thirty frames a second, its states
/// and its input, with the [`TtyPlugin`](atglyph_tty::TtyPlugin) drawing the
/// same grid every panel draws into. So a game adds its own plugins after
/// either group and plays the same.
///
/// Nothing that needs a window: no capture, no fullscreen setting, and no
/// typeface, since the terminal draws in its own. No `LogPlugin` either,
/// whose lines would land on the game's screen. The GPU crates are still
/// compiled, since the workspace names them, but never started.
///
/// A game served to many people over SSH swaps the plugin for
/// [`TtyPlugin::to_writer`](atglyph_tty::TtyPlugin::to_writer) the way Bevy's
/// groups allow, and saves each player's run apart with
/// [`Saves::per_user`](atglyph_save::Saves::per_user).
///
/// ```no_run
/// use bevy::prelude::*;
/// use atglyph::prelude::*;
///
/// App::new().add_plugins(TtyRoguelikePlugins::new(80, 40).map(GridRect::new(0, 1, 80, 34))).add_plugins((CombatPlugin, MindsPlugin)).run();
/// ```
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct TtyRoguelikePlugins {
    cols: i32,
    rows: i32,
    map: Option<atglyph_core::GridRect>,
    across: i32,
}

#[cfg(not(target_arch = "wasm32"))]
impl TtyRoguelikePlugins {
    /// A grid `cols` by `rows` cells, with the map filling it.
    pub fn new(cols: i32, rows: i32) -> Self {
        Self { cols, rows, map: None, across: 1 }
    }

    /// The terminal cells the map is drawn in; the rest is left to panels.
    pub fn map(mut self, viewport: atglyph_core::GridRect) -> Self {
        self.map = Some(viewport);
        self
    }

    /// Draws each map tile `cells` cells across, as
    /// [`RoguelikePlugins::across`] does.
    pub fn across(mut self, cells: i32) -> Self {
        self.across = cells.max(1);
        self
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PluginGroup for TtyRoguelikePlugins {
    fn build(self) -> PluginGroupBuilder {
        let map = self.map.unwrap_or(atglyph_core::GridRect::new(0, 0, self.cols, self.rows));
        // Thirty frames a second: a terminal repeats a held key about as
        // often, and a frame with nothing to write writes nothing.
        let frames = bevy::app::ScheduleRunnerPlugin::run_loop(std::time::Duration::from_secs_f64(1.0 / 30.0));
        PluginGroupBuilder::start::<Self>()
            .add_group(MinimalPlugins.set(frames))
            .add(bevy::state::app::StatesPlugin)
            .add(bevy::input::InputPlugin)
            .add(atglyph_tty::TtyPlugin::new(self.cols, self.rows))
            .add(atglyph_bevy::CorePlugin)
            .add(atglyph_bevy::FovPlugin)
            .add(atglyph_render::MapViewPlugin::new(map).across(self.across))
            .add(atglyph_render::ParticlesPlugin)
            .add(atglyph_ui::UiPlugin)
            .add(atglyph_ui::ReplayPlugin::from_env())
    }
}

/// The curated set of names a game needs most of the time.
/// Everything a game reaches for, in one glob.
///
/// ```
/// use bevy::prelude::*;
/// use atglyph::prelude::*;
/// ```
///
/// The grid's rectangle is [`GridRect`](atglyph_core::GridRect), so it sits
/// beside Bevy's own `Rect` and neither glob shadows the other; the doc
/// test below names both, which is what keeps it that way.
///
/// ```
/// use bevy::prelude::*;
/// use atglyph::prelude::*;
///
/// // One name from each crate the prelude covers, named rather than
/// // glob-imported, so a future collision with Bevy fails here.
/// fn takes(_: Point, _: GridRect, _: Rect, _: Direction, _: TileId, _: Terrain, _: Chain) {}
/// fn rules(_: Registry<StatDef>, _: NameRef<StatDef>, _: Names, _: Brain<Entity>, _: Ledger, _: MovementProfile) {}
/// fn bevy_side(_: Position, _: Player, _: Health, _: Intent<Step>, _: MapId, _: EngineSet, _: PresentSet) {}
/// fn title(_: HoldStart, _: SavedRun, _: ContinueRun, _: AbandonRun, _: LeaveRun) {}
/// let _begin: fn(&mut World) = begin_run;
/// fn records(_: RunRecord, _: RunRecords, _: RecordOutcome, _: RecordSection, _: RecordRun, _: RecordsPlugin, _: RecordsPanel) {}
/// fn drawn(_: Glyph, _: Cell, _: Terminal, _: MapView, _: Palette, _: MessageLog, _: Log, _: Saves) {}
/// // Bevy's own names a game uses beside the engine's: the shape a mesh
/// // is made from, which a grown place's shapes must not shadow.
/// fn bevy_own(_: Rectangle, _: Circle) {}
/// fn panels(_: NearbyView, _: VitalsView, _: GearView, _: InspectView, _: Row, _: Facet, _: ModalId, _: ViewSet) {}
/// fn title_screen(_: TitlePanel, _: TitleWords, _: TitleMenu, _: TitleRow, _: TitleSet) {}
/// let _up = title_is_up;
/// ```
pub mod prelude {
    pub use crate::RoguelikePlugins;
    #[cfg(not(target_arch = "wasm32"))]
    pub use crate::TtyRoguelikePlugins;
    pub use atglyph_bevy::prelude::*;
    pub use atglyph_core::prelude::{
        BASE_ACTION_COST, DequeueOutcome, DiceRoll, Direction, Grid, Grid2D, GridRect, Id, Interner, PlaceSeed, Point, RunSeed, SeedDomain, Steps, TurnQueue,
        geometry,
    };
    pub use atglyph_grid::prelude::*;
    pub use atglyph_mapgen::prelude::*;
    pub use atglyph_render::prelude::*;
    pub use atglyph_rules::prelude::*;
    pub use atglyph_save::prelude::*;
    pub use atglyph_ui::prelude::*;
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::prelude::*;
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A writer whose bytes the test reads back.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl std::io::Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// One room, the player in it.
    struct Room(TileRegistry);

    impl PlaceRules for Room {
        fn build(&self, _: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
            let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
            let mut ctx = BaseContext::blank(20, 10, self.0.clone(), wall);
            Chain::new().then(dungeon::Rooms { floor, attempts: 20, min_size: 4, max_size: 6, min_rooms: 1 }).then(dungeon::RandomStart).run(&mut ctx, seed)?;
            PlaceBuild::from_context(ctx)
        }

        fn tiles(&self) -> TileTables {
            self.0.tables()
        }
    }

    fn start(mut commands: Commands) {
        commands.spawn_player(MapId::FIRST, (Viewshed::new(6), RevealsMap, Glyph::new('@', Color::WHITE)));
    }

    /// The group with its terminal written to a buffer, a game added after
    /// it as after the window's, and a key typed into the terminal: the
    /// wait it is bound to passes a turn, and the player is on the screen.
    #[test]
    fn the_terminal_group_builds_an_app_that_plays_a_turn() {
        let out = Shared::default();
        let mut app = App::new();
        app.add_plugins(TtyRoguelikePlugins::new(30, 12).build().set(atglyph_tty::TtyPlugin::to_writer(30, 12, out.clone())))
            .add_plugins(PlayerKeysPlugin::new().walk("Move", "walk").wait("Act", "wait a turn"))
            .places(Room(TileRegistry::standard()))
            .add_systems(NewRun, start)
            .insert_resource(Seed(RunSeed(3)));
        testing::player(&mut app);
        let now = app.world().resource::<Turns>().now();
        let wait =
            atglyph_tty::crossterm::event::KeyEvent::new(atglyph_tty::crossterm::event::KeyCode::Char('.'), atglyph_tty::crossterm::event::KeyModifiers::NONE);
        app.world_mut().write_message(atglyph_tty::TtyInput(atglyph_tty::crossterm::event::Event::Key(wait)));
        testing::settle(&mut app);
        assert!(app.world().resource::<Turns>().now() > now, "the wait typed into the terminal passed a turn");
        let screen = String::from_utf8(out.0.lock().unwrap().clone()).unwrap();
        assert!(screen.contains('@'), "and the player was drawn to it");
    }
}

/// The README's code samples, compiled and run as doc-tests so the
/// front page of the repository cannot drift from the API.
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
pub(crate) struct ReadmeDoctests;
