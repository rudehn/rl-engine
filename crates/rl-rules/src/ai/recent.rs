//! A bounded cache that forgets what was asked for longest ago.

use std::collections::BTreeMap;

/// How many fields, and how many floods, are kept. A moving goal is a new
/// key every turn, so without a bound either would grow for as long as
/// nothing changed the map.
pub const FIELD_CACHE: usize = 32;

/// A bounded cache that forgets what was asked for longest ago.
///
/// Starting over when full, which this replaced, threw away the field
/// every hunter was reading along with the thirty-one nobody was, once a
/// turn in a busy fight.
pub struct Recent<K, V> {
    /// Counts every ask, so each entry knows how lately it was wanted.
    asked: u64,
    /// The entries, each with the ask it was last wanted at.
    pub entries: BTreeMap<K, (u64, V)>,
}

impl<K, V> Default for Recent<K, V> {
    fn default() -> Self {
        Self { asked: 0, entries: BTreeMap::new() }
    }
}

impl<K: Ord + Clone, V> Recent<K, V> {
    /// Whether `key` is held, marking it as just asked for if so.
    pub fn touch(&mut self, key: &K) -> bool {
        self.asked += 1;
        match self.entries.get_mut(key) {
            Some((asked, _)) => {
                *asked = self.asked;
                true
            }
            None => false,
        }
    }

    /// Keeps `value`, in place of the entry asked for longest ago when
    /// there is no room.
    pub fn insert(&mut self, key: K, value: V) {
        if self.entries.len() >= FIELD_CACHE
            && let Some(oldest) = self.entries.iter().min_by_key(|(_, (asked, _))| *asked).map(|(k, _)| k.clone())
        {
            self.entries.remove(&oldest);
        }
        self.asked += 1;
        self.entries.insert(key, (self.asked, value));
    }

    /// The value held for `key`, without marking it asked for.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key).map(|(_, v)| v)
    }

    /// Forgets everything.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_cache_forgets_the_entry_asked_for_longest_ago_and_keeps_the_one_in_use() {
        let mut cache: Recent<i32, ()> = Recent::default();
        cache.insert(0, ());
        for i in 1..=(FIELD_CACHE as i32 + 8) {
            cache.insert(i, ());
            assert!(cache.touch(&0));
        }
        assert_eq!(cache.entries.len(), FIELD_CACHE, "never more than the bound");
        assert!(cache.get(&0).is_some(), "the one asked for every time is still held");
        assert!(cache.get(&1).is_none(), "the one asked for once, longest ago, went first");
        assert!(cache.get(&(FIELD_CACHE as i32 + 8)).is_some(), "and the newest is held");
    }

    #[test]
    fn touching_a_key_that_is_not_held_says_so_and_clearing_forgets_everything() {
        let mut cache: Recent<i32, &str> = Recent::default();
        assert!(!cache.touch(&1));
        cache.insert(1, "one");
        assert!(cache.touch(&1));
        assert_eq!(cache.get(&1), Some(&"one"));
        cache.clear();
        assert!(cache.get(&1).is_none());
    }
}
