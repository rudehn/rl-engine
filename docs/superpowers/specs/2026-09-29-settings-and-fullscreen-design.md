# A terminal that is sharp at any size, and a settings screen that remembers fullscreen

Status: design, agreed in conversation on 2026-09-29 against `main` at `06016da`.
Nothing here is built yet.

## 1. What this is for

A game on the engine opens in a window sized to its grid and has no way to fill the screen from inside the game.
Making the window bigger already works, because the camera stretches the grid to fit, but it stretches a picture that was drawn for the small size.
Measured in Foundry at borderless fullscreen on a 2880 by 1800 display: the glyphs are drawn at 14 pixels and stretched about 1.44 times with nearest-pixel sampling, and the stems of `T` and `e` come out uneven.
Nothing the player chooses is remembered between launches either; `rl-save` stores a run and nothing else.

Done when:

- the terminal is laid out again whenever its window changes size, with glyphs drawn at the size they are shown, so text is sharp windowed, maximized and fullscreen;
- at the window's native size the picture is exactly what it is today, so every `RL_CAPTURE` screenshot is unchanged;
- the engine has a settings registry a game adds its own rows to, a settings screen that lists and changes them, and an opt-in plugin that remembers them;
- Fullscreen is the one row the engine declares, toggled from the screen or with F11, and a game relaunched opens the way it was left;
- Foundry offers Settings from its title screen and from the Esc menu, checked in the running game.

## 2. What was decided

These were settled in conversation and are not reopened here.

1. **The screen ships with one row, Fullscreen.**
   Key rebinding, text size and particles were weighed and left out; each is a row or a screen of its own later.
2. **Settings are a registry of plain data in `rl-bevy`**, declared once and read everywhere, the way controls are.
   A row is a name, a group, a label, a list of choices, a default and optionally a key.
   No closed enum of settings and no typed resource per setting.
3. **Three crates each do one part.**
   `rl-render` declares Fullscreen and applies it to the window, `rl-ui` draws and edits the rows, `rl-save` remembers them.
   `rl-bevy` is the one crate all three already depend on, so no new edge between them is needed, and `rl-ui` still does not depend on `rl-save`.
4. **Remembering is opt-in and forgiving.**
   A game adds `SettingsSavePlugin`; without it the settings work and reset each launch.
   The file is a plain map of name to choice text, not the `Versioned` envelope, because that envelope refuses a save whole on a version mismatch and a preference should outlive an upgrade.
5. **Zoom is fractional, cells are whole pixels.**
   The grid takes the largest zoom that fits the window, and each cell is then rounded down to whole physical pixels.
   Whole-number zoom only (1x, 2x) was rejected: it is sharp, but leaves wide empty borders on most displays.
6. **No resize limit.**
   A window smaller than the grid's native size gets a zoom below 1, drawn at that size, rather than a minimum the operating system may not be able to honor on a small screen.
7. **Fullscreen is not offered on the web.**
   A browser grants fullscreen only from inside a key or click handler and Bevy reads keys a frame later.
   The canvas already follows the page, so the browser's own fullscreen works, and the new layout keeps it sharp.

### The approaches weighed

- **Typed settings**, each its own resource reached through a trait by the screen and by saving.
  Game code would read a typed value, but every setting would need plumbing in three crates, which one row does not justify.
- **The store in `rl-ui` with saving behind a trait.**
  `rl-render` sits below `rl-ui` and could not read Fullscreen, so window handling would move into the UI crate, and `rl-save` would implement a UI trait, which is the dependency the game menu dropped on purpose.

## 3. The layout

One pure function in `rl-render`, tested without an `App`:

`fit(cols, rows, base_cell, window_physical, scale_factor) -> Fit`

- `zoom` is the smaller of window width over native grid width and window height over native grid height.
- A cell's physical size is `floor(base_cell * scale_factor * zoom)`, width and height separately, never below one pixel.
  The aspect of a cell therefore drifts by at most one physical pixel.
- The grid is centred with the margin rounded down to whole physical pixels, measured from the window's top-left corner, so every cell edge lies on the pixel grid and no seam shows between backgrounds.
- The font size is the declared font size scaled by shown cell height over declared cell height.
- When the window is exactly the native size, the result is the declared cell and font and a zero margin.

The camera's projection becomes one world unit per logical pixel and no longer scales.
A relayout system compares the primary window's physical size and scale factor with the ones last laid out for, and when they differ sets every background sprite's size and position and every glyph's position and font size from `Fit`.
Bevy rasterizes the glyphs again at the new size; that happens once per change of size, not per frame.
The margin is the camera's clear color, black, as now.

`Terminal`'s public API is unchanged.
With no window, as in the `Stage` harness, nothing is laid out and nothing fails.

## 4. The settings registry, in `rl-bevy`

- `Setting::new(name, group, label, choices)`, with `.default_choice(index)` and `.key(KeyCode)`.
  `name` is the stable word the setting is remembered under; `group` and `label` are what the screen shows.
- `app.add_setting(setting) -> SettingId`.
  Declaring a name twice panics, naming it: two plugins claiming one setting is a setup mistake.
- `Settings`, a resource: `chosen(id)`, `choose(id, index)`, `cycle(id, step)` wrapping at both ends, and the rows in declaration order.
  A reader notices a change through Bevy's change detection on the resource.

## 5. Fullscreen, in `rl-render`

`FullscreenPlugin`, added by `RoguelikePlugins`, declares `fullscreen` in the `Display` group with choices `Off` and `On`, default `Off`, key F11.
When the choice differs from the primary window's mode it sets `WindowMode::BorderlessFullscreen(MonitorSelection::Current)` or `WindowMode::Windowed`.
The remembered choice is on the window before the first frame is presented, so a game left fullscreen does not flash a window first.
On `wasm32` the plugin declares nothing.

## 6. The screen, in `rl-ui`

`SettingsPanel::new(rect)` declares the `settings` modal.

- Rows are listed under their group headings; the picked row is highlighted, and each row shows its label and its current choice.
- Up and down pick a row, left and right cycle its choice, confirm cycles forward, and the close key returns to whatever was under it.
  The keys are the engine's existing direction and cursor bindings, written along the bottom border as on every other screen.
- It draws after all of `EngineSet::Present` and in any engine state, because presenters do not run while a title screen holds the engine idle, and because it must cover the menu that opened it.
- A setting with a key becomes a control in the controls registry, so the controls screen lists it.
  The key cycles the setting when no screen is open or the settings screen is the top one, in any engine state.
- The game menu offers a `Settings` row, above `Quit`, when `SettingsPanel` was added and at least one setting is declared.
  A screen with no rows is never offered, which is what the web build gets.

## 7. Remembering, in `rl-save`

`SettingsSavePlugin` needs `Saves` and says so through `app.needs`.

- It loads the `settings` slot before the first frame and applies each entry whose name is declared and whose text is one of that setting's choices.
  An unknown name or an unknown choice is skipped; text that does not parse is logged as a warning and the defaults stand.
- It writes the slot when `Settings` changes after loading, as a RON map of name to choice text, for example `{"fullscreen": "On"}`.
  Loading does not write.
- A new run, a restart and the end of a run do not touch the slot.

## 8. Foundry

- Adds `SettingsPanel` and `SettingsSavePlugin`.
- The title screen gains a `Settings` row between `New Game` and `Exit`, which opens the modal.
  The title's own keys are not read while a modal is open, and the key that closes the settings screen is not read by the title as its own.

## 9. Testing

- **Layout**, as properties over a range of window sizes and scale factors: the grid fits inside the window; it is centred to within one physical pixel; cells are whole physical pixels; the native size reproduces the declared cell; a larger window never gives a smaller cell.
- **Registry**: defaults, cycling and wrapping, declaration order, the duplicate-name panic.
- **Remembering**, through `MemoryBackend`: a round trip; unknown names and choices skipped; garbled text falls back to defaults; loading does not write.
- **Screen**, in the `Stage` harness: the menu offers Settings only with the panel and a row; opening, cycling and closing back to the menu; the key flipping the setting with nothing open; the controls screen listing it.
- **Fullscreen**: in a headless app with a window entity, the setting drives the window's mode both ways.
- **In the running game**: Foundry set fullscreen from the title screen and screenshotted, glyphs magnified and compared with the stretched capture this design started from; relaunched to see it remembered with no windowed flash; F11 mid-run; a native-size `RL_CAPTURE` identical to one from `main`.

## 10. Documentation

Paid once when the branch finishes, as `CLAUDE.md` describes.

- `docs/guide/src/systems/settings.md`, new, quoting Foundry's wiring; `rendering.md` updated for the layout.
- `docs/design/settings.md`, new, listed in `docs/README.md` and in the layout section of `CLAUDE.md`.
- `docs/OVERVIEW.md`: the three new plugins and the registry.
- `CHANGELOG.md` under `Unreleased`, a line per commit.
- `README.md`'s feature list: one line for settings and fullscreen.

## 11. Review focus

- The native-size capture is byte-identical; any difference means `fit` is not the identity where it must be.
- No seam between cell backgrounds at an odd window size on a 2x display and on a 1x display.
- `rl-ui` gained no dependency on `rl-save`, and nothing orders itself after another crate's system function.
- The settings screen over the title screen: keys do not leak to the title, and the title redraws cleanly when it closes.

## 12. Future decisions

- macOS's green window button enters the system's own fullscreen without going through the setting, so the row can read `Off` while the window fills a screen.
  Whether to read the window's state back into the setting is left until it is seen to matter.
- Remembering the windowed size and position.
- Rows a game or the engine might add: text size, particles, key rebinding.
