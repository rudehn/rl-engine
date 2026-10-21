//! Item tags: what a thing counts as, for whatever asks by kind rather
//! than by name.
//!
//! A tag is a registered name, "weapon", "armor", "slug", and nothing
//! more: an ability asks for a shield on the arm by one, a container's row
//! draws by one, a lock names one its key carries, and a shot spends a
//! thing of one. The names are the game's; the registry is the shape.
//! [`carries`] is who has one: a lock and an offer's need both ask it.

use rl_core::Id;
use serde::{Deserialize, Serialize};

use crate::content::Named;

/// A registered item tag: "weapon", "armor", "hat".
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

/// Whether someone carries `tag`: tagged with it themselves, in `own`, or
/// holding a thing whose tags, one slice in `held` each, name it.
///
/// The one rule a lock and an offer that needs a tag are both answered by,
/// so a door and a person never disagree over who carries a key. Their own
/// tags count because that is how a game lets someone pass without giving
/// them a thing that could be dropped: a warden, or a player a quest let
/// in. What is worn is held, so it counts too.
pub fn carries<'a>(tag: TagId, own: &[TagId], held: impl IntoIterator<Item = &'a [TagId]>) -> bool {
    own.contains(&tag) || held.into_iter().any(|tags| tags.contains(&tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: TagId = TagId::from_raw(0);
    const OTHER: TagId = TagId::from_raw(1);

    #[test]
    fn a_tag_is_carried_by_being_tagged_with_it_or_holding_a_thing_tagged_with_it() {
        assert!(carries(KEY, &[KEY], []), "tagged itself, with nothing held");
        assert!(carries(KEY, &[], [&[OTHER][..], &[OTHER, KEY][..]]), "held, on any thing among others");
        assert!(!carries(KEY, &[OTHER], [&[OTHER][..], &[][..]]), "neither it nor anything it holds");
        assert!(!carries(KEY, &[], []), "nothing at all");
    }
}
