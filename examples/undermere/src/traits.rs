//! Traits: the named bundles a monster is written as and an item lends.
//!
//! A monster in `monsters.ron` is a short core, the things every monster
//! has, and a list of trait names. A trait is written once, in
//! `traits.ron`, and bundles what the engine already knows how to do:
//! wits, speed, sight, a flee threshold, how it notices and hears,
//! light, resistances, armor, abilities, statuses held for life, what its
//! stats do while it holds a status, how far it keeps from an enemy, its
//! own triggers, and the words the examine
//! panel says about it. [`fold`] turns a list of traits into one
//! [`Folded`] set of values at load, so every name is checked once and a
//! spawn reads plain values.
//!
//! Lists add up across the traits folded together: resistances,
//! abilities, statuses, what holding one does, triggers, and wits, which
//! are a set of
//! capabilities rather than a level. Every other value may be set by one
//! trait of the list only, and a second setting is a [`Clash`] naming
//! both. Not last-wins,
//! which would make a monster's speed hang on the order its traits were
//! written in, and not first-wins, which would hide the second trait's
//! number from whoever reads the file: a monster is always its core plus
//! its traits, with nothing hidden in between.

use std::fmt;

use atglyph::prelude::all::*;
use atglyph::rules::parts::KeepAway;
use serde::Deserialize;

/// The traits file, compiled in so the binary runs from anywhere.
const TRAITS_RON: &str = include_str!("../assets/traits.ron");

/// A registered trait id.
pub type TraitId = Id<TraitDef>;

/// What a kind's stats do while it holds one status, as `traits.ron`
/// writes it: the status, and each stat with its change.
pub type HoldingRule = (NameRef<StatusDef>, Vec<(NameRef<StatDef>, Op)>);

/// One trait, as authored in `traits.ron`.
///
/// Every field but `name` may be left out, and one left out says nothing:
/// a trait is only what it sets. Unknown fields are refused, so a value
/// written under a name the fold does not read fails the load rather than
/// loading a trait that quietly does less.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraitDef {
    /// Unique; what a monster's `traits` and an item's `lends` name it by.
    pub name: String,
    /// The examine panel's words for it.
    #[serde(default)]
    pub says: Option<String>,
    /// What it lets a mind do, added to what the other traits let it do.
    #[serde(default)]
    pub wits: Option<Wits>,
    /// Percent of normal speed.
    #[serde(default)]
    pub speed: Option<u32>,
    /// How far it sees, in tiles, through the light.
    #[serde(default)]
    pub perception: Option<i32>,
    /// Tiles it sees with no light at all.
    #[serde(default)]
    pub dark_sight: Option<i32>,
    /// Percent health at or below which it runs; zero never runs.
    #[serde(default)]
    pub flee_at: Option<i32>,
    /// How it notices what is trying not to be seen.
    #[serde(default)]
    pub notice: Option<NoticeStats>,
    /// How it hears.
    #[serde(default)]
    pub hearing: Option<HearingStats>,
    /// The light it sheds.
    #[serde(default)]
    pub light: Option<Glow>,
    /// Percent removed per damage kind, added to the other traits'.
    #[serde(default)]
    pub resists: Vec<(NameRef<DamageKind>, i32)>,
    /// Flat armor.
    #[serde(default)]
    pub armor: Option<i32>,
    /// The abilities it knows, beside the other traits'.
    #[serde(default)]
    pub abilities: Vec<NameRef<AbilityDef>>,
    /// The statuses it holds for life, beside the other traits'.
    #[serde(default)]
    pub statuses: Vec<NameRef<StatusDef>>,
    /// What its stats do while it holds a status, by status: the engine's
    /// `while_holding`, beside the other traits'.
    #[serde(default)]
    pub while_holding: Vec<HoldingRule>,
    /// What it does at its own moments, beside the other traits': a bite
    /// that poisons on `hit`, a burst where it dies on `died`. Landed
    /// through the engine's actor triggers, built once per monster by
    /// [`Bestiary`](crate::bestiary::Bestiary).
    #[serde(default)]
    pub triggers: Vec<TriggerSpec>,
    /// How far it lets an enemy get before it closes, and how close it
    /// lets one get before it backs away: a will-o'-wisp's.
    #[serde(default)]
    pub keep_away: Option<KeepAway>,
    /// Whether this is a trick: unknown of a kind until the adventurer has
    /// seen it land, a bite that poisons or a jelly that splits, rather
    /// than known of it on sight, its look, its speed or whether it
    /// flies. Absent, false: most of what a trait sets is plain.
    #[serde(default)]
    pub trick: bool,
}

impl Named for TraitDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Reads a traits file, every name in it resolved through `names`, which
/// must hold the abilities beside the registries, as [`load_defs`] gives
/// it them.
///
/// Reports every unknown name and every entry that does not parse or does
/// not make sense at once, each under the trait it is in.
pub fn load(text: &str, names: &Names<'_>) -> Result<Registry<TraitDef>, ContentError> {
    let defs: Registry<TraitDef> = names.load(text)?;
    defs.validate(validate)?;
    Ok(defs)
}

/// One trait's numbers, checked against nothing but themselves: a speed
/// of nothing is a monster that never acts, and a share of health past the
/// whole of it is a typo for a smaller one.
fn validate(t: &TraitDef, _: &Registry<TraitDef>) -> Result<(), String> {
    if t.speed == Some(0) {
        return Err("a speed of 0 never acts".into());
    }
    if let Some(pct) = t.flee_at.filter(|pct| !(0..=100).contains(pct)) {
        return Err(format!("flee_at {pct} is not a share of health, which runs from 0 to 100"));
    }
    Ok(())
}

/// `traits.ron`, read against `registries` and the `abilities` it grants.
/// Panics naming the file and every problem in it, since a broken trait is
/// a game that cannot start.
pub fn load_defs(registries: &Registries, abilities: &Registry<AbilityDef>) -> Registry<TraitDef> {
    load(TRAITS_RON, &registries.names().with("ability", abilities)).unwrap_or_else(|e| panic!("assets/traits.ron: {e}"))
}

/// A list of traits folded into one set of values: what a monster is
/// besides its core, or what an item lends whoever wears it.
///
/// A single value is `None` where no trait set it, and whoever reads it
/// says what absence means: a monster with no `speed` moves at a normal
/// pace. `says` is every trait's name beside its words, in the order the
/// traits were listed, for the examine panel; a trait with nothing to say
/// is left out of it.
#[derive(Debug, Clone)]
pub struct Folded {
    /// Every capability any of the traits gives; mindless when none gives
    /// any.
    pub wits: Wits,
    /// Percent of normal speed.
    pub speed: Option<u32>,
    /// How far it sees, in tiles.
    pub perception: Option<i32>,
    /// Tiles it sees with no light.
    pub dark_sight: Option<i32>,
    /// Percent health at or below which it runs.
    pub flee_at: Option<i32>,
    /// How it notices a quiet subject.
    pub notice: Option<NoticeStats>,
    /// How it hears.
    pub hearing: Option<HearingStats>,
    /// The light it sheds, as the engine draws it.
    pub light: Option<LightSource>,
    /// Every trait's resistances, kind by kind, added together.
    pub resists: Resistances,
    /// Flat armor.
    pub armor: Option<i32>,
    /// Every ability any of the traits knows, each once, in order.
    pub abilities: Vec<AbilityId>,
    /// Every status any of the traits holds, each once, in order.
    pub statuses: Vec<StatusId>,
    /// What every trait's stats do while it holds a status, in the order
    /// the traits were listed.
    pub while_holding: Vec<(StatusId, Vec<(StatId, Op)>)>,
    /// Every trait's triggers, in the order the traits were listed: a
    /// monster's own answers to `hit`, `struck` and `died`.
    pub triggers: Vec<TriggerSpec>,
    /// How far it keeps from an enemy, and how close it lets one get: what
    /// a trait set, or `None` for a kind that closes in as far as it can.
    pub keep_away: Option<KeepAway>,
    /// Each trait with words for the examine panel, by its name, its
    /// words, and whether it is a trick: [`crate::bestiary_lore`]'s look
    /// panel says a plain one's words outright and a trick's only once
    /// `MonsterLore` has shown it, a "?" until then.
    pub says: Vec<(String, String, bool)>,
}

/// Nothing folded: no wits, no values, no lists. By hand rather than
/// derived, because the engine's own default for wits is sapient, the
/// right default for a mind nobody flagged and the wrong identity for a
/// union of what traits give.
impl Default for Folded {
    fn default() -> Self {
        Self {
            wits: Wits::MINDLESS,
            speed: None,
            perception: None,
            dark_sight: None,
            flee_at: None,
            notice: None,
            hearing: None,
            light: None,
            resists: Resistances::new(),
            armor: None,
            abilities: Vec::new(),
            statuses: Vec::new(),
            while_holding: Vec::new(),
            triggers: Vec::new(),
            keep_away: None,
            says: Vec::new(),
        }
    }
}

/// Two traits of one list setting the same single value: `field`, by
/// `first` and then by `second`, in the order they were listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    /// The value both set, as the file writes it: `speed`, `flee_at`.
    pub field: &'static str,
    /// The trait that set it first.
    pub first: String,
    /// The trait that set it again.
    pub second: String,
}

impl fmt::Display for Clash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` is set by both `{}` and `{}`", self.field, self.first, self.second)
    }
}

impl std::error::Error for Clash {}

/// Folds `traits`, ids in `all`, into one set of values, in the order
/// they are listed. Refuses, naming the field and both traits, the first
/// single value two of them set.
pub fn fold(traits: &[TraitId], all: &Registry<TraitDef>) -> Result<Folded, Clash> {
    let mut folded = Folded::default();
    let mut setters = Setters::default();
    for &id in traits {
        let t = all.get(id);
        let by = t.name.as_str();
        setters.set("speed", t.speed, &mut folded.speed, by)?;
        setters.set("perception", t.perception, &mut folded.perception, by)?;
        setters.set("dark_sight", t.dark_sight, &mut folded.dark_sight, by)?;
        setters.set("flee_at", t.flee_at, &mut folded.flee_at, by)?;
        setters.set("notice", t.notice, &mut folded.notice, by)?;
        setters.set("hearing", t.hearing, &mut folded.hearing, by)?;
        setters.set("light", t.light.map(LightSource), &mut folded.light, by)?;
        setters.set("armor", t.armor, &mut folded.armor, by)?;
        setters.set("keep_away", t.keep_away, &mut folded.keep_away, by)?;
        if let Some(wits) = t.wits {
            folded.wits = folded.wits.with(wits);
        }
        for (kind, pct) in &t.resists {
            folded.resists.add(kind.id(), *pct);
        }
        for ability in t.abilities.iter().map(|a| a.id()) {
            if !folded.abilities.contains(&ability) {
                folded.abilities.push(ability);
            }
        }
        for status in t.statuses.iter().map(|s| s.id()) {
            if !folded.statuses.contains(&status) {
                folded.statuses.push(status);
            }
        }
        for (status, changes) in &t.while_holding {
            folded.while_holding.push((status.id(), changes.iter().map(|(stat, op)| (stat.id(), *op)).collect()));
        }
        folded.triggers.extend(t.triggers.iter().cloned());
        if let Some(says) = &t.says {
            folded.says.push((t.name.clone(), says.clone(), t.trick));
        }
    }
    Ok(folded)
}

/// Which trait set each single value so far, by the value's name: what a
/// [`Clash`] names as the first.
#[derive(Default)]
struct Setters(Vec<(&'static str, String)>);

impl Setters {
    /// Puts `value` in `slot` for the trait `by`, when it gives one, or
    /// refuses when another trait already set `field`.
    fn set<T>(&mut self, field: &'static str, value: Option<T>, slot: &mut Option<T>, by: &str) -> Result<(), Clash> {
        let Some(value) = value else { return Ok(()) };
        if let Some((_, first)) = self.0.iter().find(|(set, _)| *set == field) {
            return Err(Clash { field, first: first.clone(), second: by.to_string() });
        }
        self.0.push((field, by.to_string()));
        *slot = Some(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registries and every trait in `traits.ron`, read the way the
    /// game reads them.
    fn shipped() -> (Registries, Registry<TraitDef>) {
        let registries = crate::content::registries();
        let abilities = crate::content::abilities(&registries, &crate::content::effect_kinds());
        let traits = load_defs(&registries, abilities.defs());
        (registries, traits)
    }

    fn ids(all: &Registry<TraitDef>, names: &[&str]) -> Vec<TraitId> {
        names.iter().map(|n| all.expect(n)).collect()
    }

    /// A tripwire over the skeleton's traits, by name, so one is added or
    /// dropped on purpose; and every one has words for the examine panel,
    /// which is what a trait is for: a monster read as its traits is read
    /// off those words.
    #[test]
    fn every_trait_in_the_file_loads_and_every_one_says_what_it_is() {
        let (_, traits) = shipped();
        let skeleton = [
            "mindless",
            "animal",
            "sapient",
            "cowardly",
            "swift",
            "slow",
            "keen-nosed",
            "sees in the dark",
            "deathless",
            "thick-hided",
            "armored",
            "fire-blooded",
            "fungal glow",
            "ember glow",
            "grave light",
            "regenerates",
            "fire breath",
            "drowning grasp",
            "keen-eared",
            "venomous",
            "webbing",
            "spore-burst",
            "brittle",
            "paralysing",
            "withering",
            "blinking",
            "warding",
            "mends allies",
            "firebolt",
            "slowness",
            "burns-badly",
            "luring",
            "wakeful",
            "sleepy",
            "splitting",
            "stealing",
            "shrouded",
            "flitting",
            "fire-trail",
            "reflecting",
            "packing",
            "led",
            "conjuring",
            "raising",
            "draining",
            "war-cry",
            "singing",
            "spit",
            "calling the drowned",
            "spawning",
            "welling",
            "wyrm breath",
            "mercenary",
            "timid",
            "harmless",
        ];
        for name in skeleton {
            let id = traits.id(name).unwrap_or_else(|| panic!("no trait called {name:?}"));
            assert!(traits.get(id).says.as_deref().is_some_and(|s| !s.is_empty()), "{name} says nothing");
        }
        assert_eq!(traits.len(), skeleton.len(), "a trait this list does not name");
    }

    /// Part 3's single-value traits set what their words say: bones
    /// blades and arrows glance off but hammers break (and poison does
    /// nothing, deathless's own), fire undoes a troll's regeneration by
    /// hurting it worse than any blow, a golem's notice is keener than the
    /// house's, and a will-o'-wisp keeps enemies no closer than three
    /// cells and no further than five before it closes.
    #[test]
    fn part_3s_single_value_traits_set_what_their_words_say() {
        let (registries, traits) = shipped();
        let kind = |name: &str| registries.damage_kinds.expect(name);
        let brittle = fold(&ids(&traits, &["brittle"]), &traits).expect("brittle alone sets nothing twice");
        assert_eq!((brittle.resists.get(kind("slash")), brittle.resists.get(kind("pierce")), brittle.resists.get(kind("crush"))), (33, 33, -50));
        let burns_badly = fold(&ids(&traits, &["burns-badly"]), &traits).expect("burns-badly alone");
        assert_eq!(burns_badly.resists.get(kind("fire")), -100, "fire hurts a burns-badly kind twice as hard");
        let wakeful = fold(&ids(&traits, &["wakeful"]), &traits).expect("wakeful alone").notice.expect("wakeful sets notice");
        assert!(wakeful.chance_pct > crate::bestiary::NOTICE.chance_pct, "keener than the house's default: {wakeful:?}");
        let luring = fold(&ids(&traits, &["luring"]), &traits).expect("luring alone").keep_away.expect("luring keeps its distance");
        assert_eq!(luring, KeepAway { within: 5, no_closer: 3 }, "lets an enemy to five cells before closing, and backs off within three");
    }

    #[test]
    fn lists_add_up_across_a_monsters_traits() {
        let (registries, traits) = shipped();
        let kind = |name: &str| registries.damage_kinds.expect(name);
        let folded = fold(&ids(&traits, &["deathless", "fire-blooded"]), &traits).expect("no single value is set twice");
        assert_eq!(folded.resists.get(kind("poison")), 100, "deathless");
        assert_eq!(folded.resists.get(kind("fire")), 100, "fire-blooded");
        assert_eq!(folded.resists.get(kind("cold")), -50, "fire-blooded's weakness");

        let wits = fold(&ids(&traits, &["mindless", "cowardly"]), &traits).expect("wits add up").wits;
        assert!(wits.has(Wits::FLEES), "a coward runs, whatever else its wits are");
        assert!(!wits.has(Wits::OPENS_DOORS), "and gains nothing else from it");

        let breath = fold(&ids(&traits, &["fire breath", "drowning grasp", "regenerates"]), &traits).expect("lists never clash");
        let ability = |trait_name: &str| traits.get(traits.expect(trait_name)).abilities[0].id();
        assert_eq!(breath.abilities, vec![ability("fire breath"), ability("drowning grasp")]);
        assert_eq!(breath.statuses, vec![registries.statuses.expect("regenerating")]);
        let said: Vec<(&str, bool)> = breath.says.iter().map(|(name, _, trick)| (name.as_str(), *trick)).collect();
        assert_eq!(
            said,
            [("fire breath", true), ("drowning grasp", true), ("regenerates", false)],
            "in the order they were listed, and a trait granting an ability is a trick, unlike a passive status held for life"
        );
    }

    /// Review Focus 5: the second setting of one value is refused, naming
    /// the value and both traits, whichever order they were listed in.
    #[test]
    fn two_traits_setting_one_value_are_refused_naming_both_and_the_field() {
        let (_, traits) = shipped();
        let clash = fold(&ids(&traits, &["swift", "slow"]), &traits).expect_err("two speeds");
        assert_eq!(clash, Clash { field: "speed", first: "swift".into(), second: "slow".into() });
        assert_eq!(clash.to_string(), "`speed` is set by both `swift` and `slow`");
        let clash = fold(&ids(&traits, &["cowardly", "animal", "deathless"]), &traits).expect_err("a deathless coward");
        assert_eq!(clash.to_string(), "`flee_at` is set by both `cowardly` and `deathless`");
    }

    /// No single value slips through the fold unguarded: for each one, two
    /// traits that set it are refused naming it.
    #[test]
    fn every_single_value_is_refused_when_two_traits_set_it() {
        let registries = crate::content::registries();
        let abilities = crate::content::abilities(&registries, &crate::content::effect_kinds());
        let names = registries.names().with("ability", abilities.defs());
        let values = [
            ("speed", "speed: 120"),
            ("perception", "perception: 9"),
            ("dark_sight", "dark_sight: 3"),
            ("flee_at", "flee_at: 20"),
            ("notice", "notice: (certain: 2, chance_pct: 30)"),
            ("hearing", "hearing: (threshold: 1)"),
            ("light", "light: (intensity: 200, radius: 3)"),
            ("armor", "armor: 1"),
            ("keep_away", "keep_away: (within: 3, no_closer: 5)"),
        ];
        for (field, value) in values {
            let text = format!(r#"[(name: "one", {value}), (name: "two", {value})]"#);
            let fixture = load(&text, &names).unwrap_or_else(|e| panic!("{field}: {e}"));
            let clash = fold(&ids(&fixture, &["one", "two"]), &fixture).expect_err(field);
            assert_eq!((clash.field, clash.first.as_str(), clash.second.as_str()), (field, "one", "two"));
        }
    }

    #[test]
    fn a_trait_naming_something_that_does_not_exist_is_refused_by_name() {
        let registries = crate::content::registries();
        let names = registries.names();
        let Err(ContentError::Invalid(errors)) = load(r#"[(name: "venomous", resists: [("venom", 50)])]"#, &names) else {
            panic!("a resistance to nothing loaded")
        };
        assert_eq!(errors, vec!["venomous: unknown damage kind \"venom\"".to_string()]);
    }

    /// A trait whose numbers cannot mean anything is refused at load: a
    /// speed of nothing never acts, and a share of health past the whole of
    /// it is a typo.
    #[test]
    fn a_trait_with_a_speed_of_nothing_or_a_threshold_past_the_whole_is_refused() {
        let registries = crate::content::registries();
        let names = registries.names();
        let Err(ContentError::Invalid(errors)) = load(r#"[(name: "frozen", speed: 0), (name: "craven", flee_at: 150)]"#, &names) else {
            panic!("nonsense loaded")
        };
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors[0].starts_with("frozen: "), "{errors:?}");
        assert!(errors[1].starts_with("craven: "), "{errors:?}");
    }
}
