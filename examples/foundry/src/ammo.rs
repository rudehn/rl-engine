//! What a shot spends, when it spends anything at all.
//!
//! Declared here, ahead of any behaviour, so `gear::spawn_item` can
//! attach [`Ammo`] to the one weapon in the slice that runs on it rather
//! than on heat. Finding a slug in the wielder's bag, taking it, and what
//! an empty bag does to the weapon are Task 7's, in `TurnSet::React`.

use bevy::prelude::*;
use rl_engine::rl_rules::TagId;

/// What one shot of an ammunition-fed weapon spends: the tag its
/// wielder's bag is searched for, one at a time.
///
/// A weapon carries this instead of [`Heat`](crate::heat::Heat), never
/// both; `gear::Armory::load` is the one place that can say so, since it
/// is the only reader of the file that names either.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ammo {
    /// The tag one shot spends, e.g. the tag a box of slugs carries.
    pub tag: TagId,
}
