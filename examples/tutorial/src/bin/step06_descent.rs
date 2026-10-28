//! Warren, step 6: four floors, stairs between them, and a way out.
//!
//! The guide chapter is `docs/guide/src/06-two-floors.md`.
//!
//! `cargo run -p tutorial --bin step06_descent`
//!
//! Keys: arrows to walk or strike, `>` take the stairs, `a` screech, `g` get, `e` eat, `r` throw
//! a rock (Enter to confirm an aim, Escape to put it away), `t` lantern,
//! `.` wait, `q` quit.

use atglyph::prelude::*;
use atglyph::prelude::{combat::*, items::*, mapgen::*, minds::*, ui::*, world::*};
use atglyph::rand::Rng;
use atglyph::rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use atglyph::rules::ai::tactics::{Scavenge, ThrowAtRange, UseAbility};
use atglyph::rules::faction::FactionDef;
use atglyph::rules::{AbilityId, TagDef, TagId, Wits};
use bevy::prelude::*;
use std::sync::Arc;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows at the bottom of the terminal given over to the message log.
const LOG_ROWS: i32 = 5;

// ANCHOR: floors
/// How deep the warren goes.
const FLOORS: u32 = 4;

/// Floor `n` is map `n`.
fn map_of(floor: u32) -> MapId {
    MapId(floor)
}

/// The floor a map is.
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
        // ANCHOR: target
        // Draws the aiming cursor: the flight, who it would hit, and a box
        // saying so. Without it the cursor still opens, unseen.
        .add_plugins(TargetPanel::new(GridRect::new(1, ROWS - LOG_ROWS - 9, 44, 8)).hints("enter confirm  tab next  esc cancel"))
        // ANCHOR_END: target
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
        // Items lie on the floor or in a bag; thrown ones fly. The target
        // view runs the aiming cursor that `main` draws.
        app.add_plugins((ItemsPlugin, ThrowingPlugin, TargetViewPlugin));
        // Abilities, and the effects one may name: the engine's seven.
        app.add_plugins(AbilitiesPlugin).add_engine_effects();
        app.add_systems(NewRun, start);
        // Inside the turn: a floor fills the first time it is entered, and
        // a crust heals before the next rat bites.
        app.add_systems(Turn, (populate, eat, leave).in_set(TurnSet::React));
        app.add_systems(Update, narrate.in_set(PresentSet::Narrate));
        // ANCHOR: gate
        app.add_systems(Update, (player_input, tend_lantern).in_set(EngineSet::Input).run_if(no_modal));
        // ANCHOR_END: gate
        // The vitals view is data the panel in `main` draws; the game writes
        // a line onto it, so the game adds it.
        app.add_plugins(VitalsViewPlugin).add_systems(Update, (note_explored, note_bag).in_set(ViewSet::Annotate));
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
    fn build(&self, map: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
        let (wall, floor) = (self.tiles.expect("earth"), self.tiles.expect("dirt"));
        let mut ctx = BaseContext::blank(84, 42, self.tiles.clone(), wall);
        let chain = match floor_of(map) {
            // Dug rooms near the surface.
            1 | 2 => Chain::new()
                .then(dungeon::Rooms { floor, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 })
                .then(dungeon::Doors { door: self.tiles.expect("roots") }),
            // Gnawed-out caves below them.
            3 => Chain::new().then(passes::CellularCave { wall, floor, fill_pct: 45, ..Default::default() }).then(passes::KeepLargestRegion { wall }),
            // One wide chamber at the bottom.
            _ => Chain::new().then(dungeon::Rooms { floor, attempts: 60, min_size: 12, max_size: 18, min_rooms: 2 }),
        };
        // A start, and the point farthest from it: where the stairs down go.
        chain.then(dungeon::RandomStart).then(dungeon::FarthestExit).run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }

    fn tiles(&self) -> TileTables {
        self.tiles.tables()
    }

    // ANCHOR: ladder
    /// The floors, top first, and how their stairs look. The engine lays a
    /// stair up at each floor's start and a stair down at its far point.
    /// The last floor's far point is the game's: the way out.
    fn ladder(&self) -> Option<Ladder> {
        let stone = Rgb::new(204, 204, 219);
        let (up, down) = (StairLook::new('<', stone, "stairs up"), StairLook::new('>', stone, "stairs down"));
        Some(Ladder::new((1..=FLOORS).map(map_of), up, down).bottom(LadderEnd::Game))
    }
    // ANCHOR_END: ladder
}
// ANCHOR_END: rules

// ANCHOR: start
/// Hands combat its registries and the rats their brain, then spawns the
/// player at the first floor's start, lantern open.
fn start(mut commands: Commands, seed: Res<Seed>, effect_kinds: Res<EffectKinds>, mut log: Log) {
    // What damage can be, and who hates whom: the game's content, which
    // the engine knows only by id.
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("kick"), DamageKind::new("din")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new().hostile(you, vermin));
    // ANCHOR: load
    // A tag a gnaw's cost names, so a crust can be spent by it.
    let tags = Registry::from_defs(vec![TagDef::new("bread")]).unwrap();
    let bread = tags.expect("bread");
    let registries = Registries { damage_kinds: kinds.clone(), factions: sides, tags, ..default() };
    // Checked against the registries and the effects as it loads, so a
    // misspelt name fails here, at startup.
    let knacks = Abilities::load(KNACKS_RON, &effect_kinds, &registries.names()).unwrap_or_else(|e| panic!("assets/knacks.ron: {e}"));
    let screech = knacks.defs().expect("screech");
    let gnaw = knacks.defs().expect("gnaw");
    commands.insert_resource(Knacks { screech });
    commands.insert_resource(knacks);
    // ANCHOR_END: load
    commands.insert_resource(Rats {
        // Asked in order, and the first that answers wins: bite what is
        // beside you, run when badly hurt, chase what you see, else wander.
        mind: Arc::new(Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { at_pct: 30 }).then(Hunt).then(Wander { chance_pct: 40 })),
        // A ratling has hands: it fetches rocks and throws them.
        ratling: Arc::new(
            Brain::new()
                .then(MeleeAdjacent)
                .then(FleeWhenHurt { at_pct: 25 })
                // Uses a knack when it would help: a ratling gnaws its
                // crust when hurt.
                .then(UseAbility { chance_pct: 80 })
                .then(ThrowAtRange { chance_pct: 60 })
                .then(Scavenge { reach: 9 })
                .then(Hunt)
                .then(Wander { chance_pct: 30 }),
        ),
        bite: kinds.expect("bite"),
        faction: vermin,
        bread,
        gnaw,
    });
    commands.insert_resource(registries);

    commands.spawn_player(
        map_of(1),
        (
            (Viewshed::new(9), RevealsMap, LANTERN, Faction(you), Grants(vec![screech]), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack::new(kinds.expect("kick"), DiceRoll::new(1, 6)), Inventory::default()),
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
    screech: ControlId,
    eat: ControlId,
    throw: ControlId,
    lantern: ControlId,
    quit: ControlId,
}

/// Declares the keys. Walking and waiting are the engine's standard keys.
fn declare_controls(app: &mut App) {
    app.add_plugins(
        PlayerKeysPlugin::new()
            .walk("Move", "walk or attack")
            .go_through("Move", "take the stairs")
            .pick_up("Act", "pick up what is here", Otherwise::say("There is nothing here."))
            .wait("Act", "wait a turn"),
    );
    let binds = Binds {
        screech: app.add_control("Act", "screech", KeyCode::KeyA),
        eat: app.add_control("Act", "eat a crust", KeyCode::KeyE),
        throw: app.add_control("Act", "throw a rock", KeyCode::KeyR),
        lantern: app.add_control("Act", "lantern on or off", KeyCode::KeyT),
        quit: app.add_control("Game", "quit", KeyCode::KeyQ),
    };
    app.insert_resource(binds);
}

// ANCHOR_END: keys

// ANCHOR: input
/// The player, while it holds the turn, and its bag.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Inventory), (With<Player>, With<MyTurn>)>;

/// Reads Warren's own keys once a frame. Asking to act is writing an
/// intent: the engine spends the turn, or refuses what cannot be done.
fn player_input(
    keys: ControlInput,
    binds: Res<Binds>,
    player: PlayerTurn,
    carried: Carried,
    knacks: Res<Knacks>,
    mut intents: PlayerIntents,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(binds.quit) {
        exit.write(AppExit::Success);
    }
    // Not the player's turn: drop the key.
    let Ok((entity, bag)) = player.single() else { return };
    if keys.just_pressed(binds.screech) {
        // Opens the aiming cursor. The engine uses the knack on Enter.
        intents.knacks.write(AimAt { user: entity, ability: knacks.screech });
    } else if keys.just_pressed(binds.eat) {
        if let Some(crust) = bag.items.iter().copied().find(|i| carried.crusts.contains(*i)) {
            intents.uses.write(Intent::new(entity, UseItem(crust)));
        }
    } else if keys.just_pressed(binds.throw)
        && let Some(rock) = bag.items.iter().copied().find(|i| carried.rocks.contains(*i))
    {
        // Opens the aiming cursor. The engine writes the throw on Enter.
        intents.aims.write(AimThrow { user: entity, item: rock });
    }
}

/// Every intent Warren's keys write, in one parameter.
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerIntents<'w> {
    uses: MessageWriter<'w, Intent<UseItem>>,
    aims: MessageWriter<'w, AimThrow>,
    knacks: MessageWriter<'w, AimAt>,
}

/// What the bag might hold, so a key can pick the right thing.
#[derive(bevy::ecs::system::SystemParam)]
struct Carried<'w, 's> {
    crusts: Query<'w, 's, (), With<Crust>>,
    rocks: Query<'w, 's, (), With<Throwable>>,
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
    ratling: Arc<Brain<Entity>>,
    bite: atglyph::rules::damage::DamageKindId,
    faction: FactionId,
    /// The tag that makes a crust food for a gnaw, and the gnaw.
    bread: TagId,
    gnaw: AbilityId,
}
// ANCHOR_END: creatures

// ANCHOR: populate
/// The random stream rats are placed from. Each floor draws it from its own
/// seed, so floors can be entered in any order.
const RATS: SeedDomain = SeedDomain::new(b"warren.rats");

/// Fills a floor the first time it is entered.
fn populate(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    rats: Res<Rats>,
    map: Res<WorldMap>,
    seed: Res<Seed>,
    mut tell: MessageWriter<Tell>,
) {
    for ev in entered.read() {
        let depth = floor_of(ev.map);
        tell.write(Tell::new(format!("Floor {depth}: {}.", name_of(depth)), Tones::NOTICE));
        if !ev.first {
            continue;
        }
        // The stairs are the engine's. The way out, at the last floor's far
        // point, is an entity the game spawns.
        if let Some(far) = ev.exit.filter(|_| depth == FLOORS) {
            commands.spawn((Position(far), WayOut, Name::new("crack of daylight"), Glyph::new('<', Color::srgb(1.0, 0.95, 0.70)).on_layer(1)));
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
            // Every fourth is a ratling: the same body, a cleverer mind.
            let clever = placed % 4 == 3;
            // A ratling carries its supper, and knows how to eat it.
            let supper = clever.then(|| commands.spawn((Item, Crust(6), Tagged(vec![rats.bread]), Name::new("crust of bread"))).id());
            let mut rat = commands.spawn((
                (Actor, Blocks, Position(p), Speed(110), Faction(rats.faction)),
                (Health::full(6), Armor(0), Perception(7), DarkSight(9)),
                (MeleeAttack::new(rats.bite, DiceRoll::new(1, 3)),),
            ));
            if clever {
                rat.insert((
                    Mind(rats.ratling.clone()),
                    // Wits say what a mind may consider: a ratling opens
                    // doors, picks things up and throws them.
                    Intelligence(Wits::SAPIENT),
                    Inventory { items: supper.into_iter().collect() },
                    Grants(vec![rats.gnaw]),
                    Glyph::new('R', Color::srgb(0.85, 0.66, 0.50)).on_layer(5),
                    Name::new("ratling"),
                ));
            } else {
                rat.insert((Mind(rats.mind.clone()), Intelligence(Wits::ANIMAL), Glyph::new('r', Color::srgb(0.72, 0.55, 0.45)).on_layer(5), Name::new("rat")));
            }
            placed += 1;
        }
        // Bread and rocks, scattered the same way.
        let mut dropped = 0;
        while dropped < 14 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            if map.is_walkable(p) {
                litter(&mut commands, &rats, p, dropped % 2 == 0);
                dropped += 1;
            }
        }
    }
}
// ANCHOR_END: populate

// ANCHOR: litter
/// A crust of bread, and how much it heals.
#[derive(Component, Clone, Copy)]
struct Crust(i32);

/// Drops a crust or a rock at `p`.
fn litter(commands: &mut Commands, rats: &Rats, p: Point, bread: bool) {
    if bread {
        commands.spawn((Item, Crust(8), Name::new("crust of bread"), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
    } else {
        // Thrown up to 7 cells, for 1d4.
        let flies = Throwable::new(Some(7), Some((rats.bite, DiceRoll::new(1, 4))));
        commands.spawn((Item, flies, Name::new("rock"), Position(p), Glyph::new('*', Color::srgb(0.66, 0.66, 0.70)).on_layer(2)));
    }
}
// ANCHOR_END: litter

// ANCHOR: eat
/// What eating a crust does. The engine has spent the turn and taken the
/// crust out of the bag; healing is the game's part.
fn eat(mut commands: Commands, mut used: MessageReader<ItemEvent>, crusts: Query<&Crust>, mut eaters: Query<&mut Health>) {
    for ev in used.read() {
        let ItemEvent::Used { actor, item } = *ev else { continue };
        let (Ok(crust), Ok(mut health)) = (crusts.get(item), eaters.get_mut(actor)) else { continue };
        health.current = (health.current + crust.0).min(health.max);
        commands.entity(item).despawn();
    }
}

/// The log line for eating. Blows, deaths and pickups the engine narrates.
fn narrate(mut items: MessageReader<ItemEvent>, mut log: Log) {
    for ev in items.read() {
        if let ItemEvent::Used { .. } = *ev {
            log.push("You eat the crust. It helps.", Tones::GOOD);
        }
    }
}

/// Adds a line to the vitals panel: how many crusts you carry.
fn note_bag(mut vitals: ResMut<VitalsView>, player: Query<&Inventory, With<Player>>, crusts: Query<(), With<Crust>>) {
    let Ok(bag) = player.single() else { return };
    let count = bag.items.iter().filter(|i| crusts.contains(**i)).count();
    vitals.facets.push(Facet::new("crusts", format!("crusts {count}")));
}
// ANCHOR_END: eat

// ANCHOR: way_out
/// The crack of daylight on the last floor.
#[derive(Component, Clone, Copy)]
struct WayOut;

/// Standing on the daylight wins the run, on the step that reached it.
fn leave(player: Query<(&Position, &OnMap), With<Player>>, ways: Query<(&Position, &OnMap), With<WayOut>>, mut over: MessageWriter<RunOver>) {
    let Ok((at, on)) = player.single() else { return };
    if ways.iter().any(|(way, way_on)| way.0 == at.0 && way_on.0 == on.0) {
        over.write(RunOver::won().saying("You come up into the roots, and the scratching stays below."));
    }
}
// ANCHOR_END: way_out

// ANCHOR: knack
/// The knacks, compiled in so the binary runs from anywhere.
const KNACKS_RON: &str = include_str!("../../assets/knacks.ron");

/// The knack the player's key names.
#[derive(Resource)]
struct Knacks {
    screech: AbilityId,
}
// ANCHOR_END: knack

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph::testing::{KeyScriptPlugin, at, headless, player, press, settle};

    // ANCHOR: headless
    fn started(seed: u64) -> (App, Entity) {
        let mut app = headless(RunSeed(seed), narrator(), (Warren, KeyScriptPlugin));
        let me = player(&mut app);
        (app, me)
    }
    // ANCHOR_END: headless

    /// A cell `steps` away from the player in some direction, with open
    /// floor all the way, so a thrown rock can reach it.
    fn open_line(app: &App, from: Point, steps: i32) -> Point {
        let map = app.world().resource::<WorldMap>();
        Direction::ALL
            .into_iter()
            .find(|d| (1..=steps).all(|n| map.is_walkable(Point::new(from.x + d.offset().x * n, from.y + d.offset().y * n))))
            .map(|d| Point::new(from.x + d.offset().x * steps, from.y + d.offset().y * steps))
            .expect("the start of a built floor has room around it")
    }

    #[test]
    fn a_pressed_direction_key_spends_a_turn() {
        let (mut app, me) = started(7);
        let before = (at(&app, me), app.world().resource::<Turns>().now());
        for key in [KeyCode::ArrowRight, KeyCode::ArrowLeft, KeyCode::ArrowUp, KeyCode::ArrowDown] {
            press(&mut app, key);
            settle(&mut app);
            if (at(&app, me), app.world().resource::<Turns>().now()) != before {
                return;
            }
        }
        panic!("no direction key moved the player or spent a turn");
    }

    /// `r` opens the aiming cursor on the rat, and Enter throws the rock at it.
    #[test]
    fn r_then_enter_throws_the_rock_at_the_rat() {
        let (mut app, me) = started(7);
        let here = at(&app, me);
        let vermin = app.world().resource::<Rats>().faction;
        let there = open_line(&app, here, 3);
        app.world_mut().spawn((Actor, Blocks, Position(there), Faction(vermin), Health::full(6), Name::new("rat"), Glyph::new('r', Color::WHITE)));
        let rock = app.world_mut().spawn((Item, Throwable::new(Some(7), None), Name::new("rock"))).id();
        app.world_mut().get_mut::<Inventory>(me).unwrap().items.push(rock);
        settle(&mut app);

        press(&mut app, KeyCode::KeyR);
        settle(&mut app);
        assert!(app.world().resource::<Modals>().any_open(), "r opened no aiming cursor");
        let before = app.world().resource::<Turns>().now();
        press(&mut app, KeyCode::Enter);
        settle(&mut app);

        assert!(!app.world().resource::<Modals>().any_open(), "the cursor is still open, holding the keys");
        assert!(!app.world().get::<Inventory>(me).unwrap().items.contains(&rock), "the rock is still in the bag");
        assert!(app.world().resource::<Turns>().now() > before, "the throw spent no time");
        assert_eq!(app.world().get::<Position>(rock).map(|p| p.0), Some(there), "the rock did not land at the rat");
    }

    /// Every floor of a span of seeds has somewhere to stand at its start
    /// and somewhere to go from there.
    #[test]
    fn every_floor_has_a_walkable_start_and_a_far_point() {
        let floors = Floors::new();
        let tables = floors.tiles.tables();
        for seed in atglyph::testing::seeds(1..13) {
            for depth in 1..=FLOORS {
                let built = floors.build(map_of(depth), PlaceSeed::of(RunSeed(seed), depth)).unwrap_or_else(|e| panic!("seed {seed} floor {depth}: {e}"));
                let walkable = |p| built.terrain.get(p).is_some_and(|t: TileId| tables.walkable[t.index()]);
                assert!(walkable(built.entry), "seed {seed} floor {depth}: the start is in rock");
                assert!(built.exit.is_some_and(walkable), "seed {seed} floor {depth}: no far point");
            }
        }
    }

    /// The stairs lead down floor by floor, and the last has the daylight.
    #[test]
    fn the_stairs_lead_down_to_the_daylight() {
        let (mut app, me) = started(7);
        for depth in 1..FLOORS {
            let down = app.world().resource::<WorldMap>().place(map_of(depth)).unwrap().exit.expect("a way down");
            app.world_mut().write_message(WarpRequest { actor: me, to: Destination::Place { map: map_of(depth), arrive: Arrive::At(down) }, after: None });
            settle(&mut app);
            app.world_mut().write_message(Intent::new(me, GoThrough));
            settle(&mut app);
            assert_eq!(app.world().resource::<WorldMap>().current(), map_of(depth + 1), "took the stairs from floor {depth}");
        }
        let world = app.world_mut();
        let daylight = world.query_filtered::<&OnMap, With<WayOut>>().iter(world).filter(|on| on.0 == map_of(FLOORS)).count();
        assert_eq!(daylight, 1, "one way out, on the last floor");
    }

    /// `a` opens the aiming cursor on the rat, and Enter screeches at it.
    #[test]
    fn a_then_enter_screeches_at_the_rat() {
        let (mut app, me) = started(7);
        let here = at(&app, me);
        let vermin = app.world().resource::<Rats>().faction;
        let there = open_line(&app, here, 3);
        let rat =
            app.world_mut().spawn((Actor, Blocks, Position(there), Faction(vermin), Health::full(50), Name::new("rat"), Glyph::new('r', Color::WHITE))).id();
        settle(&mut app);

        press(&mut app, KeyCode::KeyA);
        settle(&mut app);
        assert!(app.world().resource::<Modals>().any_open(), "a opened no aiming cursor");
        press(&mut app, KeyCode::Enter);
        settle(&mut app);

        assert!(!app.world().resource::<Modals>().any_open(), "the cursor is still open, holding the keys");
        assert!(app.world().get::<Health>(rat).unwrap().current < 50, "the screech did not hurt the rat");
    }
}
