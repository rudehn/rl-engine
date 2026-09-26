//! Whether an attack lands, as the world answers it.
//!
//! [`HitRules`] holds the game's [`HitModel`], [`Certain`] unless the game
//! inserts another, and [`Marksmanship`] is the one place the facts a model
//! reads are gathered: where the two stand, the weapon's reach as an
//! [`Attempt`] names it, the light at the target, and the stats
//! [`CombatRules`] names. The attack resolver, the throw resolver, the
//! targeting cursor and the inspect panel all ask it, so the chance a panel
//! prints is the chance the roll uses.
//!
//! A miss is written as [`Missed`], at the moment the attack would have
//! landed, for a narrator to say.

use bevy::prelude::*;
use rl_core::geometry;
use rl_rules::StatId;
use rl_rules::accuracy::{Certain, Delivery, HitModel, Odds, Shot};

use crate::combat::{CombatRules, Loadout, RangedAttack, Reach};
use crate::components::Position;
use crate::lighting::{Lighting, band_at};
use crate::registries::Registries;
use crate::status::StatBlock;
use crate::throwing::Throwable;

/// The game's hit model. [`Certain`] by default, which rolls nothing and
/// draws nothing, so a game that never chose accuracy plays and replays
/// exactly as it did.
#[derive(Resource)]
pub struct HitRules(pub Box<dyn HitModel>);

impl Default for HitRules {
    fn default() -> Self {
        Self(Box::new(Certain))
    }
}

/// An attack that was rolled and missed.
///
/// Written where a hit would have landed: at once for a blow or an unseen
/// shot, when the flight is seen for a watched one, when the item comes
/// down for a throw. The weapon still fired, so
/// [`Struck`](crate::combat::Struck) and its `fire` moment were written;
/// nothing else was.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missed {
    /// Who attacked.
    pub attacker: Entity,
    /// Who it was at.
    pub target: Entity,
    /// The worn or thrown item it was made with, if any.
    pub with: Option<Entity>,
    /// How it travelled.
    pub reach: Reach,
}

/// What an attack is made with, which decides how it travels and how far.
#[derive(Debug, Clone, Copy)]
pub enum Attempt<'a> {
    /// A blow, with whatever the attacker strikes with.
    Blow,
    /// A shot with this weapon.
    Shot(&'a RangedAttack),
    /// A throw of this.
    Throw(&'a Throwable),
}

/// The facts a hit model reads, and the one call that asks it.
///
/// Holds no [`Loadout`], which reads `Equipped`, so it can sit beside a
/// system that changes what is worn, as the throw resolver does; a caller
/// that already has one passes it to [`Marksmanship::at_distance`].
#[derive(bevy::ecs::system::SystemParam)]
pub struct Marksmanship<'w, 's> {
    rules: Res<'w, HitRules>,
    lighting: Option<Res<'w, Lighting>>,
    combat: Option<Res<'w, CombatRules>>,
    registries: Option<Res<'w, Registries>>,
    positions: Query<'w, 's, &'static Position>,
    stats: Query<'w, 's, &'static StatBlock>,
}

impl Marksmanship<'_, '_> {
    /// The odds of `attacker` landing `attempt` on `target` where the two
    /// stand now, or `None` when the model does not roll it or either is
    /// nowhere. A throw that strikes with nothing is never rolled, since
    /// nothing it does depends on striking.
    pub fn odds(&self, attacker: Entity, target: Entity, attempt: Attempt) -> Option<Odds> {
        let (from, to) = (self.positions.get(attacker).ok()?.0, self.positions.get(target).ok()?.0);
        let (delivery, effective, range) = match attempt {
            Attempt::Blow => (Delivery::Melee, 0, 1),
            Attempt::Shot(gun) => (Delivery::Shot, gun.effective_range(), gun.range),
            Attempt::Throw(thrown) => {
                thrown.strike?;
                (Delivery::Thrown, thrown.effective_range(), thrown.range)
            }
        };
        let shot = Shot {
            delivery,
            distance: geometry::chebyshev(from, to),
            effective,
            range,
            light: band_at(self.lighting.as_deref(), to),
            accuracy: self.stat(attacker, |r| r.accuracy),
            evasion: self.stat(target, |r| r.evasion),
        };
        self.rules.0.odds(&shot)
    }

    /// The odds of what `attacker` would attack `target` with from where it
    /// stands: a blow when adjacent, a shot while its shot reaches, and
    /// nothing further, which is the rule `resolve_attacks` picks by and
    /// `forecast::Arms::at` states.
    pub fn at_distance(&self, loadout: &Loadout, attacker: Entity, target: Entity) -> Option<Odds> {
        let (from, to) = (self.positions.get(attacker).ok()?.0, self.positions.get(target).ok()?.0);
        let distance = geometry::chebyshev(from, to);
        if distance <= 1 {
            loadout.melee(attacker)?;
            return self.odds(attacker, target, Attempt::Blow);
        }
        let gun = loadout.ranged(attacker).filter(|g| distance <= g.range)?;
        self.odds(attacker, target, Attempt::Shot(&gun))
    }

    /// The value of the stat `pick` names on `who`, `None` when the game
    /// names none, so a model can tell "no stat" from "a stat of zero".
    fn stat(&self, who: Entity, pick: impl Fn(&CombatRules) -> Option<StatId>) -> Option<i32> {
        let stat = pick(self.combat.as_deref()?)?;
        let registries = self.registries.as_deref()?;
        Some(self.stats.get(who).map_or(0, |s| s.0.value(stat, &registries.stats)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, Health, MeleeAttack, RangedAttack};
    use crate::components::{Actor, Blocks, Position};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use rl_core::DiceRoll;
    use rl_rules::accuracy::{Odds, Percent};

    /// What a system asked of `Marksmanship` for a shot from the shooter at
    /// the target, read back out of the world.
    #[derive(Resource, Default)]
    struct Asked(Option<Odds>);

    #[derive(Resource, Clone, Copy)]
    struct Pair(Entity, Entity);

    fn ask_shot(marks: Marksmanship, pair: Res<Pair>, guns: Query<&RangedAttack>, mut out: ResMut<Asked>) {
        let gun = guns.get(pair.0).expect("the shooter has a gun");
        out.0 = marks.odds(pair.0, pair.1, Attempt::Shot(gun));
    }

    /// A shooter with a gun reaching twelve, three of them without penalty,
    /// and a target `gap` cells east, under `rules` and, with `lighting`,
    /// an ambient of that intensity and nothing else.
    fn range(gap: i32, rules: Option<HitRules>, lighting: Option<u8>) -> App {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin));
        if lighting.is_some() {
            app.add_plugins(crate::lighting::LightingPlugin);
        }
        let start = crate::testing::surface(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        if let Some(rules) = rules {
            app.insert_resource(rules);
        }
        let gun = RangedAttack::new(sides.kind, DiceRoll::flat(1), 12).effective_to(3);
        // The player, since the loaded window, and the light cast over it,
        // follows the player: with none, every tile reads as dark.
        let shooter = app
            .world_mut()
            .spawn((Actor, crate::components::Player, Blocks, Position(start), Health::full(10), gun, MeleeAttack::new(sides.kind, DiceRoll::flat(1))))
            .id();
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(gap, 0)), Health::full(10))).id();
        app.insert_resource(Pair(shooter, target)).init_resource::<Asked>().add_systems(PostUpdate, ask_shot);
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        // After play begins, since a new run resets lighting to dark.
        if let Some(intensity) = lighting {
            app.world_mut().resource_mut::<crate::lighting::Lighting>().ambient = rl_grid::Light::white(intensity);
        }
        app.update();
        app.update();
        app
    }

    #[test]
    fn with_no_model_chosen_nothing_is_rolled() {
        let app = range(5, None, None);
        assert_eq!(app.world().resource::<Asked>().0, None);
    }

    #[test]
    fn the_facts_reach_the_model_distance_past_effective_and_the_light_at_the_target() {
        let app = range(5, Some(HitRules(Box::new(Percent::new(5, 16, 30)))), Some(40));
        let odds = app.world().resource::<Asked>().0.clone().expect("percent rolls");
        assert_eq!(odds.hits, 100 - 10 - 16, "two tiles past an effective range of three, in dim light");
    }

    #[test]
    fn without_lighting_every_target_is_lit_and_no_light_line_is_given() {
        let app = range(5, Some(HitRules(Box::new(Percent::new(5, 16, 30)))), None);
        let odds = app.world().resource::<Asked>().0.clone().unwrap();
        assert_eq!(odds.hits, 90);
        assert!(odds.lines.iter().all(|l| l.label != "for dim light" && l.label != "for darkness"));
    }

    #[test]
    fn effective_defaults_to_a_third_of_range() {
        let kind = rl_rules::damage::DamageKindId::from_raw(0);
        assert_eq!(RangedAttack::new(kind, DiceRoll::flat(1), 12).effective_range(), 4);
        assert_eq!(RangedAttack::new(kind, DiceRoll::flat(1), 5).effective_range(), 1);
        assert_eq!(RangedAttack::new(kind, DiceRoll::flat(1), 12).effective_to(9).effective_range(), 9);
        assert_eq!(crate::throwing::Throwable::new(6, None).effective_range(), 2);
    }
}
