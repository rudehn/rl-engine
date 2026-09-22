//! Light over the loaded window, and the gate it puts on sight.
//!
//! Opt-in: a game that inserts a [`Lighting`] resource gets a light field
//! rebuilt whenever a source moves, changes or burns out, and every
//! viewshed is then cut down to what is lit, within an actor's
//! [`DarkSight`], or adjacent. A game that inserts none sees exactly what
//! it saw before, at no cost.
//!
//! One component, [`LightSource`], serves every kind of thing that glows.
//! On an entity that takes no turns it is a fixture and lives in the
//! static layer, rebuilt only when a fixture or the map changes. On an
//! actor it moves with the actor. On an item it lights the floor where the
//! item lies and, once carried, sheds from the carrier's tile. Whether a
//! thing is lit is the game's call, made by inserting or removing the
//! component; the engine reads only what is there.
//!
//! Ambient is a plain field the game writes: once at start for a dungeon,
//! from a system of its own for a surface with nights. The engine has no
//! clock hook and no notion of a day.

use bevy::prelude::*;
use rl_core::{Grid2D, Point, Rect, geometry};
use rl_grid::{BitGrid, Emitter, Light, LightField, Rgb};

use crate::components::{Actor, Position, Viewshed};
use crate::items::{Inventory, Item};
use crate::places::{MapId, OnMap};
use crate::turn::TurnEnd;
use crate::world::WorldMap;

/// Sheds light from wherever its entity is.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LightSource {
    /// Brightness at the source's own tile, 0 to 255.
    pub intensity: u8,
    /// How far it reaches, in tiles.
    pub radius: i32,
    /// Its hue at full strength.
    pub color: Rgb,
    /// How far it flickers when drawn, 0 for steady to 255 for a flame
    /// that can dip to nothing. Gameplay never reads it.
    #[serde(default)]
    pub flicker: u8,
}

impl LightSource {
    /// A steady source of `intensity` reaching `radius` tiles in `color`.
    pub const fn new(intensity: u8, radius: i32, color: Rgb) -> Self {
        Self { intensity, radius, color, flicker: 0 }
    }

    /// The same source, flickering by `amount` when drawn.
    pub const fn flickering(mut self, amount: u8) -> Self {
        self.flicker = amount;
        self
    }

    fn at(&self, origin: Point) -> Emitter {
        Emitter { origin, intensity: self.intensity, radius: self.radius, color: self.color, flicker: self.flicker }
    }
}

/// How far an actor sees with no light at all. Absent means only what it
/// is touching.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DarkSight(pub i32);

/// Turns of light left. Ticks down every whole turn while the entity
/// sheds light; at zero the engine removes the [`LightSource`] and reports
/// it. The game decides what, if anything, happens to the entity then.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Fuel(pub u32);

/// What the lighting systems report.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightEvent {
    /// A source ran out of fuel and went dark.
    BurntOut {
        /// The entity that was shedding light.
        entity: Entity,
    },
}

/// The intensity a tile needs to count as lit, unless the game says otherwise.
pub const DEFAULT_THRESHOLD: u8 = 16;

/// The light over the loaded window. Inserting it turns lighting on.
#[derive(Resource, Debug, Clone)]
pub struct Lighting {
    /// Light everywhere on the current map, blended in last. The game
    /// writes it; the engine never changes it.
    pub ambient: Light,
    /// The intensity at or above which a tile is seen.
    pub threshold: u8,
    origin: Point,
    statics: LightField,
    dynamics: LightField,
    combined: LightField,
    scratch: BitGrid,
    static_emitters: Vec<Emitter>,
    dynamic_emitters: Vec<Emitter>,
    composed_with: Light,
    generation: Option<u64>,
    epoch: u64,
    static_dirty: bool,
    /// Light shed by cells rather than entities, and the map it is on.
    glow: (MapId, Vec<(Point, LightSource)>),
}

impl Lighting {
    /// Lighting with `ambient` everywhere and nothing cast yet.
    pub fn new(ambient: Light) -> Self {
        Self {
            ambient,
            threshold: DEFAULT_THRESHOLD,
            origin: Point::ZERO,
            statics: LightField::new(0, 0),
            dynamics: LightField::new(0, 0),
            combined: LightField::new(0, 0),
            scratch: BitGrid::new(0, 0),
            static_emitters: Vec::new(),
            dynamic_emitters: Vec::new(),
            composed_with: Light::DARK,
            generation: None,
            epoch: 0,
            static_dirty: true,
            glow: (MapId::SURFACE, Vec::new()),
        }
    }

    /// Pitch black until something is lit.
    pub fn dark() -> Self {
        Self::new(Light::DARK)
    }

    /// The light on world tile `p`; dark outside the window.
    pub fn at(&self, p: Point) -> Light {
        self.combined.at(p - self.origin)
    }

    /// Whether world tile `p` is lit enough to be seen.
    pub fn is_lit(&self, p: Point) -> bool {
        self.at(p).intensity >= self.threshold
    }

    /// The world tile at the field's top-left.
    pub fn origin(&self) -> Point {
        self.origin
    }

    /// Forces the fixtures to be recast on the next update.
    pub fn mark_static_dirty(&mut self) {
        self.static_dirty = true;
    }

    /// Light shed by cells of `map` rather than by entities, replacing what
    /// was set before: a fire's burning cells, which come and go every turn
    /// and so are cast with the sources that move. Nothing is shed while
    /// another map is current.
    pub fn set_glow(&mut self, map: MapId, glow: Vec<(Point, LightSource)>) {
        self.glow = (map, glow);
    }

    /// Matches the fields to the window. Returns whether they were remade.
    fn fit(&mut self, map: &WorldMap) -> bool {
        if self.generation == Some(map.generation()) {
            return false;
        }
        let tiles = map.window_tiles();
        self.generation = Some(map.generation());
        self.origin = tiles.origin();
        if self.combined.width() != tiles.width || self.combined.height() != tiles.height {
            self.statics = LightField::new(tiles.width, tiles.height);
            self.dynamics = LightField::new(tiles.width, tiles.height);
            self.combined = LightField::new(tiles.width, tiles.height);
            self.scratch = BitGrid::new(tiles.width, tiles.height);
        }
        self.static_dirty = true;
        true
    }
}

/// A source and where it stands.
type SourceData = (&'static Position, &'static LightSource, Has<Actor>, Option<&'static OnMap>);
/// Someone who might be carrying a light.
type CarrierData = (&'static Position, &'static Inventory, Option<&'static OnMap>);

/// Everything that sheds light.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Sources<'w, 's> {
    placed: Query<'w, 's, SourceData>,
    carriers: Query<'w, 's, CarrierData, With<Actor>>,
    carried: Query<'w, 's, &'static LightSource, (With<Item>, Without<Position>)>,
}

/// The cells whose light may differ between two emitter lists: the discs
/// of every emitter in one and not the other.
///
/// Both lists are sorted, so this is a merge. `None` means the lists are
/// the same and nothing moved.
fn changed_area(was: &[Emitter], now: &[Emitter]) -> Option<Rect> {
    let mut area: Option<Rect> = None;
    let mut cover = |e: &Emitter| {
        // The rim, where light reaches exactly zero, is still a cell whose
        // value can change, so the disc is taken inclusive of it.
        let disc = Rect::from_corners(e.origin.offset(-e.radius, -e.radius), e.origin.offset(e.radius, e.radius));
        area = Some(match area {
            Some(a) => a.union(&disc),
            None => disc,
        });
    };
    let (mut i, mut j) = (0, 0);
    while i < was.len() || j < now.len() {
        match (was.get(i), now.get(j)) {
            (Some(a), Some(b)) if a == b => {
                i += 1;
                j += 1;
            }
            (Some(a), Some(b)) if a < b => {
                cover(a);
                i += 1;
            }
            (Some(_), Some(b)) => {
                cover(b);
                j += 1;
            }
            (Some(a), None) => {
                cover(a);
                i += 1;
            }
            (None, Some(b)) => {
                cover(b);
                j += 1;
            }
            (None, None) => break,
        }
    }
    area
}

/// A viewer whose sight the light may have cut differently: where it
/// stands and how far it sees.
type Lit = (Option<&'static Position>, &'static mut Viewshed);

/// Recasts whatever changed and marks stale the viewsheds the change could
/// reach. Runs after the turns and before sight.
///
/// Only those it could reach: a viewshed is cut down to what is lit inside
/// its own range, so a lamp that moved at the far end of the window cannot
/// change what a mind twenty tiles away sees. Marking every viewshed
/// instead cost a shadowcast per actor on every frame in which any light
/// moved, which in a lit game is every frame the player walks: measured at
/// 11 microseconds per actor per frame by `crates/rl-ui/benches/frame.rs`,
/// or a frame three times over at sixty-four of them.
pub fn update_lighting(map: Res<WorldMap>, lighting: Option<ResMut<Lighting>>, sources: Sources, mut viewsheds: Query<Lit>) {
    let Some(mut lighting) = lighting else { return };
    let lighting = &mut *lighting;
    let here = map.current();
    let on_map = |on: Option<&OnMap>| on.map(|m| m.0).unwrap_or(MapId::SURFACE) == here;
    let mut recast_all = lighting.fit(&map);
    if lighting.epoch != map.opacity_epoch() {
        lighting.epoch = map.opacity_epoch();
        recast_all = true;
    }
    if lighting.combined.width() == 0 || lighting.combined.height() == 0 {
        return;
    }
    let origin = lighting.origin;
    let local = |p: Point| p - origin;

    let mut statics = Vec::new();
    let mut dynamics = Vec::new();
    for (pos, source, is_actor, on) in &sources.placed {
        if !on_map(on) {
            continue;
        }
        if is_actor { &mut dynamics } else { &mut statics }.push(source.at(local(pos.0)));
    }
    for (pos, bag, on) in &sources.carriers {
        if !on_map(on) {
            continue;
        }
        for item in &bag.items {
            if let Ok(source) = sources.carried.get(*item) {
                dynamics.push(source.at(local(pos.0)));
            }
        }
    }
    if lighting.glow.0 == here {
        dynamics.extend(lighting.glow.1.iter().map(|(p, source)| source.at(local(*p))));
    }
    statics.sort_unstable();
    dynamics.sort_unstable();

    // What moved, before the lists are replaced. `None` where the whole
    // field is being rebuilt anyway, which is every viewshed's business.
    let mut changed = false;
    let mut touched: Option<Rect> = None;
    let mut everywhere = recast_all;
    let view = map.view();
    if recast_all || lighting.static_dirty || statics != lighting.static_emitters {
        touched = union(touched, changed_area(&lighting.static_emitters, &statics));
        // A dirty flag says the map changed under the fixtures rather than
        // that a fixture moved, so the area it touches is not knowable.
        everywhere |= lighting.static_dirty;
        lighting.statics.clear();
        lighting.statics.cast_all(&view, &mut statics, &mut lighting.scratch);
        lighting.static_emitters = statics;
        lighting.static_dirty = false;
        changed = true;
    }
    if recast_all || dynamics != lighting.dynamic_emitters {
        touched = union(touched, changed_area(&lighting.dynamic_emitters, &dynamics));
        lighting.dynamics.clear();
        lighting.dynamics.cast_all(&view, &mut dynamics, &mut lighting.scratch);
        lighting.dynamic_emitters = dynamics;
        changed = true;
    }
    if changed || lighting.composed_with != lighting.ambient {
        // Ambient lands on every cell, so a change to it reaches everyone.
        everywhere |= lighting.composed_with != lighting.ambient;
        lighting.combined.compose(&lighting.statics, &lighting.dynamics, lighting.ambient);
        lighting.composed_with = lighting.ambient;
        for (pos, mut v) in &mut viewsheds {
            v.dirty |= everywhere || reaches(touched, pos.map(|p| local(p.0)), v.range);
        }
    }
}

/// The smaller rectangle covering both, where there are two.
fn union(a: Option<Rect>, b: Option<Rect>) -> Option<Rect> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(&b)),
        (a, b) => a.or(b),
    }
}

/// Whether a viewer at `at` seeing `range` tiles could have had its sight
/// cut differently by a change over `touched`.
///
/// A viewer whose cell nobody knows is taken as reachable: it is one
/// entity, and guessing wrong the other way is sight that never updates.
fn reaches(touched: Option<Rect>, at: Option<Point>, range: i32) -> bool {
    let (Some(touched), Some(at)) = (touched, at) else { return true };
    Rect::from_corners(at.offset(-range, -range), at.offset(range, range)).intersects(&touched)
}

/// Cuts a computed line of sight down to what light, dark sight or touch
/// lets `viewer` at `at` actually see.
pub fn gate(lighting: &Lighting, at: Point, dark_sight: i32, viewshed: &mut Viewshed) {
    let reach = dark_sight.max(1);
    let origin = viewshed.origin;
    let (visible, line) = (&mut viewshed.visible, &viewshed.line);
    visible.clear();
    for idx in line.iter_indices() {
        let p = line.idx_point(idx) + origin;
        if lighting.is_lit(p) || geometry::chebyshev(at, p) <= reach {
            visible.insert_idx(idx);
        }
    }
}

/// Whether `observer` at `from`, with `dark_sight`, perceives a target at
/// `to` that it has a line to. With no lighting, always.
pub fn perceives(lighting: Option<&Lighting>, from: Point, dark_sight: i32, to: Point) -> bool {
    lighting.is_none_or(|l| l.is_lit(to) || geometry::chebyshev(from, to) <= dark_sight.max(1))
}

/// Burns fuel every whole turn and puts out what runs dry.
pub fn tick_fuel(
    mut commands: Commands,
    mut ends: MessageReader<TurnEnd>,
    mut burning: Query<(Entity, &mut Fuel), With<LightSource>>,
    mut events: MessageWriter<LightEvent>,
) {
    let turns = ends.read().count() as u32;
    if turns == 0 {
        return;
    }
    for (entity, mut fuel) in &mut burning {
        fuel.0 = fuel.0.saturating_sub(turns);
        if fuel.0 == 0 {
            commands.entity(entity).remove::<LightSource>();
            events.write(LightEvent::BurntOut { entity });
        }
    }
}

/// Lighting: what each source sheds, what fuel costs, and the gate that
/// decides what a viewshed actually sees. Dark until a game says
/// otherwise.
pub struct LightingPlugin;

impl Plugin for LightingPlugin {
    fn build(&self, app: &mut App) {
        use crate::plugin::{EngineSet, ResetsOnNewRun, ResolveSet, Turn};
        app.add_message::<LightEvent>()
            .insert_resource(Lighting::dark())
            // Dark rather than the ambient the game chose: a new run writes
            // its own ambient in `NewRun`, as the first one did.
            .reset_on_new_run_with::<Lighting>(Lighting::dark)
            .add_systems(Update, update_lighting.in_set(EngineSet::Light))
            .add_systems(Turn, tick_fuel.in_set(ResolveSet::Effects));
    }

    fn finish(&self, app: &mut App) {
        crate::plugin::depends_on::<crate::plugin::CorePlugin>(app, "LightingPlugin");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{Blocks, MyTurn, Player, RevealsMap};
    use crate::items::{DropItem, PickUp};
    use crate::knowledge::Knowledge;
    use crate::places::{PlaceBuild, PlaceRules, PlaceRulesRes, WarpRequest};
    use crate::plugin::headless_app;
    use crate::state::EngineState;
    use crate::turn::{Action, Intent};
    use crate::turn::{Step, Wait};
    use bevy::ecs::system::RunSystemOnce;
    use rl_grid::{Terrain, TileId, TileRegistry};
    use rl_mapgen::BuildError;
    use rl_world::WorldGraph;

    const ROOM: [&str; 9] = [
        "####################",
        "#..................#",
        "#..................#",
        "#..................#",
        "#..................#",
        "#..................#",
        "#..................#",
        "#..................#",
        "####################",
    ];

    /// One dark room, whatever map is asked for.
    struct Room(TileRegistry);

    impl PlaceRules for Room {
        fn build(&self, _: MapId, _: Option<&WorldGraph>) -> Result<PlaceBuild, BuildError> {
            let (wall, floor) = (self.0.expect("wall"), self.0.expect("floor"));
            let terrain = Terrain::from_fn(20, 9, |p| if ROOM[p.y as usize].as_bytes()[p.x as usize] == b'#' { wall } else { floor });
            Ok(PlaceBuild { terrain, entry: Point::new(5, 4), exit: None, spots: Vec::new() })
        }
    }

    struct Rig {
        app: App,
        player: Entity,
        wall: TileId,
    }

    impl Rig {
        fn new(lighting: Option<Lighting>, dark_sight: Option<i32>) -> Self {
            let mut app = headless_app();
            app.add_plugins((crate::fov::FovPlugin, crate::items::ItemsPlugin));
            let tiles = TileRegistry::standard();
            let wall = tiles.expect("wall");
            app.insert_resource(WorldMap::new(tiles.tables()));
            app.insert_resource(PlaceRulesRes(Box::new(Room(tiles))));
            // Lighting is a plugin: without it there is no gate at all,
            // which is what a game that never lights anything gets.
            if let Some(l) = lighting {
                app.add_plugins(LightingPlugin);
                app.insert_resource(l);
            }
            let mut player = app.world_mut().spawn((Actor, Player, Blocks, Position(Point::ZERO), Viewshed::new(12), RevealsMap, Inventory::default()));
            if let Some(d) = dark_sight {
                player.insert(DarkSight(d));
            }
            let player = player.id();
            app.world_mut().write_message(WarpRequest::into_place(player, MapId(1)));
            app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
            app.update();
            app.update();
            assert!(app.world().get::<MyTurn>(player).is_some());
            Self { app, player, wall }
        }

        fn act<A: Action>(&mut self, action: A) {
            self.app.world_mut().write_message(Intent { actor: self.player, action });
            self.app.update();
        }

        fn sees(&self, p: Point) -> bool {
            self.app.world().get::<Viewshed>(self.player).unwrap().can_see(p)
        }

        fn seen_count(&self) -> usize {
            self.app.world().get::<Viewshed>(self.player).unwrap().visible.count()
        }

        fn in_line(&self, p: Point) -> bool {
            self.app.world().get::<Viewshed>(self.player).unwrap().in_line(p)
        }
    }

    fn amber() -> Rgb {
        Rgb::new(255, 150, 40)
    }

    #[test]
    fn without_lighting_everything_in_line_is_seen() {
        let rig = Rig::new(None, None);
        let v = rig.app.world().get::<Viewshed>(rig.player).unwrap();
        assert!(v.line.count() > 9);
        assert_eq!(v.visible.count(), v.line.count());
    }

    #[test]
    fn in_the_dark_the_player_sees_only_what_it_touches() {
        let rig = Rig::new(Some(Lighting::dark()), None);
        assert_eq!(rig.seen_count(), 9, "itself and its eight neighbours");
        assert!(rig.in_line(Point::new(15, 4)) && !rig.sees(Point::new(15, 4)));
        assert!(!rig.app.world().resource::<Knowledge>().is_explored(Point::new(15, 4)), "not remembered until lit");
    }

    #[test]
    fn dark_sight_reaches_further_than_touch() {
        let rig = Rig::new(Some(Lighting::dark()), Some(4));
        assert!(rig.sees(Point::new(9, 4)));
        assert!(!rig.sees(Point::new(10, 4)));
    }

    #[test]
    fn ambient_lights_the_whole_room() {
        let rig = Rig::new(Some(Lighting::new(Light::white(40))), None);
        let v = rig.app.world().get::<Viewshed>(rig.player).unwrap();
        assert_eq!(v.visible.count(), v.line.count());
    }

    #[test]
    fn a_fixture_lights_what_it_can_see_until_a_wall_goes_up() {
        let mut rig = Rig::new(Some(Lighting::dark()), None);
        let brazier = Point::new(14, 4);
        rig.app.world_mut().spawn((Position(brazier), LightSource::new(200, 5, amber())));
        rig.act(Wait);
        assert!(rig.sees(brazier), "the brazier's own tile is lit");
        assert!(rig.sees(Point::new(12, 4)));
        assert!(!rig.sees(Point::new(8, 4)), "beyond its radius");
        let light = rig.app.world().resource::<Lighting>();
        assert_eq!(light.at(brazier).intensity, 200);
        assert_eq!(light.at(brazier).color, amber().scaled(200));
        assert_eq!(light.at(Point::new(5, 4)).intensity, 0);
        // A wall goes up between them; nobody moves, and the view still updates.
        let wall = rig.wall;
        for y in 1..8 {
            rig.app.world_mut().resource_mut::<WorldMap>().set_tile(Point::new(11, y), wall);
        }
        rig.app.update();
        assert!(!rig.sees(brazier), "shadowed now");
        let light = rig.app.world().resource::<Lighting>();
        assert_eq!(light.at(Point::new(10, 4)).intensity, 0, "the light is cut too");
        // The brazier still lights its own side, but a lit tile the
        // player has no line to is not seen and not drawn.
        assert!(light.at(Point::new(13, 4)).intensity > 0);
        assert!(!rig.in_line(Point::new(13, 4)) && !rig.sees(Point::new(13, 4)));
    }

    #[test]
    fn a_carried_light_moves_with_the_carrier_and_lies_where_dropped() {
        let mut rig = Rig::new(Some(Lighting::dark()), None);
        let torch = rig.app.world_mut().spawn((Item, Position(Point::new(7, 4)), LightSource::new(180, 4, amber()))).id();
        rig.act(Wait);
        assert!(rig.sees(Point::new(9, 4)), "lit by the torch on the floor");
        rig.act(Step(rl_core::Direction::East));
        rig.act(Step(rl_core::Direction::East));
        rig.act(PickUp);
        assert!(rig.app.world().get::<Inventory>(rig.player).unwrap().contains(torch));
        assert_eq!(rig.app.world().resource::<Lighting>().at(Point::new(7, 4)).intensity, 180, "shed from the carrier");
        rig.act(Step(rl_core::Direction::West));
        rig.act(Step(rl_core::Direction::West));
        assert_eq!(rig.app.world().resource::<Lighting>().at(Point::new(5, 4)).intensity, 180);
        assert!(rig.sees(Point::new(8, 4)) && !rig.sees(Point::new(10, 4)));
        rig.act(DropItem(torch));
        rig.act(Step(rl_core::Direction::West));
        rig.act(Step(rl_core::Direction::West));
        rig.act(Step(rl_core::Direction::West));
        let light = rig.app.world().resource::<Lighting>();
        assert_eq!(light.at(Point::new(5, 4)).intensity, 180, "still burning where it lies");
        assert_eq!(light.at(Point::new(1, 4)).intensity, 0, "four tiles off, the rim");
    }

    /// How many `LightEvent`s have been read, by a reader that sees each
    /// exactly once.
    #[derive(Resource, Default)]
    struct BurntOut(usize);

    fn count_burnt(mut events: MessageReader<LightEvent>, mut burnt: ResMut<BurntOut>) {
        burnt.0 += events.read().count();
    }

    #[test]
    fn fuel_burns_down_and_the_light_goes_out_once() {
        let mut rig = Rig::new(Some(Lighting::dark()), None);
        // Counted through a reader rather than by peeking at the buffer:
        // when a headless app swaps its message buffers depends on wall
        // time, so a peek sees a message on one frame, two, or none, and
        // this test failed on the frames it saw none.
        rig.app.init_resource::<BurntOut>().add_systems(PostUpdate, count_burnt);
        let lamp = rig.app.world_mut().spawn((Item, LightSource::new(150, 4, amber()), Fuel(2))).id();
        rig.app.world_mut().get_mut::<Inventory>(rig.player).unwrap().items.push(lamp);
        rig.act(Wait);
        assert!(rig.seen_count() > 9, "lit while it burns");
        rig.act(Wait);
        rig.act(Wait);
        assert!(rig.app.world().get::<LightSource>(lamp).is_none(), "out");
        assert_eq!(rig.app.world().get::<Fuel>(lamp), Some(&Fuel(0)));
        assert_eq!(rig.seen_count(), 9);
        assert_eq!(rig.app.world().resource::<BurntOut>().0, 1, "reported when it went out");
        rig.act(Wait);
        rig.act(Wait);
        assert_eq!(rig.app.world().resource::<BurntOut>().0, 1, "and never again");
    }

    #[test]
    fn perception_follows_the_light_and_dark_sight() {
        let dark = Lighting::dark();
        let here = Point::new(3, 3);
        assert!(perceives(None, here, 0, Point::new(9, 9)), "no lighting, no gate");
        assert!(perceives(Some(&dark), here, 0, Point::new(4, 4)), "adjacent");
        assert!(!perceives(Some(&dark), here, 0, Point::new(5, 3)));
        assert!(perceives(Some(&dark), here, 3, Point::new(6, 3)));
        assert!(!perceives(Some(&dark), here, 3, Point::new(7, 3)));
    }

    /// The unit the bound is built on, tested without an `App`: the area a
    /// change touched, and who it can reach.
    #[test]
    fn a_lamp_that_moved_touches_its_two_discs_and_reaches_only_viewers_whose_range_meets_them() {
        let lamp = |x: i32| Emitter { origin: Point::new(x, 0), intensity: 200, radius: 4, color: rl_grid::Rgb::new(255, 255, 255), flicker: 0 };

        assert_eq!(changed_area(&[lamp(0)], &[lamp(0)]), None, "nothing moved, nothing touched");

        // Moved from 0 to 20: both discs, since one cell went dark and the
        // other lit, and nothing between them changed.
        let touched = changed_area(&[lamp(0)], &[lamp(20)]).expect("it moved");
        assert!(touched.contains(Point::new(0, 0)) && touched.contains(Point::new(20, 0)), "{touched:?} covers where it was and where it is");

        // A viewer standing on the old cell sees the change; one far off to
        // the side, whose whole range is outside both discs, does not.
        assert!(reaches(Some(touched), Some(Point::new(0, 0)), 8), "the one it left");
        assert!(reaches(Some(touched), Some(Point::new(20, 0)), 8), "the one it reached");
        assert!(!reaches(Some(touched), Some(Point::new(10, 60)), 8), "sixty tiles away, and its range does not meet either disc");
        assert!(reaches(Some(touched), None, 8), "a viewer nobody can place is recast rather than left wrong");
        assert!(reaches(None, Some(Point::new(10, 60)), 8), "no area known means everyone, which is what a rebuilt field gets");

        // A lamp that appeared, and one that went out, each touch their own disc.
        let lit = changed_area(&[], &[lamp(5)]).expect("it appeared");
        assert!(lit.contains(Point::new(5, 0)));
        let out = changed_area(&[lamp(5)], &[]).expect("it went out");
        assert_eq!(out, lit, "going out touches exactly what coming on did");
    }

    /// And the whole of it, through the app: a lamp moving at one end of
    /// the window leaves a mind at the other end alone, and moving beside
    /// it does not.
    #[test]
    fn a_light_that_moved_marks_only_the_viewsheds_it_could_have_changed() {
        let mut app = headless_app();
        app.add_plugins((crate::fov::FovPlugin, crate::world::StreamingPlugin, LightingPlugin));
        let start = crate::testing::surface(&mut app);
        app.insert_resource(crate::seed::Seed(crate::testing::TEST_SEED));
        let player = app.world_mut().spawn((Actor, Player, Blocks, Position(start), Viewshed::new(8), RevealsMap)).id();
        let near = app.world_mut().spawn((Actor, Position(start.offset(6, 0)), Viewshed::new(8))).id();
        let far = app.world_mut().spawn((Actor, Position(start.offset(60, 0)), Viewshed::new(8))).id();
        // The lamp the player carries.
        app.world_mut().entity_mut(player).insert(LightSource::new(200, 6, rl_grid::Rgb::new(255, 255, 255)));
        app.world_mut().resource_mut::<NextState<EngineState>>().set(EngineState::Playing);
        for _ in 0..4 {
            app.update();
        }

        let clean = |app: &mut App| {
            for e in [player, near, far] {
                app.world_mut().get_mut::<Viewshed>(e).expect("a viewshed").dirty = false;
            }
        };
        clean(&mut app);
        // The lamp moves one cell, by hand, so nothing else in the frame
        // touches sight.
        app.world_mut().get_mut::<Position>(player).expect("somewhere").0 = start.offset(1, 0);
        app.world_mut().run_system_once(update_lighting).expect("the light ran");

        assert!(app.world().get::<Viewshed>(player).unwrap().dirty, "the one carrying it");
        assert!(app.world().get::<Viewshed>(near).unwrap().dirty, "and one whose range meets the light it moved");
        assert!(!app.world().get::<Viewshed>(far).unwrap().dirty, "but not one sixty tiles away, which is what cost a shadowcast per actor a frame");
    }
}
