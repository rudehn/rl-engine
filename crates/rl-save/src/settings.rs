//! Remembering what a player chose.
//!
//! [`Settings`](rl_bevy::settings::Settings) is plain data that knows
//! nothing of storage, and the settings screen draws it without knowing
//! either. This is the third part: one slot, read once before the first
//! frame and written whenever a setting changes.
//!
//! Not a [`Versioned`](crate::Versioned) envelope. That refuses a save
//! whole when its version differs, which is right for a run and wrong for
//! a preference: a player who chose fullscreen should still have it after
//! an upgrade. So the file is a map of names to the words of their
//! choices, and anything in it a build does not recognize is passed over.
//!
//! Apart from the run's slot, and never forgotten with it: a new run, a
//! restart and the end of a run leave it alone.

use std::collections::BTreeMap;

use bevy::prelude::*;
use rl_bevy::plugin::Needs;
use rl_bevy::settings::Settings;

use crate::backend::{SaveBackend, Saves};

/// The slot settings are remembered in.
pub const SETTINGS_SLOT: &str = "settings";

/// Recalls what was remembered before the first frame, and writes it down
/// when a setting changes.
///
/// Opt-in: without it the settings work and start on their defaults every
/// launch. What it recalls is what was declared in a plugin's `build`; a
/// setting declared later starts on its default.
pub struct SettingsSavePlugin;

/// What the slot is known to hold, so a frame that changed nothing, and
/// the load itself, write nothing.
#[derive(Resource, Debug, Default)]
struct Remembered(BTreeMap<String, String>);

impl Plugin for SettingsSavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Settings>()
            .init_resource::<Remembered>()
            .needs::<Saves>("SettingsSavePlugin", "`Saves`, the backend settings are remembered through, such as `Saves::platform_default(\"my-game\")`")
            .add_systems(Last, remember.run_if(resource_changed::<Settings>).run_if(resource_exists::<Saves>));
    }

    /// In `finish`, when every plugin has built and so declared its
    /// settings, and before any plugin's `cleanup`, where the renderer
    /// puts the window in the mode it finds chosen.
    fn finish(&self, app: &mut App) {
        let Some(saves) = app.world().get_resource::<Saves>() else { return };
        let kept = match saves.load(SETTINGS_SLOT) {
            Ok(Some(text)) => ron::from_str::<BTreeMap<String, String>>(&text).unwrap_or_else(|e| {
                warn!("the remembered settings could not be read, so the defaults stand: {e}");
                BTreeMap::new()
            }),
            Ok(None) => BTreeMap::new(),
            Err(e) => {
                warn!("the remembered settings could not be loaded, so the defaults stand: {e}");
                BTreeMap::new()
            }
        };
        let mut settings = app.world_mut().resource_mut::<Settings>();
        settings.recall(&kept);
        let now = settings.remembered();
        app.world_mut().resource_mut::<Remembered>().0 = now;
    }
}

/// Writes the settings down when they are not what the slot holds.
fn remember(settings: Res<Settings>, saves: Res<Saves>, mut remembered: ResMut<Remembered>) {
    let now = settings.remembered();
    if now == remembered.0 {
        return;
    }
    match ron::ser::to_string_pretty(&now, ron::ser::PrettyConfig::default()) {
        Ok(text) => match saves.persist(SETTINGS_SLOT, &text) {
            Ok(()) => remembered.0 = now,
            Err(e) => warn!("the settings could not be remembered: {e}"),
        },
        Err(e) => warn!("the settings could not be written down: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MemoryBackend;
    use rl_bevy::settings::{AddSettings, Setting};
    use std::sync::Arc;

    /// An app with two settings declared and `backend` to remember them
    /// in, finished the way `App::run` finishes one.
    fn game(backend: &Arc<MemoryBackend>) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(Saves(backend.clone()));
        app.add_setting(Setting::new("glow", "Display", "Glow", ["Off", "On"]));
        app.add_setting(Setting::new("pace", "Play", "Pace", ["Slow", "Even", "Fast"]).default_choice(1));
        app.add_plugins(SettingsSavePlugin);
        app.finish();
        app.cleanup();
        app
    }

    fn choice(app: &App, name: &str) -> String {
        let settings = app.world().resource::<Settings>();
        settings.choice(settings.find(name).unwrap()).to_string()
    }

    fn choose(app: &mut App, name: &str, index: usize) {
        let mut settings = app.world_mut().resource_mut::<Settings>();
        let id = settings.find(name).unwrap();
        settings.choose(id, index);
    }

    #[test]
    fn what_a_player_chose_is_there_the_next_time_the_game_is_started() {
        let backend = Arc::new(MemoryBackend::default());
        let mut app = game(&backend);
        app.update();
        choose(&mut app, "glow", 1);
        app.update();
        let again = game(&backend);
        assert_eq!(choice(&again, "glow"), "On", "recalled before any frame has run");
        assert_eq!(choice(&again, "pace"), "Even");
    }

    #[test]
    fn nothing_is_written_until_a_setting_changes() {
        let backend = Arc::new(MemoryBackend::default());
        let mut app = game(&backend);
        app.update();
        app.update();
        assert!(!backend.exists(SETTINGS_SLOT));
    }

    #[test]
    fn loading_what_was_remembered_does_not_write_it_back() {
        let backend = Arc::new(MemoryBackend::default());
        let text = "{\"glow\": \"On\", \"gone\": \"Yes\"}";
        backend.persist(SETTINGS_SLOT, text).unwrap();
        let mut app = game(&backend);
        app.update();
        app.update();
        assert_eq!(backend.load(SETTINGS_SLOT).unwrap().as_deref(), Some(text), "byte for byte what was there");
    }

    #[test]
    fn a_name_nobody_declares_and_a_choice_that_is_gone_are_passed_over() {
        let backend = Arc::new(MemoryBackend::default());
        backend.persist(SETTINGS_SLOT, "{\"glow\": \"Blinding\", \"gone\": \"Yes\", \"pace\": \"Fast\"}").unwrap();
        let app = game(&backend);
        assert_eq!(choice(&app, "glow"), "Off");
        assert_eq!(choice(&app, "pace"), "Fast");
    }

    #[test]
    fn a_file_that_is_not_ron_at_all_leaves_the_defaults_and_is_not_touched() {
        let backend = Arc::new(MemoryBackend::default());
        backend.persist(SETTINGS_SLOT, "this is not a map").unwrap();
        let mut app = game(&backend);
        app.update();
        assert_eq!(choice(&app, "glow"), "Off");
        assert_eq!(backend.load(SETTINGS_SLOT).unwrap().as_deref(), Some("this is not a map"));
    }

    #[test]
    fn a_change_is_written_as_names_and_the_words_of_their_choices() {
        let backend = Arc::new(MemoryBackend::default());
        let mut app = game(&backend);
        app.update();
        choose(&mut app, "pace", 2);
        app.update();
        let text = backend.load(SETTINGS_SLOT).unwrap().expect("written");
        let kept: BTreeMap<String, String> = ron::from_str(&text).unwrap();
        assert_eq!(kept, BTreeMap::from([("glow".to_string(), "Off".to_string()), ("pace".to_string(), "Fast".to_string())]));
    }

    #[test]
    fn the_plugin_says_it_needs_a_backend() {
        let mut app = rl_bevy::plugin::headless_app();
        app.add_plugins(SettingsSavePlugin);
        let missing = app.world().resource::<rl_bevy::plugin::Requirements>().missing(app.world());
        assert!(missing.iter().any(|m| m.contains("SettingsSavePlugin") && m.contains("Saves")), "{missing:?}");
    }
}
