<!-- documents:
     plugins: TtyPlugin
     files: crates/atglyph-tty/src/lib.rs
            crates/atglyph-tty/src/plugin.rs
            crates/atglyph-tty/src/keys.rs
            crates/atglyph-tty/src/paint.rs
            crates/atglyph/src/lib.rs
            crates/atglyph-save/src/backend.rs
     fingerprint: e1092f89 -->

# The text terminal

A game can be played in a text terminal instead of a window, the one it was started in or one at the far end of an SSH connection.
Every panel, the map and every screen over it already draw into one buffer of cells, `Terminal`, and the window is only one way of showing it; `TtyPlugin` is another, which writes the cells that changed to a terminal and turns the keys typed there into the keys every input system reads.
So a game, its panels and its keys run unchanged, and which front end it runs on is decided in `main`.

## Turning it on

`TtyRoguelikePlugins::new(cols, rows)`, in `atglyph::prelude::ui`, is the group a game adds in place of `RoguelikePlugins`, with the same `map` and `across`.
It is Bevy's `MinimalPlugins` running thirty frames a second, about as often as a terminal repeats a held key, with states and input, `TtyPlugin`, and the engine's half of `RoguelikePlugins`: `CorePlugin`, `FovPlugin`, `MapViewPlugin` with `ParticlesPlugin`, `UiPlugin` and `ReplayPlugin`.
It leaves out whatever needs a window, `CapturePlugin`, `FullscreenPlugin` and a typeface, since the terminal draws in its own font, and Bevy's `LogPlugin`, whose lines would land on the game's screen.
A game adds its own plugins after either group and plays the same.
The crate and the group are desktop only; the browser has no terminal.

`TtyPlugin` itself inserts the `Terminal` and the `Pointer` the window's `TerminalPlugin` would, adds Bevy's `InputPlugin` if nothing has, takes the terminal in `Startup`, reads the keys in `PreUpdate` after Bevy's own input systems, and writes the frame in `PostUpdate`.

## The model

`TtyPlugin::new(cols, rows)` is a grid of that many cells in the process's own terminal, which is put into raw mode and the alternate screen at startup, with the cursor hidden, and polled for keys every frame.
It is handed back as it was, the cursor shown, the colours reset, the shell's screen back and raw mode off, when the app ends, and from a panic hook before a panic's message prints, once whichever comes first.
`TtyPlugin::to_writer(cols, rows, out)` writes the grid to any other writer, an SSH channel or a test's buffer, with no raw mode and no polling: the keys and the size of the terminal at the other end come in as `TtyInput` messages, each a `crossterm` `Event`, written by whatever reads that end, `Event::Key` for a key and `Event::Resize` when the client's window changes.
`TtyPlugin::colors` overrides the colours the plugin assumed.
`atglyph::tty::crossterm` is the `crossterm` it speaks, re-exported so a server feeding it events names the same version.

Each frame writes only the cells of the grid that differ from the last frame's, `changes`, each as a move, its colours where they differ from the cell written before it, and its glyph, inside one synchronized update so a terminal that knows the sequence never shows half a frame.
A frame where nothing changed writes nothing.
The grid keeps the size the game gave it, drawn centred in a larger terminal; in a smaller one the plugin says how large the terminal needs to be, once, and draws the grid again whole once it is.
A resize clears the screen and writes every cell again.
A writer that fails, a client gone, ends the app with `AppExit::Success`, which saves the run as a window's close does.

`Colors` is how many colours the terminal draws: `Colors::True` draws each cell's colour as it is, and `Colors::Ansi256` the nearest of xterm's 256, leaving out the sixteen a terminal themes itself; `Colors::from_env` reads `True` from `COLORTERM` set to `truecolor` or `24bit` and `Ansi256` otherwise, and `to_writer` assumes `True`.
`shown` is the glyph a terminal draws for a cell in one column: the cell's own, or `?` for a glyph two columns wide or one that draws nothing, warned about once each, since either would tear the rest of its row.

A terminal reports a key when it is typed and what it typed, `?` or `K`, never when it is let go.
`keystroke` turns a key event into a `Keystroke`, the key a US keyboard would have pressed and whether Shift went with it, or nothing for a release, a key the engine never binds, or a chord with Control or Alt, which terminals report differently from one emulator to the next.
It reads letters, digits, a US keyboard's punctuation, the arrows, Enter, Escape, Tab, Backspace, Space, Home, End, Page Up and Down, Insert, Delete and F1 to F12.
A numpad key is a numpad key only where the terminal says so, as one speaking the kitty keyboard protocol does, which the plugin asks the local terminal for when it is there; elsewhere the numpad types the digits and arrows printed on it.
Each keystroke is pressed into `ButtonInput<KeyCode>` for the frame it was typed in, with `ShiftLeft` for a capital or a shifted symbol, and released the next, so `just_pressed` fires once a press.
One key is pressed a frame and the rest wait their turn, and a key the terminal repeated within one frame is queued once, so a held key walks a step a frame and stops when it is let go.
`is_interrupt` is Control-C, which reaches a game in raw mode as a key, and the plugin ends the app on it as closing a window does.

`Saves::per_user(name, user)` keeps each user's saves apart, in `saves/<name>/users/<user>` beside the executable, for a game many people play over SSH, inserted before `SavePlugin`; a user name that is not a plain name of letters, digits, `-` and `_` is refused with `SaveError::Slot`.

## Using it

Choosing the front end in `main`, as the starter template does on `--tty`, is the instance: the window's group or the terminal's, and every line after it the same.

<!-- include: ../../../../templates/starter/src/main.rs:front_end -->
```rust,no_run
    // The window and glyph terminal, the turn loop, field of view, the map
    // between the status row and the log, and the base every panel needs;
    // or, with `--tty`, all of it in the text terminal you started the game
    // in, which plays over SSH too.
    let map = GridRect::new(0, 1, COLS, ROWS - 1 - LOG_ROWS);
    if std::env::args().any(|arg| arg == "--tty") {
        app.add_plugins(TtyRoguelikePlugins::new(COLS, ROWS).map(map));
    } else {
        app.add_plugins(RoguelikePlugins::new(TITLE, COLS, ROWS).map(map));
    }
```

## The line

The engine decides how the grid reaches the terminal and how the terminal's keys reach the game: what is written each frame, where the grid stands in a terminal of any size, how many colours it is drawn in, what a key typed comes to, and when the terminal is taken and handed back.
The game decides which front end it runs on, its grid's size, and everything it draws, which is the same whichever front end shows it.
Serving over SSH is the game's: the engine depends on no server, and a game that serves one feeds each session's events in as `TtyInput` and hands `to_writer` the channel, swapping it into the group the way any Bevy plugin group allows, with `Saves::per_user` keeping each player's run apart.
A glyph a terminal cannot draw in one column is the game's to avoid, since the terminal draws in its own font and the engine can only mark the cell.
A game that wants a mouse in the terminal reads one itself; the plugin reads none.

## Where it lives

`atglyph-tty` is a tier 2 crate on `atglyph-render`, which holds `Terminal`, and `crossterm`, so the window's `TerminalPlugin` and this plugin are two ways of showing one buffer and no presenter knows which runs.
What a frame writes, what a key comes to, and how a colour is reduced are plain functions, `changes`, `shown` and `keystroke`, tested on values with no terminal; the plugin's own tests run an `App` against `to_writer` over a buffer and read the bytes back, so the whole front end is tested headless.
`TtyRoguelikePlugins` lives in the facade beside `RoguelikePlugins`, since it names the engine's plugins from four crates.
