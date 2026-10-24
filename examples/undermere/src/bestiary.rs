//! The bestiary: what lives under the lake, loaded from `monsters.ron`,
//! with where each kind turns up from `monster_spawns.ron`.
//!
//! A monster is written as a short core, the things every monster has,
//! and a list of traits from `traits.ron`. [`read`] resolves both at load:
//! the core's attack is the engine's own blow part, `(roll: "1d4", kind:
//! "slash")`, every name is looked up once, and the traits are folded into one set of values, so a
//! [`MonsterDef`] is the monster with nothing left to look up and a spawn
//! reads plain numbers. Two traits of one monster that set the same value
//! are refused there, naming the monster, the value and both traits.
//!
//! The folded values then fill the engine's `ActorParts`, laid over
//! [`defaults`], Undermere's own rules for what a kind that says nothing
//! gets, so a spawn hands the engine the parts and adds only what is
//! Undermere's: its marker, its look, its tricks, the pack and slots of a
//! kind that uses things, and sleep.
//!
//! The [`Bestiary`] is Undermere's [`ActorMaker`]: a monster at a piece's
//! slot is made by [`spawn_monster`] exactly as one a floor's population
//! places, so the two can never drift apart.

use std::sync::Arc;

use atglyph::ecs::{DEFAULT_PERCEPTION, Inflict, TriggerKindId};
use atglyph::prelude::all::*;
use atglyph::rules::ability::EffectSpec;
use atglyph::rules::ai::tactics::Keep;
use atglyph::rules::ai::{Repertoire, Standard};
use atglyph::rules::faction::FactionDef;
use atglyph::rules::parts;
use bevy::prelude::*;

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

/// What a kind gets that its core and its traits say nothing of, laid
/// under every kind's parts: it notices by [`NOTICE`], since every monster
/// here watches for the adventurer. Undermere's rule rather than the
/// engine's, which gives a kind that names no notice none at all. A kind
/// whose traits give it no wits is mindless, which the fold already says,
/// since wits are the union of what the traits give.
pub fn defaults() -> ActorParts<ItemDef> {
    ActorParts { notice: Some(NOTICE), ..ActorParts::default() }
}

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
    pub attack: parts::Melee,
    /// Its shot, for a kind that shoots.
    pub ranged: Option<parts::Ranged>,
    /// Flat armor: its own and what a trait gives, together.
    pub armor: i32,
    /// Its side.
    pub faction: FactionId,
    /// Whether its death wins the run.
    pub boss: bool,
    /// Whether it is a mini-boss, worth the score's five hundred when the
    /// adventurer kills it.
    pub miniboss: bool,
    /// Whether it is only ever called up, a staff's or a conjurer's
    /// blade, and leaves no body when it falls: nothing of it was ever
    /// flesh to raise.
    pub conjured: bool,
    /// Its traits' names, in the order the file lists them.
    pub traits: Vec<String>,
    /// Its traits, folded.
    pub folded: Folded,
    /// Everything of it the engine plays, built once from its core and its
    /// folded traits over [`defaults`]: its health, side, sight, wits,
    /// notice and hearing, its attacks and light, what it knows and holds
    /// for life, what it does at its own moments, a bite that poisons what
    /// it struck or a burst where it died, what it leaves when it dies, by
    /// the armory's ids, whether it leaves a body, and the brain every one
    /// of its kind decides with.
    pub parts: ActorParts<ItemDef>,
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
    /// Every trick trait landed through the engine's own actor triggers
    /// that does whatever it does every time its moment comes, by the
    /// moment and the trait's name: a burst of spores where something
    /// died. What [`crate::bestiary_lore::learn_from_triggers`] marks shown
    /// the moment [`Fired::at`] is where the adventurer can see it.
    ///
    /// A moment that fired is not an effect that landed: a touch whose
    /// chance failed fires all the same. So a trigger's effect that may
    /// come to nothing is read off what it does instead, into
    /// [`trick_statuses`](Self::trick_statuses) for a status put on, or
    /// learned by the effect itself where it lands, as a theft is.
    pub trick_moments: Vec<(TriggerKindId, String)>,
    /// Every trick trait whose triggers put a status on what they touch,
    /// by the status and the trait's name: a bite's poison, a touch that
    /// freezes, a touch that saps strength. What
    /// [`crate::bestiary_lore::learn_from_statuses`] marks shown the moment
    /// that status, put on by one of this kind, is seen to take hold.
    pub trick_statuses: Vec<(StatusId, String)>,
    /// Its own id in the bestiary, so a spawn handed only the definition
    /// can mark what it made.
    id: Id<MonsterDef>,
    /// The brain one of its kind decides with once it fights for the
    /// adventurer, built beside the other.
    allied: Arc<Brain<Entity>>,
    /// The brain one of its kind decides with once it is freed from
    /// shackles and follows the one who freed it, built beside the other
    /// two: a freed captive, who keeps station on the adventurer by
    /// name through `Leader` rather than on whichever ally is nearest.
    following: Arc<Brain<Entity>>,
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

    /// The mind one of its kind thinks with once it is freed from
    /// shackles, built once for the kind: it strikes as its kind does,
    /// then keeps station on whoever freed it by `Leader` rather than on
    /// the nearest of the adventurer's side, since a freed captive is one
    /// follower of one adventurer, not a member of a company.
    pub fn follow_mind(&self) -> Mind {
        Mind(self.following.clone())
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
        let blow = self.attack.roll.avg();
        self.ranged.map_or(blow, |shot| blow.max(shot.roll.avg()))
    }

    fn speed_pct(&self) -> u32 {
        self.speed()
    }
}

/// Marks a monster with the definition it was made from: what a save
/// writes down, and what anything that wants the definition back, a tally
/// of the dead or an examine panel, reads it through.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kind(pub Id<MonsterDef>);

/// The same definition as [`Kind`], on every monster, for what stands in
/// for one once it is gone: the engine's remnant of a death, which lands
/// the monster's `died` triggers a pass later, takes this with it by
/// `keep_on_remnant`, so a burst of spores seen where a myconid fell is
/// still known to be a myconid's.
///
/// Apart from [`Kind`] because [`Kind`] is what the save writes a monster
/// by: a remnant carrying it would be written down as a monster of its
/// own, and come back from a continued run standing.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindTag(pub Id<MonsterDef>);

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
    pub fn load(registries: &Registries, traits: &Registry<TraitDef>, items: &Registry<ItemDef>, kinds: &EffectKinds, moments: &TriggerKinds) -> Self {
        let defs = read(MONSTERS_RON, registries, traits, items, kinds, moments).unwrap_or_else(|e| panic!("assets/monsters.ron: {e}"));
        let spawns =
            load_spawns(MONSTER_SPAWNS_RON, &registries.names().with("monster", &defs), &defs).unwrap_or_else(|e| panic!("assets/monster_spawns.ron: {e}"));
        let wrong = spawn_problems(&defs, &spawns);
        assert!(wrong.is_empty(), "assets/monster_spawns.ron: {wrong:#?}");
        Self { defs, spawns }
    }

    /// One kind of `floor`'s own, drawn by weight from the floor's table
    /// as the population draws one, its small life left out: what fills a
    /// part someone is cast in, a vault's sleeper or captive, a patrol, the
    /// run's own captive. A moth or a blind cave rat is never a fight, so
    /// one asleep in a hoard, walking a beat or shackled for rescue would
    /// be a part nobody plays; the table keeps them for the floor's
    /// ordinary people alone. Told by side rather than by name, so a kind
    /// of small life added later is left out with them. `None` for a floor
    /// whose table draws nothing else.
    pub fn pick_cast(&self, floor: u32, registries: &Registries, rng: &mut impl atglyph::rand::Rng) -> Option<Id<MonsterDef>> {
        let wildlife = registries.factions.id(crate::content::WILDLIFE);
        self.spawns.pick_where(floor as i32, |id| Some(self.defs.get(*id).faction) != wildlife, rng).map(|e| e.item)
    }
}

/// Why the spawn table cannot stand as it is, all at once: a floor of the
/// ten with nothing of a weight more than nought to draw, which the
/// population would come to nothing on, or a row drawing a boss or a
/// mini-boss, which would stand a second throne's master out on a floor
/// besides the one its own piece posts, or a lair's keeper off its lair
/// and drawn a second time in a run that drew it once.
pub(crate) fn spawn_problems(defs: &Registry<MonsterDef>, spawns: &BandedTable<Id<MonsterDef>>) -> Vec<String> {
    let deepest = crate::dungeon::FLOORS as i32;
    let mut wrong: Vec<String> = spawns.gaps(1..=deepest).into_iter().map(|f| format!("floor {f} has nothing to draw")).collect();
    for row in spawns.entries().iter().filter(|r| r.weight > 0 && r.min_band <= deepest && r.max_band >= 1) {
        let def = defs.get(row.item);
        let kind = match (def.boss, def.miniboss) {
            (true, _) => "a boss",
            (_, true) => "a mini-boss",
            _ => continue,
        };
        wrong.push(format!("{} is {kind}, and drawn on floors {} to {}", def.name, row.min_band.max(1), row.max_band.min(deepest)));
    }
    wrong
}

/// One monster as `monsters.ron` writes it, before its attack is read and
/// its traits folded.
///
/// Unknown fields are refused, so a `speed` written on a monster rather
/// than on a trait fails the load naming it, instead of being a number
/// nothing reads.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Authored {
    name: String,
    glyph: char,
    color: (f32, f32, f32),
    hp: i32,
    attack: parts::Melee,
    #[serde(default)]
    ranged: Option<parts::Ranged>,
    #[serde(default)]
    armor: i32,
    faction: NameRef<FactionDef>,
    traits: Vec<NameRef<TraitDef>>,
    #[serde(default)]
    boss: bool,
    #[serde(default)]
    miniboss: bool,
    #[serde(default)]
    conjured: bool,
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
    moments: &TriggerKinds,
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
    moments: &TriggerKinds,
) -> Result<MonsterDef, Vec<String>> {
    let mut errors = Vec::new();
    let (attack, ranged) = (a.attack, a.ranged);
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
    let Some(folded) = folded else { return Err(errors) };
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
    let mut trick_moments: Vec<(TriggerKindId, String)> = Vec::new();
    let mut trick_statuses: Vec<(StatusId, String)> = Vec::new();
    for t in listed.iter().map(|&t| traits.get(t)).filter(|t| t.trick) {
        for spec in &t.triggers {
            for effect in spec.effects.iter().flatten() {
                match learned_by(effect, names) {
                    Learned::TriggerKind => trick_moments.extend(moments.get(&spec.on).map(|on| (on, t.name.clone()))),
                    Learned::Status(status) => trick_statuses.push((status, t.name.clone())),
                    Learned::Itself => {}
                }
            }
        }
    }
    trick_moments.dedup();
    trick_statuses.dedup();
    let armor = a.armor + folded.armor.unwrap_or(0);
    // Every value as the core and the traits wrote it, and nothing they
    // did not: what a kind that says nothing gets is `defaults`'s. A boss
    // leaves no body, since his death ends the run before anything could
    // stand him back up, and nor does a conjured blade, which was never
    // anything to leave.
    let parts = ActorParts {
        hp: Some(a.hp),
        armor: Some(armor),
        faction: Some(a.faction.id()),
        speed: folded.speed,
        perception: folded.perception,
        flee_at: folded.flee_at,
        wits: Some(folded.wits),
        notice: folded.notice,
        hearing: folded.hearing,
        dark_sight: folded.dark_sight,
        attacks: Attacks { melee: Some(attack), ranged, ..Attacks::default() },
        keep_away: folded.keep_away,
        light: folded.light.map(|light| light.0),
        abilities: folded.abilities.clone(),
        statuses: folded.statuses.clone(),
        while_holding: folded.while_holding.clone(),
        triggers,
        resists: folded.resists.clone(),
        drops: parts::drops(&a.drops),
        leaves_remains: Some(!a.boss && !a.conjured),
        mind: None,
    }
    .over(&defaults());
    let wrong = parts.check();
    if !wrong.is_empty() {
        return Err(wrong);
    }
    let can = parts.repertoire();
    let parts = ActorParts { mind: Some(Arc::new(brain(can, tricks))), ..parts };
    let allied = Arc::new(ally_brain(can));
    let following = Arc::new(follow_brain(can));
    Ok(MonsterDef {
        name: a.name.clone(),
        glyph: a.glyph,
        color: a.color,
        hp: a.hp,
        attack,
        ranged,
        armor,
        faction: a.faction.id(),
        boss: a.boss,
        miniboss: a.miniboss,
        conjured: a.conjured,
        traits: listed.iter().map(|t| traits.get(*t).name.clone()).collect(),
        folded,
        parts,
        tricks,
        trick_abilities,
        trick_moments,
        trick_statuses,
        id,
        allied,
        following,
    })
}

/// How the adventurer comes to know a trick a trigger's effect lands: the
/// one question [`resolve`] asks of each, so a trick is named only once
/// what it does is seen, never once its moment merely came round.
enum Learned {
    /// Off the moment firing: an effect that lands whenever it fires,
    /// whatever it lands on, a burst of spores.
    TriggerKind,
    /// Off this status taking hold, put on by one of the kind: an effect
    /// that may roll and miss, or be refused once enough is on.
    Status(StatusId),
    /// By the effect itself, where it lands: one that may find nothing to
    /// do, a theft from an empty pack.
    Itself,
}

/// What `effect` is learned by, as [`Learned`] says, read off its kind and,
/// for a status it inflicts, its own arguments.
fn learned_by(effect: &EffectSpec, names: &Names<'_>) -> Learned {
    match effect.kind.as_str() {
        Inflict::KIND => Inflict::from_args(&effect.args, names).map_or(Learned::TriggerKind, |i| Learned::Status(i.status)),
        crate::tricks::Steal::KIND => Learned::Itself,
        _ => Learned::TriggerKind,
    }
}

/// How often a monster that knows an ability reaches for it on a turn one
/// is ready: below a hundred, so a breather does not
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
/// four, a short detour, so a goblin that sees a better blade
/// across the room takes it up on its way and one at the far end of the
/// floor is left for the adventurer.
pub(crate) const SCAVENGE_REACH: i32 = 4;

/// The chain every monster here decides with: the engine's standard
/// chain whole, at Undermere's own numbers.
///
/// Running comes first, before any blow, where Foundry puts it after the
/// blow: a trait's words are what the examine panel promises,
/// and a monster said to run when hurt that fights on while its enemy
/// stands beside it, or shoots on from where it stands, never runs at
/// all. With nowhere to run it fights, since the flight finds no step.
/// A kind that keeps its distance, a will-o'-wisp's, comes right after
/// running: backing away or closing to stay within its trait's span
/// outranks a blow, so a thing that lures keeps what it lures at arm's
/// length rather than trading blows with it, and only fights when
/// `Keep::enemies` finds no step to back away by. An ability comes next,
/// then the blow, then a shot for a kind that has one, which only ever
/// fires at what is not beside it.
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
/// Every kind ends the same way, as Foundry's do: hunt what it sees, search
/// where it last saw it, walk back to the post a piece stood it at, and
/// drift. A monster with no post passes straight over `KeepPost`.
const MONSTER: Standard = Standard {
    flee: true,
    flee_after_attacks: false,
    keep_away: true,
    ability_chance: Some(ABILITY_CHANCE_PCT),
    melee: true,
    shoot: true,
    throw_chance: Some(THROW_CHANCE_PCT),
    scavenge_reach: Some(SCAVENGE_REACH),
    hunt: true,
    search: true,
    keep_post: true,
    wander_chance: Some(WANDER_CHANCE_PCT),
};

/// The chain one fighting for the adventurer decides with: what it runs
/// from, breathes and strikes as its own kind does, and then the hunt, with
/// station kept before it. It neither keeps a span from its enemies, nor
/// fetches nor throws, so an ally never picks up what the adventurer meant
/// to, nor goes back to a post a piece stood it at, and it does not
/// wander: with nothing to fight and the adventurer near, it waits there.
const ALLY: Standard = Standard { flee: true, ability_chance: Some(ABILITY_CHANCE_PCT), melee: true, shoot: true, hunt: true, ..Standard::NONE };

/// The brain a monster that can do what `can` says decides with, built
/// once per kind from what it is, by [`MONSTER`], so a monster that never
/// runs has no `FleeWhenHurt` in it rather than one that checks a
/// threshold of nothing every turn.
///
/// `tricks` adds four more, none of them a value the trait model's own
/// fold reads, and the route a floor's population may give it: a monkey
/// that has stolen flees for the way out before anything else, outranking
/// even `FleeWhenHurt`, since the job is done and nothing is worth trading
/// a blow for now; a packing or a led kind keeps station on its own allies
/// or its one leader before the hunt, and a flitting kind's steps are one
/// in three random, in [`crate::tricks::Flit`], right there too; and every
/// kind walks its own loop of waypoints, if it was given one, before it
/// drifts, one with no route passing straight over
/// [`crate::tricks::Patrol`], so one brain serves a kind whether a piece
/// placed it, a floor's population put it on a patrol, or neither.
fn brain(can: Repertoire, tricks: Tricks) -> Brain<Entity> {
    // Fetching is laid by hand rather than by the chain, at the very place
    // the chain lays it, first before the hunt, so it can be anchored: a
    // shackled goblin walked off its chain to take up a sword lying three
    // steps away, and the freeing broke off with the captive out of reach.
    let takes_things = can.wits.takes_things();
    let mut brain = Standard { scavenge_reach: None, ..MONSTER }.brain(can);
    if takes_things {
        brain = brain.before_hunt(crate::tricks::Unanchored(atglyph::rules::ai::tactics::Scavenge { reach: SCAVENGE_REACH }));
    }
    if tricks.steals {
        brain = brain.first(crate::tricks::FleeAfterStealing);
    }
    if tricks.packs {
        brain = brain.before_hunt(crate::tricks::Unanchored(Keep::allies(PACK_KEEPS_WITHIN, PACK_NO_CLOSER_THAN)));
    }
    if tricks.led {
        brain = brain.before_hunt(Keep::leader(LED_KEEPS_WITHIN, LED_NO_CLOSER_THAN));
    }
    if tricks.flits {
        brain = brain.before_hunt(crate::tricks::Unanchored(crate::tricks::Flit { chance_pct: FLIT_CHANCE_PCT }));
    }
    brain.before_wander(crate::tricks::Patrol).build()
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

/// The brain a monster that can do what `can` says decides with once it
/// fights for the adventurer, dominated or conjured, by [`ALLY`]: it keeps
/// station on its allies, whom the faction matrix says are the
/// adventurer's side, before it hunts, so it fights what reaches it and
/// otherwise follows.
fn ally_brain(can: Repertoire) -> Brain<Entity> {
    ALLY.brain(can).before_hunt(Keep::allies(ALLY_KEEPS_WITHIN, 1)).build()
}

/// The brain a monster that can do what `can` says decides with once it is
/// freed from shackles: what it runs from, breathes and strikes as its own
/// kind does, exactly as [`ally_brain`], and then keeps station on whoever
/// freed it, by the engine's own `Keep::leader` over the `Leader`
/// [`crate::captives::free`] puts on it, rather than `Keep::allies`'s "whoever of
/// my side is nearest": a freed captive follows the one who freed it
/// specifically, through a crowd of the adventurer's other allies as
/// readily as alone with them.
fn follow_brain(can: Repertoire) -> Brain<Entity> {
    ALLY.brain(can).before_hunt(Keep::leader(ALLY_KEEPS_WITHIN, 1)).build()
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

/// The engine's monsters are made here: one at a piece's slot is made
/// exactly as one a floor's population places.
impl ActorMaker for Bestiary {
    type Def = MonsterDef;

    fn make_at(&self, commands: &mut Commands, registries: &Registries, def: Id<MonsterDef>, at: Point, map: MapId) -> Entity {
        spawn_monster(commands, self.defs.get(def), at, map, registries, false)
    }

    fn id_of(&self, name: &str) -> Option<Id<MonsterDef>> {
        self.defs.id(name)
    }

    fn name_of(&self, def: Id<MonsterDef>) -> &str {
        self.defs.name(def)
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
        spawn_monster(&mut commands, bestiary.defs.get(def), at, map, world.resource::<Registries>(), false)
    };
    queue.apply(world);
    made
}

/// Spawns `def` on `map` at `at`: its marker, its name and its look, and
/// then everything of it the engine plays from its parts, which put on
/// what its core and its traits give it and nothing else, so a kind with
/// no hearing is deaf and one with no light sheds none. Statuses a trait
/// holds are on from its first turn and never announced: a monster that
/// regenerates did not just start to.
///
/// What is Undermere's own goes on beside the parts. A kind whose wits take
/// things up has an empty pack and empty slots, one for every slot the
/// registries hold, so what it finds it can carry and wear, and what its
/// own blow is worth, since the engine strikes with a worn blade in place
/// of its own and a blade worse than its own is no gain. A shrouded kind is
/// concealed, and a kind with a trick of its own carries that trick's
/// marker.
///
/// `asleep` puts the engine's own `asleep` on it beside what it holds,
/// [`FOR_LIFE`] so it holds until woken rather than for a counted span: the
/// share of a floor's monsters that starts a run already dozing. `false`
/// for every caller but a floor's own population, which alone decides who
/// sleeps.
pub fn spawn_monster(commands: &mut Commands, def: &MonsterDef, at: Point, map: MapId, registries: &Registries, asleep: bool) -> Entity {
    let (r, g, b) = def.color;
    let mut e = commands.spawn((
        (Actor, Blocks, Position(at), OnMap(map), Kind(def.id), KindTag(def.id)),
        (Name::new(def.name.clone()), Glyph::new(def.glyph, Color::srgb(r, g, b)).on_layer(5)),
    ));
    def.parts.insert(&mut e, registries);
    if def.folded.wits.takes_things() {
        // And what its own blow is worth, which a weapon it would strike
        // with instead has to beat: an ogre keeps its fists.
        let own = OwnBlowScore(crate::armory::blow_score(def.attack.roll, None));
        e.insert((Inventory::default(), Equipped(Equipment::with_slot_count(registries.slots.len())), own));
    }
    // A sleeper holds `asleep` beside what its kind holds.
    if asleep {
        let mut held = def.parts.statuses.clone();
        if asleep {
            held.push(registries.statuses.expect(crate::content::ASLEEP));
        }
        e.insert(innate_statuses(&held, &registries.statuses));
    }
    // Unseen out of the light, the engine's own rule for what the
    // adventurer sees, decided again every frame: a wraith's `shrouded`.
    if def.tricks.shrouds {
        e.insert(Concealed);
    }
    if def.tricks.needs_marker() {
        if def.tricks.splits {
            e.insert(crate::tricks::Splits);
        }
        if def.tricks.fire_trail {
            e.insert(crate::tricks::FireTrail);
        }
        if def.tricks.reflects {
            e.insert(Reflects::default());
        }
        if def.tricks.drains {
            e.insert(crate::tricks::Drains);
        }
        if def.tricks.sings {
            e.insert(crate::tricks::Choir);
        }
        if def.tricks.spawns {
            e.insert(crate::tricks::Spawns::default());
        }
        if def.tricks.wells {
            e.insert(crate::tricks::Wells::default());
        }
    }
    e.id()
}

#[cfg(test)]
mod tests;
