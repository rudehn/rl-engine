//! The map, drawn from the player's point of view.
//!
//! Visible tiles are drawn lit, remembered tiles in the colours of
//! memory, unknown tiles blank. What a tile looks like is the game's
//! business: it fills a [`TileAppearance`] table keyed by [`TileId`], with
//! a glyph, both colours, and how the tile varies from cell to cell. With
//! lighting turned on, a visible tile's glyph and background and whatever
//! stands on it are coloured by the light that lands there; see
//! [`shade`](crate::shade) for how.

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::{Point, Rect};
use rl_grid::{Light, TileId};

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
            .insert_resource(MapView::new(self.0))
            .add_systems(Update, (follow_player, draw_map).chain().in_set(PresentSet::Map));
    }

    fn finish(&self, app: &mut App) {
        depends_on::<CorePlugin>(app, "MapViewPlugin");
        depends_on::<FovPlugin>(app, "MapViewPlugin");
    }
}

/// Keeps the view centred on the player.
pub fn follow_player(mut view: ResMut<MapView>, player: Query<&Position, With<Player>>) {
    if let Ok(pos) = player.single() {
        view.center_on(pos.0);
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
    glyphs: Query<'w, 's, (&'static Position, &'static Glyph, Option<&'static OnMap>)>,
}

/// Paints the viewport: lit where the player sees, dim where remembered.
pub fn draw_map(mut terminal: ResMut<Terminal>, scene: Scene) {
    let Scene { time, view, map, look, knowledge, lighting, overlay, player, glyphs } = &scene;
    let t = (time.elapsed_secs() * ANIMATION_RATE).floor() / ANIMATION_RATE;
    let viewshed = player.single().ok();
    let here = map.current();
    let lighting = lighting.as_deref();
    let overlay = overlay.as_deref().is_some_and(|o| o.0) && lighting.is_some();
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
