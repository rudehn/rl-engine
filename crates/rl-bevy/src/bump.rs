//! Bump: the action that becomes another.
//!
//! Every roguelike's walk key means three things: step onto open ground,
//! open the door in the way, strike the foe standing there. Nine copies of
//! the same match on `Occupancy::first_at` in nine input systems is the
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
//! A bump into an ally is a free refusal unless the game says otherwise:
//! [`BumpRules`] with [`OnAlly::Swap`] makes it a [`Swap`], the two
//! changing places for the cost of the step, which is what every game with
//! a companion wants of its walk key.
//!
//! Opt-in by being written: a game that wants its walk key to mean a step
//! and nothing more writes [`Step`] as before.

use bevy::prelude::*;
use rl_core::turn::BASE_ACTION_COST;
use rl_core::{Direction, Point};
use rl_rules::Relation;

use crate::combat::{Attack, CombatRules, Faction, Health};
use crate::components::{MyTurn, Position, Viewshed};
use crate::doors::Open;
use crate::turn::{Action, Intent, Occupancy, Resolution, Step, corner_ok};
use crate::world::WorldMap;

/// Walk this way, whatever that comes to: a step, opening a door, or a
/// blow at a foe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bump(pub Direction);
impl Action for Bump {}

/// Change places with an adjacent actor.
///
/// What a [`Bump`] into an ally comes to under [`OnAlly::Swap`]. Written
/// directly by a game whose swap key has its own rules about whom; the
/// resolver only asks that the two stand a step apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swap(pub Entity);
impl Action for Swap {}

/// An actor bumped into someone it will not strike: an ally, a neutral, or
/// anyone at all in a game with no sides. The turn was kept.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bumped {
    /// Who bumped.
    pub actor: Entity,
    /// Whom.
    pub into: Entity,
}

/// Two actors changed places.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swapped {
    /// Who moved first.
    pub actor: Entity,
    /// Who moved the other way.
    pub with: Entity,
}

/// What a bump into an ally comes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OnAlly {
    /// A free refusal, reported as [`Bumped`].
    #[default]
    Refuse,
    /// The two change places for the cost of the step.
    Swap,
}

/// What a bump into someone the actor will not strike comes to.
///
/// Optional data: without it every such bump is refused, which is what a
/// game with no companions wants. An ally is whoever [`CombatRules`] calls
/// allied; a neutral, and anyone at all in a game with no sides, is always
/// refused, since walking through a shopkeeper is not what anyone meant.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BumpRules {
    /// A bump into an ally.
    pub allies: OnAlly,
}

impl BumpRules {
    /// Every bump into a non-foe refused.
    pub fn new() -> Self {
        Self::default()
    }

    /// A bump into an ally changes places with them.
    pub fn swap_allies(mut self) -> Self {
        self.allies = OnAlly::Swap;
        self
    }
}

/// Who stands where, and how they stand to one another.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Way<'w, 's> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    rules: Option<Res<'w, CombatRules>>,
    bumps: Option<Res<'w, BumpRules>>,
    bumpers: Query<'w, 's, (&'static Position, Option<&'static Faction>), With<MyTurn>>,
    standing: Query<'w, 's, Option<&'static Faction>, With<Health>>,
}

/// What a bump writes.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Alternates<'w> {
    steps: MessageWriter<'w, Intent<Step>>,
    opens: MessageWriter<'w, Intent<Open>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
    swaps: MessageWriter<'w, Intent<Swap>>,
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
            Some(other) => match relation(way.rules.as_deref(), side, way.standing.get(other).ok().flatten()) {
                Some(Relation::Hostile) => {
                    out.attacks.write(Intent::new(actor, Attack(other)));
                }
                Some(Relation::Allied) if way.bumps.as_deref().is_some_and(|r| r.allies == OnAlly::Swap) => {
                    out.swaps.write(Intent::new(actor, Swap(other)));
                }
                _ => {
                    if resolution.claim(actor) {
                        resolution.failed(actor, BASE_ACTION_COST);
                        out.bumped.write(Bumped { actor, into: other });
                    }
                }
            },
            None if way.map.opens(target).is_some() => {
                out.opens.write(Intent::new(actor, Open(dir)));
            }
            None => {
                out.steps.write(Intent::new(actor, Step(dir)));
            }
        }
    }
}

/// How `other` stands to `me`, when there are rules and both take a side.
fn relation(rules: Option<&CombatRules>, me: Option<&Faction>, other: Option<&Faction>) -> Option<Relation> {
    match (rules, me, other) {
        (Some(rules), Some(me), Some(other)) => Some(rules.factions.relation(me.0, other.0)),
        _ => None,
    }
}

/// Whoever may change places: where they stand, what they see, and
/// whether they block.
type Movers<'w, 's> = Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>, Has<crate::components::Blocks>)>;

/// Resolves a swap: the two change places and both see anew.
///
/// Refused, for free to the player, when the other is not a step away,
/// which is also what stops a swap through a corner a step could not cut.
/// The other keeps its place in the queue; it was moved, not spent.
pub fn resolve_swaps(
    mut intents: MessageReader<Intent<Swap>>,
    mut resolution: Resolution,
    map: Res<WorldMap>,
    mut occupancy: ResMut<Occupancy>,
    holding: Query<(), With<MyTurn>>,
    mut movers: Movers,
    mut swapped: MessageWriter<Swapped>,
) {
    for intent in intents.read() {
        let (actor, other) = (intent.actor, intent.action.0);
        if !holding.contains(actor) || !resolution.claim(actor) {
            continue;
        }
        let Ok([(mut here, my_sight, i_block), (mut there, their_sight, they_block)]) = movers.get_many_mut([actor, other]) else {
            resolution.failed(actor, BASE_ACTION_COST);
            continue;
        };
        let (from, to) = (here.0, there.0);
        let step = to - from;
        let adjacent = step.x.abs() <= 1 && step.y.abs() <= 1 && step != Point::ZERO;
        let dir = Direction::from_delta(step.x, step.y);
        if !adjacent || dir.is_none_or(|d| !corner_ok(&map, from, d)) {
            resolution.failed(actor, BASE_ACTION_COST);
            continue;
        }
        if i_block {
            occupancy.relocate(actor, from, to);
        }
        if they_block {
            occupancy.relocate(other, to, from);
        }
        here.0 = to;
        there.0 = from;
        for sight in [my_sight, their_sight].into_iter().flatten() {
            sight.into_inner().dirty = true;
        }
        let cost = map.cost(to).unwrap_or(BASE_ACTION_COST);
        let cost = if dir.is_some_and(Direction::is_diagonal) { cost * 1414 / 1000 } else { cost };
        swapped.write(Swapped { actor, with: other });
        resolution.done(actor, cost);
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
                    MeleeAttack::new(sides.kind, DiceRoll::flat(5)),
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
        assert_eq!(rig.app.world().get::<Health>(foe).unwrap().current, 15, "a foe: a blow");
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
        assert_eq!(rig.app.world().get::<Health>(friend).unwrap().current, 20);
        let _ = rig.kind;
    }

    /// With the rule set, a bump into an ally changes places: both stand
    /// where the other stood, the index agrees, the turn costs a step, and
    /// the ally is not spent. A neutral is still refused.
    #[test]
    fn with_the_rule_a_bump_into_an_ally_swaps_places_and_a_neutral_is_still_refused() {
        let mut rig = Rig::new();
        rig.app.insert_resource(BumpRules::new().swap_allies());
        let start = rig.start;
        let friend = rig.app.world_mut().spawn((Actor, Blocks, Position(start.offset(1, 0)), Health::full(20), Faction(rig.ours))).id();
        rig.app.update();
        rig.bump(Direction::East);
        assert_eq!(rig.at(), start.offset(1, 0), "the actor stands where the ally stood");
        assert_eq!(rig.app.world().get::<Position>(friend).unwrap().0, start, "and the ally where the actor stood");
        assert_eq!(rig.now(), 100, "for the cost of a step");
        let occupancy = rig.app.world().resource::<Occupancy>();
        assert!(occupancy.at(start).contains(&friend) && occupancy.at(start.offset(1, 0)).contains(&rig.player), "the index followed both");
        assert!(rig.app.world().resource::<Heard>().0.is_empty(), "nothing was refused");
        assert!(rig.app.world().resource::<Turns>().contains(friend), "the ally was moved, not spent");

        // Someone who takes no side is neither a foe nor an ally.
        let neutral = rig.app.world_mut().spawn((Actor, Blocks, Position(start.offset(1, 1)), Health::full(20))).id();
        rig.app.update();
        rig.bump(Direction::South);
        assert_eq!((rig.at(), rig.now()), (start.offset(1, 0), 100), "someone of no side is in the way");
        assert_eq!(rig.app.world().resource::<Heard>().0, vec![Bumped { actor: rig.player, into: neutral }]);
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
        assert_eq!(app.world().get::<Health>(other).unwrap().current, 5);
        assert_eq!(app.world().resource::<Turns>().now(), 0);
        assert!(app.world().get::<MyTurn>(player).is_some());
    }
}
