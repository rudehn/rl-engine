//! Monsters, spawned for a test to face or inspect.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::Point;
use rl_engine::rl_grid::Rgb;

use crate::droids::Roster;

/// Spawns `name` `range` tiles east of the player, on floor stamped clear
/// for it the way [`super::fire_at_a_target`] clears its own line, so its
/// brain always has a shot regardless of what the deck generated there.
/// Also lights the player's own tile: the foundry's decks are otherwise
/// pitch dark (`run::start` inserts `Lighting::dark()`), and a monster
/// with no `DarkSight` of its own, such as a line droid, sees nothing
/// past what it is touching outside of light, spec section 8.2's whole
/// point for radar. Returns the monster, then the player.
pub fn droid_facing_player(app: &mut App, name: &str, range: i32) -> (Entity, Entity) {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let roster = Roster::load(&registries);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let pos = app.world().get::<Position>(player).copied().expect("the player stands somewhere");
    let map = app.world().resource::<WorldMap>().current();
    let floor = app.world().resource::<WorldMap>().tile(pos.0).expect("the player's own tile is loaded");
    {
        let mut world_map = app.world_mut().resource_mut::<WorldMap>();
        for dx in 1..=range {
            world_map.set_tile(pos.0.offset(dx, 0), floor);
        }
    }
    let id = roster.defs.expect(name);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    commands.spawn((Position(pos.0), LightSource::new(255, range + 2, Rgb::WHITE)));
    let droid = crate::droids::spawn_monster(&mut commands, &roster, id, pos.0.offset(range, 0), map, &registries);
    queue.apply(app.world_mut());
    app.update();
    (droid, player)
}

/// What one `Struck` a monster caused looked like, for a test that only
/// cares about a monster's own attack rather than an item's economy.
pub struct DroidStruck {
    /// Whether it was a shot rather than a blow in reach.
    pub ranged: bool,
    /// Whom it hit.
    pub target: Entity,
}

/// Runs the app forward up to `max_turns` whole turns, waiting the player
/// forward one at a time so the scheduler moves past its held turn onto
/// whatever else is due, and watching for a `Struck` `attacker` caused,
/// returning it the moment one lands.
///
/// Panics if `attacker` never strikes within `max_turns`: a brain wired
/// wrong so it never shoots is a test failure, not a silent value the
/// caller might overlook.
pub fn run_until_struck(app: &mut App, attacker: Entity, max_turns: usize) -> DroidStruck {
    if !app.world().contains_resource::<super::StruckLog>() {
        app.init_resource::<super::StruckLog>();
        app.add_systems(PostUpdate, super::collect_struck);
    }
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    for _ in 0..max_turns {
        app.world_mut().write_message(Intent::new(player, Wait));
        app.update();
        let mut log = app.world_mut().resource_mut::<super::StruckLog>();
        if let Some(i) = log.0.iter().position(|s| s.attacker == attacker) {
            let struck = log.0.remove(i);
            return DroidStruck { ranged: struck.ranged, target: struck.target };
        }
        log.0.clear();
    }
    panic!("{attacker:?} never struck within {max_turns} turns");
}

/// A probe carrying `Alarm`, the player, and two line droids elsewhere on
/// the same deck that have noticed nothing, for a test of `sound_alarm`.
/// Returns the probe, then the player, then the two sleepers.
pub fn probe_and_sleepers(app: &mut App) -> (Entity, Entity, Vec<Entity>) {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let roster = Roster::load(&registries);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let pos = app.world().get::<Position>(player).copied().expect("the player stands somewhere");
    let map = app.world().resource::<WorldMap>().current();
    let probe_id = roster.defs.expect("probe droid");
    let line_id = roster.defs.expect("line droid");
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    let probe = crate::droids::spawn_monster(&mut commands, &roster, probe_id, pos.0, map, &registries);
    let a = crate::droids::spawn_monster(&mut commands, &roster, line_id, pos.0, map, &registries);
    let b = crate::droids::spawn_monster(&mut commands, &roster, line_id, pos.0, map, &registries);
    queue.apply(app.world_mut());
    (probe, player, vec![a, b])
}

/// Spawns `name` alone on the current deck, at no point that matters to the
/// caller.
pub fn lone_monster(app: &mut App, name: &str) -> Entity {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let roster = Roster::load(&registries);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let pos = app.world().get::<Position>(player).copied().expect("the player stands somewhere");
    let map = app.world().resource::<WorldMap>().current();
    let id = roster.defs.expect(name);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    let monster = crate::droids::spawn_monster(&mut commands, &roster, id, pos.0, map, &registries);
    queue.apply(app.world_mut());
    monster
}

/// What walking from `from` to `to` on the current deck costs, in
/// hundredths of a step, round the walls and never through a shut door.
///
/// Never less than what a sound spends on the same way, which is how a
/// test places a listener in earshot: sound goes by the rooms, not
/// through them.
pub fn walk(app: &App, from: Point, to: Point) -> i32 {
    let map = app.world().resource::<WorldMap>();
    let view = map.view();
    let mut flood = rl_engine::rl_grid::DijkstraMap::covering(&view);
    flood.build(&view, map.to_local(to), rl_engine::rl_grid::PathRules::EIGHT_WAY);
    map.to_local(from).and_then(|l| flood.value(l)).unwrap_or(i32::MAX)
}

/// Spawns `name` on the current deck somewhere the player has no line to,
/// `min..=max` hundredths of a step's walk from the player: out of sight,
/// so whatever it learns of the player's corner of the deck it learns by
/// ear. Returns the monster and where it stands.
pub fn out_of_sight(app: &mut App, name: &str, min: i32, max: i32) -> (Entity, Point) {
    app.update();
    app.update();
    let registries = app.world().resource::<Registries>().clone();
    let roster = Roster::load(&registries);
    let player = app.world_mut().query_filtered::<Entity, With<Player>>().single(app.world()).unwrap();
    let from = app.world().get::<Position>(player).copied().expect("the player stands somewhere").0;
    let at = {
        let world = app.world();
        let (deck, occupancy, sight) = (world.resource::<WorldMap>(), world.resource::<Occupancy>(), world.get::<Viewshed>(player));
        // One flood from the player, read at every cell.
        let view = deck.view();
        let mut flood = rl_engine::rl_grid::DijkstraMap::covering(&view);
        flood.build(&view, deck.to_local(from), rl_engine::rl_grid::PathRules::EIGHT_WAY);
        let bounds = deck.window_tiles();
        (bounds.y..bounds.bottom())
            .flat_map(|y| (bounds.x..bounds.right()).map(move |x| Point::new(x, y)))
            .find(|p| {
                let steps = deck.to_local(*p).and_then(|l| flood.value(l)).unwrap_or(i32::MAX);
                deck.is_walkable(*p) && !occupancy.is_occupied(*p) && !sight.is_some_and(|v| v.in_line(*p)) && (min..=max).contains(&steps)
            })
            .expect("a corner of the deck out of the player's line and within the walk asked for")
    };
    let map = app.world().resource::<WorldMap>().current();
    let id = roster.defs.expect(name);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    let monster = crate::droids::spawn_monster(&mut commands, &roster, id, at, map, &registries);
    queue.apply(app.world_mut());
    (monster, at)
}
