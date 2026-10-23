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
use rl_engine::rl_core::{PlaceSeed, RunSeed};
use rl_engine::rl_grid::{Light, Rgb, TileId, TileProps, TileRegistry, TileTables};
use rl_engine::rl_mapgen::dungeon::{FarthestExit, RandomStart};
use rl_engine::rl_mapgen::prefab::{Orient, Placement, Prefab, StampEachRoom, StampOneOf, StampPrefab};
use rl_engine::rl_mapgen::{BaseContext, BuildError, Chain, Survey};
use rl_engine::rl_render::TileAppearance;
use rl_engine::rl_rules::layout::{self, LayoutDef};

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

/// Every piece a deck's chain stamps in a room, by name, with whether it
/// may be turned: the two armories and the two stores on every deck, the
/// guard post on two, four, five, seven and eight, a reactor on three, six
/// and nine and the core on ten, both stamped fixed, as
/// [`Foundry::generate`] lays them.
fn stamped_on(deck: u32) -> Vec<(&'static str, bool)> {
    let mut pieces = vec![("armory wide", true), ("armory tall", true), ("store wide", true), ("store tall", true)];
    if matches!(deck, 2 | 4 | 5 | 7 | 8) {
        pieces.push(("guard post", true));
    }
    match deck {
        3 | 6 | 9 => pieces.push(("reactor", false)),
        DECKS => pieces.push(("core", false)),
        _ => {}
    }
    pieces
}

/// The pieces a room nothing else took may be furnished as, by name: the
/// maintenance closet and each room, small and large, as
/// [`Foundry::rooms`](Foundry) draws them.
const FURNISHINGS: [&str; 15] = [
    "maintenance closet",
    "machine shop",
    "machine shop large",
    "charging bay",
    "charging bay large",
    "parts store",
    "parts store large",
    "pump room",
    "pump room large",
    "control room",
    "control room large",
    "scrap sorting",
    "scrap sorting large",
    "loading dock",
    "loading dock large",
];

/// Every piece some deck lays, by name: what [`stamped_on`] stamps on any
/// deck and every one of [`FURNISHINGS`]. A piece named nowhere here is one
/// no run ever lays.
#[cfg(test)]
pub(crate) fn laid_pieces() -> std::collections::BTreeSet<&'static str> {
    (1..=DECKS).flat_map(stamped_on).map(|(name, _)| name).chain(FURNISHINGS).collect()
}

/// Why some piece could never be laid where a deck asks for it, read from
/// the files with no deck grown: a piece a deck stamps that no room its
/// recipe grows holds with a cell all round it, at the largest the room
/// is drawn, in any facing the stamp may give it, so the deck fails every
/// attempt; and a furnishing no room of any deck's recipe could hold, so
/// it is never laid. [`ShapeKind::could_hold`](rl_engine::rl_rules::layout::ShapeKind::could_hold)
/// is the rule, the one `rl_mapgen` stamps by.
fn fit_problems(layout: &LayoutDef, prefabs: &Prefabs<MonsterDef>) -> Vec<String> {
    let size = |name: &str| -> Option<(i32, i32)> {
        let def = prefabs.defs().get(prefabs.defs().id(name)?);
        let rows = def.rows();
        Some((rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as i32, rows.len() as i32))
    };
    let holds = |recipe: &str, name: &str, turned: bool| {
        let Some((w, h)) = size(name) else { return false };
        let recipe = layout.recipe(recipe).expect("every deck's recipe is in the layout");
        layout.shapes_of(recipe).iter().any(|s| s.kind.could_hold(w, h) || (turned && s.kind.could_hold(h, w)))
    };
    let mut wrong = Vec::new();
    for deck in 1..=DECKS {
        for (name, turned) in stamped_on(deck) {
            if !holds(recipe_of(deck), name, turned) {
                wrong.push(format!("{name}: deck {deck} stamps it, and no room {} grows holds it with a cell all round", recipe_of(deck)));
            }
        }
    }
    for name in FURNISHINGS {
        if !(1..=DECKS).any(|deck| holds(recipe_of(deck), name, true)) {
            wrong.push(format!("{name}: no room of any deck holds it with a cell all round, so it is never laid"));
        }
    }
    wrong
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
        rl_engine::rl_rules::tiles::light_tiles(TILES_RON, &mut tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"));
        // After the tiles, which the pieces paint by name.
        let prefabs = crate::prefabs::load(&tiles);
        let layout = layout::load(LAYOUT_RON).unwrap_or_else(|e| panic!("assets/layout.ron: {e}"));
        let misfits = fit_problems(&layout, &prefabs);
        assert!(misfits.is_empty(), "assets/prefabs: {misfits:#?}");
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
        FURNISHINGS
            .iter()
            .map(|name| {
                // The closet a variety's weight, a large room the weight a
                // hall that takes it usually gives it, a small one between.
                let weight = if *name == "maintenance closet" {
                    1
                } else if name.ends_with(" large") {
                    3
                } else {
                    2
                };
                Ok((self.prefabs.piece(name)?, weight))
            })
            .collect()
    }

    // ANCHOR: grow
    /// Runs the deck's chain and hands back the context still open, so a
    /// test can read a [`Stamped`](rl_engine::rl_mapgen::prefab::Stamped)
    /// pass emitted before [`PlaceBuild::from_context`] flattens every
    /// mark into a [`Spot`] and drops the stamp's own bounds.
    ///
    /// Grown from `seed`, the deck's own, so building deck 2 first does not
    /// change deck 1.
    fn generate(&self, map: MapId, seed: PlaceSeed) -> Result<BaseContext, BuildError> {
        let deck = deck_of(map);
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
        let built = self.build(map, PlaceSeed::of(self.seed, map.0))?;
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
    fn build(&self, map: MapId, seed: PlaceSeed) -> Result<PlaceBuild, BuildError> {
        let mut built = PlaceBuild::from_context(self.generate(map, seed)?)?;
        built.ambient = ambient_of(deck_of(map));
        Ok(built)
    }

    // ANCHOR: ladder
    /// The decks, one under another, with a lift between each two: up at
    /// a deck's entry, down at the farthest exit the deck was built with,
    /// so the way on is as far from where the commando arrived as the
    /// deck allows. Deck one's way up is the lift out, which the game
    /// lays, and the core has nothing below it.
    fn tiles(&self) -> TileTables {
        self.tiles.tables()
    }

    fn ladder(&self) -> Option<Ladder> {
        let (up, down) = (StairLook::new('<', crate::lifts::LIFT, "lift up"), StairLook::new('>', crate::lifts::LIFT, "lift down"));
        Some(Ladder::new((1..=DECKS).map(map_of), up, down).top(LadderEnd::Game))
    }
    // ANCHOR_END: ladder
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rl_engine::rl_core::{Grid2D, Rect};
    use rl_engine::rl_grid::TileTables;
    use rl_engine::rl_mapgen::BuildContext;
    use rl_engine::rl_mapgen::prefab::Stamped;

    use super::*;
    use crate::testing::{par_seeds, seeds};

    /// The run's backstop: every deck of a run grown once a seed over a
    /// span of seeds, and held to every promise the generator makes of a
    /// deck with the real content in it. Each promise has a pass that owns
    /// it, tested on a made-up map in that pass's own suite; this is what
    /// catches two passes that keep their own promises and break each
    /// other's, so it grows each deck once and asks everything of it rather
    /// than growing every deck again for each question. Each check below is
    /// named for its promise, and says which deck and seed broke it.
    #[test]
    fn every_deck_of_a_run_keeps_every_promise_the_generator_makes_over_a_span_of_seeds() {
        let registries = crate::content::registries();
        let armory = crate::testing::armory(&registries);
        par_seeds(seeds(0..100), |s| {
            let foundry = Foundry::new(RunSeed(s));
            let tables = foundry.tiles().tables();
            let mut maps = Vec::new();
            for deck in 1..=DECKS {
                let at = format!("deck {deck}, seed {s}");
                let ctx = foundry.generate(map_of(deck), PlaceSeed::of(RunSeed(s), map_of(deck).0)).unwrap_or_else(|e| panic!("{at}: {e}"));
                meets_what_its_recipe_requires(&ctx, &at);
                no_two_pieces_overlap_and_every_mark_sits_on_a_floor_tile(&ctx, &tables, &at);
                no_room_is_left_empty(&ctx, &at);
                let post = foundry.prefabs().defs().expect("guard post").raw();
                let post_stamp = ctx.outputs().iter::<Stamped>().find(|st| st.prefab == Some(post)).cloned();
                let built = PlaceBuild::from_context(ctx).unwrap_or_else(|e| panic!("{at}: {e}"));
                assert_ne!(Some(built.entry), built.exit, "{at}: the way in is the way out");
                holds_its_own_reactor_and_guard_post_and_no_other(deck, &built, &at);
                no_guard_stands_near_the_way_in(&foundry, &built, &at);
                if let Some(stamp) = post_stamp {
                    every_cell_of_the_guard_post_is_walked_to_with_its_guards_home(&foundry, &stamp, &built, &tables, &at);
                }
                every_open_cell_is_walked_to_with_everything_standing(&foundry, &registries, &built, &tables, &at);
                every_mark_and_the_way_down_is_walked_to(&built, &tables, &at);
                scatters_walkable_items_beside_their_own_marks(&armory, deck, s, &built, &tables, &at);
                maps.push(built.terrain);
            }
            for i in 0..maps.len() {
                for j in i + 1..maps.len() {
                    assert!(maps[i] != maps[j], "seed {s}: decks {} and {} are the same map", i + 1, j + 1);
                }
            }
        });
        assert_ne!(recipe_of(1), recipe_of(5), "the upper and the lower decks name different recipes");
    }

    /// A deck is grown, surveyed and held to its recipe, within the
    /// attempts a deck is given.
    fn meets_what_its_recipe_requires(ctx: &BaseContext, at: &str) {
        use rl_engine::rl_mapgen::{Area, Surveyed};
        let surveyed = ctx.outputs().first::<Surveyed>().unwrap_or_else(|| panic!("{at}: not surveyed"));
        assert!(surveyed.loops >= 2, "{at}: {} loops", surveyed.loops);
        assert_eq!(surveyed.areas.len(), ctx.outputs().iter::<Area>().count(), "{at}");
    }

    /// No two stamped pieces overlap, and every mark a player or the
    /// engine fills sits on a floor tile: `Placement::AnyRoom` once drew a
    /// room an earlier stamp had used, and drew over it while its mark was
    /// still reported.
    fn no_two_pieces_overlap_and_every_mark_sits_on_a_floor_tile(ctx: &BaseContext, tables: &TileTables, at: &str) {
        let bounds: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|st| st.bounds).collect();
        for i in 0..bounds.len() {
            for j in (i + 1)..bounds.len() {
                assert!(bounds[i].intersection(&bounds[j]).is_none(), "{at}: pieces {i} and {j} overlap: {:?} and {:?}", bounds[i], bounds[j]);
            }
        }
        for st in ctx.outputs().iter::<Stamped>() {
            for (c, p) in st.marks.iter().filter(|(c, _)| matches!(c, 'A' | 'L' | 'R' | 's' | 'w' | 'b')) {
                let tile = ctx.terrain().get(*p).unwrap_or_else(|| panic!("{at}: mark {c} at {p:?} is off the map"));
                assert!(tables.walkable[tile.index()], "{at}: mark {c} at {p:?} sits on a wall");
            }
        }
    }

    /// No room is left empty: the armory, the store and the charge pieces
    /// take theirs, and every other room is furnished as what it was for.
    fn no_room_is_left_empty(ctx: &BaseContext, at: &str) {
        use rl_engine::rl_mapgen::dungeon::Room;
        let stamps: Vec<Rect> = ctx.outputs().iter::<Stamped>().map(|st| st.bounds).collect();
        for room in ctx.outputs().iter::<Room>() {
            assert!(stamps.iter().any(|b| room.0.intersection(b).is_some()), "{at}: room {:?} is empty", room.0);
        }
    }

    /// The charge decks, and only they, hold a reactor, one each, since two
    /// consoles on a deck would let one run set the same charge twice; the
    /// guard post decks, and only they, hold one post, counted by the brute
    /// at its mouth.
    fn holds_its_own_reactor_and_guard_post_and_no_other(deck: u32, built: &PlaceBuild, at: &str) {
        let count = |glyph: char| built.spots.iter().filter(|s| s.tag == glyph as u32).count();
        assert_eq!(count('R'), usize::from(matches!(deck, 3 | 6 | 9 | 10)), "{at}: reactors");
        assert_eq!(count('b'), usize::from(matches!(deck, 2 | 4 | 5 | 7 | 8)), "{at}: guard posts");
    }

    /// No guard post stands a guard within the distance the deck's own
    /// population keeps from the way in: the start is drawn clear of every
    /// stamped piece, where it once came from anywhere in the first room.
    fn no_guard_stands_near_the_way_in(foundry: &Foundry, built: &PlaceBuild, at: &str) {
        use rl_engine::rl_core::geometry::chebyshev;
        use rl_engine::rl_rules::prefab::Slot;
        for spot in &built.spots {
            let slot = spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, c)| foundry.prefabs().slot(key, c));
            if matches!(slot, Some(Slot::Monster { .. })) {
                let d = chebyshev(spot.at, built.entry);
                assert!(d >= crate::droids::MIN_DISTANCE_FROM_ENTRY, "{at}: a guard's slot at {:?} is {d} from the entry at {:?}", spot.at, built.entry);
            }
        }
    }

    /// Every cell reachable from `from` one orthogonal step at a time
    /// through cells `open` admits: the strictest reading of the engine's
    /// rule that a diagonal never squeezes between two walls.
    fn walked(from: rl_engine::rl_core::Point, open: impl Fn(rl_engine::rl_core::Point) -> bool) -> BTreeSet<(i32, i32)> {
        let mut seen = BTreeSet::from([(from.x, from.y)]);
        let mut queue = vec![from];
        while let Some(p) = queue.pop() {
            for n in [p.offset(1, 0), p.offset(-1, 0), p.offset(0, 1), p.offset(0, -1)] {
                if open(n) && seen.insert((n.x, n.y)) {
                    queue.push(n);
                }
            }
        }
        seen
    }

    /// With every guard on its post and the locker standing, every floor
    /// cell of the guard post can still be walked to from the entry: a
    /// brute in a one-cell door once kept both sentries out for good.
    fn every_cell_of_the_guard_post_is_walked_to_with_its_guards_home(foundry: &Foundry, st: &Stamped, built: &PlaceBuild, tables: &TileTables, at: &str) {
        use rl_engine::rl_rules::prefab::Slot;
        let post = st.prefab.expect("the guard post's own stamp");
        let standing: Vec<rl_engine::rl_core::Point> = st
            .marks
            .iter()
            .filter(|(c, p)| *p != built.entry && matches!(foundry.prefabs().slot(post, *c), Some(Slot::Monster { .. } | Slot::Prop(_))))
            .map(|(_, p)| *p)
            .collect();
        let open = |p: rl_engine::rl_core::Point| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]) && !standing.contains(&p);
        let seen = walked(built.entry, open);
        for p in st.bounds.cells().filter(|p| open(*p)) {
            assert!(seen.contains(&(p.x, p.y)), "{at}: {p:?} in the guard post cannot be walked to with its guards home");
        }
    }

    /// With every prop that stands in the way and every guard at its post,
    /// every open cell of a deck can still be walked to from the entry: a
    /// piece that walled a cell in, or a corridor off, is a deck with a
    /// place nobody reaches.
    fn every_open_cell_is_walked_to_with_everything_standing(foundry: &Foundry, registries: &Registries, built: &PlaceBuild, tables: &TileTables, at: &str) {
        use rl_engine::rl_core::Grid2D;
        use rl_engine::rl_rules::prefab::Slot;
        let standing: BTreeSet<(i32, i32)> = built
            .spots
            .iter()
            .filter(|spot| spot.at != built.entry)
            .filter(|spot| match spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, c)| foundry.prefabs().slot(key, c)) {
                Some(Slot::Prop(id)) => registries.props.get(*id).blocks,
                Some(Slot::Monster { .. }) => true,
                _ => false,
            })
            .map(|spot| (spot.at.x, spot.at.y))
            .collect();
        let open = |p: rl_engine::rl_core::Point| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]) && !standing.contains(&(p.x, p.y));
        let seen = walked(built.entry, open);
        if let Some(p) = built.terrain.bounds().cells().find(|p| open(*p) && !seen.contains(&(p.x, p.y))) {
            panic!("{at}: {p:?} cannot be walked to with everything standing");
        }
    }

    /// Every mark a player must reach, and the way down, can be walked to
    /// from the entry: the reactor chamber once put its console square in
    /// the hatch's way, and the mission could not be finished on any seed.
    fn every_mark_and_the_way_down_is_walked_to(built: &PlaceBuild, tables: &TileTables, at: &str) {
        let seen = walked(built.entry, |p| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]));
        let reached = |p: rl_engine::rl_core::Point| seen.contains(&(p.x, p.y));
        for spot in built.spots.iter().filter(|s| matches!(char::from_u32(s.tag), Some('A' | 'L' | 'R' | 's' | 'w' | 'b'))) {
            assert!(reached(spot.at), "{at}: mark {:?} at {:?} cannot be walked to", char::from_u32(spot.tag), spot.at);
        }
        if let Some(exit) = built.exit {
            assert!(reached(exit), "{at}: the way down at {exit:?} cannot be walked to");
        }
    }

    /// The engine's scatter on the real deck's own marks, which sit closer
    /// together than any made-up layout: every item on walkable ground off
    /// every mark, no two on one cell, and what each armory and store mark
    /// is owed laid beside it.
    fn scatters_walkable_items_beside_their_own_marks(armory: &crate::gear::Armory, deck: u32, s: u64, built: &PlaceBuild, tables: &TileTables, at: &str) {
        use rand::SeedableRng;
        use rl_engine::rl_rules::{Layout, plan_scatter};
        let rules = crate::loot::scatter_rules();
        let marks: Vec<(u32, rl_engine::rl_core::Point)> = built.spots.iter().map(|sp| (sp.tag, sp.at)).collect();
        let on_marks: Vec<rl_engine::rl_core::Point> = marks.iter().map(|(_, p)| *p).collect();
        let mut free = |p: rl_engine::rl_core::Point| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]) && !on_marks.contains(&p);
        let mut rng = rand::rngs::StdRng::seed_from_u64(s);
        let loose = rules.loose_count(deck as i32, &mut rng);
        let plan = plan_scatter(&armory.table, deck as i32, &rules, Layout { marks: &marks, bounds: built.terrain.bounds() }, loose, &mut free, &mut rng);
        let mut seen = Vec::new();
        for p in &plan {
            assert!(free(p.at), "{at}: an item at {:?} on a wall or a mark", p.at);
            assert!(!seen.contains(&p.at), "{at}: two items on {:?}", p.at);
            seen.push(p.at);
        }
        let owed: Vec<rl_engine::rl_core::Point> = rules
            .spots
            .iter()
            .flat_map(|&(tag, n)| marks.iter().filter(move |(t, _)| *t == tag).flat_map(move |(_, p)| std::iter::repeat_n(*p, n as usize)))
            .collect();
        assert!(plan.len() >= owed.len(), "{at}: {} placed, {} owed to marks", plan.len(), owed.len());
        for (placed, mark) in plan.iter().zip(&owed) {
            assert_eq!(rl_engine::rl_core::geometry::chebyshev(placed.at, *mark), 1, "{at}: an item owed to the mark at {mark:?} lies at {:?}", placed.at);
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
            .map(|s| Foundry::new(RunSeed(s)).generate(map_of(1), PlaceSeed::of(RunSeed(s), 1)).unwrap())
            .filter_map(|ctx| {
                let st = ctx.outputs().iter::<Stamped>().find(|st| st.marks.iter().any(|(c, _)| *c == 'A'))?;
                let (_, at) = st.marks.iter().find(|(c, _)| *c == 'A').unwrap();
                Some((at.x - st.bounds.x, at.y - st.bounds.y))
            })
            .collect();
        assert!(facings.len() > 1, "forty seeds laid the armory exactly one way");
    }

    #[test]
    fn a_deck_is_the_size_its_recipe_says() {
        use rl_engine::rl_core::Grid2D;
        let built = Foundry::new(RunSeed(1)).build(map_of(1), PlaceSeed::of(RunSeed(1), 1)).unwrap();
        assert_eq!((built.terrain.bounds().width, built.terrain.bounds().height), (84, 48));
    }
}
