<!-- documents:
     plugins: OverworldPlugin
     files: crates/rl-overworld/src/lib.rs
            crates/rl-bevy/src/fov.rs
            crates/rl-bevy/src/knowledge.rs
            crates/rl-bevy/src/world.rs
            crates/rl-world/src/graph.rs
            crates/rl-world/src/sites.rs
            crates/rl-render/src/terminal.rs
            crates/rl-bevy/src/plugin.rs
     fingerprint: 583f8d86 -->

# The overworld

The overworld is one screen: the world graph drawn at region scale, with the regions the player has been in sight of lit and a picker over the landmarks it has found.
It is a reading of what the game already holds, and it owns none of it, neither the player's position nor travel nor the fog it draws.
Choosing a site writes a message, and what that message means is the game's.
A game that leaves the plugin out loses a screen and nothing else.

## Turning it on

`OverworldPlugin` is opt-in and takes no arguments.
It declares `needs::<OverworldLayout>`, the terminal rectangle the map is drawn in, with a hint naming the shape to insert.
It declares the `overworld` modal, inserts `OverworldScreen`, `BandAppearance`, `OverworldStyle` and `OverworldKeys` at their defaults, adds `PortalRequest` as a message, and puts `handle_keys` in `EngineSet::Input` and `draw_overworld` in `PresentSet::Overlay`.
In `finish` it asserts that `CorePlugin` and `UiPlugin` were added, and lists its own keys on the controls screen under the heading `Map`, read after the game has finished building so a game that inserted its own `OverworldKeys` has those listed instead.
`BandAppearance` is the one table a game has to fill, because a band nobody set is drawn as a magenta `?`.
The screen reads `WorldRes`, `WorldMap` and `Knowledge`, so it belongs to a game with a streamed surface: a game of floors alone has no world graph for it to draw.

## The model

`OverworldScreen` is all the state the screen keeps, one `selected`, an index into the discovered-site list.
Whether the screen is open is deliberately not in it: that is on the shared `Modals` stack [Controls, modals and cursors](controls.md) describes, which is what stops this screen and a game's own from both believing they own the arrow keys.
`overworld_modal(&modals)` is its `ModalId` and panics when the plugin was not added, and `overworld_open` is the run condition a game gates its own input on.
`OverworldKeys` is `toggle`, `close`, `prev`, `next` and `go`, defaulting to `m`, escape, the two horizontal arrows and enter.
`handle_keys` toggles the screen only while it is top of the stack or nothing at all is open, so the map key does not open the map from inside an inventory, and it reads every other key only while the screen is top.
`go` writes a `PortalRequest` and closes the screen, and moves nothing itself.
`PortalRequest` carries one field, `site`, an index into `WorldRes`'s `sites()`, where a `Site` is a `SiteKindId` and the region it occupies.
`BandAppearance` is a table of `Cell` by `BandId`, the band a region was classified into, with `set` and `get` and that magenta fallback.
`OverworldStyle` is the colours the screen draws for itself rather than out of the table: rivers, roads, a discovered site, the selected one, the player's marker, regions never seen, and `show_unexplored`, which decides whether an untouched region is drawn dimmed or left blank.
`OverworldLayout` is one `Rect` of terminal cells.
`draw_overworld` runs only while the modal is open and writes into the `Terminal` in `PresentSet::Overlay`, the last layer of the frame, so it covers the map view and the panels under it.
It centres the viewport on the player's region when the world is wider than the rectangle, clamped to the world's edges, and sits on the world's corner when there is no marker to centre on.
Each cell is the region's band, then a `~` for a river, a `+` where a road runs, an `O` on a discovered site and an `@` on the player's own region, each of those keeping the background of what it replaced.
The marker is derived from the player's `Position` through `WorldRes::region_of_tile`, and is left out altogether when `WorldMap::current()` is not the surface, because a tile position on another map says nothing about which region it is under.
What the screen knows of the world is `Knowledge`: `region_touched` for the fog, and `site_discovered` and `discovered_sites` for the list and the markers.
Those are filled by `update_viewsheds` in `FovPlugin`, which touches a region and discovers any site in it for every tile a `RevealsMap` viewer sees while on the surface, so the overworld shows nothing the player has not looked at.

## Using it

A game answers a `PortalRequest` however its own fiction says travel works, and corsair's answer is a warp to the middle of the chosen site's region, from wherever the player happens to be.

<!-- include: ../../../../examples/corsair/src/main.rs:portals -->
```rust,no_run
/// Asks the engine to move the player to a discovered site when the
/// overworld asks, from wherever the player is, a cave included.
fn honour_portals(
    mut requests: MessageReader<PortalRequest>,
    mut warps: MessageWriter<WarpRequest>,
    world: Res<WorldRes>,
    mut log: ResMut<MessageLog>,
    turns: Res<Turns>,
    player: Query<Entity, With<Player>>,
) {
    let Ok(entity) = player.single() else { return };
    for req in requests.read() {
        let Some(site) = world.sites().get(req.site) else { continue };
        let target = world.region_tiles(site.position).center();
        warps.write(WarpRequest { actor: entity, to: Destination::Surface(target) });
        log.push("The portal takes you.", Tones::NOTICE, turns.turn_number());
    }
}
```

The table of how each band looks is the part of turning the screen on that the plugin cannot default, and the numbering in it is the game's from end to end.

<!-- include: ../../../../examples/corsair/src/content.rs:bands -->
```rust,no_run
    pub fn band_appearance(&self) -> BandAppearance {
        let mut look = BandAppearance::new();
        look.set(SEA, Cell::new('~', Color::srgb(0.2, 0.35, 0.7)).on(Color::srgb(0.03, 0.08, 0.2)));
        look.set(LAKE, Cell::new('=', Color::srgb(0.3, 0.55, 0.9)));
        look.set(BEACH, Cell::new(':', Color::srgb(0.85, 0.78, 0.5)));
        look.set(GRASS, Cell::new('.', Color::srgb(0.35, 0.65, 0.3)));
        look.set(JUNGLE, Cell::new('T', Color::srgb(0.15, 0.5, 0.2)));
        look.set(DUNES, Cell::new(',', Color::srgb(0.85, 0.7, 0.4)));
        look.set(MANGROVE, Cell::new('"', Color::srgb(0.3, 0.5, 0.4)));
        look.set(HILL, Cell::new('n', Color::srgb(0.55, 0.6, 0.35)));
        look.set(MOUNTAIN, Cell::new('A', Color::srgb(0.6, 0.55, 0.5)));
        look.set(VOLCANO, Cell::new('^', Color::srgb(0.95, 0.95, 1.0)));
        look
    }
```

## The line

The overworld is for looking: a picture of the world graph at region scale, and a list of the places the player has found in it.
It is not travel, not a second map and not a record of anything.
It writes no `Position`, switches no map and asks for no chunk; the one thing it writes is a `PortalRequest`, and a game that reads no such message is left with a screen that draws and does nothing.
That is the seam with places and streaming: [Places and streaming](places.md) owns which map is current, which window of regions is loaded and what a warp does, and the overworld reads the outcome rather than taking any part in it.
The regions the screen draws are the same regions the window is measured in, and drawing one neither loads it nor keeps it loaded, since a band, a river and a road belong to the world graph and are known without generating a tile.
So the screen shows the whole world while the game holds a few regions of it, which is the reason to have it at all.
The engine decides which keys the screen answers, that it answers them only while it is the top modal, and that choosing a site is a request rather than a move.
The game decides what a band looks like, what a site is, what a portal costs, whether it is refused, and whether there is a portal to ask for.
The fog is the engine's, but it is filled by sight rather than by this screen: a region is touched when a revealer sees a tile in it and a site is discovered the same way, so the map is a record of where the player has been rather than something handed out at the start.
Colours here are a `Color` in a resource rather than a `ToneId` in a palette, unlike the widgets [Panels](panels.md) covers, because this is a screen of its own and not a widget a game composes; a game that wants other colours replaces `OverworldStyle`.

## Where it lives

`rl-overworld` is tier 2 and a crate of its own rather than a module of `rl-ui`, because nothing depends on it: the screen is one resource of state, four tables and two systems, and dropping the plugin costs a game no other line.
Being separate is what keeps `rl-ui` clear of `rl-world` as well, since a panel crate that knew what a river was would be one that a game with no surface still paid for.
What the split buys downwards is that everything drawn here is already tested below it: bands, rivers, roads and sites are `rl-world`'s and are properties of a generated `WorldGraph` checked over a range of seeds with no `App`, and the fog is `Knowledge`'s, round-tripped through a save.
What is left in this crate is the frame-shaped part: which modal is on top, where the viewport is centred, and which glyph each cell ends up with.
