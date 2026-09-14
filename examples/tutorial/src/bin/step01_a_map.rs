//! Warren, step 1: a floor on the screen with the player standing on it.
//!
//! The guide chapter is `docs/guide/src/01-a-map-on-screen.md`. Nothing
//! moves yet: this is the smallest program that generates a map, hands it
//! to the engine and draws it from the player's point of view.
//!
//! `cargo run -p tutorial --bin step01_a_map`

use bevy::prelude::*;
use rl_engine::prelude::*;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;

/// Map zero is the streamed surface. The warren has no surface, so its
/// one floor is map one.
const WARREN: MapId = MapId(1);

// ANCHOR: main
fn main() -> AppExit {
    let mut app = App::new();
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map across the whole terminal, and the UI base.
    // `CapturePlugin` inside it only takes this guide's screenshots.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS)).insert_resource(Seed(RunSeed(7))).add_systems(Startup, start);
    app.run()
}
// ANCHOR_END: main

/// The seed the whole run derives from.
#[derive(Resource, Clone, Copy)]
struct Seed(RunSeed);

// ANCHOR: tiles
/// The warren's tiles, and how each one looks in full light.
struct Warren {
    tiles: TileRegistry,
    seed: RunSeed,
}

impl Warren {
    fn new(seed: RunSeed) -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("earth")).unwrap();
        tiles.register(TileProps::floor("dirt")).unwrap();
        Self { tiles, seed }
    }

    /// Both colours of every tile, and how much each cell jitters from
    /// its neighbours. The renderer derives darkness and memory from these.
    fn appearance(&self) -> TileAppearance {
        let mut look = TileAppearance::new();
        let t = |name| self.tiles.expect(name);
        look.set_varied(t("earth"), Cell::new('#', Color::srgb(0.78, 0.66, 0.50)).on(Color::srgb(0.34, 0.27, 0.21)), Vary::new(0.20, 0.05));
        look.set_varied(t("dirt"), Cell::new('.', Color::srgb(0.66, 0.58, 0.45)).on(Color::srgb(0.18, 0.15, 0.12)), Vary::new(0.28, 0.06));
        look
    }
}
// ANCHOR_END: tiles

// ANCHOR: rules
/// How a floor is built. The engine calls this once, the first time
/// something enters the map, and keeps what comes back.
impl PlaceRules for Warren {
    fn build(&self, _: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let (wall, floor) = (self.tiles.expect("earth"), self.tiles.expect("dirt"));
        let mut ctx = BaseContext::blank(84, 42, self.tiles.clone(), wall);
        Chain::new()
            .then(dungeon::Rooms { floor, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 })
            .then(dungeon::RandomStart)
            .run(&mut ctx, self.seed)?;
        PlaceBuild::from_context(ctx)
    }
}
// ANCHOR_END: rules

// ANCHOR: start
/// Hands the engine the map rules and the player, then warps the player in.
fn start(mut commands: Commands, seed: Res<Seed>, mut warps: MessageWriter<WarpRequest>, mut next: ResMut<NextState<EngineState>>) {
    let warren = Warren::new(seed.0);
    commands.insert_resource(warren.appearance());
    commands.insert_resource(WorldMap::new(warren.tiles.tables()));
    commands.insert_resource(PlaceRulesRes(Box::new(warren)));

    let player = commands
        .spawn(((Actor, Player, Blocks, Position(Point::ZERO), Speed(100)), (Viewshed::new(9), RevealsMap, Glyph::new('@', Color::WHITE).on_layer(10))))
        .id();
    warps.write(WarpRequest::into_place(player, WARREN));
    next.set(EngineState::Playing);
}
// ANCHOR_END: start
