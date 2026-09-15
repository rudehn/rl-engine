//! A game photographing its own window, for the README and for looking at
//! a change end to end without anyone at the keyboard.
//!
//! With `RL_CAPTURE=shot.png` in the environment, [`CapturePlugin`] can
//! first play `RL_CAPTURE_KEYS` through the real keyboard input, one key
//! every few frames, then waits for the turns to settle, saves the window
//! to that path, and exits. `RL_CAPTURE_FRAMES` sets the earliest frame to
//! shoot on, 60 by default. The shot also waits for a few seconds of wall
//! time, because a hidden window has no vsync to pace it and would reach
//! any frame count before the renderer has compiled its shaders. Without
//! `RL_CAPTURE` the plugin does nothing.
//!
//! Keys are separated by spaces. A letter is its key and a capital letter
//! is that key with shift; `.` `,` `>` `<` `/` `?` are what they look like;
//! `up`, `down`, `left`, `right`, `enter`, `esc`, `tab` and `space` name
//! the rest; `x*4` repeats a key. So `RL_CAPTURE_KEYS="l*5 L ."` walks east
//! five times, presses shift and L, then waits.
//!
//! Only the game's own frame is read, never the screen. [`prepare`] opens a
//! capture's window above the others without taking focus, so a capture
//! never takes the keyboard from whoever is at the machine. The window
//! does have to be shown and the screen unlocked: macOS draws nothing for
//! a hidden window or a locked session, and a camera drawing into an
//! offscreen image came back black, so both were tried and neither is used.
//! A frame that comes back entirely black is refused with an error rather
//! than saved, since it can only mean nothing was drawn.

use std::path::PathBuf;

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk};
use bevy::window::WindowLevel;

/// The environment variable naming where to save.
pub const CAPTURE_VAR: &str = "RL_CAPTURE";
/// The environment variable naming the earliest frame to shoot on.
pub const FRAMES_VAR: &str = "RL_CAPTURE_FRAMES";
/// The environment variable naming keys to press first.
pub const KEYS_VAR: &str = "RL_CAPTURE_KEYS";

/// The frame the first scripted key is pressed on: the world is built by then.
const FIRST_KEY: u32 = 30;
/// Frames between scripted keys.
const KEY_GAP: u32 = 3;
/// Frames to wait after the last key before shooting.
const SETTLE: u32 = 20;
/// Wall time before the shot, for the renderer to have everything compiled.
const WARM_UP_SECS: f32 = 4.0;

/// Saves the window and exits, when [`CAPTURE_VAR`] asks.
pub struct CapturePlugin;

/// Whether this run is a capture.
pub fn requested() -> bool {
    std::env::var_os(CAPTURE_VAR).is_some()
}

/// `window` as a capture needs it: above the others so it is drawn, and
/// unfocused so it takes no keys. Unchanged when this run is not a capture.
pub fn prepare(mut window: Window) -> Window {
    if requested() {
        window.focused = false;
        window.window_level = WindowLevel::AlwaysOnTop;
    }
    window
}

#[derive(Resource)]
struct Capture {
    path: PathBuf,
    keys: Vec<Vec<KeyCode>>,
    shoot_on: u32,
    frame: u32,
    held: Vec<KeyCode>,
    shot: bool,
}

impl Plugin for CapturePlugin {
    fn build(&self, app: &mut App) {
        let Some(path) = std::env::var_os(CAPTURE_VAR) else { return };
        let keys = match std::env::var(KEYS_VAR) {
            Ok(script) => parse_keys(&script).unwrap_or_else(|e| panic!("{KEYS_VAR}: {e}")),
            Err(_) => Vec::new(),
        };
        let earliest = std::env::var(FRAMES_VAR).ok().and_then(|v| v.parse().ok()).unwrap_or(60);
        let shoot_on = earliest.max(FIRST_KEY + KEY_GAP * keys.len() as u32 + SETTLE);
        app.insert_resource(Capture { path: path.into(), keys, shoot_on, frame: 0, held: Vec::new(), shot: false })
            .add_systems(PreUpdate, press_keys.after(InputSystems))
            .add_systems(Last, shoot);
    }
}

/// Parses a key script; see the module docs.
pub fn parse_keys(script: &str) -> Result<Vec<Vec<KeyCode>>, String> {
    let mut out = Vec::new();
    for word in script.split_whitespace() {
        let (key, times) = match word.rsplit_once('*') {
            Some((k, n)) if !k.is_empty() => (k, n.parse::<usize>().map_err(|_| format!("bad repeat in {word:?}"))?),
            _ => (word, 1),
        };
        let chord = chord(key).ok_or_else(|| format!("unknown key {key:?}"))?;
        out.extend(std::iter::repeat_n(chord, times));
    }
    Ok(out)
}

fn chord(key: &str) -> Option<Vec<KeyCode>> {
    let named = match key {
        "up" => Some(KeyCode::ArrowUp),
        "down" => Some(KeyCode::ArrowDown),
        "left" => Some(KeyCode::ArrowLeft),
        "right" => Some(KeyCode::ArrowRight),
        "enter" => Some(KeyCode::Enter),
        "esc" => Some(KeyCode::Escape),
        "tab" => Some(KeyCode::Tab),
        "space" => Some(KeyCode::Space),
        _ => None,
    };
    if let Some(k) = named {
        return Some(vec![k]);
    }
    let mut chars = key.chars();
    let (c, None) = (chars.next()?, chars.next()) else { return None };
    let shift = KeyCode::ShiftLeft;
    Some(match c {
        '.' => vec![KeyCode::Period],
        ',' => vec![KeyCode::Comma],
        '/' => vec![KeyCode::Slash],
        '>' => vec![shift, KeyCode::Period],
        '<' => vec![shift, KeyCode::Comma],
        '?' => vec![shift, KeyCode::Slash],
        '0'..='9' => vec![DIGITS[c as usize - '0' as usize]],
        'a'..='z' => vec![LETTERS[c as usize - 'a' as usize]],
        'A'..='Z' => vec![shift, LETTERS[c as usize - 'A' as usize]],
        _ => return None,
    })
}

const LETTERS: [KeyCode; 26] = [
    KeyCode::KeyA,
    KeyCode::KeyB,
    KeyCode::KeyC,
    KeyCode::KeyD,
    KeyCode::KeyE,
    KeyCode::KeyF,
    KeyCode::KeyG,
    KeyCode::KeyH,
    KeyCode::KeyI,
    KeyCode::KeyJ,
    KeyCode::KeyK,
    KeyCode::KeyL,
    KeyCode::KeyM,
    KeyCode::KeyN,
    KeyCode::KeyO,
    KeyCode::KeyP,
    KeyCode::KeyQ,
    KeyCode::KeyR,
    KeyCode::KeyS,
    KeyCode::KeyT,
    KeyCode::KeyU,
    KeyCode::KeyV,
    KeyCode::KeyW,
    KeyCode::KeyX,
    KeyCode::KeyY,
    KeyCode::KeyZ,
];

const DIGITS: [KeyCode; 10] = [
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// Presses the next scripted key on its frame, through the same
/// `ButtonInput` the game reads, and lets go of it the frame after.
fn press_keys(mut state: ResMut<Capture>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    state.frame += 1;
    for k in std::mem::take(&mut state.held) {
        keys.release(k);
    }
    let Some(offset) = state.frame.checked_sub(FIRST_KEY) else { return };
    if offset % KEY_GAP != 0 {
        return;
    }
    let Some(chord) = state.keys.get((offset / KEY_GAP) as usize).cloned() else { return };
    for k in &chord {
        keys.press(*k);
    }
    state.held = chord;
}

fn shoot(mut commands: Commands, mut state: ResMut<Capture>, time: Res<Time<Real>>) {
    if state.shot || state.frame < state.shoot_on || time.elapsed_secs() < WARM_UP_SECS {
        return;
    }
    state.shot = true;
    let mut save = save_to_disk(state.path.clone());
    commands.spawn(Screenshot::primary_window()).observe(move |shot: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
        let drawn = shot.image.data.as_ref().is_some_and(|px| px.as_chunks::<4>().0.iter().any(|p| p[0] != 0 || p[1] != 0 || p[2] != 0));
        if drawn {
            save(shot);
            exit.write(AppExit::Success);
        } else {
            error!("the captured frame is entirely black: nothing was drawn. Is the screen locked or the window hidden?");
            exit.write(AppExit::error());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_reads_as_keys() {
        let keys = parse_keys("l*3 L . > up").unwrap();
        assert_eq!(keys.len(), 7);
        assert_eq!(keys[0], vec![KeyCode::KeyL]);
        assert_eq!(keys[3], vec![KeyCode::ShiftLeft, KeyCode::KeyL]);
        assert_eq!(keys[4], vec![KeyCode::Period]);
        assert_eq!(keys[5], vec![KeyCode::ShiftLeft, KeyCode::Period]);
        assert_eq!(keys[6], vec![KeyCode::ArrowUp]);
        assert!(parse_keys("ctrl").is_err());
        assert!(parse_keys("l*x").is_err());
    }
}
