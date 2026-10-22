//! Warren, step 1: a floor on the screen, and walking on it.
//!
//! The guide chapter is `docs/guide/src/01-a-map-and-walking.md`. A map
//! generation chain digs the floor, the engine keeps it and draws it, and
//! the standard keys become [`Intent`]s the engine decides the cost of.
//!
//! `cargo run -p tutorial --bin step01_walking`
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk, `.` to wait, `q` to quit.

use bevy::prelude::*;
use rl_engine::prelude::*;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;

// ANCHOR: main
fn main() -> AppExit {
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map across the whole terminal, and the UI base.
    // Then the game, which is one plugin, and the seed it is played on.
    App::new().add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS)).add_plugins(Warren).insert_resource(Seed(RunSeed(7))).run()
}
// ANCHOR_END: main

/// The game: its keys, its places and its start. A plugin, so `main` and
/// the tests add the same game.
struct Warren;

impl Plugin for Warren {
    fn build(&self, app: &mut App) {
        // ANCHOR: input
        // The keys every roguelike shares, named in Warren's words for the
        // `?` screen. Each press becomes an intent; the engine claims the
        // turn, charges it, and refuses what cannot be done.
        app.add_plugins(PlayerKeysPlugin::new().walk("Move", "walk").wait("Act", "wait a turn"));
        // ANCHOR_END: input
        // ANCHOR: tiles
        // The engine's standard tiles, a wall, a floor and doors, drawn in
        // its standard look until the game gives them its own.
        app.places(Floors(TileRegistry::standard()));
        // ANCHOR_END: tiles
        app.add_systems(NewRun, start);
        // One key of Warren's own.
        let quit = app.add_control("Game", "quit", KeyCode::KeyQ);
        app.add_systems(Update, move |keys: ControlInput, mut exit: MessageWriter<AppExit>| {
            if keys.just_pressed(quit) {
                exit.write(AppExit::Success);
            }
        });
    }
}

// ANCHOR: rules
/// How a floor is built. The engine calls this once, the first time
/// something enters the map, with that floor's own seed, and keeps what
/// comes back.
struct Floors(TileRegistry);

impl PlaceRules for Floors {
    fn build(&self, _: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
        let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
        let mut ctx = BaseContext::blank(84, 42, self.0.clone(), wall);
        Chain::new().then(dungeon::Rooms { floor, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 }).then(dungeon::RandomStart).run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }

    fn tiles(&self) -> TileTables {
        self.0.tables()
    }
}
// ANCHOR_END: rules

// ANCHOR: start
/// The player, spawned into the first floor at its entry. The engine built
/// the world from the floors' tiles; the first warp in builds the floor.
fn start(mut commands: Commands) {
    commands.spawn_player(MapId::FIRST, (Viewshed::new(9), RevealsMap, Glyph::new('@', Color::WHITE).on_layer(10)));
}
// ANCHOR_END: start

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::testing::{at, headless, player};

    /// The same plugin `main` adds, without a window: the player arrives on
    /// the floor, not in its rock.
    #[test]
    fn the_player_arrives_standing_on_floor() {
        let mut app = headless(RunSeed(1), (), Warren);
        let me = player(&mut app);
        let here = at(&app, me);
        assert!(app.world().resource::<WorldMap>().is_walkable(here), "arrived at {here:?}, which is not floor");
    }
}
