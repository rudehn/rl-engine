//! Keys to intents.
//!
//! The standard keys, walking, the way through, waiting, picking up,
//! shooting and throwing, are the engine's [`PlayerKeysPlugin`], added in
//! [`declare_controls`] in Undermere's words. Undermere's own keys are
//! declared beside them and read by name through [`Binds`]. The `?`
//! screen lists those same declarations, so the keys it shows are the
//! keys the systems check. The screens' own keys, `i` for the pack and `x`
//! to look about, are declared by the engine panels that read them.
//!
//! Registered by [`UndermerePlugin`](crate::plugin::UndermerePlugin), not
//! by the binary, so a test presses the same keys the player does.

use atglyph::prelude::*;
use bevy::prelude::*;

/// Every key of Undermere's own, by name.
#[derive(Resource, Clone, Copy)]
pub struct Binds {
    /// Cycle every hired mercenary's order, by
    /// [`order_party`](crate::mercenaries::order_party).
    pub order: ControlId,
    /// Mark the doors seen and not yet walked through, until the next key;
    /// read by [`mark_doors`](crate::doors::mark_doors).
    pub mark_doors: ControlId,
}

/// Declares the keys, under the headings the `?` screen groups them by:
/// the engine's standard ones first, then Undermere's own.
///
/// Enter, `>` and `<` all take whatever way through stands underfoot,
/// stairs or the gate: which way it leads is the way's own, so a key for
/// each would only be a key to get wrong. `f` shoots whatever is in hand
/// through the engine's targeting cursor. `t` opens the pack on the first
/// thing made to be thrown, and only when there is none on the first that
/// can be: a potion can be thrown, but a hand reaching for `t` means the
/// darts before the healing. Either with nothing to do says so without
/// spending the turn, since a mistyped key should not cost a life.
pub fn declare_controls(app: &mut App) {
    app.add_plugins(
        PlayerKeysPlugin::new()
            .walk("Move", "walk, or strike whoever is there")
            .go_through("Move", "take the stairs, or go through the gate")
            .wait("Act", "wait a turn")
            .pick_up("Act", "pick up what is here")
            .fire("Act", "shoot at the nearest foe", Otherwise::say("You have nothing in hand to shoot with."))
            .throw("Act", "throw something from the pack", Otherwise::say("You have nothing to throw."))
            .throw_first("thrown"),
    );
    let binds = Binds {
        order: app.add_control("Act", "order the party: follow, hold, fight freely", KeyCode::KeyO),
        mark_doors: app.add_control("Look", "mark the doors not yet gone through", KeyCode::KeyM),
    };
    app.insert_resource(binds);
}

#[cfg(test)]
mod tests {
    use atglyph::ecs::testing::{KeyScriptPlugin, press};
    use atglyph::primitives::RunSeed;

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
