//! A terminal's key presses as the keys a keyboard would press.
//!
//! A terminal reports what a key typed, `?` or `K`, where a window reports
//! which keys went down, Shift and the slash. The engine reads the second,
//! as `ButtonInput<KeyCode>`, so each press is turned back into its key
//! and whether Shift went with it, on a US layout, which is the one the
//! engine's own key names assume.

use bevy::input::keyboard::KeyCode;
use crossterm::event::{KeyCode as Typed, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

/// One key as a terminal reports it: the key a keyboard pressed, and
/// whether Shift was down with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keystroke {
    /// The key.
    pub key: KeyCode,
    /// Whether Shift was held: a capital, a shifted symbol such as `?`, or
    /// Shift with an arrow or Tab.
    pub shift: bool,
}

impl Keystroke {
    const fn plain(key: KeyCode) -> Self {
        Self { key, shift: false }
    }

    const fn shifted(key: KeyCode) -> Self {
        Self { key, shift: true }
    }
}

/// The keystroke `event` is, or `None` for a key the engine never binds,
/// a release, or a chord with Control or Alt, which a terminal reports
/// differently from one emulator to the next.
///
/// Letters, digits, the punctuation of a US keyboard, arrows, Enter,
/// Escape, Tab, Backspace, Space, Home, End, Page Up and Down, Insert,
/// Delete and F1 to F12. A key from the numpad is a numpad key only when
/// the terminal says so, which a terminal speaking the kitty keyboard
/// protocol does; any other terminal sends the numpad as the digits and
/// arrows it types, and they become those keys. The numpad's middle key
/// with Num Lock off is the numpad's five either way.
pub fn keystroke(event: &KeyEvent) -> Option<Keystroke> {
    if event.kind == KeyEventKind::Release || event.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) {
        return None;
    }
    let shift = event.modifiers.contains(KeyModifiers::SHIFT);
    let with_shift = |key| Keystroke { key, shift };
    if event.state.contains(KeyEventState::KEYPAD) {
        return numpad(event.code).map(Keystroke::plain);
    }
    Some(match event.code {
        Typed::Char(c) => typed(c)?,
        Typed::Up => with_shift(KeyCode::ArrowUp),
        Typed::Down => with_shift(KeyCode::ArrowDown),
        Typed::Left => with_shift(KeyCode::ArrowLeft),
        Typed::Right => with_shift(KeyCode::ArrowRight),
        Typed::Home => with_shift(KeyCode::Home),
        Typed::End => with_shift(KeyCode::End),
        Typed::PageUp => with_shift(KeyCode::PageUp),
        Typed::PageDown => with_shift(KeyCode::PageDown),
        Typed::Insert => with_shift(KeyCode::Insert),
        Typed::Delete => with_shift(KeyCode::Delete),
        Typed::Enter => with_shift(KeyCode::Enter),
        Typed::Esc => with_shift(KeyCode::Escape),
        Typed::Tab => with_shift(KeyCode::Tab),
        Typed::BackTab => Keystroke::shifted(KeyCode::Tab),
        Typed::Backspace => with_shift(KeyCode::Backspace),
        Typed::KeypadBegin => Keystroke::plain(KeyCode::Numpad5),
        Typed::F(n @ 1..=12) => with_shift(FUNCTION[usize::from(n - 1)]),
        _ => return None,
    })
}

/// Whether `event` is Control-C, which in raw mode reaches the game as a
/// key rather than as the signal that would have ended it.
pub fn is_interrupt(event: &KeyEvent) -> bool {
    event.kind != KeyEventKind::Release && event.modifiers.contains(KeyModifiers::CONTROL) && matches!(event.code, Typed::Char('c' | 'C'))
}

/// The numpad key a key reported as the numpad's is: its digits and
/// symbols, Enter, and its arrows and the rest with Num Lock off, which
/// are the directions printed on the digits under them.
fn numpad(code: Typed) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match code {
        Typed::Char(c @ '0'..='9') => NUMPAD[c as usize - '0' as usize],
        Typed::Char('.') => NumpadDecimal,
        Typed::Char('/') => NumpadDivide,
        Typed::Char('*') => NumpadMultiply,
        Typed::Char('-') => NumpadSubtract,
        Typed::Char('+') => NumpadAdd,
        Typed::Char('=') => NumpadEqual,
        Typed::Char(',') => NumpadComma,
        Typed::Enter => NumpadEnter,
        Typed::Up => Numpad8,
        Typed::Down => Numpad2,
        Typed::Left => Numpad4,
        Typed::Right => Numpad6,
        Typed::Home => Numpad7,
        Typed::PageUp => Numpad9,
        Typed::End => Numpad1,
        Typed::PageDown => Numpad3,
        Typed::KeypadBegin => Numpad5,
        Typed::Insert => Numpad0,
        Typed::Delete => NumpadDecimal,
        _ => return None,
    })
}

/// The key that types `c` on a US keyboard, and whether Shift goes with it.
fn typed(c: char) -> Option<Keystroke> {
    use KeyCode::*;
    let plain = |key| Some(Keystroke::plain(key));
    let shifted = |key| Some(Keystroke::shifted(key));
    match c {
        'a'..='z' => plain(LETTERS[c as usize - 'a' as usize]),
        'A'..='Z' => shifted(LETTERS[c as usize - 'A' as usize]),
        '0'..='9' => plain(DIGITS[c as usize - '0' as usize]),
        ' ' => plain(Space),
        '.' => plain(Period),
        ',' => plain(Comma),
        '/' => plain(Slash),
        ';' => plain(Semicolon),
        '\'' => plain(Quote),
        '-' => plain(Minus),
        '=' => plain(Equal),
        '[' => plain(BracketLeft),
        ']' => plain(BracketRight),
        '\\' => plain(Backslash),
        '`' => plain(Backquote),
        '!' => shifted(Digit1),
        '@' => shifted(Digit2),
        '#' => shifted(Digit3),
        '$' => shifted(Digit4),
        '%' => shifted(Digit5),
        '^' => shifted(Digit6),
        '&' => shifted(Digit7),
        '*' => shifted(Digit8),
        '(' => shifted(Digit9),
        ')' => shifted(Digit0),
        '>' => shifted(Period),
        '<' => shifted(Comma),
        '?' => shifted(Slash),
        ':' => shifted(Semicolon),
        '"' => shifted(Quote),
        '_' => shifted(Minus),
        '+' => shifted(Equal),
        '{' => shifted(BracketLeft),
        '}' => shifted(BracketRight),
        '|' => shifted(Backslash),
        '~' => shifted(Backquote),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn typed_as(code: Typed) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn on_the_numpad(code: Typed) -> KeyEvent {
        KeyEvent { state: KeyEventState::KEYPAD, ..typed_as(code) }
    }

    #[test]
    fn vi_letters_arrows_and_the_numpad_become_their_key_codes() {
        let cases = [
            (typed_as(Typed::Char('k')), Keystroke::plain(KeyCode::KeyK)),
            (typed_as(Typed::Char('y')), Keystroke::plain(KeyCode::KeyY)),
            (typed_as(Typed::Up), Keystroke::plain(KeyCode::ArrowUp)),
            (typed_as(Typed::Left), Keystroke::plain(KeyCode::ArrowLeft)),
            (on_the_numpad(Typed::Char('8')), Keystroke::plain(KeyCode::Numpad8)),
            (on_the_numpad(Typed::Home), Keystroke::plain(KeyCode::Numpad7)),
            (typed_as(Typed::KeypadBegin), Keystroke::plain(KeyCode::Numpad5)),
            (typed_as(Typed::Char('8')), Keystroke::plain(KeyCode::Digit8)),
            (typed_as(Typed::Char('.')), Keystroke::plain(KeyCode::Period)),
            (typed_as(Typed::Enter), Keystroke::plain(KeyCode::Enter)),
            (typed_as(Typed::Esc), Keystroke::plain(KeyCode::Escape)),
            (typed_as(Typed::Tab), Keystroke::plain(KeyCode::Tab)),
            (typed_as(Typed::Backspace), Keystroke::plain(KeyCode::Backspace)),
            (typed_as(Typed::F(11)), Keystroke::plain(KeyCode::F11)),
        ];
        for (event, stroke) in cases {
            assert_eq!(keystroke(&event), Some(stroke), "{event:?}");
        }
    }

    #[test]
    fn a_capital_or_a_shifted_symbol_is_its_key_with_shift() {
        let cases = [
            (typed_as(Typed::Char('K')), KeyCode::KeyK),
            (typed_as(Typed::Char('?')), KeyCode::Slash),
            (typed_as(Typed::Char('>')), KeyCode::Period),
            (typed_as(Typed::BackTab), KeyCode::Tab),
            (KeyEvent::new(Typed::Up, KeyModifiers::SHIFT), KeyCode::ArrowUp),
        ];
        for (event, key) in cases {
            assert_eq!(keystroke(&event), Some(Keystroke::shifted(key)), "{event:?}");
        }
    }

    #[test]
    fn a_key_the_engine_never_binds_becomes_nothing() {
        for event in [
            typed_as(Typed::Char('é')),
            typed_as(Typed::F(13)),
            typed_as(Typed::CapsLock),
            KeyEvent::new(Typed::Char('k'), KeyModifiers::CONTROL),
            KeyEvent::new(Typed::Char('k'), KeyModifiers::ALT),
            KeyEvent::new_with_kind(Typed::Char('k'), KeyModifiers::NONE, KeyEventKind::Release),
        ] {
            assert_eq!(keystroke(&event), None, "{event:?}");
        }
    }

    #[test]
    fn control_c_is_an_interrupt_and_c_alone_is_not() {
        assert!(is_interrupt(&KeyEvent::new(Typed::Char('c'), KeyModifiers::CONTROL)));
        assert!(!is_interrupt(&typed_as(Typed::Char('c'))));
    }
}
