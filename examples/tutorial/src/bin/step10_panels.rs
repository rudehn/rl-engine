//! Warren, step 10: the panels, and a rail down the side.
//!
//! The guide chapter is `docs/guide/src/10-panels.md`. Everything in the
//! rail comes from a view the engine keeps current: what is in sight with
//! its health, what is worn, and the look cursor's reading of a rat. What
//! the engine cannot know reaches a panel two ways, and both are here: a
//! `Name` on an entity, and a `Facet` pushed onto a row.
//!
//! `cargo run -p tutorial --bin step10_panels`, and `WARREN_SEED=19` picks
//! another warren, which is how this chapter's screenshot was taken.
//!
//! Keys: as step 9, plus `x` for the look cursor and `p` for the log.

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
/// Columns down the right given over to the rail.
const RAIL: i32 = 24;
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

// ANCHOR: layout
/// The screen, cut up once, so the map and every panel agree on it.
///
/// `panel::split_*` take a rectangle and a size and hand back both
/// halves, which is the whole of the engine's opinion about layout.
struct Screen {
    map: Rect,
    log: Rect,
    vitals: Rect,
    nearby: Rect,
    inspect: Rect,
    scrollback: Rect,
}

impl Screen {
    fn new() -> Self {
        let (left, rail) = panel::split_right(Rect::new(0, 0, COLS, ROWS), RAIL);
        let (map, log) = panel::split_bottom(left, LOG_ROWS);
        let (vitals, nearby) = panel::split_top(rail, 9);
        // Over the map, because a modal covers what it is about.
        let inspect = Rect::new(map.x + 2, map.bottom() - 10, map.width.min(46), 9);
        Self { map, log, vitals, nearby, inspect, scrollback: map.inflate(-2) }
    }
}
// ANCHOR_END: layout

// ANCHOR: main
fn main() -> AppExit {
    let screen = Screen::new();
    let mut app = App::new();
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map in its share of the screen, and the UI base.
    // `CapturePlugin` inside it only takes this guide's screenshots.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(screen.map))
        // Minds live in the combat plugin: deciding where to move and
        // deciding whom to hit are the same decision.
        .add_plugins((CombatPlugin, MindsPlugin, ItemsPlugin))
        .insert_resource(Seed(RunSeed(std::env::var("WARREN_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(7))))
        // ANCHOR: panels
        // Five panels. Each holds its own rectangle, reads a view the engine
        // keeps current, and draws itself: none of them needs a system here.
        // Warren has no equipment slots, so it takes no `GearPanel`. Opt-in
        // is per panel: you add the ones you have a game for.
        .add_plugins((
            VitalsPanel::new(screen.vitals).heading("Vitals").bars(10),
            NearbyPanel::new(screen.nearby).titled("").headings("In sight", "On the floor"),
            LogPanel::new(screen.log),
            InspectPanel::new(screen.inspect),
            // A second presenter over the log the strip already draws: `p`
            // opens all of it, scrollable and filterable by tone.
            ScrollbackPanel::new(screen.scrollback),
        ))
        // What the engine cannot know about a row. Named by set, never by
        // ordering after a collector function.
        .add_systems(Update, (note_bag_and_floor, note_what_a_rat_is_doing).in_set(ViewSet::Annotate))
        // ANCHOR_END: panels
        .add_systems(Startup, start)
        // Once a frame, before the turns: whatever the player pressed becomes
        // at most one intent, however many passes the turn loop then runs.
        // The game's own action: registered, then resolved alongside the
        // engine's. Without the resolver the sweep would refuse every shove.
        .add_action::<Shove>()
        .add_message::<Shoved>()
        .add_systems(Turn, resolve_shoves.in_set(ResolveSet::Act))
        // ANCHOR: gate
        // One gate for every screen there is and every screen added later:
        // the stack is empty, or the world does not have the keys.
        .add_systems(Update, player_input.in_set(EngineSet::Input).run_if(no_modal))
        // ANCHOR_END: gate
        // Both inside the turn: a floor fills the first time it is entered,
        // and a crust eaten heals before the next rat gets its bite in.
        .add_systems(Turn, (populate, eat).in_set(TurnSet::React))
        .add_systems(Update, narrate.in_set(PresentSet::Narrate));
    // ANCHOR: tone
    // A role the engine never heard of, and the colour for it. Every
    // widget that takes a tone honours it from here on.
    app.add_tone("fleeing", Color::srgb(0.6, 0.8, 1.0));
    // ANCHOR_END: tone
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
                (Name::new(def.name.clone()),),
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
            // What the panels call you. The engine has no names of its own.
            (Name::new("you"),),
            (Health::full(24), Armor(1), MeleeAttack { kind: kinds.expect("kick"), dice: DiceRoll::new(1, 6) }),
            (Inventory::default(),),
        ))
        .id();
    warps.write(WarpRequest::into_place(player, map_of(1)));
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), Tones::NOTICE, 0);
    log.push("g gets, e eats, > descends, x looks, p reads back.", Tones::MUTED, 0);
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
    shoves: MessageWriter<'w, Intent<Shove>>,
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
    let shifted = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if let Some((_, dir)) = MOVES.iter().find(|(codes, _)| shifted && keys.any_just_pressed(codes.iter().copied())) {
        intents.shoves.write(Intent::new(entity, Shove(*dir)));
    } else if let Some((_, dir)) = MOVES.iter().find(|(codes, _)| !shifted && keys.any_just_pressed(codes.iter().copied())) {
        match occupancy.first_at(pos.0 + dir.offset()) {
            Some(other) => {
                intents.attacks.write(Intent::new(entity, Attack(other)));
            }
            None => {
                intents.steps.write(Intent::new(entity, Step(*dir)));
            }
        }
    } else if keys.just_pressed(KeyCode::Enter) || (shifted && keys.any_just_pressed([KeyCode::Period, KeyCode::Comma])) {
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
///
/// A facet is a note on a view: a key, some words, and a tone. The engine
/// has no idea what a crust is, and this is how it never needs one.
fn note_bag_and_floor(mut vitals: ResMut<VitalsView>, mut facets: ResMut<Facets>, map: Res<WorldMap>, player: Query<&Inventory, With<Player>>) {
    let Ok(bag) = player.single() else { return };
    let depth = floor_of(map.current());
    vitals.facets.push(facets.facet("crusts", format!("crusts {}", bag.items.len())));
    vitals.facets.push(facets.facet("floor", format!("floor {depth}/{FLOORS}: {}", name_of(depth))));
}
// ANCHOR_END: status

// ANCHOR: annotate
/// What a rat is up to, on the row the engine built for it.
///
/// `MonsterAIMode` is not a thing the engine has; `flee_at` is this
/// game's rule. So the row gets a facet, in a tone this game declared,
/// and the rail prints it without knowing what fleeing is.
fn note_what_a_rat_is_doing(
    mut nearby: ResMut<NearbyView>,
    mut facets: ResMut<Facets>,
    tones: Res<Tones>,
    bestiary: Res<Bestiary>,
    rats: Query<(&Kind, &Health)>,
) {
    let fleeing = tones.get("fleeing").expect("declared while building");
    for row in nearby.actors.iter_mut() {
        let Ok((kind, health)) = rats.get(row.entity) else { continue };
        if health.hp <= bestiary.defs.get(kind.0).flee_at {
            row.facets.push(facets.facet("mood", "fleeing").toned(fleeing));
        }
    }
}
// ANCHOR_END: annotate

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
            // The `Name` is what puts it on the rail. Without one the
            // collector skips it: a nameless row is a spawn that forgot,
            // and a blank line is the hardest kind of that to notice.
            commands.spawn((Item, Crust(8), Name::new("a crust of bread"), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
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
fn narrate(
    mut items: MessageReader<ItemEvent>,
    mut shoved: MessageReader<Shoved>,
    mut dealt: MessageReader<DamageDealt>,
    mut deaths: MessageReader<DeathEvent>,
    mut voice: Voice,
) {
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
    for ev in shoved.read() {
        log.push(format!("You shove {} back.", name(ev.target)), Tones::TEXT, turn);
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

// ANCHOR: action
/// Shove whoever stands one cell away in this direction back another cell.
///
/// An action is a type. There is no list in the engine for it to be added
/// to; registering it makes `Intent<Shove>` a message, and the sweep
/// refuses any that no resolver claims.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shove(Direction);
impl Action for Shove {}

/// A shove that landed, for the log to read.
#[derive(Message, Debug, Clone, Copy)]
struct Shoved {
    target: Entity,
}
// ANCHOR_END: action

// ANCHOR: resolver
/// What the shove costs. A shove is quicker than a swing.
const SHOVE_COST: u32 = BASE_ACTION_COST / 2;

/// Everything the resolver moves.
#[derive(bevy::ecs::system::SystemParam)]
struct Shoving<'w, 's> {
    occupancy: ResMut<'w, Occupancy>,
    map: Res<'w, WorldMap>,
    holders: Query<'w, 's, &'static Position, With<MyTurn>>,
    targets: Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>), Without<MyTurn>>,
}

/// Resolves a shove.
///
/// The shape every resolver has: claim the turn so nothing else spends it,
/// do the thing, and say how it went. `done` charges what it cost; `failed`
/// leaves the player holding the turn and charges anyone else, so a
/// monster cannot try the same impossible shove forever.
fn resolve_shoves(mut intents: MessageReader<Intent<Shove>>, mut resolution: Resolution, mut shoved: MessageWriter<Shoved>, mut world: Shoving) {
    for intent in intents.read() {
        if !resolution.claim(intent.actor) {
            continue;
        }
        let Ok(from) = world.holders.get(intent.actor) else {
            resolution.failed(intent.actor, SHOVE_COST);
            continue;
        };
        let offset = intent.action.0.offset();
        let behind = from.0 + offset + offset;
        let room = world.map.is_walkable(behind) && !world.occupancy.is_occupied(behind);
        let pushed = world.occupancy.first_at(from.0 + offset).filter(|_| room).and_then(|t| world.targets.get_mut(t).ok().map(|found| (t, found)));
        let Some((target, (mut pos, viewshed))) = pushed else {
            // Nobody there, or nowhere for them to go: nothing happens, and
            // the turn is still the player's to spend on something else.
            resolution.failed(intent.actor, SHOVE_COST);
            continue;
        };
        world.occupancy.relocate(target, pos.0, behind);
        pos.0 = behind;
        if let Some(mut v) = viewshed {
            v.dirty = true;
        }
        shoved.write(Shoved { target });
        resolution.done(intent.actor, SHOVE_COST);
    }
}
// ANCHOR_END: resolver

#[cfg(test)]
mod tests {
    use super::*;

    // ANCHOR: headless
    /// The warren with no window: the engine plugins the game uses, the
    /// game's own systems, and nothing that needs a screen.
    fn headless(seed: u64) -> App {
        let mut app = rl_engine::rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, ItemsPlugin));
        app.insert_resource(Seed(RunSeed(seed)))
            .add_plugins(UiPlugin)
            .add_action::<Shove>()
            .add_message::<Shoved>()
            .add_systems(Startup, start)
            .add_systems(Turn, resolve_shoves.in_set(ResolveSet::Act))
            .add_systems(Turn, (populate, eat).in_set(TurnSet::React))
            .add_systems(Update, narrate.in_set(PresentSet::Narrate));
        app
    }

    /// A started run: two frames is enough for the warp to build floor one
    /// and the scheduler to deal the player its first turn.
    fn started(seed: u64) -> (App, Entity) {
        let mut app = headless(seed);
        app.update();
        app.update();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        (app, player)
    }

    fn act<A: Action>(app: &mut App, actor: Entity, action: A) {
        app.world_mut().write_message(Intent::new(actor, action));
        app.update();
    }
    // ANCHOR_END: headless

    // ANCHOR: property
    /// The property that has to hold for every floor of every run: you can
    /// stand where you arrive, and there is somewhere to go from there.
    #[test]
    fn every_floor_of_every_seed_has_a_walkable_way_in_and_a_way_on() {
        for seed in 1u64..=12 {
            let warren = Warren::new(RunSeed(seed));
            let tables = warren.tiles.tables();
            for depth in 1..=FLOORS {
                let built = warren.build(map_of(depth), None).unwrap_or_else(|e| panic!("seed {seed} floor {depth}: {e}"));
                let walkable = |p| built.terrain.get(p).is_some_and(|t: TileId| tables.walkable[t.index()]);
                assert!(walkable(built.entry), "seed {seed} floor {depth}: arrived inside a wall");
                assert!(built.exit.is_some_and(walkable), "seed {seed} floor {depth}: nowhere to go on to");
            }
        }
    }
    // ANCHOR_END: property

    #[test]
    fn the_stairs_lead_all_the_way_down_and_the_king_waits_on_the_last_floor() {
        let (mut app, player) = started(7);
        assert_eq!(app.world().resource::<WorldMap>().current(), map_of(1), "the run starts on floor one");
        for depth in 1..FLOORS {
            let down = app.world().resource::<WorldMap>().place(map_of(depth)).unwrap().exit.expect("a way down");
            app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: map_of(depth), arrive: Arrive::At(down) } });
            app.update();
            act(&mut app, player, GoThrough);
            assert_eq!(app.world().resource::<WorldMap>().current(), map_of(depth + 1), "took the stairs from floor {depth}");
        }
        // One more frame: what the last floor spawned gets its map tag.
        app.update();
        let bottom = map_of(FLOORS);
        let king = app.world().resource::<Bestiary>().defs.expect("rat king");
        let mut on_map = app.world_mut().query::<(&Kind, &OnMap)>();
        let kings = on_map.iter(app.world()).filter(|(k, on)| k.0 == king && on.0 == bottom).count();
        assert_eq!(kings, 1, "exactly one king, and it is on the bottom floor");
    }

    // ANCHOR: shove_tests
    /// A walkable cell next to the player with another walkable cell
    /// behind it, which is what a shove needs to land.
    fn room_to_shove(app: &App, from: Point) -> Direction {
        let map = app.world().resource::<WorldMap>();
        Direction::ALL
            .into_iter()
            .find(|d| map.is_walkable(from + d.offset()) && map.is_walkable(from + d.offset() + d.offset()))
            .expect("the entry of a built floor has room around it")
    }

    #[test]
    fn a_shove_moves_the_rat_one_cell_further_off_and_spends_half_a_turn() {
        let (mut app, player) = started(7);
        let at = app.world().get::<Position>(player).unwrap().0;
        let dir = room_to_shove(&app, at);
        let rat = app.world_mut().spawn((Actor, Blocks, Position(at + dir.offset()))).id();
        app.update();

        let before = app.world().resource::<Turns>().now();
        act(&mut app, player, Shove(dir));
        assert_eq!(app.world().get::<Position>(rat).unwrap().0, at + dir.offset() + dir.offset(), "the rat went back a cell");
        assert_eq!(app.world().resource::<Turns>().now(), before + SHOVE_COST, "and it cost half a turn");
    }

    #[test]
    fn a_shove_at_nobody_is_refused_costs_nothing_and_leaves_the_turn_in_hand() {
        let (mut app, player) = started(7);
        let at = app.world().get::<Position>(player).unwrap().0;
        let dir = room_to_shove(&app, at);

        let before = app.world().resource::<Turns>().now();
        act(&mut app, player, Shove(dir));
        assert_eq!(app.world().resource::<Turns>().now(), before, "no time passed");
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player still holds the turn");
    }
    // ANCHOR_END: shove_tests

    #[test]
    fn eating_a_crust_heals_inside_the_turn_and_leaves_nothing_in_the_bag() {
        let (mut app, player) = started(7);
        let crust = app.world_mut().spawn((Item, Crust(8))).id();
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(crust);
        app.world_mut().get_mut::<Health>(player).unwrap().hp = 10;

        act(&mut app, player, UseItem(crust));
        assert_eq!(app.world().get::<Health>(player).unwrap().hp, 18, "healed by the crust");
        // A despawned item is dropped from every bag by the engine.
        app.update();
        assert!(app.world().get_entity(crust).is_err(), "the crust is eaten");
        assert!(app.world().get::<Inventory>(player).unwrap().items.is_empty(), "and gone from the bag");
    }

    #[test]
    fn a_crust_never_heals_past_the_maximum() {
        let (mut app, player) = started(3);
        let crust = app.world_mut().spawn((Item, Crust(8))).id();
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(crust);
        let max = app.world().get::<Health>(player).unwrap().max;
        app.world_mut().get_mut::<Health>(player).unwrap().hp = max - 2;

        act(&mut app, player, UseItem(crust));
        assert_eq!(app.world().get::<Health>(player).unwrap().hp, max);
    }
}
