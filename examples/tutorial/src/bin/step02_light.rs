//! Warren, step 2: what you can see, and a lantern to see it by.
//!
//! The guide chapter is `docs/guide/src/02-sight-and-light.md`.
//!
//! `cargo run -p tutorial --bin step02_light`
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk, `t` lantern, `.` wait,
//! `q` quit.

use atglyph::prelude::ui::*;
use atglyph::prelude::*;
use bevy::prelude::*;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows at the bottom of the terminal given over to the message log.
const LOG_ROWS: i32 = 5;

// ANCHOR: main
fn main() -> AppExit {
    App::new()
        // The map now leaves the top row for the vitals and the bottom rows
        // for the log.
        .add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(GridRect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        .add_plugins(Warren)
        .add_plugins((
            VitalsPanel::new(GridRect::new(0, 0, COLS - 13, 1)),
            LogPanel::new(GridRect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)),
            // `?` lists every key the game declared; the top-right corner
            // says so.
            ControlsPanel::new(GridRect::new(4, 3, COLS - 8, ROWS - LOG_ROWS - 6)).hint(GridRect::new(COLS - 13, 0, 12, 1)),
        ))
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
        // Without lighting the world is lit everywhere. With it, you see
        // only what a light reaches.
        app.add_plugins(LightingPlugin).insert_resource(Lighting::dark());
        app.add_systems(NewRun, start);
        // ANCHOR: gate
        app.add_systems(Update, (player_input, tend_lantern).in_set(EngineSet::Input).run_if(no_modal));
        // ANCHOR_END: gate
        // The vitals view is data the panel in `main` draws; the game writes
        // a line onto it, so the game adds it.
        app.add_plugins(VitalsViewPlugin).add_systems(Update, note_explored.in_set(ViewSet::Annotate));
        declare_controls(app);
    }
}
// ANCHOR_END: plugin

// ANCHOR: tiles
/// The warren's tiles: what each one is, and how it looks in full light.
struct Floors {
    tiles: TileRegistry,
}

impl Floors {
    fn new() -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("earth").look(TileLook::new('#', (0.78, 0.66, 0.50)).on((0.34, 0.27, 0.21)).vary(0.20, 0.05))).unwrap();
        tiles.register(TileProps::floor("dirt").look(TileLook::new('.', (0.66, 0.58, 0.45)).on((0.18, 0.15, 0.12)).vary(0.28, 0.06))).unwrap();
        // Walkable, but you cannot see through it.
        let roots = TileLook::new('+', (0.55, 0.74, 0.45)).on((0.16, 0.22, 0.13)).vary(0.18, 0.05);
        tiles.register(TileProps::floor("roots").opaque(true).look(roots)).unwrap();
        Self { tiles }
    }

    /// The renderer's colour table, from the looks the tiles carry.
    fn appearance(&self) -> TileAppearance {
        TileAppearance::from_tiles(&self.tiles).expect("every tile has a look")
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
/// Spawns the player at the first floor's start, lantern open.
fn start(mut commands: Commands, seed: Res<Seed>, mut log: Log) {
    commands.spawn_player(MapId::FIRST, (Viewshed::new(9), RevealsMap, LANTERN, Glyph::new('@', Color::WHITE).on_layer(10)));
    log.notice(format!("Seed {}. You squeeze into the warren.", seed.0.0));
}
// ANCHOR_END: start

// ANCHOR: keys
/// Warren's own keys.
#[derive(Resource, Clone, Copy)]
struct Binds {
    lantern: ControlId,
    quit: ControlId,
}

/// Declares the keys. Walking and waiting are the engine's standard keys.
fn declare_controls(app: &mut App) {
    app.add_plugins(PlayerKeysPlugin::new().walk("Move", "walk").wait("Act", "wait a turn"));
    let binds = Binds { lantern: app.add_control("Act", "lantern on or off", KeyCode::KeyT), quit: app.add_control("Game", "quit", KeyCode::KeyQ) };
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

// ANCHOR: lantern
/// The lantern's light: warm, and a little restless.
const LANTERN: LightSource = LightSource::new(150, 7, Rgb::new(255, 210, 140)).flickering(30);

/// The player, while it holds the turn, and whether its lantern is open.
type Lantern<'w, 's> = Query<'w, 's, (Entity, Has<LightSource>), (With<Player>, With<MyTurn>)>;

/// Opens or shades the lantern, which spends the turn. The light is a
/// component, so shading it is removing it.
fn tend_lantern(keys: ControlInput, binds: Res<Binds>, mut commands: Commands, player: Lantern, mut waits: MessageWriter<Intent<Wait>>, mut log: Log) {
    if !keys.just_pressed(binds.lantern) {
        return;
    }
    let Ok((entity, lit)) = player.single() else { return };
    if lit {
        commands.entity(entity).remove::<LightSource>();
        log.muted("You shade the lantern.");
    } else {
        commands.entity(entity).insert(LANTERN);
        log.notice("You open the lantern.");
    }
    waits.write(Intent::new(entity, Wait));
}
// ANCHOR_END: lantern

// ANCHOR: status
/// Adds a line to the vitals panel: how much of the map you have seen.
fn note_explored(mut vitals: ResMut<VitalsView>, knowledge: Res<Knowledge>) {
    vitals.facets.push(Facet::new("explored", format!("explored {}", knowledge.explored_count())));
}
// ANCHOR_END: status

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
