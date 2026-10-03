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
use rl_engine::rl_bevy::DEFAULT_PERCEPTION;
use rl_engine::rl_rules::ai::tactics::{FleeWhenHurt, Hunt, KeepPost, MeleeAdjacent, SearchLastKnown, ShootAtRange, UseAbility, Wander};
use rl_engine::rl_rules::damage::DamageKindId;
use rl_engine::rl_rules::faction::FactionDef;
use serde::Deserialize;

use crate::armory::ItemDef;
use crate::traits::{Folded, TraitDef};

/// The bestiary file, compiled in so the binary runs from anywhere.
const MONSTERS_RON: &str = include_str!("../assets/monsters.ron");
/// Where each kind turns up, compiled in beside it.
const MONSTER_SPAWNS_RON: &str = include_str!("../assets/monster_spawns.ron");

/// How long a status a trait holds lasts: a whole life. A status counts
/// its turns down, and this many never run out.
pub const FOR_LIFE: u32 = u32::MAX;

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
    /// What it leaves when it dies, by the armory's ids.
    pub drops: DropTable<Id<ItemDef>>,
    /// Its traits' names, in the order the file lists them.
    pub traits: Vec<String>,
    /// Its traits, folded.
    pub folded: Folded,
    /// Its own id in the bestiary, so a spawn handed only the definition
    /// can mark what it made.
    id: Id<MonsterDef>,
    /// The brain every one of its kind decides with, built once from what
    /// it is.
    brain: Arc<Brain<Entity>>,
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
    /// the `traits` a monster is written as and the `items` it drops.
    /// Panics naming the file and every problem in it, since a broken
    /// bestiary is a game that cannot start.
    pub fn load(registries: &Registries, traits: &Registry<TraitDef>, items: &Registry<ItemDef>) -> Self {
        let defs = read(MONSTERS_RON, registries, traits, items).unwrap_or_else(|e| panic!("assets/monsters.ron: {e}"));
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
    drops: Vec<(NameRef<ItemDef>, u32)>,
}

impl Named for Authored {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Reads a monsters file against `registries`, the `traits` a monster is
/// written as and the `items` it drops, resolving each monster's core and
/// folding its traits.
///
/// Every problem in the file at once, each under the monster it is in:
/// an unknown name, an attack that does not read, a trait listed twice,
/// and two traits setting one value. An unknown name is reported before
/// anything else is checked, since nothing else can be read without it.
pub fn read(text: &str, registries: &Registries, traits: &Registry<TraitDef>, items: &Registry<ItemDef>) -> Result<Registry<MonsterDef>, ContentError> {
    let authored: Registry<Authored> = registries.names().with("trait", traits).with("item", items).load(text)?;
    let names = registries.names();
    let mut defs = Vec::new();
    let mut errors = Vec::new();
    for (id, a) in authored.iter() {
        match resolve(a, Id::from_raw(id.raw()), &names, traits) {
            Ok(def) => defs.push(def),
            Err(mine) => errors.extend(mine.into_iter().map(|e| format!("{}: {e}", a.name))),
        }
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

/// `a`, the monster that will be `id`, with its attack read and its
/// traits folded, or every reason it cannot be.
fn resolve(a: &Authored, id: Id<MonsterDef>, names: &Names<'_>, traits: &Registry<TraitDef>) -> Result<MonsterDef, Vec<String>> {
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
    let brain = Arc::new(brain(&folded, ranged.is_some()));
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
        drops: DropTable(a.drops.iter().map(|(item, pct)| DropRow::new(item.id(), *pct)).collect()),
        traits: listed.iter().map(|t| traits.get(*t).name.clone()).collect(),
        folded,
        id,
        brain,
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
/// Every kind ends the same way, as Foundry's do: hunt what it sees, search
/// where it last saw it, walk back to the post a piece stood it at, and
/// drift. A monster with no post passes straight over `KeepPost`, so one
/// brain serves a kind whether a piece placed it or a floor's population.
fn brain(folded: &Folded, shoots: bool) -> Brain<Entity> {
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
    brain.then(Hunt).then(SearchLastKnown).then(KeepPost).then(Wander { chance_pct: WANDER_CHANCE_PCT })
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

/// Spawns `def` on `map` at `at`, carrying what its core and its traits
/// give it as the engine's own components: health, armor, resistances,
/// its side, its blow, how far it sees and how fast it moves, a mind and
/// the wits it decides with, and how it notices what tries not to be
/// seen, which every monster carries.
///
/// Its shot, its dark sight, its hearing, its light, what it knows, the
/// statuses it holds and what it drops are added only for a kind that has
/// them. Statuses a trait holds are put on here, [`FOR_LIFE`], rather than
/// asked for, so they are there from the monster's first turn and never
/// announced: a monster that regenerates did not just start to.
pub fn spawn_monster(commands: &mut Commands, def: &MonsterDef, at: Point, map: MapId, registries: &Registries) -> Entity {
    let f = &def.folded;
    let (r, g, b) = def.color;
    let mut e = commands.spawn((
        (Actor, Blocks, Position(at), OnMap(map), Kind(def.id)),
        (Health::full(def.hp), Armor(def.armor), Resists(f.resists.clone()), Faction(def.faction), def.attack),
        (Perception(def.perception()), Speed(def.speed()), Mind(def.brain.clone()), Intelligence(f.wits), Notice(f.notice.unwrap_or_default())),
        (Name::new(def.name.clone()), Glyph::new(def.glyph, Color::srgb(r, g, b)).on_layer(5)),
    ));
    if let Some(ranged) = def.ranged {
        e.insert(ranged);
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
    if !f.statuses.is_empty() {
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
    e.id()
}

#[cfg(test)]
mod tests {
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
        read(text, &library.registries, &library.traits, &library.armory.defs)
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

    /// A tripwire over the skeleton's bestiary, by name, so a kind is
    /// added or dropped on purpose; and the king is the one death that
    /// wins the run.
    #[test]
    fn every_monster_of_the_skeleton_is_in_the_file_and_the_drowned_king_is_its_one_boss() {
        let library = library();
        let skeleton = [
            "rat",
            "jackal",
            "kobold",
            "goblin",
            "cave spider",
            "goblin archer",
            "myconid",
            "ogre",
            "skeleton",
            "ghoul",
            "drowned",
            "wight",
            "fire imp",
            "salamander",
            "skeleton knight",
            "the Drowned King",
        ];
        let defs = &library.bestiary.defs;
        for name in skeleton {
            assert!(defs.id(name).is_some(), "no monster called {name:?}");
        }
        assert_eq!(defs.len(), skeleton.len(), "a monster this list does not name");
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
    /// is drawn on none of them: the throne's piece stands him on floor
    /// ten, its one slot for him, and a table that could draw him would
    /// put a second king on a floor.
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
        assert!(bestiary.spawns.at(DEEPEST).any(|row| row.item == court), "the court keeps the last floor");
        assert!((1..DEEPEST).all(|floor| bestiary.spawns.at(floor).all(|row| row.item != court)), "and only the last");
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
}
