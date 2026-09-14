//! Warren, step 3: what the player can see, and what it remembers.
//!
//! The guide chapter is `docs/guide/src/03-what-the-player-knows.md`.
//! Doors block sight, the explored map fills in behind the player, and
//! the game gets a status line and a message log to talk through.
//!
//! `cargo run -p tutorial --bin step03_sight`
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk, `.` to wait, `q` to quit.

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows at the bottom of the terminal given over to the message log.
const LOG_ROWS: i32 = 5;

/// Map zero is the streamed surface. The warren has no surface, so its
/// one floor is map one.
const WARREN: MapId = MapId(1);

// ANCHOR: main
fn main() -> AppExit {
    let mut app = App::new();
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map in everything but the status row and the log, and the UI base.
    // `CapturePlugin` inside it only takes this guide's screenshots.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        .insert_resource(Seed(RunSeed(7)))
        // Two panels: the vitals strip on the top row, the log along the
        // bottom. Each draws itself; neither needs a system of yours.
        .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)).hints("[.] wait  [q]uit"))
        .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
        .add_systems(Startup, start)
        // Once a frame, before the turns: whatever the player pressed becomes
        // at most one intent, however many passes the turn loop then runs.
        .add_systems(Update, player_input.in_set(EngineSet::Input))
        .add_systems(Update, note_explored.in_set(ViewSet::Annotate));
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
        // Walkable and opaque: you can step through a curtain of roots,
        // but you cannot see past one until you do.
        tiles.register(TileProps::floor("roots").opaque(true)).unwrap();
        Self { tiles, seed }
    }

    /// Both colours of every tile, and how much each cell jitters from
    /// its neighbours. The renderer derives darkness and memory from these.
    fn appearance(&self) -> TileAppearance {
        let mut look = TileAppearance::new();
        let t = |name| self.tiles.expect(name);
        look.set_varied(t("earth"), Cell::new('#', Color::srgb(0.78, 0.66, 0.50)).on(Color::srgb(0.34, 0.27, 0.21)), Vary::new(0.20, 0.05));
        look.set_varied(t("dirt"), Cell::new('.', Color::srgb(0.66, 0.58, 0.45)).on(Color::srgb(0.18, 0.15, 0.12)), Vary::new(0.28, 0.06));
        look.set_varied(t("roots"), Cell::new('+', Color::srgb(0.55, 0.74, 0.45)).on(Color::srgb(0.16, 0.22, 0.13)), Vary::new(0.18, 0.05));
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
            .then(dungeon::Doors { door: self.tiles.expect("roots") })
            .then(dungeon::RandomStart)
            .run(&mut ctx, self.seed)?;
        PlaceBuild::from_context(ctx)
    }
}
// ANCHOR_END: rules

// ANCHOR: start
/// Hands the engine the map rules and the player, then warps the player in.
fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    mut warps: MessageWriter<WarpRequest>,
    mut log: ResMut<MessageLog>,
    mut next: ResMut<NextState<EngineState>>,
) {
    let warren = Warren::new(seed.0);
    commands.insert_resource(warren.appearance());
    commands.insert_resource(WorldMap::new(warren.tiles.tables()));
    commands.insert_resource(PlaceRulesRes(Box::new(warren)));

    let player =
        commands.spawn(((Actor, Player, Blocks, Position(Point::ZERO)), (Viewshed::new(9), RevealsMap, Glyph::new('@', Color::WHITE).on_layer(10)))).id();
    warps.write(WarpRequest::into_place(player, WARREN));
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), Tones::NOTICE, 0);
    log.push("Walk with the arrows, hjklyubn or the numpad. . waits, q quits.", Tones::MUTED, 0);
    next.set(EngineState::Playing);
}
// ANCHOR_END: start

// ANCHOR: keys
/// The eight directions and every key that asks for each.
const MOVES: [(&[KeyCode], Direction); 8] = [
    (&[KeyCode::ArrowUp, KeyCode::KeyK, KeyCode::Numpad8], Direction::North),
    (&[KeyCode::ArrowDown, KeyCode::KeyJ, KeyCode::Numpad2], Direction::South),
    (&[KeyCode::ArrowLeft, KeyCode::KeyH, KeyCode::Numpad4], Direction::West),
    (&[KeyCode::ArrowRight, KeyCode::KeyL, KeyCode::Numpad6], Direction::East),
    (&[KeyCode::KeyY, KeyCode::Numpad7], Direction::NorthWest),
    (&[KeyCode::KeyU, KeyCode::Numpad9], Direction::NorthEast),
    (&[KeyCode::KeyB, KeyCode::Numpad1], Direction::SouthWest),
    (&[KeyCode::KeyN, KeyCode::Numpad3], Direction::SouthEast),
];
// ANCHOR_END: keys

// ANCHOR: input
/// The player, but only while it is holding the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>;

/// Keys to intents. Writing an intent is the whole of asking to act: the
/// engine claims the turn, charges it, and refuses what cannot be done.
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    player: PlayerTurn,
    mut steps: MessageWriter<Intent<Step>>,
    mut waits: MessageWriter<Intent<Wait>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move; the key is dropped.
    let Ok(entity) = player.single() else { return };
    if let Some((_, dir)) = MOVES.iter().find(|(codes, _)| keys.any_just_pressed(codes.iter().copied())) {
        steps.write(Intent::new(entity, Step(*dir)));
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        waits.write(Intent::new(entity, Wait));
    }
}
// ANCHOR_END: input

// ANCHOR: status
/// The one thing the vitals panel cannot know: how much of the map is
/// ours. A note on the view, in the game's own words.
fn note_explored(mut vitals: ResMut<VitalsView>, mut facets: ResMut<Facets>, knowledge: Res<Knowledge>) {
    vitals.facets.push(facets.facet("explored", format!("{} tiles explored", knowledge.explored_count())));
}
// ANCHOR_END: status
