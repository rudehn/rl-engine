//! Tones: the semantic roles a game gives colours to.
//!
//! A widget that took a `Color` would make every game that wanted a
//! different look fork the widget. A widget that takes a [`ToneId`] asks
//! what a thing *means* - ordinary text, a warning, a selection - and
//! leaves what that looks like to the [`Palette`]. Restyling is then one
//! resource, and a game that needs a role the engine never thought of
//! interns its own and every widget honours it without a line changing
//! here.
//!
//! Interned rather than an enum for the same reason damage kinds and
//! statuses are: a closed list of five is a closed list for as long as the
//! crate lives, and the game that needs a sixth is the game that forks.

use bevy::prelude::*;
use rl_core::{Id, Interner};

/// The type [`ToneId`] indexes. Never constructed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tone;

/// A registered tone.
pub type ToneId = Id<Tone>;

/// Every tone in play, in the order they were first named.
///
/// The engine's own are interned first, so their ids are the constants on
/// this type and no lookup is needed to use them.
#[derive(Resource, Debug, Clone)]
pub struct Tones(Interner<Tone>);

impl Tones {
    /// Ordinary text.
    pub const TEXT: ToneId = ToneId::from_raw(0);
    /// Text that matters less than the text beside it.
    pub const MUTED: ToneId = ToneId::from_raw(1);
    /// Good news.
    pub const GOOD: ToneId = ToneId::from_raw(2);
    /// Bad news.
    pub const BAD: ToneId = ToneId::from_raw(3);
    /// Worth noticing, neither good nor bad.
    pub const NOTICE: ToneId = ToneId::from_raw(4);
    /// A heading or a title.
    pub const TITLE: ToneId = ToneId::from_raw(5);
    /// Box borders and rules.
    pub const FRAME: ToneId = ToneId::from_raw(6);
    /// What a panel is drawn on.
    pub const SURFACE: ToneId = ToneId::from_raw(7);
    /// The background of whatever the cursor is on.
    pub const SELECT: ToneId = ToneId::from_raw(8);

    /// The engine's tones, in the order their ids are handed out. The
    /// constants above are the indices into this.
    pub const BUILT_IN: [&'static str; 9] = ["text", "muted", "good", "bad", "notice", "title", "frame", "surface", "select"];

    /// The id for `name`, assigning a new one if it is unseen.
    ///
    /// Call it while the app is being built, keep the id, and pass it to
    /// whatever needs it. Interning during play works and is stable, but a
    /// tone first seen mid-run has no colour until something sets one.
    pub fn declare(&mut self, name: &str) -> ToneId {
        self.0.intern(name)
    }

    /// The id for `name`, if it has been declared.
    pub fn get(&self, name: &str) -> Option<ToneId> {
        self.0.get(name)
    }

    /// The name behind `tone`.
    ///
    /// # Panics
    /// Panics if `tone` did not come from this registry.
    pub fn name(&self, tone: ToneId) -> &str {
        self.0.name(tone)
    }

    /// How many tones exist.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether no tone has been declared, which never happens after
    /// [`Default`] has run.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Every tone, with its name.
    pub fn iter(&self) -> impl Iterator<Item = (ToneId, &str)> {
        self.0.iter()
    }
}

impl Default for Tones {
    fn default() -> Self {
        let mut names = Interner::new();
        for name in Tones::BUILT_IN {
            names.intern(name);
        }
        Self(names)
    }
}

/// A colour per tone, indexed by the tone's dense id.
///
/// Replace the resource, or set one tone, and every widget that asks for
/// that tone changes at once. Ids are dense and handed out in order, so
/// this is a `Vec` and not a map: reading a colour is an index.
#[derive(Resource, Debug, Clone)]
pub struct Palette(Vec<Option<Color>>);

impl Palette {
    /// A palette with nothing set.
    pub fn blank() -> Self {
        Self(Vec::new())
    }

    /// The colour of `tone`, or the colour of [`Tones::TEXT`] when it has
    /// none.
    ///
    /// Falls back rather than panicking because a missing colour is a
    /// styling mistake and not a reason to end a run; [`unset`](Self::unset)
    /// reports it at startup instead, once, by name.
    pub fn get(&self, tone: ToneId) -> Color {
        self.0.get(tone.index()).copied().flatten().or_else(|| self.0.first().copied().flatten()).unwrap_or(Color::srgb(1.0, 0.0, 1.0))
    }

    /// Sets the colour of `tone`.
    pub fn set(&mut self, tone: ToneId, color: Color) -> &mut Self {
        if self.0.len() <= tone.index() {
            self.0.resize(tone.index() + 1, None);
        }
        self.0[tone.index()] = Some(color);
        self
    }

    /// Every declared tone that has no colour, by name.
    pub fn unset<'a>(&self, tones: &'a Tones) -> Vec<&'a str> {
        tones.iter().filter(|(id, _)| self.0.get(id.index()).copied().flatten().is_none()).map(|(_, name)| name).collect()
    }
}

impl Default for Palette {
    fn default() -> Self {
        let mut p = Palette::blank();
        p.set(Tones::TEXT, Color::srgb(0.85, 0.85, 0.80))
            .set(Tones::MUTED, Color::srgb(0.50, 0.50, 0.50))
            .set(Tones::GOOD, Color::srgb(0.40, 0.85, 0.40))
            .set(Tones::BAD, Color::srgb(0.90, 0.35, 0.30))
            .set(Tones::NOTICE, Color::srgb(0.95, 0.80, 0.30))
            .set(Tones::TITLE, Color::srgb(0.95, 0.90, 0.70))
            .set(Tones::FRAME, Color::srgb(0.45, 0.45, 0.50))
            .set(Tones::SURFACE, Color::srgb(0.06, 0.06, 0.08))
            .set(Tones::SELECT, Color::srgb(0.20, 0.22, 0.30));
        p
    }
}

/// Warns once, when play begins, about tones nothing gave a colour.
///
/// A game that declares a tone and forgets the colour would otherwise see
/// its rows silently in the text colour and wonder why; this names them.
pub fn report_unset_tones(tones: Res<Tones>, palette: Res<Palette>) {
    let missing = palette.unset(&tones);
    if !missing.is_empty() {
        warn!("these tones have no colour and will draw as text: {}", missing.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_constants_are_the_ids_the_names_intern_to() {
        let tones = Tones::default();
        assert_eq!(tones.get("text"), Some(Tones::TEXT));
        assert_eq!(tones.get("select"), Some(Tones::SELECT));
        assert_eq!(tones.len(), Tones::BUILT_IN.len());
        assert_eq!(tones.name(Tones::NOTICE), "notice");
    }

    #[test]
    fn a_game_tone_gets_its_own_colour_and_leaves_the_engines_alone() {
        let mut tones = Tones::default();
        let mut palette = Palette::default();
        let deadly = tones.declare("deadly");
        assert_eq!(palette.get(deadly), palette.get(Tones::TEXT), "unset falls back to text");
        assert_eq!(palette.unset(&tones), vec!["deadly"]);
        palette.set(deadly, Color::srgb(1.0, 0.2, 0.6));
        assert_ne!(palette.get(deadly), palette.get(Tones::TEXT));
        assert_eq!(palette.get(Tones::BAD), Palette::default().get(Tones::BAD), "the engine's tone is untouched");
        assert!(palette.unset(&tones).is_empty());
    }

    #[test]
    fn declaring_a_tone_twice_gives_the_same_id() {
        let mut tones = Tones::default();
        assert_eq!(tones.declare("deadly"), tones.declare("deadly"));
        assert_eq!(tones.len(), Tones::BUILT_IN.len() + 1);
    }
}
