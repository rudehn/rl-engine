//! Bump: the action that becomes another.
//!
//! Every roguelike's walk key means three things: step onto open ground,
//! open the door in the way, strike the foe standing there. Nine copies of
//! the same match on [`Occupancy::first_at`] in nine input systems is the
//! signal that the choice is the engine's to make, so [`Bump`] is an action
//! whose whole job is to resolve to one of [`Step`], [`Open`] or [`Attack`].
//!
//! It does so in [`ResolveSet::Redirect`](crate::plugin::ResolveSet::Redirect),
//! the stage before any resolver claims a turn: the bump is read, the
//! intent it stands for is written, and the resolver that owns that intent
//! claims the turn as it always does. That is the general shape of an
//! alternate action, and a game's own follows it: read the intent in
//! `Redirect`, write the one it comes to, claim nothing. A bump into someone
//! who is neither open ground nor a foe is refused the way a step into a
//! wall is, and reported as [`Bumped`] for the narrator.
//!
//! Opt-in by being written: a game that wants its walk key to mean a step
//! and nothing more writes [`Step`] as before.

use bevy::prelude::*;
use rl_core::Direction;
use rl_core::turn::BASE_ACTION_COST;
use rl_rules::Relation;

use crate::combat::{Attack, CombatRules, Faction, Health};
use crate::components::{MyTurn, Position};
use crate::doors::Open;
use crate::turn::{Action, Intent, Occupancy, Resolution, Step};
use crate::world::WorldMap;

/// Walk this way, whatever that comes to: a step, opening a door, or a
/// blow at a foe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bump(pub Direction);
impl Action for Bump {}

/// An actor bumped into someone it will not strike: an ally, a neutral, or
/// anyone at all in a game with no sides. The turn was kept.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bumped {
    /// Who bumped.
    pub actor: Entity,
    /// Whom.
    pub into: Entity,
}

/// Who stands where, and how they stand to one another.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Way<'w, 's> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    rules: Option<Res<'w, CombatRules>>,
    bumpers: Query<'w, 's, (&'static Position, Option<&'static Faction>), With<MyTurn>>,
    standing: Query<'w, 's, Option<&'static Faction>, With<Health>>,
}

/// What a bump writes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Alternates<'w> {
    steps: MessageWriter<'w, Intent<Step>>,
    opens: MessageWriter<'w, Intent<Open>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
    bumped: MessageWriter<'w, Bumped>,
}

/// Turns each bump into the intent it stands for.
///
/// Nothing is claimed here except a bump that comes to nothing, so the
/// resolver of whatever was written spends the turn, and the sweep finds it
/// spent.
pub fn redirect_bumps(mut intents: MessageReader<Intent<Bump>>, mut resolution: Resolution, way: Way, mut out: Alternates) {
    for intent in intents.read() {
        let actor = intent.actor;
        let Ok((pos, side)) = way.bumpers.get(actor) else { continue };
        let dir = intent.action.0;
        let target = pos.0 + dir.offset();
        // Whoever stands there and can be struck, by the rule the attack
        // resolver strikes by: alive, with health to lose.
        let foe = way.occupancy.at(target).iter().copied().find(|other| way.standing.contains(*other));
        match foe {
            Some(other) if hostile(way.rules.as_deref(), side, way.standing.get(other).ok().flatten()) => {
                out.attacks.write(Intent::new(actor, Attack(other)));
            }
            Some(other) => {
                if resolution.claim(actor) {
                    resolution.failed(actor, BASE_ACTION_COST);
                    out.bumped.write(Bumped { actor, into: other });
                }
            }
            None if way.map.opens(target).is_some() => {
                out.opens.write(Intent::new(actor, Open(dir)));
            }
            None => {
                out.steps.write(Intent::new(actor, Step(dir)));
            }
        }
    }
}

/// Whether `other` is someone `me` strikes on sight: hostile by the rules,
/// when there are rules and both take a side.
fn hostile(rules: Option<&CombatRules>, me: Option<&Faction>, other: Option<&Faction>) -> bool {
    match (rules, me, other) {
        (Some(rules), Some(me), Some(other)) => rules.factions.relation(me.0, other.0) == Relation::Hostile,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, MeleeAttack};
    use crate::components::{Actor, Blocks, Player, Viewshed};
    use crate::doors::DoorEvent;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::Turns;
    use rl_core::{DiceRoll, Point};
    use rl_grid::TileRegistry;

    #[derive(Resource, Default)]
    struct Heard(Vec<Bumped>, Vec<DoorEvent>);

    fn hear(mut bumps: MessageReader<Bumped>, mut doors: MessageReader<DoorEvent>, mut heard: ResMut<Heard>) {
        heard.0.extend(bumps.read().copied());
        heard.1.extend(doors.read().copied());
    }

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        tiles: TileRegistry,
        theirs: rl_rules::FactionId,
        ours: rl_rules::FactionId,
        kind: rl_rules::damage::DamageKindId,
    }

    impl Rig {
        fn new() -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::world::StreamingPlugin));
            app.init_resource::<Heard>().add_systems(PostUpdate, hear);
            let start = crate::testing::surface(&mut app);
            let sides = crate::testing::two_sides(&mut app);
            let tiles = TileRegistry::standard();
            let player = app
                .world_mut()
                .spawn((
                    Actor,
                    Player,
                    Blocks,
                    Position(start),
                    Viewshed::new(8),
                    Health::full(30),
                    Faction(sides.ours),
                    MeleeAttack { kind: sides.kind, dice: DiceRoll::flat(5) },
                ))
                .id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            Self { app, player, start, tiles, theirs: sides.theirs, ours: sides.ours, kind: sides.kind }
        }

        fn bump(&mut self, dir: Direction) {
            self.app.world_mut().write_message(Intent::new(self.player, Bump(dir)));
            self.app.update();
        }

        fn at(&self) -> Point {
            self.app.world().get::<Position>(self.player).unwrap().0
        }

        fn now(&self) -> u32 {
            self.app.world().resource::<Turns>().now()
        }
    }

    /// One key, four outcomes: a step onto ground, a blow at a foe, a door
    /// opened, and a refusal at someone who is not a foe, each costing what
    /// the action it became costs.
    #[test]
    fn a_bump_becomes_a_step_a_blow_an_opening_or_a_refusal() {
        let mut rig = Rig::new();
        let start = rig.start;
        rig.bump(Direction::East);
        assert_eq!((rig.at(), rig.now()), (start.offset(1, 0), 100), "open ground: a step");

        let foe = rig.app.world_mut().spawn((Actor, Blocks, Position(start.offset(2, 0)), Health::full(20), Faction(rig.theirs))).id();
        rig.app.update();
        rig.bump(Direction::East);
        assert_eq!(rig.app.world().get::<Health>(foe).unwrap().hp, 15, "a foe: a blow");
        assert_eq!((rig.at(), rig.now()), (start.offset(1, 0), 200), "and no step");

        let door = start.offset(1, 1);
        let shut = rig.tiles.expect("door_closed");
        assert!(rig.app.world_mut().resource_mut::<WorldMap>().set_tile(door, shut));
        rig.app.update();
        rig.bump(Direction::South);
        assert_eq!(rig.app.world().resource::<WorldMap>().tile(door), Some(rig.tiles.expect("door_open")), "a door: opened");
        assert_eq!((rig.at(), rig.now()), (start.offset(1, 0), 300), "where the actor stood");
        assert_eq!(rig.app.world().resource::<Heard>().1.len(), 1);

        let friend = rig.app.world_mut().spawn((Actor, Blocks, Position(start.offset(0, 0)), Health::full(20), Faction(rig.ours))).id();
        rig.app.update();
        rig.bump(Direction::West);
        assert_eq!((rig.at(), rig.now()), (start.offset(1, 0), 300), "an ally: refused, for nothing");
        assert!(rig.app.world().get::<MyTurn>(rig.player).is_some(), "and the turn is kept");
        assert_eq!(rig.app.world().resource::<Heard>().0, vec![Bumped { actor: rig.player, into: friend }]);
        assert_eq!(rig.app.world().get::<Health>(friend).unwrap().hp, 20);
        let _ = rig.kind;
    }

    /// Without rules to call anyone a foe, nobody is one: a bump into any
    /// actor is a refusal, never a swing.
    #[test]
    fn with_no_sides_a_bump_into_anyone_is_a_refusal() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8))).id();
        let other = app.world_mut().spawn((Actor, Blocks, Position(start.offset(1, 0)), Health::full(5))).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        app.world_mut().write_message(Intent::new(player, Bump(Direction::East)));
        app.update();
        assert_eq!(app.world().get::<Health>(other).unwrap().hp, 5);
        assert_eq!(app.world().resource::<Turns>().now(), 0);
        assert!(app.world().get::<MyTurn>(player).is_some());
    }
}
