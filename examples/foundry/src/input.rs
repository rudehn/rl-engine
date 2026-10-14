//! Keys to intents.
//!
//! The standard keys, walking, the lift, waiting, picking up, firing and
//! throwing, are the engine's [`PlayerKeysPlugin`], added in
//! [`declare_controls`] in Foundry's words. Foundry's own keys are declared
//! beside them and read by name through [`Binds`]. The `?` screen lists
//! those same declarations, so the keys it shows are the keys the systems
//! check. The screens' own keys (`i` for the pack, `a` for abilities, `x`
//! to look about) are declared by the engine panels that read them, beside
//! these.
//!
//! Registered by [`FoundryPlugin`](crate::plugin::FoundryPlugin), not by
//! `main.rs`, so a test presses the same keys the player does.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::heat::Heat;

/// Every key of Foundry's own, by name.
#[derive(Resource, Clone, Copy)]
pub struct Binds {
    /// Switch the shoulder lamp off or on; read by
    /// [`light::toggle_lamp`](crate::light::toggle_lamp).
    pub lamp: ControlId,
    /// Open the cheat menu; read by [`cheats`](crate::cheats).
    pub cheats: ControlId,
}

/// Declares the keys, under the headings the `?` screen groups them by:
/// the engine's standard ones first, then Foundry's own.
///
/// `f` opens the engine's targeting cursor, which starts on the nearest
/// foe in sight, cycles the rest with Tab, previews the line and acts on
/// confirm; with nothing ready to fire, [`say_why_not`] says why. `t`
/// opens the pack on the first thing in it that can be thrown, rather than
/// throwing it: the pack's own `t` sends that row to the same cursor, and
/// Esc goes back to the list when another of several is meant. Neither
/// spends a turn when it has nothing to do, since a mistyped key should
/// not cost a life.
///
/// A chord is matched exactly, so `L`, the lamp, is never `l`, a step
/// east.
pub fn declare_controls(app: &mut App) {
    app.add_plugins(
        PlayerKeysPlugin::new()
            .walk("Move", "walk, or strike whoever is there")
            .go_through("Move", "take the lift")
            .wait("Act", "wait a turn")
            .pick_up("Act", "pick up what is here")
            .fire("Act", "fire at the nearest droid", Otherwise::Nothing)
            .throw("Act", "throw something from the pack", Otherwise::say("You have nothing to throw.")),
    );
    let binds = Binds {
        lamp: app.add_control("Act", "switch the lamp off, or on", Chord::shift(KeyCode::KeyL)),
        cheats: app.add_control("Debug", "the cheat menu", KeyCode::Backslash),
    };
    app.insert_resource(binds);
}

/// Everything on the deck `f` looks for.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Reach<'w, 's> {
    loadout: Loadout<'w, 's>,
    idle: Query<'w, 's, (&'static Name, Option<&'static Heat>)>,
    worn: Query<'w, 's, &'static Equipped>,
}

/// Says why `f` found nothing to fire, the turn it was pressed: the
/// engine's key declines and spends nothing, and the reason is Foundry's.
///
/// A shot is a weapon's, never the commando's own: what is wielded and
/// loaded, cool enough to fire.
pub fn say_why_not(mut declined: MessageReader<Declined>, reach: Reach, turns: Res<Turns>, mut log: ResMut<MessageLog>) {
    for asked in declined.read().filter(|d| d.verb == PlayerVerb::Fire) {
        let worn: Vec<Entity> = reach.worn.get(asked.actor).map(|e| e.0.worn().map(|(_, item)| item).collect()).unwrap_or_default();
        log.muted(why_not(&worn, &reach), turns.turn_number());
    }
}

/// Why nothing worn can fire: the first gun that is locked or dry, by
/// name, so the player knows whether to wait or to find slugs, or plainly
/// that there is no gun in hand at all.
fn why_not(worn: &[Entity], reach: &Reach) -> String {
    for &item in worn {
        let Ok((name, heat)) = reach.idle.get(item) else { continue };
        if heat.is_some_and(|h| h.locked) {
            return format!("Your {name} is locked until it cools.");
        }
        if reach.loadout.dry(item) {
            return format!("Your {name} is out of ammunition.");
        }
    }
    "You have nothing in hand to fire.".to_string()
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::Ending;
    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};
    use rl_engine::rl_core::RunSeed;

    use super::*;

    fn said(app: &App, line: &str) -> bool {
        app.world().resource::<MessageLog>().iter().any(|e| e.text == line)
    }

    #[test]
    fn f_with_no_weapon_ready_says_so_and_spends_no_turn() {
        let mut app = crate::testing::headless(RunSeed(5));
        app.add_plugins(KeyScriptPlugin);
        crate::testing::settle(&mut app);
        crate::testing::empty_handed(&mut app);
        let before = crate::testing::clock(&app);
        press(&mut app, KeyCode::KeyF);
        assert!(said(&app, "You have nothing in hand to fire."));
        assert_eq!(crate::testing::clock(&app), before, "no turn spent on an empty hand");
    }

    #[test]
    fn f_with_only_a_dry_pistol_in_hand_says_it_is_out_of_ammunition() {
        let mut app = crate::testing::headless(RunSeed(5));
        app.add_plugins(KeyScriptPlugin);
        crate::testing::slug_pistol_with(&mut app, 0);
        press(&mut app, KeyCode::KeyF);
        assert!(said(&app, "Your slug pistol is out of ammunition."));
    }

    /// The whole pick path through the real keys: walking into the
    /// console sets the charge, because a bump into a prop that offers one
    /// thing is that offer taken up; the pick opens, Down and Enter take
    /// the second upgrade, and the run carries on with it fitted rather
    /// than ending, since one pick out of four charges is not a run won.
    #[test]
    fn walking_into_the_console_then_down_and_enter_on_the_pick_fits_uplink_and_leaves_the_run_playing() {
        let mut app = crate::testing::headless(RunSeed(2));
        app.add_plugins(KeyScriptPlugin);
        let me = crate::testing::beside_the_console(&mut app);
        crate::testing::settle(&mut app);
        let into_it = crate::testing::key_toward_the_console(&app, me);
        press(&mut app, into_it);
        crate::testing::settle(&mut app);
        assert!(crate::testing::quest_done(&app, "first_charge"), "walking into it set the charge");
        let pick = app.world().resource::<Modals>().get(crate::upgrades::MODAL).unwrap();
        assert!(app.world().resource::<Modals>().is_top(pick), "and the pick is up");

        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        crate::testing::settle(&mut app);
        assert!(app.world().get_resource::<Ending>().is_none(), "one pick does not end the run");
        assert!(app.world().get::<crate::upgrades::Uplinked>(me).is_some(), "the second row, Uplink, was the one fitted");
        assert!(!app.world().resource::<Modals>().is_open(pick), "and the pick is closed");
    }
}
