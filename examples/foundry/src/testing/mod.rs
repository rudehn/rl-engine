//! A headless Foundry, for the tests this crate and its integration
//! tests add.
//!
//! The engine plugins the design needs, with no window, plus
//! [`FoundryPlugin`](crate::plugin::FoundryPlugin): the same one `main.rs`
//! adds, so a test exercises exactly what the player runs rather than a
//! harness that quietly fell behind it.
//!
//! Split by what a helper sets up rather than kept as one file: `gear`
//! for weapons, armor and ammunition, `droids` for monsters and their
//! attacks, `loot` for what a deck scatters and a kill drops, and
//! `mission` for the reactor console and the upgrade pick. Every helper
//! is re-exported here, so `crate::testing::x` still finds whichever file
//! `x` actually lives in.

mod droids;
mod gear;
mod loot;
mod mission;

pub use droids::*;
pub use gear::*;
pub use loot::*;
pub use mission::*;

use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_rules::Hit;
use rl_engine::rl_rules::prelude::Ledger;
use rl_engine::rl_ui::UiPlugin;

/// A run with no window, seeded, with every plugin Foundry's stealth,
/// radar and combat need already added.
///
/// `FactsPlugin` needs `Quests` or `Counters` inserted before play begins;
/// `mission::start`, added to `NewRun` by `FoundryPlugin`, inserts both the
/// moment the run starts, the same as the real binary. `AbilitiesPlugin`
/// needs `Abilities`, loaded here from `assets/abilities.ron` the way
/// `main.rs` loads it, since it names no seed and is the same for every
/// run.
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
        crate::upgrades::load_abilities(kinds, registries)
    };
    app.insert_resource(abilities);
    app.add_plugins(UiPlugin);
    app.add_plugins(crate::plugin::FoundryPlugin);
    app
}

/// `Struck` messages copied out as they are written, the way the engine's
/// own combat tests keep them: a headless app rotates its message buffers
/// on wall time, so reading them straight off `Messages<Struck>` after
/// `update()` can miss what a reader added this same run would have
/// caught.
#[derive(Resource, Default)]
struct StruckLog(Vec<Struck>);

/// Copies every `Struck` written this frame onto the end of `StruckLog`.
fn collect_struck(mut events: MessageReader<Struck>, mut log: ResMut<StruckLog>) {
    log.0.extend(events.read().copied());
}

/// Places a target three tiles east of `shooter`, on floor stamped clear
/// for it so the shot always has a line regardless of what the deck
/// generated there, and sends `shots` attacks at it one at a time,
/// returning every `Struck` the shooter wrote, in order. Only the
/// shooter's: the commando's lamp lets a deck's droids see it, and a
/// droid that closes in and fights back writes `Struck`s of its own.
///
/// The target carries no `Actor`, so it never enters the turn queue and
/// never acts: a still target for a test that cares only about what the
/// shooter's own weapon does.
pub fn fire_at_a_target(app: &mut App, shooter: Entity, shots: usize) -> Vec<Struck> {
    if !app.world().contains_resource::<StruckLog>() {
        app.init_resource::<StruckLog>();
        app.add_systems(PostUpdate, collect_struck);
    }
    let pos = app.world().get::<Position>(shooter).copied().expect("the shooter stands somewhere");
    let floor = app.world().resource::<WorldMap>().tile(pos.0).expect("the shooter's own tile is loaded");
    let at = pos.0.offset(3, 0);
    {
        let mut map = app.world_mut().resource_mut::<WorldMap>();
        for dx in 1..=3 {
            map.set_tile(pos.0.offset(dx, 0), floor);
        }
    }
    let target = app.world_mut().spawn((Blocks, Position(at), Health::full(10_000))).id();
    for _ in 0..shots {
        app.world_mut().write_message(Intent::new(shooter, Attack(target)));
        app.update();
    }
    app.world_mut().resource_mut::<StruckLog>().0.drain(..).filter(|s| s.attacker == shooter).collect()
}

/// Writes a `DamageDealt` of `kind` dealing `amount` straight to `target`,
/// bypassing combat entirely, and runs the turn that lets whatever reacts
/// to it react.
pub fn hit(app: &mut App, target: Entity, kind: &str, amount: i32) {
    let registries = app.world().resource::<Registries>().clone();
    let kind = registries.damage_kinds.expect(kind);
    app.world_mut().write_message(DamageDealt { target, hit: Hit::from_source(None, kind, amount), dealt: amount });
    app.update();
}

/// Two updates: enough for one resolved action and whatever it triggers to
/// settle, the way most of this module's own helpers already open with.
pub fn settle(app: &mut App) {
    app.update();
    app.update();
}

/// Waits the player forward `n` whole turns, one at a time.
pub fn pass_turns(app: &mut App, n: u32) {
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    for _ in 0..n {
        app.world_mut().write_message(Intent::new(player, Wait));
        app.update();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_headless_run_wires_every_plugin_the_harness_names() {
        let _app = headless(RunSeed(7));
    }
}
