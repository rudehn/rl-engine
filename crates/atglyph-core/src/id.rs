//! Typed content ids and the interner that hands them out.
//!
//! Content is referenced by name in data files and by [`Id<T>`] at runtime.
//! An id is a `u32` with a phantom type, so a tile id cannot be passed where
//! an item id is expected, and there is no string hashing in a hot loop.

use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A dense index into a registry of `T`.
pub struct Id<T> {
    raw: u32,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    /// An id from its raw index. Registries hand these out; games should
    /// not construct them by hand.
    pub const fn from_raw(raw: u32) -> Self {
        Self { raw, _marker: PhantomData }
    }

    /// The raw index.
    pub const fn raw(self) -> u32 {
        self.raw
    }

    /// The raw index as a `usize`, for indexing a `Vec`.
    pub const fn index(self) -> usize {
        self.raw as usize
    }
}

// Manual impls so `Id<T>` is Copy/Eq/Hash/Ord regardless of `T`.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T> Eq for Id<T> {}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.raw.cmp(&other.raw)
    }
}
impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Id<{}>({})", std::any::type_name::<T>().rsplit("::").next().unwrap_or("?"), self.raw)
    }
}
impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.raw)
    }
}
impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        u32::deserialize(d).map(Id::from_raw)
    }
}

/// Assigns dense [`Id<T>`]s to names, first come first served.
///
/// Ordered by name internally so iteration is deterministic.
#[derive(Debug, Clone)]
pub struct Interner<T> {
    by_name: BTreeMap<String, Id<T>>,
    names: Vec<String>,
}

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Self { by_name: BTreeMap::new(), names: Vec::new() }
    }
}

impl<T> Interner<T> {
    /// An empty interner.
    pub fn new() -> Self {
        Self::default()
    }

    /// The id for `name`, assigning a new one if it is unseen.
    pub fn intern(&mut self, name: &str) -> Id<T> {
        if let Some(id) = self.by_name.get(name) {
            return *id;
        }
        let id = Id::from_raw(self.names.len() as u32);
        self.names.push(name.to_string());
        self.by_name.insert(name.to_string(), id);
        id
    }

    /// The id for `name`, if it has been interned.
    pub fn get(&self, name: &str) -> Option<Id<T>> {
        self.by_name.get(name).copied()
    }

    /// The name behind `id`.
    ///
    /// # Panics
    /// Panics if `id` did not come from this interner.
    pub fn name(&self, id: Id<T>) -> &str {
        &self.names[id.index()]
    }

    /// Number of ids handed out.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether nothing has been interned.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Every id in raw order, with its name.
    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &str)> {
        self.names.iter().enumerate().map(|(i, n)| (Id::from_raw(i as u32), n.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tile;
    struct Item;

    #[test]
    fn ids_are_dense_and_stable() {
        let mut names: Interner<Tile> = Interner::new();
        let wall = names.intern("wall");
        let floor = names.intern("floor");
        assert_eq!(wall.raw(), 0);
        assert_eq!(floor.raw(), 1);
        assert_eq!(names.intern("wall"), wall);
        assert_eq!(names.get("floor"), Some(floor));
        assert_eq!(names.get("door"), None);
        assert_eq!(names.name(floor), "floor");
        assert_eq!(names.len(), 2);
        assert_eq!(names.iter().map(|(_, n)| n).collect::<Vec<_>>(), vec!["wall", "floor"]);
    }

    #[test]
    fn ids_of_different_types_do_not_mix() {
        let a: Id<Tile> = Id::from_raw(3);
        let b: Id<Item> = Id::from_raw(3);
        // Would not compile: assert_eq!(a, b);
        assert_eq!(a.raw(), b.raw());
        assert!(format!("{a:?}").contains("Tile"));
    }

    #[test]
    fn ids_are_copy_hash_and_serde() {
        let a: Id<Tile> = Id::from_raw(9);
        let b = a;
        let set: std::collections::HashSet<Id<Tile>> = [a, b].into_iter().collect();
        assert_eq!(set.len(), 1);
        let text = ron::to_string(&a).unwrap();
        let back: Id<Tile> = ron::from_str(&text).unwrap();
        assert_eq!(a, back);
    }
}
