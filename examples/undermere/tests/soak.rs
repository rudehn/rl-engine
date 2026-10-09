//! A soak: fifty seeds of Undermere, each counted floor by floor for what
//! it holds and played to its end by a scripted adventurer, printed for
//! reading the content's numbers against the design's.
//!
//! Not a property and not a tripwire: it asserts only that the runs were
//! played, and it is ignored by a plain `cargo test`, since a hundred and
//! fifty delves are ten minutes of play. Run it by hand when the content
//! is tuned, and read what it prints:
//!
//! `cargo test -p undermere --test soak -- --ignored --nocapture`
//!
//! with `UNDERMERE_SOAK_SEEDS=10` for the first ten seeds only, or
//! `UNDERMERE_SOAK_SEEDS=30,40,` for those seeds alone.
//!
//! Two halves to every seed. [`census`] walks a sturdy adventurer through
//! all ten floors with nobody fighting and counts what each holds, lying
//! about or in a chest, so a run's whole stock is read whatever became of
//! whoever played it. [`delve`] plays the seed with the forty health a run
//! starts with, once for each [`Plan`], as a careful player would: it
//! fights what comes to it, takes up what it finds, wears what is better,
//! drinks what heals when a fight goes badly, tries what it does not know
//! where nothing can see it, puts its scrolls of enchanting into the one
//! thing its plan says, and goes down when its floor is picked clean. What
//! it took up, how long it took, where and to what it died, and what it
//! had at the end are what it counts; the same seed under three plans says
//! whether a run is decided by the seed or by what was made of it.
//!
//! The adventurer knows where everything lies, as a player who walked the
//! whole floor would, but not what it is: a thing not yet known is tried
//! to be known, never read off the armory.

use std::collections::{BTreeMap, BTreeSet};

use bevy::ecs::message::{MessageCursor, Messages};
use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::RunSeed;
use undermere::armory::{Armory, ItemDef, ItemKind};
use undermere::bestiary::{Bestiary, Kind};
use undermere::dungeon::{FLOORS, floor_of, map_of};
use undermere::lore::{Lore, Unfamiliar};
use undermere::stairs::Way;
use undermere::tally::Tally;

/// The seeds every half plays: one to fifty, or one to as many as
/// `UNDERMERE_SOAK_SEEDS` says for a quick look while a change is tried,
/// or the very seeds it lists when it lists them with commas, `30,40,`,
/// for a second look at a few.
fn seeds() -> Vec<u64> {
    let asked = std::env::var("UNDERMERE_SOAK_SEEDS").unwrap_or_default();
    if asked.contains(',') {
        return asked.split(',').filter_map(|n| n.trim().parse().ok()).collect();
    }
    (1..=asked.parse().unwrap_or(50)).collect()
}

/// The kinds a find is counted under, by the tag its kind carries, in the
/// order they are printed. A kind is counted by its finds, as the tally
/// counts what is found: a pile of arrows or of gold is one.
const KINDS: [&str; 9] = ["potion", "scroll", "weapon", "armor", "staff", "wand", "ring", "thrown", "gold"];

/// The kind `def` is counted under: the first of [`KINDS`] it is tagged
/// with, so the torch, a light thrown, is among the thrown things.
fn kind_of(def: &ItemDef, registries: &Registries) -> Option<&'static str> {
    KINDS.into_iter().find(|kind| def.tags.iter().any(|tag| tag.id() == registries.tags.expect(kind)))
}

/// What one floor held the first time it was entered: how many finds of
/// each kind, and how many of each thing by name, a stack by the things in
/// it, so two potions of strength in one chest are two; and the gold its
/// monsters carry, as the coins each would leave on the mean, since what a
/// monster leaves is rolled only when it dies.
#[derive(Debug, Clone, Default)]
struct Holding {
    kinds: BTreeMap<&'static str, u32>,
    names: BTreeMap<String, u32>,
    carried_gold: f32,
}

impl Holding {
    /// The coins lying about or in a chest.
    fn lying_gold(&self) -> u32 {
        self.names.get(undermere::finds::GOLD).copied().unwrap_or(0)
    }
}

/// The coins the monsters standing on `floor` would leave on the mean: each
/// gold row of what they drop at its chance, times its mean count of piles
/// and a pile's mean roll times the floor, as the armory makes a pile there.
fn carried_gold(world: &mut World, floor: u32) -> f32 {
    let map = map_of(floor);
    let armory = world.resource::<Armory>().clone();
    let gold = armory.defs.expect(undermere::finds::GOLD);
    let pile = armory.defs.get(gold).pile.map_or(1.0, |roll| roll.avg()) * floor as f32;
    let mut query = world.query_filtered::<(&Drops<ItemDef>, &OnMap), Without<Dead>>();
    let rows = query.iter(world).filter(|(_, on)| on.0 == map).flat_map(|(drops, _)| drops.0.0.iter().filter(|row| row.item == gold));
    rows.map(|row| row.pct as f32 / 100.0 * (row.count.0 + row.count.1) as f32 / 2.0 * pile).fold(0.0, |all, coins| all + coins)
}

/// Every thing on `floor` and every thing in a chest on it, each with how
/// many it is: what the floor holds before anyone has taken anything.
fn holding(app: &mut App, floor: u32) -> Holding {
    let map = map_of(floor);
    let world = app.world_mut();
    let armory = world.resource::<Armory>().clone();
    let registries = world.resource::<Registries>().clone();
    let mut things: Vec<Entity> =
        world.query_filtered::<(Entity, &OnMap), (With<Item>, With<Position>)>().iter(world).filter(|(_, on)| on.0 == map).map(|(e, _)| e).collect();
    let chests: Vec<Entity> =
        world.query_filtered::<(&Inventory, &OnMap), With<Prop>>().iter(world).filter(|(_, on)| on.0 == map).flat_map(|(bag, _)| bag.items.clone()).collect();
    things.extend(chests);
    let mut held = Holding::default();
    for thing in things {
        let Some(def) = world.get::<ItemKind>(thing).map(|k| armory.defs.get(k.0)) else { continue };
        let Some(kind) = kind_of(def, &registries) else { continue };
        *held.kinds.entry(kind).or_default() += 1;
        *held.names.entry(def.name.clone()).or_default() += world.get::<Stack>(thing).map_or(1, |s| s.count);
    }
    held.carried_gold = carried_gold(world, floor);
    held
}

/// What seed `seed`'s ten floors hold, floor by floor: a sturdy adventurer
/// taken down through every one with nobody fighting anybody, so nothing
/// a floor holds is a thing the dead left on it or a thing taken from it.
fn census(seed: u64) -> Vec<Holding> {
    let mut app = undermere::testing::headless(RunSeed(seed));
    let me = undermere::testing::adventurer(&mut app);
    undermere::testing::sturdy(&mut app, me);
    let peace = CombatRules::new(&app.world().resource::<Registries>().factions);
    app.insert_resource(peace);
    (1..=FLOORS)
        .map(|floor| {
            undermere::testing::arrive(&mut app, map_of(floor));
            holding(&mut app, floor)
        })
        .collect()
}

/// Where a delve's scrolls of enchanting go: the one thing its player
/// commits to, a scroll at a time, as a build is made in play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Plan {
    /// Into the armor on their back.
    Armor,
    /// Into the weapon in their hand.
    Weapon,
    /// Into the staff that strikes hardest, and into the armor until one
    /// is known.
    Staff,
}

/// Every plan, each played on every seed.
const PLANS: [Plan; 3] = [Plan::Armor, Plan::Weapon, Plan::Staff];

/// The staffs a delve strikes with once it knows them, at whatever it
/// sees: the ones that hurt or hold a foe from afar.
const STRIKING: [&str; 4] = ["staff of firebolt", "staff of lightning", "staff of poison", "staff of slowness"];

/// A foe the adventurer can see.
#[derive(Debug, Clone)]
struct Foe {
    who: Entity,
    at: Point,
    /// Steps away, counting a diagonal as one.
    far: i32,
    /// Its health, and the most it has.
    health: (i32, i32),
    /// Whether it shoots from afar.
    shoots: bool,
}

/// A thing the adventurer carries, as they know it.
#[derive(Debug, Clone)]
struct Thing {
    it: Entity,
    def: Id<ItemDef>,
    /// Its kind's name once its kind is known, so a murky potion is `None`.
    known: Option<String>,
    /// Its level, once they know it.
    level: Option<i32>,
    /// Charges left in it, for a thing that has charges.
    left: Option<u16>,
    worn: bool,
    stuck: bool,
    /// The kind it is counted under, which its glyph alone tells.
    kind: Option<&'static str>,
    /// The slot it goes in, by name, for a thing to wear: a sword's is the
    /// main hand, though it may be worn in the other.
    slot: Option<String>,
    /// The slot it is worn in now, by name, while it is worn.
    worn_in: Option<String>,
}

/// What the adventurer decides one of their turns by.
struct Sight {
    here: MapId,
    at: Point,
    health: (i32, i32),
    strength: i32,
    /// Every foe in sight, the nearest first.
    foes: Vec<Foe>,
    pack: Vec<Thing>,
    /// The slots nothing is worn in, by name.
    free: Vec<String>,
}

impl Sight {
    /// Whether their health is at or under `pct` percent of the most.
    fn under(&self, pct: i32) -> bool {
        self.health.0 * 100 <= self.health.1 * pct
    }
}

/// A living body on the place being read: who, where, its health and the
/// most it has, its side, and its kind.
type Body = (Entity, Point, (i32, i32), FactionId, Kind);

/// What a thing to wear is worth, by one measure of it.
type Worth<'a> = &'a dyn Fn(&Thing) -> i32;

/// What `me` sees and carries, as they know it.
fn look(world: &mut World, me: Entity) -> Sight {
    let here = world.resource::<WorldMap>().current();
    let at = world.get::<Position>(me).expect("standing somewhere").0;
    let health = world.get::<Health>(me).map(|h| (h.current, h.max)).expect("alive");
    let registries = world.resource::<Registries>().clone();
    let stats = &registries.stats;
    let strength = world.get::<StatBlock>(me).map_or(12, |s| s.0.value(stats.expect("strength"), stats));
    let side = world.get::<Faction>(me).expect("a side").0;
    let sees = world.get::<Viewshed>(me).cloned();
    let everyone: Vec<Body> = world
        .query_filtered::<(Entity, &Position, &OnMap, &Health, &Faction, &Kind), Without<Dead>>()
        .iter(world)
        .filter(|(_, p, on, _, _, _)| on.0 == here && p.0 != at)
        .map(|(e, p, _, h, f, k)| (e, p.0, (h.current, h.max), f.0, *k))
        .collect();
    let (rules, bestiary) = (world.resource::<CombatRules>(), world.resource::<Bestiary>());
    let mut foes: Vec<Foe> = everyone
        .into_iter()
        .filter(|(_, p, _, f, _)| rules.factions.is_hostile(side, *f) && sees.as_ref().is_some_and(|v| v.can_see(*p)))
        .map(|(who, p, health, _, kind)| Foe { who, at: p, far: geometry::chebyshev(at, p), health, shoots: bestiary.defs.get(kind.0).ranged.is_some() })
        .collect();
    foes.sort_by_key(|f| (f.far, f.who.index()));
    let free = match world.get::<Equipped>(me) {
        Some(worn) => registries.slots.iter().filter(|(slot, _)| worn.0.is_free(*slot)).map(|(slot, _)| registries.slots.name(slot).to_string()).collect(),
        None => Vec::new(),
    };
    let pack = carried(world, me);
    Sight { here, at, health, strength, foes, pack, free }
}

/// Everything `me` carries, worn or not, as they know it: a kind by name
/// only once the run's lore knows it, and a level only once nothing about
/// it is unfamiliar.
fn carried(world: &mut World, me: Entity) -> Vec<Thing> {
    let items = world.get::<Inventory>(me).map(|bag| bag.items.clone()).unwrap_or_default();
    let worn: Vec<(SlotId, Entity)> = world.get::<Equipped>(me).map(|e| e.0.worn().collect()).unwrap_or_default();
    let (armory, lore, registries) = (world.resource::<Armory>(), world.resource::<Lore>(), world.resource::<Registries>());
    let mut things = Vec::new();
    for it in items {
        let Some(id) = world.get::<ItemKind>(it).map(|k| k.0) else { continue };
        let def = armory.defs.get(id);
        let known = lore.knows(&def.name).then(|| def.name.clone());
        let level = (!world.entity(it).contains::<Unfamiliar>()).then(|| world.get::<Enchant>(it).map_or(0, |e| e.0.level));
        let left = world.get::<Consumable>(it).map(|c| c.left);
        let stuck = world.entity(it).contains::<Stuck>();
        let (kind, slot) = (kind_of(def, registries), def.slot.map(|s| registries.slots.name(s.id()).to_string()));
        let worn_in = worn.iter().find(|(_, item)| *item == it).map(|(slot, _)| registries.slots.name(*slot).to_string());
        things.push(Thing { it, def: id, known, level, left, worn: worn_in.is_some(), stuck, kind, slot, worn_in });
    }
    things
}

/// The ground the adventurer walks: the engine's own reading of the place
/// for a mover that opens doors, less every cell a prop stands in the way
/// on, which a walk into would come to an interaction rather than a step,
/// and every cell alight but the one they stand on.
struct Ground<'a, V> {
    view: V,
    map: &'a WorldMap,
    closed: &'a BTreeSet<Point>,
}

impl<V: Grid2D> Grid2D for Ground<'_, V> {
    fn width(&self) -> i32 {
        self.view.width()
    }

    fn height(&self) -> i32 {
        self.view.height()
    }
}

impl<V: CostSource> CostSource for Ground<'_, V> {
    fn cost_idx(&self, idx: usize) -> Option<u32> {
        let at = self.map.to_world(self.view.idx_point(idx));
        if self.closed.contains(&at) { None } else { self.view.cost_idx(idx) }
    }
}

/// The first step from `from` on the shortest way to the nearest of
/// `goals`, round whatever props stand in it, every fire and every one of
/// `around` that is not itself standing on a goal, and through its doors;
/// `None` when none of them can be reached.
fn toward(world: &mut World, from: Point, goals: &[Point], around: &BTreeSet<Entity>) -> Option<Direction> {
    let here = world.resource::<WorldMap>().current();
    let mut closed: BTreeSet<Point> =
        world.query_filtered::<(&Position, &OnMap), (With<Prop>, With<Blocks>)>().iter(world).filter(|(_, on)| on.0 == here).map(|(p, _)| p.0).collect();
    closed.extend(world.resource::<Fire>().burning().filter(|p| *p != from));
    closed.extend(around.iter().filter_map(|who| world.get::<Position>(*who)).map(|p| p.0).filter(|p| !goals.contains(p)));
    let map = world.resource::<WorldMap>();
    let from = map.to_local(from)?;
    let goals: Vec<Point> = goals.iter().filter_map(|g| map.to_local(*g)).collect();
    let ground = Ground { view: map.opening_view(), map, closed: &closed };
    let mut flood = DijkstraMap::covering(&ground);
    flood.build(&ground, goals, PathRules::default());
    Direction::between(from, flood.descend(from)?)
}

/// The first step from `from`, which is alight, on the shortest way to
/// the nearest cell within six steps that is not, through fire if it has
/// to be: a grass fire is run out of, not stood in waiting for a way round.
fn out_of_fire(world: &mut World, from: Point) -> Option<Direction> {
    let here = world.resource::<WorldMap>().current();
    let closed: BTreeSet<Point> =
        world.query_filtered::<(&Position, &OnMap), (With<Prop>, With<Blocks>)>().iter(world).filter(|(_, on)| on.0 == here).map(|(p, _)| p.0).collect();
    let (map, fire) = (world.resource::<WorldMap>(), world.resource::<Fire>());
    let cool: Vec<Point> = geometry::square(from, 6).filter(|p| map.is_walkable_on(here, *p) && !fire.is_burning(*p) && !closed.contains(p)).collect();
    let local = map.to_local(from)?;
    let goals: Vec<Point> = cool.iter().filter_map(|g| map.to_local(*g)).collect();
    let ground = Ground { view: map.opening_view(), map, closed: &closed };
    let mut flood = DijkstraMap::covering(&ground);
    flood.build(&ground, goals, PathRules::default());
    Direction::between(local, flood.descend(local)?)
}

/// What the adventurer does with a turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    /// A walk that way: a blow when a foe stands there, a step otherwise.
    Bump(Direction),
    /// A blow that way, at that foe: a walk into it, remembered by whom
    /// it was struck at.
    Strike(Direction, Entity),
    /// Through the way on underfoot.
    Through,
    /// Whatever lies underfoot, taken up.
    TakeUp,
    /// Everything in a chest beside them, taken out.
    Rummage(Entity),
    /// A thing used from the pack: drunk, read or zapped where it stands.
    Use(Entity),
    /// A scroll read onto another thing carried.
    UseOn(Entity, Entity),
    /// A staff or a wand zapped at a cell.
    Zap(Entity, Point),
    /// A thing thrown at a cell.
    Throw(Entity, Point),
    /// A thing put on.
    Wear(Entity),
    /// A captive beside them freed, by the one thing it offers.
    Free(Entity, VerbId),
    /// Nothing.
    Wait,
}

impl Choice {
    /// The thing the choice is made with, for one that is made with a
    /// thing: what a refusal is remembered by.
    fn thing(self) -> Option<Entity> {
        match self {
            Choice::Rummage(it)
            | Choice::Use(it)
            | Choice::UseOn(it, _)
            | Choice::Zap(it, _)
            | Choice::Throw(it, _)
            | Choice::Wear(it)
            | Choice::Free(it, _) => Some(it),
            _ => None,
        }
    }
}

/// What a delve remembers between its turns.
#[derive(Default)]
struct Memory {
    plan: Option<Plan>,
    /// Things lying about already taken up or found out of reach, and
    /// chests already emptied, by entity.
    done: BTreeSet<Entity>,
    /// Things whose choice was refused since the last turn spent, so the
    /// next choice is another.
    refused: BTreeSet<Entity>,
    /// Things a choice was refused for for good: what would not go on.
    never: BTreeSet<Entity>,
    /// Whether a walk or a blow was refused since the last turn spent.
    stuck: bool,
    /// Turns decided on the floor being played.
    on_floor: u32,
    /// Where the scrolls of enchanting went, by the kind of thing.
    enchanted: BTreeMap<&'static str, u32>,
    /// Blows struck in a row at each foe that took nothing from them.
    futile: BTreeMap<Entity, u32>,
    /// Foes struck [`FUTILE_BLOWS`] times in a row for nothing: armor it
    /// found that their weapon cannot get through, left alone thereafter
    /// as a player would leave it.
    untouchable: BTreeSet<Entity>,
    /// A foe that shoots, gone after, with the turns it has been out of
    /// sight and the turns it has been gone after: one at a time, so two
    /// archers seen by turns are not walked towards by turns, and given up
    /// once it has been out of sight [`CHASE_TURNS`] turns, gone after
    /// [`CHASE_LIMIT`], or cannot be reached.
    chasing: Option<(Entity, u32, u32)>,
    /// Shooters given up on, never gone after again: a coward that keeps
    /// running and shooting would otherwise lead the adventurer round the
    /// floor for good.
    let_go: BTreeSet<Entity>,
    /// A diagonal step refused, and how many of its two straight halves
    /// have been tried since: the engine refuses a diagonal between a
    /// closed door and a wall, which the way it is walked counts as open
    /// ground, so the walk takes it a straight step at a time instead.
    squeezed: Option<(Direction, u8)>,
}

/// How many blows in a row that take nothing from a foe make it one to
/// leave alone.
const FUTILE_BLOWS: u32 = 25;

/// How many turns a shooter out of sight is still gone after.
const CHASE_TURNS: u32 = 15;

/// How many turns a shooter is gone after at most.
const CHASE_LIMIT: u32 = 40;

/// How many of their turns a delve spends on a floor before it leaves
/// what is still lying about and makes for the way down.
const FLOOR_TURNS: u32 = 1500;

/// The adventurer's choice for their turn, the first that applies: out of
/// a fire; a potion when a fight goes badly; a blow at the weakest foe
/// beside them that a blow has not long failed to hurt; a staff, a wand or
/// a thing thrown at a foe further off; with nothing in sight, what to
/// wear, drink and read; whatever lies underfoot; and otherwise a step
/// towards a foe that shoots, the nearest thing still to take, or the way
/// down, or the king on the throne's floor.
fn decide(world: &mut World, me: Entity, memory: &mut Memory) -> Choice {
    let view = look(world, me);
    if world.resource::<Fire>().is_burning(view.at)
        && !memory.stuck
        && let Some(dir) = out_of_fire(world, view.at)
    {
        return Choice::Bump(dir);
    }
    let armory = world.resource::<Armory>().clone();
    let free = |t: &&Thing| !memory.refused.contains(&t.it) && !memory.never.contains(&t.it);
    let usable: Vec<&Thing> = view.pack.iter().filter(free).collect();
    let choices = [heal(&view, &usable), strike(&view, memory), from_afar(&view, &usable, &armory), chores(&view, &usable, &armory, memory)];
    choices.into_iter().flatten().next().unwrap_or_else(|| walk(world, &view, memory))
}

/// The first of `things` whose kind is known to be one of `names`, in the
/// order of the names, and not spent.
fn first<'a>(things: &[&'a Thing], names: &[&str]) -> Option<&'a Thing> {
    names.iter().find_map(|name| things.iter().find(|t| t.known.as_deref() == Some(*name) && t.left != Some(0)).copied())
}

/// A thing of `kind` not yet known, carried.
fn unknown<'a>(things: &[&'a Thing], kind: &str) -> Option<&'a Thing> {
    things.iter().find(|t| t.known.is_none() && t.kind == Some(kind)).copied()
}

/// A potion when a fight goes badly, at half health with a foe in sight or
/// under a third at any time: the extra healing first when it is that bad,
/// and the plain otherwise. Nothing known that heals, and under a third in
/// a fight, the way out they know, or the chance an unknown potion heals.
fn heal(view: &Sight, things: &[&Thing]) -> Option<Choice> {
    let fighting = !view.foes.is_empty();
    let dire = view.under(33);
    if !(dire || (fighting && view.under(50))) {
        return None;
    }
    let order = if dire { ["potion of extra healing", "potion of healing"] } else { ["potion of healing", "potion of extra healing"] };
    if let Some(potion) = first(things, &order) {
        return Some(Choice::Use(potion.it));
    }
    if !(fighting && dire) {
        return None;
    }
    let out = first(things, &["scroll of teleportation", "potion of descent"]).or_else(|| unknown(things, "potion"));
    out.map(|t| Choice::Use(t.it))
}

/// A blow at the weakest foe beside them.
fn strike(view: &Sight, memory: &Memory) -> Option<Choice> {
    if memory.stuck {
        return None;
    }
    let foe = view.foes.iter().filter(|f| f.far <= 1 && !memory.untouchable.contains(&f.who)).min_by_key(|f| (f.health.0, f.who.index()))?;
    Direction::between(view.at, foe.at).map(|dir| Choice::Strike(dir, foe.who))
}

/// A staff or wand at the nearest foe two to seven steps off: one known to
/// strike, a wand of teleport other when they are hurt, one of domination
/// at a foe half beaten, and one not yet known, to know it; or a dart or a
/// javelin at a foe five steps off or nearer. A staff used where it stands
/// rather than aimed is used, once a foe is near.
fn from_afar(view: &Sight, things: &[&Thing], armory: &Armory) -> Option<Choice> {
    let foe = view.foes.iter().find(|f| (2..=7).contains(&f.far))?;
    for t in things.iter().filter(|t| matches!(t.kind, Some("staff" | "wand")) && t.left != Some(0)) {
        let wanted = match t.known.as_deref() {
            None => true,
            Some("wand of teleport other") => view.under(50),
            Some("wand of domination") => foe.health.0 * 2 <= foe.health.1,
            Some("staff of conjuration") => foe.far <= 3,
            Some(name) => STRIKING.contains(&name),
        };
        if !wanted {
            continue;
        }
        let aimed = armory.defs.get(t.def).triggers.iter().any(|trigger| trigger.targeting.is_some());
        return Some(if aimed { Choice::Zap(t.it, foe.at) } else { Choice::Use(t.it) });
    }
    let thrown = first(things, &["javelin", "dart"]).filter(|_| foe.far <= 5)?;
    Some(Choice::Throw(thrown.it, foe.at))
}

/// A step, or what ends one: whatever lies underfoot taken up; a step
/// towards a foe that shoots, seen or lately seen, so it is fought rather
/// than let shoot; a chest beside them emptied; a step to the nearest
/// thing still lying about or in a chest, until the floor has had its
/// turns; and otherwise the way on, through it when they stand on it, or
/// on the throne's floor the king. A walk refused since the last turn
/// spent is a wait.
fn walk(world: &mut World, view: &Sight, memory: &mut Memory) -> Choice {
    if let Some((dir, tried)) = memory.squeezed.filter(|(_, tried)| *tried < 2) {
        memory.squeezed = Some((dir, tried + 1));
        let (dx, dy) = (dir.offset().x, dir.offset().y);
        let halves = [Point::new(dx, 0), Point::new(0, dy)];
        if let Some(half) = Direction::between(Point::ZERO, halves[tried as usize]) {
            return Choice::Bump(half);
        }
    }
    if memory.stuck {
        return Choice::Wait;
    }
    let lying: Vec<(Entity, Point)> = world
        .query_filtered::<(Entity, &Position, &OnMap), With<Item>>()
        .iter(world)
        .filter(|(e, _, on)| on.0 == view.here && !memory.done.contains(e))
        .map(|(e, p, _)| (e, p.0))
        .collect();
    if lying.iter().any(|(_, p)| *p == view.at) {
        memory.done.extend(lying.iter().filter(|(_, p)| *p == view.at).map(|(e, _)| *e));
        return Choice::TakeUp;
    }
    // A captive beside them is freed, as a player freeing the one they
    // found shackled would, and never walked into: walking into one asks
    // what to do with it, and a captive shackled in a corridor otherwise
    // held a delve still on the far side of it for good.
    let free = world.resource::<Verbs>().get(undermere::captives::FREE_VERB);
    let shackled = world
        .query::<(Entity, &Position, &OnMap, &Offers)>()
        .iter(world)
        .find(|(e, p, on, offers)| {
            on.0 == view.here && geometry::chebyshev(p.0, view.at) == 1 && !memory.done.contains(e) && offers.0.iter().any(|o| Some(o.verb) == free)
        })
        .map(|(e, ..)| e);
    if let (Some(captive), Some(verb)) = (shackled, free) {
        memory.done.insert(captive);
        return Choice::Free(captive, verb);
    }
    if memory.chasing.is_none()
        && let Some(shooter) = view.foes.iter().find(|f| f.shoots && f.far > 1 && !memory.let_go.contains(&f.who))
    {
        memory.chasing = Some((shooter.who, 0, 0));
    }
    if let Some((who, unseen, chased)) = memory.chasing {
        let here = world.get::<OnMap>(who).is_some_and(|on| on.0 == view.here) && world.get::<Dead>(who).is_none();
        let at = world.get::<Position>(who).map(|p| p.0).filter(|_| here && unseen < CHASE_TURNS && chased < CHASE_LIMIT);
        let step = at.and_then(|at| toward(world, view.at, &[at], &memory.untouchable));
        match step {
            Some(dir) => {
                let seen = view.foes.iter().any(|f| f.who == who);
                memory.chasing = Some((who, if seen { 0 } else { unseen + 1 }, chased + 1));
                return Choice::Bump(dir);
            }
            None => {
                memory.chasing = None;
                memory.let_go.insert(who);
            }
        }
    }
    if memory.on_floor < FLOOR_TURNS {
        let chests: Vec<(Entity, Point)> = world
            .query_filtered::<(Entity, &Position, &OnMap, &Inventory), With<Prop>>()
            .iter(world)
            .filter(|(e, _, on, bag)| on.0 == view.here && !bag.items.is_empty() && !memory.done.contains(e))
            .map(|(e, p, _, _)| (e, p.0))
            .collect();
        if let Some((chest, _)) = chests.iter().find(|(_, p)| geometry::chebyshev(*p, view.at) <= 1) {
            memory.done.insert(*chest);
            return Choice::Rummage(*chest);
        }
        let mut goals: Vec<Point> = lying.iter().map(|(_, p)| *p).collect();
        goals.extend(chests.iter().flat_map(|(_, p)| Direction::ALL.map(|d| *p + d.offset())));
        if !goals.is_empty() {
            match toward(world, view.at, &goals, &memory.untouchable) {
                Some(dir) => return Choice::Bump(dir),
                None => memory.done.extend(lying.iter().map(|(e, _)| *e).chain(chests.iter().map(|(e, _)| *e))),
            }
        }
    }
    let onward = if view.here == undermere::dungeon::TOWN { Way::Gate } else { Way::Down };
    let way_on = world.query::<(&Position, &OnMap, &Way)>().iter(world).find(|(_, on, way)| on.0 == view.here && **way == onward).map(|(p, _, _)| p.0);
    if way_on == Some(view.at) {
        return Choice::Through;
    }
    let goal = way_on.or_else(|| king(world, view.here));
    goal.and_then(|goal| toward(world, view.at, &[goal], &memory.untouchable)).map_or(Choice::Wait, Choice::Bump)
}

/// Where the king stands on `here`, while he stands.
fn king(world: &mut World, here: MapId) -> Option<Point> {
    let everyone: Vec<(Point, Kind)> =
        world.query_filtered::<(&Position, &OnMap, &Kind), Without<Dead>>().iter(world).filter(|(_, on, _)| on.0 == here).map(|(p, _, k)| (p.0, *k)).collect();
    let bestiary = world.resource::<Bestiary>();
    everyone.into_iter().find(|(_, kind)| bestiary.defs.get(kind.0).boss).map(|(p, _)| p)
}

/// Writes `choice` as the intent it stands for.
fn act(world: &mut World, me: Entity, choice: Choice) {
    fn send<A: Action>(world: &mut World, me: Entity, action: A) {
        world.write_message(Intent::new(me, action));
    }
    match choice {
        Choice::Bump(dir) | Choice::Strike(dir, _) => send(world, me, Bump(dir)),
        Choice::Through => send(world, me, GoThrough),
        Choice::TakeUp => send(world, me, PickUp),
        Choice::Rummage(chest) => send(world, me, Take { from: chest, item: None }),
        Choice::Use(it) => send(world, me, UseItem(it)),
        Choice::UseOn(item, on) => send(world, me, UseOn { item, on }),
        Choice::Zap(item, at) => send(world, me, UseItemAt { item, at }),
        Choice::Throw(item, at) => send(world, me, Throw { item, at }),
        Choice::Wear(it) => send(world, me, Equip(it)),
        Choice::Free(target, verb) => send(world, me, Interact { target, verb }),
        Choice::Wait => send(world, me, Wait),
    }
}

/// How a delve ended.
#[derive(Debug, Clone, PartialEq)]
enum End {
    /// The king died.
    Won,
    /// The adventurer died, to what by name, or to the kind of harm that
    /// last struck them when nobody gets the credit.
    Died(String),
    /// Neither, and why: it had played as long as a delve may, or the
    /// adventurer's turn stopped coming round.
    Stalled(&'static str),
}

/// What one floor of a delve came to.
#[derive(Debug, Clone, Default)]
struct Leg {
    /// Whole turns on it.
    turns: u32,
    /// Things taken up on it, by kind, a stack as one.
    taken: BTreeMap<&'static str, u32>,
    /// Coins taken up on it: every rise in the purse, so a pile counts its
    /// coins and not as one find.
    gold: u32,
    /// Health on arriving and on leaving, or dying.
    health: (i32, i32),
}

/// A delve played to its end.
#[derive(Debug, Clone)]
struct Delve {
    plan: Plan,
    /// Each floor reached, by its number.
    legs: BTreeMap<u32, Leg>,
    end: End,
    /// The floor it ended on, and the whole turn.
    at: (u32, u32),
    tally: Tally,
    /// Where its scrolls of enchanting went, by the kind of thing.
    enchanted: BTreeMap<&'static str, u32>,
    /// What the adventurer's armor came to at the end, as a blow meets it.
    armor: i32,
    /// The most health they had at the end, which only a potion of life
    /// raises.
    most: i32,
    /// The level of the best staff of firebolt or lightning they knew they
    /// carried at the end, if they carried one.
    staff: Option<i32>,
    /// Potions of healing and extra healing still in the pack at the end,
    /// known or not: what the run had to spare.
    spare: u32,
    /// The mini-bosses and the boss it met, and what came of them.
    encounters: Encounters,
}

/// The mini-bosses and the boss a delve met: whether a lair is a fight a
/// run can win, and which of the three bosses it came to.
#[derive(Debug, Clone, Default)]
struct Encounters {
    /// Every mini-boss seen, by name.
    minis: Vec<String>,
    /// The mini-bosses seen that died, by whoever's hand.
    fallen: Vec<String>,
    /// The boss seen, by name, once the throne's floor showed it.
    boss: Option<String>,
    /// The artifacts taken up, by name.
    artifacts: Vec<String>,
}

/// The most turns a delve decides before it is called stalled.
const DELVE_TURNS: u32 = 12_000;

/// The most frames a delve waits for the adventurer's turn to come round.
const FRAMES_A_TURN: u32 = 400;

/// What a delve reads off what the frames since the last did: each thing
/// the adventurer took up, counted on `leg` by its kind, but a dart they
/// threw and took up again only the first time; and the last harm that
/// struck them, by who dealt it or by its kind when nobody did.
struct Watch {
    picked: MessageCursor<ItemEvent>,
    harmed: MessageCursor<DamageDealt>,
    died: MessageCursor<DeathEvent>,
    last_harm: String,
    /// What the adventurer has thrown, which taking up again is no find.
    thrown: BTreeSet<Entity>,
    /// Every mini-boss and boss the adventurer has seen, by entity, with
    /// its name and whether it is the boss: read off by name at its death,
    /// when the dead are already despawned.
    met: BTreeMap<Entity, (String, bool)>,
    /// The mini-bosses that died after they were seen, by whoever's hand.
    fallen: Vec<String>,
    /// Every artifact's name: what a mini-boss leaves.
    artifacts: BTreeSet<String>,
    /// The artifacts the adventurer took up, in the order they did.
    took: Vec<String>,
    /// The coins the adventurer carried when last read, by the engine's
    /// count, so what each frame adds to it is what they took up.
    purse: u32,
}

impl Watch {
    /// Notes every mini-boss and boss `me` can see now.
    fn sight(&mut self, world: &mut World, me: Entity) {
        let Some(sees) = world.get::<Viewshed>(me).cloned() else { return };
        let here = world.resource::<WorldMap>().current();
        let seen: Vec<(Entity, Kind)> = world
            .query_filtered::<(Entity, &Position, &OnMap, &Kind), Without<Dead>>()
            .iter(world)
            .filter(|(_, p, on, _)| on.0 == here && sees.can_see(p.0))
            .map(|(e, _, _, k)| (e, *k))
            .collect();
        let bestiary = world.resource::<Bestiary>();
        for (who, kind) in seen {
            let def = bestiary.defs.get(kind.0);
            if def.miniboss || def.boss {
                self.met.entry(who).or_insert_with(|| (def.name.clone(), def.boss));
            }
        }
    }

    fn read(&mut self, world: &World, me: Entity, leg: &mut Leg) {
        let (armory, registries, bestiary) = (world.resource::<Armory>(), world.resource::<Registries>(), world.resource::<Bestiary>());
        for death in self.died.read(world.resource::<Messages<DeathEvent>>()) {
            if let Some((name, false)) = self.met.get(&death.entity) {
                self.fallen.push(name.clone());
            }
        }
        for event in self.picked.read(world.resource::<Messages<ItemEvent>>()) {
            let (actor, item, merged_into) = match *event {
                ItemEvent::Thrown { actor, item, .. } if actor == me => {
                    self.thrown.insert(item);
                    continue;
                }
                ItemEvent::PickedUp { actor, item, merged_into } if !self.thrown.contains(&item) => (actor, item, merged_into),
                _ => continue,
            };
            let kind = [Some(item), merged_into].into_iter().flatten().find_map(|e| world.get::<ItemKind>(e));
            if let Some(name) = kind.filter(|_| actor == me).map(|k| &armory.defs.get(k.0).name).filter(|name| self.artifacts.contains(*name)) {
                self.took.push(name.clone());
            }
            if let Some(kind) = kind.filter(|_| actor == me).and_then(|k| kind_of(armory.defs.get(k.0), registries)) {
                *leg.taken.entry(kind).or_default() += 1;
            }
        }
        let purse = purse(world, me);
        leg.gold += purse.saturating_sub(self.purse);
        self.purse = purse;
        for dealt in self.harmed.read(world.resource::<Messages<DamageDealt>>()).filter(|d| d.target == me && d.dealt > 0) {
            let by = dealt.hit.attacker.or(dealt.hit.credit).and_then(|who| world.get::<Kind>(who)).map(|k| bestiary.defs.get(k.0).name.clone());
            self.last_harm = by.unwrap_or_else(|| registries.damage_kinds.name(dealt.hit.kind).to_string());
        }
    }
}

/// The coins `me` carries, counted as a trade counts them.
fn purse(world: &World, me: Entity) -> u32 {
    let Some(bag) = world.get::<Inventory>(me) else { return 0 };
    world.resource::<Currency>().count(bag, |item| (world.get::<Tagged>(item), world.get::<Stack>(item)))
}

/// Seed `seed` played to its end by the scripted adventurer, spending its
/// scrolls of enchanting as `plan` says: out of Mereholt with what a run
/// sets out with and the forty health it starts with, each turn as
/// [`decide`] chooses, until the king dies, they do, or it has played
/// [`DELVE_TURNS`] turns.
fn delve(seed: u64, plan: Plan) -> Delve {
    let mut app = undermere::testing::headless(RunSeed(seed));
    let me = undermere::testing::adventurer(&mut app);
    let mut memory = Memory { plan: Some(plan), ..Memory::default() };
    let world = app.world();
    let (bestiary, armory) = (world.resource::<Bestiary>(), world.resource::<Armory>());
    let artifacts =
        bestiary.defs.iter().filter(|(_, d)| d.miniboss).flat_map(|(_, d)| d.drops.0.iter().map(|row| armory.defs.get(row.item).name.clone())).collect();
    let mut watch = Watch {
        picked: world.resource::<Messages<ItemEvent>>().get_cursor(),
        harmed: world.resource::<Messages<DamageDealt>>().get_cursor(),
        died: world.resource::<Messages<DeathEvent>>().get_cursor(),
        last_harm: String::from("nothing"),
        thrown: BTreeSet::new(),
        met: BTreeMap::new(),
        fallen: Vec::new(),
        artifacts,
        took: Vec::new(),
        purse: purse(world, me),
    };
    let (mut legs, mut floor, mut arrived, mut decided, mut idle) = (BTreeMap::<u32, Leg>::new(), 0, 0, 0, 0);
    let end = loop {
        let world = app.world_mut();
        let turn = world.resource::<Turns>().turn_number();
        if let Some(ending) = world.get_resource::<Ending>() {
            break match ending.outcome {
                Outcome::Won => End::Won,
                Outcome::Died { by } => End::Died(
                    by.and_then(|who| world.get::<Kind>(who)).map_or(watch.last_harm.clone(), |k| world.resource::<Bestiary>().defs.get(k.0).name.clone()),
                ),
                Outcome::Abandoned => End::Stalled("abandoned"),
            };
        }
        if decided >= DELVE_TURNS {
            break End::Stalled("out of turns");
        }
        if idle >= FRAMES_A_TURN * 10 {
            break End::Stalled("no turn came round");
        }
        let health = world.get::<Health>(me).map_or(0, |h| h.current);
        let now = floor_of(world.resource::<WorldMap>().current()).unwrap_or(0);
        if now != floor {
            if let Some(leg) = legs.get_mut(&floor) {
                (leg.turns, leg.health.1) = (turn - arrived, health);
            }
            legs.entry(now).or_default().health.0 = health;
            (floor, arrived, memory.on_floor, memory.chasing) = (now, turn, 0, None);
        }
        if world.get::<MyTurn>(me).is_none() {
            app.update();
            watch.read(app.world(), me, legs.entry(floor).or_default());
            idle += 1;
            continue;
        }
        idle = 0;
        let before = world.resource::<Turns>().now();
        watch.sight(world, me);
        let choice = decide(world, me, &mut memory);
        if (FLOOR_TURNS * 3..FLOOR_TURNS * 3 + 3).contains(&memory.on_floor) {
            // Long past when it should have gone down: say where it is
            // and what it keeps choosing, for whoever reads a stall.
            let here = world.resource::<WorldMap>().current();
            let ways: Vec<(Point, Way)> =
                world.query::<(&Position, &OnMap, &Way)>().iter(world).filter(|(_, on, _)| on.0 == here).map(|(p, _, w)| (p.0, *w)).collect();
            let at = world.get::<Position>(me).map(|p| p.0);
            let with = choice.thing().and_then(|it| world.get::<Name>(it)).map(|n| n.as_str().to_string());
            eprintln!("seed {seed} {plan:?}: {} turns on floor {floor}, at {at:?}, ways {ways:?}, choosing {choice:?} with {with:?}", memory.on_floor);
            let beside: Vec<String> = world
                .query::<(&Position, &OnMap, &Name, Option<&Faction>)>()
                .iter(world)
                .filter(|(p, on, ..)| on.0 == here && at.is_some_and(|at| geometry::chebyshev(p.0, at) <= 2))
                .map(|(p, _, name, side)| format!("{} at {:?} of {:?}", name.as_str(), p.0, side.map(|s| s.0)))
                .collect();
            eprintln!("    beside them: {beside:?}");
        }
        let level = |world: &World, on: Entity| world.get::<Enchant>(on).map(|e| e.0.level);
        let into = match choice {
            Choice::UseOn(_, on) => Some((on, level(world, on))),
            _ => None,
        };
        let health_of = |world: &World, who: Entity| world.get::<Health>(who).map_or(0, |h| h.current);
        let struck = match choice {
            Choice::Strike(_, foe) => Some((foe, health_of(world, foe))),
            _ => None,
        };
        act(world, me, choice);
        (decided, memory.on_floor) = (decided + 1, memory.on_floor + 1);
        for _ in 0..FRAMES_A_TURN {
            app.update();
            watch.read(app.world(), me, legs.entry(floor).or_default());
            if app.world().contains_resource::<Ending>() || app.world().get::<MyTurn>(me).is_some() {
                break;
            }
        }
        let world = app.world_mut();
        if world.resource::<Turns>().now() == before && !world.contains_resource::<Ending>() {
            match (choice, choice.thing()) {
                (_, Some(it)) if matches!(choice, Choice::Wear(_) | Choice::Rummage(_) | Choice::Free(..)) => _ = memory.never.insert(it),
                (_, Some(it)) => _ = memory.refused.insert(it),
                (Choice::Bump(dir), None) if dir.is_diagonal() && memory.squeezed.is_none() => memory.squeezed = Some((dir, 0)),
                _ => memory.stuck = true,
            }
            continue;
        }
        (memory.stuck, memory.refused, memory.squeezed) = (false, BTreeSet::new(), None);
        if let Some((foe, was)) = struck {
            let futile = memory.futile.entry(foe).or_default();
            *futile = if health_of(world, foe) < was { 0 } else { *futile + 1 };
            if *futile >= FUTILE_BLOWS {
                memory.untouchable.insert(foe);
            }
        }
        if let Some((on, was)) = into
            && level(world, on) > was
            && let Some(kind) = world.get::<ItemKind>(on).and_then(|k| kind_of(world.resource::<Armory>().defs.get(k.0), world.resource::<Registries>()))
        {
            *memory.enchanted.entry(kind).or_default() += 1;
        }
    };
    let armor = app.world_mut().run_system_once(move |loadout: Loadout| loadout.armor(me)).unwrap_or(0);
    let pack = carried(app.world_mut(), me);
    let staff = pack.iter().filter(|t| t.known.as_deref().is_some_and(|name| STRIKING[..2].contains(&name))).map(|t| t.level.unwrap_or(0)).max();
    let spare = {
        let (world, armory) = (app.world(), app.world().resource::<Armory>());
        let heals = |t: &&Thing| matches!(armory.defs.get(t.def).name.as_str(), "potion of healing" | "potion of extra healing");
        pack.iter().filter(heals).map(|t| world.get::<Stack>(t.it).map_or(1, |s| s.count)).sum()
    };
    let world = app.world();
    let (turn, health) = (world.resource::<Turns>().turn_number(), world.get::<Health>(me).map_or((0, 0), |h| (h.current, h.max)));
    if let Some(leg) = legs.get_mut(&floor) {
        (leg.turns, leg.health.1) = (turn - arrived, health.0);
    }
    eprintln!("seed {seed} {plan:?}: {end:?} on floor {floor} at turn {turn}");
    // A death before the middle floors is the one worth reading how it
    // came about, and the log's last forty lines go back far enough to say:
    // a rat a wand of polymorph made a salamander is thirty lines before
    // the salamander's last blow.
    if matches!(end, End::Died(_)) && floor <= 4 {
        let lines: Vec<&str> = world.resource::<MessageLog>().iter().map(|line| line.text.as_str()).collect();
        for line in &lines[lines.len().saturating_sub(40)..] {
            eprintln!("    {line}");
        }
    }
    let tally = world.resource::<Tally>().clone();
    let met = |boss: bool| watch.met.values().filter(|(_, b)| *b == boss).map(|(name, _)| name.clone()).collect::<Vec<String>>();
    let (minis, boss) = (met(false), met(true).into_iter().next());
    let encounters = Encounters { minis, fallen: watch.fallen, boss, artifacts: watch.took };
    Delve { plan, legs, end, at: (floor, turn), tally, enchanted: memory.enchanted, armor, most: health.1, staff, spare, encounters }
}

/// Prints the census: each kind's average a floor, floor by floor; a run's
/// whole stock of the things the design counts; and every thing's count
/// over all the runs, by name.
fn print_census(census: &[Vec<Holding>]) {
    let runs = census.len() as f32;
    let names = ["scroll of enchanting", "potion of strength", "potion of life"];
    println!("\n== What the floors hold, over {} seeds: lying about or in a chest, a kind by its finds and a thing by its count ==", census.len());
    println!("floor {}  enchant strength  life", KINDS.map(|k| format!("{k:>7}")).join(""));
    let count = |h: &Holding, name: &str| h.names.get(name).copied().unwrap_or(0);
    for floor in 1..=FLOORS as usize {
        let mean = |f: &dyn Fn(&Holding) -> u32| census.iter().map(|run| f(&run[floor - 1])).sum::<u32>() as f32 / runs;
        let kinds = KINDS.map(|kind| format!("{:>7.2}", mean(&|h| h.kinds.get(kind).copied().unwrap_or(0)))).join("");
        let named = names.map(|name| format!("{:>8.2}", mean(&|h| count(h, name)))).join("");
        println!("{floor:>5} {kinds}{named}");
    }
    print_census_gold(census);
    println!("A run's whole stock, fewest / mean / most:");
    let whole = |f: &dyn Fn(&Holding) -> u32| census.iter().map(|run| run.iter().map(f).sum::<u32>()).collect::<Vec<u32>>();
    let spread = |all: Vec<u32>| format!("{} / {:.1} / {}", all.iter().min().unwrap(), all.iter().sum::<u32>() as f32 / runs, all.iter().max().unwrap());
    for kind in KINDS {
        println!("  {kind:<22} {}", spread(whole(&|h| h.kinds.get(kind).copied().unwrap_or(0))));
    }
    for name in names {
        println!("  {name:<22} {}", spread(whole(&|h| count(h, name))));
    }
    let mut all: BTreeMap<&str, u32> = BTreeMap::new();
    for holding in census.iter().flatten() {
        for (name, n) in &holding.names {
            *all.entry(name.as_str()).or_default() += n;
        }
    }
    println!("Every thing over all {} runs, a run's mean:", census.len());
    for (name, n) in all {
        println!("  {name:<28} {:>6.2}", n as f32 / runs);
    }
}

/// The gold the town's prices were set against by each floor, counted
/// together: thirty a floor times its number, so ninety by floor two, 450
/// by floor five and 1350 by floor nine.
fn estimated_gold(floor: u32) -> u32 {
    15 * floor * (floor + 1)
}

/// Prints the gold each floor gives on the mean, lying about or in a chest
/// and carried by the monsters standing on it, and the two together counted
/// up floor by floor beside the estimate the prices were set against.
fn print_census_gold(census: &[Vec<Holding>]) {
    let runs = census.len() as f32;
    println!("Gold a floor, on the mean: lying about or in a chest, carried by its monsters, and all of it by that floor against the estimate:");
    println!("floor   lying carried   total by then estimate");
    let mut by_then = 0.0;
    for floor in 1..=FLOORS as usize {
        let mean = |f: &dyn Fn(&Holding) -> f32| census.iter().map(|run| f(&run[floor - 1])).fold(0.0, |all, coins| all + coins) / runs;
        let (lying, carried) = (mean(&|h| h.lying_gold() as f32), mean(&|h| h.carried_gold));
        by_then += lying + carried;
        println!("{floor:>5} {lying:>7.0} {carried:>7.0} {:>7.0} {by_then:>7.0} {:>8}", lying + carried, estimated_gold(floor as u32));
    }
}

/// Prints each plan's delves: how they ended and where, what killed them,
/// what each floor reached came to on the mean, and where the scrolls of
/// enchanting went; then every delve on a line of its own.
fn print_delves(delves: &[(u64, Delve)]) {
    for plan in PLANS {
        let mine: Vec<&Delve> = delves.iter().map(|(_, d)| d).filter(|d| d.plan == plan).collect();
        let runs = mine.len() as f32;
        let won = mine.iter().filter(|d| d.end == End::Won).count();
        let stalled = mine.iter().filter(|d| matches!(d.end, End::Stalled(_))).count();
        println!("\n== {} delves enchanting {plan:?}: {won} won, {stalled} stalled ==", mine.len());
        let mut died = [0u32; FLOORS as usize + 1];
        let mut causes: BTreeMap<&str, u32> = BTreeMap::new();
        for d in &mine {
            if let End::Died(by) = &d.end {
                died[d.at.0 as usize] += 1;
                *causes.entry(by.as_str()).or_default() += 1;
            }
        }
        println!("died on floor: {}", (1..=FLOORS as usize).map(|f| format!("{f}:{}", died[f])).collect::<Vec<_>>().join("  "));
        let mut causes: Vec<(&str, u32)> = causes.into_iter().collect();
        causes.sort_by_key(|(name, n)| (std::cmp::Reverse(*n), *name));
        println!("died to: {}", causes.iter().map(|(name, n)| format!("{name} {n}")).collect::<Vec<_>>().join(", "));
        println!("floor reached turns health-in health-out taken: {}", KINDS.join(" "));
        for floor in 1..=FLOORS {
            let legs: Vec<&Leg> = mine.iter().filter_map(|d| d.legs.get(&floor)).collect();
            if legs.is_empty() {
                continue;
            }
            let n = legs.len() as f32;
            let mean = |f: &dyn Fn(&Leg) -> i32| legs.iter().map(|l| f(l)).sum::<i32>() as f32 / n;
            let taken = KINDS.map(|k| format!("{:.1}", mean(&|l| l.taken.get(k).copied().unwrap_or(0) as i32))).join(" ");
            println!("{floor:>5} {:>7} {:>5.0} {:>9.1} {:>10.1}  {taken}", legs.len(), mean(&|l| l.turns as i32), mean(&|l| l.health.0), mean(&|l| l.health.1));
        }
        println!("Gold taken up on the mean of the delves reaching a floor: there, and all of it by then against the estimate:");
        println!("floor reached  there by then estimate");
        for floor in 1..=FLOORS {
            let reached: Vec<&&Delve> = mine.iter().filter(|d| d.legs.contains_key(&floor)).collect();
            if reached.is_empty() {
                continue;
            }
            let n = reached.len() as f32;
            let there = reached.iter().map(|d| d.legs[&floor].gold).sum::<u32>() as f32 / n;
            let by_then = reached.iter().map(|d| d.legs.range(..=floor).map(|(_, l)| l.gold).sum::<u32>()).sum::<u32>() as f32 / n;
            println!("{floor:>5} {:>7} {there:>6.0} {by_then:>7.0} {:>8}", reached.len(), estimated_gold(floor));
        }
        let mut into: BTreeMap<&str, u32> = BTreeMap::new();
        for d in &mine {
            for (kind, n) in &d.enchanted {
                *into.entry(kind).or_default() += n;
            }
        }
        let deepest = mine.iter().map(|d| d.tally.deepest).sum::<u32>() as f32 / runs;
        let kills = mine.iter().map(|d| d.tally.kills).sum::<u32>() as f32 / runs;
        println!(
            "deepest {deepest:.1} on the mean, {kills:.1} kills; enchanted a run: {}",
            into.iter().map(|(k, n)| format!("{k} {:.1}", *n as f32 / runs)).collect::<Vec<_>>().join(", ")
        );
    }
    // Whether one thing decides a run: those that went down with a staff of
    // firebolt or lightning known and those without, among the delves that
    // got as far as the floor a run is most often lost on.
    for (with, those) in [("with", true), ("without", false)] {
        let deep: Vec<&Delve> = delves.iter().map(|(_, d)| d).filter(|d| d.at.0 >= 9 && d.staff.is_some() == those).collect();
        let won = deep.iter().filter(|d| d.end == End::Won).count();
        println!("{} delves reaching floor nine {with} a striking staff known: {won} won", deep.len());
    }
    println!("\nseed plan       end                       floor  turn  kills drunk found enchanted armor most staff spare");
    for (seed, d) in delves {
        let end = match &d.end {
            End::Won => "won".to_string(),
            End::Died(by) => format!("died to {by}"),
            End::Stalled(why) => format!("stalled, {why}"),
        };
        let enchanted: u32 = d.enchanted.values().sum();
        let t = &d.tally;
        let staff = d.staff.map_or("-".to_string(), |level| format!("{level:+}"));
        println!(
            "{seed:>4} {:<10} {end:<25} {:>5} {:>5} {:>6} {:>5} {:>5} {enchanted:>9} {:>5} {:>4} {staff:>5} {:>5}",
            format!("{:?}", d.plan),
            d.at.0,
            d.at.1,
            t.kills,
            t.potions_drunk,
            t.items_found,
            d.armor,
            d.most,
            d.spare
        );
    }
}

/// Prints every plan's delves together: where and to what they died, the
/// one kind that killed most and its share, and what came of the
/// mini-bosses, the boss and the artifacts, each by name.
fn print_encounters(delves: &[(u64, Delve)]) {
    let all: Vec<&Delve> = delves.iter().map(|(_, d)| d).collect();
    let deaths: Vec<(u32, &str)> = all.iter().filter_map(|d| if let End::Died(by) = &d.end { Some((d.at.0, by.as_str())) } else { None }).collect();
    let won = all.iter().filter(|d| d.end == End::Won).count();
    println!("\n== All {} delves: {won} won ({:.0}%), {} died ==", all.len(), won as f32 * 100.0 / all.len() as f32, deaths.len());
    println!("died on floor: {}", (1..=FLOORS).map(|f| format!("{f}:{}", deaths.iter().filter(|(at, _)| *at == f).count())).collect::<Vec<_>>().join("  "));
    let mut causes: BTreeMap<&str, u32> = BTreeMap::new();
    for (_, by) in &deaths {
        *causes.entry(by).or_default() += 1;
    }
    let mut causes: Vec<(&str, u32)> = causes.into_iter().collect();
    causes.sort_by_key(|(name, n)| (std::cmp::Reverse(*n), *name));
    if let Some((name, n)) = causes.first() {
        println!("the deadliest, {name}, took {n} of {} ({:.0}%)", deaths.len(), *n as f32 * 100.0 / deaths.len().max(1) as f32);
    }
    let named = |of: &dyn Fn(&Delve) -> Vec<String>| {
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        for d in &all {
            for name in of(d) {
                *counts.entry(name).or_default() += 1;
            }
        }
        counts
    };
    let met = named(&|d| d.encounters.minis.clone());
    let fallen = named(&|d| d.encounters.fallen.clone());
    let artifacts = named(&|d| d.encounters.artifacts.clone());
    println!("mini-boss                  met  fell  killed the adventurer");
    for (name, n) in &met {
        let killed = deaths.iter().filter(|(_, by)| by == name).count();
        println!("  {name:<24} {n:>4} {:>5} {killed:>5}", fallen.get(name).copied().unwrap_or(0));
    }
    let with_minis = all.iter().filter(|d| !d.encounters.minis.is_empty()).count();
    let felled = all.iter().filter(|d| !d.encounters.fallen.is_empty()).count();
    let took = all.iter().filter(|d| !d.encounters.artifacts.is_empty()).count();
    println!("{with_minis} delves met a mini-boss, {felled} saw one fall, {took} took up an artifact");
    for (name, n) in &artifacts {
        println!("  {name:<28} {n:>4}");
    }
    println!("boss                       met   won  killed the adventurer");
    for (name, n) in named(&|d| d.encounters.boss.clone().into_iter().collect()) {
        let won = all.iter().filter(|d| d.encounters.boss.as_deref() == Some(name.as_str()) && d.end == End::Won).count();
        let killed = deaths.iter().filter(|(_, by)| *by == name).count();
        println!("  {name:<24} {n:>4} {won:>5} {killed:>5}");
    }
    let turns: Vec<u32> = all.iter().filter(|d| d.end == End::Won).map(|d| d.at.1).collect();
    if !turns.is_empty() {
        println!("a run won took {} turns on the mean", turns.iter().sum::<u32>() / turns.len() as u32);
    }
}

/// Runs `job` on each of `jobs` across the machine's threads, and gives
/// back what each came to in the order the jobs were given.
fn spread<J: Copy + Send + Sync, R: Send>(jobs: &[J], job: impl Fn(J) -> R + Sync) -> Vec<R> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut done: Vec<(usize, R)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(j) = jobs.get(at) else { break mine };
                        mine.push((at, job(*j)));
                    }
                })
            })
            .collect();
        workers.into_iter().flat_map(|w| w.join().expect("a worker played its runs")).collect()
    });
    done.sort_by_key(|(at, _)| *at);
    done.into_iter().map(|(_, r)| r).collect()
}

/// The soak itself: the census of every seed, then every seed delved once
/// under each plan, all printed. Asserts only that every delve got as far
/// as the first floor, so a soak that played nothing is not read as one
/// that played badly.
#[test]
#[ignore]
fn soak_fifty_seeds_counted_and_delved_to_their_ends() {
    let seeds = seeds();
    let census = spread(&seeds, census);
    print_census(&census);
    let jobs: Vec<(u64, Plan)> = seeds.iter().flat_map(|seed| PLANS.map(|plan| (*seed, plan))).collect();
    let delves: Vec<(u64, Delve)> = jobs.iter().map(|(seed, _)| *seed).zip(spread(&jobs, |(seed, plan)| delve(seed, plan))).collect();
    print_delves(&delves);
    print_encounters(&delves);
    assert!(delves.iter().all(|(_, d)| d.legs.contains_key(&1)), "every delve went through the gate");
}

/// What the plan puts a scroll of enchanting into: the armor worn, the
/// weapon in hand, or the hardest-striking staff known, at its highest
/// level, and the armor while no such staff is known.
fn enchantable(view: &Sight, plan: Plan) -> Option<Entity> {
    let worn = |slot: &str| view.pack.iter().find(|t| t.worn_in.as_deref() == Some(slot)).map(|t| t.it);
    match plan {
        Plan::Armor => worn("body"),
        Plan::Weapon => worn("main hand").or_else(|| worn("body")),
        Plan::Staff => {
            let staffs = view.pack.iter().filter(|t| t.known.as_deref().is_some_and(|name| STRIKING[..2].contains(&name)));
            staffs.max_by_key(|t| (t.level.unwrap_or(0), std::cmp::Reverse(t.it.index()))).map(|t| t.it).or_else(|| worn("body"))
        }
    }
}

/// With nothing in sight: a potion of strength or of life drunk once known;
/// something better put on; a scroll of enchanting read into the plan's
/// thing; a curse lifted from something stuck on; a spent staff refilled;
/// a thing not known made known by a scroll of identify; and what is not
/// known tried, a scroll read, onto the plan's thing when it asks, and a
/// potion drunk while they are in good health.
fn chores(view: &Sight, things: &[&Thing], armory: &Armory, memory: &Memory) -> Option<Choice> {
    if !view.foes.is_empty() {
        return None;
    }
    if let Some(potion) = first(things, &["potion of strength", "potion of life"]) {
        return Some(Choice::Use(potion.it));
    }
    if let Some(choice) = dress(view, things, armory) {
        return Some(choice);
    }
    let into = memory.plan.and_then(|plan| enchantable(view, plan));
    if let (Some(scroll), Some(on)) = (first(things, &["scroll of enchanting"]), into) {
        return Some(Choice::UseOn(scroll.it, on));
    }
    let stuck = view.pack.iter().any(|t| t.worn && t.stuck);
    let empty = view.pack.iter().any(|t| matches!(t.kind, Some("staff" | "wand")) && t.left == Some(0));
    for (wanted, scroll) in [(stuck, "scroll of remove curse"), (empty, "scroll of recharging")] {
        if let Some(scroll) = first(things, &[scroll]).filter(|_| wanted) {
            return Some(Choice::Use(scroll.it));
        }
    }
    if let Some(scroll) = first(things, &["scroll of identify"])
        && let Some(on) = view.pack.iter().find(|t| t.known.is_none() || t.level.is_none())
    {
        return Some(Choice::UseOn(scroll.it, on.it));
    }
    if let Some(scroll) = unknown(things, "scroll") {
        let asks = armory.defs.get(scroll.def).triggers.iter().any(|t| t.asks.is_some());
        let on = into.or_else(|| view.pack.iter().find(|t| t.it != scroll.it).map(|t| t.it));
        return Some(match (asks, on) {
            (true, Some(on)) => Choice::UseOn(scroll.it, on),
            _ => Choice::Use(scroll.it),
        });
    }
    let potion = unknown(things, "potion").filter(|_| !view.under(70))?;
    Some(Choice::Use(potion.it))
}

/// Something better to put on, by what the adventurer can tell of it:
/// body armor turning more of a blow than what they wear, or a weapon
/// dealing more a turn, each with its strength's cost counted against it
/// and its level counted once it is known, a sweep worth a third more and
/// a pierce a tenth; and a helm, a shield or a ring where nothing is worn.
fn dress(view: &Sight, things: &[&Thing], armory: &Armory) -> Option<Choice> {
    let short = |t: &Thing| (armory.defs.get(t.def).strength.unwrap_or(0) - view.strength).max(0);
    let turns = |t: &Thing| armory.defs.get(t.def).armor + t.level.unwrap_or(0) - short(t);
    let deals = |t: &Thing| {
        let def = armory.defs.get(t.def);
        let Some(melee) = def.melee else { return 0 };
        let shape = match melee.shape {
            Some(AttackShape::Sweep) => 130,
            Some(AttackShape::Pierce(_)) => 110,
            None => 100,
        };
        let per_turn = undermere::armory::blow_score(melee.roll, def.cost) + 10 * t.level.unwrap_or(0) * 100 / def.cost.unwrap_or(100) as i32;
        per_turn * (100 - 4 * short(t)).max(0) / 100 * shape / 100
    };
    let measures: [(&str, &str, Worth); 2] = [("body", "armor", &turns), ("main hand", "weapon", &deals)];
    for (slot, kind, worth) in measures {
        let worn = view.pack.iter().find(|t| t.worn_in.as_deref() == Some(slot));
        if worn.is_some_and(|w| w.stuck) {
            continue;
        }
        let now = worn.map_or(0, worth);
        let best = things
            .iter()
            .filter(|t| !t.worn && t.kind == Some(kind) && t.slot.as_deref() == Some(slot))
            .max_by_key(|t| (worth(t), std::cmp::Reverse(t.it.index())));
        if let Some(best) = best.filter(|b| worth(b) > now) {
            return Some(Choice::Wear(best.it));
        }
    }
    let free =
        |t: &&&Thing| !t.worn && t.slot.as_deref().is_some_and(|slot| view.free.iter().any(|f| f == slot || (slot == "left finger" && f == "right finger")));
    let spare = things.iter().filter(free).find(|t| matches!(t.kind, Some("armor" | "ring")) && short(t) == 0 && t.level.is_none_or(|l| l >= 0))?;
    Some(Choice::Wear(spare.it))
}
