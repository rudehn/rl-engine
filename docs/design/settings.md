# Settings, and a terminal laid out for its window

What a player chooses once and expects to find again, and the one such choice the engine owns: whether the game fills the screen.
The two were built together because the second needed the first to be worth having.
A fullscreen that resets every launch is an annoyance, and a fullscreen that stretches a small picture is a blurry one.

The reference page is `docs/guide/src/systems/settings.md`; this is why it is shaped that way.

## What was wrong

A game opened in a window sized to its grid and could not fill the screen from inside the game.
Making the window bigger worked, because the camera scaled the grid to fit, but it scaled a picture drawn for the small size.
Measured in Foundry at borderless fullscreen on a 2880 by 1800 display: glyphs rasterized at 14 pixels were stretched about 1.44 times with nearest-pixel sampling, and the stems of `T` and `e` came out uneven.
Nothing a player chose was remembered; `rl-save` stored a run and nothing else.

## The layout

The grid is laid out for the window it is in, by one pure function, `rl_render::layout::fit`.

- **Fractional zoom, whole-pixel cells.**
  The zoom is the largest that fits the grid in the window, and each cell is then floored to whole physical pixels, width and height separately.
  Whole pixels because a cell of a fractional width leaves a seam between two backgrounds; separately because flooring them together would waste most of a cell's worth of screen, and the shape of a cell drifts by at most one pixel.
- **Not whole-number zoom.**
  Zooming only by 1, 2 or 3 is sharp without any of this, and was rejected: on most displays the next whole zoom does not fit, and the game sits in a wide empty border.
- **The camera does not scale.**
  One world unit is one logical pixel, the cells are moved and sized, and the glyphs take a font size scaled with the cell, so Bevy rasterizes them at the size they are shown.
  That happens once per change of window size, not per frame, and in `PreUpdate`: Bevy works out where a thing is drawn and lays text out late in `PostUpdate`, and a layout written there in no order against them could be drawn a frame late.
- **The native size is the identity.**
  A window of exactly the grid's declared size gets exactly the declared cell and font and no margin, so nothing a game looked like before changes, and a capture is the same file.
  That holds where the declared cell is a whole number of physical pixels, at 100%, 150% and 200%; at 125% a ten-pixel cell is twelve and a half, the cell is floored, and the game opens inside a thin border, which is the price of never drawing a cell across a pixel.
  A float's rounding nearly broke this: a zoom of `0.99999994` floors a twenty-pixel cell to nineteen, so the floor is taken a thousandth of a pixel high, which is far less than one pixel across any grid a terminal has.
- **No resize limit.**
  A window smaller than the grid gets a zoom below 1, drawn at that size.
  A minimum size was rejected because the operating system may not be able to honor one on a small screen, and a minimized window, of no size at all, is simply left as it was last laid out.

## The registry

`Settings` is plain data in `rl-bevy`: a name, a group, a label, the choices, a default and an optional key.

- **Why `rl-bevy`.**
  It is the one crate `rl-render`, `rl-ui` and `rl-save` all already depend on.
  The renderer declares and applies fullscreen, the UI lists and changes whatever is declared, and saving remembers names and words, with no new edge between them.
  In particular `rl-ui` still does not depend on `rl-save`, an edge the game menu dropped on purpose when it stopped filing obituaries.
- **Why not typed settings.**
  Each setting its own resource, reached through a trait by the screen and by saving, would hand game code a typed value.
  It would also need plumbing in three crates per setting, which one row does not justify, and a game's settings would be declared differently from the engine's.
- **Why not in `rl-ui`.**
  `rl-render` sits below `rl-ui` and could not read fullscreen, so window handling would move into the UI crate, and `rl-save` would implement a UI trait.
- **Why a choice among words.**
  One shape of row keeps the screen one presenter and the file one map.
  A slider, a rebound key and a line of text are different screens, and a game that wants one writes it.
- **Why words are remembered, not places.**
  A choice added in the middle of a setting would turn everyone's remembered index into another choice.

## Remembering

- **Opt-in**, as every subsystem is: without `SettingsSavePlugin` the settings work and start on their defaults.
- **Not the versioned envelope.**
  That refuses a save whole when its version differs, which is right for a run and wrong for a preference.
  The file is a map of names to words, and a name nobody declares, a choice that is gone and text that is not RON at all each leave a default standing and stop nothing.
- **Recalled in `finish`, applied in `cleanup`.**
  Every plugin has declared its settings by `finish`, and `cleanup` runs after every `finish` and before the first frame, so a game left fullscreen opens that way with no window shown first.
  On the primary monitor then, since a window that does not exist yet is on none; a switch during play uses the monitor the window is on.

## The screen

- **Outside the engine's sets.**
  Presenters run only while a world is shown, and the settings screen is the one a game opens before there is one, from its own title screen.
  So it is read before `EngineSet::Input` and drawn after the whole of `EngineSet::Present`, in any state, which also puts it over the menu that opened it.
- **The frame it opens and the frame it closes.**
  The key that opened it is the confirm key and the key that closes it is the close key, both of which mean something to the screen underneath.
  The screen reads nothing in the frame it opened, and a screen under it reads nothing in a frame where one closed, so neither order of the two systems double-reads a key.
- **Nothing drawn under it.**
  It and the menu are each as tall as their rows and seldom the same height, so the menu steps aside while it is up.
  A game's own screen under it does the same; Foundry's title stops drawing its menu rows and moves the settings screen to stand where they were.

## Not on the web

A browser grants fullscreen only from inside the handler of a key or a click, and Bevy reads keys a frame later, so the setting could not be honored there and is not declared.
The canvas already follows the page, the browser's own fullscreen fills the screen, and the layout keeps it sharp.

## Left for later

- The operating system's own fullscreen, such as the green button on macOS, does not go through the setting, so the row can read `Off` over a window that fills a screen.
  Reading the window back into the setting waits until it is seen to matter.
- Remembering the window's size and place.
- Text size, particles and key rebinding as settings or screens of their own.
