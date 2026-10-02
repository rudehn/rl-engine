//! What a player chooses once and expects to find again: declared once,
//! read everywhere, and listed.
//!
//! A setting is a name, where it is listed, and the choices it has. It is
//! plain data in the one crate the renderer, the UI and saving all depend
//! on, so each does its own part without knowing the others: whoever owns
//! the thing a setting changes declares it and reads it, the settings
//! screen lists and changes whatever was declared, and saving remembers
//! names and words without knowing what any of them mean.
//!
//! A registry, not an enum: a game's settings are declared the same way
//! as the engine's, and neither needs a variant anywhere.
//!
//! Declare in a plugin's `build`. What was remembered is recalled once
//! every plugin has built, and a setting declared after that starts on
//! its default.
//!
//! ```
//! # use bevy::prelude::*;
//! # use rl_bevy::settings::{AddSettings, Setting, Settings};
//! let mut app = App::new();
//! let trails = app.add_setting(Setting::new("trails", "Display", "Trails", ["On", "Off"]));
//! assert_eq!(app.world().resource::<Settings>().choice(trails), "On");
//! ```

use std::collections::BTreeMap;

use bevy::prelude::*;

/// One thing a player may choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    /// The word it is remembered under. Stable: renaming it forgets what
    /// every player chose.
    pub name: String,
    /// The heading it is listed under.
    pub group: String,
    /// What it is called on the screen.
    pub label: String,
    /// What it may be set to, in the order a player cycles through.
    pub choices: Vec<String>,
    /// The choice it starts on, by its place in `choices`.
    pub default: usize,
    /// A key that cycles it from anywhere, if it has one.
    pub key: Option<KeyCode>,
}

impl Setting {
    /// A setting remembered as `name`, listed under `group` as `label`,
    /// starting on the first of `choices`.
    pub fn new(name: impl Into<String>, group: impl Into<String>, label: impl Into<String>, choices: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { name: name.into(), group: group.into(), label: label.into(), choices: choices.into_iter().map(Into::into).collect(), default: 0, key: None }
    }

    /// Starts on the choice at `index` instead of the first.
    pub fn default_choice(mut self, index: usize) -> Self {
        self.default = index;
        self
    }

    /// Cycled by `key` as well as from the settings screen.
    pub fn key(mut self, key: KeyCode) -> Self {
        self.key = Some(key);
        self
    }
}

/// A declared setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SettingId(u16);

/// Every setting declared, in the order it was declared, and what each is
/// set to.
///
/// A reader that acts on a change watches the resource with Bevy's change
/// detection and compares against what it last applied; the registry does
/// not say which row moved, because the readers are few and each knows
/// its own.
#[derive(Resource, Debug, Clone, Default)]
pub struct Settings {
    rows: Vec<(Setting, usize)>,
}

impl Settings {
    /// Declares a setting and returns its id.
    ///
    /// # Panics
    /// Panics if the name is already declared, or if it has no choices or
    /// a default past the end of them: each is a mistake in a game's
    /// setup, not something to carry on from.
    pub fn add(&mut self, setting: Setting) -> SettingId {
        assert!(self.find(&setting.name).is_none(), "the setting `{}` is declared twice", setting.name);
        assert!(!setting.choices.is_empty(), "the setting `{}` has no choices", setting.name);
        assert!(setting.default < setting.choices.len(), "the setting `{}` defaults to a choice it does not have", setting.name);
        let chosen = setting.default;
        self.rows.push((setting, chosen));
        SettingId((self.rows.len() - 1) as u16)
    }

    /// The setting `id` names.
    ///
    /// # Panics
    /// Panics on an id from another registry.
    pub fn get(&self, id: SettingId) -> &Setting {
        &self.rows[id.0 as usize].0
    }

    /// The setting remembered as `name`.
    pub fn find(&self, name: &str) -> Option<SettingId> {
        self.rows.iter().position(|(s, _)| s.name == name).map(|i| SettingId(i as u16))
    }

    /// What `id` is set to, by its place in the setting's choices.
    pub fn chosen(&self, id: SettingId) -> usize {
        self.rows[id.0 as usize].1
    }

    /// What `id` is set to, in words.
    pub fn choice(&self, id: SettingId) -> &str {
        let (setting, chosen) = &self.rows[id.0 as usize];
        &setting.choices[*chosen]
    }

    /// Sets `id` to the choice at `index`, held to the last one.
    pub fn choose(&mut self, id: SettingId, index: usize) {
        let (setting, chosen) = &mut self.rows[id.0 as usize];
        *chosen = index.min(setting.choices.len() - 1);
    }

    /// Moves `id` `step` choices along, wrapping round at both ends.
    pub fn cycle(&mut self, id: SettingId, step: i32) {
        let (setting, chosen) = &mut self.rows[id.0 as usize];
        *chosen = (*chosen as i32 + step).rem_euclid(setting.choices.len() as i32) as usize;
    }

    /// Every setting with what it is set to, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (SettingId, &Setting, usize)> {
        self.rows.iter().enumerate().map(|(i, (s, chosen))| (SettingId(i as u16), s, *chosen))
    }

    /// Whether nothing was declared.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Each name with the words of its choice: what saving writes down.
    ///
    /// Words rather than a place in the list, so a choice added in the
    /// middle of a setting does not quietly turn everyone's into another.
    /// Ordered, so the file is the same from one write to the next.
    pub fn remembered(&self) -> BTreeMap<String, String> {
        self.rows.iter().map(|(s, chosen)| (s.name.clone(), s.choices[*chosen].clone())).collect()
    }

    /// Sets every declared setting `kept` names to the choice it names.
    ///
    /// A name nobody declared and a choice the setting no longer has are
    /// passed over: what a player chose in an older build must not stop a
    /// newer one starting.
    pub fn recall(&mut self, kept: &BTreeMap<String, String>) {
        for (setting, chosen) in &mut self.rows {
            if let Some(index) = kept.get(&setting.name).and_then(|words| setting.choices.iter().position(|c| c == words)) {
                *chosen = index;
            }
        }
    }
}

/// Declaring settings while the app is built.
pub trait AddSettings {
    /// Declares a setting and returns its id. See [`Settings::add`].
    fn add_setting(&mut self, setting: Setting) -> SettingId;
}

impl AddSettings for App {
    fn add_setting(&mut self, setting: Setting) -> SettingId {
        self.init_resource::<Settings>();
        self.world_mut().resource_mut::<Settings>().add(setting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> (Settings, SettingId, SettingId) {
        let mut settings = Settings::default();
        let glow = settings.add(Setting::new("glow", "Display", "Glow", ["Off", "On"]));
        let pace = settings.add(Setting::new("pace", "Play", "Pace", ["Slow", "Even", "Fast"]).default_choice(1));
        (settings, glow, pace)
    }

    #[test]
    fn a_setting_starts_on_its_default_and_the_first_choice_is_the_default_default() {
        let (settings, glow, pace) = two();
        assert_eq!((settings.chosen(glow), settings.choice(glow)), (0, "Off"));
        assert_eq!((settings.chosen(pace), settings.choice(pace)), (1, "Even"));
    }

    #[test]
    fn cycling_wraps_round_at_both_ends() {
        let (mut settings, _, pace) = two();
        settings.cycle(pace, 1);
        assert_eq!(settings.choice(pace), "Fast");
        settings.cycle(pace, 1);
        assert_eq!(settings.choice(pace), "Slow");
        settings.cycle(pace, -1);
        assert_eq!(settings.choice(pace), "Fast");
    }

    #[test]
    fn settings_are_listed_in_the_order_they_were_declared() {
        let (settings, ..) = two();
        assert_eq!(settings.iter().map(|(_, s, _)| s.name.as_str()).collect::<Vec<_>>(), ["glow", "pace"]);
    }

    #[test]
    #[should_panic(expected = "the setting `glow` is declared twice")]
    fn declaring_one_name_twice_panics_and_names_it() {
        let (mut settings, ..) = two();
        settings.add(Setting::new("glow", "Other", "Glow again", ["A", "B"]));
    }

    #[test]
    #[should_panic(expected = "the setting `bare` has no choices")]
    fn a_setting_with_no_choices_panics_when_declared() {
        Settings::default().add(Setting::new("bare", "Display", "Bare", Vec::<String>::new()));
    }

    #[test]
    fn what_is_remembered_is_each_name_and_the_words_of_its_choice() {
        let (mut settings, _, pace) = two();
        settings.choose(pace, 2);
        let kept = settings.remembered();
        assert_eq!(kept.get("glow").map(String::as_str), Some("Off"));
        assert_eq!(kept.get("pace").map(String::as_str), Some("Fast"));
    }

    #[test]
    fn recalling_skips_a_name_nobody_declared_and_a_choice_that_is_gone() {
        let (mut settings, glow, pace) = two();
        let kept =
            BTreeMap::from([("glow".to_string(), "On".to_string()), ("pace".to_string(), "Breakneck".to_string()), ("gone".to_string(), "On".to_string())]);
        settings.recall(&kept);
        assert_eq!(settings.choice(glow), "On");
        assert_eq!(settings.choice(pace), "Even", "an unknown choice leaves the default");
    }

    #[test]
    fn an_app_declares_a_setting_without_being_told_to_make_the_registry_first() {
        let mut app = App::new();
        let id = app.add_setting(Setting::new("glow", "Display", "Glow", ["Off", "On"]).key(KeyCode::F11));
        assert_eq!(app.world().resource::<Settings>().get(id).key, Some(KeyCode::F11));
    }
}
