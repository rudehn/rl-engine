//! Doors: tiles that open and close.
//!
//! A door is any tile whose [`TileProps`](rl_grid::TileProps) names what it
//! opens or closes into; the standard registry's `door_closed` and
//! `door_open` are one such pair and a game registers as many as it likes.
//! [`Open`] and `Close` are actions of their own, spent where the actor
//! stands, because working a door is not stepping: what blocks sight
//! changes, so light and every viewshed are recast, and a step into a shut
//! door is a step into a wall. A walk key that should open the door in its
//! way writes [`Bump`](crate::bump::Bump), which comes to an [`Open`] when
//! there is one to open; a mind that reaches a door writes [`Open`] itself.
//!
//! Both need the wits for it, [`Wits::OPENS_DOORS`]. An actor carrying no
//! [`Intelligence`] at all, the player unless a game says otherwise, has
//! them, so a game that never flags anyone keeps doors working for
//! everyone. A mind that opens doors paths through the map as
//! [`WorldMap::opening_view`] reads it, so its flow fields go through a
//! door when that is shorter; one that cannot treats a closed door as wall.
//!
//! A place may lock a door to a tag, or seal it, with a
//! [`Lock`](crate::places::Lock): it opens only for an opener carrying the
//! tag, and a seal for nobody until the game lifts it.

use bevy::prelude::*;
use rl_core::turn::BASE_ACTION_COST;
use rl_core::{Direction, Point};
use rl_rules::Wits;

use crate::components::{MyTurn, Position};
use crate::items::{Inventory, Tagged, carries_tag};
use crate::minds::Intelligence;
use crate::places::{MapId, OnMap};
use crate::turn::{Action, Intent, Occupancy, Resolution};
use crate::world::WorldMap;

/// Open the door in the next cell in this direction.
///
/// Refused when there is nothing there that opens, or the actor lacks the
/// wits to. The actor stays where it stands: going through is the next step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Open(pub Direction);
impl Action for Open {}

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
pub(crate) fn works_doors(intelligence: Option<&Intelligence>) -> bool {
    intelligence.is_none_or(|i| i.0.has(Wits::OPENS_DOORS))
}

/// Anything with a position, for asking whether a doorway is clear.
type Lying<'w, 's> = Query<'w, 's, (&'static Position, Option<&'static OnMap>)>;

/// Who may be opening a door, and what they carry.
type Openers<'w, 's> = Query<'w, 's, (&'static Position, Option<&'static Intelligence>, Option<&'static Tagged>, Option<&'static Inventory>), With<MyTurn>>;

/// Resolves [`Open`] for the actor holding the turn.
///
/// A door under a [`Lock`](crate::places::Lock) opens only for an opener
/// carrying its tag, by the rule an offer's need is asked by too, and
/// spends the lock when it does; a seal opens for nobody. Refused, either
/// is the free refusal any door that will not open is, so the player is
/// not charged for trying a key it lacks.
pub(crate) fn resolve_opens(
    mut intents: MessageReader<Intent<Open>>,
    mut resolution: Resolution,
    mut map: ResMut<WorldMap>,
    openers: Openers,
    tagged: Query<&Tagged>,
    mut doors: MessageWriter<DoorEvent>,
) {
    for intent in intents.read() {
        let actor = intent.actor;
        if !resolution.claim(actor) {
            continue;
        }
        let Ok((pos, intelligence, own, bag)) = openers.get(actor) else {
            resolution.failed(actor, BASE_ACTION_COST);
            continue;
        };
        let at = pos.0 + intent.action.0.offset();
        let lock = map.lock(at);
        let unlocked = lock.is_none_or(|l| l.key.is_some_and(|key| carries_tag(key, own, bag, |item| tagged.get(item).ok())));
        match map.opens(at) {
            Some(open) if works_doors(intelligence) && unlocked => {
                if lock.is_some() {
                    let here = map.current();
                    map.unlock(here, at);
                }
                map.set_tile(at, open);
                doors.write(DoorEvent::Opened { actor, at });
                resolution.done(actor, BASE_ACTION_COST);
            }
            _ => resolution.failed(actor, BASE_ACTION_COST),
        }
    }
}

/// Resolves [`Close`] for the actor holding the turn.
pub(crate) fn resolve_closes(
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
        let in_the_way = occupancy.is_occupied(at) || lying.iter().any(|(p, on)| p.0 == at && on.map(|m| m.0).unwrap_or(MapId::ZERO) == here);
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
    use crate::state::EngineState;
    use crate::testing::headless_app;
    use crate::turn::{Step, Turns};
    use rl_grid::TileRegistry;
    use rl_rules::TagId;

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
            app.add_plugins(crate::fov::FovPlugin);
            app.init_resource::<Heard>().add_systems(PostUpdate, hear);
            let start = crate::testing::ground(&mut app);
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
    fn opening_a_closed_door_spends_the_turn_where_you_stand_and_a_step_into_one_is_refused() {
        let mut rig = Rig::new("door_closed");
        let beyond = rig.start.offset(3, 0);
        assert!(!rig.app.world().get::<Viewshed>(rig.player).unwrap().can_see(beyond), "the closed door hides what is past it");

        rig.act(Step(Direction::East));
        assert!(rig.is(rig.door(), "door_closed"), "a step is a step: it does not work a latch");
        assert_eq!((rig.at(), rig.now()), (rig.start, 0), "refused, for nothing");

        rig.act(Open(Direction::East));
        assert!(rig.is(rig.door(), "door_open"));
        assert_eq!(rig.at(), rig.start, "opening is not stepping");
        assert_eq!(rig.now(), 100, "and it took the turn");
        assert_eq!(rig.app.world().resource::<Heard>().0, vec![DoorEvent::Opened { actor: rig.player, at: rig.door() }]);
        assert!(rig.app.world().get::<Viewshed>(rig.player).unwrap().can_see(beyond), "and what was past it is in sight");

        rig.act(Step(Direction::East));
        assert_eq!(rig.at(), rig.door(), "the next step goes through");
        rig.act(Open(Direction::East));
        assert_eq!(rig.now(), 200, "open ground has nothing to open, and costs nothing to try");
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

    /// A mover that opens doors paths over the opening view, where a closed
    /// door can be entered at its opening's cost; the way it is handed past
    /// the door's corner is still one the move resolver takes, since both
    /// ask one corner rule, and never a diagonal it refuses every turn.
    #[test]
    fn a_way_through_the_opening_view_never_squeezes_past_a_closed_doors_corner() {
        let mut rig = Rig::new("door_closed");
        // The door is east; the goal north of it, one diagonal away past
        // the door's corner, or two straight steps round it.
        let goal = rig.door().offset(0, -1);
        let step = {
            let map = rig.app.world().resource::<WorldMap>();
            let view = map.opening_view();
            let mut flood = rl_grid::DijkstraMap::covering(&view);
            flood.build(&view, [map.to_local(goal).unwrap()], rl_grid::PathRules::default());
            let next = flood.descend(map.to_local(rig.start).unwrap()).expect("a way to the goal");
            Direction::between(map.to_local(rig.start).unwrap(), next).expect("a neighbour")
        };
        rig.act(Step(step));
        assert_ne!(rig.at(), rig.start, "the step the way began with, {step:?}, is taken, not refused");
    }

    #[test]
    fn an_actor_without_the_wits_for_doors_is_stopped_by_one() {
        let mut rig = Rig::new("door_closed");
        rig.app.world_mut().entity_mut(rig.player).insert(Intelligence(Wits::ANIMAL));
        rig.act(Open(Direction::East));
        assert!(rig.is(rig.door(), "door_closed"), "a paw does not work a latch");
        assert_eq!((rig.at(), rig.now()), (rig.start, 0), "refused, where it stood, for nothing");

        rig.set(rig.door(), "door_open");
        rig.act(Close(Direction::East));
        assert!(rig.is(rig.door(), "door_open"), "nor close one");
    }

    /// A place with one door, joined `join` as growth joins a piece, and a
    /// player beside it carrying a key tagged `carried`, if anything.
    struct Vault {
        app: App,
        player: Entity,
        door: Point,
        tiles: TileRegistry,
    }

    const KEY: u32 = 3;
    const PLACE: MapId = MapId(1);

    impl Vault {
        fn new(join: rl_mapgen::Join, carried: Option<u32>) -> Self {
            use rl_mapgen::{BaseContext, BuildContext, Opening};
            let mut app = headless_app();
            let mut tiles = TileRegistry::standard();
            tiles.register(rl_grid::TileProps::wall("vault door").opens_to("door_open")).unwrap();
            let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
            let mut ctx = BaseContext::blank(9, 3, tiles.clone(), wall);
            for x in 1..8 {
                ctx.terrain_mut().set(Point::new(x, 1), floor);
            }
            let (start, door) = (Point::new(2, 1), Point::new(3, 1));
            ctx.terrain_mut().set(door, tiles.expect("vault door"));
            ctx.emit(Opening { cells: vec![door], facing: Direction::East, areas: (0, 1), join });
            ctx.emit(rl_mapgen::passes::StartPoint(start));
            let build = crate::places::PlaceBuild::from_context(ctx).unwrap();
            let mut world = WorldMap::new(tiles.tables());
            world.install_place(PLACE, build);
            world.switch_to(PLACE);
            app.insert_resource(world);
            let bag = Inventory { items: carried.map(|tag| app.world_mut().spawn(Tagged(vec![TagId::from_raw(tag)])).id()).into_iter().collect() };
            let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), OnMap(PLACE), Viewshed::new(8), bag)).id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            Self { app, player, door, tiles }
        }

        fn open(&mut self) {
            self.app.world_mut().write_message(Intent::new(self.player, Open(Direction::East)));
            self.app.update();
        }

        fn is(&self, tile: &str) -> bool {
            self.app.world().resource::<WorldMap>().tile(self.door) == Some(self.tiles.expect(tile))
        }

        fn now(&self) -> u32 {
            self.app.world().resource::<Turns>().now()
        }
    }

    #[test]
    fn a_locked_join_opens_only_for_an_opener_carrying_its_tag_and_opened_stays_open() {
        for carried in [None, Some(KEY + 1)] {
            let mut vault = Vault::new(rl_mapgen::Join::Locked(KEY), carried);
            vault.open();
            assert!(vault.is("vault door"), "carrying {carried:?}: it stays shut");
            assert_eq!(vault.now(), 0, "carrying {carried:?}: and trying cost nothing");
        }
        let mut vault = Vault::new(rl_mapgen::Join::Locked(KEY), Some(KEY));
        vault.open();
        assert!(vault.is("door_open"), "the key's carrier opens it");
        assert_eq!(vault.now(), 100);
        assert_eq!(vault.app.world().resource::<WorldMap>().lock(vault.door), None, "and the lock is spent");

        // Tagged itself, with no key in its bag: a game's say-so is as good as a key.
        let mut warden = Vault::new(rl_mapgen::Join::Locked(KEY), None);
        warden.app.world_mut().entity_mut(warden.player).insert(Tagged(vec![TagId::from_raw(KEY)]));
        warden.open();
        assert!(warden.is("door_open"), "an opener tagged with it opens it");
    }

    #[test]
    fn a_sealed_join_opens_for_nobody_until_the_game_lifts_it() {
        let mut vault = Vault::new(rl_mapgen::Join::Sealed, Some(KEY));
        vault.open();
        assert!(vault.is("vault door"), "no key opens a seal");
        let door = vault.door;
        let lifted = vault.app.world_mut().resource_mut::<WorldMap>().unlock(PLACE, door);
        assert_eq!(lifted, Some(crate::places::Lock { at: door, key: None }));
        vault.open();
        assert!(vault.is("door_open"), "lifted, it opens like any door");
    }
}
