//! Warren, step 3: rats, blows, and a log that says what happened.
//!
//! The guide chapter is `docs/guide/src/03-blows-and-the-log.md`.
//!
//! `cargo run -p tutorial --bin step03_blows`
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk or strike, `t` lantern,
//! `.` wait, `q` quit, Escape for the menu.

use atglyph::prelude::ui::*;
use atglyph::prelude::*;
use atglyph::rand::Rng;
use atglyph::rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use atglyph::rules::faction::FactionDef;
use bevy::prelude::*;
use std::sync::Arc;

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
        .add_plugins(narrator())
        // Escape opens the menu, and dying opens it by itself.
        .add_plugins(GameMenuPanel::new(GridRect::new(COLS / 2 - 20, 8, 40, 12)).died("The warren keeps you."))
        .insert_resource(Seed(RunSeed(7)))
        .run()
}
// ANCHOR_END: main

// ANCHOR: narrator
/// The engine narrates blows and deaths into the log. Warren changes one
/// phrase: what a rat does to you is a bite.
fn narrator() -> NarratorPlugin {
    NarratorPlugin::default().phrase(Phrase::HitsYou, "{Who} bites you for {n}.", Tones::BAD)
}
// ANCHOR_END: narrator

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
        // Combat resolves blows; minds decide the monsters' turns.
        app.add_plugins((CombatPlugin, MindsPlugin));
        app.add_systems(NewRun, start);
        // Inside the turn: a floor fills the first time it is entered.
        app.add_systems(Turn, populate.in_set(TurnSet::React));
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
/// Hands combat its registries and the rats their brain, then spawns the
/// player at the first floor's start, lantern open.
fn start(mut commands: Commands, seed: Res<Seed>, mut log: Log) {
    // What damage can be, and who hates whom: the game's content, which
    // the engine knows only by id.
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("kick")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new().hostile(you, vermin));
    commands.insert_resource(Rats {
        // Asked in order, and the first that answers wins: bite what is
        // beside you, run when badly hurt, chase what you see, else wander.
        mind: Arc::new(Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { at_pct: 30 }).then(Hunt).then(Wander { chance_pct: 40 })),
        bite: kinds.expect("bite"),
        faction: vermin,
    });
    commands.insert_resource(Registries { damage_kinds: kinds.clone(), factions: sides, ..default() });

    commands.spawn_player(
        MapId::FIRST,
        (
            (Viewshed::new(9), RevealsMap, LANTERN, Faction(you), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack::new(kinds.expect("kick"), DiceRoll::new(1, 6))),
        ),
    );
    log.notice(format!("Seed {}. You squeeze into the warren.", seed.0.0));
    log.muted("Something is scratching in the dark.");
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
    app.add_plugins(PlayerKeysPlugin::new().walk("Move", "walk or attack").wait("Act", "wait a turn"));
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

// ANCHOR: creatures
/// What every rat shares: one brain, one bite, one side.
#[derive(Resource)]
struct Rats {
    mind: Arc<Brain<Entity>>,
    bite: atglyph::rules::damage::DamageKindId,
    faction: FactionId,
}
// ANCHOR_END: creatures

// ANCHOR: populate
/// The random stream rats are placed from. Each floor draws it from its own
/// seed, so floors can be entered in any order.
const RATS: SeedDomain = SeedDomain::new(b"warren.rats");

/// Fills a floor the first time it is entered.
fn populate(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, rats: Res<Rats>, map: Res<WorldMap>, seed: Res<Seed>) {
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        let mut rng = seed.place(ev.map).rng(RATS, 0);
        let mut placed = 0;
        while placed < 16 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            // On floor, and not close enough to the start to be unfair.
            if !map.is_walkable(p) || geometry::chebyshev(p, ev.entry) < 8 {
                continue;
            }
            let mut rat = commands.spawn((
                (Actor, Blocks, Position(p), Speed(110), Faction(rats.faction)),
                (Health::full(6), Armor(0), Perception(7), DarkSight(9)),
                (MeleeAttack::new(rats.bite, DiceRoll::new(1, 3)),),
            ));
            rat.insert((Mind(rats.mind.clone()), Glyph::new('r', Color::srgb(0.72, 0.55, 0.45)).on_layer(5), Name::new("rat")));
            placed += 1;
        }
    }
}
// ANCHOR_END: populate

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph::testing::{at, headless, player};

    #[test]
    fn the_player_arrives_standing_on_floor() {
        let mut app = headless(RunSeed(1), narrator(), Warren);
        let me = player(&mut app);
        let here = at(&app, me);
        assert!(app.world().resource::<WorldMap>().is_walkable(here), "arrived at {here:?}, which is not floor");
    }
}
