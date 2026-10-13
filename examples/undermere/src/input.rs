//! Keys to intents.
//!
//! Every key Undermere answers to is declared once, in
//! [`declare_controls`], and read by name through [`Binds`]. The `?`
//! screen lists that same declaration, so the keys it shows are the keys
//! the systems here check. The screens' own keys, `i` for the pack and
//! `x` to look about, are declared by the engine panels that read them.
//!
//! Registered by [`UndermerePlugin`](crate::plugin::UndermerePlugin), not
//! by the binary, so a test presses the same keys the player does.

use bevy::prelude::*;
use rl_engine::prelude::*;

/// Every key Undermere answers to, by name.
#[derive(Resource, Clone, Copy)]
pub struct Binds {
    /// Walk, or strike whoever is there.
    pub walk: ControlId,
    /// Take the stairs, or go through the gate.
    pub go_through: ControlId,
    /// Wait a turn.
    pub wait: ControlId,
    /// Pick up what is underfoot.
    pub pick_up: ControlId,
    /// Shoot what is wielded, through the targeting cursor.
    pub fire: ControlId,
    /// Open the pack on the first thing in it that can be thrown.
    pub throw: ControlId,
    /// Cycle every hired mercenary's order, by
    /// [`order_party`](crate::mercenaries::order_party).
    pub order: ControlId,
    /// Mark the doors seen and not yet walked through, until the next key;
    /// read by [`mark_doors`](crate::doors::mark_doors).
    pub mark_doors: ControlId,
}

/// Declares the keys, under the headings the `?` screen groups them by.
///
/// Enter, `>` and `<` all take whatever way through stands underfoot,
/// stairs or the gate: which way it leads is the way's own, so a key for
/// each would only be a key to get wrong.
pub fn declare_controls(app: &mut App) {
    let binds = Binds {
        walk: app.add_control("Move", "walk, or strike whoever is there", Keys::Directions { shift: false }),
        go_through: app.add_control(
            "Move",
            "take the stairs, or go through the gate",
            [Chord::key(KeyCode::Enter), Chord::shift(KeyCode::Period), Chord::shift(KeyCode::Comma)],
        ),
        wait: app.add_control("Act", "wait a turn", [KeyCode::Period, KeyCode::Numpad5]),
        pick_up: app.add_control("Act", "pick up what is here", [KeyCode::KeyG, KeyCode::Comma]),
        fire: app.add_control("Act", "shoot at the nearest foe", KeyCode::KeyF),
        throw: app.add_control("Act", "throw something from the pack", KeyCode::KeyT),
        order: app.add_control("Act", "order the party: follow, hold, fight freely", KeyCode::KeyO),
        mark_doors: app.add_control("Look", "mark the doors not yet gone through", KeyCode::KeyM),
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

/// Turns keys into an [`Intent`] for the player while it holds the turn,
/// and nothing at all while a screen is up.
///
/// `f` opens the engine's targeting cursor on the nearest foe in sight,
/// with whatever shoots in hand. `t` opens the pack with the first thing in
/// it that can be thrown opened, rather than throwing it: the pack's own
/// `t` sends that row to the same cursor, and Esc goes back to the list
/// when another of several is meant. A key with nothing to do says so without
/// spending the turn, since a mistyped key should not cost a life.
pub fn player_input(mut kb: Keyboard, loadout: Loadout, mut pack: Pack, mut intents: PlayerIntents) {
    if kb.modals.any_open() {
        return;
    }
    let Ok(me) = kb.player.single() else { return };
    let now = kb.turns.turn_number();
    let (keys, binds) = (&kb.keys, *kb.binds);
    if let Some(dir) = keys.direction(binds.walk) {
        // A step, a blow, or a door opened: the engine decides.
        intents.bumps.write(Intent::new(me, Bump(dir)));
    } else if keys.just_pressed(binds.go_through) {
        intents.transits.write(Intent::new(me, GoThrough));
    } else if keys.just_pressed(binds.wait) {
        intents.waits.write(Intent::new(me, Wait));
    } else if keys.just_pressed(binds.pick_up) {
        intents.pick_ups.write(Intent::new(me, PickUp));
    } else if keys.just_pressed(binds.fire) {
        if loadout.ranged(me).is_some() {
            intents.fires.write(AimFire { user: me });
        } else {
            kb.log.muted("You have nothing in hand to shoot with.", now);
        }
    } else if keys.just_pressed(binds.throw) {
        let (Some(view), Some(menu)) = (pack.view.as_deref(), pack.menu.as_deref_mut()) else { return };
        match view.rows.iter().position(|row| row.throw_range.is_some()) {
            Some(row) => {
                menu.open(row);
                let bag = inventory_modal(&kb.modals);
                kb.modals.open(bag);
            }
            None => kb.log.muted("You have nothing to throw.", now),
        }
    }
}

#[cfg(test)]
mod tests {
    use rl_engine::rl_bevy::testing::{KeyScriptPlugin, press};
    use rl_engine::rl_core::RunSeed;

    use super::*;

    fn said(app: &App, line: &str) -> bool {
        app.world().resource::<MessageLog>().iter().any(|e| e.text == line)
    }

    /// The adventurer sets out with a sword and no bow, so `f` says there
    /// is nothing to shoot with and costs nothing.
    #[test]
    fn f_with_nothing_to_shoot_with_says_so_and_spends_no_turn() {
        let mut app = crate::testing::headless(RunSeed(5));
        app.add_plugins(KeyScriptPlugin);
        crate::testing::settle(&mut app);
        let before = app.world().resource::<Turns>().now();
        press(&mut app, KeyCode::KeyF);
        assert!(said(&app, "You have nothing in hand to shoot with."));
        assert_eq!(app.world().resource::<Turns>().now(), before, "no turn spent on a sword");
    }
}
