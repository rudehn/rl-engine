//! Knacks: one engine, five genres, eighteen abilities and no branching.
//!
//! The engine's fourth example, and the one that answers "will abilities
//! work for my game". One arena, one set of monsters, and five ability
//! files in `assets/`: a fantasy caster, a pirate, a marine, a
//! man-at-arms and a thief. Press `Tab` to change which set the player
//! knows. Nothing else about the run changes, because nothing else can:
//! the sets differ only in RON.
//!
//! All five load into one registry at startup, which is the sharpest form
//! of the claim. A fireball and a broadside and a smoke bomb are the same
//! kind of thing sitting in the same table, and the resolver that fires
//! them cannot tell which genre it is in.
//!
//! What is not data is the vocabulary: `src/effects.rs` holds the five
//! effects the engine does not ship, one per genre. Read it next to this
//! file and the boundary is the whole design in two screens.
//!
//! `cargo run -p knacks -- --seed 7 --set pirates`
//!
//! Keys: move with arrows, vi keys or the numpad; `1` to `4` open the
//! targeting cursor on the abilities in the order the set lists them;
//! `a` lists them all with the reasons the ones greyed out cannot be
//! used; `Tab` changes set; `.` waits; `q` quits.
//!
//! While the cursor is up: the direction keys step it, `Tab` cycles the
//! things the ability wants, `Enter` or space spends the turn on it, and
//! `Esc` puts it away. An ability that wants no cursor is used at once.
//! None of that is in this file: a key press writes [`AimAt`] and the
//! engine does the rest.

mod effects;

use bevy::prelude::*;
use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_mapgen::passes::{Border, CentralStart, Fill, Scatter};
use rl_engine::rl_rules::ability::{AbilityDef, Cost, Lookup};
use rl_engine::rl_rules::ai::tactics::{Hunt, MeleeAdjacent, UseAbility, Wander};
use rl_engine::rl_rules::damage::DamageKindId;
use rl_engine::rl_rules::damage::SubtractArmor;
use rl_engine::rl_rules::faction::FactionDef;
use rl_engine::rl_rules::{SlotId, StatId, StatusId, TagId};
use std::sync::Arc;

use effects::{Banner, Drain, Hack, Plunder, Smoke, SmokeTile};

const COLS: i32 = 80;
const ROWS: i32 = 36;
const LOG_ROWS: i32 = 5;
const ARENA: MapId = MapId(1);

/// The five sets, in the order `Tab` walks them.
const SETS: [(&str, &str); 5] = [
    ("fantasy", include_str!("../assets/fantasy.ron")),
    ("pirates", include_str!("../assets/pirates.ron")),
    ("scifi", include_str!("../assets/scifi.ron")),
    ("medieval", include_str!("../assets/medieval.ron")),
    ("crime", include_str!("../assets/crime.ron")),
];

fn main() -> AppExit {
    let mut seed = RunSeed::fresh();
    let mut set = 0usize;
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--seed") {
        seed = RunSeed(args[i + 1].parse().expect("seed"));
    }
    if let Some(i) = args.iter().position(|a| a == "--set") {
        set = SETS.iter().position(|(name, _)| *name == args[i + 1]).unwrap_or_else(|| panic!("--set is one of {:?}", SETS.map(|(n, _)| n)));
    }

    let mut app = App::new();
    app.add_plugins(RoguelikePlugins::new("Knacks", COLS, ROWS).map(Rect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS)))
        .add_plugins((CombatPlugin, ItemsPlugin, StatusPlugin, AbilitiesPlugin))
        // The engine's effects, then this game's. Nothing is registered by
        // default, so an ability naming an effect nobody added fails at load.
        .add_engine_effects()
        .add_effect::<Drain>()
        .add_effect::<Plunder>()
        .add_effect::<Hack>()
        .add_effect::<Banner>()
        .add_effect::<Smoke>()
        .insert_resource(Seed(seed))
        .insert_resource(Chosen(set))
        .add_plugins(VitalsPanel::new(Rect::new(0, 0, COLS, 1)))
        .add_plugins(LogPanel::new(Rect::new(0, ROWS - LOG_ROWS, COLS, LOG_ROWS)))
        // The cursor and the list: the whole of what this game writes for
        // aiming is the key that opens one and the key that opens the other.
        .add_plugins(TargetPanel::new(Rect::new(0, ROWS - LOG_ROWS - 1, COLS, 1)).hints("[enter] fire  [tab] next  [esc] back"))
        .add_plugins(AbilityPanel::new(Rect::new(COLS / 2 - 18, 6, 36, 14)).title("What you can call on").hints("[a] close"))
        .add_systems(Startup, start)
        .add_systems(Update, (player_input, switch_set).in_set(EngineSet::Input))
        .add_systems(Turn, populate.in_set(TurnSet::React))
        .add_systems(Update, show_pools.in_set(ViewSet::Annotate))
        .add_systems(Update, narrate.in_set(PresentSet::Narrate));
    app.run()
}

#[derive(Resource, Clone, Copy)]
struct Seed(RunSeed);

/// Which of [`SETS`] the player currently knows.
#[derive(Resource, Clone, Copy)]
struct Chosen(usize);

/// Coin, which only `Plunder` cares about.
#[derive(Component, Debug, Clone, Copy)]
pub struct Coin(pub u32);

/// Something standing on the map that is not an actor.
#[derive(Component, Debug, Clone, Copy)]
pub struct Prop;

/// Every registry an ability file names, and the lookup over them.
///
/// One implementation, five files. This is the whole cost of authoring
/// abilities by name rather than mirroring the schema in a type per game.
#[derive(Resource)]
struct Content {
    stats: Registry<StatDef>,
    statuses: Registry<StatusDef>,
    tags: Registry<TagDef>,
    slots: Registry<SlotDef>,
    kinds: Registry<DamageKind>,
}

impl Lookup for Content {
    fn stat(&self, n: &str) -> Option<StatId> {
        self.stats.id(n)
    }
    fn status(&self, n: &str) -> Option<StatusId> {
        self.statuses.id(n)
    }
    fn tag(&self, n: &str) -> Option<TagId> {
        self.tags.id(n)
    }
    fn slot(&self, n: &str) -> Option<SlotId> {
        self.slots.id(n)
    }
    fn damage(&self, n: &str) -> Option<DamageKindId> {
        self.kinds.id(n)
    }
}

impl Content {
    fn new() -> Self {
        Self {
            // One pool per genre, and the engine cannot tell them apart.
            stats: Registry::from_defs(vec![StatDef::new("mana", 40), StatDef::new("power", 40), StatDef::new("stamina", 30), StatDef::new("nerve", 20)])
                .unwrap(),
            statuses: Registry::from_defs(vec![
                StatusDef::new("scorched").ticks(0, 1),
                StatusDef::new("deafened"),
                StatusDef::new("stunned"),
                StatusDef::new("cloaked"),
                StatusDef::new("dazed"),
                StatusDef::new("inspired"),
                StatusDef::new("blinded"),
                StatusDef::new("bleeding"),
                StatusDef::new("spotted"),
            ])
            .unwrap(),
            tags: Registry::from_defs(vec![TagDef::new("powder"), TagDef::new("rum"), TagDef::new("cash"), TagDef::new("shield")]).unwrap(),
            slots: Registry::from_defs(vec![SlotDef::new("main hand"), SlotDef::new("off hand")]).unwrap(),
            kinds: Registry::from_defs(vec![
                DamageKind::new("fire").unarmored(),
                DamageKind::new("shot"),
                DamageKind::new("shock").unarmored(),
                DamageKind::new("blunt"),
                DamageKind::new("pierce"),
                DamageKind::new("care").unarmored(),
            ])
            .unwrap(),
        }
    }

    /// Every set in one registry, so a fireball and a smoke bomb are two
    /// rows of the same table.
    fn abilities(&self) -> Registry<AbilityDef> {
        let mut all: Vec<AbilityDef> = Vec::new();
        for (name, text) in SETS {
            let loaded = rl_engine::rl_rules::ability::load(text, self).unwrap_or_else(|e| panic!("assets/{name}.ron: {e}"));
            all.extend(loaded.iter().map(|(_, d)| d.clone()));
        }
        Registry::from_defs(all).expect("no two sets name the same ability")
    }
}

/// The arena: a walled room with pillars, and a smoke tile that blocks
/// sight so a thief's bomb does something a wall does.
struct Arena {
    tiles: TileRegistry,
    seed: RunSeed,
}

impl Arena {
    fn new(seed: RunSeed) -> Self {
        let mut tiles = TileRegistry::new();
        tiles.register(TileProps::wall("wall")).unwrap();
        tiles.register(TileProps::floor("floor")).unwrap();
        tiles.register(TileProps::floor("smoke").opaque(true)).unwrap();
        Self { tiles, seed }
    }

    fn appearance(&self) -> TileAppearance {
        let mut look = TileAppearance::new();
        let t = |name| self.tiles.expect(name);
        look.set_varied(t("wall"), Cell::new('#', Color::srgb(0.78, 0.74, 0.68)).on(Color::srgb(0.3, 0.28, 0.26)), Vary::new(0.15, 0.04));
        look.set_varied(t("floor"), Cell::new('.', Color::srgb(0.72, 0.69, 0.63)).on(Color::srgb(0.16, 0.16, 0.17)), Vary::new(0.2, 0.05));
        look.set_varied(t("smoke"), Cell::new('§', Color::srgb(0.7, 0.7, 0.72)).on(Color::srgb(0.3, 0.3, 0.32)), Vary::new(0.3, 0.08));
        look
    }
}

impl PlaceRules for Arena {
    fn build(&self, _: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let t = |name| self.tiles.expect(name);
        let (wall, floor) = (t("wall"), t("floor"));
        let mut ctx = BaseContext::blank(64, 26, self.tiles.clone(), wall);
        Chain::new()
            .then(Fill { tile: floor })
            .then(Border { tile: wall })
            .then(Scatter { name: "pillars", tile: wall, on: floor, chance_pct: 5 })
            .then(CentralStart)
            .run(&mut ctx, self.seed)?;
        PlaceBuild::from_context(ctx)
    }
}

/// The brains and sides the arena's creatures share.
#[derive(Resource)]
struct Creatures {
    brute: Arc<Brain<Entity>>,
    adept: Arc<Brain<Entity>>,
    fist: DamageKindId,
    them: rl_engine::rl_rules::FactionId,
    overload: AbilityId,
}

/// Rules, the player, and the warp into the arena.
fn start(
    mut commands: Commands,
    seed: Res<Seed>,
    chosen: Res<Chosen>,
    kinds: Res<EffectKinds>,
    mut warps: MessageWriter<WarpRequest>,
    mut log: ResMut<MessageLog>,
    mut next: ResMut<NextState<EngineState>>,
) {
    let arena = Arena::new(seed.0);
    let content = Content::new();
    let abilities = content.abilities();

    let facs = Registry::from_defs(vec![FactionDef { name: "you".into() }, FactionDef { name: "them".into() }]).unwrap();
    let (you, them) = (facs.expect("you"), facs.expect("them"));
    let mut factions = Factions::new(&facs);
    factions.set_mutual(you, them, Relation::Hostile);

    commands.insert_resource(Creatures {
        brute: Arc::new(Brain::new().then(MeleeAdjacent).then(Hunt).then(Wander { chance_pct: 40 })),
        // The ability tactic above the teeth: an adept leads with what it
        // knows and closes only when nothing is worth firing at.
        adept: Arc::new(Brain::new().then(UseAbility { chance_pct: 70 }).then(MeleeAdjacent).then(Hunt)),
        fist: content.kinds.expect("blunt"),
        them,
        overload: abilities.expect("overload"),
    });
    commands.insert_resource(SmokeTile(arena.tiles.expect("smoke")));
    commands.insert_resource(StatRules(content.stats.clone()));
    commands.insert_resource(StatusRules { defs: content.statuses.clone() });
    commands.insert_resource(Slots(content.slots.clone()));
    commands.insert_resource(CombatRules { kinds: content.kinds.clone(), factions });
    commands.insert_resource(DamageStages(vec![Box::new(SubtractArmor)]));
    commands.insert_resource(CombatRng::for_run(seed.0));
    commands.insert_resource(AbilityRng::for_run(seed.0));

    // The effects were registered while the app was built, so an ability
    // naming one nobody added fails here, at startup, naming the ability.
    let built = Abilities::build(abilities, &kinds, &content).unwrap_or_else(|e| panic!("assets: {e}"));
    commands.insert_resource(built);

    commands.insert_resource(arena.appearance());
    commands.insert_resource(WorldMap::new(arena.tiles.tables()));
    commands.insert_resource(PlaceRulesRes(Box::new(arena)));
    commands.insert_resource(content);

    let player = commands
        .spawn((
            (Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(14), RevealsMap, Speed(100)),
            (Health::full(60), Armor(1), Faction(you), MeleeAttack { kind: DamageKindId::from_raw(3), dice: DiceRoll::new(1, 4) }),
            (StatBlock::default(), Afflicted::default(), Pools::new(), Cooldowns::new(), Known::new(), Grants(Vec::new())),
            (Inventory::default(), Equipped(Equipment::with_slot_count(2)), Coin(0), Name::new("you")),
            Glyph::new('@', Color::WHITE).on_layer(10),
        ))
        .id();
    warps.write(WarpRequest::into_place(player, ARENA));
    log.push(format!("Seed {}. The {} set.", seed.0.0, SETS[chosen.0].0), Tones::NOTICE, 0);
    log.push("1-4 aim   a abilities   Tab change set   . wait   q quit", Tones::TEXT, 0);
    next.set(EngineState::Playing);
}

/// Everything in the arena, once it exists: the player's kit and pools,
/// and the three kinds of thing to point an ability at.
/// The tables `populate` fills the arena from.
#[derive(bevy::ecs::system::SystemParam)]
struct Tables<'w> {
    creatures: Res<'w, Creatures>,
    content: Res<'w, Content>,
    chosen: Res<'w, Chosen>,
    abilities: Res<'w, Abilities>,
    seed: Res<'w, Seed>,
}

fn populate(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, tables: Tables, map: Res<WorldMap>, player: Query<Entity, With<Player>>) {
    let Tables { creatures, content, chosen, abilities, seed } = &tables;
    for ev in entered.read() {
        if !ev.first {
            continue;
        }
        let Some(place) = map.place(ev.map) else { continue };
        let bounds = place.terrain.bounds();
        let mut rng = seed.0.rng(SeedDomain::new(b"knacks"), 1);
        let mut spot = |min: i32| loop {
            let p = Point::new(rng.random_range(bounds.x..bounds.right()), rng.random_range(bounds.y..bounds.bottom()));
            if map.is_walkable(p) && geometry::chebyshev(p, ev.entry) >= min {
                return p;
            }
        };
        if let Ok(you) = player.single() {
            kit_out(&mut commands, you, content, chosen.0, abilities);
        }
        for _ in 0..4 {
            commands.spawn((
                (Actor, Blocks, Position(spot(5)), Name::new("a brute"), Health::full(24), Faction(creatures.them), Speed(100)),
                (Perception(9), Mind(creatures.brute.clone()), MeleeAttack { kind: creatures.fist, dice: DiceRoll::new(1, 5) }),
                (StatBlock::default(), Afflicted::default(), Coin(0)),
                Glyph::new('b', Color::srgb(0.85, 0.45, 0.35)).on_layer(5),
            ));
        }
        for _ in 0..2 {
            commands.spawn((
                (Actor, Blocks, Position(spot(7)), Name::new("a sentry"), Health::full(18), Faction(creatures.them), Speed(100)),
                (Perception(9), Mind(creatures.brute.clone()), MeleeAttack { kind: creatures.fist, dice: DiceRoll::new(1, 3) }),
                (StatBlock::default(), Afflicted::default(), Coin(40)),
                Glyph::new('s', Color::srgb(0.8, 0.75, 0.4)).on_layer(5),
            ));
        }
        // One that answers in kind: the same resolver, the same file, the
        // other side of the room.
        let mut pools = Pools::new();
        pools.set(content.stats.expect("power"), 40);
        commands.spawn((
            (Actor, Blocks, Position(spot(9)), Name::new("an adept"), Health::full(20), Faction(creatures.them), Speed(100)),
            (Perception(12), Mind(creatures.adept.clone()), MeleeAttack { kind: creatures.fist, dice: DiceRoll::new(1, 3) }),
            (StatBlock::default(), Afflicted::default(), pools, Cooldowns::new(), Known::new(), Grants(vec![creatures.overload])),
            (Inventory::default(), Coin(10)),
            Glyph::new('a', Color::srgb(0.5, 0.75, 0.95)).on_layer(5),
        ));
    }
}

/// Gives the player the chosen set, the pools it draws on and the kit its
/// requirements ask for.
///
/// Everything genre-shaped happens here, and all of it is lookups by name.
fn kit_out(commands: &mut Commands, you: Entity, content: &Content, set: usize, abilities: &Abilities) {
    let names: Vec<String> =
        rl_engine::rl_rules::ability::load(SETS[set].1, content).expect("the set loaded once already").iter().map(|(_, d)| d.name.clone()).collect();
    let grants: Vec<AbilityId> = names.iter().map(|n| abilities.expect(n)).collect();

    let mut pools = Pools::new();
    for (id, stat) in content.stats.iter() {
        pools.set(id, stat.base);
    }

    // The kit the sets ask for: powder and rum for a pirate, cash for a
    // thief, a shield for a man-at-arms. A requirement that reads the slot
    // graph needs something in the slot.
    let mut bag = Vec::new();
    let mut carry = |commands: &mut Commands, tag: &str, count: u32| {
        let tag = content.tags.expect(tag);
        bag.push(commands.spawn((Item, Tagged(vec![tag]), Stack { key: tag.raw() as u64, count })).id());
    };
    carry(commands, "powder", 6);
    carry(commands, "rum", 3);
    carry(commands, "cash", 5);

    let shield = commands.spawn((Item, Tagged(vec![content.tags.expect("shield")]), Wearable(EquipShape::in_slot(content.slots.expect("off hand"))))).id();
    bag.push(shield);
    let mut worn = Equipment::with_slot_count(2);
    worn.equip(shield, &EquipShape::in_slot(content.slots.expect("off hand"))).expect("one shield, one arm");

    commands.entity(you).insert((Grants(grants), pools, Cooldowns::new(), Inventory { items: bag }, Equipped(worn)));
}

/// Swaps the player's set. Costs no turn: it is a switch on the example,
/// not a move in a game.
fn switch_set(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut chosen: ResMut<Chosen>,
    content: Res<Content>,
    abilities: Res<Abilities>,
    mut log: ResMut<MessageLog>,
    player: Query<Entity, With<Player>>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    let Ok(you) = player.single() else { return };
    chosen.0 = (chosen.0 + 1) % SETS.len();
    kit_out(&mut commands, you, &content, chosen.0, &abilities);
    log.push(format!("The {} set.", SETS[chosen.0].0), Tones::NOTICE, 0);
}

type PlayerTurn<'w, 's> = Query<'w, 's, (Entity, &'static Position, &'static Known), (With<Player>, With<MyTurn>)>;

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

const SLOTS: [KeyCode; 4] = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4];

#[derive(bevy::ecs::system::SystemParam)]
struct PlayerIntents<'w> {
    steps: MessageWriter<'w, Intent<Step>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    aims: MessageWriter<'w, AimAt>,
}

/// Keys to intents.
///
/// An ability key writes [`AimAt`] and stops there. Whether that opens a
/// cursor, which cell it opens on, how it is steered and what it costs are
/// all the engine's, which is why this function has no idea what any of
/// the eighteen abilities do.
fn player_input(
    keys: Res<ButtonInput<KeyCode>>,
    occupancy: Res<Occupancy>,
    mut modals: ResMut<Modals>,
    player: PlayerTurn,
    mut intents: PlayerIntents,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
        return;
    }
    let abilities = ability_modal(&modals);
    if keys.just_pressed(KeyCode::KeyA) {
        modals.toggle(abilities);
        return;
    }
    // Any screen that is up owns the keys, the targeting cursor included.
    if modals.any_open() {
        return;
    }
    let Ok((entity, pos, known)) = player.single() else { return };
    if let Some((_, dir)) = MOVES.iter().find(|(codes, _)| keys.any_just_pressed(codes.iter().copied())) {
        match occupancy.first_at(pos.0 + dir.offset()) {
            Some(other) => {
                intents.attacks.write(Intent::new(entity, Attack(other)));
            }
            None => {
                intents.steps.write(Intent::new(entity, Step(*dir)));
            }
        }
        return;
    }
    if keys.just_pressed(KeyCode::Period) || keys.just_pressed(KeyCode::Numpad5) {
        intents.waits.write(Intent::new(entity, Wait));
        return;
    }
    if let Some(slot) = SLOTS.iter().position(|k| keys.just_pressed(*k))
        && let Some((ability, _)) = known.iter().nth(slot)
    {
        intents.aims.write(AimAt { user: entity, ability });
    }
}

/// What happened, in words.
fn narrate(
    mut events: MessageReader<AbilityEvent>,
    mut deaths: MessageReader<DeathEvent>,
    abilities: Res<Abilities>,
    mut log: ResMut<MessageLog>,
    names: Query<&Name>,
    players: Query<(), With<Player>>,
) {
    for ev in events.read() {
        match ev {
            AbilityEvent::Used { user, ability, targets, .. } => {
                let what = &abilities.get(*ability).name;
                let who = if players.contains(*user) { "You".to_string() } else { first_letter_upper(name_of(*user, &names)) };
                let verb = if players.contains(*user) { "use" } else { "uses" };
                match targets.len() {
                    0 => log.push(format!("{who} {verb} {what}."), Tones::TEXT, 0),
                    1 => log.push(format!("{who} {verb} {what} on {}.", name_of(targets[0], &names)), Tones::NOTICE, 0),
                    n => log.push(format!("{who} {verb} {what}, catching {n}."), Tones::NOTICE, 0),
                }
            }
            AbilityEvent::Refused { user, ability, why } => {
                if !players.contains(*user) {
                    continue;
                }
                let what = &abilities.get(*ability).name;
                log.push(format!("You cannot use {what}: {}.", reasons(why)), Tones::BAD, 0);
            }
        }
    }
    for d in deaths.read() {
        if !d.was_player {
            log.push(format!("{} falls.", first_letter_upper(name_of(d.entity, &names))), Tones::GOOD, 0);
        }
    }
}

/// The pools the chosen set draws on, the set's name and the purse.
///
/// Facets rather than bars: a one-row strip has room for one bar and puts
/// everything else on the line beside it, so four pools would be four
/// bars nobody sees. Only the pools this set actually spends are shown,
/// which is why a thief's strip reads `nerve` and a marine's `power`
/// without either word appearing in a match arm.
fn show_pools(
    mut vitals: ResMut<VitalsView>,
    mut facets: ResMut<Facets>,
    content: Option<Res<Content>>,
    abilities: Option<Res<Abilities>>,
    chosen: Res<Chosen>,
    player: Query<(&Pools, &StatBlock, &Known, Option<&Coin>), With<Player>>,
) {
    let (Some(content), Some(abilities), Ok((pools, stats, known, coin))) = (content, abilities, player.single()) else { return };
    vitals.facets.push(facets.facet("set", SETS[chosen.0].0));
    let mut drawn: Vec<StatId> = Vec::new();
    for (id, _) in known.iter() {
        for cost in &abilities.get(id).costs {
            if let Cost::Pool { stat, .. } = cost
                && !drawn.contains(stat)
            {
                drawn.push(*stat);
            }
        }
    }
    for stat in drawn {
        let def = content.stats.get(stat);
        vitals.facets.push(facets.facet(&def.name, format!("{} {}/{}", def.name, pools.get(stat), stats.0.value(stat, &content.stats))));
    }
    if let Some(coin) = coin {
        vitals.facets.push(facets.facet("coin", format!("coin {}", coin.0)));
    }
}

fn name_of(e: Entity, names: &Query<&Name>) -> String {
    names.get(e).map(|n| n.as_str().to_string()).unwrap_or_else(|_| "something".into())
}

fn first_letter_upper(s: String) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}

/// Every reason at once, which is what the gate reports and what a player
/// wants to read.
fn reasons(why: &[Blocked]) -> String {
    let words: Vec<&str> = why
        .iter()
        .map(|b| match b {
            Blocked::Cooling { .. } => "not ready yet",
            Blocked::Cannot(_) => "nothing left to spend",
            Blocked::Needs(_) => "something is missing",
            Blocked::NoTarget => "nothing to aim at",
        })
        .collect();
    words.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use rl_engine::rl_bevy::plugin::headless_app;

    /// A run with no window: the same plugins, the same effects, the same
    /// startup, so a test exercises the real wiring.
    fn headless(set: usize) -> App {
        let mut app = headless_app();
        app.add_plugins((bevy::input::InputPlugin, FovPlugin, CombatPlugin, ItemsPlugin, StatusPlugin, AbilitiesPlugin))
            // The same panels the game adds, so a key handler that asks for
            // the ability menu's modal finds it. They draw into a terminal,
            // so there is one, and nothing looks at it.
            .add_plugins((UiPlugin, TargetViewPlugin, AbilityPanel::new(Rect::new(0, 0, 36, 14))))
            .insert_resource(rl_engine::rl_render::Terminal::new(COLS, ROWS, Vec2::ONE))
            .init_resource::<Script>()
            .add_systems(PreUpdate, play.after(bevy::input::InputSystems))
            .add_engine_effects()
            .add_effect::<Drain>()
            .add_effect::<Plunder>()
            .add_effect::<Hack>()
            .add_effect::<Banner>()
            .add_effect::<Smoke>()
            .init_resource::<MessageLog>()
            .insert_resource(Seed(RunSeed(9)))
            .insert_resource(Chosen(set))
            .add_systems(Startup, start)
            .add_systems(Update, (player_input, switch_set).in_set(EngineSet::Input))
            .add_systems(Turn, populate.in_set(TurnSet::React));
        for _ in 0..3 {
            app.update();
        }
        app
    }

    fn player(app: &mut App) -> Entity {
        let w = app.world_mut();
        let mut q = w.query_filtered::<Entity, With<Player>>();
        q.single(w).expect("a player")
    }

    /// Puts `what` next to the player and returns it, so an adjacent
    /// ability has something to land on.
    fn beside(app: &mut App, what: impl Bundle) -> Entity {
        let you = player(app);
        let at = app.world().get::<Position>(you).expect("a position").0;
        let free = [Direction::East, Direction::West, Direction::North, Direction::South]
            .into_iter()
            .map(|d| at + d.offset())
            .find(|p| app.world().resource::<WorldMap>().is_walkable(*p) && !app.world().resource::<Occupancy>().is_occupied(*p))
            .expect("room beside the player");
        app.world_mut().spawn((Actor, Blocks, Position(free), Health::full(20), Faction(FactionId::from_raw(1)), Speed(100), what)).id()
    }

    /// A key to deliver on the next frame, and the one to lift after it.
    #[derive(Resource, Default)]
    struct Script {
        next: Option<KeyCode>,
        held: Option<KeyCode>,
    }

    /// Plays [`Script`] after the input plugin has cleared the frame, the
    /// way a keyboard delivers a key. A press staged from outside the
    /// schedule is wiped in `PreUpdate` before any system can see it.
    fn play(mut script: ResMut<Script>, mut keys: ResMut<ButtonInput<KeyCode>>) {
        if let Some(k) = script.held.take() {
            keys.release(k);
        }
        if let Some(k) = script.next.take() {
            keys.press(k);
            script.held = Some(k);
        }
    }

    /// Presses `key` for one frame and releases it on the next.
    fn press(app: &mut App, key: KeyCode) {
        app.world_mut().resource_mut::<Script>().next = Some(key);
        app.update();
        app.update();
    }

    fn use_it(app: &mut App, name: &str, aim: Point) {
        let you = player(app);
        let ability = app.world().resource::<Abilities>().expect(name);
        app.world_mut().write_message(Intent::new(you, Use { ability, aim }));
        app.update();
    }

    /// The claim, as one assertion: five genres of ability in one table,
    /// loaded by the same loader, addressed by the same resolver.
    #[test]
    fn all_five_sets_load_into_one_registry() {
        let content = Content::new();
        let all = content.abilities();
        assert_eq!(all.len(), 18, "eighteen abilities across five genres");
        for name in ["fireball", "broadside", "overload", "shield bash", "smoke bomb"] {
            assert!(all.id(name).is_some(), "{name} is missing");
        }
        // One registry means one namespace, which is what makes the
        // switch in `kit_out` a change of grants and nothing more.
        assert_ne!(all.expect("fireball"), all.expect("overload"));
    }

    /// Fantasy's own effect reaches a component the engine owns but ships
    /// no effect for.
    #[test]
    fn drain_takes_health_and_gives_back_mana() {
        let mut app = headless(0);
        let you = player(&mut app);
        let mana = app.world().resource::<Content>().stats.expect("mana");
        app.world_mut().get_mut::<Pools>(you).expect("pools").set(mana, 10);
        let them = beside(&mut app, StatBlock::default());
        let at = app.world().get::<Position>(them).expect("a position").0;

        use_it(&mut app, "drain", at);
        assert!(app.world().get::<Health>(them).unwrap().hp < 20, "it hurt");
        assert!(app.world().get::<Pools>(you).unwrap().get(mana) > 10, "and it fed");
    }

    /// The pirate's own effect moves one of this game's components
    /// between two entities.
    #[test]
    fn plunder_moves_the_coin_across() {
        let mut app = headless(1);
        let you = player(&mut app);
        let them = beside(&mut app, (StatBlock::default(), Coin(40)));
        let at = app.world().get::<Position>(them).expect("a position").0;

        use_it(&mut app, "plunder", at);
        assert_eq!(app.world().get::<Coin>(you).map(|c| c.0), Some(40), "the whole purse");
        assert_eq!(app.world().get::<Coin>(them).map(|c| c.0), Some(0));
    }

    /// The marine's own effect replaces an engine component outright.
    #[test]
    fn a_hacked_sentry_changes_sides() {
        let mut app = headless(2);
        let you = player(&mut app);
        let mine = app.world().get::<Faction>(you).copied().expect("a side");
        let them = beside(&mut app, StatBlock::default());
        let at = app.world().get::<Position>(them).expect("a position").0;
        assert_ne!(app.world().get::<Faction>(them).unwrap().0, mine.0);

        use_it(&mut app, "hack", at);
        assert_eq!(app.world().get::<Faction>(them).unwrap().0, mine.0, "it came over");
    }

    /// The man-at-arms' own effect spawns an entity where the ability
    /// landed.
    #[test]
    fn a_banner_is_planted_where_it_lands() {
        let mut app = headless(3);
        let you = player(&mut app);
        let at = app.world().get::<Position>(you).expect("a position").0;
        assert_eq!(app.world_mut().query_filtered::<Entity, With<Prop>>().iter(app.world()).count(), 0);

        use_it(&mut app, "banner", at);
        let planted: Vec<Point> = {
            let w = app.world_mut();
            let mut q = w.query_filtered::<&Position, With<Prop>>();
            q.iter(w).map(|p| p.0).collect()
        };
        assert_eq!(planted, vec![at], "one banner, where the user stands");
    }

    /// The thief's own effect edits the map itself, which is as far as
    /// the escape hatch goes and still one function long.
    #[test]
    fn a_smoke_bomb_writes_tiles_that_block_sight() {
        let mut app = headless(4);
        let you = player(&mut app);
        let at = app.world().get::<Position>(you).expect("a position").0;
        let smoke = app.world().resource::<SmokeTile>().0;
        let target = at.offset(3, 0);
        assert!(app.world().resource::<WorldMap>().is_walkable(target), "somewhere to throw it");
        assert!(!app.world().resource::<WorldMap>().is_opaque(target));

        use_it(&mut app, "smoke bomb", target);
        let map = app.world().resource::<WorldMap>();
        assert_eq!(map.tile(target), Some(smoke), "smoke where it burst");
        assert!(map.is_opaque(target), "and it blocks sight");
    }

    /// A requirement that reads the slot graph: the shield is on the arm,
    /// so the bash is allowed; take it off and the gate says no.
    #[test]
    fn shield_bash_needs_the_shield_the_slot_graph_says_it_needs() {
        let mut app = headless(3);
        let you = player(&mut app);
        let them = beside(&mut app, StatBlock::default());
        let at = app.world().get::<Position>(them).expect("a position").0;

        use_it(&mut app, "shield bash", at);
        assert!(app.world().get::<Health>(them).unwrap().hp < 20, "the shield was on the arm");

        // Take it off and try again: refused, and it costs nothing.
        let worn: Vec<Entity> = app.world().get::<Equipped>(you).expect("gear").0.worn().map(|(_, i)| i).collect();
        for item in worn {
            app.world_mut().get_mut::<Equipped>(you).expect("gear").0.unequip(item);
        }
        let before = app.world().get::<Health>(them).unwrap().hp;
        let clock = app.world().resource::<Turns>().now();
        use_it(&mut app, "shield bash", at);
        assert_eq!(app.world().get::<Health>(them).unwrap().hp, before, "nothing landed");
        assert_eq!(app.world().resource::<Turns>().now(), clock, "and no time passed");
    }

    /// Switching sets is a change of grants and nothing else, which is
    /// the whole example in one test.
    #[test]
    fn changing_the_set_changes_only_what_the_player_knows() {
        let mut app = headless(0);
        let you = player(&mut app);
        let fantasy: Vec<AbilityId> = app.world().get::<Known>(you).expect("known").iter().map(|(a, _)| a).collect();
        assert_eq!(fantasy.len(), 4, "the fantasy set");
        let names = |app: &App, ids: &[AbilityId]| ids.iter().map(|i| app.world().resource::<Abilities>().get(*i).name.clone()).collect::<Vec<_>>();
        assert!(names(&app, &fantasy).contains(&"fireball".to_string()));

        // Run the key handler directly: the input plugin clears
        // `just_pressed` in PreUpdate, so a press staged before `update`
        // never reaches Update.
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::Tab);
        app.world_mut().run_system_once(switch_set).expect("the switch ran");
        app.update();

        let pirates: Vec<AbilityId> = app.world().get::<Known>(you).expect("known").iter().map(|(a, _)| a).collect();
        assert!(names(&app, &pirates).contains(&"broadside".to_string()), "{:?}", names(&app, &pirates));
        assert!(!names(&app, &pirates).contains(&"fireball".to_string()), "and the fireball went with the set");
    }

    /// The whole flow through the keys a player presses: an ability key
    /// opens the cursor on the nearest foe, and confirming spends the
    /// turn on it. Nothing in this file aims anything.
    #[test]
    fn an_ability_key_opens_the_cursor_and_enter_fires_it() {
        let mut app = headless(0);
        let you = player(&mut app);
        let them = beside(&mut app, (StatBlock::default(), Name::new("a dummy")));
        app.update();

        // `4` is drain, the fourth of the fantasy set, and adjacent is
        // within a bolt's range.
        press(&mut app, KeyCode::Digit4);
        let view = app.world().resource::<TargetView>();
        assert!(view.aiming(), "the cursor opened");
        assert_eq!(view.cursor, app.world().get::<Position>(them).expect("a position").0, "on the only thing in sight");
        assert_eq!(view.targets.len(), 1);
        assert!(app.world().resource::<Modals>().any_open(), "and it holds the keys");

        let before = app.world().get::<Health>(them).expect("health").hp;
        press(&mut app, KeyCode::Enter);
        assert!(!app.world().resource::<TargetView>().aiming(), "the cursor went away");
        assert!(app.world().get::<Health>(them).expect("health").hp < before, "and the drain landed");
        assert!(app.world().get::<Coin>(you).is_some(), "the player is still whole");
    }

    /// A screen that is up owns the keys: a direction key while aiming
    /// moves the cursor and never the player.
    #[test]
    fn the_cursor_takes_the_keys_while_it_is_up() {
        let mut app = headless(0);
        let you = player(&mut app);
        beside(&mut app, (StatBlock::default(), Name::new("a dummy")));
        app.update();
        let stood = app.world().get::<Position>(you).expect("a position").0;

        press(&mut app, KeyCode::Digit4);
        let aimed = app.world().resource::<TargetView>().cursor;
        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(app.world().get::<Position>(you).expect("a position").0, stood, "the player stayed put");
        assert_eq!(app.world().resource::<TargetView>().cursor, aimed.offset(0, -1), "the cursor moved instead");

        press(&mut app, KeyCode::Escape);
        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(app.world().get::<Position>(you).expect("a position").0, stood.offset(0, -1), "and the keys came back");
    }

    /// An ability aimed at allies counts its user as one, which is what
    /// `Aim::Ally` promises: a hurt marine alone in the arena sprays
    /// themself, the banner says so before the turn is spent, and the
    /// spray mends.
    #[test]
    fn a_spray_aimed_at_allies_mends_the_hurt_user() {
        let mut app = headless(2);
        let you = player(&mut app);
        app.world_mut().get_mut::<Health>(you).expect("health").hp = 20;
        app.update();
        let at = app.world().get::<Position>(you).expect("a position").0;

        // `3` is medspray, the third of the sci-fi set.
        press(&mut app, KeyCode::Digit3);
        let view = app.world().resource::<TargetView>();
        assert!(view.aiming(), "the cursor opened");
        assert_eq!(view.cursor, at, "on the only hurt ally in sight, which is you");
        let named: Vec<&str> = view.targets.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(named, vec!["you"], "and the banner names you");

        press(&mut app, KeyCode::Enter);
        assert!(app.world().get::<Health>(you).expect("health").hp > 20, "and the spray mended you");
    }

    /// What the banner refuses, the resolver refuses: a fireball pointed
    /// at the caster's own feet has nowhere to fly, reads as refused, and
    /// costs nothing when fired anyway.
    #[test]
    fn an_aim_the_banner_refuses_costs_nothing_when_fired() {
        let mut app = headless(0);
        let you = player(&mut app);
        let mana = app.world().resource::<Content>().stats.expect("mana");
        let at = app.world().get::<Position>(you).expect("a position").0;

        // `1` is the fireball; put the cursor back on the caster.
        press(&mut app, KeyCode::Digit1);
        app.world_mut().resource_mut::<TargetView>().cursor = at;
        app.update();
        let view = app.world().resource::<TargetView>();
        assert!(view.aiming());
        assert!(!view.legal, "nowhere to fly, and it says so: {:?}", view.why);

        let before = app.world().get::<Pools>(you).expect("pools").get(mana);
        let clock = app.world().resource::<Turns>().now();
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.world().get::<Pools>(you).expect("pools").get(mana), before, "refused, as the banner said, so nothing was spent");
        assert_eq!(app.world().resource::<Turns>().now(), clock, "and no time passed");
    }

    /// The plainest key there is: a wait spends a turn and opens no screen.
    #[test]
    fn the_wait_key_spends_a_turn_and_opens_no_screen() {
        let mut app = headless(0);
        let clock = app.world().resource::<Turns>().now();
        press(&mut app, KeyCode::Period);
        assert!(!app.world().resource::<Modals>().any_open(), "no screen opened");
        assert!(app.world().resource::<Turns>().now() > clock, "and the turn was spent");
    }
}
