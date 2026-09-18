//! Monsters, spawned for a test to face or inspect.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::Point;
use rl_engine::rl_grid::Rgb;
use rl_engine::rl_rules::Awareness;

use crate::droids::Roster;

/// Spawns `name` `range` tiles east of the player, on floor stamped clear
/// for it, the way [`droid_down_a_lane`] does with a lane as long as the
/// range. Returns the monster, then the player.
pub fn droid_facing_player(app: &mut App, name: &str, range: i32) -> (Entity, Entity) {
    droid_down_a_lane(app, name, range, range)
}

/// Spawns `name` `at` tiles east of the player, with the `lane` tiles east
/// of the player stamped to floor the way [`super::fire_at_a_target`]
/// clears its own line, so its brain always has a shot, and room to close
/// or back off, regardless of what the deck generated there. Also lights
/// the player's own tile: the foundry's decks are otherwise pitch dark
/// (`run::start` inserts `Lighting::dark()`), and a monster with no
/// `DarkSight` of its own, such as a line droid, sees nothing past what it
/// is touching outside of light, spec section 8.2's whole point for radar.
/// Returns the monster, then the player.
pub fn droid_down_a_lane(app: &mut App, name: &str, at: i32, lane: i32) -> (Entity, Entity) {
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
        for dx in 1..=lane.max(at) {
            world_map.set_tile(pos.0.offset(dx, 0), floor);
        }
    }
    let id = roster.defs.expect(name);
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, app.world_mut());
    commands.spawn((Position(pos.0), LightSource::new(255, lane.max(at) + 2, Rgb::WHITE)));
    let droid = crate::droids::spawn_monster(&mut commands, &roster, id, pos.0.offset(at, 0), map, &registries);
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

/// Makes `observer` alert to `subject` where `subject` stands, as if it
/// had just noticed it: what a probe that knows where the commando is
/// holds.
pub fn alert(app: &mut App, observer: Entity, subject: Entity) {
    let at = app.world().get::<Position>(subject).expect("the subject stands somewhere").0;
    let mut aware = Aware::default();
    aware.0.insert(subject, Awareness::Alert { at, stale_turns: 0 });
    app.world_mut().entity_mut(observer).insert(aware);
}

/// Every alarm shouted and every cue played, copied out as they are
/// written, for the same reason [`super::fire_at_a_target`] copies out
/// `Struck`: a headless app rotates its buffers on wall time.
#[derive(Resource, Default)]
pub struct Alarms {
    /// Who shouted the alarm, and where, in order.
    pub shouts: Vec<(Entity, Point)>,
    /// Every cue, in order.
    pub cues: Vec<Cued>,
}

/// Copies every alarm and every cue written this frame into [`Alarms`].
fn record_alarms(mut noise: MessageReader<MakeNoise>, mut cues: MessageReader<Cued>, sounds: Res<Sounds>, mut alarms: ResMut<Alarms>) {
    let alarm = sounds.get(crate::droids::ALARM_SOUND);
    alarms.shouts.extend(noise.read().filter(|n| Some(n.sound) == alarm).filter_map(|n| Some((n.maker?, n.at))));
    alarms.cues.extend(cues.read().cloned());
}

/// Starts keeping [`Alarms`], from this frame on.
pub fn record_alarms_from_now(app: &mut App) {
    app.init_resource::<Alarms>();
    app.add_systems(PostUpdate, record_alarms);
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
