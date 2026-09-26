//! Whether an attack lands: the odds, and the one roll that settles them.
//!
//! The engine gathers the facts, who is attacking, how far, in what light,
//! with what accuracy against what evasion, into a [`Shot`], and a
//! [`HitModel`] the game chose turns them into [`Odds`]: some number of
//! chances in some number of outcomes, and the labelled [`Line`]s that
//! produced them. Every model reduces to that, a percent roll, a d20
//! against a target number, two dice against each other, so a panel can
//! print a chance and a list of reasons whatever the model is, and the
//! resolver rolls one draw whatever the model is.
//!
//! [`Certain`] is the default and never rolls, so a game that has not
//! chosen accuracy draws nothing and plays as it did. [`Percent`] is the
//! one the engine ships: accuracy less evasion, less [`range_penalty`] past
//! the weapon's effective range, less a penalty for dim or dark light at
//! the target, with [`PercentLabels`] for what each line is called. No
//! floor and no ceiling beyond 0 and 100: a game that wants "never
//! certain, never hopeless" writes that into its own model. [`Delivery`]
//! says how an attack travels, which decides whether range and light apply.

use rand::Rng;
use rl_grid::LightBand;

/// How an attack travels, which decides what distance and light mean to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Delivery {
    /// Struck in reach. Distance and light are not read: a blow lands on
    /// the cell beside the attacker, which the adjacency floor always shows.
    Melee,
    /// Fired down a line.
    Shot,
    /// Thrown.
    Thrown,
}

/// One labelled contribution to a chance, in the model's own units:
/// percentage points for [`Percent`], steps of one on a die for a d20
/// model. What a panel prints under the chance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// What the game called it.
    pub label: String,
    /// How much it moved the chance, negative for worse.
    pub value: i32,
}

/// The chance an attack lands: `hits` of `out_of` equally likely outcomes,
/// and what shaped it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Odds {
    /// Outcomes that hit.
    pub hits: u32,
    /// Outcomes there are. Never zero.
    pub out_of: u32,
    /// What moved the chance, in the order the model counted it.
    pub lines: Vec<Line>,
}

impl Odds {
    /// `hits` in `out_of`, with `out_of` at least one and `hits` at most
    /// `out_of`, so a model that miscounts gives a certain hit or miss
    /// rather than a panic in the middle of a turn.
    pub fn new(hits: u32, out_of: u32, lines: Vec<Line>) -> Odds {
        let out_of = out_of.max(1);
        Odds { hits: hits.min(out_of), out_of, lines }
    }

    /// The chance as a whole percent, rounded to nearest, for a panel.
    pub fn percent(&self) -> u32 {
        (self.hits * 100 + self.out_of / 2) / self.out_of
    }

    /// One draw: whether this attack lands.
    pub fn roll(&self, rng: &mut impl Rng) -> bool {
        rng.random_range(0..self.out_of) < self.hits
    }
}

/// The facts a [`HitModel`] reads, plain data gathered by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shot {
    /// How the attack travels.
    pub delivery: Delivery,
    /// Chebyshev distance from attacker to target.
    pub distance: i32,
    /// The furthest distance with no range penalty. Zero for a blow.
    pub effective: i32,
    /// The furthest the attack reaches. One for a blow.
    pub range: i32,
    /// The light at the target.
    pub light: LightBand,
    /// The attacker's accuracy stat, `None` when the game names none.
    pub accuracy: Option<i32>,
    /// The target's evasion stat, `None` when the game names none.
    pub evasion: Option<i32>,
}

/// A way of turning a [`Shot`] into [`Odds`]: the game's choice, held by
/// the Bevy layer's `HitRules`.
pub trait HitModel: Send + Sync {
    /// The odds of `shot` landing, or `None` for an attack that is not
    /// rolled at all and simply lands.
    fn odds(&self, shot: &Shot) -> Option<Odds>;
}

/// What range costs: nothing at or inside `effective`, and `per_tile` for
/// every tile past it.
pub fn range_penalty(distance: i32, effective: i32, per_tile: i32) -> i32 {
    (distance - effective).max(0) * per_tile
}

/// Every attack lands and nothing is drawn: the default, and how every game
/// played before accuracy existed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Certain;

impl HitModel for Certain {
    fn odds(&self, _: &Shot) -> Option<Odds> {
        None
    }
}

/// What [`Percent`] calls each of its lines. English by default, as the
/// narrator's phrasebook is; a game says its own with [`Percent::labelled`].
/// Short, because a line is printed after its value in a box that sits in a
/// rail two dozen cells wide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PercentLabels {
    /// The attacker's accuracy, against 100.
    pub accuracy: String,
    /// The target's evasion.
    pub evasion: String,
    /// The range penalty.
    pub range: String,
    /// Dim light at the target.
    pub dim: String,
    /// Dark at the target, seen only by dark sight.
    pub dark: String,
}

impl Default for PercentLabels {
    fn default() -> Self {
        Self { accuracy: "accuracy".into(), evasion: "evasion".into(), range: "for range".into(), dim: "for dim light".into(), dark: "for darkness".into() }
    }
}

/// A percent roll: accuracy (100 when the game names no stat) less
/// evasion (0 when it names none), less [`range_penalty`], less `dim` or
/// `dark` for the light at the target, clamped to 0..=100. A blow reads
/// neither range nor light.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Percent {
    /// Points lost per tile past effective range.
    pub per_tile: i32,
    /// Points lost for a target in dim light.
    pub dim: i32,
    /// Points lost for a target in the dark.
    pub dark: i32,
    /// What each line is called.
    pub labels: PercentLabels,
}

impl Percent {
    /// The model with these penalties and the English labels.
    pub fn new(per_tile: i32, dim: i32, dark: i32) -> Percent {
        Percent { per_tile, dim, dark, labels: PercentLabels::default() }
    }

    /// The same, with the game's own words for its lines.
    pub fn labelled(mut self, labels: PercentLabels) -> Percent {
        self.labels = labels;
        self
    }
}

impl HitModel for Percent {
    fn odds(&self, shot: &Shot) -> Option<Odds> {
        let mut lines = Vec::new();
        let mut push = |label: &str, value: i32| {
            if value != 0 {
                lines.push(Line { label: label.to_string(), value });
            }
        };
        push(&self.labels.accuracy, shot.accuracy.unwrap_or(100) - 100);
        push(&self.labels.evasion, -shot.evasion.unwrap_or(0));
        if shot.delivery != Delivery::Melee {
            push(&self.labels.range, -range_penalty(shot.distance, shot.effective, self.per_tile));
            match shot.light {
                LightBand::Lit => {}
                LightBand::Dim => push(&self.labels.dim, -self.dim),
                LightBand::Dark => push(&self.labels.dark, -self.dark),
            }
        }
        let total = 100 + lines.iter().map(|l| l.value).sum::<i32>();
        Some(Odds::new(total.clamp(0, 100) as u32, 100, lines))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};

    fn shot(delivery: Delivery, distance: i32, light: LightBand) -> Shot {
        Shot { delivery, distance, effective: 3, range: 9, light, accuracy: None, evasion: None }
    }

    #[test]
    fn range_costs_nothing_inside_effective_and_exactly_per_tile_past_it() {
        for effective in 0..8 {
            for per_tile in 0..10 {
                for distance in 0..20 {
                    let expected = if distance <= effective { 0 } else { (distance - effective) * per_tile };
                    assert_eq!(range_penalty(distance, effective, per_tile), expected);
                }
            }
        }
    }

    #[test]
    fn percent_is_always_a_probability_whatever_it_is_fed() {
        let model = Percent::new(5, 16, 30);
        for accuracy in [None, Some(-50), Some(0), Some(60), Some(100), Some(250)] {
            for evasion in [None, Some(-40), Some(0), Some(35), Some(300)] {
                for distance in 0..30 {
                    for light in [LightBand::Dark, LightBand::Dim, LightBand::Lit] {
                        for delivery in [Delivery::Melee, Delivery::Shot, Delivery::Thrown] {
                            let odds = model.odds(&Shot { accuracy, evasion, ..shot(delivery, distance, light) }).expect("percent always rolls");
                            assert_eq!(odds.out_of, 100);
                            assert!(odds.hits <= 100);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_blow_reads_no_range_and_no_light_and_a_shot_reads_both() {
        let model = Percent::new(5, 16, 30);
        let blow = model.odds(&shot(Delivery::Melee, 1, LightBand::Dark)).unwrap();
        assert_eq!((blow.hits, blow.lines.len()), (100, 0), "a blow at accuracy 100 is certain and says nothing");

        let far = model.odds(&shot(Delivery::Shot, 5, LightBand::Dim)).unwrap();
        assert_eq!(far.hits, 100 - 10 - 16);
        let lines: Vec<(i32, &str)> = far.lines.iter().map(|l| (l.value, l.label.as_str())).collect();
        assert_eq!(lines, vec![(-10, "for range"), (-16, "for dim light")]);
    }

    #[test]
    fn accuracy_and_evasion_are_lines_only_when_named() {
        let model = Percent::new(5, 16, 30);
        let named = model.odds(&Shot { accuracy: Some(80), evasion: Some(10), ..shot(Delivery::Shot, 2, LightBand::Lit) }).unwrap();
        assert_eq!(named.hits, 70);
        let lines: Vec<i32> = named.lines.iter().map(|l| l.value).collect();
        assert_eq!(lines, vec![-20, -10], "accuracy against 100, then evasion");
    }

    #[test]
    fn a_game_names_its_own_lines() {
        let model = Percent::new(5, 16, 30).labelled(PercentLabels { dim: "shadowed".into(), ..PercentLabels::default() });
        let odds = model.odds(&shot(Delivery::Thrown, 2, LightBand::Dim)).unwrap();
        assert_eq!(odds.lines[0].label, "shadowed");
    }

    #[test]
    fn certain_never_rolls() {
        for distance in 0..20 {
            assert!(Certain.odds(&shot(Delivery::Shot, distance, LightBand::Dark)).is_none());
        }
    }

    #[test]
    fn a_roll_hits_about_as_often_as_the_odds_say() {
        for hits in [0u32, 1, 25, 67, 99, 100] {
            let odds = Odds::new(hits, 100, Vec::new());
            let mut rng = StdRng::seed_from_u64(u64::from(hits) + 7);
            let landed = (0..20_000).filter(|_| odds.roll(&mut rng)).count() as f64 / 20_000.0;
            let expected = f64::from(hits) / 100.0;
            assert!((landed - expected).abs() < 0.015, "{hits} in 100 landed {landed}");
        }
    }

    #[test]
    fn percent_rounds_and_never_divides_by_zero() {
        assert_eq!(Odds::new(13, 20, Vec::new()).percent(), 65);
        assert_eq!(Odds::new(1, 3, Vec::new()).percent(), 33);
        assert_eq!(Odds::new(2, 3, Vec::new()).percent(), 67);
        assert_eq!(Odds::new(9, 0, Vec::new()).out_of, 1, "an empty denominator is one, not a panic");
        assert_eq!(Odds::new(9, 4, Vec::new()).hits, 4, "never more hits than outcomes");
    }

    /// A d20 model: d20 + accuracy against 10 + evasion, a natural 1
    /// always missing and a 20 always hitting. Written here to prove the
    /// trait is not a percent trait in disguise.
    struct D20;

    impl HitModel for D20 {
        fn odds(&self, shot: &Shot) -> Option<Odds> {
            let bonus = shot.accuracy.unwrap_or(0) - if shot.light == LightBand::Dim { 2 } else { 0 };
            let needed = 10 + shot.evasion.unwrap_or(0) - bonus;
            let faces = (21 - needed).clamp(1, 19) as u32;
            let mut lines = Vec::new();
            if shot.light == LightBand::Dim {
                lines.push(Line { label: "for dim light".into(), value: -2 });
            }
            Some(Odds::new(faces, 20, lines))
        }
    }

    #[test]
    fn a_d20_model_fits_the_trait_with_its_exact_chance() {
        let odds = D20.odds(&Shot { accuracy: Some(3), evasion: Some(2), ..shot(Delivery::Shot, 4, LightBand::Dim) }).unwrap();
        // Needs 10 + 2 - (3 - 2) = 11 or better: ten faces of twenty.
        assert_eq!((odds.hits, odds.out_of, odds.percent()), (10, 20, 50));
        assert_eq!(odds.lines[0].value, -2);
    }
}
