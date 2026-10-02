//! Charts: what someone other than the player has seen of each map.
//!
//! [`Knowledge`](crate::knowledge::Knowledge) is what the player has seen,
//! and the map view draws from it. A chart is the same record kept for
//! whoever a game says: one scout, a party, a whole side. A mind carrying
//! [`Charting`] writes what it sees into its chart on each of its turns,
//! and is told, as the [`Uncharted`] sense, the ground the chart does not
//! hold yet and could be walked onto, which is what the
//! [`Explore`](rl_rules::ai::tactics::Explore) tactic walks to. Minds that
//! name one chart share it: what one has seen none of them goes to look at.
//!
//! A chart outlives whoever filled it, and [`Charts::copy`] hands what one
//! holds to another, so what a scout saw can be given to those who come
//! after. What a chart means is the game's: the engine keeps tiles and
//! never asks whose side an id is.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_core::{Direction, Point};
use rl_grid::BitGrid;
use rl_rules::ai::tactics::Uncharted;

use crate::components::{MyTurn, Player, Viewshed};
use crate::knowledge::TileSet;
use crate::minds::{Mind, Thinking};
use crate::places::MapId;
use crate::world::WorldMap;

/// Which chart, in the game's own numbering: a party, a side, one scout.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct ChartId(pub u32);

/// Says that what this mind sees is written into a chart, and which.
///
/// On a [`Mind`]: its sight is charted on each turn it is dealt, and it is
/// told what its chart lacks. Two minds naming one chart fill one record.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Charting(pub ChartId);

/// Every chart, per map.
///
/// Inserted by [`MindsPlugin`](crate::minds::MindsPlugin) and emptied when
/// a run ends. A chart nobody has written to holds nothing and costs
/// nothing; one is made by the first mark.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct Charts(BTreeMap<(ChartId, MapId), TileSet>);

impl Charts {
    /// Whether `chart` holds the tile `p` of `map`.
    pub fn holds(&self, chart: ChartId, map: MapId, p: Point) -> bool {
        self.0.get(&(chart, map)).is_some_and(|seen| seen.contains(p))
    }

    /// Records the tile `p` of `map` in `chart`.
    pub fn mark(&mut self, chart: ChartId, map: MapId, p: Point) {
        self.0.entry((chart, map)).or_default().insert(p);
    }

    /// What `chart` holds of `map`, if anything.
    pub fn of(&self, chart: ChartId, map: MapId) -> Option<&TileSet> {
        self.0.get(&(chart, map))
    }

    /// How many tiles of `map` are in `chart`.
    pub fn count(&self, chart: ChartId, map: MapId) -> usize {
        self.of(chart, map).map_or(0, TileSet::count)
    }

    /// Adds everything `from` holds, of every map, to `into`, and leaves
    /// `from` as it was: what one learned handed on to another.
    pub fn copy(&mut self, from: ChartId, into: ChartId) {
        if from == into {
            return;
        }
        let held: Vec<(MapId, TileSet)> = self.0.iter().filter(|((chart, _), _)| *chart == from).map(|((_, map), seen)| (*map, seen.clone())).collect();
        for (map, seen) in held {
            self.0.entry((into, map)).or_default().union_with(&seen);
        }
    }

    /// Empties `chart`, on every map.
    pub fn forget(&mut self, chart: ChartId) {
        self.0.retain(|(held, _), _| *held != chart);
    }

    /// Every chart, for saving.
    pub fn export(&self) -> ChartsSave {
        ChartsSave(self.0.iter().map(|((chart, map), seen)| ChartSave { chart: *chart, map: *map, tiles: seen.export() }).collect())
    }

    /// Replaces every chart with a save.
    pub fn import(&mut self, save: ChartsSave) {
        self.0 = save.0.into_iter().map(|held| ((held.chart, held.map), TileSet::import(held.tiles))).collect();
    }
}

/// What [`Charts::export`] produces: each chart of each map.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChartsSave(pub Vec<ChartSave>);

/// One chart of one map, as a save holds it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChartSave {
    /// Which chart.
    pub chart: ChartId,
    /// Of which map.
    pub map: MapId,
    /// The tiles it holds, in buckets.
    pub tiles: Vec<(Point, BitGrid)>,
}

/// A mind holding the turn that keeps a chart.
type Charter = (With<Mind>, With<MyTurn>, Without<Player>);

/// Writes what the mind holding the turn sees into its chart.
///
/// After its sight is recast for the turn, so the chart holds what it sees
/// from where it now stands before it is told what the chart lacks.
pub fn chart_sight(map: Res<WorldMap>, mut charts: ResMut<Charts>, viewers: Query<(&Viewshed, &Charting), Charter>) {
    let Ok((sight, charting)) = viewers.single() else { return };
    let seen = charts.0.entry((charting.0, map.current())).or_default();
    for p in sight.iter() {
        seen.insert(p);
    }
}

/// Tells the mind holding the turn what ground its chart lacks: every
/// cell of the loaded window it could stand on that the chart does not
/// hold, beside one it could stand on that the chart does.
///
/// Beside by a straight step, not a diagonal, so each is a step from known
/// ground whatever the corners round it. A tile that opens counts as
/// ground to stand on, since a mind that works doors walks through one.
/// Nothing is pushed for a mind with no [`Charting`], and an empty list
/// when its chart is whole.
pub fn sense_uncharted(mut thinking: ResMut<Thinking>, map: Res<WorldMap>, charts: Res<Charts>, charting: Query<&Charting>) {
    let Some(chart) = thinking.actor().and_then(|actor| charting.get(actor).ok()).map(|c| c.0) else { return };
    let Some(snapshot) = thinking.snapshot_mut() else { return };
    let here = map.current();
    let ground = |p: Point| map.is_walkable(p) || map.opens(p).is_some();
    let unseen: Vec<Point> = match charts.of(chart, here) {
        Some(seen) => {
            let known_ground = |p: Point| seen.contains(p) && ground(p);
            map.window_tiles()
                .cells()
                .filter(|p| !seen.contains(*p) && ground(*p) && Direction::CARDINALS.iter().any(|d| known_ground(*p + d.offset())))
                .collect()
        }
        None => Vec::new(),
    };
    snapshot.add_sense(Uncharted(unseen));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Blocks, Position, RevealsMap};
    use crate::minds::{MindsPlugin, Perception};
    use crate::places::{PlaceBuild, PlaceRules, PlaceRulesRes, WarpRequest};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use rl_grid::{Terrain, TileRegistry};
    use rl_rules::ai::Brain;
    use rl_rules::ai::tactics::Explore;
    use std::sync::Arc;

    /// Two rooms and a side chamber, joined by doorways no wider than a
    /// cell, so nothing is seen whole from where an explorer starts.
    const HALLS: [&str; 9] = [
        "#####################",
        "#.....#.......#.....#",
        "#.....#.......#.....#",
        "#.....###.#####.....#",
        "#...................#",
        "#.....####.####.....#",
        "#.....#.......#.....#",
        "#.....#...#...#.....#",
        "#####################",
    ];
    const HALL: MapId = MapId(1);

    struct Halls(TileRegistry);
    impl PlaceRules for Halls {
        fn build(&self, _: MapId, _: Option<&rl_world::WorldGraph>) -> Result<PlaceBuild, rl_mapgen::BuildError> {
            let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
            let terrain =
                Terrain::from_fn(
                    HALLS[0].len() as i32,
                    HALLS.len() as i32,
                    |p| if HALLS[p.y as usize].as_bytes()[p.x as usize] == b'#' { wall } else { floor },
                );
            Ok(PlaceBuild { terrain, entry: Point::new(1, 1), exit: None, spots: Vec::new() })
        }
    }

    /// The halls as the map being read, under an onlooker, with nobody in
    /// them yet.
    fn halls() -> App {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, MindsPlugin));
        let tiles = TileRegistry::standard();
        app.insert_resource(WorldMap::new(tiles.tables()))
            .insert_resource(PlaceRulesRes(Box::new(Halls(tiles))))
            .insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        let onlooker = app.world_mut().spawn((Player, Position(Point::ZERO), Viewshed::everywhere(), RevealsMap)).id();
        app.world_mut().write_message(WarpRequest::into_place(onlooker, HALL));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
        assert_eq!(app.world().resource::<WorldMap>().current(), HALL);
        app
    }

    fn explorer(app: &mut App, at: Point, chart: ChartId) -> Entity {
        app.world_mut().spawn((Actor, Blocks, Position(at), Perception(4), Charting(chart), Mind(Arc::new(Brain::new().then(Explore))))).id()
    }

    /// Every cell of the halls an actor could stand on.
    fn ground(app: &App) -> Vec<Point> {
        let map = app.world().resource::<WorldMap>();
        map.window_tiles().cells().filter(|p| map.is_walkable(*p)).collect()
    }

    /// An explorer that sees four cells walks the halls until its chart
    /// holds every cell it could stand on, and then has nothing to do.
    #[test]
    fn an_explorer_charts_every_cell_it_can_reach_and_then_stops() {
        let mut app = halls();
        let chart = ChartId(7);
        // One step a second, watched for a hundredth of one: its first turn.
        app.insert_resource(crate::turn::Pace::per_second(100))
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_millis(10)));
        let scout = explorer(&mut app, Point::new(1, 1), chart);
        app.update();
        app.update();
        let charted_ground = |app: &App| ground(app).into_iter().filter(|p| app.world().resource::<Charts>().holds(chart, HALL, *p)).count();
        let at_first = charted_ground(&app);
        assert!(at_first > 0 && at_first < ground(&app).len(), "it starts knowing only what it can see: {at_first}");
        app.insert_resource(crate::turn::Pace::unpaced());
        for _ in 0..12 {
            app.update();
        }
        let charts = app.world().resource::<Charts>();
        let missed: Vec<Point> = ground(&app).into_iter().filter(|p| !charts.holds(chart, HALL, *p)).collect();
        assert!(missed.is_empty(), "every cell it could stand on is charted, but not {missed:?}");
        assert!(!charts.holds(ChartId(8), HALL, Point::new(1, 1)), "and nobody else's chart was written");
        let rested = app.world().get::<Position>(scout).unwrap().0;
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().get::<Position>(scout).unwrap().0, rested, "with nothing left to find it stays where it is");
        assert_eq!(app.world().get::<crate::minds::Doing>(scout), Some(&crate::minds::Doing(None)), "no tactic decided");
    }

    /// What one chart holds, handed to another, is ground the second never
    /// goes to look at.
    #[test]
    fn a_mind_given_a_whole_chart_has_nothing_to_explore() {
        let mut app = halls();
        let (first, second) = (ChartId(1), ChartId(2));
        explorer(&mut app, Point::new(1, 1), first);
        for _ in 0..14 {
            app.update();
        }
        app.world_mut().resource_mut::<Charts>().copy(first, second);
        let late = explorer(&mut app, Point::new(19, 7), second);
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(app.world().get::<Position>(late).unwrap().0, Point::new(19, 7), "it was handed the map and stays put");
    }

    /// Two that share a chart go different ways: what one has seen the
    /// other does not walk to.
    #[test]
    fn two_minds_sharing_a_chart_finish_sooner_than_one_alone() {
        let turns_to_chart = |explorers: &[Point]| {
            let mut app = halls();
            for at in explorers {
                explorer(&mut app, *at, ChartId(3));
            }
            app.insert_resource(crate::turn::Pace::per_second(100))
                .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs(1)));
            let all = ground(&app).len();
            for _ in 0..400 {
                app.update();
                if app.world().resource::<Charts>().count(ChartId(3), HALL) >= all {
                    return app.world().resource::<crate::turn::Turns>().turn_number();
                }
            }
            panic!("the halls were never charted");
        };
        let alone = turns_to_chart(&[Point::new(1, 1)]);
        let together = turns_to_chart(&[Point::new(1, 1), Point::new(19, 7)]);
        assert!(together < alone, "two took {together} turns and one took {alone}");
    }

    #[test]
    fn a_chart_is_kept_per_map_copied_and_forgotten() {
        let (scout, party) = (ChartId(1), ChartId(2));
        let mut charts = Charts::default();
        charts.mark(scout, MapId(1), Point::new(3, 3));
        charts.mark(scout, MapId(2), Point::new(9, 9));
        charts.mark(party, MapId(1), Point::new(4, 4));
        assert!(charts.holds(scout, MapId(1), Point::new(3, 3)));
        assert!(!charts.holds(scout, MapId(2), Point::new(3, 3)), "a tile of one map is not a tile of another");
        assert!(!charts.holds(party, MapId(1), Point::new(3, 3)), "nor of another chart");

        charts.copy(scout, party);
        assert_eq!(charts.count(party, MapId(1)), 2, "what it had and what it was given");
        assert!(charts.holds(party, MapId(2), Point::new(9, 9)), "on every map");
        assert_eq!(charts.count(scout, MapId(1)), 1, "and the giver keeps its own");

        let mut back = Charts::default();
        back.import(charts.export());
        assert_eq!(back, charts, "a save holds every chart of every map");

        charts.forget(scout);
        assert_eq!(charts.count(scout, MapId(1)) + charts.count(scout, MapId(2)), 0);
        assert_eq!(charts.count(party, MapId(1)), 2, "forgetting one leaves the other");
    }
}
