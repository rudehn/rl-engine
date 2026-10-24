//! {{project-name}}: a roguelike started from atglyph's starter template.
//!
//! One floor of rooms, dark except for the torch you carry and the braziers
//! in its halls. Goblins wander it. They notice you when they can see you,
//! sooner when you stand in light, and hunt where they last saw you. Walk
//! into one to strike it. Potions lie about the floor, and a dagger and a
//! leather coat, to pick up, drink and wear.
//!
//! Everything is in this one file, in the order it happens: the screen, the
//! map, the start of a run, the keys, filling the floor, and what a potion
//! does. Change any of it; the tests at the bottom say what has to stay
//! true.
//!
//! `cargo run` opens a window; `cargo run -- --tty` plays in the terminal.
//!
//! Keys: arrows, `hjklyubn` or the numpad to walk, and walk into a goblin
//! to attack it; `.` to wait; `g` to pick up what is here, `d` to drink a
//! potion and `w` to put on or wield what you carry; `t` to put your torch out
//! or light it again; `x` to look around; Escape for the menu; `q` to quit;
//! `?` lists them all. Each is declared once, in `declare_controls`, and
//! read by name: that one list is what the `?` screen shows and what the
//! input system checks, so the two cannot disagree.

use std::sync::Arc;

use atglyph::prelude::*;
use atglyph::prelude::{items::*, minds::*, ui::*};
use atglyph::rand::Rng;
use atglyph::rules::ai::tactics::{Hunt, MeleeAdjacent, SearchLastKnown, Wander};
use atglyph::rules::damage::DamageKindId;
use atglyph::rules::faction::FactionDef;
use bevy::prelude::*;

/// The window's title.
const TITLE: &str = "{{project-name}}";
/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows along the bottom of the terminal given to the message log.
const LOG_ROWS: i32 = 5;
/// The one floor.
const FLOOR: MapId = MapId(1);

/// How many braziers light the floor, and how many goblins walk it.
const BRAZIERS: usize = 5;
const GOBLINS: usize = 10;
/// How many potions lie on the floor, and how much health each mends.
const POTIONS: usize = 3;
const POTION_HEALS: i32 = 6;

/// The random stream a floor is stocked from, by name, drawn from the
/// floor's own seed: a spawner added later draws from a stream of its own
/// and cannot change what this one places.
const POPULATE: SeedDomain = SeedDomain::new(b"starter.populate");

/// The torch you carry: a flame to see by, and to be seen by.
const TORCH: LightSource = LightSource::new(210, 7, Rgb::new(255, 180, 110)).flickering(70);

fn main() -> AppExit {
    let mut app = App::new();
    // ANCHOR: front_end
    // The window and glyph terminal, the turn loop, field of view, the map
    // between the status row and the log, and the base every panel needs;
    // or, with `--tty`, all of it in the text terminal you started the game
    // in, which plays over SSH too.
    let map = GridRect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS);
    if std::env::args().any(|arg| arg == "--tty") {
        app.add_plugins(TtyRoguelikePlugins::new(COLS, ROWS).map(map));
    } else {
        app.add_plugins(RoguelikePlugins::new(TITLE, COLS, ROWS).map(map));
    }
    // ANCHOR_END: front_end
    // The game itself, the same plugin the tests add.
    app.add_plugins(Starter)
        // The panels draw themselves from views the engine keeps current.
        // The controls screen lists every key declared below, and prints the
        // one hint that opens it at the right of the status row.
        // The engine narrates blows, deaths and what is picked up, naming
        // things in their own colours; the menu on Escape offers a new run,
        // the same seed again, or quitting, and opens by itself when you die.
        .add_plugins((
            NarratorPlugin::default(),
            GameMenuPanel::new(GridRect::new(COLS / 2 - 20, 8, 40, 12))
                .died("The dark closes over you."),
        ))
        .add_plugins((
            // The status row and the log keep a cell of margin at either end.
            VitalsPanel::new(GridRect::new(1, 0, COLS - 14, 1)),
            LogPanel::new(GridRect::new(1, ROWS - LOG_ROWS, COLS - 2, LOG_ROWS)),
            InspectPanel::new(GridRect::new(2, ROWS - LOG_ROWS - 11, 50, 10)),
            ControlsPanel::new(GridRect::new(4, 3, COLS - 8, ROWS - LOG_ROWS - 6))
                .hint(GridRect::new(COLS - 13, 0, 12, 1)),
        ))
        // `--seed 7` replays a run; without it every run is new.
        .insert_resource(Seed::from_args());
    app.run()
}

/// The game: its subsystems, its floor, its keys and its turns. What is
/// drawn is added in `main`; the tests add this alone, so they play the
/// game the player plays.
struct Starter;

impl Plugin for Starter {
    fn build(&self, app: &mut App) {
        let dungeon = Dungeon::new();
        // Every subsystem is a plugin you name. Combat resolves blows, minds
        // decide the monsters' turns, lighting decides what can be seen, and
        // stealth decides what has been noticed.
        app.add_plugins((CombatPlugin, MindsPlugin, LightingPlugin, StealthPlugin))
            // Items lie on the floor, in a bag, or worn, and the engine moves
            // them between the three and charges a turn for each move.
            .add_plugins(ItemsPlugin)
            // How the floor looks, and how it is built: the engine builds it
            // the first time something enters it, from its own seed.
            .insert_resource(dungeon.appearance())
            .places(dungeon)
            .add_systems(NewRun, start)
            // Keys become intents once a frame, and only while no screen,
            // such as the look cursor, has them.
            .add_systems(
                Update,
                player_input.in_set(EngineSet::Input).run_if(no_modal),
            )
            // Inside the turn: the floor fills the moment it is first entered,
            // and a potion mends the moment it is drunk.
            .add_systems(Turn, (populate, drink).in_set(TurnSet::React));
        // The keys, once, for the input system and the `?` screen alike.
        declare_controls(app);
    }
}

/// The floor's tiles, and how each looks in full light. The renderer works
/// out darkness and remembered tiles from these colours.
struct Dungeon {
    tiles: TileRegistry,
}

impl Dungeon {
    fn new() -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("stone")).unwrap();
        tiles.register(TileProps::floor("flagstone")).unwrap();
        Self { tiles }
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
    fn build(&self, _: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
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
            .run(&mut ctx, seed)?;
        PlaceBuild::from_context(ctx)
    }

    fn tiles(&self) -> TileTables {
        self.tiles.tables()
    }
}

/// What every goblin shares: one brain, one side, one kind of blow.
#[derive(Resource)]
struct Goblins {
    mind: Arc<Brain<Entity>>,
    stab: DamageKindId,
    side: FactionId,
}

// ANCHOR: goblin
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
// ANCHOR_END: goblin

/// What lies about the floor to be found: potions, a dagger and a coat.
/// An item is an entity like any other: on the floor it has a position,
/// and picked up it is listed in a bag instead.
#[derive(Resource)]
struct Finds {
    stab: DamageKindId,
    main_hand: SlotId,
    body: SlotId,
}

impl Finds {
    /// A potion. The engine only knows that it was used; what drinking it
    /// does is [`drink`], further down.
    fn potion(&self) -> impl Bundle + use<> {
        (
            Item,
            Potion(POTION_HEALS),
            Name::new("potion"),
            Glyph::new('!', Color::srgb(0.90, 0.35, 0.50)).on_layer(2),
        )
    }

    /// A dagger. Wielded, its blow is the one you strike with, in place of
    /// your fists.
    fn dagger(&self) -> impl Bundle + use<> {
        (
            (Item, Name::new("dagger")),
            (
                Wearable(EquipShape::in_slot(self.main_hand)),
                MeleeAttack::new(self.stab, DiceRoll::new(2, 4)),
                // What it is worth beside other gear, to a monster that
                // carries things: a goblin here carries nothing.
                GearScore(3),
            ),
            Glyph::new(')', Color::srgb(0.75, 0.78, 0.85)).on_layer(2),
        )
    }

    /// A leather coat. Worn, its armor is added to yours on every blow.
    fn coat(&self) -> impl Bundle + use<> {
        (
            (Item, Name::new("leather coat")),
            (
                Wearable(EquipShape::in_slot(self.body)),
                Armor(2),
                GearScore(2),
            ),
            Glyph::new('[', Color::srgb(0.70, 0.50, 0.30)).on_layer(2),
        )
    }
}

/// Hands the engine the rules, then sends the player down to the floor's
/// entry.
fn start(mut commands: Commands, seed: Res<Seed>, mut log: Log) {
    // What damage can be, who is at war with whom, and where things are
    // worn. All are registries the game fills, so the engine never names a
    // goblin or a coat.
    let kinds =
        Registry::from_defs(vec![DamageKind::new("stab"), DamageKind::new("punch")]).unwrap();
    let sides =
        Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("goblins")]).unwrap();
    let slots = Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("body")]).unwrap();
    let (you, goblins) = (sides.expect("you"), sides.expect("goblins"));
    commands.insert_resource(CombatRules::new().hostile(you, goblins));
    commands.insert_resource(Registries {
        damage_kinds: kinds.clone(),
        factions: sides,
        slots: slots.clone(),
        ..default()
    });
    commands.insert_resource(Finds {
        stab: kinds.expect("stab"),
        main_hand: slots.expect("main hand"),
        body: slots.expect("body"),
    });
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

    commands.spawn_player(
        FLOOR,
        (
            Name::new("you"),
            (
                Viewshed::new(12),
                RevealsMap,
                Faction(you),
                Glyph::new('@', Color::WHITE).on_layer(10),
            ),
            (
                Health::full(30),
                Armor(1),
                // Your fists, struck with while you wield nothing.
                MeleeAttack::new(kinds.expect("punch"), DiceRoll::new(1, 6)),
            ),
            // A bag to carry things in, and a slot for each place to wear one.
            (Inventory::default(), Equipped(Equipment::for_slots(&slots))),
            // Lit, you see further and are noticed sooner.
            TORCH,
            // A goblin has to be a step closer to be sure of you, and rolls a
            // little lower beyond that.
            Stealth(StealthStats {
                quiet: 1,
                subtlety: 10,
            }),
        ),
    );
    log.push(
        format!(
            "Seed {}. You light your torch and climb down into the dark.",
            seed.0.0
        ),
        Tones::NOTICE,
    );
}

/// The player, only while it holds the turn: whether its torch is lit,
/// what it carries, what it wears, and how hurt it is.
type PlayerTurn<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Has<LightSource>,
        &'static Inventory,
        &'static Equipped,
        &'static Health,
    ),
    (With<Player>, With<MyTurn>),
>;

/// The game's own keys, by name.
///
/// Declared once in [`declare_controls`], so the `?` screen and the input
/// system read the same list. To add a key, add a field here, declare it
/// there, and check it in [`player_input`].
#[derive(Resource, Clone, Copy)]
struct Binds {
    torch: ControlId,
    drink: ControlId,
    wear: ControlId,
    quit: ControlId,
}

/// Declares the keys, under the headings the `?` screen groups them by.
///
/// Walking, picking up and waiting are the same in every roguelike, so they
/// are the engine's `PlayerKeysPlugin`, named here in the game's words: a
/// walk is a step, a blow at whoever hostile stands there, or the door in
/// the way opened, and the engine decides which. The engine adds its own
/// keys alongside: the look cursor's, and `?` itself.
fn declare_controls(app: &mut App) {
    app.add_plugins(
        PlayerKeysPlugin::new()
            .walk("Move", "walk, or strike whoever is there")
            .pick_up("Act", "pick up what is here")
            .wait("Act", "wait a turn"),
    );
    let binds = Binds {
        drink: app.add_control("Act", "drink a potion", KeyCode::KeyD),
        wear: app.add_control("Act", "put on or wield what you carry", KeyCode::KeyW),
        torch: app.add_control("Act", "put the torch out, or light it", KeyCode::KeyT),
        quit: app.add_control("Game", "quit", KeyCode::KeyQ),
    };
    app.insert_resource(binds);
}

/// Every intent the game's own keys can write, as one parameter, so the
/// input system has room for as many keys as the game grows.
#[derive(bevy::ecs::system::SystemParam)]
struct Intents<'w> {
    waits: MessageWriter<'w, Intent<Wait>>,
    uses: MessageWriter<'w, Intent<UseItem>>,
    equips: MessageWriter<'w, Intent<Equip>>,
    exit: MessageWriter<'w, AppExit>,
}

/// The game's own keys to intents. Writing an intent is all it takes to
/// act: the engine spends the turn, and refuses what cannot be done.
fn player_input(
    keys: ControlInput,
    binds: Res<Binds>,
    player: PlayerTurn,
    things: Query<(Has<Potion>, Has<Wearable>)>,
    mut intents: Intents,
    mut commands: Commands,
    mut log: Log,
) {
    if keys.just_pressed(binds.quit) {
        intents.exit.write(AppExit::Success);
        return;
    }
    // No turn in hand means it is somebody else's move.
    let Ok((me, lit, bag, worn, health)) = player.single() else {
        return;
    };
    if keys.just_pressed(binds.torch) {
        // Dark, you are hidden from anything that needs light to see you,
        // and as blind as it is. Either way it takes the turn.
        if lit {
            commands.entity(me).remove::<LightSource>();
        } else {
            commands.entity(me).insert(TORCH);
        }
        intents.waits.write(Intent::new(me, Wait));
    } else if keys.just_pressed(binds.drink) {
        // The first potion in the bag. The engine spends the turn and
        // reports the use, and `drink` says what it does.
        let potion = bag
            .items
            .iter()
            .find(|i| things.get(**i).is_ok_and(|(p, _)| p));
        // Refused here rather than drunk for nothing: a potion is too dear
        // to spend on no wound, and a refusal spends no turn.
        match potion {
            None => log.muted("You have no potion to drink."),
            Some(_) if health.current >= health.max => log.muted("You are not hurt."),
            Some(&potion) => {
                intents.uses.write(Intent::new(me, UseItem(potion)));
            }
        }
    } else if keys.just_pressed(binds.wear) {
        // The first thing carried that can be worn and is not yet. The
        // engine puts it in its slot and takes off whatever was there.
        let wearable = |i: &&Entity| !worn.contains(**i) && things.get(**i).is_ok_and(|(_, w)| w);
        match bag.items.iter().find(wearable) {
            Some(&item) => {
                intents.equips.write(Intent::new(me, Equip(item)));
            }
            None => log.muted("You carry nothing more to put on."),
        }
    }
}

/// Braziers, goblins and things to find, the one time the floor is built.
fn populate(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    goblins: Res<Goblins>,
    finds: Res<Finds>,
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
        let mut rng = seed.place(ev.map).rng(POPULATE, 0);
        let walkable = |p| map.is_walkable(p);
        let stock = stock(place.terrain.bounds(), ev.entry, walkable, &mut rng);

        for at in stock.braziers {
            // A fixture: it never moves, so its light is cast once.
            commands.spawn((
                Position(at),
                Name::new("brazier"),
                LightSource::new(230, 6, Rgb::new(255, 150, 80)).flickering(110),
                Glyph::new('*', Color::srgb(1.0, 0.6, 0.25)).on_layer(1),
            ));
        }
        for at in stock.goblins {
            commands.spawn(goblins.goblin(at));
        }
        for at in stock.potions {
            commands.spawn((finds.potion(), Position(at)));
        }
        if let Some(at) = stock.dagger {
            commands.spawn((finds.dagger(), Position(at)));
        }
        if let Some(at) = stock.coat {
            commands.spawn((finds.coat(), Position(at)));
        }
    }
}

/// Where everything on a floor goes. Worked out from the floor alone, so
/// the tests can ask it of every seed without running the game.
struct Stock {
    braziers: Vec<Point>,
    goblins: Vec<Point>,
    potions: Vec<Point>,
    dagger: Option<Point>,
    coat: Option<Point>,
}

/// A cell for each thing on a floor, no two on one cell and none on
/// `entry`, where you arrive.
fn stock(
    bounds: GridRect,
    entry: Point,
    walkable: impl Fn(Point) -> bool,
    rng: &mut impl Rng,
) -> Stock {
    let mut taken = vec![entry];
    let mut place = |away: i32| {
        let at = open_cell(&walkable, bounds, entry, away, &taken, rng)?;
        taken.push(at);
        Some(at)
    };
    Stock {
        braziers: (0..BRAZIERS).map_while(|_| place(6)).collect(),
        // Never close enough to strike on the first turn.
        goblins: (0..GOBLINS).map_while(|_| place(10)).collect(),
        potions: (0..POTIONS).map_while(|_| place(3)).collect(),
        dagger: place(3),
        coat: place(3),
    }
}

/// A walkable cell at least `away` steps from `from` that nothing has taken,
/// or `None` when a fair number of tries finds none.
fn open_cell(
    walkable: impl Fn(Point) -> bool,
    bounds: GridRect,
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
        .find(|p| walkable(*p) && geometry::chebyshev(*p, from) >= away && !taken.contains(p))
}

/// A potion, and how much health it mends.
#[derive(Component, Clone, Copy)]
struct Potion(i32);

/// What drinking a potion means. The engine has already spent the turn on
/// it; this is the part only the game knows. It runs inside the turn, so
/// you are mended before the next goblin strikes.
fn drink(
    mut commands: Commands,
    mut used: MessageReader<ItemEvent>,
    potions: Query<&Potion>,
    mut drinkers: Query<(&mut Health, Has<Player>)>,
    mut log: Log,
) {
    for ev in used.read() {
        let ItemEvent::Used { actor, item } = *ev else {
            continue;
        };
        let (Ok(potion), Ok((mut health, you))) = (potions.get(item), drinkers.get_mut(actor))
        else {
            continue;
        };
        health.current = (health.current + potion.0).min(health.max);
        // Drunk, it is gone, and the engine takes it out of the bag.
        commands.entity(item).despawn();
        if you {
            log.push("You drink the potion. Your wounds close.", Tones::GOOD);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use atglyph::prelude::{combat::*, world::*};
    use atglyph::testing::{KeyScriptPlugin, press};

    /// The game with no window: the same `Starter` plugin `main` adds, and
    /// keys played the way a keyboard plays them.
    fn headless(seed: u64) -> App {
        atglyph::testing::headless(RunSeed(seed), (), (Starter, KeyScriptPlugin))
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
            let dungeon = Dungeon::new();
            let tables = dungeon.tiles.tables();
            let built = dungeon
                .build(FLOOR, PlaceSeed::of(RunSeed(seed), FLOOR.0))
                .unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            let walkable = |t: TileId| tables.walkable[t.index()];
            assert!(
                built.terrain.get(built.entry).is_some_and(walkable),
                "seed {seed}: you arrive inside a wall"
            );
            let open = built.terrain.iter().filter(|(_, t)| walkable(*t)).count();
            assert!(open > 200, "seed {seed}: only {open} open cells");

            // And everything `populate` lays out lands on a cell you can
            // reach, the potions, the dagger and the coat all of them.
            let on_floor = |p: Point| built.terrain.get(p).is_some_and(walkable);
            let mut rng = PlaceSeed::of(RunSeed(seed), FLOOR.0).rng(POPULATE, 0);
            let stock = stock(built.terrain.bounds(), built.entry, on_floor, &mut rng);
            assert_eq!(stock.potions.len(), POTIONS, "seed {seed}: potions");
            let things = stock.potions.iter().chain(&stock.dagger).chain(&stock.coat);
            assert_eq!(
                things.clone().count(),
                POTIONS + 2,
                "seed {seed}: the dagger or the coat is missing"
            );
            assert!(
                things.copied().all(on_floor),
                "seed {seed}: a thing lies in a wall"
            );
        }
    }

    /// `d` drinks the first potion in the bag: it mends what it says, never
    /// past full health, and is gone once drunk.
    #[test]
    fn drinking_a_potion_mends_your_wounds() {
        let (mut app, me) = started(7);
        let max = app.world().get::<Health>(me).unwrap().max;
        app.world_mut().get_mut::<Health>(me).unwrap().current = max - 10;
        let drink = |app: &mut App| {
            let potion = app.world().resource::<Finds>().potion();
            let potion = app.world_mut().spawn(potion).id();
            app.world_mut()
                .get_mut::<Inventory>(me)
                .unwrap()
                .items
                .push(potion);
            press(app, KeyCode::KeyD);
            app.update();
            potion
        };
        let health = |app: &App| app.world().get::<Health>(me).unwrap().current;

        let potion = drink(&mut app);
        assert_eq!(health(&app), max - 10 + POTION_HEALS, "the potion mended");
        assert!(app.world().get_entity(potion).is_err(), "and was drunk");
        assert!(
            !app.world().get::<Inventory>(me).unwrap().contains(potion),
            "and is gone from the bag"
        );
        drink(&mut app);
        assert_eq!(health(&app), max, "a second mends only up to full");

        // At full health a potion is refused, kept, and costs no turn.
        let kept = drink(&mut app);
        assert!(
            app.world().get::<Inventory>(me).unwrap().contains(kept),
            "a potion is not drunk for nothing"
        );
        assert!(
            app.world().get::<MyTurn>(me).is_some(),
            "and the turn is still yours"
        );
        let said = app.world().resource::<MessageLog>().iter().last().cloned();
        assert_eq!(
            said.map(|e| (e.text, e.tone)),
            Some(("You are not hurt.".to_string(), Tones::MUTED)),
            "and the log says why"
        );
    }

    /// The dagger, wielded, is what you strike with in place of your fists.
    #[test]
    fn a_wielded_dagger_is_the_blow_you_strike_with() {
        let (mut app, me) = started(7);
        let blow = |app: &mut App| {
            let mut loadout = bevy::ecs::system::SystemState::<Loadout>::new(app.world_mut());
            loadout
                .get(app.world())
                .expect("the loadout reads")
                .melee(me)
                .map(|m| m.dice)
        };
        let fists = blow(&mut app);
        let dagger = app.world().resource::<Finds>().dagger();
        let dagger = app.world_mut().spawn(dagger).id();
        atglyph::testing::equip(&mut app, me, dagger);
        assert!(
            app.world().get::<Equipped>(me).unwrap().contains(dagger),
            "the dagger is wielded"
        );
        assert_eq!(
            blow(&mut app),
            Some(DiceRoll::new(2, 4)),
            "its blow is yours"
        );
        assert_ne!(fists, blow(&mut app), "and not your fists'");
    }

    /// `g` picks the coat up and `w` puts it on, and from then on its armor
    /// is added to yours: the same goblin's same blow takes less.
    #[test]
    fn a_coat_picked_up_and_worn_takes_its_share_of_each_blow() {
        let (mut bare, me) = started(7);
        let unarmored = first_blow_taken(&mut bare, me);

        let (mut app, me) = started(7);
        let at = app.world().get::<Position>(me).unwrap().0;
        let coat = app.world().resource::<Finds>().coat();
        let coat = app.world_mut().spawn((coat, Position(at))).id();
        app.update();
        press(&mut app, KeyCode::KeyG);
        app.update();
        press(&mut app, KeyCode::KeyW);
        app.update();
        assert!(
            app.world().get::<Equipped>(me).unwrap().contains(coat),
            "the player wears the coat"
        );
        let armored = first_blow_taken(&mut app, me);
        assert!(
            armored < unarmored,
            "the coat took its share: {armored} through it, {unarmored} without"
        );
    }

    /// How much the first blow of a goblin stood beside the player takes,
    /// its blow made the same every time so two runs can be compared.
    fn first_blow_taken(app: &mut App, me: Entity) -> i32 {
        let at = app.world().get::<Position>(me).unwrap().0;
        let beside = Direction::ALL
            .into_iter()
            .map(|dir| at + dir.offset())
            .find(|p| {
                app.world().resource::<WorldMap>().is_walkable(*p)
                    && !app.world().resource::<Occupancy>().is_occupied(*p)
            })
            .expect("room beside the player");
        let goblins = app.world().resource::<Goblins>();
        let (goblin, stab) = (goblins.goblin(beside), goblins.stab);
        app.world_mut()
            .spawn(goblin)
            .insert(MeleeAttack::new(stab, DiceRoll::flat(4)));
        app.update();
        let health = |app: &App| app.world().get::<Health>(me).unwrap().current;
        let before = health(app);
        for _ in 0..5 {
            press(app, KeyCode::Period);
            app.update();
            if health(app) < before {
                return before - health(app);
            }
        }
        panic!("the goblin beside the player never struck");
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
