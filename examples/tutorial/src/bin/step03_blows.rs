//! Warren, step 3: blows, and the log that tells you about them.
//!
//! The guide chapter is `docs/guide/src/03-blows-and-the-log.md`. Walking
//! into a rat strikes it, the engine puts every blow down one damage
//! pipeline, and the narrator says what happened in the log.
//!
//! `cargo run -p tutorial --bin step03_blows`
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk or strike, `t` for the
//! lantern, `.` to wait, `q` to quit.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use rl_engine::rl_rules::faction::FactionDef;
use std::sync::Arc;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows at the bottom of the terminal given over to the message log.
const LOG_ROWS: i32 = 5;

/// The warren's one floor is map one.
const WARREN: MapId = MapId(1);

// ANCHOR: main
fn main() -> AppExit {
    let mut app = App::new();
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map in everything but the status row and the log, and the UI base.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        // Without this the world is lit everywhere and sight is geometry
        // alone. With it, `visible` shrinks to what a light reaches.
        .add_plugins((CombatPlugin, MindsPlugin, LightingPlugin))
        .insert_resource(Lighting::dark())
        .insert_resource(Seed(RunSeed(7)))
        // Two panels: the vitals strip on the top row, the log along the
        // bottom. Each draws itself; neither needs a system of yours.
        .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)).hints("[t]orch  [.]wait  [q]uit"))
        .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
        // The engine narrates blows, deaths and pickups into the log, naming
        // things in their own colours. Warren changes one phrase: what a rat
        // does to you is a bite.
        .add_plugins(NarratorPlugin::default().phrase(Phrase::HitsYou, "{Who} bites you for {n}.", Tones::BAD))
        // Escape opens the menu, and the run's end opens it by itself.
        .add_plugins(GameMenuPanel::new(Rect::new(COLS / 2 - 20, 8, 40, 12)).died("The warren keeps you."))
        .add_systems(NewRun, start)
        // A floor fills the first time it is entered, inside the turn.
        .add_systems(Turn, populate.in_set(TurnSet::React))
        // Once a frame, before the turns: the lantern is a key of Warren's
        // own, read beside the standard ones.
        .add_systems(Update, tend_lantern.in_set(EngineSet::Input))
        .add_systems(Update, note_explored.in_set(ViewSet::Annotate));
    // The floors, handed over once: the engine builds each the first time
    // something enters it, from that floor's own seed.
    let warren = Warren::new();
    app.insert_resource(warren.appearance()).places(warren);
    declare_controls(&mut app);
    app.run()
}
// ANCHOR_END: main

// ANCHOR: tiles
/// The warren's tiles, and how each one looks in full light.
struct Warren {
    tiles: TileRegistry,
}

impl Warren {
    fn new() -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("earth")).unwrap();
        tiles.register(TileProps::floor("dirt")).unwrap();
        // Walkable and opaque: you can step through a curtain of roots,
        // but you cannot see past one until you do.
        tiles.register(TileProps::floor("roots").opaque(true)).unwrap();
        Self { tiles }
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
/// Hands the engine what combat reads, then spawns the player into the
/// warren, at the first floor's entry.
fn start(mut commands: Commands, seed: Res<Seed>, mut log: ResMut<MessageLog>) {
    // ANCHOR: combat
    // The two registries combat reads: what damage can be, and who hates
    // whom. Both are the game's content, named nowhere in the engine.
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("kick")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new(&sides).hostile(you, vermin));
    commands.insert_resource(Registries { damage_kinds: kinds.clone(), factions: sides, ..default() });
    // ANCHOR_END: combat
    commands.insert_resource(Rats {
        // Asked in order, first that answers wins: bite what is next to
        // you, run when badly hurt, chase what you can see, else mill about.
        mind: Arc::new(Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { at_pct: 30 }).then(Hunt).then(Wander { chance_pct: 40 })),
        bite: kinds.expect("bite"),
        faction: vermin,
    });

    commands.spawn_player(
        WARREN,
        (
            (Viewshed::new(9), RevealsMap, LANTERN, Faction(you), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack::new(kinds.expect("kick"), DiceRoll::new(1, 6))),
        ),
    );
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), Tones::NOTICE, 0);
    log.push("Something is scratching in the dark.", Tones::MUTED, 0);
}
// ANCHOR_END: start

// ANCHOR: input
/// The keys. Walking and waiting are the same in every roguelike, so they
/// are the engine's `PlayerKeysPlugin`: each press becomes an intent, and
/// the engine claims the turn, charges it, and refuses what cannot be done.
/// Quitting is Warren's own.
///
/// A walk key writes a [`Bump`], which the engine resolves to a step, a
/// blow at a foe, or opening a door, whichever is in the way.
fn declare_controls(app: &mut App) {
    app.add_plugins(PlayerKeysPlugin::new().walk("Move", "walk, or strike whoever is there").wait("Act", "wait a turn"));
    let quit = app.add_control("Game", "quit", KeyCode::KeyQ);
    app.add_systems(Update, move |keys: ControlInput, mut exit: MessageWriter<AppExit>| {
        if keys.just_pressed(quit) {
            exit.write(AppExit::Success);
        }
    });
}
// ANCHOR_END: input

// ANCHOR: status
/// The one thing the vitals panel cannot know: how much of the map is
/// ours. A note on the view, in the game's own words.
fn note_explored(mut vitals: ResMut<VitalsView>, mut facets: ResMut<Facets>, knowledge: Res<Knowledge>) {
    vitals.facets.push(facets.facet("explored", format!("{} tiles explored", knowledge.explored_count())));
}
// ANCHOR_END: status

// ANCHOR: lantern
/// What the lantern sheds when it is open: a warm, slightly restless pool.
const LANTERN: LightSource = LightSource::new(150, 7, Rgb::new(255, 210, 140)).flickering(30);

/// The player and whether its lantern is open, while it holds the turn.
type Lantern<'w, 's> = Query<'w, 's, (Entity, Has<LightSource>), (With<Player>, With<MyTurn>)>;

/// `t` opens the lantern or shades it, and spends the turn either way.
///
/// The light is a component on the player, so shading it is removing one.
/// Nothing else changes: sight is still sight, and the explored map still
/// remembers what the light once reached.
fn tend_lantern(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player: Lantern,
    mut waits: MessageWriter<Intent<Wait>>,
    mut log: ResMut<MessageLog>,
    turns: Res<Turns>,
) {
    if !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    let Ok((entity, lit)) = player.single() else { return };
    if lit {
        commands.entity(entity).remove::<LightSource>();
        log.muted("You shade the lantern. The warren closes to arm's length.", turns.turn_number());
    } else {
        commands.entity(entity).insert(LANTERN);
        log.notice("You open the lantern. The dirt comes up warm around you.", turns.turn_number());
    }
    waits.write(Intent::new(entity, Wait));
}
// ANCHOR_END: lantern

// ANCHOR: creatures
/// What every rat in the warren shares: one brain, one faction, one bite.
#[derive(Resource)]
struct Rats {
    mind: Arc<Brain<Entity>>,
    bite: rl_engine::rl_rules::damage::DamageKindId,
    faction: FactionId,
}
// ANCHOR_END: creatures

// ANCHOR: populate
/// Fills the floor the one time it is built. `PlaceEntered::first` is
/// true only on that arrival, so coming back does not restock it.
fn populate(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, rats: Res<Rats>, map: Res<WorldMap>, seed: Res<Seed>) {
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        // A stream of its own, keyed by name: adding another spawner later
        // cannot shift the numbers this one draws.
        let mut rng = seed.rng(rl_engine::rl_core::SeedDomain::new(b"warren.rats"), ev.map.0 as u64);
        let mut placed = 0;
        while placed < 16 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            // Not on top of the player, and not close enough to be unfair.
            if !map.is_walkable(p) || geometry::chebyshev(p, ev.entry) < 8 {
                continue;
            }
            commands.spawn((
                (Actor, Blocks, Position(p), Speed(110), Faction(rats.faction)),
                (Health::full(6), Armor(0), Perception(7), DarkSight(9), Mind(rats.mind.clone()), Glyph::new('r', Color::srgb(0.72, 0.55, 0.45)).on_layer(5)),
                (MeleeAttack::new(rats.bite, DiceRoll::new(1, 3)), Name::new("rat")),
            ));
            placed += 1;
        }
    }
}
// ANCHOR_END: populate
