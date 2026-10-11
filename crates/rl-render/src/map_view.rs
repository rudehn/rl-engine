//! The map, drawn from the player's point of view.
//!
//! Visible tiles are drawn lit, remembered tiles in the colours of
//! memory, unknown tiles blank. A remembered tile shows the still thing
//! last seen on it, an item, a prop or a way through, faded with it; a
//! creature is never remembered, since where one stood says nothing of
//! where it is. What a tile looks like is the game's
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
//! is drawn is what [`ShownFields`] shows,
//! blended in and stirred, rather than the fields as they stand. Only on
//! tiles in sight; memory holds no smoke.
//!
//! Who the map drew this frame is kept in [`DrawnActors`], for whatever is
//! drawn over it after, a cursor, to leave them showing.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use bevy::color::Mix;
use bevy::prelude::*;
use rl_bevy::Recalled;
use rl_bevy::prelude::*;
use rl_core::Grid2D;
use rl_core::{Point, Rect};
use rl_grid::{Light, Rgb, TileId};
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

    /// `ground`, a remembered tile, with `thing` drawn on it in the colours
    /// of memory, the way the tile's own glyph is.
    pub fn recalled(&self, ground: Cell, thing: Recalled) -> Cell {
        let Rgb { r, g, b } = thing.color;
        Cell { glyph: thing.glyph, fg: self.memory.recall(Color::srgb_u8(r, g, b)), bg: ground.bg }
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

/// Where on the terminal the map is drawn, which world tile sits at its
/// top-left, and how many cells across a tile is.
///
/// A tile is one cell unless the view is [`across`](Self::across) more:
/// a terminal's cells are narrow so that text reads well, and a map of
/// square tiles on it wants each tile two cells wide. Everything that
/// draws on the map asks the view where a tile is, so a wide tile is the
/// view's business and nobody else's.
#[derive(Resource, Debug, Clone, Copy)]
pub struct MapView {
    /// The terminal cells the map occupies.
    pub viewport: Rect,
    /// The world tile drawn at the viewport's top-left.
    pub origin: Point,
    /// How many cells across one tile is drawn. At least one.
    across: i32,
}

impl MapView {
    /// A view filling `viewport`, a cell to a tile.
    pub fn new(viewport: Rect) -> Self {
        Self { viewport, origin: Point::ZERO, across: 1 }
    }

    /// Draws each tile `cells` cells across: two, for square tiles on
    /// cells half as wide as they are tall.
    pub fn across(mut self, cells: i32) -> Self {
        self.across = cells.max(1);
        self
    }

    /// How many cells across one tile is drawn.
    pub fn tile_width(&self) -> i32 {
        self.across
    }

    /// How many tiles the viewport is wide. Cells left over at its right
    /// edge, too few for a whole tile, show nothing.
    pub fn tiles_wide(&self) -> i32 {
        self.viewport.width / self.across
    }

    /// Centres the view on `p`.
    pub fn center_on(&mut self, p: Point) {
        self.origin = Point::new(p.x - self.tiles_wide() / 2, p.y - self.viewport.height / 2);
    }

    /// Pulls the view back inside `bounds`, so it never shows void past
    /// the edge of the map.
    ///
    /// Centring alone is not enough on a map close to the size of the
    /// viewport: standing near an edge puts the map's rim mid-screen and
    /// spends the rest on nothing. A map smaller than the viewport is
    /// centred in it instead, since there is nothing to scroll.
    pub fn clamp_to(&mut self, bounds: Rect) {
        let slack = Point::new(bounds.width - self.tiles_wide(), bounds.height - self.viewport.height);
        self.origin.x = if slack.x <= 0 { bounds.x + slack.x / 2 } else { self.origin.x.clamp(bounds.x, bounds.x + slack.x) };
        self.origin.y = if slack.y <= 0 { bounds.y + slack.y / 2 } else { self.origin.y.clamp(bounds.y, bounds.y + slack.y) };
    }

    /// The terminal cell a world tile is drawn at, its first when it is
    /// more than one across, if inside the viewport.
    pub fn to_screen(&self, p: Point) -> Option<Point> {
        let tile = p - self.origin;
        let inside = (0..self.tiles_wide()).contains(&tile.x) && (0..self.viewport.height).contains(&tile.y);
        inside.then(|| Point::new(self.viewport.x + tile.x * self.across, self.viewport.y + tile.y))
    }

    /// The world tile drawn at a terminal cell, if inside the viewport.
    pub fn to_world(&self, s: Point) -> Option<Point> {
        let local = s - self.viewport.origin();
        let tile = Point::new(local.x.div_euclid(self.across), local.y);
        (self.viewport.contains(s) && tile.x < self.tiles_wide()).then(|| tile + self.origin)
    }

    /// Every terminal cell a world tile is drawn in, left to right; none
    /// when it is outside the viewport.
    pub fn cells(&self, p: Point) -> impl Iterator<Item = Point> + use<> {
        let across = self.across;
        self.to_screen(p).into_iter().flat_map(move |first| (0..across).map(move |i| Point::new(first.x + i, first.y)))
    }

    /// Every world tile in the viewport, row by row.
    pub fn tiles(&self) -> impl Iterator<Item = Point> + use<> {
        let (origin, wide, high) = (self.origin, self.tiles_wide(), self.viewport.height);
        (0..high).flat_map(move |y| (0..wide).map(move |x| origin + Point::new(x, y)))
    }

    /// Writes `cell` over the whole of a world tile, as one wide cell of
    /// the terminal: the glyph drawn centred across the tile's cells, and
    /// the same colours in every one.
    pub fn set(&self, terminal: &mut Terminal, p: Point, cell: Cell) {
        if let Some(at) = self.to_screen(p) {
            terminal.set_wide(at.x, at.y, self.across, cell);
        }
    }

    /// Repaints the background of a world tile, every cell of it, leaving
    /// whatever glyph is drawn there: a highlight that marks without
    /// hiding.
    pub fn wash(&self, terminal: &mut Terminal, p: Point, bg: Color) {
        for at in self.cells(p) {
            terminal.set_bg(at.x, at.y, bg);
        }
    }

    /// Writes a glyph and its colour over a world tile, keeping its
    /// colours: what stands on the tile, drawn on it as the tile's own
    /// glyph is, centred across it.
    pub fn put(&self, terminal: &mut Terminal, p: Point, glyph: char, fg: Color) {
        if let Some(at) = self.to_screen(p) {
            terminal.put(at.x, at.y, glyph, fg);
        }
    }

    /// What is drawn for a world tile: its first cell.
    pub fn get(&self, terminal: &Terminal, p: Point) -> Option<Cell> {
        self.to_screen(p).and_then(|at| terminal.get(at.x, at.y))
    }
}

/// Draws the map and the entities on it into the terminal every frame.
///
/// Takes the rectangle it draws in, the way every panel does, so a game
/// has no [`MapView`] of its own to insert and cannot forget one. Needs
/// field of view: without it no tile is ever seen or remembered, so the
/// map would draw as nothing at all.
pub struct MapViewPlugin {
    viewport: Rect,
    across: i32,
}

impl MapViewPlugin {
    /// The map, drawn in the terminal cells of `viewport`, a cell to a
    /// tile.
    pub fn new(viewport: Rect) -> Self {
        Self { viewport, across: 1 }
    }

    /// Draws each tile `cells` cells across, and has the terminal draw a
    /// glyph or a picture centred across them: two, for square tiles on
    /// cells half as wide as they are tall.
    pub fn across(mut self, cells: i32) -> Self {
        self.across = cells.max(1);
        self
    }
}

impl Plugin for MapViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileAppearance>()
            .init_resource::<FieldAppearance>()
            .init_resource::<ShownFields>()
            .init_resource::<Wavefronts>()
            .init_resource::<DrawnActors>()
            .insert_resource(MapView::new(self.viewport).across(self.across))
            .add_systems(Update, (follow_player, track_fields, remember_still_things, draw_map).chain().in_set(PresentSet::Map))
            // In `Update` rather than in a play-only set, because props are
            // put down while a place is built and `Added` matches for one
            // frame only. After the turns, which put a place's props down
            // the frame it is first entered, and before anything is drawn
            // or remembered, so a prop is never drawn bare for a frame and
            // a place known by heart remembers every prop in it.
            .add_systems(Update, (dress_props, redress_emptied).after(EngineSet::Turns).before(EngineSet::Present));
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

/// What the map draws that stands still: anything but a creature, and
/// nothing nobody has spotted, since a hidden prop is not seen and so
/// cannot be remembered either.
type StillThings<'w, 's> = Query<'w, 's, (&'static Position, &'static Glyph, Option<&'static OnMap>), (Without<Actor>, Without<Hidden>)>;

/// `c` at eight bits a channel, the way [`Knowledge`] keeps a colour.
fn rgb(c: Color) -> Rgb {
    let c = c.to_srgba();
    let channel = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgb::new(channel(c.red), channel(c.green), channel(c.blue))
}

/// Remembers the still things the player sees, so that the map draws each
/// where it was seen once its tile is out of sight.
///
/// Part of the map view rather than a plugin of its own: a map that
/// forgets the stairs behind the player is a broken map, not a simpler
/// one. The memory itself is [`Knowledge`]'s, so it is kept per map and
/// saved with the rest of what the player knows.
///
/// Once a frame, just before the map is drawn, rather than once a turn:
/// memory is what the player was shown, and this is the frame shown. Of
/// several things on one tile the one remembered is the one drawn, the
/// highest layer.
pub fn remember_still_things(map: Res<WorldMap>, mut knowledge: ResMut<Knowledge>, player: Query<&Viewshed, With<Player>>, still: StillThings) {
    let Ok(sight) = player.single() else { return };
    let here = map.current();
    let mut top: BTreeMap<Point, (i32, Recalled)> = BTreeMap::new();
    for (pos, glyph, on) in &still {
        if on.map_or(MapId::SURFACE, |m| m.0) != here {
            continue;
        }
        let look = (glyph.layer, Recalled { glyph: glyph.ch, color: rgb(glyph.fg) });
        match top.entry(pos.0) {
            Entry::Vacant(v) => {
                v.insert(look);
            }
            Entry::Occupied(mut o) if o.get().0 < glyph.layer => {
                o.insert(look);
            }
            Entry::Occupied(_) => {}
        }
    }
    knowledge.see_still(|p| sight.can_see(p), top.into_iter().map(|(p, (_, look))| (p, look)));
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

/// What a thing on the map is drawn by: where, as what, on which map, and
/// whether it is [`Concealed`] in the dark.
type Drawn = (&'static Position, &'static Glyph, Option<&'static OnMap>, Has<Concealed>, Has<Actor>);

/// The tiles the map drew someone on this frame: every actor the player
/// sees, the player among them, refilled by [`draw_map`].
///
/// What a mark drawn over the map after it asks so as not to cover
/// anyone, a cursor's ticks above all: a tick standing on the player's
/// own cell, the commonest neighbour of whatever they look at, hides
/// where they are. The map answers it rather than the world, since it
/// is what decided who was shown, by the one question of sight a panel
/// listing them asks too. Anyone, not anything: a tick may stand on a
/// thing lying about or a table as it does on a wall, for a cursor
/// framed on fewer sides in every furnished room would read as no
/// cursor.
#[derive(Resource, Debug, Default)]
pub struct DrawnActors(Vec<Point>);

impl DrawnActors {
    /// Whether the map drew someone on `p` this frame.
    pub fn at(&self, p: Point) -> bool {
        self.0.contains(&p)
    }
}

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
    glyphs: Query<'w, 's, Drawn, Without<Hidden>>,
    /// Which of them the player sees: the question the list of what is in
    /// sight asks too, so the map never draws a thing no panel names.
    sight: PlayerSight<'w, 's>,
    fields: Res<'w, FieldAppearance>,
    shown: Res<'w, ShownFields>,
    registries: Option<Res<'w, Registries>>,
}

/// Paints the viewport: lit where the player sees, dim where remembered.
pub fn draw_map(mut terminal: ResMut<Terminal>, mut actors: ResMut<DrawnActors>, scene: Scene) {
    let Scene { time, view, map, look, knowledge, lighting, overlay, player, glyphs, sight, fields, shown, registries } = &scene;
    let now = time.elapsed_secs();
    let t = (now * ANIMATION_RATE).floor() / ANIMATION_RATE;
    let viewshed = player.single().ok();
    let lighting = lighting.as_deref();
    let overlay = overlay.as_deref().is_some_and(|o| o.0) && lighting.is_some();
    let sees = |p: Point| viewshed.is_some_and(|v| v.can_see(p));
    let cloud = shown.clouds_in_sight(view.tiles(), now, sees, |p| knowledge.is_explored(p));
    for p in view.tiles() {
        let mut cell = match (map.tile(p), viewshed.is_some_and(|v| v.can_see(p))) {
            (Some(id), true) => match lighting {
                Some(l) => look.under(look.seen(id, p, t), l.at(p), p, t),
                None => look.seen(id, p, t),
            },
            (Some(id), false) if knowledge.is_explored(p) => match knowledge.recalled(p) {
                Some(thing) => look.recalled(look.remembered(id, p), thing),
                None => look.remembered(id, p),
            },
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
        view.set(&mut terminal, p, cell);
    }
    let mut drawn: Vec<(Point, i32)> = Vec::new();
    actors.0.clear();
    for (pos, glyph, on, concealed, actor) in glyphs {
        if !sight.shows(pos.0, on, concealed) {
            continue;
        }
        if view.to_screen(pos.0).is_none() {
            continue;
        }
        if actor {
            actors.0.push(pos.0);
        }
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
        view.put(&mut terminal, pos.0, glyph.ch, fg);
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

    /// The map as drawn for whoever is the player at `start`, with a wall
    /// two tiles east of it and something standing behind that wall:
    /// the glyphs drawn at the wall, behind it and on the thing.
    fn drawn_for(sight: Viewshed) -> [char; 3] {
        use rl_bevy::components::RevealsMap;
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, rl_bevy::world::StreamingPlugin, MapViewPlugin::new(Rect::new(0, 0, 40, 20))));
        app.insert_resource(Terminal::new(40, 20, Vec2::ONE));
        let start = rl_bevy::testing::surface(&mut app);
        let tiles = rl_grid::TileRegistry::standard();
        let (wall, floor) = (tiles.expect("wall"), tiles.expect("floor"));
        let mut look = TileAppearance::new();
        look.set(wall, Cell::new('#', Color::WHITE));
        look.set(floor, Cell::new('.', Color::WHITE));
        app.insert_resource(look);
        app.world_mut().spawn((Player, Position(start), sight, RevealsMap));
        app.world_mut().spawn((Actor, Position(start.offset(4, 0)), Glyph { ch: 'g', fg: Color::WHITE, layer: 1 }));
        app.world_mut().resource_mut::<NextState<rl_bevy::state::EngineState>>().set(rl_bevy::state::EngineState::Playing);
        app.update();
        app.update();
        // A wall the height of the sight, so nothing sees round its ends.
        for dy in -9..=9 {
            app.world_mut().resource_mut::<WorldMap>().set_tile(start.offset(2, dy), wall);
        }
        app.update();
        app.update();
        let view = *app.world().resource::<MapView>();
        let terminal = app.world().resource::<Terminal>();
        let at = |dx: i32| {
            let s = view.to_screen(start.offset(dx, 0)).expect("on screen");
            terminal.get(s.x, s.y).expect("a cell").glyph
        };
        [at(2), at(3), at(4)]
    }

    /// An onlooker, a player that sees everywhere, is drawn the whole map:
    /// the same view code, asked by a viewshed no wall stops.
    #[test]
    fn the_map_is_drawn_whole_for_a_player_that_sees_everywhere() {
        assert_eq!(
            drawn_for(Viewshed::new(8)),
            ['#', '.', '.'],
            "an ordinary sight stops at the wall: the ground behind is remembered, and the creature seen standing on it is not"
        );
        assert_eq!(drawn_for(Viewshed::everywhere()), ['#', '.', 'g'], "one that sees everywhere is shown what is behind it");
    }

    /// A player with ordinary sight on open ground, drawn on a terminal,
    /// the way `drawn_for` draws one: the app, where the player started,
    /// and the player.
    fn open_ground() -> (App, Point, Entity) {
        use rl_bevy::components::RevealsMap;
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins((FovPlugin, rl_bevy::world::StreamingPlugin, MapViewPlugin::new(Rect::new(0, 0, 60, 30))));
        app.insert_resource(Terminal::new(60, 30, Vec2::ONE));
        let start = rl_bevy::testing::surface(&mut app);
        let tiles = rl_grid::TileRegistry::standard();
        let mut look = TileAppearance::new();
        look.set(tiles.expect("wall"), Cell::new('#', Color::WHITE));
        look.set(tiles.expect("floor"), Cell::new('.', Color::srgb(0.5, 0.5, 0.5)));
        app.insert_resource(look);
        let player = app.world_mut().spawn((Actor, Player, Position(start), Viewshed::new(6), RevealsMap)).id();
        app.world_mut().resource_mut::<NextState<rl_bevy::state::EngineState>>().set(rl_bevy::state::EngineState::Playing);
        app.update();
        app.update();
        (app, start, player)
    }

    /// Stands the player at `at` and lets sight and the map catch up.
    fn stand_at(app: &mut App, player: Entity, at: Point) {
        app.world_mut().get_mut::<Position>(player).expect("a player").0 = at;
        app.world_mut().get_mut::<Viewshed>(player).expect("eyes").dirty = true;
        app.update();
        app.update();
    }

    /// What the map draws at the world tile `p`.
    fn drawn_at(app: &App, p: Point) -> Cell {
        let view = *app.world().resource::<MapView>();
        view.get(app.world().resource::<Terminal>(), p).expect("on screen")
    }

    /// Whether the player sees `p` now.
    fn sees(app: &mut App, p: Point) -> bool {
        app.world_mut().query_filtered::<&Viewshed, With<Player>>().single(app.world()).expect("a player").can_see(p)
    }

    /// The bug a playtest found: the stairs vanished from the map the
    /// moment they left sight, since only tiles were remembered. A thing
    /// seen lying somewhere is drawn there out of sight, in the colours of
    /// memory, so it reads as remembered rather than seen.
    #[test]
    fn a_still_thing_once_seen_is_drawn_where_it_lay_after_it_leaves_sight() {
        let (mut app, start, player) = open_ground();
        let bright = Color::srgb_u8(242, 217, 77);
        let stairs = start.offset(3, 0);
        app.world_mut().spawn((Position(stairs), Glyph::new('>', bright).on_layer(1)));
        app.update();
        assert_eq!(drawn_at(&app, stairs).glyph, '>', "in sight it is drawn");
        stand_at(&mut app, player, start.offset(-12, 0));
        assert!(!sees(&mut app, stairs), "the player has walked out of sight of it");
        let drawn = drawn_at(&app, stairs);
        assert_eq!(drawn.glyph, '>', "and it is still drawn where it was seen");
        let memory = app.world().resource::<TileAppearance>().memory;
        assert_eq!(drawn.fg, memory.recall(bright), "in the colours of memory");
    }

    /// Where a creature stood says nothing of where it is, so one seen and
    /// walked away from leaves the ground it stood on drawn bare.
    #[test]
    fn a_creature_seen_is_never_remembered() {
        let (mut app, start, player) = open_ground();
        let rat = start.offset(3, 0);
        app.world_mut().spawn((Actor, Position(rat), Glyph::new('r', Color::WHITE).on_layer(5)));
        app.update();
        assert_eq!(drawn_at(&app, rat).glyph, 'r');
        stand_at(&mut app, player, start.offset(-12, 0));
        assert_eq!(drawn_at(&app, rat).glyph, '.', "the ground, and nothing on it");
    }

    /// The map keeps who it drew, for whatever is drawn over it after: a
    /// creature in sight, and neither one out of sight nor a thing lying
    /// in sight.
    #[test]
    fn the_map_keeps_the_tiles_it_drew_someone_on_and_only_those() {
        let (mut app, start, _) = open_ground();
        let (rat, far, lying) = (start.offset(3, 0), start.offset(20, 0), start.offset(2, 1));
        app.world_mut().spawn((Actor, Position(rat), Glyph::new('r', Color::WHITE)));
        app.world_mut().spawn((Actor, Position(far), Glyph::new('r', Color::WHITE)));
        app.world_mut().spawn((Position(lying), Glyph::new('!', Color::WHITE)));
        app.update();
        assert_eq!(drawn_at(&app, rat).glyph, 'r', "the rat is drawn");
        let drawn = app.world().resource::<DrawnActors>();
        assert!(drawn.at(rat), "and kept as drawn");
        assert!(!drawn.at(far), "not the one out of sight");
        assert!(!drawn.at(lying), "nor a thing lying about");
    }

    /// Memory is what was seen, not what is true: a thing taken away
    /// behind the player's back is drawn until its tile is seen again, and
    /// forgotten from then on; a thing put down out of sight on ground the
    /// player knows is not drawn until it is seen.
    #[test]
    fn what_comes_or_goes_out_of_sight_changes_nothing_until_its_tile_is_seen_again() {
        let (mut app, start, player) = open_ground();
        let (lying, dropped) = (start.offset(3, 0), start.offset(3, 1));
        let thing = app.world_mut().spawn((Position(lying), Glyph::new('!', Color::WHITE).on_layer(2))).id();
        app.update();
        stand_at(&mut app, player, start.offset(-12, 0));
        app.world_mut().despawn(thing);
        app.world_mut().spawn((Position(dropped), Glyph::new('?', Color::WHITE).on_layer(2)));
        app.update();
        assert!(!sees(&mut app, lying) && !sees(&mut app, dropped));
        assert_eq!(drawn_at(&app, lying).glyph, '!', "gone, but not seen to go");
        assert_eq!(drawn_at(&app, dropped).glyph, '.', "put down, but not seen put down");

        stand_at(&mut app, player, start);
        assert_eq!((drawn_at(&app, lying).glyph, drawn_at(&app, dropped).glyph), ('.', '?'), "in sight, what is there");
        stand_at(&mut app, player, start.offset(-12, 0));
        assert_eq!(drawn_at(&app, lying).glyph, '.', "and what went is forgotten once seen gone");
        assert_eq!(drawn_at(&app, dropped).glyph, '?', "and what came is remembered once seen");
    }

    /// Of a prop and a thing lying on it, the one remembered is the one
    /// drawn: the higher layer.
    #[test]
    fn of_two_things_on_a_tile_the_one_drawn_on_top_is_the_one_remembered() {
        let (mut app, start, player) = open_ground();
        let p = start.offset(3, 0);
        app.world_mut().spawn((Position(p), Glyph::new('&', Color::WHITE).on_layer(1)));
        app.world_mut().spawn((Position(p), Glyph::new('/', Color::WHITE).on_layer(2)));
        app.update();
        stand_at(&mut app, player, start.offset(-12, 0));
        assert_eq!(drawn_at(&app, p).glyph, '/');
    }

    /// A hidden prop is not seen, so it is not remembered either; once
    /// spotted it is a prop like any other.
    #[test]
    fn a_hidden_prop_is_remembered_only_once_it_is_spotted() {
        let (mut app, start, player) = open_ground();
        let p = start.offset(3, 0);
        let trap = app.world_mut().spawn((Position(p), Glyph::new('^', Color::WHITE).on_layer(1), Hidden { spot: 0 })).id();
        app.update();
        stand_at(&mut app, player, start.offset(-12, 0));
        assert_eq!(drawn_at(&app, p).glyph, '.', "never seen, so never remembered");

        app.world_mut().entity_mut(trap).remove::<Hidden>();
        stand_at(&mut app, player, start);
        stand_at(&mut app, player, start.offset(-12, 0));
        assert_eq!(drawn_at(&app, p).glyph, '^', "spotted, and remembered");
    }

    /// A tile learned rather than seen, as a game writes in a place the
    /// player knows by heart, remembers what stands on it as it stands,
    /// though the player has never been in sight of it.
    #[test]
    fn a_tile_learned_unseen_remembers_what_stands_on_it() {
        let (mut app, start, _) = open_ground();
        let (far, beside) = (start.offset(15, 0), start.offset(15, 1));
        app.world_mut().spawn((Position(far), Glyph::new('>', Color::WHITE).on_layer(1)));
        app.world_mut().spawn((Position(beside), Glyph::new('&', Color::WHITE).on_layer(1)));
        app.world_mut().resource_mut::<Knowledge>().learn(far);
        app.update();
        assert!(!sees(&mut app, far));
        assert_eq!(drawn_at(&app, far).glyph, '>', "known by heart, and drawn from memory");
        assert!(!app.world().resource::<Knowledge>().is_explored(beside), "and nothing more than was learned");
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

    /// A view two cells to a tile: a tile is found at its first cell, any
    /// of its cells is that tile, and a write covers both.
    #[test]
    fn a_wide_view_maps_a_tile_to_its_cells_and_any_of_them_back() {
        let mut v = MapView::new(Rect::new(10, 2, 41, 20)).across(2);
        v.origin = Point::new(100, 50);
        assert_eq!((v.tile_width(), v.tiles_wide()), (2, 20), "forty-one cells hold twenty tiles and one cell over");
        assert_eq!(v.to_screen(Point::new(100, 50)), Some(Point::new(10, 2)));
        assert_eq!(v.to_screen(Point::new(103, 51)), Some(Point::new(16, 3)), "three tiles along is six cells along");
        assert_eq!(v.to_screen(Point::new(120, 50)), None, "the twenty-first tile is off the view");
        assert_eq!(v.to_world(Point::new(16, 3)), Some(Point::new(103, 51)));
        assert_eq!(v.to_world(Point::new(17, 3)), Some(Point::new(103, 51)), "its second cell is the same tile");
        assert_eq!(v.to_world(Point::new(50, 3)), None, "the cell left over is no tile");
        assert_eq!(v.cells(Point::new(103, 51)).collect::<Vec<_>>(), vec![Point::new(16, 3), Point::new(17, 3)]);
        assert_eq!(v.tiles().count(), 20 * 20);
        v.center_on(Point::new(200, 200));
        assert_eq!(v.origin, Point::new(190, 190), "centred by tiles, not by cells");
        v.clamp_to(Rect::new(0, 0, 10, 10));
        assert_eq!(v.origin, Point::new(-5, -5), "a map of ten tiles is centred in a view of twenty");

        let mut terminal = Terminal::new(60, 30, Vec2::ONE);
        v.origin = Point::new(100, 50);
        v.set(&mut terminal, Point::new(103, 51), Cell::new('@', Color::WHITE).on(Color::srgb(0.2, 0.2, 0.2)));
        assert_eq!(terminal.get(16, 3).map(|c| c.glyph), Some('@'));
        assert_eq!(terminal.get(17, 3).map(|c| (c.glyph, c.bg)), Some((' ', Color::srgb(0.2, 0.2, 0.2))), "the glyph once, the colours across");
        assert_eq!((terminal.span(16, 3), terminal.span(17, 3)), (Some(2), Some(0)), "one glyph drawn across the tile's two cells");
        v.wash(&mut terminal, Point::new(103, 51), Color::srgb(1.0, 0.0, 0.0));
        assert_eq!(terminal.get(16, 3).map(|c| (c.glyph, c.bg)), Some(('@', Color::srgb(1.0, 0.0, 0.0))), "a wash keeps the glyph");
        assert_eq!(terminal.get(17, 3).map(|c| c.bg), Some(Color::srgb(1.0, 0.0, 0.0)), "and covers the whole tile");
        assert_eq!(terminal.span(16, 3), Some(2), "and keeps it one tile");
        v.put(&mut terminal, Point::new(103, 51), 'g', Color::WHITE);
        assert_eq!((v.get(&terminal, Point::new(103, 51)).map(|c| c.glyph), terminal.span(16, 3)), (Some('g'), Some(2)), "what stands on it, across it");
        v.put(&mut terminal, Point::new(103, 51), '@', Color::WHITE);
        assert_eq!(v.get(&terminal, Point::new(103, 51)).map(|c| c.glyph), Some('@'));
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
