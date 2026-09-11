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
//! Only the player travels. A [`Transition`] is an entity standing on a
//! cell; [`Action::Enter`] on that cell takes the player through it. A
//! [`WarpRequest`] does the same from anywhere, for portals.

use bevy::prelude::*;
use rl_core::Point;
use rl_core::turn::BASE_ACTION_COST;
use rl_grid::Terrain;
use rl_mapgen::BuildError;
use rl_world::WorldGraph;

use crate::combat::FlowFields;
use crate::components::{Blocks, MyTurn, Player, Position, Viewshed};
use crate::knowledge::Knowledge;
use crate::turn::{Action, ActionDone, ActionRefused, Intent, Occupancy};
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
    pub fn from_context(ctx: rl_mapgen::BaseContext) -> Result<Self, BuildError> {
        use rl_mapgen::BuildContext;
        let entry = ctx.outputs().first::<rl_mapgen::passes::StartPoint>().ok_or_else(|| BuildError::new("place", "the chain emitted no start point"))?.0;
        let exit = ctx.outputs().first::<rl_mapgen::dungeon::ExitPoint>().map(|e| e.0);
        let spots = ctx.outputs().iter::<rl_mapgen::prefab::Stamped>().flat_map(|s| s.marks.iter().map(|(c, p)| Spot { tag: *c as u32, at: *p })).collect();
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

/// Asks the engine to move the player somewhere, building the place if
/// needed. Resolved in [`TurnSet::Resolve`](crate::plugin::TurnSet::Resolve)
/// without costing a turn; a game charges what it likes.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarpRequest {
    /// Who. Only the player travels; anyone else is ignored.
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

/// The player changed maps.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapChanged {
    /// From.
    pub from: MapId,
    /// To.
    pub to: MapId,
}

/// A place was entered. `first` is true the one time it was just built,
/// which is when a game populates it.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceEntered {
    /// Which.
    pub map: MapId,
    /// Whether it was built for this arrival.
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
    done: MessageWriter<'w, ActionDone>,
    refused: MessageWriter<'w, ActionRefused>,
    changed: MessageWriter<'w, MapChanged>,
    entered: MessageWriter<'w, PlaceEntered>,
}

/// The player as a traveller.
type Traveller<'w, 's> = Query<'w, 's, (&'static mut Position, Option<&'static mut Viewshed>, Option<&'static OnMap>, Has<Blocks>), With<Player>>;

/// Transitions, wherever they stand. Never the player, which keeps this
/// disjoint from the traveller's mutable position.
type Transitions<'w, 's> = Query<'w, 's, (&'static Position, Option<&'static OnMap>, &'static Transition), Without<Player>>;

/// Who travels and what they travel through.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Travel<'w, 's> {
    travellers: Traveller<'w, 's>,
    transitions: Transitions<'w, 's>,
    holding: Query<'w, 's, (), (With<Player>, With<MyTurn>)>,
}

/// Takes the player through the transition it stands on for an
/// [`Action::Enter`], and anywhere a [`WarpRequest`] asks.
pub fn resolve_warps(
    mut commands: Commands,
    mut intents: MessageReader<Intent>,
    mut requests: MessageReader<WarpRequest>,
    mut maps: Maps,
    mut report: WarpReport,
    travel: Travel,
) {
    let Travel { mut travellers, transitions, holding } = travel;
    let mut trips: Vec<(Entity, Destination, bool)> = Vec::new();
    for intent in intents.read() {
        if intent.action != Action::Enter || holding.get(intent.actor).is_err() {
            continue;
        }
        let Ok((pos, _, on, _)) = travellers.get(intent.actor) else { continue };
        let here = on.map(|m| m.0).unwrap_or(MapId::SURFACE);
        let found = transitions.iter().find(|(p, m, _)| p.0 == pos.0 && m.map(|m| m.0).unwrap_or(MapId::SURFACE) == here).map(|(_, _, t)| t.to);
        match found {
            Some(to) => trips.push((intent.actor, to, true)),
            None => {
                report.refused.write(ActionRefused { actor: intent.actor });
            }
        }
    }
    for req in requests.read() {
        debug!("warp request {req:?}");
        if travellers.get(req.actor).is_ok() {
            trips.push((req.actor, req.to, false));
        }
    }
    for (actor, to, is_action) in trips {
        match warp(&mut commands, &mut maps, &mut report, &mut travellers, actor, to) {
            Ok(()) => {
                if is_action {
                    report.done.write(ActionDone { actor, cost: BASE_ACTION_COST });
                }
            }
            Err(e) => {
                error!("warp failed: {e}");
                if is_action {
                    report.refused.write(ActionRefused { actor });
                }
            }
        }
    }
}

fn warp(
    commands: &mut Commands,
    maps: &mut Maps,
    report: &mut WarpReport,
    travellers: &mut Traveller,
    actor: Entity,
    to: Destination,
) -> Result<(), BuildError> {
    let (target_map, arrive) = match to {
        Destination::Surface(p) => (MapId::SURFACE, Arrive::At(p)),
        Destination::Place { map, arrive } => (map, arrive),
    };
    let mut first = false;
    if !target_map.is_surface() && !maps.map.has_place(target_map) {
        let rules = maps.rules.as_ref().ok_or_else(|| BuildError::new("warp", "no PlaceRulesRes to build a place with"))?;
        let build = rules.0.build(target_map, maps.world.as_deref().map(|w| &w.0))?;
        maps.map.install_place(target_map, build);
        first = true;
    }
    let Ok((mut pos, viewshed, on, blocks)) = travellers.get_mut(actor) else {
        return Err(BuildError::new("warp", "the traveller has no position"));
    };
    let from = on.map(|m| m.0).unwrap_or(MapId::SURFACE);
    if blocks {
        maps.occupancy.remove(pos.0, actor);
    }
    if from != target_map {
        maps.map.switch_to(target_map);
        maps.occupancy.switch(target_map);
        maps.knowledge.switch(target_map);
        maps.fields.invalidate();
    }
    let landing = match (arrive, maps.map.place(target_map)) {
        (Arrive::At(p), _) => p,
        (Arrive::Entry, Some(place)) => place.entry,
        (Arrive::Exit, Some(place)) => place.exit.unwrap_or(place.entry),
        (_, None) => return Err(BuildError::new("warp", "the surface has no entry or exit")),
    };
    pos.0 = landing;
    if blocks {
        maps.occupancy.insert(landing, actor);
    }
    if let Some(mut v) = viewshed {
        v.dirty = true;
    }
    commands.entity(actor).insert(OnMap(target_map));
    if from != target_map {
        report.changed.write(MapChanged { from, to: target_map });
        if let Some(place) = maps.map.place(target_map) {
            report.entered.write(PlaceEntered { map: target_map, first, entry: place.entry, exit: place.exit });
        }
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
    use crate::components::{Actor, RevealsMap, Speed};
    use crate::items::{Inventory, Item, ItemEvent};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::Turns;
    use crate::world::ChunkRulesRes;
    use rl_core::{Rect, RunSeed};
    use rl_grid::{TileId, TileRegistry};
    use rl_mapgen::dungeon::{FarthestExit, RandomStart, Rooms};
    use rl_mapgen::passes::StartPoint;
    use rl_mapgen::{BaseContext, BuildContext, Chain};
    use rl_world::{BandId, CellFacts, ChunkContext, ChunkRules, Layers, Site, Surroundings, WorldConfig, WorldRules};

    struct Flat;
    impl WorldRules for Flat {
        fn classify(&self, f: &CellFacts) -> BandId {
            BandId(if f.is_sea { 0 } else { 1 })
        }
        fn road_friction(&self, _: BandId, _: &CellFacts) -> Option<f32> {
            None
        }
        fn settlements(&self, _: &Layers, _: u64) -> Vec<Site> {
            Vec::new()
        }
    }
    struct Open(TileRegistry);
    impl ChunkRules for Open {
        fn tiles(&self) -> &TileRegistry {
            &self.0
        }
        fn fill(&self, _: &Surroundings) -> TileId {
            self.0.expect("floor")
        }
        fn chain(&self, _: &WorldGraph, _: &Surroundings) -> Chain<ChunkContext> {
            Chain::new().then(rl_mapgen::passes::Fill { tile: self.0.expect("floor") })
        }
    }
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
            Ok(PlaceBuild { terrain, entry, exit, spots: vec![Spot { tag: 7, at: entry }] })
        }
    }

    struct Rig {
        app: App,
        player: Entity,
        start: Point,
    }

    fn rig() -> Rig {
        let mut app = headless_app();
        let tiles = TileRegistry::standard();
        let world = WorldGraph::generate(RunSeed(5), WorldConfig { region_size: 16, ..WorldConfig::regions(12, 10) }, &Flat);
        let (region, _) = world.layers().bands.iter().find(|(_, b)| b.0 == 1).expect("land");
        let start = world.tile_origin(region).offset(8, 8);
        app.insert_resource(WorldMap::new(16, tiles.tables()));
        app.insert_resource(WorldRes(world));
        app.insert_resource(ChunkRulesRes(Box::new(Open(tiles.clone()))));
        app.insert_resource(PlaceRulesRes(Box::new(Caves(tiles))));
        app.insert_resource(Knowledge::new(16));
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(6), RevealsMap, Speed(100), Inventory::default())).id();
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        Rig { app, player, start }
    }

    fn intend(rig: &mut Rig, action: Action) {
        rig.app.world_mut().write_message(Intent { actor: rig.player, action });
        rig.app.update();
    }

    #[test]
    fn a_transition_takes_the_player_down_and_a_warp_brings_it_back_to_the_same_place() {
        let mut r = rig();
        let cave = MapId(3);
        r.app.world_mut().spawn((Position(r.start), Transition { to: Destination::Place { map: cave, arrive: Arrive::Entry } }));
        let watcher = r.app.world_mut().spawn((Actor, Blocks, Position(r.start.offset(2, 0)), Speed(100))).id();
        r.app.update();
        assert_eq!(r.app.world().get::<OnMap>(watcher).map(|m| m.0), Some(MapId::SURFACE), "tagged on arrival");

        intend(&mut r, Action::Enter);
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
            let entered: Vec<PlaceEntered> = w.resource::<Messages<PlaceEntered>>().iter_current_update_messages().copied().collect();
            assert_eq!(entered, vec![PlaceEntered { map: cave, first: true, entry: place.entry, exit: place.exit }]);
            assert_eq!(place.spots, vec![Spot { tag: 7, at: place.entry }]);
            let v = w.get::<Viewshed>(r.player).unwrap();
            assert!(!v.dirty && v.can_see(place.entry), "sight was recomputed in the place");
            assert!(w.resource::<Knowledge>().is_explored(place.entry));
        }
        // Time passes below; the watcher above is frozen, not dealt a turn.
        let before = r.app.world().resource::<Turns>().now();
        intend(&mut r, Action::Wait);
        intend(&mut r, Action::Wait);
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
        intend(&mut r, Action::Enter);
        {
            let w = r.app.world();
            assert_eq!(w.resource::<WorldMap>().current(), cave);
            let entered: Vec<PlaceEntered> = w.resource::<Messages<PlaceEntered>>().iter_current_update_messages().copied().collect();
            // Messages linger until a fixed update, so look at the latest.
            assert!(!entered.last().unwrap().first, "built once: {entered:?}");
            assert!(w.resource::<Knowledge>().is_explored(entry), "place knowledge survived too");
            assert_eq!(w.get::<Position>(coin).unwrap().0, entry);
        }
        // And the coin can be picked up: ground lookups see this map.
        intend(&mut r, Action::PickUp);
        let events: Vec<ItemEvent> = r.app.world_mut().resource_mut::<Messages<ItemEvent>>().drain().collect();
        assert_eq!(events.len(), 1);
        assert!(r.app.world().get::<Inventory>(r.player).unwrap().contains(coin));
    }

    #[test]
    fn entering_off_a_transition_is_refused_for_free() {
        let mut r = rig();
        let before = r.app.world().resource::<Turns>().now();
        intend(&mut r, Action::Enter);
        assert_eq!(r.app.world().resource::<Turns>().now(), before);
        assert!(r.app.world().get::<MyTurn>(r.player).is_some());
        assert_eq!(r.app.world().resource::<WorldMap>().current(), MapId::SURFACE);
    }
}
