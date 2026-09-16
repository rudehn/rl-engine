//! Affixes and enchantment: what makes one cutlass different from the
//! next.
//!
//! An item instance carries an enchant level and a few affixes. An affix
//! is a registered definition: which item tags it may roll onto, what it
//! adds to the wearer's stats, and what extra dice it adds to a strike,
//! each as a base plus a term that grows with the level. An enchant rule
//! says what one level buys on the item itself. Everything folds down to
//! the vocabulary the rest of the rules already speak: changes to
//! registered stats, which the layer that folds gear turns into
//! [`Modifier`]s, and dice of a registered damage kind. The names, the
//! tags, the stats and the kinds are the game's; the shape is here.

use crate::content::{ContentError, Named, Registry};
use rand::Rng;
use rl_core::{DiceRoll, Id};
use serde::{Deserialize, Serialize};

use crate::damage::DamageKindId;
use crate::names::Names;
use crate::stats::{Op, StatId};

/// A registered item tag: "weapon", "blade", "armor", "hat". An affix's
/// eligibility is a list of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagDef {
    /// The name content refers to it by.
    pub name: String,
}

impl TagDef {
    /// A tag called `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Named for TagDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered tag id.
pub type TagId = Id<TagDef>;

/// Whether an affix reads before the name or after it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AffixKind {
    /// "Sharp cutlass".
    Prefix,
    /// "Cutlass of the deep".
    #[default]
    Suffix,
}

/// A stat bonus that grows with the enchant level: `base + level /
/// levels_per_point`. The base term matters because `+0` is a reachable
/// state, and an affix that does nothing at `+0` reads as a bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Scaled {
    /// Which stat.
    pub stat: StatId,
    /// At level zero.
    pub base: i32,
    /// Levels per extra point; zero means the base only.
    pub levels_per_point: u32,
}

impl Scaled {
    /// The bonus at `level`.
    pub fn at(&self, level: i32) -> i32 {
        let extra = level.max(0).checked_div(self.levels_per_point as i32).unwrap_or(0);
        self.base + extra
    }
}

/// Extra dice on a strike that grow with the enchant level: `base_dice +
/// level / levels_per_die` dice of `sides`, never fewer than one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaledStrike {
    /// What kind of damage.
    pub kind: DamageKindId,
    /// Dice at level zero.
    pub base_dice: u32,
    /// Levels per extra die; zero means the base only.
    pub levels_per_die: u32,
    /// Faces per die.
    pub sides: u32,
}

impl ScaledStrike {
    /// The roll at `level`.
    pub fn at(&self, level: i32) -> DiceRoll {
        let extra = (level.max(0) as u32).checked_div(self.levels_per_die).unwrap_or(0);
        DiceRoll::new((self.base_dice + extra).max(1), self.sides)
    }
}

/// A registered affix.
///
/// Built by [`load`] from names, or by hand. Not deserialized as it stands,
/// for the reason a status definition is not: its ids index registries a
/// content file cannot see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffixDef {
    /// The fragment: "Sharp", or "the deep" for "of the deep".
    pub name: String,
    /// Before or after.
    pub kind: AffixKind,
    /// Tags it may roll onto; any one match is enough.
    pub applies_to: Vec<TagId>,
    /// Stat bonuses while worn.
    pub grants: Vec<Scaled>,
    /// Extra dice on a strike while wielded.
    pub strikes: Vec<ScaledStrike>,
    /// Relative chance among eligible affixes.
    pub weight: u32,
}

fn one() -> u32 {
    1
}

impl Named for AffixDef {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered affix id.
pub type AffixId = Id<AffixDef>;

impl AffixDef {
    /// Whether an item with `tags` may carry this.
    pub fn applies(&self, tags: &[TagId]) -> bool {
        self.applies_to.iter().any(|t| tags.contains(t))
    }
}

/// An affix as authored, before its names are resolved.
#[derive(Debug, Deserialize)]
struct Authored {
    name: String,
    #[serde(default)]
    kind: AffixKind,
    applies_to: Vec<String>,
    #[serde(default)]
    grants: Vec<(String, i32, u32)>,
    #[serde(default)]
    strikes: Vec<(String, u32, u32, u32)>,
    #[serde(default = "one")]
    weight: u32,
}

impl Named for Authored {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Loads affixes from RON, resolving every tag, stat and damage kind
/// through `names`.
///
/// One entry per affix:
///
/// - `name`: unique; the fragment, so a prefix reads "Sharp cutlass" and a
///   suffix "cutlass of flame".
/// - `kind`: optional, `Prefix` or `Suffix`, the default.
/// - `applies_to`: the item tags it may roll onto; any one match is enough.
/// - `grants`: optional; stat bonuses while worn, each
///   `(stat, base, levels_per_point)`: the bonus at `+0`, and the levels per
///   extra point, zero for none.
/// - `strikes`: optional; extra dice on every hit while wielded, each
///   `(damage kind, base_dice, levels_per_die, sides)`.
/// - `weight`: optional; relative chance among what applies, one by default.
///
/// Reports every unknown name, and every die with no sides, at once.
pub fn load(text: &str, names: &Names<'_>) -> Result<Registry<AffixDef>, ContentError> {
    let authored: Registry<Authored> = Registry::from_ron_str(text)?;
    let mut errors = Vec::new();
    let mut defs = Vec::new();
    for (_, a) in authored.iter() {
        let mut failed = |e: String| errors.push(format!("{}: {e}", a.name));
        let applies_to = a.applies_to.iter().filter_map(|t| names.tag(t).map_err(&mut failed).ok()).collect();
        let grants = a
            .grants
            .iter()
            .filter_map(|(stat, base, per)| names.stat(stat).map_err(&mut failed).ok().map(|stat| Scaled { stat, base: *base, levels_per_point: *per }))
            .collect();
        let mut strikes = Vec::new();
        for (kind, base_dice, per, sides) in &a.strikes {
            if *sides == 0 {
                failed(format!("a strike of {kind:?} rolls dice with no sides"));
            }
            if let Ok(kind) = names.damage_kind(kind).map_err(&mut failed) {
                strikes.push(ScaledStrike { kind, base_dice: *base_dice, levels_per_die: *per, sides: *sides });
            }
        }
        defs.push(AffixDef { name: a.name.clone(), kind: a.kind, applies_to, grants, strikes, weight: a.weight });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

/// What one enchant level buys on the item itself.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EnhanceRule {
    /// Stat points granted per level.
    #[serde(default)]
    pub per_level: Vec<(StatId, i32)>,
    /// Stat points granted every `.0` levels.
    #[serde(default)]
    pub per_k_levels: Vec<(u32, StatId, i32)>,
    /// Flat damage added to the item's own strike per level.
    #[serde(default)]
    pub damage_per_level: i32,
}

impl EnhanceRule {
    /// The stat bonuses at `level`, summed per stat.
    pub fn bonuses_at(&self, level: i32) -> Vec<(StatId, i32)> {
        let n = level.max(0);
        let mut out: Vec<(StatId, i32)> = Vec::new();
        let mut add = |stat: StatId, amount: i32| {
            if amount == 0 {
                return;
            }
            match out.iter_mut().find(|(s, _)| *s == stat) {
                Some((_, a)) => *a += amount,
                None => out.push((stat, amount)),
            }
        };
        for (stat, per) in &self.per_level {
            add(*stat, per * n);
        }
        for (k, stat, per) in &self.per_k_levels {
            if *k > 0 {
                add(*stat, per * (n / *k as i32));
            }
        }
        out
    }

    /// Flat damage on the item's own strike at `level`.
    pub fn damage_bonus_at(&self, level: i32) -> i32 {
        self.damage_per_level * level.max(0)
    }
}

/// The per-instance state of an item: its level and its affixes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Enchanted {
    /// `+N`.
    pub level: i32,
    /// Rolled affixes, at most one of each kind.
    pub affixes: Vec<AffixId>,
}

impl Enchanted {
    /// A plain `+0` item with no affixes.
    pub fn plain() -> Self {
        Self::default()
    }

    /// Every stat change this instance grants its wearer, from its affixes
    /// and from `rule` at its level: what an item bestows while worn.
    ///
    /// Untagged, because the item does not know what will fold it: the
    /// layer that puts these on a wearer's stats tags each with the item.
    pub fn grants(&self, defs: &Registry<AffixDef>, rule: &EnhanceRule) -> Vec<(StatId, Op)> {
        let mut out = Vec::new();
        for a in &self.affixes {
            for s in &defs.get(*a).grants {
                let n = s.at(self.level);
                if n != 0 {
                    out.push((s.stat, Op::Add(n)));
                }
            }
        }
        for (stat, n) in rule.bonuses_at(self.level) {
            out.push((stat, Op::Add(n)));
        }
        out
    }

    /// Every extra roll a strike with this instance carries.
    pub fn strikes(&self, defs: &Registry<AffixDef>) -> Vec<(DamageKindId, DiceRoll)> {
        self.affixes.iter().flat_map(|a| defs.get(*a).strikes.iter().map(|s| (s.kind, s.at(self.level)))).collect()
    }

    /// The item's own strike with the level's flat bonus folded in.
    pub fn strike(&self, base: DiceRoll, rule: &EnhanceRule) -> DiceRoll {
        DiceRoll { bonus: base.bonus + rule.damage_bonus_at(self.level), ..base }
    }

    /// "Sharp cutlass of the deep +2".
    pub fn display_name(&self, base: &str, defs: &Registry<AffixDef>) -> String {
        let mut name = String::new();
        for a in &self.affixes {
            let d = defs.get(*a);
            if d.kind == AffixKind::Prefix {
                name.push_str(&d.name);
                name.push(' ');
            }
        }
        name.push_str(base);
        for a in &self.affixes {
            let d = defs.get(*a);
            if d.kind == AffixKind::Suffix {
                name.push_str(" of ");
                name.push_str(&d.name);
            }
        }
        if self.level != 0 {
            name.push_str(&format!(" {:+}", self.level));
        }
        name
    }
}

/// Rolls up to `count` affixes for an item with `tags`: at most one
/// prefix and one suffix, each drawn by weight from what applies.
pub fn roll_affixes(rng: &mut impl Rng, defs: &Registry<AffixDef>, tags: &[TagId], count: usize) -> Vec<AffixId> {
    let mut out = Vec::new();
    for _ in 0..count.min(2) {
        let taken: Vec<AffixKind> = out.iter().map(|a| defs.get(*a).kind).collect();
        let pool: Vec<(AffixId, u32)> =
            defs.iter().filter(|(_, d)| d.applies(tags) && !taken.contains(&d.kind) && d.weight > 0).map(|(id, d)| (id, d.weight)).collect();
        let total: u32 = pool.iter().map(|(_, w)| w).sum();
        if total == 0 {
            break;
        }
        let mut pick = rng.random_range(0..total);
        for (id, w) in pool {
            if pick < w {
                out.push(id);
                break;
            }
            pick -= w;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::damage::DamageKind;
    use crate::stats::{StatDef, Stats};
    use rand::SeedableRng;

    fn setup() -> (Registry<TagDef>, Registry<StatDef>, Registry<DamageKind>, Registry<AffixDef>) {
        let tags = Registry::from_defs(vec![TagDef::new("weapon"), TagDef::new("armor")]).unwrap();
        let stats = Registry::from_defs(vec![StatDef::new("armor", 0), StatDef::new("attack", 0)]).unwrap();
        let kinds = Registry::from_defs(vec![DamageKind::new("cutlass"), DamageKind::new("fire")]).unwrap();
        let affixes = Registry::from_defs(vec![
            AffixDef {
                name: "Sharp".into(),
                kind: AffixKind::Prefix,
                applies_to: vec![tags.expect("weapon")],
                grants: vec![Scaled { stat: stats.expect("attack"), base: 1, levels_per_point: 2 }],
                strikes: vec![],
                weight: 3,
            },
            AffixDef {
                name: "flame".into(),
                kind: AffixKind::Suffix,
                applies_to: vec![tags.expect("weapon")],
                grants: vec![],
                strikes: vec![ScaledStrike { kind: kinds.expect("fire"), base_dice: 1, levels_per_die: 3, sides: 4 }],
                weight: 1,
            },
            AffixDef {
                name: "Stout".into(),
                kind: AffixKind::Prefix,
                applies_to: vec![tags.expect("armor")],
                grants: vec![Scaled { stat: stats.expect("armor"), base: 1, levels_per_point: 0 }],
                strikes: vec![],
                weight: 1,
            },
        ])
        .unwrap();
        (tags, stats, kinds, affixes)
    }

    #[test]
    fn scaled_terms_grow_with_the_level_and_never_vanish() {
        let attack = StatId::from_raw(1);
        let s = Scaled { stat: attack, base: 1, levels_per_point: 2 };
        assert_eq!((s.at(0), s.at(1), s.at(2), s.at(5)), (1, 1, 2, 3));
        let f = ScaledStrike { kind: DamageKindId::from_raw(1), base_dice: 1, levels_per_die: 3, sides: 4 };
        assert_eq!(f.at(0), DiceRoll::new(1, 4));
        assert_eq!(f.at(9), DiceRoll::new(4, 4));
        let zero = ScaledStrike { kind: DamageKindId::from_raw(1), base_dice: 0, levels_per_die: 3, sides: 6 };
        assert_eq!(zero.at(0), DiceRoll::new(1, 6), "never 0d6");
    }

    #[test]
    fn an_instance_folds_to_modifiers_strikes_and_a_name() {
        let (_, stats, kinds, affixes) = setup();
        let (armor, attack) = (stats.expect("armor"), stats.expect("attack"));
        let rule = EnhanceRule { per_level: vec![(attack, 1)], per_k_levels: vec![(2, armor, 1)], damage_per_level: 1 };
        let item = Enchanted { level: 3, affixes: vec![affixes.expect("Sharp"), affixes.expect("flame")] };
        let mut s = Stats::new();
        for (stat, op) in item.grants(&affixes, &rule) {
            s.add(crate::stats::Modifier::new(stat, op, crate::stats::Source::Item(42)));
        }
        assert_eq!(s.value(attack, &stats), 2 + 3, "Sharp +2 at level 3, plus one per level");
        assert_eq!(s.value(armor, &stats), 1, "one per two levels");
        assert_eq!(item.strikes(&affixes), vec![(kinds.expect("fire"), DiceRoll::new(2, 4))]);
        assert_eq!(item.strike(DiceRoll::new(1, 6), &rule), DiceRoll { num: 1, sides: 6, bonus: 3 });
        assert_eq!(item.display_name("cutlass", &affixes), "Sharp cutlass of flame +3");
        assert_eq!(Enchanted::plain().display_name("cutlass", &affixes), "cutlass");
    }

    #[test]
    fn rolling_respects_tags_kinds_and_weights() {
        let (tags, _, _, affixes) = setup();
        let weapon = [tags.expect("weapon")];
        let mut rng = rand::rngs::StdRng::seed_from_u64(1);
        let mut sharp = 0;
        for _ in 0..200 {
            let rolled = roll_affixes(&mut rng, &affixes, &weapon, 2);
            assert!(!rolled.is_empty() && rolled.len() <= 2);
            let kinds: Vec<AffixKind> = rolled.iter().map(|a| affixes.get(*a).kind).collect();
            assert!(kinds.len() == 1 || kinds[0] != kinds[1], "one prefix and one suffix at most");
            assert!(!rolled.contains(&affixes.expect("Stout")), "armor affixes do not land on weapons");
            if rolled.contains(&affixes.expect("Sharp")) {
                sharp += 1;
            }
        }
        assert_eq!(sharp, 200, "with two picks the only prefix always lands");
        let mut one = 0;
        for _ in 0..200 {
            if roll_affixes(&mut rng, &affixes, &weapon, 1) == vec![affixes.expect("Sharp")] {
                one += 1;
            }
        }
        assert!((120..=180).contains(&one), "weight 3 of 4: {one}");
        assert!(roll_affixes(&mut rng, &affixes, &[], 2).is_empty());
    }

    #[test]
    fn an_affix_file_names_tags_stats_and_kinds_the_way_the_rest_of_the_content_does() {
        let (tags, stats, kinds, by_hand) = setup();
        let names = Names::new().tags(&tags).stats(&stats).damage_kinds(&kinds);
        let loaded = load(
            r#"#![enable(implicit_some)]
            [
                (name: "Sharp", kind: Prefix, applies_to: ["weapon"], grants: [("attack", 1, 2)], weight: 3),
                (name: "flame", applies_to: ["weapon"], strikes: [("fire", 1, 3, 4)]),
                (name: "Stout", kind: Prefix, applies_to: ["armor"], grants: [("armor", 1, 0)]),
            ]"#,
            &names,
        )
        .expect("the file loads");
        let all = |r: &Registry<AffixDef>| r.iter().map(|(_, d)| d.clone()).collect::<Vec<_>>();
        assert_eq!(all(&loaded), all(&by_hand), "the same three affixes the test builds by hand, suffix and weight one by default");

        let Err(ContentError::Invalid(errs)) = load(r#"[(name: "odd", applies_to: ["hat"], grants: [("luck", 1, 0)], strikes: [("frost", 1, 0, 0)])]"#, &names)
        else {
            panic!("a file of unknown names loaded");
        };
        assert_eq!(errs.len(), 4, "the tag, the stat, the kind and the die with no sides: {errs:#?}");
        assert!(errs.iter().all(|e| e.starts_with("odd: ")), "each names the affix: {errs:#?}");
    }
}
