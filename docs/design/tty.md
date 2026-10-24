# The text terminal

Status: built 2026-10-22, as usability branch 08, against section 2.5 of `docs/reviews/engine_usability.md`.
The reasoning is here; `docs/OVERVIEW.md` lists what exists, and `docs/guide/src/systems/tty.md` is the reference.

## 0. Summary

Every panel, the map and every screen over it draw into one buffer of cells, `atglyph_render::Terminal`, and the window was only one way of showing it.
`atglyph-tty` is a second: `TtyPlugin` writes the cells that changed since the last frame to a text terminal, the one the game was started in or one at the far end of an SSH channel, and turns the keys typed there back into the `ButtonInput<KeyCode>` every input system reads.
So a game, its panels and its keys run unchanged, and choosing the front end is one line in `main`, as the template's `--tty` is.
`atglyph::TtyRoguelikePlugins` is the group a game adds in place of `RoguelikePlugins`.

## 1. The seam is the cell buffer

Nothing in `atglyph-bevy` or `atglyph-ui` names a window: a presenter writes `Cell`s into `Terminal`, and `TerminalPlugin` was already the only thing that turned them into pixels.
So the terminal front end replaces that one plugin and nothing above it.

The alternative was a second set of presenters, one per view, drawing with a terminal widget library.
It was refused for the reason `docs/design/ui.md` gives for not writing Bevy UI presenters yet: every view would be drawn twice, tested twice, and forked by the first game that wanted the two to look alike.
A game with no Bevy at all still owns its loop with `crossterm` directly, as `examples/terminal` does, on the tier 1 crates alone; this crate is for a game on the framework.

A frame writes only what changed, each cell as a move, its colours where they differ from the cell written before it, and its glyph, inside one synchronized update so a terminal that knows the sequence never shows half a frame.
A whole screen of escape codes thirty times a second would fill an SSH link for a game that mostly stands still; a frame where nothing moved writes nothing at all.

## 2. A key is a press for one frame

A terminal reports a key when it is typed and never when it is let go, and it reports what the key typed, `?` or `K`, where a window reports which keys went down.
The engine reads the second, so each press is turned back into its key on a US layout, the one the engine's own key names assume, with Shift held for the same frame when it typed a capital or a shifted symbol.

A key is pressed for the frame it was typed in and released the next, so `just_pressed` fires once for each press, and a key held down walks as the terminal repeats it.
One key is pressed a frame and the rest wait their turn, since a chord's Shift is down for the frame its key is and two keys in one frame would share it.
A key the terminal repeated within one frame is queued once, so a held key walks a step a frame and stops when it is let go, rather than walking on through a backlog the terminal built up while a turn was being drawn.

A chord with Control or Alt is dropped, because terminals report them differently from one emulator to the next, and a binding that worked in one terminal and not another is worse than one that is not offered.
The numpad is a numpad only where the terminal speaks the kitty keyboard protocol, which the plugin asks for when it is there; anywhere else the numpad types the digits and arrows printed on it, and they become those keys.
Control-C reaches a game in raw mode as a key rather than as the signal that would have ended it, so the plugin ends the app on it as closing a window does, and the run is saved the same way.

## 3. The terminal is always handed back

The process's own terminal is put in raw mode and the alternate screen at startup, and handed back as it was when the app ends: the cursor shown, the colours reset, the shell's screen back, raw mode off.
A panic hands it back too, from a hook set at startup, before the message prints, since a panic that left the shell in raw mode would leave the player unable to read why.
Both paths can run, so the hand-back is guarded to happen once.

A writer that fails, an SSH client gone, ends the app, which saves the run as a window's close would.

## 4. Size and colour

The grid keeps the size the game asked for whatever the terminal's, drawn centred in a larger one.
A terminal too small says how large it needs to be, once, and draws the grid again whole once it is resized large enough.
There is no scaling, since a cell is a character and a terminal has no smaller one to offer.

Colours are drawn as the cell has them where `COLORTERM` says the terminal draws any colour, and as the nearest of xterm's 256 otherwise, leaving out the first sixteen, which every terminal shows in a theme of its own.
A glyph a terminal draws two columns wide, an ideograph or most emoji, would push every cell after it on its row one column right, so it is drawn as `?` and warned about once.
The window draws box-drawing and block characters itself; a terminal draws them in its own font, which every terminal has.

## 5. Over SSH

`TtyPlugin::to_writer(cols, rows, out)` writes the grid to any writer, and the terminal's keys and size arrive as `TtyInput` messages from whoever holds the other end.
The SSH server is the game's: the engine depends on no server crate, and re-exports the `crossterm` it speaks so a server feeding it events names the same version.
`Saves::per_user(name, user)` keeps each player's saves apart, refusing a user name that is not a plain name, so no session reaches another's slot or outside the save directory.

`TtyRoguelikePlugins` is Bevy's `MinimalPlugins` at thirty frames a second, about as often as a terminal repeats a held key, with states and input, and the engine's core, sight, the map view and its particles, the UI base and the replay.
It leaves out everything that needs a window, capture, fullscreen and a typeface, and Bevy's `LogPlugin`, whose lines would land on the game's screen.

## 6. What is not here

- A mouse: a terminal can report one, and nothing reads it yet, as nothing reads `Pointer` in the window beyond a cell.
- The hang-up signal: a session cut off mid-run exits without saving; `docs/TODO.md` has it.
- Somewhere for warnings to go, a file or stderr when that is not the terminal; `docs/TODO.md` has it.
- A terminal build that skips the GPU crates: `Terminal` lives in `atglyph-render` beside the window's drawing, so a terminal game still compiles them and never starts them; splitting the buffer into a crate of its own is in `docs/TODO.md`.
- The browser, which has no terminal; the crate is desktop only.
