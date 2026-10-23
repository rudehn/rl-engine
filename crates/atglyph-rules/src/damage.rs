//! Damage: a registered kind, a resistance ladder, and a pipeline of
//! stages the game composes.
//!
//! A [`Hit`] carries the attacker who triggers on-hit effects apart from
//! the one who gets kill credit, so a burn's tick can credit the caster
//! without recursing its riders. Mitigation is a list of [`DamageStage`]s
//! run in order; the engine ships the common ones and a game inserts its
//! own anywhere in the list.

use crate::ai::Vitals;
use crate::content::{Named, Registry};
use crate::status::StatusId;
use atglyph_core::Id;
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
/// 100 immune, negative is vulnerability, above 100 absorbs: the hit
/// restores the share past a hundred.
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

    /// Adds every one of `other`'s to these, kind by kind: what a wearer
    /// resists is its own and its gear's together.
    pub fn plus(&mut self, other: &Resistances) {
        if self.pct.len() < other.pct.len() {
            self.pct.resize(other.pct.len(), 0);
        }
        for (mine, theirs) in self.pct.iter_mut().zip(&other.pct) {
            *mine += theirs;
        }
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
    /// Damage before mitigation, never below zero: [`resolve`] reads a
    /// negative one as nothing. Mending is not a hit of any kind, and has
    /// its own path to health, so no kind has to be invented for it and no
    /// stage has to know to leave it alone. Whoever rolls a blow floors it
    /// at zero first all the same: a weapon with a negative bonus that
    /// rolls low has missed.
    pub amount: i32,
    /// Whether the hit was a critical, for stages that care.
    pub critical: bool,
    /// The status whose tick this is, when it is one, so a narrator can
    /// say what hurt rather than that something did.
    pub status: Option<StatusId>,
}

impl<A: Copy> Hit<A> {
    /// A hit by `attacker` who also gets credit.
    pub fn by(attacker: A, kind: DamageKindId, amount: i32) -> Self {
        Self { attacker: Some(attacker), credit: Some(attacker), kind, amount, critical: false, status: None }
    }

    /// Damage with no attacker to trigger riders, crediting `credit`.
    pub fn from_source(credit: Option<A>, kind: DamageKindId, amount: i32) -> Self {
        Self { attacker: None, credit, kind, amount, critical: false, status: None }
    }

    /// One tick of `status`, crediting whoever applied it.
    pub fn from_status(credit: Option<A>, status: StatusId, kind: DamageKindId, amount: i32) -> Self {
        Self { status: Some(status), ..Self::from_source(credit, kind, amount) }
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
    /// Adjusts `amount` for this hit. Returning zero means it was fully
    /// stopped; returning a negative amount means the defender absorbed
    /// it, which only a resistance past a hundred does.
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
/// from health: positive hurts, zero was stopped, and negative was
/// absorbed, which is what a resistance past a hundred makes of a hit.
///
/// A hit offered below zero is read as zero. It was once how a mend was
/// written, a negative hit of a kind the game invented for it, and every
/// stage had to know to let one through; a mend now never comes here.
pub fn resolve<A: Copy>(hit: &Hit<A>, defender: &Defender, resistances: &Resistances, kinds: &Registry<DamageKind>, stages: &[&dyn DamageStage<A>]) -> i32 {
    let mut amount = hit.amount.max(0);
    for stage in stages {
        amount = stage.apply(hit, defender, resistances, kinds, amount);
    }
    amount
}

/// What a hit left of the one it landed on.
///
/// `dealt` and `lost` differ on purpose: a blow past the last health is
/// all dealt, for the log and for on-hit riders, but only what was there
/// is lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    /// The health after the hit, below nought for a blow past the last.
    pub current: i32,
    /// What the hit did: its whole amount when it harms, and what it
    /// restored (negative, nothing at full health) when it was absorbed.
    pub dealt: i32,
    /// Health actually gone: what was there, never the overkill.
    pub lost: i32,
    /// Whether health is spent.
    pub died: bool,
}

/// Applies a mitigated `amount` to `health`: negative is a hit absorbed,
/// which restores up to the most and no further.
pub fn apply_hit(health: Vitals, amount: i32) -> Applied {
    let current = (health.current - amount).min(health.max);
    let dealt = if amount < 0 { health.current - current } else { amount };
    Applied { current, dealt, lost: health.current - current.max(0), died: current <= 0 }
}

/// The health after a heal of `amount`, scaled by `mending_pct` percent
/// (100 is as asked), stopped at the most, and never below what it was:
/// a heal does not take health away, however it is scaled.
pub fn heal(health: Vitals, amount: i32, mending_pct: i32) -> i32 {
    let offered = (i64::from(amount.max(0)) * i64::from(mending_pct) / 100) as i32;
    health.current.saturating_add(offered).min(health.max).max(health.current)
}

/// What of `amount` lands after the target's standing and the place it is
/// in: nothing in a `peaceful` place either way, nothing that harms an
/// `invulnerable` one but what it absorbs (negative) is kept.
pub fn mitigate(amount: i32, invulnerable: bool, peaceful: bool) -> i32 {
    if peaceful {
        0
    } else if invulnerable {
        amount.min(0)
    } else {
        amount
    }
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

    /// A hit offered below zero is nothing, not a mend: mending has its own
    /// path to health, and a kind is no longer invented to carry it. What a
    /// resistance past a hundred absorbs is still negative, and armor and a
    /// block leave it alone, since they stop blows.
    #[test]
    fn a_hit_offered_below_zero_is_nothing_and_what_is_absorbed_passes_armor_and_a_block() {
        let k = kinds();
        let kinetic = k.expect("kinetic");
        let mut r = Resistances::new();
        let stages: [&dyn DamageStage<u32>; 3] = [&ApplyResistance, &SubtractArmor, &HalveIfBlocked];
        let braced = Defender { armor: 4, blocked: true };
        assert_eq!(resolve(&Hit::by(1u32, kinetic, -6), &braced, &r, &k, &stages), 0, "a negative hit restores nothing");
        r.set(kinetic, 150);
        assert_eq!(resolve(&Hit::by(1u32, kinetic, 6), &braced, &r, &k, &stages), -3, "half again resistant absorbs half, armor and block aside");
    }

    #[test]
    fn kinds_load_from_ron() {
        let r: Registry<DamageKind> = Registry::from_ron_str(r#"[(name: "slash"), (name: "radiation", armored: false)]"#).unwrap();
        assert!(r.get(r.expect("slash")).armored);
        assert!(!r.get(r.expect("radiation")).armored);
    }

    #[test]
    fn a_hit_takes_health_and_a_blow_past_the_last_reports_all_of_it_dealt_and_only_what_was_left_lost() {
        let v = |current| Vitals { current, max: 10 };
        assert_eq!(apply_hit(v(10), 4), Applied { current: 6, dealt: 4, lost: 4, died: false });
        assert_eq!(apply_hit(v(3), 8), Applied { current: -5, dealt: 8, lost: 3, died: true });
    }

    #[test]
    fn an_absorbed_hit_restores_up_to_full_and_reports_what_it_restored() {
        let v = |current| Vitals { current, max: 10 };
        assert_eq!(apply_hit(v(10), -5), Applied { current: 10, dealt: 0, lost: 0, died: false });
        assert_eq!(apply_hit(v(6), -3), Applied { current: 9, dealt: -3, lost: -3, died: false });
    }

    #[test]
    fn a_heal_is_scaled_by_mending_stopped_at_full_and_never_takes_health_away() {
        let v = |current| Vitals { current, max: 10 };
        assert_eq!(heal(v(5), 4, 100), 9);
        assert_eq!(heal(v(9), 4, 100), 10);
        assert_eq!(heal(v(5), 4, 50), 7);
        assert_eq!(heal(v(5), 4, 0), 5);
        assert_eq!(heal(v(5), -4, 100), 5);
        assert_eq!(heal(Vitals { current: i32::MAX - 1, max: i32::MAX }, i32::MAX, 100), i32::MAX);
    }

    #[test]
    fn an_invulnerable_target_keeps_what_it_absorbs_and_a_peaceful_place_takes_nothing_either_way() {
        assert_eq!(mitigate(7, false, false), 7);
        assert_eq!(mitigate(7, true, false), 0);
        assert_eq!(mitigate(-3, true, false), -3);
        assert_eq!(mitigate(7, false, true), 0);
        assert_eq!(mitigate(-3, false, true), 0);
    }
}
