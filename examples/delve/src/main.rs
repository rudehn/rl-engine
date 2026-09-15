//! The Hollow Whale: five floors down a beached leviathan, mouth to heart.
//!
//! The engine's second worked example, and the one with no surface: no
//! world graph, no chunk streaming, no overworld. A floor is a place, the
//! stairs are transitions, and the run is won when the heart warden on
//! the fifth floor dies. Everything the delve needs from the engine fits
//! in this file and `floors.rs`.
//!
//! It is also the first game here with the lights off: below the Maw the
//! only light is the brand the player carries, the bile that pools on the
//! floor, and whatever the bestiary says a beast sheds.
//!
//! `cargo run -p delve -- --seed 7`, or `--floor 3` to start deeper.

mod effects;
mod floors;

use std::sync::Arc;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_rules::AbilityId;
use rl_engine::rl_rules::ai::awareness::{NoticeStats, StealthStats};
use rl_engine::rl_rules::ai::tactics::SearchLastKnown;
use rl_engine::rl_rules::ai::tactics::UseAbility;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, MeleeAdjacent, Wander};
use rl_engine::rl_rules::damage::SubtractArmor;
use rl_engine::rl_rules::faction::FactionDef;
use serde::Deserialize;

use crate::floors::{FLOORS, Whale, ambient_of, floor_of, map_of, name_of};

const COLS: i32 = 90;
const ROWS: i32 = 46;
const LOG_ROWS: i32 = 4;
const BEASTS_RON: &str = include_str!("../assets/beasts.ron");

fn main() -> AppExit {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.iter().position(|a| a == "--floor").map(|i| args[i + 1].parse::<u32>().expect("floor").clamp(1, FLOORS)).unwrap_or(1);
    let screen = Screen::new();
    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("The Hollow Whale", COLS, ROWS).map(screen.map))
        .add_plugins((CombatPlugin, MindsPlugin, StatusPlugin, ItemsPlugin, LightingPlugin, AbilitiesPlugin, StealthPlugin, FirePlugin, GasPlugin))
        // The engine's seven effects, and the one the delve adds.
        .add_engine_effects()
        .add_effect::<effects::Drain>()
        .init_resource::<LightOverlay>()
        .insert_resource(Seed::from_args())
        .insert_resource(FirstFloor(first))
        .add_plugins((
            VitalsPanel::new(screen.vitals).heading("Vitals").bars(10),
            NearbyPanel::new(screen.nearby).titled("").headings("In sight", "On the floor"),
            LogPanel::new(screen.log),
            InspectPanel::new(screen.inspect),
            ScrollbackPanel::new(screen.scrollback),
            TargetPanel::new(screen.target).hints("[enter] use  [tab] next  [esc] back"),
            AbilityPanel::new(screen.knacks).title("Knacks").hints("[a] close"),
        ))
        .add_systems(Update, (note_floor, show_pools).in_set(ViewSet::Annotate))
        .add_systems(Startup, start)
        // A screen that is up owns the keys: the knack keys decide that for
        // themselves, and everything else waits for the stack to be empty.
        .add_systems(Update, (call_on, (tend_brand, pick_and_drop, toggle_overlay, player_input).chain().run_if(no_modal)).chain().in_set(EngineSet::Input))
        .add_systems(Update, set_ambient.after(EngineSet::Turns).before(EngineSet::Light).run_if(in_state(EngineState::Playing)))
        // A floor fills the moment it is entered, inside the turn.
        .add_systems(Turn, populate_floor.in_set(TurnSet::React))
        .add_systems(Update, (narrate, narrate_knacks, narrate_items).in_set(PresentSet::Narrate));
    app.run()
}

/// The screen, cut once so the map and every panel agree on it: a rail down
/// the right for vitals and what is in sight, the log under the map, and the
/// screens that cover the map while they are up.
struct Screen {
    map: Rect,
    log: Rect,
    vitals: Rect,
    nearby: Rect,
    inspect: Rect,
    scrollback: Rect,
    target: Rect,
    knacks: Rect,
}

impl Screen {
    fn new() -> Self {
        let (left, rail) = panel::split_right(Rect::new(0, 0, COLS, ROWS), RAIL);
        let (map, log) = panel::split_bottom(left, LOG_ROWS);
        let (vitals, nearby) = panel::split_top(rail, 11);
        Self {
            map,
            log,
            vitals,
            nearby,
            inspect: Rect::new(map.x + 2, map.bottom() - 11, map.width.min(50), 10),
            scrollback: map.inflate(-2),
            target: Rect::new(map.x, map.bottom() - 1, map.width, 1),
            knacks: Rect::new(map.x + map.width / 2 - 18, map.y + 4, 36, 14),
        }
    }
}

/// Every registry the delve's content names, filled before anything is
/// loaded against them.
fn registries() -> Registries {
    let damage_kinds = Registry::from_defs(vec![
        DamageKind::new("bite"),
        DamageKind::new("blade"),
        DamageKind::new("blunt"),
        DamageKind::new("bile"),
        DamageKind::new("fire").unarmored(),
        DamageKind::new("care").unarmored(),
    ])
    .unwrap();
    let fire = damage_kinds.expect("fire");
    let statuses = Registry::from_defs(vec![
        StatusDef { badge: Some('s'), ..StatusDef::new("scorched").ticks(fire, 1) },
        StatusDef { badge: Some('z'), ..StatusDef::new("dazed") },
    ])
    .unwrap();
    let gases = Registry::from_defs(vec![
        // What burning flesh gives off: thick enough to hide in while it hangs.
        GasDef::new("smoke").spread(55).fade(9).veils_at(70),
        // The reek off a pool of bile. It burns, and a lungful makes the head swim.
        GasDef::new("reek").spread(35).fade(12).burns().inflicts(60, statuses.expect("dazed"), 1),
    ])
    .unwrap();
    Registries {
        factions: Registry::from_defs(vec![FactionDef::new("you"), FactionDef::new("whale")]).unwrap(),
        stats: Registry::from_defs(vec![StatDef::new("mana", 30)]).unwrap(),
        statuses,
        tags: Registry::from_defs(vec![TagDef::new("shield")]).unwrap(),
        slots: Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("off hand")]).unwrap(),
        gases,
        damage_kinds,
    }
}

/// The player, and only while it holds the turn.
type PlayerHolding = (With<Player>, With<MyTurn>);

/// Marks a flame to carry, a torch or a lamp, so `d` knows which carried
/// thing to set down.
#[derive(Component, Clone, Copy)]
struct Torch;

/// The floor the run starts on.
#[derive(Resource, Clone, Copy)]
struct FirstFloor(u32);

/// What the player's brand sheds: a whale-oil flame.
const BRAND: LightSource = LightSource::new(200, 8, Rgb::new(255, 190, 120)).flickering(60);
/// The light a torch or a whaler's lamp sheds.
const FLAME: Rgb = Rgb::new(255, 170, 90);
/// Columns given to the rail down the right.
const RAIL: i32 = 26;
/// The keys, in two lines because one ran past the edge of the log.
const KEY_HINTS: [&str; 2] = ["1-5 knacks  a list  x look  p log", "g get  d drop torch  L brand  v light"];
/// The delver's knacks and the one a beast has, compiled in.
const ABILITIES_RON: &str = include_str!("../assets/abilities.ron");
/// What the player knows, in the order `1` to `5` aim them.
const KNACKS: [&str; 5] = ["fireball", "blink", "mend", "drain", "shield bash"];

/// One kind of beast, as authored.
#[derive(Debug, Clone, Deserialize)]
struct BeastDef {
    name: String,
    glyph: char,
    color: (f32, f32, f32),
    hp: i32,
    armor: i32,
    attack: DiceRoll,
    wits: Wits,
    perception: i32,
    speed: u32,
    flee_at: i32,
    spawn: (i32, i32, u32, u32, u32),
    #[serde(default)]
    glow: Option<LightSource>,
    #[serde(default)]
    dark_sight: Option<i32>,
    #[serde(default)]
    notice: Option<NoticeStats>,
    #[serde(default)]
    abilities: Vec<NameRef<AbilityDef>>,
}

impl Named for BeastDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Marks a beast with its kind.
#[derive(Component, Clone, Copy)]
struct Kind(Id<BeastDef>);

/// The bestiary and its spawn table.
#[derive(Resource)]
struct Beasts {
    defs: Registry<BeastDef>,
    table: BandedTable<Id<BeastDef>>,
    brains: Vec<Arc<Brain<Entity>>>,
    bite: rl_engine::rl_rules::damage::DamageKindId,
    whale: rl_engine::rl_rules::FactionId,
}

impl Beasts {
    fn spawn(&self, commands: &mut Commands, id: Id<BeastDef>, at: Point) -> Entity {
        let d = self.defs.get(id);
        let mut beast = commands.spawn((
            (Actor, Blocks, Position(at), Health::full(d.hp), Armor(d.armor), Faction(self.whale)),
            (
                MeleeAttack { kind: self.bite, dice: d.attack },
                Perception(d.perception),
                Speed(d.speed),
                Mind(self.brains[id.index()].clone()),
                Intelligence(d.wits),
                Kind(id),
                Name::new(d.name.clone()),
                Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(5),
            ),
        ));
        if !d.abilities.is_empty() {
            beast.insert(Grants(d.abilities.iter().map(|a| a.id()).collect()));
        }
        if let Some(glow) = d.glow {
            beast.insert(glow);
        }
        if let Some(reach) = d.dark_sight {
            beast.insert(DarkSight(reach));
        }
        if let Some(notice) = d.notice {
            beast.insert(Notice(notice));
        }
        beast.id()
    }
}

/// Builds the rules, spawns the player, and sends it down the whale's throat.
fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    first: Option<Res<FirstFloor>>,
    mut warps: MessageWriter<WarpRequest>,
    mut log: ResMut<MessageLog>,
    mut next: ResMut<NextState<EngineState>>,
    effect_kinds: Res<EffectKinds>,
) {
    let whale = Whale::new(seed.0);
    let registries = registries();
    // The knacks, built against the registries and the effects registered
    // while the app was built, so a file naming one nobody added fails here.
    let abilities = Abilities::load(ABILITIES_RON, &effect_kinds, &registries.names()).unwrap_or_else(|e| panic!("assets/abilities.ron: {e}"));
    let (you, whale_side) = (registries.factions.expect("you"), registries.factions.expect("whale"));
    let combat = CombatRules::new(&registries.factions).hostile(you, whale_side);
    // A beast names the knacks it knows, so the bestiary loads against them too.
    let defs: Registry<BeastDef> = registries.names().with("ability", abilities.defs()).load(BEASTS_RON).unwrap_or_else(|e| panic!("assets/beasts.ron: {e}"));
    let mut table = BandedTable::default();
    let mut brains = Vec::new();
    for (id, b) in defs.iter() {
        let (lo, hi, w, gmin, gmax) = b.spawn;
        table.push(BandedEntry::new(id).bands(lo, hi).weight(w).group(gmin, gmax));
        // A beast that knows an ability leads with it when one is worth firing.
        let mut brain = if b.abilities.is_empty() { Brain::new() } else { Brain::new().then(UseAbility { chance_pct: 40 }) }.then(MeleeAdjacent);
        if b.flee_at > 0 {
            brain = brain.then(FleeWhenHurt { at_pct: b.flee_at });
        }
        // Hunt what it sees, search where it last saw you, then drift.
        brains.push(Arc::new(brain.then(Hunt).then(SearchLastKnown).then(Wander { chance_pct: 30 })));
    }
    commands.insert_resource(Beasts { defs, table, brains, bite: registries.damage_kinds.expect("bite"), whale: whale_side });
    commands.insert_resource(combat);
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    // Standing in fire scorches, and burning flesh smokes.
    commands.insert_resource(FireRules::new().inflicts(registries.statuses.expect("scorched"), 3).smoke(registries.gases.expect("smoke"), 30));
    commands.insert_resource(whale.appearance());
    commands.insert_resource(WorldMap::new(whale.tiles().tables()));
    commands.insert_resource(Bile(whale.bile()));
    commands.insert_resource(PlaceRulesRes(Box::new(whale)));
    // The lights go off; each floor sets its own ambient as it is entered.
    commands.insert_resource(Lighting::dark());

    // A shield on the arm, which is what shield bash asks the slot graph for.
    let off_hand = registries.slots.expect("off hand");
    let shield = commands
        .spawn((
            Item,
            Name::new("a whalebone shield"),
            Tagged(vec![registries.tags.expect("shield")]),
            Wearable(EquipShape::in_slot(off_hand)),
            Glyph::new(']', Color::srgb(0.9, 0.88, 0.8)),
        ))
        .id();
    let mut worn = Equipment::with_slot_count(registries.slots.len());
    worn.equip(shield, &EquipShape::in_slot(off_hand)).expect("one shield, one arm");
    let mana = registries.stats.expect("mana");
    let mut pools = Pools::new();
    pools.set(mana, registries.stats.get(mana).base);
    let knacks: Vec<AbilityId> = KNACKS.iter().map(|n| abilities.expect(n)).collect();
    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(30), RevealsMap),
            (
                Health::full(30),
                Armor(1),
                Faction(you),
                MeleeAttack { kind: registries.damage_kinds.expect("blade"), dice: DiceRoll::new(1, 6) },
                BRAND,
                // A brand burns down while it is lit, and keeps what is left
                // while it is smothered.
                Fuel(900),
                // Quiet, and a little subtle: enough that a beast has to be
                // close, or the brand has to be lit, before it is sure.
                Stealth(StealthStats { quiet: 1, subtlety: 10 }),
                Name::new("you"),
                Glyph::new('@', Color::WHITE).on_layer(10),
            ),
            (pools, Grants(knacks)),
            (Inventory { items: vec![shield] }, Equipped(worn)),
        ))
        .id();
    commands.insert_resource(abilities);
    commands.insert_resource(registries);
    // No surface: the first floor is the first place, and the run starts in it.
    warps.write(WarpRequest::into_place(player, map_of(first.map(|f| f.0).unwrap_or(1))));
    log.push(format!("Seed {}. The whale's jaw is propped open with a mast.", seed.0.0), Tones::NOTICE, 0);
    log.push("You light a brand and climb in.", Tones::NOTICE, 0);
    next.set(EngineState::Playing);
}

/// Each floor's own ambient, set between the turns and the light so the
/// frame the stairs land on is drawn in the right light.
fn set_ambient(map: Res<WorldMap>, mut lighting: ResMut<Lighting>) {
    let ambient = ambient_of(floor_of(map.current()));
    if lighting.ambient != ambient {
        lighting.ambient = ambient;
    }
}

/// The tile that glows, kept from the whale once it is handed to the engine.
#[derive(Resource, Clone, Copy)]
struct Bile(rl_engine::rl_grid::TileId);

/// What a floor is populated from.
#[derive(bevy::ecs::system::SystemParam)]
struct Stock<'w> {
    beasts: Res<'w, Beasts>,
    map: Res<'w, WorldMap>,
    bile: Res<'w, Bile>,
    seed: Res<'w, Seed>,
    first: Option<Res<'w, FirstFloor>>,
    registries: Res<'w, Registries>,
}

/// Stairs, glowing bile and beasts, the first time a floor is entered.
fn populate_floor(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, stock: Stock, turns: Res<Turns>, mut log: ResMut<MessageLog>) {
    let Stock { beasts, map, bile, seed, first, registries } = &stock;
    let reek = registries.gases.expect("reek");
    for ev in entered.read() {
        let floor = floor_of(ev.map);
        log.push(format!("Floor {floor}: {}.", name_of(floor)), Tones::NOTICE, turns.turn_number());
        // The keys, once, under the name of the floor the run starts on. Not
        // in `start`: the name is written when the warp lands, a frame later,
        // and would read as if it came after them.
        if ev.first && floor == first.as_ref().map_or(1, |f| f.0) {
            for line in KEY_HINTS {
                log.push(line, Tones::MUTED, turns.turn_number());
            }
        }
        if !ev.first {
            continue;
        }
        let stairs = Color::srgb(0.9, 0.9, 0.6);
        if floor > 1 {
            commands.spawn((
                Position(ev.entry),
                Transition { to: Destination::Place { map: map_of(floor - 1), arrive: Arrive::Exit } },
                Glyph::new('<', stairs).on_layer(1),
            ));
        }
        if let Some(exit) = ev.exit {
            commands.spawn((
                Position(exit),
                Transition { to: Destination::Place { map: map_of(floor + 1), arrive: Arrive::Entry } },
                Glyph::new('>', stairs).on_layer(1),
            ));
        }
        let Some(place) = map.place(ev.map) else { continue };
        // Bile glows: a faint steady source on every pooled tile. One pool in
        // four reeks as well, which is enough to fill a chamber without
        // drowning a floor.
        for (p, _) in place.terrain.iter().filter(|(_, t)| *t == bile.0) {
            let mut pool = commands.spawn((Position(p), LightSource::new(70, 2, Rgb::new(160, 255, 70))));
            if rl_engine::rl_core::seed::position_hash(seed.0.0, p.x, p.y).is_multiple_of(4) {
                pool.insert(Vents { gas: reek, amount: 16 });
            }
        }
        // A whaler's lamp, left burning by whoever came before. Brighter than
        // a torch and never running dry, and as free to carry off.
        let mut kit = seed.stream(b"whale.lamps", floor as u64);
        if let Some(at) = spot_between(place.terrain.bounds(), map, ev.entry, 6, 14, &mut kit) {
            commands.spawn((
                Position(at),
                Item,
                Torch,
                Name::new("a whaler's lamp"),
                LightSource::new(235, 9, FLAME).flickering(150),
                Glyph::new('*', Color::srgb(1.0, 0.75, 0.3)).on_layer(1),
            ));
        }
        // On the first floor, a torch to carry: lit where it lies, shed from
        // whoever picks it up, and lit on the floor again when set down.
        if floor == 1
            && let Some(at) = spot_between(place.terrain.bounds(), map, ev.entry, 2, 5, &mut kit)
        {
            commands.spawn((
                Position(at),
                Item,
                Torch,
                Name::new("a whaler's torch"),
                LightSource::new(190, 6, FLAME).flickering(120),
                Fuel(600),
                Glyph::new('!', Color::srgb(1.0, 0.7, 0.3)).on_layer(2),
            ));
        }
        // The warden stands where the heart's prefab marked it.
        for spot in place.spots.iter().filter(|s| s.tag == 'W' as u32) {
            beasts.spawn(&mut commands, beasts.defs.expect("heart warden"), spot.at);
        }
        let mut rng = seed.stream(b"whale.beasts", floor as u64);
        let bounds = place.terrain.bounds();
        let mut placed_groups = 0;
        for _ in 0..40 {
            if placed_groups >= 4 + floor as usize {
                break;
            }
            let Some((id, count)) = beasts.table.pick_group(floor as i32, &mut rng) else { break };
            let id = *id;
            let anchor = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            let mut placed = 0;
            for p in geometry::square(anchor, 2) {
                if placed >= count {
                    break;
                }
                if map.is_walkable(p) && geometry::chebyshev(p, ev.entry) >= 7 {
                    beasts.spawn(&mut commands, id, p);
                    placed += 1;
                }
            }
            if placed > 0 {
                placed_groups += 1;
            }
        }
    }
}

/// The player, while it holds the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position), (With<Player>, With<MyTurn>)>;

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
    stairs: MessageWriter<'w, Intent<GoThrough>>,
}

/// Keys to intents: walk, bump to attack, `.` to wait, `>` `<` or Enter for stairs, `q` to quit.
fn player_input(keys: Res<ButtonInput<KeyCode>>, occupancy: Res<Occupancy>, player: PlayerTurn, mut intents: PlayerIntents, mut exit: MessageWriter<AppExit>) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    let Ok((entity, pos)) = player.single() else { return };
    let shifted = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    // Shift and L is the brand, not a step east.
    if shifted && keys.just_pressed(KeyCode::KeyL) {
        return;
    }
    if let Some((_, dir)) = MOVES.iter().find(|(codes, _)| keys.any_just_pressed(codes.iter().copied())) {
        // Bump to attack: walking into someone is a strike.
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
    } else if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        intents.waits.write(Intent::new(entity, Wait));
    }
}

/// The player holding the turn, and whether its brand is lit.
type BrandBearer = (Entity, Has<LightSource>, Option<&'static Fuel>);

/// `L` smothers the brand or lights it again, and spends the turn.
///
/// Its own system rather than a branch of `player_input`, which is at the
/// argument limit, and because the brand is the one thing in the delve the
/// player chooses to be seen by.
fn tend_brand(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    player: Query<BrandBearer, (With<Player>, With<MyTurn>)>,
    mut waits: MessageWriter<Intent<Wait>>,
    mut log: ResMut<MessageLog>,
    turns: Res<Turns>,
) {
    let shifted = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if !(shifted && keys.just_pressed(KeyCode::KeyL)) {
        return;
    }
    let Ok((entity, lit, fuel)) = player.single() else { return };
    if !lit && fuel.is_some_and(|f| f.0 == 0) {
        log.bad("The brand is spent. There is nothing left in it to light.", turns.turn_number());
        return;
    }
    if lit {
        commands.entity(entity).remove::<LightSource>();
        log.muted("You smother the brand. The dark closes in, and hides you.", turns.turn_number());
    } else {
        commands.entity(entity).insert(BRAND);
        log.notice("The brand catches again.", turns.turn_number());
    }
    waits.write(Intent::new(entity, Wait));
}

/// A walkable tile between `min` and `max` steps from `entry`, or `None` when
/// a floor has no such tile after a fair number of tries.
fn spot_between(bounds: Rect, map: &WorldMap, entry: Point, min: i32, max: i32, rng: &mut impl Rng) -> Option<Point> {
    (0..400)
        .map(|_| Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom())))
        .find(|p| map.is_walkable(*p) && (min..=max).contains(&geometry::chebyshev(*p, entry)))
}

/// What picking up and setting down read and write.
#[derive(bevy::ecs::system::SystemParam)]
struct Hands<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    map: Res<'w, WorldMap>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
    player: Query<'w, 's, (Entity, &'static Position, &'static Inventory), PlayerHolding>,
    torches: Query<'w, 's, (), With<Torch>>,
    ground: Query<'w, 's, (&'static Position, Option<&'static OnMap>), With<Item>>,
    picks: MessageWriter<'w, Intent<PickUp>>,
    drops: MessageWriter<'w, Intent<DropItem>>,
}

/// `g` picks up whatever lies here; `d` sets a carried flame down, where it
/// goes on lighting the floor. Either says so when there is nothing to do,
/// rather than a key that does nothing.
fn pick_and_drop(mut hands: Hands) {
    let Ok((me, pos, bag)) = hands.player.single() else { return };
    let turn = hands.turns.turn_number();
    if hands.keys.just_pressed(KeyCode::KeyG) {
        let here = hands.map.current();
        if hands.ground.iter().any(|(p, on)| p.0 == pos.0 && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here) {
            hands.picks.write(Intent::new(me, PickUp));
        } else {
            hands.log.muted("There is nothing here to pick up.", turn);
        }
    } else if hands.keys.just_pressed(KeyCode::KeyD) {
        match bag.items.iter().copied().find(|i| hands.torches.contains(*i)) {
            Some(flame) => {
                hands.drops.write(Intent::new(me, DropItem(flame)));
            }
            None => hands.log.muted("You carry no flame to set down.", turn),
        }
    }
}

/// `v` draws the light on each tile as a digit, which is how a dark floor is
/// read when the shading alone is too subtle to judge.
fn toggle_overlay(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<LightOverlay>) {
    if keys.just_pressed(KeyCode::KeyV) {
        overlay.0 = !overlay.0;
    }
}

/// `1` to `5` aim the knacks in order; `a` lists them with the reasons any is
/// out of reach. A key writes `AimAt` and stops: the cursor, the preview and
/// the use are the engine's.
fn call_on(keys: Res<ButtonInput<KeyCode>>, mut modals: ResMut<Modals>, player: Query<(Entity, &Known), PlayerHolding>, mut aims: MessageWriter<AimAt>) {
    const SLOTS: [KeyCode; 5] = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5];
    let list = ability_modal(&modals);
    if keys.just_pressed(KeyCode::KeyA) && (modals.is_top(list) || !modals.any_open()) {
        modals.toggle(list);
        return;
    }
    if modals.any_open() {
        return;
    }
    let Ok((user, known)) = player.single() else { return };
    if let Some(slot) = SLOTS.iter().position(|k| keys.just_pressed(*k))
        && let Some((ability, _)) = known.iter().nth(slot)
    {
        aims.write(AimAt { user, ability });
    }
}

/// Knacks used and refused, a brand or a torch going out, and standing in
/// fire.
fn narrate_knacks(
    mut used: MessageReader<AbilityEvent>,
    mut lights: MessageReader<LightEvent>,
    mut fires: MessageReader<FireEvent>,
    abilities: Res<Abilities>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
    names: Query<(&Name, Has<Player>)>,
) {
    let turn = turns.turn_number();
    let named = |e: Entity| names.get(e).map(|(n, _)| n.as_str().to_string()).unwrap_or_else(|_| "something".into());
    let is_you = |e: Entity| names.get(e).is_ok_and(|(_, you)| you);
    for ev in used.read() {
        match ev {
            AbilityEvent::Used { user, ability, targets, .. } => {
                let what = &abilities.get(*ability).name;
                let (who, verb) = if is_you(*user) { ("You".to_string(), "use") } else { (upper_first(&named(*user)), "uses") };
                let line = match targets.as_slice() {
                    [] => format!("{who} {verb} {what}."),
                    [one] => format!("{who} {verb} {what} on {}.", named(*one)),
                    many => format!("{who} {verb} {what}, catching {}.", many.len()),
                };
                log.push(line, if is_you(*user) { Tones::TEXT } else { Tones::BAD }, turn);
            }
            AbilityEvent::Refused { user, ability, .. } if is_you(*user) => {
                log.bad(format!("You cannot use {} right now; `a` says why.", abilities.get(*ability).name), turn);
            }
            AbilityEvent::Refused { .. } => {}
        }
    }
    for ev in lights.read() {
        let LightEvent::BurntOut { entity } = *ev;
        let what = if is_you(entity) { "Your brand".to_string() } else { upper_first(&named(entity)) };
        log.bad(format!("{what} gutters and goes out."), turn);
    }
    for ev in fires.read() {
        if let FireEvent::Scorched { entity, .. } = *ev
            && is_you(entity)
        {
            log.bad("You are standing in fire.", turn);
        }
    }
}

/// What the player picks up and sets down.
fn narrate_items(mut items: MessageReader<ItemEvent>, turns: Res<Turns>, mut log: ResMut<MessageLog>, names: Query<&Name>, players: Query<(), With<Player>>) {
    let turn = turns.turn_number();
    let named = |e: Entity| names.get(e).map(|n| n.as_str().to_string()).unwrap_or_else(|_| "something".into());
    for ev in items.read() {
        match *ev {
            ItemEvent::PickedUp { actor, item, merged_into } if players.contains(actor) => {
                log.push(format!("You pick up {}.", named(merged_into.unwrap_or(item))), Tones::TEXT, turn);
            }
            ItemEvent::Dropped { actor, item, .. } if players.contains(actor) => {
                log.push(format!("You set down {}.", named(item)), Tones::TEXT, turn);
            }
            _ => {}
        }
    }
}

fn upper_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The player's pools as bars beside health: a pool is a quantity with a
/// maximum, so the panel draws it without learning what mana is.
fn show_pools(mut vitals: ResMut<VitalsView>, registries: Res<Registries>, player: Query<(&Pools, &StatBlock), With<Player>>) {
    let Ok((pools, stats)) = player.single() else { return };
    for (id, def) in registries.stats.iter() {
        vitals.bars.push(Bar::new(def.name.clone(), pools.get(id), stats.0.value(id, &registries.stats), Tones::NOTICE));
    }
}

/// What narration reads and writes.
#[derive(bevy::ecs::system::SystemParam)]
struct Voice<'w, 's> {
    beasts: Res<'w, Beasts>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
    next: ResMut<'w, NextState<EngineState>>,
    kinds: Query<'w, 's, &'static Kind>,
    players: Query<'w, 's, (), With<Player>>,
}

fn narrate(mut dealt: MessageReader<DamageDealt>, mut deaths: MessageReader<DeathEvent>, mut voice: Voice) {
    let Voice { beasts, turns, log, next, kinds, players } = &mut voice;
    let turn = turns.turn_number();
    let name = |e: Entity| -> String {
        if players.get(e).is_ok() {
            "you".into()
        } else {
            kinds.get(e).map(|k| format!("the {}", beasts.defs.get(k.0).name)).unwrap_or_else(|_| "something".into())
        }
    };
    for d in dealt.read() {
        let attacker = d.hit.attacker.map(name).unwrap_or_else(|| "something".into());
        let target = name(d.target);
        let (verb, cat) = if attacker == "you" { ("hit", Tones::TEXT) } else { ("hits", Tones::BAD) };
        let mut line = format!("{}{} {verb} {target}", attacker[..1].to_uppercase(), &attacker[1..]);
        line.push_str(&if d.dealt <= 0 { " but does nothing.".to_string() } else { format!(" for {}.", d.dealt) });
        log.push(line, cat, turn);
    }
    for d in deaths.read() {
        if d.was_player {
            log.push("The whale keeps you. Press q to quit.", Tones::BAD, turn);
            next.set(EngineState::Idle);
        } else if kinds.get(d.entity).is_ok_and(|k| beasts.defs.get(k.0).name == "heart warden") {
            log.push("The heart stops. The whale shudders, and daylight opens above you.", Tones::NOTICE, turn);
            log.push("You have won. Press q to quit.", Tones::NOTICE, turn);
            next.set(EngineState::Idle);
        } else {
            log.push(
                format!("{} dies.", {
                    let n = name(d.entity);
                    format!("{}{}", n[..1].to_uppercase(), &n[1..])
                }),
                Tones::GOOD,
                turn,
            );
        }
    }
}

fn note_floor(mut vitals: ResMut<VitalsView>, mut facets: ResMut<Facets>, map: Res<WorldMap>, lighting: Res<Lighting>, player: Query<&Position, With<Player>>) {
    let floor = floor_of(map.current());
    vitals.facets.push(facets.facet("floor", format!("floor {floor} of {FLOORS}: {}", name_of(floor))));
    if let Ok(pos) = player.single() {
        vitals.facets.push(facets.facet("light", format!("light here {}", lighting.at(pos.0).intensity)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A headless whale: the engine plugins and the delve's own systems.
    fn headless(seed: u64) -> App {
        let mut app = rl_engine::rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, StatusPlugin, ItemsPlugin, LightingPlugin, AbilitiesPlugin, FirePlugin, GasPlugin));
        app.add_engine_effects().add_effect::<effects::Drain>();
        app.insert_resource(Seed(RunSeed(seed)))
            .add_plugins(UiPlugin)
            .add_systems(Startup, start)
            .add_systems(Turn, populate_floor.in_set(TurnSet::React))
            .add_systems(Update, narrate.in_set(PresentSet::Narrate));
        app
    }

    #[test]
    fn the_stairs_lead_down_to_the_heart_and_the_warden_waits_there() {
        let mut app = headless(7);
        app.update();
        app.update();
        let mut q = app.world_mut().query_filtered::<Entity, With<Player>>();
        let player = q.single(app.world()).unwrap();
        assert_eq!(app.world().resource::<WorldMap>().current(), map_of(1), "the run starts in the Maw");
        for floor in 1..FLOORS {
            let exit = app.world().resource::<WorldMap>().place(map_of(floor)).unwrap().exit.expect("stairs down");
            app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: map_of(floor), arrive: Arrive::At(exit) } });
            app.update();
            app.world_mut().write_message(Intent::new(player, GoThrough));
            app.update();
            assert_eq!(app.world().resource::<WorldMap>().current(), map_of(floor + 1), "took the stairs from floor {floor}");
        }
        // One more frame: what the heart floor spawned gets its map tag.
        app.update();
        let mut wardens = app.world_mut().query::<(&Kind, &OnMap)>();
        let beasts = app.world().resource::<Beasts>();
        let on_heart = wardens.iter(app.world()).filter(|(k, on)| beasts.defs.get(k.0).name == "heart warden" && on.0 == map_of(FLOORS)).count();
        assert_eq!(on_heart, 1);
        // And back up: the stairs up land on the floor above's stairs down.
        app.world_mut().write_message(Intent::new(player, GoThrough));
        app.update();
        assert_eq!(app.world().resource::<WorldMap>().current(), map_of(FLOORS - 1));
    }

    /// A run settled into the Maw with the player holding its turn.
    fn settled(seed: u64) -> (App, Entity) {
        let mut app = headless(seed);
        for _ in 0..3 {
            app.update();
        }
        let player = {
            let w = app.world_mut();
            let mut q = w.query_filtered::<Entity, With<Player>>();
            q.single(w).unwrap()
        };
        (app, player)
    }

    /// A beast of `kind` on a free tile beside the player.
    fn beside(app: &mut App, player: Entity, kind: &str) -> Entity {
        let at = app.world().get::<Position>(player).unwrap().0;
        let free = Direction::ALL
            .into_iter()
            .map(|d| at + d.offset())
            .find(|p| app.world().resource::<WorldMap>().is_walkable(*p) && !app.world().resource::<Occupancy>().is_occupied(*p))
            .expect("room beside the player");
        let beast = app.world_mut().resource_scope(|world: &mut World, beasts: Mut<Beasts>| {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            let e = beasts.spawn(&mut commands, beasts.defs.expect(kind), free);
            queue.apply(world);
            e
        });
        app.update();
        beast
    }

    fn use_knack(app: &mut App, player: Entity, name: &str, aim: Point) {
        let ability = app.world().resource::<Abilities>().expect(name);
        app.world_mut().write_message(Intent::new(player, Use { ability, aim }));
        app.update();
    }

    /// The log reads in the order things happened: the climb, the floor it
    /// lands on, and only then the keys.
    #[test]
    fn the_first_floor_is_named_before_the_keys_are_listed() {
        let (app, _) = settled(7);
        let lines: Vec<&str> = app.world().resource::<MessageLog>().iter().map(|e| e.text.as_str()).collect();
        let at = |needle: &str| lines.iter().position(|l| l.starts_with(needle)).unwrap_or_else(|| panic!("no {needle:?} in {lines:#?}"));
        assert!(at("You light a brand") < at("Floor 1:"), "{lines:#?}");
        assert!(at("Floor 1:") < at(KEY_HINTS[0]), "{lines:#?}");
        assert_eq!(lines[at(KEY_HINTS[0])..at(KEY_HINTS[0]) + 2], KEY_HINTS, "both key lines, together and once");
        assert_eq!(lines.iter().filter(|l| **l == KEY_HINTS[0]).count(), 1);
    }

    /// Waits `turns` whole turns, as the player.
    fn wait(app: &mut App, player: Entity, turns: usize) {
        for _ in 0..turns {
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
        }
    }

    /// A fireball is fire: the fat it lands on catches and burns away to
    /// cinder, and what it scorches goes on burning after the blast.
    #[test]
    fn a_fireball_sets_the_fat_it_lands_on_alight_and_it_burns_to_cinder() {
        let (mut app, player) = settled(7);
        let at = app.world().get::<Position>(player).unwrap().0;
        let whale = Whale::new(RunSeed(7));
        let (tallow, cinder) = (whale.tiles().expect("tallow"), whale.tiles().expect("cinder"));
        let open = |app: &App, p: Point| app.world().resource::<WorldMap>().is_walkable(p) && !app.world().resource::<Occupancy>().is_occupied(p);
        let along = |dir: Direction, n: i32| {
            let (dx, dy) = dir.delta();
            at.offset(dx * n, dy * n)
        };
        let side = Direction::ALL.into_iter().find(|d| !d.is_diagonal() && (1..=6).all(|n| open(&app, along(*d, n)))).expect("a clear run from the player");
        let slick: Vec<Point> = (3..=5).map(|n| along(side, n)).collect();
        for p in &slick {
            app.world_mut().resource_mut::<WorldMap>().set_tile(*p, tallow);
        }
        app.update();

        use_knack(&mut app, player, "fireball", slick[1]);
        assert!(slick.iter().all(|p| app.world().resource::<Fire>().is_burning(*p)), "the fat caught");
        wait(&mut app, player, 8);
        let map = app.world().resource::<WorldMap>();
        assert!(slick.iter().all(|p| map.tile(*p) == Some(cinder)), "and burnt to cinder");
        assert_eq!(app.world().resource::<Fire>().burning().count(), 0, "and went out");
    }

    /// A reeking pool of bile fills the air over it with a gas that burns.
    #[test]
    fn a_reeking_pool_fills_the_air_over_it() {
        let (mut app, player) = settled(7);
        app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: map_of(3), arrive: Arrive::Entry } });
        app.update();
        app.update();
        let vent = {
            let w = app.world_mut();
            let mut q = w.query::<(&Position, &Vents, &OnMap)>();
            q.iter(w).find(|(_, _, on)| on.0 == map_of(3)).map(|(p, v, _)| (p.0, v.gas)).expect("the Stomach has a reeking pool")
        };
        wait(&mut app, player, 4);
        let (at, reek) = vent;
        let gases = app.world().resource::<Gases>();
        assert!(gases.at(reek, at) > 0, "the air over the pool reeks");
        assert!(app.world().resource::<Registries>().gases.get(reek).burns, "and what it reeks of burns");
    }

    #[test]
    fn drain_hurts_what_it_hits_and_pours_mana_back_into_the_pool() {
        let (mut app, player) = settled(7);
        let mana = app.world().resource::<Registries>().stats.expect("mana");
        app.world_mut().get_mut::<Pools>(player).unwrap().set(mana, 10);
        let crab = beside(&mut app, player, "stomach crab");
        let at = app.world().get::<Position>(crab).unwrap().0;

        use_knack(&mut app, player, "drain", at);
        assert!(app.world().get::<Health>(crab).is_none_or(|h| h.hp < 9), "fire past a crab's armor always lands");
        assert!(app.world().get::<Pools>(player).unwrap().get(mana) > 10, "and the mana came back");
    }

    /// A requirement that reads the slot graph: with the shield on the arm
    /// the bash spends the turn; without it the gate refuses, for free.
    #[test]
    fn shield_bash_needs_the_shield_the_slot_graph_says_is_on_the_arm() {
        let (mut armed, player) = settled(7);
        let crab = beside(&mut armed, player, "stomach crab");
        let at = armed.world().get::<Position>(crab).unwrap().0;
        let clock = armed.world().resource::<Turns>().now();
        use_knack(&mut armed, player, "shield bash", at);
        assert!(armed.world().resource::<Turns>().now() > clock, "with the shield, the bash spent the turn");

        let (mut bare, player) = settled(7);
        let crab = beside(&mut bare, player, "stomach crab");
        let at = bare.world().get::<Position>(crab).unwrap().0;
        let worn: Vec<Entity> = bare.world().get::<Equipped>(player).unwrap().0.worn().map(|(_, i)| i).collect();
        for item in worn {
            bare.world_mut().get_mut::<Equipped>(player).unwrap().0.unequip(item);
        }
        let clock = bare.world().resource::<Turns>().now();
        use_knack(&mut bare, player, "shield bash", at);
        assert_eq!(bare.world().resource::<Turns>().now(), clock, "without it, refused and no time passed");
    }

    #[test]
    fn a_carried_torch_lights_its_bearer_and_a_dropped_one_lights_the_floor() {
        let (mut app, player) = settled(7);
        let torch = {
            let w = app.world_mut();
            let mut q = w.query_filtered::<(Entity, &Name), With<Torch>>();
            q.iter(w).find(|(_, n)| n.as_str() == "a whaler's torch").map(|(e, _)| e).expect("a torch on the first floor")
        };
        let at = app.world().get::<Position>(torch).unwrap().0;
        // Smother the brand, so the only flame near the player is the torch.
        app.world_mut().entity_mut(player).remove::<LightSource>();
        app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: map_of(1), arrive: Arrive::At(at) } });
        app.update();
        let ambient = ambient_of(1).intensity;

        app.world_mut().write_message(Intent::new(player, PickUp));
        app.update();
        app.update();
        assert!(app.world().get::<Inventory>(player).unwrap().contains(torch), "picked up");
        assert!(app.world().get::<Position>(torch).is_none(), "and off the floor");
        let here = app.world().get::<Position>(player).unwrap().0;
        assert!(app.world().resource::<Lighting>().at(here).intensity > ambient, "the bearer is lit by what it carries");

        app.world_mut().write_message(Intent::new(player, DropItem(torch)));
        app.update();
        app.update();
        assert_eq!(app.world().get::<Position>(torch).map(|p| p.0), Some(here), "set down where the player stands");
        assert!(app.world().get::<LightSource>(torch).is_some(), "and still burning on the floor");
    }

    #[test]
    fn the_whalers_lamp_can_be_picked_up_carried_and_set_down_again() {
        let (mut app, player) = settled(7);
        let lamp = {
            let w = app.world_mut();
            let mut q = w.query::<(Entity, &Name)>();
            q.iter(w).find(|(_, n)| n.as_str() == "a whaler's lamp").map(|(e, _)| e).expect("a lamp on the first floor")
        };
        let at = app.world().get::<Position>(lamp).unwrap().0;
        app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: map_of(1), arrive: Arrive::At(at) } });
        app.update();

        app.world_mut().write_message(Intent::new(player, PickUp));
        app.update();
        app.update();
        assert!(app.world().get::<Inventory>(player).unwrap().contains(lamp), "picked up");
        assert!(app.world().get::<Position>(lamp).is_none(), "and off the floor");

        // `d` sets down whichever flame is carried, the lamp as much as a torch.
        {
            use bevy::ecs::system::RunSystemOnce;
            let mut keys = ButtonInput::<KeyCode>::default();
            keys.press(KeyCode::KeyD);
            app.insert_resource(keys);
            app.world_mut().run_system_once(pick_and_drop).expect("the drop key ran");
        }
        app.update();
        app.update();
        let here = app.world().get::<Position>(player).unwrap().0;
        assert_eq!(app.world().get::<Position>(lamp).map(|p| p.0), Some(here), "set down again");
    }

    #[test]
    fn a_brand_that_runs_dry_goes_out_and_will_not_light_again() {
        use bevy::ecs::system::RunSystemOnce;
        let (mut app, player) = settled(7);
        app.world_mut().entity_mut(player).insert(Fuel(2));
        for _ in 0..3 {
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
        }
        assert!(app.world().get::<LightSource>(player).is_none(), "burnt out");
        assert_eq!(app.world().get::<Fuel>(player), Some(&Fuel(0)));

        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::ShiftLeft);
        keys.press(KeyCode::KeyL);
        app.insert_resource(keys);
        app.world_mut().run_system_once(tend_brand).expect("the brand key ran");
        app.update();
        assert!(app.world().get::<LightSource>(player).is_none(), "a spent brand does not catch again");
    }
}
