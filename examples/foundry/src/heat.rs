//! What firing a weapon that runs hot costs it, when it stands still.
//!
//! A weapon carrying [`Heat`] locks at [`CAPACITY`] and stays locked until
//! it has vented all the way back to zero, so emptying it is a commitment
//! to cool it rather than a pause; it vents only on a turn it did not
//! fire, which is what lets heat build on a fast weapon at all. Locking
//! takes its attack off the item and stows it as [`Stowed`] rather than
//! marking it somehow unusable, which is what makes the engine's own
//! [`Loadout`](rl_engine::rl_bevy::Loadout) fall through to the next
//! worn item with no knowledge of heat at all: a dual-wielded pair of
//! blasters alternates because the locked one simply has nothing to
//! shoot with.
//!
//! [`heat_on_struck`] and [`vent_heat`] are the whole of the behaviour,
//! both reactions to messages the engine already writes: a [`Struck`]
//! naming the item an attack came from, and a whole [`TurnEnd`].

use bevy::prelude::*;
use rl_engine::rl_bevy::{Equipped, MeleeAttack, Player, RangedAttack, Struck, TurnEnd, Turns};
use rl_engine::rl_ui::{Facets, GearView, MessageLog, Tones};

/// How hot a weapon may run before it locks.
pub const CAPACITY: u32 = 100;

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

    /// Records a shot, and answers whether this shot is the one that
    /// locked it.
    pub fn fire(&mut self) -> bool {
        self.now = self.now.saturating_add(self.per_shot);
        self.fired = true;
        let locks = !self.locked && self.now >= CAPACITY;
        self.locked |= locks;
        locks
    }

    /// Ends a turn: vents if it did not fire this turn, and answers
    /// whether this turn is the one that unlocked it.
    ///
    /// Never below zero, and never vents on a turn it fired: a weapon
    /// that only ever gets one shot off between quiet turns never locks,
    /// which is the trade a fast, low-heat weapon makes for never taking
    /// the burst a slow one does. The turn a weapon locks is a turn it
    /// fired, so that turn does not vent either, spec section 6.2's
    /// "vents only on a turn its wielder does not fire it" applying with
    /// no exception for locking: a weapon that locks on turn `T` reaches
    /// zero at the end of turn `T` plus one whole quiet turn per
    /// [`Heat::vent`] of [`CAPACITY`] left to shed.
    pub fn turn_end(&mut self) -> bool {
        if !self.fired {
            self.now = self.now.saturating_sub(self.vent);
        }
        self.fired = false;
        let unlocks = self.locked && self.now == 0;
        if unlocks {
            self.locked = false;
        }
        unlocks
    }
}

/// An attack a locked weapon had, put by until it cools.
///
/// Taking the attack off the item entirely, rather than leaving it in
/// place behind some other flag, is what makes the engine's own loadout
/// pass over a locked weapon for the next one worn: nothing about combat
/// has to learn what heat is.
#[derive(Component, Debug, Clone, Copy)]
pub enum Stowed {
    /// A blow, put by.
    Melee(MeleeAttack),
    /// A shot, put by.
    Ranged(RangedAttack),
}

/// Reacts to every [`Struck`] whose weapon runs hot: records the shot on
/// its [`Heat`], and when that shot locks it, takes its attack off and
/// stows it, so the loadout finds nothing to fire from that hand until it
/// cools. Says so in the log only when the player fired it: a droid's
/// weapon locking is something the player sees in the droid falling
/// silent, not a line to read.
pub fn heat_on_struck(
    mut commands: Commands,
    mut struck: MessageReader<Struck>,
    mut heats: Query<&mut Heat>,
    weapons: Query<(Option<&MeleeAttack>, Option<&RangedAttack>)>,
    mut said: Said,
) {
    for ev in struck.read() {
        let Some(item) = ev.with else { continue };
        let Ok(mut heat) = heats.get_mut(item) else { continue };
        if !heat.fire() {
            continue;
        }
        let mut e = commands.entity(item);
        if let Ok((melee, ranged)) = weapons.get(item) {
            if let Some(attack) = melee {
                e.remove::<MeleeAttack>().insert(Stowed::Melee(*attack));
            } else if let Some(attack) = ranged {
                e.remove::<RangedAttack>().insert(Stowed::Ranged(*attack));
            }
        }
        if said.player.contains(ev.attacker) {
            let now = said.turns.turn_number();
            let line = format!("Your {} overheats and locks.", said.name(item));
            said.log.bad(line, now);
        }
    }
}

/// Reacts to every whole [`TurnEnd`]: vents every [`Heat`], and for one
/// that unlocks, takes its [`Stowed`] attack back off the shelf and gives
/// it back to the item. As with locking, only the player's own weapon
/// says so in the log.
pub fn vent_heat(mut commands: Commands, mut ends: MessageReader<TurnEnd>, mut heats: Query<(Entity, &mut Heat, Option<&Stowed>)>, mut said: Said) {
    for ev in ends.read() {
        for (entity, mut heat, stowed) in &mut heats {
            if !heat.turn_end() {
                continue;
            }
            match stowed {
                Some(Stowed::Melee(attack)) => {
                    commands.entity(entity).remove::<Stowed>().insert(*attack);
                }
                Some(Stowed::Ranged(attack)) => {
                    commands.entity(entity).remove::<Stowed>().insert(*attack);
                }
                None => {}
            }
            if said.worn.iter().any(|e| e.0.worn().any(|(_, item)| item == entity)) {
                let line = format!("Your {} cools and unlocks.", said.name(entity));
                said.log.notice(line, ev.turn);
            }
        }
    }
}

/// What the heat systems need to say something in the log about the
/// player's own weapon, and nobody else's.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Said<'w, 's> {
    player: Query<'w, 's, (), With<Player>>,
    worn: Query<'w, 's, &'static Equipped, With<Player>>,
    names: Query<'w, 's, &'static Name>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
}

impl Said<'_, '_> {
    /// What `item` is called, for a log line.
    fn name(&self, item: Entity) -> &str {
        self.names.get(item).map(Name::as_str).unwrap_or("weapon")
    }
}

/// Notes each hot weapon's heat on the gear panel's row for it, since the
/// panel is built from what the engine knows and the engine has never
/// heard of heat: `NN%` while it stands, `locked` while it is, in
/// [`Tones::BAD`] from 70 percent up so a climbing gauge reads as trouble
/// before it seizes.
///
/// `GearView` is `None` until a game adds `GearViewPlugin`, which the
/// binary's `GearPanel` does and a headless test need not: absence here
/// means no gear panel exists to annotate, not that anything is wrong.
pub fn note_heat(view: Option<ResMut<GearView>>, mut facets: ResMut<Facets>, heats: Query<&Heat>) {
    let Some(mut view) = view else { return };
    for row in view.rows_mut() {
        let Ok(heat) = heats.get(row.entity) else { continue };
        let pct = heat.now * 100 / CAPACITY;
        let hot = heat.locked || pct >= 70;
        let text = if heat.locked { "locked".to_string() } else { format!("{pct}%") };
        let facet = facets.facet("heat", text);
        row.facets.push(if hot { facet.toned(Tones::BAD) } else { facet });
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::RangedAttack;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_ui::GearViewPlugin;

    use super::*;

    #[test]
    fn a_weapon_fires_exactly_its_burst_before_it_locks() {
        // Spec sections 6.3 and 6.4: burst is the shots to reach 100.
        for (per_shot, burst) in [(15, 7), (22, 5), (25, 4), (20, 5), (30, 4), (35, 3), (40, 3), (45, 3)] {
            let mut h = Heat::new(per_shot, 20);
            let shots = (1..=10).find(|_| h.fire()).expect("locks within ten");
            assert_eq!(shots, burst, "per shot {per_shot}");
        }
    }

    #[test]
    fn a_locked_weapon_stays_locked_until_it_has_vented_all_the_way_to_zero() {
        let mut h = Heat::new(25, 20);
        while !h.fire() {}
        assert_eq!(h.now, 100);
        // The lock turn is a turn it fired, so it vents nothing: still
        // locked, still at 100, and this first `turn_end` reports no
        // unlock.
        assert!(!h.turn_end(), "the locking turn fired, so it does not vent");
        assert_eq!(h.now, 100);
        assert!(h.locked);
        // 100 at 20 a quiet turn is five more turns; unlocked on the
        // fifth of them, the sixth `turn_end` counted from the lock.
        let unlocked_on = (1..=10).find(|_| h.turn_end()).unwrap();
        assert_eq!(unlocked_on, 5);
        assert_eq!(h.now, 0);
    }

    #[test]
    fn a_turn_it_fired_on_does_not_vent_and_heat_never_goes_below_zero() {
        let mut h = Heat::new(15, 20);
        h.fire();
        h.turn_end();
        assert_eq!(h.now, 15, "fired this turn, so nothing vented");
        h.turn_end();
        assert_eq!(h.now, 0, "a quiet turn vents, and 15 less 20 is zero, not less");
    }

    #[test]
    fn an_overheated_weapon_stops_firing_and_the_next_one_in_hand_takes_over() {
        // Two hand blasters, both hands. The first fires its burst of seven and
        // locks; the loadout then finds the second, because the locked one no
        // longer carries a ranged attack. That is dual wielding under heat.
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, first, second) = crate::testing::dual_blasters(&mut app);
        let struck: Vec<_> = crate::testing::fire_at_a_target(&mut app, player, 8);
        assert!(struck[..7].iter().all(|s| s.with == Some(first)));
        assert_eq!(struck[7].with, Some(second), "the eighth shot comes from the other hand");
        assert!(app.world().get::<RangedAttack>(first).is_none(), "locked: stowed");
    }

    #[test]
    fn heat_on_struck_and_vent_heat_give_different_heat_depending_which_runs_first() {
        // This is the mechanism, not the wiring: it shows the two systems
        // are order-sensitive at all, on the exact pair of messages a
        // same-pass race (see the test below) would hand them. It does
        // not exercise `FoundryPlugin`'s own registration, so it proves
        // nothing about which order `Turn`'s `TurnSet::React` actually
        // runs them in; that is
        // `foundry_plugin_runs_vent_heat_before_heat_on_struck_so_the_race_resolves_correctly`,
        // below.
        //
        // The world is never `update`d before the messages are written:
        // a headless app's message buffers hold up to two frames, so
        // running it first, the way the other tests do to let a real
        // turn resolve, would leave stray `TurnEnd`s in the buffer for a
        // fresh `RunSystemOnce` reader (which starts counting from zero
        // every call, unlike a system registered once and left running)
        // to find and double-count.
        use bevy::ecs::system::RunSystemOnce;

        fn scenario(vent_first: bool) -> u32 {
            let mut app = crate::testing::headless(RunSeed(1));
            // A hand blaster mid-cooldown: quiet so far this turn.
            let item = app.world_mut().spawn(Heat { per_shot: 15, vent: 20, now: 15, locked: false, fired: false }).id();
            let bystander = app.world_mut().spawn_empty().id();
            app.world_mut().write_message(TurnEnd { turn: 1 });
            app.world_mut().write_message(Struck { attacker: bystander, target: bystander, with: Some(item), ranged: true });
            if vent_first {
                app.world_mut().run_system_once(vent_heat).unwrap();
                app.world_mut().run_system_once(heat_on_struck).unwrap();
            } else {
                app.world_mut().run_system_once(heat_on_struck).unwrap();
                app.world_mut().run_system_once(vent_heat).unwrap();
            }
            app.world().get::<Heat>(item).unwrap().now
        }

        assert_eq!(
            scenario(false),
            30,
            "heat_on_struck first: the new shot marks it fired, so the turn that actually just ended quietly wrongly skips its vent"
        );
        assert_eq!(scenario(true), 15, "vent_heat first: the ending turn vents (15 - 20, floored at 0), then the new shot adds its own 15");
    }

    #[test]
    fn foundry_plugin_runs_vent_heat_before_heat_on_struck_so_the_race_resolves_correctly() {
        // The wiring itself, not just the mechanism above: `FoundryPlugin`
        // is what `main.rs` and `testing::headless` both add, so if this
        // reads `Heat` correctly, the shipped game does too.
        //
        // The engine's own `schedule` (crates/rl-bevy/src/turn.rs) can
        // write a `TurnEnd` and deal the next actor's turn in the same
        // pass, so `TurnSet::React` can read the ending turn's `TurnEnd`
        // together with the very next turn's `Struck`. Reproducing that
        // exact race through the real scheduler (a second, AI-driven
        // actor timed to be dealt a turn on the very pass a `TurnEnd` is
        // written) is not reliable to pin to a seed, but the race's
        // *effect* on `TurnSet::React` is reproduced exactly: two
        // messages arriving in front of React together, which is cheap
        // to force directly, one whole `Turn` pass to run them through.
        //
        // `app.update()` twice, as `testing::dual_blasters` does, gets
        // the player past `NewRun` and admitted to the queue; by then it
        // holds a turn nothing has resolved (no intent was ever queued
        // for it), so `schedule()` finds it already holding and returns
        // at once, `Decide` finds no mind to run and no intent to
        // resolve, and the one further `Turn` pass below runs
        // `TurnSet::React` on exactly the two messages this test writes
        // and nothing `schedule()` adds of its own. `vent_heat` and
        // `heat_on_struck` are also both real, persistently-registered
        // systems by this point, so each already read and discarded
        // whatever the two setup frames wrote; only what this test writes
        // afterward is unread.
        let mut app = crate::testing::headless(RunSeed(1));
        app.update();
        app.update();
        // A hand blaster mid-cooldown: quiet so far this turn.
        let item = app.world_mut().spawn(Heat { per_shot: 15, vent: 20, now: 15, locked: false, fired: false }).id();
        let bystander = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(TurnEnd { turn: 1 });
        app.world_mut().write_message(Struck { attacker: bystander, target: bystander, with: Some(item), ranged: true });
        app.world_mut().run_schedule(rl_engine::rl_bevy::plugin::Turn);
        assert_eq!(
            app.world().get::<Heat>(item).unwrap().now,
            15,
            "FoundryPlugin must run vent_heat before heat_on_struck: the ending turn vents (15 - 20, floored at 0), then the new shot adds its own 15"
        );
    }

    #[test]
    fn a_weapons_row_reads_its_heat_and_reads_locked_once_it_seizes() {
        let mut app = crate::testing::headless(RunSeed(1));
        app.add_plugins(GearViewPlugin);
        let (player, first, _second) = crate::testing::dual_blasters(&mut app);
        let _ = player;
        app.update();
        let pct_at = |app: &App, item: Entity| -> Option<String> {
            let view = app.world().resource::<GearView>();
            let row = view.worn().map(|(_, row)| row).find(|row| row.entity == item)?;
            row.facet(app.world().resource::<Facets>().get("heat")?).map(|f| f.text.clone())
        };
        assert_eq!(pct_at(&app, first).as_deref(), Some("0%"), "cold and freshly worn");
        crate::testing::fire_at_a_target(&mut app, player, 7);
        app.update();
        assert_eq!(pct_at(&app, first).as_deref(), Some("locked"), "the burst that reaches 100 locks it");
    }
}
