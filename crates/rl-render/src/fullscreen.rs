//! Filling the screen, as a setting.
//!
//! The window is the renderer's, so the renderer declares the setting and
//! is the one thing that reads it. The settings screen changes it without
//! knowing what it does, and saving remembers it without knowing either.
//!
//! Borderless rather than exclusive: it takes the display at the
//! resolution it already has, so nothing flickers and the layout in
//! [`layout`](crate::layout) draws the grid for exactly those pixels.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use rl_bevy::settings::{AddSettings, Setting, Settings};

/// The name the fullscreen setting is declared and remembered under.
pub const FULLSCREEN: &str = "fullscreen";

/// Declares the fullscreen setting and keeps the primary window in the
/// mode it asks for.
///
/// On the web it declares nothing. A browser grants fullscreen only from
/// inside the handler of a key or a click, and Bevy reads keys a frame
/// later, so the setting could not be honored; the canvas follows the
/// page instead, and the browser's own fullscreen fills the screen.
pub struct FullscreenPlugin;

impl Plugin for FullscreenPlugin {
    fn build(&self, app: &mut App) {
        if cfg!(target_arch = "wasm32") {
            return;
        }
        app.add_setting(Setting::new(FULLSCREEN, "Display", "Fullscreen", ["Off", "On"]).key(KeyCode::F11));
        app.add_systems(PostUpdate, apply.run_if(resource_changed::<Settings>));
    }

    /// Once, before the first frame and after every plugin has finished,
    /// which is after what was remembered has been recalled: a game left
    /// fullscreen opens that way rather than flashing a window first.
    fn cleanup(&self, app: &mut App) {
        if cfg!(target_arch = "wasm32") {
            return;
        }
        let wanted = wanted(app.world().resource::<Settings>());
        let mut windows = app.world_mut().query_filtered::<&mut Window, With<PrimaryWindow>>();
        if let Ok(mut window) = windows.single_mut(app.world_mut())
            && window.mode != wanted
        {
            window.mode = wanted;
        }
    }
}

/// The mode the setting asks for.
fn wanted(settings: &Settings) -> WindowMode {
    let on = settings.find(FULLSCREEN).is_some_and(|id| settings.chosen(id) == 1);
    if on { WindowMode::BorderlessFullscreen(MonitorSelection::Current) } else { WindowMode::Windowed }
}

/// Puts the window in the mode the setting asks for, when it is not.
///
/// Compared before it is written, so a change to some other setting does
/// not mark the window changed.
fn apply(settings: Res<Settings>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let wanted = wanted(&settings);
    if let Ok(mut window) = windows.single_mut()
        && window.mode != wanted
    {
        window.mode = wanted;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(FullscreenPlugin);
        let window = app.world_mut().spawn((Window::default(), PrimaryWindow)).id();
        (app, window)
    }

    fn mode(app: &App, window: Entity) -> WindowMode {
        app.world().get::<Window>(window).unwrap().mode
    }

    fn set(app: &mut App, on: bool) {
        let mut settings = app.world_mut().resource_mut::<Settings>();
        let id = settings.find(FULLSCREEN).unwrap();
        settings.choose(id, on as usize);
    }

    #[test]
    fn the_setting_drives_the_window_into_fullscreen_and_back() {
        let (mut app, window) = app();
        app.finish();
        app.cleanup();
        app.update();
        assert_eq!(mode(&app, window), WindowMode::Windowed);
        set(&mut app, true);
        app.update();
        assert_eq!(mode(&app, window), WindowMode::BorderlessFullscreen(MonitorSelection::Current));
        set(&mut app, false);
        app.update();
        assert_eq!(mode(&app, window), WindowMode::Windowed);
    }

    #[test]
    fn a_choice_made_before_the_first_frame_is_on_the_window_before_it() {
        let (mut app, window) = app();
        set(&mut app, true);
        app.finish();
        app.cleanup();
        assert_eq!(mode(&app, window), WindowMode::BorderlessFullscreen(MonitorSelection::Current), "no frame has run yet");
    }

    #[test]
    fn with_no_window_nothing_fails() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_plugins(FullscreenPlugin);
        app.finish();
        app.cleanup();
        app.update();
    }
}
