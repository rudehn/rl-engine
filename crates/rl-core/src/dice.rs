//! Dice notation as a first-class value.
//!
//! [`DiceRoll`] is the parsed form of `NdS+B` (`"2d6"`, `"1d4+1"`, `"1d3-1"`,
//! or a flat `"3"`). Parsing happens once, at content load, through the
//! `Deserialize` impl, so a typo is a load-time error rather than a silent
//! zero at the point of use.

use std::fmt;
use std::str::FromStr;

use rand::Rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A parsed `NdS+B` dice expression.
///
/// A flat value parses as `num: 0, sides: 0, bonus: value` and rolls as a
/// constant, so "flat or dice" is one type at every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct DiceRoll {
    /// Number of dice. `0` means the expression is a flat bonus.
    pub num: u32,
    /// Faces per die. `0` only ever pairs with `num: 0`.
    pub sides: u32,
    /// Flat modifier added after the dice.
    pub bonus: i32,
}

impl DiceRoll {
    /// A constant with no dice.
    pub const fn flat(value: i32) -> Self {
        Self {
            num: 0,
            sides: 0,
            bonus: value,
        }
    }

    /// `num`d`sides` with no modifier.
    pub const fn new(num: u32, sides: u32) -> Self {
        Self { num, sides, bonus: 0 }
    }

    /// Lowest possible result.
    pub const fn min(self) -> i32 {
        self.num as i32 + self.bonus
    }

    /// Highest possible result.
    pub const fn max(self) -> i32 {
        (self.num * self.sides) as i32 + self.bonus
    }

    /// Mean result.
    pub fn avg(self) -> f32 {
        self.num as f32 * (self.sides as f32 + 1.0) / 2.0 + self.bonus as f32
    }

    /// Rolls on `rng`. A flat expression consumes no randomness, so a
    /// constant never perturbs the caller's stream.
    pub fn roll(self, rng: &mut impl Rng) -> i32 {
        if self.num == 0 || self.sides == 0 {
            return self.bonus;
        }
        let mut total = 0i32;
        for _ in 0..self.num {
            total += rng.random_range(1..=self.sides) as i32;
        }
        total + self.bonus
    }

    /// Rolls, clamped to at least `floor`.
    pub fn roll_at_least(self, rng: &mut impl Rng, floor: i32) -> i32 {
        self.roll(rng).max(floor)
    }
}

/// Why a dice string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiceParseError(pub String);

impl fmt::Display for DiceParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid dice notation {:?} (expected NdS, NdS+B, NdS-B, or a flat number)",
            self.0
        )
    }
}

impl std::error::Error for DiceParseError {}

impl FromStr for DiceRoll {
    type Err = DiceParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || DiceParseError(s.to_string());
        let t = s.trim();
        if t.is_empty() {
            return Err(err());
        }
        let Some((num_s, rest)) = t.split_once(['d', 'D']) else {
            return t.parse::<i32>().map(DiceRoll::flat).map_err(|_| err());
        };
        let num: u32 = num_s.trim().parse().map_err(|_| err())?;
        let (sides_s, bonus) = match rest.rfind(['+', '-']) {
            Some(i) => {
                let (faces, sign_and_mag) = rest.split_at(i);
                let mag: i32 = sign_and_mag[1..].trim().parse().map_err(|_| err())?;
                (faces, if sign_and_mag.starts_with('-') { -mag } else { mag })
            }
            None => (rest, 0),
        };
        let sides: u32 = sides_s.trim().parse().map_err(|_| err())?;
        if num == 0 || sides == 0 {
            return Err(err());
        }
        Ok(DiceRoll { num, sides, bonus })
    }
}

impl fmt::Display for DiceRoll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.num == 0 || self.sides == 0 {
            return write!(f, "{}", self.bonus);
        }
        write!(f, "{}d{}", self.num, self.sides)?;
        match self.bonus.cmp(&0) {
            std::cmp::Ordering::Greater => write!(f, "+{}", self.bonus),
            std::cmp::Ordering::Less => write!(f, "-{}", -self.bonus),
            std::cmp::Ordering::Equal => Ok(()),
        }
    }
}

impl<'de> Deserialize<'de> for DiceRoll {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl Serialize for DiceRoll {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn parses_every_authored_shape() {
        assert_eq!("2d6".parse::<DiceRoll>().unwrap(), DiceRoll::new(2, 6));
        assert_eq!("1d4+1".parse::<DiceRoll>().unwrap(), DiceRoll { num: 1, sides: 4, bonus: 1 });
        assert_eq!("1d3-1".parse::<DiceRoll>().unwrap(), DiceRoll { num: 1, sides: 3, bonus: -1 });
        assert_eq!("3".parse::<DiceRoll>().unwrap(), DiceRoll::flat(3));
        assert_eq!(" 1 D 4 + 2 ".parse::<DiceRoll>().unwrap(), DiceRoll { num: 1, sides: 4, bonus: 2 });
    }

    #[test]
    fn rejects_malformed_notation() {
        for bad in ["", "d6", "2d", "xdy", "2d6+", "0d6", "2d0", "1d4+x"] {
            assert!(bad.parse::<DiceRoll>().is_err(), "{bad:?} must not parse");
        }
    }

    #[test]
    fn round_trips_through_notation() {
        for s in ["2d6", "1d4+1", "1d3-1", "3", "-2"] {
            let d: DiceRoll = s.parse().unwrap();
            assert_eq!(d.to_string(), s);
        }
    }

    #[test]
    fn bounds_bracket_the_average_and_rolls_stay_inside() {
        let mut rng = StdRng::seed_from_u64(1);
        for s in ["1d2", "1d4+1", "2d6", "1d3-1", "4"] {
            let d: DiceRoll = s.parse().unwrap();
            assert!(d.min() as f32 <= d.avg() && d.avg() <= d.max() as f32);
            for _ in 0..200 {
                let r = d.roll(&mut rng);
                assert!(r >= d.min() && r <= d.max(), "{s} rolled {r}");
            }
        }
        assert_eq!("1d4+1".parse::<DiceRoll>().unwrap().avg(), 3.5);
    }

    #[test]
    fn flat_rolls_consume_no_randomness() {
        let mut a = StdRng::seed_from_u64(7);
        let mut b = StdRng::seed_from_u64(7);
        assert_eq!(DiceRoll::flat(3).roll(&mut a), 3);
        assert_eq!(a.random::<u64>(), b.random::<u64>());
    }

    #[test]
    fn roll_at_least_floors_low_rolls() {
        let mut rng = StdRng::seed_from_u64(3);
        let d: DiceRoll = "1d3-5".parse().unwrap();
        for _ in 0..100 {
            assert!(d.roll_at_least(&mut rng, 1) >= 1);
        }
    }

    #[test]
    fn deserializes_from_ron_string() {
        let d: DiceRoll = ron::from_str(r#""1d4+1""#).unwrap();
        assert_eq!(d, DiceRoll { num: 1, sides: 4, bonus: 1 });
        assert!(ron::from_str::<DiceRoll>(r#""nonsense""#).is_err());
        assert_eq!(ron::to_string(&d).unwrap(), r#""1d4+1""#);
    }
}
