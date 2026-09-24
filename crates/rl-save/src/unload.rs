//! What is written when the game is left without saying so.
//!
//! A browser tab closes, or a window's close button is pressed, and the
//! run is gone unless something wrote it first. On the web the only
//! chance is the `pagehide` or `beforeunload` event, and it runs outside
//! the app's loop: it cannot borrow the world, run a system or wait for a
//! frame, and it must finish before it returns. So the game does its
//! saving earlier, and the event does only the writing.
//!
//! [`Stash`] is the last save the game encoded, kept in memory. The game
//! refreshes it as often as it likes, every turn or every few, and clears
//! it when a save must not outlive the moment, such as the death that
//! deletes the run. [`UnloadPlugin`] writes whatever is stashed through
//! [`Saves`] when the page is being left and, on every platform, when the
//! app exits, so a native window closed by its button saves too.
//!
//! The hard-won part is that the handler must be synchronous and must
//! outlive the frame that installed it: the closure is leaked on purpose,
//! and it writes straight to storage, with no message and no schedule in
//! between.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use rl_bevy::Needs;

use crate::backend::{SaveBackend, SaveError, Saves};

/// The last save the game encoded, waiting to be written if the game is
/// left.
///
/// Shared by handle, so the unload handler outside the app holds the same
/// stash the game refreshes inside it.
#[derive(Resource, Clone, Default)]
pub struct Stash(Arc<Mutex<Option<(String, String)>>>);

impl Stash {
    /// Keeps `text` as the save to write to `slot`, replacing what was
    /// stashed before.
    pub fn stash(&self, slot: &str, text: &str) {
        *self.0.lock().expect("the stash") = Some((slot.to_string(), text.to_string()));
    }

    /// Forgets what was stashed, so nothing is written on the way out. For
    /// the moment a save is deleted: a death, a run given up.
    pub fn clear(&self) {
        *self.0.lock().expect("the stash") = None;
    }

    /// The slot a save is waiting for, if one is.
    pub fn pending(&self) -> Option<String> {
        self.0.lock().expect("the stash").as_ref().map(|(slot, _)| slot.clone())
    }

    /// Writes what is stashed through `backend`. `Ok(false)` when there was
    /// nothing to write. The stash is kept, since the two unload events a
    /// browser sends may both arrive and writing twice is harmless.
    pub fn flush(&self, backend: &dyn SaveBackend) -> Result<bool, SaveError> {
        let pending = self.0.lock().expect("the stash").clone();
        match pending {
            Some((slot, text)) => backend.persist(&slot, &text).map(|()| true),
            None => Ok(false),
        }
    }
}

/// Writes the [`Stash`] when the game is left: on the web when the page is
/// hidden or unloaded, and everywhere when the app exits.
///
/// Needs [`Saves`], the backend it writes through. The game keeps the stash
/// fresh and clears it when a save must not survive; this plugin decides
/// nothing about either.
pub struct UnloadPlugin;

impl Plugin for UnloadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stash>()
            .needs::<Saves>("UnloadPlugin", "`Saves`, the backend the stash is written through, such as `Saves::platform_default(\"my-game\")`")
            // In the save stage, after the stash is refreshed, so what is
            // written on the way out is the run as it stood last.
            .add_systems(Last, flush_on_exit.in_set(rl_bevy::EndOfFrame::Save).after(crate::run::refresh_stash));
    }

    fn finish(&self, app: &mut App) {
        // The page's handler holds the same backend the game saves through,
        // which is why `Saves` shares its backend rather than owning it.
        #[cfg(target_arch = "wasm32")]
        if let Some(saves) = app.world().get_resource::<Saves>() {
            web::install(app.world().resource::<Stash>().clone(), saves.0.clone());
        }
        let _ = app;
    }
}

/// Writes the stash on the frame the app is told to exit, which is the
/// frame a native window's close button produces.
pub fn flush_on_exit(mut exits: MessageReader<AppExit>, stash: Res<Stash>, saves: Res<Saves>) {
    if exits.read().next().is_none() {
        return;
    }
    match stash.flush(saves.0.as_ref()) {
        Ok(true) => info!("the stashed save was written on exit"),
        Ok(false) => {}
        Err(e) => error!("the stashed save could not be written on exit: {e}"),
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    /// Listens for the page being left and writes the stash then and
    /// there. Both events, because `pagehide` is the one mobile browsers
    /// send and `beforeunload` the one desktop browsers always have.
    pub(super) fn install(stash: Stash, backend: Arc<dyn SaveBackend + Send + Sync>) {
        let Some(window) = web_sys::window() else { return };
        let flush = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            if let Err(e) = stash.flush(backend.as_ref()) {
                error!("the stashed save could not be written as the page was left: {e}");
            }
        });
        for name in ["pagehide", "beforeunload"] {
            if window.add_event_listener_with_callback(name, flush.as_ref().unchecked_ref()).is_err() {
                warn!("the page would not take a {name} listener; a closed tab will not save");
            }
        }
        // Leaked on purpose: the page owns it for as long as the page lasts.
        flush.forget();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MemoryBackend;

    #[test]
    fn the_stash_holds_one_save_writes_it_on_demand_and_forgets_it_when_cleared() {
        let backend = MemoryBackend::default();
        let stash = Stash::default();
        assert!(!stash.flush(&backend).unwrap(), "nothing stashed, nothing written");
        stash.stash("run", "(turn: 1)");
        stash.stash("run", "(turn: 2)");
        assert_eq!(stash.pending().as_deref(), Some("run"));
        assert!(stash.flush(&backend).unwrap());
        assert_eq!(backend.load("run").unwrap().as_deref(), Some("(turn: 2)"), "the latest, not the first");
        assert!(stash.flush(&backend).unwrap(), "kept after a flush, since a browser may send two unload events");
        stash.clear();
        assert_eq!(stash.pending(), None);
        assert!(!stash.flush(&backend).unwrap());
    }

    #[test]
    fn the_stash_is_written_on_the_frame_the_app_exits() {
        let mut app = rl_bevy::plugin::headless_app();
        let backend: Arc<dyn SaveBackend + Send + Sync> = Arc::new(MemoryBackend::default());
        app.insert_resource(Saves(backend.clone())).add_plugins(UnloadPlugin);
        app.finish();
        app.cleanup();
        app.world().resource::<Stash>().stash("run", "(turn: 7)");
        app.update();
        assert!(!backend.exists("run"), "not written while the app runs");
        app.world_mut().write_message(AppExit::Success);
        app.update();
        assert_eq!(backend.load("run").unwrap().as_deref(), Some("(turn: 7)"));
    }
}
