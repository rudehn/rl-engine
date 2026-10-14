//! Authored parts made components: what a game's own definition wrote in
//! [`rl_rules::parts`], put on the entity it spawns.
//!
//! The engine owns no definition. A game reads its file into its own type,
//! and where a field is one of the engine's parts it hands the part here
//! rather than writing the component itself, so a blow written the same in
//! two games is made the same in both. Using this is optional: a game that
//! inserts `MeleeAttack` by hand gets exactly what it inserted.

use bevy::prelude::*;
use rl_rules::parts::{Melee, Ranged, Throw};

use crate::combat::{MeleeAttack, RangedAttack};
use crate::throwing::Throwable;

/// What something strikes, shoots and is thrown as: the attack parts of
/// one definition, a weapon's or a monster's, with what one blow or shot
/// costs.
///
/// One cost for the blow and the shot alike, because a cost is how quick
/// the thing is in the hand, not a fact about either attack; a throw costs
/// what throwing does. Every part is optional and each absent one inserts
/// nothing, so a monster that only bites is `Attacks { melee, ..default }`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attacks {
    /// The blow, while wielded or, on an actor, its own.
    pub melee: Option<Melee>,
    /// The shot.
    pub ranged: Option<Ranged>,
    /// How it flies when thrown, for an item.
    pub throw: Option<Throw>,
    /// What one blow or shot costs, in hundredths of a step; `None` is an
    /// ordinary turn.
    pub cost: Option<u32>,
}

impl Attacks {
    /// Inserts on `e` the components these parts describe: a
    /// [`MeleeAttack`], a [`RangedAttack`] and a [`Throwable`], each only
    /// where its part is written.
    pub fn insert(&self, e: &mut EntityCommands) {
        if let Some(melee) = &self.melee {
            e.insert(MeleeAttack::from_part(melee, self.cost));
        }
        if let Some(ranged) = &self.ranged {
            e.insert(RangedAttack::from_part(ranged, self.cost));
        }
        if let Some(throw) = &self.throw {
            e.insert(Throwable::from_part(throw));
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;
    use rl_core::DiceRoll;
    use rl_rules::parts::AttackShape;

    use super::*;

    #[test]
    fn each_written_attack_part_becomes_its_component_at_the_cost_given_and_nothing_else_appears() {
        let kind = rl_rules::damage::DamageKindId::from_raw(0);
        let melee = Melee { roll: DiceRoll::new(1, 6), kind: kind.into(), look: None, shape: Some(AttackShape::Sweep), unaware: Some(500) };
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let e = {
            let mut commands = Commands::new(&mut queue, &world);
            let mut e = commands.spawn_empty();
            Attacks { melee: Some(melee), cost: Some(200), ..Default::default() }.insert(&mut e);
            e.id()
        };
        queue.apply(&mut world);
        let blow = world.get::<MeleeAttack>(e).copied().expect("a blow");
        assert_eq!(blow, MeleeAttack::new(kind, DiceRoll::new(1, 6)).costing(200).shaped(AttackShape::Sweep).unaware(500));
        assert!(world.get::<RangedAttack>(e).is_none() && world.get::<Throwable>(e).is_none(), "no part, no component");
    }
}
