//! Warren, step 6: two floors, and a way out of the second.
//!
//! The guide chapter is `docs/guide/src/06-two-floors.md`. A floor is a
//! *place*: built the first time it is entered and kept whole after that.
//! The floors are a ladder the rules describe, and the engine lays the
//! stairs between them, entities with a [`Transition`]; the way out at the
//! bottom is an entity too: stand on it and the run is won.
//!
//! `cargo run -p tutorial --bin step06_descent`
//!
//! Keys: arrows to walk or strike, `>` to take stairs, `a` screech,
//! `g` get, `e` eat, `r` throw, `t` lantern, `.` wait, `q` quit.

use atglyph::prelude::*;
use atglyph::primitives::Rect;
use atglyph::rand::Rng;
use atglyph::rules::AbilityId;
use atglyph::rules::TagDef;
use atglyph::rules::Wits;
use atglyph::rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use atglyph::rules::ai::tactics::{Scavenge, ThrowAtRange, UseAbility};
use atglyph::rules::faction::FactionDef;
use bevy::prelude::*;
use std::sync::Arc;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows at the bottom of the terminal given over to the message log.
const LOG_ROWS: i32 = 5;

// ANCHOR: floors
/// How deep the warren goes before it lets you out again.
const FLOORS: u32 = 2;

/// The warren's floors are maps one upward, so a floor's number is its
/// map's.
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
        _ => "the Deep Nest",
    }
}
// ANCHOR_END: floors

// ANCHOR: main
fn main() -> AppExit {
    let mut app = App::new();
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map in everything but the status row and the log, and the UI base.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        // Without this the world is lit everywhere and sight is geometry
        // alone. With it, `visible` shrinks to what a light reaches.
        .add_plugins((CombatPlugin, MindsPlugin, LightingPlugin, ItemsPlugin, ThrowingPlugin, AbilitiesPlugin, TargetViewPlugin))
        .insert_resource(Lighting::dark())
        // The effects an ability may name. The engine ships seven; a game
        // adds its own beside them with `add_effect`.
        .add_engine_effects()
        .insert_resource(Seed(RunSeed(7)))
        // Two panels: the vitals strip on the top row, the log along the
        // bottom. Each draws itself; neither needs a system of yours.
        .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)).hints("[>]stairs [a]screech [g]et [e]at [r]ock [t]orch [.]wait [q]uit"))
        .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
        // The engine narrates blows, deaths and pickups into the log, naming
        // things in their own colours. Warren changes one phrase: what a rat
        // does to you is a bite.
        .add_plugins(NarratorPlugin::default().phrase(Phrase::HitsYou, "{Who} bites you for {n}.", Tones::BAD))
        // Escape opens the menu, and the run's end opens it by itself.
        .add_plugins(GameMenuPanel::new(Rect::new(COLS / 2 - 20, 8, 40, 12)).died("The warren keeps you."))
        .add_systems(NewRun, start)
        // A floor fills the first time it is entered, inside the turn.
        .add_systems(Turn, (populate, eat, leave).in_set(TurnSet::React))
        .add_systems(Update, note_bag.in_set(ViewSet::Annotate))
        .add_systems(Update, narrate.in_set(PresentSet::Narrate))
        // Once a frame, before the turns: whatever the player pressed of
        // Warren's own keys becomes at most one intent, beside the standard
        // keys, however many passes the turn loop then runs.
        .add_systems(Update, (player_input, tend_lantern).in_set(EngineSet::Input))
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
    fn build(&self, map: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
        let depth = floor_of(map);
        let (wall, open, roots) = (self.tiles.expect("earth"), self.tiles.expect("dirt"), self.tiles.expect("roots"));
        let mut ctx = BaseContext::blank(84, 42, self.tiles.clone(), wall);
        // `seed` is this floor's own, handed in by the engine, so the second
        // floor is the same whether or not you dawdled on the first.
        let chain = match depth {
            // Dug rooms near the surface.
            1 => Chain::new().then(dungeon::Rooms { floor: open, attempts: 40, min_size: 5, max_size: 10, min_rooms: 6 }).then(dungeon::Doors { door: roots }),
            // Gnawed-out caves under them.
            _ => Chain::new().then(passes::CellularCave { wall, floor: open, fill_pct: 45, ..Default::default() }).then(passes::KeepLargestRegion { wall }),
        };
        // Every floor gets a start and a point as far from it as the floor
        // allows: the stairs down on the first, the way out on the second.
        chain.then(dungeon::RandomStart).then(dungeon::FarthestExit).run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }

    fn tiles(&self) -> TileTables {
        self.tiles.tables()
    }

    /// The floors, top first, and how their stairs look. The engine lays a
    /// stair up at every floor's start but the first's and a stair down
    /// at every floor's far point but the last's, when the floor is built.
    /// The last floor's far point is ours: the way out.
    fn ladder(&self) -> Option<Ladder> {
        let stone = Rgb::new(204, 204, 219);
        let (up, down) = (StairLook::new('<', stone, "stairs up"), StairLook::new('>', stone, "stairs down"));
        Some(Ladder::new((1..=FLOORS).map(map_of), up, down).bottom(LadderEnd::Game))
    }
}
// ANCHOR_END: rules

// ANCHOR: start
/// Hands the engine what combat and the knacks read, then spawns the
/// player into the warren, at the first floor's entry.
fn start(mut commands: Commands, seed: Res<Seed>, effect_kinds: Res<EffectKinds>, mut log: Log) {
    // The two registries combat reads: what damage can be, and who hates
    // whom. Both are the game's content, named nowhere in the engine.
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("kick"), DamageKind::new("din")]).unwrap();
    let sides = Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("vermin")]).unwrap();
    let (you, vermin) = (sides.expect("you"), sides.expect("vermin"));
    commands.insert_resource(CombatRules::new().hostile(you, vermin));
    let tags = Registry::from_defs(vec![TagDef::new("bread")]).unwrap();
    let bread = tags.expect("bread");
    let registries = Registries { damage_kinds: kinds.clone(), factions: sides, tags, ..default() };
    // Loaded against the registries and the effects registered when the app
    // was built, so a file naming either wrongly fails here and not later.
    let knacks = Abilities::load(KNACKS_RON, &effect_kinds, &registries.names()).unwrap_or_else(|e| panic!("assets/knacks.ron: {e}"));
    let screech = knacks.defs().expect("screech");
    let gnaw = knacks.defs().expect("gnaw");
    commands.insert_resource(Knacks { screech });
    commands.insert_resource(knacks);
    commands.insert_resource(registries);
    commands.insert_resource(Rats {
        bread,
        gnaw,
        // A ratling has hands and a grudge: it fetches a rock and throws it,
        // and leaves an enemy at its elbow to its teeth. A rat has neither.
        ratling: Arc::new(
            Brain::new()
                .then(MeleeAdjacent)
                .then(FleeWhenHurt { at_pct: 25 })
                // Scored by what the aim would land on, so a ratling at full
                // health gains nothing by eating and does not bother.
                .then(UseAbility { chance_pct: 80 })
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

    commands.spawn_player(
        map_of(1),
        (
            (Viewshed::new(9), RevealsMap, LANTERN, Faction(you), Grants(vec![screech]), Glyph::new('@', Color::WHITE).on_layer(10)),
            (Health::full(24), Armor(1), MeleeAttack::new(kinds.expect("kick"), DiceRoll::new(1, 6)), Inventory::default()),
        ),
    );
    log.push(format!("Seed {}. You squeeze into the warren.", seed.0.0), Tones::NOTICE);
    log.push("Something is scratching in the dark.", Tones::MUTED);
}
// ANCHOR_END: start

// ANCHOR: input
/// The player, but only while it is holding the turn, and what it carries.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, Option<&'static Inventory>), (With<Player>, With<MyTurn>)>;

/// The standard keys. Walking, the stairs, picking up and waiting are the
/// same in every roguelike, so they are the engine's `PlayerKeysPlugin`. A
/// walk key writes a [`Bump`], which the engine resolves to a step, a blow
/// at a foe, or opening a door, whichever is in the way; Enter, `>` or `<`
/// take the stairs underfoot.
fn declare_controls(app: &mut App) {
    app.add_plugins(
        PlayerKeysPlugin::new()
            .walk("Move", "walk, or strike whoever is there")
            .go_through("Move", "take the stairs")
            .pick_up("Act", "pick up what is here")
            .wait("Act", "wait a turn"),
    );
}

/// Warren's own keys to intents. Writing an intent is the whole of asking
/// to act: the engine claims the turn, charges it, and refuses what cannot
/// be done.
///
/// `r` writes no intent at all: it opens the engine's aiming cursor, and
/// the throw is written when the cursor is committed.
fn player_input(keys: Res<ButtonInput<KeyCode>>, player: PlayerTurn, carried: Carried, mut intents: PlayerIntents, knacks: Res<Knacks>) {
    if keys.just_pressed(KeyCode::KeyQ) {
        intents.exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move; the key is dropped.
    let Ok((entity, bag)) = player.single() else { return };
    if keys.just_pressed(KeyCode::KeyE) {
        if let Some(crust) = bag.into_iter().flat_map(|b| b.items.iter().copied()).find(|i| carried.crusts.contains(*i)) {
            intents.uses.write(Intent::new(entity, UseItem(crust)));
        }
    } else if keys.just_pressed(KeyCode::KeyA) {
        // The key writes an aim and stops. The cursor, the preview of what
        // the burst would cover, and the spending of the turn are engine.
        intents.aim_ability.write(AimAt { user: entity, ability: knacks.screech });
    } else if keys.just_pressed(KeyCode::KeyR)
        && let Some(rock) = bag.into_iter().flat_map(|b| b.items.iter().copied()).find(|i| carried.rocks.contains(*i))
    {
        intents.aims.write(AimThrow { user: entity, item: rock });
    }
}
// ANCHOR_END: input

// ANCHOR: status
/// The one thing the vitals panel cannot know: how much of the map is
/// ours. A note on the view, in the game's own words.
fn note_explored(mut vitals: ResMut<VitalsView>, knowledge: Res<Knowledge>) {
    vitals.facets.push(Facet::new("explored", format!("{} tiles explored", knowledge.explored_count())));
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
fn tend_lantern(keys: Res<ButtonInput<KeyCode>>, mut commands: Commands, player: Lantern, mut waits: MessageWriter<Intent<Wait>>, mut log: Log) {
    if !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    let Ok((entity, lit)) = player.single() else { return };
    if lit {
        commands.entity(entity).remove::<LightSource>();
        log.muted("You shade the lantern. The warren closes to arm's length.");
    } else {
        commands.entity(entity).insert(LANTERN);
        log.notice("You open the lantern. The dirt comes up warm around you.");
    }
    waits.write(Intent::new(entity, Wait));
}
// ANCHOR_END: lantern

// ANCHOR: creatures
/// What every rat in the warren shares: one brain, one faction, one bite.
#[derive(Resource)]
struct Rats {
    mind: Arc<Brain<Entity>>,
    /// The tag that says a thing is bread, and the knack that eats it.
    bread: atglyph::rules::TagId,
    gnaw: AbilityId,
    /// The brain of something with hands.
    ratling: Arc<Brain<Entity>>,
    bite: atglyph::rules::damage::DamageKindId,
    faction: FactionId,
}
// ANCHOR_END: creatures

// ANCHOR: populate
/// The stream the rats are placed from, named once and drawn from each
/// floor's own seed: a spawner added later draws from a stream of its own
/// and cannot shift what this one places, and the floors may be entered in
/// any order.
const RATS: SeedDomain = SeedDomain::new(b"warren.rats");

/// Fills the floor the one time it is built. `PlaceEntered::first` is
/// true only on that arrival, so coming back does not restock it.
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
            // Not on top of the player, and not close enough to be unfair.
            if !map.is_walkable(p) || geometry::chebyshev(p, ev.entry) < 8 {
                continue;
            }
            // Every fourth is a ratling: the same body, a better mind.
            let clever = placed % 4 == 3;
            // A ratling carries its own supper. Nothing picks bread up off
            // the floor: `Scavenge` fetches gear and missiles, not meals.
            let supper = clever.then(|| {
                commands
                    .spawn((Item, Crust(6), Tagged(vec![rats.bread]), Name::new("crust of bread"), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)))
                    .id()
            });
            let mut e = commands.spawn((
                (Actor, Blocks, Position(p), Speed(110), Faction(rats.faction)),
                (Health::full(6), Armor(0), Perception(7), DarkSight(9)),
                (MeleeAttack::new(rats.bite, DiceRoll::new(1, 3)),),
            ));
            if clever {
                e.insert((
                    Mind(rats.ratling.clone()),
                    // Wits are what a mind is allowed to consider. A ratling
                    // opens doors, fetches what it can throw, and throws it.
                    Intelligence(Wits::SAPIENT),
                    Inventory { items: supper.into_iter().collect() },
                    Grants(vec![rats.gnaw]),
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
        // The stairs are the engine's, laid from the ladder. The bottom of
        // the ladder is ours: the way out, at the last floor's far point,
        // an entity standing on a cell and nothing more.
        if let Some(far) = ev.exit.filter(|_| floor_of(ev.map) == FLOORS) {
            commands.spawn((Position(far), WayOut, Name::new("crack of daylight"), Glyph::new('<', Color::srgb(1.0, 0.95, 0.70)).on_layer(1)));
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
    uses: MessageWriter<'w, Intent<UseItem>>,
    aims: MessageWriter<'w, AimThrow>,
    aim_ability: MessageWriter<'w, AimAt>,
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
fn narrate(mut items: MessageReader<ItemEvent>, mut log: Log) {
    for ev in items.read() {
        if let ItemEvent::Used { .. } = *ev {
            log.push("You eat the crust. It helps.", Tones::GOOD);
        }
    }
}
// ANCHOR_END: narrate

// ANCHOR: status
/// What the engine cannot know: how many crusts are in the bag.
fn note_bag(mut vitals: ResMut<VitalsView>, player: Query<(&Inventory, Option<&OnMap>), With<Player>>) {
    let Ok((bag, on)) = player.single() else { return };
    vitals.facets.push(Facet::new("crusts", format!("crusts {}", bag.items.len())));
    // Which floor you are on is the game's to know: the engine has map ids
    // and no names for them.
    if let Some(depth) = on.map(|m| floor_of(m.0)) {
        vitals.facets.push(Facet::new("floor", format!("floor {depth}/{FLOORS}: {}", name_of(depth))));
    }
}
// ANCHOR_END: status

// ANCHOR: litter
/// What the floor is littered with: bread that mends, rocks that fly.
fn litter(commands: &mut Commands, rats: &Rats, p: Point, bread: bool) {
    if bread {
        commands.spawn((Item, Crust(8), Name::new("crust of bread"), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
    } else {
        commands.spawn((
            Item,
            Throwable::new(7, Some((rats.bite, DiceRoll::new(1, 4)))),
            Name::new("rock"),
            Position(p),
            Glyph::new('*', Color::srgb(0.66, 0.66, 0.70)).on_layer(2),
        ));
    }
}
// ANCHOR_END: litter

// ANCHOR: knack
/// The knacks this game loads, and the one the player is given.
const KNACKS_RON: &str = include_str!("../../assets/knacks.ron");

/// The ability ids the game holds on to, so a key can name one.
#[derive(Resource)]
struct Knacks {
    screech: AbilityId,
}
// ANCHOR_END: knack

// ANCHOR: way_out
/// The crack of daylight on the last floor. Standing on it ends the run.
#[derive(Component, Clone, Copy)]
struct WayOut;

/// Winning is a component and an `if`. It runs inside the turn, so the run
/// ends on the step that reached the daylight and not a frame later.
fn leave(player: Query<(&Position, Option<&OnMap>), With<Player>>, ways: Query<(&Position, Option<&OnMap>), With<WayOut>>, mut over: MessageWriter<RunOver>) {
    let Ok((at, on)) = player.single() else { return };
    for (way, way_on) in &ways {
        if way.0 == at.0 && way_on.map(|m| m.0) == on.map(|m| m.0) {
            over.write(RunOver::won().saying("You come up into the roots, and the scratching stays below."));
        }
    }
}
// ANCHOR_END: way_out

#[cfg(test)]
mod tests {
    use super::*;

    /// The warren with no window, wired as `main` wires it, input included.
    fn started(seed: u64) -> (App, Entity) {
        let mut app = atglyph::testing::windowless(RunSeed(seed));
        app.add_plugins((CombatPlugin, MindsPlugin, LightingPlugin, ItemsPlugin, ThrowingPlugin, AbilitiesPlugin));
        app.add_plugins(UiPlugin).add_plugins(TargetViewPlugin).add_plugins(atglyph::testing::KeyScriptPlugin);
        app.add_engine_effects();
        app.insert_resource(Lighting::dark())
            .add_systems(NewRun, start)
            .add_systems(Turn, (populate, eat, leave).in_set(TurnSet::React))
            .add_systems(Update, (player_input, tend_lantern).in_set(EngineSet::Input))
            .places(Warren::new());
        declare_controls(&mut app);
        app.update();
        app.update();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        (app, player)
    }

    /// A key pressed through the real input path spends a turn. Tried in
    /// every direction, so a wall beside the start cannot pass for a
    /// dropped key.
    #[test]
    fn a_pressed_direction_key_is_read_and_spends_a_turn() {
        let (mut app, player) = started(7);
        assert!(app.world().get::<Inventory>(player).is_some(), "the player carries a bag");
        let mut moved = false;
        for key in [KeyCode::ArrowRight, KeyCode::ArrowLeft, KeyCode::ArrowUp, KeyCode::ArrowDown] {
            let before = (app.world().get::<Position>(player).unwrap().0, app.world().resource::<Turns>().now());
            atglyph::testing::press(&mut app, key);
            for _ in 0..3 {
                app.update();
            }
            if (app.world().get::<Position>(player).unwrap().0, app.world().resource::<Turns>().now()) != before {
                moved = true;
                break;
            }
        }
        assert!(moved, "no direction key moved the player or spent a turn: input never reached the game");
    }

    /// Over a span of seeds, every floor has one way up, but the first,
    /// where the run begins, and one way down, but the last, each a stair
    /// the engine laid to the floor beside it; the last floor's far point
    /// is the crack of daylight instead, which is ours, and nothing else
    /// stands there.
    #[test]
    fn every_floor_has_a_stair_to_each_floor_beside_it_and_the_last_its_daylight() {
        for seed in 1u64..=8 {
            let (mut app, player) = started(seed);
            for depth in 1..=FLOORS {
                app.world_mut().write_message(WarpRequest::into_place(player, map_of(depth)));
                app.update();
                app.update();
                let (entry, exit) = {
                    let place = app.world().resource::<WorldMap>().place(map_of(depth)).expect("built");
                    (place.entry, place.exit.expect("a far point"))
                };
                let world = app.world_mut();
                let mut q = world.query::<(&Position, &OnMap, &Transition, &Stair)>();
                let mut stairs: Vec<(Point, Destination, Climb)> =
                    q.iter(world).filter(|(_, on, _, _)| on.0 == map_of(depth)).map(|(p, _, t, s)| (p.0, t.to, s.climb)).collect();
                stairs.sort_by_key(|(p, _, c)| (*p, *c));
                let mut expected = Vec::new();
                if depth > 1 {
                    expected.push((entry, Destination::Place { map: map_of(depth - 1), arrive: Arrive::Exit }, Climb::Up));
                }
                if depth < FLOORS {
                    expected.push((exit, Destination::Place { map: map_of(depth + 1), arrive: Arrive::Entry }, Climb::Down));
                }
                expected.sort_by_key(|(p, _, c)| (*p, *c));
                assert_eq!(stairs, expected, "seed {seed}, floor {depth}");
                let daylight: Vec<Point> =
                    world.query_filtered::<(&Position, &OnMap), With<WayOut>>().iter(world).filter(|(_, on)| on.0 == map_of(depth)).map(|(p, _)| p.0).collect();
                let want: Vec<Point> = if depth == FLOORS { vec![exit] } else { Vec::new() };
                assert_eq!(daylight, want, "seed {seed}, floor {depth}: the daylight is the last floor's far point, and nowhere else");
            }
        }
    }
}
