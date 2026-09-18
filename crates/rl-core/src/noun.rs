//! A thing's name, said of one or of several.
//!
//! A game names a thing once, singular, as it would say one of them:
//! `pebble`, `throwing knife`, `bottle of water`. Everything that shows a
//! name asks here how to say it: [`listed`] for a row in a list, `5
//! pebbles`; [`counted`] for a sentence, `a pebble` or `5 pebbles`.
//! Storing a second name for the plural was the alternative, and it is a
//! second string on every stackable thing to keep in step with the first.
//!
//! English only, and the common rules rather than a dictionary: a regular
//! plural, the endings that change (`knife`, `berry`, `box`), and a short
//! list of the irregular. A mass noun has no plural of its own, so a game
//! names it by its measure: `bottle of water` becomes `3 bottles of water`,
//! since the word before `of` is the one that counts. A name that already
//! carries an article, `a crust of bread`, or starts with a capital, a
//! proper name, is said as written.

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

/// `name` in the plural: `pebble` to `pebbles`, `throwing knife` to
/// `throwing knives`, `bottle of water` to `bottles of water`.
///
/// ```
/// use rl_core::noun::plural;
/// assert_eq!(plural("berry"), "berries");
/// assert_eq!(plural("cup of tea"), "cups of tea");
/// ```
pub fn plural(name: &str) -> String {
    let name = bare(name);
    // The word that counts is the last one before "of", or the last one.
    let (head, tail) = match name.find(" of ") {
        Some(at) => name.split_at(at),
        None => (name, ""),
    };
    let (before, word) = match head.rfind(' ') {
        Some(at) => head.split_at(at + 1),
        None => ("", head),
    };
    format!("{before}{}{tail}", plural_word(word))
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
    ARTICLES.iter().any(|a| name.starts_with(a)) || name.chars().next().is_some_and(char::is_uppercase)
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
    // Spelled with a vowel, said with a consonant, and the reverse.
    let sounds_like_a_vowel = if ["uni", "use", "usu", "uti", "one", "eu", "ure"].iter().any(|p| lower.starts_with(p)) {
        false
    } else if ["hour", "honest", "honor", "honour", "heir"].iter().any(|p| lower.starts_with(p)) {
        true
    } else {
        lower.starts_with(['a', 'e', 'i', 'o', 'u'])
    };
    format!("{} {name}", if sounds_like_a_vowel { "an" } else { "a" })
}

/// `count` of `name` as a sentence says it: `a pebble`, or `5 pebbles`.
pub fn counted(name: &str, count: u32) -> String {
    if count == 1 { with_article(name) } else { format!("{count} {}", plural(name)) }
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
    fn an_article_is_chosen_by_sound_and_never_doubled() {
        assert_eq!(with_article("pebble"), "a pebble");
        assert_eq!(with_article("ember"), "an ember");
        assert_eq!(with_article("hour glass"), "an hour glass");
        assert_eq!(with_article("uniform"), "a uniform");
        assert_eq!(with_article("the stairs"), "the stairs");
        assert_eq!(with_article("Ada"), "Ada", "a proper name takes none");
        assert_eq!(with_article(""), "");
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
