//! What the Sunken Lantern's innkeeper has heard of this run's dungeon:
//! always true, never exact.
//!
//! [`Rumours`] is `assets/rumours.ron`, a [`Rumour`] for each thing a run
//! may draw and the line said once a run's are all told. [`Rumours::told`]
//! is one run's, in the order they are told: the floor ten boss it drew,
//! each mini-boss it drew, shallowest first, each biome it drew on a floor
//! that could have drawn another, shallowest first, each stratum that has a
//! rumour of its own, shallowest first, and then the last line. It reads the draws by the
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

use atglyph::prelude::*;
use atglyph::primitives::RunSeed;
use bevy::prelude::*;
use serde::Deserialize;

use crate::dungeon::{FINAL_BOSSES, MINI_BOSSES, STRATA, biomes_drawn, final_boss, lairs};
use crate::townsfolk::{Line, Townsfolk, placeholders};

const RUMOURS_RON: &str = include_str!("../assets/rumours.ron");

/// One rumour, as `rumours.ron` writes it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rumour {
    /// What it is of, by name: a boss or a mini-boss as `monsters.ron`
    /// names it, a biome as the log names it on arrival, or a stratum as
    /// [`STRATA`] names it.
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
    /// A biome's row is not checked against the biomes, since a biome's is
    /// told only once a run draws it from more than one; a test over a
    /// span of seeds holds every biome a run draws to having one.
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
    /// first, each biome it drew on a floor that could have drawn another,
    /// shallowest first, each stratum with a rumour of its own, shallowest
    /// first, and the last line. Every line always holds, so
    /// talking says each in turn and then the last again. Panics on a biome
    /// drawn with no rumour, which the file's own test keeps from happening.
    pub fn told(&self, seed: RunSeed) -> Vec<Line> {
        let mut lairs = lairs(seed);
        lairs.sort_by_key(|(_, floor)| *floor);
        let biomes = biomes_drawn(seed);
        let drawn = once(final_boss(seed).name).chain(lairs.iter().map(|(boss, _)| MINI_BOSSES[*boss].name)).chain(biomes.iter().map(String::as_str));
        let strata = STRATA.iter().map(|s| s.name).filter(|name| self.of(name).is_some());
        let said = drawn.chain(strata).map(|name| self.of(name).unwrap_or_else(|| panic!("assets/rumours.ron: {name} was drawn, and it has no rumour")));
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
    use super::*;
    use crate::content::Library;
    use crate::dungeon::{TOWN, map_of};
    use crate::testing::{adventurer, arrive, continued, headless, talk_to};
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
    /// ten bosses, the five mini-bosses, the fifteen biomes of floors one to
    /// nine a run may draw from more than one, the Underlake alone on its
    /// floors having none, and the four strata with a hint of their own,
    /// twenty-seven in all, and the design's last line.
    #[test]
    fn the_rumours_file_reads_with_a_rumour_of_every_boss_mini_boss_and_biome_of_the_design() {
        let rumours = library().rumours;
        let biomes = [
            "the Upper Halls",
            "the Cellars",
            "the Watch Barracks",
            // The Drowned Town's first wave.
            "the Old Town",
            "the Harbour",
            "the Sunken Library",
            "the Drowned Cathedral",
            "the Catacombs",
            "the Old Mines",
            "the Fungal Deeps",
            "the Spider Warrens",
            "the Forge of Embers",
            "the Ashen Galleries",
            // The Palace grounds' first wave.
            "the Royal Wood",
            "the Palace Gardens",
        ];
        let strata = ["The Palace and its Grounds", "Beneath the Kingdom", "Before the Kingdom", "The Oldest Deep"];
        let drawable: Vec<&str> = FINAL_BOSSES.iter().map(|b| b.name).chain(MINI_BOSSES.iter().map(|b| b.name)).chain(biomes).chain(strata).collect();
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

    /// Every biome a run may draw on a floor that could have drawn another
    /// has a rumour, read from the files with no seed drawn, so telling a
    /// run's rumours never finds a biome it has nothing to say of. That the
    /// rumours a run tells are of the biomes it draws is the draw's own
    /// test, beside it.
    #[test]
    fn every_biome_a_run_may_draw_from_more_than_one_has_a_rumour() {
        let rumours = library().rumours;
        let told = crate::dungeon::biomes_told_of();
        assert!(!told.is_empty(), "some floor stands more than one biome");
        for biome in told {
            assert!(rumours.of(&biome).is_some(), "{biome} may be drawn, with no rumour");
        }
    }

    /// A run's rumours come in the order they are told: the floor ten boss
    /// it drew, then its lairs shallowest first, then the biomes it drew,
    /// then each stratum with a rumour, then the last line. Read off the
    /// pure draws with no `App`, since [`Rumours::told`] asks only the seed:
    /// that the floors then hold what is told is the generator backstop's
    /// (`the_throne_holds_the_drawn_boss...` in `dungeon.rs`, and the lairs
    /// by `whichever_lair_a_floor_draws_holds_its_boss...` in
    /// `population.rs`), and that the biomes are the ones the floors draw
    /// is `the_biomes_a_rumour_is_told_of_are_the_ones_the_floors_draw...`.
    #[test]
    fn the_rumours_are_told_boss_first_then_each_lair_each_biome_each_stratum_then_the_last_line_over_a_span_of_seeds() {
        let rumours = library().rumours;
        for seed in seeds().into_iter().chain(crate::testing::seeds(0..200).map(RunSeed)) {
            let mut lairs = lairs(seed);
            lairs.sort_by_key(|(_, floor)| *floor);
            let lair_names: Vec<&str> = lairs.iter().map(|(boss, _)| MINI_BOSSES[*boss].name).collect();
            assert!(lairs[0].1 < lairs[1].1, "{seed:?}: two lairs on floors of their own: {lairs:?}");
            let biomes = biomes_drawn(seed);
            let strata: Vec<&str> = STRATA.iter().map(|s| s.name).filter(|n| rumours.of(n).is_some()).collect();
            let said =
                |names: &[&str]| -> Vec<String> { names.iter().map(|n| rumours.of(n).unwrap_or_else(|| panic!("a rumour of {n}")).to_string()).collect() };
            let expected: Vec<String> = said(&[final_boss(seed).name])
                .into_iter()
                .chain(said(&lair_names))
                .chain(said(&biomes.iter().map(String::as_str).collect::<Vec<_>>()))
                .chain(said(&strata))
                .chain([rumours.last().to_string()])
                .collect();
            let told: Vec<String> = rumours.told(seed).into_iter().map(|line| line.say).collect();
            assert_eq!(told, expected, "{seed:?}: boss {}, lairs {lair_names:?}, biomes {biomes:?}", final_boss(seed).name);
        }
    }

    /// Talking to the innkeeper says each rumour in turn and then the last
    /// line again on every talk after: one run, no floor arrived at, since
    /// which line is next is kept in `Folk` and not drawn from the floors.
    #[test]
    fn the_innkeeper_tells_each_rumour_in_turn_and_the_last_line_again_on_every_talk_after() {
        let seed = RunSeed(0);
        let mut app = headless(seed);
        let me = adventurer(&mut app);
        let told = app.world().resource::<Rumours>().told(seed);
        let said: Vec<String> = (0..told.len() + 1).map(|_| words(&talk_to(&mut app, me, "innkeeper")).to_string()).collect();
        let folk = app.world().resource::<Folk>();
        let last = &told.last().expect("the last line is always told").say;
        let expected: Vec<String> = told.iter().map(|line| folk.say(&line.say)).chain([folk.say(last)]).collect();
        assert_eq!(said, expected, "every rumour in turn, then the last line again");
        assert_eq!(said[said.len() - 2], said[said.len() - 1], "the last line twice running");
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
