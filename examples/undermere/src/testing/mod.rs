//! A headless Undermere, for the tests this crate adds.
//!
//! The engine plugins the places need, with no window, and the places' own
//! systems as the game orders them: the ways through and the fixtures'
//! light laid on a first arrival after the engine has filled a piece's
//! slots, and each place's ambient set between the turns and the light.
//! Nothing begins a run yet, so a test stands a body in for the
//! adventurer, with no side and no light of its own, wherever it wants one
//! with [`arrive`].

use bevy::prelude::*;
use rl_engine::prelude::*;
use rl_engine::rl_core::RunSeed;

use crate::armory::Armory;
use crate::bestiary::Bestiary;
use crate::content::Library;
use crate::dungeon::Undermere;
use crate::light::Fixtures;

/// A world with Undermere's places in it and nobody in them, seeded.
///
/// Combat and minds are there for the throne's court, which the engine
/// stands at its posts on floor ten's first arrival: a monster nobody
/// decides for would hold its turn for good. The narrator is there for the
/// log, which every line a place says reaches through a `Tell`.
pub fn headless(seed: RunSeed) -> App {
    let mut app = rl_engine::rl_bevy::plugin::headless_app();
    app.add_plugins((FovPlugin, LightingPlugin, CombatPlugin, MindsPlugin));
    let library = Library::load(&crate::content::effect_kinds(), &Moments::default());
    let undermere = Undermere::new(seed);
    let fixtures = Fixtures::new(&undermere, &library.registries);
    let combat = crate::content::combat_rules(&library.registries);
    app.insert_resource(Seed(seed))
        .insert_resource(combat)
        .insert_resource(WorldMap::new(undermere.tiles().tables()))
        .insert_resource(undermere.prefabs().clone())
        .insert_resource(fixtures)
        .insert_resource(library.registries)
        .insert_resource(library.bestiary)
        .insert_resource(library.armory)
        .insert_resource(PlaceRulesRes(Box::new(undermere)));
    app.add_plugins(PrefabPlugin::<Bestiary, Armory>::default());
    app.add_plugins((UiPlugin, NarratorPlugin::default()));
    // Chained, the ways before the light, and after the slots are filled,
    // so a place's entities are spawned in one order every run.
    app.add_systems(Turn, (crate::stairs::link_places, crate::light::light_fixtures).chain().in_set(TurnSet::React).after(PrefabSet::Fill));
    app.add_systems(Update, crate::light::set_ambient.after(EngineSet::Turns).before(EngineSet::Light).run_if(in_state(EngineState::Playing)));
    app
}

/// Two updates: enough for one resolved action and whatever it sets off
/// to settle.
pub fn settle(app: &mut App) {
    app.update();
    app.update();
}

/// The body standing in for the adventurer, if one has been put anywhere.
pub fn player(app: &mut App) -> Option<Entity> {
    app.world_mut().query_filtered::<Entity, With<Player>>().iter(app.world()).next()
}

/// Puts the body at `map`'s way in, making it first if there is none, the
/// way a run warps the adventurer into Mereholt. Returns the body.
pub fn arrive(app: &mut App, map: MapId) -> Entity {
    let me = match player(app) {
        Some(me) => me,
        None => {
            let me = app.world_mut().spawn((Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(8), RevealsMap)).id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            settle(app);
            me
        }
    };
    app.world_mut().write_message(WarpRequest::into_place(me, map));
    settle(app);
    me
}

/// Moves the body to `at` on the map it is on, through the engine's own
/// warp, so what it stood on and what it stands on are both kept right.
pub fn stand_on(app: &mut App, me: Entity, at: Point) {
    let here = app.world().resource::<WorldMap>().current();
    app.world_mut().write_message(WarpRequest { actor: me, to: Destination::Place { map: here, arrive: Arrive::At(at) } });
    settle(app);
}

/// Takes the way through the body stands on.
pub fn go_through(app: &mut App, me: Entity) {
    app.world_mut().write_message(Intent::new(me, GoThrough));
    settle(app);
}
