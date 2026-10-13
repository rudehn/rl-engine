//! The foundry's ten decks, each grown piece by piece from a recipe in
//! `assets/layout.ron`, halls opening onto each other on the upper decks
//! and a tighter warren below, each
//! holding an armory and a supply store, a guard post on five of them, a
//! reactor chamber on three and the core chamber on the tenth, so all four
//! charges have somewhere to be set, and every other room furnished as
//! what it was for.
//!
//! This is the whole of a deck builder: a tile registry, the layout the
//! engine grows a deck to and holds it to, the pieces in
//! `assets/prefabs/` loaded against the tiles by [`crate::prefabs::load`],
//! one chain per deck stamping them by name, run again from the next
//! attempt when a deck falls short of its recipe, and [`PlaceBuild::from_context`]
//! to read the entry, the exit and the pieces' marks back out. What stands at a mark is
//! the engine's to put down, from what the piece's file says.

use rl_engine::rl_bevy::layout::LayoutTiles;
use rl_engine::rl_bevy::prelude::*;
use rl_engine::rl_core::RunSeed;
use rl_engine::rl_grid::{Light, Rgb, TileId, TileProps, TileRegistry};
use rl_engine::rl_mapgen::dungeon::{FarthestExit, RandomStart};
use rl_engine::rl_mapgen::prefab::{Orient, Placement, Prefab, StampEachRoom, StampOneOf, StampPrefab};
use rl_engine::rl_mapgen::{BaseContext, BuildError, Chain, Survey};
use rl_engine::rl_render::TileAppearance;
use rl_engine::rl_rules::layout::{self, LayoutDef};
use rl_engine::rl_world::WorldGraph;

use crate::droids::MonsterDef;

/// How many decks the foundry has. Charges are set on three, six and
/// nine, and on the core on ten.
pub const DECKS: u32 = 10;

/// How every tile looks, compiled in so the binary runs from anywhere.
const TILES_RON: &str = include_str!("../assets/tiles.ron");

/// How a deck is grown, compiled in beside the tiles.
const LAYOUT_RON: &str = include_str!("../assets/layout.ron");

/// How many times a deck is built before the run gives up on it. A deck
/// that fails this many is a recipe that asks too much, which the test
/// over a span of seeds is there to find before a player does.
const ATTEMPTS: u32 = 40;

/// The recipe a deck is grown to: wide assembly halls on the upper decks,
/// and tighter, with more ways round, from deck four down.
pub fn recipe_of(deck: u32) -> &'static str {
    if deck <= 3 { "assembly" } else { "warren" }
}

/// The deck a map id is; decks count from one.
pub fn deck_of(map: MapId) -> u32 {
    map.0
}

/// The light everywhere on `deck`, which its build carries. Deck one
/// keeps a dim working light, lit enough to be seen, so the first fight
/// is learned with the lights on; every deck below it is dark, which is
/// where a probe's radar starts to matter.
pub fn ambient_of(deck: u32) -> Light {
    match deck {
        1 => Light::new(36, Rgb::new(185, 200, 225)),
        _ => Light::DARK,
    }
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
    layout: LayoutDef,
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
        tiles.register(TileProps::wall("lamp")).unwrap();
        // Floor that says what a room was for, and crosses like any deck.
        for floor in ["hazard stripe", "oil stain", "scorch mark", "drain grate", "cable trench"] {
            tiles.register(TileProps::floor(floor)).unwrap();
        }
        // A wall lamp sheds its light by itself, as `assets/tiles.ron`
        // writes it beside its look, and the engine hangs that light on
        // every lamp of every deck built or restored.
        rl_engine::rl_render::looks::light_tiles(TILES_RON, &mut tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"));
        // After the tiles, which the pieces paint by name.
        let prefabs = crate::prefabs::load(&tiles);
        let layout = layout::load(LAYOUT_RON).unwrap_or_else(|e| panic!("assets/layout.ron: {e}"));
        Self { tiles, hull, deck, hatch, layout, prefabs, seed }
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

    /// What every other room of a deck was for: a machine shop, a charging
    /// bay, a parts store, a pump room, a control room, scrap sorting or a
    /// loading dock, each drawn small enough for the smallest room and
    /// again larger, weighted so a hall that takes the larger usually gets
    /// it, and the maintenance corner for variety. None holds anything to
    /// take or anyone to fight, so furnishing a deck changes what it looks
    /// like and where things stand, never what it is worth.
    fn rooms(&self) -> Result<Vec<(Prefab, u32)>, BuildError> {
        let mut rooms = vec![(self.prefabs.piece("maintenance closet")?, 1)];
        for name in ["machine shop", "charging bay", "parts store", "pump room", "control room", "scrap sorting", "loading dock"] {
            rooms.push((self.prefabs.piece(name)?, 2));
            let large = format!("{name} large");
            rooms.push((self.prefabs.piece(&large)?, 3));
        }
        Ok(rooms)
    }

    // ANCHOR: grow
    /// Runs the deck's chain and hands back the context still open, so a
    /// test can read a [`Stamped`](rl_engine::rl_mapgen::prefab::Stamped)
    /// pass emitted before [`PlaceBuild::from_context`] flattens every
    /// mark into a [`Spot`] and drops the stamp's own bounds.
    fn generate(&self, map: MapId) -> Result<BaseContext, BuildError> {
        let deck = deck_of(map);
        // One stream per deck, so building deck 2 first does not change deck 1.
        let seed = RunSeed(self.seed.0 ^ (deck as u64) << 32);
        // Foundry grows no locked or sealed piece, so its hatch stands for both.
        let tiles = LayoutTiles { floor: self.deck, door: self.hatch, locked: self.hatch, sealed: self.hatch };
        let grown = rl_engine::rl_bevy::layout::passes(&self.layout, recipe_of(deck), &tiles)?;
        let (width, height) = grown.size;
        let mut chain = Chain::new()
            .then(grown.accrete)
            .then(grown.loops)
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
        // The start keeps the population's own distance from every piece,
        // so no guard at a post stands beside the arrival.
        let start = RandomStart.clear_of_stamps(crate::droids::MIN_DISTANCE_FROM_ENTRY);
        // Last, once the ways in and out are known so no room piece lands
        // on either: every room nothing was stamped in is furnished.
        let rooms = StampEachRoom { name: "furnish", choices: self.rooms()?, orient: Orient::TurnedOrMirrored };
        // The survey once the ways in and out are known, and what the
        // recipe requires last: a deck that falls short is built again.
        // The phases put the survey before the furnishing, so the open
        // share it is held to is the deck's before its machinery; a room
        // piece keeps floor all round it, so the route is the same after.
        let make = || BaseContext::blank(width, height, self.tiles.clone(), self.hull);
        chain.then(start).then(FarthestExit).then(Survey).then(rooms).then(grown.require).run_retrying(make, seed, ATTEMPTS)
    }
    // ANCHOR_END: grow

    /// The deck as rows of glyphs, for `foundry --deck` and for reading a
    /// layout before calling a recipe done: `#` what cannot be walked on,
    /// `.` what can, `+` a hatch, `<` the way in and `>` the way down.
    pub fn render(&self, map: MapId) -> Result<String, BuildError> {
        use rl_engine::rl_core::Grid2D;
        let built = self.build(map, None)?;
        let tables = self.tiles.tables();
        let bounds = built.terrain.bounds();
        let mut out = String::new();
        for y in 0..bounds.height {
            for x in 0..bounds.width {
                let p = rl_engine::rl_core::Point::new(x, y);
                let tile = built.terrain.get(p).expect("in bounds");
                out.push(if p == built.entry {
                    '<'
                } else if Some(p) == built.exit {
                    '>'
                } else if tile == self.hatch {
                    '+'
                } else if tables.walkable[tile.index()] {
                    '.'
                } else {
                    '#'
                });
            }
            out.push('\n');
        }
        Ok(out)
    }
}

impl PlaceRules for Foundry {
    fn build(&self, map: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        let mut built = PlaceBuild::from_context(self.generate(map)?)?;
        built.ambient = ambient_of(deck_of(map));
        Ok(built)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    use rl_engine::rl_core::Rect;
    use rl_engine::rl_mapgen::BuildContext;
    use rl_engine::rl_mapgen::prefab::Stamped;

    use super::*;
    use crate::testing::{par_seeds, seeds};

    #[test]
    fn every_deck_builds_over_a_span_of_seeds_with_an_entry_and_an_exit() {
        par_seeds(seeds(0..300), |s| {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap_or_else(|e| panic!("deck {deck}, seed {s}: {e}"));
                assert_ne!(Some(built.entry), built.exit, "deck {deck}, seed {s}: the way in is the way out");
            }
        });
    }

    /// The charge decks, and only they, hold a reactor: three, six and
    /// nine, and the core on ten. One each, since two consoles on a deck
    /// would let one run set the same charge twice.
    #[test]
    fn only_the_charge_decks_hold_a_reactor_and_each_holds_exactly_one() {
        par_seeds(0..20, |s| {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap();
                let reactors = built.spots.iter().filter(|s| s.tag == 'R' as u32).count();
                assert_eq!(reactors, usize::from(matches!(deck, 3 | 6 | 9 | 10)), "deck {deck}, seed {s}");
            }
        });
    }

    /// The guard post decks, and only they, hold one: two, four, five,
    /// seven and eight. One each, counted by the brute at its mouth,
    /// which every post has exactly one of.
    #[test]
    fn only_the_guard_post_decks_hold_one_and_each_holds_exactly_one() {
        par_seeds(0..20, |s| {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap();
                let posts = built.spots.iter().filter(|s| s.tag == 'b' as u32).count();
                assert_eq!(posts, usize::from(matches!(deck, 2 | 4 | 5 | 7 | 8)), "deck {deck}, seed {s}");
            }
        });
    }

    /// No guard post stands a guard within the distance the deck's own
    /// population keeps from the way in, over a span of seeds: the start
    /// is drawn clear of every stamped piece. It used to be drawn from
    /// anywhere in the first room, which `Placement::AnyRoom` sometimes
    /// stamped the post into, and about one guard post deck in sixteen put
    /// a guard within two cells of the arrival.
    #[test]
    fn no_guard_stands_within_the_populations_distance_of_the_entry_over_a_span_of_seeds() {
        use rl_engine::rl_core::geometry::chebyshev;
        use rl_engine::rl_rules::prefab::Slot;
        let min = crate::droids::MIN_DISTANCE_FROM_ENTRY;
        let guards = AtomicUsize::new(0);
        par_seeds(seeds(0..100), |s| {
            let foundry = Foundry::new(RunSeed(s));
            for deck in [2, 4, 5, 7, 8] {
                let built = foundry.build(map_of(deck), None).unwrap();
                for spot in &built.spots {
                    let slot = spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, c)| foundry.prefabs().slot(key, c));
                    if matches!(slot, Some(Slot::Monster { .. })) {
                        let d = chebyshev(spot.at, built.entry);
                        assert!(d >= min, "deck {deck}, seed {s}: a guard's slot at {:?} is {d} from the entry at {:?}", spot.at, built.entry);
                        guards.fetch_add(1, Relaxed);
                    }
                }
            }
        });
        assert!(guards.into_inner() > 0, "some guard's slot was measured");
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
        let checked = AtomicUsize::new(0);
        par_seeds(seeds(0..60), |s| {
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
                            checked.fetch_add(1, Relaxed);
                        }
                    }
                }
            }
        });
        assert!(checked.into_inner() > 0, "some guard post cell was checked");
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
        par_seeds(seeds(0..200), |s| {
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
        });
    }

    /// No room on any deck is left empty: the armory, the store and the
    /// charge pieces take theirs, and every other room is furnished as what
    /// it was for. A blank room is the thing furnishing exists to stop.
    #[test]
    fn no_room_on_any_deck_is_left_empty_over_a_span_of_seeds() {
        use rl_engine::rl_mapgen::dungeon::Room;
        par_seeds(seeds(0..100), |s| {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let ctx = foundry.generate(map_of(deck)).unwrap();
                let stamps: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|st| st.bounds).collect();
                for room in ctx.outputs().iter::<Room>() {
                    assert!(stamps.iter().any(|b| room.0.intersection(b).is_some()), "deck {deck}, seed {s}: room {:?} is empty", room.0);
                }
            }
        });
    }

    /// With every prop that stands in the way and every guard at its post,
    /// every open cell of a deck can still be walked to from the entry,
    /// one orthogonal step at a time. A room piece stands machinery about
    /// a room a corridor may cross anywhere, so a piece that walled a cell
    /// in, or a corridor off, would be a deck with a place nobody reaches.
    #[test]
    fn with_everything_standing_every_open_cell_of_a_deck_can_be_walked_to_from_the_entry() {
        use rl_engine::rl_core::Grid2D;
        use rl_engine::rl_rules::prefab::Slot;
        let registries = crate::content::registries();
        let tables = Foundry::new(RunSeed(0)).tiles().tables();
        par_seeds(seeds(0..60), |s| {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let built = foundry.build(map_of(deck), None).unwrap();
                let standing: BTreeSet<(i32, i32)> = built
                    .spots
                    .iter()
                    .filter(|spot| spot.at != built.entry)
                    .filter(|spot| {
                        let slot = spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, c)| foundry.prefabs().slot(key, c));
                        match slot {
                            Some(Slot::Prop(id)) => registries.props.get(*id).blocks,
                            Some(Slot::Monster { .. }) => true,
                            _ => false,
                        }
                    })
                    .map(|spot| (spot.at.x, spot.at.y))
                    .collect();
                let open = |p: rl_engine::rl_core::Point| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]) && !standing.contains(&(p.x, p.y));
                let mut seen = BTreeSet::from([(built.entry.x, built.entry.y)]);
                let mut queue = vec![built.entry];
                while let Some(p) = queue.pop() {
                    for n in [p.offset(1, 0), p.offset(-1, 0), p.offset(0, 1), p.offset(0, -1)] {
                        if open(n) && seen.insert((n.x, n.y)) {
                            queue.push(n);
                        }
                    }
                }
                let bounds = built.terrain.bounds();
                for p in bounds.cells().filter(|p| open(*p)) {
                    assert!(seen.contains(&(p.x, p.y)), "deck {deck}, seed {s}: {p:?} cannot be walked to with everything standing");
                }
            }
        });
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
        par_seeds(seeds(0..60), |s| {
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
        });
    }

    /// Every deck is grown, surveyed and held to its recipe, and builds
    /// within the attempts a deck is given on every seed of the span.
    #[test]
    fn every_deck_meets_what_its_recipe_requires_over_a_span_of_seeds() {
        use rl_engine::rl_mapgen::{Area, Surveyed};
        par_seeds(seeds(0..150), |s| {
            let foundry = Foundry::new(RunSeed(s));
            for deck in 1..=DECKS {
                let ctx = foundry.generate(map_of(deck)).unwrap_or_else(|e| panic!("deck {deck}, seed {s}: {e}"));
                let surveyed = ctx.outputs().first::<Surveyed>().unwrap_or_else(|| panic!("deck {deck}, seed {s}: not surveyed"));
                assert!(surveyed.loops >= 2, "deck {deck}, seed {s}: {} loops", surveyed.loops);
                assert_eq!(surveyed.areas.len(), ctx.outputs().iter::<Area>().count());
            }
        });
    }

    /// Two decks of one run are not grown to one plan: over a span of
    /// seeds no two decks of a run are the same map, and the upper and the
    /// lower decks name different recipes.
    #[test]
    fn the_decks_of_a_run_differ_from_each_other() {
        par_seeds(0..20, |s| {
            let foundry = Foundry::new(RunSeed(s));
            let maps: Vec<String> = (1..=DECKS).map(|d| foundry.render(map_of(d)).unwrap()).collect();
            for i in 0..maps.len() {
                for j in i + 1..maps.len() {
                    assert_ne!(maps[i], maps[j], "seed {s}: decks {} and {} are the same map", i + 1, j + 1);
                }
            }
        });
        assert_ne!(recipe_of(1), recipe_of(5), "the upper and the lower decks name different recipes");
    }

    #[test]
    fn a_deck_is_the_size_its_recipe_says() {
        use rl_engine::rl_core::Grid2D;
        let built = Foundry::new(RunSeed(1)).build(map_of(1), None).unwrap();
        assert_eq!((built.terrain.bounds().width, built.terrain.bounds().height), (84, 48));
    }
}
