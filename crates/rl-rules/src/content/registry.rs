//! Named definitions with dense ids.

use std::fmt;

use rl_core::{Id, Interner};
use serde::de::DeserializeOwned;

/// A definition that content files refer to by name.
pub trait Named {
    /// The unique name.
    fn name(&self) -> &str;
}

/// Why content could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentError {
    /// The RON did not parse.
    Parse(String),
    /// Two definitions share a name.
    Duplicate(String),
    /// A validation rule failed; every failure is listed.
    Invalid(Vec<String>),
}

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentError::Parse(e) => write!(f, "content did not parse: {e}"),
            ContentError::Duplicate(n) => write!(f, "two definitions are named {n:?}"),
            ContentError::Invalid(errs) => {
                write!(f, "content failed validation:")?;
                for e in errs {
                    write!(f, "\n  {e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ContentError {}

/// Every definition of one kind, addressable by [`Id<T>`] or by name.
///
/// Ids are dense and issued in file order, so a `Vec` indexed by id is a
/// valid per-definition table.
#[derive(Debug, Clone)]
pub struct Registry<T> {
    defs: Vec<T>,
    names: Interner<T>,
}

impl<T> Default for Registry<T> {
    fn default() -> Self {
        Self { defs: Vec::new(), names: Interner::new() }
    }
}

impl<T: Named> Registry<T> {
    /// A registry from definitions in order. Duplicate names are refused.
    pub fn from_defs(defs: Vec<T>) -> Result<Self, ContentError> {
        let mut r = Self::default();
        for d in defs {
            r.push(d)?;
        }
        Ok(r)
    }

    /// A registry from a RON list of `T`.
    pub fn from_ron_str(text: &str) -> Result<Self, ContentError>
    where
        T: DeserializeOwned,
    {
        let defs: Vec<T> = ron::from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        Self::from_defs(defs)
    }

    /// Adds a definition. Refuses a duplicate name.
    pub fn push(&mut self, def: T) -> Result<Id<T>, ContentError> {
        if self.names.get(def.name()).is_some() {
            return Err(ContentError::Duplicate(def.name().to_string()));
        }
        let id = self.names.intern(def.name());
        self.defs.push(def);
        Ok(id)
    }

    /// Runs `check` over every definition and collects every failure, so a
    /// content file with three typos reports three, not one.
    pub fn validate(&self, check: impl Fn(&T, &Self) -> Result<(), String>) -> Result<(), ContentError> {
        let errors: Vec<String> = self.iter().filter_map(|(_, d)| check(d, self).err().map(|e| format!("{}: {e}", d.name()))).collect();
        if errors.is_empty() { Ok(()) } else { Err(ContentError::Invalid(errors)) }
    }
}

impl<T> Registry<T> {
    /// The definition behind `id`.
    ///
    /// # Panics
    /// Panics if `id` did not come from this registry.
    pub fn get(&self, id: Id<T>) -> &T {
        &self.defs[id.index()]
    }

    /// The definition behind `id`, if any.
    pub fn try_get(&self, id: Id<T>) -> Option<&T> {
        self.defs.get(id.index())
    }

    /// The id registered under `name`.
    pub fn id(&self, name: &str) -> Option<Id<T>> {
        self.names.get(name)
    }

    /// The id registered under `name`.
    ///
    /// # Panics
    /// Panics if there is none. For names the game knows it shipped.
    pub fn expect(&self, name: &str) -> Id<T> {
        self.id(name).unwrap_or_else(|| panic!("no {} named {name:?}", std::any::type_name::<T>()))
    }

    /// The name behind `id`.
    pub fn name(&self, id: Id<T>) -> &str {
        self.names.name(id)
    }

    /// Every definition in id order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.defs.iter().enumerate().map(|(i, d)| (Id::from_raw(i as u32), d))
    }

    /// Number of definitions.
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Monster {
        name: String,
        hp: u32,
        #[serde(default)]
        summons: Option<String>,
    }

    impl Named for Monster {
        fn name(&self) -> &str {
            &self.name
        }
    }

    const RON: &str = r#"[
        (name: "rat", hp: 3),
        (name: "wolf", hp: 12, summons: Some("rat")),
    ]"#;

    #[test]
    fn loads_names_and_ids_in_file_order() {
        let r: Registry<Monster> = Registry::from_ron_str(RON).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r.expect("rat").raw(), 0);
        assert_eq!(r.get(r.expect("wolf")).hp, 12);
        assert_eq!(r.name(r.expect("wolf")), "wolf");
        assert_eq!(r.id("dragon"), None);
    }

    #[test]
    fn duplicates_and_parse_errors_are_refused() {
        let dup = Registry::<Monster>::from_ron_str(r#"[(name: "a", hp: 1), (name: "a", hp: 2)]"#);
        assert_eq!(dup.unwrap_err(), ContentError::Duplicate("a".into()));
        assert!(matches!(Registry::<Monster>::from_ron_str("nonsense"), Err(ContentError::Parse(_))));
    }

    #[test]
    fn validation_lists_every_failure_with_its_name() {
        let r: Registry<Monster> = Registry::from_ron_str(r#"[(name: "a", hp: 0, summons: Some("ghost")), (name: "b", hp: 1), (name: "c", hp: 0)]"#).unwrap();
        let err = r
            .validate(|m, reg| {
                if m.hp == 0 {
                    return Err("hp must be positive".into());
                }
                if let Some(s) = &m.summons
                    && reg.id(s).is_none()
                {
                    return Err(format!("summons unknown {s:?}"));
                }
                Ok(())
            })
            .unwrap_err();
        match err {
            ContentError::Invalid(errs) => {
                assert_eq!(errs.len(), 2);
                assert!(errs[0].starts_with("a: "));
                assert!(errs[1].starts_with("c: "));
            }
            other => panic!("{other}"),
        }
        assert!(r.validate(|_, _| Ok(())).is_ok());
    }
}
