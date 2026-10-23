//! Places: bounded maps, entered from each other.
//!
//! A place is a bounded map built once
//! by the game's [`PlaceRules`] the first time something enters it and
//! kept whole after that, actors and items included: leaving a place
//! freezes what is in it, returning finds it as it was. Every entity with
//! a position is on exactly one map, named by [`OnMap`]; the scheduler
//! freezes actors on other maps and the readers of the current map do not
//! see them.
//!
//! A game hands its rules over once with [`AddPlaces::places`], and each
//! place is built from its own [`PlaceSeed`], so the order places are
//! built in changes none of them. [`SpawnPlayer::spawn_player`] puts the
//! player at a place's entry in one call.
//!
//! A [`Transition`] is an entity standing on a cell; [`GoThrough`] on
//! that cell takes an actor through it, and a [`WarpRequest`] does the
//! same from anywhere, for portals. The map being read follows the player.
//! Anyone else travels only when the game says its kind does: a mind with
//! [`Wits::FOLLOWS_THROUGH`] goes through after whoever it was hunting or
//! keeping beside, when it stood next to them as they left, and one with
//! [`Wits::FLEES_THROUGH`] takes a way out when it runs; content's
//! `travels` is both. It is frozen where it lands until the player comes,
//! like everything else on a map nobody is reading.
//!
//! A game whose places are floors one under another says so with a
//! [`Ladder`], from [`PlaceRules::ladder`], and the engine lays the
//! stairs: when a floor is built, a [`Stair`] up at its entry to the
//! floor above's exit and a stair down at its exit to the floor below's
//! entry, each a [`Transition`] like any other, which the engine saves.
//! What stands at either end of the ladder, a way out of the run or
//! nothing, is the game's; so is what a game says on arriving.
//!
//! What a place is and how a game builds one, [`MapId`], [`PlaceRules`],
//! [`PlaceBuild`], [`Ladder`] and the rest, is plain data in
//! [`atglyph_mapgen::place`], so a terminal game or a tool builds the same
//! places with no `App`; this module re-exports it, and keeps the
//! components, the warp and the travel.
//!
//! A place keeps its [`Lock`]s, the doors its builder locked to a tag and
//! the gates it sealed, and its save keeps them with it. It keeps its
//! [`Secret`](crate::secrets::Secret)s the same way: the cells its builder
//! walled over, each something else once found.

use atglyph_core::turn::BASE_ACTION_COST;
use atglyph_core::{PlaceSeed, Point};
use atglyph_mapgen::BuildError;
pub use atglyph_mapgen::place::{Climb, Ladder, LadderEnd, LadderStair, Lock, MapId, PlaceBuild, PlaceRules, Spot, StairLook};
use atglyph_rules::Wits;
use bevy::prelude::*;

use crate::combat::Dead;
use crate::components::{Actor, Blocks, Player, Position, Viewshed};
use crate::knowledge::Knowledge;
use crate::minds::{FlowFields, Intelligence, Post, Trails};
use crate::turn::{Action, Intent, Occupancy, Resolution};
use crate::world::WorldMap;

/// The map an entity with a position is on. Missing means map zero;
/// the engine fills it in for anything that gains a position.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Deref, serde::Serialize, serde::Deserialize)]
pub struct OnMap(pub MapId);

/// Where in a place an arrival stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Arrive {
    /// At the place's entry, as built.
    Entry,
    /// At the place's exit, as built. Falls back to the entry if it has none.
    Exit,
    /// At this cell.
    At(Point),
}

/// The far side of a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Destination {
    /// A place, built on first arrival.
    Place {
        /// Which.
        map: MapId,
        /// Where in it.
        arrive: Arrive,
    },
}

/// A way through: stairs, a cave mouth, a hatch. Stands on a cell of one
/// map and leads to a cell of another.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Transition {
    /// Where it leads.
    pub to: Destination,
}

/// A stair the engine laid between two floors of a [`Ladder`]: which way
/// it goes, and how it looks. Beside the [`Transition`] that says where it
/// leads, which a save keeps with the entity as it keeps any transition,
/// so the stair is written down once, as a stair.
#[derive(Component, Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Stair {
    /// Up or down.
    pub climb: Climb,
    /// How it is drawn and called.
    pub look: StairLook,
}

/// The game's place builder.
///
/// Inserted by [`AddPlaces::places`] once, with the game, or by a start
/// that makes its rules again for every run.
#[derive(Resource)]
pub struct PlaceRulesRes(pub Box<dyn PlaceRules>);

/// Hands the engine a game's place rules.
pub trait AddPlaces {
    /// Builds every place of the game by `rules`, and the run's
    /// [`WorldMap`] over their [`tiles`](PlaceRules::tiles) when a start
    /// makes none.
    ///
    /// Once, where the game is put together, rather than in every run's
    /// start: the rules are handed each place's seed, so nothing in them
    /// changes from run to run. Says the game needs a
    /// [`Seed`](crate::seed::Seed), since a place is built from one, so a
    /// game that forgot it is told when play begins rather than at its
    /// first warp.
    fn places(&mut self, rules: impl PlaceRules + 'static) -> &mut Self;
}

impl AddPlaces for App {
    fn places(&mut self, rules: impl PlaceRules + 'static) -> &mut Self {
        use crate::plugin::Needs;
        self.insert_resource(PlaceRulesRes(Box::new(rules)))
            .needs::<crate::seed::Seed>("places", "`Seed(RunSeed(n))`, the run's seed every stream derives from, or `Seed::from_args()` for `--seed`")
    }
}

/// Spawns the player into a place.
pub trait SpawnPlayer {
    /// Spawns the player, an [`Actor`] that [`Blocks`], with `bundle`, and
    /// warps it to the entry of `map`, building the place if nobody has.
    ///
    /// The warp is written when the spawn is applied, so the player is
    /// never warped before it exists, and the place's entry is where the
    /// game's [`PlaceRules`] put it. `bundle` holds what the game's player
    /// has besides: sight, a glyph, health. It must hold no [`Position`],
    /// [`Player`], [`Actor`] or [`Blocks`], which this gives it already.
    fn spawn_player(&mut self, map: MapId, bundle: impl Bundle) -> EntityCommands<'_>;
}

impl SpawnPlayer for Commands<'_, '_> {
    fn spawn_player(&mut self, map: MapId, bundle: impl Bundle) -> EntityCommands<'_> {
        let player = self.spawn((Actor, Player, Blocks, Position(Point::ZERO), bundle)).id();
        self.queue(move |world: &mut World| {
            world.write_message(WarpRequest::into_place(player, map));
        });
        self.entity(player)
    }
}

/// Go through the [`Transition`] on the actor's cell. Refused off one,
/// before the turn is claimed and so at no cost to the player, since the
/// key a game binds it to may mean something else there. Refused to anyone
/// but the player whose [`Intelligence`] lacks [`Wits::FLEES_THROUGH`], the
/// wit for going through of its own accord: a monster stays on the map it
/// was put on unless the game says its kind does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoThrough;
impl Action for GoThrough {}

/// Asks the engine to move someone somewhere, building the place if
/// needed. Resolved in [`TurnSet::Resolve`](crate::plugin::TurnSet::Resolve)
/// without costing a turn; a game charges what it likes.
///
/// Anyone: this is the game moving an actor, so nothing is asked of its
/// wits. The map being read follows the player and nobody else, so an
/// actor sent to another map is frozen there until the player arrives.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarpRequest {
    /// Who.
    pub actor: Entity,
    /// Where to.
    pub to: Destination,
    /// Whom it goes after, when the game sends it after someone rather
    /// than of its own accord: a party sent down the stairs behind the
    /// player. Carried onto its [`Travelled`], so whatever tells of the
    /// journey tells it as following, and the warp is skipped when the
    /// one it follows was refused the same pass. `None` for a warp of
    /// its own.
    pub after: Option<Entity>,
}

impl WarpRequest {
    /// `actor` to `to`, of its own accord.
    pub fn new(actor: Entity, to: Destination) -> Self {
        Self { actor, to, after: None }
    }

    /// Into `map` at its entry: how a delve starts on its first floor.
    pub fn into_place(actor: Entity, map: MapId) -> Self {
        Self::new(actor, Destination::Place { map, arrive: Arrive::Entry })
    }

    /// The same warp, going after `leader`.
    pub fn after(self, leader: Entity) -> Self {
        Self { after: Some(leader), ..self }
    }
}

/// The player changed maps, and the map being read with it.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapChanged {
    /// From.
    pub from: MapId,
    /// To.
    pub to: MapId,
}

/// Someone changed maps: the player, or anyone else.
///
/// Beside [`MapChanged`] rather than in place of it, because that one
/// says the map every reader reads has switched, which only the player's
/// travel does. This is what a game answers to say that something ran
/// down the stairs or came up them after the player.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Travelled {
    /// Who.
    pub actor: Entity,
    /// From.
    pub from: MapId,
    /// The cell of `from` it left, which is where anyone who saw it go was
    /// looking; where it stands now is a cell of another map.
    pub left: Point,
    /// To.
    pub to: MapId,
    /// Who it went after, when it followed someone through rather than
    /// going of its own accord.
    pub after: Option<Entity>,
}

/// A place was entered by the player. `first` is true the first time,
/// which is when a game populates it.
///
/// The first time the player arrives, not the time it was built: a
/// monster that travels can get there first and build it, and the floor
/// is still to be filled when the player follows.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceEntered {
    /// Which.
    pub map: MapId,
    /// Whether this is the player's first arrival.
    pub first: bool,
    /// Its entry.
    pub entry: Point,
    /// Its exit, if any.
    pub exit: Option<Point>,
}

/// The maps and indexes a warp rewrites.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Maps<'w> {
    map: ResMut<'w, WorldMap>,
    occupancy: ResMut<'w, Occupancy>,
    knowledge: ResMut<'w, Knowledge>,
    fields: ResMut<'w, FlowFields>,
    rules: Option<Res<'w, PlaceRulesRes>>,
    seed: Option<Res<'w, crate::seed::Seed>>,
}

/// What a warp reports.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct WarpReport<'w> {
    changed: MessageWriter<'w, MapChanged>,
    entered: MessageWriter<'w, PlaceEntered>,
    travelled: MessageWriter<'w, Travelled>,
}

/// Anyone who might travel: where it stands, whether it is the player,
/// what its wits allow, and whom it would go after.
type TravellerData = (
    &'static mut Position,
    Option<&'static mut Viewshed>,
    Option<&'static OnMap>,
    Has<Blocks>,
    Has<Player>,
    Option<&'static Intelligence>,
    Option<&'static Trails>,
    Has<Dead>,
);

/// Everyone with a position but the ways through themselves, which keeps
/// this disjoint from the transitions it is read beside.
type Traveller<'w, 's> = Query<'w, 's, (Entity, TravellerData), Without<Transition>>;

/// Transitions, wherever they stand.
type Transitions<'w, 's> = Query<'w, 's, (&'static Position, Option<&'static OnMap>, &'static Transition)>;

/// Who travels and what they travel through.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Travel<'w, 's> {
    travellers: Traveller<'w, 's>,
    transitions: Transitions<'w, 's>,
}

/// One journey to resolve: who, where to, whether it is the actor's turn
/// spent, and whom it follows.
struct Trip {
    actor: Entity,
    to: Destination,
    is_action: bool,
    after: Option<Entity>,
}

/// Takes an actor through the transition it stands on for a
/// [`GoThrough`], and anywhere a [`WarpRequest`] asks.
///
/// Whoever was beside an actor that went through, travels, and was after
/// it or with it on its own last turn goes through behind it, in the same
/// pass: [`Trails`] is the list a mind keeps of those, and it is empty
/// for a mind without [`Wits::FOLLOWS_THROUGH`]. A warp is never followed,
/// since nobody saw where it went; a warp the game sends after someone
/// says so with [`WarpRequest::after`], and is skipped when that one went
/// nowhere.
pub(crate) fn resolve_warps(
    mut commands: Commands,
    mut intents: MessageReader<Intent<GoThrough>>,
    mut requests: MessageReader<WarpRequest>,
    mut resolution: Resolution,
    mut maps: Maps,
    mut report: WarpReport,
    travel: Travel,
) {
    let Travel { mut travellers, transitions } = travel;
    let mut trips: Vec<Trip> = Vec::new();
    for intent in intents.read() {
        let found = travellers.get(intent.actor).ok().and_then(|(_, (pos, _, on, _, player, wits, ..))| {
            let (at, here) = (pos.0, on.map(|m| m.0).unwrap_or(MapId::ZERO));
            let travels = player || wits.is_some_and(|w| w.has(Wits::FLEES_THROUGH));
            transitions.iter().find(|(p, m, _)| p.0 == at && m.map(|m| m.0).unwrap_or(MapId::ZERO) == here).map(|(_, _, t)| (at, here, t.to, travels))
        });
        // Nothing to go through is refused before the turn is claimed: a
        // game may bind its way to a key that means something else off
        // one, and the press then still does that.
        let Some((at, here, to, travels)) = found else {
            resolution.refuse(intent.actor, BASE_ACTION_COST);
            continue;
        };
        if !resolution.claim(intent.actor) {
            continue;
        }
        if !travels {
            resolution.failed(intent.actor, BASE_ACTION_COST);
            continue;
        }
        trips.push(Trip { actor: intent.actor, to, is_action: true, after: None });
        // Who is beside the traveller, travels and was after it; the rule
        // orders them, so who gets the cell beside the landing never
        // depends on the order a query happened to run in.
        let candidates = travellers.iter().map(|(e, (pos, _, on, _, _, wits, trails, dead))| {
            let follows = e != intent.actor
                && !dead
                && on.map(|m| m.0).unwrap_or(MapId::ZERO) == here
                && atglyph_core::geometry::chebyshev(pos.0, at) <= 1
                && wits.is_some_and(|w| w.has(Wits::FOLLOWS_THROUGH))
                && trails.is_some_and(|t| t.0.contains(&intent.actor));
            (e, pos.0, follows)
        });
        let behind = atglyph_rules::travel::followers(candidates, at);
        trips.extend(behind.into_iter().map(|e| Trip { actor: e, to, is_action: false, after: Some(intent.actor) }));
    }
    for req in requests.read() {
        debug!("warp request {req:?}");
        if travellers.get(req.actor).is_ok() {
            trips.push(Trip { actor: req.actor, to: req.to, is_action: false, after: req.after });
        }
    }
    // Whoever a trip follows has to have got there: nobody goes through
    // after someone the way refused.
    let mut stayed: Vec<Entity> = Vec::new();
    for Trip { actor, to, is_action, after } in trips {
        if after.is_some_and(|led| stayed.contains(&led)) {
            continue;
        }
        match warp(&mut commands, &mut maps, &mut report, &mut travellers, actor, to, after) {
            Ok(()) => {
                if is_action {
                    resolution.done(actor, BASE_ACTION_COST);
                }
            }
            Err(e) => {
                stayed.push(actor);
                if is_action {
                    error!("warp failed: {e}");
                    resolution.failed(actor, BASE_ACTION_COST);
                } else {
                    debug!("{actor:?} did not travel: {e}");
                }
            }
        }
    }
}

/// How many cells round a taken landing are tried before an arrival is
/// refused: the landing's neighbours and theirs.
const LANDING_REACH: usize = 25;

fn warp(
    commands: &mut Commands,
    maps: &mut Maps,
    report: &mut WarpReport,
    travellers: &mut Traveller,
    actor: Entity,
    to: Destination,
    after: Option<Entity>,
) -> Result<(), BuildError> {
    let Destination::Place { map: target_map, arrive } = to;
    if !maps.map.has_place(target_map) {
        let rules = maps.rules.as_ref().ok_or_else(|| BuildError::new("warp", "no PlaceRulesRes to build a place with"))?;
        let seed = maps.seed.as_ref().ok_or_else(|| BuildError::new("warp", "no Seed to build a place from"))?;
        let build = rules.0.build(target_map, PlaceSeed::of(seed.0, target_map.0))?;
        let stairs = rules.0.ladder().map(|ladder| ladder.stairs(target_map, build.entry, build.exit)).unwrap_or_default();
        maps.map.install_place(target_map, build);
        // Laid with the build, by whoever's arrival built it, so a floor
        // has its way back up before anyone stands on it.
        for LadderStair { at, climb, look, to } in stairs {
            // A stair up lands where the floor above goes down, and a stair
            // down where the floor below comes up.
            let arrive = match climb {
                Climb::Up => Arrive::Exit,
                Climb::Down => Arrive::Entry,
            };
            let name = Name::new(look.name.clone());
            commands.spawn((Position(at), OnMap(target_map), Transition { to: Destination::Place { map: to, arrive } }, Stair { climb, look }, name));
        }
    }
    let Ok((_, (mut pos, viewshed, on, blocks, player, ..))) = travellers.get_mut(actor) else {
        return Err(BuildError::new("warp", "the traveller has no position"));
    };
    let from = on.map(|m| m.0).unwrap_or(MapId::ZERO);
    let want = match (arrive, maps.map.place(target_map)) {
        (Arrive::At(p), _) => p,
        (Arrive::Entry, Some(place)) => place.entry,
        (Arrive::Exit, Some(place)) => place.exit.unwrap_or(place.entry),
        (_, None) => return Err(BuildError::new("warp", "a place built but not kept")),
    };
    // Found before anything is switched, so a refused arrival leaves the
    // traveller and the maps exactly as they were.
    let landing = if blocks {
        maps.occupancy.remove_on(from, pos.0, actor);
        match atglyph_grid::landing::landing(want, |p| !maps.occupancy.is_occupied_on(target_map, p), |p| maps.map.is_walkable_on(target_map, p), LANDING_REACH)
        {
            Some(landing) => landing,
            None => {
                maps.occupancy.insert_on(from, pos.0, actor);
                return Err(BuildError::new("warp", "nowhere to stand on the far side"));
            }
        }
    } else {
        want
    };
    // The map being read follows the player and nobody else.
    let moved = from != target_map;
    if player && moved {
        maps.map.switch_to(target_map);
        maps.occupancy.switch(target_map);
        maps.knowledge.switch(target_map);
        maps.fields.invalidate();
    }
    let left = std::mem::replace(&mut pos.0, landing);
    if blocks {
        maps.occupancy.insert_on(target_map, landing, actor);
    }
    if let Some(mut v) = viewshed {
        v.dirty = true;
    }
    commands.entity(actor).insert(OnMap(target_map));
    if !moved {
        return Ok(());
    }
    report.travelled.write(Travelled { actor, from, left, to: target_map, after });
    if player {
        report.changed.write(MapChanged { from, to: target_map });
        let first = maps.map.visit(target_map);
        if let Some(place) = maps.map.place(target_map) {
            report.entered.write(PlaceEntered { map: target_map, first, entry: place.entry, exit: place.exit });
        }
    } else {
        // A post is a cell of the map it was given on, and means nothing
        // on another: an actor that left its map has left its post.
        commands.entity(actor).remove::<Post>();
    }
    Ok(())
}

/// Puts anything that just gained a position on the current map, unless
/// it says otherwise.
pub(crate) fn tag_new_positions(mut commands: Commands, map: Res<WorldMap>, fresh: Query<Entity, (Added<Position>, Without<OnMap>)>) {
    for e in &fresh {
        commands.entity(e).insert(OnMap(map.current()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{MyTurn, RevealsMap};
    use crate::items::{DropItem, Inventory, Item, ItemEvent, PickUp};
    use crate::state::EngineState;
    use crate::testing::headless_app;
    use crate::turn::Wait;
    use crate::turn::{ActionRefused, Turns};
    use atglyph_core::Rect;
    use atglyph_grid::{TileRegistry, TileTables};
    use atglyph_mapgen::dungeon::{FarthestExit, RandomStart, Rooms};
    use atglyph_mapgen::passes::StartPoint;
    use atglyph_mapgen::{BaseContext, Chain};

    struct Caves(TileRegistry);
    impl PlaceRules for Caves {
        fn build(&self, _: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
            let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
            let mut ctx = BaseContext::blank(40, 30, self.0.clone(), wall);
            Chain::new().then(Rooms { floor, ..Default::default() }).then(RandomStart).then(FarthestExit).run(&mut ctx, seed)?;
            let entry = ctx.outputs().first::<StartPoint>().unwrap().0;
            let exit = ctx.outputs().first::<atglyph_mapgen::dungeon::ExitPoint>().map(|e| e.0);
            let (terrain, _) = ctx.finish();
            Ok(PlaceBuild {
                terrain,
                entry,
                exit,
                spots: vec![Spot { tag: 7, at: entry, prefab: None }],
                locks: Vec::new(),
                secrets: Vec::new(),
                ambient: atglyph_grid::Light::DARK,
                peaceful: false,
            })
        }

        fn tiles(&self) -> TileTables {
            self.0.tables()
        }
    }

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
    }

    fn rig() -> Rig {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::items::ItemsPlugin));
        let start = crate::testing::ground(&mut app);
        app.insert_resource(PlaceRulesRes(Box::new(Caves(TileRegistry::standard())))).insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Inventory::default())).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        Rig { app, player, start }
    }

    fn intend<A: Action>(rig: &mut Rig, action: A) {
        rig.app.world_mut().write_message(Intent { actor: rig.player, action });
        rig.app.update();
    }

    /// Every place entered, recorded by a reader rather than peeked at in
    /// the buffer, whose swap depends on wall time in a headless app.
    #[derive(Resource, Default)]
    struct Entered(Vec<PlaceEntered>);

    fn record_entered(mut events: MessageReader<PlaceEntered>, mut entered: ResMut<Entered>) {
        entered.0.extend(events.read().copied());
    }

    /// An onlooker is a player that is no actor: it takes no turn, and the
    /// map being read still follows it, so a run nobody plays is set in a
    /// place by warping its onlooker there.
    #[test]
    fn the_map_being_read_follows_an_onlooker_into_a_place() {
        let mut app = headless_app();
        app.add_plugins(crate::fov::FovPlugin);
        let start = crate::testing::ground(&mut app);
        app.insert_resource(PlaceRulesRes(Box::new(Caves(TileRegistry::standard()))))
            .insert_resource(crate::seed::Seed(crate::testing::TEST_SEED))
            .init_resource::<Entered>()
            .add_systems(PostUpdate, record_entered);
        let onlooker = app.world_mut().spawn((Player, Position(start), Viewshed::everywhere(), RevealsMap)).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        let cave = MapId(3);
        app.world_mut().write_message(WarpRequest::into_place(onlooker, cave));
        app.update();
        // Someone put in the place once it is the map being read acts there.
        let entry = app.world().resource::<WorldMap>().place(cave).expect("the place was built").entry;
        let dweller = app.world_mut().spawn((Actor, Blocks, Position(entry))).id();
        for _ in 0..3 {
            app.update();
        }

        let w = app.world();
        let map = w.resource::<WorldMap>();
        assert_eq!(map.current(), cave, "the place is the map being read");
        let place = map.place(cave).unwrap();
        assert_eq!(w.resource::<Entered>().0, vec![PlaceEntered { map: cave, first: true, entry: place.entry, exit: place.exit }], "and it was entered, once");
        assert!(w.get::<MyTurn>(onlooker).is_none(), "the onlooker is never dealt a turn");
        let sight = w.get::<Viewshed>(onlooker).unwrap();
        assert_eq!(sight.visible.count(), 40 * 30, "it sees the whole place, walls and all");
        assert!(w.resource::<Knowledge>().is_explored(Point::new(0, 0)), "and remembers its far corner");
        assert_eq!(w.get::<OnMap>(dweller).map(|m| m.0), Some(cave));
        assert!(w.resource::<Turns>().now() > 0, "whoever is in the place is dealt turns with nobody playing");
    }

    #[test]
    fn a_transition_takes_the_player_down_and_a_warp_brings_it_back_to_the_same_place() {
        let mut r = rig();
        r.app.init_resource::<Entered>().add_systems(PostUpdate, record_entered);
        let cave = MapId(3);
        r.app.world_mut().spawn((Position(r.start), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let watcher = r.app.world_mut().spawn((Actor, Blocks, Position(r.start.offset(2, 0)))).id();
        r.app.update();
        assert_eq!(r.app.world().get::<OnMap>(watcher).map(|m| m.0), Some(MapId::ZERO), "tagged on arrival");

        intend(&mut r, GoThrough);
        {
            let w = r.app.world();
            let map = w.resource::<WorldMap>();
            assert_eq!(map.current(), cave);
            let place = map.place(cave).unwrap();
            assert_eq!(w.get::<Position>(r.player).unwrap().0, place.entry);
            assert_eq!(w.get::<OnMap>(r.player).unwrap().0, cave);
            assert!(map.is_walkable(place.entry));
            assert_eq!(map.window_tiles(), Rect::new(0, 0, 40, 30), "the place is the whole window");
            assert!(w.resource::<Occupancy>().is_occupied(place.entry));
            assert!(!w.resource::<Occupancy>().is_occupied(r.start.offset(2, 0)), "the watcher on map zero is not on this map's index");
            assert_eq!(w.resource::<Entered>().0, vec![PlaceEntered { map: cave, first: true, entry: place.entry, exit: place.exit }]);
            assert_eq!(place.spots, vec![Spot { tag: 7, at: place.entry, prefab: None }]);
            let v = w.get::<Viewshed>(r.player).unwrap();
            assert!(!v.dirty && v.can_see(place.entry), "sight was recomputed in the place");
            assert!(w.resource::<Knowledge>().is_explored(place.entry));
        }
        // Time passes below; the watcher above is frozen, not dealt a turn.
        let before = r.app.world().resource::<Turns>().now();
        intend(&mut r, Wait);
        intend(&mut r, Wait);
        assert!(r.app.world().resource::<Turns>().now() > before);
        assert!(r.app.world().get::<MyTurn>(watcher).is_none());

        // Drop something here, leave, and come back: it is where it was.
        let entry = r.app.world().resource::<WorldMap>().place(cave).unwrap().entry;
        let coin = r.app.world_mut().spawn((Item, Position(entry))).id();
        r.app.update();
        assert_eq!(r.app.world().get::<OnMap>(coin).unwrap().0, cave);
        r.app.world_mut().write_message(WarpRequest { actor: r.player, to: Destination::Place { map: MapId::ZERO, arrive: Arrive::At(r.start) }, after: None });
        r.app.update();
        assert_eq!(r.app.world().resource::<WorldMap>().current(), MapId::ZERO);
        assert_eq!(r.app.world().get::<Position>(r.player).unwrap().0, r.start);
        assert!(r.app.world().resource::<WorldMap>().is_loaded(r.start), "map zero is back");
        assert!(r.app.world().resource::<Knowledge>().is_explored(r.start), "what was seen of map zero survived the trip");
        assert!(r.app.world().resource::<Occupancy>().is_occupied(r.start.offset(2, 0)), "the watcher is indexed again");
        intend(&mut r, GoThrough);
        {
            let w = r.app.world();
            assert_eq!(w.resource::<WorldMap>().current(), cave);
            let entered = &w.resource::<Entered>().0;
            assert_eq!(entered.len(), 3, "into the cave, back to map zero, and into the cave again: {entered:?}");
            assert_eq!((entered[1].map, entered[1].first), (MapId::ZERO, false), "map zero was where the player began: {entered:?}");
            assert!(!entered[2].first, "built once: {entered:?}");
            assert!(w.resource::<Knowledge>().is_explored(entry), "place knowledge survived too");
            assert_eq!(w.get::<Position>(coin).unwrap().0, entry);
        }
        // And the coin can be picked up: ground lookups see this map.
        intend(&mut r, PickUp);
        let events: Vec<ItemEvent> = r.app.world_mut().resource_mut::<Messages<ItemEvent>>().drain().collect();
        assert_eq!(events.len(), 1);
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().contains(coin));
    }

    /// A rig with two sides and minds, for whoever else travels.
    fn crowd() -> (Rig, crate::testing::TwoSides) {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::items::ItemsPlugin, crate::combat::CombatPlugin, crate::minds::MindsPlugin));
        let start = crate::testing::ground(&mut app);
        let sides = crate::testing::two_sides(&mut app);
        app.insert_resource(PlaceRulesRes(Box::new(Caves(TileRegistry::standard()))));
        app.init_resource::<Went>().add_systems(PostUpdate, record_travels);
        let player = app
            .world_mut()
            .spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap, crate::combat::Health::full(30), crate::combat::Faction(sides.ours)))
            .id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        (Rig { app, player, start }, sides)
    }

    /// Everyone who travelled, recorded by a reader as `Entered` is.
    #[derive(Resource, Default)]
    struct Went(Vec<Travelled>);

    fn record_travels(mut events: MessageReader<Travelled>, mut went: ResMut<Went>) {
        went.0.extend(events.read().copied());
    }

    fn mind(
        r: &mut Rig,
        at: Point,
        side: atglyph_rules::FactionId,
        kind: atglyph_rules::damage::DamageKindId,
        wits: Wits,
        brain: atglyph_rules::Brain<Entity>,
    ) -> Entity {
        use crate::combat::{Faction, Health, MeleeAttack};
        use crate::minds::{Mind, Perception};
        r.app
            .world_mut()
            .spawn((
                Actor,
                Blocks,
                Position(at),
                Health::full(10),
                Faction(side),
                Perception(8),
                Intelligence(wits),
                MeleeAttack::new(kind, atglyph_core::DiceRoll::flat(1)),
                Mind(std::sync::Arc::new(brain)),
            ))
            .id()
    }

    fn on(r: &Rig, e: Entity) -> MapId {
        r.app.world().get::<OnMap>(e).map_or(MapId::ZERO, |m| m.0)
    }

    fn at(r: &Rig, e: Entity) -> Point {
        r.app.world().get::<Position>(e).unwrap().0
    }

    /// A hunter that travels and stood beside the player goes through
    /// behind it and lands beside it, still hunting; one whose kind does
    /// not travel stays, and so does one that travels but was not beside
    /// the player when it left.
    #[test]
    fn a_hunter_that_travels_follows_the_player_through_and_one_that_does_not_stays() {
        use atglyph_rules::ai::tactics::{Hunt, MeleeAdjacent};
        let (mut r, sides) = crowd();
        let cave = MapId(3);
        r.app.world_mut().spawn((Position(r.start), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let hunts = || atglyph_rules::Brain::new().then(MeleeAdjacent).then(Hunt);
        let travels = Wits::SAPIENT.with(Wits::TRAVELS);
        let (beside, stays, far) = (r.start.offset(1, 0), r.start.offset(0, 1), r.start.offset(5, 0));
        let follower = mind(&mut r, beside, sides.theirs, sides.kind, travels, hunts());
        let homebody = mind(&mut r, stays, sides.theirs, sides.kind, Wits::SAPIENT, hunts());
        let straggler = mind(&mut r, far, sides.theirs, sides.kind, travels, hunts());
        intend(&mut r, Wait);
        assert!(r.app.world().get::<Trails>(follower).unwrap().0.contains(&r.player), "it is after the player");
        assert!(r.app.world().get::<Trails>(homebody).unwrap().0.is_empty(), "one that does not travel keeps no trail");

        intend(&mut r, GoThrough);
        let entry = r.app.world().resource::<WorldMap>().place(cave).unwrap().entry;
        assert_eq!((on(&r, r.player), at(&r, r.player)), (cave, entry));
        assert_eq!(on(&r, follower), cave, "the one beside it came through");
        assert_eq!(atglyph_core::geometry::chebyshev(at(&r, follower), entry), 1, "and stands beside it, not on it");
        assert!(r.app.world().resource::<Occupancy>().is_occupied(at(&r, follower)), "indexed where it landed");
        assert_eq!((on(&r, homebody), at(&r, homebody)), (MapId::ZERO, stays), "its kind does not travel");
        assert_eq!(on(&r, straggler), MapId::ZERO, "it was not beside the player");
        assert_eq!(
            r.app.world().resource::<Went>().0,
            vec![
                Travelled { actor: r.player, from: MapId::ZERO, left: r.start, to: cave, after: None },
                Travelled { actor: follower, from: MapId::ZERO, left: beside, to: cave, after: Some(r.player) },
            ]
        );

        let before = r.app.world().get::<crate::combat::Health>(r.player).unwrap().current;
        intend(&mut r, Wait);
        intend(&mut r, Wait);
        assert!(r.app.world().get::<crate::combat::Health>(r.player).unwrap().current < before, "and it goes on with the fight below");
    }

    /// A companion comes through with the one it keeps beside, whatever
    /// decided its last turn: idling inside the distance it keeps, it is
    /// still a companion.
    #[test]
    fn a_companion_that_travels_comes_through_with_the_player() {
        use atglyph_rules::ai::tactics::Keep;
        let (mut r, sides) = crowd();
        let cave = MapId(3);
        r.app.world_mut().spawn((Position(r.start), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let beside = r.start.offset(1, 0);
        let friend = mind(&mut r, beside, sides.ours, sides.kind, Wits::SAPIENT.with(Wits::TRAVELS), atglyph_rules::Brain::new().then(Keep::allies(3, 1)));
        intend(&mut r, Wait);
        assert_eq!(r.app.world().get::<crate::minds::LastTactic>(friend).unwrap().0, None, "close enough that nothing decided its turn");
        intend(&mut r, GoThrough);
        assert_eq!(on(&r, friend), cave);
        assert_eq!(atglyph_core::geometry::chebyshev(at(&r, friend), at(&r, r.player)), 1);
    }

    /// A warp the game sends after someone says so on its journey, and
    /// goes nowhere when the one it follows was refused the same pass.
    #[test]
    fn a_warp_sent_after_someone_is_told_as_following_them_and_stays_when_they_could_not_go() {
        let (mut r, _) = crowd();
        let cave = MapId(3);
        let (player, friend) = (r.player, r.app.world_mut().spawn((Actor, Blocks, Position(r.start.offset(2, 0)))).id());
        let into = |map: MapId| Destination::Place { map, arrive: Arrive::Entry };
        // A cell taken and walled in on every side leaves nowhere to land,
        // so the player's warp there is refused, and the one sent after
        // them stays.
        let pen = r.start.offset(-5, -5);
        r.app.world_mut().spawn((Actor, Blocks, Position(pen)));
        let ring: Vec<Point> = atglyph_core::Direction::ALL.iter().map(|d| pen + d.offset()).collect();
        crate::testing::wall_off(&mut r.app, &ring);
        r.app.update();
        r.app.world_mut().write_message(WarpRequest::new(player, Destination::Place { map: MapId::ZERO, arrive: Arrive::At(pen) }));
        r.app.world_mut().write_message(WarpRequest::new(friend, into(cave)).after(player));
        intend(&mut r, Wait);
        assert_eq!(on(&r, friend), MapId::ZERO, "the one it followed went nowhere, and nor did it");
        r.app.world_mut().write_message(WarpRequest::into_place(player, cave));
        r.app.world_mut().write_message(WarpRequest::new(friend, into(cave)).after(player));
        intend(&mut r, Wait);
        assert_eq!((on(&r, player), on(&r, friend)), (cave, cave), "both went");
        let went = &r.app.world().resource::<Went>().0;
        assert_eq!(went.iter().find(|t| t.actor == friend).map(|t| t.after), Some(Some(player)), "after the player: {went:?}");
        assert_eq!(went.iter().find(|t| t.actor == player).map(|t| t.after), Some(None), "and the player of its own accord");
    }

    /// A hurt mind that travels runs for the way out and takes it. The
    /// floor it lands on is built for it and waits, unvisited, so the
    /// player's own arrival is still the first, and the player lands
    /// beside what is standing on the entry rather than on it.
    #[test]
    fn a_hurt_mind_that_travels_leaves_by_a_way_and_the_floor_is_still_new_to_the_player() {
        use atglyph_rules::ai::tactics::{FleeWhenHurt, Hunt};
        let (mut r, sides) = crowd();
        r.app.init_resource::<Entered>().add_systems(PostUpdate, record_entered);
        let cave = MapId(3);
        let way = r.start.offset(4, 0);
        r.app.world_mut().spawn((Position(way), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let brain = atglyph_rules::Brain::new().then(FleeWhenHurt { at_pct: 50 }).then(Hunt);
        let from = r.start.offset(3, 0);
        let runner = mind(&mut r, from, sides.theirs, sides.kind, Wits::ANIMAL.with(Wits::TRAVELS), brain);
        r.app.world_mut().get_mut::<crate::combat::Health>(runner).unwrap().current = 2;
        r.app.world_mut().entity_mut(runner).insert(Post(way));
        intend(&mut r, Wait);
        assert_eq!((on(&r, runner), at(&r, runner)), (MapId::ZERO, way), "onto the way first");
        intend(&mut r, Wait);
        assert_eq!(on(&r, runner), cave, "then through it");
        {
            let w = r.app.world();
            let map = w.resource::<WorldMap>();
            assert_eq!(map.current(), MapId::ZERO, "the map being read stays with the player");
            let place = map.place(cave).expect("built for the one that arrived");
            assert!(!place.visited);
            assert_eq!(at(&r, runner), place.entry);
            assert!(!w.resource::<Occupancy>().is_occupied(way), "no longer standing on the way");
            assert!(w.resource::<Occupancy>().is_occupied_on(cave, place.entry), "and indexed on the map it went to");
            assert!(w.get::<Post>(runner).is_none(), "a post is a cell of the map it left");
            assert!(w.resource::<Entered>().0.is_empty(), "nobody has entered it as far as the game is told");
        }

        r.app.world_mut().write_message(WarpRequest::into_place(r.player, cave));
        r.app.update();
        let entered = r.app.world().resource::<Entered>().0.clone();
        assert!(entered.len() == 1 && entered[0].first, "the player's arrival is the first: {entered:?}");
        assert_eq!(atglyph_core::geometry::chebyshev(at(&r, r.player), at(&r, runner)), 1, "beside what stands on the entry");
        assert!(r.app.world().resource::<WorldMap>().is_walkable(at(&r, r.player)));
    }

    /// Whatever its brain decides, a mind whose kind does not travel is
    /// refused the way: the wits gate the action itself, not only the
    /// tactics that would choose it.
    #[test]
    fn going_through_is_refused_to_a_mind_whose_kind_does_not_travel() {
        struct Through;
        impl atglyph_rules::ai::Tactic<Entity> for Through {
            fn name(&self) -> &'static str {
                "through"
            }
            fn evaluate(&self, _: &mut atglyph_rules::ai::TacticCtx<'_, Entity>) -> Option<atglyph_rules::Decision<Entity>> {
                Some(atglyph_rules::Decision::GoThrough)
            }
        }
        let (mut r, sides) = crowd();
        let cave = MapId(3);
        let way = r.start.offset(3, 0);
        r.app.world_mut().spawn((Position(way), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let stuck = mind(&mut r, way, sides.theirs, sides.kind, Wits::SAPIENT, atglyph_rules::Brain::new().then(Through));
        intend(&mut r, Wait);
        intend(&mut r, Wait);
        assert_eq!((on(&r, stuck), at(&r, stuck)), (MapId::ZERO, way));
        assert!(!r.app.world().resource::<WorldMap>().has_place(cave), "and nothing was built for it");

        r.app.world_mut().get_mut::<Intelligence>(stuck).unwrap().0 = Wits::SAPIENT.with(Wits::TRAVELS);
        intend(&mut r, Wait);
        assert_eq!(on(&r, stuck), cave, "the same brain, once its kind travels");
    }

    /// Found while building throwing: an item picked up on one map and put
    /// down on another kept the map it was picked up on, so it lay where
    /// nobody on the map it was dropped on could see it or take it back.
    #[test]
    fn an_item_carried_to_another_map_and_dropped_lies_on_the_map_it_was_dropped_on() {
        let mut r = rig();
        let coin = r.app.world_mut().spawn((Item, Position(r.start))).id();
        r.app.update();
        intend(&mut r, PickUp);
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().contains(coin));

        let cave = MapId(3);
        r.app.world_mut().write_message(WarpRequest::into_place(r.player, cave));
        r.app.update();
        intend(&mut r, DropItem(coin));
        assert_eq!(r.app.world().get::<OnMap>(coin).map(|m| m.0), Some(cave), "it lies on the map it was dropped on");
        intend(&mut r, PickUp);
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().contains(coin), "and can be taken back up there");
    }

    #[test]
    fn entering_off_a_transition_is_refused_for_free_and_goes_nowhere() {
        let mut r = rig();
        /// Trips taken, and refusals given, from now on.
        #[derive(Resource, Default)]
        struct Went(usize, usize);
        r.app.init_resource::<Went>().add_systems(
            PostUpdate,
            |mut went: ResMut<Went>, mut trips: MessageReader<Travelled>, mut no: MessageReader<ActionRefused>| {
                went.0 += trips.read().count();
                went.1 += no.read().count();
            },
        );
        let before = r.app.world().resource::<Turns>().now();
        intend(&mut r, GoThrough);
        assert_eq!(r.app.world().resource::<Turns>().now(), before);
        assert!(r.app.world().get::<MyTurn>(r.player).is_some());
        assert_eq!(r.app.world().resource::<WorldMap>().current(), MapId::ZERO);
        assert_eq!(r.app.world().get::<Position>(r.player).map(|p| p.0), Some(r.start));
        assert_eq!(r.app.world().resource::<Went>().0, 0, "and nobody travelled");
        assert_eq!(r.app.world().resource::<Went>().1, 1, "refused once, not again by the sweep for an ask nothing claimed");
    }

    /// A game may read one key as two asks, its way through and what is
    /// here. Off any way the first finds nothing, and the turn is still the
    /// second's to spend, rather than claimed by an ask that did nothing.
    #[test]
    fn a_way_asked_for_off_any_way_leaves_the_same_press_free_to_take_up_what_is_underfoot() {
        let mut r = rig();
        let coin = r.app.world_mut().spawn((Item, Position(r.start))).id();
        r.app.update();
        let before = r.app.world().resource::<Turns>().now();
        r.app.world_mut().write_message(Intent { actor: r.player, action: GoThrough });
        intend(&mut r, PickUp);
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().contains(coin), "the same press picked the coin up");
        assert!(r.app.world().resource::<Turns>().now() > before, "and spent the turn on it");
    }

    #[test]
    fn a_player_spawned_into_a_place_stands_on_its_entry() {
        use bevy::ecs::system::RunSystemOnce;
        let mut app = headless_app();
        app.add_plugins(crate::fov::FovPlugin);
        crate::testing::ground(&mut app);
        app.places(Caves(TileRegistry::standard())).insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        let cave = MapId(3);
        let player = app
            .world_mut()
            .run_system_once(move |mut commands: Commands| commands.spawn_player(cave, (Viewshed::new(6), RevealsMap)).id())
            .expect("the spawn ran");
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();

        let w = app.world();
        let entry = w.resource::<WorldMap>().place(cave).expect("the place was built").entry;
        assert_eq!(w.get::<Position>(player).map(|p| p.0), Some(entry), "the player stands on the entry");
        assert_eq!(w.get::<OnMap>(player), Some(&OnMap(cave)), "of the place it was spawned into");
        assert_eq!(w.resource::<WorldMap>().current(), cave, "which is the map being read");
        assert!(
            w.get::<Actor>(player).is_some() && w.get::<Blocks>(player).is_some() && w.get::<RevealsMap>(player).is_some(),
            "an actor that blocks, with its bundle"
        );
    }

    /// The caves, noting the seed each place was handed.
    struct Recording(Caves, std::sync::Arc<std::sync::Mutex<Vec<(MapId, PlaceSeed)>>>);

    impl PlaceRules for Recording {
        fn build(&self, map: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
            self.1.lock().unwrap().push((map, seed));
            self.0.build(map, seed)
        }

        fn tiles(&self) -> TileTables {
            self.0.tiles()
        }
    }

    #[test]
    fn a_place_is_built_from_its_own_seed() {
        let mut r = rig();
        let handed = std::sync::Arc::default();
        r.app.insert_resource(PlaceRulesRes(Box::new(Recording(Caves(TileRegistry::standard()), std::sync::Arc::clone(&handed)))));
        for map in [MapId(3), MapId(2)] {
            r.app.world_mut().write_message(WarpRequest::into_place(r.player, map));
            r.app.update();
        }
        let run = crate::testing::TEST_SEED;
        assert_eq!(*handed.lock().unwrap(), vec![(MapId(3), PlaceSeed::of(run, 3)), (MapId(2), PlaceSeed::of(run, 2))], "each from the run and its own number");
    }

    #[test]
    fn a_warp_in_a_game_with_no_seed_builds_nothing() {
        let mut r = rig();
        r.app.world_mut().remove_resource::<crate::seed::Seed>();
        r.app.world_mut().write_message(WarpRequest::into_place(r.player, MapId(3)));
        r.app.update();
        assert!(r.app.world().resource::<WorldMap>().place(MapId(3)).is_none(), "no place is built from a seed nobody gave");
        assert_eq!(r.app.world().get::<Position>(r.player).unwrap().0, r.start, "and the traveller stays");
    }

    /// The caves again, as the floors of a ladder: maps three, four and
    /// five, top first, with a way of the game's own past the top.
    struct LadderCaves(Caves);

    impl PlaceRules for LadderCaves {
        fn build(&self, map: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
            self.0.build(map, seed)
        }

        fn tiles(&self) -> TileTables {
            self.0.tiles()
        }

        fn ladder(&self) -> Option<Ladder> {
            let (up, down) = (
                StairLook::new('<', atglyph_grid::Rgb::new(230, 220, 160), "stairs up"),
                StairLook::new('>', atglyph_grid::Rgb::new(230, 220, 160), "stairs down"),
            );
            Some(Ladder::new([MapId(3), MapId(4), MapId(5)], up, down).top(LadderEnd::Game))
        }
    }

    /// Every way through standing on `map`: where, where it leads, and
    /// which way it climbs when it is a stair.
    fn ways(app: &mut App, map: MapId) -> Vec<(Point, Destination, Option<Climb>)> {
        let world = app.world_mut();
        let mut q = world.query::<(&Position, &OnMap, &Transition, Option<&Stair>)>();
        let mut found: Vec<(Point, Destination, Option<Climb>)> =
            q.iter(world).filter(|(_, on, _, _)| on.0 == map).map(|(p, _, t, s)| (p.0, t.to, s.map(|s| s.climb))).collect();
        found.sort_by_key(|(p, _, c)| (*p, *c));
        found
    }

    /// Down a ladder of three floors from map zero: every floor has one
    /// stair up at its entry to the floor above's exit, but the top, whose
    /// way up is the game's and which the engine leaves alone, and one
    /// stair down at its exit to the floor below's entry, but the bottom.
    /// Each is laid once, when its floor is built, and named as its look
    /// says.
    #[test]
    fn the_engine_lays_one_stair_up_and_one_down_on_every_floor_of_a_ladder_but_past_its_ends() {
        let mut r = rig();
        r.app.insert_resource(PlaceRulesRes(Box::new(LadderCaves(Caves(TileRegistry::standard())))));
        let player = r.player;
        r.app.world_mut().write_message(WarpRequest::into_place(player, MapId(3)));
        r.app.update();
        for (i, map) in [MapId(3), MapId(4), MapId(5)].into_iter().enumerate() {
            let (entry, exit) = {
                let place = r.app.world().resource::<WorldMap>().place(map).expect("built on arrival");
                (place.entry, place.exit.expect("a cave with a far point"))
            };
            let mut expected = Vec::new();
            if i > 0 {
                expected.push((entry, Destination::Place { map: MapId(map.0 - 1), arrive: Arrive::Exit }, Some(Climb::Up)));
            }
            if i < 2 {
                expected.push((exit, Destination::Place { map: MapId(map.0 + 1), arrive: Arrive::Entry }, Some(Climb::Down)));
            }
            expected.sort_by_key(|(p, _, c)| (*p, *c));
            assert_eq!(ways(&mut r.app, map), expected, "{map:?}");
            if i < 2 {
                let at = r
                    .app
                    .world_mut()
                    .query::<(&Position, &OnMap, &Stair, &Name)>()
                    .iter(r.app.world())
                    .find(|(_, on, s, _)| on.0 == map && s.climb == Climb::Down)
                    .map(|(p, _, _, n)| (p.0, n.as_str().to_string()));
                assert_eq!(at, Some((exit, "stairs down".to_string())));
                r.app.world_mut().write_message(WarpRequest { actor: player, to: Destination::Place { map, arrive: Arrive::At(exit) }, after: None });
                r.app.update();
                intend(&mut r, GoThrough);
                assert_eq!(r.app.world().resource::<WorldMap>().current(), MapId(map.0 + 1), "down the stairs from {map:?}");
            }
        }
        r.app.world_mut().write_message(WarpRequest::into_place(player, MapId(4)));
        r.app.update();
        assert_eq!(ways(&mut r.app, MapId(4)).len(), 2, "coming back lays nothing twice");
    }

    /// A game with no ladder gets no stairs: its places are entered and
    /// left only by the ways it lays itself.
    #[test]
    fn a_game_with_no_ladder_gets_no_stairs() {
        let mut r = rig();
        let player = r.player;
        r.app.world_mut().write_message(WarpRequest::into_place(player, MapId(3)));
        r.app.update();
        assert_eq!(r.app.world().resource::<WorldMap>().current(), MapId(3));
        assert!(ways(&mut r.app, MapId(3)).is_empty());
        assert_eq!(r.app.world_mut().query::<&Stair>().iter(r.app.world()).count(), 0);
    }
}
