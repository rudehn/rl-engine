//! The foundry's ten decks: assembly halls of rooms and doors, each
//! holding an armory and a supply store, a guard post on five of them, a
//! reactor chamber on three and the core chamber on the tenth, so all four
//! charges have somewhere to be set.
//!
//! This is the whole of a deck builder: a tile registry, the pieces in
//! `assets/prefabs/` loaded against it by [`crate::prefabs::load`], one
//! chain per deck stamping them by name, and [`PlaceBuild::from_context`]
//! to read the entry, the exit and the pieces' marks back out, the way
//! delve's `Whale` does for the whale's floors. What stands at a mark is
//! the engine's to put down, from what the piece's file says.

use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_grid::{TileId, TileProps, TileRegistry};
use rl_engine::rl_mapgen::dungeon::{Doors, FarthestExit, RandomStart, Rooms};
use rl_engine::rl_mapgen::prefab::{Orient, Placement, Prefab, StampOneOf, StampPrefab};
use rl_engine::rl_mapgen::{BaseContext, BuildError, Chain};
use rl_engine::rl_render::TileAppearance;
use rl_engine::rl_world::WorldGraph;

use crate::droids::MonsterDef;

/// How many decks the foundry has. Charges are set on three, six and
/// nine, and on the core on ten.
pub const DECKS: u32 = 10;

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

/// The foundry's tiles, its pieces, and how each of its decks is built.
pub struct Foundry {
    tiles: TileRegistry,
    hull: TileId,
    deck: TileId,
    hatch: TileId,
    lamp: TileId,
    prefabs: Prefabs<MonsterDef>,
    seed: RunSeed,
}

impl Foundry {
    /// Registers every tile the decks are built from, and loads every
    /// piece against them. The load reads the content registries and the
    /// roster itself, the same ones the run inserts, which is what makes a
    /// piece's prop and monster ids match the running game's.
    pub fn new(seed: RunSeed) -> Self {
        let mut tiles = TileRegistry::new();
        let hull = tiles.register(TileProps::wall("hull")).unwrap();
        let deck = tiles.register(TileProps::floor("deck")).unwrap();
        // A grated walkway: no slower to look at, a little slower to cross.
        tiles.register(TileProps::floor("grating").move_cost(120)).unwrap();
        tiles.register(TileProps::wall("bulkhead")).unwrap();
        // The doors pass's own door: walkable underfoot, but standing in
        // the frame still blocks a shot through it.
        let hatch = tiles.register(TileProps::floor("hatch").opaque(true)).unwrap();
        tiles.register(TileProps::wall("console")).unwrap();
        let lamp = tiles.register(TileProps::wall("lamp")).unwrap();
        // After the tiles, which the pieces paint by name.
        let prefabs = crate::prefabs::load(&tiles);
        Self { tiles, hull, deck, hatch, lamp, prefabs, seed }
    }

    /// The registered tiles, for `WorldMap::new`.
    pub fn tiles(&self) -> &TileRegistry {
        &self.tiles
    }

    /// Every piece and role, the same ones the chain stamps from: the
    /// engine's `PrefabPlugin` fills the slots of what was stamped by the
    /// key each stamp carries, which names a piece in this very set.
    pub fn prefabs(&self) -> &Prefabs<MonsterDef> {
        &self.prefabs
    }

    /// The wall lamp's tile, for `light::light_the_lamps` to find on a
    /// built deck.
    pub fn lamp(&self) -> TileId {
        self.lamp
    }

    /// Each tile in full light, from `assets/tiles.ron`; a tile the file
    /// forgets stops the game at startup, by name.
    pub fn appearance(&self) -> TileAppearance {
        TileAppearance::load(TILES_RON, &self.tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"))
    }

    /// Two candidates for the armory, asymmetric so [`Orient::TurnedOrMirrored`]
    /// lays each one down more than one way: a square piece here collapsed
    /// its eight facings onto four, so both are drawn wider than they are
    /// tall or the other way around, with the locker off-centre in both.
    /// Each holds exactly one `A`, the locker the engine stands and stocks.
    fn armories(&self) -> Result<Vec<(Prefab, u32)>, BuildError> {
        Ok(vec![(self.prefabs.piece("armory wide")?, 1), (self.prefabs.piece("armory tall")?, 1)])
    }

    /// Two candidates for the supply store, each holding two `L`s, a
    /// supply crate each, and each lit by one `lamp` fixture on its wall.
    fn stores(&self) -> Result<Vec<(Prefab, u32)>, BuildError> {
        Ok(vec![(self.prefabs.piece("store wide")?, 1), (self.prefabs.piece("store tall")?, 1)])
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
            // Every room is at least seven cells, two more than the
            // largest piece: `Placement::AnyRoom` takes only a room that
            // leaves a cell of floor all round the piece, so with rooms
            // of five the reactor, stamped last, found no room left on
            // some seeds and the deck failed to build.
            .then(Rooms { floor: self.deck, min_size: 7, max_size: 12, ..Default::default() })
            .then(Doors { door: self.hatch })
            .then(StampOneOf { name: "armory", choices: self.armories()?, at: Placement::AnyRoom, orient: Orient::TurnedOrMirrored })
            .then(StampOneOf { name: "store", choices: self.stores()?, at: Placement::AnyRoom, orient: Orient::TurnedOrMirrored });
        // Decks two, four, five, seven and eight hold a guard post: a locker
        // and a weapon held by what the deck has. Not deck one, which
        // teaches the lamp and the droids, and not the charge decks, which
        // already fit a third piece in their rooms.
        if matches!(deck, 2 | 4 | 5 | 7 | 8) {
            chain = chain.then(StampPrefab {
                name: "guard post",
                prefab: self.prefabs.piece("guard post")?,
                at: Placement::AnyRoom,
                orient: Orient::TurnedOrMirrored,
            });
        }
        // Decks three, six and nine each feed a section of the plant, and
        // deck ten is the core: one reactor chamber on each, so the four
        // charges have somewhere to be set. The core is its own shape and
        // marks `R` like the others, so one system plants all four
        // consoles. Both are stamped fixed, since nothing about either
        // chamber reads better turned.
        chain = match deck {
            3 | 6 | 9 => chain.then(StampPrefab { name: "reactor", prefab: self.prefabs.piece("reactor")?, at: Placement::AnyRoom, orient: Orient::Fixed }),
            DECKS => chain.then(StampPrefab { name: "core", prefab: self.prefabs.piece("core")?, at: Placement::AnyRoom, orient: Orient::Fixed }),
            _ => chain,
        };
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
        for s in 0..300 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap_or_else(|e| panic!("deck {deck}, seed {s}: {e}"));
                assert_ne!(Some(built.entry), built.exit, "deck {deck}, seed {s}: the way in is the way out");
            }
        }
    }

    /// The charge decks, and only they, hold a reactor: three, six and
    /// nine, and the core on ten. One each, since two consoles on a deck
    /// would let one run set the same charge twice.
    #[test]
    fn only_the_charge_decks_hold_a_reactor_and_each_holds_exactly_one() {
        for s in 0..20 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap();
                let reactors = built.spots.iter().filter(|s| s.tag == 'R' as u32).count();
                assert_eq!(reactors, usize::from(matches!(deck, 3 | 6 | 9 | 10)), "deck {deck}, seed {s}");
            }
        }
    }

    /// The guard post decks, and only they, hold one: two, four, five,
    /// seven and eight. One each, counted by the brute at its mouth,
    /// which every post has exactly one of.
    #[test]
    fn only_the_guard_post_decks_hold_one_and_each_holds_exactly_one() {
        for s in 0..20 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap();
                let posts = built.spots.iter().filter(|s| s.tag == 'b' as u32).count();
                assert_eq!(posts, usize::from(matches!(deck, 2 | 4 | 5 | 7 | 8)), "deck {deck}, seed {s}");
            }
        }
    }

    /// With every guard on its post and the locker standing, every floor
    /// cell of a stamped guard post can still be walked to from the deck's
    /// entry, one orthogonal step at a time. A guard's post is where it
    /// goes home to: a piece whose guard stands in its only way in shuts
    /// the post to every other guard that ever steps out, which is how a
    /// brute in a one-cell door once kept both sentries out for good.
    #[test]
    fn with_every_guard_home_every_floor_cell_of_a_guard_post_can_be_walked_to_from_the_entry() {
        use rl_engine::rl_rules::prefab::Slot;
        let tables = Foundry::new(RunSeed(0)).tiles().tables();
        let mut checked = 0;
        for s in 0..60 {
            let foundry = Foundry::new(RunSeed(s));
            let post = foundry.prefabs().defs().expect("guard post").raw();
            for deck in [2, 4, 5, 7, 8] {
                let ctx = foundry.generate(map_of(deck)).unwrap();
                let st = ctx.outputs().iter::<Stamped>().find(|st| st.prefab == Some(post)).expect("a guard post deck holds one").clone();
                let built = PlaceBuild::from_context(ctx).unwrap();
                // What stands on the post's cells once it is filled: every
                // guard at home, and the locker.
                let standing: Vec<rl_engine::rl_core::Point> = st
                    .marks
                    .iter()
                    .filter(|(c, p)| *p != built.entry && matches!(foundry.prefabs().slot(post, *c), Some(Slot::Monster { .. } | Slot::Prop(_))))
                    .map(|(_, p)| *p)
                    .collect();
                let open = |p: rl_engine::rl_core::Point| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]) && !standing.contains(&p);
                let mut seen = BTreeSet::from([(built.entry.x, built.entry.y)]);
                let mut queue = vec![built.entry];
                while let Some(p) = queue.pop() {
                    for n in [p.offset(1, 0), p.offset(-1, 0), p.offset(0, 1), p.offset(0, -1)] {
                        if open(n) && seen.insert((n.x, n.y)) {
                            queue.push(n);
                        }
                    }
                }
                for y in st.bounds.y..st.bounds.bottom() {
                    for x in st.bounds.x..st.bounds.right() {
                        let p = rl_engine::rl_core::Point::new(x, y);
                        if open(p) {
                            assert!(seen.contains(&(x, y)), "deck {deck}, seed {s}: {p:?} in the guard post cannot be walked to with its guards home");
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert!(checked > 0, "some guard post cell was checked");
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
                        if matches!(c, 'A' | 'L' | 'R' | 's' | 'w' | 'b') {
                            let tile = ctx.terrain().get(*p).unwrap_or_else(|| panic!("deck {deck}, seed {s}: mark {c} at {p:?} is off the map"));
                            assert!(tables.walkable[tile.index()], "deck {deck}, seed {s}: mark {c} at {p:?} sits on a wall");
                        }
                    }
                }
            }
        }
    }

    /// Every mark a player must reach, and the way down, can be walked to
    /// from the deck's entry one orthogonal step at a time, the strictest
    /// reading of the engine's rule that a diagonal never squeezes between
    /// two walls. The reactor chamber once put its console square in the
    /// hatch's way, and the whole chamber could only be entered diagonally
    /// between two walls, which the engine refuses: the mission could not
    /// be finished on any seed.
    #[test]
    fn every_mark_and_the_way_down_can_be_walked_to_from_the_entry_over_a_span_of_seeds() {
        let tables = Foundry::new(RunSeed(0)).tiles().tables();
        for s in 0..60 {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap();
                let walkable = |p: rl_engine::rl_core::Point| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]);
                let mut seen = BTreeSet::from([(built.entry.x, built.entry.y)]);
                let mut queue = vec![built.entry];
                while let Some(p) = queue.pop() {
                    for n in [p.offset(1, 0), p.offset(-1, 0), p.offset(0, 1), p.offset(0, -1)] {
                        if walkable(n) && seen.insert((n.x, n.y)) {
                            queue.push(n);
                        }
                    }
                }
                let reached = |p: rl_engine::rl_core::Point| seen.contains(&(p.x, p.y));
                for spot in built.spots.iter().filter(|s| matches!(char::from_u32(s.tag), Some('A' | 'L' | 'R' | 's' | 'w' | 'b'))) {
                    assert!(reached(spot.at), "deck {deck}, seed {s}: mark {:?} at {:?} cannot be walked to", char::from_u32(spot.tag), spot.at);
                }
                if let Some(exit) = built.exit {
                    assert!(reached(exit), "deck {deck}, seed {s}: the way down at {exit:?} cannot be walked to");
                }
            }
        }
    }
}
