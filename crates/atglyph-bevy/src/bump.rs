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
//! [`BumpRules`] with [`OnAlly::Swap`] makes it a `Swap`, the two
//! changing places for the cost of the step, which is what every game with
//! a companion wants of its walk key. A bump into a neutral is refused the
//! same way, unless its side is one [`BumpRules::give_way`] names: the small
//! life of a place, which nobody fights and nobody should be held up by, is
//! swapped with like an ally, and with [`BumpRules::minds_too`] a mind
//! steps past it the same way.
//!
//! A bump into something that offers one thing, a crate or a captive,
//! takes it up as an [`Interact`]; one offering more, or one it refuses,
//! is a [`Bumped`] with `offers` set, for a screen to ask or explain. A
//! foe is struck and an ally bumped as an ally whatever either offers.
//!
//! Opt-in by being written: a game that wants its walk key to mean a step
//! and nothing more writes [`Step`] as before.

use atglyph_core::turn::{BASE_ACTION_COST, step_cost};
use atglyph_core::{Direction, Point};
use atglyph_rules::FactionId;
use atglyph_rules::bump::{BumpOutcome, Offered, classify_offers, resolve_bump};
use atglyph_rules::faction::stance;
use bevy::prelude::*;

use crate::combat::{Attack, CombatRules, Faction, Health};
use crate::components::{MyTurn, Position, Viewshed};
use crate::doors::Open;
use crate::props::{Interact, OfferedHere};
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
pub(crate) struct Swap(pub Entity);
impl Action for Swap {}

/// An actor bumped into someone it will not strike, or something it
/// would not guess at. The turn was kept.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bumped {
    /// Who bumped.
    pub actor: Entity,
    /// Whom, or what.
    pub into: Entity,
    /// Whether it came to nothing because of what `into` offers: more than
    /// one thing, which a walk key cannot choose between, or one thing
    /// refused, which it cannot explain. A screen asks or explains on
    /// these, and only these: an ally offering something is still bumped
    /// as an ally, and asks nothing. The narrator says nothing of these,
    /// since what a screen asks about is not something in the way.
    pub offers: bool,
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
/// allied; a neutral, and anyone at all in a game with no sides, is
/// refused, since walking through a shopkeeper is not what anyone meant,
/// unless the neutral's side is one the rules say gives way.
#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub struct BumpRules {
    /// A bump into an ally.
    pub allies: OnAlly,
    /// The sides whose every member gives way: a bump into one that is
    /// neutral to the bumper changes places with it, as with an ally under
    /// [`OnAlly::Swap`], whatever it offers.
    ///
    /// Sides, not a mark on each actor, because what gives way is a kind
    /// of creature a game has already put on a side of its own so that
    /// nobody fights it; and a list a game names in content, since which
    /// sides are harmless is the game's to say.
    pub give_way: Vec<FactionId>,
    /// Whether a mind steps past one that gives way too, rather than being
    /// held behind it in a corridor one cell wide: its step into the cell
    /// is a `Swap`. One that gives way never pushes past another that
    /// does, so two of them do not shuffle round each other. A swap sets
    /// off what each of the two is put onto, as a step would, so stepping
    /// past one is no way over the plate it stood on.
    pub minds_too: bool,
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

    /// Everyone of `side` gives way to whoever it is neutral to.
    pub fn give_way(mut self, side: FactionId) -> Self {
        if !self.give_way.contains(&side) {
            self.give_way.push(side);
        }
        self
    }

    /// Minds step past whoever gives way, as the player's bump does.
    pub fn minds_too(mut self) -> Self {
        self.minds_too = true;
        self
    }

    /// Whether one of `side` gives way.
    pub(crate) fn gives_way(&self, side: Option<FactionId>) -> bool {
        side.is_some_and(|side| self.give_way.contains(&side))
    }
}

/// Who stands where, and how they stand to one another.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Way<'w, 's> {
    map: Res<'w, WorldMap>,
    occupancy: Res<'w, Occupancy>,
    rules: Option<Res<'w, CombatRules>>,
    bumps: Option<Res<'w, BumpRules>>,
    bumpers: Query<'w, 's, (&'static Position, Option<&'static Faction>), With<MyTurn>>,
    standing: Query<'w, 's, Option<&'static Faction>, With<Health>>,
    /// What is offered here, when props are in play at all. `None` in a
    /// game without [`PropsPlugin`](crate::props::PropsPlugin), and the
    /// offer branch below is then dead code rather than a special case.
    offered: Option<Res<'w, OfferedHere>>,
}

/// What a bump writes.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Alternates<'w> {
    steps: MessageWriter<'w, Intent<Step>>,
    opens: MessageWriter<'w, Intent<Open>>,
    attacks: MessageWriter<'w, Intent<Attack>>,
    swaps: MessageWriter<'w, Intent<Swap>>,
    interacts: MessageWriter<'w, Intent<Interact>>,
    bumped: MessageWriter<'w, Bumped>,
}

/// Turns each bump into the intent it stands for.
///
/// Nothing is claimed here except a bump that comes to nothing, so the
/// resolver of whatever was written spends the turn, and the sweep finds it
/// spent. A bump whose turn is already spent, as a status that holds
/// spends one before anything resolves, comes to nothing at all: no
/// resolver would take what it wrote, and the question a prop offering
/// several things raises would open a screen for a turn that is gone.
pub(crate) fn redirect_bumps(mut intents: MessageReader<Intent<Bump>>, mut resolution: Resolution, way: Way, mut out: Alternates) {
    for intent in intents.read() {
        let actor = intent.actor;
        if resolution.spent(actor) {
            continue;
        }
        let Ok((pos, side)) = way.bumpers.get(actor) else { continue };
        let dir = intent.action.0;
        let target = pos.0 + dir.offset();
        // Whoever stands there and can be struck, by the rule the attack
        // resolver strikes by: alive, with health to lose.
        let someone = way.occupancy.at(target).iter().copied().find(|other| way.standing.contains(*other));
        let theirs = someone.and_then(|other| way.standing.get(other).ok().flatten());
        let regard = someone.and_then(|_| stance(way.rules.as_deref().map(|r| &r.factions), side.map(|f| f.0), theirs.map(|f| f.0)));
        let bumps = way.bumps.as_deref();
        let gives_way = bumps.is_some_and(|r| r.gives_way(theirs.map(|f| f.0)));
        let (offered, what) = way.offers_at(actor, target);
        match resolve_bump(someone, regard, gives_way, bumps.is_some_and(|r| r.allies == OnAlly::Swap), offered, way.map.opens(target).is_some()) {
            BumpOutcome::Interact(offer) => {
                out.interacts.write(Intent::new(actor, Interact { target: offer.target, verb: offer.verb, thing: None }));
            }
            BumpOutcome::Ask => {
                out.bumped.write(Bumped { actor, into: what.expect("an offer to ask about came from something"), offers: true });
            }
            BumpOutcome::Attack(other) => {
                out.attacks.write(Intent::new(actor, Attack(other)));
            }
            BumpOutcome::Swap(other) => {
                out.swaps.write(Intent::new(actor, Swap(other)));
            }
            BumpOutcome::Refuse(other) => {
                if resolution.claim(actor) {
                    resolution.failed(actor, BASE_ACTION_COST);
                    out.bumped.write(Bumped { actor, into: other, offers: false });
                }
            }
            BumpOutcome::Open => {
                out.opens.write(Intent::new(actor, Open(dir)));
            }
            BumpOutcome::Step => {
                out.steps.write(Intent::new(actor, Step(dir)));
            }
        }
    }
}

impl Way<'_, '_> {
    /// What stands in `target` offers `actor`, a prop or an actor alike:
    /// the gate already worked out who offers what, and who it is.
    ///
    /// One thing it could do now, for no price, and nothing else at all is
    /// the only case a walk key answers. Something that offers several, or
    /// one it refuses, is a question or an explanation, which is a screen's;
    /// and so is one with a price, since spending currency is a choice a
    /// movement key must never make by walking into someone, and one that
    /// asks first, a yes or one of the actor's things, since a movement key
    /// cannot answer.
    fn offers_at(&self, actor: Entity, target: Point) -> (Offered<&crate::props::Offer>, Option<Entity>) {
        let Some(offered) = self.offered.as_deref() else { return (Offered::None, None) };
        let mine = offered.for_actor(actor);
        let Some(what) = self.occupancy.at(target).iter().copied().find(|e| mine.iter().any(|o| o.target == *e)) else { return (Offered::None, None) };
        let all: Vec<&crate::props::Offer> = mine.iter().filter(|o| o.target == what).collect();
        (classify_offers(&all, |o| o.refused.is_none() && o.price.is_none() && o.asks.is_none()), Some(what))
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
pub(crate) fn resolve_swaps(
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
        let cost = step_cost(map.cost(to).unwrap_or(BASE_ACTION_COST), dir.is_some_and(Direction::is_diagonal));
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
    use crate::props::Interacted;
    use crate::state::EngineState;
    use crate::testing::headless_app;
    use crate::turn::Turns;
    use atglyph_core::{DiceRoll, Point};
    use atglyph_grid::TileRegistry;

    #[derive(Resource, Default)]
    struct Heard(Vec<Bumped>, Vec<DoorEvent>, Vec<Interacted>);

    fn hear(mut bumps: MessageReader<Bumped>, mut doors: MessageReader<DoorEvent>, mut done: MessageReader<Interacted>, mut heard: ResMut<Heard>) {
        heard.0.extend(bumps.read().copied());
        heard.1.extend(doors.read().copied());
        heard.2.extend(done.read().copied());
    }

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
        tiles: TileRegistry,
        theirs: atglyph_rules::FactionId,
        ours: atglyph_rules::FactionId,
        kind: atglyph_rules::damage::DamageKindId,
    }

    impl Rig {
        fn new() -> Self {
            Self::built(false)
        }

        /// The rig, with minds in play when `minds` is.
        fn built(minds: bool) -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, CombatPlugin, crate::props::PropsPlugin));
            if minds {
                app.add_plugins(crate::minds::MindsPlugin);
            }
            crate::props::AddVerb::add_verb(&mut app, "free");
            crate::props::AddVerb::add_verb(&mut app, "talk");
            app.init_resource::<Heard>().add_systems(PostUpdate, hear);
            let start = crate::testing::ground(&mut app);
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
        assert_eq!(rig.app.world().resource::<Heard>().0, vec![Bumped { actor: rig.player, into: friend, offers: false }]);
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
        assert_eq!(rig.app.world().resource::<Heard>().0, vec![Bumped { actor: rig.player, into: neutral, offers: false }]);
    }

    /// Without rules to call anyone a foe, nobody is one: a bump into any
    /// actor is a refusal, never a swing.
    #[test]
    fn with_no_sides_a_bump_into_anyone_is_a_refusal() {
        let mut app = headless_app();
        app.add_plugins(crate::fov::FovPlugin);
        let start = crate::testing::ground(&mut app);
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

    impl Rig {
        fn verb(&self, name: &str) -> crate::props::VerbId {
            self.app.world().resource::<crate::props::Verbs>().get(name).expect("declared")
        }

        /// Someone a step east of the player, of `side` if any, offering
        /// `offers`.
        fn beside(&mut self, side: Option<atglyph_rules::FactionId>, offers: Vec<crate::props::Offering>) -> Entity {
            let at = self.at().offset(1, 0);
            let mut someone = self.app.world_mut().spawn((Actor, Blocks, Position(at), Health::full(20), crate::props::Offers(offers)));
            if let Some(side) = side {
                someone.insert(Faction(side));
            }
            let someone = someone.id();
            self.app.update();
            someone
        }
    }

    /// Walking into someone who is neither foe nor friend and offers one
    /// thing takes it up, the way walking into a crate opens it.
    #[test]
    fn a_bump_into_a_neutral_actor_offering_one_thing_comes_to_it() {
        let mut rig = Rig::new();
        let free = rig.verb("free");
        let captive = rig.beside(None, vec![crate::props::Offering::new(free, 200)]);
        let (start, before) = (rig.at(), rig.now());
        rig.bump(Direction::East);
        let heard = rig.app.world().resource::<Heard>();
        assert_eq!(heard.2, vec![Interacted { actor: rig.player, target: captive, verb: free, thing: None }], "the walk key freed the captive");
        assert!(heard.0.is_empty(), "and nothing was refused");
        assert_eq!((rig.at(), rig.now()), (start, before + 200), "for what the offer costs, without a step");
    }

    /// Someone offering two things, or one it refuses, is a question or
    /// an explanation: the bump does neither, keeps the turn, and says the
    /// bump was into something that offers, so a screen can ask.
    #[test]
    fn a_bump_into_an_actor_offering_two_things_or_one_refused_asks_rather_than_acts() {
        let mut rig = Rig::new();
        let (free, talk) = (rig.verb("free"), rig.verb("talk"));
        let two = rig.beside(None, vec![crate::props::Offering::new(free, 100), crate::props::Offering::new(talk, 100)]);
        let before = rig.now();
        rig.bump(Direction::East);
        let heard = rig.app.world().resource::<Heard>();
        assert_eq!(heard.0, vec![Bumped { actor: rig.player, into: two, offers: true }], "a question for a screen");
        assert!(heard.2.is_empty(), "and neither was taken up");
        assert_eq!(rig.now(), before, "for nothing");

        let mut rig = Rig::new();
        let tag = atglyph_rules::TagId::from_raw(0);
        let locked = rig.beside(None, vec![crate::props::Offering::new(free, 100).needing(tag)]);
        rig.bump(Direction::East);
        let heard = rig.app.world().resource::<Heard>();
        assert_eq!(heard.0, vec![Bumped { actor: rig.player, into: locked, offers: true }], "an explanation for a screen");
        assert!(heard.2.is_empty());
    }

    /// Walking into someone whose one offer has a price, one the player
    /// can pay, asks rather than pays: the purse is untouched and the turn
    /// kept, and a screen is told there is something to ask about. The same
    /// offer without a price is still taken up by the walk key.
    #[test]
    fn bumping_someone_whose_one_offer_has_a_price_asks_rather_than_paying() {
        let mut rig = Rig::new();
        let free = rig.verb("free");
        let tags = atglyph_rules::Registry::from_defs(vec![atglyph_rules::TagDef::new("scrip")]).expect("one tag");
        let scrip = tags.expect("scrip");
        rig.app.insert_resource(crate::trade::Currency(scrip));
        let coins = rig.app.world_mut().spawn((crate::items::Item, crate::items::Tagged(vec![scrip]), crate::items::Stack { key: 1, count: 80 })).id();
        let player = rig.player;
        rig.app.world_mut().entity_mut(player).insert(crate::items::Inventory { items: vec![coins] });
        let ferry = rig.beside(None, vec![crate::props::Offering::new(free, 200).priced(50)]);
        let (start, before) = (rig.at(), rig.now());
        rig.bump(Direction::East);
        let heard = rig.app.world().resource::<Heard>();
        assert_eq!(heard.0, vec![Bumped { actor: rig.player, into: ferry, offers: true }], "a question for a screen");
        assert!(heard.2.is_empty(), "and nothing was taken up");
        assert_eq!((rig.at(), rig.now()), (start, before), "for nothing");
        assert_eq!(rig.app.world().get::<crate::items::Stack>(coins).map(|s| s.count), Some(80), "the purse is untouched");

        let mut rig = Rig::new();
        let captive = rig.beside(None, vec![crate::props::Offering::new(free, 200)]);
        rig.bump(Direction::East);
        assert_eq!(
            rig.app.world().resource::<Heard>().2,
            vec![Interacted { actor: rig.player, target: captive, verb: free, thing: None }],
            "unpriced, the walk key takes it up"
        );
    }

    /// Walking into someone whose one offer asks first, a yes or a thing,
    /// comes to a question for a screen, never to the offer: nothing is
    /// taken up, and the step and the turn are kept.
    #[test]
    fn bumping_someone_whose_one_offer_asks_or_chooses_first_asks_rather_than_acts() {
        for chooses in [false, true] {
            let mut rig = Rig::new();
            let free = rig.verb("free");
            let among = rig.app.register_system(|_: In<crate::props::OfferChooser>| Vec::new());
            let offer = crate::props::Offering::new(free, 200);
            let offer = if chooses { offer.choosing("Free them with which?", among) } else { offer.asking("Free them?") };
            let captive = rig.beside(None, vec![offer]);
            let (start, before) = (rig.at(), rig.now());
            rig.bump(Direction::East);
            let heard = rig.app.world().resource::<Heard>();
            assert_eq!(heard.0, vec![Bumped { actor: rig.player, into: captive, offers: true }], "chooses {chooses}: a question for a screen");
            assert!(heard.2.is_empty(), "chooses {chooses}: and nothing was taken up");
            assert_eq!((rig.at(), rig.now()), (start, before), "chooses {chooses}: for nothing");
        }
    }

    /// A third side neutral to both of the rig's, and the rules for all
    /// three.
    fn small_side(rig: &mut Rig) -> atglyph_rules::FactionId {
        small_side_hunted(rig, false)
    }

    /// [`small_side`], with the rig's other side hostile to it when
    /// `hunted`.
    fn small_side_hunted(rig: &mut Rig, hunted: bool) -> atglyph_rules::FactionId {
        let names = atglyph_rules::Registry::from_defs(["ours", "theirs", "small"].map(atglyph_rules::faction::FactionDef::new).to_vec()).expect("three sides");
        let small = names.expect("small");
        let rules = CombatRules::new().hostile(rig.ours, rig.theirs);
        rig.app.insert_resource(if hunted { rules.hostile(rig.theirs, small) } else { rules });
        small
    }

    /// A bump into a neutral of a side that gives way changes places with
    /// it for a step's time, whatever it offers; into a neutral of any other
    /// side, or with no rules naming its side, it is refused as before.
    #[test]
    fn a_bump_into_a_neutral_that_gives_way_changes_places_with_it_and_any_other_neutral_is_refused() {
        let mut rig = Rig::new();
        let small = small_side(&mut rig);
        rig.app.insert_resource(BumpRules::new().give_way(small));
        let talk = rig.verb("talk");
        let start = rig.at();
        let critter = rig.beside(Some(small), vec![crate::props::Offering::new(talk, 100)]);
        let before = rig.now();
        rig.bump(Direction::East);
        assert_eq!(rig.at(), start.offset(1, 0), "past it");
        assert_eq!(rig.app.world().get::<Position>(critter).unwrap().0, start, "which stands where the player stood");
        assert_eq!(rig.now(), before + 100, "for a step's time");
        assert!(rig.app.world().resource::<Heard>().2.is_empty(), "and nothing it offers was taken up");

        let mut rig = Rig::new();
        let small = small_side(&mut rig);
        rig.app.insert_resource(BumpRules::new().swap_allies());
        let start = rig.at();
        let critter = rig.beside(Some(small), Vec::new());
        rig.bump(Direction::East);
        assert_eq!(rig.at(), start, "a side nobody named gives no way");
        assert_eq!(rig.app.world().resource::<Heard>().0, vec![Bumped { actor: rig.player, into: critter, offers: false }]);
    }

    /// A dead-end corridor one cell wide east of the player, a critter of the small
    /// side two cells along it, and a hunter of the rig's other side two
    /// further, under `rules`, with the small side hunted by the other
    /// when `hunted`, something else that blocks on the critter's cell when
    /// `crowded`, and the hunter striking what is beside it before it hunts
    /// when it `strikes`. Where the hunter and the critter stood after each
    /// of forty frames, the critter's `None` once it is gone, and the
    /// corridor's start.
    fn past_a_critter(
        rules: impl Fn(atglyph_rules::FactionId, atglyph_rules::FactionId) -> BumpRules,
        hunted: bool,
        crowded: bool,
        strikes: bool,
    ) -> (Vec<(Point, Option<Point>)>, Point) {
        let mut rig = Rig::built(true);
        let small = small_side_hunted(&mut rig, hunted);
        rig.app.insert_resource(rules(small, rig.theirs));
        let start = rig.start;
        let wall = rig.tiles.expect("wall");
        // Shut at the hunter's end, so the only way to the player is past
        // the critter.
        for dx in -1..=7 {
            let mut map = rig.app.world_mut().resource_mut::<WorldMap>();
            map.set_tile(start.offset(dx, -1), wall);
            map.set_tile(start.offset(dx, 1), wall);
        }
        rig.app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(7, 0), wall);
        let critter = rig.app.world_mut().spawn((Actor, Blocks, Position(start.offset(2, 0)), Health::full(5), Faction(small))).id();
        if crowded {
            // Blocking, and no prop, so the hunter still sees the player
            // past it.
            rig.app.world_mut().spawn((Blocks, Position(start.offset(2, 0))));
        }
        let brain = atglyph_rules::Brain::new();
        let brain = if strikes { brain.then(atglyph_rules::ai::tactics::MeleeAdjacent) } else { brain };
        let brain = brain.then(atglyph_rules::ai::tactics::Hunt);
        let hunter = rig
            .app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(start.offset(4, 0)),
                Health::full(20),
                Faction(rig.theirs),
                crate::minds::Perception(8),
                MeleeAttack::new(rig.kind, DiceRoll::flat(1)),
                crate::minds::Mind(std::sync::Arc::new(brain)),
            ))
            .id();
        let mut trace = Vec::new();
        for _ in 0..40 {
            if rig.app.world().get::<MyTurn>(rig.player).is_some() {
                rig.app.world_mut().write_message(Intent::new(rig.player, crate::turn::Wait));
            }
            rig.app.update();
            let at = |e: Entity| rig.app.world().get::<Position>(e).map(|p| p.0);
            trace.push((at(hunter).expect("the hunter stands"), at(critter)));
        }
        (trace, start)
    }

    /// A hunter behind a neutral that gives way in a corridor one cell wide
    /// steps past it to the player when the rules say minds do too, and
    /// waits behind it when they do not.
    #[test]
    fn a_mind_steps_past_a_neutral_that_gives_way_only_when_the_rules_say_minds_do() {
        let (trace, start) = past_a_critter(|small, _| BumpRules::new().give_way(small).minds_too(), false, false, true);
        assert_eq!(trace.last().unwrap().0, start.offset(1, 0), "it stepped past the neutral to the player");
        let (trace, start) = past_a_critter(|small, _| BumpRules::new().give_way(small), false, false, true);
        assert_eq!(trace.last().unwrap().0, start.offset(3, 0), "it waits behind the neutral");
    }

    /// One that gives way never pushes past another that does: a hunter
    /// whose own side gives way waits behind the critter, minds too or not.
    #[test]
    fn a_mind_of_a_side_that_gives_way_never_steps_past_another_that_does() {
        let (trace, start) = past_a_critter(|small, theirs| BumpRules::new().give_way(small).give_way(theirs).minds_too(), false, false, true);
        assert!(trace.iter().all(|(_, critter)| *critter == Some(start.offset(2, 0))), "the critter was never moved");
        assert_eq!(trace.last().unwrap().0, start.offset(3, 0), "and the hunter waits behind it");
    }

    /// A mind hostile to a side that gives way strikes one of it rather than
    /// changing places with it: the critter is never swapped, and dies where
    /// it stood before the hunter walks on.
    #[test]
    fn a_mind_hostile_to_a_side_that_gives_way_strikes_rather_than_swaps() {
        let (trace, start) = past_a_critter(|small, _| BumpRules::new().give_way(small).minds_too(), true, false, true);
        assert!(trace.iter().all(|(_, critter)| critter.is_none_or(|c| c == start.offset(2, 0))), "the critter was never swapped");
        assert_eq!(trace.last().unwrap().1, None, "it was struck down");
        // And with nothing to strike with, it hunts the critter to its cell
        // and is refused there, rather than changing places with its prey.
        let (trace, start) = past_a_critter(|small, _| BumpRules::new().give_way(small).minds_too(), true, false, false);
        assert!(trace.iter().all(|(_, critter)| *critter == Some(start.offset(2, 0))), "the critter was never swapped: {trace:?}");
    }

    /// A cell holding a critter that gives way and something else that
    /// blocks is no way past: the swap would put the hunter onto the other.
    #[test]
    fn a_cell_holding_a_neutral_that_gives_way_and_another_blocker_is_refused() {
        let (trace, start) = past_a_critter(|small, _| BumpRules::new().give_way(small).minds_too(), false, true, true);
        assert!(trace.iter().all(|(hunter, critter)| hunter.x > start.x + 2 && *critter == Some(start.offset(2, 0))), "it never got past: {trace:?}");
    }
}
