//! What firing a weapon that runs hot costs it, when it stands still.
//!
//! Declared here, ahead of any behaviour, so `gear::spawn_item` can
//! attach [`Heat`] to a weapon whose entry names one. What a shot does to
//! it, when it locks, and how it cools are Task 6's, in `TurnSet::React`;
//! this module only holds the fields and the constructor that task reads
//! and writes.

use bevy::prelude::*;

/// A weapon's thermal budget: what one shot adds, what a quiet turn
/// sheds, and where it stands right now.
///
/// Lives on the weapon rather than the wielder, since two guns in the
/// same hands run their own budgets, and dropping one leaves its heat
/// behind with it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heat {
    /// Heat one shot or swing adds.
    pub per_shot: u32,
    /// Heat a turn without firing sheds.
    pub vent: u32,
    /// Heat carried right now.
    pub now: u32,
    /// Whether it has locked and not yet cooled all the way to zero.
    pub locked: bool,
    /// Whether it fired this turn.
    pub fired: bool,
}

impl Heat {
    /// A fresh weapon: cold, unlocked, and not yet fired.
    pub fn new(per_shot: u32, vent: u32) -> Self {
        Self { per_shot, vent, now: 0, locked: false, fired: false }
    }
}
