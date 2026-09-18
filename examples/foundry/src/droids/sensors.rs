//! Ion jams a sensor. Spec section 8.2: an ion hit strips whatever
//! `DarkSight` a target currently reads as for three whole turns, which is
//! what makes ion the answer to a probe's radar as well as to plating.
//!
//! [`Jammed`] is only a clock now: what `DarkSight` an actor gets back once
//! it clears is not this module's to remember, because it is not a fixed
//! fact about the hit. [`sync_dark_sight`] derives it fresh every pass,
//! from whatever the actor is built with and whatever it currently wears,
//! the same pattern `ammo.rs`'s `sync_ammo` uses for a bag rather than
//! reacting to whichever event happens to touch it: an earlier version
//! stored the radius a jam took away and restored exactly that, and a
//! `grant_dark_sight` recomputed the same component from gear alone on
//! every `ItemEvent`; the two fought over the one component the moment a
//! wearer took a jammed helmet off or swapped it for another, since
//! neither knew the other existed.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::gear::WornDarkSight;

use super::NativeDarkSight;

/// How many whole turns an ion hit blinds a sensor for.
const JAM_TURNS: u32 = 3;

/// A sensor an ion hit blinded, and how many whole turns are left before
/// [`unjam_sensors`] clears it. Carries nothing else: what `DarkSight`
/// comes back as, if anything, is [`sync_dark_sight`]'s question to
/// answer fresh once this is gone, not a fact frozen at the moment of the
/// hit.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Jammed {
    /// Whole turns left before it clears.
    pub turns: u32,
}

/// Reacts to every [`DamageDealt`] of the `ion` kind that dealt more than
/// nothing to a target currently reading as sighted (`DarkSight` present):
/// jams it for [`JAM_TURNS`]. A target already [`Jammed`] only has its
/// clock reset to [`JAM_TURNS`] rather than stacking a second jam on top
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
            commands.entity(ev.target).insert(Jammed { turns: JAM_TURNS });
        } else if let Ok(mut jam) = jammed.get_mut(ev.target) {
            jam.turns = JAM_TURNS;
        } else {
            continue;
        }
        afflict.write(Afflict { target: ev.target, status: sensors_down, turns: JAM_TURNS, by: ev.hit.attacker });
    }
}

/// Counts every [`Jammed`] down by one whole turn on each [`TurnEnd`], and
/// removes it once its clock reaches zero. Leaves `DarkSight` alone
/// either way: [`sync_dark_sight`], chained right after this, is what
/// decides whether an unjammed actor gets any back, and at what radius.
pub fn unjam_sensors(mut commands: Commands, mut ends: MessageReader<TurnEnd>, mut jammed: Query<(Entity, &mut Jammed)>) {
    for _ in ends.read() {
        for (entity, mut jam) in &mut jammed {
            jam.turns = jam.turns.saturating_sub(1);
            if jam.turns == 0 {
                commands.entity(entity).remove::<Jammed>();
            }
        }
    }
}

/// An actor `sync_dark_sight` might owe a `DarkSight`: its own radar, what
/// it wears, and whether it is jammed.
type Candidate = (Entity, Option<&'static NativeDarkSight>, Option<&'static Equipped>, Has<Jammed>);
/// Anyone with a source of `DarkSight` to fold together, or a jam to
/// silence it: naming none of the three means nothing here could ever
/// change, so it is left alone.
type CanSeeInTheDark = Or<(With<NativeDarkSight>, With<Equipped>, With<Jammed>)>;

/// Sets every relevant actor's `DarkSight` from the one fact that actually
/// decides it: none while [`Jammed`], otherwise the larger of its
/// [`NativeDarkSight`] and the largest [`WornDarkSight`] among whatever it
/// currently has equipped, removed entirely when that comes to nothing.
///
/// Run every pass rather than kept in step by reacting to whichever event
/// happens to touch gear or a jam, for the reason `ammo.rs`'s `sync_ammo`
/// gives for a bag: equipping, unequipping, an ion hit landing and a jam
/// expiring each once needed their own listener, and a wearer who swapped
/// helmets mid-jam kept the *old* helmet's radius the moment the jam
/// cleared, because nothing had reacted to the swap while it was jammed.
/// Deriving `DarkSight` from the native radius, the bag and the jam
/// directly has no such gap: whatever changed, the very next pass reads
/// all three as they now stand. Idempotent: an actor already reading right
/// is left untouched.
///
/// Chained after [`jam_sensors`] and [`unjam_sensors`] in
/// [`FoundryPlugin`](crate::plugin::FoundryPlugin): the engine's own
/// scheduler can write a `TurnEnd` and deal the very next turn's
/// `DamageDealt` in the same pass, so this must read the jam as that pass
/// leaves it, not as it stood before either system ran.
pub fn sync_dark_sight(mut commands: Commands, candidates: Query<Candidate, CanSeeInTheDark>, worn: Query<&WornDarkSight>, current: Query<&DarkSight>) {
    for (entity, native, equipped, jammed) in &candidates {
        let best = if jammed {
            None
        } else {
            let native = native.map_or(0, |n| n.0);
            let worn_best = equipped.map_or(0, |e| e.0.worn().filter_map(|(_, item)| worn.get(item).ok()).map(|s| s.0).max().unwrap_or(0));
            let best = native.max(worn_best);
            (best > 0).then_some(best)
        };
        if current.get(entity).ok().map(|d| d.0) == best {
            continue;
        }
        match best {
            Some(n) => {
                commands.entity(entity).insert(DarkSight(n));
            }
            None => {
                commands.entity(entity).remove::<DarkSight>();
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
        let item = app.world_mut().spawn(Jammed { turns: 3 }).id();
        app.world_mut().write_message(TurnEnd { turn: 1 });
        app.world_mut().write_message(DamageDealt { target: item, hit: Hit::from_source(None, ion, 1), dealt: 1 });
        app.world_mut().run_schedule(rl_engine::rl_bevy::plugin::Turn);
        assert_eq!(
            app.world().get::<Jammed>(item).unwrap().turns,
            3,
            "FoundryPlugin must run unjam_sensors before jam_sensors: the ending turn counts down the old jam, then the new hit resets it to three"
        );
    }

    #[test]
    fn foundry_plugin_runs_sync_dark_sight_after_jam_sensors_so_a_fresh_hit_blinds_within_the_same_pass() {
        // The race this guards: a monster already reading `DarkSight` from
        // a previous pass takes an ion hit this pass. `sync_dark_sight`
        // must see the `Jammed` `jam_sensors` just inserted, not the
        // unjammed state the actor stood in before this pass began, or a
        // fresh hit would leave it seeing for one more frame than it
        // should.
        let mut app = crate::testing::headless(RunSeed(1));
        app.update();
        app.update();
        let registries = app.world().resource::<Registries>().clone();
        let ion = registries.damage_kinds.expect("ion");
        let target = app.world_mut().spawn((NativeDarkSight(4), DarkSight(4))).id();
        app.world_mut().write_message(DamageDealt { target, hit: Hit::from_source(None, ion, 1), dealt: 1 });
        app.world_mut().run_schedule(rl_engine::rl_bevy::plugin::Turn);
        assert_eq!(
            app.world().get::<DarkSight>(target),
            None,
            "FoundryPlugin must run sync_dark_sight after jam_sensors: a fresh ion hit landing this pass must blind within the same pass"
        );
    }
}
