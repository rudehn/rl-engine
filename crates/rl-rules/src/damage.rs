//! Damage: a registered kind, a resistance ladder, and a pipeline of
//! stages the game composes.
//!
//! A [`Hit`] carries the attacker who triggers on-hit effects apart from
//! the one who gets kill credit, so a burn's tick can credit the caster
//! without recursing its riders. Mitigation is a list of [`DamageStage`]s
//! run in order; the engine ships the common ones and a game inserts its
//! own anywhere in the list.

use crate::content::{Named, Registry};
use rl_core::Id;
use serde::{Deserialize, Serialize};

/// A registered kind of damage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageKind {
    /// The name content refers to it by.
    pub name: String,
    /// Whether flat armor applies. Off for poison, fire, and the like.
    #[serde(default = "yes")]
    pub armored: bool,
}

fn yes() -> bool {
    true
}

impl DamageKind {
    /// A kind that armor applies to.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), armored: true }
    }

    /// A kind that ignores armor.
    pub fn unarmored(mut self) -> Self {
        self.armored = false;
        self
    }
}

impl Named for DamageKind {
    fn name(&self) -> &str {
        &self.name
    }
}

/// A registered damage kind id.
pub type DamageKindId = Id<DamageKind>;

/// Resistance per kind, as a percentage of damage removed: 0 is none,
/// 100 immune, negative is vulnerability, above 100 absorbs (heals).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Resistances {
    pct: Vec<i32>,
}

impl Resistances {
    /// No resistances.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the resistance to `kind`.
    pub fn set(&mut self, kind: DamageKindId, pct: i32) {
        let i = kind.index();
        if self.pct.len() <= i {
            self.pct.resize(i + 1, 0);
        }
        self.pct[i] = pct;
    }

    /// Adds to the resistance to `kind`.
    pub fn add(&mut self, kind: DamageKindId, pct: i32) {
        let cur = self.get(kind);
        self.set(kind, cur + pct);
    }

    /// The resistance to `kind`.
    pub fn get(&self, kind: DamageKindId) -> i32 {
        self.pct.get(kind.index()).copied().unwrap_or(0)
    }
}

/// One instance of damage about to be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit<A: Copy> {
    /// Who triggers on-hit riders. `None` for damage over time and splash,
    /// so riders cannot recurse.
    pub attacker: Option<A>,
    /// Who gets kill credit. Usually the attacker; set for damage over time
    /// so experience still lands.
    pub credit: Option<A>,
    /// The kind.
    pub kind: DamageKindId,
    /// Damage before mitigation. Negative mends, and goes down the same
    /// stages, so a resistance to the kind a heal is dealt as scales the
    /// heal and immunity to it means nothing can patch the defender up.
    /// Whoever rolls a blow floors it at zero first: a weapon with a
    /// negative bonus that rolls low has missed, not healed.
    pub amount: i32,
    /// Whether the hit was a critical, for stages that care.
    pub critical: bool,
}

impl<A: Copy> Hit<A> {
    /// A hit by `attacker` who also gets credit.
    pub fn by(attacker: A, kind: DamageKindId, amount: i32) -> Self {
        Self { attacker: Some(attacker), credit: Some(attacker), kind, amount, critical: false }
    }

    /// Damage with no attacker to trigger riders, crediting `credit`.
    pub fn from_source(credit: Option<A>, kind: DamageKindId, amount: i32) -> Self {
        Self { attacker: None, credit, kind, amount, critical: false }
    }
}

/// What a stage knows about the defender.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defender {
    /// Flat damage removed from armored kinds.
    pub armor: i32,
    /// Whether a shield block succeeded this hit, if the game rolled one.
    pub blocked: bool,
}

/// One step of mitigation. The engine's stages are plain values; a game's
/// can be anything that implements this.
pub trait DamageStage<A: Copy> {
    /// Adjusts `amount` for this hit. Returning a negative amount means the
    /// hit heals; returning zero means it was fully stopped.
    fn apply(&self, hit: &Hit<A>, defender: &Defender, resistances: &Resistances, kinds: &Registry<DamageKind>, amount: i32) -> i32;
}

/// Removes the defender's resistance percentage. Immunity stops the hit;
/// over-resistance turns it into healing.
pub struct ApplyResistance;

impl<A: Copy> DamageStage<A> for ApplyResistance {
    fn apply(&self, hit: &Hit<A>, _: &Defender, resistances: &Resistances, _: &Registry<DamageKind>, amount: i32) -> i32 {
        let pct = resistances.get(hit.kind);
        amount - (amount as i64 * pct as i64 / 100) as i32
    }
}

/// Subtracts flat armor from armored kinds, never below zero.
pub struct SubtractArmor;

impl<A: Copy> DamageStage<A> for SubtractArmor {
    fn apply(&self, hit: &Hit<A>, defender: &Defender, _: &Resistances, kinds: &Registry<DamageKind>, amount: i32) -> i32 {
        if amount <= 0 || !kinds.get(hit.kind).armored {
            return amount;
        }
        (amount - defender.armor).max(0)
    }
}

/// A successful block halves what is left.
pub struct HalveIfBlocked;

impl<A: Copy> DamageStage<A> for HalveIfBlocked {
    fn apply(&self, _: &Hit<A>, defender: &Defender, _: &Resistances, _: &Registry<DamageKind>, amount: i32) -> i32 {
        if defender.blocked && amount > 0 { amount / 2 } else { amount }
    }
}

/// Runs `stages` in order over `hit.amount`. The result is what to take
/// from health: positive hurts, negative heals, zero was stopped.
///
/// The amount goes in with its sign. Clamping it here once made every
/// heal a no-op, because a mend is a negative hit; the stages that must
/// not touch a heal, armor and a block, already leave one alone.
pub fn resolve<A: Copy>(hit: &Hit<A>, defender: &Defender, resistances: &Resistances, kinds: &Registry<DamageKind>, stages: &[&dyn DamageStage<A>]) -> i32 {
    let mut amount = hit.amount;
    for stage in stages {
        amount = stage.apply(hit, defender, resistances, kinds, amount);
    }
    amount
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds() -> Registry<DamageKind> {
        Registry::from_defs(vec![DamageKind::new("kinetic"), DamageKind::new("burn").unarmored()]).unwrap()
    }

    /// A game-defined stage: crits ignore armor by doubling after it.
    struct CritDoubles;

    impl DamageStage<u32> for CritDoubles {
        fn apply(&self, hit: &Hit<u32>, _: &Defender, _: &Resistances, _: &Registry<DamageKind>, amount: i32) -> i32 {
            if hit.critical { amount * 2 } else { amount }
        }
    }

    #[test]
    fn the_pipeline_runs_in_order_and_games_insert_stages() {
        let k = kinds();
        let kinetic = k.expect("kinetic");
        let mut r = Resistances::new();
        r.set(kinetic, 50);
        let defender = Defender { armor: 3, blocked: false };
        let hit = Hit::by(1u32, kinetic, 20);
        let plain = resolve(&hit, &defender, &r, &k, &[&ApplyResistance, &SubtractArmor]);
        assert_eq!(plain, 7, "20 halved to 10, minus 3 armor");
        let mut crit = hit;
        crit.critical = true;
        let doubled = resolve(&crit, &defender, &r, &k, &[&ApplyResistance, &SubtractArmor, &CritDoubles]);
        assert_eq!(doubled, 14);
        let blocked = resolve(&hit, &Defender { armor: 3, blocked: true }, &r, &k, &[&ApplyResistance, &SubtractArmor, &HalveIfBlocked]);
        assert_eq!(blocked, 3);
    }

    #[test]
    fn unarmored_kinds_skip_armor_and_immunity_and_absorb_work() {
        let k = kinds();
        let burn = k.expect("burn");
        let mut r = Resistances::new();
        let defender = Defender { armor: 100, blocked: false };
        let hit = Hit::from_source(Some(7u32), burn, 9);
        assert_eq!(resolve(&hit, &defender, &r, &k, &[&ApplyResistance, &SubtractArmor]), 9);
        assert_eq!(hit.attacker, None, "a burn triggers no riders");
        r.set(burn, 100);
        assert_eq!(resolve(&hit, &defender, &r, &k, &[&ApplyResistance, &SubtractArmor]), 0);
        r.set(burn, 150);
        assert_eq!(resolve(&hit, &defender, &r, &k, &[&ApplyResistance, &SubtractArmor]), -4, "absorbs into healing");
        r.add(burn, -200);
        assert_eq!(r.get(burn), -50);
        assert_eq!(resolve(&hit, &defender, &r, &k, &[&ApplyResistance]), 13, "vulnerable takes more");
    }

    #[test]
    fn a_negative_hit_heals_past_armor_and_a_block_and_resistance_scales_it() {
        let k = kinds();
        let kinetic = k.expect("kinetic");
        let mut r = Resistances::new();
        let stages: [&dyn DamageStage<u32>; 3] = [&ApplyResistance, &SubtractArmor, &HalveIfBlocked];
        let heal = Hit::by(1u32, kinetic, -6);
        let braced = Defender { armor: 4, blocked: true };
        assert_eq!(resolve(&heal, &braced, &r, &k, &stages), -6, "armor and a block stop blows, not mending");
        r.set(kinetic, 50);
        assert_eq!(resolve(&heal, &braced, &r, &k, &stages), -3, "half resistant to the kind is half mended");
        r.set(kinetic, 100);
        assert_eq!(resolve(&heal, &braced, &r, &k, &stages), 0, "and immune to it cannot be patched up");
    }

    #[test]
    fn kinds_load_from_ron() {
        let r: Registry<DamageKind> = Registry::from_ron_str(r#"[(name: "slash"), (name: "radiation", armored: false)]"#).unwrap();
        assert!(r.get(r.expect("slash")).armored);
        assert!(!r.get(r.expect("radiation")).armored);
    }
}
