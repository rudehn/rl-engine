//! What the adventurer knows of their things, and what each is called for
//! it.
//!
//! Potions, scrolls, staffs, wands and rings wear a face each run: a
//! colour, a title of nonsense, a wood, a metal, a stone, drawn from the
//! run's seed out of `assets/lore.ron`, the [`Faces`]. [`Lore`] holds each
//! kind's face and whether the kind is known, and is saved with the run. A
//! thing of an unknown kind is called by its face and drawn in its colour,
//! and carries the engine's [`Unknown`], which keeps what it does and is
//! worth off every panel; its real triggers still work, so an unknown
//! potion drunk does what it does.
//!
//! A kind becomes known when what it did was plain to see: [`learn_by_use`]
//! makes it so when the adventurer uses a thing of it, throws one that does
//! something where it lands, or puts one on, and says what it was. The
//! scroll of identify's [`Identify`] makes one chosen thing known outright.
//!
//! A weapon, armor or ring is known by its kind and not by its level: one
//! that was found is [`Unfamiliar`] and called `+? sword` until the
//! adventurer has worn it for [`FAMILIAR_AFTER`] turns, which
//! [`grow_familiar`] counts, or it is identified. The kit they set out with
//! is known whole.
//!
//! Every name is [`keep_names`]'s to give, from what is known, and it gives
//! them after the pass's narration has read the old ones: so the log says
//! "You use a murky potion." and then "That was a potion of strength.", and
//! never calls a thing by its true name before the line that makes it so.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use rand::Rng;
use rand::seq::SliceRandom;
use rl_engine::prelude::*;
use rl_engine::rl_core::noun::{plural, with_article};
use rl_engine::rl_rules::ability::RawValue;
use serde::{Deserialize, Serialize};

use crate::armory::{Armory, ItemDef, ItemKind};
use crate::run::Adventurer;

/// The faces file, compiled in so the binary runs from anywhere.
const LORE_RON: &str = include_str!("../assets/lore.ron");

/// How many turns the adventurer wears a weapon, armor or ring before
/// they know its level.
pub const FAMILIAR_AFTER: u32 = 200;

/// One face as `lore.ron` writes it: a word, and the colour a thing
/// wearing it is drawn in, `(r, g, b)` in `0..=1`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FaceDef {
    /// The word: `murky`, `ash`, `garnet`.
    pub name: String,
    /// What a thing wearing it is drawn in.
    pub color: (f32, f32, f32),
}

/// Titles made up for a kind, as `lore.ron` writes them: words of
/// syllables, in upper case, `XOMBA RENTOR`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TitlesDef {
    /// What every word is made of.
    pub syllables: Vec<String>,
    /// The fewest and the most words in a title.
    pub words: (u32, u32),
    /// The fewest and the most syllables in a word.
    pub length: (u32, u32),
    /// What every thing with a title is drawn in.
    pub color: (f32, f32, f32),
}

impl TitlesDef {
    /// How many titles could be made, at most `u64::MAX`: what tells a
    /// file that can give every kind a title of its own from one that
    /// cannot.
    fn count(&self) -> u64 {
        let syllables = self.syllables.len() as u64;
        let words: u64 = (self.length.0..=self.length.1).fold(0, |sum, n| sum.saturating_add(syllables.saturating_pow(n)));
        (self.words.0..=self.words.1).fold(0, |sum, n| sum.saturating_add(words.saturating_pow(n)))
    }

    /// One title, drawn from `rng`.
    fn draw(&self, rng: &mut impl Rng) -> String {
        let mut words = Vec::new();
        for _ in 0..rng.random_range(self.words.0..=self.words.1) {
            let mut word = String::new();
            for _ in 0..rng.random_range(self.length.0..=self.length.1) {
                word.push_str(&self.syllables[rng.random_range(0..self.syllables.len())].to_uppercase());
            }
            words.push(word);
        }
        words.join(" ")
    }
}

/// One family of faces as `lore.ron` writes it: the tag its kinds carry,
/// what one is called while its kind is unknown, and the faces it draws
/// from, a list to deal out or titles to make up.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyDef {
    /// What every kind of the family counts as: `potion`, `ring`.
    pub tag: NameRef<TagDef>,
    /// What a thing of an unknown kind is called, `{}` where its face
    /// goes: `{} potion`, `scroll titled {}`.
    pub called: String,
    /// The faces dealt out, one to a kind.
    #[serde(default)]
    pub faces: Vec<FaceDef>,
    /// Or the titles made up, one to a kind.
    #[serde(default)]
    pub titles: Option<TitlesDef>,
}

/// Every family of faces, read once from `assets/lore.ron` and checked
/// against the items that wear them.
#[derive(Resource, Debug, Clone)]
pub struct Faces {
    /// The families, in the file's order, which is the order their faces
    /// are drawn in.
    pub families: Vec<FamilyDef>,
}

impl Faces {
    /// Reads `assets/lore.ron` against `registries`, checked against the
    /// items in `defs`. Panics naming the file and every problem in it.
    pub fn load(registries: &Registries, defs: &Registry<ItemDef>) -> Self {
        Self::read(LORE_RON, registries, defs).unwrap_or_else(|e| panic!("assets/lore.ron: {e}"))
    }

    /// Reads a faces file, every problem in it at once: a family with both
    /// faces and titles or neither, one with fewer faces than it has kinds
    /// to deal them to, a colour outside `0..=1`, titles that cannot be
    /// made, or a kind two families would both give a face.
    pub fn read(text: &str, registries: &Registries, defs: &Registry<ItemDef>) -> Result<Self, ContentError> {
        let families: Vec<FamilyDef> = registries.names().load_list(text)?;
        let mut errors = Vec::new();
        let unit = |c: (f32, f32, f32)| [c.0, c.1, c.2].iter().all(|v| (0.0..=1.0).contains(v));
        for family in &families {
            let tag = registries.tags.name(family.tag.id());
            let kinds = defs.iter().filter(|(_, d)| d.tags.iter().any(|t| t.id() == family.tag.id())).count();
            match &family.titles {
                Some(_) if !family.faces.is_empty() => errors.push(format!("{tag}: faces and titles both")),
                None if family.faces.is_empty() => errors.push(format!("{tag}: neither faces nor titles")),
                None if family.faces.len() < kinds => errors.push(format!("{tag}: {} faces for {kinds} kinds", family.faces.len())),
                Some(t) if t.syllables.is_empty() || t.words.0 == 0 || t.words.0 > t.words.1 || t.length.0 == 0 || t.length.0 > t.length.1 => {
                    errors.push(format!("{tag}: titles of no syllables, or of no words, or of none to a word"))
                }
                Some(t) if t.count() < kinds as u64 => errors.push(format!("{tag}: titles to be made for {} of its {kinds} kinds", t.count())),
                _ => {}
            }
            let mut seen = BTreeSet::new();
            if let Some(twice) = family.faces.iter().find(|f| !seen.insert(f.name.as_str())) {
                errors.push(format!("{tag}: the face {:?} twice", twice.name));
            }
            if !family.called.contains("{}") {
                errors.push(format!("{tag}: called {:?}, with nowhere for the face to go", family.called));
            }
            if !family.faces.iter().all(|f| unit(f.color)) || family.titles.as_ref().is_some_and(|t| !unit(t.color)) {
                errors.push(format!("{tag}: a colour outside 0..=1"));
            }
        }
        for (_, d) in defs.iter() {
            if families.iter().filter(|f| d.tags.iter().any(|t| t.id() == f.tag.id())).count() > 1 {
                errors.push(format!("{}: two families would give it a face", d.name));
            }
        }
        if errors.is_empty() { Ok(Self { families }) } else { Err(ContentError::Invalid(errors)) }
    }
}

/// The face one kind wears: what a thing of it is called while the kind is
/// unknown, and the colour every thing of it is drawn in, known or not, so
/// a potion's colour never gives away what it is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Face {
    /// What a thing of it is called while the kind is unknown: `murky
    /// potion`.
    pub called: String,
    /// What it is drawn in, `(r, g, b)` in `0..=1`.
    pub color: (f32, f32, f32),
}

/// What the adventurer knows of the run's things: the face each kind that
/// wears one wears, and which of those kinds they know.
///
/// By the kind's name rather than its id, so a save names what it knew in
/// the words the content uses, as it names everything else. A kind with
/// no face is known from the start, and so is any kind a save gave no
/// face to.
#[derive(Resource, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Lore {
    /// Each kind that wears a face, by its name.
    faces: BTreeMap<String, Face>,
    /// The kinds wearing a face that the adventurer knows.
    known: BTreeSet<String>,
}

impl Lore {
    /// Deals the faces out for the run of `seed`: each family's to the
    /// kinds that carry its tag, in the order of their names, each family
    /// from a stream of its own, so a kind added to one family never moves
    /// another's faces. Nothing is known yet.
    pub fn draw(faces: &Faces, defs: &Registry<ItemDef>, seed: &Seed) -> Self {
        let mut lore = Self::default();
        for (i, family) in faces.families.iter().enumerate() {
            let mut kinds: Vec<&str> = defs.iter().filter(|(_, d)| d.tags.iter().any(|t| t.id() == family.tag.id())).map(|(_, d)| d.name.as_str()).collect();
            kinds.sort();
            let mut rng = seed.stream(b"lore", i as u64);
            let called = |face: &str| family.called.replace("{}", face);
            match &family.titles {
                Some(titles) => {
                    let mut taken = BTreeSet::new();
                    for kind in kinds {
                        let title = (0..1000).map(|_| titles.draw(&mut rng)).find(|t| taken.insert(t.clone())).expect("a title no other kind has");
                        lore.faces.insert(kind.to_string(), Face { called: called(&title), color: titles.color });
                    }
                }
                None => {
                    let mut dealt: Vec<&FaceDef> = family.faces.iter().collect();
                    dealt.shuffle(&mut rng);
                    for (kind, face) in kinds.into_iter().zip(dealt) {
                        lore.faces.insert(kind.to_string(), Face { called: called(&face.name), color: face.color });
                    }
                }
            }
        }
        lore
    }

    /// The face `kind` wears, if it wears one.
    pub fn face(&self, kind: &str) -> Option<&Face> {
        self.faces.get(kind)
    }

    /// Whether the adventurer knows `kind`: one that wears no face always.
    pub fn knows(&self, kind: &str) -> bool {
        !self.faces.contains_key(kind) || self.known.contains(kind)
    }

    /// Makes `kind` known. Returns whether it was not known before, which
    /// is never for a kind that wears no face.
    pub fn learn(&mut self, kind: &str) -> bool {
        self.faces.contains_key(kind) && self.known.insert(kind.to_string())
    }
}

/// A thing whose level the adventurer does not know yet, and how long
/// they have worn it towards knowing it: a weapon, armor or ring that was
/// found rather than set out with.
///
/// Gone once they know it, by [`FAMILIAR_AFTER`] turns worn or by being
/// identified, and kept in the save with the turns worn so far.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Unfamiliar {
    /// Whole turns the adventurer has worn it.
    pub worn: u32,
    /// The turn it was last counted on while worn, which the next count
    /// counts from; `None` while it is not worn, or not since a save.
    since: Option<u32>,
}

impl Unfamiliar {
    /// One worn `worn` turns already, as a save gives it back.
    pub fn worn_for(worn: u32) -> Self {
        Self { worn, since: None }
    }
}

/// What a thing of `def` is called by its kind alone, as the adventurer
/// knows it: its face while the kind is unknown, its kind's name once it
/// is. What a line in the log says it is when its level is beside the
/// point: "your chain mail glows".
pub fn kind_called(lore: &Lore, def: &ItemDef) -> String {
    match lore.face(&def.name).filter(|_| !lore.knows(&def.name)) {
        Some(face) => face.called.clone(),
        None => def.name.clone(),
    }
}

/// What a thing of `def` at `level` is called, as the adventurer knows
/// it: by [`kind_called`], and with its level before it for a thing whose
/// level is learned by wearing it: `+2 sword`, `-1 ring of stealth`, the
/// kind's name alone at nought, and `+?` for a level not yet known. A kind
/// not yet known says nothing of a level, since a garnet ring could be
/// anything.
pub fn called(armory: &Armory, lore: &Lore, def: Id<ItemDef>, level: i32, level_known: bool) -> String {
    let d = armory.defs.get(def);
    let kind = kind_called(lore, d);
    match (armory.learned_by_wear(def) && lore.knows(&d.name), level_known, level) {
        (false, _, _) | (true, true, 0) => kind,
        (true, true, level) => format!("{level:+} {kind}"),
        (true, false, _) => format!("+? {kind}"),
    }
}

/// Whether a thing of `def` reads as the engine's [`Unknown`], what it does
/// and is worth kept off the panels: while its kind is unknown, and while
/// its level is, for a thing whose numbers would give its level away.
pub fn reads_unknown(armory: &Armory, lore: &Lore, def: Id<ItemDef>, level_known: bool) -> bool {
    !lore.knows(&armory.defs.get(def).name) || (armory.learned_by_wear(def) && !level_known)
}

/// A thing whose name may have changed: one just made, one whose level
/// changed, and one just found to be unfamiliar.
type Renamed = (With<ItemKind>, Or<(Added<ItemKind>, Changed<Enchant>, Added<Unfamiliar>)>);

/// What every thing's name, colour and [`Unknown`] answer to.
type Named<'a> = (Entity, &'a ItemKind, Option<&'a Enchant>, Has<Unfamiliar>, Has<Unknown>, &'a mut Name, Option<&'a mut Glyph>);

/// Gives every thing the name, the colour and the [`Unknown`] that what is
/// known of it says: by [`called`] and [`reads_unknown`], drawn in its
/// kind's face's colour when its kind wears one.
///
/// Whenever something changed what that is: a thing appeared, a level or
/// a kind became known, a level changed. In `TurnSet::Cleanup`, after the
/// narrator has read what the pass's things were called, so a potion drunk
/// is told of under the face it was drunk under and only then renamed, and
/// again between the turns and the drawing, for what appeared outside a
/// pass, a run's kit or a continued run's things. Every thing of a kind
/// at once, wherever it lies: in the pack, on any floor, in a goblin's
/// bag.
pub fn keep_names(
    mut commands: Commands,
    lore: Res<Lore>,
    armory: Res<Armory>,
    changed: Query<(), Renamed>,
    mut familiar: RemovedComponents<Unfamiliar>,
    mut things: Query<Named>,
) {
    let learned = familiar.read().count() > 0;
    if !(lore.is_changed() || learned || !changed.is_empty()) {
        return;
    }
    for (thing, kind, level, unfamiliar, unknown, mut name, glyph) in &mut things {
        let level = level.map_or(0, |l| l.level);
        let now = called(&armory, &lore, kind.0, level, !unfamiliar);
        if name.as_str() != now {
            *name = Name::new(now);
        }
        match (reads_unknown(&armory, &lore, kind.0, !unfamiliar), unknown) {
            (true, false) => {
                commands.entity(thing).try_insert(Unknown);
            }
            (false, true) => {
                commands.entity(thing).try_remove::<Unknown>();
            }
            _ => {}
        }
        if let (Some(face), Some(mut glyph)) = (lore.face(&armory.defs.get(kind.0).name), glyph) {
            let (r, g, b) = face.color;
            let color = Color::srgb(r, g, b);
            if glyph.fg != color {
                glyph.fg = color;
            }
        }
    }
}

/// How a thing's kind showed itself.
#[derive(Clone, Copy)]
enum Shown {
    /// It was used: drunk, read, zapped.
    Used,
    /// It was thrown and did something where it landed.
    Thrown,
    /// It was put on.
    Worn,
}

/// Makes known each kind the adventurer's own hands showed this pass, and
/// says what it was: one they used, one they threw that does something
/// where it lands, and one they put on, since a ring's work is plain the
/// moment it is on. A thing thrown that does nothing where it lands
/// showed nothing, and stays as unknown as it was.
///
/// Only the adventurer's: what a monster does with a thing is not
/// something they learn from. In `TurnSet::React`, where the pass's item
/// events are read; the names follow in the same pass's cleanup, once the
/// narrator has said what was used under the name it was used under.
pub fn learn_by_use(
    mut events: MessageReader<ItemEvent>,
    adventurer: Query<Entity, With<Adventurer>>,
    things: Query<(&ItemKind, Option<&Triggers>)>,
    armory: Res<Armory>,
    mut lore: ResMut<Lore>,
    mut tell: MessageWriter<Tell>,
) {
    let Ok(me) = adventurer.single() else { return };
    for ev in events.read() {
        let (item, shown) = match *ev {
            ItemEvent::Used { actor, item } if actor == me => (item, Shown::Used),
            ItemEvent::Thrown { actor, item, .. } if actor == me => (item, Shown::Thrown),
            ItemEvent::Equipped { actor, item } if actor == me => (item, Shown::Worn),
            _ => continue,
        };
        let Ok((kind, triggers)) = things.get(item) else { continue };
        if matches!(shown, Shown::Thrown) && triggers.is_none_or(|t| t.on(Moments::LAND).next().is_none()) {
            continue;
        }
        let name = &armory.defs.get(kind.0).name;
        if lore.knows(name) || !lore.learn(name) {
            continue;
        }
        let line = match shown {
            Shown::Used | Shown::Thrown => format!("That was {}.", with_article(name)),
            Shown::Worn => format!("It is {}.", with_article(name)),
        };
        tell.write(Tell::new(line, Tones::NOTICE));
    }
}

/// Counts the turns the adventurer wears each [`Unfamiliar`] thing, and
/// once it has been worn [`FAMILIAR_AFTER`] of them makes its level known,
/// with a line saying what it is.
///
/// Counted off the turn the clock reads at each pass, from the turn the
/// thing was last counted on, so a long action counts every turn it took
/// and a save counts on from what it saved. A thing taken off keeps what
/// it has counted, and counts on when it goes back on.
pub fn grow_familiar(
    mut commands: Commands,
    turns: Res<Turns>,
    adventurer: Query<&Equipped, With<Adventurer>>,
    mut things: Query<(Entity, &ItemKind, Option<&Enchant>, &mut Unfamiliar)>,
    armory: Res<Armory>,
    lore: Res<Lore>,
    mut tell: MessageWriter<Tell>,
) {
    let now = turns.turn_number();
    let worn = adventurer.single().ok();
    for (thing, kind, level, mut unfamiliar) in &mut things {
        if !worn.is_some_and(|w| w.contains(thing)) {
            if unfamiliar.since.is_some() {
                unfamiliar.since = None;
            }
            continue;
        }
        let since = unfamiliar.since.unwrap_or(now);
        if since == now && unfamiliar.since.is_some() {
            continue;
        }
        unfamiliar.worn += now.saturating_sub(since);
        unfamiliar.since = Some(now);
        if unfamiliar.worn >= FAMILIAR_AFTER {
            let level = level.map_or(0, |l| l.level);
            let kind = kind_called(&lore, armory.defs.get(kind.0));
            tell.write(Tell::new(format!("You are familiar enough with your {kind} to know it is {level:+}."), Tones::NOTICE));
            commands.entity(thing).try_remove::<Unfamiliar>();
        }
    }
}

/// Make the thing a use was made on known, its kind and its level: the
/// scroll of identify's.
///
/// Reads [`Landing::on`]; a landing on nothing in particular does nothing.
/// What turned out to be what is said in the log in the words it was known
/// by before and after, and a thing already known says so.
#[derive(Debug, Clone, Copy, Default)]
pub struct Identify;

impl Effect for Identify {
    fn apply(&self, landing: &Landing, world: &mut EffectWorld<'_, '_>) {
        if let Some(thing) = landing.on {
            world.commands.queue(move |world: &mut World| identify(world, thing));
        }
    }

    fn describe(&self, _: &Registries, _: EffectBonus) -> String {
        "makes known the thing it is read on".to_string()
    }
}

impl FromArgs for Identify {
    const KIND: &'static str = "Identify";

    fn from_args(_: &RawValue, _: &Names<'_>) -> Result<Self, String> {
        Ok(Self)
    }
}

/// Makes `thing` known, kind and level alike, and tells the adventurer
/// what it turned out to be: a face's kind for every thing of it, and a
/// level for this thing alone.
pub fn identify(world: &mut World, thing: Entity) {
    let Some(kind) = world.get::<ItemKind>(thing).map(|k| k.0) else { return };
    let count = world.get::<Stack>(thing).map_or(1, |s| s.count);
    let level = world.get::<Enchant>(thing).map_or(0, |l| l.level);
    let unfamiliar = world.get::<Unfamiliar>(thing).is_some();
    let (line, tone) = world.resource_scope(|world, mut lore: Mut<Lore>| {
        let armory = world.resource::<Armory>();
        let d = armory.defs.get(kind);
        let before = kind_called(&lore, d);
        let new_kind = lore.learn(&d.name);
        let after = called(armory, &lore, kind, level, true);
        let (one, many) = (count == 1, |name: &str| if count == 1 { name.to_string() } else { plural(name) });
        match (new_kind, unfamiliar && armory.learned_by_wear(kind)) {
            (true, _) if one => (format!("The {before} is {}.", with_article(&after)), Tones::GOOD),
            (true, _) => (format!("The {} are {}.", plural(&before), plural(&after)), Tones::GOOD),
            (false, true) => (format!("Your {before} is {level:+}."), Tones::GOOD),
            (false, false) => (format!("You know your {} already.", many(&called(armory, &lore, kind, level, !unfamiliar))), Tones::MUTED),
        }
    });
    world.entity_mut(thing).remove::<Unfamiliar>();
    world.write_message(Tell::new(line, tone));
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;
    use crate::content::Library;
    use crate::dungeon::map_of;
    use crate::testing::{
        adventurer, arrive, at, clear_monsters, equip, find, give, headless, lay, monster, name_of, pass_turns, read_on, said, take_up, use_thing,
    };

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// The kinds of `family`, by name.
    fn kinds<'a>(library: &'a Library, family: &FamilyDef) -> Vec<&'a str> {
        library.armory.defs.iter().filter(|(_, d)| d.tags.iter().any(|t| t.id() == family.tag.id())).map(|(_, d)| d.name.as_str()).collect()
    }

    /// Whether `word` is one to three of `syllables`, end to end.
    fn spelled_of(word: &str, syllables: &[String], left: u32) -> bool {
        word.is_empty()
            || (left > 0 && syllables.iter().any(|s| word.strip_prefix(&s.to_uppercase()).is_some_and(|rest| spelled_of(rest, syllables, left - 1))))
    }

    /// Over a span of seeds, every kind of a family wears one of its faces
    /// and no two kinds wear one face; a scroll's is a title of two or
    /// three words of one to three of the file's syllables, in capitals;
    /// the same seed deals the same faces, another seed others; and
    /// nothing wearing a face is known before it is used.
    #[test]
    fn faces_are_dealt_from_the_seed_one_to_a_kind_and_the_same_seed_deals_the_same() {
        let library = library();
        let deal = |seed: u64| Lore::draw(&library.faces, &library.armory.defs, &Seed(RunSeed(seed)));
        let first = deal(0);
        let mut another = false;
        for seed in 0..20 {
            let lore = deal(seed);
            assert_eq!(lore, deal(seed), "seed {seed}: one seed, one deal");
            for family in &library.faces.families {
                let kinds = kinds(&library, family);
                assert!(!kinds.is_empty() || family.titles.is_none(), "seed {seed}: a family of no kinds deals nothing");
                let faces: BTreeSet<&str> = kinds.iter().map(|k| lore.face(k).expect("every kind wears a face").called.as_str()).collect();
                assert_eq!(faces.len(), kinds.len(), "seed {seed}: no two kinds of a family wear one face");
                for called in faces {
                    match &family.titles {
                        Some(titles) => {
                            let title = called.strip_prefix("scroll titled ").expect("a title");
                            let words: Vec<&str> = title.split(' ').collect();
                            assert!((2..=3).contains(&words.len()), "seed {seed}: {title:?}");
                            assert!(words.iter().all(|w| spelled_of(w, &titles.syllables, 3)), "seed {seed}: {title:?}");
                        }
                        None => assert!(family.faces.iter().any(|f| family.called.replace("{}", &f.name) == called), "seed {seed}: {called:?}"),
                    }
                }
            }
            another |= lore.face("potion of healing") != first.face("potion of healing");
        }
        assert!(another, "another seed deals the potions otherwise");
        assert!(!first.knows("potion of healing") && !first.knows("scroll of enchanting") && first.knows("sword"), "what wears a face is unknown");
    }

    /// A faces file with fewer faces than kinds or one face twice, a
    /// family with both faces and titles, titles too few to go round, a
    /// colour out of range or a call with nowhere for the face to go is
    /// refused, every one at once.
    #[test]
    fn a_faces_file_that_cannot_dress_every_kind_is_refused_naming_each_problem() {
        let library = library();
        let bad = r#"[
            (tag: "potion", called: "{} potion", faces: [(name: "murky", color: (0.5, 0.5, 0.5)), (name: "murky", color: (0.4, 0.4, 0.4))]),
            (tag: "scroll", called: "scroll titled {}", titles: (syllables: ["ab"], words: (1, 1), length: (1, 2), color: (0.5, 0.5, 0.5))),
            (tag: "ring", called: "a ring", faces: [(name: "jade", color: (2.0, 0.5, 0.5))], titles: (syllables: ["ab"], words: (1, 1), length: (1, 1), color: (0.5, 0.5, 0.5))),
        ]"#;
        let Err(ContentError::Invalid(errors)) = Faces::read(bad, &library.registries, &library.armory.defs) else { panic!("it read") };
        assert_eq!(
            errors,
            [
                "potion: 2 faces for 4 kinds",
                "potion: the face \"murky\" twice",
                "scroll: titles to be made for 2 of its 3 kinds",
                "ring: faces and titles both",
                "ring: called \"a ring\", with nowhere for the face to go",
                "ring: a colour outside 0..=1"
            ]
        );
    }

    /// Every thing of `kind` in the world, wherever it is, by its name and
    /// whether it reads as unknown.
    fn every(app: &mut App, kind: &str) -> Vec<(String, bool)> {
        let id = app.world().resource::<Armory>().defs.expect(kind);
        let world = app.world_mut();
        world.query::<(&ItemKind, &Name, Has<Unknown>)>().iter(world).filter(|(k, ..)| k.0 == id).map(|(_, n, u)| (n.as_str().to_string(), u)).collect()
    }

    /// An unknown potion drunk becomes known, and every potion of its kind
    /// is renamed at once, wherever it is: the rest of the stack in the
    /// pack, one lying on the floor and one in a goblin's bag.
    #[test]
    fn an_unknown_potion_drunk_becomes_known_and_renames_its_kind_everywhere() {
        let mut app = headless(RunSeed(3));
        let me = arrive(&mut app, map_of(1));
        clear_monsters(&mut app);
        let carried = find(&mut app, "potion of mending", 0);
        app.world_mut().get_mut::<Stack>(carried).expect("a stack").count = 2;
        give(&mut app, me, carried);
        let here = at(&app, me);
        let lying = find(&mut app, "potion of mending", 0);
        lay(&mut app, lying, here);
        let goblin = monster(&mut app, "goblin", here.offset(30, 30));
        let held = find(&mut app, "potion of mending", 0);
        give(&mut app, goblin, held);
        app.update();
        let face = name_of(&app, carried);
        assert!(face.ends_with(" potion") && face != "potion of mending", "{face:?}");
        assert_eq!(every(&mut app, "potion of mending"), vec![(face.clone(), true); 3], "every one wears the face, and reads as unknown");
        use_thing(&mut app, me, carried);
        assert_eq!(every(&mut app, "potion of mending"), vec![("potion of mending".to_string(), false); 3], "every one is known, wherever it is");
        let lines = said(&app);
        assert!(lines.iter().any(|l| l == "That was a potion of mending."), "{lines:#?}");
    }

    /// The bag's row for `thing`, by its label, its count and whether it
    /// reads as unknown.
    fn row(app: &App, thing: Entity) -> Option<(String, u32, bool)> {
        app.world().resource::<InventoryView>().rows.iter().find(|r| r.entity == thing).map(|r| (r.label.clone(), r.count, r.unknown))
    }

    /// What the look cursor says of whatever lies at `cell`: its name and
    /// whether it reads as unknown, the cursor opened on it and put away
    /// again.
    fn looked_at(app: &mut App, cell: Point) -> Option<(String, bool)> {
        let modal = rl_engine::rl_ui::view::inspect::inspect_modal(app.world().resource::<Modals>());
        app.world_mut().resource_mut::<Modals>().open(modal);
        app.world_mut().resource_mut::<InspectView>().cursor = cell;
        app.update();
        let view = app.world().resource::<InspectView>();
        let seen = view.subject.as_ref().map(|s| (s.label.clone(), view.unknown));
        app.world_mut().resource_mut::<Modals>().close_one(modal);
        app.update();
        seen
    }

    /// An unknown thing is called by its face in the log and on every
    /// panel, and by its true name only from the line that makes it known
    /// on: lying on the floor, the look cursor calls it by its face; taken
    /// up, it is a pair of murky potions in the log and in the bag; drunk,
    /// the log says one was used under its face and then what it was; and
    /// only then do the bag, the look cursor and the log call it what it
    /// is. A ring put on is named by its stone, and then on the gear rail
    /// by its kind, its level not yet known.
    #[test]
    fn an_unknown_things_name_in_the_log_and_on_the_panels_is_its_face_until_the_line_that_makes_it_known() {
        let mut app = headless(RunSeed(5));
        app.add_plugins((InventoryViewPlugin, GearViewPlugin, InspectViewPlugin));
        let me = arrive(&mut app, map_of(1));
        clear_monsters(&mut app);
        crate::testing::clear_floor(&mut app);
        let (here, east) = (at(&app, me), at(&app, me).offset(1, 0));
        crate::testing::lay_floor(&mut app, &[east]);
        let potion = find(&mut app, "potion of mending", 0);
        app.world_mut().get_mut::<Stack>(potion).expect("a stack").count = 2;
        lay(&mut app, potion, here);
        let lying = find(&mut app, "potion of mending", 0);
        lay(&mut app, lying, east);
        app.update();
        let face = name_of(&app, potion);
        assert_eq!(looked_at(&mut app, east), Some((face.clone(), true)), "the look cursor calls it by its face");
        take_up(&mut app, me);
        assert_eq!(row(&app, potion), Some((face.clone(), 2, true)), "the bag calls them by their face");
        use_thing(&mut app, me, potion);
        assert_eq!(row(&app, potion), Some(("potion of mending".to_string(), 1, false)), "and what they are once one is drunk");
        assert_eq!(looked_at(&mut app, east), Some(("potion of mending".to_string(), false)), "and so does the look cursor");
        let lines = said(&app);
        let at_line = |line: &str| lines.iter().position(|l| l == line).unwrap_or_else(|| panic!("no {line:?} in {lines:#?}"));
        let (taken, drunk, known) = (
            at_line(&format!("You pick up 2 {}.", plural(&face))),
            at_line(&format!("You use {}.", with_article(&face))),
            at_line("That was a potion of mending."),
        );
        assert!(taken < drunk && drunk < known, "{lines:#?}");
        assert!(lines[..known].iter().all(|l| !l.contains("potion of mending")), "never its true name before the line that makes it known: {lines:#?}");

        let ring = find(&mut app, "ring of light", 2);
        give(&mut app, me, ring);
        app.update();
        let stone = name_of(&app, ring);
        equip(&mut app, me, ring);
        let lines = said(&app);
        let (put_on, is) =
            (lines.iter().position(|l| *l == format!("You put on {}.", with_article(&stone))), lines.iter().position(|l| l == "It is a ring of light."));
        assert!(put_on.is_some() && put_on < is, "{lines:#?}");
        let gear = app.world().resource::<GearView>();
        assert_eq!(gear.by_name("left finger").and_then(|s| s.item.as_ref()).map(|r| r.label.clone()), Some("+? ring of light".to_string()));
    }

    /// The kit is known whole, its kinds and its levels: the potions of
    /// healing the adventurer sets out with are potions of healing, and so
    /// is one found later. A potion of a kind they never had is not.
    #[test]
    fn what_the_adventurer_sets_out_with_is_known_and_what_they_find_of_another_kind_is_not() {
        let mut app = headless(RunSeed(8));
        let me = adventurer(&mut app);
        let healing = find(&mut app, "potion of healing", 0);
        let mending = find(&mut app, "potion of mending", 0);
        give(&mut app, me, healing);
        give(&mut app, me, mending);
        app.update();
        let pack = crate::testing::pack(&app, me);
        assert!(pack.iter().any(|(n, _)| n == "sword") && pack.iter().any(|(n, _)| n == "leather armor"), "{pack:?}");
        assert_eq!(name_of(&app, healing), "potion of healing");
        assert_ne!(name_of(&app, mending), "potion of mending");
        assert!(app.world().get::<Unknown>(healing).is_none() && app.world().get::<Unknown>(mending).is_some());
    }

    /// A thing found and worn reads `+?` until the adventurer has worn it
    /// two hundred turns, and then its level is known, with a line saying
    /// what it is.
    #[test]
    fn armor_found_reads_plus_unknown_until_it_has_been_worn_two_hundred_turns() {
        let mut app = headless(RunSeed(9));
        let me = adventurer(&mut app);
        let mail = find(&mut app, "chain mail", 2);
        equip(&mut app, me, mail);
        assert_eq!((name_of(&app, mail), app.world().get::<Unknown>(mail).is_some()), ("+? chain mail".to_string(), true));
        pass_turns(&mut app, FAMILIAR_AFTER - 10);
        assert_eq!(name_of(&app, mail), "+? chain mail", "not yet");
        pass_turns(&mut app, 20);
        assert_eq!((name_of(&app, mail), app.world().get::<Unknown>(mail).is_some()), ("+2 chain mail".to_string(), false));
        let lines = said(&app);
        assert!(lines.iter().any(|l| l == "You are familiar enough with your chain mail to know it is +2."), "{lines:#?}");
    }

    /// A scroll of identify read on a ring nobody knows makes the ring
    /// known, kind and level, and the scroll itself known by the reading.
    #[test]
    fn a_scroll_of_identify_makes_known_the_thing_it_is_read_on_and_itself() {
        let mut app = headless(RunSeed(10));
        let me = adventurer(&mut app);
        let ring = find(&mut app, "ring of stealth", 3);
        let scroll = find(&mut app, "scroll of identify", 0);
        give(&mut app, me, ring);
        give(&mut app, me, scroll);
        app.update();
        let stone = name_of(&app, ring);
        read_on(&mut app, me, scroll, ring);
        assert_eq!((name_of(&app, ring), app.world().get::<Unknown>(ring).is_some()), ("+3 ring of stealth".to_string(), false));
        assert!(app.world().resource::<Lore>().knows("scroll of identify"), "and the scroll is known");
        let lines = said(&app);
        assert!(lines.iter().any(|l| *l == format!("The {stone} is a +3 ring of stealth.")), "{lines:#?}");
        assert!(lines.iter().any(|l| l == "That was a scroll of identify."), "{lines:#?}");
    }
}
