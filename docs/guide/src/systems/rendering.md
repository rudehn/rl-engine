<!-- documents:
     plugins: TerminalPlugin, ParticlesPlugin, MapViewPlugin, CapturePlugin
     files: crates/atglyph-render/src/lib.rs
            crates/atglyph-render/src/terminal.rs
            crates/atglyph-render/src/layout.rs
            crates/atglyph-render/src/pointer.rs
            crates/atglyph-render/src/tileset.rs
            crates/atglyph-render/src/map_view.rs
            crates/atglyph-render/src/fields.rs
            crates/atglyph-render/src/shade.rs
            crates/atglyph-render/src/looks.rs
            crates/atglyph-render/src/particles.rs
            crates/atglyph-render/src/capture.rs
            crates/atglyph-bevy/src/cue.rs
            crates/atglyph-ui/src/tone.rs
            crates/atglyph/src/lib.rs
     fingerprint: 5fd825bd -->

# Rendering

Drawing is a glyph terminal and three things that write into it: the map from the player's point of view, what flies over it for a moment, and a screenshot taken with nobody at the keyboard.
A game writes `Cell`s into a back buffer as if it were a console, and the plugin pushes to the screen only the cells whose contents changed, so a turn-based frame where nothing moved costs nothing.
What a tile looks like is the game's to say; what light, memory, fire and gas do to that look is the engine's.

## Turning it on

`RoguelikePlugins` adds all four, and `FullscreenPlugin` beside them, which the settings page covers, because a game with no terminal has nothing to draw into and a map with no viewport is a map nobody sees.
`TerminalPlugin::new` takes the grid in cells, the pixel size of one cell and the font height, spawns the camera and the cell entities in `Startup`, lays the grid out again in `PreUpdate` when the window has changed, early enough that it is drawn where it was put in the same frame, and flushes the buffer in `PostUpdate`.
`TerminalPlugin::typeface`, and `RoguelikePlugins::typeface` for a game on the group, takes the bytes of a TrueType or OpenType file the game ships, a `&'static` slice or a `Vec<u8>` read at run time, and every glyph is drawn in it.
Bytes rather than an asset path, because an asset arrives after the first frames are drawn, and on the web only from a page that serves the game's assets beside it.
Without one, glyphs come from the system's monospace family, which needs Bevy's `system_font_discovery` feature and is another face on every platform, a glyph it lacks drawn as an empty box; a browser has no font database to search, so on wasm they come from the font Bevy embeds, which covers printable ASCII and nothing else.
`MapViewPlugin` takes the `GridRect` it draws in, the way every panel does, so a game has no `MapView` of its own to insert and cannot forget one.
Its `finish` declares `depends_on::<CorePlugin>` and `depends_on::<FovPlugin>`: without field of view no tile is ever seen or remembered, and the map would draw as nothing at all.
In `PresentSet::Map` it gives the standard tiles of a newly added world the standard look where the game gave them none, then follows the player, tracks the fields, remembers still things and draws, in that order.
It dresses props, emptied containers and the stairs the engine laid in plain `Update` rather than in a play-only set, because a prop is put down while a place is built and `Added` matches for one frame only, and between `EngineSet::Turns` and `EngineSet::Present`, so a prop put down in a turn is never drawn or remembered bare.
`ParticlesPlugin` declares `depends_on::<MapViewPlugin>` and draws after the map, so a burst shows through a targeting cursor and under a menu.
It reads cues in `PresentSet::Narrate` and watches the turns' `CueWait` on entering `Playing`, but only if the style takes time, so a headless test with an instant style is never made to wait a frame for a fade.
`CapturePlugin` does nothing unless `ATGLYPH_CAPTURE` names a file, which is why it can sit in the group a game adds without asking.

## The model

`Cell` is one character position: a `glyph`, a foreground `fg` and a fill `bg`, with `new`, `on` and `dimmed` to build one.
`Terminal` is the back buffer, a resource, so any crate's presenter writes into it: `set`, `set_wide`, `set_bg`, `put`, `print`, `print_on`, `fill` and `clear` write, `get` and `span` read back, and every write outside the grid is dropped rather than wrapping or panicking.
`Fit` is how the grid sits in a window, a cell and a margin in physical pixels, and `layout::fit` works it out: the largest zoom that fits, each cell floored to whole pixels so no seam shows between two backgrounds, and the grid centred.
`Pointer` is the terminal cell the mouse is over, kept by `TerminalPlugin` once a frame after the grid is laid out, by `Fit::cell_at`, the same arithmetic run backwards: `cell` is that cell, `within(rect)` the cell when it is inside a panel's rectangle, and `tile(&view)` the map tile a `MapView` draws there; it is `None` in the margin and outside the window, and with no window it is left where a test put it with `Pointer::at`.
`Tileset` is pictures in place of glyphs: a sheet, how it is cut up, a picture per character with `with(glyph, index)`, and with `within(rect)` the part of the terminal it covers; the flush shows a cell whose glyph it names as that picture, tinted by the cell's colour, and no glyph, on a sprite spawned per cell the first time there is a tileset, and `turned(false)`, or removing the resource, draws every cell as its glyph again.
`MapViewPlugin::new(rect).across(2)` draws each tile two cells across, for square tiles on narrow cells: `MapView::to_screen` answers a tile's first cell, `to_world` takes any of its cells back, `tiles` walks the view, and `set`, `wash` and `put` write a whole tile, `set` as one wide cell of the terminal, whose glyph or picture is drawn centred across its run until something else is written into it, so a screen drawn over the map is drawn a letter to a cell.
The camera never scales; the glyphs take a font size scaled with the cell and are drawn at the size they are shown, and a window of exactly the size the game opened in gets exactly the declared cell, wherever that cell is a whole number of physical pixels; at a display scaling of 125% a ten-pixel cell is not, and is floored.
`Glyph` is how an entity is drawn, a `ch`, an `fg` and a `layer`, and on a tile with two things on it the higher layer wins.
`TileAppearance` is what each tile looks like in full light, indexed by `TileId`: `set` and `set_varied` fill it, `lit` reads it back, and an id the game never described draws as a magenta question mark so the gap is visible rather than blank.
`seen` is that look jittered for the cell and the moment, `remembered` is it jittered as it was seen and then faded, and `under` colours both of a cell's colours by the light landing there.
The map fills `Knowledge` once a frame with the topmost glyph on every tile in sight of anything that is not an `Actor` and not `Hidden`, or nothing, and `recalled` draws it over the tile once it is out of sight, in its own colour faded by the same `Memory`, so the stairs behind the player read as remembered and stay legible.
`TileAppearance::from_tiles` builds the table from the `TileLook` each tile in a `TileRegistry` carries, written in code or read with the tile from a file by `atglyph_rules::tiles::load`, and names at once every tile without one, so a gap is reported at startup rather than found on the map.
`Vary` is how a tile strays from cell to cell and over time, a `brightness` spread, a `hue` spread per channel and a `shimmer` that drifts; `Memory` is the brightness, saturation and cool `tint` a remembered tile keeps; `Shading` is how light becomes colour, with the level a tile shows its authored colour at, what an unlit but seen tile keeps, and the speeds a flicker and a shimmer move at.
All of it is cosmetic, and the only clock is the frame's, so a replayed seed plays identically however it is coloured.
`FieldAppearance` is fire and gas over the tiles they are on: a `flame` cell and the `flare` it flickers toward, the `ember` that now and then rises off a flame into the cell over it, the `haze` glyphs gas thick enough to hide behind is drawn with, thinnest first, and a tint per `GasId` that is grey until the game names one.
What the map draws of fire and gas is `ShownFields`, not the fields as they stand: when a cell's fields change it blends from what it showed to what they hold over `BLEND_SECS`, so a cloud rolls on a turn rather than jumping, and a player who acts mid-blend sees it pick up from where it had got to.
A blend starts when the last `Wavefront` still playing reaches the cell, which is how the smoke and fire a blast left show behind its front rather than before it has gone off.
`churn` stirs how thick a cloud looks, in patches that swell and thin with the wall clock, so smoke hanging between turns still moves, and a cloud in sight is drawn whole over the ground the player knows through `ShownFields::clouds_in_sight`, since a cloud that hides its own far side would otherwise read as a wall.
`MapView` is a `viewport` of terminal cells and the world `origin` drawn at its top-left, with `center_on`, `clamp_to`, `to_screen` and `to_world`.
`clamp_to` is why the view never shows void past the edge of a map barely larger than it, and a map smaller than the viewport is centred in it instead, since there is nothing to scroll.
`LightOverlay` draws each visible tile's light as a digit, and only where there is a `Lighting` to read: a game with no lighting plugin sees the map drawn as it always is, since a world with no light to measure has nothing to put in the digit.
`Particles` is what is playing, and `is_waited_on` says whether any of it keeps the turns waiting.
What plays is the engine's own: one animation per actor per frame, built from that actor's `Cued` messages in the order they were written, each waited on, and left behind when the player leaves its map.
An animation is a list of beats, each a trail, a glyph flying between two anchors with three cells of fading tail, or a burst, every anchor lit and fading, each cell showing one of a few glyphs picked by position so it reads as embers rather than a stamp.
A burst with a source is a blast: each anchor lights `ring_secs` later for every cell it is further from the source, give or take half a ring so the front is ragged, burns down its glyphs from white-hot to embers while lighting the ground under it, and throws a few sparks ahead of the front to its far edge.
A beat's frame is a list of sparks, each a cell, a glyph, a colour, how far it has faded and how far it lights the ground, worked out from where the anchors stand that frame, so the line is redrawn every frame between where the two anchors are now and a flight at someone who walks on bends to follow them.
`ParticleStyle` is the pace and the look of anything with none of its own: the `plain` glyph and colour, `cell_secs` per cell of flight, `burst_secs` a burst takes, the `burst_glyphs` it picks from, and a blast's `ring_secs` and the `blast_glyphs` it cools through.
A style whose `cell_secs` and `burst_secs` are both nought takes no time, which is what a headless test wants, and then the plugin never watches the turns; it tells the map where every blast's front is before the map is drawn either way.
It keeps the loop stopped exactly while something waited on is playing and the player does not already hold a turn, and `skip_on_key` lets a key pressed during the wait drop everything waited on, run the turns on, and still be read with the player's turn in hand.
`CapturePlugin` reads `ATGLYPH_CAPTURE` for the path, `ATGLYPH_CAPTURE_KEYS` for keys to press first, `ATGLYPH_CAPTURE_FRAMES` for the earliest frame, and `ATGLYPH_CAPTURE_AT=hold` to shoot a tenth of a second into the first hold rather than after it.
`capture::prepare` puts the window above the others and unfocused, so a capture never takes the keyboard from whoever is at the machine, and a frame that comes back entirely black is refused with an error rather than saved.

## Using it

Turning drawing on is one plugin group, and the rectangle the map gets is the only decision in it.

<!-- include: ../../../../examples/tutorial/src/bin/step01_walking.rs:main -->
```rust,no_run
fn main() -> AppExit {
    App::new()
        // The window, the glyph terminal, the turn loop, sight, the map and
        // the UI base.
        .add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS))
        // The game itself, the same plugin the tests add.
        .add_plugins(Warren)
        .insert_resource(Seed(RunSeed(7)))
        .run()
}
```

A game drawn in its own face hands the group the bytes of its font file, compiled in with `include_bytes!` and shipped beside its licence, as `RoguelikePlugins::new(title, cols, rows).typeface(FACE)`, with `.cell` and `.font` for the size of a cell and of the glyph in it.

What the engine cannot supply is the look of a tile, which is a table the game fills or a file it loads.

<!-- include: ../../../../examples/tutorial/src/bin/step07_content.rs:looks -->
```rust,no_run
    /// What each tile is and how it looks, read from `assets/tiles.ron`.
    fn new() -> Self {
        Self { tiles: atglyph::rules::tiles::load(TILES_RON).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}")) }
    }
```

## The line

The engine draws; the game says what things look like.
A tile's colours, a prop's glyph and an item's are content, authored per tile or per definition, and the engine only darkens, fades, tints and jitters what it was handed.
That is why nothing here takes a `ToneId`: `Cell` and `Glyph` carry a `Color` outright, because a green slime is green in every palette and a floor's brown is not a semantic role.
A `ToneId` is the other half of the same rule, one layer up: a widget takes a role, `Palette` holds the colour per role, and a game restyles every panel at once by replacing one resource.
So the boundary falls between a thing and a word about a thing: the map and the entities on it are drawn in their own colours, and everything a panel says about them is drawn in a tone.
A prop is described twice for the same reason, and the second half is here: the engine spawns it with its kind and no glyph, and the map view gives it the one its definition asks for, so nothing below this crate names a `Color`.
A prop a game dressed itself keeps what it was given, since the query asks only for those with no glyph.
Memory is what was seen, not what is true, and which things it keeps is the engine's: an item, a prop or a way through is remembered where it was last seen until its tile is seen again, and a creature never is, since where one stood says nothing of where it is; a game decides only which places are known by heart, by `Knowledge::learn`.
Fire and gas are drawn on tiles in sight and on the rest of a cloud in sight over known ground, and nowhere else, because memory holds no smoke and a cloud must not show the shape of a room nobody has seen; what hides behind a haze is decided by the map's opacity rather than by the renderer's taste.
The blending and the waiting are the drawing's alone: what the fields hold, what hides whom and what a turn does never wait for a blast to reach a cell, so a headless game plays the same turns.
The engine will not animate a game's own action: a resolver says what is worth seeing by writing a `Cued`, and what plays it is a plugin that may not be there.
Without a watcher the cues are written and forgotten and the loop runs as if there were none, which is what a headless game gets and why the same rules run with and without a window.
The pace is the engine's and the look is the game's: `ParticleStyle` is one resource, and a cue carries the look its ability, its item or the game gave it, so a game shapes what plays through the cues it writes rather than through the animations.
The terminal is not a widget toolkit and the map view is not a camera: there is no scene graph, no z-ordering beyond a glyph's layer, and nothing between turns is interpolated but how thick fire and gas look.
A run of cells drawn as one is the buffer's, written with the tile, rather than a region of the screen declared wide once: the same cell is half of a tile one frame and a letter of a screen over the map the next, and only what was written there knows which.
A tileset is a font and not a second renderer: everything that writes cells, the light, memory, fire, particles, cursors and a game's own overlays, is drawn in pictures without changing, and a picture is chosen by the character in the cell, so the terminal still knows nothing of content.
The engine says which cell and which tile the mouse is over and nothing more: what a click there means, and which button, is the game's, read from Bevy's own `ButtonInput<MouseButton>`.
One sprite and one text entity per cell is fine at a hundred columns and would be replaced by an instanced grid for a bigger one, and nothing above this crate would notice.

## Where it lives

`atglyph-render` is tier 2 and sits below `atglyph-ui` and above `atglyph-bevy`, which is what lets a panel and the map write into one `Terminal` without either knowing the other.
Keeping the shading in a module of its own makes it plain functions over a colour, a light level and a position, tested with no `App` and read by no rule.
The loader is separate again, so a game's colours are a file beside its monsters and a new tile is a line rather than a recompile.
The layout is the same again: `fit` is arithmetic over a grid, a window and a scale factor, so that it fits, is centred and is whole pixels are properties tested over a range of windows with no `App`.
The particles' arithmetic is a beat's and an animation's frame over a time and the anchors, which is why what a flight shows at a given moment is a test rather than a screenshot, and `fields.rs` is the same for the map's side of a blast: when a cell's smoke shows and how thick is `ShownFields` over a time, with no `App`.
`atglyph-bevy` owns the cue and the wait, not the drawing, so a headless game raises the same cues and waits for nothing.
`CapturePlugin` is its own plugin because it is the one thing here that is not about a player: a game disables it the way Bevy's groups allow, and a run without the environment variable never notices it.
