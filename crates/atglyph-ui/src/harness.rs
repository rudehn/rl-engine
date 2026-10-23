//! A world small enough to test a view against.
//!
//! Every view reads the same handful of things: a map to see across, a
//! player with a viewshed, and the combat rules a relation comes from.
//! Building those once here keeps each view's test about the view.

use atglyph_bevy::prelude::*;
use atglyph_core::Point;
use bevy::prelude::*;

/// A world, a player standing in it, and the ids a test needs to spawn
/// something the player can see.
pub(crate) struct Stage {
    /// The app, already updated into `EngineState::Playing`.
    pub app: App,
    /// The player entity.
    pub player: Entity,
    /// Where the player stands.
    pub at: Point,
    /// A faction hostile to the player's.
    pub theirs: atglyph_rules::FactionId,
    /// The one damage kind.
    pub kind: atglyph_rules::damage::DamageKindId,
}

impl Stage {
    /// An open world with a player at its middle, the given UI plugins
    /// added, and one turn run so sight is computed.
    pub fn new<M>(plugins: impl bevy::app::Plugins<M>) -> Stage {
        Stage::new_with(plugins, |_| {})
    }

    /// The same, with `setup` run before play begins, for a plugin that
    /// asserts on a resource the game is meant to have inserted by then.
    pub(crate) fn new_with<M>(plugins: impl bevy::app::Plugins<M>, setup: impl FnOnce(&mut App)) -> Stage {
        let mut app = atglyph_bevy::testing::headless_app();
        app.add_plugins((FovPlugin, CombatPlugin, ItemsPlugin));
        app.add_plugins((atglyph_bevy::testing::KeyScriptPlugin, crate::UiPlugin));
        app.add_plugins(plugins);
        // A panel draws into a terminal, so there is always one; `screen`
        // replaces it when a test wants a particular size.
        app.insert_resource(atglyph_render::Terminal::new(100, 40, Vec2::ONE));

        let at = atglyph_bevy::testing::ground(&mut app);
        let atglyph_bevy::testing::TwoSides { ours, theirs, kind } = atglyph_bevy::testing::two_sides(&mut app);

        let player = app
            .world_mut()
            .spawn((
                (Actor, Player, Blocks, Position(at), Viewshed::new(10), RevealsMap),
                (
                    Health::full(30),
                    Armor(0),
                    Faction(ours),
                    MeleeAttack::new(kind, atglyph_core::DiceRoll::new(1, 6)),
                    Name::new("you"),
                    Inventory::default(),
                    atglyph_render::Glyph::new('@', Color::WHITE).on_layer(10),
                ),
            ))
            .id();
        setup(&mut app);
        // What `App::run` would do before its first frame and `update` does
        // not: run every plugin's `finish`, where a plugin declares what it
        // could only declare once the game had built everything else, such
        // as its controls.
        app.finish();
        app.cleanup();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        Stage { app, player, at, theirs, kind }
    }

    /// Spawns an actor `dx, dy` from the player, hostile by default.
    pub(crate) fn actor(&mut self, name: &str, glyph: char, dx: i32, dy: i32) -> Entity {
        let at = self.at.offset(dx, dy);
        let (kind, theirs) = (self.kind, self.theirs);
        self.app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(at),
                Health::full(10),
                Armor(0),
                Faction(theirs),
                MeleeAttack::new(kind, atglyph_core::DiceRoll::new(1, 4)),
                Name::new(name.to_string()),
                atglyph_render::Glyph::new(glyph, Color::WHITE).on_layer(5),
            ))
            .id()
    }

    /// Spawns a thing on the ground `dx, dy` from the player.
    pub(crate) fn thing(&mut self, name: &str, glyph: char, dx: i32, dy: i32) -> Entity {
        let at = self.at.offset(dx, dy);
        self.app.world_mut().spawn((Item, Position(at), Name::new(name.to_string()), atglyph_render::Glyph::new(glyph, Color::WHITE).on_layer(2))).id()
    }

    /// Runs a frame.
    pub(crate) fn tick(&mut self) {
        self.app.update();
    }

    /// Resizes the terminal, for a panel test that asserts on whole rows.
    pub(crate) fn screen(mut self, width: i32, height: i32) -> Stage {
        self.app.insert_resource(atglyph_render::Terminal::new(width, height, Vec2::ONE));
        self.app.update();
        self
    }

    /// One row of the terminal as text, trailing blanks trimmed.
    pub(crate) fn row(&self, y: i32) -> String {
        crate::testing::row(&self.app, y).trim_end().to_string()
    }

    /// Every row of the terminal, for an assertion over the whole panel.
    pub(crate) fn rows(&self) -> Vec<String> {
        let height = self.app.world().resource::<atglyph_render::Terminal>().height();
        (0..height).map(|y| self.row(y)).collect()
    }

    /// Presses `key` for one frame and releases it on the next.
    ///
    /// Through [`KeyScriptPlugin`](atglyph_bevy::testing::KeyScriptPlugin)
    /// rather than `ButtonInput::press`, because Bevy clears `just_pressed`
    /// at the top of every frame.
    pub fn press(&mut self, key: KeyCode) {
        atglyph_bevy::testing::press(&mut self.app, key);
    }

    /// Presses every key in `keys` together for one frame, for a chord such
    /// as Shift with a letter.
    pub(crate) fn chord(&mut self, keys: &[KeyCode]) {
        for key in keys {
            self.app.world_mut().resource_mut::<atglyph_bevy::testing::KeyScript>().press(*key);
        }
        self.app.update();
        self.app.update();
    }
}
