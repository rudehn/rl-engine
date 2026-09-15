//! Warren, step 8: the bestiary moves out of Rust and into a file.
//!
//! The guide chapter is `docs/guide/src/08-content-in-files.md`. A
//! [`Registry`] validates the file at load and hands out a dense
//! [`Id`] per entry; a [`BandedTable`] says which of them belong at which
//! depth, and in what numbers.
//!
//! `cargo run -p tutorial --bin step08_content`
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk, `.` to wait, `q` to quit.

use std::sync::Arc;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use rl_engine::rl_rules::damage::SubtractArmor;
use rl_engine::rl_rules::faction::FactionDef;
use serde::Deserialize;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows at the bottom of the terminal given over to the message log.
const LOG_ROWS: i32 = 5;
/// The bestiary, compiled in so the binary runs from anywhere.
const RATS_RON: &str = include_str!("../../assets/rats.ron");

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
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map in everything but the status row and the log, and the UI base.
    // `CapturePlugin` inside it only takes this guide's screenshots.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        // Minds live in the combat plugin: deciding where to move and
        // deciding whom to hit are the same decision.
        .add_plugins((CombatPlugin, MindsPlugin, ItemsPlugin))
        .insert_resource(Seed(RunSeed(7)))
        // Two panels: the vitals strip on the top row, the log along the
        // bottom. Each draws itself; neither needs a system of yours.
        .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)).hints("[g]et [e]at [>]down [q]uit"))
        .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
        .add_systems(Update, note_bag_and_floor.in_set(ViewSet::Annotate))
        .add_systems(Startup, start)
        // Once a frame, before the turns: whatever the player pressed becomes
        // at most one intent, however many passes the turn loop then runs.
        .add_systems(Update, player_input.in_set(EngineSet::Input))
        // Both inside the turn: a floor fills the first time it is entered,
        // and a crust eaten heals before the next rat gets its bite in.
        .add_systems(Turn, (populate, eat).in_set(TurnSet::React))
        .add_systems(Update, narrate.in_set(PresentSet::Narrate));
    app.run()
}
// ANCHOR_END: main

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

// ANCHOR: crust
/// A crust of bread: the one item the warren has, and how much it heals.
#[derive(Component, Clone, Copy)]
struct Crust(i32);
// ANCHOR_END: crust

// ANCHOR: def
/// One kind of vermin, exactly as `assets/rats.ron` writes it. Serde
/// parses the file; [`Named`] is how the registry knows what to key it by,
/// and a [`NameRef`] is a name in the file that the load turns into an id.
#[derive(Debug, Clone, Deserialize)]
struct RatDef {
    name: String,
    glyph: char,
    color: (f32, f32, f32),
    hp: i32,
    armor: i32,
    attack: DiceRoll,
    kind: NameRef<DamageKind>,
    perception: i32,
    speed: u32,
    flee_at: i32,
    spawn: (i32, i32, u32, u32, u32),
}

impl Named for RatDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// What an entity on the map was spawned from.
#[derive(Component, Clone, Copy)]
struct Kind(Id<RatDef>);
// ANCHOR_END: def

// ANCHOR: bestiary
/// The bestiary: the defs, one brain per def, and the table that says
/// what belongs at what depth.
#[derive(Resource)]
struct Bestiary {
    defs: Registry<RatDef>,
    table: BandedTable<Id<RatDef>>,
    minds: Vec<Arc<Brain<Entity>>>,
    faction: FactionId,
}

impl Bestiary {
    /// Reads the file against `names`, builds a brain for each entry from its own fields,
    /// and bands the ones with a weight into the spawn table.
    fn load(names: &Names, faction: FactionId) -> Self {
        let defs: Registry<RatDef> = names.load(RATS_RON).unwrap_or_else(|e| panic!("assets/rats.ron: {e}"));
        let mut table = BandedTable::default();
        let mut minds = Vec::new();
        for (id, def) in defs.iter() {
            let (first, last, weight, group_min, group_max) = def.spawn;
            if weight > 0 {
                table.push(BandedEntry::new(id).bands(first, last).weight(weight).group(group_min, group_max));
            }
            let mut brain = Brain::new().then(MeleeAdjacent);
            if def.flee_at > 0 {
                brain = brain.then(FleeWhenHurt { at_pct: def.flee_at });
            }
            minds.push(Arc::new(brain.then(Hunt).then(Wander { chance_pct: 40 })));
        }
        Self { defs, table, minds, faction }
    }

    /// Spawns one of `id` at `at`.
    fn spawn(&self, commands: &mut Commands, id: Id<RatDef>, at: Point) -> Entity {
        let def = self.defs.get(id);
        commands
            .spawn((
                (Actor, Blocks, Kind(id), Position(at), Speed(def.speed), Faction(self.faction)),
                (Health::full(def.hp), Armor(def.armor), Perception(def.perception), Mind(self.minds[id.index()].clone())),
                (MeleeAttack { kind: def.kind.id(), dice: def.attack }, Glyph::new(def.glyph, Color::srgb(def.color.0, def.color.1, def.color.2)).on_layer(5)),
            ))
            .id()
    }
}
// ANCHOR_END: bestiary

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
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("venom"), DamageKind::new("kick")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new(&sides).hostile(you, vermin));
    let registries = Registries { damage_kinds: kinds.clone(), factions: sides, ..default() };
    // What a hit passes through on its way to the target. One stage here;
    // resistances, a shield, a critical rule would each be another.
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    // The bestiary names the damage each creature deals, so it loads against
    // the registries before they are handed to the engine.
    commands.insert_resource(Bestiary::load(&registries.names(), vermin));
    commands.insert_resource(registries);

    commands.insert_resource(warren.appearance());
    commands.insert_resource(WorldMap::new(warren.tiles.tables()));
    commands.insert_resource(PlaceRulesRes(Box::new(warren)));

    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO)),
            (Viewshed::new(9), RevealsMap, Faction(you), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack { kind: kinds.expect("kick"), dice: DiceRoll::new(1, 6) }),
            (Inventory::default(),),
        ))
        .id();
    warps.write(WarpRequest::into_place(player, map_of(1)));
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), Tones::NOTICE, 0);
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
/// What the engine cannot know: the bag, and which floor this is.
fn note_bag_and_floor(mut vitals: ResMut<VitalsView>, mut facets: ResMut<Facets>, map: Res<WorldMap>, player: Query<&Inventory, With<Player>>) {
    let Ok(bag) = player.single() else { return };
    let depth = floor_of(map.current());
    vitals.facets.push(facets.facet("crusts", format!("crusts {}", bag.items.len())));
    vitals.facets.push(facets.facet("floor", format!("floor {depth}/{FLOORS}: {}", name_of(depth))));
}
// ANCHOR_END: status

// ANCHOR: populate
/// Fills the floor the one time it is built. `PlaceEntered::first` is
/// true only on that arrival, so coming back does not restock it.
fn populate(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    bestiary: Res<Bestiary>,
    map: Res<WorldMap>,
    seed: Res<Seed>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
) {
    for ev in entered.read() {
        let depth = floor_of(ev.map);
        log.push(format!("Floor {depth}: {}.", name_of(depth)), Tones::NOTICE, turns.turn_number());
        // The keys, once, under the first floor's name. Not in `start`: the
        // name is written when the warp lands, a frame later, and would read
        // as though it came after them.
        if ev.first && depth == 1 {
            log.push("Something is scratching in the dark. g picks up, e eats, > goes down.", Tones::MUTED, turns.turn_number());
        }
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        // A stream of its own, keyed by name: adding another spawner later
        // cannot shift the numbers this one draws.
        let mut rng = seed.stream(b"warren.rats", ev.map.0 as u64);

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
            // Weight zero keeps it out of the table, so it is placed by name.
            (false, Some(throne)) => {
                bestiary.spawn(&mut commands, bestiary.defs.expect("rat king"), throne);
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
        // Groups, not individuals: the table says what may appear at this
        // depth, how likely it is, and how many come at once.
        let mut groups = 0;
        for _ in 0..60 {
            if groups >= 3 + depth as usize {
                break;
            }
            let Some((&id, count)) = bestiary.table.pick_group(depth as i32, &mut rng) else { break };
            let anchor = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            let mut placed = 0;
            for p in geometry::square(anchor, 2) {
                if placed >= count {
                    break;
                }
                // Not close enough to the stairs to be unfair.
                if map.is_walkable(p) && geometry::chebyshev(p, ev.entry) >= 8 {
                    bestiary.spawn(&mut commands, id, p);
                    placed += 1;
                }
            }
            if placed > 0 {
                groups += 1;
            }
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
    next: ResMut<'w, NextState<EngineState>>,
    bestiary: Res<'w, Bestiary>,
    players: Query<'w, 's, (), With<Player>>,
    kinds: Query<'w, 's, &'static Kind>,
}

/// Turns what the pipeline reported into English.
fn narrate(mut items: MessageReader<ItemEvent>, mut dealt: MessageReader<DamageDealt>, mut deaths: MessageReader<DeathEvent>, mut voice: Voice) {
    let Voice { turns, log, next, bestiary, players, kinds } = &mut voice;
    let turn = turns.turn_number();
    let name = |e: Entity| -> String {
        if players.contains(e) {
            "you".into()
        } else {
            kinds.get(e).map(|k| format!("the {}", bestiary.defs.get(k.0).name)).unwrap_or_else(|_| "something".into())
        }
    };
    for ev in items.read() {
        match *ev {
            ItemEvent::PickedUp { .. } => log.push("You pocket a crust of bread.", Tones::TEXT, turn),
            ItemEvent::Used { .. } => log.push("You eat the crust. It helps.", Tones::GOOD, turn),
            _ => {}
        }
    }
    for d in dealt.read() {
        let attacker = d.hit.attacker.map(&name).unwrap_or_else(|| "something".into());
        let mine = attacker == "you";
        let (verb, category) = if mine { ("hit", Tones::TEXT) } else { ("bites", Tones::BAD) };
        let tail = if d.dealt <= 0 { " and does nothing.".to_string() } else { format!(" for {}.", d.dealt) };
        log.push(format!("{} {verb} {}{tail}", capital(&attacker), name(d.target)), category, turn);
    }
    for d in deaths.read() {
        if d.was_player {
            log.push("The warren keeps you. Press q to quit.", Tones::BAD, turn);
            // Leaving `Playing` stops the loop: no turns, no input, but
            // the last frame stays on the screen.
            next.set(EngineState::Idle);
        } else if kinds.get(d.entity).is_ok_and(|k| bestiary.defs.get(k.0).name == "rat king") {
            log.push("The rat king falls. The scratching stops. Press q to quit.", Tones::NOTICE, turn);
            next.set(EngineState::Idle);
        } else {
            log.push(format!("{} dies.", capital(&name(d.entity))), Tones::GOOD, turn);
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
