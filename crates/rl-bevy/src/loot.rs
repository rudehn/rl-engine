//! Loot: items put down where a place is first built, left by the dead,
//! and stocked into containers.
//!
//! What an item *is* stays a game's own, and the engine never learns it.
//! What is here is when, where, how many and from which stream, the loop
//! every game with loot wrote for itself: a game implements [`ItemMaker`]
//! on the resource that holds its item definitions, and the engine calls
//! it to make each thing it has decided to put down, then puts it there.
//!
//! - **A place's floor**, the first time it is entered: its
//!   [`ScatterRules`] put a count beside each kind of mark and a number more
//!   loose, each drawn from the game's [`LootTable`] at the place's band,
//!   from a stream derived for that place alone, so what lies on a deck
//!   never depends on how many kills came before it.
//! - **A region of the streamed surface**, the first time it loads, the
//!   same way, remembered in [`Scattered`] so a region streamed out and
//!   back in is not scattered twice.
//! - **The dead**: an actor carrying [`Drops`] leaves what its table rolls
//!   where it died, from [`LootRng`], the engine's own stream for it, so a
//!   kill's loot never shifts a blow that has not been struck.
//! - **Containers**: the engine answers every
//!   [`FillContainer`] itself. A fixed item is
//!   made in the count rolled; a tag is drawn that many times from the loot
//!   table at the container's band plus the row's offset, each draw its
//!   own, from a stream derived for the container's cell.
//!
//! A band is how deep, far or dangerous somewhere is, and only the game
//! knows, so [`ItemMaker::band`] answers it for a [`LootArea`]. Whatever is
//! made is told why, as [`Found`], so a game that rolls quality rolls a
//! hoard's differently from a floor's, from the stream it is handed.
//!
//! Two games did all of this by hand before it moved here, and wrote it the
//! same way; `docs/design/loot.md` has the reasoning.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::marker::PhantomData;

use bevy::prelude::*;
use rand::rngs::StdRng;
use rl_core::seed::position_hash;
use rl_core::{Grid2D, Id, Point, SeedDomain};
use rl_rules::loot::{DropTable, Layout, LootTable, ScatterRules, plan_scatter};
use rl_rules::prop::Stock;

use crate::combat::DeathEvent;
use crate::components::Position;
use crate::items::Inventory;
use crate::places::{MapId, OnMap, PlaceEntered};
use crate::props::{FillContainer, Prop};
use crate::registries::Registries;
use crate::seed::Seed;
use crate::world::{ChunkLoaded, WorldMap, WorldRes};

/// Where a band is asked for: a place, or a region of the streamed surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LootArea {
    /// A place built on its own, a deck or a cave.
    Place(MapId),
    /// A region of the streamed surface, by region coordinates.
    Region(Point),
}

/// Why something is being made, for a game that makes it differently
/// depending on how it was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// Lying on a floor when the place was first built.
    Scatter,
    /// Left by the dead.
    Drop,
    /// Stocked into a container.
    Container,
    /// Laid at a prefab's slot when the place was first built.
    Placed,
}

/// A game's item registry, as the engine asks it for things.
///
/// Implemented on the resource that holds a game's item definitions, which
/// is the one place that knows what an item is: the engine decides when,
/// where and how many, and asks this to make them.
pub trait ItemMaker: Resource {
    /// The game's item definition.
    type Def: Send + Sync + 'static;

    /// Makes `count` of `def`, found as `found`, and returns every entity
    /// made, placed nowhere: the engine puts them on the floor or into a
    /// container. A thing that stacks is one stack of `count`; anything
    /// else is `count` entities. `rng` is the engine's stream for what is
    /// being made, for whatever the game rolls on the thing itself.
    fn make(&self, commands: &mut Commands, registries: &Registries, def: Id<Self::Def>, count: u32, found: Found, rng: &mut StdRng) -> Vec<Entity>;

    /// The definition called `name`, which a container names its fixed
    /// contents by.
    fn id_of(&self, name: &str) -> Option<Id<Self::Def>>;

    /// What can be found where, loaded with
    /// [`rl_rules::loot::load`].
    fn table(&self) -> &LootTable<Id<Self::Def>>;

    /// How a place's floor is laid out when it is first built; nothing
    /// beside any mark and nothing loose unless the game says.
    fn scatter(&self) -> ScatterRules {
        ScatterRules::new()
    }

    /// How deep, far or dangerous `area` is, in the band numbering the
    /// table is written in.
    fn band(&self, area: LootArea) -> i32;

    /// How many loose items `area` gets, given the `planned` its rules
    /// rolled: the rules' own count unless the game knows better, a port
    /// that is always stocked.
    fn loose(&self, area: LootArea, planned: u32) -> u32 {
        let _ = area;
        planned
    }
}

/// What an actor leaves when it dies, keyed by the game's item ids.
///
/// Put on by the game when it spawns the actor, from whatever its own
/// bestiary says; the engine rolls it on the death.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Drops<D: Send + Sync + 'static>(pub DropTable<Id<D>>);

/// The engine's stream for what the dead leave.
///
/// Its own, like props' and combat's, so a kill's loot never nudges a
/// later blow, and kept from one death to the next, so a second kill rolls
/// on from where the first left off.
#[derive(Resource)]
pub struct LootRng(pub StdRng);

impl crate::seed::Stream for LootRng {
    fn for_run(seed: rl_core::RunSeed) -> Self {
        Self(seed.rng(SeedDomain::new(b"loot"), 0))
    }
}

/// The regions of the streamed surface already scattered, so one streamed
/// out and back in is not scattered again. A place needs no such record:
/// it is scattered on the arrival that built it, which it already knows.
///
/// Saved by `rl-save`: a game that streams its surface adds
/// `save_state::<Scattered>()`.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct Scattered(pub BTreeSet<Point>);

/// Where loot is put down in a turn, inside
/// [`TurnSet::React`](crate::plugin::TurnSet::React).
///
/// A game that builds a place in the same pass orders what it puts down
/// before [`LootSet::Scatter`], so loot never lands under a crate a later
/// system would put down.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LootSet {
    /// A place's floor, on the arrival that built it.
    Scatter,
    /// What the dead leave.
    Drop,
}

/// A stream for one area's scatter or one container's contents, derived
/// from the run's seed under `domain` for `index` alone, so nothing that
/// happened elsewhere first changes what is drawn there.
pub(crate) fn stream_for(seed: &Seed, domain: &[u8], index: u64) -> StdRng {
    seed.0.rng(SeedDomain::new(domain), index)
}

/// The cells props stand on in `map`, where nothing is put down: an item
/// under a crate is one nothing can pick up.
fn standing(props: &Query<(&Position, Option<&OnMap>), With<Prop>>, map: MapId) -> Vec<Point> {
    props.iter().filter(|(_, on)| on.map_or(MapId::SURFACE, |m| m.0) == map).map(|(at, _)| at.0).collect()
}

/// Makes what a plan or a slot put down, found as `found`, and lays it on
/// the floor of `map`.
pub(crate) fn lay<M: ItemMaker>(
    commands: &mut Commands,
    maker: &M,
    registries: &Registries,
    what: (Id<M::Def>, u32, Point),
    map: MapId,
    found: Found,
    rng: &mut StdRng,
) {
    let (def, count, at) = what;
    for item in maker.make(commands, registries, def, count, found, rng) {
        let mut e = commands.entity(item);
        e.insert(Position(at));
        if map != MapId::SURFACE {
            e.insert(OnMap(map));
        }
    }
}

/// What scattering reads.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Floor<'w, 's, M: ItemMaker> {
    maker: Res<'w, M>,
    registries: Res<'w, Registries>,
    map: Res<'w, WorldMap>,
    seed: Res<'w, Seed>,
    props: Query<'w, 's, (&'static Position, Option<&'static OnMap>), With<Prop>>,
}

/// Scatters a place's loot on the arrival that built it: a count beside
/// each kind of mark its rules name and a number more loose, at the band
/// the game gives the place. A revisit is not a first arrival, so nothing
/// is scattered twice.
pub fn scatter_places<M: ItemMaker>(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, floor: Floor<M>) {
    let Floor { maker, registries, map, seed, props } = &floor;
    for ev in entered.read().filter(|ev| ev.first) {
        let Some(place) = map.place(ev.map) else { continue };
        let area = LootArea::Place(ev.map);
        let (band, rules) = (maker.band(area), maker.scatter());
        let mut rng = stream_for(seed, b"loot.place", u64::from(ev.map.0));
        let marks: Vec<(u32, Point)> = place.spots.iter().map(|s| (s.tag, s.at)).collect();
        let standing = standing(props, ev.map);
        let mut free = |p: Point| map.is_walkable(p) && !standing.contains(&p);
        let loose = maker.loose(area, rules.loose_count(band, &mut rng));
        for placed in plan_scatter(maker.table(), band, &rules, Layout { marks: &marks, bounds: place.terrain.bounds() }, loose, &mut free, &mut rng) {
            lay(&mut commands, &**maker, registries, (placed.item, placed.count, placed.at), ev.map, Found::Scatter, &mut rng);
        }
    }
}

/// Scatters a region of the streamed surface the first time it loads, the
/// way a place is scattered, with no marks: only loose items, as many as
/// the rules and the game say.
pub fn scatter_regions<M: ItemMaker>(
    mut commands: Commands,
    mut loaded: MessageReader<ChunkLoaded>,
    mut scattered: ResMut<Scattered>,
    world: Option<Res<WorldRes>>,
    floor: Floor<M>,
) {
    let Some(world) = world else { return };
    let Floor { maker, registries, map, seed, props } = &floor;
    for ev in loaded.read() {
        if !scattered.0.insert(ev.region) {
            continue;
        }
        let area = LootArea::Region(ev.region);
        let (band, rules) = (maker.band(area), maker.scatter());
        let mut rng = stream_for(seed, b"loot.region", position_hash(0, ev.region.x, ev.region.y));
        let standing = standing(props, MapId::SURFACE);
        let mut free = |p: Point| map.is_walkable(p) && !standing.contains(&p);
        let loose = maker.loose(area, rules.loose_count(band, &mut rng));
        for placed in plan_scatter(maker.table(), band, &rules, Layout { marks: &[], bounds: world.0.region_tiles(ev.region) }, loose, &mut free, &mut rng) {
            lay(&mut commands, &**maker, registries, (placed.item, placed.count, placed.at), MapId::SURFACE, Found::Scatter, &mut rng);
        }
    }
}

/// Leaves what each dead actor's [`Drops`] rolls where it died, on its own
/// map, from [`LootRng`].
///
/// Read off the dying entity while it is still there: the dead are buried
/// at the end of the frame, after every reaction.
pub fn drop_on_death<M: ItemMaker>(
    mut commands: Commands,
    mut deaths: MessageReader<DeathEvent>,
    maker: Res<M>,
    registries: Res<Registries>,
    rng: Option<ResMut<LootRng>>,
    carriers: Query<(&Drops<M::Def>, Option<&OnMap>)>,
) {
    let Some(mut rng) = rng else { return };
    for death in deaths.read() {
        let Ok((drops, on)) = carriers.get(death.entity) else { continue };
        for (def, count) in drops.0.roll(&mut rng.0) {
            for item in maker.make(&mut commands, &registries, def, count, Found::Drop, &mut rng.0) {
                let mut e = commands.entity(item);
                e.insert(Position(death.at));
                if let Some(on) = on {
                    e.insert(*on);
                }
            }
        }
    }
}

/// What stocking a container reads besides the maker.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Stocking<'w, 's> {
    registries: Res<'w, Registries>,
    seed: Res<'w, Seed>,
    world: Option<Res<'w, WorldRes>>,
    places: Query<'w, 's, (&'static Position, Option<&'static OnMap>)>,
    bags: Query<'w, 's, &'static mut Inventory>,
}

/// The area a cell of `map` is in: its place, or its region on a streamed
/// surface.
fn area_of(map: MapId, at: Point, world: Option<&WorldRes>) -> LootArea {
    match world {
        Some(world) if map == MapId::SURFACE => LootArea::Region(world.0.region_of_tile(at)),
        _ => LootArea::Place(map),
    }
}

/// What `count` of `what` comes to at `band`: a fixed item in that count,
/// or that many draws of a tag, each its own, with draws of one thing
/// made together. By definition id, so what is made comes out in one
/// order however the draws fell. Shared by containers and prefab slots,
/// which ask in the same words.
pub(crate) fn draw_stock<M: ItemMaker>(maker: &M, what: &Stock, count: u32, band: i32, rng: &mut StdRng) -> Vec<(Id<M::Def>, u32)> {
    let mut wanted: BTreeMap<u32, (Id<M::Def>, u32)> = BTreeMap::new();
    match what {
        Stock::Item(name) => {
            if let Some(def) = maker.id_of(name) {
                wanted.insert(def.raw(), (def, count));
            }
        }
        Stock::Tag(tag) => {
            for _ in 0..count {
                let Some(def) = maker.table().pick_tagged(*tag, band, rng) else { break };
                wanted.entry(def.raw()).or_insert((def, 0)).1 += 1;
            }
        }
    }
    wanted.into_values().collect()
}

/// Answers every [`FillContainer`]: a fixed item in the count rolled, and
/// a tag as that many draws from the loot table at the container's band
/// plus the row's offset, from a stream derived for the container's cell.
///
/// Draws of the same thing are made together, so a row of ammunition that
/// drew one kind six times is one stack of six rather than six stacks of
/// one.
pub fn fill_containers<M: ItemMaker>(mut commands: Commands, mut asks: MessageReader<FillContainer>, maker: Res<M>, stocking: Stocking) {
    let Stocking { registries, seed, world, places, mut bags } = stocking;
    for ask in asks.read() {
        let Ok((at, on)) = places.get(ask.prop) else { continue };
        let map = on.map_or(MapId::SURFACE, |m| m.0);
        let mut rng = stream_for(&seed, b"loot.container", position_hash(u64::from(map.0), at.0.x, at.0.y));
        let band = maker.band(area_of(map, at.0, world.as_deref())) + ask.band;
        let mut made = Vec::new();
        for (def, count) in draw_stock(&*maker, &ask.what, ask.count, band, &mut rng) {
            made.extend(maker.make(&mut commands, &registries, def, count, Found::Container, &mut rng));
        }
        if let Ok(mut bag) = bags.get_mut(ask.prop) {
            bag.items.extend(made);
        }
    }
}

/// Refuses play when a container names an item the game's registry has
/// never heard of, or asks for a tag no row of its table carries, so a
/// typo is a startup failure rather than a crate that is quietly empty.
fn check_containers<M: ItemMaker>(maker: Res<M>, registries: Res<Registries>) {
    let mut problems = Vec::new();
    for (_, def) in registries.props.iter() {
        let Some(container) = def.container.as_ref() else { continue };
        for row in &container.contents {
            match &row.what {
                Stock::Item(name) if maker.id_of(name).is_none() => problems.push(format!("{} holds {name:?}, which is no item", def.name)),
                Stock::Tag(tag) if !maker.table().carries(*tag) => {
                    problems.push(format!("{} asks for anything tagged {:?}, and nothing in the loot table carries it", def.name, registries.tags.name(*tag)))
                }
                _ => {}
            }
        }
    }
    assert!(problems.is_empty(), "LootPlugin: {}", problems.join("; "));
}

/// Present once a [`LootPlugin`] answers containers, which
/// [`PropsPlugin`](crate::props::PropsPlugin) checks for before it lets a
/// container that asks for a tag into play.
#[derive(Resource, Debug, Default)]
pub struct ContainersAnswered;

/// Loot: a place's floor on the arrival that built it, the streamed
/// surface region by region, what the dead leave, and what containers
/// hold, all made through the game's [`ItemMaker`] `M`.
///
/// Opt-in, and for one maker: a game adds it once, for the resource that
/// holds its items. Needs `M` itself, which it asks for with a hint, and
/// the run's seed, from which its streams derive. Answers
/// [`FillContainer`], so a game with it answers no container of its own.
pub struct LootPlugin<M>(PhantomData<M>);

impl<M> Default for LootPlugin<M> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<M: ItemMaker> Plugin for LootPlugin<M> {
    fn build(&self, app: &mut App) {
        use crate::plugin::{EngineSet, Needs, Reads, TurnSet};
        use crate::props::PropSet;
        use crate::seed::AddStream;
        app.needs::<M>("LootPlugin", "the resource holding the game's items, implementing `ItemMaker`, inserted before play begins")
            .needs::<Registries>("LootPlugin", "`Registries`, whose props say what containers hold")
            .add_stream::<LootRng>("LootPlugin")
            .init_resource::<Scattered>()
            .init_resource::<ContainersAnswered>()
            .add_message::<FillContainer>()
            .add_message::<ChunkLoaded>()
            .add_message::<PlaceEntered>()
            .reads::<DeathEvent>()
            .configure_sets(crate::plugin::Turn, (LootSet::Scatter, LootSet::Drop).in_set(TurnSet::React))
            .add_systems(crate::plugin::Turn, scatter_places::<M>.in_set(LootSet::Scatter))
            .add_systems(crate::plugin::Turn, drop_on_death::<M>.in_set(LootSet::Drop))
            // A region is scattered as it streams in, which is outside any
            // turn, and a container is filled in the stage the engine keeps
            // for answering what props asked.
            .add_systems(Update, scatter_regions::<M>.in_set(EngineSet::Stream).after(crate::world::stream_chunks))
            .add_systems(Update, fill_containers::<M>.in_set(PropSet::Fill))
            .add_systems(OnEnter(crate::state::EngineState::Playing), check_containers::<M>);
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "LootPlugin");
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::combat::DeathEvent;
    use crate::items::{Item, Stack};
    use crate::places::PlaceBuild;
    use crate::plugin::{Turn, headless_app};
    use crate::props::{PropsPlugin, spawn_prop};
    use crate::state::EngineState;
    use rl_grid::{Terrain, TileRegistry};
    use rl_rules::loot::{DropRow, LootRow};
    use rl_rules::{Named, Names, Registry, TagDef, TagId};

    /// A test game's item: a name, its tags, and whether it stacks.
    pub(crate) struct Toy {
        name: &'static str,
        tags: Vec<TagId>,
        stack: bool,
    }

    impl Named for Toy {
        fn name(&self) -> &str {
            self.name
        }
    }

    /// How a made thing was found, so a test can see what the engine said.
    #[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct FoundAs(pub(crate) Found);

    /// A test game's item registry: blades that improve with the band,
    /// plate from band two, and slugs in handfuls. The band of a place is
    /// its map number, and of a region its x.
    #[derive(Resource)]
    pub(crate) struct Toys {
        defs: Registry<Toy>,
        table: LootTable<Id<Toy>>,
        rules: ScatterRules,
    }

    impl ItemMaker for Toys {
        type Def = Toy;

        fn make(&self, commands: &mut Commands, _: &Registries, def: Id<Toy>, count: u32, found: Found, _: &mut StdRng) -> Vec<Entity> {
            let toy = self.defs.get(def);
            let made = (Item, Name::new(toy.name), FoundAs(found));
            if toy.stack {
                vec![commands.spawn((made, Stack { key: u64::from(def.raw()), count })).id()]
            } else {
                (0..count).map(|_| commands.spawn(made.clone()).id()).collect()
            }
        }

        fn id_of(&self, name: &str) -> Option<Id<Toy>> {
            self.defs.id(name)
        }

        fn table(&self) -> &LootTable<Id<Toy>> {
            &self.table
        }

        fn scatter(&self) -> ScatterRules {
            self.rules.clone()
        }

        fn band(&self, area: LootArea) -> i32 {
            match area {
                LootArea::Place(map) => map.0 as i32,
                LootArea::Region(region) => region.x,
            }
        }
    }

    const PROPS: &str = r#"#![enable(implicit_some)]
    [
        (name: "armory", glyph: '&', color: (r: 1, g: 2, b: 3), blocks: true, offers: [(verb: "open")],
         container: (contents: [(tag: "weapon", count: 1, band: 2), (tag: "armor", count: 2), (item: "slug", count: 5)])),
    ]"#;

    /// The toy registry over `tags`, which names a weapon and an armor,
    /// laying a place's floor by `rules`. Shared with the prefab tests,
    /// whose item slots draw from the same table.
    pub(crate) fn toys(tags: &Registry<TagDef>, rules: ScatterRules) -> Toys {
        let (weapon, armor) = (tags.expect("weapon"), tags.expect("armor"));
        let defs = Registry::from_defs(vec![
            Toy { name: "blade", tags: vec![weapon], stack: false },
            Toy { name: "saber", tags: vec![weapon], stack: false },
            Toy { name: "lance", tags: vec![weapon], stack: false },
            Toy { name: "plate", tags: vec![armor], stack: false },
            Toy { name: "slug", tags: Vec::new(), stack: true },
        ])
        .unwrap();
        let rows = [("blade", 1, 3), ("saber", 4, 6), ("lance", 7, 9), ("plate", 2, 9)]
            .into_iter()
            .map(|(name, min, max)| LootRow::new(defs.expect(name)).bands(min, max).tagged(defs.get(defs.expect(name)).tags.clone()))
            .chain([LootRow::new(defs.expect("slug")).bands(1, 9).weight(3).group(2, 4)])
            .collect();
        let table = LootTable::new(rows, |id| defs.get(id).name.to_string());
        Toys { defs, table, rules }
    }

    /// An app with loot over the toy registry, the props above, the test
    /// seed, and `rules` for a place's floor.
    fn app(rules: ScatterRules) -> App {
        let mut app = headless_app();
        app.add_plugins((PropsPlugin, LootPlugin::<Toys>::default()));
        let tags = Registry::from_defs(vec![TagDef::new("weapon"), TagDef::new("armor")]).unwrap();
        let toys = toys(&tags, rules);
        let props = rl_rules::prop::load(PROPS, &Names::new().tags(&tags)).unwrap();
        app.insert_resource(Registries { tags, props, ..Default::default() }).insert_resource(toys).insert_resource(Seed(crate::testing::TEST_SEED));
        app
    }

    /// Every item on the floor of `map`: its name, where, and how many.
    fn floor(app: &mut App, map: MapId) -> Vec<(String, Point, u32)> {
        let world = app.world_mut();
        let mut q = world.query_filtered::<(&Name, &Position, Option<&OnMap>, Option<&Stack>), With<Item>>();
        q.iter(world)
            .filter(|(.., on, _)| on.map_or(MapId::SURFACE, |m| m.0) == map)
            .map(|(n, at, _, s)| (n.as_str().to_string(), at.0, s.map_or(1, |s| s.count)))
            .collect()
    }

    /// Installs a place of open floor as `map`, with an armory mark in it.
    fn place(app: &mut App, map: MapId) -> Point {
        let tiles = TileRegistry::standard();
        let mut world = WorldMap::new(tiles.tables());
        let mark = Point::new(5, 5);
        world.install_place(
            map,
            PlaceBuild {
                terrain: Terrain::filled(20, 12, tiles.expect("floor")),
                entry: Point::new(1, 1),
                exit: None,
                spots: vec![crate::places::Spot { tag: 'A' as u32, at: mark, prefab: None }],
            },
        );
        world.switch_to(map);
        app.insert_resource(world);
        mark
    }

    fn arrive(app: &mut App, map: MapId, first: bool) {
        app.world_mut().write_message(PlaceEntered { map, first, entry: Point::new(1, 1), exit: None });
        app.world_mut().run_schedule(Turn);
    }

    /// A place's first arrival lays its rules' count beside its mark and
    /// its loose ones, from what applies at its band, each made as found
    /// on a floor; a revisit lays nothing more.
    #[test]
    fn a_place_is_scattered_on_the_arrival_that_built_it_and_never_again() {
        let mut app = app(ScatterRules::new().at_spot('A' as u32, 1).loose(2, 2));
        let deck = MapId(5);
        let mark = place(&mut app, deck);
        app.update();
        arrive(&mut app, deck, true);
        let laid = floor(&mut app, deck);
        assert_eq!(laid.len(), 3, "one beside the mark and two loose: {laid:?}");
        assert!(laid.iter().any(|(_, at, _)| rl_core::geometry::chebyshev(*at, mark) == 1), "beside the mark");
        assert!(laid.iter().all(|(name, _, _)| ["saber", "plate", "slug"].contains(&name.as_str())), "only what applies at band five: {laid:?}");
        assert!(laid.iter().filter(|(n, ..)| n == "slug").all(|(.., count)| (2..=4).contains(count)), "slugs in their handfuls: {laid:?}");
        let world = app.world_mut();
        assert!(world.query::<&FoundAs>().iter(world).all(|f| f.0 == Found::Scatter), "made as found on a floor");

        arrive(&mut app, deck, false);
        assert_eq!(floor(&mut app, deck).len(), 3, "a revisit scatters nothing");
    }

    /// The dead leave what their table rolls, where they died and on their
    /// own map, made as a drop; a row at nought leaves nothing.
    #[test]
    fn the_dead_leave_what_their_drops_roll_where_they_fell() {
        let mut app = app(ScatterRules::new());
        place(&mut app, MapId(2));
        app.update();
        let slug = app.world().resource::<Toys>().defs.expect("slug");
        let plate = app.world().resource::<Toys>().defs.expect("plate");
        let at = Point::new(4, 4);
        let dead = app.world_mut().spawn((Drops::<Toy>(DropTable(vec![DropRow::new(slug, 100).count(3, 3), DropRow::new(plate, 0)])), OnMap(MapId(2)))).id();
        app.world_mut().write_message(DeathEvent { entity: dead, at, credit: None, was_player: false });
        app.world_mut().run_schedule(Turn);
        assert_eq!(floor(&mut app, MapId(2)), vec![("slug".to_string(), at, 3)], "a stack of three where it fell, and no plate");
        let world = app.world_mut();
        assert!(world.query::<&FoundAs>().iter(world).all(|f| f.0 == Found::Drop));
    }

    /// A container asks for kinds of thing: a weapon two bands deeper than
    /// it stands, two pieces of armor, and five slugs, which are one stack.
    /// Past the table's end, the weapon is the deepest one there is.
    #[test]
    fn a_container_draws_its_kinds_at_its_band_and_its_offset() {
        let contents = |map: MapId| {
            let mut app = app(ScatterRules::new());
            place(&mut app, map);
            let registries = app.world().resource::<Registries>().clone();
            let armory = registries.props.expect("armory");
            let prop = spawn_prop(&mut app.world_mut().commands(), &registries, armory, Point::new(8, 8), map);
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            for _ in 0..3 {
                app.update();
            }
            let held = app.world().get::<Inventory>(prop).expect("a container holds a bag").items.clone();
            let world = app.world();
            let mut names: Vec<(String, u32)> =
                held.iter().map(|i| (world.get::<Name>(*i).unwrap().as_str().to_string(), world.get::<Stack>(*i).map_or(1, |s| s.count))).collect();
            names.sort();
            assert!(held.iter().all(|i| world.get::<FoundAs>(*i).map(|f| f.0) == Some(Found::Container)));
            names
        };
        assert_eq!(
            contents(MapId(3)),
            vec![("plate".to_string(), 1), ("plate".to_string(), 1), ("saber".to_string(), 1), ("slug".to_string(), 5)],
            "at band three the weapon is drawn at five, a saber, never a blade"
        );
        assert!(contents(MapId(9)).contains(&("lance".to_string(), 1)), "at nine plus two, past the table, the deepest weapon there is");
    }

    /// A streamed surface is scattered region by region as it loads, and a
    /// region loaded again is remembered and not scattered twice.
    #[test]
    fn a_region_is_scattered_the_first_time_it_loads_and_remembered() {
        let mut app = app(ScatterRules::new().loose(1, 1));
        app.add_plugins(crate::world::StreamingPlugin);
        let start = crate::testing::surface(&mut app);
        app.world_mut().spawn((crate::components::Actor, crate::components::Player, Position(start), crate::components::Viewshed::new(4)));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        for _ in 0..3 {
            app.update();
        }
        let scattered = app.world().resource::<Scattered>().0.clone();
        assert!(!scattered.is_empty(), "the regions around the player were scattered as they loaded");
        let laid = floor(&mut app, MapId::SURFACE).len();
        assert!(laid > 0 && laid <= scattered.len(), "at most one loose item a region: {laid} over {}", scattered.len());

        let again = *scattered.iter().next().unwrap();
        app.world_mut().write_message(ChunkLoaded { region: again });
        app.update();
        assert_eq!(floor(&mut app, MapId::SURFACE).len(), laid, "a region loaded again is not scattered again");
    }

    /// A container naming an item the game never heard of is refused when
    /// play begins, rather than left quietly empty.
    #[test]
    #[should_panic(expected = "which is no item")]
    fn a_container_naming_no_item_is_refused_when_play_begins() {
        let mut app = app(ScatterRules::new());
        let tags = app.world().resource::<Registries>().tags.clone();
        let props = rl_rules::prop::load(
            r#"#![enable(implicit_some)] [(name: "crate", glyph: '&', color: (r: 1, g: 2, b: 3), offers: [(verb: "open")], container: (contents: [(item: "slugg", count: 1)]))]"#,
            &Names::new().tags(&tags),
        )
        .unwrap();
        app.world_mut().resource_mut::<Registries>().props = props;
        place(&mut app, MapId(1));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        app.update();
        app.update();
    }
}
