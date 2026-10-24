//! What a game and a panel call on the [`App`] while it is put together,
//! in one trait, as [`EngineApp`](atglyph_bevy::app::EngineApp) is the
//! engine's: one name to import rather than one per method.

use bevy::prelude::*;

use crate::controls::{ControlId, Keys};

/// The UI's methods on [`App`].
///
/// Each makes sure the resource it declares into exists first, so it works
/// whether it comes before [`UiPlugin`](crate::UiPlugin) or after.
pub trait UiApp {
    /// Declares the tone `name` and colours it `color`, in one call.
    ///
    /// Look the id up again with [`Tones::get`](crate::tone::Tones::get)
    /// where it is used.
    fn add_tone(&mut self, name: &str, color: Color) -> &mut Self;

    /// Declares a control under `group` and returns its id. See
    /// `Controls::add`.
    fn add_control(&mut self, group: &str, action: &str, keys: impl Into<Keys>) -> ControlId;

    /// Declares the modal `name`, so a key handler or a run condition can
    /// look its id up by that name.
    fn add_modal(&mut self, name: &str) -> &mut Self;
}

impl UiApp for App {
    fn add_tone(&mut self, name: &str, color: Color) -> &mut Self {
        crate::tone::add_tone(self, name, color)
    }

    fn add_control(&mut self, group: &str, action: &str, keys: impl Into<Keys>) -> ControlId {
        crate::controls::add_control(self, group, action, keys)
    }

    fn add_modal(&mut self, name: &str) -> &mut Self {
        crate::modal::add_modal(self, name)
    }
}
