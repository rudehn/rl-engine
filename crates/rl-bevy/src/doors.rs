//! Doors: tiles that open and close.
//!
//! A door is any tile whose [`TileProps`](rl_grid::TileProps) names what it
//! opens or closes into; the standard registry's `door_closed` and
//! `door_open` are one such pair and a game registers as many as it likes.
//! Opening is a step: walking into a closed door opens it and spends the
//! turn where the actor stands, resolved with every other step in
//! [`resolve_moves`](crate::turn::resolve_moves). Closing is [`Close`], here.
//!
//! Both need the wits for it, [`Wits::OPENS_DOORS`]. An actor carrying no
//! [`Intelligence`] at all, the player unless a game says otherwise, has
//! them, so a game that never flags anyone keeps doors working for
//! everyone. A mind that opens doors paths through the map as
//! [`WorldMap::opening_view`] reads it, so its flow fields go through a
//! door when that is shorter; one that cannot treats a closed door as wall.

use bevy::prelude::*;
use rl_core::turn::BASE_ACTION_COST;
use rl_core::{Direction, Point};
use rl_rules::Wits;

use crate::components::{MyTurn, Position};
use crate::minds::Intelligence;
use crate::places::{MapId, OnMap};
use crate::turn::{Action, Intent, Occupancy, Resolution};
use crate::world::WorldMap;

/// Close the door in the next cell in this direction.
///
/// Refused when there is nothing there that closes, or when anything at all
/// stands or lies in the doorway: a door does not close on a body, a dropped
/// sword or a lantern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Close(pub Direction);
impl Action for Close {}

/// A door was opened or closed, for narration.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoorEvent {
    /// `actor` opened the door at `at`.
    Opened {
        /// Who.
        actor: Entity,
        /// Where.
        at: Point,
    },
    /// `actor` closed the door at `at`.
    Closed {
        /// Who.
        actor: Entity,
        /// Where.
        at: Point,
    },
}

/// Whether an actor with `intelligence` may open and close doors: when it
/// has the wits, or carries none.
pub fn works_doors(intelligence: Option<&Intelligence>) -> bool {
    intelligence.is_none_or(|i| i.0.has(Wits::OPENS_DOORS))
}

/// Anything with a position, for asking whether a doorway is clear.
type Lying<'w, 's> = Query<'w, 's, (&'static Position, Option<&'static OnMap>)>;

/// Resolves [`Close`] for the actor holding the turn.
pub fn resolve_closes(
    mut intents: MessageReader<Intent<Close>>,
    mut resolution: Resolution,
    mut map: ResMut<WorldMap>,
    occupancy: Res<Occupancy>,
    closers: Query<(&Position, Option<&Intelligence>), With<MyTurn>>,
    lying: Lying,
    mut doors: MessageWriter<DoorEvent>,
) {
    let here = map.current();
    for intent in intents.read() {
        let actor = intent.actor;
        if !resolution.claim(actor) {
            continue;
        }
        let Ok((pos, intelligence)) = closers.get(actor) else {
            resolution.failed(actor, BASE_ACTION_COST);
            continue;
        };
        let at = pos.0 + intent.action.0.offset();
        let in_the_way = occupancy.is_occupied(at) || lying.iter().any(|(p, on)| p.0 == at && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here);
        match map.closes(at) {
            Some(shut) if works_doors(intelligence) && !in_the_way => {
                map.set_tile(at, shut);
                doors.write(DoorEvent::Closed { actor, at });
                resolution.done(actor, BASE_ACTION_COST);
            }
            _ => resolution.failed(actor, BASE_ACTION_COST),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, Player, RevealsMap, Viewshed};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::{Step, Turns};
    use rl_grid::TileRegistry;

    /// Every door event, recorded by a reader: when a headless app swaps its
    /// message buffers depends on wall time, so a peek can miss one.
    #[derive(Resource, Default)]
    struct Heard(Vec<DoorEvent>);

    fn hear(mut events: MessageReader<DoorEvent>, mut heard: ResMut<Heard>) {
        heard.0.extend(events.read().copied());
    }

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        tiles: TileRegistry,
    }

    impl Rig {
        /// A player on open ground with the door east of it set to `door`.
        fn new(door: &str) -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin));
            app.init_resource::<Heard>().add_systems(PostUpdate, hear);
            let start = crate::testing::surface(&mut app);
            let tiles = TileRegistry::standard();
            let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap)).id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            let mut rig = Self { app, player, start, tiles };
            rig.set(rig.door(), door);
            rig.app.update();
            rig
        }

        fn door(&self) -> Point {
            self.start.offset(1, 0)
        }

        fn set(&mut self, at: Point, tile: &str) {
            let id = self.tiles.expect(tile);
            assert!(self.app.world_mut().resource_mut::<WorldMap>().set_tile(at, id));
        }

        fn is(&self, at: Point, tile: &str) -> bool {
            self.app.world().resource::<WorldMap>().tile(at) == Some(self.tiles.expect(tile))
        }

        fn act<A: Action>(&mut self, action: A) {
            self.app.world_mut().write_message(Intent::new(self.player, action));
            self.app.update();
        }

        fn now(&self) -> u32 {
            self.app.world().resource::<Turns>().now()
        }

        fn at(&self) -> Point {
            self.app.world().get::<Position>(self.player).unwrap().0
        }
    }

    #[test]
    fn walking_into_a_closed_door_opens_it_and_spends_the_turn_where_you_stand() {
        let mut rig = Rig::new("door_closed");
        let beyond = rig.start.offset(3, 0);
        assert!(!rig.app.world().get::<Viewshed>(rig.player).unwrap().can_see(beyond), "the closed door hides what is past it");

        rig.act(Step(Direction::East));
        assert!(rig.is(rig.door(), "door_open"));
        assert_eq!(rig.at(), rig.start, "opening is not stepping");
        assert_eq!(rig.now(), 100, "and it took the turn");
        assert_eq!(rig.app.world().resource::<Heard>().0, vec![DoorEvent::Opened { actor: rig.player, at: rig.door() }]);
        assert!(rig.app.world().get::<Viewshed>(rig.player).unwrap().can_see(beyond), "and what was past it is in sight");

        rig.act(Step(Direction::East));
        assert_eq!(rig.at(), rig.door(), "the next step goes through");
    }

    #[test]
    fn a_door_closes_from_beside_it_unless_something_lies_in_the_doorway() {
        let mut rig = Rig::new("door_open");
        let door = rig.door();
        let lantern = rig.app.world_mut().spawn(Position(door)).id();
        rig.app.update();

        rig.act(Close(Direction::East));
        assert!(rig.is(door, "door_open"), "it will not close on what lies there");
        assert_eq!(rig.now(), 0, "and trying cost nothing");

        rig.app.world_mut().entity_mut(lantern).despawn();
        rig.act(Close(Direction::East));
        assert!(rig.is(door, "door_closed"));
        assert_eq!(rig.now(), 100);
        assert_eq!(rig.app.world().resource::<Heard>().0, vec![DoorEvent::Closed { actor: rig.player, at: door }]);

        rig.act(Close(Direction::East));
        assert_eq!(rig.now(), 100, "a closed door has nothing left to close, and costs nothing to try");
        rig.act(Close(Direction::West));
        assert_eq!(rig.now(), 100, "nor does open ground");
    }

    #[test]
    fn an_actor_without_the_wits_for_doors_is_stopped_by_one() {
        let mut rig = Rig::new("door_closed");
        rig.app.world_mut().entity_mut(rig.player).insert(Intelligence(Wits::ANIMAL));
        rig.act(Step(Direction::East));
        assert!(rig.is(rig.door(), "door_closed"), "a paw does not work a latch");
        assert_eq!((rig.at(), rig.now()), (rig.start, 0), "refused, where it stood, for nothing");

        rig.set(rig.door(), "door_open");
        rig.act(Close(Direction::East));
        assert!(rig.is(rig.door(), "door_open"), "nor close one");
    }
}
