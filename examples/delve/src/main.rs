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

mod floors;

use std::sync::Arc;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_rules::ai::awareness::{NoticeStats, StealthStats};
use rl_engine::rl_rules::ai::tactics::SearchLastKnown;
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
    let mut seed = RunSeed::fresh();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--seed") {
        seed = RunSeed(args[i + 1].parse().expect("seed"));
    }
    let first = args.iter().position(|a| a == "--floor").map(|i| args[i + 1].parse::<u32>().expect("floor").clamp(1, FLOORS)).unwrap_or(1);
    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("The Hollow Whale", COLS, ROWS).map(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        .add_plugins((CombatPlugin, LightingPlugin))
        .insert_resource(Seed(seed))
        .insert_resource(FirstFloor(first))
        .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)).hints("[>] down [<] up [.] wait [q]uit"))
        .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
        .add_systems(Update, note_floor.in_set(ViewSet::Annotate))
        .add_systems(Startup, start)
        .add_systems(Update, (tend_brand, player_input).chain().in_set(EngineSet::Input))
        .add_systems(Update, set_ambient.after(EngineSet::Turns).before(EngineSet::Light).run_if(in_state(EngineState::Playing)))
        // A floor fills the moment it is entered, inside the turn.
        .add_systems(Turn, populate_floor.in_set(TurnSet::React))
        .add_systems(Update, narrate.in_set(PresentSet::Narrate));
    app.add_plugins(StealthPlugin);
    app.run()
}

#[derive(Resource, Clone, Copy)]
struct Seed(RunSeed);

/// The floor the run starts on.
#[derive(Resource, Clone, Copy)]
struct FirstFloor(u32);

/// What the player's brand sheds: a whale-oil flame.
const BRAND: LightSource = LightSource::new(200, 8, Rgb::new(255, 190, 120)).flickering(60);

/// One kind of beast, as authored.
#[derive(Debug, Clone, Deserialize)]
struct BeastDef {
    name: String,
    glyph: char,
    color: (f32, f32, f32),
    hp: i32,
    armor: i32,
    attack: DiceRoll,
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
                Kind(id),
                Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(5),
            ),
        ));
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
) {
    let whale = Whale::new(seed.0);
    let kinds = Registry::from_defs(vec![DamageKind::new("bite"), DamageKind::new("blade")]).unwrap();
    let facs = Registry::from_defs(vec![FactionDef { name: "you".into() }, FactionDef { name: "whale".into() }]).unwrap();
    let (you, whale_side) = (facs.expect("you"), facs.expect("whale"));
    let mut factions = Factions::new(&facs);
    factions.set_mutual(you, whale_side, Relation::Hostile);
    let defs: Registry<BeastDef> = Registry::from_ron_str(BEASTS_RON).unwrap_or_else(|e| panic!("assets/beasts.ron: {e}"));
    let mut table = BandedTable::default();
    let mut brains = Vec::new();
    for (id, b) in defs.iter() {
        let (lo, hi, w, gmin, gmax) = b.spawn;
        table.push(BandedEntry::new(id).bands(lo, hi).weight(w).group(gmin, gmax));
        let mut brain = Brain::new().then(MeleeAdjacent);
        if b.flee_at > 0 {
            brain = brain.then(FleeWhenHurt { at_pct: b.flee_at });
        }
        // Hunt what it sees, search where it last saw you, then drift.
        brains.push(Arc::new(brain.then(Hunt).then(SearchLastKnown).then(Wander { chance_pct: 30 })));
    }
    commands.insert_resource(Beasts { defs, table, brains, bite: kinds.expect("bite"), whale: whale_side });
    commands.insert_resource(CombatRules { kinds: kinds.clone(), factions });
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    commands.insert_resource(CombatRng::for_run(seed.0));
    commands.insert_resource(whale.appearance());
    commands.insert_resource(WorldMap::new(whale.tiles().tables()));
    commands.insert_resource(Bile(whale.bile()));
    commands.insert_resource(PlaceRulesRes(Box::new(whale)));
    // The lights go off; each floor sets its own ambient as it is entered.
    commands.insert_resource(Lighting::dark());

    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(30), RevealsMap),
            (
                Health::full(30),
                Armor(1),
                Faction(you),
                MeleeAttack { kind: kinds.expect("blade"), dice: DiceRoll::new(1, 6) },
                BRAND,
                // Quiet, and a little subtle: enough that a beast has to be
                // close, or the brand has to be lit, before it is sure.
                Stealth(StealthStats { quiet: 1, subtlety: 10 }),
                Glyph::new('@', Color::WHITE).on_layer(10),
            ),
        ))
        .id();
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
}

/// Stairs, glowing bile and beasts, the first time a floor is entered.
fn populate_floor(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, stock: Stock, turns: Res<Turns>, mut log: ResMut<MessageLog>) {
    let Stock { beasts, map, bile, seed } = &stock;
    for ev in entered.read() {
        let floor = floor_of(ev.map);
        log.push(format!("Floor {floor}: {}.", name_of(floor)), Tones::NOTICE, turns.turn_number());
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
        // Bile glows: a faint steady source on every pooled tile.
        for (p, _) in place.terrain.iter().filter(|(_, t)| *t == bile.0) {
            commands.spawn((Position(p), LightSource::new(70, 2, Rgb::new(160, 255, 70))));
        }
        // The warden stands where the heart's prefab marked it.
        for spot in place.spots.iter().filter(|s| s.tag == 'W' as u32) {
            beasts.spawn(&mut commands, beasts.defs.expect("heart warden"), spot.at);
        }
        let mut rng = seed.0.rng(SeedDomain::new(b"whale.beasts"), floor as u64);
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
type BrandBearer = (Entity, Has<LightSource>);

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
    let Ok((entity, lit)) = player.single() else { return };
    if lit {
        commands.entity(entity).remove::<LightSource>();
        log.muted("You smother the brand. The dark closes in, and hides you.", turns.turn_number());
    } else {
        commands.entity(entity).insert(BRAND);
        log.notice("The brand catches again.", turns.turn_number());
    }
    waits.write(Intent::new(entity, Wait));
}

/// Hits, deaths, and the end of the run either way.
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

fn note_floor(mut vitals: ResMut<VitalsView>, mut facets: ResMut<Facets>, map: Res<WorldMap>) {
    let floor = floor_of(map.current());
    vitals.facets.push(facets.facet("floor", format!("floor {floor} of {FLOORS}: {}", name_of(floor))));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A headless whale: the engine plugins and the delve's own systems.
    fn headless(seed: u64) -> App {
        let mut app = rl_engine::rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, CombatPlugin, LightingPlugin));
        app.insert_resource(Seed(RunSeed(seed)))
            .init_resource::<MessageLog>()
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
}
