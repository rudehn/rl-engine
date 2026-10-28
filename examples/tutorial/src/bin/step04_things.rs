//! Warren, step 4: things to pick up, eat and throw, and ratlings that
//! know what to do with them.
//!
//! The guide chapter is `docs/guide/src/04-things-and-minds.md`.
//!
//! `cargo run -p tutorial --bin step04_things`
//!
//! Keys: arrows to walk or strike, `g` get, `e` eat, `r` throw a rock
//! (Enter to throw, Escape to put it away), `t` lantern, `.` wait, `q` quit.

use atglyph::prelude::*;
use atglyph::prelude::{combat::*, items::*, minds::*, ui::*, world::*};
use atglyph::rand::Rng;
use atglyph::rules::Wits;
use atglyph::rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use atglyph::rules::ai::tactics::{Scavenge, ThrowAtRange};
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
        // ANCHOR: target
        // Draws the aiming cursor: the flight, who it would hit, and a box
        // saying so. Without it the cursor still opens, unseen.
        .add_plugins(TargetPanel::new(GridRect::new(1, ROWS - LOG_ROWS - 9, 44, 8)).hints("enter throw  tab next  esc cancel"))
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
        app.add_systems(NewRun, start);
        // Inside the turn: a floor fills the first time it is entered, and
        // a crust heals before the next rat bites.
        app.add_systems(Turn, (populate, eat).in_set(TurnSet::React));
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
    // ANCHOR: combat
    // What damage can be, and who hates whom: the game's content, which
    // the engine knows only by id.
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("kick")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new().hostile(you, vermin));
    commands.insert_resource(Registries { damage_kinds: kinds.clone(), factions: sides, ..default() });
    // ANCHOR_END: combat
    commands.insert_resource(Rats {
        // Asked in order, and the first that answers wins: bite what is
        // beside you, run when badly hurt, chase what you see, else wander.
        mind: Arc::new(Brain::new().then(MeleeAdjacent).then(FleeWhenHurt { at_pct: 30 }).then(Hunt).then(Wander { chance_pct: 40 })),
        // A ratling has hands: it fetches rocks and throws them.
        ratling: Arc::new(
            Brain::new()
                .then(MeleeAdjacent)
                .then(FleeWhenHurt { at_pct: 25 })
                .then(ThrowAtRange { chance_pct: 60 })
                .then(Scavenge { reach: 9 })
                .then(Hunt)
                .then(Wander { chance_pct: 30 }),
        ),
        bite: kinds.expect("bite"),
        faction: vermin,
    });

    commands.spawn_player(
        MapId::FIRST,
        (
            (Viewshed::new(9), RevealsMap, LANTERN, Faction(you), Glyph::new('@', Color::WHITE).on_layer(10)),
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
    eat: ControlId,
    throw: ControlId,
    lantern: ControlId,
    quit: ControlId,
}

/// Declares the keys. Walking and waiting are the engine's standard keys.
fn declare_controls(app: &mut App) {
    app.add_plugins(PlayerKeysPlugin::new().walk("Move", "walk or attack").pick_up("Act", "pick up what is here").wait("Act", "wait a turn"));
    let binds = Binds {
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
fn player_input(keys: ControlInput, binds: Res<Binds>, player: PlayerTurn, carried: Carried, mut intents: PlayerIntents, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(binds.quit) {
        exit.write(AppExit::Success);
    }
    // Not the player's turn: drop the key.
    let Ok((entity, bag)) = player.single() else { return };
    if keys.just_pressed(binds.eat) {
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
            // Every fourth is a ratling: the same body, a cleverer mind.
            let clever = placed % 4 == 3;
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
                    Inventory::default(),
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
}
