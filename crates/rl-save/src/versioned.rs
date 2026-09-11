//! The versioning policy: one number, matched exactly, no migrations.
//!
//! A save whose version is not the one this build writes is refused
//! whole, the same as a save that does not parse. Bump the version when
//! the shape changes in a way an old save would parse wrongly into, and
//! say why next to the constant. Migrations are a cost a personal game
//! never recovers; a refused save is a clear message.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::backend::SaveError;

/// A save with its version on the outside, so the version can be read
/// before the rest is trusted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versioned<T> {
    /// The schema version the data was written under.
    pub version: u32,
    /// The save.
    pub data: T,
}

/// Just the version, read first.
#[derive(Deserialize)]
struct Header {
    version: u32,
}

/// Writes `data` as RON under `version`.
pub fn encode<T: Serialize>(version: u32, data: &T) -> Result<String, SaveError> {
    let wrapped = Versioned { version, data };
    ron::ser::to_string_pretty(&wrapped, ron::ser::PrettyConfig::default()).map_err(|e| SaveError::Encode(e.to_string()))
}

/// Reads a save written by [`encode`], refusing any version but
/// `expected` and any text that does not parse.
pub fn decode<T: DeserializeOwned>(expected: u32, text: &str) -> Result<T, SaveError> {
    let header: Header = ron::from_str(text).map_err(|e| SaveError::Decode(e.to_string()))?;
    if header.version != expected {
        return Err(SaveError::Version { found: header.version, expected });
    }
    let wrapped: Versioned<T> = ron::from_str(text).map_err(|e| SaveError::Decode(e.to_string()))?;
    Ok(wrapped.data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Run {
        seed: u64,
        turn: u32,
    }

    #[test]
    fn round_trips_and_refuses_other_versions_and_garbage() {
        let text = encode(3, &Run { seed: 7, turn: 12 }).unwrap();
        assert_eq!(decode::<Run>(3, &text).unwrap(), Run { seed: 7, turn: 12 });
        assert!(matches!(decode::<Run>(4, &text), Err(SaveError::Version { found: 3, expected: 4 })));
        assert!(matches!(decode::<Run>(3, "not ron"), Err(SaveError::Decode(_))));
        assert!(matches!(decode::<Run>(3, "(version: 3, data: (seed: \"x\", turn: 1))"), Err(SaveError::Decode(_))));
    }
}
