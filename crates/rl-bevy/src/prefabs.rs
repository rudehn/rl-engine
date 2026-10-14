//! Prefabs: what a place's prefab slots hold, filled on the arrival that
//! built it.
//!
//! A prefab file names, glyph by glyph, a tile or a slot: a prop, items or
//! a monster ([`rl_rules::prefab`]). The terrain half is stamped by the
//! game's mapgen chain from [`Prefabs::piece`], keyed so each mark of the
//! stamp remembers which prefab it came from, and arrives here as a
//! place's [`Spot`](crate::places::Spot)s, or grown as an area of its own
//! from [`Prefabs::grown`], whose stamp arrives the same way. A dressing
//! ([`rl_rules::dressing`]) is a prefab's legend laid by rules rather
//! than rows, by [`Dress`](rl_mapgen::Dress) from [`Prefabs::dressing`],
//! and its slots arrive and fill the same way again. What is here is the
//! other half, the loop every game wrote by hand at its marks:
//!
//! - **When**: on the arrival that built the place, and never again, so a
//!   revisit finds what it left.
//! - **From which stream**: one derived for each slot's own cell, so a
//!   slot filled differently, or a prefab changed, never moves what is
//!   drawn at another.
//! - **What is skipped**: a slot draws first and then decides. On the
//!   arrival cell, on a cell a later pass walled over, or on one another
//!   slot already filled this arrival, it spawns nothing, and nothing
//!   anywhere else moves because of it.
//!
//! What a monster and an item *are* stays the game's: it implements
//! [`ActorMaker`] and [`ItemMaker`] on the resources that hold them, and
//! the engine asks them to make what it decided. A [`Slot::Mark`] is the
//! game's too, found among the spots by its glyph, and so is every spot of
//! a piece that was not keyed. A monster placed at a slot holds that cell
//! as its [`Post`]. `docs/design/prefabs.md` has the reasoning.

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::sync::Arc;

use bevy::prelude::*;
use rand::Rng;
use rand::rngs::StdRng;
use rl_core::seed::position_hash;
use rl_core::{Id, Point};
use rl_mapgen::BuildError;
use rl_mapgen::accrete::Join;
use rl_mapgen::dress::{Dressing, Spread, Stand, Thing};
use rl_mapgen::prefab::{Cell, Mirror, Prefab, PrefabShape, Turn};
use rl_rules::content::{BandedTable, Registry};
use rl_rules::dressing::DressingDef;
use rl_rules::prefab::{self, Legended, Pick, PrefabDef, Slot};
use rl_rules::prop::Stock;
use rl_rules::role::{self, RoleDef};

use crate::loot::{Found, ItemMaker, LootArea, Provenance, draw_stock, lay, stream_for};
use crate::minds::Post;
use crate::places::{MapId, PlaceEntered};
use crate::props::spawn_prop;
use crate::registries::Registries;
use crate::seed::Seed;
use crate::world::WorldMap;

/// A game's monster registry, as the engine asks it for things.
///
/// Implemented on the resource holding a game's monster definitions, the
/// one place that knows what a monster is. The engine decides when, where
/// and which, and asks this to make it. A game's own population should
/// build through the same builder `make` uses, as Foundry's `make` wraps
/// the `spawn_monster` its decks populate by, so a monster placed at a
/// slot and one placed by the game are built the same way and cannot
/// drift apart.
pub trait ActorMaker: Resource {
    /// The game's monster definition.
    type Def: Send + Sync + 'static;

    /// Makes one `def` standing at `at` on `map`, and returns it. `rng`
    /// is the engine's stream for this slot, for whatever the game rolls
    /// on the monster itself.
    fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<Self::Def>, at: Point, map: MapId, rng: &mut StdRng) -> Entity;

    /// Where each monster turns up, which a role draws from.
    fn table(&self) -> &BandedTable<Id<Self::Def>>;

    /// How deep, far or dangerous `map` is, in the numbering the table is
    /// written in.
    fn band(&self, map: MapId) -> i32;
}

/// Every prefab and dressing a game loaded and the roles their monster
/// slots name.
struct Loaded<M> {
    defs: Registry<PrefabDef<M>>,
    roles: Registry<RoleDef<M>>,
    dressings: Registry<DressingDef<M>>,
}

/// The bit a dressing's key carries and a prefab's never does, so one
/// number on a spot names either and the two registries never collide:
/// a game has far fewer than two billion prefabs.
const DRESSING: u32 = 1 << 31;

/// A game's prefabs and roles, keyed by the registry id a stamped piece
/// carries.
///
/// Shared, not copied: a game's place builder holds a clone to stamp
/// pieces from and the engine reads the same files to fill them, so the
/// key a stamp carries always names the prefab it was cut from.
#[derive(Resource)]
pub struct Prefabs<M: Send + Sync + 'static>(Arc<Loaded<M>>);

impl<M: Send + Sync + 'static> Clone for Prefabs<M> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<M: Send + Sync + 'static> Prefabs<M> {
    /// Every prefab, loaded with [`rl_rules::prefab::load`], and every
    /// role, loaded with [`rl_rules::role::load`].
    pub fn new(defs: Registry<PrefabDef<M>>, roles: Registry<RoleDef<M>>) -> Self {
        Self::with_dressings(defs, roles, Registry::default())
    }

    /// As [`new`](Self::new), with the dressings loaded with
    /// [`rl_rules::dressing::load`], whose slots then fill as a prefab's.
    pub fn with_dressings(defs: Registry<PrefabDef<M>>, roles: Registry<RoleDef<M>>, dressings: Registry<DressingDef<M>>) -> Self {
        Self(Arc::new(Loaded { defs, roles, dressings }))
    }

    /// Every dressing.
    pub fn dressings(&self) -> &Registry<DressingDef<M>> {
        &self.0.dressings
    }

    /// Every prefab.
    pub fn defs(&self) -> &Registry<PrefabDef<M>> {
        &self.0.defs
    }

    /// Every role.
    pub fn roles(&self) -> &Registry<RoleDef<M>> {
        &self.0.roles
    }

    /// The piece called `name`, keyed so its stamp's marks find their slots
    /// again: each tile as the file paints it, each slot a mark painted with
    /// the prefab's ground, and declaring the turns and mirrors its file
    /// allows, so no stamp lays it a way its drawing refuses. An error
    /// naming the piece when there is none, so a chain can `?` it.
    pub fn piece(&self, name: &str) -> Result<Prefab, BuildError> {
        let id = self.defs().id(name).ok_or_else(|| BuildError::new("prefab", format!("no prefab is called {name:?}")))?;
        let def = self.defs().get(id);
        let cell = |c: char| match (def.tile(c), def.slot(c)) {
            (Some(t), _) => Cell::Tile(t),
            (None, Some(_)) => Cell::Mark(def.ground),
            (None, None) if def.entry(c) => Cell::Entry,
            (None, None) => Cell::Clear,
        };
        let turn = match def.turn {
            prefab::Turn::None => Turn::None,
            prefab::Turn::Half => Turn::Half,
            prefab::Turn::Quarter => Turn::Quarter,
        };
        let mirror = match def.mirror {
            prefab::Mirror::None => Mirror::None,
            prefab::Mirror::LeftRight => Mirror::LeftRight,
            prefab::Mirror::UpDown => Mirror::UpDown,
            prefab::Mirror::Both => Mirror::Both,
        };
        Prefab::parse_cells(&def.rows(), cell).map(|p| p.keyed(id.raw()).declaring(turn, mirror)).map_err(|e| BuildError::new("prefab", format!("{name}: {e}")))
    }

    /// The piece called `name` as a shape [`Accrete`](rl_mapgen::accrete::Accrete)
    /// grows as an area of its own, joined by `join` through the entries its
    /// file draws, and keyed as [`piece`](Self::piece) keys it, so its
    /// slots fill as a stamped piece's do. An error naming the piece when
    /// there is none or it has no entry.
    pub fn grown(&self, name: &str, join: Join) -> Result<PrefabShape, BuildError> {
        PrefabShape::new(name, &self.piece(name)?, join)
    }

    /// The dressing called `name`, as [`Dress`](rl_mapgen::Dress) lays it:
    /// each glyph of a rule a tile when its legend paints one and a slot
    /// otherwise, keyed so the slots it emits find their meaning again. An
    /// error naming it when there is none, so a chain can `?` it.
    pub fn dressing(&self, name: &str) -> Result<Dressing, BuildError> {
        let id = self.dressings().id(name).ok_or_else(|| BuildError::new("dressing", format!("no dressing is called {name:?}")))?;
        let def = self.dressings().get(id);
        let things = def
            .things
            .iter()
            .map(|t| Thing {
                glyph: t.glyph,
                stands: def.legend().tile(t.glyph).map_or(Stand::Slot, Stand::Tile),
                at: match t.at {
                    rl_rules::dressing::Spread::Walls => Spread::Walls,
                    rl_rules::dressing::Spread::Corners => Spread::Corners,
                    rl_rules::dressing::Spread::Center => Spread::Center,
                    rl_rules::dressing::Spread::Rows { aisle } => Spread::Rows { aisle },
                    rl_rules::dressing::Spread::Scatter => Spread::Scatter,
                },
                density_pct: t.density_pct,
            })
            .collect();
        Ok(Dressing { name: def.name.clone(), things, repaint: def.repaint, key: Some(DRESSING | id.raw()) })
    }

    /// What `glyph` stands for in the prefab or the dressing keyed `key`,
    /// if the key names one and the glyph a slot in it.
    pub fn slot(&self, key: u32, glyph: char) -> Option<&Slot<M>> {
        if key & DRESSING != 0 {
            return self.dressings().try_get(Id::from_raw(key & !DRESSING))?.legend().slot(glyph);
        }
        self.defs().try_get(Id::from_raw(key))?.slot(glyph)
    }
}

/// Where a place's prefab slots are filled, inside
/// [`TurnSet::React`](crate::plugin::TurnSet::React) and before
/// [`LootSet::Scatter`](crate::loot::LootSet::Scatter), so loot never lands
/// under a prefab's prop.
///
/// A game that builds a place in the same pass orders what it spawns after
/// [`PrefabSet::Fill`], so its own population can keep off the slots, and
/// before `LootSet::Scatter`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrefabSet {
    /// Every keyed slot of a place, on the arrival that built it.
    Fill,
}

/// What filling slots reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Filling<'w, A, I>
where
    A: ActorMaker,
    I: ItemMaker,
{
    prefabs: Res<'w, Prefabs<<A as ActorMaker>::Def>>,
    actors: Res<'w, A>,
    items: Res<'w, I>,
    registries: Res<'w, Registries>,
    map: Res<'w, WorldMap>,
    seed: Res<'w, Seed>,
}

/// What one slot drew, before it is decided whether it is spawned.
enum Drawn<A, I> {
    Prop(rl_rules::prop::PropId),
    Items(Vec<(Id<I>, u32)>, i32),
    Monster(Id<A>),
    Nothing,
}

/// Fills every keyed spot of a place on the arrival that built it: a prop,
/// items or a monster, whatever the prefab's slot says, each drawn from a
/// stream derived for its own cell, so nothing that happens at one slot
/// moves what is drawn at another. A slot draws first and then decides
/// whether to spawn: on the arrival cell, on a cell no longer walkable,
/// or on one this pass already filled, it spawns nothing. An item slot
/// whose count rolls nought lays nothing and leaves its cell unfilled.
pub fn fill_prefabs<A, I>(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, fill: Filling<A, I>)
where
    A: ActorMaker,
    I: ItemMaker,
{
    let Filling { prefabs, actors, items, registries, map, seed } = &fill;
    for ev in entered.read().filter(|ev| ev.first) {
        let Some(place) = map.place(ev.map) else { continue };
        let mut filled = BTreeSet::new();
        for spot in &place.spots {
            let Some(key) = spot.prefab else { continue };
            let Some(slot) = char::from_u32(spot.tag).and_then(|c| prefabs.slot(key, c)) else { continue };
            let mut rng = stream_for(seed, b"prefab.content", position_hash(u64::from(ev.map.0), spot.at.x, spot.at.y));
            let drawn: Drawn<A::Def, I::Def> = match slot {
                Slot::Mark => continue,
                Slot::Prop(id) => Drawn::Prop(*id),
                Slot::Item(roll) => {
                    let count = if roll.min >= roll.max { roll.max } else { rng.random_range(roll.min..=roll.max) };
                    let band = items.band(LootArea::Place(ev.map)) + roll.band;
                    Drawn::Items(draw_stock(&**items, &roll.what, count, band, &mut rng), band)
                }
                Slot::Monster { pick, band } => match pick {
                    Pick::Kind(id) => Drawn::Monster(*id),
                    Pick::Role(role) => {
                        let band = actors.band(ev.map) + band;
                        role::draw(actors.table(), prefabs.roles().get(*role), band, &mut rng).map_or(Drawn::Nothing, Drawn::Monster)
                    }
                },
            };
            let walkable = place.terrain.get(spot.at).is_some_and(|t| map.tables().walkable[t.index()]);
            if spot.at == ev.entry || !walkable || filled.contains(&spot.at) {
                continue;
            }
            match drawn {
                Drawn::Prop(id) => {
                    spawn_prop(&mut commands, registries, id, spot.at, ev.map);
                }
                Drawn::Items(made, band) if !made.is_empty() => {
                    for (def, count) in made {
                        lay(&mut commands, &**items, registries, (def, count, spot.at), ev.map, Provenance { found: Found::Placed, band }, &mut rng);
                    }
                }
                Drawn::Monster(def) => {
                    let monster = actors.make(&mut commands, registries, def, spot.at, ev.map, &mut rng);
                    commands.entity(monster).insert(Post(spot.at));
                }
                Drawn::Items(..) | Drawn::Nothing => continue,
            }
            filled.insert(spot.at);
        }
    }
}

/// Refuses play when a prefab's or a dressing's slot names an item the
/// game has no definition for, asks for a tag no row of the loot table
/// carries, or a role none of whose members has a row in the spawn
/// table: each needs the game's own tables, which a file cannot see when
/// it loads, so a typo is a startup failure rather than a slot that is
/// quietly empty.
fn check_prefabs<A: ActorMaker, I: ItemMaker>(prefabs: Res<Prefabs<A::Def>>, actors: Res<A>, items: Res<I>, registries: Res<Registries>) {
    let mut problems = Vec::new();
    let legends =
        prefabs.defs().iter().map(|(_, d)| (d.name.as_str(), d.legend())).chain(prefabs.dressings().iter().map(|(_, d)| (d.name.as_str(), d.legend())));
    for (prefab, legend) in legends {
        for (c, slot) in legend.slots() {
            match slot {
                Slot::Item(roll) => match &roll.what {
                    Stock::Item(name) if items.id_of(name).is_none() => problems.push(format!("{prefab}: '{c}' holds {name:?}, which is no item")),
                    Stock::Tag(tag) if !items.table().carries(*tag) => {
                        let tag = registries.tags.name(*tag);
                        problems.push(format!("{prefab}: '{c}' asks for anything tagged {tag:?}, and nothing in the loot table carries it"))
                    }
                    _ => {}
                },
                Slot::Monster { pick: Pick::Role(id), .. } => {
                    let role = prefabs.roles().get(*id);
                    if role::band_for(actors.table(), role, 0).is_none() {
                        let role = &role.name;
                        problems.push(format!("{prefab}: '{c}' asks for role {role:?}, and no monster in it has a row in the spawn table"));
                    }
                }
                _ => {}
            }
        }
    }
    assert!(problems.is_empty(), "PrefabPlugin: {}", problems.join("; "));
}

/// Prefab slots: every prop, item and monster a place's keyed pieces name,
/// put down on the arrival that built it, the monsters through the game's
/// [`ActorMaker`] `A` and the items through its [`ItemMaker`] `I`.
///
/// Opt-in, and for one pair of makers. Needs [`Prefabs`] over `A`'s
/// monsters, both makers, [`Registries`] and the run's [`Seed`], from
/// which each slot's stream derives, each asked for with a hint. Does not
/// need [`LootPlugin`](crate::loot::LootPlugin): a game may lay items at
/// slots and nowhere else.
pub struct PrefabPlugin<A, I>(PhantomData<(A, I)>);

impl<A, I> Default for PrefabPlugin<A, I> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<A: ActorMaker, I: ItemMaker> Plugin for PrefabPlugin<A, I> {
    fn build(&self, app: &mut App) {
        use crate::plugin::{Needs, Turn, TurnSet};
        app.needs::<Prefabs<A::Def>>("PrefabPlugin", "the game's prefabs and roles, as `Prefabs::new(prefabs, roles)`, inserted before play begins")
            .needs::<A>("PrefabPlugin", "the resource holding the game's monsters, implementing `ActorMaker`, inserted before play begins")
            .needs::<I>("PrefabPlugin", "the resource holding the game's items, implementing `ItemMaker`, inserted before play begins")
            .needs::<Registries>("PrefabPlugin", "`Registries`, whose props a prefab's slots name")
            .needs::<Seed>("PrefabPlugin", "`Seed(RunSeed(n))`, which each slot's stream derives from")
            .add_message::<PlaceEntered>()
            .configure_sets(Turn, PrefabSet::Fill.in_set(TurnSet::React).before(crate::loot::LootSet::Scatter))
            .add_systems(Turn, fill_prefabs::<A, I>.in_set(PrefabSet::Fill))
            .add_systems(OnEnter(crate::state::EngineState::Playing), check_prefabs::<A, I>);
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "PrefabPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Actor, Position};
    use crate::items::Item;
    use crate::loot::tests::{FoundAs, Toys, toys};
    use crate::places::{OnMap, PlaceBuild, PlaceEntered};
    use crate::plugin::{Turn, headless_app};
    use crate::props::PropKind;
    use crate::seed::Seed;
    use crate::state::EngineState;
    use crate::world::WorldMap;
    use rl_core::{Point, RunSeed};
    use rl_grid::TileRegistry;
    use rl_mapgen::prefab::{Orient, Placement, Prefab, StampPrefab};
    use rl_mapgen::{BaseContext, BuildContext, Chain};
    use rl_rules::content::BandedEntry;
    use rl_rules::loot::ScatterRules;
    use rl_rules::{Named, Names, Registry, TagDef};

    /// A test game's monster: only a name.
    struct Beast(&'static str);

    impl Named for Beast {
        fn name(&self) -> &str {
            self.0
        }
    }

    /// Which beast an actor is, the one thing the toy maker records.
    #[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
    struct ToyKind(Id<Beast>);

    /// A test game's monster registry: a rat on every band from one to
    /// ten, a heavy from three to eight, and a moth the spawn table never
    /// names. The band of a place is one past its map number, one deeper
    /// than the toy items reckon it, so a monster slot drawn at the item
    /// maker's band is a test that fails.
    #[derive(Resource)]
    struct Beasts {
        defs: Registry<Beast>,
        table: BandedTable<Id<Beast>>,
    }

    impl ActorMaker for Beasts {
        type Def = Beast;

        fn make(&self, commands: &mut Commands, _: &Registries, def: Id<Beast>, at: Point, map: MapId, _: &mut StdRng) -> Entity {
            commands.spawn((Actor, Position(at), OnMap(map), ToyKind(def))).id()
        }

        fn table(&self) -> &BandedTable<Id<Beast>> {
            &self.table
        }

        fn band(&self, map: MapId) -> i32 {
            map.0 as i32 + 1
        }
    }

    fn beasts() -> Beasts {
        let defs = Registry::from_defs(vec![Beast("rat"), Beast("heavy"), Beast("moth")]).unwrap();
        let table = BandedTable::new(vec![BandedEntry::new(defs.expect("rat")).bands(1, 10), BandedEntry::new(defs.expect("heavy")).bands(3, 8)]);
        Beasts { defs, table }
    }

    const PROPS: &str = r#"[(name: "locker", glyph: 'L', color: (r: 255, g: 255, b: 255), blocks: true)]"#;

    const ROLES: &str = r#"{ "pair": ["rat", "heavy"], "fliers": ["moth"] }"#;

    /// A locker, a weapon two bands deep, a heavy, a pair member two bands
    /// deep and a mark the game keeps, walled round. Stamped at (2, 2), its
    /// slots stand at: `A` (3, 3), `w` (5, 3), `k` (4, 4), `s` (3, 5) and
    /// `m` (5, 5).
    const GUARDED: &str = r######"(
        name: "guarded locker",
        ground: "floor",
        rows: [
            "#####",
            "#A.w#",
            "#.k.#",
            "#s.m#",
            "#####",
        ],
        legend: {
            '#': Tile("wall"),
            '.': Tile("floor"),
            'A': Prop("locker"),
            'w': Item(tag: "weapon", band: 2),
            'k': Monster(monster: "heavy"),
            's': Monster(role: "pair", band: 2),
            'm': Mark,
        },
    )"######;

    /// One locker, under the glyph `GUARDED` gives a monster.
    const STORE: &str = r#"(name: "store", ground: "floor", rows: ["s"], legend: { 's': Prop("locker") })"#;

    /// One cell of floor and no slot, for stamping over another piece.
    const PATCH: &str = r#"(name: "patch", rows: ["."], legend: { '.': Tile("floor") })"#;

    const LOCKER: Point = Point::new(3, 3);
    const WEAPON: Point = Point::new(5, 3);
    const HEAVY: Point = Point::new(4, 4);
    const PAIR: Point = Point::new(3, 5);
    const MARK: Point = Point::new(5, 5);
    const CORNER: Point = Point::new(2, 2);
    const ENTRY: Point = Point::new(1, 1);

    /// An app filling prefabs from `files` over the toy monsters and items,
    /// a locker among the props, and the run seed `seed`.
    fn app(files: &[&str], seed: RunSeed) -> App {
        dressed_app(files, "[]", seed)
    }

    /// As [`app`], with the dressings `dressings` beside the prefabs.
    fn dressed_app(files: &[&str], dressings: &str, seed: RunSeed) -> App {
        let mut app = headless_app();
        app.add_plugins(PrefabPlugin::<Beasts, Toys>::default());
        let tiles = TileRegistry::standard();
        let tags = Registry::from_defs(vec![TagDef::new("weapon"), TagDef::new("armor"), TagDef::new("relic")]).unwrap();
        let props = rl_rules::prop::load(PROPS, &Names::new().tags(&tags)).unwrap();
        let beasts = beasts();
        let roles = rl_rules::role::load::<Beast>(ROLES, &Names::new().with("monster", &beasts.defs)).unwrap();
        let defs = {
            let names = Names::new().tags(&tags).with("prop", &props).with("monster", &beasts.defs).with("role", &roles);
            Registry::from_defs(files.iter().map(|text| rl_rules::prefab::load(text, &tiles, &names).unwrap()).collect()).unwrap()
        };
        let dressings = {
            let names = Names::new().tags(&tags).with("prop", &props).with("monster", &beasts.defs).with("role", &roles);
            rl_rules::dressing::load(dressings, &tiles, &names).unwrap()
        };
        let items = toys(&tags, ScatterRules::new());
        app.insert_resource(Prefabs::with_dressings(defs, roles, dressings))
            .insert_resource(beasts)
            .insert_resource(items)
            .insert_resource(Registries { tags, props, ..Default::default() })
            .insert_resource(Seed(seed));
        app
    }

    /// The piece called `name`, keyed, as a game's chain would ask for it.
    fn piece(app: &App, name: &str) -> Prefab {
        app.world().resource::<Prefabs<Beast>>().piece(name).unwrap()
    }

    /// Stamps each piece at its corner onto open floor, in one chain, and
    /// builds the place the way a game's builder does.
    fn stamped(pieces: Vec<(Prefab, Point)>) -> PlaceBuild {
        const NAMES: [&str; 3] = ["first", "second", "third"];
        let tiles = TileRegistry::standard();
        let mut ctx = BaseContext::blank(20, 12, tiles.clone(), tiles.expect("floor"));
        let mut chain = Chain::new();
        for (i, (prefab, corner)) in pieces.into_iter().enumerate() {
            chain = chain.then(StampPrefab { name: NAMES[i], prefab, at: Placement::At(corner), orient: Orient::Fixed });
        }
        chain.run(&mut ctx, RunSeed(0)).unwrap();
        ctx.emit(rl_mapgen::passes::StartPoint(ENTRY));
        PlaceBuild::from_context(ctx).unwrap()
    }

    /// Installs `build` as `map` and makes it the current map.
    fn install(app: &mut App, map: MapId, build: PlaceBuild) {
        let mut world = WorldMap::new(TileRegistry::standard().tables());
        world.install_place(map, build);
        world.switch_to(map);
        app.insert_resource(world);
        app.update();
    }

    /// Arrives in `map` at `entry` and runs one turn.
    fn arrive(app: &mut App, map: MapId, entry: Point, first: bool) {
        app.world_mut().write_message(PlaceEntered { map, first, entry, exit: None });
        app.world_mut().run_schedule(Turn);
    }

    /// The guarded locker stamped at its corner in `map`, entered for the
    /// first time at `entry`.
    fn guarded(map: MapId, seed: RunSeed, entry: Point) -> App {
        let mut app = app(&[GUARDED], seed);
        let build = stamped(vec![(piece(&app, "guarded locker"), CORNER)]);
        install(&mut app, map, build);
        arrive(&mut app, map, entry, true);
        app
    }

    /// Everything standing on `map`, as where and what: `prop locker`,
    /// `item blade (Placed)` or `beast heavy`, sorted.
    fn everything(app: &mut App, map: MapId) -> Vec<(Point, String)> {
        let world = app.world_mut();
        let mut found = Vec::new();
        let registries = world.resource::<Registries>().clone();
        let mut props = world.query::<(&PropKind, &Position, &OnMap)>();
        found.extend(props.iter(world).filter(|(.., on)| on.0 == map).map(|(k, at, _)| (at.0, format!("prop {}", registries.props.get(k.0).name))));
        let mut items = world.query_filtered::<(&Name, &FoundAs, &Position, &OnMap), With<Item>>();
        found.extend(items.iter(world).filter(|(.., on)| on.0 == map).map(|(n, f, at, _)| (at.0, format!("item {} ({:?})", n.as_str(), f.0.found))));
        let mut actors = world.query::<(&ToyKind, &Position, &OnMap)>();
        let named: Vec<(Point, Id<Beast>)> = actors.iter(world).filter(|(.., on)| on.0 == map).map(|(k, at, _)| (at.0, k.0)).collect();
        let beasts = world.resource::<Beasts>();
        found.extend(named.into_iter().map(|(at, id)| (at, format!("beast {}", beasts.defs.get(id).0))));
        found.sort();
        found
    }

    /// What stands at `cell` on `map`.
    fn at(app: &mut App, map: MapId, cell: Point) -> Vec<String> {
        everything(app, map).into_iter().filter(|(p, _)| *p == cell).map(|(_, what)| what).collect()
    }

    /// `GUARDED` with an entry in its top wall, to be grown.
    const GROWN: &str = r######"(
        name: "grown locker",
        ground: "floor",
        rows: [
            "##E##",
            "#A.w#",
            "#.k.#",
            "#s.m#",
            "#####",
        ],
        legend: {
            '#': Tile("wall"),
            '.': Tile("floor"),
            'A': Prop("locker"),
            'w': Item(tag: "weapon", band: 2),
            'k': Monster(monster: "heavy"),
            's': Monster(role: "pair", band: 2),
            'm': Mark,
            'E': Entry,
        },
    )"######;

    /// The grown locker laid by growth among plain rooms, and the place
    /// built the way a game's builder does.
    fn grown(app: &App, seed: RunSeed) -> PlaceBuild {
        use rl_mapgen::accrete::{Accrete, District, Join, Rectangle};
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let piece = app.world().resource::<Prefabs<Beast>>().grown("grown locker", Join::Door).unwrap();
        let grow = Accrete {
            floor,
            door: floor,
            shapes: vec![Box::new(Rectangle { name: "room".into(), w: (5, 8), h: (5, 8), join: Join::Door })],
            districts: vec![District { name: "all".into(), shapes: vec![(0, 1)] }],
            mix: vec![(0, 1)],
            districts_per_10k: 5,
            open_pct: 30,
            patience: 300,
            barriers: Vec::new(),
            locked: floor,
            sealed: floor,
            pieces: vec![Box::new(piece)],
            late_pct: 10,
            first: None,
        };
        let chain = Chain::new().then(grow).then(rl_mapgen::dungeon::RandomStart);
        PlaceBuild::from_context(chain.run_retrying(|| BaseContext::blank(40, 30, tiles.clone(), wall), seed, 20).unwrap()).unwrap()
    }

    /// A piece grown as an area of its own fills each slot where its stamp
    /// put the mark, as a stamped piece does.
    #[test]
    fn a_grown_piece_s_slots_fill_as_a_stamped_piece_s() {
        let deck = MapId(2);
        for s in 0..6 {
            let mut app = app(&[GROWN], crate::testing::TEST_SEED);
            let build = grown(&app, RunSeed(s));
            let slot =
                |glyph: char| build.spots.iter().find(|spot| spot.tag == glyph as u32 && spot.prefab.is_some()).expect("its stamp's marks are keyed spots").at;
            let (locker, weapon, heavy) = (slot('A'), slot('w'), slot('k'));
            let entry = build.entry;
            install(&mut app, deck, build);
            arrive(&mut app, deck, entry, true);
            assert_eq!(at(&mut app, deck, locker), vec!["prop locker"], "seed {s}");
            assert_eq!(at(&mut app, deck, weapon), vec!["item saber (Placed)"], "seed {s}");
            assert_eq!(at(&mut app, deck, heavy), vec!["beast heavy"], "seed {s}");
            assert_eq!(everything(&mut app, deck).len(), 4, "seed {s}: one thing a slot and nothing more");
        }
    }

    /// The first arrival puts the locker, a weapon drawn at the place's
    /// band plus two, and the fixed heavy each on its own cell; at band two
    /// plus two the only weapon is a saber, where band two alone would give
    /// a blade.
    #[test]
    fn a_prefab_fills_its_prop_item_and_monster_slots_on_the_first_entry() {
        let deck = MapId(2);
        let mut app = guarded(deck, crate::testing::TEST_SEED, ENTRY);
        assert_eq!(at(&mut app, deck, LOCKER), vec!["prop locker"]);
        assert_eq!(at(&mut app, deck, WEAPON), vec!["item saber (Placed)"]);
        assert_eq!(at(&mut app, deck, HEAVY), vec!["beast heavy"]);
        let pair = at(&mut app, deck, PAIR);
        assert!(pair == vec!["beast rat"] || pair == vec!["beast heavy"], "a member of the pair at its slot: {pair:?}");
        assert_eq!(everything(&mut app, deck).len(), 4, "one thing a slot and nothing more: {:?}", everything(&mut app, deck));
    }

    /// A role slot two bands deeper than the place draws at that band, the
    /// band the monster maker gives the place: map one is band two and
    /// draws at four, where a heavy may stand though band two alone never
    /// has one, and map six is band seven and draws at nine, where only a
    /// rat does, though the item maker's band six plus two would allow a
    /// heavy. Every draw is a member.
    #[test]
    fn a_role_slot_draws_a_member_of_the_role_at_the_places_band_plus_its_offset() {
        let mut shallow = Vec::new();
        for s in 0..16 {
            let deep = MapId(6);
            assert_eq!(at(&mut guarded(deep, RunSeed(s), ENTRY), deep, PAIR), vec!["beast rat"], "seed {s}: at band nine only a rat applies");
            let deck = MapId(1);
            shallow.extend(at(&mut guarded(deck, RunSeed(s), ENTRY), deck, PAIR));
        }
        assert!(shallow.iter().all(|b| b == "beast rat" || b == "beast heavy"), "only members: {shallow:?}");
        assert!(shallow.iter().any(|b| b == "beast heavy"), "a heavy, which band two alone never draws, stands at band two plus two: {shallow:?}");
    }

    /// Both monsters placed at slots hold their own cells as their posts.
    #[test]
    fn a_monster_placed_at_a_slot_holds_that_cell_as_its_post() {
        let mut app = guarded(MapId(1), crate::testing::TEST_SEED, ENTRY);
        let world = app.world_mut();
        let mut posts: Vec<(Point, Point)> = world.query_filtered::<(&Position, &Post), With<ToyKind>>().iter(world).map(|(at, post)| (at.0, post.0)).collect();
        posts.sort();
        assert_eq!(posts, vec![(HEAVY, HEAVY), (PAIR, PAIR)]);
    }

    /// Arriving on the weapon's cell, a slot that draws, spawns nothing
    /// there, and every other slot, the pair member drawn after it among
    /// them, holds exactly what it holds when the arrival is elsewhere.
    #[test]
    fn a_slot_on_the_arrival_cell_spawns_nothing_and_every_other_slot_is_the_same() {
        let deck = MapId(3);
        let mut elsewhere = guarded(deck, crate::testing::TEST_SEED, ENTRY);
        let mut on_slot = guarded(deck, crate::testing::TEST_SEED, WEAPON);
        assert_eq!(at(&mut elsewhere, deck, WEAPON), vec!["item saber (Placed)"]);
        assert!(at(&mut on_slot, deck, WEAPON).is_empty(), "nothing on the arrival cell");
        let rest = |app: &mut App| everything(app, deck).into_iter().filter(|(p, _)| *p != WEAPON).collect::<Vec<_>>();
        let expected = rest(&mut elsewhere);
        assert_eq!(expected.len(), 3);
        assert_eq!(rest(&mut on_slot), expected);
    }

    /// A slot walled over after the stamp spawns nothing, and the rest of
    /// the piece is filled as usual.
    #[test]
    fn a_slot_whose_cell_is_no_longer_walkable_spawns_nothing() {
        let deck = MapId(1);
        let mut app = app(&[GUARDED], crate::testing::TEST_SEED);
        let mut build = stamped(vec![(piece(&app, "guarded locker"), CORNER)]);
        build.terrain.set(HEAVY, TileRegistry::standard().expect("wall"));
        install(&mut app, deck, build);
        arrive(&mut app, deck, ENTRY, true);
        assert!(at(&mut app, deck, HEAVY).is_empty(), "nothing inside a wall");
        assert_eq!(at(&mut app, deck, LOCKER), vec!["prop locker"]);
        assert_eq!(at(&mut app, deck, WEAPON), vec!["item blade (Placed)"]);
        assert_eq!(at(&mut app, deck, PAIR).len(), 1);
    }

    /// `s` is a pair member in the guarded locker and a locker in the
    /// store: each stamp's `s` is filled by its own prefab's legend.
    #[test]
    fn two_prefabs_sharing_a_glyph_fill_it_each_their_own_way() {
        let deck = MapId(1);
        let mut app = app(&[GUARDED, STORE], crate::testing::TEST_SEED);
        let store = Point::new(10, 2);
        let build = stamped(vec![(piece(&app, "guarded locker"), CORNER), (piece(&app, "store"), store)]);
        install(&mut app, deck, build);
        arrive(&mut app, deck, ENTRY, true);
        assert_eq!(at(&mut app, deck, store), vec!["prop locker"]);
        let pair = at(&mut app, deck, PAIR);
        assert!(pair == vec!["beast rat"] || pair == vec!["beast heavy"], "the guarded locker's own `s`: {pair:?}");
    }

    /// A piece stamped over a slot owns that cell: the heavy's slot under a
    /// patch of floor spawns nothing, and the rest of the piece is filled.
    #[test]
    fn a_slot_a_later_stamp_covered_spawns_nothing() {
        let deck = MapId(1);
        let mut app = app(&[GUARDED, PATCH], crate::testing::TEST_SEED);
        let build = stamped(vec![(piece(&app, "guarded locker"), CORNER), (piece(&app, "patch"), HEAVY)]);
        install(&mut app, deck, build);
        arrive(&mut app, deck, ENTRY, true);
        assert!(at(&mut app, deck, HEAVY).is_empty(), "the patch owns the heavy's cell");
        assert_eq!(at(&mut app, deck, LOCKER), vec!["prop locker"]);
        assert_eq!(at(&mut app, deck, PAIR).len(), 1);
    }

    /// A slot of nought to one slug is sometimes empty and never a stack of
    /// none: a count that rolls nought lays nothing, rather than handing the
    /// maker a nought it may round up to one, as Foundry's does.
    #[test]
    fn an_item_slot_whose_count_rolls_nought_lays_nothing() {
        const SLUGS: &str = r#"(name: "slugs", ground: "floor", rows: ["i"], legend: { 'i': Item(item: "slug", count: (0, 1)) })"#;
        let deck = MapId(1);
        let cell = Point::new(4, 4);
        let mut empty = 0;
        for s in 0..32 {
            let mut app = app(&[SLUGS], RunSeed(s));
            let build = stamped(vec![(piece(&app, "slugs"), cell)]);
            install(&mut app, deck, build);
            arrive(&mut app, deck, ENTRY, true);
            let world = app.world_mut();
            let laid: Vec<u32> = world
                .query_filtered::<(&crate::items::Stack, &Position), With<Item>>()
                .iter(world)
                .filter(|(_, at)| at.0 == cell)
                .map(|(s, _)| s.count)
                .collect();
            assert!(laid.is_empty() || laid == vec![1], "seed {s}: one slug or nothing, never a stack of none: {laid:?}");
            empty += usize::from(laid.is_empty());
        }
        assert!(empty > 0 && empty < 32, "sometimes a slug and sometimes none: {empty} of 32 empty");
    }

    /// A revisit is not the arrival that built the place, and fills
    /// nothing more.
    #[test]
    fn a_revisit_fills_nothing() {
        let deck = MapId(1);
        let mut app = guarded(deck, crate::testing::TEST_SEED, ENTRY);
        let first = everything(&mut app, deck);
        arrive(&mut app, deck, ENTRY, false);
        assert_eq!(everything(&mut app, deck), first);
    }

    /// The prefab's `Mark` is the game's, and so is a mark of an unkeyed
    /// piece even when its glyph is a locker in a keyed one.
    #[test]
    fn a_mark_and_an_unkeyed_spot_are_left_to_the_game() {
        let deck = MapId(1);
        let mut app = app(&[GUARDED], crate::testing::TEST_SEED);
        let unkeyed = Prefab::parse(&["A"], |_| None).unwrap();
        let loose = Point::new(12, 8);
        let build = stamped(vec![(piece(&app, "guarded locker"), CORNER), (unkeyed, loose)]);
        assert!(build.spots.iter().any(|s| s.at == loose && s.prefab.is_none() && s.tag == 'A' as u32));
        install(&mut app, deck, build);
        arrive(&mut app, deck, ENTRY, true);
        assert!(at(&mut app, deck, MARK).is_empty(), "the mark is the game's");
        assert!(at(&mut app, deck, loose).is_empty(), "the unkeyed spot is the game's");
        assert_eq!(at(&mut app, deck, LOCKER), vec!["prop locker"], "while the keyed `A` is filled");
    }

    /// Every slot's stream derives from the run's seed, so a game that
    /// forgot one is told so by name as play begins, beside everything
    /// else it forgot.
    #[test]
    fn prefab_slots_without_a_seed_say_so_when_play_begins() {
        let mut app = app(&[GUARDED], crate::testing::TEST_SEED);
        app.world_mut().remove_resource::<Seed>();
        let missing = app.world().resource::<crate::plugin::Requirements>().missing(app.world());
        assert!(missing.iter().any(|m| m.starts_with("PrefabPlugin needs") && m.contains("Seed(RunSeed(n))")), "{missing:#?}");
    }

    /// Installs a place and begins play, where the plugin's check runs.
    fn play(files: &[&str]) {
        let mut app = app(files, crate::testing::TEST_SEED);
        install(&mut app, MapId(1), stamped(Vec::new()));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
    }

    #[test]
    #[should_panic(expected = "which is no item")]
    fn play_refuses_a_prefab_holding_an_item_the_game_has_no_definition_for() {
        play(&[r#"(name: "ingots", ground: "floor", rows: ["i"], legend: { 'i': Item(item: "ingot") })"#]);
    }

    /// A dressing's slots are held to the game's tables as a prefab's are.
    #[test]
    #[should_panic(expected = "ingots: 'i' holds \"ingot\", which is no item")]
    fn play_refuses_a_dressing_holding_an_item_the_game_has_no_definition_for() {
        let mut app = dressed_app(
            &[],
            r#"[(name: "ingots", legend: { 'i': Item(item: "ingot") }, place: [(glyph: 'i', at: Walls, density_pct: 10)])]"#,
            crate::testing::TEST_SEED,
        );
        install(&mut app, MapId(1), stamped(Vec::new()));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
    }

    #[test]
    #[should_panic(expected = "nothing in the loot table carries it")]
    fn play_refuses_a_prefab_asking_for_a_tag_nothing_carries() {
        play(&[r#"(name: "relics", ground: "floor", rows: ["r"], legend: { 'r': Item(tag: "relic") })"#]);
    }

    #[test]
    #[should_panic(expected = "no monster in it has a row in the spawn table")]
    fn play_refuses_a_role_none_of_whose_members_can_be_drawn() {
        play(&[r#"(name: "nest", ground: "floor", rows: ["f"], legend: { 'f': Monster(role: "fliers") })"#]);
    }

    /// Weapons two bands deep in every corner, lockers along the walls,
    /// heavies down the middle and a mark scattered: a dressing in the test
    /// game's terms.
    const BUNKS: &str = r#"[(
        name: "bunks",
        legend: { 'A': Prop("locker"), 'k': Monster(monster: "heavy"), 'w': Item(tag: "weapon", band: 2), 'm': Mark },
        place: [
            (glyph: 'w', at: Corners, density_pct: 100),
            (glyph: 'A', at: Walls, density_pct: 30),
            (glyph: 'k', at: Center, density_pct: 100),
            (glyph: 'm', at: Scatter, density_pct: 5),
        ],
    )]"#;

    /// A room of nine by seven at (2, 3) dressed with `bunks` by its
    /// district, its door in its top wall and the way in beyond it.
    fn bunked(app: &App, seed: RunSeed) -> PlaceBuild {
        use rl_mapgen::{Area, Dress, Opening};
        let tiles = TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut ctx = BaseContext::blank(15, 13, tiles, wall);
        let bounds = rl_core::Rect::new(2, 3, 9, 7);
        for c in bounds.cells().chain([Point::new(6, 2), Point::new(6, 1)]) {
            ctx.terrain_mut().set(c, floor);
        }
        ctx.emit(rl_mapgen::dungeon::Room(bounds));
        ctx.emit(Area { index: 0, shape: "room".into(), district: "d".into(), cells: bounds.cells().collect(), bounds, inner: bounds, entries: None });
        ctx.emit(Opening { cells: vec![Point::new(6, 2)], facing: rl_core::Direction::North, areas: (0, 0), join: rl_mapgen::Join::Door });
        ctx.emit(rl_mapgen::passes::StartPoint(Point::new(6, 1)));
        let bunks = app.world().resource::<Prefabs<Beast>>().dressing("bunks").unwrap();
        let pass = Dress { name: "dress", floor, dressings: vec![bunks], districts: vec![rl_mapgen::DistrictDressings::new("d", vec![(0, 1)])] };
        Chain::new().then(pass).run(&mut ctx, seed).unwrap();
        PlaceBuild::from_context(ctx).unwrap()
    }

    /// A dressing's slots are spots keyed to it, filled on the first
    /// arrival each by what its legend names, as a prefab's are, and a
    /// save of the place keeps them keyed, so they still find their
    /// meaning once loaded.
    #[test]
    fn a_dressing_s_slots_fill_and_save_as_a_prefab_s() {
        let deck = MapId(2);
        for s in 0..8 {
            let mut app = dressed_app(&[], BUNKS, crate::testing::TEST_SEED);
            let build = bunked(&app, RunSeed(s));
            let spots: Vec<(char, Point)> = build.spots.iter().map(|spot| (char::from_u32(spot.tag).unwrap(), spot.at)).collect();
            let keyed = |glyph: char| spots.iter().filter(|(c, _)| *c == glyph).map(|(_, p)| *p).collect::<BTreeSet<_>>();
            assert_eq!(keyed('w').len(), 4, "seed {s}: a weapon in every corner");
            assert_eq!(keyed('k'), BTreeSet::from([Point::new(5, 6), Point::new(6, 6), Point::new(7, 6)]), "seed {s}: heavies down the middle");
            assert!(!keyed('A').is_empty(), "seed {s}: lockers along the walls");
            assert!(build.spots.iter().all(|spot| spot.prefab.is_some()), "seed {s}: every slot is keyed");
            let entry = build.entry;
            install(&mut app, deck, build);
            arrive(&mut app, deck, entry, true);
            for p in keyed('w') {
                assert_eq!(at(&mut app, deck, p), vec!["item saber (Placed)"], "seed {s}: drawn two bands deep");
            }
            for p in keyed('k') {
                assert_eq!(at(&mut app, deck, p), vec!["beast heavy"], "seed {s}");
            }
            for p in keyed('A') {
                assert_eq!(at(&mut app, deck, p), vec!["prop locker"], "seed {s}");
            }
            let marks = keyed('m').len();
            assert_eq!(everything(&mut app, deck).len(), spots.len() - marks, "seed {s}: one thing a slot, and a mark is the game's");

            let saved = app.world().resource::<WorldMap>().export();
            let mut back = WorldMap::new(TileRegistry::standard().tables());
            back.import(ron::from_str(&ron::to_string(&saved).unwrap()).unwrap());
            let before = &app.world().resource::<WorldMap>().place(deck).unwrap().spots;
            let after = &back.place(deck).unwrap().spots;
            assert_eq!(after, before, "seed {s}: the save keeps every spot and its key");
            let prefabs = app.world().resource::<Prefabs<Beast>>();
            assert!(
                after.iter().all(|spot| prefabs.slot(spot.prefab.unwrap(), char::from_u32(spot.tag).unwrap()).is_some()),
                "seed {s}: and each still finds its slot"
            );
        }
    }
}
