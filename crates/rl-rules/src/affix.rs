//! Affixes and enchantment: what makes one cutlass different from the
//! next.
//!
//! An item instance carries an enchant level and a few affixes. An affix
//! is a registered definition: which item tags it may roll onto, what it
//! adds to the wearer's stats, and what extra dice it adds to a strike,
//! each as a base plus a term that grows with the level. An enchant rule
//! says what one level buys on the item itself. Everything folds down to
//! the vocabulary the rest of the rules already speak: [`Modifier`]s on
//! registered stats and dice of a registered damage kind. The names, the
//! tags, the stats and the kinds are the game's; the shape is here.

use crate::content::{Named, Registry};
use rand::Rng;
use rl_core::{DiceRoll, Id};
use serde::{Deserialize, Serialize};

use crate::damage::DamageKindId;
use crate::stats::{Modifier, Op, StatId};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scaled {
    /// Which stat.
    pub stat: StatId,
    /// At level zero.
    pub base: i32,
    /// Levels per extra point; zero means the base only.
    #[serde(default)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaledStrike {
    /// What kind of damage.
    pub kind: DamageKindId,
    /// Dice at level zero.
    pub base_dice: u32,
    /// Levels per extra die; zero means the base only.
    #[serde(default)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AffixDef {
    /// The fragment: "Sharp", or "the deep" for "of the deep".
    pub name: String,
    /// Before or after.
    #[serde(default)]
    pub kind: AffixKind,
    /// Tags it may roll onto; any one match is enough.
    pub applies_to: Vec<TagId>,
    /// Stat bonuses while worn.
    #[serde(default)]
    pub grants: Vec<Scaled>,
    /// Extra dice on a strike while wielded.
    #[serde(default)]
    pub strikes: Vec<ScaledStrike>,
    /// Relative chance among eligible affixes.
    #[serde(default = "one")]
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

    /// Every stat modifier this instance grants its wearer, tagged with
    /// `source`, from its affixes and from `rule` at its level.
    pub fn modifiers(&self, defs: &Registry<AffixDef>, rule: &EnhanceRule, source: u64) -> Vec<Modifier> {
        let mut out = Vec::new();
        for a in &self.affixes {
            for s in &defs.get(*a).grants {
                let n = s.at(self.level);
                if n != 0 {
                    out.push(Modifier::new(s.stat, Op::Add(n), source));
                }
            }
        }
        for (stat, n) in rule.bonuses_at(self.level) {
            out.push(Modifier::new(stat, Op::Add(n), source));
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
        let mods = item.modifiers(&affixes, &rule, 42);
        let mut s = Stats::new();
        for m in mods {
            assert_eq!(m.source, 42);
            s.add(m);
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
}
