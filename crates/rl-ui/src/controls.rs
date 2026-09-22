//! The keys a game answers to: declared once, read through, and listed.
//!
//! The registry is for keys that work while the world is in front of the
//! player: walking, looking, a game's own verbs, and the key that opens
//! each screen. A key that only means something while a screen is up is
//! that screen's own, read only while it is the top one, and written along
//! its bottom border where the player is already looking. It is not
//! declared here: two `a`s in one game are no clash when one of them can
//! only be pressed inside a modal, and a second list of a screen's keys is
//! a second place to keep in step.
//!
//! A game that checks `KeyCode::KeyG` in one system and prints "g get" in
//! another holds two copies of one fact, and the copy on screen is the one
//! nobody updates. So a game declares each control once, as a group, an
//! action and the keys that ask for it, keeps the [`ControlId`] it gets
//! back, and asks [`ControlInput`] whether that control was pressed. The
//! controls screen lists the same registry, so what it says is what the
//! game reads.
//!
//! ```
//! # use bevy::prelude::*;
//! # use rl_ui::{AddControls, Chord, ControlId, ControlInput, UiPlugin};
//! #[derive(Resource)]
//! struct Binds {
//!     pick_up: ControlId,
//! }
//!
//! fn pick_up(keys: ControlInput, binds: Res<Binds>) {
//!     if keys.just_pressed(binds.pick_up) {
//!         // write the intent
//!     }
//! }
//!
//! let mut app = App::new();
//! app.add_plugins(UiPlugin);
//! let pick_up_key = app.add_control("Items", "pick up", [KeyCode::KeyG, KeyCode::Comma]);
//! app.insert_resource(Binds { pick_up: pick_up_key }).add_systems(Update, pick_up);
//! ```
//!
//! The engine's own keys are in the registry too, declared by the plugins
//! that read them, but their keys are never copied in: [`Keys::Engine`]
//! names the binding and the key is read from [`DirectionKeys`],
//! [`CursorKeys`], [`ScrollbackKeys`], [`InventoryKeys`] or [`ControlsKeys`]
//! every time it is listed, so a game that rebinds a cursor key sees the new
//! key on the screen without telling anyone.
//!
//! A [`Chord`] is a key with Shift held or not, and is matched exactly: `l`
//! is not pressed while Shift is down. So `L` and `l` can mean different
//! things without either system checking for the other.

use bevy::prelude::*;
use rl_core::Direction;

use crate::cursor::{CursorKeys, shifted};
use crate::game_menu::MenuKeys;
use crate::keys::DirectionKeys;
use crate::panel::ability::AbilityKeys;
use crate::panel::inventory::InventoryKeys;
use crate::panel::scrollback::ScrollbackKeys;
use crate::panel::sheet::SheetKeys;

/// One key, with Shift held or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    /// The key.
    pub key: KeyCode,
    /// Whether Shift is held with it.
    pub shift: bool,
}

impl Chord {
    /// The key alone.
    pub const fn key(key: KeyCode) -> Self {
        Self { key, shift: false }
    }

    /// The key with Shift.
    pub const fn shift(key: KeyCode) -> Self {
        Self { key, shift: true }
    }

    /// Whether this went down this frame: the key, with Shift held exactly
    /// when the chord says so.
    pub fn just_pressed(&self, input: &ButtonInput<KeyCode>) -> bool {
        input.just_pressed(self.key) && shifted(input) == self.shift
    }

    /// Whether this is held down, Shift matched the same way.
    pub fn pressed(&self, input: &ButtonInput<KeyCode>) -> bool {
        input.pressed(self.key) && shifted(input) == self.shift
    }

    /// The chord as a player reads it: `g`, `G` for Shift and g, `?` for
    /// Shift and slash, `shift+tab` where Shift makes no character.
    pub fn label(&self) -> String {
        if !self.shift {
            return key_name(self.key);
        }
        if let Some(upper) = letter(self.key) {
            return upper.to_ascii_uppercase().to_string();
        }
        match shifted_symbol(self.key) {
            Some(symbol) => symbol.to_string(),
            None => format!("shift+{}", key_name(self.key)),
        }
    }
}

impl From<KeyCode> for Chord {
    fn from(key: KeyCode) -> Self {
        Chord::key(key)
    }
}

/// The name of a key as a player reads it on a keycap, short enough for a
/// column: letters and digits as themselves, arrows as arrows, `num5` for
/// the numpad and `pgup` for Page Up.
pub fn key_name(key: KeyCode) -> String {
    use KeyCode::*;
    if let Some(c) = letter(key).or_else(|| digit(key)) {
        return c.to_string();
    }
    if let Some(n) = NUMPAD.iter().position(|k| *k == key) {
        return format!("num{n}");
    }
    if let Some(n) = FUNCTION.iter().position(|k| *k == key) {
        return format!("f{}", n + 1);
    }
    let named = match key {
        ArrowUp => "\u{2191}",
        ArrowDown => "\u{2193}",
        ArrowLeft => "\u{2190}",
        ArrowRight => "\u{2192}",
        Enter | NumpadEnter => "enter",
        Escape => "esc",
        Tab => "tab",
        Space => "space",
        Backspace => "bksp",
        Delete => "del",
        Insert => "ins",
        Home => "home",
        End => "end",
        PageUp => "pgup",
        PageDown => "pgdn",
        Period => ".",
        Comma => ",",
        Slash => "/",
        Semicolon => ";",
        Quote => "'",
        Minus => "-",
        Equal => "=",
        BracketLeft => "[",
        BracketRight => "]",
        Backslash => "\\",
        Backquote => "`",
        other => return format!("{other:?}").to_lowercase(),
    };
    named.to_string()
}

const LETTERS: [KeyCode; 26] = {
    use KeyCode::*;
    [KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ]
};
const DIGITS: [KeyCode; 10] = {
    use KeyCode::*;
    [Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9]
};
const NUMPAD: [KeyCode; 10] = {
    use KeyCode::*;
    [Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9]
};
const FUNCTION: [KeyCode; 12] = {
    use KeyCode::*;
    [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12]
};

/// The lowercase letter a key types, if it types one.
fn letter(key: KeyCode) -> Option<char> {
    LETTERS.iter().position(|k| *k == key).map(|i| (b'a' + i as u8) as char)
}

/// The digit a key along the top row types, if it types one.
fn digit(key: KeyCode) -> Option<char> {
    DIGITS.iter().position(|k| *k == key).map(|i| (b'0' + i as u8) as char)
}

/// What Shift makes of a punctuation or digit key on the US layout, the
/// one the engine's names are written for. A key code is a physical key,
/// so `@` is the key that types `@` there and whatever it types elsewhere;
/// `?` and `>` already read that way, and `@` is what a player expects to
/// read beside a character sheet.
fn shifted_symbol(key: KeyCode) -> Option<char> {
    use KeyCode::*;
    Some(match key {
        Digit1 => '!',
        Digit2 => '@',
        Digit3 => '#',
        Digit4 => '$',
        Digit5 => '%',
        Digit6 => '^',
        Digit7 => '&',
        Digit8 => '*',
        Digit9 => '(',
        Digit0 => ')',
        Period => '>',
        Comma => '<',
        Slash => '?',
        Semicolon => ':',
        Quote => '"',
        Minus => '_',
        Equal => '+',
        BracketLeft => '{',
        BracketRight => '}',
        Backslash => '|',
        Backquote => '~',
        _ => return None,
    })
}

/// A key the engine reads from one of its own binding resources.
///
/// Closed, because it enumerates the bindings the engine owns; a game's
/// keys are [`Keys::Chords`] and never need a variant here.
///
/// Every one of them is a key that works while the world is in front of
/// the player: a cursor, or the key that opens a screen. A screen's own
/// keys are not here, because they are read only while that screen is the
/// top one and are written along its own bottom border.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKey {
    /// Opening the look cursor, from [`CursorKeys::look`].
    Look,
    /// The next thing in sight, from [`CursorKeys::next`].
    Next,
    /// The one before, which is Shift with [`CursorKeys::next`].
    Previous,
    /// Acting on a cursor, from [`CursorKeys::confirm`] and `also_confirm`.
    Confirm,
    /// Putting a cursor away, from [`CursorKeys::close`].
    Close,
    /// Opening the whole log, from [`ScrollbackKeys::toggle`].
    OpenLog,
    /// Opening this list, from [`ControlsKeys::toggle`].
    ShowControls,
    /// Opening the character sheet, from [`SheetKeys::toggle`].
    OpenSheet,
    /// Opening the ability menu, from [`AbilityKeys::toggle`].
    ListAbilities,
    /// Opening the bag, from [`InventoryKeys::toggle`].
    OpenInventory,
    /// Opening the menu, from [`MenuKeys::toggle`].
    OpenMenu,
}

/// What asks for a control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keys {
    /// Any of these, in order: a slot row of `1` to `4` is one control, and
    /// [`ControlInput::which`] says which of them went down.
    Chords(Vec<Chord>),
    /// Every key [`DirectionKeys`] binds, with Shift held or not: a shove, a
    /// run, a throw in a direction. [`ControlInput::direction`] says where.
    Directions {
        /// Whether Shift is held with the direction.
        shift: bool,
    },
    /// One of the engine's own bindings, read from its resource.
    Engine(EngineKey),
}

impl From<KeyCode> for Keys {
    fn from(key: KeyCode) -> Self {
        Keys::Chords(vec![Chord::key(key)])
    }
}

impl From<Chord> for Keys {
    fn from(chord: Chord) -> Self {
        Keys::Chords(vec![chord])
    }
}

impl<const N: usize> From<[KeyCode; N]> for Keys {
    fn from(keys: [KeyCode; N]) -> Self {
        Keys::Chords(keys.into_iter().map(Chord::key).collect())
    }
}

impl<const N: usize> From<[Chord; N]> for Keys {
    fn from(chords: [Chord; N]) -> Self {
        Keys::Chords(chords.to_vec())
    }
}

impl From<Vec<Chord>> for Keys {
    fn from(chords: Vec<Chord>) -> Self {
        Keys::Chords(chords)
    }
}

impl From<EngineKey> for Keys {
    fn from(key: EngineKey) -> Self {
        Keys::Engine(key)
    }
}

/// A control a game or the engine declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ControlId(u16);

/// One declared control: where it is listed, what it does, and what asks
/// for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    /// The heading it is listed under.
    pub group: String,
    /// What it does, in the game's words.
    pub action: String,
    /// What asks for it.
    pub keys: Keys,
}

/// The keys the controls screen answers to.
#[derive(Resource, Debug, Clone)]
pub struct ControlsKeys {
    /// Opens and closes the screen, and is what the hint names.
    pub toggle: Chord,
    /// Closes it.
    pub close: KeyCode,
    /// The page before, when the controls run to more than one.
    pub prev_page: KeyCode,
    /// The page after.
    pub next_page: KeyCode,
}

impl Default for ControlsKeys {
    fn default() -> Self {
        Self { toggle: Chord::shift(KeyCode::Slash), close: KeyCode::Escape, prev_page: KeyCode::ArrowLeft, next_page: KeyCode::ArrowRight }
    }
}

/// The binding resources an engine key is read from, borrowed together.
///
/// The scrollback's and the sheet's are optional because a game without
/// that panel has none, and a control naming them then lists no keys.
#[derive(Debug, Clone, Copy)]
pub struct Bindings<'a> {
    /// The eight directions.
    pub directions: &'a DirectionKeys,
    /// The cursors'.
    pub cursor: &'a CursorKeys,
    /// The controls screen's.
    pub help: &'a ControlsKeys,
    /// The scrollback's, when there is one.
    pub log: Option<&'a ScrollbackKeys>,
    /// The character sheet's, when there is one.
    pub sheet: Option<&'a SheetKeys>,
    /// The ability menu's, when there is one.
    pub abilities: Option<&'a AbilityKeys>,
    /// The bag's, when there is one.
    pub inventory: Option<&'a InventoryKeys>,
    /// The menu's, when there is one.
    pub menu: Option<&'a MenuKeys>,
}

impl Bindings<'_> {
    /// Every chord `keys` answers to, in order.
    pub fn chords(&self, keys: &Keys) -> Vec<Chord> {
        match keys {
            Keys::Chords(chords) => chords.clone(),
            Keys::Directions { shift } => self.directions.0.iter().map(|(key, _)| Chord { key: *key, shift: *shift }).collect(),
            Keys::Engine(engine) => self.engine(*engine),
        }
    }

    fn engine(&self, key: EngineKey) -> Vec<Chord> {
        let cursor = self.cursor;
        match key {
            EngineKey::Look => vec![cursor.look.into()],
            EngineKey::Next => vec![cursor.next.into()],
            EngineKey::Previous => vec![Chord::shift(cursor.next)],
            EngineKey::Confirm => vec![cursor.confirm.into(), cursor.also_confirm.into()],
            EngineKey::Close => vec![cursor.close.into()],
            EngineKey::OpenLog => self.log.map(|log| vec![log.toggle.into()]).unwrap_or_default(),
            EngineKey::ShowControls => vec![self.help.toggle],
            EngineKey::OpenSheet => self.sheet.map(|sheet| vec![sheet.toggle]).unwrap_or_default(),
            EngineKey::ListAbilities => self.abilities.map(|menu| vec![menu.toggle]).unwrap_or_default(),
            EngineKey::OpenInventory => self.inventory.map(|bag| vec![bag.toggle]).unwrap_or_default(),
            EngineKey::OpenMenu => self.menu.map(|menu| vec![menu.toggle.into()]).unwrap_or_default(),
        }
    }

    /// `keys` as a player reads them: each chord once, a run of three or
    /// more digits as a range, and the direction keys as the families they
    /// come in, `arrows hjklyubn numpad`, so a walk is one row rather
    /// than eight.
    pub fn label(&self, keys: &Keys) -> String {
        if let Keys::Directions { shift } = keys {
            return direction_families(self.directions, *shift);
        }
        let mut chords: Vec<Chord> = Vec::new();
        for chord in self.chords(keys) {
            if !chords.contains(&chord) {
                chords.push(chord);
            }
        }
        let digits: Option<Vec<u32>> = chords.iter().map(|c| if c.shift { None } else { digit(c.key).and_then(|d| d.to_digit(10)) }).collect();
        if let Some(run) = digits
            && run.len() > 2
            && run.windows(2).all(|w| w[1] == w[0] + 1)
        {
            return format!("{}-{}", run[0], run[run.len() - 1]);
        }
        chords.iter().map(Chord::label).collect::<Vec<_>>().join(" ")
    }
}

/// The direction keys as families: the arrows, the letters in the order
/// every roguelike lists them, the numpad, and anything else on its own.
/// Shift is written once per family, and on the letters as capitals.
fn direction_families(directions: &DirectionKeys, shift: bool) -> String {
    const CUSTOMARY: &str = "hjklyubn";
    let (mut arrows, mut numpad) = (false, false);
    let mut letters: Vec<char> = Vec::new();
    let mut others: Vec<String> = Vec::new();
    for (key, _) in &directions.0 {
        if ARROWS.contains(key) {
            arrows = true;
        } else if NUMPAD.contains(key) {
            numpad = true;
        } else if let Some(c) = letter(*key) {
            if !letters.contains(&c) {
                letters.push(c);
            }
        } else {
            let label = Chord { key: *key, shift }.label();
            if !others.contains(&label) {
                others.push(label);
            }
        }
    }
    letters.sort_by_key(|c| (CUSTOMARY.find(*c).unwrap_or(usize::MAX), *c));
    let family = |name: &str| if shift { format!("shift+{name}") } else { name.to_string() };
    let mut parts = Vec::new();
    if arrows {
        parts.push(family("arrows"));
    }
    if !letters.is_empty() {
        let run: String = letters.into_iter().collect();
        parts.push(if shift { run.to_uppercase() } else { run });
    }
    if numpad {
        parts.push(family("numpad"));
    }
    parts.extend(others);
    parts.join(" ")
}

const ARROWS: [KeyCode; 4] = [KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight];

/// Every control declared, in the order it was declared.
///
/// The order is the order the controls screen lists them in, a group
/// placed where its first control was declared, so a game that declares
/// its movement before its menus reads that way on screen.
#[derive(Resource, Debug, Clone, Default)]
pub struct Controls {
    controls: Vec<Control>,
}

impl Controls {
    /// Declares a control and returns its id.
    ///
    /// The same control declared twice, the same words over the same keys,
    /// is one control with one id, so two plugins that read the same cursor
    /// key list it once.
    pub fn add(&mut self, group: impl Into<String>, action: impl Into<String>, keys: impl Into<Keys>) -> ControlId {
        let control = Control { group: group.into(), action: action.into(), keys: keys.into() };
        if let Some(i) = self.controls.iter().position(|c| *c == control) {
            return ControlId(i as u16);
        }
        self.controls.push(control);
        ControlId((self.controls.len() - 1) as u16)
    }

    /// The control `id` names.
    ///
    /// # Panics
    /// Panics on an id from another registry.
    pub fn get(&self, id: ControlId) -> &Control {
        &self.controls[id.0 as usize]
    }

    /// The control listed under `group` as `action`, for a game rewording
    /// one the engine declared.
    pub fn find(&self, group: &str, action: &str) -> Option<ControlId> {
        self.controls.iter().position(|c| c.group == group && c.action == action).map(|i| ControlId(i as u16))
    }

    /// Rewords what `id` does.
    pub fn rename(&mut self, id: ControlId, action: impl Into<String>) {
        self.controls[id.0 as usize].action = action.into();
    }

    /// Lists `id` under another heading.
    pub fn regroup(&mut self, id: ControlId, group: impl Into<String>) {
        self.controls[id.0 as usize].group = group.into();
    }

    /// Every control, in declaration order.
    pub fn iter(&self) -> impl Iterator<Item = (ControlId, &Control)> {
        self.controls.iter().enumerate().map(|(i, c)| (ControlId(i as u16), c))
    }

    /// Which of `id`'s chords went down this frame, by its place in the
    /// list, or `None`. For [`Keys::Directions`], the place of the binding
    /// in [`DirectionKeys`].
    pub fn which(&self, id: ControlId, input: &ButtonInput<KeyCode>, bindings: &Bindings) -> Option<usize> {
        bindings.chords(&self.get(id).keys).iter().position(|chord| chord.just_pressed(input))
    }

    /// Whether any of `id`'s chords is held down.
    pub fn pressed(&self, id: ControlId, input: &ButtonInput<KeyCode>, bindings: &Bindings) -> bool {
        bindings.chords(&self.get(id).keys).iter().any(|chord| chord.pressed(input))
    }

    /// The direction `id` asked for this frame, for a control over
    /// [`Keys::Directions`]: the direction of the first of its keys that
    /// went down. `None` for any other control.
    pub fn direction(&self, id: ControlId, input: &ButtonInput<KeyCode>, bindings: &Bindings) -> Option<Direction> {
        let Keys::Directions { shift } = self.get(id).keys else { return None };
        bindings.directions.0.iter().find(|(key, _)| Chord { key: *key, shift }.just_pressed(input)).map(|(_, d)| *d)
    }

    /// The direction `id` is holding down, for a game that repeats a walk
    /// while the key is held.
    pub fn direction_held(&self, id: ControlId, input: &ButtonInput<KeyCode>, bindings: &Bindings) -> Option<Direction> {
        let Keys::Directions { shift } = self.get(id).keys else { return None };
        bindings.directions.0.iter().find(|(key, _)| Chord { key: *key, shift }.pressed(input)).map(|(_, d)| *d)
    }

    /// `id`'s keys as a player reads them. Empty when nothing binds it.
    pub fn label(&self, id: ControlId, bindings: &Bindings) -> String {
        bindings.label(&self.get(id).keys)
    }
}

/// Declaring controls while the app is built.
pub trait AddControls {
    /// Declares a control under `group` and returns its id. See
    /// [`Controls::add`].
    fn add_control(&mut self, group: &str, action: &str, keys: impl Into<Keys>) -> ControlId;
}

impl AddControls for App {
    fn add_control(&mut self, group: &str, action: &str, keys: impl Into<Keys>) -> ControlId {
        self.init_resource::<Controls>();
        self.world_mut().resource_mut::<Controls>().add(group, action, keys)
    }
}

/// This frame's keys, read through the registry.
///
/// What a game's input system takes in place of `ButtonInput<KeyCode>`.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ControlInput<'w> {
    input: Res<'w, ButtonInput<KeyCode>>,
    controls: Res<'w, Controls>,
    directions: Res<'w, DirectionKeys>,
    cursor: Res<'w, CursorKeys>,
    help: Res<'w, ControlsKeys>,
    log: Option<Res<'w, ScrollbackKeys>>,
    sheet: Option<Res<'w, SheetKeys>>,
    abilities: Option<Res<'w, AbilityKeys>>,
    inventory: Option<Res<'w, InventoryKeys>>,
    menu: Option<Res<'w, MenuKeys>>,
    repeats: Res<'w, Repeats>,
}

impl ControlInput<'_> {
    /// The binding resources, borrowed together.
    pub fn bindings(&self) -> Bindings<'_> {
        Bindings {
            directions: &self.directions,
            cursor: &self.cursor,
            help: &self.help,
            log: self.log.as_deref(),
            sheet: self.sheet.as_deref(),
            abilities: self.abilities.as_deref(),
            inventory: self.inventory.as_deref(),
            menu: self.menu.as_deref(),
        }
    }

    /// The registry.
    pub fn controls(&self) -> &Controls {
        &self.controls
    }

    /// Whether `id` went down this frame.
    pub fn just_pressed(&self, id: ControlId) -> bool {
        self.which(id).is_some()
    }

    /// Which of `id`'s keys went down this frame. See [`Controls::which`].
    pub fn which(&self, id: ControlId) -> Option<usize> {
        self.controls.which(id, &self.input, &self.bindings())
    }

    /// Whether `id` is held down.
    pub fn pressed(&self, id: ControlId) -> bool {
        self.controls.pressed(id, &self.input, &self.bindings())
    }

    /// The direction `id` asked for this frame: a key just pressed, or a
    /// key held long enough to repeat, at [`RepeatPace`]. See
    /// [`Controls::direction`] and [`Repeats`].
    pub fn direction(&self, id: ControlId) -> Option<Direction> {
        self.controls.direction(id, &self.input, &self.bindings()).or_else(|| {
            let Keys::Directions { shift } = self.controls.get(id).keys else { return None };
            self.repeats.firing(shift)
        })
    }

    /// The direction `id` is holding down. See
    /// [`Controls::direction_held`].
    pub fn direction_held(&self, id: ControlId) -> Option<Direction> {
        self.controls.direction_held(id, &self.input, &self.bindings())
    }

    /// `id`'s keys as a player reads them.
    pub fn label(&self, id: ControlId) -> String {
        self.controls.label(id, &self.bindings())
    }

    /// The raw keys, for the rare reading the registry has no word for.
    pub fn input(&self) -> &ButtonInput<KeyCode> {
        &self.input
    }
}

/// How a held direction key repeats: the wait before the first repeat,
/// and the wait between repeats, in seconds.
///
/// One resource for every game, since a player who holds a key expects
/// the same walk in each.
#[derive(Resource, Debug, Clone, Copy)]
pub struct RepeatPace {
    /// Seconds a key is held before it repeats.
    pub delay: f32,
    /// Seconds between repeats after that.
    pub every: f32,
}

impl Default for RepeatPace {
    fn default() -> Self {
        Self { delay: 0.25, every: 0.08 }
    }
}

/// What the held direction key is doing this frame.
///
/// A held key repeats its direction at [`RepeatPace`], and every
/// direction control reads the repeat through [`ControlInput::direction`]
/// as if the key had been pressed again. That is what a player holding a
/// key means in every roguelike, so it is the engine's to do and no
/// game's to remember: a walk, a cursor and a shove all repeat, and each
/// game's input system reads `direction` exactly as before.
///
/// One state, not one per control: the direction keys are one physical
/// set, and a player holds one of them at a time.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct Repeats {
    /// Seconds the current key has been down.
    held_for: f32,
    /// Seconds since its last repeat.
    since_last: f32,
    /// The direction down this frame and whether Shift is with it, if the
    /// hold is long enough to be repeating this frame.
    firing: Option<(Direction, bool)>,
}

impl Repeats {
    /// Moves the hold on by `delta` seconds, where `held` is the direction
    /// key down this frame and whether Shift is with it, and `fresh`
    /// whether that key went down this frame. A fresh press restarts the
    /// hold and fires nothing, since the press itself is read as a press.
    pub fn advance(&mut self, held: Option<(Direction, bool)>, fresh: bool, pace: &RepeatPace, delta: f32) {
        self.firing = None;
        let Some(held) = held else {
            *self = Self::default();
            return;
        };
        if fresh {
            *self = Self::default();
            return;
        }
        self.held_for += delta;
        self.since_last += delta;
        if self.held_for >= pace.delay && self.since_last >= pace.every {
            self.since_last = 0.0;
            self.firing = Some(held);
        }
    }

    /// The direction repeating this frame for a control whose chords use
    /// Shift as `shift` says.
    pub fn firing(&self, shift: bool) -> Option<Direction> {
        self.firing.filter(|(_, with_shift)| *with_shift == shift).map(|(d, _)| d)
    }

    /// The direction repeating this frame, and whether Shift is with it,
    /// for whoever writes it down.
    pub fn firing_any(&self) -> Option<(Direction, bool)> {
        self.firing
    }
}

/// Moves [`Repeats`] on by this frame. Runs before any input is read.
pub fn advance_repeats(input: Res<ButtonInput<KeyCode>>, directions: Res<DirectionKeys>, pace: Res<RepeatPace>, time: Res<Time>, mut repeats: ResMut<Repeats>) {
    let held = directions.0.iter().find(|(key, _)| input.pressed(*key));
    let fresh = held.is_some_and(|(key, _)| input.just_pressed(*key));
    let held = held.map(|(_, dir)| (*dir, shifted(&input)));
    repeats.advance(held, fresh, &pace, time.delta_secs());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A held key says nothing until the delay is up, then repeats at the
    /// pace, and a fresh press or a release starts over.
    #[test]
    fn a_held_direction_repeats_after_the_delay_and_a_release_starts_over() {
        let pace = RepeatPace { delay: 0.25, every: 0.1 };
        let mut repeats = Repeats::default();
        let west = Some((Direction::West, false));
        repeats.advance(west, true, &pace, 0.016);
        assert_eq!(repeats.firing(false), None, "the press is read as a press");
        let mut fired = vec![];
        for frame in 1..=40 {
            repeats.advance(west, false, &pace, 0.016);
            if repeats.firing(false).is_some() {
                fired.push(frame);
            }
        }
        assert_eq!(fired, vec![16, 23, 30, 37], "once past the delay, then every tenth of a second");
        assert_eq!(repeats.firing(true), None, "a shifted control does not read an unshifted hold");
        repeats.advance(None, false, &pace, 0.016);
        repeats.advance(west, false, &pace, 0.016);
        assert_eq!(repeats.firing(false), None, "released and pressed again, the delay is owed again");
        repeats.advance(Some((Direction::East, true)), false, &pace, 0.3);
        assert_eq!(repeats.firing(true), Some(Direction::East), "and a shifted hold is read by a shifted control");
    }

    fn input(keys: &[KeyCode]) -> ButtonInput<KeyCode> {
        let mut input = ButtonInput::default();
        for key in keys {
            input.press(*key);
        }
        input
    }

    /// Default bindings, borrowed for one assertion.
    fn with_defaults<R>(log: bool, f: impl FnOnce(&Bindings) -> R) -> R {
        let (directions, cursor, help, scrollback) = (DirectionKeys::default(), CursorKeys::default(), ControlsKeys::default(), ScrollbackKeys::default());
        f(&Bindings {
            directions: &directions,
            cursor: &cursor,
            help: &help,
            log: log.then_some(&scrollback),
            sheet: None,
            abilities: None,
            inventory: None,
            menu: None,
        })
    }

    #[test]
    fn a_chord_is_named_the_way_a_player_reads_a_keycap() {
        assert_eq!(Chord::key(KeyCode::KeyG).label(), "g");
        assert_eq!(Chord::shift(KeyCode::KeyL).label(), "L");
        assert_eq!(Chord::shift(KeyCode::Slash).label(), "?");
        assert_eq!(Chord::shift(KeyCode::Period).label(), ">");
        assert_eq!(Chord::key(KeyCode::Period).label(), ".");
        assert_eq!(Chord::shift(KeyCode::Tab).label(), "shift+tab");
        assert_eq!(Chord::shift(KeyCode::Digit2).label(), "@");
        assert_eq!(Chord::key(KeyCode::Numpad5).label(), "num5");
        assert_eq!(Chord::key(KeyCode::ArrowUp).label(), "\u{2191}");
        assert_eq!(Chord::key(KeyCode::Escape).label(), "esc");
        assert_eq!(Chord::key(KeyCode::F3).label(), "f3");
    }

    #[test]
    fn shift_is_matched_exactly_so_a_letter_and_its_capital_are_two_controls() {
        let (plain, capital) = (Chord::key(KeyCode::KeyL), Chord::shift(KeyCode::KeyL));
        let bare = input(&[KeyCode::KeyL]);
        assert!(plain.just_pressed(&bare) && !capital.just_pressed(&bare));
        let held = input(&[KeyCode::ShiftRight, KeyCode::KeyL]);
        assert!(capital.just_pressed(&held) && !plain.just_pressed(&held), "Shift makes it the other control");
    }

    #[test]
    fn a_slot_row_says_which_slot_and_reads_as_a_range() {
        let mut controls = Controls::default();
        let slots = controls.add("Abilities", "call on", [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4]);
        with_defaults(false, |b| {
            assert_eq!(controls.which(slots, &input(&[KeyCode::Digit3]), b), Some(2));
            assert_eq!(controls.which(slots, &input(&[KeyCode::Digit5]), b), None);
            assert_eq!(controls.label(slots, b), "1-4");
        });
        let two = controls.add("Abilities", "the first two", [KeyCode::Digit1, KeyCode::Digit2]);
        with_defaults(false, |b| assert_eq!(controls.label(two, b), "1 2", "two keys are not a range"));
    }

    #[test]
    fn a_direction_control_says_where_and_minds_its_shift() {
        let mut controls = Controls::default();
        let shove = controls.add("Fight", "shove", Keys::Directions { shift: true });
        let walk = controls.add("Move", "walk", Keys::Directions { shift: false });
        with_defaults(false, |b| {
            assert_eq!(controls.direction(shove, &input(&[KeyCode::ShiftLeft, KeyCode::KeyK]), b), Some(Direction::North));
            assert_eq!(controls.direction(shove, &input(&[KeyCode::KeyK]), b), None, "no Shift, no shove");
            assert_eq!(controls.direction(walk, &input(&[KeyCode::Numpad8]), b), Some(Direction::North));
            assert_eq!(controls.direction_held(walk, &input(&[KeyCode::Numpad8]), b), Some(Direction::North));
            assert_eq!(controls.label(shove, b), "shift+arrows HJKLYUBN shift+numpad");
            assert_eq!(controls.label(walk, b), "arrows hjklyubn numpad");
        });
        let wasd = DirectionKeys::none().bind(KeyCode::KeyW, Direction::North).bind(KeyCode::KeyS, Direction::South).bind(KeyCode::Space, Direction::East);
        let (cursor, help) = (CursorKeys::default(), ControlsKeys::default());
        let bindings = Bindings { directions: &wasd, cursor: &cursor, help: &help, log: None, sheet: None, abilities: None, inventory: None, menu: None };
        assert_eq!(controls.label(walk, &bindings), "sw space", "letters outside the custom sort after it, and any other key on its own");
    }

    #[test]
    fn an_engine_control_lists_whatever_its_resource_binds_now_and_is_declared_once() {
        let mut controls = Controls::default();
        let next = controls.add("Look and aim", "next in sight", EngineKey::Next);
        assert_eq!(controls.add("Look and aim", "next in sight", EngineKey::Next), next, "a second plugin declaring the same thing lists it once");
        assert_ne!(controls.add("Elsewhere", "other words", EngineKey::Next), next, "other words are another row");
        let log = controls.add("Log", "open the log", EngineKey::OpenLog);
        let (directions, help, scrollback) = (DirectionKeys::default(), ControlsKeys::default(), ScrollbackKeys::default());
        let cursor = CursorKeys { next: KeyCode::KeyN, ..CursorKeys::default() };
        let bindings = Bindings { directions: &directions, cursor: &cursor, help: &help, log: None, sheet: None, abilities: None, inventory: None, menu: None };
        assert_eq!(controls.label(next, &bindings), "n", "rebound, and listed as rebound");
        assert_eq!(controls.label(log, &bindings), "", "no scrollback, nothing to list");
        assert_eq!(controls.label(log, &Bindings { log: Some(&scrollback), ..bindings }), "p");
    }

    #[test]
    fn a_control_the_engine_declared_can_be_found_and_reworded() {
        let mut controls = Controls::default();
        let look = controls.add("Look and aim", "look around", EngineKey::Look);
        assert_eq!(controls.find("Look and aim", "look around"), Some(look));
        controls.rename(look, "examine");
        controls.regroup(look, "Eyes");
        assert_eq!(controls.get(look), &Control { group: "Eyes".into(), action: "examine".into(), keys: Keys::Engine(EngineKey::Look) });
        assert_eq!(controls.find("Look and aim", "look around"), None);
    }
}
