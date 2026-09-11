//! A virtual glyph terminal drawn with Bevy.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSize, FontSmoothing, FontSource};
use rl_core::Rect;
use rl_grid::Rgb;

const BACKGROUND_Z: f32 = 0.0;
const GLYPH_Z: f32 = 1.0;

/// One character cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cell {
    /// The character drawn. A space draws only the background.
    pub glyph: char,
    /// Glyph colour.
    pub fg: Color,
    /// Fill colour.
    pub bg: Color,
}

impl Default for Cell {
    fn default() -> Self {
        Self { glyph: ' ', fg: Color::WHITE, bg: Color::BLACK }
    }
}

impl Cell {
    /// A glyph in `fg` on black.
    pub fn new(glyph: char, fg: Color) -> Self {
        Self { glyph, fg, bg: Color::BLACK }
    }

    /// The same cell with `bg`.
    pub fn on(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    /// The cell with both colours scaled toward black by `factor`, for a
    /// remembered-but-unseen tile.
    pub fn dimmed(self, factor: f32) -> Self {
        let dim = |c: Color| {
            let l = c.to_linear();
            Color::linear_rgb(l.red * factor, l.green * factor, l.blue * factor)
        };
        Self { glyph: self.glyph, fg: dim(self.fg), bg: dim(self.bg) }
    }

    /// The cell under `light`: both colours multiplied by the light's
    /// colour, with `floor` of their own brightness kept so that a tile
    /// seen in the dark by touch or dark sight still reads.
    pub fn lit_by(self, light: Rgb, floor: f32) -> Self {
        Self { glyph: self.glyph, fg: tint(self.fg, light, floor), bg: tint(self.bg, light, floor) }
    }
}

/// `color` under `light`, keeping `floor` of its own brightness.
pub fn tint(color: Color, light: Rgb, floor: f32) -> Color {
    let l = color.to_linear();
    let f = |c: f32, ch: u8| c * (floor + (1.0 - floor) * ch as f32 / 255.0);
    Color::linear_rgb(f(l.red, light.r), f(l.green, light.g), f(l.blue, light.b))
}

/// Sets up the terminal grid and keeps it on screen.
///
/// Glyphs come from the system's generic monospace family, which needs
/// Bevy's `system_font_discovery` feature; the workspace enables it. With
/// it off, every cell draws its background and no glyph.
pub struct TerminalPlugin {
    /// Width in cells.
    pub width: i32,
    /// Height in cells.
    pub height: i32,
    /// Size of one cell in pixels.
    pub cell_size: Vec2,
    /// Font height in pixels, usually a little under the cell height.
    pub font_size: f32,
}

impl Plugin for TerminalPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Terminal::new(self.width, self.height, self.cell_size))
            .insert_resource(TerminalFont { size: self.font_size })
            .init_resource::<CellEntities>()
            .add_systems(Startup, spawn_grid)
            .add_systems(PostUpdate, flush_terminal);
    }
}

/// The back buffer game code draws into. Writes are clipped to the grid.
#[derive(Resource, Debug, Clone)]
pub struct Terminal {
    width: i32,
    height: i32,
    cell_size: Vec2,
    cells: Vec<Cell>,
}

impl Terminal {
    /// A terminal of `width` by `height` cells.
    pub fn new(width: i32, height: i32, cell_size: Vec2) -> Self {
        assert!(width > 0 && height > 0, "terminal must have cells");
        Self { width, height, cell_size, cells: vec![Cell::default(); (width * height) as usize] }
    }

    /// Width in cells.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Height in cells.
    pub fn height(&self) -> i32 {
        self.height
    }

    /// The whole grid as a rectangle of cells.
    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    /// Size of the grid in pixels.
    pub fn pixel_size(&self) -> Vec2 {
        Vec2::new(self.width as f32 * self.cell_size.x, self.height as f32 * self.cell_size.y)
    }

    /// Resets every cell to blank on `bg`.
    pub fn clear(&mut self, bg: Color) {
        for cell in &mut self.cells {
            *cell = Cell { bg, ..Cell::default() };
        }
    }

    /// Fills a rectangle with `cell`, clipped.
    pub fn fill(&mut self, rect: Rect, cell: Cell) {
        for p in rect.cells() {
            self.set(p.x, p.y, cell);
        }
    }

    /// Writes a whole cell, ignoring out-of-bounds coordinates.
    pub fn set(&mut self, x: i32, y: i32, cell: Cell) {
        if let Some(i) = self.index(x, y) {
            self.cells[i] = cell;
        }
    }

    /// Writes a glyph and its colour, keeping the background.
    pub fn put(&mut self, x: i32, y: i32, glyph: char, fg: Color) {
        if let Some(i) = self.index(x, y) {
            self.cells[i].glyph = glyph;
            self.cells[i].fg = fg;
        }
    }

    /// Writes a string left to right from `(x, y)`, clipped at the edge.
    pub fn print(&mut self, x: i32, y: i32, text: &str, fg: Color) {
        for (i, glyph) in text.chars().enumerate() {
            self.put(x + i as i32, y, glyph, fg);
        }
    }

    /// Writes a string on a background.
    pub fn print_on(&mut self, x: i32, y: i32, text: &str, fg: Color, bg: Color) {
        for (i, glyph) in text.chars().enumerate() {
            self.set(x + i as i32, y, Cell { glyph, fg, bg });
        }
    }

    /// The cell at `(x, y)`, or `None` if out of bounds.
    pub fn get(&self, x: i32, y: i32) -> Option<Cell> {
        self.index(x, y).map(|i| self.cells[i])
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width && y < self.height).then(|| (y * self.width + x) as usize)
    }

    /// Pixel centre of a cell in world space, grid centred on the origin,
    /// row 0 at the top.
    fn cell_center(&self, x: i32, y: i32) -> Vec2 {
        let half = self.pixel_size() * 0.5;
        Vec2::new(-half.x + (x as f32 + 0.5) * self.cell_size.x, half.y - (y as f32 + 0.5) * self.cell_size.y)
    }
}

#[derive(Resource)]
struct TerminalFont {
    size: f32,
}

#[derive(Resource, Default)]
struct CellEntities {
    background: Vec<Entity>,
    glyph: Vec<Entity>,
    displayed: Vec<Cell>,
}

fn spawn_grid(mut commands: Commands, terminal: Res<Terminal>, font: Res<TerminalFont>, mut entities: ResMut<CellEntities>) {
    let pixel_size = terminal.pixel_size();
    commands.spawn((
        Camera2d,
        Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() },
        Projection::from(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::AutoMin { min_width: pixel_size.x, min_height: pixel_size.y },
            ..OrthographicProjection::default_2d()
        }),
    ));
    let text_font = TextFont { font: FontSource::Monospace, font_size: FontSize::Px(font.size), font_smoothing: FontSmoothing::AntiAliased, ..default() };
    let count = (terminal.width() * terminal.height()) as usize;
    entities.background = Vec::with_capacity(count);
    entities.glyph = Vec::with_capacity(count);
    entities.displayed = vec![Cell::default(); count];
    for y in 0..terminal.height() {
        for x in 0..terminal.width() {
            let c = terminal.cell_center(x, y);
            let background = commands.spawn((Sprite::from_color(Color::BLACK, terminal.cell_size), Transform::from_xyz(c.x, c.y, BACKGROUND_Z))).id();
            let glyph =
                commands.spawn((Text2d::new(" "), text_font.clone(), TextColor(Color::WHITE), Anchor::CENTER, Transform::from_xyz(c.x, c.y, GLYPH_Z))).id();
            entities.background.push(background);
            entities.glyph.push(glyph);
        }
    }
}

/// Pushes changed cells to their entities.
fn flush_terminal(
    terminal: Res<Terminal>,
    mut entities: ResMut<CellEntities>,
    mut backgrounds: Query<&mut Sprite>,
    mut glyphs: Query<(&mut Text2d, &mut TextColor)>,
) {
    if entities.displayed.len() != terminal.cells.len() {
        return;
    }
    for i in 0..terminal.cells.len() {
        let next = terminal.cells[i];
        let current = entities.displayed[i];
        if next == current {
            continue;
        }
        if next.bg != current.bg
            && let Ok(mut sprite) = backgrounds.get_mut(entities.background[i])
        {
            sprite.color = next.bg;
        }
        if let Ok((mut text, mut color)) = glyphs.get_mut(entities.glyph[i]) {
            if next.glyph != current.glyph {
                text.0.clear();
                text.0.push(next.glyph);
            }
            if next.fg != current.fg {
                color.0 = next.fg;
            }
        }
        entities.displayed[i] = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terminal() -> Terminal {
        Terminal::new(8, 4, Vec2::new(10.0, 20.0))
    }

    #[test]
    fn writes_land_where_addressed_and_out_of_bounds_writes_drop() {
        let mut t = terminal();
        t.put(3, 2, '@', Color::WHITE);
        assert_eq!(t.get(3, 2).unwrap().glyph, '@');
        t.put(-1, 0, 'X', Color::WHITE);
        t.put(8, 0, 'X', Color::WHITE);
        assert!(t.cells.iter().all(|c| c.glyph != 'X'));
        assert_eq!(t.get(8, 0), None);
    }

    #[test]
    fn print_clips_and_clear_resets() {
        let mut t = terminal();
        t.print(6, 0, "abcdef", Color::WHITE);
        let row: String = (0..8).map(|x| t.get(x, 0).unwrap().glyph).collect();
        assert_eq!(row, "      ab");
        t.clear(Color::BLACK);
        assert!(t.cells.iter().all(|c| c.glyph == ' '));
    }

    #[test]
    fn the_grid_is_centered_on_the_origin() {
        let t = terminal();
        assert_eq!(t.pixel_size(), Vec2::new(80.0, 80.0));
        assert_eq!(t.cell_center(0, 0), Vec2::new(-35.0, 30.0));
        assert_eq!(t.cell_center(7, 3), Vec2::new(35.0, -30.0));
    }

    #[test]
    fn dimming_darkens_both_colours() {
        let c = Cell::new('#', Color::linear_rgb(1.0, 0.5, 0.0)).on(Color::linear_rgb(0.2, 0.2, 0.2)).dimmed(0.5);
        assert!((c.fg.to_linear().red - 0.5).abs() < 1e-6);
        assert!((c.bg.to_linear().red - 0.1).abs() < 1e-6);
    }
}
