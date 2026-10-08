//! Every place a run walks through: Mereholt, the town on the black lake,
//! and the ten floors under it, each built the first time it is entered
//! and kept whole after that, as the engine keeps any place.
//!
//! The town is one hand-drawn piece, `assets/prefabs/mereholt.ron`,
//! stamped over a blank map of its own size, so the piece is the whole
//! town and every run comes to the same one. A floor is a chain of the
//! engine's passes, varied by its [`Biome`] so a band is told apart from
//! the first step onto it: the Upper Halls are rooms behind doors, the
//! Fungal Deeps caves grown by cellular automata, the Catacombs a
//! partition of crypts, the Drowned Cathedral halls with deep water
//! standing in them, the Forge of Embers caves with pools of lava, and the
//! Throne rooms with the throne laid at one end.
//!
//! A few passes are Undermere's own, each small and each here because the
//! engine's leave something a floor needs: caves joined the way a body
//! walks, water and lava that never cut a floor in two, furniture that
//! never shuts a way, a slot that stands on the floor drawn round it, and
//! the throne's hall, as far from it as the floor allows. Everything that
//! decides what a place is made of is in this module and nowhere else, so
//! a later part's generation changes this file and nothing that reads a
//! built place.
//!
//! [`PlaceBuild::from_context`] reads the way in, the way on and the
//! pieces' slots back out, the way Foundry's decks are read. What stands
//! at a slot is the engine's to put down; the ways through are
//! [`stairs`](crate::stairs)'s and the light is [`light`](crate::light)'s.

use std::collections::{BTreeMap, BTreeSet};

use rand::Rng;
use rl_engine::prelude::*;
use rl_engine::rl_core::Rect;
use rl_engine::rl_mapgen::dungeon::{Bsp, Doors, ExitPoint, FarthestExit, RandomStart, Room, Rooms};
use rl_engine::rl_mapgen::passes::{CellularCave, StartPoint};
use rl_engine::rl_render::TileAppearance;

use crate::bestiary::MonsterDef;

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

/// How large a floor is, in tiles: a little wider than the map's part of
/// the screen and as tall, so a floor scrolls a few tiles side to side and
/// never up and down.
const FLOOR_SIZE: (i32, i32) = (70, 40);

/// How much of a cave starts as rock, in percent, before the engine's
/// cellular automaton wears it away: enough that what is left is chambers
/// and the passages between them rather than an open field, which the
/// engine's own forty-five is. What splits off is joined back by
/// [`JoinCaves`].
const CAVE_FILL_PCT: u32 = 56;

/// How every tile looks, compiled in so the binary runs from anywhere.
const TILES_RON: &str = include_str!("../assets/tiles.ron");

/// The map id of floor `floor`; floors count from one.
pub fn map_of(floor: u32) -> MapId {
    MapId(floor)
}

/// The floor a map id is, or `None` for the town, which is no floor, and
/// for any id no place of the Undermere has.
pub fn floor_of(map: MapId) -> Option<u32> {
    (1..=FLOORS).contains(&map.0).then_some(map.0)
}

/// The band a floor belongs to, which decides how it is built, what it is
/// built of and what lights it.
///
/// A closed list, since the bands are the game's own and nothing outside
/// it adds one. Each band draws one biome in this part; the others the
/// design names for it are a later part's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Biome {
    /// Floors one and two: ruined masonry, rooms behind doors, rubble, and
    /// braziers left burning in a few of the rooms.
    UpperHalls,
    /// Floors three and four: caves of moss, lit by clumps of glowcaps.
    FungalDeeps,
    /// Floors five and six: crypts and ossuaries, bones, and the candles
    /// kept for the dead.
    Catacombs,
    /// Floors seven and eight: marble halls with the lake standing in them,
    /// deep in the middle and shallow at the edge, and shafts of pale
    /// light coming down through the water.
    DrownedCathedral,
    /// Floor nine: caves of ash round pools of lava, lit by nothing else.
    ForgeOfEmbers,
    /// Floor ten: rooms, and the throne's hall at one end of them.
    Throne,
}

impl Biome {
    /// Every biome, shallowest first.
    pub const ALL: [Biome; 6] = [Biome::UpperHalls, Biome::FungalDeeps, Biome::Catacombs, Biome::DrownedCathedral, Biome::ForgeOfEmbers, Biome::Throne];

    /// The biome of floor `floor`: one and two the halls, three and four the
    /// deeps, five and six the catacombs, seven and eight the cathedral,
    /// nine the forge and ten the throne. A number past either end reads as
    /// the nearer end, which no map id reaches.
    pub fn of(floor: u32) -> Biome {
        match floor {
            ..=2 => Biome::UpperHalls,
            3 | 4 => Biome::FungalDeeps,
            5 | 6 => Biome::Catacombs,
            7 | 8 => Biome::DrownedCathedral,
            9 => Biome::ForgeOfEmbers,
            _ => Biome::Throne,
        }
    }

    /// What the log calls it on arrival.
    pub fn name(self) -> &'static str {
        match self {
            Biome::UpperHalls => "the Upper Halls",
            Biome::FungalDeeps => "the Fungal Deeps",
            Biome::Catacombs => "the Catacombs",
            Biome::DrownedCathedral => "the Drowned Cathedral",
            Biome::ForgeOfEmbers => "the Forge of Embers",
            Biome::Throne => "the Throne",
        }
    }

    /// Its wall tile, which a floor of it is filled with before anything
    /// is carved.
    pub fn wall(self) -> &'static str {
        self.tiles()[0]
    }

    /// Its floor tile, what its rooms and caves are carved as.
    pub fn floor(self) -> &'static str {
        self.tiles()[1]
    }

    /// Every tile a floor of it is built of, its wall first and its floor
    /// second, so a band is told apart at a glance and none borrows
    /// another's. The doors the halls and the crypts hang are the one
    /// thing two bands share.
    ///
    /// Task 8a's own lairs carry their own tiles wherever their boss's
    /// range takes them, never the host band's, so a band whose floors a
    /// boss's own range reaches past its first lists that boss's lair
    /// tiles too, beside its own: Grubnak's hall over the Fungal Deeps
    /// and the Catacombs, the Widow's web over the same two, the
    /// Choirmaster's nave over the Catacombs and the Drowned Cathedral,
    /// the Abbess's chapel over the same two, and the Forgemaster's forge
    /// over the Drowned Cathedral and the Forge of Embers.
    pub fn tiles(self) -> &'static [&'static str] {
        match self {
            Biome::UpperHalls => &["hall wall", "hall floor", "rubble", "door", "open door"],
            Biome::FungalDeeps => &["cave wall", "moss", "glowcaps", "goblin hall wall", "goblin hall floor", "web"],
            Biome::Catacombs => &[
                "crypt wall",
                "crypt floor",
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
            ],
            Biome::DrownedCathedral => {
                &["cathedral wall", "marble", "flooded floor", "deep water", "moonlit marble", "crypt wall", "crypt floor", "forge wall", "ash", "lava"]
            }
            Biome::ForgeOfEmbers => &["forge wall", "ash", "lava"],
            Biome::Throne => &["throne wall", "throne floor", "dais", "black pool"],
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
            // The Upper Halls.
            TileProps::wall("hall wall"),
            TileProps::floor("hall floor"),
            TileProps::floor("rubble"),
            // The Fungal Deeps.
            TileProps::wall("cave wall"),
            TileProps::floor("moss"),
            TileProps::floor("glowcaps"),
            // The Catacombs.
            TileProps::wall("crypt wall"),
            TileProps::floor("crypt floor"),
            TileProps::floor("bones"),
            // The Drowned Cathedral.
            TileProps::wall("cathedral wall"),
            TileProps::floor("marble"),
            wading("flooded floor"),
            TileProps::named("deep water"),
            TileProps::floor("moonlit marble"),
            // The Forge of Embers.
            TileProps::wall("forge wall"),
            TileProps::floor("ash"),
            TileProps::named("lava"),
            // The Throne.
            TileProps::wall("throne wall"),
            TileProps::floor("throne floor"),
            TileProps::floor("dais"),
            wading("black pool"),
            // Task 8a's lairs: Grubnak's hall, since no band his own
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

/// Undermere's tiles, its pieces, and how each of its places is built.
pub struct Undermere {
    tiles: TileRegistry,
    prefabs: Prefabs<MonsterDef>,
    seed: RunSeed,
}

impl Undermere {
    /// Registers every tile a place is built of and loads every piece
    /// against them. The load reads the content registries and the bestiary
    /// itself, the same ones the run inserts, which is what makes a piece's
    /// prop and monster ids match the running game's.
    pub fn new(seed: RunSeed) -> Self {
        let tiles = registry();
        // After the tiles, which the pieces paint by name.
        let prefabs = crate::prefabs::load(&tiles);
        Self { tiles, prefabs, seed }
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
    /// can read a [`Stamped`] pass emitted before [`PlaceBuild::from_context`]
    /// flattens every mark into a `Spot` and drops the stamp's own bounds.
    fn generate(&self, map: MapId) -> Result<BaseContext, BuildError> {
        let (mut ctx, chain) = match (map, floor_of(map)) {
            (TOWN, _) => self.town()?,
            (_, Some(floor)) => self.floor(floor)?,
            (_, None) => return Err(BuildError::new("undermere", format!("{map:?} is no place in the Undermere"))),
        };
        // One stream per place, so building floor two first does not change
        // floor one.
        let seed = RunSeed(self.seed.derive(SeedDomain::new(b"undermere.place"), u64::from(map.0)));
        chain.run(&mut ctx, seed)?;
        Ok(ctx)
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

    /// Floor `floor`, as its biome builds it.
    ///
    /// Every floor but the last ends the same way: its way in drawn clear of
    /// whatever was stamped before it, its way on the cell farthest from
    /// that by walking, doors opened, and every slot then stood on the floor
    /// round it. The last is the throne's: its way in is as far from the
    /// throne as the floor allows, and it has no way on. Everything in
    /// between is the biome's.
    fn floor(&self, floor: u32) -> Result<(BaseContext, Chain<BaseContext>), BuildError> {
        let biome = Biome::of(floor);
        let t = |name: &str| self.tile(name);
        let (width, height) = FLOOR_SIZE;
        let ctx = BaseContext::blank(width, height, self.tiles.clone(), t(biome.wall()));
        let start = RandomStart.clear_of_stamps(MIN_DISTANCE_FROM_ENTRY);
        // A chest or two on every floor but the throne's, where the run ends.
        let chests = Furnish { name: "chests", piece: self.prefabs.piece("chest")?, count: (1, 2) };
        let chain = match biome {
            Biome::UpperHalls => {
                let hall = t("hall floor");
                let chain = Chain::new()
                    .then(Rooms { floor: hall, attempts: 40, min_size: 5, max_size: 11, min_rooms: 5 })
                    .then(Doors { door: t("door") })
                    .then(start)
                    .then(FarthestExit)
                    .then(Patches { name: "rubble", tile: t("rubble"), on: hall, count: (5, 9), size: (2, 6), phase: Phase::Finish })
                    .then(Furnish { name: "braziers", piece: self.prefabs.piece("brazier")?, count: (3, 5) })
                    .then(chests);
                // Floor one's way in is where the gate lets out, so its way
                // up is the gate's other side.
                let chain = if floor == 1 { chain.then(GateBack { gate: t("gate") }) } else { chain };
                chain.then(Underfoot)
            }
            Biome::FungalDeeps => {
                let (wall, moss) = (t("cave wall"), t("moss"));
                let chain = Chain::new().then(CellularCave { wall, floor: moss, fill_pct: CAVE_FILL_PCT, ..Default::default() });
                // Before `JoinCaves`, so the lair's own hall is just one
                // more pocket for it to join to the rest of the cave; a
                // cave band needs no corridor of its own.
                let chain = match self.lair_pass(floor, false)? {
                    Some(lair) => chain.then(lair),
                    None => chain,
                };
                chain
                    .then(JoinCaves { floor: moss, wall })
                    .then(start)
                    .then(FarthestExit)
                    .then(Patches { name: "glowcaps", tile: t("glowcaps"), on: moss, count: (8, 12), size: (2, 6), phase: Phase::Finish })
                    .then(chests)
                    .then(Underfoot)
            }
            Biome::Catacombs => {
                let crypt = t("crypt floor");
                let chain = Chain::new().then(Bsp { floor: crypt, min_leaf: 9, padding: 1 }).then(Doors { door: t("door") });
                // After the rooms and their doors, so the lair's own
                // corridor can find one clear of its hall to join.
                let chain = match self.lair_pass(floor, true)? {
                    Some(lair) => chain.then(lair),
                    None => chain,
                };
                chain
                    .then(start)
                    .then(FarthestExit)
                    .then(Patches { name: "bones", tile: t("bones"), on: crypt, count: (6, 10), size: (1, 4), phase: Phase::Finish })
                    .then(Furnish { name: "candles", piece: self.prefabs.piece("candelabra")?, count: (5, 8) })
                    .then(chests)
                    .then(Underfoot)
            }
            Biome::DrownedCathedral => {
                let (marble, deep, flooded) = (t("marble"), t("deep water"), t("flooded floor"));
                let chain = Chain::new().then(Rooms { floor: marble, attempts: 40, min_size: 6, max_size: 12, min_rooms: 4 }).then(Patches {
                    name: "pools",
                    tile: deep,
                    on: marble,
                    count: (4, 7),
                    size: (6, 24),
                    phase: Phase::Structures,
                });
                let chain = match self.lair_pass(floor, true)? {
                    Some(lair) => chain.then(lair),
                    None => chain,
                };
                chain
                    .then(start)
                    .then(FarthestExit)
                    .then(Rim { name: "shallows", tile: flooded, on: marble, around: deep, chance_pct: 70 })
                    .then(Patches { name: "puddles", tile: flooded, on: marble, count: (3, 6), size: (2, 6), phase: Phase::Finish })
                    .then(Patches { name: "shafts", tile: t("moonlit marble"), on: marble, count: (3, 5), size: (1, 1), phase: Phase::Finish })
                    .then(chests)
                    .then(Underfoot)
            }
            Biome::ForgeOfEmbers => {
                let (wall, ash) = (t("forge wall"), t("ash"));
                let chain = Chain::new().then(CellularCave { wall, floor: ash, fill_pct: CAVE_FILL_PCT, ..Default::default() });
                let chain = match self.lair_pass(floor, false)? {
                    Some(lair) => chain.then(lair),
                    None => chain,
                };
                chain
                    .then(JoinCaves { floor: ash, wall })
                    .then(Patches { name: "lava", tile: t("lava"), on: ash, count: (5, 8), size: (8, 30), phase: Phase::Connect })
                    .then(start)
                    .then(FarthestExit)
                    .then(chests)
                    .then(Underfoot)
            }
            // No way down: the throne is where the Undermere ends.
            Biome::Throne => {
                let floor = t("throne floor");
                Chain::new()
                    .then(Rooms { floor, attempts: 40, min_size: 5, max_size: 10, min_rooms: 4 })
                    .then(ThroneHall { piece: self.prefabs.piece("the throne")?, floor })
                    .then(FarthestFromTheThrone)
                    .then(Underfoot)
            }
        };
        Ok((ctx, chain))
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

/// The cell a body can stand on farthest from `from` by walking, doors
/// opened, leaving out `from` and whatever `skip` says; the first in row
/// order of those equally far, so a seed always picks the same one.
fn farthest(ctx: &BaseContext, from: Point, skip: impl Fn(Point) -> bool) -> Option<Point> {
    let tables = ctx.tiles().tables();
    let opened = Opened { terrain: ctx.terrain(), tables: &tables };
    let mut walk = DijkstraMap::covering(&opened);
    walk.build(&opened, [from], PathRules::default());
    walk.iter().filter(|(p, _)| *p != from && walkable(ctx, &tables, *p) && !skip(*p)).max_by_key(|(p, v)| (*v, std::cmp::Reverse(*p))).map(|(p, _)| p)
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

/// How many cells a pocket of cave needs to be worth a tunnel: anything
/// smaller is walled in.
const SMALLEST_POCKET: usize = 12;

/// Joins every region of a cave a body can walk across, one orthogonal
/// step at a time, to the largest, by the shortest tunnel through the rock,
/// and walls in a pocket too small to be worth one.
///
/// The engine's `KeepLargestRegion` walls in everything but the largest
/// region instead. Its cellular cave wears the rock away every round, so a
/// fill sparse enough to stay in one piece is an open field with pillars
/// in it, and at the fill that reads as a cave, chambers and the passages
/// between them, it splits into islands, and keeping one walled in half the
/// floor on some seeds. Joined, the cave keeps its whole extent. It also
/// joins a region by diagonals, where a body may not step diagonally between
/// two walls; this joins only what a body can walk.
struct JoinCaves {
    /// What a tunnel is cut as.
    floor: TileId,
    /// What a pocket too small to join is filled with.
    wall: TileId,
}

impl Pass<BaseContext> for JoinCaves {
    fn name(&self) -> &'static str {
        "join_caves"
    }

    fn phase(&self) -> Phase {
        Phase::Connect
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let tables = ctx.tiles().tables();
        loop {
            let terrain = ctx.terrain();
            let regions = rl_engine::rl_grid::region::label_regions(terrain, |i| tables.walkable[terrain.get_idx(i).index()], Steps::Four);
            let main = regions.largest().ok_or_else(|| BuildError::new("join_caves", "no open cell to keep"))?;
            let pockets: Vec<usize> =
                (0..terrain.len()).filter(|idx| regions.label_idx(*idx).is_some_and(|l| l != main && regions.size(l) < SMALLEST_POCKET)).collect();
            if !pockets.is_empty() {
                for idx in pockets {
                    ctx.terrain_mut().set_idx(idx, self.wall);
                }
                continue;
            }
            if regions.count() == 1 {
                return Ok(());
            }
            // Out from the largest region through anything but the map's
            // edge, which stays rock, to the nearest cell of another.
            let (width, height) = (terrain.width(), terrain.height());
            let inside = |p: Point| p.x > 0 && p.y > 0 && p.x < width - 1 && p.y < height - 1;
            let mut came: Vec<Option<usize>> = vec![None; terrain.len()];
            let mut seen = vec![false; terrain.len()];
            let mut queue = std::collections::VecDeque::new();
            for idx in (0..terrain.len()).filter(|idx| regions.label_idx(*idx) == Some(main)) {
                seen[idx] = true;
                queue.push_back(idx);
            }
            let mut reached = None;
            while let Some(idx) = queue.pop_front() {
                if regions.label_idx(idx).is_some_and(|l| l != main) {
                    reached = Some(idx);
                    break;
                }
                let p = terrain.idx_point(idx);
                for n in Direction::CARDINALS.map(|d| p + d.offset()).into_iter().filter(|n| inside(*n)) {
                    let next = terrain.checked_idx(n).expect("inside the map");
                    if !std::mem::replace(&mut seen[next], true) {
                        came[next] = Some(idx);
                        queue.push_back(next);
                    }
                }
            }
            let mut at = reached.ok_or_else(|| BuildError::new("join_caves", "a region no tunnel reaches"))?;
            while let Some(back) = came[at] {
                if !tables.walkable[ctx.terrain().get_idx(at).index()] {
                    ctx.terrain_mut().set_idx(at, self.floor);
                }
                at = back;
            }
        }
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

/// Edges `around` with `tile`: each cell of `on` beside it becomes `tile`
/// on a roll of `chance_pct`, so deep water has a ragged shallow edge, the
/// way the town's lake has its shallows.
struct Rim {
    /// A stable name, so two rims in one chain draw different streams.
    name: &'static str,
    /// What the edge is made of.
    tile: TileId,
    /// What the edge is laid over.
    on: TileId,
    /// What it edges.
    around: TileId,
    /// The chance for each cell beside it, in percent.
    chance_pct: u32,
}

impl Pass<BaseContext> for Rim {
    fn name(&self) -> &'static str {
        self.name
    }

    fn phase(&self) -> Phase {
        Phase::Finish
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let kept = ways(ctx);
        let terrain = ctx.terrain();
        let edge: Vec<Point> = terrain
            .iter()
            .filter(|(p, t)| *t == self.on && !kept.contains(p) && geometry::square(*p, 1).any(|q| terrain.get(q) == Some(self.around)))
            .map(|(p, _)| p)
            .collect();
        for p in edge {
            if ctx.rng().random_range(0..100) < self.chance_pct {
                ctx.terrain_mut().set(p, self.tile);
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

/// Where the throne's mouth lets out, emitted by [`ThroneHall`] for the
/// way in to be measured from.
struct Mouth(Point);

/// Lays the throne at the north end of floor ten, after the rooms are cut
/// and before the way in is chosen: a hall a cell larger than the piece all
/// round, carved over whatever the rooms put there, the piece in the middle
/// of it facing south down the floor, and a corridor from under its mouth
/// to the nearest room clear of the hall. A floor with no room clear of
/// the hall is refused, since nothing would reach the throne.
///
/// The ordinary rooms are never large enough for the throne, so it does not
/// wait for one. The hall's ring of floor is what keeps the floor whole: a
/// room or corridor the hall cut through meets the ring, and the ring goes
/// round the piece.
struct ThroneHall {
    /// The throne, as drawn: its mouth in the middle of its bottom row.
    piece: Prefab,
    /// What the hall is floored with.
    floor: TileId,
}

impl Pass<BaseContext> for ThroneHall {
    fn name(&self) -> &'static str {
        "throne_hall"
    }

    fn phase(&self) -> Phase {
        Phase::Structures
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let (w, h) = (self.piece.width(), self.piece.height());
        let bounds = ctx.terrain().bounds();
        // The hall, and a row of wall all round it at the map's edge.
        if w + 4 > bounds.width || h + 5 > bounds.height {
            return Err(BuildError::new("throne_hall", format!("a {w}x{h} throne does not fit a {}x{} floor", bounds.width, bounds.height)));
        }
        let x = ctx.rng().random_range(1..=bounds.width - w - 3);
        let hall = Rect::new(x, 1, w + 2, h + 2);
        for p in hall.cells() {
            ctx.terrain_mut().set(p, self.floor);
        }
        StampPrefab { name: "throne", prefab: self.piece.clone(), at: Placement::At(Point::new(hall.x + 1, hall.y + 1)), orient: Orient::Fixed }.apply(ctx)?;
        let mouth = Point::new(hall.x + 1 + w / 2, hall.y + 1 + h);
        let under = hall.bottom();
        let nearest = ctx
            .outputs()
            .iter::<Room>()
            .map(|room| room.0)
            .filter(|room| room.intersection(&hall).is_none())
            .min_by_key(|room| (geometry::manhattan(room.center(), mouth), room.center()));
        // Refused rather than laid shut away: a hall with nothing to join
        // its mouth to is a floor the walk to the throne cannot cross, and
        // a change to the floor's size or the throne's that brings it about
        // should fail where it happens.
        let room = nearest.ok_or_else(|| BuildError::new("throne_hall", format!("no room clear of the {w}x{h} throne's hall to join its mouth to")))?;
        // Straight down out of the hall, along the row under it, then up
        // or down into the room: never back into the hall.
        let to = room.center();
        let corridor = (mouth.y..=under)
            .map(|y| Point::new(mouth.x, y))
            .chain((mouth.x.min(to.x)..=mouth.x.max(to.x)).map(|x| Point::new(x, under)))
            .chain((under.min(to.y)..=under.max(to.y)).map(|y| Point::new(to.x, y)))
            .collect::<Vec<_>>();
        for p in corridor {
            ctx.terrain_mut().set(p, self.floor);
        }
        ctx.emit(Mouth(mouth));
        Ok(())
    }
}

/// Emits as the way in the cell farthest by walking from the throne's
/// mouth, so the throne is where the floor ends and the walk to it is the
/// whole of floor ten.
struct FarthestFromTheThrone;

impl Pass<BaseContext> for FarthestFromTheThrone {
    fn name(&self) -> &'static str {
        "farthest_from_the_throne"
    }

    fn phase(&self) -> Phase {
        Phase::Exits
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let mouth = ctx.outputs().first::<Mouth>().ok_or_else(|| BuildError::new("farthest_from_the_throne", "no throne laid"))?.0;
        let pieces = pieces(ctx);
        let far = farthest(ctx, mouth, |p| pieces.iter().any(|r| r.contains(p)))
            .ok_or_else(|| BuildError::new("farthest_from_the_throne", "nowhere to walk to from the throne"))?;
        ctx.emit(StartPoint(far));
        Ok(())
    }
}

/// One of the five mini-bosses a run may draw a lair for: task 8a's own.
/// Its name is `monsters.ron`'s and its lair's own `assets/prefabs/`
/// file's, its floors the span it may stand on, and its margin the tile
/// its lair's hall is floored with outside the piece's own footprint,
/// chosen to match the band most of its own floors fall in; only
/// Grubnak's is new, since no band his own floors fall in is a hall.
pub(crate) struct MiniBoss {
    /// The name both `monsters.ron` and the prefab's `name:` field use.
    pub(crate) name: &'static str,
    /// The shallowest and the deepest floor it may stand on, both
    /// included: `content-sheet-monsters.md`'s own "Mini-bosses" row.
    pub(crate) floors: (u32, u32),
    /// What floors the margin round its lair's own piece.
    margin: &'static str,
}

/// The five, in the content sheet's own order. `pub(crate)` so
/// `population.rs`'s own tests can read a boss's name and floors without
/// a second list of them to keep in step.
pub(crate) const MINI_BOSSES: [MiniBoss; 5] = [
    MiniBoss { name: "Grubnak the goblin king", floors: (3, 5), margin: "goblin hall floor" },
    MiniBoss { name: "the Widow", floors: (4, 6), margin: "moss" },
    MiniBoss { name: "the Choirmaster", floors: (5, 7), margin: "crypt floor" },
    MiniBoss { name: "the Drowned Abbess", floors: (6, 8), margin: "flooded floor" },
    MiniBoss { name: "the Forgemaster", floors: (7, 9), margin: "ash" },
];

impl Undermere {
    /// The two mini-bosses this run draws, and which floor each stands
    /// on, by index into [`MINI_BOSSES`]: never the same boss twice, each
    /// on a floor of its own within its own range. Pure over `self.seed`,
    /// so every call during one run agrees without keeping the draw
    /// anywhere, the same reasoning `generate`'s own per-place stream
    /// follows.
    pub(crate) fn lairs(&self) -> [(usize, u32); 2] {
        let mut rng = self.seed.rng(SeedDomain::new(b"undermere.lairs"), 0);
        let mut order: Vec<usize> = (0..MINI_BOSSES.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, rng.random_range(0..=i));
        }
        let mut chosen: Vec<(usize, u32)> = Vec::new();
        for &boss in order.iter().take(2) {
            let (lo, hi) = MINI_BOSSES[boss].floors;
            let mut floor = rng.random_range(lo..=hi);
            // Its range is always at least three floors wide and only one
            // other boss was drawn before it, so there is always a floor
            // in its own range the other did not take.
            if chosen.iter().any(|(_, f)| *f == floor) {
                floor = (lo..=hi).find(|f| chosen.iter().all(|(_, g)| g != f)).unwrap_or(floor);
            }
            chosen.push((boss, floor));
        }
        [chosen[0], chosen[1]]
    }

    /// The mini-boss and its lair's own margin tile this run stands on
    /// `floor`, if any does.
    fn lair_on(&self, floor: u32) -> Option<(&'static MiniBoss, TileId)> {
        self.lairs().iter().find(|(_, f)| *f == floor).map(|(boss, _)| (&MINI_BOSSES[*boss], self.tile(MINI_BOSSES[*boss].margin)))
    }

    /// The [`Lair`] pass `floor`'s own chain stamps, if this run draws one
    /// there: the piece named exactly as its boss is in `monsters.ron`,
    /// so the one name serves both. `connect` is the caller's: a room
    /// band's own, since `JoinCaves` reads a closed door as a wall and
    /// would as readily wall a room behind one in as join it, which only
    /// a cave band, with no doors to misread, may ask this to leave to it.
    fn lair_pass(&self, floor: u32, connect: bool) -> Result<Option<Lair>, BuildError> {
        match self.lair_on(floor) {
            Some((boss, margin)) => Ok(Some(Lair { name: boss.name, piece: self.prefabs.piece(boss.name)?, margin, connect })),
            None => Ok(None),
        }
    }
}

/// Lays a mini-boss's lair somewhere midway through the floor's own
/// generation: a hall a cell larger than the piece all round, carved
/// over whatever stood there, the piece stamped fixed in its centre, as
/// [`ThroneHall`] lays the throne. A cave band's own `JoinCaves`, which
/// runs right after this, joins the hall to the rest of the floor the
/// same way it joins off any cut-off pocket of its own cave; a room band
/// cannot be handed to `JoinCaves` the same way, since a closed door
/// reads to it as a wall and a room reached by one alone as a pocket to
/// wall in rather than join, so `connect` asks this to join itself
/// instead, by a corridor to the nearest room clear of the hall, exactly
/// as the throne's own hall does.
struct Lair {
    /// A stable name, so two lairs, or the throne, draw different streams.
    name: &'static str,
    /// The piece.
    piece: Prefab,
    /// What the hall is floored with outside the piece's own footprint.
    margin: TileId,
    /// Whether to carve its own corridor to the nearest room: a room
    /// band needs one; a cave band's `JoinCaves` already joins whatever
    /// this leaves disconnected.
    connect: bool,
}

impl Pass<BaseContext> for Lair {
    fn name(&self) -> &'static str {
        self.name
    }

    fn phase(&self) -> Phase {
        Phase::Structures
    }

    fn apply(&self, ctx: &mut BaseContext) -> Result<(), BuildError> {
        let (w, h) = (self.piece.width(), self.piece.height());
        let bounds = ctx.terrain().bounds();
        // The hall, a row of wall all round it at the map's edge, and,
        // where it must connect itself, one more row below for the
        // corridor out of its mouth.
        if w + 4 > bounds.width || h + 5 > bounds.height {
            return Err(BuildError::new(self.name, format!("a {w}x{h} lair does not fit a {}x{} floor", bounds.width, bounds.height)));
        }
        let x = ctx.rng().random_range(1..=bounds.width - w - 3);
        let y = ctx.rng().random_range(1..=bounds.height - h - 4);
        let hall = Rect::new(x, y, w + 2, h + 2);
        for p in hall.cells() {
            ctx.terrain_mut().set(p, self.margin);
        }
        StampPrefab { name: self.name, prefab: self.piece.clone(), at: Placement::At(Point::new(hall.x + 1, hall.y + 1)), orient: Orient::Fixed }.apply(ctx)?;
        if !self.connect {
            return Ok(());
        }
        let mouth = Point::new(hall.x + 1 + w / 2, hall.y + 1 + h);
        let under = hall.bottom();
        let nearest = ctx
            .outputs()
            .iter::<Room>()
            .map(|room| room.0)
            .filter(|room| room.intersection(&hall).is_none())
            .min_by_key(|room| (geometry::manhattan(room.center(), mouth), room.center()));
        let room = nearest.ok_or_else(|| BuildError::new(self.name, format!("no room clear of the {w}x{h} lair's hall to join its mouth to")))?;
        let to = room.center();
        let corridor = (mouth.y..=under)
            .map(|y| Point::new(mouth.x, y))
            .chain((mouth.x.min(to.x)..=mouth.x.max(to.x)).map(|x| Point::new(x, under)))
            .chain((under.min(to.y)..=under.max(to.y)).map(|y| Point::new(to.x, y)))
            .collect::<Vec<_>>();
        for p in corridor {
            ctx.terrain_mut().set(p, self.margin);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rl_engine::rl_core::Rect;
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

    #[test]
    fn floor_ten_holds_the_throne_and_the_king() {
        let library = crate::content::Library::load(&crate::content::effect_kinds(), &Moments::default());
        for s in 0..40 {
            let undermere = Undermere::new(RunSeed(s));
            let key = undermere.prefabs().defs().expect("the throne").raw();
            let ctx = undermere.generate(map_of(FLOORS)).unwrap();
            let throne: Vec<Rect> = ctx.outputs().iter::<Stamped>().filter(|st| st.prefab == Some(key)).map(|st| st.bounds).collect();
            assert_eq!(throne.len(), 1, "seed {s}: one throne on floor ten");
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
                assert!(throne.contains(spot.at), "seed {s}: a slot at {:?} outside the throne", spot.at);
            }
            monsters.sort_unstable();
            props.sort_unstable();
            assert_eq!(monsters, ["skeleton knight"; 5].into_iter().chain(["the Drowned King"]).collect::<Vec<_>>(), "seed {s}: the king and his court");
            assert_eq!(props, ["brazier", "brazier", "brazier", "brazier", "throne"], "seed {s}");
            // The way in is as far from the throne's mouth, walking, as any
            // cell of the floor outside the throne.
            let view = built.terrain.view(undermere.tiles());
            let mut walk = DijkstraMap::covering(&view);
            let mouth = Point::new(throne.x + throne.width / 2, throne.bottom());
            walk.build(&view, [mouth], PathRules::default());
            let farthest = walk.iter().filter(|(p, _)| !throne.contains(*p)).map(|(_, v)| v).max().unwrap();
            assert_eq!(walk.value(built.entry), Some(farthest), "seed {s}: the way in at {:?} is not the farthest from the throne", built.entry);
            let clear = geometry::chebyshev(built.entry, geometry::clamp_to(built.entry, throne));
            assert!(clear >= MIN_DISTANCE_FROM_ENTRY, "seed {s}: the way in is {clear} from the throne");
        }
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
    fn each_biome_draws_its_own_walls_and_floors() {
        for (i, a) in Biome::ALL.iter().enumerate() {
            for b in &Biome::ALL[i + 1..] {
                let (theirs, ours) = ([a.wall(), a.floor()], [b.wall(), b.floor()]);
                assert!(theirs.iter().all(|t| !ours.contains(t)), "{} and {} share a wall or a floor", a.name(), b.name());
            }
        }
        for s in 0..20 {
            let undermere = Undermere::new(RunSeed(s));
            for floor in 1..=FLOORS {
                let biome = Biome::of(floor);
                let built = undermere.build(map_of(floor), None).unwrap();
                let drawn: BTreeSet<&str> = built.terrain.iter().map(|(_, t)| undermere.tiles().get(t).name.as_str()).collect();
                assert!(drawn.contains(biome.wall()) && drawn.contains(biome.floor()), "floor {floor}, seed {s}: {drawn:?}");
                // The gate lets out on floor one: its other side.
                let allowed: BTreeSet<&str> = biome.tiles().iter().copied().chain((floor == 1).then_some("gate")).collect();
                let strays: Vec<&&str> = drawn.difference(&allowed).collect();
                assert!(strays.is_empty(), "floor {floor}, seed {s}: {strays:?} on a floor of {}", biome.name());
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
    /// of a room or a cave reads at a glance even in the dark. Every band's
    /// wall has a ground lighter than each of its floors, the doors hung in
    /// it and the tiles that glow aside: in every channel, so it is the
    /// lighter under any light that lands on both, and by a margin the eye
    /// reads, in full light and remembered alike.
    #[test]
    fn every_biomes_walls_are_lighter_than_its_floors() {
        let undermere = Undermere::new(RunSeed(0));
        let look = undermere.appearance();
        let glows = |name: &str| crate::light::GLOWING.iter().any(|(glowing, _)| *glowing == name);
        // A tile that is itself some band's own wall, task 8a's lairs
        // listing a foreign one beside their floors, reads against that
        // band's own floor elsewhere in this very loop and is never asked
        // to read as a floor here, which two walls' colours were never
        // chosen against each other to do. Every wall in this file is
        // named for it, `"... wall"`, which is what this reads rather
        // than a second list of them to keep in step with `tiles.ron`.
        for biome in Biome::ALL {
            for floor in biome.tiles().iter().filter(|t| **t != biome.wall() && !["door", "open door"].contains(*t) && !glows(t) && !t.ends_with("wall")) {
                reads_against(&undermere, &look, biome.name(), biome.wall(), floor);
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
        let undermere = Undermere::new(RunSeed(0));
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

    /// A throne hall laid where no room is clear of it to join its mouth
    /// to is refused, naming the pass, rather than laid shut away from the
    /// rest of the floor for the walk test to find later, or never.
    #[test]
    fn a_throne_with_no_room_to_join_its_mouth_to_is_refused() {
        let undermere = Undermere::new(RunSeed(0));
        let floor = undermere.tile("throne floor");
        let mut ctx = BaseContext::blank(70, 40, undermere.tiles().clone(), undermere.tile("throne wall"));
        let hall = ThroneHall { piece: undermere.prefabs().piece("the throne").unwrap(), floor };
        let refused = Chain::new().then(hall).run(&mut ctx, RunSeed(0)).expect_err("no rooms, so nothing to join the hall to");
        assert_eq!(refused.pass, "throne_hall", "{refused}");
    }

    /// Task 8a's own draw: two mini-bosses a run, never the same one
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
}
