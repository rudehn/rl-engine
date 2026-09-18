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
use rl_engine::rl_bevy::{MeleeAttack, RangedAttack, Struck, TurnEnd, Turns};
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

    /// Ends a turn: vents unless it is still working and fired this turn,
    /// and answers whether this turn is the one that unlocked it.
    ///
    /// A weapon that only ever gets one shot off between quiet turns
    /// never locks, which is the trade a fast, low-heat weapon makes for
    /// never taking the burst a slow one does; that trade is this gate.
    /// A locked weapon has nothing left to fire, so it is exempt from the
    /// gate and vents every turn without exception, the turn it seized
    /// included: nothing thereafter but cooling is left for it to do.
    /// Never below zero.
    pub fn turn_end(&mut self) -> bool {
        if self.locked || !self.fired {
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
/// cools.
pub fn heat_on_struck(
    mut commands: Commands,
    mut struck: MessageReader<Struck>,
    mut heats: Query<&mut Heat>,
    weapons: Query<(Option<&MeleeAttack>, Option<&RangedAttack>)>,
    names: Query<&Name>,
    turns: Res<Turns>,
    mut log: ResMut<MessageLog>,
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
        let name = names.get(item).map(Name::as_str).unwrap_or("it");
        log.bad(format!("{name} overheats and locks."), turns.turn_number());
    }
}

/// Reacts to every whole [`TurnEnd`]: vents every [`Heat`], and for one
/// that unlocks, takes its [`Stowed`] attack back off the shelf and gives
/// it back to the item.
pub fn vent_heat(
    mut commands: Commands,
    mut ends: MessageReader<TurnEnd>,
    mut heats: Query<(Entity, &mut Heat, Option<&Stowed>)>,
    names: Query<&Name>,
    mut log: ResMut<MessageLog>,
) {
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
            let name = names.get(entity).map(Name::as_str).unwrap_or("it");
            log.notice(format!("{name} cools and unlocks."), ev.turn);
        }
    }
}

/// Notes each hot weapon's heat on the gear panel's row for it, since the
/// panel is built from what the engine knows and the engine has never
/// heard of heat: `NN%` while it stands, `locked` while it is, in
/// [`Tones::BAD`] from 70 percent up so a climbing gauge reads as trouble
/// before it seizes.
///
/// `GearView` is `None` until a game adds `GearViewPlugin`; this slice's
/// binary does not yet, so absence here means no gear panel exists to
/// annotate, not that anything is wrong. Registered unconditionally so the
/// day the panel is added, the facet is already correct.
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
        // 100 at 20 a quiet turn is five turns; unlocked on the fifth, not before.
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
