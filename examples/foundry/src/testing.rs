//! A headless Foundry, for the tests this crate and its integration
//! tests add.
//!
//! The engine plugins the design needs, with no window: enough of the
//! real wiring that a test exercises it rather than a mock of it. Only
//! `gear::grant_dark_sight` is Foundry's own system so far; later tasks
//! add more here.

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_rules::prelude::Ledger;
use rl_engine::rl_ui::UiPlugin;

/// A run with no window, seeded, with every plugin Foundry's stealth,
/// radar and combat need already added.
///
/// `FactsPlugin` and `AbilitiesPlugin` are here for later tasks; a plugin
/// asserts what it cannot work without the moment play begins, so this
/// harness satisfies both with the smallest thing that counts as
/// "nothing yet": an empty ledger, and abilities loaded from no
/// definitions at all.
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
    app.add_engine_effects().insert_resource(Seed(seed)).insert_resource(crate::content::registries());
    app.insert_resource(Counters(Ledger::default()));
    let abilities = {
        let world = app.world();
        let (kinds, registries) = (world.resource::<EffectKinds>(), world.resource::<Registries>());
        Abilities::load("[]", kinds, &registries.names()).unwrap_or_else(|e| panic!("no abilities: {e}"))
    };
    app.insert_resource(abilities);
    app.add_plugins(UiPlugin);
    app.add_systems(NewRun, crate::run::start);
    app.add_systems(Turn, crate::gear::grant_dark_sight.in_set(TurnSet::React));
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
