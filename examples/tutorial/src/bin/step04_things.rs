//! Warren, step 4: things to carry, throw and eat, and the minds that
//! know what to do with them.
//!
//! The guide chapter is `docs/guide/src/04-things-and-minds.md`. Rocks fly
//! at whoever is in the way, bread mends, and what a monster does with
//! either is its [`Intelligence`]: a rat is an animal, a ratling is not.
//!
//! `cargo run -p tutorial --bin step04_things`
//!
//! Keys: arrows to walk or strike, `g` get, `e` eat, `r` throw a rock,
//! `t` lantern, `.` wait, `q` quit.

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_rules::Wits;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use rl_engine::rl_rules::ai::tactics::{Scavenge, ThrowAtRange};
use rl_engine::rl_rules::damage::SubtractArmor;
use rl_engine::rl_rules::faction::FactionDef;
use std::sync::Arc;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows at the bottom of the terminal given over to the message log.
const LOG_ROWS: i32 = 5;

/// Map zero is the streamed surface. The warren has no surface, so its
/// one floor is map one.
const WARREN: MapId = MapId(1);

// ANCHOR: main
fn main() -> AppExit {
    let mut app = App::new();
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map in everything but the status row and the log, and the UI base.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        // Without this the world is lit everywhere and sight is geometry
        // alone. With it, `visible` shrinks to what a light reaches.
        .add_plugins((CombatPlugin, MindsPlugin, LightingPlugin, ItemsPlugin, ThrowingPlugin, TargetViewPlugin))
        .insert_resource(Lighting::dark())
        .insert_resource(Seed(RunSeed(7)))
        // Two panels: the vitals strip on the top row, the log along the
        // bottom. Each draws itself; neither needs a system of yours.
        .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)).hints("[g]et [e]at [r]ock [t]orch [.]wait [q]uit"))
        .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
        // The engine narrates blows, deaths and pickups into the log, naming
        // things in their own colours. Warren changes one phrase: what a rat
        // does to you is a bite.
        .add_plugins(NarratorPlugin::default().phrase(Phrase::HitsYou, "{Who} bites you for {n}.", Tones::BAD))
        // Escape opens the menu, and the run's end opens it by itself.
        .add_plugins(GameMenuPanel::new(Rect::new(COLS / 2 - 20, 8, 40, 12)).died("The warren keeps you."))
        .insert_resource(Morgue::platform_default("warren", "Warren"))
        .add_systems(NewRun, start)
        // A floor fills the first time it is entered, inside the turn.
        .add_systems(Turn, (populate, eat).in_set(TurnSet::React))
        .add_systems(Update, note_bag.in_set(ViewSet::Annotate))
        .add_systems(Update, narrate.in_set(PresentSet::Narrate))
        // Once a frame, before the turns: whatever the player pressed becomes
        // at most one intent, however many passes the turn loop then runs.
        .add_systems(Update, (player_input, tend_lantern).in_set(EngineSet::Input))
        .add_systems(Update, note_explored.in_set(ViewSet::Annotate));
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
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new(&sides).hostile(you, vermin));
    commands.insert_resource(Registries { damage_kinds: kinds.clone(), factions: sides, ..default() });
    // What a hit passes through on its way to the target. One stage here;
    // resistances, a shield, a critical rule would each be another.
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    commands.insert_resource(Rats {
        // A ratling has hands and a grudge: it fetches a rock and throws it,
        // and leaves an enemy at its elbow to its teeth. A rat has neither.
        ratling: Arc::new(
            Brain::new()
                .then(MeleeAdjacent)
                .then(FleeWhenHurt { at_pct: 25 })
                .then(ThrowAtRange { chance_pct: 60 })
                .then(Scavenge { reach: 9 })
                .then(Hunt)
                .then(Wander { chance_pct: 30 }),
        ),
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
            (Actor, Player, Blocks, Position(Point::ZERO)),
            (Viewshed::new(9), RevealsMap, LANTERN, Faction(you), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack { kind: kinds.expect("kick"), dice: DiceRoll::new(1, 6) }, Inventory::default()),
        ))
        .id();
    warps.write(WarpRequest::into_place(player, WARREN));
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), Tones::NOTICE, 0);
    log.push("Something is scratching in the dark.", Tones::MUTED, 0);
    next.set(EngineState::Playing);
}
// ANCHOR_END: start

// ANCHOR: input
/// The player, but only while it is holding the turn, and what it carries.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, Option<&'static Inventory>), (With<Player>, With<MyTurn>)>;

/// Keys to intents. Writing an intent is the whole of asking to act: the
/// engine claims the turn, charges it, and refuses what cannot be done.
///
/// The walk keys write a [`Bump`], which the engine resolves to a step, a
/// blow at a foe, or opening a door, whichever is in the way. `r` writes no
/// intent at all: it opens the engine's aiming cursor, and the throw is
/// written when the cursor is committed.
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    dirs: Res<DirectionKeys>,
    repeats: Res<Repeats>,
    player: PlayerTurn,
    carried: Carried,
    mut intents: PlayerIntents,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        intents.exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move; the key is dropped.
    let Ok((entity, bag)) = player.single() else { return };
    // A press walks, and a key held down keeps walking: `Repeats` is the
    // engine's hold, already advanced before input is read.
    if let Some(dir) = dirs.just_pressed(&keys).or_else(|| repeats.firing_any().map(|(d, _)| d)) {
        intents.bumps.write(Intent::new(entity, Bump(dir)));
    } else if keys.just_pressed(KeyCode::KeyG) {
        intents.pick_ups.write(Intent::new(entity, PickUp));
    } else if keys.just_pressed(KeyCode::KeyE) {
        if let Some(crust) = bag.into_iter().flat_map(|b| b.items.iter().copied()).find(|i| carried.crusts.contains(*i)) {
            intents.uses.write(Intent::new(entity, UseItem(crust)));
        }
    } else if keys.just_pressed(KeyCode::KeyR) {
        if let Some(rock) = bag.into_iter().flat_map(|b| b.items.iter().copied()).find(|i| carried.rocks.contains(*i)) {
            intents.aims.write(AimThrow { user: entity, item: rock });
        }
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        intents.waits.write(Intent::new(entity, Wait));
    }
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
    /// The brain of something with hands.
    ratling: Arc<Brain<Entity>>,
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
        let mut rng = seed.stream(b"warren.rats", ev.map.0 as u64);
        let mut placed = 0;
        while placed < 16 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            // Not on top of the player, and not close enough to be unfair.
            if !map.is_walkable(p) || geometry::chebyshev(p, ev.entry) < 8 {
                continue;
            }
            // Every fourth is a ratling: the same body, a better mind.
            let clever = placed % 4 == 3;
            let mut e = commands.spawn((
                (Actor, Blocks, Position(p), Speed(110), Faction(rats.faction)),
                (Health::full(6), Armor(0), Perception(7), DarkSight(9)),
                (MeleeAttack { kind: rats.bite, dice: DiceRoll::new(1, 3) },),
            ));
            if clever {
                e.insert((
                    Mind(rats.ratling.clone()),
                    // Wits are what a mind is allowed to consider. A ratling
                    // opens doors, fetches what it can throw, and throws it.
                    Intelligence(Wits::SAPIENT),
                    Inventory::default(),
                    Glyph::new('R', Color::srgb(0.85, 0.66, 0.50)).on_layer(5),
                    Name::new("ratling"),
                ));
            } else {
                e.insert((
                    Mind(rats.mind.clone()),
                    // An animal flees and searches, and that is all.
                    Intelligence(Wits::ANIMAL),
                    Glyph::new('r', Color::srgb(0.72, 0.55, 0.45)).on_layer(5),
                    Name::new("rat"),
                ));
            }
            placed += 1;
        }
        // Bread to mend with and rocks to throw, scattered the same way.
        let mut dropped = 0;
        while dropped < 14 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            if !map.is_walkable(p) {
                continue;
            }
            litter(&mut commands, &rats, p, dropped % 2 == 0);
            dropped += 1;
        }
    }
}
// ANCHOR_END: populate

// ANCHOR: crust
/// A crust of bread: the one item the warren has, and how much it heals.
#[derive(Component, Clone, Copy)]
struct Crust(i32);
// ANCHOR_END: crust

// ANCHOR: intents
/// Everything the player's keys can ask for. A system may take seven
/// parameters; bundling the writers into one `SystemParam` keeps room for
/// as many actions as the game grows.
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerIntents<'w> {
    bumps: MessageWriter<'w, Intent<Bump>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    pick_ups: MessageWriter<'w, Intent<PickUp>>,
    uses: MessageWriter<'w, Intent<UseItem>>,
    aims: MessageWriter<'w, AimThrow>,
    exit: MessageWriter<'w, AppExit>,
}

/// What the bag might hold, so the keys can tell one thing from another.
#[derive(bevy::ecs::system::SystemParam)]
struct Carried<'w, 's> {
    crusts: Query<'w, 's, (), With<Crust>>,
    rocks: Query<'w, 's, (), With<Throwable>>,
}
// ANCHOR_END: intents

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
        health.current = (health.current + crust.0).min(health.max);
        commands.entity(item).despawn();
    }
}
// ANCHOR_END: eat

// ANCHOR: narrate
/// What only Warren can put into words. Blows, deaths and pickups are the
/// engine's narrator's; what eating a crust means is this game's.
fn narrate(mut items: MessageReader<ItemEvent>, turns: Res<Turns>, mut log: ResMut<MessageLog>) {
    for ev in items.read() {
        if let ItemEvent::Used { .. } = *ev {
            log.push("You eat the crust. It helps.", Tones::GOOD, turns.turn_number());
        }
    }
}
// ANCHOR_END: narrate

// ANCHOR: status
/// What the engine cannot know: how many crusts are in the bag.
fn note_bag(mut vitals: ResMut<VitalsView>, mut facets: ResMut<Facets>, player: Query<&Inventory, With<Player>>) {
    let Ok(bag) = player.single() else { return };
    vitals.facets.push(facets.facet("crusts", format!("crusts {}", bag.items.len())));
}
// ANCHOR_END: status

// ANCHOR: litter
/// What the floor is littered with: bread that mends, rocks that fly.
fn litter(commands: &mut Commands, rats: &Rats, p: Point, bread: bool) {
    if bread {
        commands.spawn((Item, Crust(8), Name::new("a crust of bread"), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
    } else {
        commands.spawn((
            Item,
            Throwable { range: 7, strike: Some((rats.bite, DiceRoll::new(1, 4))) },
            Name::new("a rock"),
            Position(p),
            Glyph::new('*', Color::srgb(0.66, 0.66, 0.70)).on_layer(2),
        ));
    }
}
// ANCHOR_END: litter

#[cfg(test)]
mod tests {
    use super::*;

    /// The warren with no window, wired as `main` wires it.
    fn started(seed: u64) -> (App, Entity) {
        let mut app = rl_engine::rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, LightingPlugin, ItemsPlugin, ThrowingPlugin));
        app.add_plugins(UiPlugin).add_plugins(TargetViewPlugin).add_plugins(rl_engine::rl_bevy::testing::KeyScriptPlugin);
        app.insert_resource(Lighting::dark())
            .insert_resource(Seed(RunSeed(seed)))
            .add_systems(NewRun, start)
            .add_systems(Turn, (populate, eat).in_set(TurnSet::React))
            // Exactly as `main` wires input, or the test cannot see the bug.
            .add_systems(Update, (player_input, tend_lantern).in_set(EngineSet::Input));
        app.update();
        app.update();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        (app, player)
    }

    /// A key is only read while the player holds the turn, so if the run
    /// starts with nothing dealt, every key is dropped and the game looks
    /// frozen while it still draws.
    #[test]
    fn the_player_is_dealt_a_turn_and_no_screen_is_in_the_way() {
        let (app, player) = started(7);
        let modals = app.world().resource::<Modals>();
        assert!(!modals.any_open(), "a screen is open over the world: {modals:?}");
        assert!(app.world().get::<MyTurn>(player).is_some(), "the player was never dealt a turn");
        // `PlayerTurn` reads the bag, so a player with no bag matches nothing
        // and every key is dropped on the first line of `player_input`.
        assert!(app.world().get::<Inventory>(player).is_some(), "the player has no Inventory, so the input query cannot match it");
    }

    /// A key pressed through the real input path spends a turn. Tried in
    /// every direction, so a wall beside the start cannot pass for a
    /// dropped key.
    #[test]
    fn a_pressed_direction_key_is_read_and_spends_a_turn() {
        let (mut app, player) = started(7);
        let arrows = [KeyCode::ArrowRight, KeyCode::ArrowLeft, KeyCode::ArrowUp, KeyCode::ArrowDown];
        let mut moved = false;
        for key in arrows {
            let before = (app.world().get::<Position>(player).unwrap().0, app.world().resource::<Turns>().now());
            rl_engine::rl_bevy::testing::press(&mut app, key);
            for _ in 0..3 {
                app.update();
            }
            let after = (app.world().get::<Position>(player).unwrap().0, app.world().resource::<Turns>().now());
            if after != before {
                moved = true;
                break;
            }
        }
        assert!(moved, "no direction key moved the player or spent a turn: input never reached the game");
    }
}
