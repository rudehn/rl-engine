//! Gas: which gases there are is a game's content; how any gas spreads,
//! fades and hides what is behind it is the engine's.
//!
//! A [`GasDef`] holds only what the engine acts on: how fast it spreads and
//! fades, the concentration that hides what is behind it, whether fire
//! catches in it, and a status it inflicts on whoever breathes enough of it.
//! Anything else a gas does is the game's.
//!
//! A gas is a [`TileField<u8>`] of concentrations, and [`diffuse`] steps it a
//! turn. Spreading is an exchange with each neighbour, so the densest cell
//! never gains, and every cell holding gas then loses at least one unit, so
//! every cloud clears, however it was shaped and whatever it was put down on.

use rl_core::{Id, Point};
use rl_grid::TileField;
use serde::Deserialize;

use crate::content::{ContentError, Named, Registry};
use crate::names::Names;
use crate::status::StatusId;

/// A kind of gas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GasDef {
    /// The name content refers to it by.
    pub name: String,
    /// Percent of each difference to a neighbour that flows across it in a
    /// turn, shared among eight: a hundred levels a room fastest.
    pub spread: u8,
    /// Percent of what is in a cell lost each turn, and never less than one
    /// unit.
    pub fade: u8,
    /// The concentration at or above which it hides what is behind it.
    pub veils_at: Option<u8>,
    /// Whether fire catches in it, burning it away.
    pub burns: bool,
    /// What breathing enough of it does.
    pub inflicts: Option<Breath>,
}

/// A status for whoever breathes a gas at or above a concentration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Breath {
    /// The concentration it takes.
    pub at: u8,
    /// The status.
    pub status: StatusId,
    /// For how many whole turns.
    pub turns: u32,
}

/// A registered gas id.
pub type GasId = Id<GasDef>;

impl Named for GasDef {
    fn name(&self) -> &str {
        &self.name
    }
}

impl GasDef {
    /// A gas called `name` that spreads at half speed, fades a tenth a turn,
    /// hides nothing, does not burn and does nothing to whoever breathes it.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), spread: 50, fade: 10, veils_at: None, burns: false, inflicts: None }
    }

    /// Builder: sets `spread`.
    pub fn spread(mut self, pct: u8) -> Self {
        self.spread = pct;
        self
    }

    /// Builder: sets `fade`.
    pub fn fade(mut self, pct: u8) -> Self {
        self.fade = pct;
        self
    }

    /// Builder: it hides what is behind it at or above `concentration`.
    pub fn veils_at(mut self, concentration: u8) -> Self {
        self.veils_at = Some(concentration);
        self
    }

    /// Builder: fire catches in it.
    pub fn burns(mut self) -> Self {
        self.burns = true;
        self
    }

    /// Builder: breathing it at or above `at` inflicts `status` for `turns`.
    pub fn inflicts(mut self, at: u8, status: StatusId, turns: u32) -> Self {
        self.inflicts = Some(Breath { at, status, turns });
        self
    }

    /// Whether `concentration` of it hides what is behind it.
    pub fn veils(&self, concentration: u8) -> bool {
        self.veils_at.is_some_and(|at| concentration >= at)
    }

    /// What breathing `concentration` of it inflicts, if that is enough to.
    pub fn breathed(&self, concentration: u8) -> Option<Breath> {
        self.inflicts.filter(|b| concentration > 0 && concentration >= b.at)
    }
}

/// A gas as a content file writes it, before its status is resolved.
#[derive(Deserialize)]
struct Authored {
    name: String,
    #[serde(default = "half")]
    spread: u8,
    #[serde(default = "tenth")]
    fade: u8,
    #[serde(default)]
    veils_at: Option<u8>,
    #[serde(default)]
    burns: bool,
    #[serde(default)]
    inflicts: Option<(u8, String, u32)>,
}

fn half() -> u8 {
    50
}

fn tenth() -> u8 {
    10
}

impl Named for Authored {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Loads gases from RON, every status named resolved through `names`.
///
/// Every field but `name` may be left out:
///
/// - `spread`: percent of each difference to a neighbour that flows across it
///   a turn, 0 to 100; 50 when left out.
/// - `fade`: percent lost from each cell a turn, and at least one unit; 10.
/// - `veils_at`: the concentration, 1 to 255, at or above which it hides what
///   is behind it; hides nothing when left out.
/// - `burns`: whether fire catches in it; false.
/// - `inflicts`: `(at, status, turns)`, a status for whoever breathes `at` or
///   more; nothing.
///
/// Reports every unknown status in the file at once.
///
/// ```
/// use rl_rules::{Names, Registry, StatusDef, gas};
///
/// let statuses = Registry::from_defs(vec![StatusDef::new("choking")]).unwrap();
/// let gases = gas::load(r#"[(name: "smoke", veils_at: Some(90)), (name: "fumes", burns: true, inflicts: Some((60, "choking", 2)))]"#, &Names::new().statuses(&statuses)).unwrap();
/// assert!(gases.get(gases.expect("smoke")).veils(120));
/// assert_eq!(gases.get(gases.expect("fumes")).breathed(80).map(|b| b.turns), Some(2));
/// ```
pub fn load(text: &str, names: &Names<'_>) -> Result<Registry<GasDef>, ContentError> {
    let authored: Registry<Authored> = Registry::from_ron_str(text)?;
    let mut errors = Vec::new();
    let mut defs = Vec::new();
    for (_, a) in authored.iter() {
        let inflicts = a.inflicts.as_ref().and_then(|(at, status, turns)| match names.status(status) {
            Ok(status) => Some(Breath { at: *at, status, turns: *turns }),
            Err(e) => {
                errors.push(format!("{}: {e}", a.name));
                None
            }
        });
        defs.push(GasDef { name: a.name.clone(), spread: a.spread.min(100), fade: a.fade.min(100), veils_at: a.veils_at, burns: a.burns, inflicts });
    }
    if !errors.is_empty() {
        return Err(ContentError::Invalid(errors));
    }
    Registry::from_defs(defs)
}

/// Puts `amount` more gas at `p`, never past the most a cell holds.
pub fn emit(field: &mut TileField<u8>, p: Point, amount: u8) {
    field.update(p, |c| c.saturating_add(amount));
}

/// One turn of `def` spreading and fading over `field`, never into a cell
/// that `holds` says gas cannot be in.
///
/// Each neighbour that can hold gas exchanges a share of the difference with
/// this cell, so gas flows downhill and a cell never rises above its densest
/// neighbour. Then what is left fades by `fade` percent and never by less than
/// one, which is what ends every cloud rather than leaving a thin one forever.
pub fn diffuse(field: &mut TileField<u8>, def: &GasDef, holds: impl Fn(Point) -> bool) {
    if field.is_clear() {
        return;
    }
    let (spread, fade) = (i32::from(def.spread.min(100)), i32::from(def.fade.min(100)));
    field.step(|p, around| {
        if !holds(p) {
            return 0;
        }
        let here = i32::from(around.here());
        let flow: i32 = around.neighbours().filter(|(n, _)| holds(*n)).map(|(_, c)| i32::from(c) - here).sum();
        let mixed = here + flow * spread / 800;
        let left = if mixed <= 0 { 0 } else { mixed - (mixed * fade / 100).max(1) };
        left.clamp(0, 255) as u8
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_core::seed::position_hash;

    fn open(_: Point) -> bool {
        true
    }

    #[test]
    fn a_puff_spreads_to_its_neighbours_its_densest_cell_never_gains_and_it_clears() {
        let def = GasDef::new("reek").spread(80).fade(10);
        let mut field = TileField::new(15, 15);
        let middle = Point::new(7, 7);
        field.set(middle, 200);
        diffuse(&mut field, &def, open);
        assert!(field.get(middle) < 200, "the middle gave some away");
        assert!(field.get(Point::new(8, 7)) > 0 && field.get(Point::new(8, 8)) > 0, "to every neighbour, the corners too");
        let mut densest = *field.cells().iter().max().unwrap();
        for turn in 0..200 {
            if field.is_clear() {
                return;
            }
            diffuse(&mut field, &def, open);
            let now = *field.cells().iter().max().unwrap();
            assert!(now < densest, "turn {turn}: the densest cell went from {densest} to {now}");
            densest = now;
        }
        panic!("the puff never cleared");
    }

    #[test]
    fn a_wall_holds_gas_back() {
        let def = GasDef::new("reek").spread(100).fade(1);
        let wall = |p: Point| p.x == 5;
        let mut field = TileField::new(10, 5);
        field.set(Point::new(2, 2), 255);
        for _ in 0..30 {
            diffuse(&mut field, &def, |p| !wall(p));
            assert!(field.set_cells().all(|(p, _)| p.x < 5), "nothing crossed the wall: {:?}", field.set_cells().collect::<Vec<_>>());
        }
    }

    /// The property the fade exists for, over clouds of every shape: every
    /// one clears, and nothing ever grows denser than the densest cell it
    /// started with.
    #[test]
    fn every_cloud_clears_whatever_its_shape() {
        for seed in 0..32u64 {
            let def = GasDef::new("reek").spread((position_hash(seed, 0, 0) % 101) as u8).fade((position_hash(seed, 1, 0) % 30) as u8);
            let mut field = TileField::new(20, 12);
            for i in 0..12 {
                let h = position_hash(seed, i, 7);
                field.set(Point::new((h % 20) as i32, ((h >> 8) % 12) as i32), (h >> 16) as u8);
            }
            let start = *field.cells().iter().max().unwrap();
            let holds = |p: Point| !position_hash(seed, p.x, p.y).is_multiple_of(7);
            let mut turns = 0;
            while !field.is_clear() {
                diffuse(&mut field, &def, holds);
                assert!(*field.cells().iter().max().unwrap() <= start, "seed {seed}: grew denser than it started");
                turns += 1;
                assert!(turns <= 255, "seed {seed}: still hanging after {turns} turns");
            }
        }
    }

    #[test]
    fn gases_load_by_name_and_an_unknown_status_is_named() {
        use crate::status::StatusDef;
        let statuses = Registry::from_defs(vec![StatusDef::new("choking")]).unwrap();
        let names = Names::new().statuses(&statuses);
        let gases =
            load(r#"[(name: "smoke", spread: 70, veils_at: Some(90)), (name: "fumes", burns: true, inflicts: Some((60, "choking", 2)))]"#, &names).unwrap();
        let smoke = gases.get(gases.expect("smoke"));
        assert_eq!((smoke.spread, smoke.fade, smoke.veils(89), smoke.veils(90)), (70, 10, false, true));
        let fumes = gases.get(gases.expect("fumes"));
        assert!(fumes.burns);
        assert_eq!(fumes.breathed(59), None, "too thin to bite");
        assert_eq!(fumes.breathed(60), Some(Breath { at: 60, status: statuses.expect("choking"), turns: 2 }));

        let err = load(r#"[(name: "fumes", inflicts: Some((60, "chokign", 2)))]"#, &names).unwrap_err().to_string();
        assert!(err.contains("fumes") && err.contains("chokign"), "{err}");
    }
}
