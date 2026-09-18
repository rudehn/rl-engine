//! A headless Foundry, for the tests this crate and later tasks add.
//!
//! The engine plugins the design needs, with no window: enough of the
//! real wiring that a test exercises it rather than a mock of it. No
//! systems of Foundry's own run yet; later tasks add them here.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_ui::UiPlugin;

/// A run with no window, seeded, with every plugin Foundry's stealth,
/// radar and combat need already added.
pub fn headless(seed: RunSeed) -> App {
    let mut app = rl_engine::rl_bevy::plugin::headless_app();
    app.add_plugins((
        FovPlugin,
        CombatPlugin,
        MindsPlugin,
        StatusPlugin,
        ItemsPlugin,
        ThrowingPlugin,
        LightingPlugin,
        StealthPlugin,
        FactsPlugin,
        AbilitiesPlugin,
    ));
    app.add_engine_effects().insert_resource(Seed(seed));
    app.add_plugins(UiPlugin);
    app
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_headless_run_wires_every_plugin_the_harness_names() {
        let _app = headless(RunSeed(7));
    }
}
