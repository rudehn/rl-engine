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

use atglyph_core::{Point, geometry};
use atglyph_rules::StatId;
use atglyph_rules::accuracy::{Certain, Delivery, HitModel, Odds, Shot};
use bevy::prelude::*;

use crate::combat::{CombatRules, DamageRoute, Loadout, RangedAttack};
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
    pub reach: DamageRoute,
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
    ///
    /// `shift` is added to the attacker's accuracy, and is `0` for the
    /// attack as it truly is, which is what every resolver asks. A panel
    /// asks with another to show the chance as the player understands it,
    /// as a game whose worn things are not yet known has its look and
    /// targeting panels count them at what they seem to lend. Added only
    /// when the game names an accuracy stat: with none there is no accuracy
    /// to move, and a model told "no accuracy" is not told a number.
    pub fn odds(&self, attacker: Entity, target: Entity, attempt: Attempt, shift: i32) -> Option<Odds> {
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
            accuracy: self.stat(attacker, |r| r.accuracy).map(|a| a + shift),
            evasion: self.stat(target, |r| r.evasion),
        };
        self.rules.0.odds(&shot)
    }

    /// The odds of what `attacker` would attack `target` with from where it
    /// stands: a blow when adjacent, a shot while its shot reaches, and
    /// nothing further, which is the rule `resolve_attacks` picks by and
    /// `forecast::Arms::at` states. `shift` is as [`Marksmanship::odds`]
    /// takes it.
    pub fn at_distance(&self, loadout: &Loadout, attacker: Entity, target: Entity, shift: i32) -> Option<Odds> {
        let (from, to) = (self.positions.get(attacker).ok()?.0, self.positions.get(target).ok()?.0);
        let distance = geometry::chebyshev(from, to);
        let (melee, gun) = (loadout.melee(attacker), loadout.ranged(attacker));
        let in_range = gun.is_some_and(|g| distance <= g.range);
        match atglyph_rules::arms::choose_reach(distance <= 1, in_range, melee.is_some(), gun.is_some())? {
            atglyph_rules::arms::Reach::Melee => self.odds(attacker, target, Attempt::Blow, shift),
            atglyph_rules::arms::Reach::Ranged => self.odds(attacker, target, Attempt::Shot(&gun?), shift),
        }
    }

    /// The odds of `user` firing `gun` at `target` with the aim on `cursor`,
    /// as the targeting cursor previews a shot: struck as a blow when the
    /// aim is on a cell beside the user, point blank, and as the shot
    /// further off. `shift` is as [`Marksmanship::odds`] takes it.
    ///
    /// One call for the preview, so a game that shows the chance another
    /// way asks the rule the box asks.
    pub fn fire_odds(&self, user: Entity, target: Entity, cursor: Point, gun: &RangedAttack, shift: i32) -> Option<Odds> {
        if geometry::is_adjacent(self.positions.get(user).ok()?.0, cursor) {
            return self.odds(user, target, Attempt::Blow, shift);
        }
        self.odds(user, target, Attempt::Shot(gun), shift)
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
    use crate::state::EngineState;
    use crate::testing::headless_app;
    use atglyph_core::DiceRoll;
    use atglyph_rules::accuracy::{Odds, Percent};

    /// What a system asked of `Marksmanship` for a shot from the shooter at
    /// the target, read back out of the world.
    #[derive(Resource, Default)]
    struct Asked(Option<Odds>);

    #[derive(Resource, Clone, Copy)]
    struct Pair(Entity, Entity, i32);

    fn ask_shot(marks: Marksmanship, pair: Res<Pair>, guns: Query<&RangedAttack>, mut out: ResMut<Asked>) {
        let gun = guns.get(pair.0).expect("the shooter has a gun");
        out.0 = marks.odds(pair.0, pair.1, Attempt::Shot(gun), pair.2);
    }

    /// A shooter with a gun reaching twelve, three of them without penalty,
    /// and a target `gap` cells east, under `rules` and, with `lighting`,
    /// an ambient of that intensity and nothing else.
    fn range(gap: i32, rules: Option<HitRules>, lighting: Option<u8>) -> App {
        asked(gap, rules, lighting, None, 0)
    }

    /// [`range`], with the shooter's accuracy a stat at `accuracy` when the
    /// game names one, and asked with `shift` on it.
    fn asked(gap: i32, rules: Option<HitRules>, lighting: Option<u8>, accuracy: Option<i32>, shift: i32) -> App {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, CombatPlugin));
        if lighting.is_some() {
            app.add_plugins(crate::lighting::LightingPlugin);
        }
        let start = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        if let Some(rules) = rules {
            app.insert_resource(rules);
        }
        if let Some(base) = accuracy {
            let stats = atglyph_rules::Registry::from_defs(vec![atglyph_rules::StatDef::new("accuracy", base)]).expect("one stat");
            let stat = stats.expect("accuracy");
            app.world_mut().resource_mut::<Registries>().stats = stats;
            app.world_mut().resource_mut::<CombatRules>().accuracy = Some(stat);
        }
        let gun = RangedAttack::new(sides.kind, DiceRoll::flat(1), 12).effective_to(3);
        // The player, since the loaded window, and the light cast over it,
        // follows the player: with none, every tile reads as dark.
        let shooter = app
            .world_mut()
            .spawn((Actor, crate::components::Player, Blocks, Position(start), Health::full(10), gun, MeleeAttack::new(sides.kind, DiceRoll::flat(1))))
            .id();
        app.world_mut().entity_mut(shooter).insert(StatBlock(atglyph_rules::Stats::new()));
        let target = app.world_mut().spawn((Actor, Blocks, Position(start.offset(gap, 0)), Health::full(10))).id();
        app.insert_resource(Pair(shooter, target, shift)).init_resource::<Asked>().add_systems(PostUpdate, ask_shot);
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        // After play begins, since a new run resets lighting to dark.
        if let Some(intensity) = lighting {
            app.world_mut().resource_mut::<crate::lighting::Lighting>().ambient = atglyph_grid::Light::white(intensity);
        }
        app.update();
        app.update();
        app
    }

    /// A shift moves the accuracy a game names, and there is nothing to
    /// move in a game that names none: its chance stays the model's own
    /// "no accuracy", whatever a panel asks with.
    #[test]
    fn a_shift_moves_the_accuracy_only_where_the_game_names_an_accuracy_stat() {
        let hits = |accuracy: Option<i32>, shift: i32| {
            let app = asked(2, Some(HitRules(Box::new(Percent::new(5, 16, 30)))), None, accuracy, shift);
            app.world().resource::<Asked>().0.clone().expect("percent rolls").hits
        };
        assert_eq!((hits(Some(80), 0), hits(Some(80), -15), hits(Some(80), 10)), (80, 65, 90), "the named stat, shifted");
        assert_eq!((hits(None, 0), hits(None, -15)), (100, 100), "no stat named, nothing to shift");
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
        let kind = atglyph_rules::damage::DamageKindId::from_raw(0);
        assert_eq!(RangedAttack::new(kind, DiceRoll::flat(1), 12).effective_range(), 4);
        assert_eq!(RangedAttack::new(kind, DiceRoll::flat(1), 5).effective_range(), 1);
        assert_eq!(RangedAttack::new(kind, DiceRoll::flat(1), 12).effective_to(9).effective_range(), 9);
        assert_eq!(crate::throwing::Throwable::new(6, None).effective_range(), 2);
    }
}
