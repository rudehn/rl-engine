//! What Foundry says when its own ammunition-fed weapon dries or reloads.
//!
//! The loop itself, what a shot spends, the tag searched for, the bag read
//! fresh every pass, lives in the engine now (`rl_bevy::ammo`), lifted out
//! of this game once a second game with a bow needed the same thing a
//! commando's slug pistol already had. All that is left here is this
//! game's own words for the moment it happens: [`narrate_ammo`] reacts to
//! the [`AmmoChanged`] the engine writes for every transition, the way
//! `heat`'s own pair says so in its own file for a weapon that locks or
//! cools.

use bevy::prelude::*;
use rl_engine::rl_bevy::{AmmoChanged, Equipped, Player};
use rl_engine::rl_ui::{Tell, Tones};

/// Tells the player when their own ammunition-fed weapon runs dry or is
/// loaded again, the way `heat::heat_on_struck` and `heat::vent_heat` say
/// so for one that locks or cools: narration is this game's word choice,
/// [`AmmoChanged`] is the engine's.
///
/// Only the player's own weapon says so in the log: a pistol in a droid's
/// holster running dry is nothing the player can act on, and reads as
/// noise between the lines that matter.
pub fn narrate_ammo(mut changed: MessageReader<AmmoChanged>, worn: Query<&Equipped, With<Player>>, names: Query<&Name>, mut tell: MessageWriter<Tell>) {
    let yours = |item: Entity| worn.iter().any(|e| e.0.worn().any(|(_, w)| w == item));
    let name = |item: Entity| names.get(item).map(Name::as_str).unwrap_or("weapon");
    for ev in changed.read().filter(|ev| yours(ev.item)) {
        let (line, tone) = if ev.dry {
            (format!("Your {} runs dry.", name(ev.item)), Tones::BAD)
        } else {
            (format!("Your {} is loaded again.", name(ev.item)), Tones::NOTICE)
        };
        tell.write(Tell::new(line, tone));
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::RangedAttack;
    use rl_engine::rl_core::RunSeed;
    use rl_engine::rl_ui::MessageLog;

    use super::*;

    #[test]
    fn the_players_own_pistol_running_dry_and_loading_again_both_say_so() {
        let mut app = crate::testing::headless(RunSeed(1));
        let (player, pistol) = crate::testing::slug_pistol_with(&mut app, 1);
        let said = |app: &App, line: &str| app.world().resource::<MessageLog>().iter().any(|e| e.text == line);

        crate::testing::fire_at_a_target(&mut app, player, 1);
        app.update();
        assert!(app.world().get::<RangedAttack>(pistol).is_none(), "dry: that was the only slug");
        assert!(said(&app, "Your slug pistol runs dry."));

        crate::testing::give_slugs(&mut app, player, 1);
        app.update();
        assert!(app.world().get::<RangedAttack>(pistol).is_some(), "loaded again");
        assert!(said(&app, "Your slug pistol is loaded again."));
    }

    /// A weapon nobody has equipped running dry says nothing: narration
    /// answers for the player's own gear, not for every item the engine's
    /// loop happens to touch, a droid's holster among them.
    #[test]
    fn a_weapon_nobody_has_equipped_running_dry_says_nothing() {
        let mut app = crate::testing::headless(RunSeed(1));
        app.update();
        app.update();
        let unworn = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(AmmoChanged { item: unworn, dry: true });
        app.update();
        assert!(!app.world().resource::<MessageLog>().iter().any(|e| e.text.contains("runs dry")));
    }
}
