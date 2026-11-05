//! Opening the window as large as the screen allows.
//!
//! A grid declared at ten by sixteen pixels a cell opens at the size it
//! was declared, which on a large screen is a small window in the middle of
//! it. [`FitScreenPlugin`] opens it instead at the largest size that keeps
//! the grid's shape and fits the part of the monitor the system leaves to
//! windows, centred there, and the renderer's [`layout`](crate::layout)
//! draws the grid larger to fill it.
//!
//! The window is sized before it is shown, never shown small and then
//! grown: Bevy knows the monitors only once the window exists, so the
//! window is created hidden, sized on the first frame a monitor is known,
//! and placed and shown on the next. Sizing and placing are two frames
//! because a resize can move the window's top edge, which would undo a
//! place made in the same frame.

use atglyph_bevy::settings::Settings;
use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryMonitor, PrimaryWindow, WindowMode, WindowPosition};

use crate::layout;

/// Opens the primary window as large as the screen allows, keeping the
/// grid's shape, centred in what the system leaves to windows.
///
/// Takes the grid it opens for: `cols` by `rows` cells of `cell` logical
/// pixels, as the [`TerminalPlugin`](crate::TerminalPlugin) declares it.
/// The facade's `RoguelikePlugins::fit_screen` adds it.
///
/// Only the opening: the window is the player's afterwards, resized or
/// made fullscreen as they like. It leaves alone a window that opens
/// fullscreen, a game left that way and remembered by the
/// [`FullscreenPlugin`](crate::FullscreenPlugin), a window whose scale
/// factor the game overrides, a capture, which shoots the declared size
/// on every machine, and the web, where the canvas follows its page.
///
/// What the system keeps is read from the platform where it says: on
/// macOS, the menu bar and the dock, from the screen's visible frame.
/// Elsewhere it is taken as 32 logical pixels along the top and 48 along
/// the bottom, a panel and a taskbar.
pub struct FitScreenPlugin {
    cols: i32,
    rows: i32,
    cell: Vec2,
    /// Whether to ask the platform what it keeps; tests do not, so they
    /// read the same on every machine.
    platform: bool,
}

impl FitScreenPlugin {
    /// Fits a grid of `cols` by `rows` cells of `cell` logical pixels.
    pub fn new(cols: i32, rows: i32, cell: Vec2) -> Self {
        Self { cols, rows, cell, platform: true }
    }
}

/// Frames to wait for a monitor before showing the window at the size it
/// was made: a platform that reports none never leaves it hidden.
const GIVE_UP: u32 = 30;

/// What the system keeps when the platform does not say.
const KEPT: Insets = Insets { top: 32.0, bottom: 48.0, left: 0.0, right: 0.0 };

#[derive(Resource)]
struct Fitting {
    cols: i32,
    rows: i32,
    cell: Vec2,
    /// What the platform says it keeps on the primary monitor.
    kept: Option<Insets>,
    stage: Stage,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Stage {
    /// Hidden, waiting for a monitor; frames waited so far.
    Measure(u32),
    /// Sized, to be placed here and shown.
    Place(IVec2),
    /// Opened, or left alone.
    Done,
}

impl Plugin for FitScreenPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(target_arch = "wasm32") {
            return;
        }
        app.insert_resource(Fitting { cols: self.cols, rows: self.rows, cell: self.cell, kept: None, stage: Stage::Done }).add_systems(Update, open);
    }

    /// Once, before the window is made: hides it until it is sized, unless
    /// it is one to leave alone. Here rather than in `build` so a
    /// fullscreen setting recalled from a save is known, whichever order
    /// the plugins were added in.
    fn cleanup(&self, app: &mut App) {
        if cfg!(target_arch = "wasm32") || crate::capture::requested() {
            return;
        }
        let world = app.world_mut();
        let fullscreen = world.get_resource::<Settings>().is_some_and(crate::fullscreen::on);
        let mut windows = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
        let Ok(mut window) = windows.single_mut(world) else { return };
        if fullscreen || window.mode != WindowMode::Windowed || window.resolution.scale_factor_override().is_some() {
            return;
        }
        window.visible = false;
        let kept = if self.platform { platform::kept() } else { None };
        let mut fitting = world.resource_mut::<Fitting>();
        fitting.kept = kept;
        fitting.stage = Stage::Measure(0);
    }
}

/// Sizes the hidden window on the first frame a monitor is known, then
/// places and shows it on the next.
///
/// The primary monitor, where a new window opens; any monitor when none is
/// marked primary, as on Wayland.
fn open(mut fitting: ResMut<Fitting>, mut windows: Query<&mut Window, With<PrimaryWindow>>, monitors: Query<(&Monitor, Has<PrimaryMonitor>)>) {
    if fitting.stage == Stage::Done {
        return;
    }
    let Ok(mut window) = windows.single_mut() else { return };
    match fitting.stage {
        Stage::Measure(waited) => {
            let monitor = monitors.iter().find(|(_, primary)| *primary).or_else(|| monitors.iter().next());
            let Some((monitor, primary)) = monitor else {
                fitting.stage = if waited + 1 >= GIVE_UP {
                    window.visible = true;
                    Stage::Done
                } else {
                    Stage::Measure(waited + 1)
                };
                return;
            };
            let kept = fitting.kept.filter(|_| primary).unwrap_or(KEPT);
            let scale = monitor.scale_factor as f32;
            let screen = Screen::new(monitor.physical_size(), scale, kept);
            let opening = opening(fitting.cols, fitting.rows, fitting.cell, &screen);
            window.resolution.set(opening.size.x as f32, opening.size.y as f32);
            info!(
                "the window opens at {} by {} logical pixels, on a screen of {} by {} keeping {kept:?}",
                opening.size.x, opening.size.y, screen.size.x, screen.size.y
            );
            fitting.stage = Stage::Place(physical_frame(&opening, monitor.physical_position, scale));
        }
        Stage::Place(at) => {
            window.position = WindowPosition::At(at);
            window.visible = true;
            fitting.stage = Stage::Done;
        }
        Stage::Done => {}
    }
}

/// What the system keeps, read from the platform.
#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::NSScreen;
    use objc2_foundation::MainThreadMarker;

    use super::Insets;

    /// The menu bar and the dock on the primary screen, the first AppKit
    /// lists, from how far its visible frame falls short of its frame.
    /// AppKit's y runs up from the bottom. `None` off the main thread,
    /// where AppKit cannot be asked.
    pub(super) fn kept() -> Option<Insets> {
        let main = MainThreadMarker::new()?;
        let screens = NSScreen::screens(main);
        let primary = screens.first()?;
        let (frame, visible) = (primary.frame(), primary.visibleFrame());
        Some(Insets {
            top: (frame.max().y - visible.max().y) as f32,
            bottom: (visible.min().y - frame.min().y) as f32,
            left: (visible.min().x - frame.min().x) as f32,
            right: (frame.max().x - visible.max().x) as f32,
        })
    }
}

/// What the system keeps, where the platform is not asked.
#[cfg(not(target_os = "macos"))]
mod platform {
    use super::Insets;

    pub(super) fn kept() -> Option<Insets> {
        None
    }
}

/// Logical pixels kept for the window's own title bar, above the area the
/// grid is drawn in: macOS draws one of 28, Windows one of about 31.
pub(crate) const TITLE_BAR: f32 = 32.0;

/// Logical pixels kept clear between the window and each edge of the
/// usable area, so it neither touches the dock nor fills the screen
/// edge to edge as if it were fullscreen.
pub(crate) const GAP: f32 = 16.0;

/// What the system keeps for itself along each edge of a monitor, in
/// logical pixels: a menu bar, a dock, a taskbar.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct Insets {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

/// A monitor as a window sees it: its size in logical pixels, its scale
/// factor, and the part of it the system leaves to windows, relative to
/// its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Screen {
    pub size: Vec2,
    pub scale: f32,
    pub usable: Rect,
}

impl Screen {
    /// A monitor `physical` pixels across at `scale`, with `insets` kept
    /// by the system.
    pub(crate) fn new(physical: UVec2, scale: f32, insets: Insets) -> Self {
        let size = physical.as_vec2() / scale;
        let usable = Rect::new(insets.left, insets.top, size.x - insets.right, size.y - insets.bottom);
        Self { size, scale, usable }
    }
}

/// Where a window opens and how large, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Opening {
    /// The area the grid is drawn in, in whole logical pixels.
    pub size: UVec2,
    /// The top-left corner of the window's frame, its title bar included,
    /// relative to the monitor's top-left corner.
    pub frame: Vec2,
}

/// Where a grid of `cols` by `rows` cells of `cell` logical pixels opens on
/// `screen`.
///
/// The size is whole logical pixels, since that is what a window is
/// asked for before it exists, and the renderer draws its largest cell in
/// it: the cell [`layout::fit`] gives for the whole room, in the smallest
/// window that draws that cell. The renderer zooms both axes alike and
/// floors each, so a window exactly the grid's size at that cell may draw
/// a smaller one on the other axis; the window is the grid's declared size
/// zoomed by as much as the larger of the cell's two ratios needs, which
/// leaves a border under a pixel a column on the other axis. That window
/// is never larger than the room, since the room draws the same cell. A
/// room smaller than the grid at its declared size gives a window smaller
/// than that, and a room of no size a window of one pixel.
pub(crate) fn opening(cols: i32, rows: i32, cell: Vec2, screen: &Screen) -> Opening {
    let room = (screen.usable.size() - Vec2::splat(2.0 * GAP) - Vec2::new(0.0, TITLE_BAR)).floor().max(Vec2::ONE);
    let pixels = (room * screen.scale + ROUNDING).floor().as_uvec2();
    let largest = layout::fit(cols, rows, cell, pixels, screen.scale).cell.as_vec2() / (cell * screen.scale);
    let native = Vec2::new(cols.max(1) as f32 * cell.x, rows.max(1) as f32 * cell.y) * screen.scale;
    // In physical pixels first, since a window asked for in logical ones
    // is that many times the scale, truncated.
    let needed = (native * largest.max_element() - ROUNDING).ceil();
    let size = (needed / screen.scale - ROUNDING).ceil().min(room).max(Vec2::ONE);
    let outer = size + Vec2::new(0.0, TITLE_BAR);
    let frame = (screen.usable.min + (screen.usable.size() - outer) / 2.0).round().max(screen.usable.min.max(Vec2::ZERO));
    Opening { size: size.as_uvec2(), frame }
}

/// What a float's rounding may cost before a floor or a ceiling, in
/// pixels: far under one, so a size that is whole stays whole.
const ROUNDING: f32 = 1e-3;

/// The top-left corner, in physical pixels on the desktop, of a window's
/// frame at `opening` on a monitor whose top-left corner is at `monitor`.
pub(crate) fn physical_frame(opening: &Opening, monitor: IVec2, scale: f32) -> IVec2 {
    monitor + (opening.frame * scale).round().as_ivec2()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fit;
    use bevy::window::WindowResolution;

    const CELL: Vec2 = Vec2::new(10.0, 16.0);

    /// The user's screen: a 13-inch Retina panel at 1440 by 900 points, a
    /// 24-point menu bar and a dock of 68-point icons along the bottom.
    fn laptop() -> Screen {
        Screen::new(UVec2::new(2880, 1800), 2.0, Insets { top: 24.0, bottom: 78.0, ..default() })
    }

    fn default() -> Insets {
        Insets::default()
    }

    /// A headless app with the plugin, never asking the platform, and a
    /// primary window of the grid's declared size, as Bevy holds it once
    /// made on a Retina display.
    fn app(fullscreen: bool) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(crate::FullscreenPlugin)
            .add_plugins(FitScreenPlugin { platform: false, ..FitScreenPlugin::new(80, 40, CELL) });
        let mut resolution = WindowResolution::new(800, 640);
        resolution.set_scale_factor_and_apply_to_physical_size(2.0);
        let window = app.world_mut().spawn((Window { resolution, ..Window::default() }, PrimaryWindow)).id();
        if fullscreen {
            let mut settings = app.world_mut().resource_mut::<Settings>();
            let id = settings.find(crate::fullscreen::FULLSCREEN).unwrap();
            settings.choose(id, 1);
        }
        app.finish();
        app.cleanup();
        (app, window)
    }

    fn monitor(app: &mut App, physical: UVec2, at: IVec2, primary: bool) {
        let monitor = Monitor {
            name: None,
            physical_width: physical.x,
            physical_height: physical.y,
            physical_position: at,
            refresh_rate_millihertz: None,
            scale_factor: 2.0,
            video_modes: Vec::new(),
        };
        let id = app.world_mut().spawn(monitor).id();
        if primary {
            app.world_mut().entity_mut(id).insert(PrimaryMonitor);
        }
    }

    fn window(app: &App, window: Entity) -> &Window {
        app.world().get::<Window>(window).unwrap()
    }

    /// Monitors from smaller than the grid to far larger, at the scale
    /// factors displays come in, with and without a dock down one side.
    fn screens() -> impl Iterator<Item = Screen> {
        let factors = [1.0_f32, 1.25, 1.5, 2.0, 3.0];
        let insets = [default(), Insets { top: 24.0, bottom: 78.0, ..default() }, Insets { top: 37.0, left: 90.0, ..default() }];
        factors.into_iter().flat_map(move |scale| {
            insets.into_iter().flat_map(move |inset| {
                (0..40).map(move |i| {
                    let logical = Vec2::new(500.0 + i as f32 * 71.0, 380.0 + i as f32 * 43.0);
                    Screen::new((logical * scale).as_uvec2(), scale, inset)
                })
            })
        })
    }

    const GRIDS: [(i32, i32); 3] = [(80, 40), (138, 55), (41, 13)];

    /// The room for the drawing area: the usable area less the gap all
    /// round and the title bar.
    fn room(screen: &Screen) -> Vec2 {
        screen.usable.size() - Vec2::splat(2.0 * GAP) - Vec2::new(0.0, TITLE_BAR)
    }

    /// The window's physical size, as Bevy works it out from a logical
    /// one: multiplied by the scale and truncated.
    fn physical(size: UVec2, scale: f32) -> UVec2 {
        (size.as_vec2() * scale).as_uvec2()
    }

    #[test]
    fn on_the_laptop_the_grid_opens_larger_keeping_its_shape() {
        let screen = laptop();
        let open = opening(80, 40, CELL, &screen);
        let f = fit(80, 40, CELL, physical(open.size, 2.0), 2.0);
        assert!(f.cell.x > 20 && f.cell.y > 32, "larger than the native 20 by 32 pixels: {f:?}");
        assert!((f.cell.x as f32 - f.cell.y as f32 * CELL.x / CELL.y).abs() < 1.0, "the cell keeps its shape: {f:?}");
        assert_eq!(f.cell, UVec2::new(22, 36), "{f:?}");
        assert_eq!(f.margin, UVec2::new(20, 0), "a border of ten points down each side: {f:?}");
        assert_eq!(open.size, UVec2::new(900, 720), "{open:?}");
        assert_eq!(open.frame, Vec2::new(270.0, 47.0), "centred in the usable area: {open:?}");
    }

    #[test]
    fn the_window_and_its_title_bar_stay_inside_the_usable_area_less_the_gap() {
        for (cols, rows) in GRIDS {
            for screen in screens() {
                let open = opening(cols, rows, CELL, &screen);
                let (top_left, bottom_right) = (open.frame, open.frame + open.size.as_vec2() + Vec2::new(0.0, TITLE_BAR));
                let inner = Rect::from_corners(screen.usable.min + GAP, screen.usable.max - GAP);
                if room(&screen).cmpge(Vec2::new(cols as f32, rows as f32)).all() {
                    assert!(top_left.cmpge(inner.min).all() && bottom_right.cmple(inner.max).all(), "{cols}x{rows} on {screen:?}: {open:?}");
                }
            }
        }
    }

    #[test]
    fn the_window_is_centred_in_the_usable_area_to_within_a_pixel() {
        for (cols, rows) in GRIDS {
            for screen in screens() {
                let open = opening(cols, rows, CELL, &screen);
                let outer = open.size.as_vec2() + Vec2::new(0.0, TITLE_BAR);
                let left = open.frame - screen.usable.min;
                let right = screen.usable.max - (open.frame + outer);
                assert!((left - right).abs().cmple(Vec2::ONE).all(), "{cols}x{rows} on {screen:?}: {open:?}");
            }
        }
    }

    #[test]
    fn the_renderer_draws_the_largest_cell_the_room_allows_in_the_smallest_window_that_draws_it() {
        for (cols, rows) in GRIDS {
            for screen in screens() {
                let open = opening(cols, rows, CELL, &screen);
                let room = (room(&screen).floor() * screen.scale + 1e-3).floor().as_uvec2();
                if room.x < cols as u32 || room.y < rows as u32 {
                    continue;
                }
                let largest = fit(cols, rows, CELL, room, screen.scale);
                let drawn = fit(cols, rows, CELL, physical(open.size, screen.scale), screen.scale);
                assert_eq!(drawn.cell, largest.cell, "{cols}x{rows} on {screen:?}: {open:?}");
                assert!(
                    drawn.margin.x < cols as u32 && drawn.margin.y < rows as u32,
                    "under a pixel a column of border: {cols}x{rows} on {screen:?}: {drawn:?}"
                );
                let smaller = fit(cols, rows, CELL, physical(open.size - UVec2::ONE, screen.scale), screen.scale);
                assert_ne!(smaller.cell, drawn.cell, "a logical pixel less draws a smaller cell: {cols}x{rows} on {screen:?}: {open:?}");
            }
        }
    }

    #[test]
    fn the_window_is_never_smaller_than_native_unless_the_room_is() {
        for (cols, rows) in GRIDS {
            let native = Vec2::new(cols as f32 * CELL.x, rows as f32 * CELL.y);
            // The factors at which the declared cell is whole pixels; at
            // 1.25 the renderer itself draws it smaller.
            for screen in screens().filter(|s| s.scale.fract() == 0.0 || s.scale == 1.5) {
                let open = opening(cols, rows, CELL, &screen);
                if room(&screen).cmpge(native).all() {
                    assert!(open.size.as_vec2().cmpge(native).all(), "{cols}x{rows} on {screen:?}: {open:?}");
                } else {
                    assert!(open.size.as_vec2().cmplt(native).any(), "{cols}x{rows} on {screen:?}: {open:?}");
                }
            }
        }
    }

    #[test]
    fn a_screen_with_no_room_still_opens_a_window_of_a_pixel() {
        let screen = Screen::new(UVec2::new(40, 30), 1.0, default());
        let open = opening(80, 40, CELL, &screen);
        assert!(open.size.cmpge(UVec2::ONE).all(), "{open:?}");
        assert!(open.frame.cmpge(Vec2::ZERO).all(), "{open:?}");
    }

    #[test]
    fn a_window_stays_hidden_until_a_monitor_is_known_then_is_sized_then_placed_and_shown() {
        let (mut app, id) = app(false);
        assert!(!window(&app, id).visible, "hidden before it is made");
        app.update();
        assert!(!window(&app, id).visible, "no monitor yet");
        assert_eq!(window(&app, id).resolution.size(), Vec2::new(800.0, 640.0));
        monitor(&mut app, UVec2::new(2880, 1800), IVec2::ZERO, true);
        app.update();
        let expected = opening(80, 40, CELL, &Screen::new(UVec2::new(2880, 1800), 2.0, KEPT));
        assert_eq!(window(&app, id).resolution.size(), expected.size.as_vec2(), "sized first");
        assert_eq!(window(&app, id).resolution.physical_size(), expected.size * 2);
        assert!(!window(&app, id).visible, "still hidden while it is sized");
        assert_eq!(window(&app, id).position, WindowPosition::Automatic);
        app.update();
        assert_eq!(window(&app, id).position, WindowPosition::At((expected.frame * 2.0).as_ivec2()), "then placed");
        assert!(window(&app, id).visible, "and shown");
        app.update();
        assert_eq!(window(&app, id).resolution.size(), expected.size.as_vec2(), "and then left to the player");
    }

    #[test]
    fn the_window_opens_on_the_primary_monitor_where_it_lies_on_the_desktop() {
        let (mut app, id) = app(false);
        monitor(&mut app, UVec2::new(1280, 1024), IVec2::new(-1280, 0), false);
        monitor(&mut app, UVec2::new(2880, 1800), IVec2::new(0, 0), false);
        monitor(&mut app, UVec2::new(3840, 2160), IVec2::new(2880, -200), true);
        app.update();
        app.update();
        let expected = opening(80, 40, CELL, &Screen::new(UVec2::new(3840, 2160), 2.0, KEPT));
        assert_eq!(window(&app, id).resolution.size(), expected.size.as_vec2());
        assert_eq!(window(&app, id).position, WindowPosition::At(IVec2::new(2880, -200) + (expected.frame * 2.0).as_ivec2()));
    }

    #[test]
    fn a_window_opening_fullscreen_is_left_as_it_opens() {
        let (mut app, id) = app(true);
        monitor(&mut app, UVec2::new(2880, 1800), IVec2::ZERO, true);
        for _ in 0..3 {
            app.update();
        }
        assert!(window(&app, id).visible);
        assert_eq!(window(&app, id).resolution.size(), Vec2::new(800.0, 640.0));
        assert_eq!(window(&app, id).position, WindowPosition::Automatic);
    }

    #[test]
    fn a_window_no_monitor_is_ever_reported_for_is_shown_as_it_was_made() {
        let (mut app, id) = app(false);
        for _ in 0..GIVE_UP {
            app.update();
        }
        assert!(window(&app, id).visible);
        assert_eq!(window(&app, id).resolution.size(), Vec2::new(800.0, 640.0));
    }
}
