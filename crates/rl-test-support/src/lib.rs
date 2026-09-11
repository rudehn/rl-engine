//! Fixtures and property helpers for tests in the engine and in games.
//!
//! - [`ascii_map`] turns rows of characters into a terrain, so a test can
//!   draw the situation it is about.
//! - [`render`] turns a terrain back into rows, so a failure prints as a
//!   picture.
//! - [`for_seeds`] runs a property over a seed range, which is how every
//!   generation claim in the engine is checked.
//! - [`Neighbourhood`] builds a [`Surroundings`] without hand-assembling
//!   nine region records.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use rl_core::{Direction, DirectionSet, Grid2D, Point, RunSeed};
use rl_grid::{Terrain, TileId, TileProps, TileRegistry};
use rl_world::{BandId, CellFacts, RegionFacts, SiteKindId, Surroundings};

/// The characters [`ascii_map`] understands by default.
///
/// `#` wall, `.` floor, `+` closed door, `/` open door, `~` mud (a slow
/// floor), and a space for void.
pub const STANDARD_LEGEND: &[(char, &str)] = &[(' ', "void"), ('#', "wall"), ('.', "floor"), ('+', "door_closed"), ('/', "door_open"), ('~', "mud")];

/// The standard registry plus a slow `mud` floor, matching [`STANDARD_LEGEND`].
pub fn standard_registry() -> TileRegistry {
    let mut r = TileRegistry::standard();
    r.register(TileProps::floor("mud").move_cost(300)).expect("fresh name");
    r
}

/// A terrain drawn as rows of characters, using [`STANDARD_LEGEND`].
///
/// # Panics
/// Panics on a ragged map or an unknown character.
pub fn ascii_map(rows: &[&str]) -> (Terrain, TileRegistry) {
    let registry = standard_registry();
    let terrain = ascii_map_with(rows, &registry, STANDARD_LEGEND);
    (terrain, registry)
}

/// A terrain drawn as rows of characters with a custom legend.
///
/// # Panics
/// Panics on a ragged map, an unknown character, or a legend name that is
/// not in `registry`.
pub fn ascii_map_with(rows: &[&str], registry: &TileRegistry, legend: &[(char, &str)]) -> Terrain {
    let height = rows.len() as i32;
    let width = rows.first().map(|r| r.chars().count()).unwrap_or(0) as i32;
    let lookup = |c: char| -> TileId {
        let name = legend.iter().find(|(k, _)| *k == c).map(|(_, n)| *n).unwrap_or_else(|| panic!("no legend entry for {c:?}"));
        registry.expect(name)
    };
    let grid: Vec<Vec<char>> = rows.iter().map(|r| r.chars().collect()).collect();
    for (y, row) in grid.iter().enumerate() {
        assert_eq!(row.len() as i32, width, "row {y} has a different width");
    }
    Terrain::from_fn(width, height, |p| lookup(grid[p.y as usize][p.x as usize]))
}

/// The terrain as rows of characters, using `legend` and `?` for anything
/// the legend does not name.
pub fn render(terrain: &Terrain, registry: &TileRegistry, legend: &[(char, &str)]) -> String {
    let mut out = String::new();
    for y in 0..terrain.height() {
        for x in 0..terrain.width() {
            let id = terrain.get(Point::new(x, y)).expect("in bounds");
            let name = &registry.get(id).name;
            out.push(legend.iter().find(|(_, n)| n == name).map(|(c, _)| *c).unwrap_or('?'));
        }
        out.push('\n');
    }
    out
}

/// Runs `check` once per seed in `seeds`, naming the seed on a panic.
pub fn for_seeds(seeds: std::ops::Range<u64>, mut check: impl FnMut(RunSeed)) {
    for seed in seeds {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(RunSeed(seed))));
        if let Err(payload) = result {
            let message = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "non-string panic".to_string());
            panic!("seed {seed}: {message}");
        }
    }
}

/// Builds a [`Surroundings`] one neighbour at a time.
#[derive(Debug, Clone)]
pub struct Neighbourhood {
    here: RegionFacts,
    neighbours: [Option<RegionFacts>; 8],
}

impl Neighbourhood {
    /// A region of `band` at `(1, 1)` surrounded by regions of the same
    /// band, all with [`CellFacts::plain`].
    pub fn new(band: BandId) -> Self {
        let here = Self::region(Point::new(1, 1), band);
        let mut neighbours = [None; 8];
        for d in Direction::ALL {
            neighbours[d.index()] = Some(Self::region(here.region + d.offset(), band));
        }
        Self { here, neighbours }
    }

    /// A plain region record.
    pub fn region(at: Point, band: BandId) -> RegionFacts {
        RegionFacts { region: at, band, facts: CellFacts::plain(), site: None, roads: DirectionSet::NONE, rivers: DirectionSet::NONE, river_downstream: None }
    }

    /// Places the neighbourhood at `at`.
    pub fn at(mut self, at: Point) -> Self {
        self.here.region = at;
        for d in Direction::ALL {
            if let Some(n) = &mut self.neighbours[d.index()] {
                n.region = at + d.offset();
            }
        }
        self
    }

    /// Replaces the neighbour in `d`, or removes it with `None` for a
    /// world edge.
    pub fn with_neighbour(mut self, d: Direction, facts: Option<RegionFacts>) -> Self {
        self.neighbours[d.index()] = facts;
        self
    }

    /// Sets the neighbour in `d` to `band` with plain facts.
    pub fn band_toward(mut self, d: Direction, band: BandId) -> Self {
        let at = self.here.region + d.offset();
        self.neighbours[d.index()] = Some(Self::region(at, band));
        self
    }

    /// Puts a site of `kind` here.
    pub fn with_site(mut self, kind: SiteKindId) -> Self {
        self.here.site = Some(kind);
        self
    }

    /// Runs a road out of here in `d`, and into the neighbour from the
    /// other side, so the seam holds.
    pub fn with_road(mut self, d: Direction) -> Self {
        self.here.roads.insert(d);
        if let Some(n) = &mut self.neighbours[d.index()] {
            n.roads.insert(d.opposite());
        }
        self
    }

    /// Runs a river in from `from` and out toward `to`, of `width`.
    pub fn with_river(mut self, from: Direction, to: Direction, width: u8) -> Self {
        self.here.rivers.insert(from);
        self.here.rivers.insert(to);
        self.here.river_downstream = Some(to);
        self.here.facts.river_width = width;
        self
    }

    /// Edits the facts of the region itself.
    pub fn with_facts(mut self, facts: CellFacts) -> Self {
        self.here.facts = facts;
        self
    }

    /// The finished neighbourhood.
    pub fn build(self) -> Surroundings {
        let neighbours = self.neighbours;
        Surroundings::new(self.here, |p| neighbours.iter().flatten().find(|n| n.region == p).copied())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_maps_round_trip() {
        let rows = ["#####", "#.~+#", "#####"];
        let (t, r) = ascii_map(&rows);
        assert_eq!(t.get(Point::new(2, 1)), r.id("mud"));
        assert_eq!(render(&t, &r, STANDARD_LEGEND), "#####\n#.~+#\n#####\n");
    }

    #[test]
    #[should_panic(expected = "seed 3")]
    fn for_seeds_names_the_failing_seed() {
        for_seeds(0..5, |s| assert!(s.0 != 3, "three is out"));
    }

    #[test]
    fn a_neighbourhood_agrees_across_its_seams() {
        let s = Neighbourhood::new(BandId(2))
            .at(Point::new(5, 5))
            .with_road(Direction::North)
            .with_river(Direction::West, Direction::East, 2)
            .band_toward(Direction::South, BandId(0))
            .with_neighbour(Direction::East, None)
            .with_site(SiteKindId(1))
            .build();
        assert_eq!(s.region(), Point::new(5, 5));
        assert!(s.neighbour(Direction::North).unwrap().roads.contains(Direction::South));
        assert_eq!(s.neighbour(Direction::South).unwrap().band, BandId(0));
        assert!(s.neighbour(Direction::East).is_none());
        assert_eq!(s.here.river_downstream, Some(Direction::East));
        assert_eq!(s.here.facts.river_width, 2);
        assert_eq!(s.here.site, Some(SiteKindId(1)));
        assert_eq!(s.directions_where(|n| n.band == BandId(0)).len(), 1);
    }
}
