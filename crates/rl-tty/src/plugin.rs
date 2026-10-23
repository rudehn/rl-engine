//! The plugin that shows the grid in a terminal and reads its keys.

use std::collections::{BTreeSet, VecDeque};
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bevy::input::InputSystems;
use bevy::prelude::*;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{Event, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
use crossterm::queue;
use crossterm::style::{Print, ResetColor};
use crossterm::terminal::{BeginSynchronizedUpdate, Clear, ClearType, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen};
use rl_render::{Cell, Terminal};

use crate::keys::{Keystroke, is_interrupt, keystroke};
use crate::paint::{Colors, changes, paint};

/// Shows the [`Terminal`] grid in a text terminal and reads the keys typed
/// into it, in place of the window's `TerminalPlugin`.
///
/// [`new`](Self::new) is the process's own terminal, put into raw mode and
/// the alternate screen at startup and handed back as it was when the app
/// ends, panics included, and polled for keys every frame.
/// [`to_writer`](Self::to_writer) is any other, an SSH channel or a test's
/// buffer: the grid is written to it, and its keys and size come in as
/// [`TtyInput`] messages from whoever holds the other end.
///
/// A terminal reports a key when it is typed and never when it is let go,
/// so a key is held for the frame it was typed in and released the next:
/// `just_pressed` fires once for each press, and a key held down walks as
/// the terminal repeats it. One key a frame, the rest waiting their turn,
/// since a chord's Shift is down for the frame its key is.
///
/// The grid keeps its size whatever the terminal's, drawn centred in a
/// larger one; a smaller one shows how large it needs to be until it is.
pub struct TtyPlugin {
    cols: i32,
    rows: i32,
    local: bool,
    colors: Colors,
    /// Taken once, when the plugin is built: a plugin is built from `&self`.
    out: Mutex<Option<Box<dyn Write + Send + Sync>>>,
}

impl TtyPlugin {
    /// A grid `cols` by `rows` cells in the terminal the game was started
    /// in, in as many colours as `COLORTERM` says it draws.
    pub fn new(cols: i32, rows: i32) -> Self {
        Self { cols, rows, local: true, colors: Colors::from_env(), out: Mutex::new(Some(Box::new(std::io::stdout()))) }
    }

    /// A grid `cols` by `rows` cells written to `out`, a terminal at the
    /// other end of an SSH channel or a test's buffer, in any colour. No
    /// raw mode and no polling: the terminal's keys and size are written
    /// as [`TtyInput`] messages by whatever reads that end.
    pub fn to_writer(cols: i32, rows: i32, out: impl Write + Send + Sync + 'static) -> Self {
        Self { cols, rows, local: false, colors: Colors::True, out: Mutex::new(Some(Box::new(out))) }
    }

    /// Draws in `colors` rather than what the plugin assumed.
    pub fn colors(mut self, colors: Colors) -> Self {
        self.colors = colors;
        self
    }
}

impl Plugin for TtyPlugin {
    fn build(&self, app: &mut App) {
        let out = self.out.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take().expect("a TtyPlugin is built once");
        if !app.is_plugin_added::<bevy::input::InputPlugin>() {
            app.add_plugins(bevy::input::InputPlugin);
        }
        app.insert_resource(Terminal::new(self.cols, self.rows, Vec2::ONE))
            .init_resource::<rl_render::pointer::Pointer>()
            .insert_resource(Screen::new(out, self.local, self.colors))
            .init_resource::<Typed>()
            .add_message::<TtyInput>()
            .add_systems(Startup, begin)
            // After Bevy's own input systems, which clear last frame's
            // presses at the top of the frame, so what is pressed here is
            // still pressed when the game reads it.
            .add_systems(PreUpdate, (release_last_key, read_input, press_next_key).chain().after(InputSystems))
            .add_systems(PostUpdate, draw);
        if self.local {
            app.add_systems(PreUpdate, poll_terminal.before(read_input).after(InputSystems));
        }
    }
}

/// Something the terminal said: a key typed, its new size, anything else
/// crossterm reports.
///
/// The plugin's own terminal writes these itself. A game serving the
/// grid [`to_writer`](TtyPlugin::to_writer) writes them for the terminal
/// at the other end: a key as the SSH client sends it, and a
/// `Event::Resize` when the client says its window changed.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct TtyInput(pub Event);

/// Whether the process's own terminal is in raw mode and the alternate
/// screen, for the panic hook, which cannot reach the app's resources, to
/// hand it back once and only once.
static LOCAL_TAKEN: AtomicBool = AtomicBool::new(false);
/// Whether the process's own terminal was asked for the kitty keyboard
/// protocol, which must be popped when it is handed back.
static LOCAL_KEYPAD: AtomicBool = AtomicBool::new(false);

/// The terminal the grid is written to, and what was last written there.
///
/// Its drop hands the terminal back, so an app that ends, or unwinds from
/// a panic, leaves a usable shell behind.
#[derive(Resource)]
struct Screen {
    out: Box<dyn Write + Send + Sync>,
    local: bool,
    colors: Colors,
    /// Whether [`begin`] took the terminal, so there is something to hand
    /// back.
    taken: bool,
    /// The terminal's size, columns and rows, once it is known; until
    /// then it is taken to be the grid's.
    size: Option<(u16, u16)>,
    /// The cells as the terminal shows them, or `None` when every cell
    /// must be written again: at the start, and after a resize.
    last: Option<Vec<Cell>>,
    /// Whether the terminal must be cleared before the next frame, after a
    /// resize, so nothing of the old frame stays in the margins.
    clear: bool,
    /// Whether the terminal, too small for the grid, has been told so since
    /// its last resize.
    told: bool,
    /// Every glyph found wider than a column, warned about once each.
    odd: BTreeSet<char>,
}

impl Screen {
    fn new(out: Box<dyn Write + Send + Sync>, local: bool, colors: Colors) -> Self {
        Self { out, local, colors, taken: false, size: None, last: None, clear: false, told: false, odd: BTreeSet::new() }
    }

    /// Hands the terminal back: the cursor shown, the colours reset and the
    /// screen the shell was on, then raw mode off for the local terminal.
    fn hand_back(&mut self) {
        if !std::mem::take(&mut self.taken) {
            return;
        }
        if self.local {
            restore_local();
        } else {
            let _ = self.out.write_all(&restoring(false));
            let _ = self.out.flush();
        }
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.hand_back();
    }
}

/// The bytes that hand a terminal back, popping the keyboard protocol
/// first when it was pushed.
fn restoring(keypad: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if keypad {
        let _ = queue!(out, PopKeyboardEnhancementFlags);
    }
    let _ = queue!(out, ResetColor, Show, LeaveAlternateScreen);
    out
}

/// Hands the process's own terminal back, once, whichever of the app's end
/// and a panic comes first.
fn restore_local() {
    if !LOCAL_TAKEN.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut stdout = std::io::stdout();
    let _ = stdout.write_all(&restoring(LOCAL_KEYPAD.swap(false, Ordering::SeqCst)));
    let _ = stdout.flush();
    let _ = crossterm::terminal::disable_raw_mode();
}

/// Takes the terminal: raw mode, the kitty keyboard protocol where the
/// terminal speaks it, for a numpad that is a numpad, and a panic hook
/// that hands it back before the message prints, for the local one; the
/// alternate screen, cleared, with the cursor hidden, for either.
fn begin(mut screen: ResMut<Screen>) {
    let mut out = Vec::new();
    if screen.local && !LOCAL_TAKEN.swap(true, Ordering::SeqCst) {
        let _ = crossterm::terminal::enable_raw_mode();
        if crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false) {
            let _ = queue!(out, PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES));
            LOCAL_KEYPAD.store(true, Ordering::SeqCst);
        }
        screen.size = crossterm::terminal::size().ok();
        let earlier = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_local();
            earlier(info);
        }));
    }
    let _ = queue!(out, EnterAlternateScreen, Hide, ResetColor, Clear(ClearType::All));
    screen.taken = true;
    let _ = screen.out.write_all(&out);
    let _ = screen.out.flush();
}

/// Reads every event the local terminal has waiting, without waiting for
/// one.
fn poll_terminal(mut input: MessageWriter<TtyInput>) {
    while crossterm::event::poll(Duration::ZERO).unwrap_or(false) {
        match crossterm::event::read() {
            Ok(event) => {
                input.write(TtyInput(event));
            }
            Err(_) => break,
        }
    }
}

/// The keys typed and not yet pressed, and the one pressed last frame.
#[derive(Resource, Default)]
struct Typed {
    waiting: VecDeque<Keystroke>,
    down: Option<Keystroke>,
}

/// Lets go of the key pressed last frame, and its Shift.
fn release_last_key(mut typed: ResMut<Typed>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    if let Some(down) = typed.down.take() {
        keys.release(down.key);
        if down.shift {
            keys.release(KeyCode::ShiftLeft);
        }
    }
}

/// What a frame's events asked of the terminal and the game.
#[derive(Debug, Default, PartialEq)]
struct Heard {
    /// The terminal's new size, the last one reported.
    resized: Option<(u16, u16)>,
    /// Whether Control-C was typed.
    interrupted: bool,
}

/// Queues the keystrokes among `events` behind those `waiting`, and says
/// what else they asked. A key the terminal repeated within one frame is
/// queued once, so a key held down walks a step a frame and stops when it
/// is let go, rather than walking on through a backlog.
fn hear(events: impl IntoIterator<Item = Event>, waiting: &mut VecDeque<Keystroke>) -> Heard {
    let mut heard = Heard::default();
    let mut repeated = None;
    for event in events {
        match event {
            Event::Key(key) if is_interrupt(&key) => heard.interrupted = true,
            Event::Key(key) => {
                if let Some(stroke) = keystroke(&key) {
                    if repeated != Some(stroke) {
                        waiting.push_back(stroke);
                    }
                    repeated = Some(stroke);
                }
            }
            Event::Resize(cols, rows) => heard.resized = Some((cols, rows)),
            _ => {}
        }
    }
    heard
}

/// Hears this frame's events: keys queued, a resize marking every cell to
/// be written again, Control-C ending the app as a window's close does.
fn read_input(mut input: MessageReader<TtyInput>, mut typed: ResMut<Typed>, mut screen: ResMut<Screen>, mut exit: MessageWriter<AppExit>) {
    let heard = hear(input.read().map(|TtyInput(event)| event.clone()), &mut typed.waiting);
    if let Some(size) = heard.resized {
        screen.size = Some(size);
        screen.last = None;
        screen.clear = true;
        screen.told = false;
    }
    if heard.interrupted {
        exit.write(AppExit::Success);
    }
}

/// Presses the next key waiting, with Shift when it was typed with it.
fn press_next_key(mut typed: ResMut<Typed>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    let Some(next) = typed.waiting.pop_front() else { return };
    if next.shift {
        keys.press(KeyCode::ShiftLeft);
    }
    keys.press(next.key);
    typed.down = Some(next);
}

/// Where the grid's top left corner goes in a terminal of `size`, centred,
/// or `None` when the terminal is too small to hold it.
fn origin(grid: (u16, u16), size: Option<(u16, u16)>) -> Option<(u16, u16)> {
    let Some((cols, rows)) = size else { return Some((0, 0)) };
    (cols >= grid.0 && rows >= grid.1).then(|| ((cols - grid.0) / 2, (rows - grid.1) / 2))
}

/// Writes the frame: the cells that changed, centred, inside one
/// synchronized update so a terminal that knows the sequence never shows
/// half a frame, and one that does not ignores it. A terminal too small
/// for the grid is told how large it must be instead. A writer that fails,
/// an SSH client gone, ends the app, which saves the run as a window's
/// close would.
fn draw(terminal: Res<Terminal>, mut screen: ResMut<Screen>, mut exit: MessageWriter<AppExit>) {
    if !screen.taken {
        return;
    }
    let grid = (terminal.width() as u16, terminal.height() as u16);
    let mut out = Vec::new();
    let _ = queue!(out, BeginSynchronizedUpdate);
    let cleared = std::mem::take(&mut screen.clear);
    if cleared {
        let _ = queue!(out, ResetColor, Clear(ClearType::All));
    }
    match origin(grid, screen.size) {
        Some(at) => {
            let cells = changes(screen.last.as_deref(), &terminal);
            if cells.is_empty() && !cleared {
                return;
            }
            let colors = screen.colors;
            for glyph in paint(&mut out, &cells, at, colors, &mut screen.odd) {
                warn!("{glyph:?} is wider than one column in a terminal and is drawn as '?'");
            }
            screen.last = Some(terminal.cells().to_vec());
        }
        None if !screen.told => {
            let size = screen.size.unwrap_or(grid);
            too_small(&mut out, grid, size);
            screen.told = true;
        }
        None => return,
    }
    let _ = queue!(out, EndSynchronizedUpdate);
    if screen.out.write_all(&out).and_then(|()| screen.out.flush()).is_err() {
        exit.write(AppExit::Success);
    }
}

/// Says, centred in a terminal of `size`, how large it must be to hold the
/// grid, cut to fit. The screen is already clear: `begin` cleared it, and a
/// resize clears it before this is written.
fn too_small(out: &mut Vec<u8>, grid: (u16, u16), size: (u16, u16)) {
    let lines =
        [format!("This terminal is {} by {}.", size.0, size.1), format!("The game needs {} by {}.", grid.0, grid.1), "Make the window larger.".to_string()];
    let top = size.1.saturating_sub(lines.len() as u16) / 2;
    for (i, line) in lines.iter().take(usize::from(size.1)).enumerate() {
        let line: String = line.chars().take(usize::from(size.0)).collect();
        let left = size.0.saturating_sub(line.chars().count() as u16) / 2;
        let _ = queue!(out, MoveTo(left, top + i as u16), Print(line));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode as TypedKey, KeyEvent, KeyModifiers};
    use std::sync::Arc;

    /// A writer whose bytes a test reads back.
    #[derive(Clone, Default)]
    struct Shared(Arc<Mutex<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Shared {
        /// What was written since the last time this was asked.
        fn take(&self) -> String {
            String::from_utf8(std::mem::take(&mut *self.0.lock().unwrap())).unwrap()
        }
    }

    /// A terminal of hashes over a writer, its first frame drawn.
    fn drawn() -> (App, Shared) {
        let out = Shared::default();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TtyPlugin::to_writer(6, 3, out.clone())));
        let mut terminal = app.world_mut().resource_mut::<Terminal>();
        let bounds = terminal.bounds();
        terminal.fill(bounds, Cell::new('#', Color::WHITE));
        app.update();
        (app, out)
    }

    fn key(c: char) -> TtyInput {
        TtyInput(Event::Key(KeyEvent::new(TypedKey::Char(c), KeyModifiers::NONE)))
    }

    #[test]
    fn a_frame_writes_only_the_cells_that_changed_since_the_last() {
        let (mut app, out) = drawn();
        assert_eq!(out.take().matches('#').count(), 18, "the first frame writes every cell");
        app.update();
        assert_eq!(out.take(), "", "a frame where nothing changed writes nothing");
        app.world_mut().resource_mut::<Terminal>().put(4, 2, '@', Color::WHITE);
        app.update();
        let frame = out.take();
        assert_eq!((frame.matches('@').count(), frame.matches('#').count()), (1, 0), "{frame:?}");
        assert!(frame.contains("\x1b[3;5H"), "moved to the cell: {frame:?}");
    }

    #[test]
    fn after_a_resize_every_cell_is_written_again() {
        let (mut app, out) = drawn();
        out.take();
        app.world_mut().write_message(TtyInput(Event::Resize(10, 5)));
        app.update();
        let frame = out.take();
        assert_eq!(frame.matches('#').count(), 18, "every cell: {frame:?}");
        assert!(frame.contains("\x1b[2J"), "on a cleared screen");
        assert!(frame.contains("\x1b[2;3H"), "centred in the larger terminal: {frame:?}");
    }

    #[test]
    fn a_terminal_too_small_says_how_large_it_must_be_and_draws_the_grid_once_it_is() {
        let (mut app, out) = drawn();
        out.take();
        app.world_mut().write_message(TtyInput(Event::Resize(30, 2)));
        app.update();
        let frame = out.take();
        assert!(frame.contains("needs 6 by 3") && !frame.contains('#'), "{frame:?}");
        app.update();
        assert_eq!(out.take(), "", "said once");
        app.world_mut().write_message(TtyInput(Event::Resize(6, 3)));
        app.update();
        assert_eq!(out.take().matches('#').count(), 18);
    }

    #[test]
    fn a_key_pressed_is_released_the_frame_after() {
        let (mut app, _out) = drawn();
        app.world_mut().write_message(key('k'));
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(keys.just_pressed(KeyCode::KeyK), "pressed the frame it was typed");
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(!keys.pressed(KeyCode::KeyK) && keys.just_released(KeyCode::KeyK), "and let go the next");
        app.world_mut().write_message(key('k'));
        app.update();
        assert!(app.world().resource::<ButtonInput<KeyCode>>().just_pressed(KeyCode::KeyK), "so the next press is a press again");
    }

    #[test]
    fn a_shifted_key_holds_shift_for_its_frame_and_two_keys_in_one_frame_take_a_frame_each() {
        let (mut app, _out) = drawn();
        app.world_mut().write_message(key('?'));
        app.world_mut().write_message(key('j'));
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(keys.just_pressed(KeyCode::Slash) && keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::KeyJ));
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(keys.just_pressed(KeyCode::KeyJ) && !keys.pressed(KeyCode::ShiftLeft), "j, unshifted, the frame after");
    }

    #[test]
    fn a_key_repeated_within_one_frame_is_pressed_once() {
        let mut waiting = VecDeque::new();
        let held = Event::Key(KeyEvent::new(TypedKey::Char('l'), KeyModifiers::NONE));
        hear([held.clone(), held.clone(), held], &mut waiting);
        assert_eq!(waiting.len(), 1);
    }

    #[test]
    fn control_c_ends_the_app() {
        let (mut app, _out) = drawn();
        app.world_mut().write_message(TtyInput(Event::Key(KeyEvent::new(TypedKey::Char('c'), KeyModifiers::CONTROL))));
        app.update();
        let exits = app.world().resource::<Messages<AppExit>>();
        assert!(exits.iter_current_update_messages().any(|e| *e == AppExit::Success));
    }

    #[test]
    fn the_guard_restores_the_terminal_when_dropped() {
        let out = Shared::default();
        let mut screen = Screen::new(Box::new(out.clone()), false, Colors::True);
        drop(Screen::new(Box::new(out.clone()), false, Colors::True));
        assert_eq!(out.take(), "", "a terminal never taken is not handed back");
        screen.taken = true;
        drop(screen);
        let written = out.take();
        for (what, code) in [("the cursor shown", "\x1b[?25h"), ("the shell's screen", "\x1b[?1049l"), ("the colours reset", "\x1b[0m")] {
            assert!(written.contains(code), "{what}: {written:?}");
        }
    }

    #[test]
    fn an_app_that_ends_hands_its_terminal_back() {
        let (app, out) = drawn();
        assert!(out.take().contains("\x1b[?1049h"), "the alternate screen was taken");
        drop(app);
        assert!(out.take().contains("\x1b[?1049l"), "and handed back");
    }
}
