//! The palette chrome is drawn in.

use bevy::prelude::*;

use crate::log::LogCategory;

/// Colours for chrome. A game replaces the resource to restyle everything.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Theme {
    /// Ordinary text.
    pub text: Color,
    /// Dimmer text.
    pub muted: Color,
    /// Panel fill.
    pub panel_bg: Color,
    /// Something good happened.
    pub good: Color,
    /// Something bad happened.
    pub bad: Color,
    /// Something notable happened.
    pub notice: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            text: Color::srgb(0.85, 0.85, 0.80),
            muted: Color::srgb(0.5, 0.5, 0.5),
            panel_bg: Color::srgb(0.06, 0.06, 0.08),
            good: Color::srgb(0.4, 0.85, 0.4),
            bad: Color::srgb(0.9, 0.35, 0.3),
            notice: Color::srgb(0.95, 0.8, 0.3),
        }
    }
}

impl Theme {
    /// The colour a log category is drawn in.
    pub fn log_color(&self, category: LogCategory) -> Color {
        match category {
            LogCategory::Info => self.text,
            LogCategory::Muted => self.muted,
            LogCategory::Good => self.good,
            LogCategory::Bad => self.bad,
            LogCategory::Notice => self.notice,
        }
    }
}
