//! The map, drawn from the player's point of view.
//!
//! Visible tiles are drawn lit, remembered tiles in the colours of
//! memory, unknown tiles blank. What a tile looks like is the game's
//! business: it fills a [`TileAppearance`] table keyed by [`TileId`], with
//! a glyph, both colours, and how the tile varies from cell to cell. With
//! lighting turned on, a visible tile's glyph and background and whatever
//! stands on it are coloured by the light that lands there; see
//! [`shade`](crate::shade) for how.
//!
//! Fire and gas are drawn over the tiles they are on, from
//! [`FieldAppearance`]: a flame over every burning cell with embers rising
//! off it, and the ground tinted by the densest gas on it, thick enough to
//! hide behind drawn as a haze that grows heavier the thicker it is. What
//! is drawn is what [`ShownFields`](crate::fields::ShownFields) shows,
//! blended in and stirred, rather than the fields as they stand. Only on
//! tiles in sight; memory holds no smoke.

use bevy::color::Mix;
use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::Grid2D;
use rl_core::{Point, Rect};
use rl_grid::{Light, TileId};
use rl_rules::GasId;

use crate::fields::{ShownFields, Wavefronts, churn, track_fields};
use crate::shade::{Memory, Shading, Vary};
use crate::terminal::{Cell, Terminal};

/// How an entity is drawn.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    /// The character.
    pub ch: char,
    /// Its colour.
    pub fg: Color,
    /// Draw order among entities on one tile; higher wins.
    pub layer: i32,
}

impl Glyph {
    /// A glyph on the default layer.
    pub fn new(ch: char, fg: Color) -> Self {
        Self { ch, fg, layer: 0 }
    }

    /// The same glyph on `layer`.
    pub fn on_layer(mut self, layer: i32) -> Self {
        self.layer = layer;
        self
    }
}

/// What each tile looks like in full light, indexed by [`TileId`], and
/// how light and memory change it.
///
/// Filled from a file by [`load`](Self::load), or by hand with
/// [`set_varied`](Self::set_varied) for a game whose tiles are few.
#[derive(Resource, Debug, Clone, Default)]
pub struct TileAppearance {
    looks: Vec<Option<(Cell, Vary)>>,
    /// How a remembered tile is drawn.
    pub memory: Memory,
    /// How light, flicker and jitter become colour.
    pub shading: Shading,
}

impl TileAppearance {
    /// An empty table.
    pub fn new() -> Self {
        Self { looks: Vec::new(), memory: Memory::default(), shading: Shading::default() }
    }

    /// Sets the appearance of `id`, the same in every cell.
    pub fn set(&mut self, id: TileId, cell: Cell) {
        self.set_varied(id, cell, Vary::NONE);
    }

    /// Sets the appearance of `id`, varying from cell to cell by `vary`.
    pub fn set_varied(&mut self, id: TileId, cell: Cell, vary: Vary) {
        let i = id.index();
        if self.looks.len() <= i {
            self.looks.resize(i + 1, None);
        }
        self.looks[i] = Some((cell, vary));
    }

    /// The authored appearance of `id`; a magenta question mark for an id
    /// the game never described, so the gap is visible rather than blank.
    pub fn lit(&self, id: TileId) -> Cell {
        self.look(id).0
    }

    fn look(&self, id: TileId) -> (Cell, Vary) {
        self.looks.get(id.index()).copied().flatten().unwrap_or((Cell::new('?', Color::srgb(1.0, 0.0, 1.0)), Vary::NONE))
    }

    /// `id` as seen at `p` at time `t`, before light: jittered per cell.
    pub fn seen(&self, id: TileId, p: Point, t: f32) -> Cell {
        let (cell, vary) = self.look(id);
        Cell { glyph: cell.glyph, fg: self.shading.vary(cell.fg, vary, p, t), bg: self.shading.vary(cell.bg, vary, p, t) }
    }

    /// `id` as remembered at `p`: jittered as it was seen, then faded.
    pub fn remembered(&self, id: TileId, p: Point) -> Cell {
        let (cell, vary) = self.look(id);
        let still = Vary { shimmer: 0.0, ..vary };
        let fade = |c| self.memory.recall(self.shading.vary(c, still, p, 0.0));
        Cell { glyph: cell.glyph, fg: fade(cell.fg), bg: fade(cell.bg) }
    }

    /// `cell` under `light` at `p` at time `t`: both colours coloured by it.
    pub fn under(&self, cell: Cell, light: Light, p: Point, t: f32) -> Cell {
        Cell { glyph: cell.glyph, fg: self.shading.light(cell.fg, light, p, t), bg: self.shading.light(cell.bg, light, p, t) }
    }
}

/// How fire and gas look over the tiles they are on.
///
/// Fire is the engine's, so it has a look of its own a game only recolours;
/// gases are the game's, and each is a tint the game names, grey until it
/// does.
#[derive(Resource, Debug, Clone)]
pub struct FieldAppearance {
    /// A burning cell: the flame's glyph and colour over the embers.
    pub flame: Cell,
    /// The colour a flame flickers toward, and embers rise in.
    pub flare: Color,
    /// The glyphs gas thick enough to hide behind is drawn with, thinnest
    /// first: the thicker the gas, the further along.
    pub haze: Vec<char>,
    /// What rises off a flame now and then into the cell over it.
    pub ember: char,
    gases: Vec<Option<Color>>,
}

impl Default for FieldAppearance {
    fn default() -> Self {
        Self {
            flame: Cell::new('^', Color::srgb(1.0, 0.55, 0.15)).on(Color::srgb(0.4, 0.09, 0.02)),
            flare: Color::srgb(1.0, 0.9, 0.4),
            // No heavier shade: in a terminal font the heavy blocks leave a
            // gap between columns, and a thick cloud drawn in them reads as
            // bars.
            haze: vec!['░', '▒'],
            ember: '\'',
            gases: Vec::new(),
        }
    }
}

impl FieldAppearance {
    /// Tints the ground under `gas` toward `tint`.
    pub fn set_gas(&mut self, gas: GasId, tint: Color) {
        if self.gases.len() <= gas.index() {
            self.gases.resize(gas.index() + 1, None);
        }
        self.gases[gas.index()] = Some(tint);
    }

    /// The tint of `gas`, grey for a gas nobody coloured.
    pub fn gas(&self, gas: GasId) -> Color {
        self.gases.get(gas.index()).copied().flatten().unwrap_or(Color::srgb(0.62, 0.62, 0.6))
    }

    /// `cell` as `amount` of a gas tinted `tint` over it; `hides` when it is
    /// thick enough to hide what is behind it, and then drawn in the haze
    /// its thickness picks, the heaviest only near full.
    pub fn under_gas(&self, mut cell: Cell, tint: Color, amount: u8, hides: bool) -> Cell {
        let share = f32::from(amount) / 255.0;
        cell.bg = cell.bg.mix(&tint, 0.15 + share * 0.6);
        if hides && !self.haze.is_empty() {
            let step = ((share * share * self.haze.len() as f32) as usize).min(self.haze.len() - 1);
            cell.glyph = self.haze[step];
            cell.fg = tint.mix(&Color::WHITE, 0.3);
        }
        cell
    }

    /// `cell`, over a burning one, at `p` at time `t`: now and then an
    /// ember rising through it, a few cells in a dozen at any moment.
    pub fn over_flame(&self, mut cell: Cell, p: Point, t: f32) -> Cell {
        if rl_core::seed::position_hash((t * 8.0) as u64, p.x, p.y).is_multiple_of(9) {
            cell.glyph = self.ember;
            cell.fg = self.flare;
        }
        cell
    }

    /// A burning cell at `p` at time `t`, flickering cell by cell.
    pub fn burning(&self, p: Point, t: f32) -> Cell {
        let beat = rl_core::seed::position_hash((t * 12.0) as u64, p.x, p.y) % 100;
        Cell { glyph: self.flame.glyph, fg: self.flame.fg.mix(&self.flare, beat as f32 / 160.0), bg: self.flame.bg }
    }
}

/// Where on the terminal the map is drawn, and which world tile sits at
/// its top-left.
#[derive(Resource, Debug, Clone, Copy)]
pub struct MapView {
    /// The terminal cells the map occupies.
    pub viewport: Rect,
    /// The world tile drawn at the viewport's top-left.
    pub origin: Point,
}

impl MapView {
    /// A view filling `viewport`.
    pub fn new(viewport: Rect) -> Self {
        Self { viewport, origin: Point::ZERO }
    }

    /// Centres the view on `p`.
    pub fn center_on(&mut self, p: Point) {
        self.origin = Point::new(p.x - self.viewport.width / 2, p.y - self.viewport.height / 2);
    }

    /// Pulls the view back inside `bounds`, so it never shows void past
    /// the edge of the map.
    ///
    /// Centring alone is not enough on a map close to the size of the
    /// viewport: standing near an edge puts the map's rim mid-screen and
    /// spends the rest on nothing. A map smaller than the viewport is
    /// centred in it instead, since there is nothing to scroll.
    pub fn clamp_to(&mut self, bounds: Rect) {
        let slack = Point::new(bounds.width - self.viewport.width, bounds.height - self.viewport.height);
        self.origin.x = if slack.x <= 0 { bounds.x + slack.x / 2 } else { self.origin.x.clamp(bounds.x, bounds.x + slack.x) };
        self.origin.y = if slack.y <= 0 { bounds.y + slack.y / 2 } else { self.origin.y.clamp(bounds.y, bounds.y + slack.y) };
    }

    /// The terminal cell a world tile is drawn at, if inside the viewport.
    pub fn to_screen(&self, p: Point) -> Option<Point> {
        let s = p - self.origin + self.viewport.origin();
        self.viewport.contains(s).then_some(s)
    }

    /// The world tile drawn at a terminal cell, if inside the viewport.
    pub fn to_world(&self, s: Point) -> Option<Point> {
        self.viewport.contains(s).then(|| s - self.viewport.origin() + self.origin)
    }
}

/// Draws the map and the entities on it into the terminal every frame.
///
/// Takes the rectangle it draws in, the way every panel does, so a game
/// has no [`MapView`] of its own to insert and cannot forget one. Needs
/// field of view: without it no tile is ever seen or remembered, so the
/// map would draw as nothing at all.
pub struct MapViewPlugin(Rect);

impl MapViewPlugin {
    /// The map, drawn in the terminal cells of `viewport`.
    pub fn new(viewport: Rect) -> Self {
        Self(viewport)
    }
}

impl Plugin for MapViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileAppearance>()
            .init_resource::<FieldAppearance>()
            .init_resource::<ShownFields>()
            .init_resource::<Wavefronts>()
            .insert_resource(MapView::new(self.0))
            .add_systems(Update, (follow_player, track_fields, draw_map).chain().in_set(PresentSet::Map))
            // Before the map is drawn, and in `Update` rather than in a
            // play-only set, because props are put down while a place is
            // built and `Added` matches for one frame only.
            .add_systems(Update, (dress_props, redress_emptied).before(draw_map));
    }

    fn finish(&self, app: &mut App) {
        depends_on::<CorePlugin>(app, "MapViewPlugin");
        depends_on::<FovPlugin>(app, "MapViewPlugin");
    }
}

/// A prop nobody has dressed yet, and which definition to dress it from.
type Undressed<'w, 's> = Query<'w, 's, (Entity, &'static PropKind), (Added<PropKind>, Without<Glyph>)>;

/// A container that has just been emptied, and which definition says what
/// an emptied one looks like.
type JustEmptied<'w, 's> = Query<'w, 's, (Entity, &'static PropKind), Added<Emptied>>;

/// Gives an emptied container the look its definition keeps for one that
/// is done, so a crate already gone through reads as done at a glance.
pub fn redress_emptied(mut commands: Commands, registries: Option<Res<Registries>>, emptied: JustEmptied) {
    let Some(registries) = registries else { return };
    for (entity, kind) in &emptied {
        let Some(look) = registries.props.get(kind.0).container.as_ref().and_then(|c| c.opened) else { continue };
        commands.entity(entity).insert(Glyph { ch: look.glyph, fg: Color::srgb_u8(look.color.r, look.color.g, look.color.b), layer: look.layer });
    }
}

/// Gives a prop the glyph its definition asks for.
///
/// A prop is described twice, as a tile is: what it is, in `props.ron`,
/// which `rl-bevy` reads and which knows nothing of colours on a screen,
/// and how it looks, which is this. The engine spawns a prop with its
/// [`PropKind`] and no glyph, and whoever draws dresses it, so nothing
/// below this crate has to name a `Color`.
///
/// A prop a game dressed itself keeps what it was given: the query asks
/// for those with no glyph, so a game that wants one crate to look
/// different says so and is not overruled.
pub fn dress_props(mut commands: Commands, registries: Option<Res<Registries>>, bare: Undressed) {
    let Some(registries) = registries else { return };
    for (entity, kind) in &bare {
        let look = registries.props.get(kind.0).look;
        commands.entity(entity).insert(Glyph { ch: look.glyph, fg: Color::srgb_u8(look.color.r, look.color.g, look.color.b), layer: look.layer });
    }
}

/// Keeps the view centred on the player, and inside the map.
pub fn follow_player(mut view: ResMut<MapView>, map: Res<WorldMap>, player: Query<&Position, With<Player>>) {
    let Ok(pos) = player.single() else { return };
    view.center_on(pos.0);
    if let Some(place) = map.place(map.current()) {
        view.clamp_to(place.terrain.bounds());
    }
}

/// Whether to draw light intensity as a digit over every visible tile.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LightOverlay(pub bool);

/// How often per second flickering and shimmering cells are redrawn: fast
/// enough to read as motion, slow enough that a still frame costs nothing.
const ANIMATION_RATE: f32 = 12.0;

/// What the map is drawn from.
#[derive(bevy::ecs::system::SystemParam)]
pub struct Scene<'w, 's> {
    time: Res<'w, Time>,
    view: Res<'w, MapView>,
    map: Res<'w, WorldMap>,
    look: Res<'w, TileAppearance>,
    knowledge: Res<'w, Knowledge>,
    lighting: Option<Res<'w, Lighting>>,
    overlay: Option<Res<'w, LightOverlay>>,
    player: Query<'w, 's, &'static Viewshed, With<Player>>,
    /// What is drawn on the map. A prop nobody has spotted is not: being
    /// unseen is the whole of what `Hidden` means, and it is one filter
    /// here rather than a second drawing path.
    glyphs: Query<'w, 's, (&'static Position, &'static Glyph, Option<&'static OnMap>), Without<Hidden>>,
    fields: Res<'w, FieldAppearance>,
    shown: Res<'w, ShownFields>,
    registries: Option<Res<'w, Registries>>,
}

/// Paints the viewport: lit where the player sees, dim where remembered.
pub fn draw_map(mut terminal: ResMut<Terminal>, scene: Scene) {
    let Scene { time, view, map, look, knowledge, lighting, overlay, player, glyphs, fields, shown, registries } = &scene;
    let now = time.elapsed_secs();
    let t = (now * ANIMATION_RATE).floor() / ANIMATION_RATE;
    let viewshed = player.single().ok();
    let here = map.current();
    let lighting = lighting.as_deref();
    let overlay = overlay.as_deref().is_some_and(|o| o.0) && lighting.is_some();
    let sees = |p: Point| viewshed.is_some_and(|v| v.can_see(p));
    let cloud = shown.clouds_in_sight(view.viewport.cells().filter_map(|s| view.to_world(s)), now, sees, |p| knowledge.is_explored(p));
    for s in view.viewport.cells() {
        let Some(p) = view.to_world(s) else { continue };
        let mut cell = match (map.tile(p), viewshed.is_some_and(|v| v.can_see(p))) {
            (Some(id), true) => match lighting {
                Some(l) => look.under(look.seen(id, p, t), l.at(p), p, t),
                None => look.seen(id, p, t),
            },
            (Some(id), false) if knowledge.is_explored(p) => look.remembered(id, p),
            _ => Cell::default(),
        };
        let here = shown.at(p, now);
        if let Some((gas, amount)) = here.gas.filter(|_| cloud.contains(&p)) {
            // Hazed by how thick it shows, not by how it stirs, so the haze
            // does not blink on and off at the edge of a cloud.
            let hides = registries.as_deref().is_some_and(|r| r.gases.get(gas).veils(amount));
            let stirred = (f32::from(amount) * (1.0 + 0.45 * churn(p, now))).clamp(1.0, 255.0) as u8;
            cell = fields.under_gas(cell, fields.gas(gas), stirred, hides);
        }
        if sees(p) {
            if here.burning {
                cell = fields.burning(p, t);
            } else if shown.at(p.offset(0, 1), now).burning {
                cell = fields.over_flame(cell, p, t);
            }
        }
        if overlay && viewshed.is_some_and(|v| v.can_see(p)) {
            let level = (lighting.map(|l| l.at(p).intensity).unwrap_or(0) as u32 * 10 / 256) as u8;
            cell.glyph = (b'0' + level) as char;
            cell.fg = Color::WHITE;
        }
        terminal.set(s.x, s.y, cell);
    }
    let mut drawn: Vec<(Point, i32)> = Vec::new();
    for (pos, glyph, on) in glyphs {
        if on.map(|m| m.0).unwrap_or(MapId::SURFACE) != here || !viewshed.is_some_and(|v| v.can_see(pos.0)) {
            continue;
        }
        let Some(s) = view.to_screen(pos.0) else { continue };
        if let Some(existing) = drawn.iter_mut().find(|(q, _)| *q == pos.0) {
            if existing.1 >= glyph.layer {
                continue;
            }
            existing.1 = glyph.layer;
        } else {
            drawn.push((pos.0, glyph.layer));
        }
        let fg = match lighting {
            Some(l) => look.shading.light(glyph.fg, l.at(pos.0), pos.0, t),
            None => glyph.fg,
        };
        terminal.put(s.x, s.y, glyph.ch, fg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A prop is described twice, and this is the second half: the engine
    /// spawns it knowing nothing of colours, and the renderer dresses it.
    #[test]
    fn a_prop_is_dressed_from_its_definition_and_one_already_dressed_is_left_alone() {
        let mut app = App::new();
        app.add_systems(Update, dress_props);
        let props = rl_rules::prop::load(r#"[(name: "supply crate", glyph: '&', color: (r: 190, g: 165, b: 115), layer: 2)]"#, &rl_rules::Names::new())
            .expect("the props load");
        let id = props.expect("supply crate");
        let registries = Registries { props, ..Default::default() };
        app.insert_resource(registries);

        let bare = app.world_mut().spawn((Prop, PropKind(id))).id();
        let dressed = app.world_mut().spawn((Prop, PropKind(id), Glyph { ch: '#', fg: Color::WHITE, layer: 9 })).id();
        app.update();

        let glyph = app.world().get::<Glyph>(bare).expect("it was dressed");
        assert_eq!((glyph.ch, glyph.layer), ('&', 2), "the definition's glyph and layer");
        assert_eq!(glyph.fg, Color::srgb_u8(190, 165, 115), "and its colour");
        assert_eq!(app.world().get::<Glyph>(dressed).map(|g| g.ch), Some('#'), "a game that dressed its own is not overruled");
    }

    #[test]
    fn view_maps_between_world_and_screen() {
        let mut v = MapView::new(Rect::new(10, 2, 40, 20));
        v.center_on(Point::new(100, 100));
        assert_eq!(v.origin, Point::new(80, 90));
        assert_eq!(v.to_screen(Point::new(100, 100)), Some(Point::new(30, 12)));
        assert_eq!(v.to_world(Point::new(10, 2)), Some(Point::new(80, 90)));
        assert_eq!(v.to_screen(Point::new(0, 0)), None);
        assert_eq!(v.to_world(Point::new(0, 0)), None);
    }

    #[test]
    fn gas_tints_the_ground_thicker_for_more_and_a_haze_hides_the_glyph_heavier_the_thicker_it_is() {
        let fields = FieldAppearance::default();
        let ground = Cell::new('.', Color::WHITE).on(Color::BLACK);
        let tint = Color::srgb(0.2, 0.9, 0.2);
        let thin = fields.under_gas(ground, tint, 40, false);
        let thick = fields.under_gas(ground, tint, 220, false);
        assert_eq!(thin.glyph, '.', "thin gas leaves the ground showing");
        assert!(thick.bg.to_linear().green > thin.bg.to_linear().green, "more gas, more tint");
        assert_eq!(fields.under_gas(ground, tint, 90, true).glyph, '░', "what hides is drawn as haze");
        assert_eq!(fields.under_gas(ground, tint, 170, true).glyph, '░', "the lighter haze until it is thick");
        assert_eq!(fields.under_gas(ground, tint, 200, true).glyph, '▒', "and heavier near full");
        let mut coloured = FieldAppearance::default();
        coloured.set_gas(GasId::from_raw(2), tint);
        assert_eq!(coloured.gas(GasId::from_raw(2)), tint);
        assert_ne!(coloured.gas(GasId::from_raw(0)), tint, "a gas nobody coloured is grey");
        assert_eq!(fields.burning(Point::new(3, 4), 0.5).glyph, fields.flame.glyph);
    }

    #[test]
    fn appearance_table_grows_varies_and_remembers() {
        let mut look = TileAppearance::new();
        look.set(TileId(3), Cell::new('.', Color::WHITE));
        look.set_varied(TileId(4), Cell::new('#', Color::WHITE).on(Color::srgb(0.3, 0.3, 0.3)), Vary::new(0.2, 0.0));
        assert_eq!(look.lit(TileId(3)).glyph, '.');
        assert_eq!(look.lit(TileId(1)).glyph, '?');
        assert!(look.remembered(TileId(3), Point::ZERO).fg.to_linear().red < 0.5);
        let a = look.seen(TileId(4), Point::new(1, 1), 0.0);
        let b = look.seen(TileId(4), Point::new(2, 1), 0.0);
        assert_ne!(a.bg, b.bg, "each cell its own shade");
        assert_eq!(look.seen(TileId(3), Point::new(1, 1), 0.0), look.seen(TileId(3), Point::new(2, 1), 0.0));
    }
}

#[cfg(test)]
mod view_tests {
    use super::*;

    /// A map barely larger than the viewport still fills it: standing at
    /// the rim pulls the view back rather than showing what is not there.
    #[test]
    fn the_view_never_runs_off_the_edge_of_the_map() {
        let mut v = MapView::new(Rect::new(0, 0, 80, 40));
        let map = Rect::new(0, 0, 84, 42);
        v.center_on(Point::new(77, 11));
        v.clamp_to(map);
        // Centring wants (37, -9); the map allows 4 of slack across and 2
        // down, so x comes back to the far edge and y up to the near one.
        assert_eq!(v.origin, Point::new(4, 0), "pulled back inside the map on both axes");
        v.center_on(Point::new(2, 2));
        v.clamp_to(map);
        assert_eq!(v.origin, Point::new(0, 0), "and to its near corner");
    }

    /// A map smaller than the viewport has nothing to scroll, so it sits
    /// in the middle instead of in a corner.
    #[test]
    fn a_map_smaller_than_the_view_is_centred_in_it() {
        let mut v = MapView::new(Rect::new(0, 0, 80, 40));
        v.center_on(Point::new(5, 5));
        v.clamp_to(Rect::new(0, 0, 60, 20));
        assert_eq!(v.origin, Point::new(-10, -10), "half the difference on each side");
    }
}
