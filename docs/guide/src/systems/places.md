<!-- documents:
     plugins: StreamingPlugin
     files: crates/rl-bevy/src/places.rs
            crates/rl-bevy/src/world.rs
            crates/rl-bevy/src/turn.rs
            crates/rl-bevy/src/plugin.rs
            crates/rl-bevy/src/knowledge.rs
            crates/rl-bevy/src/minds.rs
            crates/rl-world/src/chunk.rs
            crates/rl-world/src/graph.rs
     fingerprint: b71ec517 -->

# Places and streaming

A game's maps are of two kinds, and everything that reads tiles reads both through one resource.
A place is bounded, built once by the game's rules the first time something enters it, and kept whole from then on, actors and items included.
The surface is streamed: a window of regions around the player, generated on demand, dropped when it leaves the window, and remembered only as the edits made to it.
Every entity with a position is on exactly one map, and the turn loop freezes whatever is not on the one being played.

## Turning it on

Places need no plugin: `CorePlugin` puts `resolve_warps` in `ResolveSet::Travel` and `tag_new_positions` in `TurnSet::Schedule` before anyone is dealt a turn, and declares `needs::<WorldMap>`, so a game that builds no map is told so by name when play begins.
What a game adds is a `PlaceRulesRes` wrapping its `PlaceRules`, and the engine calls it the first time a map id is entered.
`StreamingPlugin` is the surface, and it is the part a game leaves out: it adds `stream_chunks` in `EngineSet::Stream` and declares `needs::<WorldRes>` and `needs::<ChunkRulesRes>`, each with a hint naming what to build one from.
`EngineSet::Stream` runs first in the frame so the window is loaded around wherever the player ended the previous one before anybody is dealt a turn on it.
A game of floors and nothing above them adds neither: with no `WorldRes` nothing streams, `WorldMap::new` takes the tile tables and nothing else, and how big a region is never enters its vocabulary.
`WorldSettings` is how much stays loaded, a `window_radius` of 1 giving the 3x3 default.

## The model

`MapId` is a `u32`, `MapId::SURFACE` is zero, and a game numbers its places however it likes above that.
`OnMap` is the component saying which; it is missing until something first gains a position, and `tag_new_positions` fills it in with the current map.
`WorldMap` is the resource every reader reads: `tile`, `set_tile`, `is_walkable`, `is_opaque`, `blocks_projectiles`, `cost`, `opens`, `closes` and `is_loaded` all answer for the current map, whichever kind it is, and `None` or opaque-and-impassable for a cell that is not loaded.
`current`, `window`, `window_tiles`, `to_world` and `to_local` say which map and which window; `view` and `opening_view` hand a `WindowView` to a grid algorithm, the second costing a closed door as the turn spent opening it plus the step onto what it opens into, so a flood routes through a door when that is shorter.
Three counters tell readers when to recompute without anyone asking them: `generation` moves when the window moves or the map changes, `opacity_epoch` when an edit changes what blocks sight, and `cost_epoch` when one changes where an actor may walk or what a step costs.
`set_veil` is the fourth case, a set of cells that hide what is behind them for a reason other than their tile, rewritten each turn by whatever makes it and lifted whenever the map or the window under it changes.
`PlaceRules::build` is the one method a game writes, taking a `MapId` and the `WorldGraph` when there is one, and returning a `PlaceBuild`: a `Terrain`, an `entry`, an optional `exit`, and a `Vec<Spot>` of points of interest tagged in the game's own numbering.
`PlaceBuild::from_context` is the usual way to make one from a finished chain, taking the chain's `StartPoint` as the entry, its `ExitPoint` as the exit, and every prefab mark as a `Spot` tagged with its character.
A mark inside the bounds of a stamp emitted after its own is dropped, keyed or not, because the piece drawn on top owns its cells.
A `Spot` from a keyed piece carries its `prefab`, the key set on the `Prefab` that stamped it, so the game can trace it back to what defined it; `None` for a spot a game made itself or a mark of an unkeyed piece.
`Transition` is a component on an entity standing on a cell, holding the `Destination` it leads to: a cell of the surface, or a place with an `Arrive` of its entry, its exit or a named cell.
`GoThrough` is the action that takes an actor through the one it is standing on, and `WarpRequest` does the same from anywhere for a portal or a first arrival, resolved without charging a turn so the game charges what it likes.
The player always may go through; anyone else is refused unless its `Intelligence` has `Wits::TRAVELS`, which no preset has, while a `WarpRequest` moves anyone, since that is the game doing the moving.
A mind that travels, stood beside an actor as it went through, and was after it or beside it on its own last turn goes through behind it in the same pass; a warp is never followed.
An arrival on a taken cell lands on the nearest free one that can be walked to from it, so a follower stands beside whom it followed.
A warp that lands on a map nobody has built calls the rules and installs the result with `install_place`, whoever arrived; `PlaceEntered` is written for the player alone, with `first` true on the player's first arrival, which is the one arrival on which a game populates a floor.
`MapChanged` is written when the player changes map and `Travelled` when anyone does, naming the cell it left and whom it went after.
The player's warp also switches the per-map indexes with the map: `Occupancy` swaps its `SpatialGrid` for the arriving map's and stashes the one it had, `Knowledge` does the same with explored tiles and the still things last seen on them, and `FlowFields` is invalidated so the next mind rebuilds.
On the streaming side, `stream_chunks` asks `desired_window` for the square of radius `window_radius` around the player's region clipped to the world, and does nothing if that is the window already or if the player is in a place.
Loading generates each new region through `WorldGraph::build_chunk` and the game's `ChunkRules`, replays that region's stored delta onto it, keeps every chunk still in the window, and files the edits of the ones that left.
`ChunkLoaded` is written once per region generated, and every viewshed is marked stale when the window moves.
`export` and `import` are the save shape: every surface edit, loaded or not, and every built place whole, with loaded chunks dropped on import so the next stream replays the edits onto fresh generation.

## Using it

A game reaches a place by spawning a `Transition` on a cell, and where those cells are is the game's to decide: Foundry lays a deck's lifts on the entry and the exit the deck was built with, the first time it is entered.

<!-- include: ../../../../examples/foundry/src/lifts.rs:lifts -->
```rust,no_run
/// Lays a deck's lifts the first time it is entered, and names the deck in
/// the log every time. On the run's first arrival it also says how to see
/// the keys, under the deck's own line: in `run::start` it would be
/// written before the warp lands and read as if it came first.
pub fn link_decks(mut commands: Commands, mut entered: MessageReader<PlaceEntered>, help: Res<ControlsKeys>, mut tell: MessageWriter<Tell>) {
    for ev in entered.read() {
        let deck = deck_of(ev.map);
        tell.write(Tell::new(deck_line(deck), Tones::NOTICE));
        if !ev.first {
            continue;
        }
        if deck == 1 {
            tell.write(Tell::new(format!("Press {} for the controls.", help.toggle.label()), Tones::MUTED));
        }
        if deck > 1 {
            let up = Transition { to: Destination::Place { map: map_of(deck - 1), arrive: Arrive::Exit } };
            commands.spawn((Position(ev.entry), OnMap(ev.map), Lift, up, Name::new("lift up"), Glyph::new('<', LIFT).on_layer(1)));
        } else {
            // The way out, where the commando came in.
            commands.spawn((Position(ev.entry), OnMap(ev.map), Lift, LiftOut, Name::new("lift out"), Glyph::new('<', LIFT).on_layer(1)));
        }
        if let Some(exit) = ev.exit.filter(|_| deck < DECKS) {
            let down = Transition { to: Destination::Place { map: map_of(deck + 1), arrive: Arrive::Entry } };
            commands.spawn((Position(exit), OnMap(ev.map), Lift, down, Name::new("lift down"), Glyph::new('>', LIFT).on_layer(1)));
        }
    }
}
```

No example game streams a surface yet, so none listens for `ChunkLoaded` to place a way into a region as it loads.

## The line

The engine decides when a place is built, which is the first time anything enters it, and that it is never built twice.
It decides that a place is kept whole once built, so leaving one freezes what is in it and returning finds it as it was, and that the surface is not: a region outside the window is thrown away and only its edits survive.
It decides that a chunk is a pure function of the run seed and its region, through `WorldGraph::chunk_seed`, so a region regenerates identically however many times it has been walked across, which is what makes storing a delta instead of a map correct rather than merely cheap.
It decides that the map being read follows the player and nobody else, that an actor on another map or outside the loaded window is frozen and requeued a full step without acting, and that such an actor can neither act nor be seen.
It decides who follows whom through a way and where each lands; the game decides which kinds travel at all, by writing `travels` in their wits.
The game decides what a map id means, what is built there, where the transitions stand and what they lead to, what an arrival costs, and everything that goes in a place on the arrival `PlaceEntered` marks as its first.
The game also decides whether there is a surface at all, and that decision is made by adding `StreamingPlugin` or not rather than by any resource being present or absent.
Which regions are in the window is the engine's; what a region generates into is the game's `ChunkRules`, and the seam where two chunks meet is arithmetic both sides compute from the same unordered pair of regions rather than a negotiation either could lose.
A game that wants a region populated the first time it is seen listens for `ChunkLoaded` and remembers which it has done, because the engine will load a region again after it has been unloaded and will not tell it apart from the first time.

## Where it lives

`rl-bevy` is tier 2 and owns both halves, because both are about what the running game is reading right now, which is not a question a lower tier can answer.
`places.rs` is the vocabulary and the warp; `world.rs` is `WorldMap`, the window, the chunk store and `StreamingPlugin`.
The part that can be tested without an `App` was pushed below: `rl-world` is tier 1 and owns the world graph, the chunk builder and the seam arithmetic, so that two neighbouring chunks agreeing at their shared edge is a property checked over a range of region pairs by calling `build_chunk` twice, with no frame anywhere.
What is left above the line is genuinely frame-shaped: which map is current, which window is loaded, and which indexes have to be swapped when that changes.
Keeping the surface's whole existence optional is the other thing the split buys, since a game of floors alone links `rl-world` without ever constructing a `WorldGraph`, and nothing in its vocabulary mentions a region.
