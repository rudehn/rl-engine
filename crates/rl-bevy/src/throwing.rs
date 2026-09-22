//! Throwing: a carried item flies at a cell, strikes whoever it meets
//! first, and comes to rest.
//!
//! Its own plugin, because a throw is both an item and a blow: the item
//! leaves a bag the way a dropped one does, and whoever it strikes is hurt
//! down the same damage pipeline a sword's blow goes down. So neither
//! [`ItemsPlugin`](crate::items::ItemsPlugin) nor
//! [`CombatPlugin`](crate::combat::CombatPlugin) has to know the other
//! exists.
//!
//! [`flight`] is the one answer to where a throw goes. The resolver throws
//! with it and the targeting cursor previews with it, so the cells a player
//! is shown are the cells the item will fly through.

use bevy::prelude::*;
use rl_core::turn::BASE_ACTION_COST;
use rl_core::{DiceRoll, Point};
use rl_grid::{Footprint, TargetMode, footprint};
use rl_rules::Hit;
use rl_rules::damage::DamageKindId;

use crate::combat::{CombatRng, DamageEvent, Dead, Health};
use crate::components::{MyTurn, Position};
use crate::cue::{AddAirborne, Airborne, Anchor, Cue, Cued, LookOf, TurnHold};
use crate::items::{Equipped, Inventory, Item, ItemEvent, Stack};
use crate::places::{MapId, OnMap};
use crate::turn::{Action, Intent, Occupancy, Resolution};
use crate::world::WorldMap;

/// An item made to be thrown: how far it goes, and what it does to whoever
/// it strikes.
///
/// Items without one cannot be thrown. A flask that should only shatter
/// where it lands has no strike, and the game answers
/// [`ItemEvent::Thrown`] to spill whatever was in it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throwable {
    /// The furthest cell it reaches.
    pub range: i32,
    /// The kind of damage and the roll it deals whoever it strikes, if any.
    pub strike: Option<(DamageKindId, DiceRoll)>,
}

/// Throw a carried item at a cell.
///
/// At a cell rather than at an actor, for the reason an ability is: a throw
/// goes where it is aimed whether or not anyone is standing there. One from
/// a stack is thrown and the rest stay in the bag; a worn item is taken off
/// first. Refused when the item is not carried, cannot be thrown, or is
/// aimed at the thrower's own feet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Throw {
    /// What is thrown.
    pub item: Entity,
    /// Where it is aimed.
    pub at: Point,
}
impl Action for Throw {}

/// Where a throw goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flight {
    /// The cells it flies through, the last one included.
    pub path: Vec<Point>,
    /// Whoever it meets first, if it meets anyone.
    pub struck: Option<Entity>,
    /// Where it comes to rest: at the feet of whoever it struck, short of a
    /// wall it hit, or at the end of its flight.
    pub rests: Point,
}

/// Where an item thrown from `from` at `at`, reaching `range`, goes: a
/// projectile that stops at the first wall or body in its way, which is the
/// flight a bolt takes, so a knife and a spell agree about what is in the
/// way.
pub fn flight(map: &WorldMap, occupancy: &Occupancy, from: Point, at: Point, range: i32) -> Flight {
    let stops = |p: Point| p != from && (map.blocks_projectiles(p) || occupancy.is_occupied(p));
    let Footprint { path, landing, .. } = footprint(TargetMode::Bolt { range }, from, at, map.window_tiles(), stops);
    let Some(end) = landing else { return Flight { path, struck: None, rests: from } };
    let struck = occupancy.first_at(end);
    // A wall is not somewhere to lie: it falls in the last open cell.
    let rests = if map.blocks_projectiles(end) { path.iter().rev().nth(1).copied().unwrap_or(from) } else { end };
    Flight { path, struck, rests }
}

/// What a throw reads and moves.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Launch<'w, 's> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    rng: ResMut<'w, CombatRng>,
    throwers: Query<'w, 's, (&'static Position, &'static mut Inventory, Option<&'static mut Equipped>), With<MyTurn>>,
    missiles: Query<'w, 's, (&'static Throwable, Option<&'static mut Stack>), With<Item>>,
    alive: Query<'w, 's, (), (With<Health>, Without<Dead>)>,
    cues: MessageWriter<'w, Cued>,
    hold: ResMut<'w, TurnHold>,
    airborne: ResMut<'w, Airborne<ThrowLanding>>,
}

/// A throw that has left the hand and not yet come down: what it will do
/// when it does.
#[derive(Debug, Clone, Copy)]
pub struct ThrowLanding {
    actor: Entity,
    item: Entity,
    rests: Point,
    struck: Option<Entity>,
    strike: Option<(DamageKindId, DiceRoll)>,
}

impl crate::cue::Lands for ThrowLanding {}

/// Resolves [`Throw`] for the actor holding the turn: the flight, the one
/// that leaves the hand, the blow if it strikes anyone, and where it lands.
pub fn resolve_throws(
    mut commands: Commands,
    mut intents: MessageReader<Intent<Throw>>,
    mut resolution: Resolution,
    launch: Launch,
    mut damage: MessageWriter<DamageEvent>,
    mut events: MessageWriter<ItemEvent>,
) {
    let Launch { map, occupancy, mut rng, mut throwers, mut missiles, alive, mut cues, mut hold, mut airborne } = launch;
    for intent in intents.read() {
        let (actor, Throw { item, at }) = (intent.actor, intent.action);
        if !resolution.claim(actor) {
            continue;
        }
        let (Ok((pos, mut bag, worn)), Ok((throwable, stack))) = (throwers.get_mut(actor), missiles.get_mut(item)) else {
            resolution.failed(actor, BASE_ACTION_COST);
            continue;
        };
        if !bag.contains(item) || at == pos.0 {
            resolution.failed(actor, BASE_ACTION_COST);
            continue;
        }
        let Throwable { range, strike } = *throwable;
        let Flight { struck, rests, .. } = flight(&map, &occupancy, pos.0, at, range);

        // One leaves the hand: off the top of a stack, which makes it a
        // thing of its own, or the item itself.
        let thrown = match stack {
            Some(mut stack) if stack.count > 1 => {
                stack.count -= 1;
                let key = stack.key;
                commands.entity(item).clone_and_spawn().insert(Stack { key, count: 1 }).id()
            }
            _ => {
                bag.remove(item);
                if let Some(mut worn) = worn
                    && worn.unequip(item)
                {
                    events.write(ItemEvent::Unequipped { actor, item });
                }
                item
            }
        };
        // The flight, in the item's own glyph, landing on whoever it struck
        // wherever they are by the time it is seen.
        let struck = struck.filter(|who| alive.contains(*who));
        let to = struck.map(|who| Anchor::on(who, rests)).unwrap_or(Anchor::cell(rests));
        cues.write(Cued { actor, cue: Cue::Flight { from: Anchor::on(actor, pos.0), to, look: LookOf::Item(thrown) } });
        let landing = ThrowLanding { actor, item: thrown, rests, struck, strike };
        resolution.done(actor, BASE_ACTION_COST);
        // The turn is spent on the throw. With something watching, the
        // knife is in the air until its flight has been seen, and lies
        // nowhere until it comes down.
        let Some(landing) = airborne.launched(&mut hold, landing) else { continue };
        land(landing, map.current(), &mut commands, &mut rng, &alive, &mut damage, &mut events);
    }
}

/// Lands every throw in the air, on the first pass after its flight has
/// been seen. Landing counts as progress, so the loop goes on to deal the
/// next turn.
pub fn land_throws(
    mut commands: Commands,
    launch: Launch,
    mut damage: MessageWriter<DamageEvent>,
    mut events: MessageWriter<ItemEvent>,
    mut turns: ResMut<crate::turn::Turns>,
) {
    let Launch { map, mut rng, alive, mut hold, mut airborne, .. } = launch;
    for landing in airborne.landing(&mut hold, &mut turns) {
        land(landing, map.current(), &mut commands, &mut rng, &alive, &mut damage, &mut events);
    }
}

/// Where the thrown thing comes down: on the ground where it rests, and
/// into whoever it struck.
fn land(
    landing: ThrowLanding,
    map: MapId,
    commands: &mut Commands,
    rng: &mut CombatRng,
    alive: &Query<(), (With<Health>, Without<Dead>)>,
    damage: &mut MessageWriter<DamageEvent>,
    events: &mut MessageWriter<ItemEvent>,
) {
    let ThrowLanding { actor, item, rests, struck, strike } = landing;
    commands.entity(item).insert((Position(rests), OnMap(map)));
    let struck = struck.filter(|who| alive.contains(*who));
    if let (Some(target), Some((kind, dice))) = (struck, strike) {
        // Floored where it is rolled, as a blow is: a throw that rolls
        // below nothing has missed, not healed.
        let amount = dice.roll_at_least(&mut **rng, 0);
        damage.write(DamageEvent { target, hit: Hit::by(actor, kind, amount) });
    }
    events.write(ItemEvent::Thrown { actor, item, at: Position(rests), struck });
}

/// Throwing: [`Throw`], resolved beside every other action, down combat's
/// damage pipeline.
///
/// Needs [`ItemsPlugin`](crate::items::ItemsPlugin) for the bag a throw
/// leaves and [`CombatPlugin`](crate::combat::CombatPlugin) for the blow it
/// strikes. Minds throw when their wits allow and their brain holds
/// [`ThrowAtRange`](rl_rules::ai::tactics::ThrowAtRange).
pub struct ThrowingPlugin;

impl Plugin for ThrowingPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{ResolveSet, Turn};
        use crate::turn::AddAction;
        app.add_airborne::<ThrowLanding>()
            .add_action::<Throw>()
            .add_systems(Turn, (land_throws.in_set(crate::plugin::LandSet::Throw), resolve_throws).chain().in_set(ResolveSet::Act));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::items::ItemsPlugin>(app, "ThrowingPlugin");
        crate::plugin::depends_on::<crate::combat::CombatPlugin>(app, "ThrowingPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{CombatPlugin, Faction};
    use crate::components::{Actor, Blocks, Player, RevealsMap, Viewshed};
    use crate::items::ItemsPlugin;
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::Turns;
    use rl_grid::TileId;

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        sides: crate::testing::Sides,
    }

    impl Rig {
        fn new() -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, ItemsPlugin, ThrowingPlugin, crate::world::StreamingPlugin));
            let start = crate::testing::surface(&mut app);
            let sides = crate::testing::two_sides(&mut app);
            let player = app
                .world_mut()
                .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap, Health::full(30), Faction(sides.ours), Inventory::default()))
                .id();
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            Self { app, player, start, sides }
        }

        /// A stack of `count` knives in the player's bag.
        fn knives(&mut self, count: u32) -> Entity {
            let knife = Throwable { range: 5, strike: Some((self.sides.kind, DiceRoll::flat(3))) };
            let stack = self.app.world_mut().spawn((Item, knife, Stack { key: 1, count }, Name::new("knife"))).id();
            self.app.world_mut().get_mut::<Inventory>(self.player).unwrap().items.push(stack);
            stack
        }

        /// Someone to throw at, `dx` east of the player.
        fn mark(&mut self, dx: i32) -> Entity {
            let at = self.start.offset(dx, 0);
            let theirs = self.sides.theirs;
            let e = self.app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(20), Faction(theirs))).id();
            self.app.update();
            e
        }

        fn throw(&mut self, item: Entity, dx: i32) -> Vec<ItemEvent> {
            let at = self.start.offset(dx, 0);
            self.app.world_mut().write_message(Intent::new(self.player, Throw { item, at }));
            self.app.update();
            self.app.world_mut().resource_mut::<Messages<ItemEvent>>().drain().collect()
        }

        fn hp(&self, who: Entity) -> i32 {
            self.app.world().get::<Health>(who).unwrap().current
        }

        fn now(&self) -> u32 {
            self.app.world().resource::<Turns>().now()
        }
    }

    /// With something watching, a knife that has left the hand lies
    /// nowhere and hurts nobody until its flight has been seen.
    #[test]
    fn watched_a_thrown_knife_comes_down_only_after_its_flight_has_been_seen() {
        let mut rig = Rig::new();
        let knife = rig.knives(1);
        let near = rig.mark(2);
        rig.app.world_mut().resource_mut::<TurnHold>().watch();

        let events = rig.throw(knife, 4);
        assert!(events.is_empty(), "nothing has landed");
        assert!(!rig.app.world().get::<Inventory>(rig.player).unwrap().contains(knife), "it left the hand");
        assert!(rig.app.world().get::<Position>(knife).is_none(), "and lies nowhere");
        assert_eq!(rig.hp(near), 20);
        assert_eq!(rig.now(), 0, "no turn is dealt while it flies, so the clock waits with it");
        assert!(rig.app.world().resource::<TurnHold>().in_flight());

        rig.app.world_mut().resource_mut::<TurnHold>().release();
        rig.app.update();
        let events: Vec<ItemEvent> = rig.app.world_mut().resource_mut::<Messages<ItemEvent>>().drain().collect();
        assert!(matches!(events.as_slice(), [ItemEvent::Thrown { struck: Some(who), .. }] if *who == near), "{events:?}");
        assert_eq!(rig.app.world().get::<Position>(knife).map(|p| p.0), Some(rig.start.offset(2, 0)), "at their feet");
        assert_eq!(rig.hp(near), 17, "and it hurt");
        assert!(!rig.app.world().resource::<TurnHold>().in_flight());
        assert_eq!(rig.now(), 100, "and the turn the throw spent goes by");
    }

    #[test]
    fn a_thrown_knife_strikes_the_first_body_in_its_way_and_lies_at_its_feet() {
        let mut rig = Rig::new();
        let knives = rig.knives(3);
        let (near, far) = (rig.mark(2), rig.mark(4));

        let events = rig.throw(knives, 4);
        let [ItemEvent::Thrown { actor, item, at, struck, .. }] = events.as_slice() else { panic!("one throw: {events:?}") };
        assert_eq!((*actor, *at, *struck), (rig.player, Position(rig.start.offset(2, 0)), Some(near)), "the near one was in the way");
        assert_eq!((rig.hp(near), rig.hp(far)), (17, 20), "and took the knife");
        assert_ne!(*item, knives, "one left the stack as a thing of its own");
        assert_eq!(rig.app.world().get::<Stack>(*item).map(|s| s.count), Some(1));
        assert_eq!(rig.app.world().get::<Stack>(knives).map(|s| s.count), Some(2), "and two stay in hand");
        assert!(rig.app.world().get::<Inventory>(rig.player).unwrap().contains(knives));
        assert_eq!(rig.app.world().get::<Name>(*item).map(|n| n.as_str()), Some("knife"), "it is still a knife");
        assert_eq!(rig.now(), 100, "a throw is one action");
    }

    #[test]
    fn a_throw_falls_short_of_a_wall_and_lands_at_the_end_of_its_reach() {
        let mut rig = Rig::new();
        let knives = rig.knives(2);
        rig.app.world_mut().resource_mut::<WorldMap>().set_tile(rig.start.offset(3, 0), TileId(1));
        rig.app.update();
        let events = rig.throw(knives, 5);
        assert!(
            matches!(events.as_slice(), [ItemEvent::Thrown { at, struck: None, .. }] if *at == Position(rig.start.offset(2, 0))),
            "short of the wall: {events:?}"
        );

        rig.app.world_mut().resource_mut::<WorldMap>().set_tile(rig.start.offset(3, 0), TileId(2));
        rig.app.update();
        let last = rig.app.world().get::<Inventory>(rig.player).unwrap().items[0];
        let events = rig.throw(last, 7);
        assert!(
            matches!(events.as_slice(), [ItemEvent::Thrown { at, item, .. }] if *at == Position(rig.start.offset(5, 0)) && *item == last),
            "at its reach, and the last one is the stack itself: {events:?}"
        );
        assert!(rig.app.world().get::<Inventory>(rig.player).unwrap().items.is_empty(), "the hand is empty");
    }

    #[test]
    fn a_throw_of_nothing_carried_or_at_your_own_feet_is_refused_for_free() {
        let mut rig = Rig::new();
        let knives = rig.knives(2);
        assert!(rig.throw(knives, 0).is_empty(), "not at your own feet");
        let lying = rig.app.world_mut().spawn((Item, Position(rig.start.offset(1, 0)), Throwable { range: 3, strike: None })).id();
        assert!(rig.throw(lying, 3).is_empty(), "not what lies on the ground");
        let stone = rig.app.world_mut().spawn(Item).id();
        rig.app.world_mut().get_mut::<Inventory>(rig.player).unwrap().items.push(stone);
        assert!(rig.throw(stone, 3).is_empty(), "not what was never made to be thrown");
        assert_eq!(rig.now(), 0);
    }

    #[test]
    fn what_the_dead_carried_falls_where_they_died() {
        let mut rig = Rig::new();
        let mark = rig.mark(2);
        let purse = rig.app.world_mut().spawn(Item).id();
        rig.app.world_mut().entity_mut(mark).insert(Inventory { items: vec![purse] });
        rig.app.world_mut().write_message(DamageEvent { target: mark, hit: Hit::by(rig.player, rig.sides.kind, 99) });
        rig.app.world_mut().write_message(Intent::new(rig.player, crate::turn::Wait));
        rig.app.update();
        assert_eq!(rig.app.world().get::<Position>(purse).map(|p| p.0), Some(rig.start.offset(2, 0)), "it lies where its carrier fell");
        assert_eq!(rig.app.world().get::<OnMap>(purse).map(|m| m.0), Some(rig.app.world().resource::<WorldMap>().current()));
    }
}
