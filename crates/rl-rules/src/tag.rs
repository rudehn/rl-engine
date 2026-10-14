//! Item tags: what a thing counts as, for whatever asks by kind rather
//! than by name.
//!
//! A tag is a registered name, "weapon", "armor", "slug", and nothing
//! more: an ability asks for a shield on the arm by one, a container's row
//! draws by one, a lock names one its key carries, and a shot spends a
//! thing of one. The names are the game's; the registry is the shape.

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
