//! Ion jams a sensor. Spec section 8.2: an ion hit strips whatever
//! [`DarkSight`] a target carries for three whole turns, which is what
//! makes ion the answer to a probe's radar as well as to plating.
//!
//! [`Jammed`] holds the radius the hit took away, so [`unjam_sensors`] can
//! give back exactly what was there rather than a fixed default; a second
//! ion hit while still jammed only resets the clock, since the radius
//! taken away cannot itself get any darker.

use bevy::prelude::*;
use rl_engine::prelude::*;

/// How many whole turns an ion hit blinds a sensor for.
const JAM_TURNS: u32 = 3;

/// A sensor an ion hit blinded: what [`DarkSight`] it had, and how many
/// whole turns are left before [`unjam_sensors`] gives it back.
///
/// Taking [`DarkSight`] off entirely, rather than zeroing it in place, is
/// what makes a jammed wearer read as sightless to everything that already
/// asks for `DarkSight`, with nothing new for any of them to learn.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Jammed {
    /// The `DarkSight` radius to give back once the jam runs out.
    pub sight: i32,
    /// Whole turns left before it clears.
    pub turns: u32,
}

/// Reacts to every [`DamageDealt`] of the `ion` kind that dealt more than
/// nothing: takes [`DarkSight`] off the target and puts it by as [`Jammed`]
/// for [`JAM_TURNS`], or, on a target already jammed, only resets the
/// clock rather than stacking a second jam on top of the first. Also
/// inflicts `sensors down` so a badge shows it.
///
/// A hit that finds neither `DarkSight` nor an existing `Jammed` has
/// nothing to jam and is skipped: a coolant rat has no sensor for ion to
/// take.
pub fn jam_sensors(
    mut commands: Commands,
    mut dealt: MessageReader<DamageDealt>,
    registries: Res<Registries>,
    sights: Query<&DarkSight>,
    mut jammed: Query<&mut Jammed>,
    mut afflict: MessageWriter<Afflict>,
) {
    let ion = registries.damage_kinds.expect("ion");
    let sensors_down = registries.statuses.expect("sensors down");
    for ev in dealt.read() {
        if ev.hit.kind != ion || ev.dealt <= 0 {
            continue;
        }
        if let Ok(sight) = sights.get(ev.target) {
            commands.entity(ev.target).remove::<DarkSight>().insert(Jammed { sight: sight.0, turns: JAM_TURNS });
        } else if let Ok(mut jam) = jammed.get_mut(ev.target) {
            jam.turns = JAM_TURNS;
        } else {
            continue;
        }
        afflict.write(Afflict { target: ev.target, status: sensors_down, turns: JAM_TURNS, by: ev.hit.attacker });
    }
}

/// Counts every [`Jammed`] down by one whole turn on each [`TurnEnd`], and
/// on the one that empties it, gives its [`DarkSight`] back at the radius
/// it had before the ion hit took it.
///
/// Chained after [`jam_sensors`] in [`FoundryPlugin`](crate::plugin::FoundryPlugin):
/// the engine's own scheduler can write a `TurnEnd` and deal the very next
/// turn's `DamageDealt` in the same pass, so a fresh jam from that new
/// turn must not be counted down before it has stood for even one whole
/// turn of its own.
pub fn unjam_sensors(mut commands: Commands, mut ends: MessageReader<TurnEnd>, mut jammed: Query<(Entity, &mut Jammed)>) {
    for _ in ends.read() {
        for (entity, mut jam) in &mut jammed {
            jam.turns = jam.turns.saturating_sub(1);
            if jam.turns == 0 {
                commands.entity(entity).remove::<Jammed>().insert(DarkSight(jam.sight));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_core::RunSeed;

    use super::*;

    /// Runs `jam_sensors` and `unjam_sensors` in the order `unjam_first`
    /// says, against one target already `Jammed` for three turns that
    /// takes a second ion hit and a `TurnEnd` in the same pass, and
    /// returns the turns left on its `Jammed` afterward.
    fn scenario(unjam_first: bool) -> u32 {
        use bevy::ecs::system::RunSystemOnce;

        let mut app = crate::testing::headless(RunSeed(1));
        let registries = app.world().resource::<Registries>().clone();
        let ion = registries.damage_kinds.expect("ion");
        let item = app.world_mut().spawn(Jammed { sight: 4, turns: 3 }).id();
        app.world_mut().write_message(TurnEnd { turn: 1 });
        app.world_mut().write_message(DamageDealt { target: item, hit: Hit::from_source(None, ion, 1), dealt: 1 });
        if unjam_first {
            app.world_mut().run_system_once(unjam_sensors).unwrap();
            app.world_mut().run_system_once(jam_sensors).unwrap();
        } else {
            app.world_mut().run_system_once(jam_sensors).unwrap();
            app.world_mut().run_system_once(unjam_sensors).unwrap();
        }
        app.world().get::<Jammed>(item).unwrap().turns
    }

    #[test]
    fn jam_sensors_and_unjam_sensors_give_different_turns_left_depending_which_runs_first() {
        assert_eq!(scenario(true), 3, "unjam first: the ending turn counts down the old jam, then the new hit resets it to three");
        assert_eq!(scenario(false), 2, "jam first: the new hit resets to three, then the ending turn wrongly counts the fresh jam down");
    }

    #[test]
    fn foundry_plugin_runs_unjam_sensors_before_jam_sensors_so_a_fresh_hit_keeps_its_first_turn() {
        // The wiring itself, not just the mechanism above: `FoundryPlugin`
        // is what `main.rs` and `testing::headless` both add, so if this
        // reads `Jammed` correctly, the shipped game does too. Forces
        // exactly one `Turn` pass, as `heat.rs`'s own wiring test does, to
        // isolate the race the chain is for.
        let mut app = crate::testing::headless(RunSeed(1));
        app.update();
        app.update();
        let registries = app.world().resource::<Registries>().clone();
        let ion = registries.damage_kinds.expect("ion");
        let item = app.world_mut().spawn(Jammed { sight: 4, turns: 3 }).id();
        app.world_mut().write_message(TurnEnd { turn: 1 });
        app.world_mut().write_message(DamageDealt { target: item, hit: Hit::from_source(None, ion, 1), dealt: 1 });
        app.world_mut().run_schedule(rl_engine::rl_bevy::plugin::Turn);
        assert_eq!(
            app.world().get::<Jammed>(item).unwrap().turns,
            3,
            "FoundryPlugin must run unjam_sensors before jam_sensors: the ending turn counts down the old jam, then the new hit resets it to three"
        );
    }
}
