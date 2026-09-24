<!-- documents:
     plugins: TerminalPlugin, ParticlesPlugin, MapViewPlugin, CapturePlugin
     files: crates/rl-render/src/lib.rs
            crates/rl-render/src/terminal.rs
            crates/rl-render/src/map_view.rs
            crates/rl-render/src/shade.rs
            crates/rl-render/src/looks.rs
            crates/rl-render/src/particles.rs
            crates/rl-render/src/capture.rs
            crates/rl-bevy/src/cue.rs
            crates/rl-ui/src/tone.rs
            crates/rl-engine/src/lib.rs
     fingerprint: 7fc8194d -->

# Rendering

Drawing is a glyph terminal and three things that write into it: the map from the player's point of view, what flies over it for a moment, and a screenshot taken with nobody at the keyboard.
A game writes `Cell`s into a back buffer as if it were a console, and the plugin pushes to the screen only the cells whose contents changed, so a turn-based frame where nothing moved costs nothing.
What a tile looks like is the game's to say; what light, memory, fire and gas do to that look is the engine's.

## Turning it on

`RoguelikePlugins` adds all four, because a game with no terminal has nothing to draw into and a map with no viewport is a map nobody sees.
`TerminalPlugin` takes the grid in cells, the pixel size of one cell and the font height, spawns the camera and the cell entities in `Startup`, and flushes the buffer in `PostUpdate`.
Glyphs come from the system's monospace family, which needs Bevy's `system_font_discovery` feature; a browser has no font database to search, so on wasm they come from the font Bevy embeds, which covers printable ASCII and nothing else.
`MapViewPlugin` takes the `Rect` it draws in, the way every panel does, so a game has no `MapView` of its own to insert and cannot forget one.
Its `finish` declares `depends_on::<CorePlugin>` and `depends_on::<FovPlugin>`: without field of view no tile is ever seen or remembered, and the map would draw as nothing at all.
It chains `follow_player` and `draw_map` in `PresentSet::Map`, and puts `dress_props` before `draw_map` in plain `Update` rather than in a play-only set, because a prop is put down while a place is built and `Added` matches for one frame only.
`ParticlesPlugin` declares `depends_on::<MapViewPlugin>` and draws after `draw_map`, so a burst shows through a targeting cursor and under a menu.
It reads cues in `PresentSet::Narrate` and takes the turns' hold on entering `Playing`, but only if the style takes time, so a headless test with an instant style is never made to wait a frame for a fade.
`CapturePlugin` does nothing unless `RL_CAPTURE` names a file, which is why it can sit in the group a game adds without asking.

## The model

`Cell` is one character position: a `glyph`, a foreground `fg` and a fill `bg`, with `new`, `on` and `dimmed` to build one.
`Terminal` is the back buffer, a resource, so any crate's presenter writes into it: `set`, `put`, `print`, `print_on`, `fill` and `clear` write, `get` reads back, and every write outside the grid is dropped rather than wrapping or panicking.
`Glyph` is how an entity is drawn, a `ch`, an `fg` and a `layer`, and on a tile with two things on it the higher layer wins.
`TileAppearance` is what each tile looks like in full light, indexed by `TileId`: `set` and `set_varied` fill it, `lit` reads it back, and an id the game never described draws as a magenta question mark so the gap is visible rather than blank.
`seen` is that look jittered for the cell and the moment, `remembered` is it jittered as it was seen and then faded, and `under` colours both of a cell's colours by the light landing there.
`TileAppearance::load` reads the same table from RON against a `TileRegistry`, refusing the file and naming at once every entry for a tile that is not registered, every tile described twice, and every registered tile the file leaves out, so a gap is reported at startup by name rather than found on the map.
`Vary` is how a tile strays from cell to cell and over time, a `brightness` spread, a `hue` spread per channel and a `shimmer` that drifts; `Memory` is the brightness, saturation and cool `tint` a remembered tile keeps; `Shading` is how light becomes colour, with the level a tile shows its authored colour at, what an unlit but seen tile keeps, and the speeds a flicker and a shimmer move at.
All of it is cosmetic, and the only clock is the frame's, so a replayed seed plays identically however it is coloured.
`FieldAppearance` is fire and gas over the tiles they are on: a `flame` cell and the `flare` it flickers toward, the `haze` glyph gas thick enough to hide behind is drawn with, and a tint per `GasId` that is grey until the game names one.
`MapView` is a `viewport` of terminal cells and the world `origin` drawn at its top-left, with `center_on`, `clamp_to`, `to_screen` and `to_world`.
`clamp_to` is why the view never shows void past the edge of a map barely larger than it, and a map smaller than the viewport is centred in it instead, since there is nothing to scroll.
`LightOverlay` draws each visible tile's light as a digit, and only where there is a `Lighting` to read: a game with no lighting plugin sees the map drawn as it always is, since a world with no light to measure has nothing to put in the digit.
`Particles` is what is playing: `play` queues an `Animation` and refuses one that would take no time, `is_playing` says whether anything is on screen and `is_holding` whether any of it holds the turns, and `skip_held` drops what holds them and leaves the rest to fade.
An `Animation` is a `map`, a list of `steps` and whether it `holds`, and it is left behind when the player leaves that map.
A `Beat` is a `Trail`, a glyph flying between two anchors with three cells of fading tail, or a `Burst`, every anchor lit at once and fading, each cell showing one of a few glyphs picked by position so it reads as embers rather than a stamp.
`Beat::frame` answers with `Spark`s, a cell, a glyph, a colour and how far it has faded, and it is handed a resolver for anchors, so the line is redrawn every frame between where the two anchors are now and a flight at someone who walks on bends to follow them.
`ParticleStyle` is the pace and the look of anything with none of its own: the `plain` glyph and colour, seconds per cell of flight, seconds a burst takes, and the glyphs a burst picks from.
`ParticleStyle::instant` takes no time at all, which is what a headless test wants, and `takes_time` is what the plugin asks before it watches the turns.
`play_cues` turns the frame's `Cued` messages into one animation per actor, that actor's cues in the order they were written, each holding the turns.
`hold_turns` keeps the loop stopped exactly while something that holds it is playing and the player does not already hold a turn, and `skip_on_key` lets a key pressed during the wait drop every hold in the way, run the turns on, and still be read with the player's turn in hand.
`CapturePlugin` reads `RL_CAPTURE` for the path, `RL_CAPTURE_KEYS` for keys to press first, `RL_CAPTURE_FRAMES` for the earliest frame, and `RL_CAPTURE_AT=hold` to shoot a tenth of a second into the first hold rather than after it.
`capture::prepare` puts the window above the others and unfocused, so a capture never takes the keyboard from whoever is at the machine, and a frame that comes back entirely black is refused with an error rather than saved.

## Using it

Turning drawing on is one plugin group, and the rectangle the map gets is the only decision in it.

<!-- include: ../../../../examples/tutorial/src/bin/step01_walking.rs:main -->
```rust,no_run
fn main() -> AppExit {
    let mut app = App::new();
    // What every game adds: the window and the glyph terminal, the turn
    // loop, sight, the map across the whole terminal, and the UI base.
    app.add_plugins(RoguelikePlugins::new("Warren", COLS, ROWS))
        .insert_resource(Seed(RunSeed(7)))
        .add_systems(NewRun, start)
        // Once a frame, before the turns: whatever the player pressed becomes
        // at most one intent, however many passes the turn loop then runs.
        .add_systems(Update, player_input.in_set(EngineSet::Input));
    app.run()
}
```

What the engine cannot supply is the look of a tile, which is a table the game fills or a file it loads.

<!-- include: ../../../../examples/tutorial/src/bin/step08_content.rs:looks -->
```rust,no_run
    /// Both colours of every tile, and how much each cell strays from its
    /// neighbours, read from `assets/tiles.ron` against the tiles registered
    /// above. A tile the file forgets is reported at startup, by name.
    fn appearance(&self) -> TileAppearance {
        TileAppearance::load(TILES_RON, &self.tiles).unwrap_or_else(|e| panic!("assets/tiles.ron: {e}"))
    }
```

## The line

The engine draws; the game says what things look like.
A tile's colours, a prop's glyph and an item's are content, authored per tile or per definition, and the engine only darkens, fades, tints and jitters what it was handed.
That is why nothing here takes a `ToneId`: `Cell` and `Glyph` carry a `Color` outright, because a green slime is green in every palette and a floor's brown is not a semantic role.
A `ToneId` is the other half of the same rule, one layer up: a widget takes a role, `Palette` holds the colour per role, and a game restyles every panel at once by replacing one resource.
So the boundary falls between a thing and a word about a thing: the map and the entities on it are drawn in their own colours, and everything a panel says about them is drawn in a tone.
A prop is described twice for the same reason, and the second half is here: the engine spawns it with its kind and no glyph, and `dress_props` gives it the one its definition asks for, so nothing below this crate names a `Color`.
A prop a game dressed itself keeps what it was given, since the query asks only for those with no glyph.
Fire and gas are drawn only on tiles in sight, because memory holds no smoke, and what hides behind a haze is decided by the map's opacity rather than by the renderer's taste.
The engine will not animate a game's own action: a resolver says what is worth seeing by writing a `Cued`, and what plays it is a plugin that may not be there.
Without a watcher the cues are written and forgotten and the loop runs as if there were none, which is what a headless game gets and why the same rules run with and without a window.
The pace is the engine's and the look is the game's: `ParticleStyle` is one resource, and a game with a look of its own writes `Animation`s to `Particles` directly.
The terminal is not a widget toolkit and the map view is not a camera: there is no scene graph, no z-ordering beyond a glyph's layer, and no interpolation between turns.
One sprite and one text entity per cell is fine at a hundred columns and would be replaced by an instanced grid for a bigger one, and nothing above this crate would notice.

## Where it lives

`rl-render` is tier 2 and sits below `rl-ui` and above `rl-bevy`, which is what lets a panel and the map write into one `Terminal` without either knowing the other.
Keeping the shading in a module of its own makes it plain functions over a colour, a light level and a position, tested with no `App` and read by no rule.
The loader is separate again, so a game's colours are a file beside its monsters and a new tile is a line rather than a recompile.
The particles' arithmetic is `Beat::frame` and `Animation::frame` over a time and a resolver, which is why what a flight shows at a given moment is a test rather than a screenshot.
`rl-bevy` owns the cue and the hold, not the drawing, so a headless game raises the same cues and waits for nothing.
`CapturePlugin` is its own plugin because it is the one thing here that is not about a player: a game disables it the way Bevy's groups allow, and a run without the environment variable never notices it.
