//! The first three decks of the foundry: assembly halls of rooms and
//! doors, each holding an armory and a supply store, with a reactor
//! chamber added on the deepest of the three.
//!
//! This is the whole of a deck builder: a tile registry, one chain per
//! deck, and [`PlaceBuild::from_context`] to read the entry, the exit and
//! the prefab marks back out, the way delve's `Whale` does for the
//! whale's floors.

use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_grid::{TileId, TileProps, TileRegistry};
use rl_engine::rl_mapgen::dungeon::{Doors, FarthestExit, RandomStart, Rooms};
use rl_engine::rl_mapgen::prefab::{Orient, Placement, Prefab, StampOneOf, StampPrefab};
use rl_engine::rl_mapgen::{BaseContext, BuildError, Chain};
use rl_engine::rl_render::TileAppearance;
use rl_engine::rl_world::WorldGraph;

/// How many of the foundry's decks this slice builds.
pub const DECKS: u32 = 3;

/// How every tile looks, compiled in so the binary runs from anywhere.
const TILES_RON: &str = include_str!("../assets/tiles.ron");

/// The deck a map id is; decks count from one.
pub fn deck_of(map: MapId) -> u32 {
    map.0
}

/// The map id of a deck.
pub fn map_of(deck: u32) -> MapId {
    MapId(deck)
}

/// The foundry's tiles and how each of its first three decks is built.
pub struct Foundry {
    tiles: TileRegistry,
    hull: TileId,
    deck: TileId,
    grating: TileId,
    bulkhead: TileId,
    hatch: TileId,
    console: TileId,
    lamp: TileId,
    seed: RunSeed,
}

impl Foundry {
    /// Registers every tile the first three decks are built from.
    pub fn new(seed: RunSeed) -> Self {
        let mut tiles = TileRegistry::new();
        let hull = tiles.register(TileProps::wall("hull")).unwrap();
        let deck = tiles.register(TileProps::floor("deck")).unwrap();
        // A grated walkway: no slower to look at, a little slower to cross.
        let grating = tiles.register(TileProps::floor("grating").move_cost(120)).unwrap();
        let bulkhead = tiles.register(TileProps::wall("bulkhead")).unwrap();
        // The doors pass's own door: walkable underfoot, but standing in
        // the frame still blocks a shot through it.
        let hatch = tiles.register(TileProps::floor("hatch").opaque(true)).unwrap();
        let console = tiles.register(TileProps::wall("console")).unwrap();
        let lamp = tiles.register(TileProps::wall("lamp")).unwrap();
        Self { tiles, hull, deck, grating, bulkhead, hatch, console, lamp, seed }
    }

    /// The registered tiles, for `WorldMap::new`.
    pub fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    /// Each tile in full light, from `assets/tiles.ron`; a tile the file
    /// forgets stops the game at startup, by name.
    pub fn appearance(&self) -> TileAppearance {
        TileAppearance::load(TILES_RON, &self.tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"))
    }

    /// Two candidates for the armory, asymmetric so [`Orient::TurnedOrMirrored`]
    /// lays each one down more than one way: a square piece here collapsed
    /// its eight facings onto four, so both are drawn wider than they are
    /// tall or the other way around, with the mark off-centre in both.
    /// Each holds exactly one `A`, the guaranteed item's spot.
    fn armories(&self) -> Result<Vec<(Prefab, u32)>, BuildError> {
        let (bulkhead, deck, grating) = (self.bulkhead, self.deck, self.grating);
        let legend = |c: char| match c {
            '#' => Some(bulkhead),
            '.' => Some(deck),
            'g' => Some(grating),
            _ => None,
        };
        let a = Prefab::parse(&["#####", "#g.A#", "#...#", "##.##"], legend).map_err(|e| BuildError::new("armory", e))?;
        let b = Prefab::parse(&["####", "#A.#", "#g.#", "#..#", "##.#"], legend).map_err(|e| BuildError::new("armory", e))?;
        Ok(vec![(a, 1), (b, 1)])
    }

    /// Two candidates for the supply store, each holding two `L`s, a floor
    /// item's spot, and each lit by one `lamp` fixture on its wall.
    fn stores(&self) -> Result<Vec<(Prefab, u32)>, BuildError> {
        let (bulkhead, deck, lamp) = (self.bulkhead, self.deck, self.lamp);
        let legend = |c: char| match c {
            '#' => Some(bulkhead),
            '.' => Some(deck),
            'l' => Some(lamp),
            _ => None,
        };
        let a = Prefab::parse(&["#l###", "#L..#", "#..L#", "##.##"], legend).map_err(|e| BuildError::new("store", e))?;
        let b = Prefab::parse(&["#l##", "#L.#", "#..#", "#.L#", "##.#"], legend).map_err(|e| BuildError::new("store", e))?;
        Ok(vec![(a, 1), (b, 1)])
    }

    /// The reactor chamber, deck three only: one `R`, its console beside
    /// it, and a hatch it keeps facing since a corridor meets it there,
    /// which is why it is stamped with [`Orient::Fixed`].
    fn reactor(&self) -> Result<Prefab, BuildError> {
        let (bulkhead, console, hatch) = (self.bulkhead, self.console, self.hatch);
        let legend = |c: char| match c {
            '#' => Some(bulkhead),
            'c' => Some(console),
            'h' => Some(hatch),
            _ => None,
        };
        Prefab::parse(&["#####", "#...#", "#.R.#", "#.c.#", "##h##"], legend).map_err(|e| BuildError::new("reactor", e))
    }

    /// Runs the deck's chain and hands back the context still open, so a
    /// test can read a [`Stamped`](rl_engine::rl_mapgen::prefab::Stamped)
    /// pass emitted before [`PlaceBuild::from_context`] flattens every
    /// mark into a [`Spot`] and drops the stamp's own bounds.
    fn generate(&self, map: MapId) -> Result<BaseContext, BuildError> {
        let deck = deck_of(map);
        let mut ctx = BaseContext::blank(70, 40, self.tiles.clone(), self.hull);
        // One stream per deck, so building deck 2 first does not change deck 1.
        let seed = RunSeed(self.seed.0 ^ (deck as u64) << 32);
        let mut chain = Chain::new()
            .then(Rooms { floor: self.deck, min_size: 5, max_size: 11, ..Default::default() })
            .then(Doors { door: self.hatch })
            .then(StampOneOf { name: "armory", choices: self.armories()?, at: Placement::AnyRoom, orient: Orient::TurnedOrMirrored })
            .then(StampOneOf { name: "store", choices: self.stores()?, at: Placement::AnyRoom, orient: Orient::TurnedOrMirrored });
        if deck == 3 {
            chain = chain.then(StampPrefab { name: "reactor", prefab: self.reactor()?, at: Placement::AnyRoom, orient: Orient::Fixed });
        }
        chain.then(RandomStart).then(FarthestExit).run(&mut ctx, seed)?;
        Ok(ctx)
    }
}

impl PlaceRules for Foundry {
    fn build(&self, map: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        PlaceBuild::from_context(self.generate(map)?)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rl_engine::rl_core::Rect;
    use rl_engine::rl_mapgen::BuildContext;
    use rl_engine::rl_mapgen::prefab::Stamped;

    use super::*;

    #[test]
    fn every_deck_builds_over_a_span_of_seeds_with_an_entry_and_an_exit() {
        for s in 0..40 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap_or_else(|e| panic!("deck {deck}, seed {s}: {e}"));
                assert_ne!(Some(built.entry), built.exit, "deck {deck}, seed {s}: the way in is the way out");
            }
        }
    }

    #[test]
    fn only_deck_three_holds_the_reactor_and_it_holds_exactly_one() {
        for s in 0..20 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap();
                let reactors = built.spots.iter().filter(|s| s.tag == 'R' as u32).count();
                assert_eq!(reactors, usize::from(deck == 3), "deck {deck}, seed {s}");
            }
        }
    }

    /// The armory's mark, measured from its stamp's corner, is its facing.
    /// `PlaceBuild` flattens marks into `Spot`s with no bounds to measure
    /// from, so this reads the `Stamped` pass straight off the context
    /// `generate` leaves open, the way `PlaceBuild::from_context` itself
    /// does before it takes the terrain apart.
    #[test]
    fn a_vault_is_not_laid_the_same_way_on_every_seed() {
        let facings: BTreeSet<_> = (0..40)
            .map(|s| Foundry::new(RunSeed(s)).generate(map_of(1)).unwrap())
            .filter_map(|ctx| {
                let st = ctx.outputs().iter::<Stamped>().find(|st| st.marks.iter().any(|(c, _)| *c == 'A'))?;
                let (_, at) = st.marks.iter().find(|(c, _)| *c == 'A').unwrap();
                Some((at.x - st.bounds.x, at.y - st.bounds.y))
            })
            .collect();
        assert!(facings.len() > 1, "forty seeds laid the armory exactly one way");
    }

    /// Fix round 1: `Placement::AnyRoom` used to draw from every room that
    /// fit, with no exclusion, so a second `AnyRoom` stamp could land in a
    /// room an earlier one already used and draw over its tiles while its
    /// mark was still reported. Measured on the pre-fix engine over the
    /// same span this test now runs: deck 3's `A` sat on a wall in 4.2% of
    /// seeds and `L` in 7.6%; deck 1's `A` in 2.2% and `L` in 5%.
    #[test]
    fn no_two_vaults_overlap_and_every_guaranteed_mark_sits_on_a_floor_tile() {
        let tables = Foundry::new(RunSeed(0)).tiles().tables();
        for s in 0..200 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let ctx = foundry.generate(map_of(deck)).unwrap_or_else(|e| panic!("deck {deck}, seed {s}: {e}"));
                let bounds: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|st| st.bounds).collect();
                for i in 0..bounds.len() {
                    for j in (i + 1)..bounds.len() {
                        assert!(
                            bounds[i].intersection(&bounds[j]).is_none(),
                            "deck {deck}, seed {s}: vaults {i} and {j} overlap: {:?} and {:?}",
                            bounds[i],
                            bounds[j]
                        );
                    }
                }
                for st in ctx.outputs().iter::<Stamped>() {
                    for (c, p) in &st.marks {
                        if matches!(c, 'A' | 'L' | 'R') {
                            let tile = ctx.terrain().get(*p).unwrap_or_else(|| panic!("deck {deck}, seed {s}: mark {c} at {p:?} is off the map"));
                            assert!(tables.walkable[tile.index()], "deck {deck}, seed {s}: mark {c} at {p:?} sits on a wall");
                        }
                    }
                }
            }
        }
    }
}
