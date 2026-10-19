//! A thing's name, said of one or of several.
//!
//! A game names a thing once, singular, as it would say one of them:
//! `pebble`, `throwing knife`, `bottle of water`. Everything that shows a
//! name asks here how to say it: [`listed`] for a row in a list, `5
//! pebbles`; `counted` for a sentence, `a pebble` or `5 pebbles`.
//! Storing a second name for the plural was the alternative, and it is a
//! second string on every stackable thing to keep in step with the first.
//!
//! English only, and the common rules rather than a dictionary: a regular
//! plural, the endings that change (`knife`, `berry`, `box`), and a short
//! list of the irregular. A mass noun has no plural of its own, so a game
//! names it by its measure: `bottle of water` becomes `3 bottles of water`,
//! since the word before `of` is the one that counts. So is the word before
//! a short list like it, `titled`, `called`, `named`, `from`, `with` and a
//! few more, written in lower case, and what follows is kept as written:
//! `2 scrolls titled XOMBA RENTOR`. A name that already carries an article,
//! `a crust of bread`, or starts with a capital, a proper name, is said as
//! written.
//!
//! Some names take no `a` at all: a stuff, `chain mail` or `leather
//! armor`, and a thing named in the plural, `gauntlets`. Which ones is a
//! fact about the name the rules here cannot read off its spelling, since
//! `mail` is a stuff and `nail` is not, so a game says so, and
//! [`counted_as`] takes it as a [`Countability`]. What is the speaker's own
//! is [`owned`]: `your chain mail`, whichever kind of name it is, and a
//! proper name as written, `the Widow's Fang`, since it is called that in
//! anyone's hand. Whether a name is proper is read off how it is written,
//! by [`is_proper`] alone: a capital, or `the` and a capital.

/// Words whose plural is not made by a rule.
const IRREGULAR: [(&str, &str); 12] = [
    ("child", "children"),
    ("foot", "feet"),
    ("tooth", "teeth"),
    ("goose", "geese"),
    ("mouse", "mice"),
    ("louse", "lice"),
    ("person", "people"),
    ("ox", "oxen"),
    ("man", "men"),
    ("woman", "women"),
    ("thief", "thieves"),
    ("sheaf", "sheaves"),
];

/// Words the same in the plural.
const UNCHANGED: [&str; 5] = ["sheep", "deer", "fish", "moose", "aircraft"];

/// Words ending in a consonant and `o` that take `es`.
const TAKES_ES: [&str; 5] = ["potato", "tomato", "hero", "echo", "torpedo"];

/// Articles a name may already carry.
const ARTICLES: [&str; 4] = ["a ", "an ", "the ", "some "];

/// Words that join a name's head to what tells one of it from another:
/// `bottle of water`, `scroll titled XOMBA RENTOR`, `letter from Ada`. The
/// head is the last word before the first of them, which is the word that
/// counts, and everything from it on is kept as written.
///
/// Only as written here, in lower case, since a title or a proper name
/// written in capitals has words of its own that are not the name's
/// grammar. A short list rather than every preposition, because a word on
/// it that turns up inside a head moves the count to the wrong word, and
/// these are the ones a thing's name is joined by.
const CONNECTIVES: [&str; 13] = ["of", "from", "with", "for", "to", "in", "on", "titled", "called", "named", "labelled", "labeled", "marked"];

/// `name` in the plural: `pebble` to `pebbles`, `throwing knife` to
/// `throwing knives`, `bottle of water` to `bottles of water`.
///
/// ```
/// use rl_core::noun::plural;
/// assert_eq!(plural("berry"), "berries");
/// assert_eq!(plural("cup of tea"), "cups of tea");
/// assert_eq!(plural("scroll titled ZUM KA"), "scrolls titled ZUM KA");
/// ```
///
/// A proper name that names its owner counts what is owned, however many
/// words the owner's own name runs to: `Queen of Thorns's Crown` to
/// `Queen of Thorns's Crowns`, where the connective in the owner's name
/// would otherwise count the queen.
///
/// ```
/// use rl_core::noun::plural;
/// assert_eq!(plural("the Queen of Thorns's Crown"), "Queen of Thorns's Crowns");
/// assert_eq!(plural("the Crown of Grubnak"), "Crowns of Grubnak");
/// ```
pub fn plural(name: &str) -> String {
    let name = bare(name);
    let (owner, name) = split_owner(name);
    // The word that counts is the last one before a connective, or the
    // last one.
    let (head, tail) = split_head(name);
    let (before, word) = match head.rfind(' ') {
        Some(at) => head.split_at(at + 1),
        None => ("", head),
    };
    format!("{owner}{before}{}{tail}", plural_word(word))
}

/// A proper name split after its last possessive, the owner with its
/// space and what is owned: `Queen of Thorns's ` and `Crown`. Anything
/// else is all owned.
///
/// Only a proper name, because only there is the owner's name a name in
/// its own right, written with its own connectives: `the Queen of
/// Thorns's Crown` is one crown of one queen. In a kind's name a
/// possessive after a connective belongs to what the connective joins,
/// `letter from Ada's mother`, and is kept as written with it.
fn split_owner(name: &str) -> (&str, &str) {
    if !is_proper(name) {
        return ("", name);
    }
    let owner_ends = ["'s ", "s' ", "\u{2019}s ", "s\u{2019} "].iter().filter_map(|mark| name.rfind(mark).map(|at| at + mark.len())).max();
    match owner_ends {
        Some(at) if at < name.len() => name.split_at(at),
        _ => ("", name),
    }
}

/// `name` split before its first connective, the space with what follows:
/// `scroll` and ` titled ZUM KA`. A connective is never the first word,
/// which would leave no head, nor the last, which would join nothing.
fn split_head(name: &str) -> (&str, &str) {
    for (at, _) in name.match_indices(' ').filter(|(at, _)| *at > 0) {
        // The word after this space, and only when another follows it.
        if let Some((word, _)) = name[at + 1..].split_once(' ')
            && CONNECTIVES.contains(&word)
        {
            return name.split_at(at);
        }
    }
    (name, "")
}

/// One word in the plural.
fn plural_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    if let Some((_, many)) = IRREGULAR.iter().find(|(one, _)| *one == lower) {
        return keep_case(word, many);
    }
    // Compounds of man and woman: craftsman, fisherman, horsewoman. Not
    // every word ending in "man" is one, which is why human stays human.
    if ["sman", "erman", "swoman"].iter().any(|e| lower.ends_with(e)) {
        return format!("{}en", &word[..word.len() - 2]);
    }
    if UNCHANGED.contains(&lower.as_str()) {
        return word.to_string();
    }
    let stem = |cut: usize| &word[..word.len() - cut];
    let consonant_before = |cut: usize| lower.as_bytes().get(lower.len().wrapping_sub(cut + 1)).is_some_and(|c| !b"aeiou".contains(c));
    if lower.ends_with("fe") {
        return format!("{}ves", stem(2));
    }
    if ["lf", "eaf", "oaf", "arf"].iter().any(|e| lower.ends_with(e)) {
        return format!("{}ves", stem(1));
    }
    if ["s", "x", "z", "ch", "sh"].iter().any(|e| lower.ends_with(e)) {
        return format!("{word}es");
    }
    if lower.ends_with('y') && consonant_before(1) {
        return format!("{}ies", stem(1));
    }
    if lower.ends_with('o') && TAKES_ES.contains(&lower.as_str()) {
        return format!("{word}es");
    }
    format!("{word}s")
}

/// `many` with the first letter capitalised if `like`'s was.
fn keep_case(like: &str, many: &str) -> String {
    match like.chars().next() {
        Some(c) if c.is_uppercase() => {
            let mut out: String = many.chars().take(1).flat_map(char::to_uppercase).collect();
            out.push_str(&many[many.chars().next().map_or(0, char::len_utf8)..]);
            out
        }
        _ => many.to_string(),
    }
}

/// `name` without an article it was written with.
fn bare(name: &str) -> &str {
    ARTICLES.iter().find_map(|a| name.strip_prefix(a)).unwrap_or(name)
}

/// Whether `name` is said as written: it carries an article, or it is a
/// proper name.
fn as_written(name: &str) -> bool {
    ARTICLES.iter().any(|a| name.starts_with(a)) || is_proper(name)
}

/// Whether `name` is a proper name, by the one test every function here
/// uses: it starts with a capital, or with `the` and then a capital. A
/// kind is written lower case, `goblin`, and someone with a name of their
/// own is not, `Brenna`, so a sentence that reads for one can be told from
/// one that does not without a list of who is who. A title is a name too,
/// `the Widow`, and its article is part of how it is said rather than a
/// sign of a kind, which is why `the stairs` is not one and `the Widow`
/// is. Only `the`: nobody is called `a Widow`.
///
/// ```
/// use rl_core::noun::is_proper;
/// assert!(is_proper("Brenna"));
/// assert!(is_proper("the Widow"));
/// assert!(!is_proper("goblin"));
/// assert!(!is_proper("the stairs"));
/// ```
pub fn is_proper(name: &str) -> bool {
    let capital = |s: &str| s.chars().next().is_some_and(char::is_uppercase);
    capital(name) || name.strip_prefix("the ").is_some_and(capital)
}

/// `name` with `a` or `an` before it, unless it already carries an article
/// or is a proper name.
///
/// ```
/// use rl_core::noun::with_article;
/// assert_eq!(with_article("pebble"), "a pebble");
/// assert_eq!(with_article("owl"), "an owl");
/// assert_eq!(with_article("a crust of bread"), "a crust of bread");
/// ```
pub fn with_article(name: &str) -> String {
    if name.is_empty() || as_written(name) {
        return name.to_string();
    }
    let lower = name.to_ascii_lowercase();
    // Spelled with a vowel, said with a consonant, and the reverse. A
    // word that starts "un" and then "i" is "un-" before most of them,
    // "unidentified", "unimportant", "uninvited", and "uni-" said "you"
    // before the rest, "uniform", "unicorn", "unique".
    let sounds_like_a_vowel = if ["unid", "unim", "unin"].iter().any(|p| lower.starts_with(p)) {
        true
    } else if ["uni", "use", "usu", "uti", "one", "eu", "ure"].iter().any(|p| lower.starts_with(p)) {
        false
    } else if ["hour", "honest", "honor", "honour", "heir"].iter().any(|p| lower.starts_with(p)) {
        true
    } else {
        lower.starts_with(['a', 'e', 'i', 'o', 'u'])
    };
    format!("{} {name}", if sounds_like_a_vowel { "an" } else { "a" })
}

/// `name` with `the` before it, unless it already carries an article or
/// is a proper name, in which case it is said as written.
///
/// A name already written with its own article keeps it: `the stairs` is
/// said `the stairs`, not `the the stairs`, and a bare proper name like
/// `Grubnak` takes no article at all.
///
/// ```
/// use rl_core::noun::with_the;
/// assert_eq!(with_the("pebble"), "the pebble");
/// assert_eq!(with_the("the stairs"), "the stairs");
/// assert_eq!(with_the("a crust of bread"), "a crust of bread");
/// assert_eq!(with_the("Grubnak"), "Grubnak");
/// ```
pub fn with_the(name: &str) -> String {
    if name.is_empty() || as_written(name) { name.to_string() } else { format!("the {name}") }
}

/// Whether a name is said with `a`: one of something, `a sword`, or a
/// stuff or a pair, `chain mail` and `gauntlets`, which is said with none.
///
/// Grammar rather than content, so a closed pair: every name is one or the
/// other, and which one is the game's to say of each.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Countability {
    /// Counted one at a time: `a sword`, `5 arrows`.
    #[default]
    Count,
    /// A stuff, or a thing named in the plural: `chain mail`, `gauntlets`.
    /// It has no plural of its own, so several are counted as written.
    Mass,
}

/// `count` of `name` as a sentence says it: `a pebble`, or `5 pebbles`.
pub fn counted(name: &str, count: u32) -> String {
    counted_as(name, count, Countability::Count)
}

/// `count` of `name` as a sentence says it, counted as `countability`
/// says: `a pebble` or `5 pebbles`, and `chain mail`, with no `a`, or `2
/// chain mail`, since a stuff has no plural of its own.
///
/// ```
/// use rl_core::noun::{Countability, counted_as};
/// assert_eq!(counted_as("chain mail", 1, Countability::Mass), "chain mail");
/// assert_eq!(counted_as("sword", 1, Countability::Count), "a sword");
/// ```
pub fn counted_as(name: &str, count: u32, countability: Countability) -> String {
    match (countability, count) {
        (Countability::Count, 1) => with_article(name),
        (Countability::Count, _) => format!("{count} {}", plural(name)),
        (Countability::Mass, 1) => name.to_string(),
        (Countability::Mass, _) => format!("{count} {}", bare(name)),
    }
}

/// `count` of `name` as `owner`'s own, `your` or `its`: `your sword`,
/// `your 5 arrows`, `your chain mail`. An article the name was written
/// with gives way to the owner, so a crust of bread is `your crust of
/// bread`; a stuff is counted as [`counted_as`] counts it.
///
/// One thing with a proper name is said as written, whoever holds it:
/// `the Widow's Fang`, never `your Widow's Fang`. Its name is what it is
/// called in anyone's hand, and the owner in front of it took the place of
/// the article that is part of the name. Proper is [`is_proper`]'s one
/// test, the same every function here reads.
///
/// ```
/// use rl_core::noun::{Countability, owned};
/// assert_eq!(owned("your", "sword", 1, Countability::Count), "your sword");
/// assert_eq!(owned("your", "arrow", 5, Countability::Count), "your 5 arrows");
/// assert_eq!(owned("your", "chain mail", 1, Countability::Mass), "your chain mail");
/// assert_eq!(owned("your", "the Widow's Fang", 1, Countability::Count), "the Widow's Fang");
/// ```
pub fn owned(owner: &str, name: &str, count: u32, countability: Countability) -> String {
    if count == 1 && is_proper(name) {
        return name.to_string();
    }
    let name = bare(name);
    match (countability, count) {
        (_, 1) => format!("{owner} {name}"),
        (Countability::Count, _) => format!("{owner} {count} {}", plural(name)),
        (Countability::Mass, _) => format!("{owner} {count} {name}"),
    }
}

/// `count` of `name` as a list shows it: `pebble`, or `5 pebbles`.
pub fn listed(name: &str, count: u32) -> String {
    if count == 1 { name.to_string() } else { format!("{count} {}", plural(name)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_common_endings_each_take_their_own_plural() {
        for (one, many) in [
            ("pebble", "pebbles"),
            ("knife", "knives"),
            ("wolf", "wolves"),
            ("leaf", "leaves"),
            ("roof", "roofs"),
            ("berry", "berries"),
            ("key", "keys"),
            ("box", "boxes"),
            ("torch", "torches"),
            ("brush", "brushes"),
            ("glass", "glasses"),
            ("potato", "potatoes"),
            ("piano", "pianos"),
            ("tooth", "teeth"),
            ("thief", "thieves"),
            ("sheep", "sheep"),
            ("craftsman", "craftsmen"),
            ("human", "humans"),
        ] {
            assert_eq!(plural(one), many, "{one}");
        }
    }

    #[test]
    fn the_word_that_counts_is_the_last_before_of_or_the_last_of_all() {
        assert_eq!(plural("throwing knife"), "throwing knives");
        assert_eq!(plural("bottle of water"), "bottles of water");
        assert_eq!(plural("sheet of plate glass"), "sheets of plate glass");
        assert_eq!(plural("a crust of bread"), "crusts of bread", "an article is dropped, not pluralised");
    }

    #[test]
    fn the_head_before_a_lower_case_connective_counts_and_what_follows_is_kept_as_written() {
        for (one, many) in [
            ("scroll titled XOMBA RENTOR", "scrolls titled XOMBA RENTOR"),
            ("throwing knife called Biter", "throwing knives called Biter"),
            ("dog named Rex", "dogs named Rex"),
            ("jar labelled SALT", "jars labelled SALT"),
            ("crate marked FRAGILE", "crates marked FRAGILE"),
            ("letter from Ada", "letters from Ada"),
            ("box with a false bottom", "boxes with a false bottom"),
            ("key to the lower gate", "keys to the lower gate"),
            ("ship in a bottle", "ships in a bottle"),
            ("scroll titled ZUM of KA", "scrolls titled ZUM of KA"),
        ] {
            assert_eq!(plural(one), many, "{one}");
        }
        assert_eq!(listed("scroll titled XOMBA RENTOR", 2), "2 scrolls titled XOMBA RENTOR");
        assert_eq!(counted("scroll titled XOMBA RENTOR", 1), "a scroll titled XOMBA RENTOR");
    }

    #[test]
    fn a_connective_in_capitals_or_with_nothing_after_it_joins_nothing_and_a_proper_name_is_said_as_written() {
        assert!(plural("XOMBA OF RENTOR").starts_with("XOMBA OF "), "a title's own words are not the name's grammar");
        assert_eq!(plural("lean to"), "lean tos", "a connective needs something to join");
        assert_eq!(counted("Grubnak", 1), "Grubnak");
        assert_eq!(with_article("Grubnak from the Hills"), "Grubnak from the Hills");
    }

    #[test]
    fn an_article_is_chosen_by_sound_and_never_doubled() {
        assert_eq!(with_article("pebble"), "a pebble");
        assert_eq!(with_article("ember"), "an ember");
        assert_eq!(with_article("hour glass"), "an hour glass");
        assert_eq!(with_article("uniform"), "a uniform");
        assert_eq!(with_article("unicorn"), "a unicorn");
        assert_eq!(with_article("unidentified sword"), "an unidentified sword", "un- before an i is said with a vowel");
        assert_eq!(with_article("unimportant thing"), "an unimportant thing");
        assert_eq!(with_article("the stairs"), "the stairs");
        assert_eq!(with_article("Ada"), "Ada", "a proper name takes none");
        assert_eq!(with_article(""), "");
    }

    #[test]
    fn the_is_not_doubled_before_a_name_that_already_carries_an_article_or_is_proper() {
        assert_eq!(with_the("pebble"), "the pebble");
        assert_eq!(with_the("the stairs"), "the stairs");
        assert_eq!(with_the("a crust of bread"), "a crust of bread");
        assert_eq!(with_the("Grubnak"), "Grubnak");
        assert_eq!(with_the(""), "");
    }

    #[test]
    fn a_capital_or_the_and_a_capital_makes_a_proper_name_and_any_other_article_does_not() {
        for name in ["Brenna", "Grubnak the goblin king", "the Widow", "the Drowned Abbess", "The Choirmaster"] {
            assert!(is_proper(name), "{name}");
        }
        for name in ["goblin", "the stairs", "a Widow", "an Ember", "some Gold", "the ", "the", ""] {
            assert!(!is_proper(name), "{name}");
        }
    }

    #[test]
    fn a_mass_name_is_said_with_no_article_and_no_plural() {
        for name in ["chain mail", "leather armor", "gauntlets"] {
            assert_eq!(counted_as(name, 1, Countability::Mass), name);
        }
        assert_eq!(counted_as("chain mail", 2, Countability::Mass), "2 chain mail", "a stuff has no plural of its own");
        assert_eq!(counted_as("ember", 1, Countability::Count), "an ember", "a count name is said as counted says it");
        assert_eq!(counted_as("pebble", 3, Countability::Count), counted("pebble", 3));
    }

    #[test]
    fn what_is_owned_is_said_with_its_owner_in_place_of_any_article() {
        assert_eq!(owned("your", "sword", 1, Countability::Count), "your sword");
        assert_eq!(owned("your", "arrow", 5, Countability::Count), "your 5 arrows");
        assert_eq!(owned("your", "chain mail", 1, Countability::Mass), "your chain mail");
        assert_eq!(owned("your", "gauntlets", 2, Countability::Mass), "your 2 gauntlets");
        assert_eq!(owned("its", "a crust of bread", 1, Countability::Count), "its crust of bread", "the owner takes the article's place");
    }

    #[test]
    fn one_thing_with_a_proper_name_is_said_as_written_by_whoever_owns_it() {
        for name in ["the Widow's Fang", "the Crown of Grubnak", "Biter"] {
            assert_eq!(owned("your", name, 1, Countability::Count), name);
            assert_eq!(owned("its", name, 1, Countability::Mass), name);
        }
        assert_eq!(owned("your", "the stairs", 1, Countability::Count), "your stairs", "a kind's article still gives way");
        assert_eq!(owned("your", "the Widow's Fang", 2, Countability::Count), "your 2 Widow's Fangs", "several are counted as anything is");
    }

    #[test]
    fn a_proper_name_through_a_connective_pluralises_the_word_that_counts() {
        for (one, many) in [
            ("the Crown of Grubnak", "Crowns of Grubnak"),
            ("the Widow's Fang", "Widow's Fangs"),
            ("the Queen of Thorns's Crown", "Queen of Thorns's Crowns"),
            ("the Lord of the Deep's Ring of Ages", "Lord of the Deep's Rings of Ages"),
            ("the Brothers' Knife", "Brothers' Knives"),
            ("Lady in Red\u{2019}s Fan", "Lady in Red\u{2019}s Fans"),
        ] {
            assert_eq!(plural(one), many, "{one}");
        }
        assert_eq!(plural("letter from Ada's mother"), "letters from Ada's mother", "a kind's possessive after a connective is kept as written");
    }

    #[test]
    fn one_is_said_with_an_article_and_listed_bare_and_many_are_counted_either_way() {
        assert_eq!(counted("pebble", 1), "a pebble");
        assert_eq!(counted("pebble", 5), "5 pebbles");
        assert_eq!(listed("pebble", 1), "pebble");
        assert_eq!(listed("pebble", 5), "5 pebbles");
        assert_eq!(counted("pebble", 0), "0 pebbles");
    }
}
