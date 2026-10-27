//! Warren, step 1: a floor on the screen, and walking on it.
//!
//! The guide chapter is `docs/guide/src/01-a-map-and-walking.md`.
//!
//! `cargo run -p tutorial --bin step01_walking`
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk, `.` to wait, `q` to quit.

use atglyph::prelude::ui::*;
use atglyph::prelude::*;
use bevy::prelude::*;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;

// ANCHOR: main
fn main() -> AppExit {
    App::new()
        // The window, the glyph terminal, the turn loop, sight, the map and
        // the UI base.
        .add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS))
        // The game itself, the same plugin the tests add.
        .add_plugins(Warren)
        .insert_resource(Seed(RunSeed(7)))
        .run()
}
// ANCHOR_END: main

// ANCHOR: plugin
/// The game: its floors, its start and its keys.
struct Warren;

impl Plugin for Warren {
    fn build(&self, app: &mut App) {
        let floors = Floors::new();
        app.insert_resource(floors.appearance()).places(floors);
        app.add_systems(NewRun, start);
        // ANCHOR: gate
        app.add_systems(Update, player_input.in_set(EngineSet::Input).run_if(no_modal));
        // ANCHOR_END: gate
        declare_controls(app);
    }
}
// ANCHOR_END: plugin

// ANCHOR: tiles
/// The warren's tiles.
struct Floors {
    tiles: TileRegistry,
}

impl Floors {
    fn new() -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("earth")).unwrap();
        tiles.register(TileProps::floor("dirt")).unwrap();
        // Walkable, but you cannot see through it.
        tiles.register(TileProps::floor("roots").opaque(true)).unwrap();
        Self { tiles }
    }

    /// How each tile looks in full light: a glyph, its colour, the cell's
    /// background, and how much each cell varies from its neighbours.
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
/// How a floor is built. The engine calls this the first time something
/// enters the floor, with that floor's own seed, and keeps the result.
impl PlaceRules for Floors {
    fn build(&self, _: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
        let (wall, floor) = (self.tiles.expect("earth"), self.tiles.expect("dirt"));
        let mut ctx = BaseContext::blank(84, 42, self.tiles.clone(), wall);
        Chain::new()
            .then(dungeon::Rooms { floor, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 })
            .then(dungeon::Doors { door: self.tiles.expect("roots") })
            .then(dungeon::RandomStart)
            .run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }

    fn tiles(&self) -> TileTables {
        self.tiles.tables()
    }
}
// ANCHOR_END: rules

// ANCHOR: start
/// Spawns the player at the first floor's start.
fn start(mut commands: Commands) {
    commands.spawn_player(MapId::FIRST, (Viewshed::new(9), RevealsMap, Glyph::new('@', Color::WHITE).on_layer(10)));
}
// ANCHOR_END: start

// ANCHOR: keys
/// Warren's own keys.
#[derive(Resource, Clone, Copy)]
struct Binds {
    quit: ControlId,
}

/// Declares the keys. Walking and waiting are the engine's standard keys.
fn declare_controls(app: &mut App) {
    app.add_plugins(PlayerKeysPlugin::new().walk("Move", "walk").wait("Act", "wait a turn"));
    let binds = Binds { quit: app.add_control("Game", "quit", KeyCode::KeyQ) };
    app.insert_resource(binds);
}

// ANCHOR_END: keys

// ANCHOR: input
/// Reads Warren's own keys once a frame.
fn player_input(keys: ControlInput, binds: Res<Binds>, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(binds.quit) {
        exit.write(AppExit::Success);
    }
}
// ANCHOR_END: input

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph::testing::{at, headless, player};

    #[test]
    fn the_player_arrives_standing_on_floor() {
        let mut app = headless(RunSeed(1), (), Warren);
        let me = player(&mut app);
        let here = at(&app, me);
        assert!(app.world().resource::<WorldMap>().is_walkable(here), "arrived at {here:?}, which is not floor");
    }
}
