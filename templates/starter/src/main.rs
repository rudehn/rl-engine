//! {{project-name}}: a roguelike started from rl-engine's starter template.
//!
//! One floor of rooms, dark except for the torch you carry and the braziers
//! in its halls. Goblins wander it. They notice you when they can see you,
//! sooner when you stand in light, and hunt where they last saw you. Walk
//! into one to strike it.
//!
//! Everything is in this one file, in the order it happens: the screen, the
//! map, the start of a run, the keys, filling the floor, and the words in
//! the log. Change any of it; the tests at the bottom say what has to stay
//! true.
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk, and walk into a goblin
//! to attack it; `.` to wait; `t` to put your torch out or light it again;
//! `x` to look around; Escape for the menu; `q` to quit; `?` lists them all. Each is declared
//! once, in `declare_controls`, and read by name: that one list is what the
//! `?` screen shows and what the input system checks, so the two cannot
//! disagree.

use std::sync::Arc;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_rules::ai::tactics::{Hunt, MeleeAdjacent, SearchLastKnown, Wander};
use rl_engine::rl_rules::damage::{DamageKindId, SubtractArmor};
use rl_engine::rl_rules::faction::FactionDef;

/// The window's title.
const TITLE: &str = "{{project-name}}";
/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows along the bottom of the terminal given to the message log.
const LOG_ROWS: i32 = 5;
/// The one floor. Map zero is the streamed overworld, which this game has
/// none of.
const FLOOR: MapId = MapId(1);

/// How many braziers light the floor, and how many goblins walk it.
const BRAZIERS: usize = 5;
const GOBLINS: usize = 10;

/// The torch you carry: a flame to see by, and to be seen by.
const TORCH: LightSource = LightSource::new(210, 7, Rgb::new(255, 180, 110)).flickering(70);

fn main() -> AppExit {
    let mut app = App::new();
    // The window and glyph terminal, the turn loop, field of view, the map
    // between the status row and the log, and the base every panel needs.
    let map = Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS);
    app.add_plugins(RoguelikePlugins::new(TITLE, COLS, ROWS).map(map))
        // Every subsystem is a plugin you name. Combat resolves blows, minds
        // decide the monsters' turns, lighting decides what can be seen, and
        // stealth decides what has been noticed.
        .add_plugins((CombatPlugin, MindsPlugin, LightingPlugin, StealthPlugin))
        // The panels draw themselves from views the engine keeps current.
        // The controls screen lists every key declared below, and prints the
        // one hint that opens it at the right of the status row.
        // The engine narrates blows, deaths and what is picked up, naming
        // things in their own colours; the menu on Escape offers a new run,
        // the same seed again, or quitting, and opens by itself when you die;
        // the morgue writes each run down beside the executable.
        .add_plugins((
            NarratorPlugin::default(),
            GameMenuPanel::new(Rect::new(COLS / 2 - 20, 8, 40, 12))
                .died("The dark closes over you."),
        ))
        .insert_resource(Morgue::platform_default(TITLE, TITLE))
        .add_plugins((
            VitalsPanel::new(Rect::new(0, 0, COLS - 12, 1)),
            LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)),
            InspectPanel::new(Rect::new(2, ROWS - LOG_ROWS - 11, 50, 10)),
            ControlsPanel::new(Rect::new(4, 3, COLS - 8, ROWS - LOG_ROWS - 6)).hint(Rect::new(
                COLS - 12,
                0,
                12,
                1,
            )),
        ))
        // `--seed 7` replays a run; without it every run is new.
        .insert_resource(Seed::from_args())
        .add_systems(NewRun, start)
        // Keys become intents once a frame, and only while no screen, such as
        // the look cursor, has them.
        .add_systems(
            Update,
            player_input.in_set(EngineSet::Input).run_if(no_modal),
        )
        // Inside the turn: the floor fills the moment it is first entered.
        .add_systems(Turn, populate.in_set(TurnSet::React));
    // The keys, once, for the input system and the `?` screen alike.
    declare_controls(&mut app);
    app.run()
}

/// The floor's tiles, and how each looks in full light. The renderer works
/// out darkness and remembered tiles from these colours.
struct Dungeon {
    tiles: TileRegistry,
    seed: RunSeed,
}

impl Dungeon {
    fn new(seed: RunSeed) -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("stone")).unwrap();
        tiles.register(TileProps::floor("flagstone")).unwrap();
        Self { tiles, seed }
    }

    fn appearance(&self) -> TileAppearance {
        let mut look = TileAppearance::new();
        let t = |name| self.tiles.expect(name);
        look.set_varied(
            t("stone"),
            Cell::new('#', Color::srgb(0.70, 0.68, 0.64)).on(Color::srgb(0.28, 0.27, 0.26)),
            Vary::new(0.18, 0.04),
        );
        look.set_varied(
            t("flagstone"),
            Cell::new('.', Color::srgb(0.60, 0.58, 0.52)).on(Color::srgb(0.14, 0.13, 0.12)),
            Vary::new(0.25, 0.05),
        );
        look
    }
}

/// How the floor is built. The engine calls this the first time something
/// enters the map, and keeps what comes back.
impl PlaceRules for Dungeon {
    fn build(&self, _: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let (wall, floor) = (self.tiles.expect("stone"), self.tiles.expect("flagstone"));
        let mut ctx = BaseContext::blank(84, 42, self.tiles.clone(), wall);
        Chain::new()
            .then(dungeon::Rooms {
                floor,
                attempts: 60,
                min_size: 5,
                max_size: 11,
                min_rooms: 8,
            })
            .then(dungeon::RandomStart)
            .run(&mut ctx, self.seed)?;
        PlaceBuild::from_context(ctx)
    }
}

/// What every goblin shares: one brain, one side, one kind of blow.
#[derive(Resource)]
struct Goblins {
    mind: Arc<Brain<Entity>>,
    stab: DamageKindId,
    side: FactionId,
}

impl Goblins {
    /// One goblin, standing at `at`.
    fn goblin(&self, at: Point) -> impl Bundle + use<> {
        (
            (
                Actor,
                Blocks,
                Position(at),
                Name::new("goblin"),
                Faction(self.side),
            ),
            (
                Health::full(8),
                Armor(0),
                MeleeAttack::new(self.stab, DiceRoll::new(1, 4)),
                Perception(10),
                Mind(self.mind.clone()),
                // Sapient: it searches where it lost you, and opens doors.
                // `Wits::ANIMAL` would stop at a door, and `Wits::MINDLESS`
                // would forget you the moment it lost sight of you.
                Intelligence(Wits::SAPIENT),
            ),
            // It sees three steps without light, always notices you two
            // steps away (further while you stand in light), sometimes
            // beyond that, and remembers where you were for six turns.
            (
                DarkSight(3),
                Notice(NoticeStats {
                    certain: 2,
                    chance_pct: 20,
                    lit_bonus: 4,
                    memory: 6,
                }),
                Glyph::new('g', Color::srgb(0.55, 0.80, 0.35)).on_layer(5),
            ),
        )
    }
}

/// Hands the engine the rules, the map and the player, then sends the
/// player down.
fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    mut warps: MessageWriter<WarpRequest>,
    mut log: ResMut<MessageLog>,
    mut next: ResMut<NextState<EngineState>>,
) {
    let dungeon = Dungeon::new(seed.0);

    // What damage can be, and who is at war with whom. Both are registries
    // the game fills, so the engine never names a goblin.
    let kinds =
        Registry::from_defs(vec![DamageKind::new("stab"), DamageKind::new("slash")]).unwrap();
    let sides =
        Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("goblins")]).unwrap();
    let (you, goblins) = (sides.expect("you"), sides.expect("goblins"));
    commands.insert_resource(CombatRules::new(&sides).hostile(you, goblins));
    commands.insert_resource(Registries {
        damage_kinds: kinds.clone(),
        factions: sides,
        ..default()
    });
    // What a blow passes through on its way in; armor, here.
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    commands.insert_resource(Goblins {
        // Asked in order, and the first that answers wins: strike what is
        // adjacent, chase what it has noticed, search where it last saw you,
        // and otherwise wander.
        mind: Arc::new(
            Brain::new()
                .then(MeleeAdjacent)
                .then(Hunt)
                .then(SearchLastKnown)
                .then(Wander { chance_pct: 30 }),
        ),
        stab: kinds.expect("stab"),
        side: goblins,
    });

    commands.insert_resource(dungeon.appearance());
    commands.insert_resource(WorldMap::new(dungeon.tiles.tables()));
    commands.insert_resource(PlaceRulesRes(Box::new(dungeon)));

    let player = commands
        .spawn((
            (
                Actor,
                Player,
                Blocks,
                Position(Point::ZERO),
                Name::new("you"),
            ),
            (
                Viewshed::new(12),
                RevealsMap,
                Faction(you),
                Glyph::new('@', Color::WHITE).on_layer(10),
            ),
            (
                Health::full(30),
                Armor(1),
                MeleeAttack::new(kinds.expect("slash"), DiceRoll::new(1, 6)),
            ),
            // Lit, you see further and are noticed sooner.
            TORCH,
            // A goblin has to be a step closer to be sure of you, and rolls a
            // little lower beyond that.
            Stealth(StealthStats {
                quiet: 1,
                subtlety: 10,
            }),
        ))
        .id();
    warps.write(WarpRequest::into_place(player, FLOOR));
    log.push(
        format!(
            "Seed {}. You light your torch and climb down into the dark.",
            seed.0.0
        ),
        Tones::NOTICE,
        0,
    );
    next.set(EngineState::Playing);
}

/// What the player's keys can ask for.
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerIntents<'w> {
    bumps: MessageWriter<'w, Intent<Bump>>,
    waits: MessageWriter<'w, Intent<Wait>>,
}

/// The player, only while it holds the turn, and whether its torch is lit.
type PlayerTurn<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, Has<LightSource>), (With<Player>, With<MyTurn>)>;

/// Every key the game answers to, by name.
///
/// Declared once in [`declare_controls`], so the `?` screen and the input
/// system read the same list. To add a key, add a field here, declare it
/// there, and check it in [`player_input`].
#[derive(Resource, Clone, Copy)]
struct Binds {
    walk: ControlId,
    wait: ControlId,
    torch: ControlId,
    quit: ControlId,
}

/// Declares the keys, under the headings the `?` screen groups them by.
/// The engine adds its own alongside: the look cursor's, and `?` itself.
fn declare_controls(app: &mut App) {
    let binds = Binds {
        walk: app.add_control(
            "Move",
            "walk, or strike whoever is there",
            Keys::Directions { shift: false },
        ),
        wait: app.add_control("Act", "wait a turn", [KeyCode::Period, KeyCode::Numpad5]),
        torch: app.add_control("Act", "put the torch out, or light it", KeyCode::KeyT),
        quit: app.add_control("Game", "quit", KeyCode::KeyQ),
    };
    app.insert_resource(binds);
}

/// Keys to intents. Writing an intent is all it takes to act: the engine
/// spends the turn, and refuses what cannot be done.
fn player_input(
    keys: ControlInput,
    binds: Res<Binds>,
    player: PlayerTurn,
    mut intents: PlayerIntents,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(binds.quit) {
        exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move.
    let Ok((me, _, lit)) = player.single() else {
        return;
    };
    if let Some(dir) = keys.direction(binds.walk) {
        // A bump is a step, a blow at whoever hostile stands there, or the
        // door in the way opened: the engine decides which.
        intents.bumps.write(Intent::new(me, Bump(dir)));
    } else if keys.just_pressed(binds.torch) {
        // Dark, you are hidden from anything that needs light to see you,
        // and as blind as it is. Either way it takes the turn.
        if lit {
            commands.entity(me).remove::<LightSource>();
        } else {
            commands.entity(me).insert(TORCH);
        }
        intents.waits.write(Intent::new(me, Wait));
    } else if keys.just_pressed(binds.wait) {
        intents.waits.write(Intent::new(me, Wait));
    }
}

/// Braziers and goblins, the one time the floor is built.
fn populate(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    goblins: Res<Goblins>,
    map: Res<WorldMap>,
    seed: Res<Seed>,
) {
    for ev in entered.read() {
        // `first` is true only on the arrival that built the map, so coming
        // back would not restock it.
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else {
            continue;
        };
        let bounds = place.terrain.bounds();
        // A random stream of its own, by name: a spawner added later draws
        // from its own and cannot change what this one places.
        let mut rng = seed.stream(b"starter.populate", ev.map.0 as u64);
        let mut taken = vec![ev.entry];

        for _ in 0..BRAZIERS {
            let Some(at) = open_cell(&map, bounds, ev.entry, 6, &taken, &mut rng) else {
                break;
            };
            taken.push(at);
            // A fixture: it never moves, so its light is cast once.
            commands.spawn((
                Position(at),
                Name::new("a brazier"),
                LightSource::new(230, 6, Rgb::new(255, 150, 80)).flickering(110),
                Glyph::new('*', Color::srgb(1.0, 0.6, 0.25)).on_layer(1),
            ));
        }
        for _ in 0..GOBLINS {
            // Never close enough to strike on the first turn.
            let Some(at) = open_cell(&map, bounds, ev.entry, 10, &taken, &mut rng) else {
                break;
            };
            taken.push(at);
            commands.spawn(goblins.goblin(at));
        }
    }
}

/// A walkable cell at least `away` steps from `from` that nothing has taken,
/// or `None` when a fair number of tries finds none.
fn open_cell(
    map: &WorldMap,
    bounds: Rect,
    from: Point,
    away: i32,
    taken: &[Point],
    rng: &mut impl Rng,
) -> Option<Point> {
    (0..500)
        .map(|_| {
            Point::new(
                rng.random_range(bounds.x..bounds.right()),
                rng.random_range(bounds.y..bounds.bottom()),
            )
        })
        .find(|p| {
            map.is_walkable(*p) && geometry::chebyshev(*p, from) >= away && !taken.contains(p)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};

    /// The game with no window: what `main` adds, minus what draws.
    fn headless(seed: u64) -> App {
        let mut app = rl_engine::rl_bevy::plugin::headless_app();
        app.add_plugins((
            FovPlugin,
            CombatPlugin,
            MindsPlugin,
            LightingPlugin,
            StealthPlugin,
            KeyScriptPlugin,
        ))
        .add_plugins(UiPlugin)
        .insert_resource(Seed(RunSeed(seed)))
        .add_systems(NewRun, start)
        .add_systems(Update, player_input.in_set(EngineSet::Input))
        .add_systems(Turn, populate.in_set(TurnSet::React));
        declare_controls(&mut app);
        app
    }

    /// A run with the player holding its first turn: one frame builds the
    /// floor, the next deals the turn.
    fn started(seed: u64) -> (App, Entity) {
        let mut app = headless(seed);
        app.update();
        app.update();
        let player = app
            .world_mut()
            .query_filtered::<Entity, With<Player>>()
            .single(app.world())
            .unwrap();
        (app, player)
    }

    /// Over many seeds, because a generator that walls you in does it on one
    /// seed in fifty: you arrive somewhere you can stand, with room to move.
    #[test]
    fn every_seed_builds_a_floor_you_can_stand_on() {
        for seed in 0u64..48 {
            let dungeon = Dungeon::new(RunSeed(seed));
            let tables = dungeon.tiles.tables();
            let built = dungeon
                .build(FLOOR, None)
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            let walkable = |t: TileId| tables.walkable[t.index()];
            assert!(
                built.terrain.get(built.entry).is_some_and(walkable),
                "seed {seed}: you arrive inside a wall"
            );
            let open = built.terrain.iter().filter(|(_, t)| walkable(*t)).count();
            assert!(open > 200, "seed {seed}: only {open} open cells");
        }
    }

    /// The keys go through the real input system, as a keyboard would press
    /// them, and walking into a goblin is a blow.
    #[test]
    fn walking_into_a_goblin_strikes_it() {
        let (mut app, player) = started(7);
        let at = app.world().get::<Position>(player).unwrap().0;
        let arrows = [
            (KeyCode::ArrowRight, Direction::East),
            (KeyCode::ArrowLeft, Direction::West),
            (KeyCode::ArrowUp, Direction::North),
            (KeyCode::ArrowDown, Direction::South),
        ];
        let free = |app: &App, p: Point| {
            app.world().resource::<WorldMap>().is_walkable(p)
                && !app.world().resource::<Occupancy>().is_occupied(p)
        };
        let (key, dir) = arrows
            .into_iter()
            .find(|(_, dir)| free(&app, at + dir.offset()))
            .expect("room beside the player");
        let goblin = app.world().resource::<Goblins>().goblin(at + dir.offset());
        let goblin = app.world_mut().spawn(goblin).id();
        app.update();

        press(&mut app, key);
        assert!(
            app.world()
                .get::<Health>(goblin)
                .is_some_and(|h| h.current < h.max),
            "the goblin took the blow"
        );
        assert_eq!(
            app.world().get::<Position>(player).unwrap().0,
            at,
            "and the player stayed where it was"
        );
    }

    /// The torch is the player's light: out, the cell goes dark, and putting
    /// it out spends the turn.
    #[test]
    fn putting_the_torch_out_leaves_you_in_the_dark() {
        let (mut app, player) = started(7);
        let light_here = |app: &App| {
            app.world()
                .resource::<Lighting>()
                .at(app.world().get::<Position>(player).unwrap().0)
                .intensity
        };
        let (lit, clock) = (light_here(&app), app.world().resource::<Turns>().now());

        press(&mut app, KeyCode::KeyT);
        app.update();
        assert!(
            app.world().get::<LightSource>(player).is_none(),
            "the torch is out"
        );
        assert!(
            light_here(&app) < lit,
            "and the player stands in less light: {} then {}",
            lit,
            light_here(&app)
        );
        assert!(
            app.world().resource::<Turns>().now() > clock,
            "which took the turn"
        );
    }

    /// Whether each actor dealt a turn was the player, in the order dealt,
    /// read inside the pass that dealt it: a reader in `Update` would miss
    /// every turn dealt and spent within one frame.
    #[derive(Resource, Default)]
    struct Dealt(Vec<bool>);

    fn record_deals(mut dealt: ResMut<Dealt>, fresh: Query<Has<Player>, Added<MyTurn>>) {
        dealt.0.extend(fresh.iter());
    }

    #[test]
    fn on_a_fresh_run_the_player_is_dealt_the_first_turn_over_a_span_of_seeds() {
        for seed in 0..8u64 {
            let mut app = headless(seed);
            app.init_resource::<Dealt>()
                .add_systems(Turn, record_deals.in_set(TurnSet::Decide));
            app.update();
            app.update();
            let dealt = &app.world().resource::<Dealt>().0;
            assert_eq!(
                dealt.first(),
                Some(&true),
                "seed {seed}: {dealt:?} dealt someone else the first turn before the player"
            );
        }
    }
}
