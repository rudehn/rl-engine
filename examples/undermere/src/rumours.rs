//! What the Sunken Lantern's innkeeper has heard of this run's dungeon:
//! always true, never exact.
//!
//! [`Rumours`] is `assets/rumours.ron`, a [`Rumour`] for each thing a run
//! may draw and the line said once a run's are all told. [`Rumours::told`]
//! is one run's, in the order they are told: the floor ten boss it drew,
//! each mini-boss it drew, shallowest first, each band whose biome it drew
//! from more than one, and then the last line. It reads the draws by the
//! very functions the floors are built by, `dungeon::final_boss`,
//! `dungeon::lairs` and `dungeon::biomes_drawn`, each pure over the run's
//! seed, so a rumour told in
//! Mereholt before a floor is ever built is of what that floor will hold;
//! a draw made any other way would have no rumour at all.
//!
//! The rumours are lines like any other person's: the role whose row in
//! `townsfolk.ron` says `rumours: true` says [`Rumours::told`] in place of
//! lines of its own, through the same talk, so which have been told is
//! kept in [`Folk`](crate::townsfolk::Folk) and saved with it, and the last
//! line, once reached, is said on every talk after.

use std::iter::once;

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::RunSeed;
use serde::Deserialize;

use crate::dungeon::{FINAL_BOSSES, MINI_BOSSES, biomes_drawn, final_boss, lairs};
use crate::townsfolk::{Line, Townsfolk, placeholders};

const RUMOURS_RON: &str = include_str!("../assets/rumours.ron");

/// One rumour, as `rumours.ron` writes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rumour {
    /// What it is of, by name: a boss or a mini-boss as `monsters.ron`
    /// names it, or a biome as the log names its band.
    pub of: String,
    /// The words, with a `{role}` for that role's name this run.
    pub say: String,
}

/// Every rumour the innkeeper may tell, and the line said once a run's are
/// all told.
///
/// Read once with the rest of the content and never changed by a run; a
/// run's own are [`Rumours::told`], picked from these by its seed.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rumours {
    rumours: Vec<Rumour>,
    last: String,
}

impl Rumours {
    /// Reads `assets/rumours.ron` against the roles a `{role}` names.
    /// Panics naming the file and every problem in it.
    pub fn load(townsfolk: &Townsfolk) -> Self {
        Self::read(RUMOURS_RON, townsfolk).unwrap_or_else(|e| panic!("assets/rumours.ron: {e}"))
    }

    /// Reads a rumours file, reporting every problem at once, each under
    /// what it is of: a thing with two rumours, a boss or a mini-boss a run
    /// may draw with none, a floor ten boss's rumour that names a floor ten
    /// boss, which would tell which of them waits before floor ten shows
    /// it, and a `{role}` that is nobody's.
    ///
    /// A biome's row is not checked against the biomes, since a band's is
    /// told only once a run draws it from more than one; a test over a
    /// span of seeds holds every band a run draws to having one.
    pub fn read(text: &str, townsfolk: &Townsfolk) -> Result<Self, ContentError> {
        let rumours: Rumours = ron::from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        let mut errors = Vec::new();
        for (i, r) in rumours.rumours.iter().enumerate() {
            if rumours.rumours[..i].iter().any(|o| o.of == r.of) {
                errors.push(format!("{}: a second rumour of it", r.of));
            }
            if FINAL_BOSSES.iter().any(|b| b.name == r.of)
                && let Some(named) = names_a_boss(&r.say)
            {
                errors.push(format!("{}: names {named}, which floor ten is to show first", r.of));
            }
        }
        let drawable = FINAL_BOSSES.iter().map(|b| b.name).chain(MINI_BOSSES.iter().map(|b| b.name));
        for name in drawable.filter(|n| rumours.of(n).is_none()) {
            errors.push(format!("{name}: a run may draw it, and it has no rumour"));
        }
        for (of, say) in rumours.rumours.iter().map(|r| (r.of.as_str(), r.say.as_str())).chain(once(("last", rumours.last.as_str()))) {
            for named in placeholders(say).filter(|n| townsfolk.role(n).is_none()) {
                errors.push(format!("{of}: says {{{named}}}, which is nobody's role"));
            }
        }
        if !errors.is_empty() {
            return Err(ContentError::Invalid(errors));
        }
        Ok(rumours)
    }

    /// The rumour of `name`, if the file has one.
    pub fn of(&self, name: &str) -> Option<&str> {
        self.rumours.iter().find(|r| r.of == name).map(|r| r.say.as_str())
    }

    /// Every rumour, in the file's order.
    pub fn rumours(&self) -> impl Iterator<Item = &Rumour> {
        self.rumours.iter()
    }

    /// The line said once a run's rumours are all told.
    pub fn last(&self) -> &str {
        &self.last
    }

    /// The run of `seed`'s rumours as lines, in the order they are told:
    /// the floor ten boss it drew, each mini-boss it drew, shallowest
    /// first, each band whose biome it drew from more than one, shallowest
    /// first, and the last line. Every line always holds, so talking says
    /// each in turn and then the last again. Panics on a band drawn with no
    /// rumour, which the file's own test keeps from happening.
    pub fn told(&self, seed: RunSeed) -> Vec<Line> {
        let mut lairs = lairs(seed);
        lairs.sort_by_key(|(_, floor)| *floor);
        let drawn =
            once(final_boss(seed).name).chain(lairs.iter().map(|(boss, _)| MINI_BOSSES[*boss].name)).chain(biomes_drawn(seed).into_iter().map(|b| b.name()));
        let said = drawn.map(|name| self.of(name).unwrap_or_else(|| panic!("assets/rumours.ron: {name} was drawn, and it has no rumour")));
        said.chain(once(self.last())).map(|say| Line { say: say.to_string(), when: None }).collect()
    }
}

/// The floor ten boss `text` names, if any: by the whole name or by the
/// name without its article, in any case, so "the mother below" names her
/// as surely as "the Mother Below".
fn names_a_boss(text: &str) -> Option<&'static str> {
    let text = text.to_lowercase();
    FINAL_BOSSES.iter().map(|b| b.name).find(|name| {
        let name = name.to_lowercase();
        text.contains(name.strip_prefix("the ").unwrap_or(&name))
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::bestiary::{Bestiary, Kind};
    use crate::content::Library;
    use crate::dungeon::{FLOORS, TOWN, map_of};
    use crate::testing::{adventurer, arrive, continued, headless, sturdy, talk_to};
    use crate::townsfolk::Folk;

    fn library() -> Library {
        Library::load(&crate::content::effect_kinds(), &Moments::default())
    }

    /// The words of a log line `Name: "words"`, as the innkeeper says them.
    fn words(line: &str) -> &str {
        let (_, quoted) = line.split_once(": \"").unwrap_or_else(|| panic!("a person's line: {line}"));
        quoted.strip_suffix('"').unwrap_or_else(|| panic!("quoted: {line}"))
    }

    /// A seed of each of the three floor ten bosses, and a few more, so
    /// the span tells every boss's rumour.
    fn seeds() -> Vec<RunSeed> {
        let mut seeds: Vec<RunSeed> =
            FINAL_BOSSES.iter().map(|b| (0..200).map(RunSeed).find(|s| final_boss(*s).name == b.name).expect("some seed draws each boss")).collect();
        seeds.extend((0..3).map(RunSeed));
        seeds.sort_unstable_by_key(|s| s.0);
        seeds.dedup();
        seeds
    }

    /// The shipped file reads, with a rumour of each of the three floor
    /// ten bosses, the five mini-bosses and the eight biomes of the design,
    /// sixteen in all, and the design's last line.
    #[test]
    fn the_rumours_file_reads_with_a_rumour_of_every_boss_mini_boss_and_biome_of_the_design() {
        let rumours = library().rumours;
        let biomes = [
            "the Upper Halls",
            "the Cellars",
            "the Fungal Deeps",
            "the Spider Warrens",
            "the Catacombs",
            "the Sunken Library",
            "the Drowned Cathedral",
            "the Underlake",
        ];
        let drawable: Vec<&str> = FINAL_BOSSES.iter().map(|b| b.name).chain(MINI_BOSSES.iter().map(|b| b.name)).chain(biomes).collect();
        let of: Vec<&str> = rumours.rumours().map(|r| r.of.as_str()).collect();
        assert_eq!(of, drawable, "one rumour each, in the design's order");
        assert_eq!(rumours.last(), "That's all I've heard. Come back when you've been somewhere.");
    }

    /// No floor ten boss's rumour names a floor ten boss, its own or
    /// another's, whole or without its article, in any case, since which
    /// of them waits is kept until floor ten shows it.
    #[test]
    fn no_floor_ten_bosss_rumour_names_any_floor_ten_boss() {
        let rumours = library().rumours;
        for boss in &FINAL_BOSSES {
            let say = rumours.of(boss.name).expect("a rumour of every boss").to_lowercase();
            for named in &FINAL_BOSSES {
                let name = named.name.to_lowercase();
                let bare = name.trim_start_matches("the ");
                assert!(!say.contains(&name) && !say.contains(bare), "{}'s rumour names {}: {say}", boss.name, named.name);
            }
        }
    }

    /// A rumours file with each mistake is refused, each named under what
    /// it is of.
    #[test]
    fn a_rumours_file_with_each_mistake_is_refused_naming_each() {
        let library = library();
        let text = r#"(
            rumours: [
                (of: "the Drowned King", say: "Something down there still thinks it rules."),
                (of: "the Mother Below", say: "{nobody} says the MOTHER BELOW stirs."),
                (of: "the Ember Wyrm", say: "Something breathes fire. Ember Wyrm, they say."),
                (of: "Grubnak the goblin king", say: "A king."),
                (of: "Grubnak the goblin king", say: "A king again."),
                (of: "the Widow", say: "Silk."),
                (of: "the Drowned Abbess", say: "An abbess."),
                (of: "the Forgemaster", say: "A hammer."),
            ],
            last: "That's all, {nobody}.",
        )"#;
        let errors = match Rumours::read(text, &library.townsfolk) {
            Err(ContentError::Invalid(errors)) => errors,
            other => panic!("expected the file refused, got {other:?}"),
        };
        let expected = [
            "the Mother Below: names the Mother Below, which floor ten is to show first",
            "the Mother Below: says {nobody}, which is nobody's role",
            "the Ember Wyrm: names the Ember Wyrm, which floor ten is to show first",
            "Grubnak the goblin king: a second rumour of it",
            "the Choirmaster: a run may draw it, and it has no rumour",
            "last: says {nobody}, which is nobody's role",
        ];
        for e in expected {
            assert!(errors.iter().any(|x| x == e), "{e:?} not in {errors:#?}");
        }
        assert_eq!(errors.len(), expected.len(), "{errors:#?}");
    }

    /// Every band whose biome a run draws from more than one has a rumour,
    /// over a span of seeds, so telling a run's rumours never finds a band
    /// it has nothing to say of. No band is drawn from more than one yet.
    #[test]
    fn every_band_a_run_draws_from_more_than_one_has_a_rumour_over_a_span_of_seeds() {
        let rumours = library().rumours;
        for s in 0..500 {
            for biome in biomes_drawn(RunSeed(s)) {
                assert!(rumours.of(biome.name()).is_some(), "seed {s}: {} drawn, with no rumour", biome.name());
            }
        }
    }

    /// Talking to the innkeeper tells the boss floor ten holds, then each
    /// mini-boss the floors hold, shallowest first, each found by going
    /// down and looking, then the last line, and the last line again on
    /// every talk after, over a span of seeds that draws each boss.
    #[test]
    fn the_innkeeper_tells_what_this_runs_floors_hold_boss_first_then_each_lair_shallowest_first_then_the_last_line_again() {
        for seed in seeds() {
            let mut app = headless(seed);
            let me = adventurer(&mut app);
            let told: Vec<String> = (0..5).map(|_| words(&talk_to(&mut app, me, "innkeeper")).to_string()).collect();
            sturdy(&mut app, me);
            let mut held: BTreeMap<u32, String> = BTreeMap::new();
            for floor in 3..=FLOORS {
                let map = map_of(floor);
                arrive(&mut app, map);
                let world = app.world_mut();
                let kinds: Vec<Kind> = world.query::<(&Kind, &OnMap)>().iter(world).filter(|(_, on)| on.0 == map).map(|(k, _)| *k).collect();
                let bestiary = world.resource::<Bestiary>();
                let drawable = |name: &str| FINAL_BOSSES.iter().any(|b| b.name == name) || MINI_BOSSES.iter().any(|b| b.name == name);
                for name in kinds.iter().map(|k| bestiary.defs.get(k.0).name.clone()).filter(|n| drawable(n)) {
                    assert!(held.insert(floor, name).is_none(), "{seed:?}: one boss a floor at most, floor {floor}");
                }
            }
            assert_eq!(held.len(), 3, "{seed:?}: two lairs and a throne held: {held:?}");
            let (throne, boss) = held.pop_last().expect("three held");
            assert_eq!(throne, FLOORS, "{seed:?}: the boss on floor ten, the lairs above it: {held:?}");
            let rumours = app.world().resource::<Rumours>();
            let folk = app.world().resource::<Folk>();
            let of = |name: &String| folk.say(rumours.of(name).unwrap_or_else(|| panic!("a rumour of {name}")));
            let last = rumours.last().to_string();
            let expected: Vec<String> = once(&boss).chain(held.values()).map(of).chain([last.clone(), last]).collect();
            assert_eq!(told, expected, "{seed:?}: what the floors hold, {boss} below {held:?}");
        }
    }

    /// Which rumours the innkeeper has told survive a save: a run saved in
    /// Mereholt after two and continued tells the third next.
    #[test]
    fn the_rumours_told_survive_a_save_and_the_innkeeper_tells_on() {
        for s in 0..2 {
            let seed = RunSeed(s);
            let mut app = headless(seed);
            let me = adventurer(&mut app);
            let rumours = app.world().resource::<Rumours>().told(seed);
            for line in &rumours[..2] {
                assert_eq!(words(&talk_to(&mut app, me, "innkeeper")), app.world().resource::<Folk>().say(&line.say), "seed {s}");
            }
            arrive(&mut app, map_of(1));
            arrive(&mut app, TOWN);
            let text = app.world().resource::<Saves>().load(crate::save::SLOT).expect("the slot reads").expect("the arrival wrote the run down");
            let mut back = continued(&text);
            let me = adventurer(&mut back);
            let third = words(&talk_to(&mut back, me, "innkeeper")).to_string();
            assert_eq!(third, back.world().resource::<Folk>().say(&rumours[2].say), "seed {s}: tells on where it left off");
        }
    }
}
