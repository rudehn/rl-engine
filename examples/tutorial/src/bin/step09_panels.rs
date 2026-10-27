//! Warren, step 9: the panels, and a rail down the side.
//!
//! The guide chapter is `docs/guide/src/09-where-to-go-next.md`.
//!
//! `cargo run -p tutorial --bin step09_panels`, and `WARREN_SEED=19` picks
//! another warren.
//!
//! Keys: `x` look, `p` the log, `?` every key; arrows to walk or strike, Shift and a direction to shove, `>` take the stairs, `a` screech, `g` get, `e` eat, `r` throw
//! a rock (Enter to confirm an aim, Escape to put it away), `t` lantern,
//! `.` wait, `q` quit.

use atglyph::prelude::*;
use atglyph::prelude::{combat::*, items::*, mapgen::*, minds::*, ui::*, world::*};
use atglyph::rand::Rng;
use atglyph::rules::ai::{Repertoire, Standard};
use atglyph::rules::faction::FactionDef;
use atglyph::rules::{AbilityId, TagDef, TagId, Wits};
use bevy::prelude::*;
use serde::Deserialize;
use std::sync::Arc;

/// The terminal, in cells.
const COLS: i32 = 80;
const ROWS: i32 = 40;
/// Rows at the bottom of the terminal given over to the message log.
const LOG_ROWS: i32 = 5;
/// Columns down the right given over to the rail.
const RAIL: i32 = 24;
/// The bestiary and the tiles' looks, compiled in so the binary runs from
/// anywhere.
const RATS_RON: &str = include_str!("../../assets/rats.ron");
const TILES_RON: &str = include_str!("../../assets/tiles.ron");

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

// ANCHOR: layout
/// The screen, cut once so the map and every panel agree on it: the map,
/// the log under it, a rail down the right, and boxes over the map.
struct Screen {
    map: GridRect,
    log: GridRect,
    vitals: GridRect,
    nearby: GridRect,
    over_map: GridRect,
    scrollback: GridRect,
    controls: GridRect,
    hint: GridRect,
}

impl Screen {
    fn new() -> Self {
        // Vitals at the head of the rail, the hint for the controls screen
        // in its last row, and what is in sight between them.
        let rail = RailLayout::new(COLS, ROWS, RAIL).log(LOG_ROWS).vitals(9).hint().cut();
        let map = rail.map;
        let over_map = GridRect::new(map.x + 2, map.bottom() - 10, map.width.min(46), 9);
        Self {
            map,
            log: rail.log,
            vitals: rail.vitals,
            nearby: rail.nearby,
            over_map,
            scrollback: map.inflate(-2),
            controls: GridRect::new(0, 0, COLS, ROWS - LOG_ROWS),
            hint: rail.hint,
        }
    }
}
// ANCHOR_END: layout

// ANCHOR: main
fn main() -> AppExit {
    let screen = Screen::new();
    let seed = std::env::var("WARREN_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(7);
    App::new()
        .add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS).map(screen.map))
        .add_plugins(Warren)
        // ANCHOR: panels
        // Each panel draws a view the engine keeps current, in its own
        // rectangle. Add the ones your game has a use for.
        .add_plugins((
            VitalsPanel::new(screen.vitals).heading("Vitals").bars(10),
            NearbyPanel::new(screen.nearby).titled("").headings("In sight", "On the floor"),
            LogPanel::new(screen.log),
            // `x` opens the look cursor; this says what it is on.
            InspectPanel::new(screen.over_map),
            // `p` opens the whole log, scrollable.
            ScrollbackPanel::new(screen.scrollback),
            // `?` lists every key, and the rail's last row says so.
            ControlsPanel::new(screen.controls).hint(screen.hint),
        ))
        // ANCHOR_END: panels
        .add_plugins(TargetPanel::new(screen.over_map).hints("enter confirm  tab next  esc cancel"))
        .add_plugins(narrator())
        // Escape opens the menu, and dying opens it by itself.
        .add_plugins(GameMenuPanel::new(GridRect::new(COLS / 2 - 20, 8, 40, 12)).died("The warren keeps you."))
        .insert_resource(Seed(RunSeed(seed)))
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
        // ANCHOR: register
        // The shove: something the player may do and a mind may choose,
        // and the system that resolves it.
        app.add_choice::<Shove>().add_message::<Shoved>().add_systems(Turn, resolve_shoves.in_set(ResolveSet::Act));
        // ANCHOR_END: register
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
        // ANCHOR: tone
        // A tone the engine never heard of, for a note on the nearby list.
        app.add_tone("fleeing", Color::srgb(0.6, 0.8, 1.0));
        app.add_plugins(NearbyViewPlugin).add_systems(Update, (note_floor, note_what_a_rat_is_doing).in_set(ViewSet::Annotate));
        // ANCHOR_END: tone
        declare_controls(app);
    }
}
// ANCHOR_END: plugin

// ANCHOR: tiles
/// The warren's tiles.
struct Floors {
    tiles: TileRegistry,
}

impl Floors {
    // ANCHOR: looks
    /// What each tile is, read from `assets/tiles.ron`.
    fn new() -> Self {
        Self { tiles: atglyph::rules::tiles::load(TILES_RON).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}")) }
    }

    /// How each tile looks, read from the same file. A tile the file gives
    /// no look is reported at startup, by name.
    fn appearance(&self) -> TileAppearance {
        TileAppearance::load(TILES_RON, &self.tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"))
    }
    // ANCHOR_END: looks
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
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("venom"), DamageKind::new("kick"), DamageKind::new("din")]).unwrap();
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
    // The bestiary names damage kinds, so it loads against the registries.
    commands.insert_resource(Bestiary::load(&registries.names(), Kin { faction: vermin, bite: kinds.expect("bite"), bread, gnaw }));
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
    shove: ControlId,
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
            .pick_up("Act", "pick up what is here")
            .wait("Act", "wait a turn"),
    );
    let binds = Binds {
        // The same direction keys with Shift held. A chord matches exactly,
        // so Shift and `l` never reads as a step.
        shove: app.add_control("Move", "shove", Keys::Directions { shift: true }),
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
    if let Some(dir) = keys.direction(binds.shove) {
        intents.shoves.write(Intent::new(entity, Shove(dir)));
    } else if keys.just_pressed(binds.screech) {
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
    shoves: MessageWriter<'w, Intent<Shove>>,
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

// ANCHOR: def
/// One kind of vermin, exactly as `assets/rats.ron` writes it. Serde parses
/// it; [`Named`] says what to key it by; a [`NameRef`] is a name in the
/// file that the load turns into an id.
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
    #[serde(default)]
    clever: bool,
    #[serde(default)]
    shoves: bool,
    spawn: (i32, i32, u32, u32, u32),
}

impl Named for RatDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// What a spawned creature was made from, so a system can look its
/// definition up.
#[derive(Component, Clone, Copy)]
struct Kind(Id<RatDef>);
// ANCHOR_END: def

// ANCHOR: bestiary
/// The brain every kind is built from: bite, run once hurt enough, chase
/// what it sees, else wander.
const WARREN: Standard = Standard { melee: true, flee: true, flee_after_attacks: true, hunt: true, wander_chance: Some(40), ..Standard::NONE };

/// A clever kind's brain adds its knacks, throwing and fetching rocks.
const CLEVER: Standard = Standard { ability_chance: Some(80), throw_chance: Some(60), scavenge_reach: Some(9), wander_chance: Some(30), ..WARREN };

/// What every creature shares that is not in the file.
struct Kin {
    faction: FactionId,
    bite: atglyph::rules::damage::DamageKindId,
    bread: TagId,
    gnaw: AbilityId,
}

/// The bestiary: the kinds, a brain for each, and a table of what lives
/// at which depth.
#[derive(Resource)]
struct Bestiary {
    defs: Registry<RatDef>,
    table: BandedTable<Id<RatDef>>,
    minds: Vec<Arc<Brain<Entity>>>,
    kin: Kin,
}

impl Bestiary {
    fn load(names: &Names, kin: Kin) -> Self {
        let defs: Registry<RatDef> = names.load(RATS_RON).unwrap_or_else(|e| panic!("assets/rats.ron: {e}"));
        let mut table = BandedTable::default();
        let mut minds = Vec::new();
        for (id, def) in defs.iter() {
            let (first, last, weight, group_min, group_max) = def.spawn;
            if weight > 0 {
                table.push(BandedEntry::new(id).bands(first, last).weight(weight).group(group_min, group_max));
            }
            // ANCHOR: brain
            let wits = if def.clever { Wits::SAPIENT } else { Wits::ANIMAL };
            let can = Repertoire { flee_at: def.flee_at, melee: true, abilities: def.clever, wits, ..Repertoire::default() };
            let mut brain = if def.clever { CLEVER.brain(can) } else { WARREN.brain(can) };
            // A shover leads with its shove: Warren's own choice, routed to
            // Warren's own action.
            if def.shoves {
                brain = brain.first(ShoveAdjacent);
            }
            minds.push(Arc::new(brain.build()));
            // ANCHOR_END: brain
        }
        Self { defs, table, minds, kin }
    }

    /// Spawns one of `id` at `at`.
    fn spawn(&self, commands: &mut Commands, id: Id<RatDef>, at: Point) -> Entity {
        let def = self.defs.get(id);
        // A clever kind carries its supper, and knows how to eat it.
        let supper = def.clever.then(|| commands.spawn((Item, Crust(6), Tagged(vec![self.kin.bread]), Name::new("crust of bread"))).id());
        let mut e = commands.spawn((
            (Actor, Blocks, Kind(id), Position(at), Speed(def.speed), Faction(self.kin.faction)),
            (Health::full(def.hp), Armor(def.armor), Perception(def.perception), DarkSight(9), Mind(self.minds[id.index()].clone())),
            (MeleeAttack::new(def.kind.id(), def.attack), Glyph::new(def.glyph, Color::srgb(def.color.0, def.color.1, def.color.2)).on_layer(5)),
            (Name::new(def.name.clone()),),
        ));
        if def.clever {
            e.insert((Intelligence(Wits::SAPIENT), Inventory { items: supper.into_iter().collect() }, Grants(vec![self.kin.gnaw])));
        } else {
            e.insert(Intelligence(Wits::ANIMAL));
        }
        e.id()
    }
}
// ANCHOR_END: bestiary

// ANCHOR: populate
/// The random stream rats are placed from. Each floor draws it from its own
/// seed, so floors can be entered in any order.
const RATS: SeedDomain = SeedDomain::new(b"warren.rats");

/// Fills a floor the first time it is entered.
fn populate(
    mut commands: Commands,
    mut entered: MessageReader<PlaceEntered>,
    bestiary: Res<Bestiary>,
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
            // The rat king sits on it. Its weight of 0 keeps it out of the
            // table, so it is placed by name.
            bestiary.spawn(&mut commands, bestiary.defs.expect("rat king"), far);
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        let mut rng = seed.place(ev.map).rng(RATS, 0);
        // Groups, not single rats: the table says what may live at this
        // depth, how likely it is, and how many come together.
        let mut groups = 0;
        for _ in 0..60 {
            if groups >= 3 + depth as usize {
                break;
            }
            let Some((&id, count)) = bestiary.table.pick_group(depth as i32, &mut rng) else { break };
            let anchor = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            let mut placed = 0;
            for p in geometry::square(anchor, 2) {
                // On floor, and not close enough to the start to be unfair.
                if placed < count && map.is_walkable(p) && geometry::chebyshev(p, ev.entry) >= 8 {
                    bestiary.spawn(&mut commands, id, p);
                    placed += 1;
                }
            }
            if placed > 0 {
                groups += 1;
            }
        }
        // Bread and rocks, scattered the same way.
        let mut dropped = 0;
        while dropped < 14 {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            if map.is_walkable(p) {
                litter(&mut commands, &bestiary, p, dropped % 2 == 0);
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
fn litter(commands: &mut Commands, bestiary: &Bestiary, p: Point, bread: bool) {
    if bread {
        commands.spawn((Item, Crust(8), Name::new("crust of bread"), Position(p), Glyph::new('%', Color::srgb(0.85, 0.72, 0.40)).on_layer(2)));
    } else {
        // Thrown up to 7 cells, for 1d4.
        let flies = Throwable::new(7, Some((bestiary.kin.bite, DiceRoll::new(1, 4))));
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

/// The log lines only Warren can write. Blows, deaths and pickups the
/// engine narrates.
fn narrate(mut items: MessageReader<ItemEvent>, mut shoved: MessageReader<Shoved>, names: Query<&Name>, mut log: Log) {
    for ev in items.read() {
        if let ItemEvent::Used { .. } = *ev {
            log.push("You eat the crust. It helps.", Tones::GOOD);
        }
    }
    for ev in shoved.read() {
        let target = names.get(ev.target).map(|n| format!("the {}", n.as_str())).unwrap_or_else(|_| "something".into());
        log.push(format!("You shove {target} back."), Tones::TEXT);
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

// ANCHOR: annotate
/// The rail has room for which floor this is.
fn note_floor(mut vitals: ResMut<VitalsView>, map: Res<WorldMap>) {
    let depth = floor_of(map.current());
    vitals.facets.push(Facet::new("floor", format!("floor {depth}/{FLOORS}: {}", name_of(depth))));
}

/// What a rat is up to, as a note on its row in the nearby list. Fleeing
/// is this game's rule, so the engine prints the note without knowing it.
fn note_what_a_rat_is_doing(mut nearby: ResMut<NearbyView>, tones: Res<Tones>, bestiary: Res<Bestiary>, rats: Query<(&Kind, &Health)>) {
    let fleeing = tones.get("fleeing").expect("declared in Warren");
    for row in nearby.actors.iter_mut() {
        let Ok((kind, health)) = rats.get(row.entity) else { continue };
        if health.current * 100 <= bestiary.defs.get(kind.0).flee_at * health.max {
            row.facets.push(Facet::new("mood", "fleeing").toned(fleeing));
        }
    }
}
// ANCHOR_END: annotate

// ANCHOR: action
/// Shove whoever stands next to you, in this direction, back one cell.
///
/// An action is a type. Registering it as a choice makes `Intent<Shove>` a
/// message, and lets a mind choose it too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shove(Direction);
impl Action for Shove {}
impl Choice for Shove {
    fn name(&self) -> &'static str {
        "shove"
    }
}

/// A shove that landed, for the log.
#[derive(Message, Debug, Clone, Copy)]
struct Shoved {
    target: Entity,
}
// ANCHOR_END: action

// ANCHOR: tactic
/// The hog's move: shove whoever is beside it instead of biting.
struct ShoveAdjacent;

impl Tactic<Entity> for ShoveAdjacent {
    fn name(&self) -> &'static str {
        "shove_adjacent"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let me = ctx.snapshot.me.pos;
        let foe = ctx.snapshot.adjacent_enemies().next()?;
        Direction::between(me, foe.pos).map(|d| Decision::own(Shove(d)))
    }
}
// ANCHOR_END: tactic

// ANCHOR: resolver
/// A shove costs half a turn.
const SHOVE_COST: u32 = BASE_ACTION_COST / 2;

/// What the resolver reads and moves.
#[derive(bevy::ecs::system::SystemParam)]
struct Shoving<'w, 's> {
    occupancy: ResMut<'w, Occupancy>,
    map: Res<'w, WorldMap>,
    holders: Query<'w, 's, &'static Position, With<MyTurn>>,
    targets: Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>), Without<MyTurn>>,
}

/// Resolves a shove, the shape every resolver has: claim the turn, do the
/// thing, and say how it went. `failed` leaves the player holding the turn.
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
            // Nobody there, or nowhere for them to go.
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
        let vermin = app.world().resource::<Bestiary>().kin.faction;
        let there = open_line(&app, here, 3);
        app.world_mut().spawn((Actor, Blocks, Position(there), Faction(vermin), Health::full(6), Name::new("rat"), Glyph::new('r', Color::WHITE)));
        let rock = app.world_mut().spawn((Item, Throwable::new(7, None), Name::new("rock"))).id();
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

    // ANCHOR: shove_tests
    #[test]
    fn a_shove_moves_the_rat_one_cell_further_off_and_spends_half_a_turn() {
        let (mut app, me) = started(7);
        let here = at(&app, me);
        let dir = Direction::ALL.into_iter().find(|d| open_line(&app, here, 2) == here + d.offset() + d.offset()).unwrap();
        let rat = app.world_mut().spawn((Actor, Blocks, Position(here + dir.offset()))).id();
        settle(&mut app);
        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(me, Shove(dir)));
        settle(&mut app);
        assert_eq!(app.world().get::<Position>(rat).unwrap().0, here + dir.offset() + dir.offset(), "the rat went back a cell");
        assert_eq!(app.world().resource::<Turns>().now(), before + SHOVE_COST, "and it cost half a turn");
    }

    #[test]
    fn a_shove_at_nobody_costs_nothing_and_leaves_the_turn_in_hand() {
        let (mut app, me) = started(7);
        let here = at(&app, me);
        let dir = Direction::ALL.into_iter().find(|d| open_line(&app, here, 2) == here + d.offset() + d.offset()).unwrap();
        let before = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(me, Shove(dir)));
        settle(&mut app);
        assert_eq!(app.world().resource::<Turns>().now(), before, "no time passed");
        assert!(app.world().get::<MyTurn>(me).is_some(), "the player still holds the turn");
    }
    // ANCHOR_END: shove_tests

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
        let daylight: Vec<Point> =
            world.query_filtered::<(&Position, &OnMap), With<WayOut>>().iter(world).filter(|(_, on)| on.0 == map_of(FLOORS)).map(|(p, _)| p.0).collect();
        assert_eq!(daylight.len(), 1, "one way out, on the last floor");
        let kings: Vec<Point> = world.query::<(&Name, &Position)>().iter(world).filter(|(n, _)| n.as_str() == "rat king").map(|(_, p)| p.0).collect();
        assert_eq!(kings.len(), 1, "one king");
        assert!(geometry::chebyshev(kings[0], daylight[0]) <= 2, "the king waits by the daylight, not at {:?}", kings[0]);
    }

    /// `a` opens the aiming cursor on the rat, and Enter screeches at it.
    #[test]
    fn a_then_enter_screeches_at_the_rat() {
        let (mut app, me) = started(7);
        let here = at(&app, me);
        let vermin = app.world().resource::<Bestiary>().kin.faction;
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
