//! Warren, step 5: bumping into a rat is a blow, and blows kill.
//!
//! The guide chapter is `docs/guide/src/05-blows.md`. Walking into an
//! occupied cell becomes an [`Attack`]; damage runs a pipeline of stages
//! the game composes; the log says what happened.
//!
//! `cargo run -p tutorial --bin step05_combat`
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk, `.` to wait, `q` to quit.

use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_render::capture;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use rl_engine::rl_rules::damage::SubtractArmor;
use rl_engine::rl_rules::faction::FactionDef;

/// The terminal, in cells and in pixels per cell.
const COLS: i32 = 80;
const ROWS: i32 = 40;
const CELL: Vec2 = Vec2::new(10.0, 16.0);
/// Rows at the bottom of the terminal given over to the message log.
const LOG_ROWS: i32 = 5;

/// Map zero is the streamed surface. The warren has no surface, so its
/// one floor is map one.
const WARREN: MapId = MapId(1);

// ANCHOR: main
fn main() -> AppExit {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(capture::prepare(Window {
                    title: "Warren".into(),
                    resolution: WindowResolution::new((COLS as f32 * CELL.x) as u32, (ROWS as f32 * CELL.y) as u32),
                    ..default()
                })),
                ..default()
            })
            .set(ImagePlugin::default_nearest()),
    )
    .add_plugins(TerminalPlugin { width: COLS, height: ROWS, cell_size: CELL, font_size: 14.0 })
    // The engine: the turn loop and the map, then sight.
    // Minds live in the combat plugin: deciding where to move and
    // deciding whom to hit are the same decision.
    .add_plugins((CorePlugin, FovPlugin, CombatPlugin))
    // The drawing. `CapturePlugin` is only how this guide's screenshots
    // are taken; delete it and nothing changes.
    .add_plugins((MapViewPlugin, UiPlugin, CapturePlugin))
    .insert_resource(Seed(RunSeed(7)))
    // The map gets everything but the status row and the log.
    .insert_resource(MapView::new(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
    // Two panels: the vitals strip on the top row, the log along the
    // bottom. Each draws itself; neither needs a system of yours.
    .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)).hints("[.] wait  [q]uit"))
    .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
    .add_systems(Startup, start)
    // Once a frame, before the turns: whatever the player pressed becomes
    // at most one intent, however many passes the turn loop then runs.
    .add_systems(Update, player_input.in_set(EngineSet::Input))
    // A floor fills the first time it is entered, inside the turn.
    .add_systems(Turn, populate.in_set(TurnSet::React))
    .add_systems(Update, narrate.in_set(PresentSet::Narrate));
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

// ANCHOR: creatures
/// What every rat in the warren shares: one brain, one faction, one bite.
#[derive(Resource)]
struct Rats {
    mind: Arc<Brain<Entity>>,
    bite: rl_engine::rl_rules::damage::DamageKindId,
    faction: FactionId,
}
// ANCHOR_END: creatures

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

    // The two registries combat reads: what damage can be, and who hates
    // whom. Both are the game's content, named nowhere in the engine.
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("kick")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef { name: "you".into() }, FactionDef { name: "vermin".into() }]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    let mut factions = Factions::new(&sides);
    factions.set_mutual(you, vermin, Relation::Hostile);
    commands.insert_resource(CombatRules { kinds: kinds.clone(), factions });
    commands.insert_resource(CombatRng::for_run(seed.0));
    // What a hit passes through on its way to the target. One stage here;
    // resistances, a shield, a critical rule would each be another.
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    commands.insert_resource(Rats {
        // Asked in order, first that answers wins: bite what is next to
        // you, run when badly hurt, chase what you can see, else mill about.
        mind: Arc::new(Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { at_pct: 30 }).then(Hunt).then(Wander { chance_pct: 40 })),
        bite: kinds.expect("bite"),
        faction: vermin,
    });

    commands.insert_resource(warren.appearance());
    commands.insert_resource(WorldMap::new(warren.tiles.tables()));
    commands.insert_resource(PlaceRulesRes(Box::new(warren)));

    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO), Speed(100)),
            (Viewshed::new(9), RevealsMap, Faction(you), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack { kind: kinds.expect("kick"), dice: DiceRoll::new(1, 6) }),
        ))
        .id();
    warps.write(WarpRequest::into_place(player, WARREN));
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), Tones::NOTICE, 0);
    log.push("Something is scratching in the dark.", Tones::MUTED, 0);
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
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position), (With<Player>, With<MyTurn>)>;

/// Keys to intents. Writing an intent is the whole of asking to act: the
/// engine claims the turn, charges it, and refuses what cannot be done.
///
/// Bump to attack is a decision the game makes, not the engine: a step
/// into an occupied cell is written as an [`Attack`] instead.
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    occupancy: Res<Occupancy>,
    player: PlayerTurn,
    mut steps: MessageWriter<Intent<Step>>,
    mut attacks: MessageWriter<Intent<Attack>>,
    mut waits: MessageWriter<Intent<Wait>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move; the key is dropped.
    let Ok((entity, pos)) = player.single() else { return };
    if let Some((_, dir)) = MOVES.iter().find(|(codes, _)| keys.any_just_pressed(codes.iter().copied())) {
        match occupancy.first_at(pos.0 + dir.offset()) {
            Some(other) => {
                attacks.write(Intent::new(entity, Attack(other)));
            }
            None => {
                steps.write(Intent::new(entity, Step(*dir)));
            }
        }
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        waits.write(Intent::new(entity, Wait));
    }
}
// ANCHOR_END: input

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
        let mut rng = seed.0.rng(SeedDomain::new(b"warren.rats"), ev.map.0 as u64);
        let mut placed = 0;
        while placed < 16 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            // Not on top of the player, and not close enough to be unfair.
            if !map.is_walkable(p) || geometry::chebyshev(p, ev.entry) < 8 {
                continue;
            }
            commands.spawn((
                (Actor, Blocks, Position(p), Speed(110), Faction(rats.faction)),
                (Health::full(6), Armor(0), Perception(7), Mind(rats.mind.clone()), Glyph::new('r', Color::srgb(0.72, 0.55, 0.45)).on_layer(5)),
                (MeleeAttack { kind: rats.bite, dice: DiceRoll::new(1, 3) },),
            ));
            placed += 1;
        }
    }
}
// ANCHOR_END: populate

// ANCHOR: narrate
/// Turns what the pipeline reported into English.
fn narrate(
    mut dealt: MessageReader<DamageDealt>,
    mut deaths: MessageReader<DeathEvent>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
    mut next: ResMut<NextState<EngineState>>,
    players: Query<(), With<Player>>,
) {
    let turn = turns.turn_number();
    let name = |e: Entity| if players.contains(e) { "you" } else { "the rat" };
    for d in dealt.read() {
        let attacker = d.hit.attacker.map(name).unwrap_or("something");
        let mine = attacker == "you";
        let (verb, category) = if mine { ("hit", Tones::TEXT) } else { ("bites", Tones::BAD) };
        let tail = if d.dealt <= 0 { " and does nothing.".to_string() } else { format!(" for {}.", d.dealt) };
        log.push(format!("{} {verb} {}{tail}", capital(attacker), name(d.target)), category, turn);
    }
    for d in deaths.read() {
        if d.was_player {
            log.push("The warren keeps you. Press q to quit.", Tones::BAD, turn);
            // Leaving `Playing` stops the loop: no turns, no input, but
            // the last frame stays on the screen.
            next.set(EngineState::Idle);
        } else {
            log.push("The rat dies.", Tones::GOOD, turn);
        }
    }
}

/// The same string with its first letter raised.
fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
// ANCHOR_END: narrate
