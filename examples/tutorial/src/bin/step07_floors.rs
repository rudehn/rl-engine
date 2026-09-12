//! Warren, step 7: four floors down, and a rat king at the bottom.
//!
//! The guide chapter is `docs/guide/src/07-down-the-stairs.md`. A floor
//! is a *place*: a bounded map the engine builds the first time it is
//! entered and keeps whole after that. Stairs are entities standing on a
//! cell with a [`Transition`] on them.
//!
//! `cargo run -p tutorial --bin step07_floors`
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

// ANCHOR: floors
/// How deep the warren goes.
const FLOORS: u32 = 4;

/// Map zero is the streamed surface, which the warren has none of, so its
/// floors are maps one upward.
fn map_of(floor: u32) -> MapId {
    MapId(floor)
}

/// The floor a map id is.
fn floor_of(map: MapId) -> u32 {
    map.0
}

/// What a floor is called.
fn name_of(floor: u32) -> &'static str {
    match floor {
        1 => "the Burrow",
        2 => "the Middens",
        3 => "the Bone Nest",
        _ => "the King's Chamber",
    }
}
// ANCHOR_END: floors

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
    .add_plugins((CorePlugin, FovPlugin, CombatPlugin, ItemsPlugin))
    // The drawing. `CapturePlugin` is only how this guide's screenshots
    // are taken; delete it and nothing changes.
    .add_plugins((MapViewPlugin, ChromePlugin, CapturePlugin))
    .insert_resource(Seed(RunSeed(7)))
    // The map gets everything but the status row and the log.
    .insert_resource(MapView::new(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
    .insert_resource(ChromeLayout { log_rows: Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS), status_row: 0 })
    .add_systems(Startup, start)
    // Once a frame, before the turns: whatever the player pressed becomes
    // at most one intent, however many passes the turn loop then runs.
    .add_systems(Update, player_input.in_set(EngineSet::Input))
    // Both inside the turn: a floor fills the first time it is entered,
    // and a crust eaten heals before the next rat gets its bite in.
    .add_systems(Turn, (populate, eat).in_set(TurnSet::React))
    .add_systems(Update, (narrate, update_status).chain().in_set(PresentSet::Narrate));
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
    fn build(&self, map: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let depth = floor_of(map);
        let (wall, open, roots) = (self.tiles.expect("earth"), self.tiles.expect("dirt"), self.tiles.expect("roots"));
        let mut ctx = BaseContext::blank(84, 42, self.tiles.clone(), wall);
        // A stream per floor, so floor three is the same whether or not
        // you dawdled on floor one.
        let seed = RunSeed(self.seed.0 ^ (depth as u64) << 32);
        let chain = match depth {
            // Dug rooms near the surface.
            1 | 2 => {
                Chain::new().then(dungeon::Rooms { floor: open, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 }).then(dungeon::Doors { door: roots })
            }
            // Gnawed-out caves below them.
            3 => Chain::new().then(passes::CellularCave { wall, floor: open, fill_pct: 45, ..Default::default() }).then(passes::KeepLargestRegion { wall }),
            // One wide chamber for the king.
            _ => Chain::new().then(dungeon::Rooms { floor: open, attempts: 60, min_size: 12, max_size: 18, min_rooms: 2 }),
        };
        // Every floor gets a start and a farthest point from it: the way
        // down on the upper floors, and where the king waits on the last.
        chain.then(dungeon::RandomStart).then(dungeon::FarthestExit).run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }
}
// ANCHOR_END: rules

/// The rat king. Killing it wins the run.
#[derive(Component, Clone, Copy)]
struct King;

// ANCHOR: crust
/// A crust of bread: the one item the warren has, and how much it heals.
#[derive(Component, Clone, Copy)]
struct Crust(i32);
// ANCHOR_END: crust

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
            (Inventory::default(),),
        ))
        .id();
    warps.write(WarpRequest::into_place(player, map_of(1)));
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), LogCategory::Notice, 0);
    log.push("Something is scratching in the dark. g picks up, e eats, > goes down.", LogCategory::Muted, 0);
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

// ANCHOR: intents
/// Everything the player's keys can ask for. A system may take seven
/// parameters; bundling the writers into one `SystemParam` keeps room for
/// as many actions as the game grows.
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerIntents<'w> {
    steps: MessageWriter<'w, Intent<Step>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    pick_ups: MessageWriter<'w, Intent<PickUp>>,
    uses: MessageWriter<'w, Intent<UseItem>>,
    stairs: MessageWriter<'w, Intent<GoThrough>>,
}
// ANCHOR_END: intents

// ANCHOR: input
/// The player, but only while it is holding the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position, &'static Inventory), (With<Player>, With<MyTurn>)>;

/// Keys to intents. Writing an intent is the whole of asking to act: the
/// engine claims the turn, charges it, and refuses what cannot be done.
///
/// Bump to attack is a decision the game makes, not the engine: a step
/// into an occupied cell is written as an [`Attack`] instead.
fn player_input(keys: Res<ButtonInput<KeyCode>>, occupancy: Res<Occupancy>, player: PlayerTurn, mut intents: PlayerIntents, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move; the key is dropped.
    let Ok((entity, pos, bag)) = player.single() else { return };
    if let Some((_, dir)) = MOVES.iter().find(|(codes, _)| keys.any_just_pressed(codes.iter().copied())) {
        match occupancy.first_at(pos.0 + dir.offset()) {
            Some(other) => {
                intents.attacks.write(Intent::new(entity, Attack(other)));
            }
            None => {
                intents.steps.write(Intent::new(entity, Step(*dir)));
            }
        }
    } else if keys.just_pressed(KeyCode::Enter)
        || (keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) && keys.any_just_pressed([KeyCode::Period, KeyCode::Comma]))
    {
        intents.stairs.write(Intent::new(entity, GoThrough));
    } else if keys.just_pressed(KeyCode::KeyG) {
        intents.pick_ups.write(Intent::new(entity, PickUp));
    } else if keys.just_pressed(KeyCode::KeyE) {
        // Eating is a use; the engine spends the turn and reports it back.
        if let Some(crust) = bag.items.first().copied() {
            intents.uses.write(Intent::new(entity, UseItem(crust)));
        }
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        intents.waits.write(Intent::new(entity, Wait));
    }
}
// ANCHOR_END: input

// ANCHOR: status
/// One line at the top of the screen, rewritten every frame.
fn update_status(
    mut status: ResMut<StatusLine>,
    turns: Res<Turns>,
    map: Res<WorldMap>,
    state: Res<State<EngineState>>,
    player: Query<(&Health, &Inventory), With<Player>>,
) {
    if *state.get() != EngineState::Playing {
        return;
    }
    let Ok((hp, bag)) = player.single() else { return };
    let depth = floor_of(map.current());
    status.0 =
        format!("HP {}/{}   Crusts {}   Turn {}   Floor {depth}/{FLOORS}: {}   [q]uit", hp.hp, hp.max, bag.items.len(), turns.turn_number(), name_of(depth));
}
// ANCHOR_END: status

// ANCHOR: populate
/// Fills the floor the one time it is built. `PlaceEntered::first` is
/// true only on that arrival, so coming back does not restock it.
fn populate(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    rats: Res<Rats>,
    map: Res<WorldMap>,
    seed: Res<Seed>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
) {
    for ev in entered.read() {
        let depth = floor_of(ev.map);
        log.push(format!("Floor {depth}: {}.", name_of(depth)), LogCategory::Notice, turns.turn_number());
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        // A stream of its own, keyed by name: adding another spawner later
        // cannot shift the numbers this one draws.
        let mut rng = seed.0.rng(SeedDomain::new(b"warren.rats"), ev.map.0 as u64);

        // The stairs. A transition is an entity on a cell, nothing more.
        let stone = Color::srgb(0.86, 0.86, 0.70);
        if depth > 1 {
            commands.spawn((
                Position(ev.entry),
                Transition { to: Destination::Place { map: map_of(depth - 1), arrive: Arrive::Exit } },
                Glyph::new('<', stone).on_layer(1),
            ));
        }
        match (depth < FLOORS, ev.exit) {
            (true, Some(down)) => {
                commands.spawn((
                    Position(down),
                    Transition { to: Destination::Place { map: map_of(depth + 1), arrive: Arrive::Entry } },
                    Glyph::new('>', stone).on_layer(1),
                ));
            }
            // The bottom floor: the king stands where the stairs would be.
            (false, Some(throne)) => {
                commands.spawn((
                    (Actor, Blocks, King, Position(throne), Speed(100), Faction(rats.faction)),
                    (Health::full(40), Armor(2), Perception(12), Mind(rats.mind.clone())),
                    (MeleeAttack { kind: rats.bite, dice: DiceRoll::new(2, 4) }, Glyph::new('R', Color::srgb(0.95, 0.78, 0.35)).on_layer(6)),
                ));
            }
            _ => {}
        }
        // Crusts, dropped by whatever came down here before you.
        let mut crusts = 0;
        while crusts < 6 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            if !map.is_walkable(p) {
                continue;
            }
            commands.spawn((Item, Crust(8), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
            crusts += 1;
        }
        let mut placed = 0;
        while placed < 8 + depth as usize * 3 {
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
/// What narration reads and writes.
#[derive(bevy::ecs::system::SystemParam)]
struct Voice<'w, 's> {
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
    status: ResMut<'w, StatusLine>,
    next: ResMut<'w, NextState<EngineState>>,
    players: Query<'w, 's, (), With<Player>>,
    kings: Query<'w, 's, (), With<King>>,
}

/// Turns what the pipeline reported into English.
fn narrate(mut items: MessageReader<ItemEvent>, mut dealt: MessageReader<DamageDealt>, mut deaths: MessageReader<DeathEvent>, mut voice: Voice) {
    let Voice { turns, log, status, next, players, kings } = &mut voice;
    let turn = turns.turn_number();
    let name = |e: Entity| if players.contains(e) { "you" } else { "the rat" };
    for ev in items.read() {
        match *ev {
            ItemEvent::PickedUp { .. } => log.push("You pocket a crust of bread.", LogCategory::Info, turn),
            ItemEvent::Used { .. } => log.push("You eat the crust. It helps.", LogCategory::Good, turn),
            _ => {}
        }
    }
    for d in dealt.read() {
        let attacker = d.hit.attacker.map(name).unwrap_or("something");
        let mine = attacker == "you";
        let (verb, category) = if mine { ("hit", LogCategory::Info) } else { ("bites", LogCategory::Bad) };
        let tail = if d.dealt <= 0 { " and does nothing.".to_string() } else { format!(" for {}.", d.dealt) };
        log.push(format!("{} {verb} {}{tail}", capital(attacker), name(d.target)), category, turn);
    }
    for d in deaths.read() {
        if d.was_player {
            log.push("The warren keeps you. Press q to quit.", LogCategory::Bad, turn);
            status.0 = format!("Eaten on turn {turn}.   [q]uit");
            // Leaving `Playing` stops the loop: no turns, no input, but
            // the last frame stays on the screen.
            next.set(EngineState::Idle);
        } else if kings.contains(d.entity) {
            log.push("The rat king falls. The scratching stops. Press q to quit.", LogCategory::Notice, turn);
            status.0 = format!("Won on turn {turn}.   [q]uit");
            next.set(EngineState::Idle);
        } else {
            log.push("The rat dies.", LogCategory::Good, turn);
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

// ANCHOR: eat
/// What eating a crust means. The engine has already spent the turn and
/// taken the item out of the bag; this is the part only the game knows.
///
/// It runs in [`TurnSet::React`], inside the turn, so the healing lands
/// before the next rat is dealt its move. In the drawing phase it would
/// land a blow too late.
fn eat(mut commands: Commands, mut used: MessageReader<ItemEvent>, crusts: Query<&Crust>, mut eaters: Query<&mut Health>) {
    for ev in used.read() {
        let ItemEvent::Used { actor, item } = *ev else { continue };
        let (Ok(crust), Ok(mut health)) = (crusts.get(item), eaters.get_mut(actor)) else { continue };
        health.hp = (health.hp + crust.0).min(health.max);
        commands.entity(item).despawn();
    }
}
// ANCHOR_END: eat
