//! Places: bounded maps entered from the surface and from each other.
//!
//! The surface is the streamed world. A place is a bounded map built once
//! by the game's [`PlaceRules`] the first time something enters it and
//! kept whole after that, actors and items included: leaving a place
//! freezes what is in it, returning finds it as it was. Every entity with
//! a position is on exactly one map, named by [`OnMap`]; the scheduler
//! freezes actors on other maps and the readers of the current map do not
//! see them.
//!
//! A [`Transition`] is an entity standing on a cell; [`GoThrough`] on
//! that cell takes an actor through it, and a [`WarpRequest`] does the
//! same from anywhere, for portals. The map being read follows the player.
//! Anyone else travels only when the game says its kind does, with
//! [`Wits::TRAVELS`]: such a mind goes through after whoever it was
//! hunting or keeping beside, when it stood next to them as they left,
//! and takes a way out when it runs. It is frozen where it lands until
//! the player comes, like everything else on a map nobody is reading.

use bevy::prelude::*;
use rl_core::Point;
use rl_core::turn::BASE_ACTION_COST;
use rl_grid::Terrain;
use rl_mapgen::BuildError;
use rl_rules::Wits;
use rl_world::WorldGraph;

use crate::combat::Dead;
use crate::components::{Blocks, Player, Position, Viewshed};
use crate::knowledge::Knowledge;
use crate::minds::{FlowFields, Intelligence, Post, Trails};
use crate::turn::{Action, Intent, Occupancy, Resolution};
use crate::world::{WorldMap, WorldRes};

/// Which map an entity is on. Zero is the surface; a game numbers its
/// places however it likes above that.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct MapId(pub u32);

impl MapId {
    /// The streamed world.
    pub const SURFACE: MapId = MapId(0);

    /// Whether this is the surface.
    pub const fn is_surface(self) -> bool {
        self.0 == 0
    }
}

/// The map an entity with a position is on. Missing means the surface;
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
    /// A cell of the surface.
    Surface(Point),
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

/// A point of interest the builder reports, for the game to populate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Spot {
    /// What kind, in the game's own numbering.
    pub tag: u32,
    /// Where.
    pub at: Point,
    /// The key of the prefab whose mark this is, when it was keyed, so the
    /// engine can find what the mark stands for. `None` for a spot a game
    /// made itself or a mark of an unkeyed piece. A mark a later stamp
    /// covered is never a spot, since the piece drawn on top owns its
    /// cells and what the earlier one meant there is no longer on the map.
    #[serde(default)]
    pub prefab: Option<u32>,
}

/// What building a place produced.
#[derive(Debug)]
pub struct PlaceBuild {
    /// The tiles.
    pub terrain: Terrain,
    /// Where an arrival from above stands.
    pub entry: Point,
    /// Where an arrival from below stands, if the place goes deeper.
    pub exit: Option<Point>,
    /// Points of interest for the game's spawns.
    pub spots: Vec<Spot>,
}

impl PlaceBuild {
    /// A build from a finished chain: the entry is the chain's
    /// [`StartPoint`](rl_mapgen::passes::StartPoint), the exit its
    /// [`ExitPoint`](rl_mapgen::dungeon::ExitPoint) if one was emitted,
    /// and every prefab mark becomes a spot tagged with its character.
    /// Fails if no start was emitted.
    ///
    /// A mark inside the bounds of a stamp emitted after its own is
    /// dropped, keyed or not: the piece drawn on top owns its cells, so a
    /// slot or a mark it painted over never fills a cell that now belongs
    /// to another piece.
    pub fn from_context(ctx: rl_mapgen::BaseContext) -> Result<Self, BuildError> {
        use rl_mapgen::BuildContext;
        let entry = ctx.outputs().first::<rl_mapgen::passes::StartPoint>().ok_or_else(|| BuildError::new("place", "the chain emitted no start point"))?.0;
        let exit = ctx.outputs().first::<rl_mapgen::dungeon::ExitPoint>().map(|e| e.0);
        let stamps: Vec<&rl_mapgen::prefab::Stamped> = ctx.outputs().iter::<rl_mapgen::prefab::Stamped>().collect();
        let covered = |i: usize, p: Point| stamps[i + 1..].iter().any(|later| later.bounds.contains(p));
        let spots = stamps
            .iter()
            .enumerate()
            .flat_map(|(i, s)| s.marks.iter().filter(move |(_, p)| !covered(i, *p)).map(|(c, p)| Spot { tag: *c as u32, at: *p, prefab: s.prefab }))
            .collect();
        let (terrain, _) = ctx.finish();
        Ok(Self { terrain, entry, exit, spots })
    }
}

/// How the game builds a place the first time it is entered.
pub trait PlaceRules: Send + Sync {
    /// Builds `map`. The world, when the game has one, is there for the
    /// seed and for whatever the place is under; a delve with no surface
    /// gets `None` and seeds itself.
    fn build(&self, map: MapId, world: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError>;
}

/// The game's place builder.
#[derive(Resource)]
pub struct PlaceRulesRes(pub Box<dyn PlaceRules>);

/// Go through the [`Transition`] on the actor's cell. Refused off one,
/// and refused to anyone but the player whose
/// [`Intelligence`](crate::minds::Intelligence) lacks
/// [`Wits::TRAVELS`]: a monster stays on the map it was put on unless the
/// game says its kind does not.
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
}

impl WarpRequest {
    /// Into `map` at its entry: how a delve starts on its first floor.
    pub fn into_place(actor: Entity, map: MapId) -> Self {
        Self { actor, to: Destination::Place { map, arrive: Arrive::Entry } }
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
pub struct Maps<'w> {
    map: ResMut<'w, WorldMap>,
    occupancy: ResMut<'w, Occupancy>,
    knowledge: ResMut<'w, Knowledge>,
    fields: ResMut<'w, FlowFields>,
    world: Option<Res<'w, WorldRes>>,
    rules: Option<Res<'w, PlaceRulesRes>>,
}

/// What a warp reports.
#[derive(bevy::ecs::system::SystemParam)]
pub struct WarpReport<'w> {
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
pub struct Travel<'w, 's> {
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
/// for a mind without [`Wits::TRAVELS`]. A warp is never followed, since
/// nobody saw where it went.
pub fn resolve_warps(
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
        if !resolution.claim(intent.actor) {
            continue;
        }
        let Ok((_, (pos, _, on, _, player, wits, ..))) = travellers.get(intent.actor) else {
            resolution.failed(intent.actor, BASE_ACTION_COST);
            continue;
        };
        if !player && !wits.is_some_and(|w| w.has(Wits::TRAVELS)) {
            resolution.failed(intent.actor, BASE_ACTION_COST);
            continue;
        }
        let (at, here) = (pos.0, on.map(|m| m.0).unwrap_or(MapId::SURFACE));
        let found = transitions.iter().find(|(p, m, _)| p.0 == at && m.map(|m| m.0).unwrap_or(MapId::SURFACE) == here).map(|(_, _, t)| t.to);
        let Some(to) = found else {
            resolution.failed(intent.actor, BASE_ACTION_COST);
            continue;
        };
        trips.push(Trip { actor: intent.actor, to, is_action: true, after: None });
        // Nearest first, then by entity, so who gets the cell beside the
        // landing never depends on the order a query happened to run in.
        let mut behind: Vec<(i32, Entity)> = travellers
            .iter()
            .filter(|(e, (pos, _, on, _, _, wits, trails, dead))| {
                *e != intent.actor
                    && !dead
                    && on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here
                    && rl_core::geometry::chebyshev(pos.0, at) <= 1
                    && wits.is_some_and(|w| w.has(Wits::TRAVELS))
                    && trails.is_some_and(|t| t.0.contains(&intent.actor))
            })
            .map(|(e, (pos, ..))| (rl_core::geometry::chebyshev(pos.0, at), e))
            .collect();
        behind.sort();
        trips.extend(behind.into_iter().map(|(_, e)| Trip { actor: e, to, is_action: false, after: Some(intent.actor) }));
    }
    for req in requests.read() {
        debug!("warp request {req:?}");
        if travellers.get(req.actor).is_ok() {
            trips.push(Trip { actor: req.actor, to: req.to, is_action: false, after: None });
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

/// Where an arrival on `map` that takes up its cell stands: `want` when
/// nothing stands there, and otherwise the nearest free cell that can be
/// walked to from it.
///
/// `want` is taken on trust, as it always was: it is where the builder or
/// the game said arrivals stand, and on the surface it may not be loaded
/// yet. Only the cells searched round it are asked whether they are floor,
/// so the second through a way lands beside the first rather than on it or
/// in a wall.
fn landing(maps: &Maps, map: MapId, want: Point) -> Option<Point> {
    if !maps.occupancy.is_occupied_on(map, want) {
        return Some(want);
    }
    let mut seen = vec![want];
    let mut next = 0;
    while next < seen.len() && seen.len() < LANDING_REACH {
        let from = seen[next];
        next += 1;
        for dir in rl_core::Direction::ALL {
            let p = from + dir.offset();
            if seen.contains(&p) || !maps.map.is_walkable_on(map, p) {
                continue;
            }
            if !maps.occupancy.is_occupied_on(map, p) {
                return Some(p);
            }
            seen.push(p);
        }
    }
    None
}

fn warp(
    commands: &mut Commands,
    maps: &mut Maps,
    report: &mut WarpReport,
    travellers: &mut Traveller,
    actor: Entity,
    to: Destination,
    after: Option<Entity>,
) -> Result<(), BuildError> {
    let (target_map, arrive) = match to {
        Destination::Surface(p) => (MapId::SURFACE, Arrive::At(p)),
        Destination::Place { map, arrive } => (map, arrive),
    };
    if !target_map.is_surface() && !maps.map.has_place(target_map) {
        let rules = maps.rules.as_ref().ok_or_else(|| BuildError::new("warp", "no PlaceRulesRes to build a place with"))?;
        let build = rules.0.build(target_map, maps.world.as_deref().map(|w| &w.0))?;
        maps.map.install_place(target_map, build);
    }
    let Ok((_, (mut pos, viewshed, on, blocks, player, ..))) = travellers.get_mut(actor) else {
        return Err(BuildError::new("warp", "the traveller has no position"));
    };
    let from = on.map(|m| m.0).unwrap_or(MapId::SURFACE);
    let want = match (arrive, maps.map.place(target_map)) {
        (Arrive::At(p), _) => p,
        (Arrive::Entry, Some(place)) => place.entry,
        (Arrive::Exit, Some(place)) => place.exit.unwrap_or(place.entry),
        (_, None) => return Err(BuildError::new("warp", "the surface has no entry or exit")),
    };
    // Found before anything is switched, so a refused arrival leaves the
    // traveller and the maps exactly as they were.
    let landing = if blocks {
        maps.occupancy.remove_on(from, pos.0, actor);
        match landing(maps, target_map, want) {
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
pub fn tag_new_positions(mut commands: Commands, map: Res<WorldMap>, fresh: Query<Entity, (Added<Position>, Without<OnMap>)>) {
    for e in &fresh {
        commands.entity(e).insert(OnMap(map.current()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, MyTurn, RevealsMap};
    use crate::items::{DropItem, Inventory, Item, ItemEvent, PickUp};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::Turns;
    use crate::turn::Wait;
    use rl_core::{Rect, RunSeed};
    use rl_grid::TileRegistry;
    use rl_mapgen::dungeon::{FarthestExit, RandomStart, Rooms};
    use rl_mapgen::passes::StartPoint;
    use rl_mapgen::{BaseContext, BuildContext, Chain};

    struct Caves(TileRegistry);
    impl PlaceRules for Caves {
        fn build(&self, map: MapId, world: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
            let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
            let mut ctx = BaseContext::blank(40, 30, self.0.clone(), wall);
            let world = world.expect("the test has a world");
            Chain::new()
                .then(Rooms { floor, ..Default::default() })
                .then(RandomStart)
                .then(FarthestExit)
                .run(&mut ctx, RunSeed(world.seed().0 ^ map.0 as u64))?;
            let entry = ctx.outputs().first::<StartPoint>().unwrap().0;
            let exit = ctx.outputs().first::<rl_mapgen::dungeon::ExitPoint>().map(|e| e.0);
            let (terrain, _) = ctx.finish();
            Ok(PlaceBuild { terrain, entry, exit, spots: vec![Spot { tag: 7, at: entry, prefab: None }] })
        }
    }

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
    }

    fn rig() -> Rig {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::items::ItemsPlugin, crate::world::StreamingPlugin));
        let start = crate::testing::surface(&mut app);
        app.insert_resource(PlaceRulesRes(Box::new(Caves(TileRegistry::standard()))));
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

    #[test]
    fn a_transition_takes_the_player_down_and_a_warp_brings_it_back_to_the_same_place() {
        let mut r = rig();
        r.app.init_resource::<Entered>().add_systems(PostUpdate, record_entered);
        let cave = MapId(3);
        r.app.world_mut().spawn((Position(r.start), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let watcher = r.app.world_mut().spawn((Actor, Blocks, Position(r.start.offset(2, 0)))).id();
        r.app.update();
        assert_eq!(r.app.world().get::<OnMap>(watcher).map(|m| m.0), Some(MapId::SURFACE), "tagged on arrival");

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
            assert!(!w.resource::<Occupancy>().is_occupied(r.start.offset(2, 0)), "the surface watcher is not on this map's index");
            assert_eq!(w.resource::<Entered>().0, vec![PlaceEntered { map: cave, first: true, entry: place.entry, exit: place.exit }]);
            assert_eq!(place.spots, vec![Spot { tag: 7, at: place.entry, prefab: None }]);
            let v = w.get::<Viewshed>(r.player).unwrap();
            assert!(!v.dirty && v.can_see(place.entry), "sight was recomputed in the place");
            assert!(w.resource::<Knowledge>().is_explored(place.entry));
            let surface_region = w.resource::<WorldRes>().region_of_tile(r.start);
            assert!(w.resource::<Knowledge>().region_touched(surface_region), "the overworld's fog of the surface survives going underground");
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
        r.app.world_mut().write_message(WarpRequest { actor: r.player, to: Destination::Surface(r.start) });
        r.app.update();
        assert_eq!(r.app.world().resource::<WorldMap>().current(), MapId::SURFACE);
        assert_eq!(r.app.world().get::<Position>(r.player).unwrap().0, r.start);
        assert!(r.app.world().resource::<WorldMap>().is_loaded(r.start), "the surface window is back");
        assert!(r.app.world().resource::<Knowledge>().is_explored(r.start), "surface knowledge survived the trip");
        assert!(r.app.world().resource::<Occupancy>().is_occupied(r.start.offset(2, 0)), "the watcher is indexed again");
        intend(&mut r, GoThrough);
        {
            let w = r.app.world();
            assert_eq!(w.resource::<WorldMap>().current(), cave);
            let entered = &w.resource::<Entered>().0;
            assert_eq!(entered.len(), 2, "entered twice: {entered:?}");
            assert!(!entered[1].first, "built once: {entered:?}");
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
    fn crowd() -> (Rig, crate::testing::Sides) {
        let mut app = headless_app();
        app.add_plugins((
            crate::fov::FovPlugin,
            crate::items::ItemsPlugin,
            crate::world::StreamingPlugin,
            crate::combat::CombatPlugin,
            crate::minds::MindsPlugin,
        ));
        let start = crate::testing::surface(&mut app);
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

    fn mind(r: &mut Rig, at: Point, side: rl_rules::FactionId, kind: rl_rules::damage::DamageKindId, wits: Wits, brain: rl_rules::Brain<Entity>) -> Entity {
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
                MeleeAttack::new(kind, rl_core::DiceRoll::flat(1)),
                Mind(std::sync::Arc::new(brain)),
            ))
            .id()
    }

    fn on(r: &Rig, e: Entity) -> MapId {
        r.app.world().get::<OnMap>(e).map_or(MapId::SURFACE, |m| m.0)
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
        use rl_rules::ai::tactics::{Hunt, MeleeAdjacent};
        let (mut r, sides) = crowd();
        let cave = MapId(3);
        r.app.world_mut().spawn((Position(r.start), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let hunts = || rl_rules::Brain::new().then(MeleeAdjacent).then(Hunt);
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
        assert_eq!(rl_core::geometry::chebyshev(at(&r, follower), entry), 1, "and stands beside it, not on it");
        assert!(r.app.world().resource::<Occupancy>().is_occupied(at(&r, follower)), "indexed where it landed");
        assert_eq!((on(&r, homebody), at(&r, homebody)), (MapId::SURFACE, stays), "its kind does not travel");
        assert_eq!(on(&r, straggler), MapId::SURFACE, "it was not beside the player");
        assert_eq!(
            r.app.world().resource::<Went>().0,
            vec![
                Travelled { actor: r.player, from: MapId::SURFACE, left: r.start, to: cave, after: None },
                Travelled { actor: follower, from: MapId::SURFACE, left: beside, to: cave, after: Some(r.player) },
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
        use rl_rules::ai::tactics::Keep;
        let (mut r, sides) = crowd();
        let cave = MapId(3);
        r.app.world_mut().spawn((Position(r.start), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let beside = r.start.offset(1, 0);
        let friend = mind(&mut r, beside, sides.ours, sides.kind, Wits::SAPIENT.with(Wits::TRAVELS), rl_rules::Brain::new().then(Keep::allies(3, 1)));
        intend(&mut r, Wait);
        assert_eq!(r.app.world().get::<crate::minds::Doing>(friend).unwrap().0, None, "close enough that nothing decided its turn");
        intend(&mut r, GoThrough);
        assert_eq!(on(&r, friend), cave);
        assert_eq!(rl_core::geometry::chebyshev(at(&r, friend), at(&r, r.player)), 1);
    }

    /// A hurt mind that travels runs for the way out and takes it. The
    /// floor it lands on is built for it and waits, unvisited, so the
    /// player's own arrival is still the first, and the player lands
    /// beside what is standing on the entry rather than on it.
    #[test]
    fn a_hurt_mind_that_travels_leaves_by_a_way_and_the_floor_is_still_new_to_the_player() {
        use rl_rules::ai::tactics::{FleeWhenHurt, Hunt};
        let (mut r, sides) = crowd();
        r.app.init_resource::<Entered>().add_systems(PostUpdate, record_entered);
        let cave = MapId(3);
        let way = r.start.offset(4, 0);
        r.app.world_mut().spawn((Position(way), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let brain = rl_rules::Brain::new().then(FleeWhenHurt { at_pct: 50 }).then(Hunt);
        let from = r.start.offset(3, 0);
        let runner = mind(&mut r, from, sides.theirs, sides.kind, Wits::ANIMAL.with(Wits::TRAVELS), brain);
        r.app.world_mut().get_mut::<crate::combat::Health>(runner).unwrap().current = 2;
        r.app.world_mut().entity_mut(runner).insert(Post(way));
        intend(&mut r, Wait);
        assert_eq!((on(&r, runner), at(&r, runner)), (MapId::SURFACE, way), "onto the way first");
        intend(&mut r, Wait);
        assert_eq!(on(&r, runner), cave, "then through it");
        {
            let w = r.app.world();
            let map = w.resource::<WorldMap>();
            assert_eq!(map.current(), MapId::SURFACE, "the map being read stays with the player");
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
        assert_eq!(rl_core::geometry::chebyshev(at(&r, r.player), at(&r, runner)), 1, "beside what stands on the entry");
        assert!(r.app.world().resource::<WorldMap>().is_walkable(at(&r, r.player)));
    }

    /// Whatever its brain decides, a mind whose kind does not travel is
    /// refused the way: the wits gate the action itself, not only the
    /// tactics that would choose it.
    #[test]
    fn going_through_is_refused_to_a_mind_whose_kind_does_not_travel() {
        struct Through;
        impl rl_rules::ai::Tactic<Entity> for Through {
            fn name(&self) -> &'static str {
                "through"
            }
            fn evaluate(&self, _: &mut rl_rules::ai::TacticCtx<'_, Entity>) -> Option<rl_rules::Decision<Entity>> {
                Some(rl_rules::Decision::GoThrough)
            }
        }
        let (mut r, sides) = crowd();
        let cave = MapId(3);
        let way = r.start.offset(3, 0);
        r.app.world_mut().spawn((Position(way), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let stuck = mind(&mut r, way, sides.theirs, sides.kind, Wits::SAPIENT, rl_rules::Brain::new().then(Through));
        intend(&mut r, Wait);
        intend(&mut r, Wait);
        assert_eq!((on(&r, stuck), at(&r, stuck)), (MapId::SURFACE, way));
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
    fn a_keyed_stamps_marks_become_spots_carrying_its_key() {
        use rl_mapgen::prefab::{Orient, Placement, Prefab, StampPrefab};
        use rl_mapgen::{BaseContext, BuildContext, Chain};
        let tiles = rl_grid::TileRegistry::standard();
        let wall = tiles.expect("wall");
        let piece = Prefab::parse(&["#A#"], |c| (c == '#').then_some(wall)).unwrap().keyed(3);
        let mut ctx = BaseContext::blank(10, 10, tiles.clone(), tiles.expect("floor"));
        Chain::new()
            .then(StampPrefab { name: "keyed", prefab: piece, at: Placement::At(Point::new(1, 1)), orient: Orient::Fixed })
            .run(&mut ctx, rl_core::RunSeed(0))
            .unwrap();
        ctx.emit(rl_mapgen::passes::StartPoint(Point::new(5, 5)));
        let build = PlaceBuild::from_context(ctx).unwrap();
        assert_eq!(build.spots, vec![Spot { tag: 'A' as u32, at: Point::new(2, 1), prefab: Some(3) }]);
    }

    #[test]
    fn a_mark_under_a_later_stamp_is_dropped_and_the_piece_on_top_keeps_its_own() {
        use rl_mapgen::prefab::{Orient, Placement, Prefab, StampPrefab};
        use rl_mapgen::{BaseContext, BuildContext, Chain};
        let tiles = rl_grid::TileRegistry::standard();
        let wall = tiles.expect("wall");
        let under = Prefab::parse(&["A.B"], |c| (c == '.').then_some(wall)).unwrap().keyed(1);
        let over = Prefab::parse(&["C"], |_| None).unwrap().keyed(2);
        let mut ctx = BaseContext::blank(10, 10, tiles.clone(), tiles.expect("floor"));
        Chain::new()
            .then(StampPrefab { name: "under", prefab: under, at: Placement::At(Point::new(1, 1)), orient: Orient::Fixed })
            .then(StampPrefab { name: "over", prefab: over, at: Placement::At(Point::new(3, 1)), orient: Orient::Fixed })
            .run(&mut ctx, rl_core::RunSeed(0))
            .unwrap();
        ctx.emit(rl_mapgen::passes::StartPoint(Point::new(5, 5)));
        let build = PlaceBuild::from_context(ctx).unwrap();
        assert_eq!(
            build.spots,
            vec![Spot { tag: 'A' as u32, at: Point::new(1, 1), prefab: Some(1) }, Spot { tag: 'C' as u32, at: Point::new(3, 1), prefab: Some(2) }],
            "the first piece's `B` at (3, 1) is under the second, whose `C` owns the cell"
        );
    }

    #[test]
    fn entering_off_a_transition_is_refused_for_free() {
        let mut r = rig();
        let before = r.app.world().resource::<Turns>().now();
        intend(&mut r, GoThrough);
        assert_eq!(r.app.world().resource::<Turns>().now(), before);
        assert!(r.app.world().get::<MyTurn>(r.player).is_some());
        assert_eq!(r.app.world().resource::<WorldMap>().current(), MapId::SURFACE);
    }
}
