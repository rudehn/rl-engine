//! The Counting House: three floors of a heist, in the dark.
//!
//! The engine's worked example of stealth and light. You are a thief in
//! a counting house after hours, from the cellars up through the counting
//! floor to the strongroom, and out of a window onto the roofs. The coin
//! you carry out is the score. The watch carry cudgels and you carry a
//! fist, so a fight is the thing that has gone wrong.
//!
//! What there is to do besides keep your distance: the lamps on the walls
//! are the only light, and `s` snuffs the one beside you, which is where
//! the watch cannot see you and where a sapient watchman will come to
//! relight it; `L` opens your shaded lantern, which shows you the room and
//! shows you to the room; `t` throws a pebble, and whoever hears it
//! clatter goes to look; a shut door stops sight and light, and a hound
//! cannot open one; and a watchman who spots you shouts, and everyone in
//! earshot comes.
//!
//! Every piece of that is an engine seam a game fills: a `Sense` the game
//! pushes for its own tactic, a `Choice` that tactic makes and the engine
//! routes to the game's own action, a shout of the game's own carried by
//! the engine's noise, and a `RunOver` with the score in it.
//!
//! `cargo run -p heist -- --seed 7`

mod floors;

use std::sync::Arc;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::Sight;
use rl_engine::rl_core::Rect;
use rl_engine::rl_rules::ai::awareness::{NoticeStats, StealthStats};
use rl_engine::rl_rules::ai::hearing::HearingStats;
use rl_engine::rl_rules::ai::tactics::{Hunt, MeleeAdjacent, SearchLastKnown, Wander};
use rl_engine::rl_rules::damage::SubtractArmor;
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::{Choice, Tactic, TacticCtx};
use serde::Deserialize;

use crate::floors::{FLOORS, House, floor_of, map_of, name_of};

const COLS: i32 = 90;
const ROWS: i32 = 44;
const LOG_ROWS: i32 = 4;
/// Columns given to the rail down the right.
const RAIL: i32 = 26;
/// The watch, compiled in so the binary runs from anywhere.
const WATCH_RON: &str = include_str!("../assets/watch.ron");
/// How loud the house is at night, in steps of bare boards. A thief's
/// step is heard by nothing but a hound close by, a pebble's clatter
/// carries as far as a shout, and a shut door takes three steps off either.
const NOISE: NoiseRules = NoiseRules { step: 2, strike: 6, door: 4, landing: 10, door_muffle: 3 };
/// A watchman's shout, or a hound's bay.
const SHOUT: i32 = 10;

fn main() -> AppExit {
    let screen = Screen::new();
    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("The Counting House", COLS, ROWS).map(screen.map))
        .add_plugins((CombatPlugin, MindsPlugin, ItemsPlugin, ThrowingPlugin, LightingPlugin, StealthPlugin, NoisePlugin::new(NOISE)))
        .add_sound("shout")
        .init_resource::<LightOverlay>()
        .insert_resource(Seed::from_args())
        .add_plugins((
            VitalsPanel::new(screen.vitals).heading("Thief").bars(10),
            NearbyPanel::new(screen.nearby).titled("").headings("In sight", "On the floor"),
            LogPanel::new(screen.log),
            InspectPanel::new(screen.inspect),
            ScrollbackPanel::new(screen.scrollback),
            TargetPanel::new(screen.target).hints("[enter] throw  [tab] next  [esc] back"),
            InventoryPanel::new(screen.pockets).title("Pockets").called("pockets").empty("Lint, and a plan."),
            ControlsPanel::new(screen.controls).hint(screen.hint),
            GameMenuPanel::new(screen.menu).title("The Counting House").died("The watch have you.").won("Over the roofs and away."),
        ))
        .add_plugins(NarratorPlugin::default().phrase(Phrase::NoticesYou, "{Who} has seen you!", Tones::BAD))
        .insert_resource(Lighting::dark())
        .add_systems(NewRun, start)
        .add_systems(Update, (tend_lantern, snuff, pick_up, throw, toggle_overlay, player_input).chain().run_if(no_modal).in_set(EngineSet::Input))
        .add_systems(Update, (note_take, note_light).in_set(ViewSet::Annotate))
        // The game's actions, alongside the engine's: snuffing a lamp and
        // escaping are the player's, relighting is a watchman's choice.
        .add_action::<Snuff>()
        .add_action::<Escape>()
        .add_choice::<Relight>()
        .add_systems(Turn, (resolve_snuffs, resolve_relights, resolve_escapes).in_set(ResolveSet::Act))
        // What a watchman knows that the engine does not: which lamps are out.
        .add_systems(Turn, notice_dark_lamps.in_set(PerceiveSet::Annotate))
        // What this turn caused, answered inside it: a floor fills on
        // arrival, and a watchman who sees you shouts.
        .add_systems(Turn, (populate_floor, raise_alarm).in_set(TurnSet::React))
        .add_systems(Update, show_the_take.in_set(ViewSet::Annotate))
        // After the narrator, so the pebble clatters after it is thrown.
        .add_systems(Update, (narrate_the_house, narrate_what_the_thief_hears).after(ViewSet::Speak).in_set(PresentSet::Narrate));
    declare_controls(&mut app);
    app.run()
}

/// The screen, cut once so the map and every panel agree on it.
struct Screen {
    map: Rect,
    log: Rect,
    vitals: Rect,
    nearby: Rect,
    inspect: Rect,
    scrollback: Rect,
    target: Rect,
    pockets: Rect,
    menu: Rect,
    controls: Rect,
    hint: Rect,
}

impl Screen {
    fn new() -> Self {
        let (left, rail) = panel::split_right(Rect::new(0, 0, COLS, ROWS), RAIL);
        let (map, log) = panel::split_bottom(left, LOG_ROWS);
        let (vitals, nearby) = panel::split_top(rail, 11);
        let (nearby, hint) = panel::split_bottom(nearby, 1);
        Self {
            map,
            log,
            vitals,
            nearby,
            inspect: Rect::new(map.x + 2, map.bottom() - 11, map.width.min(50), 10),
            scrollback: map.inflate(-2),
            target: Rect::new(map.x, map.bottom() - 1, map.width, 1),
            pockets: Rect::new(map.x + map.width / 2 - 24, map.y + 3, 48, 18),
            menu: Rect::new(map.x + map.width / 2 - 22, map.y + 6, 44, 14),
            controls: map.inflate(-2),
            hint,
        }
    }
}

/// Every registry the house's content names.
fn registries() -> Registries {
    Registries {
        damage_kinds: Registry::from_defs(vec![DamageKind::new("cudgel"), DamageKind::new("bite"), DamageKind::new("fist")]).unwrap(),
        factions: Registry::from_defs(vec![FactionDef::new("thief"), FactionDef::new("watch")]).unwrap(),
        ..Default::default()
    }
}

/// One kind of watcher, as `assets/watch.ron` writes it.
#[derive(Debug, Clone, Deserialize)]
struct WatchDef {
    name: String,
    glyph: char,
    color: (f32, f32, f32),
    hp: i32,
    armor: i32,
    attack: DiceRoll,
    kind: NameRef<DamageKind>,
    wits: Wits,
    perception: i32,
    dark_sight: i32,
    speed: u32,
    notice: NoticeStats,
    hearing: HearingStats,
    spawn: (i32, i32, u32, u32, u32),
}

impl Named for WatchDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Marks a watcher with its kind.
#[derive(Component, Clone, Copy)]
struct Kind(Id<WatchDef>);

/// The watch: the definitions, one brain per kind, and the table that says
/// who stands where.
#[derive(Resource)]
struct Watch {
    defs: Registry<WatchDef>,
    table: BandedTable<Id<WatchDef>>,
    brains: Vec<Arc<Brain<Entity>>>,
    faction: FactionId,
}

impl Watch {
    // ANCHOR: watch
    fn load(names: &Names, faction: FactionId) -> Self {
        let defs: Registry<WatchDef> = names.load(WATCH_RON).unwrap_or_else(|e| panic!("assets/watch.ron: {e}"));
        let mut table = BandedTable::default();
        let mut brains = Vec::new();
        for (id, def) in defs.iter() {
            let (lo, hi, w, gmin, gmax) = def.spawn;
            table.push(BandedEntry::new(id).bands(lo, hi).weight(w).group(gmin, gmax));
            // Strike what is in reach, hunt what is seen, search where it was
            // last seen; a watcher with hands relights the lamps on its round;
            // otherwise drift.
            let mut brain = Brain::new().then(MeleeAdjacent).then(Hunt).then(SearchLastKnown);
            if def.wits.has(Wits::OPENS_DOORS) {
                brain = brain.then(RelightLamps);
            }
            brains.push(Arc::new(brain.then(Wander { chance_pct: 30 })));
        }
        Self { defs, table, brains, faction }
    }

    fn spawn(&self, commands: &mut Commands, id: Id<WatchDef>, at: Point) -> Entity {
        let d = self.defs.get(id);
        commands
            .spawn((
                (Actor, Blocks, Kind(id), Position(at), Speed(d.speed), Faction(self.faction), Health::full(d.hp), Armor(d.armor)),
                (
                    MeleeAttack::new(d.kind.id(), d.attack),
                    Perception(d.perception),
                    DarkSight(d.dark_sight),
                    Notice(d.notice),
                    Hearing(d.hearing),
                    Mind(self.brains[id.index()].clone()),
                    Intelligence(d.wits),
                    Name::new(d.name.clone()),
                    Glyph::new(d.glyph, Color::srgb(d.color.0, d.color.1, d.color.2)).on_layer(5),
                ),
            ))
            .id()
    }
    // ANCHOR_END: watch
}

/// Coin, the score.
#[derive(Component, Clone, Copy)]
struct Coin;

/// A pebble, thrown to make a noise somewhere else.
#[derive(Component, Clone, Copy)]
struct Pebble;

/// A lamp on the wall, and what it sheds when lit.
#[derive(Component, Clone, Copy)]
struct Lamp(LightSource);

/// A lamp that has been put out.
#[derive(Component, Clone, Copy, Default)]
struct Snuffed;

/// The way out, on the top floor.
#[derive(Component, Clone, Copy)]
struct Exit;

/// The player's shaded lantern: what it sheds when open.
const LANTERN: LightSource = LightSource::new(150, 4, Rgb::new(255, 210, 140)).flickering(30);
/// What a wall lamp sheds.
const LAMP: LightSource = LightSource::new(180, 5, Rgb::new(255, 190, 110)).flickering(50);

/// Put out the lamp `0`, which must be beside you. The player's own action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Snuff(Entity);
impl Action for Snuff {}

/// Light the lamp `0` again, which must be beside you. A watchman's choice,
/// routed to this action by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Relight(Entity);
impl Action for Relight {}
impl Choice for Relight {
    fn name(&self) -> &'static str {
        "relight"
    }
}

/// Go out of the window you are standing at, with what you carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Escape;
impl Action for Escape {}

/// The lamps that have been put out, wherever they hang.
type SnuffedLamps<'w, 's> = Query<'w, 's, (Entity, &'static Position, Option<&'static OnMap>), (With<Lamp>, With<Snuffed>)>;

/// The lamps still burning.
type LitLamps<'w, 's> = Query<'w, 's, (Entity, &'static Position), (With<Lamp>, Without<Snuffed>)>;

/// What a watchman knows that the engine does not: the lamps it can see
/// that have gone out.
#[derive(Debug, Clone)]
struct DarkLamps(Vec<(Entity, Point)>);

/// Tells the watcher holding the turn which lamps in its sight are out.
fn notice_dark_lamps(mut thinking: ResMut<Thinking>, sight: Sight, lamps: SnuffedLamps) {
    if thinking.actor().is_none() {
        return;
    }
    let out: Vec<(Entity, Point)> = lamps.iter().filter(|(_, p, on)| sight.perceives(&thinking, p.0, *on)).map(|(e, p, _)| (e, p.0)).collect();
    if let Some(snapshot) = thinking.snapshot_mut() {
        snapshot.add_sense(DarkLamps(out));
    }
}

/// Relight a lamp that is out: at it, if beside one; toward it otherwise.
struct RelightLamps;

impl Tactic<Entity> for RelightLamps {
    fn name(&self) -> &'static str {
        "relight_lamps"
    }

    fn evaluate(&self, ctx: &mut TacticCtx<'_, Entity>) -> Option<Decision<Entity>> {
        let me = ctx.snapshot.me.pos;
        let lamps = ctx.snapshot.sense::<DarkLamps>()?.0.clone();
        if let Some((lamp, _)) = lamps.iter().find(|(_, at)| geometry::is_adjacent(me, *at)) {
            return Some(Decision::own(Relight(*lamp)));
        }
        let cells: Vec<Point> = lamps.iter().map(|(_, at)| *at).collect();
        ctx.step_toward(&cells).map(Decision::Step)
    }
}

/// The lamp `0` as a resolver reaches it, and where it hangs.
type LampAt<'w, 's> = Query<'w, 's, (&'static Position, &'static Lamp, Has<Snuffed>), With<Lamp>>;

/// Puts out a lamp beside the actor and spends the turn.
fn resolve_snuffs(
    mut commands: Commands,
    mut intents: MessageReader<Intent<Snuff>>,
    mut resolution: Resolution,
    holders: Query<&Position, With<MyTurn>>,
    lamps: LampAt,
) {
    for intent in intents.read() {
        if !resolution.claim(intent.actor) {
            continue;
        }
        let beside = holders.get(intent.actor).ok().zip(lamps.get(intent.action.0).ok());
        match beside {
            Some((me, (at, _, false))) if geometry::is_adjacent(me.0, at.0) => {
                commands.entity(intent.action.0).remove::<LightSource>().insert(Snuffed);
                resolution.done(intent.actor, rl_engine::rl_core::turn::BASE_ACTION_COST);
            }
            _ => resolution.failed(intent.actor, rl_engine::rl_core::turn::BASE_ACTION_COST),
        }
    }
}

/// Lights a lamp beside the actor again and spends the turn.
fn resolve_relights(
    mut commands: Commands,
    mut intents: MessageReader<Intent<Relight>>,
    mut resolution: Resolution,
    holders: Query<&Position, With<MyTurn>>,
    lamps: LampAt,
) {
    for intent in intents.read() {
        if !resolution.claim(intent.actor) {
            continue;
        }
        let beside = holders.get(intent.actor).ok().zip(lamps.get(intent.action.0).ok());
        match beside {
            Some((me, (at, lamp, true))) if geometry::is_adjacent(me.0, at.0) => {
                commands.entity(intent.action.0).insert(lamp.0).remove::<Snuffed>();
                resolution.done(intent.actor, rl_engine::rl_core::turn::BASE_ACTION_COST);
            }
            _ => resolution.failed(intent.actor, rl_engine::rl_core::turn::BASE_ACTION_COST),
        }
    }
}

/// How much coin `who` carries.
fn take_of(bag: Option<&Inventory>, coins: &Query<&Stack, With<Coin>>) -> u32 {
    bag.map(|b| b.items.iter().filter_map(|i| coins.get(*i).ok()).map(|s| s.count).sum()).unwrap_or(0)
}

/// Ends the run won, with the take as the score, for an actor standing at
/// the way out; refused, for free, anywhere else.
fn resolve_escapes(
    mut intents: MessageReader<Intent<Escape>>,
    mut resolution: Resolution,
    holders: Query<(&Position, Option<&Inventory>), With<MyTurn>>,
    exits: Query<&Position, With<Exit>>,
    coins: Query<&Stack, With<Coin>>,
    mut over: MessageWriter<RunOver>,
) {
    for intent in intents.read() {
        if !resolution.claim(intent.actor) {
            continue;
        }
        let Ok((me, bag)) = holders.get(intent.actor) else {
            resolution.failed(intent.actor, rl_engine::rl_core::turn::BASE_ACTION_COST);
            continue;
        };
        if !exits.iter().any(|at| at.0 == me.0) {
            resolution.failed(intent.actor, rl_engine::rl_core::turn::BASE_ACTION_COST);
            continue;
        }
        let take = take_of(bag, &coins);
        over.write(RunOver::won().saying(format!("You go out of the window with {take} in coin. The watch never knew.")));
        resolution.done(intent.actor, rl_engine::rl_core::turn::BASE_ACTION_COST);
    }
}

// ANCHOR: take
/// The take, on the screen the run ends on however it ended.
///
/// Pushed every frame in `ViewSet::Annotate` rather than once on
/// `RunOver`, because `EndingView` is a view and is cleared and refilled
/// like every other one. The bag is still there to count after the run
/// ends, so the number does not have to be captured at the moment of
/// death.
fn show_the_take(mut view: ResMut<EndingView>, player: Query<Option<&Inventory>, With<Player>>, coins: Query<&Stack, With<Coin>>) {
    let Ok(bag) = player.single() else { return };
    view.section("The take", format!("{} in coin", take_of(bag, &coins)));
}
// ANCHOR_END: take

/// The player, and only while it holds the turn.
type PlayerHolding = (With<Player>, With<MyTurn>);

/// Hands the engine the house and the thief, then warps the thief in.
fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    mut warps: MessageWriter<WarpRequest>,
    mut log: ResMut<MessageLog>,
    mut next: ResMut<NextState<EngineState>>,
) {
    let house = House::new(seed.0);
    let registries = registries();
    let (thief, watch) = (registries.factions.expect("thief"), registries.factions.expect("watch"));
    commands.insert_resource(CombatRules::new(&registries.factions).hostile(thief, watch));
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    commands.insert_resource(Watch::load(&registries.names(), watch));
    commands.insert_resource(house.appearance());
    commands.insert_resource(WorldMap::new(house.tiles().tables()));
    commands.insert_resource(Tiles { carpet: house.carpet(), window: house.window() });
    commands.insert_resource(PlaceRulesRes(Box::new(house)));
    commands.insert_resource(Lighting::dark());

    // A handful of pebbles: thrown, they make a noise where they land.
    let pebbles = commands
        .spawn((
            Item,
            Pebble,
            Name::new("pebble"),
            Stack { key: 2, count: 6 },
            Throwable { range: 7, strike: None },
            Glyph::new('\u{b7}', Color::srgb(0.7, 0.7, 0.7)).on_layer(2),
        ))
        .id();
    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(12), RevealsMap),
            (
                Health::full(10),
                Armor(0),
                Faction(thief),
                // A fist, for a hound that has you cornered. Against a cudgel it is a mistake.
                MeleeAttack::new(registries.damage_kinds.expect("fist"), DiceRoll::new(1, 2)),
                // Enough to make out the floor at your feet in the dark.
                DarkSight(2),
                // Quiet and subtle: a watchman has to be close, or you have
                // to be standing in light, before it is sure of you.
                Stealth(StealthStats { quiet: 2, subtlety: 15 }),
                // A thief listens: a door or a scuffle out of sight is told
                // in the log, with which way it came from.
                Hearing(HearingStats { threshold: 0, memory: 0 }),
                Name::new("you"),
                Glyph::new('@', Color::WHITE).on_layer(10),
            ),
            Inventory { items: vec![pebbles] },
        ))
        .id();
    commands.insert_resource(registries);
    warps.write(WarpRequest::into_place(player, map_of(1)));
    log.push(format!("Seed {}. You let yourself into the cellars of the counting house.", seed.0.0), Tones::NOTICE, 0);
    log.push("Three floors up there is a window onto the roofs. Whatever coin you carry out is yours.", Tones::NOTICE, 0);
    next.set(EngineState::Playing);
}

/// The two tiles stocking a floor places by hand, kept from the house once
/// it is handed over: the strongroom's carpet and the window out.
#[derive(Resource, Clone, Copy)]
struct Tiles {
    carpet: rl_engine::rl_grid::TileId,
    window: rl_engine::rl_grid::TileId,
}

/// What a floor is stocked from.
#[derive(bevy::ecs::system::SystemParam)]
struct Stock<'w> {
    watch: Res<'w, Watch>,
    map: ResMut<'w, WorldMap>,
    tiles: Res<'w, Tiles>,
    seed: Res<'w, Seed>,
    help: Res<'w, ControlsKeys>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
}

/// Stairs, the window, lamps, coin and the watch, the first time a floor is
/// entered; a line in the log every time.
fn populate_floor(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, mut stock: Stock) {
    for ev in entered.read() {
        let floor = floor_of(ev.map);
        let turn = stock.turns.turn_number();
        stock.log.notice(format!("Floor {floor}: {}.", name_of(floor)), turn);
        if ev.first && floor == 1 {
            stock.log.muted(format!("Press {} for the controls. Stay out of the light.", stock.help.toggle.label()), turn);
        }
        if !ev.first {
            continue;
        }
        // The house is climbed, so the way on is `<` and the way back `>`.
        let stone = Color::srgb(0.86, 0.86, 0.70);
        if floor > 1 {
            commands.spawn((
                Position(ev.entry),
                Transition { to: Destination::Place { map: map_of(floor - 1), arrive: Arrive::Exit } },
                Glyph::new('>', stone).on_layer(1),
            ));
        }
        let Some(exit) = ev.exit else { continue };
        if floor < FLOORS {
            commands.spawn((
                Position(exit),
                Transition { to: Destination::Place { map: map_of(floor + 1), arrive: Arrive::Entry } },
                Glyph::new('<', stone).on_layer(1),
            ));
        } else {
            // The way out: the window is set into the nearest wall, and the
            // cell under it is where you climb out from.
            let window = stock.tiles.window;
            if let Some(wall) = Direction::ALL.into_iter().map(|d| exit + d.offset()).find(|p| !stock.map.is_walkable(*p) && stock.map.tile(*p).is_some()) {
                stock.map.set_tile(wall, window);
            }
            commands.spawn((Exit, Position(exit), Name::new("the window"), Glyph::new('^', Color::srgb(0.6, 0.8, 1.0)).on_layer(1)));
        }
        let Some(place) = stock.map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        let carpet: Vec<Point> = place.terrain.iter().filter(|(_, t)| *t == stock.tiles.carpet).map(|(p, _)| p).collect();
        let mut rng = stock.seed.stream(b"house.floor", floor as u64);
        let map = &*stock.map;

        // Lamps, well apart, none right by the way in.
        let mut lamps: Vec<Point> = Vec::new();
        for _ in 0..300 {
            if lamps.len() >= 5 + floor as usize {
                break;
            }
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            if map.is_walkable(p) && geometry::chebyshev(p, ev.entry) > 3 && lamps.iter().all(|l| geometry::chebyshev(*l, p) >= 6) {
                lamps.push(p);
                commands.spawn((Lamp(LAMP), LAMP, Position(p), Name::new("a lamp"), Glyph::new('*', Color::srgb(1.0, 0.8, 0.4)).on_layer(1)));
            }
        }
        // Coin: a few piles a floor, more and heavier the higher you climb,
        // and in the strongroom on the carpet, where the watch stand thickest.
        let piles = 5 + floor;
        for _ in 0..piles {
            let p = if floor == FLOORS && !carpet.is_empty() {
                carpet[rng.random_range(0..carpet.len())]
            } else {
                match spot_between(bounds, map, ev.entry, 4, 60, &mut rng) {
                    Some(p) => p,
                    None => continue,
                }
            };
            let count = rng.random_range(3 * floor..=10 * floor);
            spawn_coin(&mut commands, p, count);
        }
        if floor == FLOORS
            && let Some(p) = spot_between(bounds, map, ev.entry, 10, 60, &mut rng)
        {
            spawn_coin(&mut commands, p, 60);
        }
        // The watch, by the table, never near the way in.
        let mut placed = 0;
        for _ in 0..60 {
            if placed > floor {
                break;
            }
            let Some((id, count)) = stock.watch.table.pick_group(floor as i32, &mut rng) else { break };
            let id = *id;
            let anchor = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            let mut here = 0;
            for p in geometry::square(anchor, 1) {
                if here >= count {
                    break;
                }
                if map.is_walkable(p) && geometry::chebyshev(p, ev.entry) >= 9 {
                    stock.watch.spawn(&mut commands, id, p);
                    here += 1;
                }
            }
            if here > 0 {
                placed += 1;
            }
        }
    }
}

/// A pile of `count` coins at `at`.
fn spawn_coin(commands: &mut Commands, at: Point, count: u32) {
    commands.spawn((Item, Coin, Name::new("coin"), Stack { key: 1, count }, Position(at), Glyph::new('$', Color::srgb(1.0, 0.85, 0.2)).on_layer(2)));
}

/// A walkable tile between `min` and `max` steps from `from`, or `None`
/// after a fair number of tries.
fn spot_between(bounds: Rect, map: &WorldMap, from: Point, min: i32, max: i32, rng: &mut impl Rng) -> Option<Point> {
    (0..400)
        .map(|_| Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom())))
        .find(|p| map.is_walkable(*p) && (min..=max).contains(&geometry::chebyshev(*p, from)))
}

/// Where everyone is, what turn it is, and where a line goes: what the
/// systems that tell the player something all need.
#[derive(bevy::ecs::system::SystemParam)]
struct Say<'w> {
    map: Res<'w, WorldMap>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
}

// ANCHOR: alarm
/// A watchman who spots you shouts, and a hound bays: a noise of the
/// game's own at the watcher, which everyone in earshot comes to.
fn raise_alarm(
    mut noticed: MessageReader<Noticed>,
    mut noise: MessageWriter<MakeNoise>,
    player: Query<Entity, With<Player>>,
    watchers: Query<&Position>,
    sounds: Res<Sounds>,
) {
    let Ok(me) = player.single() else { return };
    let shout = sounds.get("shout").expect("declared in main");
    for ev in noticed.read() {
        if ev.subject != me {
            continue;
        }
        if let Ok(at) = watchers.get(ev.observer) {
            noise.write(MakeNoise { at: at.0, loudness: SHOUT, sound: shout, maker: Some(ev.observer) });
        }
    }
}
// ANCHOR_END: alarm

/// What the house heard, told to the thief: a pebble's clatter, and
/// whether anything turned to look; and a shout, when anyone was near
/// enough to come.
fn narrate_the_house(
    mut thrown: MessageReader<ItemEvent>,
    mut heard: MessageReader<NoiseHeard>,
    pebbles: Query<(), With<Pebble>>,
    kinds: Query<&Kind>,
    watch: Res<Watch>,
    sounds: Res<Sounds>,
    mut say: Say,
) {
    let heard: Vec<NoiseHeard> = heard.read().copied().collect();
    let turn = say.turns.turn_number();
    for ev in thrown.read() {
        let ItemEvent::Thrown { item, at, .. } = *ev else { continue };
        if !pebbles.contains(item) {
            continue;
        }
        let looked = heard.iter().any(|h| h.sound == Sounds::LANDING && h.at == at.0 && kinds.contains(h.listener));
        let line = if looked { "The pebble clatters across the floor. Something turns to look." } else { "The pebble clatters across the floor." };
        say.log.muted(line, turn);
    }
    let shout = sounds.get("shout");
    let mut criers: Vec<Entity> = Vec::new();
    for h in heard.iter().filter(|h| Some(h.sound) == shout && kinds.contains(h.listener)) {
        if let Some(crier) = h.maker.filter(|c| !criers.contains(c)) {
            criers.push(crier);
        }
    }
    for crier in criers {
        let hound = kinds.get(crier).is_ok_and(|k| watch.defs.get(k.0).name == "hound");
        let cry = if hound { "A hound bays. Boots on the boards." } else { "A shout goes up. Boots on the boards." };
        say.log.bad(cry, turn);
    }
}

/// A door or a scuffle the thief heard and could not see, and which way it
/// came from. Footsteps are left out: the watch walk all night, and a log
/// of every step is a log nobody reads.
fn narrate_what_the_thief_hears(mut heard: MessageReader<NoiseHeard>, thief: Query<(Entity, &Position, &Viewshed), With<Player>>, mut say: Say) {
    let turn = say.turns.turn_number();
    let Ok((me, pos, sight)) = thief.single() else { return };
    for h in heard.read().filter(|h| h.listener == me && !sight.can_see(h.at)) {
        let what = if h.sound == Sounds::DOOR {
            "A door"
        } else if h.sound == Sounds::STRIKE {
            "Blows"
        } else {
            continue;
        };
        say.log.muted(format!("{what}, somewhere to the {}.", compass(pos.0, h.at)), turn);
    }
}

/// Which way `to` lies from `from`, as a thief would say it.
fn compass(from: Point, to: Point) -> &'static str {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    // Within a step of straight along an axis for every two along it reads
    // as that axis; otherwise it is a diagonal.
    let (ew, ns) = (dx.signum(), dy.signum());
    let (ew, ns) = if dx.abs() > 2 * dy.abs() {
        (ew, 0)
    } else if dy.abs() > 2 * dx.abs() {
        (0, ns)
    } else {
        (ew, ns)
    };
    match (ew, ns) {
        (0, -1) => "north",
        (1, -1) => "northeast",
        (1, 0) => "east",
        (1, 1) => "southeast",
        (0, 1) => "south",
        (-1, 1) => "southwest",
        (-1, 0) => "west",
        (-1, -1) => "northwest",
        _ => "near",
    }
}

/// Every key the heist answers to, by name.
#[derive(Resource, Clone, Copy)]
struct Binds {
    walk: ControlId,
    stairs: ControlId,
    wait: ControlId,
    pick_up: ControlId,
    throw: ControlId,
    snuff: ControlId,
    close: ControlId,
    lantern: ControlId,
    overlay: ControlId,
    quit: ControlId,
}

/// Declares the keys, under the headings the `?` screen groups them by.
fn declare_controls(app: &mut App) {
    let binds = Binds {
        walk: app.add_control("Move", "walk, or strike whoever is there", Keys::Directions { shift: false }),
        stairs: app.add_control(
            "Move",
            "take the stairs, or the window",
            [Chord::key(KeyCode::Enter), Chord::shift(KeyCode::Period), Chord::shift(KeyCode::Comma)],
        ),
        wait: app.add_control("Act", "wait a turn", [KeyCode::Period, KeyCode::Numpad5]),
        pick_up: app.add_control("Act", "pick up what is here", KeyCode::KeyG),
        throw: app.add_control("Act", "throw a pebble", KeyCode::KeyT),
        snuff: app.add_control("Act", "snuff the lamp beside you", KeyCode::KeyS),
        close: app.add_control("Act", "shut the door beside you", KeyCode::KeyC),
        lantern: app.add_control("Act", "open or shade your lantern", Chord::shift(KeyCode::KeyL)),
        overlay: app.add_control("Game", "show the light on each tile", KeyCode::KeyV),
        quit: app.add_control("Game", "quit", KeyCode::KeyQ),
    };
    app.insert_resource(binds);
}

/// The player, while it holds the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position), PlayerHolding>;

/// What the player's keys can ask for.
#[derive(bevy::ecs::system::SystemParam)]
struct PlayerIntents<'w> {
    bumps: MessageWriter<'w, Intent<Bump>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    stairs: MessageWriter<'w, Intent<GoThrough>>,
    escapes: MessageWriter<'w, Intent<Escape>>,
    closes: MessageWriter<'w, Intent<Close>>,
}

/// Keys to intents: walk, wait, the stairs or the window, shut a door, quit.
fn player_input(
    keys: ControlInput,
    binds: Res<Binds>,
    map: Res<WorldMap>,
    player: PlayerTurn,
    exits: Query<&Position, With<Exit>>,
    mut intents: PlayerIntents,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(binds.quit) {
        exit.write(AppExit::Success);
        return;
    }
    let Ok((entity, pos)) = player.single() else { return };
    if let Some(dir) = keys.direction(binds.walk) {
        intents.bumps.write(Intent::new(entity, Bump(dir)));
    } else if keys.just_pressed(binds.stairs) {
        if exits.iter().any(|at| at.0 == pos.0) {
            intents.escapes.write(Intent::new(entity, Escape));
        } else {
            intents.stairs.write(Intent::new(entity, GoThrough));
        }
    } else if keys.just_pressed(binds.wait) {
        intents.waits.write(Intent::new(entity, Wait));
    } else if keys.just_pressed(binds.close)
        && let Some(dir) = Direction::ALL.into_iter().find(|d| map.closes(pos.0 + d.offset()).is_some())
    {
        intents.closes.write(Intent::new(entity, Close(dir)));
    }
}

/// `L` opens the shaded lantern or shades it, and spends the turn.
fn tend_lantern(
    keys: ControlInput,
    binds: Res<Binds>,
    mut commands: Commands,
    player: Query<(Entity, Has<LightSource>), PlayerHolding>,
    mut waits: MessageWriter<Intent<Wait>>,
    mut log: ResMut<MessageLog>,
    turns: Res<Turns>,
) {
    if !keys.just_pressed(binds.lantern) {
        return;
    }
    let Ok((entity, lit)) = player.single() else { return };
    if lit {
        commands.entity(entity).remove::<LightSource>();
        log.muted("You shade the lantern. The dark comes back, and hides you in it.", turns.turn_number());
    } else {
        commands.entity(entity).insert(LANTERN);
        log.notice("You open the lantern. The room comes up around you, and so do you.", turns.turn_number());
    }
    waits.write(Intent::new(entity, Wait));
}

/// `s` snuffs the lit lamp beside you, if there is one.
fn snuff(
    keys: ControlInput,
    binds: Res<Binds>,
    player: PlayerTurn,
    lamps: LitLamps,
    mut snuffs: MessageWriter<Intent<Snuff>>,
    mut log: ResMut<MessageLog>,
    turns: Res<Turns>,
) {
    if !keys.just_pressed(binds.snuff) {
        return;
    }
    let Ok((entity, pos)) = player.single() else { return };
    match lamps.iter().find(|(_, at)| geometry::is_adjacent(pos.0, at.0)) {
        Some((lamp, _)) => {
            snuffs.write(Intent::new(entity, Snuff(lamp)));
        }
        None => log.muted("No lit lamp within reach.", turns.turn_number()),
    }
}

/// `g` picks up what lies here, or says there is nothing.
fn pick_up(
    keys: ControlInput,
    binds: Res<Binds>,
    player: PlayerTurn,
    ground: Query<(&Position, Option<&OnMap>), With<Item>>,
    mut picks: MessageWriter<Intent<PickUp>>,
    mut say: Say,
) {
    if !keys.just_pressed(binds.pick_up) {
        return;
    }
    let Ok((entity, pos)) = player.single() else { return };
    let here = say.map.current();
    if ground.iter().any(|(p, on)| p.0 == pos.0 && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here) {
        picks.write(Intent::new(entity, PickUp));
    } else {
        say.log.muted("Nothing here worth the bending.", say.turns.turn_number());
    }
}

/// `t` throws a pebble through the targeting cursor, which opens on the
/// nearest watcher; the aim is yours to move, since the point is to make a
/// noise somewhere you are not.
fn throw(
    keys: ControlInput,
    binds: Res<Binds>,
    player: Query<(Entity, &Inventory), PlayerHolding>,
    pebbles: Query<(), With<Pebble>>,
    mut aims: MessageWriter<AimThrow>,
    mut log: ResMut<MessageLog>,
    turns: Res<Turns>,
) {
    if !keys.just_pressed(binds.throw) {
        return;
    }
    let Ok((user, bag)) = player.single() else { return };
    match bag.items.iter().copied().find(|i| pebbles.contains(*i)) {
        Some(item) => {
            aims.write(AimThrow { user, item });
        }
        None => log.muted("Your pockets are out of pebbles.", turns.turn_number()),
    }
}

/// `v` draws the light on each tile as a digit.
fn toggle_overlay(keys: ControlInput, binds: Res<Binds>, mut overlay: ResMut<LightOverlay>) {
    if keys.just_pressed(binds.overlay) {
        overlay.0 = !overlay.0;
    }
}

/// The take so far, the pebbles left, and which floor this is.
fn note_take(
    mut vitals: ResMut<VitalsView>,
    mut facets: ResMut<Facets>,
    map: Res<WorldMap>,
    player: Query<&Inventory, With<Player>>,
    coins: Query<&Stack, With<Coin>>,
    pebbles: Query<&Stack, With<Pebble>>,
) {
    let floor = floor_of(map.current());
    vitals.facets.push(facets.facet("floor", format!("floor {floor} of {FLOORS}: {}", name_of(floor))));
    let Ok(bag) = player.single() else { return };
    vitals.facets.push(facets.facet("take", format!("take {}", take_of(Some(bag), &coins))).toned(Tones::GOOD));
    let left: u32 = bag.items.iter().filter_map(|i| pebbles.get(*i).ok()).map(|s| s.count).sum();
    vitals.facets.push(facets.facet("pebbles", format!("pebbles {left}")));
}

/// How lit the cell you stand on is, and whether your lantern is open:
/// the two numbers a thief lives by.
fn note_light(mut vitals: ResMut<VitalsView>, mut facets: ResMut<Facets>, lighting: Res<Lighting>, player: Query<(&Position, Has<LightSource>), With<Player>>) {
    let Ok((pos, lantern)) = player.single() else { return };
    let here = lighting.at(pos.0).intensity;
    let tone = if lighting.is_lit(pos.0) { Tones::BAD } else { Tones::GOOD };
    vitals.facets.push(facets.facet("light", format!("light here {here}{}", if lantern { ", lantern open" } else { "" })).toned(tone));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A headless house: the engine plugins and the heist's own systems,
    /// with the keys wired as `main` wires them.
    fn headless(seed: u64) -> App {
        let mut app = rl_engine::rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, CombatPlugin, MindsPlugin, ItemsPlugin, ThrowingPlugin, LightingPlugin, StealthPlugin, NoisePlugin::new(NOISE)));
        app.add_sound("shout")
            .insert_resource(Seed(RunSeed(seed)))
            .add_plugins((UiPlugin, rl_engine::rl_bevy::testing::KeyScriptPlugin))
            .insert_resource(rl_engine::rl_render::Terminal::new(COLS, ROWS, Vec2::ONE))
            .init_resource::<LightOverlay>()
            .add_plugins(TargetPanel::new(Screen::new().target))
            .add_systems(NewRun, start)
            .add_systems(Update, (tend_lantern, snuff, pick_up, throw, toggle_overlay, player_input).chain().run_if(no_modal).in_set(EngineSet::Input))
            .add_action::<Snuff>()
            .add_action::<Escape>()
            .add_choice::<Relight>()
            .add_systems(Turn, (resolve_snuffs, resolve_relights, resolve_escapes).in_set(ResolveSet::Act))
            .add_systems(Turn, notice_dark_lamps.in_set(PerceiveSet::Annotate))
            .add_systems(Turn, (populate_floor, raise_alarm).in_set(TurnSet::React))
            .add_systems(Update, (narrate_the_house, narrate_what_the_thief_hears).after(ViewSet::Speak).in_set(PresentSet::Narrate));
        declare_controls(&mut app);
        app
    }

    /// A run settled into the cellars with the player holding its turn,
    /// the watch the floor spawned taken away so a test can place its own.
    fn settled(seed: u64) -> (App, Entity, Point) {
        let mut app = headless(seed);
        for _ in 0..3 {
            app.update();
        }
        let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
        let spawned: Vec<Entity> = app.world_mut().query_filtered::<Entity, With<Kind>>().iter(app.world()).collect();
        for e in spawned {
            app.world_mut().despawn(e);
        }
        app.update();
        let at = app.world().get::<Position>(player).unwrap().0;
        (app, player, at)
    }

    /// A watchman of `notice` at `at`, from the file's definition otherwise.
    fn watchman(app: &mut App, at: Point, notice: NoticeStats) -> Entity {
        let id = app.world().resource::<Watch>().defs.expect("watchman");
        let e = app.world_mut().resource_scope(|world: &mut World, watch: Mut<Watch>| {
            let mut queue = bevy::ecs::world::CommandQueue::default();
            let mut commands = Commands::new(&mut queue, world);
            let e = watch.spawn(&mut commands, id, at);
            queue.apply(world);
            e
        });
        app.world_mut().entity_mut(e).insert(Notice(notice));
        app.update();
        e
    }

    /// A cell `steps` away from `from` along the first direction that has
    /// open floor all the way, so a test can stand two things in line.
    fn open_line(app: &App, from: Point, steps: i32) -> (Direction, Point) {
        let map = app.world().resource::<WorldMap>();
        Direction::ALL
            .into_iter()
            .filter(|d| !d.is_diagonal())
            .find(|d| (1..=steps).all(|i| map.is_walkable(from + Point::new(d.offset().x * i, d.offset().y * i))))
            .map(|d| (d, from + Point::new(d.offset().x * steps, d.offset().y * steps)))
            .expect("the cellars have a straight run of floor")
    }

    fn wait(app: &mut App, player: Entity, turns: usize) {
        for _ in 0..turns {
            app.world_mut().write_message(Intent::new(player, Wait));
            app.update();
        }
    }

    /// What walking from `from` to `to` costs, in hundredths of a step,
    /// round the walls and never through a shut door.
    fn walk(app: &App, from: Point, to: Point) -> i32 {
        let map = app.world().resource::<WorldMap>();
        let view = map.view();
        let mut flood = DijkstraMap::covering(&view);
        flood.build(&view, map.to_local(to), PathRules::EIGHT_WAY);
        map.to_local(from).and_then(|l| flood.value(l)).unwrap_or(i32::MAX)
    }

    /// Where `watcher` last heard something, if it still remembers.
    fn heard(app: &App, watcher: Entity) -> Option<Point> {
        app.world().get::<Heard>(watcher).and_then(|h| h.last_known())
    }

    fn aware_of(app: &App, watcher: Entity, subject: Entity) -> bool {
        app.world().get::<Aware>(watcher).is_some_and(|a| a.knows(subject))
    }

    /// The point of the light: a watchman who never rolls to notice sees a
    /// quiet thief only within its certain radius, which a lamp on the
    /// thief widens past the gap between them.
    #[test]
    fn a_thief_in_the_dark_is_missed_and_a_thief_under_a_lamp_is_seen() {
        let (mut app, player, at) = settled(7);
        let (_, far) = open_line(&app, at, 4);
        let notice = NoticeStats { certain: 2, chance_pct: 0, lit_bonus: 4, memory: 6 };
        let watcher = watchman(&mut app, far, notice);
        wait(&mut app, player, 3);
        assert!(!aware_of(&app, watcher, player), "four cells off in the dark, never noticed");

        app.world_mut().spawn((Lamp(LAMP), LAMP, Position(at.offset(0, 1))));
        wait(&mut app, player, 2);
        assert!(app.world().resource::<Lighting>().is_lit(at), "the lamp lights the thief");
        assert!(aware_of(&app, watcher, player), "and lit, the thief is seen from four cells");
    }

    /// A pebble thrown across the cellar is heard where it lands: the
    /// watchman who heard it walks to the sound and, finding nothing there,
    /// forgets it, and never learns the thief was the one who threw it.
    #[test]
    fn a_thrown_pebble_draws_a_watchman_to_where_it_clattered() {
        let (mut app, player, at) = settled(7);
        let (dir, _) = open_line(&app, at, 3);
        let landing = at + Point::new(dir.offset().x * 3, dir.offset().y * 3);
        // Out of the thief's certain radius, and in earshot of the landing
        // by a way round the walls: sound goes by the rooms, not through
        // them. Walking there costs at least what the sound spends, so six
        // steps' walk is inside the watchman's eight.
        let post = {
            let bounds = app.world().resource::<WorldMap>().window_tiles();
            (bounds.y..bounds.bottom())
                .flat_map(|y| (bounds.x..bounds.right()).map(move |x| Point::new(x, y)))
                .find(|p| {
                    app.world().resource::<WorldMap>().is_walkable(*p) && (400..=600).contains(&walk(&app, *p, landing)) && geometry::chebyshev(*p, at) >= 3
                })
                .expect("floor in earshot of the landing")
        };
        let deaf_to_sight = NoticeStats { certain: 1, chance_pct: 0, lit_bonus: 0, memory: 6 };
        let watcher = watchman(&mut app, post, deaf_to_sight);
        let pebbles = app.world().get::<Inventory>(player).unwrap().items[0];
        let before = walk(&app, post, landing);
        app.world_mut().write_message(Intent::new(player, Throw { item: pebbles, at: landing }));
        app.update();
        assert_eq!(heard(&app, watcher), Some(landing), "it heard the clatter, and where");
        wait(&mut app, player, 4);
        let after = walk(&app, app.world().get::<Position>(watcher).unwrap().0, landing);
        assert!(after < before, "it went to look: {before} hundredths of a step from the sound, then {after}");
        // Arrived where it could see the spot, it found a pebble and
        // nobody, and let the sound go without learning who threw it.
        assert_eq!(heard(&app, watcher), None, "and the sound was forgotten once the spot was seen");
        assert!(!aware_of(&app, watcher, player), "the thief was never in it");
    }

    /// A door opened out of the thief's sight is told in the log, with the
    /// way it lies.
    #[test]
    fn a_door_the_thief_hears_and_cannot_see_is_told_with_its_direction() {
        let (mut app, player, at) = settled(7);
        let door = {
            let map = app.world().resource::<WorldMap>();
            let sight = app.world().get::<Viewshed>(player).unwrap();
            let bounds = map.window_tiles();
            (bounds.y..bounds.bottom())
                .flat_map(|y| (bounds.x..bounds.right()).map(move |x| Point::new(x, y)))
                .filter(|p| !sight.can_see(*p) && map.is_walkable(*p) && geometry::chebyshev(*p, at) <= 6)
                .find(|p| walk(&app, *p, at) <= 400)
                .expect("floor out of sight and within earshot")
        };
        let hand = app.world_mut().spawn(Position(door)).id();
        app.world_mut().write_message(DoorEvent::Opened { actor: hand, at: door });
        wait(&mut app, player, 1);
        let lines: Vec<String> = app.world().resource::<MessageLog>().iter().map(|e| e.text.clone()).collect();
        let told = format!("A door, somewhere to the {}.", compass(at, door));
        assert!(lines.contains(&told), "{told:?} in {lines:?}");
    }

    #[test]
    fn a_compass_reads_an_axis_when_nearly_along_it_and_a_diagonal_otherwise() {
        let o = Point::new(10, 10);
        assert_eq!(compass(o, Point::new(10, 2)), "north");
        assert_eq!(compass(o, Point::new(15, 8)), "east", "five across and two up is east");
        assert_eq!(compass(o, Point::new(14, 13)), "southeast");
        assert_eq!(compass(o, Point::new(3, 10)), "west");
        assert_eq!(compass(o, o), "near");
    }

    /// Snuffing a lamp puts its light out, and a watchman with hands who
    /// sees a dark lamp comes and lights it again: the game's own tactic,
    /// over the game's own sense, choosing the game's own action.
    #[test]
    fn a_snuffed_lamp_goes_dark_and_a_watchman_comes_to_relight_it() {
        let (mut app, player, at) = settled(7);
        let (dir, far) = open_line(&app, at, 5);
        let lamp_at = at + dir.offset();
        let lamp = app.world_mut().spawn((Lamp(LAMP), LAMP, Position(lamp_at))).id();
        let blind = NoticeStats { certain: 1, chance_pct: 0, lit_bonus: 0, memory: 6 };
        let watcher = watchman(&mut app, far, blind);
        assert!(app.world().resource::<Lighting>().is_lit(lamp_at), "lit to begin with");

        app.world_mut().write_message(Intent::new(player, Snuff(lamp)));
        app.update();
        assert!(app.world().get::<Snuffed>(lamp).is_some() && app.world().get::<LightSource>(lamp).is_none(), "snuffed");
        assert!(!app.world().resource::<Lighting>().is_lit(lamp_at), "and dark");

        wait(&mut app, player, 12);
        assert!(app.world().get::<Snuffed>(lamp).is_none(), "the watchman relit it");
        assert!(app.world().resource::<Lighting>().is_lit(lamp_at));
        let _ = watcher;
    }

    /// Standing at the window on the top floor and climbing out ends the
    /// run won, with the coin carried as the score, in the words and the
    /// morgue file both; anywhere else the same key is refused for free.
    #[test]
    fn climbing_out_of_the_window_wins_with_the_take_as_the_score() {
        let (mut app, player, _) = settled(7);
        let clock = app.world().resource::<Turns>().now();
        app.world_mut().write_message(Intent::new(player, Escape));
        app.update();
        assert_eq!(app.world().resource::<Turns>().now(), clock, "no window here: refused, for nothing");

        // Straight to the strongroom's far end, with a purse.
        let coins = app.world_mut().spawn((Item, Coin, Stack { key: 1, count: 47 })).id();
        app.world_mut().get_mut::<Inventory>(player).unwrap().items.push(coins);
        app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map: map_of(FLOORS), arrive: Arrive::Exit } });
        app.update();
        app.update();
        let exits: Vec<Point> = app.world_mut().query_filtered::<&Position, With<Exit>>().iter(app.world()).map(|p| p.0).collect();
        assert_eq!(exits.len(), 1, "one window on the top floor");
        assert_eq!(app.world().get::<Position>(player).unwrap().0, exits[0], "the stairs land at it");

        app.world_mut().write_message(Intent::new(player, Escape));
        app.update();
        let ending = app.world().resource::<Ending>();
        assert_eq!(ending.outcome, Outcome::Won);
        assert!(ending.epitaph.contains("47 in coin"), "{}", ending.epitaph);
    }
}
