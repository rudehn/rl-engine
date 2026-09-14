//! Lamplight: one dark cave and everything that glows in it.
//!
//! The engine's third example, small enough to read in one sitting, and
//! the one that shows lighting. There is no surface, one map, and no goal
//! beyond seeing: the player carries a lantern that burns oil, a brazier
//! burns in a chamber nearby, wisps drift about with a glow of their own,
//! patches of fungus glow green, a torch lies on the floor to be picked up
//! and dropped, and lurkers hunt in the dark, seen only when a light
//! reaches them. Every tile varies from cell to cell and flames flicker,
//! all of it authored in `Cave::appearance` and the light sources below.
//!
//! Everything lighting-shaped is one component, [`LightSource`], on a
//! prop, an actor or an item; the engine casts it, gates sight by it and
//! tints the map with it. This file only decides what glows.
//!
//! `cargo run -p lamplight -- --seed 7`
//!
//! Keys: move with arrows, vi keys or the numpad; `L` lights or douses the
//! lantern; `g` picks up; `d` drops the torch; `v` shows light as digits;
//! `.` waits; `q` quits.

use std::sync::Arc;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_mapgen::passes::{CellularCave, CentralStart, KeepLargestRegion};
use rl_engine::rl_rules::ai::tactics::{Hunt, MeleeAdjacent, Wander};
use rl_engine::rl_rules::damage::SubtractArmor;
use rl_engine::rl_rules::faction::FactionDef;

const COLS: i32 = 80;
const ROWS: i32 = 40;
const LOG_ROWS: i32 = 4;
const CAVE: MapId = MapId(1);

const LANTERN: Rgb = Rgb::new(255, 205, 140);
const FLAME: Rgb = Rgb::new(255, 115, 35);
const WISP: Rgb = Rgb::new(95, 165, 255);
const SPORES: Rgb = Rgb::new(70, 235, 130);

fn main() -> AppExit {
    let mut seed = RunSeed::fresh();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--seed") {
        seed = RunSeed(args[i + 1].parse().expect("seed"));
    }
    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("Lamplight", COLS, ROWS).map(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        .add_plugins((CombatPlugin, ItemsPlugin, LightingPlugin))
        .insert_resource(Seed(seed))
        // No hints on the strip: the log's second line already lists the
        // keys, and forty characters of them would squeeze the lantern
        // readout off the row.
        .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)))
        .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
        .init_resource::<LightOverlay>()
        .add_systems(Update, note_the_dark.in_set(ViewSet::Annotate))
        .add_systems(Startup, start)
        .add_systems(Update, player_input.in_set(EngineSet::Input))
        .add_systems(Turn, populate.in_set(TurnSet::React))
        .add_systems(Update, narrate.in_set(PresentSet::Narrate));
    app.run()
}

#[derive(Resource, Clone, Copy)]
struct Seed(RunSeed);

/// The cave's two tiles.
struct Cave {
    tiles: TileRegistry,
    seed: RunSeed,
}

impl Cave {
    fn new(seed: RunSeed) -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("rock")).unwrap();
        tiles.register(TileProps::floor("floor")).unwrap();
        tiles.register(TileProps::floor("fungus")).unwrap();
        tiles.register(TileProps::floor("water").move_cost(200)).unwrap();
        Self { tiles, seed }
    }

    /// Each tile in full light, both colours, and how it varies from cell
    /// to cell. The light does the rest: these are the colours a torch at
    /// full strength shows, and darkness and memory are derived from them.
    fn appearance(&self) -> TileAppearance {
        let mut look = TileAppearance::new();
        let t = |name| self.tiles.expect(name);
        look.set_varied(t("rock"), Cell::new('#', Color::srgb(0.8, 0.74, 0.66)).on(Color::srgb(0.47, 0.42, 0.38)), Vary::new(0.2, 0.05));
        look.set_varied(t("floor"), Cell::new('.', Color::srgb(0.74, 0.7, 0.62)).on(Color::srgb(0.23, 0.2, 0.18)), Vary::new(0.3, 0.07));
        look.set_varied(t("fungus"), Cell::new('"', Color::srgb(0.45, 0.95, 0.6)).on(Color::srgb(0.07, 0.2, 0.12)), Vary::new(0.25, 0.1));
        look.set_varied(t("water"), Cell::new('~', Color::srgb(0.4, 0.6, 1.0)).on(Color::srgb(0.06, 0.14, 0.38)), Vary::new(0.15, 0.05).shimmering(0.3));
        look
    }
}

/// Blobs of one tile laid over another after the cave is carved: pools
/// and fungus. A finish-phase pass, since growth may not follow structures.
struct Patches {
    name: &'static str,
    tile: TileId,
    on: TileId,
    count: u32,
    radius: i32,
}

impl Pass<BaseContext> for Patches {
    fn name(&self) -> &'static str {
        self.name
    }
    fn phase(&self) -> Phase {
        Phase::Finish
    }
    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let bounds = ctx.terrain().bounds();
        for _ in 0..self.count {
            let centre = Point::new(ctx.rng().random_range(bounds.x..bounds.right()), ctx.rng().random_range(bounds.y..bounds.bottom()));
            for p in geometry::disc(centre, self.radius) {
                if ctx.terrain().get(p) == Some(self.on) && ctx.rng().random_range(0..100) < 70 {
                    ctx.terrain_mut().set(p, self.tile);
                }
            }
        }
        Ok(())
    }
}

impl PlaceRules for Cave {
    fn build(&self, _: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let t = |name| self.tiles.expect(name);
        let (wall, floor) = (t("rock"), t("floor"));
        let mut ctx = BaseContext::blank(70, 30, self.tiles.clone(), wall);
        Chain::new()
            .then(CellularCave { wall, floor, fill_pct: 42, ..Default::default() })
            .then(KeepLargestRegion { wall })
            .then(CentralStart)
            .then(Patches { name: "pools", tile: t("water"), on: floor, count: 4, radius: 3 })
            .then(Patches { name: "fungus", tile: t("fungus"), on: floor, count: 6, radius: 2 })
            .run(&mut ctx, self.seed)?;
        PlaceBuild::from_context(ctx)
    }
}

/// What a name on the map is called.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Thing {
    Lantern,
    Torch,
    Brazier,
    Wisp,
    Lurker,
}

impl Thing {
    fn name(self) -> &'static str {
        match self {
            Thing::Lantern => "the lantern",
            Thing::Torch => "the torch",
            Thing::Brazier => "the brazier",
            Thing::Wisp => "a wisp",
            Thing::Lurker => "a lurker",
        }
    }
}

/// What a light sheds when lit, kept while it is doused.
#[derive(Component, Clone, Copy)]
struct Lamp(LightSource);

/// The brains and factions the cave's creatures share.
#[derive(Resource)]
struct Creatures {
    wisp: Arc<Brain<Entity>>,
    lurker: Arc<Brain<Entity>>,
    bite: rl_engine::rl_rules::damage::DamageKindId,
    cave: rl_engine::rl_rules::FactionId,
    wisps: rl_engine::rl_rules::FactionId,
    /// The tile that glows.
    fungus: TileId,
}

/// Rules, the player and its lantern, and the warp into the cave.
fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    mut warps: MessageWriter<WarpRequest>,
    mut log: ResMut<MessageLog>,
    mut next: ResMut<NextState<EngineState>>,
) {
    let cave = Cave::new(seed.0);
    let kinds = Registry::from_defs(vec![DamageKind::new("bite")]).unwrap();
    let facs = Registry::from_defs(vec![FactionDef { name: "you".into() }, FactionDef { name: "cave".into() }, FactionDef { name: "wisps".into() }]).unwrap();
    let (you, cave_side, wisps) = (facs.expect("you"), facs.expect("cave"), facs.expect("wisps"));
    let mut factions = Factions::new(&facs);
    factions.set_mutual(you, cave_side, Relation::Hostile);
    commands.insert_resource(Creatures {
        wisp: Arc::new(Brain::new().then(Wander { chance_pct: 70 })),
        lurker: Arc::new(Brain::new().then(MeleeAdjacent).then(Hunt).then(Wander { chance_pct: 40 })),
        bite: kinds.expect("bite"),
        cave: cave_side,
        wisps,
        fungus: cave.tiles.expect("fungus"),
    });
    commands.insert_resource(CombatRules { kinds: kinds.clone(), factions });
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    commands.insert_resource(CombatRng::for_run(seed.0));
    commands.insert_resource(cave.appearance());
    commands.insert_resource(WorldMap::new(cave.tiles.tables()));
    commands.insert_resource(PlaceRulesRes(Box::new(cave)));
    // The whole of turning lighting on. Ambient stays dark for good.
    commands.insert_resource(Lighting::dark());

    let lantern = LightSource::new(200, 8, LANTERN).flickering(30);
    let lamp = commands.spawn((Item, Thing::Lantern, Lamp(lantern), lantern, Fuel(400))).id();
    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(30), RevealsMap, Speed(100)),
            (Health::full(40), Armor(0), Faction(you), MeleeAttack { kind: kinds.expect("bite"), dice: DiceRoll::new(1, 4) }),
            (Inventory { items: vec![lamp] }, Glyph::new('@', Color::WHITE).on_layer(10)),
        ))
        .id();
    warps.write(WarpRequest::into_place(player, CAVE));
    log.push(format!("Seed {}. The lantern is lit. Something moves in the dark.", seed.0.0), Tones::NOTICE, 0);
    log.push("L lantern   g pick up   d drop torch   v show light   q quit", Tones::TEXT, 0);
    next.set(EngineState::Playing);
}

/// The brazier, the fungus, the torch, the wisps and the lurkers, once the cave exists.
fn populate(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, creatures: Res<Creatures>, map: Res<WorldMap>, seed: Res<Seed>) {
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        let mut rng = seed.0.rng(SeedDomain::new(b"lamplight"), 1);
        let mut spot = |min: i32, max: i32| loop {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            let d = geometry::chebyshev(p, ev.entry);
            if map.is_walkable(p) && d >= min && d <= max {
                return p;
            }
        };
        // A prop: a fixture that never moves, so it lives in the static layer.
        commands.spawn((
            Position(spot(8, 14)),
            Thing::Brazier,
            LightSource::new(235, 9, FLAME).flickering(150),
            Glyph::new('*', Color::srgb(1.0, 0.75, 0.3)).on_layer(1),
        ));
        // Tiles that glow are props too: one faint source on each.
        for (p, _) in place.terrain.iter().filter(|(_, t)| *t == creatures.fungus) {
            commands.spawn((Position(p), LightSource::new(60, 3, SPORES)));
        }
        // An item: lit where it lies, shed from whoever carries it.
        commands.spawn((
            Position(spot(3, 6)),
            Item,
            Thing::Torch,
            LightSource::new(190, 6, FLAME).flickering(120),
            Glyph::new('!', Color::srgb(1.0, 0.7, 0.3)).on_layer(2),
        ));
        // Actors that glow: their light moves with them.
        for _ in 0..3 {
            commands.spawn((
                (Actor, Blocks, Position(spot(6, 30)), Thing::Wisp, Health::full(4), Faction(creatures.wisps), Speed(120)),
                (
                    Perception(6),
                    Mind(creatures.wisp.clone()),
                    LightSource::new(130, 4, WISP).flickering(70),
                    Glyph::new('o', Color::srgb(0.7, 0.9, 1.0)).on_layer(5),
                ),
            ));
        }
        // Actors that see in the dark and shed nothing: only a light finds them.
        for _ in 0..3 {
            commands.spawn((
                (Actor, Blocks, Position(spot(14, 40)), Thing::Lurker, Health::full(12), Armor(1), Faction(creatures.cave), Speed(90)),
                (
                    Perception(12),
                    DarkSight(9),
                    Mind(creatures.lurker.clone()),
                    MeleeAttack { kind: creatures.bite, dice: DiceRoll::new(1, 3) },
                    Glyph::new('L', Color::srgb(0.55, 0.5, 0.6)).on_layer(5),
                ),
            ));
        }
    }
}

type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position, &'static Inventory), (With<Player>, With<MyTurn>)>;

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

/// What the player's keys can ask for.
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerIntents<'w> {
    steps: MessageWriter<'w, Intent<Step>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    pick_ups: MessageWriter<'w, Intent<PickUp>>,
    drops: MessageWriter<'w, Intent<DropItem>>,
    uses: MessageWriter<'w, Intent<UseItem>>,
}

/// Keys to intents. The lantern is used with [`UseItem`], so lighting
/// and dousing cost a turn like anything else and come back as an
/// [`ItemEvent::Used`] for the game to act on.
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    occupancy: Res<Occupancy>,
    things: Query<&Thing>,
    player: PlayerTurn,
    mut overlay: ResMut<LightOverlay>,
    mut intents: PlayerIntents,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    if keys.just_pressed(KeyCode::KeyV) {
        overlay.0 = !overlay.0;
        return;
    }
    let Ok((entity, pos, bag)) = player.single() else { return };
    let carried = |thing: Thing| bag.items.iter().copied().find(|i| things.get(*i) == Ok(&thing));
    let shifted = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if let Some((_, dir)) = MOVES.iter().find(|(codes, _)| !shifted && keys.any_just_pressed(codes.iter().copied())) {
        match occupancy.first_at(pos.0 + dir.offset()) {
            Some(other) => {
                intents.attacks.write(Intent::new(entity, Attack(other)));
            }
            None => {
                intents.steps.write(Intent::new(entity, Step(*dir)));
            }
        }
    } else if shifted && keys.just_pressed(KeyCode::KeyL) {
        if let Some(lamp) = carried(Thing::Lantern) {
            intents.uses.write(Intent::new(entity, UseItem(lamp)));
        }
    } else if keys.just_pressed(KeyCode::KeyG) {
        intents.pick_ups.write(Intent::new(entity, PickUp));
    } else if keys.just_pressed(KeyCode::KeyD) {
        if let Some(torch) = carried(Thing::Torch) {
            intents.drops.write(Intent::new(entity, DropItem(torch)));
        }
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        intents.waits.write(Intent::new(entity, Wait));
    }
}

/// What narration reads and writes.
#[derive(bevy::ecs::system::SystemParam)]
struct Voice<'w, 's> {
    commands: Commands<'w, 's>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
    next: ResMut<'w, NextState<EngineState>>,
    things: Query<'w, 's, &'static Thing>,
    lamps: Query<'w, 's, (&'static Lamp, Option<&'static LightSource>, Option<&'static Fuel>)>,
    players: Query<'w, 's, (), With<Player>>,
}

/// Lights toggled, things picked up and dropped, lights going out, bites
/// and deaths.
fn narrate(
    mut items: MessageReader<ItemEvent>,
    mut lights: MessageReader<LightEvent>,
    mut dealt: MessageReader<DamageDealt>,
    mut deaths: MessageReader<DeathEvent>,
    mut voice: Voice,
) {
    let Voice { commands, turns, log, next, things, lamps, players } = &mut voice;
    let turn = turns.turn_number();
    let name = |e: Entity| -> &'static str { if players.get(e).is_ok() { "you" } else { things.get(e).map(|t| t.name()).unwrap_or("something") } };
    for ev in items.read() {
        match *ev {
            ItemEvent::Used { item, .. } => {
                let Ok((lamp, lit, fuel)) = lamps.get(item) else { continue };
                if lit.is_some() {
                    commands.entity(item).remove::<LightSource>();
                    log.push("You douse the lantern. The dark comes in close.", Tones::TEXT, turn);
                } else if fuel.is_some_and(|f| f.0 == 0) {
                    log.push("The lantern is dry.", Tones::BAD, turn);
                } else {
                    commands.entity(item).insert(lamp.0);
                    log.push("You light the lantern.", Tones::TEXT, turn);
                }
            }
            ItemEvent::PickedUp { item, .. } => log.push(format!("You pick up {}. It keeps burning in your hand.", name(item)), Tones::TEXT, turn),
            ItemEvent::Dropped { item, .. } => log.push(format!("You set {} down. It lights the floor where it lies.", name(item)), Tones::TEXT, turn),
            _ => {}
        }
    }
    for ev in lights.read() {
        let LightEvent::BurntOut { entity } = *ev;
        log.push(format!("{} gutters and goes out.", capital(name(entity))), Tones::BAD, turn);
    }
    for d in dealt.read() {
        let attacker = d.hit.attacker.map(name).unwrap_or("something");
        let (verb, cat) = if attacker == "you" { ("bite", Tones::TEXT) } else { ("bites", Tones::BAD) };
        let tail = if d.dealt <= 0 { " for nothing.".to_string() } else { format!(" for {}.", d.dealt) };
        log.push(format!("{} {verb} {}{tail}", capital(attacker), name(d.target)), cat, turn);
    }
    for d in deaths.read() {
        if d.was_player {
            log.push("The dark has you. Press q to quit.", Tones::BAD, turn);
            next.set(EngineState::Idle);
        } else {
            log.push(format!("{} dies.", capital(name(d.entity))), Tones::GOOD, turn);
        }
    }
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// What the engine cannot know: the lantern, and how bright it is here.
fn note_the_dark(
    mut vitals: ResMut<VitalsView>,
    mut facets: ResMut<Facets>,
    lighting: Res<Lighting>,
    player: Query<(&Position, &Inventory), With<Player>>,
    lamps: Query<(Option<&LightSource>, Option<&Fuel>), With<Lamp>>,
) {
    let Ok((pos, bag)) = player.single() else { return };
    let lantern = bag
        .items
        .iter()
        .find_map(|i| lamps.get(*i).ok())
        .map(|(lit, fuel)| {
            let oil = fuel.map(|f| f.0).unwrap_or(0);
            if lit.is_some() { format!("lit, {oil} oil") } else { format!("out, {oil} oil") }
        })
        .unwrap_or_else(|| "gone".into());
    vitals.facets.push(facets.facet("lantern", format!("lantern {lantern}")));
    vitals.facets.push(facets.facet("light", format!("light here {}", lighting.at(pos.0).intensity)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headless(seed: u64) -> (App, Entity, Entity) {
        let mut app = rl_engine::rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, CombatPlugin, ItemsPlugin, LightingPlugin));
        app.insert_resource(Seed(RunSeed(seed)))
            .init_resource::<MessageLog>()
            .add_systems(Startup, start)
            .add_systems(Turn, populate.in_set(TurnSet::React))
            .add_systems(Update, narrate.in_set(PresentSet::Narrate));
        app.update();
        app.update();
        app.update();
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let lamp = app.world_mut().query_filtered::<Entity, With<Lamp>>().single(app.world()).unwrap();
        (app, player, lamp)
    }

    fn seen(app: &mut App, player: Entity) -> usize {
        app.world().get::<Viewshed>(player).unwrap().visible.count()
    }

    fn act<A: Action>(app: &mut App, player: Entity, action: A) {
        app.world_mut().write_message(Intent::new(player, action));
        app.update();
    }

    #[test]
    fn dousing_the_lantern_closes_the_dark_in_and_lighting_it_opens_it_again() {
        let (mut app, player, lamp) = headless(7);
        assert!(app.world().get::<MyTurn>(player).is_some());
        let lit = seen(&mut app, player);
        assert!(lit > 9, "the lantern shows more than what you touch: {lit}");
        act(&mut app, player, UseItem(lamp));
        app.update();
        let doused = seen(&mut app, player);
        assert!(doused < lit, "doused {doused} < lit {lit}");
        assert!(app.world().get::<LightSource>(lamp).is_none());
        act(&mut app, player, UseItem(lamp));
        app.update();
        assert!(app.world().get::<LightSource>(lamp).is_some());
        assert!(seen(&mut app, player) > doused, "the wisps have drifted, but the lantern's reach is back");
    }

    #[test]
    fn every_light_in_the_cave_lands_on_its_own_tile() {
        let (mut app, _, _) = headless(3);
        let mut glowing = app.world_mut().query::<(&Position, &LightSource, &Thing)>();
        let lights: Vec<(Point, u8, Thing)> = glowing.iter(app.world()).map(|(p, s, t)| (p.0, s.intensity, *t)).collect();
        assert_eq!(lights.iter().filter(|l| l.2 == Thing::Brazier).count(), 1);
        assert_eq!(lights.iter().filter(|l| l.2 == Thing::Torch).count(), 1);
        assert_eq!(lights.iter().filter(|l| l.2 == Thing::Wisp).count(), 3);
        let lighting = app.world().resource::<Lighting>();
        // At least its own glow; more where another light overlaps.
        for (at, intensity, thing) in lights {
            assert!(lighting.at(at).intensity >= intensity, "{} at {at:?}", thing.name());
        }
    }

    #[test]
    fn lurkers_shed_nothing_and_see_in_the_dark() {
        let (mut app, _, _) = headless(3);
        let mut lurkers = app.world_mut().query_filtered::<(Option<&LightSource>, &DarkSight), With<Thing>>();
        let found: Vec<_> = lurkers.iter(app.world()).collect();
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|(light, dark)| light.is_none() && dark.0 > 0));
    }
}
