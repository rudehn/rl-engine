//! Names drawn from syllables: a person, a place, a label in capitals.
//!
//! A [`NameBook`] is a file of named [`NameSet`]s, loaded once with
//! [`load`]. A set is pools of syllables, each named by the set, and
//! patterns that say which pools a name is put together from, drawn by
//! weight. A drawn name that reads badly is thrown away and drawn again,
//! up to `ATTEMPTS` times, so a set never loops however strict it is.
//!
//! The lineage is libtcod's `namegen`: named sets in one file, syllable
//! pools, parts with a chance, and rejection with a redraw, by libtcod's
//! own rules (three of a letter running, a pair repeated back to back, a
//! run of three letters seen again later, a forbidden substring, a stray
//! space), and one of its own, a word given twice. What differs, and why, is in `docs/design/names.md`: a pattern
//! is RON a load can check rather than a `$s$35m$e` string it copies
//! through, pools are named by the set rather than fixed as start, middle
//! and end, patterns are drawn once by weight rather than retried by
//! chance, and a set carries its own case and length.
//!
//! Load checks the shape and never the luck: every pool a pattern names
//! exists, holds no empty syllable and is named by some pattern, every
//! weight and chance makes sense, and every pattern can make a name inside
//! the set's length. Whether a set's rejections leave enough names for a
//! run is a property for the game that ships it to test over its seeds,
//! since the engine never makes a generator of its own to sample with.
//!
//! The file, with every option a set may carry:
//!
//! ```text
//! {
//!     "person": (                      // a set, by the name the game asks for
//!         pools: {                     // syllables, by a name the patterns use
//!             "start": ["ka", "lo"],
//!             "end": ["n", "dra"],
//!         },
//!         patterns: [                  // one drawn per name, by weight
//!             (weight: 3, parts: [Pool("start"), Pool("end")]),
//!             (weight: 1, parts: [Pool("start"), Maybe(35, "end"), Text(" the Younger")]),
//!         ],
//!         case: Title,                 // optional: Title (the default), Upper, Lower, AsWritten
//!         length: (3, 20),             // optional: fewest and most characters, (1, 32) if left out
//!         forbid: ["kak"],             // optional: substrings no name may hold, in any case
//!     ),
//! }
//! ```
//!
//! `Pool(p)` is one syllable of `p`, `Maybe(pct, p)` one `pct` times in a
//! hundred, and `Text(t)` is `t` as written, a space or a byname. The
//! case is applied to the syllables and never to the text, so a title-cased
//! set writes `Kalo the Younger` rather than `Kalo The Younger`, and a
//! word's first letter is the first after a space or a hyphen, whichever
//! part it came from.
//!
//! ```
//! use rand::SeedableRng;
//! use rl_rules::namegen;
//!
//! let book = namegen::load(
//!     r#"{ "person": (pools: { "start": ["ka", "lo", "mer"], "end": ["n", "dra", "vik"] },
//!                     patterns: [(weight: 1, parts: [Pool("start"), Pool("end")])]) }"#,
//! )
//! .unwrap();
//! let person = book.set("person").unwrap();
//! let mut rng = rand::rngs::StdRng::seed_from_u64(7);
//! let name = person.draw(&mut rng).unwrap();
//! assert!(person.accepts(&name));
//! assert!(name.starts_with(char::is_uppercase), "a person's name is in title case");
//! let other = person.draw_distinct(&mut rng, |taken| taken == name).unwrap();
//! assert_ne!(other, name, "never one the caller already has");
//! ```

use std::collections::BTreeMap;

use rand::Rng;
use serde::Deserialize;

use crate::content::ContentError;

/// How many names a draw puts together before it gives up on a set whose
/// rejections refuse them all. A draw that gives up returns `None` rather
/// than looping, so the strictest set costs a bounded number of tries.
pub(crate) const ATTEMPTS: u32 = 100;

/// How a set's names are cased once put together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
pub(crate) enum NameCase {
    /// The first letter of every word, after a space or a hyphen, upper,
    /// and the rest lower: a person's name.
    #[default]
    Title,
    /// Every letter upper: a label.
    Upper,
    /// Every letter lower.
    Lower,
    /// As the syllables are written, for a set whose pools carry their own
    /// capitals.
    ///
    /// Text a pattern writes is never cased, under any of these.
    AsWritten,
}

/// One set as the file writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetFile {
    pools: BTreeMap<String, Vec<String>>,
    patterns: Vec<PatternFile>,
    #[serde(default)]
    case: NameCase,
    #[serde(default = "default_length")]
    length: (u32, u32),
    #[serde(default)]
    forbid: Vec<String>,
}

fn default_length() -> (u32, u32) {
    (1, 32)
}

/// One pattern as the file writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatternFile {
    weight: u32,
    parts: Vec<PartFile>,
}

/// One part of a pattern as the file writes it.
#[derive(Deserialize)]
enum PartFile {
    Pool(String),
    Maybe(u32, String),
    Text(String),
}

/// One part of a pattern with its pool resolved to an index.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Pool(usize),
    Maybe(u32, usize),
    Text(String),
}

/// One kind of name: pools of syllables and the weighted patterns that
/// put them together, with the case, the length and the substrings that
/// decide which names are kept. Made by [`load`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameSet {
    pools: Vec<Vec<String>>,
    patterns: Vec<(u32, Vec<Piece>)>,
    total: u32,
    case: NameCase,
    length: (usize, usize),
    forbid: Vec<String>,
}

impl NameSet {
    /// A name, or `None` when `ATTEMPTS` in a row were refused.
    pub fn draw(&self, rng: &mut impl Rng) -> Option<String> {
        self.draw_distinct(rng, |_| false)
    }

    /// A name `taken` does not claim, or `None` when `ATTEMPTS` in a row
    /// were refused or taken: how a run never gives two people one name.
    /// A taken name is drawn again like a refused one, so the draw is
    /// bounded however few names are left.
    pub fn draw_distinct(&self, rng: &mut impl Rng, taken: impl Fn(&str) -> bool) -> Option<String> {
        (0..ATTEMPTS).find_map(|_| Some(self.assemble(rng)).filter(|name| self.accepts(name) && !taken(name)))
    }

    /// Whether `name` passes this set's rejections: inside its length in
    /// characters, counted as cased; no space at either end or two
    /// together; no character three times running; no pair repeated back
    /// to back, `abab`; no run of three characters seen again later,
    /// `karkar`; no word given twice, `Lo Lo`, which the runs miss when the
    /// word is short; and none of its forbidden substrings. Everything but
    /// the length is compared lowercased, so case never hides a repeat.
    pub fn accepts(&self, name: &str) -> bool {
        let (fewest, most) = self.length;
        if !(fewest..=most).contains(&name.chars().count()) {
            return false;
        }
        let lower = name.to_lowercase();
        let chars: Vec<char> = lower.chars().collect();
        if chars.first() == Some(&' ') || chars.last() == Some(&' ') || chars.windows(2).any(|w| w == [' ', ' ']) {
            return false;
        }
        if chars.windows(3).any(|w| w[0] == w[1] && w[1] == w[2]) {
            return false;
        }
        if chars.windows(4).any(|w| w[0] == w[2] && w[1] == w[3]) {
            return false;
        }
        let runs: Vec<&[char]> = chars.windows(3).collect();
        if runs.iter().enumerate().any(|(i, run)| runs[i + 1..].contains(run)) {
            return false;
        }
        let words: Vec<&str> = lower.split(' ').collect();
        if words.iter().enumerate().any(|(i, word)| words[i + 1..].contains(word)) {
            return false;
        }
        !self.forbid.iter().any(|f| lower.contains(f.as_str()))
    }

    /// One name put together and cased, before any rejection.
    fn assemble(&self, rng: &mut impl Rng) -> String {
        let mut roll = rng.random_range(0..self.total);
        let mut chosen = &self.patterns[0].1;
        for (weight, parts) in &self.patterns {
            if roll < *weight {
                chosen = parts;
                break;
            }
            roll -= weight;
        }
        let mut segments: Vec<(&str, bool)> = Vec::new();
        for part in chosen {
            match part {
                Piece::Pool(pool) => segments.push((self.syllable(*pool, rng), true)),
                Piece::Maybe(pct, pool) => {
                    if rng.random_range(0..100) < *pct {
                        segments.push((self.syllable(*pool, rng), true));
                    }
                }
                Piece::Text(text) => segments.push((text, false)),
            }
        }
        cased(&segments, self.case)
    }

    fn syllable(&self, pool: usize, rng: &mut impl Rng) -> &str {
        let pool = &self.pools[pool];
        &pool[rng.random_range(0..pool.len())]
    }
}

/// The segments of a name joined, each one flagged a syllable put in
/// `case` and every other one copied as written, a character at a time so
/// a letter outside ASCII is cased as itself. A word starts after a space
/// or a hyphen in any segment, so a syllable after a pattern's `Text(" ")`
/// starts a word.
fn cased(segments: &[(&str, bool)], case: NameCase) -> String {
    let mut out = String::new();
    let mut word_start = true;
    for (text, syllable) in segments {
        for c in text.chars() {
            match (syllable, case) {
                (false, _) | (true, NameCase::AsWritten) => out.push(c),
                (true, NameCase::Upper) => out.extend(c.to_uppercase()),
                (true, NameCase::Lower) => out.extend(c.to_lowercase()),
                (true, NameCase::Title) if word_start => out.extend(c.to_uppercase()),
                (true, NameCase::Title) => out.extend(c.to_lowercase()),
            }
            word_start = c == ' ' || c == '-';
        }
    }
    out
}

/// Every set of a file, by the name the file gives it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameBook {
    sets: BTreeMap<String, NameSet>,
}

impl NameBook {
    /// The set the file calls `name`, if it has one.
    pub fn set(&self, name: &str) -> Option<&NameSet> {
        self.sets.get(name)
    }

    /// Every set with its name, in name order.
    #[cfg(test)]
    pub(crate) fn sets(&self) -> impl Iterator<Item = (&str, &NameSet)> {
        self.sets.iter().map(|(name, set)| (name.as_str(), set))
    }
}

/// Reads a file of name sets, refusing it with every mistake listed, each
/// naming its set, when one is not a shape a draw can use.
pub fn load(text: &str) -> Result<NameBook, ContentError> {
    let files: BTreeMap<String, SetFile> = ron::from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
    let mut errors = Vec::new();
    let mut sets = BTreeMap::new();
    for (name, file) in files {
        let mut wrong = |what: String| errors.push(format!("set {name:?}: {what}"));
        if let Some(set) = resolve(file, &mut wrong) {
            sets.insert(name, set);
        }
    }
    if errors.is_empty() { Ok(NameBook { sets }) } else { Err(ContentError::Invalid(errors)) }
}

/// One set resolved, or `None` with every mistake told to `wrong`.
fn resolve(file: SetFile, wrong: &mut impl FnMut(String)) -> Option<NameSet> {
    let SetFile { pools, patterns, case, length, forbid } = file;
    let mut errs: Vec<String> = Vec::new();
    let names: Vec<&String> = pools.keys().collect();
    let index = |pool: &str| names.iter().position(|n| n.as_str() == pool);
    for (pool, syllables) in &pools {
        if syllables.is_empty() {
            errs.push(format!("the pool {pool:?} is empty"));
        } else if syllables.iter().any(String::is_empty) {
            errs.push(format!("the pool {pool:?} holds an empty syllable"));
        }
    }
    if patterns.is_empty() {
        errs.push("it has no patterns".to_string());
    }
    if forbid.iter().any(String::is_empty) {
        errs.push("it forbids an empty substring, which every name holds".to_string());
    }
    let (fewest, most) = (length.0 as usize, length.1 as usize);
    let length_ok = fewest >= 1 && fewest <= most;
    if !length_ok {
        errs.push(format!("its length {length:?} is not a fewest of one or more, no more than the most"));
    }
    let mut named = vec![false; names.len()];
    // Each pattern resolved, and whether every pool it names was found, so
    // the length check below can still judge the patterns that were.
    let mut resolved: Vec<(u32, Vec<Piece>, bool)> = Vec::new();
    for (n, pattern) in patterns.into_iter().enumerate() {
        let n = n + 1;
        if pattern.weight == 0 {
            errs.push(format!("pattern {n} has a weight of nought"));
        }
        if pattern.parts.is_empty() {
            errs.push(format!("pattern {n} has no parts"));
        }
        let mut pieces = Vec::new();
        let mut complete = true;
        for part in pattern.parts {
            let (p, maybe) = match part {
                PartFile::Text(text) => {
                    pieces.push(Piece::Text(text));
                    continue;
                }
                PartFile::Pool(p) => (p, None),
                PartFile::Maybe(pct, p) => (p, Some(pct)),
            };
            let Some(i) = index(&p) else {
                errs.push(format!("pattern {n} names the pool {p:?}, which the set does not have"));
                complete = false;
                continue;
            };
            named[i] = true;
            pieces.push(match maybe {
                None => Piece::Pool(i),
                Some(pct) => {
                    if pct == 0 || pct > 100 {
                        errs.push(format!("pattern {n} has a chance of {pct}, where one in a hundred to a hundred is a chance"));
                    }
                    Piece::Maybe(pct, i)
                }
            });
        }
        resolved.push((pattern.weight, pieces, complete));
    }
    for (i, _) in named.iter().enumerate().filter(|(_, named)| !**named) {
        errs.push(format!("no pattern names the pool {:?}", names[i]));
    }
    let pools: Vec<Vec<String>> = pools.into_values().collect();
    // A syllable's fewest and most characters once cased: a few letters
    // change their count with their case, `ß` upper is `SS`, so the bounds
    // take whichever count a letter could come to.
    let span = |s: &String| -> (usize, usize) {
        s.chars().fold((0, 0), |(short, long), c| {
            let (u, l) = (c.to_uppercase().count(), c.to_lowercase().count());
            if case == NameCase::AsWritten { (short + 1, long + 1) } else { (short + u.min(l), long + u.max(l)) }
        })
    };
    let pool_span = |p: usize| {
        let spans: Vec<(usize, usize)> = pools[p].iter().map(span).collect();
        (spans.iter().map(|s| s.0).min().unwrap_or(0), spans.iter().map(|s| s.1).max().unwrap_or(0))
    };
    for (n, (_, pieces, complete)) in resolved.iter().enumerate() {
        if !length_ok || !complete {
            continue;
        }
        let (short, long) = pieces.iter().fold((0, 0), |(short, long), piece| match piece {
            Piece::Pool(p) => (short + pool_span(*p).0, long + pool_span(*p).1),
            Piece::Maybe(_, p) => (short, long + pool_span(*p).1),
            Piece::Text(text) => (short + text.chars().count(), long + text.chars().count()),
        });
        if short > most || long < fewest {
            errs.push(format!("pattern {} makes names of {short} to {long} characters, never inside its length {length:?}", n + 1));
        }
    }
    let total: u64 = resolved.iter().map(|(weight, ..)| u64::from(*weight)).sum();
    if total > u64::from(u32::MAX) {
        errs.push(format!("its weights add up to {total}, past the most a draw can roll, {}", u32::MAX));
    }
    if !errs.is_empty() {
        errs.into_iter().for_each(wrong);
        return None;
    }
    let patterns = resolved.into_iter().map(|(weight, pieces, _)| (weight, pieces)).collect();
    let forbid = forbid.iter().map(|f| f.to_lowercase()).collect();
    Some(NameSet { pools, patterns, total: total as u32, case, length: (fewest, most), forbid })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    fn one(text: &str) -> NameSet {
        load(&format!("{{ \"s\": ({text}) }}")).unwrap().set("s").unwrap().clone()
    }

    fn rich() -> NameSet {
        one(r#"pools: { "start": ["ka", "lo", "mer", "tav", "sil"], "middle": ["a", "en", "or"],
                       "end": ["n", "dra", "vik", "set"], "byname": ["younger", "longstep"] },
               patterns: [(weight: 3, parts: [Pool("start"), Maybe(35, "middle"), Pool("end")]),
                          (weight: 1, parts: [Pool("start"), Pool("end"), Text(" "), Pool("byname")])],
               length: (3, 24), forbid: ["kak"]"#)
    }

    #[test]
    fn every_drawn_name_passes_its_own_rejections_and_length() {
        let set = rich();
        for seed in 0..500 {
            let name = set.draw(&mut StdRng::seed_from_u64(seed)).expect("a generous set always finds a name");
            assert!(set.accepts(&name), "{name:?} on seed {seed}");
            assert!((3..=24).contains(&name.chars().count()));
        }
    }

    #[test]
    fn a_maybe_part_appears_at_about_its_chance() {
        let set = one(r#"pools: { "a": ["ka"], "b": ["lo"] }, patterns: [(weight: 1, parts: [Pool("a"), Maybe(30, "b")])]"#);
        let with = (0..2000).filter(|seed| set.draw(&mut StdRng::seed_from_u64(*seed)).unwrap() == "Kalo").count();
        assert!((500..700).contains(&with), "{with} of 2000");
    }

    #[test]
    fn patterns_are_drawn_by_weight() {
        let set = one(r#"pools: { "a": ["ka"], "b": ["lo"] },
                        patterns: [(weight: 3, parts: [Pool("a")]), (weight: 1, parts: [Pool("b")])]"#);
        let first = (0..2000).filter(|seed| set.draw(&mut StdRng::seed_from_u64(*seed)).unwrap() == "Ka").count();
        assert!((1400..1600).contains(&first), "{first} of 2000");
    }

    #[test]
    fn the_same_stream_draws_the_same_name() {
        let set = rich();
        for seed in 0..100 {
            assert_eq!(set.draw(&mut StdRng::seed_from_u64(seed)), set.draw(&mut StdRng::seed_from_u64(seed)));
        }
    }

    #[test]
    fn draw_distinct_never_returns_a_taken_name() {
        let set = one(r#"pools: { "a": ["ka", "lo", "mer"] }, patterns: [(weight: 1, parts: [Pool("a")])]"#);
        for seed in 0..200 {
            let name = set.draw_distinct(&mut StdRng::seed_from_u64(seed), |n| n == "Ka").unwrap();
            assert_ne!(name, "Ka");
        }
    }

    #[test]
    fn draw_distinct_gives_up_when_every_name_is_taken() {
        let set = one(r#"pools: { "a": ["ka"] }, patterns: [(weight: 1, parts: [Pool("a")])]"#);
        assert_eq!(set.draw_distinct(&mut StdRng::seed_from_u64(3), |n| n == "Ka"), None);
    }

    #[test]
    fn a_set_whose_rejections_refuse_everything_gives_up_rather_than_loops() {
        let set = one(r#"pools: { "a": ["aaa"] }, patterns: [(weight: 1, parts: [Pool("a")])]"#);
        assert_eq!(set.draw(&mut StdRng::seed_from_u64(4)), None);
    }

    #[test]
    fn title_case_capitalises_each_word_after_a_space_or_a_hyphen() {
        assert_eq!(cased(&[("osk the-YOUNGER", true)], NameCase::Title), "Osk The-Younger");
        assert_eq!(cased(&[("osk", true)], NameCase::Upper), "OSK");
        assert_eq!(cased(&[("OsK", true)], NameCase::Lower), "osk");
        assert_eq!(cased(&[("mcKay", true)], NameCase::AsWritten), "mcKay");
    }

    #[test]
    fn text_a_pattern_writes_is_never_cased_and_a_syllable_after_its_space_starts_a_word() {
        let set = |case| {
            one(&format!(r#"pools: {{ "a": ["ka"], "b": ["lo"] }}, patterns: [(weight: 1, parts: [Pool("a"), Text(" the "), Pool("b")])], case: {case}"#))
        };
        assert_eq!(set("Title").draw(&mut StdRng::seed_from_u64(6)).as_deref(), Some("Ka the Lo"));
        assert_eq!(set("Upper").draw(&mut StdRng::seed_from_u64(6)).as_deref(), Some("KA the LO"));
    }

    #[test]
    fn length_and_case_count_characters_not_bytes() {
        assert_eq!(cased(&[("ørn", true)], NameCase::Title), "Ørn");
        let set = one(r#"pools: { "a": ["ørn"] }, patterns: [(weight: 1, parts: [Pool("a")])], length: (3, 3)"#);
        assert_eq!(set.draw(&mut StdRng::seed_from_u64(5)).as_deref(), Some("Ørn"));
    }

    #[test]
    fn load_lists_every_mistake_naming_its_set() {
        let err = load(
            r#"{
                "bad": (pools: { "start": ["ka", ""], "unused": ["x"] },
                        patterns: [(weight: 0, parts: [Pool("start")]),
                                   (weight: 1, parts: [Pool("midle"), Maybe(0, "start"), Maybe(101, "start")])]),
                "short": (pools: { "a": ["ka"], "u": ["x"] }, patterns: [(weight: 1, parts: [Pool("a")])], length: (5, 9), forbid: [""]),
                "heavy": (pools: { "a": ["ka"] }, patterns: [(weight: 3000000000, parts: [Pool("a")]), (weight: 3000000000, parts: [Pool("a")])]),
                "none": (pools: {}, patterns: [], length: (4, 2)),
            }"#,
        )
        .unwrap_err();
        let errors = match err {
            ContentError::Invalid(errors) => errors,
            other => panic!("{other}"),
        };
        for expected in [
            r#"set "bad": the pool "start" holds an empty syllable"#,
            r#"set "bad": pattern 1 has a weight of nought"#,
            r#"set "bad": pattern 2 names the pool "midle", which the set does not have"#,
            r#"set "bad": pattern 2 has a chance of 0"#,
            r#"set "bad": pattern 2 has a chance of 101"#,
            r#"set "bad": no pattern names the pool "unused""#,
            r#"set "short": pattern 1 makes names of 2 to 2 characters, never inside its length (5, 9)"#,
            r#"set "short": no pattern names the pool "u""#,
            r#"set "short": it forbids an empty substring"#,
            r#"set "heavy": its weights add up to 6000000000"#,
            r#"set "none": it has no patterns"#,
            r#"set "none": its length (4, 2)"#,
        ] {
            assert!(errors.iter().any(|e| e.starts_with(expected)), "missing {expected:?} in {errors:#?}");
        }
    }

    #[test]
    fn length_is_counted_as_cased_and_load_allows_for_a_letter_that_grows() {
        let set = one(r#"pools: { "a": ["ßa"] }, patterns: [(weight: 1, parts: [Pool("a")])], case: Upper, length: (3, 3)"#);
        assert_eq!(set.draw(&mut StdRng::seed_from_u64(7)).as_deref(), Some("SSA"));
    }

    #[test]
    fn a_misspelt_field_is_refused() {
        assert!(matches!(load(r#"{ "s": (pools: {}, patterns: [], lenght: (1, 2)) }"#), Err(ContentError::Parse(_))));
    }

    #[test]
    fn a_book_answers_its_sets_by_name() {
        let book = load(
            r#"{ "b": (pools: { "a": ["ka"] }, patterns: [(weight: 1, parts: [Pool("a")])]),
                 "a": (pools: { "a": ["lo"] }, patterns: [(weight: 1, parts: [Pool("a")])]) }"#,
        )
        .unwrap();
        assert!(book.set("a").is_some() && book.set("nobody").is_none());
        assert_eq!(book.sets().map(|(name, _)| name).collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn the_rejections_are_libtcods() {
        let set = one(r#"pools: { "a": ["ka"] }, patterns: [(weight: 1, parts: [Pool("a")])], forbid: ["LO"]"#);
        for refused in ["aaab", "abab", "karkar", " ka", "ka ", "ka  lo", "kalom", "Ka Ka", "mer ka mer"] {
            assert!(!set.accepts(refused), "{refused:?} should be refused");
        }
        for kept in ["kamer", "ka mer", "Tavik"] {
            assert!(set.accepts(kept), "{kept:?} should be kept");
        }
    }
}
