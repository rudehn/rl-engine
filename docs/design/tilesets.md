# Tilesets: pictures in place of glyphs

Written 2026-10-02, and built the same day.

## 1. What it is for

A game drawn in pictures rather than letters, without a second renderer.
The same game in letters for a player who prefers them, at the flick of a setting.

## 2. What was considered

The map is drawn by `draw_map` writing cells into the terminal, and everything else that shows on the map writes cells too: the light that tints a tile, the fade of what is remembered, fire and gas, particles, the look cursor, the targeting overlay, and whatever a game draws over the map itself.

The first plan was a second presenter over a shared scene: a collector that says what is in each cell, and a sprite presenter beside the glyph one.
It meant porting every one of those writers to the scene, about twelve hundred lines of particles and fields and both cursors in `rl-ui`, and keeping two drawings of each in step from then on.
Every game's own overlay would have needed porting as well, and a game that forgot would have drawn letters over its pictures.

## 3. The design

### 3.1 A tileset is a font

`Tileset` says how a character looks: for each character it names, a picture from a sheet.
The terminal draws by it at the one place cells become entities, `flush_terminal`: a cell whose glyph has a picture shows the picture and no glyph, tinted by the cell's own colour, and any other cell is drawn as before.

Nothing that writes cells changes.
The light on a tile, the cold blue of memory, a flame's flicker and a cursor's wash are all colours on a cell, and a picture takes its cell's colour as a letter does.
A game's own overlay is drawn in pictures the day the game adds a tileset, without knowing one exists.

### 3.2 Chosen by character

A picture is chosen by the character in the cell, not by what the thing is.
Two things that should look different are given different characters, which a game drawn in letters wants anyway, and a game with more things than letters has the rest of Unicode.

Declined: choosing by content id, a tile's or a monster's.
The terminal holds characters and colours and knows nothing of content, which is why every panel and every game can draw into it, and a cell that also carried an id would be a second thing for every writer to fill.

### 3.3 Where it covers

`Tileset::within(rect)` limits it to part of the terminal, the map's rectangle for a game whose panels stay text.
Text drawn there would otherwise turn into pictures wherever a word used a character the sheet names.

### 3.4 Off, and gone

`Tileset::turned(false)` draws every cell as its glyph, and removing the resource does the same.
Either redraws the whole grid once, since any cell may now be drawn the other way, and is the whole of a letters-or-pictures setting.

### 3.5 Tinted

A picture is multiplied by its cell's colour.
A sheet of white pictures is therefore coloured by the game exactly as its letters are, and one sheet serves every monster's colour.
A sheet in full colour is shown as drawn by a game that gives those cells a white glyph colour, and is still dimmed by light and memory, which is the point.

### 3.6 What it costs

One more sprite per cell, spawned the first time there is a tileset and never in a game without one.
Hidden until a cell shows a picture, and laid out with the cell, so it is sharp in a window of any size as the glyphs are.

## 4. What waits

- A picture larger than its cell, or drawn across several.
- Pictures that animate.
- Panels in anything but text, which is the Bevy UI presenters `docs/design/ui.md` defers.
