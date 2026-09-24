//! Keys to intents.
//!
//! Every key Foundry answers to is declared once, in [`declare_controls`],
//! and read by name through [`Binds`]. The `?` screen lists that same
//! declaration, so the keys it shows are the keys the systems here check.
//! The screens' own keys (`i` for the pack, `a` for abilities, `x` to look
//! about) are declared by the engine panels that read them, beside these.
//!
//! Registered by [`FoundryPlugin`](crate::plugin::FoundryPlugin), not by
//! `main.rs`, so a test presses the same keys the player does.

use bevy::prelude::*;
use rl_engine::prelude::*;

use crate::ammo::Dry;
use crate::heat::Heat;

/// Every key Foundry answers to, by name.
#[derive(Resource, Clone, Copy)]
pub struct Binds {
    /// Walk, or strike whoever is there.
    pub walk: ControlId,
    /// Take the lift down or up.
    pub go_through: ControlId,
    /// Wait a turn.
    pub wait: ControlId,
    /// Pick up what is underfoot.
    pub pick_up: ControlId,
    /// Fire what is wielded, through the targeting cursor.
    pub fire: ControlId,
    /// Open the pack on the first thing in it that can be thrown.
    pub throw: ControlId,
    /// Switch the shoulder lamp off or on; read by
    /// [`light::toggle_lamp`](crate::light::toggle_lamp).
    pub lamp: ControlId,
    /// Open the cheat menu; read by [`cheats`](crate::cheats).
    pub cheats: ControlId,
}

/// Declares the keys, under the headings the `?` screen groups them by.
///
/// A chord is matched exactly, so `L`, the lamp, is never `l`, a step
/// east.
pub fn declare_controls(app: &mut App) {
    let binds = Binds {
        walk: app.add_control("Move", "walk, or strike whoever is there", Keys::Directions { shift: false }),
        go_through: app.add_control("Move", "take the lift", [Chord::key(KeyCode::Enter), Chord::shift(KeyCode::Period), Chord::shift(KeyCode::Comma)]),
        wait: app.add_control("Act", "wait a turn", [KeyCode::Period, KeyCode::Numpad5]),
        pick_up: app.add_control("Act", "pick up what is here", [KeyCode::KeyG, KeyCode::Comma]),
        fire: app.add_control("Act", "fire at the nearest droid", KeyCode::KeyF),
        throw: app.add_control("Act", "throw something from the pack", KeyCode::KeyT),
        lamp: app.add_control("Act", "switch the lamp off, or on", Chord::shift(KeyCode::KeyL)),
        cheats: app.add_control("Debug", "the cheat menu", KeyCode::Backslash),
    };
    app.insert_resource(binds);
}

/// The player while it holds the turn.
type PlayerTurn<'w, 's> = Query<'w, 's, Entity, (With<Player>, With<MyTurn>)>;

/// What every key here reads before it asks for anything.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Keyboard<'w, 's> {
    keys: ControlInput<'w>,
    binds: Res<'w, Binds>,
    modals: ResMut<'w, Modals>,
    turns: Res<'w, Turns>,
    log: ResMut<'w, MessageLog>,
    player: PlayerTurn<'w, 's>,
}

/// What the player's keys can ask for.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PlayerIntents<'w> {
    bumps: MessageWriter<'w, Intent<Bump>>,
    waits: MessageWriter<'w, Intent<Wait>>,
    transits: MessageWriter<'w, Intent<GoThrough>>,
    pick_ups: MessageWriter<'w, Intent<PickUp>>,
    fires: MessageWriter<'w, AimFire>,
}

/// The pack's screen, which `t` opens: absent from a test that draws no
/// screens, where `t` has nothing to open.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Pack<'w> {
    view: Option<Res<'w, InventoryView>>,
    menu: Option<ResMut<'w, InventoryMenu>>,
}

/// Everything on the deck `f` looks for.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Reach<'w, 's> {
    loadout: Loadout<'w, 's>,
    idle: Query<'w, 's, (&'static Name, Option<&'static Heat>, Has<Dry>)>,
    worn: Query<'w, 's, &'static Equipped>,
}

/// Turns keys into an [`Intent`] for the player while it holds the turn,
/// and nothing at all while a screen is up: the stack is empty or the
/// world does not have the keys.
///
/// `f` opens the engine's targeting cursor, which starts on the nearest
/// foe in sight, cycles the rest with Tab, previews the line and acts on
/// confirm. `t` opens the pack on the first thing in it that can be
/// thrown, rather than throwing it: which of several is meant is the
/// player's to pick, and the pack's own `t` sends the row picked out to
/// the same cursor. A key with nothing to do says so without spending the
/// turn, since a mistyped key should not cost a life.
pub fn player_input(mut kb: Keyboard, reach: Reach, mut pack: Pack, mut intents: PlayerIntents) {
    if kb.modals.any_open() {
        return;
    }
    let Ok(me) = kb.player.single() else { return };
    let now = kb.turns.turn_number();
    let (keys, binds) = (&kb.keys, *kb.binds);
    if let Some(dir) = keys.direction(binds.walk) {
        // A step, a blow at a droid, or a hatch opened: the engine decides.
        intents.bumps.write(Intent::new(me, Bump(dir)));
    } else if keys.just_pressed(binds.go_through) {
        intents.transits.write(Intent::new(me, GoThrough));
    } else if keys.just_pressed(binds.wait) {
        intents.waits.write(Intent::new(me, Wait));
    } else if keys.just_pressed(binds.pick_up) {
        intents.pick_ups.write(Intent::new(me, PickUp));
    } else if keys.just_pressed(binds.fire) {
        // A shot is a weapon's, never the commando's own: what is wielded
        // and loaded, cool enough to fire.
        if reach.loadout.ranged(me).is_some() {
            intents.fires.write(AimFire { user: me });
        } else {
            let worn: Vec<Entity> = reach.worn.get(me).map(|e| e.0.worn().map(|(_, item)| item).collect()).unwrap_or_default();
            kb.log.muted(why_not(&worn, &reach), now);
        }
    } else if keys.just_pressed(binds.throw) {
        let (Some(view), Some(menu)) = (pack.view.as_deref(), pack.menu.as_deref_mut()) else { return };
        match view.rows.iter().position(|row| row.throw_range.is_some()) {
            Some(row) => {
                menu.selected = row;
                let bag = inventory_modal(&kb.modals);
                kb.modals.open(bag);
            }
            None => kb.log.muted("You have nothing to throw.", now),
        }
    }
}

/// Why nothing worn can fire: the first gun that is locked or dry, by
/// name, so the player knows whether to wait or to find slugs, or plainly
/// that there is no gun in hand at all.
fn why_not(worn: &[Entity], reach: &Reach) -> String {
    for (name, heat, dry) in worn.iter().filter_map(|item| reach.idle.get(*item).ok()) {
        if heat.is_some_and(|h| h.locked) {
            return format!("Your {name} is locked until it cools.");
        }
        if dry {
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
