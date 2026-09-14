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
use rl_core::{Grid2D, Point, geometry};
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

/// Recasts whatever changed and marks every viewshed stale when the
/// light did. Runs after the turns and before sight.
pub fn update_lighting(map: Res<WorldMap>, lighting: Option<ResMut<Lighting>>, sources: Sources, mut viewsheds: Query<&mut Viewshed>) {
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
    statics.sort_unstable();
    dynamics.sort_unstable();

    let mut changed = false;
    let view = map.view();
    if recast_all || lighting.static_dirty || statics != lighting.static_emitters {
        lighting.statics.clear();
        lighting.statics.cast_all(&view, &mut statics, &mut lighting.scratch);
        lighting.static_emitters = statics;
        lighting.static_dirty = false;
        changed = true;
    }
    if recast_all || dynamics != lighting.dynamic_emitters {
        lighting.dynamics.clear();
        lighting.dynamics.cast_all(&view, &mut dynamics, &mut lighting.scratch);
        lighting.dynamic_emitters = dynamics;
        changed = true;
    }
    if changed || lighting.composed_with != lighting.ambient {
        lighting.combined.compose(&lighting.statics, &lighting.dynamics, lighting.ambient);
        lighting.composed_with = lighting.ambient;
        for mut v in &mut viewsheds {
            v.dirty = true;
        }
    }
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
        use crate::plugin::{EngineSet, ResolveSet, Turn};
        app.add_message::<LightEvent>()
            .insert_resource(Lighting::dark())
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

    #[test]
    fn fuel_burns_down_and_the_light_goes_out_once() {
        let mut rig = Rig::new(Some(Lighting::dark()), None);
        let lamp = rig.app.world_mut().spawn((Item, LightSource::new(150, 4, amber()), Fuel(2))).id();
        rig.app.world_mut().get_mut::<Inventory>(rig.player).unwrap().items.push(lamp);
        rig.act(Wait);
        assert!(rig.seen_count() > 9, "lit while it burns");
        rig.act(Wait);
        rig.act(Wait);
        assert!(rig.app.world().get::<LightSource>(lamp).is_none(), "out");
        assert_eq!(rig.app.world().get::<Fuel>(lamp), Some(&Fuel(0)));
        assert_eq!(rig.seen_count(), 9);
        // Headless message buffers linger for a frame or two, so count
        // what is there: one report now, and never a second one.
        let burnt = |app: &App| app.world().resource::<Messages<LightEvent>>().iter_current_update_messages().count();
        assert_eq!(burnt(&rig.app), 1);
        rig.act(Wait);
        rig.act(Wait);
        assert!(burnt(&rig.app) <= 1, "reported once, not every turn");
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
}
