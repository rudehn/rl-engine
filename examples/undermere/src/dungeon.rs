//! Every place a run walks through: Mereholt, the town on the black lake,
//! and the ten floors under it, each built the first time it is entered
//! and kept whole after that, as the engine keeps any place.
//!
//! The town is one hand-drawn piece, `assets/prefabs/mereholt.ron`,
//! stamped over a blank map of its own size, so the piece is the whole
//! town and every run comes to the same one. A floor is grown, as
//! Foundry's decks are, by the engine's accretion from a recipe of
//! `assets/layout.ron`: its biome, drawn by [`biomes`], names the recipes
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
use rl_engine::prelude::*;
use rl_engine::rl_bevy::layout::LayoutTiles;
use rl_engine::rl_core::Rect;
use rl_engine::rl_mapgen::dungeon::{ExitPoint, FarthestExit, RandomStart};
use rl_engine::rl_mapgen::passes::StartPoint;
use rl_engine::rl_mapgen::{Area, Opening};
use rl_engine::rl_render::TileAppearance;
use rl_engine::rl_rules::layout::{self, LayoutDef};

use crate::bestiary::MonsterDef;

mod biomes;
mod lairs;

pub(crate) use biomes::BiomeDraw;
use lairs::ClearOfTheThrone;
#[cfg(test)]
pub(crate) use lairs::{FINAL_BOSSES, MINI_BOSSES};

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

/// How every tile looks, compiled in so the binary runs from anywhere.
const TILES_RON: &str = include_str!("../assets/tiles.ron");

/// How every floor is grown, compiled in beside the tiles.
const LAYOUT_RON: &str = include_str!("../assets/layout.ron");

/// The layout every floor is grown from; a mistake in the file stops the
/// game at startup, every one of them named.
fn layout() -> LayoutDef {
    layout::load(LAYOUT_RON).unwrap_or_else(|e| panic!("assets/layout.ron: {e}"))
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

/// A floor's band, for what is strewn on it once it is grown: rubble and
/// braziers, glowcaps, bones and candles, puddles and moonlight. Fixed by
/// the floor number, same as the design's own bands, whichever biome a
/// seed drew: the biomes' own decor replaces it once a pool file names
/// it, and until then every biome of a band is strewn alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    /// Floors one and two: rubble and braziers.
    UpperHalls,
    /// Floors three and four: glowcaps.
    FungalDeeps,
    /// Floors five and six: bones and candles.
    Catacombs,
    /// Floors seven and eight: puddles and shafts of moonlight.
    DrownedCathedral,
    /// Floor nine: nothing but its lava.
    ForgeOfEmbers,
    /// Floor ten: nothing but the throne.
    Throne,
}

impl Family {
    /// The band floor `floor` stands in.
    fn of(floor: u32) -> Family {
        match floor {
            ..=2 => Family::UpperHalls,
            3 | 4 => Family::FungalDeeps,
            5 | 6 => Family::Catacombs,
            7 | 8 => Family::DrownedCathedral,
            9 => Family::ForgeOfEmbers,
            _ => Family::Throne,
        }
    }

    /// Every tile a floor of this band may show beyond the drawn biome's
    /// own wall, floor, paint and lakes: the doors every grown floor but
    /// the throne's hangs, the band's patches and furnishings, and a
    /// mini-boss's lair tiles wherever its own range
    /// reaches this family's floors, exactly as the fixed six biomes
    /// listed before this task. A lair's own tiles are listed on every
    /// family its boss's range reaches, including the family whose own
    /// biome the lair's tiles used to double as, before a family could
    /// draw more than one biome: the Widow's cave and moss stand on
    /// floors 4 to 6, so `FungalDeeps` lists them too; the Choirmaster's
    /// crypt nave stands on floors 5 to 7, so `Catacombs` lists it beside
    /// `DrownedCathedral`; the Forgemaster's forge stands on floors 7 to
    /// 9, so `ForgeOfEmbers` lists it beside `DrownedCathedral`. The
    /// throne's own floor is read off [`Undermere::final_boss`] rather
    /// than this, so it carries none.
    #[cfg(test)]
    fn shared_tiles(self) -> &'static [&'static str] {
        match self {
            Family::UpperHalls => &["door", "open door", "rubble"],
            Family::FungalDeeps => &["door", "open door", "glowcaps", "goblin hall wall", "goblin hall floor", "web", "cave wall", "moss"],
            Family::Catacombs => &[
                "bones",
                "door",
                "open door",
                "goblin hall wall",
                "goblin hall floor",
                "cave wall",
                "moss",
                "web",
                "cathedral wall",
                "flooded floor",
                "crypt wall",
                "crypt floor",
            ],
            Family::DrownedCathedral => {
                &["door", "open door", "flooded floor", "deep water", "moonlit marble", "crypt wall", "crypt floor", "forge wall", "ash", "lava"]
            }
            Family::ForgeOfEmbers => &["door", "open door", "lava", "forge wall", "ash"],
            // Whichever of the three `FINAL_BOSSES` floor ten draws
            // paints its own hall beyond its wall and its floor: the
            // Drowned King's dais and black pool, the Mother Below's
            // glowcaps and bones, the Ember Wyrm's lava.
            Family::Throne => &["dais", "black pool", "cave wall", "moss", "glowcaps", "bones", "forge wall", "ash", "lava"],
        }
    }
}

/// Every tile a place is built of, the town's first, then the doors, then
/// each band's in order. What a tile looks like is `assets/tiles.ron`'s;
/// what it is to a body is here.
///
/// Wading costs twice a step, whether the water is the town's shallows,
/// the cathedral's flooded floor or the black pools before the throne.
/// Deep water, the lake and lava stop a body and nothing else: they are
/// seen across and shot over.
fn registry() -> TileRegistry {
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
            TileProps::wall("lit window"),
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
            // The Cellars, floors one and two's other biome.
            TileProps::wall("cellar wall"),
            TileProps::floor("cellar floor"),
            // Bare ground: the cellars' root caves and the mines' caves.
            TileProps::floor("earth"),
            // The Watch Barracks, floors one and two's third biome.
            TileProps::wall("barracks wall"),
            TileProps::floor("barracks floor"),
            // The Fungal Deeps.
            TileProps::wall("cave wall"),
            TileProps::floor("moss"),
            TileProps::floor("glowcaps"),
            // The Spider Warrens, floors three and four's other biome.
            TileProps::wall("warren wall"),
            TileProps::floor("warren floor"),
            // The Old Mines, floors three and four's third biome.
            TileProps::wall("mine wall"),
            TileProps::floor("mine floor"),
            // The Catacombs.
            TileProps::wall("crypt wall"),
            TileProps::floor("crypt floor"),
            TileProps::floor("bones"),
            // The Sunken Library, floors five and six's other biome.
            TileProps::wall("library wall"),
            TileProps::floor("library floor"),
            // The Drowned Cathedral.
            TileProps::wall("cathedral wall"),
            TileProps::floor("marble"),
            wading("flooded floor"),
            TileProps::named("deep water"),
            TileProps::floor("moonlit marble"),
            // The Underlake, floors seven and eight's other biome.
            TileProps::wall("underlake wall"),
            TileProps::floor("underlake floor"),
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
        ])
        .expect("every tile has a name of its own");
    tiles
}

/// Undermere's tiles, its pieces, how each of its places is built, and
/// every floor's own biome, drawn once here and handed out by
/// [`biome_of`](Self::biome_of).
pub struct Undermere {
    tiles: TileRegistry,
    prefabs: Prefabs<MonsterDef>,
    seed: RunSeed,
    layout: LayoutDef,
    defs: Registry<biomes::BiomeDef>,
    biomes: BTreeMap<u32, BiomeDraw>,
}

impl Undermere {
    /// Registers every tile a place is built of, loads every piece and the
    /// layout against them, and draws every floor's biome and recipe. The
    /// load reads the content registries and the bestiary itself, the same
    /// ones the run inserts, which is what makes a piece's prop and
    /// monster ids match the running game's.
    pub fn new(seed: RunSeed) -> Self {
        let tiles = registry();
        // After the tiles, which the pieces paint by name.
        let prefabs = crate::prefabs::load(&tiles);
        let layout = layout();
        let defs = biomes::load(&tiles, &layout);
        let boss = lairs::draw_final_boss(seed);
        let mut biomes = BTreeMap::new();
        for floor in 1..FLOORS {
            biomes.insert(floor, biomes::draw(&defs, floor, seed, &tiles));
        }
        biomes.insert(FLOORS, biomes::throne(&defs, boss.biome, seed, &tiles));
        Self { tiles, prefabs, seed, layout, defs, biomes }
    }

    /// The biome `floor` draws: pure over `self.seed`, so every call
    /// agrees without the draw being kept anywhere but here, and the same
    /// whichever floor is asked for first, since every floor's draw is
    /// made once, in [`Undermere::new`], from the floor's own stream.
    pub(crate) fn biome_of(&self, floor: u32) -> &BiomeDraw {
        self.biomes.get(&floor).unwrap_or_else(|| panic!("floor {floor} outside 1..={FLOORS}"))
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
        // One stream per place, so building floor two first does not change
        // floor one.
        let seed = RunSeed(self.seed.derive(SeedDomain::new(b"undermere.place"), u64::from(map.0)));
        match (map, floor_of(map)) {
            (TOWN, _) => {
                let (mut ctx, chain) = self.town()?;
                chain.run(&mut ctx, seed)?;
                Ok(ctx)
            }
            (_, Some(floor)) => self.grow(floor, self.biome_of(floor), seed, ATTEMPTS),
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
    /// of its biome, the way in and the way down, the survey, the band's
    /// decor, and the recipe's requirements last, the whole grown again
    /// from the next attempt, up to `attempts`, whenever a pass refuses it.
    ///
    /// The way in is drawn clear of whatever was stamped before it and the
    /// way down among the farthest third of the floor from it, clear of
    /// every piece. Floor ten is the throne's: no doors, since the walk
    /// there is through halls, its way in held at least half the floor's
    /// longer side from the throne by [`ClearOfTheThrone`], and no way
    /// down.
    fn grow(&self, floor: u32, drawn: &BiomeDraw, seed: RunSeed, attempts: u32) -> Result<BaseContext, BuildError> {
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
        // Undermere grows no locked or sealed piece yet, so its door
        // stands for both.
        let tiles = LayoutTiles { floor: floor_tile, door, locked: door, sealed: door };
        let mut grown = rl_engine::rl_bevy::layout::passes(&self.layout, &drawn.recipe, &tiles)?;
        let (width, height) = grown.size;
        // The boss's hall and a mini-boss's lair are grown as pieces of
        // the recipe's own accretion, late, through the one entry each
        // prefab marks, rather than carved over what grew.
        if throne {
            grown.accrete.pieces.push(self.throne_piece()?);
        } else if let Some(piece) = self.lair_piece(floor)? {
            grown.accrete.pieces.push(piece);
        }
        let mut chain = Chain::new().then(grown.accrete).then(grown.loops).then(Paint { floor: floor_tile, rules: drawn.paint.clone() });
        if let Some((lakes, bridges)) = drawn.lakes.clone() {
            chain = chain.then(lakes);
            if let Some(bridges) = bridges {
                chain = chain.then(bridges);
            }
        }
        chain = if throne {
            chain.then(RandomStart.clear_of_stamps(MIN_DISTANCE_FROM_ENTRY)).then(ClearOfTheThrone)
        } else {
            chain.then(RandomStart.clear_of_stamps(MIN_DISTANCE_FROM_ENTRY)).then(FarthestExit.among_farthest(EXIT_AMONG_PCT).clear_of_stamps(EXIT_CLEAR))
        };
        chain = self.decor(chain.then(Survey), floor, drawn)?;
        // Floor one's way in is where the gate lets out, so its way up is
        // the gate's other side.
        if floor == 1 {
            chain = chain.then(GateBack { gate: self.tile("gate") });
        }
        let make = || BaseContext::blank(width, height, self.tiles.clone(), fill);
        chain.then(Underfoot).then(grown.require).run_retrying(make, seed, attempts)
    }
    // ANCHOR_END: grow

    /// What a band strews on its floor once the ways are known: rubble and
    /// braziers in the first band, glowcaps in the second, bones and
    /// candles in the third, puddles and moonlight in the fourth, and a
    /// chest or two on every floor but the throne's. The biomes' own decor
    /// replaces this by band once a pool file names it.
    fn decor(&self, chain: Chain<BaseContext>, floor: u32, drawn: &BiomeDraw) -> Result<Chain<BaseContext>, BuildError> {
        let t = |name: &str| self.tile(name);
        let on = drawn.floor;
        let chain = match Family::of(floor) {
            Family::UpperHalls => chain
                .then(Patches { name: "rubble", tile: t("rubble"), on, count: (5, 9), size: (2, 6), phase: Phase::Finish })
                .then(Furnish { name: "braziers", piece: self.prefabs.piece("brazier")?, count: (3, 5) }),
            Family::FungalDeeps => chain.then(Patches { name: "glowcaps", tile: t("glowcaps"), on, count: (8, 12), size: (2, 6), phase: Phase::Finish }),
            Family::Catacombs => chain
                .then(Patches { name: "bones", tile: t("bones"), on, count: (6, 10), size: (1, 4), phase: Phase::Finish })
                .then(Furnish { name: "candles", piece: self.prefabs.piece("candelabra")?, count: (5, 8) }),
            Family::DrownedCathedral => chain
                .then(Patches { name: "puddles", tile: t("flooded floor"), on, count: (3, 6), size: (2, 6), phase: Phase::Finish })
                .then(Patches { name: "shafts", tile: t("moonlit marble"), on, count: (3, 5), size: (1, 1), phase: Phase::Finish }),
            Family::ForgeOfEmbers | Family::Throne => chain,
        };
        Ok(if floor == FLOORS { chain } else { chain.then(Furnish { name: "chests", piece: self.prefabs.piece("chest")?, count: (1, 2) }) })
    }

    /// Floor `floor` as rows of glyphs, for `undermere --floor N --seed S`
    /// and for reading a recipe before calling it done: `#` what cannot be
    /// walked on, `.` what can, `+` a door, `~` what a body wades through,
    /// `≈` what stops it, `=` a bridge, `<` the way in and `>` the way
    /// down. `recipe` grows the floor by that recipe of a biome of its band
    /// in place of the one the seed draws; an error names a recipe no
    /// biome of the floor grows.
    pub fn render(&self, floor: u32, recipe: Option<&str>) -> Result<String, BuildError> {
        if !(1..=FLOORS).contains(&floor) {
            return Err(BuildError::new("undermere", format!("floor {floor} is outside 1 to {FLOORS}")));
        }
        let drawn = match recipe {
            None => self.biome_of(floor).clone(),
            Some(r) => biomes::with_recipe(&self.defs, floor, r, &self.tiles)
                .ok_or_else(|| BuildError::new("undermere", format!("no biome of floor {floor} grows {r:?}")))?,
        };
        let seed = RunSeed(self.seed.derive(SeedDomain::new(b"undermere.place"), u64::from(floor)));
        let built = PlaceBuild::from_context(self.grow(floor, &drawn, seed, ATTEMPTS)?)?;
        let tables = self.tiles.tables();
        let (door, bridges) = (self.tile("door"), [self.tile("bridge"), self.tile("iron bridge")]);
        let bounds = built.terrain.bounds();
        let mut out = format!("floor {floor}: {}, {}\n", drawn.name, drawn.recipe);
        for y in 0..bounds.height {
            for x in 0..bounds.width {
                let p = Point::new(x, y);
                let tile = built.terrain.get(p).expect("in bounds");
                out.push(match () {
                    _ if p == built.entry => '<',
                    _ if Some(p) == built.exit => '>',
                    _ if tile == door => '+',
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
    fn build(&self, map: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
        PlaceBuild::from_context(self.generate(map)?)
    }
}

/// The way in and the way on, as far as the chain has chosen them.
fn ways(ctx: &BaseContext) -> Vec<Point> {
    ctx.outputs().iter::<StartPoint>().map(|s| s.0).chain(ctx.outputs().iter::<ExitPoint>().map(|e| e.0)).collect()
}

/// The bounds of every piece stamped so far.
fn pieces(ctx: &BaseContext) -> Vec<Rect> {
    ctx.outputs().iter::<Stamped>().map(|st| st.bounds).collect()
}

/// Whether a body can stand on `p` of `ctx`'s terrain.
fn walkable(ctx: &BaseContext, tables: &TileTables, p: Point) -> bool {
    ctx.terrain().get(p).is_some_and(|t| tables.walkable[t.index()])
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

/// Emits the cell a piece marked `mark` as the way in: the town's `@`.
struct MarkedStart {
    mark: char,
}

impl Pass<BaseContext> for MarkedStart {
    fn name(&self) -> &'static str {
        "marked_start"
    }

    fn phase(&self) -> Phase {
        Phase::Exits
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let at = ctx.outputs().iter::<Stamped>().flat_map(|st| st.marks.iter()).find(|(c, _)| *c == self.mark).map(|(_, p)| *p);
        let at = at.ok_or_else(|| BuildError::new("marked_start", format!("no piece marks a '{}'", self.mark)))?;
        ctx.emit(StartPoint(at));
        Ok(())
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

/// The ring of eight round a cell, clockwise from north: each a single
/// step from the next, so a run of walkable cells round the ring is a run
/// a body can walk along.
const RING: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// Whether `p` can stop being walkable without cutting anything off: the
/// cells a body steps to from it all join up through its own ring, so
/// whatever walked through `p` can walk round it instead.
///
/// A local answer, and a sure one: if the cells beside `p` are joined round
/// it they are joined everywhere, so the floor stays one piece. It refuses
/// some cells that a longer way round would have spared, and a patch only
/// grows elsewhere for it.
fn keeps_the_walk_whole(ctx: &BaseContext, tables: &TileTables, p: Point) -> bool {
    let open: Vec<bool> = RING.iter().map(|(dx, dy)| walkable(ctx, tables, p.offset(*dx, *dy))).collect();
    if open.iter().all(|o| *o) {
        return true;
    }
    // Every run of walkable cells round the ring, counted only when it
    // holds a cell a body steps to straight from `p`: those are the even
    // places, north, east, south and west.
    let mut runs = 0;
    for start in (0..8).filter(|i| open[*i] && !open[(i + 7) % 8]) {
        let beside = (start..start + 8).take_while(|i| open[i % 8]).any(|i| i % 2 == 0);
        runs += usize::from(beside);
    }
    runs <= 1
}

/// Grows patches of `tile` over cells of `on`: rubble in the halls, bones in
/// the crypts, glowcaps on the moss, deep water and puddles in the
/// cathedral, shafts of light on its marble, and lava in the forge.
///
/// Each patch grows from a cell with open floor all round it, so it starts
/// in a room or a cave rather than a corridor, one cell at a time to a
/// size drawn from `size`. A tile a body cannot stand on is never laid
/// where it would cut the floor in two ([`keeps_the_walk_whole`]), so a
/// lake grows round a corridor rather than across it. It never covers the
/// way in or out, nor a piece already stamped.
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
    /// When it runs: before the ways are chosen for a patch that stops a
    /// body, so they are chosen round it, and after for one that does not.
    phase: Phase,
}

impl Pass<BaseContext> for Patches {
    fn name(&self) -> &'static str {
        self.name
    }

    fn phase(&self) -> Phase {
        self.phase
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        let solid = !tables.walkable[self.tile.index()];
        let (kept, pieces) = (ways(ctx), pieces(ctx));
        let free = |p: Point| !kept.contains(&p) && pieces.iter().all(|r| !r.contains(p));
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
                if ctx.terrain().get(p) != Some(self.on) || !free(p) || (solid && !keeps_the_walk_whole(ctx, &tables, p)) {
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
            let (kept, pieces) = (ways(ctx), pieces(ctx));
            let clear = |p: Point| {
                kept.iter().all(|w| geometry::chebyshev(p, *w) > 1)
                    && open_all_round(ctx, &tables, p)
                    && walkable(ctx, &tables, p)
                    && geometry::square(p, 1).all(|q| pieces.iter().all(|r| !r.contains(q)))
            };
            let spots: Vec<Point> = ctx.terrain().bounds().cells().filter(|p| clear(*p)).collect();
            if spots.is_empty() {
                break;
            }
            let at = spots[ctx.rng().random_range(0..spots.len())];
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
        let slots: BTreeSet<Point> = ctx.outputs().iter::<Stamped>().flat_map(|st| st.marks.iter().map(|(_, p)| *p)).collect();
        let repaint: Vec<(Point, TileId)> = slots.iter().filter_map(|p| Self::floor_round(ctx, &tables, &slots, *p).map(|t| (*p, t))).collect();
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

    use rl_engine::rl_core::Rect;
    use rl_engine::rl_mapgen::Join;
    use rl_engine::rl_mapgen::prefab::Stamped;
    use rl_engine::rl_rules::prefab::{Pick, Slot};

    use super::*;

    /// Every place a run walks through: the town, then the floors in order.
    fn places() -> impl Iterator<Item = MapId> {
        std::iter::once(TOWN).chain((1..=FLOORS).map(map_of))
    }

    /// What a piece's slot at `spot` holds, if the spot is a slot.
    fn slot<'a>(undermere: &'a Undermere, spot: &Spot) -> Option<&'a Slot<MonsterDef>> {
        spot.prefab.zip(char::from_u32(spot.tag)).and_then(|(key, glyph)| undermere.prefabs().slot(key, glyph))
    }

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

    /// A floor that fails to build is a run that crashes on its stairs, so
    /// every place is built over three hundred seeds.
    #[test]
    fn every_floor_and_the_town_build_over_a_span_of_seeds_with_an_entry_and_a_way_on() {
        for s in 0..300 {
            let undermere = Undermere::new(RunSeed(s));
            let tables = undermere.tiles().tables();
            for map in places() {
                let built = undermere.build(map, None).unwrap_or_else(|e| panic!("{map:?}, seed {s}: {e}"));
                let walkable = |p: Point| built.terrain.get(p).is_some_and(|t| tables.walkable[t.index()]);
                assert!(walkable(built.entry), "{map:?}, seed {s}: the way in at {:?} is in a wall", built.entry);
                if floor_of(map) == Some(FLOORS) {
                    assert_eq!(built.exit, None, "seed {s}: the last floor has no way down");
                } else {
                    let on = built.exit.unwrap_or_else(|| panic!("{map:?}, seed {s}: no way on"));
                    assert!(walkable(on), "{map:?}, seed {s}: the way on at {on:?} is in a wall");
                    assert_ne!(on, built.entry, "{map:?}, seed {s}: the way in is the way on");
                }
            }
        }
    }

    /// With every prop that stands in the way and every monster a piece
    /// stands at its post, every cell a body may cross on every floor can
    /// still be walked to from the floor's way in, doors opened: a floor
    /// with a cell nobody reaches is a floor with a chest, a stair or a
    /// king nobody can get to, and water, lava and the furniture are each
    /// laid so they never cut one off.
    #[test]
    fn every_open_cell_of_every_floor_can_be_walked_to_from_its_entry_with_everything_standing() {
        let registries = crate::content::registries();
        for s in 0..60 {
            let undermere = Undermere::new(RunSeed(s));
            let tables = undermere.tiles().tables();
            for floor in 1..=FLOORS {
                let built = undermere.build(map_of(floor), None).unwrap();
                let standing = standing(&undermere, &registries, &built);
                let open = |p: Point| built.terrain.get(p).is_some_and(|t| tables.passable[t.index()]) && !standing.contains(&p);
                let seen = walked(built.entry, open);
                for p in built.terrain.bounds().cells().filter(|p| open(*p)) {
                    assert!(seen.contains(&p), "floor {floor}, seed {s}: {p:?} cannot be walked to with everything standing");
                }
                if let Some(exit) = built.exit {
                    assert!(seen.contains(&exit), "floor {floor}, seed {s}: something stands on the way down at {exit:?}");
                }
            }
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
        let built = undermere.build(TOWN, None).unwrap();
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

    /// Floor ten holds whichever of the three `FINAL_BOSSES` this seed
    /// draws, alone at its own throne but for the Drowned King's own
    /// five knights, and the way in is held at least half the floor's
    /// longer side from it, walking, for every one of the three floor
    /// ten bosses.
    #[test]
    fn floor_ten_holds_whichever_boss_this_seed_draws_alone_at_its_own_throne() {
        let library = crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default());
        for s in 0..40 {
            let undermere = Undermere::new(RunSeed(s));
            let boss = undermere.final_boss();
            let key = undermere.prefabs().defs().expect(boss.piece).raw();
            let ctx = undermere.generate(map_of(FLOORS)).unwrap();
            let throne: Vec<Rect> = ctx.outputs().iter::<Stamped>().filter(|st| st.prefab == Some(key)).map(|st| st.bounds).collect();
            assert_eq!(throne.len(), 1, "seed {s}: one {} on floor ten", boss.piece);
            let throne = throne[0];
            let built = PlaceBuild::from_context(ctx).unwrap();
            let mut monsters: Vec<&str> = Vec::new();
            let mut props: Vec<&str> = Vec::new();
            for spot in &built.spots {
                match slot(&undermere, spot) {
                    Some(Slot::Monster { pick: Pick::Kind(id), .. }) => monsters.push(&library.bestiary.defs.get(*id).name),
                    Some(Slot::Prop(id)) => props.push(&library.registries.props.get(*id).name),
                    _ => {}
                }
                assert!(throne.contains(spot.at), "seed {s}: a slot at {:?} outside the {}", spot.at, boss.piece);
            }
            monsters.sort_unstable();
            props.sort_unstable();
            // The Drowned King's piece keeps its part 1 name, `"the
            // throne"`, rather than his own, `"the Drowned King"`; the
            // other two's piece is named exactly as their own monster.
            let (expected_monsters, expected_props): (Vec<&str>, Vec<&str>) = if boss.piece == "the throne" {
                (["skeleton knight"; 5].into_iter().chain(["the Drowned King"]).collect(), vec!["brazier", "brazier", "brazier", "brazier", "throne"])
            } else {
                (vec![boss.piece], vec![])
            };
            assert_eq!(monsters, expected_monsters, "seed {s}: {}", boss.piece);
            assert_eq!(props, expected_props, "seed {s}: {}", boss.piece);
            // The way in is held at least half the floor's longer side
            // from the throne, walking, as `ClearOfTheThrone` asks: the
            // throne is grown now, so its mouth may face any way.
            let view = built.terrain.view(undermere.tiles());
            let mut walk = DijkstraMap::covering(&view);
            walk.build(&view, [built.entry], PathRules::default());
            let clear = throne.cells().filter_map(|p| walk.value(p)).min().expect("seed {s}: the throne can be walked to");
            let bounds = built.terrain.bounds();
            let need = bounds.width.max(bounds.height) / 2;
            assert!(clear >= need, "seed {s}: the way in is {clear} steps from the throne, nearer than half the {} side", bounds.width.max(bounds.height));
        }
    }

    /// The floor ten boss is drawn from the three `FINAL_BOSSES`, stable
    /// over repeated calls of one seed and over every one of them drawn
    /// somewhere across a span.
    #[test]
    fn the_floor_ten_boss_is_drawn_from_three_stably_and_all_three_drawn_over_a_span_of_seeds() {
        let mut seen = BTreeSet::new();
        for s in 0..60 {
            let undermere = Undermere::new(RunSeed(s));
            let first = undermere.final_boss().piece;
            assert_eq!(undermere.final_boss().piece, first, "seed {s}: two calls disagree");
            seen.insert(first);
        }
        assert_eq!(seen.len(), FINAL_BOSSES.len(), "every one of the three drawn somewhere over the span: {seen:?}");
    }

    #[test]
    fn the_town_has_one_gate_and_the_arrival_is_not_on_it() {
        let undermere = Undermere::new(RunSeed(0));
        let built = undermere.build(TOWN, None).unwrap();
        let gate = undermere.tiles().expect("gate");
        let gates: Vec<Point> = built.terrain.iter().filter(|(_, t)| *t == gate).map(|(p, _)| p).collect();
        assert_eq!(gates.len(), 1, "one gate: {gates:?}");
        assert_eq!(built.exit, Some(gates[0]), "the town's way on is the gate");
        assert_ne!(built.entry, gates[0], "a run does not arrive on the gate");
        assert_eq!(built.terrain.get(built.entry), Some(undermere.tiles().expect("cobbles")), "it arrives on the square's cobbles");
        // Hand-drawn: every run comes to the same town.
        for s in 1..4 {
            let again = Undermere::new(RunSeed(s)).build(TOWN, None).unwrap();
            assert_eq!((again.terrain, again.entry, again.exit), (built.terrain.clone(), built.entry, built.exit), "seed {s} drew another town");
        }
    }

    #[test]
    fn each_floor_draws_its_own_wall_and_floor_and_shows_nothing_outside_its_family() {
        for s in 0..20 {
            let undermere = Undermere::new(RunSeed(s));
            for floor in 1..=FLOORS {
                let drawn = undermere.biome_of(floor);
                let built = undermere.build(map_of(floor), None).unwrap();
                let shown: BTreeSet<&str> = built.terrain.iter().map(|(_, t)| undermere.tiles().get(t).name.as_str()).collect();
                // Floor ten's own wall and floor are whichever of the
                // three `FINAL_BOSSES` this seed draws, never the drawn
                // throne biome's own, which stands beside the Drowned
                // King's own as a schema the other two bosses share.
                let (wall, floor_tile) = if floor == FLOORS {
                    let boss = undermere.final_boss();
                    (boss.wall, boss.floor)
                } else {
                    (undermere.tiles().get(drawn.wall).name.as_str(), undermere.tiles().get(drawn.floor).name.as_str())
                };
                assert!(shown.contains(wall) && shown.contains(floor_tile), "floor {floor}, seed {s}: {shown:?}");
                // The gate lets out on floor one: its other side. Beyond
                // its band's own, a floor shows what its biome paints, its
                // lakes, their shore and their bridges.
                let shared = Family::of(floor).shared_tiles();
                let name = |t: TileId| undermere.tiles().get(t).name.as_str();
                let painted = drawn.paint.iter().map(|(_, t)| name(*t));
                let lakes = drawn.lakes.iter().flat_map(|(l, b)| [Some(l.tile), l.wreath.map(|(t, _)| t), b.map(|b| b.tile)]).flatten().map(name);
                let allowed: BTreeSet<&str> =
                    shared.iter().copied().chain([wall, floor_tile]).chain(painted).chain(lakes).chain((floor == 1).then_some("gate")).collect();
                let strays: Vec<&&str> = shown.difference(&allowed).collect();
                assert!(strays.is_empty(), "floor {floor}, seed {s}: {strays:?} on a floor of {}", drawn.name);
            }
        }
    }

    /// Every floor's drawn biome stands on a band that includes it: the
    /// draw never hands a floor a biome from another band.
    #[test]
    fn each_floor_draws_a_biome_whose_band_includes_it() {
        for s in 0..40 {
            let undermere = Undermere::new(RunSeed(s));
            let defs = &undermere.defs;
            for floor in 1..=FLOORS {
                let drawn = undermere.biome_of(floor);
                let bare = drawn.name.trim_start_matches("the ");
                let def = defs.get(defs.expect(bare));
                assert!((def.floors.0..=def.floors.1).contains(&floor), "seed {s}, floor {floor}: {bare} stands on {:?}, not it", def.floors);
            }
        }
    }

    /// A run's biomes are a pure function of its seed: building floor five
    /// before floor two, or the other way round, never changes what either
    /// floor draws.
    #[test]
    fn the_biome_draw_is_the_same_whichever_floor_is_built_first() {
        for s in 0..10 {
            let a = Undermere::new(RunSeed(s));
            a.build(map_of(5), None).unwrap();
            a.build(map_of(2), None).unwrap();
            let b = Undermere::new(RunSeed(s));
            b.build(map_of(2), None).unwrap();
            b.build(map_of(5), None).unwrap();
            for floor in 1..=FLOORS {
                assert_eq!(a.biome_of(floor).name, b.biome_of(floor).name, "seed {s}, floor {floor}");
            }
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
        let town = undermere.build(TOWN, None).unwrap();
        let under = |x: i32, y: i32| name(&town, Point::new(x, y));
        assert_eq!(under(25, 4), "planks", "a table in the tavern");
        assert_eq!(under(20, 3), "planks", "a counter among counters and barrels");
        assert_eq!(under(46, 4), "planks", "the smithy's anvil");
        assert_eq!(under(35, 12), "cobbles", "the well in the square");
        assert_eq!(under(40, 7), "grass", "a lamp post on the verge");
        assert_eq!(name(&town, town.entry), "cobbles", "where a run arrives");
        let throne = undermere.build(map_of(FLOORS), None).unwrap();
        for glyph in ['K', 'T'] {
            let spot = throne.spots.iter().find(|s| s.tag == glyph as u32).unwrap_or_else(|| panic!("the throne marks a '{glyph}'"));
            assert_eq!(name(&throne, spot.at), "dais", "'{glyph}' stands on the dais");
        }
    }

    /// Nothing a piece stands at its post is nearer the way in than the
    /// population keeps, over a span of seeds: the way in is drawn clear of
    /// every piece stamped before it, and on the last floor as far from the
    /// throne as the floor allows.
    #[test]
    fn no_monster_a_piece_stands_is_within_the_populations_distance_of_the_way_in() {
        let mut measured = 0;
        for s in 0..60 {
            let undermere = Undermere::new(RunSeed(s));
            for map in places() {
                let built = undermere.build(map, None).unwrap();
                for spot in built.spots.iter().filter(|spot| matches!(slot(&undermere, spot), Some(Slot::Monster { .. }))) {
                    let d = geometry::chebyshev(spot.at, built.entry);
                    assert!(d >= MIN_DISTANCE_FROM_ENTRY, "{map:?}, seed {s}: a monster's post at {:?} is {d} from the way in", spot.at);
                    measured += 1;
                }
            }
        }
        assert!(measured > 0, "some monster's post was measured");
    }

    /// A throne or a lair too large for the floor it is asked to grow on
    /// is refused by name, for `run_retrying` to try again rather than
    /// lay it shut away from the rest of the floor for the walk test to
    /// find later, or never: `PrefabShape` and `Accrete` are the engine's
    /// own, covered end to end in `rl_mapgen::accrete`'s own suite, so
    /// this only pins that Undermere's own pieces are among them.
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
        for s in 0..60 {
            let undermere = Undermere::new(RunSeed(s));
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

    /// Every lair and the throne, grown as `Accrete`'s pieces are, is
    /// entered only through the one entry its own prefab file marks: no
    /// other cell of its own wall ever opens onto something a body can
    /// walk through.
    #[test]
    fn every_lair_and_the_throne_is_entered_only_through_its_entries() {
        let mut measured = 0;
        for s in 0..40 {
            let undermere = Undermere::new(RunSeed(s));
            let tables = undermere.tiles().tables();
            for floor in 1..=FLOORS {
                let ctx = undermere.generate(map_of(floor)).unwrap();
                for area in ctx.outputs().iter::<Area>().filter(|a| a.entries.is_some()) {
                    let entries = area.entries.as_ref().expect("filtered on it");
                    for c in &area.cells {
                        for d in Direction::CARDINALS {
                            let n = *c + d.offset();
                            if !area.cells.contains(&n) && walkable(&ctx, &tables, n) {
                                assert!(entries.contains(&n), "seed {s} floor {floor}: {n:?} opens into {} and is no entry", area.shape);
                            }
                        }
                    }
                    measured += 1;
                }
            }
        }
        assert!(measured > 0, "some lair or the throne was measured");
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

    /// Every recipe grown on seed `s`, by a run of that seed so its lairs
    /// stand where that run's do, on one of the floors its biome stands
    /// on: within the forty attempts a floor is given, its requirements
    /// met, since `Require` is the chain's last pass; the size the recipe
    /// says; the way down outside every piece and at least the recipe's
    /// route, a third of the longer side or more, from the way in; every
    /// open cell walked to from the way in with every secret found and
    /// every lock opened, and the way down with none of them.
    fn every_recipe_meets_itself_on(s: u64) {
        let undermere = Undermere::new(RunSeed(s));
        let tables = undermere.tiles().tables();
        for recipe in &undermere.layout.recipes {
            let (floor, drawn) = grown_by(&undermere, &recipe.name, s);
            let at = format!("{} on floor {floor}, seed {s}", recipe.name);
            let ctx = undermere.grow(floor, &drawn, RunSeed(s), ATTEMPTS).unwrap_or_else(|e| panic!("{at}: {e}"));
            let bounds = ctx.terrain().bounds();
            assert_eq!((bounds.width, bounds.height), recipe.size, "{at}");
            let route = ctx.outputs().first::<rl_engine::rl_mapgen::Surveyed>().unwrap_or_else(|| panic!("{at}: not surveyed")).route;
            let pieces = pieces(&ctx);
            let built = PlaceBuild::from_context(ctx).unwrap_or_else(|e| panic!("{at}: {e}"));
            if floor == FLOORS {
                assert_eq!(built.exit, None, "{at}: the throne's floor has no way down");
            } else {
                let exit = built.exit.unwrap_or_else(|| panic!("{at}: no way down"));
                assert!(pieces.iter().all(|r| !r.contains(exit)), "{at}: the way down at {exit:?} is inside a piece");
                let route = route.unwrap_or_else(|| panic!("{at}: no route"));
                let third = bounds.width.max(bounds.height) as u32 / 3;
                assert!(route >= recipe.require.route && recipe.require.route >= third, "{at}: {route} steps down, asked {}", recipe.require.route);
            }
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
    }

    /// [`every_recipe_meets_itself_on`] over a few seeds, so the suite
    /// stays quick; the span the design asks for is the ignored test
    /// beside it.
    #[test]
    fn every_recipe_grows_within_forty_attempts_and_meets_itself_over_a_few_seeds() {
        for s in 0..6 {
            every_recipe_meets_itself_on(s);
        }
    }

    /// [`every_recipe_meets_itself_on`] over seeds 0 to 200, every recipe
    /// on each: some minutes, so run by hand with `--ignored` whenever
    /// `assets/layout.ron` or a pass of the floor chain changes.
    #[test]
    #[ignore = "seeds 0 to 200 of every recipe take minutes; run by hand when the layout or the chain changes"]
    fn every_recipe_grows_within_forty_attempts_and_meets_itself_over_seeds_0_to_200() {
        for s in 0..200 {
            every_recipe_meets_itself_on(s);
        }
    }

    /// Floor one is grown anew each run: no two of twenty seeds lay it out
    /// alike.
    #[test]
    fn floor_ones_layout_differs_between_any_two_of_twenty_seeds() {
        let floors: Vec<Terrain> = (0..20).map(|s| Undermere::new(RunSeed(s)).build(map_of(1), None).unwrap().terrain).collect();
        for i in 0..floors.len() {
            for j in i + 1..floors.len() {
                assert_ne!(floors[i], floors[j], "seeds {i} and {j} lay floor one out alike");
            }
        }
    }

    /// A tuning aid, not a property: for every recipe, or the one named by
    /// `SWEEP_RECIPE`, how many of `SWEEP_SEEDS` seeds (a hundred if unset)
    /// grow on their first attempt, how long one attempt takes, and why the
    /// rest were refused, numbers masked so like reasons count together.
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
            for s in 0..seeds {
                if let Err(e) = undermere.grow(floor, &drawn, RunSeed(s), 1) {
                    let masked: String = e.to_string().chars().map(|c| if c.is_ascii_digit() { '#' } else { c }).collect();
                    *why.entry(masked).or_default() += 1;
                }
            }
            let failed: u32 = why.values().sum();
            eprintln!("{recipe}: {} of {seeds} first attempts grow, {:?} an attempt", seeds - u64::from(failed), started.elapsed() / seeds as u32);
            let mut why: Vec<(String, u32)> = why.into_iter().collect();
            why.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            for (reason, n) in why.iter().take(4) {
                eprintln!("    {n:>4}  {reason}");
            }
        }
    }
}
