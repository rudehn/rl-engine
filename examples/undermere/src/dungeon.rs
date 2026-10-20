//! Every place a run walks through: Mereholt, the town on the black lake,
//! and the ten floors under it, each built the first time it is entered
//! and kept whole after that, as the engine keeps any place.
//!
//! The town is one hand-drawn piece, `assets/prefabs/mereholt.ron`,
//! stamped over a blank map of its own size, so the piece is the whole
//! town and every run comes to the same one. A floor is grown, as
//! Foundry's decks are, by the engine's accretion from a recipe of
//! `assets/layout.ron`: its biome, drawn by `biomes`, names the recipes
//! it may be grown by, and the floor draws one. Rooms and halls behind
//! doors, passages one or two wide, caves, and districts that mix them are
//! all shapes in that file, so the Cellars' crawlways and the Old Mines'
//! galleries differ by data rather than by code. Each floor is surveyed
//! and held to what its recipe requires, and grown again, up to forty
//! attempts, when it falls short.
//!
//! The boss's hall and a mini-boss's lair are prefabs grown as areas of
//! their own, exactly as the recipe's own shapes are, through the one
//! entry each prefab marks; the throne's floor holds its way in at least
//! half the floor's longer side from it, since nothing else draws it that
//! far. A few further passes are Undermere's own, each small and each
//! here because the engine's leave something a floor needs: an area
//! floored again by its shape and district, furniture that never shuts a
//! way, and a slot that stands on the floor drawn round it. Everything
//! that decides what a place is made of is in this module and nowhere
//! else, so a later part's generation changes this file and nothing that
//! reads a built place.
//!
//! [`PlaceBuild::from_context`] reads the way in, the way on and the
//! pieces' slots back out, the way Foundry's decks are read. What stands
//! at a slot is the engine's to put down; the ways through are
//! [`stairs`](crate::stairs)'s and the light is [`light`](crate::light)'s.

use std::collections::{BTreeMap, BTreeSet};

use rand::Rng;
use rand::seq::SliceRandom;
use rl_engine::prelude::*;
use rl_engine::rl_bevy::layout::LayoutTiles;
use rl_engine::rl_core::Rect;
use rl_engine::rl_mapgen::dungeon::{ExitPoint, FarthestExit, MarkedStart, RandomStart, Stairs};
use rl_engine::rl_mapgen::passes::StartPoint;
use rl_engine::rl_mapgen::{Area, Opening};
use rl_engine::rl_render::TileAppearance;
use rl_engine::rl_rules::dressing;
use rl_engine::rl_rules::layout::{self, LayoutDef};
use rl_engine::rl_rules::pool::{Pick, Place, Pool, Prefer};

use crate::bestiary::MonsterDef;

mod biomes;
mod lairs;
mod pieces;
mod strata;

#[cfg(test)]
pub(crate) use biomes::biomes_told_of;
pub(crate) use biomes::{BiomeDraw, Decor, biomes_drawn};
use lairs::ClearOfTheThrone;
pub(crate) use lairs::{FINAL_BOSSES, MINI_BOSSES, final_boss, lairs};
use pieces::{PieceLook, QuietWayIn};
pub use strata::{STRATA, Stratum, crossed_into, depth_line, name_the_stratum, stratum_of};

/// How many floors the Undermere goes down. The tenth is the throne's, and
/// has no way down.
pub const FLOORS: u32 = 10;

/// The town's map id: past every floor, so a floor's number reads straight
/// off its map and nothing that counts floors ever counts the town.
pub const TOWN: MapId = MapId(100);

/// How near the way in nothing that fights may stand.
///
/// The distance a floor's population is to keep from the way in when it is
/// put down; and the way in is drawn at least this far from every piece
/// stamped before it, so no monster a piece stands at its post waits
/// beside the arrival either.
pub const MIN_DISTANCE_FROM_ENTRY: i32 = 6;

/// How many times a floor is grown before the run gives up on it. A floor
/// that fails this many is a recipe that asks too much, which the test
/// over a span of seeds is there to find before a player does.
const ATTEMPTS: u32 = 40;

/// The way down is drawn among the cells whose walk from the way in is
/// at least this much less than a hundred, in a hundred, of the longest:
/// the farthest third, so the stairs are not always in the one corner a
/// player learns to make for.
const EXIT_AMONG_PCT: u32 = 33;

/// How far the way down keeps from every piece stamped before it, so it
/// is never inside a lair nor in its doorway.
const EXIT_CLEAR: i32 = 2;

/// How many chests every floor but the throne's is furnished with, the
/// fewest and the most: one or two, each what `assets/props.ron` stocks a
/// chest with.
pub(crate) const CHESTS: (u32, u32) = (1, 2);

/// How every tile looks, compiled in so the binary runs from anywhere.
const TILES_RON: &str = include_str!("../assets/tiles.ron");

/// How every floor is grown, compiled in beside the tiles.
const LAYOUT_RON: &str = include_str!("../assets/layout.ron");

/// The layout every floor is grown from; a mistake in the file stops the
/// game at startup, every one of them named.
pub(crate) fn layout() -> LayoutDef {
    layout::load(LAYOUT_RON).unwrap_or_else(|e| panic!("assets/layout.ron: {e}"))
}

/// The tag a place grown as `drawn` carries in the pool's `where` and
/// `not`: the biome's bare name, as `assets/biomes.ron` and
/// `assets/pool.ron` both write it.
fn place_tag(drawn: &BiomeDraw) -> String {
    drawn.name.trim_start_matches("the ").to_string()
}

/// The map id of floor `floor`; floors count from one.
pub fn map_of(floor: u32) -> MapId {
    MapId(floor)
}

/// The floor a map id is, or `None` for the town, which is no floor, and
/// for any id no place of the Undermere has.
pub fn floor_of(map: MapId) -> Option<u32> {
    (1..=FLOORS).contains(&map.0).then_some(map.0)
}

/// Every tile floor `floor` may show beyond the drawn biome's own wall,
/// floor, paint, lakes and decor: the doors every grown floor but the
/// throne's hangs, every flavor dressing of `assets/pool.ron`'s own "any
/// biome" rows, and a mini-boss's lair tiles wherever its own range
/// reaches. By floor rather than by biome, since a lair's range is floors:
/// the Widow's cave and moss, Grubnak's hall and the Choirmaster's crypt
/// nave stand on floors 5 and 6, the Drowned Abbess's flooded chapel on 3
/// and 4 and the Forgemaster's forge on 7 to 9. The throne's own floor is read off
/// [`Undermere::final_boss`] rather than this, so it carries none of its
/// own wall and floor.
#[cfg(test)]
fn shared_tiles(floor: u32) -> &'static [&'static str] {
    // "hall wall" and "hall floor" are what every piece is drawn in
    // before `PieceLook` and `Underfoot` lay the floor's own over it;
    // "web" (Web-Choked Corners, Silk Curtains), "rubble" (Collapsed
    // Ceiling), "bones" (Bone Scatter) and "flooded floor" and "moss"
    // (Seep) are every flavor dressing of `assets/pool.ron`'s own "any
    // biome" rows; "blood" is `decor::BloodTrail`'s, the same "any floor"
    // reach. Every floor but the throne's carries all six.
    match floor {
        ..=2 => &["door", "open door", "rubble", "blood", "hall wall", "hall floor", "web", "bones", "flooded floor", "moss"],
        // The Drowned Abbess's chapel stands on 3 and 4 in her cathedral's
        // wall, and `Fallen Bell`, the cathedral's own fixed piece, in flagstones.
        3 | 4 => &["door", "open door", "cathedral wall", "flagstones", "web", "hall wall", "hall floor", "rubble", "blood", "bones", "flooded floor", "moss"],
        // Grubnak's hall, the Widow's cave and moss and the Choirmaster's
        // crypt nave, the beneath-the-kingdom lairs.
        5 | 6 => &[
            "bones",
            "door",
            "open door",
            "goblin hall wall",
            "goblin hall floor",
            "cave wall",
            "moss",
            "web",
            "flooded floor",
            "crypt wall",
            "crypt floor",
            "hall wall",
            "hall floor",
            "rubble",
            "blood",
        ],
        7 | 8 => &[
            "door",
            "open door",
            "flooded floor",
            "deep water",
            "forge wall",
            "ash",
            "lava",
            "hall wall",
            "hall floor",
            "web",
            "rubble",
            "blood",
            "bones",
            "moss",
        ],
        9 => &["door", "open door", "lava", "forge wall", "ash", "hall wall", "hall floor", "web", "rubble", "blood", "bones", "flooded floor", "moss"],
        // Whichever of the three `FINAL_BOSSES` floor ten draws paints its
        // own hall beyond its wall and its floor: the Drowned King's dais
        // and black pool, the Mother Below's glowcaps and bones, the Ember
        // Wyrm's lava.
        _ => &[
            "dais",
            "black pool",
            "cave wall",
            "moss",
            "glowcaps",
            "bones",
            "forge wall",
            "ash",
            "lava",
            "hall wall",
            "hall floor",
            "web",
            "rubble",
            "flooded floor",
        ],
    }
}

impl Undermere {
    /// Every risk-and-reward and reward piece the pool lays, by name: the
    /// pieces whose marks `crate::vaults` fills.
    #[cfg(test)]
    pub(crate) fn vault_pieces(&self) -> Vec<String> {
        let kinds: Vec<usize> = ["risk_reward", "reward"].iter().filter_map(|k| self.pool.kinds.iter().position(|n| n == k)).collect();
        self.pool.entries.iter().filter(|e| kinds.contains(&e.kind)).map(|e| e.name.clone()).collect()
    }

    /// Every piece and every dressing a run may lay, by name: the town, the
    /// three thrones and the five lairs, what the lived-in dungeon and the
    /// chests lay on every floor, every biome's decor and dressings, and
    /// every pool entry in play. A file named nowhere here is one no run
    /// ever lays, and what it names is reached by nothing through it.
    #[cfg(test)]
    pub(crate) fn laid_files(&self) -> (BTreeSet<String>, BTreeSet<String>) {
        let mut pieces: BTreeSet<String> = ["Mereholt", "fallen body", "wall inscription", "chest"].map(String::from).into();
        pieces.extend(FINAL_BOSSES.iter().map(|b| b.piece.to_string()).chain(MINI_BOSSES.iter().map(|b| b.name.to_string())));
        let mut dressings = BTreeSet::new();
        for (_, def) in self.defs.iter() {
            pieces.extend(def.decor.iter().filter_map(|d| match d {
                biomes::DecorDef::Furnish { piece, .. } => Some(piece.clone()),
                biomes::DecorDef::Patches { .. } => None,
            }));
            dressings.extend(def.dressings.iter().cloned());
        }
        for entry in self.pool.entries.iter().filter(|e| e.weight > 0) {
            match &entry.placed {
                rl_engine::rl_rules::pool::Placed::Dressed { dressing } => dressings.insert(dressing.clone()),
                _ => pieces.insert(entry.piece().expect("a piece for any but a dressing").to_string()),
            };
        }
        (pieces, dressings)
    }

    /// Every biome drawn with each of its recipes, every way a floor may
    /// be grown, for a test that reads what a biome lays without drawing
    /// seeds until each turns up.
    #[cfg(test)]
    pub(crate) fn every_draw(&self) -> Vec<BiomeDraw> {
        self.defs.iter().flat_map(|(_, def)| (0..def.recipes.len()).map(move |r| biomes::to_draw(def, r, &self.tiles))).collect()
    }

    /// Every tile floor `floor` grown as `drawn` may show, by name: its
    /// wall and floor, floor ten's the drawn boss's own; what its biome
    /// paints and strews, its lakes, their shore and their bridges; the
    /// tiles [`shared_tiles`] gives the floor; a risk-and-reward or reward
    /// piece's own on any floor above the throne's; and the gate's other
    /// side on floor one.
    #[cfg(test)]
    pub(crate) fn tiles_a_floor_may_show(&self, floor: u32, drawn: &BiomeDraw) -> BTreeSet<String> {
        let name = |t: TileId| self.tiles.get(t).name.clone();
        let (wall, floor_tile) = if floor == FLOORS {
            let boss = self.final_boss();
            (boss.wall.to_string(), boss.floor.to_string())
        } else {
            (name(drawn.wall), name(drawn.floor))
        };
        let strewn = drawn.decor.iter().filter_map(|d| match d {
            Decor::Patches { tile, .. } => Some(*tile),
            Decor::Furnish { .. } => None,
        });
        let painted = drawn.paint.iter().map(|(_, t)| *t).chain(strewn).map(name);
        let lakes = drawn.lakes.iter().flat_map(|(l, b)| [Some(l.tile), l.wreath.map(|(t, _)| t), b.map(|b| b.tile)]).flatten().map(name);
        let pieces = (floor < FLOORS).then_some(PIECE_TILES).into_iter().flatten().map(|t| t.to_string());
        let shared = shared_tiles(floor).iter().map(|t| t.to_string());
        let gate = (floor == 1).then(|| "gate".to_string());
        shared.chain([wall, floor_tile]).chain(painted).chain(lakes).chain(pieces).chain(gate).collect()
    }
}

/// The first seed whose run draws the pool entry `name` for a floor, and
/// that floor: for a test of what a piece does where a run lays it. Panics
/// when no run over 400 seeds draws it, so a renamed piece fails the test
/// rather than leaving it to pass over nothing.
#[cfg(test)]
pub(crate) fn drawn(name: &str) -> (RunSeed, u32) {
    let loaded = Undermere::new(RunSeed(0));
    for s in 0..400 {
        let undermere = loaded.reseeded(RunSeed(s));
        if let Some(floor) = (1..FLOORS).find(|f| undermere.plan_of(*f).iter().any(|p| undermere.pool.entries[p.entry].name == name)) {
            return (RunSeed(s), floor);
        }
    }
    panic!("no run over 400 seeds draws {name}")
}

/// What a risk-and-reward or reward piece draws in a tile of its own, on
/// any floor it may stand on: a vault's door and gate, open and shut, the
/// straw, the planks and the hole they leave, a causeway and the deep water
/// round it, fallen rock, and the strewn floors a piece is dressed with.
#[cfg(test)]
const PIECE_TILES: [&str; 16] = [
    "vault door",
    "open vault door",
    "carved door",
    "open carved door",
    "stone gate",
    "raised gate",
    "straw",
    "rotten planks",
    "hole",
    "sunken causeway",
    "deep water",
    "fallen rock",
    "flooded floor",
    "web",
    "rubble",
    "bones",
];

/// Every tile a place is built of, the town's first, then the doors, then
/// each band's in order. What a tile looks like, and what light it sheds,
/// is `assets/tiles.ron`'s; what it is to a body is here.
///
/// Wading costs twice a step, whether the water is the town's shallows,
/// the cathedral's flooded floor or the black pools before the throne.
/// Deep water, the lake and lava stop a body and nothing else: they are
/// seen across and shot over.
pub(crate) fn registry() -> TileRegistry {
    let wading = |name: &str| TileProps::floor(name).move_cost(200);
    let mut tiles = TileRegistry::new();
    tiles
        .register_all([
            // Mereholt.
            TileProps::floor("grass"),
            TileProps::floor("cobbles"),
            TileProps::floor("road"),
            TileProps::floor("planks"),
            TileProps::floor("garden"),
            TileProps::wall("timber wall"),
            TileProps::wall("stone wall"),
            // Glass: seen and lit through, shot and walked into no more than
            // a wall is.
            TileProps::named("lit window").blocks_projectiles(true),
            TileProps::wall("tree"),
            wading("shallows"),
            TileProps::named("lake"),
            TileProps::wall("standing stone"),
            TileProps::floor("gate"),
            // A shut door stops sight and a shot; anyone with hands opens
            // it by walking into it, and it shuts again behind them.
            TileProps::named("door").passable(true).opaque(true).blocks_projectiles(true).opens_to("open door"),
            TileProps::floor("open door").closes_to("door"),
            // What a lake of water and a lake of lava are crossed by.
            TileProps::floor("bridge"),
            TileProps::floor("iron bridge"),
            // The Upper Halls.
            TileProps::wall("hall wall"),
            TileProps::floor("hall floor"),
            TileProps::floor("rubble"),
            // `decor::BloodTrail`'s own, on any floor its chance lands on.
            TileProps::floor("blood"),
            // The Cellars, floors one and two's other biome.
            TileProps::wall("cellar wall"),
            TileProps::floor("cellar floor"),
            // Bare ground: the cellars' root caves and the mines' caves.
            TileProps::floor("earth"),
            // The Watch Barracks, floors one and two's third biome.
            TileProps::wall("barracks wall"),
            TileProps::floor("barracks floor"),
            // The Fungal Deeps, floors five and six's third biome.
            TileProps::wall("cave wall"),
            TileProps::floor("moss"),
            TileProps::floor("glowcaps"),
            // The Spider Warrens, floors five and six's fourth biome.
            TileProps::wall("warren wall"),
            TileProps::floor("warren floor"),
            // The Old Mines, floors five and six's second biome.
            TileProps::wall("mine wall"),
            TileProps::floor("mine floor"),
            // The Catacombs, floors five and six's first biome.
            TileProps::wall("crypt wall"),
            TileProps::floor("crypt floor"),
            TileProps::floor("bones"),
            // The Sunken Library, floors three and four.
            TileProps::wall("library wall"),
            TileProps::floor("library floor"),
            // The Drowned Cathedral, floors three and four's other biome.
            TileProps::wall("cathedral wall"),
            TileProps::floor("marble"),
            wading("flooded floor"),
            TileProps::named("deep water"),
            TileProps::floor("moonlit marble"),
            // The Underlake, floors seven and eight, alone for now.
            TileProps::wall("underlake wall"),
            TileProps::floor("underlake floor"),
            // Shallow water lit from below, the Underlake's own light.
            wading("glowing shallows"),
            // The Forge of Embers.
            TileProps::wall("forge wall"),
            TileProps::floor("ash"),
            TileProps::named("lava"),
            // A smithy's floor among the ash.
            TileProps::floor("flagstones"),
            // The Ashen Galleries, floor nine's other biome.
            TileProps::wall("cinder wall"),
            TileProps::floor("cinder floor"),
            // The Throne.
            TileProps::wall("throne wall"),
            TileProps::floor("throne floor"),
            TileProps::floor("dais"),
            wading("black pool"),
            // The mini-bosses' lairs: Grubnak's hall, since no band his own
            // floors fall in is one, and the Widow's web, slow to cross
            // and gone three turns after it catches, burnt back to the
            // moss of the den it is strung across.
            TileProps::wall("goblin hall wall"),
            TileProps::floor("goblin hall floor"),
            TileProps::floor("web").move_cost(300).burns(100, 3, "moss"),
            // The risk-and-reward pieces' own, on any floor they stand on.
            // A vault's door opens only for its own key and a sealed
            // vault's gate for nobody until its lever is pulled; both are
            // passable, so the engine's own walk with every lock opened
            // goes through them, and a lock is what refuses a body.
            TileProps::named("vault door").passable(true).opaque(true).blocks_projectiles(true).opens_to("open vault door"),
            TileProps::floor("open vault door").closes_to("vault door"),
            // Any other piece's door is carved, a door unlike the floor's
            // others, so the piece is seen before it is understood.
            TileProps::named("carved door").passable(true).opaque(true).blocks_projectiles(true).opens_to("open carved door"),
            TileProps::floor("open carved door").closes_to("carved door"),
            TileProps::named("stone gate").passable(true).opaque(true).blocks_projectiles(true),
            TileProps::floor("raised gate"),
            // Dry straw catches from anything burning and burns to rubble.
            TileProps::floor("straw").burns(100, 4, "rubble"),
            // Planks over a drop, and the drop once they give way: seen
            // across, crossed by nobody.
            TileProps::floor("rotten planks"),
            TileProps::named("hole"),
            // A drowned reliquary's causeway, hidden as deep water until
            // it is spotted, and waded once it is.
            wading("sunken causeway"),
            // A collapsing gallery's way back, fallen in.
            TileProps::wall("fallen rock"),
        ])
        .expect("every tile has a name of its own");
    // What glows by itself, the town's lit windows and the gate, the
    // glowcaps, the shafts of light on the cathedral's marble, the
    // Underlake's shallows and lava, says so beside its look, and the
    // engine hangs that light on every cell of it.
    rl_engine::rl_render::looks::light_tiles(TILES_RON, &mut tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"));
    tiles
}

/// Undermere's tiles, its pieces, how each of its places is built, and
/// every floor's own biome, drawn once here and handed out by
/// `biome_of`.
#[derive(Clone)]
pub struct Undermere {
    tiles: TileRegistry,
    prefabs: Prefabs<MonsterDef>,
    seed: RunSeed,
    layout: LayoutDef,
    defs: Registry<biomes::BiomeDef>,
    biomes: BTreeMap<u32, BiomeDraw>,
    /// Every district's weighted list of dressings, built once from each
    /// biome's own `dressings` and checked against `prefabs.dressings()`
    /// and `layout` here rather than on the floor that first grows it.
    districts: Vec<dressing::DistrictDressings>,
    /// The run's pool, `assets/pool.ron`, checked against `prefabs` and
    /// `layout` at load.
    pool: Pool,
    /// The whole run's picks, one list per floor, drawn once by
    /// [`Pool::plan_run`] in [`Undermere::new`] from a stream of the
    /// seed kept beside the lair draw, so a floor's picks never depend
    /// on which floor is built first.
    plan: BTreeMap<u32, Vec<Pick>>,
    /// What a floor hides, `assets/hidden.ron`.
    hiding: crate::traps::Hiding,
}

impl Undermere {
    /// Registers every tile a place is built of, loads every piece and the
    /// layout against them, and draws every floor's biome and recipe. The
    /// load reads the content registries and the bestiary itself, the same
    /// ones the run inserts, which is what makes a piece's prop and
    /// monster ids match the running game's.
    pub fn new(seed: RunSeed) -> Self {
        let tiles = registry();
        let layout = layout();
        // After the tiles, which the pieces paint by name, and the
        // layout, which the pool's grown and inside picks are checked
        // against.
        let (prefabs, pool, bestiary) = crate::prefabs::load(&tiles, &layout);
        let defs = biomes::load(&tiles, &layout);
        biomes::check_favored(&defs, &bestiary);
        biomes::check_pieces(&defs, &prefabs);
        biomes::check_pool(&defs, &pool, &layout, prefabs.defs());
        let districts = biomes::district_dressings(&defs, &layout);
        dressing::check_districts(&districts, prefabs.dressings(), &layout).unwrap_or_else(|e| panic!("assets/biomes.ron: {e}"));
        let hiding = crate::traps::Hiding::load();
        let mut undermere = Self { tiles, prefabs, seed, layout, defs, biomes: BTreeMap::new(), districts, pool, plan: BTreeMap::new(), hiding };
        undermere.draw(seed);
        undermere
    }

    /// The same content drawn for `seed`: what `Undermere::new(seed)`
    /// comes to, without reading and checking every file again, which is
    /// most of what `new` costs. For a test over hundreds of runs' draws.
    #[cfg(test)]
    pub(crate) fn reseeded(&self, seed: RunSeed) -> Self {
        let mut again = self.clone();
        again.draw(seed);
        again
    }

    /// Draws everything `seed` decides before a floor is built: every
    /// floor's biome, the throne's by the boss the seed draws, and the
    /// whole run's picks.
    fn draw(&mut self, seed: RunSeed) {
        let boss = final_boss(seed);
        let mut biomes = BTreeMap::new();
        for floor in 1..FLOORS {
            biomes.insert(floor, biomes::draw(&self.defs, floor, seed, &self.tiles));
        }
        biomes.insert(FLOORS, biomes::throne(&self.defs, boss.biome, seed, &self.tiles));
        // The whole run's places, in floor order, each tagged with the
        // bare name of the biome it draws, so the pool's `where` and
        // `not` read the same name `assets/biomes.ron` and
        // `assets/pool.ron` both use; drawn at once, from its own
        // stream, kept beside the lair and the boss draws above, so a
        // floor's picks never depend on which floor is built first.
        let places: Vec<Place> = (1..=FLOORS)
            .map(|floor| {
                let drawn = biomes.get(&floor).expect("every floor drew a biome above");
                Place { band: floor as i32, tags: vec![place_tag(drawn)] }
            })
            .collect();
        let mut rng = seed.rng(SeedDomain::new(b"undermere.pool"), 0);
        self.plan = (1..=FLOORS).zip(self.pool.plan_run(&mut rng, &places)).collect();
        self.biomes = biomes;
        self.seed = seed;
    }

    /// The picks `floor` was drawn, in [`Undermere::new`]: pure over
    /// `self.seed`, so every call agrees without the plan being kept
    /// anywhere but here.
    fn plan_of(&self, floor: u32) -> &[Pick] {
        self.plan.get(&floor).unwrap_or_else(|| panic!("floor {floor} outside 1..={FLOORS}"))
    }

    /// The run's picks for `floor` that may stand in `drawn`: every one of
    /// them for the biome the run drew there, and for another biome of the
    /// band, which `undermere --recipe` and the tests grow a floor as, only
    /// those whose `where` and `not` let them stand in it, so a cave piece
    /// the Underlake may hold is never asked of the cathedral's nave.
    fn picks_for(&self, floor: u32, drawn: &BiomeDraw) -> Vec<Pick> {
        let tags = [place_tag(drawn)];
        self.plan_of(floor).iter().filter(|p| self.pool.entries[p.entry].fits(floor as i32, &tags)).copied().collect()
    }

    /// The stream place `map` is grown from: one per place, so building
    /// floor two first does not change floor one, and the printed floor is
    /// grown from the very stream the run's is.
    fn place_seed(&self, map: MapId) -> RunSeed {
        RunSeed(self.seed.derive(SeedDomain::new(b"undermere.place"), u64::from(map.0)))
    }

    /// The biome `floor` draws: pure over `self.seed`, so every call
    /// agrees without the draw being kept anywhere but here, and the same
    /// whichever floor is asked for first, since every floor's draw is
    /// made once, in [`Undermere::new`], from the floor's own stream.
    pub(crate) fn biome_of(&self, floor: u32) -> &BiomeDraw {
        self.biomes.get(&floor).unwrap_or_else(|| panic!("floor {floor} outside 1..={FLOORS}"))
    }

    /// The light everywhere in `map` before anything in it is lit, which
    /// its build carries: in every place, the engine's dim band, above the
    /// least a cell needs to be seen and below the bright band.
    ///
    /// Mereholt is at dusk, blue and low; every floor below is its own drawn
    /// biome's own ambient, from `assets/biomes.ron`, darker the deeper a
    /// band stands, down to the throne's, among the darkest; the Spider
    /// Warrens' alone is nought, truly dark, so the lantern is the only
    /// light. Seen at all otherwise, so a floor's shape reads beyond the
    /// lantern in deep, dark colour and is remembered where it was seen, as
    /// Brogue keeps it; and no brighter than dim, so a body out there with no
    /// light of its own is harder to pick out than one in a lamp's light, and
    /// the lantern's pool stands out warm and bright. Any other map is dark.
    pub fn ambient_of(&self, map: MapId) -> rl_engine::rl_grid::Light {
        if map == TOWN {
            return rl_engine::rl_grid::Light::new(30, rl_engine::rl_grid::Rgb::new(120, 140, 215));
        }
        floor_of(map).map_or(rl_engine::rl_grid::Light::DARK, |floor| self.biome_of(floor).ambient)
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

    /// Every piece, the same ones the chains stamp from: the engine's
    /// `PrefabPlugin` fills the slots of what was stamped by the key each
    /// stamp carries, which names a piece in this very set.
    pub fn prefabs(&self) -> &Prefabs<MonsterDef> {
        &self.prefabs
    }

    /// The registered tile called `name`; a name nobody registered is a
    /// typo in this file, which every test that builds a place finds.
    fn tile(&self, name: &str) -> TileId {
        self.tiles.expect(name)
    }

    /// Runs `map`'s chain and hands back the context still open, so a test
    /// can read a [`Stamped`] or a [`Surveyed`] pass emitted before
    /// [`PlaceBuild::from_context`] flattens every mark into a `Spot` and
    /// drops the rest.
    fn generate(&self, map: MapId) -> Result<BaseContext, BuildError> {
        let seed = self.place_seed(map);
        match (map, floor_of(map)) {
            (TOWN, _) => {
                let (mut ctx, chain) = self.town()?;
                chain.run(&mut ctx, seed)?;
                Ok(ctx)
            }
            (_, Some(floor)) => self.grow(floor, self.biome_of(floor), self.plan_of(floor), seed, ATTEMPTS),
            (_, None) => Err(BuildError::new("undermere", format!("{map:?} is no place in the Undermere"))),
        }
    }

    /// Mereholt: its piece stamped over a blank map of its own size, so the
    /// piece is the whole town. A run arrives at the piece's `@`, and the
    /// town's way on is its one gate tile.
    fn town(&self) -> Result<(BaseContext, Chain<BaseContext>), BuildError> {
        let town = self.prefabs.piece("Mereholt")?;
        let ctx = BaseContext::blank(town.width(), town.height(), self.tiles.clone(), self.tile("grass"));
        let chain = Chain::new()
            .then(StampPrefab { name: "mereholt", prefab: town, at: Placement::At(Point::ZERO), orient: Orient::Fixed })
            .then(MarkedStart { mark: '@' })
            .then(TileExit { tile: self.tile("gate") })
            .then(Underfoot);
        Ok((ctx, chain))
    }

    // ANCHOR: grow
    /// Floor `floor`, grown as `drawn` says: its recipe of
    /// `assets/layout.ron` accreted, the boss's hall or a mini-boss's lair
    /// grown with it as a piece of its own, loops cut through it, each
    /// area floored by its shape and district, the lakes and the bridges
    /// of its biome, the way in and the way down, the survey, the biome's
    /// decor, and the recipe's requirements last, the whole grown again
    /// from the next attempt, up to `attempts`, whenever a pass refuses it.
    ///
    /// The way in is drawn clear of whatever was stamped before it and the
    /// way down among the farthest third of the floor from it, clear of
    /// every piece. Floor ten is the throne's: no doors, since the walk
    /// there is through halls, its way in held at least half the floor's
    /// longer side from the throne by [`ClearOfTheThrone`], and no way
    /// down.
    fn grow(&self, floor: u32, drawn: &BiomeDraw, picks: &[Pick], seed: RunSeed, attempts: u32) -> Result<BaseContext, BuildError> {
        let throne = floor == FLOORS;
        // Floor ten's own wall and floor are whichever of the three
        // [`FINAL_BOSSES`] this run draws, so its ordinary rooms read as
        // the one place the throne stands in rather than the drawn throne
        // biome's own, which stands beside the Drowned King's own as a
        // schema the other two share.
        let (fill, floor_tile) = if throne {
            let boss = self.final_boss();
            (self.tile(boss.wall), self.tile(boss.floor))
        } else {
            (drawn.wall, drawn.floor)
        };
        let door = if throne { floor_tile } else { self.tile("door") };
        // A vault's own door and gate, which the pool's locked and sealed
        // pieces are joined by.
        let tiles = LayoutTiles { floor: floor_tile, door, locked: self.tile("vault door"), sealed: self.tile("stone gate") };
        let mut grown = rl_engine::rl_bevy::layout::passes(&self.layout, &drawn.recipe, &tiles)?;
        let (width, height) = grown.size;
        // This floor's picks, drawn once for the whole run in
        // `Undermere::new`: the grown ones join the boss's hall and a
        // mini-boss's lair as pieces of the recipe's own accretion, and
        // the rest, stamped inside an area or dressed, are placed by
        // `picked.stamp` once the floor is surveyed, below.
        let picked = rl_engine::rl_bevy::layout::picked(&self.pool, picks, &self.prefabs, Orient::TurnedOrMirrored)?;
        // The boss's hall and a mini-boss's lair are grown as pieces of
        // the recipe's own accretion, late, through the one entry each
        // prefab marks, rather than carved over what grew.
        if throne {
            grown.accrete.pieces.push(self.throne_piece()?);
        } else if let Some(piece) = self.lair_piece(floor)? {
            grown.accrete.pieces.push(piece);
        }
        grown.accrete.pieces.extend(picked.pieces);
        // A landmark the pool seeded on this floor is its first piece,
        // and its stairs say where the way in and the way down stand. The
        // throne's floor has no way down and draws its way in clear of the
        // throne, so a landmark there may only leave its stairs free; one
        // that asks otherwise is refused rather than quietly ignored.
        let stairs = picked.stairs;
        if throne && stairs != Stairs::Free {
            let name = picked.first.as_ref().map_or("", |f| f.name());
            return Err(BuildError::new("layout", format!("{name}: a landmark on the throne's floor, which has no way down, asks its stairs {stairs:?}")));
        }
        grown.accrete.first = picked.first;
        let mut chain = Chain::new().then(grown.accrete).then(grown.loops).then(Paint { floor: floor_tile, rules: drawn.paint.clone() });
        if let Some((lakes, bridges)) = drawn.lakes.clone() {
            chain = chain.then(lakes);
            if let Some(bridges) = bridges {
                chain = chain.then(bridges);
            }
        }
        let look = PieceLook {
            wall: fill,
            floor: floor_tile,
            piece_wall: self.tile("hall wall"),
            piece_floor: self.tile("hall floor"),
            door,
            carved: self.tile("carved door"),
        };
        chain = chain.then(look);
        chain = if throne {
            chain.then(RandomStart.clear_of_stamps(MIN_DISTANCE_FROM_ENTRY)).then(ClearOfTheThrone)
        } else {
            let start = stairs.start(RandomStart.clear_of_stamps(MIN_DISTANCE_FROM_ENTRY));
            chain.then(start).then(stairs.exit(FarthestExit.among_farthest(EXIT_AMONG_PCT).clear_of_stamps(EXIT_CLEAR)))
        };
        chain = chain.then(Survey);
        // The floor's secret doors and hidden rooms, walled over before
        // the picks are stamped, so a pick that prefers a hidden room
        // finds one: as many as the floor's picks ask for at the least.
        let hidden_picks = picks.iter().filter(|p| self.pool.entries[p.entry].prefer == Prefer::Hidden).count() as u32;
        if let Some(hide) = self.hiding.openings(floor, hidden_picks) {
            chain = chain.then(hide);
        }
        chain = chain.then(picked.stamp).then(QuietWayIn);
        // `Dress` lays a dressed pick where `picked.stamp` chose for it,
        // then dresses every area nothing else took, from the biome's
        // own weighted list, `nothing` among the choices.
        chain = chain.then(rl_engine::rl_bevy::layout::dress(&self.prefabs, floor_tile, &self.districts)?);
        chain = self.decor(chain, floor, drawn)?;
        // The floor's traps and loose flagstones, once everything that
        // stands somewhere has taken its cell.
        if let Some(lay) = self.hiding.laying(floor) {
            chain = chain.then(lay);
        }
        // Floor one's way in is where the gate lets out, so its way up is
        // the gate's other side.
        if floor == 1 {
            chain = chain.then(GateBack { gate: self.tile("gate") });
        }
        let make = || BaseContext::blank(width, height, self.tiles.clone(), fill);
        chain.then(Underfoot).then(grown.require).run_retrying(make, seed, attempts)
    }
    // ANCHOR_END: grow

    /// What the drawn biome strews on its floor once the ways are known,
    /// from its `decor` in `assets/biomes.ron`: rubble and braziers in the
    /// Upper Halls, glowcaps in the Fungal Deeps, candles and bones in the
    /// Catacombs, webs and nothing that shines in the Spider Warrens; then
    /// the lived-in dungeon's own and a chest or two on every floor but the
    /// throne's.
    fn decor(&self, chain: Chain<BaseContext>, floor: u32, drawn: &BiomeDraw) -> Result<Chain<BaseContext>, BuildError> {
        let t = |name: &str| self.tile(name);
        let mut chain = chain;
        for (decor, name) in drawn.decor.iter().zip(biomes::DECOR_PASSES) {
            chain = match decor {
                Decor::Patches { tile, count, size } => chain.then(Patches { name, tile: *tile, on: drawn.floor, count: *count, size: *size }),
                Decor::Furnish { piece, count } => chain.then(Furnish { name, piece: self.prefabs.piece(piece)?, count: *count }),
            };
        }
        // The lived-in dungeon's own, on any floor but the throne's: a
        // body a dead end took, and a line of the kingdom's history
        // carved where a wall stands over it. Most floors get neither.
        let chain = if floor == FLOORS {
            chain
        } else {
            chain
                .then(crate::decor::BloodTrail {
                    name: "blood trail",
                    blood: t("blood"),
                    body: self.prefabs.piece("fallen body")?,
                    chance_pct: 35,
                    max_cells: 5,
                })
                .then(crate::decor::WallInscriptions { name: "wall inscription", piece: self.prefabs.piece("wall inscription")?, chance_pct: 25 })
        };
        Ok(if floor == FLOORS { chain } else { chain.then(Furnish { name: "chests", piece: self.prefabs.piece("chest")?, count: CHESTS }) })
    }

    /// Floor `floor` as rows of glyphs, for `undermere --floor N --seed S`
    /// and for reading a recipe before calling it done: `#` what cannot be
    /// walked on, `.` what can, `+` a door, `~` what a body wades through,
    /// `≈` what stops it, `=` a bridge, `<` the way in and `>` the way
    /// down, `*` a locked or sealed door, `?` a cell hiding what it is,
    /// `K` and `L` a vault's key and lever, `^` a trap or a flagstone, and
    /// a piece's or a dressing's slot by its own legend glyph.
    /// `recipe` grows the floor by that recipe of a biome of its band
    /// in place of the one the seed draws; an error names a recipe no
    /// biome of the floor grows, and a floor that cannot be grown is
    /// named with its biome and recipe.
    pub fn render(&self, floor: u32, recipe: Option<&str>) -> Result<String, BuildError> {
        if !(1..=FLOORS).contains(&floor) {
            return Err(BuildError::new("undermere", format!("floor {floor} is outside 1 to {FLOORS}")));
        }
        let drawn = match recipe {
            None => self.biome_of(floor).clone(),
            Some(r) => biomes::with_recipe(&self.defs, floor, r, &self.tiles)
                .ok_or_else(|| BuildError::new("undermere", format!("no biome of floor {floor} grows {r:?}")))?,
        };
        let grown = self
            .grow(floor, &drawn, &self.picks_for(floor, &drawn), self.place_seed(map_of(floor)), ATTEMPTS)
            .map_err(|e| BuildError::new(e.pass, format!("{}, {}: {}", drawn.name, drawn.recipe, e.reason)))?;
        let mut built = PlaceBuild::from_context(grown)?;
        crate::vaults::place_vault_fixtures(&mut built, &self.tiles, &self.prefabs, self.seed, map_of(floor));
        let tables = self.tiles.tables();
        let bridges = [self.tile("bridge"), self.tile("iron bridge")];
        let bounds = built.terrain.bounds();
        let mut out = format!("floor {floor}: {}, {}\n", drawn.name, drawn.recipe);
        for y in 0..bounds.height {
            for x in 0..bounds.width {
                let p = Point::new(x, y);
                let tile = built.terrain.get(p).expect("in bounds");
                // What a piece or a dressing laid at a cell is shown by
                // its own glyph, so a printed floor reads as its rooms.
                let laid = built.spots.iter().find(|s| s.at == p).and_then(|s| match s.tag {
                    crate::vaults::KEY_TAG => Some('K'),
                    crate::vaults::LEVER_TAG => Some('L'),
                    tag if crate::traps::is_hidden_tag(tag) => Some('^'),
                    tag => char::from_u32(tag),
                });
                out.push(match () {
                    _ if p == built.entry => '<',
                    _ if Some(p) == built.exit => '>',
                    _ if built.locks.iter().any(|l| l.at == p) => '*',
                    _ if built.secrets.iter().any(|s| s.at == p) => '?',
                    _ if tables.opens[tile.index()].is_some() => '+',
                    _ if laid.is_some() => laid.expect("just checked"),
                    _ if bridges.contains(&tile) => '=',
                    _ if !tables.passable[tile.index()] && !tables.opaque[tile.index()] => '≈',
                    _ if !tables.passable[tile.index()] => '#',
                    _ if tables.move_cost[tile.index()] > 100 => '~',
                    _ => '.',
                });
            }
            out.push('\n');
        }
        Ok(out)
    }
}

impl PlaceRules for Undermere {
    fn build(&self, map: MapId) -> Result<PlaceBuild, BuildError> {
        let mut built = PlaceBuild::from_context(self.generate(map)?)?;
        crate::vaults::place_vault_fixtures(&mut built, &self.tiles, &self.prefabs, self.seed, map);
        built.ambient = self.ambient_of(map);
        // Mereholt's peace: nothing in the town is harmed, a blow, a shot
        // or a tick of fire or poison alike, and nobody in it is forecast
        // as a fight, since its people are not there to be fought.
        built.peaceful = map == TOWN;
        Ok(built)
    }

    /// The ten floors under the lake, with stairs between them; Mereholt
    /// is no floor, and its gate and floor one's are Undermere's own.
    fn ladder(&self) -> Option<Ladder> {
        Some(crate::stairs::ladder())
    }
}

/// The way in and the way on, as far as the chain has chosen them.
pub(crate) fn ways(ctx: &BaseContext) -> Vec<Point> {
    ctx.outputs().iter::<StartPoint>().map(|s| s.0).chain(ctx.outputs().iter::<ExitPoint>().map(|e| e.0)).collect()
}

/// The bounds of every piece stamped so far.
pub(crate) fn pieces(ctx: &BaseContext) -> Vec<Rect> {
    ctx.outputs().iter::<Stamped>().map(|st| st.bounds).collect()
}

/// Every cell of every piece stamped so far, as a grid over the floor:
/// [`pieces`] read once, where asking each of its rectangles about every
/// cell of the floor was most of what [`Furnish`] and [`Patches`] cost, and
/// the floor's most expensive passes after the accretion itself.
pub(crate) fn under_pieces(ctx: &BaseContext) -> rl_engine::rl_grid::BitGrid {
    let terrain = ctx.terrain();
    let mut under = rl_engine::rl_grid::BitGrid::new(terrain.width(), terrain.height());
    for p in pieces(ctx).into_iter().flat_map(Rect::cells) {
        under.insert(p);
    }
    under
}

/// Every cell a stamp has marked for a monster or a prop, which may yet
/// block: the same cells `rl_mapgen::Dress` itself treats as blocking,
/// whatever is put there, since neither pass knows that until the place's
/// own arrival fills it.
pub(crate) fn slots(ctx: &BaseContext) -> BTreeSet<Point> {
    ctx.outputs().iter::<Stamped>().flat_map(|st| st.marks.iter().map(|(_, p)| *p)).collect()
}

/// Whether a body can stand on `p` of `ctx`'s terrain.
pub(crate) fn walkable(ctx: &BaseContext, tables: &TileTables, p: Point) -> bool {
    ctx.terrain().get(p).is_some_and(|t| tables.walkable[t.index()])
}

/// Every cell reachable from `from`, one orthogonal step at a time,
/// through terrain a body can stand on and no cell of `blocked`.
fn flood(ctx: &BaseContext, tables: &TileTables, blocked: &BTreeSet<Point>, from: Point) -> BTreeSet<Point> {
    let mut seen = BTreeSet::from([from]);
    let mut stack = vec![from];
    while let Some(p) = stack.pop() {
        for d in Direction::CARDINALS {
            let n = p + d.offset();
            if walkable(ctx, tables, n) && !blocked.contains(&n) && seen.insert(n) {
                stack.push(n);
            }
        }
    }
    seen
}

/// Whether making `at` impassable, on top of every cell already in
/// `blocked`, would leave two of its own open neighbours with no other
/// way to reach each other: the floor-wide reading of the guard
/// `rl_mapgen::Dress` already gives one area, since [`Furnish`] scatters
/// a piece over the whole of it rather than one area at a time. If
/// blocking `at` disconnects anything at all, it disconnects two of its
/// own neighbours from one another, since every path between the two
/// sides of such a cut must have crossed the one cell that made it.
fn disconnects(ctx: &BaseContext, tables: &TileTables, blocked: &BTreeSet<Point>, at: Point) -> bool {
    let mut around = blocked.clone();
    around.insert(at);
    let open = |p: Point| walkable(ctx, tables, p) && !around.contains(&p);
    let neighbors: Vec<Point> = Direction::CARDINALS.iter().map(|d| at + d.offset()).filter(|p| open(*p)).collect();
    let Some((first, rest)) = neighbors.split_first() else { return false };
    let seen = flood(ctx, tables, &around, *first);
    rest.iter().any(|n| !seen.contains(n))
}

/// The terrain as a walker crosses it with every door opened on the way:
/// what a shut door costs is a step, where the engine's own reading of a
/// terrain has it a wall.
struct Opened<'a> {
    terrain: &'a Terrain,
    tables: &'a TileTables,
}

impl Grid2D for Opened<'_> {
    fn width(&self) -> i32 {
        self.terrain.width()
    }

    fn height(&self) -> i32 {
        self.terrain.height()
    }
}

impl CostSource for Opened<'_> {
    fn cost_idx(&self, idx: usize) -> Option<u32> {
        let id = self.terrain.get_idx(idx).index();
        self.tables.passable[id].then(|| self.tables.move_cost[id])
    }
}

/// Emits the one cell of `tile` as the way on: the town's gate. A tile, so
/// it shimmers and glows the way a tile does; and the town's way on, so
/// floor one's way up arrives on it by naming the town's exit, where a cell
/// could not be named before the town is built. None, or two, is a town
/// drawn wrong.
struct TileExit {
    tile: TileId,
}

impl Pass<BaseContext> for TileExit {
    fn name(&self) -> &'static str {
        "tile_exit"
    }

    fn phase(&self) -> Phase {
        Phase::Exits
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let cells: Vec<Point> = ctx.terrain().iter().filter(|(_, t)| *t == self.tile).map(|(p, _)| p).collect();
        let [at] = cells.as_slice() else {
            return Err(BuildError::new("tile_exit", format!("{} cells hold the way on, where one should", cells.len())));
        };
        ctx.emit(ExitPoint(*at));
        Ok(())
    }
}

/// Paints the gate at the way in: on floor one the way up is the far side
/// of the Drowned Gate, which shimmers and glows as the near side does.
struct GateBack {
    gate: TileId,
}

impl Pass<BaseContext> for GateBack {
    fn name(&self) -> &'static str {
        "gate_back"
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let start = ctx.outputs().first::<StartPoint>().ok_or_else(|| BuildError::new("gate_back", "no way in to set the gate at"))?.0;
        ctx.terrain_mut().set(start, self.gate);
        Ok(())
    }
}

/// Floors each grown area again by the name of its shape or district, as
/// the biome's `paint` list says: a ruin's caves in the Upper Halls are
/// rubble, a cellar's root caves bare earth. The engine records the names
/// and reads no meaning into them; this is where Undermere does.
///
/// Only the area's own floor is painted, so a door stays a door, and an
/// arch or a mouth between two areas takes the paint only when both sides
/// share it, so a threshold reads as belonging to neither when they
/// differ. Runs straight after growth and its loops, before anything is
/// stamped over the floor or laid on it.
struct Paint {
    /// What growth carved every piece as.
    floor: TileId,
    /// The biome's rules: an area named by either of its names, its shape
    /// first, is floored with the tile.
    rules: Vec<(String, TileId)>,
}

impl Paint {
    /// What an area of `shape` in `district` is floored with.
    fn floor_of(&self, shape: &str, district: &str) -> TileId {
        self.rules.iter().find(|(on, _)| on == shape || on == district).map_or(self.floor, |(_, t)| *t)
    }
}

impl Pass<BaseContext> for Paint {
    fn name(&self) -> &'static str {
        "paint"
    }

    fn phase(&self) -> Phase {
        Phase::Structures
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        if self.rules.is_empty() {
            return Ok(());
        }
        let tiles: Vec<TileId> = ctx.outputs().iter::<Area>().map(|a| self.floor_of(&a.shape, &a.district)).collect();
        let tile_of = |index: usize| tiles.get(index).copied().unwrap_or(self.floor);
        let mut paint: Vec<(Point, TileId)> = Vec::new();
        for (area, tile) in ctx.outputs().iter::<Area>().zip(&tiles) {
            paint.extend(area.cells.iter().filter(|_| *tile != self.floor).map(|p| (*p, *tile)));
        }
        for opening in ctx.outputs().iter::<Opening>() {
            let (a, b) = (tile_of(opening.areas.0), tile_of(opening.areas.1));
            if a == b && a != self.floor {
                paint.extend(opening.cells.iter().map(|p| (*p, a)));
            }
        }
        for (p, tile) in paint {
            if ctx.terrain().get(p) == Some(self.floor) {
                ctx.terrain_mut().set(p, tile);
            }
        }
        Ok(())
    }
}

/// Whether every cell of the ring of eight round `p` can be walked on.
fn open_all_round(ctx: &BaseContext, tables: &TileTables, p: Point) -> bool {
    geometry::square(p, 1).all(|q| q == p || walkable(ctx, tables, q))
}

/// Grows patches of `tile` over cells of `on`, as a biome's `decor` in
/// `assets/biomes.ron` asks: rubble in the halls, bones in the crypts,
/// glowcaps on the moss, puddles and shafts of light on the cathedral's
/// marble.
///
/// Each patch grows from a cell with open floor all round it, so it starts
/// in a room or a cave rather than a corridor, one cell at a time to a
/// size drawn from `size`. It never covers the way in or out, nor a piece
/// already stamped. Only a tile a body can stand on is laid, which the
/// biome file's load checks, so a patch never cuts the floor in two: the water and the lava that stop a body
/// are `rl_mapgen::Lakes`'s, laid with the structures so the ways are
/// chosen round them.
struct Patches {
    /// A stable name, so two patchings in one chain draw different streams.
    name: &'static str,
    /// What a patch is made of.
    tile: TileId,
    /// What it grows over.
    on: TileId,
    /// The fewest and the most patches.
    count: (u32, u32),
    /// The fewest and the most cells in each.
    size: (u32, u32),
}

impl Pass<BaseContext> for Patches {
    fn name(&self) -> &'static str {
        self.name
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        let (kept, under) = (ways(ctx), under_pieces(ctx));
        let free = |p: Point| !kept.contains(&p) && !under.contains(p);
        let count = ctx.rng().random_range(self.count.0..=self.count.1);
        for _ in 0..count {
            let seeds: Vec<Point> =
                ctx.terrain().iter().filter(|(p, t)| *t == self.on && free(*p) && open_all_round(ctx, &tables, *p)).map(|(p, _)| p).collect();
            if seeds.is_empty() {
                break;
            }
            let seed = seeds[ctx.rng().random_range(0..seeds.len())];
            let size = ctx.rng().random_range(self.size.0..=self.size.1);
            let (mut laid, mut frontier) = (0, vec![seed]);
            while laid < size && !frontier.is_empty() {
                let next = ctx.rng().random_range(0..frontier.len());
                let p = frontier.swap_remove(next);
                if ctx.terrain().get(p) != Some(self.on) || !free(p) {
                    continue;
                }
                ctx.terrain_mut().set(p, self.tile);
                laid += 1;
                for d in Direction::CARDINALS {
                    let n = p + d.offset();
                    if ctx.terrain().get(n) == Some(self.on) && !frontier.contains(&n) {
                        frontier.push(n);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Stands a one-cell piece, `count` of it, on open floor: braziers in the
/// halls, candles in the crypts, a chest or two anywhere.
///
/// Each is set where every cell round it can be walked on and no piece
/// stands, never on or beside the way in or out. A piece that stands in the
/// way therefore never shuts anything off: whatever walked through its
/// cell walks round it. Fewer are set where fewer such cells are left.
struct Furnish {
    /// A stable name, so two furnishings in one chain draw different streams.
    name: &'static str,
    /// The piece, one cell of it.
    piece: Prefab,
    /// The fewest and the most.
    count: (u32, u32),
}

impl Pass<BaseContext> for Furnish {
    fn name(&self) -> &'static str {
        self.name
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        if (self.piece.width(), self.piece.height()) != (1, 1) {
            return Err(BuildError::new(self.name, "furnishes with pieces of one cell"));
        }
        let tables = ctx.tiles().tables();
        let count = ctx.rng().random_range(self.count.0..=self.count.1);
        for _ in 0..count {
            let (kept, under, blocked) = (ways(ctx), under_pieces(ctx), slots(ctx));
            let clear = |p: Point| {
                kept.iter().all(|w| geometry::chebyshev(p, *w) > 1)
                    && open_all_round(ctx, &tables, p)
                    && walkable(ctx, &tables, p)
                    && geometry::square(p, 1).all(|q| !under.contains(q))
            };
            let mut spots: Vec<Point> = ctx.terrain().bounds().cells().filter(|p| clear(*p)).collect();
            spots.shuffle(ctx.rng());
            // The first spot, by this shuffle, that does not cut the
            // floor in two once something fills it: a scatter, like
            // `rl_mapgen::Dress`, never cuts off a cell it leaves clear.
            let Some(at) = spots.into_iter().find(|p| !disconnects(ctx, &tables, &blocked, *p)) else {
                break;
            };
            StampPrefab { name: self.name, prefab: self.piece.clone(), at: Placement::At(at), orient: Orient::Fixed }.apply(ctx)?;
        }
        Ok(())
    }
}

/// Stands every slot of every stamped piece on the floor round it.
///
/// A piece's file paints one ground under all its slots, so without this a
/// tavern's tables stood on the square's cobbles, the king stood on bare
/// floor in a gap in his own dais, and a chest stood on a floor of another
/// band. Each slot takes the floor most of its nearest neighbours a body
/// can stand on are made of, looking outward a step at a time through
/// other slots, never through a wall; on a tie it keeps what it has, then
/// takes the first tile registered. Runs last, once everything is stamped.
struct Underfoot;

impl Underfoot {
    /// The floor round the slot at `at`, `slots` being every slot of every
    /// piece; `None` when no floor can be found within a few steps.
    fn floor_round(ctx: &BaseContext, tables: &TileTables, slots: &BTreeSet<Point>, at: Point) -> Option<TileId> {
        let mut seen = BTreeSet::from([at]);
        let mut layer = vec![at];
        for _ in 0..8 {
            let mut next = Vec::new();
            let mut floors: BTreeMap<TileId, usize> = BTreeMap::new();
            for p in &layer {
                for d in Direction::CARDINALS {
                    let n = *p + d.offset();
                    let (slot, floor) = (slots.contains(&n), walkable(ctx, tables, n));
                    if !(slot || floor) || !seen.insert(n) {
                        continue;
                    }
                    if slot {
                        next.push(n);
                    } else if let Some(t) = ctx.terrain().get(n) {
                        *floors.entry(t).or_default() += 1;
                    }
                }
            }
            if let Some(most) = floors.values().max().copied() {
                let tied: Vec<TileId> = floors.iter().filter(|(_, n)| **n == most).map(|(t, _)| *t).collect();
                let here = ctx.terrain().get(at);
                return here.filter(|t| tied.contains(t)).or_else(|| tied.first().copied());
            }
            if next.is_empty() {
                return None;
            }
            layer = next;
        }
        None
    }
}

impl Pass<BaseContext> for Underfoot {
    fn name(&self) -> &'static str {
        "underfoot"
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        let marked = slots(ctx);
        let repaint: Vec<(Point, TileId)> = marked.iter().filter_map(|p| Self::floor_round(ctx, &tables, &marked, *p).map(|t| (*p, t))).collect();
        for (p, t) in repaint {
            ctx.terrain_mut().set(p, t);
        }
        Ok(())
    }
}

/// The one floor this run's single captive stands shackled on: pure over
/// `seed`, the same reasoning [`Undermere::lairs`] and
/// [`Undermere::final_boss`] already follow, so `population::populate`
/// can ask it on any floor's own arrival without a run keeping the draw
/// anywhere. Never the throne's own floor, which `FLOORS` is past: a run
/// draws at most one captive, somewhere in the nine floors above it.
pub(crate) fn captive_floor(seed: RunSeed) -> u32 {
    let mut rng = seed.rng(SeedDomain::new(b"undermere.captive"), 0);
    rng.random_range(1..FLOORS)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

    use rl_engine::rl_core::Rect;
    use rl_engine::rl_mapgen::Join;
    use rl_engine::rl_mapgen::prefab::Stamped;
    use rl_engine::rl_rules::prefab::{Pick, Slot};

    use super::*;
    use crate::testing::{par_seeds, seeds, slot_of as slot};

    /// The cells something stands in the way on once `built` is filled: a
    /// prop that blocks, or a monster, at a piece's slot. Never the entry,
    /// where the engine fills no slot.
    fn standing(undermere: &Undermere, registries: &Registries, built: &PlaceBuild) -> BTreeSet<Point> {
        built
            .spots
            .iter()
            .filter(|spot| spot.at != built.entry)
            .filter(|spot| match slot(undermere, spot) {
                Some(Slot::Prop(id)) => registries.props.get(*id).blocks,
                Some(Slot::Monster { .. }) => true,
                _ => false,
            })
            .map(|spot| spot.at)
            .collect()
    }

    /// Every cell reachable from `from` one orthogonal step at a time
    /// through cells `open` admits: the strictest reading of the engine's
    /// rule that a diagonal step never squeezes between two walls.
    fn walked(from: Point, open: impl Fn(Point) -> bool) -> BTreeSet<Point> {
        let mut seen = BTreeSet::from([from]);
        let mut queue = vec![from];
        while let Some(p) = queue.pop() {
            for d in Direction::CARDINALS {
                let n = p + d.offset();
                if open(n) && seen.insert(n) {
                    queue.push(n);
                }
            }
        }
        seen
    }

    /// The run's backstop: every place of a run, the town and the ten
    /// floors, grown once a seed over a span of seeds, and held to every
    /// promise the generator makes of a floor with the real content in it.
    /// Each promise has a pass that owns it, tested on a made-up map in
    /// that pass's own suite; this is what catches two passes that keep
    /// their own promises and break each other's, so it grows each floor
    /// once and asks everything of it rather than growing every floor again
    /// for each question. Each check below is named for its promise, and
    /// says which floor and seed broke it.
    #[test]
    fn every_place_of_a_run_keeps_every_promise_the_generator_makes_over_a_span_of_seeds() {
        let registries = crate::content::registries();
        let library = crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default());
        let hiding = crate::traps::Hiding::load();
        let (told, locked) = (std::sync::Mutex::new(BTreeSet::new()), AtomicUsize::new(0));
        let first_floors = std::sync::Mutex::new(BTreeMap::new());
        let loaded = Undermere::new(RunSeed(0));
        par_seeds(seeds(0..60), |s| {
            let undermere = loaded.reseeded(RunSeed(s));
            every_recipe_meets_itself_on(&undermere, &recipes_seed_grows(&undermere, s), s, false);
            let tables = undermere.tiles().tables();
            let town = undermere.build(TOWN).unwrap_or_else(|e| panic!("Mereholt, seed {s}: {e}"));
            builds_with_a_way_in_and_a_way_on(&town, &tables, false, &format!("Mereholt, seed {s}"));
            no_post_is_near_the_way_in(&undermere, &town, &format!("Mereholt, seed {s}"));
            for floor in 1..=FLOORS {
                let at = format!("floor {floor}, seed {s}");
                let map = map_of(floor);
                let ctx = undermere.generate(map).unwrap_or_else(|e| panic!("{at}: {e}"));
                every_lair_is_entered_only_through_its_entries(&ctx, &tables, &at);
                let stamped: Vec<Stamped> = ctx.outputs().iter::<Stamped>().cloned().collect();
                let mut built = PlaceBuild::from_context(ctx).unwrap_or_else(|e| panic!("{at}: {e}"));
                crate::vaults::place_vault_fixtures(&mut built, undermere.tiles(), undermere.prefabs(), RunSeed(s), map);
                builds_with_a_way_in_and_a_way_on(&built, &tables, floor == FLOORS, &at);
                if floor == 1 {
                    first_floors.lock().expect("one writer at a time").insert(s, built.terrain.clone());
                }
                every_open_cell_is_walked_to_with_everything_standing(&undermere, &registries, &built, &tables, &at);
                shows_only_what_its_biome_and_its_lairs_lay(&undermere, floor, &built, &at);
                no_post_is_near_the_way_in(&undermere, &built, &at);
                if floor == FLOORS {
                    the_throne_holds_the_drawn_boss_alone_and_far_from_the_way_in(&undermere, &library, &stamped, &built, &at);
                } else {
                    every_prize_is_reached_once_its_key_lever_or_secret_is_found(&undermere, &registries, &built, &tables, &at);
                }
                told.lock().expect("one writer at a time").extend(crate::traps::hidden_keeps_its_promises(&hiding, &built, floor, &tables, &at));
                if crate::vaults::nothing_lands_behind_a_lock(&tables, built, map, &at) {
                    locked.fetch_add(1, Relaxed);
                }
            }
        });
        let first_floors: Vec<(u64, Terrain)> = first_floors.into_inner().expect("every writer done").into_iter().collect();
        for (i, (a, ta)) in first_floors.iter().enumerate() {
            for (b, tb) in &first_floors[i + 1..] {
                assert_ne!(ta, tb, "seeds {a} and {b} lay floor one out alike");
            }
        }
        let told = told.into_inner().expect("every writer done");
        assert!(
            told.contains(&crate::traps::SecretKind::Door) && told.contains(&crate::traps::SecretKind::Room),
            "both doors and rooms over the span: {told:?}"
        );
        assert!(locked.into_inner() >= 3, "some floors over the span lay a lock or a seal");
    }

    /// A place builds with its way in on walkable ground, and, but the
    /// throne's, a way on of its own on walkable ground: a floor that
    /// fails to build is a run that crashes on its stairs.
    fn builds_with_a_way_in_and_a_way_on(built: &PlaceBuild, tables: &TileTables, last: bool, at: &str) {
        let walkable = |p: Point| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]);
        assert!(walkable(built.entry), "{at}: the way in at {:?} is in a wall", built.entry);
        if last {
            assert_eq!(built.exit, None, "{at}: the last floor has no way down");
        } else {
            let on = built.exit.unwrap_or_else(|| panic!("{at}: no way on"));
            assert!(walkable(on) && on != built.entry, "{at}: the way on at {on:?} is in a wall or the way in");
        }
    }

    /// With every prop that stands in the way and every monster a piece
    /// stands at its post, every cell a body may cross can still be walked
    /// to from the way in, doors opened, every secret found and every lock
    /// and seal opened, and the way down with none of them: a cell nobody
    /// reaches is a chest, a stair or a king nobody can get to, and water,
    /// lava and the furniture are each laid so they never cut one off.
    fn every_open_cell_is_walked_to_with_everything_standing(
        undermere: &Undermere,
        registries: &Registries,
        built: &PlaceBuild,
        tables: &TileTables,
        at: &str,
    ) {
        let standing = standing(undermere, registries, built);
        let mut found = built.terrain.clone();
        for secret in &built.secrets {
            found.set(secret.at, secret.is);
        }
        let open = |p: Point| found.get(p).is_some_and(|t| tables.passable[t.index()]) && !standing.contains(&p);
        let seen = walked(built.entry, open);
        if let Some(p) = found.bounds().cells().find(|p| open(*p) && !seen.contains(p)) {
            panic!("{at}: {p:?} cannot be walked to with everything standing");
        }
        let shut: BTreeSet<Point> = built.locks.iter().map(|l| l.at).collect();
        let plain = |p: Point| built.terrain.get(p).is_some_and(|t| tables.passable[t.index()]) && !standing.contains(&p) && !shut.contains(&p);
        if let Some(exit) = built.exit {
            assert!(walked(built.entry, plain).contains(&exit), "{at}: the way down at {exit:?} is shut off");
        }
    }

    /// A floor shows its own wall and floor, and nothing its biome, its
    /// lairs, its pieces and its floor's shared tiles do not lay.
    fn shows_only_what_its_biome_and_its_lairs_lay(undermere: &Undermere, floor: u32, built: &PlaceBuild, at: &str) {
        let drawn = undermere.biome_of(floor);
        let shown: BTreeSet<String> = built.terrain.iter().map(|(_, t)| undermere.tiles().get(t).name.clone()).collect();
        let allowed = undermere.tiles_a_floor_may_show(floor, drawn);
        let (wall, floor_tile) = if floor == FLOORS {
            let boss = undermere.final_boss();
            (boss.wall.to_string(), boss.floor.to_string())
        } else {
            (undermere.tiles().get(drawn.wall).name.clone(), undermere.tiles().get(drawn.floor).name.clone())
        };
        assert!(shown.contains(&wall) && shown.contains(&floor_tile), "{at}: {shown:?}");
        let strays: Vec<&String> = shown.difference(&allowed).collect();
        assert!(strays.is_empty(), "{at}: {strays:?} on a floor of {}", drawn.name);
    }

    /// Nothing a piece stands at its post is nearer the way in than the
    /// population keeps: the way in is drawn clear of every piece stamped
    /// before it, and on the last floor as far from the throne as the
    /// floor allows.
    fn no_post_is_near_the_way_in(undermere: &Undermere, built: &PlaceBuild, at: &str) {
        for spot in built.spots.iter().filter(|spot| matches!(slot(undermere, spot), Some(Slot::Monster { .. }))) {
            let d = geometry::chebyshev(spot.at, built.entry);
            assert!(d >= MIN_DISTANCE_FROM_ENTRY, "{at}: a monster's post at {:?} is {d} from the way in", spot.at);
        }
    }

    /// Every lair and the throne, grown as `Accrete`'s pieces are, is
    /// entered only through the one entry its own prefab file marks: no
    /// other cell of its own wall ever opens onto something a body can
    /// walk through.
    fn every_lair_is_entered_only_through_its_entries(ctx: &BaseContext, tables: &TileTables, at: &str) {
        for area in ctx.outputs().iter::<Area>().filter(|a| a.entries.is_some()) {
            let entries = area.entries.as_ref().expect("filtered on it");
            for c in &area.cells {
                for d in Direction::CARDINALS {
                    let n = *c + d.offset();
                    if !area.cells.contains(&n) && walkable(ctx, tables, n) {
                        assert!(entries.contains(&n), "{at}: {n:?} opens into {} and is no entry", area.shape);
                    }
                }
            }
        }
    }

    /// Floor ten holds whichever of the three `FINAL_BOSSES` this seed
    /// draws, alone at its own throne but for the Drowned King's own five
    /// knights and his braziers, every slot of the throne inside it, and
    /// the way in held at least half the floor's longer side from it,
    /// walking.
    fn the_throne_holds_the_drawn_boss_alone_and_far_from_the_way_in(
        undermere: &Undermere,
        library: &crate::content::Library,
        stamped: &[Stamped],
        built: &PlaceBuild,
        at: &str,
    ) {
        let boss = undermere.final_boss();
        let key = undermere.prefabs().defs().expect(boss.piece).raw();
        let throne: Vec<Rect> = stamped.iter().filter(|st| st.prefab == Some(key)).map(|st| st.bounds).collect();
        assert_eq!(throne.len(), 1, "{at}: one {} on floor ten", boss.piece);
        let throne = throne[0];
        let (mut monsters, mut props): (Vec<&str>, Vec<&str>) = (Vec::new(), Vec::new());
        for spot in built.spots.iter().filter(|s| s.prefab == Some(key)) {
            match slot(undermere, spot) {
                Some(Slot::Monster { pick: Pick::Kind(id), .. }) => monsters.push(&library.bestiary.defs.get(*id).name),
                Some(Slot::Prop(id)) => props.push(&library.registries.props.get(*id).name),
                _ => {}
            }
            assert!(throne.contains(spot.at), "{at}: a slot at {:?} outside the {}", spot.at, boss.piece);
        }
        monsters.sort_unstable();
        props.sort_unstable();
        // The Drowned King's piece keeps its part 1 name, `"the throne"`,
        // rather than his own; the other two's is named as their monster.
        let (expected_monsters, expected_props): (Vec<&str>, Vec<&str>) = if boss.piece == "the throne" {
            (
                ["skeleton knight"; 5].into_iter().chain(["the Drowned King"]).collect(),
                vec!["cold brazier", "cold brazier", "cold brazier", "cold brazier", "throne"],
            )
        } else {
            (vec![boss.piece], vec![])
        };
        assert_eq!((monsters.clone(), props), (expected_monsters, expected_props), "{at}: {}", boss.piece);
        assert!(monsters.contains(&boss.name), "{at}: the boss stood is the one {:?} names, as its rumour is told", boss.name);
        let view = built.terrain.view(undermere.tiles());
        let mut walk = DijkstraMap::covering(&view);
        walk.build(&view, [built.entry], PathRules::default());
        let clear = throne.cells().filter_map(|p| walk.value(p)).min().unwrap_or_else(|| panic!("{at}: the throne can be walked to"));
        let side = built.terrain.bounds().width.max(built.terrain.bounds().height);
        assert!(clear >= side / 2, "{at}: the way in is {clear} steps from the throne, nearer than half the {side} side");
    }

    /// Every prize of every risk-and-reward and reward piece can be reached
    /// from the way in once the floor's secrets are found and its locks and
    /// seals opened, with everything standing: an item on a cell walked to,
    /// a container beside one; and every vault's key and lever lies where
    /// the way in reaches with every lock shut and every secret hidden, so
    /// what opens a vault is never behind it, and every vault has its
    /// opener on its floor.
    fn every_prize_is_reached_once_its_key_lever_or_secret_is_found(
        undermere: &Undermere,
        registries: &Registries,
        built: &PlaceBuild,
        tables: &TileTables,
        at: &str,
    ) {
        let pieces: BTreeSet<u32> = undermere.vault_pieces().iter().filter_map(|name| undermere.prefabs.defs().id(name)).map(|id| id.raw()).collect();
        let standing = standing(undermere, registries, built);
        let mut found = built.terrain.clone();
        for secret in &built.secrets {
            found.set(secret.at, secret.is);
        }
        let reached = walked(built.entry, |p| found.get(p).is_some_and(|t| tables.passable[t.index()]) && !standing.contains(&p));
        let near = |p: Point| reached.contains(&p) || Direction::ALL.iter().any(|d| reached.contains(&(p + d.offset())));
        for spot in built.spots.iter().filter(|s| s.prefab.is_some_and(|k| pieces.contains(&k))) {
            let prize = match slot(undermere, spot) {
                Some(Slot::Item { .. }) => true,
                Some(Slot::Prop(id)) => registries.props.get(*id).container.is_some(),
                _ => false,
            };
            assert!(!prize || near(spot.at), "{at}: a prize at {:?} cannot be reached", spot.at);
        }
        let shut: BTreeSet<Point> = built.locks.iter().map(|l| l.at).collect();
        let plain = walked(built.entry, |p| built.terrain.get(p).is_some_and(|t| tables.passable[t.index()]) && !standing.contains(&p) && !shut.contains(&p));
        for spot in built.spots.iter().filter(|s| [crate::vaults::KEY_TAG, crate::vaults::LEVER_TAG].contains(&s.tag)) {
            assert!(plain.contains(&spot.at), "{at}: a vault's key or lever at {:?} is shut away", spot.at);
        }
        for lock in &built.locks {
            let tag = if lock.key.is_some() { crate::vaults::KEY_TAG } else { crate::vaults::LEVER_TAG };
            assert!(built.spots.iter().any(|s| s.tag == tag), "{at}: a vault at {:?} with nothing on its floor to open it", lock.at);
        }
    }

    /// Mereholt is drawn by hand, so the grass the tree line and the backs
    /// of the houses close in is scenery nobody walks to. Everything else
    /// can be walked to from where a run arrives, with every prop standing:
    /// every door and the room behind it, the square, the streets, the
    /// shore, the causeway and the gate.
    #[test]
    fn everything_in_mereholt_but_the_grass_behind_its_walls_can_be_walked_to_from_the_arrival() {
        let registries = crate::content::registries();
        let undermere = Undermere::new(RunSeed(0));
        let tables = undermere.tiles().tables();
        let built = undermere.build(TOWN).unwrap();
        let standing = standing(&undermere, &registries, &built);
        let open = |p: Point| built.terrain.get(p).is_some_and(|t| tables.passable[t.index()]) && !standing.contains(&p);
        let seen = walked(built.entry, open);
        let grass = undermere.tiles().expect("grass");
        let shut_away: Vec<Point> = built.terrain.bounds().cells().filter(|p| open(*p) && !seen.contains(p)).collect();
        for p in &shut_away {
            assert_eq!(built.terrain.get(*p), Some(grass), "{p:?} in Mereholt cannot be walked to and is not grass");
        }
        assert!(shut_away.len() < 40, "most of the town's grass is walked on: {} cells are not", shut_away.len());
        let door = undermere.tiles().expect("door");
        let doors: Vec<Point> = built.terrain.iter().filter(|(_, t)| *t == door).map(|(p, _)| p).collect();
        assert_eq!(doors.len(), 8, "every building has a door");
        assert!(doors.iter().all(|d| seen.contains(d)), "and every door can be walked to");
        assert!(built.exit.is_some_and(|gate| seen.contains(&gate)), "and so can the gate");
    }

    /// The floor ten boss is drawn from the three `FINAL_BOSSES`, stable
    /// over repeated calls of one seed and over every one of them drawn
    /// somewhere across a span.
    #[test]
    fn the_floor_ten_boss_is_drawn_from_three_stably_and_all_three_drawn_over_a_span_of_seeds() {
        let mut seen = BTreeSet::new();
        let loaded = Undermere::new(RunSeed(0));
        for s in 0..60 {
            let undermere = loaded.reseeded(RunSeed(s));
            let first = undermere.final_boss().piece;
            assert_eq!(undermere.final_boss().piece, first, "seed {s}: two calls disagree");
            seen.insert(first);
        }
        assert_eq!(seen.len(), FINAL_BOSSES.len(), "every one of the three drawn somewhere over the span: {seen:?}");
    }

    /// The town is stamped at the map's corner, so the cell its drawing
    /// marks `@` is where the engine's `MarkedStart` puts the way in.
    #[test]
    fn the_town_s_way_in_is_the_cell_its_drawing_marks_with_an_at_sign() {
        let undermere = Undermere::new(RunSeed(0));
        let town = undermere.prefabs().piece("Mereholt").unwrap();
        let marked: Vec<Point> = town.marks().iter().filter(|(c, _)| *c == '@').map(|(_, p)| *p).collect();
        assert_eq!(marked.len(), 1, "the town marks one way in");
        assert_eq!(undermere.build(TOWN).unwrap().entry, marked[0]);
    }

    #[test]
    fn the_town_has_one_gate_and_the_arrival_is_not_on_it() {
        let undermere = Undermere::new(RunSeed(0));
        let built = undermere.build(TOWN).unwrap();
        let gate = undermere.tiles().expect("gate");
        let gates: Vec<Point> = built.terrain.iter().filter(|(_, t)| *t == gate).map(|(p, _)| p).collect();
        assert_eq!(gates.len(), 1, "one gate: {gates:?}");
        assert_eq!(built.exit, Some(gates[0]), "the town's way on is the gate");
        assert_ne!(built.entry, gates[0], "a run does not arrive on the gate");
        assert_eq!(built.terrain.get(built.entry), Some(undermere.tiles().expect("cobbles")), "it arrives on the square's cobbles");
        // Hand-drawn: every run comes to the same town.
        for s in 1..4 {
            let again = Undermere::new(RunSeed(s)).build(TOWN).unwrap();
            assert_eq!((again.terrain, again.entry, again.exit), (built.terrain.clone(), built.entry, built.exit), "seed {s} drew another town");
        }
    }

    /// Every floor's drawn biome stands on a band that includes it: the
    /// draw never hands a floor a biome from another band.
    #[test]
    fn each_floor_draws_a_biome_whose_band_includes_it() {
        let loaded = Undermere::new(RunSeed(0));
        for s in 0..40 {
            let undermere = loaded.reseeded(RunSeed(s));
            let defs = &undermere.defs;
            for floor in 1..=FLOORS {
                let drawn = undermere.biome_of(floor);
                let bare = drawn.name.trim_start_matches("the ");
                let def = defs.get(defs.expect(bare));
                assert!((def.floors.0..=def.floors.1).contains(&floor), "seed {s}, floor {floor}: {bare} stands on {:?}, not it", def.floors);
            }
        }
    }

    /// A run's floors are a pure function of its seed, whichever is built
    /// first: a floor built deepest first comes out cell for cell as
    /// it does built shallowest first, its way in, its way down, its spots
    /// and its secrets with it, and so do its biome and its picks. A floor
    /// that read a stream or a cache another floor's build had moved would
    /// differ between the two. The floors are the first, a middle one and
    /// the last, on one seed: the property is that every place draws from
    /// its own stream, which no seed's content changes, so one seed shows
    /// it and more only repeat it.
    #[test]
    fn every_floor_is_the_same_whichever_floor_is_built_first() {
        type Built = (Terrain, Point, Option<Point>, Vec<rl_engine::rl_bevy::Spot>, Vec<rl_engine::rl_mapgen::Secret>);
        let build_all = |undermere: &Undermere, floors: &[u32]| -> BTreeMap<u32, Built> {
            floors
                .iter()
                .map(|f| {
                    let b = undermere.build(map_of(*f)).unwrap();
                    (*f, (b.terrain, b.entry, b.exit, b.spots, b.secrets))
                })
                .collect()
        };
        let floors = [1, 5, 10];
        let shallow_first = floors.to_vec();
        let deep_first: Vec<u32> = floors.iter().rev().copied().collect();
        let s = 1;
        let loaded = Undermere::new(RunSeed(0));
        let (a, b) = (loaded.reseeded(RunSeed(s)), loaded.reseeded(RunSeed(s)));
        let (built_a, built_b) = (build_all(&a, &shallow_first), build_all(&b, &deep_first));
        for floor in floors {
            assert_eq!(a.biome_of(floor).name, b.biome_of(floor).name, "seed {s}, floor {floor}: the biome");
            let names = |u: &Undermere| -> Vec<String> { u.plan_of(floor).iter().map(|p| u.pool.entries[p.entry].name.clone()).collect() };
            assert_eq!(names(&a), names(&b), "seed {s}, floor {floor}: the picks");
            assert!(built_a[&floor] == built_b[&floor], "seed {s}, floor {floor}: built otherwise for being built after floor ten");
        }
    }

    /// A run reseeded from content already read draws what a new run of
    /// that seed draws, biome, picks and floor, so a test over hundreds
    /// of seeds' draws may read the content once.
    #[test]
    fn a_reseeded_run_draws_what_a_new_run_of_its_seed_draws() {
        let loaded = Undermere::new(RunSeed(0));
        for s in [1, 7] {
            let (again, new) = (loaded.reseeded(RunSeed(s)), Undermere::new(RunSeed(s)));
            assert_eq!(again.final_boss().piece, new.final_boss().piece, "seed {s}: the boss");
            assert_eq!(again.lairs(), new.lairs(), "seed {s}: the lairs");
            for floor in 1..=FLOORS {
                assert_eq!(again.biome_of(floor).name, new.biome_of(floor).name, "seed {s}, floor {floor}: the biome");
                assert_eq!(again.plan_of(floor), new.plan_of(floor), "seed {s}, floor {floor}: the picks");
            }
            let (a, b) = (again.build(map_of(3)).unwrap(), new.build(map_of(3)).unwrap());
            assert!((a.terrain, a.entry, a.exit) == (b.terrain, b.entry, b.exit), "seed {s}: floor three");
        }
    }

    /// One tile's look as `tiles.ron` writes it, read with the fill and the
    /// spread required rather than optional, so a line that leaves either
    /// out fails to read.
    #[derive(serde::Deserialize)]
    struct Look {
        tile: String,
        glyph: char,
        fg: (f32, f32, f32),
        bg: (f32, f32, f32),
        vary: (f32, f32),
        #[serde(default)]
        shimmer: f32,
    }

    impl Named for Look {
        fn name(&self) -> &str {
            &self.tile
        }
    }

    /// Every registered tile is drawn, and drawn the way the design asks:
    /// both colours in `0..=1`, every cell its own shade of its tile, and
    /// the water, the lava and the gate moving.
    #[test]
    fn every_tile_is_drawn_with_both_colours_and_a_spread_and_water_lava_and_the_gate_shimmer() {
        let undermere = Undermere::new(RunSeed(0));
        // Refuses a registered tile the file forgets, and a line naming one
        // nobody registered.
        let _ = undermere.appearance();
        let looks: Registry<Look> = Registry::from_ron_str(TILES_RON).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"));
        assert_eq!(looks.len(), undermere.tiles().len(), "one look for each tile");
        let unit = |(r, g, b): (f32, f32, f32)| [r, g, b].iter().all(|c| (0.0..=1.0).contains(c));
        for (_, look) in looks.iter() {
            assert!(unit(look.fg) && unit(look.bg), "{}: a colour out of 0..=1", look.tile);
            assert!(look.vary.0 > 0.0 && look.vary.1 >= 0.0, "{}: every cell its own shade", look.tile);
            assert!(!look.glyph.is_whitespace(), "{}: a glyph to see", look.tile);
            let moving = ["shallows", "lake", "flooded floor", "deep water", "black pool", "lava", "gate"].contains(&look.tile.as_str());
            assert!(!moving || look.shimmer > 0.0, "{} shimmers", look.tile);
        }
    }

    /// Brogue draws a wall lighter than the floor it bounds, so the shape
    /// of a room or a cave reads at a glance even in the dark. Every biome's
    /// wall has a ground lighter than each of its floors, painted or not, the doors hung in
    /// it and the tiles that glow aside: in every channel, so it is the
    /// lighter under any light that lands on both, and by a margin the eye
    /// reads, in full light and remembered alike.
    #[test]
    fn every_biomes_walls_are_lighter_than_its_floors() {
        let undermere = Undermere::new(RunSeed(0));
        let look = undermere.appearance();
        for (_, def) in undermere.defs.iter() {
            reads_against(&undermere, &look, &def.name, &def.wall, &def.floor);
            for paint in &def.paint {
                reads_against(&undermere, &look, &def.name, &def.wall, &paint.tile);
            }
        }
    }

    /// The walls of Mereholt's houses read against its streets as a band's
    /// walls do against its floors, the more for the town being known:
    /// most of it is drawn remembered from the first frame. Its trees are a
    /// dark canopy rather than a wall, and a lit window glows.
    #[test]
    fn mereholts_walls_are_lighter_than_its_streets() {
        let undermere = Undermere::new(RunSeed(0));
        let look = undermere.appearance();
        for wall in ["timber wall", "stone wall"] {
            for street in ["grass", "cobbles", "road", "planks", "garden"] {
                reads_against(&undermere, &look, "Mereholt", wall, street);
            }
        }
    }

    /// Asserts that `wall` reads against `floor` in `place` as a Brogue
    /// wall reads against its room: a ground lighter in every channel, so
    /// it stays the lighter under any light that lands on both, and by a
    /// margin the eye reads, in full light and remembered.
    fn reads_against(undermere: &Undermere, look: &TileAppearance, place: &str, wall: &str, floor: &str) {
        use bevy::color::{Color, Oklaba};
        // Oklab lightness, 0 to 1, which steps as evenly as the eye sees:
        // the Upper Halls' rooms read in the photographs at these.
        const IN_FULL_LIGHT: f32 = 0.15;
        const REMEMBERED: f32 = 0.09;
        let ground = |name: &str| look.lit(undermere.tiles().expect(name)).bg;
        let lightness = |c: Color| Oklaba::from(c).lightness;
        let (w, f) = (ground(wall), ground(floor));
        let (lw, lf) = (w.to_linear(), f.to_linear());
        assert!(lw.red >= lf.red && lw.green >= lf.green && lw.blue >= lf.blue, "{place}: the {wall} is darker than the {floor} in some channel");
        for (how, margin, w, f) in [("in full light", IN_FULL_LIGHT, w, f), ("remembered", REMEMBERED, look.memory.recall(w), look.memory.recall(f))] {
            let (w, f) = (lightness(w), lightness(f));
            assert!(w - f >= margin, "{place}: the {wall}'s ground is {w:.2} against the {floor}'s {f:.2} {how}, where a wall reads at {margin} lighter");
        }
    }

    /// A piece's file paints one ground under every slot, and each slot then
    /// stands on the floor drawn round it: in Mereholt a table stands on the
    /// tavern's planks, a counter boxed in by others and by barrels on the
    /// planks past them, the well on the square's cobbles and a lamp post
    /// on its verge of grass; before the throne the king and his throne
    /// stand on the dais.
    #[test]
    fn a_slot_stands_on_the_floor_drawn_round_it() {
        // Seed 1: one of the three `FINAL_BOSSES` a run draws between,
        // and this test's own last two lines are the Drowned King's own,
        // so it wants a seed that draws him.
        let undermere = Undermere::new(RunSeed(1));
        let name = |built: &PlaceBuild, p: Point| undermere.tiles().get(built.terrain.get(p).expect("on the map")).name.clone();
        let town = undermere.build(TOWN).unwrap();
        let under = |x: i32, y: i32| name(&town, Point::new(x, y));
        assert_eq!(under(25, 4), "planks", "a table in the tavern");
        assert_eq!(under(20, 3), "planks", "a counter among counters and barrels");
        assert_eq!(under(46, 4), "planks", "the smithy's anvil");
        assert_eq!(under(35, 12), "cobbles", "the well in the square");
        assert_eq!(under(40, 7), "grass", "a lamp post on the verge");
        assert_eq!(name(&town, town.entry), "cobbles", "where a run arrives");
        let throne = undermere.build(map_of(FLOORS)).unwrap();
        for glyph in ['K', 'T'] {
            let spot = throne.spots.iter().find(|s| s.tag == glyph as u32).unwrap_or_else(|| panic!("the throne marks a '{glyph}'"));
            assert_eq!(name(&throne, spot.at), "dais", "'{glyph}' stands on the dais");
        }
    }

    /// A throne or a lair too large for the floor it is asked to grow on
    /// is refused by name, for `run_retrying` to try again rather than
    /// lay it shut away from the rest of the floor for the walk test to
    /// find later, or never: `PrefabShape` and `Accrete` are the engine's
    /// own, covered end to end in `rl_mapgen::accrete`'s own suite, so
    /// this only pins that Undermere's own pieces are among them.
    /// The throne's floor has no way down and its own way in, so a
    /// landmark seeded there whose stairs are not free is refused by name
    /// rather than grown with its stairs ignored.
    #[test]
    fn a_landmark_on_the_throne_floor_asking_its_stairs_placed_is_refused_by_name() {
        use rl_engine::rl_rules::pool::{Placed, Placement, Stairs as Wanted};
        let mut undermere = Undermere::new(RunSeed(0));
        let grown = undermere.pool.entries.iter().find(|e| matches!(e.placed, Placed::Grown { .. })).expect("a grown piece").clone();
        let piece = grown.piece().expect("it lays a piece").to_string();
        let seeded = rl_engine::rl_rules::pool::Entry {
            name: "a seeded hall".into(),
            placed: Placed::Seeded { piece: piece.clone(), placements: vec![Placement::Center], stairs: Wanted::Beyond },
            ..grown
        };
        undermere.pool.entries.push(seeded);
        let pick = [rl_engine::rl_rules::pool::Pick { entry: undermere.pool.entries.len() - 1, required: true }];
        let drawn = undermere.biome_of(FLOORS).clone();
        let refused = undermere.grow(FLOORS, &drawn, &pick, RunSeed(0), 1).expect_err("the throne's floor holds no way down");
        assert!(refused.pass == "layout" && refused.reason.contains(&piece) && refused.reason.contains("Beyond"), "{refused}");
    }

    #[test]
    fn a_throne_too_large_for_the_floor_is_refused_by_name() {
        let undermere = Undermere::new(RunSeed(0));
        let piece = undermere.throne_piece().unwrap();
        let name = piece.name().to_string();
        let mut pass = rl_engine::rl_mapgen::accrete::Accrete {
            floor: undermere.tile("throne floor"),
            door: undermere.tile("throne floor"),
            shapes: Vec::new(),
            districts: vec![rl_engine::rl_mapgen::accrete::District { name: "only".into(), shapes: Vec::new() }],
            mix: Vec::new(),
            districts_per_10k: 1,
            open_pct: 10,
            patience: 10,
            barriers: Vec::new(),
            locked: undermere.tile("throne floor"),
            sealed: undermere.tile("throne floor"),
            pieces: vec![piece],
            late_pct: 0,
            first: None,
        };
        pass.shapes.push(Box::new(rl_engine::rl_mapgen::accrete::Rectangle { name: "seed".into(), w: (4, 4), h: (4, 4), join: Join::Door }));
        pass.districts[0].shapes.push((0, 1));
        pass.mix.push((0, 1));
        let mut ctx = BaseContext::blank(10, 10, undermere.tiles().clone(), undermere.tile("throne wall"));
        let refused = Chain::new().then(pass).run(&mut ctx, RunSeed(0)).expect_err("a 10x10 floor cannot hold it");
        assert!(refused.pass == "accrete" && refused.reason.contains(&name), "{refused}");
    }

    /// The mini-bosses' draw: two a run, never the same one
    /// twice, each on a floor within its own range, and over enough
    /// seeds every one of the five is drawn somewhere.
    #[test]
    fn two_lairs_a_run_never_the_same_boss_twice_and_all_five_drawn_over_a_span_of_seeds() {
        let mut seen = BTreeSet::new();
        let loaded = Undermere::new(RunSeed(0));
        for s in 0..60 {
            let undermere = loaded.reseeded(RunSeed(s));
            let lairs = undermere.lairs();
            assert_ne!(lairs[0].0, lairs[1].0, "seed {s}: the same boss drawn twice: {lairs:?}");
            for (boss, floor) in lairs {
                let (lo, hi) = MINI_BOSSES[boss].floors;
                assert!((lo..=hi).contains(&floor), "seed {s}: {} drawn on floor {floor}, outside {lo}-{hi}", MINI_BOSSES[boss].name);
                seen.insert(boss);
            }
            assert_ne!(lairs[0].1, lairs[1].1, "seed {s}: both lairs on floor {}", lairs[0].1);
        }
        assert_eq!(seen.len(), MINI_BOSSES.len(), "every mini-boss drawn somewhere over the span: {seen:?}");
    }

    /// The run's one captive stands on a floor of the nine above the throne,
    /// never floor ten: `populate` places it where `floor == captive_floor`,
    /// so this range is the whole of where it can stand.
    #[test]
    fn the_floor_a_run_draws_its_captive_on_is_one_of_the_nine_above_the_throne_over_a_span_of_seeds() {
        for s in seeds(0..200) {
            let floor = captive_floor(RunSeed(s));
            assert!((1..FLOORS).contains(&floor), "seed {s}: the captive on floor {floor}, outside 1..{FLOORS}");
        }
    }

    /// A floor of a biome that grows `recipe`, the `pick`th of those it
    /// may stand on round and round, so a span of picks reaches both floors
    /// of a band; and that biome drawn with it.
    fn grown_by(undermere: &Undermere, recipe: &str, pick: u64) -> (u32, BiomeDraw) {
        let floors: Vec<(u32, BiomeDraw)> =
            (1..=FLOORS).filter_map(|floor| biomes::with_recipe(&undermere.defs, floor, recipe, undermere.tiles()).map(|d| (floor, d))).collect();
        assert!(!floors.is_empty(), "no biome grows {recipe}");
        floors[(pick % floors.len() as u64) as usize].clone()
    }

    /// The recipes of `undermere` that seed `s` grows when the daily seeds
    /// share them out: recipe `i` on every seed with `i % QUICK == s %
    /// QUICK`, so the first `QUICK` seeds grow each recipe once and the
    /// full range in CI grows each several times.
    fn recipes_seed_grows(undermere: &Undermere, s: u64) -> Vec<layout::RecipeDef> {
        let quick = crate::testing::QUICK;
        undermere.layout.recipes.iter().enumerate().filter(|(i, _)| *i as u64 % quick == s % quick).map(|(_, r)| r.clone()).collect()
    }

    /// Each of `recipes` grown on seed `s`, by a run of that seed so its
    /// lairs stand where that run's do, on one of the floors its biome
    /// stands on: within the forty attempts a floor is given, its
    /// requirements met, since `Require` is the chain's last pass; the size
    /// the recipe says; and the way down outside every stamped slot and at
    /// least the recipe's route, a third of the longer side or more, from
    /// the way in. That every open cell is walked to is the run's backstop's
    /// to ask, of the floors a run draws; the span by hand asks it of these
    /// too, in [`recipe_is_walked`].
    ///
    /// The run's picks for the floor are kept only where they may stand
    /// in the recipe's own biome: the run drew them for whichever biome
    /// it drew there, and a cave piece the Underlake may hold has no cave
    /// to stand in when the same floor is grown as the cathedral's nave.
    fn every_recipe_meets_itself_on(undermere: &Undermere, recipes: &[layout::RecipeDef], s: u64, walk: bool) {
        let tables = undermere.tiles().tables();
        for recipe in recipes {
            let (floor, drawn) = grown_by(undermere, &recipe.name, s);
            let at = format!("{} on floor {floor}, seed {s}", recipe.name);
            let grown = undermere.grow(floor, &drawn, &undermere.picks_for(floor, &drawn), RunSeed(s), ATTEMPTS);
            let ctx = grown.unwrap_or_else(|e| panic!("{at}: {e}"));
            let bounds = ctx.terrain().bounds();
            assert_eq!((bounds.width, bounds.height), recipe.size, "{at}");
            let route = ctx.outputs().first::<rl_engine::rl_mapgen::Surveyed>().unwrap_or_else(|| panic!("{at}: not surveyed")).route;
            // Not "outside every piece's own rect": `rl_mapgen::prefab::laid_in`
            // lets a pick's blank cells (the room's own floor, left as it
            // was) coexist with the way in or down inside the rect, so a
            // small piece like `Cold Campsite` can legally have the way
            // down read as "inside" its bounds while painting nothing
            // there. What actually matters is that nothing was stamped
            // onto the exit's own cell.
            let marks = slots(&ctx);
            let built = PlaceBuild::from_context(ctx).unwrap_or_else(|e| panic!("{at}: {e}"));
            if floor == FLOORS {
                assert_eq!(built.exit, None, "{at}: the throne's floor has no way down");
            } else {
                let exit = built.exit.unwrap_or_else(|| panic!("{at}: no way down"));
                assert!(!marks.contains(&exit), "{at}: the way down at {exit:?} is a stamped slot");
                let route = route.unwrap_or_else(|| panic!("{at}: no route"));
                let third = bounds.width.max(bounds.height) as u32 / 3;
                assert!(route >= recipe.require.route && recipe.require.route >= third, "{at}: {route} steps down, asked {}", recipe.require.route);
            }
            if walk {
                recipe_is_walked(&built, &tables, bounds, &at);
            }
        }
    }

    /// Every open cell of a recipe's grown floor walked to from the way in
    /// with every secret found and every lock opened, and the way down with
    /// none of them: the by-hand span's check, beside the run's backstop
    /// that asks the same of the floors a run draws.
    fn recipe_is_walked(built: &PlaceBuild, tables: &TileTables, bounds: Rect, at: &str) {
        let mut found = built.terrain.clone();
        for secret in &built.secrets {
            found.set(secret.at, secret.is);
        }
        let locks: BTreeSet<Point> = built.locks.iter().map(|l| l.at).collect();
        let passable = |terrain: &Terrain, p: Point| terrain.get(p).is_some_and(|t| tables.passable[t.index()]);
        let all_open = |p: Point| passable(&found, p) || locks.contains(&p);
        let seen = walked(built.entry, all_open);
        for p in bounds.cells().filter(|p| all_open(*p)) {
            assert!(seen.contains(&p), "{at}: {p:?} is not walked to with every secret found and every lock opened");
        }
        if let Some(exit) = built.exit {
            let none_open = walked(built.entry, |p| passable(&built.terrain, p) && !locks.contains(&p));
            assert!(none_open.contains(&exit), "{at}: the way down is walked to only through a secret or a lock");
        }
    }

    /// [`every_recipe_meets_itself_on`] with its walk over seeds 0 to 200,
    /// every recipe on each: some minutes, so run by hand with `--ignored`
    /// whenever `assets/layout.ron` or a pass of the floor chain changes.
    /// The everyday span is the run's backstop, which shares the recipes
    /// out over its seeds.
    #[test]
    #[ignore = "seeds 0 to 200 of every recipe take minutes; run by hand when the layout or the chain changes"]
    fn every_recipe_grows_within_forty_attempts_and_meets_itself_over_seeds_0_to_200() {
        let loaded = Undermere::new(RunSeed(0));
        for s in 0..200 {
            let undermere = loaded.reseeded(RunSeed(s));
            every_recipe_meets_itself_on(&undermere, &undermere.layout.recipes.clone(), s, true);
        }
    }

    /// How wide a strip of rock with nothing walkable in it a floor can
    /// leave along a side before it reads as space the floor never used.
    const BAND: i32 = 10;

    /// The widest strip of rock along any one side of `terrain` with no
    /// cell `walkable` in it: a floor grown short of its size.
    fn unused_band(terrain: &Terrain, walkable: impl Fn(rl_engine::rl_grid::TileId) -> bool) -> i32 {
        let bounds = terrain.bounds();
        let open: Vec<Point> = bounds.cells().filter(|p| terrain.get(*p).is_some_and(&walkable)).collect();
        let (Some(left), Some(right)) = (open.iter().map(|p| p.x).min(), open.iter().map(|p| p.x).max()) else { return bounds.width.max(bounds.height) };
        let (top, bottom) = (open.iter().map(|p| p.y).min().unwrap_or(0), open.iter().map(|p| p.y).max().unwrap_or(0));
        let (x1, y1) = (bounds.x + bounds.width - 1, bounds.y + bounds.height - 1);
        [left - bounds.x, x1 - right, top - bounds.y, y1 - bottom].into_iter().max().unwrap_or(0)
    }

    /// A tuning aid, not a property: for every recipe, or the one named by
    /// `SWEEP_RECIPE`, how many of `SWEEP_SEEDS` seeds (a hundred if unset)
    /// grow on their first attempt, how long one attempt takes, why the
    /// rest were refused, numbers masked so like reasons count together,
    /// and how wide a band of unused rock the grown ones leave along a side.
    /// Run by hand with `--ignored --nocapture` while tuning
    /// `assets/layout.ron`; a recipe most of whose first attempts fail is
    /// one to loosen before the forty attempts a floor is given run out.
    #[test]
    #[ignore = "a tuning aid that prints, run by hand"]
    fn sweep_every_recipe_and_print_why_first_attempts_fail() {
        let seeds: u64 = std::env::var("SWEEP_SEEDS").ok().and_then(|v| v.parse().ok()).unwrap_or(100);
        let only = std::env::var("SWEEP_RECIPE").ok();
        let undermere = Undermere::new(RunSeed(0));
        for recipe in undermere.layout.recipes.iter().map(|r| r.name.clone()).filter(|r| only.as_ref().is_none_or(|o| o == r)) {
            let (floor, drawn) = grown_by(&undermere, &recipe, 0);
            let mut why: BTreeMap<String, u32> = BTreeMap::new();
            let started = std::time::Instant::now();
            let tables = undermere.tiles().tables();
            let mut bands: Vec<i32> = Vec::new();
            for s in 0..seeds {
                match undermere.grow(floor, &drawn, &undermere.picks_for(floor, &drawn), RunSeed(s), 1) {
                    Err(e) => {
                        let masked: String = e.to_string().chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
                        *why.entry(masked).or_default() += 1;
                    }
                    Ok(ctx) => bands.push(unused_band(ctx.terrain(), |t| tables.passable[t.index()])),
                }
            }
            let failed: u32 = why.values().sum();
            eprintln!("{recipe}: {} of {seeds} first attempts grow, {:?} an attempt", seeds - u64::from(failed), started.elapsed() / seeds as u32);
            bands.sort_unstable();
            if let (Some(most), false) = (bands.last(), bands.is_empty()) {
                let wide = bands.iter().filter(|b| **b >= BAND).count();
                eprintln!("    unused rock along a side: median {}, most {most}, {wide} of {} floors {BAND} or wider", bands[bands.len() / 2], bands.len());
            }
            let mut why: Vec<(String, u32)> = why.into_iter().collect();
            why.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            for (reason, n) in why.iter().take(4) {
                eprintln!("    {n:>4}  {reason}");
            }
        }
    }

    #[test]
    fn a_lit_window_is_seen_through_and_is_not_walked_or_shot_through() {
        let tiles = registry();
        let (wall, window, floor) = (tiles.id("stone wall").unwrap(), tiles.id("lit window").unwrap(), tiles.id("cobbles").unwrap());
        // A street cell and a room cell with a wall between them, and a
        // window set in that wall in the street's line of sight.
        let terrain = rl_engine::rl_grid::Terrain::from_fn(7, 5, |p| if p.x == 3 { if p.y == 2 { window } else { wall } } else { floor });
        let view = terrain.view(&tiles);
        let (street, room) = (Point::new(1, 2), Point::new(5, 2));
        assert!(rl_engine::rl_grid::fov::can_see(&view, street, room, 8), "the room is seen through the window");
        assert!(!rl_engine::rl_grid::fov::can_see(&view, Point::new(1, 0), Point::new(5, 0), 8), "a wall beside it is not");
        assert!(!view.is_walkable(Point::new(3, 2)), "a body does not pass the glass");
        assert!(view.blocks_projectiles_idx(2 * 7 + 3), "a shot stops at it");
    }
}
