//! A virtual glyph terminal drawn with Bevy.

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSize, FontSmoothing, FontSource};
use rl_core::{Point, Rect};

use crate::layout::fit;
use crate::tileset::Tileset;

const BACKGROUND_Z: f32 = 0.0;
const PICTURE_Z: f32 = 0.5;
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
}

/// Sets up the terminal grid and keeps it filling its window.
///
/// The grid is laid out again whenever the window changes size, in whole
/// pixels, and the glyphs are drawn at the size they are shown; the camera
/// never scales, since scaling stretches glyphs drawn for another size.
///
/// Glyphs come from the system's generic monospace family, which needs
/// Bevy's `system_font_discovery` feature; the workspace enables it. With
/// it off, every cell draws its background and no glyph. A browser has no
/// font database to search, so on wasm the glyphs come from the font Bevy
/// embeds under `default_font`, which covers printable ASCII and nothing
/// else: a game drawing box art or block shades picks its own font there.
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
            .init_resource::<crate::pointer::Pointer>()
            .add_systems(Startup, spawn_grid)
            // Early in the frame, by which time the window already has its
            // new size: Bevy works out where a thing is drawn and lays its
            // text out late in `PostUpdate`, and a layout written there in
            // no order against them could be drawn a frame late.
            .add_systems(PreUpdate, (relayout, crate::pointer::track_pointer).chain())
            .add_systems(PostUpdate, (spawn_pictures, flush_terminal).chain());
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

    /// The size of one cell as declared, in logical pixels: what the grid
    /// is laid out from, not what a window of another size shows it at.
    pub fn cell_size(&self) -> Vec2 {
        self.cell_size
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
    /// One sprite per cell for a tileset's pictures, spawned the first
    /// time there is a tileset, and never in a game without one.
    picture: Vec<Entity>,
    displayed: Vec<Cell>,
    /// Which picture each cell is showing, `None` for a glyph.
    pictured: Vec<Option<usize>>,
    /// The size and place the grid was last laid out to, so sprites
    /// spawned after a layout are put where the cells already are.
    laid: Option<(Vec2, Vec<Vec2>)>,
}

/// Marks a cell's picture sprite, which keeps it apart from the
/// background sprite the same cell has.
#[derive(Component)]
struct Picture;

fn spawn_grid(mut commands: Commands, terminal: Res<Terminal>, font: Res<TerminalFont>, mut entities: ResMut<CellEntities>) {
    // No projection of its own: one world unit is one logical pixel, and
    // `relayout` sizes the grid for the window. A camera that scaled the
    // grid to fit would stretch glyphs drawn for another size.
    commands.spawn((Camera2d, Camera { clear_color: ClearColorConfig::Custom(Color::BLACK), ..default() }));
    // No system font database in a browser: take the embedded font.
    #[cfg(target_arch = "wasm32")]
    let family = FontSource::default();
    #[cfg(not(target_arch = "wasm32"))]
    let family = FontSource::Monospace;
    let text_font = TextFont { font: family, font_size: FontSize::Px(font.size), font_smoothing: FontSmoothing::AntiAliased, ..default() };
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

/// The window the grid was last laid out for: its physical size and its
/// scale factor's bits, compared exactly.
#[derive(Default)]
struct LaidOutFor(Option<(UVec2, u32)>);

/// Lays the grid out again when its window is another size or on another
/// display.
///
/// Sizes and positions come from [`fit`], and the glyphs take a font size
/// scaled with the cell, so Bevy rasterizes them at the size they are
/// shown rather than stretching what it drew for the declared one. Once
/// per change, not per frame.
///
/// With no window, as in a headless test, or a window of no size, as when
/// minimized, the last layout stands.
fn relayout(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    terminal: Res<Terminal>,
    font: Res<TerminalFont>,
    mut entities: ResMut<CellEntities>,
    mut laid: Local<LaidOutFor>,
    mut backgrounds: Query<(&mut Sprite, &mut Transform), Without<Text2d>>,
    mut glyphs: Query<(&mut TextFont, &mut Transform), With<Text2d>>,
) {
    let Ok(window) = windows.single() else { return };
    let size = UVec2::new(window.physical_width(), window.physical_height());
    let scale = window.scale_factor();
    if size.x == 0 || size.y == 0 || entities.background.is_empty() || laid.0 == Some((size, scale.to_bits())) {
        return;
    }
    laid.0 = Some((size, scale.to_bits()));
    let fit = fit(terminal.width, terminal.height, terminal.cell_size, size, scale);
    let (cell, font_size) = (fit.cell_logical(scale), fit.font(font.size, terminal.cell_size, scale));
    let mut centres = Vec::with_capacity(entities.background.len());
    for y in 0..terminal.height {
        for x in 0..terminal.width {
            let i = (y * terminal.width + x) as usize;
            let c = fit.center(x, y, size, scale);
            centres.push(c);
            if let Ok((mut sprite, mut transform)) = backgrounds.get_mut(entities.background[i]) {
                sprite.custom_size = Some(cell);
                transform.translation = c.extend(BACKGROUND_Z);
            }
            // A picture is a sprite like the background, and is laid out
            // with it: the same size, the same place, a little nearer.
            if let Some(Ok((mut sprite, mut transform))) = entities.picture.get(i).map(|e| backgrounds.get_mut(*e)) {
                sprite.custom_size = Some(cell);
                transform.translation = c.extend(PICTURE_Z);
            }
            if let Ok((mut text, mut transform)) = glyphs.get_mut(entities.glyph[i]) {
                text.font_size = FontSize::Px(font_size);
                transform.translation = c.extend(GLYPH_Z);
            }
        }
    }
    entities.laid = Some((cell, centres));
}

/// Spawns a picture sprite for every cell, the first time there is a
/// [`Tileset`], hidden until a cell shows one.
///
/// Laid out where the cells are now: as the grid was last laid out for
/// its window, or as declared when it never has been.
fn spawn_pictures(mut commands: Commands, terminal: Res<Terminal>, tileset: Option<Res<Tileset>>, mut entities: ResMut<CellEntities>) {
    let Some(tileset) = tileset else { return };
    if !entities.picture.is_empty() || entities.background.is_empty() {
        return;
    }
    let laid = entities.laid.clone();
    for y in 0..terminal.height() {
        for x in 0..terminal.width() {
            let i = (y * terminal.width() + x) as usize;
            let (size, at) = match &laid {
                Some((size, centres)) => (*size, centres[i]),
                None => (terminal.cell_size, terminal.cell_center(x, y)),
            };
            let sprite = Sprite {
                image: tileset.image().clone(),
                texture_atlas: Some(TextureAtlas { layout: tileset.layout().clone(), index: 0 }),
                custom_size: Some(size),
                ..default()
            };
            entities.picture.push(commands.spawn((Picture, sprite, Visibility::Hidden, Transform::from_xyz(at.x, at.y, PICTURE_Z))).id());
        }
    }
    entities.pictured = vec![None; entities.picture.len()];
}

/// Pushes changed cells to their entities.
///
/// With a [`Tileset`], a cell whose glyph it has a picture for shows the
/// picture, tinted by the cell's colour, and no glyph; any other cell
/// shows its glyph and no picture. A change to the tileset itself, its
/// pictures or whether it is on, redraws every cell, since any of them
/// may now be drawn the other way.
fn flush_terminal(
    terminal: Res<Terminal>,
    tileset: Option<Res<Tileset>>,
    mut entities: ResMut<CellEntities>,
    mut backgrounds: Query<&mut Sprite, Without<Picture>>,
    mut pictures: Query<(&mut Sprite, &mut Visibility), With<Picture>>,
    mut glyphs: Query<(&mut Text2d, &mut TextColor)>,
    mut had_tileset: Local<bool>,
) {
    if entities.displayed.len() != terminal.cells.len() {
        return;
    }
    // Every cell again when the tileset changed, came or went.
    let redraw = tileset.as_ref().is_some_and(|t| t.is_changed()) || *had_tileset != tileset.is_some();
    *had_tileset = tileset.is_some();
    let tileset = tileset.as_deref().filter(|_| entities.pictured.len() == terminal.cells.len());
    for i in 0..terminal.cells.len() {
        let next = terminal.cells[i];
        let current = entities.displayed[i];
        if next == current && !redraw {
            continue;
        }
        let cell = Point::new(i as i32 % terminal.width, i as i32 / terminal.width);
        let picture = tileset.and_then(|t| t.picture(next.glyph, cell));
        let was_pictured = entities.pictured.get(i).copied().flatten();
        if next.bg != current.bg
            && let Ok(mut sprite) = backgrounds.get_mut(entities.background[i])
        {
            sprite.color = next.bg;
        }
        // The glyph the text shows: none while a picture stands for it.
        let (shown, was_shown) = (if picture.is_some() { ' ' } else { next.glyph }, if was_pictured.is_some() { ' ' } else { current.glyph });
        if let Ok((mut text, mut color)) = glyphs.get_mut(entities.glyph[i]) {
            if shown != was_shown {
                text.0.clear();
                text.0.push(shown);
            }
            if next.fg != current.fg {
                color.0 = next.fg;
            }
        }
        if let Some(Ok((mut sprite, mut visibility))) = entities.picture.get(i).map(|e| pictures.get_mut(*e)) {
            match (picture, tileset) {
                (Some(index), Some(tileset)) => {
                    sprite.image = tileset.image().clone();
                    sprite.texture_atlas = Some(TextureAtlas { layout: tileset.layout().clone(), index });
                    sprite.color = next.fg;
                    *visibility = Visibility::Inherited;
                }
                _ => *visibility = Visibility::Hidden,
            }
            entities.pictured[i] = picture;
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

    use bevy::window::{PrimaryWindow, WindowResolution};

    /// A headless app with a terminal of eight by four cells of ten by
    /// twenty, and a primary window of `width` by `height` physical pixels.
    fn windowed(width: u32, height: u32, scale: f32) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(TerminalPlugin { width: 8, height: 4, cell_size: Vec2::new(10.0, 20.0), font_size: 16.0 });
        let mut resolution = WindowResolution::new(width, height);
        resolution.set_scale_factor_override(Some(scale));
        resolution.set_physical_resolution(width, height);
        let window = app.world_mut().spawn((Window { resolution, ..default() }, PrimaryWindow)).id();
        app.update();
        (app, window)
    }

    fn first_cell(app: &mut App) -> (Vec2, Vec2, f32) {
        let entities = app.world().resource::<CellEntities>();
        let (background, glyph) = (entities.background[0], entities.glyph[0]);
        let size = app.world().get::<Sprite>(background).unwrap().custom_size.unwrap();
        let at = app.world().get::<Transform>(background).unwrap().translation.truncate();
        let FontSize::Px(font) = app.world().get::<TextFont>(glyph).unwrap().font_size else { panic!("a pixel font size") };
        (size, at, font)
    }

    #[test]
    fn a_window_of_the_native_size_leaves_the_cells_as_declared() {
        let (mut app, _) = windowed(80, 80, 1.0);
        assert_eq!(first_cell(&mut app), (Vec2::new(10.0, 20.0), Vec2::new(-35.0, 30.0), 16.0));
    }

    #[test]
    fn a_window_twice_the_size_doubles_the_cells_and_the_font() {
        let (mut app, _) = windowed(160, 160, 1.0);
        assert_eq!(first_cell(&mut app), (Vec2::new(20.0, 40.0), Vec2::new(-70.0, 60.0), 32.0));
    }

    #[test]
    fn resizing_the_window_lays_the_grid_out_again() {
        let (mut app, window) = windowed(80, 80, 1.0);
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_physical_resolution(240, 240);
        app.update();
        assert_eq!(first_cell(&mut app).0, Vec2::new(30.0, 60.0));
    }

    #[test]
    fn a_change_of_scale_factor_alone_lays_the_grid_out_again() {
        let (mut app, window) = windowed(160, 160, 1.0);
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_scale_factor_override(Some(2.0));
        app.update();
        // The same physical pixels, half as many logical ones.
        assert_eq!(first_cell(&mut app).0, Vec2::new(10.0, 20.0));
    }

    /// What is drawn is the `GlobalTransform`, which Bevy works out from
    /// the `Transform` late in the frame. A layout written after that is
    /// drawn a frame late: backgrounds of the new size at the old places.
    #[test]
    fn a_resize_is_where_it_will_be_drawn_in_the_same_frame() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::transform::TransformPlugin)).add_plugins(TerminalPlugin {
            width: 8,
            height: 4,
            cell_size: Vec2::new(10.0, 20.0),
            font_size: 16.0,
        });
        let mut resolution = WindowResolution::new(80, 80);
        resolution.set_scale_factor_override(Some(1.0));
        let window = app.world_mut().spawn((Window { resolution, ..default() }, PrimaryWindow)).id();
        app.update();
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_physical_resolution(160, 160);
        app.update();
        let background = app.world().resource::<CellEntities>().background[0];
        let drawn = app.world().get::<GlobalTransform>(background).unwrap().translation().truncate();
        assert_eq!(drawn, Vec2::new(-70.0, 60.0), "one update after the window changed");
    }

    #[test]
    fn a_window_of_no_size_is_left_alone_and_recovers() {
        let (mut app, window) = windowed(160, 160, 1.0);
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_physical_resolution(0, 0);
        app.update();
        assert_eq!(first_cell(&mut app).0, Vec2::new(20.0, 40.0), "the last layout stands while minimized");
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_physical_resolution(80, 80);
        app.update();
        assert_eq!(first_cell(&mut app).0, Vec2::new(10.0, 20.0));
    }

    /// The cursor's cell is read off the layout the frame is drawn with:
    /// in a window twice the native size a cell is twenty by forty.
    #[test]
    fn the_pointer_is_the_cell_under_the_cursor_and_none_off_the_grid() {
        use crate::pointer::Pointer;
        use bevy::math::DVec2;
        use rl_core::Point;
        // Wider than the grid needs, so there is a margin to be in.
        let (mut app, window) = windowed(200, 160, 1.0);
        let cell = |app: &mut App, at: Option<DVec2>| {
            app.world_mut().get_mut::<Window>(window).unwrap().set_physical_cursor_position(at);
            app.update();
            app.world().resource::<Pointer>().cell()
        };
        assert_eq!(cell(&mut app, None), None, "no cursor in the window, no cell");
        assert_eq!(cell(&mut app, Some(DVec2::new(20.0, 0.0))), Some(Point::new(0, 0)), "the grid starts after a margin of twenty");
        assert_eq!(cell(&mut app, Some(DVec2::new(19.0, 0.0))), None, "and the margin is no cell");
        assert_eq!(cell(&mut app, Some(DVec2::new(20.0 + 7.0 * 20.0 + 19.0, 159.0))), Some(Point::new(7, 3)), "the last pixel of the last cell");
        assert_eq!(cell(&mut app, Some(DVec2::new(75.0, 45.0))), Some(Point::new(2, 1)));
        // The window is resized under a cursor that did not move.
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_physical_resolution(80, 80);
        app.update();
        assert_eq!(app.world().resource::<Pointer>().cell(), Some(Point::new(7, 2)), "the same pixel is another cell in a smaller grid");
    }

    /// With no window the pointer is left where a test put it.
    #[test]
    fn with_no_window_a_pointer_set_by_hand_stays() {
        use crate::pointer::Pointer;
        use rl_core::{Point, Rect};
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(TerminalPlugin { width: 8, height: 4, cell_size: Vec2::new(10.0, 20.0), font_size: 16.0 });
        app.insert_resource(Pointer::at(Point::new(5, 2)));
        app.update();
        let pointer = *app.world().resource::<Pointer>();
        assert_eq!(pointer.cell(), Some(Point::new(5, 2)));
        assert_eq!(pointer.within(Rect::new(4, 0, 4, 4)), Some(Point::new(5, 2)), "inside a panel's rectangle");
        assert_eq!(pointer.within(Rect::new(0, 0, 4, 4)), None, "and outside another's");
        let mut view = crate::map_view::MapView::new(Rect::new(2, 1, 6, 3));
        view.origin = Point::new(100, 50);
        assert_eq!(pointer.tile(&view), Some(Point::new(103, 51)), "the tile the map draws there");
        assert_eq!(Pointer::at(Point::new(0, 0)).tile(&view), None, "and none where the map is not drawn");
    }

    /// What cell `(x, y)` shows: its glyph as text, and the picture it
    /// shows with the picture's tint, or `None` while its sprite is hidden.
    fn shows(app: &App, x: i32, y: i32) -> (String, Option<(usize, Color)>) {
        let entities = app.world().resource::<CellEntities>();
        let i = (y * 8 + x) as usize;
        let text = app.world().get::<Text2d>(entities.glyph[i]).unwrap().0.clone();
        let picture = entities.picture.get(i).and_then(|e| {
            let visible = *app.world().get::<Visibility>(*e).unwrap() != Visibility::Hidden;
            let sprite = app.world().get::<Sprite>(*e).unwrap();
            visible.then(|| (sprite.texture_atlas.as_ref().unwrap().index, sprite.color))
        });
        (text, picture)
    }

    /// A tileset is a font of pictures: the cells are drawn as ever, and
    /// inside what it covers a glyph it names is a picture in the cell's
    /// colour, the rest stay glyphs, and turning it off is glyphs again.
    #[test]
    fn a_tileset_draws_pictures_for_the_glyphs_it_names_where_it_covers_and_glyphs_everywhere_else() {
        use rl_core::Rect;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(TerminalPlugin { width: 8, height: 4, cell_size: Vec2::new(10.0, 20.0), font_size: 16.0 });
        let red = Color::srgb(1.0, 0.0, 0.0);
        let draw = |app: &mut App| {
            let mut terminal = app.world_mut().resource_mut::<Terminal>();
            terminal.put(1, 1, '#', red);
            terminal.put(2, 1, 'x', Color::WHITE);
            terminal.put(1, 3, '#', Color::WHITE);
        };
        draw(&mut app);
        app.update();
        assert_eq!(shows(&app, 1, 1), ("#".to_string(), None), "with no tileset a glyph is a glyph");
        assert!(app.world().resource::<CellEntities>().picture.is_empty(), "and no sprite is spawned for pictures nobody has");

        // Covering the top three rows, with a picture for '#' alone.
        app.insert_resource(Tileset::new(Handle::default(), Handle::default()).within(Rect::new(0, 0, 8, 3)).with('#', 5));
        app.update();
        assert_eq!(shows(&app, 1, 1), (" ".to_string(), Some((5, red))), "the picture, in the cell's colour, and no glyph under it");
        assert_eq!(shows(&app, 2, 1), ("x".to_string(), None), "a character with no picture stays a glyph");
        assert_eq!(shows(&app, 1, 3), ("#".to_string(), None), "and so does one outside what the tileset covers");

        // The cell changes to something with no picture, and back.
        app.world_mut().resource_mut::<Terminal>().put(1, 1, 'x', red);
        app.update();
        assert_eq!(shows(&app, 1, 1), ("x".to_string(), None));
        draw(&mut app);
        app.update();
        assert_eq!(shows(&app, 1, 1), (" ".to_string(), Some((5, red))));

        // Off: the same cells, in letters. On again: in pictures.
        app.world_mut().resource_mut::<Tileset>().turned(false);
        app.update();
        assert_eq!(shows(&app, 1, 1), ("#".to_string(), None), "turned off, every cell is its glyph without being redrawn");
        app.world_mut().resource_mut::<Tileset>().turned(true);
        app.update();
        assert_eq!(shows(&app, 1, 1), (" ".to_string(), Some((5, red))));
        app.world_mut().remove_resource::<Tileset>();
        app.update();
        assert_eq!(shows(&app, 1, 1), ("#".to_string(), None), "and taken away altogether, glyphs");
    }

    /// A picture sprite is the cell's own size and stands where the cell
    /// does, in a window of any size, whenever it was spawned.
    #[test]
    fn a_picture_is_laid_out_with_its_cell() {
        let (mut app, window) = windowed(160, 160, 1.0);
        app.insert_resource(Tileset::new(Handle::default(), Handle::default()).with('#', 1));
        app.update();
        let picture = |app: &App| {
            let e = app.world().resource::<CellEntities>().picture[0];
            (app.world().get::<Sprite>(e).unwrap().custom_size.unwrap(), app.world().get::<Transform>(e).unwrap().translation.truncate())
        };
        assert_eq!(picture(&app), (Vec2::new(20.0, 40.0), Vec2::new(-70.0, 60.0)), "spawned after the layout, where the layout put the cell");
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set_physical_resolution(80, 80);
        app.update();
        assert_eq!(picture(&app), (Vec2::new(10.0, 20.0), Vec2::new(-35.0, 30.0)), "and laid out again with it");
    }

    #[test]
    fn with_no_window_the_grid_is_spawned_as_declared_and_nothing_fails() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(TerminalPlugin { width: 8, height: 4, cell_size: Vec2::new(10.0, 20.0), font_size: 16.0 });
        app.update();
        app.update();
        assert_eq!(first_cell(&mut app), (Vec2::new(10.0, 20.0), Vec2::new(-35.0, 30.0), 16.0));
    }
}
