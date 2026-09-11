//! The map, drawn from the player's point of view.
//!
//! Visible tiles are drawn lit, remembered tiles dimmed, unknown tiles
//! blank. What a tile looks like is the game's business: it fills a
//! [`TileAppearance`] table keyed by [`TileId`].

use bevy::prelude::*;
use rl_bevy::prelude::*;
use rl_core::{Point, Rect};
use rl_grid::TileId;

use crate::terminal::{Cell, Terminal};

/// How an entity is drawn.
#[derive(Component, Debug, Clone, Copy)]
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

/// What each tile looks like when lit, indexed by [`TileId`].
#[derive(Resource, Debug, Clone, Default)]
pub struct TileAppearance {
    cells: Vec<Option<Cell>>,
    /// Brightness of a remembered tile, 0 to 1.
    pub remembered: f32,
}

impl TileAppearance {
    /// An empty table.
    pub fn new() -> Self {
        Self { cells: Vec::new(), remembered: 0.35 }
    }

    /// Sets the appearance of `id`.
    pub fn set(&mut self, id: TileId, cell: Cell) {
        let i = id.index();
        if self.cells.len() <= i {
            self.cells.resize(i + 1, None);
        }
        self.cells[i] = Some(cell);
    }

    /// The lit appearance of `id`; a magenta question mark for an id the
    /// game never described, so the gap is visible rather than blank.
    pub fn lit(&self, id: TileId) -> Cell {
        self.cells.get(id.index()).copied().flatten().unwrap_or(Cell::new('?', Color::srgb(1.0, 0.0, 1.0)))
    }

    /// The remembered appearance of `id`.
    pub fn dim(&self, id: TileId) -> Cell {
        self.lit(id).dimmed(self.remembered)
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
pub struct MapViewPlugin;

impl Plugin for MapViewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileAppearance>().add_systems(Update, (follow_player, draw_map).chain().in_set(EngineSet::Present));
    }
}

/// Keeps the view centred on the player.
pub fn follow_player(mut view: ResMut<MapView>, player: Query<&Position, With<Player>>) {
    if let Ok(pos) = player.single() {
        view.center_on(pos.0);
    }
}

/// Paints the viewport: lit where the player sees, dim where remembered.
pub fn draw_map(
    mut terminal: ResMut<Terminal>,
    view: Res<MapView>,
    map: Res<WorldMap>,
    look: Res<TileAppearance>,
    knowledge: Res<Knowledge>,
    player: Query<&Viewshed, With<Player>>,
    glyphs: Query<(&Position, &Glyph)>,
) {
    let viewshed = player.single().ok();
    for s in view.viewport.cells() {
        let Some(p) = view.to_world(s) else { continue };
        let cell = match (map.tile(p), viewshed.is_some_and(|v| v.can_see(p))) {
            (Some(t), true) => look.lit(t),
            (Some(t), false) if knowledge.is_explored(p) => look.dim(t),
            _ => Cell::default(),
        };
        terminal.set(s.x, s.y, cell);
    }
    let mut drawn: Vec<(Point, i32)> = Vec::new();
    for (pos, glyph) in &glyphs {
        if !viewshed.is_some_and(|v| v.can_see(pos.0)) {
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
        terminal.put(s.x, s.y, glyph.ch, glyph.fg);
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
    fn appearance_table_grows_and_dims() {
        let mut look = TileAppearance::new();
        look.set(TileId(3), Cell::new('.', Color::WHITE));
        assert_eq!(look.lit(TileId(3)).glyph, '.');
        assert_eq!(look.lit(TileId(1)).glyph, '?');
        assert!(look.dim(TileId(3)).fg.to_linear().red < 0.5);
    }
}
