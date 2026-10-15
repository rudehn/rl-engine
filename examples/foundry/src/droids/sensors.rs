//! Ion jams a sensor. Spec section 8.2: an ion hit strips whatever
//! `DarkSight` a target currently reads as for three whole turns, which is
//! what makes ion the answer to a probe's radar as well as to plating.
//!
//! [`Jammed`] is only a clock and the engine's `DarkSightSuppressed` beside
//! it: what `DarkSight` an actor gets back once it clears is not this
//! module's to remember, because it is not a fixed fact about the hit. The
//! engine folds it fresh every pass from the actor's own radar and whatever
//! it wears now: an earlier version stored the radius a jam took away and
//! restored exactly that, and a `grant_dark_sight` recomputed the same
//! component from gear alone on every `ItemEvent`; the two fought over the
//! one component the moment a wearer took a jammed helmet off or swapped
//! it for another, since neither knew the other existed.

use bevy::prelude::*;
use rl_engine::prelude::*;

/// How many whole turns an ion hit blinds a sensor for.
const JAM_TURNS: u32 = 3;

/// A sensor an ion hit blinded, and how many whole turns are left before
/// [`unjam_sensors`] clears it, with the engine's `DarkSightSuppressed`
/// beside it for as long. Carries nothing else: what `DarkSight` comes
/// back as, if anything, is the engine's fold to answer fresh once this is
/// gone, not a fact frozen at the moment of the hit.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Jammed {
    /// Whole turns left before it clears.
    pub turns: u32,
}

/// Reacts to every [`DamageDealt`] of the `ion` kind that dealt more than
/// nothing to a target currently reading as sighted (`DarkSight` present):
/// jams it for `JAM_TURNS`. A target already [`Jammed`] only has its
/// clock reset to `JAM_TURNS` rather than stacking a second jam on top
/// of the first. Also inflicts `sensors down` so a badge shows it.
///
/// A hit that finds neither a current `DarkSight` nor an existing `Jammed`
/// has nothing to jam and is skipped: a coolant rat has no sensor for ion
/// to take.
pub fn jam_sensors(
    mut commands: Commands,
    mut dealt: MessageReader<DamageDealt>,
    registries: Res<Registries>,
    sighted: Query<(), With<DarkSight>>,
    mut jammed: Query<&mut Jammed>,
    mut afflict: MessageWriter<Afflict>,
) {
    let ion = registries.damage_kinds.expect("ion");
    let sensors_down = registries.statuses.expect("sensors down");
    for ev in dealt.read() {
        if ev.hit.kind != ion || ev.dealt <= 0 {
            continue;
        }
        if sighted.contains(ev.target) {
            commands.entity(ev.target).insert((Jammed { turns: JAM_TURNS }, DarkSightSuppressed));
        } else if let Ok(mut jam) = jammed.get_mut(ev.target) {
            jam.turns = JAM_TURNS;
        } else {
            continue;
        }
        afflict.write(Afflict { target: ev.target, status: sensors_down, turns: JAM_TURNS, by: ev.hit.attacker, held_by: None });
    }
}

/// Counts every [`Jammed`] down by one whole turn on each [`TurnEnd`], and
/// removes it, and the suppression with it, once its clock reaches zero.
/// Leaves `DarkSight` alone either way: the engine's fold, after this, is
/// what decides whether an unjammed actor gets any back, and at what
/// radius.
pub fn unjam_sensors(mut commands: Commands, mut ends: MessageReader<TurnEnd>, mut jammed: Query<(Entity, &mut Jammed)>) {
    for _ in ends.read() {
        for (entity, mut jam) in &mut jammed {
            jam.turns = jam.turns.saturating_sub(1);
            if jam.turns == 0 {
                commands.entity(entity).remove::<(Jammed, DarkSightSuppressed)>();
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
        let item = app.world_mut().spawn(Jammed { turns: 3 }).id();
        app.world_mut().write_message(TurnEnd { turn: 1 });
        app.world_mut().write_message(DamageDealt { target: item, hit: Hit::from_source(None, ion, 1), dealt: 1, lost: 1, reach: Reach::default() });
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
        let item = app.world_mut().spawn(Jammed { turns: 3 }).id();
        app.world_mut().write_message(TurnEnd { turn: 1 });
        app.world_mut().write_message(DamageDealt { target: item, hit: Hit::from_source(None, ion, 1), dealt: 1, lost: 1, reach: Reach::default() });
        app.world_mut().run_schedule(rl_engine::rl_bevy::plugin::Turn);
        assert_eq!(
            app.world().get::<Jammed>(item).unwrap().turns,
            3,
            "FoundryPlugin must run unjam_sensors before jam_sensors: the ending turn counts down the old jam, then the new hit resets it to three"
        );
    }

    #[test]
    fn foundry_plugin_folds_dark_sight_after_jam_sensors_so_a_fresh_hit_blinds_within_the_same_pass() {
        // The race this guards: a monster already reading `DarkSight` from
        // a previous pass takes an ion hit this pass. The engine's fold
        // must see the suppression `jam_sensors` just put on, not the
        // unjammed state the actor stood in before this pass began, or a
        // fresh hit would leave it seeing for one more frame than it
        // should.
        let mut app = crate::testing::headless(RunSeed(1));
        app.update();
        app.update();
        let registries = app.world().resource::<Registries>().clone();
        let ion = registries.damage_kinds.expect("ion");
        let target = app.world_mut().spawn((OwnDarkSight(4), DarkSight(4))).id();
        app.world_mut().write_message(DamageDealt { target, hit: Hit::from_source(None, ion, 1), dealt: 1, lost: 1, reach: Reach::default() });
        app.world_mut().run_schedule(rl_engine::rl_bevy::plugin::Turn);
        assert_eq!(
            app.world().get::<DarkSight>(target),
            None,
            "FoundryPlugin must run jam_sensors before the engine folds dark sight: a fresh ion hit landing this pass must blind within the same pass"
        );
    }
}
