//! The bestiary: what lives under the lake, loaded from `monsters.ron`,
//! with where each kind turns up from `monster_spawns.ron`.
//!
//! A monster is written as a short core, the things every monster has,
//! and a list of traits from `traits.ron`. [`read`] resolves both at load:
//! the core's attack is read from its `"1d4 slash"`, every name is looked
//! up once, and the traits are folded into one set of values, so a
//! [`MonsterDef`] is the monster with nothing left to look up and a spawn
//! reads plain numbers. Two traits of one monster that set the same value
//! are refused there, naming the monster, the value and both traits.
//!
//! The [`Bestiary`] is Undermere's [`ActorMaker`]: a monster at a piece's
//! slot is made by [`spawn_monster`] exactly as one a floor's population
//! places, so the two can never drift apart.

use std::sync::Arc;

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::{DEFAULT_PERCEPTION, MomentId};
use rl_engine::rl_rules::ai::tactics::{
    FleeWhenHurt, Hunt, Keep, KeepPost, MeleeAdjacent, Scavenge, SearchLastKnown, ShootAtRange, ThrowAtRange, UseAbility, Wander,
};
use rl_engine::rl_rules::damage::DamageKindId;
use rl_engine::rl_rules::faction::FactionDef;
use serde::Deserialize;

use crate::armory::ItemDef;
use crate::traits::{Folded, TraitDef};
use crate::tricks::Tricks;

/// The bestiary file, compiled in so the binary runs from anywhere.
const MONSTERS_RON: &str = include_str!("../assets/monsters.ron");
/// Where each kind turns up, compiled in beside it.
const MONSTER_SPAWNS_RON: &str = include_str!("../assets/monster_spawns.ron");

/// How long a status a trait holds lasts: a whole life, the engine's
/// [`UNTIL_CURED`], which bites and mends every turn and is never counted
/// down, so a panel that counts a status's turns shows none for it.
pub const FOR_LIFE: u32 = UNTIL_CURED;

/// How a monster notices an adventurer trying not to be seen, unless one
/// of its traits says otherwise: sure of one beside it, a quarter of the
/// time beyond that, and sure from four tiles further while the
/// adventurer stands in light, so a lantern is seen coming and the dark
/// is somewhere to hide. The engine's own default has no light bonus at
/// all, which would make the lantern cost nothing.
pub const NOTICE: NoticeStats = NoticeStats { certain: 1, chance_pct: 25, lit_bonus: 4, memory: 6 };

/// One kind of monster, resolved: its core as written and its traits
/// folded, with every name looked up.
#[derive(Debug, Clone)]
pub struct MonsterDef {
    /// Unique; the spawn table and the pieces name it by this.
    pub name: String,
    /// One character.
    pub glyph: char,
    /// `(r, g, b)` in `0..=1`.
    pub color: (f32, f32, f32),
    /// Maximum health.
    pub hp: i32,
    /// Its blow.
    pub attack: MeleeAttack,
    /// Its shot, for a kind that shoots.
    pub ranged: Option<RangedAttack>,
    /// Flat armor: its own and what a trait gives, together.
    pub armor: i32,
    /// Its side.
    pub faction: FactionId,
    /// Whether its death wins the run.
    pub boss: bool,
    /// Whether it is a mini-boss, worth the score's five hundred when the
    /// adventurer kills it.
    pub miniboss: bool,
    /// What it leaves when it dies, by the armory's ids.
    pub drops: DropTable<Id<ItemDef>>,
    /// Its traits' names, in the order the file lists them.
    pub traits: Vec<String>,
    /// Its traits, folded.
    pub folded: Folded,
    /// What it does at its own moments, folded from its traits and built
    /// once: a bite that poisons what it struck, a burst where it died.
    /// Empty for a kind with nothing of its own to answer with.
    pub triggers: Triggers,
    /// Whether it carries the trait of the same name, read off its
    /// `traits` list once at load rather than every spawn: each is a
    /// small game system of its own, built in [`crate::tricks`], and none
    /// is a value the trait model's own folding already knows how to add
    /// up.
    pub tricks: Tricks,
    /// Every trick trait that grants an ability, by the ability's id and
    /// the trait's name: what [`crate::bestiary_lore::learn_from_abilities`]
    /// marks shown in [`crate::bestiary_lore::MonsterLore`] the moment the
    /// ability lands where the adventurer can see it. At most one trait
    /// per ability, since every ability-granting trait in this bestiary
    /// grants its own.
    pub trick_abilities: Vec<(AbilityId, String)>,
    /// Every trick trait landed through the engine's own actor triggers,
    /// by the moment it answers and the trait's name: a bite's poison, a
    /// touch's chill or its sapped strength, a hand that steals, a burst
    /// of spores where something died. What
    /// [`crate::bestiary_lore::learn_from_triggers`] marks shown the
    /// moment [`Fired::at`] is where the adventurer can see it.
    pub trick_moments: Vec<(MomentId, String)>,
    /// Its own id in the bestiary, so a spawn handed only the definition
    /// can mark what it made.
    id: Id<MonsterDef>,
    /// The brain every one of its kind decides with, built once from what
    /// it is.
    brain: Arc<Brain<Entity>>,
    /// The brain one of its kind decides with once it fights for the
    /// adventurer, built beside the other.
    allied: Arc<Brain<Entity>>,
}

impl Named for MonsterDef {
    fn name(&self) -> &str {
        &self.name
    }
}

impl MonsterDef {
    /// Its id in the bestiary it was read into.
    pub fn id(&self) -> Id<MonsterDef> {
        self.id
    }

    /// Its speed, percent of normal: what a trait set, else normal.
    pub fn speed(&self) -> u32 {
        self.folded.speed.unwrap_or(Speed::default().0)
    }

    /// How far it sees, in tiles: what a trait set, else what the engine
    /// gives a mind that names none.
    pub fn perception(&self) -> i32 {
        self.folded.perception.unwrap_or(DEFAULT_PERCEPTION)
    }

    /// Percent health at or below which it runs: what a trait set, else
    /// never.
    pub fn flee_at(&self) -> i32 {
        self.folded.flee_at.unwrap_or(0)
    }

    /// The mind one of its kind thinks with once it fights for the
    /// adventurer, built once for the kind: it strikes as its kind does,
    /// then keeps station on the adventurer's side, then hunts.
    pub fn ally_mind(&self) -> Mind {
        Mind(self.allied.clone())
    }
}

/// How dangerous a kind is, for the score's line for the slain: the
/// engine's `balance::threat` of its health and armor against the harder
/// of its blow and its shot, at its speed. What its abilities do is left
/// out, as the engine's ranking leaves it out.
impl ThreatSubject for MonsterDef {
    fn hp(&self) -> i32 {
        self.hp
    }

    fn armor(&self) -> i32 {
        self.armor
    }

    fn damage_per_hit(&self) -> f32 {
        let blow = self.attack.dice.avg();
        self.ranged.map_or(blow, |shot| blow.max(shot.dice.avg()))
    }

    fn speed_pct(&self) -> u32 {
        self.speed()
    }
}

/// One row of `monster_spawns.ron`: a kind, the floors it turns up on, how
/// often, and how many at once.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonsterSpawn {
    /// Which kind.
    pub monster: NameRef<MonsterDef>,
    /// The first and last floor it applies on, both included.
    pub floors: (i32, i32),
    /// How often, against every other row that applies on a floor.
    pub weight: u32,
    /// The fewest and the most that come together.
    pub group: (u32, u32),
}

/// Marks a monster with the definition it was made from: what a save
/// writes down, and what anything that wants the definition back, a tally
/// of the dead or an examine panel, reads it through.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind(pub Id<MonsterDef>);

/// The monster definitions and the floor-banded table they spawn from.
#[derive(Resource)]
pub struct Bestiary {
    /// The monster definitions, by id.
    pub defs: Registry<MonsterDef>,
    /// What can turn up on which floor, in what groups.
    pub spawns: BandedTable<Id<MonsterDef>>,
}

impl Bestiary {
    /// Loads `monsters.ron` and `monster_spawns.ron` against `registries`,
    /// the `traits` a monster is written as, the `items` it drops, and the
    /// effect `kinds` and `moments` its own triggers are built against, so
    /// a trait naming an unregistered effect is caught here rather than by
    /// a bite that quietly poisons nothing. Panics naming the file and
    /// every problem in it, since a broken bestiary is a game that cannot
    /// start.
    pub fn load(registries: &Registries, traits: &Registry<TraitDef>, items: &Registry<ItemDef>, kinds: &EffectKinds, moments: &Moments) -> Self {
        let defs = read(MONSTERS_RON, registries, traits, items, kinds, moments).unwrap_or_else(|e| panic!("assets/monsters.ron: {e}"));
        let rows: Vec<MonsterSpawn> =
            registries.names().with("monster", &defs).load_list(MONSTER_SPAWNS_RON).unwrap_or_else(|e| panic!("assets/monster_spawns.ron: {e}"));
        let mut spawns = BandedTable::default();
        for row in rows {
            spawns.push(BandedEntry::new(row.monster.id()).bands(row.floors.0, row.floors.1).weight(row.weight).group(row.group.0, row.group.1));
        }
        Self { defs, spawns }
    }
}

/// One monster as `monsters.ron` writes it, before its attack is read and
/// its traits folded.
///
/// Unknown fields are refused, so a `speed` written on a monster rather
/// than on a trait fails the load naming it, instead of being a number
/// nothing reads.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authored {
    name: String,
    glyph: char,
    color: (f32, f32, f32),
    hp: i32,
    attack: String,
    #[serde(default)]
    ranged: Option<String>,
    #[serde(default)]
    armor: i32,
    faction: NameRef<FactionDef>,
    traits: Vec<NameRef<TraitDef>>,
    #[serde(default)]
    boss: bool,
    #[serde(default)]
    miniboss: bool,
    #[serde(default)]
    drops: Vec<(NameRef<ItemDef>, u32)>,
}

impl Named for Authored {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Reads a monsters file against `registries`, the `traits` a monster is
/// written as, the `items` it drops, and the effect `kinds` and `moments`
/// a trait's own triggers are built against, resolving each monster's core,
/// folding its traits and building what it does at its own moments.
///
/// Every problem in the file at once, each under the monster it is in:
/// an unknown name, an attack that does not read, a trait listed twice,
/// two traits setting one value, and a trigger naming an effect nobody
/// registered. An unknown name is reported before anything else is
/// checked, since nothing else can be read without it.
pub fn read(
    text: &str,
    registries: &Registries,
    traits: &Registry<TraitDef>,
    items: &Registry<ItemDef>,
    kinds: &EffectKinds,
    moments: &Moments,
) -> Result<Registry<MonsterDef>, ContentError> {
    let authored: Registry<Authored> = registries.names().with("trait", traits).with("item", items).load(text)?;
    let names = registries.names();
    let mut defs = Vec::new();
    let mut errors = Vec::new();
    for (id, a) in authored.iter() {
        match resolve(a, Id::from_raw(id.raw()), &names, traits, kinds, moments) {
            Ok(def) => defs.push(def),
            Err(mine) => errors.extend(mine.into_iter().map(|e| format!("{}: {e}", a.name))),
        }
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

/// `a`, the monster that will be `id`, with its attack read, its traits
/// folded and its own triggers built against `kinds` and `moments`, or
/// every reason it cannot be.
fn resolve(
    a: &Authored,
    id: Id<MonsterDef>,
    names: &Names<'_>,
    traits: &Registry<TraitDef>,
    kinds: &EffectKinds,
    moments: &Moments,
) -> Result<MonsterDef, Vec<String>> {
    let mut errors = Vec::new();
    let attack = blow(&a.attack, names).map_err(|e| errors.push(format!("attack {:?}: {e}", a.attack))).ok();
    let ranged = match &a.ranged {
        Some(text) => shot(text, names).map_err(|e| errors.push(format!("ranged {text:?}: {e}"))).ok(),
        None => None,
    };
    let listed: Vec<_> = a.traits.iter().map(|t| t.id()).collect();
    // Each trait once, said at its second listing: a list says nothing
    // twice, so a trait written twice is a typo for another. Folded only
    // without one, since its own values would clash with themselves.
    let twice: Vec<String> = listed
        .iter()
        .enumerate()
        .filter(|(i, t)| listed[..*i].iter().filter(|seen| seen == t).count() == 1)
        .map(|(_, t)| format!("lists `{}` twice", traits.get(*t).name))
        .collect();
    let folded = if twice.is_empty() {
        crate::traits::fold(&listed, traits).map_err(|clash| errors.push(clash.to_string())).ok()
    } else {
        errors.extend(twice);
        None
    };
    let (Some(attack), Some(folded)) = (attack, folded) else { return Err(errors) };
    if !errors.is_empty() {
        return Err(errors);
    }
    let triggers = Triggers::build(&folded.triggers, &[], moments, kinds, names)?;
    let tricks = Tricks::from_listed(&listed, traits);
    // Every trick trait's ability or moment, read off the traits it was
    // listed with rather than off `folded`, which has already merged
    // every trait's abilities and triggers into one list with no trait
    // of its own left to name.
    let trick_abilities: Vec<(AbilityId, String)> =
        listed.iter().map(|&t| traits.get(t)).filter(|t| t.trick).flat_map(|t| t.abilities.iter().map(move |a| (a.id(), t.name.clone()))).collect();
    let trick_moments: Vec<(MomentId, String)> = listed
        .iter()
        .map(|&t| traits.get(t))
        .filter(|t| t.trick)
        .flat_map(|t| t.triggers.iter().filter_map(move |spec| moments.get(&spec.on).map(|on| (on, t.name.clone()))))
        .collect();
    let brain = Arc::new(brain(&folded, ranged.is_some(), tricks));
    let allied = Arc::new(ally_brain(&folded, ranged.is_some()));
    Ok(MonsterDef {
        name: a.name.clone(),
        glyph: a.glyph,
        color: a.color,
        hp: a.hp,
        attack,
        ranged,
        armor: a.armor + folded.armor.unwrap_or(0),
        faction: a.faction.id(),
        boss: a.boss,
        miniboss: a.miniboss,
        drops: DropTable(a.drops.iter().map(|(item, pct)| DropRow::new(item.id(), *pct)).collect()),
        traits: listed.iter().map(|t| traits.get(*t).name.clone()).collect(),
        folded,
        triggers,
        tricks,
        trick_abilities,
        trick_moments,
        id,
        brain,
        allied,
    })
}

/// A blow as `monsters.ron` writes it, `"1d4 slash"`: a roll and a kind,
/// and never a range, which is a shot's.
fn blow(text: &str, names: &Names<'_>) -> Result<MeleeAttack, String> {
    if text.contains('@') {
        return Err("a blow has no range; a shot is written as the monster's `ranged`".into());
    }
    let (dice, kind) = roll_and_kind(text, names)?;
    Ok(MeleeAttack::new(kind, dice))
}

/// A shot as `monsters.ron` writes it, `"1d6 pierce @6"`: a blow's roll
/// and kind, and its range in tiles after an `@`.
fn shot(text: &str, names: &Names<'_>) -> Result<RangedAttack, String> {
    let Some((head, cells)) = text.split_once('@') else { return Err("a shot names its range after an `@`, as \"1d6 pierce @6\"".into()) };
    let (dice, kind) = roll_and_kind(head, names)?;
    let cells = cells.trim();
    let range =
        cells.parse::<i32>().ok().filter(|range| *range > 0).ok_or_else(|| format!("{cells:?} is no range; a range is a number of tiles, one or more"))?;
    Ok(RangedAttack::new(kind, dice, range))
}

/// `"NdS+B kind"`, read as its roll and the kind it names.
fn roll_and_kind(text: &str, names: &Names<'_>) -> Result<(DiceRoll, DamageKindId), String> {
    let mut words = text.split_whitespace();
    let dice = words.next().ok_or("names no roll")?.parse::<DiceRoll>().map_err(|e| e.to_string())?;
    let kind = words.collect::<Vec<_>>().join(" ");
    if kind.is_empty() {
        return Err("names no kind of damage after its roll".into());
    }
    Ok((dice, names.damage_kind(&kind)?))
}

/// How often a monster that knows an ability reaches for it on a turn one
/// is ready, as Delve's beasts do: below a hundred, so a breather does not
/// breathe the very turn its breath comes back, which reads as a machine
/// rather than a monster.
const ABILITY_CHANCE_PCT: u32 = 40;

/// How often a monster with nothing better to do takes a step.
const WANDER_CHANCE_PCT: u32 = 30;

/// How often a monster with something to throw throws it on a turn a
/// throw is there to be made: half the time, so a thrower sometimes
/// closes in instead, and a goblin with one flask does not spend it the
/// moment it sees whom to spend it on.
const THROW_CHANCE_PCT: u32 = 50;

/// The furthest a monster goes for something worth having, in steps:
/// Corsair's four, a short detour, so a goblin that sees a better blade
/// across the room takes it up on its way and one at the far end of the
/// floor is left for the adventurer.
const SCAVENGE_REACH: i32 = 4;

/// The brain a monster of `folded` decides with, built once per kind from
/// what it is, so a monster that never runs has no `FleeWhenHurt` in it
/// rather than one that checks a threshold of nothing every turn.
///
/// Running comes first, before any blow, where Foundry and Delve put it
/// after the blow: a trait's words are what the examine panel promises,
/// and a monster said to run when hurt that fights on while its enemy
/// stands beside it, or shoots on from where it stands, never runs at
/// all. With nowhere to run it fights, since the flight finds no step.
/// An ability comes next, then the blow, then a shot for a kind that has
/// one, which only ever fires at what is not beside it.
///
/// A kind whose wits use things uses what it finds, as `sapient` says it
/// does: it throws what it carries to throw, and fetches gear better than
/// what it wears, a weapon better than its own blow besides, or something
/// to throw while it has nothing, from a few steps away. Both come before
/// the hunt, so a goblin that sees a better
/// blade on its way to a fight takes it up first, and both answer only
/// when there is something to throw or to fetch, so a goblin with empty
/// hands and nothing in sight hunts as any other kind does.
///
/// A kind that keeps its distance, a will-o'-wisp's, comes right after
/// running: backing away or closing to stay within its trait's span
/// outranks a blow, so a thing that lures keeps what it lures at arm's
/// length rather than trading blows with it, and only fights when
/// `Keep::enemies` finds no step to back away by.
///
/// Every kind ends the same way, as Foundry's do: hunt what it sees, search
/// where it last saw it, walk back to the post a piece stood it at, and
/// drift. A monster with no post passes straight over `KeepPost`, so one
/// brain serves a kind whether a piece placed it or a floor's population.
///
/// `tricks` adds four more, none of them a value the trait model's own
/// fold reads: a monkey that has stolen flees for the way out before
/// anything else, outranking even `FleeWhenHurt`, since the job is done
/// and nothing is worth trading a blow for now; a flitting kind's steps
/// are one in three random, in [`crate::tricks::Flit`] right before the
/// hunt; and a packing or a led kind keeps station on its own allies or
/// its one leader there too, above `Wander` and below the blow, exactly
/// where [`Keep`] already sits for a will-o'-wisp's enemies.
fn brain(folded: &Folded, shoots: bool, tricks: Tricks) -> Brain<Entity> {
    let mut brain = Brain::new();
    if tricks.steals {
        brain = brain.then(crate::tricks::FleeAfterStealing);
    }
    if let Some(at_pct) = folded.flee_at.filter(|pct| *pct > 0) {
        brain = brain.then(FleeWhenHurt { at_pct });
    }
    if let Some((keep_within, no_closer_than)) = folded.keeps_away {
        brain = brain.then(Keep::enemies(keep_within, no_closer_than));
    }
    if !folded.abilities.is_empty() {
        brain = brain.then(UseAbility { chance_pct: ABILITY_CHANCE_PCT });
    }
    brain = brain.then(MeleeAdjacent);
    if shoots {
        brain = brain.then(ShootAtRange::default());
    }
    if folded.wits.has(Wits::THROWS) {
        brain = brain.then(ThrowAtRange { chance_pct: THROW_CHANCE_PCT });
    }
    if uses_things(folded.wits) {
        brain = brain.then(Scavenge { reach: SCAVENGE_REACH });
    }
    if tricks.packs {
        brain = brain.then(Keep::allies(PACK_KEEPS_WITHIN, PACK_NO_CLOSER_THAN));
    }
    if tricks.led {
        brain = brain.then(Keep::leader(LED_KEEPS_WITHIN, LED_NO_CLOSER_THAN));
    }
    if tricks.flits {
        brain = brain.then(crate::tricks::Flit { chance_pct: FLIT_CHANCE_PCT });
    }
    brain.then(Hunt).then(SearchLastKnown).then(KeepPost).then(Wander { chance_pct: WANDER_CHANCE_PCT })
}

/// How far a packing kind, a rat's or a jackal's, lets its own kind get
/// before it closes, and how close it lets one get before it backs off:
/// the same span `led` keeps on its one leader, since both are "stay near
/// somebody of your own" and the content sheet gives neither a number of
/// its own.
const PACK_KEEPS_WITHIN: i32 = 3;
/// How close a packing kind lets its own get before backing off: never,
/// since packing only wants nobody too far, not elbow room from its own.
const PACK_NO_CLOSER_THAN: i32 = 1;
/// How far a led kind, the dar and the goblins of a war band, lets its
/// leader get before it closes: the brief's own number.
const LED_KEEPS_WITHIN: i32 = 3;
/// How close a led kind lets its leader get before backing off: the
/// brief's own number.
const LED_NO_CLOSER_THAN: i32 = 1;
/// How often a flitting kind's step is random rather than its usual
/// tactic's: one in three, the trait's own words.
const FLIT_CHANCE_PCT: u32 = 33;

/// How far an ally lets the adventurer get before it follows: three
/// steps, so it fights what comes near without wandering off after
/// whatever it sees across the room, and stays a step or two behind.
const ALLY_KEEPS_WITHIN: i32 = 3;

/// The brain a monster of `folded` decides with once it fights for the
/// adventurer, dominated or conjured: what it runs from, breathes and
/// strikes as its own kind does, and then it keeps station on its allies,
/// whom the faction matrix says are the adventurer's side, before it
/// hunts, so it fights what reaches it and otherwise follows. It neither
/// fetches nor throws, so an ally never picks up what the adventurer meant
/// to, nor goes back to a post a piece stood it at, and it does not
/// wander: with nothing to fight and the adventurer near, it waits there.
fn ally_brain(folded: &Folded, shoots: bool) -> Brain<Entity> {
    let mut brain = Brain::new();
    if let Some(at_pct) = folded.flee_at.filter(|pct| *pct > 0) {
        brain = brain.then(FleeWhenHurt { at_pct });
    }
    if !folded.abilities.is_empty() {
        brain = brain.then(UseAbility { chance_pct: ABILITY_CHANCE_PCT });
    }
    brain = brain.then(MeleeAdjacent);
    if shoots {
        brain = brain.then(ShootAtRange::default());
    }
    brain.then(Keep::allies(ALLY_KEEPS_WITHIN, 1)).then(Hunt)
}

/// Leaves the mind holding the turn only what strikes to throw: a dart, a
/// javelin, an arrow, and not a potion, whose worth to a thrower the mind
/// cannot weigh, so a kobold that threw what it found would heal the
/// adventurer with one draught and hold them with the next. What a monster
/// does with the things it carries is part three's; until then it throws
/// what it hurts with and fetches only that to throw.
///
/// In `DecideSet::Perceive`, after the engine has told the mind what it
/// carries and what lies in sight, in `PerceiveSet::Annotate`, and before
/// it decides.
pub fn throw_what_strikes(mut thinking: ResMut<Thinking>, throwables: Query<&Throwable>) {
    let strikes = |thing: Entity| throwables.get(thing).is_ok_and(|t| t.strike.is_some());
    let Some(snapshot) = thinking.snapshot_mut() else { return };
    snapshot.missiles.retain(|missile| strikes(missile.item));
    for lying in snapshot.items.iter_mut().filter(|lying| !strikes(lying.id)) {
        lying.throw_range = None;
    }
}

/// Whether a mind of `wits` takes things up: puts gear on, or picks up
/// something to throw. Such a kind has hands, a pack and slots, and its
/// brain fetches what it sees worth having.
fn uses_things(wits: Wits) -> bool {
    wits.has(Wits::EQUIPS) || wits.has(Wits::PICKS_UP.with(Wits::THROWS))
}

/// The engine's monsters are made here: one at a piece's slot is made
/// exactly as one a floor's population places.
impl ActorMaker for Bestiary {
    type Def = MonsterDef;

    fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<MonsterDef>, at: Point, map: MapId, _: &mut rand::rngs::StdRng) -> Entity {
        spawn_monster(commands, self.defs.get(def), at, map, registries)
    }

    fn table(&self) -> &BandedTable<Id<MonsterDef>> {
        &self.spawns
    }

    /// A floor's band is its number, since floor `n` is `MapId(n)`; the
    /// town's is past every row the table has, so a role drawn there finds
    /// nobody.
    fn band(&self, map: MapId) -> i32 {
        map.0 as i32
    }
}

/// Spawns the kind `def` on `map` at `at` from inside something that holds
/// the whole world, an effect's closure, made by [`spawn_monster`] as a
/// floor's own are: what a wand of plenty, a staff of conjuration and a
/// scroll of summoning make, and what a polymorphed foe becomes.
pub fn spawn_monster_now(world: &mut World, def: Id<MonsterDef>, at: Point, map: MapId) -> Entity {
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let made = {
        let mut commands = Commands::new(&mut queue, world);
        let bestiary = world.resource::<Bestiary>();
        spawn_monster(&mut commands, bestiary.defs.get(def), at, map, world.resource::<Registries>())
    };
    queue.apply(world);
    made
}

/// Spawns `def` on `map` at `at`, carrying what its core and its traits
/// give it as the engine's own components: health, armor, resistances,
/// its side, its blow, how far it sees and how fast it moves, a mind and
/// the wits it decides with, and how it notices what tries not to be
/// seen, which every monster carries, by [`NOTICE`] unless a trait says.
///
/// A kind whose wits take things up has an empty pack and empty slots,
/// one for every slot the registries hold, so what it finds it can carry
/// and wear, and what its own blow is worth, since the engine strikes with
/// a worn blade in place of its own and a blade worse than its own is no
/// gain.
///
/// Its shot, its dark sight, its hearing, its light, what it knows, the
/// statuses it holds, its own triggers and what it drops are added only
/// for a kind that has them. Statuses a trait holds are put on here,
/// [`FOR_LIFE`], rather than asked for, so they are there from the
/// monster's first turn and never announced: a monster that regenerates
/// did not just start to.
pub fn spawn_monster(commands: &mut Commands, def: &MonsterDef, at: Point, map: MapId, registries: &Registries) -> Entity {
    let f = &def.folded;
    let (r, g, b) = def.color;
    let mut e = commands.spawn((
        (Actor, Blocks, Position(at), OnMap(map), Kind(def.id)),
        (Health::full(def.hp), Armor(def.armor), Resists(f.resists.clone()), Faction(def.faction), def.attack),
        (Perception(def.perception()), Speed(def.speed()), Mind(def.brain.clone()), Intelligence(f.wits), Notice(f.notice.unwrap_or(NOTICE))),
        (Name::new(def.name.clone()), Glyph::new(def.glyph, Color::srgb(r, g, b)).on_layer(5)),
    ));
    if let Some(ranged) = def.ranged {
        e.insert(ranged);
    }
    if uses_things(f.wits) {
        // And what its own blow is worth, which a weapon it would strike
        // with instead has to beat: an ogre keeps its fists.
        let own = OwnBlowScore(crate::armory::blow_score(def.attack.dice, def.attack.cost));
        e.insert((Inventory::default(), Equipped(Equipment::with_slot_count(registries.slots.len())), own));
    }
    if let Some(cells) = f.dark_sight {
        e.insert(DarkSight(cells));
    }
    if let Some(hearing) = f.hearing {
        e.insert(Hearing(hearing));
    }
    if let Some(light) = f.light {
        e.insert(light);
    }
    if !f.abilities.is_empty() {
        e.insert(Grants(f.abilities.clone()));
    }
    // `Shrouds` and `BurnsBadly` hold no status of their own at spawn, but
    // `keep_shrouded` and `keep_regen_halted` still need `Afflicted` and
    // `StatBlock` on the entity to read and to modify, exactly as a
    // trait's own held statuses do.
    if !f.statuses.is_empty() || def.tricks.shrouds || def.tricks.burns_badly {
        let (mut held, mut stats) = (Statuses::new(), Stats::new());
        for status in &f.statuses {
            held.apply(*status, FOR_LIFE, None, &registries.statuses, &mut stats);
        }
        e.insert((Afflicted(held), StatBlock(stats)));
    }
    // What it leaves is the engine's to roll and put down when it dies.
    if !def.drops.is_empty() {
        e.insert(Drops::<ItemDef>(def.drops.clone()));
    }
    // What it does at its own moments, a bite that poisons or a burst
    // where it dies, built once by the bestiary and shared by every copy.
    if !def.triggers.0.is_empty() {
        e.insert(def.triggers.clone());
    }
    // A monster leaves what `necromancer`'s `raising` can find within
    // sight and reach, never the king, whose own death ends the run
    // before anything could stand him back up.
    if !def.boss {
        e.insert(LeavesRemains);
    }
    if def.tricks.needs_marker() {
        if def.tricks.splits {
            e.insert(crate::tricks::Splits);
        }
        if def.tricks.shrouds {
            e.insert(crate::tricks::Shrouds);
        }
        if def.tricks.fire_trail {
            e.insert(crate::tricks::FireTrail);
        }
        if def.tricks.reflects {
            e.insert(Reflects);
        }
        if def.tricks.burns_badly {
            e.insert(crate::tricks::BurnsBadly);
        }
        if def.tricks.drains {
            e.insert(crate::tricks::Drains);
        }
    }
    e.id()
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::ecs::world::CommandQueue;
    use rand::SeedableRng;
    use rl_engine::rl_core::Rect;

    use super::*;
    use crate::content::Library;

    /// The deepest floor, the throne's.
    const DEEPEST: i32 = 10;

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// `text`, a monsters file of a test's own, read against the shipped
    /// registries, traits and items.
    fn fixture(library: &Library, text: &str) -> Result<Registry<MonsterDef>, ContentError> {
        read(text, &library.registries, &library.traits, &library.armory.defs, &crate::content::effect_kinds(), &Moments::default())
    }

    /// Every reason a fixture that should not read was refused.
    fn refused(library: &Library, text: &str) -> Vec<String> {
        match fixture(library, text) {
            Err(ContentError::Invalid(errors)) => errors,
            Err(other) => panic!("refused for the wrong reason: {other}"),
            Ok(_) => panic!("it read"),
        }
    }

    /// Spawns `def` at the origin of floor one in a bare world.
    fn spawn(library: &Library, def: &MonsterDef) -> (World, Entity) {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        let e = spawn_monster(&mut commands, def, Point::ZERO, MapId(1), &library.registries);
        queue.apply(&mut world);
        (world, e)
    }

    /// The name of the tactic `e`'s mind decides with at `hp` of `max`,
    /// an enemy of the adventurer's side standing beside it.
    fn decides(world: &World, e: Entity, hp: i32, max: i32, library: &Library) -> Option<&'static str> {
        let brain = world.get::<Mind>(e).expect("a mind").0.clone();
        let me = ActorView { id: e, pos: Point::ZERO, health: Some(Vitals { current: hp, max }), faction: world.get::<Faction>(e).map(|f| f.0) };
        let foe = ActorView {
            id: Entity::PLACEHOLDER,
            pos: Point::new(1, 0),
            health: Some(Vitals { current: 10, max: 10 }),
            faction: Some(library.registries.factions.expect("adventurer")),
        };
        let mut snapshot = Snapshot::alone(me);
        snapshot.enemies.push(foe);
        snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let ctx = &mut TacticCtx {
            snapshot: &snapshot,
            fields: &mut NoFields,
            can_step: &|_| true,
            blocks_shot: &|_| false,
            blocks_burst: &|_| false,
            bounds: Rect::new(-20, -20, 40, 40),
            rng: &mut rng,
        };
        brain.decide(ctx).1
    }

    /// A tripwire over the bestiary, by name, so a kind is added or
    /// dropped on purpose, and the spectral blade a staff conjures; and
    /// the king is the one death that wins the run.
    #[test]
    fn every_monster_of_the_bestiary_is_in_the_file_and_the_drowned_king_is_its_one_boss() {
        let library = library();
        let bestiary_kinds = [
            "rat",
            "jackal",
            "kobold",
            "goblin",
            "monkey",
            "cave spider",
            "goblin archer",
            "goblin conjurer",
            "goblin mystic",
            "myconid",
            "pink jelly",
            "vampire bat",
            "ogre",
            "skeleton",
            "ghoul",
            "necromancer",
            "wraith",
            "will-o'-wisp",
            "drowned thrall",
            "wight",
            "troll",
            "golem",
            "dar blademaster",
            "dar priestess",
            "dar battlemage",
            "fire imp",
            "salamander",
            "skeleton knight",
            "the Drowned King",
            "spectral blade",
        ];
        let defs = &library.bestiary.defs;
        for name in bestiary_kinds {
            assert!(defs.id(name).is_some(), "no monster called {name:?}");
        }
        assert_eq!(defs.len(), bestiary_kinds.len(), "a monster this list does not name");
        let bosses: Vec<&str> = defs.iter().filter(|(_, d)| d.boss).map(|(_, d)| d.name.as_str()).collect();
        assert_eq!(bosses, ["the Drowned King"]);
        assert!(defs.iter().all(|(id, d)| d.id() == id), "each knows its own id");
    }

    #[test]
    fn a_monster_is_its_core_and_its_traits_folded_together() {
        let library = library();
        let defs = fixture(
            &library,
            r#"[(name: "test hound", glyph: 'h', color: (0.5, 0.4, 0.3), hp: 7, attack: "1d4+1 pierce", armor: 1, faction: "beasts", traits: ["animal", "swift", "thick-hided"])]"#,
        )
        .expect("a hound reads");
        let hound = defs.get(defs.expect("test hound"));
        assert_eq!((hound.glyph, hound.color, hound.hp), ('h', (0.5, 0.4, 0.3), 7), "the core as written");
        assert_eq!(hound.attack.dice, DiceRoll { num: 1, sides: 4, bonus: 1 });
        assert_eq!(hound.attack.kind, library.registries.damage_kinds.expect("pierce"));
        assert_eq!(hound.folded.speed, Some(150), "swift");
        assert_eq!(hound.speed(), 150);
        assert_eq!(hound.folded.wits, Wits::ANIMAL, "animal");
        assert_eq!(hound.armor, 3, "its own and its hide's");
        assert_eq!(hound.traits, ["animal", "swift", "thick-hided"]);
        assert_eq!(hound.perception(), DEFAULT_PERCEPTION, "no trait says how far it sees");
        assert_eq!(hound.flee_at(), 0, "no trait says it runs");
    }

    /// A shot is a blow with a range after an `@`, and both name a kind
    /// the registries hold.
    #[test]
    fn a_shot_reads_its_dice_its_kind_and_its_range() {
        let library = library();
        let archer = library.bestiary.defs.get(library.bestiary.defs.expect("goblin archer"));
        let shot = archer.ranged.expect("the archer shoots");
        assert_eq!((shot.dice, shot.kind, shot.range), (DiceRoll::new(1, 6), library.registries.damage_kinds.expect("pierce"), 6));
        assert_eq!(archer.attack.dice, DiceRoll::new(1, 3), "and still has a blow");
    }

    #[test]
    fn an_attack_that_does_not_read_is_refused_naming_the_monster_and_the_text() {
        let library = library();
        let errors = refused(
            &library,
            r#"[
                (name: "a", glyph: 'a', color: (1.0, 1.0, 1.0), hp: 1, attack: "1q4 slash", faction: "beasts", traits: []),
                (name: "b", glyph: 'b', color: (1.0, 1.0, 1.0), hp: 1, attack: "1d4 slsh", faction: "beasts", traits: []),
                (name: "c", glyph: 'c', color: (1.0, 1.0, 1.0), hp: 1, attack: "1d4", faction: "beasts", traits: []),
                (name: "d", glyph: 'd', color: (1.0, 1.0, 1.0), hp: 1, attack: "1d4 slash", ranged: "1d6 pierce", faction: "beasts", traits: []),
                (name: "e", glyph: 'e', color: (1.0, 1.0, 1.0), hp: 1, attack: "1d4 slash @3", faction: "beasts", traits: []),
                (name: "f", glyph: 'f', color: (1.0, 1.0, 1.0), hp: 1, attack: "1d4 slash", ranged: "1d6 pierce @far", faction: "beasts", traits: []),
            ]"#,
        );
        assert_eq!(errors.len(), 6, "{errors:#?}");
        let expected = [
            ("a", "attack \"1q4 slash\""),
            ("b", "attack \"1d4 slsh\": unknown damage kind \"slsh\""),
            ("c", "attack \"1d4\""),
            ("d", "ranged \"1d6 pierce\""),
            ("e", "attack \"1d4 slash @3\""),
            ("f", "ranged \"1d6 pierce @far\""),
        ];
        for ((monster, says), error) in expected.iter().zip(&errors) {
            assert!(error.starts_with(&format!("{monster}: {says}")), "{error}");
        }
    }

    #[test]
    fn an_unknown_trait_is_refused_by_name() {
        let library = library();
        let errors = refused(
            &library,
            r#"[(name: "jackal", glyph: 'j', color: (1.0, 1.0, 1.0), hp: 6, attack: "1d3 pierce", faction: "beasts", traits: ["animal", "swfit"])]"#,
        );
        assert_eq!(errors, ["jackal: unknown trait \"swfit\""]);
    }

    /// Review Focus 5, where the game meets it: a monster whose two traits
    /// set one value stops the load, naming the monster, the value and
    /// both traits.
    #[test]
    fn a_monster_whose_traits_set_one_value_twice_is_refused_naming_it_the_field_and_both_traits() {
        let library = library();
        let errors = refused(
            &library,
            r#"[(name: "jackal", glyph: 'j', color: (1.0, 1.0, 1.0), hp: 6, attack: "1d3 pierce", faction: "beasts", traits: ["animal", "swift", "slow"])]"#,
        );
        assert_eq!(errors, ["jackal: `speed` is set by both `swift` and `slow`"]);
    }

    #[test]
    fn a_monster_lists_each_trait_once() {
        let library = library();
        let errors = refused(
            &library,
            r#"[(name: "rat", glyph: 'r', color: (1.0, 1.0, 1.0), hp: 4, attack: "1d3 pierce", faction: "beasts", traits: ["animal", "regenerates", "regenerates"])]"#,
        );
        assert_eq!(errors, ["rat: lists `regenerates` twice"]);
    }

    /// Every floor of the ten has something to draw, and the Drowned King
    /// and his knights are drawn on none of them: the throne's piece stands
    /// them on floor ten, at its slots, and a table that could draw them
    /// would put a second king on a floor, or his court out walking it.
    #[test]
    fn every_band_of_the_spawn_table_can_draw_a_monster_and_only_floor_ten_has_the_king() {
        let library = library();
        let bestiary = &library.bestiary;
        assert_eq!(bestiary.spawns.gaps(1..=DEEPEST), Vec::<i32>::new(), "a floor with nothing to draw");
        let king = bestiary.defs.expect("the Drowned King");
        assert!(bestiary.defs.get(king).boss);
        for floor in 1..=DEEPEST {
            assert!(bestiary.spawns.at(floor).all(|row| row.item != king), "the king drawn on floor {floor}");
            assert!(bestiary.spawns.at(floor).all(|row| !bestiary.defs.get(row.item).boss), "a boss drawn on floor {floor}");
        }
        let court = bestiary.defs.expect("skeleton knight");
        assert!((1..=DEEPEST).all(|floor| bestiary.spawns.at(floor).all(|row| row.item != court)), "the court is the throne's alone, drawn on no floor");
    }

    #[test]
    fn every_drop_and_every_item_spawn_names_a_real_item() {
        let library = library();
        let (bestiary, armory) = (&library.bestiary, &library.armory);
        let mut drops = 0;
        for (_, def) in bestiary.defs.iter() {
            for row in &def.drops.0 {
                assert!(armory.defs.try_get(row.item).is_some(), "{} drops an item the armory does not have", def.name);
                drops += 1;
            }
        }
        assert!(drops > 0, "something drops something");
        let gold = armory.defs.expect("gold coin");
        for name in ["kobold", "goblin", "goblin archer", "ogre"] {
            let def = bestiary.defs.get(bestiary.defs.expect(name));
            assert!(def.drops.0.iter().any(|row| row.item == gold), "{name} carries gold");
        }
        assert!(bestiary.defs.get(bestiary.defs.expect("the Drowned King")).drops.is_empty(), "the run is over when he falls");
        for row in armory.table.rows() {
            assert!(armory.defs.try_get(row.item).is_some(), "a spawn row names an item the armory does not have");
        }
        let errors = refused(
            &library,
            r#"[(name: "magpie", glyph: 'm', color: (1.0, 1.0, 1.0), hp: 2, attack: "1d2 pierce", faction: "beasts", traits: [], drops: [("goblet", 50)])]"#,
        );
        assert_eq!(errors, ["magpie: unknown item \"goblet\""]);
    }

    /// The traits reach the engine's own components: a glow is a light
    /// the monster carries, and a coward's threshold is where its mind
    /// turns from fighting to running, with an enemy beside it.
    #[test]
    fn a_spawned_monster_carries_what_its_traits_gave_it() {
        let library = library();
        let defs = fixture(
            &library,
            r#"[(name: "lamp beetle", glyph: 'b', color: (0.4, 0.9, 0.7), hp: 10, attack: "1d3 pierce", faction: "beasts", traits: ["fungal glow", "cowardly"])]"#,
        )
        .expect("a beetle reads");
        let beetle = defs.get(defs.expect("lamp beetle"));
        let (world, e) = spawn(&library, beetle);
        let glow = library.traits.get(library.traits.expect("fungal glow")).light.expect("fungal glow is a light").source();
        assert_eq!(world.get::<LightSource>(e), Some(&glow));
        let threshold = library.traits.get(library.traits.expect("cowardly")).flee_at.expect("cowardly runs at some share");
        let at = threshold * beetle.hp / 100;
        assert_eq!(decides(&world, e, at, beetle.hp, &library), Some("flee_when_hurt"), "at {at} of {}, it runs", beetle.hp);
        assert_eq!(decides(&world, e, at + 1, beetle.hp, &library), Some("melee_adjacent"), "above it, it fights");
    }

    /// The king, as the file writes him: every one of his traits reaches
    /// what the engine reads, and his core's armor is his.
    #[test]
    fn the_drowned_king_spawns_with_everything_his_traits_give_him() {
        let library = library();
        let king = library.bestiary.defs.get(library.bestiary.defs.expect("the Drowned King"));
        let (world, e) = spawn(&library, king);
        let registries = &library.registries;
        assert_eq!(world.get::<Kind>(e), Some(&Kind(king.id())));
        assert_eq!(world.get::<Armor>(e).map(|a| a.0), Some(4));
        assert_eq!(world.get::<Health>(e).map(|h| h.max), Some(120));
        assert!(world.get::<LightSource>(e).is_some(), "grave light");
        assert!(world.get::<Hearing>(e).is_some(), "keen-eared");
        assert_eq!(world.get::<Resists>(e).map(|r| r.get(registries.damage_kinds.expect("poison"))), Some(100), "deathless");
        let grasp = library.abilities.expect("drowning grasp");
        assert_eq!(world.get::<Grants>(e).map(|g| g.0.clone()), Some(vec![grasp]), "drowning grasp");
        let regenerating = registries.statuses.expect("regenerating");
        let held = world.get::<Afflicted>(e).and_then(|a| a.0.iter().find(|s| s.id == regenerating).copied()).expect("he regenerates");
        assert_eq!(held.turns, FOR_LIFE, "for life");
        assert!(world.get::<Intelligence>(e).is_some_and(|w| w.0 == Wits::SAPIENT), "sapient");
        assert_ne!(decides(&world, e, 1, king.hp, &library), Some("flee_when_hurt"), "deathless: he never runs");
    }

    /// Every monster is surer of an adventurer standing in light, so the
    /// lantern is seen coming, except one whose trait says how it
    /// notices: a keen nose notices by smell, in the dark as in the light.
    #[test]
    fn every_monster_notices_a_lit_adventurer_sooner_unless_a_trait_says_how_it_notices() {
        let library = library();
        for (_, def) in library.bestiary.defs.iter() {
            let (world, e) = spawn(&library, def);
            let notice = world.get::<Notice>(e).expect("every monster notices").0;
            match def.folded.notice {
                Some(own) => assert_eq!(notice, own, "{}: its trait's", def.name),
                None => assert!(notice.lit_bonus > 0, "{}: sees a lantern coming", def.name),
            }
        }
        let ghoul = library.bestiary.defs.get(library.bestiary.defs.expect("ghoul"));
        assert_eq!(ghoul.folded.notice.map(|n| n.lit_bonus), Some(0), "keen-nosed");
    }

    /// A kind with the wits to use things has a pack and a slot for
    /// everything the registries hold, so what it finds it can carry and
    /// wear; a beast has neither.
    #[test]
    fn a_kind_that_uses_things_has_hands_and_a_beast_has_none() {
        let library = library();
        for (name, hands) in [("goblin", true), ("kobold", true), ("the Drowned King", true), ("rat", false), ("skeleton", false)] {
            let def = library.bestiary.defs.get(library.bestiary.defs.expect(name));
            let (world, e) = spawn(&library, def);
            assert_eq!(world.get::<Inventory>(e).is_some(), hands, "{name}: a pack");
            let slots = world.get::<Equipped>(e).map(|w| w.slot_count());
            assert_eq!(slots, hands.then_some(library.registries.slots.len()), "{name}: slots");
        }
    }

    /// An ogre's own two fists hit harder than any sword, so an ogre with
    /// a sword lying beside it leaves it there, and the king, whose blow is
    /// the hardest under the lake, would leave any weapon at all; a goblin's
    /// own blow is worth less than the sword it would take up.
    #[test]
    fn an_ogre_leaves_a_sword_lying_that_would_hit_softer_than_its_fists() {
        use crate::testing::{adventurer, arrive, at, clear_floor, clear_monsters, lay, lay_floor, make, monster, pass_turns, sturdy};
        let library = library();
        let worth = |name: &str| {
            let (world, e) = spawn(&library, library.bestiary.defs.get(library.bestiary.defs.expect(name)));
            world.get::<OwnBlowScore>(e).map(|s| s.0)
        };
        let armory = &library.armory;
        let (world, made) = {
            let mut world = World::new();
            let mut queue = CommandQueue::default();
            let made = crate::armory::spawn_item(&mut Commands::new(&mut queue, &world), armory, armory.defs.expect("war hammer"), &library.registries);
            queue.apply(&mut world);
            (world, made)
        };
        let hammer = world.get::<GearScore>(made).expect("a war hammer is worth something").0;
        assert!(worth("the Drowned King").is_some_and(|king| king > hammer), "the king's blow is worth more than the heaviest weapon");
        assert!(worth("goblin").is_some_and(|goblin| goblin < hammer), "a goblin's is worth less");

        for s in 0..3 {
            let mut app = crate::testing::headless(RunSeed(s));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            arrive(&mut app, crate::dungeon::map_of(1));
            clear_monsters(&mut app);
            clear_floor(&mut app);
            let from = at(&app, me);
            let width = app.world().resource::<WorldMap>().place(crate::dungeon::map_of(1)).unwrap().terrain.bounds().width;
            let step = if from.x < width / 2 { 1 } else { -1 };
            let lane: Vec<Point> = (1..=10).map(|d| from.offset(step * d, 0)).collect();
            lay_floor(&mut app, &lane);
            let ogre = monster(&mut app, "ogre", lane[6]);
            let sword = make(&mut app, "sword");
            lay(&mut app, sword, lane[8]);
            pass_turns(&mut app, 8);
            assert_eq!(app.world().get::<Position>(sword).map(|p| p.0), Some(lane[8]), "seed {s}: the sword lies where it lay");
            assert!(app.world().get::<Inventory>(ogre).is_some_and(|bag| bag.items.is_empty()), "seed {s}: and the ogre carries nothing");
        }
    }

    /// A goblin with a dagger in hand and a shield on its arm sees a sword
    /// lying a few steps off and a second dagger nearer: it walks to the
    /// sword, puts it on and keeps the dagger in its pack, and leaves the
    /// dagger that would only replace its own where it lies. The shield is
    /// what makes the second dagger worth nothing: a dagger fits either
    /// hand, and a goblin with a hand free takes up any blade for it.
    #[test]
    fn a_goblin_picks_up_and_wears_a_better_weapon_it_sees() {
        use crate::testing::{adventurer, arrive, at, clear_floor, clear_monsters, lay, lay_floor, make, monster, pass_turns, sturdy, wear};
        for s in 0..3 {
            let mut app = crate::testing::headless(RunSeed(s));
            let me = adventurer(&mut app);
            sturdy(&mut app, me);
            arrive(&mut app, crate::dungeon::map_of(1));
            clear_monsters(&mut app);
            clear_floor(&mut app);
            // A lane of open floor away from the adventurer, toward the
            // side of the floor with more room, lit by the halls' own dim
            // light whatever stood there.
            let from = at(&app, me);
            let width = app.world().resource::<WorldMap>().place(crate::dungeon::map_of(1)).unwrap().terrain.bounds().width;
            let step = if from.x < width / 2 { 1 } else { -1 };
            let lane: Vec<Point> = (1..=10).map(|d| from.offset(step * d, 0)).collect();
            lay_floor(&mut app, &lane);
            let goblin = monster(&mut app, "goblin", lane[5]);
            let (dagger, shield) = (make(&mut app, "dagger"), make(&mut app, "shield"));
            wear(&mut app, goblin, dagger);
            wear(&mut app, goblin, shield);
            let (nearer, sword) = (make(&mut app, "dagger"), make(&mut app, "sword"));
            lay(&mut app, nearer, lane[7]);
            lay(&mut app, sword, lane[8]);
            pass_turns(&mut app, 8);
            let world = app.world();
            let main = world.resource::<Registries>().slots.expect("main hand");
            assert_eq!(world.get::<Equipped>(goblin).and_then(|w| w.in_slot(main)), Some(sword), "seed {s}: it wields the sword");
            assert!(world.get::<Inventory>(goblin).is_some_and(|bag| bag.contains(dagger)), "seed {s}: and keeps its dagger in its pack");
            assert_eq!(world.get::<Position>(nearer).map(|p| p.0), Some(lane[7]), "seed {s}: the dagger no better than its own lies where it lay");
        }
    }

    /// A cave spider's `venomous` trait: its bite lands `hit`, which
    /// poisons whoever it struck, as its row in the content sheet says.
    /// The spider sometimes spits its web instead of biting, so this
    /// gives it turns enough for a bite to land at least once.
    #[test]
    fn a_cave_spiders_bite_poisons_the_adventurer() {
        use crate::testing::{arena, monster, pass_turns};
        let (mut app, me, here) = arena(3);
        monster(&mut app, "cave spider", here.offset(1, 0));
        let poisoned = app.world().resource::<Registries>().statuses.expect("poisoned");
        let mut bitten = false;
        for _ in 0..40 {
            pass_turns(&mut app, 1);
            if app.world().get::<Afflicted>(me).is_some_and(|a| a.0.has(poisoned)) {
                bitten = true;
                break;
            }
        }
        assert!(bitten, "the spider's bite never poisoned the adventurer in forty turns");
    }

    /// A myconid's `spore-burst`: a cloud of confusion where it died,
    /// reaching the adventurer who stood beside it when it fell, read
    /// straight off the gas field rather than off a chance-based status,
    /// so the test does not itself depend on the chance a breath holds.
    #[test]
    fn a_myconid_bursts_into_a_confusing_cloud_exactly_where_it_dies() {
        use crate::testing::{arena, kill, monster};
        let (mut app, _, here) = arena(4);
        let at = here.offset(1, 0);
        let myconid = monster(&mut app, "myconid", at);
        kill(&mut app, myconid, None);
        let confusing = app.world().resource::<Registries>().gases.expect("confusing");
        assert!(app.world().resource::<Gases>().at(confusing, at) > 0, "a confusing cloud stands where the myconid died");
    }

    /// Review Focus 1's game half: a monster the test puts to sleep with
    /// Undermere's own `asleep` is unaware of the adventurer standing
    /// over it, takes their blow at the unaware bonus, and the harm wakes
    /// it, as every sleeper wakes of a blow landed on it.
    #[test]
    fn a_sleeping_goblin_takes_the_unaware_blow_and_then_wakes() {
        use crate::testing::{arena, equip, make, monster, settle};
        let (mut app, me, here) = arena(5);
        let goblin = monster(&mut app, "goblin", here.offset(1, 0));
        let asleep = app.world().resource::<Registries>().statuses.expect(crate::content::ASLEEP);
        app.world_mut().write_message(Afflict { target: goblin, status: asleep, turns: 20, by: None, held_by: None });
        settle(&mut app);
        assert!(app.world().get::<Afflicted>(goblin).is_some_and(|a| a.0.has(asleep)), "asleep before the blow lands");
        let unaware = app.world_mut().run_system_once(move |watchers: Watchers| watchers.unaware(goblin, me)).expect("the watchers");
        assert!(unaware, "a sleeper has not noticed the adventurer standing over it");
        let dagger = make(&mut app, "dagger");
        equip(&mut app, me, dagger);
        app.world_mut().write_message(Intent::new(me, Attack(goblin)));
        settle(&mut app);
        assert!(app.world().get::<Afflicted>(goblin).is_none_or(|a| !a.0.has(asleep)), "the blow woke it");
    }

    /// Withering: a wraith's touch saps strength, a stack a hit, and
    /// repeated touches stack rather than only refresh one instance's
    /// clock, so a fight that runs on leaves whoever it catches weaker
    /// than the first touch did.
    #[test]
    fn a_wraiths_touch_weakens_the_adventurer_more_the_more_it_lands() {
        use crate::testing::{arena, monster, pass_turns};
        let (mut app, me, here) = arena(6);
        monster(&mut app, "wraith", here.offset(1, 0));
        let weakened = app.world().resource::<Registries>().statuses.expect(crate::content::WEAKENED);
        let stacks_of = |app: &App| app.world().get::<Afflicted>(me).map_or(0, |a| a.0.iter().filter(|s| s.id == weakened).count());
        let mut saw_two = false;
        for _ in 0..60 {
            pass_turns(&mut app, 1);
            if stacks_of(&app) >= 2 {
                saw_two = true;
                break;
            }
        }
        assert!(saw_two, "the wraith never touched the adventurer twice in sixty turns");
    }

    /// Luring: a will-o'-wisp keeps its distance from a foe closer than
    /// its trait says, backing away rather than closing to melee the
    /// moment the adventurer stands beside it.
    #[test]
    fn a_will_o_wisp_keeps_its_distance_rather_than_closing_to_melee() {
        let library = library();
        let wisp = library.bestiary.defs.get(library.bestiary.defs.expect("will-o'-wisp"));
        assert_eq!(wisp.folded.keeps_away, Some((5, 3)), "its trait's span");
        let (world, e) = spawn(&library, wisp);
        let brain = world.get::<Mind>(e).expect("a mind").0.clone();
        let me = ActorView { id: e, pos: Point::ZERO, health: Some(Vitals { current: wisp.hp, max: wisp.hp }), faction: world.get::<Faction>(e).map(|f| f.0) };
        let foe = ActorView {
            id: Entity::PLACEHOLDER,
            pos: Point::new(1, 0),
            health: Some(Vitals { current: 10, max: 10 }),
            faction: Some(library.registries.factions.expect("adventurer")),
        };
        let mut snapshot = Snapshot::alone(me);
        snapshot.enemies.push(foe);
        snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let ctx = &mut TacticCtx {
            snapshot: &snapshot,
            fields: &mut NoFields,
            can_step: &|_| true,
            blocks_shot: &|_| false,
            blocks_burst: &|_| false,
            bounds: Rect::new(-20, -20, 40, 40),
            rng: &mut rng,
        };
        assert_eq!(brain.decide(ctx).1, Some("shadow"), "a foe one cell off, inside its no-closer-than, is backed away from rather than struck");
    }

    /// Splitting: a blow that leaves a pink jelly alive splits it into
    /// two, each with half what was left, neither below two.
    #[test]
    fn a_jelly_struck_becomes_two_each_with_its_share() {
        use crate::testing::{arena, monster, settle};
        let (mut app, _, here) = arena(18);
        let jelly = monster(&mut app, "pink jelly", here.offset(1, 0));
        app.world_mut().entity_mut(jelly).insert(Health { current: 10, max: 30 });
        let crush = app.world().resource::<Registries>().damage_kinds.expect("crush");
        app.world_mut().write_message(DamageEvent::new(jelly, rl_engine::rl_rules::Hit::from_source(None, crush, 4)));
        settle(&mut app);
        let jelly_id = app.world().resource::<Bestiary>().defs.expect("pink jelly");
        let world = app.world_mut();
        let halves: Vec<i32> = world.query::<(&Kind, &Health)>().iter(world).filter(|(k, _)| k.0 == jelly_id).map(|(_, h)| h.current).collect();
        assert_eq!(halves.len(), 2, "the jelly and its half");
        assert!(halves.iter().all(|h| *h == 3), "ten taken four leaves six, split evenly: {halves:?}");
    }

    /// Stealing: a landed blow takes a random carried thing, not worn,
    /// and marks the thief to flee with it; killed, it drops what it
    /// took where it fell.
    #[test]
    fn a_monkey_takes_a_thing_and_runs_and_drops_it_when_killed() {
        use crate::testing::{arena, give, kill, make, monster, pass_turns};
        let (mut app, me, here) = arena(21);
        let dagger = make(&mut app, "dagger");
        give(&mut app, me, dagger);
        let monkey = monster(&mut app, "monkey", here.offset(1, 0));
        let mut stole = false;
        for _ in 0..40 {
            pass_turns(&mut app, 1);
            if app.world().get::<Inventory>(monkey).is_some_and(|bag| bag.items.contains(&dagger)) {
                stole = true;
                break;
            }
        }
        assert!(stole, "the monkey never stole the dagger in forty turns");
        assert!(app.world().get::<crate::tricks::Stole>(monkey).is_some(), "marked to flee with it");
        kill(&mut app, monkey, Some(me));
        assert_eq!(app.world().get::<Position>(dagger).map(|p| p.0), app.world().get::<Position>(monkey).map(|p| p.0), "the dagger lies where the monkey fell");
    }

    /// Raising: a necromancer raises a body in its sight and reach as its
    /// own ally, which then fights for it rather than its raiser's kind.
    #[test]
    fn a_necromancer_raises_a_body_in_its_sight_and_it_fights_for_it() {
        use crate::testing::{arena, kill, monster, settle};
        let (mut app, me, here) = arena(25);
        let rat = monster(&mut app, "rat", here.offset(2, 0));
        kill(&mut app, rat, Some(me));
        assert!(app.world().get::<Actor>(rat).is_none(), "the rat is a body now, not an actor");
        let necromancer = monster(&mut app, "necromancer", here.offset(1, 0));
        // A pass so the necromancer's own sight is cast at least once:
        // `raise_dead` is called directly here, outside the turn its
        // `Viewshed` is normally cast within.
        settle(&mut app);
        let raised = crate::effects::bodies::raise_dead(app.world_mut(), necromancer);
        assert!(raised, "the necromancer found the rat's body to raise");
        assert!(app.world().get::<Actor>(rat).is_some(), "it stands again");
        assert_eq!(
            app.world().get::<Faction>(rat).map(|f| f.0),
            app.world().get::<Faction>(necromancer).map(|f| f.0),
            "fighting for its raiser rather than its own kind"
        );
        assert!(app.world().get::<Mind>(rat).is_some(), "a mind to fight with again");
    }

    /// Draining: a landed blow mends its own attacker what it dealt.
    #[test]
    fn a_draining_kinds_landed_blow_mends_it_what_it_dealt() {
        use crate::testing::{arena, monster, pass_turns};
        let (mut app, _, here) = arena(27);
        let bat = monster(&mut app, "vampire bat", here.offset(1, 0));
        app.world_mut().entity_mut(bat).insert(Health { current: 1, max: 9 });
        let mut healed = false;
        for _ in 0..40 {
            pass_turns(&mut app, 1);
            if app.world().get::<Health>(bat).is_some_and(|h| h.current > 1) {
                healed = true;
                break;
            }
        }
        assert!(healed, "the bat never mended itself off a landed blow in forty turns");
    }

    /// Fire-trail: a salamander's own step ignites the cell it left.
    #[test]
    fn a_fire_trail_kind_ignites_the_cell_it_steps_off() {
        use crate::testing::{arena, monster, settle};
        let (mut app, _, here) = arena(29);
        let salamander = monster(&mut app, "salamander", here.offset(1, 0));
        let (from, to) = (here.offset(1, 0), here.offset(1, 1));
        app.world_mut().write_message(Stepped { actor: salamander, from, to });
        settle(&mut app);
        assert!(app.world().resource::<Fire>().is_burning(from), "the cell it stepped off is alight");
    }

    /// Reflecting: a golem carries the engine's own `Reflects` the moment
    /// it is spawned, so an aimed use or an ability that lands on it
    /// turns back on whoever aimed it, one time in two, exactly as
    /// `rl-bevy`'s own test of `Reflects` says.
    #[test]
    fn a_golem_carries_the_engines_own_reflects() {
        let library = library();
        let golem = library.bestiary.defs.get(library.bestiary.defs.expect("golem"));
        let (world, e) = spawn(&library, golem);
        assert!(world.get::<Reflects>(e).is_some(), "turns magic back");
    }

    /// Packing: a rat too far from its own kind, with nothing else in
    /// sight, closes the gap rather than waiting or wandering off alone.
    #[test]
    fn a_packing_kind_closes_on_its_own_kind_when_nothing_else_is_in_sight() {
        let library = library();
        let rat = library.bestiary.defs.get(library.bestiary.defs.expect("rat"));
        let (world, e) = spawn(&library, rat);
        let brain = world.get::<Mind>(e).expect("a mind").0.clone();
        let me = ActorView { id: e, pos: Point::ZERO, health: Some(Vitals { current: rat.hp, max: rat.hp }), faction: world.get::<Faction>(e).map(|f| f.0) };
        let ally = ActorView { id: Entity::PLACEHOLDER, pos: Point::new(5, 0), health: Some(Vitals { current: rat.hp, max: rat.hp }), faction: me.faction };
        let mut snapshot = Snapshot::alone(me);
        snapshot.allies.push(ally);
        snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let ctx = &mut TacticCtx {
            snapshot: &snapshot,
            fields: &mut NoFields,
            can_step: &|_| true,
            blocks_shot: &|_| false,
            blocks_burst: &|_| false,
            bounds: Rect::new(-20, -20, 40, 40),
            rng: &mut rng,
        };
        assert_eq!(brain.decide(ctx).1, Some("follow"), "closes the gap on its own kind");
    }

    /// Led: a dar priestess with its leader sensed closes the gap on it
    /// rather than waiting or wandering off alone.
    #[test]
    fn a_led_kind_closes_on_its_leader_when_nothing_else_is_in_sight() {
        let library = library();
        let priestess = library.bestiary.defs.get(library.bestiary.defs.expect("dar priestess"));
        let (world, e) = spawn(&library, priestess);
        let brain = world.get::<Mind>(e).expect("a mind").0.clone();
        let me = ActorView {
            id: e,
            pos: Point::ZERO,
            health: Some(Vitals { current: priestess.hp, max: priestess.hp }),
            faction: world.get::<Faction>(e).map(|f| f.0),
        };
        let mut snapshot = Snapshot::alone(me);
        snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
        snapshot.add_sense(rl_engine::rl_rules::ai::tactics::Led { at: Point::new(5, 0), in_sight: true });
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let ctx = &mut TacticCtx {
            snapshot: &snapshot,
            fields: &mut NoFields,
            can_step: &|_| true,
            blocks_shot: &|_| false,
            blocks_burst: &|_| false,
            bounds: Rect::new(-20, -20, 40, 40),
            rng: &mut rng,
        };
        assert_eq!(brain.decide(ctx).1, Some("follow"), "closes the gap on its leader");
    }

    /// Review Focus 5: a band whose leader has died keeps together
    /// rather than freezing. The engine's own `Keep::leader` falls back
    /// to keeping station on allies, exactly as `Keep::allies` would, the
    /// moment nothing senses a `Led` to follow, which is what a dead
    /// leader looks like to a follower's snapshot: a dar priestess with
    /// an ally in sight and no leader sensed still closes the gap on its
    /// ally, a real decision, rather than deciding nothing.
    #[test]
    fn a_dar_priestess_whose_leader_is_gone_keeps_station_on_its_allies_rather_than_freezing() {
        let library = library();
        let priestess = library.bestiary.defs.get(library.bestiary.defs.expect("dar priestess"));
        let (world, e) = spawn(&library, priestess);
        let brain = world.get::<Mind>(e).expect("a mind").0.clone();
        let me = ActorView {
            id: e,
            pos: Point::ZERO,
            health: Some(Vitals { current: priestess.hp, max: priestess.hp }),
            faction: world.get::<Faction>(e).map(|f| f.0),
        };
        let ally = ActorView {
            id: Entity::PLACEHOLDER,
            pos: Point::new(5, 0),
            health: Some(Vitals { current: priestess.hp, max: priestess.hp }),
            faction: me.faction,
        };
        let mut snapshot = Snapshot::alone(me);
        snapshot.allies.push(ally);
        snapshot.wits = world.get::<Intelligence>(e).expect("wits").0;
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let ctx = &mut TacticCtx {
            snapshot: &snapshot,
            fields: &mut NoFields,
            can_step: &|_| true,
            blocks_shot: &|_| false,
            blocks_burst: &|_| false,
            bounds: Rect::new(-20, -20, 40, 40),
            rng: &mut rng,
        };
        let decision = brain.decide(ctx);
        assert_eq!(decision.1, Some("follow"), "keeps station on its own ally instead of freezing, a real decision and not a wait");
    }
}
